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
    pub repository_head_sha:Option<String>,
    pub worktree_status_sha256:Option<String>,
    pub worktree_diff_sha256:Option<String>,
    pub observed_status_entries:Option<usize>,
    pub checkout_observation:String,
    pub source_attribution:String,
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

fn git_read(root:&Path,args:&[&str],limit:usize)->Result<Vec<u8>> {
    let output=crate::workspace::run_git_bounded(
        root,args,limit,false,Duration::from_secs(10),
    ).map_err(|_|"Native read-only Git observation unavailable.".to_owned())?;
    if !output.status.success() || output.stdout.truncated {
        return Err("Native Git inspection failed or exceeded its byte limit.".into());
    }
    Ok(output.stdout.bytes)
}

fn observe_git(root:&Path)->Result<(String,String,String,usize)> {
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
    let verified:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_work_item_evidence_receipts evidence
         JOIN sa_work_items item ON item.id=evidence.item_id
         WHERE item.ticket_id=?1 AND item.state='completed'
           AND item.child_ticket_id=evidence.child_ticket_id",
        [ticket_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    let unresolved:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_effect_receipts effect
         WHERE effect.state IN ('pending','uncertain')
           AND (effect.ticket_id=?1 OR effect.ticket_id IN (
             SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1))",
        [ticket_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
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
        delivery_state:classify(&ticket.state,total,verified,unresolved).into(),
        total_work_items:total,verified_work_items:verified,
        unresolved_external_effects:unresolved,
        repository_head_sha:None,worktree_status_sha256:None,
        worktree_diff_sha256:None,observed_status_entries:None,
        checkout_observation:if allowed {"pending"} else {"unavailable_or_unpinned"}.into(),
        source_attribution:"not_attributed_to_ticket".into(),
        test_execution_verified:false,ci_result_verified:false,
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
    fn delivery_state_never_claims_a_verified_release_without_independent_evidence() {
        assert_eq!(classify("completed",3,3,0),"native_complete_delivery_unverified");
        assert_eq!(classify("completed",3,2,0),"partially_verified");
        assert_eq!(classify("completed",3,3,1),"partially_verified");
        assert_eq!(classify("blocked",3,1,0),"blocked");
        assert_eq!(classify("waiting_review",3,2,0),"awaiting_native_review");
        assert_eq!(classify("running",2,0,0),"in_progress");
    }
}
