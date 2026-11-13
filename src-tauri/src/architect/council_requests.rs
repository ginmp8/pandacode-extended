//! Trusted, task-bound native Council admission. This does not dispatch reviewers
//! or create an alternative score/approval engine. Channel delivery admission
//! owns the transaction; only the existing Council may decide a verdict.
use super::{council, CouncilConfig};
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS extended_council_task_requests(
      request_id TEXT PRIMARY KEY,
      delivery_id TEXT NOT NULL UNIQUE REFERENCES extended_channel_council_deliveries(delivery_id) ON DELETE CASCADE,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      context_json TEXT NOT NULL CHECK(json_valid(context_json)),
      created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS extended_council_task_requests_session ON extended_council_task_requests(session_id,created_at);
";

/// Schema v45: receipts are written only by the winning native channel settlement.
pub(crate) const RECEIPT_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS extended_council_task_receipts(
      request_id TEXT PRIMARY KEY REFERENCES extended_council_task_requests(request_id) ON DELETE CASCADE,
      delivery_id TEXT NOT NULL UNIQUE REFERENCES extended_channel_council_deliveries(delivery_id) ON DELETE CASCADE,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      receipt_json TEXT NOT NULL CHECK(json_valid(receipt_json)),
      created_at INTEGER NOT NULL
    );
";

/// Immutable per-delivery scope. The digest covers the admitted *objective*;
/// it is not a verified source-code SHA or an approval token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CouncilTaskReviewRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub delivery_id: String,
    pub session_id: String,
    pub channel_id: String,
    pub source_message_id: String,
    pub executor_bot_id: String,
    pub delivery_cycle_id: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
}

/// Read-only current-cycle projection from the *existing* native Council.
/// It is not a durable terminal receipt or permission to publish an effect.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CouncilTaskInspection {
    pub request: CouncilTaskReviewRequest,
    pub native: council::CouncilInspection,
}

struct ChannelScope {
    session_id: String,
    channel_id: String,
    source_message_id: String,
    executor_bot_id: String,
    delivery_cycle_id: String,
    objective: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn channel_scope(store: &Store, delivery_id: &str) -> Result<ChannelScope> {
    store.db.query_row(
        "SELECT d.session_id,d.channel_id,d.message_id,d.bot_id,c.cycle_id,c.objective
         FROM bot_shared_channel_deliveries d
         JOIN extended_channel_council_deliveries c ON c.delivery_id=d.id
           AND c.channel_id=d.channel_id
           AND c.source_message_id=d.message_id
           AND c.executor_bot_id=d.bot_id
         WHERE d.id=?1",
        [delivery_id],
        |row| Ok(ChannelScope {
            session_id: row.get(0)?, channel_id: row.get(1)?,
            source_message_id: row.get(2)?, executor_bot_id: row.get(3)?,
            delivery_cycle_id: row.get(4)?, objective: row.get(5)?,
        }),
    ).optional().map_err(|e| e.to_string())?
        .ok_or_else(|| "council_task_denied: linked channel delivery not found.".to_owned())
}

fn frozen_config(store: &Store, session_id: &str) -> Result<(CouncilConfig, bool)> {
    if super::parent_of(store, session_id)?.is_some() {
        return Err("council_task_denied: a reviewer worker cannot own a root request.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native Council task was not configured.")?;
    let config = team.config.council
        .ok_or("council_task_denied: task Council is disabled.")?;
    Ok((config, team.council_since == 0 && team.workers.is_empty()))
}

fn request_for(scope: &ChannelScope, delivery_id: &str, config: &CouncilConfig, request_id: String) -> Result<CouncilTaskReviewRequest> {
    let policy = serde_json::to_vec(config).map_err(|e| e.to_string())?;
    Ok(CouncilTaskReviewRequest {
        schema_version: 1,
        request_id,
        delivery_id: delivery_id.to_owned(),
        session_id: scope.session_id.clone(),
        channel_id: scope.channel_id.clone(),
        source_message_id: scope.source_message_id.clone(),
        executor_bot_id: scope.executor_bot_id.clone(),
        delivery_cycle_id: scope.delivery_cycle_id.clone(),
        objective_sha256: digest(scope.objective.as_bytes()),
        policy_sha256: digest(&policy),
    })
}

/// Idempotent admission after channel + native team rows exist in the SAME
/// transaction. No permissions, reviewers or approvals are created here.
pub(crate) fn admit_channel(store: &Store, delivery_id: &str) -> Result<CouncilTaskReviewRequest> {
    let scope = channel_scope(store, delivery_id)?;
    let (config, fresh) = frozen_config(store, &scope.session_id)?;
    let existing: Option<String> = store.db.query_row(
        "SELECT context_json FROM extended_council_task_requests WHERE delivery_id=?1",
        [delivery_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let candidate = request_for(&scope, delivery_id, &config, storage::id())?;
    if let Some(raw) = existing {
        let previous: CouncilTaskReviewRequest = serde_json::from_str(&raw)
            .map_err(|_| "council_task_denied: stored request is invalid.")?;
        let same = previous == CouncilTaskReviewRequest { request_id: previous.request_id.clone(), ..candidate };
        return if same { Ok(previous) } else {
            Err("council_task_conflict: this delivery was already admitted with different authority or evidence.".into())
        };
    }
    if !fresh {
        return Err("council_task_denied: a review cannot be admitted after its native cycle started.".into());
    }
    let body = serde_json::to_string(&candidate).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO extended_council_task_requests(request_id,delivery_id,session_id,context_json,created_at)
         VALUES(?1,?2,?3,?4,?5)",
        params![candidate.request_id,delivery_id,scope.session_id,body,storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(candidate)
}

/// Native-only scoped read; callers must know this request's exact delivery
/// and session, which are revalidated against persisted ownership and pins.
pub(crate) fn inspect_channel(store: &Store, request_id: &str, delivery_id: &str, session_id: &str) -> Result<CouncilTaskInspection> {
    let raw: String = store.db.query_row(
        "SELECT context_json FROM extended_council_task_requests
         WHERE request_id=?1 AND delivery_id=?2 AND session_id=?3",
        params![request_id,delivery_id,session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("council_task_denied: request, delivery or task do not match.")?;
    let recorded: CouncilTaskReviewRequest = serde_json::from_str(&raw)
        .map_err(|_| "council_task_denied: stored request is invalid.")?;
    if recorded.request_id != request_id || recorded.delivery_id != delivery_id || recorded.session_id != session_id {
        return Err("council_task_denied: request identity is inconsistent.".into());
    }
    let scope = channel_scope(store, delivery_id)?;
    let (config, _) = frozen_config(store, session_id)?;
    let current = request_for(&scope, delivery_id, &config, recorded.request_id.clone())?;
    if current != recorded {
        return Err("council_task_stale: source, objective, cycle or policy has changed.".into());
    }
    let native = super::council_inspection_state(store, session_id)?
        .ok_or("council_task_denied: native task inspection is unavailable.")?;
    Ok(CouncilTaskInspection { request: recorded, native })
}


/// Native terminal receipt for one winning, approved Channel delivery.
/// This is correlated evidence, not a bearer token or a substitute for the
/// live native authorization check performed on every read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CouncilTaskTerminalReceipt {
    pub schema_version: u32,
    pub request_id: String,
    pub delivery_id: String,
    pub session_id: String,
    pub root_cycle_id: String,
    pub team_revision: u64,
    /// "passed" or "accepted_by_user"; user acceptance never invents a score.
    pub outcome: String,
    pub lowest_verified_score: Option<f64>,
    /// Digest of the trusted request and current native verdict references,
    /// not a signature, source-code hash, external execution proof or grant.
    pub evidence_sha256: String,
}

fn current_terminal(
    store: &Store, request_id: &str, delivery_id: &str, session_id: &str,
) -> Result<CouncilTaskTerminalReceipt> {
    let inspected = inspect_channel(store, request_id, delivery_id, session_id)?;
    if !crate::bots::shared_channels::council_callback_allowed(store, delivery_id)? {
        return Err("council_task_denied: the original native callback is not authorized.".into());
    }
    let native = &inspected.native;
    if native.status.stale {
        return Err("council_task_stale: the native Council revision changed.".into());
    }
    let (outcome, lowest) = match native.status.state.as_str() {
        "passed" if native.verified_pass => {
            let proof = native.verified_completion.as_ref()
                .ok_or("council_task_denied: missing independently verified completion.")?;
            ("passed", Some(proof.lowest))
        }
        // The callback gate independently verifies an attributed, revision-bound
        // local-user acceptance. It must not be converted to a passing score.
        "accepted" => ("accepted_by_user", None),
        _ => return Err("council_task_denied: no approved terminal Council verdict.".into()),
    };
    let root_cycle_id = format!("{}:{}", session_id, native.cycle_started_at);
    if native.verified_completion.as_ref().is_some_and(|proof|
        proof.session_id != session_id || proof.cycle_id != root_cycle_id
            || proof.team_revision != native.team_revision)
    {
        return Err("council_task_stale: verified completion has different root ownership.".into());
    }
    let evidence = serde_json::to_vec(&(
        &inspected.request, &root_cycle_id, native.team_revision,
        native.status.round, outcome, lowest,
    )).map_err(|e| e.to_string())?;
    Ok(CouncilTaskTerminalReceipt {
        schema_version: 1,
        request_id: request_id.to_owned(),
        delivery_id: delivery_id.to_owned(),
        session_id: session_id.to_owned(),
        root_cycle_id,
        team_revision: native.team_revision,
        outcome: outcome.to_owned(),
        lowest_verified_score: lowest,
        evidence_sha256: digest(&evidence),
    })
}

/// Called inside the existing winning channel settlement transaction, after
/// its status is "completed" but before its single final reply can be stored.
/// A legacy pre-v44 delivery has no native request and keeps the old gate.
pub(crate) fn record_terminal_channel(
    store: &Store, delivery_id: &str, session_id: &str,
) -> Result<Option<CouncilTaskTerminalReceipt>> {
    let issued: Option<String> = store.db.query_row(
        "SELECT request_id FROM extended_council_task_requests WHERE delivery_id=?1 AND session_id=?2",
        params![delivery_id,session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let Some(request_id) = issued else { return Ok(None) };
    let expected = current_terminal(store, &request_id, delivery_id, session_id)?;
    let raw = serde_json::to_string(&expected).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT OR IGNORE INTO extended_council_task_receipts
         (request_id,delivery_id,session_id,receipt_json,created_at)
         VALUES(?1,?2,?3,?4,?5)",
        params![request_id,delivery_id,session_id,raw,storage::now()],
    ).map_err(|e| e.to_string())?;
    read_terminal_channel(store, &request_id, delivery_id, session_id).map(Some)
}

/// Task-scoped read-only terminal receipt. Revalidates live native authority;
/// a stale request or altered proof cannot become an approval by replay.
pub(crate) fn read_terminal_channel(
    store: &Store, request_id: &str, delivery_id: &str, session_id: &str,
) -> Result<CouncilTaskTerminalReceipt> {
    let raw: String = store.db.query_row(
        "SELECT receipt_json FROM extended_council_task_receipts
         WHERE request_id=?1 AND delivery_id=?2 AND session_id=?3",
        params![request_id,delivery_id,session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("council_task_denied: no terminal receipt for this authorized delivery.")?;
    let stored: CouncilTaskTerminalReceipt = serde_json::from_str(&raw)
        .map_err(|_| "council_task_denied: stored receipt is invalid.")?;
    let current = current_terminal(store, request_id, delivery_id, session_id)?;
    if stored != current {
        return Err("council_task_stale: terminal receipt no longer matches native evidence.".into());
    }
    Ok(stored)
}


/// A non-Channel native caller may bind only its own existing root task. The
/// artifact digest is caller-supplied correlation, NOT proof that files were
/// checked. No reviewer, tool grant, Council approval or callback is created.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTaskCouncilRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub session_id: String,
    pub project_id: String,
    pub assignment_key: String,
    pub cycle_id: String,
    pub artifact_sha256: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
    pub task_scope_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTaskCouncilReceipt {
    pub schema_version: u32,
    pub request_id: String,
    pub session_id: String,
    pub cycle_id: String,
    pub team_revision: u64,
    pub outcome: String,
    pub lowest_verified_score: f64,
    pub evidence_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTaskCouncilInspection {
    pub request: NativeTaskCouncilRequest,
    pub native: council::CouncilInspection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeTaskRecord {
    request: NativeTaskCouncilRequest,
    receipt: Option<NativeTaskCouncilReceipt>,
}

fn native_task_key(assignment_key: &str) -> Result<String> {
    if !super::valid_id(assignment_key) {
        return Err("council_task_denied: assignment key must be a stable 1-80 byte identifier.".into());
    }
    Ok(format!("native-council-task:{assignment_key}"))
}

fn native_task_candidate(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: String,
) -> Result<NativeTaskCouncilRequest> {
    native_task_key(assignment_key)?;
    if artifact_sha256.len() != 64 ||
        !artifact_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("council_task_denied: artifact digest must be 64 lowercase hex characters.".into());
    }
    let session = store.session(session_id)?;
    if session.project_id != project_id || session.remote_device.is_some() {
        return Err("council_task_denied: native task owner or project does not match.".into());
    }
    // Reject missing or non-root Council cycles before examining any Channel
    // routing state. An idle configured team is not an admitted review task.
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native Architect task does not exist.")?;
    if super::parent_of(store, session_id)?.is_some() ||
        team.council_since <= 0 || team.objective.trim().is_empty()
    {
        return Err("council_task_denied: an active root Council cycle is required.".into());
    }
    // A Channel delivery has its own per-message authority and receipt path.
    let channel_count: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM extended_channel_council_deliveries c
         JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
         WHERE d.session_id=?1",
        [session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if channel_count != 0 {
        return Err("council_task_denied: Channel reviews require channel-scoped admission.".into());
    }
    let config = team.config.council
        .ok_or("council_task_denied: native Council is disabled.")?;
    let scope = serde_json::to_vec(&(
        &session.project_id, &session.worktree_path, &session.provider,
        &session.model, &session.reasoning_effort,
    )).map_err(|e| e.to_string())?;
    Ok(NativeTaskCouncilRequest {
        schema_version: 1, request_id, session_id: session_id.to_owned(),
        project_id: project_id.to_owned(), assignment_key: assignment_key.to_owned(),
        cycle_id: format!("{}:{}", session_id, team.council_since),
        artifact_sha256: artifact_sha256.to_owned(),
        objective_sha256: digest(team.objective.as_bytes()),
        policy_sha256: digest(&serde_json::to_vec(&config).map_err(|e| e.to_string())?),
        task_scope_sha256: digest(&scope),
    })
}

fn native_task_record(
    store: &Store, session_id: &str, assignment_key: &str,
) -> Result<(String, NativeTaskRecord)> {
    let key = native_task_key(assignment_key)?;
    let raw: String = store.db.query_row(
        "SELECT content FROM architect_requests WHERE parent_session_id=?1
         AND key=?2 AND kind='council_native_task' AND worker_id=?1",
        params![session_id, key], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("council_task_denied: this native task request was not admitted.")?;
    let record = serde_json::from_str(&raw)
        .map_err(|_| "council_task_denied: native task request record is invalid.")?;
    Ok((raw, record))
}

/// Existing architect_requests provides task-scoped, cascade-deleted storage.
/// This is a native-only operation; it does not launch or grant reviewers.
pub(crate) fn admit_native_task(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str,
) -> Result<NativeTaskCouncilRequest> {
    let key = native_task_key(assignment_key)?;
    let candidate = native_task_candidate(
        store, session_id, project_id, assignment_key, artifact_sha256, storage::id(),
    )?;
    let existing: Option<String> = store.db.query_row(
        "SELECT content FROM architect_requests WHERE parent_session_id=?1 AND key=?2",
        params![session_id, key], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(raw) = existing {
        let saved: NativeTaskRecord = serde_json::from_str(&raw)
            .map_err(|_| "council_task_conflict: existing native task record is invalid.")?;
        let matching = NativeTaskCouncilRequest {
            request_id: saved.request.request_id.clone(), ..candidate
        };
        return if saved.request == matching { Ok(saved.request) }
            else { Err("council_task_conflict: an assignment cannot be rebound to another Council cycle, objective, policy or artifact.".into()) };
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: root team disappeared.")?;
    // Admission is prospective: a completed, paused or merely configured
    // Council must not gain a new task/artefact binding from an old pass.
    // Existing identical requests remain idempotently inspectable/settleable.
    if team.status != "running" {
        return Err("council_task_denied: new requests require an active root Council execution.".into());
    }
    if team.workers.iter().any(|worker|
        worker.created_at >= team.council_since && worker.council.is_some())
    {
        return Err("council_task_denied: admission after reviewer dispatch is not allowed.".into());
    }
    let body = serde_json::to_string(&NativeTaskRecord {
        request: candidate.clone(), receipt: None,
    }).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
         VALUES(?1,?2,'council_native_task',?3,?1)",
        params![session_id, key, body],
    ).map_err(|e| format!("council_task_conflict: {e}"))?;
    Ok(candidate)
}

/// The caller must present its task, project, assignment, artifact and request ID.
/// A request ID alone is never a capability or an approval token.
pub(crate) fn inspect_native_task(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeTaskCouncilInspection> {
    let (_, record) = native_task_record(store, session_id, assignment_key)?;
    if record.request.request_id != request_id {
        return Err("council_task_denied: native request identity mismatch.".into());
    }
    let current = native_task_candidate(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id.to_owned(),
    )?;
    if record.request != current {
        return Err("council_task_stale: native task, cycle, objective, policy or artifact changed.".into());
    }
    let native = super::council_inspection_state(store, session_id)?
        .ok_or("council_task_denied: native Council inspection is unavailable.")?;
    if format!("{}:{}", session_id, native.cycle_started_at) != current.cycle_id {
        return Err("council_task_stale: native Council cycle changed.".into());
    }
    Ok(NativeTaskCouncilInspection { request: current, native })
}

fn native_terminal_expected(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeTaskCouncilReceipt> {
    let inspected = inspect_native_task(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    let native = &inspected.native;
    // A user exception is never relabeled "passed"; it needs its own
    // attributed, reasoned consumer contract before it can be emitted here.
    if native.status.state != "passed" || native.status.stale || !native.verified_pass {
        return Err("council_task_denied: no independently verified root Council pass.".into());
    }
    let proof = native.verified_completion.as_ref()
        .ok_or("council_task_denied: native Council has no terminal completion.")?;
    if proof.session_id != session_id || proof.cycle_id != inspected.request.cycle_id
        || proof.team_revision != native.team_revision
    {
        return Err("council_task_stale: native completion has different ownership.".into());
    }
    let evidence = serde_json::to_vec(&(&inspected.request, proof))
        .map_err(|e| e.to_string())?;
    Ok(NativeTaskCouncilReceipt {
        schema_version: 1, request_id: request_id.to_owned(),
        session_id: session_id.to_owned(), cycle_id: proof.cycle_id.clone(),
        team_revision: proof.team_revision, outcome: "passed".into(),
        lowest_verified_score: proof.lowest, evidence_sha256: digest(&evidence),
    })
}

/// Durable one-winner terminal result, checked against the live native gate.
/// This never publishes, approves on behalf of a user or dispatches a callback.
pub(crate) fn record_native_terminal(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeTaskCouncilReceipt> {
    let expected = native_terminal_expected(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    let (raw, mut record) = native_task_record(store, session_id, assignment_key)?;
    if let Some(stored) = record.receipt {
        return if stored == expected { Ok(stored) } else {
            Err("council_task_stale: previous terminal receipt differs from current native proof.".into())
        };
    }
    record.receipt = Some(expected.clone());
    let updated = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    let changed = store.db.execute(
        "UPDATE architect_requests SET content=?1 WHERE parent_session_id=?2
         AND key=?3 AND kind='council_native_task' AND worker_id=?2 AND content=?4",
        params![updated, session_id, native_task_key(assignment_key)?, raw],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("council_task_conflict: concurrent terminal settlement changed the request.".into());
    }
    Ok(expected)
}

pub(crate) fn read_native_terminal(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeTaskCouncilReceipt> {
    let (_, record) = native_task_record(store, session_id, assignment_key)?;
    let stored = record.receipt
        .ok_or("council_task_denied: no verified terminal receipt is stored.")?;
    let expected = native_terminal_expected(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    if stored != expected {
        return Err("council_task_stale: terminal receipt no longer matches native evidence.".into());
    }
    Ok(stored)
}
