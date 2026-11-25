//! Durable, explicit local-operator dispatch intents consumed only by the live
//! headless host. A request is not a grant, assignment, provider receipt, or
//! Council verdict. Admission and consumption share the existing native
//! Assignment transaction; a committed reservation is never automatically replayed.
use super::{configuration, effects, get_ticket, mandate, open_headless_transition_store, Ticket};
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension, Transaction};

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

pub(super) fn enqueue(
    store: &Store, ticket_id: &str, revision: i64, key: &str,
) -> Result<DispatchRequest> {
    if !valid_key(key) || revision < 0 || ticket_id.is_empty() || ticket_id.len() > 128
        || !ticket_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'))
    {
        return Err("Dispatch requires a bounded Ticket ID, revision and stable request key.".into());
    }
    let tx = Transaction::new_unchecked(&store.db, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let now = storage::now();
    expire_pending(&tx, now)?;
    if let Some(prior) = by_key(store, ticket_id, key)? {
        if prior.expected_revision != revision {
            return Err("Dispatch request key is already bound to another Ticket revision.".into());
        }
        tx.commit().map_err(|e| e.to_string())?;
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
    let request = by_key(store, ticket_id, key)?.ok_or("Dispatch request was not persisted.")?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(request)
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

pub(super) fn next_pending(store: &Store) -> Result<Option<DispatchRequest>> {
    store.db.query_row(&format!("{SELECT} WHERE state='pending' ORDER BY created_at,id LIMIT 1"),
        [], row).optional().map_err(|e| e.to_string())
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
