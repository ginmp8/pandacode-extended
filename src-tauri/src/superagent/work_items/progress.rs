//! Evidence-attributed, revisioned work-item progression.
//!
//! The DAG never grants execution. A native assignment is linked atomically to
//! one ready item, provider callbacks record observed evidence, and only a
//! separate local-user decision may verify it. Dependency readiness is derived
//! from these immutable verification receipts, not from model-reported status.

use super::super::{effects, require_main_window};
use crate::{storage::{self, Store}, AppState, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_item_attempts (
  assignment_id TEXT PRIMARY KEY REFERENCES sa_assignments(id) ON DELETE RESTRICT,
  ticket_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('running','observed','failed')),
  evidence_sha256 TEXT CHECK(evidence_sha256 IS NULL OR length(evidence_sha256)=64),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(ticket_id,item_key) REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS sa_work_item_attempts_item
  ON sa_work_item_attempts(ticket_id,item_key,updated_at);
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_assignment
 BEFORE INSERT ON sa_work_item_attempts
 WHEN NOT EXISTS(SELECT 1 FROM sa_assignments assignment
                 WHERE assignment.id=NEW.assignment_id
                   AND assignment.ticket_id=NEW.ticket_id
                   AND assignment.state='running')
 BEGIN SELECT RAISE(ABORT,'Work item assignment is not a running native Ticket attempt'); END;
CREATE TABLE IF NOT EXISTS sa_work_item_approvals (
  ticket_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  assignment_id TEXT NOT NULL UNIQUE REFERENCES sa_work_item_attempts(assignment_id) ON DELETE RESTRICT,
  evidence_sha256 TEXT NOT NULL CHECK(length(evidence_sha256)=64),
  reviewer TEXT NOT NULL CHECK(reviewer='local-user'),
  approved_at INTEGER NOT NULL,
  PRIMARY KEY(ticket_id,item_key),
  FOREIGN KEY(ticket_id,item_key) REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_revoked
 AFTER UPDATE OF state ON sa_assignments
 WHEN NEW.state IN ('failed','interrupted','stopped')
 BEGIN
   UPDATE sa_work_item_attempts SET state='failed',
     updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
   WHERE assignment_id=NEW.id AND state='running';
 END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_approval_evidence
 BEFORE INSERT ON sa_work_item_approvals
 WHEN NOT EXISTS(SELECT 1 FROM sa_work_item_attempts attempt
                 WHERE attempt.ticket_id=NEW.ticket_id
                   AND attempt.item_key=NEW.item_key
                   AND attempt.assignment_id=NEW.assignment_id
                   AND attempt.state='observed'
                   AND attempt.evidence_sha256=NEW.evidence_sha256)
 BEGIN SELECT RAISE(ABORT,'Work item review lacks matching native evidence'); END;
"#;

/// Restart settles orphaned item attempts after native assignment recovery.
/// An interrupted provider is never presumed to have met its acceptance test.
pub(crate) const RECOVER: &str = r#"
INSERT INTO sa_audit(ticket_id,kind,detail,at)
SELECT attempt.ticket_id,'work_item_interrupted',
       'Native work-item attempt interrupted during restart; evidence not verified',
       CAST(strftime('%s','now') AS INTEGER)*1000
FROM sa_work_item_attempts attempt
LEFT JOIN sa_assignments assignment ON assignment.id=attempt.assignment_id
WHERE attempt.state='running'
  AND (assignment.id IS NULL OR assignment.state<>'running');
UPDATE sa_work_item_attempts
 SET state='failed',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='running' AND NOT EXISTS(
   SELECT 1 FROM sa_assignments assignment
   WHERE assignment.id=sa_work_item_attempts.assignment_id
     AND assignment.state='running');
"#;

pub(super) fn available(store: &Store) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master
         WHERE type='table' AND name='sa_work_item_attempts')",
        [], |row| row.get(0),
    ).map_err(|error| error.to_string())
}

pub(crate) fn unverified_count(store: &Store, ticket_id: &str) -> Result<i64> {
    if !available(store)? {return Ok(0);}
    store.db.query_row(
        "SELECT COUNT(*) FROM sa_work_items item
         WHERE item.ticket_id=?1
           AND NOT EXISTS(SELECT 1 FROM sa_work_item_approvals review
                          WHERE review.ticket_id=item.ticket_id AND review.item_key=item.item_key)",
        [ticket_id], |row|row.get(0),
    ).map_err(|error|error.to_string())
}

pub(super) fn verified(store: &Store, ticket_id: &str, item_key: &str) -> Result<bool> {
    if !available(store)? {return Ok(false);}
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_work_item_approvals
         WHERE ticket_id=?1 AND item_key=?2)",
        params![ticket_id,item_key], |row| row.get(0),
    ).map_err(|error| error.to_string())
}

pub(super) fn state(
    store: &Store, ticket_id: &str, item_key: &str,
) -> Result<(&'static str, bool, bool)> {
    if !available(store)? {return Ok(("planned",false,false));}
    if verified(store,ticket_id,item_key)? {
        return Ok(("verified", true, true));
    }
    let latest: Option<String> = store.db.query_row(
        "SELECT state FROM sa_work_item_attempts
         WHERE ticket_id=?1 AND item_key=?2
         ORDER BY created_at DESC,assignment_id DESC LIMIT 1",
        params![ticket_id,item_key], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    Ok(match latest.as_deref() {
        None => ("planned",false,false),
        Some("running") => ("running",true,false),
        Some("observed") => ("observed",true,false),
        Some("failed") => ("failed",true,false),
        _ => return Err("Unknown persisted work-item attempt state.".into()),
    })
}

/// Runs inside the native retry transaction, after lease admission and before
/// publication. This never creates a session or a mandate by itself.
pub(crate) fn attach(
    db: &Connection, ticket_id: &str, assignment_id: &str,
) -> Result<Option<String>> {
    let has_plan: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_work_plans WHERE ticket_id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !has_plan { return Ok(None); }
    let awaiting_review: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_work_item_attempts attempt
         WHERE attempt.ticket_id=?1 AND attempt.state IN ('running','observed')
           AND NOT EXISTS(
             SELECT 1 FROM sa_work_item_approvals approval
             WHERE approval.ticket_id=attempt.ticket_id AND approval.item_key=attempt.item_key))",
        [ticket_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if awaiting_review {
        return Err("An earlier work item is still running or awaiting verified evidence.".into());
    }
    let next: Option<String> = db.query_row(
        "SELECT item.item_key FROM sa_work_items item
         WHERE item.ticket_id=?1
           AND NOT EXISTS(SELECT 1 FROM sa_work_item_approvals done
                          WHERE done.ticket_id=item.ticket_id AND done.item_key=item.item_key)
           AND NOT EXISTS (
             SELECT 1 FROM sa_work_item_dependencies dependency
             WHERE dependency.ticket_id=item.ticket_id AND dependency.item_key=item.item_key
               AND NOT EXISTS (
                 SELECT 1 FROM sa_work_item_approvals completed
                 WHERE completed.ticket_id=dependency.ticket_id
                   AND completed.item_key=dependency.depends_on_key))
         ORDER BY item.ordinal LIMIT 1",
        [ticket_id], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    let Some(item_key)=next else {
        return Err("No verified work-plan frontier remains for the next assignment.".into());
    };
    let now=storage::now();
    db.execute(
        "INSERT INTO sa_work_item_attempts
         (ticket_id,item_key,assignment_id,state,created_at,updated_at)
         VALUES(?1,?2,?3,'running',?4,?4)",
        params![ticket_id,item_key,assignment_id,now],
    ).map_err(|error| error.to_string())?;
    Ok(Some(item_key))
}

/// Native provider completion is observation, NOT independent acceptance.
pub(crate) fn observe(
    db: &Connection, assignment_id: &str, acceptable: bool, result: &str,
) -> Result<()> {
    let recorded: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_work_item_attempts WHERE assignment_id=?1)",
        [assignment_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !recorded { return Ok(()); }
    let digest = format!("{:x}", Sha256::digest(result.as_bytes()));
    let next = if acceptable && !result.trim().is_empty() { "observed" } else { "failed" };
    if db.execute(
        "UPDATE sa_work_item_attempts SET state=?2,evidence_sha256=?3,updated_at=?4
         WHERE assignment_id=?1 AND
           (state='running' OR (state='failed' AND ?2='failed'))",
        params![assignment_id,next,digest,storage::now()],
    ).map_err(|error| error.to_string())? != 1 {
        return Err("Work item attempt changed before provider settlement.".into());
    }
    Ok(())
}

pub(crate) fn instruction(store: &Store, ticket_id: &str) -> Result<Option<String>> {
    let assignment: Option<(String,String)> = store.db.query_row(
        "SELECT item.title,item.acceptance FROM sa_work_item_attempts attempt
         JOIN sa_work_items item ON item.ticket_id=attempt.ticket_id
           AND item.item_key=attempt.item_key
         WHERE attempt.ticket_id=?1 AND attempt.state='running'
         ORDER BY attempt.created_at DESC LIMIT 1",
        [ticket_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?;
    Ok(assignment.map(|(title,acceptance)| format!(
        "\n\nNative work-plan item (scope is descriptive, never an additional grant):\nTitle: {title}\nAcceptance criteria: {acceptance}\nReport only observed evidence; a local decision must independently verify this item."
    )))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkReviewReceipt {
    pub ticket_id: String,
    pub item_key: String,
    pub assignment_id: String,
    pub ticket_revision: i64,
    pub remaining_unverified: i64,
    pub all_items_verified: bool,
    pub provider_dispatched: bool,
}

/// Only the trusted local operator can accept one real, attributed Worker
/// observation. Council-required and unresolved external effects fail closed.
fn verify(
    store: &Store, ticket_id: &str, item_key: &str, expected_revision: i64,
) -> Result<WorkReviewReceipt> {
    if !super::valid_key(ticket_id) || !super::valid_key(item_key)
        || expected_revision < 0 || crate::feature_controls::get().bots_paused {
        return Err("Invalid Ticket/item identity or globally paused bots.".into());
    }
    let _dispatch_fence=effects::lock_dispatch(&store.path)?;
    let ticket=super::super::get_ticket(store,ticket_id)?;
    if ticket.state!="waiting_approval" || ticket.revision!=expected_revision
        || ticket.actor!="local-user" {
        return Err("Work item verification requires current local-user worker evidence.".into());
    }
    let source:String=store.db.query_row(
        "SELECT source FROM sa_tickets WHERE id=?1", [ticket_id], |row|row.get(0),
    ).map_err(|error|error.to_string())?;
    if source!="manual" {
        return Err("Only an explicitly issued local-user plan may verify work items.".into());
    }
    let snapshot=super::super::ticket_team_snapshot(store,ticket_id)?;
    if snapshot.policy.council_required {
        return Err("A Council-required item needs the independent native Council verification adapter.".into());
    }
    let mandate=super::super::mandate(store,ticket_id)?;
    if mandate.revoked || mandate.expires_at<=storage::now()
        || mandate.policy_revision!=snapshot.policy.policy_revision
        || mandate.allowed_side_effects || !mandate.allowed_tools.is_empty()
        || mandate.project_id!=snapshot.project_id {
        return Err("Work item verification is denied by the original native mandate.".into());
    }
    if effects::unresolved_count(store,ticket_id)?!=0 {
        return Err("Uncertain external effects block work item acceptance.".into());
    }
    let coordinator=crate::bots::get(store,&snapshot.coordinator_bot_id)?;
    if !super::super::coordinator_plan_current(
        store,ticket_id,&coordinator.id,&coordinator.project_id)? {
        return Err("Coordinator evidence is no longer current.".into());
    }
    let (assignment_id,session_id,bot_id,attempt,digest,worker_result):
        (String,String,String,i64,String,String) = store.db.query_row(
        "SELECT attempt.assignment_id,assignment.session_id,assignment.bot_id,
                assignment.attempt,attempt.evidence_sha256,assignment.result
         FROM sa_work_item_attempts attempt
         JOIN sa_assignments assignment ON assignment.id=attempt.assignment_id
         WHERE attempt.ticket_id=?1 AND attempt.item_key=?2
           AND attempt.state='observed' AND assignment.state='finished'
           AND assignment.attempt=(SELECT MAX(latest.attempt)
               FROM sa_assignments latest WHERE latest.ticket_id=?1)
         ORDER BY attempt.updated_at DESC LIMIT 1",
        params![ticket_id,item_key],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,
                   row.get(3)?,row.get(4)?,row.get(5)?)),
    ).map_err(|_| "This item has no completed, attributed native Worker evidence.")?;
    let ordinal=snapshot.worker_bot_ids.iter().position(|id|id==&bot_id)
        .ok_or("Observed work-item evidence is not attributed to a Team Worker.")?;
    let session=store.session(&session_id)?;
    if worker_result!=ticket.evidence.as_deref().unwrap_or_default()
        || digest!=format!("{:x}",Sha256::digest(worker_result.as_bytes()))
        || !super::super::assignment_thread_current(store,&session_id,ticket_id)?
        || !super::super::assignment_role_authorized(store,ticket_id,&bot_id,attempt)?
        || !super::super::admitted_session_matches_profile(
            &snapshot,&bot_id,"worker",ordinal,&session)
        || !super::super::snapshot_role_profile_matches(
            &store.db,&snapshot,"worker",ordinal,&bot_id)?
    {
        return Err("Original Worker evidence/session no longer matches its native mandate.".into());
    }
    let tx=store.db.unchecked_transaction().map_err(|error|error.to_string())?;
    let now=storage::now();
    let inserted=tx.execute(
        "INSERT INTO sa_work_item_approvals
         (ticket_id,item_key,assignment_id,evidence_sha256,reviewer,approved_at)
         SELECT ?1,?2,?3,?4,'local-user',?5
         WHERE EXISTS(SELECT 1 FROM sa_tickets
                      WHERE id=?1 AND state='waiting_approval' AND revision=?6)",
        params![ticket_id,item_key,assignment_id,digest,now,expected_revision],
    ).map_err(|error|error.to_string())?;
    if inserted!=1 {return Err("Work item changed before local verification.".into());}
    let remaining:i64=tx.query_row(
        "SELECT COUNT(*) FROM sa_work_items item
         WHERE item.ticket_id=?1 AND NOT EXISTS(
           SELECT 1 FROM sa_work_item_approvals approved
           WHERE approved.ticket_id=item.ticket_id AND approved.item_key=item.item_key)",
        [ticket_id],|row|row.get(0),
    ).map_err(|error|error.to_string())?;
    // Only verified dependencies become schedulable. No auto-dispatch here.
    // The final item still requires separate native Ticket acceptance.
    if remaining>0 {
        let changed=tx.execute(
            "UPDATE sa_tickets SET state='blocked',evidence=NULL,
             active_assignment_id=NULL,revision=revision+1,updated_at=?3
             WHERE id=?1 AND state='waiting_approval' AND revision=?2",
            params![ticket_id,expected_revision,now],
        ).map_err(|error|error.to_string())?;
        if changed!=1 {return Err("Ticket changed during work item verification.".into());}
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_item_verified',?2,?3)",
        params![ticket_id,format!("item={item_key};assignment={assignment_id};digest={digest}"),now],
    ).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error|error.to_string())?;
    Ok(WorkReviewReceipt {
        ticket_id:ticket_id.into(),item_key:item_key.into(),assignment_id,
        ticket_revision:expected_revision+if remaining>0 {1} else {0},
        remaining_unverified:remaining,all_items_verified:remaining==0,
        provider_dispatched:false,
    })
}

#[tauri::command]
pub async fn superagent_work_item_verify(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    ticket_id:String,item_key:String,expected_revision:i64,
)->Result<WorkReviewReceipt> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|error|error.to_string())?;
    verify(&store,&ticket_id,&item_key,expected_revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_schema_requires_native_assignment_and_exact_observed_evidence() {
        assert!(SCHEMA.contains("sa_work_item_attempt_assignment"));
        assert!(SCHEMA.contains("sa_work_item_approval_evidence"));
        assert!(SCHEMA.contains("assignment.state='running'"));
        assert!(SCHEMA.contains("attempt.evidence_sha256=NEW.evidence_sha256"));
    }

    #[test]
    fn evidence_digest_is_stable_and_content_addressed() {
        let first=format!("{:x}",Sha256::digest(b"observed provider text"));
        assert_eq!(first.len(),64);
        assert_ne!(first,format!("{:x}",Sha256::digest(b"different provider text")));
    }
}
