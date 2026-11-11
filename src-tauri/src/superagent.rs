//! Native, opt-in, read-only manual SuperAgent intake (issue #131, S1 slice).
//! A ticket and its mandate outlive bot and session records. This module does
//! not execute untrusted model proposals as authority or create another runner.

use crate::{bots, storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

pub(crate) const CHANNEL: &str = "superagent";

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
CREATE TABLE IF NOT EXISTS sa_audit (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL,
  detail TEXT NOT NULL,
  at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sa_audit_ticket ON sa_audit(ticket_id,id);
"#;

/// An active harness is not resumed automatically after process restart.
pub(crate) const RECOVER: &str = "
UPDATE sa_assignments SET state='interrupted',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='running';
UPDATE sa_tickets SET state='interrupted',revision=revision+1,active_assignment_id=NULL,
 updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='running';
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
    let id = storage::id();
    let now = storage::now();
    store.db.execute(
        "INSERT INTO sa_orchestrations(id,name,coordinator_bot_id,worker_bot_id,project_id,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?6)",
        params![id,name,coordinator.id,worker.id,worker.project_id,now],
    ).map_err(|e| e.to_string())?;
    configuration(store, &id)
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

fn record(store: &Store, id: &str, kind: &str, detail: &str) -> Result<()> {
    store.db.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,?2,?3,?4)",
        params![id,kind,detail,storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Native manual request with a stable dedup key. A blocked request remains
/// auditable and must be explicitly retried; no background inference or launch.
fn admit(store: &mut Store, config_id: &str, objective: &str, request_key: &str, busy: bool) -> Result<(Ticket, bool)> {
    let objective = objective.trim();
    if objective.is_empty() || objective.len() > 32_768 || objective.chars().any(|c| c == '\0') {
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
        return Ok((ticket, false));
    }
    let config = configuration(store, config_id)?;
    if !config.enabled {
        return Err("Orchestration is disabled; opt in before submitting.".into());
    }
    let coordinator = bots::get(store, &config.coordinator_bot_id)?;
    let worker = bots::get(store, &config.worker_bot_id)?;
    if coordinator.paused || worker.paused || worker.project_id != config.project_id {
        return Err("A selected bot is paused, missing or outside the worker project scope.".into());
    }
    let occupied: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE bot_id=?1 AND state='running')",
        [&worker.id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let blocked = busy || occupied;
    let ticket_id = storage::id();
    let assignment_id = (!blocked).then(storage::id);
    // Reserve the internal conversation in the SAME SQLite transaction as the
    // ticket, mandate and worker lease. If any insert or CHECK fails, the
    // ephemeral session and bot-thread mapping must roll back as well.
    // Store methods share this connection, so use unchecked_transaction here
    // instead of borrowing it mutably while provisioning the bot thread.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let session_id = if blocked { None } else {
        Some(bots::thread_session(store, &worker, CHANNEL, &ticket_id, "SuperAgent · Manual task")?.0)
    };
    let now = storage::now();
    tx.execute(
        "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,project_id,worker_bot_id,policy_revision,state,active_assignment_id,created_at,updated_at)
         VALUES(?1,?2,?3,?4,'local-user','manual',?5,?6,?7,?8,?9,?10,?10)",
        params![ticket_id,config.id,request_key,objective,config.project_id,worker.id,config.policy_revision,
            if blocked { "blocked" } else { "running" }, assignment_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_mandates(ticket_id,actor,authorization_id,policy_revision,project_id,expires_at,created_at)
         VALUES(?1,'local-user',?2,?3,?4,?5,?6)",
        params![ticket_id,storage::id(),config.policy_revision,config.project_id,now + 4 * 60 * 60 * 1000,now],
    ).map_err(|e| e.to_string())?;
    if let (Some(assignment_id),Some(session_id)) = (&assignment_id,&session_id) {
        tx.execute(
            "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES(?1,?2,?3,?4,'running',1,?5,?5)",
            params![assignment_id,ticket_id,worker.id,session_id,now],
        ).map_err(|e| e.to_string())?;
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'admitted',?2,?3)",
        params![ticket_id, if blocked { "blocked: worker occupied" } else { "manual readonly assignment reserved" },now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((get_ticket(store, &ticket_id)?, !blocked))
}

fn assignment_session(store: &Store, task_id: &str) -> Result<Option<String>> {
    store.db.query_row(
        "SELECT a.session_id FROM sa_assignments a JOIN sa_tickets t ON t.active_assignment_id=a.id
         WHERE t.id=?1 AND a.state='running'", [task_id],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())
}

/// Only the active, user-admitted read-only assignment is allowed to run.
/// Leverages the existing provider-level Council isolation (native plan,
/// tool-host removal, external MCP checks and permission denial), not a prompt.
pub(crate) fn read_only_policy(
    store: &Store, session_id: &str,
) -> Result<Option<crate::architect::reviewer_security::ReviewerToolPolicy>> {
    let scope: Option<(String, String, i64, i64, i64, i64, i64)> = store.db.query_row(
        "SELECT t.state,a.state,m.expires_at,m.revoked,m.policy_revision,o.policy_revision,o.enabled
         FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE a.session_id=?1 AND t.active_assignment_id=a.id",
        [session_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)),
    ).optional().map_err(|e| e.to_string())?;
    if let Some((task_state,assignment_state,expiry,revoked,mandate_version,config_version,enabled)) = scope {
        if task_state != "running" || assignment_state != "running" || revoked != 0
            || expiry <= storage::now() || mandate_version != config_version || enabled == 0
        {
            return Err("policy_unsupported: SuperAgent mandate is inactive or stale.".into());
        }
        return Ok(Some(crate::architect::reviewer_security::ReviewerToolPolicy::NativeReadOnly));
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

fn fail_launch(store: &Store, ticket_id: &str, reason: &str) -> Result<()> {
    store.db.execute(
        "UPDATE sa_assignments SET state='failed',updated_at=?2,result=?3
         WHERE id=(SELECT active_assignment_id FROM sa_tickets WHERE id=?1) AND state='running'",
        params![ticket_id,storage::now(),reason],
    ).map_err(|e| e.to_string())?;
    store.db.execute(
        "UPDATE sa_tickets SET state='blocked',active_assignment_id=NULL,revision=revision+1,updated_at=?2
         WHERE id=?1 AND state='running'",params![ticket_id,storage::now()],
    ).map_err(|e| e.to_string())?;
    record(store, ticket_id, "launch_failed", &reason.chars().take(500).collect::<String>())
}

/// A provider turn is evidence for a task, not its approval. The only successful
/// transition here is running -> waiting_approval; native human acceptance is separate.
pub(crate) fn on_run_finished(store: &Store, session_id: &str, successful: bool, text: &str) -> Result<bool> {
    // A callback may arrive twice, after Stop, or after process restart.
    // Result, Ticket revision and audit must settle in one transaction; an
    // injected storage failure cannot strand an assignment in a terminal state.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let assignment: Option<(String, String)> = tx.query_row(
        "SELECT a.ticket_id,a.id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id AND t.active_assignment_id=a.id
         WHERE a.session_id=?1 AND a.state='running' AND t.state='running'",
        [session_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((task_id, assignment_id)) = assignment else { return Ok(false) };
    let authorized: bool = tx.query_row(
        "SELECT m.revoked=0 AND m.expires_at>?2 AND m.policy_revision=o.policy_revision AND o.enabled=1
         FROM sa_mandates m JOIN sa_tickets t ON t.id=m.ticket_id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id WHERE t.id=?1",
        params![task_id,storage::now()], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    let trimmed = text.trim().chars().take(4096).collect::<String>();
    let next = if !authorized { "blocked" }
        else if !successful { "failed" }
        else if trimmed.is_empty() { "inconclusive" }
        else { "waiting_approval" };
    let now = storage::now();
    let moved = tx.execute(
        "UPDATE sa_assignments SET state=?2,result=?3,updated_at=?4
         WHERE id=?1 AND state='running'",
        params![assignment_id,if successful { "finished" } else { "failed" },trimmed,now],
    ).map_err(|e| e.to_string())?;
    if moved != 1 { return Err("SuperAgent assignment changed before settlement.".into()); }
    let updated = tx.execute(
        "UPDATE sa_tickets SET state=?2,evidence=?3,active_assignment_id=NULL,
         revision=revision+1,updated_at=?4
         WHERE id=?1 AND state='running' AND active_assignment_id=?5",
        params![task_id,next,trimmed,now,assignment_id],
    ).map_err(|e| e.to_string())?;
    if updated != 1 { return Err("SuperAgent ticket changed before settlement.".into()); }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'turn_settled',?2,?3)",
        params![task_id,next,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

fn accept(store: &Store, ticket_id: &str, expected_revision: i64) -> Result<Ticket> {
    let ticket = get_ticket(store,ticket_id)?;
    if ticket.state != "waiting_approval" || ticket.revision != expected_revision
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
    {
        return Err("Task has no current verified worker result to accept.".into());
    }
    let granted: bool = store.db.query_row(
        "SELECT m.revoked=0 AND m.expires_at>?2 AND m.policy_revision=o.policy_revision AND o.enabled=1
         FROM sa_mandates m JOIN sa_tickets t ON t.id=m.ticket_id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id WHERE t.id=?1",
        params![ticket_id,storage::now()], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if !granted {
        return Err("Mandate was revoked, expired or changed; acceptance denied.".into());
    }
    let changed = store.db.execute(
        "UPDATE sa_tickets SET state='completed',revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2 AND state='waiting_approval'",
        params![ticket_id,expected_revision,storage::now()],
    ).map_err(|e| e.to_string())?;
    if changed != 1 { return Err("Concurrent task state change; acceptance denied.".into()); }
    record(store,ticket_id,"human_accepted","local-user explicitly accepted worker evidence")?;
    get_ticket(store,ticket_id)
}

fn stop(store: &Store, ticket_id: &str) -> Result<(Ticket,Option<String>)> {
    let ticket = get_ticket(store,ticket_id)?;
    if matches!(ticket.state.as_str(),"completed"|"failed"|"stopped"|"interrupted"|"inconclusive") {
        return Err("Task is already terminal.".into());
    }
    let session = assignment_session(store,ticket_id)?;
    let now = storage::now();
    store.db.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[ticket_id])
        .map_err(|e| e.to_string())?;
    store.db.execute(
        "UPDATE sa_assignments SET state='stopped',updated_at=?2 WHERE id=?1 AND state='running'",
        params![ticket.active_assignment_id,now],
    ).map_err(|e| e.to_string())?;
    store.db.execute(
        "UPDATE sa_tickets SET state='stopped',active_assignment_id=NULL,revision=revision+1,updated_at=?2
         WHERE id=?1 AND revision=?3",params![ticket_id,now,ticket.revision],
    ).map_err(|e| e.to_string())?;
    record(store,ticket_id,"stopped","local-user revoked mandate")?;
    Ok((get_ticket(store,ticket_id)?,session))
}

#[tauri::command]
pub async fn superagent_create(state: State<'_,AppState>, input: NewOrchestration) -> Result<Orchestration> {
    let store=state.store.lock().map_err(|e|e.to_string())?;
    create_configuration(&store,input)
}

#[tauri::command]
pub async fn superagent_list(state: State<'_,AppState>) -> Result<Vec<Orchestration>> {
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let mut stmt=store.db.prepare("SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision FROM sa_orchestrations ORDER BY created_at")
        .map_err(|e|e.to_string())?;
    stmt.query_map([],orchestration_row).map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
}

#[tauri::command]
pub async fn superagent_set_enabled(state: State<'_,AppState>, orchestration_id: String, enabled: bool) -> Result<Orchestration> {
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let current=configuration(&store,&orchestration_id)?;
    if current.enabled != enabled {
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=?2,policy_revision=policy_revision+1,updated_at=?3 WHERE id=?1",
            params![orchestration_id,enabled,storage::now()],
        ).map_err(|e|e.to_string())?;
    }
    configuration(&store,&orchestration_id)
}

#[tauri::command]
pub async fn superagent_ticket(state: State<'_,AppState>, ticket_id: String) -> Result<Ticket> {
    get_ticket(&state.store.lock().map_err(|e|e.to_string())?,&ticket_id)
}

#[tauri::command]
pub async fn superagent_submit(
    app: tauri::AppHandle, state: State<'_,AppState>,
    orchestration_id: String, objective: String, request_key: String,
) -> Result<Ticket> {
    if crate::feature_controls::get().bots_paused {
        return Err("Bots are paused; no SuperAgent dispatch is allowed.".into());
    }
    let (ticket, launch, worker_id) = {
        let active: Vec<String> = state.agents.lock().map_err(|e|e.to_string())?
            .keys().cloned().collect();
        let mut store=state.store.lock().map_err(|e|e.to_string())?;
        let cfg=configuration(&store,&orchestration_id)?;
        let busy=active.iter().any(|sid| bots::owner_of(&store,sid).ok().flatten()
            .is_some_and(|(bot,_)|bot.id==cfg.worker_bot_id));
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
            text: format!("SuperAgent ticket {}\n\n{}\n\nRead-only assignment. Report evidence and remaining gaps; do not claim task approval.",ticket.id,objective),
            trusted: true,
            image_ids: None,
            moodboard_ids: None,
        },
    ).await;
    if let Err(error) = delivered {
        if let Ok(store)=state.store.lock() { let _=fail_launch(&store,&ticket.id,&error); }
        return Err(format!("Dispatch failed; ticket {} retained for inspection: {error}",ticket.id));
    }
    get_ticket(&state.store.lock().map_err(|e|e.to_string())?,&ticket.id)
}

#[tauri::command]
pub async fn superagent_accept(state: State<'_,AppState>, ticket_id: String, expected_revision: i64) -> Result<Ticket> {
    accept(&state.store.lock().map_err(|e|e.to_string())?,&ticket_id,expected_revision)
}

#[tauri::command]
pub async fn superagent_stop(app: tauri::AppHandle, state: State<'_,AppState>, ticket_id: String) -> Result<Ticket> {
    let (ticket,session)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        stop(&store,&ticket_id)?
    };
    if let Some(session)=session {
        let _=crate::stop_session(app.clone(),app.state::<AppState>(),session).await;
    }
    Ok(ticket)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(on_run_finished(&store,"wrong-session",true,"fake").unwrap()==false);
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
}
