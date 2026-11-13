/* Council: independent reviewers score the work out of 10 until every one reaches the user's target.
 * What a snapshot means, with no DOM, plus the remembered defaults. See docs/architect-workers.md. */
import { teamActive, type ChildCouncilReceipt, type CouncilComposition, type CouncilConfig, type CouncilCriteriaSource, type CouncilCriterion, type CouncilFinding, type CouncilFindingDisposition, type CouncilReviewReceipt, type CouncilState, type CouncilStatus, type CouncilStrategy, type CouncilReviewerMode, type CouncilUsageRecord, type TeamSnapshot, type TeamWorker } from './architect-types.ts';

export type { CouncilState, CouncilStatus };
export const COUNCIL_KEY = 'pandacode.council';
export const COUNCIL_TARGETS = [7, 8, 8.5, 9, 9.5, 10] as const;
/** Native allows 1-100 rounds; these are the choices offered. */
export const COUNCIL_ROUNDS = [3, 5, 10, 25] as const;
/** 0 = the lead picks reviewers for the work. */
export const COUNCIL_REVIEWERS = [0, 2, 3, 4, 5] as const;
const DEFAULTS: CouncilConfig = { target: 9, maxRounds: 10, reviewers: 0 };

export type CouncilModeId = 'M1' | 'M2' | 'M3' | 'M4' | 'M5' | 'M6' | 'M7';
export const COUNCIL_COMPOSITION_MODES: readonly { id: CouncilModeId; label: string; automatic: boolean; direct: boolean; nested: boolean }[] = [
  { id: 'M1', label: 'Architect reviewers', automatic: true, direct: false, nested: false },
  { id: 'M2', label: 'Direct bots', automatic: false, direct: true, nested: false },
  { id: 'M3', label: 'Direct bots + Architect', automatic: true, direct: true, nested: false },
  { id: 'M4', label: 'Bots with child Councils', automatic: false, direct: false, nested: true },
  { id: 'M5', label: 'Child Councils + Architect', automatic: true, direct: false, nested: true },
  { id: 'M6', label: 'Mixed direct + child bots', automatic: false, direct: true, nested: true },
  { id: 'M7', label: 'Mixed bots + Architect', automatic: true, direct: true, nested: true },
];

/** Native derives one of seven reviewer-source modes; strategy and criterion ownership stay independent. */
export function councilCompositionMode(composition: CouncilComposition): CouncilModeId | null {
  const direct = composition.selected.some(reviewer => reviewer.mode === 'direct');
  const nested = composition.selected.some(reviewer => reviewer.mode === 'nested');
  return COUNCIL_COMPOSITION_MODES.find(mode => mode.automatic === composition.automatic.enabled && mode.direct === direct && mode.nested === nested)?.id ?? null;
}

export const DEFAULT_COUNCIL_LIMITS = Object.freeze({
  maxDepth: 2,
  maxRootSeats: 8,
  maxConcurrentLeaves: 64,
  maxLeafReviewsPerRound: 128,
  timeoutMs: 86_400_000,
  maxEstimatedCostMicros: 1_000_000,
});

/** Defaults are finite and valid. Composition is task-scoped, so these are only the starting values for this panel. */
export function newCouncilComposition(): CouncilComposition {
  return {
    schemaVersion: 1,
    automatic: { enabled: true, count: 'auto' },
    selected: [],
    criteriaSource: 'architect',
    criteria: [],
    strategy: 'parallel',
    limits: { ...DEFAULT_COUNCIL_LIMITS },
  };
}

export interface CouncilCompositionValidationIssue { field: string; message: string }

/** Client-side mirror of the native schema gate, with stable field paths for accessible inline errors. */
export function councilCompositionValidationIssues(composition: CouncilComposition, target: number): CouncilCompositionValidationIssue[] {
  const issues: CouncilCompositionValidationIssue[] = [], { limits } = composition;
  const add = (field: string, message: string): void => { issues.push({ field, message }); };
  if (composition.schemaVersion !== 1) add('schemaVersion', 'Only Council composition schema version 1 is supported.');
  if (limits.maxDepth !== 2) add('limits.maxDepth', 'Maximum depth is fixed at 2.');
  if (limits.maxRootSeats !== 8) add('limits.maxRootSeats', 'Maximum root seats are fixed at 8.');
  if (!Number.isInteger(limits.maxConcurrentLeaves) || limits.maxConcurrentLeaves < 1 || limits.maxConcurrentLeaves > 64) add('limits.maxConcurrentLeaves', 'Concurrent leaves must be between 1 and 64.');
  if (!Number.isInteger(limits.maxLeafReviewsPerRound) || limits.maxLeafReviewsPerRound < 1 || limits.maxLeafReviewsPerRound > 128) add('limits.maxLeafReviewsPerRound', 'Leaf reviews per round must be between 1 and 128.');
  if (!Number.isInteger(limits.timeoutMs) || limits.timeoutMs < 1 || limits.timeoutMs > 86_400_000) add('limits.timeoutMs', 'Timeout must be between 1 millisecond and 24 hours.');
  if (!Number.isSafeInteger(limits.maxEstimatedCostMicros) || limits.maxEstimatedCostMicros <= 0) add('limits.maxEstimatedCostMicros', 'Estimated cost cap must be a positive whole number of micro-units.');
  const ids = new Set<string>(), lenses = new Set<string>();
  for (const [index, reviewer] of composition.selected.entries()) {
    const field = (name: string): string => `reviewers.${index}.${name}`;
    if (!/^[A-Za-z0-9_-]{1,80}$/.test(reviewer.botId)) add(field('botId'), `Reviewer ID "${reviewer.botId}" is not a valid native ID.`);
    if (ids.has(reviewer.botId)) add(field('botId'), `Reviewer "${reviewer.botId}" is selected more than once.`);
    ids.add(reviewer.botId);
    if (reviewer.required !== true) add(field('botId'), `Reviewer "${reviewer.botId}" must be required.`);
    if (reviewer.mode === 'nested') {
      const nested = reviewer.nested;
      if (!nested) { add(field('nested'), `Reviewer "${reviewer.botId}" needs child Council settings.`); continue; }
      if (!Number.isInteger(nested.childReviewerCount) || nested.childReviewerCount < 2 || nested.childReviewerCount > 8) add(field('nested.childReviewerCount'), `Child reviewer count for "${reviewer.botId}" must be 2–8.`);
      if (!Number.isFinite(nested.target) || nested.target < 1 || nested.target > 10) add(field('nested.target'), `Child target for "${reviewer.botId}" must be 1–10.`);
      if (!Number.isInteger(nested.maxRounds) || nested.maxRounds < 1 || nested.maxRounds > 100) add(field('nested.maxRounds'), `Child max rounds for "${reviewer.botId}" must be 1–100.`);
    } else if (reviewer.nested) add(field('mode'), `Direct reviewer "${reviewer.botId}" cannot include child Council settings.`);
  }
  if (composition.selected.length > 8) add('reviewers', 'Select no more than 8 root reviewers.');
  if (composition.automatic.enabled && composition.automatic.count !== 'auto'
    && (!Number.isInteger(composition.automatic.count) || composition.automatic.count < 1 || composition.automatic.count > 8)) add('automatic.count', 'Automatic reviewer count must be 1–8 or Auto.');
  if (!composition.automatic.enabled && typeof composition.automatic.count === 'number'
    && (!Number.isInteger(composition.automatic.count) || composition.automatic.count < 0 || composition.automatic.count > 8)) add('automatic.count', 'Disabled automatic reviewer count must be 0–8.');
  const minimumAutomatic = composition.automatic.enabled
    ? composition.automatic.count === 'auto' ? Math.max(1, 2 - composition.selected.length) : composition.automatic.count
    : 0;
  const rootSeats = composition.selected.length + minimumAutomatic;
  if (rootSeats < 2) add('reviewers', 'At least 2 root reviewers are required.');
  if (rootSeats > 8) add('reviewers', 'The plan exceeds the 8-seat root limit.');
  if (composition.criteria.length > 64) add('criteria', 'At most 64 criteria are allowed.');
  for (const [index, criterion] of composition.criteria.entries()) {
    const field = (name: string): string => `criteria.${index}.${name}`;
    if (!/^[A-Za-z0-9_-]{1,80}$/.test(criterion.id)) add(field('id'), `Criterion ID "${criterion.id}" must use letters, numbers, underscores or hyphens.`);
    if (ids.has(`criterion:${criterion.id}`)) add(field('id'), `Criterion ID "${criterion.id}" is duplicated.`);
    ids.add(`criterion:${criterion.id}`);
    const lens = criterion.lens.trim();
    if (!lens || lens.length > 80 || /[\u0000-\u001f\u007f]/.test(lens)) add(field('lens'), `Criterion "${criterion.id}" needs a plain lens name up to 80 characters.`);
    if (lenses.has(lens.toLowerCase())) add(field('lens'), `Criterion lens "${lens}" is duplicated.`);
    lenses.add(lens.toLowerCase());
    if (!criterion.description.trim() || criterion.description.length > 4096) add(field('description'), `Criterion "${criterion.id}" needs a description up to 4,096 characters.`);
    if (!Number.isFinite(criterion.target) || criterion.target < 1 || criterion.target > 10) add(field('target'), `Criterion target for "${criterion.id}" must be 1–10.`);
    if ((composition.criteriaSource === 'user' && criterion.source !== 'user')
      || (composition.criteriaSource === 'architect' && criterion.source !== 'architect')) add('criteriaSource', `Criterion "${criterion.id}" does not match the selected criteria source.`);
  }
  if (composition.criteriaSource === 'user' && (!composition.criteria.length || !composition.criteria.some(criterion => criterion.required))) add('criteria', 'User criteria need at least one required criterion.');
  if (composition.criteriaSource === 'hybrid' && !composition.criteria.some(criterion => criterion.source === 'user' && criterion.required)) add('criteria', 'Hybrid criteria need at least one required user criterion.');
  if (!Number.isFinite(target) || target < 1 || target > 10) add('councilTarget', 'Council target must be between 1 and 10.');
  if (!supportedCouncilStrategy(composition.strategy)) add('strategy', 'Choose a supported Council execution strategy.');
  const seen = new Set<string>();
  return issues.filter(issue => { if (seen.has(issue.message)) return false; seen.add(issue.message); return true; });
}

/** Keep the existing plain-message API for command validation and the preview host. */
export function validateCouncilComposition(composition: CouncilComposition, target: number): string[] {
  return councilCompositionValidationIssues(composition, target).map(issue => issue.message);
}

export function withCouncilComposition(config: CouncilConfig, composition: CouncilComposition): CouncilConfig {
  const selected = composition.selected.map(reviewer => reviewer.botId);
  const reviewers = composition.automatic.enabled && typeof composition.automatic.count === 'number' ? composition.automatic.count : 0;
  return { ...config, reviewers, reviewerBotIds: selected, composition };
}

export function withoutCouncilComposition(config: CouncilConfig): CouncilConfig {
  const { composition: _composition, ...legacy } = config;
  return legacy;
}

export const criterionSourceFor = (source: CouncilCriteriaSource): CouncilCriterion['source'] => source === 'user' ? 'user' : 'architect';
export const supportedCouncilStrategy = (strategy: CouncilStrategy): boolean =>
  strategy === 'parallel' || strategy === 'escalated' || strategy === 'two_phase';

/** Native pins a root Council cycle to `<sessionId>:<councilSince>`; mirror that identity at the view boundary. */
export const councilCycleId = (team: Pick<TeamSnapshot, 'sessionId' | 'councilSince'>): string => `${team.sessionId}:${team.councilSince ?? 0}`;

/** Reject a snapshot from another task, an earlier cycle, or an older revision. Equal revisions in the same cycle
 *  are allowed because projection arrays can be refreshed without changing visible worker state. */
export function acceptsCouncilSnapshot(candidate: TeamSnapshot, current: TeamSnapshot | null, sessionId: string): boolean {
  if (candidate.sessionId !== sessionId) return false;
  if (!current || current.sessionId !== sessionId) return true;
  return candidate.revision >= current.revision && (candidate.councilSince ?? 0) >= (current.councilSince ?? 0);
}

export interface CouncilLedgerProjection {
  cycleId: string;
  findings?: CouncilFinding[];
  findingDispositions?: CouncilFindingDisposition[];
  childReceipts?: ChildCouncilReceipt[];
  reviewReceipts?: CouncilReviewReceipt[];
  usage?: CouncilUsageRecord[];
}

/** Keep the UI bounded to native's projected root cycle and its correlated child cycles. RootCycleId is native's
 *  `<sessionId>:<councilSince>`; child cycles are admitted only by current-root child receipts or usage records. */
export function councilLedgerForCurrentCycle(
  team: Pick<TeamSnapshot, 'sessionId' | 'councilSince' | 'councilFindings' | 'councilFindingDispositions' | 'councilChildReceipts' | 'councilReviewReceipts' | 'councilUsage'>,
): CouncilLedgerProjection {
  const cycleId = councilCycleId(team);
  const childReceipts = team.councilChildReceipts?.filter(receipt => receipt.rootCycleId === cycleId);
  const usage = team.councilUsage?.filter(record => record.rootCycleId === cycleId);
  const childCycleIds = new Set([
    ...(childReceipts ?? []).map(receipt => receipt.childCycleId),
    ...(usage ?? []).map(record => record.childCycleId).filter((id): id is string => Boolean(id)),
  ]);
  const allowedCycleIds = new Set([cycleId, ...childCycleIds]);
  const findings = team.councilFindings?.filter(finding => allowedCycleIds.has(finding.cycleId));
  const findingIds = new Set((findings ?? []).map(finding => finding.id));
  return {
    cycleId,
    ...(findings === undefined ? {} : { findings }),
    ...(team.councilFindingDispositions === undefined ? {} : { findingDispositions: team.councilFindingDispositions.filter(item => findingIds.has(item.findingId)) }),
    ...(childReceipts === undefined ? {} : { childReceipts }),
    ...(team.councilReviewReceipts === undefined ? {} : { reviewReceipts: team.councilReviewReceipts.filter(receipt => allowedCycleIds.has(receipt.cycleId)) }),
    ...(usage === undefined ? {} : { usage }),
  };
}

export type CouncilSeatWorker = TeamWorker & { council: NonNullable<TeamWorker['council']> };
/** A round as shown: its seats and their lowest score. Whether it is done is native's to say (`councilStatus`). */
export interface CouncilRound { number: number; seats: CouncilSeatWorker[]; lowest: number | null }
/** `status` is native's `councilStatus`; the webview derives no state (native also sees edits it cannot, like the lead's own). */
export interface CouncilProgress { council: CouncilConfig; rounds: CouncilRound[]; latest: CouncilRound | undefined; state: CouncilState; status: CouncilStatus }
type CouncilTeam = Pick<TeamSnapshot, 'workers' | 'config' | 'councilSince' | 'councilStatus'>;

export const seatScore = (seat: Pick<CouncilSeatWorker, 'council'>): number | null => {
  const { score } = seat.council;
  return typeof score === 'number' && Number.isFinite(score) ? score : null;
};
/** Every score and target, on screen and read aloud, has one decimal: 9.0, 9.5, 10.0. */
export const formatScore = (score: number): string => (Math.round(score * 10) / 10).toFixed(1);
/** A score against the target, for names and announcements: "9.6 out of 10 (target 9.5)". */
export const outOfTen = (score: number, target: number): string => `${formatScore(score)} out of 10 (target ${formatScore(target)})`;

/** The current cycle's rounds, oldest first. Reviewers from before the cycle are not part of it; an abandoned seat
 *  drops out unless it was scored (native refuses to cancel or replace a scored seat). */
export function councilRounds(team: Pick<TeamSnapshot, 'workers' | 'councilSince'>): CouncilRound[] {
  const since = team.councilSince ?? 0, byRound = new Map<number, CouncilSeatWorker[]>();
  for (const worker of team.workers) {
    if (!worker.council || worker.createdAt < since || (worker.abandoned && seatScore(worker as CouncilSeatWorker) === null)) continue;
    byRound.set(worker.council.round, [...(byRound.get(worker.council.round) ?? []), worker as CouncilSeatWorker]);
  }
  return [...byRound].sort(([a], [b]) => a - b).map(([number, seats]) => {
    const scores = seats.map(seatScore).filter((score): score is number => score !== null);
    return { number, seats, lowest: scores.length ? Math.min(...scores) : null };
  });
}

/** The council's rounds and overall state; null while the team has no council. */
export function councilProgress(team: CouncilTeam): CouncilProgress | null {
  const council = team.config.council;
  if (!council) return null;
  // A host that sends no status yet (an older remote) reads as waiting; its rounds still show.
  const rounds = councilRounds(team), status = team.councilStatus ?? { state: 'waiting', round: 0, lowest: null, target: council.target, stale: false };
  return { council, rounds, latest: rounds.at(-1), state: status.state, status };
}

/** Whether a round's lowest is only so far: native is still reviewing that round. */
export const provisional = ({ status }: CouncilProgress, round: CouncilRound): boolean => (status.state === 'reviewing' || status.state === 'dispatch') && status.round === round.number;

/** The team rests for the user: native's state, nothing else. */
export const councilAsking = (status: CouncilStatus): boolean => status.state === 'asking';

/** What the user reads in place of a Council refusal in `team.error` (written for the lead, never shown raw): native's notice,
 *  set whenever there is one (asking or paused). */
export function councilNotice(team: Pick<TeamSnapshot, 'councilStatus'>): string | null {
  const status = team.councilStatus;
  return status?.notice || (status && councilAsking(status) ? 'The lead is asking how to continue.' : null);
}

/** How a team that is not running reads: Failed, Stopped, Paused or Interrupted. */
export const runWord = (run: Pick<TeamSnapshot, 'status' | 'error'>): string =>
  run.status === 'failed' ? 'Failed' : run.status === 'paused' ? (/^Team stopped/.test(run.error ?? '') ? 'Stopped' : 'Paused') : run.status === 'interrupted' ? 'Interrupted' : 'Stopped';

/** The team finished (nothing running, nothing asked) with the work still below target. */
export function finishedBelow({ status }: CouncilProgress, run: Pick<TeamSnapshot, 'status'>): boolean {
  return !teamActive(run as TeamSnapshot) && (run.status === 'completed' || run.status === 'ready') && (status.state === 'fixing' || status.state === 'reviewing' || status.state === 'dispatch')
    && status.lowest !== null && status.lowest < status.target;
}

/** The state in words. With the team's run, a council whose team is not running never claims to wait, review or fix. */
/** `lowered`: the target was lowered during this cycle, so a pass says so. */
export function councilStateText({ council, status, rounds: cycle }: CouncilProgress, run?: Pick<TeamSnapshot, 'status' | 'error'>, lowered = false): string {
  const target = formatScore(status.target), lowest = status.lowest === null ? null : formatScore(status.lowest);
  const rounds = ` of ${council.maxRounds}`;
  // Native's `stale`: something changed after the latest round started. Only a passing round is undone by it;
  // a stale round below target is the ordinary fixing loop.
  const changedAfterPass = status.stale && status.lowest !== null && status.lowest >= status.target;
  const idle = run && !teamActive(run as TeamSnapshot);
  if (idle && status.state === 'waiting') return run.status === 'completed' || run.status === 'ready' ? 'No changes to review yet' : `${runWord(run)} before the first round`;
  if (idle && (status.state === 'reviewing' || status.state === 'dispatch' || status.state === 'fixing')) {
    // The footer carries the numbers.
    if (run.status === 'completed' || run.status === 'ready') return changedAfterPass ? `Finished with changes since passing at ${lowest}` : lowest ? 'Finished below target' : 'Finished before any score';
    // An open round's lowest is only the lowest so far.
    return [runWord(run), `round ${status.round}${rounds}`, lowest && `lowest ${status.state === 'fixing' ? '' : 'so far '}${lowest}`, lowest && `target ${target}`].filter(Boolean).join(' · ');
  }
  switch (status.state) {
    case 'waiting': return 'Waiting for reviewers';
    case 'reviewing': return `Round ${status.round}${rounds} · reviewing`;
    case 'dispatch': return `Round ${status.round}${rounds} · waiting for the lead`;
    // The score is in native's notice, shown with the card's actions.
    case 'asking': return 'Needs you';
    // The card's footer (round history, target) carries the scores.
    case 'fixing': return changedAfterPass ? `Changed after passing at ${lowest}: fixing` : `Round ${status.round}${rounds} · fixing`;
    case 'passed': return `Passed at ${lowest}${lowered ? ` (target lowered to ${target})` : ''}`;
    // Accepted while the latest round has no score: the last complete round's lowest, else just "Accepted".
    case 'accepted': { const last = status.lowest ?? [...cycle].reverse().find(round => round.lowest !== null)?.lowest; return last === undefined || last === null ? 'Accepted' : `Accepted at ${formatScore(last)}`; }
    case 'limit': return `${changedAfterPass ? `Stopped at round limit: changed after passing at ${lowest}` : 'Stopped at round limit'} (${status.round}${rounds})`;
  }
}

/** A reviewer's score against the target, in words and a tone ('at', 'below', or none when no target is known). */
export const scoreVerdict = (score: number, target: number | undefined): { text: string; tone: '' | 'at' | 'below' } =>
  target === undefined ? { text: `Scored ${formatScore(score)}`, tone: '' }
    : score >= target ? { text: `Scored ${formatScore(score)} · at target`, tone: 'at' } : { text: `Scored ${formatScore(score)} · below ${formatScore(target)}`, tone: 'below' };


/** Each lens's first score this cycle and its latest, for the settled card's first-vs-final table; lenses in first-seen order. */
export function firstAndFinal(rounds: readonly CouncilRound[]): { lens: string; first: number; final: number }[] {
  const byLens = new Map<string, { lens: string; first: number; final: number }>();
  for (const round of rounds) for (const seat of round.seats) {
    const score = seatScore(seat), lens = seat.council.lens.trim() || seat.name;
    if (score === null) continue;
    const known = byLens.get(lens);
    byLens.set(lens, known ? { ...known, final: score } : { lens, first: score, final: score });
  }
  return [...byLens.values()];
}

/** The seats the scorecard shows instead of plain worker rows. */
export const councilSeatIds = (progress: CouncilProgress | null): Set<string> => new Set(progress?.rounds.flatMap(round => round.seats.map(seat => seat.id)) ?? []);

const pick = <T extends number>(value: unknown, options: readonly T[], fallback: T): T => options.includes(value as T) ? value as T : fallback;

/** The settings a new council starts with; council itself is off on each new thread. A value no longer offered
 *  (an old 0, "no limit", for rounds) reads as the default. */
export function councilDefaults(): CouncilConfig {
  let saved: Partial<Record<keyof CouncilConfig, unknown>> | null = null;
  try { saved = JSON.parse(localStorage.getItem(COUNCIL_KEY) ?? 'null'); } catch { /* Unreadable or malformed storage falls back. */ }
  return {
    target: pick(saved?.target, COUNCIL_TARGETS, DEFAULTS.target),
    maxRounds: pick(saved?.maxRounds, COUNCIL_ROUNDS, DEFAULTS.maxRounds),
    reviewers: pick(saved?.reviewers, COUNCIL_REVIEWERS, DEFAULTS.reviewers),
  };
}
function savedDefaults(): Record<string, unknown> {
  try { const saved = JSON.parse(localStorage.getItem(COUNCIL_KEY) ?? 'null'); return saved && typeof saved === 'object' ? saved : {}; } catch { return {}; }
}
export function saveCouncilDefaults(council: CouncilConfig): void {
  // Reviewer bot identities belong to one task, never global defaults for new threads.
  const safeDefaults = { target: council.target, maxRounds: council.maxRounds, reviewers: council.reviewers };
  try { localStorage.setItem(COUNCIL_KEY, JSON.stringify({ ...savedDefaults(), ...safeDefaults, reviewerBotIds: undefined })); } catch { /* Kept for this window only. */ }
}
/** Whether a new thread starts with Council on (when its team starts, or on its first send). Stored with the defaults. */
export const councilForNewThreads = (): boolean => savedDefaults().newThreads === true;
export function setCouncilForNewThreads(on: boolean): void {
  try { localStorage.setItem(COUNCIL_KEY, JSON.stringify({ ...savedDefaults(), newThreads: on })); } catch { /* Kept for this window only. */ }
}

/** A reviewer row's verdict, one vocabulary for the card, the thread's rows and the Workers list: at target in the success
 *  tone; below it, the warning only on its round's lowest seat once that round is complete (others, and an open round's
 *  lowest, stay neutral). The target is native's (`councilStatus.target`) when it has one. */
export function seatVerdict(team: Pick<TeamSnapshot, 'workers' | 'config' | 'councilSince' | 'councilStatus'>, seat: Pick<CouncilSeatWorker, 'council' | 'createdAt'>): { text: string; tone: '' | 'at' | 'below' } | null {
  const score = seatScore(seat), target = team.councilStatus?.target ?? team.config.council?.target;
  if (score === null) return null;
  const verdict = scoreVerdict(score, target);
  if (verdict.tone !== 'below') return verdict;
  const since = team.councilSince ?? 0, current = seat.createdAt >= since;
  const peers = team.workers.filter(worker => worker.council?.round === seat.council.round && (worker.createdAt >= since) === current);
  const lowest = Math.min(...peers.map(worker => seatScore(worker as CouncilSeatWorker)).filter((value): value is number => value !== null));
  const status = team.councilStatus, open = current && (status?.state === 'reviewing' || status?.state === 'dispatch') && status.round === seat.council.round;
  return { ...verdict, tone: score === lowest && !open ? 'below' : '' };
}
