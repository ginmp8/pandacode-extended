//! Additive native Council reviewer identity and immutable authority contract.
//! Selected-bot and per-delivery bindings are populated only in later scopes.
use super::{council::CouncilSeat, TeamSnapshot, Worker};
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerToolPolicy {
    NativeReadOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerSnapshot {
    pub provider: String,
    pub model: String,
    pub lens: String,
    pub tool_policy: ReviewerToolPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerBinding {
    pub council_cycle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_id: Option<String>,
    pub round: usize,
    pub seat_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_bot_id: Option<String>,
    /// Native revision token, not an evidence or source-content hash.
    pub artifact_revision: String,
    pub snapshot: ReviewerSnapshot,
}

/// Future delivery states, independent of the existing Council score parser.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerState {
    Queued,
    Running,
    Completed,
    Blocked,
    Failed,
    Inconclusive,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerFailureCode {
    ReadOnlyUnsupported,
    ReadOnlyPolicyViolation,
    ReviewerUnavailable,
    ReviewerSessionMismatch,
    ReviewRevisionStale,
}

impl ReviewerBinding {
    pub fn native(
        team: &TeamSnapshot,
        worker_id: &str,
        provider: &str,
        model: &str,
        seat: &CouncilSeat,
    ) -> Self {
        Self {
            council_cycle_id: format!("{}:{}", team.session_id, team.council_since),
            delivery_id: None,
            round: seat.round,
            seat_id: worker_id.to_owned(),
            reviewer_bot_id: None,
            artifact_revision: format!("{}:{}", team.dispatch_revision, team.changed_at),
            snapshot: ReviewerSnapshot {
                provider: provider.to_owned(),
                model: model.to_owned(),
                lens: seat.lens.clone(),
                tool_policy: ReviewerToolPolicy::NativeReadOnly,
            },
        }
    }

    pub fn validate(&self, team: &TeamSnapshot, worker: &Worker) -> Result<()> {
        let seat = worker.council.as_ref().ok_or("reviewer_session_mismatch: not a Council seat.")?;
        if self.council_cycle_id != format!("{}:{}", team.session_id, team.council_since)
            || self.seat_id != worker.id
            || self.round != seat.round
            || self.snapshot.lens != seat.lens
            || self.snapshot.provider != worker.provider
            || self.snapshot.model != worker.model
            || self.snapshot.tool_policy != ReviewerToolPolicy::NativeReadOnly
        {
            return Err("reviewer_session_mismatch: immutable reviewer binding differs from scheduled worker.".into());
        }
        Ok(())
    }
}
