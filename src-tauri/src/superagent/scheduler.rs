//! Durable, demand-scoped dependency scheduler. Every item uses a separate
//! native Ticket/Assignment; a plan is not a tool grant or a reviewer verdict.
//! No model-produced topology may widen the original local-user authorization.

use super::{configuration, coordinator_plan_current, get_ticket, ticket_team_snapshot,
    snapshot_role_profile_matches, TeamSnapshotProvenance};
use crate::{bots, storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tauri::{Manager,State};

mod dispatch;
pub(crate) mod evidence;
pub(crate) use dispatch::{advance,parent_active,revoke_descendants,sessions_for_root};

const MAX_ITEMS: usize = 32;
const MAX_ITEM_OBJECTIVE: usize = 4096;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_demand_plans (
    ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    ticket_revision INTEGER NOT NULL,
    team_version INTEGER NOT NULL CHECK(team_version>0),
    policy_revision INTEGER NOT NULL,
    plan_sha256 TEXT NOT NULL CHECK(length(plan_sha256)=64),
    state TEXT NOT NULL CHECK(state IN ('active','ready_for_review','blocked','stopped','completed')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sa_work_items (
    id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES sa_demand_plans(ticket_id) ON DELETE RESTRICT,
    item_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal>=0 AND ordinal<32),
    objective TEXT NOT NULL,
    worker_bot_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending','running','waiting_review','completed','blocked')),
    child_ticket_id TEXT UNIQUE REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    last_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(ticket_id,item_key),
    UNIQUE(ticket_id,ordinal)
);
CREATE TABLE IF NOT EXISTS sa_work_dependencies (
    ticket_id TEXT NOT NULL,
    item_key TEXT NOT NULL,
    depends_on TEXT NOT NULL,
    PRIMARY KEY(ticket_id,item_key,depends_on),
    FOREIGN KEY(ticket_id,item_key)
      REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT,
    FOREIGN KEY(ticket_id,depends_on)
      REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT,
    CHECK(item_key<>depends_on)
);
CREATE INDEX IF NOT EXISTS sa_work_items_progress
  ON sa_work_items(ticket_id,state,ordinal);
CREATE INDEX IF NOT EXISTS sa_work_items_child
  ON sa_work_items(child_ticket_id) WHERE child_ticket_id IS NOT NULL;
"#;

/// A model may REQUEST a write-required WorkItem, but only a fresh local-user
/// confirmation stores the exact scope. Scopes are immutable and cannot be
/// copied into another child Ticket or replayed with different paths.
pub(crate) const WRITE_SCOPES_SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_work_item_write_scopes (
    item_id TEXT PRIMARY KEY REFERENCES sa_work_items(id) ON DELETE RESTRICT,
    allowed_paths TEXT NOT NULL CHECK(json_valid(allowed_paths)),
    root_ticket_revision INTEGER NOT NULL,
    authorized_at INTEGER NOT NULL
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_write_scope_immutable
 BEFORE UPDATE ON sa_work_item_write_scopes
 BEGIN SELECT RAISE(ABORT,'WorkItem file scope cannot be widened'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_write_scope_no_delete
 BEFORE DELETE ON sa_work_item_write_scopes
 BEGIN SELECT RAISE(ABORT,'WorkItem write authorization is durable'); END;
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkItemInput {
    pub key: String,
    pub objective: String,
    pub worker_bot_id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// The Coordinator can request this capability, but cannot authorize it.
    #[serde(default)]
    pub requires_write: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkItem {
    pub id: String,
    pub key: String,
    pub ordinal: usize,
    pub worker_bot_id: String,
    pub objective: String,
    pub depends_on: Vec<String>,
    pub state: String,
    pub child_ticket_id: Option<String>,
    pub child_ticket_state: Option<String>,
    pub last_reason: Option<String>,
    pub requires_write: bool,
    pub write_authorized: bool,
    pub evidence_sha256: Option<String>,
    pub approval_kind: Option<String>,
    pub evidence_verified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct DemandPlan {
    pub ticket_id: String,
    pub ticket_revision: i64,
    pub team_version: i64,
    pub policy_revision: i64,
    pub plan_sha256: String,
    pub state: String,
    pub items: Vec<WorkItem>,
}

/// An item graph is data only. Its workers, objective size and topology must
/// be checked before the first persistent write, irrespective of source.
fn validate_items(items: &[WorkItemInput]) -> Result<()> {
    if items.is_empty() || items.len()>MAX_ITEMS {
        return Err("A Demand plan requires between 1 and 32 work items.".into());
    }
    let mut keys=HashSet::new();
    for item in items {
        if item.key.is_empty() || item.key.len()>64 ||
            !item.key.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte,b'-'|b'_'|b'.')) ||
            !keys.insert(item.key.as_str()) {
            return Err("Work-item keys must be unique, bounded and URL-safe.".into());
        }
        let objective=item.objective.trim();
        if objective.is_empty() || objective.len()>MAX_ITEM_OBJECTIVE ||
            objective.chars().any(|character| character=='\0') || item.worker_bot_id.is_empty() {
            return Err("Each WorkItem needs a bounded objective and an existing Worker.".into());
        }
        if item.depends_on.len()>MAX_ITEMS {
            return Err("A WorkItem has too many prerequisites.".into());
        }
        let unique=item.depends_on.iter().collect::<HashSet<_>>();
        if unique.len()!=item.depends_on.len() || item.depends_on.iter().any(|key| key==&item.key) {
            return Err("Duplicate or self-referential WorkItem prerequisite.".into());
        }
    }
    let mut settled=HashSet::<&str>::new();
    for _ in 0..items.len() {
        let before=settled.len();
        for item in items {
            if item.depends_on.iter().all(|dep| settled.contains(dep.as_str())) &&
                !settled.contains(item.key.as_str()) {
                settled.insert(&item.key);
            }
        }
        if settled.len()==items.len() { return Ok(()); }
        if before==settled.len() { break; }
    }
    Err("WorkItem dependencies contain a cycle or unknown key.".into())
}

fn sha256(items: &[WorkItemInput]) -> Result<String> {
    let bytes=serde_json::to_vec(items).map_err(|error|error.to_string())?;
    Ok(Sha256::digest(bytes).iter().map(|byte|format!("{byte:02x}")).collect())
}

fn inspect(store:&Store,ticket_id:&str) -> Result<Option<DemandPlan>> {
    let row:Option<(i64,i64,i64,String,String)>=store.db.query_row(
        "SELECT ticket_revision,team_version,policy_revision,plan_sha256,state
         FROM sa_demand_plans WHERE ticket_id=?1",
        [ticket_id], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
    ).optional().map_err(|error|error.to_string())?;
    let Some((ticket_revision,team_version,policy_revision,plan_sha256,state))=row else {
        return Ok(None);
    };
    let mut statement=store.db.prepare(
        "SELECT id,item_key,ordinal,worker_bot_id,objective,state,child_ticket_id,last_reason,requires_write
         FROM sa_work_items WHERE ticket_id=?1 ORDER BY ordinal,id"
    ).map_err(|error|error.to_string())?;
    let rows=statement.query_map([ticket_id],|row| {
        Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,
            row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,
            row.get::<_,Option<String>>(6)?,row.get::<_,Option<String>>(7)?,
            row.get::<_,bool>(8)?))
    }).map_err(|error|error.to_string())?;
    let mut items=Vec::new();
    for row in rows {
        let (id,key,ordinal,worker_bot_id,objective,item_state,child_ticket_id,last_reason,requires_write)=
            row.map_err(|error|error.to_string())?;
        let mut deps=store.db.prepare(
            "SELECT depends_on FROM sa_work_dependencies WHERE ticket_id=?1 AND item_key=?2
             ORDER BY depends_on"
        ).map_err(|error|error.to_string())?;
        let depends_on=deps.query_map(params![ticket_id,key],|row|row.get::<_,String>(0))
            .map_err(|error|error.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(|error|error.to_string())?;
        let child_ticket_state=child_ticket_id.as_deref().map(|id| {
            store.db.query_row("SELECT state FROM sa_tickets WHERE id=?1",[id],
                |row|row.get::<_,String>(0)).map_err(|error|error.to_string())
        }).transpose()?;
        let write_authorized:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_item_write_scopes WHERE item_id=?1)",
            [&id],|r|r.get(0),
        ).map_err(|error|error.to_string())?;
        let verification=evidence::receipt(store,&id)?;
        let (evidence_sha256,approval_kind,evidence_verified_at)=match verification {
            Some((digest,kind,at))=>(Some(digest),Some(kind),Some(at)),
            None=>(None,None,None),
        };
        items.push(WorkItem{id,key,ordinal:ordinal as usize,worker_bot_id,
            objective,depends_on,state:item_state,child_ticket_id,child_ticket_state,
            last_reason,requires_write,write_authorized,evidence_sha256,
            approval_kind,evidence_verified_at});
    }
    Ok(Some(DemandPlan{ticket_id:ticket_id.into(),ticket_revision,team_version,
        policy_revision,plan_sha256,state,items}))
}

/// Creating this plan is an explicit trusted local-user act. Coordinator text
/// is input context, not delegated authority. Only the pinned Workers can be
/// named, every item remains initially read-only, and admission still rechecks
/// all native Ticket/Mandate/project/lease guards.
fn create_plan(store:&Store,ticket_id:&str,revision:i64,items:&[WorkItemInput])->Result<DemandPlan> {
    validate_items(items)?;
    let digest=sha256(items)?;
    if let Some(current)=inspect(store,ticket_id)? {
        if current.plan_sha256==digest && current.ticket_revision==revision {
            return Ok(current);
        }
        return Err("A different immutable WorkItem plan already owns this Demand.".into());
    }
    let ticket=get_ticket(store,ticket_id)?;
    if !super::auto::ticket_authorized(store,ticket_id)? {
        return Err("Original GitHub Auto authorization was revoked or expired.".into());
    }
    if ticket.revision!=revision || ticket.state!="queued" || ticket.active_assignment_id.is_some() {
        return Err("Demand must have an exact-revision, finished coordinator plan.".into());
    }
    let snapshot=ticket_team_snapshot(store,ticket_id)?;
    if snapshot.provenance!=TeamSnapshotProvenance::Native || snapshot.team_version<=0 {
        return Err("Legacy or unverified Teams cannot issue autonomous WorkItems.".into());
    }
    let config=configuration(store,&ticket.orchestration_id)?;
    if !config.enabled || crate::feature_controls::get().bots_paused ||
        config.policy_revision!=snapshot.policy.policy_revision ||
        config.team_version!=snapshot.team_version {
        return Err("Original Team policy is no longer eligible for work admission.".into());
    }
    let coordinator=bots::get(store,&snapshot.coordinator_bot_id)?;
    if !coordinator_plan_current(store,ticket_id,&coordinator.id,&coordinator.project_id)? {
        return Err("Original Coordinator plan session is missing or stale.".into());
    }
    let authorized:bool=store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_mandates m
         JOIN sa_tickets t ON t.id=m.ticket_id
         WHERE t.id=?1 AND t.actor='local-user' AND t.source='manual'
         AND m.revoked=0 AND m.expires_at>?2 AND m.policy_revision=?3
         AND m.project_id=?4 AND m.allowed_tools='[]' AND m.allowed_side_effects=0
         AND EXISTS(SELECT 1 FROM sa_audit a WHERE a.ticket_id=t.id
           AND a.kind='admitted' AND a.detail='manual readonly coordinator reserved'))",
        params![ticket_id,storage::now(),snapshot.policy.policy_revision,snapshot.project_id],
        |row|row.get(0),
    ).map_err(|error|error.to_string())?;
    if !authorized {
        return Err("No current local-user read-only Demand authorization.".into());
    }
    let mut selected=HashMap::new();
    for (index,worker_id) in snapshot.worker_bot_ids.iter().enumerate() {
        selected.insert(worker_id.as_str(),index);
    }
    for item in items {
        let ordinal=selected.get(item.worker_bot_id.as_str())
            .ok_or("A WorkItem selected a Worker outside the immutable Team.")?;
        let worker=bots::get(store,&item.worker_bot_id)?;
        if worker.paused || worker.project_id!=snapshot.project_id ||
            !snapshot_role_profile_matches(&store.db,&snapshot,"worker",*ordinal,&worker.id)? {
            return Err("Selected Worker is paused, moved or differs from the admitted Team profile.".into());
        }
    }
    let now=storage::now();
    let tx=store.db.unchecked_transaction().map_err(|error|error.to_string())?;
    let inserted=tx.execute(
        "INSERT INTO sa_demand_plans(ticket_id,ticket_revision,team_version,policy_revision,
          plan_sha256,state,created_at,updated_at)
         SELECT ?1,?2,?3,?4,?5,'active',?6,?6
         WHERE EXISTS(SELECT 1 FROM sa_tickets WHERE id=?1 AND revision=?2
           AND state='queued' AND active_assignment_id IS NULL)",
        params![ticket_id,revision,snapshot.team_version,snapshot.policy.policy_revision,digest,now],
    ).map_err(|error|error.to_string())?;
    if inserted!=1 { return Err("Demand changed before plan admission.".into()); }
    for (ordinal,item) in items.iter().enumerate() {
        tx.execute(
            "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,worker_bot_id,
                requires_write,state,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,'pending',?8,?8)",
            params![storage::id(),ticket_id,item.key,ordinal as i64,item.objective.trim(),
                item.worker_bot_id,item.requires_write,now],
        ).map_err(|error|error.to_string())?;
    }
    for item in items {
        for predecessor in &item.depends_on {
            tx.execute(
                "INSERT INTO sa_work_dependencies(ticket_id,item_key,depends_on)
                 VALUES(?1,?2,?3)",
                params![ticket_id,item.key,predecessor],
            ).map_err(|error|error.to_string())?;
        }
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_plan_issued',?2,?3)",
        params![ticket_id,format!("{} read-only items; plan digest {}",items.len(),digest),now],
    ).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error|error.to_string())?;
    inspect(store,ticket_id)?.ok_or("WorkItem plan disappeared after admission.".into())
}

#[tauri::command]
pub async fn superagent_plan_demand(
    window:tauri::WebviewWindow,app:tauri::AppHandle,state:State<'_,AppState>,
    ticket_id:String,expected_revision:i64,items:Vec<WorkItemInput>,
)->Result<DemandPlan> {
    super::require_main_window(&window)?;
    let plan={
        let store=state.store.lock().map_err(|error|error.to_string())?;
        create_plan(&store,&ticket_id,expected_revision,&items)?
    };
    // Dispatch follows the persisted, validated plan. An unavailable Worker
    // leaves its WorkItem ready instead of losing the operator's plan.
    advance(&app).await?;
    Ok(plan)
}

#[tauri::command]
pub async fn superagent_demand_plan(
    window:tauri::WebviewWindow,state:State<'_,AppState>,ticket_id:String,
)->Result<Option<DemandPlan>> {
    super::require_main_window(&window)?;
    let store=state.store.lock().map_err(|error|error.to_string())?;
    inspect(&store,&ticket_id)
}


/// A single initial operator action starts the configured Team Coordinator.
/// Its JSON plan is strictly validated and may only schedule read-only native
/// WorkItems for the Workers in the immutable Team; no model grants effects.
#[tauri::command]
pub async fn superagent_start_demand(
    window:tauri::WebviewWindow,app:tauri::AppHandle,state:State<'_,AppState>,
    orchestration_id:String,objective:String,request_key:String,
)->Result<super::Ticket> {
    super::require_main_window(&window)?;
    if crate::feature_controls::get().bots_paused {
        return Err("Bots are globally paused; Demand admission denied.".into());
    }
    let (ticket,launch,coordinator_id,workers)={
        let active=state.agents.lock().map_err(|error|error.to_string())?
            .keys().cloned().collect::<Vec<_>>();
        let mut store=state.store.lock().map_err(|error|error.to_string())?;
        let config=configuration(&store,&orchestration_id)?;
        let busy=super::active_manual_conflict(&store,&active,
            &config.coordinator_bot_id,&config.project_id)?;
        let (ticket,launch)=super::admit_role_with_selected(
            &mut store,&orchestration_id,&objective,&request_key,busy,true,false,
            Some(&config.worker_bot_id),
        )?;
        if launch {
            store.db.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'demand_auto_plan_requested',
                   'Local user authorized bounded read-only Team planning',?2)",
                params![ticket.id,storage::now()],
            ).map_err(|error|error.to_string())?;
        }
        (ticket,launch,config.coordinator_bot_id,config.worker_bot_ids)
    };
    if !launch {return Ok(ticket)}
    let selected=workers.iter().map(|id|format!("- {id}")).collect::<Vec<_>>().join("\n");
    let delivered=bots::gateway::deliver(&app,bots::gateway::Inbound{
        bot_id:coordinator_id,
        channel_id:super::CHANNEL.into(),
        thread:ticket.id.clone(),
        sender_id:"local-user".into(),
        sender_name:"Authorized Team Demand".into(),
        text:format!(
            "SuperAgent Demand {}\n\n{}\n\n\
             Output a SINGLE JSON object with one field, workItems, containing 1 to 32 \
             objects with key, objective, workerBotId and dependsOn (array of prerequisite keys). \
             Use ONLY these existing Worker IDs:\n{}\n\n\
             Keep every objective inside the user request. Plan read-only tasks only; \
             neither you nor the Workers have permission to change code, run shell commands, \
             invoke external tools, approve Council or widen task scope. \
             Each Worker gets a fresh native Ticket, session and policy check. \
             Output valid JSON only, no Markdown or commentary.",
            ticket.id,ticket.objective,selected,
        ),
        trusted:true,image_ids:None,moodboard_ids:None,
    }).await;
    if let Err(error)=delivered {
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let _=super::fail_launch(&store,&ticket.id,&error);
        return Err(format!("Coordinator launch failed; Demand {} remains inspectable: {error}",ticket.id));
    }
    let store=state.store.lock().map_err(|error|error.to_string())?;
    get_ticket(&store,&ticket.id)
}


/// Persist scope and evidence atomically. A failed audit insertion must not
/// leave a runnable write permission behind; admission is pinned to the
/// exact still-pending WorkItem and frozen Demand revision.
fn commit_work_item_write_scope(
    store:&Store,ticket_id:&str,item_id:&str,item_key:&str,
    paths_json:&str,expected_revision:i64,
)->Result<()> {
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let existing:Option<String>=tx.query_row(
        "SELECT allowed_paths FROM sa_work_item_write_scopes WHERE item_id=?1",
        [item_id],|row|row.get(0),
    ).optional().map_err(|e|e.to_string())?;
    if let Some(previous)=existing {
        if previous!=paths_json {
            return Err("A different immutable file scope already belongs to this WorkItem.".into());
        }
    } else {
        let inserted=tx.execute(
            "INSERT INTO sa_work_item_write_scopes(
                item_id,allowed_paths,root_ticket_revision,authorized_at)
             SELECT item.id,?3,?4,?5
             FROM sa_work_items item
             JOIN sa_demand_plans plan ON plan.ticket_id=item.ticket_id
             WHERE item.ticket_id=?1 AND item.id=?2
               AND item.state='pending' AND item.child_ticket_id IS NULL
               AND item.requires_write=1 AND plan.state='active'
               AND plan.ticket_revision=?4",
            params![ticket_id,item_id,paths_json,expected_revision,storage::now()],
        ).map_err(|e|e.to_string())?;
        if inserted!=1 {
            return Err("WorkItem became ineligible before write scope admission.".into());
        }
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'work_item_write_scope_confirmed',?2,?3)",
            params![ticket_id,format!("WorkItem {item_key}"),storage::now()],
        ).map_err(|e|e.to_string())?;
    }
    tx.commit().map_err(|e|e.to_string())
}

/// A write-required WorkItem remains pending until the local user confirms
/// exactly which paths it may modify. A Coordinator JSON proposal is NEVER a
/// file-write grant. Only the native Codex ToolHost can execute this scope.
#[tauri::command]
pub async fn superagent_authorize_work_item_write(
    window:tauri::WebviewWindow,app:tauri::AppHandle,state:State<'_,AppState>,
    ticket_id:String,expected_revision:i64,item_key:String,allowed_paths:Vec<String>,
)->Result<DemandPlan> {
    super::require_main_window(&window)?;
    if !crate::workspace::scoped_writes_supported() {
        return Err("policy_unsupported: scoped file writes are not supported on this platform.".into());
    }
    super::write_grants::validate_paths(&allowed_paths)?;
    let confirm_preflight=|store:&Store|->Result<()> {
        let root=get_ticket(store,&ticket_id)?;
        let plan=inspect(store,&ticket_id)?.ok_or("Demand plan not found.")?;
        if root.revision!=expected_revision || root.state!="queued" ||
            plan.state!="active" || plan.ticket_revision!=expected_revision {
            return Err("Demand or plan changed before file-write authorization.".into());
        }
        let item=plan.items.iter().find(|item|item.key==item_key)
            .ok_or("WorkItem key does not belong to this Demand.")?;
        if !item.requires_write || item.state!="pending" || item.child_ticket_id.is_some() {
            return Err("WorkItem is not waiting for a new file-write authorization.".into());
        }
        let snapshot=ticket_team_snapshot(store,&ticket_id)?;
        let current=configuration(store,&root.orchestration_id)?;
        if crate::feature_controls::get().bots_paused || !current.enabled ||
            current.team_version!=snapshot.team_version ||
            current.policy_revision!=snapshot.policy.policy_revision {
            return Err("Team changed or bots paused before scoped write authorization.".into());
        }
        let root_mandate:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_mandates mandate
               JOIN sa_tickets ticket ON ticket.id=mandate.ticket_id
               WHERE ticket.id=?1 AND ticket.state='queued'
                 AND ticket.active_assignment_id IS NULL
                 AND ticket.actor='local-user' AND ticket.source='manual'
                 AND mandate.actor=ticket.actor AND mandate.revoked=0
                 AND mandate.expires_at>?2 AND mandate.policy_revision=?3
                 AND mandate.project_id=?4 AND mandate.allowed_tools='[]'
                 AND mandate.allowed_side_effects=0)",
            params![ticket_id,storage::now(),snapshot.policy.policy_revision,
                snapshot.project_id],
            |row|row.get(0),
        ).map_err(|e|e.to_string())?;
        if !root_mandate || !super::auto::ticket_authorized(store,&ticket_id)? {
            return Err("Original Demand mandate or GitHub Auto grant is revoked or expired.".into());
        }
        super::roster::require_current_profile(
            store,&ticket_id,"worker",&item.worker_bot_id,
        )?;
        let worker=bots::get(store,&item.worker_bot_id)?;
        if worker.provider!="codex" || worker.paused ||
            worker.project_id!=snapshot.project_id {
            return Err("Only an eligible existing Codex Worker may receive a scoped write grant.".into());
        }
        let root_path=store.workspace_root(&snapshot.project_id,None)?;
        super::write_grants::validate_paths_in_root(&root_path,&allowed_paths)
    };
    {
        let store=state.store.lock().map_err(|error|error.to_string())?;
        confirm_preflight(&store)?;
    }
    use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
    let paths=allowed_paths.iter().map(|path|format!("  - {path}")).collect::<Vec<_>>().join("\n");
    let dialog=window.dialog()
        .message(format!(
            "Authorize this Demand WorkItem to modify ONLY these exact project files?\n\n\
             Demand: {ticket_id}\nWorkItem: {item_key}\n\n{paths}\n\n\
             Provider-native shell, all other files and external effects remain denied. \
             This one-time authorization is bound to the child Ticket and expires on Stop."
        ))
        .title("Authorize WorkItem file writes")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Authorize exact paths".into(),"Cancel".into()
        ));
    #[cfg(desktop)]
    let dialog=dialog.parent(&window);
    let (send,receive)=tokio::sync::oneshot::channel();
    dialog.show(move |ok|{let _=send.send(ok);});
    if !receive.await.unwrap_or(false) {
        return Err("WorkItem file-write permission was not confirmed.".into());
    }
    let result={
        let store=state.store.lock().map_err(|error|error.to_string())?;
        confirm_preflight(&store)?;
        let original=inspect(&store,&ticket_id)?.ok_or("Demand plan not found.")?;
        let item=original.items.iter().find(|item|item.key==item_key)
            .ok_or("WorkItem not found.")?;
        let paths_json=serde_json::to_string(&allowed_paths).map_err(|error|error.to_string())?;
        commit_work_item_write_scope(
            &store,&ticket_id,&item.id,&item_key,&paths_json,expected_revision,
        )?;
        inspect(&store,&ticket_id)?.ok_or_else(||"Demand plan disappeared after write scope.".to_owned())?
    };
    advance(&app).await?;
    Ok(result)
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CoordinatorProposal {
    work_items:Vec<WorkItemInput>,
}

/// Called after the native Coordinator callback has settled, never before.
/// Parse only the exact structured read-only plan previously opted into by the
/// local user. An invalid plan is recorded once and never retried in a loop.
pub(crate) fn capture_coordinator_plans(store:&Store)->Result<usize> {
    let roots={
        let mut statement=store.db.prepare(
            "SELECT t.id,t.revision,t.evidence FROM sa_tickets t
             WHERE t.state='queued' AND t.evidence IS NOT NULL
               AND EXISTS(SELECT 1 FROM sa_audit audit
                 WHERE audit.ticket_id=t.id AND audit.kind='demand_auto_plan_requested')
               AND NOT EXISTS(SELECT 1 FROM sa_demand_plans plan
                 WHERE plan.ticket_id=t.id)
               AND NOT EXISTS(SELECT 1 FROM sa_audit audit
                 WHERE audit.ticket_id=t.id AND audit.kind IN (
                    'demand_auto_plan_accepted','demand_auto_plan_rejected'))
             ORDER BY t.created_at,t.id LIMIT 16"
        ).map_err(|error|error.to_string())?;
        let rows=statement.query_map([],|row|Ok((
            row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,
        ))).map_err(|error|error.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|error|error.to_string())?
    };
    let mut accepted=0;
    for (id,revision,evidence) in roots {
        let outcome=serde_json::from_str::<CoordinatorProposal>(&evidence)
            .map_err(|_|"Coordinator did not produce a valid structured plan.".to_owned())
            .and_then(|proposal|create_plan(store,&id,revision,&proposal.work_items));
        let (kind,detail)=match outcome {
            Ok(_) => {accepted+=1;("demand_auto_plan_accepted","Validated native Worker DAG")},
            Err(_) => ("demand_auto_plan_rejected",
                "Invalid or unauthorized Coordinator proposal; use explicit plan correction"),
        };
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,?2,?3,?4)",
            params![id,kind,detail,storage::now()],
        ).map_err(|error|error.to_string())?;
    }
    Ok(accepted)
}


/// Run native recovery and dependency progression in both the desktop app and
/// \`pandacode serve\`. This does not introduce an autonomous source poll, new
/// tool grant or external effect: only already-authorized local-user Demands
/// with verifiable outstanding WorkItems are eligible.
pub(crate) fn start_native(app:tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // AppState already exists before this service is installed. Allow
        // the native bot/gateway service to finish the rest of setup.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let mut last_error=String::new();
        loop {
            let state=app.state::<AppState>();
            if state.shutdown.load(std::sync::atomic::Ordering::SeqCst)!=0 {
                break;
            }
            let capture={
                match state.store.lock() {
                    Ok(store)=>capture_coordinator_plans(&store),
                    Err(error)=>Err(error.to_string()),
                }
            };
            let progressed=match capture {
                Ok(_)=>advance(&app).await,
                Err(error)=>Err(error),
            };
            match progressed {
                Ok(_) => last_error.clear(),
                Err(error) if last_error!=error => {
                    eprintln!("SuperAgent native Demand recovery: {error}");
                    last_error=error;
                }
                Err(_)=>{}
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

/// Trusted one-shot headless inspection does not open a mutable Store; opening
/// a live host's database must not run startup settlement or provider replay.
pub(crate) fn headless_inspect_plan(
    database_path:&std::path::Path,ticket_id:&str,
)->Result<String> {
    let store=Store::open_read_only(database_path)?;
    let plan=inspect(&store,ticket_id)?;
    serde_json::to_string(&serde_json::json!({
        "ticketId":ticket_id,
        "plan":plan,
        "readOnly":true,
    })).map_err(|error|error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(key:&str,deps:&[&str])->WorkItemInput {
        WorkItemInput{key:key.into(),objective:format!("Implement {key}"),
            worker_bot_id:"worker".into(),depends_on:deps.iter().map(|s|s.to_string()).collect(),
            requires_write:false}
    }
    #[test]
    fn work_item_graph_rejects_unknown_cycles_duplicates_and_unbounded_intent() {
        assert!(validate_items(&[item("first",&[]),item("second",&["first"])]).is_ok());
        for values in [
            vec![item("first",&["missing"])],
            vec![item("first",&["second"]),item("second",&["first"])],
            vec![item("first",&[]),item("first",&[])],
            vec![item("first",&["first"])],
            vec![],
        ] {
            assert!(validate_items(&values).is_err(),"{values:?}");
        }
        let mut oversized=item("long",&[]);
        oversized.objective="x".repeat(MAX_ITEM_OBJECTIVE+1);
        assert!(validate_items(&[oversized]).is_err());
    }
    #[test]
    fn write_scope_requires_real_local_confirmation_and_is_immutable() {
        let direct: WorkItemInput = serde_json::from_value(serde_json::json!({
            "key":"implement","objective":"Scoped change","workerBotId":"worker",
            "dependsOn":[],"requiresWrite":true
        })).unwrap();
        assert!(direct.requires_write);
        assert!(!item("readonly",&[]).requires_write);
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;
             CREATE TABLE sa_tickets(id TEXT PRIMARY KEY);").unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch("ALTER TABLE sa_work_items ADD COLUMN requires_write INTEGER
            NOT NULL DEFAULT 0 CHECK(requires_write IN (0,1));").unwrap();
        db.execute_batch(WRITE_SCOPES_SCHEMA).unwrap();
        db.execute("INSERT INTO sa_tickets(id) VALUES('root')",[]).unwrap();
        db.execute(
            "INSERT INTO sa_demand_plans(ticket_id,ticket_revision,team_version,
             policy_revision,plan_sha256,state,created_at,updated_at)
             VALUES('root',1,1,1,?1,'active',1,1)",
            ["0".repeat(64)],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,
             worker_bot_id,state,created_at,updated_at,requires_write)
             VALUES('item','root','implement',0,'Scoped change','worker','pending',1,1,1)",
            [],
        ).unwrap();
        assert_eq!(db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_item_write_scopes WHERE item_id='item')",
            [],|r|r.get::<_,i64>(0),
        ).unwrap(),0,"a model-requested file change is not preauthorized");
        db.execute(
            "INSERT INTO sa_work_item_write_scopes(item_id,allowed_paths,
             root_ticket_revision,authorized_at) VALUES('item',?1,1,2)",
            [r#"["src/allowed.rs"]"#],
        ).unwrap();
        assert!(db.execute(
            "UPDATE sa_work_item_write_scopes SET allowed_paths=?1 WHERE item_id='item'",
            [r#"["src/outside.rs"]"#],
        ).is_err(),"a confirmed scope cannot be silently widened");
        assert!(db.execute(
            "DELETE FROM sa_work_item_write_scopes WHERE item_id='item'",[],
        ).is_err(),"the authorization audit must outlive Stop");
    }

    #[test]
    fn scoped_write_grant_and_audit_are_atomic_and_idempotent() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE sa_tickets(id TEXT PRIMARY KEY);
             CREATE TABLE sa_audit(ticket_id TEXT,kind TEXT,detail TEXT,at INTEGER);
             INSERT INTO sa_tickets(id) VALUES('root');",
        ).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch(
            "ALTER TABLE sa_work_items ADD COLUMN requires_write INTEGER
             NOT NULL DEFAULT 0 CHECK(requires_write IN (0,1));",
        ).unwrap();
        db.execute_batch(WRITE_SCOPES_SCHEMA).unwrap();
        db.execute(
            "INSERT INTO sa_demand_plans(ticket_id,ticket_revision,team_version,
             policy_revision,plan_sha256,state,created_at,updated_at)
             VALUES('root',2,1,1,?1,'active',1,1)",
            ["0".repeat(64)],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,
             worker_bot_id,state,created_at,updated_at,requires_write)
             VALUES('item','root','implement',0,'Scoped change','worker','pending',1,1,1)",
            [],
        ).unwrap();
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        store.db.execute_batch(
            "CREATE TRIGGER reject_scope_audit BEFORE INSERT ON sa_audit
             BEGIN SELECT RAISE(ABORT,'audit write rejected'); END;",
        ).unwrap();
        assert!(commit_work_item_write_scope(
            &store,"root","item","implement",r#"["src/allowed.rs"]"#,2,
        ).is_err());
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_work_item_write_scopes",[],
            |r|r.get::<_,i64>(0),
        ).unwrap(),0,"a failed audit must roll back the executable scope");
        store.db.execute_batch("DROP TRIGGER reject_scope_audit;").unwrap();
        for _ in 0..2 {
            commit_work_item_write_scope(
                &store,"root","item","implement",r#"["src/allowed.rs"]"#,2,
            ).unwrap();
        }
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit",[],|r|r.get::<_,i64>(0),
        ).unwrap(),1,"an identical replay must not create a second audit");
        assert!(commit_work_item_write_scope(
            &store,"root","item","implement",r#"["src/other.rs"]"#,2,
        ).is_err(),"a replay cannot broaden the confirmed scope");
    }

    #[test]
    fn work_item_digest_is_stable_and_depends_on_exact_requested_plan() {
        let first=vec![item("compile",&[]),item("test",&["compile"])];
        assert_eq!(sha256(&first).unwrap(),sha256(&first).unwrap());
        let changed=vec![item("compile",&[]),item("test",&[])];
        assert_ne!(sha256(&first).unwrap(),sha256(&changed).unwrap());
    }
}
