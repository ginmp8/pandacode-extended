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

fn frozen_config(store: &Store, session_id: &str) -> Result<(CouncilConfig, bool, i64)> {
    if super::parent_of(store, session_id)?.is_some() {
        return Err("council_task_denied: a reviewer worker cannot own a root request.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native Council task was not configured.")?;
    let config = team.config.council
        .ok_or("council_task_denied: task Council is disabled.")?;
    Ok((config, team.council_since == 0 && team.workers.is_empty(), team.council_since))
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
    let (config, fresh, _) = frozen_config(store, &scope.session_id)?;
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

/// Recheck the frozen native delivery identity without constructing an entire
/// Council inspector. Compact Channel rows use this same authority boundary.
fn validated_channel_request(
    store: &Store, request_id: &str, delivery_id: &str, session_id: &str,
) -> Result<CouncilTaskReviewRequest> {
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
    let (config, fresh, cycle_since) = frozen_config(store, session_id)?;
    // The original request also owns one native Council generation. A later
    // root cycle must not expose its reviewers or terminal proof through an old
    // Channel delivery, even if objective and policy still match.
    let frozen_revision: String = store.db.query_row(
        "SELECT artifact_revision FROM extended_channel_council_deliveries
         WHERE delivery_id=?1 AND channel_id=?2 AND source_message_id=?3 AND executor_bot_id=?4",
        params![delivery_id, &scope.channel_id, &scope.source_message_id, &scope.executor_bot_id],
        |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("council_task_denied: frozen Channel review cycle is missing.")?;
    let matches_cycle = if cycle_since == 0 {
        fresh && frozen_revision == "0"
    } else {
        cycle_since > 0 && frozen_revision == format!("cycle:{cycle_since}")
    };
    if !matches_cycle {
        return Err("council_task_stale: Channel delivery is bound to another native Council cycle.".into());
    }
    let current = request_for(&scope, delivery_id, &config, recorded.request_id.clone())?;
    if current != recorded {
        return Err("council_task_stale: source, objective, cycle or policy has changed.".into());
    }
    Ok(recorded)
}

/// Read-only and fail-closed. A stale, missing or malformed native request
/// cannot expose task-private reviewer data through a Channel summary.
pub(crate) fn channel_request_is_current(store: &Store, delivery_id: &str, session_id: &str) -> bool {
    let issued: Option<String> = match store.db.query_row(
        "SELECT request_id FROM extended_council_task_requests
         WHERE delivery_id=?1 AND session_id=?2",
        params![delivery_id,session_id], |row| row.get(0),
    ).optional() {
        Ok(issued) => issued,
        Err(_) => return false,
    };
    issued.as_deref().is_some_and(|request_id| {
        validated_channel_request(store, request_id, delivery_id, session_id).is_ok()
    })
}

/// Native-only scoped read; callers must know this request's exact delivery
/// and session, which are revalidated against persisted ownership and pins.
pub(crate) fn inspect_channel(store: &Store, request_id: &str, delivery_id: &str, session_id: &str) -> Result<CouncilTaskInspection> {
    let recorded = validated_channel_request(store, request_id, delivery_id, session_id)?;
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
    /// Explicit native task correlation for new receipts. Legacy v1 receipts
    /// deserialize without these fields and remain verifiable as historical proof.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub assignment_key: String,
    /// Caller-supplied artifact correlation, not a host-verified source digest.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub policy_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_scope_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_round: Option<usize>,
}

/// Compare one immutable receipt with the same live native proof. V1 lacks the
/// explicit correlation fields but already hashes the exact request and proof;
/// a replay may not use V1 to bypass a changed cycle, revision or policy.
fn native_receipt_matches(stored: &NativeTaskCouncilReceipt, expected: &NativeTaskCouncilReceipt) -> bool {
    if stored.schema_version == 2 {
        return stored == expected;
    }
    if stored.schema_version != 1 {
        return false;
    }
    let mut legacy = expected.clone();
    legacy.schema_version = 1;
    legacy.project_id.clear();
    legacy.assignment_key.clear();
    legacy.artifact_sha256.clear();
    legacy.policy_sha256.clear();
    legacy.task_scope_sha256.clear();
    legacy.reviewed_round = None;
    stored == &legacy
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTaskCouncilInspection {
    pub request: NativeTaskCouncilRequest,
    pub native: council::CouncilInspection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_checks: Vec<NativeTaskHostCheckReceipt>,
}

/// A native host's completed manifest-declared check, correlated to an
/// admitted Council task. The artifact SHA is a caller-supplied correlation,
/// NOT a host-verified source tree or a Council approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTaskHostCheckReceipt {
    pub schema_version: u32,
    pub observation_id: String,
    pub request_id: String,
    pub session_id: String,
    pub cycle_id: String,
    pub policy_sha256: String,
    pub task_scope_sha256: String,
    pub artifact_correlation_sha256: String,
    pub check_name: String,
    pub manifest_sha256: String,
    pub observed_at: i64,
    pub check_passed: bool,
    pub output_truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeTaskRecord {
    request: NativeTaskCouncilRequest,
    receipt: Option<NativeTaskCouncilReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_checks: Vec<NativeTaskHostCheckReceipt>,
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
    // Any Channel-owned task has a different per-message authority boundary,
    // even when the original channel delivery did not opt into Council.
    let channel_count: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_deliveries WHERE session_id=?1",
        [session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if channel_count != 0 {
        return Err("council_task_denied: Channel reviews require channel-scoped admission.".into());
    }
    let config = team.config.council
        .ok_or("council_task_denied: native Council is disabled.")?;
    // Revalidate the persisted task policy at this trusted admission/read
    // boundary. A malformed or downgraded Council policy must not mint a
    // reusable task request even if the original root cycle is still active.
    config.validate().map_err(|reason|
        format!("council_task_denied: invalid native Council policy: {reason}"))?;
    // V2 binds the task's effective native permission mode and checkout branch
    // as well as its harness and project. A V1 request is deliberately stale
    // rather than silently inheriting a different execution authority.
    let scope = serde_json::to_vec(&(
        &session.project_id, &session.worktree_path, &session.worktree_branch,
        &session.provider, &session.model, &session.reasoning_effort,
        &session.mode,
    )).map_err(|e| e.to_string())?;
    Ok(NativeTaskCouncilRequest {
        schema_version: 2, request_id, session_id: session_id.to_owned(),
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

/// After an atomic admission loses to another writer, return the winning
/// request only when every immutable task pin still matches. The winning
/// request ID is persisted by the other writer and must never be regenerated.
pub(super) fn resolve_native_admission_collision(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str,
) -> Result<NativeTaskCouncilRequest> {
    let (_, existing) = native_task_record(store, session_id, assignment_key)
        .map_err(|_| "council_task_conflict: no matching native task admission winner.".to_string())?;
    let current = native_task_candidate(
        store, session_id, project_id, assignment_key, artifact_sha256,
        existing.request.request_id.clone(),
    )?;
    if existing.request != current {
        return Err("council_task_conflict: native task admission winner has different task, cycle, policy or artifact.".into());
    }
    Ok(existing.request)
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
        "SELECT content FROM architect_requests WHERE parent_session_id=?1 AND key=?2
         AND kind='council_native_task' AND worker_id=?1",
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
    // A user-accepted Council cycle is terminal even if its TeamSnapshot still
    // says running. An existing request above stays idempotently inspectable.
    if team.council_accepted == Some(team.council_since) {
        return Err("council_task_denied: user-accepted Council cannot admit a new native task.".into());
    }
    if team.status != "running" {
        return Err("council_task_denied: new requests require an active root Council execution.".into());
    }
    // The team snapshot can remain "running" after the session is stopped,
    // archived or completed. Do not mint new task authority from stale state.
    // Identical requests above remain idempotently readable after settlement.
    let session = store.session(session_id)?;
    if session.archived || session.task_stage == "completed"
        || matches!(session.status.as_str(), "interrupted" | "error" | "completed" | "stopped" | "paused")
    {
        return Err("council_task_denied: new requests require a live root session.".into());
    }
    if team.workers.iter().any(|worker|
        worker.created_at >= team.council_since && worker.council.is_some())
    {
        return Err("council_task_denied: admission after reviewer dispatch is not allowed.".into());
    }
    let body = serde_json::to_string(&NativeTaskRecord {
        request: candidate.clone(), receipt: None, host_checks: Vec::new(),
    }).map_err(|e| e.to_string())?;
    // One immutable reviewed artifact and assignment per root Council cycle.
    // An older request for a different cycle remains historical, but a second
    // assignment in the current cycle must not inherit the same native verdict.
    // The SQL predicate makes this guard atomic across SQLite connections.
    let inserted = store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
         SELECT ?1,?2,'council_native_task',?3,?1
         WHERE NOT EXISTS (
             SELECT 1 FROM architect_requests
              WHERE parent_session_id=?1 AND kind='council_native_task'
                AND (json_extract(content,'$.request.cycleId')=?4
                     OR json_type(content,'$.request.cycleId') IS NULL)
         )",
        params![session_id, key, body, &candidate.cycle_id],
    ).map_err(|e| format!("council_task_conflict: {e}"))?;
    if inserted != 1 {
        // SELECT and INSERT are not one transaction across independently
        // opened SQLite connections. A concurrent identical caller may have
        // won the reservation; a different assignment must still fail closed.
        return resolve_native_admission_collision(
            store, session_id, project_id, assignment_key, artifact_sha256,
        ).map_err(|_| "council_task_conflict: one root Council cycle cannot authorize multiple task assignments or artifacts.".to_owned());
    }
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
    for check in &record.host_checks {
        if !native_host_check_is_current(check, &current) {
            return Err("council_task_stale: stored native host check has invalid task provenance.".into());
        }
    }
    Ok(NativeTaskCouncilInspection { request: current, native, host_checks: record.host_checks })
}


fn native_host_check_is_current(
    receipt: &NativeTaskHostCheckReceipt, request: &NativeTaskCouncilRequest,
) -> bool {
    receipt.schema_version == 1
        && !receipt.observation_id.trim().is_empty()
        && receipt.request_id == request.request_id
        && receipt.session_id == request.session_id
        && receipt.cycle_id == request.cycle_id
        && receipt.policy_sha256 == request.policy_sha256
        && receipt.task_scope_sha256 == request.task_scope_sha256
        && receipt.artifact_correlation_sha256 == request.artifact_sha256
        && matches!(receipt.check_name.as_str(), "npm test" | "cargo check")
        && receipt.manifest_sha256.len() == 64
        && receipt.manifest_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && receipt.observed_at > 0
}

/// Observe an already-admitted non-Channel root task before invoking the
/// optional user-enabled host check. A later Council cycle cannot inherit
/// this binding. Legacy ordinary chats continue without a task receipt.
pub(crate) fn active_native_task_for_checks(
    store: &Store, session_id: &str,
) -> Result<Option<NativeTaskCouncilRequest>> {
    let Some(team) = super::load(store, session_id)? else { return Ok(None) };
    if team.council_since <= 0 { return Ok(None) }
    let cycle_id = format!("{}:{}", session_id, team.council_since);
    let mut stmt = store.db.prepare(
        "SELECT content FROM architect_requests
         WHERE parent_session_id=?1 AND worker_id=?1 AND kind='council_native_task'",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let records = rows.collect::<rusqlite::Result<Vec<String>>>()
        .map_err(|e| e.to_string())?;
    let mut selected: Option<NativeTaskCouncilRequest> = None;
    for raw in records {
        let record: NativeTaskRecord = serde_json::from_str(&raw)
            .map_err(|_| "council_task_denied: stored native task request is invalid.")?;
        if record.request.cycle_id != cycle_id { continue }
        if selected.replace(record.request).is_some() {
            return Err("council_task_conflict: multiple tasks claim the same root Council cycle.".into());
        }
    }
    let Some(request) = selected else { return Ok(None) };
    // A previous task can be stale while the user's ordinary turn checks
    // remain enabled. Never turn stale task evidence into a new receipt, but
    // do not deny the original non-Council check merely for historical drift.
    if inspect_native_task(
        store, session_id, &request.project_id, &request.assignment_key,
        &request.artifact_sha256, &request.request_id,
    ).is_err() {
        return Ok(None);
    }
    Ok(Some(request))
}

/// Persist one *observed* host check using the pre-execution task binding.
/// Only run_turn_checks invokes this after a real process completes. Native
/// CAS and live request revalidation prevent post-run policy/cycle drift,
/// competing receipt writes or arbitrary reviewer-script execution.
pub(crate) fn record_native_host_check(
    store: &Store, request: &NativeTaskCouncilRequest, report: &serde_json::Value,
) -> Result<NativeTaskHostCheckReceipt> {
    let project = crate::checks::eligible_turn_check_project(store, &request.session_id)?;
    if project != request.project_id {
        return Err("council_task_stale: check owner changed projects.".into());
    }
    let current = inspect_native_task(
        store, &request.session_id, &request.project_id, &request.assignment_key,
        &request.artifact_sha256, &request.request_id,
    )?;
    if current.request != *request {
        return Err("council_task_stale: the check was not run for this frozen Council task.".into());
    }
    let observed = report.get("hostObservation")
        .ok_or("council_task_denied: no completed native check observation.")?;
    let check_name = observed["name"].as_str()
        .ok_or("council_task_denied: check name is missing.")?;
    let manifest_sha256 = observed["manifestSha256"].as_str()
        .ok_or("council_task_denied: checked manifest digest is missing.")?;
    let observed_at = observed["observedAt"].as_i64()
        .ok_or("council_task_denied: host check timestamp is missing.")?;
    let check_passed = observed["passed"].as_bool()
        .ok_or("council_task_denied: native check result is missing.")?;
    let output_truncated = observed["outputTruncated"].as_bool()
        .ok_or("council_task_denied: native output status is missing.")?;
    if report["checked"] != true
        || report["ok"] != check_passed
        || observed["kind"] != "manifest_declared_check"
        || observed["scope"] != "manifest_only_not_artifact_verification"
        || observed["provenance"] != "native_host"
    {
        return Err("council_task_denied: native check provenance is inconsistent.".into());
    }
    let receipt = NativeTaskHostCheckReceipt {
        schema_version: 1, observation_id: storage::id(),
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        cycle_id: request.cycle_id.clone(),
        policy_sha256: request.policy_sha256.clone(),
        task_scope_sha256: request.task_scope_sha256.clone(),
        artifact_correlation_sha256: request.artifact_sha256.clone(),
        check_name: check_name.to_owned(),
        manifest_sha256: manifest_sha256.to_owned(),
        observed_at, check_passed, output_truncated,
    };
    if !native_host_check_is_current(&receipt, request) {
        return Err("council_task_denied: malformed host check evidence.".into());
    }
    let (old, mut record) = native_task_record(store, &request.session_id, &request.assignment_key)?;
    if record.request != *request || record.receipt.is_some() {
        return Err("council_task_stale: task has changed or already settled.".into());
    }
    if record.host_checks.len() >= 16 {
        return Err("council_task_denied: maximum retained native checks reached.".into());
    }
    record.host_checks.push(receipt.clone());
    let updated = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    let changed = store.db.execute(
        "UPDATE architect_requests SET content=?1
         WHERE parent_session_id=?2 AND worker_id=?2
           AND key=?3 AND kind='council_native_task' AND content=?4",
        params![updated, &request.session_id, native_task_key(&request.assignment_key)?, old],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("council_task_conflict: concurrent native task receipt change.".into());
    }
    Ok(receipt)
}

fn native_terminal_expected(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
) -> Result<NativeTaskCouncilReceipt> {
    let inspected = inspect_native_task(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id,
    )?;
    // Legacy/conflicting durable records must also fail closed at terminal
    // settlement. An old multi-request cycle cannot reuse one root pass for
    // another assignment merely because both requests predate admission guards.
    let related_requests: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM architect_requests
         WHERE parent_session_id=?1 AND kind='council_native_task'
           AND (json_extract(content,'$.request.cycleId')=?2
                OR json_type(content,'$.request.cycleId') IS NULL)",
        params![session_id, &inspected.request.cycle_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if related_requests != 1 {
        return Err("council_task_conflict: root Council cycle has multiple or invalid task assignments.".into());
    }
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
        schema_version: 2, request_id: request_id.to_owned(),
        session_id: session_id.to_owned(), cycle_id: proof.cycle_id.clone(),
        team_revision: proof.team_revision, outcome: "passed".into(),
        lowest_verified_score: proof.lowest, evidence_sha256: digest(&evidence),
        project_id: inspected.request.project_id.clone(),
        assignment_key: inspected.request.assignment_key.clone(),
        artifact_sha256: inspected.request.artifact_sha256.clone(),
        policy_sha256: inspected.request.policy_sha256.clone(),
        task_scope_sha256: inspected.request.task_scope_sha256.clone(),
        reviewed_round: Some(proof.round),
    })
}

/// A losing compare-and-swap may observe an identical receipt written by the
/// winning task consumer. Revalidate its current native Council proof before
/// treating that concurrent settlement as an idempotent success.
pub(super) fn resolve_native_terminal_collision(
    store: &Store, session_id: &str, project_id: &str,
    assignment_key: &str, artifact_sha256: &str, request_id: &str,
    expected: &NativeTaskCouncilReceipt,
) -> Result<NativeTaskCouncilReceipt> {
    let winner = read_native_terminal(
        store, session_id, project_id, assignment_key, artifact_sha256, request_id,
    ).map_err(|_| "council_task_conflict: concurrent terminal settlement has no valid matching proof.".to_string())?;
    if !native_receipt_matches(&winner, expected) {
        return Err("council_task_conflict: concurrent terminal settlement changed the native evidence.".into());
    }
    Ok(winner)
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
        return if native_receipt_matches(&stored, &expected) { Ok(stored) } else {
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
        return resolve_native_terminal_collision(
            store, session_id, project_id, assignment_key, artifact_sha256,
            request_id, &expected,
        );
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
    if !native_receipt_matches(&stored, &expected) {
        return Err("council_task_stale: terminal receipt no longer matches native evidence.".into());
    }
    Ok(stored)
}
