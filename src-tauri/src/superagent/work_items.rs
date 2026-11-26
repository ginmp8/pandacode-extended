//! Operator-authored WorkItem plans and native manual attempt receipts.
//! Planning alone grants no provider execution, tool authority or Council approval.
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

/// Immutable, task-scoped operator cancellation receipts. This is an additive
/// migration because original planned WorkItems must keep their evidence and
/// original schema CHECK constraint without rewriting historical rows.
pub(crate) const CANCELLATION_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_item_cancellations (
 item_id TEXT PRIMARY KEY REFERENCES sa_work_items(id) ON DELETE RESTRICT,
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 request_key TEXT NOT NULL,
 cancelled_at INTEGER NOT NULL CHECK(cancelled_at>0),
 UNIQUE(ticket_id,request_key)
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_cancel_same_ticket
 BEFORE INSERT ON sa_work_item_cancellations
 WHEN NOT EXISTS(SELECT 1 FROM sa_work_items w
                 WHERE w.id=NEW.item_id AND w.ticket_id=NEW.ticket_id)
 BEGIN SELECT RAISE(ABORT,'WorkItem cancellation must match its Demand'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_cancel_immutable_update
 BEFORE UPDATE ON sa_work_item_cancellations
 BEGIN SELECT RAISE(ABORT,'WorkItem cancellation evidence is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_cancel_immutable_delete
 BEFORE DELETE ON sa_work_item_cancellations
 BEGIN SELECT RAISE(ABORT,'WorkItem cancellation evidence is immutable'); END;
"#;


/// Append-only native WorkItem execution receipts; no authority is inferred
/// from historical prompt-only context.
pub(crate) const EXECUTION_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_item_attempts (
 assignment_id TEXT PRIMARY KEY REFERENCES sa_assignments(id) ON DELETE RESTRICT,
 item_id TEXT NOT NULL REFERENCES sa_work_items(id) ON DELETE RESTRICT,
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 confirmed_revision INTEGER NOT NULL CHECK(confirmed_revision>=0)
);
CREATE INDEX IF NOT EXISTS sa_work_item_attempts_item ON sa_work_item_attempts(item_id);
CREATE TABLE IF NOT EXISTS sa_work_item_acceptances (
 item_id TEXT PRIMARY KEY REFERENCES sa_work_items(id) ON DELETE RESTRICT,
 assignment_id TEXT NOT NULL UNIQUE REFERENCES sa_work_item_attempts(assignment_id) ON DELETE RESTRICT,
 accepted_revision INTEGER NOT NULL CHECK(accepted_revision>=0),
 accepted_at INTEGER NOT NULL CHECK(accepted_at>0)
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_guard
 BEFORE INSERT ON sa_work_item_attempts
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_items w
   JOIN sa_assignments a ON a.id=NEW.assignment_id AND a.ticket_id=w.ticket_id
   JOIN sa_tickets t ON t.id=w.ticket_id
   WHERE w.id=NEW.item_id AND w.ticket_id=NEW.ticket_id
     AND t.active_assignment_id=a.id AND t.state='running'
     AND t.revision=NEW.confirmed_revision+1 AND a.state='running'
     AND a.bot_id=t.worker_bot_id
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_acceptances c WHERE c.item_id=w.id)
     AND (w.parent_id IS NULL OR EXISTS(
         SELECT 1 FROM sa_work_item_acceptances p WHERE p.item_id=w.parent_id))
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem attempt must match its active native Worker and ready plan'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_acceptance_guard
 BEFORE INSERT ON sa_work_item_acceptances
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_item_attempts b
   JOIN sa_assignments a ON a.id=b.assignment_id AND a.ticket_id=b.ticket_id
   JOIN sa_tickets t ON t.id=b.ticket_id
   WHERE b.item_id=NEW.item_id AND b.assignment_id=NEW.assignment_id
     AND t.state='waiting_approval' AND t.revision=NEW.accepted_revision
     AND a.state='finished' AND a.bot_id=t.worker_bot_id
     AND a.result=t.evidence AND length(trim(a.result))>0
     AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=t.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=b.item_id)
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem acceptance requires current native Worker evidence'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_immutable_update
 BEFORE UPDATE ON sa_work_item_attempts
 BEGIN SELECT RAISE(ABORT,'WorkItem attempt evidence is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_immutable_delete
 BEFORE DELETE ON sa_work_item_attempts
 BEGIN SELECT RAISE(ABORT,'WorkItem attempt evidence is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_acceptance_immutable_update
 BEFORE UPDATE ON sa_work_item_acceptances
 BEGIN SELECT RAISE(ABORT,'WorkItem acceptance evidence is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_acceptance_immutable_delete
 BEFORE DELETE ON sa_work_item_acceptances
 BEGIN SELECT RAISE(ABORT,'WorkItem acceptance evidence is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_cancel_unattempted
 BEFORE INSERT ON sa_work_item_cancellations
 WHEN EXISTS(SELECT 1 FROM sa_work_item_attempts b WHERE b.item_id=NEW.item_id)
 BEGIN SELECT RAISE(ABORT,'Attempted WorkItems cannot be cancelled as unexecuted plans'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_completion_guard
 BEFORE UPDATE OF state ON sa_tickets
 WHEN NEW.state='completed' AND OLD.state<>'completed' AND EXISTS(
   SELECT 1 FROM sa_work_items w WHERE w.ticket_id=NEW.id
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id)
 )
 BEGIN SELECT RAISE(ABORT,'Demand has unresolved WorkItems'); END;
"#;

/// Operator-reviewed routing rationale; never a grant or an autonomous reassignment.
pub(crate) const ROUTING_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_work_item_routes (
 item_id TEXT PRIMARY KEY REFERENCES sa_work_items(id) ON DELETE RESTRICT,
 worker_bot_id TEXT NOT NULL CHECK(length(worker_bot_id) BETWEEN 1 AND 128),
 routing_reason TEXT NOT NULL CHECK(length(trim(routing_reason)) BETWEEN 1 AND 512)
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_route_guard
 BEFORE INSERT ON sa_work_item_routes
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_items w JOIN sa_tickets t ON t.id=w.ticket_id
   WHERE w.id=NEW.item_id AND t.worker_bot_id=NEW.worker_bot_id
     AND t.state IN ('queued','blocked') AND t.active_assignment_id IS NULL
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_attempts b WHERE b.item_id=w.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem route must match its selected native Worker and unexecuted plan'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_route_immutable_update
 BEFORE UPDATE ON sa_work_item_routes
 BEGIN SELECT RAISE(ABORT,'WorkItem route is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_route_immutable_delete
 BEFORE DELETE ON sa_work_item_routes
 BEGIN SELECT RAISE(ABORT,'WorkItem route is immutable'); END;
"#;


/// Version 59 keeps the original Ticket's default Worker immutable. An
/// operator-reviewed WorkItem route may authorize another frozen same-project
/// Worker for that item only. The attempt receipt, not live bot configuration
/// or provider output, resolves the exact recipient at every later boundary.
pub(crate) const MULTI_WORKER_SCHEMA: &str = r#"
DROP TRIGGER IF EXISTS sa_work_item_route_guard;
DROP TRIGGER IF EXISTS sa_work_item_attempt_guard;
DROP TRIGGER IF EXISTS sa_work_item_acceptance_guard;
CREATE VIEW IF NOT EXISTS sa_assignment_worker_authority AS
 SELECT a.id AS assignment_id,
   CASE WHEN b.assignment_id IS NULL THEN t.worker_bot_id
        WHEN w.id IS NULL THEN NULL
        ELSE COALESCE(r.worker_bot_id,t.worker_bot_id) END AS worker_bot_id
 FROM sa_assignments a JOIN sa_tickets t ON t.id=a.ticket_id
 LEFT JOIN sa_work_item_attempts b ON b.assignment_id=a.id
 LEFT JOIN sa_work_items w ON w.id=b.item_id AND w.ticket_id=t.id AND b.ticket_id=t.id
 LEFT JOIN sa_work_item_routes r ON r.item_id=w.id;
CREATE TRIGGER IF NOT EXISTS sa_work_item_route_guard
 BEFORE INSERT ON sa_work_item_routes
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_items w JOIN sa_tickets t ON t.id=w.ticket_id
   JOIN sa_ticket_profiles p ON p.ticket_id=t.id AND p.policy_revision=t.policy_revision
   JOIN json_each(p.profiles_json) member
   WHERE w.id=NEW.item_id
     AND json_extract(member.value,'$.botId')=NEW.worker_bot_id
     AND json_extract(member.value,'$.role')='worker'
     AND json_extract(member.value,'$.projectId')=t.project_id
     AND t.state IN ('queued','blocked') AND t.active_assignment_id IS NULL
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_attempts b WHERE b.item_id=w.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem route must match its selected native Worker and unexecuted plan'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_attempt_guard
 BEFORE INSERT ON sa_work_item_attempts
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_items w
   JOIN sa_assignments a ON a.id=NEW.assignment_id AND a.ticket_id=w.ticket_id
   JOIN sa_tickets t ON t.id=w.ticket_id
   WHERE w.id=NEW.item_id AND w.ticket_id=NEW.ticket_id
     AND t.active_assignment_id=a.id AND t.state='running'
     AND t.revision=NEW.confirmed_revision+1 AND a.state='running'
     AND a.bot_id=COALESCE((SELECT r.worker_bot_id FROM sa_work_item_routes r
                             WHERE r.item_id=w.id),t.worker_bot_id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_acceptances c WHERE c.item_id=w.id)
     AND (w.parent_id IS NULL OR EXISTS(
         SELECT 1 FROM sa_work_item_acceptances p WHERE p.item_id=w.parent_id))
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem attempt must match its active native Worker and ready plan'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_acceptance_guard
 BEFORE INSERT ON sa_work_item_acceptances
 WHEN NOT EXISTS(
   SELECT 1 FROM sa_work_item_attempts b
   JOIN sa_assignments a ON a.id=b.assignment_id AND a.ticket_id=b.ticket_id
   JOIN sa_tickets t ON t.id=b.ticket_id
   WHERE b.item_id=NEW.item_id AND b.assignment_id=NEW.assignment_id
     AND t.state='waiting_approval' AND t.revision=NEW.accepted_revision
     AND a.state='finished' AND a.bot_id=COALESCE(
         (SELECT r.worker_bot_id FROM sa_work_item_routes r WHERE r.item_id=b.item_id),
         t.worker_bot_id)
     AND a.result=t.evidence AND length(trim(a.result))>0
     AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=t.id)
     AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=b.item_id)
 )
 BEGIN SELECT RAISE(ABORT,'WorkItem acceptance requires current native Worker evidence'); END;
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
    /// Durable native cancellation receipt, separate from the immutable plan.
    pub cancelled_at: Option<i64>,
    /// Latest real native attempt; an attempt is not accepted delivery.
    pub assignment_id: Option<String>,
    pub worker_bot_id: Option<String>,
    pub routing_reason: Option<String>,
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkItem> {
    Ok(WorkItem {
        id:row.get(0)?,ticket_id:row.get(1)?,request_key:row.get(2)?,
        parent_id:row.get(3)?,summary:row.get(4)?,state:row.get(5)?,
        created_at:row.get(6)?,cancelled_at:row.get(7)?,assignment_id:row.get(8)?,
        worker_bot_id:row.get(9)?,routing_reason:row.get(10)?,
    })
}

pub(super) fn list(store:&Store,ticket_id:&str,limit:i64)->Result<(Vec<WorkItem>,bool)> {
    if !(1..=100).contains(&limit) {
        return Err("WorkItem page size must be between 1 and 100.".into());
    }
    let mut statement=store.db.prepare(
        "SELECT w.id,w.ticket_id,w.request_key,w.parent_id,w.summary,
                CASE WHEN c.item_id IS NOT NULL THEN 'cancelled'
                     WHEN EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id)
                     THEN 'accepted' ELSE w.state END,w.created_at,c.cancelled_at,
                (SELECT b.assignment_id FROM sa_work_item_attempts b
                 JOIN sa_assignments a ON a.id=b.assignment_id
                 WHERE b.item_id=w.id ORDER BY a.attempt DESC LIMIT 1),
                r.worker_bot_id,r.routing_reason
         FROM sa_work_items w LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
         LEFT JOIN sa_work_item_routes r ON r.item_id=w.id
         WHERE w.ticket_id=?1 ORDER BY w.created_at,w.id LIMIT ?2",
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
    let tx = rusqlite::Transaction::new_unchecked(
        &store.db, rusqlite::TransactionBehavior::Immediate,
    ).map_err(|e|e.to_string())?;
    let item = plan_in_transaction(store, &tx, ticket_id, expected_revision,
        request_key, summary, PlanMetadata { parent_id, route: None })?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(item)
}

struct PlanMetadata<'a> {
    parent_id: Option<&'a str>,
    route: Option<(&'a str, &'a str)>,
}

fn plan_in_transaction(
    store: &Store, tx: &rusqlite::Transaction<'_>, ticket_id: &str,
    expected_revision: i64, request_key: &str, summary: &str, metadata: PlanMetadata<'_>,
) -> Result<WorkItem> {
    let PlanMetadata { parent_id, route } = metadata;
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
    let route = route.map(|(worker, reason)| (worker, reason.trim()));
    if route.is_some_and(|(worker, reason)| worker.is_empty() || worker.len() > 128
        || !worker.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        || reason.is_empty() || reason.len() > 512 || reason.chars().any(char::is_control)) {
        return Err("WorkItem routing requires a bounded Worker ID and rationale.".into());
    }
    let previous=tx.query_row(
        "SELECT w.id,w.ticket_id,w.request_key,w.parent_id,w.summary,
                CASE WHEN c.item_id IS NOT NULL THEN 'cancelled'
                     WHEN EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id)
                     THEN 'accepted' ELSE w.state END,w.created_at,c.cancelled_at,
                (SELECT b.assignment_id FROM sa_work_item_attempts b
                 JOIN sa_assignments a ON a.id=b.assignment_id
                 WHERE b.item_id=w.id ORDER BY a.attempt DESC LIMIT 1),
                r.worker_bot_id,r.routing_reason
         FROM sa_work_items w LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
         LEFT JOIN sa_work_item_routes r ON r.item_id=w.id
         WHERE w.ticket_id=?1 AND w.request_key=?2",
        params![ticket_id,request_key],from_row,
    ).optional().map_err(|e|e.to_string())?;
    if let Some(item)=previous {
        if item.summary!=summary || item.parent_id.as_deref()!=parent_id
            || item.worker_bot_id.as_deref()!=route.map(|(worker, _)| worker)
            || item.routing_reason.as_deref()!=route.map(|(_, reason)| reason) {
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
    if ticket.revision != expected_revision {
        return Err("Demand revision changed; planning denied.".into());
    }
    // An unreviewed structured Coordinator plan cannot be resolved by an
    // implicit default Worker, including the single-item planning endpoint.
    // Identical historical request-key replays already returned above.
    if route.is_none() && super::unreviewed_coordinator_plan(store, &ticket)? {
        return Err("Review each Coordinator WorkItem with an explicit Worker ID and routing rationale.".into());
    }
    if let Some((worker, _)) = route {
        if super::roster::ticket_profile(store, ticket_id)?.is_none() {
            return Err("WorkItem routing requires an admitted Team profile.".into());
        }
        let config = super::configuration(store, &ticket.orchestration_id)?;
        super::direct_team_member(store, &config, worker)?;
        super::roster::require_current_profile(store, ticket_id, "worker", worker)?;
    }
    // Each remaining non-cancelled, unaccepted item needs at least one
    // more native Worker attempt. Failed attempts count too. Reserve this
    // capacity in the same transaction as the plan revision, using the
    // immutable Ticket budget rather than the Team's mutable preferences.
    let budget = super::roster::ticket_worker_attempt_limit(store, ticket_id)?;
    let (used, pending): (i64, i64) = tx.query_row(
        "SELECT
           (SELECT COUNT(*) FROM sa_assignments a JOIN sa_tickets t
            ON t.id=a.ticket_id JOIN sa_orchestrations o ON o.id=t.orchestration_id
            WHERE t.id=?1 AND a.bot_id<>o.coordinator_bot_id),
           (SELECT COUNT(*) FROM sa_work_items w WHERE w.ticket_id=?1
            AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
            AND NOT EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id))",
        [ticket_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;
    if pending >= budget.saturating_sub(used) {
        return Err("WorkItem plan exceeds remaining native Worker attempt budget; cancel an unexecuted item or create a new authorized Demand.".into());
    }
    if let Some(parent)=parent_id {
        let valid:bool=tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sa_work_items w
                LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
                WHERE w.id=?1 AND w.ticket_id=?2 AND c.item_id IS NULL)",
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
    super::headless_dispatch::reject_stale_pending(tx, ticket_id, now)?;
    tx.execute(
        "INSERT INTO sa_work_items
         (id,ticket_id,request_key,parent_id,summary,state,actor,created_at)
         VALUES(?1,?2,?3,?4,?5,'planned','local-user',?6)",
        params![id,ticket_id,request_key,parent_id,summary,now],
    ).map_err(|e|e.to_string())?;
    if let Some((worker, reason)) = route {
        tx.execute(
            "INSERT INTO sa_work_item_routes(item_id,worker_bot_id,routing_reason) VALUES(?1,?2,?3)",
            params![id,worker,reason],
        ).map_err(|e|e.to_string())?;
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_item_planned',?2,?3)",
        params![ticket_id,format!("Operator planned WorkItem {id}; no provider dispatched"),now],
    ).map_err(|e|e.to_string())?;
    tx.query_row(
        "SELECT w.id,w.ticket_id,w.request_key,w.parent_id,w.summary,
                CASE WHEN c.item_id IS NOT NULL THEN 'cancelled'
                     WHEN EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id)
                     THEN 'accepted' ELSE w.state END,w.created_at,c.cancelled_at,
                (SELECT b.assignment_id FROM sa_work_item_attempts b
                 JOIN sa_assignments a ON a.id=b.assignment_id
                 WHERE b.item_id=w.id ORDER BY a.attempt DESC LIMIT 1),
                r.worker_bot_id,r.routing_reason
         FROM sa_work_items w LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
         LEFT JOIN sa_work_item_routes r ON r.item_id=w.id
         WHERE w.id=?1",
        [&id],from_row,
    ).map_err(|e|e.to_string())
}


/// An operator-reviewed dependency plan. It may be prepared from Coordinator
/// advice, but this document has no execution or permission fields.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlannedWorkItemInput {
    request_key: String,
    summary: String,
    parent_request_key: Option<String>,
    worker_bot_id: Option<String>,
    routing_reason: Option<String>,
}

pub(super) const MAX_PLAN_BYTES: usize = 16 * 1024;

pub(crate) fn decode_plan(text: &str) -> Result<Vec<PlannedWorkItemInput>> {
    if text.len() > MAX_PLAN_BYTES {
        return Err("WorkItem plan exceeds the native 16 KiB limit.".into());
    }
    serde_json::from_str(text)
        .map_err(|_| "WorkItem plan must be a JSON array of requestKey, summary and optional parentRequestKey and paired workerBotId/routingReason.".into())
}

/// Check the shape and admitted routing of an advisory Coordinator response.
/// No inserts, reservations or authority changes are allowed on this read path.
pub(super) fn validate_coordinator_proposal(
    store: &Store, ticket: &super::Ticket, config: &super::Orchestration,
    inputs: &[PlannedWorkItemInput],
) -> Result<()> {
    let budget = super::roster::ticket_worker_attempt_limit(store, &ticket.id)?;
    if inputs.is_empty() || inputs.len() > 16 || inputs.len() as i64 > budget {
        return Err("Coordinator proposal exceeds the admitted Worker attempt budget.".into());
    }
    let mut keys = std::collections::BTreeSet::new();
    for input in inputs {
        let key = input.request_key.as_str();
        let summary = input.summary.trim();
        if key.is_empty() || key.len() > 128
            || !key.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
            || summary.is_empty() || summary.len() > 1000 || summary.chars().any(char::is_control)
            || input.parent_request_key.as_deref().is_some_and(|parent| !keys.contains(parent))
            || !keys.insert(key)
        {
            return Err("Invalid or unordered Coordinator WorkItem proposal.".into());
        }
        let (Some(worker), Some(reason)) = (
            input.worker_bot_id.as_deref(), input.routing_reason.as_deref(),
        ) else {
            return Err("Coordinator proposals must identify an admitted Worker and rationale.".into());
        };
        let reason = reason.trim();
        if worker.is_empty() || worker.len() > 128
            || !worker.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            || reason.is_empty() || reason.len() > 512 || reason.chars().any(char::is_control)
        {
            return Err("Invalid Coordinator Worker route.".into());
        }
        super::direct_team_member(store, config, worker)?;
        super::roster::require_current_profile(store, &ticket.id, "worker", worker)?;
    }
    Ok(())
}

/// Apply an ordered, bounded plan atomically through the same native policy,
/// budget, revision and immutable request-key checks as individual planning.
/// Existing identical keys are read-only receipts, including on stale replay.
pub(super) fn plan_batch(
    store: &Store, ticket_id: &str, expected_revision: i64,
    inputs: &[PlannedWorkItemInput],
) -> Result<Vec<WorkItem>> {
    if inputs.is_empty() || inputs.len() > 16 || expected_revision < 0 {
        return Err("A WorkItem plan must contain 1 to 16 items and a valid revision.".into());
    }
    let tx = rusqlite::Transaction::new_unchecked(
        &store.db, rusqlite::TransactionBehavior::Immediate,
    ).map_err(|e|e.to_string())?;
    // Freeze the review requirement before the first insert. Otherwise an
    // explicitly routed first item would hide the pending review from later
    // items in this same transaction and permit an implicit default route.
    let ticket = super::get_ticket(store, ticket_id)?;
    if inputs.iter().any(|input| input.worker_bot_id.is_none() || input.routing_reason.is_none())
        && super::unreviewed_coordinator_plan(store, &ticket)?
    {
        return Err("Review each Coordinator WorkItem with an explicit Worker ID and routing rationale.".into());
    }
    let mut revision = expected_revision;
    let mut keys = std::collections::BTreeMap::<String, String>::new();
    let mut items = Vec::with_capacity(inputs.len());
    for input in inputs {
        if keys.contains_key(&input.request_key) {
            return Err("Duplicate WorkItem key in dependency plan.".into());
        }
        let parent = input.parent_request_key.as_ref().map(|key| {
            keys.get(key).map(String::as_str)
                .ok_or("WorkItem parent key must precede its child in the same plan.")
        }).transpose()?;
        let route = match (input.worker_bot_id.as_deref(), input.routing_reason.as_deref()) {
            (None, None) => None,
            (Some(worker), Some(reason)) => Some((worker, reason)),
            _ => return Err("WorkItem route requires both workerBotId and routingReason.".into()),
        };
        let before = super::get_ticket(store, ticket_id)?.revision;
        let item = plan_in_transaction(store, &tx, ticket_id, revision,
            &input.request_key, &input.summary, PlanMetadata { parent_id: parent, route })?;
        let after = super::get_ticket(store, ticket_id)?.revision;
        // An idempotent receipt cannot refresh a stale caller's authority
        // for the next, genuinely new item.
        if before != after {
            revision = after;
        }
        keys.insert(input.request_key.clone(), item.id.clone());
        items.push(item);
    }
    tx.commit().map_err(|e|e.to_string())?;
    Ok(items)
}

pub(crate) fn headless_plan_batch(
    database_path: &std::path::Path, ticket_id: &str, expected_revision: i64,
    inputs: &[PlannedWorkItemInput],
) -> Result<String> {
    let store = super::open_headless_transition_store(database_path)?;
    let items = plan_batch(&store, ticket_id, expected_revision, inputs)?;
    let ticket = super::get_ticket(&store, ticket_id)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId": ticket_id, "revision": ticket.revision,
        "items": items.iter().map(|item| serde_json::json!({
            "workItemId": item.id, "requestKey": item.request_key,
            "parentId": item.parent_id, "state": item.state,
            "workerBotId": item.worker_bot_id, "routingReason": item.routing_reason,
        })).collect::<Vec<_>>(),
        "providerDispatched": false, "grantIssued": false, "externalEffectReplayed": false,
    })).map_err(|e|e.to_string())
}

/// A trusted local operator cancels only an unexecuted WorkItem plan. The
/// immutable original plan, child lineage, Ticket and audit history survive.
/// No Worker, provider, Council or remote action is invoked. Same-key replay
/// is read-only even when the original Ticket revision has subsequently moved.
pub(super) fn cancel(
    store:&Store,ticket_id:&str,expected_revision:i64,item_id:&str,request_key:&str,
)->Result<WorkItem> {
    if expected_revision<0 || item_id.is_empty() || item_id.len()>128
        || !item_id.bytes().all(|b|b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'))
        || request_key.is_empty() || request_key.len()>128
        || !request_key.bytes().all(|b|b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'))
    {
        return Err("WorkItem cancellation requires a valid item, request key and Ticket revision.".into());
    }
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let previous:Option<(WorkItem,Option<String>)>=tx.query_row(
        "SELECT w.id,w.ticket_id,w.request_key,w.parent_id,w.summary,
                CASE WHEN c.item_id IS NOT NULL THEN 'cancelled'
                     WHEN EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id)
                     THEN 'accepted' ELSE w.state END,w.created_at,c.cancelled_at,
                (SELECT b.assignment_id FROM sa_work_item_attempts b
                 JOIN sa_assignments a ON a.id=b.assignment_id
                 WHERE b.item_id=w.id ORDER BY a.attempt DESC LIMIT 1),
                r.worker_bot_id,r.routing_reason,c.request_key
         FROM sa_work_items w LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
         LEFT JOIN sa_work_item_routes r ON r.item_id=w.id
         WHERE w.id=?1 AND w.ticket_id=?2",
        params![item_id,ticket_id],
        |row|Ok((from_row(row)?,row.get(11)?)),
    ).optional().map_err(|e|e.to_string())?;
    let (item,prior_key)=previous.ok_or("WorkItem does not belong to this Demand.")?;
    if item.state=="cancelled" {
        if prior_key.as_deref()==Some(request_key) {return Ok(item);}
        return Err("WorkItem was already cancelled by another operator request.".into());
    }
    if item.state != "planned" || item.assignment_id.is_some() {
        return Err("An attempted WorkItem is not an unexecuted plan; use native Demand recovery or Stop.".into());
    }
    let now=storage::now();
    let changed=tx.execute(
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
           AND EXISTS(SELECT 1 FROM sa_work_items w
                      WHERE w.id=?4 AND w.ticket_id=sa_tickets.id
                        AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c
                                       WHERE c.item_id=w.id))
           AND NOT EXISTS(
               SELECT 1 FROM sa_work_items child
               LEFT JOIN sa_work_item_cancellations canceled ON canceled.item_id=child.id
               WHERE child.ticket_id=sa_tickets.id AND child.parent_id=?4
                 AND canceled.item_id IS NULL)",
        params![ticket_id,expected_revision,now,item_id],
    ).map_err(|e|e.to_string())?;
    if changed!=1 {
        return Err("Demand revision, state, active child or effect policy changed; cancellation denied.".into());
    }
    super::headless_dispatch::reject_stale_pending(&tx, ticket_id, now)?;
    tx.execute(
        "INSERT INTO sa_work_item_cancellations(item_id,ticket_id,request_key,cancelled_at)
         VALUES(?1,?2,?3,?4)",
        params![item_id,ticket_id,request_key,now],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'work_item_cancelled',?2,?3)",
        params![ticket_id,
            format!("Operator cancelled planned WorkItem {item_id}; no provider dispatched"),
            now],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(WorkItem{state:"cancelled".into(),cancelled_at:Some(now),..item})
}


/// Bind one ready plan inside the native Assignment admission transaction.
pub(super) fn bind_attempt(
    store: &Store, ticket_id: &str, assignment_id: &str, item_id: &str, revision: i64,
) -> Result<()> {
    store.db.execute(
        "INSERT INTO sa_work_item_attempts(assignment_id,item_id,ticket_id,confirmed_revision)
         VALUES(?1,?2,?3,?4)",
        params![assignment_id,item_id,ticket_id,revision],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Accept only the exact item bound to the already validated Worker evidence.
/// Return whether another item remains. Historical prompt-only context is not
/// a binding and can never be promoted to accepted WorkItems.
pub(super) fn accept_current_attempt(
    store: &Store, ticket_id: &str, session_id: &str, revision: i64, now: i64,
) -> Result<bool> {
    let pending = || -> Result<bool> {
        store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_items w WHERE w.ticket_id=?1
             AND NOT EXISTS(SELECT 1 FROM sa_work_item_cancellations c WHERE c.item_id=w.id)
             AND NOT EXISTS(SELECT 1 FROM sa_work_item_acceptances v WHERE v.item_id=w.id))",
            [ticket_id], |row| row.get(0),
        ).map_err(|e| e.to_string())
    };
    let binding: Option<(String,String)> = store.db.query_row(
        "SELECT b.item_id,b.assignment_id FROM sa_work_item_attempts b
         JOIN sa_assignments a ON a.id=b.assignment_id AND a.ticket_id=b.ticket_id
         WHERE b.ticket_id=?1 AND a.session_id=?2 AND a.state='finished'
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?1)",
        params![ticket_id,session_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((item_id,assignment_id)) = binding else {
        if pending()? {
            return Err("Current Worker evidence has no individual WorkItem binding; reject it and dispatch a current item.".into());
        }
        return Ok(false);
    };
    store.db.execute(
        "INSERT INTO sa_work_item_acceptances(item_id,assignment_id,accepted_revision,accepted_at)
         VALUES(?1,?2,?3,?4)",
        params![item_id,assignment_id,revision,now],
    ).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at) VALUES(?1,'work_item_human_accepted',?2,?3)",
        params![ticket_id,serde_json::json!({
            "workItemId":item_id,"assignmentId":assignment_id,"acceptedRevision":revision,
        }).to_string(),now],
    ).map_err(|e| e.to_string())?;
    pending()
}

/// Headless operators share Desktop WorkItem policy and Ticket CAS without
/// invoking startup recovery on a concurrently running PandaCode host.
pub(crate) fn headless_plan(
    database_path:&std::path::Path, ticket_id:&str, expected_revision:i64,
    request_key:&str, summary:&str, parent_id:Option<&str>,
)->Result<String> {
    let store=super::open_headless_transition_store(database_path)?;
    let item=plan(&store,ticket_id,expected_revision,request_key,summary,parent_id)?;
    let ticket=super::get_ticket(&store,ticket_id)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId":item.ticket_id,"workItemId":item.id,"state":item.state,
        "parentId":item.parent_id,"revision":ticket.revision,
        "providerDispatched":false,"grantIssued":false,"externalEffectReplayed":false,
    })).map_err(|e|e.to_string())
}

/// A cancelled plan preserves the original WorkItem, immutable receipt and
/// audit; no Worker, external effect or Council review is dispatched.
pub(crate) fn headless_cancel(
    database_path:&std::path::Path, ticket_id:&str, expected_revision:i64,
    item_id:&str, request_key:&str,
)->Result<String> {
    let store=super::open_headless_transition_store(database_path)?;
    let item=cancel(&store,ticket_id,expected_revision,item_id,request_key)?;
    let ticket=super::get_ticket(&store,ticket_id)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "ticketId":item.ticket_id,"workItemId":item.id,"state":item.state,
        "revision":ticket.revision,
        "providerDispatched":false,"grantIssued":false,"externalEffectReplayed":false,
    })).map_err(|e|e.to_string())
}

#[cfg(test)]
mod headless_tests {
    use super::*;
    use crate::superagent::{self, NewOrchestration};

    #[test]
    fn headless_work_item_plan_and_cancel_share_native_revision_and_restart_safety() {
        let path=std::env::temp_dir().join(format!(
            "pandacode-headless-work-items-{}.sqlite3",storage::id(),
        ));
        let mut store=Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('project','Disposable','/tmp/headless-workitem-test')",[],
        ).unwrap();
        for id in ["router","worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config=superagent::create_configuration(&store,NewOrchestration {
            name:"Headless plans".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();
        let (ticket,launched)=superagent::admit_role_with_staging(
            &mut store,&config.id,"Plan without dispatch","headless-plan",false,false,true,
        ).unwrap();
        assert!(!launched);
        let assignments_before=superagent::assignments(&store,&ticket.id,100).unwrap().len();
        let root:serde_json::Value=serde_json::from_str(&headless_plan(
            &path,&ticket.id,ticket.revision,"root","Prepare change",None,
        ).unwrap()).unwrap();
        assert_eq!(root["state"],"planned");
        assert_eq!(root["providerDispatched"],false);
        assert_eq!(root["grantIssued"],false);
        let root_id=root["workItemId"].as_str().unwrap().to_owned();
        let replay:serde_json::Value=serde_json::from_str(&headless_plan(
            &path,&ticket.id,ticket.revision,"root","Prepare change",None,
        ).unwrap()).unwrap();
        assert_eq!(replay["workItemId"],root["workItemId"]);
        assert_eq!(replay["revision"],root["revision"]);
        assert!(headless_plan(&path,&ticket.id,ticket.revision,
            "stale","Change after revision",None).is_err());
        let next=root["revision"].as_i64().unwrap();
        let child:serde_json::Value=serde_json::from_str(&headless_plan(
            &path,&ticket.id,next,"child","Verify change",Some(&root_id),
        ).unwrap()).unwrap();
        assert_eq!(child["parentId"].as_str(),Some(root_id.as_str()));
        let current=child["revision"].as_i64().unwrap();
        assert!(headless_cancel(&path,&ticket.id,current,&root_id,"cancel-root").is_err());
        assert!(headless_cancel(&path,&ticket.id,current+1,
            child["workItemId"].as_str().unwrap(),"stale").is_err());
        let child_id=child["workItemId"].as_str().unwrap();
        let cancelled:serde_json::Value=serde_json::from_str(&headless_cancel(
            &path,&ticket.id,current,child_id,"cancel-child",
        ).unwrap()).unwrap();
        assert_eq!(cancelled["state"],"cancelled");
        let replay_cancel:serde_json::Value=serde_json::from_str(&headless_cancel(
            &path,&ticket.id,current,child_id,"cancel-child",
        ).unwrap()).unwrap();
        assert_eq!(replay_cancel["revision"],cancelled["revision"]);
        assert!(headless_cancel(&path,&ticket.id,current,child_id,"another-key").is_err());
        let root_cancel:serde_json::Value=serde_json::from_str(&headless_cancel(
            &path,&ticket.id,cancelled["revision"].as_i64().unwrap(),&root_id,"cancel-root",
        ).unwrap()).unwrap();
        assert_eq!(root_cancel["state"],"cancelled");
        assert_eq!(superagent::assignments(&store,&ticket.id,100).unwrap().len(),assignments_before);
        assert!(!superagent::mandate(&store,&ticket.id).unwrap().allowed_side_effects);
        drop(store);
        let reopened=Store::open_read_only(&path).unwrap();
        let plans=list(&reopened,&ticket.id,100).unwrap().0;
        assert_eq!(plans.len(),2);
        assert!(plans.iter().all(|item|item.state=="cancelled"));
    }

    #[test]
    fn desktop_and_headless_plan_payloads_share_bounded_strict_decoding() {
        let valid = r#"[{"requestKey":"root","summary":"Inspect"}]"#;
        assert_eq!(decode_plan(valid).unwrap()[0].request_key, "root");
        assert!(decode_plan(&" ".repeat(16 * 1024 + 1)).is_err());
        assert!(decode_plan(r#"[{"requestKey":"root","summary":"Inspect","allowedSideEffects":true}]"#).is_err());
        assert!(decode_plan("not JSON").is_err());
    }

    #[test]
    fn headless_dependency_plan_is_atomic_and_replay_does_not_refresh_authority() {
        let path = std::env::temp_dir().join(format!(
            "pandacode-headless-plan-batch-{}.sqlite3", storage::id(),
        ));
        let mut store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('project','Disposable','/tmp/headless-plan-batch')", [],
        ).unwrap();
        for id in ["router", "worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let config = superagent::create_configuration(&store, NewOrchestration {
            name: "Batch plans".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&config.id],
        ).unwrap();
        let (ticket, launched) = superagent::admit_role_with_staging(
            &mut store, &config.id, "Plan dependency work", "batch-plan", false, false, true,
        ).unwrap();
        assert!(!launched);
        let input = |key: &str, parent: Option<&str>| PlannedWorkItemInput {
            request_key: key.into(), summary: format!("Inspect {key}"),
            parent_request_key: parent.map(str::to_owned),
            worker_bot_id: None, routing_reason: None,
        };
        let invalid = [input("root", None), input("child", Some("foreign"))];
        assert!(headless_plan_batch(&path, &ticket.id, ticket.revision, &invalid).is_err());
        assert!(list(&store, &ticket.id, 100).unwrap().0.is_empty());
        assert_eq!(superagent::get_ticket(&store, &ticket.id).unwrap().revision, ticket.revision);
        let valid = [input("root", None), input("child", Some("root"))];
        // Desktop applies to the live store. Detached CLI replays the exact
        // same keys and must return the already committed native identities.
        let desktop = plan_batch(&store, &ticket.id, ticket.revision, &valid).unwrap();
        assert_eq!(desktop[1].parent_id.as_deref(), Some(desktop[0].id.as_str()));
        let result: serde_json::Value = serde_json::from_str(
            &headless_plan_batch(&path, &ticket.id, ticket.revision, &valid).unwrap(),
        ).unwrap();
        assert_eq!(result["items"][0]["workItemId"], desktop[0].id);
        assert_eq!(result["items"][1]["workItemId"], desktop[1].id);
        assert_eq!(result["items"][1]["parentId"], result["items"][0]["workItemId"]);
        assert_eq!(result["grantIssued"], false);
        assert_eq!(result["providerDispatched"], false);
        let revision = result["revision"].as_i64().unwrap();
        assert_eq!(revision, ticket.revision + 2);
        let replay: serde_json::Value = serde_json::from_str(
            &headless_plan_batch(&path, &ticket.id, ticket.revision, &valid).unwrap(),
        ).unwrap();
        assert_eq!(replay, result);
        let extension = [input("root", None), input("third", Some("root"))];
        assert!(headless_plan_batch(&path, &ticket.id, ticket.revision, &extension).is_err());
        let duplicate = [input("root", None), input("root", None)];
        assert!(headless_plan_batch(&path, &ticket.id, revision, &duplicate).is_err());
        assert_eq!(superagent::get_ticket(&store, &ticket.id).unwrap().revision, revision);
        assert!(superagent::assignments(&store, &ticket.id, 100).unwrap().is_empty());
        assert!(!superagent::mandate(&store, &ticket.id).unwrap().allowed_side_effects);
        drop(store);
        let reopened = Store::open_read_only(&path).unwrap();
        let items = list(&reopened, &ticket.id, 100).unwrap().0;
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.state == "planned"));
    }
}
