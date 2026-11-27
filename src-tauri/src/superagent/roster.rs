//! Persistent configured Team member roles, separate from runtime assignments.
//! Additional Workers/Reviewers do not become authorized providers or Council seats.
//! The current native runner remains single-worker until scoped role admission exists.
use super::{configuration,create_configuration,require_main_window,NewOrchestration,Orchestration};
use crate::{bots,storage::{self,Store},AppState,Result};
use rusqlite::{params,OptionalExtension};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::collections::{HashMap,HashSet};
use tauri::State;

pub(crate) const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_team_roles(
    orchestration_id TEXT NOT NULL REFERENCES sa_orchestrations(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('worker','reviewer')),
    position INTEGER NOT NULL CHECK(position >= 0),
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE RESTRICT,
    PRIMARY KEY(orchestration_id,role,position),
    UNIQUE(orchestration_id,bot_id)
);
CREATE INDEX IF NOT EXISTS sa_team_roles_bot ON sa_team_roles(bot_id);
"#;
/// Additive v46 sidecar. Stored profiles are admission evidence only, not
/// executable grants, verified harness capabilities, or Council decisions.
pub(crate) const TICKET_PROFILE_SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_ticket_profiles(
    ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    policy_revision INTEGER NOT NULL CHECK(policy_revision > 0),
    profiles_json TEXT NOT NULL CHECK(json_valid(profiles_json)),
    captured_at INTEGER NOT NULL
);
"#;

 // Additive policy, never a provider grant. Historic tickets are backfilled
 // with the original two-attempt ceiling in the v49 migration.
pub(crate) const ATTEMPT_POLICY_SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_team_attempt_limits(
    orchestration_id TEXT PRIMARY KEY REFERENCES sa_orchestrations(id) ON DELETE CASCADE,
    max_worker_attempts INTEGER NOT NULL CHECK(max_worker_attempts BETWEEN 1 AND 4)
);
CREATE TABLE IF NOT EXISTS sa_ticket_attempt_limits(
    ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    policy_revision INTEGER NOT NULL CHECK(policy_revision > 0),
    max_worker_attempts INTEGER NOT NULL CHECK(max_worker_attempts BETWEEN 1 AND 4)
);
"#;
/// v50: scenario and optimization direction are descriptive configuration,
/// never a tool grant, price promise, or permission to bypass acceptance.
pub(crate) const TEAM_STRATEGY_SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_team_strategies(
    orchestration_id TEXT PRIMARY KEY REFERENCES sa_orchestrations(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK(length(purpose)<=400),
    strategy TEXT NOT NULL CHECK(strategy IN ('balanced','economy','performance','security_critical','custom'))
);
CREATE TABLE IF NOT EXISTS sa_ticket_strategies(
    ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    policy_revision INTEGER NOT NULL CHECK(policy_revision>0),
    purpose TEXT NOT NULL CHECK(length(purpose)<=400),
    strategy TEXT NOT NULL CHECK(strategy IN ('balanced','economy','performance','security_critical','custom')),
    captured_at INTEGER NOT NULL
);
"#;
pub(super) fn validate_team_strategy(purpose:&str,strategy:&str)->Result<(String,String)> {
    let purpose=purpose.trim();
    if purpose.chars().count()>400 ||
        purpose.chars().any(|c| c.is_control() && c!='\n') {
        return Err("Team purpose must be at most 400 characters and contain no unsafe controls.".into());
    }
    if !matches!(strategy,"balanced"|"economy"|"performance"|"security_critical"|"custom") {
        return Err("Unsupported Team optimization strategy.".into());
    }
    Ok((purpose.into(),strategy.into()))
}
fn read_team_strategy(store:&Store,id:&str)->Result<(String,String)> {
    let saved:Option<(String,String)>=store.db.query_row(
        "SELECT purpose,strategy FROM sa_team_strategies WHERE orchestration_id=?1",
        [id],|row|Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    match saved {
        Some((purpose,strategy))=>validate_team_strategy(&purpose,&strategy)
            .and_then(|(p,s)|if p==purpose {Ok((p,s))} else {
                Err("Stored Team purpose is not canonical.".into())
            }),
        None=>Ok(("".into(),"balanced".into())),
    }
}
#[derive(Debug,Clone,Serialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct AdmittedTeamStrategy {
    pub policy_revision:i64,
    pub purpose:String,
    pub strategy:String,
    pub captured_at:i64,
}
pub(super) fn ticket_strategy(store:&Store,ticket_id:&str)->Result<Option<AdmittedTeamStrategy>> {
    store.db.query_row(
        "SELECT s.policy_revision,s.purpose,s.strategy,s.captured_at
         FROM sa_ticket_strategies s JOIN sa_tickets t
           ON t.id=s.ticket_id AND t.policy_revision=s.policy_revision
         WHERE s.ticket_id=?1",
        [ticket_id],|row|Ok(AdmittedTeamStrategy {
            policy_revision:row.get(0)?,purpose:row.get(1)?,
            strategy:row.get(2)?,captured_at:row.get(3)?,
        }),
    ).optional().map_err(|e|e.to_string())
}
pub(super) const MAX_WORKERS:usize=16;
pub(super) const MAX_REVIEWERS:usize=8;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RosterInput {
    pub worker_bot_ids:Vec<String>,
    pub reviewer_bot_ids:Vec<String>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TeamRoster {
    pub orchestration_id:String,
    pub policy_revision:i64,
    pub coordinator_bot_id:String,
    pub worker_bot_ids:Vec<String>,
    pub reviewer_bot_ids:Vec<String>,
    pub max_worker_attempts:i64,
    pub purpose:String,
    pub strategy:String,
    pub execution_ready:bool,
}
/// A bounded prompt package pinned at admission, not proof of execution.
/// Old Tickets without this field preserve their existing no-provisioning behavior.
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(tag="status",rename_all="snake_case",deny_unknown_fields)]
pub enum WorkerSkillPromptPin {
    Ready { sha256:String },
    Unavailable,
    Unsupported,
}

/// A bounded, configuration-only snapshot of actual Team member profiles.
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct AdmittedMemberProfile {
    pub role:String,
    pub position:usize,
    pub bot_id:String,
    pub project_id:String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub project_path:Option<String>,
    /// Resolved checkout identity at admission, when the directory exists.
    /// Legacy read-only snapshots without it remain compatible.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub resolved_project_path:Option<String>,
    /// Some(true/false) pins admission-time checkout existence.
    /// None preserves legacy read-only snapshots predating this witness.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub checkout_existed_at_admission:Option<bool>,
    pub provider:String,
    pub model:String,
    pub mode:String,
    pub reasoning_effort:String,
    pub assigned_skill_ids:Vec<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub worker_skill_prompt:Option<WorkerSkillPromptPin>,
    /// Length-framed digest of the original bot role and instructions. Raw prompt text stays private.
    /// Legacy read-only Tickets may omit this field; absence never authorizes tools.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub prompt_identity_sha256:Option<String>,
    /// SuperAgent's actual assignment brief includes the bot name. Pin it for
    /// new Tickets without changing the meaning of legacy prompt digests.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub brief_name_sha256:Option<String>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct AdmittedTeamProfile {
    pub policy_revision:i64,
    pub captured_at:i64,
    pub members:Vec<AdmittedMemberProfile>,
}

/// Validate stored execution fields using the native Bots editor's rules.
/// Legacy imports or stale database rows can bypass that editor. Provider
/// catalog membership does not prove harness installation or tool support.
pub(super) fn member_execution_issue(bot:&bots::Bot)->Option<&'static str> {
    if !bots::known_provider(&bot.provider) {
        Some("Bot harness is not supported by PandaCode.")
    } else if !storage::valid_permission_mode(&bot.mode) {
        Some("Bot permission mode is not supported by PandaCode.")
    } else if bot.model.len()>200 {
        Some("Bot model configuration exceeds the native limit.")
    } else if bot.reasoning_effort.len()>40
        || bot.reasoning_effort.chars().any(|c| !(c.is_ascii_alphanumeric() || c=='-' || c=='_')) {
        Some("Bot reasoning effort is invalid.")
    } else {
        None
    }
}

/// Bind the actual task-facing role and instructions without copying private text into the Ticket.
/// Length framing prevents ambiguous concatenations of role and instruction strings.
pub(super) fn prompt_identity(bot:&bots::Bot)->String {
    let mut digest=Sha256::new();
    for field in [&bot.role,&bot.instructions] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("{:x}",digest.finalize())
}

// Unlike ordinary bot conversations, SuperAgent briefs deliberately omit
// goals and soul. The bot name is included and must not drift mid-Demand.
pub(super) fn brief_name_identity(bot:&bots::Bot)->String {
    let mut digest=Sha256::new();
    digest.update(bot.name.as_bytes());
    format!("{:x}",digest.finalize())
}

fn member_profile(store:&Store,bot:&bots::Bot,role:&str,position:usize)->Result<AdmittedMemberProfile> {
    if let Some(issue)=member_execution_issue(bot) {
        return Err(issue.into());
    }
    let project_path:String=store.db.query_row(
        "SELECT path FROM projects WHERE id=?1",
        [&bot.project_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    if project_path.trim().is_empty() {
        return Err("Team member checkout path is unavailable.".into());
    }
    // Pin the filesystem target, not only the project path string: a local
    // symlink can be repointed without changing the stored project record.
    // A missing checkout is legacy read-only metadata, never a write grant.
    let resolved_project_path=match std::fs::symlink_metadata(&project_path) {
        Ok(_) => Some(std::fs::canonicalize(&project_path)
            .map_err(|_|"Team member checkout cannot be resolved safely.")?
            .to_string_lossy().into_owned()),
        Err(error) if error.kind()==std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Team member checkout cannot be inspected safely.".into()),
    };
    Ok(AdmittedMemberProfile {
        role:role.into(),position,bot_id:bot.id.clone(),
        project_id:bot.project_id.clone(),project_path:Some(project_path),
        checkout_existed_at_admission:Some(resolved_project_path.is_some()),
        resolved_project_path,
        provider:bot.provider.clone(),model:bot.model.clone(),mode:bot.mode.clone(),
        reasoning_effort:bot.reasoning_effort.clone(),
        assigned_skill_ids:bot.skills.clone(),
        worker_skill_prompt:None,
        prompt_identity_sha256:Some(prompt_identity(bot)),
        brief_name_sha256:Some(brief_name_identity(bot)),
    })
}

/// The actual desktop database and Skills Library share the application data
/// directory. Do not guess a library for an arbitrary detached database copy.
fn worker_skill_library(store:&Store)->Result<crate::skills::Library> {
    let root=store.path.parent().filter(|p|p.is_absolute())
        .ok_or("Worker Skill Library location is unavailable.")?;
    if store.path.file_name().and_then(|name|name.to_str())!=Some("kingcode.sqlite3") {
        return Err("Worker Skill Library is unavailable for a nonstandard database path.".into());
    }
    Ok(crate::skills::Library::new(root.join("skills"),root.to_path_buf()))
}

fn capture_worker_skill_prompt(store:&Store,member:&AdmittedMemberProfile)->Option<WorkerSkillPromptPin> {
    if member.role!="worker" || member.assigned_skill_ids.is_empty() {
        return None;
    }
    if !crate::skills::reviewer::supports_worker_prompt(&member.provider) {
        return Some(WorkerSkillPromptPin::Unsupported);
    }
    let result=worker_skill_library(store).and_then(|library|
        crate::skills::reviewer::worker_prompt(
            &library,&member.assigned_skill_ids,&member.provider,&Default::default(),
        )
    );
    Some(match result {
        Ok((sha256,_))=>WorkerSkillPromptPin::Ready{sha256},
        Err(_)=>WorkerSkillPromptPin::Unavailable,
    })
}

/// Re-resolve only the active native Worker and its exact admission package.
/// Never upgrade an unsupported/unavailable or historical snapshot using live
/// library state. The native provider receives a reference prompt, not a skill
/// invocation receipt; scripts, activation and additional tools remain forbidden.
pub(crate) fn worker_skill_suffix(store:&Store,session_id:&str)->Result<Option<String>> {
    worker_skill_suffix_with(store,session_id,||crate::skill_manager::disabled_for(session_id))
}

fn worker_skill_suffix_with(
    store:&Store,session_id:&str,disabled:impl FnOnce()->std::collections::BTreeSet<String>,
)->Result<Option<String>> {
    let assignment:Option<(String,String)>=store.db.query_row(
        "SELECT ticket_id,bot_id FROM sa_assignments WHERE session_id=?1",
        [session_id],|row|Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((ticket_id,bot_id))=assignment else {return Ok(None);};
    let Some(profile)=ticket_profile(store,&ticket_id)? else {return Ok(None);};
    let Some(member)=profile.members.iter().find(|m|m.role=="worker" && m.bot_id==bot_id)
        else {return Ok(None);};
    let Some(WorkerSkillPromptPin::Ready{sha256})=member.worker_skill_prompt.as_ref()
        else {return Ok(None);};
    if super::read_only_policy(store,session_id)?.is_none() {
        return Err("Worker Skill prompt has no active native authority.".into());
    }
    let library=worker_skill_library(store)?;
    let (actual,suffix)=crate::skills::reviewer::worker_prompt(
        &library,&member.assigned_skill_ids,&member.provider,&disabled(),
    )?;
    if &actual!=sha256 {
        return Err("Admitted Worker Skill package changed; create a newly authorized Demand.".into());
    }
    Ok(Some(suffix))
}

/// Called inside the same native SQLite transaction that creates the Ticket,
/// Mandate, audit and first Assignment. Capturing must fail atomically.
pub(super) fn capture_ticket_profile(
    store:&Store,ticket_id:&str,configuration:&super::Orchestration,at:i64,
)->Result<()> {
    let roster=read(store,&configuration.id)?;
    if roster.policy_revision!=configuration.policy_revision {
        return Err("Team policy changed during task admission.".into());
    }
    let mut profiles=Vec::new();
    let coordinator=bots::get(store,&roster.coordinator_bot_id)?;
    profiles.push(member_profile(store,&coordinator,"coordinator",0)?);
    for (position,id) in roster.worker_bot_ids.iter().enumerate() {
        let mut profile=member_profile(store,&bots::get(store,id)?,"worker",position)?;
        profile.worker_skill_prompt=capture_worker_skill_prompt(store,&profile);
        profiles.push(profile);
    }
    for (position,id) in roster.reviewer_bot_ids.iter().enumerate() {
        profiles.push(member_profile(store,&bots::get(store,id)?,"reviewer",position)?);
    }
    let json=serde_json::to_string(&profiles).map_err(|e|e.to_string())?;
    if json.len()>64*1024 {
        return Err("Team profile snapshot exceeds the 64 KiB admission limit.".into());
    }
    let inserted=store.db.execute(
        "INSERT INTO sa_ticket_profiles(ticket_id,policy_revision,profiles_json,captured_at)
         SELECT ?1,?2,?3,?4 FROM sa_tickets
         WHERE id=?1 AND orchestration_id=?5 AND policy_revision=?2",
        params![ticket_id,configuration.policy_revision,json,at,configuration.id],
    ).map_err(|e|e.to_string())?;
    if inserted!=1 {
        return Err("Task profile could not be pinned to the admitted Team revision.".into());
    }
    // Freeze the actual retry ceiling before the first Assignment can launch.
    // Editing this reusable Team must never retroactively widen a live Ticket.
    let max_attempts=team_worker_attempt_limit(store,&configuration.id)?;
    let pinned=store.db.execute(
        "INSERT INTO sa_ticket_attempt_limits(ticket_id,policy_revision,max_worker_attempts)
         SELECT ?1,?2,?3 FROM sa_tickets
         WHERE id=?1 AND orchestration_id=?4 AND policy_revision=?2",
        params![ticket_id,configuration.policy_revision,max_attempts,configuration.id],
    ).map_err(|e|e.to_string())?;
    if pinned!=1 {
        return Err("Task worker-attempt budget could not be frozen.".into());
    }
    let (purpose,strategy)=read_team_strategy(store,&configuration.id)?;
    let inserted=store.db.execute(
        "INSERT INTO sa_ticket_strategies(ticket_id,policy_revision,purpose,strategy,captured_at)
         SELECT ?1,?2,?3,?4,?5 FROM sa_tickets
         WHERE id=?1 AND orchestration_id=?6 AND policy_revision=?2",
        params![ticket_id,configuration.policy_revision,purpose,strategy,at,configuration.id],
    ).map_err(|e|e.to_string())?;
    if inserted!=1 {
        return Err("Task strategy could not be pinned to the admitted Team revision.".into());
    }
    Ok(())
}

pub(super) fn ticket_profile(store:&Store,ticket_id:&str)->Result<Option<AdmittedTeamProfile>> {
    use rusqlite::OptionalExtension;
    let row:Option<(i64,String,i64)>=store.db.query_row(
        "SELECT policy_revision,profiles_json,captured_at FROM sa_ticket_profiles
         WHERE ticket_id=?1",
        [ticket_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    row.map(|(policy_revision,json,captured_at)| {
        let members=serde_json::from_str::<Vec<AdmittedMemberProfile>>(&json)
            .map_err(|_|"Invalid persisted task-effective Team profile.".to_owned())?;
        let (ticket_revision, primary_worker, ticket_project):(i64,String,String)=store.db.query_row(
            "SELECT policy_revision,worker_bot_id,project_id FROM sa_tickets WHERE id=?1",
            [ticket_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).map_err(|e|e.to_string())?;
        if ticket_revision!=policy_revision || members.len()<2
            || members.len()>1+MAX_WORKERS+MAX_REVIEWERS {
            return Err("Admitted Team profile disagrees with immutable Ticket policy.".into());
        }
        // A stored JSON snapshot is not self-authenticating authority. Fail
        // closed on duplicate identities, reordered/forged roles or a Worker
        // whose original execution scope differs from the admitted Ticket.
        // Compare with the immutable Ticket, not the editable Team roster:
        // historical settled Demands must survive later Team revisions.
        let mut identities=HashSet::new();
        let (mut workers,mut reviewers)=(0_usize,0_usize);
        for (index,member) in members.iter().enumerate() {
            if member.prompt_identity_sha256.as_deref().is_some_and(|digest| {
                digest.len()!=64 || !digest.bytes().all(|b| matches!(b,b'0'..=b'9'|b'a'..=b'f'))
            }) {
                return Err("Invalid persisted task-effective bot prompt identity.".into());
            }
            if member.brief_name_sha256.as_deref().is_some_and(|digest| {
                digest.len()!=64 || !digest.bytes().all(|b| matches!(b,b'0'..=b'9'|b'a'..=b'f'))
            }) {
                return Err("Invalid persisted task-effective bot brief name identity.".into());
            }
            if member.worker_skill_prompt.as_ref().is_some_and(|pin|
                member.role!="worker" || member.assigned_skill_ids.is_empty()
                || matches!(pin,WorkerSkillPromptPin::Ready{sha256} if
                    !crate::skills::reviewer::supports_worker_prompt(&member.provider)
                    || sha256.len()!=64 || !sha256.bytes().all(|b|matches!(b,b'0'..=b'9'|b'a'..=b'f')))
            ) {
                return Err("Invalid persisted Worker Skill prompt admission identity.".into());
            }
            if member.checkout_existed_at_admission.is_some_and(|existed|
                existed != member.resolved_project_path.is_some()
            ) {
                return Err("Inconsistent admitted checkout existence evidence.".into());
            }
            if member.bot_id.is_empty() || !identities.insert(member.bot_id.as_str()) {
                return Err("Invalid persisted task-effective Team role identity.".into());
            }
            let valid=match member.role.as_str() {
                "coordinator"=>index==0 && member.position==0,
                "worker"=>index>0 && reviewers==0 && workers<MAX_WORKERS
                    && member.position==workers
                    && member.project_id==ticket_project,
                "reviewer"=>index>0 && workers>0 && reviewers<MAX_REVIEWERS
                    && member.position==reviewers,
                _=>false,
            };
            if !valid {
                return Err("Invalid persisted task-effective Team role ordering or scope.".into());
            }
            match member.role.as_str() {
                "worker"=>workers+=1,
                "reviewer"=>reviewers+=1,
                _=>{},
            }
        }
        if workers==0 || !members.iter().any(|member|
            member.role=="worker" && member.bot_id==primary_worker
        ) {
            return Err("Admitted Team profile has no matching selected Worker.".into());
        }
        Ok(AdmittedTeamProfile{policy_revision,captured_at,members})
    }).transpose()
}

/// Check project checkout identity at each native admission and settlement.
/// Historical read-only snapshots without this field remain compatible.
pub(super) fn checkout_paths_current(store:&Store,ticket_id:&str)->Result<bool> {
    use rusqlite::OptionalExtension;
    let Some(profile)=ticket_profile(store,ticket_id)? else {return Ok(true);};
    for member in profile.members {
        let Some(expected)=member.project_path else {continue;};
        let actual:Option<String>=store.db.query_row(
            "SELECT path FROM projects WHERE id=?1",
            [&member.project_id],|row|row.get(0),
        ).optional().map_err(|e|e.to_string())?;
        if actual.as_deref()!=Some(expected.as_str()) {return Ok(false);}
        // A repository alias must still resolve to the exact admitted root,
        // even if the projects.path text and native session ID are unchanged.
        if let Some(original)=member.resolved_project_path {
            let Some(current_path)=actual else {return Ok(false);};
            let Ok(resolved)=std::fs::canonicalize(current_path) else {return Ok(false);};
            if resolved.to_string_lossy().into_owned()!=original {return Ok(false);}
        } else if member.checkout_existed_at_admission==Some(false) {
            // The local checkout did not exist at admission. A subsequently
            // created directory or symlink must not silently acquire authority.
            // Existing legacy read-only snapshots retain their old semantics.
            match std::fs::symlink_metadata(&expected) {
                Err(error) if error.kind()==std::io::ErrorKind::NotFound => {},
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

/// New provider sessions for this Ticket may not silently adopt later
/// model/mode/Skill edits. Existing admitted sessions keep their own pinned
/// configuration, and legacy Tickets without this v46 evidence remain legacy.
pub(super) fn require_current_profile(
    store:&Store,ticket_id:&str,role:&str,bot_id:&str,
)->Result<()> {
    let Some(snapshot)=ticket_profile(store,ticket_id)? else {return Ok(());};
    let recorded=snapshot.members.iter()
        .find(|m|m.role==role && m.bot_id==bot_id)
        .ok_or("Bot role was not captured in the original Demand.")?;
    let mut current=member_profile(store,&bots::get(store,bot_id)?,role,recorded.position)?;
    if recorded.project_path.is_none() {current.project_path=None;}
    if recorded.resolved_project_path.is_none() {current.resolved_project_path=None;}
    if recorded.checkout_existed_at_admission.is_none() {
        current.checkout_existed_at_admission=None;
    }
    if recorded.prompt_identity_sha256.is_none() {current.prompt_identity_sha256=None;}
    // Older admitted snapshots did not pin display names. Keep their historical
    // admission semantics; never reinterpret a missing digest as a new grant.
    if recorded.brief_name_sha256.is_none() {current.brief_name_sha256=None;}
    // The immutable package is checked at the real provider boundary, not
    // regenerated by a reusable-profile comparison.
    current.worker_skill_prompt=recorded.worker_skill_prompt.clone();
    if *recorded!=current {
        return Err("Bot provider, model, mode, project or Skills changed after Demand admission; create a newly authorized Demand.".into());
    }
    Ok(())
}

pub(super) fn has_additional_members(store:&Store,id:&str)->Result<bool> {
    let count:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_team_roles WHERE orchestration_id=?1",[id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(count>0)
}
/// A missing Team row represents the original fixed policy, not permission
/// to retry without limits. SQLite checks reject corrupted values.
fn team_worker_attempt_limit(store:&Store,id:&str)->Result<i64> {
    let value:Option<i64>=store.db.query_row(
        "SELECT max_worker_attempts FROM sa_team_attempt_limits WHERE orchestration_id=?1",
        [id],|row|row.get(0),
    ).optional().map_err(|e|e.to_string())?;
    let limit=value.unwrap_or(2);
    if !(1..=4).contains(&limit) {
        return Err("Invalid persisted Team worker-attempt limit.".into());
    }
    Ok(limit)
}

/// Fail closed if a new or migrated Ticket lost its immutable attempt budget.
/// The migration explicitly pins the legacy two-attempt policy for old Tickets.
pub(super) fn ticket_worker_attempt_limit(store:&Store,ticket_id:&str)->Result<i64> {
    store.db.query_row(
        "SELECT l.max_worker_attempts FROM sa_ticket_attempt_limits l
         JOIN sa_tickets t ON t.id=l.ticket_id AND t.policy_revision=l.policy_revision
         WHERE t.id=?1",
        [ticket_id],|row|row.get(0),
    ).optional().map_err(|e|e.to_string())?
    .filter(|limit| (1..=4).contains(limit))
    .ok_or_else(||"Missing or invalid task-effective worker-attempt budget.".into())
}

pub(crate) fn read(store:&Store,id:&str)->Result<TeamRoster> {
    let config=configuration(store,id)?;
    let mut workers=vec![config.worker_bot_id.clone()];
    let mut reviewers=Vec::new();
    let mut stmt=store.db.prepare(
        "SELECT role,position,bot_id FROM sa_team_roles WHERE orchestration_id=?1
         ORDER BY role,position",
    ).map_err(|e|e.to_string())?;
    let rows=stmt.query_map([id],|row|Ok((
        row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,
    ))).map_err(|e|e.to_string())?;
    // Invalid imported/legacy metadata must never silently reorder, omit or
    // over-allocate trusted member roles during native preflight.
    let mut next_worker_position=1_i64;
    let mut next_reviewer_position=0_i64;
    for row in rows {
        let (role,position,bot_id)=row.map_err(|e|e.to_string())?;
        match role.as_str() {
            "worker" if position==next_worker_position && workers.len()<MAX_WORKERS=>{
                workers.push(bot_id);
                next_worker_position+=1;
            }
            "reviewer" if position==next_reviewer_position && reviewers.len()<MAX_REVIEWERS=>{
                reviewers.push(bot_id);
                next_reviewer_position+=1;
            }
            _=>return Err("Malformed Team role metadata; no authority granted.".into()),
        }
    }
    // Role/position keys do not prohibit one bot from occupying multiple
    // persisted slots. Legacy or damaged rosters must fail closed on read.
    let mut unique=HashSet::new();
    unique.insert(config.coordinator_bot_id.as_str());
    if workers.iter().chain(reviewers.iter())
        .any(|id|!unique.insert(id.as_str())) {
        return Err("Malformed Team role identity; no authority granted.".into());
    }
    let ready=workers.len()==1 && reviewers.is_empty();
    let (purpose,strategy)=read_team_strategy(store,id)?;
    Ok(TeamRoster {
        orchestration_id:config.id,
        policy_revision:config.policy_revision,
        coordinator_bot_id:config.coordinator_bot_id,
        worker_bot_ids:workers,
        reviewer_bot_ids:reviewers,
        max_worker_attempts:team_worker_attempt_limit(store,id)?,
        purpose,
        strategy,
        execution_ready:ready,
    })
}
/// Data from the already installed Skills Library. Trust is not provider eligibility.
#[derive(Debug,Clone,Copy)]
struct LibraryStatus {
    safety: &'static str,
}

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct EffectiveSkill {
    pub id:String,
    pub library_status:String,
    pub safety_status:String,
    pub harness_eligibility:String,
    pub invocation_evidence:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct EffectiveMember {
    pub role:String,
    pub position:usize,
    pub bot_id:String,
    pub name:String,
    pub provider:String,
    pub model:String,
    pub mode:String,
    pub reasoning_effort:String,
    pub project_id:String,
    pub paused:bool,
    /// False only when the referenced existing Bot has been removed.
    pub present:bool,
    pub skills:Vec<EffectiveSkill>,
    /// Local, attributable configuration issues; not an execution capability verdict.
    pub configuration_issues:Vec<String>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct EffectiveTeam {
    pub orchestration_id:String,
    pub policy_revision:i64,
    pub enabled:bool,
    pub execution_ready:bool,
    /// Native read-side checks for the currently supported manual admission path.
    /// Not proof of provider/tool support, Skill invocation or an issued grant.
    pub manual_admission_blockers:Vec<String>,
    pub members:Vec<EffectiveMember>,
    pub grants_issued:bool,
}

/// Read-only, policy-revision-bound description of configured bot identities.
/// Library presence or an accepted package finding cannot prove a harness will
/// actually invoke a Skill. No admission/permission is inferred by this view.
fn effective_team(
    store:&Store,id:&str,library:&HashMap<String,LibraryStatus>,
)->Result<EffectiveTeam> {
    let config=configuration(store,id)?;
    let roster=read(store,id)?;
    let mut members=Vec::new();
    let mut missing_member_ids=HashSet::new();
    let mut add=|role:&str,position:usize,bot_id:&str|->Result<()> {
        let bot=match bots::get(store,bot_id) {
            Ok(bot)=>bot,
            Err(error)=>{
                // Only a truly missing Bot can be represented as an unavailable
                // member. Propagate corrupt/unreadable profile errors as errors.
                let exists:i64=store.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM bots WHERE id=?1)",
                    [bot_id],|row|row.get(0),
                ).map_err(|e|e.to_string())?;
                if exists!=0 {return Err(error);}
                missing_member_ids.insert(bot_id.to_owned());
                members.push(EffectiveMember {
                    role:role.into(),position,bot_id:bot_id.into(),
                    name:"Missing bot".into(),provider:"unavailable".into(),
                    model:"unavailable".into(),mode:"unavailable".into(),
                    reasoning_effort:String::new(),project_id:"unavailable".into(),
                    paused:false,present:false,skills:Vec::new(),
                    configuration_issues:vec![
                        "Configured bot no longer exists; replace it in the disabled Team before admission.".into(),
                    ],
                });
                return Ok(());
            }
        };
        let skills:Vec<EffectiveSkill>=bot.skills.iter().map(|skill_id| {
            let state=library.get(skill_id);
            EffectiveSkill {
                id:skill_id.clone(),
                library_status:if state.is_some() {"installed"} else {"missing"}.into(),
                safety_status:state.map(|s|s.safety).unwrap_or("unknown").into(),
                harness_eligibility:"not_verified".into(),
                invocation_evidence:"not_verified".into(),
            }
        }).collect();
        let mut configuration_issues=Vec::new();
        if bot.paused {
            configuration_issues.push("Paused bot: resume it in Bots before admission.".into());
        }
        if let Some(issue)=member_execution_issue(&bot) {
            configuration_issues.push(issue.into());
        }
        // Worker project drift can occur after configuring a reusable Team.
        // Coordinator projects may differ; Workers own the execution scope.
        if role == "worker" && bot.project_id != config.project_id {
            configuration_issues.push(
                "Worker project differs from the Team execution project: restore its project or reconfigure this disabled Team.".into(),
            );
        }
        for skill in &skills {
            if skill.library_status=="missing" {
                configuration_issues.push(format!(
                    "Assigned Skill {} is missing: install or unassign it in Skills Manager.",skill.id,
                ));
            } else if skill.safety_status=="review_required" {
                configuration_issues.push(format!(
                    "Assigned Skill {} has unresolved safety findings: review it in Skills Manager.",
                    skill.id,
                ));
            } else if skill.safety_status=="integrity_mismatch" {
                configuration_issues.push(format!(
                    "Assigned Skill {} package integrity differs from the library metadata: review or reimport it in Skills Manager.",skill.id,
                ));
            } else if matches!(skill.safety_status.as_str(),
                "dependency_unavailable" | "dependency_cycle" | "dependency_limit") {
                configuration_issues.push(format!(
                    "Assigned Skill {} has an unresolved required dependency closure ({}): review it in Skills Manager.",
                    skill.id,skill.safety_status,
                ));
            } else if matches!(skill.safety_status.as_str(),"source_unpinned" | "unavailable") {
                configuration_issues.push(format!(
                    "Assigned Skill {} has an unavailable or unpinned package: review it in Skills Manager.",skill.id,
                ));
            }
        }
        members.push(EffectiveMember {
            role:role.into(),position,bot_id:bot.id,name:bot.name,
            provider:bot.provider,model:bot.model,mode:bot.mode,
            reasoning_effort:bot.reasoning_effort,
            project_id:bot.project_id,paused:bot.paused,present:true,skills,configuration_issues,
        });
        Ok(())
    };
    add("coordinator",0,&roster.coordinator_bot_id)?;
    for (position,id) in roster.worker_bot_ids.iter().enumerate() {
        add("worker",position,id)?;
    }
    for (position,id) in roster.reviewer_bot_ids.iter().enumerate() {
        add("reviewer",position,id)?;
    }
    // Report the same locally observable topology, pause and primary-project
    // barriers as the native manual admission path, without treating this
    // read-only diagnosis as an authorization or a harness capability check.
    let mut manual_admission_blockers=Vec::new();
    if !config.enabled {
        manual_admission_blockers.push(
            "Team is disabled; explicitly enable it before admitting a Demand.".into(),
        );
    }
    // execution_ready is the legacy single-worker topology flag, not a
    // verdict for explicitly selected Workers. Native manual admission
    // supports multiple Workers without configured Council reviewers.
    if !roster.reviewer_bot_ids.is_empty() {
        manual_admission_blockers.push(
            "Multi-role Team with configured Council reviewers requires native task-bound review; manual dispatch is unavailable.".into(),
        );
    }
    if crate::feature_controls::get().bots_paused {
        manual_admission_blockers.push(
            "Bots are globally paused; native manual admission is unavailable.".into(),
        );
    }
    for member in &members {
        if missing_member_ids.contains(&member.bot_id) {
            manual_admission_blockers.push(format!(
                "{} bot no longer exists; native admission is unavailable.",member.role,
            ));
            continue;
        }
        if (member.role=="coordinator" || member.role=="worker")
            && member.paused {
            manual_admission_blockers.push(format!(
                "{} bot is paused; native manual admission is unavailable.",member.role,
            ));
        }
        if member.role=="worker" && member.project_id!=config.project_id {
            manual_admission_blockers.push(format!(
                "Worker {} project differs from the Team execution project.",member.bot_id,
            ));
        }
        if member.role=="coordinator" || member.role=="worker" {
            let bot=bots::get(store,&member.bot_id)?;
            if let Some(issue)=member_execution_issue(&bot) {
                manual_admission_blockers.push(issue.into());
            }
        }
    }
    Ok(EffectiveTeam {
        orchestration_id:config.id,policy_revision:config.policy_revision,
        enabled:config.enabled,execution_ready:roster.execution_ready,
        manual_admission_blockers,members,grants_issued:false,
    })
}

/// Installed, assigned, harness-eligible and evidence-verified are separate
/// concepts. The native library scan can verify package presence and safety
/// findings; no tool is run and no Skill is installed/activated here.
#[tauri::command]
pub async fn superagent_effective_team(
    window:tauri::WebviewWindow,app:tauri::AppHandle,
    state:State<'_,AppState>,orchestration_id:String,
)->Result<EffectiveTeam> {
    require_main_window(&window)?;
    let initial={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        effective_team(&store,&orchestration_id,&HashMap::new())?
    };
    let assigned:HashSet<String>=initial.members.iter()
        .flat_map(|member|member.skills.iter().map(|skill|skill.id.clone())).collect();
    // Release the store lock before the bounded, read-only library scan.
    // The ordinary library listing may recover imports and mutate storage.
    let library=crate::skills::library(&app)?;
    let statuses=crate::skills::reviewer::inspect_team_skills(&library,&assigned)?;
    let statuses=statuses.into_iter().map(|(id,safety)|(id,LibraryStatus{safety})).collect();
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let current=effective_team(&store,&orchestration_id,&statuses)?;
    let current_assigned:HashSet<String>=current.members.iter()
        .flat_map(|member|member.skills.iter().map(|skill|skill.id.clone())).collect();
    if current.policy_revision!=initial.policy_revision || current_assigned!=assigned {
        return Err("Team configuration changed during Skills inspection; refresh the Team.".into());
    }
    Ok(current)
}

fn validate_member_ids(store:&Store,cfg:&Orchestration,input:&RosterInput)->Result<()> {
    if input.worker_bot_ids.is_empty() || input.worker_bot_ids.len()>MAX_WORKERS
       || input.reviewer_bot_ids.len()>MAX_REVIEWERS {
        return Err("Choose 1 to 16 Workers and at most 8 Reviewers.".into());
    }
    // The primary Coordinator is not listed in the additional-role table,
    // but must meet the same eligibility contract as every selected member.
    let coordinator = bots::get(store, &cfg.coordinator_bot_id)?;
    if coordinator.paused {
        return Err("Paused bots cannot join a Team.".into());
    }
    if let Some(issue) = member_execution_issue(&coordinator) {
        return Err(issue.into());
    }
    let mut unique=HashSet::new();
    unique.insert(cfg.coordinator_bot_id.as_str());
    for bot_id in input.worker_bot_ids.iter().chain(input.reviewer_bot_ids.iter()) {
        if !unique.insert(bot_id.as_str()) {
            return Err("A bot cannot occupy multiple Team roles.".into());
        }
        let bot=bots::get(store,bot_id)?;
        if bot.paused {return Err("Paused bots cannot join a Team.".into());}
        if let Some(issue)=member_execution_issue(&bot) {return Err(issue.into());}
        if input.worker_bot_ids.contains(bot_id) && bot.project_id!=cfg.project_id {
            return Err("All configured Workers must belong to the Team project.".into());
        }
    }
    Ok(())
}

fn insert_member_roles(store:&Store,id:&str,input:&RosterInput)->Result<()> {
    // The legacy worker_bot_id is a first-member compatibility projection.
    // A create or roster edit must update it in the same transaction as roles.
    if input.worker_bot_ids.first()!=Some(&configuration(store,id)?.worker_bot_id) {
        return Err("Team Worker compatibility projection differs from the roster.".into());
    }
    for (idx,bot_id) in input.worker_bot_ids.iter().enumerate().skip(1) {
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'worker',?2,?3)",params![id,idx as i64,bot_id],
        ).map_err(|e|e.to_string())?;
    }
    for (idx,bot_id) in input.reviewer_bot_ids.iter().enumerate() {
        store.db.execute(
            "INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
             VALUES(?1,'reviewer',?2,?3)",params![id,idx as i64,bot_id],
        ).map_err(|e|e.to_string())?;
    }
    Ok(())
}

/// Atomic creation never leaves a disabled orphan Team after failed role preflight.
pub(super) fn create_with_roster(
    store:&Store,input:NewOrchestration,roster:RosterInput,
)->Result<Orchestration> {
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let cfg=create_configuration(store,input)?;
    validate_member_ids(store,&cfg,&roster)?;
    insert_member_roles(store,&cfg.id,&roster)?;
    tx.commit().map_err(|e|e.to_string())?;
    configuration(store,&cfg.id)
}

/// Atomically create a disabled Team with a locally chosen retry policy.
/// Configuration is not a grant; invalid policies/roles must leave no partial Team.
pub(super) fn create_with_attempt_policy(
    store:&Store,input:NewOrchestration,roster:Option<RosterInput>,limit:i64,
)->Result<Orchestration> {
    create_with_attempt_policy_and_strategy(store,input,roster,limit,"","balanced")
}

/// Create a disabled Team with its descriptive policy in one transaction.
/// A suggested strategy cannot create an assignment, mandate or tool grant.
pub(super) fn create_with_attempt_policy_and_strategy(
    store:&Store,input:NewOrchestration,roster:Option<RosterInput>,limit:i64,
    purpose:&str,strategy:&str,
)->Result<Orchestration> {
    let (purpose,strategy)=validate_team_strategy(purpose,strategy)?;
    if !(1..=4).contains(&limit) {
        return Err("Worker-attempt limit must be between 1 and 4.".into());
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let cfg=create_configuration(store,input)?;
    if let Some(members)=roster {
        validate_member_ids(store,&cfg,&members)?;
        insert_member_roles(store,&cfg.id,&members)?;
    }
    tx.execute(
        "INSERT INTO sa_team_attempt_limits(orchestration_id,max_worker_attempts)
         VALUES(?1,?2)",
        params![&cfg.id,limit],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_team_strategies(orchestration_id,purpose,strategy)
         VALUES(?1,?2,?3)",
        params![&cfg.id,&purpose,&strategy],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    configuration(store,&cfg.id)
}

/// Clone one versioned Team policy and its ordered native roles in the same
/// SQLite transaction. A stale source revision, malformed stored roster, or
/// ineligible bot rolls back the entire disabled copy. No Tickets, source
/// subscriptions, grants, Council cycles, or historical evidence are copied.
pub(super) fn duplicate_with_roster(
    store: &Store, source_id: &str, expected_revision: i64, name: String,
) -> Result<Orchestration> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let source = configuration(store, source_id)?;
    if source.policy_revision != expected_revision {
        return Err("Source Team policy revision changed; reload before duplicating.".into());
    }
    let source_roles = read(store, source_id)?;
    if source_roles.policy_revision != expected_revision {
        return Err("Source Team roster changed; reload before duplicating.".into());
    }
    let created = create_configuration(store, NewOrchestration {
        name,
        coordinator_bot_id: source.coordinator_bot_id,
        worker_bot_id: source.worker_bot_id,
    })?;
    let roster = RosterInput {
        worker_bot_ids: source_roles.worker_bot_ids,
        reviewer_bot_ids: source_roles.reviewer_bot_ids,
    };
    validate_member_ids(store, &created, &roster)?;
    insert_member_roles(store, &created.id, &roster)?;
    // Duplicate durable policy, never task grants or source subscriptions.
    tx.execute(
        "INSERT INTO sa_team_attempt_limits(orchestration_id,max_worker_attempts)
         VALUES(?1,?2)",
        params![created.id,source_roles.max_worker_attempts],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_team_strategies(orchestration_id,purpose,strategy)
         VALUES(?1,?2,?3)",
        params![&created.id,&source_roles.purpose,&source_roles.strategy],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    configuration(store, &created.id)
}

pub(super) fn set(store:&Store,id:&str,expected:i64,input:&RosterInput)->Result<TeamRoster> {
    let cfg=configuration(store,id)?;
    if cfg.enabled || cfg.policy_revision!=expected {
        return Err("Disable the Team and reload its policy revision before changing members.".into());
    }
    // A reusable Team may change its roster after previous Demands settle.
    // Live or reviewable Tickets and uncertain effects still pin their exact
    // policy: never change that authority while work/reconciliation is open.
    let unsettled:bool=store.db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
              AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
                OR EXISTS(SELECT 1 FROM sa_assignments a
                          WHERE a.ticket_id=t.id AND a.state='running')
                OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                          WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain')))
         )",[id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    if unsettled {return Err("Active Tickets or unresolved effects prevent Team roster changes.".into());}
    validate_member_ids(store,&cfg,input)?;
    let previous=read(store,id)?;
    if previous.worker_bot_ids==input.worker_bot_ids
       && previous.reviewer_bot_ids==input.reviewer_bot_ids {return Ok(previous);}
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let changed=tx.execute(
        "UPDATE sa_orchestrations SET policy_revision=policy_revision+1,updated_at=?3,
            worker_bot_id=?4
         WHERE id=?1 AND enabled=0 AND policy_revision=?2
           AND NOT EXISTS(
             SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
               AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
                 OR EXISTS(SELECT 1 FROM sa_assignments a
                           WHERE a.ticket_id=t.id AND a.state='running')
                 OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                           WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain')))
           )",
        params![id,expected,storage::now(),&input.worker_bot_ids[0]],
    ).map_err(|e|e.to_string())?;
    if changed!=1{return Err("Concurrent Team policy change; reload required.".into());}
    tx.execute("DELETE FROM sa_team_roles WHERE orchestration_id=?1",[id])
        .map_err(|e|e.to_string())?;
    insert_member_roles(store,id,input)?;
    tx.commit().map_err(|e|e.to_string())?;
    read(store,id)
}
/// A trusted local-user edit of a reusable retry budget. Active/reviewable
/// work and unresolved effects pin their current Team policy; atomic SQL
/// rechecks those dependencies before the revision can change.
pub(super) fn set_worker_attempt_limit(
    store:&Store,id:&str,expected_revision:i64,limit:i64,
)->Result<TeamRoster> {
    if !(1..=4).contains(&limit) {
        return Err("Worker-attempt limit must be between 1 and 4.".into());
    }
    let current=read(store,id)?;
    let cfg=configuration(store,id)?;
    if cfg.enabled || current.policy_revision!=expected_revision {
        return Err("Disable the Team and reload its policy revision before changing budgets.".into());
    }
    if current.max_worker_attempts==limit {
        return Ok(current);
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let changed=tx.execute(
        "UPDATE sa_orchestrations SET policy_revision=policy_revision+1,updated_at=?3
         WHERE id=?1 AND enabled=0 AND policy_revision=?2
           AND NOT EXISTS(
              SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
                AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
                  OR EXISTS(SELECT 1 FROM sa_assignments a
                            WHERE a.ticket_id=t.id AND a.state='running')
                  OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                            WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain')))
           )",
        params![id,expected_revision,storage::now()],
    ).map_err(|e|e.to_string())?;
    if changed!=1 {
        return Err("Team policy or unsettled Demand changed before the budget edit.".into());
    }
    tx.execute(
        "INSERT INTO sa_team_attempt_limits(orchestration_id,max_worker_attempts)
         VALUES(?1,?2) ON CONFLICT(orchestration_id)
         DO UPDATE SET max_worker_attempts=excluded.max_worker_attempts",
        params![id,limit],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    read(store,id)
}

/// A strategy edit changes future Demand metadata only. It never changes
/// native bot profiles, execution modes, existing Tickets, or grant authority.
pub(super) fn set_team_strategy(
    store:&Store,id:&str,expected_revision:i64,purpose:&str,strategy:&str,
)->Result<TeamRoster> {
    let (purpose,strategy)=validate_team_strategy(purpose,strategy)?;
    let current=read(store,id)?;
    let cfg=configuration(store,id)?;
    if cfg.enabled || current.policy_revision!=expected_revision {
        return Err("Disable the Team and reload its policy revision before changing strategy.".into());
    }
    if current.purpose==purpose && current.strategy==strategy {
        return Ok(current);
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let changed=tx.execute(
        "UPDATE sa_orchestrations SET policy_revision=policy_revision+1,updated_at=?3
         WHERE id=?1 AND enabled=0 AND policy_revision=?2
           AND NOT EXISTS(
              SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
                AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
                  OR EXISTS(SELECT 1 FROM sa_assignments a
                            WHERE a.ticket_id=t.id AND a.state='running')
                  OR EXISTS(SELECT 1 FROM sa_effect_receipts e
                            WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain')))
           )",
        params![id,expected_revision,storage::now()],
    ).map_err(|e|e.to_string())?;
    if changed!=1 {
        return Err("Team policy or unsettled Demand changed before the strategy edit.".into());
    }
    tx.execute(
        "INSERT INTO sa_team_strategies(orchestration_id,purpose,strategy)
         VALUES(?1,?2,?3) ON CONFLICT(orchestration_id)
         DO UPDATE SET purpose=excluded.purpose,strategy=excluded.strategy",
        params![id,purpose,strategy],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    read(store,id)
}
#[tauri::command]
pub async fn superagent_team_strategy_update(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    orchestration_id:String,expected_revision:i64,purpose:String,strategy:String,
)->Result<TeamRoster> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    set_team_strategy(&store,&orchestration_id,expected_revision,&purpose,&strategy)
}

#[tauri::command]
pub async fn superagent_worker_attempt_limit_update(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    orchestration_id:String,expected_revision:i64,max_worker_attempts:i64,
)->Result<TeamRoster> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    set_worker_attempt_limit(&store,&orchestration_id,expected_revision,max_worker_attempts)
}

#[tauri::command]
pub async fn superagent_roster(window:tauri::WebviewWindow,state:State<'_,AppState>,
    orchestration_id:String)->Result<TeamRoster> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    read(&store,&orchestration_id)
}
#[tauri::command]
pub async fn superagent_roster_update(window:tauri::WebviewWindow,state:State<'_,AppState>,
    orchestration_id:String,expected_revision:i64,input:RosterInput)->Result<TeamRoster> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    set(&store,&orchestration_id,expected_revision,&input)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    #[test]
    fn duplicate_member_identity_and_malformed_persisted_position_fail_closed_after_restart() {
        let path=std::env::temp_dir().join(format!("sa-duplicate-roles-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-duplicate-roles')",[],
        ).unwrap();
        for id in ["lead","worker","additional","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=create_with_roster(&store,NewOrchestration {
            name:"Role identity".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker".into(),
        },RosterInput {
            worker_bot_ids:vec!["worker".into(),"additional".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        }).unwrap();
        assert_eq!(read(&store,&cfg.id).unwrap().reviewer_bot_ids,vec!["reviewer"]);
        // The persisted UNIQUE(orchestration_id,bot_id) constraint already
        // rejects duplicate Worker/Reviewer IDs before the native reader.
        let duplicate = store.db.execute(
            "UPDATE sa_team_roles SET bot_id='additional'
             WHERE orchestration_id=?1 AND role='reviewer'",
            [&cfg.id],
        );
        assert!(duplicate.is_err(), "SQLite must reject duplicate Team member identities");
        assert_eq!(read(&store,&cfg.id).unwrap().reviewer_bot_ids,vec!["reviewer"]);
        // A legal SQLite value can still corrupt role ordering; test the
        // independent native preflight guard on the persistent database.
        store.db.execute(
            "UPDATE sa_team_roles SET position=2
             WHERE orchestration_id=?1 AND role='reviewer'",
            [&cfg.id],
        ).unwrap();
        assert!(read(&store,&cfg.id).unwrap_err().contains("Malformed Team role metadata"));
        assert!(effective_team(&store,&cfg.id,&HashMap::new()).unwrap_err()
            .contains("Malformed Team role metadata"));
        let reopened=Store::open_read_only(&path).unwrap();
        assert!(read(&reopened,&cfg.id).unwrap_err().contains("Malformed Team role metadata"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|row|row.get::<_,i64>(0),
        ).unwrap(),0,"invalid metadata must never imply an authorized Demand");
    }

    #[cfg(unix)]
    #[test]
    fn admitted_checkout_alias_cannot_redirect_an_existing_provider_assignment() {
        use std::os::unix::fs::symlink;
        let root=std::env::temp_dir().join(format!("sa-checkout-pin-{}",storage::id()));
        let first=root.join("first");
        let second=root.join("second");
        let alias=root.join("checkout");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        symlink(&first,&alias).unwrap();
        let path=root.join("state.sqlite3");
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Checkout alias',?1)",
            [alias.to_string_lossy().into_owned()],
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=super::super::create_configuration(&store,NewOrchestration {
            name:"Pinned checkout".into(),
            coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        super::super::change_enabled(&store,&config.id,config.policy_revision,true).unwrap();
        let (ticket,launched)=super::super::admit(
            &mut store,&config.id,"Observe only the pinned checkout","pinned-alias",false,
        ).unwrap();
        assert!(launched);
        let session=super::super::assignment_session(&store,&ticket.id).unwrap().unwrap();
        let snapshot=ticket_profile(&store,&ticket.id).unwrap().unwrap();
        let canonical=std::fs::canonicalize(&first).unwrap().to_string_lossy().into_owned();
        assert!(snapshot.members.iter().all(|member|
            member.resolved_project_path.as_deref()==Some(canonical.as_str())));
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());

        std::fs::remove_file(&alias).unwrap();
        symlink(&second,&alias).unwrap();
        assert!(!checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_err());
        assert!(super::super::read_only_policy(&store,&session).is_err());
        let reopened=Store::open_read_only(&path).unwrap();
        assert!(!checkout_paths_current(&reopened,&ticket.id).unwrap());
        assert!(super::super::read_only_policy(&reopened,&session).is_err());
        drop(reopened);

        std::fs::remove_file(&alias).unwrap();
        symlink(&first,&alias).unwrap();
        assert!(checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());
        assert!(!super::super::mandate(&store,&ticket.id).unwrap().allowed_side_effects);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }


    #[test]
    fn missing_checkout_appearance_blocks_native_admission_and_preserves_legacy_snapshots() {
        let root=std::env::temp_dir().join(format!("sa-missing-checkout-{}",storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        let checkout=root.join("checkout");
        assert!(!checkout.exists());
        let path=root.join("state.sqlite3");
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Pending checkout',?1)",
            [checkout.to_string_lossy().into_owned()],
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=super::super::create_configuration(&store,NewOrchestration {
            name:"Missing checkout".into(),
            coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        super::super::change_enabled(&store,&config.id,config.policy_revision,true).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&config.id,"Observe the original checkout","missing-checkout",false,
        ).unwrap();
        assert!(started);
        let session=super::super::assignment_session(&store,&ticket.id).unwrap().unwrap();
        let profile=ticket_profile(&store,&ticket.id).unwrap().unwrap();
        assert!(profile.members.iter().all(|member|
            member.checkout_existed_at_admission==Some(false)
                && member.resolved_project_path.is_none()));
        assert!(checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());

        std::fs::create_dir(&checkout).unwrap();
        assert!(!checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_err());
        assert!(super::super::read_only_policy(&store,&session).is_err());
        assert!(super::super::guard_run_admission(&store,&session,&[]).is_err());
        let reopened=Store::open_read_only(&path).unwrap();
        assert!(!checkout_paths_current(&reopened,&ticket.id).unwrap());
        assert!(super::super::read_only_policy(&reopened,&session).is_err());
        drop(reopened);

        // A snapshot from before this optional field existed remains readable
        // and read-only rather than gaining any new tool authorization.
        let raw:String=store.db.query_row(
            "SELECT profiles_json FROM sa_ticket_profiles WHERE ticket_id=?1",
            [&ticket.id],|row|row.get(0),
        ).unwrap();
        let mut legacy:Vec<serde_json::Value>=serde_json::from_str(&raw).unwrap();
        for member in &mut legacy {
            member.as_object_mut().unwrap().remove("checkoutExistedAtAdmission");
        }
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
            params![&ticket.id,serde_json::to_string(&legacy).unwrap()],
        ).unwrap();
        assert!(ticket_profile(&store,&ticket.id).unwrap().unwrap().members
            .iter().all(|member|member.checkout_existed_at_admission.is_none()));
        assert!(checkout_paths_current(&store,&ticket.id).unwrap());
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());
        assert!(!super::super::mandate(&store,&ticket.id).unwrap().allowed_side_effects);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_stored_harness_mode_and_reasoning_fail_closed_before_manual_admission() {
        let path=std::env::temp_dir().join(format!("sa-profile-preflight-{}.sqlite3",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-profile-preflight')",[],
        ).unwrap();
        for id in ["lead","worker","second"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=super::super::create_configuration(&store,NewOrchestration {
            name:"Capability validation".into(),
            coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        super::super::change_enabled(&store,&cfg.id,cfg.policy_revision,true).unwrap();

        for (field,invalid,restored,expected) in [
            ("provider","unsupported-harness","codex","Bot harness"),
            ("mode","root-access","plan","Bot permission mode"),
            ("reasoning_effort","spaces invalid","","Bot reasoning effort"),
        ] {
            let sql=format!("UPDATE bots SET {field}=?1 WHERE id='worker'");
            store.db.execute(&sql,[invalid]).unwrap();
            let view=effective_team(&store,&cfg.id,&HashMap::new()).unwrap();
            assert!(view.members[1].configuration_issues.iter().any(|s|s.contains(expected)));
            assert!(view.manual_admission_blockers.iter().any(|s|s.contains(expected)));
            let request=format!("invalid-profile-{field}");
            let err=super::super::admit(
                &mut store,&cfg.id,"Do not start unsupported execution",&request,false,
            ).unwrap_err();
            assert!(err.contains(expected),"{field}: {err}");
            assert_eq!(store.db.query_row(
                "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0),
            ).unwrap(),0,"invalid profiles must roll back the Ticket transaction");
            assert_eq!(store.db.query_row(
                "SELECT COUNT(*) FROM sa_assignments",[],|r|r.get::<_,i64>(0),
            ).unwrap(),0,"invalid profiles must not reserve an Assignment");
            store.db.execute(&sql,[restored]).unwrap();
        }

        store.db.execute(
            "UPDATE bots SET provider='unsupported-harness' WHERE id='second'",[],
        ).unwrap();
        // Disabled multi-role Teams must not retain unsupported extra workers.
        let disabled=super::super::change_enabled(
            &store,&cfg.id,cfg.policy_revision+1,false,
        ).unwrap();
        let rejected=set(&store,&cfg.id,disabled.policy_revision,&RosterInput {
            worker_bot_ids:vec!["worker".into(),"second".into()],
            reviewer_bot_ids:vec![],
        }).unwrap_err();
        assert!(rejected.contains("Bot harness"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_team_roles",[],|r|r.get::<_,i64>(0),
        ).unwrap(),0);
        store.db.execute("UPDATE bots SET provider='codex' WHERE id='second'",[])
            .unwrap();
        let enabled=super::super::change_enabled(
            &store,&cfg.id,disabled.policy_revision,true,
        ).unwrap();
        assert!(enabled.enabled);
        let (ticket,launched)=super::super::admit(
            &mut store,&cfg.id,"Valid read-only request","valid-profile",false,
        ).unwrap();
        assert!(launched);
        assert!(ticket_profile(&store,&ticket.id).unwrap().is_some());
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_tickets WHERE id=?1",[&ticket.id],
            |r|r.get::<_,String>(0),
        ).unwrap(),"running");
    }

    #[test]
    fn admitted_team_snapshot_rejects_corrupt_roles_without_reauthorizing_execution() {
        let path=std::env::temp_dir().join(format!("sa-profile-integrity-{}.sqlite3",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-profile-integrity')",[],
        ).unwrap();
        for id in ["coordinator","primary"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=super::super::create_configuration(&store,NewOrchestration {
            name:"Snapshot integrity".into(),
            coordinator_bot_id:"coordinator".into(),worker_bot_id:"primary".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&cfg.id],
        ).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&cfg.id,"Unchanged scope","snapshot-integrity",false,
        ).unwrap();
        assert!(started);
        let original=ticket_profile(&store,&ticket.id).unwrap().unwrap().members;
        assert_eq!(original.len(),2);

        let mut duplicate=original.clone();
        duplicate[1].bot_id=duplicate[0].bot_id.clone();
        let mut swapped=original.clone();
        swapped.swap(0,1);
        let mut wrong_primary=original.clone();
        wrong_primary[1].bot_id="unassigned-worker".into();
        let mut wrong_project=original.clone();
        wrong_project[1].project_id="unassigned-project".into();
        let mut skipped_position=original.clone();
        skipped_position[1].position=1;
        let mut missing_worker=original.clone();
        missing_worker[1].role="reviewer".into();
        let mut reviewer_first=original.clone();
        reviewer_first[1].role="reviewer".into();
        let mut later_worker=original[1].clone();
        later_worker.bot_id="another-worker".into();
        later_worker.position=1;
        reviewer_first.push(later_worker);
        for (case,members) in [
            ("duplicate",duplicate),("swapped",swapped),
            ("wrong primary",wrong_primary),("wrong project",wrong_project),
            ("skipped position",skipped_position),("no worker",missing_worker),
            ("worker after reviewer",reviewer_first),
        ] {
            let json=serde_json::to_string(&members).unwrap();
            store.db.execute(
                "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
                params![&ticket.id,json],
            ).unwrap();
            assert!(ticket_profile(&store,&ticket.id).is_err(),"{case} must be rejected");
            assert!(require_current_profile(&store,&ticket.id,"worker","primary").is_err(),
                "{case} must not authorize provider admission");
        }
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
            params![&ticket.id,serde_json::to_string(&original).unwrap()],
        ).unwrap();
        assert_eq!(ticket_profile(&store,&ticket.id).unwrap().unwrap().members,original);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(ticket_profile(&reopened,&ticket.id).unwrap().unwrap().members,original);
    }

    #[test]
    fn worker_prompt_is_admission_pinned_and_required_at_native_launch() {
        let root=std::env::temp_dir().join(format!("sa-worker-prompt-{}",storage::id()));
        let checkout=root.join("checkout");
        std::fs::create_dir_all(&checkout).unwrap();
        let path=root.join("kingcode.sqlite3");
        let mut store=Store::open(&path).unwrap();
        let library=crate::skills::Library::new(root.join("skills"),root.clone());
        let source=|| crate::skills::Source {
            kind:"folder".into(),location:"fixture".into(),git_ref:None,commit:None,
        };
        let import=|name:&str,body:&str,dependencies:&[&str]| {
            let mut files=vec![("SKILL.md".to_owned(),
                format!("---\nname: {name}\n---\n{body}").into_bytes())];
            if !dependencies.is_empty() {
                files.push(("skill-dependencies.json".into(),
                    serde_json::to_vec(&serde_json::json!({"requiredSkills":dependencies})).unwrap()));
            }
            library.import(crate::skills::Kind::Skill,files,name,source()).unwrap();
        };
        import("sa-prompt-dep","Check the dependency evidence.",&[]);
        import("sa-prompt-root","Inspect the authorized fixture.",&["sa-prompt-dep"]);
        import("sa-prompt-other","Unrelated private instructions.",&[]);
        store.db.execute("INSERT INTO projects(id,name,path) VALUES('p','Fixture',?1)",
            params![checkout.to_string_lossy().into_owned()]).unwrap();
        for (id,provider) in [("lead","codex"),("worker","kimi")] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p',?2,'default','plan',?3,?3)",
                params![id,provider,storage::now()],
            ).unwrap();
        }
        store.db.execute("UPDATE bots SET skills='[\"sa-prompt-root\"]' WHERE id='worker'",[]).unwrap();
        let team=super::super::create_configuration(&store,NewOrchestration {
            name:"Prompt fixture".into(),coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        super::super::change_enabled(&store,&team.id,team.policy_revision,true).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&team.id,"Inspect the fixture","worker-prompt",false,
        ).unwrap();
        assert!(started);
        let session_id=super::super::assignment_session(&store,&ticket.id).unwrap().unwrap();
        let original=ticket_profile(&store,&ticket.id).unwrap().unwrap().members;
        assert!(original[0].worker_skill_prompt.is_none());
        assert!(matches!(original[1].worker_skill_prompt.as_ref(),Some(WorkerSkillPromptPin::Ready{..})));
        let suffix=worker_skill_suffix_with(&store,&session_id,std::collections::BTreeSet::new)
            .unwrap().unwrap();
        assert!(suffix.contains("Inspect the authorized fixture."));
        assert!(suffix.contains("Check the dependency evidence."));
        assert!(!suffix.contains("Unrelated private instructions."));
        assert!(!suffix.contains("Read-only pinned files:"));
        assert!(worker_skill_suffix_with(&store,&session_id,||
            std::collections::BTreeSet::from(["sa-prompt-dep".to_owned()])).is_err());

        let session=store.session(&session_id).unwrap();
        let mut config=crate::providers::RunConfig {
            session_id:session_id.clone(),provider:session.provider,model:session.model,
            reasoning_effort:session.reasoning_effort,mode:"plan".into(),reviewer_boundary:true,
            reviewer_policy:super::super::read_only_policy(&store,&session_id).unwrap(),
            cwd:store.workspace_root("p",Some(&session_id)).unwrap(),resume_id:None,
            prompt:"Inspect".into(),instructions:String::new(),skill:None,goal:None,
            codex_settings:None,images:Vec::new(),
        };
        assert!(super::super::guard_prepared_provider_launch(&store,&config).is_err(),
            "a prepared native launch must carry the exact pinned prompt");
        config.prompt.push_str(&suffix);
        // The native launch path can append application/title instructions after
        // preparing the Worker input; the complete pinned block must still exist.
        config.prompt.push_str("\nApplication instructions.");
        assert!(super::super::guard_prepared_provider_launch(&store,&config).is_ok());
        assert!(config.skill.is_none(),"prompt delivery is not a native invocation receipt");
        let ordinary=store.create_session("p","kimi","default","ask").unwrap();
        assert!(worker_skill_suffix_with(&store,&ordinary.id,std::collections::BTreeSet::new)
            .unwrap().is_none());
        assert_eq!(std::fs::read_dir(&checkout).unwrap().count(),0);
        assert!(!root.join("skills/.council-reviewer-snapshots").exists());
        import("sa-prompt-dep","A newly imported dependency revision.",&[]);
        assert!(worker_skill_suffix_with(&store,&session_id,std::collections::BTreeSet::new).is_err());
        assert!(super::super::guard_prepared_provider_launch(&store,&config).is_err());
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(ticket_profile(&reopened,&ticket.id).unwrap().unwrap().members,original);
        assert!(worker_skill_suffix_with(&reopened,&session_id,std::collections::BTreeSet::new).is_err());
        drop(reopened);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn admitted_brief_name_is_frozen_without_breaking_legacy_tickets() {
        let path=std::env::temp_dir().join(format!("sa-brief-name-{}.sqlite3",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-brief-name')",[],
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let team=super::super::create_configuration(&store,NewOrchestration {
            name:"Brief identity".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        super::super::change_enabled(&store,&team.id,team.policy_revision,true).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&team.id,"Inspect the fixture","brief-name",false,
        ).unwrap();
        assert!(started);
        let frozen=ticket_profile(&store,&ticket.id).unwrap().unwrap().members;
        assert!(frozen.iter().all(|member|member.brief_name_sha256.is_some()));
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_ok());
        let session=super::super::assignment_session(&store,&ticket.id).unwrap().unwrap();
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());

        // The name is part of bots::brief::superagent. A new assignment
        // cannot silently adopt a different effective prompt after admission.
        store.db.execute("UPDATE bots SET name='Renamed worker' WHERE id='worker'",[])
            .unwrap();
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_err());
        // A live provider/tool admission is fenced as well, not just retries.
        assert!(super::super::read_only_policy(&store,&session).is_err());
        let reopened_active=Store::open_read_only(&path).unwrap();
        assert!(super::super::read_only_policy(&reopened_active,&session).is_err());
        drop(reopened_active);
        assert_eq!(ticket_profile(&store,&ticket.id).unwrap().unwrap().members,frozen);
        store.db.execute("UPDATE bots SET name='worker' WHERE id='worker'",[]).unwrap();
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_ok());
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());

        // Historical v46 snapshots omitted the name digest. Treat that
        // absence as legacy read-only evidence, not as a fabricated digest.
        let mut legacy=frozen.clone();
        legacy[1].brief_name_sha256=None;
        store.db.execute(
            "UPDATE sa_ticket_profiles SET profiles_json=?2 WHERE ticket_id=?1",
            params![&ticket.id,serde_json::to_string(&legacy).unwrap()],
        ).unwrap();
        store.db.execute("UPDATE bots SET name='Renamed worker' WHERE id='worker'",[])
            .unwrap();
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_ok());
        assert!(super::super::read_only_policy(&store,&session).unwrap().is_some());
        // The original legacy role/instructions identity still rejects drift.
        store.db.execute(
            "UPDATE bots SET instructions='New instruction' WHERE id='worker'",[],
        ).unwrap();
        assert!(require_current_profile(&store,&ticket.id,"worker","worker").is_err());
        assert!(super::super::read_only_policy(&store,&session).is_err());
        assert!(super::super::on_run_finished(&store,&session,true,
            "Untrusted prompt from changed live bot").unwrap());
        let blocked=super::super::get_ticket(&store,&ticket.id).unwrap();
        assert_eq!(blocked.state,"blocked");
        assert!(blocked.evidence.is_none());
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(
            ticket_profile(&reopened,&ticket.id).unwrap().unwrap().members,legacy,
        );
        assert!(require_current_profile(&reopened,&ticket.id,"worker","worker").is_err());
    }

    #[test]
    fn effective_team_reports_actual_roles_models_and_skill_inventory_without_grants() {
        let path=std::env::temp_dir().join(format!("sa-effective-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('effective-project','Fixture','/tmp/sa-effective')",[],
        ).unwrap();
        for id in ["lead","worker","another-worker","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'effective-project','codex','gpt-default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        store.db.execute(
            "UPDATE bots SET skills='[\"known\",\"missing\"]',reasoning_effort='high' WHERE id='worker'",[],
        ).unwrap();
        store.db.execute("UPDATE bots SET paused=1 WHERE id='reviewer'",[]).unwrap();
        let cfg=super::super::create_configuration(&store,super::super::NewOrchestration {
            name:"Effective".into(),coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        let one=effective_team(&store,&cfg.id,&HashMap::new()).unwrap();
        assert!(one.execution_ready);
        assert!(!one.enabled);
        assert!(one.manual_admission_blockers.iter()
            .any(|reason|reason.contains("Team is disabled")));
        assert_eq!(one.members.len(),2);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='worker'",[]).unwrap();
        let paused_primary=effective_team(&store,&cfg.id,&HashMap::new()).unwrap();
        assert!(paused_primary.manual_admission_blockers.iter()
            .any(|reason|reason.contains("worker bot is paused")));
        assert!(!paused_primary.grants_issued);
        store.db.execute("UPDATE bots SET paused=0 WHERE id='worker'",[]).unwrap();
        assert!(!one.grants_issued);
        assert_eq!(one.members[1].skills[0].library_status,"missing");
        let roster=set(&store,&cfg.id,cfg.policy_revision,&RosterInput {
            worker_bot_ids:vec!["worker".into(),"another-worker".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        }).unwrap_err();
        assert!(roster.contains("Paused bots"));
        store.db.execute("UPDATE bots SET paused=0 WHERE id='reviewer'",[]).unwrap();
        let roster=set(&store,&cfg.id,cfg.policy_revision,&RosterInput {
            worker_bot_ids:vec!["worker".into(),"another-worker".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        }).unwrap();
        let library=HashMap::from([("known".to_string(),LibraryStatus{safety:"review_required"})]);
        let view=effective_team(&store,&cfg.id,&library).unwrap();
        let changed_library=HashMap::from([("known".to_string(),LibraryStatus{safety:"integrity_mismatch"})]);
        let changed=effective_team(&store,&cfg.id,&changed_library).unwrap();
        assert_eq!(changed.members[1].skills[0].library_status,"installed");
        assert_eq!(changed.members[1].skills[0].safety_status,"integrity_mismatch");
        assert_eq!(changed.members[1].skills[0].invocation_evidence,"not_verified");
        assert!(changed.members[1].configuration_issues.iter().any(|issue|
            issue.contains("package integrity")));
        assert!(!changed.grants_issued);
        for status in ["dependency_unavailable", "dependency_cycle", "dependency_limit"] {
            let unresolved = HashMap::from([("known".to_string(), LibraryStatus { safety: status })]);
            let report = effective_team(&store, &cfg.id, &unresolved).unwrap();
            let member = &report.members[1];
            assert_eq!(member.skills[0].library_status, "installed");
            assert_eq!(member.skills[0].safety_status, status);
            assert_eq!(member.skills[0].invocation_evidence, "not_verified");
            assert!(member.configuration_issues.iter().any(|issue|
                issue.contains("unresolved required dependency closure") && issue.contains(status)));
            assert!(!report.grants_issued);
        }
        assert_eq!(view.policy_revision,roster.policy_revision);
        assert_eq!(view.members.iter().map(|m|m.role.as_str()).collect::<Vec<_>>(),
            vec!["coordinator","worker","worker","reviewer"]);
        assert_eq!(view.members[1].model,"gpt-default");
        assert_eq!(view.members[1].reasoning_effort,"high");
        assert_eq!(view.members[1].skills[0].library_status,"installed");
        assert_eq!(view.members[1].skills[0].safety_status,"review_required");
        assert_eq!(view.members[1].skills[0].harness_eligibility,"not_verified");
        assert_eq!(view.members[1].skills[0].invocation_evidence,"not_verified");
        assert_eq!(view.members[1].skills[1].library_status,"missing");
        assert_eq!(view.members[1].configuration_issues.len(),2);
        assert!(view.members[1].configuration_issues.iter().any(|issue|
            issue.contains("missing") && issue.contains("Skills Manager")));
        assert!(view.members[1].configuration_issues.iter().any(|issue|
            issue.contains("unresolved safety findings")));
        assert!(view.members[0].configuration_issues.is_empty());
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('moved-project','Moved','/tmp/sa-moved')",[],
        ).unwrap();
        store.db.execute(
            "UPDATE bots SET project_id='moved-project' WHERE id='another-worker'",[],
        ).unwrap();
        let reopened=Store::open_read_only(&path).unwrap();
        let drift=effective_team(&reopened,&cfg.id,&library).unwrap();
        assert_eq!(drift.policy_revision,roster.policy_revision);
        assert!(drift.members[2].configuration_issues.iter().any(|issue|
            issue.contains("Worker project differs from the Team execution project")));
        assert!(drift.members[0].configuration_issues.is_empty());
        assert!(!drift.grants_issued);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='reviewer'",[]).unwrap();
        let paused=effective_team(&store,&cfg.id,&library).unwrap();
        assert_eq!(paused.policy_revision,roster.policy_revision);
        assert!(paused.members[3].configuration_issues.iter().any(|issue|
            issue.contains("Paused bot")));
        assert!(!paused.grants_issued);
        assert!(!view.execution_ready);
        assert!(view.manual_admission_blockers.iter()
            .any(|reason|reason.contains("Multi-role Team")));
        assert!(!view.grants_issued);
        assert!(view.members.iter().all(|member|member.present));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0)).unwrap(),0);

        // Explicitly selected multi-Worker admission is supported without
        // Council. Restore valid profiles and verify the actual preflight agrees
        // with the read-only Team projection, including secondary Worker drift.
        store.db.execute("UPDATE bots SET project_id='effective-project' WHERE id='another-worker'",[]).unwrap();
        store.db.execute("UPDATE bots SET paused=0 WHERE id='reviewer'",[]).unwrap();
        let multi=set(&store,&cfg.id,roster.policy_revision,&RosterInput {
            worker_bot_ids:vec!["worker".into(),"another-worker".into()],reviewer_bot_ids:Vec::new(),
        }).unwrap();
        let multi_enabled=super::super::change_enabled(&store,&cfg.id,multi.policy_revision,true).unwrap();
        let multi_report=effective_team(&store,&cfg.id,&library).unwrap();
        assert!(!multi_report.execution_ready, "Legacy single-worker topology flag is unchanged");
        assert!(multi_report.manual_admission_blockers.is_empty());
        assert!(super::super::manual_preflight_worker(&store,&cfg.id,"Inspect","another-worker",false).unwrap().eligible);
        assert!(!multi_report.grants_issued);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='another-worker'",[]).unwrap();
        let paused_peer=effective_team(&store,&cfg.id,&library).unwrap();
        assert!(paused_peer.manual_admission_blockers.iter().any(|reason|reason.contains("worker bot is paused")));
        assert!(super::super::manual_preflight_worker(&store,&cfg.id,"Inspect","another-worker",false).is_err());
        store.db.execute("UPDATE bots SET paused=0 WHERE id='another-worker'",[]).unwrap();
        store.db.execute("UPDATE bots SET project_id='moved-project' WHERE id='another-worker'",[]).unwrap();
        let moved_peer=effective_team(&store,&cfg.id,&library).unwrap();
        assert!(moved_peer.manual_admission_blockers.iter().any(|reason|reason.contains("another-worker")));
        assert!(super::super::manual_preflight_worker(&store,&cfg.id,"Inspect","another-worker",false).is_err());
        store.db.execute("UPDATE bots SET project_id='effective-project' WHERE id='another-worker'",[]).unwrap();
        let multi_off=super::super::change_enabled(&store,&cfg.id,multi_enabled.policy_revision,false).unwrap();

        // Topology readiness is not user authorization: enabling and disabling
        // the same supported Team must change native admission diagnostics.
        let reset=set(&store,&cfg.id,multi_off.policy_revision,&RosterInput {
            worker_bot_ids:vec!["worker".into()],reviewer_bot_ids:Vec::new(),
        }).unwrap();
        let enabled=super::super::change_enabled(&store,&cfg.id,reset.policy_revision,true).unwrap();
        let ready=effective_team(&store,&cfg.id,&HashMap::new()).unwrap();
        assert!(ready.enabled);
        assert!(ready.manual_admission_blockers.is_empty());
        assert!(!ready.grants_issued);
        super::super::change_enabled(&store,&cfg.id,enabled.policy_revision,false).unwrap();
        let reopened=Store::open_read_only(&path).unwrap();
        let off=effective_team(&reopened,&cfg.id,&HashMap::new()).unwrap();
        assert!(!off.enabled);
        assert!(off.manual_admission_blockers.iter()
            .any(|reason|reason.contains("Team is disabled")));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }


    #[test]
    fn removed_worker_remains_visible_in_native_team_policy_without_execution_authority() {
        let path=std::env::temp_dir().join(format!("sa-missing-member-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('missing-project','Fixture','/tmp/sa-missing')",[],
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'missing-project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=super::super::create_configuration(&store,super::super::NewOrchestration {
            name:"Missing Worker Team".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        let enabled=super::super::change_enabled(
            &store,&config.id,config.policy_revision,true,
        ).unwrap();
        // Removing a Bot must not destroy a durable Team's identity, nor make
        // its effective-policy inspector fail before the user can diagnose it.
        store.db.execute("DELETE FROM bots WHERE id='worker'",[]).unwrap();
        let report=effective_team(&store,&config.id,&HashMap::new()).unwrap();
        assert!(report.enabled);
        assert_eq!(report.policy_revision,enabled.policy_revision);
        assert_eq!(report.members.len(),2);
        assert!(report.members[0].present);
        assert!(!report.members[1].present);
        assert_eq!(report.members[1].bot_id,"worker");
        assert_eq!(report.members[1].role,"worker");
        assert!(report.members[1].configuration_issues.iter()
            .any(|reason|reason.contains("no longer exists")));
        assert!(report.manual_admission_blockers.iter()
            .any(|reason|reason.contains("worker bot no longer exists")));
        assert!(!report.grants_issued);
        assert!(bots::get(&store,"worker").is_err());
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|row|row.get::<_,i64>(0)).unwrap(),0);
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        let persisted=effective_team(&reopened,&config.id,&HashMap::new()).unwrap();
        assert_eq!(persisted.members[1].bot_id,"worker");
        assert!(!persisted.members[1].present);
        assert!(persisted.manual_admission_blockers.iter()
            .any(|reason|reason.contains("worker bot no longer exists")));
    }


    #[test]
    fn native_roster_supports_sixteen_workers_and_eight_reviewers_without_execution() {
        let path=std::env::temp_dir().join(format!("sa-role-capacity-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Capacity','/tmp/sa-capacity')",[],
        ).unwrap();
        for id in std::iter::once("lead".to_owned())
            .chain((0..17).map(|n|format!("worker-{n}")))
            .chain((0..9).map(|n|format!("reviewer-{n}"))) {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=super::super::create_configuration(&store,NewOrchestration {
            name:"Capacity".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker-0".into(),
        }).unwrap();
        let allowed=RosterInput {
            worker_bot_ids:(0..16).map(|n|format!("worker-{n}")).collect(),
            reviewer_bot_ids:(0..8).map(|n|format!("reviewer-{n}")).collect(),
        };
        let current=set(&store,&cfg.id,cfg.policy_revision,&allowed).unwrap();
        assert_eq!(current.worker_bot_ids.len(),16);
        assert_eq!(current.reviewer_bot_ids.len(),8);
        assert!(!current.execution_ready);
        assert!(super::super::change_enabled(&store,&cfg.id,current.policy_revision,true).is_err());
        let mut extra_worker=allowed.clone();
        extra_worker.worker_bot_ids.push("worker-16".into());
        assert!(set(&store,&cfg.id,current.policy_revision,&extra_worker).is_err());
        let mut extra_reviewer=allowed.clone();
        extra_reviewer.reviewer_bot_ids.push("reviewer-8".into());
        assert!(set(&store,&cfg.id,current.policy_revision,&extra_reviewer).is_err());
        assert_eq!(read(&store,&cfg.id).unwrap().policy_revision,current.policy_revision);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|row|row.get::<_,i64>(0),
        ).unwrap(),0);
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        let persisted=read(&reopened,&cfg.id).unwrap();
        assert_eq!(persisted.worker_bot_ids,allowed.worker_bot_ids);
        assert_eq!(persisted.reviewer_bot_ids,allowed.reviewer_bot_ids);
        assert!(!persisted.execution_ready);
    }
    #[test]
    fn roster_is_persistent_revision_guarded_role_scoped_and_execution_stays_disabled() {
        let path=std::env::temp_dir().join(format!("sa-roster-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('test-project','Fixture','/tmp/sa-roster')",[],
        ).unwrap();
        for id in ["coordinator","worker-a","worker-b","worker-c","reviewer-a","reviewer-b"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'test-project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=super::super::create_configuration(&store,super::super::NewOrchestration {
            name:"Test Team".into(),coordinator_bot_id:"coordinator".into(),
            worker_bot_id:"worker-a".into(),
        }).unwrap();
        let input=RosterInput{worker_bot_ids:vec!["worker-a".into(),"worker-b".into(),"worker-c".into()],
            reviewer_bot_ids:vec!["reviewer-a".into(),"reviewer-b".into()]};
        let saved=set(&store,&config.id,config.policy_revision,&input).unwrap();
        assert_eq!(saved.worker_bot_ids,input.worker_bot_ids);
        assert_eq!(saved.reviewer_bot_ids,input.reviewer_bot_ids);
        assert!(!saved.execution_ready);
        assert!(super::super::change_enabled(&store,&config.id,saved.policy_revision,true).is_err());
        assert!(set(&store,&config.id,config.policy_revision,&input).is_err());
        let duplicate=RosterInput{worker_bot_ids:vec!["worker-a".into(),"worker-b".into()],
            reviewer_bot_ids:vec!["worker-b".into()]};
        assert!(set(&store,&config.id,saved.policy_revision,&duplicate).is_err());
        assert_eq!(read(&store,&config.id).unwrap().policy_revision,saved.policy_revision);
        assert_eq!(set(&store,&config.id,saved.policy_revision,&input)
            .unwrap().policy_revision,saved.policy_revision);
        // Reordering any Worker updates only the future legacy first-member projection.
        let reordered=RosterInput{
            worker_bot_ids:vec!["worker-b".into(),"worker-a".into(),"worker-c".into()],
            reviewer_bot_ids:vec!["reviewer-a".into(),"reviewer-b".into()],
        };
        let promoted=set(&store,&config.id,saved.policy_revision,&reordered).unwrap();
        assert_eq!(promoted.worker_bot_ids,reordered.worker_bot_ids);
        assert_eq!(configuration(&store,&config.id).unwrap().worker_bot_id,"worker-b");
        assert!(!promoted.execution_ready);
        assert!(set(&store,&config.id,saved.policy_revision,&input).is_err());
        let clear=RosterInput{worker_bot_ids:vec!["worker-b".into()],reviewer_bot_ids:vec![]};
        let reset=set(&store,&config.id,promoted.policy_revision,&clear).unwrap();
        assert!(reset.execution_ready);
        assert_eq!(reset.worker_bot_ids,vec!["worker-b"]);
        assert_eq!(configuration(&store,&config.id).unwrap().worker_bot_id,"worker-b");
        assert!(super::super::change_enabled(&store,&config.id,reset.policy_revision,true)
            .unwrap().enabled);
    }
    #[test]
    fn settled_ticket_history_allows_versioned_team_edits_but_live_work_and_effects_block() {
        let path=std::env::temp_dir().join(format!("sa-roster-history-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('history-project','Fixture','/tmp/sa-roster-history')",[],
        ).unwrap();
        for id in ["history-coordinator","history-worker","history-worker-b"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'history-project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=super::super::create_configuration(&store,super::super::NewOrchestration {
            name:"Reusable Team".into(),coordinator_bot_id:"history-coordinator".into(),
            worker_bot_id:"history-worker".into(),
        }).unwrap();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_tickets(
                id,orchestration_id,request_key,objective,actor,source,project_id,
                worker_bot_id,policy_revision,state,created_at,updated_at
             ) VALUES(
                'prior-demand',?1,'prior-request','Original immutable intent','local-user',
                'manual','history-project','history-worker',?2,'completed',?3,?3
             )",
            params![config.id,config.policy_revision,now],
        ).unwrap();
        let expanded=RosterInput {
            worker_bot_ids:vec!["history-worker".into(),"history-worker-b".into()],
            reviewer_bot_ids:vec![],
        };
        // Completed Demand history is retained under its original admission
        // policy even when the reusable Team advances to a new revision.
        let revision=set(&store,&config.id,config.policy_revision,&expanded).unwrap();
        assert_eq!(revision.policy_revision,config.policy_revision+1);
        assert!(!revision.execution_ready);
        let original:(i64,String,String)=store.db.query_row(
            "SELECT policy_revision,worker_bot_id,objective FROM sa_tickets WHERE id='prior-demand'",
            [],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).unwrap();
        assert_eq!(original,(config.policy_revision,"history-worker".into(),
            "Original immutable intent".into()));
        let reset=RosterInput{worker_bot_ids:vec!["history-worker".into()],reviewer_bot_ids:vec![]};

        // A nonterminal review gate must not be invalidated by a roster edit.
        store.db.execute("UPDATE sa_tickets SET state='waiting_review' WHERE id='prior-demand'",[])
            .unwrap();
        assert!(set(&store,&config.id,revision.policy_revision,&reset).unwrap_err()
            .contains("Active Tickets"));
        assert_eq!(read(&store,&config.id).unwrap().policy_revision,revision.policy_revision);
        store.db.execute("UPDATE sa_tickets SET state='completed' WHERE id='prior-demand'",[])
            .unwrap();

        // A terminal Ticket may still own an uncertain provider effect.
        store.db.execute(
            "INSERT INTO sa_effect_receipts(
                ticket_id,effect_key,payload_sha256,state,created_at,updated_at
             ) VALUES('prior-demand','external-reply','fixture-digest','uncertain',?1,?1)",
            [now],
        ).unwrap();
        assert!(set(&store,&config.id,revision.policy_revision,&reset).unwrap_err()
            .contains("unresolved effects"));
        assert_eq!(read(&store,&config.id).unwrap().worker_bot_ids,expanded.worker_bot_ids);
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied' WHERE ticket_id='prior-demand'",[],
        ).unwrap();
        let restored=set(&store,&config.id,revision.policy_revision,&reset).unwrap();
        assert_eq!(restored.policy_revision,revision.policy_revision+1);
        assert!(restored.execution_ready);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",[&config.id],
            |r|r.get::<_,i64>(0),
        ).unwrap(),1);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(read(&reopened,&config.id).unwrap().policy_revision,restored.policy_revision);
        let original_after:(i64,String)=reopened.db.query_row(
            "SELECT policy_revision,state FROM sa_tickets WHERE id='prior-demand'",[],
            |r|Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(original_after,(config.policy_revision,"completed".into()));
    }


    #[test]
    fn multirole_creation_is_atomic_disabled_and_persistent() {
        let path=std::env::temp_dir().join(format!("sa-new-team-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute("INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-team-p')",[]).unwrap();
        store.db.execute("INSERT INTO projects(id,name,path) VALUES('other','Other','/tmp/sa-other')",[]).unwrap();
        for id in ["lead","primary","w2","w3","r1","r2","foreign"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,?2,'codex','default','plan',?3,?3)",
                params![id,if id=="foreign" {"other"} else {"p"},storage::now()],
            ).unwrap();
        }
        let input=||NewOrchestration {
            name:"Atomic Team".into(),coordinator_bot_id:"lead".into(),worker_bot_id:"primary".into(),
        };
        let roles=RosterInput {
            worker_bot_ids:vec!["primary".into(),"w3".into(),"w2".into()],
            reviewer_bot_ids:vec!["r2".into(),"r1".into()],
        };
        assert!(create_with_roster(&store,input(),RosterInput {
            worker_bot_ids:vec!["primary".into(),"foreign".into()],reviewer_bot_ids:vec![],
        }).unwrap_err().contains("Team project"));
        assert!(create_with_roster(&store,input(),RosterInput {
            worker_bot_ids:vec!["primary".into(),"w2".into()],reviewer_bot_ids:vec!["w2".into()],
        }).unwrap_err().contains("multiple Team roles"));
        store.db.execute("UPDATE bots SET paused=1 WHERE id='r1'",[]).unwrap();
        assert!(create_with_roster(&store,input(),roles.clone()).unwrap_err().contains("Paused bots"));
        store.db.execute("UPDATE bots SET paused=0 WHERE id='r1'",[]).unwrap();
        store.db.execute("UPDATE bots SET provider='not-a-real-harness' WHERE id='lead'",[]).unwrap();
        assert!(create_with_roster(&store,input(),roles.clone()).unwrap_err().contains("harness"));
        store.db.execute("UPDATE bots SET provider='codex' WHERE id='lead'",[]).unwrap();
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations",[],|r|r.get::<_,i64>(0),
        ).unwrap(),0,"all failed role preflights must roll back the Team");
        let cfg=create_with_roster(&store,input(),roles.clone()).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.policy_revision,1);
        let saved=read(&store,&cfg.id).unwrap();
        assert_eq!(saved.worker_bot_ids,roles.worker_bot_ids);
        assert_eq!(saved.reviewer_bot_ids,roles.reviewer_bot_ids);
        assert!(!saved.execution_ready);
        assert!(super::super::change_enabled(&store,&cfg.id,cfg.policy_revision,true)
            .unwrap_err().contains("multi-member"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0),
        ).unwrap(),0);
        let reopened=Store::open_read_only(&path).unwrap();
        let persisted=read(&reopened,&cfg.id).unwrap();
        assert_eq!(persisted.worker_bot_ids,roles.worker_bot_ids);
        assert_eq!(persisted.reviewer_bot_ids,roles.reviewer_bot_ids);
        let legacy=super::super::create_configuration(&store,NewOrchestration {
            name:"Legacy".into(),coordinator_bot_id:"lead".into(),worker_bot_id:"primary".into(),
        }).unwrap();
        assert!(read(&store,&legacy.id).unwrap().execution_ready);
    }
    #[test]
    fn configured_attempt_budget_is_atomic_disabled_and_survives_restart() {
        let path=std::env::temp_dir().join(format!("sa-new-budget-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-budget')",[]
        ).unwrap();
        for id in ["lead","primary","second"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let input=||NewOrchestration {
            name:"Budget Team".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"primary".into(),
        };
        let roles=RosterInput {
            worker_bot_ids:vec!["primary".into(),"second".into()],
            reviewer_bot_ids:vec![],
        };
        for limit in [0,5] {
            assert!(create_with_attempt_policy(&store,input(),Some(roles.clone()),limit)
                .unwrap_err().contains("between 1 and 4"));
        }
        assert!(create_with_attempt_policy(&store,input(),Some(RosterInput {
            worker_bot_ids:vec!["primary".into(),"lead".into()],
            reviewer_bot_ids:vec![],
        }),2).unwrap_err().contains("multiple Team roles"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations",[],|r|r.get::<_,i64>(0),
        ).unwrap(),0,"invalid policies or roles must never leave a Team");
        let multi=create_with_attempt_policy(&store,input(),Some(roles),3).unwrap();
        assert!(!multi.enabled);
        assert_eq!(multi.policy_revision,1);
        assert_eq!(read(&store,&multi.id).unwrap().max_worker_attempts,3);
        assert!(!read(&store,&multi.id).unwrap().execution_ready);
        let single=create_with_attempt_policy(&store,NewOrchestration {
            name:"Single Attempt Team".into(),..input()
        },None,1).unwrap();
        assert!(!single.enabled);
        assert_eq!(read(&store,&single.id).unwrap().max_worker_attempts,1);
        let legacy=super::super::create_configuration(&store,NewOrchestration {
            name:"Legacy Attempt Team".into(),..input()
        }).unwrap();
        assert_eq!(read(&store,&legacy.id).unwrap().max_worker_attempts,2);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(read(&reopened,&multi.id).unwrap().max_worker_attempts,3);
        assert_eq!(read(&reopened,&single.id).unwrap().max_worker_attempts,1);
        assert_eq!(read(&reopened,&legacy.id).unwrap().max_worker_attempts,2);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0),
        ).unwrap(),0,"configuration must never infer work");
    }
    #[test]
    fn team_creation_pins_strategy_atomically_without_execution() {
        let path=std::env::temp_dir().join(format!("sa-strategy-create-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-strategy-create')",[]
        ).unwrap();
        for id in ["lead","worker","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let new_team=||NewOrchestration {
            name:"Strategy Team".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker".into(),
        };
        let roles=||RosterInput {
            worker_bot_ids:vec!["worker".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        };
        assert!(create_with_attempt_policy_and_strategy(
            &store,new_team(),Some(roles()),3,"Safe","unauthorized"
        ).unwrap_err().contains("Unsupported Team optimization"));
        assert!(create_with_attempt_policy_and_strategy(
            &store,new_team(),Some(roles()),3,"invalid\0purpose","performance"
        ).unwrap_err().contains("unsafe controls"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations",[],|row|row.get::<_,i64>(0)
        ).unwrap(),0,"invalid strategy must never leave a partial Team");
        let created=create_with_attempt_policy_and_strategy(
            &store,new_team(),Some(roles()),3,
            "Verify native execution before acceptance","security_critical",
        ).unwrap();
        assert!(!created.enabled);
        assert_eq!(created.policy_revision,1);
        let saved=read(&store,&created.id).unwrap();
        assert_eq!(saved.purpose,"Verify native execution before acceptance");
        assert_eq!(saved.strategy,"security_critical");
        assert_eq!(saved.max_worker_attempts,3);
        assert_eq!(saved.reviewer_bot_ids,vec!["reviewer".to_string()]);
        let reopened=Store::open_read_only(&path).unwrap();
        let after_restart=read(&reopened,&created.id).unwrap();
        assert_eq!(after_restart.purpose,saved.purpose);
        assert_eq!(after_restart.strategy,saved.strategy);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|row|row.get::<_,i64>(0)
        ).unwrap(),0,"Team creation must not infer an authorized Demand");
    }

    #[test]
    fn duplicate_team_is_atomic_revision_guarded_and_never_copies_runtime_authority() {
        let path=std::env::temp_dir().join(format!("sa-clone-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-team-clone')",[]
        ).unwrap();
        for id in ["lead","primary","second","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let source=create_with_roster(&store,NewOrchestration {
            name:"Source Team".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"primary".into(),
        },RosterInput {
            worker_bot_ids:vec!["primary".into(),"second".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        }).unwrap();
        let count=||store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations",[],|row|row.get::<_,i64>(0),
        ).unwrap();
        assert!(duplicate_with_roster(&store,&source.id,source.policy_revision+1,
            "Stale copy".into()).unwrap_err().contains("revision changed"));
        assert_eq!(count(),1);
        store.db.execute("UPDATE bots SET paused=1 WHERE id='reviewer'",[]).unwrap();
        assert!(duplicate_with_roster(&store,&source.id,source.policy_revision,
            "Unavailable copy".into()).unwrap_err().contains("Paused bots"));
        assert_eq!(count(),1,"bot ineligibility must roll back the new Team");
        store.db.execute("UPDATE bots SET paused=0 WHERE id='reviewer'",[]).unwrap();
        let cloned=duplicate_with_roster(&store,&source.id,source.policy_revision,
            "Source Team copy".into()).unwrap();
        assert_eq!(count(),2);
        assert!(!cloned.enabled);
        assert_eq!(cloned.policy_revision,1);
        assert_ne!(cloned.id,source.id);
        assert_eq!(cloned.coordinator_bot_id,source.coordinator_bot_id);
        let roles=read(&store,&cloned.id).unwrap();
        assert_eq!(roles.worker_bot_ids,vec!["primary","second"]);
        assert_eq!(roles.reviewer_bot_ids,vec!["reviewer"]);
        assert!(!roles.execution_ready);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets WHERE orchestration_id=?1",
            [&cloned.id],|row|row.get::<_,i64>(0),
        ).unwrap(),0);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_event_sources WHERE orchestration_id=?1",
            [&cloned.id],|row|row.get::<_,i64>(0),
        ).unwrap(),0);
        store.db.execute(
            "UPDATE sa_team_roles SET position=9 WHERE orchestration_id=?1 AND role='worker'",
            [&source.id],
        ).unwrap();
        assert!(duplicate_with_roster(&store,&source.id,source.policy_revision,
            "Malformed copy".into()).unwrap_err().contains("Malformed Team role metadata"));
        assert_eq!(count(),2);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(read(&reopened,&cloned.id).unwrap().reviewer_bot_ids,
            vec!["reviewer"]);
    }


    #[test]
    fn native_ticket_snapshot_preserves_admitted_policy_after_team_changes_and_restart() {
        let path=std::env::temp_dir().join(format!("sa-admitted-policy-{}.sqlite3",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-policy-evidence')",[]
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','original-model','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let team=super::super::create_configuration(&store,NewOrchestration {
            name:"Policy history".into(),
            coordinator_bot_id:"lead".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        let enabled=super::super::change_enabled(
            &store,&team.id,team.policy_revision,true,
        ).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&team.id,"Capture original Team configuration","policy-evidence",false,
        ).unwrap();
        assert!(started);
        let original=super::super::ticket_snapshot(&store,&ticket.id,50).unwrap();
        assert_eq!(original.current_team_policy_revision,enabled.policy_revision);
        assert!(!original.team_policy_changed_since_admission);
        let admitted=original.admitted_profile.unwrap();
        assert_eq!(admitted.policy_revision,enabled.policy_revision);
        assert_eq!(admitted.members.len(),2);
        assert_eq!(admitted.members[1].model,"original-model");

        // Complete the disposable fixture's native records before changing
        // the reusable Team; never mutate the Ticket's admitted Mandate.
        store.db.execute(
            "UPDATE sa_assignments SET state='finished' WHERE ticket_id=?1",
            [&ticket.id],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_tickets SET state='completed',active_assignment_id=NULL WHERE id=?1",
            [&ticket.id],
        ).unwrap();
        let off=super::super::change_enabled(
            &store,&team.id,enabled.policy_revision,false,
        ).unwrap();
        let updated=set_worker_attempt_limit(&store,&team.id,off.policy_revision,3).unwrap();
        assert!(updated.policy_revision>admitted.policy_revision);
        store.db.execute("UPDATE bots SET model='new-model' WHERE id='worker'",[])
            .unwrap();

        let snapshot=super::super::ticket_snapshot(&store,&ticket.id,50).unwrap();
        assert_eq!(snapshot.current_team_policy_revision,updated.policy_revision);
        assert!(snapshot.team_policy_changed_since_admission);
        let frozen=snapshot.admitted_profile.unwrap();
        assert_eq!(frozen.policy_revision,admitted.policy_revision);
        assert_eq!(frozen.members[1].model,"original-model");
        assert_eq!(frozen.members[1].bot_id,"worker");
        assert!(!snapshot.mandate.allowed_side_effects);
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        let snapshot=super::super::ticket_snapshot(&reopened,&ticket.id,50).unwrap();
        assert!(snapshot.team_policy_changed_since_admission);
        assert_eq!(snapshot.current_team_policy_revision,updated.policy_revision);
        assert_eq!(snapshot.admitted_profile.unwrap().members[1].model,"original-model");
    }

    #[test]
    fn malformed_persisted_roster_positions_and_primary_aliases_fail_closed() {
        let path=std::env::temp_dir().join(format!("sa-roster-integrity-{}.sqlite3",storage::id()));
        let store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Team','/tmp/sa-roster-integrity')",[]
        ).unwrap();
        for id in ["lead","primary","additional","reviewer"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg=super::super::create_configuration(&store,NewOrchestration {
            name:"Validated roles".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"primary".into(),
        }).unwrap();
        let valid=RosterInput {
            worker_bot_ids:vec!["primary".into(),"additional".into()],
            reviewer_bot_ids:vec!["reviewer".into()],
        };
        set(&store,&cfg.id,cfg.policy_revision,&valid).unwrap();
        assert_eq!(read(&store,&cfg.id).unwrap().worker_bot_ids,valid.worker_bot_ids);

        store.db.execute(
            "UPDATE sa_team_roles SET position=4 WHERE orchestration_id=?1 AND role='worker'",
            [&cfg.id],
        ).unwrap();
        assert!(read(&store,&cfg.id).unwrap_err().contains("Malformed Team role metadata"));
        store.db.execute(
            "UPDATE sa_team_roles SET position=1 WHERE orchestration_id=?1 AND role='worker'",
            [&cfg.id],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_team_roles SET position=3 WHERE orchestration_id=?1 AND role='reviewer'",
            [&cfg.id],
        ).unwrap();
        assert!(read(&store,&cfg.id).unwrap_err().contains("Malformed Team role metadata"));
        store.db.execute(
            "UPDATE sa_team_roles SET position=0,bot_id='primary'
             WHERE orchestration_id=?1 AND role='reviewer'",
            [&cfg.id],
        ).unwrap();
        assert!(read(&store,&cfg.id).unwrap_err().contains("Malformed Team role identity"));
        store.db.execute(
            "UPDATE sa_team_roles SET bot_id='reviewer'
             WHERE orchestration_id=?1 AND role='reviewer'",
            [&cfg.id],
        ).unwrap();
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(read(&reopened,&cfg.id).unwrap().reviewer_bot_ids,valid.reviewer_bot_ids);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|row|row.get::<_,i64>(0)).unwrap(),0);
    }

    #[test]
    fn team_strategy_is_versioned_portable_and_pinned_per_demand_after_restart() {
        let path=std::env::temp_dir().join(format!("sa-strategy-{}.sqlite3",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Strategy','/tmp/sa-strategy')",[]
        ).unwrap();
        for id in ["lead","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let original=super::super::create_configuration(&store,NewOrchestration {
            name:"Scenario Team".into(),coordinator_bot_id:"lead".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        assert_eq!(read(&store,&original.id).unwrap().strategy,"balanced");
        assert!(set_team_strategy(&store,&original.id,original.policy_revision,
            "Scope","unrestricted").is_err());
        assert!(set_team_strategy(&store,&original.id,original.policy_revision,
            &"x".repeat(401),"performance").is_err());
        assert_eq!(configuration(&store,&original.id).unwrap().policy_revision,1);
        let configured=set_team_strategy(&store,&original.id,original.policy_revision,
            "Build secure integrations","performance").unwrap();
        assert_eq!(configured.policy_revision,2);
        assert_eq!(configured.purpose,"Build secure integrations");
        assert_eq!(set_team_strategy(&store,&original.id,2,
            "Build secure integrations","performance").unwrap().policy_revision,2);
        assert!(set_team_strategy(&store,&original.id,1,
            "Stale mutation","economy").is_err());
        let active=super::super::change_enabled(&store,&original.id,2,true).unwrap();
        let (ticket,started)=super::super::admit(
            &mut store,&original.id,"Inspect the integration","strategy-run",false,
        ).unwrap();
        assert!(started);
        let admitted=super::super::ticket_snapshot(&store,&ticket.id,20).unwrap()
            .admitted_strategy.unwrap();
        assert_eq!(admitted.policy_revision,active.policy_revision);
        assert_eq!(admitted.strategy,"performance");
        assert_eq!(admitted.purpose,"Build secure integrations");
        assert!(set_team_strategy(&store,&original.id,active.policy_revision,
            "Changed","economy").is_err());
        super::super::stop(&store,&ticket.id).unwrap();
        let off=super::super::change_enabled(
            &store,&original.id,active.policy_revision,false,
        ).unwrap();
        let revised=set_team_strategy(&store,&original.id,off.policy_revision,
            "Economize future runs","economy").unwrap();
        assert!(revised.policy_revision>admitted.policy_revision);
        assert_eq!(super::super::ticket_snapshot(&store,&ticket.id,20).unwrap()
            .admitted_strategy.unwrap(),admitted,
            "Settled Ticket must keep the original strategy snapshot");
        let cloned=duplicate_with_roster(&store,&original.id,revised.policy_revision,
            "Scenario Team copy".into()).unwrap();
        assert!(!cloned.enabled);
        assert_eq!(read(&store,&cloned.id).unwrap().strategy,"economy");
        assert_eq!(read(&store,&cloned.id).unwrap().purpose,"Economize future runs");
        assert!(super::super::ticket_snapshot(&store,&ticket.id,20).unwrap()
            .admitted_strategy.is_some());
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        assert_eq!(ticket_strategy(&reopened,&ticket.id).unwrap(),Some(admitted));
        assert_eq!(read(&reopened,&original.id).unwrap().strategy,"economy");
        assert_eq!(read(&reopened,&cloned.id).unwrap().strategy,"economy");
    }

}
