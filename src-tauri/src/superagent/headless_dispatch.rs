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

/// An explicit Desktop authorization is distinct from a CLI-only intent.
/// A continuation inherits this immutable marker, never ambient host authority.
pub(crate) const DESKTOP_PLAN_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sa_desktop_plan_dispatch (
 request_id TEXT PRIMARY KEY REFERENCES sa_headless_plan_dispatch(request_id) ON DELETE RESTRICT
);
CREATE TRIGGER IF NOT EXISTS sa_desktop_plan_dispatch_immutable_update
 BEFORE UPDATE ON sa_desktop_plan_dispatch
 BEGIN SELECT RAISE(ABORT,'Desktop plan authorization is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_desktop_plan_dispatch_immutable_delete
 BEFORE DELETE ON sa_desktop_plan_dispatch
 BEGIN SELECT RAISE(ABORT,'Desktop plan authorization is immutable'); END;
";

#[derive(Debug, Clone, serde::Serialize)]
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

/// Read the single pending intent inside the Ticket snapshot transaction.
pub(super) fn pending_for_ticket(store: &Store, ticket_id: &str) -> Result<Option<DispatchRequest>> {
    store.db.query_row(&format!("{SELECT} WHERE ticket_id=?1 AND state='pending'"),
        [ticket_id], row).optional().map_err(|e| e.to_string())
}

/// Manual admission must not silently replace a live queued authorization.
/// The same immediate transaction serializes this check with consumption
/// and operator cancellation. Expired or stale intents cannot block a new
/// explicit admission; successful manual admission retires them atomically.
pub(super) fn require_no_live_pending(store: &Store, ticket: &Ticket) -> Result<()> {
    if pending_for_ticket(store, &ticket.id)?.is_some_and(|request|
        request.expected_revision == ticket.revision && is_current(&request, storage::now()))
    {
        return Err("Demand already has a pending dispatch; cancel it or wait for native admission.".into());
    }
    Ok(())
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
    tx: &rusqlite::Connection, ticket_id: &str, now: i64,
) -> Result<()> {
    retire_stale_pending(tx, Some(ticket_id), now)
}

/// The queue capacity is global. Retire obsolete intents across all Tickets
/// before counting capacity, even if another writer left an old intent behind.
/// This only rejects pending requests whose recorded revision no longer matches;
/// it cannot admit, replay or modify a current or already admitted request.
fn retire_stale_pending(
    tx: &rusqlite::Connection, ticket_id: Option<&str>, now: i64,
) -> Result<()> {
    let predicate = "(?1 IS NULL OR ticket_id=?1) AND state='pending' AND EXISTS(
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
    enqueue_mode(store,ticket_id,revision,key,false,false)
}

pub(super) fn enqueue_plan(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    enqueue_mode(store,ticket_id,revision,key,true,false)
}

pub(super) fn enqueue_desktop_plan(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    enqueue_mode(store,ticket_id,revision,key,true,true)
}

fn desktop_authorized(db: &rusqlite::Connection, request_id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_desktop_plan_dispatch WHERE request_id=?1)",
        [request_id], |r| r.get(0),
    ).map_err(|e| e.to_string())
}

fn mark_desktop(tx: &Transaction<'_>, request_id: &str) -> Result<()> {
    tx.execute("INSERT INTO sa_desktop_plan_dispatch(request_id) VALUES(?1)",
        [request_id]).map_err(|e| e.to_string())?;
    Ok(())
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

// Preserve legacy first-attempt plan digests. An explicitly authorized retry
// additionally pins the exact previous attempt; older intents cannot acquire
// retry authority merely because the selected WorkItem has changed.
fn attempt_plan_digest(plan: &str, previous: Option<&str>) -> String {
    match previous {
        Some(id) => format!("{:x}", Sha256::digest(format!("reviewed-plan-retry-v1:{plan}:{id}").as_bytes())),
        None => plan.to_owned(),
    }
}

fn previous_item_attempt(db: &rusqlite::Connection, ticket_id: &str, assignment_id: &str) -> Result<Option<String>> {
    db.query_row(
        "SELECT (SELECT prior.assignment_id FROM sa_work_item_attempts prior
                 JOIN sa_assignments a ON a.id=prior.assignment_id AND a.ticket_id=prior.ticket_id
                 WHERE prior.item_id=b.item_id AND prior.ticket_id=b.ticket_id
                   AND prior.assignment_id<>b.assignment_id
                 ORDER BY a.attempt DESC LIMIT 1)
         FROM sa_work_item_attempts b WHERE b.assignment_id=?1 AND b.ticket_id=?2",
        params![assignment_id,ticket_id], |r| r.get(0),
    ).map_err(|e| e.to_string())
}

fn binding(db: &rusqlite::Connection, request_id: &str) -> Result<Option<String>> {
    db.query_row("SELECT plan_sha256 FROM sa_headless_plan_dispatch WHERE request_id=?1",
        [request_id],|r|r.get(0)).optional().map_err(|e|e.to_string())
}

// A plain plan digest authorizes new WorkItems only. An explicit retry digest
// retains attempted-item priority, including requests created by older hosts.
// Consumption still atomically checks the actual reserved Assignment.
pub(super) fn prefers_new_work(store: &Store, request: Option<&DispatchRequest>) -> Result<bool> {
    let Some(request) = request else { return Ok(false); };
    let Some(digest) = binding(&store.db, &request.id)? else { return Ok(false); };
    Ok(digest == plan_digest(&store.db, &request.ticket_id)?)
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
    store: &Store, ticket_id: &str, revision: i64, key: &str, plan: bool, desktop: bool,
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
        if binding(&tx,&request.id)?.is_some()!=plan || desktop_authorized(&tx,&request.id)?!=desktop {
            return Err("Dispatch request key is bound to another progression mode or operator surface.".into());
        }
    } else if plan {
        if super::initial_coordinator_pending(store,ticket_id)? {
            return Err("Dispatch the initial Coordinator before authorizing a reviewed plan.".into());
        }
        let item=super::next_work_item_with_preference(store,ticket_id,true)?
            .ok_or("Plan progression requires a blocked or queued Demand and a ready WorkItem.")?;
        if item.assignment_id.is_some() && request.kind!="retry" {
            return Err("An attempted WorkItem requires a blocked Demand and explicit retry authorization.".into());
        }
        let digest=attempt_plan_digest(&plan_digest(&tx,ticket_id)?,item.assignment_id.as_deref());
        bind_plan(&tx,&request,&digest,None,now)?;
        if desktop { mark_desktop(&tx,&request.id)?; }
    }
    tx.commit().map_err(|e|e.to_string())?;
    Ok(request)
}

fn enqueue_in_transaction(
    store: &Store, tx: &Transaction<'_>, ticket_id: &str, revision: i64, key: &str, now: i64,
) -> Result<DispatchRequest> {
    expire_pending(tx,now)?;
    retire_stale_pending(tx,None,now)?;
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
    // A fresh intent must belong to the original admitted Ticket scope, not
    // just to a Mandate that matches today's Team. Check the persisted
    // Ticket fields in the same immediate transaction as queue insertion.
    let ticket_scope_current: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets
         WHERE id=?1 AND policy_revision=?2 AND project_id=?3)",
        params![ticket_id, grant.policy_revision, &grant.project_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !cfg.enabled || grant.revoked || grant.expires_at <= now
        || grant.actor != ticket.actor || grant.policy_revision != cfg.policy_revision
        || grant.project_id != cfg.project_id || !grant.allowed_tools.is_empty()
        || grant.authorization_id.trim().is_empty() || !ticket_scope_current
        || grant.allowed_side_effects || effects::unresolved_count(store, ticket_id)? != 0
    {
        return Err("Demand no longer has a current read-only native mandate.".into());
    }
    // Reuse the same ready-plan fence as native retry/coordinated admission.
    // A fresh confirmation cannot widen a cancelled plan into an unbound
    // objective; old request keys above remain inspectable.
    super::next_work_item(store, ticket_id)?;
    // Use the same immutable admission-time ceiling as the native Worker
    // dispatcher. Exhausted Demands must not reserve queue capacity for an
    // impossible retry. Coordinator attempts do not spend the Worker budget.
    // Existing request-key receipts returned above are still inspectable.
    let worker_attempts: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1 AND bot_id<>?2",
        params![ticket_id, &cfg.coordinator_bot_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if worker_attempts >= super::roster::ticket_worker_attempt_limit(store, ticket_id)? {
        return Err("Native Worker attempt limit reached; create a new authorized Demand.".into());
    }
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
    let prior:Option<(String,String,String)>=tx.query_row(
        "SELECT r.id,p.plan_sha256,b.assignment_id FROM sa_headless_plan_dispatch p
         JOIN sa_headless_dispatch_requests r ON r.id=p.request_id
         JOIN sa_work_item_attempts b ON b.assignment_id=r.assignment_id
         JOIN sa_work_item_acceptances v ON v.assignment_id=b.assignment_id AND v.item_id=b.item_id
         WHERE r.ticket_id=?1 AND b.ticket_id=r.ticket_id AND r.state='admitted'
           AND v.accepted_revision=?2
           AND NOT EXISTS(SELECT 1 FROM sa_headless_plan_dispatch n WHERE n.previous_request_id=r.id)",
        params![ticket_id,accepted_revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((previous,digest,assignment_id))=prior else {return Ok(());};
    let ticket=get_ticket(store,ticket_id)?;
    if !matches!(ticket.state.as_str(),"queued"|"blocked")
        || ticket.revision!=accepted_revision.saturating_add(1) {
        return Ok(());
    }
    expire_pending(tx,now)?;
    retire_stale_pending(tx,None,now)?;
    let occupied:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_headless_dispatch_requests WHERE ticket_id=?1 AND state='pending')
         OR (SELECT COUNT(*) FROM sa_headless_dispatch_requests WHERE state='pending')>=200",
        [ticket_id],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let ready=super::next_work_item_with_preference(store,ticket_id,true)
        .is_ok_and(|item|item.is_some_and(|w|w.assignment_id.is_none()));
    let current_plan=plan_digest(tx,ticket_id)?;
    let previous_attempt=previous_item_attempt(tx,ticket_id,&assignment_id)?;
    if occupied || !ready || attempt_plan_digest(&current_plan,previous_attempt.as_deref())!=digest {
        tx.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'headless_plan_progression_paused','Plan changed, queue occupied or next item not new; explicit dispatch required',?2)",
            params![ticket_id,now],
        ).map_err(|e|e.to_string())?;
        return Ok(());
    }
    let key=format!("plan-next.{}",storage::id());
    let request=enqueue_in_transaction(store,tx,ticket_id,ticket.revision,&key,now)?;
    // Only the explicitly requested retry may repeat an item. Every automatic
    // continuation is bound to a new item and the original plain plan digest.
    bind_plan(tx,&request,&current_plan,Some(&previous),now)?;
    if desktop_authorized(tx,&previous)? { mark_desktop(tx,&request.id)?; }
    Ok(())
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

/// Desktop consumes only plans explicitly authorized on Desktop. The CLI
/// retains its existing serve-only semantics. A later serve host may continue
/// an explicitly authorized Desktop plan through the same native CAS.
pub(super) fn pending_for_host(store: &Store, desktop_only: bool) -> Result<Vec<DispatchRequest>> {
    if !desktop_only { return pending_batch(store); }
    let mut stmt = store.db.prepare(&format!(
        "{SELECT} WHERE state='pending' AND EXISTS(
            SELECT 1 FROM sa_desktop_plan_dispatch d
            WHERE d.request_id=sa_headless_dispatch_requests.id)
         ORDER BY created_at,id LIMIT 200"
    )).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], row).map_err(|e| e.to_string())?;
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
        let previous=previous_item_attempt(tx,&ticket.id,assignment_id)?;
        if (previous.is_some() && expected_kind!="retry")
            || attempt_plan_digest(&plan_digest(tx,&ticket.id)?,previous.as_deref())!=digest
        {
            return Err("Reviewed WorkItem plan or prior attempt changed; progression authorization is stale.".into());
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

/// An occupied, still-current request can wait. A terminal pre-admission
/// rejection also lets the bounded poll scan continue: it did not reserve
/// an Assignment. An admitted receipt always consumes the poll's admission
/// budget, even if delivery subsequently failed or another host admitted it.
pub(super) fn settle_poll_failure(
    store: &Store, request: &DispatchRequest, reason: &str,
) -> Result<bool> {
    let busy = reason == super::RETRY_WORKER_OCCUPIED
        || reason == super::COORDINATED_WORKER_OCCUPIED
        || reason == super::INITIAL_COORDINATOR_OCCUPIED;
    let rejection = if busy {
        "Explicit headless dispatch request expired; inspect the Demand and confirm a new request."
    } else {
        reason
    };
    settle_pending(store, request, rejection, busy)
}

/// Rejection is terminal for this key. Return whether the poll may continue
/// without exceeding its one-native-reservation budget.
pub(super) fn reject_pending(store: &Store, request: &DispatchRequest, reason: &str) -> Result<bool> {
    settle_pending(store, request, reason, false)
}

fn settle_pending(
    store: &Store, request: &DispatchRequest, reason: &str, defer_if_busy: bool,
) -> Result<bool> {
    let tx = Transaction::new_unchecked(&store.db, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let now = storage::now();
    // The durable receipt, not a stale in-memory snapshot or a provider
    // error string, decides whether this poll has already reserved work.
    let state: String = tx.query_row(
        "SELECT state FROM sa_headless_dispatch_requests
         WHERE id=?1 AND ticket_id=?2 AND expected_revision=?3",
        params![request.id, request.ticket_id, request.expected_revision],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    match state.as_str() {
        "admitted" | "rejected" => {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(state == "rejected");
        }
        "pending" => {}
        _ => return Err("Invalid native dispatch receipt state.".into()),
    }
    if defer_if_busy && is_current(request, now) {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(true);
    }
    let detail: String = reason.chars().take(500).collect();
    let changed = tx.execute(
        "UPDATE sa_headless_dispatch_requests SET state='rejected',rejection=?2,updated_at=?3
         WHERE id=?1 AND ticket_id=?4 AND expected_revision=?5 AND state='pending'",
        params![request.id,detail,now,request.ticket_id,request.expected_revision],
    ).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Dispatch request was already settled or changed.".into());
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_dispatch_rejected',?2,?3)",
        params![request.ticket_id,format!("Dispatch request {} rejected: {detail}",request.id),now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}


#[cfg(test)]
mod stale_capacity_tests {
    use super::*;

    #[test]
    fn new_dispatch_denies_drifted_ticket_scope_or_blank_mandate_issuer() {
        let path = std::env::temp_dir().join(format!("sa-dispatch-scope-{}.sqlite3", storage::id()));
        let store = Store::open(&path).unwrap();
        for (id, checkout) in [("project", "/tmp/sa-dispatch-scope"), ("other", "/tmp/sa-dispatch-other")] {
            store.db.execute(
                "INSERT INTO projects(id,name,path) VALUES(?1,?1,?2)",
                params![id,checkout],
            ).unwrap();
        }
        for id in ["router", "worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg = super::super::create_configuration(&store, super::super::NewOrchestration {
            name: "Dispatch scope".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&cfg.id],
        ).unwrap();
        for (case, mutation) in [
            ("ticket-policy", "UPDATE sa_tickets SET policy_revision=policy_revision+1 WHERE id=?1"),
            ("ticket-project", "UPDATE sa_tickets SET project_id='other' WHERE id=?1"),
            ("missing-issuer", "UPDATE sa_mandates SET authorization_id='' WHERE ticket_id=?1"),
            ("blank-issuer", "UPDATE sa_mandates SET authorization_id='  ' WHERE ticket_id=?1"),
        ] {
            let raw = super::super::headless_queue_demand(
                &path, &cfg.id, "Inspect source", case,
            ).unwrap();
            let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
            let id = view["ticketId"].as_str().unwrap();
            let revision = view["revision"].as_i64().unwrap();
            store.db.execute(mutation, [id]).unwrap();
            let audit_before: i64 = store.db.query_row(
                "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
                [id], |row| row.get(0),
            ).unwrap();
            assert!(enqueue(&store, id, revision, "denied").is_err(), "{case}");
            assert!(by_key(&store, id, "denied").unwrap().is_none(), "{case}");
            assert_eq!(store.db.query_row(
                "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
                [id], |row| row.get::<_, i64>(0),
            ).unwrap(), audit_before, "{case}");
            assert_eq!(store.db.query_row(
                "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
                [id], |row| row.get::<_, i64>(0),
            ).unwrap(), 0, "{case}");
            assert_eq!(get_ticket(&store, id).unwrap().revision, revision);
        }

        let raw = super::super::headless_queue_demand(
            &path, &cfg.id, "Inspect source", "valid-scope",
        ).unwrap();
        let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let id = view["ticketId"].as_str().unwrap();
        let revision = view["revision"].as_i64().unwrap();
        let request = enqueue(&store, id, revision, "current").unwrap();
        assert_eq!(request.state, "pending");
        // Idempotent inspection of an existing key is not a new grant,
        // even if the scope subsequently becomes invalid.
        store.db.execute(
            "UPDATE sa_mandates SET authorization_id='' WHERE ticket_id=?1", [id],
        ).unwrap();
        assert_eq!(enqueue(&store, id, revision, "current").unwrap().id, request.id);
        assert!(enqueue(&store, id, revision, "another").is_err());
        assert_eq!(pending_batch(&store).unwrap().len(), 1);
    }

    #[test]
    fn new_dispatch_retires_unrelated_stale_intents_without_touching_current_requests() {
        let path = std::env::temp_dir().join(format!("sa-stale-capacity-{}.sqlite3", storage::id()));
        let store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('project','Disposable','/tmp/sa-stale-capacity')",
            [],
        ).unwrap();
        for id in ["router", "worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg = super::super::create_configuration(&store, super::super::NewOrchestration {
            name: "Dispatch capacity".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&cfg.id],
        ).unwrap();
        let stage = |key: &str| {
            let raw = super::super::headless_queue_demand(&path, &cfg.id, "Inspect source", key).unwrap();
            let view: serde_json::Value = serde_json::from_str(&raw).unwrap();
            (view["ticketId"].as_str().unwrap().to_owned(), view["revision"].as_i64().unwrap())
        };
        let (stale, stale_rev) = stage("stale-demand");
        let (current, current_rev) = stage("current-demand");
        let (fresh, fresh_rev) = stage("fresh-demand");
        let old = enqueue(&store, &stale, stale_rev, "old").unwrap();
        let live = enqueue(&store, &current, current_rev, "live").unwrap();
        // Simulate a legacy writer leaving an obsolete intent behind.
        store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1", [&stale]).unwrap();
        store.db.execute_batch(
            "CREATE TRIGGER fail_stale_audit BEFORE INSERT ON sa_audit
             WHEN NEW.kind='headless_dispatch_rejected'
             BEGIN SELECT RAISE(ABORT,'test audit failure'); END;"
        ).unwrap();
        assert!(enqueue(&store, &fresh, fresh_rev, "new").is_err());
        assert_eq!(by_key(&store, &stale, "old").unwrap().unwrap().state, "pending");
        assert!(by_key(&store, &fresh, "new").unwrap().is_none());
        store.db.execute_batch("DROP TRIGGER fail_stale_audit;").unwrap();

        let next = enqueue(&store, &fresh, fresh_rev, "new").unwrap();
        let retired = by_key(&store, &stale, "old").unwrap().unwrap();
        assert_eq!(retired.id, old.id);
        assert_eq!(retired.state, "rejected");
        assert_eq!(retired.rejection.as_deref(), Some("Ticket revision changed"));
        assert_eq!(by_key(&store, &current, "live").unwrap().unwrap().id, live.id);
        assert_eq!(by_key(&store, &current, "live").unwrap().unwrap().state, "pending");
        assert_eq!(enqueue(&store, &fresh, fresh_rev, "new").unwrap().id, next.id);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1 AND kind='headless_dispatch_rejected'",
            [&stale], |row| row.get::<_, i64>(0),
        ).unwrap(), 1);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id IN (?1,?2,?3)",
            params![stale,current,fresh], |row| row.get::<_, i64>(0),
        ).unwrap(), 0);
    }

    #[test]
    fn exhausted_worker_budget_rejects_new_dispatch_but_preserves_receipts_and_other_demands() {
        let path = std::env::temp_dir().join(format!("sa-dispatch-budget-{}.sqlite3", storage::id()));
        let mut store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('project','Disposable','/tmp/sa-dispatch-budget')",
            [],
        ).unwrap();
        for id in ["router", "worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'project','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let cfg = super::super::create_configuration(&store, super::super::NewOrchestration {
            name: "Dispatch budget".into(), coordinator_bot_id: "router".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let bounded = super::super::roster::set_worker_attempt_limit(
            &store, &cfg.id, cfg.policy_revision, 1,
        ).unwrap();
        let cfg = super::super::change_enabled(
            &store, &cfg.id, bounded.policy_revision, true,
        ).unwrap();
        let (ticket, launched) = super::super::admit_role_with_staging(
            &mut store, &cfg.id, "Inspect source", "budgeted-demand", false, false, true,
        ).unwrap();
        assert!(!launched);
        super::super::work_items::plan(
            &store, &ticket.id, ticket.revision, "root", "Inspect root", None,
        ).unwrap();
        let revision = get_ticket(&store, &ticket.id).unwrap().revision;
        let original = enqueue_desktop_plan(&store, &ticket.id, revision, "first").unwrap();
        let (running, _, _, _) = super::super::retry_blocked_with_request(
            &mut store, &ticket.id, revision, false, Some(&original),
        ).unwrap();
        super::super::fail_launch(&store, &running, "test provider unavailable").unwrap();
        let blocked = get_ticket(&store, &ticket.id).unwrap();
        assert_eq!(blocked.state, "blocked");
        let audit_before: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |r| r.get(0),
        ).unwrap();

        assert!(enqueue(&store, &ticket.id, blocked.revision, "retry").unwrap_err()
            .contains("attempt limit reached"));
        assert!(enqueue_plan(&store, &ticket.id, blocked.revision, "cli-plan").is_err());
        assert!(enqueue_desktop_plan(&store, &ticket.id, blocked.revision, "desktop-plan").is_err());
        assert!(pending_batch(&store).unwrap().is_empty());
        assert_eq!(get_ticket(&store, &ticket.id).unwrap().revision, blocked.revision);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_audit WHERE ticket_id=?1",
            [&ticket.id], |r| r.get::<_, i64>(0),
        ).unwrap(), audit_before);
        let replay = enqueue_desktop_plan(&store, &ticket.id, revision, "first").unwrap();
        assert_eq!(replay.id, original.id);
        assert_eq!(replay.state, "admitted");

        // A spent Ticket must not consume queue capacity or another Demand's budget.
        let (other, launched) = super::super::admit_role_with_staging(
            &mut store, &cfg.id, "Inspect another component", "independent-demand", false, false, true,
        ).unwrap();
        assert!(!launched);
        let next = enqueue(&store, &other.id, other.revision, "independent").unwrap();
        assert_eq!(pending_batch(&store).unwrap()[0].id, next.id);
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_assignments WHERE ticket_id=?1",
            [&ticket.id], |r| r.get::<_, i64>(0),
        ).unwrap(), 1);
    }

}
