//! Native, opt-in, read-only manual SuperAgent intake (issue #131, S1 slice).
//! A ticket and its mandate outlive bot and session records. This module does
//! not execute untrusted model proposals as authority or create another runner.

use crate::{bots, storage::{self, Store}, AppState, Result};

pub(crate) mod manifest;
pub(crate) mod roster;
pub(crate) mod events;
pub(crate) mod council_adapter;
pub mod effects;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

/// A desktop window is not itself a task mandate. It is nevertheless the
/// trusted local-user entrypoint for issuing, revoking and inspecting manual
/// tasks. Headless orchestration must use a separate authenticated native
/// adapter, not an arbitrary Webview invoking these commands.
fn require_main_window_label(label: &str) -> Result<()> {
    if label == "main" {
        Ok(())
    } else {
        Err("SuperAgent desktop commands require the main window.".into())
    }
}

fn require_main_window(window: &tauri::WebviewWindow) -> Result<()> {
    require_main_window_label(window.label())
}

pub(crate) const CHANNEL: &str = "superagent";
/// The existing bot gateway accepts at most 32 KiB of inbound text. Reserve
/// headroom for native task metadata, read-only instructions and coordinator
/// evidence; never persist an objective that is silently truncated at dispatch.
const MAX_OBJECTIVE_BYTES: usize = 16 * 1024;

/// A native ceiling for each provider attempt, independent of the longer
/// ticket mandate lifetime. A late provider result must never become newly
/// approvable after its execution budget has elapsed.
const MAX_ASSIGNMENT_RUN_MS: i64 = 30 * 60 * 1000;

fn assignment_within_budget(started_at: i64, now: i64) -> bool {
    started_at > 0 && now >= started_at
        && now.saturating_sub(started_at) < MAX_ASSIGNMENT_RUN_MS
}

/// A redacted headless lease diagnostic uses exactly the same native expiry
/// rule as tool admission and callback settlement. This is observation only:
/// inspecting a live database must neither reap a session nor grant a new one.
fn headless_assignment_budget(assignment: &Assignment, now: i64) -> (bool, Option<i64>) {
    if assignment.state != "running" {
        return (false, None);
    }
    let valid_start = assignment.created_at > 0 && now >= assignment.created_at;
    let remaining = if valid_start {
        MAX_ASSIGNMENT_RUN_MS.saturating_sub(now.saturating_sub(assignment.created_at)).max(0)
    } else {
        0
    };
    (!assignment_within_budget(assignment.created_at, now), Some(remaining))
}

/// Global pause is an immediate native revocation of execution authority,
/// even for an already-running read-only assignment. It never modifies the
/// original user-issued mandate or silently renews it on resume.
fn execution_permitted(orchestration_enabled: bool, bots_paused: bool) -> bool {
    orchestration_enabled && !bots_paused
}

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_orchestrations (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  coordinator_bot_id TEXT NOT NULL,
  worker_bot_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0,1)),
  policy_revision INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sa_tickets (
  id TEXT PRIMARY KEY,
  orchestration_id TEXT NOT NULL REFERENCES sa_orchestrations(id) ON DELETE RESTRICT,
  request_key TEXT NOT NULL,
  objective TEXT NOT NULL,
  actor TEXT NOT NULL,
  source TEXT NOT NULL CHECK(source='manual'),
  project_id TEXT NOT NULL,
  worker_bot_id TEXT NOT NULL,
  policy_revision INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('queued','running','waiting_approval','waiting_review','blocked','completed','failed','stopped','interrupted','inconclusive')),
  revision INTEGER NOT NULL DEFAULT 0,
  active_assignment_id TEXT,
  evidence TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(orchestration_id,request_key)
);
CREATE TABLE IF NOT EXISTS sa_mandates (
  ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  actor TEXT NOT NULL,
  authorization_id TEXT NOT NULL,
  policy_revision INTEGER NOT NULL,
  project_id TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)),
  allowed_tools TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(allowed_tools)),
  allowed_side_effects INTEGER NOT NULL DEFAULT 0 CHECK(allowed_side_effects IN (0,1)),
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sa_assignments (
  id TEXT PRIMARY KEY,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  bot_id TEXT NOT NULL,
  session_id TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK(state IN ('running','finished','failed','stopped','interrupted')),
  attempt INTEGER NOT NULL CHECK(attempt > 0),
  result TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(ticket_id,attempt)
);
CREATE UNIQUE INDEX IF NOT EXISTS sa_one_live_assignment_per_bot
  ON sa_assignments(bot_id) WHERE state='running';
CREATE INDEX IF NOT EXISTS sa_assignment_ticket ON sa_assignments(ticket_id,created_at);

-- A frozen review target is not a verdict: only the existing native Council
-- can produce a verified review, and no target grants task-completion authority.
CREATE TABLE IF NOT EXISTS sa_council_targets (
  ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  assignment_id TEXT NOT NULL REFERENCES sa_assignments(id) ON DELETE RESTRICT,
  session_id TEXT NOT NULL,
  ticket_revision INTEGER NOT NULL,
  policy_revision INTEGER NOT NULL,
  artifact_sha256 TEXT NOT NULL,
  council_since INTEGER NOT NULL,
  council_generation INTEGER NOT NULL,
  council_config_sha256 TEXT NOT NULL,
  bound_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sa_audit (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL,
  detail TEXT NOT NULL,
  at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sa_audit_ticket ON sa_audit(ticket_id,id);
"#;

/// Cross-bot resource admission is a database invariant, not only an
/// optimistic application check. Every active assignment with the same ticket
/// project shares an exclusive lease; ordinary bots keep their existing behavior.
/// These triggers also protect future retry/update callers from bypassing admission.
pub(crate) const PROJECT_LEASE: &str = r#"
CREATE TRIGGER IF NOT EXISTS sa_project_lease_insert
 BEFORE INSERT ON sa_assignments
 WHEN NEW.state='running' AND EXISTS(
   SELECT 1 FROM sa_assignments a
   JOIN sa_tickets previous ON previous.id=a.ticket_id
   JOIN sa_tickets incoming ON incoming.id=NEW.ticket_id
   LEFT JOIN sessions previous_session ON previous_session.id=a.session_id
   LEFT JOIN sessions incoming_session ON incoming_session.id=NEW.session_id
   WHERE a.state='running' AND (
     previous.project_id=incoming.project_id
     OR previous.project_id=incoming_session.project_id
     OR previous_session.project_id=incoming.project_id
     OR previous_session.project_id=incoming_session.project_id
     OR previous_session.id IS NULL OR incoming_session.id IS NULL
     OR EXISTS(SELECT 1 FROM projects old_checkout,projects new_checkout
               WHERE old_checkout.id IN (previous.project_id,previous_session.project_id)
                 AND new_checkout.id IN (incoming.project_id,incoming_session.project_id)
                 AND old_checkout.path=new_checkout.path)
   )
 )
 BEGIN SELECT RAISE(ABORT,'SuperAgent project already has an active assignment'); END;
CREATE TRIGGER IF NOT EXISTS sa_project_lease_update
 BEFORE UPDATE OF state ON sa_assignments
 WHEN NEW.state='running' AND EXISTS(
   SELECT 1 FROM sa_assignments a
   JOIN sa_tickets previous ON previous.id=a.ticket_id
   JOIN sa_tickets incoming ON incoming.id=NEW.ticket_id
   LEFT JOIN sessions previous_session ON previous_session.id=a.session_id
   LEFT JOIN sessions incoming_session ON incoming_session.id=NEW.session_id
   WHERE a.id<>NEW.id AND a.state='running' AND (
     previous.project_id=incoming.project_id
     OR previous.project_id=incoming_session.project_id
     OR previous_session.project_id=incoming.project_id
     OR previous_session.project_id=incoming_session.project_id
     OR previous_session.id IS NULL OR incoming_session.id IS NULL
     OR EXISTS(SELECT 1 FROM projects old_checkout,projects new_checkout
               WHERE old_checkout.id IN (previous.project_id,previous_session.project_id)
                 AND new_checkout.id IN (incoming.project_id,incoming_session.project_id)
                 AND old_checkout.path=new_checkout.path)
   )
 )
 BEGIN SELECT RAISE(ABORT,'SuperAgent project already has an active assignment'); END;
"#;

/// An active harness is not resumed automatically after process restart.
pub(crate) const RECOVER: &str = "
-- A crash or legacy partial write may leave a running assignment on a
-- waiting or terminal ticket. Revoke its grant and settle it once, before
-- the ordinary running-ticket recovery handles the remaining assignments.
INSERT INTO sa_audit(ticket_id,kind,detail,at)
 SELECT t.id,'restart_inconsistent_assignment',
  'Active assignment did not match ticket state; execution revoked',
  CAST(strftime('%s','now') AS INTEGER)*1000
 FROM sa_tickets t WHERE t.state<>'running' AND EXISTS (
  SELECT 1 FROM sa_assignments a WHERE a.ticket_id=t.id AND a.state='running'
 );
UPDATE sa_mandates SET revoked=1 WHERE revoked=0 AND ticket_id IN (
 SELECT ticket_id FROM sa_assignments WHERE state='running'
);
UPDATE sa_tickets SET
 state=CASE WHEN state IN ('completed','failed','stopped','inconclusive')
  THEN state ELSE 'interrupted' END,
 active_assignment_id=NULL,revision=revision+1,
 updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state<>'running' AND EXISTS (
  SELECT 1 FROM sa_assignments a WHERE a.ticket_id=sa_tickets.id AND a.state='running'
 );
INSERT INTO sa_audit(ticket_id,kind,detail,at)
 SELECT id,'restart_interrupted','Native worker session interrupted; no automatic replay',
 CAST(strftime('%s','now') AS INTEGER)*1000
 FROM sa_tickets WHERE state='running';
UPDATE sa_assignments SET state='interrupted',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='running';
UPDATE sa_tickets SET state='interrupted',revision=revision+1,active_assignment_id=NULL,
 updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='running';
-- Interrupted tickets must not keep live mandates after a crash, including
-- legacy rows already interrupted without the required revocation.
UPDATE sa_mandates SET revoked=1 WHERE revoked=0 AND ticket_id IN (
 SELECT id FROM sa_tickets WHERE state='interrupted'
);
";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewOrchestration {
    pub name: String,
    pub coordinator_bot_id: String,
    pub worker_bot_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orchestration {
    pub id: String,
    pub name: String,
    pub coordinator_bot_id: String,
    pub worker_bot_id: String,
    pub project_id: String,
    pub enabled: bool,
    pub policy_revision: i64,
}

fn orchestration_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Orchestration> {
    Ok(Orchestration {
        id: row.get(0)?, name: row.get(1)?, coordinator_bot_id: row.get(2)?,
        worker_bot_id: row.get(3)?, project_id: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0, policy_revision: row.get(6)?,
    })
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationOverview {
    #[serde(flatten)]
    pub config: Orchestration,
    pub ticket_count: i64,
    pub running_count: i64,
    pub blocked_count: i64,
    pub last_activity_at: i64,
    pub worker_count: i64,
    pub reviewer_count: i64,
    /// Read-only reason why a disabled Team cannot currently enter manual mode.
    /// Native enabling rechecks the same guard under its revision-bound mutation.
    pub manual_enable_blocker: Option<String>,
    /// Read-only Team edit guard across every durable Demand and effect.
    pub policy_edit_blocker: Option<String>,
}

/// Derive overview counts from durable tickets rather than a page of sessions.
/// This is a read-only projection and does not change policy or admission.
fn orchestration_overview(store: &Store) -> Result<Vec<OrchestrationOverview>> {
    let mut stmt = store.db.prepare(
        "SELECT o.id,o.name,o.coordinator_bot_id,o.worker_bot_id,o.project_id,
                o.enabled,o.policy_revision,
                COUNT(t.id),
                COALESCE(SUM(CASE WHEN t.state='running' THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN t.state='blocked' THEN 1 ELSE 0 END),0),
                COALESCE(MAX(t.updated_at),o.updated_at),
                1 + (SELECT COUNT(*) FROM sa_team_roles r
                     WHERE r.orchestration_id=o.id AND r.role='worker'),
                (SELECT COUNT(*) FROM sa_team_roles r
                 WHERE r.orchestration_id=o.id AND r.role='reviewer')
         FROM sa_orchestrations o
         LEFT JOIN sa_tickets t ON t.orchestration_id=o.id
         GROUP BY o.id ORDER BY o.created_at,o.id",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(OrchestrationOverview {
            config: orchestration_row(row)?,
            ticket_count: row.get(7)?,
            running_count: row.get(8)?,
            blocked_count: row.get(9)?,
            last_activity_at: row.get(10)?,
            worker_count: row.get(11)?,
            reviewer_count: row.get(12)?,
            manual_enable_blocker: None,
            policy_edit_blocker: None,
        })
    }).map_err(|e| e.to_string())?;
    let mut result = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    for item in &mut result {
        if !item.config.enabled {
            item.manual_enable_blocker = manual_enable_blocker(store, &item.config)?;
        }
        item.policy_edit_blocker = team_policy_edit_blocker(store, &item.config)?;
    }
    Ok(result)
}

fn configuration(store: &Store, id: &str) -> Result<Orchestration> {
    store.db.query_row(
        "SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision FROM sa_orchestrations WHERE id=?1",
        [id], orchestration_row,
    ).optional().map_err(|e| e.to_string())?
        .ok_or_else(|| "Orchestration not found.".into())
}

fn create_configuration(store: &Store, input: NewOrchestration) -> Result<Orchestration> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err("Choose an orchestration name of at most 120 characters.".into());
    }
    if input.coordinator_bot_id == input.worker_bot_id {
        return Err("Coordinator and worker must be different existing bots.".into());
    }
    let coordinator = bots::get(store, &input.coordinator_bot_id)?;
    let worker = bots::get(store, &input.worker_bot_id)?;
    // Every native bot owns its own project/workspace. A coordinator is a separate
    // existing bot, not a second profile inside the worker\'s checkout. The
    // worker project is the initial execution scope; never require bot projects
    // to coincide or silently switch one bot\'s global project.
    if coordinator.paused || worker.paused {
        return Err("Choose active coordinator and worker bots.".into());
    }
    // A disabled Team is still trusted local configuration. Never save a
    // primary role that the native admission harness cannot validate.
    for bot in [&coordinator, &worker] {
        if let Some(issue) = roster::member_execution_issue(bot) {
            return Err(issue.into());
        }
    }
    let id = storage::id();
    let now = storage::now();
    store.db.execute(
        "INSERT INTO sa_orchestrations(id,name,coordinator_bot_id,worker_bot_id,project_id,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?6)",
        params![id,name,coordinator.id,worker.id,worker.project_id,now],
    ).map_err(|e| e.to_string())?;
    configuration(store, &id)
}

/// Durable, read-only description of an explicitly issued task mandate.
/// Exposes the grant's native identity without introducing an edit or escalation
/// endpoint; a model result never alters the original authorization.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateSummary {
    pub ticket_id: String,
    pub actor: String,
    pub authorization_id: String,
    pub policy_revision: i64,
    pub project_id: String,
    pub expires_at: i64,
    pub revoked: bool,
    pub allowed_tools: Vec<String>,
    pub allowed_side_effects: bool,
    pub created_at: i64,
}

fn mandate(store: &Store, ticket_id: &str) -> Result<MandateSummary> {
    get_ticket(store, ticket_id)?;
    let (mut grant, tools): (MandateSummary, String) = store.db.query_row(
        "SELECT ticket_id,actor,authorization_id,policy_revision,project_id,expires_at,
                revoked,allowed_tools,allowed_side_effects,created_at
         FROM sa_mandates WHERE ticket_id=?1",
        [ticket_id],
        |r| Ok((MandateSummary {
            ticket_id: r.get(0)?, actor: r.get(1)?, authorization_id: r.get(2)?,
            policy_revision: r.get(3)?, project_id: r.get(4)?,
            expires_at: r.get(5)?, revoked: r.get::<_, i64>(6)? != 0,
            allowed_tools: Vec::new(), allowed_side_effects: r.get::<_, i64>(8)? != 0,
            created_at: r.get(9)?,
        }, r.get(7)?)),
    ).map_err(|e| e.to_string())?;
    grant.allowed_tools = serde_json::from_str::<Vec<String>>(&tools)
        .map_err(|_| "SuperAgent mandate contains invalid tool authorization.")?;
    Ok(grant)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: String,
    pub orchestration_id: String,
    pub request_key: String,
    pub objective: String,
    pub actor: String,
    pub state: String,
    pub revision: i64,
    pub active_assignment_id: Option<String>,
    pub evidence: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn ticket_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Ticket> {
    Ok(Ticket {
        id: row.get(0)?, orchestration_id: row.get(1)?, request_key: row.get(2)?,
        objective: row.get(3)?, actor: row.get(4)?, state: row.get(5)?,
        revision: row.get(6)?, active_assignment_id: row.get(7)?,
        evidence: row.get(8)?, created_at: row.get(9)?, updated_at: row.get(10)?,
    })
}

fn get_ticket(store: &Store, id: &str) -> Result<Ticket> {
    store.db.query_row(
        "SELECT id,orchestration_id,request_key,objective,actor,state,revision,active_assignment_id,evidence,created_at,updated_at
         FROM sa_tickets WHERE id=?1", [id], ticket_row,
    ).optional().map_err(|e| e.to_string())?
        .ok_or_else(|| "SuperAgent ticket not found.".into())
}

/// A bounded native task index is required to recover a ticket when the
/// original bot conversation has been deleted. Neither view depends on session
/// or bot foreign keys.
fn list_tickets(store: &Store, config_id: &str, limit: i64) -> Result<Vec<Ticket>> {
    if !(1..=200).contains(&limit) {
        return Err("Ticket page size must be between 1 and 200.".into());
    }
    configuration(store, config_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,orchestration_id,request_key,objective,actor,state,revision,
                active_assignment_id,evidence,created_at,updated_at
         FROM sa_tickets WHERE orchestration_id=?1
         ORDER BY created_at DESC,id DESC LIMIT ?2"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![config_id,limit], ticket_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

/// Bounded cross-Team demand index for the trusted local workspace. The
/// projection intentionally excludes worker evidence, actor, request keys,
/// mandates and session IDs; detailed access stays within native Ticket reads.
/// A read-only request groups native page filters without adding implicit authority.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DemandQuery {
    limit: Option<i64>,
    before_updated_at: Option<i64>,
    before_id: Option<String>,
    orchestration_id: Option<String>,
    state_filter: Option<String>,
    search_query: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandIndex {
    pub id: String,
    pub orchestration_id: String,
    pub objective: String,
    pub state: String,
    pub revision: i64,
    pub updated_at: i64,
    /// Immutable project chosen when this Ticket was admitted, never the Team's current project.
    pub project_id: String,
}

/// Test-only compatibility helper for pagination oracles; production callers
/// use the state-filtered native query directly.
#[cfg(test)]
fn list_all_demands_page(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
    orchestration_id: Option<&str>,
) -> Result<Vec<DemandIndex>> {
    list_all_demands_page_filtered(
        store, limit, before_updated_at, before_id, orchestration_id, None,
    )
}

/// Exact native Ticket-state filtering happens in SQLite before keyset paging.
/// Unknown values fail closed: provider input is never SQL or execution authority.
/// Production uses the searched query directly; this wrapper is a test oracle.
#[cfg(test)]
fn list_all_demands_page_filtered(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
    orchestration_id: Option<&str>, state_filter: Option<&str>,
) -> Result<Vec<DemandIndex>> {
    list_all_demands_page_for_project(
        store, limit, before_updated_at, before_id, DemandPageFilters {
            orchestration_id, state_filter, search_query: None, project_id: None,
        },
    )
}

/// Search regression tests use the exact production query. This test-only
/// adapter preserves their search/pagination assertions without leaving
/// an unused production wrapper behind.
#[cfg(test)]
fn list_all_demands_page_searched(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
    orchestration_id: Option<&str>, state_filter: Option<&str>, search_query: Option<&str>,
) -> Result<Vec<DemandIndex>> {
    list_all_demands_page_for_project(
        store, limit, before_updated_at, before_id, DemandPageFilters {
            orchestration_id, state_filter, search_query, project_id: None,
        },
    )
}

/// Keep native index filters together when the read-only board gains dimensions.
struct DemandPageFilters<'a> {
    orchestration_id: Option<&'a str>,
    state_filter: Option<&'a str>,
    search_query: Option<&'a str>,
    project_id: Option<&'a str>,
}

/// Native project scope is evaluated against each Ticket's immutable admitted
/// project, not a Team's current configuration or the loaded page.
fn list_all_demands_page_for_project(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
    filters: DemandPageFilters<'_>,
) -> Result<Vec<DemandIndex>> {
    let DemandPageFilters { orchestration_id, state_filter, search_query, project_id } = filters;
    if let Some(id) = project_id {
        if id.is_empty() || id.len() > 128
            || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_')) {
            return Err("Demand project filter requires a bounded native project ID.".into());
        }
    }
    let search_query = search_query.map(str::trim);
    if search_query.is_some_and(|query| query.is_empty() || query.len() > 128
        || query.chars().any(char::is_control)) {
        return Err("Demand search requires 1 to 128 non-control UTF-8 bytes.".into());
    }
    if let Some(state) = state_filter {
        if !matches!(state, "queued" | "running" | "waiting_approval" |
            "waiting_review" | "blocked" | "completed" | "failed" |
            "stopped" | "interrupted" | "inconclusive") {
            return Err("Demand state filter must be an exact native Ticket state.".into());
        }
    }
    if !(1..=200).contains(&limit) {
        return Err("Demand index page size must be between 1 and 200.".into());
    }
    if before_updated_at.is_some() != before_id.is_some()
        || before_updated_at.is_some_and(|time| time <= 0)
        || before_id.is_some_and(|id| id.is_empty() || id.len() > 128
            || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_')))
    {
        return Err("Demand cursor requires a bounded Ticket ID and positive timestamp.".into());
    }
    if let Some(id) = orchestration_id {
        if id.is_empty() || id.len() > 128
            || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_')) {
            return Err("Demand Team filter requires a bounded native Team ID.".into());
        }
        // A deleted or unknown Team must not silently look like an empty board.
        configuration(store, id)?;
    }
    // A syntactically valid timestamp/ID is not a valid pagination boundary.
    // Require the anchor to remain visible under the exact native filters;
    // otherwise a stale or foreign cursor could silently skip real Demands.
    if let (Some(at), Some(id)) = (before_updated_at, before_id) {
        let visible: i64 = store.db.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sa_tickets t JOIN sa_orchestrations o
                    ON o.id=t.orchestration_id
                WHERE t.id=?1 AND t.updated_at=?2
                  AND (?3 IS NULL OR t.orchestration_id=?3)
                  AND (?4 IS NULL OR t.state=?4)
                  AND (?5 IS NULL OR instr(lower(t.objective),lower(?5)) > 0)
                  AND (?6 IS NULL OR t.project_id=?6)
            )",
            params![id,at,orchestration_id,state_filter,search_query,project_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if visible != 1 {
            return Err("Demand cursor is stale or outside the current native filters; refresh the index.".into());
        }
    }
    let mut stmt = store.db.prepare(
        "SELECT t.id,t.orchestration_id,t.objective,t.state,t.revision,t.updated_at,t.project_id
         FROM sa_tickets t JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE (?2 IS NULL OR t.updated_at < ?2
                OR (t.updated_at=?2 AND t.id < ?3))
           AND (?4 IS NULL OR t.orchestration_id=?4)
           AND (?5 IS NULL OR t.state=?5)
           AND (?6 IS NULL OR instr(lower(t.objective),lower(?6)) > 0)
           AND (?7 IS NULL OR t.project_id=?7)
         ORDER BY t.updated_at DESC,t.id DESC LIMIT ?1",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![limit,before_updated_at,before_id,orchestration_id,state_filter,search_query,project_id], |row| Ok(DemandIndex {
        id: row.get(0)?, orchestration_id: row.get(1)?, objective: row.get(2)?,
        state: row.get(3)?, revision: row.get(4)?, updated_at: row.get(5)?,
        project_id: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

// Retain the pre-pagination native oracle only for existing unit coverage.
#[cfg(test)]
fn list_all_demands(store: &Store, limit: i64) -> Result<Vec<DemandIndex>> {
    list_all_demands_page(store, limit, None, None, None)
}

/// Cross-Team native decision queue, independent of the general 200-row
/// Demand backlog. Only native states are represented; waiting_review is NOT
/// human-approvable until an actual task-bound Council verdict is available.
#[cfg(test)]
fn list_review_inbox(store: &Store, limit: i64) -> Result<Vec<DemandIndex>> {
    list_review_inbox_page(store, limit, None, None)
}

/// Keyset pagination keeps pending reviews with equal timestamps reachable.
fn list_review_inbox_page(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<Vec<DemandIndex>> {
    list_review_inbox_page_scoped(store, limit, before_updated_at, before_id, None)
}

/// Apply Team scope in SQLite before LIMIT. A Team cursor is valid only while
/// its anchor is still a pending Ticket in that exact Team and revision clock.
fn list_review_inbox_page_scoped(
    store: &Store, limit: i64, before_updated_at: Option<i64>, before_id: Option<&str>,
    orchestration_id: Option<&str>,
) -> Result<Vec<DemandIndex>> {
    if !(1..=200).contains(&limit) {
        return Err("Review inbox page size must be between 1 and 200.".into());
    }
    if before_updated_at.is_some() != before_id.is_some()
        || before_updated_at.is_some_and(|value| value <= 0)
        || before_id.is_some_and(|id| id.is_empty() || id.len() > 128
            || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_')))
    {
        return Err("Review cursor needs a positive timestamp and bounded Ticket ID.".into());
    }
    if let Some(team_id) = orchestration_id {
        configuration(store, team_id)?;
        if let (Some(at), Some(ticket_id)) = (before_updated_at, before_id) {
            let visible: i64 = store.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sa_tickets
                  WHERE id=?1 AND updated_at=?2 AND orchestration_id=?3
                    AND state IN ('waiting_approval','waiting_review'))",
                params![ticket_id, at, team_id], |row| row.get(0),
            ).map_err(|e| e.to_string())?;
            if visible != 1 {
                return Err("Review cursor is stale or outside the selected Team; refresh the inbox.".into());
            }
        }
    }
    let mut stmt = store.db.prepare(
        "SELECT t.id,t.orchestration_id,t.objective,t.state,t.revision,t.updated_at,t.project_id
         FROM sa_tickets t JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE t.state IN ('waiting_approval','waiting_review')
           AND (?2 IS NULL OR t.updated_at < ?2 OR (t.updated_at=?2 AND t.id < ?3))
           AND (?4 IS NULL OR t.orchestration_id=?4)
         ORDER BY t.updated_at DESC,t.id DESC LIMIT ?1",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![limit,before_updated_at,before_id,orchestration_id],
        |row| Ok(DemandIndex {
            id: row.get(0)?, orchestration_id: row.get(1)?, objective: row.get(2)?,
            state: row.get(3)?, revision: row.get(4)?, updated_at: row.get(5)?,
            project_id: row.get(6)?,
        }),
    ).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

/// Bounded observational projection. Review-pending never means passed.
pub(crate) fn list_headless_review_inbox(database_path: &std::path::Path) -> Result<String> {
    list_headless_review_inbox_page(database_path, None, None)
}

pub(crate) fn list_headless_review_inbox_page(
    database_path: &std::path::Path, before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<String> {
    list_headless_review_inbox_page_scoped(database_path, None, before_updated_at, before_id)
}

pub(crate) fn list_headless_team_review_inbox_page(
    database_path: &std::path::Path, orchestration_id: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<String> {
    list_headless_review_inbox_page_scoped(
        database_path, Some(orchestration_id), before_updated_at, before_id,
    )
}

fn list_headless_review_inbox_page_scoped(
    database_path: &std::path::Path, orchestration_id: Option<&str>,
    before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    // Page and lookahead share one read-only SQLite snapshot.
    let snapshot = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let items = list_review_inbox_page_scoped(
        &store, 200, before_updated_at, before_id, orchestration_id,
    )?;
    let cursor = items.last().map(|item| (item.updated_at, item.id.clone()));
    let may_have_more = match cursor.as_ref() {
        Some((time, id)) => !list_review_inbox_page_scoped(
            &store, 1, Some(*time), Some(id), orchestration_id,
        )?.is_empty(),
        None => false,
    };
    snapshot.commit().map_err(|e| e.to_string())?;
    let next_cursor = if may_have_more {
        cursor.map(|(updated_at, ticket_id)| serde_json::json!({
            "updatedAt": updated_at, "ticketId": ticket_id,
        }))
    } else { None };
    let entries = items.into_iter().map(|item| {
        let next_action = if item.state == "waiting_review" {
            "await_fresh_task_bound_native_council"
        } else {
            "inspect_current_revision_before_human_decision"
        };
        serde_json::json!({
            "ticketId": item.id,
            "orchestrationId": item.orchestration_id,
            "state": item.state,
            "revision": item.revision,
            "updatedAt": item.updated_at,
            "nextOperatorAction": next_action,
            "nativeCouncilPassVerifiedByThisCommand": false,
            "acceptanceAuthorizedByThisCommand": false
        })
    }).collect::<Vec<_>>();
    let mut response = serde_json::json!({
        "tickets": entries,
        "limit": 200,
        "mayHaveMore": may_have_more,
        "nextCursor": next_cursor,
        "observationalOnly": true,
        "startupRecoveryPerformed": false
    });
    if let Some(team_id) = orchestration_id {
        response["orchestrationId"] = serde_json::json!(team_id);
    }
    serde_json::to_string_pretty(&response).map_err(|e| e.to_string())
}

/// Immutable native assignment history, including completed and failed attempts.
/// Never infer task success or authorization from an assignment's terminal state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub id: String,
    pub ticket_id: String,
    pub bot_id: String,
    pub session_id: String,
    pub state: String,
    pub attempt: i64,
    pub result: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn assignments(store: &Store, ticket_id: &str, limit: i64) -> Result<Vec<Assignment>> {
    if !(1..=200).contains(&limit) {
        return Err("Assignment page size must be between 1 and 200.".into());
    }
    // Reject unknown tickets before enumerating any durable history.
    get_ticket(store, ticket_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,ticket_id,bot_id,session_id,state,attempt,result,created_at,updated_at
         FROM sa_assignments WHERE ticket_id=?1 ORDER BY attempt DESC LIMIT ?2",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id, limit], |row| Ok(Assignment {
        id: row.get(0)?, ticket_id: row.get(1)?, bot_id: row.get(2)?,
        session_id: row.get(3)?, state: row.get(4)?, attempt: row.get(5)?,
        result: row.get(6)?, created_at: row.get(7)?, updated_at: row.get(8)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub kind: String,
    pub detail: String,
    pub at: i64,
}

fn history(store: &Store, ticket_id: &str, limit: i64) -> Result<Vec<AuditEntry>> {
    if !(1..=200).contains(&limit) {
        return Err("Activity page size must be between 1 and 200.".into());
    }
    get_ticket(store, ticket_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,kind,detail,at FROM sa_audit WHERE ticket_id=?1
         ORDER BY id DESC LIMIT ?2"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id,limit], |r| Ok(AuditEntry {
        id: r.get(0)?, kind: r.get(1)?, detail: r.get(2)?, at: r.get(3)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPage {
    pub ticket_id: String,
    pub items: Vec<AuditEntry>,
    pub has_more: bool,
    pub next_cursor: Option<i64>,
}

/// Bounded, immutable keyset history for the authorized desktop Ticket inspector.
/// Details stay inside the main-window boundary; the headless inspector remains redacted.
fn activity_page(
    store: &Store, ticket_id: &str, before_id: Option<i64>, limit: i64,
) -> Result<ActivityPage> {
    if !(1..=100).contains(&limit) {
        return Err("Activity page size must be between 1 and 100.".into());
    }
    if before_id.is_some_and(|id| id <= 0) {
        return Err("Activity cursor must be a positive audit ID.".into());
    }
    get_ticket(store, ticket_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,kind,detail,at FROM sa_audit
         WHERE ticket_id=?1 AND (?2 IS NULL OR id < ?2)
         ORDER BY id DESC LIMIT ?3",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id, before_id, limit + 1], |row| Ok(AuditEntry {
        id: row.get(0)?, kind: row.get(1)?, detail: row.get(2)?, at: row.get(3)?,
    })).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_cursor = if has_more { items.last().map(|entry| entry.id) } else { None };
    Ok(ActivityPage { ticket_id: ticket_id.into(), items, has_more, next_cursor })
}

/// Consistent native read of a Ticket, its mandate, attempt history and audit.
/// A single SQLite read transaction prevents a pre-Stop assignment being
/// displayed alongside a post-Stop authority record.
/// Native per-attempt insights are observed in the same transaction as
/// Ticket, Mandate and Assignment. These are neither a tool grant nor
/// proof of a live provider or an approved Council verdict.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentInsight {
    pub assignment_id: String,
    pub current_assignment: bool,
    pub has_result: bool,
    pub mandate_revoked: bool,
    pub run_budget_expired: bool,
    pub remaining_run_budget_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub ticket: Ticket,
    pub mandate: MandateSummary,
    pub assignments: Vec<Assignment>,
    pub attempt_insights: Vec<AssignmentInsight>,
    pub activity: Vec<AuditEntry>,
    /// Historical task-effective bot profiles, not permission or execution proof.
    /// Legacy Tickets admitted before schema v46 report None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admitted_profile: Option<roster::AdmittedTeamProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admitted_strategy: Option<roster::AdmittedTeamStrategy>,
    /// Read-side policy drift only: a Team edit cannot reauthorize or rewrite a Demand.
    pub current_team_policy_revision: i64,
    pub team_policy_changed_since_admission: bool,
    /// Snapshot explanation only. Provider admission and native acceptance
    /// always recheck their own authority, revision and effect state.
    pub finalization_gate: String,
    pub unresolved_external_effects: i64,
    /// Advisory native Council status, never a SuperAgent approval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub council: Option<council_adapter::CouncilObservation>,
}

fn ticket_snapshot(store: &Store, ticket_id: &str, limit: i64) -> Result<TaskSnapshot> {
    if !(1..=200).contains(&limit) {
        return Err("Snapshot page size must be between 1 and 200.".into());
    }
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let ticket = get_ticket(store, ticket_id)?;
    let native_mandate = mandate(store, ticket_id)?;
    // Compare both revisions inside this same read transaction. This is not a
    // provider/tool authorization decision and cannot advance task state.
    let current_team_policy_revision = configuration(store, &ticket.orchestration_id)?.policy_revision;
    let team_policy_changed_since_admission =
        current_team_policy_revision != native_mandate.policy_revision;
    let native_assignments = assignments(store, ticket_id, limit)?;
    let observed_at = storage::now();
    let attempt_insights = native_assignments.iter().map(|a| {
        let (expired, remaining) = headless_assignment_budget(a, observed_at);
        AssignmentInsight {
            assignment_id: a.id.clone(),
            current_assignment: ticket.state == "running" && a.state == "running"
                && ticket.active_assignment_id.as_deref() == Some(a.id.as_str()),
            has_result: a.result.as_ref().is_some_and(|text| !text.is_empty()),
            mandate_revoked: native_mandate.revoked,
            run_budget_expired: expired,
            remaining_run_budget_ms: remaining,
        }
    }).collect();
    let unresolved_external_effects = effects::unresolved_count(store, ticket_id)?;
    let candidate_session = native_assignments.iter()
        .find(|attempt| attempt.state == "finished"
            && attempt.result == ticket.evidence)
        .map(|attempt| attempt.session_id.as_str()).unwrap_or_default();
    let finalization_gate = if ticket.state != "waiting_approval" {
        "not_pending"
    } else if council_adapter::requires_task_review(store, ticket_id, candidate_session)? {
        "council_required"
    } else if unresolved_external_effects > 0 {
        "effects_unresolved"
    } else {
        "native_recheck_required"
    };
    let result = TaskSnapshot {
        current_team_policy_revision,
        team_policy_changed_since_admission,
        finalization_gate: finalization_gate.to_owned(),
        unresolved_external_effects,
        council: council_adapter::inspect(store, &ticket)?,
        ticket,
        mandate: native_mandate,
        assignments: native_assignments,
        attempt_insights,
        activity: history(store, ticket_id, limit)?,
        admitted_profile: roster::ticket_profile(store, ticket_id)?,
        admitted_strategy: roster::ticket_strategy(store, ticket_id)?,
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}


/// Native, bounded Ticket execution graph. Every edge is a durable
/// Ticket->Assignment ownership relation, not inferred agent delegation.
/// Review is a status node only; this does not fabricate Council approval.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphNode {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub label: String,
    pub bot_id: Option<String>,
    pub attempt: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraph {
    pub ticket_id: String,
    pub ticket_revision: i64,
    pub truncated_attempts: bool,
    pub nodes: Vec<RunGraphNode>,
    pub edges: Vec<RunGraphEdge>,
}

fn run_graph(store: &Store, ticket_id: &str) -> Result<RunGraph> {
    // Same native snapshot transaction as the Inspector, not a client-side
    // graph built from unrelated Ticket and Assignment revisions.
    // Read one extra row to distinguish exactly fifty attempts from
    // truncated history without exposing any older attempt in the graph.
    let snapshot = ticket_snapshot(store, ticket_id, 51)?;
    let ticket = snapshot.ticket;
    let root_id = format!("ticket:{}", ticket.id);
    let mut nodes = vec![RunGraphNode {
        id: root_id.clone(), kind: "ticket".into(),
        state: ticket.state.clone(), label: "Demand / Ticket".into(),
        bot_id: None, attempt: None, active: false,
    }];
    let mut edges = Vec::new();
    for assignment in snapshot.assignments.iter().take(50).rev() {
        let id = format!("assignment:{}", assignment.id);
        nodes.push(RunGraphNode {
            id: id.clone(), kind: "assignment".into(),
            state: assignment.state.clone(), label: format!("Attempt {}", assignment.attempt),
            bot_id: Some(assignment.bot_id.clone()), attempt: Some(assignment.attempt),
            active: ticket.state == "running" && assignment.state == "running"
                && ticket.active_assignment_id.as_deref() == Some(assignment.id.as_str()),
        });
        edges.push(RunGraphEdge {
            from: root_id.clone(), to: id, relation: "owns_attempt".into(),
        });
    }
    if ticket.state == "waiting_approval" || ticket.state == "waiting_review" {
        let id = format!("decision:{}", ticket.id);
        nodes.push(RunGraphNode {
            id: id.clone(), kind: "pending_decision".into(),
            state: ticket.state.clone(),
            label: if ticket.state == "waiting_review" {
                "Native Council verification required".into()
            } else { "Human decision pending".into() },
            bot_id: None, attempt: None, active: false,
        });
        edges.push(RunGraphEdge {
            from: root_id, to: id, relation: "awaits_decision".into(),
        });
    }
    Ok(RunGraph {
        ticket_id: ticket.id, ticket_revision: ticket.revision,
        truncated_attempts: snapshot.assignments.len() > 50,
        nodes, edges,
    })
}

/// Headless parity with the workspace's native, revisioned Ticket execution
/// graph. This is an observation of durable attempts, NOT a record of
/// dynamic subagents, native Council verdicts or live provider handles.
/// Opening read-only must not run restart recovery beside an active host.
pub(crate) fn inspect_headless_ticket_graph(
    database_path: &std::path::Path, ticket_id: &str,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let graph = run_graph(&store, ticket_id)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "graph": graph,
        "observationalOnly": true,
        "dynamicSubagentsRepresented": false,
        "councilVerdictIncluded": false,
        "providerDispatchedByCommand": false,
        "recoveryPerformed": false
    })).map_err(|e| e.to_string())
}

/// Keyset-paginated, redacted native audit metadata for one Ticket. Unlike
/// Store::open, this inspector cannot migrate, interrupt or replay live work.
/// Only controlled audit kinds and timestamps are exposed; details may contain
/// private objectives, user reasons or provider output and must stay private.
pub(crate) fn list_headless_ticket_activity(
    database_path: &std::path::Path, ticket_id: &str, before_id: Option<i64>,
) -> Result<String> {
    if before_id.is_some_and(|id| id <= 0) {
        return Err("Activity cursor must be a positive audit ID.".into());
    }
    let store = Store::open_read_only(database_path)?;
    get_ticket(&store, ticket_id)?;
    let mut stmt = store.db.prepare(
        "SELECT id,kind,at FROM sa_audit
         WHERE ticket_id=?1 AND (?2 IS NULL OR id < ?2)
         ORDER BY id DESC LIMIT 101",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id, before_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "kind": row.get::<_, String>(1)?,
            "at": row.get::<_, i64>(2)?,
        }))
    }).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let has_more = items.len() > 100;
    items.truncate(100);
    let next_cursor = if has_more {
        items.last().and_then(|entry| entry["id"].as_i64())
    } else {
        None
    };
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket_id,
        "pageLimit": 100,
        "hasMore": has_more,
        "nextCursor": next_cursor,
        "items": items,
        "redacted": true,
        "observationalOnly": true,
        "recoveryPerformed": false,
    })).map_err(|e| e.to_string())
}

/// A one-shot local CLI inspector is deliberately read-only: it must not run
/// Store::open's migration/restart settlement behind a live serve process.
/// Return bounded operational metadata, never the task objective, worker result,
/// private evidence, rejection reason, authorization token, or transcript.
pub(crate) fn inspect_headless_status(
    database_path: &std::path::Path, ticket_id: Option<&str>,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let observed_at = storage::now();
    let value = if let Some(ticket_id) = ticket_id {
        let snapshot = ticket_snapshot(&store, ticket_id, 50)?;
        serde_json::json!({
            "ticketId": &snapshot.ticket.id,
            "orchestrationId": &snapshot.ticket.orchestration_id,
            "state": &snapshot.ticket.state,
            "revision": snapshot.ticket.revision,
            "activeAssignmentId": &snapshot.ticket.active_assignment_id,
            "hasEvidence": snapshot.ticket.evidence.as_ref().is_some_and(|e| !e.is_empty()),
            "unresolvedExternalEffects": effects::unresolved_count(&store, ticket_id)?,
            "recovery": headless_recovery_summary(&store, &snapshot.ticket, observed_at)?,
            "mandate": {
                "revoked": snapshot.mandate.revoked,
                "policyRevision": snapshot.mandate.policy_revision,
                "expiresAt": snapshot.mandate.expires_at,
                "allowedSideEffects": snapshot.mandate.allowed_side_effects,
                "allowedToolCount": snapshot.mandate.allowed_tools.len()
            },
            "assignments": snapshot.assignments.iter().map(|a| {
                let (run_budget_expired, remaining_run_budget_ms) =
                    headless_assignment_budget(a, observed_at);
                serde_json::json!({
                    "id": &a.id, "botId": &a.bot_id, "sessionId": &a.session_id,
                    "state": &a.state, "attempt": a.attempt,
                    "hasResult": a.result.as_ref().is_some_and(|text| !text.is_empty()),
                    "runBudgetExpired": run_budget_expired,
                    "remainingRunBudgetMs": remaining_run_budget_ms,
                })
            }).collect::<Vec<_>>(),
            "activity": snapshot.activity.iter().map(|a| serde_json::json!({
                "kind": &a.kind, "at": a.at
            })).collect::<Vec<_>>()
        })
    } else {
        serde_json::to_value(orchestration_overview(&store)?)
            .map_err(|e| e.to_string())?
    };
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

/// Read-only, bounded recovery classification for a local operator. The CLI
/// cannot observe another process's live provider handle, and these fields are
/// never a grant to retry or to acknowledge an external effect.
fn headless_recovery_summary(store: &Store, ticket: &Ticket, now: i64) -> Result<serde_json::Value> {
    let (running, active_linked, missing_sessions): (i64, i64, i64) = store.db.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN a.id=?2 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN live.id IS NULL THEN 1 ELSE 0 END),0)
         FROM sa_assignments a
         LEFT JOIN sessions live ON live.id=a.session_id
         WHERE a.ticket_id=?1 AND a.state='running'",
        params![&ticket.id, ticket.active_assignment_id.as_deref()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| e.to_string())?;
    let (pending, uncertain): (i64, i64) = store.db.query_row(
        "SELECT COALESCE(SUM(CASE WHEN state='pending' THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN state='uncertain' THEN 1 ELSE 0 END),0)
         FROM sa_effect_receipts WHERE ticket_id=?1",
        [&ticket.id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;
    let expired_running: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM sa_assignments
         WHERE ticket_id=?1 AND state='running'
           AND (created_at<=0 OR created_at>?2 OR created_at<=?3)",
        params![&ticket.id, now, now.saturating_sub(MAX_ASSIGNMENT_RUN_MS)],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let mandate = mandate(store, &ticket.id)?;
    let consistent = if ticket.state == "running" {
        // One active native assignment is the current Ticket contract. A
        // duplicate attempt with a valid active link must still be triaged.
        running == 1 && active_linked == 1 && missing_sessions == 0
    } else {
        // A completed/stopped Ticket with an orphan live assignment is not
        // healthy, even if its own state is terminal.
        running == 0
    };
    // A structurally linked Assignment may still have lost its native
    // provider/tool authority (disabled Team, paused bot, stale role/profile,
    // changed policy or detached task thread). Reuse admission enforcement
    // rather than presenting an invalid provider as healthy.
    let active_read_only_authorized = if ticket.state == "running" && consistent {
        let session_id: Option<String> = store.db.query_row(
            "SELECT session_id FROM sa_assignments
             WHERE ticket_id=?1 AND id=?2 AND state='running'",
            params![&ticket.id, ticket.active_assignment_id.as_deref()],
            |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?;
        session_id.as_deref().is_some_and(|id|
            matches!(read_only_policy(store, id), Ok(Some(_))))
    } else {
        false
    };
    let terminal = matches!(ticket.state.as_str(),
        "completed" | "failed" | "stopped" | "interrupted" | "inconclusive");
    let next_action = if ticket.state == "running" && !consistent {
        "investigate_native_assignment_state"
    } else if ticket.state == "running" && expired_running > 0 {
        "investigate_expired_native_assignment_before_retry"
    } else if ticket.state == "running" && !active_read_only_authorized {
        // Contain the live provider before reconciling an uncertain effect;
        // neither action may imply that the remote operation succeeded.
        "stop_or_quarantine_revoked_native_execution"
    } else if !consistent {
        // A terminal Ticket can still have a live orphan provider attempt.
        // Contain that native execution before acting on uncertain remote
        // effects; reconciliation cannot race an unrevoked provider.
        "investigate_native_assignment_state"
    } else if pending + uncertain > 0 {
        "reconcile_authoritative_remote_effects"
    } else if running > 0 {
        "verify_live_serve_provider_before_intervention"
    } else if ticket.state == "waiting_review" {
        "await_fresh_task_bound_native_council"
    } else if ticket.state == "waiting_approval" {
        "decide_with_current_ticket_revision"
    } else if terminal {
        "do_not_replay_terminal_ticket"
    } else {
        "explicit_native_admission_or_operator_review_required"
    };
    Ok(serde_json::json!({
        "observedAt": now,
        "activeNativeAssignments": running,
        "missingNativeSessions": missing_sessions,
        "expiredNativeAssignments": expired_running,
        "activeAssignmentLinked": active_linked == 1,
        "assignmentStateConsistent": consistent,
        "activeNativeReadOnlyAdmissionValid": active_read_only_authorized,
        "pendingExternalEffects": pending,
        "uncertainExternalEffects": uncertain,
        "requiresIndependentRemoteReconciliation": pending + uncertain > 0,
        "requiresLiveHostVerification": running > 0 || pending > 0,
        "mandateRevoked": mandate.revoked,
        "mandateExpired": mandate.expires_at <= now,
        "terminal": terminal,
        "nextOperatorAction": next_action,
        "automaticReplayAllowed": false,
        "observationalOnly": true
    }))
}

/// Preflight an exact headless Demand objective against the same native
/// manual policy used by the desktop. This reads an existing database without
/// startup recovery or provider admission; live process occupancy is unknown.
pub(crate) fn inspect_headless_demand_preflight(
    database_path: &std::path::Path, orchestration_id: &str,
    objective: &str, coordinated: bool,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let status = manual_preflight(&store, orchestration_id, objective, coordinated, false)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": status.orchestration_id,
        "policyRevision": status.policy_revision,
        "projectId": status.project_id,
        "selectedRole": status.selected_role,
        "selectedBotId": status.selected_bot_id,
        "eligibleAtSnapshot": status.eligible,
        "canStageBlocked": status.can_stage_blocked,
        "reason": status.reason,
        "objectiveBytes": objective.trim().len(),
        "allowedTools": status.allowed_tools,
        "allowedSideEffects": status.allowed_side_effects,
        "attemptBudgetMs": status.attempt_budget_ms,
        "requiresLiveAdmissionRecheck": true,
        "mandateIssued": false,
        "providerDispatched": false,
        "observationalOnly": true
    })).map_err(|e| e.to_string())
}

/// Observe native Team readiness beside a live headless process without
/// running migration or startup recovery. This shares the desktop's native
/// manual role/checkout checks, but a detached CLI cannot attest live in-memory
/// provider occupancy. Actual admission MUST independently revalidate.
pub(crate) fn inspect_headless_team_readiness(
    database_path: &std::path::Path, orchestration_id: &str,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let worker = manual_preflight(&store, orchestration_id, "Headless readiness probe", false, false)?;
    let coordinator = manual_preflight(&store, orchestration_id, "Headless readiness probe", true, false)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": orchestration_id,
        "policyRevision": worker.policy_revision,
        "projectId": worker.project_id,
        "worker": {
            "botId": worker.selected_bot_id,
            "eligibleAtSnapshot": worker.eligible,
            "reason": worker.reason
        },
        "coordinator": {
            "botId": coordinator.selected_bot_id,
            "eligibleAtSnapshot": coordinator.eligible,
            "reason": coordinator.reason
        },
        "allowedTools": [],
        "allowedSideEffects": false,
        "requiresLiveAdmissionRecheck": true,
        "admitted": false,
        "attemptBudgetMs": worker.attempt_budget_ms
    })).map_err(|e| e.to_string())
}

/// Redacted to native receipt metadata; no external provider call, replay,
/// model text or recovery mutation. Inspecting a live serve database cannot
/// open Store::open because startup recovery interrupts active assignments.
/// Read the persisted roster and existing bot profiles without opening a
/// provider or relying on a desktop Webview. Unlike a live capability scan,
/// assigned Skills alone cannot prove installation, harness support or use.
pub(crate) fn inspect_headless_team_roster(
    database_path: &std::path::Path, orchestration_id: &str,
) -> Result<String> {
    let store=Store::open_read_only(database_path)?;
    let config=configuration(&store,orchestration_id)?;
    let roster=roster::read(&store,orchestration_id)?;
    let mut members=Vec::new();
    let mut append=|role:&str,position:usize,bot_id:&str| -> Result<()> {
        let bot=bots::get(&store,bot_id)?;
        members.push(serde_json::json!({
            "role":role, "position":position, "botId":bot.id,
            "name":bot.name, "provider":bot.provider, "model":bot.model,
            "mode":bot.mode, "paused":bot.paused,
            "assignedSkillIds":bot.skills,
            "harnessCapability":"not_verified",
            "skillLibraryStatus":"not_scanned",
            "invocationEvidence":"not_verified"
        }));
        Ok(())
    };
    append("coordinator",0,&roster.coordinator_bot_id)?;
    for (position,id) in roster.worker_bot_ids.iter().enumerate() {
        append("worker",position,id)?;
    }
    for (position,id) in roster.reviewer_bot_ids.iter().enumerate() {
        append("reviewer",position,id)?;
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId":config.id,
        "policyRevision":config.policy_revision,
        "enabled":config.enabled,
        "members":members,
        "manualSingleWorkerTopology":roster.execution_ready,
        "multiRoleExecutionAvailable":false,
        "allowedTools":[],
        "allowedSideEffects":false,
        "authorizationIssued":false,
        "observationalOnly":true,
        "requiresLiveAdmissionRecheck":true
    })).map_err(|e|e.to_string())
}

pub(crate) fn inspect_headless_effect_receipts(
    database_path: &std::path::Path, ticket_id: &str,
) -> Result<String> {
    inspect_headless_effect_receipts_page(database_path, ticket_id, None, None, false)
}

/// Page native effect history without Store restart recovery or provider calls.
pub(crate) fn inspect_headless_effect_receipts_page(
    database_path: &std::path::Path, ticket_id: &str,
    before_created_at: Option<i64>, before_key: Option<&str>, unresolved_only: bool,
) -> Result<String> {
    if before_created_at.is_some() != before_key.is_some() {
        return Err("Effect receipt cursor requires both timestamp and key.".into());
    }
    let store = Store::open_read_only(database_path)?;
    let (receipts, next_cursor) = effects::receipt_history_page(
        &store, ticket_id, 100, before_created_at.zip(before_key), unresolved_only,
    )?;
    let total_unresolved = effects::unresolved_count(&store, ticket_id)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket_id,
        "receipts": receipts,
        "nextCursor": next_cursor.map(|(created_at, effect_key)| serde_json::json!({
            "createdAt": created_at, "effectKey": effect_key
        })),
        "filter": if unresolved_only { "unresolved" } else { "all" },
        "totalUnresolved": total_unresolved,
        "observationalOnly": true,
        "requiresIndependentRemoteReconciliation": total_unresolved > 0,
        "replayed": false
    })).map_err(|e| e.to_string())
}


/// Inspect every unresolved effect across Tickets, rather than relying on
/// a bounded per-Ticket view or confusing a missing first-page item with safety.
/// Read-only snapshots do not start recovery or contact the remote provider.
pub(crate) fn list_headless_unresolved_effects(
    database_path: &std::path::Path,
    before_updated_at: Option<i64>, before_ticket_id: Option<&str>,
    before_effect_key: Option<&str>,
) -> Result<String> {
    list_headless_unresolved_effects_scoped(
        database_path,None,before_updated_at,before_ticket_id,before_effect_key,
    )
}

/// Read the same reconciliation queue with a native Team boundary, preserving
/// ticket revisions for subsequent CAS-guarded operator decisions.
pub(crate) fn list_headless_team_unresolved_effects(
    database_path: &std::path::Path, orchestration_id: &str,
    before_updated_at: Option<i64>, before_ticket_id: Option<&str>,
    before_effect_key: Option<&str>,
) -> Result<String> {
    list_headless_unresolved_effects_scoped(
        database_path,Some(orchestration_id),before_updated_at,before_ticket_id,before_effect_key,
    )
}

fn list_headless_unresolved_effects_scoped(
    database_path: &std::path::Path, orchestration_id: Option<&str>,
    before_updated_at: Option<i64>, before_ticket_id: Option<&str>,
    before_effect_key: Option<&str>,
) -> Result<String> {
    let before = match (before_updated_at, before_ticket_id, before_effect_key) {
        (None,None,None) => None,
        (Some(at),Some(ticket),Some(effect)) => Some((at,ticket,effect)),
        _ => return Err("Unresolved effect cursor requires timestamp, Ticket ID and effect key.".into()),
    };
    let store = Store::open_read_only(database_path)?;
    let snapshot = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let (items, next) = match orchestration_id {
        Some(team) => effects::unresolved_effects_page_for_team(
            &store, 100, before, Some(team),
        )?,
        None => effects::unresolved_effects_page(&store, 100, before)?,
    };
    let total: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM sa_effect_receipts e
         JOIN sa_tickets t ON t.id=e.ticket_id
         WHERE e.state IN ('pending','uncertain')
           AND (?1 IS NULL OR t.orchestration_id=?1)",
        [orchestration_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    snapshot.commit().map_err(|e| e.to_string())?;
    let mut result = serde_json::json!({
        "unresolvedTotal": total,
        "pageLimit": 100,
        "items": items,
        "nextCursor": next.map(|(updated_at,ticket_id,effect_key)| serde_json::json!({
            "updatedAt":updated_at, "ticketId":ticket_id, "effectKey":effect_key
        })),
        "observationalOnly": true,
        "automaticReplayAllowed": false,
        "requiresIndependentRemoteReconciliation": total > 0
    });
    if let Some(id) = orchestration_id {
        result["orchestrationId"] = serde_json::json!(id);
    }
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

/// One-shot headless Team Demand inspection. Reuse the desktop's native
/// state-filtered page; no migration, startup recovery, provider session,
/// inferred task, authorization or external effect. Operational metadata
/// excludes objectives, results, mandates and private transcripts.
pub(crate) fn list_headless_team_demands(
    database_path: &std::path::Path, orchestration_id: &str, state: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<String> {
    list_headless_team_demands_searched(
        database_path, orchestration_id, state, before_updated_at, before_id, None,
    )
}

/// Search applies the same pre-pagination native filters as desktop without
/// exposing the objective, query text, or any worker/session evidence.
pub(crate) fn list_headless_team_demands_searched(
    database_path: &std::path::Path, orchestration_id: &str, state: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>, search_query: Option<&str>,
) -> Result<String> {
    list_headless_team_demands_project_searched(
        database_path, orchestration_id, state, before_updated_at, before_id,
        search_query, None,
    )
}

/// Apply an exact admitted Project filter to the native Ticket index before
/// keyset pagination; current Team settings do not rewrite historical scope.
pub(crate) fn list_headless_team_demands_project_searched(
    database_path: &std::path::Path, orchestration_id: &str, state: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>,
    search_query: Option<&str>, project_id: Option<&str>,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let state_filter = if state == "all" { None } else { Some(state) };
    let page = list_all_demands_page_for_project(
        &store, 200, before_updated_at, before_id, DemandPageFilters {
            orchestration_id: Some(orchestration_id), state_filter,
            search_query, project_id,
        },
    )?;
    let may_have_more = page.len() == 200;
    let next_cursor = if may_have_more {
        page.last().map(|item| serde_json::json!({
            "beforeUpdatedAt": item.updated_at, "beforeId": &item.id
        }))
    } else { None };
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": orchestration_id,
        "stateFilter": state,
        "searchApplied": search_query.is_some(),
        "projectFilterApplied": project_id.is_some(),
        "pageLimit": 200,
        "mayHaveMore": may_have_more,
        "nextCursor": next_cursor,
        "observationalOnly": true,
        "demands": page.iter().map(|item| serde_json::json!({
            "ticketId": &item.id,
            "state": &item.state,
            "revision": item.revision,
            "updatedAt": item.updated_at
        })).collect::<Vec<_>>()
    })).map_err(|e| e.to_string())
}

/// Global headless Demand index using the desktop's native pre-pagination search.
/// The operator gets only Ticket IDs, Team IDs, states and revisions, not
/// objectives, actor identifiers, mandates, provider sessions or evidence.
pub(crate) fn list_headless_all_demands_searched(
    database_path: &std::path::Path, state: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>, search_query: Option<&str>,
) -> Result<String> {
    list_headless_all_demands_project_searched(
        database_path, state, before_updated_at, before_id, search_query, None,
    )
}

/// Global headless index shares desktop's immutable admitted Project filter.
/// Only redacted Ticket metadata is returned, never task/provider evidence.
pub(crate) fn list_headless_all_demands_project_searched(
    database_path: &std::path::Path, state: &str,
    before_updated_at: Option<i64>, before_id: Option<&str>,
    search_query: Option<&str>, project_id: Option<&str>,
) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let state_filter = if state == "all" { None } else { Some(state) };
    let page = list_all_demands_page_for_project(
        &store, 200, before_updated_at, before_id, DemandPageFilters {
            orchestration_id: None, state_filter, search_query, project_id,
        },
    )?;
    let may_have_more = page.len() == 200;
    let next_cursor = if may_have_more {
        page.last().map(|item| serde_json::json!({
            "beforeUpdatedAt": item.updated_at, "beforeId": &item.id
        }))
    } else { None };
    serde_json::to_string_pretty(&serde_json::json!({
        "scope": "allTeams",
        "stateFilter": state,
        "searchApplied": search_query.is_some(),
        "projectFilterApplied": project_id.is_some(),
        "pageLimit": 200,
        "mayHaveMore": may_have_more,
        "nextCursor": next_cursor,
        "observationalOnly": true,
        "demands": page.iter().map(|item| serde_json::json!({
            "ticketId": &item.id,
            "orchestrationId": &item.orchestration_id,
            "state": &item.state,
            "revision": item.revision,
            "updatedAt": item.updated_at
        })).collect::<Vec<_>>()
    })).map_err(|e| e.to_string())
}

/// Redacted cross-Team recovery queue for a trusted local operator. Only
/// genuine native Ticket state and unresolved durable effect receipts qualify:
/// a healthy running provider is deliberately omitted. This is NOT recovery
/// admission, permission issuance, or proof that a remote effect was applied.
/// One read transaction provides a consistent ordered operational snapshot.
/// Retain the legacy first-page entrypoint for desktop and existing callers.
pub(crate) fn list_headless_recovery_inbox(database_path: &std::path::Path) -> Result<String> {
    list_headless_recovery_inbox_page(database_path,None,None,None)
}

/// Cursor pagination keeps old unresolved remote effects discoverable beyond
/// the initial 200 results. Each call is a fresh read-only SQLite snapshot:
/// after concurrent changes, operators must reconcile current native state
/// again before attempting any authorized recovery.
pub(crate) fn list_headless_recovery_inbox_page(
    database_path: &std::path::Path, before_priority: Option<i64>,
    before_updated_at: Option<i64>, before_id: Option<&str>,
) -> Result<String> {
    list_headless_recovery_inbox_page_scoped(
        database_path, None, before_priority, before_updated_at, before_id,
    )
}

/// A Team-scoped operational recovery view. Apply the native Team boundary
/// before priority ordering and pagination, including cursor-anchor validation.
/// An operator cannot use a cursor from a different Team to inspect its Tickets.
pub(crate) fn list_headless_team_recovery_inbox_page(
    database_path: &std::path::Path, orchestration_id: &str,
    before_priority: Option<i64>, before_updated_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<String> {
    list_headless_recovery_inbox_page_scoped(
        database_path, Some(orchestration_id), before_priority, before_updated_at, before_id,
    )
}

fn list_headless_recovery_inbox_page_scoped(
    database_path: &std::path::Path, orchestration_id: Option<&str>,
    before_priority: Option<i64>, before_updated_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<String> {
    match (before_priority,before_updated_at,before_id) {
        (None,None,None)=>{},
        (Some(priority),Some(updated_at),Some(id))
            if (0..=2).contains(&priority) && updated_at>0
               && !id.is_empty() && id.len()<=128
               && id.bytes().all(|c|c.is_ascii_alphanumeric() || matches!(c,b'-'|b'_'))=>{},
        _=>return Err("Invalid recovery cursor: supply priority 0..2, positive timestamp and bounded Ticket ID.".into()),
    }
    let store = Store::open_read_only(database_path)?;
    if let Some(id) = orchestration_id {
        configuration(&store, id)?;
    }
    let snapshot = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let observed_at = storage::now();
    let cutoff = observed_at.saturating_sub(MAX_ASSIGNMENT_RUN_MS);
    let ids: Vec<(String, i64, i64)> = {
        let mut stmt = store.db.prepare(
            "WITH scoped_tickets AS (
               SELECT * FROM sa_tickets WHERE (?7 IS NULL OR orchestration_id=?7)
             ), candidates AS (
               SELECT t.id,t.updated_at,
                 CASE WHEN EXISTS(SELECT 1 FROM sa_effect_receipts e
                                  WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain'))
                      THEN 0 WHEN EXISTS(SELECT 1 FROM sa_assignments a
                                         WHERE a.ticket_id=t.id AND a.state='running')
                      THEN 1 ELSE 2 END AS priority
               FROM scoped_tickets t
               WHERE t.state IN ('queued','blocked','waiting_approval','waiting_review',
                                 'failed','interrupted','inconclusive')
                  OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                            WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain'))
                  OR (t.state<>'running' AND EXISTS(
                      SELECT 1 FROM sa_assignments a
                      WHERE a.ticket_id=t.id AND a.state='running'))
                  OR (t.state='running' AND (
                      t.active_assignment_id IS NULL
                      -- A live assignment whose provider session vanished
                      -- needs investigation even before the watchdog settles it.
                      OR EXISTS(SELECT 1 FROM sa_assignments a
                          LEFT JOIN sessions live ON live.id=a.session_id
                          WHERE a.ticket_id=t.id AND a.state='running'
                            AND live.id IS NULL)
                      -- The recovery index must find an active provider whose
                      -- task-owned channel binding was deleted or remapped.
                      -- Native read_only_policy already refuses this session;
                      -- otherwise the read-only inbox incorrectly appears clear.
                      OR NOT EXISTS(
                        SELECT 1 FROM sa_assignments a
                        JOIN bot_threads thread ON thread.session_id=a.session_id
                          AND thread.bot_id=a.bot_id AND thread.channel_id=?6
                        WHERE a.id=t.active_assignment_id AND a.ticket_id=t.id
                          AND a.state='running'
                          AND (
                            (a.attempt=1 AND thread.external_id=t.id)
                            OR (a.attempt=1 AND thread.external_id=t.id || ':attempt:1'
                                AND EXISTS(SELECT 1 FROM sa_audit initial
                                  WHERE initial.ticket_id=t.id AND initial.kind='admitted'
                                    AND initial.detail IN ('blocked: worker occupied',
                                      'blocked: explicit headless staging'))
                                AND EXISTS(SELECT 1 FROM sa_audit retry
                                  WHERE retry.ticket_id=t.id AND retry.kind='manual_retry'))
                            OR (a.attempt>1 AND
                                thread.external_id=t.id || ':attempt:' || a.attempt)
                            OR (a.attempt>1 AND
                                thread.external_id=t.id || ':worker:' || a.attempt)
                          ))
                      OR (SELECT COUNT(*) FROM sa_assignments a
                          WHERE a.ticket_id=t.id AND a.state='running')<>1
                      OR NOT EXISTS(SELECT 1 FROM sa_assignments a
                                    WHERE a.id=t.active_assignment_id
                                      AND a.ticket_id=t.id AND a.state='running'
                                      AND a.created_at>0 AND a.created_at<=?1
                                      AND a.created_at>?2)
                      -- Detect durable native authority drift even before
                      -- a watchdog tick. The full provider/tool admission
                      -- policy is independently rechecked in the projection.
                      OR NOT EXISTS(SELECT 1 FROM sa_mandates m
                                    WHERE m.ticket_id=t.id AND m.revoked=0
                                      AND m.expires_at>?1
                                      AND m.policy_revision=t.policy_revision
                                      AND m.project_id=t.project_id
                                      AND m.actor=t.actor
                                      AND length(trim(m.authorization_id))>0
                                      AND m.allowed_tools='[]'
                                      AND m.allowed_side_effects=0)
                      OR NOT EXISTS(
                        SELECT 1 FROM sa_orchestrations o
                        JOIN bots coordinator ON coordinator.id=o.coordinator_bot_id
                        JOIN sa_assignments active ON active.id=t.active_assignment_id
                          AND active.ticket_id=t.id AND active.state='running'
                        JOIN bots assignee ON assignee.id=active.bot_id
                        JOIN sessions live ON live.id=active.session_id
                        WHERE o.id=t.orchestration_id AND o.enabled=1
                          AND o.policy_revision=t.policy_revision
                          AND o.project_id=t.project_id
                          AND coordinator.paused=0 AND assignee.paused=0
                          AND live.project_id=assignee.project_id
                          AND (active.bot_id=o.coordinator_bot_id OR
                               (active.bot_id=t.worker_bot_id
                                AND assignee.project_id=t.project_id)))))
             ), page AS (
               SELECT id,priority,updated_at FROM candidates
               WHERE ?3 IS NULL OR priority>?3
                  OR (priority=?3 AND (updated_at<?4
                      OR (updated_at=?4 AND id<?5)))
               ORDER BY priority ASC,updated_at DESC,id DESC
               LIMIT 201
             )
             SELECT page.id,page.priority,page.updated_at,
               CASE WHEN ?3 IS NULL THEN 1 ELSE EXISTS(
                 SELECT 1 FROM candidates anchor
                 WHERE anchor.id=?5 AND anchor.priority=?3 AND anchor.updated_at=?4
               ) END AS cursor_valid
             FROM (SELECT 1) singleton LEFT JOIN page ON 1=1
             ORDER BY page.priority ASC,page.updated_at DESC,page.id DESC",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(
            params![observed_at,cutoff,before_priority,before_updated_at,before_id,CHANNEL,orchestration_id],
            |row| Ok((
                row.get::<_,Option<String>>(0)?,
                row.get::<_,Option<i64>>(1)?,
                row.get::<_,Option<i64>>(2)?,
                row.get::<_,bool>(3)?,
            )),
        ).map_err(|e| e.to_string())?;
        let records = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
        if records.first().is_some_and(|row| !row.3) {
            return Err("Recovery cursor no longer belongs to a recoverable Ticket; reload inbox.".into());
        }
        records.into_iter().filter_map(|(id,priority,updated_at,_)| {
            Some((id?,priority?,updated_at?))
        }).collect()
    };
    let mut items = Vec::with_capacity(ids.len().min(200));
    for (id,_,_) in ids.iter().take(200) {
        let ticket = get_ticket(&store,id)?;
        let recovery = headless_recovery_summary(&store,&ticket,observed_at)?;
        items.push(serde_json::json!({
            "ticketId": ticket.id,
            "orchestrationId": ticket.orchestration_id,
            "state": ticket.state,
            "revision": ticket.revision,
            "updatedAt": ticket.updated_at,
            "recovery": recovery
        }));
    }
    let next_cursor=if ids.len()>200 {
        ids.get(199).map(|(id,priority,updated_at)| serde_json::json!({
            "priority": priority,
            "updatedAt": updated_at,
            "ticketId": id
        }))
    } else {None};
    snapshot.commit().map_err(|e| e.to_string())?;
    let mut response = serde_json::json!({
        "observedAt": observed_at,
        "pageLimit": 200,
        "mayHaveMore": ids.len() > 200,
        "nextCursor": next_cursor,
        "items": items,
        "observationalOnly": true,
        "automaticRecoveryPerformed": false,
        "requiresCurrentNativeRecheckBeforeAnyAction": true
    });
    if let Some(id) = orchestration_id {
        response["orchestrationId"] = serde_json::json!(id);
    }
    serde_json::to_string_pretty(&response).map_err(|e| e.to_string())
}

/// Bounded local task index for a one-shot CLI. Keep this read-only so it can
/// run beside a healthy serve process without invoking Store::open recovery.
/// Ticket objectives, evidence, assignment results and mandate identities are
/// intentionally excluded from this operational listing.
pub(crate) fn list_headless_tasks(database_path: &std::path::Path) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    let mut stmt = store.db.prepare(
        "SELECT t.id,t.orchestration_id,t.state,t.revision,t.active_assignment_id,
                t.created_at,t.updated_at,
                (SELECT COUNT(*) FROM sa_effect_receipts r
                 WHERE r.ticket_id=t.id AND r.state!='applied')
         FROM sa_tickets t ORDER BY t.updated_at DESC,t.id DESC LIMIT 200",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(serde_json::json!({
        "ticketId": r.get::<_, String>(0)?,
        "orchestrationId": r.get::<_, String>(1)?,
        "state": r.get::<_, String>(2)?,
        "revision": r.get::<_, i64>(3)?,
        "activeAssignmentId": r.get::<_, Option<String>>(4)?,
        "createdAt": r.get::<_, i64>(5)?,
        "updatedAt": r.get::<_, i64>(6)?,
        "unresolvedExternalEffects": r.get::<_, i64>(7)?,
    }))).map_err(|e| e.to_string())?;
    let tickets = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&tickets).map_err(|e| e.to_string())
}

/// Open the live database for one explicitly requested Ticket transition.
/// Store::open is intentionally unsuitable here: its restart recovery would
/// interrupt healthy sessions owned by a concurrently running serve process.
/// This adapter requires the existing schema and opens the database without
/// create, migration, repair, or recovery behavior.
fn open_headless_transition_store(database_path: &std::path::Path) -> Result<Store> {
    let db = rusqlite::Connection::open_with_flags(
        database_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    ).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
    db.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
    let version: i64 = db.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version != storage::SCHEMA_VERSION {
        return Err(format!("Database schema {version} differs from expected {}; open PandaCode to migrate it.", storage::SCHEMA_VERSION));
    }
    Ok(Store { db, path: database_path.into() })
}

fn headless_ticket_status(ticket: &Ticket) -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket.id,
        "state": ticket.state,
        "revision": ticket.revision,
    })).map_err(|e| e.to_string())
}

/// An operator-issued headless Demand is durable and idempotent but deliberately
/// blocked: no provider session, inferred launch, additional tools or effects.
/// The existing native manual retry must independently recheck all admission
/// conditions before it can dispatch this Ticket.
pub(crate) fn headless_queue_demand(
    database_path: &std::path::Path, orchestration_id: &str,
    objective: &str, request_key: &str,
) -> Result<String> {
    let mut store = open_headless_transition_store(database_path)?;
    let (ticket, launched) = admit_role_with_staging(
        &mut store, orchestration_id, objective, request_key, true, false, true,
    )?;
    debug_assert!(!launched);
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket.id,
        "state": ticket.state,
        "revision": ticket.revision,
        "providerDispatchedByCommand": false,
        "requiresExplicitNativeDispatch": ticket.state == "blocked",
        "allowedTools": [],
        "allowedSideEffects": false,
    })).map_err(|e| e.to_string())
}

/// Explicit local headless Team rename uses the same desktop CAS and guard
/// without startup recovery. This never changes bot identities, roles, grants
/// or the historical Ticket snapshot; an active Team is always rejected.
pub(crate) fn headless_rename_orchestration(
    database_path: &std::path::Path, orchestration_id: &str,
    expected_revision: i64, name: &str,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let existing = configuration(&store, orchestration_id)?;
    let updated = update_configuration(&store, orchestration_id, expected_revision,
        &NewOrchestration {
            name: name.to_owned(),
            coordinator_bot_id: existing.coordinator_bot_id,
            worker_bot_id: existing.worker_bot_id,
        })?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.id,
        "name": updated.name,
        "enabled": updated.enabled,
        "policyRevision": updated.policy_revision,
        "grantIssued": false,
    })).map_err(|e| e.to_string())
}

/// A local headless operator edits the exact existing Team roster, using the
/// same revision-guarded, disabled-only transaction as the desktop. Adding a
/// member never issues a task grant or admits a provider; the current
/// multi-member execution limitation remains visible to the operator.
pub(crate) fn headless_set_team_roster(
    database_path: &std::path::Path, orchestration_id: &str,
    expected_revision: i64, input: &roster::RosterInput,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let updated = roster::set(&store, orchestration_id, expected_revision, input)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.orchestration_id,
        "policyRevision": updated.policy_revision,
        "workerCount": updated.worker_bot_ids.len(),
        "reviewerCount": updated.reviewer_bot_ids.len(),
        "executionReady": updated.execution_ready,
        "grantIssued": false,
        "providerDispatched": false,
    })).map_err(|e| e.to_string())
}

/// A local headless operator may choose a finite Worker retry budget for
/// future Demands. The native policy CAS rejects unsettled Tickets and
/// uncertain external effects; this operation does not retry existing work.
pub(crate) fn headless_set_team_attempt_limit(
    database_path: &std::path::Path, orchestration_id: &str,
    expected_revision: i64, limit: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let updated = roster::set_worker_attempt_limit(
        &store, orchestration_id, expected_revision, limit,
    )?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.orchestration_id,
        "policyRevision": updated.policy_revision,
        "maxWorkerAttempts": updated.max_worker_attempts,
        "grantIssued": false,
        "providerDispatched": false,
    })).map_err(|e| e.to_string())
}

/// Local headless strategy changes reuse the desktop's native Team policy
/// transaction. A purpose file is read by the CLI, never echoed as a command
/// argument or treated as authority. Only future Demand snapshots change.
pub(crate) fn headless_set_team_strategy(
    database_path: &std::path::Path, orchestration_id: &str,
    expected_revision: i64, purpose: &str, strategy: &str,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let updated = roster::set_team_strategy(
        &store, orchestration_id, expected_revision, purpose, strategy,
    )?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.orchestration_id,
        "policyRevision": updated.policy_revision,
        // The purpose was read from a private local file; do not reflect it
        // into terminal logs or captured automation output.
        "purposeConfigured": !updated.purpose.is_empty(),
        "strategy": updated.strategy,
        "grantIssued": false,
        "providerDispatched": false,
    })).map_err(|e| e.to_string())
}

/// Explicit local CLI acceptance; the native revision guard and audit remain
/// the same as the desktop action.
pub(crate) fn headless_accept_ticket(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    headless_ticket_status(&accept(&store, ticket_id, expected_revision)?)
}

/// Explicit local CLI rejection; the native reason validation, revision guard
/// and audit remain the same as the desktop action.
pub(crate) fn headless_reject_ticket(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64, reason: &str,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    headless_ticket_status(&reject(&store, ticket_id, expected_revision, reason)?)
}

/// Explicit local operator enable uses the same native policy CAS and
/// harness/profile admission checks as desktop. Enabled means available for
/// future manual Demands, never an inferred Ticket, grant or provider launch.
pub(crate) fn headless_enable_orchestration(
    database_path: &std::path::Path, orchestration_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let updated = change_enabled(&store, orchestration_id, expected_revision, true)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.id,
        "enabled": updated.enabled,
        "policyRevision": updated.policy_revision,
        "grantIssued": false,
        "providerDispatched": false,
    })).map_err(|e| e.to_string())
}

/// Local operator emergency disable. Reuse the desktop's native CAS,
/// audit, lease settlement and revocation without invoking restart recovery
/// against a live host. Never dispatch work or implicitly re-enable a Team.
pub(crate) fn headless_disable_orchestration(
    database_path: &std::path::Path, orchestration_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let updated = change_enabled(&store, orchestration_id, expected_revision, false)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "orchestrationId": updated.id,
        "enabled": updated.enabled,
        "policyRevision": updated.policy_revision,
    })).map_err(|e| e.to_string())
}

/// Explicit local CLI Stop revokes the mandate in the shared database. A
/// running serve process may still have a provider call in memory; its late
/// callback is denied by the existing state/mandate checks in on_run_finished.
pub(crate) fn headless_stop_ticket(database_path: &std::path::Path, ticket_id: &str) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let (ticket, _session) = stop(&store, ticket_id)?;
    headless_ticket_status(&ticket)
}

/// Revision-bound headless Stop prevents a stale observation from revoking
/// a Ticket changed by another operator. Emergency Stop remains unconditional.
pub(crate) fn headless_stop_ticket_at_revision(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let (ticket, _session) =
        stop_with_expected_revision(&store, ticket_id, Some(expected_revision))?;
    headless_ticket_status(&ticket)
}

/// Explicit local-operator quarantine for one exact pending effect receipt.
/// An unknown provider outcome is never interpreted as failure or success.
/// CAS on Ticket revision and the stored payload digest prevents a stale
/// operator view from changing a different receipt. The audit and transition
/// are atomic; a repeated request for an already uncertain receipt is a no-op.
pub(crate) fn headless_quarantine_pending_effect(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
    effect_key: &str, payload_sha256: &str,
) -> Result<String> {
    if expected_revision < 0
        || effect_key.is_empty() || effect_key.len() > 128
        || !effect_key.bytes().all(|b| b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'_' | b'.' | b':'))
        || payload_sha256.len() != 64
        || !payload_sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("Invalid pending effect quarantine identity.".into());
    }
    let store = open_headless_transition_store(database_path)?;
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let revision: i64 = tx.query_row(
        "SELECT revision FROM sa_tickets WHERE id=?1",
        [ticket_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("SuperAgent Ticket not found.")?;
    if revision != expected_revision {
        return Err("SuperAgent Ticket revision changed before effect quarantine.".into());
    }
    let (state, original_digest): (String, String) = tx.query_row(
        "SELECT state,payload_sha256 FROM sa_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2",
        params![ticket_id,effect_key],
        |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("External effect receipt not found.")?;
    if original_digest != payload_sha256 {
        return Err("External effect payload identity changed.".into());
    }
    let changed = match state.as_str() {
        "pending" => {
            let now = storage::now();
            if tx.execute(
                "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?4
                 WHERE ticket_id=?1 AND effect_key=?2
                   AND payload_sha256=?3 AND state='pending'",
                params![ticket_id,effect_key,payload_sha256,now],
            ).map_err(|e| e.to_string())? != 1 {
                return Err("External effect changed during quarantine.".into());
            }
            tx.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'headless_effect_quarantined',?2,?3)",
                params![ticket_id,
                    format!("Receipt {effect_key} quarantined; independently verify provider outcome before retry"),
                    now],
            ).map_err(|e| e.to_string())?;
            true
        }
        "uncertain" => false,
        _ => return Err("Only pending or already uncertain effects may be quarantined.".into()),
    };
    tx.commit().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket_id,
        "effectKey": effect_key,
        "state": "uncertain",
        "changed": changed,
        "remoteResultVerified": false,
        "providerDispatchedByCommand": false,
        "externalEffectReplayAllowed": false,
    })).map_err(|e| e.to_string())
}

/// Quarantine an explicit, revisioned running Ticket whose single active native
/// Assignment has no SQLite provider session. The detached process never
/// claims a provider was stopped, replays a remote effect or grants a retry.
pub(crate) fn headless_quarantine_detached_ticket(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let (state, revision, active): (String, i64, Option<String>) = tx.query_row(
        "SELECT state,revision,active_assignment_id FROM sa_tickets WHERE id=?1",
        [ticket_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("SuperAgent Ticket not found.")?;
    if revision != expected_revision {
        return Err("SuperAgent Ticket revision changed before quarantine.".into());
    }
    if state != "running" {
        return Err("Only a running Ticket with a detached session may be quarantined.".into());
    }
    let active = active.ok_or("Missing active Assignment; inspect recovery state first.")?;
    let (running, linked, missing): (i64,i64,i64) = tx.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN a.id=?2 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN a.id=?2 AND s.id IS NULL THEN 1 ELSE 0 END),0)
         FROM sa_assignments a LEFT JOIN sessions s ON s.id=a.session_id
         WHERE a.ticket_id=?1 AND a.state='running'",
        params![ticket_id,&active],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).map_err(|e| e.to_string())?;
    if running != 1 || linked != 1 || missing != 1 {
        return Err("Expected exactly one linked running Assignment with a missing native session.".into());
    }
    let now=storage::now();
    if tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[ticket_id])
        .map_err(|e| e.to_string())? != 1 {
        return Err("SuperAgent Mandate missing; quarantine aborted.".into());
    }
    if tx.execute(
        "UPDATE sa_tickets SET state='interrupted',active_assignment_id=NULL,
         revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2 AND state='running' AND active_assignment_id=?4",
        params![ticket_id,expected_revision,now,&active],
    ).map_err(|e| e.to_string())? != 1 {
        return Err("Ticket changed before detached quarantine.".into());
    }
    if tx.execute(
        "UPDATE sa_assignments SET state='interrupted',updated_at=?2
         WHERE ticket_id=?1 AND id=?3 AND state='running'",
        params![ticket_id,now,&active],
    ).map_err(|e| e.to_string())? != 1 {
        return Err("Assignment changed before detached quarantine.".into());
    }
    let uncertain=tx.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
         WHERE ticket_id=?1 AND state='pending'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_detached_quarantined',
           'Missing native session; revoked without provider or effect replay',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId":ticket_id,"state":"interrupted","revision":revision+1,
        "quarantinedAssignments":1,"pendingEffectsMarkedUncertain":uncertain,
        "providerStoppedByCommand":false,"liveProviderState":"unknown",
        "externalEffectReplayAllowed":false
    })).map_err(|e| e.to_string())
}

/// A trusted local operator can quarantine an expired or invalid-clock native
/// Assignment without opening startup recovery beside a live serve process.
/// This is not a provider stop or permission to retry: the owning serve host
/// observes the durable revocation and cancels its task-owned handle.
pub(crate) fn headless_quarantine_expired_ticket(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let (state, revision, active): (String, i64, Option<String>) = tx.query_row(
        "SELECT state,revision,active_assignment_id FROM sa_tickets WHERE id=?1",
        [ticket_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("SuperAgent Ticket not found.")?;
    if revision != expected_revision {
        return Err("SuperAgent Ticket revision changed before expiry quarantine.".into());
    }
    if state != "running" {
        return Err("Only a running Ticket can have an expired Assignment quarantined.".into());
    }
    let active = active.ok_or("Running Ticket has no active Assignment; inspect recovery state.")?;
    let running: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1 AND state='running'",
        [ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if running != 1 {
        return Err("Expiry quarantine requires exactly one running native Assignment.".into());
    }
    let started_at: i64 = tx.query_row(
        "SELECT created_at FROM sa_assignments
         WHERE id=?1 AND ticket_id=?2 AND state='running'",
        params![&active,ticket_id], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("Active native Assignment does not match the running Ticket.")?;
    let now = storage::now();
    if assignment_within_budget(started_at, now) {
        return Err("Assignment is within its native run budget; expiry quarantine denied.".into());
    }
    if tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
        [ticket_id]).map_err(|e| e.to_string())? != 1 {
        return Err("SuperAgent Mandate missing; expiry quarantine aborted.".into());
    }
    if tx.execute(
        "UPDATE sa_tickets SET state='interrupted',active_assignment_id=NULL,
         revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2 AND state='running' AND active_assignment_id=?4",
        params![ticket_id,expected_revision,now,&active],
    ).map_err(|e| e.to_string())? != 1 {
        return Err("Ticket changed before expiry quarantine.".into());
    }
    if tx.execute(
        "UPDATE sa_assignments SET state='interrupted',updated_at=?2
         WHERE id=?1 AND ticket_id=?3 AND state='running'",
        params![&active,now,ticket_id],
    ).map_err(|e| e.to_string())? != 1 {
        return Err("Assignment changed before expiry quarantine.".into());
    }
    let uncertain = tx.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
         WHERE ticket_id=?1 AND state='pending'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_expired_quarantined',
           'Expired or invalid-clock native assignment revoked; verify live provider and external effects before retry',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId":ticket_id, "state":"interrupted", "revision":revision+1,
        "quarantinedAssignments":1, "pendingEffectsMarkedUncertain":uncertain,
        "providerStoppedByCommand":false, "liveProviderState":"unknown",
        "externalEffectReplayAllowed":false
    })).map_err(|e| e.to_string())
}

/// Explicitly quarantine an orphaned provider attempt on a terminal Ticket.
/// A detached headless process never replays effects or claims to stop the provider.
pub(crate) fn headless_quarantine_terminal_orphan(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let (state, revision): (String, i64) = tx.query_row(
        "SELECT state,revision FROM sa_tickets WHERE id=?1",
        [ticket_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("SuperAgent Ticket not found.")?;
    if revision != expected_revision {
        return Err("SuperAgent Ticket revision changed before quarantine.".into());
    }
    if !matches!(state.as_str(), "completed" | "failed" | "stopped"
        | "interrupted" | "inconclusive")
    {
        return Err("Only a terminal Ticket with an orphaned attempt can be quarantined.".into());
    }
    let running: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1 AND state='running'",
        [ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if running == 0 {
        return Err("No orphaned running native assignment to quarantine.".into());
    }
    let now = storage::now();
    if tx.execute(
        "UPDATE sa_tickets SET active_assignment_id=NULL,revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2
         AND state IN ('completed','failed','stopped','interrupted','inconclusive')",
        params![ticket_id,expected_revision,now],
    ).map_err(|e| e.to_string())? != 1 {
        return Err("SuperAgent Ticket changed before quarantine.".into());
    }
    tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
        [ticket_id]).map_err(|e| e.to_string())?;
    let quarantined = tx.execute(
        "UPDATE sa_assignments SET state='interrupted',updated_at=?2
         WHERE ticket_id=?1 AND state='running'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    let uncertain = tx.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
         WHERE ticket_id=?1 AND state='pending'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_orphan_quarantined',
           'Terminal native assignment quarantined; no provider or effect replay',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket_id, "state": state, "revision": revision + 1,
        "quarantinedAssignments": quarantined, "pendingEffectsMarkedUncertain": uncertain,
        "providerStoppedByCommand": false, "liveProviderState": "unknown",
        "externalEffectReplayAllowed": false,
    })).map_err(|e| e.to_string())
}

/// Revoke and interrupt assignments that exceeded the same native attempt
/// budget used by tool admission. This is called only by the owning serve
/// process; callers must stop the returned sessions after the transaction.
/// No attempt is replayed and unresolved effect receipts stay unresolved.
/// Return task-owned provider sessions that a separate local headless
/// control command has already stopped or interrupted in SQLite. A live serve
/// process can observe those revocations without taking a second writer lease,
/// attempting a restart recovery, or stopping an unrelated ordinary bot session.
/// This read never treats a stopped Ticket as a new dispatch/retry permission.
/// A native SuperAgent session is never eligible for unattended queue replay or
/// usage-limit resume, even after Stop, settlement, or process restart. Query
/// assignment history rather than a live-only policy or a bot-thread record:
/// those can disappear or become invalid after revocation. Ordinary sessions
/// retain their existing automatic-resume behavior.
/// A task-owned internal bot thread remains a SuperAgent session even if a
/// damaged or legacy store has lost its Assignment row. Never let headless
/// queue/usage-limit recovery or the live-provider cancellation poll fall back
/// to ordinary session privileges after that authority record disappears.
pub(crate) fn has_task_assignment_history(store: &Store, session_id: &str) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE session_id=?1)
             OR EXISTS(SELECT 1 FROM bot_threads
                       WHERE session_id=?1 AND channel_id=?2)",
        params![session_id, CHANNEL], |row| row.get(0),
    ).map_err(|error| error.to_string())
}

/// A live server may still own a provider handle after a local operator
/// revokes a Mandate or after expiry, pause, policy/profile drift, or recovery
/// detaches an Assignment. Poll native authority directly so cancellation need
/// not wait for a state-writing watchdog tick. Stop only task-owned sessions;
/// this query never settles the Ticket, grants a retry, or replays effects.
pub(crate) fn headless_cancelled_assignments(
    store: &Store, active_sessions: &[String],
) -> Result<Vec<String>> {
    headless_cancelled_assignments_with_pause(
        store, active_sessions, crate::feature_controls::get().bots_paused,
    )
}

// Inject the pause state to test native cancellation without changing global settings.
fn headless_cancelled_assignments_with_pause(
    store: &Store, active_sessions: &[String], bots_paused: bool,
) -> Result<Vec<String>> {
    if active_sessions.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = store.db.prepare(
        "SELECT EXISTS(
            SELECT 1 FROM sa_assignments a
            JOIN sa_tickets t ON t.id=a.ticket_id
            LEFT JOIN sa_mandates m ON m.ticket_id=t.id
            LEFT JOIN sa_orchestrations o ON o.id=t.orchestration_id
            LEFT JOIN bots b ON b.id=a.bot_id
            LEFT JOIN bots coordinator ON coordinator.id=o.coordinator_bot_id
            LEFT JOIN sessions s ON s.id=a.session_id
            WHERE a.session_id=?1
              AND (a.state!='running'
                   OR t.state IN ('stopped','interrupted')
                   OR (a.state='running' AND (
                       ?4=1
                       OR t.state!='running'
                       OR t.active_assignment_id IS NULL
                       OR t.active_assignment_id!=a.id
                       OR m.ticket_id IS NULL
                       OR m.revoked!=0
                       OR m.expires_at<=?2
                       OR length(trim(m.authorization_id))=0
                       OR m.actor!=t.actor
                       OR m.project_id!=t.project_id
                       OR m.allowed_tools!='[]'
                       OR m.allowed_side_effects!=0
                       OR m.policy_revision!=t.policy_revision
                       OR o.id IS NULL OR o.enabled=0
                       OR o.project_id!=t.project_id
                       OR o.policy_revision!=t.policy_revision
                       OR coordinator.id IS NULL OR coordinator.paused!=0
                       OR b.id IS NULL OR b.paused!=0
                       OR (a.bot_id!=o.coordinator_bot_id AND
                           (a.bot_id!=t.worker_bot_id OR b.project_id!=t.project_id))
                       OR s.id IS NULL OR s.project_id!=b.project_id
                       OR a.created_at<=0 OR a.created_at>?2
                       OR (?2-a.created_at)>=?3
                   )))
        )",
    ).map_err(|e| e.to_string())?;
    let mut cancelled = Vec::new();
    let now = storage::now();
    for session_id in active_sessions {
        let cancelled_native: bool = stmt.query_row(
            params![session_id, now, MAX_ASSIGNMENT_RUN_MS, i64::from(bots_paused)], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        // The SQL fast path detects durable revocation and role drift. It does
        // not encode the entire admission-time profile and task-thread proof.
        // Recheck those exact native invariants before retaining a live task
        // handle; a model, harness, checkout or thread change must cancel it
        // without waiting for the watchdog to settle the Ticket.
        let task_policy_invalid = !cancelled_native
            && has_task_assignment_history(store, session_id)?
            && !matches!(read_only_policy(store, session_id), Ok(Some(_)));
        if (cancelled_native || task_policy_invalid) && !cancelled.contains(session_id) {
            cancelled.push(session_id.clone());
        }
    }
    Ok(cancelled)
}

pub(crate) fn watchdog_expired_assignments(store: &Store, now: i64) -> Result<Vec<String>> {
    // Only the owning serve process calls this. Inspect every live assignment:
    // limiting the SQL page before filtering healthy entries could starve a
    // later revoked assignment forever. No automatic retry is permitted; the
    // same ticket revision guard is used at settlement.
    let stopped = {
        let mut stmt = store.db.prepare(
            "SELECT a.id,a.ticket_id,a.session_id,t.revision,
                    CASE
                      WHEN a.created_at<=0 OR a.created_at>?1
                        THEN 'Invalid assignment clock; no replay'
                      WHEN (?1-a.created_at)>=?2
                        THEN 'Assignment exceeded its bounded run time; no replay'
                      WHEN ?3=1
                        THEN 'Bots paused; native execution authority revoked'
                      WHEN o.enabled=0
                        THEN 'Team disabled; no replay'
                      WHEN m.ticket_id IS NULL OR m.revoked!=0 OR m.expires_at<=?1
                           OR trim(m.authorization_id)='' OR m.actor!=t.actor
                           OR m.policy_revision!=o.policy_revision
                           OR t.policy_revision!=o.policy_revision
                           OR m.allowed_side_effects!=0 OR m.allowed_tools!='[]'
                           OR m.project_id!=t.project_id OR o.project_id!=t.project_id
                        THEN 'Mandate expired, revoked or stale; no replay'
                      WHEN coordinator.id IS NULL OR coordinator.paused!=0
                        THEN 'Coordinator unavailable; no replay'
                      -- The first coordinator attempt may be in its own
                      -- checkout, distinct from the Ticket's worker project.
                      -- Do not treat a different, unadmitted bot as that role.
                      WHEN worker.id IS NULL OR worker.paused!=0
                           OR (a.bot_id=o.coordinator_bot_id AND
                               (a.attempt!=1 OR NOT EXISTS(
                                  SELECT 1 FROM sa_audit admission
                                  WHERE admission.ticket_id=t.id
                                    AND admission.kind='admitted'
                                    AND admission.detail='manual readonly coordinator reserved'
                               )))
                           OR (a.bot_id!=o.coordinator_bot_id AND
                               (a.bot_id!=t.worker_bot_id OR worker.project_id!=t.project_id))
                        THEN 'Assigned bot unavailable or out of scope; no replay'
                      WHEN active_session.id IS NULL
                           OR active_session.project_id!=worker.project_id
                        THEN 'Assignment session missing or out of scope; no replay'
                      ELSE NULL
                    END
             FROM sa_assignments a
             JOIN sa_tickets t ON t.id=a.ticket_id
             JOIN sa_orchestrations o ON o.id=t.orchestration_id
             LEFT JOIN sa_mandates m ON m.ticket_id=t.id
             LEFT JOIN bots coordinator ON coordinator.id=o.coordinator_bot_id
             LEFT JOIN bots worker ON worker.id=a.bot_id
             LEFT JOIN sessions active_session ON active_session.id=a.session_id
             WHERE a.state='running' AND t.state='running'
               AND t.active_assignment_id=a.id
             ORDER BY a.created_at,a.id",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(
            params![now,MAX_ASSIGNMENT_RUN_MS,i64::from(crate::feature_controls::get().bots_paused)],
            |r| Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?,
                r.get::<_, String>(2)?, r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
            )),
        ).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?
    };
    let mut sessions = Vec::new();
    for (assignment_id,ticket_id,session_id,revision,reason) in stopped {
        let Some(reason) = reason else { continue };
        let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let changed = tx.execute(
            "UPDATE sa_tickets SET state='interrupted',active_assignment_id=NULL,
             revision=revision+1,updated_at=?3
             WHERE id=?1 AND state='running' AND active_assignment_id=?2 AND revision=?4",
            params![ticket_id,assignment_id,now,revision],
        ).map_err(|e| e.to_string())?;
        if changed == 0 {
            tx.rollback().map_err(|e| e.to_string())?;
            continue;
        }
        tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&ticket_id])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE sa_assignments SET state='interrupted',updated_at=?2
             WHERE id=?1 AND state='running'",
            params![assignment_id,now],
        ).map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
             WHERE ticket_id=?1 AND state='pending'",
            params![ticket_id,now],
        ).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'watchdog_interrupted',?2,?3)",
            params![ticket_id,reason,now],
        ).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        sessions.push(session_id);
    }
    Ok(sessions)
}

fn record(store: &Store, id: &str, kind: &str, detail: &str) -> Result<()> {
    store.db.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,?2,?3,?4)",
        params![id,kind,detail,storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// One native read of checkout/bot contention used both by the real admit
/// transition and the non-authoritative preview. Admission always repeats it.
fn checkout_path(store:&Store,project_id:&str)->Result<String> {
    let path:String=store.db.query_row(
        "SELECT path FROM projects WHERE id=?1",[project_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    if path.trim().is_empty() {return Err("Project checkout path is unavailable.".into());}
    Ok(path)
}

/// Two distinct stored project paths may resolve to the same physical checkout.
/// Compare harmless dot components even before a checkout exists, then resolve
/// physical aliases (including symlinks) whenever both paths are available.
/// Keep parent components intact: collapsing `..` through a symlink is unsafe.
/// This is an admission fence, not permission to write to a checkout.
fn checkout_paths_equivalent(left: &str, right: &str) -> bool {
    if left == right { return true; }
    if std::path::Path::new(left)
        .components()
        .filter(|component| *component != std::path::Component::CurDir)
        .eq(std::path::Path::new(right)
            .components()
            .filter(|component| *component != std::path::Component::CurDir))
    {
        return true;
    }
    match (canonical_checkout_intent(left), canonical_checkout_intent(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// A project rooted inside another project's checkout can edit the same
/// files even though its canonical path is not equal. Serialize native work
/// across ancestor/descendant roots, including as-yet-uncreated children,
/// without treating prefix-shaped siblings as the same checkout.
fn checkout_paths_overlap(left: &str, right: &str) -> bool {
    if checkout_paths_equivalent(left, right) {
        return true;
    }
    match (canonical_checkout_intent(left), canonical_checkout_intent(right)) {
        (Some(left), Some(right)) => left.starts_with(&right) || right.starts_with(&left),
        // Unknown filesystem identity is not proof of disjoint write scopes.
        // An invalid path, dangling symlink or denied ancestor must never
        // let a second provider bypass the native checkout lease.
        _ => true,
    }
}

/// Resolve the existing checkout ancestor without creating the requested
/// directory. A missing leaf beneath a symlinked parent must still share the
/// same native resource lease as the corresponding physical checkout path.
/// A permission error, dangling symlink or unresolved parent component does
/// not prove that the paths are equivalent.
fn canonical_checkout_intent(path: &str) -> Option<std::path::PathBuf> {
    let source = std::path::Path::new(path);
    let absolute = if source.is_absolute() {
        source.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(source)
    };
    let mut cursor = absolute.as_path();
    let mut unresolved = Vec::new();
    loop {
        match std::fs::canonicalize(cursor) {
            Ok(mut resolved) => {
                for segment in unresolved.into_iter().rev() {
                    resolved.push(segment);
                }
                return Some(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A dangling symlink is not a safe ordinary missing directory.
                if std::fs::symlink_metadata(cursor).is_ok() {
                    return None;
                }
                unresolved.push(cursor.file_name()?.to_os_string());
                cursor = cursor.parent()?;
            }
            Err(_) => return None,
        }
    }
}

/// Native admission must account for ordinary running chats/direct tasks,
/// not just persisted SuperAgent leases. Unknown live sessions fail closed.
fn active_manual_conflict(
    store: &Store, active: &[String], candidate_id: &str, project_id: &str,
) -> Result<bool> {
    let requested_path=checkout_path(store,project_id)?;
    for session_id in active {
        let session = store.session(session_id)
            .map_err(|_| "Active provider session is unavailable; checkout admission denied.".to_string())?;
        if session.project_id == project_id
            || checkout_paths_overlap(
                &checkout_path(store,&session.project_id)?, &requested_path,
            ) {
            return Ok(true);
        }
        if bots::owner_of(store, session_id)?
            .is_some_and(|(bot, _)| bot.id == candidate_id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn manual_candidate_occupied(
    store: &Store, candidate_id: &str, team_project: &str, candidate_project: &str,
) -> Result<bool> {
    let team_path=checkout_path(store,team_project)?;
    let candidate_path=checkout_path(store,candidate_project)?;
    let exact:bool=store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_assignments a
           JOIN sa_tickets previous ON previous.id=a.ticket_id
           LEFT JOIN sessions active_session ON active_session.id=a.session_id
           WHERE a.state='running'
             AND (a.bot_id=?1 OR previous.project_id IN (?2,?3)
                  OR active_session.project_id IN (?2,?3) OR active_session.id IS NULL
                   OR EXISTS(SELECT 1 FROM projects old_checkout,projects new_checkout
                             WHERE old_checkout.id IN (previous.project_id,active_session.project_id)
                               AND new_checkout.id IN (?2,?3)
                               AND old_checkout.path=new_checkout.path))
         )",
        params![candidate_id,team_project,candidate_project], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if exact { return Ok(true); }
    // SQL equality cannot detect different stored paths for one real checkout.
    // Resolve only live native assignments; a missing origin fails closed.
    let mut statement=store.db.prepare(
        "SELECT previous.project_id, active_session.project_id
         FROM sa_assignments a
         JOIN sa_tickets previous ON previous.id=a.ticket_id
         LEFT JOIN sessions active_session ON active_session.id=a.session_id
         WHERE a.state='running'",
    ).map_err(|e|e.to_string())?;
    let active=statement.query_map([],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,
    ))).map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    for (original,session_project) in active {
        for id in std::iter::once(original.as_str()).chain(session_project.as_deref()) {
            let path=checkout_path(store,id)?;
            if checkout_paths_overlap(&path,&team_path)
                || checkout_paths_overlap(&path,&candidate_path) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Read-only, time-sensitive advisory from the same native source data as
/// admission. It issues no Ticket, grant, session, lease or provider request.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualPreflight {
    pub orchestration_id: String,
    pub policy_revision: i64,
    pub project_id: String,
    pub selected_bot_id: String,
    pub selected_role: String,
    pub eligible: bool,
    pub reason: Option<String>,
    /// Only an occupied direct Worker can safely stage a blocked Ticket.
    /// This is a read-only observation, not an admission or grant.
    pub can_stage_blocked: bool,
    pub allowed_tools: Vec<String>,
    pub allowed_side_effects: bool,
    pub attempt_budget_ms: i64,
}

fn manual_preflight(
    store: &Store, config_id: &str, objective: &str, coordinate: bool, busy: bool,
) -> Result<ManualPreflight> {
    let trimmed = objective.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_OBJECTIVE_BYTES || trimmed.contains('\0') {
        return Err("A nonempty, bounded objective is required.".into());
    }
    let cfg = configuration(store, config_id)?;
    let coordinator = bots::get(store, &cfg.coordinator_bot_id)?;
    let worker = bots::get(store, &cfg.worker_bot_id)?;
    let candidate = if coordinate { &coordinator } else { &worker };
    let (reason, can_stage_blocked) =
        if !execution_permitted(cfg.enabled, crate::feature_controls::get().bots_paused) {
            (Some("Team is disabled or bots are globally paused.".to_owned()), false)
        } else if coordinator.paused || worker.paused || worker.project_id != cfg.project_id {
            (Some("Selected bot is paused or outside the Team project.".to_owned()), false)
        } else if let Some(issue)=roster::member_execution_issue(&coordinator)
            .or_else(||roster::member_execution_issue(&worker)) {
            (Some(issue.to_owned()), false)
        } else if roster::has_additional_members(store, config_id)? {
            (Some("Multi-member execution requires native scoped role admission; manual runner is single-worker.".to_owned()), false)
        } else if busy || manual_candidate_occupied(store, &candidate.id, &cfg.project_id, &candidate.project_id)? {
            (Some(if coordinate {
                "Coordinator is occupied; this request cannot be admitted yet.".to_owned()
            } else {
                "Worker or checkout is occupied; manual admission may create a blocked Ticket.".to_owned()
            }), !coordinate)
        } else { (None, false) };
    Ok(ManualPreflight {
        orchestration_id: cfg.id, policy_revision: cfg.policy_revision,
        project_id: cfg.project_id, selected_bot_id: candidate.id.clone(),
        selected_role: if coordinate { "coordinator" } else { "worker" }.into(),
        eligible: reason.is_none(), reason, can_stage_blocked,
        allowed_tools: Vec::new(), allowed_side_effects: false,
        attempt_budget_ms: MAX_ASSIGNMENT_RUN_MS,
    })
}

/// Native manual request with a stable dedup key. A blocked request remains
/// auditable and must be explicitly retried; no background inference or launch.
fn admit(store: &mut Store, config_id: &str, objective: &str, request_key: &str, busy: bool) -> Result<(Ticket, bool)> {
    admit_role(store, config_id, objective, request_key, busy, false)
}

/// A coordinated manual request first reserves a native, read-only session for
/// the existing coordinator bot. Its output is advice, not a grant or a task
/// completion; the selected worker is admitted only by a separate native action.
fn admit_role(
    store: &mut Store, config_id: &str, objective: &str,
    request_key: &str, busy: bool, coordinate: bool,
) -> Result<(Ticket, bool)> {
    admit_role_with_staging(store, config_id, objective, request_key, busy, coordinate, false)
}

// A detached operator can stage a blocked Ticket but cannot reserve a provider
// session. This preserves the existing manual admission API and its tests.
fn admit_role_with_staging(
    store: &mut Store, config_id: &str, objective: &str,
    request_key: &str, busy: bool, coordinate: bool, headless_staging: bool,
) -> Result<(Ticket, bool)> {
    let objective = objective.trim();
    if objective.is_empty() || objective.len() > MAX_OBJECTIVE_BYTES || objective.chars().any(|c| c == '\0') {
        return Err("A nonempty, bounded objective is required.".into());
    }
    if request_key.is_empty() || request_key.len() > 128
        || !request_key.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err("Provide a stable alphanumeric manual-request key.".into());
    }
    if let Some(existing) = store.db.query_row(
        "SELECT id FROM sa_tickets WHERE orchestration_id=?1 AND request_key=?2",
        params![config_id,request_key], |row| row.get::<_, String>(0),
    ).optional().map_err(|e| e.to_string())? {
        let ticket = get_ticket(store, &existing)?;
        if ticket.objective != objective {
            return Err("The manual-request key is already bound to another objective.".into());
        }
        // A key identifies the original native dispatch intent as well as its
        // objective. Replaying a direct worker request as a coordinator request
        // (or vice versa) must never silently return the other execution mode.
        // The admission audit is durable even after the assignment finishes.
        let admission: String = store.db.query_row(
            "SELECT detail FROM sa_audit WHERE ticket_id=?1 AND kind='admitted' ORDER BY id LIMIT 1",
            [&ticket.id], |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?
            .ok_or("Manual request admission audit missing.")?;
        let originally_coordinated = match admission.as_str() {
            "manual readonly coordinator reserved" => true,
            "manual readonly worker reserved" | "blocked: worker occupied"
            | "blocked: explicit headless staging" => false,
            _ => return Err("Manual request admission mode is unknown.".into()),
        };
        if headless_staging && admission != "blocked: explicit headless staging" {
            return Err("Request key already belongs to a different manual dispatch mode.".into());
        }
        if originally_coordinated != coordinate {
            return Err("Manual request key cannot change between direct and coordinated dispatch.".into());
        }
        return Ok((ticket, false));
    }
    let config = configuration(store, config_id)?;
    if !execution_permitted(config.enabled, crate::feature_controls::get().bots_paused) {
        return Err("Orchestration is disabled or bots are globally paused.".into());
    }
    // A previously-enabled or externally corrupted multi-role roster cannot
    // silently fall back to the primary Worker, regardless of UI preflight.
    if roster::has_additional_members(store, config_id)? {
        return Err("Multi-member Team requires native scoped role admission; manual dispatch denied.".into());
    }
    let coordinator = bots::get(store, &config.coordinator_bot_id)?;
    let worker = bots::get(store, &config.worker_bot_id)?;
    if coordinator.paused || worker.paused || worker.project_id != config.project_id {
        return Err("A selected bot is paused, missing or outside the worker project scope.".into());
    }
    // Preflight stored execution profiles BEFORE creating a native provider
    // session. The session layer also validates its inputs, but it cannot
    // report the Team policy blocker or authorize a Demand snapshot.
    for member in [&coordinator,&worker] {
        if let Some(issue)=roster::member_execution_issue(member) {
            return Err(issue.into());
        }
    }
    let candidate = if coordinate { &coordinator } else { &worker };
    let occupied = manual_candidate_occupied(store, &candidate.id, &config.project_id, &candidate.project_id)?;
    // Do not create a coordinated ticket without an admitted coordinator.
    // Otherwise a worker retry could bypass missing coordinator evidence.
    if coordinate && (busy || occupied) {
        return Err("Coordinator is busy; retry this explicit manual request later.".into());
    }
    let blocked = busy || occupied || headless_staging;
    let ticket_id = storage::id();
    let assignment_id = (!blocked).then(storage::id);
    // Reserve the internal conversation in the SAME SQLite transaction as the
    // ticket, mandate and worker lease. If any insert or CHECK fails, the
    // ephemeral session and bot-thread mapping must roll back as well.
    // Store methods share this connection, so use unchecked_transaction here
    // instead of borrowing it mutably while provisioning the bot thread.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let session_id = if blocked { None } else {
        Some(bots::thread_session(store, candidate, CHANNEL, &ticket_id,
            if coordinate { "SuperAgent · Coordinator" } else { "SuperAgent · Manual task" })?.0)
    };
    let now = storage::now();
    tx.execute(
        "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,project_id,worker_bot_id,policy_revision,state,active_assignment_id,created_at,updated_at)
         VALUES(?1,?2,?3,?4,'local-user','manual',?5,?6,?7,?8,?9,?10,?10)",
        params![ticket_id,config.id,request_key,objective,config.project_id,worker.id,config.policy_revision,
            if blocked { "blocked" } else { "running" }, assignment_id,now],
    ).map_err(|e| e.to_string())?;
    roster::capture_ticket_profile(store,&ticket_id,&config,now)?;
    tx.execute(
        "INSERT INTO sa_mandates(ticket_id,actor,authorization_id,policy_revision,project_id,expires_at,created_at)
         VALUES(?1,'local-user',?2,?3,?4,?5,?6)",
        params![ticket_id,storage::id(),config.policy_revision,config.project_id,now + 4 * 60 * 60 * 1000,now],
    ).map_err(|e| e.to_string())?;
    if let (Some(assignment_id),Some(session_id)) = (&assignment_id,&session_id) {
        tx.execute(
            "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES(?1,?2,?3,?4,'running',1,?5,?5)",
            params![assignment_id,ticket_id,candidate.id,session_id,now],
        ).map_err(|e| e.to_string())?;
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'admitted',?2,?3)",
        params![ticket_id, if headless_staging { "blocked: explicit headless staging" }
             else if blocked { "blocked: worker occupied" }
             else if coordinate { "manual readonly coordinator reserved" }
             else { "manual readonly worker reserved" },now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((get_ticket(store, &ticket_id)?, !blocked))
}

/// A recorded coordinator plan is not a transferable authorization. Its
/// original native session must still exist in the coordinator's current
/// project. For a direct manual ticket with no coordinator attempt, this
/// check is intentionally vacuous; worker admission remains independently
/// gated by its own native mandate.
fn coordinator_plan_current(
    store: &Store, ticket_id: &str, coordinator_id: &str, project_id: &str,
) -> Result<bool> {
    // The original native admission fixes whether a coordinator was required.
    // No coordinator on a direct ticket is valid; a coordinated ticket with a
    // deleted/relabelled finished assignment is not a valid plan.
    let original: Option<String> = store.db.query_row(
        "SELECT detail FROM sa_audit WHERE ticket_id=?1 AND kind='admitted'
         ORDER BY id LIMIT 1",
        [ticket_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    match original.as_deref() {
        Some("manual readonly coordinator reserved") => {
            let proven: bool = store.db.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sa_assignments a
                    JOIN sessions original ON original.id=a.session_id
                    WHERE a.ticket_id=?1 AND a.bot_id=?2 AND a.attempt=1
                      AND a.state='finished' AND original.project_id=?3
                      AND length(trim(COALESCE(a.result,'')))>0
                      AND EXISTS(
                        SELECT 1 FROM sa_audit proof
                        WHERE proof.ticket_id=a.ticket_id
                          AND proof.kind='coordinator_planned')
                )",
                params![ticket_id,coordinator_id,project_id], |row| row.get(0),
            ).map_err(|e| e.to_string())?;
            if !proven { return Ok(false); }
        }
        Some("manual readonly worker reserved" | "blocked: worker occupied"
            | "blocked: explicit headless staging") => {}
        _ => return Ok(false),
    }
    let invalid: bool = store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_assignments a
           LEFT JOIN sessions original ON original.id=a.session_id
           WHERE a.ticket_id=?1 AND a.bot_id=?2
             AND (a.state<>'finished' OR original.id IS NULL OR original.project_id<>?3)
         )",
        params![ticket_id,coordinator_id,project_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if invalid { return Ok(false); }
    // A provider's finished coordinator plan is still bound to the original
    // native task thread. Removing or remapping that thread after settlement
    // must not turn the old plan into an authority for a new worker attempt.
    let mut statement = store.db.prepare(
        "SELECT session_id FROM sa_assignments WHERE ticket_id=?1 AND bot_id=?2",
    ).map_err(|e| e.to_string())?;
    let session_ids = statement.query_map(params![ticket_id, coordinator_id],
        |row| row.get::<_, String>(0),
    ).map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    for session_id in session_ids {
        if !assignment_thread_current(store, &session_id, ticket_id)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn assignment_session(store: &Store, task_id: &str) -> Result<Option<String>> {
    store.db.query_row(
        "SELECT a.session_id FROM sa_assignments a JOIN sa_tickets t ON t.active_assignment_id=a.id
         WHERE t.id=?1 AND a.state='running'", [task_id],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())
}

/// Verify that a provider session still belongs to the exact native
/// ticket/attempt thread it was admitted under. Recheck this at execution
/// admission AND result settlement, not just at the gateway ingress.
fn assignment_thread_current(store: &Store, session_id: &str, ticket_id: &str) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM bot_threads thread
            JOIN sa_assignments assignment ON assignment.session_id=thread.session_id
            WHERE assignment.session_id=?1 AND assignment.bot_id=thread.bot_id
              AND assignment.ticket_id=?3 AND thread.channel_id=?2
              AND (
                (assignment.attempt=1 AND thread.external_id=?3)
                OR (assignment.attempt=1 AND thread.external_id=?3 || ':attempt:1'
                    AND EXISTS (SELECT 1 FROM sa_audit initial
                      WHERE initial.ticket_id=assignment.ticket_id
                        AND initial.kind='admitted'
                        AND initial.detail IN ('blocked: worker occupied',
                           'blocked: explicit headless staging'))
                    AND EXISTS (SELECT 1 FROM sa_audit retry
                      WHERE retry.ticket_id=assignment.ticket_id
                        AND retry.kind='manual_retry'))
                OR (assignment.attempt>1 AND
                    thread.external_id=?3 || ':attempt:' || assignment.attempt)
                OR (assignment.attempt>1 AND
                    thread.external_id=?3 || ':worker:' || assignment.attempt)
              )
        )",
        params![session_id, CHANNEL, ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())
}

/// A registered bot is not automatically authorized for every assignment
/// phase. The original user-issued admission fixes whether attempt one is a
/// coordinator plan or a direct worker. Later attempts can only be workers.
fn assignment_role_authorized(
    store: &Store, ticket_id: &str, bot_id: &str, attempt: i64,
    coordinator_id: &str, worker_id: &str,
) -> Result<bool> {
    let admission: Option<String> = store.db.query_row(
        "SELECT detail FROM sa_audit WHERE ticket_id=?1 AND kind='admitted' ORDER BY id LIMIT 1",
        [ticket_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    Ok(match admission.as_deref() {
        Some("manual readonly coordinator reserved") if attempt == 1 => bot_id == coordinator_id,
        Some("manual readonly coordinator reserved" | "manual readonly worker reserved"
            | "blocked: worker occupied" | "blocked: explicit headless staging")
            if attempt > 0 => bot_id == worker_id,
        _ => false,
    })
}

/// Fence the actual provider session to its admission-time native Team
/// profile. Mutating a session cannot elevate its model, mode or harness, while
/// changes to a reusable bot must not rewrite an already admitted session.
/// Pre-v46 legacy tasks remain read-only under their original no-tools Mandate.
fn admitted_session_matches_profile(
    store: &Store, ticket_id: &str, bot_id: &str, role: &str,
    session: &storage::Session,
) -> Result<bool> {
    let Some(profile) = roster::ticket_profile(store, ticket_id)? else {
        return Ok(true);
    };
    if !roster::checkout_paths_current(store,ticket_id)? {return Ok(false);}
    let Some(member) = profile.members.iter()
        .find(|member| member.role == role && member.bot_id == bot_id)
    else {
        return Ok(false);
    };
    Ok(session.project_id == member.project_id
        && session.provider == member.provider
        && session.model == member.model
        && session.mode == member.mode
        && session.reasoning_effort.as_deref().unwrap_or("") == member.reasoning_effort)
}

/// Only the active, user-admitted read-only assignment is allowed to run.
/// Leverages the existing provider-level Council isolation (native plan,
/// tool-host removal, external MCP checks and permission denial), not a prompt.
pub(crate) fn read_only_policy(
    store: &Store, session_id: &str,
) -> Result<Option<crate::architect::reviewer_security::ReviewerToolPolicy>> {
    struct AssignmentPolicyScope {
        ticket_state: String,
        assignment_state: String,
        expiry: i64,
        revoked: bool,
        mandate_version: i64,
        ticket_version: i64,
        config_version: i64,
        configuration_project: String,
        enabled: bool,
        bot_id: String,
        coordinator_id: String,
        worker_id: String,
        project_id: String,
        ticket_id: String,
        started_at: i64,
        side_effects: bool,
        tools_json: String,
        mandate_project: String,
        mandate_actor: String,
        ticket_actor: String,
        authorization_id: String,
        attempt: i64,
    }
    let scope: Option<AssignmentPolicyScope> = store.db.query_row(
        "SELECT t.state,a.state,m.expires_at,m.revoked,m.policy_revision,o.policy_revision,o.enabled,
                a.bot_id,o.coordinator_bot_id,t.worker_bot_id,t.project_id,t.id,a.created_at,
                m.allowed_side_effects,m.allowed_tools,m.project_id,m.actor,t.actor,a.attempt,
                t.policy_revision,o.project_id,m.authorization_id
         FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE a.session_id=?1 AND t.active_assignment_id=a.id",
        [session_id],
        |r| Ok(AssignmentPolicyScope {
            ticket_state: r.get(0)?, assignment_state: r.get(1)?,
            expiry: r.get(2)?, revoked: r.get::<_, i64>(3)? != 0,
            mandate_version: r.get(4)?, config_version: r.get(5)?,
            enabled: r.get::<_, i64>(6)? != 0, bot_id: r.get(7)?,
            coordinator_id: r.get(8)?, worker_id: r.get(9)?, project_id: r.get(10)?,
            ticket_id: r.get(11)?, started_at: r.get(12)?,
            side_effects: r.get::<_, i64>(13)? != 0, tools_json: r.get(14)?,
            mandate_project: r.get(15)?, mandate_actor: r.get(16)?, ticket_actor: r.get(17)?,
            attempt: r.get(18)?, ticket_version: r.get(19)?,
            configuration_project: r.get(20)?, authorization_id: r.get(21)?,
        }),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(scope) = scope {
        if scope.ticket_state != "running" || scope.assignment_state != "running" || scope.revoked
            || scope.expiry <= storage::now()
            || scope.mandate_version != scope.config_version
            || scope.ticket_version != scope.config_version
            || scope.configuration_project != scope.project_id
            || !assignment_within_budget(scope.started_at, storage::now())
            // The current task channel has only native read-only enforcement.
            // A persisted write/tool grant must not be silently treated as a
            // runnable permission (or as harmless metadata) until a separate
            // user-issued, provider-enforced write adapter is implemented.
            || scope.side_effects || scope.tools_json != "[]"
            || scope.mandate_project != scope.project_id
            || scope.mandate_actor != scope.ticket_actor
            || scope.authorization_id.trim().is_empty()
            || !execution_permitted(scope.enabled, crate::feature_controls::get().bots_paused)
        {
            return Err("policy_unsupported: SuperAgent mandate is inactive or stale.".into());
        }
        // A native assignment never inherits ambient bot permissions if its
        // internal thread has been unlinked or remapped after admission.
        // This guards direct session launch as well as the inbound gateway.
        let native_thread = assignment_thread_current(store, session_id, &scope.ticket_id)?;
        if !native_thread {
            return Err("policy_unsupported: SuperAgent session no longer matches its native thread.".into());
        }
        // A valid native thread must also have the role originally authorized
        // by the Ticket. Remapping both the assignment and the thread to a
        // third existing bot cannot grant that bot access to this project.
        if !assignment_role_authorized(store, &scope.ticket_id, &scope.bot_id,
            scope.attempt, &scope.coordinator_id, &scope.worker_id)? {
            return Err("policy_unsupported: assignment bot does not match the original task role.".into());
        }
        let active_bot = bots::get(store, &scope.bot_id)?;
        let coordinator = bots::get(store, &scope.coordinator_id)?;
        let assigned_session = store.session(session_id)?;
        let assigned_role = if scope.bot_id == scope.coordinator_id {
            "coordinator"
        } else {
            "worker"
        };
        if !admitted_session_matches_profile(
            store, &scope.ticket_id, &scope.bot_id, assigned_role, &assigned_session,
        )? {
            return Err("policy_unsupported: native SuperAgent session profile differs from admitted Demand.".into());
        }
        if coordinator.paused || active_bot.paused || active_bot.project_id != assigned_session.project_id
            || (scope.bot_id == scope.worker_id && active_bot.project_id != scope.project_id)
            || (scope.bot_id == scope.worker_id && !coordinator_plan_current(
                store, &scope.ticket_id, &scope.coordinator_id, &coordinator.project_id,
            )?)
        {
            return Err("policy_unsupported: assigned bot is paused or moved outside its mandate.".into());
        }
        return Ok(Some(crate::architect::reviewer_security::ReviewerToolPolicy::NativeReadOnly));
    }
    // A settled or detached assignment still owns this provider session.
    // Never interpret a missing bot_threads row as permission to fall back
    // to a normal, potentially writable bot session.
    let native_assignment: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE session_id=?1)",
        [session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if native_assignment {
        return Err("policy_unsupported: SuperAgent session has no active native authority.".into());
    }
    let internal: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM bot_threads WHERE session_id=?1 AND channel_id=?2)",
        params![session_id,CHANNEL], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if internal {
        return Err("policy_unsupported: unassigned SuperAgent conversation.".into());
    }
    Ok(None)
}

/// Cloud reattachment skips the normal native provider policy preparation.
/// Even a currently valid read-only assignment must fail closed here until a
/// cloud adapter can enforce its mandate at every provider/tool admission.
/// A settled, revoked or detached task also fails; ordinary cloud sessions
/// retain the original attach behavior.
pub(crate) fn guard_cloud_attach(store: &Store, session_id: &str) -> Result<()> {
    if read_only_policy(store, session_id)?.is_some() {
        return Err("policy_unsupported: SuperAgent task cannot attach through an unrestricted cloud provider.".into());
    }
    Ok(())
}

/// Require a pre-existing, active native assignment before a message may enter
/// the internal SuperAgent gateway channel. This check runs before the gateway
/// creates a bot thread or parses commands, so an external sender or a stale
/// callback cannot create an orphan conversation or trigger bot-wide commands.
/// Normal paired channels remain on their existing authentication path.
pub(crate) fn guard_gateway_inbound(
    store: &Store, bot_id: &str, external_id: &str, sender_id: &str, trusted: bool,
) -> Result<()> {
    if !trusted || sender_id != "local-user" {
        return Err("policy_unsupported: internal SuperAgent delivery requires local authority.".into());
    }
    let session_id: Option<String> = store.db.query_row(
        "SELECT thread.session_id
         FROM bot_threads thread
         JOIN sa_assignments assignment ON assignment.session_id=thread.session_id
         JOIN sa_tickets ticket ON ticket.id=assignment.ticket_id
         WHERE thread.bot_id=?1 AND thread.channel_id=?2 AND thread.external_id=?3
           AND assignment.bot_id=?1 AND assignment.state='running'
           AND ticket.state='running' AND ticket.active_assignment_id=assignment.id
           AND (
             (assignment.attempt=1 AND thread.external_id=ticket.id)
              OR (assignment.attempt=1 AND thread.external_id=ticket.id || ':attempt:1'
                  AND EXISTS (SELECT 1 FROM sa_audit initial
                    WHERE initial.ticket_id=assignment.ticket_id
                      AND initial.kind='admitted'
                      AND initial.detail IN ('blocked: worker occupied',
                           'blocked: explicit headless staging'))
                  AND EXISTS (SELECT 1 FROM sa_audit retry
                    WHERE retry.ticket_id=assignment.ticket_id
                      AND retry.kind='manual_retry'))
             OR (assignment.attempt>1 AND
                 thread.external_id=ticket.id || ':attempt:' || assignment.attempt)
             OR (assignment.attempt>1 AND
                 thread.external_id=ticket.id || ':worker:' || assignment.attempt)
           )",
        params![bot_id, CHANNEL, external_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let session_id = session_id.ok_or(
        "policy_unsupported: no active native assignment owns this SuperAgent thread.",
    )?;
    if read_only_policy(store, &session_id)?.is_none() {
        return Err("policy_unsupported: internal SuperAgent assignment has no native policy.".into());
    }
    Ok(())
}

/// Called while the shared running-agent mutex and then the Store mutex
/// are held, immediately before native run registration. This is the second
/// admission fence after a durable SuperAgent lease was reserved: a normal bot
/// or direct task cannot start in the same project while that lease is active,
/// and a SuperAgent cannot race a previously admitted ordinary session.
pub(crate) fn guard_run_admission(
    store: &Store, session_id: &str, running_sessions: &[String],
) -> Result<()> {
    let session = store.session(session_id)?;
    let superagent_run = read_only_policy(store, session_id)?.is_some();
    // A bot lease protects its native identity across all provider sessions,
    // even when an older conversation still points at a different project.
    let session_bot_id = bots::owner_of(store, session_id)?.map(|(bot, _)| bot.id);
    let leased_by_other: bool = store.db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sa_assignments a
            JOIN sa_tickets t ON t.id=a.ticket_id AND t.active_assignment_id=a.id
            LEFT JOIN sessions assigned ON assigned.id=a.session_id
            WHERE a.state='running' AND t.state='running'
              AND (t.project_id=?1 OR assigned.project_id=?1 OR assigned.id IS NULL
                    OR (?3 IS NOT NULL AND a.bot_id=?3)
                    OR EXISTS(SELECT 1 FROM projects old_checkout,projects new_checkout
                              WHERE old_checkout.id IN (t.project_id,assigned.project_id)
                                AND new_checkout.id=?1
                                AND old_checkout.path=new_checkout.path))
              AND a.session_id<>?2
         )",
        params![session.project_id, session_id, session_bot_id.as_deref()],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if leased_by_other {
        return Err("Project is reserved by another native SuperAgent assignment.".into());
    }
    // Keep ordinary chats unchanged when there is no competing native lease.
    // With a lease, compare physical checkouts as well as persisted path text.
    let mut statement=store.db.prepare(
        "SELECT t.project_id, assigned.project_id
         FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id AND t.active_assignment_id=a.id
         LEFT JOIN sessions assigned ON assigned.id=a.session_id
         WHERE a.state='running' AND t.state='running' AND a.session_id<>?1",
    ).map_err(|e|e.to_string())?;
    let leased=statement.query_map([session_id],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,
    ))).map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    if !leased.is_empty() {
        let session_path=checkout_path(store,&session.project_id)?;
        for (ticket_project,assigned_project) in leased {
            for id in std::iter::once(ticket_project.as_str()).chain(assigned_project.as_deref()) {
                if checkout_paths_overlap(&session_path,&checkout_path(store,id)?) {
                    return Err("Physical checkout is reserved by another native SuperAgent assignment.".into());
                }
            }
        }
    }
    // A stopped or revoked Ticket releases its durable lease immediately,
    // but the underlying provider may still be running until cancellation
    // finishes. Keep ordinary sessions off the same checkout or bot until
    // the native session actually leaves the live agent registry.
    if !superagent_run && !running_sessions.is_empty() {
        let session_path=checkout_path(store,&session.project_id)?;
        for other_id in running_sessions {
            if other_id == session_id { continue; }
            let task_owned: bool = store.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE session_id=?1)",
                [other_id], |row| row.get(0),
            ).map_err(|e| e.to_string())?;
            if !task_owned { continue; }
            let native_session=store.session(other_id)?;
            let same_bot = if let Some(bot_id)=session_bot_id.as_ref() {
                bots::owner_of(store,other_id)?
                    .is_some_and(|(owner,_)| owner.id.as_str()==bot_id.as_str())
            } else { false };
            if same_bot || checkout_paths_overlap(
                &session_path,&checkout_path(store,&native_session.project_id)?,
            ) {
                return Err("Native SuperAgent provider is still running on this bot or checkout.".into());
            }
        }
    }
    if superagent_run {
        let session_path=checkout_path(store,&session.project_id)?;
        for other_id in running_sessions {
            if other_id == session_id { continue; }
            // A disappeared active session is not proof that a lease is free.
            let other = store.session(other_id)?;
            let other_bot_id = bots::owner_of(store, other_id)?.map(|(bot, _)| bot.id);
            if (session_bot_id.is_some() && session_bot_id == other_bot_id)
                || other.project_id == session.project_id
                || checkout_paths_overlap(
                    &checkout_path(store,&other.project_id)?, &session_path,
                ) {
                return Err("Project has another active agent; SuperAgent dispatch denied.".into());
            }
        }
    }
    Ok(())
}

fn fail_launch(store: &Store, ticket_id: &str, reason: &str) -> Result<()> {
    // A failed gateway launch must release the worker lease, change the ticket
    // and retain its audit receipt atomically. An audit/storage failure cannot
    // leave a partially failed assignment or silently permit a second worker.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let now = storage::now();
    let detail = reason.chars().take(500).collect::<String>();
    let assignment = tx.execute(
        "UPDATE sa_assignments SET state='failed',updated_at=?2,result=?3
         WHERE id=(SELECT active_assignment_id FROM sa_tickets
                   WHERE id=?1 AND state='running') AND state='running'",
        params![ticket_id, now, &detail],
    ).map_err(|e| e.to_string())?;
    if assignment != 1 {
        return Err("SuperAgent launch failure no longer owns an active assignment.".into());
    }
    let ticket = tx.execute(
        "UPDATE sa_tickets SET state='blocked',active_assignment_id=NULL,
         revision=revision+1,updated_at=?2
         WHERE id=?1 AND state='running' AND active_assignment_id IS NOT NULL",
        params![ticket_id, now],
    ).map_err(|e| e.to_string())?;
    if ticket != 1 {
        return Err("SuperAgent ticket changed before launch failure settlement.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'launch_failed',?2,?3)",
        params![ticket_id, &detail, now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// A provider turn is evidence for a task, not its approval. The only successful
/// transition here is running -> waiting_approval; native human acceptance is separate.
pub(crate) fn on_run_finished(store: &Store, session_id: &str, successful: bool, text: &str) -> Result<bool> {
    // A callback may arrive twice, after Stop, or after process restart.
    // Result, Ticket revision and audit must settle in one transaction; an
    // injected storage failure cannot strand an assignment in a terminal state.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let assignment: Option<(String, String, String, bool, i64, i64)> = tx.query_row(
        "SELECT a.ticket_id,a.id,a.bot_id,a.bot_id=o.coordinator_bot_id,a.created_at,a.attempt
         FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id AND t.active_assignment_id=a.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE a.session_id=?1 AND a.state='running' AND t.state='running'",
        [session_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((task_id, assignment_id, bot_id, coordinator, started_at, attempt)) = assignment else { return Ok(false) };
    let cfg = configuration(store, &get_ticket(store, &task_id)?.orchestration_id)?;
    let role_current = assignment_role_authorized(
        store, &task_id, &bot_id, attempt, &cfg.coordinator_bot_id, &cfg.worker_bot_id,
    )?;
    let authorized: bool = tx.query_row(
        "SELECT m.revoked=0 AND m.expires_at>?2 AND m.policy_revision=o.policy_revision AND o.enabled=1
         AND m.allowed_side_effects=0 AND m.allowed_tools='[]'
         AND m.project_id=t.project_id AND m.actor=t.actor
         AND trim(m.authorization_id)<>''
         AND t.policy_revision=o.policy_revision AND o.project_id=t.project_id
         AND EXISTS (
           SELECT 1 FROM bots coordinator
           WHERE coordinator.id=o.coordinator_bot_id AND coordinator.paused=0
         )
         AND EXISTS (
           SELECT 1 FROM sa_assignments active
           JOIN bots current_bot ON current_bot.id=active.bot_id
           JOIN sessions active_session ON active_session.id=active.session_id
           WHERE active.id=t.active_assignment_id AND current_bot.paused=0
             AND current_bot.project_id=active_session.project_id
             AND (active.bot_id=o.coordinator_bot_id OR
               (active.bot_id=t.worker_bot_id AND current_bot.project_id=t.project_id))
         )
         FROM sa_mandates m JOIN sa_tickets t ON t.id=m.ticket_id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id WHERE t.id=?1",
        params![task_id,storage::now()], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    let provenance_ok = if coordinator { true } else {
        bots::get(store, &cfg.coordinator_bot_id)
            .and_then(|bot| coordinator_plan_current(store, &task_id, &bot.id, &bot.project_id))
            .unwrap_or(false)
    };
    let trimmed = text.trim().chars().take(4096).collect::<String>();
    // A stop-all transition may happen after admission but before the provider
    // callback. A stale success must not become approvable after global pause.
    let within_budget = assignment_within_budget(started_at, storage::now());
    let thread_current = assignment_thread_current(store, session_id, &task_id)?;
    // A provider callback cannot authorize a changed runtime profile, even
    // when the original bot and native Ticket still exist.
    let session_profile_current = store.session(session_id).ok()
        .and_then(|session| admitted_session_matches_profile(
            store, &task_id, &bot_id,
            if coordinator { "coordinator" } else { "worker" }, &session,
        ).ok())
        .unwrap_or(false);
    let authorized = execution_permitted(
        authorized && role_current && provenance_ok && within_budget && thread_current
            && session_profile_current,
        crate::feature_controls::get().bots_paused,
    );
    let next = if !authorized { "blocked" }
        else if !successful { "failed" }
        else if trimmed.is_empty() { "inconclusive" }
        else if coordinator { "queued" }
        else { "waiting_approval" };
    let now = storage::now();
    // A stale, revoked or remapped task may transition to blocked, but its
    // provider output must not survive as trusted worker evidence or an
    // assignment result consumed by later review/recovery paths.
    let trusted_result = authorized.then_some(trimmed.as_str());
    let moved = tx.execute(
        "UPDATE sa_assignments SET state=?2,result=?3,updated_at=?4
         WHERE id=?1 AND state='running'",
        // Rejected callbacks are not successful coordinator plans, even if
        // the provider returned text. Otherwise a later manual retry after
        // pause/revocation could promote that unauthorized plan to authority.
        params![assignment_id,if authorized && successful { "finished" } else { "failed" },trusted_result,now],
    ).map_err(|e| e.to_string())?;
    if moved != 1 { return Err("SuperAgent assignment changed before settlement.".into()); }
    let updated = tx.execute(
        "UPDATE sa_tickets SET state=?2,evidence=?3,active_assignment_id=NULL,
         revision=revision+1,updated_at=?4
         WHERE id=?1 AND state='running' AND active_assignment_id=?5",
        params![task_id,next,trusted_result,now,assignment_id],
    ).map_err(|e| e.to_string())?;
    if updated != 1 { return Err("SuperAgent ticket changed before settlement.".into()); }
    // Atomically pin the native Council cycle and exact finished artifact,
    // when this worker session already has a Council team. This is advisory:
    // an unrelated or previously completed Council round never approves it.
    if next == "waiting_approval" {
        council_adapter::bind_finished(store, &task_id, &assignment_id, session_id)?;
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,?2,?3,?4)",
        params![task_id,if !within_budget { "assignment_timed_out" }
            else if !authorized { "callback_rejected" }
            else if coordinator { "coordinator_planned" } else { "turn_settled" },next,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

fn accept(store: &Store, ticket_id: &str, expected_revision: i64) -> Result<Ticket> {
    accept_with_pause(store, ticket_id, expected_revision, crate::feature_controls::get().bots_paused)
}

fn accept_with_pause(
    store: &Store, ticket_id: &str, expected_revision: i64, bots_paused: bool,
) -> Result<Ticket> {
    // Native approval is still a state-changing orchestration action. A
    // globally paused bot fleet cannot complete a waiting task merely because
    // its worker had already produced evidence before the pause.
    if bots_paused {
        return Err("Bots are globally paused; SuperAgent acceptance is denied.".into());
    }
    // Acceptance and its attributed audit entry are one durable decision.
    // No terminal state may be visible without matching acceptance evidence.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let ticket = get_ticket(store,ticket_id)?;
    if ticket.state != "waiting_approval" || ticket.revision != expected_revision
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
    {
        return Err("Task has no current verified worker result to accept.".into());
    }
    // A provider timeout or restart can leave a remote change uncertain
    // even after the worker produced text evidence. Human acceptance must
    // never turn that uncertainty into a completed Ticket or final reply.
    // Check inside the same SQLite transaction as the terminal transition.
    let unresolved: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_effect_receipts
         WHERE ticket_id=?1 AND state IN ('pending','uncertain'))",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if unresolved {
        return Err("SuperAgent has unresolved external effects; reconcile authoritative provider state before accepting.".into());
    }
    let coordinator = configuration(store, &ticket.orchestration_id)?;
    let coordinator = bots::get(store, &coordinator.coordinator_bot_id)?;
    if !coordinator_plan_current(store, ticket_id, &coordinator.id, &coordinator.project_id)? {
        return Err("Coordinator plan is stale or its original session is unavailable.".into());
    }
    // The worker's completed assignment is evidence only while its original
    // native session and project are still attributable. A bot move, session
    // deletion, or replacement of stored result text must not turn an old
    // model reply into a new, human-approved task completion.
    let worker_evidence_session: Option<String> = tx.query_row(
        "SELECT assignment.session_id FROM sa_assignments assignment
         JOIN sessions original ON original.id=assignment.session_id
         WHERE assignment.ticket_id=?1 AND assignment.bot_id=?2
           AND assignment.state='finished'
           AND assignment.attempt=(
               SELECT MAX(newest.attempt) FROM sa_assignments newest
               WHERE newest.ticket_id=?1
           )
           AND original.project_id=?3 AND assignment.result=?4",
        params![ticket_id,ticket_worker(store, ticket_id)?,mandate(store, ticket_id)?.project_id,
            ticket.evidence.as_deref().unwrap_or_default()],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    // The worker's result cannot be accepted after its task thread was
    // detached or remapped, even if the provider session and text still exist.
    let worker_evidence_current = worker_evidence_session.as_deref()
        .map(|session_id| assignment_thread_current(store, session_id, ticket_id))
        .transpose()?.unwrap_or(false);
    if !worker_evidence_current {
        return Err("Original worker evidence session is missing or outside its mandate.".into());
    }
    // A human pressing the legacy Council-Off approval button is NOT an
    // attributed Council exception. If a worker has any native Council
    // requirement or previously frozen Council target, do not finalize on
    // text evidence, a cached passing status, or a missing/removed team.
    // The #126 task-bound request and verified exception workflow must
    // explicitly implement their own authorization and terminal gate.
    if council_adapter::requires_task_review(
        store, ticket_id, worker_evidence_session.as_deref().unwrap_or_default(),
    )? {
        return Err("Native Council review is required; task-bound verified verdict and attributed exception are not available.".into());
    }
    let now = storage::now();
    let changed = tx.execute(
        "UPDATE sa_tickets SET state='completed',revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2 AND state='waiting_approval'
         AND EXISTS (
            SELECT 1 FROM sa_mandates m JOIN sa_orchestrations o
            ON o.id=sa_tickets.orchestration_id
            WHERE m.ticket_id=sa_tickets.id AND m.revoked=0 AND m.expires_at>?3
              AND m.allowed_side_effects=0 AND m.allowed_tools='[]'
              AND m.project_id=sa_tickets.project_id AND m.actor=sa_tickets.actor
              AND trim(m.authorization_id)<>''
              AND m.policy_revision=o.policy_revision AND o.enabled=1
              AND EXISTS (
                SELECT 1 FROM bots worker JOIN bots coordinator
                  ON coordinator.id=o.coordinator_bot_id
                WHERE worker.id=sa_tickets.worker_bot_id
                  AND worker.paused=0 AND coordinator.paused=0
                  AND worker.project_id=sa_tickets.project_id
              )
         )",
        params![ticket_id,expected_revision,now],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Mandate expired, was revoked or revised; acceptance denied.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'human_accepted','local-user explicitly accepted worker evidence',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    get_ticket(store,ticket_id)
}

/// A negative human decision is not Council approval, completion, or a
/// new grant. Preserve the rejected result in assignment history; only a
/// separately authorized, bounded manual retry can produce fresh evidence.
fn reject(store: &Store, ticket_id: &str, expected_revision: i64, reason: &str) -> Result<Ticket> {
    let reason = reason.trim();
    if reason.is_empty() || reason.len() > 500 || reason.chars().any(char::is_control) {
        return Err("Provide a nonempty rejection reason of at most 500 bytes.".into());
    }
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let now = storage::now();
    let changed = tx.execute(
        "UPDATE sa_tickets SET state='blocked',revision=revision+1,updated_at=?3
         WHERE id=?1 AND state='waiting_approval' AND revision=?2",
        params![ticket_id,expected_revision,now],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Task is not awaiting a decision at the expected revision.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'human_rejected',?2,?3)",
        params![ticket_id,reason,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    get_ticket(store,ticket_id)
}

fn stop(store: &Store, ticket_id: &str) -> Result<(Ticket,Option<String>)> {
    stop_with_expected_revision(store, ticket_id, None)
}

fn stop_with_expected_revision(
    store: &Store, ticket_id: &str, expected_revision: Option<i64>,
) -> Result<(Ticket,Option<String>)> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let ticket = get_ticket(store,ticket_id)?;
    if expected_revision.is_some_and(|revision| revision != ticket.revision) {
        return Err("SuperAgent Ticket revision changed before Stop; inspect the current Ticket.".into());
    }
    if matches!(ticket.state.as_str(),"completed"|"failed"|"stopped"|"interrupted"|"inconclusive") {
        return Err("Task is already terminal.".into());
    }
    let session = assignment_session(store,ticket_id)?;
    let now = storage::now();
    let revoked = tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[ticket_id])
        .map_err(|e| e.to_string())?;
    if revoked != 1 { return Err("SuperAgent mandate missing; Stop aborted.".into()); }
    let updated = tx.execute(
        "UPDATE sa_tickets SET state='stopped',active_assignment_id=NULL,revision=revision+1,updated_at=?2
         WHERE id=?1 AND revision=?3",
        params![ticket_id,now,ticket.revision],
    ).map_err(|e| e.to_string())?;
    if updated != 1 { return Err("SuperAgent ticket changed before Stop.".into()); }
    // Stop every still-running task-owned attempt, including a detached
    // legacy/orphan assignment. The live host observes these stopped rows and
    // cancels the corresponding provider sessions; no callback may revive them.
    tx.execute(
        "UPDATE sa_assignments SET state='stopped',updated_at=?2
         WHERE ticket_id=?1 AND state='running'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    // Revoking a Ticket cannot prove an in-flight provider effect failed.
    // Preserve its identity and require explicit remote reconciliation.
    tx.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
         WHERE ticket_id=?1 AND state='pending'",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'stopped','local-user revoked mandate',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((get_ticket(store,ticket_id)?,session))
}

/// A native bot reset/delete invalidates in-flight SuperAgent ownership, but
/// its Ticket/Mandate/Assignment evidence remains independent of bot history.
/// Called inside the bot reset's existing transaction; never makes a new runner.
pub(crate) fn invalidate_bot(store: &Store, bot_id: &str, removed: bool) -> Result<()> {
    // Existing Team-role references use ON DELETE RESTRICT. Reject a Bot
    // deletion *before* revoking Ticket mandates or clearing Bot sessions;
    // otherwise the eventual FK failure leaves a live Bot with lost work.
    // A reset is different: its identity remains and its task grants are revoked.
    if removed {
        let referenced: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_team_roles WHERE bot_id=?1)",
            [bot_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if referenced {
            return Err("Remove this bot from disabled SuperAgent Team roles before deleting it.".into());
        }
    }
    // Resetting either selected bot invalidates every nonterminal mandate:
    // coordinator sessions are just as ephemeral as worker sessions, and a
    // previously finished plan must not authorize a worker after reset.
    let sql =
        "SELECT t.id,t.state FROM sa_tickets t JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE (t.worker_bot_id=?1 OR o.coordinator_bot_id=?1
            OR EXISTS (SELECT 1 FROM sa_team_roles r
                       WHERE r.orchestration_id=t.orchestration_id AND r.bot_id=?1)
            OR EXISTS (SELECT 1 FROM sa_ticket_profiles p, json_each(p.profiles_json) m
                       WHERE p.ticket_id=t.id AND json_extract(m.value,'$.botId')=?1))
         AND t.state IN ('queued','running','waiting_approval','waiting_review','blocked')";
    let affected = {
        let mut stmt = store.db.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([bot_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?
    };
    let now = storage::now();
    for (ticket_id, previous) in affected {
        let next = if previous == "running" { "interrupted" } else { "blocked" };
        store.db.execute(
            "UPDATE sa_tickets SET state=?2,revision=revision+1,
             active_assignment_id=NULL,updated_at=?3 WHERE id=?1",
            params![ticket_id,next,now],
        ).map_err(|e| e.to_string())?;
        store.db.execute(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
            [&ticket_id],
        ).map_err(|e| e.to_string())?;
        store.db.execute(
            "UPDATE sa_assignments SET state='interrupted',updated_at=?2
             WHERE ticket_id=?1 AND state='running'",
            params![ticket_id,now],
        ).map_err(|e| e.to_string())?;
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
             WHERE ticket_id=?1 AND state='pending'",
            params![ticket_id,now],
        ).map_err(|e| e.to_string())?;
        record(store,&ticket_id, if removed { "bot_removed" } else { "bot_reset" },
            "Native bot session unavailable; mandate revoked without replay")?;
    }
    if removed {
        // A deleted local bot identity must not leave a runnable preset. The
        // original names/IDs remain in history for audit, never cascaded away.
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=0,policy_revision=policy_revision+1,
             updated_at=?2 WHERE (coordinator_bot_id=?1 OR worker_bot_id=?1
               OR id IN (SELECT orchestration_id FROM sa_team_roles WHERE bot_id=?1)) AND enabled=1",
            params![bot_id,now],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}


#[tauri::command]
pub async fn superagent_create(
    window: tauri::WebviewWindow, state: State<'_,AppState>,
    input: NewOrchestration, roster: Option<roster::RosterInput>,
    max_worker_attempts: Option<i64>, purpose: Option<String>, strategy: Option<String>,
) -> Result<Orchestration> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    if purpose.is_some() || strategy.is_some() {
        return roster::create_with_attempt_policy_and_strategy(
            &store,input,roster,max_worker_attempts.unwrap_or(2),
            purpose.as_deref().unwrap_or(""),
            strategy.as_deref().unwrap_or("balanced"),
        );
    }
    match max_worker_attempts {
        Some(limit) => roster::create_with_attempt_policy(&store,input,roster,limit),
        None => match roster {
            Some(members) => roster::create_with_roster(&store,input,members),
            None => create_configuration(&store,input),
        },
    }
}

/// A single native, revision-checked Team duplication boundary. Do not
/// reconstruct a roster from separate renderer reads: it can change between
/// inspection and creation, and an enabled source never grants the new Team
/// any execution authority.
#[tauri::command]
pub async fn superagent_duplicate(
    window: tauri::WebviewWindow, state: State<'_,AppState>,
    source_id: String, expected_revision: i64, name: String,
) -> Result<Orchestration> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    roster::duplicate_with_roster(&store,&source_id,expected_revision,name)
}

#[tauri::command]
pub async fn superagent_list(window: tauri::WebviewWindow, state: State<'_,AppState>) -> Result<Vec<OrchestrationOverview>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    orchestration_overview(&store)
}

#[tauri::command]
pub async fn superagent_set_enabled(window: tauri::WebviewWindow,
    app: tauri::AppHandle, state: State<'_,AppState>, orchestration_id: String,
    expected_revision: i64, enabled: bool,
) -> Result<Orchestration> {
    require_main_window(&window)?;
    let (updated, sessions) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        // Capture only active attempts owned by this orchestration. Authority is
        // revoked transactionally before any asynchronous provider cancellation.
        let sessions = if enabled { Vec::new() } else {
            let mut stmt = store.db.prepare(
                "SELECT a.session_id FROM sa_assignments a
                 JOIN sa_tickets t ON t.id=a.ticket_id
                 WHERE t.orchestration_id=?1
                   AND t.state IN ('queued','running','waiting_approval','waiting_review','blocked')
                   AND a.state='running'"
            ).map_err(|e| e.to_string())?;
            let sessions = stmt.query_map([&orchestration_id], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            sessions
        };
        (change_enabled(&store, &orchestration_id, expected_revision, enabled)?, sessions)
    };
    for session in sessions {
        // Even if the provider cannot be stopped, the native mandate and
        // assignment have already been revoked. Never restore authority on
        // re-enable, and do not cancel sessions belonging to other presets.
        if let Err(error) = crate::stop_session(
            app.clone(), app.state::<AppState>(), session,
        ).await {
            eprintln!("SuperAgent disabled-session cancellation failed: {error}");
        }
    }
    Ok(updated)
}

/// A policy edit must not silently overwrite an authorization change from
/// another window. Even enabling is Off-mode only; it does not infer work.
/// This is an explanation of current local eligibility, never a grant or
/// reservation. The same native checks are repeated when enabling with CAS.
fn manual_enable_blocker(store: &Store, current: &Orchestration) -> Result<Option<String>> {
    if roster::has_additional_members(store, &current.id)? {
        return Ok(Some(
            "Configured multi-member Teams cannot run without native scoped admission.".into(),
        ));
    }
    // Historic/disabled Teams must remain visible even after their bots are
    // removed. Do not let a missing identity break the entire Team directory.
    let coordinator = match bots::get(store, &current.coordinator_bot_id) {
        Ok(bot) => bot,
        Err(_) => return Ok(Some("Configured Coordinator bot is unavailable.".into())),
    };
    let worker = match bots::get(store, &current.worker_bot_id) {
        Ok(bot) => bot,
        Err(_) => return Ok(Some("Configured Worker bot is unavailable.".into())),
    };
    if coordinator.paused || worker.paused
        || worker.project_id != current.project_id
        || crate::feature_controls::get().bots_paused
    {
        return Ok(Some("Selected bots are paused or their worker scope has changed.".into()));
    }
    for bot in [&coordinator, &worker] {
        if let Some(issue) = roster::member_execution_issue(bot) {
            return Ok(Some(issue.into()));
        }
    }
    Ok(None)
}

/// Display-only native state; writes retain independent revision and SQL guards.
fn team_policy_edit_blocker(store: &Store, current: &Orchestration) -> Result<Option<String>> {
    if current.enabled {
        return Ok(Some("Disable the Team before changing its policy.".into()));
    }
    let unsettled: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
          AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
            OR EXISTS(SELECT 1 FROM sa_assignments a WHERE a.ticket_id=t.id AND a.state='running')))",
        [&current.id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if unsettled {
        return Ok(Some("Active or reviewable Demands prevent Team policy edits.".into()));
    }
    let unresolved: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_effect_receipts e JOIN sa_tickets t ON t.id=e.ticket_id
          WHERE t.orchestration_id=?1 AND e.state IN ('pending','uncertain'))",
        [&current.id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if unresolved {
        return Ok(Some("Unresolved external effects prevent Team policy edits.".into()));
    }
    Ok(None)
}

fn change_enabled(
    store: &Store, orchestration_id: &str,
    expected_revision: i64, enabled: bool,
) -> Result<Orchestration> {
    let current=configuration(store,orchestration_id)?;
    if current.policy_revision != expected_revision {
        return Err("Orchestration policy revision changed; reload before editing.".into());
    }
    if enabled {
        // This is the authoritative mutation boundary. A prior directory
        // projection may have become stale and can never authorize enabling.
        if let Some(reason) = manual_enable_blocker(store, &current)? {
            return Err(reason);
        }
    }
    if current.enabled == enabled {
        if enabled { return Ok(current); }
        // A previous interrupted disable or legacy state may leave running
        // assignments under a disabled configuration. Repeating Off must
        // revoke those grants rather than returning without settling them.
        let unsettled: bool = store.db.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sa_tickets t
                JOIN sa_mandates m ON m.ticket_id=t.id
                WHERE t.orchestration_id=?1
                  AND t.state IN ('queued','running','waiting_approval','waiting_review','blocked')
                  AND (m.revoked=0 OR t.state<>'blocked'
                    OR t.active_assignment_id IS NOT NULL
                    OR EXISTS(SELECT 1 FROM sa_assignments a
                              WHERE a.ticket_id=t.id AND a.state='running'))
             )",
            [orchestration_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        // Legacy or interrupted state writers can leave a pending remote
        // effect on an already terminal Ticket. A repeated Off must quarantine
        // that effect too, even when no active Mandate remains to revoke.
        let pending_effects: bool = store.db.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sa_effect_receipts e
                 JOIN sa_tickets t ON t.id=e.ticket_id
                 WHERE t.orchestration_id=?1 AND e.state='pending'
             )",
            [orchestration_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if !unsettled && !pending_effects { return Ok(current); }
    }
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let now = storage::now();
    let updated = tx.execute(
        "UPDATE sa_orchestrations SET enabled=?2,policy_revision=policy_revision+1,
         updated_at=?3 WHERE id=?1 AND policy_revision=?4",
        params![orchestration_id,enabled,now,expected_revision],
    ).map_err(|e| e.to_string())?;
    if updated != 1 {
        return Err("Concurrent orchestration policy change; update rejected.".into());
    }
    if !enabled {
        // Off is a native revocation, not just a UI preference. Record the
        // affected tickets before changing their state, then revoke all
        // nonterminal grants and active leases in the same SQLite transaction.
        const PENDING: &str = "('queued','running','waiting_approval','waiting_review','blocked')";
        let audit = format!(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             SELECT id,'orchestration_disabled','Local-user disabled orchestration; mandate revoked',?2
             FROM sa_tickets WHERE orchestration_id=?1 AND state IN {PENDING}"
        );
        tx.execute(&audit, params![orchestration_id,now]).map_err(|e| e.to_string())?;
        // Legacy terminal Tickets can still carry pending external effects
        // after a partially interrupted state writer. Record their recovery
        // obligation before changing the receipts; terminal Ticket state and
        // original payload identity must remain untouched.
        let terminal_effect_audit = format!(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             SELECT t.id,'orchestration_disabled_effect_uncertain',
               'Pending remote effect quarantined; independent reconciliation required',?2
             FROM sa_tickets t
             WHERE t.orchestration_id=?1 AND t.state NOT IN {PENDING}
               AND EXISTS(SELECT 1 FROM sa_effect_receipts e
                          WHERE e.ticket_id=t.id AND e.state='pending')"
        );
        tx.execute(&terminal_effect_audit,params![orchestration_id,now])
            .map_err(|e| e.to_string())?;
        // Quarantine every pending effect of this Team, including legacy
        // terminal receipts. Never infer failure, mark applied, or resend.
        tx.execute(
            "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
             WHERE state='pending' AND ticket_id IN (
                SELECT id FROM sa_tickets WHERE orchestration_id=?1)",
            params![orchestration_id,now],
        ).map_err(|e| e.to_string())?;
        let grants = format!(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id IN (
               SELECT id FROM sa_tickets WHERE orchestration_id=?1 AND state IN {PENDING})"
        );
        tx.execute(&grants, [orchestration_id]).map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE sa_assignments SET state='interrupted',updated_at=?2
             WHERE ticket_id IN (
               SELECT id FROM sa_tickets WHERE orchestration_id=?1
               AND state IN ('queued','running','waiting_approval','waiting_review','blocked'))
               AND state='running'",
            params![orchestration_id,now],
        ).map_err(|e| e.to_string())?;
        let settle = format!(
            "UPDATE sa_tickets SET state=CASE WHEN state='running' THEN 'interrupted' ELSE 'blocked' END,
             active_assignment_id=NULL,revision=revision+1,updated_at=?2
             WHERE orchestration_id=?1 AND state IN {PENDING}"
        );
        tx.execute(&settle, params![orchestration_id,now]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    configuration(store,orchestration_id)
}

/// Only disabled, unused configurations can change their bot identities.
/// Existing tickets preserve immutable mandate/assignment history.
fn update_configuration(store: &Store, id: &str, expected_revision: i64, input: &NewOrchestration) -> Result<Orchestration> {
    let current = configuration(store, id)?;
    if current.policy_revision != expected_revision || current.enabled {
        return Err("Disable the orchestration and reload its policy revision before editing.".into());
    }
    let name = input.name.trim();
    if name.is_empty() || name.len()>120 || name.chars().any(char::is_control)
        || input.coordinator_bot_id == input.worker_bot_id
    {
        return Err("Choose a valid name and two distinct existing bots.".into());
    }
    let coordinator = bots::get(store, &input.coordinator_bot_id)?;
    let worker = bots::get(store, &input.worker_bot_id)?;
    let identity_changed = current.coordinator_bot_id != coordinator.id
        || current.worker_bot_id != worker.id || current.project_id != worker.project_id;
    if identity_changed {
        if coordinator.paused || worker.paused {
            return Err("Paused bots cannot be selected for a new orchestration policy.".into());
        }
        for bot in [&coordinator, &worker] {
            if let Some(issue) = roster::member_execution_issue(bot) {
                return Err(issue.into());
            }
        }
        if roster::has_additional_members(store,id)? {
            return Err("Clear additional Team roles before changing primary bot identities.".into());
        }
        // Existing Demands must keep their admitted roles after the reusable
        // Team changes. Legacy history without immutable v46 profiles cannot
        // safely recover the original Coordinator; clone that Team instead.
        let historical_ids: Vec<String> = {
            let mut stmt = store.db.prepare(
                "SELECT id FROM sa_tickets WHERE orchestration_id=?1 ORDER BY id",
            ).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([id], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?
        };
        for ticket_id in &historical_ids {
            if roster::ticket_profile(store, ticket_id)?.is_none() {
                return Err("Legacy Ticket history has no immutable Team profile; duplicate the Team to change primary roles.".into());
            }
        }
        // Source configuration is Team-scoped rather than Ticket-snapshotted.
        // Do not silently change its effective project/actor authority when
        // rotating primary roles, even while sources are Off or Observe-only.
        let has_sources: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_event_sources WHERE orchestration_id=?1)",
            [id], |r| r.get(0),
        ).map_err(|e| e.to_string())?;
        if has_sources {
            return Err("Team has GitHub sources; duplicate the Team to change primary roles or project.".into());
        }
    }
    if current.name==name && !identity_changed { return Ok(current); }

    // A Team is reusable: after every Ticket has settled, changing its display
    // name is safe without rebinding old Tickets, roles, Mandates or approvals.
    // Never rename a Team with active/reviewable work or uncertain effects.
    // The SQL predicate repeats the checks atomically with the revision CAS.
    let changed = store.db.execute(
        "UPDATE sa_orchestrations SET name=?2,coordinator_bot_id=?3,worker_bot_id=?4,
            project_id=?5,policy_revision=policy_revision+1,updated_at=?6
         WHERE id=?1 AND policy_revision=?7 AND enabled=0
           AND NOT EXISTS(
             SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
               AND ((?8=1 AND NOT EXISTS(
                     SELECT 1 FROM sa_ticket_profiles p
                     WHERE p.ticket_id=t.id AND p.policy_revision=t.policy_revision))
                 OR t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
                 OR EXISTS(SELECT 1 FROM sa_assignments a
                           WHERE a.ticket_id=t.id AND a.state='running')
                 OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                           WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain')))
           )
           AND (?8=0 OR (
             NOT EXISTS(SELECT 1 FROM sa_event_sources WHERE orchestration_id=?1)
             AND NOT EXISTS(SELECT 1 FROM sa_team_roles WHERE orchestration_id=?1)
           ))",
        params![id,name,coordinator.id,worker.id,worker.project_id,
                storage::now(),expected_revision,identity_changed],
    ).map_err(|e| e.to_string())?;
    if changed!=1 {
        return Err("Team policy or unsettled work changed before the edit; reload required.".into());
    }
    configuration(store,id)
}

/// Never cascade deletion into durable ticket or mandate history.
fn delete_configuration(store: &Store, id: &str, expected_revision: i64) -> Result<()> {
    let current = configuration(store,id)?;
    if current.enabled || current.policy_revision!=expected_revision {
        return Err("Disable the orchestration and reload before deleting it.".into());
    }
    let count = store.db.execute(
        "DELETE FROM sa_orchestrations WHERE id=?1 AND enabled=0 AND policy_revision=?2
         AND NOT EXISTS(SELECT 1 FROM sa_tickets WHERE orchestration_id=?1)",
        params![id,expected_revision],
    ).map_err(|e| e.to_string())?;
    if count!=1 { return Err("Orchestration has ticket history or was updated; deletion denied.".into()); }
    Ok(())
}

#[tauri::command]
pub async fn superagent_update(window: tauri::WebviewWindow, 
    state: State<'_,AppState>, orchestration_id: String,
    expected_revision: i64, input: NewOrchestration,
) -> Result<Orchestration> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e| e.to_string())?;
    update_configuration(&store,&orchestration_id,expected_revision,&input)
}

#[tauri::command]
pub async fn superagent_delete(window: tauri::WebviewWindow, 
    state: State<'_,AppState>, orchestration_id: String, expected_revision: i64,
) -> Result<()> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e| e.to_string())?;
    delete_configuration(&store,&orchestration_id,expected_revision)
}

#[tauri::command]
pub async fn superagent_ticket(window: tauri::WebviewWindow, state: State<'_,AppState>, ticket_id: String) -> Result<Ticket> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    get_ticket(&store, &ticket_id)
}

#[tauri::command]
pub async fn superagent_mandate(window: tauri::WebviewWindow, 
    state: State<'_, AppState>, ticket_id: String,
) -> Result<MandateSummary> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    mandate(&store, &ticket_id)
}

#[tauri::command]
pub async fn superagent_tickets(window: tauri::WebviewWindow, 
    state: State<'_, AppState>, orchestration_id: String, limit: Option<i64>,
) -> Result<Vec<Ticket>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_tickets(&store, &orchestration_id, limit.unwrap_or(100))
}

#[tauri::command]
pub async fn superagent_all_demands(
    window: tauri::WebviewWindow, state: State<'_, AppState>, query: DemandQuery,
) -> Result<Vec<DemandIndex>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_all_demands_page_for_project(
        &store, query.limit.unwrap_or(200), query.before_updated_at,
        query.before_id.as_deref(), DemandPageFilters {
            orchestration_id: query.orchestration_id.as_deref(),
            state_filter: query.state_filter.as_deref(),
            search_query: query.search_query.as_deref(),
            project_id: query.project_id.as_deref(),
        },
    )
}

#[tauri::command]
pub async fn superagent_review_inbox(
    window: tauri::WebviewWindow, state: State<'_, AppState>, limit: Option<i64>,
    before_updated_at: Option<i64>, before_id: Option<String>,
) -> Result<Vec<DemandIndex>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_review_inbox_page(&store, limit.unwrap_or(200), before_updated_at, before_id.as_deref())
}

/// Native Team review projection; scope and cursor are checked inside SQLite.
#[tauri::command]
pub async fn superagent_team_review_inbox(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    orchestration_id: String, limit: Option<i64>,
    before_updated_at: Option<i64>, before_id: Option<String>,
) -> Result<Vec<DemandIndex>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    list_review_inbox_page_scoped(
        &store, limit.unwrap_or(200), before_updated_at,
        before_id.as_deref(), Some(&orchestration_id),
    )
}

#[tauri::command]
pub async fn superagent_assignments(window: tauri::WebviewWindow, 
    state: State<'_, AppState>, ticket_id: String, limit: Option<i64>,
) -> Result<Vec<Assignment>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    assignments(&store, &ticket_id, limit.unwrap_or(100))
}

#[tauri::command]
pub async fn superagent_activity(window: tauri::WebviewWindow, 
    state: State<'_, AppState>, ticket_id: String, limit: Option<i64>,
) -> Result<Vec<AuditEntry>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    history(&store, &ticket_id, limit.unwrap_or(100))
}


#[tauri::command]
pub async fn superagent_activity_page(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    ticket_id: String, before_id: Option<i64>, limit: Option<i64>,
) -> Result<ActivityPage> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    activity_page(&store, &ticket_id, before_id, limit.unwrap_or(50))
}

#[tauri::command]
pub async fn superagent_effect_receipts(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    ticket_id: String, limit: Option<i64>,
) -> Result<Vec<effects::EffectReceiptView>> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    effects::receipt_history(&store, &ticket_id, limit.unwrap_or(50))
}

#[tauri::command]
pub async fn superagent_graph(
    window: tauri::WebviewWindow, state: State<'_, AppState>, ticket_id: String,
) -> Result<RunGraph> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    run_graph(&store, &ticket_id)
}

#[tauri::command]
pub async fn superagent_snapshot(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    ticket_id: String, limit: Option<i64>,
) -> Result<TaskSnapshot> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    ticket_snapshot(&store, &ticket_id, limit.unwrap_or(50))
}

/// The UI can preview the current native manual admission boundary without
/// acquiring a Ticket or issuing a permission. This is NOT authorization:
/// superagent_submit/submit_coordinated must recheck on their own path.
#[tauri::command]
pub async fn superagent_preflight(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    orchestration_id: String, objective: String, coordinated: bool,
) -> Result<ManualPreflight> {
    require_main_window(&window)?;
    let active: Vec<String> = state.agents.lock().map_err(|e| e.to_string())?
        .keys().cloned().collect();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let config = configuration(&store, &orchestration_id)?;
    let bot_id = if coordinated { &config.coordinator_bot_id } else { &config.worker_bot_id };
    let busy = active_manual_conflict(&store, &active, bot_id, &config.project_id)?;
    manual_preflight(&store, &orchestration_id, &objective, coordinated, busy)
}

/// Deliver the exact objective admitted and persisted by native authorization.
/// Never send the raw Desktop input: whitespace outside the bounded objective
/// must not bypass the gateway's maximum inbound-message size.
fn initial_manual_packet(ticket: &Ticket, coordinated: bool) -> String {
    let instructions = if coordinated {
        "Read-only coordinator: recommend the assigned worker's approach and evidence. Your response cannot grant tools, change the worker or approve completion."
    } else {
        "Read-only assignment. Report evidence and remaining gaps; do not claim task approval."
    };
    format!("SuperAgent ticket {}\n\n{}\n\n{}", ticket.id, ticket.objective, instructions)
}

#[tauri::command]
pub async fn superagent_submit(window: tauri::WebviewWindow, 
    app: tauri::AppHandle, state: State<'_,AppState>,
    orchestration_id: String, objective: String, request_key: String,
) -> Result<Ticket> {
    require_main_window(&window)?;
    if crate::feature_controls::get().bots_paused {
        return Err("Bots are paused; no SuperAgent dispatch is allowed.".into());
    }
    let (ticket, launch, worker_id) = {
        let active: Vec<String> = state.agents.lock().map_err(|e|e.to_string())?
            .keys().cloned().collect();
        let mut store=state.store.lock().map_err(|e|e.to_string())?;
        let cfg=configuration(&store,&orchestration_id)?;
        let busy=active_manual_conflict(&store,&active,&cfg.worker_bot_id,&cfg.project_id)?;
        let (ticket,launch)=admit(&mut store,&orchestration_id,&objective,&request_key,busy)?;
        (ticket,launch,cfg.worker_bot_id)
    };
    if !launch { return Ok(ticket); }
    let delivered=bots::gateway::deliver(
        &app,
        bots::gateway::Inbound {
            bot_id: worker_id,
            channel_id: CHANNEL.into(),
            thread: ticket.id.clone(),
            sender_id: "local-user".into(),
            sender_name: "Manual task".into(),
            text: initial_manual_packet(&ticket, false),
            trusted: true,
            image_ids: None,
            moodboard_ids: None,
        },
    ).await;
    if let Err(error) = delivered {
        if let Ok(store)=state.store.lock() { let _=fail_launch(&store,&ticket.id,&error); }
        return Err(format!("Dispatch failed; ticket {} retained for inspection: {error}",ticket.id));
    }
    let store = state.store.lock().map_err(|e| e.to_string())?;
    get_ticket(&store, &ticket.id)
}

/// An occupied worker leaves its ticket blocked, not discarded. Only another
/// explicit local request may retry it, under the original immutable mandate.
/// Each attempt uses a separate native session; a late callback from a failed
/// attempt never becomes evidence for a successor.
fn retry_blocked(
    store: &mut Store, ticket_id: &str, busy: bool,
) -> Result<(Ticket, String, String)> {
    let ticket = get_ticket(store,ticket_id)?;
    if ticket.state != "blocked" {
        return Err("Only a blocked SuperAgent ticket can be retried manually.".into());
    }
    let config = configuration(store,&ticket.orchestration_id)?;
    if !config.enabled || crate::feature_controls::get().bots_paused {
        return Err("SuperAgent remains disabled or globally paused.".into());
    }
    if roster::has_additional_members(store, &config.id)? {
        return Err("Multi-member Team requires native scoped role admission; manual retry denied.".into());
    }
    let coordinator=bots::get(store,&config.coordinator_bot_id)?;
    let worker=bots::get(store,&config.worker_bot_id)?;
    if coordinator.paused || worker.paused || worker.id != ticket_worker(store,ticket_id)?
        || worker.project_id != config.project_id
    {
        return Err("Original worker or coordinator is unavailable or out of scope.".into());
    }
    let mandate_ok: bool = store.db.query_row(
        "SELECT revoked=0 AND expires_at>?2 AND policy_revision=?3
                AND project_id=?4 AND allowed_side_effects=0 AND allowed_tools='[]'
                AND actor=(SELECT actor FROM sa_tickets WHERE id=?1)
         FROM sa_mandates WHERE ticket_id=?1",
        params![ticket_id,storage::now(),config.policy_revision,config.project_id],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?.unwrap_or(false);
    if !mandate_ok {
        return Err("Original mandate expired, was revoked or differs from current policy.".into());
    }
    if !coordinator_plan_current(store, ticket_id, &coordinator.id, &coordinator.project_id)? {
        return Err("Coordinator evidence is stale; a worker retry cannot bypass coordination.".into());
    }
    roster::require_current_profile(store,ticket_id,"worker",&worker.id)?;
    let (attempts, worker_attempts): (i64,i64) = store.db.query_row(
        "SELECT COALESCE(MAX(attempt),0),COUNT(*) FILTER (WHERE bot_id=?2)
         FROM sa_assignments WHERE ticket_id=?1",
        params![ticket_id,worker.id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).map_err(|e| e.to_string())?;
    // This is an immutable, admission-time ceiling, not the Team's editable
    // current preference. Missing snapshots deny retries rather than widening.
    if worker_attempts >= roster::ticket_worker_attempt_limit(store,ticket_id)? {
        return Err("Manual worker retry limit reached; create a new authorized ticket.".into());
    }
    // Reuse the same native lease probe as initial admission: a concurrent
    // coordinator can hold a *different* project than its Ticket's worker,
    // and a missing session must retain its reservation until recovery.
    let occupied = manual_candidate_occupied(
        store, &worker.id, &config.project_id, &worker.project_id,
    )?;
    if busy || occupied {
        return Err("Worker is occupied; the original blocked ticket remains queued for manual retry.".into());
    }
    let attempt=attempts+1;
    let thread=format!("{ticket_id}:attempt:{attempt}");
    let assignment_id=storage::id();
    let tx=store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let session_id=bots::thread_session(store,&worker,CHANNEL,&thread,"SuperAgent · Manual retry")?.0;
    let now=storage::now();
    tx.execute(
        "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
         VALUES(?1,?2,?3,?4,'running',?5,?6,?6)",
        params![assignment_id,ticket_id,worker.id,session_id,attempt,now],
    ).map_err(|e| e.to_string())?;
    let changed=tx.execute(
        "UPDATE sa_tickets SET state='running',active_assignment_id=?2,evidence=NULL,
         revision=revision+1,updated_at=?3
         WHERE id=?1 AND state='blocked' AND revision=?4",
        params![ticket_id,assignment_id,now,ticket.revision],
    ).map_err(|e| e.to_string())?;
    if changed != 1 { return Err("Ticket changed before native retry admission.".into()); }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'manual_retry','user-requested bounded read-only worker attempt',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((get_ticket(store,ticket_id)?,thread,worker.id))
}

fn ticket_worker(store: &Store, ticket_id: &str) -> Result<String> {
    store.db.query_row("SELECT worker_bot_id FROM sa_tickets WHERE id=?1",
        [ticket_id], |r| r.get(0)).map_err(|e| e.to_string())
}

/// Admit the selected existing worker only after a successful, current
/// read-only coordinator turn. A coordinator may recommend work but cannot
/// select another bot, mutate permissions, or expand the immutable mandate.
fn prepare_coordinated_worker(
    store: &mut Store, ticket_id: &str, busy: bool,
) -> Result<(Ticket, String, String, String)> {
    let ticket = get_ticket(store, ticket_id)?;
    if ticket.state != "queued" || ticket.active_assignment_id.is_some() {
        return Err("Coordinator has not produced a current, dispatchable plan.".into());
    }
    let config = configuration(store, &ticket.orchestration_id)?;
    if !config.enabled || crate::feature_controls::get().bots_paused {
        return Err("Orchestration or bots are paused; coordinated dispatch denied.".into());
    }
    if roster::has_additional_members(store, &config.id)? {
        return Err("Multi-member Team requires native scoped role admission; coordinated dispatch denied.".into());
    }
    let coordinator = bots::get(store, &config.coordinator_bot_id)?;
    let worker = bots::get(store, &config.worker_bot_id)?;
    if coordinator.paused || worker.paused || worker.project_id != config.project_id
        || worker.id != ticket_worker(store, ticket_id)?
    {
        return Err("A selected bot is unavailable or outside the immutable project scope.".into());
    }
    // Coordinator advice is tied to the original native session workspace.
    // A later bot project change or deleted source session cannot make that
    // older result an authorized plan for another checkout.
    if !coordinator_plan_current(store, ticket_id, &coordinator.id, &coordinator.project_id)? {
        return Err("Coordinator plan or original workspace is no longer current.".into());
    }
    roster::require_current_profile(store,ticket_id,"worker",&worker.id)?;
    let plan: Option<String> = store.db.query_row(
        "SELECT a.result FROM sa_assignments a
         JOIN sessions coordinator_session ON coordinator_session.id=a.session_id
         WHERE a.ticket_id=?1 AND a.bot_id=?2 AND a.state='finished'
           AND coordinator_session.project_id=?3
         ORDER BY a.attempt DESC LIMIT 1",
        params![ticket_id,coordinator.id,coordinator.project_id], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let Some(plan) = plan.filter(|text| !text.trim().is_empty()) else {
        return Err("No successful coordinator evidence is available.".into());
    };
    let mandate_ok: bool = store.db.query_row(
        "SELECT revoked=0 AND expires_at>?2 AND policy_revision=?3
                AND project_id=?4 AND allowed_side_effects=0 AND allowed_tools='[]'
                AND actor=(SELECT actor FROM sa_tickets WHERE id=?1)
         FROM sa_mandates WHERE ticket_id=?1",
        params![ticket_id,storage::now(),config.policy_revision,config.project_id],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?.unwrap_or(false);
    if !mandate_ok {
        return Err("Coordinator plan cannot extend an expired or revised mandate.".into());
    }
    // A running coordinator may own a different checkout from the project
    // stored on its Ticket. Use the same native lease probe as manual admission
    // and retry, including missing sessions (which must fail closed).
    let occupied = manual_candidate_occupied(
        store, &worker.id, &config.project_id, &worker.project_id,
    )?;
    if busy || occupied {
        return Err("Worker is occupied; coordinated ticket stays queued.".into());
    }
    let next_attempt: i64 = store.db.query_row(
        "SELECT COALESCE(MAX(attempt),0)+1 FROM sa_assignments WHERE ticket_id=?1",
        [ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    let thread = format!("{ticket_id}:worker:{next_attempt}");
    let assignment_id = storage::id();
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let session_id = bots::thread_session(store, &worker, CHANNEL, &thread,
        "SuperAgent · Coordinated worker")?.0;
    let now = storage::now();
    tx.execute(
        "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
         VALUES(?1,?2,?3,?4,'running',?5,?6,?6)",
        params![assignment_id,ticket_id,worker.id,session_id,next_attempt,now],
    ).map_err(|e| e.to_string())?;
    let moved = tx.execute(
        "UPDATE sa_tickets SET state='running',active_assignment_id=?2,evidence=NULL,revision=revision+1,updated_at=?3
         WHERE id=?1 AND state='queued' AND revision=?4",
        params![ticket_id,assignment_id,now,ticket.revision],
    ).map_err(|e| e.to_string())?;
    if moved != 1 { return Err("Coordinator plan was invalidated before worker admission.".into()); }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'worker_admitted','manual dispatch after coordinator evidence',?2)",
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((get_ticket(store,ticket_id)?,thread,worker.id,plan))
}

#[tauri::command]
pub async fn superagent_submit_coordinated(window: tauri::WebviewWindow, 
    app: tauri::AppHandle, state: State<'_, AppState>,
    orchestration_id: String, objective: String, request_key: String,
) -> Result<Ticket> {
    require_main_window(&window)?;
    if crate::feature_controls::get().bots_paused {
        return Err("Bots are paused; coordinated dispatch is denied.".into());
    }
    let (ticket,launch,coordinator_id) = {
        let active: Vec<String> = state.agents.lock().map_err(|e|e.to_string())?
            .keys().cloned().collect();
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let cfg = configuration(&store, &orchestration_id)?;
        let busy = active_manual_conflict(&store,&active,&cfg.coordinator_bot_id,&cfg.project_id)?;
        let (ticket,launch) = admit_role(&mut store,&orchestration_id,&objective,&request_key,busy,true)?;
        (ticket,launch,cfg.coordinator_bot_id)
    };
    if !launch { return Ok(ticket); }
    let delivered = bots::gateway::deliver(&app,bots::gateway::Inbound {
        bot_id: coordinator_id,
        channel_id: CHANNEL.into(),
        thread: ticket.id.clone(),
        sender_id: "local-user".into(),
        sender_name: "Manual coordination".into(),
        text: initial_manual_packet(&ticket, true),
        trusted: true, image_ids: None, moodboard_ids: None,
    }).await;
    if let Err(error) = delivered {
        if let Ok(store) = state.store.lock() { let _ = fail_launch(&store,&ticket.id,&error); }
        return Err(format!("Coordinator dispatch failed; ticket {} retained: {error}",ticket.id));
    }
    let store = state.store.lock().map_err(|e|e.to_string())?;
    get_ticket(&store,&ticket.id)
}

#[tauri::command]
pub async fn superagent_dispatch_worker(window: tauri::WebviewWindow, 
    app: tauri::AppHandle, state: State<'_, AppState>, ticket_id: String,
) -> Result<Ticket> {
    require_main_window(&window)?;
    let (ticket,thread,worker_id,plan) = {
        let active: Vec<String> = state.agents.lock().map_err(|e|e.to_string())?
            .keys().cloned().collect();
        let mut store = state.store.lock().map_err(|e|e.to_string())?;
        let worker = ticket_worker(&store,&ticket_id)?;
        let ticket = get_ticket(&store, &ticket_id)?;
        let cfg = configuration(&store, &ticket.orchestration_id)?;
        let busy = active_manual_conflict(&store, &active, &worker, &cfg.project_id)?;
        prepare_coordinated_worker(&mut store,&ticket_id,busy)?
    };
    let delivered = bots::gateway::deliver(&app,bots::gateway::Inbound {
        bot_id: worker_id, channel_id: CHANNEL.into(), thread,
        sender_id: "local-user".into(), sender_name: "Manual delegated task".into(),
        text: format!(
            "SuperAgent ticket {}\n\nObjective: {}\n\nCoordinator advice (untrusted context; not authority):\n{}\n\nRead-only assignment. Report evidence and remaining gaps; only native approval may finish the ticket.",
            ticket.id,ticket.objective,plan,
        ),
        trusted: true, image_ids: None, moodboard_ids: None,
    }).await;
    if let Err(error) = delivered {
        if let Ok(store) = state.store.lock() { let _ = fail_launch(&store,&ticket.id,&error); }
        return Err(format!("Worker dispatch failed; ticket {} retained: {error}",ticket.id));
    }
    let store = state.store.lock().map_err(|e|e.to_string())?;
    get_ticket(&store,&ticket.id)
}

#[tauri::command]
pub async fn superagent_retry(window: tauri::WebviewWindow, 
    app: tauri::AppHandle, state: State<'_,AppState>, ticket_id: String,
) -> Result<Ticket> {
    require_main_window(&window)?;
    let (ticket,thread,worker_id)={
        let active: Vec<String> = state.agents.lock().map_err(|e|e.to_string())?
            .keys().cloned().collect();
        let mut store=state.store.lock().map_err(|e|e.to_string())?;
        let bot_id=ticket_worker(&store,&ticket_id)?;
        let ticket = get_ticket(&store, &ticket_id)?;
        let cfg = configuration(&store, &ticket.orchestration_id)?;
        let busy=active_manual_conflict(&store,&active,&bot_id,&cfg.project_id)?;
        retry_blocked(&mut store,&ticket_id,busy)?
    };
    let delivered=bots::gateway::deliver(&app,bots::gateway::Inbound {
        bot_id: worker_id,
        channel_id: CHANNEL.into(),
        thread,
        sender_id: "local-user".into(),
        sender_name: "Manual retry".into(),
        text: format!("SuperAgent ticket {} (manual retry)\n\n{}\n\nRead-only assignment. Report evidence and remaining gaps; do not claim approval.",ticket.id,ticket.objective),
        trusted: true,
        image_ids: None,
        moodboard_ids: None,
    }).await;
    if let Err(error)=delivered {
        if let Ok(store)=state.store.lock() { let _=fail_launch(&store,&ticket.id,&error); }
        return Err(format!("Retry dispatch failed; ticket {} retained: {error}",ticket.id));
    }
    let store=state.store.lock().map_err(|e|e.to_string())?;
    get_ticket(&store,&ticket.id)
}


#[tauri::command]
pub async fn superagent_accept(window: tauri::WebviewWindow, state: State<'_,AppState>, ticket_id: String, expected_revision: i64) -> Result<Ticket> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    accept(&store, &ticket_id, expected_revision)
}

#[tauri::command]
pub async fn superagent_reject(window: tauri::WebviewWindow, 
    state: State<'_, AppState>, ticket_id: String, expected_revision: i64, reason: String,
) -> Result<Ticket> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    reject(&store, &ticket_id, expected_revision, &reason)
}

#[tauri::command]
pub async fn superagent_stop(
    window: tauri::WebviewWindow, app: tauri::AppHandle, state: State<'_, AppState>,
    ticket_id: String, expected_revision: Option<i64>,
) -> Result<Ticket> {
    require_main_window(&window)?;
    let (ticket,session)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        stop_with_expected_revision(&store, &ticket_id, expected_revision)?
    };
    if let Some(session)=session {
        let _=crate::stop_session(app.clone(),app.state::<AppState>(),session).await;
    }
    Ok(ticket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_policy_projection_sees_unpaged_review_and_restart_uncertain_effect() {
        let store=fixture();
        let active=enabled(&store);
        assert!(team_policy_edit_blocker(&store,&active).unwrap().is_some());
        let team=change_enabled(&store,&active.id,active.policy_revision,false).unwrap();
        assert!(team_policy_edit_blocker(&store,&team).unwrap().is_none());
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,
             project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES('old-review',?1,'old-review-key','Objective','local-user','manual',?2,?3,
             ?4,'waiting_review',?5,?5)",
            params![team.id,team.project_id,team.worker_bot_id,team.policy_revision,now-1],
        ).unwrap();
        for i in 0..55 {
            store.db.execute(
                "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,
                 project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Closed','local-user','manual',?4,?5,?6,'completed',?7,?7)",
                params![format!("old-{i}"),team.id,format!("key-{i}"),team.project_id,
                    team.worker_bot_id,team.policy_revision,now+i+1],
            ).unwrap();
        }
        let reason=team_policy_edit_blocker(&store,&team).unwrap().unwrap();
        assert!(reason.contains("Active or reviewable"));
        assert_eq!(orchestration_overview(&store).unwrap().iter()
            .find(|entry|entry.config.id==team.id).unwrap().policy_edit_blocker.as_deref(),Some(reason.as_str()));
        assert!(roster::set_worker_attempt_limit(&store,&team.id,team.policy_revision,3).is_err());
        store.db.execute("UPDATE sa_tickets SET state='completed' WHERE id='old-review'",[]).unwrap();
        assert!(team_policy_edit_blocker(&store,&team).unwrap().is_none());
        store.db.execute(
            "INSERT INTO sa_effect_receipts(ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES('old-review','remote','digest','uncertain',?1,?1)",[now],
        ).unwrap();
        let path=store.path.clone(); drop(store);
        let opened=Store::open_read_only(&path).unwrap();
        assert_eq!(team_policy_edit_blocker(&opened,&team).unwrap().as_deref(),
            Some("Unresolved external effects prevent Team policy edits."));
    }

    #[test]
    fn desktop_stop_requires_fresh_ticket_revision_and_preserves_late_callback_denial() {
        let mut store = fixture();
        let team = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &team.id, "Revision-scoped Demand stop",
            "desktop-stop-revision-native", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let audit_before = history(&store, &ticket.id, 20).unwrap().len();

        assert!(stop_with_expected_revision(
            &store, &ticket.id, Some(ticket.revision + 1),
        ).is_err(), "a stale global Demand card must never revoke authority");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), audit_before);

        let (stopped, provider_session) = stop_with_expected_revision(
            &store, &ticket.id, Some(ticket.revision),
        ).unwrap();
        assert_eq!(stopped.state, "stopped");
        assert_eq!(stopped.revision, ticket.revision + 1);
        assert_eq!(provider_session.as_deref(), Some(session.as_str()));
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), audit_before + 1);
        assert!(stop_with_expected_revision(
            &store, &ticket.id, Some(ticket.revision),
        ).is_err(), "a repeated stop must not duplicate the audit");
        assert!(!on_run_finished(&store, &session, true, "Late result").unwrap());
    }

    #[test]
    fn configured_worker_budget_is_immutable_for_a_demand_and_denies_extra_retry() {
        let mut store=fixture();
        let enabled_config=enabled(&store);
        let disabled=change_enabled(&store,&enabled_config.id,enabled_config.policy_revision,false).unwrap();
        let first=roster::read(&store,&disabled.id).unwrap();
        assert_eq!(first.max_worker_attempts,2);
        assert!(roster::set_worker_attempt_limit(&store,&disabled.id,disabled.policy_revision,0).is_err());
        assert!(roster::set_worker_attempt_limit(&store,&disabled.id,disabled.policy_revision,5).is_err());
        let configured=roster::set_worker_attempt_limit(&store,&disabled.id,disabled.policy_revision,1).unwrap();
        assert_eq!(configured.max_worker_attempts,1);
        assert_eq!(configured.policy_revision,disabled.policy_revision+1);
        assert!(roster::set_worker_attempt_limit(&store,&disabled.id,disabled.policy_revision,3).is_err());
        let reenabled=change_enabled(&store,&disabled.id,configured.policy_revision,true).unwrap();
        assert!(roster::set_worker_attempt_limit(&store,&disabled.id,reenabled.policy_revision,4).is_err());
        let (ticket,started)=admit(&mut store,&disabled.id,"Budgeted Worker","worker-attempt-budget-one",false).unwrap();
        assert!(started);
        assert_eq!(roster::ticket_worker_attempt_limit(&store,&ticket.id).unwrap(),1);
        fail_launch(&store,&ticket.id,"test provider unavailable").unwrap();
        let sessions_before:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sessions",[],|r|r.get(0),
        ).unwrap();
        assert!(retry_blocked(&mut store,&ticket.id,false).unwrap_err().contains("limit reached"));
        let sessions_after:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sessions",[],|r|r.get(0),
        ).unwrap();
        assert_eq!(sessions_before,sessions_after);
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"blocked");
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(roster::ticket_worker_attempt_limit(&reopened,&ticket.id).unwrap(),1);
        assert_eq!(roster::read(&reopened,&disabled.id).unwrap().max_worker_attempts,1);
    }

    #[test]
    fn new_team_budget_copy_is_disabled_and_never_copies_task_authority() {
        let store=fixture();
        let original=enabled(&store);
        let disabled=change_enabled(&store,&original.id,original.policy_revision,false).unwrap();
        let cap=roster::set_worker_attempt_limit(&store,&original.id,disabled.policy_revision,3).unwrap();
        let copy=roster::duplicate_with_roster(&store,&original.id,cap.policy_revision,
            "Bounded copy".into()).unwrap();
        assert!(!copy.enabled);
        assert_eq!(roster::read(&store,&copy.id).unwrap().max_worker_attempts,3);
        assert_eq!(roster::read(&store,&original.id).unwrap().max_worker_attempts,3);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",[copy.id],
            |r|r.get::<_,i64>(0),
        ).unwrap(),0);
    }

    #[test]
    fn native_demand_query_accepts_camel_case_and_rejects_execution_fields() {
        let query: DemandQuery = serde_json::from_value(serde_json::json!({
            "limit": 25, "beforeUpdatedAt": 2000, "beforeId": "ticket-5",
            "orchestrationId": "team-1", "stateFilter": "blocked",
            "searchQuery": "literal %_needle", "projectId": "project-a",
        })).unwrap();
        assert_eq!(query.limit, Some(25));
        assert_eq!(query.before_updated_at, Some(2000));
        assert_eq!(query.before_id.as_deref(), Some("ticket-5"));
        assert_eq!(query.orchestration_id.as_deref(), Some("team-1"));
        assert_eq!(query.state_filter.as_deref(), Some("blocked"));
        assert_eq!(query.search_query.as_deref(), Some("literal %_needle"));
        assert_eq!(query.project_id.as_deref(), Some("project-a"));
        let empty: DemandQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty.limit, None);
        assert_eq!(empty.search_query, None);
        assert!(serde_json::from_value::<DemandQuery>(serde_json::json!({
            "limit": 200, "allowedTools": ["write_file"]
        })).is_err());
    }

    #[test]
    fn native_demand_project_filter_precedes_paging_and_survives_restart() {
        let store = fixture();
        let team = enabled(&store);
        for index in 0..205_i64 {
            let project = if index == 204 {
                "other-native-project"
            } else {
                team.project_id.as_str()
            };
            store.db.execute(
                "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,
                        source,project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,'local-user','manual',?5,'worker',?6,'blocked',?7,?7)",
                params![
                    format!("native-project-{index:03}"), team.id,
                    format!("native-project-request-{index:03}"),
                    format!("Native project demand {index}"), project,
                    team.policy_revision, 1_000 + index,
                ],
            ).unwrap();
        }
        let page = |store: &Store, before_at: Option<i64>, before_id: Option<&str>,
                    project: Option<&str>| {
            list_all_demands_page_for_project(
                store, 200, before_at, before_id, DemandPageFilters {
                    orchestration_id: Some(&team.id), state_filter: Some("blocked"),
                    search_query: None, project_id: project,
                },
            )
        };
        let first = page(&store, None, None, Some(&team.project_id)).unwrap();
        assert_eq!(first.len(), 200);
        assert!(!first.iter().any(|entry| entry.id == "native-project-204"));
        let last = first.last().unwrap();
        let second = page(&store, Some(last.updated_at), Some(&last.id),
            Some(&team.project_id)).unwrap();
        assert_eq!(second.len(), 4);
        // S2 Demand history must reject forged, stale and cross-project
        // cursors before they can silently conceal earlier native Tickets.
        assert!(page(&store, Some(last.updated_at), Some("missing-ticket"),
            Some(&team.project_id)).is_err());
        assert!(page(&store, Some(last.updated_at + 1), Some(&last.id),
            Some(&team.project_id)).is_err());
        assert!(page(&store, Some(last.updated_at), Some(&last.id),
            Some("other-native-project")).is_err());
        store.db.execute(
            "UPDATE sa_tickets SET state='running' WHERE id=?1", [&last.id],
        ).unwrap();
        assert!(page(&store, Some(last.updated_at), Some(&last.id),
            Some(&team.project_id)).is_err());
        store.db.execute(
            "UPDATE sa_tickets SET state='blocked' WHERE id=?1", [&last.id],
        ).unwrap();
        assert_eq!(page(&store, Some(last.updated_at), Some(&last.id),
            Some(&team.project_id)).unwrap().len(), 4);
        let ids: std::collections::HashSet<_> = first.iter().chain(second.iter())
            .map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids.len(), 204, "Filtering before paging must not lose Tickets");
        let foreign = page(&store, None, None, Some("other-native-project")).unwrap();
        assert_eq!(foreign.len(), 1);
        assert_eq!(foreign[0].id, "native-project-204");
        assert_eq!(foreign[0].project_id, "other-native-project");
        assert!(first.iter().all(|entry| entry.project_id == team.project_id));
        for invalid in ["", "../escape", "project%secret", " spaces "] {
            assert!(page(&store, None, None, Some(invalid)).is_err());
        }
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(page(&reopened, None, None, Some(&team.project_id)).unwrap().len(), 200);
        assert_eq!(page(&reopened, Some(last.updated_at), Some(&last.id),
            Some(&team.project_id)).unwrap().len(), 4);
        assert!(page(&reopened, Some(last.updated_at), Some("missing-ticket"),
            Some(&team.project_id)).is_err());
        assert_eq!(page(&reopened, None, None, Some("other-native-project")).unwrap().len(), 1);
    }

    #[test]
    fn demand_review_inbox_keeps_admitted_project_when_team_config_changes() {
        let store = fixture();
        let team = enabled(&store);
        store.db.execute(
            "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,
                    source,project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES('historical-review',?1,'historical-review-request','Historical project',
                    'local-user','manual','archived-project','worker',?2,'waiting_review',1000,1000)",
            params![team.id, team.policy_revision],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET project_id='new-team-project' WHERE id=?1",
            [&team.id],
        ).unwrap();
        let review = list_review_inbox(&store, 200).unwrap();
        let historical = review.iter().find(|item| item.id == "historical-review").unwrap();
        assert_eq!(historical.project_id, "archived-project");
        let page = list_all_demands_page_for_project(
            &store, 200, None, None, DemandPageFilters {
                orchestration_id: Some(&team.id), state_filter: Some("waiting_review"),
                search_query: None, project_id: Some("archived-project"),
            },
        ).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].project_id, "archived-project");
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(list_review_inbox(&reopened, 200).unwrap()
            .into_iter().find(|item| item.id == "historical-review").unwrap().project_id,
            "archived-project");
    }

    #[test]
    fn headless_roster_and_attempt_budget_are_native_revisioned_and_restart_durable() {
        let store = fixture();
        let config = create_configuration(&store, NewOrchestration {
            name: "Operator Team".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('worker-two','Another worker','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        let members = roster::RosterInput {
            worker_bot_ids: vec!["worker".into(), "worker-two".into()],
            reviewer_bot_ids: Vec::new(),
        };
        let raw = headless_set_team_roster(
            &store.path, &config.id, config.policy_revision, &members,
        ).unwrap();
        let response: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(response["workerCount"], 2);
        assert_eq!(response["reviewerCount"], 0);
        assert_eq!(response["executionReady"], false);
        assert_eq!(response["grantIssued"], false);
        assert_eq!(response["providerDispatched"], false);
        assert!(!raw.contains("Another worker"));
        let revised = response["policyRevision"].as_i64().unwrap();
        assert_eq!(revised, config.policy_revision + 1);
        assert!(headless_set_team_roster(
            &store.path, &config.id, config.policy_revision, &members,
        ).is_err(), "A stale operator cannot replace the roster");
        let same = headless_set_team_roster(
            &store.path, &config.id, revised, &members,
        ).unwrap();
        let same: serde_json::Value = serde_json::from_str(&same).unwrap();
        assert_eq!(same["policyRevision"], revised, "No-op edits must not increment policy");

        let invalid = roster::RosterInput {
            worker_bot_ids: vec!["worker".into(), "worker".into()],
            reviewer_bot_ids: Vec::new(),
        };
        assert!(headless_set_team_roster(&store.path, &config.id, revised, &invalid).is_err());
        assert_eq!(roster::read(&store, &config.id).unwrap().policy_revision, revised);

        let budget = headless_set_team_attempt_limit(
            &store.path, &config.id, revised, 3,
        ).unwrap();
        let budget: serde_json::Value = serde_json::from_str(&budget).unwrap();
        assert_eq!(budget["maxWorkerAttempts"], 3);
        assert_eq!(budget["grantIssued"], false);
        assert_eq!(budget["providerDispatched"], false);
        let latest = budget["policyRevision"].as_i64().unwrap();
        assert_eq!(latest, revised + 1);
        assert!(headless_set_team_attempt_limit(
            &store.path, &config.id, revised, 4,
        ).is_err(), "Stale budget edits are rejected");
        assert!(headless_set_team_attempt_limit(
            &store.path, &config.id, latest, 0,
        ).is_err(), "Invalid budgets are rejected");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |r| r.get::<_, i64>(0),
        ).unwrap(), 0);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_mandates", [], |r| r.get::<_, i64>(0),
        ).unwrap(), 0);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        let persisted = roster::read(&reopened, &config.id).unwrap();
        assert_eq!(persisted.worker_bot_ids, members.worker_bot_ids);
        assert_eq!(persisted.max_worker_attempts, 3);
        assert_eq!(persisted.policy_revision, latest);
        assert!(!configuration(&reopened, &config.id).unwrap().enabled);
    }

    #[test]
    fn headless_queue_demand_is_durable_idempotent_and_never_dispatches() {
        let store = fixture();
        let cfg = enabled(&store);
        let objective = "Private headless objective";
        let raw = headless_queue_demand(
            &store.path, &cfg.id, objective, "operator-key-1",
        ).unwrap();
        let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let id = view["ticketId"].as_str().unwrap();
        assert_eq!(view["state"], "blocked");
        assert_eq!(view["providerDispatchedByCommand"], false);
        assert_eq!(view["requiresExplicitNativeDispatch"], true);
        assert_eq!(view["allowedTools"], serde_json::json!([]));
        assert!(!raw.contains(objective));
        assert!(!raw.contains("operator-key-1"));
        assert!(get_ticket(&store, id).unwrap().active_assignment_id.is_none());
        assert!(!mandate(&store, id).unwrap().revoked);
        assert!(coordinator_plan_current(
            &store, id, "router", &cfg.project_id,
        ).unwrap());
        let assignment_count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
            [id], |row| row.get(0),
        ).unwrap();
        assert_eq!(assignment_count, 0);
        assert_eq!(raw, headless_queue_demand(
            &store.path, &cfg.id, objective, "operator-key-1",
        ).unwrap());
        assert!(headless_queue_demand(
            &store.path, &cfg.id, "Changed objective", "operator-key-1",
        ).is_err());
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",
            [&cfg.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
        assert!(headless_stop_ticket(&store.path, id).unwrap().contains("stopped"));
        let after: serde_json::Value = serde_json::from_str(&headless_queue_demand(
            &store.path, &cfg.id, objective, "operator-key-1",
        ).unwrap()).unwrap();
        assert_eq!(after["state"], "stopped");
        assert_eq!(after["requiresExplicitNativeDispatch"], false);
        assert!(headless_queue_demand(
            &store.path, "missing-team", objective, "new-key",
        ).is_err());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(get_ticket(&reopened, id).unwrap().state, "stopped");
    }


    #[test]
    fn headless_staged_demand_can_run_only_after_explicit_native_retry() {
        let mut store = fixture();
        let config = enabled(&store);
        let raw = headless_queue_demand(
            &store.path, &config.id, "Staged readonly work", "headless-retry-a",
        ).unwrap();
        let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let ticket_id = view["ticketId"].as_str().unwrap();
        let native_thread = format!("{ticket_id}:attempt:1");

        // Staging is not dispatch. A forged inbound message has no assignment
        // and must be denied before the gateway can create a conversation.
        assert!(assignment_session(&store, ticket_id).unwrap().is_none());
        assert!(guard_gateway_inbound(
            &store, "worker", &native_thread, "local-user", true,
        ).is_err());

        let (running, thread, worker) = retry_blocked(
            &mut store, ticket_id, false,
        ).unwrap();
        assert_eq!(running.state, "running");
        assert_eq!(thread, native_thread);
        assert_eq!(worker, "worker");
        let session = assignment_session(&store, ticket_id).unwrap().unwrap();
        assert!(guard_gateway_inbound(
            &store, &worker, &thread, "local-user", true,
        ).is_ok());
        assert!(guard_gateway_inbound(
            &store, &worker, &thread, "untrusted", true,
        ).is_err());
        assert!(read_only_policy(&store, &session).unwrap().is_some());
        assert!(on_run_finished(&store, &session, true, "Verified worker evidence").unwrap());
        let awaiting = get_ticket(&store, ticket_id).unwrap();
        assert_eq!(awaiting.state, "waiting_approval");
        assert_eq!(accept(&store, ticket_id, awaiting.revision).unwrap().state, "completed");
        assert!(!on_run_finished(&store, &session, true, "Stale callback").unwrap());
        assert!(guard_gateway_inbound(
            &store, &worker, &thread, "local-user", true,
        ).is_err());
    }

    #[test]
    fn headless_staged_retry_without_original_audit_cannot_enter_gateway() {
        let mut store = fixture();
        let config = enabled(&store);
        let raw = headless_queue_demand(
            &store.path, &config.id, "Staged scope", "headless-retry-b",
        ).unwrap();
        let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let ticket_id = view["ticketId"].as_str().unwrap();
        let (_, thread, worker) = retry_blocked(
            &mut store, ticket_id, false,
        ).unwrap();
        let session = assignment_session(&store, ticket_id).unwrap().unwrap();
        store.db.execute(
            "UPDATE sa_audit SET kind='invalidated_retry'
             WHERE ticket_id=?1 AND kind='manual_retry'",
            [ticket_id],
        ).unwrap();
        assert!(guard_gateway_inbound(
            &store, &worker, &thread, "local-user", true,
        ).is_err());
        assert!(read_only_policy(&store, &session).is_err());
        assert_eq!(get_ticket(&store, ticket_id).unwrap().state, "running");
    }

    #[test]
    fn headless_activity_is_paginated_private_and_read_only_across_reopen() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private activity objective",
            "activity-page-main", false,
        ).unwrap();
        assert!(launched);
        for index in 0..205 {
            store.db.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'activity_test',?2,?3)",
                params![&ticket.id, format!("private-marker-{index}"), storage::now()],
            ).unwrap();
        }
        let (other, _) = admit(
            &mut store, &cfg.id, "Other private objective",
            "activity-page-other", true,
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'unrelated', 'cross-ticket-marker',?2)",
            params![&other.id, storage::now()],
        ).unwrap();
        let audit_count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let mut cursor = None;
        let mut previous = None;
        let mut seen = std::collections::HashSet::new();
        loop {
            let raw = list_headless_ticket_activity(&store.path, &ticket.id, cursor).unwrap();
            assert!(!raw.contains("private-marker"));
            assert!(!raw.contains("Private activity objective"));
            assert!(!raw.contains("cross-ticket-marker"));
            let page: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(page["ticketId"], ticket.id);
            assert_eq!(page["pageLimit"], 100);
            assert_eq!(page["redacted"], true);
            assert_eq!(page["observationalOnly"], true);
            assert_eq!(page["recoveryPerformed"], false);
            let entries = page["items"].as_array().unwrap();
            assert!(entries.len() <= 100);
            for entry in entries {
                let id = entry["id"].as_i64().unwrap();
                assert!(previous.is_none_or(|last| id < last));
                assert!(seen.insert(id), "audit row replayed across pages");
                assert!(entry.get("detail").is_none());
                previous = Some(id);
            }
            if page["hasMore"] == false {
                assert!(page["nextCursor"].is_null());
                break;
            }
            cursor = Some(page["nextCursor"].as_i64().unwrap());
        }
        assert_eq!(seen.len() as i64, audit_count);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, i64>(0),
        ).unwrap(), audit_count);
        assert!(list_headless_ticket_activity(&store.path, &ticket.id, Some(0)).is_err());
        assert!(list_headless_ticket_activity(&store.path, "missing-ticket", None).is_err());
        let path = store.path.clone();
        drop(store);
        let restarted: serde_json::Value = serde_json::from_str(
            &list_headless_ticket_activity(&path, &ticket.id, None).unwrap(),
        ).unwrap();
        assert_eq!(restarted["items"].as_array().unwrap().len(), 100);
    }


    #[test]
    fn native_desktop_activity_pages_are_ticket_scoped_strict_and_restart_durable() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Sensitive desktop activity",
            "desktop-activity-main", false,
        ).unwrap();
        assert!(launched);
        for index in 0..125 {
            store.db.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'activity_test',?2,?3)",
                params![&ticket.id, format!("private-{index}"), storage::now()],
            ).unwrap();
        }
        let (other, _) = admit(
            &mut store, &cfg.id, "Other activity", "desktop-activity-other", true,
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'unrelated','foreign-detail',?2)",
            params![&other.id, storage::now()],
        ).unwrap();
        let expected: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let mut cursor = None;
        let mut visited = std::collections::HashSet::new();
        let mut previous = i64::MAX;
        loop {
            let page = activity_page(&store, &ticket.id, cursor, 50).unwrap();
            assert_eq!(page.ticket_id, ticket.id);
            assert!(page.items.len() <= 50);
            for item in &page.items {
                assert!(item.id < previous);
                assert!(visited.insert(item.id), "activity row was replayed");
                assert_ne!(item.detail, "foreign-detail");
                previous = item.id;
            }
            if !page.has_more {
                assert!(page.next_cursor.is_none());
                break;
            }
            assert_eq!(page.next_cursor, page.items.last().map(|item| item.id));
            cursor = page.next_cursor;
        }
        assert_eq!(visited.len() as i64, expected);
        assert!(activity_page(&store, &ticket.id, Some(0), 50).is_err());
        assert!(activity_page(&store, &ticket.id, None, 0).is_err());
        assert!(activity_page(&store, &ticket.id, None, 101).is_err());
        assert!(activity_page(&store, "missing-ticket", None, 50).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        let db_path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&db_path).unwrap();
        assert_eq!(activity_page(&reopened, &ticket.id, None, 50).unwrap().items.len(), 50);
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "running");
    }

    #[test]
    fn headless_roster_inspection_preserves_multi_role_profiles_without_admission() {
        let store=fixture();
        for id in ["additional-worker","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        store.db.execute(
            "UPDATE bots SET skills='[\"scope-skill\"]' WHERE id='additional-worker'",[],
        ).unwrap();
        let config=create_configuration(&store,NewOrchestration {
            name:"Headless roster".into(),
            coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'worker',1,'additional-worker')",[&config.id],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'reviewer',0,'reviewer')",[&config.id],
        ).unwrap();
        let path=store.path.clone();
        drop(store);
        let output=inspect_headless_team_roster(&path,&config.id).unwrap();
        let view:serde_json::Value=serde_json::from_str(&output).unwrap();
        assert_eq!(view["policyRevision"],config.policy_revision);
        assert_eq!(view["members"].as_array().unwrap().len(),4);
        assert_eq!(view["members"][0]["role"],"coordinator");
        assert_eq!(view["members"][1]["role"],"worker");
        assert_eq!(view["members"][2]["botId"],"additional-worker");
        assert_eq!(view["members"][2]["assignedSkillIds"],serde_json::json!(["scope-skill"]));
        assert_eq!(view["members"][2]["skillLibraryStatus"],"not_scanned");
        assert_eq!(view["members"][3]["role"],"reviewer");
        assert_eq!(view["manualSingleWorkerTopology"],false);
        assert_eq!(view["multiRoleExecutionAvailable"],false);
        assert_eq!(view["authorizationIssued"],false);
        assert_eq!(view["allowedTools"],serde_json::json!([]));
        for sensitive in ["authorizationId","sessionId","PRIVATE OBJECTIVE","worktreePath"] {
            assert!(!output.contains(sensitive),"Unexpected private field: {sensitive}");
        }
        let verified=Store::open_read_only(&path).unwrap();
        assert_eq!(verified.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(verified.db.query_row(
            "SELECT COUNT(*) FROM sa_mandates",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert!(inspect_headless_team_roster(&path,"not-a-team").is_err());
    }
    #[test]
    fn headless_exact_demand_preflight_is_read_only_and_revision_bound() {
        let store = fixture();
        let cfg = create_configuration(&store, NewOrchestration {
            name: "Scoped headless preflight".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let objective = "Private scope: inspect only repository paths";
        let disabled: serde_json::Value = serde_json::from_str(
            &inspect_headless_demand_preflight(&store.path, &cfg.id, objective, false).unwrap()
        ).unwrap();
        assert_eq!(disabled["eligibleAtSnapshot"], false);
        assert_eq!(disabled["mandateIssued"], false);
        assert_eq!(disabled["providerDispatched"], false);
        assert_eq!(disabled["allowedTools"], serde_json::json!([]));
        assert!(!disabled.to_string().contains(objective));
        let enabled = change_enabled(&store, &cfg.id, cfg.policy_revision, true).unwrap();
        let direct: serde_json::Value = serde_json::from_str(
            &inspect_headless_demand_preflight(&store.path, &cfg.id, objective, false).unwrap()
        ).unwrap();
        let coordinated: serde_json::Value = serde_json::from_str(
            &inspect_headless_demand_preflight(&store.path, &cfg.id, objective, true).unwrap()
        ).unwrap();
        assert_eq!(direct["selectedRole"], "worker");
        assert_eq!(direct["selectedBotId"], "worker");
        assert_eq!(coordinated["selectedRole"], "coordinator");
        assert_eq!(coordinated["selectedBotId"], "router");
        assert_eq!(direct["policyRevision"], enabled.policy_revision);
        assert_eq!(direct["eligibleAtSnapshot"], true);
        assert_eq!(coordinated["eligibleAtSnapshot"], true);
        assert_eq!(direct["requiresLiveAdmissionRecheck"], true);
        assert_eq!(direct["objectiveBytes"], objective.len());
        assert!(inspect_headless_demand_preflight(&store.path, &cfg.id, "  ", false).is_err());
        assert!(inspect_headless_demand_preflight(&store.path, "missing", objective, true).is_err());
        assert!(inspect_headless_demand_preflight(
            &store.path, &cfg.id, &"x".repeat(MAX_OBJECTIVE_BYTES + 1), false
        ).is_err());
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        let paused: serde_json::Value = serde_json::from_str(
            &inspect_headless_demand_preflight(&store.path, &cfg.id, objective, false).unwrap()
        ).unwrap();
        assert_eq!(paused["eligibleAtSnapshot"], false);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 0);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_mandates", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 0);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 0);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 0);
    }

    #[test]
    fn headless_team_readiness_is_redacted_nonmutating_and_never_admits() {
        let store = fixture();
        let cfg = create_configuration(&store, NewOrchestration {
            name: "Headless readiness".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let disabled: serde_json::Value = serde_json::from_str(
            &inspect_headless_team_readiness(&store.path, &cfg.id).unwrap()
        ).unwrap();
        assert_eq!(disabled["worker"]["eligibleAtSnapshot"], false);
        assert_eq!(disabled["admitted"], false);
        assert_eq!(disabled["requiresLiveAdmissionRecheck"], true);
        assert_eq!(disabled["allowedSideEffects"], false);
        assert_eq!(disabled["policyRevision"], cfg.policy_revision);
        let enabled = change_enabled(&store, &cfg.id, cfg.policy_revision, true).unwrap();
        let ready: serde_json::Value = serde_json::from_str(
            &inspect_headless_team_readiness(&store.path, &cfg.id).unwrap()
        ).unwrap();
        assert_eq!(ready["policyRevision"], enabled.policy_revision);
        assert_eq!(ready["worker"]["botId"], "worker");
        assert_eq!(ready["coordinator"]["botId"], "router");
        assert_eq!(ready["worker"]["eligibleAtSnapshot"], true);
        assert_eq!(ready["admitted"], false);
        assert_eq!(ready["allowedTools"], serde_json::json!([]));
        let tickets: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_tickets", [], |r| r.get(0)).unwrap();
        let mandates: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_mandates", [], |r| r.get(0)).unwrap();
        assert_eq!((tickets, mandates), (0, 0));
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        let paused: serde_json::Value = serde_json::from_str(
            &inspect_headless_team_readiness(&store.path, &cfg.id).unwrap()
        ).unwrap();
        assert_eq!(paused["worker"]["eligibleAtSnapshot"], false);
        assert_eq!(paused["coordinator"]["eligibleAtSnapshot"], false);
    }

    #[test]
    fn headless_recovery_inbox_prioritizes_uncertain_effects_and_preserves_history() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (running, launched) = admit(&mut store, &cfg.id,
            "Private remote delivery", "recovery-inbox-effect", false).unwrap();
        assert!(launched);
        let (blocked, dispatched) = admit(&mut store, &cfg.id,
            "Private next demand", "recovery-inbox-blocked", true).unwrap();
        assert!(!dispatched);
        let (settled, dispatched) = admit(&mut store, &cfg.id,
            "Private stopped demand", "recovery-inbox-settled", true).unwrap();
        assert!(!dispatched);
        stop(&store, &settled.id).unwrap();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'original-remote-write',
                    '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    'pending',?2,?2)",
            params![&running.id,storage::now()],
        ).unwrap();
        stop(&store, &running.id).unwrap();
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0)
        ).unwrap();
        let inbox: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        let items = inbox["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["ticketId"], running.id);
        assert_eq!(items[0]["recovery"]["uncertainExternalEffects"], 1);
        assert_eq!(items[0]["recovery"]["nextOperatorAction"],
            "reconcile_authoritative_remote_effects");
        assert_eq!(items[1]["ticketId"], blocked.id);
        assert_eq!(items[1]["state"], "blocked");
        let cursor_at = items[0]["updatedAt"].as_i64().unwrap();
        let cursor_id = items[0]["ticketId"].as_str().unwrap();
        let continuation: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox_page(
                &store.path, Some(0), Some(cursor_at), Some(cursor_id),
            ).unwrap(),
        ).unwrap();
        assert_eq!(continuation["items"].as_array().unwrap().len(), 1);
        assert_eq!(continuation["items"][0]["ticketId"], blocked.id);
        assert!(list_headless_recovery_inbox_page(
            &store.path, Some(0), Some(cursor_at), Some("forged-ticket"),
        ).is_err());
        assert!(list_headless_recovery_inbox_page(
            &store.path, Some(1), Some(cursor_at), Some(cursor_id),
        ).is_err());
        assert!(list_headless_recovery_inbox_page(
            &store.path, Some(0), Some(cursor_at + 1), Some(cursor_id),
        ).is_err());
        assert!(!items.iter().any(|item| item["ticketId"] == settled.id));
        assert_eq!(inbox["observationalOnly"], true);
        assert_eq!(inbox["automaticRecoveryPerformed"], false);
        assert_eq!(inbox["requiresCurrentNativeRecheckBeforeAnyAction"], true);
        assert!(!inbox["mayHaveMore"].as_bool().unwrap());
        let encoded = serde_json::to_string(&inbox).unwrap();
        for secret in ["Private remote delivery", "Private next demand",
            "Private stopped demand", "original-remote-write"] {
            assert!(!encoded.contains(secret), "Recovery inbox leaked protected content");
        }
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get::<_, i64>(0)
        ).unwrap(), before);
        assert_eq!(get_ticket(&store, &running.id).unwrap().state, "stopped");
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
            [&running.id], |row| row.get::<_, String>(0)
        ).unwrap(), "uncertain");
        let path = store.path.clone();
        drop(store);
        let again: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap()
        ).unwrap();
        assert_eq!(again["items"].as_array().unwrap().len(), 2);
        assert_eq!(again["items"][0]["ticketId"], running.id);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute("UPDATE sa_effect_receipts SET state='applied' WHERE ticket_id=?1",
            [&running.id]).unwrap();
        drop(db);
        // A stale cursor cannot silently skip a Ticket that stopped qualifying.
        assert!(list_headless_recovery_inbox_page(
            &path, Some(0), Some(cursor_at), Some(cursor_id),
        ).is_err());
    }

    #[test]
    fn headless_pending_effect_quarantine_is_exact_atomic_idempotent_and_restart_safe() {
        let mut store = fixture();
        let team = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &team.id, "Inspect unknown remote effect",
            "headless-pending-effect-quarantine", false,
        ).unwrap();
        assert!(launched);
        let revision = get_ticket(&store, &ticket.id).unwrap().revision;
        let digest = "a".repeat(64);
        let at = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'github:comment:pending',?2,'pending',?3,?3)",
            params![&ticket.id,&digest,at],
        ).unwrap();
        let path = store.path.clone();
        let quarantine = |revision, digest: &str| headless_quarantine_pending_effect(
            &path, &ticket.id, revision, "github:comment:pending", digest,
        );
        assert!(quarantine(revision+1, &digest).is_err());
        assert!(quarantine(revision, &"b".repeat(64)).is_err());
        assert!(headless_quarantine_pending_effect(
            &path, &ticket.id, revision, "../other", &digest,
        ).is_err());
        assert_eq!(effects::receipt_history(&store,&ticket.id,5).unwrap()[0].state,"pending");
        store.db.execute_batch(
            "CREATE TRIGGER deny_effect_quarantine_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='headless_effect_quarantined'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
        ).unwrap();
        assert!(quarantine(revision, &digest).is_err());
        assert_eq!(effects::receipt_history(&store,&ticket.id,5).unwrap()[0].state,"pending");
        store.db.execute_batch("DROP TRIGGER deny_effect_quarantine_audit").unwrap();

        let first: serde_json::Value = serde_json::from_str(
            &quarantine(revision, &digest).unwrap(),
        ).unwrap();
        assert_eq!(first["state"],"uncertain");
        assert_eq!(first["changed"],true);
        assert_eq!(first["remoteResultVerified"],false);
        assert_eq!(first["externalEffectReplayAllowed"],false);
        let repeat: serde_json::Value = serde_json::from_str(
            &quarantine(revision, &digest).unwrap(),
        ).unwrap();
        assert_eq!(repeat["changed"],false);
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().revision, revision);
        assert_eq!(history(&store,&ticket.id,30).unwrap().iter()
            .filter(|audit|audit.kind=="headless_effect_quarantined").count(),1);
        assert_eq!(effects::receipt_history(&store,&ticket.id,5).unwrap()[0].state,"uncertain");
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id=?1 AND effect_key='github:comment:pending'",
            [&ticket.id],
        ).unwrap();
        assert!(quarantine(revision, &digest).is_err(),
            "a terminal applied receipt must not be demoted");
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(effects::receipt_history(&reopened,&ticket.id,5).unwrap()[0].state,"applied");
        assert_eq!(history(&reopened,&ticket.id,30).unwrap().iter()
            .filter(|audit|audit.kind=="headless_effect_quarantined").count(),1);
    }

    #[test]
    fn headless_detached_quarantine_is_revisioned_atomic_and_restart_safe() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket,launched)=admit(&mut store,&cfg.id,"Private detached objective",
            "headless-detached-quarantine",false).unwrap();
        assert!(launched);
        let session=assignment_session(&store,&ticket.id).unwrap().unwrap();
        let revision=get_ticket(&store,&ticket.id).unwrap().revision;
        assert!(headless_quarantine_detached_ticket(&store.path,&ticket.id,revision).is_err());
        store.db.execute(
            "UPDATE sa_assignments SET session_id='missing-native-session'
             WHERE ticket_id=?1 AND state='running'",[&ticket.id],
        ).unwrap();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'detached-write','private-digest','pending',?2,?2)",
            params![&ticket.id,now],
        ).unwrap();
        assert!(headless_quarantine_detached_ticket(&store.path,&ticket.id,revision+1).is_err());
        store.db.execute_batch(
            "CREATE TRIGGER deny_detached_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='headless_detached_quarantined'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
        ).unwrap();
        assert!(headless_quarantine_detached_ticket(&store.path,&ticket.id,revision).is_err());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"running");
        assert!(!mandate(&store,&ticket.id).unwrap().revoked);
        assert_eq!(assignments(&store,&ticket.id,10).unwrap()[0].state,"running");
        store.db.execute_batch("DROP TRIGGER deny_detached_audit").unwrap();
        let output:serde_json::Value=serde_json::from_str(
            &headless_quarantine_detached_ticket(&store.path,&ticket.id,revision).unwrap(),
        ).unwrap();
        assert_eq!(output["state"],"interrupted");
        assert_eq!(output["pendingEffectsMarkedUncertain"],1);
        assert_eq!(output["providerStoppedByCommand"],false);
        assert_eq!(output["externalEffectReplayAllowed"],false);
        assert!(headless_quarantine_detached_ticket(&store.path,&ticket.id,revision).is_err());
        assert!(!on_run_finished(&store,&session,true,"Late evidence").unwrap());
        assert_eq!(history(&store,&ticket.id,20).unwrap().iter()
            .filter(|a|a.kind=="headless_detached_quarantined").count(),1);
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        let state=get_ticket(&reopened,&ticket.id).unwrap();
        assert_eq!(state.state,"interrupted");
        assert!(state.active_assignment_id.is_none());
        assert!(mandate(&reopened,&ticket.id).unwrap().revoked);
        assert_eq!(assignments(&reopened,&ticket.id,10).unwrap()[0].state,"interrupted");
        assert_eq!(reopened.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
            [&ticket.id],|r|r.get::<_,String>(0),
        ).unwrap(),"uncertain");
        assert!(!list_headless_recovery_inbox(&path).unwrap().contains("Private detached objective"));
    }

    #[test]
    fn headless_terminal_orphan_quarantine_is_atomic_revisioned_and_restart_safe() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(&mut store, &cfg.id,
            "Private objective", "headless-orphan-quarantine", false).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let original = get_ticket(&store, &ticket.id).unwrap().revision;
        assert!(headless_quarantine_terminal_orphan(
            &store.path, &ticket.id, original,
        ).is_err(), "Running Ticket cannot be quarantined");
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'unverified-effect','private-digest','pending',?2,?2)",
            params![&ticket.id,now],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_tickets SET state='completed',revision=revision+1,
             evidence='Original evidence' WHERE id=?1", [&ticket.id],
        ).unwrap();
        let revision = get_ticket(&store, &ticket.id).unwrap().revision;
        assert!(headless_quarantine_terminal_orphan(
            &store.path, &ticket.id, revision - 1,
        ).is_err(), "Stale revision must deny repair");
        store.db.execute_batch(
            "CREATE TRIGGER deny_quarantine_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='headless_orphan_quarantined'
             BEGIN SELECT RAISE(ABORT,'injected audit error'); END;",
        ).unwrap();
        assert!(headless_quarantine_terminal_orphan(
            &store.path, &ticket.id, revision,
        ).is_err(), "Audit failure must roll back the quarantine");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, revision);
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "running");
        store.db.execute_batch("DROP TRIGGER deny_quarantine_audit").unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &headless_quarantine_terminal_orphan(
                &store.path, &ticket.id, revision,
            ).unwrap(),
        ).unwrap();
        assert_eq!(json["state"], "completed");
        assert_eq!(json["quarantinedAssignments"], 1);
        assert_eq!(json["pendingEffectsMarkedUncertain"], 1);
        assert_eq!(json["externalEffectReplayAllowed"], false);
        assert!(headless_quarantine_terminal_orphan(
            &store.path, &ticket.id, revision,
        ).is_err());
        assert!(headless_quarantine_terminal_orphan(
            &store.path, &ticket.id, revision + 1,
        ).is_err());
        assert_eq!(history(&store, &ticket.id, 20).unwrap().iter()
            .filter(|e| e.kind == "headless_orphan_quarantined").count(), 1);
        assert!(!on_run_finished(&store, &session, true, "Late provider result").unwrap());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        let terminal = get_ticket(&reopened, &ticket.id).unwrap();
        assert_eq!(terminal.state, "completed");
        assert_eq!(terminal.evidence.as_deref(), Some("Original evidence"));
        assert!(terminal.active_assignment_id.is_none());
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&reopened, &ticket.id, 10).unwrap()[0].state, "interrupted");
        assert_eq!(reopened.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
            [&ticket.id], |r| r.get::<_, String>(0),
        ).unwrap(), "uncertain");
    }

    #[test]
    fn headless_expired_quarantine_is_revisioned_denies_healthy_and_never_replays() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private expired work", "headless-expired-native", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let path = store.path.clone();
        assert!(headless_quarantine_expired_ticket(&path, &ticket.id, ticket.revision).is_err(),
            "A healthy running provider must not be quarantined");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        let expired_at = storage::now() - MAX_ASSIGNMENT_RUN_MS - 1;
        store.db.execute(
            "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1",
            params![&ticket.id,expired_at],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'external-expired',?2,'pending',?3,?3)",
            params![&ticket.id,"a".repeat(64),storage::now()],
        ).unwrap();
        assert!(headless_quarantine_expired_ticket(&path, &ticket.id,
            ticket.revision + 1).is_err(), "Stale or forged revision must not mutate state");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        let raw = headless_quarantine_expired_ticket(
            &path, &ticket.id, ticket.revision,
        ).unwrap();
        let result: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(result["revision"], ticket.revision + 1);
        assert_eq!(result["state"], "interrupted");
        assert_eq!(result["pendingEffectsMarkedUncertain"], 1);
        assert_eq!(result["providerStoppedByCommand"], false);
        assert_eq!(result["externalEffectReplayAllowed"], false);
        assert!(!raw.contains("Private expired work"));
        assert!(!raw.contains(&session));
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert!(!on_run_finished(&store, &session, true, "Late provider success").unwrap());
        assert!(headless_quarantine_expired_ticket(&path, &ticket.id,
            ticket.revision).is_err(), "Duplicate quarantine must not add another audit entry");
        assert_eq!(history(&store, &ticket.id, 20).unwrap()
            .iter().filter(|a| a.kind == "headless_expired_quarantined").count(), 1);
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "interrupted");
        assert_eq!(effects::unresolved_count(&reopened, &ticket.id).unwrap(), 1);
        assert_eq!(effects::receipt_history(&reopened, &ticket.id, 10).unwrap()[0].state,
            "uncertain");
    }

    #[test]
    fn headless_recovery_inbox_flags_terminal_ticket_with_orphan_running_assignment() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(&mut store, &cfg.id,
            "Private output", "recovery-orphan-terminal", false).unwrap();
        assert!(launched);
        let healthy: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        assert!(healthy["items"].as_array().unwrap().is_empty());
        // Simulate a partial/legacy state transition. Normal native stop
        // would settle the assignment in the same transaction.
        store.db.execute(
            "UPDATE sa_tickets SET state='completed',revision=revision+1 WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        let before_audit: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let result: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
        assert_eq!(result["items"][0]["ticketId"], ticket.id);
        let recovery = &result["items"][0]["recovery"];
        assert_eq!(recovery["terminal"], true);
        assert_eq!(recovery["activeNativeAssignments"], 1);
        assert_eq!(recovery["assignmentStateConsistent"], false);
        assert_eq!(recovery["nextOperatorAction"], "investigate_native_assignment_state");
        assert_eq!(result["automaticRecoveryPerformed"], false);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "completed");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, i64>(0),
        ).unwrap(), before_audit);
        let path = store.path.clone();
        drop(store);
        // Read-only restart must preserve the anomaly for investigation,
        // not run Store::open recovery and erase evidence prematurely.
        let opened = Store::open_read_only(&path).unwrap();
        assert_eq!(get_ticket(&opened, &ticket.id).unwrap().state, "completed");
        let again: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap()
        ).unwrap();
        assert_eq!(again["items"][0]["recovery"]["assignmentStateConsistent"], false);
        assert_eq!(again["items"][0]["recovery"]["nextOperatorAction"],
            "investigate_native_assignment_state");
    }


    #[test]
    fn headless_recovery_contains_terminal_orphans_before_reconciling_remote_effects() {
        for initial_state in ["pending", "uncertain"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &cfg.id, "Private remote operation",
                "terminal-orphan-and-effect", false,
            ).unwrap();
            assert!(launched);
            // Simulate an interrupted legacy settlement: the Ticket became
            // terminal, but an Assignment/provider attempt was left running.
            store.db.execute(
                "UPDATE sa_tickets SET state='completed',revision=revision+1
                 WHERE id=?1", [&ticket.id],
            ).unwrap();
            let now = storage::now();
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,'remote-effect',?2,?3,?4,?4)",
                params![&ticket.id, "a".repeat(64), initial_state, now],
            ).unwrap();
            let revision = get_ticket(&store, &ticket.id).unwrap().revision;
            let audit_before = history(&store, &ticket.id, 30).unwrap().len();
            let path = store.path.clone();

            let before: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            let recovery = &before["items"][0]["recovery"];
            assert_eq!(before["items"].as_array().unwrap().len(), 1);
            assert_eq!(before["items"][0]["ticketId"], ticket.id);
            assert_eq!(recovery["activeNativeAssignments"], 1);
            assert_eq!(recovery["assignmentStateConsistent"], false);
            assert_eq!(recovery["requiresLiveHostVerification"], true);
            assert_eq!(recovery["requiresIndependentRemoteReconciliation"], true);
            assert_eq!(recovery["nextOperatorAction"],
                "investigate_native_assignment_state",
                "Do not reconcile while a terminal Ticket has an orphan provider: {initial_state}");
            assert_eq!(before["automaticRecoveryPerformed"], false);
            assert_eq!(history(&store, &ticket.id, 30).unwrap().len(), audit_before);
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, revision);

            // The existing local-operator quarantine is a separately
            // revision-checked action, not a side effect of listing.
            let quarantined: serde_json::Value = serde_json::from_str(
                &headless_quarantine_terminal_orphan(
                    &path, &ticket.id, revision,
                ).unwrap(),
            ).unwrap();
            assert_eq!(quarantined["quarantinedAssignments"], 1);
            assert_eq!(quarantined["providerStoppedByCommand"], false);
            assert_eq!(quarantined["externalEffectReplayAllowed"], false);
            let after: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(after["items"][0]["recovery"]["activeNativeAssignments"], 0);
            assert_eq!(after["items"][0]["recovery"]["uncertainExternalEffects"], 1);
            assert_eq!(after["items"][0]["recovery"]["nextOperatorAction"],
                "reconcile_authoritative_remote_effects");
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "completed");
            drop(store);
            let reopened = Store::open_read_only(&path).unwrap();
            assert_eq!(effects::receipt_history(&reopened, &ticket.id, 1)
                .unwrap()[0].state, "uncertain");
            assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "completed");
        }
    }

    #[test]
    fn headless_recovery_inbox_finds_detached_live_provider_session_before_watchdog() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private detached session objective",
            "recovery-missing-provider-session", false,
        ).unwrap();
        assert!(launched);
        let original_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let path = store.path.clone();
        let healthy: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap(),
        ).unwrap();
        assert!(healthy["items"].as_array().unwrap().is_empty());
        let audit_before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |r| r.get(0),
        ).unwrap();

        // Simulate a damaged/legacy assignment referring to a provider
        // session that is absent. Normal native admission never does this.
        store.db.execute(
            "UPDATE sa_assignments SET session_id='missing-native-session'
             WHERE ticket_id=?1 AND state='running'",
            [&ticket.id],
        ).unwrap();
        assert!(read_only_policy(&store, &original_session).is_err(),
            "detached task threads cannot fall back to ambient bot authority");
        let found: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap(),
        ).unwrap();
        assert_eq!(found["items"].as_array().unwrap().len(), 1);
        assert_eq!(found["items"][0]["ticketId"], ticket.id);
        let recovery = &found["items"][0]["recovery"];
        assert_eq!(recovery["activeNativeAssignments"], 1);
        assert_eq!(recovery["missingNativeSessions"], 1);
        assert_eq!(recovery["activeAssignmentLinked"], true);
        assert_eq!(recovery["assignmentStateConsistent"], false);
        assert_eq!(recovery["nextOperatorAction"], "investigate_native_assignment_state");
        assert_eq!(recovery["automaticReplayAllowed"], false);
        assert_eq!(found["automaticRecoveryPerformed"], false);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |r| r.get::<_, i64>(0),
        ).unwrap(), audit_before);
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "running");
        let again: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap(),
        ).unwrap();
        assert_eq!(again["items"][0]["recovery"]["missingNativeSessions"], 1);
        assert!(!again.to_string().contains("Private detached session objective"));
    }

    #[test]
    fn headless_recovery_inbox_detects_expired_live_attempt_without_interrupting_it() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(&mut store, &cfg.id,
            "Private running objective", "recovery-inbox-budget", false).unwrap();
        assert!(launched);
        let initial: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        assert!(initial["items"].as_array().unwrap().is_empty(),
            "A healthy active provider is not an abandoned recovery item");
        store.db.execute(
            "UPDATE sa_assignments SET created_at=1 WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        let overdue: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        assert_eq!(overdue["items"][0]["ticketId"], ticket.id);
        assert_eq!(overdue["items"][0]["state"], "running");
        assert_eq!(overdue["items"][0]["recovery"]["expiredNativeAssignments"], 1);
        assert_eq!(overdue["items"][0]["recovery"]["nextOperatorAction"],
            "investigate_expired_native_assignment_before_retry");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_assignments WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, String>(0)
        ).unwrap(), "running");
    }

    #[test]
    fn headless_recovery_inbox_detects_detached_task_thread_before_watchdog() {
        for fault in ["missing_thread", "remapped_identity"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &cfg.id, "Private detached task thread objective",
                "recovery-task-thread", false,
            ).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(matches!(read_only_policy(&store, &session), Ok(Some(_))));
            let path = store.path.clone();
            let before: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert!(before["items"].as_array().unwrap().is_empty(), "{fault}");
            let audit_count = history(&store, &ticket.id, 100).unwrap().len();

            let changed = match fault {
                "missing_thread" => store.db.execute(
                    "DELETE FROM bot_threads WHERE session_id=?1 AND channel_id=?2",
                    params![&session, CHANNEL],
                ).unwrap(),
                "remapped_identity" => store.db.execute(
                    "UPDATE bot_threads SET external_id='unrelated-task'
                     WHERE session_id=?1 AND channel_id=?2",
                    params![&session, CHANNEL],
                ).unwrap(),
                _ => unreachable!(),
            };
            assert_eq!(changed, 1, "{fault}");
            assert!(read_only_policy(&store, &session).is_err(), "{fault}");

            let found: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(found["items"].as_array().unwrap().len(), 1, "{fault}");
            assert_eq!(found["items"][0]["ticketId"], ticket.id, "{fault}");
            assert_eq!(found["items"][0]["recovery"]["assignmentStateConsistent"], true);
            assert_eq!(found["items"][0]["recovery"]["activeNativeReadOnlyAdmissionValid"], false);
            assert_eq!(found["items"][0]["recovery"]["nextOperatorAction"],
                "stop_or_quarantine_revoked_native_execution");
            assert_eq!(found["items"][0]["recovery"]["automaticReplayAllowed"], false);
            assert_eq!(found["automaticRecoveryPerformed"], false);
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
            assert_eq!(history(&store, &ticket.id, 100).unwrap().len(), audit_count);
            drop(store);

            let reopened = Store::open_read_only(&path).unwrap();
            assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "running");
            let again: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(again["items"][0]["ticketId"], ticket.id);
            assert!(!again.to_string().contains("Private detached task thread objective"));
        }
    }

    #[test]
    fn headless_recovery_inbox_surfaces_native_admission_drift_without_replay() {
        for fault in ["team_disabled", "coordinator_paused", "worker_paused",
                      "policy_revision", "unsupported_tools"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(&mut store, &cfg.id,
                "Private drift recovery objective", "drift-recovery", false).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(matches!(read_only_policy(&store, &session), Ok(Some(_))));
            let path = store.path.clone();
            let before: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert!(before["items"].as_array().unwrap().is_empty(), "{fault}");
            let audit_before = history(&store, &ticket.id, 100).unwrap().len();

            match fault {
                "team_disabled" => {
                    store.db.execute("UPDATE sa_orchestrations SET enabled=0 WHERE id=?1",
                        [&cfg.id]).unwrap();
                }
                "coordinator_paused" => {
                    store.db.execute("UPDATE bots SET paused=1 WHERE id=?1",
                        [&cfg.coordinator_bot_id]).unwrap();
                }
                "worker_paused" => {
                    store.db.execute("UPDATE bots SET paused=1 WHERE id=?1",
                        [&cfg.worker_bot_id]).unwrap();
                }
                "policy_revision" => {
                    store.db.execute(
                        "UPDATE sa_orchestrations SET policy_revision=policy_revision+1 WHERE id=?1",
                        [&cfg.id],
                    ).unwrap();
                }
                "unsupported_tools" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET allowed_tools='[\"shell\"]' WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(read_only_policy(&store, &session).is_err(), "{fault}");

            // An uncertain remote operation must not outrank containment of
            // a now-unauthorized running native provider.
            if fault == "coordinator_paused" {
                store.db.execute(
                    "INSERT INTO sa_effect_receipts
                     (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                     VALUES(?1,'github:comment:pending',?2,'uncertain',?3,?3)",
                    params![&ticket.id, "a".repeat(64), storage::now()],
                ).unwrap();
            }
            let found: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(found["items"].as_array().unwrap().len(), 1, "{fault}");
            assert_eq!(found["items"][0]["ticketId"], ticket.id, "{fault}");
            assert_eq!(found["items"][0]["recovery"]["assignmentStateConsistent"], true, "{fault}");
            assert_eq!(found["items"][0]["recovery"]["activeNativeReadOnlyAdmissionValid"], false, "{fault}");
            assert_eq!(found["items"][0]["recovery"]["nextOperatorAction"],
                "stop_or_quarantine_revoked_native_execution", "{fault}");
            assert_eq!(found["items"][0]["recovery"]["automaticReplayAllowed"], false);
            assert_eq!(found["automaticRecoveryPerformed"], false);
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
            assert_eq!(history(&store, &ticket.id, 100).unwrap().len(), audit_before);
            drop(store);
            let reopened = Store::open_read_only(&path).unwrap();
            assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "running");
            let again: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(again["items"][0]["recovery"]["activeNativeReadOnlyAdmissionValid"], false);
            assert!(!again.to_string().contains("Private drift recovery objective"));
        }
    }

    #[test]
    fn headless_recovery_inbox_finds_revoked_and_expired_live_mandates_before_watchdog() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private native mandate recovery",
            "mandate-recovery-inbox", false,
        ).unwrap();
        assert!(launched);
        let path = store.path.clone();
        let initial: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap(),
        ).unwrap();
        assert!(initial["items"].as_array().unwrap().is_empty());

        let audit_before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        for fault in ["revoked", "expired"] {
            store.db.execute(
                "UPDATE sa_mandates SET revoked=?2,expires_at=?3 WHERE ticket_id=?1",
                params![ticket.id, i64::from(fault == "revoked"),
                    if fault == "expired" { storage::now() - 1 }
                    else { storage::now() + 120_000 }],
            ).unwrap();
            let found: serde_json::Value = serde_json::from_str(
                &list_headless_recovery_inbox(&path).unwrap(),
            ).unwrap();
            assert_eq!(found["items"].as_array().unwrap().len(), 1, "{fault}");
            assert_eq!(found["items"][0]["ticketId"], ticket.id, "{fault}");
            assert_eq!(found["items"][0]["state"], "running", "{fault}");
            assert_eq!(found["items"][0]["recovery"]["nextOperatorAction"],
                "stop_or_quarantine_revoked_native_execution", "{fault}");
            assert_eq!(found["items"][0]["recovery"]["automaticReplayAllowed"], false);
            assert_eq!(found["items"][0]["recovery"]["mandateRevoked"],
                fault == "revoked");
            assert_eq!(found["items"][0]["recovery"]["mandateExpired"],
                fault == "expired");
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
            assert_eq!(store.db.query_row(
                "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
                [&ticket.id], |row| row.get::<_, i64>(0),
            ).unwrap(), audit_before);
        }
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        let after_restart: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&path).unwrap(),
        ).unwrap();
        assert_eq!(after_restart["items"][0]["ticketId"], ticket.id);
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "running");
        assert!(!serde_json::to_string(&after_restart).unwrap()
            .contains("Private native mandate recovery"));
    }

    #[test]
    fn headless_team_recovery_inbox_filters_before_pagination_and_rejects_foreign_cursors() {
        let mut store = fixture();
        let first = enabled(&store);
        let second = create_configuration(&store, NewOrchestration {
            name: "Other recovery Team".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&second.id],
        ).unwrap();
        let second = configuration(&store, &second.id).unwrap();
        let (private, launched) = admit(
            &mut store, &first.id, "Private first-Team recovery objective",
            "first-team-recovery", true,
        ).unwrap();
        assert!(!launched);
        store.db.execute(
            "UPDATE sa_tickets SET updated_at=updated_at-30000 WHERE id=?1",
            [&private.id],
        ).unwrap();
        for index in 0..201 {
            let (ticket, launched) = admit(
                &mut store, &second.id, "Private second-Team recovery objective",
                &format!("second-team-recovery-{index}"), true,
            ).unwrap();
            assert_eq!(ticket.state, "blocked");
            assert!(!launched);
        }
        let before_audit: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        let first_page: serde_json::Value = serde_json::from_str(
            &list_headless_team_recovery_inbox_page(
                &store.path, &first.id, None, None, None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(first_page["orchestrationId"], first.id);
        assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
        assert_eq!(first_page["items"][0]["ticketId"], private.id);
        assert_eq!(first_page["mayHaveMore"], false);
        let second_page: serde_json::Value = serde_json::from_str(
            &list_headless_team_recovery_inbox_page(
                &store.path, &second.id, None, None, None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(second_page["items"].as_array().unwrap().len(), 200);
        assert_eq!(second_page["mayHaveMore"], true);
        let cursor = &second_page["nextCursor"];
        let older: serde_json::Value = serde_json::from_str(
            &list_headless_team_recovery_inbox_page(
                &store.path, &second.id, cursor["priority"].as_i64(),
                cursor["updatedAt"].as_i64(), cursor["ticketId"].as_str(),
            ).unwrap(),
        ).unwrap();
        assert_eq!(older["items"].as_array().unwrap().len(), 1);
        assert_eq!(older["mayHaveMore"], false);
        assert!(list_headless_team_recovery_inbox_page(
            &store.path, &first.id, cursor["priority"].as_i64(),
            cursor["updatedAt"].as_i64(), cursor["ticketId"].as_str(),
        ).is_err(), "A cursor from another Team must not authorize cross-Team pages");
        assert!(list_headless_team_recovery_inbox_page(
            &store.path, "unknown-team", None, None, None,
        ).is_err());
        let encoded = format!("{first_page}{second_page}{older}");
        assert!(!encoded.contains("Private first-Team"));
        assert!(!encoded.contains("Private second-Team"));
        assert_eq!(first_page["observationalOnly"], true);
        assert_eq!(second_page["automaticRecoveryPerformed"], false);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get::<_, i64>(0),
        ).unwrap(), before_audit);
        let database = store.path.clone();
        drop(store);
        let after_restart: serde_json::Value = serde_json::from_str(
            &list_headless_team_recovery_inbox_page(
                &database, &first.id, None, None, None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(after_restart["items"][0]["ticketId"], private.id);
    }

    #[test]
    fn headless_recovery_inbox_is_bounded_across_many_unsettled_demands() {
        let mut store = fixture();
        let cfg = enabled(&store);
        for n in 0..201 {
            let (ticket, launched) = admit(&mut store, &cfg.id,
                "Private batch objective", &format!("recovery-page-{n}"), true).unwrap();
            assert_eq!(ticket.state, "blocked");
            assert!(!launched);
        }
        let inbox: serde_json::Value = serde_json::from_str(
            &list_headless_recovery_inbox(&store.path).unwrap()
        ).unwrap();
        assert_eq!(inbox["items"].as_array().unwrap().len(), 200);
        assert_eq!(inbox["mayHaveMore"], true);
        assert_eq!(inbox["pageLimit"], 200);
        let cursor=&inbox["nextCursor"];
        assert_eq!(cursor["priority"],2);
        let second:serde_json::Value=serde_json::from_str(
            &list_headless_recovery_inbox_page(
                &store.path,cursor["priority"].as_i64(),
                cursor["updatedAt"].as_i64(),cursor["ticketId"].as_str(),
            ).unwrap(),
        ).unwrap();
        assert_eq!(second["items"].as_array().unwrap().len(),1);
        assert_eq!(second["mayHaveMore"],false);
        assert!(second["nextCursor"].is_null());
        let first_ids:std::collections::HashSet<&str>=inbox["items"].as_array().unwrap()
            .iter().map(|item|item["ticketId"].as_str().unwrap()).collect();
        assert!(!first_ids.contains(second["items"][0]["ticketId"].as_str().unwrap()));
        assert!(list_headless_recovery_inbox_page(&store.path,Some(3),Some(1),Some("valid-id")).is_err());
        assert!(list_headless_recovery_inbox_page(&store.path,Some(2),Some(1),Some("../escape")).is_err());
        assert!(list_headless_recovery_inbox_page(&store.path,Some(2),None,Some("valid-id")).is_err());
        assert!(!serde_json::to_string(&inbox).unwrap()
            .contains("Private batch objective"));
        assert!(!serde_json::to_string(&second).unwrap()
            .contains("Private batch objective"));
    }

    #[test]
    fn headless_recovery_inspection_is_read_only_and_never_replays_uncertain_effects() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private recovery objective",
            "headless-recovery-inspection", false,
        ).unwrap();
        assert!(launched);
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let running: serde_json::Value = serde_json::from_str(
            &inspect_headless_status(&store.path, Some(&ticket.id)).unwrap(),
        ).unwrap();
        assert_eq!(running["state"], "running");
        assert_eq!(running["recovery"]["activeNativeAssignments"], 1);
        assert_eq!(running["recovery"]["assignmentStateConsistent"], true);
        assert_eq!(running["recovery"]["nextOperatorAction"],
            "verify_live_serve_provider_before_intervention");
        assert_eq!(running["recovery"]["automaticReplayAllowed"], false);
        assert_eq!(running["recovery"]["observationalOnly"], true);
        assert!(!serde_json::to_string(&running).unwrap().contains("Private recovery objective"));
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running",
            "A parallel read-only inspector must not invoke startup recovery");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, i64>(0),
        ).unwrap(), before);

        // Simulate a durable external request whose provider outcome is unknown.
        // Inspection may report it, but may not acknowledge, resend or drop it.
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'remote-effect','0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    'pending',?2,?2)",
            params![&ticket.id, storage::now()],
        ).unwrap();
        let pending: serde_json::Value = serde_json::from_str(
            &inspect_headless_status(&store.path, Some(&ticket.id)).unwrap(),
        ).unwrap();
        assert_eq!(pending["recovery"]["pendingExternalEffects"], 1);
        assert_eq!(pending["recovery"]["requiresIndependentRemoteReconciliation"], true);
        assert_eq!(pending["recovery"]["nextOperatorAction"],
            "reconcile_authoritative_remote_effects");
        assert_eq!(pending["recovery"]["automaticReplayAllowed"], false);
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, String>(0),
        ).unwrap(), "pending");

        stop(&store, &ticket.id).unwrap();
        let stopped: serde_json::Value = serde_json::from_str(
            &inspect_headless_status(&store.path, Some(&ticket.id)).unwrap(),
        ).unwrap();
        assert_eq!(stopped["state"], "stopped");
        assert_eq!(stopped["recovery"]["activeNativeAssignments"], 0);
        assert_eq!(stopped["recovery"]["uncertainExternalEffects"], 1);
        assert_eq!(stopped["recovery"]["mandateRevoked"], true);
        assert_eq!(stopped["recovery"]["nextOperatorAction"],
            "reconcile_authoritative_remote_effects");
        assert_eq!(stopped["recovery"]["automaticReplayAllowed"], false);
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, String>(0),
        ).unwrap(), "uncertain");
        assert!(inspect_headless_status(&store.path, Some("unknown-ticket")).is_err());
    }

    #[test]
    fn admitted_session_profile_drift_denies_provider_and_final_callback() {
        for (field, changed) in [
            ("provider", "claude"),
            ("model", "unauthorized-model"),
            ("mode", "ask"),
            ("reasoning_effort", "high"),
        ] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let key = format!("session-drift-{field}");
            let (ticket, started) = admit(
                &mut store, &cfg.id, "Session-bound inspection", &key, false,
            ).unwrap();
            assert!(started, "{field}");
            let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(read_only_policy(&store, &session_id).unwrap().is_some());
            // Only fixed test-owned SQL column names are used; model text
            // never has permission to change the production session directly.
            let change = format!("UPDATE sessions SET {field}=?2 WHERE id=?1");
            store.db.execute(&change, params![session_id, changed]).unwrap();
            assert!(read_only_policy(&store, &session_id).is_err(),
                "{field} drift must deny native provider/tool admission");
            assert!(on_run_finished(
                &store, &session_id, true, "Untrusted apparent completion",
            ).unwrap());
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked",
                "{field} drift must not become a human-approvable result");
            let grant = mandate(&store, &ticket.id).unwrap();
            assert!(grant.allowed_tools.is_empty() && !grant.allowed_side_effects);
        }
    }

    #[test]
    fn admitted_session_survives_bot_profile_edits_without_inheriting_permissions() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Frozen provider inspection", "frozen-session", false,
        ).unwrap();
        assert!(started);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let original = store.session(&session_id).unwrap();
        store.db.execute(
            "UPDATE bots SET provider='claude',model='other-model',mode='full',
             reasoning_effort='high' WHERE id='worker'",
            [],
        ).unwrap();
        assert!(read_only_policy(&store, &session_id).unwrap().is_some());
        let preserved = store.session(&session_id).unwrap();
        assert_eq!(preserved.provider, original.provider);
        assert_eq!(preserved.model, original.model);
        assert_eq!(preserved.mode, original.mode);
        assert_eq!(preserved.reasoning_effort, original.reasoning_effort);
        assert!(on_run_finished(&store, &session_id, true, "Read-only evidence").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
    }

    #[test]
    fn admitted_team_profiles_are_immutable_restart_safe_and_recheck_new_attempts() {
        let mut store=fixture();
        let cfg=enabled(&store);
        store.db.execute(
            "UPDATE bots SET skills='[\"reviewer-skill\"]',reasoning_effort='high'
             WHERE id='worker'",[],
        ).unwrap();
        let (ticket,started)=admit(&mut store,&cfg.id,"Private scope","profile-pin",true).unwrap();
        assert!(!started);
        let original=ticket_snapshot(&store,&ticket.id,10).unwrap()
            .admitted_profile.expect("new Tickets must capture profiles");
        assert_eq!(original.policy_revision,cfg.policy_revision);
        assert_eq!(original.members.len(),2);
        assert_eq!(original.members[1].role,"worker");
        assert_eq!(original.members[1].model,"default");
        assert_eq!(original.members[1].reasoning_effort,"high");
        assert_eq!(original.members[1].assigned_skill_ids,vec!["reviewer-skill"]);
        let audit_before:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",[&ticket.id],
            |r|r.get(0),
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET provider='claude',model='different',mode='full',
                reasoning_effort='low',skills='[]' WHERE id='worker'",[],
        ).unwrap();
        let replay=admit(&mut store,&cfg.id,"Private scope","profile-pin",true).unwrap();
        assert_eq!(replay.0.id,ticket.id);
        assert!(!replay.1);
        let pinned=ticket_snapshot(&store,&ticket.id,10).unwrap()
            .admitted_profile.unwrap();
        assert_eq!(pinned.members,original.members);
        let denied=retry_blocked(&mut store,&ticket.id,false).unwrap_err();
        assert!(denied.contains("changed after Demand admission"));
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"blocked");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",[&ticket.id],
            |r|r.get::<_,i64>(0),
        ).unwrap(),0);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",[&ticket.id],
            |r|r.get::<_,i64>(0),
        ).unwrap(),audit_before);
        let (fresh,_) = admit(&mut store,&cfg.id,"New private scope","fresh-profile",true).unwrap();
        let new_profile=ticket_snapshot(&store,&fresh.id,10).unwrap()
            .admitted_profile.unwrap();
        assert_eq!(new_profile.members[1].model,"different");
        assert_eq!(new_profile.members[1].assigned_skill_ids,Vec::<String>::new());
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(roster::ticket_profile(&reopened,&ticket.id).unwrap().unwrap().members,
            original.members);
        assert_eq!(roster::ticket_profile(&reopened,&fresh.id).unwrap().unwrap().members,
            new_profile.members);
    }

    #[test]
    fn v45_upgrade_preserves_historical_ticket_without_fabricating_profile() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (legacy,_) = admit(&mut store,&cfg.id,"Legacy demand","legacy-profile",true).unwrap();
        store.db.execute_batch(
            "DROP TABLE sa_ticket_profiles; PRAGMA user_version=45;",
        ).unwrap();
        let path=store.path.clone();
        drop(store);
        let mut migrated=Store::open(&path).unwrap();
        assert_eq!(get_ticket(&migrated,&legacy.id).unwrap().state,"blocked");
        assert!(ticket_snapshot(&migrated,&legacy.id,10).unwrap().admitted_profile.is_none());
        let (fresh,_) = admit(&mut migrated,&cfg.id,"New demand","migrated-profile",true).unwrap();
        assert!(ticket_snapshot(&migrated,&fresh.id,10).unwrap().admitted_profile.is_some());
        assert_eq!(migrated.db.pragma_query_value(None,"user_version",
            |r|r.get::<_,i64>(0)).unwrap(),storage::SCHEMA_VERSION);
    }

    #[test]
    fn native_ticket_snapshot_inspects_attempt_budget_and_revocation_without_grants() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Private fixture objective", "insight-1", false,
        ).unwrap();
        assert!(started);
        let first = ticket_snapshot(&store, &ticket.id, 50).unwrap();
        assert_eq!(first.attempt_insights.len(), 1);
        let insight = &first.attempt_insights[0];
        assert_eq!(insight.assignment_id, first.assignments[0].id);
        assert!(insight.current_assignment);
        assert!(!insight.mandate_revoked);
        assert!(!insight.has_result);
        assert!(!insight.run_budget_expired);
        assert!(insight.remaining_run_budget_ms.unwrap() > 0);
        let count_before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments", [], |r| r.get(0),
        ).unwrap();
        store.db.execute(
            "UPDATE sa_assignments SET created_at=?2, result='private provider result'
             WHERE id=?1",
            params![insight.assignment_id, storage::now() - MAX_ASSIGNMENT_RUN_MS - 1000],
        ).unwrap();
        let overdue = ticket_snapshot(&store, &ticket.id, 50).unwrap();
        assert!(overdue.attempt_insights[0].run_budget_expired);
        assert!(overdue.attempt_insights[0].has_result);
        assert_eq!(overdue.attempt_insights[0].remaining_run_budget_ms, Some(0));
        assert!(!serde_json::to_string(&overdue.attempt_insights).unwrap()
            .contains("private provider result"));
        assert_eq!(store.db.query_row("SELECT COUNT(*) FROM sa_assignments", [],
            |r| r.get::<_, i64>(0)).unwrap(), count_before);
        stop(&store, &ticket.id).unwrap();
        let stopped = ticket_snapshot(&store, &ticket.id, 50).unwrap();
        assert!(!stopped.attempt_insights[0].current_assignment);
        assert!(stopped.attempt_insights[0].mandate_revoked);
        assert_eq!(stopped.attempt_insights[0].remaining_run_budget_ms, None);
    }


    #[test]
    fn headless_run_graph_is_durable_redacted_and_never_recovers_a_live_task() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, admitted) = admit(
            &mut store, &config.id, "Private headless graph objective",
            "headless-graph-1", false,
        ).unwrap();
        assert!(admitted);
        let before_audit: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let before_mandates: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_mandates", [], |row| row.get(0),
        ).unwrap();
        let output = inspect_headless_ticket_graph(&store.path, &ticket.id).unwrap();
        let view: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(view["graph"]["ticketId"], ticket.id);
        assert_eq!(view["graph"]["ticketRevision"], ticket.revision);
        assert_eq!(view["graph"]["nodes"][0]["kind"], "ticket");
        assert_eq!(view["graph"]["nodes"][1]["kind"], "assignment");
        assert_eq!(view["graph"]["nodes"][1]["active"], true);
        assert_eq!(view["graph"]["edges"][0]["relation"], "owns_attempt");
        assert_eq!(view["observationalOnly"], true);
        assert_eq!(view["dynamicSubagentsRepresented"], false);
        assert_eq!(view["councilVerdictIncluded"], false);
        assert_eq!(view["providerDispatchedByCommand"], false);
        assert_eq!(view["recoveryPerformed"], false);
        for private in ["Private headless graph objective", "headless-graph-1",
            "authorizationId", "private provider result"] {
            assert!(!output.contains(private), "Graph disclosed {private}");
        }
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |row| row.get::<_, i64>(0),
        ).unwrap(), before_audit);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_mandates", [], |row| row.get::<_, i64>(0),
        ).unwrap(), before_mandates);
        let path = store.path.clone();
        drop(store);
        let reopened = inspect_headless_ticket_graph(&path, &ticket.id).unwrap();
        let after: serde_json::Value = serde_json::from_str(&reopened).unwrap();
        assert_eq!(after["graph"], view["graph"]);
        assert!(inspect_headless_ticket_graph(&path, "missing-ticket").is_err());
    }

    #[test]
    fn native_run_graph_uses_only_durable_ticket_attempt_edges_and_is_redacted() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(&mut store, &config.id,
            "Private graph objective", "graph-1", false).unwrap();
        assert!(launched);
        let graph = run_graph(&store, &ticket.id).unwrap();
        assert_eq!(graph.ticket_id, ticket.id);
        assert_eq!(graph.ticket_revision, ticket.revision);
        assert!(!graph.truncated_attempts);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].relation, "owns_attempt");
        assert_eq!(graph.edges[0].from, format!("ticket:{}", ticket.id));
        assert_eq!(graph.nodes[1].kind, "assignment");
        assert_eq!(graph.nodes[1].attempt, Some(1));
        assert!(graph.nodes[1].active);
        let serialized = serde_json::to_string(&graph).unwrap();
        assert!(!serialized.contains("Private graph objective"));
        assert!(!serialized.contains("authorizationId"));
        assert!(run_graph(&store, "another-ticket").is_err());
        stop(&store, &ticket.id).unwrap();
        let after = run_graph(&store, &ticket.id).unwrap();
        assert!(!after.nodes[1].active);
        assert_eq!(after.nodes[0].state, "stopped");
        let total: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |row| row.get(0)
        ).unwrap();
        assert_eq!(total,1,"Graph inspection must not launch another demand");
        // The native graph is bounded but must not claim truncation at
        // exactly 50 attempts. Once a 51st exists, preserve the latest 50.
        for attempt in 2..=50 {
            store.db.execute(
                "INSERT INTO sa_assignments
                 (id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
                 VALUES(?1,?2,'worker',?3,'finished',?4,?5,?5)",
                params![
                    format!("graph-assignment-{attempt}"), &ticket.id,
                    format!("graph-session-{attempt}"),attempt,storage::now(),
                ],
            ).unwrap();
        }
        let exactly_fifty = run_graph(&store, &ticket.id).unwrap();
        assert_eq!(exactly_fifty.nodes.len(), 51);
        assert!(!exactly_fifty.truncated_attempts);
        store.db.execute(
            "INSERT INTO sa_assignments
             (id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES('graph-assignment-51',?1,'worker','graph-session-51',
                    'finished',51,?2,?2)",
            params![&ticket.id, storage::now()],
        ).unwrap();
        let capped = run_graph(&store, &ticket.id).unwrap();
        assert_eq!(capped.nodes.len(),51);
        assert!(capped.truncated_attempts);
        assert_eq!(capped.nodes[1].attempt, Some(2));
        assert_eq!(capped.nodes[50].attempt, Some(51));
    }

    #[test]
    fn headless_revision_bound_stop_denies_stale_observation_without_revocation() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, admitted) = admit(
            &mut store, &cfg.id, "Revision-safe operator Stop", "headless-stop-cas", false,
        ).unwrap();
        assert!(admitted);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let audit_before = history(&store, &ticket.id, 20).unwrap().len();
        assert!(headless_stop_ticket_at_revision(
            &store.path, &ticket.id, ticket.revision + 1,
        ).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), audit_before);
        let result: serde_json::Value = serde_json::from_str(
            &headless_stop_ticket_at_revision(&store.path, &ticket.id, ticket.revision).unwrap(),
        ).unwrap();
        assert_eq!(result["state"], "stopped");
        assert_eq!(result["revision"], ticket.revision + 1);
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), audit_before + 1);
        assert!(headless_stop_ticket_at_revision(
            &store.path, &ticket.id, ticket.revision,
        ).is_err(), "a stale command may not repeat or revive Stop");
        assert!(!on_run_finished(&store, &session, true, "Late provider result").unwrap());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "stopped");
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
    }

    #[test]
    fn headless_stopped_ticket_cancels_only_its_live_native_provider_session() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, admitted) = admit(&mut store, &cfg.id,
            "Stop test in disposable workspace", "headless-stop-live", false).unwrap();
        assert!(admitted);
        let assigned = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let worker = bots::get(&store, "worker").unwrap();
        let ordinary = bots::thread_session(
            &store, &worker, "desktop", "unrelated-normal-chat",
            "Unrelated ordinary chat",
        ).unwrap().0;
        let active = vec![ordinary.clone(), assigned.clone(), assigned.clone()];
        assert!(headless_cancelled_assignments(&store, &active).unwrap().is_empty());
        // Simulates a one-shot headless control process writing revocation to
        // the same SQLite database while the owner retains its provider handle.
        let stopped = headless_stop_ticket(&store.path, &ticket.id).unwrap();
        assert!(stopped.contains("stopped"));
        let to_cancel = headless_cancelled_assignments(&store, &active).unwrap();
        assert_eq!(to_cancel, vec![assigned.clone()]);
        assert!(!to_cancel.contains(&ordinary));
        assert!(headless_cancelled_assignments(&store, &[]).unwrap().is_empty());
        assert_eq!(headless_cancelled_assignments(&store, &active).unwrap(), to_cancel);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "stopped");
        assert!(read_only_policy(&store, &assigned).is_err());
        assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
            &store, &ordinary,
        ).is_ok(), "normal bot paths cannot inherit task revocation");
    }

    #[test]
    fn headless_poll_fences_admission_profile_and_thread_drift_before_watchdog() {
        for scenario in ["thread-remapped", "model-changed", "checkout-moved"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &cfg.id, "Inspect native task authority",
                &format!("headless-exact-authority-{scenario}"), false,
            ).unwrap();
            assert!(launched, "{scenario}");
            let assigned = assignment_session(&store, &ticket.id).unwrap().unwrap();
            let worker = bots::get(&store, "worker").unwrap();
            let ordinary = bots::thread_session(
                &store, &worker, "desktop", "ordinary-exact-authority",
                "Ordinary bot chat",
            ).unwrap().0;
            let active = vec![ordinary.clone(), assigned.clone(), assigned.clone()];
            assert!(headless_cancelled_assignments_with_pause(
                &store, &active, false,
            ).unwrap().is_empty(), "{scenario}: original authority is valid");

            match scenario {
                "thread-remapped" => {
                    assert_eq!(store.db.execute(
                        "UPDATE bot_threads SET external_id='untrusted-ticket'
                         WHERE session_id=?1 AND channel_id=?2",
                        params![&assigned, CHANNEL],
                    ).unwrap(), 1);
                }
                "model-changed" => {
                    assert_eq!(store.db.execute(
                        "UPDATE sessions SET model='unapproved-model' WHERE id=?1",
                        [&assigned],
                    ).unwrap(), 1);
                }
                "checkout-moved" => {
                    assert_eq!(store.db.execute(
                        "UPDATE projects SET path='/tmp/disposable-superagent-moved'
                         WHERE id='project'",
                        [],
                    ).unwrap(), 1);
                }
                _ => unreachable!(),
            }
            assert!(read_only_policy(&store, &assigned).is_err(), "{scenario}");
            let to_cancel = headless_cancelled_assignments_with_pause(
                &store, &active, false,
            ).unwrap();
            assert_eq!(to_cancel, vec![assigned.clone()], "{scenario}");
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running",
                "polling may cancel a handle but cannot settle a Ticket");
            assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
                &store, &ordinary,
            ).is_ok(), "{scenario}: ordinary chat remains unaffected");
            let path = store.path.clone();
            drop(store);
            let reopened = Store::open_read_only(&path).unwrap();
            assert_eq!(headless_cancelled_assignments_with_pause(
                &reopened, &active, false,
            ).unwrap(), vec![assigned], "{scenario}: restart-safe native refusal");
        }
    }

    #[test]
    fn headless_poll_cancels_orphaned_or_revoked_running_native_sessions() {
        for scenario in ["detached", "revoked", "policy-drift", "blocked",
            "expired-mandate", "disabled-team", "paused-worker",
            "paused-coordinator", "worker-project-drift", "expired-assignment",
            "unexpected-tools"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, admitted) = admit(
                &mut store, &cfg.id, "Recover task-owned provider",
                "headless-authority-loss", false,
            ).unwrap();
            assert!(admitted, "{scenario}");
            let assigned = assignment_session(&store, &ticket.id).unwrap().unwrap();
            let worker = bots::get(&store, "worker").unwrap();
            let ordinary = bots::thread_session(
                &store, &worker, "desktop", "unrelated-normal-chat",
                "Unrelated ordinary chat",
            ).unwrap().0;
            let active = vec![ordinary.clone(), assigned.clone(), assigned.clone()];
            assert!(headless_cancelled_assignments(&store, &active).unwrap().is_empty());

            // Another local operator or recovery path may invalidate native
            // authority while the owning serve process retains a provider.
            match scenario {
                "detached" => store.db.execute(
                    "UPDATE sa_tickets SET active_assignment_id=NULL WHERE id=?1",
                    [&ticket.id],
                ).unwrap(),
                "revoked" => store.db.execute(
                    "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
                    [&ticket.id],
                ).unwrap(),
                "policy-drift" => store.db.execute(
                    "UPDATE sa_mandates SET policy_revision=policy_revision+1 WHERE ticket_id=?1",
                    [&ticket.id],
                ).unwrap(),
                "blocked" => store.db.execute(
                    "UPDATE sa_tickets SET state='blocked' WHERE id=?1",
                    [&ticket.id],
                ).unwrap(),
                "expired-mandate" => store.db.execute(
                    "UPDATE sa_mandates SET expires_at=?2 WHERE ticket_id=?1",
                    params![ticket.id, storage::now()-1],
                ).unwrap(),
                "disabled-team" => store.db.execute(
                    "UPDATE sa_orchestrations SET enabled=0 WHERE id=?1",
                    [&cfg.id],
                ).unwrap(),
                "paused-worker" => store.db.execute(
                    "UPDATE bots SET paused=1 WHERE id='worker'", [],
                ).unwrap(),
                "paused-coordinator" => store.db.execute(
                    "UPDATE bots SET paused=1 WHERE id='router'", [],
                ).unwrap(),
                "worker-project-drift" => {
                    store.db.execute(
                        "INSERT INTO projects(id,name,path)
                         VALUES('other-project','Other','/tmp/sa-worker-moved')", [],
                    ).unwrap();
                    store.db.execute(
                        "UPDATE bots SET project_id='other-project' WHERE id='worker'", [],
                    ).unwrap()
                },
                "expired-assignment" => store.db.execute(
                    "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1",
                    params![ticket.id, storage::now()-MAX_ASSIGNMENT_RUN_MS-1000],
                ).unwrap(),
                "unexpected-tools" => store.db.execute(
                    "UPDATE sa_mandates SET allowed_tools=?2 WHERE ticket_id=?1",
                    params![ticket.id, "[\"write\"]"],
                ).unwrap(),
                _ => unreachable!(),
            };
            assert!(read_only_policy(&store, &assigned).is_err(), "{scenario}");
            assert_eq!(
                headless_cancelled_assignments(&store, &active).unwrap(),
                vec![assigned.clone()],
                "{scenario}",
            );
            assert_eq!(
                headless_cancelled_assignments(&store, &active).unwrap(),
                vec![assigned],
                "watchdog polling is read-only and idempotent: {scenario}",
            );
            assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
                &store, &ordinary,
            ).is_ok(), "ordinary bot paths must not inherit task revocation: {scenario}");
        }
    }

    #[test]
    fn headless_poll_immediately_fences_settled_and_paused_task_handles_only() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, admitted) = admit(
            &mut store, &cfg.id, "Finish without a lingering provider handle",
            "headless-completion-lease", false,
        ).unwrap();
        assert!(admitted);
        let task_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let worker = bots::get(&store, "worker").unwrap();
        let ordinary = bots::thread_session(
            &store, &worker, "desktop", "ordinary-headless-poll",
            "Unrelated ordinary chat",
        ).unwrap().0;
        let handles = vec![ordinary.clone(), task_session.clone(), task_session.clone()];

        assert!(headless_cancelled_assignments_with_pause(
            &store, &handles, false,
        ).unwrap().is_empty(), "healthy task and ordinary chat remain live");
        assert_eq!(headless_cancelled_assignments_with_pause(
            &store, &handles, true,
        ).unwrap(), vec![task_session.clone()], "a global pause stops only task-owned providers");
        assert!(headless_cancelled_assignments_with_pause(
            &store, std::slice::from_ref(&ordinary), true,
        ).unwrap().is_empty(), "ordinary sessions keep their independent cancellation path");

        // A provider can retain a live handle after its native Assignment
        // has settled; late turns must not continue that finished task.
        assert!(on_run_finished(&store, &task_session, true, "Coordinator plan recorded").unwrap());
        let state: String = store.db.query_row(
            "SELECT state FROM sa_assignments WHERE session_id=?1",
            [&task_session], |row| row.get(0),
        ).unwrap();
        assert_eq!(state, "finished");
        assert_eq!(headless_cancelled_assignments_with_pause(
            &store, &handles, false,
        ).unwrap(), vec![task_session.clone()]);
        assert_eq!(headless_cancelled_assignments_with_pause(
            &store, &handles, false,
        ).unwrap(), vec![task_session], "polling remains read-only and deduplicated");
        assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
            &store, &ordinary,
        ).is_ok(), "task settlement cannot alter ordinary bot permissions");
    }

    #[test]
    fn orphaned_superagent_thread_is_cancelled_and_never_resumes_as_ordinary_work() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Protect the original task session",
            "orphaned-internal-thread", false,
        ).unwrap();
        assert!(launched);
        let task_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let ordinary = store.create_session("project", "codex", "default", "ask")
            .unwrap().id;
        let both = vec![task_session.clone(), ordinary.clone()];
        assert!(headless_cancelled_assignments_with_pause(&store, &both, false)
            .unwrap().is_empty(), "healthy native and ordinary sessions stay live");

        // Simulate a corrupted legacy writer removing the native Assignment
        // without deleting the still-running provider session or bot thread.
        store.db.execute(
            "DELETE FROM sa_assignments WHERE session_id=?1", [&task_session],
        ).unwrap();
        assert!(has_task_assignment_history(&store, &task_session).unwrap());
        assert!(!has_task_assignment_history(&store, &ordinary).unwrap());
        assert!(read_only_policy(&store, &task_session).is_err());
        assert_eq!(
            headless_cancelled_assignments_with_pause(&store, &both, false).unwrap(),
            vec![task_session.clone()],
            "an orphaned internal task must be cancelled without touching ordinary work",
        );

        let database_path = store.path.clone();
        drop(store);
        let reopened = Store::open(&database_path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "interrupted");
        assert!(has_task_assignment_history(&reopened, &task_session).unwrap());
        assert!(!has_task_assignment_history(&reopened, &ordinary).unwrap());
        assert_eq!(
            headless_cancelled_assignments_with_pause(&reopened, &both, false).unwrap(),
            vec![task_session],
            "restart must not reclassify an orphaned provider as a regular chat",
        );
    }

    #[test]
    fn headless_unattended_resume_rejects_task_session_after_stop_and_restart() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Do not replay this assignment",
            "headless-no-unattended-replay", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(has_task_assignment_history(&store, &session).unwrap());
        assert!(!has_task_assignment_history(&store, "unrelated-normal-session").unwrap());
        stop(&store, &ticket.id).unwrap();
        assert!(has_task_assignment_history(&store, &session).unwrap());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "stopped");
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
        assert!(has_task_assignment_history(&reopened, &session).unwrap());
        assert!(!has_task_assignment_history(&reopened, "unrelated-normal-session").unwrap());
        // Both headless automatic delivery routes consult assignment history.
        // Neither a discarded bot-thread nor a restart can reclassify this as
        // an ordinary session eligible for unattended provider turns.
        let attempts: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(attempts, 1);
    }

    #[test]
    fn headless_run_budget_diagnostic_is_native_redacted_and_nonmutating() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Secret headless objective",
            "headless-budget", false,
        ).unwrap();
        assert!(launched);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let original: serde_json::Value = serde_json::from_str(
            &inspect_headless_status(&store.path, Some(&ticket.id)).unwrap(),
        ).unwrap();
        assert_eq!(original["assignments"][0]["runBudgetExpired"], false);
        assert!(original["assignments"][0]["remainingRunBudgetMs"].as_i64().unwrap() > 0);
        // Simulate a silent provider past the native maximum assignment run.
        store.db.execute(
            "UPDATE sa_assignments SET created_at=?2 WHERE session_id=?1",
            params![sid, storage::now() - MAX_ASSIGNMENT_RUN_MS - 1000],
        ).unwrap();
        let overdue_text = inspect_headless_status(&store.path, Some(&ticket.id)).unwrap();
        let overdue: serde_json::Value = serde_json::from_str(&overdue_text).unwrap();
        assert_eq!(overdue["assignments"][0]["runBudgetExpired"], true);
        assert_eq!(overdue["assignments"][0]["remainingRunBudgetMs"], 0);
        assert!(!overdue_text.contains("Secret headless objective"));
        // Inspection itself cannot terminate a native session, release its
        // project lease or rewrite audit. An expired run is denied execution
        // by the same native policy, while its owner must stop/reconcile it.
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), 1);
        assert!(read_only_policy(&store, &sid).is_err());
        assert!(!on_run_finished(&store, &sid, true, "late worker success").unwrap_or(false)
            || get_ticket(&store, &ticket.id).unwrap().state != "waiting_approval");
    }

    #[test]
    fn native_review_inbox_separates_human_and_unverified_council_states() {
        let mut store = fixture();
        let first = enabled(&store);
        let second = create_configuration(&store, NewOrchestration {
            name: "Second native review Team".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&second.id],
        ).unwrap();
        let (human, started) = admit(
            &mut store, &first.id, "Human review only", "review-inbox-human", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &human.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "PRIVATE NATIVE EVIDENCE").unwrap());
        let (council, started) = admit(
            &mut store, &second.id, "Council pending", "review-inbox-council", true,
        ).unwrap();
        assert!(!started);
        store.db.execute(
            "UPDATE sa_tickets SET state='waiting_review',updated_at=updated_at+1 WHERE id=?1",
            [&council.id],
        ).unwrap();
        let pending = list_review_inbox(&store, 200).unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|row| row.id == human.id
            && row.state == "waiting_approval" && row.orchestration_id == first.id));
        assert!(pending.iter().any(|row| row.id == council.id
            && row.state == "waiting_review" && row.orchestration_id == second.id));
        let json = serde_json::to_string(&pending).unwrap();
        assert!(!json.contains("PRIVATE NATIVE EVIDENCE"));
        assert!(!json.contains("review-inbox-human"));
        assert!(list_review_inbox(&store, 1).unwrap().len() == 1);
        assert!(list_review_inbox(&store, 0).is_err());
        assert!(list_review_inbox(&store, 201).is_err());
        let path = store.path.clone();
        drop(store);
        let read_only = Store::open_read_only(&path).unwrap();
        assert_eq!(list_review_inbox(&read_only, 200).unwrap().len(), 2);
        let headless = list_headless_review_inbox(&path).unwrap();
        let projection:serde_json::Value = serde_json::from_str(&headless).unwrap();
        let rows = projection["tickets"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|item| item["ticketId"] == human.id
            && item["state"] == "waiting_approval"
            && item["nextOperatorAction"] == "inspect_current_revision_before_human_decision"));
        assert!(rows.iter().any(|item| item["ticketId"] == council.id
            && item["state"] == "waiting_review"
            && item["nextOperatorAction"] == "await_fresh_task_bound_native_council"));
        assert_eq!(projection["observationalOnly"],true);
        assert_eq!(projection["startupRecoveryPerformed"],false);
        assert_eq!(projection["mayHaveMore"],false);
        assert!(!headless.contains("Human review only"));
        assert!(!headless.contains("Council pending"));
        assert!(!headless.contains("PRIVATE NATIVE EVIDENCE"));
        assert!(list_headless_review_inbox(
            &std::env::temp_dir().join(format!("absent-sa-review-{}.db",storage::id()))
        ).is_err());
    }

    #[test]
    fn headless_review_inbox_pages_tied_timestamps_after_restart_without_grants() {
        let store = fixture();
        let team = enabled(&store);
        for index in 0..205_i64 {
            let state = if index % 2 == 0 { "waiting_approval" } else { "waiting_review" };
            store.db.execute(
                "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,
                         source,project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'PRIVATE review objective','local-user','manual',?4,
                        'worker',?5,?6,100000,100000)",
                params![format!("review-page-{index:03}"), team.id,
                    format!("review-request-{index:03}"), team.project_id,
                    team.policy_revision, state],
            ).unwrap();
        }
        store.db.execute(
            "UPDATE sa_tickets SET state='blocked' WHERE id='review-page-204'", [],
        ).unwrap();
        // Desktop and headless read the same durable native Ticket-state/cursor ordering.
        let desktop_first = list_review_inbox_page(&store, 200, None, None).unwrap();
        assert_eq!(desktop_first.len(), 200);
        let last = desktop_first.last().unwrap();
        let desktop_next = list_review_inbox_page(
            &store, 200, Some(last.updated_at), Some(&last.id),
        ).unwrap();
        assert_eq!(desktop_next.len(), 4);
        let unique: std::collections::HashSet<_> = desktop_first.iter()
            .chain(desktop_next.iter()).map(|item| item.id.as_str()).collect();
        assert_eq!(unique.len(), 204);
        assert!(desktop_first.iter().chain(desktop_next.iter()).all(|item|
            item.state == "waiting_approval" || item.state == "waiting_review"));
        assert!(list_review_inbox_page(&store, 200, Some(last.updated_at), None).is_err());
        assert!(list_review_inbox_page(&store, 200, Some(last.updated_at), Some("../bad")).is_err());
        let first: serde_json::Value = serde_json::from_str(
            &list_headless_review_inbox(&store.path).unwrap()
        ).unwrap();
        assert_eq!(first["tickets"].as_array().unwrap().len(), 200);
        assert_eq!(first["mayHaveMore"], true);
        let at = first["nextCursor"]["updatedAt"].as_i64().unwrap();
        let id = first["nextCursor"]["ticketId"].as_str().unwrap().to_string();
        let path = store.path.clone();
        drop(store);
        let later_text = list_headless_review_inbox_page(&path, Some(at), Some(&id)).unwrap();
        let later: serde_json::Value = serde_json::from_str(&later_text).unwrap();
        assert_eq!(later["tickets"].as_array().unwrap().len(), 4);
        assert_eq!(later["mayHaveMore"], false);
        assert!(later["nextCursor"].is_null());
        let mut ids = std::collections::HashSet::new();
        for page in [&first, &later] {
            for item in page["tickets"].as_array().unwrap() {
                assert!(ids.insert(item["ticketId"].as_str().unwrap()));
                assert!(item["state"] == "waiting_approval" || item["state"] == "waiting_review");
                assert_eq!(item["nativeCouncilPassVerifiedByThisCommand"], false);
                assert_eq!(item["acceptanceAuthorizedByThisCommand"], false);
            }
        }
        assert_eq!(ids.len(), 204);
        assert!(!later_text.contains("PRIVATE review objective"));
        assert!(list_headless_review_inbox_page(&path, Some(at), None).is_err());
        assert!(list_headless_review_inbox_page(&path, None, Some(&id)).is_err());
        assert!(list_headless_review_inbox_page(&path, Some(at), Some("../bad")).is_err());
        assert!(list_headless_review_inbox_page(&path, Some(0), Some(&id)).is_err());
    }

    #[test]
    fn headless_team_review_inbox_scopes_before_paging_and_rejects_foreign_cursors() {
        let store = fixture();
        let first = enabled(&store);
        let second = create_configuration(&store, NewOrchestration {
            name: "Other review Team".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        let audit_before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        for index in 0..208_i64 {
            let team = if index < 205 { &first } else { &second };
            store.db.execute(
                "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,
                         source,project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'PRIVATE TEAM REVIEW','local-user','manual',?4,
                        'worker',?5,?6,100000,100000)",
                params![format!("team-review-{index:03}"),team.id,
                    format!("team-review-key-{index:03}"),team.project_id,
                    team.policy_revision,
                    if index % 2 == 0 { "waiting_approval" } else { "waiting_review" }],
            ).unwrap();
        }
        let path = store.path.clone();
        let first_team_id = first.id.clone();
        let second_team_id = second.id.clone();
        let first_text = list_headless_team_review_inbox_page(
            &path, &first_team_id, None, None,
        ).unwrap();
        let first_page: serde_json::Value = serde_json::from_str(&first_text).unwrap();
        assert_eq!(first_page["tickets"].as_array().unwrap().len(), 200);
        assert_eq!(first_page["mayHaveMore"], true);
        assert_eq!(first_page["orchestrationId"].as_str(), Some(first_team_id.as_str()));
        let at = first_page["nextCursor"]["updatedAt"].as_i64().unwrap();
        let id = first_page["nextCursor"]["ticketId"].as_str().unwrap().to_owned();
        assert!(list_headless_team_review_inbox_page(
            &path, &second_team_id, Some(at), Some(&id),
        ).is_err());
        assert!(list_headless_team_review_inbox_page(
            &path, "unknown-team", None, None,
        ).is_err());
        assert!(list_headless_team_review_inbox_page(
            &path, &first_team_id, Some(at), Some("team-review-205"),
        ).is_err());
        assert!(list_headless_team_review_inbox_page(
            &path, &first_team_id, Some(at), None,
        ).is_err());
        drop(store);
        let next_text = list_headless_team_review_inbox_page(
            &path, &first_team_id, Some(at), Some(&id),
        ).unwrap();
        let next: serde_json::Value = serde_json::from_str(&next_text).unwrap();
        assert_eq!(next["tickets"].as_array().unwrap().len(), 5);
        assert_eq!(next["mayHaveMore"], false);
        assert!(next["nextCursor"].is_null());
        let mut seen = std::collections::HashSet::new();
        for page in [&first_page, &next] {
            for row in page["tickets"].as_array().unwrap() {
                assert_eq!(row["orchestrationId"].as_str(), Some(first_team_id.as_str()));
                assert_eq!(row["nativeCouncilPassVerifiedByThisCommand"], false);
                assert_eq!(row["acceptanceAuthorizedByThisCommand"], false);
                assert!(seen.insert(row["ticketId"].as_str().unwrap().to_owned()));
            }
        }
        assert_eq!(seen.len(), 205);
        let other_text = list_headless_team_review_inbox_page(
            &path, &second_team_id, None, None,
        ).unwrap();
        let other: serde_json::Value = serde_json::from_str(&other_text).unwrap();
        assert_eq!(other["tickets"].as_array().unwrap().len(), 3);
        assert!(!first_text.contains("PRIVATE TEAM REVIEW"));
        assert!(!next_text.contains("PRIVATE TEAM REVIEW"));
        let after = Store::open_read_only(&path).unwrap();
        let audit_after: i64 = after.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_before, audit_after);
        assert!(list_headless_team_review_inbox_page(
            &path, &first_team_id, Some(0), Some(&id),
        ).is_err());
    }

    #[test]
    fn native_cross_team_demand_index_is_bounded_and_excludes_private_evidence() {
        let mut store = fixture();
        let first = enabled(&store);
        let second = create_configuration(&store, NewOrchestration {
            name: "Second reusable Team".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&second.id],
        ).unwrap();
        let (one, started) = admit(
            &mut store, &first.id, "First demand", "first-demand", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &one.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "PRIVATE WORKER EVIDENCE").unwrap());
        let pending = get_ticket(&store, &one.id).unwrap();
        accept(&store, &one.id, pending.revision).unwrap();
        let (two, started) = admit(
            &mut store, &second.id, "Second demand", "second-demand", false,
        ).unwrap();
        assert!(started);
        let all = list_all_demands(&store, 200).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|row| row.id == one.id && row.orchestration_id == first.id
            && row.state == "completed"));
        assert!(all.iter().any(|row| row.id == two.id && row.orchestration_id == second.id
            && row.state == "running"));
        assert_eq!(list_all_demands(&store, 1).unwrap().len(), 1);
        assert!(list_all_demands(&store, 0).is_err());
        assert!(list_all_demands(&store, 201).is_err());
        assert_eq!(list_tickets(&store, &first.id, 50).unwrap().len(), 1);
        let serialized = serde_json::to_string(&all).unwrap();
        assert!(!serialized.contains("PRIVATE WORKER EVIDENCE"));
        assert!(!serialized.contains("first-demand"));
        assert!(!serialized.contains("authorizationId"));
        assert!(!serialized.contains("sessionId"));
    }


    #[test]
    fn native_demand_search_is_pre_paginated_scoped_literal_and_restart_safe() {
        let store = fixture();
        let first = create_configuration(&store, NewOrchestration {
            name: "Searchable Team".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let other = create_configuration(&store, NewOrchestration {
            name: "Unrelated Team".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        for index in 0..205_i64 {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Unrelated newer demand','local-user','manual',
                        ?4,'worker',?5,'blocked',?6,?6)",
                params![format!("recent-{index:03}"), first.id,
                    format!("recent-request-{index:03}"), first.project_id,
                    first.policy_revision, 5000 + index],
            ).unwrap();
        }
        for (id, objective, timestamp) in [
            ("match-a", "Needle delivery", 3000_i64),
            ("match-b", "Review 100%_NEEDLE evidence", 2000_i64),
            ("match-c", "needle regression", 1000_i64),
        ] {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,'local-user','manual',
                        ?5,'worker',?6,'blocked',?7,?7)",
                params![id,first.id,format!("{id}-request"),objective,
                    first.project_id,first.policy_revision,timestamp],
            ).unwrap();
        }
        store.db.execute(
            "INSERT INTO sa_tickets
             (id,orchestration_id,request_key,objective,actor,source,project_id,
              worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES('other-needle',?1,'other-needle-request','Needle other Team',
                    'local-user','manual',?2,'worker',?3,'blocked',9000,9000)",
            params![other.id,other.project_id,other.policy_revision],
        ).unwrap();
        let search = |before_at, before_id| list_all_demands_page_searched(
            &store, 2, before_at, before_id, Some(&first.id), Some("blocked"),
            Some("NEEDLE"),
        );
        let first_page = search(None, None).unwrap();
        assert_eq!(first_page.iter().map(|item| item.id.as_str())
            .collect::<Vec<_>>(), vec!["match-a", "match-b"]);
        let second_page = search(Some(2000), Some("match-b")).unwrap();
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, "match-c");
        assert!(search(Some(1000), Some("match-c")).unwrap().is_empty());
        assert_eq!(list_all_demands_page_searched(
            &store, 200, None, None, Some(&first.id), Some("blocked"), Some("%_needle"),
        ).unwrap()[0].id, "match-b");
        assert!(list_all_demands_page_searched(
            &store, 200, None, None, Some(&first.id), Some("running"), Some("needle"),
        ).unwrap().is_empty());
        assert!(list_all_demands_page_searched(
            &store, 200, None, None, Some(&first.id), Some("blocked"),
            Some("' OR 1=1 --"),
        ).unwrap().is_empty());
        for invalid in ["", "   ", "a\nneedle", "a\u{0000}needle"] {
            assert!(list_all_demands_page_searched(
                &store, 200, None, None, Some(&first.id), None, Some(invalid),
            ).is_err());
        }
        let oversized = "n".repeat(129);
        assert!(list_all_demands_page_searched(
            &store, 200, None, None, None, None, Some(&oversized),
        ).is_err());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        assert_eq!(list_all_demands_page_searched(
            &reopened, 200, None, None, Some(&first.id), Some("blocked"), Some("needle"),
        ).unwrap().len(), 3);
    }

    #[test]
    fn native_state_filter_is_bounded_pre_paginated_team_scoped_and_restart_safe() {
        let store = fixture();
        let first = create_configuration(&store, NewOrchestration {
            name:"State filter".into(), coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        let second = create_configuration(&store, NewOrchestration {
            name:"Other state".into(), coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        for i in 0..205_i64 {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Filtered native objective','local-user','manual',
                        ?4,'worker',?5,'blocked',?6,?6)",
                params![format!("blocked-{i:03}"),first.id,format!("blocked-key-{i:03}"),
                    first.project_id,first.policy_revision,1000+i/3],
            ).unwrap();
        }
        for i in 0..3_i64 {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Other state objective','local-user','manual',
                        ?4,'worker',?5,'running',?6,?6)",
                params![format!("running-{i}"),first.id,format!("running-key-{i}"),
                    first.project_id,first.policy_revision,3000+i],
            ).unwrap();
        }
        store.db.execute(
            "INSERT INTO sa_tickets
             (id,orchestration_id,request_key,objective,actor,source,project_id,
              worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES('other-blocked',?1,'other-key','Other team objective',
                    'local-user','manual',?2,'worker',?3,'blocked',4000,4000)",
            params![second.id,second.project_id,second.policy_revision],
        ).unwrap();
        let first_page = list_all_demands_page_filtered(
            &store,200,None,None,Some(&first.id),Some("blocked"),
        ).unwrap();
        assert_eq!(first_page.len(),200);
        assert!(first_page.iter().all(|t| t.orchestration_id==first.id
            && t.state=="blocked"));
        let boundary=first_page.last().unwrap();
        let last_page=list_all_demands_page_filtered(
            &store,200,Some(boundary.updated_at),Some(&boundary.id),
            Some(&first.id),Some("blocked"),
        ).unwrap();
        assert_eq!(last_page.len(),5);
        let ids:std::collections::HashSet<_>=first_page.iter().chain(last_page.iter())
            .map(|t|t.id.as_str()).collect();
        assert_eq!(ids.len(),205);
        assert_eq!(list_all_demands_page_filtered(
            &store,200,None,None,Some(&first.id),Some("running"),
        ).unwrap().len(),3);
        assert_eq!(list_all_demands_page_filtered(
            &store,200,None,None,Some(&second.id),Some("blocked"),
        ).unwrap().len(),1);
        assert_eq!(list_all_demands_page_filtered(
            &store,200,None,None,None,Some("blocked"),
        ).unwrap()[0].orchestration_id,second.id);
        for invalid in ["all","BLOCKED","blocked' OR 1=1 --","", "running "] {
            assert!(list_all_demands_page_filtered(
                &store,200,None,None,Some(&first.id),Some(invalid),
            ).is_err());
        }
        assert!(list_all_demands_page_filtered(
            &store,200,Some(5),None,Some(&first.id),Some("blocked"),
        ).is_err());
        let path=store.path.clone(); drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(list_all_demands_page_filtered(
            &reopened,200,None,None,Some(&first.id),Some("running"),
        ).unwrap().len(),3);
    }

    #[test]
    fn native_team_filtered_demand_pages_are_complete_isolated_and_restart_safe() {
        let store = fixture();
        let first_team = create_configuration(&store, NewOrchestration {
            name: "Economy Team".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let second_team = create_configuration(&store, NewOrchestration {
            name: "Performance Team".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        for index in 0..205_i64 {
            let id = format!("economy-{index:03}");
            let key = format!("economy-request-{index:03}");
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Private native objective','local-user','manual',
                        ?4,'worker',?5,'queued',?6,?6)",
                params![id,first_team.id,key,first_team.project_id,
                    first_team.policy_revision,1_000 + index / 3],
            ).unwrap();
        }
        for index in 0..3_i64 {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'Other Team private demand','local-user','manual',
                        ?4,'worker',?5,'queued',?6,?6)",
                params![format!("performance-{index}"),second_team.id,
                    format!("performance-request-{index}"),second_team.project_id,
                    second_team.policy_revision,2_000 + index],
            ).unwrap();
        }
        let first = list_all_demands_page(
            &store, 200, None, None, Some(&first_team.id),
        ).unwrap();
        assert_eq!(first.len(), 200);
        assert!(first.iter().all(|ticket| ticket.orchestration_id == first_team.id));
        let last = first.last().unwrap();
        let next = list_all_demands_page(
            &store, 200, Some(last.updated_at), Some(&last.id),
            Some(&first_team.id),
        ).unwrap();
        assert_eq!(next.len(), 5);
        let ids: std::collections::HashSet<_> = first.iter().chain(next.iter())
            .map(|ticket| ticket.id.as_str()).collect();
        assert_eq!(ids.len(), 205, "Pagination must not skip or duplicate Team Tickets");
        let other = list_all_demands_page(
            &store, 200, None, None, Some(&second_team.id),
        ).unwrap();
        assert_eq!(other.len(), 3);
        assert!(other.iter().all(|ticket| ticket.orchestration_id == second_team.id));
        assert_eq!(list_all_demands_page(&store, 200, None, None, None).unwrap().len(), 200);
        assert!(list_all_demands_page(&store, 200, None, None, Some("../escape")).is_err());
        assert!(list_all_demands_page(&store, 200, None, None, Some("unknown-team")).is_err());
        let path = store.path.clone();
        drop(store);
        let readonly = Store::open_read_only(&path).unwrap();
        assert_eq!(list_all_demands_page(
            &readonly, 200, None, None, Some(&second_team.id)
        ).unwrap().len(), 3);
    }

    #[test]
    fn cross_team_demand_pages_preserve_order_ties_and_durable_history() {
        let store = fixture();
        let team = enabled(&store);
        for index in 0..205_i64 {
            let id = format!("page-{index:03}");
            let key = format!("request-{index:03}");
            let objective = format!("Work item {index}");
            // Several Tickets share the same clock tick; the ID breaks ties.
            let at = 1_000 + index / 3;
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,'local-user','manual',?5,'worker',?6,'queued',?7,?7)",
                params![id,team.id,key,objective,team.project_id,team.policy_revision,at],
            ).unwrap();
        }
        let first = list_all_demands_page(&store, 200, None, None, None).unwrap();
        assert_eq!(first.len(), 200);
        assert_eq!(first[0].id, "page-204");
        let boundary = first.last().unwrap();
        let second = list_all_demands_page(
            &store, 200, Some(boundary.updated_at), Some(&boundary.id), None
        ).unwrap();
        assert_eq!(second.len(), 5);
        assert_eq!(second.last().unwrap().id, "page-000");
        let all_ids: std::collections::HashSet<_> = first.iter().chain(second.iter())
            .map(|item| item.id.clone()).collect();
        assert_eq!(all_ids.len(), 205, "No skipped or repeated Ticket at timestamp ties");
        assert!(list_all_demands_page(&store, 10, Some(1), None, None).is_err());
        assert!(list_all_demands_page(&store, 10, None, Some("page-000"), None).is_err());
        assert!(list_all_demands_page(&store, 10, Some(-1), Some("page-000"), None).is_err());
        assert!(list_all_demands_page(&store, 10, Some(1000), Some("../leak"), None).is_err());
        assert!(list_all_demands_page(&store, 201, None, None, None).is_err());
        // A newer update cannot enter a page requested with an older cursor.
        store.db.execute(
            "UPDATE sa_tickets SET updated_at=9999 WHERE id='page-000'", []
        ).unwrap();
        assert!(!list_all_demands_page(
            &store, 200, Some(boundary.updated_at), Some(&boundary.id), None
        ).unwrap().iter().any(|item| item.id == "page-000"));
        assert_eq!(list_all_demands_page(&store, 1, None, None, None).unwrap()[0].id, "page-000");
        assert!(first.iter().all(|item| item.revision == 0));
    }

    #[test]
    fn headless_team_demand_listing_is_native_read_only_redacted_and_state_scoped() {
        let store = fixture();
        let team = create_configuration(&store, NewOrchestration {
            name:"Headless filtered Team".into(),
            coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        let other = create_configuration(&store, NewOrchestration {
            name:"Other filtered Team".into(),
            coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        for (id,owner,state) in [
            ("task-blocked",&team,"blocked"),
            ("task-running",&team,"running"),
            ("other-blocked",&other,"blocked"),
        ] {
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?1,'PRIVATE OBJECTIVE','local-user','manual',
                        ?3,'worker',?4,?5,1000,1000)",
                params![id,owner.id,owner.project_id,owner.policy_revision,state],
            ).unwrap();
        }
        let path=store.path.clone();
        drop(store);
        let output=list_headless_team_demands(&path,&team.id,"blocked",None,None).unwrap();
        let parsed:serde_json::Value=serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["orchestrationId"],team.id);
        assert_eq!(parsed["observationalOnly"],true);
        assert_eq!(parsed["mayHaveMore"],false);
        assert_eq!(parsed["demands"].as_array().unwrap().len(),1);
        assert_eq!(parsed["demands"][0]["ticketId"],"task-blocked");
        for private in ["PRIVATE OBJECTIVE","authorizationId","sessionId",
            "worker_bot_id","other-blocked","task-running"] {
            assert!(!output.contains(private), "Leak: {private}");
        }
        let all=list_headless_team_demands(&path,&team.id,"all",None,None).unwrap();
        let parsed:serde_json::Value=serde_json::from_str(&all).unwrap();
        assert_eq!(parsed["demands"].as_array().unwrap().len(),2);
        assert!(list_headless_team_demands(&path,&team.id,"BLOCKED",None,None).is_err());
        assert!(list_headless_team_demands(&path,&team.id,"blocked' OR 1=1 --",None,None).is_err());
        assert!(list_headless_team_demands(&path,"not-a-Team","all",None,None).is_err());
        let verify=Store::open_read_only(&path).unwrap();
        assert_eq!(list_all_demands_page_filtered(
            &verify,200,None,None,Some(&team.id),None,
        ).unwrap().len(),2);
    }

    #[test]
    fn headless_team_demand_cursor_is_complete_redacted_and_restart_safe() {
        let store = fixture();
        let team = create_configuration(&store, NewOrchestration {
            name: "Headless paged Team".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        for index in 0..205 {
            let id = format!("page-{index:03}");
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?1,'PRIVATE CURSOR OBJECTIVE','local-user','manual',
                        ?3,'worker',?4,'blocked',1000,2000)",
                params![id,team.id,team.project_id,team.policy_revision],
            ).unwrap();
        }
        let path = store.path.clone();
        drop(store);
        let first_raw = list_headless_team_demands(&path,&team.id,"blocked",None,None).unwrap();
        let searched_raw = list_headless_team_demands_searched(
            &path, &team.id, "blocked", None, None, Some("CURSOR"),
        ).unwrap();
        let searched: serde_json::Value = serde_json::from_str(&searched_raw).unwrap();
        assert_eq!(searched["searchApplied"], true);
        assert_eq!(searched["demands"].as_array().unwrap().len(), 200);
        assert!(!searched_raw.contains("PRIVATE CURSOR OBJECTIVE"));
        let no_match = list_headless_team_demands_searched(
            &path, &team.id, "blocked", None, None, Some("unrelated"),
        ).unwrap();
        let no_match: serde_json::Value = serde_json::from_str(&no_match).unwrap();
        assert_eq!(no_match["demands"].as_array().unwrap().len(), 0);
        let injection = list_headless_team_demands_searched(
            &path, &team.id, "blocked", None, None, Some("' OR 1=1 --"),
        ).unwrap();
        let injection: serde_json::Value = serde_json::from_str(&injection).unwrap();
        assert_eq!(injection["demands"].as_array().unwrap().len(), 0);
        for invalid in ["", "  ", "a\nnewline"] {
            assert!(list_headless_team_demands_searched(
                &path, &team.id, "blocked", None, None, Some(invalid),
            ).is_err());
        }
        let oversized = "x".repeat(129);
        assert!(list_headless_team_demands_searched(
            &path, &team.id, "blocked", None, None, Some(&oversized),
        ).is_err());
        let first:serde_json::Value = serde_json::from_str(&first_raw).unwrap();
        assert_eq!(first["demands"].as_array().unwrap().len(),200);
        assert_eq!(first["mayHaveMore"],true);
        let time = first["nextCursor"]["beforeUpdatedAt"].as_i64().unwrap();
        let id = first["nextCursor"]["beforeId"].as_str().unwrap();
        let next_raw = list_headless_team_demands(
            &path,&team.id,"blocked",Some(time),Some(id),
        ).unwrap();
        let next:serde_json::Value = serde_json::from_str(&next_raw).unwrap();
        assert_eq!(next["demands"].as_array().unwrap().len(),5);
        let searched_next = list_headless_team_demands_searched(
            &path, &team.id, "blocked", Some(time), Some(id), Some("CURSOR"),
        ).unwrap();
        let searched_next: serde_json::Value = serde_json::from_str(&searched_next).unwrap();
        assert_eq!(searched_next["demands"].as_array().unwrap().len(), 5);
        assert_eq!(next["mayHaveMore"],false);
        assert!(next["nextCursor"].is_null());
        let ids:std::collections::HashSet<_> = first["demands"].as_array().unwrap()
            .iter().chain(next["demands"].as_array().unwrap().iter())
            .map(|item| item["ticketId"].as_str().unwrap().to_owned()).collect();
        assert_eq!(ids.len(),205);
        for secret in ["PRIVATE CURSOR OBJECTIVE","request_key","actor","sessionId",
            "authorizationId","worker_bot_id"] {
            assert!(!first_raw.contains(secret) && !next_raw.contains(secret));
        }
        assert!(list_headless_team_demands(&path,&team.id,"blocked",Some(time),None).is_err());
        assert!(list_headless_team_demands(&path,&team.id,"blocked",None,Some(id)).is_err());
        assert!(list_headless_team_demands(&path,&team.id,"blocked",Some(-1),Some(id)).is_err());
        assert!(list_headless_team_demands(
            &path,&team.id,"blocked",Some(time),Some("../invalid"),
        ).is_err());
        let verify = Store::open_read_only(&path).unwrap();
        let total:i64 = verify.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",
            [&team.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(total,205);
    }


    #[test]
    fn headless_global_demand_search_is_cross_team_paginated_and_redacted() {
        let store = fixture();
        let economy = create_configuration(&store, NewOrchestration {
            name: "Global headless Economy".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        let performance = create_configuration(&store, NewOrchestration {
            name: "Global headless Performance".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        for index in 0..208 {
            let owner = if index < 201 { &economy } else { &performance };
            let id = format!("global-{index:03}");
            let objective = if index >= 205 { "PRIVATE UNRELATED OBJECTIVE" }
                else { "PRIVATE MATCHING NEEDLE OBJECTIVE" };
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?1,?3,'local-user','manual',
                        ?4,'worker',?5,'blocked',1000,2000)",
                params![id,owner.id,objective,owner.project_id,owner.policy_revision],
            ).unwrap();
        }
        let path = store.path.clone();
        drop(store);
        let first_raw = list_headless_all_demands_searched(
            &path, "blocked", None, None, Some("needle"),
        ).unwrap();
        let first: serde_json::Value = serde_json::from_str(&first_raw).unwrap();
        assert_eq!(first["scope"], "allTeams");
        assert_eq!(first["observationalOnly"], true);
        assert_eq!(first["searchApplied"], true);
        assert_eq!(first["demands"].as_array().unwrap().len(), 200);
        assert!(first["demands"].as_array().unwrap().iter()
            .any(|item| item["orchestrationId"] == performance.id));
        let time = first["nextCursor"]["beforeUpdatedAt"].as_i64().unwrap();
        let id = first["nextCursor"]["beforeId"].as_str().unwrap();
        let second_raw = list_headless_all_demands_searched(
            &path, "blocked", Some(time), Some(id), Some("needle"),
        ).unwrap();
        let second: serde_json::Value = serde_json::from_str(&second_raw).unwrap();
        assert_eq!(second["demands"].as_array().unwrap().len(), 5);
        let distinct: std::collections::HashSet<_> = first["demands"].as_array().unwrap()
            .iter().chain(second["demands"].as_array().unwrap().iter())
            .map(|item| item["ticketId"].as_str().unwrap()).collect();
        assert_eq!(distinct.len(), 205);
        assert_eq!(second["mayHaveMore"], false);
        for secret in ["PRIVATE", "OBJECTIVE", "local-user", "authorizationId", "request_key"] {
            assert!(!first_raw.contains(secret) && !second_raw.contains(secret));
        }
        let empty = list_headless_all_demands_searched(
            &path, "running", None, None, None,
        ).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&empty).unwrap()
            ["demands"].as_array().unwrap().is_empty());
        let injection = list_headless_all_demands_searched(
            &path, "blocked", None, None, Some("' OR 1=1 --"),
        ).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&injection).unwrap()
            ["demands"].as_array().unwrap().is_empty());
        assert!(list_headless_all_demands_searched(
            &path, "ALL", None, None, None,
        ).is_err());
        assert!(list_headless_all_demands_searched(
            &path, "blocked", Some(time), None, None,
        ).is_err());
        assert!(list_headless_all_demands_searched(
            &path, "blocked", None, None, Some("a\ninvalid"),
        ).is_err());
        let check = Store::open_read_only(&path).unwrap();
        assert_eq!(check.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 208);
    }


    #[test]
    fn headless_project_filter_uses_admitted_ticket_project_before_paging() {
        let store = fixture();
        let team = create_configuration(&store, NewOrchestration {
            name: "Headless historical project".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        let other = create_configuration(&store, NewOrchestration {
            name: "Headless independent Team".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        for index in 0..205 {
            let owner = if index == 203 { &other } else { &team };
            let project = if index == 204 { "other-project" } else { "archived-project" };
            let id = format!("history-{index:03}");
            store.db.execute(
                "INSERT INTO sa_tickets
                 (id,orchestration_id,request_key,objective,actor,source,project_id,
                  worker_bot_id,policy_revision,state,created_at,updated_at)
                 VALUES(?1,?2,?1,'PRIVATE ORIGINAL OBJECTIVE','local-user','manual',
                        ?3,'worker',?4,'blocked',1000,9000)",
                params![id,owner.id,project,owner.policy_revision],
            ).unwrap();
        }
        store.db.execute(
            "UPDATE sa_orchestrations SET project_id='current-project' WHERE id=?1",
            [&team.id],
        ).unwrap();
        let path = store.path.clone();
        drop(store);

        let team_first_raw = list_headless_team_demands_project_searched(
            &path, &team.id, "blocked", None, None,
            Some("ORIGINAL"), Some("archived-project"),
        ).unwrap();
        let team_first: serde_json::Value = serde_json::from_str(&team_first_raw).unwrap();
        assert_eq!(team_first["projectFilterApplied"], true);
        assert_eq!(team_first["searchApplied"], true);
        assert_eq!(team_first["demands"].as_array().unwrap().len(), 200);
        assert!(team_first["demands"].as_array().unwrap().iter()
            .all(|item| item["ticketId"].as_str().unwrap() != "history-203"));
        let timestamp = team_first["nextCursor"]["beforeUpdatedAt"].as_i64().unwrap();
        let cursor = team_first["nextCursor"]["beforeId"].as_str().unwrap();
        let team_next_raw = list_headless_team_demands_project_searched(
            &path, &team.id, "blocked", Some(timestamp), Some(cursor),
            Some("ORIGINAL"), Some("archived-project"),
        ).unwrap();
        let team_next: serde_json::Value = serde_json::from_str(&team_next_raw).unwrap();
        assert_eq!(team_next["demands"].as_array().unwrap().len(), 3);
        let team_ids: std::collections::HashSet<_> = team_first["demands"].as_array().unwrap()
            .iter().chain(team_next["demands"].as_array().unwrap().iter())
            .map(|item| item["ticketId"].as_str().unwrap()).collect();
        assert_eq!(team_ids.len(), 203);
        assert!(team_next["nextCursor"].is_null());
        let global_first_raw = list_headless_all_demands_project_searched(
            &path, "blocked", None, None, None, Some("archived-project"),
        ).unwrap();
        let global_first: serde_json::Value = serde_json::from_str(&global_first_raw).unwrap();
        assert_eq!(global_first["demands"].as_array().unwrap().len(), 200);
        let global_time = global_first["nextCursor"]["beforeUpdatedAt"].as_i64().unwrap();
        let global_id = global_first["nextCursor"]["beforeId"].as_str().unwrap();
        let global_next_raw = list_headless_all_demands_project_searched(
            &path, "blocked", Some(global_time), Some(global_id), None,
            Some("archived-project"),
        ).unwrap();
        let global_next: serde_json::Value = serde_json::from_str(&global_next_raw).unwrap();
        assert_eq!(global_next["demands"].as_array().unwrap().len(), 4);
        assert!(global_first["demands"].as_array().unwrap().iter()
            .chain(global_next["demands"].as_array().unwrap().iter())
            .any(|item| item["orchestrationId"] == other.id));
        let excluded = list_headless_team_demands_project_searched(
            &path, &team.id, "blocked", None, None, None, Some("other-project"),
        ).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&excluded).unwrap()
            ["demands"].as_array().unwrap().len(), 1);
        let missing = list_headless_team_demands_project_searched(
            &path, &team.id, "blocked", None, None, None, Some("current-project"),
        ).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&missing).unwrap()
            ["demands"].as_array().unwrap().len(), 0);
        for invalid in ["", "../escape", "project' OR 1=1 --", " spaced "] {
            assert!(list_headless_all_demands_project_searched(
                &path, "blocked", None, None, None, Some(invalid),
            ).is_err());
        }
        assert!(list_headless_team_demands_project_searched(
            &path, "other-nonexistent", "blocked", None, None, None,
            Some("archived-project"),
        ).is_err());
        for private in ["PRIVATE", "ORIGINAL OBJECTIVE", "local-user", "authorizationId"] {
            assert!(!team_first_raw.contains(private)
                && !team_next_raw.contains(private)
                && !global_first_raw.contains(private));
        }
        let check = Store::open_read_only(&path).unwrap();
        assert_eq!(check.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |row| row.get::<_, i64>(0),
        ).unwrap(), 205);
    }

    #[test]
    fn headless_task_list_is_bounded_redacted_and_read_only() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, _) = admit(
            &mut store, &config.id, "Private task objective", "headless-list", false,
        ).unwrap();
        let before = get_ticket(&store, &ticket.id).unwrap();
        let listed = list_headless_tasks(&store.path).unwrap();
        assert!(listed.contains(&ticket.id));
        assert!(listed.contains("running"));
        assert!(!listed.contains("Private task objective"));
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, before.revision);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
    }

    #[test]
    fn watchdog_revokes_and_interrupts_an_expired_assignment_once_without_replay() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Bounded watchdog work", "watchdog-expired", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let started_at = 10_000_i64;
        store.db.execute(
            "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1",
            params![ticket.id,started_at],
        ).unwrap();
        let now = started_at + MAX_ASSIGNMENT_RUN_MS;
        assert_eq!(watchdog_expired_assignments(&store, now).unwrap(), vec![session]);
        assert!(watchdog_expired_assignments(&store, now + 1).unwrap().is_empty());
        let interrupted = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(interrupted.state, "interrupted");
        assert!(interrupted.active_assignment_id.is_none());
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "interrupted");
        assert_eq!(history(&store, &ticket.id, 10).unwrap()
            .iter().filter(|entry| entry.kind == "watchdog_interrupted").count(), 1);
    }

    #[test]
    fn watchdog_keeps_healthy_assignments_and_revokes_stale_runtime_paths() {
        for fault in ["clock", "paused-worker", "disabled-team", "expired-mandate", "revoked-mandate"] {
            let mut store = fixture();
            let config = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &config.id, "Private unfinished work", "watchdog-stale", false,
            ).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            let now = storage::now();
            assert!(watchdog_expired_assignments(&store, now).unwrap().is_empty());
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
            match fault {
                "clock" => {
                    store.db.execute(
                        "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1",
                        params![ticket.id, now + 60_000],
                    ).unwrap();
                }
                "paused-worker" => {
                    store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
                }
                "disabled-team" => {
                    store.db.execute(
                        "UPDATE sa_orchestrations SET enabled=0 WHERE id=?1",
                        [&config.id],
                    ).unwrap();
                }
                "expired-mandate" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET expires_at=?2 WHERE ticket_id=?1",
                        params![ticket.id, now - 1],
                    ).unwrap();
                }
                "revoked-mandate" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(watchdog_expired_assignments(&store, now).unwrap(), vec![session.clone()], "{fault}");
            assert!(watchdog_expired_assignments(&store, now + 1).unwrap().is_empty());
            let interrupted = get_ticket(&store, &ticket.id).unwrap();
            assert_eq!(interrupted.state, "interrupted");
            assert!(interrupted.active_assignment_id.is_none());
            assert!(mandate(&store, &ticket.id).unwrap().revoked);
            assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "interrupted");
            assert_eq!(history(&store, &ticket.id, 20).unwrap()
                .iter().filter(|entry| entry.kind == "watchdog_interrupted").count(), 1);
            assert!(!on_run_finished(&store, &session, true, "Late success").unwrap());
            assert_ne!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
        }
    }

    #[test]
    fn watchdog_preserves_separate_coordinator_checkout_and_fences_session_drift() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('watchdog-coordinator-project','Coordinator','/tmp/sa-watchdog-coordinator')",
            [],
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET project_id='watchdog-coordinator-project' WHERE id='router'",
            [],
        ).unwrap();
        let config = enabled(&store);
        let (ticket, started) = admit_role(
            &mut store, &config.id, "Plan in coordinator checkout",
            "watchdog-coordinator-workspace", false, true,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_eq!(store.session(&session).unwrap().project_id, "watchdog-coordinator-project");

        // A healthy admitted coordinator is not a worker in the Ticket's
        // project. It must survive watchdog polling until its turn completes.
        assert!(watchdog_expired_assignments(&store, storage::now()).unwrap().is_empty());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(history(&store, &ticket.id, 20).unwrap()
            .iter().filter(|entry| entry.kind == "watchdog_interrupted").count(), 0);

        // A moved or relabelled native session is no longer in the original
        // coordinator's checkout, so the same watchdog must revoke it.
        store.db.execute(
            "UPDATE sessions SET project_id='project' WHERE id=?1",
            [&session],
        ).unwrap();
        assert_eq!(watchdog_expired_assignments(&store, storage::now()).unwrap(), vec![session.clone()]);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert!(!on_run_finished(&store, &session, true, "Late coordinator advice").unwrap());
        assert!(watchdog_expired_assignments(&store, storage::now()).unwrap().is_empty());
    }

    #[test]
    fn watchdog_interruption_and_uncertain_effect_survive_reopen_without_replay() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &config.id, "Private provider payload",
            "watchdog-restart", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        // A transport timeout might already have issued an external effect.
        // Never infer a missing remote outcome is safe to retry after restart.
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'github:issue:42',?2,'pending',?3,?3)",
            params![ticket.id, "a".repeat(64), storage::now()],
        ).unwrap();
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        assert_eq!(watchdog_expired_assignments(&store, storage::now()).unwrap(), vec![session.clone()]);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "interrupted");
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
        assert_eq!(effects::receipt_history(&reopened, &ticket.id, 10).unwrap()[0].state, "uncertain");
        assert_eq!(effects::unresolved_count(&reopened, &ticket.id).unwrap(), 1);
        assert!(watchdog_expired_assignments(&reopened, storage::now()).unwrap().is_empty());
        assert!(!on_run_finished(&reopened, &session, true, "Replay success").unwrap());
        assert_eq!(history(&reopened, &ticket.id, 20).unwrap()
            .iter().filter(|entry| entry.kind == "watchdog_interrupted").count(), 1);
    }

    #[test]
    fn headless_inspection_is_read_only_redacted_and_safe_during_live_work() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Private objective must never reach CLI",
            "headless-read-only", false,
        ).unwrap();
        assert!(launched);
        let path = store.path.clone();
        let summary: serde_json::Value = serde_json::from_str(
            &inspect_headless_status(&path, None).unwrap(),
        ).unwrap();
        assert_eq!(summary[0]["ticketCount"], 1);
        assert_eq!(summary[0]["runningCount"], 1);

        let live_status = inspect_headless_status(&path, Some(&ticket.id)).unwrap();
        let live: serde_json::Value = serde_json::from_str(&live_status).unwrap();
        assert_eq!(live["ticketId"], ticket.id);
        assert_eq!(live["state"], "running");
        assert_eq!(live["mandate"]["revoked"], false);
        assert_eq!(live["assignments"][0]["state"], "running");
        assert!(!live_status.contains("Private objective"));
        // A second inspection must not invoke writable startup recovery,
        // expire the assignment, or insert a restart audit record.
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(history(&store, &ticket.id, 20).unwrap().len(), 1);

        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &sid, true, "Sensitive provider evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        reject(&store, &ticket.id, waiting.revision, "Sensitive rejection reason").unwrap();
        let final_status = inspect_headless_status(&path, Some(&ticket.id)).unwrap();
        let final_view: serde_json::Value = serde_json::from_str(&final_status).unwrap();
        assert_eq!(final_view["state"], "blocked");
        assert_eq!(final_view["assignments"][0]["hasResult"], true);
        assert_eq!(final_view["activity"][0]["kind"], "human_rejected");
        for secret in ["Private objective", "Sensitive provider evidence", "Sensitive rejection reason"] {
            assert!(!final_status.contains(secret), "CLI metadata leaked confidential content");
        }
        assert!(inspect_headless_status(&path, Some("nonexistent-ticket")).is_err());
        let readonly = Store::open_read_only(&path).unwrap();
        assert!(readonly.db.execute(
            "UPDATE sa_tickets SET state='stopped' WHERE id=?1", [&ticket.id],
        ).is_err());
    }

    #[test]
    fn list_overview_counts_all_durable_tickets_without_cross_preset_leaks() {
        let mut store = fixture();
        let config = enabled(&store);
        let other = create_configuration(&store, NewOrchestration {
            name: "Another preset".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();

        // Correlated role counts must not inflate native Ticket aggregates.
        for id in ["worker-extra", "reviewer-extra"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'worker',1,'worker-extra')", [&other.id],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'reviewer',0,'reviewer-extra')", [&other.id],
        ).unwrap();
        let before = orchestration_overview(&store).unwrap();
        assert_eq!(before.len(), 2);
        let first = before.iter().find(|item| item.config.id == config.id).unwrap();
        let second = before.iter().find(|item| item.config.id == other.id).unwrap();
        assert_eq!((first.worker_count, first.reviewer_count), (1, 0));
        assert_eq!((second.worker_count, second.reviewer_count), (2, 1));
        assert_eq!(serde_json::to_value(second).unwrap()["workerCount"], 2);
        assert_eq!(serde_json::to_value(second).unwrap()["reviewerCount"], 1);
        for item in &before {
            assert_eq!(item.ticket_count, 0);
            assert_eq!(item.running_count, 0);
            assert_eq!(item.blocked_count, 0);
            assert!(item.last_activity_at > 0);
        }
        let (active, launched) = admit(
            &mut store, &config.id, "Inspect disposable project",
            "overview-active", false,
        ).unwrap();
        assert!(launched);
        let active_counts = orchestration_overview(&store).unwrap();
        let selected = active_counts.iter().find(|c| c.config.id == config.id).unwrap();
        assert_eq!((selected.ticket_count, selected.running_count, selected.blocked_count), (1,1,0));
        let unrelated = active_counts.iter().find(|c| c.config.id == other.id).unwrap();
        assert_eq!((unrelated.ticket_count, unrelated.running_count, unrelated.blocked_count), (0,0,0));
        let serialized = serde_json::to_value(selected).unwrap();
        assert_eq!(serialized["coordinatorBotId"], "router");
        assert_eq!(serialized["ticketCount"], 1);
        assert_eq!(serialized["runningCount"], 1);
        assert_eq!(serialized["workerCount"], 1);
        assert_eq!(serialized["reviewerCount"], 0);

        stop(&store, &active.id).unwrap();
        let (blocked, launched) = admit(
            &mut store, &config.id, "Blocked pending admission",
            "overview-blocked", true,
        ).unwrap();
        assert!(!launched);
        assert_eq!(blocked.state, "blocked");
        let after = orchestration_overview(&store).unwrap();
        let selected = after.iter().find(|c| c.config.id == config.id).unwrap();
        assert_eq!((selected.ticket_count, selected.running_count, selected.blocked_count), (2,0,1));
        assert!(selected.last_activity_at >= before[0].last_activity_at);
    }

    #[test]
    fn native_ticket_snapshot_keeps_revocation_attempts_and_audit_consistent() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Inspect a disposable fixture", "snapshot-1", false,
        ).unwrap();
        assert!(launched);
        let before = ticket_snapshot(&store, &ticket.id, 50).unwrap();
        assert_eq!(before.ticket.state, "running");
        assert!(!before.mandate.revoked);
        assert!(!before.mandate.allowed_side_effects);
        assert_eq!(before.assignments.len(), 1);
        assert_eq!(before.assignments[0].state, "running");
        assert_eq!(before.activity[0].kind, "admitted");

        stop(&store, &ticket.id).unwrap();
        let after = ticket_snapshot(&store, &ticket.id, 50).unwrap();
        assert_eq!(after.ticket.state, "stopped");
        assert!(after.mandate.revoked);
        assert_eq!(after.assignments[0].state, "stopped");
        assert_eq!(after.activity[0].kind, "stopped");
        assert!(ticket_snapshot(&store, &ticket.id, 0).is_err());
        assert!(ticket_snapshot(&store, &ticket.id, 201).is_err());
    }




    #[test]
    fn mutated_mandate_is_denied_before_retry_or_worker_session_provisioning() {
        let mut store = fixture();
        let config = enabled(&store);
        let (blocked, launched) = admit(
            &mut store, &config.id, "Blocked task", "preflight-retry-1", true,
        ).unwrap();
        assert!(!launched);
        assert_eq!(blocked.state, "blocked");
        store.db.execute(
            "UPDATE sa_mandates SET actor='untrusted' WHERE ticket_id=?1",
            [&blocked.id],
        ).unwrap();
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |r| r.get(0),
        ).unwrap();
        assert!(retry_blocked(&mut store, &blocked.id, false).is_err());
        let after: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(after, before, "invalid retry must not allocate a session");

        let (coordinated, launched) = admit_role(
            &mut store, &config.id, "Coordinator plan",
            "preflight-coordinate-2", false, true,
        ).unwrap();
        assert!(launched);
        let coordinator_session = assignment_session(&store, &coordinated.id)
            .unwrap().unwrap();
        assert!(on_run_finished(
            &store, &coordinator_session, true, "Evidence-backed plan",
        ).unwrap());
        assert_eq!(get_ticket(&store, &coordinated.id).unwrap().state, "queued");
        store.db.execute(
            "UPDATE sa_mandates SET actor='untrusted' WHERE ticket_id=?1",
            [&coordinated.id],
        ).unwrap();
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |r| r.get(0),
        ).unwrap();
        assert!(prepare_coordinated_worker(&mut store, &coordinated.id, false).is_err());
        let after: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(after, before, "invalid worker mandate must not allocate a session");
        assert_eq!(get_ticket(&store, &coordinated.id).unwrap().state, "queued");
    }

    #[test]
    fn unsupported_mandate_escalation_cannot_run_or_turn_into_human_approval() {
        let mut store = fixture();
        let config = enabled(&store);
        for (suffix, mutation) in [
            ("side-effects", "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1"),
            ("tool-grants", "UPDATE sa_mandates SET allowed_tools='[\"shell\"]' WHERE ticket_id=?1"),
            ("project", "UPDATE sa_mandates SET project_id='other-project' WHERE ticket_id=?1"),
            ("actor", "UPDATE sa_mandates SET actor='model-authored' WHERE ticket_id=?1"),
        ] {
            let (ticket, launched) = admit(
                &mut store, &config.id, "Inspect with original read-only scope",
                &format!("grant-escalation-{suffix}"), false,
            ).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            store.db.execute(mutation, [&ticket.id]).unwrap();
            assert!(read_only_policy(&store, &session).is_err(),
                "Changed mandate must fail closed: {suffix}");
            assert!(on_run_finished(&store, &session, true, "Untrusted success").unwrap());
            let blocked = get_ticket(&store, &ticket.id).unwrap();
            assert_eq!(blocked.state, "blocked");
            assert!(blocked.evidence.is_none(), "Rejected callbacks must not retain provider evidence: {suffix}");
            assert!(assignments(&store, &ticket.id, 10).unwrap()[0].result.is_none(),
                "Rejected attempts must not retain untrusted provider text: {suffix}");
            assert!(accept(&store, &ticket.id, blocked.revision).is_err());
            assert_eq!(history(&store, &ticket.id, 10).unwrap()[0].kind, "callback_rejected");
            assert!(!on_run_finished(&store, &session, true, "Duplicate").unwrap());
        }
    }


    #[test]
    fn revoked_live_callback_discards_untrusted_evidence_across_restart() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Inspect without external effects",
            "revoked-evidence-restart", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        store.db.execute(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        assert!(read_only_policy(&store, &session).is_err());
        assert!(on_run_finished(
            &store, &session, true, "UNTRUSTED POST-REVOCATION OUTPUT",
        ).unwrap());
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        assert!(blocked.evidence.is_none());
        assert!(assignments(&store, &ticket.id, 10).unwrap()[0].result.is_none());
        assert!(history(&store, &ticket.id, 10).unwrap().iter()
            .any(|entry| entry.kind == "callback_rejected"));
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let persisted = get_ticket(&reopened, &ticket.id).unwrap();
        assert_eq!(persisted.state, "blocked");
        assert!(persisted.evidence.is_none());
        assert!(assignments(&reopened, &ticket.id, 10).unwrap()[0].result.is_none());
        assert!(accept(&reopened, &ticket.id, persisted.revision).is_err());
        assert!(!on_run_finished(&reopened, &session, true, "REPLAYED CALLBACK").unwrap());
    }

    #[test]
    fn stale_ticket_or_team_project_policy_denies_provider_and_callback_settlement() {
        for scenario in ["ticket-revision", "team-project"] {
            let mut store = fixture();
            let config = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &config.id, "Original scoped read-only task",
                &format!("policy-drift-{scenario}"), false,
            ).unwrap();
            assert!(launched);
            let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(read_only_policy(&store, &sid).unwrap().is_some());

            if scenario == "ticket-revision" {
                store.db.execute(
                    "UPDATE sa_tickets SET policy_revision=policy_revision+1 WHERE id=?1",
                    [&ticket.id],
                ).unwrap();
            } else {
                store.db.execute(
                    "INSERT INTO projects(id,name,path) VALUES('moved-team','Moved team','/tmp/sa-moved-team')",
                    [],
                ).unwrap();
                store.db.execute(
                    "UPDATE sa_orchestrations SET project_id='moved-team' WHERE id=?1",
                    [&config.id],
                ).unwrap();
            }

            assert!(read_only_policy(&store, &sid).is_err(), "{scenario}");
            assert!(guard_run_admission(&store, &sid, &[]).is_err(), "{scenario}");
            assert!(on_run_finished(&store, &sid, true, "Stale provider result").unwrap());
            let blocked = get_ticket(&store, &ticket.id).unwrap();
            assert_eq!(blocked.state, "blocked", "{scenario}");
            assert!(accept(&store, &ticket.id, blocked.revision).is_err());
            assert!(history(&store, &ticket.id, 20).unwrap().iter()
                .any(|event| event.kind == "callback_rejected"));
            assert!(!on_run_finished(&store, &sid, true, "Duplicate").unwrap());
        }
    }

    #[test]
    fn mandate_issuer_drift_denies_provider_admission_and_callback() {
        for fault in ["missing-issuer", "blank-issuer", "wrong-actor"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &cfg.id, "Private test evidence",
                &format!("issuer-callback-{fault}"), false,
            ).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(read_only_policy(&store, &session).unwrap().is_some());
            let mutation = match fault {
                "missing-issuer" => "UPDATE sa_mandates SET authorization_id='' WHERE ticket_id=?1",
                "blank-issuer" => "UPDATE sa_mandates SET authorization_id='  ' WHERE ticket_id=?1",
                "wrong-actor" => "UPDATE sa_mandates SET actor='untrusted' WHERE ticket_id=?1",
                _ => unreachable!(),
            };
            store.db.execute(mutation, [&ticket.id]).unwrap();
            assert!(read_only_policy(&store, &session).is_err(), "{fault}");
            assert!(on_run_finished(&store, &session, true, "Untrusted result").unwrap());
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked", "{fault}");
            assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "failed");
            assert!(history(&store, &ticket.id, 10).unwrap().iter()
                .any(|entry| entry.kind == "callback_rejected"));
        }
    }

    #[test]
    fn mandate_issuer_drift_blocks_human_acceptance_of_existing_evidence() {
        for fault in ["missing-issuer", "blank-issuer", "wrong-actor"] {
            let mut store = fixture();
            let cfg = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &cfg.id, "Original private task",
                &format!("issuer-approval-{fault}"), false,
            ).unwrap();
            assert!(launched);
            let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(on_run_finished(&store, &session, true, "Native worker evidence").unwrap());
            let waiting = get_ticket(&store, &ticket.id).unwrap();
            assert_eq!(waiting.state, "waiting_approval");
            let mutation = match fault {
                "missing-issuer" => "UPDATE sa_mandates SET authorization_id='' WHERE ticket_id=?1",
                "blank-issuer" => "UPDATE sa_mandates SET authorization_id='  ' WHERE ticket_id=?1",
                "wrong-actor" => "UPDATE sa_mandates SET actor='untrusted' WHERE ticket_id=?1",
                _ => unreachable!(),
            };
            store.db.execute(mutation, [&ticket.id]).unwrap();
            assert!(accept(&store, &ticket.id, waiting.revision).is_err(), "{fault}");
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
            assert!(!history(&store, &ticket.id, 10).unwrap().iter()
                .any(|entry| entry.kind == "human_accepted"));
        }
    }

    #[test]
    fn watchdog_interrupts_unsupported_grants_and_drift_before_budget_expires() {
        for scenario in ["tools", "effects", "ticket-revision", "team-project", "issuer-missing", "issuer-actor"] {
            let mut store = fixture();
            let config = enabled(&store);
            let (ticket, launched) = admit(
                &mut store, &config.id, "Watchdog must revoke stale task scope",
                &format!("watchdog-policy-{scenario}"), false,
            ).unwrap();
            assert!(launched);
            let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
            assert!(read_only_policy(&store, &sid).unwrap().is_some());

            match scenario {
                "tools" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET allowed_tools='[\"shell\"]' WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                "effects" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                "ticket-revision" => {
                    store.db.execute(
                        "UPDATE sa_tickets SET policy_revision=policy_revision+1 WHERE id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                "issuer-missing" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET authorization_id=' ' WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                "issuer-actor" => {
                    store.db.execute(
                        "UPDATE sa_mandates SET actor='untrusted' WHERE ticket_id=?1",
                        [&ticket.id],
                    ).unwrap();
                }
                "team-project" => {
                    store.db.execute(
                        "INSERT INTO projects(id,name,path) VALUES('moved-team','Moved team','/tmp/sa-moved-team')",
                        [],
                    ).unwrap();
                    store.db.execute(
                        "UPDATE sa_orchestrations SET project_id='moved-team' WHERE id=?1",
                        [&config.id],
                    ).unwrap();
                }
                _ => unreachable!(),
            }

            assert!(read_only_policy(&store, &sid).is_err(), "{scenario}");
            let cancelled = watchdog_expired_assignments(&store, storage::now()).unwrap();
            assert_eq!(cancelled, vec![sid.clone()], "{scenario}");
            assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
            let revoked: i64 = store.db.query_row(
                "SELECT revoked FROM sa_mandates WHERE ticket_id=?1",
                [&ticket.id], |row| row.get(0),
            ).unwrap();
            assert_eq!(revoked, 1, "{scenario}");
            assert!(history(&store, &ticket.id, 10).unwrap().iter()
                .any(|event| event.kind == "watchdog_interrupted"));
            assert!(watchdog_expired_assignments(&store, storage::now()).unwrap().is_empty());
            assert!(!on_run_finished(&store, &sid, true, "Late result").unwrap());
        }
    }

    #[test]
    fn expired_assignment_budget_denies_native_tools_and_late_success() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Bounded read-only fixture",
            "assignment-timeout-1", false,
        ).unwrap();
        assert!(launched);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &session_id).unwrap().is_some());
        let now = storage::now();
        assert!(!assignment_within_budget(now + 1, now), "future start must fail closed");
        assert!(!assignment_within_budget(
            now - MAX_ASSIGNMENT_RUN_MS, now,
        ), "attempt deadline is exclusive");
        let expired_at = now - MAX_ASSIGNMENT_RUN_MS - 1000;
        store.db.execute(
            "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1",
            params![ticket.id, expired_at],
        ).unwrap();
        assert!(read_only_policy(&store, &session_id).is_err());
        assert!(on_run_finished(
            &store, &session_id, true, "Late provider success is not approval",
        ).unwrap());
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        assert!(blocked.active_assignment_id.is_none());
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "failed");
        assert_eq!(history(&store, &ticket.id, 10).unwrap()[0].kind, "assignment_timed_out");
        assert!(accept(&store, &ticket.id, blocked.revision).is_err());
        assert!(!on_run_finished(&store, &session_id, true, "Duplicate").unwrap());

        // The timed-out attempt releases its native lease; it never resurrects
        // on another ticket or silently extends its original authorization.
        let (next, started) = admit(
            &mut store, &config.id, "Independent authorized ticket",
            "assignment-timeout-2", false,
        ).unwrap();
        assert!(started);
        assert_eq!(next.state, "running");
    }

    #[test]
    fn gateway_denies_untrusted_unassigned_and_revoked_internal_messages_before_thread_creation() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &config.id, "Read-only assigned task", "internal-gateway-1", false,
        ).unwrap();
        assert!(started);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let thread: String = store.db.query_row(
            "SELECT external_id FROM bot_threads WHERE session_id=?1",
            [&session_id], |row| row.get(0),
        ).unwrap();
        assert!(guard_gateway_inbound(&store, "worker", &thread, "local-user", true).is_ok());
        for (bot, external, sender, trusted) in [
            ("worker", thread.as_str(), "untrusted-external", false),
            ("worker", thread.as_str(), "bot:router", true),
            ("router", thread.as_str(), "local-user", true),
            ("worker", "unassigned-thread", "local-user", true),
        ] {
            assert!(guard_gateway_inbound(&store, bot, external, sender, trusted).is_err());
        }
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM bot_threads WHERE channel_id=?1",
            [CHANNEL], |row| row.get(0),
        ).unwrap();
        assert!(guard_gateway_inbound(
            &store, "worker", "unassigned-thread", "local-user", true,
        ).is_err());
        let after: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM bot_threads WHERE channel_id=?1",
            [CHANNEL], |row| row.get(0),
        ).unwrap();
        assert_eq!(before, after, "rejected deliveries must not create bot conversations");
        stop(&store, &ticket.id).unwrap();
        assert!(guard_gateway_inbound(&store, "worker", &thread, "local-user", true).is_err());
    }

    #[test]
    fn internal_gateway_rejects_remapped_threads_and_preserves_retry_identity() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Original task-scoped thread",
            "native-thread-binding", false,
        ).unwrap();
        assert!(launched);
        let original = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(guard_gateway_inbound(
            &store, "worker", &ticket.id, "local-user", true,
        ).is_ok());

        // Existing thread ownership alone is insufficient when a stale or
        // unrelated external ID is mapped onto this assigned session.
        store.db.execute(
            "UPDATE bot_threads SET external_id='foreign-ticket' WHERE session_id=?1",
            [&original],
        ).unwrap();
        assert!(guard_gateway_inbound(
            &store, "worker", "foreign-ticket", "local-user", true,
        ).is_err());
        store.db.execute(
            "UPDATE bot_threads SET external_id=?2 WHERE session_id=?1",
            params![original,ticket.id],
        ).unwrap();
        assert!(on_run_finished(&store, &original, true, "First attempt evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        reject(&store, &ticket.id, waiting.revision, "Requires a new attempt").unwrap();

        let (_, retry_thread, bot_id) = retry_blocked(&mut store, &ticket.id, false).unwrap();
        assert_eq!(bot_id, "worker");
        assert!(retry_thread.ends_with(":attempt:2"));
        assert!(guard_gateway_inbound(
            &store, "worker", &retry_thread, "local-user", true,
        ).is_ok());
    }

    #[test]
    fn coordinated_retry_cannot_erase_original_coordinator_provenance() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit_role(
            &mut store, &cfg.id, "Coordinate then delegate", "plan-provenance", false, true,
        ).unwrap();
        assert!(launched);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &sid, true, "Evidence-backed plan").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
        assert!(coordinator_plan_current(&store, &ticket.id, "router", "project").unwrap());

        // An assignment cannot become worker evidence by rewriting both
        // bot IDs after a successful coordinator turn.
        store.db.execute(
            "UPDATE sa_assignments SET bot_id='worker' WHERE session_id=?1",
            [&sid],
        ).unwrap();
        store.db.execute(
            "UPDATE bot_threads SET bot_id='worker' WHERE session_id=?1",
            [&sid],
        ).unwrap();
        assert!(!coordinator_plan_current(&store, &ticket.id, "router", "project").unwrap());
        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |row| row.get(0),
        ).unwrap();
        assert!(prepare_coordinated_worker(&mut store, &ticket.id, false).is_err());
        let after: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sessions", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(before, after, "denied delegation must not allocate a worker session");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
    }

    #[test]
    fn direct_worker_attempt_cannot_be_relabeled_as_coordinator_advice() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Direct worker only", "role-preservation", false,
        ).unwrap();
        assert!(started);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        // A coordinator is configured and valid, but the admitted task mode
        // did not authorize a coordinator round or a change of bot identity.
        store.db.execute(
            "UPDATE sa_assignments SET bot_id='router' WHERE session_id=?1", [&sid],
        ).unwrap();
        store.db.execute(
            "UPDATE bot_threads SET bot_id='router' WHERE session_id=?1", [&sid],
        ).unwrap();
        assert!(assignment_thread_current(&store, &sid, &ticket.id).unwrap());
        assert!(read_only_policy(&store, &sid).is_err());
        assert!(on_run_finished(&store, &sid, true, "Forged coordinator plan").unwrap());
        let ticket = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(ticket.state, "blocked");
        assert!(history(&store, &ticket.id, 30).unwrap()
            .iter().all(|entry| entry.kind != "coordinator_planned"));
        assert!(accept(&store, &ticket.id, ticket.revision).is_err());
    }

    #[test]
    fn remapped_assignment_and_thread_cannot_admit_an_unselected_bot() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Authorized worker only", "role-proof", false,
        ).unwrap();
        assert!(started);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &sid).unwrap().is_some());

        // Even changing both persisted links to a valid third bot is not a
        // user-issued task assignment or an authorization to read this scope.
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('unselected','Unselected','project','codex','default','plan',?1,?1)",
            params![storage::now()],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_assignments SET bot_id='unselected' WHERE session_id=?1",
            [&sid],
        ).unwrap();
        store.db.execute(
            "UPDATE bot_threads SET bot_id='unselected' WHERE session_id=?1",
            [&sid],
        ).unwrap();
        assert!(assignment_thread_current(&store, &sid, &ticket.id).unwrap());
        assert!(read_only_policy(&store, &sid).is_err());
        assert!(guard_gateway_inbound(&store, "unselected", &ticket.id, "local-user", true).is_err());
        assert!(guard_run_admission(&store, &sid, &[]).is_err());
        assert!(on_run_finished(&store, &sid, true, "Unauthorized third bot result").unwrap());
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        assert!(accept(&store, &ticket.id, blocked.revision).is_err());
    }

    #[test]
    fn detached_or_settled_superagent_session_never_falls_back_to_normal_bot_tools() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Scoped native session", "no-ambient-fallback", false,
        ).unwrap();
        assert!(launched);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &sid).unwrap().is_some());

        // Deleting the channel binding cannot erase the durable Ticket's
        // identity and accidentally restore the bot's normal tool permissions.
        store.db.execute(
            "DELETE FROM bot_threads WHERE session_id=?1", [&sid],
        ).unwrap();
        assert!(read_only_policy(&store, &sid).is_err());
        assert!(guard_run_admission(&store, &sid, &[]).is_err());

        // A late provider success after the thread was detached is also
        // rejected as evidence; neither a new launch nor human acceptance
        // may inherit that orphaned native task result.
        assert!(on_run_finished(&store, &sid, true, "Detached provider result").unwrap());
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        assert!(accept(&store, &ticket.id, blocked.revision).is_err());
        assert!(history(&store, &ticket.id, 20).unwrap()
            .iter().any(|event| event.kind == "callback_rejected"));
        assert!(read_only_policy(&store, &sid).is_err());

        let ordinary = bots::get(&store, "worker").unwrap();
        let normal_sid = bots::thread_session(
            &store, &ordinary, "desktop", "ordinary-session", "Ordinary bot chat",
        ).unwrap().0;
        assert!(read_only_policy(&store, &normal_sid).unwrap().is_none());
    }

    #[test]
    fn ordinary_bot_tools_cannot_escape_an_active_or_revoked_superagent_mandate() {
        let mut store = fixture();
        let config = enabled(&store);
        let worker = bots::get(&store, "worker").unwrap();
        let ordinary = bots::thread_session(
            &store, &worker, "desktop", "independent-normal", "Normal bot chat",
        ).unwrap().0;
        assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(&store, &ordinary).is_ok());

        let (ticket, launched) = admit(
            &mut store, &config.id, "No ambient tool rights", "ambient-tool-fence", false,
        ).unwrap();
        assert!(launched);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &session_id).unwrap().is_some());
        let error = crate::bots::tools::ensure_ambient_bot_tool_permission(
            &store, &session_id,
        ).unwrap_err();
        assert!(error.contains("cannot invoke ambient bot tools"));

        // A stopped assignment is denied by the same effect-time check even
        // if the native session and its original bot binding still exist.
        let (stopped, _) = stop(&store, &ticket.id).unwrap();
        assert_eq!(stopped.state, "stopped");
        assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
            &store, &session_id,
        ).is_err());
        assert!(crate::bots::tools::ensure_ambient_bot_tool_permission(
            &store, &ordinary,
        ).is_ok(), "ordinary direct bot conversations must not inherit the task fence");
    }

    #[test]
    fn desktop_superagent_commands_require_a_trusted_main_window() {
        assert!(require_main_window_label("main").is_ok());
        for window in ["", "overlay", "mini", "bot", "main-secondary"] {
            assert_eq!(
                require_main_window_label(window).unwrap_err(),
                "SuperAgent desktop commands require the main window."
            );
        }
    }


    fn fixture() -> Store {
        let path=std::env::temp_dir().join(format!("pandacode-superagent-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('project','Disposable','/tmp/disposable-superagent')",[],
        ).unwrap();
        for (id,name) in [("router","Router"),("worker","Builder")] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,'project','codex','default','plan',?3,?3)",
                params![id,name,storage::now()],
            ).unwrap();
        }
        store
    }

    fn enabled(store: &Store) -> Orchestration {
        let cfg=create_configuration(store,NewOrchestration {
            name:"Safe manual".into(),coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        assert!(!cfg.enabled);
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",[&cfg.id],
        ).unwrap();
        configuration(store,&cfg.id).unwrap()
    }

    #[test]
    fn ordinary_running_session_blocks_same_project_manual_team_admission() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let ordinary = store.create_session("project", "codex", "default", "ask").unwrap();
        let active = vec![ordinary.id.clone()];
        assert!(active_manual_conflict(&store, &active, "worker", "project").unwrap());
        assert!(!manual_preflight(&store, &cfg.id, "Inspect the checkout", false,
            active_manual_conflict(&store, &active, "worker", "project").unwrap())
            .unwrap().eligible);
        let (blocked, launched) = admit(&mut store, &cfg.id, "Inspect the checkout",
            "occupied-by-direct-chat", true).unwrap();
        assert!(!launched);
        assert_eq!(blocked.state, "blocked");
        assert!(assignment_session(&store, &blocked.id).unwrap().is_none());
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('other-project','Other','/tmp/disposable-superagent-other')", [],
        ).unwrap();
        let unrelated = store.create_session("other-project", "codex", "default", "ask").unwrap();
        assert!(!active_manual_conflict(&store, &[unrelated.id], "worker", "project").unwrap(),
            "Unrelated projects must remain independently executable");
        assert!(active_manual_conflict(&store, &["missing-live-session".to_owned()],
            "worker", "project").is_err(), "Missing live metadata must fail closed");
    }

    #[test]
    fn superagent_cloud_reconnect_is_denied_before_unrestricted_provider_admission() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Inspect disposable fixture",
            "cloud-reconnect-guard", false,
        ).unwrap();
        assert!(launched);
        let sid = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let active_error = guard_cloud_attach(&store, &sid).unwrap_err();
        assert!(active_error.contains("policy_unsupported"));
        assert!(active_error.contains("unrestricted cloud"));
        // An unrelated ordinary cloud session is not affected by this
        // SuperAgent-specific gate.
        assert!(guard_cloud_attach(&store, "unrelated-session").is_ok());
        stop(&store, &ticket.id).unwrap();
        assert!(guard_cloud_attach(&store, &sid).is_err(),
            "A revoked task must never reattach with ambient cloud permissions");
    }

    #[test]
    fn unsupported_primary_harnesses_fail_before_team_creation_or_activation() {
        let store = fixture();
        let input = || NewOrchestration {
            name: "Unsupported primary".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        };
        store.db.execute("UPDATE bots SET provider='not-a-real-harness' WHERE id='router'", []).unwrap();
        assert!(create_configuration(&store, input()).unwrap_err().contains("harness"));
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "Rejected Team must leave no disabled orphan");
        store.db.execute("UPDATE bots SET provider='codex' WHERE id='router'", []).unwrap();
        let config = create_configuration(&store, input()).unwrap();
        store.db.execute("UPDATE bots SET mode='unsupported-test-mode' WHERE id='worker'", []).unwrap();
        let error = change_enabled(&store, &config.id, config.policy_revision, true).unwrap_err();
        assert!(error.contains("permission mode"), "{error}");
        let persisted = configuration(&store, &config.id).unwrap();
        assert!(!persisted.enabled);
        assert_eq!(persisted.policy_revision, config.policy_revision);
        store.db.execute("UPDATE bots SET mode='plan' WHERE id='worker'", []).unwrap();
        assert!(change_enabled(&store, &config.id, config.policy_revision, true).unwrap().enabled);
    }

    #[test]
    fn directory_reports_native_manual_enable_denials_without_changing_authority() {
        let store = fixture();
        let cfg = create_configuration(&store, NewOrchestration {
            name: "Eligibility Team".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let view = |store: &Store| -> OrchestrationOverview {
            orchestration_overview(store).unwrap().into_iter()
                .find(|item| item.config.id == cfg.id).unwrap()
        };
        assert!(view(&store).manual_enable_blocker.is_none());
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        let paused = view(&store);
        assert_eq!(paused.manual_enable_blocker.as_deref(),
            Some("Selected bots are paused or their worker scope has changed."));
        assert!(change_enabled(&store, &cfg.id, cfg.policy_revision, true).is_err());
        store.db.execute(
            "UPDATE bots SET paused=0,mode='unsupported-test-mode' WHERE id='worker'",[],
        ).unwrap();
        let unsupported = view(&store);
        assert!(unsupported.manual_enable_blocker.as_deref().unwrap()
            .contains("permission mode"));
        let denied = change_enabled(&store, &cfg.id, cfg.policy_revision, true).unwrap_err();
        assert_eq!(Some(denied.as_str()), unsupported.manual_enable_blocker.as_deref());
        store.db.execute("UPDATE bots SET mode='plan' WHERE id='worker'", []).unwrap();
        assert!(view(&store).manual_enable_blocker.is_none());
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('reviewer','Reviewer','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'reviewer',0,'reviewer')", [&cfg.id],
        ).unwrap();
        let multirole = view(&store);
        assert_eq!(multirole.manual_enable_blocker.as_deref(),
            Some("Configured multi-member Teams cannot run without native scoped admission."));
        assert!(change_enabled(&store, &cfg.id, cfg.policy_revision, true).is_err());
        assert!(!configuration(&store, &cfg.id).unwrap().enabled);
        assert_eq!(configuration(&store, &cfg.id).unwrap().policy_revision, cfg.policy_revision);
        let db_path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&db_path).unwrap();
        assert_eq!(view(&reopened).manual_enable_blocker, multirole.manual_enable_blocker);
    }

    #[test]
    fn unused_disabled_preset_is_revision_guarded_and_never_auto_enabled() {
        let store=fixture();
        let config=create_configuration(&store,NewOrchestration{
            name:"Original".into(),coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        let input=NewOrchestration{
            name:"Renamed".into(),coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        };
        let updated=update_configuration(&store,&config.id,1,&input).unwrap();
        assert_eq!(updated.policy_revision,2);
        assert!(!updated.enabled);
        assert!(update_configuration(&store,&config.id,1,&input).is_err());
        assert_eq!(update_configuration(&store,&config.id,2,&input).unwrap().policy_revision,2);
        delete_configuration(&store,&config.id,2).unwrap();
        assert!(configuration(&store,&config.id).is_err());
    }

    #[test]
    fn settled_profiled_demands_allow_safe_primary_role_rotation_and_new_admission() {
        let mut store=fixture();
        let config=enabled(&store);
        let (previous,launched)=admit(&mut store,&config.id,
            "Original bounded task","rotation-original",false).unwrap();
        assert!(launched);
        let original=roster::ticket_profile(&store,&previous.id).unwrap().unwrap();
        assert_eq!(original.members[0].bot_id,"router");
        assert_eq!(original.members[1].bot_id,"worker");
        stop(&store,&previous.id).unwrap();
        let disabled=change_enabled(&store,&config.id,config.policy_revision,false).unwrap();
        let swap=NewOrchestration {
            name:"Reused Team".into(),coordinator_bot_id:"worker".into(),
            worker_bot_id:"router".into(),
        };

        // An unresolved effect or a Team-scoped GitHub source must not be
        // silently rebound by the new Team policy.
        store.db.execute(
            "INSERT INTO sa_effect_receipts(
                ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'unresolved','fixture','uncertain',?2,?2)",
            params![previous.id,storage::now()],
        ).unwrap();
        assert!(update_configuration(&store,&config.id,disabled.policy_revision,&swap)
            .unwrap_err().contains("unsettled work"));
        store.db.execute(
            "DELETE FROM sa_effect_receipts WHERE ticket_id=?1",[&previous.id],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_event_sources(
                id,orchestration_id,repository,actor_filter,event_kind,created_at,updated_at)
             VALUES('rotation-source',?1,'owner/repo','actor','issues.opened',?2,?2)",
            params![config.id,storage::now()],
        ).unwrap();
        assert!(update_configuration(&store,&config.id,disabled.policy_revision,&swap)
            .unwrap_err().contains("GitHub sources"));
        store.db.execute(
            "DELETE FROM sa_event_sources WHERE id='rotation-source'",[],
        ).unwrap();

        // Valid JSON with an invalid role shape cannot prove that historical
        // identity remains recoverable.
        let snapshot_json:String=store.db.query_row(
            "SELECT profiles_json FROM sa_ticket_profiles WHERE ticket_id=?1",
            [&previous.id],|row|row.get(0),
        ).unwrap();
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json='{}' WHERE ticket_id=?1",
            [&previous.id],
        ).unwrap();
        assert!(update_configuration(&store,&config.id,disabled.policy_revision,&swap)
            .unwrap_err().contains("Invalid persisted task-effective Team profile"));
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
            params![previous.id,snapshot_json],
        ).unwrap();

        let updated=update_configuration(&store,&config.id,disabled.policy_revision,&swap)
            .unwrap();
        assert!(!updated.enabled);
        assert_eq!(updated.policy_revision,disabled.policy_revision+1);
        assert_eq!(updated.coordinator_bot_id,"worker");
        assert_eq!(updated.worker_bot_id,"router");
        assert!(update_configuration(&store,&config.id,disabled.policy_revision,&swap).is_err());
        assert_eq!(roster::ticket_profile(&store,&previous.id).unwrap().unwrap()
            .members[1].bot_id,"worker");
        let original_ticket:(i64,String,String)=store.db.query_row(
            "SELECT policy_revision,worker_bot_id,project_id FROM sa_tickets WHERE id=?1",
            [&previous.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        assert_eq!(original_ticket,(config.policy_revision,"worker".into(),"project".into()));
        assert!(mandate(&store,&previous.id).unwrap().revoked);

        let path=store.path.clone();
        drop(store);
        let mut reopened=Store::open(&path).unwrap();
        assert_eq!(roster::ticket_profile(&reopened,&previous.id).unwrap().unwrap()
            .members[0].bot_id,"router");
        let resumed=change_enabled(&reopened,&config.id,updated.policy_revision,true).unwrap();
        assert!(resumed.enabled);
        // Stage a new busy request without starting a provider. It must pin
        // the new roles, not silently adopt the historical role selection.
        let (next,launched)=admit(&mut reopened,&config.id,
            "New bounded task","rotation-next",true).unwrap();
        assert!(!launched);
        let new_profile=roster::ticket_profile(&reopened,&next.id).unwrap().unwrap();
        assert_eq!(new_profile.members[0].bot_id,"worker");
        assert_eq!(new_profile.members[1].bot_id,"router");
        assert_eq!(get_ticket(&reopened,&previous.id).unwrap().state,"stopped");
        assert_ne!(previous.id,next.id);
    }

    #[test]
    fn settled_history_allows_rename_without_rebinding_roles_or_original_ticket_policy() {
        let store=fixture();
        let cfg=create_configuration(&store,NewOrchestration {
            name:"Original Team".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('review-bot','Independent Reviewer','project','codex','default','plan',?1,?1)",
            [now],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'reviewer',0,'review-bot')",[&cfg.id],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_tickets(
                id,orchestration_id,request_key,objective,actor,source,project_id,
                worker_bot_id,policy_revision,state,created_at,updated_at
             ) VALUES(
                'settled-history',?1,'settled-history-key','Original immutable objective',
                'local-user','manual','project','worker',?2,'completed',?3,?3
             )",
            params![&cfg.id,cfg.policy_revision,now],
        ).unwrap();
        let renamed=update_configuration(&store,&cfg.id,cfg.policy_revision,&NewOrchestration {
            name:"Reusable Team".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        assert_eq!(renamed.policy_revision,cfg.policy_revision+1);
        assert!(!renamed.enabled);
        assert_eq!(roster::read(&store,&cfg.id).unwrap().reviewer_bot_ids,vec!["review-bot"]);
        let original:(i64,String,String)=store.db.query_row(
            "SELECT policy_revision,worker_bot_id,objective FROM sa_tickets WHERE id='settled-history'",
            [],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).unwrap();
        assert_eq!(original,(cfg.policy_revision,"worker".into(),
            "Original immutable objective".into()));
        assert!(update_configuration(&store,&cfg.id,cfg.policy_revision,&NewOrchestration {
            name:"Stale update".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).is_err());
        assert!(delete_configuration(&store,&cfg.id,renamed.policy_revision).is_err());
    }

    #[test]
    fn rename_with_history_denies_unsettled_review_effect_and_primary_identity_change() {
        let store=fixture();
        let cfg=create_configuration(&store,NewOrchestration {
            name:"Pinned Team".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_tickets(
                id,orchestration_id,request_key,objective,actor,source,project_id,
                worker_bot_id,policy_revision,state,created_at,updated_at
             ) VALUES(
                'review-history',?1,'review-history-key','Pinned evidence',
                'local-user','manual','project','worker',?2,'waiting_review',?3,?3
             )",
            params![&cfg.id,cfg.policy_revision,now],
        ).unwrap();
        let rename=NewOrchestration {
            name:"Renamed after settlement".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        };
        assert!(update_configuration(&store,&cfg.id,cfg.policy_revision,&rename).is_err());
        store.db.execute("UPDATE sa_tickets SET state='completed' WHERE id='review-history'",[])
            .unwrap();
        store.db.execute(
            "INSERT INTO sa_effect_receipts(
                ticket_id,effect_key,payload_sha256,state,created_at,updated_at
             ) VALUES('review-history','pending-reply','fixture-digest','uncertain',?1,?1)",
            [now],
        ).unwrap();
        assert!(update_configuration(&store,&cfg.id,cfg.policy_revision,&rename).is_err());
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied' WHERE ticket_id='review-history'",[],
        ).unwrap();
        let swapped=NewOrchestration {
            name:"Unauthorized rebind".into(),coordinator_bot_id:"worker".into(),
            worker_bot_id:"router".into(),
        };
        assert!(update_configuration(&store,&cfg.id,cfg.policy_revision,&swapped).unwrap_err()
            .contains("Legacy Ticket history has no immutable Team profile"));
        assert_eq!(configuration(&store,&cfg.id).unwrap().policy_revision,cfg.policy_revision);
        let renamed=update_configuration(&store,&cfg.id,cfg.policy_revision,&rename).unwrap();
        assert_eq!(renamed.policy_revision,cfg.policy_revision+1);
        assert_eq!(get_ticket(&store,"review-history").unwrap().state,"completed");
    }

    #[test]
    fn headless_rename_reuses_native_history_and_revision_guards_without_provider_launch() {
        let path=std::env::temp_dir().join(format!("sa-headless-rename-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('rename-project','Rename','/tmp/sa-headless-rename')",[],
        ).unwrap();
        for id in ["rename-lead","rename-worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'rename-project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=create_configuration(&store,NewOrchestration {
            name:"Original".into(),coordinator_bot_id:"rename-lead".into(),
            worker_bot_id:"rename-worker".into(),
        }).unwrap();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_tickets(
                id,orchestration_id,request_key,objective,actor,source,project_id,
                worker_bot_id,policy_revision,state,created_at,updated_at
             ) VALUES(
                'headless-history',?1,'headless-history-key','Private original objective',
                'local-user','manual','rename-project','rename-worker',?2,'completed',?3,?3)",
            params![&cfg.id,cfg.policy_revision,now],
        ).unwrap();
        let result=headless_rename_orchestration(&path,&cfg.id,cfg.policy_revision,"Renamed Team").unwrap();
        let json:serde_json::Value=serde_json::from_str(&result).unwrap();
        assert_eq!(json["name"],"Renamed Team");
        assert_eq!(json["policyRevision"],cfg.policy_revision+1);
        assert_eq!(json["grantIssued"],false);
        assert!(headless_rename_orchestration(&path,&cfg.id,cfg.policy_revision,"Replay")
            .is_err());
        let original:(i64,String)=store.db.query_row(
            "SELECT policy_revision,objective FROM sa_tickets WHERE id='headless-history'",
            [],|row|Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        assert_eq!(original,(cfg.policy_revision,"Private original objective".into()));
        assert_eq!(configuration(&store,&cfg.id).unwrap().name,"Renamed Team");
        assert_eq!(store.db.query_row("SELECT COUNT(*) FROM sa_assignments",[],
            |row|row.get::<_,i64>(0)).unwrap(),0);
        store.db.execute(
            "UPDATE sa_tickets SET state='waiting_review' WHERE id='headless-history'",[],
        ).unwrap();
        assert!(headless_rename_orchestration(&path,&cfg.id,cfg.policy_revision+1,"Unsafe")
            .is_err());
        assert_eq!(configuration(&store,&cfg.id).unwrap().name,"Renamed Team");
    }

    #[test]
    fn headless_strategy_uses_native_policy_cas_and_preserves_history_across_restart() {
        let store = fixture();
        let team = create_configuration(&store, NewOrchestration {
            name: "Strategy CLI".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let path = store.path.clone();
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,
              project_id,worker_bot_id,policy_revision,state,created_at,updated_at)
             VALUES('strategy-cli-history',?1,'strategy-cli-history','Private original objective',
              'local-user','manual',?2,?3,?4,'completed',?5,?5)",
            params![&team.id,&team.project_id,&team.worker_bot_id,team.policy_revision,now],
        ).unwrap();
        let output = headless_set_team_strategy(
            &path, &team.id, team.policy_revision, "Release engineering", "economy",
        ).unwrap();
        let outcome: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(outcome["policyRevision"], team.policy_revision + 1);
        assert_eq!(outcome["purposeConfigured"], true);
        assert!(outcome.get("purpose").is_none());
        assert!(!output.contains("Release engineering"), "private purpose must not reach stdout");
        assert_eq!(outcome["strategy"], "economy");
        assert_eq!(outcome["grantIssued"], false);
        assert_eq!(outcome["providerDispatched"], false);
        assert!(headless_set_team_strategy(
            &path, &team.id, team.policy_revision, "Replay", "performance",
        ).is_err(), "stale revision cannot rewrite the Team");
        assert!(headless_set_team_strategy(
            &path, &team.id, team.policy_revision + 1, "Invalid", "unrestricted",
        ).is_err(), "CLI cannot bypass native strategy validation");
        assert_eq!(get_ticket(&store,"strategy-cli-history").unwrap().objective,
            "Private original objective");
        let recorded_policy_revision: i64 = store.db.query_row(
            "SELECT policy_revision FROM sa_tickets WHERE id=?1",
            ["strategy-cli-history"], |row| row.get(0),
        ).unwrap();
        assert_eq!(recorded_policy_revision, team.policy_revision);
        store.db.execute(
            "UPDATE sa_tickets SET state='waiting_review'
             WHERE id='strategy-cli-history'", [],
        ).unwrap();
        assert!(headless_set_team_strategy(
            &path, &team.id, team.policy_revision + 1, "Cannot change", "performance",
        ).is_err(), "reviewable Demand must block policy edits");
        store.db.execute(
            "UPDATE sa_tickets SET state='completed' WHERE id='strategy-cli-history'", [],
        ).unwrap();
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        let saved = roster::read(&reopened, &team.id).unwrap();
        assert_eq!(saved.purpose, "Release engineering");
        assert_eq!(saved.strategy, "economy");
        assert_eq!(saved.policy_revision, team.policy_revision + 1);
    }

    #[test]
    fn disabled_team_rotation_preserves_durable_ticket_and_blocks_deletion() {
        let mut store=fixture();
        let config=enabled(&store);
        let (ticket,_)=admit(&mut store,&config.id,"Immutable history","policy-guard",false).unwrap();
        let disabled=change_enabled(&store,&config.id,config.policy_revision,false).unwrap();
        let input=NewOrchestration{
            name:"Changed".into(),coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        };
        // A disabled Team with settled Ticket history may be renamed, but
        // its historical identities and audit remain immutable.
        let renamed=update_configuration(&store,&config.id,disabled.policy_revision,&input).unwrap();
        assert_eq!(renamed.name,"Changed");
        assert_eq!(renamed.policy_revision,disabled.policy_revision+1);
        let forbidden=NewOrchestration{
            name:"Rebound".into(),coordinator_bot_id:"worker".into(),worker_bot_id:"router".into(),
        };
        let rotated=update_configuration(&store,&config.id,renamed.policy_revision,&forbidden).unwrap();
        assert_eq!(rotated.coordinator_bot_id,"worker");
        assert_eq!(rotated.worker_bot_id,"router");
        let profile=roster::ticket_profile(&store,&ticket.id).unwrap().unwrap();
        assert_eq!(profile.members[0].bot_id,"router");
        assert_eq!(ticket_worker(&store,&ticket.id).unwrap(),"worker");
        assert!(delete_configuration(&store,&config.id,rotated.policy_revision).is_err());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().objective,"Immutable history");
        assert_eq!(configuration(&store,&config.id).unwrap().name,"Rebound");
    }

    #[test]
    fn disabling_an_orchestration_atomically_revokes_active_and_pending_mandates() {
        let mut store = fixture();
        let config = enabled(&store);
        let (running, launched) = admit(
            &mut store, &config.id, "Inspect disposable fixture", "disable-active", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &running.id).unwrap().unwrap();
        let (blocked, launched) = admit(
            &mut store, &config.id, "Pending independent task", "disable-pending", true,
        ).unwrap();
        assert!(!launched);
        let updated = change_enabled(&store, &config.id, config.policy_revision, false).unwrap();
        assert!(!updated.enabled);
        assert_eq!(updated.policy_revision, config.policy_revision + 1);

        let interrupted = ticket_snapshot(&store, &running.id, 20).unwrap();
        assert_eq!(interrupted.ticket.state, "interrupted");
        assert!(interrupted.ticket.active_assignment_id.is_none());
        assert!(interrupted.mandate.revoked);
        assert_eq!(interrupted.assignments[0].state, "interrupted");
        assert_eq!(interrupted.activity[0].kind, "orchestration_disabled");
        let pending = ticket_snapshot(&store, &blocked.id, 20).unwrap();
        assert_eq!(pending.ticket.state, "blocked");
        assert!(pending.mandate.revoked);
        assert_eq!(pending.activity[0].kind, "orchestration_disabled");
        assert!(read_only_policy(&store, &session).is_err());
        assert!(!on_run_finished(&store, &session, true, "Late success").unwrap());

        // Re-enabling the same preset is not a renewal of old task authority.
        let restored = change_enabled(&store, &config.id, updated.policy_revision, true).unwrap();
        assert!(restored.enabled);
        assert!(retry_blocked(&mut store, &blocked.id, false).is_err());
        assert!(accept(&store, &running.id, interrupted.ticket.revision).is_err());
        assert!(read_only_policy(&store, &session).is_err());
        let (fresh, started) = admit(
            &mut store, &config.id, "Fresh user-authorized work", "disable-new", false,
        ).unwrap();
        assert!(started);
        assert_eq!(fresh.state, "running");
    }

    #[test]
    fn global_pause_is_an_execution_revocation_not_a_new_implicit_grant() {
        assert!(execution_permitted(true, false));
        assert!(!execution_permitted(false, false));
        assert!(!execution_permitted(true, true));
        assert!(!execution_permitted(false, true));
    }

    #[test]
    fn revision_guard_and_bot_availability_prevent_stale_policy_enabling() {
        let store = fixture();
        let config = create_configuration(&store, NewOrchestration {
            name: "Guarded".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        assert_eq!(config.policy_revision, 1);
        let active = change_enabled(&store, &config.id, 1, true).unwrap();
        assert!(active.enabled);
        assert_eq!(active.policy_revision, 2);
        assert!(change_enabled(&store, &config.id, 1, false).is_err());
        assert_eq!(change_enabled(&store, &config.id, 2, true).unwrap().policy_revision, 2);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        assert!(change_enabled(&store, &config.id, 2, false).is_ok());
        assert!(change_enabled(&store, &config.id, 3, true).is_err());
        assert!(!configuration(&store, &config.id).unwrap().enabled);
    }

    #[test]
    fn real_existing_bots_in_distinct_workspaces_can_create_and_admit_a_manual_task() {
        // Native bot creation always assigns a separate workspace project.
        // A synthetic same-project fixture must not disguise this real behavior.
        let mut store = fixture();
        let root = std::env::temp_dir().join(format!("pandacode-superagent-bots-{}", storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        let input = |name: &str| bots::BotInput {
            name: name.into(),
            role: "Agent".into(),
            provider: "codex".into(),
            model: "default".into(),
            mode: "plan".into(),
            ..Default::default()
        };
        let coordinator = bots::create(&store, &input("Native coordinator"), &root).unwrap();
        let worker = bots::create(&store, &input("Native worker"), &root).unwrap();
        assert_ne!(coordinator.project_id, worker.project_id);
        let config = create_configuration(&store, NewOrchestration {
            name: "Native worker scope".into(),
            coordinator_bot_id: coordinator.id.clone(),
            worker_bot_id: worker.id.clone(),
        }).unwrap();
        assert_eq!(config.project_id, worker.project_id);
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();
        let (ticket, ready) = admit(&mut store, &config.id, "Read-only inspection", "native-scope-1", false).unwrap();
        assert!(ready);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_eq!(store.session(&session_id).unwrap().project_id, worker.project_id);
        assert_eq!(ticket.state, "running");
        assert!(read_only_policy(&store, &session_id).unwrap().is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_off_repairs_stale_active_mandates_without_reopening_work() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Inspect safely", "stale-disabled-mandate", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();

        // Simulate an old interrupted policy write that persisted Off but
        // failed before settling active work. This must never be treated as
        // an idempotent no-op with a still-live provider grant.
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=0 WHERE id=?1",
            [&cfg.id],
        ).unwrap();
        let repaired = change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        assert!(!repaired.enabled);
        assert!(repaired.policy_revision > cfg.policy_revision);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
        assert!(assignment_session(&store, &ticket.id).unwrap().is_none());
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert!(read_only_policy(&store, &session).is_err());
        assert!(!on_run_finished(&store, &session, true, "stale callback").unwrap());

        // Once repaired, repeated Off is genuinely idempotent.
        let same = change_enabled(&store, &cfg.id, repaired.policy_revision, false).unwrap();
        assert_eq!(same.policy_revision, repaired.policy_revision);
    }

    #[test]
    fn repeated_off_does_not_revise_an_already_revoked_blocked_ticket() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Pending worker", "off-idempotent-pending", true,
        ).unwrap();
        assert!(!launched);
        let disabled = change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        let settled = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(settled.state, "blocked");
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        let history_count = history(&store, &ticket.id, 100).unwrap().len();

        let same = change_enabled(&store, &cfg.id, disabled.policy_revision, false).unwrap();
        assert_eq!(same.policy_revision, disabled.policy_revision);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, settled.revision);
        assert_eq!(history(&store, &ticket.id, 100).unwrap().len(), history_count);
    }

    #[test]
    fn disable_settles_detached_running_assignment_without_reauthorizing_it() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Original assignment", "off-detached-assignment", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();

        // A legacy interrupted state writer detached the running lease from
        // the ticket; ordinary successful callbacks cannot settle this run.
        store.db.execute(
            "UPDATE sa_tickets SET state='blocked',active_assignment_id=NULL WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        let _ = change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        let assignment = assignments(&store, &ticket.id, 10).unwrap();
        assert_eq!(assignment[0].state, "interrupted");
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert!(read_only_policy(&store, &session).is_err());
        assert!(!on_run_finished(&store, &session, true, "late result").unwrap());
    }

    #[test]
    fn disable_rolls_back_policy_and_leases_when_audit_fails() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Audited stop", "off-atomic-failure", false,
        ).unwrap();
        assert!(launched);
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_off_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='orchestration_disabled'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
        ).unwrap();

        assert!(change_enabled(&store, &cfg.id, cfg.policy_revision, false).is_err());
        assert!(configuration(&store, &cfg.id).unwrap().enabled);
        assert_eq!(configuration(&store, &cfg.id).unwrap().policy_revision, cfg.policy_revision);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "running");
        store.db.execute_batch("DROP TRIGGER reject_off_audit").unwrap();
        let disabled = change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        assert!(!disabled.enabled);
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
    }

    #[test]
    fn manual_dispatch_uses_persisted_bounded_objectives_for_both_roles() {
        let mut store = fixture();
        let cfg = enabled(&store);
        // Native admission trims and bounds the objective. Raw Desktop input
        // can have much more whitespace than the bot gateway accepts.
        let raw = format!("{}Inspect fixture{}", " ".repeat(65_536), " ".repeat(65_536));
        let (direct, launched) = admit(&mut store, &cfg.id, &raw, "trim-direct", false).unwrap();
        assert!(launched);
        assert_eq!(direct.objective, "Inspect fixture");
        let direct_packet = initial_manual_packet(&direct, false);
        assert!(direct_packet.contains("\n\nInspect fixture\n\n"));
        assert!(direct_packet.contains("Read-only assignment"));
        assert!(direct_packet.len() < 32 * 1024);

        stop(&store, &direct.id).unwrap();
        let (coordinated, launched) = admit_role(
            &mut store, &cfg.id, &raw, "trim-coordinator", false, true,
        ).unwrap();
        assert!(launched);
        assert_eq!(coordinated.objective, "Inspect fixture");
        let coordinator_packet = initial_manual_packet(&coordinated, true);
        assert!(coordinator_packet.contains("\n\nInspect fixture\n\n"));
        assert!(coordinator_packet.contains("Read-only coordinator"));
        assert!(coordinator_packet.len() < 32 * 1024);
    }

    #[test]
    fn secondary_team_member_reset_revokes_only_affected_demands_and_survives_restart() {
        let mut store = fixture();
        for id in ["secondary-worker", "secondary-reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let affected_team = enabled(&store);
        let other = create_configuration(&store, NewOrchestration {
            name: "Independent Team".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&other.id],
        ).unwrap();
        let unaffected_team = configuration(&store, &other.id).unwrap();
        let (affected, started) = admit(
            &mut store, &affected_team.id, "Scoped task", "secondary-reset-1", true,
        ).unwrap();
        assert!(!started);
        let (unaffected, started) = admit(
            &mut store, &unaffected_team.id, "Other task", "secondary-reset-2", true,
        ).unwrap();
        assert!(!started);
        assert!(!mandate(&store, &affected.id).unwrap().revoked);
        // A repaired or imported legacy Team can acquire additional members
        // while a queued Ticket exists. Their reset must not leave its original
        // Mandate viable, regardless of whether the topology can execute yet.
        for (role, position, bot_id) in [
            ("worker", 1, "secondary-worker"),
            ("reviewer", 0, "secondary-reviewer"),
        ] {
            store.db.execute(
                "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
                 VALUES(?1,?2,?3,?4)",
                params![&affected_team.id, role, position, bot_id],
            ).unwrap();
        }
        invalidate_bot(&store, "secondary-worker", false).unwrap();
        assert!(mandate(&store, &affected.id).unwrap().revoked);
        assert!(!mandate(&store, &unaffected.id).unwrap().revoked);
        let previous_revision = get_ticket(&store, &affected.id).unwrap().revision;
        // Deleting a configured secondary bot must fail before any mutation;
        // the FK-restricted role still owns its identity and durable history.
        assert!(bots::delete(&store, "secondary-reviewer").unwrap_err()
            .contains("Remove this bot from disabled SuperAgent Team roles"));
        assert!(bots::get(&store, "secondary-reviewer").is_ok());
        assert_eq!(get_ticket(&store, &affected.id).unwrap().revision, previous_revision);
        assert!(configuration(&store, &affected_team.id).unwrap().enabled);
        invalidate_bot(&store, "secondary-reviewer", false).unwrap();
        assert_eq!(get_ticket(&store, &affected.id).unwrap().revision, previous_revision + 1);
        assert!(configuration(&store, &unaffected_team.id).unwrap().enabled);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert!(mandate(&reopened, &affected.id).unwrap().revoked);
        assert!(!mandate(&reopened, &unaffected.id).unwrap().revoked);
        assert!(configuration(&reopened, &affected_team.id).unwrap().enabled);
        assert!(bots::get(&reopened, "secondary-reviewer").is_ok());
    }

    #[test]
    fn reset_snapshotted_member_without_current_roster_revokes_original_ticket() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('historical-reviewer','Historical Reviewer','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        let team = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &team.id, "Pinned legacy task", "pinned-reviewer", true,
        ).unwrap();
        assert!(!launched);
        let original = roster::ticket_profile(&store, &ticket.id).unwrap().unwrap();
        let mut members = original.members;
        let bot = bots::get(&store, "historical-reviewer").unwrap();
        members.push(roster::AdmittedMemberProfile {
            role: "reviewer".into(), position: 0, bot_id: bot.id,
            project_id: bot.project_id, project_path: None, resolved_project_path: None,
            provider: bot.provider, model: bot.model, mode: bot.mode,
            reasoning_effort: bot.reasoning_effort, assigned_skill_ids: bot.skills,
        });
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
            params![ticket.id, serde_json::to_string(&members).unwrap()],
        ).unwrap();
        // Historical member was pinned by admission but is no longer in the
        // current Team roster. Reset must still revoke its Ticket Mandate.
        assert!(!roster::has_additional_members(&store, &team.id).unwrap());
        invalidate_bot(&store, "historical-reviewer", false).unwrap();
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
    }

    #[test]
    fn unsupported_multi_role_team_cannot_bypass_native_admission_or_retry() {
        let mut store = fixture();
        let config = enabled(&store);
        // Leave one blocked ticket and one coordinator-planned ticket before
        // simulating a stale enabled Team with a newly configured Worker.
        let (blocked, launched) = admit(
            &mut store, &config.id, "Pending task", "roster-blocked", true,
        ).unwrap();
        assert!(!launched);
        let (planned, launched) = admit_role(
            &mut store, &config.id, "Coordinated task", "roster-planned", false, true,
        ).unwrap();
        assert!(launched);
        let coordinator_session = assignment_session(&store, &planned.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &coordinator_session, true, "Read-only plan").unwrap());
        assert_eq!(get_ticket(&store, &planned.id).unwrap().state, "queued");

        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('worker-b','Other Worker','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        // Normal roster editing refuses enabled Teams; an inconsistent DB
        // or concurrent writer still must not gain native execution authority.
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'worker',1,'worker-b')",
            [&config.id],
        ).unwrap();

        let advisory = manual_preflight(&store, &config.id, "Fresh task", false, false).unwrap();
        assert!(!advisory.eligible);
        assert!(advisory.reason.unwrap().contains("Multi-member"));
        assert!(admit(&mut store, &config.id, "Fresh task", "roster-denied", false)
            .unwrap_err().contains("Multi-member"));
        assert!(retry_blocked(&mut store, &blocked.id, false)
            .unwrap_err().contains("Multi-member"));
        assert!(prepare_coordinated_worker(&mut store, &planned.id, false)
            .unwrap_err().contains("Multi-member"));

        assert_eq!(get_ticket(&store, &blocked.id).unwrap().state, "blocked");
        assert_eq!(get_ticket(&store, &planned.id).unwrap().state, "queued");
        let tickets: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(tickets, 2, "unsupported role topology must not create new Tickets");
        let worker_attempts: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE bot_id='worker-b'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(worker_attempts, 0, "no unsupported Worker may be admitted");
    }

    #[test]
    fn native_manual_preflight_is_read_only_and_reports_current_role_and_limits() {
        let store = fixture();
        let cfg = create_configuration(&store, NewOrchestration {
            name: "Preflight fixture".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let disabled = manual_preflight(&store, &cfg.id, "Inspect safely", false, false).unwrap();
        assert!(!disabled.eligible);
        assert!(disabled.reason.unwrap().contains("disabled"));
        let enabled = change_enabled(&store, &cfg.id, cfg.policy_revision, true).unwrap();
        let direct = manual_preflight(&store, &cfg.id, "Inspect safely", false, false).unwrap();
        assert!(direct.eligible);
        assert!(!direct.can_stage_blocked);
        assert_eq!(direct.policy_revision, enabled.policy_revision);
        assert_eq!(direct.selected_role, "worker");
        assert_eq!(direct.selected_bot_id, "worker");
        assert_eq!(direct.project_id, "project");
        assert!(direct.allowed_tools.is_empty());
        assert!(!direct.allowed_side_effects);
        assert_eq!(direct.attempt_budget_ms, MAX_ASSIGNMENT_RUN_MS);
        let coordinator = manual_preflight(&store, &cfg.id, "Inspect safely", true, false).unwrap();
        assert_eq!(coordinator.selected_role, "coordinator");
        assert_eq!(coordinator.selected_bot_id, "router");
        assert!(manual_preflight(&store, &cfg.id, " ", false, false).is_err());
        let count: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_tickets", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "Preflight must not create a Ticket or any grant");
    }

    #[test]
    fn native_manual_preflight_detects_checkout_contention_and_revoked_bot() {
        let mut store = fixture();
        let cfg = enabled(&store);
        assert!(!manual_preflight(&store, &cfg.id, "Inspect", false, true).unwrap().eligible);
        let (ticket, started) = admit(&mut store, &cfg.id, "Active task", "preflight-lease", false).unwrap();
        assert!(started);
        let busy = manual_preflight(&store, &cfg.id, "Other task", false, false).unwrap();
        assert!(!busy.eligible);
        assert!(busy.can_stage_blocked, "busy direct Workers can stage a blocked Ticket");
        assert!(busy.reason.unwrap().contains("occupied"));
        let coordinated = manual_preflight(&store, &cfg.id, "Other task", true, true).unwrap();
        assert!(!coordinated.eligible);
        assert!(!coordinated.can_stage_blocked, "busy Coordinator cannot stage");
        stop(&store, &ticket.id).unwrap();
        assert!(manual_preflight(&store, &cfg.id, "Other task", false, false).unwrap().eligible);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        let denied = manual_preflight(&store, &cfg.id, "Other task", false, false).unwrap();
        assert!(!denied.eligible);
        assert!(!denied.can_stage_blocked, "paused Worker cannot stage");
    }

    #[test]
    fn manual_dispatch_is_durable_deduplicated_and_requires_native_acceptance() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket,started)=admit(&mut store,&cfg.id,"Inspect fixture","stable-key",false).unwrap();
        assert!(started);
        assert_eq!(ticket.state,"running");
        let (same,duplicate)=admit(&mut store,&cfg.id,"Inspect fixture","stable-key",false).unwrap();
        assert!(!duplicate);
        assert_eq!(ticket.id,same.id);
        assert!(admit(&mut store,&cfg.id,"Different","stable-key",false).is_err());
        let sid=assignment_session(&store,&ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store,&sid).unwrap().is_some());
        assert!(!on_run_finished(&store,"wrong-session",true,"fake").unwrap());
        assert!(on_run_finished(&store,&sid,true,"Documented evidence").unwrap());
        let waiting=get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(waiting.state,"waiting_approval");
        assert!(accept(&store,&ticket.id,ticket.revision).is_err());
        let completed=accept(&store,&ticket.id,waiting.revision).unwrap();
        assert_eq!(completed.state,"completed");
        assert!(!on_run_finished(&store,&sid,true,"late reply").unwrap());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"completed");
    }

    #[test]
    fn native_snapshot_reports_true_finalization_gate_without_granting_acceptance() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (task, launched) = admit(&mut store, &cfg.id, "Inspect finalization",
            "snapshot-approval-gate", false).unwrap();
        assert!(launched);
        let active = ticket_snapshot(&store, &task.id, 20).unwrap();
        assert_eq!(active.finalization_gate, "not_pending");
        let sid = assignment_session(&store, &task.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &sid, true, "Recorded result").unwrap());
        let waiting = ticket_snapshot(&store, &task.id, 20).unwrap();
        assert_eq!(waiting.finalization_gate, "native_recheck_required");
        assert_eq!(waiting.unresolved_external_effects, 0);
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'effect','digest','uncertain',?2,?2)",
            params![task.id,now],
        ).unwrap();
        let blocked = ticket_snapshot(&store, &task.id, 20).unwrap();
        assert_eq!(blocked.finalization_gate, "effects_unresolved");
        assert_eq!(blocked.unresolved_external_effects, 1);
        assert!(accept(&store, &task.id, blocked.ticket.revision).is_err());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "waiting_approval");
    }

    #[test]
    fn unresolved_effects_block_manual_and_headless_acceptance_without_losing_evidence() {
        let mut store = fixture();
        let config = enabled(&store);
        let (task, started) = admit(
            &mut store, &config.id, "Verify external state",
            "unresolved-effect-finalization", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &task.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Worker evidence").unwrap());
        let waiting = get_ticket(&store, &task.id).unwrap();
        assert_eq!(waiting.state, "waiting_approval");
        let now = storage::now();
        // Simulate durable effect receipts retained from an interrupted
        // remote request, not a fabricated provider success.
        for (key, state) in [("write", "pending"), ("comment", "uncertain")] {
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                  (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,'sha256-fixture',?3,?4,?4)",
                params![task.id,key,state,now],
            ).unwrap();
        }
        let refusal = accept(&store, &task.id, waiting.revision).unwrap_err();
        assert!(refusal.contains("unresolved external effects"));
        assert!(headless_accept_ticket(&store.path, &task.id, waiting.revision)
            .unwrap_err().contains("unresolved external effects"));
        assert_eq!(get_ticket(&store, &task.id).unwrap().revision, waiting.revision);
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "waiting_approval");
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1 AND kind='human_accepted'",
            [&task.id], |row| row.get::<_, i64>(0),
        ).unwrap(), 0);
        // Settling only one receipt does not clear the other uncertainty.
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id=?1 AND effect_key='write'", [&task.id],
        ).unwrap();
        assert!(accept(&store, &task.id, waiting.revision).is_err());
        // Test-only remote reconciliation fixture: production must verify
        // provider state before changing a receipt to applied.
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id=?1 AND effect_key='comment'", [&task.id],
        ).unwrap();
        assert_eq!(accept(&store, &task.id, waiting.revision).unwrap().state, "completed");
    }

    #[test]
    fn disabling_team_quarantines_terminal_pending_effects_and_repeated_off_repairs_legacy_receipts() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private terminal effect", "terminal-effect-off", false,
        ).unwrap();
        assert!(launched);
        stop(&store, &ticket.id).unwrap();
        let terminal_revision = get_ticket(&store, &ticket.id).unwrap().revision;
        let timestamp = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'legacy-first','digest','pending',?2,?2)",
            params![&ticket.id,timestamp],
        ).unwrap();

        let off = change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        assert!(!off.enabled);
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "stopped");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, terminal_revision);
        let first_state: String = store.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key='legacy-first'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(first_state, "uncertain");

        // The Team is already Off, but a legacy or interrupted writer left
        // another pending receipt. Reissuing Off must not silently skip it.
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'legacy-second','digest','pending',?2,?2)",
            params![&ticket.id,timestamp],
        ).unwrap();
        let repaired = change_enabled(&store, &cfg.id, off.policy_revision, false).unwrap();
        assert!(!repaired.enabled);
        assert_eq!(effects::unresolved_count(&store, &ticket.id).unwrap(), 2);
        let uncertain: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts
             WHERE ticket_id=?1 AND state='uncertain'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(uncertain, 2);
        let audit_count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='orchestration_disabled_effect_uncertain'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_count, 2);
        let unchanged = change_enabled(&store, &cfg.id, repaired.policy_revision, false).unwrap();
        assert_eq!(unchanged.policy_revision, repaired.policy_revision);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(effects::unresolved_count(&reopened, &ticket.id).unwrap(), 2);
        let all_uncertain: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts
             WHERE ticket_id=?1 AND state='uncertain'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(all_uncertain, 2, "restart must not replay or bless remote effects");
    }

    #[test]
    fn terminal_pending_effect_off_quarantine_rolls_back_on_audit_failure() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Private legacy effect", "terminal-effect-rollback", false,
        ).unwrap();
        assert!(launched);
        stop(&store, &ticket.id).unwrap();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'legacy-pending','digest','pending',?2,?2)",
            params![&ticket.id,storage::now()],
        ).unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER fail_terminal_effect_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='orchestration_disabled_effect_uncertain'
             BEGIN SELECT RAISE(ABORT,'injected recovery audit failure'); END;",
        ).unwrap();
        assert!(change_enabled(&store, &cfg.id, cfg.policy_revision, false).is_err());
        assert!(configuration(&store, &cfg.id).unwrap().enabled);
        assert_eq!(configuration(&store, &cfg.id).unwrap().policy_revision, cfg.policy_revision);
        let pending: String = store.db.query_row(
            "SELECT state FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key='legacy-pending'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(pending, "pending", "audit failure must roll back the entire Off transaction");
        store.db.execute_batch("DROP TRIGGER fail_terminal_effect_audit").unwrap();
        change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        let uncertain: String = store.db.query_row(
            "SELECT state FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key='legacy-pending'",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(uncertain, "uncertain");
    }

    #[test]
    fn revocation_preserves_external_effect_identity_as_uncertain_atomically() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let receipt_state = |db: &Store, ticket: &str| -> String {
            db.db.query_row("SELECT state FROM sa_effect_receipts WHERE ticket_id=?1",
                [ticket], |r| r.get(0)).unwrap()
        };
        let (first, launched) = admit(&mut store, &cfg.id, "Inspect external receipt",
            "stop-effect-recovery", false).unwrap();
        assert!(launched);
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'effect-1','digest','pending',?2,?2)",
            params![first.id,now],
        ).unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER deny_stop_receipt BEFORE INSERT ON sa_audit
             WHEN NEW.kind='stopped'
             BEGIN SELECT RAISE(ABORT,'injected stop audit failure'); END;",
        ).unwrap();
        assert!(stop(&store, &first.id).is_err());
        assert_eq!(receipt_state(&store, &first.id), "pending");
        assert!(!mandate(&store, &first.id).unwrap().revoked);
        store.db.execute_batch("DROP TRIGGER deny_stop_receipt").unwrap();
        assert_eq!(stop(&store, &first.id).unwrap().0.state, "stopped");
        assert_eq!(receipt_state(&store, &first.id), "uncertain");
        assert!(stop(&store, &first.id).is_err());
        let (second, launched) = admit(&mut store, &cfg.id, "Inspect another effect",
            "disable-effect-recovery", false).unwrap();
        assert!(launched);
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'effect-2','digest','pending',?2,?2)",
            params![second.id,now],
        ).unwrap();
        change_enabled(&store, &cfg.id, cfg.policy_revision, false).unwrap();
        assert_eq!(receipt_state(&store, &second.id), "uncertain");
        assert_eq!(get_ticket(&store, &second.id).unwrap().state, "interrupted");
        assert!(mandate(&store, &second.id).unwrap().revoked);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE state='applied'",
            [], |r| r.get::<_, i64>(0),
        ).unwrap(), 0);
    }

    #[test]
    fn headless_enable_is_revision_bound_and_never_creates_task_authority() {
        let store = fixture();
        let cfg = create_configuration(&store, NewOrchestration {
            name: "Headless-enabled Team".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        assert!(!cfg.enabled);
        assert!(headless_enable_orchestration(&store.path, &cfg.id, 0).is_err());
        let enabled: serde_json::Value = serde_json::from_str(
            &headless_enable_orchestration(&store.path, &cfg.id, cfg.policy_revision).unwrap(),
        ).unwrap();
        assert_eq!(enabled["orchestrationId"], cfg.id);
        assert_eq!(enabled["enabled"], true);
        assert_eq!(enabled["policyRevision"], cfg.policy_revision + 1);
        assert_eq!(enabled["grantIssued"], false);
        assert_eq!(enabled["providerDispatched"], false);
        let again: serde_json::Value = serde_json::from_str(
            &headless_enable_orchestration(&store.path, &cfg.id, cfg.policy_revision + 1).unwrap(),
        ).unwrap();
        assert_eq!(again["policyRevision"], cfg.policy_revision + 1);
        assert!(headless_enable_orchestration(&store.path, &cfg.id, cfg.policy_revision).is_err());
        for table in ["sa_tickets", "sa_mandates", "sa_assignments"] {
            let count: i64 = store.db.query_row(
                &format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0),
            ).unwrap();
            assert_eq!(count, 0, "{table} must not be created by enabling a Team");
        }
        headless_disable_orchestration(&store.path, &cfg.id, cfg.policy_revision + 1).unwrap();
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        assert!(headless_enable_orchestration(&store.path, &cfg.id, cfg.policy_revision + 2).is_err());
        assert!(!configuration(&store, &cfg.id).unwrap().enabled);
        assert_eq!(configuration(&store, &cfg.id).unwrap().policy_revision,
            cfg.policy_revision + 2);
    }

    #[test]
    fn headless_disable_revokes_only_its_team_without_live_store_recovery() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let unrelated = create_configuration(&store, NewOrchestration {
            name: "Other Team".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Active, private work", "headless-disable", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(headless_disable_orchestration(
            &store.path, &cfg.id, cfg.policy_revision + 1,
        ).is_err(), "stale revision must reject the entire policy mutation");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert!(!mandate(&store, &ticket.id).unwrap().revoked);

        let outcome: serde_json::Value = serde_json::from_str(
            &headless_disable_orchestration(&store.path, &cfg.id, cfg.policy_revision).unwrap(),
        ).unwrap();
        assert_eq!(outcome["orchestrationId"], cfg.id);
        assert_eq!(outcome["enabled"], false);
        assert_eq!(outcome["policyRevision"], cfg.policy_revision + 1);
        assert!(!configuration(&store, &cfg.id).unwrap().enabled);
        assert!(!configuration(&store, &unrelated.id).unwrap().enabled);
        assert_eq!(configuration(&store, &unrelated.id).unwrap().policy_revision,
            unrelated.policy_revision, "unrelated Team must be untouched");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "interrupted");
        assert!(read_only_policy(&store, &session).is_err());
        assert!(!on_run_finished(&store, &session, true, "Late provider success").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "interrupted");
        assert!(headless_disable_orchestration(
            &store.path, &cfg.id, cfg.policy_revision,
        ).is_err(), "replayed old revision cannot change policy");
        let repeated: serde_json::Value = serde_json::from_str(
            &headless_disable_orchestration(&store.path, &cfg.id, cfg.policy_revision + 1).unwrap(),
        ).unwrap();
        assert_eq!(repeated["policyRevision"], cfg.policy_revision + 1,
            "idempotent Off does not create a spurious revision");
        assert_eq!(history(&store, &ticket.id, 10).unwrap().iter()
            .filter(|entry| entry.kind == "orchestration_disabled").count(), 1);
    }

    #[test]
    fn headless_ticket_transitions_use_native_guards_and_deny_stopped_callbacks() {
        let mut store = fixture();
        let config = enabled(&store);

        let (accepted, started) = admit(
            &mut store, &config.id, "Evidence to accept", "headless-accept", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &accepted.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Verified evidence").unwrap());
        let waiting = get_ticket(&store, &accepted.id).unwrap();
        let status: serde_json::Value = serde_json::from_str(
            &headless_accept_ticket(&store.path, &accepted.id, waiting.revision).unwrap(),
        ).unwrap();
        assert_eq!(status["state"], "completed");
        assert_eq!(status["revision"], waiting.revision + 1);
        assert!(headless_accept_ticket(&store.path, &accepted.id, waiting.revision).is_err(),
            "stale revision must be denied");
        let completed = get_ticket(&store, &accepted.id).unwrap();
        assert!(headless_accept_ticket(&store.path, &accepted.id, completed.revision).is_err(),
            "terminal state must be denied");
        assert_eq!(history(&store, &accepted.id, 10).unwrap()[0].kind, "human_accepted");

        let (rejected, started) = admit(
            &mut store, &config.id, "Evidence to reject", "headless-reject", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &rejected.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Needs review").unwrap());
        let waiting = get_ticket(&store, &rejected.id).unwrap();
        assert!(headless_reject_ticket(&store.path, &rejected.id, waiting.revision, "  ").is_err(),
            "empty reason must be denied without a transition");
        assert_eq!(get_ticket(&store, &rejected.id).unwrap().state, "waiting_approval");
        assert!(headless_reject_ticket(&store.path, &rejected.id, waiting.revision - 1, "Needs changes").is_err(),
            "stale revision must be denied");
        let status: serde_json::Value = serde_json::from_str(
            &headless_reject_ticket(&store.path, &rejected.id, waiting.revision, "Needs changes").unwrap(),
        ).unwrap();
        assert_eq!(status["state"], "blocked");
        let audit = history(&store, &rejected.id, 10).unwrap();
        assert_eq!(audit[0].kind, "human_rejected");
        assert_eq!(audit[0].detail, "Needs changes");

        let (stopped, started) = admit(
            &mut store, &config.id, "Stop before callback", "headless-stop", false,
        ).unwrap();
        assert!(started);
        let session = assignment_session(&store, &stopped.id).unwrap().unwrap();
        let status: serde_json::Value = serde_json::from_str(
            &headless_stop_ticket(&store.path, &stopped.id).unwrap(),
        ).unwrap();
        assert_eq!(status["state"], "stopped");
        assert!(mandate(&store, &stopped.id).unwrap().revoked);
        assert!(!on_run_finished(&store, &session, true, "Late success after Stop").unwrap());
        assert_eq!(get_ticket(&store, &stopped.id).unwrap().state, "stopped");
        assert_eq!(history(&store, &stopped.id, 10).unwrap()[0].kind, "stopped");
    }

    #[test]
    fn settled_worker_thread_remap_prevents_human_approval_without_replacing_evidence() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &cfg.id, "Check original thread", "thread-proof-worker", false,
        ).unwrap();
        assert!(started);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session_id, true, "Original worker evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(waiting.state, "waiting_approval");

        // A session still exists, and its persisted evidence still matches.
        // The native thread mapping, however, no longer proves task identity.
        store.db.execute(
            "UPDATE bot_threads SET external_id='foreign-task' WHERE session_id=?1",
            [&session_id],
        ).unwrap();
        assert!(accept(&store, &ticket.id, waiting.revision).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
        assert!(!history(&store, &ticket.id, 20).unwrap()
            .iter().any(|event| event.kind == "human_accepted"));

        // The unchanged original binding allows the same explicitly approved
        // evidence. No replacement callback or renewed authorization is used.
        store.db.execute(
            "UPDATE bot_threads SET external_id=?2 WHERE session_id=?1",
            params![session_id, ticket.id],
        ).unwrap();
        assert_eq!(accept(&store, &ticket.id, waiting.revision).unwrap().state, "completed");
    }

    #[test]
    fn settled_coordinator_thread_detachment_blocks_worker_admission() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit_role(
            &mut store, &cfg.id, "Coordinate exact task", "thread-proof-coord", false, true,
        ).unwrap();
        assert!(started);
        let coordinator_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &coordinator_session, true, "Inspect the assigned code").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");

        store.db.execute(
            "DELETE FROM bot_threads WHERE session_id=?1", [&coordinator_session],
        ).unwrap();
        assert!(!coordinator_plan_current(&store, &ticket.id, "router", "project").unwrap());
        assert!(prepare_coordinated_worker(&mut store, &ticket.id, false).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
        assert!(assignment_session(&store, &ticket.id).unwrap().is_none());
    }

    #[test]
    fn human_acceptance_requires_original_worker_session_and_matching_evidence() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, _) = admit(
            &mut store, &cfg.id, "Original worker evidence", "accept-provenance-1", false,
        ).unwrap();
        let original_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &original_session, true, "Verified worker result").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(waiting.state, "waiting_approval");

        // Persisted evidence is not self-authenticating: the native completed
        // assignment and the exact original provider result must still match.
        store.db.execute(
            "UPDATE sa_tickets SET evidence='Replaced by an unrelated result' WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        assert!(accept(&store, &ticket.id, waiting.revision).is_err());
        store.db.execute(
            "UPDATE sa_tickets SET evidence='Verified worker result' WHERE id=?1",
            [&ticket.id],
        ).unwrap();

        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('other','Other','/tmp/sa-foreign-evidence')", [],
        ).unwrap();
        store.db.execute(
            "UPDATE sessions SET project_id='other' WHERE id=?1",
            [&original_session],
        ).unwrap();
        assert!(accept(&store, &ticket.id, waiting.revision).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
        assert!(!history(&store, &ticket.id, 20).unwrap()
            .iter().any(|entry| entry.kind == "human_accepted"));

        // A current, matching origin is sufficient for the explicitly
        // attributed human decision; no extra owner/device step is required.
        store.db.execute(
            "UPDATE sessions SET project_id='project' WHERE id=?1",
            [&original_session],
        ).unwrap();
        assert_eq!(accept(&store, &ticket.id, waiting.revision).unwrap().state, "completed");
    }

    #[test]
    fn human_acceptance_cannot_reuse_evidence_from_a_rejected_previous_attempt() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, _) = admit(
            &mut store, &cfg.id, "Review a bounded task", "accept-attempt-1", false,
        ).unwrap();
        let first_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &first_session, true, "First worker result").unwrap());
        let first = get_ticket(&store, &ticket.id).unwrap();
        let rejected = reject(&store, &ticket.id, first.revision, "Needs corrections").unwrap();
        assert_eq!(rejected.state, "blocked");

        let (running, _, _) = retry_blocked(&mut store, &ticket.id, false).unwrap();
        assert_eq!(running.state, "running");
        let second_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_ne!(first_session, second_session);
        assert!(on_run_finished(&store, &second_session, true, "Second worker result").unwrap());
        let latest = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(latest.state, "waiting_approval");

        // Even if persisted current evidence is replaced to match the
        // earlier accepted provider turn, that superseded attempt cannot
        // satisfy the current Ticket's human acceptance gate.
        store.db.execute(
            "UPDATE sa_assignments SET state='failed' WHERE session_id=?1",
            [&second_session],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_tickets SET evidence='First worker result' WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        assert!(accept(&store, &ticket.id, latest.revision).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
        assert!(!history(&store, &ticket.id, 30).unwrap()
            .iter().any(|entry| entry.kind == "human_accepted"));
    }

    #[test]
    fn request_key_replay_cannot_switch_native_dispatch_roles() {
        let mut store = fixture();
        let cfg = enabled(&store);

        let (direct, launched) = admit(&mut store, &cfg.id, "Inspect", "direct-key", false).unwrap();
        assert!(launched);
        let (same, launched_again) = admit(&mut store, &cfg.id, "Inspect", "direct-key", true).unwrap();
        assert_eq!(same.id, direct.id);
        assert!(!launched_again);
        assert!(admit_role(&mut store, &cfg.id, "Inspect", "direct-key", false, true)
            .unwrap_err().contains("cannot change"));

        // The admission receipt remains authoritative even when a direct
        // ticket never acquired a session because the worker was occupied.
        let (blocked, launched) = admit(&mut store, &cfg.id, "Blocked", "blocked-key", true).unwrap();
        assert!(!launched);
        assert_eq!(blocked.state, "blocked");
        assert!(admit_role(&mut store, &cfg.id, "Blocked", "blocked-key", false, true)
            .unwrap_err().contains("cannot change"));

        stop(&store, &direct.id).unwrap();
        let (coordinated, launched) = admit_role(
            &mut store, &cfg.id, "Coordinate", "coordinated-key", false, true,
        ).unwrap();
        assert!(launched);
        assert!(admit(&mut store, &cfg.id, "Coordinate", "coordinated-key", false)
            .unwrap_err().contains("cannot change"));
        let (same, repeated) = admit_role(
            &mut store, &cfg.id, "Coordinate", "coordinated-key", true, true,
        ).unwrap();
        assert_eq!(coordinated.id, same.id);
        assert!(!repeated);

        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",
            [&cfg.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 3, "role-mismatched replay must not create another ticket");
    }

    #[test]
    fn failed_assignment_insert_cannot_leave_an_orphan_native_session() {
        let mut store = fixture();
        let config = enabled(&store);
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_sa_assignment BEFORE INSERT ON sa_assignments
             BEGIN SELECT RAISE(ABORT,'injected assignment failure'); END;"
        ).unwrap();
        assert!(admit(&mut store, &config.id, "Must roll back", "fault-1", false).is_err());
        for sql in [
            "SELECT COUNT(*) FROM sa_tickets",
            "SELECT COUNT(*) FROM sa_mandates",
            "SELECT COUNT(*) FROM sa_assignments",
            "SELECT COUNT(*) FROM bot_threads WHERE channel_id='superagent'",
            "SELECT COUNT(*) FROM sessions",
        ] {
            let count: i64 = store.db.query_row(sql, [], |r| r.get(0)).unwrap();
            assert_eq!(count, 0, "partial admission remained after rollback: {sql}");
        }
        store.db.execute_batch("DROP TRIGGER reject_sa_assignment").unwrap();
        let (ticket, launch) = admit(&mut store, &config.id, "Must roll back", "fault-1", false).unwrap();
        assert!(launch);
        assert_eq!(ticket.state, "running");
    }

    #[test]
    fn stop_quarantines_detached_native_attempt_and_preserves_restart_evidence() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Stop detached attempt", "stop-orphan-native-1", false,
        ).unwrap();
        assert!(launched);
        let session_id = assignment_session(&store, &ticket.id).unwrap().unwrap();

        // A legacy interrupted state writer lost the active link while its
        // provider session remained running. Stop must not leave a live
        // assignment merely because the current Ticket pointer is absent.
        store.db.execute(
            "UPDATE sa_tickets SET active_assignment_id=NULL WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        let (stopped, direct_session) = stop(&store, &ticket.id).unwrap();
        assert_eq!(stopped.state, "stopped");
        assert!(direct_session.is_none());
        assert!(mandate(&store, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&store, &ticket.id, 10).unwrap()[0].state, "stopped");
        assert_eq!(
            headless_cancelled_assignments(&store, std::slice::from_ref(&session_id)).unwrap(),
            vec![session_id.clone()],
        );
        assert!(read_only_policy(&store, &session_id).is_err());
        assert!(!on_run_finished(&store, &session_id, true, "late completion").unwrap());
        let database_path = store.path.clone();
        drop(store);
        let reopened = Store::open_read_only(&database_path).unwrap();
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "stopped");
        assert_eq!(assignments(&reopened, &ticket.id, 10).unwrap()[0].state, "stopped");
    }

    #[test]
    fn leases_and_revocation_fail_closed_without_mutating_bots() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (a,_)=admit(&mut store,&cfg.id,"A","request-a",false).unwrap();
        let (b,second)=admit(&mut store,&cfg.id,"B","request-b",false).unwrap();
        assert!(!second);
        assert_eq!(b.state,"blocked");
        let sid=assignment_session(&store,&a.id).unwrap().unwrap();
        let (stopped,_)=stop(&store,&a.id).unwrap();
        assert_eq!(stopped.state,"stopped");
        assert!(read_only_policy(&store,&sid).is_err());
        assert!(!on_run_finished(&store,&sid,true,"late completion").unwrap());
        assert!(accept(&store,&a.id,stopped.revision).is_err());
    }

    #[test]
    fn globally_paused_approval_keeps_pending_evidence_and_audit_unchanged() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(&mut store, &cfg.id, "Review evidence", "pause-approval-1", false).unwrap();
        assert!(started);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Worker evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(waiting.state, "waiting_approval");
        let original_history = history(&store, &ticket.id, 200).unwrap();

        // Inject the actual global-pause condition without mutating process
        // settings shared by concurrently executing Rust tests.
        assert!(accept_with_pause(&store, &ticket.id, waiting.revision, true).is_err());
        let unchanged = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(unchanged.state, "waiting_approval");
        assert_eq!(unchanged.revision, waiting.revision);
        assert_eq!(unchanged.evidence, waiting.evidence);
        assert_eq!(history(&store, &ticket.id, 200).unwrap().len(), original_history.len());

        // Resuming does not widen the mandate: the same revision and live
        // authorization are still required for a separate manual decision.
        let accepted = accept_with_pause(&store, &ticket.id, waiting.revision, false).unwrap();
        assert_eq!(accepted.state, "completed");
        assert!(accept_with_pause(&store, &ticket.id, waiting.revision, false).is_err());
        assert_eq!(history(&store, &ticket.id, 200).unwrap()
            .iter().filter(|entry| entry.kind == "human_accepted").count(), 1);
    }

    #[test]
    fn stop_and_accept_roll_back_on_audit_or_storage_failure() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (task, _) = admit(&mut store, &cfg.id, "Abort atomically", "guarded-stop-1", false).unwrap();
        let session = assignment_session(&store, &task.id).unwrap().unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_sa_stop BEFORE UPDATE OF state ON sa_tickets
             WHEN NEW.state='stopped'
             BEGIN SELECT RAISE(ABORT,'injected stop failure'); END;"
        ).unwrap();
        assert!(stop(&store, &task.id).is_err());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "running");
        let revoked: i64 = store.db.query_row(
            "SELECT revoked FROM sa_mandates WHERE ticket_id=?1", [&task.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(revoked, 0, "failed Stop must not partially revoke a mandate");
        store.db.execute_batch("DROP TRIGGER reject_sa_stop").unwrap();

        assert!(on_run_finished(&store, &session, true, "Recorded evidence").unwrap());
        let waiting = get_ticket(&store, &task.id).unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_sa_accept BEFORE INSERT ON sa_audit
             WHEN NEW.kind='human_accepted'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;"
        ).unwrap();
        assert!(accept(&store, &task.id, waiting.revision).is_err());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "waiting_approval");
        store.db.execute_batch("DROP TRIGGER reject_sa_accept").unwrap();
        let accepted = accept(&store, &task.id, waiting.revision).unwrap();
        assert_eq!(accepted.state, "completed");
    }

    #[test]
    fn explicit_human_rejection_is_atomic_revisioned_and_retry_requires_fresh_evidence() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(&mut store, &cfg.id, "Review artifact", "reject-1", false).unwrap();
        assert!(started);
        let original_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &original_session, true, "Old rejected evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(waiting.state, "waiting_approval");

        assert!(reject(&store, &ticket.id, waiting.revision - 1, "Wrong revision").is_err());
        assert!(reject(&store, &ticket.id, waiting.revision, " ").is_err());
        assert!(reject(&store, &ticket.id, waiting.revision, &"x".repeat(501)).is_err());

        store.db.execute_batch(
            "CREATE TEMP TRIGGER deny_rejection_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='human_rejected'
             BEGIN SELECT RAISE(ABORT,'injected rejection audit failure'); END;"
        ).unwrap();
        assert!(reject(&store, &ticket.id, waiting.revision, "Needs changes").is_err());
        let unchanged = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(unchanged.state, "waiting_approval");
        assert_eq!(unchanged.revision, waiting.revision);
        store.db.execute_batch("DROP TRIGGER deny_rejection_audit").unwrap();

        let rejected = reject(&store, &ticket.id, waiting.revision, "Needs changes").unwrap();
        assert_eq!(rejected.state, "blocked");
        assert_eq!(rejected.evidence.as_deref(), Some("Old rejected evidence"));
        assert!(accept(&store, &ticket.id, rejected.revision).is_err());
        assert!(reject(&store, &ticket.id, rejected.revision, "Again").is_err());
        assert_eq!(history(&store, &ticket.id, 20).unwrap().iter()
            .filter(|entry| entry.kind == "human_rejected").count(), 1);

        let (retry, _, _) = retry_blocked(&mut store, &ticket.id, false).unwrap();
        assert_eq!(retry.state, "running");
        assert_eq!(retry.evidence, None, "rejected evidence must not follow a new worker");
        let fresh_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_ne!(fresh_session, original_session);
        assert!(!on_run_finished(&store, &original_session, true, "Late old result").unwrap());
        assert!(on_run_finished(&store, &fresh_session, true, "New evidence").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().evidence.as_deref(), Some("New evidence"));
    }

    #[test]
    fn busy_worker_ticket_requires_manual_retry_and_uses_fresh_native_session() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (first,launch)=admit(&mut store,&cfg.id,"A","queue-a",false).unwrap();
        assert!(launch);
        let (second,launch)=admit(&mut store,&cfg.id,"B","queue-b",false).unwrap();
        assert!(!launch);
        assert_eq!(second.state,"blocked");
        assert!(retry_blocked(&mut store,&second.id,true).is_err());
        assert!(retry_blocked(&mut store,&second.id,false).is_err());
        let session_a=assignment_session(&store,&first.id).unwrap().unwrap();
        assert!(on_run_finished(&store,&session_a,true,"A evidence").unwrap());
        let (retried,thread,worker)=retry_blocked(&mut store,&second.id,false).unwrap();
        assert_eq!(retried.state,"running");
        assert_eq!(worker,"worker");
        assert_eq!(thread,format!("{}:attempt:1",second.id));
        let session_b=assignment_session(&store,&retried.id).unwrap().unwrap();
        assert_ne!(session_a,session_b);
        assert!(!on_run_finished(&store,&session_a,true,"late A").unwrap());
        // The first allocated attempt belongs to a user-issued retry of an
        // initially blocked ticket. Its suffixed thread must pass both gates.
        assert!(guard_gateway_inbound(&store, "worker", &thread, "local-user", true).is_ok());
        assert!(guard_gateway_inbound(&store, "worker", &second.id, "local-user", true).is_err());
        assert!(read_only_policy(&store,&session_b).unwrap().is_some());
        assert!(on_run_finished(&store,&session_b,false,"failed B").unwrap());
        assert_eq!(get_ticket(&store,&second.id).unwrap().state,"failed");
        assert!(retry_blocked(&mut store,&second.id,false).is_err());
    }

    #[test]
    fn first_attempt_suffix_requires_an_actual_blocked_manual_retry() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (direct, started) = admit(&mut store, &cfg.id, "Direct", "suffix-reject", false).unwrap();
        assert!(started);
        let sid = assignment_session(&store, &direct.id).unwrap().unwrap();
        let forged = format!("{}:attempt:1", direct.id);
        // A forged suffix on a regular attempt-one thread cannot impersonate
        // a user-issued retry of an originally blocked ticket.
        store.db.execute(
            "UPDATE bot_threads SET external_id=?2 WHERE session_id=?1",
            params![sid, forged],
        ).unwrap();
        assert!(read_only_policy(&store, &sid).is_err());
        assert!(guard_gateway_inbound(&store, "worker", &forged, "local-user", true).is_err());
        assert!(on_run_finished(&store, &sid, true, "Forged result").unwrap());
        assert_eq!(get_ticket(&store, &direct.id).unwrap().state, "blocked");
        assert!(!history(&store, &direct.id, 10).unwrap()
            .iter().any(|entry| entry.kind == "human_accepted"));
    }

    #[test]
    fn retry_after_launch_failure_is_bounded_and_does_not_widen_mandate() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (first,_)=admit(&mut store,&cfg.id,"Recover once","recover-1",false).unwrap();
        fail_launch(&store,&first.id,"provider unavailable").unwrap();
        let (retried,_,_)=retry_blocked(&mut store,&first.id,false).unwrap();
        assert_eq!(retried.state,"running");
        let sid=assignment_session(&store,&first.id).unwrap().unwrap();
        assert!(read_only_policy(&store,&sid).unwrap().is_some());
        fail_launch(&store,&first.id,"provider unavailable again").unwrap();
        assert!(retry_blocked(&mut store,&first.id,false).is_err());
        let allowed:i64=store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&first.id],|r|r.get(0),
        ).unwrap();
        assert_eq!(allowed,0);
    }

    #[test]
    fn native_bot_reset_and_deletion_invalidate_active_work_but_keep_history() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (running, _) = admit(&mut store, &cfg.id, "Interrupted by reset", "reset-work", false).unwrap();
        let sid = assignment_session(&store, &running.id).unwrap().unwrap();
        bots::reset(&store, "worker").unwrap();
        assert_eq!(get_ticket(&store, &running.id).unwrap().state, "interrupted");
        assert!(!on_run_finished(&store, &sid, true, "late worker result").unwrap());
        let revoked: i64 = store.db.query_row(
            "SELECT revoked FROM sa_mandates WHERE ticket_id=?1",
            [&running.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(revoked, 1);
        // Reset keeps the bot identity; a new manual request can be made.
        let (next, launched) = admit(&mut store, &cfg.id, "New request", "delete-work", false).unwrap();
        assert!(launched);
        bots::delete(&store, "worker").unwrap();
        assert_eq!(get_ticket(&store, &next.id).unwrap().state, "interrupted");
        assert!(!configuration(&store, &cfg.id).unwrap().enabled);
        assert_eq!(get_ticket(&store, &running.id).unwrap().state, "interrupted");
        let history: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&running.id], |r| r.get(0),
        ).unwrap();
        assert!(history >= 2);
    }

    #[test]
    fn resetting_coordinator_revokes_inflight_and_queued_plans_without_losing_history() {
        let mut store = fixture();
        let config = enabled(&store);
        let (running, launched) = admit_role(
            &mut store, &config.id, "Plan before reset", "coordinator-reset-running", false, true,
        ).unwrap();
        assert!(launched);
        let running_session = assignment_session(&store, &running.id).unwrap().unwrap();
        assert_eq!(bots::owner_of(&store, &running_session).unwrap().unwrap().0.id, "router");

        // A successful coordinator turn only produces advice and a queued task.
        let (queued, _) = admit_role(
            &mut store, &config.id, "Keep this plan", "coordinator-reset-queued", true, false,
        ).unwrap();
        assert_eq!(queued.state, "blocked");
        assert!(on_run_finished(&store, &running_session, true, "Plan evidence").unwrap());
        assert_eq!(get_ticket(&store, &running.id).unwrap().state, "queued");
        assert!(assignment_session(&store, &running.id).unwrap().is_none());

        bots::reset(&store, "router").unwrap();
        let affected = get_ticket(&store, &running.id).unwrap();
        assert_eq!(affected.state, "blocked");
        let mandate_revoked: i64 = store.db.query_row(
            "SELECT revoked FROM sa_mandates WHERE ticket_id=?1",
            [&running.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(mandate_revoked, 1);
        assert!(prepare_coordinated_worker(&mut store, &running.id, false).is_err());
        assert!(!on_run_finished(&store, &running_session, true, "late plan").unwrap());
        assert!(history(&store, &running.id, 100).unwrap()
            .iter().any(|entry| entry.kind == "bot_reset"));

        // A coordinator running without a settled plan is also revoked.
        let (next, _) = admit_role(
            &mut store, &config.id, "Another plan", "coordinator-reset-again", false, true,
        ).unwrap();
        let next_session = assignment_session(&store, &next.id).unwrap().unwrap();
        bots::reset(&store, "router").unwrap();
        assert_eq!(get_ticket(&store, &next.id).unwrap().state, "interrupted");
        assert_eq!(get_ticket(&store, &queued.id).unwrap().state, "blocked");
        assert!(!on_run_finished(&store, &next_session, true, "stale").unwrap());
        assert_eq!(history(&store, &next.id, 100).unwrap()[0].kind, "bot_reset");
    }

    #[test]
    fn ticket_history_survives_bot_and_session_deletion_and_reopen() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket,_)=admit(&mut store,&cfg.id,"Persist","persist-1",false).unwrap();
        store.db.execute("DELETE FROM bots WHERE id='worker'",[]).unwrap();
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().objective,"Persist");
        let count:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",[&ticket.id],|r|r.get(0)
        ).unwrap();
        assert_eq!(count,1);
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        assert_eq!(get_ticket(&reopened,&ticket.id).unwrap().state,"interrupted");
    }

    #[test]
    fn failed_launch_audit_is_atomic_and_releases_the_worker_only_on_commit() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(&mut store, &config.id, "Retry native launch", "launch-fault-1", false).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_sa_launch_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='launch_failed'
             BEGIN SELECT RAISE(ABORT,'injected launch audit failure'); END;"
        ).unwrap();
        assert!(fail_launch(&store, &ticket.id, "temporary provider failure").is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "running");
        assert_eq!(assignment_session(&store, &ticket.id).unwrap(), Some(session.clone()));
        assert!(read_only_policy(&store, &session).unwrap().is_some());
        let pending: String = store.db.query_row(
            "SELECT state FROM sa_assignments WHERE session_id=?1", [&session], |r| r.get(0),
        ).unwrap();
        assert_eq!(pending, "running");
        store.db.execute_batch("DROP TRIGGER reject_sa_launch_audit").unwrap();
        fail_launch(&store, &ticket.id, "temporary provider failure").unwrap();
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked");
        assert!(assignment_session(&store, &ticket.id).unwrap().is_none());
        assert!(fail_launch(&store, &ticket.id, "stale duplicate").is_err());
        let failures: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1 AND kind='launch_failed'",
            [&ticket.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(failures, 1);
        let (retried, _, _) = retry_blocked(&mut store, &ticket.id, false).unwrap();
        assert_eq!(retried.state, "running");
        assert_ne!(assignment_session(&store, &ticket.id).unwrap(), Some(session));
    }

    #[test]
    fn native_mandate_summary_retains_exact_grant_after_stop_and_restart() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(&mut store, &cfg.id, "Inspect without effects", "mandate-view", false).unwrap();
        assert!(launched);
        let granted = mandate(&store, &ticket.id).unwrap();
        assert_eq!(granted.ticket_id, ticket.id);
        assert_eq!(granted.project_id, cfg.project_id);
        assert_eq!(granted.actor, "local-user");
        assert!(!granted.authorization_id.is_empty());
        assert_eq!(granted.policy_revision, cfg.policy_revision);
        assert!(granted.expires_at > granted.created_at);
        assert!(!granted.revoked);
        assert!(granted.allowed_tools.is_empty());
        assert!(!granted.allowed_side_effects);
        assert!(mandate(&store, "not-a-ticket").is_err());
        let (stopped, _) = stop(&store, &ticket.id).unwrap();
        assert_eq!(stopped.state, "stopped");
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let after = mandate(&reopened, &ticket.id).unwrap();
        assert!(after.revoked);
        assert_eq!(after.authorization_id, granted.authorization_id);
        assert_eq!(after.policy_revision, granted.policy_revision);
        assert_eq!(after.project_id, granted.project_id);
        assert!(!after.allowed_side_effects);
        assert!(after.allowed_tools.is_empty());
    }

    #[test]
    fn native_assignment_history_is_bounded_correlated_and_survives_restart() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Trace a failed launch", "assignment-history-1", false,
        ).unwrap();
        assert!(launched);
        let first_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        fail_launch(&store, &ticket.id, "provider unavailable").unwrap();
        let (_, _, _) = retry_blocked(&mut store, &ticket.id, false).unwrap();
        let second_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_ne!(first_session, second_session);
        let attempts = assignments(&store, &ticket.id, 100).unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!((attempts[0].attempt, attempts[0].state.as_str()), (2, "running"));
        assert_eq!((attempts[1].attempt, attempts[1].state.as_str()), (1, "failed"));
        assert_eq!(attempts[1].session_id, first_session);
        assert_eq!(attempts[1].result.as_deref(), Some("provider unavailable"));
        assert!(attempts.iter().all(|a| a.ticket_id == ticket.id && a.bot_id == "worker"));
        assert_eq!(assignments(&store, &ticket.id, 1).unwrap().len(), 1);
        assert!(assignments(&store, &ticket.id, 0).is_err());
        assert!(assignments(&store, &ticket.id, 201).is_err());
        assert!(assignments(&store, "unknown-ticket", 20).is_err());

        assert!(on_run_finished(&store, &second_session, true, "Result evidence").unwrap());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let persisted = assignments(&reopened, &ticket.id, 200).unwrap();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[0].state, "finished");
        assert_eq!(persisted[0].result.as_deref(), Some("Result evidence"));
        assert_eq!(persisted[1].state, "failed");
        assert_eq!(get_ticket(&reopened, &ticket.id).unwrap().state, "waiting_approval");
    }

    #[test]
    fn native_ticket_index_and_audit_survive_restart_without_leaking_other_presets() {
        let mut store = fixture();
        let first_config = enabled(&store);
        let other_config = create_configuration(&store, NewOrchestration {
            name: "Another preset".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let other_config = change_enabled(&store, &other_config.id, 1, true).unwrap();
        let (first, _) = admit(&mut store, &first_config.id, "First task", "index-a", false).unwrap();
        let (second, _) = admit(&mut store, &first_config.id, "Second task", "index-b", false).unwrap();
        let (foreign, _) = admit(&mut store, &other_config.id, "Private preset task", "index-c", false).unwrap();
        let listed = list_tickets(&store, &first_config.id, 200).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|ticket| ticket.id == first.id));
        assert!(listed.iter().any(|ticket| ticket.id == second.id));
        assert!(listed.iter().all(|ticket| ticket.id != foreign.id));
        assert!(list_tickets(&store, &first_config.id, 0).is_err());
        assert!(list_tickets(&store, "unknown-orchestration", 100).is_err());
        assert_eq!(list_tickets(&store, &first_config.id, 1).unwrap().len(), 1);
        let events = history(&store, &first.id, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "admitted");
        assert!(history(&store, &first.id, 201).is_err());
        assert!(history(&store, "unknown-ticket", 100).is_err());
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(list_tickets(&reopened, &first_config.id, 100).unwrap().len(), 2);
        let replay = history(&reopened, &first.id, 100).unwrap();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].kind, "restart_interrupted");
        assert_eq!(replay[1].kind, "admitted");
        assert_eq!(get_ticket(&reopened, &first.id).unwrap().state, "interrupted");
    }

    #[test]
    fn coordinated_manual_ticket_requires_native_coordinator_evidence_before_worker() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launch) = admit_role(
            &mut store,&cfg.id,"Review source first","coord-1",false,true,
        ).unwrap();
        assert!(launch);
        let coordinator_session = assignment_session(&store,&ticket.id).unwrap().unwrap();
        let coordinator_owner = bots::owner_of(&store,&coordinator_session).unwrap().unwrap().0;
        assert_eq!(coordinator_owner.id, "router");
        assert!(read_only_policy(&store,&coordinator_session).unwrap().is_some());
        assert!(prepare_coordinated_worker(&mut store,&ticket.id,false).is_err());
        assert!(on_run_finished(&store,&coordinator_session,true,"Worker should inspect tests").unwrap());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"queued");
        assert!(accept(&store,&ticket.id,get_ticket(&store,&ticket.id).unwrap().revision).is_err());
        let (running,_,worker_id,plan) = prepare_coordinated_worker(&mut store,&ticket.id,false).unwrap();
        assert_eq!(worker_id,"worker");
        assert_eq!(plan,"Worker should inspect tests");
        assert_eq!(running.state,"running");
        let worker_session = assignment_session(&store,&ticket.id).unwrap().unwrap();
        assert_ne!(worker_session,coordinator_session);
        assert!(read_only_policy(&store,&worker_session).unwrap().is_some());
        assert!(!on_run_finished(&store,&coordinator_session,true,"stale coordinator").unwrap());
        let effects: i64=store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket.id],|r|r.get(0),
        ).unwrap();
        assert_eq!(effects,0);
        assert!(on_run_finished(&store,&worker_session,true,"Worker evidence").unwrap());
        let waiting = get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(waiting.state,"waiting_approval");
        assert_eq!(accept(&store,&ticket.id,waiting.revision).unwrap().state,"completed");
        assert!(prepare_coordinated_worker(&mut store,&ticket.id,false).is_err());
    }

    #[test]
    fn failed_coordinator_cannot_be_retried_as_an_unguarded_worker() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, _) = admit_role(
            &mut store,&cfg.id,"Refuse bypass","coord-fail",false,true,
        ).unwrap();
        fail_launch(&store,&ticket.id,"Coordinator unavailable").unwrap();
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"blocked");
        assert!(retry_blocked(&mut store,&ticket.id,false).is_err());
        assert!(prepare_coordinated_worker(&mut store,&ticket.id,false).is_err());
    }

    #[test]
    fn oversized_manual_objective_is_rejected_before_durable_or_gateway_dispatch() {
        let mut store=fixture();
        let config=enabled(&store);
        let long_objective="x".repeat(MAX_OBJECTIVE_BYTES+1);
        assert!(admit(&mut store,&config.id,&long_objective,"oversize-direct",false).is_err());
        assert!(admit_role(&mut store,&config.id,&long_objective,"oversize-coordinator",false,true).is_err());
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count,0);
        assert!(admit(&mut store,&config.id,&"x".repeat(MAX_OBJECTIVE_BYTES),"max-accepted",false).is_ok());
    }

    #[test]
    fn unresolved_checkout_identity_blocks_native_and_ordinary_session_races() {
        let mut store = fixture();
        let cfg = enabled(&store);
        // A NUL makes filesystem identity unresolvable on all platforms.
        // A rejected path must not be interpreted as a separate checkout.
        let opaque = format!("unknown-{}\0checkout", storage::id());
        assert!(canonical_checkout_intent(&opaque).is_none());
        let root = std::env::temp_dir().join(format!("sa-valid-{}", storage::id()));
        let sibling = std::env::temp_dir().join(format!("sa-other-{}", storage::id()));
        assert!(!checkout_paths_overlap(&root.to_string_lossy(), &sibling.to_string_lossy()),
            "Resolvable unrelated checkouts remain independent");
        assert!(checkout_paths_overlap(&opaque, &root.to_string_lossy()),
            "Unresolvable checkout identity must fail closed");

        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('opaque-project','Opaque',?1)",
            [&opaque],
        ).unwrap();
        let ordinary = store.create_session("opaque-project", "codex", "default", "ask").unwrap();
        assert!(active_manual_conflict(
            &store, std::slice::from_ref(&ordinary.id), &cfg.worker_bot_id, &cfg.project_id,
        ).unwrap());

        // Simulate an ordinary provider arriving after the initial Ticket
        // reservation; the second native admission fence must still deny it.
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Inspect only", "opaque-checkout-race", false,
        ).unwrap();
        assert!(launched);
        let native = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(guard_run_admission(
            &store, &native, std::slice::from_ref(&ordinary.id),
        ).is_err());
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_err());
        fail_launch(&store, &ticket.id, "No concurrent checkout access").unwrap();
    }

    #[test]
    fn nested_checkout_roots_fence_native_and_ordinary_sessions_but_not_siblings() {
        let mut store = fixture();
        let root = std::env::temp_dir().join(format!(
            "sa-nested-checkout-{}", storage::id(),
        ));
        let checkout = root.join("workspace");
        let child = checkout.join("service");
        let sibling = root.join("workspace-other");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let checkout_path_text = checkout.to_string_lossy().to_string();
        let child_path_text = child.to_string_lossy().to_string();
        let sibling_path_text = sibling.to_string_lossy().to_string();

        // Lexical prefix matches are not sufficient; only path components and
        // resolved existing ancestors define a shared writeable resource.
        assert!(!checkout_paths_equivalent(&checkout_path_text, &child_path_text));
        assert!(checkout_paths_overlap(&checkout_path_text, &child_path_text));
        assert!(checkout_paths_overlap(
            &checkout_path_text,
            &child.join("future").to_string_lossy(),
        ));
        assert!(!checkout_paths_overlap(&checkout_path_text, &sibling_path_text));

        store.db.execute(
            "UPDATE projects SET path=?1 WHERE id='project'",
            [&checkout_path_text],
        ).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('child-project','Child',?1)",
            [&child_path_text],
        ).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('sibling-project','Sibling',?1)",
            [&sibling_path_text],
        ).unwrap();
        let config = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &config.id, "Fence nested checkouts", "nested-checkout", false,
        ).unwrap();
        assert!(launched);
        let native_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let child_session = store.create_session(
            "child-project", "codex", "default", "plan",
        ).unwrap();
        let sibling_session = store.create_session(
            "sibling-project", "codex", "default", "plan",
        ).unwrap();

        assert!(active_manual_conflict(
            &store, std::slice::from_ref(&child_session.id), "worker", "project",
        ).unwrap());
        assert!(!active_manual_conflict(
            &store, std::slice::from_ref(&sibling_session.id), "worker", "project",
        ).unwrap());
        assert!(manual_candidate_occupied(
            &store, "different-bot", "child-project", "child-project",
        ).unwrap());
        assert!(!manual_candidate_occupied(
            &store, "different-bot", "sibling-project", "sibling-project",
        ).unwrap());
        assert!(guard_run_admission(&store, &child_session.id, &[]).is_err());
        assert!(guard_run_admission(&store, &sibling_session.id, &[]).is_ok());
        assert!(guard_run_admission(
            &store, &native_session, std::slice::from_ref(&child_session.id),
        ).is_err());
        assert!(guard_run_admission(
            &store, &native_session, std::slice::from_ref(&sibling_session.id),
        ).is_ok());

        stop(&store, &ticket.id).unwrap();
        assert!(!manual_candidate_occupied(
            &store, "different-bot", "child-project", "child-project",
        ).unwrap());
        assert!(guard_run_admission(&store, &child_session.id, &[]).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn physical_checkout_aliases_block_native_and_ordinary_cross_project_admission() {
        let mut store=fixture();
        let checkout=std::env::temp_dir().join(format!(
            "sa-checkout-identity-{}",storage::id(),
        ));
        std::fs::create_dir_all(&checkout).unwrap();
        let physical=checkout.to_string_lossy().to_string();
        let alias=checkout.join(".").to_string_lossy().to_string();
        assert_ne!(physical,alias);
        assert!(checkout_paths_equivalent(&physical,&alias));
        store.db.execute(
            "UPDATE projects SET path=?1 WHERE id='project'",[&physical],
        ).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('alias-project','Alias',?1)",
            [&alias],
        ).unwrap();
        let cfg=enabled(&store);
        let (ticket,started)=admit(
            &mut store,&cfg.id,"Protect the physical checkout","physical-alias",false,
        ).unwrap();
        assert!(started);
        let native=assignment_session(&store,&ticket.id).unwrap().unwrap();
        let ordinary=store.create_session(
            "alias-project","codex","default","plan",
        ).unwrap();
        assert!(active_manual_conflict(
            &store,std::slice::from_ref(&ordinary.id),"worker","project",
        ).unwrap());
        assert!(manual_candidate_occupied(
            &store,"unrelated-worker","alias-project","alias-project",
        ).unwrap());
        assert!(guard_run_admission(&store,&ordinary.id,&[]).is_err());
        assert!(guard_run_admission(
            &store,&native,std::slice::from_ref(&ordinary.id),
        ).is_err());
        stop(&store,&ticket.id).unwrap();
        assert!(!manual_candidate_occupied(
            &store,"unrelated-worker","alias-project","alias-project",
        ).unwrap());
        assert!(guard_run_admission(&store,&ordinary.id,&[]).is_ok());
        std::fs::remove_dir(&checkout).unwrap();
    }

    #[test]
    fn unresolved_dot_checkout_alias_fences_native_leases_without_path_creation() {
        let mut store = fixture();
        let root = std::env::temp_dir().join(format!(
            "sa-unresolved-checkout-{}", storage::id(),
        ));
        let original = root.join("checkout").to_string_lossy().to_string();
        let alias = root.join(".").join("checkout").to_string_lossy().to_string();
        let unrelated = root.join("other").to_string_lossy().to_string();
        assert!(!root.exists());
        assert_ne!(original, alias);
        assert!(checkout_paths_equivalent(&original, &alias));
        assert!(!checkout_paths_equivalent(&original, &unrelated));
        store.db.execute(
            "UPDATE projects SET path=?1 WHERE id='project'",
            [&original],
        ).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('alias-project','Alias',?1)",
            [&alias],
        ).unwrap();
        let config = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &config.id, "Keep an uncreated checkout reserved",
            "unresolved-dot-alias", false,
        ).unwrap();
        assert!(started);
        let ordinary = store.create_session(
            "alias-project", "codex", "default", "plan",
        ).unwrap();
        assert!(active_manual_conflict(
            &store, std::slice::from_ref(&ordinary.id), "worker", "project",
        ).unwrap());
        assert!(manual_candidate_occupied(
            &store, "unrelated-worker", "alias-project", "alias-project",
        ).unwrap());
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_err());
        stop(&store, &ticket.id).unwrap();
        assert!(!manual_candidate_occupied(
            &store, "unrelated-worker", "alias-project", "alias-project",
        ).unwrap());
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_ok());
    }

    #[test]
    fn unresolved_relative_and_absolute_checkout_aliases_share_native_identity() {
        let relative = std::path::PathBuf::from(format!(
            "sa-uncreated-relative-{}", storage::id(),
        )).join("checkout");
        let absolute = std::env::current_dir().unwrap().join(&relative);
        assert!(!absolute.exists());
        assert!(checkout_paths_equivalent(
            &relative.to_string_lossy(), &absolute.to_string_lossy(),
        ));
        let unrelated = absolute.with_file_name("another-checkout");
        assert!(!checkout_paths_equivalent(
            &relative.to_string_lossy(), &unrelated.to_string_lossy(),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unresolved_symlinked_parent_checkout_alias_keeps_native_lease() {
        let mut store = fixture();
        let root = std::env::temp_dir().join(format!(
            "sa-pending-symlink-checkout-{}", storage::id(),
        ));
        let physical = root.join("physical");
        let alias = root.join("alias");
        std::fs::create_dir_all(&physical).unwrap();
        std::os::unix::fs::symlink(&physical, &alias).unwrap();
        let pending = physical.join("future").join("checkout");
        let pending_alias = alias.join("future").join("checkout");
        assert!(!pending.exists() && !pending_alias.exists());
        let original = pending.to_string_lossy().to_string();
        let alternate = pending_alias.to_string_lossy().to_string();
        assert!(checkout_paths_equivalent(&original, &alternate));
        assert!(!checkout_paths_equivalent(
            &original, &alias.join("future").join("different").to_string_lossy(),
        ));

        store.db.execute(
            "UPDATE projects SET path=?1 WHERE id='project'",
            [&original],
        ).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('unresolved-alias','Alias',?1)",
            [&alternate],
        ).unwrap();
        let config = enabled(&store);
        let (ticket, started) = admit(
            &mut store, &config.id, "Fence an uncreated symlinked checkout",
            "unresolved-symlink-checkout", false,
        ).unwrap();
        assert!(started);
        let ordinary = store.create_session(
            "unresolved-alias", "codex", "default", "plan",
        ).unwrap();
        assert!(manual_candidate_occupied(
            &store, "other-worker", "unresolved-alias", "unresolved-alias",
        ).unwrap());
        assert!(active_manual_conflict(
            &store, std::slice::from_ref(&ordinary.id), "worker", "project",
        ).unwrap());
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_err());
        stop(&store, &ticket.id).unwrap();
        assert!(!manual_candidate_occupied(
            &store, "other-worker", "unresolved-alias", "unresolved-alias",
        ).unwrap());
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_ok());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn native_bot_lease_fences_stale_ordinary_session_in_another_project() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('old-project','Old workspace','/tmp/sa-old-session-project')",
            [],
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET project_id='old-project' WHERE id='worker'", [],
        ).unwrap();
        let previous_worker = bots::get(&store, "worker").unwrap();
        let (old_session, _) = bots::thread_session(
            &store, &previous_worker, bots::DESKTOP, "old-workspace-chat", "Older conversation",
        ).unwrap();
        // A bot can change its current project while its earlier session is
        // still active. The stale session must not race a fresh Team lease.
        store.db.execute(
            "UPDATE bots SET project_id='project' WHERE id='worker'", [],
        ).unwrap();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Read-only inspection", "same-bot-other-project", false,
        ).unwrap();
        assert!(launched);
        let native_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(guard_run_admission(&store, &native_session, &[]).is_ok());
        assert!(guard_run_admission(
            &store, &native_session, std::slice::from_ref(&old_session),
        ).is_err());
        assert!(guard_run_admission(&store, &old_session, &[]).is_err());
        fail_launch(&store, &ticket.id, "Release exclusive bot lease").unwrap();
        assert!(guard_run_admission(&store, &old_session, &[]).is_ok());
    }

    #[test]
    fn native_run_admission_closes_cross_session_project_races() {
        let mut store = fixture();
        let config = enabled(&store);
        let (ticket, _) = admit(
            &mut store, &config.id, "Read-only project inspection", "project-admission-1", false,
        ).unwrap();
        let superagent_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let worker = bots::get(&store, "worker").unwrap();
        let (ordinary_session, _) = bots::thread_session(
            &store, &worker, bots::DESKTOP, "ordinary-conversation", "Normal chat",
        ).unwrap();
        assert!(guard_run_admission(&store, &superagent_session, &[]).is_ok());
        assert!(guard_run_admission(&store, &ordinary_session, &[]).is_err());
        assert!(guard_run_admission(
            &store, &superagent_session, std::slice::from_ref(&ordinary_session),
        ).is_err());

        // Even an unrelated direct session in the same checkout is not safe
        // to start during a SuperAgent lease.
        let ordinary = store.create_session("project", "codex", "default", "plan").unwrap();
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_err());
        assert!(guard_run_admission(
            &store, &superagent_session, std::slice::from_ref(&ordinary.id),
        ).is_err());
        fail_launch(&store, &ticket.id, "leave the project untouched").unwrap();
        assert!(guard_run_admission(&store, &ordinary.id, &[]).is_ok());
        assert!(guard_run_admission(&store, &ordinary_session, &[]).is_ok());
        assert!(guard_run_admission(&store, &superagent_session, &[]).is_err());
    }

    #[test]
    fn settled_native_provider_keeps_checkout_fenced_until_process_exit() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket, started)=admit(
            &mut store,&cfg.id,"Read-only task that is stopping",
            "settled-provider-run-fence",false,
        ).unwrap();
        assert!(started);
        let native=assignment_session(&store,&ticket.id).unwrap().unwrap();
        let worker=bots::get(&store,"worker").unwrap();
        let ordinary=bots::thread_session(
            &store,&worker,bots::DESKTOP,"ordinary-after-stop","Normal chat",
        ).unwrap().0;
        stop(&store,&ticket.id).unwrap();

        // The durable lease is gone, but a delayed provider exit still
        // occupies the task's checkout and bot. No new ordinary agent
        // may execute there until the live agent registry removes it.
        assert!(guard_run_admission(&store,&ordinary,&[]).is_ok());
        let error=guard_run_admission(
            &store,&ordinary,std::slice::from_ref(&native),
        ).unwrap_err();
        assert!(error.contains("still running on this bot or checkout"));

        // A separate project and separate bot remain independently usable.
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('independent','Independent','/tmp/disposable-superagent-independent')",
            [],
        ).unwrap();
        let independent=store.create_session(
            "independent","codex","default","plan",
        ).unwrap();
        assert!(guard_run_admission(
            &store,&independent.id,std::slice::from_ref(&native),
        ).is_ok());
        assert!(guard_run_admission(&store,&ordinary,&[]).is_ok());
    }

    #[test]
    fn project_resource_lease_serializes_distinct_native_workers_and_retry() {
        let mut store=fixture();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('other-worker','Second Builder','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        let first_cfg=enabled(&store);
        let second_cfg=create_configuration(&store,NewOrchestration {
            name:"Same project other worker".into(),
            coordinator_bot_id:"router".into(),
            worker_bot_id:"other-worker".into(),
        }).unwrap();
        let second_cfg=change_enabled(&store,&second_cfg.id,1,true).unwrap();
        let (first,started)=admit(&mut store,&first_cfg.id,"First","resource-a",false).unwrap();
        assert!(started);
        let (second,other_started)=admit(&mut store,&second_cfg.id,"Second","resource-b",false).unwrap();
        assert!(!other_started);
        assert_eq!(second.state,"blocked");
        // A bypassing SQL caller must hit the same native lease, including
        // on the distinct worker ID and on a separate session.
        let blocked=store.db.execute(
            "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES('illegal-lease',?1,'other-worker','illegal-session','running',1,?2,?2)",
            params![second.id,storage::now()],
        );
        assert!(blocked.is_err());
        let sid=assignment_session(&store,&first.id).unwrap().unwrap();
        assert!(on_run_finished(&store,&sid,true,"Evidence").unwrap());
        let (resumed,_,worker)=retry_blocked(&mut store,&second.id,false).unwrap();
        assert_eq!(resumed.state,"running");
        assert_eq!(worker,"other-worker");
        assert_ne!(assignment_session(&store,&second.id).unwrap(),Some(sid));
    }

    #[test]
    fn startup_interruption_is_audited_once_and_old_callbacks_remain_denied() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, _) = admit(&mut store, &cfg.id, "Interrupted restart", "restart-ledger", false).unwrap();
        let original_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        let original_auth = mandate(&store,&ticket.id).unwrap().authorization_id;
        let path = store.path.clone();
        drop(store);

        let reopened = Store::open(&path).unwrap();
        let interrupted = get_ticket(&reopened, &ticket.id).unwrap();
        assert_eq!(interrupted.state, "interrupted");
        assert!(interrupted.active_assignment_id.is_none());
        let recovered_mandate = mandate(&reopened,&ticket.id).unwrap();
        assert!(recovered_mandate.revoked);
        assert_eq!(recovered_mandate.authorization_id,original_auth);
        assert!(read_only_policy(&reopened,&original_session).is_err());
        assert!(!on_run_finished(&reopened, &original_session, true, "delayed success").unwrap());
        let events = history(&reopened, &ticket.id, 200).unwrap();
        assert_eq!(events.iter().filter(|event| event.kind == "restart_interrupted").count(), 1);
        assert!(retry_blocked(&mut Store::open(&path).unwrap(), &ticket.id, false).is_err());
        drop(reopened);

        let again = Store::open(&path).unwrap();
        let replayed = history(&again, &ticket.id, 200).unwrap();
        assert_eq!(replayed.iter().filter(|event| event.kind == "restart_interrupted").count(), 1);
        assert_eq!(get_ticket(&again, &ticket.id).unwrap().state, "interrupted");
    }

    #[test]
    fn legacy_interrupted_ticket_with_unrevoked_mandate_is_repaired_once_on_restart() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket,launched)=admit(
            &mut store,&cfg.id,"Old interrupted work","legacy-restart-mandate",false,
        ).unwrap();
        assert!(launched);
        let path=store.path.clone();
        // Emulate a historical interruption that settled the worker but
        // retained its original native execution mandate.
        store.db.execute(
            "UPDATE sa_assignments SET state='interrupted' WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_tickets SET state='interrupted',active_assignment_id=NULL WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        assert!(!mandate(&store,&ticket.id).unwrap().revoked);
        drop(store);
        let reopened=Store::open(&path).unwrap();
        assert_eq!(get_ticket(&reopened,&ticket.id).unwrap().state,"interrupted");
        assert!(mandate(&reopened,&ticket.id).unwrap().revoked);
        let revision=get_ticket(&reopened,&ticket.id).unwrap().revision;
        let audit_count=history(&reopened,&ticket.id,100).unwrap().len();
        drop(reopened);
        let again=Store::open(&path).unwrap();
        assert_eq!(get_ticket(&again,&ticket.id).unwrap().revision,revision);
        assert_eq!(history(&again,&ticket.id,100).unwrap().len(),audit_count);
        assert!(mandate(&again,&ticket.id).unwrap().revoked);
    }

    #[test]
    fn restart_revokes_inconsistent_waiting_ticket_and_ignores_late_success() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Inspect a disposable project", "orphan-waiting", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Worker evidence").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");
        // Simulate a legacy partial settlement: the worker is still marked
        // running although the ticket is already awaiting native acceptance.
        store.db.execute(
            "UPDATE sa_assignments SET state='running' WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        let path = store.path.clone();
        drop(store);

        let reopened = Store::open(&path).unwrap();
        let recovered = get_ticket(&reopened, &ticket.id).unwrap();
        assert_eq!(recovered.state, "interrupted");
        assert!(recovered.active_assignment_id.is_none());
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&reopened, &ticket.id, 10).unwrap()[0].state, "interrupted");
        assert!(!on_run_finished(&reopened, &session, true, "Late success").unwrap());
        assert!(accept(&reopened, &ticket.id, recovered.revision).is_err());
        assert_eq!(history(&reopened, &ticket.id, 100).unwrap().iter()
            .filter(|row| row.kind == "restart_inconsistent_assignment").count(), 1);
        drop(reopened);
        let again = Store::open(&path).unwrap();
        assert_eq!(history(&again, &ticket.id, 100).unwrap().iter()
            .filter(|row| row.kind == "restart_inconsistent_assignment").count(), 1);
    }

    #[test]
    fn restart_preserves_terminal_ticket_and_revokes_orphaned_worker() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit(
            &mut store, &cfg.id, "Finished safely", "orphan-completed", false,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &session, true, "Finished evidence").unwrap());
        let waiting = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(accept(&store, &ticket.id, waiting.revision).unwrap().state, "completed");
        // A corrupted/legacy active assignment must never reopen a completed
        // task; its stale permission must nevertheless be revoked.
        store.db.execute(
            "UPDATE sa_assignments SET state='running' WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_mandates SET revoked=0 WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        let path = store.path.clone();
        drop(store);

        let reopened = Store::open(&path).unwrap();
        let recovered = get_ticket(&reopened, &ticket.id).unwrap();
        assert_eq!(recovered.state, "completed");
        assert!(recovered.active_assignment_id.is_none());
        assert!(mandate(&reopened, &ticket.id).unwrap().revoked);
        assert_eq!(assignments(&reopened, &ticket.id, 10).unwrap()[0].state, "interrupted");
        assert!(!on_run_finished(&reopened, &session, true, "Duplicate completion").unwrap());
        assert_eq!(history(&reopened, &ticket.id, 100).unwrap().iter()
            .filter(|row| row.kind == "restart_inconsistent_assignment").count(), 1);
    }

    #[test]
    fn failed_settlement_rolls_back_assignment_and_can_be_retried_once() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (task, _) = admit(&mut store, &cfg.id, "Document results", "settle-fault-1", false).unwrap();
        let session = assignment_session(&store, &task.id).unwrap().unwrap();
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_sa_settlement BEFORE UPDATE OF state ON sa_tickets
             WHEN NEW.state='waiting_approval'
             BEGIN SELECT RAISE(ABORT,'injected settlement failure'); END;"
        ).unwrap();
        assert!(on_run_finished(&store, &session, true, "Result").is_err());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "running");
        let assignment_state: String = store.db.query_row(
            "SELECT state FROM sa_assignments WHERE session_id=?1",
            [&session], |r| r.get(0),
        ).unwrap();
        assert_eq!(assignment_state, "running");
        let settled_events: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1 AND kind='turn_settled'",
            [&task.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(settled_events, 0);
        store.db.execute_batch("DROP TRIGGER reject_sa_settlement").unwrap();
        assert!(on_run_finished(&store, &session, true, "Result").unwrap());
        assert!(!on_run_finished(&store, &session, true, "Duplicated").unwrap());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "waiting_approval");
        let settled_events: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1 AND kind='turn_settled'",
            [&task.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(settled_events, 1);
    }

    #[test]
    fn v39_upgrade_replaces_ticket_only_lease_without_losing_durable_history() {
        let mut store = fixture();
        let config = enabled(&store);
        let (task, _) = admit(
            &mut store, &config.id, "Keep original objective", "lease-migration-1", false,
        ).unwrap();
        // Reproduce a v39 installation. The old trigger constrained only the
        // ticket worker project and missed the coordinator's real checkout.
        store.db.execute_batch(
            "DROP TRIGGER IF EXISTS sa_project_lease_insert;
             DROP TRIGGER IF EXISTS sa_project_lease_update;
             CREATE TRIGGER sa_project_lease_insert
             BEFORE INSERT ON sa_assignments
             WHEN NEW.state='running' AND EXISTS(
                SELECT 1 FROM sa_assignments a
                JOIN sa_tickets previous ON previous.id=a.ticket_id
                JOIN sa_tickets incoming ON incoming.id=NEW.ticket_id
                WHERE a.state='running' AND previous.project_id=incoming.project_id
             )
             BEGIN SELECT RAISE(ABORT,'old lease'); END;
             PRAGMA user_version=39;",
        ).unwrap();
        let path = store.path.clone();
        drop(store);
        let upgraded = Store::open(&path).unwrap();
        let version: i64 = upgraded.db.pragma_query_value(
            None, "user_version", |row| row.get(0),
        ).unwrap();
        assert_eq!(version, storage::SCHEMA_VERSION);
        let trigger: String = upgraded.db.query_row(
            "SELECT sql FROM sqlite_master
             WHERE type='trigger' AND name='sa_project_lease_insert'",
            [], |row| row.get(0),
        ).unwrap();
        assert!(trigger.contains("previous_session.project_id"));
        assert!(trigger.contains("incoming_session.project_id"));
        assert_eq!(get_ticket(&upgraded, &task.id).unwrap().objective, "Keep original objective");
        assert_eq!(get_ticket(&upgraded, &task.id).unwrap().state, "interrupted");
        assert!(!history(&upgraded, &task.id, 10).unwrap().is_empty());
        drop(upgraded);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(get_ticket(&reopened, &task.id).unwrap().objective, "Keep original objective");
    }

    #[test]
    fn durable_lease_blocks_other_workers_in_a_coordinators_checkout() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('coordinator-project','Separate coordinator','/tmp/sa-coordinator-lease')",
            [],
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET project_id='coordinator-project' WHERE id='router'",
            [],
        ).unwrap();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('other-worker','Independent worker','coordinator-project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        let first_config = enabled(&store);
        let (coordinated, launched) = admit_role(
            &mut store, &first_config.id, "Read-only coordinator plan", "coordinator-lease-a", false, true,
        ).unwrap();
        assert!(launched);
        let other_config = create_configuration(&store, NewOrchestration {
            name: "Other worker in coordinator checkout".into(),
            coordinator_bot_id: "worker".into(),
            worker_bot_id: "other-worker".into(),
        }).unwrap();
        let other_config = change_enabled(&store, &other_config.id, 1, true).unwrap();
        let (blocked, launched) = admit(
            &mut store, &other_config.id, "Independent task", "coordinator-lease-b", false,
        ).unwrap();
        assert!(!launched, "native admission must reserve the coordinator's actual workspace");
        assert_eq!(blocked.state, "blocked");
        // Retry must detect the coordinator's actual session checkout before
        // any session/assignment is created, not depend only on the SQL
        // trigger rejecting the conflicting insert.
        assert!(retry_blocked(&mut store, &blocked.id, false)
            .unwrap_err().contains("occupied"));
        assert_eq!(get_ticket(&store, &blocked.id).unwrap().state, "blocked");

        // The SQLite trigger is a second line of defense against callers
        // that attempt to bypass the normal occupied-workspace preflight.
        let unrelated_session = store.create_session(
            "coordinator-project", "codex", "default", "plan",
        ).unwrap();
        let bypass = store.db.execute(
            "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES('bypassing-lease',?1,'other-worker',?2,'running',1,?3,?3)",
            params![blocked.id,unrelated_session.id,storage::now()],
        );
        assert!(bypass.is_err());
        fail_launch(&store, &coordinated.id, "coordinator stopped").unwrap();
        let (retried, _, worker) = retry_blocked(&mut store, &blocked.id, false).unwrap();
        assert_eq!(retried.state, "running");
        assert_eq!(worker, "other-worker");
    }

    #[test]
    fn coordinated_worker_preflight_reserves_other_coordinator_checkout() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('other-worker-project','Other worker','/tmp/sa-coordinated-preflight')",
            [],
        ).unwrap();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('other-coordinator','Other coordinator','project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('other-worker','Other worker','other-worker-project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();

        let config = enabled(&store);
        let (ticket, started) = admit_role(
            &mut store, &config.id, "Plan a worker change",
            "coordinated-preflight-target", false, true,
        ).unwrap();
        assert!(started);
        let plan_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &plan_session, true, "Current plan").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");

        // The unrelated Team's Ticket targets another project, but its
        // coordinator session is actively using this Team's worker checkout.
        let foreign = create_configuration(&store, NewOrchestration {
            name: "Unrelated coordinated Team".into(),
            coordinator_bot_id: "other-coordinator".into(),
            worker_bot_id: "other-worker".into(),
        }).unwrap();
        let foreign = change_enabled(
            &store, &foreign.id, foreign.policy_revision, true,
        ).unwrap();
        let (foreign_ticket, foreign_started) = admit_role(
            &mut store, &foreign.id, "Plan in first worker checkout",
            "coordinated-preflight-foreign", false, true,
        ).unwrap();
        assert!(foreign_started);
        assert_eq!(get_ticket(&store, &foreign_ticket.id).unwrap().state, "running");
        assert!(manual_candidate_occupied(
            &store, "worker", &config.project_id, &config.project_id,
        ).unwrap());

        let before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        let error = prepare_coordinated_worker(&mut store, &ticket.id, false).unwrap_err();
        assert!(error.contains("occupied"), "Unexpected preflight error: {error}");
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
        let after: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
            [&ticket.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(before, after, "Denied admission must not create an assignment");

        fail_launch(&store, &foreign_ticket.id, "Coordinator cancelled").unwrap();
        let (dispatched, _, worker_id, _) = prepare_coordinated_worker(
            &mut store, &ticket.id, false,
        ).unwrap();
        assert_eq!(dispatched.state, "running");
        assert_eq!(worker_id, "worker");
    }

    #[test]
    fn native_coordinator_lease_also_protects_its_own_distinct_workspace() {
        let mut store = fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('coordinator-project','Coordinator','/tmp/sa-coordinator')",
            [],
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET project_id='coordinator-project' WHERE id='router'",
            [],
        ).unwrap();
        let config = enabled(&store);
        let (ticket, launch) = admit_role(
            &mut store, &config.id, "Plan read-only work", "coordinator-project-fence", false, true,
        ).unwrap();
        assert!(launch);
        let coordinator_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert_eq!(store.session(&coordinator_session).unwrap().project_id, "coordinator-project");

        let ordinary_coordinator = store.create_session(
            "coordinator-project", "codex", "default", "plan",
        ).unwrap();
        let ordinary_worker = store.create_session("project", "codex", "default", "plan").unwrap();
        assert!(guard_run_admission(&store, &coordinator_session, &[]).is_ok());
        // The ticket's worker checkout is reserved, but so is the actual
        // coordinator checkout. Normal direct sessions must not race either.
        assert!(guard_run_admission(&store, &ordinary_coordinator.id, &[]).is_err());
        assert!(guard_run_admission(&store, &ordinary_worker.id, &[]).is_err());

        // A missing active session must fail closed instead of silently
        // releasing the coordinator lease before restart reconciliation.
        store.db.execute(
            "DELETE FROM bot_threads WHERE session_id=?1", [&coordinator_session],
        ).unwrap();
        store.db.execute("DELETE FROM sessions WHERE id=?1", [&coordinator_session]).unwrap();
        assert!(guard_run_admission(&store, &ordinary_coordinator.id, &[]).is_err());
        assert!(guard_run_admission(&store, &ordinary_worker.id, &[]).is_err());

        fail_launch(&store, &ticket.id, "original coordinator session disappeared").unwrap();
        assert!(guard_run_admission(&store, &ordinary_coordinator.id, &[]).is_ok());
        assert!(guard_run_admission(&store, &ordinary_worker.id, &[]).is_ok());
    }

    #[test]
    fn coordinator_workspace_drift_revokes_worker_execution_result_and_approval() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, _) = admit_role(
            &mut store, &cfg.id, "Original project work", "plan-drift-running-worker", false, true,
        ).unwrap();
        let plan_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &plan_session, true, "Plan in original workspace").unwrap());
        let (running, _, _, _) = prepare_coordinated_worker(&mut store, &ticket.id, false).unwrap();
        let worker_session = assignment_session(&store, &running.id).unwrap().unwrap();

        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('moved','Moved','/tmp/sa-worker-plan-drift')", [],
        ).unwrap();
        store.db.execute("UPDATE bots SET project_id='moved' WHERE id='router'", []).unwrap();
        assert!(read_only_policy(&store, &worker_session).is_err());
        assert!(on_run_finished(&store, &worker_session, true, "Stale work").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked");
        assert!(accept(&store, &ticket.id, get_ticket(&store, &ticket.id).unwrap().revision).is_err());
        assert!(retry_blocked(&mut store, &ticket.id, false).is_err());
        assert_eq!(history(&store, &ticket.id, 200).unwrap()
            .iter().filter(|entry| entry.kind == "human_accepted").count(), 0);
    }

    #[test]
    fn rejected_coordinator_callback_cannot_be_replayed_as_authorized_worker_plan() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, launched) = admit_role(
            &mut store, &cfg.id, "Bounded coordination", "rejected-plan-replay", false, true,
        ).unwrap();
        assert!(launched);
        let session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        store.db.execute("UPDATE bots SET paused=1 WHERE id='router'", []).unwrap();

        // A provider may still return success after native authority was
        // revoked. The ticket is blocked; its coordinator assignment is not
        // a valid plan for future dispatch or manual retry.
        assert!(on_run_finished(&store, &session, true, "Untrusted late plan").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked");
        let attempts = assignments(&store, &ticket.id, 20).unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].state, "failed");
        assert!(history(&store, &ticket.id, 20).unwrap().iter()
            .any(|event| event.kind == "callback_rejected"));

        // Restoring the bot's availability is not authorization to relabel
        // an earlier rejected plan as successful coordination.
        store.db.execute("UPDATE bots SET paused=0 WHERE id='router'", []).unwrap();
        assert!(retry_blocked(&mut store, &ticket.id, false).is_err());
        assert!(prepare_coordinated_worker(&mut store, &ticket.id, false).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "blocked");
        assert_eq!(assignments(&store, &ticket.id, 20).unwrap().len(), 1);
    }

    #[test]
    fn settled_coordinator_plan_requires_its_original_workspace_and_session_at_dispatch() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit_role(
            &mut store, &cfg.id, "Review original checkout", "settled-plan-project", false, true,
        ).unwrap();
        assert!(started);
        let coordinator_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &coordinator_session, true, "Current coordinator plan").unwrap());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");

        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('other','Other','/tmp/sa-moved-plan')", [],
        ).unwrap();
        store.db.execute("UPDATE bots SET project_id='other' WHERE id='router'", []).unwrap();
        assert!(prepare_coordinated_worker(&mut store, &ticket.id, false).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
        assert!(assignments(&store, &ticket.id, 100).unwrap()
            .iter().all(|attempt| attempt.bot_id == "router"));

        store.db.execute("UPDATE bots SET project_id='project' WHERE id='router'", []).unwrap();
        store.db.execute("DELETE FROM sessions WHERE id=?1", [&coordinator_session]).unwrap();
        assert!(prepare_coordinated_worker(&mut store, &ticket.id, false).is_err());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().state, "queued");
    }

    #[test]
    fn coordinator_project_move_cannot_read_or_finalize_its_previous_context() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (task, _) = admit_role(
            &mut store, &cfg.id, "Coordinator observes a fixed project", "coordinator-project-move", false, true,
        ).unwrap();
        let session = assignment_session(&store, &task.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &session).unwrap().is_some());
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('different','Different','/tmp/coordinator-different')",
            [],
        ).unwrap();
        store.db.execute("UPDATE bots SET project_id='different' WHERE id='router'", []).unwrap();
        assert!(read_only_policy(&store, &session).is_err());
        assert!(guard_run_admission(&store, &session, &[]).is_err());
        assert!(on_run_finished(&store, &session, true, "Prior-project advice").unwrap());
        assert_eq!(get_ticket(&store, &task.id).unwrap().state, "blocked");
        assert!(prepare_coordinated_worker(&mut store, &task.id, false).is_err());
        assert!(!on_run_finished(&store, &session, true, "Late coordinator").unwrap());
    }

    #[test]
    fn coordinator_pause_invalidates_an_active_worker_without_accepting_its_result() {
        let mut store = fixture();
        let cfg = enabled(&store);
        let (ticket, started) = admit(&mut store, &cfg.id, "Read only work", "coordinator-paused-worker", false).unwrap();
        assert!(started);
        let worker_session = assignment_session(&store, &ticket.id).unwrap().unwrap();
        assert!(read_only_policy(&store, &worker_session).unwrap().is_some());

        store.db.execute("UPDATE bots SET paused=1 WHERE id='router'", []).unwrap();
        assert!(read_only_policy(&store, &worker_session).is_err());
        assert!(on_run_finished(&store, &worker_session, true, "Result after coordinator pause").unwrap());
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        assert!(blocked.active_assignment_id.is_none());
        assert!(accept(&store, &ticket.id, blocked.revision).is_err());
        assert!(!on_run_finished(&store, &worker_session, true, "Duplicate stale result").unwrap());
        assert!(!history(&store, &ticket.id, 200).unwrap()
            .iter().any(|entry| entry.kind == "human_accepted"));
    }

    #[test]
    fn paused_or_moved_native_bots_cannot_supply_or_accept_stale_evidence() {
        let mut store = fixture();
        let config = enabled(&store);
        let (first, _) = admit(&mut store, &config.id, "Safe result", "bot-drift-first", false).unwrap();
        let first_session = assignment_session(&store, &first.id).unwrap().unwrap();
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'", []).unwrap();
        assert!(read_only_policy(&store, &first_session).is_err());
        assert!(on_run_finished(&store, &first_session, true, "Untrusted late result").unwrap());
        assert_eq!(get_ticket(&store, &first.id).unwrap().state, "blocked");
        store.db.execute("UPDATE bots SET paused=0 WHERE id='worker'", []).unwrap();

        let (second, _) = admit(&mut store, &config.id, "Another result", "bot-drift-second", false).unwrap();
        let second_session = assignment_session(&store, &second.id).unwrap().unwrap();
        assert!(on_run_finished(&store, &second_session, true, "Current worker evidence").unwrap());
        let awaiting = get_ticket(&store, &second.id).unwrap();
        assert_eq!(awaiting.state, "waiting_approval");
        store.db.execute("UPDATE bots SET paused=1 WHERE id='router'", []).unwrap();
        assert!(accept(&store, &second.id, awaiting.revision).is_err());
        store.db.execute("UPDATE bots SET paused=0 WHERE id='router'", []).unwrap();
        store.db.execute("INSERT INTO projects(id,name,path) VALUES('other','Other','/tmp/sa-other')", []).unwrap();
        store.db.execute("UPDATE bots SET project_id='other' WHERE id='worker'", []).unwrap();
        assert!(accept(&store, &second.id, awaiting.revision).is_err());
        assert_eq!(get_ticket(&store, &second.id).unwrap().state, "waiting_approval");
        assert!(!history(&store, &second.id, 100).unwrap()
            .iter().any(|entry| entry.kind == "human_accepted"));
    }

    #[test]
    fn policy_revision_change_rejects_stale_worker_and_approval() {
        let mut store=fixture();
        let cfg=enabled(&store);
        let (ticket,_)=admit(&mut store,&cfg.id,"Review","revision-1",false).unwrap();
        let sid=assignment_session(&store,&ticket.id).unwrap().unwrap();
        store.db.execute("UPDATE sa_orchestrations SET policy_revision=policy_revision+1 WHERE id=?1",[&cfg.id]).unwrap();
        assert!(read_only_policy(&store,&sid).is_err());
        assert!(on_run_finished(&store,&sid,true,"Stale result").unwrap());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"blocked");
    }

    #[test]
    fn duplicate_checkout_path_is_rejected_by_storage_and_active_lease_is_preserved() {
        let mut store=fixture();
        let first=enabled(&store);
        let (ticket,launched)=admit(&mut store,&first.id,
            "First checkout owner","checkout-unique-first",false).unwrap();
        assert!(launched);

        // projects.path is UNIQUE. Two project IDs cannot have an identical
        // stored path, so the previous alias fixture was impossible to create.
        // Physical/symlink aliases with different strings need separate guards.
        assert!(store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('alias-project','Alias','/tmp/disposable-superagent')",[]
        ).is_err());
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM projects WHERE id='alias-project'",[],
            |row|row.get::<_,i64>(0)).unwrap(),0);

        // A regular session cannot bypass the original Ticket's checkout lease.
        let ordinary=store.create_session("project","codex","default","ask").unwrap();
        assert!(guard_run_admission(&store,&ordinary.id,&[]).is_err());
        assert!(active_manual_conflict(&store,&[ordinary.id],"worker","project").unwrap());
        assert_eq!(get_ticket(&store,&ticket.id).unwrap().state,"running");
    }

    #[test]
    fn changed_checkout_path_invalidates_admitted_session_and_late_result() {
        let mut store=fixture();
        let config=enabled(&store);
        let (ticket,launched)=admit(&mut store,&config.id,
            "Read original checkout","checkout-path-snapshot",false).unwrap();
        assert!(launched);
        let sid=assignment_session(&store,&ticket.id).unwrap().unwrap();
        let profile=roster::ticket_profile(&store,&ticket.id).unwrap().unwrap();
        assert!(profile.members.iter().all(|m|
            m.project_path.as_deref()==Some("/tmp/disposable-superagent")));
        assert!(roster::checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(read_only_policy(&store,&sid).unwrap().is_some());
        store.db.execute(
            "UPDATE projects SET path='/tmp/superagent-reassigned-checkout' WHERE id='project'",[]
        ).unwrap();
        assert!(!roster::checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(read_only_policy(&store,&sid).is_err());
        assert!(guard_run_admission(&store,&sid,&[]).is_err());
        assert!(on_run_finished(&store,&sid,true,"Untrusted success").unwrap());
        let blocked=get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(blocked.state,"blocked");
        assert!(accept(&store,&ticket.id,blocked.revision).is_err());
        assert!(!on_run_finished(&store,&sid,true,"Repeated success").unwrap());
    }

    #[test]
    fn v46_upgrade_restores_alias_safe_checkout_lease_triggers() {
        let store=fixture();
        let path=store.path.clone();
        store.db.execute_batch(
            "DROP TRIGGER sa_project_lease_insert;
             DROP TRIGGER sa_project_lease_update;
             PRAGMA user_version=46;",
        ).unwrap();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        let version:i64=reopened.db.pragma_query_value(None,"user_version",|r|r.get(0)).unwrap();
        assert_eq!(version,storage::SCHEMA_VERSION);
        for name in ["sa_project_lease_insert","sa_project_lease_update"] {
            let sql:String=reopened.db.query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1",
                [name],|row|row.get(0),
            ).unwrap();
            assert!(sql.contains("old_checkout.path=new_checkout.path"),"{name}");
        }
    }

    #[test]
    fn ordinary_chat_admission_retains_legacy_empty_project_path_behavior() {
        let store=fixture();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('legacy-empty-project','Legacy','')",[]
        ).unwrap();
        let ordinary=store.create_session("legacy-empty-project","codex","default","ask").unwrap();
        // No SuperAgent owns this session. The new exact-checkout guard cannot
        // turn a legacy ordinary chat into a task-scoped provider admission.
        assert!(guard_run_admission(&store,&ordinary.id,&[]).is_ok());
    }
}
