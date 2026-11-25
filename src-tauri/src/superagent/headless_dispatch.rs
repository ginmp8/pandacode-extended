//! Durable, explicit local-operator dispatch intents consumed only by the live
//! headless host. A request is not a grant, assignment, provider receipt, or
//! Council verdict. Admission and consumption share the existing native
//! Assignment transaction; a committed reservation is never automatically replayed.
use super::{configuration, effects, get_ticket, mandate, open_headless_transition_store, Ticket};
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest,Sha256};

const MAX_REQUEST_AGE_MS: i64 = 5 * 60 * 1000;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sa_headless_dispatch_requests (
    id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    request_key TEXT NOT NULL,
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    kind TEXT NOT NULL CHECK(kind IN ('retry','coordinated')),
    state TEXT NOT NULL CHECK(state IN ('pending','admitted','rejected')),
    assignment_id TEXT UNIQUE REFERENCES sa_assignments(id) ON DELETE RESTRICT,
    rejection TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(ticket_id,request_key),
    CHECK((state='admitted')=(assignment_id IS NOT NULL)),
    CHECK((state='rejected')=(rejection IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS sa_headless_dispatch_pending_ticket
    ON sa_headless_dispatch_requests(ticket_id) WHERE state='pending';
CREATE INDEX IF NOT EXISTS sa_headless_dispatch_pending_order
    ON sa_headless_dispatch_requests(created_at,id) WHERE state='pending';
";

/// Explicit opt-in to continue the exact reviewed plan after each separate
/// human acceptance. No historical request inherits this authority.
pub(crate) const PLAN_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sa_headless_plan_dispatch (
 request_id TEXT PRIMARY KEY REFERENCES sa_headless_dispatch_requests(id) ON DELETE RESTRICT,
 plan_sha256 TEXT NOT NULL CHECK(length(plan_sha256)=64),
 previous_request_id TEXT UNIQUE REFERENCES sa_headless_plan_dispatch(request_id) ON DELETE RESTRICT,
 CHECK(previous_request_id IS NULL OR previous_request_id<>request_id)
);
CREATE TRIGGER IF NOT EXISTS sa_headless_plan_dispatch_immutable_update
 BEFORE UPDATE ON sa_headless_plan_dispatch
 BEGIN SELECT RAISE(ABORT,'Plan dispatch authorization is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_headless_plan_dispatch_immutable_delete
 BEFORE DELETE ON sa_headless_plan_dispatch
 BEGIN SELECT RAISE(ABORT,'Plan dispatch authorization is immutable'); END;
";

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DispatchRequest {
    pub(super) id: String,
    pub(super) ticket_id: String,
    request_key: String,
    pub(super) expected_revision: i64,
    pub(super) kind: String,
    state: String,
    assignment_id: Option<String>,
    rejection: Option<String>,
    created_at: i64,
}

const SELECT: &str = "SELECT id,ticket_id,request_key,expected_revision,kind,state,
    assignment_id,rejection,created_at FROM sa_headless_dispatch_requests";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DispatchRequest> {
    Ok(DispatchRequest {
        id: r.get(0)?, ticket_id: r.get(1)?, request_key: r.get(2)?,
        expected_revision: r.get(3)?, kind: r.get(4)?, state: r.get(5)?,
        assignment_id: r.get(6)?, rejection: r.get(7)?, created_at: r.get(8)?,
    })
}

fn by_key(store: &Store, ticket_id: &str, key: &str) -> Result<Option<DispatchRequest>> {
    store.db.query_row(&format!("{SELECT} WHERE ticket_id=?1 AND request_key=?2"),
        params![ticket_id,key], row).optional().map_err(|e| e.to_string())
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'))
}

/// Expired intents never reserved an Assignment. Settle their receipts and
/// audit together before counting pending capacity, even without a live
/// serve host. Never touch an admitted intent or refresh an old key's lifetime.
fn expire_pending(tx: &Transaction<'_>, now: i64) -> Result<()> {
    let cutoff = now.saturating_sub(MAX_REQUEST_AGE_MS);
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         SELECT ticket_id,'headless_dispatch_rejected',
            'Dispatch request ' || id || ' rejected: expired or invalid creation time',?1
         FROM sa_headless_dispatch_requests
         WHERE state='pending' AND (created_at<=?2 OR created_at>?1 OR created_at<=0)",
        params![now,cutoff],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE sa_headless_dispatch_requests
         SET state='rejected',rejection='expired or invalid creation time',updated_at=?1
         WHERE state='pending' AND (created_at<=?2 OR created_at>?1 OR created_at<=0)",
        params![now,cutoff],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Retire only unconsumed requests whose exact Ticket revision has changed.
/// The caller owns the transaction: plan edits and their revocation audit
/// must commit together. Admitted requests are immutable native receipts.
pub(super) fn reject_stale_pending(
    tx: &Transaction<'_>, ticket_id: &str, now: i64,
) -> Result<()> {
    let predicate = "ticket_id=?1 AND state='pending' AND EXISTS(
        SELECT 1 FROM sa_tickets t WHERE t.id=sa_headless_dispatch_requests.ticket_id
        AND t.revision<>sa_headless_dispatch_requests.expected_revision)";
    tx.execute(
        &format!("INSERT INTO sa_audit(ticket_id,kind,detail,at)
         SELECT ticket_id,'headless_dispatch_rejected',
         'Dispatch request ' || id || ' rejected: Ticket revision changed',?2
         FROM sa_headless_dispatch_requests WHERE {predicate}"),
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_headless_dispatch_requests
         SET state='rejected',rejection='Ticket revision changed',updated_at=?2
         WHERE {predicate}"),
        params![ticket_id,now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Detect old pending intents even if a legacy writer already persisted Off.
pub(super) fn team_has_pending(db: &rusqlite::Connection, team_id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_headless_dispatch_requests r
         JOIN sa_tickets t ON t.id=r.ticket_id
         WHERE t.orchestration_id=?1 AND r.state='pending')",
        [team_id], |row| row.get(0),
    ).map_err(|e| e.to_string())
}

/// Team Off retires only its unconsumed requests, including legacy terminal
/// Tickets, in the same transaction as the policy and Mandate revocation.
pub(super) fn reject_team_pending(
    tx: &Transaction<'_>, team_id: &str, now: i64,
) -> Result<()> {
    let predicate = "state='pending' AND ticket_id IN (
        SELECT id FROM sa_tickets WHERE orchestration_id=?1)";
    tx.execute(
        &format!("INSERT INTO sa_audit(ticket_id,kind,detail,at)
         SELECT ticket_id,'headless_dispatch_rejected',
         'Dispatch request ' || id || ' rejected: Team disabled',?2
         FROM sa_headless_dispatch_requests WHERE {predicate}"),
        params![team_id,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_headless_dispatch_requests
         SET state='rejected',rejection='Team disabled',updated_at=?2
         WHERE {predicate}"),
        params![team_id,now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn enqueue(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    enqueue_mode(store,ticket_id,revision,key,false)
}

pub(super) fn enqueue_plan(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    enqueue_mode(store,ticket_id,revision,key,true)
}

/// Bind content, dependencies, routing and cancellations, but exclude
/// runtime acceptance and assignment changes from the reviewed plan digest.
fn plan_digest(db: &rusqlite::Connection, ticket_id: &str) -> Result<String> {
    let mut stmt=db.prepare(
        "SELECT w.id,w.parent_id,w.summary,r.worker_bot_id,r.routing_reason,c.item_id
         FROM sa_work_items w LEFT JOIN sa_work_item_routes r ON r.item_id=w.id
         LEFT JOIN sa_work_item_cancellations c ON c.item_id=w.id
         WHERE w.ticket_id=?1 ORDER BY w.id LIMIT 101",
    ).map_err(|e|e.to_string())?;
    let items=stmt.query_map([ticket_id],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?,
        r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,
    ))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e|e.to_string())?;
    if items.is_empty() || items.len()>100 {
        return Err("Plan progression requires a bounded reviewed WorkItem plan.".into());
    }
    let bytes=serde_json::to_vec(&items).map_err(|e|e.to_string())?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}

fn binding(db: &rusqlite::Connection, request_id: &str) -> Result<Option<String>> {
    db.query_row("SELECT plan_sha256 FROM sa_headless_plan_dispatch WHERE request_id=?1",
        [request_id],|r|r.get(0)).optional().map_err(|e|e.to_string())
}

fn bind_plan(tx: &Transaction<'_>, request: &DispatchRequest, digest: &str,
    previous: Option<&str>, now: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO sa_headless_plan_dispatch(request_id,plan_sha256,previous_request_id)
         VALUES(?1,?2,?3)",params![request.id,digest,previous],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_plan_dispatch_requested',?2,?3)",
        params![request.ticket_id,serde_json::json!({
            "requestId":request.id,"requestKey":request.request_key,
            "planSha256":digest,"previousRequestId":previous,"revision":request.expected_revision,
        }).to_string(),now],
    ).map_err(|e|e.to_string())?;
    Ok(())
}

fn enqueue_mode(
    store: &Store, ticket_id: &str, revision: i64, key: &str, plan: bool,
) -> Result<DispatchRequest> {
    if !valid_key(key) || revision < 0 || ticket_id.is_empty() || ticket_id.len() > 128
        || !ticket_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'))
    {
        return Err("Dispatch requires a bounded Ticket ID, revision and stable request key.".into());
    }
    let tx = Transaction::new_unchecked(&store.db, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if key.starts_with("plan-next.") {
        return Err("This request key namespace is reserved for native plan continuations.".into());
    }
    let now=storage::now();
    let existed=by_key(store,ticket_id,key)?.is_some();
    let request=enqueue_in_transaction(store,&tx,ticket_id,revision,key,now)?;
    if existed {
        if binding(&tx,&request.id)?.is_some()!=plan {
            return Err("Dispatch request key is bound to another progression mode.".into());
        }
    } else if plan {
        if request.kind!="coordinated" || super::next_work_item(store,ticket_id)?
            .is_none_or(|item|item.assignment_id.is_some())
        {
            return Err("Plan progression requires a queued coordinated Demand and a new ready WorkItem.".into());
        }
        let digest=plan_digest(&tx,ticket_id)?;
        bind_plan(&tx,&request,&digest,None,now)?;
    }
    tx.commit().map_err(|e|e.to_string())?;
    Ok(request)
}

fn enqueue_in_transaction(
    store: &Store, tx: &Transaction<'_>, ticket_id: &str, revision: i64, key: &str, now: i64,
) -> Result<DispatchRequest> {
    expire_pending(tx,now)?;
    reject_stale_pending(tx,ticket_id,now)?;
    if let Some(prior) = by_key(store, ticket_id, key)? {
        if prior.expected_revision != revision {
            return Err("Dispatch request key is already bound to another Ticket revision.".into());
        }
        return Ok(prior);
    }
    let ticket = get_ticket(store, ticket_id)?;
    let kind = match ticket.state.as_str() {
        "blocked" => "retry",
        "queued" => "coordinated",
        _ => return Err("Only a blocked or queued Demand can request native dispatch.".into()),
    };
    if ticket.revision != revision || ticket.actor != "local-user"
        || ticket.active_assignment_id.is_some()
    {
        return Err("Demand revision, actor or active assignment changed; dispatch request denied.".into());
    }
    let cfg = configuration(store, &ticket.orchestration_id)?;
    let grant = mandate(store, ticket_id)?;
    if !cfg.enabled || grant.revoked || grant.expires_at <= now
        || grant.actor != ticket.actor || grant.policy_revision != cfg.policy_revision
        || grant.project_id != cfg.project_id || !grant.allowed_tools.is_empty()
        || grant.allowed_side_effects || effects::unresolved_count(store, ticket_id)? != 0
    {
        return Err("Demand no longer has a current read-only native mandate.".into());
    }
    // Reuse the same ready-plan fence as native retry/coordinated admission.
    // A fresh confirmation cannot widen a cancelled plan into an unbound
    // objective; old request keys above remain inspectable.
    super::next_work_item(store, ticket_id)?;
    let unsafe_state: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments WHERE ticket_id=?1 AND state='running')
            OR EXISTS(SELECT 1 FROM sa_demand_archives WHERE ticket_id=?1)
            OR EXISTS(SELECT 1 FROM sa_headless_dispatch_requests WHERE ticket_id=?1 AND state='pending')
            OR (SELECT COUNT(*) FROM sa_headless_dispatch_requests WHERE state='pending')>=200",
        [ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if unsafe_state {
        return Err("Demand has an active reservation, archive or pending dispatch; inspect before requesting again.".into());
    }
    let id = storage::id();
    tx.execute(
        "INSERT INTO sa_headless_dispatch_requests
         (id,ticket_id,request_key,expected_revision,kind,state,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'pending',?6,?6)",
        params![id,ticket_id,key,revision,kind,now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_dispatch_requested',?2,?3)",
        params![ticket_id,format!("Explicit local operator dispatch request {id} at revision {revision}"),now],
    ).map_err(|e| e.to_string())?;
    by_key(store,ticket_id,key)?.ok_or_else(||"Dispatch request was not persisted.".into())
}

/// A chain may advance only from the exact accepted native Assignment.
/// Never infer authority from provider text, manual retry or a cancelled intent.
pub(super) fn advance_after_acceptance(
    store: &Store, tx: &Transaction<'_>, ticket_id: &str, accepted_revision: i64, now: i64,
) -> Result<()> {
    let prior:Option<(String,String)>=tx.query_row(
        "SELECT r.id,p.plan_sha256 FROM sa_headless_plan_dispatch p
         JOIN sa_headless_dispatch_requests r ON r.id=p.request_id
         JOIN sa_work_item_attempts b ON b.assignment_id=r.assignment_id
         JOIN sa_work_item_acceptances v ON v.assignment_id=b.assignment_id AND v.item_id=b.item_id
         WHERE r.ticket_id=?1 AND b.ticket_id=r.ticket_id AND r.state='admitted'
           AND r.kind='coordinated' AND v.accepted_revision=?2
           AND NOT EXISTS(SELECT 1 FROM sa_headless_plan_dispatch n WHERE n.previous_request_id=r.id)",
        params![ticket_id,accepted_revision],|r|Ok((r.get(0)?,r.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((previous,digest))=prior else {return Ok(());};
    let ticket=get_ticket(store,ticket_id)?;
    if ticket.state!="queued" || ticket.revision!=accepted_revision.saturating_add(1) {
        return Ok(());
    }
    expire_pending(tx,now)?;
    reject_stale_pending(tx,ticket_id,now)?;
    let occupied:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_headless_dispatch_requests WHERE ticket_id=?1 AND state='pending')
         OR (SELECT COUNT(*) FROM sa_headless_dispatch_requests WHERE state='pending')>=200",
        [ticket_id],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let ready=super::next_work_item(store,ticket_id)
        .is_ok_and(|item|item.is_some_and(|w|w.assignment_id.is_none()));
    if occupied || !ready || plan_digest(tx,ticket_id)?!=digest {
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'headless_plan_progression_paused','Plan changed, queue occupied or next item not new; explicit dispatch required',?2)",
            params![ticket_id,now],
        ).map_err(|e|e.to_string())?;
        return Ok(());
    }
    let key=format!("plan-next.{}",storage::id());
    let request=enqueue_in_transaction(store,tx,ticket_id,ticket.revision,&key,now)?;
    bind_plan(tx,&request,&digest,Some(&previous),now)
}

pub(crate) fn headless_enqueue(
    database_path: &std::path::Path, ticket_id: &str, revision: i64, key: &str,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let request = enqueue(&store, ticket_id, revision, key)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "dispatchRequest": request,
        "providerDispatchedByCommand": false,
        "grantIssued": false,
        "externalEffectReplayed": false,
    })).map_err(|e| e.to_string())
}

pub(crate) fn headless_enqueue_plan(
    database_path: &std::path::Path, ticket_id: &str, revision: i64, key: &str,
) -> Result<String> {
    let store=open_headless_transition_store(database_path)?;
    let request=enqueue_plan(&store,ticket_id,revision,key)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "dispatchRequest":request,"planProgressionRequested":true,
        "humanAcceptanceStillRequired":true,"providerDispatchedByCommand":false,
        "grantIssued":false,"externalEffectReplayed":false,
    })).map_err(|e|e.to_string())
}

/// Cancel only the exact local operator intent, not a reserved Assignment.
/// The expected revision belongs to the request, so a stale confirmation
/// remains cancellable after a WorkItem plan changes the Ticket revision.
pub(super) fn cancel(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    if !valid_key(key) || revision < 0 || ticket_id.is_empty() || ticket_id.len() > 128
        || !ticket_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'))
    {
        return Err("Cancellation requires a bounded Ticket ID, request revision and stable request key.".into());
    }
    let tx = Transaction::new_unchecked(&store.db, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let ticket = get_ticket(store, ticket_id)?;
    if ticket.actor != "local-user" {
        return Err("Only a local operator Demand dispatch can be cancelled.".into());
    }
    let prior = by_key(store, ticket_id, key)?.ok_or("Dispatch request not found.")?;
    if prior.expected_revision != revision {
        return Err("Dispatch request key is bound to another Ticket revision.".into());
    }
    match prior.state.as_str() {
        "rejected" => {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(prior);
        }
        "pending" => {}
        _ => return Err("Dispatch already admitted; use native Ticket Stop or recovery.".into()),
    }
    let now = storage::now();
    let changed = tx.execute(
        "UPDATE sa_headless_dispatch_requests
         SET state='rejected',rejection='cancelled by local operator',updated_at=?2
         WHERE id=?1 AND ticket_id=?3 AND expected_revision=?4 AND state='pending'",
        params![prior.id,now,ticket_id,revision],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Dispatch request was already settled or changed.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_dispatch_rejected',?2,?3)",
        params![ticket_id,format!("Dispatch request {} rejected: cancelled by local operator",prior.id),now],
    ).map_err(|e| e.to_string())?;
    let request = by_key(store, ticket_id, key)?.ok_or("Dispatch request was not persisted.")?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(request)
}

pub(crate) fn headless_cancel(
    database_path: &std::path::Path, ticket_id: &str, revision: i64, key: &str,
) -> Result<String> {
    let store = open_headless_transition_store(database_path)?;
    let request = cancel(&store, ticket_id, revision, key)?;
    serde_json::to_string_pretty(&serde_json::json!({
        "dispatchRequest": request,
        "providerDispatchedByCommand": false,
        "grantIssued": false,
        "externalEffectReplayed": false,
    })).map_err(|e| e.to_string())
}

/// Bounded FIFO snapshot: one occupied checkout must not prevent an
/// independently authorized Demand from reaching native admission.
pub(super) fn pending_batch(store: &Store) -> Result<Vec<DispatchRequest>> {
    let mut statement = store.db.prepare(
        &format!("{SELECT} WHERE state='pending' ORDER BY created_at,id LIMIT 200"),
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], row).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

#[cfg(test)]
pub(super) fn next_pending(store: &Store) -> Result<Option<DispatchRequest>> {
    Ok(pending_batch(store)?.into_iter().next())
}

pub(super) fn is_current(request: &DispatchRequest, now: i64) -> bool {
    request.created_at > 0 && now >= request.created_at
        && now.saturating_sub(request.created_at) < MAX_REQUEST_AGE_MS
}

/// Called from inside native Assignment admission, after the Assignment and
/// WorkItem binding have been inserted but before commit. A failed CAS rolls
/// back the entire native reservation, not just this request.
pub(super) fn consume(
    tx: &Transaction<'_>, request: &DispatchRequest, ticket: &Ticket,
    assignment_id: &str, now: i64,
) -> Result<()> {
    let expected_kind = match ticket.state.as_str() {
        "blocked" => "retry",
        "queued" => "coordinated",
        _ => return Err("Headless dispatch is not in its requested admission state.".into()),
    };
    if request.ticket_id != ticket.id || request.expected_revision != ticket.revision
        || request.kind != expected_kind || !is_current(request, now)
    {
        return Err("Headless dispatch request is stale, expired or bound to a different Demand.".into());
    }
    if let Some(digest)=binding(tx,&request.id)? {
        if request.kind!="coordinated" || plan_digest(tx,&ticket.id)?!=digest {
            return Err("Reviewed WorkItem plan changed; progression authorization is stale.".into());
        }
    }
    let changed = tx.execute(
        "UPDATE sa_headless_dispatch_requests SET state='admitted',assignment_id=?2,updated_at=?3
         WHERE id=?1 AND ticket_id=?4 AND expected_revision=?5 AND kind=?6
           AND state='pending' AND created_at=?7",
        params![request.id,assignment_id,now,ticket.id,ticket.revision,expected_kind,request.created_at],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Headless dispatch request was already settled or changed.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_dispatch_admitted',?2,?3)",
        params![ticket.id,format!("Dispatch request {} reserved native Assignment {assignment_id}",request.id),now],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Only known native pre-reservation occupancy outcomes may wait. All other
/// errors (including provider launch failures) retain terminal settlement.
/// Returning true lets the poller try the next independent pending request.
pub(super) fn settle_poll_failure(
    store: &Store, request: &DispatchRequest, reason: &str,
) -> Result<bool> {
    let busy = reason == super::RETRY_WORKER_OCCUPIED
        || reason == super::COORDINATED_WORKER_OCCUPIED;
    if busy && is_current(request, storage::now()) {
        return Ok(true);
    }
    let rejection = if busy {
        "Explicit headless dispatch request expired; inspect the Demand and confirm a new request."
    } else {
        reason
    };
    reject_pending(store, request, rejection)?;
    Ok(false)
}

/// Rejection is terminal for this key. A provider launch failure occurs only
/// after consumption, so it is settled by the existing Assignment recovery
/// and can never be relabeled a pre-admission rejection.
pub(super) fn reject_pending(store: &Store, request: &DispatchRequest, reason: &str) -> Result<()> {
    let tx = Transaction::new_unchecked(&store.db, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let now = storage::now();
    let detail: String = reason.chars().take(500).collect();
    let changed = tx.execute(
        "UPDATE sa_headless_dispatch_requests SET state='rejected',rejection=?2,updated_at=?3
         WHERE id=?1 AND ticket_id=?4 AND expected_revision=?5 AND state='pending'",
        params![request.id,detail,now,request.ticket_id,request.expected_revision],
    ).map_err(|e| e.to_string())?;
    if changed == 1 {
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'headless_dispatch_rejected',?2,?3)",
            params![request.ticket_id,format!("Dispatch request {} rejected: {detail}",request.id),now],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}
