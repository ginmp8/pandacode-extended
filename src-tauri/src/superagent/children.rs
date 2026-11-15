//! Read-only, source-backed view of provider-observed subagents.
//!
//! The app already persists real child/parent events per provider run. Never
//! infer that an observed provider child has acquired the Ticket's native
//! grant: it is evidence only, not a new Worker seat, Council reviewer, or
//! separately writable native Session.

use super::{require_main_window, ticket_team_snapshot};
use crate::{subagent_state::SubagentRow, AppState, Result};
use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tauri::State;

const MAX_NODES: usize = 256;
const MAX_ANCESTRY_HOPS: usize = 32;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct ObservedChildNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub root_assignment_id: String,
    pub root_session_id: String,
    pub root_worker_bot_id: String,
    pub name: String,
    pub reported_status: String,
    pub depth: usize,
    pub missing_parent: bool,
    pub ancestry_cycle_or_overflow: bool,
    pub tool_count: i64,
    pub started_at: i64,
    pub updated_at: i64,
    pub independently_verified: bool,
    pub native_child_grant_present: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct ObservedChildTree {
    pub ticket_id: String,
    pub team_version: i64,
    pub configured_workers: usize,
    pub council_reviewers: usize,
    pub observed_children: usize,
    pub reported_running: usize,
    pub reported_waiting: usize,
    pub reported_nested: usize,
    pub has_unresolved_lineage: bool,
    pub truncated: bool,
    pub can_spawn_native_children: bool,
    pub provider_status_is_not_verification: bool,
    pub nodes: Vec<ObservedChildNode>,
}

fn ancestry(id:&str,rows:&HashMap<String,SubagentRow>) -> (usize,bool,bool) {
    let mut depth=1;
    let mut missing=false;
    let mut cycle=false;
    let mut seen=HashSet::from([id]);
    let mut parent=rows.get(id).and_then(|row|row.parent_id.as_deref());
    while let Some(parent_id)=parent {
        if !seen.insert(parent_id) || depth>=MAX_ANCESTRY_HOPS {
            cycle=true;
            break;
        }
        depth+=1;
        let Some(record)=rows.get(parent_id) else {
            missing=true;
            break;
        };
        parent=record.parent_id.as_deref();
    }
    (depth,missing,cycle)
}

pub(super) fn map_rows(
    ticket_id:&str,team_version:i64,worker_count:usize,reviewer_count:usize,
    rooted:Vec<(String,String,String,Vec<SubagentRow>)>,
) -> ObservedChildTree {
    let mut nodes=Vec::new();
    let mut truncated=false;
    for (assignment_id,session_id,bot_id,rows) in rooted {
        let map:HashMap<String,SubagentRow>=rows.into_iter()
            .map(|row|(row.id.clone(),row)).collect();
        let mut sorted=map.values().collect::<Vec<_>>();
        sorted.sort_by(|a,b| a.started_at.cmp(&b.started_at)
            .then_with(||a.id.cmp(&b.id)));
        for row in sorted {
            if nodes.len()>=MAX_NODES {truncated=true;break;}
            let (depth,missing,cycle)=ancestry(&row.id,&map);
            nodes.push(ObservedChildNode {
                id:row.id.clone(),
                parent_id:row.parent_id.clone(),
                root_assignment_id:assignment_id.clone(),
                root_session_id:session_id.clone(),
                root_worker_bot_id:bot_id.clone(),
                name:row.name.chars().take(160).collect(),
                reported_status:row.status.clone(),
                depth,missing_parent:missing,
                ancestry_cycle_or_overflow:cycle,
                tool_count:row.tool_count,
                started_at:row.started_at,
                updated_at:row.updated_at,
                independently_verified:false,
                native_child_grant_present:false,
            });
        }
        if truncated {break;}
    }
    let reported_running=nodes.iter().filter(|node|node.reported_status=="running").count();
    let reported_waiting=nodes.iter().filter(|node|node.reported_status=="waiting").count();
    let reported_nested=nodes.iter().filter(|node|node.depth>1).count();
    let has_unresolved_lineage=nodes.iter()
        .any(|node|node.missing_parent||node.ancestry_cycle_or_overflow);
    ObservedChildTree {
        ticket_id:ticket_id.into(),team_version,configured_workers:worker_count,
        council_reviewers:reviewer_count,observed_children:nodes.len(),
        reported_running,reported_waiting,reported_nested,
        has_unresolved_lineage,truncated,can_spawn_native_children:false,
        provider_status_is_not_verification:true,nodes,
    }
}

/// Only the existing native assignment's real provider session may be used
/// to read a child stream. Never mix unrelated bot histories or Teams.
pub(crate) fn inspect(state:&AppState,ticket_id:&str)->Result<ObservedChildTree> {
    let (team_version,worker_count,reviewer_count,assignments)={
        let store=state.store.lock().map_err(|error|error.to_string())?;
        let ticket=super::get_ticket(&store,ticket_id)?;
        let snapshot=ticket_team_snapshot(&store,ticket_id)?;
        if ticket.orchestration_id!=snapshot.orchestration_id {
            return Err("Ticket Team identity changed before child inspection.".into());
        }
        let mut stmt=store.db.prepare(
            "SELECT a.id,a.session_id,a.bot_id FROM sa_assignments a
             JOIN sa_tickets t ON t.id=a.ticket_id
             WHERE a.ticket_id=?1 AND a.session_id IS NOT NULL
             ORDER BY a.attempt,a.created_at,a.id",
        ).map_err(|error|error.to_string())?;
        let rows=stmt.query_map(params![ticket_id],|row|Ok((
            row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,
        ))).map_err(|error|error.to_string())?;
        let sessions=rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error|error.to_string())?;
        (snapshot.team_version,snapshot.worker_bot_ids.len(),
         snapshot.reviewer_bot_ids.len(),sessions)
    };
    let mut rooted=Vec::new();
    for (assignment_id,session_id,bot_id) in assignments {
        let rows=crate::task_subagents(state,&session_id)?;
        rooted.push((assignment_id,session_id,bot_id,rows));
    }
    Ok(map_rows(ticket_id,team_version,worker_count,reviewer_count,rooted))
}

#[tauri::command]
pub async fn superagent_observed_child_tree(
    window:tauri::WebviewWindow,state:State<'_,AppState>,ticket_id:String,
)->Result<ObservedChildTree> {
    require_main_window(&window)?;
    inspect(&state,&ticket_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(id:&str,parent:Option<&str>,status:&str)->SubagentRow {
        SubagentRow {
            id:id.into(),parent_id:parent.map(str::to_owned),name:id.into(),
            prompt:"secret child prompt".into(),response:"private output".into(),
            status:status.into(),tool_count:1,started_at:1,updated_at:2,
        }
    }
    #[test]
    fn observes_real_child_ancestry_without_inventing_native_authority() {
        let tree=map_rows("ticket",2,3,2,vec![
            ("assignment".into(),"root-session".into(),"worker".into(),vec![
                row("child-one",None,"running"),
                row("child-two",Some("child-one"),"waiting"),
                row("orphan",Some("missing"),"completed"),
            ]),
        ]);
        assert_eq!(tree.configured_workers,3);
        assert_eq!(tree.observed_children,3);
        assert_eq!(tree.reported_nested,2);
        assert_eq!(tree.reported_running,1);
        assert_eq!(tree.reported_waiting,1);
        assert!(tree.has_unresolved_lineage);
        assert!(!tree.can_spawn_native_children);
        assert!(tree.nodes.iter().all(|node| !node.independently_verified
            && !node.native_child_grant_present));
    }
    #[test]
    fn cyclic_lineage_is_visible_and_never_a_completion_grant() {
        let tree=map_rows("ticket",1,1,0,vec![
            ("assignment".into(),"session".into(),"worker".into(),vec![
                row("a",Some("b"),"completed"),row("b",Some("a"),"completed"),
            ]),
        ]);
        assert!(tree.has_unresolved_lineage);
        assert!(tree.nodes.iter().all(|node|node.ancestry_cycle_or_overflow));
        assert!(tree.provider_status_is_not_verification);
    }
}
