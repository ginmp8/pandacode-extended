//! Persistent configured Team member roles, separate from runtime assignments.
//! Additional Workers/Reviewers do not become authorized providers or Council seats.
//! The current native runner remains single-worker until scoped role admission exists.
use super::{configuration,require_main_window};
use crate::{bots,storage::Store,AppState,Result};
use serde::{Deserialize,Serialize};
use std::collections::HashMap;
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
pub(super) fn has_additional_members(store: &Store, id: &str) -> Result<bool> {
    let config = configuration(store, id)?;
    Ok(config.worker_bot_ids.len() > 1 || !config.reviewer_bot_ids.is_empty())
}

/// View the one authoritative versioned Team roster. The legacy sidecar must
/// never override an admitted Ticket's immutable membership snapshot.
pub(crate) fn read(store: &Store, id: &str) -> Result<TeamRoster> {
    let config = configuration(store, id)?;
    Ok(TeamRoster {
        orchestration_id: config.id,
        policy_revision: config.policy_revision,
        coordinator_bot_id: config.coordinator_bot_id,
        execution_ready: config.worker_bot_ids.len() == 1 && config.reviewer_bot_ids.is_empty(),
        worker_bot_ids: config.worker_bot_ids,
        reviewer_bot_ids: config.reviewer_bot_ids,
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
    pub project_id:String,
    pub paused:bool,
    pub skills:Vec<EffectiveSkill>,
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
        let skills=bot.skills.iter().map(|skill_id| {
            let state=library.get(skill_id);
            EffectiveSkill {
                id:skill_id.clone(),
                library_status:if state.is_some() {"installed"} else {"missing"}.into(),
                safety_status:state.map(|s|s.safety).unwrap_or("unknown").into(),
                harness_eligibility:"not_verified".into(),
                invocation_evidence:"not_verified".into(),
            }
        }).collect();
        members.push(EffectiveMember {
            role:role.into(),position,bot_id:bot.id,name:bot.name,
            provider:bot.provider,model:bot.model,mode:bot.mode,
            project_id:bot.project_id,paused:bot.paused,skills,
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

fn set(store: &Store, id: &str, expected: i64, input: &RosterInput) -> Result<TeamRoster> {
    let current = configuration(store, id)?;
    let worker_id = input.worker_bot_ids.first()
        .ok_or("A Team needs at least one Worker.")?.clone();
    let draft = super::NewOrchestration {
        name: current.name.clone(),
        coordinator_bot_id: current.coordinator_bot_id.clone(),
        worker_bot_id: worker_id,
    };
    super::update_configuration_roles_with_policy(
        store, id, expected, &draft,
        input.worker_bot_ids.clone(), input.reviewer_bot_ids.clone(),
        current.council_required,
    )?;
    read(store, id)
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
    use rusqlite::params;
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
            "UPDATE bots SET skills='[\"known\",\"missing\"]' WHERE id='worker'",[],
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
        assert_eq!(view.members[1].skills[0].library_status,"installed");
        assert_eq!(view.members[1].skills[0].safety_status,"review_required");
        assert_eq!(view.members[1].skills[0].harness_eligibility,"not_verified");
        assert_eq!(view.members[1].skills[0].invocation_evidence,"not_verified");
        assert_eq!(view.members[1].skills[1].library_status,"missing");
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

}
