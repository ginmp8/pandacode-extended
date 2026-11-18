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
pub(crate) mod test_runner;
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


/// An initial local confirmation authorizes only these exact files for
/// write-required items of this ONE Demand. It never makes its Coordinator
/// writable and never expands the native read-only root Mandate.
pub(crate) const DEMAND_WRITE_SCOPES_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_demand_write_scopes (
    ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    team_version INTEGER NOT NULL CHECK(team_version>0),
    policy_revision INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    checkout_root TEXT NOT NULL,
    allowed_paths TEXT NOT NULL CHECK(json_valid(allowed_paths)),
    authorized_at INTEGER NOT NULL
);
CREATE TRIGGER IF NOT EXISTS sa_demand_write_scope_immutable
 BEFORE UPDATE ON sa_demand_write_scopes
 BEGIN SELECT RAISE(ABORT,'Demand file scope cannot be modified'); END;
CREATE TRIGGER IF NOT EXISTS sa_demand_write_scope_no_delete
 BEFORE DELETE ON sa_demand_write_scopes
 BEGIN SELECT RAISE(ABORT,'Demand file scope is durable audit evidence'); END;
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
    /// Untrusted requested subset of a confirmed Demand file scope, never a grant.
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub write_paths: Vec<String>,
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
        if !item.requires_write && !item.write_paths.is_empty() {
            return Err("Read-only WorkItems cannot request file paths.".into());
        }
        if !item.write_paths.is_empty() {
            super::write_grants::validate_paths(&item.write_paths)?;
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

/// Child grants must be narrowed explicitly; a missing subset fails closed.
fn scoped_child_paths(initial:&[String],requested:&[String])->Result<String> {
    super::write_grants::validate_paths(requested)?;
    let allowed=initial.iter().map(String::as_str).collect::<HashSet<_>>();
    if requested.iter().any(|path|!allowed.contains(path.as_str())) {
        return Err("WorkItem requested a file outside the initial Demand scope.".into());
    }
    serde_json::to_string(requested).map_err(|e|e.to_string())
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

/// A model-created plan cannot use a manual Ticket whose locally issued
/// Mandate is blank, foreign-owned, revoked, expired or broader than read-only.
fn manual_demand_mandate_current(
    store:&Store,ticket_id:&str,policy_revision:i64,project_id:&str,
)->Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_mandates m
         JOIN sa_tickets t ON t.id=m.ticket_id
         WHERE t.id=?1 AND t.actor='local-user' AND t.source='manual'
         AND m.actor=t.actor AND length(trim(m.authorization_id))>0
         AND m.revoked=0 AND m.expires_at>?2 AND m.policy_revision=?3
         AND m.project_id=?4 AND m.allowed_tools='[]' AND m.allowed_side_effects=0
         AND EXISTS(SELECT 1 FROM sa_audit a WHERE a.ticket_id=t.id
           AND a.kind='admitted' AND a.detail='manual readonly coordinator reserved'))",
        params![ticket_id,storage::now(),policy_revision,project_id],
        |row|row.get(0),
    ).map_err(|error|error.to_string())
}


/// Bind an explicit local confirmation to the already issued read-only
/// Coordinator Ticket. No model callback or imported Team can call this path.
/// Failure is fail-closed: the caller must not deliver the Coordinator turn.
pub(in crate::superagent) fn issue_initial_demand_write_scope(
    store:&Store,ticket_id:&str,allowed_paths:&[String],
    expected_team_version:i64,expected_policy_revision:i64,
    expected_project_id:&str,
)->Result<()> {
    let snapshot=ticket_team_snapshot(store,ticket_id)?;
    if snapshot.team_version!=expected_team_version ||
        snapshot.policy.policy_revision!=expected_policy_revision ||
        snapshot.project_id!=expected_project_id {
        return Err("The original Team changed before initial file authorization.".into());
    }
    let root=store.workspace_root(expected_project_id,None)?;
    super::write_grants::validate_paths_in_root(&root,allowed_paths)?;
    let root_text=root.to_string_lossy().to_string();
    let encoded=serde_json::to_string(allowed_paths).map_err(|e|e.to_string())?;
    let now=storage::now();
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let inserted=tx.execute(
        "INSERT INTO sa_demand_write_scopes(ticket_id,team_version,policy_revision,
            project_id,checkout_root,allowed_paths,authorized_at)
         SELECT ticket.id,?2,?3,?4,?5,?6,?7 FROM sa_tickets ticket
         JOIN sa_mandates mandate ON mandate.ticket_id=ticket.id
         WHERE ticket.id=?1 AND ticket.actor='local-user' AND ticket.source='manual'
           AND ticket.state='running' AND ticket.active_assignment_id IS NOT NULL
           AND ticket.policy_revision=?3 AND ticket.project_id=?4
           AND mandate.actor=ticket.actor AND length(trim(mandate.authorization_id))>0
           AND mandate.revoked=0 AND mandate.expires_at>?7
           AND mandate.policy_revision=?3 AND mandate.project_id=?4
           AND mandate.allowed_tools='[]' AND mandate.allowed_side_effects=0",
        params![ticket_id,expected_team_version,expected_policy_revision,
            expected_project_id,root_text,encoded,now],
    ).map_err(|e|e.to_string())?;
    if inserted!=1 {
        return Err("Original read-only Demand authorization changed before consent was recorded.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'demand_initial_write_scope_confirmed',?2,?3)",
        params![ticket_id,format!("{} exact local-user-approved files; future WorkItems remain separate grants",
            allowed_paths.len()),now],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}

/// Inherit confirmed scope only as an immutable, item-specific permission
/// inside the SAME transaction that inserts the validated WorkItem DAG.
fn inherit_initial_demand_paths(
    db:&rusqlite::Connection,ticket_id:&str,item_id:&str,item_key:&str,
    expected_revision:i64,paths_json:&str,now:i64,
)->Result<()> {
    let inserted=db.execute(
        "INSERT INTO sa_work_item_write_scopes(item_id,allowed_paths,root_ticket_revision,authorized_at)
         SELECT item.id,?4,?5,?6
         FROM sa_work_items item JOIN sa_demand_plans plan ON plan.ticket_id=item.ticket_id
         WHERE item.ticket_id=?1 AND item.id=?2 AND item.item_key=?3
           AND item.requires_write=1 AND item.state='pending'
           AND item.child_ticket_id IS NULL AND plan.state='active'
           AND plan.ticket_revision=?5",
        params![ticket_id,item_id,item_key,paths_json,expected_revision,now],
    ).map_err(|e|e.to_string())?;
    if inserted!=1 { return Err("Initial Demand scope could not bind the exact WorkItem.".into()); }
    db.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_item_write_scope_inherited',?2,?3)",
        params![ticket_id,format!("WorkItem {item_key}; exact initial Demand scope"),now],
    ).map_err(|e|e.to_string())?;
    Ok(())
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
    let authorized=manual_demand_mandate_current(
        store,ticket_id,snapshot.policy.policy_revision,&snapshot.project_id,
    )?;
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

    // A one-time Demand scope is separate from the read-only root Mandate.
    // Its exact checkout and pinned Team must still match at plan settlement.
    let initial_scope:Option<(String,i64,i64,String,String)>=store.db.query_row(
        "SELECT allowed_paths,team_version,policy_revision,project_id,checkout_root
         FROM sa_demand_write_scopes WHERE ticket_id=?1",
        [ticket_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,
            row.get(3)?,row.get(4)?)),
    ).optional().map_err(|e|e.to_string())?;
    let initial_paths=if let Some((encoded,team_version,policy_revision,project_id,checkout_root))=
        initial_scope {
        let root=store.workspace_root(&snapshot.project_id,None)?;
        if team_version!=snapshot.team_version ||
            policy_revision!=snapshot.policy.policy_revision ||
            project_id!=snapshot.project_id ||
            checkout_root!=root.to_string_lossy().into_owned() ||
            !crate::workspace::scoped_writes_supported() {
            return Err("Initial Demand file scope no longer matches its immutable Team or checkout.".into());
        }
        let paths:Vec<String>=serde_json::from_str(&encoded)
            .map_err(|_|"Stored initial Demand file scope is malformed.")?;
        super::write_grants::validate_paths_in_root(&root,&paths)?;
        for item in items.iter().filter(|item|item.requires_write) {
            scoped_child_paths(&paths,&item.write_paths)?;
            if bots::get(store,&item.worker_bot_id)?.provider!="codex" {
                return Err("policy_unsupported: the selected write WorkItem needs a native Codex Worker.".into());
            }
        }
        Some(paths)
    } else {
        if items.iter().any(|item|!item.write_paths.is_empty()) {
            return Err("WorkItem file paths require an explicitly authorized initial Demand scope.".into());
        }
        None
    };
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
        let item_id=storage::id();
        tx.execute(
            "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,worker_bot_id,
                requires_write,state,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,'pending',?8,?8)",
            params![item_id,ticket_id,item.key,ordinal as i64,item.objective.trim(),
                item.worker_bot_id,item.requires_write,now],
        ).map_err(|error|error.to_string())?;
        if item.requires_write {
            if let Some(authorized)=initial_paths.as_deref() {
                let child_json=scoped_child_paths(authorized,&item.write_paths)?;
                inherit_initial_demand_paths(&tx,ticket_id,&item_id,&item.key,
                    revision,&child_json,now)?;
            }
        }
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
        params![ticket_id,format!("{} WorkItems ({} request local file authorization); plan digest {}",
                items.len(),items.iter().filter(|item|item.requires_write).count(),digest),now],
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


/// Build a read-only Coordinator request. The optional requiresWrite flag is
/// untrusted intent that must pass the later local-user confirmation gate.
fn coordinator_demand_prompt(ticket_id:&str,objective:&str,workers:&[String],initial_paths:&[String])->String {
    let path_guidance=if initial_paths.is_empty() {
        "No initial file scope was authorized. Leave writePaths empty; write-required WorkItems must await separate native authorization.".to_string()
    } else {
        format!("The operator approved ONLY these project files for this Demand:\n- {}\nFor each requiresWrite=true item, select a NONEMPTY writePaths subset of that list. Never request another file.",initial_paths.join("\n- "))
    };
    let selected=workers.iter().map(|id|format!("- {id}"))
        .collect::<Vec<_>>().join("\n");
    let base=format!(
        "SuperAgent Demand {}\n\n{}\n\n\
         Output a SINGLE JSON object with one field, workItems, containing 1 to 32 \
         objects with key, objective, workerBotId, dependsOn (array of prerequisite keys) \
         and requiresWrite (boolean), writePaths (array of exact project-relative files). Use ONLY these existing Worker IDs:\n{}\n\n\
         Keep every WorkItem inside the admitted user objective. Set requiresWrite=true \
         ONLY when editing source files is necessary. This flag NEVER authorizes any write. \
         Exact native file rights may come only from an already confirmed initial Demand \
         scope or a later local-user confirmation; a model cannot extend either grant. \
         You are planning read-only: do not modify files, run shell commands or call \
         external tools. Workers may only use a separate native scoped-file ToolHost \
         after exact path confirmation, and must never infer shell, MCP, GitHub or \
         Council privileges from this plan. Every child receives a new native \
         Ticket, session, independent policy check and completion gate. \
         Output valid JSON only, no Markdown or commentary.",
        ticket_id,objective,selected,
    );
    format!("{base}\n\n{path_guidance}")
}

/// An already used Demand request key is not fresh user consent. Compare the
/// original immutable scope before prompting or accepting an idempotent replay.
fn replayed_demand_scope_confirmed(
    db:&rusqlite::Connection,team_id:&str,objective:&str,request_key:&str,
    requested:Option<(i64,i64,&str,&str,&[String])>,
)->Result<bool> {
    let old:Option<(String,String,String,String)>=db.query_row(
        "SELECT id,objective,actor,source FROM sa_tickets
         WHERE orchestration_id=?1 AND request_key=?2",
        params![team_id,request_key],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((id,original,actor,source))=old else {return Ok(false)};
    if original!=objective.trim() || actor!="local-user" || source!="manual" {
        return Err("Demand request key cannot change actor, source or objective.".into());
    }
    let saved:Option<(i64,i64,String,String,String)>=db.query_row(
        "SELECT team_version,policy_revision,project_id,checkout_root,allowed_paths
         FROM sa_demand_write_scopes WHERE ticket_id=?1",
        [&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
    ).optional().map_err(|e|e.to_string())?;
    match (saved,requested) {
        (None,None)=>Ok(true),
        (Some((version,revision,project,checkout,paths)),
         Some((v,r,p,c,wanted)))=>{
            let exact=serde_json::to_string(wanted).map_err(|e|e.to_string())?;
            if version==v && revision==r && project==p && checkout==c && paths==exact {
                Ok(true)
            } else {
                Err("Demand request-key replay cannot change previously authorized file scope.".into())
            }
        }
        _=>Err("Demand request-key replay cannot add, omit or remove original file consent.".into()),
    }
}

/// A single initial operator action starts the configured Team Coordinator.
/// Its JSON plan is strictly validated and may only schedule read-only native
/// WorkItems for the Workers in the immutable Team; no model grants effects.
#[tauri::command]
pub async fn superagent_start_demand(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    orchestration_id:String,objective:String,request_key:String,
    allowed_paths:Option<Vec<String>>,allowed_test:Option<String>,
)->Result<super::Ticket> {
    super::require_main_window(&window)?;
    let app=window.app_handle().clone();
    if crate::feature_controls::get().bots_paused {
        return Err("Bots are globally paused; Demand admission denied.".into());
    }

    // This is a trusted main-window local-user operation. Confirm ALL paths
    // before allocating any Coordinator session. No later model instruction
    // can widen this finite exact-file scope or acquire shell/MCP rights.
    let paths=allowed_paths.unwrap_or_default();
    let test=allowed_test.filter(|cmd|!cmd.trim().is_empty());
    let initial=if paths.is_empty() {None} else {
        if !crate::workspace::scoped_writes_supported() {
            return Err("policy_unsupported: initial Demand writes are not supported on this platform.".into());
        }
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let config=configuration(&store,&orchestration_id)?;
        if !config.enabled {return Err("Team must be enabled before initial file authorization.".into());}
        let checkout=store.workspace_root(&config.project_id,None)?;
        super::write_grants::validate_paths_in_root(&checkout,&paths)?;
        let mut codex_worker=false;
        for worker_id in &config.worker_bot_ids {
            let worker=bots::get(&store,worker_id)?;
            if worker.provider=="codex" && !worker.paused && worker.project_id==config.project_id {
                codex_worker=true;
            }
        }
        if !codex_worker {
            return Err("policy_unsupported: no eligible native Codex Worker can use this exact file scope.".into());
        }
        Some((config.team_version,config.policy_revision,config.project_id.clone()))
    };
    // A native test process is explicitly selected before Coordinator dispatch,
    // separately from the immutable exact-file grant for Workers.
    let test_initial=if let Some(command)=test.as_deref() {
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let config=configuration(&store,&orchestration_id)?;
        if !config.enabled {return Err("Team is disabled; tests cannot be authorized.".into());}
        let root=store.workspace_root(&config.project_id,None)?;
        let canonical=std::fs::canonicalize(&root)
            .map_err(|_|"The Demand test checkout is unavailable.")?;
        let manifest=test_runner::manifest_hash(&canonical,command)?;
        Some((config.team_version,config.policy_revision,config.project_id.clone(),
            canonical.to_string_lossy().into_owned(),manifest))
    } else {None};
    // Full Access is a native Worker permission. Tests may run using the
    // selected bot's ordinary tools; a separate test-command grant is not
    // required to authorize the Team or start a Demand.
    let prior_consent={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let checkout=initial.as_ref().map(|(_,_,project)|
            store.workspace_root(project,None).map(|root|root.to_string_lossy().into_owned())
        ).transpose()?;
        let files=replayed_demand_scope_confirmed(
            &store.db,&orchestration_id,&objective,&request_key,
            initial.as_ref().map(|(v,r,p)|(
                *v,*r,p.as_str(),checkout.as_deref().unwrap_or(""),paths.as_slice(),
            )),
        )?;
        let tests=test_runner::replay_matches(&store.db,&orchestration_id,
            &request_key,test_initial.as_ref().map(|(v,p,project,root,hash)|
                (*v,*p,project.as_str(),root.as_str(),
                 test.as_deref().unwrap_or(""),hash.as_str())))?;
        files && tests
    };
    if (initial.is_some() || test_initial.is_some()) && !prior_consent {
        use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
        let names=paths.iter().map(|path|format!("  - {path}")).collect::<Vec<_>>().join("\n");
        let tests=test.as_deref().unwrap_or("No native test process");
        let dialog=window.dialog()
            .message(format!(
                "Authorize ONE Demand for exactly the following scoped effects?\n\n\
                 Team: {orchestration_id}\nFiles:\n{names}\n\nNative test: {tests}\n\n\
                 The test command executes declared project code, with a 120-second \
                 deadline and offline package mode. No arbitrary argv, general shell, \
                 MCP, GitHub effects, additional files or Council approvals are granted. \
                 Stop, expiry and policy/manifest changes revoke acceptance."
            ))
            .title("Authorize scoped Demand files and tests")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Authorize exact Demand scope".into(),"Cancel".into(),
            ));
        #[cfg(desktop)]
        let dialog=dialog.parent(&window);
        let (send,receive)=tokio::sync::oneshot::channel();
        dialog.show(move |ok|{let _=send.send(ok);});
        if !receive.await.unwrap_or(false) {
            return Err("Initial Demand file-write scope was not confirmed.".into());
        }
    }
    let (ticket,launch,coordinator_id,workers)={
        let active=state.agents.lock().map_err(|error|error.to_string())?
            .keys().cloned().collect::<Vec<_>>();
        let mut store=state.store.lock().map_err(|error|error.to_string())?;
        let config=configuration(&store,&orchestration_id)?;
        if let Some((team_version,policy_revision,project_id))=&initial {
            if config.team_version!=*team_version ||
                config.policy_revision!=*policy_revision || config.project_id!=*project_id {
                return Err("Team changed while the initial Demand confirmation was open.".into());
            }
            let checkout=store.workspace_root(&config.project_id,None)?;
            super::write_grants::validate_paths_in_root(&checkout,&paths)?;
        }
        let busy=super::active_manual_conflict(&store,&active,
            &config.coordinator_bot_id,&config.project_id)?;
        let (ticket,launch)=super::admit_role_with_selected(
            &mut store,&orchestration_id,&objective,&request_key,busy,true,false,
            Some(&config.worker_bot_id),
        )?;
        if let Some((team_version,policy_revision,project_id))=&initial {
            if launch {
                if let Err(error)=issue_initial_demand_write_scope(
                    &store,&ticket.id,&paths,*team_version,*policy_revision,project_id
                ) {
                    let _=super::fail_launch(&store,&ticket.id,&error);
                    return Err(format!(
                        "Initial file consent was not recorded; Coordinator dispatch denied for {}: {error}",
                        ticket.id,
                    ));
                }
            } else {
                let saved:Option<String>=store.db.query_row(
                    "SELECT allowed_paths FROM sa_demand_write_scopes WHERE ticket_id=?1",
                    [&ticket.id],|r|r.get(0),
                ).optional().map_err(|e|e.to_string())?;
                let exact=serde_json::to_string(&paths).map_err(|e|e.to_string())?;
                if saved.as_deref()!=Some(exact.as_str()) {
                    return Err("Demand request-key replay cannot change the initial write scope.".into());
                }
            }
        }
        if let Some((version,policy,project,checkout,hash))=&test_initial {
            if launch {
                let current=std::fs::canonicalize(store.workspace_root(project,None)?)
                    .map_err(|_|"The authorized test checkout changed.")?;
                if current.to_string_lossy()!=checkout.as_str() ||
                    test_runner::manifest_hash(&current,test.as_deref().unwrap_or(""))?!=*hash {
                    let _=super::fail_launch(&store,&ticket.id,"Test checkout or manifest changed.");
                    return Err("Initial test scope changed before Coordinator dispatch.".into());
                }
                if let Err(error)=test_runner::issue(&store,&ticket.id,
                    test_runner::TestGrant{
                        team_version:*version,policy_revision:*policy,
                        project_id:project,checkout_root:checkout,
                        command:test.as_deref().unwrap_or(""),
                        manifest_sha256:hash,
                    },
                ) {
                    let _=super::fail_launch(&store,&ticket.id,&error);
                    return Err(format!("Initial test authorization failed for {}: {error}",ticket.id));
                }
            }
        }
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
    let delivered=bots::gateway::deliver(&app,bots::gateway::Inbound{
        bot_id:coordinator_id,
        channel_id:super::CHANNEL.into(),
        thread:ticket.id.clone(),
        sender_id:"local-user".into(),
        sender_name:"Authorized Team Demand".into(),
        text:coordinator_demand_prompt(&ticket.id,&ticket.objective,&workers,&paths),
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
fn commit_work_item_write_scopes(
    store:&Store,ticket_id:&str,items:&[(&str,&str)],
    paths_json:&str,expected_revision:i64,
)->Result<()> {
    if items.is_empty() {
        return Err("No pending WorkItems were selected for file authorization.".into());
    }
    // All WorkItem scopes and audit receipts commit together, or none do.
    // A model plan cannot inject an item belonging to another Demand.
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    for &(item_id,item_key) in items {
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
                   AND plan.ticket_revision=?4 AND item.item_key=?6",
                params![ticket_id,item_id,paths_json,expected_revision,storage::now(),item_key],
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
    }
    tx.commit().map_err(|e|e.to_string())
}

fn commit_work_item_write_scope(
    store:&Store,ticket_id:&str,item_id:&str,item_key:&str,
    paths_json:&str,expected_revision:i64,
)->Result<()> {
    commit_work_item_write_scopes(
        store,ticket_id,&[(item_id,item_key)],paths_json,expected_revision,
    )
}

fn preflight_work_item_write(
    store:&Store,ticket_id:&str,expected_revision:i64,item_key:&str,
    allowed_paths:&[String],
)->Result<()> {
        let root=get_ticket(store,ticket_id)?;
        let plan=inspect(store,ticket_id)?.ok_or("Demand plan not found.")?;
        if root.revision!=expected_revision || root.state!="queued" ||
            plan.state!="active" || plan.ticket_revision!=expected_revision {
            return Err("Demand or plan changed before file-write authorization.".into());
        }
        let item=plan.items.iter().find(|item|item.key==item_key)
            .ok_or("WorkItem key does not belong to this Demand.")?;
        if !item.requires_write || item.state!="pending" || item.child_ticket_id.is_some() {
            return Err("WorkItem is not waiting for a new file-write authorization.".into());
        }
        let snapshot=ticket_team_snapshot(store,ticket_id)?;
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
        if !root_mandate || !super::auto::ticket_authorized(store,ticket_id)? {
            return Err("Original Demand mandate or GitHub Auto grant is revoked or expired.".into());
        }
        super::roster::require_current_profile(
            store,ticket_id,"worker",&item.worker_bot_id,
        )?;
        let worker=bots::get(store,&item.worker_bot_id)?;
        if worker.provider!="codex" || worker.paused ||
            worker.project_id!=snapshot.project_id {
            return Err("Only an eligible existing Codex Worker may receive a scoped write grant.".into());
        }
        let root_path=store.workspace_root(&snapshot.project_id,None)?;
        super::write_grants::validate_paths_in_root(&root_path,allowed_paths)

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
    {
        let store=state.store.lock().map_err(|error|error.to_string())?;
        preflight_work_item_write(&store,&ticket_id,expected_revision,&item_key,&allowed_paths)?;
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
        preflight_work_item_write(&store,&ticket_id,expected_revision,&item_key,&allowed_paths)?;
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

/// Authorize multiple still-pending write-required WorkItems in one explicit
/// local confirmation. All receive the SAME exact paths displayed in the
/// dialog; no Coordinator proposal can grant or extend this scope.
/// Neither a partial insert nor a stale Demand may dispatch any Worker.
#[tauri::command]
pub async fn superagent_authorize_demand_work_item_writes(
    window:tauri::WebviewWindow,app:tauri::AppHandle,state:State<'_,AppState>,
    ticket_id:String,expected_revision:i64,allowed_paths:Vec<String>,
)->Result<DemandPlan> {
    super::require_main_window(&window)?;
    if !crate::workspace::scoped_writes_supported() {
        return Err("policy_unsupported: scoped file writes are not supported on this platform.".into());
    }
    super::write_grants::validate_paths(&allowed_paths)?;
    let pending=|store:&Store|->Result<Vec<String>> {
        let plan=inspect(store,&ticket_id)?.ok_or("Demand plan not found.")?;
        if plan.ticket_revision!=expected_revision || plan.state!="active" {
            return Err("Demand plan changed before file-write authorization.".into());
        }
        let keys=plan.items.iter()
            .filter(|item|item.requires_write && !item.write_authorized &&
                item.state=="pending" && item.child_ticket_id.is_none())
            .map(|item|item.key.clone()).collect::<Vec<_>>();
        if keys.len()<2 {
            return Err("Batch confirmation needs at least two pending, unauthorized write WorkItems.".into());
        }
        for key in &keys {
            preflight_work_item_write(store,&ticket_id,expected_revision,key,&allowed_paths)?;
        }
        Ok(keys)
    };
    let keys={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        pending(&store)?
    };
    use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
    let paths=allowed_paths.iter().map(|path|format!("  - {path}")).collect::<Vec<_>>().join("\n");
    let names=keys.iter().map(|key|format!("  - {key}")).collect::<Vec<_>>().join("\n");
    let dialog=window.dialog()
        .message(format!(
            "Authorize {count} pending Demand WorkItems to each modify ONLY the SAME exact files?\n\n\
             Demand: {ticket_id}\nWorkItems:\n{names}\n\nExact files granted to EACH WorkItem:\n{paths}\n\n\
             Each WorkItem retains its own immutable child Ticket and file grant. \
             Provider-native shell, other files, MCP and external effects remain denied. \
             Stop or mandate expiry revokes executable access.",
            count=keys.len(),
        ))
        .title("Authorize Demand WorkItem file writes")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Authorize all listed WorkItems".into(),"Cancel".into()
        ));
    #[cfg(desktop)]
    let dialog=dialog.parent(&window);
    let (send,receive)=tokio::sync::oneshot::channel();
    dialog.show(move |ok|{let _=send.send(ok);});
    if !receive.await.unwrap_or(false) {
        return Err("Demand WorkItem file-write permission was not confirmed.".into());
    }
    let result={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        if pending(&store)?!=keys {
            return Err("Pending WorkItems changed during native authorization.".into());
        }
        let current=inspect(&store,&ticket_id)?.ok_or("Demand plan not found.")?;
        let items=current.items.iter().filter(|item|keys.contains(&item.key))
            .map(|item|(item.id.as_str(),item.key.as_str()))
            .collect::<Vec<_>>();
        let paths_json=serde_json::to_string(&allowed_paths).map_err(|e|e.to_string())?;
        commit_work_item_write_scopes(
            &store,&ticket_id,&items,&paths_json,expected_revision,
        )?;
        inspect(&store,&ticket_id)?
            .ok_or_else(||"Demand plan disappeared after file authorization.".to_owned())?
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


// A wake permit survives races with a provider completion. The native
// scheduler remains the only owner of WorkItem dispatch.
static SCHEDULER_WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();

pub(crate) fn wake() {
    SCHEDULER_WAKE.notify_one();
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
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {}
                _ = SCHEDULER_WAKE.notified() => {}
            }
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
            requires_write:false,write_paths:Vec::new()}
    }
    #[test]
    fn replayed_demand_scope_requires_exact_original_native_consent() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
          CREATE TABLE sa_tickets(id TEXT PRIMARY KEY,orchestration_id TEXT,request_key TEXT,
            objective TEXT,actor TEXT,source TEXT);
          CREATE TABLE sa_demand_write_scopes(ticket_id TEXT,team_version INTEGER,
            policy_revision INTEGER,project_id TEXT,checkout_root TEXT,allowed_paths TEXT);
          INSERT INTO sa_tickets VALUES('t','team','key','Implement','local-user','manual');
          INSERT INTO sa_demand_write_scopes VALUES('t',3,7,'project','/root','["src/file.rs"]');
        "#).unwrap();
        let scope=vec!["src/file.rs".to_string()];
        let check=|objective:&str,key:&str,requested:Option<(i64,i64,&str,&str,&[String])>|{
            replayed_demand_scope_confirmed(&db,"team",objective,key,requested)
        };
        assert!(check("Implement","key",Some((3,7,"project","/root",&scope))).unwrap());
        assert!(!check("Implement","new",Some((3,7,"project","/root",&scope))).unwrap());
        assert!(check("Implement","key",None).is_err(),"omitting scope must not reuse it silently");
        assert!(check("Implement","key",
            Some((3,7,"project","/root",&["src/other.rs".to_string()]))).is_err());
        assert!(check("Implement","key",Some((4,7,"project","/root",&scope))).is_err());
        assert!(check("Changed","key",Some((3,7,"project","/root",&scope))).is_err());
        db.execute("UPDATE sa_tickets SET actor='github' WHERE id='t'",[]).unwrap();
        assert!(check("Implement","key",Some((3,7,"project","/root",&scope))).is_err());
    }

    #[test]
    fn coordinator_can_request_write_work_items_but_cannot_grant_paths() {
        let prompt=coordinator_demand_prompt(
            "demand-1","Implement a bounded change",&["worker-a".into(),"worker-b".into()],
            &["src/a.rs".into(),"src/b.rs".into()],
        );
        assert!(prompt.contains("requiresWrite (boolean)"));
        assert!(prompt.contains("requiresWrite=true"));
        assert!(prompt.contains("NEVER authorizes any write"));
        assert!(prompt.contains("native file rights") || prompt.contains("Exact native file rights"));
        assert!(prompt.contains("worker-a"));
        assert!(prompt.contains("worker-b"));
        assert!(!prompt.contains("Plan read-only tasks only"));
    }

    #[test]
    fn coordinator_child_paths_are_strict_nonempty_subsets_of_authorized_demand_files() {
        let approved=vec!["src/a.rs".into(),"src/b.rs".into()];
        let subset=vec!["src/b.rs".into()];
        assert_eq!(scoped_child_paths(&approved,&subset).unwrap(),r#"["src/b.rs"]"#);
        assert!(scoped_child_paths(&approved,&[]).is_err());
        assert!(scoped_child_paths(&approved,&["src/c.rs".into()]).is_err());
        assert!(scoped_child_paths(&approved,&["../src/a.rs".into()]).is_err());
        assert!(scoped_child_paths(&approved,&["src/a.rs".into(),"src/a.rs".into()]).is_err());
        let mut read_only=item("read-only",&[]);
        read_only.write_paths=subset;
        assert!(validate_items(&[read_only]).is_err());
    }
    #[test]
    fn initial_demand_scope_is_immutable_and_item_propagation_rolls_back() {
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
        db.execute_batch(DEMAND_WRITE_SCOPES_SCHEMA).unwrap();
        let allowed=r#"["src/approved.rs"]"#;
        db.execute(
            "INSERT INTO sa_demand_write_scopes(ticket_id,team_version,policy_revision,
             project_id,checkout_root,allowed_paths,authorized_at)
             VALUES('root',1,1,'project','/p',?1,1)",[allowed],
        ).unwrap();
        assert!(db.execute(
            "UPDATE sa_demand_write_scopes SET allowed_paths=?1 WHERE ticket_id='root'",
            [r#"["src/outside.rs"]"#],
        ).is_err(),"initial scope must never widen");
        assert!(db.execute("DELETE FROM sa_demand_write_scopes",[]).is_err());
        db.execute(
            "INSERT INTO sa_demand_plans(ticket_id,ticket_revision,team_version,
             policy_revision,plan_sha256,state,created_at,updated_at)
             VALUES('root',2,1,1,?1,'active',1,1)",["0".repeat(64)],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,
             worker_bot_id,state,created_at,updated_at,requires_write)
             VALUES('item','root','implement',0,'Edit exact file','worker','pending',1,1,1)",[],
        ).unwrap();
        db.execute_batch(
            "CREATE TRIGGER reject_initial_scope_audit BEFORE INSERT ON sa_audit
             BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;",
        ).unwrap();
        {
            let tx=db.unchecked_transaction().unwrap();
            assert!(inherit_initial_demand_paths(&tx,"root","item","implement",2,allowed,2).is_err());
        }
        assert_eq!(db.query_row("SELECT COUNT(*) FROM sa_work_item_write_scopes",
            [],|r|r.get::<_,i64>(0)).unwrap(),0);
        db.execute_batch("DROP TRIGGER reject_initial_scope_audit;").unwrap();
        {
            let tx=db.unchecked_transaction().unwrap();
            inherit_initial_demand_paths(&tx,"root","item","implement",2,allowed,2).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(db.query_row("SELECT allowed_paths FROM sa_work_item_write_scopes",
            [],|r|r.get::<_,String>(0)).unwrap(),allowed);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM sa_audit WHERE
            kind='work_item_write_scope_inherited'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        let tx=db.unchecked_transaction().unwrap();
        assert!(inherit_initial_demand_paths(&tx,"root","item","wrong",2,allowed,2).is_err());
        assert!(inherit_initial_demand_paths(&tx,"root","item","implement",3,allowed,2).is_err());
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
    fn demand_write_scope_batch_is_atomic_idempotent_and_exact() {
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
        for (ordinal,key) in ["first","second"].iter().enumerate() {
            db.execute(
                "INSERT INTO sa_work_items(id,ticket_id,item_key,ordinal,objective,
                 worker_bot_id,state,created_at,updated_at,requires_write)
                 VALUES(?1,'root',?1,?2,'Edit pinned file','worker','pending',1,1,1)",
                params![key,ordinal as i64],
            ).unwrap();
        }
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        let exact=[("first","first"),("second","second")];
        store.db.execute_batch(
            "CREATE TRIGGER reject_second_scope_audit BEFORE INSERT ON sa_audit
             WHEN NEW.detail='WorkItem second'
             BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;",
        ).unwrap();
        assert!(commit_work_item_write_scopes(
            &store,"root",&exact,r#"["src/allowed.rs"]"#,2,
        ).is_err());
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_work_item_write_scopes",[],
            |row|row.get::<_,i64>(0),
        ).unwrap(),0,"one failed audit must roll back every scope");
        store.db.execute_batch("DROP TRIGGER reject_second_scope_audit;").unwrap();
        assert!(commit_work_item_write_scopes(
            &store,"root",&[("first","second")],r#"["src/allowed.rs"]"#,2,
        ).is_err(),"an item ID/key mismatch cannot receive a scope");
        assert!(commit_work_item_write_scopes(
            &store,"root",&exact,r#"["src/allowed.rs"]"#,3,
        ).is_err(),"a stale root revision cannot grant any child");
        for _ in 0..2 {
            commit_work_item_write_scopes(
                &store,"root",&exact,r#"["src/allowed.rs"]"#,2,
            ).unwrap();
        }
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_work_item_write_scopes",[],
            |row|row.get::<_,i64>(0),
        ).unwrap(),2);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit",[],
            |row|row.get::<_,i64>(0),
        ).unwrap(),2,"replay must not duplicate receipt entries");
        assert!(commit_work_item_write_scopes(
            &store,"root",&exact,r#"["src/other.rs"]"#,2,
        ).is_err(),"a confirmed child scope cannot be widened");
    }

    #[test]
    fn planned_work_rejects_missing_or_foreign_mandate_issuer() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
            CREATE TABLE sa_tickets(id TEXT PRIMARY KEY,actor TEXT,source TEXT);
            CREATE TABLE sa_mandates(
                ticket_id TEXT,actor TEXT,authorization_id TEXT,revoked INTEGER,
                expires_at INTEGER,policy_revision INTEGER,project_id TEXT,
                allowed_tools TEXT,allowed_side_effects INTEGER);
            CREATE TABLE sa_audit(ticket_id TEXT,kind TEXT,detail TEXT);
            INSERT INTO sa_tickets VALUES('ticket','local-user','manual');
            INSERT INTO sa_mandates VALUES(
                'ticket','local-user','native-issued',0,99999999999999,3,'project','[]',0);
            INSERT INTO sa_audit VALUES('ticket','admitted','manual readonly coordinator reserved');
        "#).unwrap();
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        let current=||manual_demand_mandate_current(&store,"ticket",3,"project").unwrap();
        assert!(current());
        store.db.execute("UPDATE sa_mandates SET actor='foreign'",[]).unwrap();
        assert!(!current(),"foreign issuer cannot authorize WorkItems");
        store.db.execute("UPDATE sa_mandates SET actor='local-user', authorization_id='  '",[]).unwrap();
        assert!(!current(),"blank issuance identity cannot grant WorkItems");
        store.db.execute("UPDATE sa_mandates SET authorization_id='native-issued',revoked=1",[]).unwrap();
        assert!(!current(),"revoked authority cannot schedule children");
        store.db.execute("UPDATE sa_mandates SET revoked=0,allowed_tools='[\"shell\"]'",[]).unwrap();
        assert!(!current(),"an expanded mandate cannot bypass read-only planning");
        store.db.execute("UPDATE sa_mandates SET allowed_tools='[]'",[]).unwrap();
        store.db.execute("DELETE FROM sa_audit",[]).unwrap();
        assert!(!current(),"a mandate requires its native admission receipt");
    }

    #[test]
    fn work_item_digest_is_stable_and_depends_on_exact_requested_plan() {
        let first=vec![item("compile",&[]),item("test",&["compile"])];
        assert_eq!(sha256(&first).unwrap(),sha256(&first).unwrap());
        let changed=vec![item("compile",&[]),item("test",&[])];
        assert_ne!(sha256(&first).unwrap(),sha256(&changed).unwrap());
    }
}
