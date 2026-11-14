//! Read-only bridge from an original SuperAgent worker session to the
//! existing Architect/Council #126 state. The native Council owns reviewer
//! eligibility, snapshots, verdicts and tool restrictions; never reimplement
//! scores or treat its team result as authority for an unbound ticket artifact.
use super::Ticket;
use crate::{architect, storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CouncilObservation {
    /// The verified native worker session has an Architect team with Council
    /// enabled; this is **advisory**, not task approval.
    pub native_status: String,
    pub native_round: usize,
    pub native_stale: bool,
    pub native_pass_verified: bool,
    /// A positive root result is exposed only after a task-bound request,
    /// exact frozen-target correlation and native evidence revalidation.
    pub native_approved: bool,
    /// A frozen task artifact is only a review target, never proof that the
    /// native Council saw, approved or completed this ticket.
    pub ticket_artifact_bound: bool,
    /// Exact immutable assignment and ticket revision that were pinned.
    /// A pin does not certify that reviewers saw or accepted the artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen_assignment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen_ticket_revision: Option<i64>,
    /// All reviewers in the native latest round must have started after the
    /// artifact was frozen; still informational until native approval is wired.
    pub review_started_after_binding: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_receipt: Option<NativeCouncilTaskReceipt>,
    /// Opaque handles for the current evidence bundle. Evidence bodies are
    /// returned only by the separate, reauthorizing read endpoint.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeCouncilTaskRequest {
    schema_version: u8,
    origin_kind: String,
    request_id: String,
    request_key: String,
    idempotency_key: String,
    caller_id: String,
    ticket_id: String,
    task_id: String,
    assignment_id: String,
    ticket_revision: i64,
    policy_revision: i64,
    cycle_id: String,
    council_generation: i64,
    artifact_sha256: String,
    council_config_sha256: String,
    policy_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCouncilTaskReceipt {
    pub schema_version: u8,
    pub origin_kind: String,
    pub request_id: String,
    pub request_key: String,
    pub idempotency_key: String,
    pub caller_id: String,
    pub ticket_id: String,
    pub task_id: String,
    pub assignment_id: String,
    pub ticket_revision: i64,
    pub policy_revision: i64,
    pub cycle_id: String,
    pub council_generation: i64,
    pub artifact_sha256: String,
    pub council_config_sha256: String,
    pub policy_fingerprint: String,
    pub council_artifact_revision: String,
    pub council_policy_fingerprint: String,
    pub verdict: String,
    pub approved: bool,
    pub terminal_revision: u64,
    pub evidence_sha256: String,
    pub evidence_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}


/// A task's frozen native review target. Named fields keep cycle, policy and
/// artifact identity distinct instead of silently mixing tuple indices.
struct FrozenTarget {
    assignment_id: String,
    session_id: String,
    ticket_revision: i64,
    policy_revision: i64,
    artifact_sha256: String,
    council_since: i64,
    council_generation: i64,
    council_config_sha256: String,
    bound_at: i64,
}

/// A full unique round must be newer than the frozen artifact.
fn reviewers_started_after_binding<'a>(
    required: usize, seats: impl Iterator<Item = (&'a str, i64)>, bound_at: i64,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut total = 0_usize;
    for (id, started_at) in seats {
        if id.is_empty() || started_at < bound_at || !seen.insert(id) {
            return false;
        }
        total += 1;
    }
    required > 0 && total == required
}

/// Composed bindings freeze the whole quota separately from the selected bot
/// roster. Count a nested result once, through
/// its native-verified parent receipt, never once per leaf review receipt.
fn composed_review_started_after_binding(
    store: &Store,
    team: &architect::TeamSnapshot,
    evidence: &architect::VerifiedCouncilTaskEvidence,
    bound_at: i64,
) -> bool {
    let Some(config) = team.config.council.as_ref() else { return false };
    if config.composition.is_none() { return false; }
    let Some(status) = team.council_status.as_ref() else { return false };
    if status.round == 0 || evidence.cycle_id != format!("{}:{}", team.session_id, team.council_since) {
        return false;
    }
    let workers = team.workers.iter().filter(|worker| {
        !worker.abandoned && worker.created_at >= team.council_since
            && worker.council.as_ref().is_some_and(|seat| seat.round == status.round)
    }).collect::<Vec<_>>();
    let mut frozen_required = None;
    for worker in &workers {
        let Some(binding) = worker.reviewer_binding.as_ref() else { return false };
        if binding.validate(team, worker).is_err()
            || architect::reviewer_security::validate_issued_authority(store, &team.session_id, binding).is_err()
        {
            return false;
        }
        let Some(required) = binding.required_reviewers else { return false };
        if frozen_required.is_some_and(|previous| previous != required) {
            return false;
        }
        frozen_required = Some(required);
        let child_receipts = evidence.child_receipts.iter().filter(|receipt| {
            receipt.root_cycle_id == evidence.cycle_id && receipt.parent_seat_id == worker.id
        }).collect::<Vec<_>>();
        if child_receipts.len() > 1 { return false; }
        if let Some(receipt) = child_receipts.first() {
            if binding.composition_mode.as_deref() != Some("nested")
                || binding.reviewer_bot_id.as_deref() != Some(receipt.bot_id.as_str())
                || binding.artifact_revision != receipt.artifact_revision
            {
                return false;
            }
            let Some((child_id, _)) = receipt.child_cycle_id.rsplit_once(':') else { return false };
            let Ok(Some(child)) = architect::load(store, child_id) else { return false };
            let Some(child_status) = child.council_status.as_ref() else { return false };
            if receipt.child_cycle_id != format!("{}:{}", child.session_id, child.council_since)
                || child_status.round == 0
                || child.workers.iter().filter(|leaf| {
                    !leaf.abandoned && leaf.created_at >= child.council_since
                        && leaf.council.as_ref().is_some_and(|seat| seat.round == child_status.round)
                }).any(|leaf| leaf.council.as_ref().is_none_or(|seat| seat.started_at < bound_at))
            {
                return false;
            }
        } else if evidence.review_receipts.iter().filter(|receipt| {
            receipt.cycle_id == evidence.cycle_id
                && receipt.task_id.as_deref() == Some(team.session_id.as_str())
                && receipt.assignment_id.as_deref() == Some(worker.id.as_str())
                && receipt.terminal_revision == worker.result_revision
        }).count() != 1 {
            // Includes native-verified on-trigger preliminary direct reviews.
            return false;
        }
    }
    let Some(required) = frozen_required else { return false };
    // Auto may legitimately allocate more than the frozen minimum. Native's
    // source-coverage gate has already verified every seat in this evidence.
    workers.len() >= required && reviewers_started_after_binding(
        workers.len(), workers.iter().map(|worker| {
            (worker.session_id.as_str(), worker.council.as_ref().unwrap().started_at)
        }), bound_at,
    )
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Idempotent native records are immutable. A reused request key with any
/// different caller, task, policy or artifact data is a replay conflict.
fn persist_task_record(
    store: &Store,
    task_id: &str,
    key: &str,
    kind: &str,
    content: &str,
    caller_id: &str,
) -> Result<()> {
    store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
         VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(parent_session_id,key) DO NOTHING",
        params![task_id, key, kind, content, caller_id],
    ).map_err(|error| error.to_string())?;
    let saved: (String, String, String) = store.db.query_row(
        "SELECT kind,content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2",
        params![task_id, key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|error| error.to_string())?;
    if saved != (kind.to_string(), content.to_string(), caller_id.to_string()) {
        return Err("council_task_replay_mismatch: the native request key is already bound to different task data.".into());
    }
    Ok(())
}

fn current_finished_assignment(store: &Store, ticket_id: &str) -> Result<Option<(String, String)>> {
    store.db.query_row(
        "SELECT a.id,a.session_id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.id=?1 AND a.bot_id=t.worker_bot_id
           AND a.state='finished' AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments
                          WHERE ticket_id=?1)",
        [ticket_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())
}

fn opaque_evidence_ref(request: &NativeCouncilTaskRequest, evidence_sha256: &str) -> String {
    let material = format!(
        "native-council-evidence:v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        request.request_id,
        request.caller_id,
        request.ticket_id,
        request.task_id,
        request.assignment_id,
        request.ticket_revision,
        request.policy_revision,
        request.cycle_id,
        request.council_generation,
        request.artifact_sha256,
        request.council_config_sha256,
        request.policy_fingerprint,
        evidence_sha256,
    );
    format!("native-council-evidence:v1:{}", sha256(material.as_bytes()))
}

fn receipt_matches_request(
    request: &NativeCouncilTaskRequest,
    receipt: &NativeCouncilTaskReceipt,
) -> bool {
    request.schema_version == 1
        && request.origin_kind == "native_task"
        && receipt.schema_version == 1
        && receipt.origin_kind == request.origin_kind
        && receipt.request_id == request.request_id
        && receipt.request_key == request.request_key
        && receipt.idempotency_key == request.idempotency_key
        && receipt.caller_id == request.caller_id
        && receipt.ticket_id == request.ticket_id
        && receipt.task_id == request.task_id
        && receipt.assignment_id == request.assignment_id
        && receipt.ticket_revision == request.ticket_revision
        && receipt.policy_revision == request.policy_revision
        && receipt.cycle_id == request.cycle_id
        && receipt.council_generation == request.council_generation
        && receipt.artifact_sha256 == request.artifact_sha256
        && receipt.council_config_sha256 == request.council_config_sha256
        && receipt.policy_fingerprint == request.policy_fingerprint
}

/// Return the one durable terminal receipt correlated to this request. A
/// duplicate or altered receipt is a hard denial, even if one row looks valid.
fn terminal_receipt_for_request(
    store: &Store,
    request: &NativeCouncilTaskRequest,
) -> Result<Option<NativeCouncilTaskReceipt>> {
    let mut statement = store.db.prepare(
        "SELECT key,content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND kind='council_task_receipt'",
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([&request.task_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    }).map_err(|e| e.to_string())?;
    let mut matched: Vec<(String, String, NativeCouncilTaskReceipt)> = Vec::new();
    for row in rows {
        let (key, content, caller_id) = row.map_err(|e| e.to_string())?;
        let parsed = serde_json::from_str::<NativeCouncilTaskReceipt>(&content);
        if key == request.idempotency_key && parsed.is_err() {
            return Err("council_task_receipt_invalid: the terminal receipt is malformed.".into());
        }
        let Ok(receipt) = parsed else { continue };
        if receipt.request_id != request.request_id {
            continue;
        }
        matched.push((key, caller_id, receipt));
    }
    if matched.len() > 1 {
        return Err("council_task_receipt_duplicate: more than one terminal receipt matches this request.".into());
    }
    let Some((key, caller_id, receipt)) = matched.pop() else {
        return Ok(None);
    };
    if key != request.idempotency_key
        || caller_id != request.caller_id
        || !receipt_matches_request(request, &receipt)
    {
        return Err("council_task_receipt_mismatch: the terminal receipt is not correlated to this request.".into());
    }
    Ok(Some(receipt))
}

/// Native entrypoint for an existing task's Council. It only records the
/// idempotent, task-bound request and observes the existing Council; it never
/// creates a second Council or dispatches a remediation worker.
pub(crate) fn request_task_review(
    store: &Store,
    task_client: &super::TaskClientContext,
) -> Result<Option<CouncilObservation>> {
    let ticket = super::get_ticket(store, &task_client.ticket_id)?;
    if !matches!(ticket.state.as_str(), "waiting_approval" | "waiting_review") {
        return Ok(None);
    }
    let Some((assignment_id, session_id)) = current_finished_assignment(store, &ticket.id)? else {
        return Err("council_task_denied: no current finished worker assignment.".into());
    };
    if assignment_id != task_client.assignment_id || session_id != task_client.session_id {
        return Err("council_task_denied: task-client assignment binding changed.".into());
    }
    inspect_with_caller(store, &ticket, Some(&task_client.caller_id))
}

/// Read a verified evidence bundle through its opaque reference. Each call
/// rechecks the current ticket, assignment, mandate, Council gate and receipt.
pub(crate) fn read_task_evidence(
    store: &Store,
    task_client: &super::TaskClientContext,
    request_id: &str,
    evidence_ref: &str,
) -> Result<Value> {
    read_task_evidence_for_client(store, task_client, request_id, evidence_ref)
}

fn read_task_evidence_for_client(
    store: &Store,
    task_client: &super::TaskClientContext,
    request_id: &str,
    evidence_ref: &str,
) -> Result<Value> {
    let ticket_id = task_client.ticket_id.as_str();
    let caller_id = task_client.caller_id.as_str();
    let ticket = super::get_ticket(store, ticket_id)?;
    let Some((assignment_id, session_id)) = current_finished_assignment(store, ticket_id)? else {
        return Err("council_evidence_denied: task has no current finished assignment.".into());
    };
    if assignment_id != task_client.assignment_id || session_id != task_client.session_id {
        return Err("council_evidence_denied: task-client assignment binding changed.".into());
    }
    if !super::assignment_thread_current(store, &session_id, ticket_id)? {
        return Err("council_evidence_denied: task assignment is no longer current.".into());
    }
    let stored_request: Option<(String, String, String)> = store.db.query_row(
        "SELECT kind,content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2",
        params![session_id, request_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((kind, content, stored_caller)) = stored_request else {
        return Err("council_evidence_denied: request does not belong to this task.".into());
    };
    if kind != "council_task_request" || stored_caller != caller_id {
        return Err("council_evidence_denied: request caller or kind does not match.".into());
    }
    let request: NativeCouncilTaskRequest = serde_json::from_str(&content)
        .map_err(|_| "council_evidence_denied: stored request is invalid.".to_string())?;
    if request.schema_version != 1
        || request.origin_kind != "native_task"
        || request.request_id != request_id
        || request.request_key != request_id
        || request.caller_id != caller_id
        || request.ticket_id != ticket.id
        || request.task_id != session_id
        || request.assignment_id != assignment_id
    {
        return Err("council_evidence_denied: caller, task or assignment binding changed.".into());
    }
    let Some(previous_receipt) = terminal_receipt_for_request(store, &request)? else {
        return Err("council_evidence_denied: no terminal receipt exists for this request.".into());
    };
    if !previous_receipt.approved
        || previous_receipt.verdict != "approved"
        || !previous_receipt.evidence_refs.iter().any(|reference| reference == evidence_ref)
    {
        return Err("council_evidence_denied: receipt does not authorize this evidence reference.".into());
    }

    // Re-run the current native gate. This also checks that the immutable
    // request/receipt rows still match the exact state used to create them.
    let Some(observation) = inspect_with_caller(store, &ticket, Some(caller_id))? else {
        return Err("council_evidence_denied: task authorization is stale or revoked.".into());
    };
    let Some(current_receipt) = observation.approval_receipt else {
        return Err("council_evidence_denied: current Council result has no terminal receipt.".into());
    };
    if current_receipt != previous_receipt
        || current_receipt.request_id != request_id
        || !receipt_matches_request(&request, &current_receipt)
        || !observation.native_approved
    {
        return Err("council_evidence_denied: current Council result does not match the receipt.".into());
    }
    let team = architect::load(store, &session_id)?
        .ok_or_else(|| "council_evidence_denied: native Council task is unavailable.".to_string())?;
    let evidence = architect::verified_council_task_evidence(store, &team)
        .filter(|evidence| evidence.cycle_id == request.cycle_id)
        .ok_or_else(|| "council_evidence_denied: verified Council evidence is no longer current.".to_string())?;
    if evidence.artifact_revision != current_receipt.council_artifact_revision
        || evidence.policy_fingerprint != current_receipt.council_policy_fingerprint
        || evidence.terminal_revision != current_receipt.terminal_revision
    {
        return Err("council_evidence_denied: evidence digest or policy binding changed.".into());
    }
    let value = serde_json::to_value(&evidence).map_err(|e| e.to_string())?;
    let evidence_bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    let digest = sha256(&evidence_bytes);
    if digest != current_receipt.evidence_sha256
        || opaque_evidence_ref(&request, &digest) != evidence_ref
    {
        return Err("council_evidence_denied: evidence reference is foreign or stale.".into());
    }
    Ok(value)
}

/// Atomically freeze a finished worker's exact ticket artifact against the
/// existing #126 native team/cycle. This is called *inside* the worker-result
/// transaction, and has no effect when Council is off or not configured.
/// It never creates a team, reviewer, synthetic verdict or external action.
pub(super) fn bind_finished(
    store: &Store, ticket_id: &str, assignment_id: &str, session_id: &str,
) -> Result<()> {
    let Some(team) = architect::load(store, session_id)? else { return Ok(()) };
    let Some(council) = team.config.council.as_ref() else { return Ok(()) };
    if team.session_id != session_id || team.council_since <= 0 {
        return Ok(());
    }
    let ticket = super::get_ticket(store, ticket_id)?;
    if ticket.state != "waiting_approval" || ticket.evidence.as_deref().is_none_or(str::is_empty) {
        return Ok(());
    }
    // A caller cannot bind a coordinator plan, another ticket's assignment,
    // an uncommitted/failed callback, or a substituted evidence string.
    let matched: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE a.id=?1 AND a.ticket_id=?2 AND a.session_id=?3
           AND a.bot_id=t.worker_bot_id AND a.state='finished'
           AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?2))",
        params![assignment_id, ticket_id, session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if !matched || !super::assignment_thread_current(store, session_id, ticket_id)? {
        return Ok(());
    }
    let generation = i64::try_from(team.generation)
        .map_err(|_| "Council generation exceeds supported range.")?;
    let config_bytes = serde_json::to_vec(council).map_err(|e| e.to_string())?;
    let policy_revision = super::mandate(store, ticket_id)?.policy_revision;
    store.db.execute(
        "INSERT INTO sa_council_targets(
           ticket_id,assignment_id,session_id,ticket_revision,policy_revision,
           artifact_sha256,council_since,council_generation,council_config_sha256,bound_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(ticket_id) DO UPDATE SET
           assignment_id=excluded.assignment_id,session_id=excluded.session_id,
           ticket_revision=excluded.ticket_revision,policy_revision=excluded.policy_revision,
           artifact_sha256=excluded.artifact_sha256,council_since=excluded.council_since,
           council_generation=excluded.council_generation,
           council_config_sha256=excluded.council_config_sha256,bound_at=excluded.bound_at",
        params![ticket_id,assignment_id,session_id,ticket.revision,policy_revision,
            sha256(ticket.evidence.as_deref().unwrap_or_default().as_bytes()),
            team.council_since,generation,sha256(&config_bytes),storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Only the latest, finished, exactly correlated worker evidence is eligible
/// for a Council lookup. A different session, coordinator plan, stale attempt
/// or remapped bot thread cannot borrow Council status from another task.
pub(super) fn inspect(store: &Store, ticket: &Ticket) -> Result<Option<CouncilObservation>> {
    inspect_with_caller(store, ticket, None)
}

fn inspect_with_caller(
    store: &Store,
    ticket: &Ticket,
    caller_id: Option<&str>,
) -> Result<Option<CouncilObservation>> {
    if !matches!(ticket.state.as_str(), "waiting_approval" | "waiting_review")
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
        || super::mandate(store, &ticket.id)?.revoked
    {
        return Ok(None);
    }
    let grant = super::mandate(store, &ticket.id)?;
    if grant.expires_at <= storage::now() || grant.ticket_id != ticket.id {
        return Ok(None);
    }
    let assignment = current_finished_assignment(store, &ticket.id)?;
    let Some((assignment_id, session)) = assignment else { return Ok(None) };
    if !super::assignment_thread_current(store, &session, &ticket.id)? {
        return Ok(None);
    }
    let Some(team) = architect::load(store, &session)? else { return Ok(None) };
    if team.session_id != session || team.config.council.is_none() {
        return Ok(None);
    }
    let Some(status) = team.council_status.as_ref() else { return Ok(None) };
    // Recompute through the *existing* native Council gate. Its cached
    // "passed" or "accepted" labels never authorize a SuperAgent Ticket.
    let verified = architect::verified_council_pass(store, &team);
    let native_status = if matches!(status.state.as_str(),"passed" | "accepted") && !verified {
        "unverified".into()
    } else {
        status.state.clone()
    };

    let target: Option<FrozenTarget> = store.db.query_row(
        "SELECT assignment_id,session_id,ticket_revision,policy_revision,
                artifact_sha256,council_since,council_generation,council_config_sha256,bound_at
         FROM sa_council_targets WHERE ticket_id=?1",
        [&ticket.id], |r| Ok(FrozenTarget {
            assignment_id: r.get(0)?, session_id: r.get(1)?,
            ticket_revision: r.get(2)?, policy_revision: r.get(3)?,
            artifact_sha256: r.get(4)?, council_since: r.get(5)?,
            council_generation: r.get(6)?, council_config_sha256: r.get(7)?,
            bound_at: r.get(8)?,
        }),
    ).optional().map_err(|e| e.to_string())?;
    let bound_at = target.as_ref().and_then(|target| {
        let council = team.config.council.as_ref()?;
        let config = serde_json::to_vec(council).ok()?;
        (target.assignment_id == assignment_id
            && target.session_id == session
            && target.ticket_revision == ticket.revision
            && target.policy_revision == grant.policy_revision
            && target.artifact_sha256 == sha256(ticket.evidence.as_deref()?.as_bytes())
            && target.council_since == team.council_since
            && i64::try_from(team.generation).ok() == Some(target.council_generation)
            && target.council_config_sha256 == sha256(&config)).then_some(target.bound_at)
    });
    let request = if let (Some(_), Some(caller_id)) = (bound_at, caller_id) {
        let cycle_id = format!("{}:{}", session, team.council_since);
        let config = team.config.council.as_ref().ok_or("Native Council configuration disappeared.")?;
        let config_bytes = serde_json::to_vec(config).map_err(|error| error.to_string())?;
        let policy_bytes = serde_json::to_vec(&grant).map_err(|error| error.to_string())?;
        let request_id = format!(
            "superagent-council-request:{}:{}:{}:{}",
            ticket.id, assignment_id, ticket.revision, cycle_id,
        );
        let idempotency_key = format!("superagent-council-terminal:{request_id}");
        let request = NativeCouncilTaskRequest {
            schema_version: 1,
            origin_kind: "native_task".into(),
            request_id: request_id.clone(),
            request_key: request_id,
            idempotency_key: idempotency_key.clone(),
            caller_id: caller_id.to_string(),
            ticket_id: ticket.id.clone(),
            task_id: session.clone(),
            assignment_id: assignment_id.clone(),
            ticket_revision: ticket.revision,
            policy_revision: grant.policy_revision,
            cycle_id,
            council_generation: i64::try_from(team.generation).map_err(|_| "Council generation exceeds supported range.")?,
            artifact_sha256: sha256(ticket.evidence.as_deref().ok_or("Ticket evidence disappeared.")?.as_bytes()),
            council_config_sha256: sha256(&config_bytes),
            policy_fingerprint: sha256(&policy_bytes),
        };
        let request_json = serde_json::to_string(&request).map_err(|error| error.to_string())?;
        persist_task_record(
            store,
            &session,
            &request.request_key,
            "council_task_request",
            &request_json,
            caller_id,
        )?;
        Some(request)
    } else {
        None
    };
    let verified_evidence = architect::verified_council_task_evidence(store, &team)
        .filter(|evidence| evidence.cycle_id == format!("{}:{}", session, team.council_since));
    let review_started_after_binding = bound_at.is_some_and(|frozen_at| {
        let Some(config) = team.config.council.as_ref() else { return false };
        if config.composition.is_some() {
            return verified_evidence.as_ref().is_some_and(|evidence| {
                composed_review_started_after_binding(store, &team, evidence, frozen_at)
            });
        }
        let required = if config.reviewer_bot_ids.is_empty() {
            config.reviewers.max(2)
        } else {
            config.reviewer_bot_ids.len()
        };
        status.round > 0 && reviewers_started_after_binding(
            required,
            team.workers.iter()
                .filter(|worker| worker.created_at >= team.council_since)
                .filter_map(|worker| worker.council.as_ref()
                    .filter(|seat| seat.round == status.round)
                    .map(|seat| (worker.session_id.as_str(), seat.started_at))),
            frozen_at,
        )
    });
    let mut approval_receipt = None;
    if let Some(request) = request.as_ref() {
        let approved = verified && review_started_after_binding && verified_evidence.is_some();
        let terminal = approved || matches!(team.status.as_str(), "completed" | "failed" | "stopped" | "interrupted");
        if terminal {
            let evidence_json = verified_evidence.as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| error.to_string())?;
            let evidence_bytes = evidence_json.as_ref()
                .map(serde_json::to_vec)
                .transpose()
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            let evidence_sha256 = sha256(&evidence_bytes);
            let evidence_refs = if approved {
                vec![opaque_evidence_ref(request, &evidence_sha256)]
            } else {
                Vec::new()
            };
            let receipt = NativeCouncilTaskReceipt {
                schema_version: 1,
                origin_kind: request.origin_kind.clone(),
                request_id: request.request_id.clone(),
                request_key: request.request_key.clone(),
                idempotency_key: request.idempotency_key.clone(),
                caller_id: request.caller_id.clone(),
                ticket_id: request.ticket_id.clone(),
                task_id: request.task_id.clone(),
                assignment_id: request.assignment_id.clone(),
                ticket_revision: request.ticket_revision,
                policy_revision: request.policy_revision,
                cycle_id: request.cycle_id.clone(),
                council_generation: request.council_generation,
                artifact_sha256: request.artifact_sha256.clone(),
                council_config_sha256: request.council_config_sha256.clone(),
                policy_fingerprint: request.policy_fingerprint.clone(),
                council_artifact_revision: verified_evidence.as_ref()
                    .map_or_else(String::new, |evidence| evidence.artifact_revision.clone()),
                council_policy_fingerprint: verified_evidence.as_ref()
                    .map_or_else(String::new, |evidence| evidence.policy_fingerprint.clone()),
                verdict: if approved { "approved".into() } else { format!("not_approved:{}", status.state) },
                approved,
                terminal_revision: verified_evidence.as_ref()
                    .map_or(team.dispatch_revision, |evidence| evidence.terminal_revision),
                evidence_sha256,
                evidence_refs,
                reason: (!approved).then(|| status.notice.clone().unwrap_or_else(|| {
                    if verified { "native_pass_correlation_failed".into() } else { format!("native_council_{}", native_status) }
                })),
            };
            let receipt_json = serde_json::to_string(&receipt).map_err(|error| error.to_string())?;
            persist_task_record(
                store,
                &request.task_id,
                &request.idempotency_key,
                "council_task_receipt",
                &receipt_json,
                &request.caller_id,
            )?;
            terminal_receipt_for_request(store, request)?
                .filter(|saved| saved == &receipt)
                .ok_or_else(|| "council_task_receipt_missing: terminal receipt did not persist exactly once.".to_string())?;
            approval_receipt = Some(receipt);
        }
    }
    let native_approved = approval_receipt.as_ref().is_some_and(|receipt| receipt.approved);
    let evidence_refs = approval_receipt.as_ref()
        .filter(|receipt| receipt.approved)
        .map_or_else(Vec::new, |receipt| receipt.evidence_refs.clone());
    Ok(Some(CouncilObservation {
        native_status,
        native_round: status.round,
        native_stale: status.stale,
        native_pass_verified: verified,
        native_approved,
        ticket_artifact_bound: bound_at.is_some(),
        frozen_assignment_id: bound_at.map(|_| assignment_id),
        frozen_ticket_revision: bound_at.map(|_| ticket.revision),
        review_started_after_binding,
        approval_receipt,
        evidence_refs,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bots, storage::{self, Store}, superagent::{self, NewOrchestration}};
    use rusqlite::params;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn partial_duplicate_and_stale_rounds_cannot_claim_fresh_review() {
        assert!(!reviewers_started_after_binding(2, [("a",10)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("a",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",9),("b",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("b",11),("c",12)].into_iter(), 10));
        assert!(reviewers_started_after_binding(2, [("a",10),("b",11)].into_iter(), 10));
    }

    fn fixture() -> Store {
        fixture_with_path().0
    }

    fn fixture_with_path() -> (Store, PathBuf) {
        let path = std::env::temp_dir().join(format!("sa-council-{}.sqlite3",storage::id()));
        let store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('project','Disposable','/tmp/disposable-council-bridge')",[],
        ).unwrap();
        for (id,name) in [("router","Router"),("worker","Builder")] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,'project','codex','default','plan',?3,?3)",
                params![id,name,storage::now()],
            ).unwrap();
        }
        assert!(bots::get(&store,"worker").is_ok());
        (store, path)
    }
    fn settled(store: &mut Store, key: &str) -> (superagent::Ticket,String) {
        let cfg = superagent::create_configuration(store,NewOrchestration {
            name: format!("Council {key}"),
            coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1
             WHERE id=?1", [&cfg.id],
        ).unwrap();
        let (ticket, launched)=superagent::admit(store,&cfg.id,"Inspect safely",key,false).unwrap();
        assert!(launched);
        let session=superagent::assignment_session(store,&ticket.id).unwrap().unwrap();
        assert!(superagent::on_run_finished(store,&session,true,"Worker evidence").unwrap());
        (superagent::get_ticket(store,&ticket.id).unwrap(),session)
    }

    fn attach_native_council(store: &Store, session: &str) {
        let team=json!({
            "sessionId":session,
            "config":{
                "workers":[],"maxParallel":1,"maxRounds":2,"maxAssignments":2,
                "workspace":"/tmp/disposable-council-bridge","summaryLimit":3000,
                "council":{"target":9.0,"maxRounds":2,"reviewers":2}
            },
            "status":"ready","round":1,"revision":1,"workers":[],
            "error":null,"summary":null,"updatedAt":storage::now(),
            "councilSince":storage::now(),"generation":1,
            "councilStatus":{"state":"waiting","round":0,"lowest":null,
                             "target":9.0,"stale":false}
        });
        store.db.execute(
            "INSERT INTO architect_teams(session_id,content) VALUES(?1,?2)",
            params![session,team.to_string()],
        ).unwrap();
    }

    fn attach_correlated_passing_reviews(store: &Store, session: &str) {
        let mut team = architect::load(store, session).unwrap().unwrap();
        let bound_at: i64 = store.db.query_row(
            "SELECT bound_at FROM sa_council_targets WHERE ticket_id=(
               SELECT ticket_id FROM sa_assignments WHERE session_id=?1
             )",
            [session], |row| row.get(0),
        ).unwrap();
        team.status = "completed".into();
        team.round = 1;
        team.dispatch_revision = 30;
        team.changed_at = 0;
        team.council_status = Some(architect::CouncilStatus {
            state: "passed".into(), round: 1, lowest: Some(9.5), target: 9.0,
            stale: false, notice: None,
        });
        let project_id = store.session(session).unwrap().project_id;
        for (index, id) in ["native-review-a", "native-review-b"].into_iter().enumerate() {
            let created_at = bound_at + index as i64 + 1;
            let reviewer_session = store.create_session(&project_id, "codex", "default", "ask").unwrap();
            let seat = architect::CouncilSeat {
                round: 1, lens: format!("lens-{index}"), score: Some(9.6 - index as f64 / 10.0),
                closed_by_selection: false, started_at: created_at,
            };
            let mut worker = architect::Worker {
                id: id.into(), session_id: reviewer_session.id, name: id.into(),
                profile_id: "automatic".into(), provider: "codex".into(), model: "default".into(),
                reasoning_effort: None, status: "completed".into(), assignment: "Review the task.".into(),
                summary: "Verified current task evidence.".into(), error: None, depends_on: Vec::new(),
                files: vec!["src".into()], read_only: true, attempt: 1, created_at,
                updated_at: created_at + 1, keys: vec![format!("review-{index}")], abandoned: false,
                result_revision: 20 + index as u64, checkpoint: None, timed_out: false, failovers: 0,
                parent_worker: None, resume_prompt: None, tool_catalog_version: 0, nudged: false,
                reported: false, council: Some(seat.clone()), reviewer_binding: None,
                score_nudged: false, wrote_at: 0,
            };
            let binding = architect::reviewer_security::ReviewerBinding::native(
                &team, &worker.id, created_at, &worker.provider, &worker.model, &seat,
            );
            architect::reviewer_security::record_native_authority(store, session, &binding).unwrap();
            worker.reviewer_binding = Some(binding.clone());
            let frozen_policy = serde_json::to_vec(&(
                &binding.snapshot.tool_policy, &binding.reviewer_bot_id,
                &binding.snapshot.bot_context, &binding.snapshot.skills,
                &binding.composition_mode, &binding.delivery_id, &binding.parent_council,
            )).unwrap();
            let cycle_id = binding.council_cycle_id.clone();
            let request_id = format!("native-council-request:{cycle_id}:{}:1", worker.id);
            let receipt = json!({
                "schemaVersion":1,"requestId":request_id,"cycleId":cycle_id,
                "taskId":session,"assignmentId":worker.id,
                "policyFingerprint":sha256(&frozen_policy),
                "artifactDigest":sha256(binding.artifact_revision.as_bytes()),
                "verdict":"passed","findingsRefs":[],"evidenceRefs":[],
                "terminalRevision":worker.result_revision,
                "idempotencyKey":format!("native-council-terminal:{}:{}:1",binding.council_cycle_id,worker.id)
            });
            store.db.execute(
                "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
                 VALUES(?1,?2,'council_review_receipt',?3,?4)",
                params![session,format!("native-council-review-receipt:{}:{}:1",binding.council_cycle_id,worker.id),receipt.to_string(),worker.id],
            ).unwrap();
            team.workers.push(worker);
        }
        store.db.execute(
            "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
            params![session,serde_json::to_string(&team).unwrap()],
        ).unwrap();
    }

    fn completed_native_task(store: &mut Store, key: &str) -> (superagent::Ticket, String) {
        let cfg = superagent::create_configuration(store, NewOrchestration {
            name: format!("Council {key}"),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1
             WHERE id=?1", [&cfg.id],
        ).unwrap();
        let (ticket, launched) = superagent::admit(
            store, &cfg.id, "Review the exact native task artifact", key, false,
        ).unwrap();
        assert!(launched);
        let session = superagent::assignment_session(store, &ticket.id).unwrap().unwrap();
        attach_native_council(store, &session);
        assert!(superagent::on_run_finished(store, &session, true, "Exact worker artifact").unwrap());
        (superagent::get_ticket(store, &ticket.id).unwrap(), session)
    }

    fn test_task_client(
        store: &Store,
        ticket_id: &str,
        caller_id: Option<&str>,
    ) -> super::super::TaskClientContext {
        let (assignment_id, session_id) = current_finished_assignment(store, ticket_id)
            .unwrap().expect("fixture has a current finished worker assignment");
        super::super::TaskClientContext {
            ticket_id: ticket_id.to_string(),
            assignment_id,
            session_id,
            caller_id: caller_id.map(str::to_string)
                .unwrap_or_else(|| format!("test-task-client:{ticket_id}")),
        }
    }

    fn approved_native_task(
        store: &mut Store,
        key: &str,
    ) -> (superagent::Ticket, String, CouncilObservation) {
        let (ticket, session) = completed_native_task(store, key);
        attach_correlated_passing_reviews(store, &session);
        let client = test_task_client(store, &ticket.id, None);
        let observation = request_task_review(store, &client).unwrap().unwrap();
        (ticket, session, observation)
    }

    fn save_test_team(store: &Store, team: &architect::TeamSnapshot) {
        store.db.execute(
            "INSERT INTO architect_teams(session_id,content) VALUES(?1,?2)
             ON CONFLICT(session_id) DO UPDATE SET content=excluded.content",
            params![team.session_id, serde_json::to_string(team).unwrap()],
        ).unwrap();
    }

    fn test_council_record(store: &Store, session: &str, key: &str, kind: &str, value: Value) {
        store.db.execute(
            "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
             VALUES(?1,?2,?3,?4,'fixture')",
            params![session, key, kind, value.to_string()],
        ).unwrap();
    }

    fn test_review_receipt(team: &architect::TeamSnapshot, worker: &architect::Worker) -> Value {
        let binding = worker.reviewer_binding.as_ref().unwrap();
        let policy = serde_json::to_vec(&(
            &binding.snapshot.tool_policy, &binding.reviewer_bot_id,
            &binding.snapshot.bot_context, &binding.snapshot.skills,
            &binding.composition_mode, &binding.delivery_id, &binding.parent_council,
        )).unwrap();
        json!({
            "schemaVersion":1,
            "requestId":format!("native-council-request:{}:{}:1",binding.council_cycle_id,worker.id),
            "cycleId":binding.council_cycle_id,"taskId":team.session_id,"assignmentId":worker.id,
            "policyFingerprint":sha256(&policy),"artifactDigest":sha256(binding.artifact_revision.as_bytes()),
            "verdict":"passed","findingsRefs":[],"evidenceRefs":[],
            "terminalRevision":worker.result_revision,
            "idempotencyKey":format!("native-council-terminal:{}:{}:1",binding.council_cycle_id,worker.id)
        })
    }

    fn test_criterion_outcome(
        team: &architect::TeamSnapshot, worker: &architect::Worker,
        root_cycle: &str, artifact_revision: &str, parent_seat: Option<&str>,
    ) -> Value {
        let cycle = format!("{}:{}", team.session_id, team.council_since);
        json!({
            "schemaVersion":1,"rootCycleId":root_cycle,"taskId":team.session_id,
            "taskCycleId":cycle,"childCycleId":parent_seat.map(|_| cycle.as_str()),
            "round":1,"seatId":worker.id,"parentSeatId":parent_seat,"attempt":1,
            "artifactRevision":artifact_revision,
            "outcome":{"criterionId":"coverage","score":9.5,"verifiedEvidenceRefs":[]}
        })
    }

    /// Persist the same authority, composed quota, child ownership and
    /// terminal contracts consumed by the real native gate, without launching a CLI.
    fn completed_composed_task(store: &mut Store, mode: &str) -> (superagent::Ticket, String) {
        let (ticket, session) = completed_native_task(store, mode);
        attach_correlated_passing_reviews(store, &session);
        let mut team = architect::load(store, &session).unwrap().unwrap();
        let template = team.workers[0].clone();
        team.workers.clear();
        store.db.execute(
            "DELETE FROM architect_requests WHERE parent_session_id=?1
             AND kind IN ('reviewer_authority','council_review_receipt')", [&session],
        ).unwrap();
        let nested = json!({
            "template":"validated_default","childCriteriaSource":"architect","childReviewerCount":2,
            "target":9.0,"maxRounds":2,"escalation":"always","userForced":false
        });
        let sources: Vec<(&str, &str)> = match mode {
            "M3" => vec![("direct-bot", "direct")],
            "M5" => vec![("nested-bot", "nested")],
            "M7" => vec![("direct-bot", "direct"), ("nested-bot", "nested")],
            _ => panic!("unsupported composed fixture"),
        };
        let roster = sources.iter().map(|(id, _)| id.to_string()).collect::<Vec<_>>();
        for (id, _) in &sources {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',1,1)", [id],
            ).unwrap();
        }
        let selected = sources.iter().map(|(id, source)| {
            let mut value = json!({"botId":id,"mode":source,"required":true});
            if *source == "nested" { value["nested"] = nested.clone(); }
            value
        }).collect::<Vec<_>>();
        let composition = json!({
            "schemaVersion":1,"automatic":{"enabled":true,"count":1},"selected":selected,
            "criteriaSource":"architect","criteria":[{
                "id":"coverage","lens":"coverage","description":"Review the frozen artifact.",
                "required":true,"target":9.0,"evidenceRequired":false,"source":"architect"
            }],"strategy":"parallel",
            "limits":{"maxDepth":2,"maxRootSeats":8,"maxConcurrentLeaves":8,
                      "maxLeafReviewsPerRound":32,"timeoutMs":3600000,"maxEstimatedCostMicros":null}
        });
        let config = team.config.council.as_mut().unwrap();
        config.reviewers = 1;
        config.reviewer_bot_ids = roster.clone();
        config.composition = Some(serde_json::from_value(composition.clone()).unwrap());
        config.validate_composition().unwrap();
        save_test_team(store, &team);
        let client = test_task_client(store, &ticket.id, None);
        bind_finished(store, &ticket.id, &client.assignment_id, &session).unwrap();
        let bound_at: i64 = store.db.query_row(
            "SELECT bound_at FROM sa_council_targets WHERE ticket_id=?1", [&ticket.id], |row| row.get(0),
        ).unwrap();
        let cycle = format!("{}:{}", session, team.council_since);
        for index in 0..=sources.len() {
            let mut worker = template.clone();
            worker.id = format!("composed-{index}");
            worker.session_id = store.create_session("project", "codex", "default", "ask").unwrap().id;
            worker.created_at = bound_at + index as i64 + 1;
            worker.updated_at = worker.created_at + 1;
            worker.result_revision = 20 + index as u64;
            let seat = worker.council.as_mut().unwrap();
            seat.lens = format!("lens-{index}");
            seat.score = Some(9.5);
            seat.started_at = worker.created_at;
            let mut binding = architect::reviewer_security::ReviewerBinding::native(
                &team, &worker.id, worker.created_at, &worker.provider, &worker.model,
                worker.council.as_ref().unwrap(),
            );
            binding.composition_mode = Some("direct".into());
            if index > 0 {
                let (bot_id, source) = sources[index - 1];
                binding.reviewer_bot_id = Some(bot_id.into());
                binding.composition_mode = Some(source.into());
                binding.roster_bot_ids = roster.clone();
                binding.snapshot.bot_context = Some(architect::reviewer_security::ReviewerBotContext::from_bot(
                    &bots::get(store, bot_id).unwrap(),
                ));
            }
            architect::reviewer_security::record_native_authority(store, &session, &binding).unwrap();
            worker.reviewer_binding = Some(binding.clone());
            if binding.composition_mode.as_deref() == Some("nested") {
                let mut child = team.clone();
                child.session_id = worker.session_id.clone();
                child.council_since = bound_at + 10;
                child.workers.clear();
                let mut child_composition = composition.clone();
                child_composition["automatic"]["count"] = json!(2);
                child_composition["selected"] = json!([]);
                let child_config = child.config.council.as_mut().unwrap();
                child_config.reviewers = 2;
                child_config.reviewer_bot_ids.clear();
                child_config.composition = Some(serde_json::from_value(child_composition).unwrap());
                let child_cycle = format!("{}:{}", child.session_id, child.council_since);
                for leaf_index in 0..2 {
                    let mut leaf = template.clone();
                    leaf.id = format!("leaf-{leaf_index}");
                    leaf.session_id = store.create_session("project", "codex", "default", "ask").unwrap().id;
                    leaf.created_at = child.council_since + leaf_index + 1;
                    leaf.council.as_mut().unwrap().started_at = leaf.created_at;
                    leaf.council.as_mut().unwrap().lens = format!("leaf-lens-{leaf_index}");
                    leaf.council.as_mut().unwrap().score = Some(9.5);
                    let mut leaf_binding = architect::reviewer_security::ReviewerBinding::native(
                        &child, &leaf.id, leaf.created_at, &leaf.provider, &leaf.model,
                        leaf.council.as_ref().unwrap(),
                    );
                    leaf_binding.parent_council = Some(architect::reviewer_security::ReviewerParentBinding {
                        session_id: session.clone(), council_cycle_id: cycle.clone(),
                        artifact_revision: binding.artifact_revision.clone(), seat_id: worker.id.clone(), depth: 1,
                    });
                    architect::reviewer_security::record_native_authority(store, &child.session_id, &leaf_binding).unwrap();
                    leaf.reviewer_binding = Some(leaf_binding);
                    test_council_record(store, &session, &format!("leaf-receipt-{leaf_index}"),
                        "council_review_receipt", test_review_receipt(&child, &leaf));
                    test_council_record(store, &session, &format!("child-outcome-{leaf_index}"),
                        "council_criterion_outcome", test_criterion_outcome(
                            &child, &leaf, &cycle, &binding.artifact_revision, Some(&worker.id),
                        ));
                    test_council_record(store, &child.session_id, &format!("leaf-outcome-{leaf_index}"),
                        "council_criterion_outcome", test_criterion_outcome(
                            &child, &leaf, &child_cycle,
                            &leaf.reviewer_binding.as_ref().unwrap().artifact_revision, None,
                        ));
                    child.workers.push(leaf);
                }
                save_test_team(store, &child);
                store.db.execute("INSERT INTO architect_workers VALUES(?1,?2,?3)",
                    params![session, worker.id, child.session_id]).unwrap();
                test_council_record(store, &session, &format!("native-council-child:{cycle}:{}",worker.id),
                    "council_child_link", json!({
                        "schemaVersion":1,"rootSessionId":session,"rootWorkerId":worker.id,
                        "rootCouncilCycleId":cycle,"rootRound":1,"rootArtifactRevision":binding.artifact_revision,
                        "rootDispatchRevision":team.dispatch_revision,"rootChangedAt":0,
                        "childSessionId":child.session_id,"childCouncilCycleId":child_cycle,
                        "deliveryId":null,"depth":1,"timeoutAt":storage::now()+3600000,
                        "maxConcurrentLeaves":8,"maxLeafReviewsPerRound":32,"maxEstimatedCostMicros":null,
                        "nested":nested,"binding":binding,"createdAt":worker.created_at
                    }));
                let child_snapshot = serde_json::to_vec(&(
                    &child.session_id, child.council_since, child.dispatch_revision,
                    child.changed_at, &child.config.council, &child.workers,
                )).unwrap();
                let root_review = test_review_receipt(&team, &worker);
                test_council_record(store, &session, "child-receipt", "council_child_receipt", json!({
                    "rootCycleId":cycle,"parentSeatId":worker.id,"childCycleId":child_cycle,
                    "botId":binding.reviewer_bot_id,"artifactRevision":binding.artifact_revision,
                    "policyFingerprint":root_review["policyFingerprint"],"status":"passed","minScore":9.5,
                    "leafScoreRefs":["leaf-0:1","leaf-1:1"],"findingsRefs":[],"evidenceRefs":[],
                    "snapshotFingerprint":sha256(&child_snapshot)
                }));
            } else {
                test_council_record(store, &session, &format!("root-receipt-{index}"),
                    "council_review_receipt", test_review_receipt(&team, &worker));
                test_council_record(store, &session, &format!("root-outcome-{index}"),
                    "council_criterion_outcome", test_criterion_outcome(
                        &team, &worker, &cycle, &binding.artifact_revision, None,
                    ));
            }
            team.workers.push(worker);
        }
        let unknown = json!({"status":"unknown"});
        test_council_record(store, &session, "usage", "council_usage_record", json!({
            "id":format!("native-council-usage:{cycle}:composed-0:1"),"rootCycleId":cycle,
            "attemptId":"composed-0:1","reviewerId":"automatic","roundId":format!("{cycle}:1"),
            "seatId":"composed-0","scope":"root","phase":"review","strategy":"parallel",
            "queueWaitMs":unknown,"elapsedMs":unknown,"retries":unknown,"escalations":unknown,
            "inputTokens":unknown,"outputTokens":unknown,"requestCount":unknown,"terminalStatus":"passed",
            "budgetConsumption":{"concurrentLeaves":unknown,"leafReviews":unknown,"estimatedCostMicros":unknown}
        }));
        save_test_team(store, &team);
        (ticket, session)
    }

    #[test]
    fn composed_m3_m5_m7_issue_one_receipt_authorize_evidence_and_replay_after_restart() {
        for mode in ["M3", "M5", "M7"] {
            let (mut store, path) = fixture_with_path();
            let (ticket, session) = completed_composed_task(&mut store, mode);
            let team = architect::load(&store, &session).unwrap().unwrap();
            let config = team.config.council.as_ref().unwrap();
            let frozen_required = config.reviewers + config.reviewer_bot_ids.len();
            let round_seats = team.workers.iter().filter(|worker| {
                worker.council.as_ref().is_some_and(|seat| seat.round == 1)
            }).collect::<Vec<_>>();
            assert!(round_seats.iter().all(|worker| worker.reviewer_binding.as_ref()
                .is_some_and(|binding| binding.required_reviewers == Some(frozen_required))),
                "{mode}: every binding must retain the frozen composed quota");
            if mode == "M7" {
                assert_eq!(frozen_required, 3);
                for worker in round_seats.iter().filter(|worker| worker.reviewer_binding.as_ref()
                    .is_some_and(|binding| binding.reviewer_bot_id.is_some())) {
                    worker.reviewer_binding.as_ref().unwrap().validate(&team, worker).unwrap();
                }
            }
            let client = test_task_client(&store, &ticket.id, None);
            let first = request_task_review(&store, &client).unwrap().unwrap();
            assert!(first.native_pass_verified, "{mode}: {first:?}");
            assert!(first.review_started_after_binding, "{mode}: frozen allocation must include automatic seats");
            assert!(first.native_approved, "{mode}: {first:?}");
            let receipt = first.approval_receipt.clone().unwrap();
            assert_eq!(receipt.verdict, "approved");
            assert_eq!(receipt.evidence_refs.len(), 1);
            let reference = &receipt.evidence_refs[0];
            assert!(reference.starts_with("native-council-evidence:v1:"));
            assert!(serde_json::to_value(&first).unwrap().get("authorizedEvidence").is_none());
            let evidence = read_task_evidence(&store, &client, &receipt.request_id, reference).unwrap();
            assert_eq!(evidence["childReceipts"].as_array().unwrap().len(), usize::from(mode != "M3"));
            let foreign = test_task_client(&store, &ticket.id, Some("foreign-caller"));
            assert!(read_task_evidence(&store, &foreign, &receipt.request_id, reference).is_err());
            assert!(request_task_review(&store, &foreign).unwrap_err().contains("replay_mismatch"));
            assert!(read_task_evidence(&store, &client, &receipt.request_id, "native-council-evidence:v1:foreign").is_err());
            assert_eq!(request_task_review(&store, &client).unwrap().unwrap().approval_receipt, Some(receipt.clone()));
            drop(store);
            let reopened = Store::open(&path).unwrap();
            assert_eq!(request_task_review(&reopened, &client).unwrap().unwrap().approval_receipt, Some(receipt.clone()));
            assert_eq!(read_task_evidence(&reopened, &client, &receipt.request_id, reference).unwrap(), evidence);
            for kind in ["council_task_request", "council_task_receipt"] {
                assert_eq!(reopened.db.query_row::<i64,_,_>(
                    "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind=?2",
                    params![session, kind], |row| row.get(0),
                ).unwrap(), 1, "{mode}: {kind} is persisted exactly once");
            }
            reopened.db.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&ticket.id]).unwrap();
            assert!(read_task_evidence(&reopened, &client, &receipt.request_id, reference).is_err());
        }
    }

    #[test]
    fn composed_receipts_do_not_authorize_missing_foreign_stale_or_nonpassing_seats() {
        for mode in ["M3", "M5", "M7"] {
            for fault in ["missing_automatic", "stale_start", "accepted", "missing_receipt", "foreign_child", "stale_child_start"] {
                if mode == "M3" && matches!(fault, "foreign_child" | "stale_child_start") { continue; }
                let mut store = fixture();
                let (ticket, session) = completed_composed_task(&mut store, mode);
                let mut team = architect::load(&store, &session).unwrap().unwrap();
                match fault {
                    "missing_automatic" => { team.workers.remove(0); }
                    "stale_start" => { team.workers[0].council.as_mut().unwrap().started_at = 1; }
                    "accepted" => { team.council_status.as_mut().unwrap().state = "accepted".into(); }
                    "missing_receipt" => {
                        store.db.execute("DELETE FROM architect_requests WHERE parent_session_id=?1 AND key='root-receipt-0'", [&session]).unwrap();
                    }
                    "foreign_child" => {
                        store.db.execute("UPDATE architect_requests SET content=json_set(content,'$.rootCycleId','foreign:1')
                            WHERE parent_session_id=?1 AND kind='council_child_receipt'", [&session]).unwrap();
                    }
                    "stale_child_start" => {
                        let parent = team.workers.iter().find(|worker| {
                            worker.reviewer_binding.as_ref().unwrap().composition_mode.as_deref() == Some("nested")
                        }).unwrap();
                        let mut child = architect::load(&store, &parent.session_id).unwrap().unwrap();
                        child.workers[0].council.as_mut().unwrap().started_at = 1;
                        save_test_team(&store, &child);
                        let snapshot = serde_json::to_vec(&(
                            &child.session_id, child.council_since, child.dispatch_revision,
                            child.changed_at, &child.config.council, &child.workers,
                        )).unwrap();
                        store.db.execute("UPDATE architect_requests SET content=json_set(content,'$.snapshotFingerprint',?2)
                            WHERE parent_session_id=?1 AND kind='council_child_receipt'",
                            params![session, sha256(&snapshot)]).unwrap();
                        assert!(architect::verified_council_pass(&store, &team),
                            "{mode}: native pass alone does not prove child review started after task binding");
                    }
                    _ => unreachable!(),
                }
                save_test_team(&store, &team);
                let client = test_task_client(&store, &ticket.id, None);
                let observation = request_task_review(&store, &client).unwrap().unwrap();
                assert!(!observation.native_approved, "{mode}/{fault}: {observation:?}");
                assert!(observation.evidence_refs.is_empty());
                assert!(!observation.approval_receipt.unwrap().approved);
            }
        }
    }

    #[test]
    fn legacy_m2_selected_roster_still_authorizes_receipt_evidence_and_replay() {
        let mut store = fixture();
        let (ticket, session) = completed_native_task(&mut store, "legacy-selected");
        let mut team = architect::load(&store, &session).unwrap().unwrap();
        let roster = vec!["legacy-a".to_string(), "legacy-b".to_string()];
        let config = team.config.council.as_mut().unwrap();
        config.reviewer_bot_ids = roster.clone();
        config.reviewers = 0;
        assert!(config.composition.is_none());
        save_test_team(&store, &team);
        let client = test_task_client(&store, &ticket.id, None);
        bind_finished(&store, &ticket.id, &client.assignment_id, &session).unwrap();
        attach_correlated_passing_reviews(&store, &session);
        team = architect::load(&store, &session).unwrap().unwrap();
        store.db.execute("DELETE FROM architect_requests WHERE parent_session_id=?1
            AND kind IN ('reviewer_authority','council_review_receipt')", [&session]).unwrap();
        for (index, bot_id) in roster.iter().enumerate() {
            store.db.execute("INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                VALUES(?1,?1,'project','codex','default','plan',1,1)", [bot_id]).unwrap();
            let worker = &mut team.workers[index];
            let binding = worker.reviewer_binding.as_mut().unwrap();
            binding.reviewer_bot_id = Some(bot_id.clone());
            binding.roster_bot_ids = roster.clone();
            binding.required_reviewers = Some(roster.len());
            binding.snapshot.bot_context = Some(architect::reviewer_security::ReviewerBotContext::from_bot(
                &bots::get(&store, bot_id).unwrap(),
            ));
            architect::reviewer_security::record_native_authority(&store, &session, binding).unwrap();
        }
        for worker in &team.workers {
            test_council_record(&store, &session, &format!("legacy-receipt-{}", worker.id),
                "council_review_receipt", test_review_receipt(&team, worker));
        }
        save_test_team(&store, &team);
        let first = request_task_review(&store, &client).unwrap().unwrap();
        assert!(first.native_approved, "{first:?}");
        let receipt = first.approval_receipt.clone().unwrap();
        assert!(read_task_evidence(&store, &client, &receipt.request_id, &first.evidence_refs[0]).is_ok());
        assert_eq!(request_task_review(&store, &client).unwrap().unwrap().approval_receipt, Some(receipt));
    }

    #[test]
    fn finished_worker_pins_native_council_cycle_and_exact_artifact_without_approval() {
        let mut store=fixture();
        let cfg=superagent::create_configuration(&store,NewOrchestration {
            name:"Pinned native review".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute("UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&cfg.id]).unwrap();
        let (ticket, launched)=superagent::admit(&mut store,&cfg.id,"Review exact evidence","pin",false).unwrap();
        assert!(launched);
        let session=superagent::assignment_session(&store,&ticket.id).unwrap().unwrap();
        attach_native_council(&store,&session);
        assert!(superagent::on_run_finished(&store,&session,true,"Artifact revision A").unwrap());
        let saved=superagent::get_ticket(&store,&ticket.id).unwrap();
        let task_client = test_task_client(&store, &saved.id, None);
        let snapshot = superagent::ticket_snapshot(&store, &saved.id, 10).unwrap();
        assert!(snapshot.council.is_some());
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_request'",
            [&session], |row| row.get(0),
        ).unwrap(), 0, "a public snapshot cannot create a task-client request without a capability");
        let observed=request_task_review(&store,&task_client).unwrap().unwrap();
        assert!(observed.ticket_artifact_bound);
        assert_eq!(observed.frozen_ticket_revision,Some(saved.revision));
        assert!(observed.frozen_assignment_id.is_some());
        assert!(!observed.native_pass_verified);
        assert!(!observed.native_approved);
        assert!(!observed.review_started_after_binding);
        assert_eq!(saved.state,"waiting_approval");
        let request: (String, String, String) = store.db.query_row(
            "SELECT key,content,worker_id FROM architect_requests
             WHERE parent_session_id=?1 AND kind='council_task_request'",
            [&session], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        let request: NativeCouncilTaskRequest=serde_json::from_str(&request.1).unwrap();
        assert_eq!(request.ticket_id,saved.id);
        assert_eq!(request.task_id,session);
        assert_eq!(request.assignment_id,observed.frozen_assignment_id.unwrap());
        assert_eq!(request.ticket_revision,saved.revision);
        assert_eq!(request.cycle_id,format!("{}:{}",request.task_id,request.cycle_id.rsplit(':').next().unwrap()));
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_receipt'",[&session],|row|row.get(0),
        ).unwrap(),0);

        // Even if a stale Council team reports a passing label, it cannot
        // complete a Ticket or bypass its native evidence/authority checks.
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilStatus.state','passed','$.councilStatus.round',1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(!inspect(&store,&saved).unwrap().unwrap().native_pass_verified);
        assert_eq!(superagent::get_ticket(&store,&saved.id).unwrap().state,"waiting_approval");

        // Changing the native Council cycle, frozen policy or ticket revision
        // invalidates the bind without recycling the old passing observation.
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilSince',json_extract(content,'$.councilSince')+1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(!inspect(&store,&saved).unwrap().unwrap().ticket_artifact_bound);
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilSince',json_extract(content,'$.councilSince')-1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1",
            [&saved.id]).unwrap();
        let revised=superagent::get_ticket(&store,&saved.id).unwrap();
        assert!(!inspect(&store,&revised).unwrap().unwrap().ticket_artifact_bound);
    }

    #[test]
    fn council_target_never_leaks_across_distinct_tickets_or_remapped_threads() {
        let mut store=fixture();
        let (first,session)=settled(&mut store,"other");
        // No native Council during the first task: there is nothing to bind.
        assert!(inspect(&store,&first).unwrap().is_none());
        attach_native_council(&store,&session);
        // A post-settlement team cannot retroactively approve old evidence.
        assert!(inspect(&store,&first).unwrap().is_some());
        assert!(!inspect(&store,&first).unwrap().unwrap().ticket_artifact_bound);
        let (second,other_session)=settled(&mut store,"second");
        attach_native_council(&store,&other_session);
        assert!(!inspect(&store,&second).unwrap().unwrap().ticket_artifact_bound);
        store.db.execute(
            "UPDATE bot_threads SET external_id='foreign-ticket' WHERE session_id=?1",
            [&session],
        ).unwrap();
        assert!(inspect(&store,&first).unwrap().is_none());
    }

    #[test]
    fn council_off_manual_ticket_has_no_implicit_reviewer_or_gate() {
        let mut store=fixture();
        let (ticket,_) = settled(&mut store,"off");
        assert_eq!(ticket.state,"waiting_approval");
        assert!(inspect(&store,&ticket).unwrap().is_none());
        let snapshot=superagent::ticket_snapshot(&store,&ticket.id,10).unwrap();
        assert!(snapshot.council.is_none());
        // Existing manual Council-Off acceptance is unaffected.
        let accepted=superagent::accept(&store,&ticket.id,ticket.revision).unwrap();
        assert_eq!(accepted.state,"completed");
    }

    #[test]
    fn correlated_current_native_pass_returns_one_receipt_and_opaque_evidence_read() {
        let mut store=fixture();
        let (saved, session, first) = approved_native_task(&mut store, "positive");
        assert!(first.native_pass_verified, "native observation: {first:?}");
        assert!(first.ticket_artifact_bound);
        assert!(first.review_started_after_binding);
        assert!(first.native_approved);
        assert_eq!(first.approval_receipt.as_ref().unwrap().verdict,"approved");
        assert_eq!(first.approval_receipt.as_ref().unwrap().ticket_id,saved.id);
        assert_eq!(first.evidence_refs.len(), 1);
        assert!(first.evidence_refs[0].starts_with("native-council-evidence:v1:"));
        assert_eq!(first.evidence_refs, first.approval_receipt.as_ref().unwrap().evidence_refs);
        assert!(serde_json::to_value(&first).unwrap().get("authorizedEvidence").is_none(),
            "the observation must carry only an opaque reference, never the evidence body");

        let task_client = test_task_client(&store, &saved.id, None);
        let request_id = first.approval_receipt.as_ref().unwrap().request_id.clone();
        let evidence = read_task_evidence(&store, &task_client, &request_id, &first.evidence_refs[0]).unwrap();
        assert!(evidence["cycleId"].is_string());

        let replay=request_task_review(&store,&task_client).unwrap().unwrap();
        assert_eq!(replay.approval_receipt,first.approval_receipt,"the correlated terminal receipt is immutable across replay");
        assert_eq!(replay.evidence_refs,first.evidence_refs);
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_request'",[&session],|row|row.get(0),
        ).unwrap(),1);
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_receipt'",[&session],|row|row.get(0),
        ).unwrap(),1);

        store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1",[&saved.id]).unwrap();
        let revised=superagent::get_ticket(&store,&saved.id).unwrap();
        let denied=inspect(&store,&revised).unwrap().unwrap();
        assert!(!denied.ticket_artifact_bound);
        assert!(!denied.native_approved);
        assert!(denied.evidence_refs.is_empty());
        assert!(read_task_evidence(&store, &task_client, &request_id, &first.evidence_refs[0]).is_err());
    }

    #[test]
    fn native_task_request_replay_with_changed_caller_or_content_is_denied() {
        let mut store=fixture();
        let (_,session)=settled(&mut store,"request-replay");
        persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:a").unwrap();
        persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:a").unwrap();
        assert!(persist_task_record(&store,&session,"native-request-key","council_task_request","changed-request","superagent-ticket:a").unwrap_err().contains("council_task_replay_mismatch"));
        assert!(persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:b").unwrap_err().contains("council_task_replay_mismatch"));
    }

    #[test]
    fn task_evidence_read_rejects_wrong_caller_task_assignment_digest_policy_and_foreign_refs() {
        let mut store = fixture();
        let (ticket, _, observation) = approved_native_task(&mut store, "evidence-owner");
        let receipt = observation.approval_receipt.as_ref().unwrap();
        let evidence_ref = &observation.evidence_refs[0];
        let task_client = test_task_client(&store, &ticket.id, None);
        let foreign_client = test_task_client(&store, &ticket.id, Some("foreign-task-client"));
        assert!(read_task_evidence_for_client(
            &store, &foreign_client, &receipt.request_id, evidence_ref,
        ).unwrap_err().contains("caller"));

        let (foreign_ticket, _, foreign_observation) = approved_native_task(&mut store, "evidence-foreign");
        assert!(read_task_evidence(
            &store,
            &task_client,
            &receipt.request_id,
            &foreign_observation.evidence_refs[0],
        ).unwrap_err().contains("authorize"));
        let foreign_task_client = test_task_client(&store, &foreign_ticket.id, None);
        assert!(read_task_evidence(
            &store, &foreign_task_client, &receipt.request_id, evidence_ref,
        ).is_err());

        for (field, forged_value) in [
            ("callerId", "superagent-ticket:forged"),
            ("taskId", "foreign-task"),
            ("assignmentId", "foreign-assignment"),
            ("artifactSha256", "0000000000000000000000000000000000000000000000000000000000000000"),
            ("policyFingerprint", "1111111111111111111111111111111111111111111111111111111111111111"),
        ] {
            let mut isolated = fixture();
            let (bound_ticket, session, bound_observation) = approved_native_task(&mut isolated, &format!("tamper-{field}"));
            let bound_receipt = bound_observation.approval_receipt.as_ref().unwrap();
            let bound_client = test_task_client(&isolated, &bound_ticket.id, None);
            isolated.db.execute(
                "UPDATE architect_requests
                 SET content=json_set(content,?3,?4)
                 WHERE parent_session_id=?1 AND key=?2 AND kind='council_task_request'",
                params![session, bound_receipt.request_id, format!("$.{field}"), forged_value],
            ).unwrap();
            assert!(read_task_evidence(
                &isolated,
                &bound_client,
                &bound_receipt.request_id,
                &bound_observation.evidence_refs[0],
            ).is_err(), "forged request field {field} must be denied");
        }

        let request_id = receipt.request_id.clone();
        let duplicate = serde_json::to_string(receipt).unwrap();
        let session_id = receipt.task_id.clone();
        store.db.execute(
            "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
             VALUES(?1,'duplicate-terminal-receipt','council_task_receipt',?2,?3)",
            params![session_id, duplicate, receipt.caller_id],
        ).unwrap();
        assert!(read_task_evidence(
            &store, &task_client, &request_id, evidence_ref,
        ).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn task_request_and_terminal_receipt_replay_stably_after_database_restart() {
        let (mut store, path) = fixture_with_path();
        let (ticket, session, first) = approved_native_task(&mut store, "restart");
        let receipt = first.approval_receipt.clone().unwrap();
        let evidence_ref = first.evidence_refs[0].clone();
        let task_client = test_task_client(&store, &ticket.id, None);
        drop(store);

        let reopened = Store::open(&path).unwrap();
        let replay = request_task_review(&reopened, &task_client).unwrap().unwrap();
        assert_eq!(replay.approval_receipt, Some(receipt.clone()));
        assert_eq!(replay.evidence_refs, vec![evidence_ref.clone()]);
        assert!(read_task_evidence(&reopened, &task_client, &receipt.request_id, &evidence_ref)
            .unwrap()["cycleId"].is_string());
        assert_eq!(reopened.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_request'",
            [&session], |row| row.get(0),
        ).unwrap(), 1);
        assert_eq!(reopened.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_receipt'",
            [&session], |row| row.get(0),
        ).unwrap(), 1);
    }

    #[test]
    fn task_request_only_observes_the_existing_council_and_reads_deny_after_revocation() {
        let mut store = fixture();
        let config = superagent::create_configuration(&store, NewOrchestration {
            name: "Read-only issued task client".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();
        let (admitted, launched) = superagent::admit(
            &mut store, &config.id, "Read-only Council request", "read-only-request", false,
        ).unwrap();
        assert!(launched);
        let capability = superagent::issue_task_client_capability(&store, &admitted).unwrap();
        let session = superagent::assignment_session(&store, &admitted.id).unwrap().unwrap();
        attach_native_council(&store, &session);
        assert!(superagent::on_run_finished(&store, &session, true, "Read-only task artifact").unwrap());
        let ticket = superagent::get_ticket(&store, &admitted.id).unwrap();
        let task_client = superagent::authorize_task_client_capability(
            &store, &ticket.id, &capability,
        ).unwrap();
        attach_correlated_passing_reviews(&store, &session);
        let team_before: String = store.db.query_row(
            "SELECT content FROM architect_teams WHERE session_id=?1", [&session], |row| row.get(0),
        ).unwrap();
        let workers_before = architect::load(&store, &session).unwrap().unwrap().workers.len();
        let assignment_count_before = store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1", [&ticket.id], |row| row.get(0),
        ).unwrap();
        let request = request_task_review(&store, &task_client).unwrap().unwrap();
        let receipt = request.approval_receipt.unwrap();
        assert!(read_task_evidence(
            &store, &task_client, &receipt.request_id, &request.evidence_refs[0],
        ).is_ok());
        let team_after: String = store.db.query_row(
            "SELECT content FROM architect_teams WHERE session_id=?1", [&session], |row| row.get(0),
        ).unwrap();
        assert_eq!(team_after, team_before, "task review request/read must not mutate the Council team");
        assert_eq!(architect::load(&store, &session).unwrap().unwrap().workers.len(), workers_before,
            "task review request must not dispatch a second Council or remediation worker");
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1", [&ticket.id], |row| row.get(0),
        ).unwrap(), assignment_count_before);
        assert_eq!(superagent::get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");

        store.db.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1", [&ticket.id]).unwrap();
        assert!(read_task_evidence(
            &store, &task_client, &receipt.request_id, &request.evidence_refs[0],
        ).unwrap_err().contains("stale or revoked"));
    }

    #[test]
    fn command_boundary_denies_foreign_or_forged_capabilities_before_council_access() {
        let mut store = fixture();
        let config = superagent::create_configuration(&store, NewOrchestration {
            name: "Command boundary capability binding".into(),
            coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();

        let (first, first_launched) = superagent::admit(
            &mut store, &config.id, "First task", "boundary-first", false,
        ).unwrap();
        assert!(first_launched);
        let first_capability = superagent::issue_task_client_capability(&store, &first).unwrap();
        let first_session = superagent::assignment_session(&store, &first.id).unwrap().unwrap();
        assert!(superagent::on_run_finished(
            &store, &first_session, true, "First task evidence",
        ).unwrap());
        let (second, second_launched) = superagent::admit(
            &mut store, &config.id, "Second task", "boundary-second", false,
        ).unwrap();
        assert!(second_launched);
        let second_capability = superagent::issue_task_client_capability(&store, &second).unwrap();
        assert_ne!(first_capability, second_capability);

        let foreign_request_error = crate::superagent_council_task_request_for_client(
            &store, &first.id, &second_capability,
        ).err().expect("foreign capability must fail at the command boundary");
        assert!(foreign_request_error.contains("task_client_denied"));
        let forged_request_error = crate::superagent_council_task_request_for_client(
            &store, &first.id, "callerId=superagent-ticket:forged",
        ).err().expect("callerId text must not authenticate a request");
        assert!(forged_request_error.contains("task_client_denied"));

        let foreign_read_error = crate::superagent_council_evidence_read_for_client(
            &store,
            &first.id,
            &second_capability,
            "native-council-request:foreign",
            "native-council-evidence:v1:foreign",
        ).err().expect("foreign capability must fail before evidence lookup");
        assert!(foreign_read_error.contains("task_client_denied"));
        assert_eq!(store.db.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM architect_requests WHERE kind='council_task_request'",
            [], |row| row.get(0),
        ).unwrap(), 0, "denied requests must not create task-client records");
    }

    #[test]
    fn forged_or_unbound_council_status_cannot_become_ticket_approval() {
        let mut store=fixture();
        let (ticket,session)=settled(&mut store,"advisory");
        // A persisted Architect team is native input, but a cached "passed"
        // label without seats, frozen evidence or a ticket-artifact binding
        // must not acquire SuperAgent completion authority.
        let snapshot=json!({
            "sessionId":session,
            "config":{
                "workers":[],"maxParallel":1,"maxRounds":2,"maxAssignments":2,
                "workspace":"/tmp/disposable-council-bridge","summaryLimit":3000,
                "council":{"target":9.0,"maxRounds":2,"reviewers":1}
            },
            "status":"ready","round":1,"revision":1,"workers":[],
            "error":null,"summary":null,"updatedAt":storage::now(),
            "councilStatus":{
                "state":"passed","round":1,"lowest":9.5,
                "target":9.0,"stale":false
            }
        });
        store.db.execute(
            "INSERT INTO architect_teams(session_id,content) VALUES(?1,?2)",
            params![&session,snapshot.to_string()],
        ).unwrap();
        let observation=inspect(&store,&ticket).unwrap().unwrap();
        assert_eq!(observation.native_status,"unverified");
        assert!(!observation.native_pass_verified);
        assert!(!observation.ticket_artifact_bound);
        assert_eq!(superagent::get_ticket(&store,&ticket.id).unwrap().state,"waiting_approval");
        // Detaching the original worker thread removes even advisory status.
        store.db.execute(
            "UPDATE bot_threads SET external_id='another-ticket' WHERE session_id=?1",
            [&session],
        ).unwrap();
        assert!(inspect(&store,&ticket).unwrap().is_none());
    }
}
