//! Durable idempotency receipts for future authorized SuperAgent external effects.
//! A transport timeout or restart never authorizes blind replay.
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

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
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    // This is not a grant: only independently authorized active mandates
    // may reserve effects. Current production mandates are read-only.
    let authorized: bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets t
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         WHERE t.id=?1 AND t.state='running' AND m.revoked=0
           AND m.expires_at>?2 AND m.policy_revision=o.policy_revision
           AND m.allowed_side_effects=1 AND o.enabled=1)",
        params![ticket_id,storage::now()],|r|r.get(0)
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
