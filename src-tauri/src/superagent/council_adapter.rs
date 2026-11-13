//! Read-only bridge from an original SuperAgent worker session to the
//! existing Architect/Council #126 state. The native Council owns reviewer
//! eligibility, snapshots, verdicts and tool restrictions; never reimplement
//! scores or treat its team result as authority for an unbound ticket artifact.
use super::Ticket;
use crate::{architect, storage::{self, Store}, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
fn ticket_policy_revision(store: &Store, ticket_id: &str) -> Result<i64> {
    // Preserve the Ticket's immutable admission revision; a mutable Team or
    // another Ticket's mandate cannot substitute its Council authority.
    store.db.query_row(
        "SELECT policy_revision FROM sa_tickets WHERE id=?1",
        [ticket_id], |row| row.get(0),
    ).map_err(|error| error.to_string())
}

pub struct CouncilObservation {
    /// The verified native worker session has an Architect team with Council
    /// enabled; this is **advisory**, not task approval.
    pub native_status: String,
    pub native_round: usize,
    pub native_stale: bool,
    pub native_pass_verified: bool,
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone)]
struct RequiredRequest {
    id: String,
    ticket_id: String,
    assignment_id: String,
    worker_session_id: String,
    admitted_ticket_revision: i64,
    ticket_revision: i64,
    policy_revision: i64,
    cycle_id: String,
    cycle_started_at: i64,
    generation: i64,
    reviewer_policy_json: String,
    reviewer_policy_sha256: String,
    council_config_json: String,
    council_config_sha256: String,
    artifact_sha256: Option<String>,
    bound_at: Option<i64>,
    native_session_id: Option<String>,
    state: String,
}

fn required_request_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequiredRequest> {
    Ok(RequiredRequest {
        id: row.get(0)?, ticket_id: row.get(1)?, assignment_id: row.get(2)?,
        worker_session_id: row.get(3)?, admitted_ticket_revision: row.get(4)?,
        ticket_revision: row.get(5)?, policy_revision: row.get(6)?, cycle_id: row.get(7)?,
        cycle_started_at: row.get(8)?, generation: row.get(9)?,
        reviewer_policy_json: row.get(10)?, reviewer_policy_sha256: row.get(11)?,
        council_config_json: row.get(12)?, council_config_sha256: row.get(13)?,
        artifact_sha256: row.get(14)?, bound_at: row.get(15)?, native_session_id: row.get(16)?,
        state: row.get(17)?,
    })
}

const REQUEST_COLUMNS: &str = "id,ticket_id,assignment_id,worker_session_id,
 admitted_ticket_revision,ticket_revision,policy_revision,cycle_id,cycle_started_at,
 generation,reviewer_policy_json,reviewer_policy_sha256,council_config_json,
 council_config_sha256,artifact_sha256,bound_at,native_session_id,state";

fn council_config(snapshot: &super::TeamSnapshot) -> Result<architect::CouncilConfig> {
    if !snapshot.policy.council_required
        || !(2..=super::MAX_REVIEWERS_PER_TEAM).contains(&snapshot.reviewer_bot_ids.len())
    {
        return Err("Ticket is not pinned to a valid required native Council roster.".into());
    }
    let mut config = architect::CouncilConfig::default();
    config.reviewer_bot_ids = snapshot.reviewer_bot_ids.clone();
    Ok(config)
}

fn frozen_reviewer_policy(snapshot: &super::TeamSnapshot) -> Result<(String, String)> {
    let reviewers = snapshot.roles.iter().filter(|role| role.role == "reviewer").collect::<Vec<_>>();
    if reviewers.len() != snapshot.reviewer_bot_ids.len()
        || reviewers.iter().enumerate().any(|(ordinal, role)| {
            role.ordinal != ordinal || role.bot_id != snapshot.reviewer_bot_ids[ordinal]
                || role.effective_config.is_none() || role.effective_config_sha256.is_none()
        })
    {
        return Err("Council reviewer profiles are not fully frozen in the Ticket Team snapshot.".into());
    }
    let policy = serde_json::json!({
        "policyRevision": snapshot.policy.policy_revision,
        "reviewerBotIds": snapshot.reviewer_bot_ids,
        "reviewers": reviewers,
        "required": true,
    });
    let encoded = serde_json::to_vec(&policy).map_err(|error| error.to_string())?;
    let digest = sha256(&encoded);
    let json = String::from_utf8(encoded).map_err(|error| error.to_string())?;
    Ok((json, digest))
}

/// Freeze reviewer identities, Council policy and cycle at worker assignment
/// admission. Selected bots are preflighted by the same native #126 adapter
/// that will create their real seats after a successful result.
pub(super) fn admit_required(
    store: &Store,
    db: &Connection,
    ticket_id: &str,
    assignment_id: &str,
    worker_session_id: &str,
    admitted_ticket_revision: i64,
    snapshot: &super::TeamSnapshot,
) -> Result<()> {
    if !snapshot.policy.council_required { return Ok(()); }
    let config = council_config(snapshot)?;
    architect::validate_superagent_council(store, worker_session_id, &config)?;
    for (ordinal, reviewer_id) in snapshot.reviewer_bot_ids.iter().enumerate() {
        if !super::snapshot_role_profile_matches(&store.db, snapshot, "reviewer", ordinal, reviewer_id)? {
            return Err(format!("Council reviewer {reviewer_id} differs from the admitted Team profile."));
        }
    }
    let (reviewer_policy_json, reviewer_policy_sha256) = frozen_reviewer_policy(snapshot)?;
    let config_bytes = serde_json::to_vec(&config).map_err(|error| error.to_string())?;
    let council_config_json = String::from_utf8(config_bytes.clone()).map_err(|error| error.to_string())?;
    let council_config_sha256 = sha256(&config_bytes);
    let id = storage::id();
    let cycle_id = storage::id();
    let cycle_started_at = storage::now();
    db.execute(
        "INSERT INTO sa_council_requests(
           id,ticket_id,assignment_id,worker_session_id,admitted_ticket_revision,
           ticket_revision,policy_revision,cycle_id,cycle_started_at,generation,
           reviewer_policy_json,reviewer_policy_sha256,council_config_json,
           council_config_sha256,state,created_at,updated_at
         ) VALUES(?1,?2,?3,?4,?5,?5,?6,?7,?8,1,?9,?10,?11,?12,'admitted',?8,?8)",
        params![id,ticket_id,assignment_id,worker_session_id,admitted_ticket_revision,
            snapshot.policy.policy_revision,cycle_id,cycle_started_at,reviewer_policy_json,
            reviewer_policy_sha256,council_config_json,council_config_sha256],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

fn request_for_assignment(store: &Store, ticket_id: &str, assignment_id: &str) -> Result<Option<RequiredRequest>> {
    let sql = format!("SELECT {REQUEST_COLUMNS} FROM sa_council_requests WHERE ticket_id=?1 AND assignment_id=?2");
    store.db.query_row(&sql,params![ticket_id,assignment_id],required_request_from_row)
        .optional().map_err(|error| error.to_string())
}

fn request_for_worker(store: &Store, worker_session_id: &str) -> Result<Option<RequiredRequest>> {
    let sql = format!("SELECT {REQUEST_COLUMNS} FROM sa_council_requests WHERE worker_session_id=?1 ORDER BY created_at DESC,id DESC LIMIT 1");
    store.db.query_row(&sql,[worker_session_id],required_request_from_row)
        .optional().map_err(|error| error.to_string())
}

fn request_for_native(store: &Store, native_session_id: &str) -> Result<Option<RequiredRequest>> {
    let sql = format!("SELECT {REQUEST_COLUMNS} FROM sa_council_requests WHERE native_session_id=?1");
    store.db.query_row(&sql,[native_session_id],required_request_from_row)
        .optional().map_err(|error| error.to_string())
}

fn request_for_ticket(store: &Store, ticket_id: &str) -> Result<Option<RequiredRequest>> {
    let sql = format!("SELECT {REQUEST_COLUMNS} FROM sa_council_requests
        WHERE ticket_id=?1 ORDER BY
          (SELECT attempt FROM sa_assignments WHERE id=sa_council_requests.assignment_id) DESC,
          created_at DESC,id DESC LIMIT 1");
    store.db.query_row(&sql,[ticket_id],required_request_from_row)
        .optional().map_err(|error| error.to_string())
}

/// Pin only the result just committed by the current worker callback. This is
/// called inside that same ticket transaction before any reviewer may start.
pub(super) fn bind_required_finished(
    store: &Store, ticket_id: &str, assignment_id: &str, worker_session_id: &str,
) -> Result<()> {
    let ticket = super::get_ticket(store,ticket_id)?;
    let request = request_for_assignment(store,ticket_id,assignment_id)?
        .ok_or("Council-required worker assignment has no frozen review request.")?;
    if !matches!(request.state.as_str(),"admitted")
        || request.worker_session_id != worker_session_id
        || request.policy_revision != ticket_policy_revision(&store, &ticket.id)?
        || ticket.state != "waiting_review"
        || ticket.revision != request.admitted_ticket_revision.saturating_add(1)
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
    {
        return Err("Required Council request no longer matches the successful Ticket result.".into());
    }
    let snapshot = super::ticket_team_snapshot(store,ticket_id)?;
    if !snapshot.policy.council_required || snapshot.policy.policy_revision != request.policy_revision {
        return Err("Ticket Council policy changed after worker admission.".into());
    }
    let grant = super::mandate(store,ticket_id)?;
    if grant.revoked || grant.policy_revision != request.policy_revision {
        return Err("Ticket mandate no longer permits required Council review.".into());
    }
    let matched: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE a.id=?1 AND a.ticket_id=?2 AND a.session_id=?3
           AND a.bot_id=t.worker_bot_id AND a.state='finished'
           AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?2))",
        params![assignment_id,ticket_id,worker_session_id],|row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !matched || !super::assignment_thread_current(store,worker_session_id,ticket_id)? {
        return Err("Finished worker assignment or native thread is stale.".into());
    }
    let artifact = sha256(ticket.evidence.as_deref().unwrap_or_default().as_bytes());
    let changed = store.db.execute(
        "UPDATE sa_council_requests SET ticket_revision=?2,artifact_sha256=?3,
           bound_at=?4,state='bound',updated_at=?4
         WHERE id=?1 AND state='admitted' AND assignment_id=?5
           AND worker_session_id=?6 AND admitted_ticket_revision=?7",
        params![request.id,ticket.revision,artifact,storage::now(),assignment_id,
            worker_session_id,request.admitted_ticket_revision],
    ).map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Required Council artifact binding was replayed or superseded.".into());
    }
    Ok(())
}

pub(crate) fn required_worker_session(store: &Store, worker_session_id: &str) -> Result<bool> {
    Ok(request_for_worker(store,worker_session_id)?.is_some_and(|request| {
        request.state == "admitted"
    }))
}

fn block_request(store: &Store, request_id: &str, detail: &str) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let now = storage::now();
    let request: Option<(String,String,i64)> = tx.query_row(
        "SELECT ticket_id,state,ticket_revision FROM sa_council_requests WHERE id=?1",
        [request_id],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional().map_err(|error| error.to_string())?;
    let Some((ticket_id,state,ticket_revision)) = request else { return Ok(()) };
    if matches!(state.as_str(),"passed" | "human_accepted" | "blocked") { return Ok(()) }
    tx.execute("UPDATE sa_council_requests SET state='blocked',updated_at=?2 WHERE id=?1",params![request_id,now])
        .map_err(|error| error.to_string())?;
    let moved = tx.execute(
        "UPDATE sa_tickets SET state='blocked',active_assignment_id=NULL,
           revision=revision+1,updated_at=?3
         WHERE id=?1 AND state='waiting_review' AND revision=?2",
        params![ticket_id,ticket_revision,now],
    ).map_err(|error| error.to_string())?;
    if moved == 1 {
        tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[&ticket_id])
            .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'council_blocked',?2,?3)",
            params![ticket_id,detail.chars().take(500).collect::<String>(),now],
        ).map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())
}

/// Launch only once, only after the successful worker callback atomically bound
/// this request to the current artifact and Ticket revision.
pub(crate) fn start_after_worker_completion(app: &tauri::AppHandle, worker_session_id: &str) -> Result<bool> {
    let state = app.state::<crate::AppState>();
    // Serialize reviewer launch with Ticket Stop/restart recovery so a stop
    // cannot win the Ticket CAS while a native review team is being created.
    let database_path = state.store.lock().map_err(|error| error.to_string())?.path.clone();
    let _dispatch_fence = super::effects::lock_dispatch(&database_path)?;
    let launch = {
        let store = state.store.lock().map_err(|error| error.to_string())?;
        let Some(request) = request_for_worker(&store,worker_session_id)? else { return Ok(false) };
        if request.state != "bound" { return Ok(false) }
        let ticket = super::get_ticket(&store,&request.ticket_id)?;
        let evidence = ticket.evidence.clone().filter(|text| !text.trim().is_empty());
        let Some(evidence) = evidence else {
            block_request(&store,&request.id,"Required Council has no successful worker artifact.")?;
            return Ok(false);
        };
        let snapshot = super::ticket_team_snapshot(&store,&ticket.id)?;
        let mandate = super::mandate(&store,&ticket.id)?;
        let mut reviewer_profiles_current = true;
        for (ordinal,reviewer_id) in snapshot.reviewer_bot_ids.iter().enumerate() {
            reviewer_profiles_current &= super::snapshot_role_profile_matches(
                &store.db,&snapshot,"reviewer",ordinal,reviewer_id,
            )?;
        }
        let config_bytes = request.council_config_json.as_bytes();
        let config: architect::CouncilConfig = serde_json::from_str(&request.council_config_json)
            .map_err(|_| "Frozen Council configuration is malformed.")?;
        let policy_ok = request.reviewer_policy_sha256 == sha256(request.reviewer_policy_json.as_bytes())
            && request.council_config_sha256 == sha256(config_bytes)
            && request.policy_revision == ticket_policy_revision(&store, &ticket.id)?
            && snapshot.policy.council_required
            && snapshot.policy.policy_revision == request.policy_revision
            && snapshot.reviewer_bot_ids == config.reviewer_bot_ids
            && reviewer_profiles_current
            && frozen_reviewer_policy(&snapshot).is_ok_and(|(json,hash)| {
                json == request.reviewer_policy_json && hash == request.reviewer_policy_sha256
            });
        let artifact = sha256(evidence.as_bytes());
        let assignment_current: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_assignments a WHERE a.id=?1 AND a.ticket_id=?2
               AND a.session_id=?3 AND a.state='finished' AND a.result=?4
               AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?2))",
            params![request.assignment_id,ticket.id,request.worker_session_id,evidence],|row| row.get(0),
        ).map_err(|error| error.to_string())?;
        let valid = request.state == "bound"
            && ticket.state == "waiting_review"
            && ticket.revision == request.ticket_revision
            && request.worker_session_id == worker_session_id
            && request.artifact_sha256.as_deref() == Some(artifact.as_str())
            && request.bound_at.is_some()
            && mandate.policy_revision == request.policy_revision && !mandate.revoked
            && mandate.expires_at > storage::now()
            && assignment_current
            && super::assignment_thread_current(&store,worker_session_id,&ticket.id)?
            && policy_ok;
        if !valid {
            block_request(&store,&request.id,"Required Council binding became stale or mismatched before dispatch.")?;
            return Ok(false);
        }
        let config_sha = sha256(serde_json::to_vec(&config).map_err(|error| error.to_string())?.as_slice());
        let worker = store.session(worker_session_id)?;
        let binding = architect::SuperAgentCouncilBinding {
            ticket_id: ticket.id.clone(),
            assignment_id: request.assignment_id.clone(),
            worker_session_id: request.worker_session_id.clone(),
            ticket_revision: request.ticket_revision,
            policy_revision: request.policy_revision,
            cycle_id: request.cycle_id.clone(),
            cycle_started_at: request.cycle_started_at,
            generation: u64::try_from(request.generation).map_err(|_| "Invalid Council generation.")?,
            artifact_sha256: artifact,
            reviewer_policy_sha256: request.reviewer_policy_sha256.clone(),
            council_config_sha256: config_sha,
        };
        let changed = store.db.execute(
            "UPDATE sa_council_requests SET state='starting',updated_at=?2
             WHERE id=?1 AND state='bound' AND ticket_revision=?3 AND artifact_sha256=?4",
            params![request.id,storage::now(),request.ticket_revision,binding.artifact_sha256],
        ).map_err(|error| error.to_string())?;
        if changed != 1 { return Ok(false) }
        architect::SuperAgentCouncilLaunch {
            binding, config, project_id: worker.project_id, provider: worker.provider,
            model: worker.model, mode: worker.mode, title: format!("SuperAgent Council · {}",ticket.id),
            objective: ticket.objective, evidence,
        }
    };
    let binding = launch.binding.clone();
    let request_id = {
        let store = state.store.lock().map_err(|error| error.to_string())?;
        store.db.query_row(
            "SELECT id FROM sa_council_requests WHERE ticket_id=?1 AND assignment_id=?2 AND state='starting'",
            params![binding.ticket_id,binding.assignment_id],|row| row.get::<_,String>(0),
        ).optional().map_err(|error| error.to_string())?
    };
    let Some(request_id) = request_id else { return Ok(false) };
    let native_session_id = match architect::start_superagent_review(app,launch) {
        Ok(session) => session,
        Err(error) => {
            if let Ok(store) = state.store.lock() { let _ = block_request(&store,&request_id,"Native reviewer dispatch failed; explicit recovery is required."); }
            return Err(error);
        }
    };
    let store = state.store.lock().map_err(|error| error.to_string())?;
    let changed = store.db.execute(
        "UPDATE sa_council_requests SET native_session_id=?2,state='dispatched',updated_at=?3
         WHERE id=?1 AND state='starting' AND ticket_revision=?4
           AND artifact_sha256=?5 AND cycle_id=?6",
        params![request_id,native_session_id,storage::now(),binding.ticket_revision,
            binding.artifact_sha256,binding.cycle_id],
    ).map_err(|error| error.to_string())?;
    if changed != 1 {
        drop(store);
        if let Ok(store) = state.store.lock() { let _ = block_request(&store,&request_id,"Council cycle changed during native reviewer dispatch."); }
        return Ok(false);
    }
    drop(store);
    drop(_dispatch_fence);
    let store = state.store.lock().map_err(|error| error.to_string())?;
    if let Some(team) = architect::load(&store,&native_session_id)? {
        let settled_sessions = team.workers.iter()
            .filter(|worker| matches!(worker.status.as_str(),"completed" | "failed" | "stopped"))
            .map(|worker| worker.session_id.clone()).collect::<Vec<_>>();
        for reviewer_session in settled_sessions {
            if let Err(error) = finalize_after_native_run(&store,&reviewer_session) {
                eprintln!("SuperAgent Council finalization callback failed: {error}");
            }
        }
    }
    Ok(true)
}

fn native_binding_matches(request: &RequiredRequest, team: &architect::TeamSnapshot) -> bool {
    let Some(binding) = team.superagent_review.as_ref() else { return false };
    let Some(artifact) = request.artifact_sha256.as_deref() else { return false };
    binding.ticket_id == request.ticket_id
        && binding.assignment_id == request.assignment_id
        && binding.worker_session_id == request.worker_session_id
        && binding.ticket_revision == request.ticket_revision
        && binding.policy_revision == request.policy_revision
        && binding.cycle_id == request.cycle_id
        && binding.cycle_started_at == request.cycle_started_at
        && i64::try_from(binding.generation).ok() == Some(request.generation)
        && binding.artifact_sha256 == artifact
        && binding.reviewer_policy_sha256 == request.reviewer_policy_sha256
        && binding.council_config_sha256 == request.council_config_sha256
        && team.session_id == request.native_session_id.as_deref().unwrap_or_default()
        && team.council_since == request.cycle_started_at
        && i64::try_from(team.generation).ok() == Some(request.generation)
}

fn native_roster_matches(
    request: &RequiredRequest,
    team: &architect::TeamSnapshot,
    ticket_snapshot: &super::TeamSnapshot,
) -> bool {
    let Ok(config) = serde_json::from_str::<architect::CouncilConfig>(&request.council_config_json) else { return false };
    let round = team.council_status.as_ref().map_or(0,|status| status.round);
    if round == 0 { return false; }
    let mut seen = std::collections::HashSet::new();
    let seats = team.workers.iter()
        .filter(|worker| worker.created_at >= request.cycle_started_at
            && worker.council.as_ref().is_some_and(|seat| seat.round == round))
        .collect::<Vec<_>>();
    if seats.len() != config.reviewer_bot_ids.len() { return false; }
    for worker in seats {
        let Some(binding) = worker.reviewer_binding.as_ref() else { return false };
        let Some(reviewer_id) = binding.reviewer_bot_id.as_deref() else { return false };
        let Some(ordinal) = config.reviewer_bot_ids.iter().position(|id| id == reviewer_id) else { return false };
        if !seen.insert(reviewer_id.to_owned()) { return false; }
        let Some(profile) = ticket_snapshot.roles.iter().find(|role| {
            role.role == "reviewer" && role.ordinal == ordinal && role.bot_id == reviewer_id
        }).and_then(|role| role.effective_config.as_ref()) else { return false };
        let Some(context) = binding.snapshot.bot_context.as_ref() else { return false };
        let skills = profile["skills"].as_array().map(|values| values.iter()
            .filter_map(serde_json::Value::as_str).map(str::to_owned).collect::<Vec<_>>());
        if profile["provider"].as_str() != Some(binding.snapshot.provider.as_str())
            || profile["model"].as_str() != Some(binding.snapshot.model.as_str())
            || profile["name"].as_str() != Some(context.name.as_str())
            || profile["role"].as_str() != Some(context.role.as_str())
            || profile["instructions"].as_str() != Some(context.instructions.as_str())
            || profile["goals"].as_str() != Some(context.goals.as_str())
            || profile["soul"].as_str() != Some(context.soul.as_str())
            || profile["reasoningEffort"].as_str().unwrap_or_default() != context.reasoning_effort.as_str()
            || skills.as_deref() != Some(context.skill_ids.as_slice())
        {
            return false;
        }
    }
    config.reviewer_bot_ids.iter().all(|id| seen.contains(id))
}

fn current_binding_valid(
    store: &Store, request: &RequiredRequest, ticket: &super::Ticket,
    team: &architect::TeamSnapshot,
) -> Result<bool> {
    let Some(evidence) = ticket.evidence.as_deref() else { return Ok(false) };
    let Some(artifact) = request.artifact_sha256.as_deref() else { return Ok(false) };
    let snapshot = super::ticket_team_snapshot(store,&ticket.id)?;
    let grant = super::mandate(store,&ticket.id)?;
    let config: architect::CouncilConfig = match serde_json::from_str(&request.council_config_json) {
        Ok(config) => config,
        Err(_) => return Ok(false),
    };
    let config_bytes = serde_json::to_vec(&config).map_err(|error| error.to_string())?;
    let (policy_json,policy_hash) = match frozen_reviewer_policy(&snapshot) {
        Ok(frozen) => frozen,
        Err(_) => return Ok(false),
    };
    let matched_assignment: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments a WHERE a.id=?1 AND a.ticket_id=?2
          AND a.session_id=?3 AND a.state='finished' AND a.result=?4
          AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?2))",
        params![request.assignment_id,request.ticket_id,request.worker_session_id,evidence],|row| row.get(0),
    ).map_err(|error| error.to_string())?;
    let worker = crate::bots::get(store,&super::ticket_worker(store,&ticket.id)?)?;
    let coordinator = crate::bots::get(store,&snapshot.coordinator_bot_id)?;
    Ok(request.state == "dispatched"
        && ticket.id == request.ticket_id && ticket.state == "waiting_review"
        && ticket.revision == request.ticket_revision
        && ticket_policy_revision(&store, &ticket.id)? == request.policy_revision
        && request.cycle_started_at > 0 && request.generation > 0
        && sha256(evidence.as_bytes()) == artifact
        && sha256(request.reviewer_policy_json.as_bytes()) == request.reviewer_policy_sha256
        && policy_json == request.reviewer_policy_json && policy_hash == request.reviewer_policy_sha256
        && sha256(&config_bytes) == request.council_config_sha256
        && config.reviewer_bot_ids == snapshot.reviewer_bot_ids
        && team.config.council.as_ref() == Some(&config)
        && grant.policy_revision == request.policy_revision && !grant.revoked
        && grant.expires_at > storage::now()
        && matched_assignment
        && super::assignment_thread_current(store,&request.worker_session_id,&request.ticket_id)?
        && super::coordinator_plan_current(store,&ticket.id,&coordinator.id,&coordinator.project_id)?
        && worker.project_id == snapshot.project_id
        && native_binding_matches(request,team)
        && request.bound_at.is_some()
        && native_roster_matches(request,team,&snapshot))
}

fn finalization_exists(store: &Store, request: &RequiredRequest, verdict: &str) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_council_finalizations WHERE request_id=?1
          AND ticket_id=?2 AND assignment_id=?3 AND worker_session_id=?4
          AND ticket_revision=?5 AND policy_revision=?6 AND cycle_id=?7
          AND cycle_started_at=?8 AND generation=?9 AND native_session_id=?10
          AND artifact_sha256=?11 AND verdict=?12)",
        params![request.id,request.ticket_id,request.assignment_id,request.worker_session_id,
            request.ticket_revision,request.policy_revision,request.cycle_id,request.cycle_started_at,
            request.generation,request.native_session_id,request.artifact_sha256,verdict],|row| row.get(0),
    ).map_err(|error| error.to_string())
}

fn persist_finalization(
    store: &Store, request: &RequiredRequest, team: &architect::TeamSnapshot,
    verdict: &str, reason: Option<&str>,
) -> Result<bool> {
    let _dispatch_fence = super::effects::lock_dispatch(&store.path)?;
    // Recheck after acquiring the cross-process fence. Concurrent duplicate
    // native callbacks then observe the committed receipt and become no-ops.
    if finalization_exists(store,request,verdict)? { return Ok(true); }
    let ticket = super::get_ticket(store,&request.ticket_id)?;
    if !current_binding_valid(store,request,&ticket,team)? { return Ok(false); }
    match verdict {
        "passed" if architect::verified_council_pass(store,team) => {},
        "human_accepted" if reason.is_some_and(|text| !text.trim().is_empty())
            && architect::verified_council_accept(store,team) => {},
        _ => return Ok(false),
    }
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let now = storage::now();
    let finalization_id = storage::id();
    tx.execute(
        "INSERT INTO sa_council_finalizations(
           id,request_id,ticket_id,assignment_id,worker_session_id,ticket_revision,
           policy_revision,cycle_id,cycle_started_at,generation,native_session_id,
           artifact_sha256,verdict,actor,reason,created_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![finalization_id,request.id,request.ticket_id,request.assignment_id,
            request.worker_session_id,request.ticket_revision,request.policy_revision,
            request.cycle_id,request.cycle_started_at,request.generation,
            request.native_session_id,request.artifact_sha256,verdict,
            if verdict == "passed" { "native-council" } else { "local-user" },
            reason.map(|text| text.trim().chars().take(500).collect::<String>()),now],
    ).map_err(|error| error.to_string())?;
    let changed = tx.execute(
        "UPDATE sa_tickets SET state='completed',active_assignment_id=NULL,
           revision=revision+1,updated_at=?3
         WHERE id=?1 AND state='waiting_review' AND revision=?2 AND evidence IS NOT NULL",
        params![request.ticket_id,request.ticket_revision,now],
    ).map_err(|error| error.to_string())?;
    if changed != 1 { return Ok(false); }
    let request_changed = tx.execute(
        "UPDATE sa_council_requests SET state=?2,updated_at=?3
         WHERE id=?1 AND state='dispatched' AND ticket_revision=?4
           AND assignment_id=?5 AND cycle_id=?6 AND artifact_sha256=?7",
        params![request.id,verdict,now,request.ticket_revision,request.assignment_id,
            request.cycle_id,request.artifact_sha256],
    ).map_err(|error| error.to_string())?;
    if request_changed != 1 { return Ok(false); }
    tx.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[&request.ticket_id])
        .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,?2,?3,?4)",
        params![request.ticket_id,if verdict == "passed" { "council_passed" } else { "council_human_accepted" },
            reason.map_or_else(|| "Verified native Council pass".to_string(),|text| text.trim().chars().take(500).collect()),now],
    ).map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(true)
}

/// Persist a terminal pass after Architect has saved the exact reviewer seat.
/// Incomplete work is left waiting; failed, stale, limited or stopped rounds
/// revoke the Ticket and require an explicit retry.
pub(crate) fn finalize_after_native_run(store: &Store, reviewer_session_id: &str) -> Result<bool> {
    let Some(native_session_id) = architect::parent_of(store,reviewer_session_id)? else { return Ok(false) };
    let Some(team) = architect::load(store,&native_session_id)? else { return Ok(false) };
    let Some(binding) = team.superagent_review.as_ref() else { return Ok(false) };
    let Some(request) = request_for_native(store,&native_session_id)? else { return Ok(false) };
    if request.state == "starting" { return Ok(false); }
    if !native_binding_matches(&request,&team) {
        block_request(store,&request.id,"Native Council callback did not match the frozen Ticket cycle.")?;
        return Ok(false);
    }
    if !team.workers.iter().any(|worker| worker.session_id == reviewer_session_id && worker.council.is_some()) {
        return Ok(false);
    }
    if matches!(request.state.as_str(),"passed" | "human_accepted") {
        return finalization_exists(store,&request,&request.state);
    }
    if request.state != "dispatched" {
        block_request(store,&request.id,"Native Council callback did not match an active Ticket review.")?;
        return Ok(false);
    }
    let ticket = super::get_ticket(store,&binding.ticket_id)?;
    let Some(status) = team.council_status.as_ref() else {
        block_request(store,&request.id,"Native Council has no terminal status for this Ticket.")?;
        return Ok(false);
    };
    if status.stale {
        block_request(store,&request.id,"Native Council evidence became stale before finalization.")?;
        return Ok(false);
    }
    if status.state == "reviewing" { return Ok(false); }
    if status.state == "passed" {
        if persist_finalization(store,&request,&team,"passed",None)? { return Ok(true); }
        block_request(store,&request.id,"Native Council pass failed exact Ticket finalization checks.")?;
        return Ok(false);
    }
    if status.state == "fixing" && architect::superagent_council_human_acceptance_ready(store,&team) {
        return Ok(false);
    }
    // A raw Architect acceptance has no reason. Only the explicit SuperAgent
    // acceptance command below may persist the distinct human verdict.
    if status.state == "accepted" {
        if architect::verified_council_accept(store,&team) { return Ok(false); }
        block_request(store,&request.id,"Native Council acceptance followed an incomplete or invalid reviewer round.")?;
        return Ok(false);
    }
    if matches!(ticket.state.as_str(),"waiting_review") {
        block_request(store,&request.id,"Native Council failed, stopped, reached a limit, or did not produce a verified terminal verdict.")?;
    }
    Ok(false)
}

pub(super) fn prepare_human_accept(store: &Store, ticket_id: &str, expected_revision: i64) -> Result<String> {
    let ticket = super::get_ticket(store,ticket_id)?;
    if ticket.state != "waiting_review" || ticket.revision != expected_revision {
        return Err("Ticket is no longer waiting on this Council review.".into());
    }
    let request = request_for_ticket(store,ticket_id)?.ok_or("Required Council request is missing.")?;
    if request.state != "dispatched" {
        return Err("Native Council request is not active.".into());
    }
    let native_session_id = request.native_session_id.as_deref().ok_or("Native Council session is missing.")?;
    let team = architect::load(store,native_session_id)?.ok_or("Native Council team is missing.")?;
    if !current_binding_valid(store,&request,&ticket,&team)?
        || (!architect::superagent_council_human_acceptance_ready(store,&team)
            && !architect::verified_council_accept(store,&team))
    {
        return Err("Only a complete, current, below-target Council round can be accepted with a reason.".into());
    }
    Ok(native_session_id.to_owned())
}

pub(super) fn finalize_human_accept(
    store: &Store, ticket_id: &str, expected_revision: i64, reason: &str,
) -> Result<Ticket> {
    let reason = reason.trim();
    if reason.is_empty() || reason.len() > 500 || reason.chars().any(char::is_control) {
        return Err("Provide a plain-text human acceptance reason of 1 to 500 characters.".into());
    }
    let ticket = super::get_ticket(store,ticket_id)?;
    if ticket.state != "waiting_review" || ticket.revision != expected_revision {
        return Err("Ticket changed before Council acceptance.".into());
    }
    let request = request_for_ticket(store,ticket_id)?.ok_or("Required Council request is missing.")?;
    let native_session_id = request.native_session_id.as_deref().ok_or("Native Council session is missing.")?;
    let team = architect::load(store,native_session_id)?.ok_or("Native Council team is missing.")?;
    if !architect::verified_council_accept(store,&team)
        || !persist_finalization(store,&request,&team,"human_accepted",Some(reason))?
    {
        return Err("Attributed Council acceptance did not match the current native Ticket review.".into());
    }
    super::get_ticket(store,ticket_id)
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

/// Existing Council-enabled worker sessions are not eligible for legacy
/// Council-Off manual acceptance. The persisted review target remains binding
/// even if the underlying Architect team is later removed or its Council
/// toggle changed. This is a denial guard, not reviewer invocation, verification
/// or an exception-grant mechanism.
pub(super) fn requires_task_review(
    store: &Store, ticket_id: &str, worker_session_id: &str,
) -> Result<bool> {
    let frozen: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_council_targets WHERE ticket_id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if frozen { return Ok(true); }
    Ok(architect::load(store, worker_session_id)?
        .is_some_and(|team| team.config.council.is_some()))
}

/// Only the latest, finished, exactly correlated worker evidence is eligible
/// for a Council lookup. A different session, coordinator plan, stale attempt
/// or remapped bot thread cannot borrow Council status from another task.
pub(super) fn inspect(store: &Store, ticket: &Ticket) -> Result<Option<CouncilObservation>> {
    if !matches!(ticket.state.as_str(), "waiting_approval" | "waiting_review")
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
        || super::mandate(store, &ticket.id)?.revoked
    {
        return Ok(None);
    }
    let grant = super::mandate(store, &ticket.id)?;
    let assignment: Option<(String, String)> = store.db.query_row(
        "SELECT a.id,a.session_id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.id=?1 AND a.bot_id=t.worker_bot_id
           AND a.state='finished' AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments
                          WHERE ticket_id=?1)",
        [&ticket.id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
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
    let review_started_after_binding = bound_at.is_some_and(|frozen_at| {
        let Some(config) = team.config.council.as_ref() else { return false };
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
    Ok(Some(CouncilObservation {
        native_status,
        native_round: status.round,
        native_stale: status.stale,
        native_pass_verified: verified,
        ticket_artifact_bound: bound_at.is_some(),
        frozen_assignment_id: bound_at.map(|_| assignment_id),
        frozen_ticket_revision: bound_at.map(|_| ticket.revision),
        review_started_after_binding,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bots, storage::{self, Store}, superagent::{self, NewOrchestration}};
    use rusqlite::params;
    use serde_json::json;

    #[test]
    fn partial_duplicate_and_stale_rounds_cannot_claim_fresh_review() {
        assert!(!reviewers_started_after_binding(2, [("a",10)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("a",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",9),("b",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("b",11),("c",12)].into_iter(), 10));
        assert!(reviewers_started_after_binding(2, [("a",10),("b",11)].into_iter(), 10));
    }

    fn fixture() -> Store {
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
        store
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

    /// Build the native Ticket/request rows up to worker dispatch without
    /// requiring locally installed reviewer executables in a unit fixture.
    /// The production admission path runs validate_superagent_council before
    /// writing the same immutable request identity.
    fn required_worker(store: &mut Store, key: &str) -> (superagent::Ticket,String,String) {
        for id in ["reviewer-a", "reviewer-b"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config = superagent::create_configuration_roles_with_policy(
            store,
            NewOrchestration {
                name: format!("Required Council {key}"),
                coordinator_bot_id: "router".into(),
                worker_bot_id: "worker".into(),
            },
            vec!["worker".into()],
            vec!["reviewer-a".into(),"reviewer-b".into()],
            true,
        ).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();
        let config = superagent::configuration(store,&config.id).unwrap();
        let snapshot = superagent::make_team_snapshot(
            &store.db,&config.id,config.team_version,&config.name,&config.project_id,
            config.policy_revision,&config.coordinator_bot_id,&config.worker_bot_ids,
            &config.reviewer_bot_ids,config.council_required,
            superagent::TeamSnapshotProvenance::Native,
        ).unwrap();
        let ticket_id = storage::id();
        let assignment_id = storage::id();
        let worker = bots::get(store,"worker").unwrap();
        let session = bots::thread_session(
            store,&worker,superagent::CHANNEL,&ticket_id,"SuperAgent · Manual task",
        ).unwrap().0;
        let now = storage::now();
        let tx = store.db.unchecked_transaction().unwrap();
        tx.execute(
            "INSERT INTO sa_tickets(id,orchestration_id,request_key,objective,actor,source,
              project_id,worker_bot_id,policy_revision,state,revision,active_assignment_id,
              created_at,updated_at)
             VALUES(?1,?2,?3,'Inspect safely','local-user','manual',?4,'worker',?5,
              'running',0,?6,?7,?7)",
            params![ticket_id,config.id,key,config.project_id,config.policy_revision,assignment_id,now],
        ).unwrap();
        superagent::insert_ticket_team_snapshot(&tx,&ticket_id,&snapshot,"native").unwrap();
        tx.execute(
            "INSERT INTO sa_mandates(ticket_id,actor,authorization_id,policy_revision,
              project_id,expires_at,created_at)
             VALUES(?1,'local-user',?2,?3,?4,?5,?6)",
            params![ticket_id,storage::id(),config.policy_revision,config.project_id,
                now+4*60*60*1000,now],
        ).unwrap();
        tx.execute(
            "INSERT INTO sa_assignments(id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES(?1,?2,'worker',?3,'running',1,?4,?4)",
            params![assignment_id,ticket_id,session,now],
        ).unwrap();
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'admitted','manual readonly worker reserved',?2)",
            params![ticket_id,now],
        ).unwrap();
        let (reviewer_policy_json,reviewer_policy_sha256)=frozen_reviewer_policy(&snapshot).unwrap();
        let council = council_config(&snapshot).unwrap();
        let council_config_bytes = serde_json::to_vec(&council).unwrap();
        let council_config_json = String::from_utf8(council_config_bytes.clone()).unwrap();
        tx.execute(
            "INSERT INTO sa_council_requests(
              id,ticket_id,assignment_id,worker_session_id,admitted_ticket_revision,
              ticket_revision,policy_revision,cycle_id,cycle_started_at,generation,
              reviewer_policy_json,reviewer_policy_sha256,council_config_json,
              council_config_sha256,state,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,0,0,?5,?6,?7,1,?8,?9,?10,?11,'admitted',?7,?7)",
            params![storage::id(),ticket_id,assignment_id,session,config.policy_revision,
                storage::id(),now,reviewer_policy_json,reviewer_policy_sha256,
                council_config_json,sha256(&council_config_bytes)],
        ).unwrap();
        tx.commit().unwrap();
        (superagent::get_ticket(store,&ticket_id).unwrap(),session,assignment_id)
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
        let observed=inspect(&store,&saved).unwrap().unwrap();
        assert!(observed.ticket_artifact_bound);
        assert_eq!(observed.frozen_ticket_revision,Some(saved.revision));
        assert!(observed.frozen_assignment_id.is_some());
        assert!(!observed.native_pass_verified);
        assert!(!observed.review_started_after_binding);
        assert_eq!(saved.state,"waiting_approval");

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
    fn council_required_manual_accept_denies_unbound_and_forged_pass_without_changing_ticket() {
        let mut store = fixture();
        let (ticket, session) = settled(&mut store, "required-no-gate");
        attach_native_council(&store, &session);
        assert!(requires_task_review(&store, &ticket.id, &session).unwrap());
        let denied = superagent::accept(&store, &ticket.id, ticket.revision).unwrap_err();
        assert!(denied.contains("Native Council review is required"));
        assert_eq!(superagent::get_ticket(&store, &ticket.id).unwrap().state, "waiting_approval");

        // A status that merely claims pass must never authorize terminal
        // acceptance, even when there is no frozen Ticket artifact.
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilStatus.state','passed','$.councilStatus.round',1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(superagent::accept(&store, &ticket.id, ticket.revision).is_err());
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1
             AND kind='human_accepted'", [&ticket.id], |r| r.get::<_, i64>(0),
        ).unwrap(), 0);
    }

    #[test]
    fn frozen_council_target_never_becomes_council_off_when_team_disappears() {
        let mut store = fixture();
        let cfg = superagent::create_configuration(&store, NewOrchestration {
            name: "Frozen review must remain required".into(),
            coordinator_bot_id: "router".into(), worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1
             WHERE id=?1", [&cfg.id],
        ).unwrap();
        let (ticket, launched) = superagent::admit(
            &mut store, &cfg.id, "Review actual artifact", "frozen-review", false,
        ).unwrap();
        assert!(launched);
        let session = superagent::assignment_session(&store, &ticket.id).unwrap().unwrap();
        attach_native_council(&store, &session);
        assert!(superagent::on_run_finished(
            &store, &session, true, "Worker artifact",
        ).unwrap());
        let waiting = superagent::get_ticket(&store, &ticket.id).unwrap();
        assert!(requires_task_review(&store, &ticket.id, &session).unwrap());
        // Removing the native team must not downgrade the task to legacy
        // Council-Off approval while the original frozen target remains.
        store.db.execute(
            "DELETE FROM architect_teams WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(requires_task_review(&store, &ticket.id, &session).unwrap());
        assert!(superagent::accept(&store, &ticket.id, waiting.revision).is_err());
        assert_eq!(superagent::get_ticket(&store, &ticket.id).unwrap().state,
            "waiting_approval");
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
    fn required_worker_success_binds_exact_artifact_and_replays_are_ignored() {
        let mut store=fixture();
        let (ticket,session,assignment_id)=required_worker(&mut store,"required-success");
        assert!(superagent::on_run_finished(&store,&session,true,"Worker artifact").unwrap());
        let waiting=superagent::get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(waiting.state,"waiting_review");
        let request=request_for_assignment(&store,&ticket.id,&assignment_id).unwrap().unwrap();
        assert_eq!(request.state,"bound");
        assert_eq!(request.ticket_revision,waiting.revision);
        assert_eq!(request.artifact_sha256.as_deref(),Some(sha256(b"Worker artifact").as_str()));
        assert!(request.native_session_id.is_none());
        assert!(!superagent::on_run_finished(&store,&session,true,"Replayed artifact").unwrap());
        assert_eq!(superagent::get_ticket(&store,&ticket.id).unwrap().state,"waiting_review");
        let state: String=store.db.query_row(
            "SELECT state FROM sa_council_requests WHERE assignment_id=?1",[&assignment_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(state,"bound");
        assert_eq!(superagent::history(&store,&ticket.id,20).unwrap()
            .iter().filter(|event| event.kind=="council_passed").count(),0);
    }

    #[test]
    fn failed_worker_invalidates_admitted_review_without_starting_reviewers() {
        let mut store=fixture();
        let (ticket,session,assignment_id)=required_worker(&mut store,"required-failure");
        assert!(superagent::on_run_finished(&store,&session,false,"Partial failure").unwrap());
        let failed=superagent::get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(failed.state,"failed");
        assert!(superagent::mandate(&store,&ticket.id).unwrap().revoked);
        let request=request_for_assignment(&store,&ticket.id,&assignment_id).unwrap().unwrap();
        assert_eq!(request.state,"blocked");
        assert!(request.native_session_id.is_none());
        assert!(!required_worker_session(&store,&session).unwrap());
        assert!(!superagent::on_run_finished(&store,&session,true,"Late success").unwrap());
        assert_eq!(superagent::get_ticket(&store,&ticket.id).unwrap().state,"failed");
    }

    #[test]
    fn stopping_a_native_review_revokes_request_and_returns_its_session() {
        let mut store=fixture();
        let (ticket,session,assignment_id)=required_worker(&mut store,"required-stop");
        assert!(superagent::on_run_finished(&store,&session,true,"Worker artifact").unwrap());
        let native_session="native-review-stop";
        store.db.execute(
            "UPDATE sa_council_requests SET native_session_id=?2,state='dispatched'
             WHERE assignment_id=?1",params![assignment_id,native_session],
        ).unwrap();
        let (stopped,session_to_stop)=superagent::stop(&store,&ticket.id).unwrap();
        assert_eq!(stopped.state,"stopped");
        assert_eq!(session_to_stop.as_deref(),Some(native_session));
        assert_eq!(request_for_assignment(&store,&ticket.id,&assignment_id).unwrap().unwrap().state,"blocked");
        assert!(superagent::mandate(&store,&ticket.id).unwrap().revoked);
        assert!(!superagent::on_run_finished(&store,&session,true,"Late worker result").unwrap());
        assert_eq!(superagent::get_ticket(&store,&ticket.id).unwrap().state,"stopped");
    }

    #[test]
    fn restart_blocks_required_review_and_prevents_delayed_worker_replay() {
        let mut store=fixture();
        let (ticket,session,assignment_id)=required_worker(&mut store,"required-restart");
        assert!(superagent::on_run_finished(&store,&session,true,"Worker artifact").unwrap());
        store.db.execute(
            "UPDATE sa_council_requests SET native_session_id='native-review',state='dispatched'
             WHERE assignment_id=?1",[&assignment_id],
        ).unwrap();
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        let recovered=superagent::get_ticket(&reopened,&ticket.id).unwrap();
        assert_eq!(recovered.state,"blocked");
        assert!(superagent::mandate(&reopened,&ticket.id).unwrap().revoked);
        assert!(!superagent::on_run_finished(&reopened,&session,true,"Late worker success").unwrap());
        assert_eq!(request_for_assignment(&reopened,&ticket.id,&assignment_id).unwrap().unwrap().state,"blocked");
        let finalizations:i64=reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_council_finalizations WHERE ticket_id=?1",[&ticket.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(finalizations,0);
        drop(reopened);
        let again=Store::open(&path).unwrap();
        assert_eq!(superagent::get_ticket(&again,&ticket.id).unwrap().state,"blocked");
        assert_eq!(request_for_assignment(&again,&ticket.id,&assignment_id).unwrap().unwrap().state,"blocked");
    }

    #[test]
    fn finalization_replay_is_bound_to_full_council_identity() {
        let mut store=fixture();
        let (ticket,session,assignment_id)=required_worker(&mut store,"required-replay");
        assert!(superagent::on_run_finished(&store,&session,true,"Worker artifact").unwrap());
        let native_session="native-review";
        store.db.execute(
            "UPDATE sa_council_requests SET native_session_id=?2,state='dispatched'
             WHERE assignment_id=?1",params![assignment_id,native_session],
        ).unwrap();
        let request=request_for_assignment(&store,&ticket.id,&assignment_id).unwrap().unwrap();
        let artifact=request.artifact_sha256.as_deref().unwrap();
        store.db.execute(
            "INSERT INTO sa_council_finalizations(
              id,request_id,ticket_id,assignment_id,worker_session_id,ticket_revision,
              policy_revision,cycle_id,cycle_started_at,generation,native_session_id,
              artifact_sha256,verdict,actor,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'passed','native-council',?13)",
            params![storage::id(),request.id,request.ticket_id,request.assignment_id,
                request.worker_session_id,request.ticket_revision,request.policy_revision,
                request.cycle_id,request.cycle_started_at,request.generation,native_session,
                artifact,storage::now()],
        ).unwrap();
        assert!(finalization_exists(&store,&request,"passed").unwrap());
        assert!(!finalization_exists(&store,&request,"human_accepted").unwrap());
        let mut stale=request.clone();
        stale.cycle_id.push_str("-stale");
        assert!(!finalization_exists(&store,&stale,"passed").unwrap());
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
