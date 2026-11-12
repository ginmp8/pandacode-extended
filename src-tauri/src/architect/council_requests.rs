//! Trusted, task-bound native Council admission. This does not dispatch reviewers
//! or create an alternative score/approval engine. Channel delivery admission
//! owns the transaction; only the existing Council may decide a verdict.
use super::{council, CouncilConfig};
use crate::{storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS extended_council_task_requests(
      request_id TEXT PRIMARY KEY,
      delivery_id TEXT NOT NULL UNIQUE REFERENCES extended_channel_council_deliveries(delivery_id) ON DELETE CASCADE,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      context_json TEXT NOT NULL CHECK(json_valid(context_json)),
      created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS extended_council_task_requests_session ON extended_council_task_requests(session_id,created_at);
";

/// Immutable per-delivery scope. The digest covers the admitted *objective*;
/// it is not a verified source-code SHA or an approval token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CouncilTaskReviewRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub delivery_id: String,
    pub session_id: String,
    pub channel_id: String,
    pub source_message_id: String,
    pub executor_bot_id: String,
    pub delivery_cycle_id: String,
    pub objective_sha256: String,
    pub policy_sha256: String,
}

/// Read-only current-cycle projection from the *existing* native Council.
/// It is not a durable terminal receipt or permission to publish an effect.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CouncilTaskInspection {
    pub request: CouncilTaskReviewRequest,
    pub native: council::CouncilInspection,
}

struct ChannelScope {
    session_id: String,
    channel_id: String,
    source_message_id: String,
    executor_bot_id: String,
    delivery_cycle_id: String,
    objective: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn channel_scope(store: &Store, delivery_id: &str) -> Result<ChannelScope> {
    store.db.query_row(
        "SELECT d.session_id,d.channel_id,d.message_id,d.bot_id,c.cycle_id,c.objective
         FROM bot_shared_channel_deliveries d
         JOIN extended_channel_council_deliveries c ON c.delivery_id=d.id
           AND c.channel_id=d.channel_id
           AND c.source_message_id=d.message_id
           AND c.executor_bot_id=d.bot_id
         WHERE d.id=?1",
        [delivery_id],
        |row| Ok(ChannelScope {
            session_id: row.get(0)?, channel_id: row.get(1)?,
            source_message_id: row.get(2)?, executor_bot_id: row.get(3)?,
            delivery_cycle_id: row.get(4)?, objective: row.get(5)?,
        }),
    ).optional().map_err(|e| e.to_string())?
        .ok_or_else(|| "council_task_denied: linked channel delivery not found.".to_owned())
}

fn frozen_config(store: &Store, session_id: &str) -> Result<(CouncilConfig, bool)> {
    if super::parent_of(store, session_id)?.is_some() {
        return Err("council_task_denied: a reviewer worker cannot own a root request.".into());
    }
    let team = super::load(store, session_id)?
        .ok_or("council_task_denied: native Council task was not configured.")?;
    let config = team.config.council
        .ok_or("council_task_denied: task Council is disabled.")?;
    Ok((config, team.council_since == 0 && team.workers.is_empty()))
}

fn request_for(scope: &ChannelScope, delivery_id: &str, config: &CouncilConfig, request_id: String) -> Result<CouncilTaskReviewRequest> {
    let policy = serde_json::to_vec(config).map_err(|e| e.to_string())?;
    Ok(CouncilTaskReviewRequest {
        schema_version: 1,
        request_id,
        delivery_id: delivery_id.to_owned(),
        session_id: scope.session_id.clone(),
        channel_id: scope.channel_id.clone(),
        source_message_id: scope.source_message_id.clone(),
        executor_bot_id: scope.executor_bot_id.clone(),
        delivery_cycle_id: scope.delivery_cycle_id.clone(),
        objective_sha256: digest(scope.objective.as_bytes()),
        policy_sha256: digest(&policy),
    })
}

/// Idempotent admission after channel + native team rows exist in the SAME
/// transaction. No permissions, reviewers or approvals are created here.
pub(crate) fn admit_channel(store: &Store, delivery_id: &str) -> Result<CouncilTaskReviewRequest> {
    let scope = channel_scope(store, delivery_id)?;
    let (config, fresh) = frozen_config(store, &scope.session_id)?;
    let existing: Option<String> = store.db.query_row(
        "SELECT context_json FROM extended_council_task_requests WHERE delivery_id=?1",
        [delivery_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let candidate = request_for(&scope, delivery_id, &config, storage::id())?;
    if let Some(raw) = existing {
        let previous: CouncilTaskReviewRequest = serde_json::from_str(&raw)
            .map_err(|_| "council_task_denied: stored request is invalid.")?;
        let same = previous == CouncilTaskReviewRequest { request_id: previous.request_id.clone(), ..candidate };
        return if same { Ok(previous) } else {
            Err("council_task_conflict: this delivery was already admitted with different authority or evidence.".into())
        };
    }
    if !fresh {
        return Err("council_task_denied: a review cannot be admitted after its native cycle started.".into());
    }
    let body = serde_json::to_string(&candidate).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO extended_council_task_requests(request_id,delivery_id,session_id,context_json,created_at)
         VALUES(?1,?2,?3,?4,?5)",
        params![candidate.request_id,delivery_id,scope.session_id,body,storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(candidate)
}

/// Native-only scoped read; callers must know this request's exact delivery
/// and session, which are revalidated against persisted ownership and pins.
pub(crate) fn inspect_channel(store: &Store, request_id: &str, delivery_id: &str, session_id: &str) -> Result<CouncilTaskInspection> {
    let raw: String = store.db.query_row(
        "SELECT context_json FROM extended_council_task_requests
         WHERE request_id=?1 AND delivery_id=?2 AND session_id=?3",
        params![request_id,delivery_id,session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("council_task_denied: request, delivery or task do not match.")?;
    let recorded: CouncilTaskReviewRequest = serde_json::from_str(&raw)
        .map_err(|_| "council_task_denied: stored request is invalid.")?;
    if recorded.request_id != request_id || recorded.delivery_id != delivery_id || recorded.session_id != session_id {
        return Err("council_task_denied: request identity is inconsistent.".into());
    }
    let scope = channel_scope(store, delivery_id)?;
    let (config, _) = frozen_config(store, session_id)?;
    let current = request_for(&scope, delivery_id, &config, recorded.request_id.clone())?;
    if current != recorded {
        return Err("council_task_stale: source, objective, cycle or policy has changed.".into());
    }
    let native = super::council_inspection_state(store, session_id)?
        .ok_or("council_task_denied: native task inspection is unavailable.")?;
    Ok(CouncilTaskInspection { request: recorded, native })
}
