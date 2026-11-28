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
/// Called inside Store::open's native restart transaction. Write one
/// attribution event per Ticket *before* quarantining pending effects, so
/// failed audit insertion rolls back the quarantine and retry remains safe.
/// Distinct receipt identities and payload digests stay in sa_effect_receipts;
/// never persist raw payload or imply a remote failure/success here.
pub(crate) const RECOVER: &str = r#"
INSERT INTO sa_audit(ticket_id,kind,detail,at)
 SELECT DISTINCT ticket_id,'restart_effect_uncertain',
   'Pending external effects quarantined on restart; verify remotely before reconciliation',
   CAST(strftime('%s','now') AS INTEGER)*1000
 FROM sa_effect_receipts WHERE state='pending';
UPDATE sa_effect_receipts SET state='uncertain',
 updated_at=CAST(strftime('%s','now') AS INTEGER)*1000
 WHERE state='pending';
"#;

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
    let active_worker: Option<(String, String, i64, String, String)> = tx.query_row(
        "SELECT a.session_id,a.bot_id,a.attempt,o.coordinator_bot_id,t.worker_bot_id
         FROM sa_tickets t
         JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations o ON o.id=t.orchestration_id
         JOIN sa_assignments a ON a.id=t.active_assignment_id
           AND a.ticket_id=t.id AND a.state='running'
         JOIN sa_assignment_worker_authority authority ON authority.assignment_id=a.id
           AND authority.worker_bot_id=a.bot_id
         JOIN sessions s ON s.id=a.session_id AND s.project_id=t.project_id
         JOIN bots b ON b.id=a.bot_id
         WHERE t.id=?1 AND t.state='running' AND m.revoked=0
           AND m.expires_at>?2
           AND t.policy_revision=m.policy_revision
           AND m.policy_revision=o.policy_revision
           AND t.project_id=m.project_id AND m.project_id=o.project_id
           AND t.actor=m.actor AND length(trim(m.authorization_id))>0
           AND b.project_id=t.project_id AND b.paused=0
           AND EXISTS(SELECT 1 FROM bots coordinator
                      WHERE coordinator.id=o.coordinator_bot_id
                        AND coordinator.paused=0)
           AND a.bot_id<>o.coordinator_bot_id
           AND a.created_at>0 AND a.created_at<=?2
           AND (?2-a.created_at)<?3
           AND (SELECT COUNT(*) FROM sa_assignments other
                WHERE other.ticket_id=t.id AND other.state='running')=1
           AND m.allowed_side_effects=1 AND o.enabled=1",
        params![ticket_id,now,super::MAX_ASSIGNMENT_RUN_MS],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((session_id,bot_id,attempt,coordinator_id,worker_id))=active_worker else {
        return Err("Native effect authorization unavailable.".into());
    };
    // Receipts do not authorize a remote call. Before any future dispatch,
    // require the original task-owned thread, role and admitted session
    // profile, not merely a matching bot ID or an unrevoked mandate.
    let session=store.session(&session_id)?;
    if !super::assignment_thread_current(store,&session_id,ticket_id)?
        || !super::assignment_role_authorized(
            store,ticket_id,&bot_id,attempt,&coordinator_id,&worker_id,
        )?
        || !super::admitted_session_matches_profile(
            store,ticket_id,&bot_id,"worker",&session,
        )?
    {
        return Err("Native effect authorization unavailable: worker session changed.".into());
    }
    let existing: Option<(String,String)>=tx.query_row(
        "SELECT payload_sha256,state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
        params![ticket_id,key],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    // A pending receipt may already have reached the provider: until its
    // outcome is independently verified it is just as unsafe to sequence
    // another action as an explicitly uncertain receipt. A same-key retry
    // is inspection only and cannot create a second receipt.
    if existing.is_none() {
        let unresolved_other: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key<>?2 AND state IN ('pending','uncertain'))",
            params![ticket_id,key], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if unresolved_other {
            return Err("Another external effect is pending or uncertain; reconcile it before reserving new effects.".into());
        }
    }
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
            // The reservation is not usable without a durable, task-owned
            // audit record. If audit persistence fails, roll back the receipt
            // in this same transaction rather than permit an untraceable call.
            tx.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'effect_reserved',?2,?3)",
                params![ticket_id,
                    format!("Effect {key} reserved; provider outcome not confirmed"),
                    now],
            ).map_err(|e|e.to_string())?;
            Reservation::Reserved
        }
    };
    tx.commit().map_err(|e|e.to_string())?;
    Ok(result)
}

#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub fn mark_uncertain(store:&Store,ticket_id:&str,key:&str)->Result<()>{
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let now=storage::now();
    let n=tx.execute(
        "UPDATE sa_effect_receipts SET state='uncertain',updated_at=?3
         WHERE ticket_id=?1 AND effect_key=?2 AND state='pending'",
        params![ticket_id,key,now],
    ).map_err(|e|e.to_string())?;
    if n==1 {
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'effect_uncertain',?2,?3)",
            params![ticket_id,
                format!("Effect {key} outcome uncertain; independent remote reconciliation required"),
                now],
        ).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(());
    }
    // A duplicate timeout must not rewrite the original uncertainty time
    // or add another audit event. Applied receipts never go backwards.
    let state:Option<String>=tx.query_row(
        "SELECT state FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
        params![ticket_id,key],|r|r.get(0),
    ).optional().map_err(|e|e.to_string())?;
    if state.as_deref()==Some("uncertain") {
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(());
    }
    Err("No unresolved effect receipt.".into())
}

/// Caller must verify the external provider result independently: this
/// primitive neither executes an action nor infers success from a timeout.
#[cfg_attr(not(test), expect(dead_code, reason = "Reserved for authenticated external effects; native production grants remain read-only."))]
pub fn acknowledge_applied(store:&Store,ticket_id:&str,key:&str,payload:&[u8])->Result<()>{
    let digest=format!("{:x}",Sha256::digest(payload));
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let now=storage::now();
    let n=tx.execute(
        "UPDATE sa_effect_receipts SET state='applied',updated_at=?4
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3
           AND state IN ('pending','uncertain')",
        params![ticket_id,key,digest,now],
    ).map_err(|e|e.to_string())?;
    if n==1 {
        // The caller, not this storage primitive, must independently verify
        // the provider result. This audit records the transition, not proof
        // that a provider check actually took place.
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'effect_applied',?2,?3)",
            params![ticket_id,
                format!("Effect {key} marked applied by reconciliation caller; provider verification is caller responsibility"),
                now],
        ).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(());
    }
    // Repeated reconciliation is a no-op for the same payload only. Failed
    // attempts never produce a fabricated applied audit event.
    let state:Option<String>=tx.query_row(
        "SELECT state FROM sa_effect_receipts
         WHERE ticket_id=?1 AND effect_key=?2 AND payload_sha256=?3",
        params![ticket_id,key,digest],|r|r.get(0),
    ).optional().map_err(|e|e.to_string())?;
    if state.as_deref()==Some("applied") {
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(());
    }
    Err("Missing receipt or mismatched payload.".into())
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

pub(crate) type ReceiptPage = (Vec<EffectReceiptView>, Option<(i64, String)>);

pub(crate) fn receipt_history(
    store: &Store, ticket_id: &str, limit: i64,
) -> Result<Vec<EffectReceiptView>> {
    Ok(receipt_history_page(store, ticket_id, limit, None, false)?.0)
}

/// Stable keyset pagination over task-owned receipts. A timestamp alone
/// cannot disambiguate effects recorded within the same millisecond.
pub(crate) fn receipt_history_page(
    store: &Store, ticket_id: &str, limit: i64,
    before: Option<(i64, &str)>, unresolved_only: bool,
) -> Result<ReceiptPage> {
    if !(1..=100).contains(&limit) {
        return Err("Effect receipt page size must be between 1 and 100.".into());
    }
    if let Some((created_at, key)) = before {
        if created_at <= 0 || key.is_empty() || key.len() > 128
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        {
            return Err("Invalid effect receipt cursor.".into());
        }
    }
    let exists: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets WHERE id=?1)",
        [ticket_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !exists {
        return Err("SuperAgent Ticket not found.".into());
    }
    if let Some((created_at, key)) = before {
        // A well-formed cursor cannot claim another Ticket's receipt. If an
        // unresolved page boundary was subsequently reconciled, allow its
        // immutable identity only when native effect_applied audit proves the
        // transition. Synthetic applied receipts cannot skip unresolved work.
        let anchored: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_effect_receipts r
             WHERE r.ticket_id=?1 AND r.created_at=?2 AND r.effect_key=?3
               AND (?4=0 OR r.state IN ('pending','uncertain')
                 OR (r.state='applied' AND EXISTS(
                   SELECT 1 FROM sa_audit a
                   WHERE a.ticket_id=r.ticket_id AND a.kind='effect_applied'
                     AND a.detail='Effect ' || r.effect_key ||
                       ' marked applied by reconciliation caller; provider verification is caller responsibility'
                 ))))",
            params![ticket_id, created_at, key, i64::from(unresolved_only)],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if !anchored {
            return Err("Effect receipt cursor is not visible to this Ticket and filter.".into());
        }
    }
    let (before_at, before_key) = match before {
        Some((created_at, key)) => (Some(created_at), Some(key)),
        None => (None, None),
    };
    let mut stmt = store.db.prepare(
        "SELECT effect_key,payload_sha256,state,created_at,updated_at
         FROM sa_effect_receipts
         WHERE ticket_id=?1
           AND (?2 IS NULL OR created_at<?2 OR (created_at=?2 AND effect_key<?3))
           AND (?4=0 OR state!='applied')
         ORDER BY created_at DESC,effect_key DESC LIMIT ?5",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(
        params![ticket_id,before_at,before_key,i64::from(unresolved_only),limit+1],
        |row| Ok(EffectReceiptView {
            effect_key: row.get(0)?,
            payload_sha256: row.get(1)?,
            state: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }),
    ).map_err(|e| e.to_string())?;
    let mut receipts = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let next_cursor = if receipts.len() > limit as usize {
        receipts.truncate(limit as usize);
        receipts.last().map(|item| (item.created_at, item.effect_key.clone()))
    } else {
        None
    };
    Ok((receipts, next_cursor))
}

/// A cross-Ticket native reconciliation queue for a trusted local operator.
/// Listing pending/uncertain identities never acknowledges or retries them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnresolvedEffectView {
    pub ticket_id: String,
    pub orchestration_id: String,
    pub ticket_revision: i64,
    pub effect_key: String,
    pub payload_sha256: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) type UnresolvedEffectPage = (Vec<UnresolvedEffectView>, Option<(i64, String, String)>);

pub(crate) fn unresolved_effects_page(
    store: &Store, limit: i64, before: Option<(i64, &str, &str)>,
) -> Result<UnresolvedEffectPage> {
    unresolved_effects_page_for_team(store, limit, before, None)
}

/// Team-scoped recovery is a filtered native projection, never a grant to
/// reconcile or replay an effect from another Team.
/// Order by immutable creation identity rather than mutable updated_at.
/// A pending -> uncertain transition during pagination must not move an unseen
/// receipt ahead of the cursor and silently omit it from the recovery inbox.
pub(crate) fn unresolved_effects_page_for_team(
    store: &Store, limit: i64, before: Option<(i64, &str, &str)>,
    orchestration_id: Option<&str>,
) -> Result<UnresolvedEffectPage> {
    if let Some(id) = orchestration_id {
        if id.is_empty() || id.len() > 128
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err("Invalid Team identity for unresolved effects.".into());
        }
        let exists: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_orchestrations WHERE id=?1)",
            [id], |row| row.get(0),
        ).map_err(|err| err.to_string())?;
        if !exists { return Err("SuperAgent Team not found.".into()); }
    }
    if !(1..=100).contains(&limit) {
        return Err("Unresolved effect page size must be between 1 and 100.".into());
    }
    if let Some((at, ticket_id, effect_key)) = before {
        if at <= 0 || ticket_id.is_empty() || ticket_id.len() > 128
            || !ticket_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || effect_key.is_empty() || effect_key.len() > 128
            || !effect_key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        {
            return Err("Invalid unresolved effect cursor.".into());
        }
    }
    if let Some((at, ticket_id, effect_key)) = before {
        // Keep a page boundary valid when its effect is genuinely reconciled
        // between reads, but reject synthetic applied receipts without the
        // native task-bound audit and all cross-Team or fabricated cursors.
        let anchored: bool = store.db.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sa_effect_receipts r
               JOIN sa_tickets t ON t.id=r.ticket_id
               WHERE r.created_at=?1 AND r.ticket_id=?2 AND r.effect_key=?3
                 AND (r.state IN ('pending','uncertain') OR
                   (r.state='applied' AND EXISTS(
                     SELECT 1 FROM sa_audit a
                     WHERE a.ticket_id=r.ticket_id AND a.kind='effect_applied'
                       AND a.detail='Effect ' || r.effect_key ||
                         ' marked applied by reconciliation caller; provider verification is caller responsibility'
                   )))
                 AND (?4 IS NULL OR t.orchestration_id=?4)
             )",
            params![at, ticket_id, effect_key, orchestration_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if !anchored {
            return Err("Unresolved effect cursor is not visible in this recovery queue.".into());
        }
    }
    let (before_at, before_ticket, before_key) = match before {
        Some((at, ticket, key)) => (Some(at), Some(ticket), Some(key)),
        None => (None, None, None),
    };
    let mut stmt = store.db.prepare(
        "SELECT r.ticket_id,t.orchestration_id,t.revision,r.effect_key,r.payload_sha256,
                r.state,r.created_at,r.updated_at
         FROM sa_effect_receipts r
         JOIN sa_tickets t ON t.id=r.ticket_id
         WHERE r.state IN ('pending','uncertain')
           AND (?5 IS NULL OR t.orchestration_id=?5)
           AND (?1 IS NULL OR r.created_at<?1
                OR (r.created_at=?1 AND (r.ticket_id<?2
                     OR (r.ticket_id=?2 AND r.effect_key<?3))))
         ORDER BY r.created_at DESC,r.ticket_id DESC,r.effect_key DESC LIMIT ?4",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(
        params![before_at,before_ticket,before_key,limit+1,orchestration_id],
        |r| Ok(UnresolvedEffectView {
            ticket_id: r.get(0)?, orchestration_id: r.get(1)?,
            ticket_revision: r.get(2)?, effect_key: r.get(3)?,
            payload_sha256: r.get(4)?, state: r.get(5)?,
            created_at: r.get(6)?, updated_at: r.get(7)?,
        }),
    ).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    let next = if items.len()>limit as usize {
        items.truncate(limit as usize);
        items.last().map(|r| (r.created_at,r.ticket_id.clone(),r.effect_key.clone()))
    } else { None };
    Ok((items,next))
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
    fn unresolved_effect_cursor_does_not_skip_receipt_when_remote_outcome_becomes_uncertain() {
        let (store, ticket_id) = fixture();
        for index in 0..110_i64 {
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'uncertain',?4,?4)",
                params![&ticket_id,format!("effect:{index:03}"),
                    format!("{index:064x}"),10_000+index],
            ).unwrap();
        }
        let first:serde_json::Value = serde_json::from_str(
            &superagent::list_headless_unresolved_effects(
                &store.path,None,None,None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(),100);
        let cursor = &first["nextCursor"];
        assert_eq!(cursor["createdAt"],cursor["updatedAt"],
            "old clients may still round-trip the legacy opaque cursor field");
        // A real timeout/restart changes updated_at, not receipt identity.
        // The unseen effect would move before the first page's updated_at
        // boundary and be lost if recovery pagination sorted by that field.
        store.db.execute(
            "UPDATE sa_effect_receipts SET updated_at=99999999
             WHERE ticket_id=?1 AND effect_key='effect:005'",
            [&ticket_id],
        ).unwrap();
        let second:serde_json::Value = serde_json::from_str(
            &superagent::list_headless_unresolved_effects(
                &store.path,cursor["createdAt"].as_i64(),
                cursor["ticketId"].as_str(),cursor["effectKey"].as_str(),
            ).unwrap(),
        ).unwrap();
        let keys = first["items"].as_array().unwrap().iter()
            .chain(second["items"].as_array().unwrap())
            .map(|row| row["effectKey"].as_str().unwrap().to_owned())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(second["items"].as_array().unwrap().len(),10);
        assert_eq!(keys.len(),110);
        assert!(keys.contains("effect:005"),
            "unseen receipt must remain discoverable after uncertainty update");
        assert!(second["items"].as_array().unwrap().iter().any(|row|
            row["effectKey"]=="effect:005" && row["updatedAt"]==99999999));
        // A previous headless client following its opaque updatedAt cursor
        // alias must see the identical stable second page.
        let legacy:serde_json::Value = serde_json::from_str(
            &superagent::list_headless_unresolved_effects(
                &store.path,cursor["updatedAt"].as_i64(),
                cursor["ticketId"].as_str(),cursor["effectKey"].as_str(),
            ).unwrap(),
        ).unwrap();
        assert_eq!(legacy["items"],second["items"]);
        assert_eq!(second["observationalOnly"],true);
        assert_eq!(second["automaticReplayAllowed"],false);
    }


    #[test]
    fn audited_reconciliation_keeps_unresolved_recovery_cursors_stable() {
        let (store, ticket_id) = fixture();
        let team_id: String = store.db.query_row(
            "SELECT orchestration_id FROM sa_tickets WHERE id=?1",
            [&ticket_id], |row| row.get(0),
        ).unwrap();
        let confirmed_payload = b"verified-by-independent-test-caller";
        let native_digest = format!("{:x}", Sha256::digest(confirmed_payload));
        for index in 0..102_i64 {
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'uncertain',?4,?4)",
                params![&ticket_id, format!("effect:{index:03}"),
                    if index == 2 {native_digest.clone()} else {format!("{index:064x}")},
                    10_000 + index],
            ).unwrap();
        }
        let (first_global, global_cursor) = unresolved_effects_page(&store, 100, None).unwrap();
        let (first_team, team_cursor) = unresolved_effects_page_for_team(
            &store, 100, None, Some(&team_id),
        ).unwrap();
        let (first_ticket, ticket_cursor) = receipt_history_page(
            &store, &ticket_id, 100, None, true,
        ).unwrap();
        assert_eq!(first_global.len(), 100);
        assert_eq!(first_team.len(), 100);
        assert_eq!(first_ticket.len(), 100);
        let global_cursor = global_cursor.unwrap();
        let team_cursor = team_cursor.unwrap();
        let ticket_cursor = ticket_cursor.unwrap();
        assert_eq!(global_cursor.2, "effect:002");
        assert_eq!(team_cursor.2, "effect:002");
        assert_eq!(ticket_cursor.1, "effect:002");

        // This test-only caller explicitly simulates independent verification;
        // the receipt primitive does not execute or verify any remote effect.
        acknowledge_applied(&store, &ticket_id, "effect:002", confirmed_payload).unwrap();
        assert_eq!(unresolved_count(&store, &ticket_id).unwrap(), 101);
        let (global_rest, global_next) = unresolved_effects_page(
            &store, 100, Some((
                global_cursor.0, &global_cursor.1, &global_cursor.2,
            )),
        ).unwrap();
        let (team_rest, team_next) = unresolved_effects_page_for_team(
            &store, 100, Some((
                team_cursor.0, &team_cursor.1, &team_cursor.2,
            )), Some(&team_id),
        ).unwrap();
        let (ticket_rest, ticket_next) = receipt_history_page(
            &store, &ticket_id, 100, Some((
                ticket_cursor.0, &ticket_cursor.1,
            )), true,
        ).unwrap();
        assert!(global_next.is_none() && team_next.is_none() && ticket_next.is_none());
        assert_eq!(global_rest.len(), 2);
        assert_eq!(team_rest.len(), 2);
        assert_eq!(global_rest[0].effect_key, "effect:001");
        assert_eq!(global_rest[1].effect_key, "effect:000");
        assert_eq!(team_rest[0].effect_key, "effect:001");
        assert_eq!(team_rest[1].effect_key, "effect:000");
        assert_eq!(ticket_rest.len(), 2);
        assert_eq!(ticket_rest[0].effect_key, "effect:001");
        assert_eq!(ticket_rest[1].effect_key, "effect:000");

        // An applied receipt without a matching native reconciliation audit is
        // not a valid boundary, even if its Ticket and timestamp are genuine.
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'synthetic-applied',?2,'applied',9999,9999)",
            params![&ticket_id, "f".repeat(64)],
        ).unwrap();
        assert!(unresolved_effects_page_for_team(
            &store, 100, Some((9999, &ticket_id, "synthetic-applied")),
            Some(&team_id),
        ).is_err());
        assert!(unresolved_effects_page(
            &store, 100, Some((9999, &ticket_id, "synthetic-applied")),
        ).is_err());
        let reopened = Store::open_read_only(&store.path).unwrap();
        let (after_reopen, _) = unresolved_effects_page_for_team(
            &reopened, 100, Some((
                team_cursor.0, &team_cursor.1, &team_cursor.2,
            )), Some(&team_id),
        ).unwrap();
        assert_eq!(after_reopen, team_rest);
        assert_eq!(unresolved_count(&reopened, &ticket_id).unwrap(), 101);
    }

    #[test]
    fn global_unresolved_effect_inbox_is_complete_read_only_and_restart_safe() {
        let (store, id) = fixture();
        store.db.execute(
            "INSERT INTO sa_tickets
             (id,orchestration_id,request_key,objective,actor,source,project_id,
              worker_bot_id,policy_revision,state,created_at,updated_at)
             SELECT 'other-effect-ticket',orchestration_id,'other-effect-request',
                    'PRIVATE OBJECTIVE',actor,source,project_id,worker_bot_id,
                    policy_revision,'blocked',created_at,updated_at
             FROM sa_tickets WHERE id=?1",
            [&id],
        ).unwrap();
        for index in 0..205_i64 {
            let ticket = if index % 2 == 0 { id.as_str() } else { "other-effect-ticket" };
            let state = if index == 0 { "pending" } else { "uncertain" };
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?5)",
                params![ticket,format!("effect:{index:03}"),
                    format!("{index:064x}"),state,1000+index/5],
            ).unwrap();
        }
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'already-applied',?2,'applied',9000,9000)",
            params![&id,"a".repeat(64)],
        ).unwrap();
        let path=store.path.clone();
        let mut cursor_at=None;
        let mut cursor_ticket:Option<String>=None;
        let mut cursor_key:Option<String>=None;
        let mut seen=std::collections::HashSet::new();
        let mut pages=0;
        loop {
            let response:serde_json::Value=serde_json::from_str(
                &superagent::list_headless_unresolved_effects(
                    &path,cursor_at,cursor_ticket.as_deref(),cursor_key.as_deref(),
                ).unwrap(),
            ).unwrap();
            pages+=1;
            assert!(pages<=3, "unresolved inbox cursor must advance");
            assert_eq!(response["unresolvedTotal"],205);
            assert_eq!(response["pendingTotal"],1);
            assert_eq!(response["uncertainTotal"],204);
            assert_eq!(response["requiresLiveHostVerification"],true);
            assert_eq!(response["observationalOnly"],true);
            assert_eq!(response["automaticReplayAllowed"],false);
            assert_eq!(response["requiresIndependentRemoteReconciliation"],true);
            assert!(!response.to_string().contains("PRIVATE OBJECTIVE"));
            assert!(!response.to_string().contains("already-applied"));
            for item in response["items"].as_array().unwrap() {
                let composite=format!("{}:{}",
                    item["ticketId"].as_str().unwrap(),
                    item["effectKey"].as_str().unwrap());
                assert!(seen.insert(composite),"a receipt cannot repeat across pages");
                assert!(item["state"]=="pending" || item["state"]=="uncertain");
            }
            if response["nextCursor"].is_null() {break;}
            cursor_at=response["nextCursor"]["updatedAt"].as_i64();
            cursor_ticket=response["nextCursor"]["ticketId"].as_str().map(str::to_owned);
            cursor_key=response["nextCursor"]["effectKey"].as_str().map(str::to_owned);
        }
        assert_eq!(pages,3);
        assert_eq!(seen.len(),205);
        assert!(superagent::list_headless_unresolved_effects(&path,Some(0),Some(&id),Some("effect:000")).is_err());
        assert!(superagent::list_headless_unresolved_effects(&path,Some(1000),Some("../bad"),Some("effect:000")).is_err());
        assert!(superagent::list_headless_unresolved_effects(&path,Some(1000),Some(&id),None).is_err());
        assert!(superagent::list_headless_unresolved_effects(
            &path, Some(1000), Some(&id), Some("invented:effect"),
        ).is_err(), "syntactically valid but unowned cursor cannot skip recoverable effects");
        assert!(superagent::list_headless_unresolved_effects(
            &path, Some(9000), Some(&id), Some("already-applied"),
        ).is_err(), "applied receipts cannot anchor an unresolved recovery page");
        // The first receipt of another Ticket has a valid timestamp and key,
        // but must not page through the current Ticket's own receipt history.
        assert!(receipt_history_page(&store, &id, 10,
            Some((1000, "effect:001")), false).is_err());
        let pending:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE state='pending'",
            [],|r|r.get(0),
        ).unwrap();
        assert_eq!(pending,1,"read-only inventory cannot settle live effects");
        drop(store);
        let restarted=Store::open(&path).unwrap();
        let count:i64=restarted.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE state='uncertain'",
            [],|r|r.get(0),
        ).unwrap();
        assert_eq!(count,205,"restart must quarantine pending without replay");
        assert_eq!(serde_json::from_str::<serde_json::Value>(
            &superagent::list_headless_unresolved_effects(&path,None,None,None).unwrap(),
        ).unwrap()["unresolvedTotal"],205);
    }


    #[test]
    fn team_reconciliation_inbox_scopes_pages_and_ticket_revisions_without_replay() {
        let (store, ticket_id) = fixture();
        let first_team: String = store.db.query_row(
            "SELECT orchestration_id FROM sa_tickets WHERE id=?1",
            [&ticket_id], |r| r.get(0),
        ).unwrap();
        let second_team = superagent::create_configuration(&store, NewOrchestration {
            name: "Other Effect Team".into(),
            coordinator_bot_id: "co".into(),
            worker_bot_id: "wo".into(),
        }).unwrap();
        store.db.execute(
            "INSERT INTO sa_tickets
             (id,orchestration_id,request_key,objective,actor,source,project_id,
              worker_bot_id,policy_revision,state,created_at,updated_at)
             SELECT 'other-team-effect-ticket',?1,'other-team-effect-request',
                    'OTHER PRIVATE OBJECTIVE',actor,source,project_id,worker_bot_id,
                    ?2,'blocked',created_at,updated_at
             FROM sa_tickets WHERE id=?3",
            params![&second_team.id, second_team.policy_revision, &ticket_id],
        ).unwrap();
        let revision: i64 = store.db.query_row(
            "SELECT revision FROM sa_tickets WHERE id=?1",
            [&ticket_id], |r| r.get(0),
        ).unwrap();
        for index in 0..104_i64 {
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'uncertain',?4,?4)",
                params![&ticket_id,format!("team-effect:{index:03}"),
                    format!("{index:064x}"),1000+index/4],
            ).unwrap();
        }
        for index in 0..3_i64 {
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES('other-team-effect-ticket',?1,?2,'pending',1000,1000)",
                params![format!("other-team:{index}"),format!("{index:064x}")],
            ).unwrap();
        }
        let path = store.path.clone();
        let mut cursor: Option<(i64,String,String)> = None;
        let mut seen = std::collections::HashSet::new();
        loop {
            let result:serde_json::Value = serde_json::from_str(
                &superagent::list_headless_team_unresolved_effects(
                    &path,&first_team,cursor.as_ref().map(|c|c.0),
                    cursor.as_ref().map(|c|c.1.as_str()),
                    cursor.as_ref().map(|c|c.2.as_str()),
                ).unwrap(),
            ).unwrap();
            assert_eq!(result["orchestrationId"],first_team);
            assert_eq!(result["unresolvedTotal"],104);
            assert_eq!(result["pendingTotal"],0);
            assert_eq!(result["uncertainTotal"],104);
            assert_eq!(result["requiresLiveHostVerification"],true);
            assert_eq!(result["potentiallyLiveUnresolvedTickets"],1);
            assert_eq!(result["observationalOnly"],true);
            assert_eq!(result["automaticReplayAllowed"],false);
            assert!(!result.to_string().contains("OTHER PRIVATE OBJECTIVE"));
            for item in result["items"].as_array().unwrap() {
                assert_eq!(item["ticketId"],ticket_id);
                assert_eq!(item["ticketRevision"],revision);
                assert!(seen.insert(item["effectKey"].as_str().unwrap().to_owned()));
            }
            let next=&result["nextCursor"];
            if next.is_null(){break;}
            cursor=Some((
                next["updatedAt"].as_i64().unwrap(),
                next["ticketId"].as_str().unwrap().to_owned(),
                next["effectKey"].as_str().unwrap().to_owned(),
            ));
        }
        assert_eq!(seen.len(),104);
        let other:serde_json::Value=serde_json::from_str(
            &superagent::list_headless_team_unresolved_effects(
                &path,&second_team.id,None,None,None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(other["unresolvedTotal"],3);
        assert_eq!(other["pendingTotal"],3);
        assert_eq!(other["uncertainTotal"],0);
        assert_eq!(other["requiresLiveHostVerification"],true);
        assert_eq!(other["items"].as_array().unwrap().len(),3);
        assert_eq!(serde_json::from_str::<serde_json::Value>(
            &superagent::list_headless_unresolved_effects(&path,None,None,None).unwrap(),
        ).unwrap()["unresolvedTotal"],107);
        assert!(superagent::list_headless_team_unresolved_effects(
            &path,"missing-team",None,None,None,
        ).is_err());
        assert!(superagent::list_headless_team_unresolved_effects(
            &path,"../other",None,None,None,
        ).is_err());
        assert!(superagent::list_headless_team_unresolved_effects(
            &path,&first_team,Some(1),None,None,
        ).is_err());
        assert!(superagent::list_headless_team_unresolved_effects(
            &path,&first_team,Some(1000),Some("other-team-effect-ticket"),Some("other-team:0"),
        ).is_err(), "another Team's real pending receipt is not a valid local cursor");
        assert!(superagent::list_headless_team_unresolved_effects(
            &path,&first_team,Some(1000),Some(&ticket_id),Some("fabricated:receipt"),
        ).is_err());
        let pending:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE state='pending'",
            [],|r|r.get(0),
        ).unwrap();
        assert_eq!(pending,3,"read-only scoped recovery cannot reconcile receipts");
    }

    #[test]
    fn headless_pending_to_uncertain_quarantine_keeps_live_host_warning() {
        let (store, ticket_id) = fixture();
        let path = store.path.clone();
        let team_id: String = store.db.query_row(
            "SELECT orchestration_id FROM sa_tickets WHERE id=?1",
            [&ticket_id], |row| row.get(0),
        ).unwrap();
        let revision: i64 = store.db.query_row(
            "SELECT revision FROM sa_tickets WHERE id=?1",
            [&ticket_id], |row| row.get(0),
        ).unwrap();
        let digest = "d".repeat(64);
        let now = storage::now();
        store.db.execute(
            "INSERT INTO sa_effect_receipts
             (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             VALUES(?1,'github:inflight',?2,'pending',?3,?3)",
            params![&ticket_id,&digest,now],
        ).unwrap();

        // A detached operator can mark a pending outcome uncertain without
        // stopping the still-active native Assignment or verifying the host.
        let quarantine: serde_json::Value = serde_json::from_str(
            &superagent::headless_quarantine_pending_effect(
                &path, &ticket_id, revision, "github:inflight", &digest,
            ).unwrap(),
        ).unwrap();
        assert_eq!(quarantine["state"], "uncertain");
        assert_eq!(quarantine["remoteResultVerified"], false);

        let ticket: serde_json::Value = serde_json::from_str(
            &superagent::inspect_headless_effect_receipts(&path, &ticket_id).unwrap(),
        ).unwrap();
        assert_eq!(ticket["pendingTotal"], 0);
        assert_eq!(ticket["uncertainTotal"], 1);
        assert_eq!(ticket["potentiallyLiveUnresolvedTickets"], 1);
        assert_eq!(ticket["requiresLiveHostVerification"], true);
        let global: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_unresolved_effects(&path, None, None, None).unwrap(),
        ).unwrap();
        assert_eq!(global["pendingTotal"], 0);
        assert_eq!(global["potentiallyLiveUnresolvedTickets"], 1);
        assert_eq!(global["requiresLiveHostVerification"], true);
        let scoped: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_team_unresolved_effects(
                &path, &team_id, None, None, None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(scoped["potentiallyLiveUnresolvedTickets"], 1);
        assert_eq!(scoped["requiresLiveHostVerification"], true);
        assert_eq!(scoped["automaticReplayAllowed"], false);
        assert_eq!(scoped["observationalOnly"], true);

        let repeated: serde_json::Value = serde_json::from_str(
            &superagent::headless_quarantine_pending_effect(
                &path, &ticket_id, revision, "github:inflight", &digest,
            ).unwrap(),
        ).unwrap();
        assert_eq!(repeated["changed"], false);
        let audit_count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='headless_effect_quarantined'",
            [&ticket_id], |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_count, 1, "quarantine must not duplicate audit");
    }

    #[test]
    fn headless_receipt_inspection_reports_only_actual_unresolved_effects() {
        let (store,id) = fixture();
        let empty: serde_json::Value = serde_json::from_str(
            &crate::superagent::inspect_headless_effect_receipts(&store.path,&id).unwrap(),
        ).unwrap();
        assert_eq!(empty["requiresIndependentRemoteReconciliation"], false);
        assert_eq!(empty["pendingTotal"], 0);
        assert_eq!(empty["uncertainTotal"], 0);
        assert_eq!(empty["requiresLiveHostVerification"], false);
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
        assert_eq!(pending["pendingTotal"], 1);
        assert_eq!(pending["uncertainTotal"], 0);
        assert_eq!(pending["requiresLiveHostVerification"], true);
        acknowledge_applied(&store,&id,"github:issue:42",b"payload").unwrap();
        let applied: serde_json::Value = serde_json::from_str(
            &crate::superagent::inspect_headless_effect_receipts(&store.path,&id).unwrap(),
        ).unwrap();
        assert_eq!(applied["totalUnresolved"], 0);
        assert_eq!(applied["pendingTotal"], 0);
        assert_eq!(applied["uncertainTotal"], 0);
        assert_eq!(applied["requiresLiveHostVerification"], false);
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
        assert_eq!(status["pendingTotal"], 0);
        assert_eq!(status["uncertainTotal"], 1);
        assert_eq!(status["requiresLiveHostVerification"], true);
        assert_eq!(status["potentiallyLiveUnresolvedTickets"], 1);
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
    fn headless_receipt_snapshot_stays_consistent_during_remote_reconciliation() {
        let (store, id) = fixture();
        let path = store.path.clone();
        // The writer must be opened before pending receipts are inserted: a
        // new normal Store::open after insertion would run restart recovery.
        let writer = Store::open(&path).unwrap();
        for index in 0..32 {
            let payload = format!("private-reconciliation-{index}");
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,'pending',?4,?4)",
                params![&id, format!("race:{index:03}"),
                    format!("{:x}", Sha256::digest(payload.as_bytes())), 10_000 + index],
            ).unwrap();
        }
        let writer_id = id.clone();
        let reconciler = std::thread::spawn(move || {
            for index in 0..32 {
                let payload = format!("private-reconciliation-{index}");
                acknowledge_applied(&writer, &writer_id,
                    &format!("race:{index:03}"), payload.as_bytes()).unwrap();
                std::thread::yield_now();
            }
        });
        for _ in 0..64 {
            let status: serde_json::Value = serde_json::from_str(
                &superagent::inspect_headless_effect_receipts_page(
                    &path, &id, None, None, true,
                ).unwrap(),
            ).unwrap();
            let pending = status["pendingTotal"].as_i64().unwrap();
            let uncertain = status["uncertainTotal"].as_i64().unwrap();
            assert_eq!(status["totalUnresolved"], pending + uncertain);
            assert_eq!(status["receipts"].as_array().unwrap().len() as i64,
                pending + uncertain, "page and counts must share a SQLite snapshot");
            assert_eq!(status["requiresLiveHostVerification"], pending > 0);
            assert_eq!(status["observationalOnly"], true);
            assert_eq!(status["replayed"], false);
            std::thread::yield_now();
        }
        reconciler.join().unwrap();
        let final_status: serde_json::Value = serde_json::from_str(
            &superagent::inspect_headless_effect_receipts_page(
                &path, &id, None, None, true,
            ).unwrap(),
        ).unwrap();
        assert_eq!(final_status["totalUnresolved"], 0);
        assert!(final_status["receipts"].as_array().unwrap().is_empty());
        assert_eq!(final_status["requiresLiveHostVerification"], false);
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
    fn headless_effect_pages_preserve_all_receipts_and_unresolved_recovery() {
        let (store, id) = fixture();
        // Tied timestamps require a composite cursor; the oldest unresolved
        // receipt must be observable even after hundreds of applied effects.
        for index in 0..221 {
            let state = if index % 31 == 0 { "uncertain" } else { "applied" };
            store.db.execute(
                "INSERT INTO sa_effect_receipts
                 (ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?5)",
                params![&id, format!("effect:{index:03}"),
                    format!("{index:064x}"), state, 10_000 + index / 3],
            ).unwrap();
        }
        let path = store.path.clone();
        drop(store);
        let mut cursor_at = None;
        let mut cursor_key: Option<String> = None;
        let mut observed = Vec::new();
        let mut pages = 0;
        loop {
            let response: serde_json::Value = serde_json::from_str(
                &superagent::inspect_headless_effect_receipts_page(
                    &path, &id, cursor_at, cursor_key.as_deref(), false,
                ).unwrap(),
            ).unwrap();
            pages += 1;
            assert!(pages <= 4, "cursor must advance");
            assert_eq!(response["totalUnresolved"], 8);
            assert_eq!(response["observationalOnly"], true);
            assert_eq!(response["replayed"], false);
            for receipt in response["receipts"].as_array().unwrap() {
                observed.push(receipt["effectKey"].as_str().unwrap().to_owned());
            }
            if response["nextCursor"].is_null() { break; }
            cursor_at = response["nextCursor"]["createdAt"].as_i64();
            cursor_key = response["nextCursor"]["effectKey"].as_str().map(str::to_owned);
        }
        assert_eq!(pages, 3);
        assert_eq!(observed.len(), 221);
        assert_eq!(observed.first().map(String::as_str), Some("effect:220"));
        assert_eq!(observed.last().map(String::as_str), Some("effect:000"));
        let unique: std::collections::HashSet<_> = observed.iter().collect();
        assert_eq!(unique.len(), 221);
        let unresolved: serde_json::Value = serde_json::from_str(
            &superagent::inspect_headless_effect_receipts_page(
                &path, &id, None, None, true,
            ).unwrap(),
        ).unwrap();
        assert_eq!(unresolved["receipts"].as_array().unwrap().len(), 8);
        assert!(unresolved["receipts"].as_array().unwrap().iter()
            .all(|receipt| receipt["state"] == "uncertain"));
        assert!(unresolved["nextCursor"].is_null());
        assert_eq!(unresolved["filter"], "unresolved");
        let read = Store::open_read_only(&path).unwrap();
        assert_eq!(unresolved_count(&read, &id).unwrap(), 8);
        assert!(receipt_history_page(&read, &id, 100, Some((0, "effect:010")), false).is_err());
        assert!(receipt_history_page(&read, &id, 100, Some((10001, "../bad")), false).is_err());
        assert!(receipt_history_page(&read, &id, 100,
            Some((10_000, "invented:effect")), false).is_err());
        assert!(receipt_history_page(&read, &id, 100,
            Some((10_000, "effect:002")), true).is_err(),
            "an applied effect cannot anchor unresolved-only pagination");
        assert!(receipt_history_page(&read, "another-ticket", 100, None, false).is_err());
        let unchanged: i64 = read.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE ticket_id=?1 AND state='uncertain'",
            [&id], |r| r.get(0),
        ).unwrap();
        assert_eq!(unchanged, 8);
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
    fn effect_reservation_rechecks_coordinator_pause_before_reserving() {
        let (store, ticket_id) = fixture();
        // This fixture exercises the defensive reservation boundary only.
        // Production cannot issue a writable SuperAgent grant yet.
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&ticket_id],
        ).unwrap();
        assert_eq!(reserve(&store, &ticket_id, "github:issue:before-pause", b"body").unwrap(),
            Reservation::Reserved);
        store.db.execute(
            "UPDATE bots SET paused=1 WHERE id='co'", [],
        ).unwrap();
        assert!(reserve(&store, &ticket_id, "github:issue:after-pause", b"body").is_err(),
            "paused Coordinator must block a Worker effect reservation");
        assert_eq!(unresolved_count(&store, &ticket_id).unwrap(), 1,
            "a denied effect must not create a receipt");
        store.db.execute(
            "UPDATE bots SET paused=0 WHERE id='co'", [],
        ).unwrap();
        // Unpausing cannot override a still-pending external effect.
        assert_eq!(reserve(&store, &ticket_id, "github:issue:before-pause", b"body").unwrap(),
            Reservation::AlreadyPending);
        assert!(reserve(&store, &ticket_id, "github:issue:after-pause", b"body")
            .unwrap_err().contains("pending or uncertain"));
        acknowledge_applied(&store, &ticket_id, "github:issue:before-pause", b"body").unwrap();
        assert_eq!(reserve(&store, &ticket_id, "github:issue:after-pause", b"body").unwrap(),
            Reservation::Reserved);
        assert_eq!(unresolved_count(&store, &ticket_id).unwrap(), 1);
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
            ("UPDATE sa_mandates SET authorization_id='' WHERE ticket_id=?1", "empty authorization"),
            ("UPDATE sa_mandates SET authorization_id='   ' WHERE ticket_id=?1", "blank authorization"),
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
    fn effect_reservation_denies_detached_moved_or_modified_worker_session() {
        for mutation in ["detached_thread", "moved_session", "changed_model"] {
            let (store,id)=fixture();
            store.db.execute(
                "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
                [&id],
            ).unwrap();
            let session_id: String=store.db.query_row(
                "SELECT session_id FROM sa_assignments
                 WHERE ticket_id=?1 AND state='running'",
                [&id], |row| row.get(0),
            ).unwrap();
            let changed=match mutation {
                "detached_thread"=>store.db.execute(
                    "UPDATE bot_threads SET external_id='detached'
                     WHERE session_id=?1 AND channel_id=?2",
                    params![&session_id,superagent::CHANNEL],
                ).unwrap(),
                "moved_session"=>{
                    store.db.execute(
                        "INSERT INTO projects(id,name,path)
                         VALUES('outside','Outside','/tmp/outside-sa-effects')", [],
                    ).unwrap();
                    store.db.execute(
                        "UPDATE sessions SET project_id='outside' WHERE id=?1",
                        [&session_id],
                    ).unwrap()
                },
                "changed_model"=>store.db.execute(
                    "UPDATE sessions SET model='unapproved-model' WHERE id=?1",
                    [&session_id],
                ).unwrap(),
                _=>unreachable!(),
            };
            assert_eq!(changed,1, "fixture must change the native {mutation}");
            assert!(reserve(&store,&id,"github:issue:session-fence",b"body").is_err(),
                "{mutation} cannot reserve any external effect");
            assert_eq!(unresolved_count(&store,&id).unwrap(),0);
        }
    }

    #[test]
    fn effect_reservation_denies_multiple_active_attempts_on_one_ticket() {
        let (store,id)=fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        // Simulate an inconsistent legacy database; the native lease trigger
        // normally forbids this. The effect guard must fail closed regardless.
        store.db.execute_batch(
            "DROP TRIGGER sa_project_lease_insert;
             DROP TRIGGER sa_project_lease_update;",
        ).unwrap();
        store.db.execute(
            "INSERT INTO sa_assignments
             (id,ticket_id,bot_id,session_id,state,attempt,created_at,updated_at)
             VALUES(?1,?2,'co','orphan-legacy-session','running',2,?3,?3)",
            params![storage::id(),&id,storage::now()],
        ).unwrap();
        assert!(reserve(&store,&id,"github:issue:duplicate",b"body").is_err());
        assert_eq!(unresolved_count(&store,&id).unwrap(),0);
        store.db.execute(
            "UPDATE sa_assignments SET state='stopped'
             WHERE ticket_id=?1 AND attempt=2",
            [&id],
        ).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:duplicate",b"body").unwrap(),
            Reservation::Reserved);
    }

    #[test]
    fn unrelated_new_effect_is_fenced_until_pending_or_uncertain_outcome_is_reconciled() {
        let (store, id) = fixture();
        // Test-only receipt authority. Production task effects remain denied
        // until native scoped grants and effect-time enforcement exist.
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        let original = "github:comment:original";
        let next = "github:comment:next";
        assert_eq!(reserve(&store, &id, original, b"original-body").unwrap(),
            Reservation::Reserved);
        // A 'pending' effect can already have been dispatched. Neither a
        // simultaneous reservation nor a retry may create an unrelated action.
        assert_eq!(reserve(&store, &id, original, b"original-body").unwrap(),
            Reservation::AlreadyPending);
        assert!(reserve(&store, &id, next, b"next-body").unwrap_err()
            .contains("pending or uncertain"));
        mark_uncertain(&store, &id, original).unwrap();
        assert_eq!(reserve(&store, &id, original, b"original-body").unwrap(),
            Reservation::RequiresReconciliation,
            "same idempotency key must remain inspectable without replay");
        assert!(reserve(&store, &id, next, b"next-body").unwrap_err()
            .contains("pending or uncertain"));
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_effect_receipts WHERE ticket_id=?1",
            [&id], |row| row.get::<_, i64>(0),
        ).unwrap(), 1, "rejected reservation cannot leave a durable receipt");
        // Simulate an independently verified provider result in this fixture;
        // this primitive does not itself contact or verify any remote service.
        acknowledge_applied(&store, &id, original, b"original-body").unwrap();
        assert_eq!(reserve(&store, &id, next, b"next-body").unwrap(),
            Reservation::Reserved);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let history = receipt_history(&reopened, &id, 10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.iter().any(|receipt|
            receipt.effect_key == original && receipt.state == "applied"));
        assert!(history.iter().any(|receipt|
            receipt.effect_key == next && receipt.state == "uncertain"),
            "restart must quarantine the new pending action rather than replay");
    }

    #[test]
    fn repeated_effect_reconciliation_is_idempotent_and_preserves_terminal_receipt() {
        let (store, id) = fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        let key = "github:comment:reconcile";
        let payload = b"approved immutable bytes";
        assert_eq!(reserve(&store, &id, key, payload).unwrap(), Reservation::Reserved);
        mark_uncertain(&store, &id, key).unwrap();
        store.db.execute(
            "UPDATE sa_effect_receipts SET updated_at=111 WHERE ticket_id=?1 AND effect_key=?2",
            params![&id,key],
        ).unwrap();
        mark_uncertain(&store, &id, key).unwrap();
        let unchanged: i64 = store.db.query_row(
            "SELECT updated_at FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key=?2",
            params![&id,key], |r| r.get(0),
        ).unwrap();
        assert_eq!(unchanged, 111);
        assert!(acknowledge_applied(&store, &id, key, b"changed bytes").is_err());
        assert!(acknowledge_applied(&store, &id, "missing-key", payload).is_err());
        acknowledge_applied(&store, &id, key, payload).unwrap();
        assert!(mark_uncertain(&store, &id, key).is_err());
        store.db.execute(
            "UPDATE sa_effect_receipts SET updated_at=222 WHERE ticket_id=?1 AND effect_key=?2",
            params![&id,key],
        ).unwrap();
        acknowledge_applied(&store, &id, key, payload).unwrap();
        assert!(acknowledge_applied(&store, &id, key, b"changed bytes").is_err());
        let applied: (String, i64) = store.db.query_row(
            "SELECT state,updated_at FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2",
            params![&id,key], |r| Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(applied, ("applied".to_string(), 222));
        assert_eq!(unresolved_count(&store, &id).unwrap(), 0);
        let path = store.path.clone();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        acknowledge_applied(&reopened, &id, key, payload).unwrap();
        assert!(mark_uncertain(&reopened, &id, key).is_err());
        let persisted: (String, i64) = reopened.db.query_row(
            "SELECT state,updated_at FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key=?2",
            params![&id,key], |r| Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(persisted, applied);
    }

    #[test]
    fn effect_receipt_transitions_require_atomic_audit_and_remain_idempotent() {
        let (store,id)=fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",
            [&id],
        ).unwrap();
        let key="github:comment:audited";
        let payload=b"private provider payload must not appear in audit";

        store.db.execute_batch(
            "CREATE TRIGGER reject_effect_reservation_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='effect_reserved'
             BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;",
        ).unwrap();
        assert!(reserve(&store,&id,key,payload).is_err());
        assert_eq!(receipt_history(&store,&id,10).unwrap().len(),0,
            "a failed audit must roll back the reservation");
        store.db.execute_batch("DROP TRIGGER reject_effect_reservation_audit;").unwrap();

        assert_eq!(reserve(&store,&id,key,payload).unwrap(),Reservation::Reserved);
        assert_eq!(reserve(&store,&id,key,payload).unwrap(),Reservation::AlreadyPending);
        store.db.execute_batch(
            "CREATE TRIGGER reject_effect_uncertain_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='effect_uncertain'
             BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;",
        ).unwrap();
        assert!(mark_uncertain(&store,&id,key).is_err());
        assert_eq!(receipt_history(&store,&id,10).unwrap()[0].state,"pending",
            "a failed audit must roll back uncertainty");
        store.db.execute_batch("DROP TRIGGER reject_effect_uncertain_audit;").unwrap();

        mark_uncertain(&store,&id,key).unwrap();
        mark_uncertain(&store,&id,key).unwrap();
        store.db.execute_batch(
            "CREATE TRIGGER reject_effect_applied_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='effect_applied'
             BEGIN SELECT RAISE(ABORT,'audit unavailable'); END;",
        ).unwrap();
        assert!(acknowledge_applied(&store,&id,key,payload).is_err());
        assert_eq!(receipt_history(&store,&id,10).unwrap()[0].state,"uncertain",
            "a failed audit must roll back an applied state");
        store.db.execute_batch("DROP TRIGGER reject_effect_applied_audit;").unwrap();

        // The fixture simulates a verified provider outcome. The native
        // primitive itself does not make a provider call or prove success.
        acknowledge_applied(&store,&id,key,payload).unwrap();
        acknowledge_applied(&store,&id,key,payload).unwrap();
        assert!(acknowledge_applied(&store,&id,key,b"different bytes").is_err());
        let audit=store.db.prepare(
            "SELECT kind,detail FROM sa_audit WHERE ticket_id=?1
             AND kind IN ('effect_reserved','effect_uncertain','effect_applied')
             ORDER BY id",
        ).unwrap().query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))
            .unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert_eq!(audit.len(),3,"each transition is audited exactly once");
        assert_eq!(audit.iter().map(|row|row.0.as_str()).collect::<Vec<_>>(),
            vec!["effect_reserved","effect_uncertain","effect_applied"]);
        assert!(audit.iter().all(|row|row.1.contains(key)));
        assert!(audit.iter().all(|row|!row.1.contains("private provider payload")));

        let path=store.path.clone();
        drop(store);
        let reopened=Store::open(&path).unwrap();
        assert_eq!(receipt_history(&reopened,&id,10).unwrap()[0].state,"applied");
        let count:i64=reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1
             AND kind IN ('effect_reserved','effect_uncertain','effect_applied')",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(count,3,"restart cannot generate duplicate transition records");
    }

    #[test]
    fn restart_effect_quarantine_records_one_durable_audit_per_ticket_without_replay() {
        let (store,id)=fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id],
        ).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:alpha",b"private payload never audited").unwrap(),
            Reservation::Reserved);
        // Old persisted data can contain multiple pending receipts. Recreate
        // that pre-upgrade state directly, since new reservations fence it.
        store.db.execute(
            "INSERT INTO sa_effect_receipts(ticket_id,effect_key,payload_sha256,state,created_at,updated_at)
             SELECT ticket_id,'github:issue:beta',payload_sha256,state,created_at,updated_at
             FROM sa_effect_receipts WHERE ticket_id=?1 AND effect_key='github:issue:alpha'",
            [&id],
        ).unwrap();
        let path=store.path.clone();
        drop(store);

        let first=Store::open(&path).unwrap();
        let (pending,uncertain):(i64,i64)=first.db.query_row(
            "SELECT SUM(state='pending'),SUM(state='uncertain')
             FROM sa_effect_receipts WHERE ticket_id=?1",
            [&id],|r|Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!((pending,uncertain),(0,2));
        let audit_count:i64=first.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='restart_effect_uncertain'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(audit_count,1,"one crash quarantine record per Ticket");
        let details:String=first.db.query_row(
            "SELECT detail FROM sa_audit
             WHERE ticket_id=?1 AND kind='restart_effect_uncertain'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert!(details.contains("verify remotely"));
        assert!(!details.contains("private payload"));
        assert!(!details.contains("alpha"));
        assert_eq!(unresolved_count(&first,&id).unwrap(),2);
        drop(first);
        let again=Store::open(&path).unwrap();
        let second_count:i64=again.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='restart_effect_uncertain'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(second_count,1,"subsequent restarts cannot duplicate evidence");
        assert_eq!(unresolved_count(&again,&id).unwrap(),2);
    }

    #[test]
    fn restart_effect_audit_failure_rolls_back_quarantine_and_recovers_once() {
        let (store,id)=fixture();
        store.db.execute(
            "UPDATE sa_mandates SET allowed_side_effects=1 WHERE ticket_id=?1",[&id],
        ).unwrap();
        assert_eq!(reserve(&store,&id,"github:issue:rollback",b"payload").unwrap(),
            Reservation::Reserved);
        store.db.execute_batch(
            "CREATE TRIGGER sa_reject_restart_effect_audit
             BEFORE INSERT ON sa_audit
             WHEN NEW.kind='restart_effect_uncertain'
             BEGIN SELECT RAISE(ABORT,'test audit write denied'); END;",
        ).unwrap();
        let path=store.path.clone();
        drop(store);
        assert!(Store::open(&path).is_err(),
            "failed audit must prevent restart quarantine from committing");
        let read=Store::open_read_only(&path).unwrap();
        let state:String=read.db.query_row(
            "SELECT state FROM sa_effect_receipts
             WHERE ticket_id=?1 AND effect_key='github:issue:rollback'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(state,"pending","failed transaction cannot hide uncertain work");
        let count:i64=read.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='restart_effect_uncertain'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(count,0);
        drop(read);
        let connection=rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("DROP TRIGGER sa_reject_restart_effect_audit;").unwrap();
        drop(connection);
        let recovered=Store::open(&path).unwrap();
        assert_eq!(receipt_history(&recovered,&id,10).unwrap()[0].state,"uncertain");
        let count:i64=recovered.db.query_row(
            "SELECT COUNT(*) FROM sa_audit
             WHERE ticket_id=?1 AND kind='restart_effect_uncertain'",
            [&id],|r|r.get(0),
        ).unwrap();
        assert_eq!(count,1);
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
