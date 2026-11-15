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
const DRAFT_PR_MARKER: &str = "<!-- pandacode-superagent-draft-pr:";
const MAX_COMMENT_BYTES: usize = 60_000;
// The body is passed to `gh` as an argv value. Keep it comfortably below the
// platform command-line limit after quoting and the other command arguments.
const MAX_DRAFT_PR_BODY_BYTES: usize = 8 * 1024;

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

/// PR authority and receipts have their own tables. Issue-comment payloads and
/// reconciliation fields remain unchanged, and no Git operation is recorded
/// as part of a PR grant.
pub(crate) const DRAFT_PR_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_draft_pr_effect_grants (
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 effect_key TEXT NOT NULL,
 ticket_revision INTEGER NOT NULL,
 project_id TEXT NOT NULL,
 assignment_id TEXT NOT NULL,
 session_id TEXT NOT NULL,
 repository TEXT NOT NULL,
 head_branch TEXT NOT NULL,
 base_branch TEXT NOT NULL,
 title TEXT NOT NULL,
 body TEXT NOT NULL,
 marker TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL CHECK(length(payload_sha256)=64),
 created_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key)
);
CREATE TABLE IF NOT EXISTS sa_draft_pr_effect_receipts (
 ticket_id TEXT NOT NULL,
 effect_key TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL CHECK(length(payload_sha256)=64),
 state TEXT NOT NULL CHECK(state IN ('pending','uncertain','applied')),
 uncertainty_reason TEXT CHECK(uncertainty_reason IS NULL OR length(uncertainty_reason)<=64),
 readback_state TEXT CHECK(readback_state IS NULL OR readback_state IN ('not_attempted','exact','missing','conflict','unavailable')),
 remote_pr_number INTEGER CHECK(remote_pr_number IS NULL OR remote_pr_number>0),
 remote_pr_url TEXT CHECK(remote_pr_url IS NULL OR length(remote_pr_url)<=2048),
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key),
 FOREIGN KEY(ticket_id,effect_key)
   REFERENCES sa_draft_pr_effect_grants(ticket_id,effect_key) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS sa_draft_pr_effect_unresolved
 ON sa_draft_pr_effect_receipts(ticket_id,state) WHERE state!='applied';
"#;

pub(crate) const RECOVER: &str =
    "UPDATE sa_effect_receipts SET state='uncertain',uncertainty_reason='process_restarted_before_settlement',readback_state='not_attempted',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE state='pending';
     UPDATE sa_draft_pr_effect_receipts SET state='uncertain',uncertainty_reason='process_restarted_before_settlement',readback_state='not_attempted',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE state='pending';";

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
        // The fence is advisory-lock metadata, never content: keep any
        // existing file instead of truncating another process's lock inode.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftPullRequestIntent {
    pub ticket_id: String,
    pub expected_revision: i64,
    pub repository: String,
    pub head: String,
    pub head_sha: String,
    pub base: String,
    pub title: String,
    /// Complete GitHub body, including the PandaCode marker.
    pub body: String,
    pub marker: String,
    pub effect_key: String,
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

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DraftPullRequestEffectStatus {
    pub ticket_id: String,
    pub ticket_revision: i64,
    pub repository: String,
    pub head: String,
    pub head_sha: String,
    pub base: String,
    pub title: String,
    pub effect_key: String,
    pub payload_sha256: String,
    pub state: String,
    pub reconciliation: String,
    pub uncertainty_reason: Option<String>,
    pub readback_state: Option<String>,
    pub remote_pr_number: Option<i64>,
    pub remote_pr_url: Option<String>,
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

fn draft_pr_key(
    ticket_id: &str,
    repository: &str,
    head: &str,
    base: &str,
    title: &str,
    body: &str,
) -> Result<String> {
    // Keep this as the same ordered JSON tuple used by superagent-ui.ts.
    let bytes = serde_json::to_vec(&[ticket_id, repository, head, base, title, body])
        .map_err(|error| error.to_string())?;
    Ok(format!("github-draft-pr:{:x}", Sha256::digest(bytes)))
}

fn valid_branch_ref(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 200
        && !branch.starts_with('-')
        && !branch.ends_with('.')
        && !branch.ends_with('/')
        && !branch.contains("..")
        && !branch.contains("//")
        && !branch.contains("@{")
        && branch.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/')
        })
}

pub(crate) fn prepare_draft_pull_request(
    ticket_id: &str,
    expected_revision: i64,
    repository: &str,
    head: &str,
    head_sha: &str,
    base: &str,
    title: &str,
    body: &str,
    effect_key: &str,
) -> Result<DraftPullRequestIntent> {
    if ticket_id.is_empty()
        || ticket_id.len() > 128
        || !ticket_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || expected_revision < 0
    {
        return Err("Provide a valid Ticket ID and nonnegative Ticket revision.".into());
    }
    if repository.len() > 200 {
        return Err("Provide one exact GitHub repository in owner/name form.".into());
    }
    validate_repository(repository)?;
    validate_effect_key(effect_key)?;
    if !valid_branch_ref(head) || !valid_branch_ref(base) || head == base {
        return Err("Choose distinct valid head and base branches.".into());
    }
    if !matches!(head_sha.len(), 40 | 64)
        || !head_sha.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("Provide the exact verified GitHub head SHA.".into());
    }
    if title.trim().is_empty()
        || title.trim() != title
        || title.chars().count() > 256
        || title.chars().any(char::is_control)
    {
        return Err("Provide a plain-text pull request title of 1 to 256 characters.".into());
    }
    if body.len() > MAX_DRAFT_PR_BODY_BYTES
        || body.contains('\0')
        || body.contains(DRAFT_PR_MARKER)
    {
        return Err("Provide a pull request body within the size limit and without a PandaCode effect marker.".into());
    }
    let expected_key = draft_pr_key(ticket_id, repository, head, base, title, body)?;
    if effect_key != expected_key {
        return Err("The draft pull request effect key does not match this exact Ticket and payload.".into());
    }
    let marker = format!("{DRAFT_PR_MARKER}ticket={ticket_id};key={effect_key} -->");
    let full_body = if body.is_empty() {
        marker.clone()
    } else {
        format!("{body}\n\n{marker}")
    };
    if full_body.len() > MAX_DRAFT_PR_BODY_BYTES {
        return Err("Shorten the pull request body to leave room for PandaCode's effect marker.".into());
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload<'a> {
        ticket_id: &'a str,
        expected_revision: i64,
        repository: &'a str,
        head: &'a str,
        head_sha: &'a str,
        base: &'a str,
        title: &'a str,
        body: &'a str,
        marker: &'a str,
        effect_key: &'a str,
    }
    let bytes = serde_json::to_vec(&Payload {
        ticket_id,
        expected_revision,
        repository,
        head,
        head_sha,
        base,
        title,
        body: &full_body,
        marker: &marker,
        effect_key,
    })
    .map_err(|error| error.to_string())?;
    Ok(DraftPullRequestIntent {
        ticket_id: ticket_id.into(),
        expected_revision,
        repository: repository.into(),
        head: head.into(),
        head_sha: head_sha.into(),
        base: base.into(),
        title: title.into(),
        body: full_body,
        marker,
        effect_key: effect_key.into(),
        payload_sha256: format!("{:x}", Sha256::digest(bytes)),
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
           AND m.expires_at>?3
           AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision AND t.project_id=m.project_id
           AND m.project_id=o.project_id AND t.actor=m.actor AND m.authorization_id<>''
           AND b.project_id=t.project_id AND b.paused=0 AND o.enabled=1
           AND EXISTS (
             SELECT 1 FROM sa_audit choice
             WHERE choice.ticket_id=t.id AND choice.kind='side_effect_consent_enabled'
               AND json_extract(choice.detail,'$.authorizationId')=m.authorization_id
               AND json_extract(choice.detail,'$.policyRevision')=m.policy_revision
               AND json_extract(choice.detail,'$.projectId')=m.project_id
               AND json_extract(choice.detail,'$.assignmentId')=a.id
               AND json_extract(choice.detail,'$.sessionId')=a.session_id
               AND NOT EXISTS (
                 SELECT 1 FROM sa_audit newer
                 WHERE newer.ticket_id=choice.ticket_id
                   AND newer.kind IN ('side_effect_consent_enabled','side_effect_consent_revoked')
                   AND newer.id>choice.id
               )
           )",
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

const SUPPORTED_GITHUB_SIDE_EFFECTS: [&str; 2] = [
    "issue_and_pull_request_conversation_comments",
    "draft_pull_request_creation",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SideEffectConsentScope {
    pub objective: String,
    pub ticket_revision: i64,
    pub policy_revision: i64,
    pub project_id: String,
    pub authorization_id: String,
    pub assignment_id: String,
    pub session_id: String,
    pub expires_at: i64,
    pub worker_paused: bool,
    pub consent_active: bool,
}

fn current_side_effect_consent_scope(
    db: &Connection,
    ticket_id: &str,
    expected_revision: i64,
    now: i64,
) -> Result<Option<SideEffectConsentScope>> {
    db.query_row(
        "SELECT t.objective,t.revision,t.policy_revision,t.project_id,a.id,a.session_id,
                m.expires_at,m.authorization_id,b.paused,
                COALESCE((
                  SELECT CASE WHEN choice.kind='side_effect_consent_enabled'
                    AND json_extract(choice.detail,'$.authorizationId')=m.authorization_id
                    AND json_extract(choice.detail,'$.policyRevision')=m.policy_revision
                    AND json_extract(choice.detail,'$.projectId')=m.project_id
                    AND json_extract(choice.detail,'$.assignmentId')=a.id
                    AND json_extract(choice.detail,'$.sessionId')=a.session_id
                    THEN 1 ELSE 0 END
                  FROM sa_audit choice
                  WHERE choice.ticket_id=t.id
                    AND choice.kind IN ('side_effect_consent_enabled','side_effect_consent_revoked')
                  ORDER BY choice.id DESC LIMIT 1
                ),0)
         FROM sa_tickets t
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         JOIN sa_assignments a ON a.id=t.active_assignment_id
           AND a.ticket_id=t.id AND a.state='running' AND a.bot_id=t.worker_bot_id
         JOIN bots b ON b.id=a.bot_id
         WHERE t.id=?1 AND t.revision=?2 AND t.state='running' AND m.revoked=0
           AND m.expires_at>?3 AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision AND t.project_id=m.project_id
           AND m.project_id=o.project_id AND t.actor=m.actor AND m.authorization_id<>''
           AND b.project_id=t.project_id AND o.enabled=1",
        params![ticket_id, expected_revision, now],
        |row| {
            Ok(SideEffectConsentScope {
                objective: row.get(0)?,
                ticket_revision: row.get(1)?,
                policy_revision: row.get(2)?,
                project_id: row.get(3)?,
                assignment_id: row.get(4)?,
                session_id: row.get(5)?,
                expires_at: row.get(6)?,
                authorization_id: row.get(7)?,
                worker_paused: row.get::<_, i64>(8)? != 0,
                consent_active: row.get::<_, i64>(9)? != 0,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

/// Whether the append-only operator consent is current for this live assignment.
/// Consent survives normal Ticket revision changes but never follows a new
/// mandate, policy, project, or assignment.
pub(crate) fn has_active_side_effect_consent(store: &Store, ticket_id: &str) -> Result<bool> {
    let revision: Option<i64> = store.db.query_row(
        "SELECT revision FROM sa_tickets WHERE id=?1",
        [ticket_id],
        |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    let Some(revision) = revision else { return Ok(false) };
    Ok(current_side_effect_consent_scope(&store.db, ticket_id, revision, storage::now())?
        .is_some_and(|scope| scope.consent_active))
}

/// Read the exact live Ticket authority before opening the native confirmation.
/// The state is reloaded under the shared dispatch fence after the dialog closes.
pub(crate) fn side_effect_consent_for_change(
    store: &Store,
    ticket_id: &str,
    expected_revision: i64,
    enabled: bool,
) -> Result<SideEffectConsentScope> {
    side_effect_consent_for_change_with_pause(
        store,
        ticket_id,
        expected_revision,
        enabled,
        || crate::feature_controls::get().bots_paused,
    )
}

fn side_effect_consent_for_change_with_pause(
    store: &Store,
    ticket_id: &str,
    expected_revision: i64,
    enabled: bool,
    globally_paused: impl Fn() -> bool,
) -> Result<SideEffectConsentScope> {
    if enabled && globally_paused() {
        return Err("Bots are globally paused; GitHub side-effect authorization cannot be enabled.".into());
    }
    let scope = current_side_effect_consent_scope(
        &store.db, ticket_id, expected_revision, storage::now(),
    )?.ok_or_else(|| "The Ticket, revision, mandate, active assignment, or current policy no longer permits this change.".to_string())?;
    if enabled && scope.worker_paused {
        return Err("The active Ticket worker is paused; GitHub side-effect consent cannot be enabled.".into());
    }
    Ok(scope)
}

/// Operator-only transition for this Ticket's bounded GitHub effects. The
/// immutable admission mandate remains default-deny; operator consent is an
/// append-only, assignment-bound audit choice. Revocation stays available
/// during global pause, while enabling is denied.
pub(crate) fn set_side_effect_consent(
    store: &Store,
    ticket_id: &str,
    expected_revision: i64,
    enabled: bool,
) -> Result<()> {
    set_side_effect_consent_with_pause(
        store,
        ticket_id,
        expected_revision,
        enabled,
        || crate::feature_controls::get().bots_paused,
    )
}

fn set_side_effect_consent_with_pause(
    store: &Store,
    ticket_id: &str,
    expected_revision: i64,
    enabled: bool,
    globally_paused: impl Fn() -> bool,
) -> Result<()> {
    let _dispatch_fence = lock_dispatch(&store.path)?;
    if enabled && globally_paused() {
        return Err("Bots are globally paused; GitHub side-effect authorization cannot be enabled.".into());
    }
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let scope = current_side_effect_consent_scope(
        &tx,
        ticket_id,
        expected_revision,
        storage::now(),
    )?
    .ok_or_else(|| "The Ticket, revision, mandate, active assignment, or current policy no longer permits this change.".to_string())?;
    if scope.consent_active == enabled {
        return Err(if enabled {
            "This Ticket already has bounded GitHub side effects enabled.".into()
        } else {
            "This Ticket already has bounded GitHub side effects revoked.".into()
        });
    }
    if enabled && scope.worker_paused {
        return Err("The active Ticket worker is paused; GitHub side-effect consent cannot be enabled.".into());
    }
    // Recheck immediately before the durable grant so a concurrent global pause
    // that began after the dialog cannot enable new authority.
    if enabled && globally_paused() {
        return Err("Bots are globally paused; GitHub side-effect authorization cannot be enabled.".into());
    }
    let detail = serde_json::json!({
        "actor": "local-user",
        "choice": if enabled { "enabled" } else { "revoked" },
        "sideEffectConsent": enabled,
        "ticketRevision": scope.ticket_revision,
        "policyRevision": scope.policy_revision,
        "projectId": scope.project_id,
        "authorizationId": scope.authorization_id,
        "assignmentId": scope.assignment_id,
        "sessionId": scope.session_id,
        "expiresAt": scope.expires_at,
        "supportedEffects": SUPPORTED_GITHUB_SIDE_EFFECTS,
    }).to_string();
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,?2,?3,?4)",
        params![
            ticket_id,
            if enabled { "side_effect_consent_enabled" } else { "side_effect_consent_revoked" },
            detail,
            storage::now(),
        ],
    ).map_err(|error| error.to_string())?;
    if enabled && globally_paused() {
        return Err("Bots are globally paused; GitHub side-effect authorization cannot be enabled.".into());
    }
    tx.commit().map_err(|error| error.to_string())
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
            exact_grant(&tx, intent)?;
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
    // Stop waits on this fence before it starts its read-then-write transition.
    // Taking the same fence around the settlement write means Stop either sees
    // the settled receipt before its read snapshot or settles first; it can no
    // longer lose its write to a newer WAL snapshot committed in between.
    let reconciliation = {
        let _settlement_fence = lock_dispatch(&store.path)?;
        match observed {
            Ok(Readback::Exact(evidence)) => {
                if let Err(error) = settle_applied(store, intent, &evidence) {
                    let _ = mark_uncertain(store, intent, "readback_settlement_failed", "exact");
                    return Err(error);
                }
                "exact-readback"
            }
            Ok(Readback::Missing) => {
                mark_uncertain(store, intent, "marker_not_found", "missing")?;
                "marker-not-found"
            }
            Ok(Readback::Conflict) => {
                mark_uncertain(store, intent, "marker_conflict", "conflict")?;
                "marker-conflict"
            }
            Err(_) => {
                mark_uncertain(store, intent, "readback_unavailable", "unavailable")?;
                "readback-unavailable"
            }
        }
    };
    status(store, intent, reconciliation)
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
            let mut attempted = false;
            let dispatch = (|| -> Result<std::result::Result<crate::workspace::Capture, String>> {
                let _fence = lock_dispatch(database_path)?;
                let scope = validate_pending_dispatch(&store, intent)?;
                let root = store.workspace_root(&scope.project_id, Some(&scope.session_id))?;
                attempted = true;
                let remote_result = run_gh(
                    &root,
                    &["issue", "comment", &issue, &repo_flag, &body_flag],
                    github::TIMEOUT,
                );
                // The adapter may have timed out after GitHub accepted the
                // request. Persist uncertainty before releasing the fence and
                // before any readback, so a waiting Stop observes the
                // uncertain receipt instead of racing this write with its own
                // read-then-write transaction.
                mark_uncertain(&store, intent, "write_result_unverified", "not_attempted")?;
                Ok(remote_result)
            })();
            match dispatch {
                Err(error) => {
                    // A failed settlement after the provider call must not be
                    // overwritten by "dispatch_not_started". Retry under the
                    // fence when possible so Stop cannot interleave a commit.
                    let reason = if attempted {
                        "write_result_unverified"
                    } else {
                        "dispatch_not_started"
                    };
                    // Best-effort settlement, fenced when the lock is
                    // available so Stop cannot interleave. This also retries
                    // an uncertainty write that failed after the provider
                    // call, rather than leaving a pending receipt behind.
                    let _fence = lock_dispatch(database_path).ok();
                    let _ = mark_uncertain(&store, intent, reason, "not_attempted");
                    Err(error)
                }
                Ok(_remote_result) => {
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

struct ReceiptEvidence {
    state: String,
    uncertainty_reason: Option<String>,
    readback_state: Option<String>,
    remote_comment_id: Option<String>,
    remote_comment_url: Option<String>,
    remote_comment_author: Option<String>,
}

fn status(store: &Store, intent: &IssueCommentIntent, reconciliation: &str) -> Result<EffectStatus> {
    let evidence = store.db
        .query_row(
            "SELECT state,uncertainty_reason,readback_state,remote_comment_id,
                    remote_comment_url,remote_author_login
             FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
            params![intent.ticket_id,intent.effect_key,intent.payload_sha256],
            |row| Ok(ReceiptEvidence {
                state: row.get(0)?,
                uncertainty_reason: row.get(1)?,
                readback_state: row.get(2)?,
                remote_comment_id: row.get(3)?,
                remote_comment_url: row.get(4)?,
                remote_comment_author: row.get(5)?,
            }),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(evidence) = evidence else {
        return Err("The exact external-effect receipt disappeared during reconciliation.".into());
    };
    Ok(EffectStatus {
        ticket_id: intent.ticket_id.clone(),
        ticket_revision: intent.expected_revision,
        repository: intent.repository.clone(),
        issue_number: intent.issue_number,
        effect_key: intent.effect_key.clone(),
        payload_sha256: intent.payload_sha256.clone(),
        state: evidence.state,
        reconciliation: reconciliation.into(),
        uncertainty_reason: evidence.uncertainty_reason,
        readback_state: evidence.readback_state,
        remote_comment_id: evidence.remote_comment_id,
        remote_comment_url: evidence.remote_comment_url,
        remote_comment_author: evidence.remote_comment_author,
    })
}

fn active_draft_pr_scope_if_current(
    db: &Connection,
    intent: &DraftPullRequestIntent,
) -> Result<Option<ActiveScope>> {
    active_draft_pr_scope_for(db, &intent.ticket_id, intent.expected_revision)
}

fn active_draft_pr_scope_for(
    db: &Connection,
    ticket_id: &str,
    expected_revision: i64,
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
           AND m.expires_at>?3
           AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision AND t.project_id=m.project_id
           AND m.project_id=o.project_id AND t.actor=m.actor AND m.authorization_id<>''
           AND b.project_id=t.project_id AND b.paused=0 AND o.enabled=1
           AND EXISTS (
             SELECT 1 FROM sa_audit choice
             WHERE choice.ticket_id=t.id AND choice.kind='side_effect_consent_enabled'
               AND json_extract(choice.detail,'$.authorizationId')=m.authorization_id
               AND json_extract(choice.detail,'$.policyRevision')=m.policy_revision
               AND json_extract(choice.detail,'$.projectId')=m.project_id
               AND json_extract(choice.detail,'$.assignmentId')=a.id
               AND json_extract(choice.detail,'$.sessionId')=a.session_id
               AND NOT EXISTS (
                 SELECT 1 FROM sa_audit newer
                 WHERE newer.ticket_id=choice.ticket_id
                   AND newer.kind IN ('side_effect_consent_enabled','side_effect_consent_revoked')
                   AND newer.id>choice.id
               )
           )",
        params![ticket_id, expected_revision, storage::now()],
        |row| Ok(ActiveScope {
            project_id: row.get(0)?,
            assignment_id: row.get(1)?,
            session_id: row.get(2)?,
        }),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn active_draft_pr_scope(
    db: &Connection,
    intent: &DraftPullRequestIntent,
) -> Result<ActiveScope> {
    if crate::feature_controls::get().bots_paused {
        return Err("Native draft pull request authorization is unavailable while Bots are paused.".into());
    }
    active_draft_pr_scope_if_current(db, intent)?.ok_or_else(|| {
        "The Ticket, revision, mandate, or active worker no longer permits this effect.".into()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DraftPrGrantBinding {
    project_id: String,
    assignment_id: String,
    session_id: String,
}

fn exact_draft_pr_grant_binding(
    db: &Connection,
    intent: &DraftPullRequestIntent,
) -> Result<DraftPrGrantBinding> {
    let grant: Option<(i64,String,String,String,String,String,String,String,String,String,String,String)> = db
        .query_row(
            "SELECT ticket_revision,project_id,assignment_id,session_id,repository,
                    head_branch,head_sha,base_branch,title,body,marker,payload_sha256
             FROM sa_draft_pr_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id,intent.effect_key],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,
                row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((revision,project_id,assignment_id,session_id,repository,head,head_sha,base,title,body,marker,digest)) = grant else {
        return Err("The exact native draft pull request grant is missing.".into());
    };
    if revision != intent.expected_revision
        || repository != intent.repository
        || head != intent.head
        || head_sha != intent.head_sha
        || base != intent.base
        || title != intent.title
        || body != intent.body
        || marker != intent.marker
        || digest != intent.payload_sha256
    {
        return Err("The native draft pull request grant does not match this exact Ticket, target, key, and payload.".into());
    }
    if assignment_id.is_empty() || assignment_id.len() > 128 || session_id.is_empty() || session_id.len() > 128 {
        return Err("The exact native draft pull request assignment binding is invalid.".into());
    }
    let audit: Option<String> = db
        .query_row(
            "SELECT detail FROM sa_audit
             WHERE ticket_id=?1 AND kind='draft_pull_request_granted'
               AND json_extract(detail,'$.effectKey')=?2
               AND json_extract(detail,'$.payloadSha256')=?3
             ORDER BY id DESC LIMIT 1",
            params![intent.ticket_id,intent.effect_key,intent.payload_sha256],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let detail: serde_json::Value = serde_json::from_str(
        audit.as_deref().ok_or_else(|| "The exact native draft pull request grant audit is missing.".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let audit_matches = detail.get("ticketRevision").and_then(serde_json::Value::as_i64)
            == Some(intent.expected_revision)
        && detail.get("projectId").and_then(serde_json::Value::as_str) == Some(project_id.as_str())
        && detail.get("assignmentId").and_then(serde_json::Value::as_str) == Some(assignment_id.as_str())
        && detail.get("sessionId").and_then(serde_json::Value::as_str) == Some(session_id.as_str())
        && detail.get("repository").and_then(serde_json::Value::as_str) == Some(intent.repository.as_str())
        && detail.get("head").and_then(serde_json::Value::as_str) == Some(intent.head.as_str())
        && detail.get("headSha").and_then(serde_json::Value::as_str) == Some(intent.head_sha.as_str())
        && detail.get("base").and_then(serde_json::Value::as_str) == Some(intent.base.as_str())
        && detail.get("effectKey").and_then(serde_json::Value::as_str) == Some(intent.effect_key.as_str())
        && detail.get("payloadSha256").and_then(serde_json::Value::as_str) == Some(intent.payload_sha256.as_str());
    if !audit_matches {
        return Err("The native draft pull request grant audit does not match this exact intent.".into());
    }
    let assignment_exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE id=?1 AND ticket_id=?2 AND session_id=?3)",
        params![assignment_id,intent.ticket_id,session_id],|row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let ticket_project: Option<String> = db.query_row(
        "SELECT project_id FROM sa_tickets WHERE id=?1",[&intent.ticket_id],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if !assignment_exists || ticket_project.as_deref() != Some(project_id.as_str()) {
        return Err("The native draft pull request grant no longer matches its original assignment and project.".into());
    }
    let session_project: Option<String> = db.query_row(
        "SELECT project_id FROM sessions WHERE id=?1",[&session_id],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if session_project.as_deref().is_some_and(|value| value != project_id) {
        return Err("The native draft pull request session has moved outside its granted project.".into());
    }
    Ok(DraftPrGrantBinding { project_id, assignment_id, session_id })
}

fn exact_draft_pr_grant_scope(
    db: &Connection,
    intent: &DraftPullRequestIntent,
    scope: &ActiveScope,
) -> Result<bool> {
    let binding = exact_draft_pr_grant_binding(db,intent)?;
    Ok(binding.project_id == scope.project_id
        && binding.assignment_id == scope.assignment_id
        && binding.session_id == scope.session_id)
}

fn exact_draft_pr_grant(db: &Connection, intent: &DraftPullRequestIntent) -> Result<ActiveScope> {
    let scope = active_draft_pr_scope(db,intent)?;
    if !exact_draft_pr_grant_scope(db,intent,&scope)? {
        return Err("The native draft pull request grant does not match this exact Ticket, assignment, session, target, key, and payload.".into());
    }
    Ok(scope)
}

pub(crate) fn has_exact_draft_pr_grant(
    store: &Store,
    intent: &DraftPullRequestIntent,
) -> Result<bool> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let existing: Option<String> = tx.query_row(
        "SELECT payload_sha256 FROM sa_draft_pr_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
        params![intent.ticket_id,intent.effect_key],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    let present = match existing {
        None => { active_draft_pr_scope(&tx,intent)?; false }
        Some(digest) if digest == intent.payload_sha256 => {
            let binding = exact_draft_pr_grant_binding(&tx,intent)?;
            match active_draft_pr_scope_if_current(&tx,intent)? {
                Some(scope) if scope.project_id == binding.project_id
                    && exact_draft_pr_grant_scope(&tx,intent,&scope)? => true,
                _ => {
                    let receipt_state: Option<String> = tx.query_row(
                        "SELECT state FROM sa_draft_pr_effect_receipts
                         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
                        params![intent.ticket_id,intent.effect_key,intent.payload_sha256],|row| row.get(0),
                    ).optional().map_err(|error| error.to_string())?;
                    if matches!(receipt_state.as_deref(),Some("pending"|"uncertain")) { true }
                    else { exact_draft_pr_grant(&tx,intent)?; true }
                }
            }
        }
        Some(_) => return Err("Effect key already has a draft pull request grant for a different exact payload.".into()),
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(present)
}

pub(crate) fn authorize_draft_pull_request(
    store: &Store,
    intent: &DraftPullRequestIntent,
) -> Result<()> {
    authorize_draft_pull_request_with(store,intent,|root,intent| {
        github::verify_draft_pull_request_target(
            root,&intent.repository,&intent.head,&intent.base,
        )
    })
}

pub(crate) fn verify_draft_pull_request_target(
    store: &Store,
    ticket_id: &str,
    expected_revision: i64,
    repository: &str,
    head: &str,
    base: &str,
) -> Result<String> {
    validate_repository(repository)?;
    if expected_revision < 0 {
        return Err("Provide a nonnegative Ticket revision.".into());
    }
    let initial_scope = active_draft_pr_scope_for(&store.db, ticket_id, expected_revision)?
        .ok_or_else(|| "The Ticket, revision, mandate, or active worker no longer permits this effect.".to_string())?;
    let root = store.workspace_root(&initial_scope.project_id, Some(&initial_scope.session_id))?;
    let verified_sha = github::verify_draft_pull_request_target(root.as_path(), repository, head, base)?;
    let current_scope = active_draft_pr_scope_for(&store.db, ticket_id, expected_revision)?;
    if current_scope.as_ref() != Some(&initial_scope) {
        return Err("The active Ticket worker changed during draft pull request verification.".into());
    }
    Ok(verified_sha)
}

fn authorize_draft_pull_request_with(
    store: &Store,
    intent: &DraftPullRequestIntent,
    mut verify_target: impl FnMut(&Path,&DraftPullRequestIntent)->Result<String>,
) -> Result<()> {
    let _dispatch_fence = lock_dispatch(&store.path)?;
    let initial_scope = {
        let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
        let scope = active_draft_pr_scope(&tx,intent)?;
        tx.commit().map_err(|error| error.to_string())?;
        scope
    };
    let root = store.workspace_root(&initial_scope.project_id,Some(&initial_scope.session_id))?;
    let verified_sha = verify_target(&root,intent)?;
    if verified_sha != intent.head_sha {
        return Err("The verified remote head SHA changed; review the draft pull request again.".into());
    }
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let current_scope = active_draft_pr_scope(&tx,intent)?;
    if current_scope != initial_scope {
        return Err("The active Ticket worker changed during draft pull request authorization.".into());
    }
    let existing: Option<String> = tx.query_row(
        "SELECT payload_sha256 FROM sa_draft_pr_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
        params![intent.ticket_id,intent.effect_key],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    match existing {
        Some(digest) if digest == intent.payload_sha256 => {
            exact_draft_pr_grant(&tx,intent)?;
            tx.commit().map_err(|error| error.to_string())?;
            return Ok(());
        }
        Some(_) => return Err("Effect key already has a draft pull request grant for a different exact payload.".into()),
        None => {}
    }
    let now = storage::now();
    tx.execute(
        "INSERT INTO sa_draft_pr_effect_grants(ticket_id,effect_key,ticket_revision,project_id,
         assignment_id,session_id,repository,head_branch,head_sha,base_branch,title,body,marker,
         payload_sha256,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![intent.ticket_id,intent.effect_key,intent.expected_revision,current_scope.project_id,
            current_scope.assignment_id,current_scope.session_id,intent.repository,intent.head,
            intent.head_sha,intent.base,intent.title,intent.body,intent.marker,intent.payload_sha256,now],
    ).map_err(|error| error.to_string())?;
    let detail = serde_json::json!({
        "effectKey":intent.effect_key,"ticketRevision":intent.expected_revision,
        "projectId":current_scope.project_id,"assignmentId":current_scope.assignment_id,
        "sessionId":current_scope.session_id,"repository":intent.repository,
        "head":intent.head,"headSha":intent.head_sha,"base":intent.base,"title":intent.title,
        "marker":intent.marker,"payloadSha256":intent.payload_sha256,
    }).to_string();
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'draft_pull_request_granted',?2,?3)",
        params![intent.ticket_id,detail,now],
    ).map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())
}

fn reserve_draft_pull_request(
    store: &Store,
    intent: &DraftPullRequestIntent,
) -> Result<Reservation> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let existing: Option<(String,String)> = tx.query_row(
        "SELECT payload_sha256,state FROM sa_draft_pr_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2",
        params![intent.ticket_id,intent.effect_key],
        |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?;
    let result = match existing {
        Some((digest,_)) if digest != intent.payload_sha256 => {
            return Err("Effect key already used for a different draft pull request payload.".into());
        }
        Some((_,state)) => {
            let binding = exact_draft_pr_grant_binding(&tx,intent)?;
            match state.as_str() {
                "pending" => match active_draft_pr_scope_if_current(&tx,intent)? {
                    Some(scope) if scope.project_id == binding.project_id
                        && exact_draft_pr_grant_scope(&tx,intent,&scope)? => Reservation::AlreadyPending,
                    _ => Reservation::RequiresReconciliation,
                },
                "uncertain" => Reservation::RequiresReconciliation,
                "applied" => Reservation::AlreadyApplied,
                _ => return Err("Invalid draft pull request effect state.".into()),
            }
        }
        None => {
            exact_draft_pr_grant(&tx,intent)?;
            let now = storage::now();
            tx.execute(
                "INSERT INTO sa_draft_pr_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,readback_state,created_at,updated_at)
                 VALUES(?1,?2,?3,'pending','not_attempted',?4,?4)",
                params![intent.ticket_id,intent.effect_key,intent.payload_sha256,now],
            ).map_err(|error| error.to_string())?;
            Reservation::Reserved
        }
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(result)
}

fn validate_pending_draft_pr_dispatch(
    store: &Store,
    intent: &DraftPullRequestIntent,
) -> Result<ActiveScope> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let scope = exact_draft_pr_grant(&tx,intent)?;
    let state: Option<String> = tx.query_row(
        "SELECT state FROM sa_draft_pr_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
        params![intent.ticket_id,intent.effect_key,intent.payload_sha256],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if state.as_deref() != Some("pending") {
        return Err("The draft pull request receipt is no longer pending; dispatch was not attempted.".into());
    }
    tx.commit().map_err(|error| error.to_string())?;
    Ok(scope)
}

fn mark_draft_pr_uncertain(
    store: &Store,
    intent: &DraftPullRequestIntent,
    reason: &str,
    readback_state: &str,
) -> Result<()> {
    let changed = store.db.execute(
        "UPDATE sa_draft_pr_effect_receipts SET state='uncertain',uncertainty_reason=?4,
         readback_state=?5,remote_pr_number=NULL,remote_pr_url=NULL,updated_at=?6
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
           AND state IN ('pending','uncertain')",
        params![intent.ticket_id,intent.effect_key,intent.payload_sha256,reason,readback_state,storage::now()],
    ).map_err(|error| error.to_string())?;
    if changed == 1 { return Ok(()); }
    let state: Option<String> = store.db.query_row(
        "SELECT state FROM sa_draft_pr_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
        params![intent.ticket_id,intent.effect_key,intent.payload_sha256],|row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if matches!(state.as_deref(),Some("uncertain"|"applied")) { Ok(()) }
    else { Err("No unresolved matching draft pull request receipt.".into()) }
}

#[derive(Debug,Clone,PartialEq,Eq)]
struct DraftPullRequestEvidence {
    number: Option<i64>,
    url: Option<String>,
}

#[derive(Debug,Clone,PartialEq,Eq)]
enum DraftPullRequestReadback {
    Exact(DraftPullRequestEvidence),
    Missing,
    Conflict,
}

fn bounded_pr_text(value: Option<&serde_json::Value>, max_bytes: usize) -> Option<String> {
    let value = value?.as_str()?;
    (!value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

fn draft_pr_readback_state(
    value: &serde_json::Value,
    intent: &DraftPullRequestIntent,
) -> Result<DraftPullRequestReadback> {
    let pulls = value.as_array().ok_or("GitHub did not return pull requests.")?;
    let mut exact = Vec::new();
    let mut conflict = false;
    for pull in pulls {
        // Blank-body PRs are ordinary unrelated results. They cannot carry our
        // marker, so treat them as non-matches and keep the scan fail-closed.
        let body = pull.get("body").and_then(serde_json::Value::as_str).unwrap_or("");
        if !body.contains(&intent.marker) { continue; }
        let head = pull.get("head");
        let base = pull.get("base");
        let target_matches = head.and_then(|value| value.get("ref"))
                .and_then(serde_json::Value::as_str) == Some(intent.head.as_str())
            && head.and_then(|value| value.get("sha"))
                .and_then(serde_json::Value::as_str) == Some(intent.head_sha.as_str())
            && base.and_then(|value| value.get("ref"))
                .and_then(serde_json::Value::as_str) == Some(intent.base.as_str())
            && head.and_then(|value| value.get("repo"))
                .and_then(|value| value.get("full_name"))
                .and_then(serde_json::Value::as_str) == Some(intent.repository.as_str())
            && base.and_then(|value| value.get("repo"))
                .and_then(|value| value.get("full_name"))
                .and_then(serde_json::Value::as_str) == Some(intent.repository.as_str());
        let number = pull.get("number").and_then(serde_json::Value::as_i64);
        let url = bounded_pr_text(pull.get("html_url"),2048)
            .filter(|value| value.starts_with("https://github.com/"));
        if target_matches
            && body == intent.body
            && pull.get("title").and_then(serde_json::Value::as_str) == Some(intent.title.as_str())
            && pull.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
            && number.is_some_and(|number| number > 0)
            && url.is_some()
        {
            exact.push(DraftPullRequestEvidence { number, url });
        } else {
            conflict = true;
        }
    }
    Ok(match (exact.len(),conflict) {
        (0,false) => DraftPullRequestReadback::Missing,
        (1,false) => DraftPullRequestReadback::Exact(exact.pop().expect("one exact PR readback")),
        _ => DraftPullRequestReadback::Conflict,
    })
}

fn settle_draft_pr_applied(
    store: &Store,
    intent: &DraftPullRequestIntent,
    evidence: &DraftPullRequestEvidence,
) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    exact_draft_pr_grant_binding(&tx,intent)?;
    let changed = tx.execute(
        "UPDATE sa_draft_pr_effect_receipts SET state='applied',uncertainty_reason=NULL,
         readback_state='exact',remote_pr_number=?4,remote_pr_url=?5,updated_at=?6
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
           AND state IN ('pending','uncertain')",
        params![intent.ticket_id,intent.effect_key,intent.payload_sha256,evidence.number,evidence.url,storage::now()],
    ).map_err(|error| error.to_string())?;
    if changed != 1 {
        let state: Option<String> = tx.query_row(
            "SELECT state FROM sa_draft_pr_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
            params![intent.ticket_id,intent.effect_key,intent.payload_sha256],|row| row.get(0),
        ).optional().map_err(|error| error.to_string())?;
        if state.as_deref() == Some("applied") {
            tx.commit().map_err(|error| error.to_string())?;
            return Ok(());
        }
        return Err("Missing unresolved matching draft pull request receipt.".into());
    }
    let detail = serde_json::json!({
        "effectKey":intent.effect_key,"ticketRevision":intent.expected_revision,
        "repository":intent.repository,"head":intent.head,"base":intent.base,
        "payloadSha256":intent.payload_sha256,"pullRequestNumber":evidence.number,
    }).to_string();
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'draft_pull_request_applied',?2,?3)",
        params![intent.ticket_id,detail,storage::now()],
    ).map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())
}

fn draft_pr_status(
    store: &Store,
    intent: &DraftPullRequestIntent,
    reconciliation: &str,
) -> Result<DraftPullRequestEffectStatus> {
    let row: Option<(String,Option<String>,Option<String>,Option<i64>,Option<String>)> = store.db.query_row(
        "SELECT state,uncertainty_reason,readback_state,remote_pr_number,remote_pr_url
         FROM sa_draft_pr_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
        params![intent.ticket_id,intent.effect_key,intent.payload_sha256],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
    ).optional().map_err(|error| error.to_string())?;
    let Some((state,uncertainty_reason,readback_state,remote_pr_number,remote_pr_url)) = row else {
        return Err("The exact draft pull request receipt disappeared during reconciliation.".into());
    };
    Ok(DraftPullRequestEffectStatus {
        ticket_id:intent.ticket_id.clone(),ticket_revision:intent.expected_revision,
        repository:intent.repository.clone(),head:intent.head.clone(),head_sha:intent.head_sha.clone(),
        base:intent.base.clone(),
        title:intent.title.clone(),effect_key:intent.effect_key.clone(),
        payload_sha256:intent.payload_sha256.clone(),state,reconciliation:reconciliation.into(),
        uncertainty_reason,readback_state,remote_pr_number,remote_pr_url,
    })
}

fn reconcile_draft_pr_unresolved(
    store: &Store,
    intent: &DraftPullRequestIntent,
    root: &Path,
    run_gh: &mut impl FnMut(&Path,&[&str],Duration)->std::result::Result<crate::workspace::Capture,String>,
) -> Result<DraftPullRequestEffectStatus> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    exact_draft_pr_grant_binding(&tx,intent)?;
    let receipt: Option<(String,String)> = tx.query_row(
        "SELECT state,payload_sha256 FROM sa_draft_pr_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2",
        params![intent.ticket_id,intent.effect_key],|row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?;
    let Some((state,digest)) = receipt else { return Err("No draft pull request receipt exists to reconcile.".into()); };
    if digest != intent.payload_sha256 { return Err("Draft pull request receipt does not match this exact payload.".into()); }
    if state == "applied" {
        tx.commit().map_err(|error| error.to_string())?;
        return draft_pr_status(store,intent,"already-applied");
    }
    if !matches!(state.as_str(),"pending"|"uncertain") { return Err("Invalid draft pull request effect state.".into()); }
    tx.commit().map_err(|error| error.to_string())?;

    let observed = github::read_draft_pull_requests_with(
        root,&intent.repository,&intent.head,&intent.base,run_gh,
    ).and_then(|value| draft_pr_readback_state(&value,intent));
    let reconciliation = {
        let _settlement_fence = lock_dispatch(&store.path)?;
        match observed {
            Ok(DraftPullRequestReadback::Exact(evidence)) => {
                if let Err(error) = settle_draft_pr_applied(store,intent,&evidence) {
                    let _ = mark_draft_pr_uncertain(store,intent,"readback_settlement_failed","exact");
                    return Err(error);
                }
                "exact-readback"
            }
            Ok(DraftPullRequestReadback::Missing) => {
                mark_draft_pr_uncertain(store,intent,"marker_not_found","missing")?;
                "marker-not-found"
            }
            Ok(DraftPullRequestReadback::Conflict) => {
                mark_draft_pr_uncertain(store,intent,"marker_conflict","conflict")?;
                "marker-conflict"
            }
            Err(_) => {
                mark_draft_pr_uncertain(store,intent,"readback_unavailable","unavailable")?;
                "readback-unavailable"
            }
        }
    };
    draft_pr_status(store,intent,reconciliation)
}

pub(crate) fn create_draft_pull_request(
    database_path: &Path,
    intent: &DraftPullRequestIntent,
) -> Result<DraftPullRequestEffectStatus> {
    let mut run_gh = |root:&Path,args:&[&str],timeout| github::run_gh(root,args,timeout);
    create_draft_pull_request_with(database_path,intent,&mut run_gh)
}

fn create_draft_pull_request_with(
    database_path: &Path,
    intent: &DraftPullRequestIntent,
    run_gh: &mut impl FnMut(&Path,&[&str],Duration)->std::result::Result<crate::workspace::Capture,String>,
) -> Result<DraftPullRequestEffectStatus> {
    let store = open_effect_store(database_path)?;
    let binding = {
        let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
        let binding = exact_draft_pr_grant_binding(&tx,intent)?;
        tx.commit().map_err(|error| error.to_string())?;
        binding
    };
    let root = store.workspace_root(&binding.project_id,Some(&binding.session_id))?;
    match reserve_draft_pull_request(&store,intent)? {
        Reservation::AlreadyApplied => draft_pr_status(&store,intent,"already-applied"),
        Reservation::AlreadyPending => draft_pr_status(&store,intent,"dispatch-in-progress"),
        Reservation::RequiresReconciliation => reconcile_draft_pr_unresolved(&store,intent,&root,run_gh),
        Reservation::Reserved => {
            let mut attempted = false;
            let dispatch = (|| -> Result<std::result::Result<(),String>> {
                let _fence = lock_dispatch(database_path)?;
                let scope = validate_pending_draft_pr_dispatch(&store,intent)?;
                let dispatch_root = store.workspace_root(&scope.project_id,Some(&scope.session_id))?;
                let verified_sha = github::verify_draft_pull_request_target_with(
                    &dispatch_root,&intent.repository,&intent.head,&intent.base,run_gh,
                )?;
                if verified_sha != intent.head_sha {
                    return Err("The verified remote head SHA changed; the draft pull request was not dispatched.".into());
                }
                attempted = true;
                let remote_result = github::create_draft_pull_request_with(
                    &dispatch_root,&intent.repository,&intent.head,&intent.base,
                    &intent.title,&intent.body,run_gh,
                );
                mark_draft_pr_uncertain(&store,intent,"write_result_unverified","not_attempted")?;
                Ok(remote_result)
            })();
            match dispatch {
                Err(error) => {
                    let reason = if attempted { "write_result_unverified" } else { "dispatch_not_started" };
                    {
                        let _fence = lock_dispatch(database_path).ok();
                        let _ = mark_draft_pr_uncertain(&store,intent,reason,"not_attempted");
                    }
                    if attempted {
                        reconcile_draft_pr_unresolved(&store,intent,&root,run_gh)
                    } else {
                        let _ = error;
                        draft_pr_status(&store,intent,"dispatch-not-started")
                    }
                }
                Ok(_remote_result) => reconcile_draft_pr_unresolved(&store,intent,&root,run_gh),
            }
        }
    }
}

pub(crate) fn unresolved_count(store: &Store, ticket_id: &str) -> Result<i64> {
    store
        .db
        .query_row(
            "SELECT (SELECT COUNT(*) FROM sa_effect_receipts
                      WHERE ticket_id=?1 AND state!='applied')
                  + (SELECT COUNT(*) FROM sa_draft_pr_effect_receipts
                      WHERE ticket_id=?1 AND state!='applied')",
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
        fixture_with_side_effects(true)
    }

    fn fixture_with_side_effects(with_consent: bool) -> (Store, String) {
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
        if with_consent {
            set_side_effect_consent(&store, &ticket.id, ticket.revision, true).unwrap();
        }
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

    fn git_test(root: &Path, args: &[&str]) -> String {
        let output = crate::workspace::run_git_bounded(
            root,args,16*1024,true,Duration::from_secs(15),
        ).unwrap();
        assert!(output.status.success(),"git {:?}: {}",args,String::from_utf8_lossy(&output.stderr.bytes));
        String::from_utf8_lossy(&output.stdout.bytes).trim().to_string()
    }

    fn draft_pr_fixture() -> (Store,String,std::path::PathBuf,String,String) {
        let (store,ticket_id) = fixture();
        let root = std::env::temp_dir().join(format!("sa-draft-pr-worktree-{}",storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        git_test(&root,&["init"]);
        git_test(&root,&["config","user.name","PandaCode Test"]);
        git_test(&root,&["config","user.email","pandacode@example.invalid"]);
        git_test(&root,&["checkout","-b","main"]);
        git_test(&root,&["commit","--allow-empty","-m","base"]);
        let base_sha = git_test(&root,&["rev-parse","HEAD"]);
        git_test(&root,&["checkout","-b","feature/ticket-pr"]);
        git_test(&root,&["commit","--allow-empty","-m","pushed head"]);
        let head_sha = git_test(&root,&["rev-parse","HEAD"]);
        store.db.execute("UPDATE projects SET path=?1 WHERE id='p'",[root.to_string_lossy().as_ref()]).unwrap();
        (store,ticket_id,root,head_sha,base_sha)
    }

    fn make_draft_pr_intent(ticket_id:&str,head:&str,base:&str,body:&str)->DraftPullRequestIntent {
        let head_sha = "a".repeat(40);
        make_draft_pr_intent_with_sha(ticket_id,head,&head_sha,base,body)
    }

    fn make_draft_pr_intent_with_sha(
        ticket_id:&str,head:&str,head_sha:&str,base:&str,body:&str,
    )->DraftPullRequestIntent {
        let title = "Draft effect fixture";
        let key = draft_pr_key(ticket_id,"owner/repo",head,base,title,body).unwrap();
        prepare_draft_pull_request(ticket_id,0,"owner/repo",head,head_sha,base,title,body,&key).unwrap()
    }

    fn exact_draft_pr_readback(intent:&DraftPullRequestIntent)->crate::workspace::Capture {
        capture_json(serde_json::json!([{
            "number":86,"html_url":"https://github.com/owner/repo/pull/86",
            "title":intent.title,"body":intent.body,"draft":true,
            "head":{"ref":intent.head,"sha":intent.head_sha,"repo":{"full_name":intent.repository}},
            "base":{"ref":intent.base,"repo":{"full_name":intent.repository}},
        }]))
    }

    fn fake_draft_pr_gh(
        intent:DraftPullRequestIntent,
        head_sha:String,
        base_sha:String,
        readbacks:Vec<serde_json::Value>,
        writes:Arc<AtomicUsize>,
        read_count:Arc<AtomicUsize>,
        fail_create:bool,
    ) -> impl FnMut(&Path,&[&str],Duration)->std::result::Result<crate::workspace::Capture,String> {
        let mut next_readback = 0usize;
        move |_:&Path,args:&[&str],timeout:Duration| {
            assert!(timeout<=TIMEOUT);
            if args.first()==Some(&"repo") && args.get(1)==Some(&"view") {
                return Ok(capture_json(serde_json::json!({"nameWithOwner":"owner/repo"})));
            }
            if args.first()==Some(&"api") && args.get(1).is_some_and(|path|path.contains("/branches/")) {
                let path=args[1];
                let (name,sha)=if path.ends_with("/branches/main") { ("main",base_sha.as_str()) }
                    else { (intent.head.as_str(),head_sha.as_str()) };
                return Ok(capture_json(serde_json::json!({"name":name,"commit":{"sha":sha}})));
            }
            if args.first()==Some(&"api") && args.get(1).is_some_and(|path|path.ends_with("/pulls")) {
                writes.fetch_add(1,Ordering::SeqCst);
                assert!(args.contains(&"--method=POST"));
                assert!(args.windows(2).any(|pair|pair==["-F","draft=true"]));
                assert!(args.contains(&"maintainer_can_modify=false"));
                assert!(args.contains(&format!("head={}",intent.head).as_str()));
                assert!(args.contains(&format!("base={}",intent.base).as_str()));
                assert!(args.contains(&format!("body={}",intent.body).as_str()));
                if fail_create { return Err("GitHub request timed out after dispatch".into()); }
                return Ok(capture_json(serde_json::json!({"number":86})));
            }
            if args.first()==Some(&"api") && args.get(1).is_some_and(|path|path.contains("/pulls?")) {
                read_count.fetch_add(1,Ordering::SeqCst);
                let result=readbacks.get(next_readback).cloned().unwrap_or_else(||serde_json::json!([]));
                next_readback+=1;
                return Ok(capture_json(result));
            }
            panic!("unexpected fake gh command: {args:?}");
        }
    }

    fn capture_json(value: serde_json::Value) -> crate::workspace::Capture {
        crate::workspace::Capture {
            bytes: value.to_string().into_bytes(),
            truncated: false,
        }
    }

    fn is_issue_comment_write(args: &[&str]) -> bool {
        args.first() == Some(&"issue") && args.get(1) == Some(&"comment")
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
        let grant_count: i64 = store
            .db
            .query_row(
                "SELECT COUNT(*) FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
                params![&id, &intent.effect_key],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(grant_count, 0, "revocation must not leave an exact grant");
        let lookup_error = has_exact_grant(&store, &intent)
            .expect_err("a revoked mandate must not present an exact grant");
        assert_eq!(
            lookup_error,
            "The Ticket, revision, mandate, or active worker no longer permits this effect."
        );
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
                .truncate(false)
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

    #[test]
    fn operator_side_effect_consent_defaults_denied_and_records_bounded_enable_and_revoke() {
        let (store, ticket_id) = fixture_with_side_effects(false);
        let ticket = superagent::get_ticket(&store, &ticket_id).unwrap();
        let initial: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(initial, 0, "new Ticket mandates remain default-deny");

        let scope = side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, true,
        ).unwrap();
        assert!(!scope.consent_active);
        set_side_effect_consent(&store, &ticket_id, ticket.revision, true).unwrap();
        let enabled: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(enabled, 0, "operator consent never mutates the immutable admission mandate");
        let session_id = superagent::assignment_session(&store, &ticket_id).unwrap().unwrap();
        assert!(superagent::read_only_policy(&store, &session_id).unwrap().is_some(),
            "operator GitHub consent must not disable the worker's native read-only policy");
        assert!(has_active_side_effect_consent(&store, &ticket_id).unwrap());
        let (kind, detail): (String, String) = store.db.query_row(
            "SELECT kind,detail FROM sa_audit WHERE ticket_id=?1 ORDER BY id DESC LIMIT 1",
            [&ticket_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        let detail: serde_json::Value = serde_json::from_str(&detail).unwrap();
        assert_eq!(kind, "side_effect_consent_enabled");
        assert_eq!(detail["actor"], "local-user");
        assert_eq!(detail["choice"], "enabled");
        assert_eq!(detail["sideEffectConsent"], true);
        assert_eq!(detail["ticketRevision"], ticket.revision);
        assert_eq!(detail["assignmentId"], scope.assignment_id);
        assert_eq!(detail["sessionId"], scope.session_id);
        assert_eq!(detail["supportedEffects"].as_array().unwrap().len(), 2);

        let current = side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, false,
        ).unwrap();
        assert!(current.consent_active);
        set_side_effect_consent(&store, &ticket_id, ticket.revision, false).unwrap();
        let revoked: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(revoked, 0);
        assert!(!has_active_side_effect_consent(&store, &ticket_id).unwrap());
        let (kind, detail): (String, String) = store.db.query_row(
            "SELECT kind,detail FROM sa_audit WHERE ticket_id=?1 ORDER BY id DESC LIMIT 1",
            [&ticket_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        let detail: serde_json::Value = serde_json::from_str(&detail).unwrap();
        assert_eq!(kind, "side_effect_consent_revoked");
        assert_eq!(detail["actor"], "local-user");
        assert_eq!(detail["choice"], "revoked");
        assert_eq!(detail["sideEffectConsent"], false);
    }

    #[test]
    fn operator_side_effect_consent_requires_exact_live_ticket_assignment_and_policy() {
        let (store, ticket_id) = fixture_with_side_effects(false);
        let ticket = superagent::get_ticket(&store, &ticket_id).unwrap();
        assert!(side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision + 1, true,
        ).is_err(), "stale Ticket revisions are denied");

        store.db.execute(
            "UPDATE sa_mandates SET expires_at=?2 WHERE ticket_id=?1",
            params![ticket_id, storage::now() - 1],
        ).unwrap();
        assert!(side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, true,
        ).is_err(), "expired mandates are denied");
        store.db.execute(
            "UPDATE sa_mandates SET expires_at=?2 WHERE ticket_id=?1",
            params![ticket_id, storage::now() + 60_000],
        ).unwrap();

        store.db.execute(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",
            [&ticket_id],
        ).unwrap();
        assert!(side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, true,
        ).is_err(), "revoked mandates are denied");
        store.db.execute(
            "UPDATE sa_mandates SET revoked=0 WHERE ticket_id=?1",
            [&ticket_id],
        ).unwrap();

        store.db.execute(
            "UPDATE sa_orchestrations SET policy_revision=policy_revision+1
             WHERE id=(SELECT orchestration_id FROM sa_tickets WHERE id=?1)",
            [&ticket_id],
        ).unwrap();
        assert!(side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, true,
        ).is_err(), "changed current policy is denied");

        let audit_count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1
             AND kind IN ('side_effect_consent_enabled','side_effect_consent_revoked')",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_count, 0);
    }

    #[test]
    fn global_pause_denies_new_side_effect_authority_but_not_safe_revocation() {
        let (store, ticket_id) = fixture_with_side_effects(false);
        let ticket = superagent::get_ticket(&store, &ticket_id).unwrap();
        let paused = || true;
        assert!(set_side_effect_consent_with_pause(
            &store, &ticket_id, ticket.revision, true, paused,
        ).unwrap_err().contains("globally paused"));
        let initial: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(initial, 0);

        set_side_effect_consent(&store, &ticket_id, ticket.revision, true).unwrap();
        set_side_effect_consent_with_pause(
            &store, &ticket_id, ticket.revision, false, paused,
        ).unwrap();
        let revoked: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(revoked, 0);
    }

    #[test]
    fn changed_ticket_after_native_confirmation_cannot_enable_side_effects() {
        let (store, ticket_id) = fixture_with_side_effects(false);
        let ticket = superagent::get_ticket(&store, &ticket_id).unwrap();
        assert!(side_effect_consent_for_change(
            &store, &ticket_id, ticket.revision, true,
        ).is_ok(), "the live Ticket can be shown in a native confirmation");
        superagent::stop(&store, &ticket_id).unwrap();
        assert!(set_side_effect_consent(&store, &ticket_id, ticket.revision, true).is_err());
        let enabled: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(enabled, 0);
    }

    #[test]
    fn operator_side_effect_change_waits_on_the_shared_stop_effect_fence() {
        let (store, ticket_id) = fixture_with_side_effects(false);
        let ticket = superagent::get_ticket(&store, &ticket_id).unwrap();
        let path = store.path.clone();
        let fence = lock_dispatch(&path).unwrap();
        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let id = ticket_id.clone();
        let revision = ticket.revision;
        let thread = thread::spawn(move || {
            let store = open_effect_store(&path).unwrap();
            started_tx.send(()).unwrap();
            done_tx.send(set_side_effect_consent(&store, &id, revision, true)).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(done_rx.recv_timeout(Duration::from_millis(75)).is_err(),
            "consent transition must wait while the shared dispatch fence is held");
        let still_denied: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(still_denied, 0);
        drop(fence);
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        thread.join().unwrap();
        let enabled: i64 = store.db.query_row(
            "SELECT allowed_side_effects FROM sa_mandates WHERE ticket_id=?1",
            [&ticket_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(enabled, 0, "consent is recorded separately from the immutable mandate");
    }

    #[test]
    fn issue_comment_authorization_requires_native_side_effect_consent() {
        let (store,ticket_id)=fixture();
        let intent=make_intent(&ticket_id,"Mandate must be checked natively");
        set_side_effect_consent(&store,&ticket_id,0,false).unwrap();
        let error=authorize(&store,&intent).unwrap_err();
        assert!(error.contains("no longer permits"));
        let grants:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_grants WHERE ticket_id=?1",
            [&ticket_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(grants,0,"missing operator consent must not persist a comment grant");
    }

    #[test]
    fn draft_pr_intent_binds_ticket_repository_head_sha_base_title_body_marker_and_key() {
        let intent=make_draft_pr_intent("ticket_1","feature/topic","main","Draft text");
        assert!(intent.body.ends_with(&intent.marker));
        assert!(intent.marker.contains("ticket=ticket_1"));
        assert!(intent.marker.contains(&intent.effect_key));
        let changed_head=make_draft_pr_intent("ticket_1","feature/other","main","Draft text");
        let changed_sha=make_draft_pr_intent_with_sha("ticket_1","feature/topic",&"b".repeat(40),"main","Draft text");
        let changed_base=make_draft_pr_intent("ticket_1","feature/topic","release","Draft text");
        let changed_body=make_draft_pr_intent("ticket_1","feature/topic","main","Changed text");
        assert_ne!(intent.effect_key,changed_head.effect_key);
        assert_eq!(intent.effect_key,changed_sha.effect_key,
            "the stable effect key must keep changed-head retries tied to the original receipt");
        assert_ne!(intent.effect_key,changed_base.effect_key);
        assert_ne!(intent.effect_key,changed_body.effect_key);
        assert_ne!(intent.payload_sha256,changed_head.payload_sha256);
        assert_ne!(intent.payload_sha256,changed_sha.payload_sha256);
        assert!(prepare_draft_pull_request(
            "ticket_1",0,"owner/repo","feature/topic",&intent.head_sha,"main","Draft effect fixture",
            "Draft text","github-draft-pr:wrong",
        ).is_err());
        assert!(prepare_draft_pull_request(
            "ticket_1",0,"owner/repo","feature/topic",&intent.head_sha,"feature/topic","Draft effect fixture",
            "Draft text",&intent.effect_key,
        ).is_err());
        assert!(prepare_draft_pull_request(
            "ticket_1",0,"owner/repo","feature/topic","not-a-sha","main","Draft effect fixture",
            "Draft text",&intent.effect_key,
        ).is_err());
        let confirmation=crate::superagent::draft_pull_request_confirmation(&intent);
        for exact in ["Ticket ticket_1","Repository: owner/repo","Head (already-pushed branch): feature/topic",
            "Verified head SHA: ",&intent.head_sha,"Base: main","Title: Draft effect fixture",
            "Effect key: ",&intent.marker] {
            assert!(confirmation.contains(exact),"confirmation must show {exact}");
        }
    }

    #[test]
    fn draft_pr_authorization_rechecks_mandate_policy_after_target_validation() {
        let (store,ticket_id,root,_,_)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&git_test(&root,&["rev-parse","HEAD"]),"main","Policy check");
        let error=authorize_draft_pull_request_with(&store,&intent,|checked_root,_| {
            assert_eq!(checked_root,root);
            store.db.execute(
                "UPDATE sa_mandates SET policy_revision=policy_revision+1 WHERE ticket_id=?1",
                [&ticket_id],
            ).unwrap();
            Ok(intent.head_sha.clone())
        }).unwrap_err();
        assert!(error.contains("no longer permits"));
        let grants:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_draft_pr_effect_grants WHERE ticket_id=?1",
            [&ticket_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(grants,0,"a policy change during target validation must leave no grant");
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_authorization_requires_explicit_side_effect_consent() {
        let (store,ticket_id,root,_,_)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&git_test(&root,&["rev-parse","HEAD"]),"main","Mandate check");
        set_side_effect_consent(&store,&ticket_id,0,false).unwrap();
        let mut target_verified=false;
        let error=authorize_draft_pull_request_with(&store,&intent,|_,_| {
            target_verified=true;
            Ok(intent.head_sha.clone())
        }).unwrap_err();
        assert!(error.contains("no longer permits"));
        assert!(!target_verified,"the explicit native side-effect consent must be checked before target verification");
        let grants:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_draft_pr_effect_grants WHERE ticket_id=?1",
            [&ticket_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(grants,0,"missing operator consent must leave no PR grant");
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_authorization_rejects_a_head_sha_changed_after_review() {
        let (store,ticket_id,root,head_sha,_)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","Changed head SHA");
        let error=authorize_draft_pull_request_with(&store,&intent,|_,_|Ok("b".repeat(40))).unwrap_err();
        assert!(error.contains("verified remote head SHA changed"));
        let grants:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_draft_pr_effect_grants WHERE ticket_id=?1",
            [&ticket_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(grants,0,"a changed reviewed SHA must not persist a PR grant");
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_target_refuses_another_repository_head_or_missing_base() {
        let (_store,ticket_id,root,head_sha,base_sha)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","Target checks");
        let writes=Arc::new(AtomicUsize::new(0));
        let reads=Arc::new(AtomicUsize::new(0));
        let mut fake_gh=fake_draft_pr_gh(intent.clone(),head_sha,base_sha,vec![],
            writes,reads,false);
        assert_eq!(github::verify_draft_pull_request_target_with(
            &root,"owner/repo","feature/ticket-pr","main",&mut fake_gh,
        ).unwrap(),head_sha);
        assert!(github::verify_draft_pull_request_target_with(
            &root,"other/repo","feature/ticket-pr","main",&mut fake_gh,
        ).is_err());
        assert!(github::verify_draft_pull_request_target_with(
            &root,"owner/repo","unrelated-branch","main",&mut fake_gh,
        ).is_err());
        assert!(github::verify_draft_pull_request_target_with(
            &root,"owner/repo","feature/ticket-pr","missing-base",&mut fake_gh,
        ).is_err());
        drop(_store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_target_refuses_a_dirty_checkout_before_remote_reads() {
        let (_store,ticket_id,root,head_sha,_)=draft_pr_fixture();
        std::fs::write(root.join("untracked-after-review.txt"),"dirty").unwrap();
        let mut gh_calls=0usize;
        let mut fake_gh=|_:&Path,_:&[&str],_:Duration| {
            gh_calls+=1;
            Ok(capture_json(serde_json::json!({"nameWithOwner":"owner/repo"})))
        };
        let result=github::verify_draft_pull_request_target_with(
            &root,"owner/repo","feature/ticket-pr","main",&mut fake_gh,
        );
        assert!(result.unwrap_err().contains("checkout must be clean"));
        assert_eq!(gh_calls,0,"a dirty checkout is refused before calling GitHub");
        drop(_store);
        let _=std::fs::remove_dir_all(root);
        let _=head_sha;
        let _=ticket_id;
    }

    #[test]
    fn draft_pr_uncertain_write_reconciles_exact_readback_without_replay() {
        let (store,ticket_id,root,head_sha,base_sha)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","Reconcile the PR");
        let writes=Arc::new(AtomicUsize::new(0));
        let reads=Arc::new(AtomicUsize::new(0));
        let missing=serde_json::json!([]);
        let exact:serde_json::Value=serde_json::from_slice(&exact_draft_pr_readback(&intent).bytes).unwrap();
        let mut fake_gh=fake_draft_pr_gh(intent.clone(),head_sha,base_sha,vec![missing,exact],
            Arc::clone(&writes),Arc::clone(&reads),true);
        authorize_draft_pull_request_with(&store,&intent,|root,intent|
            github::verify_draft_pull_request_target_with(
                root,&intent.repository,&intent.head,&intent.base,&mut fake_gh,
            )
        ).unwrap();
        let path=store.path.clone();
        let first=create_draft_pull_request_with(&path,&intent,&mut fake_gh).unwrap();
        assert_eq!(first.state,"uncertain");
        assert_eq!(first.reconciliation,"marker-not-found");
        assert_eq!(first.readback_state.as_deref(),Some("missing"));
        assert_eq!(writes.load(Ordering::SeqCst),1);
        let reconciled=create_draft_pull_request_with(&path,&intent,&mut fake_gh).unwrap();
        assert_eq!(reconciled.state,"applied");
        assert_eq!(reconciled.reconciliation,"exact-readback");
        assert_eq!(reconciled.remote_pr_number,Some(86));
        assert_eq!(reconciled.remote_pr_url.as_deref(),Some("https://github.com/owner/repo/pull/86"));
        assert_eq!(writes.load(Ordering::SeqCst),1,"uncertain receipts reconcile by readback and never replay the POST");
        assert_eq!(reads.load(Ordering::SeqCst),2);
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_branch_change_after_review_fails_closed_before_dispatch() {
        let (store,ticket_id,root,head_sha,base_sha)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","No changed branch dispatch");
        authorize_draft_pull_request_with(&store,&intent,|_,intent|Ok(intent.head_sha.clone())).unwrap();
        let stored_sha:String=store.db.query_row(
            "SELECT head_sha FROM sa_draft_pr_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id,intent.effect_key],|row|row.get(0),
        ).unwrap();
        assert_eq!(stored_sha,head_sha,"the reviewed remote SHA is persisted in the native grant");
        git_test(&root,&["commit","--allow-empty","-m","branch changed after review"]);
        let writes=Arc::new(AtomicUsize::new(0));
        let reads=Arc::new(AtomicUsize::new(0));
        let mut fake_gh=fake_draft_pr_gh(intent.clone(),head_sha,base_sha,vec![],
            Arc::clone(&writes),Arc::clone(&reads),false);
        let status=create_draft_pull_request_with(&store.path,&intent,&mut fake_gh).unwrap();
        assert_eq!(status.state,"uncertain");
        assert_eq!(status.reconciliation,"dispatch-not-started");
        assert_eq!(writes.load(Ordering::SeqCst),0,"a branch change after review must not create a PR");
        assert_eq!(reads.load(Ordering::SeqCst),0,"a failed pre-dispatch check must not invent readback evidence");
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn restart_marks_pending_draft_pr_uncertain_and_reconciles_by_readback_only() {
        let (store,ticket_id,root,head_sha,base_sha)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","Restart PR receipt");
        authorize_draft_pull_request_with(&store,&intent,|_,intent|Ok(intent.head_sha.clone())).unwrap();
        assert_eq!(reserve_draft_pull_request(&store,&intent).unwrap(),Reservation::Reserved);
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        let recovered:(String,Option<String>,Option<String>)=reopened.db.query_row(
            "SELECT state,uncertainty_reason,readback_state FROM sa_draft_pr_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id,intent.effect_key],
            |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        assert_eq!(recovered,("uncertain".into(),Some("process_restarted_before_settlement".into()),Some("not_attempted".into())));
        drop(reopened);
        let writes=Arc::new(AtomicUsize::new(0));
        let reads=Arc::new(AtomicUsize::new(0));
        let exact:serde_json::Value=serde_json::from_slice(&exact_draft_pr_readback(&intent).bytes).unwrap();
        let mut fake_gh=fake_draft_pr_gh(intent.clone(),head_sha,base_sha,vec![exact],
            Arc::clone(&writes),Arc::clone(&reads),false);
        let status=create_draft_pull_request_with(&path,&intent,&mut fake_gh).unwrap();
        assert_eq!(status.state,"applied");
        assert_eq!(status.reconciliation,"exact-readback");
        assert_eq!(writes.load(Ordering::SeqCst),0,"restart recovery must not call the PR create endpoint");
        assert_eq!(reads.load(Ordering::SeqCst),1);
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn stop_before_draft_pr_dispatch_allows_only_readback() {
        let (store,ticket_id,root,head_sha,base_sha)=draft_pr_fixture();
        let intent=make_draft_pr_intent_with_sha(&ticket_id,"feature/ticket-pr",&head_sha,"main","Stop before PR");
        authorize_draft_pull_request_with(&store,&intent,|_,intent|Ok(intent.head_sha.clone())).unwrap();
        assert_eq!(reserve_draft_pull_request(&store,&intent).unwrap(),Reservation::Reserved);
        assert_eq!(superagent::stop(&store,&ticket_id).unwrap().0.state,"stopped");
        let path=store.path.clone();
        drop(store);
        let writes=Arc::new(AtomicUsize::new(0));
        let reads=Arc::new(AtomicUsize::new(0));
        let mut fake_gh=fake_draft_pr_gh(intent.clone(),head_sha,base_sha,vec![serde_json::json!([])],
            Arc::clone(&writes),Arc::clone(&reads),false);
        let status=create_draft_pull_request_with(&path,&intent,&mut fake_gh).unwrap();
        assert_eq!(status.state,"uncertain");
        assert_eq!(status.reconciliation,"marker-not-found");
        assert_eq!(writes.load(Ordering::SeqCst),0,"a stopped Ticket must not reach GitHub's PR creation endpoint");
        assert_eq!(reads.load(Ordering::SeqCst),1,"a stopped Ticket may only reconcile the existing receipt");
        let _=std::fs::remove_dir_all(root);
    }

    #[test]
    fn draft_pr_readback_skips_unrelated_pull_requests_with_empty_bodies() {
        let intent=make_draft_pr_intent("ticket_1","feature/topic","main","Readback body");
        let value=serde_json::json!([
            {"number":12,"body":null},
            {"number":13,"body":"","title":"Unrelated"},
            {
                "number":86,
                "html_url":"https://github.com/owner/repo/pull/86",
                "title":intent.title,
                "body":intent.body,
                "draft":true,
                "head":{"ref":intent.head,"sha":intent.head_sha,"repo":{"full_name":intent.repository}},
                "base":{"ref":intent.base,"repo":{"full_name":intent.repository}}
            }
        ]);
        assert!(matches!(draft_pr_readback_state(&value,&intent).unwrap(),DraftPullRequestReadback::Exact(_)));
        let mut wrong_sha=value.clone();
        wrong_sha[2]["head"]["sha"]=serde_json::Value::String("b".repeat(40));
        assert_eq!(draft_pr_readback_state(&wrong_sha,&intent).unwrap(),DraftPullRequestReadback::Conflict,
            "readback must require the exact GitHub head.sha");
    }
}
