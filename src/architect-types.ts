import type { ProviderId } from './types';

export interface WorkerProfile {
  id: string;
  name: string;
  provider: ProviderId;
  model: string;
  reasoningEffort?: string | null;
}
/** What a council must reach: every reviewer at `target`, within `maxRounds` completed rounds (1-100), with `reviewers` seats (0 = the lead decides). */
export interface CouncilReviewGrant { tool: string; action: string; resourceArgument: string; resource: string; actionArgument?: string | null }
export interface CouncilPermissionSelection { authorizationId: string; grants: CouncilReviewGrant[] }
/** Native's untagged count is either a fixed number or the string "auto". */
export type ReviewerCount = number | 'auto';
export interface CouncilAutomatic { enabled: boolean; count: ReviewerCount }
export type CouncilReviewerMode = 'direct' | 'nested';
export type CouncilCriteriaSource = 'architect' | 'user' | 'hybrid';
export type CouncilCriterionOrigin = 'user' | 'architect';
export type CouncilStrategy = 'parallel' | 'escalated' | 'two_phase';
export type NestedCouncilTemplate = 'explicit_task' | 'validated_default';
export type NestedCouncilEscalation = 'always' | 'on_trigger';
export interface NestedCouncilConfig {
  template: NestedCouncilTemplate;
  childCriteriaSource: CouncilCriteriaSource;
  childReviewerCount: number;
  target: number;
  maxRounds: number;
  escalation: NestedCouncilEscalation;
}
export interface SelectedCouncilReviewer { botId: string; mode: CouncilReviewerMode; required: true; nested?: NestedCouncilConfig | null }
export interface CouncilCriterion {
  id: string; lens: string; description: string; required: boolean; target: number;
  evidenceRequired: boolean; source: CouncilCriterionOrigin;
}
export interface CouncilLimits {
  maxDepth: number; maxRootSeats: number; maxConcurrentLeaves: number;
  maxLeafReviewsPerRound: number; timeoutMs: number; maxEstimatedCostMicros: number;
}
export interface CouncilComposition {
  schemaVersion: 1; automatic: CouncilAutomatic; selected: SelectedCouncilReviewer[];
  criteriaSource: CouncilCriteriaSource; criteria: CouncilCriterion[];
  strategy: CouncilStrategy; limits: CouncilLimits;
}
export type CouncilFindingSeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';
export type CouncilFindingLocationStatus = 'verified' | 'missing' | 'unverified';
export interface CouncilFindingLocation { file: string; line?: number | null }
/** Native verified record; suggestedFix is untrusted display text. */
export interface CouncilFinding {
  id: string; cycleId: string; roundId: string; seatId: string; artifactDigest: string;
  criterionId: string; severity: CouncilFindingSeverity; category: string; summary: string;
  evidenceRefs: string[]; locationStatus: CouncilFindingLocationStatus;
  location?: CouncilFindingLocation | null; suggestedFix?: string | null;
}
export type CouncilFindingDispositionKind = 'open' | 'addressed' | 'disputed' | 'not_reproduced' | 'waived_by_authorized_human';
export interface CouncilFindingDisposition {
  findingId: string; disposition: CouncilFindingDispositionKind; actorId: string;
  rationale: string; timestampMs: number; artifactLinks: string[];
}
export type ChildCouncilStatus = 'passed' | 'accepted' | 'fixing' | 'limit' | 'stopped' | 'failed' | 'blocked' | 'inconclusive' | 'stale';
export interface ChildCouncilReceipt {
  rootCycleId: string; parentSeatId: string; childCycleId: string; botId: string;
  artifactRevision: string; policyFingerprint: string; status: ChildCouncilStatus;
  minScore?: number | null; leafScoreRefs: string[]; findingsRefs: string[];
  evidenceRefs: string[]; snapshotFingerprint: string;
}
/** Native-authorized child reviewer row; never an extra root vote. */
export interface CouncilChildLeafProjection {
  rootCycleId: string; parentSeatId: string; childCycleId: string;
  roundId: string; seatId: string; reviewerId: string; attempt: number;
  status: string; score?: number | null;
}
export interface CouncilCriterionOutcomeReceipt {
  schemaVersion: number; rootCycleId: string; taskId: string; taskCycleId: string;
  childCycleId?: string | null; round: number; seatId: string; parentSeatId?: string | null;
  attempt: number; artifactRevision: string;
  outcome: { criterionId: string; score: number; verifiedEvidenceRefs: string[] };
}
export type CouncilReviewVerdict = 'passed' | 'accepted' | 'failed' | 'limit' | 'blocked' | 'stopped' | 'inconclusive' | 'stale';
export interface CouncilReviewReceipt {
  schemaVersion: number; requestId: string; cycleId: string; taskId?: string | null;
  assignmentId?: string | null; policyFingerprint: string; artifactDigest: string;
  verdict: CouncilReviewVerdict; findingsRefs: string[]; evidenceRefs: string[];
  terminalRevision: number; idempotencyKey: string;
}
export type CouncilUsageValueStatus = 'measured' | 'provider_estimated' | 'calculated_estimated' | 'unknown';
export interface CouncilUsageValue<T> { status: CouncilUsageValueStatus; value?: T | null }
export interface CouncilCostEstimate { amountMicros: number; currency: string; pricingSource: string; pricingVersion: string; calculatedAtMs: number }
export interface CouncilResourceBudgetConsumption {
  concurrentLeaves: CouncilUsageValue<number>; leafReviews: CouncilUsageValue<number>;
  estimatedCostMicros: CouncilUsageValue<number>;
}
export type CouncilUsageScope = 'root' | 'child';
export type CouncilUsagePhase = 'review' | 'escalation' | 'challenge';
export interface CouncilUsageRecord {
  id: string; rootCycleId: string; childCycleId?: string | null; attemptId: string;
  reviewerId: string; roundId: string; seatId: string; scope: CouncilUsageScope;
  phase: CouncilUsagePhase; strategy: CouncilStrategy; startedAtMs?: number | null;
  finishedAtMs?: number | null; queueWaitMs: CouncilUsageValue<number>;
  elapsedMs: CouncilUsageValue<number>; retries: CouncilUsageValue<number>;
  escalations: CouncilUsageValue<number>; provider?: string | null; model?: string | null;
  harness?: string | null; inputTokens: CouncilUsageValue<number>;
  outputTokens: CouncilUsageValue<number>; requestCount: CouncilUsageValue<number>;
  terminalStatus: CouncilReviewVerdict; budgetConsumption: CouncilResourceBudgetConsumption;
  costEstimate?: CouncilCostEstimate | null;
}
export interface CouncilCriterionOutcome { criterionId: string; score: number; verifiedEvidenceRefs: string[] }
export interface CouncilConfig {
  target: number; maxRounds: number; reviewers: number;
  /** Task-scoped persistent reviewer identities; absent means automatic native workers. */
  reviewerBotIds?: string[]; optionalSkillIds?: string[]; requiredCheckSkillIds?: string[];
  reviewPermissions?: CouncilPermissionSelection | null;
  postAcceptanceIssue?: { authorizationId: string; repository: string } | null;
  /** Additive schema v1. Missing keeps the original M1/M2 behavior. */
  composition?: CouncilComposition | null;
}
/** A worker's seat on the council: the round it reviews, what it looks at, and its score once given. */
export interface CouncilSeat { round: number; lens: string; score?: number | null; /** First launch; a round begins with its earliest seat. */ startedAt?: number }
/** `dispatch`: the latest round is open but no seat is live, so it waits on the lead; `asking`: the team rests for the user. */
export type CouncilState = 'waiting' | 'reviewing' | 'dispatch' | 'fixing' | 'passed' | 'limit' | 'asking' | 'accepted' | 'blocked' | 'failed' | 'stopped' | 'inconclusive' | 'stale';
/** Native's own reading of the council, which also sees edits the webview cannot (the lead's own). `stale`: the latest round passed, then the work changed. */
/** `accepted`: the user accepted this cycle's result (Council stays on). `notice`: a plain sentence for the user whenever `team.error`
 *  holds a Council refusal written for the lead (asking or paused), which is then never shown; null otherwise. */
export interface CouncilStatus { state: CouncilState; round: number; lowest: number | null; target: number; stale: boolean; notice?: string | null }
export interface TeamConfig {
  /** The task's own model leads and picks each worker's model (from `workers`, a roster of
   * installed models, or as the prompt asks). Configured teams name fixed worker profiles. */
  auto?: boolean;
  /** Named agent teams (Settings → Agent teams): the lead may only orchestrate `workers`. */
  rosterOnly?: boolean;
  /** The agent team's name, shown in the model picker and composer. */
  name?: string;
  workers: WorkerProfile[];
  /** Every model of every installed harness, up to 1000, round-robin across harnesses. `workers` stays the short
   * roster shown to the lead; Panda Router picks worker models from this wider list. */
  catalog?: WorkerProfile[];
  /** Zero means unlimited; positive values preserve a user-selected limit. */
  maxParallel: number;
  /** Ignored natively, like `maxAssignments` and `summaryLimit`; kept so old saves still parse. */
  maxRounds: number;
  maxAssignments: number;
  workspace: 'shared';
  summaryLimit: number;
  /** Extra instructions for the lead, added to the built-in team rules. Empty means the
   * built-in rules alone. */
  leadGuidance?: string | null;
  /** Independent reviewers score the work until they all reach the target. Off on each new thread. */
  council?: CouncilConfig | null;
}
export type ModelSelection = Pick<WorkerProfile, 'provider' | 'model' | 'reasoningEffort'>;
export interface TeamSetup { architect: ModelSelection; config: TeamConfig }
export interface TeamProfile { id: string; name: string; setup: TeamSetup; updatedAt: number }
export interface TeamLibrary {
  enabled: boolean;
  lastSetup: TeamSetup | null;
  activeProfileId: string | null;
  profiles: TeamProfile[];
  /** Lead guidance for every team that does not set its own (Settings → Agents). */
  leadGuidance?: string | null;
}
export interface WorkerCheckpoint {
  summary: string;
  nextSteps: string;
  blocker: string | null;
  updatedAt: number;
}
export interface TeamMessage {
  id: number;
  sender: string;
  recipient: string;
  message: string;
  createdAt: number;
}
export interface TeamMessagePage {
  messages: TeamMessage[];
  nextCursor: number;
  hasMore: boolean;
}
export interface TeamWorker extends WorkerProfile {
  sessionId: string;
  profileId: string;
  /** `parked`: its run ended while its sub-workers work; it resumes with their results. */
  status: 'queued' | 'running' | 'waiting' | 'parked' | 'completed' | 'failed' | 'stopped';
  /** The direct worker that dispatched this sub-worker. */
  parentWorker?: string;
  assignment: string;
  summary: string;
  error?: string | null;
  dependsOn: string[];
  files: string[];
  /** Advisory no-edit assignment; legacy workers reserve their declared edit scope. */
  readOnly?: boolean;
  attempt: number;
  abandoned?: boolean;
  checkpoint?: WorkerCheckpoint | null;
  council?: CouncilSeat | null;
  /** S1/S2 native authority, with a pinned persistent bot ID where selected. */
  reviewerBinding?: {
    reviewerBotId?: string | null;
    /** Native cycle pinned to this review seat; parent snapshots can contain prior-cycle workers. */
    councilCycleId?: string;
    artifactRevision?: string;
    skillEvidence?: Record<string, { contentSha256: string; artifactRevision: string; invoked: boolean; checkExecuted: boolean; evidenceVerified: boolean; checkKind?: string | null }>;
    snapshot?: {
      skills?: {
        pins: { id: string; contentSha256: string; supplied: boolean; invoked: boolean; checkExecuted: boolean; evidenceVerified: boolean; requiredForAcceptance?: boolean; checkRequired?: boolean }[];
        skipped?: { id: string; reason: string }[];
      } | null;
    };
  } | null;
  /** When a writing run last started or ended; 0 or absent for one that never ran. */
  wroteAt?: number;
  createdAt: number;
  updatedAt: number;
}
export interface TeamSnapshot {
  sessionId: string;
  config: TeamConfig;
  /** Selected during an architect turn; becomes effective at the next turn. */
  pendingSetup?: TeamSetup | null;
  status: 'ready' | 'running' | 'waiting' | 'paused' | 'completed' | 'failed' | 'interrupted';
  round: number;
  revision: number;
  coordinationRevision?: number;
  reviewedCoordinationRevision?: number;
  workers: TeamWorker[];
  error?: string | null;
  summary?: string | null;
  /** The lead's failed turn, while its retry waits; `heldUntil` is the reset of a usage limit holding it. */
  leadFailure?: { error: string; heldUntil?: number | null } | null;
  /** When the user's latest objective began: reviewers created before it belong to an earlier cycle. */
  councilSince?: number;
  /** Native's reading, the only source of the council's state: the webview never derives it (a host that sends none reads as waiting). */
  councilStatus?: CouncilStatus | null;
  /** Native's read-only projection for the live Council cycle. Older hosts may omit these fields. */
  councilFindings?: CouncilFinding[];
  councilFindingDispositions?: CouncilFindingDisposition[];
  councilChildReceipts?: ChildCouncilReceipt[];
  councilChildLeaves?: CouncilChildLeafProjection[];
  councilCriterionOutcomes?: CouncilCriterionOutcomeReceipt[];
  councilReviewReceipts?: CouncilReviewReceipt[];
  councilUsage?: CouncilUsageRecord[];
  updatedAt: number;
}
/** The note on a team whose failed lead turn PandaCode is retrying. */
export const LEAD_RETRY = /^The lead's turn failed:/;
export const teamActive = (team?: TeamSnapshot | null): boolean => team?.status === 'running' || team?.status === 'waiting';
/** The reset a working team's retry is waiting for, while it is still ahead. A paused or stopped team retries nothing. */
export const teamHeldUntil = (team?: TeamSnapshot | null, now = Date.now()): number | null => {
  const until = team?.leadFailure?.heldUntil;
  return teamActive(team) && until && until > now ? until : null;
};

export type TeamEvent = TeamSnapshot | { sessionId: string; removed: true };
