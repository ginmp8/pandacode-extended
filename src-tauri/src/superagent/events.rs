//! S4 native Off/Observe event decisions. No inbound GitHub authentication,
//! subscription, inferred worker admission or Auto tool effects exist here.
//! Only the trusted local main-window preview can submit an envelope; external
//! transports must authenticate independently before using these decisions.
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
    window: tauri::WebviewWindow, state: State<'_,AppState>, event: EventEnvelope,
) -> Result<EventDecision> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    observe(&store,&event)
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
}