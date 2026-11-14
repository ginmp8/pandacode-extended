//! Bounded, worker-requested native subagent lineage. This is an explicit
//! local-user authorization layered on the original read-only Demand; no
//! child inherits tool, shell, MCP, secret, GitHub or Council authority.

use super::{configuration,get_ticket,require_main_window,ticket_team_snapshot,
    snapshot_role_profile_matches,TeamSnapshotProvenance};
use crate::{bots,providers::{ToolHost,ToolFuture},
    storage::{self,Store},AppState,Result};
use rusqlite::{params,OptionalExtension};
use serde::Serialize;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use std::collections::HashSet;
use tauri::{Manager,State};

mod dispatch;
pub(crate) use dispatch::{advance,descendant_sessions,revoke_descendants,lineage_active,has_unsettled};

pub(crate) const TOOL_NAME:&str="pandacode_superagent_spawn_child";
const MAX_DEPTH:i64=3;
const MAX_CHILDREN:i64=4;
const MAX_TOTAL:i64=24;
const MAX_OBJECTIVE_BYTES:usize=4096;

pub(crate) const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_subagent_policies (
    root_ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    team_version INTEGER NOT NULL CHECK(team_version>0),
    policy_revision INTEGER NOT NULL CHECK(policy_revision>0),
    root_ticket_revision INTEGER NOT NULL CHECK(root_ticket_revision>0),
    max_depth INTEGER NOT NULL CHECK(max_depth BETWEEN 1 AND 3),
    max_children INTEGER NOT NULL CHECK(max_children BETWEEN 1 AND 4),
    max_total INTEGER NOT NULL CHECK(max_total BETWEEN 1 AND 24),
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS sa_subagent_nodes (
    id TEXT PRIMARY KEY,
    root_ticket_id TEXT NOT NULL REFERENCES sa_subagent_policies(root_ticket_id) ON DELETE RESTRICT,
    parent_ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    parent_assignment_id TEXT NOT NULL REFERENCES sa_assignments(id) ON DELETE RESTRICT,
    child_ticket_id TEXT UNIQUE REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    item_key TEXT NOT NULL,
    objective TEXT NOT NULL,
    worker_bot_id TEXT NOT NULL,
    depth INTEGER NOT NULL CHECK(depth BETWEEN 1 AND 3),
    request_sha256 TEXT NOT NULL CHECK(length(request_sha256)=64),
    state TEXT NOT NULL CHECK(state IN ('pending','running','waiting_review','completed','blocked','stopped')),
    reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(parent_assignment_id,item_key),
    CHECK(child_ticket_id IS NULL OR state<>'pending')
);
CREATE INDEX IF NOT EXISTS sa_subagent_nodes_parent ON sa_subagent_nodes(parent_ticket_id,state);
CREATE INDEX IF NOT EXISTS sa_subagent_nodes_root ON sa_subagent_nodes(root_ticket_id,state,depth);
CREATE INDEX IF NOT EXISTS sa_subagent_nodes_child ON sa_subagent_nodes(child_ticket_id);
CREATE TRIGGER IF NOT EXISTS sa_subagent_identity_immutable
 BEFORE UPDATE ON sa_subagent_nodes
 WHEN NEW.root_ticket_id IS NOT OLD.root_ticket_id
   OR NEW.parent_ticket_id IS NOT OLD.parent_ticket_id
   OR NEW.parent_assignment_id IS NOT OLD.parent_assignment_id
   OR NEW.item_key IS NOT OLD.item_key
   OR NEW.objective IS NOT OLD.objective
   OR NEW.worker_bot_id IS NOT OLD.worker_bot_id
   OR NEW.depth IS NOT OLD.depth
   OR NEW.request_sha256 IS NOT OLD.request_sha256
   OR (OLD.child_ticket_id IS NOT NULL AND NEW.child_ticket_id IS NOT OLD.child_ticket_id)
 BEGIN SELECT RAISE(ABORT,'Subagent lineage is immutable'); END;
"#;

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct SubagentPolicy {
    pub root_ticket_id:String,
    pub team_version:i64,
    pub policy_revision:i64,
    pub max_depth:i64,
    pub max_children:i64,
    pub max_total:i64,
    pub revoked:bool,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct SubagentNode {
    pub id:String,
    pub root_ticket_id:String,
    pub parent_ticket_id:String,
    pub parent_assignment_id:String,
    pub child_ticket_id:Option<String>,
    pub key:String,
    pub worker_bot_id:String,
    pub depth:i64,
    pub objective:String,
    pub state:String,
    pub reason:Option<String>,
}

fn policy(store:&Store,root:&str)->Result<Option<SubagentPolicy>> {
    store.db.query_row(
        "SELECT root_ticket_id,team_version,policy_revision,max_depth,
                max_children,max_total,revoked_at
         FROM sa_subagent_policies WHERE root_ticket_id=?1",
        [root],|r|Ok(SubagentPolicy{
            root_ticket_id:r.get(0)?,team_version:r.get(1)?,policy_revision:r.get(2)?,
            max_depth:r.get(3)?,max_children:r.get(4)?,max_total:r.get(5)?,
            revoked:r.get::<_,Option<i64>>(6)?.is_some(),
        }),
    ).optional().map_err(|e|e.to_string())
}

fn node_row(row:&rusqlite::Row<'_>)->rusqlite::Result<SubagentNode> {
    Ok(SubagentNode{
        id:row.get(0)?,root_ticket_id:row.get(1)?,
        parent_ticket_id:row.get(2)?,parent_assignment_id:row.get(3)?,
        child_ticket_id:row.get(4)?,key:row.get(5)?,worker_bot_id:row.get(6)?,
        depth:row.get(7)?,objective:row.get(8)?,state:row.get(9)?,reason:row.get(10)?,
    })
}

fn nodes(store:&Store,root:&str)->Result<Vec<SubagentNode>> {
    let mut query=store.db.prepare(
        "SELECT id,root_ticket_id,parent_ticket_id,parent_assignment_id,
                child_ticket_id,item_key,worker_bot_id,depth,objective,state,reason
         FROM sa_subagent_nodes WHERE root_ticket_id=?1 ORDER BY created_at,id LIMIT 24"
    ).map_err(|e|e.to_string())?;
    let rows=query.query_map([root],node_row).map_err(|e|e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
}

fn valid_key(key:&str)->bool {
    !key.is_empty() && key.len()<=64 &&
        key.bytes().all(|b|b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'))
}

fn root_for_parent(store:&Store,parent_ticket_id:&str)->Result<Option<(String,i64)>> {
    if let Some(root)=store.db.query_row(
        "SELECT ticket_id FROM sa_work_items WHERE child_ticket_id=?1",
        [parent_ticket_id],|r|r.get::<_,String>(0),
    ).optional().map_err(|e|e.to_string())? {return Ok(Some((root,0)))}
    store.db.query_row(
        "SELECT root_ticket_id,depth FROM sa_subagent_nodes WHERE child_ticket_id=?1",
        [parent_ticket_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)),
    ).optional().map_err(|e|e.to_string())
}

fn allowed_children(store:&Store,root:&str,parent_ticket_id:&str,parent_bot_id:&str)->Result<Vec<String>> {
    let snapshot=ticket_team_snapshot(store,root)?;
    if snapshot.provenance!=TeamSnapshotProvenance::Native {return Ok(Vec::new())}
    let mut ancestors=HashSet::new();
    ancestors.insert(parent_bot_id.to_owned());
    let mut ancestor=parent_ticket_id.to_owned();
    for _ in 0..MAX_DEPTH+1 {
        let node:Option<(String,String)>=store.db.query_row(
            "SELECT parent_ticket_id,worker_bot_id FROM sa_subagent_nodes
             WHERE child_ticket_id=?1",
            [&ancestor],|r|Ok((r.get(0)?,r.get(1)?)),
        ).optional().map_err(|e|e.to_string())?;
        let Some((previous,_worker))=node else {break};
        let owner:String=store.db.query_row(
            "SELECT worker_bot_id FROM sa_tickets WHERE id=?1",
            [&previous],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        ancestors.insert(owner);
        ancestor=previous;
    }
    Ok(snapshot.worker_bot_ids.into_iter()
        .filter(|id|!ancestors.contains(id)).collect())
}

fn node_input_digest(key:&str,worker:&str,objective:&str)->String {
    let bytes=serde_json::to_vec(&json!([key,worker,objective])).unwrap_or_default();
    Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect()
}

fn stage_spawn(
    store:&Store,session_id:&str,key:&str,worker_id:&str,objective:&str,
)->Result<SubagentNode> {
    let objective=objective.trim();
    if !valid_key(key) || objective.is_empty() ||
        objective.len()>MAX_OBJECTIVE_BYTES || objective.contains('\0') {
        return Err("Subagent request needs one bounded, unique key and objective.".into());
    }
    // A live, bounded native Worker assignment is mandatory at tool time.
    if super::read_only_policy(store,session_id)?.is_none() {
        return Err("Subagent spawn requires an active native SuperAgent Worker.".into());
    }
    let parent:Option<(String,String,String)>=store.db.query_row(
        "SELECT assignment.ticket_id,assignment.id,assignment.bot_id
         FROM sa_assignments assignment
         JOIN sa_tickets ticket ON ticket.id=assignment.ticket_id
         WHERE assignment.session_id=?1 AND assignment.state='running'
           AND ticket.state='running'
           AND ticket.active_assignment_id=assignment.id",
        [session_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((parent_ticket_id,parent_assignment_id,parent_bot_id))=parent else {
        return Err("No active parent Worker Assignment is available.".into());
    };
    let (root,previous_depth)=root_for_parent(store,&parent_ticket_id)?
        .ok_or("Only WorkItem Workers and their authorized descendants may spawn subagents.")?;
    let settings=policy(store,&root)?.ok_or("Subagent spawning is not authorized for this Demand.")?;
    if settings.revoked || previous_depth>=settings.max_depth {
        return Err("Subagent depth limit reached or root authorization revoked.".into());
    }
    if !super::scheduler::parent_active(store,&parent_ticket_id)? ||
        !super::auto::ticket_authorized(store,&root)? {
        return Err("Parent Demand is no longer authorized.".into());
    }
    let snapshot=ticket_team_snapshot(store,&root)?;
    let original=get_ticket(store,&root)?;
    let config=configuration(store,&original.orchestration_id)?;
    if original.state!="queued" || !config.enabled ||
        config.policy_revision!=settings.policy_revision ||
        snapshot.team_version!=settings.team_version ||
        snapshot.policy.policy_revision!=settings.policy_revision {
        return Err("Root Team or Demand policy changed before child authorization.".into());
    }
    let permitted=allowed_children(store,&root,&parent_ticket_id,&parent_bot_id)?;
    if !permitted.iter().any(|id|id==worker_id) {
        return Err("Child Worker is not a permitted, independent bot from this Team.".into());
    }
    let bot=bots::get(store,worker_id)?;
    let position=snapshot.worker_bot_ids.iter().position(|id|id==worker_id)
        .ok_or("Child Worker was not in the originally admitted Team.")?;
    if bot.paused || bot.project_id!=snapshot.project_id ||
        !snapshot_role_profile_matches(&store.db,&snapshot,"worker",position,worker_id)? {
        return Err("Child Worker provider, Skills, project or membership changed.".into());
    }
    let digest=node_input_digest(key,worker_id,objective);
    let existing=store.db.query_row(
        "SELECT id,root_ticket_id,parent_ticket_id,parent_assignment_id,
                child_ticket_id,item_key,worker_bot_id,depth,objective,state,reason
         FROM sa_subagent_nodes WHERE parent_assignment_id=?1 AND item_key=?2",
        params![parent_assignment_id,key],node_row,
    ).optional().map_err(|e|e.to_string())?;
    if let Some(existing)=existing {
        if node_input_digest(&existing.key,&existing.worker_bot_id,&existing.objective)!=digest {
            return Err("Subagent key already identifies a different immutable request.".into());
        }
        return Ok(existing);
    }
    let existing_count:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_subagent_nodes WHERE root_ticket_id=?1",
        [&root],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let parent_count:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_subagent_nodes WHERE parent_ticket_id=?1",
        [&parent_ticket_id],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    if existing_count>=settings.max_total || parent_count>=settings.max_children {
        return Err("Subagent count budget exhausted; no more native children may be admitted.".into());
    }
    let now=storage::now();
    let id=storage::id();
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_subagent_nodes(id,root_ticket_id,parent_ticket_id,
            parent_assignment_id,item_key,objective,worker_bot_id,depth,
            request_sha256,state,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10,?10)",
        params![id,root,parent_ticket_id,parent_assignment_id,key,objective,
            worker_id,previous_depth+1,digest,now],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'subagent_requested',?2,?3)",
        params![parent_ticket_id,format!("Native pending child {id} at depth {}",
            previous_depth+1),now],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    store.db.query_row(
        "SELECT id,root_ticket_id,parent_ticket_id,parent_assignment_id,
                child_ticket_id,item_key,worker_bot_id,depth,objective,state,reason
         FROM sa_subagent_nodes WHERE id=?1",
        [&id],node_row,
    ).map_err(|e|e.to_string())
}

pub(crate) fn host(
    app:tauri::AppHandle,session_id:String,stopped:Arc<AtomicBool>,
)->Result<Option<ToolHost>> {
    let selection={
        let state=app.state::<AppState>();
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let parent:Option<(String,String)>=store.db.query_row(
            "SELECT a.ticket_id,a.bot_id FROM sa_assignments a
             JOIN sa_tickets t ON t.id=a.ticket_id
             WHERE a.session_id=?1 AND a.state='running' AND
               t.state='running' AND t.active_assignment_id=a.id",
            [&session_id],|r|Ok((r.get(0)?,r.get(1)?)),
        ).optional().map_err(|e|e.to_string())?;
        let Some((parent,bot))=parent else{return Ok(None)};
        let Some((root,depth))=root_for_parent(&store,&parent)? else{return Ok(None)};
        let Some(policy)=policy(&store,&root)? else{return Ok(None)};
        if policy.revoked || depth>=policy.max_depth ||
            super::read_only_policy(&store,&session_id)?.is_none() {
            return Ok(None);
        }
        let workers=allowed_children(&store,&root,&parent,&bot)?;
        if workers.is_empty() {return Ok(None)}
        (parent,root,workers)
    };
    let (parent_id,root_id,workers)=selection;
    let spec=json!({
        "type":"function","name":TOOL_NAME,
        "description":"Request an additional existing Team Worker as a native, read-only subagent. The child is queued by the native scheduler under the current Demand, its exact parent Assignment, depth and aggregate budgets. This never creates a Team member, grants tool/write access, or approves work.",
        "inputSchema":{
            "type":"object","additionalProperties":false,
            "properties":{
                "parentTicketId":{"type":"string","enum":[parent_id]},
                "rootTicketId":{"type":"string","enum":[root_id]},
                "workerBotId":{"type":"string","enum":workers},
                "key":{"type":"string","maxLength":64},
                "objective":{"type":"string","maxLength":4096}
            },
            "required":["parentTicketId","rootTicketId","workerBotId","key","objective"]
        }
    });
    let handler=Arc::new(move |tool:String,args:Value|->ToolFuture {
        let (app,session_id,stopped)=(app.clone(),session_id.clone(),stopped.clone());
        Box::pin(async move {
            if stopped.load(Ordering::SeqCst) {
                return Err("Parent Worker has stopped; subagent admission denied.".into());
            }
            if tool!=TOOL_NAME || args.as_object().is_none_or(|o|o.len()!=5) {
                return Err("Unsupported or malformed SuperAgent subagent tool.".into());
            }
            let read=|name:&str|->Result<String> {
                args.get(name).and_then(Value::as_str).map(str::to_owned)
                    .ok_or_else(||format!("{name} must be an exact bounded string."))
            };
            let parent=read("parentTicketId")?;
            let root=read("rootTicketId")?;
            let worker=read("workerBotId")?;
            let key=read("key")?;
            let objective=read("objective")?;
            let result={
                let state=app.state::<AppState>();
                let store=state.store.lock().map_err(|e|e.to_string())?;
                let parent_link=root_for_parent(&store,&parent)?;
                if parent_link.as_ref().is_none_or(|(actual,_)|actual!=&root) {
                    return Err("Subagent request is not bound to the native parent Demand.".into());
                }
                let _fence=super::effects::lock_dispatch(&store.path)?;
                let node=stage_spawn(&store,&session_id,&key,&worker,&objective)?;
                serde_json::to_value(node).map_err(|e|e.to_string())?
            };
            Ok(result)
        })
    });
    Ok(Some(ToolHost {tools:vec![spec],handler,approval_tools:HashSet::new()}))
}


/// Compose only the two app-owned scoped SuperAgent tools. The native reviewer
/// policy revalidates this exact small catalog before provider launch.
pub(crate) fn extend_host(
    base:Option<ToolHost>,app:tauri::AppHandle,
    session_id:String,stopped:Arc<AtomicBool>,
)->Result<Option<ToolHost>> {
    let nested=host(app,session_id,stopped)?;
    match (base,nested) {
        (None,None)=>Ok(None),
        (Some(one),None)|(None,Some(one))=>Ok(Some(one)),
        (Some(writer),Some(spawner))=>{
            if writer.tools.len()!=1 ||
                writer.tools[0]["name"]!=super::write_grants::TOOL_NAME ||
                spawner.tools.len()!=1 || spawner.tools[0]["name"]!=TOOL_NAME ||
                !writer.approval_tools.is_empty() || !spawner.approval_tools.is_empty() {
                return Err("SuperAgent host catalog cannot add unscoped tools.".into());
            }
            let mut tools=writer.tools;
            tools.extend(spawner.tools);
            let write_handler=writer.handler;
            let spawn_handler=spawner.handler;
            Ok(Some(ToolHost {
                tools,approval_tools:HashSet::new(),
                handler:Arc::new(move |name,args| {
                    match name.as_str() {
                        super::write_grants::TOOL_NAME=>(write_handler)(name,args),
                        TOOL_NAME=>(spawn_handler)(name,args),
                        _=>{
                            let denied:ToolFuture=Box::pin(async {
                                Err("Unsupported tool in bounded SuperAgent host.".into())
                            });
                            denied
                        },
                    }
                }),
            }))
        }
    }
}

#[tauri::command]
pub async fn superagent_subagents(
    window:tauri::WebviewWindow,state:State<'_,AppState>,ticket_id:String,
)->Result<Value> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    get_ticket(&store,&ticket_id)?;
    let settings=policy(&store,&ticket_id)?;
    let items=nodes(&store,&ticket_id)?;
    Ok(json!({"rootTicketId":ticket_id,"policy":settings,"nodes":items}))
}

#[tauri::command]
pub async fn superagent_enable_subagents(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    ticket_id:String,expected_revision:i64,
    max_depth:i64,max_children:i64,max_total:i64,
)->Result<SubagentPolicy> {
    require_main_window(&window)?;
    if !(1..=MAX_DEPTH).contains(&max_depth) ||
       !(1..=MAX_CHILDREN).contains(&max_children) ||
       !(1..=MAX_TOTAL).contains(&max_total) ||
       max_children>max_total {
        return Err("Subagent limits must be finite: depth 1..3, per parent 1..4, total 1..24.".into());
    }
    let snapshot={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let root=get_ticket(&store,&ticket_id)?;
        let planned:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_demand_plans
             WHERE ticket_id=?1 AND state='active')",
            [&ticket_id],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        let planning:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_audit
             WHERE ticket_id=?1 AND kind='demand_auto_plan_requested')",
            [&ticket_id],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        let current=ticket_team_snapshot(&store,&ticket_id)?;
        if !matches!(root.state.as_str(),"running"|"queued") ||
            root.revision!=expected_revision || !(planned||planning) ||
            current.provenance!=TeamSnapshotProvenance::Native ||
            current.worker_bot_ids.len()<2 ||
            !super::auto::ticket_authorized(&store,&ticket_id)? {
            return Err("Subagents require a currently authorized active Demand with at least two pinned Workers.".into());
        }
        if policy(&store,&ticket_id)?.is_some() {
            return Err("This Demand already has a frozen subagent policy.".into());
        }
        current
    };
    use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
    let dialog=window.dialog()
        .message(format!(
            "Authorize native read-only Worker-created subagents for Demand {ticket_id}?\n\n\
             Maximum depth: {max_depth}\nChildren per parent: {max_children}\n\
             Total descendants: {max_total}\n\n\
             Only the original Team Workers, project and current Demand Mandate. \
             Every child receives its own Ticket/session and cannot inherit \
             parent file-write, shell, MCP or GitHub permissions. Stopping the \
             Demand revokes all descendants."
        ))
        .title("Authorize bounded SuperAgent subagents")
        .buttons(MessageDialogButtons::OkCancelCustom("Authorize bounded children".into(),"Cancel".into()));
    #[cfg(desktop)]
    let dialog=dialog.parent(&window);
    let (send,recv)=tokio::sync::oneshot::channel();
    dialog.show(move |approved|{let _=send.send(approved);});
    if !recv.await.unwrap_or(false) {
        return Err("Subagent delegation was not authorized.".into());
    }
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let root=get_ticket(&store,&ticket_id)?;
    let current=ticket_team_snapshot(&store,&ticket_id)?;
    if !matches!(root.state.as_str(),"running"|"queued") ||
       root.revision!=expected_revision ||
       current.team_version!=snapshot.team_version ||
       current.policy.policy_revision!=snapshot.policy.policy_revision ||
       policy(&store,&ticket_id)?.is_some() {
        return Err("Demand or Team changed before subagent authorization.".into());
    }
    let now=storage::now();
    store.db.execute(
        "INSERT INTO sa_subagent_policies(root_ticket_id,team_version,
           policy_revision,root_ticket_revision,max_depth,max_children,max_total,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![ticket_id,current.team_version,current.policy.policy_revision,
            expected_revision,max_depth,max_children,max_total,now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'subagent_policy_issued',?2,?3)",
        params![ticket_id,
            format!("read-only depth={max_depth},children={max_children},total={max_total}"),now],
    ).map_err(|e|e.to_string())?;
    policy(&store,&ticket_id)?.ok_or("Subagent authorization disappeared after insertion.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_subagent_lineage_and_limits_are_immutable_in_sqlite() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE sa_tickets(id TEXT PRIMARY KEY);
             CREATE TABLE sa_assignments(id TEXT PRIMARY KEY);
             INSERT INTO sa_tickets(id) VALUES('root'),('parent'),('child');
             INSERT INTO sa_assignments(id) VALUES('parent-assignment');",
        ).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute(
            "INSERT INTO sa_subagent_policies(root_ticket_id,team_version,
               policy_revision,root_ticket_revision,max_depth,max_children,
               max_total,created_at)
             VALUES('root',1,1,1,2,2,4,100)",[],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_subagent_nodes(id,root_ticket_id,parent_ticket_id,
                parent_assignment_id,item_key,objective,worker_bot_id,depth,
                request_sha256,state,created_at,updated_at)
             VALUES('node','root','parent','parent-assignment','test',
                'Review evidence','other-worker',1,?1,'pending',100,100)",
            ["0".repeat(64)],
        ).unwrap();
        db.execute(
            "UPDATE sa_subagent_nodes SET child_ticket_id='child',
               state='running' WHERE id='node'",[],
        ).unwrap();
        assert!(db.execute(
            "UPDATE sa_subagent_nodes SET worker_bot_id='unapproved'
             WHERE id='node'",[],
        ).is_err(),"the dispatched Worker binding may never be changed");
        assert!(db.execute(
            "UPDATE sa_subagent_nodes SET child_ticket_id=NULL WHERE id='node'",[],
        ).is_err(),"a child identity cannot be detached after admission");
        assert!(db.execute(
            "INSERT INTO sa_subagent_nodes(id,root_ticket_id,parent_ticket_id,
                parent_assignment_id,item_key,objective,worker_bot_id,depth,
                request_sha256,state,created_at,updated_at)
             VALUES('invalid','root','parent','parent-assignment','other',
                'Overflow','worker',4,?1,'pending',100,100)",
            ["1".repeat(64)],
        ).is_err(),"depth exceeding the hard SQLite ceiling fails closed");
        assert!(db.execute(
            "INSERT INTO sa_subagent_nodes(id,root_ticket_id,parent_ticket_id,
                parent_assignment_id,item_key,objective,worker_bot_id,depth,
                request_sha256,state,created_at,updated_at)
             VALUES('duplicate','root','parent','parent-assignment','test',
                'Other request','worker',1,?1,'pending',100,100)",
            ["2".repeat(64)],
        ).is_err(),"same parent Assignment cannot reuse a request key");
    }

    #[test]
    fn subagent_keys_and_request_digests_are_bounded_and_stable() {
        assert!(valid_key("child.01"));
        assert!(!valid_key(""));
        assert!(!valid_key("../escape"));
        assert!(!valid_key(&"a".repeat(65)));
        let first=node_input_digest("test","worker","Write test");
        assert_eq!(first.len(),64);
        assert_eq!(first,node_input_digest("test","worker","Write test"));
        assert_ne!(first,node_input_digest("test","worker","Change production"));
    }
}
