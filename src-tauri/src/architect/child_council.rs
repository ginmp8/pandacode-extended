//! Task-isolated reservations for hierarchical Council child Architects.
//! A reservation is NOT a child verdict, a runnable bot conversation or a
//! reviewer authority grant. The root gate remains blocked until a verified
//! native leaf lifecycle and composite receipt are available.
use super::{council::{CouncilConfig, CouncilNestedReviewer}, TeamConfig, TeamSnapshot};
use crate::{bots, storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    Ok(FrozenChildReservation {
        schema_version: 3,
        root_session_id: root.session_id.clone(),
        root_cycle_id: format!("{}:{}", root.session_id, root.council_since),
        bot_id: child.bot_id.clone(),
        bot_created_at: bot.created_at,
        child_session_id: session_id.to_owned(),
        objective_sha256: digest(root.objective.as_bytes()),
        policy_sha256: digest(&serde_json::to_vec(council).map_err(|e| e.to_string())?),
        child_config_sha256: digest(&serde_json::to_vec(child_config).map_err(|e| e.to_string())?),
        session_scope_sha256: digest(&scope),
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
    if stored.config != config || stored.status != "blocked" || !stored.workers.is_empty()
        || !stored.objective.is_empty() || stored.council_since != 0
        || stored.generation != 0 || stored.pending_turn.is_some()
        || stored.council_accepted.is_some()
        || store.session(&child_session_id)?.mode != "plan"
    {
        return Err("child_council_stale: child state or permissions have changed.".into());
    }
    Ok(Some(record))
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
                "Task-owned read-only Council reservation; reviewer dispatch is not yet enabled.",
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
    council.nested_bot_reviewers.iter().map(|child| {
        let record = read(store, root, child)?
            .ok_or("child_council_stale: expected native child reservation is missing.")?;
        Ok(ChildReservationView {
            bot_id: child.bot_id.clone(),
            child_session_id: record.child_session_id,
            root_cycle_id: record.root_cycle_id,
            required_leaf_reviewers: child.child_reviewers,
            status: "blocked_until_native_child_review",
        })
    }).collect()
}
