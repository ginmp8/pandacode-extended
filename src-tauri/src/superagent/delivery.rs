//! Read-only, bounded engineering delivery evidence. A Git observation does
//! not prove a task authored code, ran tests, passed CI or delivered a PR.
//! Never copy raw diffs, local paths, secrets or repository content into the UI.

use super::*;
use rusqlite::OpenFlags;
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{path::{Path,PathBuf},time::Duration};

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DeliveryObservation {
    pub ticket_id:String,
    pub ticket_revision:i64,
    pub ticket_state:String,
    pub delivery_state:String,
    pub total_work_items:i64,
    pub verified_work_items:i64,
    pub unresolved_external_effects:i64,
    pub applied_scoped_file_writes:i64,
    pub unresolved_scoped_file_writes:i64,
    pub repository_head_sha:Option<String>,
    pub worktree_status_sha256:Option<String>,
    pub worktree_diff_sha256:Option<String>,
    pub observed_status_entries:Option<usize>,
    pub checkout_observation:String,
    pub source_attribution:String,
    pub native_test_required:bool,
    pub native_test_state:String,
    pub test_execution_verified:bool,
    pub ci_result_verified:bool,
    pub external_delivery_verified:bool,
    pub checked_at:i64,
    pub read_only:bool,
}

/// A native Ticket and a verified WorkItem plan are distinct from a verified
/// engineering delivery. Never label model-only output or a clean Git HEAD
/// as "Delivered" when no test, CI or publication receipt exists.
fn classify(state:&str,total:i64,verified:i64,unresolved:i64)->&'static str {
    match state {
        "failed"|"stopped"|"interrupted"|"inconclusive"|"blocked"=>"blocked",
        "waiting_approval"|"waiting_review"=>"awaiting_native_review",
        "completed" if unresolved==0 && (total==0 || verified==total)
            =>"native_complete_delivery_unverified",
        _ if verified>0=>"partially_verified",
        _=>"in_progress",
    }
}

/// Read only the durable native test state for this specific Demand.
fn native_test_state(db:&Connection,ticket_id:&str)->Result<&'static str> {
    let stored:Option<Option<String>>=db.query_row(
        "SELECT run.state FROM sa_demand_test_grants grant
         LEFT JOIN sa_demand_test_runs run ON run.ticket_id=grant.ticket_id
         WHERE grant.ticket_id=?1",
        [ticket_id],|row|row.get(0),
    ).optional().map_err(|e|e.to_string())?;
    match stored {
        None=>Ok("not_authorized"),
        Some(None)=>Ok("authorized_not_run"),
        Some(Some(state))=>match state.as_str() {
            "running"=>Ok("running"),
            "passed"=>Ok("passed"),
            "failed"=>Ok("failed"),
            "uncertain"=>Ok("uncertain"),
            _=>Err("Stored native test state is invalid.".into()),
        },
    }
}

/// Only the latest finished native Worker attempt with a matching independent
/// approval counts as verified. Immutable historical receipts are not current
/// after a retry, interruption or changed native output.
fn verified_work_items(db:&Connection,ticket_id:&str)->Result<i64> {
    // An immutable receipt only proves the original Worker result. Re-check
    // its digest and the final assignment's applied write before presenting
    // it as current evidence after recovery, retries or effect reconciliation.
    let mut query=db.prepare(
        "SELECT proof.evidence_sha256,child.evidence
         FROM sa_work_item_evidence_receipts proof
         JOIN sa_work_items item ON item.id=proof.item_id
         JOIN sa_tickets child ON child.id=proof.child_ticket_id
         JOIN sa_assignments assignment ON assignment.id=proof.assignment_id
         WHERE item.ticket_id=?1 AND item.state='completed'
           AND item.child_ticket_id=proof.child_ticket_id
           AND child.state='completed'
           AND assignment.ticket_id=child.id AND assignment.state='finished'
           AND assignment.result=child.evidence
           AND assignment.attempt=(SELECT MAX(latest.attempt)
             FROM sa_assignments latest WHERE latest.ticket_id=child.id)
           AND proof.reviewed_at>=assignment.updated_at
           AND (item.requires_write=0 OR proof.approval_kind='native_worker_finished'
                OR EXISTS(
             SELECT 1 FROM sa_worker_write_receipts applied
             WHERE applied.ticket_id=child.id
               AND applied.assignment_id=assignment.id
               AND applied.state='applied'))"
    ).map_err(|error|error.to_string())?;
    let rows=query.query_map([ticket_id],|row|Ok((
        row.get::<_,String>(0)?,row.get::<_,String>(1)?
    ))).map_err(|error|error.to_string())?;
    let candidates=rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error|error.to_string())?;
    Ok(candidates.into_iter().filter(|(digest,evidence)| {
        format!("{:x}",Sha256::digest(evidence.as_bytes()))==digest.as_str()
    }).count() as i64)
}

/// Count only durable native effects in this Demand, including its WorkItem
/// children and every admitted nested subagent. A Git status is never proof.
fn demand_receipts(db:&Connection,ticket_id:&str)->Result<(i64,i64,i64)> {
    db.query_row(
        "WITH RECURSIVE descendants(ticket_id) AS (
           SELECT ?1
           UNION
           SELECT item.child_ticket_id FROM sa_work_items item
           WHERE item.ticket_id=?1 AND item.child_ticket_id IS NOT NULL
           UNION
           SELECT child.child_ticket_id FROM sa_subagent_nodes child
           JOIN descendants parent ON parent.ticket_id=child.parent_ticket_id
           WHERE child.child_ticket_id IS NOT NULL
         )
         SELECT
           (SELECT COUNT(*) FROM sa_effect_receipts effect
            JOIN descendants d ON d.ticket_id=effect.ticket_id
            WHERE effect.state IN ('pending','uncertain')),
           (SELECT COUNT(*) FROM sa_worker_write_receipts receipt
            JOIN descendants d ON d.ticket_id=receipt.ticket_id
            WHERE receipt.state='applied'),
           (SELECT COUNT(*) FROM sa_worker_write_receipts receipt
            JOIN descendants d ON d.ticket_id=receipt.ticket_id
            WHERE receipt.state IN ('reserved','uncertain'))",
        [ticket_id],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).map_err(|e|e.to_string())
}

fn git_read(root:&Path,args:&[&str],limit:usize)->Result<Vec<u8>> {
    let output=crate::workspace::run_git_bounded(
        root,args,limit,false,Duration::from_secs(10),
    ).map_err(|_|"Native read-only Git observation unavailable.".to_owned())?;
    if !output.status.success() || output.stdout.truncated {
        return Err("Native Git inspection failed or exceeded its byte limit.".into());
    }
    Ok(output.stdout.bytes)
}

pub(super) fn observe_git(root:&Path)->Result<(String,String,String,usize)> {
    let expected=std::fs::canonicalize(root)
        .map_err(|_|"Configured checkout is unavailable.".to_owned())?;
    let actual=git_read(&expected,&["rev-parse","--show-toplevel"],4096)?;
    let base=std::str::from_utf8(&actual).map_err(|_|"Git root is not UTF-8.")?.trim();
    if std::fs::canonicalize(base).map_err(|_|"Git root is unavailable.")?!=expected {
        return Err("The active Git repository root differs from the pinned checkout.".into());
    }
    let sha=git_read(&expected,&["rev-parse","--verify","HEAD"],128)?;
    let sha=std::str::from_utf8(&sha).map_err(|_|"Git HEAD is not UTF-8.")?.trim();
    if ![40,64].contains(&sha.len()) || !sha.bytes().all(|ch|ch.is_ascii_hexdigit()) {
        return Err("Git HEAD is not a bounded commit digest.".into());
    }
    let status=git_read(&expected,&[
        "status","--porcelain=v1","-z","--untracked-files=all",
    ],256*1024)?;
    let diff=git_read(&expected,&[
        "diff","--no-ext-diff","--no-textconv","--no-color",
        "--binary","HEAD","--",
    ],256*1024)?;
    let status_entries=status.split(|b|*b==0).filter(|entry|!entry.is_empty()).count();
    let digest=|bytes:&[u8]|format!("{:x}",Sha256::digest(bytes));
    Ok((sha.to_owned(),digest(&status),digest(&diff),status_entries))
}

fn native_observation(store:&Store,ticket_id:&str)->Result<(DeliveryObservation,Option<PathBuf>)> {
    let ticket=super::get_ticket(store,ticket_id)?;
    let snapshot=super::ticket_team_snapshot(store,ticket_id)?;
    let total:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_work_items WHERE ticket_id=?1",[ticket_id],
        |row|row.get(0),
    ).map_err(|e|e.to_string())?;
    let verified=verified_work_items(&store.db,ticket_id)?;
    let (unresolved,applied_file_writes,unresolved_file_writes)=
        demand_receipts(&store.db,ticket_id)?;
    let path:Option<String>=store.db.query_row(
        "SELECT path FROM projects WHERE id=?1",
        [&snapshot.project_id],|row|row.get(0),
    ).optional().map_err(|e|e.to_string())?;
    let pinned=snapshot.roles.iter()
        .filter(|role|role.role=="worker")
        .find_map(|role|role.project_path.as_ref());
    // Historical Team snapshots have no source-path attestation. Do not
    // substitute mutable project metadata as evidence of that original scope.
    let allowed=path.as_ref().zip(pinned).is_some_and(|(actual,admitted)|
        !actual.trim().is_empty() && actual==admitted);
    let result=DeliveryObservation{
        ticket_id:ticket.id,ticket_revision:ticket.revision,
        ticket_state:ticket.state.clone(),
        delivery_state:classify(&ticket.state,total,verified,
            unresolved.saturating_add(unresolved_file_writes)).into(),
        total_work_items:total,verified_work_items:verified,
        unresolved_external_effects:unresolved,
        applied_scoped_file_writes:applied_file_writes,
        unresolved_scoped_file_writes:unresolved_file_writes,
        repository_head_sha:None,worktree_status_sha256:None,
        worktree_diff_sha256:None,observed_status_entries:None,
        checkout_observation:if allowed {"pending"} else {"unavailable_or_unpinned"}.into(),
        source_attribution:(if applied_file_writes>0 {
            "native_scoped_file_write_receipts_only"
        } else {
            "not_attributed_to_ticket"
        }).into(),
        native_test_required:super::scheduler::test_runner::required(&store.db,ticket_id)?,
        native_test_state:native_test_state(&store.db,ticket_id)?.into(),
        test_execution_verified:super::scheduler::test_runner::verified(&store.db,ticket_id)?,
        ci_result_verified:false,
        external_delivery_verified:false,
        checked_at:storage::now(),read_only:true,
    };
    let root=if allowed {path.map(PathBuf::from)} else {None};
    Ok((result,root))
}

fn apply_git_observation(result:&mut DeliveryObservation,root:Option<PathBuf>) {
    if let Some(path)=root {
        match observe_git(&path) {
            Ok((head,status,diff,entries))=>{
                result.repository_head_sha=Some(head);
                result.worktree_status_sha256=Some(status);
                result.worktree_diff_sha256=Some(diff);
                result.observed_status_entries=Some(entries);
                result.checkout_observation="observed_unattributed".into();
            }
            Err(_)=>{result.checkout_observation="git_unavailable_or_incomplete".into();}
        }
    }
}

pub(super) fn inspect(store:&Store,ticket_id:&str)->Result<DeliveryObservation> {
    let (mut observation,root)=native_observation(store,ticket_id)?;
    apply_git_observation(&mut observation,root);
    Ok(observation)
}

#[tauri::command]
pub async fn superagent_delivery_inspect(
    window:tauri::WebviewWindow,state:State<'_,AppState>,ticket_id:String,
)->Result<DeliveryObservation> {
    super::require_main_window(&window)?;
    // Native store is locked only for durable receipt reads; the separate
    // Git observation never holds the agent/SQLite runtime mutex.
    let (mut observation,root)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        native_observation(&store,&ticket_id)?
    };
    // The external bounded Git inspection happens AFTER releasing the native
    // agent/SQLite lock. It is never invoked automatically by a background poll.
    apply_git_observation(&mut observation,root);
    Ok(observation)
}

/// One-shot headless inspection opens SQLite read-only and never runs startup
/// recovery or dispatch; source hashes remain unattributed observations.
pub(crate) fn headless_inspect(database_path:&Path,ticket_id:&str)->Result<String> {
    let db=Connection::open_with_flags(database_path,OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e|e.to_string())?;
    let version:i64=db.pragma_query_value(None,"user_version",|r|r.get(0))
        .map_err(|e|e.to_string())?;
    if version!=storage::SCHEMA_VERSION {
        return Err("Database schema requires a native migration before inspection.".into());
    }
    let store=Store{db,path:database_path.to_path_buf()};
    serde_json::to_string_pretty(&inspect(&store,ticket_id)?).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_native_test_inspector_distinguishes_missing_running_failed_and_stale_receipts() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_demand_test_grants(ticket_id TEXT PRIMARY KEY);
             CREATE TABLE sa_demand_test_runs(ticket_id TEXT PRIMARY KEY,state TEXT);
             INSERT INTO sa_demand_test_grants VALUES('root');
             INSERT INTO sa_demand_test_grants VALUES('other');
             INSERT INTO sa_demand_test_runs VALUES('other','passed');",
        ).unwrap();
        assert_eq!(native_test_state(&db,"unknown").unwrap(),"not_authorized");
        assert_eq!(native_test_state(&db,"root").unwrap(),"authorized_not_run");
        assert_eq!(native_test_state(&db,"other").unwrap(),"passed");
        for state in ["running","failed","uncertain","passed"] {
            db.execute("INSERT OR REPLACE INTO sa_demand_test_runs
                VALUES('root',?1)",[state]).unwrap();
            assert_eq!(native_test_state(&db,"root").unwrap(),state);
            assert_eq!(native_test_state(&db,"other").unwrap(),"passed",
                "One Demand must not read another Demand's test state");
        }
        db.execute(
            "UPDATE sa_demand_test_runs SET state='invalid' WHERE ticket_id='root'",[],
        ).unwrap();
        assert!(native_test_state(&db,"root").is_err(),
            "Malformed receipts must not be presented as an executed pass");
    }

    #[test]
    fn delivery_receipts_include_nested_subagents_without_cross_demand_attribution() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(ticket_id TEXT NOT NULL,child_ticket_id TEXT);
             CREATE TABLE sa_subagent_nodes(parent_ticket_id TEXT NOT NULL,child_ticket_id TEXT);
             CREATE TABLE sa_worker_write_receipts(ticket_id TEXT NOT NULL,state TEXT NOT NULL);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT NOT NULL,state TEXT NOT NULL);
             INSERT INTO sa_work_items VALUES('root','child-a');
             INSERT INTO sa_work_items VALUES('root','child-b');
             INSERT INTO sa_work_items VALUES('other','other-child');
             INSERT INTO sa_subagent_nodes VALUES('child-a','nested-a');
             INSERT INTO sa_subagent_nodes VALUES('nested-a','grandchild');
             INSERT INTO sa_subagent_nodes VALUES('other-child','other-grandchild');
             INSERT INTO sa_worker_write_receipts VALUES('root','applied');
             INSERT INTO sa_worker_write_receipts VALUES('child-a','applied');
             INSERT INTO sa_worker_write_receipts VALUES('child-b','reserved');
             INSERT INTO sa_worker_write_receipts VALUES('nested-a','applied');
             INSERT INTO sa_worker_write_receipts VALUES('grandchild','uncertain');
             INSERT INTO sa_worker_write_receipts VALUES('other-child','uncertain');
             INSERT INTO sa_effect_receipts VALUES('grandchild','pending');
             INSERT INTO sa_effect_receipts VALUES('other-grandchild','uncertain');",
        ).unwrap();
        let (effects,applied,unsettled)=demand_receipts(&db,"root").unwrap();
        assert_eq!((effects,applied,unsettled),(1,3,2));
        assert_eq!(demand_receipts(&db,"other").unwrap(),(1,0,1));
        assert_eq!(demand_receipts(&db,"absent").unwrap(),(0,0,0));
        assert_eq!(classify("completed",2,2,effects+unsettled),"partially_verified");

        db.execute_batch(
            "UPDATE sa_worker_write_receipts SET state='applied'
               WHERE ticket_id IN ('child-b','grandchild');
             UPDATE sa_effect_receipts SET state='applied' WHERE ticket_id='grandchild';",
        ).unwrap();
        let (effects,applied,unsettled)=demand_receipts(&db,"root").unwrap();
        assert_eq!((effects,applied,unsettled),(0,5,0));
        assert_eq!(classify("completed",2,2,effects+unsettled),
            "native_complete_delivery_unverified");
        assert_eq!(demand_receipts(&db,"other").unwrap(),(1,0,1));
    }

    #[test]
    fn delivery_never_counts_an_old_receipt_after_worker_retry_or_revocation() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(
               id TEXT,ticket_id TEXT,child_ticket_id TEXT,state TEXT,
               requires_write INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE sa_work_item_evidence_receipts(
               item_id TEXT,child_ticket_id TEXT,assignment_id TEXT,reviewed_at INTEGER,
               evidence_sha256 TEXT NOT NULL DEFAULT '');
             CREATE TABLE sa_worker_write_receipts(ticket_id TEXT,assignment_id TEXT,state TEXT);
             CREATE TABLE sa_tickets(id TEXT,state TEXT,evidence TEXT);
             CREATE TABLE sa_assignments(
               id TEXT,ticket_id TEXT,state TEXT,attempt INTEGER,result TEXT,updated_at INTEGER);
             INSERT INTO sa_work_items(id,ticket_id,child_ticket_id,state)
               VALUES('item','root','child','completed');
             INSERT INTO sa_work_item_evidence_receipts
               (item_id,child_ticket_id,assignment_id,reviewed_at)
               VALUES('item','child','first',80);
             INSERT INTO sa_tickets VALUES('child','completed','Worker output');
             INSERT INTO sa_assignments VALUES('first','child','finished',1,'Worker output',70);",
        ).unwrap();
        let digest=format!("{:x}",Sha256::digest(b"Worker output"));
        db.execute(
            "UPDATE sa_work_item_evidence_receipts SET evidence_sha256=?1",
            [&digest],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),1);
        assert_eq!(verified_work_items(&db,"other").unwrap(),0);
        db.execute(
            "UPDATE sa_tickets SET state='running' WHERE id='child'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
        db.execute(
            "UPDATE sa_tickets SET state='completed' WHERE id='child'",[],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_assignments VALUES('second','child','running',2,NULL,90)",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
        db.execute(
            "DELETE FROM sa_assignments WHERE id='second'",[],
        ).unwrap();
        db.execute(
            "UPDATE sa_assignments SET updated_at=90 WHERE id='first'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
        db.execute(
            "UPDATE sa_work_item_evidence_receipts SET reviewed_at=91",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),1);
        db.execute(
            "UPDATE sa_tickets SET evidence='changed output' WHERE id='child'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);

        // Replaying the exact Worker output does not allow a mismatched
        // immutable approval hash to count as newly verified delivery.
        db.execute(
            "UPDATE sa_tickets SET evidence='Worker output' WHERE id='child'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),1);
        db.execute(
            "UPDATE sa_work_item_evidence_receipts SET evidence_sha256='stale'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
        db.execute(
            "UPDATE sa_work_item_evidence_receipts SET evidence_sha256=?1",
            [&digest],
        ).unwrap();
        // WorkItems that require edits need the current final attempt's
        // *applied* native ToolHost receipt. A failed write is never enough.
        db.execute("UPDATE sa_work_items SET requires_write=1",[]).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
        db.execute(
            "INSERT INTO sa_worker_write_receipts VALUES('child','first','applied')",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),1);
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='failed'",[],
        ).unwrap();
        assert_eq!(verified_work_items(&db,"root").unwrap(),0);
    }

    #[test]
    fn delivery_state_never_claims_a_verified_release_without_independent_evidence() {
        assert_eq!(classify("completed",3,3,0),"native_complete_delivery_unverified");
        assert_eq!(classify("completed",3,2,0),"partially_verified");
        assert_eq!(classify("completed",3,3,1),"partially_verified");
        assert_eq!(classify("blocked",3,1,0),"blocked");
        assert_eq!(classify("waiting_review",3,2,0),"awaiting_native_review");
        assert_eq!(classify("running",2,0,0),"in_progress");
    }
}
