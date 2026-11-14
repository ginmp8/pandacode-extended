//! Persistent configured Team member roles, separate from runtime assignments.
//! Additional Workers/Reviewers do not become authorized providers or Council seats.
//! The current native runner remains single-worker until scoped role admission exists.
use super::{configuration,create_configuration,require_main_window,NewOrchestration,Orchestration};
use crate::{bots,storage::{self,Store},AppState,Result};
use rusqlite::params;
use serde::{Deserialize,Serialize};
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
const MAX_WORKERS:usize=8;
const MAX_REVIEWERS:usize=4;

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
    pub execution_ready:bool,
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
    pub provider:String,
    pub model:String,
    pub mode:String,
    pub reasoning_effort:String,
    pub assigned_skill_ids:Vec<String>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct AdmittedTeamProfile {
    pub policy_revision:i64,
    pub captured_at:i64,
    pub members:Vec<AdmittedMemberProfile>,
}

fn member_profile(store:&Store,bot:&bots::Bot,role:&str,position:usize)->Result<AdmittedMemberProfile> {
    let project_path:String=store.db.query_row(
        "SELECT path FROM projects WHERE id=?1",
        [&bot.project_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    if project_path.trim().is_empty() {
        return Err("Team member checkout path is unavailable.".into());
    }
    Ok(AdmittedMemberProfile {
        role:role.into(),position,bot_id:bot.id.clone(),
        project_id:bot.project_id.clone(),project_path:Some(project_path),
        provider:bot.provider.clone(),model:bot.model.clone(),mode:bot.mode.clone(),
        reasoning_effort:bot.reasoning_effort.clone(),
        assigned_skill_ids:bot.skills.clone(),
    })
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
        profiles.push(member_profile(store,&bots::get(store,id)?,"worker",position)?);
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
            if member.bot_id.is_empty() || !identities.insert(member.bot_id.as_str()) {
                return Err("Invalid persisted task-effective Team role identity.".into());
            }
            let valid=match member.role.as_str() {
                "coordinator"=>index==0 && member.position==0,
                "worker"=>index>0 && reviewers==0 && workers<MAX_WORKERS
                    && member.position==workers
                    && (workers>0 || member.bot_id==primary_worker)
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
        if workers==0 {
            return Err("Admitted Team profile is missing its primary Worker.".into());
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
    if workers.iter().chain(reviewers.iter())
           .any(|id|id==&config.coordinator_bot_id)
        || workers.iter().skip(1).chain(reviewers.iter())
           .any(|id|id==&config.worker_bot_id) {
        return Err("Malformed Team role identity; no authority granted.".into());
    }
    let ready=workers.len()==1 && reviewers.is_empty();
    Ok(TeamRoster {
        orchestration_id:config.id,
        policy_revision:config.policy_revision,
        coordinator_bot_id:config.coordinator_bot_id,
        worker_bot_ids:workers,
        reviewer_bot_ids:reviewers,
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
    let mut add=|role:&str,position:usize,bot_id:&str|->Result<()> {
        let bot=bots::get(store,bot_id)?;
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
            }
        }
        members.push(EffectiveMember {
            role:role.into(),position,bot_id:bot.id,name:bot.name,
            provider:bot.provider,model:bot.model,mode:bot.mode,
            reasoning_effort:bot.reasoning_effort,
            project_id:bot.project_id,paused:bot.paused,skills,configuration_issues,
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
    Ok(EffectiveTeam {
        orchestration_id:config.id,policy_revision:config.policy_revision,
        enabled:config.enabled,execution_ready:roster.execution_ready,
        members,grants_issued:false,
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
    let items=crate::skills::skills_list(app).await?;
    let library:HashMap<String,LibraryStatus>=items.into_iter()
        .filter(|item|item.meta.kind==crate::skills::Kind::Skill)
        .map(|item| {
            let safety=if item.findings.is_empty() {"clean"}
                else if item.meta.acknowledged.as_deref()==Some(item.meta.sha256.as_str()) {
                    "findings_acknowledged"
                } else {"review_required"};
            (item.meta.id,LibraryStatus{safety})
        }).collect();
    let store=state.store.lock().map_err(|e|e.to_string())?;
    effective_team(&store,&orchestration_id,&library)
}

fn validate_member_ids(store:&Store,cfg:&Orchestration,input:&RosterInput)->Result<()> {
    if input.worker_bot_ids.is_empty() || input.worker_bot_ids.len()>MAX_WORKERS
       || input.reviewer_bot_ids.len()>MAX_REVIEWERS
       || input.worker_bot_ids.first()!=Some(&cfg.worker_bot_id) {
        return Err("Choose the original primary Worker, up to 8 Workers and 4 Reviewers.".into());
    }
    let mut unique=HashSet::new();
    unique.insert(cfg.coordinator_bot_id.as_str());
    for bot_id in input.worker_bot_ids.iter().chain(input.reviewer_bot_ids.iter()) {
        if !unique.insert(bot_id.as_str()) {
            return Err("A bot cannot occupy multiple Team roles.".into());
        }
        let bot=bots::get(store,bot_id)?;
        if bot.paused {return Err("Paused bots cannot join a Team.".into());}
        if input.worker_bot_ids.contains(bot_id) && bot.project_id!=cfg.project_id {
            return Err("All configured Workers must belong to the Team project.".into());
        }
    }
    Ok(())
}

fn insert_member_roles(store:&Store,id:&str,input:&RosterInput)->Result<()> {
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

fn set(store:&Store,id:&str,expected:i64,input:&RosterInput)->Result<TeamRoster> {
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
        params![id,expected,storage::now()],
    ).map_err(|e|e.to_string())?;
    if changed!=1{return Err("Concurrent Team policy change; reload required.".into());}
    tx.execute("DELETE FROM sa_team_roles WHERE orchestration_id=?1",[id])
        .map_err(|e|e.to_string())?;
    insert_member_roles(store,id,input)?;
    tx.commit().map_err(|e|e.to_string())?;
    read(store,id)
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
        assert_eq!(one.members.len(),2);
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
        assert!(!view.grants_issued);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_tickets",[],|r|r.get::<_,i64>(0)).unwrap(),0);
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
        let clear=RosterInput{worker_bot_ids:vec!["worker-a".into()],reviewer_bot_ids:vec![]};
        let reset=set(&store,&config.id,saved.policy_revision,&clear).unwrap();
        assert!(reset.execution_ready);
        assert_eq!(reset.worker_bot_ids,vec!["worker-a"]);
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

}
