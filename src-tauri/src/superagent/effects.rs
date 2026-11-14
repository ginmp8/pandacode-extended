use crate::{github, storage::{self, Store}, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    time::Duration,
};

const EFFECT_MARKER: &str = "<!-- pandacode-superagent-effect:";
const MAX_COMMENT_BYTES: usize = 60_000;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_effect_receipts (
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 effect_key TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','uncertain','applied')),
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key)
);
CREATE INDEX IF NOT EXISTS sa_effect_unresolved ON sa_effect_receipts(ticket_id,state)
 WHERE state!='applied';
CREATE TABLE IF NOT EXISTS sa_effect_grants (
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 effect_key TEXT NOT NULL,
 ticket_revision INTEGER NOT NULL,
 project_id TEXT NOT NULL,
 repository TEXT NOT NULL,
 issue_number INTEGER NOT NULL CHECK(issue_number>0),
 comment_body TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key)
);
"#;

pub(crate) const GRANT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_effect_grants (
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 effect_key TEXT NOT NULL,
 ticket_revision INTEGER NOT NULL,
 project_id TEXT NOT NULL,
 repository TEXT NOT NULL,
 issue_number INTEGER NOT NULL CHECK(issue_number>0),
 comment_body TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key)
);
"#;

pub(crate) const RECOVER: &str =
    "UPDATE sa_effect_receipts SET state='uncertain',uncertainty_reason='process_restarted_before_settlement',readback_state='not_attempted',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE state='pending';";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Reservation {
    Reserved,
    AlreadyPending,
    RequiresReconciliation,
    AlreadyApplied,
}

pub(crate) fn open_effect_store(database_path: &Path) -> Result<Store> {
    let db = Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version != storage::SCHEMA_VERSION {
        return Err(format!(
            "Database schema {version} differs from expected {}; open PandaCode to migrate it.",
            storage::SCHEMA_VERSION
        ));
    }
    Ok(Store { db, path: database_path.into() })
}

/// A cross-process fence ordering effect grants and the final live-authority
/// check with Stop's mandate revocation. The GitHub write call stays under the
/// same fence as its final live-authority check. This is a file lock rather
/// than a SQLite transaction, so the bounded `gh` wait never holds a database
/// transaction open. The lock is released automatically if a process exits
/// unexpectedly.
pub(crate) struct DispatchFence {
    _file: File,
}

#[cfg(test)]
std::thread_local! {
    static DISPATCH_FENCE_ACQUIRED_OBSERVER: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
    static DISPATCH_FENCE_WAITING_OBSERVER: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn observe_next_dispatch_fence_acquired(observer: impl FnOnce() + 'static) {
    DISPATCH_FENCE_ACQUIRED_OBSERVER.with(|slot| *slot.borrow_mut() = Some(Box::new(observer)));
}

#[cfg(test)]
fn observe_next_dispatch_fence_waiting(observer: impl FnOnce() + 'static) {
    DISPATCH_FENCE_WAITING_OBSERVER.with(|slot| *slot.borrow_mut() = Some(Box::new(observer)));
}

#[cfg(test)]
fn notify_dispatch_fence_acquired() {
    let observer = DISPATCH_FENCE_ACQUIRED_OBSERVER.with(|slot| slot.borrow_mut().take());
    if let Some(observer) = observer {
        observer();
    }
}

#[cfg(test)]
fn notify_dispatch_fence_waiting() {
    let observer = DISPATCH_FENCE_WAITING_OBSERVER.with(|slot| slot.borrow_mut().take());
    if let Some(observer) = observer {
        observer();
    }
}

pub(crate) fn lock_dispatch(database_path: &Path) -> Result<DispatchFence> {
    lock_dispatch_before_wait(database_path, || {})
}

pub(crate) fn lock_dispatch_before_wait(
    database_path: &Path,
    before_wait: impl FnOnce(),
) -> Result<DispatchFence> {
    let lock_path = database_path.with_extension("superagent-effect-dispatch.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|e| e.to_string())?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            before_wait();
            #[cfg(test)]
            notify_dispatch_fence_waiting();
            file.lock().map_err(|e| e.to_string())?;
        }
        Err(std::fs::TryLockError::Error(error)) => return Err(error.to_string()),
    }
    #[cfg(test)]
    notify_dispatch_fence_acquired();
    Ok(DispatchFence { _file: file })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueCommentIntent {
    pub ticket_id: String,
    pub expected_revision: i64,
    pub repository: String,
    pub issue_number: i64,
    pub effect_key: String,
    pub comment_body: String,
    pub payload_sha256: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectStatus {
    pub ticket_id: String,
    pub ticket_revision: i64,
    pub repository: String,
    pub issue_number: i64,
    pub effect_key: String,
    pub payload_sha256: String,
    pub state: String,
    pub reconciliation: String,
    pub uncertainty_reason: Option<String>,
    pub readback_state: Option<String>,
    pub remote_comment_id: Option<String>,
    pub remote_comment_url: Option<String>,
    pub remote_comment_author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Readback {
    Exact(RemoteCommentEvidence),
    Missing,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteCommentEvidence {
    id: Option<String>,
    url: Option<String>,
    author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrantBinding {
    project_id: String,
    assignment_id: String,
    session_id: String,
}

pub(crate) fn validate_repository(repository: &str) -> Result<()> {
    let mut parts = repository.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let valid_component = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
            })
    };
    if parts.next().is_some() || !valid_component(owner) || !valid_component(name) {
        return Err("Provide one exact GitHub repository in owner/name form.".into());
    }
    Ok(())
}

pub(crate) fn validate_effect_key(key: &str) -> Result<()> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("Provide a bounded stable effect key using letters, digits, '.', '_' , ':' or '-'.".into());
    }
    Ok(())
}

pub(crate) fn prepare_issue_comment(
    ticket_id: &str,
    expected_revision: i64,
    repository: &str,
    issue_number: i64,
    effect_key: &str,
    comment: &str,
) -> Result<IssueCommentIntent> {
    if ticket_id.is_empty()
        || ticket_id.len() > 128
        || !ticket_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || expected_revision < 0
    {
        return Err("Provide a valid Ticket ID and nonnegative Ticket revision.".into());
    }
    validate_repository(repository)?;
    validate_effect_key(effect_key)?;
    if issue_number <= 0 {
        return Err("Provide a positive GitHub issue number.".into());
    }
    if comment.trim().is_empty()
        || comment.len() > MAX_COMMENT_BYTES
        || comment.contains('\0')
        || comment.contains(EFFECT_MARKER)
    {
        return Err("Provide a nonempty issue comment within the size limit and without a PandaCode effect marker.".into());
    }
    let marker = format!("{EFFECT_MARKER}ticket={ticket_id};key={effect_key} -->");
    let comment_body = format!("{comment}\n\n{marker}");
    let payload_sha256 = digest_intent(
        ticket_id,
        expected_revision,
        repository,
        issue_number,
        effect_key,
        &comment_body,
    )?;
    Ok(IssueCommentIntent {
        ticket_id: ticket_id.into(),
        expected_revision,
        repository: repository.into(),
        issue_number,
        effect_key: effect_key.into(),
        comment_body,
        payload_sha256,
    })
}

fn digest_intent(
    ticket_id: &str,
    expected_revision: i64,
    repository: &str,
    issue_number: i64,
    effect_key: &str,
    comment_body: &str,
) -> Result<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload<'a> {
        ticket_id: &'a str,
        expected_revision: i64,
        repository: &'a str,
        issue_number: i64,
        effect_key: &'a str,
        comment_body: &'a str,
    }
    let bytes = serde_json::to_vec(&Payload {
        ticket_id,
        expected_revision,
        repository,
        issue_number,
        effect_key,
        comment_body,
    })
    .map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn intent_marker(intent: &IssueCommentIntent) -> String {
    format!(
        "{EFFECT_MARKER}ticket={};key={} -->",
        intent.ticket_id, intent.effect_key
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveScope {
    project_id: String,
    assignment_id: String,
    session_id: String,
}

fn active_scope_if_current(
    db: &Connection,
    intent: &IssueCommentIntent,
) -> Result<Option<ActiveScope>> {
    if crate::feature_controls::get().bots_paused {
        return Ok(None);
    }
    db.query_row(
        "SELECT t.project_id,a.id,a.session_id FROM sa_tickets t
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         JOIN sa_assignments a ON a.id=t.active_assignment_id
           AND a.ticket_id=t.id AND a.state='running' AND a.bot_id=t.worker_bot_id
         JOIN bots b ON b.id=a.bot_id
         WHERE t.id=?1 AND t.revision=?2 AND t.state='running' AND m.revoked=0
           AND m.expires_at>?3 AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision AND t.project_id=m.project_id
           AND m.project_id=o.project_id AND t.actor=m.actor AND m.authorization_id<>''
           AND b.project_id=t.project_id AND b.paused=0 AND o.enabled=1",
        params![intent.ticket_id, intent.expected_revision, storage::now()],
        |row| Ok(ActiveScope {
            project_id: row.get(0)?,
            assignment_id: row.get(1)?,
            session_id: row.get(2)?,
        }),
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn active_scope(db: &Connection, intent: &IssueCommentIntent) -> Result<ActiveScope> {
    if crate::feature_controls::get().bots_paused {
        return Err(
            "Native issue-comment authorization is unavailable while Bots are paused.".into(),
        );
    }
    active_scope_if_current(db, intent)?.ok_or_else(|| {
        "The Ticket, revision, mandate, or active worker no longer permits this effect.".into()
    })
}

// Keep durable consent separate from current execution authority. A stored
// receipt may be reconciled after revocation, but only against the complete
// immutable grant binding and its original assignment/session.
fn exact_grant_binding(db: &Connection, intent: &IssueCommentIntent) -> Result<GrantBinding> {
    let grant: Option<(i64, String, String, i64, String, String)> = db
        .query_row(
            "SELECT ticket_revision,project_id,repository,issue_number,comment_body,payload_sha256
             FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((revision, project_id, repository, issue, body, digest)) = grant else {
        return Err("The exact native effect grant is missing.".into());
    };
    if revision != intent.expected_revision
        || repository != intent.repository
        || issue != intent.issue_number
        || body != intent.comment_body
        || digest != intent.payload_sha256
    {
        return Err(
            "The native effect grant does not match this exact Ticket, target, key, and payload."
                .into(),
        );
    }
    let detail: Option<String> = db
        .query_row(
            "SELECT detail FROM sa_audit
             WHERE ticket_id=?1 AND kind='issue_comment_granted'
               AND json_extract(detail,'$.effectKey')=?2
               AND json_extract(detail,'$.payloadSha256')=?3
             ORDER BY id DESC LIMIT 1",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let detail: serde_json::Value = serde_json::from_str(
        detail
            .as_deref()
            .ok_or_else(|| "The exact native effect grant binding is missing.".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let assignment_id = detail
        .get("assignmentId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| "The exact native effect assignment binding is invalid.".to_string())?;
    let session_id = detail
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| "The exact native effect session binding is invalid.".to_string())?;
    let project_binding = detail
        .get("projectId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| *value == project_id.as_str())
        .ok_or_else(|| "The exact native effect project binding is invalid.".to_string())?;
    let exact_detail = detail.get("ticketRevision").and_then(serde_json::Value::as_i64)
        == Some(intent.expected_revision)
        && detail.get("repository").and_then(serde_json::Value::as_str)
            == Some(intent.repository.as_str())
        && detail.get("issueNumber").and_then(serde_json::Value::as_i64)
            == Some(intent.issue_number)
        && detail.get("effectKey").and_then(serde_json::Value::as_str)
            == Some(intent.effect_key.as_str())
        && detail.get("payloadSha256").and_then(serde_json::Value::as_str)
            == Some(intent.payload_sha256.as_str());
    if !exact_detail {
        return Err("The native effect grant audit does not match this exact intent.".into());
    }
    let assignment_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_assignments
             WHERE id=?1 AND ticket_id=?2 AND session_id=?3)",
            params![assignment_id, intent.ticket_id, session_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let ticket_project: Option<String> = db
        .query_row(
            "SELECT project_id FROM sa_tickets WHERE id=?1",
            [&intent.ticket_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if !assignment_exists || ticket_project.as_deref() != Some(project_binding) {
        return Err("The native effect grant no longer matches its original assignment and project.".into());
    }
    let session_project: Option<String> = db
        .query_row(
            "SELECT project_id FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if session_project.as_deref().is_some_and(|value| value != project_binding) {
        return Err("The native effect session has moved outside its granted project.".into());
    }
    Ok(GrantBinding {
        project_id,
        assignment_id: assignment_id.into(),
        session_id: session_id.into(),
    })
}

// Kept separate for call sites that only need to locate the exact workspace.
fn exact_grant_payload(db: &Connection, intent: &IssueCommentIntent) -> Result<String> {
    Ok(exact_grant_binding(db, intent)?.project_id)
}

fn exact_grant_scope_matches(
    db: &Connection,
    intent: &IssueCommentIntent,
    scope: &ActiveScope,
) -> Result<bool> {
    let binding = exact_grant_binding(db, intent)?;
    Ok(binding.project_id == scope.project_id
        && binding.assignment_id == scope.assignment_id
        && binding.session_id == scope.session_id)
}

fn exact_grant(db: &Connection, intent: &IssueCommentIntent) -> Result<ActiveScope> {
    let scope = active_scope(db, intent)?;
    if exact_grant_payload(db, intent)? != scope.project_id
        || !exact_grant_scope_matches(db, intent, &scope)?
    {
        return Err(
            "The native effect grant does not match this exact Ticket, assignment, session, target, key, and payload.".into(),
        );
    }
    Ok(scope)
}

/// Revalidate the exact task-owned Worker immediately before reserving or
/// dispatching a remote effect. Immutable receipt/grant IDs alone are not
/// sufficient authority after a bot thread or session profile changes.
fn attested_worker_scope(
    store: &Store, intent: &IssueCommentIntent, scope: &ActiveScope,
) -> Result<()> {
    let snapshot = super::ticket_team_snapshot(store, &intent.ticket_id)?;
    let attempt_timeout = snapshot.policy.strategy.attempt_timeout_ms;
    let assignment: Option<(String, i64)> = store.db.query_row(
        "SELECT bot_id,attempt FROM sa_assignments
         WHERE id=?1 AND ticket_id=?2 AND session_id=?3 AND state='running'
           AND created_at>0 AND created_at<=?4
           AND (?4-created_at)<?5
           AND (SELECT COUNT(*) FROM sa_assignments other
                WHERE other.ticket_id=?2 AND other.state='running')=1",
        params![scope.assignment_id, intent.ticket_id, scope.session_id,
            storage::now(), attempt_timeout],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let (bot_id, attempt) = assignment
        .ok_or("Native effect worker assignment is no longer active.")?;
    let ordinal = snapshot.worker_bot_ids.iter().position(|id| id == &bot_id)
        .ok_or("Native effect assignment is not an authorized Worker.")?;
    let session = store.session(&scope.session_id)?;
    if session.project_id != scope.project_id
        || !super::assignment_thread_current(store, &scope.session_id, &intent.ticket_id)?
        || !super::assignment_role_authorized(store, &intent.ticket_id, &bot_id, attempt)?
        || !super::admitted_session_matches_profile(
            &snapshot, &bot_id, "worker", ordinal, &session,
        )
        || !super::snapshot_role_profile_matches(
            &store.db, &snapshot, "worker", ordinal, &bot_id,
        )?
    {
        return Err("Native effect Worker session or admitted role profile changed.".into());
    }
    Ok(())
}

pub(crate) fn has_exact_grant(store: &Store, intent: &IssueCommentIntent) -> Result<bool> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT payload_sha256 FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let present = match existing {
        None => {
            active_scope(&tx, intent)?;
            false
        }
        Some(digest) if digest == intent.payload_sha256 => {
            let grant_project = exact_grant_payload(&tx, intent)?;
            match active_scope_if_current(&tx, intent)? {
                Some(scope)
                    if scope.project_id == grant_project
                        && exact_grant_scope_matches(&tx, intent, &scope)? => true,
                _ => {
                    let receipt_state: Option<String> = tx
                        .query_row(
                            "SELECT state FROM sa_effect_receipts
                             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
                            params![intent.ticket_id, intent.effect_key, intent.payload_sha256],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| e.to_string())?;
                    if matches!(receipt_state.as_deref(), Some("pending" | "uncertain")) {
                        true
                    } else {
                        exact_grant(&tx, intent)?;
                        true
                    }
                }
            }
        }
        Some(_) => return Err("Effect key already has a grant for a different exact payload.".into()),
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(present)
}

pub(crate) fn authorize(store: &Store, intent: &IssueCommentIntent) -> Result<()> {
    authorize_with_lock_observer(store, intent, || {})
}

fn authorize_with_lock_observer(
    store: &Store,
    intent: &IssueCommentIntent,
    before_wait: impl FnOnce(),
) -> Result<()> {
    let _dispatch_fence = lock_dispatch_before_wait(&store.path, before_wait)?;
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let scope = active_scope(&tx, intent)?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT payload_sha256 FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match existing {
        Some(digest) if digest == intent.payload_sha256 => {
            exact_grant(&tx, intent)?;
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }
        Some(_) => return Err("Effect key already has a grant for a different exact payload.".into()),
        None => {}
    }
    let now = storage::now();
    tx.execute(
        "INSERT INTO sa_effect_grants(ticket_id,effect_key,ticket_revision,project_id,repository,
         issue_number,comment_body,payload_sha256,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            intent.ticket_id,
            intent.effect_key,
            intent.expected_revision,
            scope.project_id,
            intent.repository,
            intent.issue_number,
            intent.comment_body,
            intent.payload_sha256,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    let detail = serde_json::json!({
        "effectKey": intent.effect_key,
        "ticketRevision": intent.expected_revision,
        "projectId": scope.project_id,
        "assignmentId": scope.assignment_id,
        "sessionId": scope.session_id,
        "repository": intent.repository,
        "issueNumber": intent.issue_number,
        "payloadSha256": intent.payload_sha256,
    })
    .to_string();
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'issue_comment_granted',?2,?3)",
        params![intent.ticket_id, detail, now],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

pub(crate) fn reserve(store: &Store, intent: &IssueCommentIntent) -> Result<Reservation> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT payload_sha256,state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let result = match existing {
        Some((previous, _)) if previous != intent.payload_sha256 => {
            return Err("Effect key already used for a different payload.".into())
        }
        Some((_, state)) => {
            let grant_project = exact_grant_payload(&tx, intent)?;
            match state.as_str() {
                "pending" => match active_scope_if_current(&tx, intent)? {
                    Some(scope)
                        if scope.project_id == grant_project
                            && exact_grant_scope_matches(&tx, intent, &scope)? => Reservation::AlreadyPending,
                    _ => Reservation::RequiresReconciliation,
                },
                "uncertain" => Reservation::RequiresReconciliation,
                "applied" => Reservation::AlreadyApplied,
                _ => return Err("Invalid effect state.".into()),
            }
        }
        None => {
            let scope = exact_grant(&tx, intent)?;
            attested_worker_scope(store, intent, &scope)?;
            let now = storage::now();
            tx.execute(
                "INSERT INTO sa_effect_receipts(ticket_id,effect_key,payload_sha256,state,created_at,updated_at,readback_state)
                 VALUES(?1,?2,?3,'pending',?4,?4,'not_attempted')",
                params![intent.ticket_id, intent.effect_key, intent.payload_sha256, now],
            )
            .map_err(|e| e.to_string())?;
            Reservation::Reserved
        }
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

fn validate_pending_dispatch(store: &Store, intent: &IssueCommentIntent) -> Result<ActiveScope> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let scope = exact_grant(&tx, intent)?;
    attested_worker_scope(store, intent, &scope)?;
    let state: Option<String> = tx
        .query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2
             AND payload_sha256=?3",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if state.as_deref() != Some("pending") {
        return Err("The effect receipt is no longer pending; dispatch was not attempted.".into());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(scope)
}

fn mark_uncertain(
    store: &Store,
    intent: &IssueCommentIntent,
    reason: &str,
    readback_state: &str,
) -> Result<()> {
    let n = store
        .db
        .execute(
            "UPDATE sa_effect_receipts SET state='uncertain',uncertainty_reason=?4,
             readback_state=?5,remote_comment_id=NULL,remote_comment_url=NULL,
             remote_author_login=NULL,updated_at=?6
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
               AND state IN ('pending','uncertain')",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256,
                reason, readback_state, storage::now()],
        )
        .map_err(|e| e.to_string())?;
    if n == 1 {
        return Ok(());
    }
    let state: Option<String> = store
        .db
        .query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if matches!(state.as_deref(), Some("uncertain" | "applied")) {
        Ok(())
    } else {
        Err("No unresolved matching effect receipt.".into())
    }
}

fn settle_applied(
    store: &Store,
    intent: &IssueCommentIntent,
    evidence: &RemoteCommentEvidence,
) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    exact_grant_payload(&tx, intent)?;
    let changed = tx
        .execute(
            "UPDATE sa_effect_receipts SET state='applied',uncertainty_reason=NULL,
             readback_state='exact',remote_comment_id=?4,remote_comment_url=?5,
             remote_author_login=?6,updated_at=?7
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
               AND state IN ('pending','uncertain')",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256,
                evidence.id, evidence.url, evidence.author, storage::now()],
        )
        .map_err(|e| e.to_string())?;
    if changed != 1 {
        let state: Option<String> = tx
            .query_row(
                "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2
                 AND payload_sha256=?3",
                params![intent.ticket_id, intent.effect_key, intent.payload_sha256],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if state.as_deref() == Some("applied") {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }
        return Err("Missing unresolved matching effect receipt.".into());
    }
    let detail = serde_json::json!({
        "effectKey": intent.effect_key,
        "ticketRevision": intent.expected_revision,
        "repository": intent.repository,
        "issueNumber": intent.issue_number,
        "payloadSha256": intent.payload_sha256,
    })
    .to_string();
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'issue_comment_applied',?2,?3)",
        params![intent.ticket_id, detail, storage::now()],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn bounded_remote_value(value: Option<&serde_json::Value>, max_bytes: usize) -> Option<String> {
    let value = value?;
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty() && text.len() <= max_bytes && !text.chars().any(char::is_control))
        .then_some(text)
}

fn remote_comment_evidence(comment: &serde_json::Value) -> RemoteCommentEvidence {
    let id = bounded_remote_value(comment.get("databaseId"), 128)
        .or_else(|| bounded_remote_value(comment.get("id"), 128));
    let url = bounded_remote_value(comment.get("url"), 2048)
        .filter(|value| value.starts_with("https://"));
    let author = comment
        .get("author")
        .and_then(|author| author.get("login"))
        .and_then(|value| bounded_remote_value(Some(value), 128));
    RemoteCommentEvidence { id, url, author }
}

fn readback_state(value: &serde_json::Value, intent: &IssueCommentIntent) -> Result<Readback> {
    let comments = value
        .get("comments")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "GitHub did not return issue comments.".to_string())?;
    let marker = intent_marker(intent);
    let mut exact = Vec::new();
    let mut conflict = false;
    for comment in comments {
        let body = comment
            .get("body")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "GitHub returned an unreadable issue comment.".to_string())?;
        if body.contains(&marker) {
            if body == intent.comment_body.as_str() {
                exact.push(remote_comment_evidence(comment));
            } else {
                conflict = true;
            }
        }
    }
    Ok(match (exact.len(), conflict) {
        (0, false) => Readback::Missing,
        (1, false) => Readback::Exact(exact.pop().expect("one exact comment was observed")),
        _ => Readback::Conflict,
    })
}

fn reconcile_unresolved(
    store: &Store,
    intent: &IssueCommentIntent,
    root: &Path,
    run_gh: &mut impl FnMut(&Path, &[&str], Duration) -> std::result::Result<crate::workspace::Capture, String>,
) -> Result<EffectStatus> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    exact_grant_payload(&tx, intent)?;
    let receipt: Option<(String, String)> = tx
        .query_row(
            "SELECT state,payload_sha256 FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((state, digest)) = receipt else {
        return Err("No effect receipt exists to reconcile.".into());
    };
    if digest != intent.payload_sha256 {
        return Err("Effect receipt does not match this exact payload.".into());
    }
    if state == "applied" {
        tx.commit().map_err(|e| e.to_string())?;
        return status(store, intent, "already-applied");
    }
    if !matches!(state.as_str(), "pending" | "uncertain") {
        return Err("Invalid effect state.".into());
    }
    tx.commit().map_err(|e| e.to_string())?;

    let issue = intent.issue_number.to_string();
    let repo_flag = github::flag("repo", &intent.repository);
    let capture = run_gh(
        root,
        &["issue", "view", &issue, &repo_flag, "--json=comments"],
        github::TIMEOUT,
    );
    let observed = capture
        .and_then(|capture| github::json(&capture))
        .and_then(|value| readback_state(&value, intent));
    match observed {
        Ok(Readback::Exact(evidence)) => {
            if let Err(error) = settle_applied(store, intent, &evidence) {
                let _ = mark_uncertain(store, intent, "readback_settlement_failed", "exact");
                return Err(error);
            }
            status(store, intent, "exact-readback")
        }
        Ok(Readback::Missing) => {
            mark_uncertain(store, intent, "marker_not_found", "missing")?;
            status(store, intent, "marker-not-found")
        }
        Ok(Readback::Conflict) => {
            mark_uncertain(store, intent, "marker_conflict", "conflict")?;
            status(store, intent, "marker-conflict")
        }
        Err(_) => {
            mark_uncertain(store, intent, "readback_unavailable", "unavailable")?;
            status(store, intent, "readback-unavailable")
        }
    }
}

pub(crate) fn post_issue_comment(
    database_path: &Path,
    intent: &IssueCommentIntent,
) -> Result<EffectStatus> {
    let mut run_gh = |root: &Path, args: &[&str], timeout| github::run_gh(root, args, timeout);
    post_issue_comment_with(database_path, intent, &mut run_gh)
}

fn post_issue_comment_with(
    database_path: &Path,
    intent: &IssueCommentIntent,
    run_gh: &mut impl FnMut(&Path, &[&str], Duration) -> std::result::Result<crate::workspace::Capture, String>,
) -> Result<EffectStatus> {
    let store = open_effect_store(database_path)?;
    let project_id = {
        let tx = store
            .db
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        let project_id = exact_grant_payload(&tx, intent)?;
        tx.commit().map_err(|e| e.to_string())?;
        project_id
    };
    match reserve(&store, intent)? {
        Reservation::AlreadyApplied => status(&store, intent, "already-applied"),
        Reservation::AlreadyPending => status(&store, intent, "dispatch-in-progress"),
        Reservation::RequiresReconciliation => {
            let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
            let session_id = active_scope_if_current(&tx, intent)?
                .filter(|scope| scope.project_id == project_id)
                .map(|scope| scope.session_id);
            tx.commit().map_err(|e| e.to_string())?;
            let root = store.workspace_root(&project_id, session_id.as_deref())?;
            reconcile_unresolved(&store, intent, &root, run_gh)
        }
        Reservation::Reserved => {
            let issue = intent.issue_number.to_string();
            let repo_flag = github::flag("repo", &intent.repository);
            let body_flag = github::flag("body", &intent.comment_body);
            let dispatch = (|| -> Result<std::result::Result<crate::workspace::Capture, String>> {
                let _fence = lock_dispatch(database_path)?;
                let scope = validate_pending_dispatch(&store, intent)?;
                let root = store.workspace_root(&scope.project_id, Some(&scope.session_id))?;
                Ok(run_gh(
                    &root,
                    &["issue", "comment", &issue, &repo_flag, &body_flag],
                    github::TIMEOUT,
                ))
            })();
            match dispatch {
                Err(error) => {
                    let _ = mark_uncertain(&store, intent, "dispatch_not_started", "not_attempted");
                    Err(error)
                }
                Ok(_remote_result) => {
                    // The adapter may have timed out after GitHub accepted the
                    // request. Persist uncertainty before any readback and
                    // never replay the write, regardless of its exit status.
                    mark_uncertain(&store, intent, "write_result_unverified", "not_attempted")?;
                    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
                    let session_id = active_scope_if_current(&tx, intent)?
                        .filter(|scope| scope.project_id == project_id)
                        .map(|scope| scope.session_id);
                    tx.commit().map_err(|e| e.to_string())?;
                    let root = store.workspace_root(&project_id, session_id.as_deref())?;
                    reconcile_unresolved(&store, intent, &root, run_gh)
                }
            }
        }
    }
}

fn status(store: &Store, intent: &IssueCommentIntent, reconciliation: &str) -> Result<EffectStatus> {
    let evidence: Option<(String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = store.db
        .query_row(
            "SELECT state,uncertainty_reason,readback_state,remote_comment_id,
                    remote_comment_url,remote_author_login
             FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
            params![intent.ticket_id,intent.effect_key,intent.payload_sha256],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((state,uncertainty_reason,readback_state,remote_comment_id,remote_comment_url,remote_comment_author)) = evidence else {
        return Err("The exact external-effect receipt disappeared during reconciliation.".into());
    };
    Ok(EffectStatus {
        ticket_id: intent.ticket_id.clone(),
        ticket_revision: intent.expected_revision,
        repository: intent.repository.clone(),
        issue_number: intent.issue_number,
        effect_key: intent.effect_key.clone(),
        payload_sha256: intent.payload_sha256.clone(),
        state,
        reconciliation: reconciliation.into(),
        uncertainty_reason,
        readback_state,
        remote_comment_id,
        remote_comment_url,
        remote_comment_author,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectReceiptView {
    pub effect_key: String,
    pub payload_sha256: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) fn receipt_history(
    store: &Store, ticket_id: &str, limit: i64,
) -> Result<Vec<EffectReceiptView>> {
    if !(1..=100).contains(&limit) {
        return Err("Effect receipt page size must be between 1 and 100.".into());
    }
    let exists: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets WHERE id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !exists {
        return Err("SuperAgent Ticket not found.".into());
    }
    let mut stmt = store.db.prepare(
        "SELECT effect_key,payload_sha256,state,created_at,updated_at
         FROM sa_effect_receipts WHERE ticket_id=?1
         ORDER BY created_at DESC,effect_key DESC LIMIT ?2",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id, limit], |row| Ok(EffectReceiptView {
        effect_key: row.get(0)?,
        payload_sha256: row.get(1)?,
        state: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

pub(crate) fn unresolved_count(store: &Store, ticket_id: &str) -> Result<i64> {
    store
        .db
        .query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE ticket_id=?1 AND state!='applied'",
            [ticket_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superagent::{self, NewOrchestration};
    use std::{
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc,
            Arc,
        },
        thread,
    };

    fn fixture() -> (Store, String) {
        let path = std::env::temp_dir().join(format!("sa-effect-{}.db", storage::id()));
        let mut store = Store::open(&path).unwrap();
        store
            .db
            .execute(
                "INSERT INTO projects(id,name,path) VALUES('p','P',?1)",
                [std::env::temp_dir().to_string_lossy().into_owned()],
            )
            .unwrap();
        for (id, name) in [("co", "Coordinator"), ("wo", "Worker")] {
            store
                .db
                .execute(
                    "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                     VALUES(?1,?2,'p','codex','default','plan',?3,?3)",
                    params![id, name, storage::now()],
                )
                .unwrap();
        }
        let cfg = superagent::create_configuration(
            &store,
            NewOrchestration {
                name: "Effect Test".into(),
                coordinator_bot_id: "co".into(),
                worker_bot_id: "wo".into(),
            },
        )
        .unwrap();
        store
            .db
            .execute(
                "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
                [&cfg.id],
            )
            .unwrap();
        let (ticket, started) =
            superagent::admit(&mut store, &cfg.id, "Safe work", "receipt-fixture", false).unwrap();
        assert!(started);
        (store, ticket.id)
    }

    fn make_intent(ticket_id: &str, comment: &str) -> IssueCommentIntent {
        prepare_issue_comment(
            ticket_id,
            0,
            "owner/repo",
            7,
            "github:comment:7",
            comment,
        )
        .unwrap()
    }

    fn capture_json(value: serde_json::Value) -> crate::workspace::Capture {
        crate::workspace::Capture {
            bytes: value.to_string().into_bytes(),
            truncated: false,
        }
    }

    fn is_issue_comment_write(args: &[&str]) -> bool {
        args.get(0) == Some(&"issue") && args.get(1) == Some(&"comment")
    }

    fn exact_readback(intent: &IssueCommentIntent) -> crate::workspace::Capture {
        capture_json(serde_json::json!({
            "comments": [{
                "body": intent.comment_body,
                "databaseId": 901,
                "url": "https://github.com/owner/repo/issues/7#issuecomment-901",
                "author": { "login": "panda-reviewer" }
            }]
        }))
    }

    #[test]
    fn exact_grant_is_required_and_receipts_prevent_replay_and_mismatch() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "payload");
        assert!(reserve(&store, &intent).is_err());
        assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        assert!(!has_exact_grant(&store, &intent).unwrap());
        authorize(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::Reserved);
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::AlreadyPending);
        let other = make_intent(&id, "other");
        assert!(reserve(&store, &other).is_err());
        mark_uncertain(&store, &intent, "write_result_unverified", "not_attempted").unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::RequiresReconciliation);
        settle_applied(&store, &intent, &RemoteCommentEvidence { id: None, url: None, author: None }).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::AlreadyApplied);
        assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        store
            .db
            .execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&id])
            .unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::AlreadyApplied);
    }

    #[test]
    fn changed_worker_session_identity_denies_native_effect_reservation() {
        for mutation in ["changed_model", "detached_thread"] {
            let (store, id) = fixture();
            let intent = make_intent(&id, "worker-session-identity");
            authorize(&store, &intent).unwrap();
            let session_id: String = store.db.query_row(
                "SELECT a.session_id FROM sa_tickets t
                 JOIN sa_assignments a ON a.id=t.active_assignment_id
                 WHERE t.id=?1", [&id], |row| row.get(0),
            ).unwrap();
            match mutation {
                "changed_model" => {
                    store.db.execute(
                        "UPDATE sessions SET model='unadmitted-model' WHERE id=?1",
                        [&session_id],
                    ).unwrap();
                }
                "detached_thread" => {
                    let changed=store.db.execute(
                        "UPDATE bot_threads SET external_id='detached'
                         WHERE session_id=?1 AND channel_id=?2",
                        params![&session_id, crate::superagent::CHANNEL],
                    ).unwrap();
                    assert_eq!(changed, 1);
                }
                _ => unreachable!(),
            }
            assert!(reserve(&store, &intent).is_err(),
                "{mutation} must deny effect reservation");
            assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        }
    }

    #[test]
    fn concurrent_mandate_revocation_precedes_grant_authorization() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "race payload");
        let path = store.path.clone();
        let revocation_fence = lock_dispatch(&path).unwrap();
        let (authorize_waiting_tx, authorize_waiting_rx) = mpsc::sync_channel(1);
        let authorize_intent = intent.clone();
        let authorizer = thread::spawn(move || {
            let store = open_effect_store(&path).unwrap();
            authorize_with_lock_observer(&store, &authorize_intent, || {
                authorize_waiting_tx.send(()).unwrap();
            })
        });

        authorize_waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("grant authorization did not wait behind the revocation fence");
        store
            .db
            .execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&id])
            .unwrap();
        drop(revocation_fence);

        let error = authorizer.join().unwrap().unwrap_err();
        assert_eq!(
            error,
            "The Ticket, revision, mandate, or active worker no longer permits this effect."
        );
        assert!(!has_exact_grant(&store, &intent).unwrap());
    }

    #[test]
    fn production_stop_fences_waiting_grant_authorization() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "stop race payload");
        let stop_path = store.path.clone();
        let stop_ticket_id = id.clone();
        let (stop_fenced_tx, stop_fenced_rx) = mpsc::sync_channel(1);
        let (continue_stop_tx, continue_stop_rx) = mpsc::sync_channel(1);
        let stopper = thread::spawn(move || {
            let stop_store = open_effect_store(&stop_path).unwrap();
            observe_next_dispatch_fence_acquired(move || {
                stop_fenced_tx.send(()).unwrap();
                continue_stop_rx.recv().unwrap();
            });
            superagent::stop(&stop_store, &stop_ticket_id).map(|(ticket, _)| ticket.state)
        });

        stop_fenced_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("production Stop did not acquire the dispatch fence");

        let authorize_path = store.path.clone();
        let authorize_intent = intent.clone();
        let (authorize_waiting_tx, authorize_waiting_rx) = mpsc::sync_channel(1);
        let authorizer = thread::spawn(move || {
            let authorize_store = open_effect_store(&authorize_path).unwrap();
            observe_next_dispatch_fence_waiting(move || {
                authorize_waiting_tx.send(()).unwrap();
            });
            authorize(&authorize_store, &authorize_intent)
        });

        authorize_waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("production authorization did not wait behind Stop's dispatch fence");

        continue_stop_tx.send(()).unwrap();
        assert_eq!(stopper.join().unwrap().unwrap(), "stopped");
        assert_eq!(
            authorizer.join().unwrap().unwrap_err(),
            "The Ticket, revision, mandate, or active worker no longer permits this effect."
        );
        assert_eq!(
            superagent::get_ticket(&store, &id).unwrap().state,
            "stopped"
        );
        assert!(superagent::mandate(&store, &id).unwrap().revoked);
        let exact_grant_count: i64 = store
            .db
            .query_row(
                "SELECT COUNT(*) FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
                params![&id, &intent.effect_key],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exact_grant_count, 0, "Stop must leave no exact effect grant");
    }

    #[test]
    fn exact_grant_denies_stale_or_mismatched_mandate_scope() {
        for (change, expected) in [
            ("UPDATE sa_mandates SET actor='other' WHERE ticket_id=?1", "actor"),
            ("UPDATE sa_mandates SET project_id='other' WHERE ticket_id=?1", "project"),
            ("UPDATE sa_mandates SET policy_revision=policy_revision+1 WHERE ticket_id=?1", "revision"),
            (
                "UPDATE sa_orchestrations SET policy_revision=policy_revision+1 WHERE id=(SELECT orchestration_id FROM sa_tickets WHERE id=?1)",
                "policy",
            ),
        ] {
            let (store, id) = fixture();
            let intent = make_intent(&id, "body");
            authorize(&store, &intent).unwrap();
            store.db.execute(change, [&id]).unwrap();
            assert!(reserve(&store, &intent).is_err(), "stale {expected} must deny effect reservation");
            assert_eq!(unresolved_count(&store, &id).unwrap(), 0, "denial must not persist a receipt");
        }

        let (store, id) = fixture();
        let intent = make_intent(&id, "body");
        authorize(&store, &intent).unwrap();
        store
            .db
            .execute("UPDATE sa_assignments SET state='finished' WHERE ticket_id=?1", [&id])
            .unwrap();
        assert!(reserve(&store, &intent).is_err());
        assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
    }

    #[test]
    fn exact_grant_is_bound_to_its_assignment_session_and_project() {
        for (field, value) in [
            ("assignmentId", "replacement-assignment"),
            ("sessionId", "replacement-session"),
            ("projectId", "replacement-project"),
        ] {
            let (store, id) = fixture();
            let intent = make_intent(&id, "bound scope");
            authorize(&store, &intent).unwrap();
            let path = format!("$.{field}");
            store
                .db
                .execute(
                    "UPDATE sa_audit SET detail=json_set(detail,?1,?2)
                     WHERE ticket_id=?3 AND kind='issue_comment_granted'",
                    params![path, value, id],
                )
                .unwrap();
            assert!(reserve(&store, &intent).is_err(), "changed {field} must deny dispatch");
            assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        }
    }

    #[test]
    fn restart_dispatcher_reconciles_exact_grant_without_a_new_write() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "body");
        authorize(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::Reserved);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let state: String = reopened
            .db
            .query_row(
                "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key='github:comment:7'",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "uncertain");
        assert_eq!(unresolved_count(&reopened, &id).unwrap(), 1);
        assert!(has_exact_grant(&reopened, &intent).unwrap());
        assert_eq!(reserve(&reopened, &intent).unwrap(), Reservation::RequiresReconciliation);

        let writes = Arc::new(AtomicUsize::new(0));
        let counted_writes = Arc::clone(&writes);
        let readback_intent = intent.clone();
        let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
            if is_issue_comment_write(args) {
                counted_writes.fetch_add(1, Ordering::SeqCst);
                Ok(crate::workspace::Capture { bytes: Vec::new(), truncated: false })
            } else {
                Ok(exact_readback(&readback_intent))
            }
        };
        let status = post_issue_comment_with(&path, &intent, &mut fake_gh).unwrap();
        assert_eq!(status.state, "applied");
        assert_eq!(status.reconciliation, "exact-readback");
        assert_eq!(status.remote_comment_id.as_deref(), Some("901"));
        assert_eq!(status.remote_comment_url.as_deref(), Some("https://github.com/owner/repo/issues/7#issuecomment-901"));
        assert_eq!(status.remote_comment_author.as_deref(), Some("panda-reviewer"));
        assert_eq!(status.uncertainty_reason, None);
        assert_eq!(status.readback_state.as_deref(), Some("exact"));
        assert_eq!(writes.load(Ordering::SeqCst), 0, "restart may only read back the exact grant");
        assert_eq!(unresolved_count(&reopened, &id).unwrap(), 0);
    }

    #[test]
    fn headless_restart_reconciliation_reuses_existing_grant_without_reauthorizing() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "headless body");
        authorize(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::Reserved);
        let path = store.path.clone();
        drop(store);

        let reopened = Store::open(&path).unwrap();
        assert_eq!(unresolved_count(&reopened, &id).unwrap(), 1);
        drop(reopened);

        let mut dispatched = false;
        let output = superagent::headless_issue_comment_with_dispatch(
            &path,
            &id,
            0,
            "owner/repo",
            7,
            "github:comment:7",
            "headless body",
            None,
            |_, dispatched_intent| {
                dispatched = true;
                assert_eq!(dispatched_intent, &intent);
                Ok(EffectStatus {
                    ticket_id: id.clone(),
                    ticket_revision: 0,
                    repository: "owner/repo".into(),
                    issue_number: 7,
                    effect_key: "github:comment:7".into(),
                    payload_sha256: intent.payload_sha256.clone(),
                    state: "uncertain".into(),
                    reconciliation: "readback-unavailable".into(),
                    uncertainty_reason: Some("readback_unavailable".into()),
                    readback_state: Some("unavailable".into()),
                    remote_comment_id: None,
                    remote_comment_url: None,
                    remote_comment_author: None,
                })
            },
        )
        .unwrap();
        assert!(dispatched, "an exact pending grant must reach the read-only dispatcher after restart");
        let output: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(output["state"], "uncertain");
        let after = open_effect_store(&path).unwrap();
        assert_eq!(unresolved_count(&after, &id).unwrap(), 1);
    }

    #[test]
    fn terminal_ticket_cannot_start_a_new_write() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "terminal body");
        authorize(&store, &intent).unwrap();
        store
            .db
            .execute(
                "UPDATE sa_tickets SET state='completed',active_assignment_id=NULL,revision=revision+1
                 WHERE id=?1",
                [&id],
            )
            .unwrap();
        store
            .db
            .execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&id])
            .unwrap();
        let path = store.path.clone();
        drop(store);

        let writes = Arc::new(AtomicUsize::new(0));
        let counted_writes = Arc::clone(&writes);
        let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
            if is_issue_comment_write(args) {
                counted_writes.fetch_add(1, Ordering::SeqCst);
            }
            Ok(capture_json(serde_json::json!({ "comments": [] })))
        };
        assert!(post_issue_comment_with(&path, &intent, &mut fake_gh).is_err());
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stop_first_forbids_a_write_and_only_reads_an_existing_receipt() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "stop-first body");
        authorize(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::Reserved);
        assert_eq!(superagent::stop(&store, &id).unwrap().0.state, "stopped");
        let path = store.path.clone();
        drop(store);

        let writes = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let counted_writes = Arc::clone(&writes);
        let counted_reads = Arc::clone(&reads);
        let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
            if is_issue_comment_write(args) {
                counted_writes.fetch_add(1, Ordering::SeqCst);
            } else {
                counted_reads.fetch_add(1, Ordering::SeqCst);
            }
            Ok(capture_json(serde_json::json!({ "comments": [] })))
        };
        let status = post_issue_comment_with(&path, &intent, &mut fake_gh).unwrap();
        assert_eq!(status.state, "uncertain");
        assert_eq!(status.readback_state.as_deref(), Some("missing"));
        assert_eq!(status.uncertainty_reason.as_deref(), Some("marker_not_found"));
        assert_eq!(writes.load(Ordering::SeqCst), 0, "Stop-first must forbid a later external write");
        assert_eq!(reads.load(Ordering::SeqCst), 1, "only the existing exact receipt may be read back");
    }

    #[test]
    fn bot_reset_first_revokes_before_a_later_readback_without_a_write() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "reset-first body");
        authorize(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::Reserved);
        crate::bots::reset(&store, "wo").unwrap();
        assert_eq!(superagent::get_ticket(&store, &id).unwrap().state, "interrupted");
        let path = store.path.clone();
        drop(store);

        let writes = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let counted_writes = Arc::clone(&writes);
        let counted_reads = Arc::clone(&reads);
        let readback_intent = intent.clone();
        let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
            if is_issue_comment_write(args) {
                counted_writes.fetch_add(1, Ordering::SeqCst);
                Ok(crate::workspace::Capture { bytes: Vec::new(), truncated: false })
            } else {
                counted_reads.fetch_add(1, Ordering::SeqCst);
                Ok(exact_readback(&readback_intent))
            }
        };
        let status = post_issue_comment_with(&path, &intent, &mut fake_gh).unwrap();
        assert_eq!(status.state, "applied");
        assert_eq!(status.readback_state.as_deref(), Some("exact"));
        assert_eq!(writes.load(Ordering::SeqCst), 0, "bot reset must prevent a later write");
        assert_eq!(reads.load(Ordering::SeqCst), 1, "revocation permits readback of the existing receipt");
    }

    #[test]
    fn stop_waits_for_the_claimed_write_and_uncertain_receipt_is_never_replayed() {
        let (store, id) = fixture();
        let intent = make_intent(&id, "race payload");
        authorize(&store, &intent).unwrap();
        let path = store.path.clone();
        drop(store);

        let (write_started_tx, write_started_rx) = mpsc::sync_channel(0);
        let (continue_write_tx, continue_write_rx) = mpsc::sync_channel(0);
        let writes = Arc::new(AtomicUsize::new(0));
        let first_write_count = Arc::clone(&writes);
        let dispatch_path = path.clone();
        let dispatch_intent = intent.clone();
        let dispatcher = thread::spawn(move || {
            let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
                if is_issue_comment_write(args) {
                    first_write_count.fetch_add(1, Ordering::SeqCst);
                    write_started_tx.send(()).unwrap();
                    continue_write_rx.recv().unwrap();
                    Err("mock timeout after launch".to_string())
                } else {
                    Ok(capture_json(serde_json::json!({ "comments": [] })))
                }
            };
            post_issue_comment_with(&dispatch_path, &dispatch_intent, &mut fake_gh)
        });
        write_started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let (stop_waiting_tx, stop_waiting_rx) = mpsc::sync_channel(0);
        let (stop_done_tx, stop_done_rx) = mpsc::sync_channel(1);
        let stop_path = path.clone();
        let stop_ticket_id = id.clone();
        let stopper = thread::spawn(move || {
            let stopped = (|| -> std::result::Result<String, String> {
                let store = superagent::open_headless_transition_store(&stop_path)?;
                let (ticket, _) = superagent::stop_with_lock_observer(
                    &store,
                    &stop_ticket_id,
                    || { let _ = stop_waiting_tx.send(()); },
                )?;
                Ok(ticket.state)
            })();
            stop_done_tx.send(stopped).unwrap();
        });
        stop_waiting_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(stop_done_rx.try_recv().is_err(), "Stop must wait while the write launch fence is held");

        continue_write_tx.send(()).unwrap();
        assert_eq!(stop_done_rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), "stopped");
        stopper.join().unwrap();
        let first_status = dispatcher.join().unwrap().unwrap();
        assert_eq!(first_status.state, "uncertain");
        assert_eq!(writes.load(Ordering::SeqCst), 1);

        let second_write_count = Arc::clone(&writes);
        let mut fake_gh = move |_: &Path, args: &[&str], _: Duration| {
            if is_issue_comment_write(args) {
                second_write_count.fetch_add(1, Ordering::SeqCst);
            }
            Ok(capture_json(serde_json::json!({ "comments": [] })))
        };
        let after_stop = post_issue_comment_with(&path, &intent, &mut fake_gh).unwrap();
        assert_eq!(after_stop.state, "uncertain");
        assert_eq!(writes.load(Ordering::SeqCst), 1, "a stopped Ticket must never replay the write");
    }

    #[test]
    fn dispatch_fence_cross_process_child_holds_lock() {
        let Ok(directory) = std::env::var("PANDACODE_EFFECT_LOCK_CHILD_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        let database_path = directory.join("store.sqlite3");
        let _fence = lock_dispatch(&database_path).unwrap();
        std::fs::write(directory.join("ready"), b"ready").unwrap();
        let release = directory.join("release");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !release.exists() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(release.exists(), "parent did not release the cross-process fence test");
    }

    #[test]
    fn dispatch_fence_excludes_another_process() {
        let directory = std::env::temp_dir().join(format!("sa-effect-lock-{}", storage::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("dispatch_fence_cross_process_child_holds_lock")
            .env("PANDACODE_EFFECT_LOCK_CHILD_DIR", &directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let ready_path = directory.join("ready");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready_path.exists()
            && std::time::Instant::now() < deadline
            && child.try_wait().ok().flatten().is_none()
        {
            thread::sleep(Duration::from_millis(10));
        }
        let ready = ready_path.exists();
        let lock_path = directory
            .join("store.sqlite3")
            .with_extension("superagent-effect-dispatch.lock");
        let excluded = ready
            && OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(lock_path)
                .ok()
                .is_some_and(|file| matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
        let _ = std::fs::write(directory.join("release"), b"release");
        let child_status = child.wait().unwrap();
        let _ = std::fs::remove_dir_all(&directory);
        assert!(ready, "child process did not acquire the dispatch fence");
        assert!(excluded, "a second process acquired an already-held dispatch fence");
        assert!(child_status.success(), "child process failed while holding the dispatch fence");
    }

    #[test]
    fn readback_accepts_only_one_exact_marker_and_body() {
        let intent = make_intent("ticket_1", "comment text");
        let exact = serde_json::json!({"comments":[{"body":intent.comment_body}]});
        assert!(matches!(readback_state(&exact, &intent).unwrap(), Readback::Exact(_)));
        let absent = serde_json::json!({"comments":[{"body":"other comment"}]});
        assert_eq!(readback_state(&absent, &intent).unwrap(), Readback::Missing);
        let conflict = serde_json::json!({"comments":[{"body":format!("{} changed",intent.comment_body)}]});
        assert_eq!(readback_state(&conflict, &intent).unwrap(), Readback::Conflict);
        let duplicate = serde_json::json!({"comments":[{"body":intent.comment_body},{"body":intent.comment_body}]});
        assert_eq!(readback_state(&duplicate, &intent).unwrap(), Readback::Conflict);
    }

    #[test]
    fn confirmation_digest_binds_revision_repository_issue_key_and_body() {
        let base = prepare_issue_comment("ticket_1", 0, "owner/repo", 7, "stable-key", "body").unwrap();
        for variant in [
            prepare_issue_comment("ticket_1", 1, "owner/repo", 7, "stable-key", "body").unwrap(),
            prepare_issue_comment("ticket_1", 0, "other/repo", 7, "stable-key", "body").unwrap(),
            prepare_issue_comment("ticket_1", 0, "owner/repo", 8, "stable-key", "body").unwrap(),
            prepare_issue_comment("ticket_1", 0, "owner/repo", 7, "other-key", "body").unwrap(),
            prepare_issue_comment("ticket_1", 0, "owner/repo", 7, "stable-key", "changed").unwrap(),
        ] {
            assert_ne!(base.payload_sha256, variant.payload_sha256);
        }
    }
}
