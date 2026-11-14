//! Trusted, non-Channel native task client for the existing Council.
//! This module does not create reviewers, change permissions, or act as a
//! competing approval engine. Caller IDs must come from trusted app code.
use super::{CouncilStatus, TeamSnapshot};
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const RECORD_KIND: &str = "council_native_task";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCouncilTaskRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub caller_id: String,
    pub session_id: String,
    pub project_id: String,
    pub assignment_key: String,
    pub cycle_id: String,
    pub artifact_sha256: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
    pub task_scope_sha256: String,
    pub admitted_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCouncilTaskReceipt {
    pub schema_version: u32,
    pub request_id: String,
    pub caller_id: String,
    pub session_id: String,
    pub cycle_id: String,
    pub team_revision: u64,
    pub outcome: String,
    pub lowest_verified_score: f64,
    pub evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCouncilTaskInspection {
    pub request: NativeCouncilTaskRequest,
    pub status: Option<CouncilStatus>,
    pub team_revision: u64,
    pub verified_pass: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeTaskRecord {
    request: NativeCouncilTaskRequest,
    receipt: Option<NativeCouncilTaskReceipt>,
}

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn record_key(
    store: &Store, session_id: &str, caller_id: &str, assignment_key: &str,
) -> Result<String> {
    if !super::valid_id(caller_id) || !super::valid_id(assignment_key) {
        return Err("council_task_denied: native caller and assignment require stable identifiers.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: root Architect task does not exist.")?;
    if team.council_since <= 0 {
        return Err("council_task_denied: no active native Council cycle.".into());
    }
    // Current-cycle keys preserve old receipts while permitting a new cycle.
    Ok(format!("native-council-task:{}:{caller_id}:{assignment_key}", team.council_since))
}

fn candidate(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: String, admitted_at: i64,
) -> Result<NativeCouncilTaskRequest> {
    record_key(store, session_id, caller_id, assignment_key)?;
    if artifact_sha256.len() != 64
        || !artifact_sha256.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("council_task_denied: artifact digest must be 64 lowercase hex characters.".into());
    }
    let session = store.session(session_id)?;
    if session.project_id != project_id || session.remote_device.is_some() {
        return Err("council_task_denied: task does not belong to this local project.".into());
    }
    if super::parent_of(store, session_id)?.is_some() {
        return Err("council_task_denied: child and reviewer tasks cannot admit root Council requests.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native Architect task does not exist.")?;
    if team.council_since <= 0 || team.objective.trim().is_empty() {
        return Err("council_task_denied: an active Council cycle and objective are required.".into());
    }
    let config = team.config.council.as_ref()
        .ok_or("council_task_denied: native Council is disabled.")?;
    config.validate()?;
    let channel_count: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_deliveries WHERE session_id=?1",
        [session_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if channel_count != 0 {
        return Err("council_task_denied: Channel tasks must use delivery-scoped Council authority.".into());
    }
    let scope = serde_json::to_vec(&(
        &session.project_id, &session.worktree_path, &session.provider,
        &session.model, &session.reasoning_effort,
    )).map_err(|error| error.to_string())?;
    Ok(NativeCouncilTaskRequest {
        schema_version: 1,
        request_id, caller_id: caller_id.to_owned(),
        session_id: session_id.to_owned(), project_id: project_id.to_owned(),
        assignment_key: assignment_key.to_owned(),
        cycle_id: format!("{}:{}", session_id, team.council_since),
        artifact_sha256: artifact_sha256.to_owned(),
        objective_sha256: digest(team.objective.as_bytes()),
        policy_sha256: digest(&serde_json::to_vec(config).map_err(|error| error.to_string())?),
        task_scope_sha256: digest(&scope), admitted_at,
    })
}

fn read_record(
    store: &Store, caller_id: &str, session_id: &str, assignment_key: &str,
) -> Result<(String, NativeTaskRecord)> {
    let key = record_key(store, session_id, caller_id, assignment_key)?;
    let (kind, content, owner): (String, String, String) = store.db.query_row(
        "SELECT kind,content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2",
        params![session_id, key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|error| error.to_string())?
        .ok_or("council_task_denied: this native task request was not admitted.")?;
    if kind != RECORD_KIND || owner != caller_id {
        return Err("council_task_denied: stored native task owner or record kind mismatches.".into());
    }
    let record: NativeTaskRecord = serde_json::from_str(&content)
        .map_err(|_| "council_task_denied: stored task record is invalid.".to_string())?;
    if record.request.caller_id != caller_id || record.request.session_id != session_id
        || record.request.assignment_key != assignment_key || record.request.schema_version != 1
    {
        return Err("council_task_denied: stored task request has a different owner.".into());
    }
    Ok((content, record))
}

/// Idempotent, native-only admission before the first reviewer is dispatched.
/// A digest passed by a caller is a target identity, never independent proof
/// that any source files were inspected by a reviewer.
pub fn admit(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str,
) -> Result<NativeCouncilTaskRequest> {
    let candidate = candidate(
        store, caller_id, session_id, project_id, assignment_key,
        artifact_sha256, storage::id(), storage::now(),
    )?;
    if let Ok((_, current)) = read_record(store, caller_id, session_id, assignment_key) {
        let same = candidate(
            store, caller_id, session_id, project_id, assignment_key,
            artifact_sha256, current.request.request_id.clone(), current.request.admitted_at,
        )?;
        return if same == current.request { Ok(current.request) } else {
            Err("council_task_conflict: admission cannot rebind objective, policy, cycle or artifact.".into())
        };
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: root Architect task disappeared.")?;
    if team.status != "running" || team.workers.iter().any(|worker|
        worker.council.is_some() && worker.created_at >= team.council_since)
    {
        return Err("council_task_denied: admission must precede the first native reviewer.".into());
    }
    let body = serde_json::to_string(&NativeTaskRecord {
        request: candidate.clone(), receipt: None,
    }).map_err(|error| error.to_string())?;
    let key = record_key(store, session_id, caller_id, assignment_key)?;
    // A single atomic writer statement prevents multiple artifact requests
    // from sharing the same root review cycle, including raced assignments.
    let inserted = store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
         SELECT ?1,?2,?3,?4,?5 WHERE NOT EXISTS (
           SELECT 1 FROM architect_requests
           WHERE parent_session_id=?1 AND kind='council_native_task'
             AND key<>?2
             AND (CASE WHEN json_valid(content)
                   THEN json_extract(content,'$.request.cycleId')
                   ELSE ?6 END)=?6
         ) ON CONFLICT(parent_session_id,key) DO NOTHING",
        params![session_id,key,RECORD_KIND,body,caller_id,candidate.cycle_id],
    ).map_err(|error| error.to_string())?;
    let (_, saved) = read_record(store, caller_id, session_id, assignment_key)
        .map_err(|error| if inserted == 0 {
            "council_task_conflict: another native assignment owns the root Council cycle.".to_owned()
        } else { error })?;
    let same = candidate(
        store, caller_id, session_id, project_id, assignment_key,
        artifact_sha256, saved.request.request_id.clone(), saved.request.admitted_at,
    )?;
    if saved.request != same {
        return Err("council_task_conflict: concurrent admission bound a different native request.".into());
    }
    Ok(saved.request)
}

/// Task/project/assignment/digest/request tuple is required on every read.
/// Request IDs are opaque correlation only and never a bearer capability.
pub fn inspect(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeCouncilTaskInspection> {
    let (_, recorded) = read_record(store, caller_id, session_id, assignment_key)?;
    if recorded.request.request_id != request_id {
        return Err("council_task_denied: native request identity mismatches.".into());
    }
    let current = candidate(
        store, caller_id, session_id, project_id, assignment_key,
        artifact_sha256, request_id.to_owned(), recorded.request.admitted_at,
    )?;
    if recorded.request != current {
        return Err("council_task_stale: Council cycle, target, objective or policy changed.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native task state disappeared.")?;
    Ok(NativeCouncilTaskInspection {
        request: current, status: super::native_council_status(store, &team),
        team_revision: team.revision,
        verified_pass: super::verified_council_pass(store, &team),
    })
}

fn expected_terminal(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeCouncilTaskReceipt> {
    let inspected = inspect(
        store, caller_id, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    let team: TeamSnapshot = super::load(store, session_id)?
        .ok_or("council_task_denied: completed root Council disappeared.")?;
    if team.status != "completed" || !inspected.verified_pass {
        return Err("council_task_denied: there is no verified terminal native Council approval.".into());
    }
    let status = inspected.status.as_ref()
        .ok_or("council_task_denied: root Council status is missing.")?;
    if status.state != "passed" || status.stale {
        return Err("council_task_denied: current Council verdict is not a pass.".into());
    }
    let proof = super::verified_council_task_evidence(store, &team)
        .ok_or("council_task_denied: independent native review evidence is missing.")?;
    if proof.cycle_id != inspected.request.cycle_id {
        return Err("council_task_stale: verified evidence belongs to another cycle.".into());
    }
    let mut seats = 0usize;
    for worker in &team.workers {
        if worker.abandoned || worker.council.as_ref().is_none_or(|seat| seat.round != status.round) {
            continue;
        }
        seats += 1;
        if !worker.read_only || worker.council.as_ref().is_none_or(|seat|
            seat.started_at < inspected.request.admitted_at)
        {
            return Err("council_task_denied: a reviewer began before the frozen task admission.".into());
        }
    }
    if seats == 0 {
        return Err("council_task_denied: no root reviewer was bound to the task.".into());
    }
    let score = status.lowest
        .ok_or("council_task_denied: verified native Council has no lowest score.")?;
    let evidence = serde_json::to_vec(&(&inspected.request, &proof, team.revision, status.round, score))
        .map_err(|error| error.to_string())?;
    Ok(NativeCouncilTaskReceipt {
        schema_version: 1, request_id: request_id.to_owned(),
        caller_id: caller_id.to_owned(), session_id: session_id.to_owned(),
        cycle_id: proof.cycle_id,
        team_revision: team.revision, outcome: "passed".into(),
        lowest_verified_score: score, evidence_sha256: digest(&evidence),
    })
}

/// One-winner terminal receipt via content compare-and-swap. A model's
/// summary, cached verdict or manual score can never create a pass.
pub fn record_terminal(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeCouncilTaskReceipt> {
    let expected = expected_terminal(
        store, caller_id, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    let (old_raw, mut record) = read_record(store, caller_id, session_id, assignment_key)?;
    if let Some(saved) = record.receipt {
        return if saved == expected { Ok(saved) } else {
            Err("council_task_stale: prior receipt no longer matches the current proof.".into())
        };
    }
    record.receipt = Some(expected.clone());
    let next = serde_json::to_string(&record).map_err(|error| error.to_string())?;
    let updated = store.db.execute(
        "UPDATE architect_requests SET content=?1 WHERE parent_session_id=?2
         AND key=?3 AND kind=?4 AND worker_id=?5 AND content=?6",
        params![next,session_id,record_key(store,session_id,caller_id,assignment_key)?,RECORD_KIND,caller_id,old_raw],
    ).map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err("council_task_conflict: another native settlement won this request.".into());
    }
    Ok(expected)
}

pub fn read_terminal(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeCouncilTaskReceipt> {
    let (_, record) = read_record(store, caller_id, session_id, assignment_key)?;
    let saved = record.receipt
        .ok_or("council_task_denied: no verified terminal Council receipt was stored.")?;
    let expected = expected_terminal(
        store, caller_id, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    if saved != expected {
        return Err("council_task_stale: stored receipt differs from verified current evidence.".into());
    }
    Ok(saved)
}

/// Evidence bodies are exposed only through the same native authorization and
/// fresh terminal revalidation, never from an unbound request ID alone.
pub fn read_evidence(
    store: &Store, caller_id: &str, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<Value> {
    read_terminal(store, caller_id, session_id, project_id, assignment_key, artifact_sha256, request_id)?;
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: verified native task disappeared.")?;
    let evidence = super::verified_council_task_evidence(store, &team)
        .ok_or("council_task_denied: current evidence is no longer verified.")?;
    serde_json::to_value(evidence).map_err(|error| error.to_string())
}
