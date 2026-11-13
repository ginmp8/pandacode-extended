//! Durable idempotency receipts for future authorized SuperAgent external effects.
//! A transport timeout or restart never authorizes blind replay.
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use serde::Serialize;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_effect_receipts (
 ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 effect_key TEXT NOT NULL,
 payload_sha256 TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','uncertain','applied')),
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(ticket_id,effect_key)
);
CREATE INDEX IF NOT EXISTS sa_effect_unresolved ON sa_effect_receipts(ticket_id,state)
 WHERE state!='applied';
"#;
pub(crate) const RECOVER: &str =
    "UPDATE sa_effect_receipts SET state='uncertain',updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE state='pending';";

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub enum Reservation {
    Reserved,
    AlreadyPending,
    RequiresReconciliation,
    AlreadyApplied,
}

#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub fn reserve(store: &Store, ticket_id: &str, key: &str, payload: &[u8]) -> Result<Reservation> {
    if key.is_empty() || key.len()>128 || !key.bytes().all(|b|b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'|b':'))
        || payload.is_empty() || payload.len()>1_048_576 {
        return Err("Invalid or oversized effect identity.".into());
    }
    let digest=format!("{:x}",Sha256::digest(payload));
    // Global pause is a native revocation, not a UI-only switch. This
    // reservation guard is defense-in-depth; an eventual dispatcher must
    // revalidate the same authority immediately before the remote call.
    if crate::feature_controls::get().bots_paused {
        return Err("SuperAgent effects are disabled while bots are paused.".into());
    }
    let now = storage::now();
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    // A receipt is not a grant. Require the live worker assignment and every
    // immutable scope field to still agree before allowing a caller to attempt
    // an effect. Current production mandates are read-only.
    let authorized: bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets t
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         JOIN sa_assignments a ON a.id=t.active_assignment_id
           AND a.ticket_id=t.id AND a.state='running'
           AND a.bot_id=t.worker_bot_id
         JOIN bots b ON b.id=a.bot_id
         WHERE t.id=?1 AND t.state='running' AND m.revoked=0
           AND m.expires_at>?2
           AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision
           AND t.project_id=m.project_id AND m.project_id=o.project_id
           AND t.actor=m.actor AND m.authorization_id<>''
           AND b.project_id=t.project_id AND b.paused=0
           AND a.created_at>0 AND a.created_at<=?2
           AND (?2-a.created_at)<?3
           AND m.allowed_side_effects=1 AND o.enabled=1)",
        params![ticket_id,now,super::MAX_ASSIGNMENT_RUN_MS],|r|r.get(0)
    ).map_err(|e|e.to_string())?;
    if !authorized {return Err("Native effect authorization unavailable.".into());}
    let existing: Option<(String,String)>=tx.query_row(
        "SELECT payload_sha256,state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
        params![ticket_id,key],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    let result=match existing {
        Some((previous,_)) if previous!=digest =>
            return Err("Effect key already used for different payload.".into()),
        Some((_,state)) => match state.as_str(){
            "pending"=>Reservation::AlreadyPending,
            "uncertain"=>Reservation::RequiresReconciliation,
            "applied"=>Reservation::AlreadyApplied,
            _=>return Err("Invalid effect state.".into()),
        },
        None=>{
            let now=storage::now();
            tx.execute(
                "INSERT INTO sa_effect_receipts(ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'pending',?4,?4)",
                params![ticket_id,key,digest,now]
            ).map_err(|e|e.to_string())?;
            Reservation::Reserved
        }
    };
    tx.commit().map_err(|e|e.to_string())?;
    Ok(result)
}

#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub fn mark_uncertain(store:&Store,ticket_id:&str,key:&str)->Result<()>{
    let n=store.db.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?3
         WHERE ticket_id=?1 AND effect_key=?2 AND state IN ('pending','uncertain')",
        params![ticket_id,key,storage::now()]
    ).map_err(|e|e.to_string())?;
    if n!=1{return Err("No unresolved effect receipt.".into());}
    Ok(())
}

/// Caller must verify the external provider result independently: this
/// primitive neither executes an action nor infers success from a timeout.
#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub fn acknowledge_applied(store:&Store,ticket_id:&str,key:&str,payload:&[u8])->Result<()>{
    let digest=format!("{:x}",Sha256::digest(payload));
    let n=store.db.execute(
        "UPDATE sa_effect_receipts SET state='applied',updated_at=?4
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
        params![ticket_id,key,digest,storage::now()]
    ).map_err(|e|e.to_string())?;
    if n!=1{return Err("Missing receipt or mismatched payload.".into());}
    Ok(())
}

/// A bounded, task-owned operational projection of immutable effect identities.
/// This never performs a provider lookup, retries an effect, or authorizes a
/// pending receipt. Only the native task owner or local headless operator reads
/// it; external payload bytes and any provider responses are never returned.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EffectReceiptView {
    pub effect_key: String,
    pub payload_sha256: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) fn receipt_history(
    store: &Store, ticket_id: &str, limit: i64,
) -> Result<Vec<EffectReceiptView>> {
    if !(1..=100).contains(&limit) {
        return Err("Effect receipt page size must be between 1 and 100.".into());
    }
    let exists: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets WHERE id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !exists {
        return Err("SuperAgent Ticket not found.".into());
    }
    let mut stmt = store.db.prepare(
        "SELECT effect_key,payload_sha256,state,created_at,updated_at
         FROM sa_effect_receipts WHERE ticket_id=?1
         ORDER BY created_at DESC,effect_key DESC LIMIT ?2",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![ticket_id, limit], |row| Ok(EffectReceiptView {
        effect_key: row.get(0)?,
        payload_sha256: row.get(1)?,
        state: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

pub(crate) fn unresolved_count(store:&Store,ticket_id:&str)->Result<i64>{
    store.db.query_row(
        "SELECT COUNT(*) FROM sa_effect_receipts WHERE ticket_id=?1 AND state!='applied'",
        [ticket_id],|r|r.get(0)
    ).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::superagent::{self,NewOrchestration};

    fn fixture()->(Store,String){
        let path=std::env::temp_dir().join(format!("sa-effect-{}.db",storage::id()));
        let mut store=Store::open(&path).unwrap();
        store.db.execute("INSERT INTO projects(id,name,path) VALUES('p','P','/tmp/sa-effects')",[]).unwrap();
        for (id,name) in [("co","Coordinator"),("wo","Worker")]{
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,'p','codex','default','plan',?3,?3)",
                params![id,name,storage::now()]
            ).unwrap();
        }
        let cfg=superagent::create_configuration(&store,NewOrchestration{
            name:"Effect Test".into(),coordinator_bot_id:"co".into(),worker_bot_id:"wo".into()
        }).unwrap();
        store.db.execute("UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",[&cfg.id]).unwrap();
        let (ticket,started)=superagent::admit(&mut store,&cfg.id,"Safe work","receipt-fixture",false).unwrap();
        assert!(started);
        (store,ticket.id)
    }

    #[test]
    fn headless_receipt_inspection_reports_only_actual_unresolved_effects() {
        let (store,id) = fixture();
        let empty: serde_json::Value = serde_json::from_str(
            &crate::superagent::inspect_headless_effect_receipts(&store.path,&id).unwrap(),
        ).unwrap();
        assert_eq!(empty["requiresIndependentRemoteReconciliation"], false);
        assert_eq!(empty["observationalOnly"], true);
        assert_eq!(empty["replayed"], false);
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:42",b"payload").unwrap(),Reservation::Reserved);
        let pending: serde_json::Value = serde_json::from_str(
            &crate::superagent::inspect_headless_effect_receipts(&store.path,&id).unwrap(),
        ).unwrap();
        assert_eq!(pending["requiresIndependentRemoteReconciliation"], true);
        assert_eq!(pending["receipts"][0]["state"], "pending");
        acknowledge_applied(&store,&id,"github:issue:42",b"payload").unwrap();
        let applied: serde_json::Value = serde_json::from_str(
            &crate::superagent::inspect_headless_effect_receipts(&store.path,&id).unwrap(),
        ).unwrap();
        assert_eq!(applied["totalUnresolved"], 0);
        assert_eq!(applied["requiresIndependentRemoteReconciliation"], false);
    }

    #[test]
    fn headless_reconciliation_indicator_counts_unresolved_effects_beyond_history_page() {
        let (store, id) = fixture();
        // The read-only CLI returns at most 100 newest receipts; an older
        // uncertain effect must still block a false "nothing to reconcile".
        for index in 0..=100 {
            let state = if index == 0 { "uncertain" } else { "applied" };
            let digest = format!("{index:064x}");
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?5)",
                params![&id,format!("effect:{index:03}"),digest,state,1_000+index],
            ).unwrap();
        }
        let read = |path: &std::path::Path| -> serde_json::Value {
            serde_json::from_str(
                &superagent::inspect_headless_effect_receipts(path, &id).unwrap(),
            ).unwrap()
        };
        let status = read(&store.path);
        assert_eq!(status["receipts"].as_array().unwrap().len(), 100);
        assert!(status["receipts"].as_array().unwrap().iter()
            .all(|receipt| receipt["state"] == "applied"));
        assert_eq!(status["totalUnresolved"], 1);
        assert_eq!(status["requiresIndependentRemoteReconciliation"], true);
        assert_eq!(status["observationalOnly"], true);
        assert_eq!(status["replayed"], false);
        let path = store.path.clone();
        drop(store);
        // Inspection cannot mutate receipts; a process restart preserves
        // the older uncertain identity rather than replaying it.
        let reopened = Store::open(&path).unwrap();
        let after_restart = read(&path);
        assert_eq!(after_restart["totalUnresolved"], 1);
        assert_eq!(after_restart["requiresIndependentRemoteReconciliation"], true);
        assert_eq!(unresolved_count(&reopened, &id).unwrap(), 1);
    }

    #[test]
    fn receipt_history_is_bounded_task_owned_and_survives_restart() {
        let (store, id) = fixture();
        assert!(receipt_history(&store, &id, 0).is_err());
        assert!(receipt_history(&store, &id, 101).is_err());
        assert!(receipt_history(&store, "missing", 10).is_err());
        assert!(receipt_history(&store, &id, 10).unwrap().is_empty());
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        assert_eq!(reserve(&store, &id, "github:issue:8", b"secret body").unwrap(), Reservation::Reserved);
        let first = receipt_history(&store, &id, 1).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].effect_key, "github:issue:8");
        assert_eq!(first[0].state, "pending");
        let json = serde_json::to_string(&first).unwrap();
        assert!(!json.contains("secret body"));
        assert_eq!(unresolved_count(&store, &id).unwrap(), 1);
        let path = store.path.clone();
        drop(store);
        // Read-only headless inspection must not change an in-flight receipt.
        let read = Store::open_read_only(&path).unwrap();
        assert_eq!(receipt_history(&read, &id, 10).unwrap()[0].state, "pending");
        drop(read);
        let recovered = Store::open(&path).unwrap();
        assert_eq!(receipt_history(&recovered, &id, 10).unwrap()[0].state, "uncertain");
        assert_eq!(unresolved_count(&recovered, &id).unwrap(), 1);
        // A second task must not be able to inspect the original receipt under
        // its own ID; unknown task IDs cannot return an empty pseudo-history.
        assert!(receipt_history(&recovered, "not-a-task", 10).is_err());
    }

    #[test]
    fn receipt_never_grants_write_and_prevents_mismatched_or_uncertain_replay(){
        let (store,id)=fixture();
        assert!(reserve(&store,&id,"github:comment:7",b"payload").is_err());
        assert_eq!(unresolved_count(&store,&id).unwrap(),0);
        // Disposable fixture explicitly exercises the native receipt primitive;
        // actual SuperAgent production grants remain read-only.
        store.db.execute("UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id]).unwrap();
        assert_eq!(reserve(&store,&id,"github:comment:7",b"payload").unwrap(),Reservation::Reserved);
        assert_eq!(reserve(&store,&id,"github:comment:7",b"payload").unwrap(),Reservation::AlreadyPending);
        assert!(reserve(&store,&id,"github:comment:7",b"other").is_err());
        mark_uncertain(&store,&id,"github:comment:7").unwrap();
        assert_eq!(reserve(&store,&id,"github:comment:7",b"payload").unwrap(),Reservation::RequiresReconciliation);
        assert!(acknowledge_applied(&store,&id,"github:comment:7",b"other").is_err());
        acknowledge_applied(&store,&id,"github:comment:7",b"payload").unwrap();
        assert_eq!(reserve(&store,&id,"github:comment:7",b"payload").unwrap(),Reservation::AlreadyApplied);
        assert_eq!(unresolved_count(&store,&id).unwrap(),0);
        store.db.execute("UPDATE sa_mandates SET revoked=1 WHERE ticket_id=?1",[&id]).unwrap();
        assert!(reserve(&store,&id,"github:comment:8",b"payload").is_err());
    }

    #[test]
    fn effect_reservation_denies_expired_or_future_assignment_attempts() {
        for started_at in [
            storage::now() - crate::superagent::MAX_ASSIGNMENT_RUN_MS - 1000,
            storage::now() + 60_000,
        ] {
            let (store,id)=fixture();
            // The fixture simulates a native grant only for the receipt
            // primitive. No production API may issue this write authority.
            store.db.execute(
                "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
                [&id],
            ).unwrap();
            store.db.execute(
                "UPDATE sa_assignments SET created_at=?2 WHERE ticket_id=?1 AND state='running'",
                params![&id,started_at],
            ).unwrap();
            assert!(reserve(&store,&id,"github:issue:late",b"body").is_err());
            assert_eq!(unresolved_count(&store,&id).unwrap(),0);
        }
        let (store,id)=fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:valid",b"body").unwrap(),
            Reservation::Reserved);
    }

    #[test]
    fn effect_reservation_denies_stale_or_mismatched_mandate_scope(){
        for (change, expected) in [
            ("UPDATE sa_mandates SET actor='other' WHERE ticket_id=?1", "actor"),
            ("UPDATE sa_mandates SET project_id='other' WHERE ticket_id=?1", "project"),
            ("UPDATE sa_mandates SET policy_revision=policy_revision+1 WHERE ticket_id=?1", "revision"),
            ("UPDATE sa_orchestrations SET policy_revision=policy_revision+1 WHERE id=(SELECT orchestration_id FROM sa_tickets WHERE id=?1)", "policy"),
        ] {
            let (store,id)=fixture();
            store.db.execute("UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id]).unwrap();
            store.db.execute(change,[&id]).unwrap();
            assert!(reserve(&store,&id,"github:issue:stale",b"body").is_err(), "stale {expected} must deny effect reservation");
            assert_eq!(unresolved_count(&store,&id).unwrap(),0, "denial must not persist a receipt");
        }

        let (store,id)=fixture();
        store.db.execute("UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id]).unwrap();
        store.db.execute("UPDATE sa_assignments SET state='finished' WHERE ticket_id=?1",[&id]).unwrap();
        assert!(reserve(&store,&id,"github:issue:finished",b"body").is_err());
        assert_eq!(unresolved_count(&store,&id).unwrap(),0);
    }

    #[test]
    fn process_restart_marks_inflight_effect_uncertain_without_replay(){
        let (store,id)=fixture();
        store.db.execute("UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id]).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:9",b"body").unwrap(),Reservation::Reserved);
        let path=store.path.clone();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        let status:String=reopened.db.query_row(
            "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key='github:issue:9'",
            [&id],|r|r.get(0)
        ).unwrap();
        assert_eq!(status,"uncertain");
        assert_eq!(unresolved_count(&reopened,&id).unwrap(),1);
        assert!(reserve(&reopened,&id,"github:issue:9",b"body").is_err());
        drop(reopened);
        let again=Store::open(&path).unwrap();
        assert_eq!(unresolved_count(&again,&id).unwrap(),1);
    }
}
