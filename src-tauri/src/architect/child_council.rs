//! Task-isolated reservations for hierarchical Council child Architects.
//! A reservation alone grants no reviewer authority. A trusted root-only
//! barrier admits independent native read-only leaf sessions after writers
//! settle, but these results cannot satisfy the root without a verified
//! authenticated composite receipt.
use super::{council::{CouncilConfig, CouncilNestedReviewer, CouncilSeat}, reviewer_security::{self, ReviewerBinding, ReviewerToolPolicy}, Admission, Launch, TeamConfig, TeamSnapshot, Worker};
use crate::{bots, storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sha2::{Digest, Sha256};

const RESERVATION_DESCRIPTION: &str = "Task-owned read-only Council reservation; reviewer dispatch is not yet enabled.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrozenChildReservation {
    schema_version: u32,
    root_session_id: String,
    root_cycle_id: String,
    bot_id: String,
    // Prevent a deleted and recreated bot ID from inheriting task authority.
    bot_created_at: i64,
    child_session_id: String,
    objective_sha256: String,
    policy_sha256: String,
    child_config_sha256: String,
    /// Digest of the immutable task session's project, checkout and harness identity.
    #[serde(default)]
    session_scope_sha256: String,
    /// Hashed bot review identity and capability references; no profile or secrets copied.
    #[serde(default)]
    bot_profile_sha256: String,
}

/// Inspection reveals a pending native child, never a synthetic leaf result.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildReservationView {
    pub bot_id: String,
    pub child_session_id: String,
    pub root_cycle_id: String,
    pub required_leaf_reviewers: usize,
    pub status: &'static str,
    /// Issued native child leaves only; reservations are not measured work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_leaf_usage: Option<super::council::CouncilUsageTotals>,
    /// Native verified leaf result, not a verified composite parent vote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_child_receipt: Option<VerifiedChildReceipt>,
    /// Native correlation to exactly one child receipt, not a passed root vote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authenticated_parent_seat: Option<AuthenticatedParentSeat>,
}

fn digest(data: &[u8]) -> String { format!("{:x}", Sha256::digest(data)) }

fn key(root: &TeamSnapshot, bot_id: &str) -> String {
    format!("native-council-child:{}:{bot_id}", root.council_since)
}

fn child_config(root: &CouncilConfig, child: &CouncilNestedReviewer) -> Result<TeamConfig> {
    // Every user-required root lens remains mandatory within this child.
    let mut council = CouncilConfig {
        target: root.target.max(child.child_target),
        max_rounds: child.child_max_rounds,
        reviewers: child.child_reviewers,
        criteria_source: root.criteria_source,
        criteria: root.criteria.clone(),
        ..CouncilConfig::default()
    };
    // This is a single automatic native Council, never recursive or a
    // persistent selected-bot chat. No inherited skill or write tool grants.
    council.nested_bot_reviewers.clear();
    council.validate()?;
    let config = TeamConfig {
        auto: true, roster_only: false, name: None,
        workers: Vec::new(), catalog: Vec::new(),
        max_parallel: child.child_reviewers.min(4),
        max_rounds: child.child_max_rounds, max_assignments: 0,
        workspace: "shared".into(), summary_limit: 0,
        lead_guidance: None, council: Some(council),
    };
    config.validate()?;
    Ok(config)
}

fn frozen(
    store: &Store, root: &TeamSnapshot, child: &CouncilNestedReviewer,
    session_id: &str, child_config: &TeamConfig,
) -> Result<FrozenChildReservation> {
    let council = root.config.council.as_ref().ok_or("Council is off.")?;
    let parent = store.session(&root.session_id)?;
    let session = store.session(session_id)?;
    // A reservation must never resume an archived/completed session through a
    // fresh parent review. This is enforced on every inspection and retry.
    if session.archived || session.task_stage == "completed" {
        return Err("child_council_revoked: reserved child task is archived or completed.".into());
    }
    if session.project_id != parent.project_id || session.worktree_path != parent.worktree_path
        || session.mode != "plan"
    {
        return Err("child_council_stale: child project, checkout or permission mode changed.".into());
    }
    let bot = bots::get(store, &child.bot_id)
        .map_err(|_| "child_council_revoked: selected nested bot was removed.")?;
    if bot.paused {
        return Err("child_council_revoked: selected nested bot is paused.".into());
    }
    if bot.project_id != parent.project_id {
        return Err("child_council_denied: selected nested bot belongs to another project.".into());
    }
    // A changed bot profile must not silently replace the pinned child harness.
    // Revocation denies this reservation; it never resumes with broader authority.
    if bot.provider != session.provider || bot.model != session.model
        || session.reasoning_effort.as_deref().unwrap_or_default() != bot.reasoning_effort
    {
        return Err("child_council_revoked: selected bot review harness changed.".into());
    }
    let scope = serde_json::to_vec(&(
        &session.project_id, &session.worktree_path, &session.provider,
        &session.model, &session.reasoning_effort,
    )).map_err(|e| e.to_string())?;
    // A pending child must never be admitted under an edited coordinator persona,
    // selected Skills or MCP references. Hash-only pins avoid persisting prompts,
    // profile contents or credentials in the native reservation record.
    let profile = serde_json::to_vec(&(
        &bot.name, &bot.role, &bot.instructions, &bot.goals, &bot.soul,
        &bot.mode, &bot.skills, &bot.mcp, &bot.backup_models, &bot.fallback_mode,
    )).map_err(|e| e.to_string())?;
    Ok(FrozenChildReservation {
        schema_version: 4,
        root_session_id: root.session_id.clone(),
        root_cycle_id: format!("{}:{}", root.session_id, root.council_since),
        bot_id: child.bot_id.clone(),
        bot_created_at: bot.created_at,
        child_session_id: session_id.to_owned(),
        objective_sha256: digest(root.objective.as_bytes()),
        policy_sha256: digest(&serde_json::to_vec(council).map_err(|e| e.to_string())?),
        child_config_sha256: digest(&serde_json::to_vec(child_config).map_err(|e| e.to_string())?),
        session_scope_sha256: digest(&scope),
        bot_profile_sha256: digest(&profile),
    })
}

fn read(store: &Store, root: &TeamSnapshot, child: &CouncilNestedReviewer) -> Result<Option<FrozenChildReservation>> {
    let row: Option<(String, String)> = store.db.query_row(
        "SELECT content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2 AND kind='council_child'",
        params![root.session_id, key(root, &child.bot_id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((body, child_session_id)) = row else { return Ok(None) };
    let record: FrozenChildReservation = serde_json::from_str(&body)
        .map_err(|_| "child_council_stale: malformed frozen child reservation.")?;
    if record.child_session_id != child_session_id ||
        super::parent_of(store, &child_session_id)?.as_deref() != Some(root.session_id.as_str()) {
        return Err("child_council_stale: native child owner or session changed.".into());
    }
    // An authenticated child session is exactly one composite parent seat.
    // architect_workers already guarantees unique session ownership, but the
    // task-scoped requests table has no UNIQUE(worker_id) constraint. Reject a
    // forged second child record claiming the same session before any leaf work.
    let claims: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM architect_requests
         WHERE parent_session_id=?1 AND kind='council_child' AND worker_id=?2",
        params![root.session_id, child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if claims != 1 {
        return Err("child_council_stale: child session is claimed by multiple Council seats.".into());
    }
    let config = child_config(root.config.council.as_ref().ok_or("Council is off.")?, child)?;
    if record != frozen(store, root, child, &child_session_id, &config)? {
        return Err("child_council_stale: child policy, objective or parent cycle changed.".into());
    }
    let stored = super::load(store, &child_session_id)?
        .ok_or("child_council_stale: child Architect state is missing.")?;
    // While reservations are blocked, even one ordinary chat message means
    // this is no longer an isolated, unused child review context. Fail closed
    // instead of inheriting conversation content when native leaves are enabled.
    let chat_messages: i64 = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM messages WHERE session_id=?1)",
        [&child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if chat_messages != 0 {
        return Err("child_council_stale: reserved child contains unrelated chat history.".into());
    }
    // A blocked reservation cannot inherit a forged task description, native
    // dispatch record or sub-worker ownership. All three must be admitted by
    // the future task-scoped native child lifecycle, never by a bot chat.
    let task_description: String = store.db.query_row(
        "SELECT COALESCE(task_description, '') FROM sessions WHERE id=?1",
        [&child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if task_description != RESERVATION_DESCRIPTION {
        return Err("child_council_stale: reserved child task description changed.".into());
    }
    if stored.status == "running" {
        validate_admitted(store, root, child, &record, &stored)?;
        return Ok(Some(record));
    }
    let pending_native_requests: i64 = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM architect_requests WHERE parent_session_id=?1)",
        [&child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if pending_native_requests != 0 {
        return Err("child_council_stale: reserved child has unrelated native requests.".into());
    }
    let unadmitted_subworkers: i64 = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM architect_workers WHERE parent_session_id=?1)",
        [&child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if unadmitted_subworkers != 0 {
        return Err("child_council_stale: reserved child has unadmitted sub-workers.".into());
    }
    if stored.config != config || stored.status != "blocked" || !stored.workers.is_empty()
        || !stored.objective.is_empty() || stored.council_since != 0
        // A staged setup, prior summary or briefing can rehydrate unrelated
        // bot context/permissions even though the child still says "blocked".
        || stored.pending_setup.is_some() || stored.summary.is_some()
        || !stored.briefings.is_empty()
        || stored.generation != 0 || stored.pending_turn.is_some()
        || stored.council_accepted.is_some()
        || store.session(&child_session_id)?.mode != "plan"
    {
        return Err("child_council_stale: child state or permissions have changed.".into());
    }
    Ok(Some(record))
}

// NATIVE LEAF RUNTIME

/// Resolve task descendants through native ownership, never a bot's editable
/// roster or self-reported parent. This is deliberately limited to depth two.
fn leaf_sessions(store: &Store, root: &str) -> Result<Vec<String>> {
    let mut statement = store.db.prepare(
        "SELECT leaf.session_id FROM architect_workers AS child
         JOIN architect_workers AS leaf ON leaf.parent_session_id=child.session_id
         WHERE child.parent_session_id=?1",
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([root], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let sessions = rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(sessions)
}

/// An incomplete native stop must not let a resumed root overtake still-live
/// child reviewers. Unlike task status, live agent registry entries cannot be
/// forged from a model's conversation.
pub(super) fn active_leaf_agents(
    store: &Store, root: &str,
    agents: &std::collections::HashMap<String, crate::RunningAgent>,
) -> Result<bool> {
    Ok(leaf_sessions(store, root)?.iter().any(|id| agents.contains_key(id)))
}

/// Propagate root pause/stop to all leaf processes even if a child snapshot is
/// corrupt; authorization checks then revoke their late completion callbacks.
pub(super) fn stop_leaves(
    store: &Store, root: &TeamSnapshot,
    agents: &std::collections::HashMap<String, crate::RunningAgent>,
) -> Result<()> {
    for id in leaf_sessions(store, &root.session_id)? {
        if let Some(agent) = agents.get(&id) {
            let _ = agent.send(crate::RunInput::Stop);
        }
    }
    if let Some(config) = root.config.council.as_ref() {
        for choice in &config.nested_bot_reviewers {
            if let Ok(Some(record)) = read(store, root, choice) {
                if let Ok(Some(mut team)) = super::load(store, &record.child_session_id) {
                    for worker in &mut team.workers {
                        if super::stoppable(&worker.status) {
                            worker.status = "stopped".into();
                            worker.updated_at = storage::now();
                        }
                    }
                    // A failed persistence still leaves native process stops
                    // issued. The next admission rechecks the frozen grant.
                    if let Err(error) = super::save(store, &mut team) {
                        eprintln!("Stopping child Council {}: {error}", team.session_id);
                    }
                }
            }
        }
    }
    Ok(())
}

/// A child may run only these native-issued leaves; the common Architect
/// dispatcher remains unavailable to every task-owned child coordinator.
fn validate_admitted(
    store: &Store, root: &TeamSnapshot, choice: &CouncilNestedReviewer,
    record: &FrozenChildReservation, team: &TeamSnapshot,
) -> Result<()> {
    let invalid = || "child_council_stale: native leaf admission no longer matches its frozen task.";
    if team.status != "running" || team.generation != root.generation
        || team.council_since != root.council_since || team.objective != root.objective
        || team.config != child_config(root.config.council.as_ref().ok_or("Council is off.")?, choice)?
        || team.workers.len() != choice.child_reviewers
        || team.pending_turn.is_some() || team.pending_setup.is_some()
        || team.summary.is_some() || team.council_accepted.is_some()
    {
        return Err(invalid().into());
    }
    let requests: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1",
        [&record.child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let owned: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM architect_workers WHERE parent_session_id=?1",
        [&record.child_session_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if requests != choice.child_reviewers as i64 || owned != choice.child_reviewers as i64 {
        return Err(invalid().into());
    }
    let coordinator = store.session(&record.child_session_id)?;
    let mut ids = HashSet::new();
    let mut sessions = HashSet::new();
    for leaf in &team.workers {
        let session = store.session(&leaf.session_id)?;
        if !ids.insert(&leaf.id) || !sessions.insert(&leaf.session_id)
            || super::parent_of(store, &leaf.session_id)?.as_deref()
                != Some(record.child_session_id.as_str())
            || session.project_id != coordinator.project_id
            || session.worktree_path != coordinator.worktree_path
            || session.mode != "plan" || session.archived || session.task_stage == "completed"
            || leaf.provider != session.provider || leaf.model != session.model
            || !leaf.read_only || leaf.wrote_at != 0 || !leaf.files.is_empty()
            || leaf.parent_worker.is_some() || leaf.abandoned
            || !matches!(leaf.status.as_str(), "queued" | "running" | "completed" | "failed" | "stopped")
            || leaf.council.as_ref().is_none_or(|seat| seat.round != 1)
            || super::parent_of(store, &record.child_session_id)?.as_deref()
                != Some(root.session_id.as_str())
        {
            return Err(invalid().into());
        }
        let binding = leaf.reviewer_binding.as_ref().ok_or_else(invalid)?;
        if binding.reviewer_bot_id.is_some()
            || binding.snapshot.tool_policy != ReviewerToolPolicy::NativeReadOnly
            || binding.validate(team, leaf).is_err()
            || reviewer_security::validate_recorded_authority(
                store, &record.child_session_id, binding,
            ).is_err()
        {
            return Err(invalid().into());
        }
        let grandchild: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM architect_workers WHERE parent_session_id=?1",
            [&leaf.session_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if grandchild != 0 {
            return Err("child_council_denied: a leaf reviewer cannot create grandchildren.".into());
        }
    }
    Ok(())
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedChildReceipt {
    pub root_cycle_id: String,
    pub child_session_id: String,
    pub bot_id: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
    pub child_config_sha256: String,
    pub lowest_score: f64,
    pub leaf_worker_ids: Vec<String>,
}

/// A task-scoped composite seat backed by a revalidated real child Council.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthenticatedParentSeat {
    pub root_session_id: String,
    pub root_cycle_id: String,
    pub parent_worker_id: String,
    pub child_session_id: String,
    pub bot_id: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
    pub child_receipt_sha256: String,
    pub lowest_score: f64,
}

fn parent_seat_key(root: &TeamSnapshot, bot_id: &str) -> String {
    format!("native-council-parent-seat:{}:{bot_id}", root.council_since)
}

fn receipt_key(root: &TeamSnapshot, bot_id: &str) -> String {
    format!("native-council-child-receipt:{}:{bot_id}", root.council_since)
}

/// Construct child evidence only from settled, freshly scored native leaves.
/// The root Council remains blocked until its separate composite-seat gate exists.
fn verified_receipt(
    store: &Store, record: &FrozenChildReservation, child: &TeamSnapshot,
) -> Result<Option<VerifiedChildReceipt>> {
    if child.workers.is_empty() || child.workers.iter().any(|leaf| leaf.status != "completed") {
        return Ok(None);
    }
    let Some(status) = super::council::status(child) else { return Ok(None) };
    if status.state != "passed" || status.stale || status.round != 1
        || !super::council_evidence_is_current(store, child)
    {
        return Ok(None);
    }
    let mut lowest_score = f64::INFINITY;
    let mut leaf_worker_ids = Vec::with_capacity(child.workers.len());
    for leaf in &child.workers {
        let Some(seat) = leaf.council.as_ref() else { return Ok(None) };
        let Some(score) = seat.score else { return Ok(None) };
        let Some(binding) = leaf.reviewer_binding.as_ref() else { return Ok(None) };
        if !leaf.read_only || leaf.abandoned || leaf.wrote_at != 0
            || !leaf.files.is_empty() || seat.round != 1 || seat.started_at <= 0
            || seat.reported_severe_finding || seat.reported_conflict
            || super::council::latest_score(store, &leaf.session_id) != Some(score)
            || binding.validate(child, leaf).is_err()
            || reviewer_security::validate_recorded_authority(
                store, &child.session_id, binding,
            ).is_err()
        {
            return Ok(None);
        }
        lowest_score = lowest_score.min(score);
        leaf_worker_ids.push(leaf.id.clone());
    }
    if status.lowest != Some(lowest_score) { return Ok(None); }
    Ok(Some(VerifiedChildReceipt {
        root_cycle_id: record.root_cycle_id.clone(),
        child_session_id: record.child_session_id.clone(),
        bot_id: record.bot_id.clone(),
        objective_sha256: record.objective_sha256.clone(),
        policy_sha256: record.policy_sha256.clone(),
        child_config_sha256: record.child_config_sha256.clone(),
        lowest_score, leaf_worker_ids,
    }))
}

/// An existing receipt is never accepted on its own: rederive the exact
/// authenticated evidence from current immutable child inputs and leaf callbacks.
fn read_verified_receipt(
    store: &Store, root: &TeamSnapshot, record: &FrozenChildReservation,
    child: &TeamSnapshot,
) -> Result<Option<VerifiedChildReceipt>> {
    let existing: Option<String> = store.db.query_row(
        "SELECT content FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2 AND kind='council_child_receipt'
         AND worker_id=?3",
        params![root.session_id, receipt_key(root, &record.bot_id), record.child_session_id],
        |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let Some(existing) = existing else { return Ok(None) };
    let stored: VerifiedChildReceipt = serde_json::from_str(&existing)
        .map_err(|_| "child_council_stale: malformed child evidence receipt.")?;
    if verified_receipt(store, record, child)?.as_ref() != Some(&stored) {
        return Err("child_council_stale: child receipt no longer matches native reviewer evidence.".into());
    }
    Ok(Some(stored))
}

/// Persist exactly one child evidence receipt per frozen root cycle and seat.
/// An out-of-order retry must match the existing content rather than overwrite it.
fn record_verified_receipt(
    store: &Store, root: &TeamSnapshot, record: &FrozenChildReservation,
    child: &TeamSnapshot,
) -> Result<bool> {
    let Some(candidate) = verified_receipt(store, record, child)? else {
        // A previously recorded receipt cannot survive revoked or altered evidence.
        if read_verified_receipt(store, root, record, child)?.is_some() {
            return Err("child_council_stale: completed child evidence was revoked.".into());
        }
        return Ok(false);
    };
    if let Some(previous) = read_verified_receipt(store, root, record, child)? {
        if previous != candidate {
            return Err("child_council_stale: child receipt replay differs.".into());
        }
        return Ok(false);
    }
    super::record_request(
        store, &root.session_id, &receipt_key(root, &record.bot_id),
        "council_child_receipt",
        &serde_json::to_string(&candidate).map_err(|e| e.to_string())?,
        &record.child_session_id,
    )?;
    Ok(true)
}

/// Bind the real child receipt to one root-owned native worker identity.
fn parent_seat(
    store: &Store, root: &TeamSnapshot, record: &FrozenChildReservation,
    child: &TeamSnapshot, persist: bool,
) -> Result<Option<AuthenticatedParentSeat>> {
    let receipt = read_verified_receipt(store, root, record, child)?;
    let existing: Option<(String, String)> = store.db.query_row(
        "SELECT content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2 AND kind='council_parent_seat'",
        params![root.session_id, parent_seat_key(root, &record.bot_id)],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let candidate = if let Some(receipt) = receipt {
        let (count, worker): (i64, Option<String>) = store.db.query_row(
            "SELECT COUNT(*), MIN(worker_id) FROM architect_workers
             WHERE parent_session_id=?1 AND session_id=?2",
            params![root.session_id, record.child_session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| e.to_string())?;
        if count != 1 {
            return Err("child_council_stale: composite root seat has invalid native ownership.".into());
        }
        Some(AuthenticatedParentSeat {
            root_session_id: root.session_id.clone(),
            root_cycle_id: record.root_cycle_id.clone(),
            parent_worker_id: worker.ok_or("child_council_stale: missing parent worker identity.")?,
            child_session_id: record.child_session_id.clone(),
            bot_id: record.bot_id.clone(),
            objective_sha256: record.objective_sha256.clone(),
            policy_sha256: record.policy_sha256.clone(),
            child_receipt_sha256: digest(&serde_json::to_vec(&receipt).map_err(|e| e.to_string())?),
            lowest_score: receipt.lowest_score,
        })
    } else { None };
    if let Some((content, owned_child)) = existing {
        let recorded: AuthenticatedParentSeat = serde_json::from_str(&content)
            .map_err(|_| "child_council_stale: malformed composite parent seat.")?;
        if owned_child != record.child_session_id || candidate.as_ref() != Some(&recorded) {
            return Err("child_council_stale: composite parent seat no longer matches native evidence.".into());
        }
        return Ok(Some(recorded));
    }
    if persist {
        if let Some(candidate) = candidate.as_ref() {
            super::record_request(
                store, &root.session_id, &parent_seat_key(root, &record.bot_id),
                "council_parent_seat",
                &serde_json::to_string(candidate).map_err(|e| e.to_string())?,
                &record.child_session_id,
            )?;
        }
    }
    Ok(candidate.filter(|_| persist))
}

/// Revalidate the *entire* frozen root composite roster before any consumer can
/// use it. Individual receipts prove leaves, but cannot prove that sibling
/// child seats are distinct or that an extra persisted parent seat was not
/// injected. Incomplete children return None; stale or forged state fails closed.
/// This is an evidence boundary, not a second Council verdict or an approval.
pub(super) fn verified_parent_seats(
    store: &Store, root: &TeamSnapshot,
) -> Result<Option<Vec<AuthenticatedParentSeat>>> {
    let Some(config) = root.config.council.as_ref() else { return Ok(None) };
    if config.nested_bot_reviewers.is_empty()
        || root.council_since <= 0 || root.objective.trim().is_empty()
    {
        return Ok(None);
    }
    crate::bots::shared_channels::validate_delivery_council(
        store, &root.session_id, config,
    )?;
    let mut workers = HashSet::new();
    let mut sessions = HashSet::new();
    let mut bots = HashSet::new();
    let mut ready = true;
    let mut seats = Vec::with_capacity(config.nested_bot_reviewers.len());
    for choice in &config.nested_bot_reviewers {
        let record = read(store, root, choice)?
            .ok_or("child_council_stale: required child reservation is missing.")?;
        let child = super::load(store, &record.child_session_id)?
            .ok_or("child_council_stale: required child Council is missing.")?;
        let Some(seat) = parent_seat(store, root, &record, &child, false)? else {
            ready = false;
            continue;
        };
        if seat.parent_worker_id.is_empty() || seat.child_session_id.is_empty()
            || !bots.insert(seat.bot_id.as_str())
            || !workers.insert(seat.parent_worker_id.as_str())
            || !sessions.insert(seat.child_session_id.as_str())
            || root.workers.iter().any(|worker|
                worker.id == seat.parent_worker_id || worker.session_id == seat.child_session_id)
        {
            return Err("child_council_stale: duplicate or conflicting root composite seat identity.".into());
        }
        seats.push(seat);
    }
    // Bind the record count to this cycle only. Old cycles are harmless,
    // but phantom seats in this cycle may not be silently ignored.
    let key_prefix = format!("native-council-parent-seat:{}:", root.council_since);
    let recorded: i64 = store.db.query_row(
        "SELECT COUNT(*) FROM architect_requests
         WHERE parent_session_id=?1 AND kind='council_parent_seat'
         AND substr(key,1,length(?2))=?2",
        params![root.session_id, key_prefix],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if recorded != seats.len() as i64 {
        return Err("child_council_stale: unexpected or unverifiable composite seat in root cycle.".into());
    }
    Ok(ready.then_some(seats))
}

/// Trusted root-only transition: allocate fresh read-only reviewer sessions
/// after the root's actual writing workers settle. No bot conversation, prompt
/// claim or persisted child reservation can request this operation.
pub(super) fn admit_leaves(store: &Store, root: &TeamSnapshot) -> Result<bool> {
    let Some(config) = root.config.council.as_ref() else { return Ok(false) };
    if config.nested_bot_reviewers.is_empty() { return Ok(false) }
    if root.council_since <= 0 || root.objective.trim().is_empty()
        || !super::active(&root.status)
        || root.workers.iter().any(|w| !w.read_only && super::unsettled(&w.status))
    {
        return Err("child_council_denied: root work must be settled before native child review.".into());
    }
    crate::bots::shared_channels::validate_delivery_council(store, &root.session_id, config)?;
    let mut created = false;
    for choice in &config.nested_bot_reviewers {
        let record = read(store, root, choice)?
            .ok_or("child_council_stale: expected native reservation is missing.")?;
        let mut child = super::load(store, &record.child_session_id)?
            .ok_or("child_council_stale: reserved child Architect is missing.")?;
        if child.status == "running" {
            created |= record_verified_receipt(store, root, &record, &child)?;
            let had_parent_seat = parent_seat(store, root, &record, &child, false)?.is_some();
            if parent_seat(store, root, &record, &child, true)?.is_some() && !had_parent_seat {
                created = true;
            }
            continue;
        }
        if child.status != "blocked" || !child.workers.is_empty() {
            return Err("child_council_stale: child review is not eligible for admission.".into());
        }
        let session = store.session(&record.child_session_id)?;
        super::validate_worker_role(&session.provider, true)?;
        child.status = "running".into();
        child.generation = root.generation;
        child.objective = root.objective.clone();
        child.council_since = root.council_since;
        let lenses: Vec<String> = config.criteria.iter().map(|criterion| criterion.lens.clone()).collect();
        for index in 0..choice.child_reviewers {
            let lens = lenses.get(index).cloned().unwrap_or_else(|| format!("Child review {}", index + 1));
            let assignment = format!(
                "Review the frozen task artifact independently for the {lens} criterion. \
                 You are a task-scoped Council leaf, never the coordinating bot. \
                 Inspect only; do not edit the checkout, access a bot conversation, \
                 invoke tools with side effects or spawn another agent. \
                 Report concrete findings and end with Council score: N/10. \
                 Task objective: {}",
                root.objective,
            );
            let leaf = store.create_session(&session.project_id, &session.provider, &session.model, "plan")?;
            let now = storage::now();
            store.db.execute(
                "UPDATE sessions SET title=?2,task_description=?3,worktree_path=?4,reasoning_effort=?5 WHERE id=?1",
                params![leaf.id, format!("Child Council reviewer {}", index + 1),
                    assignment, session.worktree_path, session.reasoning_effort],
            ).map_err(|e| e.to_string())?;
            let id = storage::id();
            store.db.execute(
                "INSERT INTO architect_workers(parent_session_id,worker_id,session_id) VALUES(?1,?2,?3)",
                params![record.child_session_id, id, leaf.id],
            ).map_err(|e| e.to_string())?;
            let seat = CouncilSeat {
                round: 1, lens, score: None, started_at: 0,
                closed_by_selection: false, reported_severe_finding: false, reported_conflict: false,
            };
            let binding = ReviewerBinding::native(&child, &id, now, &session.provider, &session.model, &seat);
            reviewer_security::record_native_authority(store, &record.child_session_id, &binding)?;
            child.workers.push(Worker {
                id, session_id: leaf.id, name: format!("Child reviewer {}", index + 1),
                profile_id: "native-child-reviewer".into(), provider: session.provider.clone(),
                model: session.model.clone(), reasoning_effort: session.reasoning_effort.clone(),
                status: "queued".into(), assignment, summary: String::new(), error: None,
                depends_on: Vec::new(), files: Vec::new(), read_only: true,
                attempt: 1, created_at: now, updated_at: now, keys: vec![format!("leaf-{}", index + 1)],
                abandoned: false, result_revision: 0, checkpoint: None, timed_out: false,
                failovers: 0, parent_worker: None, resume_prompt: None,
                tool_catalog_version: super::TOOL_CATALOG_VERSION, nudged: false,
                reported: false, council: Some(seat), reviewer_binding: Some(binding),
                score_nudged: false, wrote_at: 0,
            });
        }
        super::save(store, &mut child)?;
        // Validate the complete native issuance before the parent transaction
        // commits. A partial or forged set rolls back, never becoming a vote.
        read(store, root, choice)?
            .ok_or("child_council_stale: native child admission was not persisted.")?;
        created = true;
    }
    // A completed child cannot be used while any sibling composite identity
    // or extra persisted root seat conflicts with the frozen roster.
    let _ = verified_parent_seats(store, root)?;
    Ok(created)
}

/// Root orchestration stays parked while leaf processes are queued/running.
/// Without this guard the ordinary root scheduler could pause the task and
/// revoke an otherwise valid child callback before review completes.
pub(super) fn has_pending_leaves(store: &Store, root: &TeamSnapshot) -> Result<bool> {
    let Some(config) = root.config.council.as_ref() else { return Ok(false) };
    for choice in &config.nested_bot_reviewers {
        let Some(record) = read(store, root, choice)? else {
            return Err("child_council_stale: expected reservation is missing.".into());
        };
        let child = super::load(store, &record.child_session_id)?
            .ok_or("child_council_stale: task-owned child team is missing.")?;
        if child.status == "running" && child.workers.iter().any(|leaf|
            super::unsettled(&leaf.status)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Authorize only a native leaf session, never the child coordinator, through
/// the existing task launch/callback path. Every callback rechecks recorded
/// grants, frozen root cycle and current child/worker ownership.
pub(super) fn authorize_leaf(store: &Store, sid: &str, admission: &Admission) -> Result<()> {
    // A child coordinator is never its own reviewer, including when forged active.
    if sid == admission.parent {
        return Err("child_council_denied: a child coordinator cannot admit itself as a reviewer.".into());
    }
    let root_sid = super::parent_of(store, &admission.parent)?
        .ok_or("child_council_denied: no root cycle owns this child.")?;
    let root = super::load(store, &root_sid)?
        .ok_or("child_council_stale: root task no longer exists.")?;
    if !super::active(&root.status) {
        return Err("child_council_revoked: root task is no longer active.".into());
    }
    let config = root.config.council.as_ref().ok_or("Council is off.")?;
    let mut authorized = false;
    for choice in &config.nested_bot_reviewers {
        let Some(record) = read(store, &root, choice)? else {
            return Err("child_council_stale: child reservation is missing.".into());
        };
        if record.child_session_id == admission.parent {
            authorized = true;
        }
    }
    if !authorized || super::parent_of(store, sid)?.as_deref() != Some(admission.parent.as_str()) {
        return Err("child_council_denied: only admitted leaf sessions may run.".into());
    }
    let child = super::load(store, &admission.parent)?
        .ok_or("child_council_stale: child team is missing.")?;
    if child.generation != admission.generation || child.status != "running"
        || !child.workers.iter().any(|leaf| leaf.session_id == sid
            && leaf.status == "running" && leaf.read_only && leaf.council.is_some())
    {
        return Err("child_council_revoked: native leaf run is no longer scheduled.".into());
    }
    Ok(())
}

/// Dedicated read-only leaf scheduler: no child lead turn, dispatcher, tools,
/// extra reviewer source or recursive Council is ever launched.
pub(super) fn next_leaf_launch(
    store: &Store, agents: &std::collections::HashMap<String, crate::RunningAgent>,
    root: &TeamSnapshot,
) -> Result<Option<Launch>> {
    let Some(config) = root.config.council.as_ref() else { return Ok(None) };
    if config.nested_bot_reviewers.is_empty() { return Ok(None) }
    let mut global_running = root.workers.iter().filter(|w|
        super::active(&w.status) || agents.contains_key(&w.session_id)).count();
    let mut children = Vec::new();
    for choice in &config.nested_bot_reviewers {
        let Some(record) = read(store, root, choice)? else {
            return Err("child_council_stale: child reservation is missing.".into());
        };
        let child = super::load(store, &record.child_session_id)?
            .ok_or("child_council_stale: child review is missing.")?;
        global_running += child.workers.iter().filter(|w|
            super::active(&w.status) || agents.contains_key(&w.session_id)).count();
        children.push(child);
    }
    if global_running >= 8 { return Ok(None); }
    for mut child in children {
        if child.status != "running" { continue; }
        let local_running = child.workers.iter().filter(|w|
            super::active(&w.status) || agents.contains_key(&w.session_id)).count();
        if local_running >= child.config.max_parallel { continue; }
        let Some(leaf) = child.workers.iter_mut().find(|w|
            w.status == "queued" && !w.abandoned && !agents.contains_key(&w.session_id)) else {
            continue;
        };
        leaf.status = "running".into();
        leaf.updated_at = storage::now();
        if let Some(seat) = leaf.council.as_mut() {
            if seat.started_at == 0 { seat.started_at = leaf.updated_at; }
        }
        let launch = Launch {
            sid: leaf.session_id.clone(),
            prompt: leaf.resume_prompt.clone().unwrap_or_else(|| leaf.assignment.clone()),
            images: None,
            admission: Admission { parent: child.session_id.clone(), generation: child.generation },
        };
        super::save(store, &mut child)?;
        authorize_leaf(store, &launch.sid, &launch.admission)?;
        return Ok(Some(launch));
    }
    Ok(None)
}

/// Called in the root cycle's native transaction. Failure rolls back every
/// newly created child session and ownership record together with root start.
pub(super) fn reserve(store: &Store, root: &TeamSnapshot) -> Result<()> {
    let Some(council) = root.config.council.as_ref() else { return Ok(()) };
    if council.nested_bot_reviewers.is_empty() { return Ok(()) }
    council.validate()?;
    // Membership and the exact message-level policy are authoritative throughout
    // a Shared Channel delivery. A nested coordinator is a reviewer identity too.
    crate::bots::shared_channels::validate_delivery_council(
        store, &root.session_id, council,
    )?;
    if root.council_since <= 0 || root.objective.trim().is_empty() {
        return Err("child_council_denied: child reservation requires a started root objective.".into());
    }
    let parent = store.session(&root.session_id)?;
    let executor: Option<String> = store.db.query_row(
        "SELECT bot_id FROM bot_threads WHERE session_id=?1 UNION ALL SELECT bot_id FROM bot_tasks WHERE session_id=?1 LIMIT 1",
        [&root.session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    for child in &council.nested_bot_reviewers {
        if executor.as_deref() == Some(child.bot_id.as_str()) {
            return Err("reviewer_unavailable: the task executor cannot coordinate its own child Council.".into());
        }
        if read(store, root, child)?.is_some() { continue; }
        let bot = bots::get(store, &child.bot_id)
            .map_err(|_| "child_council_unavailable: selected bot no longer exists.")?;
        if bot.project_id != parent.project_id {
            return Err("child_council_denied: selected nested bot belongs to another project.".into());
        }
        if bot.paused || bot.provider == "zcode" || bot.model.is_empty() {
            return Err("child_council_unavailable: selected bot cannot coordinate this child.".into());
        }
        let config = child_config(council, child)?;
        // Always use a new task session; never reuse a live bot thread, team,
        // permission mode, credential or skill materialization.
        let session = store.create_session(&parent.project_id, &bot.provider, &bot.model, "plan")?;
        store.db.execute(
            "UPDATE sessions SET title=?2,task_description=?3,worktree_path=?4,reasoning_effort=?5 WHERE id=?1",
            params![session.id, format!("Council child: {}", bot.name),
                RESERVATION_DESCRIPTION,
                parent.worktree_path, bot.reasoning_effort],
        ).map_err(|e| e.to_string())?;
        store.db.execute(
            "INSERT INTO architect_workers(parent_session_id,worker_id,session_id) VALUES(?1,?2,?3)",
            params![root.session_id, storage::id(), session.id],
        ).map_err(|e| e.to_string())?;
        let mut team = super::new_team(session.id.clone(), config.clone());
        team.status = "blocked".into();
        super::save(store, &mut team)?;
        let record = frozen(store, root, child, &session.id, &config)?;
        super::record_request(store, &root.session_id, &key(root, &child.bot_id),
            "council_child", &serde_json::to_string(&record).map_err(|e| e.to_string())?, &session.id)?;
    }
    Ok(())
}

/// Scoped to the root's existing authenticated read-only native Council
/// inspector. A record is never evidence of a completed or passing review.
pub(super) fn inspect(store: &Store, root: &TeamSnapshot) -> Result<Vec<ChildReservationView>> {
    let Some(council) = root.config.council.as_ref() else { return Ok(Vec::new()) };
    if council.nested_bot_reviewers.is_empty() || root.objective.is_empty() {
        return Ok(Vec::new());
    }
    crate::bots::shared_channels::validate_delivery_council(
        store, &root.session_id, council,
    )?;
    // Read-only inspection must not mistake independently valid but jointly
    // conflicting child records for a trustworthy root roster.
    let _ = verified_parent_seats(store, root)?;
    council.nested_bot_reviewers.iter().map(|child| {
        let record = read(store, root, child)?
            .ok_or("child_council_stale: expected native child reservation is missing.")?;
        let team = super::load(store, &record.child_session_id)?
            .ok_or("child_council_stale: child team is missing.")?;
        let verified_child_receipt = read_verified_receipt(store, root, &record, &team)?;
        let authenticated_parent_seat = parent_seat(store, root, &record, &team, false)?;
        let active = team.status == "running";
        Ok(ChildReservationView {
            bot_id: child.bot_id.clone(),
            child_session_id: record.child_session_id,
            root_cycle_id: record.root_cycle_id,
            required_leaf_reviewers: child.child_reviewers,
            observed_leaf_usage: (!team.workers.is_empty())
                .then(|| super::council::inspection(&team, false).map(|view| view.usage))
                .flatten(),
            status: if authenticated_parent_seat.is_some() {
                "parent_composite_ready_root_pending"
            } else if verified_child_receipt.is_some() {
                "child_evidence_ready_root_pending"
            } else if active { "reviewing" } else { "blocked_until_native_child_review" },
            verified_child_receipt,
            authenticated_parent_seat,
        })
    }).collect()
}
