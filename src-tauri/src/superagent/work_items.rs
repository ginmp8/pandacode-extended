//! Native, durable and bounded SuperAgent work-item dependency plans.
//!
//! A plan is an operator-authorized decomposition of a Ticket, never an
//! authority to dispatch a Worker, install a Skill or mark a requirement done.
//! Its immutable DAG is useful for deterministic frontier and blocked-state
//! diagnostics while the task execution scheduler remains independently gated.

pub(crate) mod progress;

use super::{effects, require_main_window};
use crate::{storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet, VecDeque}, path::Path};
use tauri::State;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_plans (
  ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  ticket_revision INTEGER NOT NULL CHECK(ticket_revision>=0),
  plan_sha256 TEXT NOT NULL CHECK(length(plan_sha256)=64),
  source TEXT NOT NULL CHECK(source='local-user'),
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sa_work_items (
  ticket_id TEXT NOT NULL REFERENCES sa_work_plans(ticket_id) ON DELETE RESTRICT,
  item_key TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK(ordinal>=0),
  title TEXT NOT NULL,
  acceptance TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'planned' CHECK(state='planned'),
  created_at INTEGER NOT NULL,
  PRIMARY KEY(ticket_id,item_key),
  UNIQUE(ticket_id,ordinal)
);
CREATE TABLE IF NOT EXISTS sa_work_item_dependencies (
  ticket_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  depends_on_key TEXT NOT NULL,
  PRIMARY KEY(ticket_id,item_key,depends_on_key),
  CHECK(item_key<>depends_on_key),
  FOREIGN KEY(ticket_id,item_key) REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT,
  FOREIGN KEY(ticket_id,depends_on_key) REFERENCES sa_work_items(ticket_id,item_key) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS sa_work_item_dependencies_parent
  ON sa_work_item_dependencies(ticket_id,depends_on_key,item_key);
"#;

const MAX_ITEMS: usize = 64;
const MAX_EDGES: usize = 192;
const MAX_TITLE: usize = 200;
const MAX_ACCEPTANCE: usize = 4096;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItemInput {
    pub key: String,
    pub title: String,
    pub acceptance: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkPlanInput {
    pub ticket_id: String,
    pub expected_revision: i64,
    pub items: Vec<WorkItemInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemView {
    pub key: String,
    pub title: String,
    pub acceptance: String,
    pub depends_on: Vec<String>,
    pub state: &'static str,
    pub ready: bool,
    pub execution_admitted: bool,
    pub evidence_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlanView {
    pub ticket_id: String,
    pub ticket_revision: i64,
    pub plan_sha256: String,
    pub source: String,
    pub created_at: i64,
    pub items: Vec<WorkItemView>,
    pub ready_frontier: Vec<String>,
    pub active_assignments: i64,
    pub execution_granted_by_plan: bool,
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 64 && key.bytes().all(|ch|
        ch.is_ascii_alphanumeric() || matches!(ch,b'-'|b'_'))
}

fn validate_plan(input: &WorkPlanInput) -> Result<String> {
    if !valid_key(&input.ticket_id) || input.expected_revision < 0
        || input.items.is_empty() || input.items.len() > MAX_ITEMS {
        return Err("Provide a Ticket, nonnegative revision and 1–64 work items.".into());
    }
    let mut keys = HashSet::new();
    let mut edge_count = 0_usize;
    for item in &input.items {
        if !valid_key(&item.key) || !keys.insert(&item.key)
            || item.title.trim().is_empty() || item.title.len() > MAX_TITLE
            || item.acceptance.trim().is_empty() || item.acceptance.len() > MAX_ACCEPTANCE
            || item.title.chars().any(char::is_control)
            || item.acceptance.contains('\0')
        {
            return Err("Work-item keys, titles and acceptance criteria must be unique, nonempty and bounded.".into());
        }
        edge_count = edge_count.saturating_add(item.depends_on.len());
        if edge_count > MAX_EDGES {
            return Err("Dependency count exceeds the 192-edge limit.".into());
        }
        let mut local = HashSet::new();
        for predecessor in &item.depends_on {
            if !valid_key(predecessor) || predecessor == &item.key
                || !local.insert(predecessor) {
                return Err("A work item cannot repeat or depend on itself.".into());
            }
        }
    }
    let index = input.items.iter().enumerate()
        .map(|(idx,item)| (item.key.as_str(),idx))
        .collect::<HashMap<_,_>>();
    let mut remaining = vec![0_usize;input.items.len()];
    let mut forward = vec![Vec::new();input.items.len()];
    for (i,item) in input.items.iter().enumerate() {
        for predecessor in &item.depends_on {
            let Some(&from) = index.get(predecessor.as_str()) else {
                return Err("Every dependency must reference a work item in the same Plan.".into());
            };
            forward[from].push(i);
            remaining[i] += 1;
        }
    }
    let mut ready = VecDeque::new();
    for (idx,degree) in remaining.iter().enumerate() {
        if *degree == 0 { ready.push_back(idx); }
    }
    let mut visited = 0;
    while let Some(idx) = ready.pop_front() {
        visited += 1;
        for &child in &forward[idx] {
            remaining[child] -= 1;
            if remaining[child] == 0 { ready.push_back(child); }
        }
    }
    if visited != input.items.len() {
        return Err("Work-item dependencies contain a directed cycle.".into());
    }
    let bytes = serde_json::to_vec(&input.items).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(&bytes).iter().map(|byte|format!("{byte:02x}")).collect())
}

fn inspect(store: &Store, ticket_id: &str) -> Result<WorkPlanView> {
    let (revision,digest,source,created_at):(i64,String,String,i64) = store.db.query_row(
        "SELECT ticket_revision,plan_sha256,source,created_at FROM sa_work_plans
         WHERE ticket_id=?1", [ticket_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).map_err(|_| "Native work plan not found.".to_owned())?;
    let mut item_statement=store.db.prepare(
        "SELECT item_key,title,acceptance,state FROM sa_work_items
         WHERE ticket_id=?1 ORDER BY ordinal,item_key"
    ).map_err(|e|e.to_string())?;
    let raw=item_statement.query_map([ticket_id],|row| Ok((
        row.get::<_,String>(0)?,row.get::<_,String>(1)?,
        row.get::<_,String>(2)?,row.get::<_,String>(3)?,
    ))).map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    let mut dependencies=store.db.prepare(
        "SELECT depends_on_key FROM sa_work_item_dependencies
         WHERE ticket_id=?1 AND item_key=?2 ORDER BY depends_on_key"
    ).map_err(|e|e.to_string())?;
    let mut items=Vec::new();
    let mut frontier=Vec::new();
    let awaiting_review:bool=if progress::available(store)? {
        store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_item_attempts attempt
             WHERE attempt.ticket_id=?1 AND attempt.state IN ('running','observed')
               AND NOT EXISTS (
                 SELECT 1 FROM sa_work_item_approvals verified
                 WHERE verified.ticket_id=attempt.ticket_id AND verified.item_key=attempt.item_key))",
            [ticket_id],|row|row.get(0),
        ).map_err(|error|error.to_string())?
    } else { false };
    for (key,title,acceptance,state) in raw {
        if state != "planned" {
            return Err("Unknown persisted work-item state; no scheduling authority inferred.".into());
        }
        let depends_on=dependencies.query_map(params![ticket_id,&key],|row|row.get::<_,String>(0))
            .map_err(|e|e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
        let (current_state,execution_admitted,evidence_verified)=
            progress::state(store,ticket_id,&key)?;
        let ready=!awaiting_review
            && matches!(current_state,"planned"|"failed")
            && depends_on.iter().try_fold(true,|ready,dependency| {
                progress::verified(store,ticket_id,dependency).map(|verified| ready&&verified)
            })?;
        if ready { frontier.push(key.clone()); }
        items.push(WorkItemView {
            key,title,acceptance,depends_on,state:current_state,ready,
            execution_admitted,evidence_verified,
        });
    }
    let active_assignments:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1 AND state='running'",
        [ticket_id],|row|row.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(WorkPlanView {
        ticket_id:ticket_id.into(),ticket_revision:revision,
        plan_sha256:digest,source,created_at,items,
        ready_frontier:frontier,active_assignments,
        execution_granted_by_plan:false,
    })
}

fn create(store: &Store, input: &WorkPlanInput) -> Result<WorkPlanView> {
    let digest=validate_plan(input)?;
    let ticket=super::get_ticket(store,&input.ticket_id)?;
    if ticket.revision != input.expected_revision
        || ticket.active_assignment_id.is_some()
        || ticket.state != "blocked" || ticket.actor != "local-user"
    {
        return Err("A work plan requires the current explicitly blocked local-user Ticket.".into());
    }
    let mandate=super::mandate(store,&ticket.id)?;
    if mandate.actor != "local-user" || mandate.revoked
        || mandate.expires_at <= storage::now()
        || mandate.allowed_side_effects || !mandate.allowed_tools.is_empty() {
        return Err("Work planning requires an active read-only native Ticket mandate.".into());
    }
    // A model may propose work items, but only this trusted local operator
    // boundary may persist a plan. Check-run and PR text never become authority.
    let existing:Option<(i64,String)>=store.db.query_row(
        "SELECT ticket_revision,plan_sha256 FROM sa_work_plans WHERE ticket_id=?1",
        [&ticket.id],|row|Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    if let Some((revision,prior_digest))=existing {
        if revision != input.expected_revision || prior_digest != digest {
            return Err("Work plan already exists with a different immutable revision or content.".into());
        }
        return inspect(store,&ticket.id);
    }
    if effects::unresolved_count(store,&ticket.id)? > 0 {
        return Err("Unreconciled external effects prevent work-plan admission.".into());
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let admissible:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets t
          JOIN sa_mandates m ON m.ticket_id=t.id
          WHERE t.id=?1 AND t.state='blocked' AND t.revision=?2
            AND t.active_assignment_id IS NULL
            AND m.revoked=0 AND m.expires_at>?3 AND m.actor='local-user')",
        params![ticket.id,input.expected_revision,storage::now()],|row|row.get(0)
    ).map_err(|e|e.to_string())?;
    if !admissible { return Err("Ticket revision or mandate changed during plan admission.".into()); }
    let now=storage::now();
    tx.execute(
        "INSERT INTO sa_work_plans(ticket_id,ticket_revision,plan_sha256,source,created_at)
         VALUES(?1,?2,?3,'local-user',?4)",
        params![ticket.id,input.expected_revision,digest,now]
    ).map_err(|e|e.to_string())?;
    for (ordinal,item) in input.items.iter().enumerate() {
        tx.execute(
            "INSERT INTO sa_work_items(ticket_id,item_key,ordinal,title,acceptance,created_at)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![ticket.id,item.key,ordinal as i64,item.title.trim(),item.acceptance.trim(),now]
        ).map_err(|e|e.to_string())?;
    }
    for item in &input.items {
        for depends_on in &item.depends_on {
            tx.execute(
                "INSERT INTO sa_work_item_dependencies(ticket_id,item_key,depends_on_key)
                 VALUES(?1,?2,?3)",
                params![ticket.id,item.key,depends_on]
            ).map_err(|e|e.to_string())?;
        }
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_plan_registered',?2,?3)",
        params![ticket.id,format!("{} items, SHA-256 {digest}",input.items.len()),now]
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    inspect(store,&ticket.id)
}

/// Same strict local operator boundary for Desktop and one-shot headless CLI.
pub(crate) fn headless_create(database_path:&Path,input:WorkPlanInput)->Result<String> {
    let store=super::open_headless_transition_store(database_path)?;
    let plan=create(&store,&input)?;
    serde_json::to_string_pretty(&plan).map_err(|e|e.to_string())
}

pub(crate) fn headless_inspect(database_path:&Path,ticket_id:&str)->Result<String> {
    let store=Store::open_read_only(database_path)?;
    serde_json::to_string_pretty(&inspect(&store,ticket_id)?).map_err(|e|e.to_string())
}

#[tauri::command]
pub async fn superagent_work_plan_create(
    window:tauri::WebviewWindow,
    state:State<'_,AppState>,
    input:WorkPlanInput,
)->Result<WorkPlanView> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    create(&store,&input)
}

#[tauri::command]
pub async fn superagent_work_plan(
    window:tauri::WebviewWindow,
    state:State<'_,AppState>,
    ticket_id:String,
)->Result<WorkPlanView> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    inspect(&store,&ticket_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_dag_rejects_duplicate_unknown_and_cyclic_dependencies() {
        let good=WorkPlanInput{
            ticket_id:"ticket_01".into(),expected_revision:0,
            items:vec![
                WorkItemInput{key:"a".into(),title:"Plan".into(),
                    acceptance:"Plan reviewed".into(),depends_on:vec![]},
                WorkItemInput{key:"b".into(),title:"Build".into(),
                    acceptance:"Code recorded".into(),depends_on:vec!["a".into()]},
                WorkItemInput{key:"c".into(),title:"Review".into(),
                    acceptance:"Independent evidence".into(),depends_on:vec!["b".into()]},
            ]
        };
        assert_eq!(validate_plan(&good).unwrap().len(),64);
        let mut changed=good.clone();
        changed.items[0].depends_on=vec!["c".into()];
        assert!(validate_plan(&changed).unwrap_err().contains("cycle"));
        let mut changed=good.clone();
        changed.items[1].depends_on=vec!["missing".into()];
        assert!(validate_plan(&changed).unwrap_err().contains("same Plan"));
        let mut changed=good.clone();
        changed.items[1].depends_on=vec!["a".into(),"a".into()];
        assert!(validate_plan(&changed).unwrap_err().contains("repeat"));
        let mut changed=good.clone();
        changed.items[2].key="a".into();
        assert!(validate_plan(&changed).is_err());
    }
}
