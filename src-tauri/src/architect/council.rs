//! Council: independent read-only reviewers score the team's work out of 10, and `finish_team`
//! refuses until every reviewer of a current round reaches the user's target (docs/architect-workers.md).
//! A round is the seats sharing a number among this cycle's reviewers; rounds are derived from the
//! workers on every check, so nothing can drift out of step with them.
use super::{unsettled, TeamSnapshot, Worker};
use crate::{storage, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const MAX_ROUNDS: usize = 100;
const MAX_REVIEWERS: usize = 8;
const MIN_REVIEWERS: usize = 2;
const LENS_CHARS: usize = 80;
const COMPOSITION_SCHEMA_VERSION: u8 = 1;
const MAX_COMPOSITION_CONCURRENT_LEAVES: usize = 64;
const MAX_COMPOSITION_LEAF_REVIEWS_PER_ROUND: usize = 128;
/// Native admission bound from the other #147 implementation (PR #149).
const MAX_COMPOSED_ROOT_REVIEW_ATTEMPTS: usize = 1024;
const MAX_COMPOSITION_TIMEOUT_MS: u64 = 86_400_000;
const MAX_COMPOSITION_CRITERIA: usize = 64;
const MAX_CRITERION_DESCRIPTION_CHARS: usize = 4096;
const MAX_EVIDENCE_REF_CHARS: usize = 512;
const MAX_CONTRACT_ID_CHARS: usize = 256;
const MAX_RESULT_REFS: usize = 128;
const REVIEW_RECEIPT_SCHEMA_VERSION: u8 = 1;
/// Characters read from a report's end for the score line; `summary` is only the report's head.
const SCORE_TAIL_CHARS: i64 = 8192;
/// The whole report again, so read_worker and the summary carry it with its score.
pub(super) const SCORE_PROMPT: &str = "Your final report did not end with a score. Repeat your final report in full, ending with the exact line `Council score: N/10`, where N is your score for the work you just reviewed.";
pub(super) const SCORE_ERROR: &str = "The reviewer ended two runs without a `Council score: N/10` line. Cancel it and dispatch a fresh reviewer with councilLens.";
pub(super) const NO_FOLLOWUP: &str = "Dispatch a fresh reviewer for the next round";
/// Below the target, the user decides whether a blocker must be fixed.
const ESCAPE: &str = "If a remaining blocker should not be fixed, end your turn and tell the user, who can lower the target or turn Council off; if they say to accept the current scores, tell them to press Accept and finish on the Council card.";
pub(super) const NUDGE_PENDING: &str = "This reviewer has reported and its score is on the way. Wait for it to settle.";
pub(super) const RUNNING_SEAT: &str = "This reviewer is running. Wait for its report; PandaCode fails it if it goes silent.";
pub(super) const SCORED_STANDS: &str = "A scored seat stands. Fix the work and convene a new round.";
/// For the failed-worker turn and the retry cap, when the worker is a reviewer.
/// Explicit local user grant to create at most one GitHub issue for a
/// Council-approved channel delivery. The action is fixed to issue creation;
/// no skill, model prompt or ambient bot capability can broaden the repo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostAcceptanceIssue {
    pub authorization_id: String,
    pub repository: String,
}
impl PostAcceptanceIssue {
    fn validate(&self) -> Result<()> {
        if self.authorization_id.is_empty() || self.authorization_id.len() > 128
            || self.authorization_id.chars().any(char::is_control)
        { return Err("A user-authorized GitHub follow-up is required.".into()); }
        let Some((owner, repo)) = self.repository.split_once('/') else {
            return Err("Choose a GitHub repository in owner/name format.".into());
        };
        let valid = |part: &str| !part.is_empty() && part.len() <= 100
            && part != "." && part != ".."
            && part.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if !valid(owner) || !valid(repo) || self.repository.matches('/').count() != 1 {
            return Err("Choose a GitHub repository in owner/name format.".into());
        }
        Ok(())
    }
}
/// Enforce a finite count of actual root reviewer admissions across rounds,
/// including stopped, failed and abandoned attempts. Legacy M1/M2 are unchanged.
/// The root dispatcher invokes this before binding the new Council seat.
pub(super) fn require_root_attempt_capacity(team: &TeamSnapshot) -> Result<()> {
    if !team.config.council.as_ref().is_some_and(|config| config.composition.is_some()) {
        return Ok(());
    }
    let admitted = team.workers.iter()
        .filter(|worker| worker.council.is_some() && worker.created_at >= team.council_since)
        .count();
    if admitted >= MAX_COMPOSED_ROOT_REVIEW_ATTEMPTS {
        return Err(format!(
            "Composed Council reached {MAX_COMPOSED_ROOT_REVIEW_ATTEMPTS} admitted root reviewer attempts; no further reviewers can be dispatched."
        ));
    }
    Ok(())
}

/// Bound real root and child reviewer admissions together for one composed
/// root cycle. Failed, abandoned and replaced attempts never refund capacity.
pub(super) fn require_composed_attempt_budget(
    team: &TeamSnapshot, child_attempts: usize, additional_attempts: usize,
) -> Result<()> {
    if !team.config.council.as_ref().is_some_and(|config| config.composition.is_some()) {
        return Ok(());
    }
    let root_attempts = team.workers.iter()
        .filter(|worker| worker.council.is_some() && worker.created_at >= team.council_since)
        .count();
    if root_attempts.saturating_add(child_attempts).saturating_add(additional_attempts)
        > MAX_COMPOSED_ROOT_REVIEW_ATTEMPTS
    {
        return Err(format!(
            "council_attempt_budget_exhausted: a composed Council permits at most              {MAX_COMPOSED_ROOT_REVIEW_ATTEMPTS} root and child reviewer admissions per root cycle."
        ));
    }
    Ok(())
}

pub(super) const FAILED_REVIEWER: &str = " A failed Council reviewer is never followed up: cancel it (it has no score) and dispatch a fresh reviewer with councilLens.";

/// The number of root automatic reviewers is either explicit or delegated to the root Architect.
/// In JSON, the latter is exactly the string `"auto"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutomaticReviewerCount {
    Fixed(usize),
    Auto(AutomaticReviewerCountAuto),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomaticReviewerCountAuto {
    Auto,
}

impl AutomaticReviewerCount {
    fn fixed(&self) -> Option<usize> {
        match self {
            Self::Fixed(count) => Some(*count),
            Self::Auto(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilAutomaticReviewers {
    pub enabled: bool,
    pub count: AutomaticReviewerCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectedReviewerMode {
    Direct,
    Nested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestedCouncilTemplate {
    ExplicitTask,
    ValidatedDefault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilCriteriaSource {
    Architect,
    User,
    Hybrid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilCriterionOrigin {
    User,
    Architect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilStrategy {
    Parallel,
    Escalated,
    TwoPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestedEscalation {
    Always,
    OnTrigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestedEscalationPhase {
    Preliminary,
    Child,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestedEscalationTrigger {
    BelowTarget,
    CriticalFinding,
    HighFinding,
    MissingMandatoryEvidence,
    SameCriterionConflict,
    UserForced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct S2EscalationSignal {
    pub seat_id: String,
    pub round: usize,
    pub attempt: usize,
    pub artifact_revision: String,
    pub trigger: NestedEscalationTrigger,
}

/// A native decision is pinned to the exact seat attempt and artifact so a
/// restart cannot reinterpret a completed preliminary review.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NestedEscalationDecision {
    pub cycle_id: String,
    pub seat_id: String,
    pub round: usize,
    pub attempt: usize,
    pub artifact_revision: String,
    pub phase: NestedEscalationPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<NestedEscalationTrigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preliminary_score: Option<f64>,
    #[serde(default)]
    pub finalized: bool,
    pub decided_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_session_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NestedCouncilConfig {
    pub template: NestedCouncilTemplate,
    pub child_criteria_source: CouncilCriteriaSource,
    pub child_reviewer_count: usize,
    pub target: f64,
    pub max_rounds: usize,
    pub escalation: NestedEscalation,
    #[serde(default)]
    pub user_forced: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedCouncilReviewer {
    pub bot_id: String,
    pub mode: SelectedReviewerMode,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested: Option<NestedCouncilConfig>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilCriterion {
    pub id: String,
    pub lens: String,
    pub description: String,
    pub required: bool,
    pub target: f64,
    pub evidence_required: bool,
    pub source: CouncilCriterionOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilCompositionLimits {
    /// Council hierarchy is always one root plus at most one child Architect.
    pub max_depth: u8,
    /// The composition contract fixes this ceiling at the native eight root seats.
    pub max_root_seats: u8,
    /// 64 is the finite maximum implied by 8 root seats × 8 child reviewers.
    pub max_concurrent_leaves: usize,
    /// 128 admits a second two-phase challenge pass over the maximum 64 child leaves.
    pub max_leaf_reviews_per_round: usize,
    /// A composition may run for at most 24 hours before it must settle as non-success.
    pub timeout_ms: u64,
    /// Optional positive caller-supplied estimate ceiling. `None` is uncapped;
    /// `Some(0)` is invalid and an unknown estimate never satisfies a cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_estimated_cost_micros: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilComposition {
    pub schema_version: u8,
    pub automatic: CouncilAutomaticReviewers,
    pub selected: Vec<SelectedCouncilReviewer>,
    pub criteria_source: CouncilCriteriaSource,
    pub criteria: Vec<CouncilCriterion>,
    pub strategy: CouncilStrategy,
    pub limits: CouncilCompositionLimits,
}

/// The seven reviewer-source combinations. Strategy and criteria ownership are independent axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilCompositionMode {
    Architect,
    Bots,
    BotsAndArchitect,
    BotsWithOwnArchitect,
    BotsWithOwnArchitectAndArchitect,
    MixedBots,
    MixedBotsAndArchitect,
}

impl CouncilCompositionMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Architect => "M1",
            Self::Bots => "M2",
            Self::BotsAndArchitect => "M3",
            Self::BotsWithOwnArchitect => "M4",
            Self::BotsWithOwnArchitectAndArchitect => "M5",
            Self::MixedBots => "M6",
            Self::MixedBotsAndArchitect => "M7",
        }
    }
}

/// Required root seats by source. Keeping this typed prevents selected bot roster size from
/// accidentally standing in for the whole composition quota.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CouncilSeatPlan {
    pub automatic: usize,
    pub direct_bots: usize,
    pub nested_bots: usize,
}

impl CouncilSeatPlan {
    pub(super) fn selected_bots(self) -> usize {
        self.direct_bots.saturating_add(self.nested_bots)
    }

    pub(super) fn required(self) -> usize {
        self.automatic.saturating_add(self.selected_bots())
    }
}

/// Concrete capabilities are supplied by the native roster/host layer before dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CouncilReviewerCapability {
    pub bot_id: String,
    pub direct: bool,
    pub nested: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CouncilCompositionCapabilities {
    pub automatic_reviewers: bool,
    pub supported_strategies: Vec<CouncilStrategy>,
    pub reviewers: Vec<CouncilReviewerCapability>,
}

/// This outcome is written only by native review/receipt handling. The references are evidence
/// receipts verified against the frozen artifact before they are stored on a seat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilCriterionOutcome {
    pub criterion_id: String,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_evidence_refs: Vec<String>,
}

/// A persisted criterion result bound to one issued reviewer attempt and the
/// exact artifact it inspected. Nested leaf results retain their single parent
/// seat so they cannot inflate the root quorum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilCriterionOutcomeReceipt {
    pub schema_version: u8,
    pub root_cycle_id: String,
    pub task_id: String,
    pub task_cycle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_cycle_id: Option<String>,
    pub round: usize,
    pub seat_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_seat_id: Option<String>,
    pub attempt: usize,
    pub artifact_revision: String,
    pub outcome: CouncilCriterionOutcome,
}

impl CouncilCriterionOutcomeReceipt {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        if self.schema_version != 1
            || !contract_token(&self.root_cycle_id, MAX_CONTRACT_ID_CHARS)
            || !contract_token(&self.task_id, MAX_CONTRACT_ID_CHARS)
            || !contract_token(&self.task_cycle_id, MAX_CONTRACT_ID_CHARS)
            || !contract_token(&self.seat_id, MAX_CONTRACT_ID_CHARS)
            || self.parent_seat_id.as_ref().is_some_and(|id| !contract_token(id, MAX_CONTRACT_ID_CHARS))
            || self.round == 0
            || self.attempt == 0
            || self.artifact_revision.is_empty()
            || self.artifact_revision.len() > MAX_CONTRACT_ID_CHARS
            || self.artifact_revision.chars().any(char::is_control)
            || (self.child_cycle_id.is_some() != self.parent_seat_id.is_some())
            || self.child_cycle_id.as_ref().is_some_and(|id| id != &self.task_cycle_id)
            || !super::valid_id(&self.outcome.criterion_id)
            || !self.outcome.score.is_finite()
            || !(0.0..=10.0).contains(&self.outcome.score)
            || self.outcome.verified_evidence_refs.len() > MAX_RESULT_REFS
            || !unique_refs(&self.outcome.verified_evidence_refs, MAX_RESULT_REFS)
            || self.outcome.verified_evidence_refs.iter().any(|reference| {
                reference.len() > MAX_EVIDENCE_REF_CHARS || reference.chars().any(char::is_control)
            })
        {
            return Err(CouncilCompositionError::InvalidCriterionOutcome(
                "receipt correlation, score, or evidence is invalid".into(),
            ));
        }
        Ok(())
    }

    pub fn validates_for(
        &self,
        root_cycle_id: &str,
        task_id: &str,
        task_cycle_id: &str,
        round: usize,
        seat_id: &str,
        parent_seat_id: Option<&str>,
        attempt: usize,
        artifact_revision: &str,
    ) -> bool {
        self.validate().is_ok()
            && self.root_cycle_id == root_cycle_id
            && self.task_id == task_id
            && self.task_cycle_id == task_cycle_id
            && self.round == round
            && self.seat_id == seat_id
            && self.parent_seat_id.as_deref() == parent_seat_id
            && self.attempt == attempt
            && self.artifact_revision == artifact_revision
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilFindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilFindingLocationStatus {
    Verified,
    Missing,
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilFindingLocation {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// A bounded, attributable review finding. `suggestedFix` is untrusted display text, never authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilFinding {
    pub id: String,
    /// Digest of this immutable finding's content and native correlation. Empty only on legacy rows.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content_digest: String,
    pub cycle_id: String,
    pub round_id: String,
    pub seat_id: String,
    pub artifact_digest: String,
    pub criterion_id: String,
    pub severity: CouncilFindingSeverity,
    pub category: String,
    pub summary: String,
    /// Optional structured details; old persisted records omit both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    pub evidence_refs: Vec<String>,
    pub location_status: CouncilFindingLocationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<CouncilFindingLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilFindingDispositionKind {
    Open,
    Addressed,
    Disputed,
    NotReproduced,
    WaivedByAuthorizedHuman,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilFindingDisposition {
    pub finding_id: String,
    /// Exact native review cycle to which this disposition applies.
    #[serde(default)]
    pub cycle_id: String,
    /// Digest of the reviewed artifact; stale dispositions cannot close a finding.
    #[serde(default)]
    pub artifact_digest: String,
    pub disposition: CouncilFindingDispositionKind,
    pub actor_id: String,
    pub rationale: String,
    pub timestamp_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_links: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildCouncilStatus {
    Passed,
    Accepted,
    Fixing,
    Limit,
    Stopped,
    Failed,
    Blocked,
    Inconclusive,
    Stale,
}

/// The single parent-facing result for a child Council; leaf references never create extra root seats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChildCouncilReceipt {
    pub root_cycle_id: String,
    pub parent_seat_id: String,
    pub child_cycle_id: String,
    pub bot_id: String,
    pub artifact_revision: String,
    pub policy_fingerprint: String,
    pub status: ChildCouncilStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leaf_score_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    pub snapshot_fingerprint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilReviewVerdict {
    Passed,
    Accepted,
    Failed,
    Limit,
    Blocked,
    Stopped,
    Inconclusive,
    Stale,
}

/// One correlated terminal result for an internal native Council request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilReviewReceipt {
    pub schema_version: u8,
    pub request_id: String,
    pub cycle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    pub policy_fingerprint: String,
    pub artifact_digest: String,
    pub verdict: CouncilReviewVerdict,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    pub terminal_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilUsageValueStatus {
    Measured,
    ProviderEstimated,
    CalculatedEstimated,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilUsageValue<T> {
    pub status: CouncilUsageValueStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilCostEstimate {
    pub amount_micros: u64,
    pub currency: String,
    pub pricing_source: String,
    pub pricing_version: String,
    pub calculated_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilResourceBudgetConsumption {
    pub concurrent_leaves: CouncilUsageValue<u64>,
    pub leaf_reviews: CouncilUsageValue<u64>,
    pub estimated_cost_micros: CouncilUsageValue<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilUsageScope {
    Root,
    Child,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouncilUsagePhase {
    Review,
    Escalation,
    Challenge,
}

/// Per-attempt usage stays attributable; unknown values remain explicit and are never fabricated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CouncilUsageRecord {
    pub id: String,
    pub root_cycle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_cycle_id: Option<String>,
    pub attempt_id: String,
    pub reviewer_id: String,
    pub round_id: String,
    pub seat_id: String,
    pub scope: CouncilUsageScope,
    pub phase: CouncilUsagePhase,
    pub strategy: CouncilStrategy,
    /// Stable native trigger/challenge receipt IDs that justify a non-review phase.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<u64>,
    pub queue_wait_ms: CouncilUsageValue<u64>,
    pub elapsed_ms: CouncilUsageValue<u64>,
    pub retries: CouncilUsageValue<u32>,
    pub escalations: CouncilUsageValue<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub input_tokens: CouncilUsageValue<u64>,
    pub output_tokens: CouncilUsageValue<u64>,
    pub request_count: CouncilUsageValue<u64>,
    pub terminal_status: CouncilReviewVerdict,
    pub budget_consumption: CouncilResourceBudgetConsumption,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_estimate: Option<CouncilCostEstimate>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CouncilCompositionError {
    InvalidSchemaVersion(u8),
    InvalidCouncilConfig(String),
    MissingReviewerSource,
    InvalidAutomaticCount,
    InvalidRosterId(String),
    DuplicateReviewerId(String),
    SelfReview(String),
    ReviewerMustBeRequired(String),
    MissingNestedConfig(String),
    UnexpectedNestedConfig(String),
    InvalidNestedConfig { bot_id: String, reason: &'static str },
    RootQuorum,
    RootSeatLimit { actual: usize, maximum: usize },
    MirrorMismatch(&'static str),
    InvalidLimit(&'static str),
    InvalidCriteria(&'static str),
    InvalidCriterionId(String),
    DuplicateCriterionId(String),
    DuplicateCriterionLens(String),
    InvalidCriterionOutcome(String),
    UnknownCriterionOutcome(String),
    DuplicateCriterionOutcome(String),
    MissingRequiredCriterion(String),
    MissingCriterionEvidence(String),
    CriterionBelowTarget { id: String, score: f64, target: f64 },
    UnsupportedCapability { bot_id: Option<String>, capability: &'static str },
    InvalidFinding(&'static str),
    InvalidDisposition(&'static str),
    InvalidChildReceipt(&'static str),
    InvalidReviewReceipt(&'static str),
    InvalidUsageRecord(&'static str),
    CorrelationMismatch(&'static str),
}

impl std::fmt::Display for CouncilCompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSchemaVersion(version) => write!(f, "Unsupported Council composition schema version {version}; expected {COMPOSITION_SCHEMA_VERSION}."),
            Self::InvalidCouncilConfig(reason) => write!(f, "Council configuration is invalid: {reason}"),
            Self::MissingReviewerSource => write!(f, "Council composition must enable automatic reviewers or select at least one reviewer bot."),
            Self::InvalidAutomaticCount => write!(f, "Council automatic reviewer count must be `auto` or a bounded positive count when enabled."),
            Self::InvalidRosterId(id) => write!(f, "Council reviewer bot ID {id:?} is invalid."),
            Self::DuplicateReviewerId(id) => write!(f, "Council reviewer bot ID {id:?} is selected more than once."),
            Self::SelfReview(id) => write!(f, "The task executor {id:?} cannot review its own work."),
            Self::ReviewerMustBeRequired(id) => write!(f, "Selected Council reviewer {id:?} must remain required."),
            Self::MissingNestedConfig(id) => write!(f, "Nested Council reviewer {id:?} needs a complete child configuration."),
            Self::UnexpectedNestedConfig(id) => write!(f, "Direct Council reviewer {id:?} cannot carry nested child settings."),
            Self::InvalidNestedConfig { bot_id, reason } => write!(f, "Nested Council reviewer {bot_id:?} has invalid {reason}."),
            Self::RootQuorum => write!(f, "Council composition requires at least two root seats."),
            Self::RootSeatLimit { actual, maximum } => write!(f, "Council composition reserves {actual} root seats, above the limit of {maximum}."),
            Self::MirrorMismatch(field) => write!(f, "Council composition does not match its legacy {field} mirror."),
            Self::InvalidLimit(limit) => write!(f, "Council composition limit {limit} is unsupported or out of range."),
            Self::InvalidCriteria(reason) => write!(f, "Council composition criteria are invalid: {reason}."),
            Self::InvalidCriterionId(id) => write!(f, "Council criterion ID {id:?} is invalid."),
            Self::DuplicateCriterionId(id) => write!(f, "Council criterion ID {id:?} is duplicated."),
            Self::DuplicateCriterionLens(lens) => write!(f, "Council criterion lens {lens:?} is duplicated."),
            Self::InvalidCriterionOutcome(reason) => write!(f, "Council criterion outcome is invalid: {reason}."),
            Self::UnknownCriterionOutcome(id) => write!(f, "Council criterion outcome {id:?} is not in the frozen criteria."),
            Self::DuplicateCriterionOutcome(id) => write!(f, "Council seat reports criterion {id:?} more than once."),
            Self::MissingRequiredCriterion(id) => write!(f, "Required Council criterion {id:?} has no scored reviewer coverage."),
            Self::MissingCriterionEvidence(id) => write!(f, "Required Council criterion {id:?} has no verified evidence."),
            Self::CriterionBelowTarget { id, score, target } => write!(f, "Required Council criterion {id:?} scored {score}, below its effective target {target}."),
            Self::UnsupportedCapability { bot_id: Some(id), capability } => write!(f, "Council reviewer {id:?} does not support required capability {capability}."),
            Self::UnsupportedCapability { bot_id: None, capability } => write!(f, "This Council composition does not support required capability {capability}."),
            Self::InvalidFinding(reason) => write!(f, "Council finding is invalid: {reason}."),
            Self::InvalidDisposition(reason) => write!(f, "Council finding disposition is invalid: {reason}."),
            Self::InvalidChildReceipt(reason) => write!(f, "Child Council receipt is invalid: {reason}."),
            Self::InvalidReviewReceipt(reason) => write!(f, "Council review receipt is invalid: {reason}."),
            Self::InvalidUsageRecord(reason) => write!(f, "Council usage record is invalid: {reason}."),
            Self::CorrelationMismatch(field) => write!(f, "Council receipt does not match its frozen {field}."),
        }
    }
}
impl std::error::Error for CouncilCompositionError {}

fn contract_token(value: &str, max_chars: usize) -> bool {
    !value.is_empty() && value.chars().count() <= max_chars && !value.chars().any(char::is_control)
}
fn safe_contract_text(value: &str, max_chars: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max_chars
        && !value.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
fn sha256_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn unique_refs(refs: &[String], maximum: usize) -> bool {
    if refs.len() > maximum { return false; }
    let mut unique = std::collections::HashSet::new();
    refs.iter().all(|reference| contract_token(reference, MAX_EVIDENCE_REF_CHARS) && unique.insert(reference.as_str()))
}
fn safe_relative_file(path: &str) -> bool {
    if !contract_token(path, 1024) || path.starts_with('/') || path.starts_with('\\') || path.contains(':') { return false; }
    !path.replace('\\', "/").split('/').any(|part| part == ".." || part.is_empty())
}
fn valid_usage_value<T>(value: &CouncilUsageValue<T>) -> bool {
    match (value.status, value.value.is_some()) {
        (CouncilUsageValueStatus::Unknown, false) => true,
        (CouncilUsageValueStatus::Measured | CouncilUsageValueStatus::ProviderEstimated | CouncilUsageValueStatus::CalculatedEstimated, true) => true,
        _ => false,
    }
}

impl CouncilFinding {
    pub fn calculated_content_digest(&self) -> String {
        let canonical = serde_json::to_vec(&(
            &self.cycle_id,
            &self.round_id,
            &self.seat_id,
            &self.artifact_digest,
            &self.criterion_id,
            self.severity,
            &self.category,
            &self.summary,
            &self.evidence_refs,
            self.location_status,
            &self.location,
            &self.suggested_fix,
        )).unwrap_or_default();
        // Preserve existing finding digests byte-for-byte for historic records,
        // while binding newly supplied title/explanation to the immutable ID.
        let mut canonical = canonical;
        if self.title.is_some() || self.explanation.is_some() {
            canonical.extend(serde_json::to_vec(&(&self.title, &self.explanation))
                .unwrap_or_default());
        }
        format!("{:x}", Sha256::digest(canonical))
    }

    pub fn has_content_identity(&self) -> bool {
        !self.content_digest.is_empty()
            && self.content_digest == self.calculated_content_digest()
            && self.id == format!("finding-{}", self.content_digest)
    }

    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        let identifiers = [&self.id, &self.cycle_id, &self.round_id, &self.seat_id];
        if identifiers.iter().any(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || !super::valid_id(&self.criterion_id)
        { return Err(CouncilCompositionError::InvalidFinding("IDs must be non-empty bounded tokens")); }
        if !sha256_digest(&self.artifact_digest) {
            return Err(CouncilCompositionError::InvalidFinding("artifactDigest must be a SHA-256 hex digest"));
        }
        if !self.content_digest.is_empty() && !self.has_content_identity() {
            return Err(CouncilCompositionError::InvalidFinding("contentDigest and ID must match the immutable finding content"));
        }
        if !safe_contract_text(&self.category, 128) || !safe_contract_text(&self.summary, MAX_CRITERION_DESCRIPTION_CHARS) {
            return Err(CouncilCompositionError::InvalidFinding("category or summary is empty, unsafe, or too long"));
        }
        if self.title.as_ref().is_some_and(|title| !safe_contract_text(title, 128))
            || self.explanation.as_ref().is_some_and(|explanation|
                !safe_contract_text(explanation, MAX_CRITERION_DESCRIPTION_CHARS))
        {
            return Err(CouncilCompositionError::InvalidFinding("finding title or explanation is empty, unsafe, or too long"));
        }
        if self.evidence_refs.is_empty() || !unique_refs(&self.evidence_refs, MAX_RESULT_REFS) {
            return Err(CouncilCompositionError::InvalidFinding("evidenceRefs must contain at least one unique bounded verified reference"));
        }
        match (self.location_status, self.location.as_ref()) {
            (CouncilFindingLocationStatus::Verified, Some(location))
                if safe_relative_file(&location.file) && location.line != Some(0) => {}
            (CouncilFindingLocationStatus::Missing | CouncilFindingLocationStatus::Unverified, None) => {}
            _ => return Err(CouncilCompositionError::InvalidFinding("location status must explicitly match a safe verified location or an absent location")),
        }
        if self.suggested_fix.as_ref().is_some_and(|fix| !safe_contract_text(fix, MAX_CRITERION_DESCRIPTION_CHARS)) {
            return Err(CouncilCompositionError::InvalidFinding("suggestedFix is empty, unsafe, or too long"));
        }
        Ok(())
    }
}

impl CouncilFindingDisposition {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        if !contract_token(&self.finding_id, MAX_CONTRACT_ID_CHARS)
            || !contract_token(&self.cycle_id, MAX_CONTRACT_ID_CHARS)
            || !sha256_digest(&self.artifact_digest)
            || !contract_token(&self.actor_id, MAX_CONTRACT_ID_CHARS)
            || !safe_contract_text(&self.rationale, MAX_CRITERION_DESCRIPTION_CHARS)
            || self.timestamp_ms == 0
            || !unique_refs(&self.artifact_links, MAX_RESULT_REFS)
        { return Err(CouncilCompositionError::InvalidDisposition("finding, actor, rationale, timestamp, or artifact links")); }
        Ok(())
    }
}

/// Structured critical findings can be closed only by native verification or a
/// separate user action. Model-written dispositions never grant approval.
pub(super) fn unresolved_critical_findings(
    findings: &[CouncilFinding],
    dispositions: &[CouncilFindingDisposition],
    cycle_id: &str,
) -> Vec<String> {
    let mut unresolved = findings.iter().filter(|finding| {
        finding.validate().is_ok()
            && finding.cycle_id == cycle_id
            && finding.severity == CouncilFindingSeverity::Critical
    }).filter(|finding| {
        !dispositions.iter().any(|disposition| {
            disposition.validate().is_ok()
                && disposition.finding_id == finding.id
                && disposition.cycle_id == finding.cycle_id
                && disposition.artifact_digest == finding.artifact_digest
                && match disposition.disposition {
                    CouncilFindingDispositionKind::Addressed => disposition.actor_id == "native:verification",
                    CouncilFindingDispositionKind::WaivedByAuthorizedHuman => disposition.actor_id == "user",
                    _ => false,
                }
        })
    }).map(|finding| finding.id.clone()).collect::<Vec<_>>();
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

impl ChildCouncilReceipt {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        if [&self.root_cycle_id, &self.parent_seat_id, &self.child_cycle_id, &self.artifact_revision,
            &self.policy_fingerprint, &self.snapshot_fingerprint]
            .iter().any(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || !super::valid_id(&self.bot_id)
        { return Err(CouncilCompositionError::InvalidChildReceipt("correlation IDs, bot ID, and fingerprints are required")); }
        if self.min_score.is_some_and(|score| !score.is_finite() || !(0.0..=10.0).contains(&score)) {
            return Err(CouncilCompositionError::InvalidChildReceipt("minScore must be finite and between 0 and 10"));
        }
        if !unique_refs(&self.leaf_score_refs, MAX_REVIEWERS)
            || !unique_refs(&self.findings_refs, MAX_RESULT_REFS)
            || !unique_refs(&self.evidence_refs, MAX_RESULT_REFS)
        { return Err(CouncilCompositionError::InvalidChildReceipt("score, finding, and evidence references must be unique and bounded")); }
        if self.status == ChildCouncilStatus::Passed
            && (self.min_score.is_none() || !(MIN_REVIEWERS..=MAX_REVIEWERS).contains(&self.leaf_score_refs.len()))
        { return Err(CouncilCompositionError::InvalidChildReceipt("passed child receipts need a composite score and 2..=8 leaf scores")); }
        Ok(())
    }

    pub fn validate_for(
        &self,
        root_cycle_id: &str,
        parent_seat_id: &str,
        bot_id: &str,
        artifact_revision: &str,
        policy_fingerprint: &str,
    ) -> std::result::Result<(), CouncilCompositionError> {
        self.validate()?;
        if self.root_cycle_id != root_cycle_id { return Err(CouncilCompositionError::CorrelationMismatch("rootCycleId")); }
        if self.parent_seat_id != parent_seat_id { return Err(CouncilCompositionError::CorrelationMismatch("parentSeatId")); }
        if self.bot_id != bot_id { return Err(CouncilCompositionError::CorrelationMismatch("botId")); }
        if self.artifact_revision != artifact_revision { return Err(CouncilCompositionError::CorrelationMismatch("artifactRevision")); }
        if self.policy_fingerprint != policy_fingerprint { return Err(CouncilCompositionError::CorrelationMismatch("policyFingerprint")); }
        Ok(())
    }
}

impl CouncilReviewReceipt {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        if self.schema_version != REVIEW_RECEIPT_SCHEMA_VERSION {
            return Err(CouncilCompositionError::InvalidReviewReceipt("unsupported schemaVersion"));
        }
        if [&self.request_id, &self.cycle_id, &self.policy_fingerprint, &self.idempotency_key]
            .iter().any(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || self.task_id.as_ref().is_some_and(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || self.assignment_id.as_ref().is_some_and(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
        { return Err(CouncilCompositionError::InvalidReviewReceipt("request, cycle, policy, or idempotency identifiers")); }
        if !sha256_digest(&self.artifact_digest) {
            return Err(CouncilCompositionError::InvalidReviewReceipt("artifactDigest must be a SHA-256 hex digest"));
        }
        if self.terminal_revision == 0 || !unique_refs(&self.findings_refs, MAX_RESULT_REFS) || !unique_refs(&self.evidence_refs, MAX_RESULT_REFS) {
            return Err(CouncilCompositionError::InvalidReviewReceipt("terminalRevision or result references"));
        }
        Ok(())
    }

    pub fn validate_for(&self, request_id: &str, cycle_id: &str, artifact_digest: &str, idempotency_key: &str) -> std::result::Result<(), CouncilCompositionError> {
        self.validate()?;
        if self.request_id != request_id { return Err(CouncilCompositionError::CorrelationMismatch("requestId")); }
        if self.cycle_id != cycle_id { return Err(CouncilCompositionError::CorrelationMismatch("cycleId")); }
        if self.artifact_digest != artifact_digest { return Err(CouncilCompositionError::CorrelationMismatch("artifactDigest")); }
        if self.idempotency_key != idempotency_key { return Err(CouncilCompositionError::CorrelationMismatch("idempotencyKey")); }
        Ok(())
    }
}

impl CouncilUsageRecord {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        let required = [&self.id, &self.root_cycle_id, &self.attempt_id, &self.reviewer_id, &self.round_id, &self.seat_id];
        if required.iter().any(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || self.child_cycle_id.as_ref().is_some_and(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
            || self.decision_ids.len() > MAX_RESULT_REFS
            || !unique_refs(&self.decision_ids, MAX_RESULT_REFS)
            || self.decision_ids.iter().any(|value| !contract_token(value, MAX_CONTRACT_ID_CHARS))
        { return Err(CouncilCompositionError::InvalidUsageRecord("correlation IDs")); }
        let phase_decisions_valid = match self.phase {
            CouncilUsagePhase::Review => self.decision_ids.is_empty(),
            CouncilUsagePhase::Escalation => self.strategy == CouncilStrategy::Escalated && !self.decision_ids.is_empty(),
            CouncilUsagePhase::Challenge => self.strategy == CouncilStrategy::TwoPhase && !self.decision_ids.is_empty(),
        };
        if !phase_decisions_valid {
            return Err(CouncilCompositionError::InvalidUsageRecord("phase must match persisted decision IDs and strategy"));
        }
        let escalation_count_matches = match self.phase {
            CouncilUsagePhase::Escalation => self.escalations.value == Some(self.decision_ids.len() as u32),
            CouncilUsagePhase::Review | CouncilUsagePhase::Challenge => self.escalations.value.is_none_or(|value| value == 0),
        };
        if !escalation_count_matches {
            return Err(CouncilCompositionError::InvalidUsageRecord("escalation count must match persisted trigger decisions"));
        }
        if (self.scope == CouncilUsageScope::Child) != self.child_cycle_id.is_some() {
            return Err(CouncilCompositionError::InvalidUsageRecord("child scope and childCycleId must agree"));
        }
        if self.started_at_ms.zip(self.finished_at_ms).is_some_and(|(start, finish)| finish < start) {
            return Err(CouncilCompositionError::InvalidUsageRecord("finishedAtMs precedes startedAtMs"));
        }
        if !valid_usage_value(&self.queue_wait_ms) || !valid_usage_value(&self.elapsed_ms)
            || !valid_usage_value(&self.retries) || !valid_usage_value(&self.escalations)
            || !valid_usage_value(&self.input_tokens) || !valid_usage_value(&self.output_tokens)
            || !valid_usage_value(&self.request_count)
            || !valid_usage_value(&self.budget_consumption.concurrent_leaves)
            || !valid_usage_value(&self.budget_consumption.leaf_reviews)
            || !valid_usage_value(&self.budget_consumption.estimated_cost_micros)
        { return Err(CouncilCompositionError::InvalidUsageRecord("measurement state and value disagree")); }
        for value in [&self.provider, &self.model, &self.harness].into_iter().flatten() {
            if !safe_contract_text(value, 256) { return Err(CouncilCompositionError::InvalidUsageRecord("provider, model, or harness is empty or too long")); }
        }
        if let Some(cost) = &self.cost_estimate {
            if cost.currency.len() != 3 || !cost.currency.bytes().all(|b| b.is_ascii_uppercase())
                || !safe_contract_text(&cost.pricing_source, 256) || !safe_contract_text(&cost.pricing_version, 128)
                || cost.calculated_at_ms == 0
            { return Err(CouncilCompositionError::InvalidUsageRecord("cost estimate needs a currency, pricing provenance, and timestamp")); }
        }
        Ok(())
    }
}

impl CouncilComposition {
    pub fn validate(&self) -> std::result::Result<(), CouncilCompositionError> {
        if self.schema_version != COMPOSITION_SCHEMA_VERSION {
            return Err(CouncilCompositionError::InvalidSchemaVersion(self.schema_version));
        }
        if self.limits.max_depth != 2 {
            return Err(CouncilCompositionError::InvalidLimit("maxDepth must be 2"));
        }
        if self.limits.max_root_seats != MAX_REVIEWERS as u8 {
            return Err(CouncilCompositionError::InvalidLimit("maxRootSeats must be 8"));
        }
        if !(1..=MAX_COMPOSITION_CONCURRENT_LEAVES).contains(&self.limits.max_concurrent_leaves) {
            return Err(CouncilCompositionError::InvalidLimit("maxConcurrentLeaves"));
        }
        if !(1..=MAX_COMPOSITION_LEAF_REVIEWS_PER_ROUND).contains(&self.limits.max_leaf_reviews_per_round) {
            return Err(CouncilCompositionError::InvalidLimit("maxLeafReviewsPerRound"));
        }
        if !(1..=MAX_COMPOSITION_TIMEOUT_MS).contains(&self.limits.timeout_ms) {
            return Err(CouncilCompositionError::InvalidLimit("timeoutMs"));
        }
        if self.limits.max_estimated_cost_micros == Some(0) {
            return Err(CouncilCompositionError::InvalidLimit("maxEstimatedCostMicros"));
        }

        let mut ids = std::collections::HashSet::new();
        for reviewer in &self.selected {
            if !super::valid_id(&reviewer.bot_id) {
                return Err(CouncilCompositionError::InvalidRosterId(reviewer.bot_id.clone()));
            }
            if !ids.insert(reviewer.bot_id.as_str()) {
                return Err(CouncilCompositionError::DuplicateReviewerId(reviewer.bot_id.clone()));
            }
            if !reviewer.required {
                return Err(CouncilCompositionError::ReviewerMustBeRequired(reviewer.bot_id.clone()));
            }
            match (reviewer.mode, reviewer.nested.as_ref()) {
                (SelectedReviewerMode::Direct, Some(_)) => return Err(CouncilCompositionError::UnexpectedNestedConfig(reviewer.bot_id.clone())),
                (SelectedReviewerMode::Direct, None) => {}
                (SelectedReviewerMode::Nested, None) => return Err(CouncilCompositionError::MissingNestedConfig(reviewer.bot_id.clone())),
                (SelectedReviewerMode::Nested, Some(nested)) => {
                    if !(MIN_REVIEWERS..=MAX_REVIEWERS).contains(&nested.child_reviewer_count) {
                        return Err(CouncilCompositionError::InvalidNestedConfig { bot_id: reviewer.bot_id.clone(), reason: "childReviewerCount (2..=8)" });
                    }
                    if !nested.target.is_finite() || !(1.0..=10.0).contains(&nested.target) {
                        return Err(CouncilCompositionError::InvalidNestedConfig { bot_id: reviewer.bot_id.clone(), reason: "target (1..=10)" });
                    }
                    if !(1..=MAX_ROUNDS).contains(&nested.max_rounds) {
                        return Err(CouncilCompositionError::InvalidNestedConfig { bot_id: reviewer.bot_id.clone(), reason: "maxRounds (1..=100)" });
                    }
                }
            }
        }

        // Mandatory nested Councils must be able to seat their minimum leaf
        // reviewers within a root round. An impossible fixed budget should
        // fail before any root/child tasks are admitted.
        let mandatory_leaves: usize = self.selected.iter()
            .filter_map(|selected| selected.nested.as_ref())
            .filter(|nested| nested.escalation == NestedEscalation::Always)
            .map(|nested| nested.child_reviewer_count)
            .sum();
        let required_reviews = mandatory_leaves
            * if self.strategy == CouncilStrategy::TwoPhase { 2 } else { 1 };
        if required_reviews > self.limits.max_leaf_reviews_per_round {
            return Err(CouncilCompositionError::InvalidLimit(
                "maxLeafReviewsPerRound cannot cover the mandatory nested Council leaf reviews",
            ));
        }
        if self.selected.len() > self.limits.max_root_seats as usize {
            return Err(CouncilCompositionError::RootSeatLimit { actual: self.selected.len(), maximum: self.limits.max_root_seats as usize });
        }
        match self.automatic.count {
            AutomaticReviewerCount::Fixed(count) if count > self.limits.max_root_seats as usize => return Err(CouncilCompositionError::InvalidAutomaticCount),
            AutomaticReviewerCount::Fixed(0) if self.automatic.enabled => return Err(CouncilCompositionError::InvalidAutomaticCount),
            _ => {}
        }
        if self.selected.is_empty() && !self.automatic.enabled {
            return Err(CouncilCompositionError::MissingReviewerSource);
        }
        let minimum_automatic = self.minimum_automatic_seats();
        let root_seats = self.selected.len().saturating_add(minimum_automatic);
        if root_seats < MIN_REVIEWERS {
            return Err(CouncilCompositionError::RootQuorum);
        }
        if root_seats > self.limits.max_root_seats as usize {
            return Err(CouncilCompositionError::RootSeatLimit { actual: root_seats, maximum: self.limits.max_root_seats as usize });
        }

        self.validate_criteria()?;
        Ok(())
    }

    fn validate_criteria(&self) -> std::result::Result<(), CouncilCompositionError> {
        if self.criteria.len() > MAX_COMPOSITION_CRITERIA {
            return Err(CouncilCompositionError::InvalidCriteria("at most 64 criteria are allowed"));
        }
        let mut ids = std::collections::HashSet::new();
        let mut lenses = std::collections::HashSet::new();
        for criterion in &self.criteria {
            if !super::valid_id(&criterion.id) {
                return Err(CouncilCompositionError::InvalidCriterionId(criterion.id.clone()));
            }
            if !ids.insert(criterion.id.as_str()) {
                return Err(CouncilCompositionError::DuplicateCriterionId(criterion.id.clone()));
            }
            let lens = criterion.lens.trim();
            if lens.is_empty() || lens.chars().count() > LENS_CHARS || lens.chars().any(char::is_control) {
                return Err(CouncilCompositionError::InvalidCriteria("criterion lens must be a plain name of 1..=80 characters"));
            }
            if !lenses.insert(lens.to_lowercase()) {
                return Err(CouncilCompositionError::DuplicateCriterionLens(criterion.lens.clone()));
            }
            if criterion.description.trim().is_empty() || criterion.description.chars().count() > MAX_CRITERION_DESCRIPTION_CHARS {
                return Err(CouncilCompositionError::InvalidCriteria("criterion description must be 1..=4096 characters"));
            }
            if !criterion.target.is_finite() || !(1.0..=10.0).contains(&criterion.target) {
                return Err(CouncilCompositionError::InvalidCriteria("criterion target must be between 1 and 10"));
            }
            match (self.criteria_source, criterion.source) {
                (CouncilCriteriaSource::Architect, CouncilCriterionOrigin::Architect)
                | (CouncilCriteriaSource::User, CouncilCriterionOrigin::User)
                | (CouncilCriteriaSource::Hybrid, _) => {}
                _ => return Err(CouncilCompositionError::InvalidCriteria("criterion source does not match criteriaSource")),
            }
        }
        match self.criteria_source {
            CouncilCriteriaSource::Architect => {}
            CouncilCriteriaSource::User if self.criteria.is_empty() || !self.criteria.iter().any(|c| c.required) => {
                return Err(CouncilCompositionError::InvalidCriteria("user criteria need at least one required criterion"));
            }
            CouncilCriteriaSource::Hybrid if !self.criteria.iter().any(|c| c.source == CouncilCriterionOrigin::User && c.required) => {
                return Err(CouncilCompositionError::InvalidCriteria("hybrid criteria need at least one required user criterion"));
            }
            _ => {}
        }
        Ok(())
    }

    fn mode_unchecked(&self) -> Option<CouncilCompositionMode> {
        let automatic = self.automatic.enabled;
        let direct = self.selected.iter().any(|r| r.mode == SelectedReviewerMode::Direct);
        let nested = self.selected.iter().any(|r| r.mode == SelectedReviewerMode::Nested);
        match (automatic, direct, nested) {
            (true, false, false) => Some(CouncilCompositionMode::Architect),
            (false, true, false) => Some(CouncilCompositionMode::Bots),
            (true, true, false) => Some(CouncilCompositionMode::BotsAndArchitect),
            (false, false, true) => Some(CouncilCompositionMode::BotsWithOwnArchitect),
            (true, false, true) => Some(CouncilCompositionMode::BotsWithOwnArchitectAndArchitect),
            (false, true, true) => Some(CouncilCompositionMode::MixedBots),
            (true, true, true) => Some(CouncilCompositionMode::MixedBotsAndArchitect),
            (false, false, false) => None,
        }
    }

    pub fn mode(&self) -> std::result::Result<CouncilCompositionMode, CouncilCompositionError> {
        self.validate()?;
        self.mode_unchecked().ok_or(CouncilCompositionError::MissingReviewerSource)
    }

    pub fn selected_bot_ids(&self) -> impl Iterator<Item = &str> {
        self.selected.iter().map(|reviewer| reviewer.bot_id.as_str())
    }

    /// For `auto`, choose only the minimum needed to preserve the two-seat quorum, and keep at least
    /// one seat attributable to the explicitly enabled automatic source in hybrid modes.
    pub fn minimum_automatic_seats(&self) -> usize {
        if !self.automatic.enabled { return 0; }
        match self.automatic.count {
            AutomaticReviewerCount::Fixed(count) => count,
            AutomaticReviewerCount::Auto(_) => 1.max(MIN_REVIEWERS.saturating_sub(self.selected.len())),
        }
    }

    pub fn maximum_automatic_seats(&self) -> usize {
        if !self.automatic.enabled { return 0; }
        (self.limits.max_root_seats as usize).saturating_sub(self.selected.len())
    }

    pub fn minimum_root_seats(&self) -> usize {
        self.selected.len().saturating_add(self.minimum_automatic_seats())
    }

    pub fn validate_required_criteria_gate(
        &self,
        root_target: f64,
        per_seat: &[Vec<CouncilCriterionOutcome>],
    ) -> std::result::Result<(), CouncilCompositionError> {
        self.validate()?;
        if !root_target.is_finite() || !(1.0..=10.0).contains(&root_target) {
            return Err(CouncilCompositionError::InvalidCriterionOutcome("root target must be between 1 and 10".into()));
        }
        let mut covered = std::collections::HashSet::new();
        for outcomes in per_seat {
            let mut seat_criteria = std::collections::HashSet::new();
            for outcome in outcomes {
                let Some(criterion) = self.criteria.iter().find(|criterion| criterion.id == outcome.criterion_id) else {
                    return Err(CouncilCompositionError::UnknownCriterionOutcome(outcome.criterion_id.clone()));
                };
                if !seat_criteria.insert(outcome.criterion_id.as_str()) {
                    return Err(CouncilCompositionError::DuplicateCriterionOutcome(outcome.criterion_id.clone()));
                }
                if !outcome.score.is_finite() || !(0.0..=10.0).contains(&outcome.score) {
                    return Err(CouncilCompositionError::InvalidCriterionOutcome(format!("score for {:?} must be between 0 and 10", outcome.criterion_id)));
                }
                let mut evidence_refs = std::collections::HashSet::new();
                if outcome.verified_evidence_refs.iter().any(|reference| reference.is_empty()
                    || reference.len() > MAX_EVIDENCE_REF_CHARS
                    || reference.chars().any(char::is_control)
                    || !evidence_refs.insert(reference.as_str()))
                {
                    return Err(CouncilCompositionError::InvalidCriterionOutcome(format!("verified evidence references for {:?} must be unique, plain, and at most {MAX_EVIDENCE_REF_CHARS} characters", outcome.criterion_id)));
                }
                if !criterion.required { continue; }
                covered.insert(criterion.id.as_str());
                let effective_target = root_target.max(criterion.target);
                if outcome.score < effective_target {
                    return Err(CouncilCompositionError::CriterionBelowTarget { id: criterion.id.clone(), score: outcome.score, target: effective_target });
                }
                if criterion.evidence_required && outcome.verified_evidence_refs.is_empty() {
                    return Err(CouncilCompositionError::MissingCriterionEvidence(criterion.id.clone()));
                }
            }
        }
        for criterion in self.criteria.iter().filter(|criterion| criterion.required) {
            if !covered.contains(criterion.id.as_str()) {
                return Err(CouncilCompositionError::MissingRequiredCriterion(criterion.id.clone()));
            }
        }
        Ok(())
    }

    fn validate_capabilities(
        &self,
        capabilities: &CouncilCompositionCapabilities,
    ) -> std::result::Result<(), CouncilCompositionError> {
        if self.automatic.enabled && !capabilities.automatic_reviewers {
            return Err(CouncilCompositionError::UnsupportedCapability { bot_id: None, capability: "automatic root reviewers" });
        }
        if !capabilities.supported_strategies.contains(&self.strategy) {
            return Err(CouncilCompositionError::UnsupportedCapability { bot_id: None, capability: "selected execution strategy" });
        }
        for selected in &self.selected {
            let Some(reviewer) = capabilities.reviewers.iter().find(|reviewer| reviewer.bot_id == selected.bot_id) else {
                return Err(CouncilCompositionError::UnsupportedCapability { bot_id: Some(selected.bot_id.clone()), capability: "selected reviewer" });
            };
            let supported = match selected.mode {
                SelectedReviewerMode::Direct => reviewer.direct,
                SelectedReviewerMode::Nested => reviewer.nested,
            };
            if !supported {
                return Err(CouncilCompositionError::UnsupportedCapability {
                    bot_id: Some(selected.bot_id.clone()),
                    capability: if selected.mode == SelectedReviewerMode::Direct { "direct review" } else { "nested Council review" },
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct CouncilConfig {
    /// The score every reviewer must reach.
    pub target: f64,
    /// Rounds that may start this cycle (a round starts when one of its seats runs), 1..=100.
    pub max_rounds: usize,
    /// 0 = the lead decides for automatic/native reviewers.
    pub reviewers: usize,
    /// Persistent bot IDs for task-scoped selected reviewers; empty keeps native Council behavior.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewer_bot_ids: Vec<String>,
    /// Explicitly user-optional bot-assigned skills, frozen per Council task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_skill_ids: Vec<String>,
    /// Explicitly required native integrity checks, distinct from mandatory methodology.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_check_skill_ids: Vec<String>,
    /// Explicit per-task host tool permissions; absent always means native read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_permissions: Option<super::reviewer_security::ReviewerPermissionSelection>,
    /// Per-message, user-authorized GitHub issue creation only after native Council approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_acceptance_issue: Option<PostAcceptanceIssue>,
    /// Additive composition contract. Missing preserves the original M1/M2 Council behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<CouncilComposition>,
}
impl Default for CouncilConfig {
    fn default() -> Self {
        Self { target: 9.0, max_rounds: 10, reviewers: 0, reviewer_bot_ids: Vec::new(), optional_skill_ids: Vec::new(), required_check_skill_ids: Vec::new(), review_permissions: None, post_acceptance_issue: None, composition: None }
    }
}
impl CouncilConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if !self.target.is_finite() || !(1.0..=10.0).contains(&self.target) {
            return Err("The Council target must be between 1 and 10.".into());
        }
        if !(1..=MAX_ROUNDS).contains(&self.max_rounds) {
            return Err(format!("Council takes 1 to {MAX_ROUNDS} rounds."));
        }
        if self.reviewers > MAX_REVIEWERS {
            return Err(format!("Council allows at most {MAX_REVIEWERS} reviewers."));
        }
        self.validate_composition().map_err(|error| error.to_string())?;
        if !self.optional_skill_ids.is_empty() {
            if self.reviewer_bot_ids.is_empty() || self.optional_skill_ids.len() > 32 {
                return Err("Optional reviewer skills require selected bots and at most 32 IDs.".into());
            }
            let mut unique = std::collections::HashSet::new();
            if self.optional_skill_ids.iter().any(|id| !super::valid_id(id) || !unique.insert(id)) {
                return Err("Optional reviewer skills must have unique valid IDs.".into());
            }
        }
        if !self.required_check_skill_ids.is_empty() {
            if self.reviewer_bot_ids.is_empty() || self.required_check_skill_ids.len() > 32 {
                return Err("Native skill checks require selected bots and at most 32 IDs.".into());
            }
            let mut unique = std::collections::HashSet::new();
            if self.required_check_skill_ids.iter().any(|id| !super::valid_id(id)
                || self.optional_skill_ids.contains(id) || !unique.insert(id))
            {
                return Err("Required skill checks must be distinct, valid and non-optional.".into());
            }
        }
        if self.review_permissions.is_some() && self.reviewer_bot_ids.is_empty() {
            return Err("Scoped Council reviewer permissions require selected bots.".into());
        }
        if let Some(permissions) = &self.review_permissions {
            if permissions.authorization_id.trim().is_empty() || permissions.authorization_id.len() > 128
                || permissions.grants.is_empty() || permissions.grants.len() > 16
            {
                return Err("A bounded user authorization and at least one grant are required.".into());
            }
            for grant in &permissions.grants { grant.validate()?; }
        }
        if let Some(followup) = &self.post_acceptance_issue { followup.validate()?; }
        if !self.reviewer_bot_ids.is_empty() {
            if (self.composition.is_none() && !(MIN_REVIEWERS..=MAX_REVIEWERS).contains(&self.reviewer_bot_ids.len()))
                || self.reviewer_bot_ids.len() > MAX_REVIEWERS
            {
                return Err(format!("Select {MIN_REVIEWERS} to {MAX_REVIEWERS} distinct Council reviewer bots."));
            }
            let mut ids = std::collections::HashSet::new();
            if self.reviewer_bot_ids.iter().any(|id| !super::valid_id(id) || !ids.insert(id)) {
                return Err("Council reviewer bot IDs must be valid and unique.".into());
            }
        }
        Ok(())
    }

    /// Validate the additive shape and both legacy mirror fields. Legacy configs return `None`.
    pub fn validate_composition(&self) -> std::result::Result<Option<CouncilCompositionMode>, CouncilCompositionError> {
        let Some(composition) = self.composition.as_ref() else { return Ok(None) };
        let mode = composition.mode()?;
        if composition.strategy == CouncilStrategy::TwoPhase && self.max_rounds < 2 {
            return Err(CouncilCompositionError::InvalidLimit("twoPhase requires at least two rounds"));
        }
        if !self.reviewer_bot_ids.iter().map(String::as_str).eq(composition.selected_bot_ids()) {
            return Err(CouncilCompositionError::MirrorMismatch("reviewerBotIds"));
        }
        let mirrored_reviewers = if composition.automatic.enabled {
            composition.automatic.count.fixed().unwrap_or(0)
        } else {
            0
        };
        if self.reviewers != mirrored_reviewers {
            return Err(CouncilCompositionError::MirrorMismatch("reviewers"));
        }
        Ok(Some(mode))
    }

    /// Typed dispatch validation. Call with the executing bot identity and the live, native roster
    /// capabilities immediately before reserving or starting any composition seat.
    pub fn validate_for_dispatch(
        &self,
        executor_bot_id: Option<&str>,
        capabilities: &CouncilCompositionCapabilities,
    ) -> std::result::Result<Option<CouncilCompositionMode>, CouncilCompositionError> {
        self.validate().map_err(CouncilCompositionError::InvalidCouncilConfig)?;
        let Some(composition) = self.composition.as_ref() else { return Ok(None) };
        if let Some(executor) = executor_bot_id {
            if composition.selected.iter().any(|reviewer| reviewer.bot_id == executor) {
                return Err(CouncilCompositionError::SelfReview(executor.to_owned()));
            }
        }
        composition.validate_capabilities(capabilities)?;
        composition.mode().map(Some)
    }

    pub(super) fn needed(&self) -> usize {
        self.seat_plan().required()
    }

    /// The minimum complete root-seat plan, separated by source. Composition-absent M1/M2 retain
    /// their historic automatic quorum and selected roster count exactly.
    pub(super) fn seat_plan(&self) -> CouncilSeatPlan {
        if let Some(composition) = self.composition.as_ref() {
            return CouncilSeatPlan {
                automatic: composition.minimum_automatic_seats(),
                direct_bots: composition.selected.iter().filter(|seat| seat.mode == SelectedReviewerMode::Direct).count(),
                nested_bots: composition.selected.iter().filter(|seat| seat.mode == SelectedReviewerMode::Nested).count(),
            };
        }
        if self.reviewer_bot_ids.is_empty() {
            CouncilSeatPlan { automatic: self.reviewers.max(MIN_REVIEWERS), ..CouncilSeatPlan::default() }
        } else {
            CouncilSeatPlan { direct_bots: self.reviewer_bot_ids.len(), ..CouncilSeatPlan::default() }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CouncilSeat {
    pub round: usize,
    pub lens: String,
    pub score: Option<f64>,
    /// The user explicitly closed this incomplete round by changing its reviewer selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed_by_selection: bool,
    /// When this seat first ran (`storage::now`); its round is timed from its seats' first runs, so a
    /// reviewer that waits on a dependency or a free slot is not out of date before it reads anything.
    #[serde(skip_serializing_if = "is_zero")]
    pub started_at: i64,
}
fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// Where Council stands, on every snapshot while it is on; the webview shows this as it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CouncilStatus {
    /// "accepted" (the user accepted this cycle's scores), "waiting" (no round yet), "reviewing" (a seat
    /// of the latest round is live), "dispatch" (the
    /// latest round is incomplete with no live seat: the lead must act), "asking" (the lead asks the
    /// user), "fixing" (below target or out of date), "passed" or "limit" (the round limit is reached
    /// without passing).
    pub state: String,
    pub round: usize,
    pub lowest: Option<f64>,
    pub target: f64,
    /// Changes landed after the latest round started.
    pub stale: bool,
    /// A short plain sentence for the user while a Council refusal holds the team (asking or paused).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// The lead's setting on a task's team; takes effect at once. It starts a lead turn only when it ends
/// the lead's question to the user (the team was asking and no longer is).
pub(super) fn set(state: &AppState, session_id: &str, council: Option<CouncilConfig>) -> Result<TeamSnapshot> {
    if let Some(config) = &council {
        config.validate()?;
    }
    let agents = state.agents.lock().map_err(|e| e.to_string())?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    super::refuse_mirror(&store, session_id)?;
    let mut team = super::load(&store, session_id)?.ok_or("This task has no team.")?;
    super::bot_reviewers::allow_roster_change(&team, &council)?;
    if let Some(config) = &council {
        super::bot_reviewers::validate_roster(&store, session_id, config)?;
    }
    // Turning it off stops this cycle's reviewers that are still out: nothing waits for their scores.
    let mut stops = if council.is_none() {
        let since = team.council_since;
        let out: Vec<String> = team.workers.iter().filter(|w| w.council.is_some() && w.created_at >= since && !w.abandoned && unsettled(&w.status)).map(|w| w.id.clone()).collect();
        stop_seats(&mut team, &out, "Stopped: Council was turned off.")
    } else {
        Vec::new()
    };
    // A queued setup must not bring back what this replaces when it applies.
    if let Some(pending) = &mut team.pending_setup {
        pending.config.council = council.clone();
    }
    // Turning it on reviews what comes next, not the thread's earlier work. A request under way,
    // paused, failed or interrupted keeps its start, so its writes are still reviewed.
    if team.config.council.is_none() && council.is_some() && !super::active(&team.status) && !super::can_resume(&team.status) {
        team.council_since = storage::now();
    }
    let old = finish_refusal(&team);
    let asking = |team: &TeamSnapshot| status(team).is_some_and(|s| s.state == "asking");
    let was_asking = asking(&team);
    // A team resting with nothing for Council to refuse (passed, or at its round limit).
    let resting = matches!(team.status.as_str(), "ready" | "completed") && old.is_none();
    let old_rounds = team.config.council.as_ref().map(|c| c.max_rounds);
    super::bot_reviewers::recover_roster_change(&mut team, &council);
    stops.extend(recover_composition_change(&mut team, &council));
    team.config.council = council;
    super::refresh_council(&store, &mut team);
    // The team's error shows the refusal as it now stands; a lifted one leaves only the rest (a pause note).
    if let Some(old) = old.filter(|old| team.error.as_deref().is_some_and(|e| e.contains(old.as_str()))) {
        let error = team.error.take().unwrap_or_default().replace(&old, &finish_refusal(&team).unwrap_or_default());
        let error = error.split_whitespace().collect::<Vec<_>>().join(" ");
        team.error = (!error.is_empty()).then_some(error);
    }
    // The user answered the lead's question: the lead verifies and finishes, once.
    if was_asking && !asking(&team) {
        let ask = match finish_refusal(&team) {
            Some(refusal) => format!("The user changed the Council setting, which answers your question, but Council still refuses: {refusal}"),
            None => finish_prompt(&team, "The user changed the Council setting, which answers your question."),
        };
        super::lift_turn(&store, &mut team, &ask)?;
    } else if let Some(refusal) = finish_refusal(&team).filter(|_| resting) {
        // The setting reopened work the lead can act on (a raised limit makes a new round possible): it
        // gets one turn, and a completed team returns to running for it.
        let rounds = team.config.council.as_ref().map(|c| c.max_rounds).filter(|n| old_rounds.is_some_and(|old| old < *n));
        let why = rounds.map_or_else(|| "The user changed the Council setting".to_string(), |n| format!("The user raised the Council round limit to {n}"));
        super::lift_turn(&store, &mut team, &format!("{why}, so Council refuses again: {refusal}"))?;
    }
    super::save(&store, &mut team)?;
    for sid in stops {
        if let Some(agent) = agents.get(&sid) {
            let _ = agent.send(crate::RunInput::Stop);
        }
    }
    Ok(team)
}

/// Asks the lead to finish with the first-and-final table; the scores travel in the prompt, since the
/// review may have no `council` block (Council off) or no refusal (accepted).
fn finish_prompt(team: &TeamSnapshot, why: &str) -> String {
    let table: Vec<String> = score_table(team).iter().map(|(lens, first, last)| format!("{lens}: first {first}, final {last}")).collect();
    format!("{why} Verify the work and call finish_team, ending with the table of each lens's first and final score ({}).", table.join("; "))
}
fn accepted(team: &TeamSnapshot) -> bool {
    team.council_accepted == Some(team.council_since)
}
/// The user accepts this cycle's scores: the gate lets the lead finish, Council stays on, and the next
/// cycle reviews as usual. An idle team's lead is woken once to finish.
pub(super) fn accept(state: &AppState, session_id: &str) -> Result<TeamSnapshot> {
    let agents = state.agents.lock().map_err(|e| e.to_string())?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    super::refuse_mirror(&store, session_id)?;
    let mut team = super::load(&store, session_id)?.ok_or("This task has no team.")?;
    if team.config.council.is_none() {
        return Err("Council is off for this team.".into());
    }
    let refusal = finish_refusal(&team);
    // Nothing waits for scores once they are accepted.
    let since = team.council_since;
    let out: Vec<String> = team.workers.iter().filter(|w| w.council.is_some() && w.created_at >= since && !w.abandoned && unsettled(&w.status)).map(|w| w.id.clone()).collect();
    let stops = stop_seats(&mut team, &out, "Stopped: the user accepted the Council scores.");
    let wake = refusal.is_some();
    team.council_accepted = Some(team.council_since);
    if let Some(refusal) = refusal.filter(|r| team.error.as_deref().is_some_and(|e| e.contains(r.as_str()))) {
        let error = team.error.take().unwrap_or_default().replace(&refusal, "");
        let error = error.split_whitespace().collect::<Vec<_>>().join(" ");
        team.error = (!error.is_empty()).then_some(error);
    }
    // Only a resting team is woken: a paused, failed or interrupted one keeps its note, and Resume
    // continues it. A cycle that already passed has nothing for the lead to do.
    if wake && team.status == "ready" {
        let ask = finish_prompt(&team, "The user accepted the current Council scores.");
        super::lift_turn(&store, &mut team, &ask)?;
    }
    super::save(&store, &mut team)?;
    for sid in stops {
        if let Some(agent) = agents.get(&sid) {
            let _ = agent.send(crate::RunInput::Stop);
        }
    }
    Ok(team)
}

/// The score in a report: the last line that starts (past markdown) with "council score", read as
/// the number after it, optionally `/10`. A last line that does not read as 0..=10 is no score.
pub(super) fn parse_score(report: &str) -> Option<f64> {
    const LABEL: &str = "council score";
    let rest = report.lines().rev().find_map(|line| {
        let line = line.trim_start_matches(|c: char| c.is_whitespace() || "*#_>-`~".contains(c));
        line.get(..LABEL.len()).filter(|head| head.eq_ignore_ascii_case(LABEL))?;
        Some(&line[LABEL.len()..])
    })?;
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || ":*_`=".contains(c));
    let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let fraction = rest[digits..].strip_prefix('.').map_or(0, |after| {
        let count = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
        if count == 0 { 0 } else { count + 1 }
    });
    let end = digits + fraction;
    let tail = rest[end..].trim_start();
    if digits == 0 || rest[end..].starts_with(|c: char| c.is_alphanumeric()) {
        return None;
    }
    if let Some(over) = tail.strip_prefix('/') {
        let over = over.trim_start();
        if !over.starts_with("10") || over[2..].starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
    }
    rest[..end].parse().ok().filter(|score: &f64| (0.0..=10.0).contains(score))
}

/// The score in a reviewer's latest reply, read from the message's tail. Reads no more than that,
/// as a report can be megabytes.
pub(super) fn latest_score(store: &storage::Store, session_id: &str) -> Option<f64> {
    store.db.query_row("SELECT substr(content,?2) FROM messages WHERE session_id=?1 AND role='assistant' AND id>(SELECT COALESCE(MAX(id),0) FROM messages WHERE session_id=?1 AND role IN ('user','coordination')) ORDER BY id DESC LIMIT 1", params![session_id, -SCORE_TAIL_CHARS], |r| r.get::<_, String>(0)).optional().ok().flatten().and_then(|tail| parse_score(&tail))
}

/// This cycle's reviewers: each new user objective starts a new cycle.
fn seats(team: &TeamSnapshot) -> impl Iterator<Item = (&Worker, &CouncilSeat)> {
    team.workers.iter().filter(|w| w.created_at >= team.council_since).filter_map(|w| Some((w, w.council.as_ref()?)))
}
fn latest_round(team: &TeamSnapshot) -> usize {
    seats(team).map(|(_, seat)| seat.round).max().unwrap_or(0)
}
fn in_round(team: &TeamSnapshot, round: usize) -> impl Iterator<Item = (&Worker, &CouncilSeat)> {
    seats(team).filter(move |(_, seat)| seat.round == round)
}
/// A seat that counts: one not abandoned, or one that scored, since a score stands once given. An
/// abandoned seat that never scored drops out.
fn counts(worker: &Worker, seat: &CouncilSeat) -> bool {
    !worker.abandoned || seat.score.is_some()
}
/// A persisted native session cannot count for multiple scored Council seats.
/// Keep this invariant for legacy and composed rounds, including restored tasks.
fn independent_scored_sessions(team: &TeamSnapshot, round: usize) -> bool {
    let mut workers = std::collections::HashSet::new();
    let mut sessions = std::collections::HashSet::new();
    in_round(team, round)
        .filter(|(worker, seat)| counts(worker, seat) && seat.score.is_some())
        .all(|(worker, _)| {
            !worker.id.is_empty() && !worker.session_id.is_empty()
                && workers.insert(worker.id.as_str())
                && sessions.insert(worker.session_id.as_str())
        })
}

fn same_lens(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}
/// Seats that have settled with a score.
fn scored(team: &TeamSnapshot, round: usize) -> impl Iterator<Item = f64> + '_ {
    in_round(team, round).filter(|(w, seat)| counts(w, seat) && !unsettled(&w.status)).filter_map(|(_, seat)| seat.score)
}
fn lowest(team: &TeamSnapshot, round: usize) -> Option<f64> {
    scored(team, round).reduce(f64::min)
}

/// A score is not evidence that a mandatory reviewer skill was invoked or a
/// check was actually executed. Only independently verified native receipts
/// can satisfy those obligations; optional skill pins never impose this gate.
fn unverified_required_skill_checks(team: &TeamSnapshot, round: usize) -> Vec<String> {
    let mut missing = Vec::new();
    for (worker, seat) in in_round(team, round) {
        if !counts(worker, seat) { continue; }
        if let Some(skills) = worker.reviewer_binding.as_ref().and_then(|binding| binding.snapshot.skills.as_ref()) {
            for pin in &skills.pins {
                if pin.required_for_acceptance && !(pin.eligible && pin.supplied) {
                    missing.push(format!("{} ({}: not supplied)", worker.name, pin.id));
                }
                if pin.check_required && !worker.reviewer_binding.as_ref()
                    .and_then(|binding| binding.skill_evidence.get(&pin.id)
                        .map(|evidence| evidence.valid_for(pin, &binding.artifact_revision)))
                    .unwrap_or(false)
                {
                    missing.push(format!("{} ({}: native check unverified)", worker.name, pin.id));
                }
            }
        }
    }
    missing
}
fn live(team: &TeamSnapshot, round: usize) -> bool {
    in_round(team, round).any(|(w, _)| unsettled(&w.status))
}
/// One check's view of the cycle's rounds, computed once: the latest round, whether each is complete
/// (oldest first), how many started, and the lenses the first round fixed.
struct Rounds {
    latest: usize,
    done: Vec<bool>,
    started: usize,
    lenses: Vec<String>,
}
impl Rounds {
    fn of(team: &TeamSnapshot, config: &CouncilConfig) -> Self {
        let latest = latest_round(team);
        let mut lenses: Vec<String> = Vec::new();
        // Every round-1 seat fixes its lens, a cancelled or overtaken one too: dropping a seat cannot
        // drop its lens from later rounds.
        for (_, seat) in in_round(team, 1) {
            if !lenses.iter().any(|lens| same_lens(lens, &seat.lens)) {
                lenses.push(seat.lens.clone());
            }
        }
        let mut view = Rounds { latest, done: Vec::new(), started: 0, lenses };
        for round in 1..=latest {
            let seated = in_round(team, round).filter(|(w, seat)| counts(w, seat)).count();
            let count = scored(team, round).count();
            let required = required_reviewers(team, config, round);
            let complete = !reconfigured(team, round) && count == seated && count >= required
                && independent_scored_sessions(team, round)
                && view.missing(team, round).is_empty();
            view.done.push(complete);
            view.started += usize::from(in_round(team, round).any(|(_, seat)| seat.started_at > 0));
        }
        view
    }
    fn complete(&self, round: usize) -> bool {
        round > 0 && self.done.get(round - 1).copied().unwrap_or(false)
    }
    fn at_limit(&self, config: &CouncilConfig) -> bool {
        self.started >= config.max_rounds
    }
    /// The first round's lenses with no seat in `round`: every one sits again in each later round.
    fn missing(&self, team: &TeamSnapshot, round: usize) -> Vec<String> {
        if round <= 1 {
            return Vec::new();
        }
        self.lenses.iter().filter(|lens| !in_round(team, round).any(|(w, seat)| counts(w, seat) && same_lens(&seat.lens, lens))).cloned().collect()
    }
}
/// S2 selected rounds freeze their quorum. Automatic/native rounds retain pre-S2 count edits,
/// including automatic seats saved by versions that erroneously issued a frozen quorum.
fn required_reviewers(team: &TeamSnapshot, config: &CouncilConfig, round: usize) -> usize {
    if config.composition.is_some() {
        let plan = config.seat_plan();
        // A native automatic seat pins the whole source-aware quota. A selected bot binding pins
        // only its frozen bot roster, so it cannot erase automatic seats still owed by M3/M5/M7.
        let frozen = if plan.automatic > 0 {
            in_round(team, round).filter_map(|(worker, _)| worker.reviewer_binding.as_ref())
                .find(|binding| binding.reviewer_bot_id.is_none() && binding.roster_bot_ids.is_empty())
                .and_then(|binding| binding.required_reviewers)
        } else {
            in_round(team, round).filter_map(|(worker, _)| worker.reviewer_binding.as_ref())
                .find(|binding| !binding.roster_bot_ids.is_empty())
                .and_then(|binding| binding.required_reviewers)
        };
        return frozen.unwrap_or_else(|| plan.required());
    }
    in_round(team, round).filter_map(|(worker, _)| worker.reviewer_binding.as_ref())
        .find(|binding| !binding.roster_bot_ids.is_empty())
        .and_then(|binding| binding.required_reviewers).unwrap_or_else(|| config.needed())
}
pub(super) fn fixed_lens_count(team: &TeamSnapshot) -> usize {
    team.config.council.as_ref().map_or(0, |config| Rounds::of(team, config).lenses.len())
}
fn reconfigured(team: &TeamSnapshot, round: usize) -> bool {
    in_round(team, round).any(|(_, seat)| seat.closed_by_selection)
}
/// A validated user selection change closes an incomplete, settled round. Its reports and lenses
/// remain in history; unscored seats cannot run again, and the next dispatch opens a fresh round.
pub(super) fn recover_round(team: &mut TeamSnapshot) {
    let round = latest_round(team);
    if !current_round_incomplete(team) { return; }
    let ids: Vec<String> = in_round(team, round).filter(|(_, seat)| seat.score.is_none()).map(|(w, _)| w.id.clone()).collect();
    stop_seats(team, &ids, "Stopped: the user changed the Council reviewer selection; dispatch a new round.");
    let since = team.council_since;
    for seat in team.workers.iter_mut().filter(|w| w.created_at >= since).filter_map(|w| w.council.as_mut()).filter(|s| s.round == round) {
        seat.closed_by_selection = true;
    }
}
/// Whether the latest round still needs scores or lenses, under its original selected quorum.
pub(super) fn current_round_incomplete(team: &TeamSnapshot) -> bool {
    let Some(config) = &team.config.council else { return false };
    let round = latest_round(team);
    round > 0 && !accepted(team) && !Rounds::of(team, config).complete(round)
}
pub(super) fn complete(team: &TeamSnapshot, config: &CouncilConfig, round: usize) -> bool {
    Rounds::of(team, config).complete(round)
}
/// When a round began: its earliest seat's first run, an abandoned one included, as the rest may
/// have read the work that early. A round none of whose seats has run has not begun.
fn round_start(team: &TeamSnapshot, round: usize) -> i64 {
    in_round(team, round).map(|(_, seat)| seat.started_at).filter(|at| *at > 0).min().unwrap_or(i64::MAX)
}
/// A writer ran, or any team conversation recorded changed files (`changed_at`), after `start`.
fn written_since(team: &TeamSnapshot, start: i64) -> bool {
    team.changed_at > start || team.workers.iter().any(|w| w.wrote_at > start)
}
fn has_writer(team: &TeamSnapshot) -> bool {
    let since = team.council_since;
    (team.changed_at > 0 && team.changed_at >= since) || team.workers.iter().any(|w| w.wrote_at > 0 && w.wrote_at >= since)
}
fn stale(team: &TeamSnapshot, round: usize) -> bool {
    round > 0 && written_since(team, round_start(team, round))
}

/// The round a new reviewer takes its seat in: the seat it replaces, else the latest round while it
/// is open, current and has room, else the next one if the limit allows. A complete round below the
/// target with nothing changed since is not re-rolled.
pub(super) fn round_for(team: &TeamSnapshot, config: &CouncilConfig, replaces: Option<&str>) -> Result<usize> {
    config.validate_composition().map_err(|error| error.to_string())?;
    let view = Rounds::of(team, config);
    let latest = view.latest;
    let current = latest > 0 && !stale(team, latest) && !reconfigured(team, latest);
    let replaced = replaces.is_some_and(|id| in_round(team, latest).any(|(w, _)| w.id == id));
    let done = view.complete(latest);
    // Abandoned seats take room too, so dropping and re-dispatching cannot go on forever.
    let room = in_round(team, latest).count() < MAX_REVIEWERS;
    if current && room && (replaced || !done) {
        return Ok(latest);
    }
    if view.at_limit(config) {
        return Err(format!("The Council round limit of {} is reached. Finish and report the scores plainly, or ask the user to raise the limit.", config.max_rounds));
    }
    // S2 treats a below-target score as an on-demand escalation trigger, so it
    // may open a fresh review round on the same artifact. Parallel keeps the
    // legacy requirement to fix the work before another round.
    let escalated = config.composition.as_ref().is_some_and(|composition| {
        composition.strategy == CouncilStrategy::Escalated
    });
    if let Some(low) = lowest(team, latest).filter(|low| current && *low < config.target && !escalated) {
        return Err(format!("Council round {latest} scored lowest {low}, below the target {}, and nothing changed since it started. Fix the blockers with workers before convening a new round.", config.target));
    }
    Ok(latest + 1)
}

/// The seat for a dispatched assignment, if it is a reviewer. A reviewer's replacement stays one,
/// with its lens unless it names another. Call before the replaced seat is abandoned.
pub(super) fn seat_for(team: &TeamSnapshot, lens: Option<&str>, replaces: Option<&str>) -> Result<Option<CouncilSeat>> {
    if let Some(lens) = lens {
        if lens.trim().is_empty() || lens.chars().count() > LENS_CHARS || lens.chars().any(char::is_control) {
            return Err(format!("councilLens must be a plain name of 1-{LENS_CHARS} characters."));
        }
    }
    let inherited = replaces.and_then(|id| team.workers.iter().find(|w| w.id == id)).and_then(|w| w.council.as_ref()).map(|seat| seat.lens.as_str());
    let Some(config) = team.config.council.as_ref() else {
        return match lens {
            Some(_) => Err("Council is off for this team, so councilLens cannot be used.".into()),
            None => Ok(None),
        };
    };
    config.validate_composition().map_err(|error| error.to_string())?;
    if let (Some(named), Some(old)) = (lens, inherited) {
        if !same_lens(named, old) {
            return Err(format!("A replacement keeps the lens of the seat it replaces ({old}): omit councilLens or use that one."));
        }
    }
    let Some(lens) = inherited.or(lens) else {
        return Ok(None);
    };
    let round = round_for(team, config, replaces)?;
    let fixed = Rounds::of(team, config).lenses;
    let bounded_roster = config.composition.is_some() || !config.reviewer_bot_ids.is_empty();
    if round == 1 && bounded_roster && fixed.len() >= config.needed()
        && !fixed.iter().any(|known| same_lens(known, lens))
    {
        return Err(if config.composition.is_some() {
            "The configured Council seat plan cannot cover another fixed Council lens.".into()
        } else {
            "The selected reviewer roster cannot cover another fixed Council lens.".into()
        });
    }
    if round > 1 && !fixed.is_empty() && !fixed.iter().any(|known| same_lens(known, lens)) {
        return Err(format!("Council lenses are fixed by round 1: {}. Use one of them for {lens:?}.", fixed.join(", ")));
    }
    Ok(Some(CouncilSeat { round, lens: lens.trim().into(), score: None, started_at: 0, closed_by_selection: false, ..CouncilSeat::default() }))
}

/// When a new round opens over an incomplete one (changes overtook it), that round's unscored
/// reviewers stop and are abandoned, so they stop spending; their seats block nothing any more.
pub(super) fn overtake(team: &mut TeamSnapshot, round: usize) -> Vec<String> {
    let since = team.council_since;
    let note = format!("Stopped: changes landed after its round started, and round {round} replaces it.");
    let overtaken: Vec<String> = team.workers.iter()
        .filter(|w| w.created_at >= since && !w.abandoned && w.council.as_ref().is_some_and(|seat| seat.round < round && seat.score.is_none()))
        .map(|w| w.id.clone())
        .collect();
    stop_seats(team, &overtaken, &note)
}
/// Settles these reviewers (parked ones included) as stopped and abandoned, with their sub-workers,
/// and returns the sessions whose runs the caller stops once its changes are saved.
fn stop_seats(team: &mut TeamSnapshot, ids: &[String], note: &str) -> Vec<String> {
    let mut stops = Vec::new();
    let none = std::collections::HashMap::new();
    for id in ids {
        stops.extend(team.workers.iter().filter(|w| (w.id == *id || super::child_of(w, id)) && super::stoppable(&w.status)).map(|w| w.session_id.clone()));
        let Some(worker) = team.workers.iter_mut().find(|w| w.id == *id) else { continue };
        if unsettled(&worker.status) {
            worker.status = "stopped".into();
        }
        worker.abandoned = true;
        worker.error = Some(note.into());
        worker.updated_at = storage::now();
        super::settle_children(team, id, &none, "Its Council seat was stopped.");
    }
    stops
}

/// A composition change creates a new frozen plan and cannot reuse seats from its previous criteria,
/// roster, limits or strategy. Turn the old round into the same explicit closed state used by roster
/// changes and stop any live native sessions after the updated team is saved.
fn recover_composition_change(team: &mut TeamSnapshot, next: &Option<CouncilConfig>) -> Vec<String> {
    let old = team.config.council.as_ref().and_then(|config| config.composition.as_ref());
    let new = next.as_ref().and_then(|config| config.composition.as_ref());
    if old == new || (old.is_none() && new.is_none()) || next.is_none() { return Vec::new(); }
    let round = latest_round(team);
    if round == 0 { return Vec::new(); }
    let ids: Vec<String> = in_round(team, round).map(|(worker, _)| worker.id.clone()).collect();
    let stops = stop_seats(team, &ids, "Stopped: the Council composition changed; dispatch a fresh round with the new frozen plan.");
    let since = team.council_since;
    for seat in team.workers.iter_mut().filter(|worker| worker.created_at >= since).filter_map(|worker| worker.council.as_mut()).filter(|seat| seat.round == round) {
        seat.closed_by_selection = true;
    }
    stops
}

/// The opening of a Continue turn when Council has not passed.
pub(super) fn continue_note(team: &TeamSnapshot, refusal: &str) -> String {
    if asks_user(team) {
        format!("Council has not passed this work: {refusal} Fix the blockers through workers and convene a new round; only if a remaining blocker should not be fixed, end your turn and tell the user.")
    } else {
        format!("Council has not passed this work: {refusal} Act on that now.")
    }
}

/// Why a reviewer may not be cancelled or replaced: a score, once given, stands.
/// A seat whose score nudge is pending has a report too, and its score is on the way.
pub(super) fn refuse_unseat(worker: &Worker) -> Result<()> {
    match worker.council.as_ref().and_then(|seat| seat.score) {
        Some(_) => Err(SCORED_STANDS.into()),
        None if worker.council.is_some() && worker.score_nudged && unsettled(&worker.status) => Err(NUDGE_PENDING.into()),
        // A running reviewer is reading the work: let it report. The watchdog fails a hung one.
        None if worker.council.as_ref().is_some_and(|seat| seat.started_at > 0) && super::holds(&worker.status) => Err(RUNNING_SEAT.into()),
        None => Ok(()),
    }
}

/// Whether the refusal is a round below the target: the lead fixes it, or else asks the user. A lead
/// that ends its turn on it after one Continue is asking the user.
pub(super) fn asks_user(team: &TeamSnapshot) -> bool {
    verdict(team).is_some_and(|(_, asks)| asks)
}

pub(super) fn asks_user_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> bool {
    verdict_with_criterion_outcomes(team, outcomes).is_some_and(|(_, asks)| asks)
}
pub(super) fn asks_user_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> bool {
    verdict_with_ledger(team, outcomes, findings).is_some_and(|(_, asks)| asks)
}

/// Why the lead may not finish yet, naming what to do next; `None` lets it finish. A cycle with no
/// changes and no round finishes as it always has; once a round exists its scores bind.
// ponytail: changes are seen through writers' runs and the lead's and workers' recorded turn changes
// (write tools, or edits reported through track_files). Known ceilings: the lead's unreported shell
// edits are not seen (nor edits from outside PandaCode); a writer run that changed nothing still counts
// as a change; and a reviewer that stopped or failed before scoring can be cancelled and its lens seated
// again within the round's 8 seats. Fingerprint the checkout if the first two start to matter.
pub(super) fn finish_refusal(team: &TeamSnapshot) -> Option<String> {
    verdict(team).map(|(refusal, _)| refusal)
}
pub(super) fn finish_refusal_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> Option<String> {
    verdict_with_criterion_outcomes(team, outcomes).map(|(refusal, _)| refusal)
}
pub(super) fn finish_refusal_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<String> {
    verdict_with_ledger(team, outcomes, findings).map(|(refusal, _)| refusal)
}
fn verdict(team: &TeamSnapshot) -> Option<(String, bool)> {
    verdict_with_criterion_outcomes(team, &[])
}
fn verdict_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> Option<(String, bool)> {
    verdict_with_ledger(team, outcomes, &[])
}
fn verdict_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<(String, bool)> {
    let config = team.config.council.as_ref()?;
    verdict_in(team, config, &Rounds::of(team, config), outcomes, findings)
}
/// The verdict from a view the caller already computed.
fn verdict_in(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    view: &Rounds,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<(String, bool)> {
    if config.composition.is_some() {
        return composed_verdict_in(team, config, view, outcomes, findings);
    }
    if accepted(team) {
        return None;
    }
    let round = view.latest;
    let needed = required_reviewers(team, config, round);
    if round == 0 {
        return has_writer(team).then(|| (format!("No Council round yet. Once writers settle, dispatch at least {needed} reviewers in one batch with councilLens."), false));
    }
    if reconfigured(team, round) && !view.at_limit(config) {
        return Some((format!("Council round {round} was closed by the user's reviewer selection change. Dispatch a new round with councilLens."), false));
    }
    // The limit is the user's brake: no further round can open. The lead finishes once nothing is live
    // and the last round is complete, out of date, or full; a current round it can still fill comes first.
    if view.at_limit(config) && !live(team, round) && (reconfigured(team, round) || view.complete(round) || stale(team, round) || in_round(team, round).count() >= MAX_REVIEWERS) {
        return None;
    }
    if !view.complete(round) {
        let missing = view.missing(team, round);
        let count = scored(team, round).count();
        let lenses = if missing.is_empty() { String::new() } else { format!(" It still needs the lenses round 1 seated: {}.", missing.join(", ")) };
        // Seats that stopped or failed without a score hold the round until they are cancelled.
        let stopped: Vec<String> = in_round(team, round).filter(|(w, seat)| !w.abandoned && seat.score.is_none() && !unsettled(&w.status)).map(|(w, _)| format!("{} ({})", w.name, w.id)).collect();
        let next = if stopped.is_empty() { "Dispatch the missing reviewers with councilLens, or wait for those still running.".to_string() } else { format!("Cancel the stopped reviewers {}, then dispatch the missing reviewers with councilLens.", stopped.join(", ")) };
        return Some((format!("Council round {round} is not complete: {count} scored, at least {needed} needed.{lenses} {next}"), false));
    }
    if stale(team, round) {
        return Some((format!("Council round {round} is out of date: changes landed after it started. Dispatch a new round with councilLens."), false));
    }
    let unverified = unverified_required_skill_checks(team, round);
    if !unverified.is_empty() {
        return Some((format!(
            "reviewer_skill_evidence_unverified: Council round {round} has missing mandatory skill input or explicitly required native skill checks without independently verified execution: {}. Native invocation, check and evidence receipts are required; reviewer claims or scores alone do not count.",
            unverified.join(", ")
        ), false));
    }
    let low = lowest(team, round)?;
    if low >= config.target {
        return None;
    }
    Some((format!("Council round {round}: lowest {low}, target {}. Fix the blockers with workers, then dispatch a new round with councilLens. {ESCAPE}", config.target), true))
}

/// Composed plans keep the legacy score/round/stale machinery but add a fail-closed criteria gate.
/// Unlike legacy direct-team completion, reaching the configured limit never waives a missing
/// criterion or a score/evidence failure.
fn composed_verdict_in(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    view: &Rounds,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<(String, bool)> {
    if let Err(error) = config.validate_composition() {
        return Some((format!("council_composition_invalid: {error}"), false));
    }
    if composition_deadline_exceeded(team, storage::now()) {
        return Some(("council_deadline_exceeded: the root Council composition exceeded its frozen timeout; this cycle cannot pass.".into(), false));
    }
    if accepted(team) {
        return None;
    }
    let round = view.latest;
    let needed = required_reviewers(team, config, round);
    if round == 0 {
        return has_writer(team).then(|| (format!("No Council round yet. Once writers settle, dispatch at least {needed} reviewers in one batch with councilLens."), false));
    }
    if reconfigured(team, round) {
        return Some((format!("Council round {round} was closed by a composition or reviewer selection change. Dispatch a fresh round with the new frozen plan and councilLens."), false));
    }
    if let Some(reason) = composition_source_coverage_error(team, config, round) {
        return Some((format!("council_source_quorum_blocked: {reason}"), false));
    }
    if !view.complete(round) {
        let missing = view.missing(team, round);
        let count = scored(team, round).count();
        let lenses = if missing.is_empty() { String::new() } else { format!(" It still needs the lenses round 1 seated: {}.", missing.join(", ")) };
        let stopped: Vec<String> = in_round(team, round).filter(|(worker, seat)| !worker.abandoned && seat.score.is_none() && !unsettled(&worker.status)).map(|(worker, _)| format!("{} ({})", worker.name, worker.id)).collect();
        let next = if stopped.is_empty() { "Dispatch the missing reviewers with councilLens, or wait for those still running.".to_string() } else { format!("Cancel the stopped reviewers {}, then dispatch the missing reviewers with councilLens.", stopped.join(", ")) };
        return Some((format!("Council round {round} is not complete: {count} scored, at least {needed} needed.{lenses} {next}"), false));
    }
    if stale(team, round) {
        return Some((format!("Council round {round} is out of date: changes landed after it started. Dispatch a new round with councilLens."), false));
    }
    let unverified = unverified_required_skill_checks(team, round);
    if !unverified.is_empty() {
        return Some((format!(
            "reviewer_skill_evidence_unverified: Council round {round} has missing mandatory skill input or explicitly required native skill checks without independently verified execution: {}. Native invocation, check and evidence receipts are required; reviewer claims or scores alone do not count.",
            unverified.join(", ")
        ), false));
    }
    let escalations = s2_escalation_signals(team, round, outcomes, findings);
    if let Some(signal) = escalations.first() {
        return Some((format!(
            "council_s2_escalation_required: S2 trigger {:?} from seat {} at target {}/10. Dispatch a fresh Council round using only the sources already enabled by this mode; the current review cannot approve the task.",
            signal.trigger, signal.seat_id, config.target
        ), false));
    }
    if let Err(error) = required_criteria_gate(team, config, round, outcomes) {
        return Some((format!("council_criteria_gate_blocked: {error}"), false));
    }
    let low = lowest(team, round)?;
    if low >= config.target {
        match config
            .composition
            .as_ref()
            .map(|composition| composition.strategy)
            .unwrap_or(CouncilStrategy::Parallel)
        {
            CouncilStrategy::Parallel => return None,
            CouncilStrategy::Escalated => return None,
            CouncilStrategy::TwoPhase if round == 1 => return Some((
                "council_phase2_required: phase 1 passed. Dispatch fresh read-only Council challengers with the same frozen lenses and artifact; phase 1 alone cannot approve the task.".into(),
                false,
            )),
            CouncilStrategy::TwoPhase => {
                if !two_phase_phase1_passes(team, config, view, outcomes)
                    || !two_phase_independent(team, 1, round)
                {
                    return Some((
                        "council_phase2_blocked: the phase-1 artifact changed or a fresh independent challenger is unavailable for a challenged lens.".into(),
                        false,
                    ));
                }
                return None;
            }
        }
    }
    Some((format!("Council round {round}: lowest {low}, target {}. Fix the blockers with workers, then dispatch a fresh round with councilLens. {ESCAPE}", config.target), true))
}

/// One deadline is shared by every root and child strategy in the composition.
pub(super) fn composition_deadline_at(team: &TeamSnapshot) -> Option<i64> {
    let composition = team.config.council.as_ref()?.composition.as_ref()?;
    (team.council_since > 0).then(|| {
        team.council_since.saturating_add(composition.limits.timeout_ms.min(i64::MAX as u64) as i64)
    })
}

pub(super) fn composition_deadline_exceeded(team: &TeamSnapshot, now_ms: i64) -> bool {
    composition_deadline_at(team).is_some_and(|deadline| now_ms >= deadline)
}

fn required_criteria_gate(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    round: usize,
    criterion_outcomes: &[CouncilCriterionOutcomeReceipt],
) -> std::result::Result<(), CouncilCompositionError> {
    let Some(composition) = config.composition.as_ref() else { return Ok(()) };
    if composition.criteria_source == CouncilCriteriaSource::Architect && composition.criteria.is_empty() {
        return Err(CouncilCompositionError::InvalidCriteria("Architect criteria were not frozen before review dispatch"));
    }
    let root_cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let mut per_seat = Vec::new();
    for (worker, seat) in in_round(team, round).filter(|(worker, seat)| counts(worker, seat)) {
        let Some(binding) = worker.reviewer_binding.as_ref() else {
            per_seat.push(Vec::new());
            continue;
        };
        let direct_preliminary = criterion_outcomes.iter().any(|receipt| {
            receipt.root_cycle_id == root_cycle_id && receipt.task_id == team.session_id
                && receipt.seat_id == worker.id && receipt.parent_seat_id.is_none()
                && receipt.round == seat.round && receipt.artifact_revision == binding.artifact_revision
        });
        let has_child_outcomes = criterion_outcomes.iter().any(|receipt| {
            receipt.root_cycle_id == root_cycle_id && receipt.parent_seat_id.as_deref() == Some(worker.id.as_str())
                && receipt.round == seat.round && receipt.artifact_revision == binding.artifact_revision
        });
        let on_trigger_preliminary = binding.composition_mode.as_deref() == Some("nested")
            && direct_preliminary && !has_child_outcomes
            && binding.reviewer_bot_id.as_deref().and_then(|bot_id| composition.selected.iter().find(|selected| selected.bot_id == bot_id))
                .and_then(|selected| selected.nested.as_ref())
                .is_some_and(|nested| nested.escalation == NestedEscalation::OnTrigger);
        let nested = binding.composition_mode.as_deref() == Some("nested") && !on_trigger_preliminary;
        let mut outcomes = std::collections::BTreeMap::<String, CouncilCriterionOutcome>::new();
        for receipt in criterion_outcomes {
            if receipt.root_cycle_id != root_cycle_id
                || receipt.artifact_revision != binding.artifact_revision
            {
                continue;
            }
            let correlated = if nested {
                // Native ledger projection admits child receipts only through a validated ownership link.
                receipt.task_id != team.session_id
                    && receipt.task_cycle_id != root_cycle_id
                    && receipt.child_cycle_id.as_deref() == Some(receipt.task_cycle_id.as_str())
                    && receipt.parent_seat_id.as_deref() == Some(worker.id.as_str())
            } else {
                receipt.validates_for(
                    &root_cycle_id,
                    &team.session_id,
                    &root_cycle_id,
                    seat.round,
                    &worker.id,
                    None,
                    worker.attempt,
                    &binding.artifact_revision,
                )
            };
            if !correlated || receipt.validate().is_err() {
                continue;
            }
            let outcome = outcomes
                .entry(receipt.outcome.criterion_id.clone())
                .or_insert_with(|| receipt.outcome.clone());
            if nested && outcome.criterion_id == receipt.outcome.criterion_id {
                outcome.score = outcome.score.min(receipt.outcome.score);
                for evidence in &receipt.outcome.verified_evidence_refs {
                    if !outcome.verified_evidence_refs.contains(evidence) {
                        outcome.verified_evidence_refs.push(evidence.clone());
                    }
                }
            }
        }
        let mut outcomes = outcomes.into_values().collect::<Vec<_>>();
        outcomes.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        per_seat.push(outcomes);
    }
    composition.validate_required_criteria_gate(config.target, &per_seat)
}

fn composition_source_coverage_error(team: &TeamSnapshot, config: &CouncilConfig, round: usize) -> Option<String> {
    let composition = config.composition.as_ref()?;
    let mut selected_seen = HashSet::<String>::new();
    let mut automatic = 0usize;
    let mut duplicate_selected = false;
    for (worker, _) in in_round(team, round).filter(|(worker, seat)| counts(worker, seat)) {
        let Some(binding) = worker.reviewer_binding.as_ref() else { continue };
        if let Some(bot_id) = binding.reviewer_bot_id.as_deref() {
            let selected = composition.selected.iter().find(|candidate| candidate.bot_id == bot_id);
            let expected_mode = selected.map(|selected| match selected.mode {
                SelectedReviewerMode::Direct => "direct",
                SelectedReviewerMode::Nested => "nested",
            });
            if expected_mode == binding.composition_mode.as_deref() {
                duplicate_selected |= !selected_seen.insert(bot_id.to_owned());
            }
        } else if composition.automatic.enabled
            && binding.composition_mode.as_deref().is_none_or(|mode| mode == "direct")
        {
            automatic = automatic.saturating_add(1);
        }
    }
    let missing = composition.selected.iter()
        .filter(|selected| selected.required && !selected_seen.contains(&selected.bot_id))
        .map(|selected| selected.bot_id.as_str())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Some(format!("required selected reviewer seats are missing: {}.", missing.join(", ")));
    }
    if duplicate_selected {
        return Some("a selected reviewer identity occupies more than one required seat.".into());
    }
    let automatic_valid = match composition.automatic.count {
        AutomaticReviewerCount::Fixed(expected) => automatic == expected,
        AutomaticReviewerCount::Auto(_) => {
            (composition.minimum_automatic_seats()..=composition.maximum_automatic_seats()).contains(&automatic)
        }
    };
    if !automatic_valid {
        return Some(match composition.automatic.count {
            AutomaticReviewerCount::Fixed(expected) => format!("the frozen plan requires exactly {expected} automatic reviewer seats, but round {round} has {automatic}."),
            AutomaticReviewerCount::Auto(_) => format!("round {round} needs between {} and {} automatic reviewer seats, but has {automatic}.", composition.minimum_automatic_seats(), composition.maximum_automatic_seats()),
        });
    }
    None
}

pub(super) fn two_phase_phase1_ready(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> bool {
    if config.composition.is_none() { return false; }
    composition_source_coverage_error(team, config, 1).is_none()
        && two_phase_phase1_passes(team, config, &Rounds::of(team, config), outcomes)
        && findings.iter().all(|finding| finding.validate().is_ok())
}

/// Return the first issue-defined trigger for one completed direct preliminary
/// review. Inputs are already filtered to the current native ledger projection.
pub(super) fn nested_preliminary_trigger(
    team: &TeamSnapshot,
    worker: &Worker,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
    user_forced: bool,
) -> Option<NestedEscalationTrigger> {
    let (Some(binding), Some(seat), Some(config)) = (
        worker.reviewer_binding.as_ref(),
        worker.council.as_ref(),
        team.config.council.as_ref(),
    ) else { return Some(NestedEscalationTrigger::MissingMandatoryEvidence) };
    if user_forced {
        return Some(NestedEscalationTrigger::UserForced);
    }
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let round_id = format!("{cycle_id}:{}", seat.round);
    let digest = super::council_artifact_digest(&binding.artifact_revision);
    let own_findings = findings.iter().filter(|finding| {
        finding.cycle_id == cycle_id && finding.round_id == round_id
            && finding.seat_id == worker.id && finding.artifact_digest == digest
            && finding.validate().is_ok()
    });
    if own_findings.clone().any(|finding| finding.severity == CouncilFindingSeverity::Critical) {
        return Some(NestedEscalationTrigger::CriticalFinding);
    }
    if own_findings.clone().any(|finding| finding.severity == CouncilFindingSeverity::High) {
        return Some(NestedEscalationTrigger::HighFinding);
    }
    let Some(composition) = config.composition.as_ref() else {
        return Some(NestedEscalationTrigger::MissingMandatoryEvidence);
    };
    let required = composition.criteria.iter().filter(|criterion| criterion.required).collect::<Vec<_>>();
    if required.is_empty() {
        return Some(NestedEscalationTrigger::MissingMandatoryEvidence);
    }
    let mut own = std::collections::BTreeMap::new();
    for receipt in outcomes.iter().filter(|receipt| receipt.validate().is_ok()) {
        if receipt.validates_for(
            &cycle_id, &team.session_id, &cycle_id, seat.round, &worker.id, None,
            worker.attempt, &binding.artifact_revision,
        ) {
            own.insert(receipt.outcome.criterion_id.as_str(), &receipt.outcome);
        }
    }
    for criterion in &required {
        let Some(outcome) = own.get(criterion.id.as_str()) else {
            return Some(NestedEscalationTrigger::MissingMandatoryEvidence);
        };
        if criterion.evidence_required && outcome.verified_evidence_refs.is_empty() {
            return Some(NestedEscalationTrigger::MissingMandatoryEvidence);
        }
        if outcome.score < criterion.target {
            return Some(NestedEscalationTrigger::BelowTarget);
        }
    }
    for criterion in &required {
        let Some(own_score) = own.get(criterion.id.as_str()).map(|outcome| outcome.score) else { continue };
        let own_pass = own_score >= criterion.target;
        if outcomes.iter().any(|receipt| {
            receipt.validate().is_ok()
                && receipt.root_cycle_id == cycle_id
                && receipt.round == seat.round
                && receipt.artifact_revision == binding.artifact_revision
                && receipt.outcome.criterion_id == criterion.id
                && receipt.parent_seat_id.as_deref().unwrap_or(&receipt.seat_id) != worker.id
                && (receipt.outcome.score >= criterion.target) != own_pass
        }) {
            return Some(NestedEscalationTrigger::SameCriterionConflict);
        }
    }
    if seat.score.is_some_and(|score| score < config.target) {
        return Some(NestedEscalationTrigger::BelowTarget);
    }
    None
}

/// S2 escalates direct/automatic seats from the source set frozen for this
/// round. Nested seats own their on-trigger child decision and are handled by
/// the nested lifecycle reconciler.
pub(super) fn s2_escalation_signals(
    team: &TeamSnapshot,
    round: usize,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Vec<S2EscalationSignal> {
    let Some(composition) = team.config.council.as_ref().and_then(|config| config.composition.as_ref()) else {
        return Vec::new();
    };
    if composition.strategy != CouncilStrategy::Escalated {
        return Vec::new();
    }
    let mut signals = in_round(team, round)
        .filter(|(worker, seat)| counts(worker, seat))
        .filter(|(worker, _)| {
            worker.reviewer_binding.as_ref()
                .and_then(|binding| binding.composition_mode.as_deref()) != Some("nested")
        })
        .filter_map(|(worker, seat)| {
            let trigger = nested_preliminary_trigger(team, worker, outcomes, findings, false)?;
            let artifact_revision = worker.reviewer_binding.as_ref()?.artifact_revision.clone();
            Some(S2EscalationSignal {
                seat_id: worker.id.clone(),
                round: seat.round,
                attempt: worker.attempt,
                artifact_revision,
                trigger,
            })
        })
        .collect::<Vec<_>>();
    signals.sort_by(|left, right| left.seat_id.cmp(&right.seat_id));
    signals
}

fn reviewer_identity(worker: &Worker) -> String {
    worker.reviewer_binding.as_ref().map_or_else(
        || format!("{}:{}", worker.provider, worker.model),
        |binding| binding.reviewer_bot_id.clone().unwrap_or_else(|| {
            format!("{}:{}", binding.snapshot.provider, binding.snapshot.model)
        }),
    )
}

pub(super) fn same_artifact_revision(left: &str, right: &str) -> bool {
    let mut left = left.split(':');
    let mut right = right.split(':');
    matches!((left.next(), right.next()), (Some(a), Some(b)) if a == b)
        && matches!((left.next(), right.next()), (Some(a), Some(b)) if a == b)
}

fn two_phase_independent(team: &TeamSnapshot, phase1: usize, phase2: usize) -> bool {
    let earlier: Vec<_> = in_round(team, phase1)
        .filter(|(worker, seat)| counts(worker, seat))
        .collect();
    let later: Vec<_> = in_round(team, phase2)
        .filter(|(worker, seat)| counts(worker, seat))
        .collect();
    !earlier.is_empty()
        && later.len() == earlier.len()
        && !stale(team, phase1)
        && !stale(team, phase2)
        && independent_scored_sessions(team, phase1)
        && independent_scored_sessions(team, phase2)
        && later.iter().enumerate().all(|(index, (challenger, _))| {
            later.iter().skip(index + 1).all(|(other, _)| reviewer_identity(challenger) != reviewer_identity(other))
        })
        && later.iter().all(|(challenger, challenge_seat)| {
            earlier.iter().all(|(author, _)| {
                author.id != challenger.id
                    && author.session_id != challenger.session_id
                    && reviewer_identity(author) != reviewer_identity(challenger)
            })
                && earlier.iter().any(|(author, author_seat)| {
                    same_lens(&author_seat.lens, &challenge_seat.lens)
                        && author.reviewer_binding.as_ref().zip(challenger.reviewer_binding.as_ref())
                            .is_some_and(|(author, challenger)| {
                                // Reviewer authority revisions are seat-specific. Shared root cycle,
                                // delivery and unchanged artifact establish the same challenge target.
                                author.council_cycle_id == challenger.council_cycle_id
                                    && author.delivery_id == challenger.delivery_id
                                    && same_artifact_revision(&author.artifact_revision, &challenger.artifact_revision)
                            })
                })
        })
}

fn two_phase_phase1_passes(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    view: &Rounds,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> bool {
    view.complete(1)
        && !stale(team, 1)
        && unverified_required_skill_checks(team, 1).is_empty()
        && required_criteria_gate(team, config, 1, outcomes).is_ok()
        && lowest(team, 1).is_some_and(|score| score >= config.target)
}

fn strategy_terminal_pass(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    view: &Rounds,
    round: usize,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> bool {
    let Some(composition) = config.composition.as_ref() else { return true };
    match composition.strategy {
        CouncilStrategy::Parallel => true,
        CouncilStrategy::Escalated => s2_escalation_signals(team, round, outcomes, findings).is_empty(),
        CouncilStrategy::TwoPhase => round >= 2
            && two_phase_phase1_passes(team, config, view, outcomes)
            && two_phase_independent(team, 1, round),
    }
}

pub(super) fn unreviewed_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> Option<String> {
    finish_refusal_with_criterion_outcomes(team, outcomes).filter(|_| has_writer(team))
}
pub(super) fn unreviewed_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<String> {
    finish_refusal_with_ledger(team, outcomes, findings).filter(|_| has_writer(team))
}

/// The snapshot's `councilStatus`, from the same checks as the gate.
pub(super) fn status(team: &TeamSnapshot) -> Option<CouncilStatus> {
    status_with_criterion_outcomes(team, &[])
}
pub(super) fn status_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> Option<CouncilStatus> {
    status_with_ledger(team, outcomes, &[])
}
pub(super) fn status_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<CouncilStatus> {
    let config = team.config.council.as_ref()?;
    Some(status_in(team, config, &Rounds::of(team, config), outcomes, findings))
}
fn status_in(
    team: &TeamSnapshot,
    config: &CouncilConfig,
    view: &Rounds,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> CouncilStatus {
    let round = view.latest;
    let low = lowest(team, round);
    let stale = stale(team, round);
    let verdict = verdict_in(team, config, view, outcomes, findings);
    // A refusal the team is resting or paused on, as its error.
    let held = verdict.as_ref().filter(|(refusal, _)| team.error.as_deref().is_some_and(|e| e.contains(refusal.as_str())));
    let asking = held.is_some_and(|(_, asks)| *asks && team.status == "ready");
    let state = if accepted(team) {
        "accepted"
    } else if round == 0 {
        "waiting"
    } else if asking {
        "asking"
    } else if live(team, round) {
        "reviewing"
    } else if config.composition.as_ref().is_some_and(|composition| {
        composition.strategy == CouncilStrategy::TwoPhase
            && view.complete(round)
            && round == 1
            && two_phase_phase1_passes(team, config, view, outcomes)
    }) {
        "dispatch"
    } else if config.composition.as_ref().is_some_and(|composition| {
        composition.strategy == CouncilStrategy::Escalated
            && view.complete(round)
            && !s2_escalation_signals(team, round, outcomes, findings).is_empty()
    }) {
        "dispatch"
    } else if config.composition.as_ref().is_some_and(|composition| {
        composition.strategy == CouncilStrategy::TwoPhase
            && view.complete(round)
            && round >= 2
            && low.is_some_and(|low| low >= config.target)
            && !strategy_terminal_pass(team, config, view, round, outcomes, findings)
    }) {
        "blocked"
    } else if view.complete(round) && !stale && unverified_required_skill_checks(team, round).is_empty()
        && required_criteria_gate(team, config, round, outcomes).is_ok()
        && config.validate_composition().is_ok()
        && strategy_terminal_pass(team, config, view, round, outcomes, findings)
        && low.is_some_and(|low| low >= config.target)
    {
        "passed"
    } else if view.at_limit(config) {
        "limit"
    } else if !view.complete(round) {
        "dispatch"
    } else {
        "fixing"
    };
    let (low_shown, target) = (low.unwrap_or_default(), config.target);
    let notice = held.map(|_| match () {
        _ if asking => format!("The lead is asking how to continue: lowest score {low_shown:.1}, target {target:.1}, after round {round}."),
        _ if round == 0 => "The team stopped before Council reviewed the work.".to_string(),
        _ if !view.complete(round) => format!("The team stopped with Council round {round} unfinished."),
        _ if stale => format!("The team stopped: the work changed after Council round {round}."),
        _ => format!("The team stopped after Council round {round}: lowest score {low_shown:.1}, target {target:.1}."),
    });
    CouncilStatus { state: state.into(), round, lowest: low, target, stale, notice }
}

/// Each lens's first score this cycle, for the lead's first-and-final table.
fn score_table(team: &TeamSnapshot) -> Vec<(String, f64, f64)> {
    let mut seats: Vec<_> = seats(team).filter(|(w, seat)| counts(w, seat)).filter_map(|(_, seat)| Some((seat.round, &seat.lens, seat.score?))).collect();
    seats.sort_by_key(|(round, _, _)| *round);
    let mut table: Vec<(String, f64, f64)> = Vec::new();
    for (_, lens, score) in seats {
        match table.iter_mut().find(|(known, _, _)| same_lens(known, lens)) {
            Some(row) => row.2 = score,
            None => table.push((lens.clone(), score, score)),
        }
    }
    table
}
fn first_scores(team: &TeamSnapshot) -> serde_json::Map<String, Value> {
    score_table(team).into_iter().map(|(lens, first, _)| (lens, json!(first))).collect()
}

/// The `council` object in the lead's review, when Council is on.
pub(super) fn review(team: &TeamSnapshot) -> Option<Value> {
    review_with_criterion_outcomes(team, &[])
}
pub(super) fn review_with_criterion_outcomes(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
) -> Option<Value> {
    review_with_ledger(team, outcomes, &[])
}
pub(super) fn review_with_ledger(
    team: &TeamSnapshot,
    outcomes: &[CouncilCriterionOutcomeReceipt],
    findings: &[CouncilFinding],
) -> Option<Value> {
    let config = team.config.council.as_ref()?;
    let view = Rounds::of(team, config);
    let status = if outcomes.is_empty() {
        team.council_status
            .clone()
            .unwrap_or_else(|| status_in(team, config, &view, outcomes, findings))
    } else {
        status_in(team, config, &view, outcomes, findings)
    };
    let round = status.round;
    let done = view.complete(round);
    let passed = status.state == "passed";
    let next = if passed {
        "Every reviewer reached the target: verify, then finish with the table of first and final scores.".into()
    } else {
        match verdict_in(team, config, &view, outcomes, findings) {
            Some((refusal, _)) => refusal,
            None if view.at_limit(config) => "The round limit is reached: make no more changes, finish, and say plainly which scores fell short and what no round reviewed.".into(),
            None => "No worker has changed anything this cycle, so no council is needed yet.".to_string(),
        }
    };
    let rounds_left = config.max_rounds.saturating_sub(view.started);
    let scores: Vec<Value> = in_round(team, round).filter(|(w, seat)| counts(w, seat)).map(|(_, seat)| json!({"lens":seat.lens,"score":seat.score})).collect();
    Some(json!({"round":round,"target":config.target,"lowest":status.lowest,"complete":done,"passed":passed,"roundsLeft":rounds_left,"scores":scores,"firstScores":first_scores(team),"next":next}))
}

/// The newest time any team conversation recorded changed files this cycle: worker turns, and
/// reviewers' only for real writes (`lastWriteAt`: a write tool or track_files), since a reviewer that
/// edits the work outdates its round but one rendering with shell commands must not,
/// and lead turns by the files the lead changed itself (`ownFiles`, saved before its workers' changes
/// are rolled up; a rolled-up row from before that marker counts only when it had none). One query:
/// the session index finds each team conversation's rows, and only this cycle's are read. A change is
/// dated by its turn's last write tool (`lastWriteAt`), else by when its turn ended.
pub(super) fn changed_at(store: &storage::Store, team: &TeamSnapshot) -> Result<i64> {
    if team.config.council.is_none() {
        return Ok(0);
    }
    let reviewer = |id: &str| team.workers.iter().any(|w| w.id == id && w.council.is_some());
    let reviewers: Vec<&str> = team.workers.iter().filter(|w| w.council.is_some() || w.parent_worker.as_deref().is_some_and(reviewer)).map(|w| w.session_id.as_str()).collect();
    let reviewers = serde_json::to_string(&reviewers).map_err(|e| e.to_string())?;
    // This cycle's rows only: message ids grow with time, so every row of the cycle has a message id
    // above the lead's last message from before it. `messages_session` finds that id by walking back
    // through this cycle's lead messages, and `turn_changes_session` then range-scans each session.
    let before: i64 = store.db.query_row("SELECT COALESCE((SELECT id FROM messages WHERE session_id=?1 AND created_at<?2 ORDER BY id DESC LIMIT 1),0)", params![team.session_id, team.council_since], |r| r.get(0)).map_err(|e| e.to_string())?;
    store.db.query_row("SELECT COALESCE(MAX(COALESCE(json_extract(summary,'$.lastWriteAt'),created_at)),0) FROM turn_changes WHERE (session_id=?2 OR session_id IN (SELECT session_id FROM architect_workers WHERE parent_session_id=?2)) AND (session_id NOT IN (SELECT value FROM json_each(?3)) OR json_extract(summary,'$.lastWriteAt') IS NOT NULL) AND message_id>?4 AND created_at>=?1 AND json_valid(summary) AND json_extract(summary,'$.attributed')=1 AND COALESCE(json_extract(summary,'$.ownFiles'),CASE WHEN json_type(summary,'$.teamSources') IS NULL THEN json_array_length(summary,'$.files') END,0)>0", params![team.council_since, team.session_id, reviewers, before], |r| r.get(0)).map_err(|e| e.to_string())
}

/// The lead's Council rules: config values only, because any change here resends the full rules.
pub(super) fn lead_rules(team: &TeamSnapshot, routed: bool) -> String {
    let Some(config) = team.config.council.as_ref() else {
        return String::new();
    };
    let selected = !config.reviewer_bot_ids.is_empty();
    let plan = config.seat_plan();
    let count = if config.composition.is_some() {
        match (plan.selected_bots(), plan.automatic) {
            (0, automatic) => format!("exactly {automatic} automatic reviewer seats"),
            (selected, 0) => format!("exactly {selected} selected bot reviewer seats"),
            (selected, automatic) => format!("exactly {} reviewer seats: {selected} selected bot seats plus {automatic} automatic seats", plan.required()),
        }
    } else if selected {
        format!("exactly {} selected configured bots", config.reviewer_bot_ids.len())
    } else {
        match config.reviewers {
            0 => format!("at least {MIN_REVIEWERS}"),
            n => format!("exactly {n}"),
        }
    };
    let models: std::collections::HashSet<_> = team.config.workers.iter().map(|p| (&p.provider, &p.model)).collect();
    let models = if config.composition.is_some() && plan.automatic > 0 && selected {
        if routed || models.len() < 2 { "frozen selected bots plus profileId \"auto\" for automatic seats" }
        else { "frozen selected bots plus a different roster model for each automatic seat" }
    } else if selected { "native selected-bot seats" } else if routed || models.len() < 2 { "profileId \"auto\"" } else { "a different roster model for each reviewer, since one model is a weak council" };
    let limit = format!("Never finish below the target unless the limit of {} rounds is reached; after the last round make no more changes, and say so plainly.", config.max_rounds);
    let selected_note = if config.composition.is_some() && selected && plan.automatic > 0 {
        format!(" The user selected persistent reviewer bot IDs: {}. Assign one frozen selected-bot seat per selected ID, then fill the remaining {} required automatic seats with independent native reviewers. Set councilSource=selected_bot and reviewerBotId to the exact selected ID on each bot seat; set councilSource=automatic on every automatic seat. Do not substitute an automatic reviewer for a selected bot or count a selected bot toward the automatic quota. Selected bots keep their frozen model and review persona; provider/model/profileId fields cannot override those selections.", config.reviewer_bot_ids.join(", "), plan.automatic)
    } else if selected {
        format!(" The user selected persistent reviewer bot IDs: {}. Native Council assigns each fresh reviewer seat to one of those bots, using that bot's frozen model and review persona. Dispatch one unique Council lens per bot; never substitute a worker or claim unavailable bots reviewed. Disclose reduced diversity if distinct bots use the same provider/model. Provider/model/profileId fields in reviewer assignments cannot override the selected bot.", config.reviewer_bot_ids.join(", "))
    } else { String::new() };
    // Reconcile the #149 native source-quota guidance with this Council's
    // existing three-source seat_plan; no second dispatch engine is needed.
    let nested_note = if config.composition.is_some() && plan.nested_bots > 0 {
        format!(" Of the {} selected bot seats, {} are task-owned child Councils and {} are direct reviewers; {} additional automatic root reviewers are separately required. Each selected nested bot occupies exactly one root seat, no matter how many native read-only leaf reviewers it coordinates. Dispatch the selected bot's root seat using its frozen bot ID and the existing native Council assignment path, then let its isolated child Architect run its own Council. Never dispatch child leaves as extra root votes. Do not finish or accept a claimed child report while its matching artifact-bound native child terminal receipt is missing, stale or unresolved. For OnTrigger escalation, preserve the direct preliminary attempt and issue a separate child task only when the native trigger is recorded. All selected/direct/automatic quotas and permitted reviewer sources are mandatory across the configured strategy.", plan.selected_bots(), plan.nested_bots, plan.direct_bots, plan.automatic)
    } else { String::new() };
    format!(" Council is on, target {}/10. Before finishing work that changed anything, convene a council: you decide the reviewers, choosing distinct lenses that fit this work (correctness, security, UX, performance, visual quality, pacing, overall senior review, whatever matters here; round 1 fixes the lenses: each sits again in every later round under its name, and no new lens joins later), {count}, dispatched in one batch with councilLens and {models}, after writers settle. Give each reviewer the user's goal, where the work is and its lens; never prior scores, and never the score you want. The bar is every reviewer at or above the target, not the average. After a round below it, read the reports, fix real blockers through workers, verify, and convene FRESH reviewers. {limit} If the user says to accept the current scores, tell them to press Accept and finish on the Council card. End your final message with a table of each lens's first and final score.{selected_note}{nested_note}", config.target)
}

/// A reviewer's rules. They never carry the target or earlier scores.
pub(super) fn reviewer_rules(worker: &Worker) -> String {
    let Some(seat) = &worker.council else {
        return String::new();
    };
    format!(" You are an independent Council reviewer; your lens is {}. Hunt real defects and verify each by inspecting the work itself (code, output, frames, renders) or running checks; skip nitpicks that do not affect the result. Say what would raise the score. Score strictly: 10 means you found nothing worth improving; deduct for each real defect, in proportion to its impact. End your final report with the exact line `Council score: N/10`.", seat.lens)
}

/// The read-only rule for a reviewer: it reviews the work its assignment names, and its findings go
/// back as its report, not as a request for a writing follow-up.
pub(super) fn reviewer_access(
    team: &TeamSnapshot,
    worker: &Worker,
    challenge_findings: &[CouncilFinding],
) -> String {
    let scope = if worker.files.is_empty() { "the work your assignment names".into() } else { worker.files.join(", ") };
    let criteria = team.config.council.as_ref().and_then(|config| config.composition.as_ref())
        .map(|composition| serde_json::to_string(&composition.criteria).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|| "[]".into());
    let base = format!("This is a read-only review. Do not change the work under review: do not edit, create, or delete files, or run commands that change the checkout; put any renders or builds outside it, such as in a temporary directory. Inspect {scope} and report findings; do not fix them. Concurrent workers may change files while you inspect them. The frozen native criteria are {criteria}. For every required criterion, call pandacode_council_record_finding with its exact criterionId, score and verified evidenceFiles; the review cannot count without complete pinned outcomes and evidence.");
    let challenge = team.config.council.as_ref()
        .and_then(|config| config.composition.as_ref())
        .is_some_and(|composition| composition.strategy == CouncilStrategy::TwoPhase)
        && worker.council.as_ref().is_some_and(|seat| seat.round == 2);
    if !challenge {
        return base;
    }
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let phase1 = challenge_findings.iter()
        .filter(|finding| finding.cycle_id == cycle_id && finding.round_id == format!("{cycle_id}:1"))
        .map(|finding| {
            let author = team.workers.iter().find(|candidate| candidate.id == finding.seat_id);
            let identity = author.and_then(|candidate| candidate.reviewer_binding.as_ref())
                .map(|binding| binding.reviewer_bot_id.clone().unwrap_or_else(|| format!("{}:{}", binding.snapshot.provider, binding.snapshot.model)))
                .unwrap_or_else(|| finding.seat_id.clone());
            json!({
                "criterionId":finding.criterion_id,
                "severity":finding.severity,
                "category":finding.category,
                "summary":finding.summary,
                "evidenceRefs":finding.evidence_refs,
                "author":identity,
                "artifactDigest":finding.artifact_digest
            })
        })
        .collect::<Vec<_>>();
    let artifact = worker.reviewer_binding.as_ref()
        .map(|binding| binding.artifact_revision.as_str())
        .unwrap_or("unavailable");
    format!("{base} This is a fresh phase-2 challenge against the frozen phase-1 artifact revision {artifact}. Independently check the artifact and each phase-1 finding below for contradictions, omissions, and missing evidence. Do not rewrite, close, or approve a phase-1 finding on behalf of its author; report your own evidence and score. Your reviewer identity must differ from the author of every challenged report. Phase-1 findings: {}", serde_json::to_string(&phase1).unwrap_or_else(|_| "[]".into()))
}

/// A live or stopped run's edits may have landed until now. A run that ended was marked when it did.
pub(super) fn note_stopped(worker: &mut Worker) {
    // Only a live run may have written just now; a parked or stopped one was marked when its run ended
    // or was stopped.
    if worker.wrote_at > 0 && !worker.read_only && !worker.abandoned && super::active(&worker.status) {
        worker.wrote_at = storage::now();
    }
}
