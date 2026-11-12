//! S4 native Off/Observe decisions and explicit local thread-to-task links.
//! No inbound GitHub authentication, automatic subscription delivery,
//! inferred worker admission or Auto tool effects exist here.
//! The public IPC intake refuses caller-supplied envelopes; the private
//! decision logic is exercised only with synthetic unit-test events.
use super::require_main_window;
use crate::{storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Additive schema. No session, Ticket, bot profile or old authorization is changed.
pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_event_sources (
  id TEXT PRIMARY KEY,
  orchestration_id TEXT NOT NULL REFERENCES sa_orchestrations(id) ON DELETE RESTRICT,
  repository TEXT NOT NULL,
  actor_filter TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  mode TEXT NOT NULL DEFAULT 'off' CHECK(mode IN ('off','observe')),
  policy_revision INTEGER NOT NULL DEFAULT 1,
  last_sequence INTEGER NOT NULL DEFAULT 0 CHECK(last_sequence>=0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(orchestration_id,repository,actor_filter,event_kind)
);
CREATE TABLE IF NOT EXISTS sa_event_decisions (
  source_id TEXT NOT NULL REFERENCES sa_event_sources(id) ON DELETE RESTRICT,
  delivery_id TEXT NOT NULL,
  repository TEXT NOT NULL,
  actor TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  subject_key TEXT NOT NULL,
  sequence INTEGER NOT NULL CHECK(sequence>0),
  decision TEXT NOT NULL CHECK(decision IN ('ignored_off','ignored_filter',
    'ignored_stale','observed_permitted')),
  policy_revision INTEGER NOT NULL,
  at INTEGER NOT NULL,
  PRIMARY KEY(source_id,delivery_id)
);
CREATE INDEX IF NOT EXISTS sa_event_decisions_source_cursor
  ON sa_event_decisions(source_id,sequence);
"#;


/// A separate additive v42 migration, leaving v41 receipt histories untouched.
/// Links are local-user references to source threads, never event authority.
pub(crate) const THREAD_LINK_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_source_thread_links (
  source_id TEXT NOT NULL REFERENCES sa_event_sources(id) ON DELETE RESTRICT,
  subject_key TEXT NOT NULL,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(source_id,subject_key)
);
CREATE INDEX IF NOT EXISTS sa_thread_links_ticket
  ON sa_source_thread_links(ticket_id,created_at);
"#;

const MAX_LINKS_PER_TICKET: i64 = 16;
const MAX_SOURCES: i64 = 64;
const MAX_RECEIPTS: i64 = 10_000;
const EVENT_KINDS: &[&str] = &[
    "issues.opened", "issues.edited", "issue_comment.created",
    "pull_request.opened", "pull_request_review.submitted",
    "check_run.completed",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSource {
    pub id: String,
    pub orchestration_id: String,
    pub repository: String,
    pub actor_filter: String,
    pub event_kind: String,
    pub mode: String,
    pub policy_revision: i64,
    pub last_sequence: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventEnvelope {
    pub source_id: String,
    pub delivery_id: String,
    pub repository: String,
    pub actor: String,
    pub event_kind: String,
    pub subject_key: String,
    /// Monotonic cursor issued by a verified upstream adapter. GitHub's
    /// delivery ID alone is not a sequence or an authorization token.
    pub sequence: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDecision {
    pub source_id: String,
    pub delivery_id: String,
    pub decision: String,
    pub policy_revision: i64,
    pub sequence: i64,
    pub at: i64,
    pub duplicate: bool,
}

/// Bounded stored decisions only; the untrusted webhook body and private
/// task transcripts are never persisted or returned by this view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDecisionRecord {
    pub source_id: String,
    pub delivery_id: String,
    pub actor: String,
    pub event_kind: String,
    pub subject_key: String,
    pub sequence: i64,
    pub decision: String,
    pub policy_revision: i64,
    pub at: i64,
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceThreadLink {
    pub source_id: String,
    pub ticket_id: String,
    pub repository: String,
    pub subject_key: String,
    pub created_at: i64,
}

/// Links may refer only to an exact issue or PR number, not URLs, arbitrary
/// text or any user-supplied action. Local approval of a link never authorizes
/// an incoming webhook, a task transition or a tool call.
fn valid_thread_key(value: &str) -> bool {
    let Some((kind, number)) = value.split_once(':') else { return false };
    matches!(kind, "issue" | "pull") && (1..=12).contains(&number.len())
        && !number.starts_with('0') && number.bytes().all(|c| c.is_ascii_digit())
}

fn list_thread_links(store: &Store, ticket_id: &str) -> Result<Vec<SourceThreadLink>> {
    super::get_ticket(store, ticket_id)?;
    let mut statement = store.db.prepare(
        "SELECT l.source_id,l.ticket_id,s.repository,l.subject_key,l.created_at
         FROM sa_source_thread_links l
         JOIN sa_event_sources s ON s.id=l.source_id
         WHERE l.ticket_id=?1 ORDER BY l.created_at,l.source_id,l.subject_key
         LIMIT 16"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([ticket_id], |row| Ok(SourceThreadLink {
        source_id: row.get(0)?, ticket_id: row.get(1)?,
        repository: row.get(2)?, subject_key: row.get(3)?,
        created_at: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

fn link_thread(store: &Store, source_id: &str, ticket_id: &str, subject_key: &str)
    -> Result<SourceThreadLink>
{
    if !valid_thread_key(subject_key) {
        return Err("A source link requires an exact issue:123 or pull:123 thread key.".into());
    }
    let ticket = super::get_ticket(store, ticket_id)?;
    let src = source(store, source_id)?;
    if src.orchestration_id != ticket.orchestration_id {
        return Err("Event source and Ticket must belong to the same orchestration.".into());
    }
    if matches!(ticket.state.as_str(), "completed" | "stopped" | "failed"
        | "interrupted" | "inconclusive")
    {
        return Err("Terminal Tickets cannot gain new source thread links.".into());
    }
    let current: Option<String> = store.db.query_row(
        "SELECT ticket_id FROM sa_source_thread_links
         WHERE source_id=?1 AND subject_key=?2",
        params![source_id,subject_key], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(existing) = current {
        if existing != ticket_id {
            return Err("Source thread is already linked to a different Ticket.".into());
        }
    } else {
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_source_thread_links WHERE ticket_id=?1",
            [ticket_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if count >= MAX_LINKS_PER_TICKET {
            return Err("Ticket source thread link limit reached.".into());
        }
        store.db.execute(
            "INSERT INTO sa_source_thread_links
             (source_id,subject_key,ticket_id,created_at) VALUES(?1,?2,?3,?4)",
            params![source_id,subject_key,ticket_id,storage::now()],
        ).map_err(|e| e.to_string())?;
    }
    list_thread_links(store, ticket_id)?.into_iter()
        .find(|link| link.source_id == source_id && link.subject_key == subject_key)
        .ok_or_else(|| "Source thread link unavailable after update.".into())
}

fn token_ok(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max
        && value.is_ascii() && value.bytes().all(|b|
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'[' | b']'))
}

fn repo_ok(repo: &str) -> bool {
    if repo.len() > 200 || repo != repo.to_ascii_lowercase() { return false; }
    let Some((owner, name)) = repo.split_once('/') else { return false };
    token_ok(owner, 80) && !name.is_empty() && name.len() <= 100
        && !name.starts_with('.') && !name.contains("..")
        && name.bytes().all(|b|
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn source_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventSource> {
    Ok(EventSource {
        id: row.get(0)?, orchestration_id: row.get(1)?,
        repository: row.get(2)?, actor_filter: row.get(3)?,
        event_kind: row.get(4)?, mode: row.get(5)?,
        policy_revision: row.get(6)?, last_sequence: row.get(7)?,
    })
}
fn source(store: &Store, id: &str) -> Result<EventSource> {
    store.db.query_row(
        "SELECT id,orchestration_id,repository,actor_filter,event_kind,
                mode,policy_revision,last_sequence FROM sa_event_sources WHERE id=?1",
        [id], source_row,
    ).map_err(|_| "SuperAgent event source unavailable.".into())
}
fn list_sources(store: &Store, orchestration_id: &str) -> Result<Vec<EventSource>> {
    super::configuration(store, orchestration_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,orchestration_id,repository,actor_filter,event_kind,
                mode,policy_revision,last_sequence FROM sa_event_sources
         WHERE orchestration_id=?1 ORDER BY created_at,id",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([orchestration_id], source_row).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

/// A user creates an exact, non-wildcard source in Off. Reusing the same
/// config is idempotent, never an implicit enable/revision change.
fn create_source(
    store: &Store, orchestration_id: &str, repository: &str,
    actor_filter: &str, event_kind: &str,
) -> Result<EventSource> {
    super::configuration(store, orchestration_id)?;
    if !repo_ok(repository) || !token_ok(actor_filter, 80)
        || !EVENT_KINDS.contains(&event_kind)
    {
        return Err("Source requires an exact canonical repository, actor and supported event class.".into());
    }
    let existing: Option<String> = store.db.query_row(
        "SELECT id FROM sa_event_sources
         WHERE orchestration_id=?1 AND repository=?2 AND actor_filter=?3 AND event_kind=?4",
        params![orchestration_id,repository,actor_filter,event_kind], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(id) = existing { return source(store, &id); }
    let count: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM sa_event_sources WHERE orchestration_id=?1",
        [orchestration_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if count >= MAX_SOURCES { return Err("SuperAgent source limit reached.".into()); }
    let id = storage::id();
    let now = storage::now();
    store.db.execute(
        "INSERT INTO sa_event_sources(id,orchestration_id,repository,actor_filter,event_kind,
            mode,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,'off',?6,?6)",
        params![id,orchestration_id,repository,actor_filter,event_kind,now],
    ).map_err(|e| e.to_string())?;
    source(store, &id)
}

/// Policy revision is native authority. Auto is deliberately unsupported until
/// its provider-side grant, rate limit and authentic GitHub intake exist.
fn set_mode(store: &Store, id: &str, expected_revision: i64, mode: &str) -> Result<EventSource> {
    if mode != "off" && mode != "observe" {
        return Err("Auto is unavailable without an authenticated source and native side-effect grants.".into());
    }
    let current = source(store,id)?;
    if current.policy_revision != expected_revision {
        return Err("Event source policy revision changed; reload before editing.".into());
    }
    if current.mode == mode { return Ok(current); }
    let now = storage::now();
    let updated = store.db.execute(
        "UPDATE sa_event_sources SET mode=?2,policy_revision=policy_revision+1,updated_at=?3
         WHERE id=?1 AND policy_revision=?4",
        params![id,mode,now,expected_revision],
    ).map_err(|e| e.to_string())?;
    if updated != 1 { return Err("Concurrent source policy change; update rejected.".into()); }
    source(store,id)
}

fn valid_event(event: &EventEnvelope) -> bool {
    token_ok(&event.delivery_id, 128) && repo_ok(&event.repository)
        && token_ok(&event.actor, 80) && EVENT_KINDS.contains(&event.event_kind.as_str())
        && !event.subject_key.is_empty() && event.subject_key.len() <= 200
        && !event.subject_key.chars().any(char::is_control)
        && event.sequence > 0
}

/// Persist a bounded, policy-revisioned **decision only**. Never write a
/// Ticket, launch a provider, publish a reply or claim GitHub authenticity.
/// A delivery ID is immutable: replay with changed fields is rejected even
/// after mode changes, restart, or later cursor advancement.
fn observe(store: &Store, event: &EventEnvelope) -> Result<EventDecision> {
    if !valid_event(event) {
        return Err("Invalid, unsupported or unbounded source event envelope.".into());
    }
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let src = source(store, &event.source_id)?;
    type Existing = (String,String,String,String,i64,String,i64,i64);
    let existing: Option<Existing> = tx.query_row(
        "SELECT repository,actor,event_kind,subject_key,sequence,decision,policy_revision,at
         FROM sa_event_decisions WHERE source_id=?1 AND delivery_id=?2",
        params![&event.source_id,&event.delivery_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
               r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)),
    ).optional().map_err(|e| e.to_string())?;
    if let Some((repository,actor,kind,subject,seq,decision,policy_revision,at)) = existing {
        if repository != event.repository || actor != event.actor
            || kind != event.event_kind || subject != event.subject_key || seq != event.sequence
        {
            return Err("Reused source delivery ID has a conflicting envelope.".into());
        }
        return Ok(EventDecision { source_id:event.source_id.clone(),
            delivery_id:event.delivery_id.clone(), decision,policy_revision,
            sequence:seq,at,duplicate:true });
    }
    let receipts: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sa_event_decisions WHERE source_id=?1",
        [&event.source_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if receipts >= MAX_RECEIPTS { return Err("Source decision receipt limit reached; no event admitted.".into()); }

    let decision = if event.sequence <= src.last_sequence { "ignored_stale" }
        else if src.mode == "off" { "ignored_off" }
        else if src.repository != event.repository || src.actor_filter != event.actor
            || src.event_kind != event.event_kind { "ignored_filter" }
        else { "observed_permitted" };
    let at = storage::now();
    tx.execute(
        "INSERT INTO sa_event_decisions(source_id,delivery_id,repository,actor,event_kind,
          subject_key,sequence,decision,policy_revision,at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![&event.source_id,&event.delivery_id,&event.repository,&event.actor,
          &event.event_kind,&event.subject_key,event.sequence,decision,src.policy_revision,at],
    ).map_err(|e| e.to_string())?;
    // Advance the source cursor even for Off and ignored filters, preventing
    // old data from being reclassified as authorized after a mode change.
    if event.sequence > src.last_sequence {
        tx.execute(
            "UPDATE sa_event_sources SET last_sequence=?2 WHERE id=?1 AND last_sequence<?2",
            params![&event.source_id,event.sequence],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(EventDecision { source_id:event.source_id.clone(),
        delivery_id:event.delivery_id.clone(),decision:decision.into(),
        policy_revision:src.policy_revision,sequence:event.sequence,at,duplicate:false })
}

/// A source-scoped, restart-durable decision timeline. Neither the read nor
/// its main-window command can change policy, admit tasks or dispatch work.
fn list_decisions(store: &Store, source_id: &str, limit: i64) -> Result<Vec<EventDecisionRecord>> {
    if !(1..=100).contains(&limit) {
        return Err("Event decision page size must be between 1 and 100.".into());
    }
    source(store, source_id)?;
    let mut stmt = store.db.prepare(
        "SELECT source_id,delivery_id,actor,event_kind,subject_key,sequence,
                decision,policy_revision,at FROM sa_event_decisions
         WHERE source_id=?1 ORDER BY sequence DESC,delivery_id DESC LIMIT ?2",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![source_id,limit], |row| {
        Ok(EventDecisionRecord {
            source_id:row.get(0)?, delivery_id:row.get(1)?,
            actor:row.get(2)?, event_kind:row.get(3)?,
            subject_key:row.get(4)?, sequence:row.get(5)?,
            decision:row.get(6)?, policy_revision:row.get(7)?,
            at:row.get(8)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn superagent_source_decisions(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    source_id: String, limit: i64,
) -> Result<Vec<EventDecisionRecord>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_decisions(&store,&source_id,limit)
}

#[tauri::command]
pub async fn superagent_source_create(
    window: tauri::WebviewWindow, state: State<'_,AppState>,
    orchestration_id: String, repository: String, actor_filter: String, event_kind: String,
) -> Result<EventSource> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    create_source(&store,&orchestration_id,&repository,&actor_filter,&event_kind)
}

#[tauri::command]
pub async fn superagent_source_list(
    window: tauri::WebviewWindow, state: State<'_,AppState>, orchestration_id: String,
) -> Result<Vec<EventSource>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_sources(&store,&orchestration_id)
}

#[tauri::command]
pub async fn superagent_source_set_mode(
    window: tauri::WebviewWindow, state: State<'_,AppState>,
    source_id: String, expected_revision: i64, mode: String,
) -> Result<EventSource> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    set_mode(&store,&source_id,expected_revision,&mode)
}

#[tauri::command]
pub async fn superagent_source_observe(
    window: tauri::WebviewWindow, _state: State<'_,AppState>, _event: EventEnvelope,
) -> Result<EventDecision> {
    require_main_window(&window)?;
    superagent_source_delivery_unavailable()
}

fn superagent_source_delivery_unavailable() -> Result<EventDecision> {
    Err("GitHub event delivery is unavailable: no authenticated delivery adapter is configured.".into())
}


#[tauri::command]
pub async fn superagent_source_link_thread(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    source_id: String, ticket_id: String, subject_key: String,
) -> Result<SourceThreadLink> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    link_thread(&store, &source_id, &ticket_id, &subject_key)
}

#[tauri::command]
pub async fn superagent_source_thread_links(
    window: tauri::WebviewWindow, state: State<'_, AppState>, ticket_id: String,
) -> Result<Vec<SourceThreadLink>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_thread_links(&store, &ticket_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Store, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("sa-event-{}.sqlite3",storage::id()));
        let store = Store::open(&path).unwrap();
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_orchestrations
               (id,name,coordinator_bot_id,worker_bot_id,project_id,created_at,updated_at)
             VALUES('orch','Disposable','router','worker','fixture',?1,?1)",
            [now],
        ).unwrap();
        (store,path)
    }
    fn event(id: &str, source: &EventSource, seq: i64) -> EventEnvelope {
        EventEnvelope {
            source_id:source.id.clone(),delivery_id:id.into(),
            repository:"test/fixture".into(),actor:"trusted-bot".into(),
            event_kind:"issues.opened".into(),subject_key:"issue:42".into(),
            sequence:seq,
        }
    }
    #[test]
    fn off_and_observe_are_durable_deduplicated_and_never_launch_tasks() {
        let (store,path) = fixture();
        let off = create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        assert_eq!(off.mode,"off");
        assert_eq!(create_source(&store,"orch","test/fixture","trusted-bot","issues.opened")
            .unwrap().id,off.id);
        let first = observe(&store,&event("delivery-1",&off,1)).unwrap();
        assert_eq!(first.decision,"ignored_off");
        let observed = set_mode(&store,&off.id,off.policy_revision,"observe").unwrap();
        assert_eq!(observed.policy_revision,off.policy_revision+1);
        let again = observe(&store,&event("delivery-1",&observed,1)).unwrap();
        assert!(again.duplicate);
        assert_eq!(again.decision,"ignored_off");
        assert_eq!(again.policy_revision,off.policy_revision);
        let decision = observe(&store,&event("delivery-2",&observed,2)).unwrap();
        assert_eq!(decision.decision,"observed_permitted");
        assert!(!decision.duplicate);
        assert_eq!(source(&store,&off.id).unwrap().last_sequence,2);
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r| r.get(0),
        ).unwrap();
        assert_eq!(count,0,"Observe cannot create a Ticket");
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(source(&reopened,&off.id).unwrap().mode,"observe");
        assert_eq!(source(&reopened,&off.id).unwrap().last_sequence,2);
        assert!(observe(&reopened,&event("delivery-2",&observed,2)).unwrap().duplicate);
    }
    #[test]
    fn stale_mismatched_and_conflicting_events_never_infer_authority() {
        let (store,_) = fixture();
        let off = create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        assert!(set_mode(&store,&off.id,off.policy_revision,"auto").is_err());
        assert!(set_mode(&store,&off.id,off.policy_revision,"observe").is_ok());
        assert!(set_mode(&store,&off.id,off.policy_revision,"off").is_err());
        let mut mismatched = event("wrong-actor",&off,1);
        mismatched.actor = "outsider".into();
        let result = observe(&store,&mismatched).unwrap();
        assert_eq!(result.decision,"ignored_filter");
        let older = observe(&store,&event("older",&off,1)).unwrap();
        assert_eq!(older.decision,"ignored_stale");
        let mut tampered = mismatched.clone();
        tampered.subject_key = "issue:99".into();
        assert!(observe(&store,&tampered).is_err());
        let valid = observe(&store,&event("valid",&off,2)).unwrap();
        assert_eq!(valid.decision,"observed_permitted");
        let count: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_tickets",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
    }
    #[test]
    fn caller_supplied_event_intake_fails_closed_without_authenticated_adapter() {
        let (store,_) = fixture();
        let cfg = create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        assert!(superagent_source_delivery_unavailable().is_err());
        assert_eq!(source(&store,&cfg.id).unwrap().last_sequence, 0);
        let receipts: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_event_decisions WHERE source_id=?1",
            [&cfg.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(receipts, 0);
    }
    #[test]
    fn failed_receipt_insert_rolls_back_cursor_and_is_safe_to_retry() {
        let (store,_) = fixture();
        let cfg = create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        let cfg = set_mode(&store,&cfg.id,cfg.policy_revision,"observe").unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_observe BEFORE INSERT ON sa_event_decisions
             BEGIN SELECT RAISE(ABORT,'simulated receipt failure'); END;",
        ).unwrap();
        assert!(observe(&store,&event("failed",&cfg,1)).is_err());
        assert_eq!(source(&store,&cfg.id).unwrap().last_sequence,0);
        store.db.execute_batch("DROP TRIGGER reject_observe").unwrap();
        assert_eq!(observe(&store,&event("failed",&cfg,1)).unwrap().decision,"observed_permitted");
        assert_eq!(source(&store,&cfg.id).unwrap().last_sequence,1);
    }
    #[test]
    fn decision_timeline_is_bounded_source_scoped_and_restart_durable() {
        let (store,path)=fixture();
        let a=create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        let b=create_source(&store,"orch","test/fixture","other-bot","issues.opened").unwrap();
        let a=set_mode(&store,&a.id,a.policy_revision,"observe").unwrap();
        let b=set_mode(&store,&b.id,b.policy_revision,"observe").unwrap();
        observe(&store,&event("one",&a,1)).unwrap();
        observe(&store,&event("two",&a,2)).unwrap();
        let mut other=event("foreign",&b,1);
        other.actor="other-bot".into();
        observe(&store,&other).unwrap();
        assert_eq!(list_decisions(&store,&a.id,1).unwrap()[0].delivery_id,"two");
        assert_eq!(list_decisions(&store,&a.id,100).unwrap().len(),2);
        assert_eq!(list_decisions(&store,&b.id,100).unwrap().len(),1);
        for invalid in [0,-1,101] { assert!(list_decisions(&store,&a.id,invalid).is_err()); }
        assert!(list_decisions(&store,"unknown",10).is_err());
        let count:i64=store.db.query_row("SELECT COUNT(*) FROM sa_tickets",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
        drop(store);
        let reopened=Store::open(&path).unwrap();
        let timeline=list_decisions(&reopened,&a.id,100).unwrap();
        assert_eq!(timeline.len(),2);
        assert_eq!(timeline[0].delivery_id,"two");
        assert_eq!(timeline[1].policy_revision,a.policy_revision);
    }

    fn insert_manual_ticket(store: &Store, id: &str, orchestration_id: &str) {
        store.db.execute(
            "INSERT INTO sa_tickets
             (id,orchestration_id,request_key,objective,actor,source,project_id,
              worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES(?1,?2,?1,'Private objective','local-user','manual','fixture',
                    'worker',1,'queued',?3,?3)",
            params![id,orchestration_id,storage::now()],
        ).unwrap();
    }

    #[test]
    fn explicit_source_thread_links_are_idempotent_durable_and_not_task_authority() {
        let (store,path)=fixture();
        insert_manual_ticket(&store,"ticket-one","orch");
        let src=create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        let link=link_thread(&store,&src.id,"ticket-one","issue:42").unwrap();
        assert_eq!(link.repository,"test/fixture");
        assert_eq!(link_thread(&store,&src.id,"ticket-one","issue:42").unwrap().created_at,link.created_at);
        assert_eq!(list_thread_links(&store,"ticket-one").unwrap().len(),1);
        let state: String = store.db.query_row(
            "SELECT state FROM sa_tickets WHERE id='ticket-one'",[],|row|row.get(0)
        ).unwrap();
        assert_eq!(state,"queued");
        let mandates: i64=store.db.query_row("SELECT COUNT(*) FROM sa_mandates",[],|row|row.get(0)).unwrap();
        assert_eq!(mandates,0,"Linking a source must not issue a mandate");
        assert_eq!(source(&store,&src.id).unwrap().mode,"off");
        drop(store);
        let reopened=Store::open(&path).unwrap();
        assert_eq!(list_thread_links(&reopened,"ticket-one").unwrap()[0].subject_key,"issue:42");
        assert_eq!(source(&reopened,&src.id).unwrap().last_sequence,0);
    }

    #[test]
    fn source_links_reject_foreign_tickets_conflicts_and_unbounded_subjects() {
        let (store,_)=fixture();
        insert_manual_ticket(&store,"one","orch");
        insert_manual_ticket(&store,"two","orch");
        store.db.execute(
            "INSERT INTO sa_orchestrations
             (id,name,coordinator_bot_id,worker_bot_id,project_id,created_at,updated_at)
             VALUES('other','Other','router','worker','fixture',?1,?1)",
            [storage::now()],
        ).unwrap();
        insert_manual_ticket(&store,"foreign","other");
        let src=create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        assert!(link_thread(&store,&src.id,"foreign","issue:42").is_err());
        for invalid in ["issue:0","issue:01","issue:-1","issue:1/../../x","pull:abc",
                        "issue:1234567890123","issue:2\nsecret","https://github.com/test/fixture/issues/2"] {
            assert!(link_thread(&store,&src.id,"one",invalid).is_err(),"{invalid}");
        }
        link_thread(&store,&src.id,"one","issue:42").unwrap();
        assert!(link_thread(&store,&src.id,"two","issue:42").is_err());
        assert!(list_thread_links(&store,"two").unwrap().is_empty());
        store.db.execute("UPDATE sa_tickets SET state='stopped' WHERE id='two'",[]).unwrap();
        assert!(link_thread(&store,&src.id,"two","pull:52").is_err());
        let count:i64=store.db.query_row("SELECT COUNT(*) FROM sa_source_thread_links",[],|row|row.get(0)).unwrap();
        assert_eq!(count,1);
    }

    #[test]
    fn source_thread_schema_upgrades_from_v41_without_dropping_event_receipts() {
        let (store,path)=fixture();
        let src=create_source(&store,"orch","test/fixture","trusted-bot","issues.opened").unwrap();
        observe(&store,&event("delivery-kept",&src,1)).unwrap();
        store.db.execute_batch("DROP TABLE sa_source_thread_links; PRAGMA user_version=41;").unwrap();
        drop(store);
        let upgraded=Store::open(&path).unwrap();
        assert_eq!(list_decisions(&upgraded,&src.id,10).unwrap()[0].delivery_id,"delivery-kept");
        insert_manual_ticket(&upgraded,"upgraded-ticket","orch");
        assert!(link_thread(&upgraded,&src.id,"upgraded-ticket","pull:123").is_ok());
        assert_eq!(upgraded.db.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),
            storage::SCHEMA_VERSION);
    }

}
