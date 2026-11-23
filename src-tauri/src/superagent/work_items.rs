//! Operator-authored WorkItem plan entries. Planning is not Worker delegation,
//! provider execution, task-scoped authorization, or Council approval.
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_items (
 id TEXT PRIMARY KEY,
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 request_key TEXT NOT NULL,
 parent_id TEXT REFERENCES sa_work_items(id) ON DELETE RESTRICT,
 summary TEXT NOT NULL CHECK(length(summary) BETWEEN 1 AND 1000),
 state TEXT NOT NULL DEFAULT 'planned' CHECK(state='planned'),
 actor TEXT NOT NULL CHECK(actor='local-user'),
 created_at INTEGER NOT NULL,
 UNIQUE(ticket_id,request_key),
 CHECK(parent_id IS NULL OR parent_id<>id)
);
CREATE INDEX IF NOT EXISTS sa_work_items_ticket ON sa_work_items(ticket_id,created_at,id);
CREATE TRIGGER IF NOT EXISTS sa_work_item_parent_same_ticket
 BEFORE INSERT ON sa_work_items
 WHEN NEW.parent_id IS NOT NULL AND NOT EXISTS(
   SELECT 1 FROM sa_work_items parent
   WHERE parent.id=NEW.parent_id AND parent.ticket_id=NEW.ticket_id
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem parent must belong to the same Demand'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_parent_update_same_ticket
 BEFORE UPDATE OF parent_id,ticket_id ON sa_work_items
 WHEN NEW.ticket_id<>OLD.ticket_id OR NEW.parent_id IS NOT OLD.parent_id
 BEGIN SELECT RAISE(ABORT,'WorkItem ancestry is immutable and Demand-scoped'); END;
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkItem {
    pub id: String,
    pub ticket_id: String,
    pub request_key: String,
    pub parent_id: Option<String>,
    pub summary: String,
    pub state: String,
    pub created_at: i64,
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkItem> {
    Ok(WorkItem {
        id:row.get(0)?,ticket_id:row.get(1)?,request_key:row.get(2)?,
        parent_id:row.get(3)?,summary:row.get(4)?,state:row.get(5)?,
        created_at:row.get(6)?,
    })
}

pub(super) fn list(store:&Store,ticket_id:&str,limit:i64)->Result<(Vec<WorkItem>,bool)> {
    if !(1..=100).contains(&limit) {
        return Err("WorkItem page size must be between 1 and 100.".into());
    }
    let mut statement=store.db.prepare(
        "SELECT id,ticket_id,request_key,parent_id,summary,state,created_at
         FROM sa_work_items WHERE ticket_id=?1
         ORDER BY created_at,id LIMIT ?2",
    ).map_err(|e|e.to_string())?;
    let mut items=statement.query_map(params![ticket_id,limit+1],from_row)
        .map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    let truncated=items.len()>limit as usize;
    items.truncate(limit as usize);
    Ok((items,truncated))
}

/// A trusted local-user planning action only. No session, assignment, grant,
/// side effect, task completion, or child provider is created here. A stable
/// request key is a receipt: replay never increments the Ticket revision.
pub(super) fn plan(
    store:&Store,ticket_id:&str,expected_revision:i64,request_key:&str,
    summary:&str,parent_id:Option<&str>,
)->Result<WorkItem> {
    let summary=summary.trim();
    if request_key.is_empty() || request_key.len()>128
        || !request_key.bytes().all(|c|c.is_ascii_alphanumeric() || matches!(c,b'-'|b'_'|b'.'))
        || summary.is_empty() || summary.len()>1000
        || summary.chars().any(char::is_control)
        || expected_revision<0
    {
        return Err("WorkItem requires a bounded summary, request key and Ticket revision.".into());
    }
    if parent_id.is_some_and(|id|id.is_empty() || id.len()>128
        || !id.bytes().all(|c|c.is_ascii_alphanumeric() || matches!(c,b'-'|b'_'))) {
        return Err("Invalid parent WorkItem identity.".into());
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let previous=tx.query_row(
        "SELECT id,ticket_id,request_key,parent_id,summary,state,created_at
         FROM sa_work_items WHERE ticket_id=?1 AND request_key=?2",
        params![ticket_id,request_key],from_row,
    ).optional().map_err(|e|e.to_string())?;
    if let Some(item)=previous {
        if item.summary!=summary || item.parent_id.as_deref()!=parent_id {
            return Err("WorkItem request key is already bound to another plan.".into());
        }
        return Ok(item);
    }
    let ticket=super::get_ticket(store,ticket_id)?;
    if !matches!(ticket.state.as_str(),"blocked"|"queued")
        || ticket.active_assignment_id.is_some() || ticket.actor!="local-user"
    {
        return Err("Only blocked or queued local Demands without an active assignment can be planned.".into());
    }
    if let Some(parent)=parent_id {
        let valid:bool=tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_items WHERE id=?1 AND ticket_id=?2)",
            params![parent,ticket_id],|row|row.get(0),
        ).map_err(|e|e.to_string())?;
        if !valid {return Err("Parent WorkItem does not belong to this Demand.".into());}
    }
    let now=storage::now();
    let id=storage::id();
    let updated=tx.execute(
        "UPDATE sa_tickets SET revision=revision+1,updated_at=?3
         WHERE id=?1 AND revision=?2 AND state IN ('blocked','queued')
           AND actor='local-user' AND source='manual'
           AND active_assignment_id IS NULL
           AND NOT EXISTS(SELECT 1 FROM sa_assignments a
                          WHERE a.ticket_id=sa_tickets.id AND a.state='running')
           AND NOT EXISTS(SELECT 1 FROM sa_demand_archives a
                          WHERE a.ticket_id=sa_tickets.id)
           AND NOT EXISTS(SELECT 1 FROM sa_effect_receipts e
                          WHERE e.ticket_id=sa_tickets.id AND e.state!='applied')
           AND (SELECT COUNT(*) FROM sa_work_items w
                WHERE w.ticket_id=sa_tickets.id)<100",
        params![ticket_id,expected_revision,now],
    ).map_err(|e|e.to_string())?;
    if updated!=1 {
        return Err("Demand revision, state, archive or effect policy changed; planning denied.".into());
    }
    tx.execute(
        "INSERT INTO sa_work_items
         (id,ticket_id,request_key,parent_id,summary,state,actor,created_at)
         VALUES(?1,?2,?3,?4,?5,'planned','local-user',?6)",
        params![id,ticket_id,request_key,parent_id,summary,now],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_item_planned',?2,?3)",
        params![ticket_id,format!("Operator planned WorkItem {id}; no provider dispatched"),now],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    store.db.query_row(
        "SELECT id,ticket_id,request_key,parent_id,summary,state,created_at
         FROM sa_work_items WHERE id=?1",
        [&id],from_row,
    ).map_err(|e|e.to_string())
}
