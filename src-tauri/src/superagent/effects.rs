use crate::{github, storage::{self, Store}, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

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
    "UPDATE sa_effect_receipts SET state='uncertain',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE state='pending';";

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readback {
    Exact,
    Missing,
    Conflict,
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

fn active_scope(db: &Connection, intent: &IssueCommentIntent) -> Result<(String, String)> {
    if crate::feature_controls::get().bots_paused {
        return Err("Native issue-comment authorization is unavailable while Bots are paused.".into());
    }
    db.query_row(
        "SELECT t.project_id,a.session_id FROM sa_tickets t
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
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "The Ticket, revision, mandate, or active worker no longer permits this effect.".into())
}

fn exact_grant(
    db: &Connection,
    intent: &IssueCommentIntent,
) -> Result<(String, String)> {
    let (project_id, session_id) = active_scope(db, intent)?;
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
    let Some((revision, grant_project, repository, issue, body, digest)) = grant else {
        return Err("The exact native effect grant is missing.".into());
    };
    if revision != intent.expected_revision
        || grant_project != project_id
        || repository != intent.repository
        || issue != intent.issue_number
        || body != intent.comment_body
        || digest != intent.payload_sha256
    {
        return Err("The native effect grant does not match this exact Ticket, target, key, and payload.".into());
    }
    Ok((project_id, session_id))
}

pub(crate) fn has_exact_grant(store: &Store, intent: &IssueCommentIntent) -> Result<bool> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    active_scope(&tx, intent)?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT payload_sha256 FROM sa_effect_grants WHERE ticket_id=?1 AND effect_key=?2",
            params![intent.ticket_id, intent.effect_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let present = match existing {
        None => false,
        Some(digest) if digest == intent.payload_sha256 => {
            exact_grant(&tx, intent)?;
            true
        }
        Some(_) => return Err("Effect key already has a grant for a different exact payload.".into()),
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(present)
}

pub(crate) fn authorize(store: &Store, intent: &IssueCommentIntent) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let (project_id, _) = active_scope(&tx, intent)?;
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
            project_id,
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
    exact_grant(&tx, intent)?;
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
        Some((_, state)) => match state.as_str() {
            "pending" => Reservation::AlreadyPending,
            "uncertain" => Reservation::RequiresReconciliation,
            "applied" => Reservation::AlreadyApplied,
            _ => return Err("Invalid effect state.".into()),
        },
        None => {
            let now = storage::now();
            tx.execute(
                "INSERT INTO sa_effect_receipts(ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'pending',?4,?4)",
                params![intent.ticket_id, intent.effect_key, intent.payload_sha256, now],
            )
            .map_err(|e| e.to_string())?;
            Reservation::Reserved
        }
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

fn validate_pending_dispatch(store: &Store, intent: &IssueCommentIntent) -> Result<(String, String)> {
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

fn mark_uncertain(store: &Store, intent: &IssueCommentIntent) -> Result<()> {
    let n = store
        .db
        .execute(
            "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?4
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3 AND state='pending'",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256, storage::now()],
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
    if state.as_deref() == Some("uncertain") {
        Ok(())
    } else {
        Err("No unresolved matching effect receipt.".into())
    }
}

fn settle_applied(store: &Store, intent: &IssueCommentIntent) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    exact_grant(&tx, intent)?;
    let changed = tx
        .execute(
            "UPDATE sa_effect_receipts SET state='applied',updated_at=?4
             WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
               AND state IN ('pending','uncertain')",
            params![intent.ticket_id, intent.effect_key, intent.payload_sha256, storage::now()],
        )
        .map_err(|e| e.to_string())?;
    if changed != 1 {
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

fn readback_state(value: &serde_json::Value, intent: &IssueCommentIntent) -> Result<Readback> {
    let comments = value
        .get("comments")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "GitHub did not return issue comments.".to_string())?;
    let marker = intent_marker(intent);
    let mut found = Vec::new();
    for comment in comments {
        let body = comment
            .get("body")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "GitHub returned an unreadable issue comment.".to_string())?;
        if body.contains(&marker) {
            found.push(body);
        }
    }
    Ok(match found.as_slice() {
        [] => Readback::Missing,
        [body] if *body == intent.comment_body => Readback::Exact,
        _ => Readback::Conflict,
    })
}

fn reconcile_unresolved(
    store: &Store,
    intent: &IssueCommentIntent,
    root: &Path,
) -> Result<EffectStatus> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    exact_grant(&tx, intent)?;
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
        return Ok(status(intent, "applied", "exact-readback"));
    }
    if !matches!(state.as_str(), "pending" | "uncertain") {
        return Err("Invalid effect state.".into());
    }
    tx.commit().map_err(|e| e.to_string())?;

    let issue = intent.issue_number.to_string();
    let repo_flag = github::flag("repo", &intent.repository);
    let capture = github::run_gh(
        root,
        &["issue", "view", &issue, &repo_flag, "--json=comments"],
        github::TIMEOUT,
    );
    let observed = capture
        .and_then(|capture| github::json(&capture))
        .and_then(|value| readback_state(&value, intent));
    match observed {
        Ok(Readback::Exact) => {
            if let Err(error) = settle_applied(store, intent) {
                let _ = mark_uncertain(store, intent);
                return Err(error);
            }
            Ok(status(intent, "applied", "exact-readback"))
        }
        Ok(Readback::Missing) => {
            mark_uncertain(store, intent)?;
            Ok(status(intent, "uncertain", "marker-not-found"))
        }
        Ok(Readback::Conflict) => {
            mark_uncertain(store, intent)?;
            Ok(status(intent, "uncertain", "marker-conflict"))
        }
        Err(_) => {
            mark_uncertain(store, intent)?;
            Ok(status(intent, "uncertain", "readback-unavailable"))
        }
    }
}

pub(crate) fn post_issue_comment(database_path: &Path, intent: &IssueCommentIntent) -> Result<EffectStatus> {
    let store = open_effect_store(database_path)?;
    let (project_id, session_id) = {
        let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let scope = exact_grant(&tx, intent)?;
        tx.commit().map_err(|e| e.to_string())?;
        scope
    };
    let root = store.workspace_root(&project_id, Some(&session_id))?;
    match reserve(&store, intent)? {
        Reservation::AlreadyApplied => Ok(status(intent, "applied", "exact-readback")),
        Reservation::AlreadyPending => Ok(status(intent, "pending", "dispatch-in-progress")),
        Reservation::RequiresReconciliation => reconcile_unresolved(&store, intent, &root),
        Reservation::Reserved => {
            if let Err(error) = validate_pending_dispatch(&store, intent) {
                let _ = mark_uncertain(&store, intent);
                return Err(error);
            }
            let issue = intent.issue_number.to_string();
            let repo_flag = github::flag("repo", &intent.repository);
            let body_flag = github::flag("body", &intent.comment_body);
            match github::run_gh(
                &root,
                &["issue", "comment", &issue, &repo_flag, &body_flag],
                github::TIMEOUT,
            ) {
                Ok(_) => {
                    let result = reconcile_unresolved(&store, intent, &root);
                    if result.is_err() {
                        let _ = mark_uncertain(&store, intent);
                    }
                    result
                }
                Err(_) => {
                    mark_uncertain(&store, intent)?;
                    let result = reconcile_unresolved(&store, intent, &root);
                    if result.is_err() {
                        let _ = mark_uncertain(&store, intent);
                    }
                    result
                }
            }
        }
    }
}

fn status(intent: &IssueCommentIntent, state: &str, reconciliation: &str) -> EffectStatus {
    EffectStatus {
        ticket_id: intent.ticket_id.clone(),
        ticket_revision: intent.expected_revision,
        repository: intent.repository.clone(),
        issue_number: intent.issue_number,
        effect_key: intent.effect_key.clone(),
        payload_sha256: intent.payload_sha256.clone(),
        state: state.into(),
        reconciliation: reconciliation.into(),
    }
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

    fn fixture() -> (Store, String) {
        let path = std::env::temp_dir().join(format!("sa-effect-{}.db", storage::id()));
        let mut store = Store::open(&path).unwrap();
        store
            .db
            .execute("INSERT INTO projects(id,name,path) VALUES('p','P','/tmp/sa-effects')", [])
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
        mark_uncertain(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::RequiresReconciliation);
        settle_applied(&store, &intent).unwrap();
        assert_eq!(reserve(&store, &intent).unwrap(), Reservation::AlreadyApplied);
        assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        store
            .db
            .execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&id])
            .unwrap();
        assert!(reserve(&store, &intent).is_err());
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
    fn restart_marks_reserved_effect_uncertain_without_replay() {
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
        assert_eq!(reserve(&reopened, &intent).unwrap(), Reservation::RequiresReconciliation);
        drop(reopened);
        let again = Store::open(&path).unwrap();
        assert_eq!(unresolved_count(&again, &id).unwrap(), 1);
    }

    #[test]
    fn readback_accepts_only_one_exact_marker_and_body() {
        let intent = make_intent("ticket_1", "comment text");
        let exact = serde_json::json!({"comments":[{"body":intent.comment_body}]});
        assert_eq!(readback_state(&exact, &intent).unwrap(), Readback::Exact);
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
