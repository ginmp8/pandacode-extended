//! Immutable WorkItem evidence receipts for the executable native DAG.
//! Only a real completed child Ticket, matching Worker assignment and native
//! human/Council verdict may satisfy a dependency. This never runs a provider.
use super::*;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest,Sha256};

pub(crate) const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_work_item_evidence_receipts(
  item_id TEXT PRIMARY KEY REFERENCES sa_work_items(id) ON DELETE RESTRICT,
  child_ticket_id TEXT NOT NULL UNIQUE REFERENCES sa_tickets(id) ON DELETE RESTRICT,
  assignment_id TEXT NOT NULL UNIQUE REFERENCES sa_assignments(id) ON DELETE RESTRICT,
  evidence_sha256 TEXT NOT NULL CHECK(length(evidence_sha256)=64),
  approval_kind TEXT NOT NULL CHECK(approval_kind IN
    ('human_accepted','council_passed','council_human_accepted')),
  reviewed_at INTEGER NOT NULL CHECK(reviewed_at>0),
  recorded_at INTEGER NOT NULL CHECK(recorded_at>0)
);
CREATE TRIGGER IF NOT EXISTS sa_work_item_evidence_immutable
 BEFORE UPDATE ON sa_work_item_evidence_receipts
 BEGIN SELECT RAISE(ABORT,'WorkItem native approval receipt is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_work_item_evidence_no_delete
 BEFORE DELETE ON sa_work_item_evidence_receipts
 BEGIN SELECT RAISE(ABORT,'WorkItem native approval receipt cannot be deleted'); END;
"#;

pub(super) fn receipt(store:&Store,id:&str)->Result<Option<(String,String,i64)>> {
    store.db.query_row(
        "SELECT evidence_sha256,approval_kind,reviewed_at
         FROM sa_work_item_evidence_receipts WHERE item_id=?1",
        [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())
}

/// Runs under the existing native effect/dispatch fence. A child being marked
/// completed is not enough: its last finished assignment, output and verdict
/// must match in the same transaction before any dependent item may advance.
/// In a completed native child, every nested subagent must also have settled.
/// Pending descendants and un-reconciled effects cannot satisfy an upstream DAG edge.
fn descendant_receipts_settled(db:&Connection,child_ticket_id:&str)->Result<bool> {
    db.query_row(
        "WITH RECURSIVE descendants(ticket_id) AS (
           SELECT ?1
           UNION
           SELECT node.child_ticket_id FROM sa_subagent_nodes node
           JOIN descendants ancestor ON ancestor.ticket_id=node.parent_ticket_id
           WHERE node.child_ticket_id IS NOT NULL
         )
         SELECT NOT EXISTS(
           SELECT 1 FROM sa_subagent_nodes node
           JOIN descendants ancestor ON ancestor.ticket_id=node.parent_ticket_id
           WHERE node.state<>'completed'
           UNION ALL
           SELECT 1 FROM sa_effect_receipts effect
           JOIN descendants subtree ON subtree.ticket_id=effect.ticket_id
           WHERE effect.state IN ('pending','uncertain')
           UNION ALL
           SELECT 1 FROM sa_worker_write_receipts write_receipt
           JOIN descendants subtree ON subtree.ticket_id=write_receipt.ticket_id
           WHERE write_receipt.state IN ('reserved','uncertain'))",
        [child_ticket_id],|row|row.get(0),
    ).map_err(|error|error.to_string())
}

pub(super) fn record_accepted(store:&Store,now:i64)->Result<usize> {
    let candidates={
        let mut stmt=store.db.prepare(
            "SELECT item.id,item.child_ticket_id,child.revision,child.evidence,
                (SELECT assignment.id FROM sa_assignments assignment
                 WHERE assignment.ticket_id=child.id AND assignment.state='finished'
                   AND assignment.result=child.evidence
                   AND assignment.attempt=(SELECT MAX(latest.attempt) FROM sa_assignments latest
                     WHERE latest.ticket_id=child.id)
                 ORDER BY assignment.id LIMIT 1),
                (SELECT audit.kind FROM sa_audit audit WHERE audit.ticket_id=child.id
                   AND audit.kind IN ('human_accepted','council_passed','council_human_accepted')
                 ORDER BY audit.id DESC LIMIT 1),
                (SELECT audit.at FROM sa_audit audit WHERE audit.ticket_id=child.id
                   AND audit.kind IN ('human_accepted','council_passed','council_human_accepted')
                 ORDER BY audit.id DESC LIMIT 1)
             FROM sa_work_items item
             JOIN sa_tickets child ON child.id=item.child_ticket_id
             WHERE item.state IN ('running','waiting_review','blocked','completed')
               AND child.state='completed'
               AND child.evidence IS NOT NULL AND length(child.evidence)>0
               AND NOT EXISTS(SELECT 1 FROM sa_effect_receipts effect
                 WHERE effect.ticket_id=child.id AND effect.state IN ('pending','uncertain'))
               AND NOT EXISTS(SELECT 1 FROM sa_worker_write_receipts write_receipt
                 WHERE write_receipt.ticket_id=child.id
                   AND write_receipt.state IN ('reserved','uncertain'))
               AND NOT EXISTS(SELECT 1 FROM sa_subagent_nodes descendant
                 WHERE descendant.parent_ticket_id=child.id AND descendant.state<>'completed')
             ORDER BY item.ticket_id,item.ordinal,item.id",
        ).map_err(|e|e.to_string())?;
        let rows=stmt.query_map([],|r|Ok((
            r.get::<_,String>(0)?,r.get::<_,String>(1)?,
            r.get::<_,i64>(2)?,r.get::<_,String>(3)?,
            r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,
            r.get::<_,Option<i64>>(6)?,
        ))).map_err(|e|e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?
    };
    let mut completed=0;
    for (item,child,revision,result,assignment,kind,approved_at) in candidates {
        let (Some(assignment),Some(kind),Some(approved_at))=(assignment,kind,approved_at)
            else {continue};
        if approved_at<=0 || revision<1 {continue}
        let digest=format!("{:x}",Sha256::digest(result.as_bytes()));
        let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        if !descendant_receipts_settled(&tx,&child)? {continue}
        tx.execute(
            "INSERT OR IGNORE INTO sa_work_item_evidence_receipts
                (item_id,child_ticket_id,assignment_id,evidence_sha256,approval_kind,reviewed_at,recorded_at)
             SELECT ?1,?2,?3,?4,?5,?6,?7 WHERE EXISTS (
               SELECT 1 FROM sa_work_items item JOIN sa_tickets ticket
                 ON ticket.id=item.child_ticket_id
               JOIN sa_assignments native ON native.id=?3
               WHERE item.id=?1 AND item.child_ticket_id=?2
                 AND item.state IN ('running','waiting_review','blocked','completed')
                 AND ticket.state='completed' AND ticket.revision=?8 AND ticket.evidence=?9
                 AND native.ticket_id=ticket.id AND native.state='finished'
                 AND native.result=ticket.evidence
                 AND native.attempt=(SELECT MAX(a.attempt) FROM sa_assignments a
                   WHERE a.ticket_id=ticket.id)
                 AND (item.requires_write=0 OR EXISTS(
                   SELECT 1 FROM sa_worker_write_receipts applied
                   WHERE applied.ticket_id=ticket.id
                     AND applied.assignment_id=native.id
                     AND applied.state='applied'))
                 AND EXISTS(SELECT 1 FROM sa_audit audit WHERE audit.ticket_id=ticket.id
                   AND audit.kind=?5 AND audit.at=?6)
                 AND NOT EXISTS(SELECT 1 FROM sa_effect_receipts effect
                   WHERE effect.ticket_id=ticket.id AND effect.state IN ('pending','uncertain'))
                 AND NOT EXISTS(SELECT 1 FROM sa_worker_write_receipts write_receipt
                   WHERE write_receipt.ticket_id=ticket.id
                     AND write_receipt.state IN ('reserved','uncertain'))
                 AND NOT EXISTS(SELECT 1 FROM sa_subagent_nodes child
                   WHERE child.parent_ticket_id=ticket.id AND child.state<>'completed'))",
            params![item,child,assignment,digest,kind,approved_at,now,revision,result],
        ).map_err(|e|e.to_string())?;
        let exact:bool=tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts proof
              JOIN sa_work_items item ON item.id=proof.item_id
              WHERE proof.item_id=?1 AND proof.child_ticket_id=?2
                AND proof.assignment_id=?3 AND proof.evidence_sha256=?4
                AND proof.approval_kind=?5 AND proof.reviewed_at=?6
                AND (item.requires_write=0 OR EXISTS(
                  SELECT 1 FROM sa_worker_write_receipts applied
                  WHERE applied.ticket_id=proof.child_ticket_id
                    AND applied.assignment_id=proof.assignment_id
                    AND applied.state='applied')))",
            params![item,child,assignment,digest,kind,approved_at],
            |r|r.get(0),
        ).map_err(|e|e.to_string())?;
        if exact {
            completed+=tx.execute(
                "UPDATE sa_work_items SET state='completed',last_reason=NULL,updated_at=?2
                 WHERE id=?1 AND state IN ('running','waiting_review','blocked')",
                params![item,now],
            ).map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e|e.to_string())?;
    }
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture()->Store {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE sa_tickets(id TEXT PRIMARY KEY,state TEXT,evidence TEXT,revision INTEGER);
             CREATE TABLE sa_assignments(id TEXT PRIMARY KEY,ticket_id TEXT,
               state TEXT,attempt INTEGER,result TEXT);
             CREATE TABLE sa_work_items(id TEXT PRIMARY KEY,ticket_id TEXT,ordinal INTEGER,
               child_ticket_id TEXT,state TEXT,last_reason TEXT,updated_at INTEGER,
               requires_write INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE sa_audit(id INTEGER PRIMARY KEY,ticket_id TEXT,kind TEXT,at INTEGER);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT,state TEXT);
             CREATE TABLE sa_worker_write_receipts(ticket_id TEXT,state TEXT,assignment_id TEXT);
             CREATE TABLE sa_subagent_nodes(parent_ticket_id TEXT,child_ticket_id TEXT,state TEXT);
             INSERT INTO sa_tickets VALUES('child','completed','native output',2);
             INSERT INTO sa_assignments VALUES('attempt','child','finished',1,'native output');
             INSERT INTO sa_work_items VALUES('item','root',0,'child','waiting_review',NULL,1);",
        ).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        Store{db,path:std::path::PathBuf::from(":memory:")}
    }
    #[test]
    fn native_child_completion_cannot_unlock_a_dependency_without_verdict() {
        let store=fixture();
        assert_eq!(record_accepted(&store,100).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','human_accepted',99)",[],
        ).unwrap();
        store.db.execute("UPDATE sa_assignments SET result='unmatched' WHERE id='attempt'",[])
            .unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
    }
    #[test]
    fn native_evidence_receipt_is_idempotent_and_immutable() {
        let store=fixture();
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','human_accepted',99)",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),1);
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        let (digest,kind,at)=receipt(&store,"item").unwrap().unwrap();
        assert_eq!(digest,format!("{:x}",Sha256::digest(b"native output")));
        assert_eq!(kind,"human_accepted");assert_eq!(at,99);
        assert!(store.db.execute("DELETE FROM sa_work_item_evidence_receipts",[]).is_err());
        assert!(store.db.execute(
            "UPDATE sa_work_item_evidence_receipts SET reviewed_at=200",[],
        ).is_err());
    }
    #[test]
    fn locally_retried_blocked_child_can_produce_one_fresh_verified_result() {
        let store=fixture();
        store.db.execute(
            "UPDATE sa_work_items SET state='blocked' WHERE id='item'",[],
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','human_accepted',99)",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),1);
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_some());
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_work_items WHERE id='item'",[],
            |row|row.get::<_,String>(0),
        ).unwrap(),"completed");
    }

    #[test]
    fn unresolved_worker_file_writes_block_dependency_release_even_after_council_pass() {
        let store=fixture();
        store.db.execute_batch(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','council_passed',99);
             INSERT INTO sa_worker_write_receipts(ticket_id,state) VALUES('child','reserved');
             INSERT INTO sa_worker_write_receipts(ticket_id,state) VALUES('other','uncertain');",
        ).unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        store.db.execute(
            "UPDATE sa_worker_write_receipts SET state='uncertain'
             WHERE ticket_id='child'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        store.db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='child'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,102).unwrap(),1);
        assert_eq!(record_accepted(&store,103).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_some());
    }


    #[test]
    fn write_required_item_rejects_missing_stale_foreign_and_failed_write_receipts() {
        let store=fixture();
        store.db.execute_batch(
            "UPDATE sa_work_items SET requires_write=1 WHERE id='item';
             INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','council_passed',99);",
        ).unwrap();
        // A successful provider turn and Council verdict do not prove a file edit.
        assert_eq!(record_accepted(&store,100).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        store.db.execute_batch(
            "INSERT INTO sa_worker_write_receipts(ticket_id,state,assignment_id)
               VALUES('child','applied','earlier-attempt');
             INSERT INTO sa_worker_write_receipts(ticket_id,state,assignment_id)
               VALUES('unrelated','applied','attempt');
             INSERT INTO sa_worker_write_receipts(ticket_id,state,assignment_id)
               VALUES('child','failed','attempt');",
        ).unwrap();
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        // Only the actual native scoped-write ToolHost receipt for the final
        // successful Assignment may release this WorkItem's dependencies.
        store.db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='child' AND assignment_id='attempt'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,102).unwrap(),1);
        assert_eq!(record_accepted(&store,103).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_some());
    }


    #[test]
    fn historical_approval_receipt_cannot_launder_missing_scoped_write() {
        let store=fixture();
        store.db.execute_batch(
            "UPDATE sa_work_items SET requires_write=1 WHERE id='item';
             INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','council_passed',99);",
        ).unwrap();
        let digest=format!("{:x}",Sha256::digest(b"native output"));
        // Simulate a pre-fix approval receipt already stored for a WorkItem.
        store.db.execute(
            "INSERT INTO sa_work_item_evidence_receipts
             (item_id,child_ticket_id,assignment_id,evidence_sha256,
              approval_kind,reviewed_at,recorded_at)
             VALUES('item','child','attempt',?1,'council_passed',99,100)",
            params![digest],
        ).unwrap();
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        assert_eq!(store.db.query_row(
            "SELECT state FROM sa_work_items WHERE id='item'",[],
            |r|r.get::<_,String>(0),
        ).unwrap(),"waiting_review");
        store.db.execute(
            "INSERT INTO sa_worker_write_receipts(ticket_id,state,assignment_id)
             VALUES('child','applied','attempt')",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,102).unwrap(),1);
        assert_eq!(record_accepted(&store,103).unwrap(),0);
    }

    #[test]
    fn unresolved_github_effect_denies_workitem_proof() {
        let store=fixture();
        store.db.execute_batch(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','council_passed',99);
             INSERT INTO sa_effect_receipts VALUES('child','uncertain');",
        ).unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),0);
    }

    #[test]
    fn nested_subagent_reconciliation_blocks_workitem_dependency_proof() {
        let store=fixture();
        store.db.execute_batch(
            "INSERT INTO sa_audit(ticket_id,kind,at) VALUES('child','council_passed',99);
             INSERT INTO sa_subagent_nodes VALUES('child','nested','completed');
             INSERT INTO sa_subagent_nodes VALUES('nested','grandchild','waiting_review');
             INSERT INTO sa_subagent_nodes VALUES('other','other-child','completed');
             INSERT INTO sa_worker_write_receipts(ticket_id,state) VALUES('nested','reserved');
             INSERT INTO sa_effect_receipts VALUES('grandchild','uncertain');
             INSERT INTO sa_effect_receipts VALUES('other-child','uncertain');",
        ).unwrap();
        assert_eq!(record_accepted(&store,100).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_none());
        store.db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='nested'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,101).unwrap(),0);
        store.db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id='grandchild'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,102).unwrap(),0);
        store.db.execute(
            "UPDATE sa_subagent_nodes SET state='completed'
             WHERE child_ticket_id='grandchild'",[],
        ).unwrap();
        assert_eq!(record_accepted(&store,103).unwrap(),1);
        assert_eq!(record_accepted(&store,104).unwrap(),0);
        assert!(receipt(&store,"item").unwrap().is_some());
    }
}
