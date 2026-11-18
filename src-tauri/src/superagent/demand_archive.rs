//! Reversible local-user Demand removal from ordinary Team boards.
//! Native Ticket, sessions, effects, mandates and evidence are never deleted.

use super::get_ticket;
use crate::{storage::{self, Store}, Result};
use rusqlite::params;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_demand_archives (
  ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  archived_at INTEGER NOT NULL CHECK(archived_at > 0),
  archived_by TEXT NOT NULL CHECK(archived_by = 'local-user')
);
"#;

pub(super) fn archived(store: &Store, ticket_id: &str) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_demand_archives WHERE ticket_id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())
}

/// Archive only settled top-level Demands. A Stop must be applied first when
/// work is active. Unknown/pending effects remain visible until reconciled.
/// No provider session, approval, audit row or immutable plan is removed.
pub(super) fn set(
    store: &Store, ticket_id: &str, expected_revision: i64, want_archived: bool,
) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let ticket = get_ticket(store, ticket_id)?;
    if ticket.revision != expected_revision {
        return Err("Demand changed; refresh its native revision before deleting or restoring.".into());
    }
    if !matches!(ticket.state.as_str(),
        "completed" | "failed" | "stopped" | "interrupted" | "inconclusive")
    {
        return Err("Cancel the Demand and settle its native tasks before deleting it.".into());
    }
    let child: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_work_items WHERE child_ticket_id=?1)
         OR EXISTS(SELECT 1 FROM sa_subagent_nodes WHERE child_ticket_id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if child {
        return Err("A delegated child Ticket cannot be deleted independently of its Demand.".into());
    }
    let current = archived(store, ticket_id)?;
    if current == want_archived {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    }
    if want_archived {
        let live: bool = tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sa_tickets t
               WHERE (t.id=?1
                 OR t.id IN (SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
                 OR t.id IN (SELECT child_ticket_id FROM sa_subagent_nodes WHERE root_ticket_id=?1))
                 AND (t.active_assignment_id IS NOT NULL
                   OR t.state IN ('queued','running','waiting_review','waiting_approval')
                   OR EXISTS(SELECT 1 FROM sa_assignments a
                             WHERE a.ticket_id=t.id AND a.state='running'))
             ) OR EXISTS(
               SELECT 1 FROM sa_subagent_nodes n WHERE n.root_ticket_id=?1
                 AND n.state IN ('pending','running','waiting_review')
             )",
            [ticket_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if live {
            return Err("Demand has live descendants or sessions; cancel and settle them first.".into());
        }
        let unresolved: bool = tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sa_effect_receipts e
               WHERE e.state IN ('pending','uncertain') AND (
                 e.ticket_id=?1
                 OR e.ticket_id IN (
                   SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
                 OR e.ticket_id IN (
                   SELECT child_ticket_id FROM sa_subagent_nodes WHERE root_ticket_id=?1))
             ) OR EXISTS(
               SELECT 1 FROM sa_worker_write_receipts receipt
               WHERE receipt.state IN ('reserved','uncertain') AND (
                 receipt.ticket_id=?1
                 OR receipt.ticket_id IN (
                   SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
                 OR receipt.ticket_id IN (
                   SELECT child_ticket_id FROM sa_subagent_nodes WHERE root_ticket_id=?1))
             ) OR EXISTS(
               SELECT 1 FROM sa_demand_test_runs run
               WHERE run.ticket_id=?1 AND run.state IN ('running','uncertain')
             ) OR EXISTS(
               SELECT 1 FROM sa_native_worker_test_receipts native
               WHERE native.state IN ('running','uncertain') AND (
                 native.ticket_id=?1
                 OR native.ticket_id IN (
                   SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
                 OR native.ticket_id IN (
                   SELECT child_ticket_id FROM sa_subagent_nodes WHERE root_ticket_id=?1))
             )",
            [ticket_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if unresolved {
            return Err("Demand has pending or uncertain external, write or native test effects. Reconcile them before deleting.".into());
        }
        let changed = tx.execute(
            "INSERT INTO sa_demand_archives(ticket_id,archived_at,archived_by)
             SELECT id,?2,'local-user' FROM sa_tickets
             WHERE id=?1 AND revision=?3
               AND state IN ('completed','failed','stopped','interrupted','inconclusive')",
            params![ticket_id,storage::now(),expected_revision],
        ).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Demand changed during archive; refresh before retrying.".into());
        }
        // Terminal Sessions cannot restart. Revoke any leftover expired or
        // forgotten grant without modifying the original evidence records.
        tx.execute(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1
             OR ticket_id IN (SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
             OR ticket_id IN (SELECT child_ticket_id FROM sa_subagent_nodes WHERE root_ticket_id=?1)",
            [ticket_id],
        ).map_err(|e| e.to_string())?;
    } else {
        let changed = tx.execute(
            "DELETE FROM sa_demand_archives WHERE ticket_id=?1",
            [ticket_id],
        ).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Demand archive state changed; refresh before restoring.".into());
        }
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,?2,?3,?4)",
        params![ticket_id,
          if want_archived {"demand_archived"} else {"demand_restored"},
          if want_archived {
              "local-user hid settled Demand; immutable tasks, chats and effects preserved"
          } else {
              "local-user restored Demand visibility; no mandate or session reactivated"
          },storage::now()],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
