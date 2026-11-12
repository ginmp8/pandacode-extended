//! Persistent configured Team member roles, separate from runtime assignments.
//! Additional Workers/Reviewers do not become authorized providers or Council seats.
//! The current native runner remains single-worker until scoped role admission exists.
use super::{configuration,require_main_window};
use crate::{bots,storage::{self,Store},AppState,Result};
use rusqlite::params;
use serde::{Deserialize,Serialize};
use std::collections::HashSet;
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
pub(super) fn has_additional_members(store:&Store,id:&str)->Result<bool> {
    let count:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_team_roles WHERE orchestration_id=?1",[id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(count>0)
}
fn read(store:&Store,id:&str)->Result<TeamRoster> {
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
    for row in rows {
        let (role,position,bot_id)=row.map_err(|e|e.to_string())?;
        match role.as_str() {
            "worker" if position>0=>workers.push(bot_id),
            "reviewer"=>reviewers.push(bot_id),
            _=>return Err("Malformed Team role metadata; no authority granted.".into()),
        }
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
fn set(store:&Store,id:&str,expected:i64,input:&RosterInput)->Result<TeamRoster> {
    let cfg=configuration(store,id)?;
    if cfg.enabled || cfg.policy_revision!=expected {
        return Err("Disable the Team and reload its policy revision before changing members.".into());
    }
    let used:bool=store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets WHERE orchestration_id=?1)",[id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    if used {return Err("Team Ticket history prevents roster reconfiguration.".into());}
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
        // Worker execution is bound to one project, even when not yet admitted.
        if input.worker_bot_ids.contains(bot_id) && bot.project_id!=cfg.project_id {
            return Err("All configured Workers must belong to the Team project.".into());
        }
    }
    let previous=read(store,id)?;
    if previous.worker_bot_ids==input.worker_bot_ids
       && previous.reviewer_bot_ids==input.reviewer_bot_ids {return Ok(previous);}
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let changed=tx.execute(
        "UPDATE sa_orchestrations SET policy_revision=policy_revision+1,updated_at=?3
         WHERE id=?1 AND enabled=0 AND policy_revision=?2
           AND NOT EXISTS(SELECT 1 FROM sa_tickets WHERE orchestration_id=?1)",
        params![id,expected,storage::now()],
    ).map_err(|e|e.to_string())?;
    if changed!=1{return Err("Concurrent Team policy change; reload required.".into());}
    tx.execute("DELETE FROM sa_team_roles WHERE orchestration_id=?1",[id])
        .map_err(|e|e.to_string())?;
    for (idx,bot_id) in input.worker_bot_ids.iter().enumerate().skip(1) {
        tx.execute("INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
                    VALUES(?1,'worker',?2,?3)",params![id,idx as i64,bot_id])
            .map_err(|e|e.to_string())?;
    }
    for (idx,bot_id) in input.reviewer_bot_ids.iter().enumerate() {
        tx.execute("INSERT INTO sa_team_roles(orchestration_id,role,position,bot_id)
                    VALUES(?1,'reviewer',?2,?3)",params![id,idx as i64,bot_id])
            .map_err(|e|e.to_string())?;
    }
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
}
