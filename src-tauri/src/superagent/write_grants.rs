//! Narrow, user-issued worker file writes on the existing provider ToolHost.
//! Provider-native shell/filesystem tools stay in their native read-only mode.

use super::*;
use crate::{
    providers::ToolHost,
    storage::Store,
    AppState,
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::Manager;

pub(crate) const TOOL_NAME: &str = "pandacode_superagent_write_file";
pub(crate) const MAX_PATHS: usize = 16;
pub(crate) const MAX_CONTENT_BYTES: usize = 1024 * 1024;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_worker_write_grants (
  id TEXT PRIMARY KEY,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  assignment_id TEXT NOT NULL UNIQUE REFERENCES sa_assignments(id) ON DELETE RESTRICT,
  session_id TEXT NOT NULL UNIQUE,
  ticket_revision INTEGER NOT NULL,
  policy_revision INTEGER NOT NULL,
  project_id TEXT NOT NULL,
  actor TEXT NOT NULL CHECK(actor='local-user'),
  tool_name TEXT NOT NULL CHECK(tool_name='pandacode_superagent_write_file'),
  allowed_paths TEXT NOT NULL CHECK(json_valid(allowed_paths)),
  expires_at INTEGER NOT NULL,
  revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)),
  created_at INTEGER NOT NULL,
  revoked_at INTEGER
);
CREATE INDEX IF NOT EXISTS sa_worker_write_grants_ticket
  ON sa_worker_write_grants(ticket_id,created_at);
CREATE TABLE IF NOT EXISTS sa_worker_write_receipts (
  id TEXT PRIMARY KEY,
  grant_id TEXT NOT NULL REFERENCES sa_worker_write_grants(id) ON DELETE RESTRICT,
  ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  assignment_id TEXT NOT NULL REFERENCES sa_assignments(id) ON DELETE RESTRICT,
  session_id TEXT NOT NULL,
  ticket_revision INTEGER NOT NULL,
  path TEXT NOT NULL,
  content_sha256 TEXT NOT NULL CHECK(length(content_sha256)=64),
  byte_count INTEGER NOT NULL CHECK(byte_count>=0),
  state TEXT NOT NULL CHECK(state IN ('reserved','applied','failed','uncertain')),
  detail TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sa_worker_write_receipts_ticket
  ON sa_worker_write_receipts(ticket_id,created_at);
CREATE TRIGGER IF NOT EXISTS sa_worker_write_revoke_mandate
AFTER UPDATE OF revoked ON sa_mandates
WHEN OLD.revoked=0 AND NEW.revoked=1
BEGIN
  UPDATE sa_worker_write_grants SET revoked=1,revoked_at=NEW.created_at
   WHERE ticket_id=NEW.ticket_id AND revoked=0;
END;
CREATE TRIGGER IF NOT EXISTS sa_worker_write_revoke_assignment
AFTER UPDATE OF state ON sa_assignments
WHEN OLD.state='running' AND NEW.state<>'running'
BEGIN
  UPDATE sa_worker_write_grants SET revoked=1,revoked_at=NEW.updated_at
   WHERE assignment_id=NEW.id AND revoked=0;
END;
"#;

/// A reserved write whose response was lost across process shutdown is never
/// replayed automatically. Its caller can inspect the exact file and receipt.
pub(crate) const RECOVER: &str = r#"
UPDATE sa_worker_write_receipts
   SET state='uncertain',detail='Process stopped before write receipt settlement',updated_at=MAX(updated_at,created_at)
 WHERE state='reserved';
"#;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WriteGrantSummary {
    pub id: String,
    pub ticket_id: String,
    pub assignment_id: String,
    pub session_id: String,
    pub ticket_revision: i64,
    pub policy_revision: i64,
    pub project_id: String,
    pub project_root: String,
    pub allowed_paths: Vec<String>,
    pub expires_at: i64,
    pub revoked: bool,
}

#[derive(Clone, Debug)]
struct ActiveWriteGrant {
    summary: WriteGrantSummary,
    actor: String,
}

pub(crate) fn validate_paths(paths: &[String]) -> Result<()> {
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return Err(format!("Authorize between 1 and {MAX_PATHS} exact file paths."));
    }
    let mut unique = HashSet::new();
    for path in paths {
        validate_path_syntax(path)?;
        if !unique.insert(path) {
            return Err("A write scope cannot repeat a file path.".into());
        }
    }
    Ok(())
}

fn validate_path_syntax(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > 4096 || path.contains(['\\', '\0', ':']) {
        return Err("Invalid exact project file path.".into());
    }
    if path.split('/').any(|part| {
        part.is_empty() || part == "." || part == ".." || part.eq_ignore_ascii_case(".git")
            || part.chars().any(char::is_control)
    }) {
        return Err("Write paths must be normalized project-relative file names.".into());
    }
    Ok(())
}

pub(crate) fn validate_paths_in_root(root: &std::path::Path, paths: &[String]) -> Result<()> {
    validate_paths(paths)?;
    for path in paths {
        crate::workspace::validate_scoped_write_path(root, path)?;
    }
    Ok(())
}

/// The grant is inserted in the same transaction as worker Assignment
/// admission. Its revision is the post-admission revision the provider sees.
pub(crate) fn insert_for_assignment(
    tx: &Transaction<'_>,
    ticket_id: &str,
    assignment_id: &str,
    session_id: &str,
    project_id: &str,
    project_root: &str,
    ticket_revision: i64,
    policy_revision: i64,
    paths: &[String],
    now: i64,
) -> Result<WriteGrantSummary> {
    validate_paths(paths)?;
    let mandate_expiry: i64 = tx.query_row(
        "SELECT expires_at FROM sa_mandates WHERE ticket_id=?1 AND revoked=0",
        [ticket_id],
        |row| row.get(0),
    ).map_err(|_| "Cannot issue a worker write grant without a live mandate.")?;
    let expires_at = mandate_expiry.min(now.saturating_add(super::MAX_ASSIGNMENT_RUN_MS));
    if expires_at <= now {
        return Err("Cannot issue a worker write grant for an expired mandate.".into());
    }
    let id = storage::id();
    let allowed_paths = serde_json::to_string(paths).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_worker_write_grants(
           id,ticket_id,assignment_id,session_id,ticket_revision,policy_revision,
           project_id,project_root,actor,tool_name,allowed_paths,expires_at,created_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'local-user',?9,?10,?11,?12)",
        params![
            id,ticket_id,assignment_id,session_id,ticket_revision,policy_revision,
            project_id,project_root,TOOL_NAME,allowed_paths,expires_at,now,
        ],
    ).map_err(|e| format!("Could not persist worker write grant: {e}"))?;
    Ok(WriteGrantSummary {
        id,
        ticket_id: ticket_id.into(),
        assignment_id: assignment_id.into(),
        session_id: session_id.into(),
        ticket_revision,
        policy_revision,
        project_id: project_id.into(),
        project_root: project_root.into(),
        allowed_paths: paths.to_vec(),
        expires_at,
        revoked: false,
    })
}

fn current_grant(
    store: &Store,
    session_id: &str,
    requested_id: Option<&str>,
) -> Result<Option<ActiveWriteGrant>> {
    // Reuse the assignment, role, thread, project, bot and runtime checks that
    // gate every SuperAgent provider launch before evaluating the write grant.
    if super::read_only_policy(store, session_id)?.is_none() {
        return Ok(None);
    }
    type Raw = (
        String,String,String,String,i64,i64,String,String,String,i64,i64,
        String,i64,Option<String>,String,String,String,i64,i64,i64,
        String,String,String,String,String,i64,i64,String,String,i64,i64,String,
    );
    let raw: Option<Raw> = store.db.query_row(
        "SELECT g.id,g.ticket_id,g.assignment_id,g.session_id,g.ticket_revision,
                g.policy_revision,g.project_id,g.actor,g.allowed_paths,g.expires_at,g.revoked,
                t.state,t.revision,t.active_assignment_id,a.state,a.bot_id,t.worker_bot_id,
                o.policy_revision,o.enabled,m.revoked,m.project_id,m.actor,t.actor,
                s.project_id,s.provider,a.created_at,a.attempt,o.coordinator_bot_id,
                t.project_id,m.expires_at,m.policy_revision,g.project_root
         FROM sa_worker_write_grants g
         JOIN sa_tickets t ON t.id=g.ticket_id
         JOIN sa_assignments a ON a.id=g.assignment_id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sessions s ON s.id=a.session_id
         WHERE g.session_id=?1 ORDER BY g.created_at DESC LIMIT 1",
        [session_id],
        |r| Ok((
            r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,
            r.get(11)?,r.get(12)?,r.get(13)?,r.get(14)?,r.get(15)?,r.get(16)?,r.get(17)?,r.get(18)?,r.get(19)?,r.get(20)?,
            r.get(21)?,r.get(22)?,r.get(23)?,r.get(24)?,r.get(25)?,r.get(26)?,r.get(27)?,r.get(28)?,r.get(29)?,r.get(30)?,r.get(31)?,
        )),
    ).optional().map_err(|e| e.to_string())?;
    let Some(raw) = raw else {
        if requested_id.is_some() {
            return Err("write_grant_not_current: no grant belongs to this worker session.".into());
        }
        return Ok(None);
    };
    let (
        id,ticket_id,assignment_id,grant_session,ticket_revision,policy_revision,
        project_id,actor,paths_json,expires_at,revoked,ticket_state,current_revision,
        active_assignment,assignment_state,bot_id,_legacy_worker_id,_config_revision,
        enabled,mandate_revoked,mandate_project,mandate_actor,ticket_actor,
        session_project,provider,started_at,attempt,_legacy_coordinator_id,ticket_project,
        mandate_expiry,mandate_revision,project_root,
    ) = raw;
    if requested_id.is_some_and(|wanted| wanted != id) {
        return Err("write_grant_not_current: the requested grant is not the active grant.".into());
    }
    let paths: Vec<String> = serde_json::from_str(&paths_json)
        .map_err(|_| "write_grant_invalid: stored file scope is malformed.")?;
    validate_paths(&paths)?;
    let snapshot = super::ticket_team_snapshot(store,&ticket_id)?;
    let role_current = super::assignment_role_authorized(store,&ticket_id,&bot_id,attempt)?;
    let is_configured_worker = snapshot.worker_bot_ids.iter().any(|id| id == &bot_id);
    let current_root = store.workspace_root(&session_project,Some(session_id))?;
    let root_matches = project_root == current_root.to_string_lossy().into_owned();
    let now = storage::now();
    if revoked != 0 || mandate_revoked != 0 || enabled == 0
        || ticket_state != "running" || assignment_state != "running"
        || active_assignment.as_deref() != Some(assignment_id.as_str())
        || grant_session != session_id || !is_configured_worker
        || bot_id == snapshot.coordinator_bot_id
        || !role_current || ticket_revision != current_revision
        || policy_revision != snapshot.policy.policy_revision
        || policy_revision != mandate_revision
        || expires_at <= now || mandate_expiry <= now
        || project_id != ticket_project || project_id != mandate_project
        || project_id != session_project || mandate_actor != ticket_actor
        || !root_matches
        || actor != "local-user" || ticket_actor != "local-user"
        || provider != "codex"
        || !super::assignment_within_budget(started_at, now)
    {
        return Err("write_grant_stale: assignment, task revision or user grant is no longer current.".into());
    }
    let summary = WriteGrantSummary {
        id,
        ticket_id,
        assignment_id,
        session_id: grant_session,
        ticket_revision,
        policy_revision,
        project_id,
        project_root,
        allowed_paths: paths,
        expires_at,
        revoked: false,
    };
    Ok(Some(ActiveWriteGrant { summary, actor }))
}

pub(crate) fn summary_for_ticket(store: &Store, ticket_id: &str) -> Result<WriteGrantSummary> {
    let raw: (String,String,String,String,i64,i64,String,String,String,i64,i64) = store.db.query_row(
        "SELECT id,ticket_id,assignment_id,session_id,ticket_revision,policy_revision,
                project_id,project_root,allowed_paths,expires_at,revoked
         FROM sa_worker_write_grants WHERE ticket_id=?1 ORDER BY created_at DESC LIMIT 1",
        [ticket_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?)),
    ).map_err(|_| "Worker write grant not found.")?;
    Ok(WriteGrantSummary {
        id: raw.0,
        ticket_id: raw.1,
        assignment_id: raw.2,
        session_id: raw.3,
        ticket_revision: raw.4,
        policy_revision: raw.5,
        project_id: raw.6,
        project_root: raw.7,
        allowed_paths: serde_json::from_str(&raw.8).map_err(|_| "Stored file scope is malformed.")?,
        expires_at: raw.9,
        revoked: raw.10 != 0,
    })
}

pub(crate) fn revoke(
    store: &Store,
    ticket_id: &str,
    grant_id: &str,
    expected_revision: i64,
) -> Result<WriteGrantSummary> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let ticket = super::get_ticket(store, ticket_id)?;
    if ticket.revision != expected_revision || ticket.state != "running"
        || ticket.active_assignment_id.is_none()
    {
        return Err("Ticket changed or is no longer running; write grant revocation denied.".into());
    }
    let now = storage::now();
    let changed = tx.execute(
        "UPDATE sa_worker_write_grants SET revoked=1,revoked_at=?3
         WHERE id=?1 AND ticket_id=?2 AND revoked=0
           AND assignment_id=?4 AND ticket_revision=?5",
        params![grant_id,ticket_id,now,ticket.active_assignment_id,expected_revision],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("The exact current worker write grant was not found or was already revoked.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'worker_write_grant_revoked',?2,?3)",
        params![ticket_id,grant_id,now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    summary_for_ticket(store,ticket_id)
}

pub(crate) fn host(
    app: tauri::AppHandle,
    session_id: String,
    stopped: Arc<AtomicBool>,
) -> Result<Option<ToolHost>> {
    let store = app.state::<AppState>().store.clone();
    host_with_store(store,session_id,stopped)
}

fn host_with_store(
    store: Arc<Mutex<Store>>,
    session_id: String,
    stopped: Arc<AtomicBool>,
) -> Result<Option<ToolHost>> {
    let grant = {
        let store = store.lock().map_err(|e| e.to_string())?;
        current_grant(&store,&session_id,None)?
    };
    let Some(grant) = grant else { return Ok(None) };
    if !crate::workspace::scoped_writes_supported() {
        return Err("policy_unsupported: this platform cannot securely pin checkout ancestors during atomic replacement; scoped worker writes are disabled.".into());
    }
    let spec = json!({
        "type":"function",
        "name":TOOL_NAME,
        "description":"Write UTF-8 text to one exact file path explicitly authorized for this SuperAgent worker assignment. This tool cannot execute shell commands or write any other path. Built-in provider file and shell tools remain read-only.",
        "inputSchema":{
            "type":"object",
            "properties":{
                "grantId":{"type":"string","enum":[grant.summary.id]},
                "path":{"type":"string","enum":grant.summary.allowed_paths},
                "content":{"type":"string","maxLength":MAX_CONTENT_BYTES}
            },
            "required":["grantId","path","content"],
            "additionalProperties":false
        }
    });
    let handler = Arc::new(move |name: String, args: Value| -> crate::providers::ToolFuture {
        let (store,session_id,stopped) = (store.clone(),session_id.clone(),stopped.clone());
        Box::pin(async move {
            if stopped.load(Ordering::SeqCst) {
                return Err("Task has stopped; worker file writes are revoked.".into());
            }
            if name != TOOL_NAME {
                return Err("unsupported_tool: SuperAgent worker write grant does not authorize this tool.".into());
            }
            let grant_id = args.get("grantId").and_then(Value::as_str)
                .ok_or("write_grant_invalid: grantId is required.")?.to_owned();
            let path = args.get("path").and_then(Value::as_str)
                .ok_or("write_grant_invalid: path is required.")?.to_owned();
            let content = args.get("content").and_then(Value::as_str)
                .ok_or("write_grant_invalid: content is required.")?.to_owned();
            if args.as_object().is_none_or(|object| object.len() != 3)
                || content.len() > MAX_CONTENT_BYTES || content.contains('\0')
            {
                return Err("write_grant_invalid: file write arguments exceed the authorized shape or size.".into());
            }
            crate::blocking(move || {
                let store = store.lock().map_err(|e| e.to_string())?;
                if stopped.load(Ordering::SeqCst) {
                    return Err("Task has stopped; worker file writes are revoked.".into());
                }
                apply_write(&store,&session_id,&grant_id,&path,&content,&stopped)
            }).await
        })
    });
    Ok(Some(ToolHost {
        tools: vec![spec],
        handler,
        approval_tools: HashSet::new(),
    }))
}

fn apply_write(
    store: &Store,
    session_id: &str,
    grant_id: &str,
    path: &str,
    content: &str,
    stopped: &AtomicBool,
) -> Result<Value> {
    let grant = current_grant(store,session_id,Some(grant_id))?
        .ok_or("write_grant_not_current: worker has no current write grant.")?;
    if !grant.summary.allowed_paths.iter().any(|allowed| allowed == path) {
        return Err("capability_not_authorized: file path is outside the exact worker grant.".into());
    }
    if grant.actor != "local-user" || content.len() > MAX_CONTENT_BYTES || content.contains('\0') {
        return Err("write_grant_invalid: user authority or bounded text content is required.".into());
    }
    let root = std::path::PathBuf::from(&grant.summary.project_root);
    crate::workspace::validate_scoped_write_path(&root,path)?;
    let digest = format!("{:x}",Sha256::digest(content.as_bytes()));
    let now = storage::now();
    let receipt_id = storage::id();
    store.db.execute(
        "INSERT INTO sa_worker_write_receipts(
           id,grant_id,ticket_id,assignment_id,session_id,ticket_revision,path,
           content_sha256,byte_count,state,created_at,updated_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'reserved',?10,?10)",
        params![receipt_id,grant.summary.id,grant.summary.ticket_id,grant.summary.assignment_id,
            session_id,grant.summary.ticket_revision,path,digest,content.len() as i64,now],
    ).map_err(|e| format!("Could not reserve the native write receipt: {e}"))?;
    // This immediate database write lock serializes the last grant check,
    // filesystem effect and receipt settlement against Stop/revoke in another
    // app process. The Store mutex additionally serializes this app's callers.
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE sa_worker_write_receipts SET updated_at=?2 WHERE id=?1 AND state='reserved'",
        params![receipt_id,storage::now()],
    ).map_err(|e| e.to_string())?;
    let rechecked = current_grant(store,session_id,Some(grant_id))?
        .ok_or("write_grant_not_current: worker has no current write grant.")?;
    if rechecked.summary.assignment_id != grant.summary.assignment_id
        || rechecked.summary.ticket_revision != grant.summary.ticket_revision
        || !rechecked.summary.allowed_paths.iter().any(|allowed| allowed == path)
        || stopped.load(Ordering::SeqCst)
    {
        tx.execute(
            "UPDATE sa_worker_write_receipts SET state='failed',detail='Grant changed before file effect',updated_at=?2 WHERE id=?1",
            params![receipt_id,storage::now()],
        ).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        return Err("write_grant_stale: grant was revoked before the file effect.".into());
    }
    let write_result = crate::workspace::write_scoped_text_file(&root,path,content);
    let settled_at = storage::now();
    let (state,detail) = match write_result {
        Ok(()) => ("applied",String::new()),
        Err(error) => ("failed",error.chars().take(400).collect::<String>()),
    };
    tx.execute(
        "UPDATE sa_worker_write_receipts SET state=?2,detail=?3,updated_at=?4 WHERE id=?1 AND state='reserved'",
        params![receipt_id,state,detail,settled_at],
    ).map_err(|e| e.to_string())?;
    if state == "applied" {
        let audit = json!({
            "grantId":grant.summary.id,
            "assignmentId":grant.summary.assignment_id,
            "sessionId":session_id,
            "path":path,
            "sha256":digest,
            "bytes":content.len(),
            "receiptId":receipt_id,
        }).to_string();
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'worker_file_write_applied',?2,?3)",
            params![grant.summary.ticket_id,audit,settled_at],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    if state != "applied" {
        return Err(format!("Scoped file write failed: {detail}"));
    }
    Ok(json!({
        "grantId":grant.summary.id,
        "ticketId":grant.summary.ticket_id,
        "assignmentId":grant.summary.assignment_id,
        "sessionId":session_id,
        "ticketRevision":grant.summary.ticket_revision,
        "path":path,
        "sha256":digest,
        "bytes":content.len(),
        "receiptId":receipt_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Fixture {
        store: Option<Arc<Mutex<Store>>>,
        database: PathBuf,
        root_a: PathBuf,
        root_b: PathBuf,
        root_c: PathBuf,
        outside_a: PathBuf,
        config_a: String,
        config_b: String,
    }

    impl Fixture {
        fn new() -> Self {
            let id = storage::id();
            let root_a = std::env::temp_dir().join(format!("pandacode-sa-write-{id}-a"));
            let root_b = std::env::temp_dir().join(format!("pandacode-sa-write-{id}-b"));
            let root_c = std::env::temp_dir().join(format!("pandacode-sa-write-{id}-c"));
            let outside_a = std::env::temp_dir().join(format!("pandacode-sa-write-{id}-outside.txt"));
            for (root, contents) in [(&root_a,"root-a"),(&root_b,"root-b"),(&root_c,"root-c")] {
                fs::create_dir_all(root.join("src")).unwrap();
                fs::write(root.join("src/allowed.txt"),format!("{contents} before")).unwrap();
                fs::write(root.join("src/other.txt"),format!("{contents} other")).unwrap();
            }
            fs::write(&outside_a,"outside before").unwrap();
            let database = std::env::temp_dir().join(format!("pandacode-sa-write-{id}.sqlite3"));
            let store = Store::open(&database).unwrap();
            for (project,name,root) in [
                ("project-a","Write fixture A",&root_a),
                ("project-b","Write fixture B",&root_b),
            ] {
                store.db.execute(
                    "INSERT INTO projects(id,name,path) VALUES(?1,?2,?3)",
                    params![project,name,root.to_string_lossy()],
                ).unwrap();
            }
            for (id,name,project) in [
                ("router-a","Router A","project-a"),("worker-a","Worker A","project-a"),
                ("router-b","Router B","project-b"),("worker-b","Worker B","project-b"),
            ] {
                store.db.execute(
                    "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                     VALUES(?1,?2,?3,'codex','default','plan',?4,?4)",
                    params![id,name,project,storage::now()],
                ).unwrap();
            }
            let config_a = Self::enabled_config(&store,"project-a","router-a","worker-a","Write fixture A");
            let config_b = Self::enabled_config(&store,"project-b","router-b","worker-b","Write fixture B");
            Self {
                store: Some(Arc::new(Mutex::new(store))),
                database,
                root_a,
                root_b,
                root_c,
                outside_a,
                config_a,
                config_b,
            }
        }

        fn enabled_config(store: &Store, project: &str, coordinator: &str, worker: &str, name: &str) -> String {
            let config = super::create_configuration(store,super::NewOrchestration {
                name:name.into(),coordinator_bot_id:coordinator.into(),worker_bot_id:worker.into(),
            }).unwrap();
            assert_eq!(config.project_id,project);
            store.db.execute(
                "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
                [&config.id],
            ).unwrap();
            config.id
        }

        fn store(&self) -> std::sync::MutexGuard<'_,Store> {
            self.store.as_ref().expect("fixture store").lock().unwrap()
        }

        fn queued_ticket(&self, config_id: &str, request_key: &str) -> super::Ticket {
            let ticket_id = {
                let mut store = self.store();
                let (ticket,started) = super::admit_role(
                    &mut store,config_id,"Write one exact file",request_key,false,true,
                ).unwrap();
                assert!(started);
                let session_id = super::assignment_session(&store,&ticket.id).unwrap().unwrap();
                assert!(super::on_run_finished(&store,&session_id,true,"Use the smallest safe change").unwrap());
                ticket.id
            };
            let store = self.store();
            super::get_ticket(&store,&ticket_id).unwrap()
        }

        #[cfg(windows)]
        fn issue(&self, config_id: &str, request_key: &str, paths: &[String]) -> WriteContext {
            let queued = self.queued_ticket(config_id,request_key);
            let (ticket,session_id,grant) = {
                let mut store = self.store();
                let (ticket,_,_,_) = super::prepare_coordinated_worker_with_write_grant(
                    &mut store,&queued.id,false,queued.revision,paths,
                ).unwrap();
                let session_id = super::assignment_session(&store,&ticket.id).unwrap().unwrap();
                let grant = summary_for_ticket(&store,&ticket.id).unwrap();
                (ticket,session_id,grant)
            };
            let host = host_with_store(
                self.store.as_ref().expect("fixture store").clone(),
                session_id.clone(),Arc::new(AtomicBool::new(false)),
            ).unwrap().expect("current user grant creates one ToolHost");
            WriteContext { ticket,session_id,grant,host }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.store.take();
            for path in [
                self.database.clone(),PathBuf::from(format!("{}-wal",self.database.display())),
                PathBuf::from(format!("{}-shm",self.database.display())),
            ] {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_dir_all(&self.root_a);
            let _ = fs::remove_dir_all(&self.root_b);
            let _ = fs::remove_dir_all(&self.root_c);
            let _ = fs::remove_file(&self.outside_a);
        }
    }

    #[cfg(windows)]
    struct WriteContext {
        ticket: super::Ticket,
        session_id: String,
        grant: WriteGrantSummary,
        host: ToolHost,
    }

    #[cfg(windows)]
    async fn call(host: &ToolHost, tool: &str, grant_id: &str, path: &str, content: &str) -> Result<Value> {
        (host.handler)(tool.into(),json!({"grantId":grant_id,"path":path,"content":content})).await
    }

    #[cfg(windows)]
    fn allowed_paths() -> Vec<String> { vec!["src/allowed.txt".into()] }

    #[test]
    fn grant_path_syntax_is_exact_and_rejects_traversal() {
        assert!(validate_paths(&["src/main.rs".into()]).is_ok());
        for path in ["../outside.rs", "src/../outside.rs", ".git/config", "C:/outside", "src\\main.rs"] {
            assert!(validate_paths(&[path.into()]).is_err(),"{path}");
        }
        assert!(validate_paths(&["src/main.rs".into(),"src/main.rs".into()]).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn editing_a_future_team_version_keeps_an_active_ticket_snapshot_and_grant_unchanged() {
        let fixture=Fixture::new();
        let old_ticket=fixture.queued_ticket(&fixture.config_a,"snapshot-grant-edit");
        let paths=vec!["src/allowed.txt".to_owned()];
        let (worker_ticket,session_id,grant_before,old_snapshot) = {
            let mut store=fixture.store();
            let old_snapshot=super::super::ticket_team_snapshot(&store,&old_ticket.id).unwrap();
            let (running,_,_,_)=super::super::prepare_coordinated_worker_inner(
                &mut store,&old_ticket.id,false,None,Some(&paths),
            ).unwrap();
            let session_id=super::super::assignment_session(&store,&running.id).unwrap().unwrap();
            let grant=summary_for_ticket(&store,&running.id).unwrap();
            assert!(current_grant(&store,&session_id,Some(&grant.id)).unwrap().is_some());
            (running,session_id,grant,old_snapshot)
        };
        {
            let store=fixture.store();
            let (project,provider): (String,String)=store.db.query_row(
                "SELECT project_id,provider FROM bots WHERE id='worker-a'",[],
                |r| Ok((r.get(0)?,r.get(1)?)),
            ).unwrap();
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES('worker-extra','Future worker',?1,?2,'default','plan',?3,?3)",
                params![project,provider,storage::now()],
            ).unwrap();
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES('reviewer-extra','Future reviewer',?1,?2,'default','plan',?3,?3)",
                params![project,provider,storage::now()],
            ).unwrap();
            let current=super::super::configuration(&store,&fixture.config_a).unwrap();
            let updated=super::super::update_configuration_roles(
                &store,&current.id,current.policy_revision,&super::super::NewOrchestration{
                    name:"Write fixture A v2".into(),coordinator_bot_id:"router-a".into(),
                    worker_bot_id:"worker-a".into(),
                },vec!["worker-a".into(),"worker-extra".into()],vec!["reviewer-extra".into()],
            ).unwrap();
            assert_eq!(updated.team_version,2);
            assert_eq!(updated.worker_bot_ids,vec!["worker-a","worker-extra"]);
            assert_eq!(updated.reviewer_bot_ids,vec!["reviewer-extra"]);
            assert_eq!(super::super::ticket_team_snapshot(&store,&worker_ticket.id).unwrap(),old_snapshot);
            assert_eq!(serde_json::to_value(summary_for_ticket(&store,&worker_ticket.id).unwrap()).unwrap(),
                serde_json::to_value(&grant_before).unwrap());
            let active=current_grant(&store,&session_id,Some(&grant_before.id)).unwrap().unwrap();
            assert_eq!(serde_json::to_value(active.summary).unwrap(),serde_json::to_value(&grant_before).unwrap());
            let (future,launched)=super::super::admit(
                &mut store,&fixture.config_a,"Future ticket","snapshot-grant-future",true,
            ).unwrap();
            assert!(!launched);
            let future_snapshot=super::super::ticket_team_snapshot(&store,&future.id).unwrap();
            assert_eq!(future_snapshot.team_version,2);
            assert_eq!(future_snapshot.worker_bot_ids,vec!["worker-a","worker-extra"]);
            assert_eq!(future_snapshot.reviewer_bot_ids,vec!["reviewer-extra"]);
            assert!(current_grant(&store,&session_id,Some(&grant_before.id)).unwrap().is_some());
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn exact_current_ticket_assignment_session_and_root_grant_writes_through_tool_host() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-current",&allowed_paths());
        assert_eq!(context.host.tools.len(),1);
        assert_eq!(context.host.tools[0]["name"],TOOL_NAME);
        assert_eq!(context.grant.ticket_id,context.ticket.id);
        assert_eq!(context.grant.session_id,context.session_id);
        assert_eq!(context.grant.project_root,fixture.root_a.canonicalize().unwrap().to_string_lossy());
        let result = call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","authorized contents").await.unwrap();
        assert_eq!(result["ticketId"],context.ticket.id);
        assert_eq!(result["assignmentId"],context.grant.assignment_id);
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"authorized contents");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn over_scope_and_path_traversal_calls_are_denied_without_file_effects() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-scope",&allowed_paths());
        let before_other = fs::read_to_string(fixture.root_a.join("src/other.txt")).unwrap();
        let before_outside = fs::read_to_string(&fixture.outside_a).unwrap();
        let traversal = format!("../{}",fixture.outside_a.file_name().unwrap().to_string_lossy());
        assert!(validate_paths_in_root(&fixture.root_a,&["../outside.txt".into()]).is_err());
        for path in ["src/other.txt",traversal.as_str()] {
            let error = call(&context.host,TOOL_NAME,&context.grant.id,path,"must not land").await.unwrap_err();
            assert!(error.contains("capability_not_authorized"),"{error}");
        }
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/other.txt")).unwrap(),before_other);
        assert_eq!(fs::read_to_string(&fixture.outside_a).unwrap(),before_outside);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stale_revision_and_revoked_grants_reject_late_tool_callbacks() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-stale",&allowed_paths());
        {
            let store = fixture.store();
            store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1",[&context.ticket.id]).unwrap();
        }
        assert!(call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","stale").await.is_err());

        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-revoked",&allowed_paths());
        {
            let store = fixture.store();
            let revoked = revoke(&store,&context.ticket.id,&context.grant.id,context.ticket.revision).unwrap();
            assert!(revoked.revoked);
        }
        assert!(call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","revoked").await.is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stop_revokes_the_worker_tool_and_rejects_late_write_and_result() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-stop-late-callback",&allowed_paths());
        let (stopped_ticket,stopped_session) = {
            let store = fixture.store();
            super::stop(&store,&context.ticket.id).unwrap()
        };
        assert_eq!(stopped_ticket.state,"stopped");
        assert_eq!(stopped_session.as_deref(),Some(context.session_id.as_str()));

        let error = call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","late write").await.unwrap_err();
        assert!(error.contains("write_grant") || error.contains("policy_unsupported"),"{error}");
        {
            let store = fixture.store();
            assert!(!super::on_run_finished(&store,&context.session_id,true,"late success").unwrap());
            assert!(summary_for_ticket(&store,&context.ticket.id).unwrap().revoked);
            let receipts: i64 = store.db.query_row(
                "SELECT COUNT(*) FROM sa_worker_write_receipts WHERE ticket_id=?1",[&context.ticket.id],|row|row.get(0),
            ).unwrap();
            assert_eq!(receipts,0);
        }
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"root-a before");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn provider_cancel_flag_denies_before_reserving_a_write_receipt() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-cancel-flag",&allowed_paths());
        let stopped = Arc::new(AtomicBool::new(false));
        let host = host_with_store(
            fixture.store.as_ref().unwrap().clone(),context.session_id.clone(),stopped.clone(),
        ).unwrap().unwrap();
        stopped.store(true,Ordering::SeqCst);

        let error = call(&host,TOOL_NAME,&context.grant.id,"src/allowed.txt","cancelled").await.unwrap_err();
        assert!(error.contains("stopped"),"{error}");
        {
            let store = fixture.store();
            let receipts: i64 = store.db.query_row(
                "SELECT COUNT(*) FROM sa_worker_write_receipts WHERE ticket_id=?1",[&context.ticket.id],|row|row.get(0),
            ).unwrap();
            assert_eq!(receipts,0);
        }
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"root-a before");
    }

    #[tokio::test]
    async fn coordinator_and_unassigned_reviewer_sessions_receive_no_worker_write_tool() {
        let fixture = Fixture::new();
        let (coordinator_session,reviewer_session) = {
            let mut store = fixture.store();
            let (ticket,started) = super::admit_role(
                &mut store,&fixture.config_a,"Read-only coordinator task","write-seat-denial",false,true,
            ).unwrap();
            assert!(started);
            let coordinator_session = super::assignment_session(&store,&ticket.id).unwrap().unwrap();
            let reviewer_session = store.create_session("project-a","codex","default","plan").unwrap().id;
            (coordinator_session,reviewer_session)
        };
        let store = fixture.store.as_ref().unwrap().clone();

        assert!(host_with_store(store.clone(),coordinator_session,Arc::new(AtomicBool::new(false)))
            .unwrap().is_none());
        assert!(host_with_store(store,reviewer_session,Arc::new(AtomicBool::new(false)))
            .unwrap().is_none());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn changing_a_worker_assignment_to_coordinator_revokes_its_existing_host() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-role-drift",&allowed_paths());
        {
            let store = fixture.store();
            store.db.execute(
                "UPDATE sa_assignments SET bot_id='router-a' WHERE id=?1",[&context.grant.assignment_id],
            ).unwrap();
        }

        let error = call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","role changed").await.unwrap_err();
        assert!(error.contains("write_grant") || error.contains("policy_unsupported"),"{error}");
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"root-a before");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn write_payload_is_bounded_by_bytes_before_receipt_reservation() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-content-budget",&allowed_paths());
        let oversized = "x".repeat(MAX_CONTENT_BYTES + 1);

        let error = call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt",&oversized).await.unwrap_err();
        assert!(error.contains("write_grant_invalid"),"{error}");
        {
            let store = fixture.store();
            let receipts: i64 = store.db.query_row(
                "SELECT COUNT(*) FROM sa_worker_write_receipts WHERE ticket_id=?1",[&context.ticket.id],|row|row.get(0),
            ).unwrap();
            assert_eq!(receipts,0);
        }
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"root-a before");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn grants_from_other_tickets_and_sessions_do_not_cross_the_tool_host_boundary() {
        let fixture = Fixture::new();
        let a = fixture.issue(&fixture.config_a,"write-cross-a",&allowed_paths());
        let b = fixture.issue(&fixture.config_b,"write-cross-b",&allowed_paths());
        assert_ne!(a.ticket.id,b.ticket.id);
        assert_ne!(a.session_id,b.session_id);
        assert!(call(&a.host,TOOL_NAME,&b.grant.id,"src/allowed.txt","cross-ticket").await.is_err());
        assert!(call(&b.host,TOOL_NAME,&a.grant.id,"src/allowed.txt","cross-session").await.is_err());
        assert_eq!(fs::read_to_string(fixture.root_a.join("src/allowed.txt")).unwrap(),"root-a before");
        assert_eq!(fs::read_to_string(fixture.root_b.join("src/allowed.txt")).unwrap(),"root-b before");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn changed_project_root_revokes_the_original_grant_scope() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-root-drift",&allowed_paths());
        {
            let store = fixture.store();
            let moved_root = fixture.root_c.to_string_lossy().into_owned();
            store.db.execute("UPDATE projects SET path=?1 WHERE id='project-a'",[moved_root]).unwrap();
        }
        let error = call(&context.host,TOOL_NAME,&context.grant.id,"src/allowed.txt","must not retarget").await.unwrap_err();
        assert!(error.contains("write_grant_stale"),"{error}");
        assert_eq!(fs::read_to_string(fixture.root_c.join("src/allowed.txt")).unwrap(),"root-c before");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn unsupported_tool_and_non_codex_scoped_dispatch_are_denied() {
        let fixture = Fixture::new();
        let context = fixture.issue(&fixture.config_a,"write-unsupported-tool",&allowed_paths());
        let error = call(&context.host,"shell_exec",&context.grant.id,"src/allowed.txt","touch forbidden").await.unwrap_err();
        assert!(error.contains("unsupported_tool"),"{error}");

        let queued = fixture.queued_ticket(&fixture.config_b,"write-unsupported-provider");
        {
            let mut store = fixture.store();
            store.db.execute("UPDATE bots SET provider='claude' WHERE id='worker-b'",[]).unwrap();
            let error = super::prepare_coordinated_worker_with_write_grant(
                &mut *store,&queued.id,false,queued.revision,&allowed_paths(),
            ).unwrap_err();
            assert!(error.contains("policy_unsupported"),"{error}");
        }
    }
}
