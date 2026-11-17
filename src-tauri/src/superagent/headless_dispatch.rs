//! Local operator-requested headless retry outbox.
//!
//! A one-shot CLI cannot safely launch a provider or recover a running host.
//! It records one exact, revision-bound intent. Only the live serve runtime
//! claims it and reuses native SuperAgent admission and the existing bot gateway.
//! Unknown dispatch outcomes are never automatically replayed.

use super::effects;
use crate::{bots, storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use tauri::Manager;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_headless_retry_requests (
    request_id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    requested_revision INTEGER NOT NULL CHECK(requested_revision>=0),
    state TEXT NOT NULL CHECK(state IN (
        'queued','claimed','dispatched','uncertain','rejected'
    )),
    requested_at INTEGER NOT NULL,
    claimed_at INTEGER,
    finished_at INTEGER,
    UNIQUE(ticket_id,requested_revision)
);
CREATE INDEX IF NOT EXISTS sa_headless_retry_pending
    ON sa_headless_retry_requests(state,requested_at,request_id);
"#;
const MAX_RETRY_REQUESTS: i64 = 10000;
const CLAIM_TIMEOUT_MS: i64 = 120_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RetryReceipt {
    pub ticket_id: String,
    pub requested_revision: i64,
    pub state: String,
    pub requested_at: i64,
    pub claimed_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub provider_completed: bool,
    pub automatic_replay_permitted: bool,
}

fn receipt(store: &Store, ticket_id: &str, expected_revision: i64) -> Result<RetryReceipt> {
    store.db.query_row(
        "SELECT ticket_id,requested_revision,state,requested_at,claimed_at,finished_at
         FROM sa_headless_retry_requests WHERE ticket_id=?1 AND requested_revision=?2",
        params![ticket_id,expected_revision],
        |row| {
            let state: String = row.get(2)?;
            Ok(RetryReceipt {
                ticket_id: row.get(0)?,
                requested_revision: row.get(1)?,
                provider_completed: false,
                automatic_replay_permitted: state == "queued",
                state,
                requested_at: row.get(3)?,
                claimed_at: row.get(4)?,
                finished_at: row.get(5)?,
            })
        },
    ).map_err(|_| "Headless retry request not found.".into())
}

/// CLI read-only status. No Store::open recovery and no native task admission.
pub(crate) fn inspect(database_path: &Path, ticket_id: &str, expected_revision: i64) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    serde_json::to_string_pretty(&receipt(&store,ticket_id,expected_revision)?)
        .map_err(|error|error.to_string())
}

/// Explicit local operator request only. A replay of the same revision returns
/// the existing receipt; it can never create a second provider attempt.
pub(crate) fn request(database_path: &Path, ticket_id: &str, expected_revision: i64) -> Result<String> {
    let store = super::open_headless_transition_store(database_path)?;
    let _fence = effects::lock_dispatch(database_path)?;
    let tx = store.db.unchecked_transaction().map_err(|error|error.to_string())?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_headless_retry_requests
          WHERE ticket_id=?1 AND requested_revision=?2)",
        params![ticket_id,expected_revision], |row|row.get(0),
    ).map_err(|error|error.to_string())?;
    if exists {
        return serde_json::to_string_pretty(&receipt(&store,ticket_id,expected_revision)?)
            .map_err(|error|error.to_string());
    }
    let ticket = super::get_ticket(&store,ticket_id)?;
    if ticket.revision != expected_revision || ticket.state != "blocked" {
        return Err("Only the current blocked Ticket revision may request a headless retry.".into());
    }
    let team = super::configuration(&store,&ticket.orchestration_id)?;
    let mandate = super::mandate(&store,ticket_id)?;
    if !team.enabled || crate::feature_controls::get().bots_paused
        || mandate.revoked || mandate.expires_at <= storage::now()
        || mandate.policy_revision != ticket_team_revision(&store,ticket_id)?
        || mandate.project_id != team.project_id
        || mandate.actor != ticket.actor
        || mandate.allowed_side_effects || !mandate.allowed_tools.is_empty()
    {
        return Err("Headless retry requires a current, unrevoked read-only native mandate.".into());
    }
    let unresolved = effects::unresolved_count(&store,ticket_id)?;
    if unresolved != 0 {
        return Err("Unresolved external effects require manual reconciliation before retry.".into());
    }
    let total: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sa_headless_retry_requests",[],|row|row.get(0),
    ).map_err(|error|error.to_string())?;
    if total >= MAX_RETRY_REQUESTS {
        return Err("Headless retry receipt limit reached; operator maintenance required.".into());
    }
    tx.execute(
        "INSERT INTO sa_headless_retry_requests(
           request_id,ticket_id,requested_revision,state,requested_at)
         VALUES(?1,?2,?3,'queued',?4)",
        params![storage::id(),ticket_id,expected_revision,storage::now()],
    ).map_err(|error|error.to_string())?;
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'headless_retry_requested','local operator requested read-only retry',?2)",
        params![ticket_id,storage::now()],
    ).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error|error.to_string())?;
    serde_json::to_string_pretty(&receipt(&store,ticket_id,expected_revision)?)
        .map_err(|error|error.to_string())
}

fn ticket_team_revision(store: &Store, ticket_id: &str) -> Result<i64> {
    Ok(super::ticket_team_snapshot(store,ticket_id)?.policy.policy_revision)
}

/// Only a claim with NO new assignment and an unchanged blocked Ticket is
/// recoverable after a possible server crash. Every other stale claim remains
/// uncertain; never attempt another provider call from ambiguous state.
fn recover_stale_claims(store: &Store, now: i64) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|error|error.to_string())?;
    tx.execute(
        "UPDATE sa_headless_retry_requests SET state='queued',claimed_at=NULL
         WHERE state='claimed' AND claimed_at<=?1
           AND EXISTS(SELECT 1 FROM sa_tickets t
             WHERE t.id=sa_headless_retry_requests.ticket_id
               AND t.revision=sa_headless_retry_requests.requested_revision
               AND t.state='blocked' AND t.active_assignment_id IS NULL)
           AND NOT EXISTS(SELECT 1 FROM sa_assignments a
             WHERE a.ticket_id=sa_headless_retry_requests.ticket_id
               AND a.created_at>=sa_headless_retry_requests.claimed_at)",
        [now.saturating_sub(CLAIM_TIMEOUT_MS)],
    ).map_err(|error|error.to_string())?;
    tx.execute(
        "UPDATE sa_headless_retry_requests SET state='uncertain',finished_at=?1
         WHERE state='claimed' AND claimed_at<=?2",
        params![now,now.saturating_sub(CLAIM_TIMEOUT_MS)],
    ).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error|error.to_string())
}

/// Compare-and-swap one durable queued request before any native admission.
fn claim_next(store: &Store, now: i64) -> Result<Option<(String,i64)>> {
    let tx = store.db.unchecked_transaction().map_err(|error|error.to_string())?;
    let next: Option<(String,i64)> = tx.query_row(
        "SELECT ticket_id,requested_revision FROM sa_headless_retry_requests
         WHERE state='queued' ORDER BY requested_at,request_id LIMIT 1",
        [], |row|Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|error|error.to_string())?;
    let Some((ticket_id,revision))=next else { return Ok(None); };
    let updated = tx.execute(
        "UPDATE sa_headless_retry_requests SET state='claimed',claimed_at=?3
         WHERE ticket_id=?1 AND requested_revision=?2 AND state='queued'",
        params![ticket_id,revision,now],
    ).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error|error.to_string())?;
    Ok((updated == 1).then_some((ticket_id,revision)))
}

fn settle(store: &Store, ticket_id: &str, requested_revision: i64, state: &str) -> Result<()> {
    if !matches!(state, "queued"|"dispatched"|"uncertain"|"rejected") {
        return Err("Unsupported headless retry settlement.".into());
    }
    let now = storage::now();
    let updated = store.db.execute(
        "UPDATE sa_headless_retry_requests
         SET state=?3,claimed_at=CASE WHEN ?3='queued' THEN NULL ELSE claimed_at END,
             finished_at=CASE WHEN ?3='queued' THEN NULL ELSE ?4 END
         WHERE ticket_id=?1 AND requested_revision=?2 AND state='claimed'",
        params![ticket_id,requested_revision,state,now],
    ).map_err(|error|error.to_string())?;
    if updated != 1 {
        return Err("Headless retry claim was already settled or replaced.".into());
    }
    Ok(())
}

/// Called only by a live, existing serve process. Provider dispatch is still
/// read-only, separately scoped and bounded by retry_blocked's native checks.
/// An in-flight CI run, remote provider failure, or another blocked Ticket never
/// widens the original mandate.
pub(crate) async fn drain_one(app: tauri::AppHandle) -> Result<bool> {
    let state = app.state::<AppState>();
    let active: Vec<String> = state.agents.lock().map_err(|error|error.to_string())?
        .keys().cloned().collect();
    let prepared = {
        let mut store = state.store.lock().map_err(|error|error.to_string())?;
        recover_stale_claims(&store,storage::now())?;
        let Some((ticket_id,revision))=claim_next(&store,storage::now())? else {
            return Ok(false);
        };
        let admitted = (|| -> Result<(super::Ticket,String,String)> {
            let ticket=super::get_ticket(&store,&ticket_id)?;
            if ticket.state!="blocked" || ticket.revision!=revision {
                return Err("Original Ticket changed before headless retry admission.".into());
            }
            let cfg=super::configuration(&store,&ticket.orchestration_id)?;
            let worker=super::ticket_worker(&store,&ticket_id)?;
            let busy=super::active_manual_conflict(
                &store,&active,&worker,&cfg.project_id,
            )?;
            super::retry_blocked(&mut store,&ticket_id,busy)
        })();
        match admitted {
            Ok((ticket,thread,worker)) => Some((ticket,thread,worker,revision)),
            Err(error) => {
                if error.contains("occupied") || error.contains("Worker is busy") {
                    // No assignment or external call was started; a bounded
                    // future poll may try again under the same original request.
                    settle(&store,&ticket_id,revision,"queued")?;
                    return Ok(false);
                }
                // No automatic retry after a failed native policy recheck.
                settle(&store,&ticket_id,revision,"rejected")?;
                return Err(error);
            }
        }
    };
    let Some((ticket,thread,worker,revision))=prepared else { return Ok(false) };
    let outcome=bots::gateway::deliver(&app,bots::gateway::Inbound {
        bot_id: worker,
        channel_id: super::CHANNEL.into(),
        thread,
        sender_id: "local-user".into(),
        sender_name: "Headless retry".into(),
        text: format!(
            "SuperAgent ticket {} (headless manual retry)\n\n{}\n\nRead-only assignment. Report evidence and remaining gaps; do not claim approval.",
            ticket.id,ticket.objective,
        ),
        trusted: true,
        image_ids: None,
        moodboard_ids: None,
    }).await;
    let store=state.store.lock().map_err(|error|error.to_string())?;
    match outcome {
        Ok(_session_id) => {
            settle(&store,&ticket.id,revision,"dispatched")?;
            Ok(true)
        }
        Err(error) => {
            // A gateway error may be ambiguous after handing off to a
            // provider. Do not automatically redeliver; keep durable evidence
            // and let native recovery or a new explicit local request decide.
            let _=super::fail_launch(&store,&ticket.id,&error);
            settle(&store,&ticket.id,revision,"uncertain")?;
            Err(format!("Headless retry delivery is uncertain for Ticket {}.",ticket.id))
        }
    }
}
