import { escape, icon, isImeKey, morph } from './dom';
import { reducedMotion, springKeyframes, trackSegment } from './motion';
import { hidePopover, showPopover } from './popover-motion';
import { teamActive, type CouncilComposition, type CouncilConfig, type CouncilStrategy, type NestedCouncilConfig, type SelectedCouncilReviewer, type TeamSnapshot, type TeamWorker } from './architect-types';
import type { ProviderId } from './types';
import { COUNCIL_COMPOSITION_MODES, COUNCIL_REVIEWERS, COUNCIL_ROUNDS, COUNCIL_TARGETS, councilCompositionMode, councilCompositionValidationIssues, councilDefaults, councilForNewThreads, councilLedgerForCurrentCycle, criterionSourceFor, newCouncilComposition, setCouncilForNewThreads, validateCouncilComposition, withCouncilComposition, councilAsking, councilNotice, councilProgress, councilStateText, firstAndFinal, finishedBelow, formatScore, outOfTen, provisional, saveCouncilDefaults, seatScore, type CouncilLedgerProjection, type CouncilProgress, type CouncilRound, type CouncilSeatWorker } from './council';

/* Council in the composer: the tray's settings button and panel, and one compact strip inside the prompt box (its state,
 * the reviewers on demand, and what to do next). The Agents flyout shows the same state line and rows. */

export type CouncilHost = {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  /** The executor is never an eligible reviewer; native validates this independently. */
  executorBotId?(sessionId: string): string | null;
  harnessAvailable?(provider: ProviderId): boolean;
  team(sessionId: string): TeamSnapshot | null;
  /** Whether the thread's team (or that it has none) is known yet: false while it still loads. */
  loaded(sessionId: string): boolean;
  /** Turns on an automatic team; false when nothing changed. May throw with the reason. */
  startTeam(sessionId: string): Promise<boolean>;
  /** A snapshot from native, for the app's usual team update. */
  updated(team: TeamSnapshot): void;
  /** Keep fixing: a follow-up to the lead. Throws with the reason when it cannot go. */
  keepFixing(sessionId: string): Promise<void>;
  /** Settings → Classic team setup: a thread with no team opens that setup rather than starting an automatic team. */
  classicSetup(): boolean;
  openClassicSetup(anchor: HTMLElement): void;
  /** Reads a change aloud through the composer's live region. */
  announce(text: string): void;
};

type Field = 'target' | 'maxRounds' | 'reviewers';
type ReviewerBot = { id: string; name: string; role: string; provider: ProviderId; model: string; paused: boolean; skills?: string[]; available?: boolean; reason?: string | null };
type ReviewerSelection = { selected: boolean; ids: string[]; idsEdited?: boolean; optionalSkills?: string[]; checkSkills?: string[]; scoped?: boolean; fileScope?: string; authorizationId?: string };
type CompositionPanelDraft = { value: CouncilComposition; enabled: boolean; botsEnabled: boolean; reviewers: Map<string, SelectedCouncilReviewer> };
const FIELDS: readonly { key: Field; label: string; options: readonly number[]; name(value: number): string }[] = [
  { key: 'target', label: 'Target score', options: COUNCIL_TARGETS, name: formatScore },
  { key: 'maxRounds', label: 'Max rounds', options: COUNCIL_ROUNDS, name: String },
  { key: 'reviewers', label: 'Reviewers', options: COUNCIL_REVIEWERS, name: value => value ? String(value) : 'Auto' },
];
/** Wide enough that "10.0" keeps clear of its segment's edge. */
const PANEL_WIDTH = 300;
const BUTTON = '.composer-council-button';
/** A balance: reviewers weighing the work. Its own glyph, apart from the sidebar's Agent usage. */
const COUNCIL_ICON = '<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v16M8 20h8M5 7h14M5 7l-3 6.5a3 3 0 0 0 6 0zM19 7l-3 6.5a3 3 0 0 0 6 0z"/></svg>';
/** The panel's one description, at most two lines: what Council does, then what turning it on means. */
const ABOUT = 'Reviewers score the work until all reach your target.';
/** The strip control whose panel is open (`session:field`), so a redraw of the strip keeps its aria-expanded. */
let expandedOpener = '';
const NOTE_ON = 'Uses more of your plan.';
const NOTE_OFF = 'These settings become your defaults.';
const NOTE_NO_TEAM = 'Starts a team; these become your defaults.';
const AUTO = 'Auto: the lead picks reviewers for the work';
/** Every refusal reads the same in the panel, the strip and announcements. */
const notChanged = (reason: string): string => `Council was not changed: ${reason}`;

type CouncilLedgerView = CouncilLedgerProjection & {
  unavailableMessage?: string;
  progress?: CouncilProgress | null;
  workers?: TeamWorker[];
  status?: { state?: string; round?: number; lowest?: number | null; target?: number; stale?: boolean } | null;
  acceptedBy?: string | null;
  acceptanceReason?: string | null;
};

const ledgerRef = (value: string): string => {
  const path = value.startsWith('file:') ? value.slice(5) : value;
  return `<code>${escape(path)}</code>`;
};
const ledgerRefs = (values: readonly string[]): string => values.length
  ? `<ul class="council-ledger-refs">${values.map(value => `<li>${ledgerRef(value)}</li>`).join('')}</ul>`
  : '<p class="council-ledger-empty">No references.</p>';
const usageValue = (value: { status: string; value?: number | null } | undefined): string =>
  !value || value.status === 'unknown' || value.value == null ? 'Unknown' : `${value.value.toLocaleString()}${value.status === 'measured' ? '' : ' · estimate only'}`;

const validationId = (field: string): string => `council-field-error-${field.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
const fieldErrorMarkup = (field: string): string => `<small class="council-field-error" id="${validationId(field)}" data-composition-field-error="${escape(field)}" hidden></small>`;

function compositionPlanPreview(active: CouncilComposition, bots: readonly ReviewerBot[]): { summary: string; details: string } {
  const mode = councilCompositionMode(active);
  const unavailable = mode === null;
  const automatic = active.automatic.enabled;
  const minimumAutomatic = automatic ? active.automatic.count === 'auto' ? Math.max(1, 2 - active.selected.length) : active.automatic.count : 0;
  const automaticDetails = automatic
    ? `<li><strong>Automatic Architect reviewers</strong><span>${active.automatic.count === 'auto' ? `Auto · at least ${minimumAutomatic}; native chooses the final count at dispatch` : `${active.automatic.count} configured`}</span></li>`
    : '<li><strong>Automatic Architect reviewers</strong><span>Off</span></li>';
  const selectedDetails = active.selected.length
    ? active.selected.map(reviewer => {
      const bot = bots.find(item => item.id === reviewer.botId), name = bot?.name ?? reviewer.botId;
      const availability = !bot ? ' · unavailable' : bot.paused || bot.available === false ? ' · currently unavailable' : '';
      const child = reviewer.mode === 'nested' ? reviewer.nested : undefined;
      const branch = child
        ? `Child Council · ${child.template.replaceAll('_', ' ')} · ${child.childReviewerCount} reviewers · target ${formatScore(child.target)} · ${child.maxRounds} rounds · ${child.childCriteriaSource} criteria · ${child.escalation.replaceAll('_', ' ')}`
        : 'Direct reviewer';
      return `<li><strong>${escape(name)} · Bot ${escape(reviewer.botId)}</strong><span>${escape(branch + availability)}</span></li>`;
    }).join('')
    : '<li><strong>No selected bot reviewers</strong><span>Only the automatic source can assign reviewers.</span></li>';
  const criteria = active.criteria.length
    ? `<ul class="council-plan-preview-list council-plan-preview-criteria">${active.criteria.map(item => `<li><strong>${escape(item.lens || item.id || 'Untitled criterion')}</strong><span>${escape(item.source)} · target ${formatScore(item.target)}${item.required ? ' · required' : ' · optional'}${item.evidenceRequired ? ' · evidence required' : ''}</span><p>${escape(item.description)}</p></li>`).join('')}</ul>`
    : '<p class="council-plan-preview-empty">No explicit criteria are saved; the selected source supplies the criteria.</p>';
  const rootCount = automatic && active.automatic.count === 'auto'
    ? `${active.selected.length} selected bot${active.selected.length === 1 ? '' : 's'} + automatic count chosen at dispatch`
    : `${active.selected.length + minimumAutomatic} configured root reviewer${active.selected.length + minimumAutomatic === 1 ? '' : 's'}`;
  const sourceNote = unavailable
    ? '<p class="council-plan-preview-unavailable">Unavailable: no reviewer source is enabled. Enable automatic reviewers or select at least one bot reviewer.</p>'
    : '';
  const details = `<div class="council-plan-preview-content"><dl><div><dt>Root reviewers</dt><dd>${escape(rootCount)}</dd></div><div><dt>Execution strategy</dt><dd>${escape(active.strategy.replaceAll('_', ' '))}</dd></div><div><dt>Criteria source</dt><dd>${escape(active.criteriaSource)}</dd></div></dl>${sourceNote}<h4>Reviewer branches</h4><ul class="council-plan-preview-list">${automaticDetails}${selectedDetails}</ul><h4>Criteria in this plan</h4>${criteria}<h4>Configured hard limits</h4><dl>${Object.entries(active.limits).map(([key, value]) => `<div><dt>${escape(key.replace(/([A-Z])/g, ' $1'))}</dt><dd>${escape(String(value))}</dd></div>`).join('')}</dl><p class="council-plan-preview-note">This previews the saved plan. Native assigns reviewers and records run scores, evidence, and usage.</p></div>`;
  return { summary: unavailable ? 'Plan preview · Unavailable' : `Plan preview · ${mode}`, details };
}

function renderCompositionFieldErrors(slot: HTMLElement, issues: ReturnType<typeof councilCompositionValidationIssues>): void {
  const grouped = new Map<string, string[]>();
  for (const issue of issues) grouped.set(issue.field, [...(grouped.get(issue.field) ?? []), issue.message]);
  for (const node of slot.querySelectorAll<HTMLElement>('[data-composition-field]')) {
    const field = node.dataset.compositionField!, messages = grouped.get(field) ?? [], error = [...slot.querySelectorAll<HTMLElement>('[data-composition-field-error]')].find(item => item.dataset.compositionFieldError === field);
    if (error) { error.hidden = messages.length === 0; error.textContent = messages.join(' '); }
    const controls: HTMLElement[] = node.matches('input,select,textarea,[role="group"]') ? [node] : [...node.querySelectorAll<HTMLElement>('input,select,textarea,[role="group"]')];
    const id = validationId(field);
    for (const control of controls) {
      if (messages.length) control.setAttribute('aria-invalid', 'true'); else control.removeAttribute('aria-invalid');
      const describedBy = new Set((control.getAttribute('aria-describedby') ?? '').split(/\s+/).filter(value => value && value !== id));
      if (messages.length && error) describedBy.add(id);
      if (describedBy.size) control.setAttribute('aria-describedby', [...describedBy].join(' ')); else control.removeAttribute('aria-describedby');
    }
  }
}

function updateCompositionPlanPreview(slot: HTMLElement, active: CouncilComposition, bots: readonly ReviewerBot[]): void {
  const preview = slot.querySelector<HTMLElement>('.council-plan-preview');
  if (!preview) return;
  const value = compositionPlanPreview(active, bots);
  const summary = preview.querySelector<HTMLElement>('[data-plan-preview-summary]'), details = preview.querySelector<HTMLElement>('[data-plan-preview-details]');
  if (summary && summary.textContent !== value.summary) summary.textContent = value.summary;
  if (details && details.innerHTML !== value.details) {
    const scrollTop = details.scrollTop, scrollLeft = details.scrollLeft;
    details.innerHTML = value.details;
    details.scrollTop = scrollTop; details.scrollLeft = scrollLeft;
  }
}

function evidenceDetails(refs: readonly string[], available: boolean): string {
  const body = refs.length ? ledgerRefs(refs) : `<p class="council-ledger-empty">${available ? 'No evidence references in the available records.' : 'Evidence references are not available in this snapshot.'}</p>`;
  return `<details class="council-inspector-evidence"><summary>Evidence · ${refs.length}${available ? '' : ' · unknown'}</summary>${body}</details>`;
}

function roundName(roundId: string): string {
  const suffix = roundId.split(':').at(-1) ?? '';
  return /^\d+$/.test(suffix) ? `Round ${suffix}` : 'Round unknown';
}

function childCouncilHierarchyHtml(value: CouncilLedgerView, receipt: NonNullable<CouncilLedgerView['childReceipts']>[number]): string {
  const cycleId = receipt.childCycleId;
  const findings = (value.findings ?? []).filter(item => item.cycleId === cycleId);
  const usage = (value.usage ?? []).filter(item => item.childCycleId === cycleId);
  const reviews = (value.reviewReceipts ?? []).filter(item => item.cycleId === cycleId);
  const rounds = new Map<string, { findings: typeof findings; usage: typeof usage; reviews: typeof reviews }>();
  const round = (id: string) => {
    let entry = rounds.get(id);
    if (!entry) { entry = { findings: [], usage: [], reviews: [] }; rounds.set(id, entry); }
    return entry;
  };
  for (const record of findings) round(record.roundId).findings.push(record);
  for (const record of usage) round(record.roundId).usage.push(record);
  const mappedReviewIds = new Set<string>();
  for (const item of reviews) {
    const linked = usage.find(record => record.seatId === item.assignmentId);
    if (linked) { round(linked.roundId).reviews.push(item); mappedReviewIds.add(item.requestId); }
  }
  for (const item of reviews) if (!mappedReviewIds.has(item.requestId)) round('').reviews.push(item);

  const roundMarkup = [...rounds.entries()].map(([roundId, entry]) => {
    const seats = new Map<string, { reviewerIds: Set<string>; findings: typeof findings; usage: typeof usage; reviews: typeof reviews }>();
    const seat = (id: string) => {
      let entry = seats.get(id);
      if (!entry) { entry = { reviewerIds: new Set(), findings: [], usage: [], reviews: [] }; seats.set(id, entry); }
      return entry;
    };
    for (const item of entry.findings) seat(item.seatId).findings.push(item);
    for (const item of entry.usage) { const found = seat(item.seatId); found.reviewerIds.add(item.reviewerId); found.usage.push(item); }
    for (const item of entry.reviews) {
      const id = item.assignmentId ?? `unknown-reviewer-${item.requestId}`;
      seat(id).reviews.push(item);
    }
    const reviewerMarkup = [...seats.entries()].map(([seatId, records]) => {
      const reviewers = [...records.reviewerIds];
      const label = reviewers.length ? reviewers.join(', ') : seatId.startsWith('unknown-reviewer-') ? 'Unknown reviewer' : `Seat ${seatId}`;
      const evidence = [...new Set([
        ...records.findings.flatMap(item => [...item.evidenceRefs, ...(item.location?.file ? [item.location.file] : [])]),
        ...records.reviews.flatMap(item => item.evidenceRefs),
      ])];
      const verdicts = [...new Set([...records.usage.map(item => item.terminalStatus), ...records.reviews.map(item => item.verdict)])];
      const findingsMarkup = records.findings.length
        ? `<ul class="council-inspector-findings">${records.findings.map(item => `<li><strong>${escape(item.category)} · ${escape(item.severity)}</strong><span>${escape(item.summary)}</span></li>`).join('')}</ul>`
        : '<p class="council-ledger-empty">No finding details were projected for this reviewer.</p>';
      const evidenceKnown = records.findings.length > 0 || records.reviews.length > 0 || value.findings !== undefined || value.reviewReceipts !== undefined;
      return `<details class="council-inspector-node" data-level="reviewer"><summary>Reviewer ${escape(label)} · Score Unknown${verdicts.length ? ` · ${escape(verdicts.join(', '))}` : ' · status Unknown'}</summary><dl><div><dt>Reviewer seat</dt><dd>${escape(seatId)}</dd></div><div><dt>Score</dt><dd>Unknown · child reviewer scores are not included in this projection.</dd></div><div><dt>Verdict</dt><dd>${escape(verdicts.join(', ') || 'Unknown')}</dd></div><div><dt>Evidence</dt><dd>${evidence.length ? `${evidence.length} reference${evidence.length === 1 ? '' : 's'}` : records.findings.length || records.reviews.length ? 'No references recorded' : 'Unknown'}</dd></div></dl>${findingsMarkup}${evidenceDetails(evidence, evidenceKnown)}</details>`;
    }).join('');
    return `<details class="council-inspector-node" data-level="round"${roundId ? '' : ' data-round-unknown="true"'}><summary>${escape(roundName(roundId))}${roundId ? ` · ${escape(roundId)}` : ' · round identity unknown'}</summary>${reviewerMarkup || '<p class="council-ledger-empty">Reviewer rows are not available for this round.</p>'}</details>`;
  }).join('');
  const minScore = receipt.minScore == null ? 'Unknown' : `${formatScore(receipt.minScore)} / 10`;
  const evidence = [...new Set(receipt.evidenceRefs)];
  const scoreRefs = receipt.leafScoreRefs.length
    ? `<p class="council-inspector-score-refs">Leaf score references: ${receipt.leafScoreRefs.map(escape).join(', ')}. Numeric reviewer scores are not included in this projection.</p>`
    : '<p class="council-ledger-empty">No leaf score references were projected.</p>';
  return `<details class="council-inspector-node" data-level="child-council" data-child-cycle="${escape(cycleId)}" data-stale="${receipt.status === 'stale'}"><summary>Child Council · ${escape(receipt.status)} · minimum score ${minScore}</summary><dl><div><dt>Bot</dt><dd>${escape(receipt.botId)}</dd></div><div><dt>Cycle</dt><dd>${escape(cycleId)}</dd></div><div><dt>Artifact revision</dt><dd>${escape(receipt.artifactRevision)}</dd></div><div><dt>Policy</dt><dd>${escape(receipt.policyFingerprint)}</dd></div><div><dt>Snapshot</dt><dd>${escape(receipt.snapshotFingerprint)}</dd></div></dl>${scoreRefs}${evidenceDetails(evidence, true)}<div class="council-inspector-children">${roundMarkup || '<p class="council-ledger-unavailable">Round details are unknown; the child receipt does not include reviewer round rows.</p>'}</div></details>`;
}

function councilHierarchyHtml(value: CouncilLedgerView): string {
  const status = value.status, progress = value.progress, target = progress?.status.target ?? status?.target;
  const lowest = status?.lowest ?? progress?.latest?.lowest ?? null, state = status?.state ?? 'Unknown';
  const score = lowest === null ? 'lowest score Unknown' : `lowest score ${formatScore(lowest)} / ${target == null ? '10' : formatScore(target)}`;
  const current = progress?.rounds.flatMap(item => item.seats) ?? [];
  const childReceipts = value.childReceipts ?? [];
  const linkedReceipts = new Set<NonNullable<CouncilLedgerView['childReceipts']>[number]>();
  const bots = current.map(seat => {
    const botId = seat.reviewerBinding?.reviewerBotId;
    const name = botId ? `Bot ${botId}` : seat.name || 'Unknown bot';
    const scoreValue = seatScore(seat), scoreText = scoreValue === null ? 'Score Unknown' : `Score ${formatScore(scoreValue)} / 10`;
    const evidenceFindings = (value.findings ?? []).filter(item => item.cycleId === value.cycleId && item.seatId === seat.id);
    const evidenceReviews = (value.reviewReceipts ?? []).filter(item => item.cycleId === value.cycleId && item.assignmentId === seat.id);
    const evidence = [...new Set([
      ...evidenceFindings.flatMap(item => [...item.evidenceRefs, ...(item.location?.file ? [item.location.file] : [])]),
      ...evidenceReviews.flatMap(item => item.evidenceRefs),
    ])];
    const receipts = childReceipts.filter(item => item.parentSeatId === seat.id);
    for (const receipt of receipts) linkedReceipts.add(receipt);
    const selected = progress?.council.composition?.selected.find(item => item.botId === botId && item.mode === 'nested');
    const children = receipts.map(receipt => childCouncilHierarchyHtml(value, receipt)).join('')
      || (selected ? '<p class="council-ledger-unavailable" data-child-state="unknown">Child Council state Unknown · native has not projected a receipt for this bot.</p>' : '');
    const evidenceKnown = value.findings !== undefined || value.reviewReceipts !== undefined;
    return `<details class="council-inspector-node" data-level="bot" data-seat-id="${escape(seat.id)}"><summary>${escape(name)} · ${escape(seat.council.lens || 'Reviewer')} · Round ${seat.council.round} · ${scoreText}</summary><dl><div><dt>Seat</dt><dd>${escape(seat.id)}</dd></div><div><dt>Review status</dt><dd>${escape(seat.status)}</dd></div><div><dt>Score</dt><dd>${scoreValue === null ? 'Unknown' : `${formatScore(scoreValue)} / 10`}</dd></div><div><dt>Target</dt><dd>${target == null ? 'Unknown' : `${formatScore(target)} / 10`}</dd></div></dl>${evidenceDetails(evidence, evidenceKnown)}${children}</details>`;
  }).join('');
  const orphanReceipts = childReceipts.filter(receipt => !linkedReceipts.has(receipt)).map(receipt => `<details class="council-inspector-node" data-level="bot" data-parent-unknown="true"><summary>Bot ${escape(receipt.botId)} · parent seat Unknown</summary><p class="council-ledger-unavailable">Parent reviewer details are not present in this root snapshot.</p>${childCouncilHierarchyHtml(value, receipt)}</details>`).join('');
  const hierarchyBody = current.length || orphanReceipts
    ? `<div class="council-inspector-children">${bots}${orphanReceipts}</div>`
    : '<p class="council-ledger-unavailable">Root round and reviewer details are Unknown in this snapshot.</p>';
  const stale = status?.stale ? ' · stale' : '';
  return `<section class="council-inspector-hierarchy" aria-label="Council review hierarchy"><h4>Review hierarchy</h4><details class="council-inspector-node" data-level="root-council" data-cycle-id="${escape(value.cycleId)}" data-stale="${Boolean(status?.stale)}" open><summary>Root Council · ${escape(state)} · ${score}${stale}</summary><dl><div><dt>Root cycle</dt><dd>${escape(value.cycleId)}</dd></div><div><dt>Round</dt><dd>${status?.round == null ? 'Unknown' : String(status.round)}</dd></div><div><dt>Target</dt><dd>${target == null ? 'Unknown' : `${formatScore(target)} / 10`}</dd></div><div><dt>Freshness</dt><dd>${status?.stale == null ? 'Unknown' : status.stale ? 'Stale · work changed after review' : 'Current'}</dd></div></dl>${hierarchyBody}</details></section>`;
}

/** Render native's projected Council records as inert text; workspace references are decorated by the bound file view. */
export function councilReadOnlyPresenterHtml(value: CouncilLedgerView): string {
  if (value.unavailableMessage) return `<section class="council-ledger"><p class="council-ledger-unavailable">${escape(value.unavailableMessage)}</p></section>`;
  const findings = value.findings ?? [], dispositions = value.findingDispositions ?? [], receipts = value.reviewReceipts ?? [], children = value.childReceipts ?? [], usage = value.usage ?? [];
  if (!value.findings && !value.findingDispositions && !value.reviewReceipts && !value.childReceipts && !value.usage) {
    return '<section class="council-ledger"><p class="council-ledger-unavailable">This host snapshot does not include Council ledger records.</p></section>';
  }
  const dispositionByFinding = new Map<string, typeof dispositions>();
  for (const item of dispositions) dispositionByFinding.set(item.findingId, [...(dispositionByFinding.get(item.findingId) ?? []), item]);
  const workerById = new Map((value.workers ?? []).map(worker => [worker.id, worker]));
  const exceptions = dispositions.filter(item => item.disposition === 'waived_by_authorized_human');
  const stale = value.status?.stale ? '<p class="council-ledger-stale">Stale: native reports that the work changed after this Council review.</p>' : '';
  const accepted = value.status?.state === 'accepted'
    ? `<p>Accepted${value.acceptedBy ? ` by ${escape(value.acceptedBy)}` : ''}${value.acceptanceReason ? `: ${escape(value.acceptanceReason)}` : ''}</p>` : '';
  const findingMarkup = findings.map(finding => {
    const worker = workerById.get(finding.seatId), binding = worker?.reviewerBinding?.reviewerBotId;
    const attribution = `<div class="council-ledger-attribution"><strong>${escape(finding.cycleId)}</strong><span>Round ${escape(finding.roundId.split(':').at(-1) ?? '')} · ${escape(finding.roundId)}</span><span>${escape(finding.seatId)}</span><span>${escape(worker ? `${worker.name}${binding ? ` · Bot ${binding}` : ''}` : binding ? `Bot ${binding}` : finding.seatId)}</span><span>${escape(finding.artifactDigest)}</span></div>`;
    const evidence = [...finding.evidenceRefs, ...(finding.location?.file ? [finding.location.file] : [])];
    const linkedDispositions = dispositionByFinding.get(finding.id) ?? [];
    const dispositionMarkup = linkedDispositions.map(item => `<li class="council-ledger-disposition" data-disposition="${escape(item.disposition)}"><strong>${escape(item.disposition.replaceAll('_', ' '))} · ${escape(item.actorId)}</strong><p>${escape(item.rationale)}</p>${ledgerRefs(item.artifactLinks)}</li>`).join('');
    return `<article class="council-ledger-finding" data-severity="${escape(finding.severity)}"><header><strong>${escape(finding.category)} · ${escape(finding.severity)}</strong><span>${escape(finding.criterionId)}</span></header><p>${escape(finding.summary)}</p>${attribution}<details><summary>Evidence and accepted exceptions</summary>${ledgerRefs(evidence)}${finding.suggestedFix ? `<div class="council-ledger-suggestion"><strong>Suggested fix</strong><p>${escape(finding.suggestedFix)}</p></div>` : ''}${dispositionMarkup ? `<ul>${dispositionMarkup}</ul>` : ''}</details></article>`;
  }).join('');
  const reviewMarkup = receipts.map(receipt => `<article class="council-ledger-receipt"><header><strong>${escape(receipt.verdict)} · ${escape(receipt.cycleId)}</strong><span>revision ${receipt.terminalRevision}</span></header><p>Request ${escape(receipt.requestId)} · task ${escape(receipt.taskId ?? 'Unknown')} · seat ${escape(receipt.assignmentId ?? 'Unknown')}</p><p>Artifact ${escape(receipt.artifactDigest)} · policy ${escape(receipt.policyFingerprint)}</p>${ledgerRefs(receipt.evidenceRefs)}</article>`).join('');
  const childMarkup = children.map(receipt => `<article class="council-ledger-receipt"><header><strong>Child Council · ${escape(receipt.status)}</strong><span>${escape(receipt.botId)}</span></header><p>${escape(receipt.childCycleId)} · parent seat ${escape(receipt.parentSeatId)}</p><p>Artifact ${escape(receipt.artifactRevision)} · policy ${escape(receipt.policyFingerprint)}</p>${ledgerRefs(receipt.evidenceRefs)}</article>`).join('');
  const usageMarkup = usage.map(record => `<article class="council-ledger-usage"><header><strong>${escape(record.scope)} ${escape(record.phase)} · ${escape(record.terminalStatus)}</strong><span>${escape(record.reviewerId)}</span></header><p>${escape(record.strategy)} · ${escape(record.model ?? 'Unknown')} · ${escape(record.provider ?? 'Unknown')}</p><dl><div><dt>Elapsed</dt><dd>${usageValue(record.elapsedMs)}</dd></div><div><dt>Queue wait</dt><dd>${usageValue(record.queueWaitMs)}</dd></div><div><dt>Input tokens</dt><dd>${usageValue(record.inputTokens)}</dd></div><div><dt>Output tokens</dt><dd>${usageValue(record.outputTokens)}</dd></div><div><dt>Retries</dt><dd>${usageValue(record.retries)}</dd></div><div><dt>Escalations</dt><dd>${usageValue(record.escalations)}</dd></div><div><dt>Concurrent leaves</dt><dd>${usageValue(record.budgetConsumption.concurrentLeaves)}</dd></div><div><dt>Leaf reviews</dt><dd>${usageValue(record.budgetConsumption.leafReviews)}</dd></div><div><dt>Estimated cost</dt><dd>${usageValue(record.budgetConsumption.estimatedCostMicros)}</dd></div></dl>${record.costEstimate ? `<p>${escape(record.costEstimate.currency)} cost estimate · estimate only · ${escape(record.costEstimate.pricingSource)} ${escape(record.costEstimate.pricingVersion)}</p>` : ''}</article>`).join('');
  return `<section class="council-ledger" data-cycle-id="${escape(value.cycleId)}">${stale}${accepted}${councilHierarchyHtml(value)}${exceptions.length ? `<details class="council-ledger-exception"><summary>Authorized finding exceptions · ${exceptions.length}</summary><ul>${exceptions.map(item => `<li>${escape(item.findingId)} · ${escape(item.actorId)}: ${escape(item.rationale)}${ledgerRefs(item.artifactLinks)}</li>`).join('')}</ul></details>` : ''}<details><summary>Findings and dispositions · ${findings.length}</summary>${findingMarkup || '<p class="council-ledger-empty">No findings.</p>'}</details><details><summary>Review receipts · ${receipts.length}</summary>${reviewMarkup || '<p class="council-ledger-empty">No review receipts.</p>'}</details><details><summary>Child Council receipts · ${children.length}</summary>${childMarkup || '<p class="council-ledger-empty">No child Council receipts.</p>'}</details><details><summary>Usage and hard-limit records · ${usage.length}</summary><p class="council-ledger-usage-note">Measured values are reported as recorded; estimated values are marked estimate only.</p>${usageMarkup || '<p class="council-ledger-empty">No usage records.</p>'}</details></section>`;
}


/** The team as the composer shows it: native's snapshot, except that a sent Keep fixing reads as running and fixing at once,
 *  until native's snapshot moves on and replaces it. */
function shownTeam(team: TeamSnapshot): TeamSnapshot {
  if (team.councilStatus?.state !== 'asking' || !actions.get(team.sessionId)?.sent) return team;
  return { ...team, status: 'running', error: null, councilStatus: { ...team.councilStatus, state: 'fixing', notice: null } };
}
const settled = (state: string): boolean => state === 'passed' || state === 'accepted' || state === 'limit';
/** The tray button is settings only: on or off, and a cue while asking or once the cycle has settled (not accepted). */
function face(native: TeamSnapshot | null): { label: string; on: boolean; cue: '' | 'asking' | 'settled' } {
  const team = native && shownTeam(native), progress = team && councilProgress(team);
  if (!progress) return { label: 'Council: off', on: false, cue: '' };
  if (councilAsking(progress.status)) return { label: 'Council needs you', on: true, cue: 'asking' };
  if (progress.state === 'blocked' || progress.state === 'failed'
    || progress.state === 'inconclusive' || progress.state === 'stale') {
    return { label: `Council: ${councilStateText(progress, team!, lowered(team!))}`, on: true, cue: 'asking' };
  }
  const ended = (settled(progress.state) && progress.state !== 'accepted') || finishedBelow(progress, team!);
  return { label: progress.rounds.length ? `Council: ${councilStateText(progress, team!, lowered(team!))}` : `Council: on, target ${formatScore(progress.status.target)}`, on: true, cue: ended ? 'settled' : '' };
}

export function createCouncil(host: CouncilHost) {
  let panel: HTMLElement | undefined, panelSession = '', cleanup: (() => void) | undefined, panelCount = 0;
  /** The control that opened the panel (the composer button, or the strip's Lower target or Raise max rounds). */
  let opener: HTMLElement | undefined;
  /** Places the open panel again when its content changes, so nothing scrolls out of view. */
  let placeNow: (() => void) | undefined;
  /** Per thread: requests in flight, in order (one thread never waits on another's), with the newest asked-for council
   *  shown until they all answer; and a refusal not yet seen in the panel. */
  const pending = new Map<string, { tail: Promise<void>; count: number; asked: CouncilConfig | null }>(), failures = new Map<string, string>();
  const reviewerBots = new Map<string, ReviewerBot[]>();
  const reviewerLoads = new Map<string, string>();
  const reviewerDraft = new Map<string, ReviewerSelection>();
  const compositionDrafts = new Map<string, CompositionPanelDraft>();
  const selection = (sessionId: string): ReviewerSelection => {
    let draft = reviewerDraft.get(sessionId);
    const persisted = shownConfig(sessionId);
    // The panel may open before the saved team appears; never keep a provisional
    // empty reviewer roster after native restoration, or override manual ID edits.
    if (draft?.selected && !draft.idsEdited && draft.ids.length === 0 && persisted?.reviewerBotIds?.length) {
      draft = { ...draft, ids: [...persisted.reviewerBotIds],
        optionalSkills: [...(persisted.optionalSkillIds ?? [])],
        checkSkills: [...(persisted.requiredCheckSkillIds ?? [])] };
      reviewerDraft.set(sessionId, draft);
    }
    return draft ?? {
      selected: Boolean(persisted?.reviewerBotIds?.length),
      ids: [...(persisted?.reviewerBotIds ?? [])],
      optionalSkills: [...(persisted?.optionalSkillIds ?? [])],
      checkSkills: [...(persisted?.requiredCheckSkillIds ?? [])],
      scoped: Boolean(persisted?.reviewPermissions),
      fileScope: persisted?.reviewPermissions?.grants[0]?.resource ?? '',
      authorizationId: persisted?.reviewPermissions?.authorizationId,
    };
  };
  const scopedFile = (path: string): boolean => path.length > 0 && path.length <= 240
    && !path.startsWith('/') && !path.startsWith('\\') && !path.includes(':')
    && path.split(/[\\/]/).every(part => Boolean(part) && part !== '..');
  function selectionConfig(sessionId: string): CouncilConfig {
    const draft = selection(sessionId), council = shownConfig(sessionId) ?? councilDefaults();
    if (!draft.selected) {
      const next = { ...council, reviewerBotIds: undefined, optionalSkillIds: undefined, requiredCheckSkillIds: undefined, reviewPermissions: undefined };
      const composition = compositionState(sessionId);
      return composition.enabled ? withCouncilComposition(next, activeComposition(sessionId)) : next;
    }
    if (draft.ids.length < 2 || draft.ids.length > 8 || new Set(draft.ids).size !== draft.ids.length) throw new Error('Select 2 to 8 distinct reviewer bots.');
    const available = reviewerBots.get(sessionId) ?? [];
    const executor = host.executorBotId?.(sessionId);
    for (const id of draft.ids) {
      const bot = available.find(bot => bot.id === id);
      if (!bot || bot.paused || bot.available === false || bot.id === executor || host.harnessAvailable?.(bot.provider) === false) throw new Error(bot?.reason || 'A selected reviewer bot is unavailable or is the executor. Choose another bot.');
    }
    const reviewPermissions = draft.scoped ? {
      authorizationId: draft.authorizationId || '',
      grants: [{ tool: 'pandacode_open_file', action: 'call', resourceArgument: 'path', resource: draft.fileScope?.trim() || '' }],
    } : undefined;
    if (draft.scoped && (!reviewPermissions?.authorizationId || !scopedFile(reviewPermissions.grants[0].resource))) {
      throw new Error('Choose a relative file path to authorize one exact native host action.');
    }
    const allAssigned = new Set(available.filter(bot => draft.ids.includes(bot.id)).flatMap(bot => bot.skills ?? []));
    const optionalSkillIds = (draft.optionalSkills ?? []).filter(id => allAssigned.has(id));
    const requiredCheckSkillIds = (draft.checkSkills ?? []).filter(id => allAssigned.has(id) && !optionalSkillIds.includes(id));
    const next = { ...council, reviewerBotIds: [...draft.ids], optionalSkillIds: optionalSkillIds.length ? optionalSkillIds : undefined, requiredCheckSkillIds: requiredCheckSkillIds.length ? requiredCheckSkillIds : undefined, reviewPermissions };
    const composition = compositionState(sessionId);
    return composition.enabled ? withCouncilComposition(next, activeComposition(sessionId)) : next;
  }
  function selectedError(sessionId: string, error: string): void {
    failures.set(sessionId, error);
    host.announce(notChanged(error));
    paint(sessionId);
  }
  function loadReviewerBots(sessionId: string): void {
    reviewerLoads.set(sessionId, 'Loading available bots…');
    void host.call<ReviewerBot[]>('architect_council_reviewer_options', { sessionId }).then(bots => {
      reviewerBots.set(sessionId, bots);
      reviewerLoads.delete(sessionId);
      if (panelSession === sessionId) paint(sessionId);
    }).catch(cause => {
      reviewerLoads.set(sessionId, `Cannot load bots: ${cause instanceof Error ? cause.message : String(cause)}`);
      if (panelSession === sessionId) paint(sessionId);
    });
  }
  /** The rounds this cycle has run: Raise max rounds offers only more than that. */
  const roundsUsed = (sessionId: string): number => { const team = host.team(sessionId), progress = team && councilProgress(team); return Math.max(progress?.status.round ?? 0, progress?.latest?.number ?? 0); };
  const shownConfig = (sessionId: string): CouncilConfig | null => pending.has(sessionId) ? pending.get(sessionId)!.asked : host.team(sessionId)?.config.council ?? null;

  function compositionState(sessionId: string): CompositionPanelDraft {
    let state = compositionDrafts.get(sessionId);
    if (!state) {
      const stored = shownConfig(sessionId)?.composition ?? newCouncilComposition();
      state = { value: structuredClone(stored), enabled: Boolean(shownConfig(sessionId)?.composition), botsEnabled: stored.selected.length > 0, reviewers: new Map(stored.selected.map(item => [item.botId, structuredClone(item)])) };
      compositionDrafts.set(sessionId, state);
    }
    return state;
  }
  function activeComposition(sessionId: string): CouncilComposition {
    const state = compositionState(sessionId), value = state.value;
    const criteria = value.criteriaSource === 'hybrid' ? value.criteria
      : value.criteria.filter(criterion => criterion.source === criterionSourceFor(value.criteriaSource));
    return { ...value, selected: state.botsEnabled ? [...state.reviewers.values()].map(item => item.mode === 'nested'
      ? { ...item, required: true, nested: structuredClone(item.nested ?? defaultNested()) }
      : { botId: item.botId, mode: 'direct', required: true }) : [], criteria };
  }
  function defaultNested(): NestedCouncilConfig {
    return { template: 'explicit_task', childCriteriaSource: 'architect', childReviewerCount: 3, target: 9, maxRounds: 3, escalation: 'on_trigger' };
  }
  function compositionNodeSelector(node: HTMLElement): string | null {
    if (node.matches('.council-composition-body')) return '.council-composition-body';
    const criterionIndex = node.closest<HTMLElement>('.council-criterion')?.dataset.criterionIndex;
    const attribute = [...node.attributes].find(item => item.name.startsWith('data-composition-')
      && item.name !== 'data-composition-field' && item.name !== 'data-composition-field-error')
      ?? [...node.attributes].find(item => ['data-add-criterion', 'data-remove-criterion', 'data-plan-preview-summary', 'data-plan-preview-details'].includes(item.name));
    if (!attribute) {
      return node.matches('.council-criterion') && criterionIndex !== undefined
        ? `.council-criterion[data-criterion-index="${CSS.escape(criterionIndex)}"]` : null;
    }
    const value = `[${attribute.name}="${CSS.escape(attribute.value)}"]`;
    if (criterionIndex !== undefined && attribute.name === 'data-composition-criterion') return `.council-criterion[data-criterion-index="${CSS.escape(criterionIndex)}"] ${value}`;
    const botId = node.dataset.botId;
    if (botId && attribute.name === 'data-composition-nested') return `[data-bot-id="${CSS.escape(botId)}"]${value}`;
    return value;
  }
  function compositionScrollSnapshot(slot: HTMLElement): {
    panelTop: number; panelLeft: number; slotTop: number; slotLeft: number; windowX: number; windowY: number;
    descendants: { selector: string; top: number; left: number }[];
  } {
    return {
      panelTop: panel?.scrollTop ?? 0, panelLeft: panel?.scrollLeft ?? 0,
      slotTop: slot.scrollTop, slotLeft: slot.scrollLeft, windowX: window.scrollX, windowY: window.scrollY,
      descendants: [...slot.querySelectorAll<HTMLElement>('*')]
        .filter(node => node.scrollTop !== 0 || node.scrollLeft !== 0)
        .flatMap(node => { const selector = compositionNodeSelector(node); return selector ? [{ selector, top: node.scrollTop, left: node.scrollLeft }] : []; }),
    };
  }
  function restoreCompositionScroll(slot: HTMLElement, saved: ReturnType<typeof compositionScrollSnapshot>): void {
    if (panel) { panel.scrollTop = saved.panelTop; panel.scrollLeft = saved.panelLeft; }
    slot.scrollTop = saved.slotTop; slot.scrollLeft = saved.slotLeft;
    for (const item of saved.descendants) {
      const node = slot.querySelector<HTMLElement>(item.selector);
      if (node) { node.scrollTop = item.top; node.scrollLeft = item.left; }
    }
    if (window.scrollX !== saved.windowX || window.scrollY !== saved.windowY) window.scrollTo(saved.windowX, saved.windowY);
  }
  function replaceCompositionSlot(slot: HTMLElement, sessionId: string, preferredFocusSelector?: string | null): void {
    const active = slot.contains(document.activeElement) ? document.activeElement as HTMLElement : null;
    const selector = preferredFocusSelector === undefined ? (active ? compositionNodeSelector(active) : null) : preferredFocusSelector;
    const selection = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
      ? (() => { try { return { start: active.selectionStart, end: active.selectionEnd, direction: active.selectionDirection }; } catch { return undefined; } })()
      : undefined;
    const scroll = compositionScrollSnapshot(slot);
    slot.innerHTML = compositionMarkup(sessionId);
    compositionStatus(sessionId);
    const next = selector ? slot.querySelector<HTMLElement>(selector) : null;
    if (next) {
      next.focus({ preventScroll: true });
      if (selection && (next instanceof HTMLInputElement || next instanceof HTMLTextAreaElement)
        && selection.start !== null && selection.end !== null) {
        try { next.setSelectionRange(selection.start, selection.end, selection.direction ?? undefined); } catch { /* Some input types do not support text selection. */ }
      }
    }
    restoreCompositionScroll(slot, scroll);
  }
  function compositionErrors(sessionId: string): string[] {
    return validateCouncilComposition(activeComposition(sessionId), shownConfig(sessionId)?.target ?? councilDefaults().target);
  }
  function compositionStatus(sessionId: string): void {
    if (!panel || panelSession !== sessionId) return;
    const slot = panel.querySelector<HTMLElement>('.council-composition-slot');
    const scroll = slot ? compositionScrollSnapshot(slot) : undefined;
    const issues = councilCompositionValidationIssues(activeComposition(sessionId), shownConfig(sessionId)?.target ?? councilDefaults().target);
    const errors = issues.map(issue => issue.message), alert = slot?.querySelector<HTMLElement>('[data-composition-errors]');
    if (alert) {
      alert.hidden = errors.length === 0;
      alert.innerHTML = errors.length ? `<strong>Fix these settings before applying</strong><ul>${errors.map(error => `<li>${escape(error)}</li>`).join('')}</ul>` : '';
    }
    if (slot) {
      renderCompositionFieldErrors(slot, issues);
      updateCompositionPlanPreview(slot, activeComposition(sessionId), reviewerBots.get(sessionId) ?? []);
    }
    const targetIssue = issues.find(issue => issue.field === 'councilTarget'), targetError = panel.querySelector<HTMLElement>('[data-composition-target-error]'), targetGroup = panel.querySelector<HTMLElement>('[data-field="target"]');
    if (targetError) { targetError.hidden = !targetIssue; targetError.textContent = targetIssue?.message ?? ''; }
    if (targetGroup) {
      if (targetIssue) targetGroup.setAttribute('aria-invalid', 'true'); else targetGroup.removeAttribute('aria-invalid');
      const id = validationId('councilTarget'), describedBy = new Set((targetGroup.getAttribute('aria-describedby') ?? '').split(/\s+/).filter(value => value && value !== id));
      if (targetIssue && targetError) describedBy.add(id);
      if (describedBy.size) targetGroup.setAttribute('aria-describedby', [...describedBy].join(' ')); else targetGroup.removeAttribute('aria-describedby');
    }
    const apply = slot?.querySelector<HTMLButtonElement>('[data-composition-apply]');
    if (apply) apply.disabled = errors.length > 0;
    if (slot && scroll) restoreCompositionScroll(slot, scroll);
  }
  function compositionMarkup(sessionId: string): string {
    const state = compositionState(sessionId), draft = state.value, active = activeComposition(sessionId);
    if (!state.enabled) return `<section class="council-composition-editor"><h3>Composition plan</h3><label class="council-composition-toggle"><input type="checkbox" data-composition-enabled><span>Use a schema v1 composition plan</span></label><p>Use direct bots, child Councils, Architect reviewers, criteria and bounded execution settings.</p></section>`;
    const modeId = councilCompositionMode(active);
    const modeReadout = modeId ?? 'Unavailable';
    const modeState = modeId
      ? `<p class="council-composition-mode-state" role="status" data-composition-mode-state="available">Current mode: <span class="council-composition-mode-readout">${modeReadout}</span></p>`
      : '<p class="council-composition-mode-state" role="status" data-composition-mode-state="unavailable">Current mode: <span class="council-composition-mode-readout">Unavailable</span> · No reviewer source is enabled. Enable automatic reviewers or select at least one bot reviewer.</p>';
    const bots = reviewerBots.get(sessionId) ?? [];
    const botIds = [...new Set([...bots.map(bot => bot.id), ...state.reviewers.keys()])];
    const botMarkup = botIds.map(id => {
      const bot = bots.find(item => item.id === id), selected = state.reviewers.get(id), mode = selected?.mode ?? 'direct';
      const nested = selected?.nested ?? defaultNested();
      const reviewerIndex = active.selected.findIndex(reviewer => reviewer.botId === id), reviewerPath = reviewerIndex >= 0 ? `reviewers.${reviewerIndex}.botId` : '';
      const modePath = reviewerIndex >= 0 ? `reviewers.${reviewerIndex}.mode` : '';
      const nestedPath = (field: string) => `reviewers.${reviewerIndex}.nested.${field}`;
      const nestedFields = selected && mode === 'nested' ? `<fieldset class="council-nested-fields"><legend>Child Council · ${escape(id)}</legend><label>Template<select data-composition-nested="template" data-bot-id="${escape(id)}"><option value="explicit_task" ${nested.template === 'explicit_task' ? 'selected' : ''}>Explicit task</option><option value="validated_default" ${nested.template === 'validated_default' ? 'selected' : ''}>Validated default</option></select></label><label>Criteria source<select data-composition-nested="childCriteriaSource" data-bot-id="${escape(id)}">${['architect','user','hybrid'].map(source => `<option value="${source}" ${nested.childCriteriaSource === source ? 'selected' : ''}>${source}</option>`).join('')}</select></label><label data-composition-field="${nestedPath('childReviewerCount')}">Child reviewers<input type="number" min="2" max="8" step="1" value="${nested.childReviewerCount}" data-composition-nested="childReviewerCount" data-bot-id="${escape(id)}">${fieldErrorMarkup(nestedPath('childReviewerCount'))}</label><label data-composition-field="${nestedPath('target')}">Target<input type="number" min="1" max="10" step="0.1" value="${nested.target}" data-composition-nested="target" data-bot-id="${escape(id)}">${fieldErrorMarkup(nestedPath('target'))}</label><label data-composition-field="${nestedPath('maxRounds')}">Max rounds<input type="number" min="1" max="100" step="1" value="${nested.maxRounds}" data-composition-nested="maxRounds" data-bot-id="${escape(id)}">${fieldErrorMarkup(nestedPath('maxRounds'))}</label><label>Escalation<select data-composition-nested="escalation" data-bot-id="${escape(id)}"><option value="on_trigger" ${nested.escalation === 'on_trigger' ? 'selected' : ''}>On trigger</option><option value="always" ${nested.escalation === 'always' ? 'selected' : ''}>Always</option></select></label></fieldset>` : '';
      return `<article class="council-composition-reviewer${bot && (bot.paused || bot.available === false) ? ' is-unavailable' : ''}"><label class="council-composition-bot"><input type="checkbox" data-composition-bot="${escape(id)}" ${reviewerPath ? `data-composition-field="${reviewerPath}"` : ''} ${selected ? 'checked' : ''}><span><strong>${escape(bot?.name ?? id)}</strong><small>${escape(bot ? `${bot.role || 'Reviewer'} · ${bot.provider} / ${bot.model}` : 'Selected reviewer · unavailable')}</small></span></label>${reviewerPath ? fieldErrorMarkup(reviewerPath) : ''}<label class="council-composition-reviewer-mode"><span>Reviewer mode</span><select data-composition-reviewer-mode="${escape(id)}" ${modePath ? `data-composition-field="${modePath}"` : ''} ${selected ? '' : 'disabled'}><option value="direct" ${mode === 'direct' ? 'selected' : ''}>Direct reviewer</option><option value="nested" ${mode === 'nested' ? 'selected' : ''}>Child Council</option></select></label>${modePath ? fieldErrorMarkup(modePath) : ''}${nestedFields}</article>`;
    }).join('');
    const shownCriteria = draft.criteria.map((item, index) => ({ item, index })).filter(({ item }) => draft.criteriaSource === 'hybrid' || item.source === criterionSourceFor(draft.criteriaSource));
    const criteriaMarkup = shownCriteria.map(({ item, index }) => {
      const path = (field: string) => `criteria.${index}.${field}`;
      return `<fieldset class="council-criterion" data-criterion-index="${index}"><legend>${escape(item.lens || item.id || 'Criterion')}</legend><label data-composition-field="${path('id')}">ID<input type="text" data-composition-criterion="id" maxlength="80" value="${escape(item.id)}">${fieldErrorMarkup(path('id'))}</label><label data-composition-field="${path('lens')}">Lens<input type="text" data-composition-criterion="lens" maxlength="80" value="${escape(item.lens)}">${fieldErrorMarkup(path('lens'))}</label><label data-composition-field="${path('description')}">Description<textarea data-composition-criterion="description" maxlength="4096" rows="2">${escape(item.description)}</textarea>${fieldErrorMarkup(path('description'))}</label><label data-composition-field="${path('target')}">Target<input type="number" data-composition-criterion="target" min="1" max="10" step="0.1" value="${item.target}">${fieldErrorMarkup(path('target'))}</label><label><input type="checkbox" data-composition-criterion="required" ${item.required ? 'checked' : ''}> Required</label><label><input type="checkbox" data-composition-criterion="evidenceRequired" ${item.evidenceRequired ? 'checked' : ''}> Evidence required</label><button type="button" class="ds-btn council-remove-criterion" data-remove-criterion-index="${index}">Remove criterion</button></fieldset>`;
    }).join('');
    const errors = compositionErrors(sessionId);
    const strategyOptions: readonly [CouncilStrategy, string][] = [['parallel','Parallel'], ['escalated','Escalated'], ['two_phase','Two phase']];
    const planPreview = compositionPlanPreview(active, bots);
    const limitFields = ([['maxDepth','Maximum depth'],['maxRootSeats','Maximum root seats'],['maxConcurrentLeaves','Concurrent leaves'],['maxLeafReviewsPerRound','Leaf reviews per round'],['timeoutMs','Timeout (ms)'],['maxEstimatedCostMicros','Estimated cost cap (micro-units)']] as const).map(([key,label]) => `<label data-composition-field="limits.${key}">${label}<input type="number" data-composition-limit="${key}" min="1" value="${draft.limits[key]}">${fieldErrorMarkup(`limits.${key}`)}</label>`).join('');
    return `<section class="council-composition-editor"><h3>Composition plan</h3><label class="council-composition-toggle"><input type="checkbox" data-composition-enabled checked><span>Use a schema v1 composition plan</span></label><div class="council-composition-body">${modeState}<ol class="council-composition-modes">${COUNCIL_COMPOSITION_MODES.map(mode => `<li data-composition-mode="${mode.id}" aria-current="${mode.id === modeId}"><strong>${mode.id}</strong><span>${escape(mode.label)}</span></li>`).join('')}</ol><label class="council-composition-toggle"><input type="checkbox" data-composition-automatic data-focus-key="automatic-toggle" ${draft.automatic.enabled ? 'checked' : ''}><span>Include automatic Architect reviewers</span></label><label class="council-composition-control" data-composition-field="automatic.count">Automatic reviewer count<select data-composition-automatic-count data-composition-field="automatic.count" data-focus-key="automatic-count" ${draft.automatic.enabled ? '' : 'disabled'}><option value="auto" ${draft.automatic.count === 'auto' ? 'selected' : ''}>Auto</option>${Array.from({length:8},(_,index)=>index+1).map(count => `<option value="${count}" ${draft.automatic.count === count ? 'selected' : ''}>${count}</option>`).join('')}</select>${fieldErrorMarkup('automatic.count')}</label><label class="council-composition-toggle"><input type="checkbox" data-composition-bots-enabled ${state.botsEnabled ? 'checked' : ''}><span>Include selected bot reviewers</span></label><div class="council-composition-bots" role="group" aria-label="Selected reviewer bots" data-composition-field="reviewers">${botMarkup || '<p class="council-composition-missing">No configured reviewer bots are available.</p>'}${fieldErrorMarkup('reviewers')}</div><label class="council-composition-control" data-composition-field="strategy">Execution strategy<select data-composition-strategy>${strategyOptions.map(([strategy,label]) => `<option value="${strategy}" ${draft.strategy === strategy ? 'selected' : ''}>${label}</option>`).join('')}</select>${fieldErrorMarkup('strategy')}</label><label class="council-composition-control" data-composition-field="criteriaSource">Criteria source<select data-composition-criteria-source><option value="architect" ${draft.criteriaSource === 'architect' ? 'selected' : ''}>Architect</option><option value="user" ${draft.criteriaSource === 'user' ? 'selected' : ''}>User</option><option value="hybrid" ${draft.criteriaSource === 'hybrid' ? 'selected' : ''}>Hybrid</option></select>${fieldErrorMarkup('criteriaSource')}</label><section class="council-composition-criteria" role="group" aria-label="Criteria" data-composition-field="criteria"><h3>Criteria</h3>${criteriaMarkup}<span><button type="button" class="ds-btn" data-add-criterion="user">Add user criterion</button> <button type="button" class="ds-btn" data-add-criterion="architect">Add Architect criterion</button></span>${fieldErrorMarkup('criteria')}</section><fieldset class="council-composition-limits"><legend>Hard limits</legend>${limitFields}</fieldset><details class="council-plan-preview"><summary data-plan-preview-summary>${escape(planPreview.summary)}</summary><div data-plan-preview-details>${planPreview.details}</div></details><div class="council-composition-errors" data-composition-errors role="alert" ${errors.length ? '' : 'hidden'}>${errors.length ? `<strong>Fix these settings before applying</strong><ul>${errors.map(error => `<li>${escape(error)}</li>`).join('')}</ul>` : ''}</div><button type="button" class="ds-btn-primary council-composition-apply" data-composition-apply ${errors.length ? 'disabled' : ''}>Apply composition</button></div></section>`;
  }
  function updateCompositionField(sessionId: string, control: HTMLElement): boolean {
    const state = compositionState(sessionId), input = control as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;
    if (control.hasAttribute('data-composition-enabled')) { state.enabled = (control as HTMLInputElement).checked; if (state.enabled && !state.value) state.value = newCouncilComposition(); return true; }
    if (control.hasAttribute('data-composition-automatic')) {
      const enabled = (control as HTMLInputElement).checked, count = state.value.automatic.count;
      state.value.automatic = { ...state.value.automatic, enabled, count: enabled ? typeof count === 'number' && count >= 1 ? count : 'auto' : 0 };
      return true;
    }
    if (control.hasAttribute('data-composition-bots-enabled')) { state.botsEnabled = (control as HTMLInputElement).checked; return true; }
    if (control.hasAttribute('data-composition-automatic-count')) { state.value.automatic.count = input.value === 'auto' ? 'auto' : Number(input.value); return true; }
    if (control.hasAttribute('data-composition-strategy')) { state.value.strategy = input.value as CouncilStrategy; return true; }
    if (control.hasAttribute('data-composition-criteria-source')) { state.value.criteriaSource = input.value as CouncilComposition['criteriaSource']; return true; }
    if (control.hasAttribute('data-composition-bot')) {
      const checkbox = control as HTMLInputElement, id = checkbox.dataset.compositionBot!;
      if (checkbox.checked) state.reviewers.set(id, state.reviewers.get(id) ?? { botId: id, mode: 'direct', required: true });
      else state.reviewers.delete(id);
      return true;
    }
    if (control.hasAttribute('data-composition-reviewer-mode')) {
      const select = control as HTMLSelectElement, id = select.dataset.compositionReviewerMode!, old = state.reviewers.get(id);
      if (old) state.reviewers.set(id, select.value === 'nested' ? { ...old, mode: 'nested', required: true, nested: old.nested ?? defaultNested() } : { ...old, mode: 'direct', required: true });
      return true;
    }
    if (control.hasAttribute('data-composition-nested')) {
      const id = input.dataset.botId!, field = control.dataset.compositionNested as keyof NestedCouncilConfig, old = state.reviewers.get(id);
      if (old) {
        const nested = { ...(old.nested ?? defaultNested()), [field]: field === 'childReviewerCount' || field === 'maxRounds' || field === 'target' ? Number(input.value) : input.value } as NestedCouncilConfig;
        state.reviewers.set(id, { ...old, mode: 'nested', nested });
      }
      return true;
    }
    if (control.hasAttribute('data-composition-limit')) {
      const key = control.dataset.compositionLimit as keyof CouncilComposition['limits'];
      state.value.limits[key] = Number(input.value);
      return true;
    }
    if (control.hasAttribute('data-composition-criterion')) {
      const field = control.dataset.compositionCriterion!, criterionIndex = Number(control.closest<HTMLElement>('.council-criterion')?.dataset.criterionIndex);
      const criterion = Number.isInteger(criterionIndex) ? state.value.criteria[criterionIndex] : undefined;
      if (!criterion) return true;
      const raw = field === 'required' || field === 'evidenceRequired' ? (control as HTMLInputElement).checked : input.value;
      (criterion as unknown as Record<string, unknown>)[field] = field === 'target' ? Number(raw) : raw;
      return true;
    }
    return false;
  }
  function addCriterion(sessionId: string, source: 'user' | 'architect'): void {
    const state = compositionState(sessionId), suffix = Math.random().toString(36).slice(2, 8);
    state.value.criteria.push({ id: `criterion-${suffix}`, lens: source === 'user' ? 'User criterion' : 'Architect criterion', description: `Review the work for ${source === 'user' ? 'the requested criterion' : 'this criterion'}.`, required: true, target: shownConfig(sessionId)?.target ?? councilDefaults().target, evidenceRequired: true, source });
  }

  /** The visible thread's state text and scored seats as last seen, so a change is read aloud once. Seeing another
   *  thread (or council turning on) only sets a new baseline: what changed while a thread was not shown is never replayed. */
  let heard: { sessionId: string; text: string; scored: Set<string> } | undefined;
  function observe(sessionId: string): void {
    const native = host.team(sessionId), team = native && shownTeam(native), progress = team && councilProgress(team);
    const text = progress ? councilStateText(progress, team, lowered(team!)) : '', scored = progress?.rounds.flatMap(round => round.seats).filter(seat => seatScore(seat) !== null) ?? [];
    const before = heard?.sessionId === sessionId && heard.text ? heard : undefined;
    heard = { sessionId, text, scored: new Set(scored.map(seat => seat.id)) };
    if (!before || !progress) return;
    const news = scored.filter(seat => !before.scored.has(seat.id)).map(seat => `${seat.council.lens.trim() || seat.name} scored ${outOfTen(seatScore(seat)!, progress.status.target)}`);
    if (text !== before.text) {
      // A new state ends what the strip's last action said (a sent Keep fixing waits for native to leave asking).
      if (!(actions.get(sessionId)?.sent && native!.councilStatus?.state === 'asking')) actions.delete(sessionId);
      const { lowest, state } = progress.status, detail = lowest !== null && (state === 'fixing' || state === 'limit' || state === 'accepted') ? `, lowest ${outOfTen(lowest, progress.status.target)}` : '';
      news.push(`Council: ${text}${detail}${councilAsking(progress.status) ? `. ${councilNotice(team!)}` : ''}`);
    }
    if (news.length) host.announce(news.join('. '));
  }

  function button(sessionId: string): string {
    threadShown(sessionId);
    // The composer is drawing another thread: the panel belongs to the one it left.
    if (panel && panelSession !== sessionId) close();
    if (host.team(sessionId)) loaded(sessionId);
    observe(sessionId);
    const { label, on, cue } = face(host.team(sessionId));
    // The composer mounts this markup after it returns.
    faceSoon(sessionId);
    return `<button type="button" class="composer-council-button" data-action="council" data-session-id="${escape(sessionId)}" aria-haspopup="dialog" aria-expanded="${Boolean(panel) && panelSession === sessionId}" aria-busy="${pending.has(sessionId)}" ${on ? 'data-on' : ''} aria-label="${escape(label)}" data-tip="${escape(label)}"><span class="council-glyph" data-cue="${cue}" aria-hidden="true">${COUNCIL_ICON}</span><span class="sr-only">Council</span></button>`;
  }
  /** The strip's slot inside the prompt box: always there (empty while hidden), so the strip updates in place. */
  function strip(sessionId: string): string {
    const team = host.team(sessionId), html = team ? stripHtml(team) : '';
    // A missing team cache is not a loaded reviewer cycle: avoid beginning the
    // motion quiet window until the first actual team snapshot is available.
    if (team) loaded(sessionId);
    // An explicit empty strip is the baseline for future Council round entrance.
    // A strip already present on first paint is not a newly started review.
    if (!html && (team || host.loaded(sessionId))) motion(sessionId, 'strip', '', null, () => []);
    return `<div class="council-strip-slot" data-session-id="${escape(sessionId)}">${html}</div>`;
  }

  function paint(sessionId: string): void {
    if (!panel || panelSession !== sessionId) return;
    const shown = shownConfig(sessionId), values = shown ?? councilDefaults();
    // One description under the title: what Council does, then what turning it on costs (or, off, what the choices do).
    const about = `${ABOUT} ${shown ? NOTE_ON : host.team(sessionId) ? NOTE_OFF : NOTE_NO_TEAM}`, aboutLine = panel.querySelector<HTMLElement>('#council-about')!;
    if (aboutLine.textContent !== about) aboutLine.textContent = about;
    // Council on, the team resting, and no round yet: say so quietly.
    const team = host.team(sessionId), idle = Boolean(shown && team && !teamActive(team) && !councilProgress(team)?.rounds.length);
    const idleLine = panel.querySelector<HTMLElement>('.council-idle')!;
    if (idleLine.hidden === idle) idleLine.hidden = !idle;
    // Raise max rounds: how many this cycle has used, under its title.
    if (panel.dataset.compact === 'maxRounds') {
      const used = `${roundsUsed(sessionId)} of ${values.maxRounds} rounds used`, usedLine = panel.querySelector<HTMLElement>('#council-maxRounds')!;
      if (usedLine.textContent !== used) usedLine.textContent = used;
    }
    const failure = failures.get(sessionId) ?? '';
    panel.querySelector<HTMLInputElement>('.council-switch')!.checked = Boolean(shown);
    if (!panel.dataset.compact) {
      const draft = selection(sessionId), bots = reviewerBots.get(sessionId) ?? [], executor = host.executorBotId?.(sessionId);
      const list = panel.querySelector<HTMLElement>('.council-bots')!;
      const assignedSkills = [...new Set(bots.filter(bot => draft.ids.includes(bot.id)).flatMap(bot => bot.skills ?? []))].sort();
      const missing = draft.ids.filter(id => !bots.some(bot => bot.id === id)).map(id => `<label class="council-bot-choice is-unavailable"><input type="checkbox" data-reviewer-bot="${escape(id)}" checked><span class="council-bot-info"><strong>${escape(id)}</strong><small>Selected reviewer · remove to choose another bot</small></span><span class="council-bot-state">${reviewerLoads.has(sessionId) ? 'Unavailable' : 'Missing bot'}</span></label>`).join('');
      const markup = draft.selected ? `<p class="council-bot-heading">Task reviewers · 2–8 distinct bots</p>${reviewerLoads.has(sessionId) ? `<p role="status">${escape(reviewerLoads.get(sessionId)!)}</p>` : ''}${bots.length ? bots.map(bot => {
        const unavailable = bot.paused || bot.available === false || bot.id === executor || host.harnessAvailable?.(bot.provider) === false;
        // An unavailable reviewer cannot be added, but an existing selection must remain removable.
        const checked = draft.ids.includes(bot.id), disabled = unavailable && !checked;
        const reason = bot.id === executor ? 'Executor' : bot.paused ? 'Paused' : bot.available === false ? (bot.reason || 'Unavailable') : host.harnessAvailable?.(bot.provider) === false ? 'Harness unavailable' : 'Available';
        return `<label class="council-bot-choice${unavailable ? ' is-unavailable' : ''}"><input type="checkbox" data-reviewer-bot="${escape(bot.id)}" ${checked ? 'checked' : ''} ${disabled ? 'disabled' : ''}><span class="council-bot-info"><strong>${escape(bot.name)}</strong><small>${escape(bot.role || 'Reviewer')} · ${escape(bot.provider)} / ${escape(bot.model)}</small></span><span class="council-bot-state">${escape(reason)}</span></label>`;
      }).join('') : !reviewerLoads.has(sessionId) ? '<p role="status">No configured bots available.</p>' : ''}${missing}<p class="council-bot-hint">${draft.ids.length} of 2–8 reviewers selected${draft.ids.length < 2 ? ' · select at least two to apply' : ''}</p>${draft.ids.length >= 2 && new Set(draft.ids.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is ReviewerBot => Boolean(bot)).map(bot => `${bot.provider}:${bot.model}`)).size < draft.ids.filter(id => bots.some(bot => bot.id === id)).length ? '<p class="council-bot-hint">These bots share a provider/model; model diversity is reduced.</p>' : ''}<div class="council-review-permissions" role="group" aria-label="Optional reviewer skills"><small>Optional skills (unavailable items are reported; required skills always block)</small>${assignedSkills.length ? assignedSkills.map(id => `<label><input type="checkbox" data-council-optional-skill="${escape(id)}" ${draft.optionalSkills?.includes(id) ? 'checked' : ''}><span>${escape(id)}</span></label>`).join('') : '<small>No assigned skills in selected bots.</small>'}</div><div class="council-review-permissions" role="group" aria-label="Required native skill checks"><small>Require a native pinned-input integrity check before Council can pass. This is not a project test.</small>${assignedSkills.length ? assignedSkills.map(id => `<label><input type="checkbox" data-council-required-check="${escape(id)}" ${draft.checkSkills?.includes(id) ? 'checked' : ''}><span>${escape(id)}</span></label>`).join('') : '<small>No assigned skills.</small>'}</div><div class="council-review-permissions"><label><input type="checkbox" data-council-scoped ${draft.scoped ? 'checked' : ''}> Allow one exact native host action (file inspector)</label>${draft.scoped ? `<label>Relative file path<input type="text" data-council-path maxlength="240" autocomplete="off" placeholder="src/example.ts" value="${escape(draft.fileScope ?? '')}" aria-label="Allowed reviewer file path"></label>` : ''}<small>Read-only by default; this per-task action never grants shell or checkout writes.</small></div>` : '';
      const focusedBot = (document.activeElement as HTMLElement | null)?.closest<HTMLInputElement>('[data-reviewer-bot]')?.dataset.reviewerBot;
      if (list.innerHTML !== markup) {
        // A checked input's property is not always reconciled by a DOM attribute morph.
        // Recreate only the bot rows; preserve keyboard focus and scroll position.
        const scroll = list.scrollTop;
        list.innerHTML = markup;
        list.scrollTop = scroll;
      }
      if (focusedBot && !list.contains(document.activeElement)) {
        const next = list.querySelector<HTMLInputElement>(`[data-reviewer-bot="${CSS.escape(focusedBot)}"]:not(:disabled)`) ?? list.querySelector<HTMLInputElement>('input:not(:disabled)');
        next?.focus({ preventScroll: true });
      }
      list.hidden = !draft.selected;
      const compositionSlot = panel.querySelector<HTMLElement>('.council-composition-slot')!;
      const compositionHtml = compositionMarkup(sessionId);
      if (compositionSlot.innerHTML !== compositionHtml) compositionSlot.innerHTML = compositionHtml;
      compositionStatus(sessionId);
    }
    // Which row had focus before this paint (its focused option may be hidden below).
    const hadFocus = document.activeElement?.closest<HTMLElement>('.council-panel [data-field]') ?? null;
    for (const { key, options, name } of FIELDS) {
      const group = panel.querySelector<HTMLElement>(`[data-field="${key}"]`)!;
      // A value set elsewhere (native allows any target, 1-100 rounds) shows as one more, selected option.
      let extra = group.querySelector<HTMLElement>('[data-extra]');
      if (options.includes(values[key])) extra?.remove();
      else {
        if (!extra) { extra = document.createElement('button'); extra.setAttribute('type', 'button'); extra.dataset.extra = ''; }
        extra.dataset.value = String(values[key]); extra.textContent = name(values[key]);
        // In numeric order among the offered options.
        const after = [...group.querySelectorAll<HTMLElement>(':scope > button:not([data-extra])')].find(option => Number(option.dataset.value) > values[key]) ?? null;
        if (extra.nextElementSibling !== after || extra.parentElement !== group) group.insertBefore(extra, after);
      }
      // A value that arrives while the panel is open fades in; the panel's own opening is the first sight.
      motion(sessionId, `panel${panelCount}:extra:${key}`, extra?.isConnected ? extra.dataset.value! : '', extra?.isConnected ? extra : null, element => fade(element));
      let selected: HTMLElement | null = null;
      const buttons = [...group.querySelectorAll<HTMLElement>('button')];
      for (const option of buttons) {
        const selectingBots = selection(sessionId).selected;
        const on = option.dataset.reviewersSource === 'selected'
          ? selectingBots
          : key !== 'reviewers' || !selectingBots ? Number(option.dataset.value) === values[key] : false;
        option.setAttribute('aria-pressed', String(on));
        if (on) selected = option;
      }
      // Compact from the strip: Lower target offers the current target and those below it; Raise max rounds only more rounds
      // than the team has used.
      if (panel.dataset.compact === 'target' && key === 'target') for (const option of buttons) option.hidden = Number(option.dataset.value) >= values.target;
      if (panel.dataset.compact === 'maxRounds' && key === 'maxRounds') for (const option of buttons) option.hidden = Number(option.dataset.value) <= roundsUsed(sessionId);
      // One Tab stop per row: the selected option (else, raising max rounds, your default limit when it is more than the rounds
      // used, else the first shown); arrows move within it.
      // (Lower target: the highest target below the current one.)
      const preferred = panel.dataset.compact === 'maxRounds' && key === 'maxRounds' ? buttons.find(option => !option.hidden && Number(option.dataset.value) === councilDefaults().maxRounds)
        : panel.dataset.compact === 'target' && key === 'target' ? buttons.filter(option => !option.hidden).at(-1) : undefined;
      const stop = selected && !selected.hidden ? selected : preferred ?? buttons.find(option => !option.hidden);
      for (const option of buttons) option.tabIndex = option === stop ? 0 : -1;
      // An option that had focus and is now hidden (Lower target to a value still above the lowest) hands focus to the row's
      // new Tab stop, so focus never drops to <body>.
      const focused = buttons.find(option => option === document.activeElement);
      if (focused?.hidden || (!document.activeElement || document.activeElement === document.body) && hadFocus === group) stop?.focus({ preventScroll: true });
      trackSegment(group, selected);
    }
    const error = panel.querySelector<HTMLElement>('.council-error')!, failed = failure && notChanged(failure);
    if (error.textContent !== failed) error.textContent = failed;
    error.hidden = !failure;
    // Beside Target score: native's current lowest, so lowering it is an informed choice.
    const status = host.team(sessionId)?.councilStatus, lowest = status?.lowest, open = status?.state === 'reviewing' || status?.state === 'dispatch';
    const lowestNote = shown && typeof lowest === 'number' ? ` · lowest ${open ? 'so far' : 'now'} ${formatScore(lowest)}` : '';
    const lowestLine = panel.querySelector<HTMLElement>('.council-lowest')!;
    if (lowestLine.textContent !== lowestNote) lowestLine.textContent = lowestNote;
    panel.querySelector<HTMLInputElement>('.council-new-threads')!.checked = councilForNewThreads();
    // The switch is busy while an automatic team starts for it.
    const sw = panel.querySelector<HTMLInputElement>('.council-switch')!, starting = pending.has(sessionId) && !host.team(sessionId);
    if (sw.getAttribute('aria-busy') !== String(starting)) sw.setAttribute('aria-busy', String(starting));
    placeNow?.();
  }

  /** Keeps the composer's thread's button, strip and open panel on the true state without redrawing the composer. */
  function sync(sessionId: string): void {
    if (panel && panelSession !== sessionId) close();
    if (host.team(sessionId)) loaded(sessionId);
    refresh(sessionId);
  }
  /** A compact panel belongs to the strip control that opened it. A redrawn strip hands it the same control; once the
   *  control is gone (the target passed, the limit raised) the panel closes and focus goes to the composer's prompt. */
  function keepOpener(): void {
    if (!panel?.dataset.compact || opener?.isConnected) return;
    const again = document.querySelector<HTMLElement>(`#composer .council-strip [data-council-focus="${panel.dataset.compact}"]`);
    if (again) { opener = again; again.setAttribute('aria-expanded', 'true'); return; }
    opener = undefined;
    close();
    document.querySelector<HTMLElement>('#prompt')?.focus({ preventScroll: true });
  }
  /** Once per frame, on whichever button is mounted then: asking pops the glyph (the composer may rebuild the button). */
  let faceFrame = 0, faceThread = '';
  function faceSoon(sessionId: string): void {
    faceThread = sessionId;
    faceFrame ||= requestAnimationFrame(() => {
      faceFrame = 0;
      const live = document.querySelector<HTMLElement>(BUTTON);
      if (live?.dataset.sessionId !== faceThread) return;
      const { cue } = face(host.team(faceThread));
      motion(faceThread, 'face-cue', cue, live.querySelector<HTMLElement>('.council-glyph'), element => cue === 'asking' ? pop(element) : []);
    });
  }
  function refresh(sessionId: string): void {
    const current = document.querySelector<HTMLElement>(BUTTON);
    if (current?.dataset.sessionId === sessionId) {
      observe(sessionId);
      const { label, on, cue } = face(host.team(sessionId));
      if (current.getAttribute('aria-label') !== label) {
        current.setAttribute('aria-label', label); current.dataset.tip = label;
        // Its tooltip, if showing, follows: the tooltip module has no update call, so the bubble's text changes here and a
        // scroll event (which the module listens for) places it again.
        const bubble = document.querySelector<HTMLElement>('.tip-bubble.tip-visible');
        if (bubble && (current.matches(':hover') || current.contains(document.activeElement))) { bubble.textContent = label; document.dispatchEvent(new Event('scroll')); }
      }
      current.toggleAttribute('data-on', on); current.querySelector<HTMLElement>('.council-glyph')!.dataset.cue = cue;
      current.setAttribute('aria-busy', String(pending.has(sessionId)));
      faceSoon(sessionId);
    }
    const slot = document.querySelector<HTMLElement>(`#composer .council-strip-slot[data-session-id="${CSS.escape(sessionId)}"]`), team = host.team(sessionId);
    if (slot) paintStrip(slot, team, host.loaded(sessionId));
    paint(sessionId);
    keepOpener();
  }

  /** Native answers in the order asked; what stays shown is the newest request, then native's answer. */
  /** `remember`: save as the default for new threads (the composer panel), not when opened from an asking card. */
  function apply(sessionId: string, next: CouncilConfig | null, remember = true): Promise<void> {
    const entry = pending.get(sessionId) ?? { tail: Promise.resolve(), count: 0, asked: next };
    entry.count++; entry.asked = next;
    pending.set(sessionId, entry); failures.delete(sessionId);
    // The button and the panel show the change as on its way.
    refresh(sessionId);
    entry.tail = entry.tail.then(async () => {
      let reason = '';
      try {
        if (next && !host.team(sessionId) && !await host.startTeam(sessionId)) {
          // The user moved to another thread while the team started: nothing to report.
          if (document.querySelector<HTMLElement>(BUTTON)?.dataset.sessionId !== sessionId) return;
          throw new Error('The team could not start. Try again.');
        }
        const was = host.team(sessionId);
        const team = await host.call<TeamSnapshot | null>('architect_set_council', { sessionId, council: next });
        // A lowered target that is still above the lowest score: say so.
        const lowest = was?.councilStatus?.lowest;
        if (next && was?.config.council && next.target < was.config.council.target && typeof lowest === 'number' && lowest < next.target) host.announce(`Target ${formatScore(next.target)}. Lowest score ${formatScore(lowest)} is still below it.`);
        if (team) host.updated(team);
        else if (next) throw new Error('This task has no team.');
        if (next && remember) saveCouncilDefaults(next);
        // Turning Council on or off is read aloud too (the badge and panel change silently otherwise).
        if (next && !was?.config.council) host.announce(`Council is on. Target ${formatScore(next.target)}.`);
        else if (!next && was?.config.council) host.announce('Council is off.');
      } catch (cause) { reason = cause instanceof Error ? cause.message : String(cause); }
      finally {
        // Only the newest request's outcome is shown; an older one's was overtaken.
        if (--entry.count) return;
        pending.delete(sessionId);
        // Keep the UI synchronized with the native snapshot after success or refusal.
        // A rejected optimistic selected roster must not remain as though it were persisted.
        reviewerDraft.delete(sessionId);
        if (reason) {
          failures.set(sessionId, reason);
          // Closed since: say it now, and show it when the panel opens again for this thread.
          // Read aloud whether or not the panel is open (it also shows it, in its alert).
          host.announce(notChanged(reason));
        }
        refresh(sessionId);
      }
    });
    return entry.tail;
  }

  function close(focus = false): void {
    if (!panel) return;
    const closing = panel;
    // A refusal seen in the panel is done with; one that lands after closing waits for the next open.
    failures.delete(panelSession);
    panel = undefined; panelSession = ''; placeNow = undefined; cleanup?.(); cleanup = undefined;
    hidePopover(closing);
    const current = document.querySelector<HTMLElement>(BUTTON), back = opener?.isConnected ? opener : current;
    current?.setAttribute('aria-expanded', 'false'); opener?.setAttribute('aria-expanded', 'false'); opener = undefined; expandedOpener = '';
    if (focus) back?.focus({ preventScroll: true });
  }

  /** `focusField`: the row to focus first (Lower target opens on Target score); otherwise the switch. */
  function open(anchor: HTMLElement, sessionId: string, focusField?: Field): void {
    if (panel && panelSession === sessionId) { close(); return; }
    close();
    panelSession = sessionId; opener = anchor; panelCount++;
    expandedOpener = anchor.dataset.councilFocus ? `${sessionId}:${anchor.dataset.councilFocus}` : '';
    panel = document.createElement('div');
    panel.className = 'floating-menu council-panel';
    // From the strip (Lower target, Raise max rounds): compact, that one row, no intro.
    if (anchor.closest('.council-strip')) panel.dataset.compact = focusField ?? 'target';
    panel.setAttribute('role', 'dialog');
    panel.setAttribute('aria-labelledby', 'council-title');
     panel.innerHTML = `<div class="council-panel-head"><label id="council-title" for="council-switch">Council</label><input id="council-switch" class="ds-toggle council-switch" type="checkbox" role="switch" tabindex="0" aria-describedby="council-about"></div><p id="council-about">${ABOUT}</p><p class="council-idle" role="status" hidden>No changes to review yet</p>${FIELDS.map(({ key, label, options, name }) => `<div class="council-field" data-field-row="${key}"><span id="council-${key}">${label}${key === 'target' ? '<small class="council-lowest"></small>' : ''}</span><div class="ds-seg" role="group" aria-labelledby="council-${key}" data-field="${key}">${options.map(value => `<button type="button" data-value="${value}" aria-pressed="false"${key === 'reviewers' && !value ? ` aria-label="${AUTO}" data-tip="${AUTO}"` : ''}>${name(value)}</button>`).join('')}${key === 'reviewers' ? '<button type="button" data-reviewers-source="selected" aria-label="Selected bots" data-tip="Review with existing configured bots" aria-pressed="false">Bots</button>' : ''}</div></div>${key === 'target' ? `<small class="council-field-error council-target-error" id="${validationId('councilTarget')}" data-composition-target-error hidden></small>` : ''}`).join('')}<div class="council-bots" role="group" aria-label="Selected reviewer bots" hidden></div><div class="council-composition-slot">${compositionMarkup(sessionId)}</div><p class="council-error" role="alert" hidden></p><div class="council-foot"><label class="council-new"><input type="checkbox" class="council-new-threads" tabindex="0"><span>Start new threads with Council on</span></label></div>`;
    document.body.append(panel);
    // Compact (from the strip): only the row it opened on, titled by what it does (a heading, no longer the switch's label).
    if (panel.dataset.compact) {
      for (const row of panel.querySelectorAll<HTMLElement>('[data-field-row]')) row.hidden = row.dataset.fieldRow !== panel.dataset.compact;
      const title = panel.querySelector<HTMLElement>('#council-title')!;
      title.removeAttribute('for'); title.textContent = panel.dataset.compact === 'target' ? 'Lower target' : 'Raise max rounds';
      // Raise max rounds: the title names the row and its line says how many rounds were used (said once each).
      if (panel.dataset.compact === 'maxRounds') {
        const group = panel.querySelector<HTMLElement>('[data-field="maxRounds"]')!;
        group.setAttribute('aria-labelledby', 'council-title'); group.setAttribute('aria-describedby', 'council-maxRounds');
      }
    }
    const place = () => {
      // Anchored to the control that opened it; the composer button again if that one is gone (a redraw).
      const current = opener?.isConnected ? opener : document.querySelector<HTMLElement>(BUTTON);
      if (!panel || !current) return;
      const rect = current.getBoundingClientRect(), width = Math.min(selection(sessionId).selected && !panel.dataset.compact ? 340 : PANEL_WIDTH, window.innerWidth - 24);
      panel.style.width = `${width}px`;
      panel.style.left = `${Math.max(12, Math.min(window.innerWidth - width - 12, rect.left + rect.width / 2 - width / 2))}px`;
      // Above it when it fits (the composer), else on the roomier side; never over the opener: capped to the space and
      // scrolling inside.
      panel.style.maxHeight = '';
      // Its full height with borders (max-height counts them), so a panel with room never scrolls.
      const room = { above: rect.top - 22, below: window.innerHeight - rect.bottom - 22 }, natural = panel.offsetHeight;
      const above = natural <= room.above || room.above >= room.below;
      panel.style.maxHeight = `${Math.max(0, Math.min(natural, above ? room.above : room.below))}px`;
      panel.style.bottom = above ? `${window.innerHeight - rect.top + 10}px` : '';
      panel.style.top = above ? '' : `${rect.bottom + 10}px`;
      panel.dataset.placement = above ? 'above' : 'below';
    };
    loadReviewerBots(sessionId);
    paint(sessionId);
    place(); placeNow = place;
    showPopover(panel, anchor);
    anchor.setAttribute('aria-expanded', 'true');
    const field = focusField && panel.querySelector<HTMLElement>(`[data-field="${focusField}"]`);
    // Focus starts on the row's Tab stop (paint chose it: the selection, or for Raise max rounds your default limit).
    (field?.querySelector<HTMLElement>('button[tabindex="0"]:not([hidden])') ?? field?.querySelector<HTMLElement>('button:not([hidden])') ?? panel.querySelector<HTMLElement>('.council-switch')!).focus({ preventScroll: true });

    // Lowering the target from the strip is for this thread only; the composer panel also sets the defaults.
    const remember = !anchor.closest('.council-strip');
    const onChange = (event: Event) => {
      const input = event.target as HTMLInputElement;
      if (input.classList.contains('council-new-threads')) { setCouncilForNewThreads(input.checked); return; }
      const compositionControl = event.target as HTMLElement;
      if (compositionControl.closest('.council-composition-slot') && updateCompositionField(sessionId, compositionControl)) {
        const slot = panel?.querySelector<HTMLElement>('.council-composition-slot');
        // Text edits already update the draft on `input`. Replacing the slot on their later `change`
        // removes the Apply button as the click that blurred the field is still being dispatched.
        const rebuild = compositionControl.matches('[data-composition-enabled], [data-composition-automatic], [data-composition-bots-enabled], [data-composition-bot], [data-composition-reviewer-mode], [data-composition-criteria-source]');
        if (slot && rebuild) replaceCompositionSlot(slot, sessionId);
        else compositionStatus(sessionId);
        return;
      }
      if (input.hasAttribute('data-council-scoped') || input.hasAttribute('data-council-path')) {
        const draft = selection(sessionId);
        reviewerDraft.set(sessionId, {
          ...draft,
          scoped: input.hasAttribute('data-council-scoped') ? input.checked : draft.scoped,
          fileScope: input.hasAttribute('data-council-path') ? input.value : draft.fileScope,
          authorizationId: input.hasAttribute('data-council-scoped') && input.checked
            ? (draft.authorizationId || crypto.randomUUID()) : draft.authorizationId,
        });
        if (shownConfig(sessionId) && draft.ids.length >= 2) {
          try { void apply(sessionId, selectionConfig(sessionId), remember); }
          catch (cause) { selectedError(sessionId, String(cause)); }
        } else paint(sessionId);
        return;
      }
      if (input.hasAttribute('data-council-optional-skill')) {
        const id = input.dataset.councilOptionalSkill!, draft = selection(sessionId);
        const optionalSkills = input.checked ? [...new Set([...(draft.optionalSkills ?? []), id])] : (draft.optionalSkills ?? []).filter(value => value !== id);
        reviewerDraft.set(sessionId, { ...draft, optionalSkills, checkSkills: input.checked ? (draft.checkSkills ?? []).filter(value => value !== id) : draft.checkSkills });
        if (shownConfig(sessionId) && draft.ids.length >= 2) {
          try { void apply(sessionId, selectionConfig(sessionId), remember); }
          catch (cause) { selectedError(sessionId, String(cause)); }
        } else paint(sessionId);
        return;
      }
      if (input.hasAttribute('data-council-required-check')) {
        const id = input.dataset.councilRequiredCheck!, draft = selection(sessionId);
        const checkSkills = input.checked ? [...new Set([...(draft.checkSkills ?? []), id])] : (draft.checkSkills ?? []).filter(value => value !== id);
        reviewerDraft.set(sessionId, { ...draft, checkSkills, optionalSkills: input.checked ? (draft.optionalSkills ?? []).filter(value => value !== id) : draft.optionalSkills });
        if (shownConfig(sessionId) && draft.ids.length >= 2) {
          try { void apply(sessionId, selectionConfig(sessionId), remember); }
          catch (cause) { selectedError(sessionId, String(cause)); }
        } else paint(sessionId);
        return;
      }
      if (input.hasAttribute('data-reviewer-bot')) {
        const id = input.dataset.reviewerBot!, draft = selection(sessionId), ids = input.checked ? [...draft.ids, id] : draft.ids.filter(item => item !== id);
        if (ids.length > 8) { input.checked = false; selectedError(sessionId, 'Council allows at most 8 reviewer bots.'); return; }
        reviewerDraft.set(sessionId, { ...draft, selected: true, ids, idsEdited: true });
        failures.delete(sessionId);
        if (shownConfig(sessionId) && ids.length >= 2) {
          try { void apply(sessionId, selectionConfig(sessionId), remember); } catch (cause) { selectedError(sessionId, String(cause)); }
        } else paint(sessionId);
        return;
      }
      // Classic team setup and no team yet: the user sets the team up first, rather than an automatic team starting.
      if (input.checked && !host.team(sessionId) && host.classicSetup()) {
        input.checked = false; close();
        host.openClassicSetup(document.querySelector<HTMLElement>(BUTTON) ?? anchor);
        return;
      }
      if (!input.checked) { void apply(sessionId, null, remember); return; }
      try { void apply(sessionId, selectionConfig(sessionId), remember); }
      catch (cause) { input.checked = false; selectedError(sessionId, cause instanceof Error ? cause.message : String(cause)); }
    };
    const onClick = (event: MouseEvent) => {
      const target = event.target as Element;
      const applyComposition = target.closest<HTMLElement>('[data-composition-apply]');
      if (applyComposition) {
        const state = compositionState(sessionId), errors = compositionErrors(sessionId);
        if (state.enabled && errors.length === 0) apply(sessionId, withCouncilComposition(shownConfig(sessionId) ?? councilDefaults(), activeComposition(sessionId)), remember);
        return;
      }
      const add = target.closest<HTMLButtonElement>('[data-add-criterion]');
      if (add?.dataset.addCriterion === 'user' || add?.dataset.addCriterion === 'architect') {
        addCriterion(sessionId, add.dataset.addCriterion);
        const slot = panel?.querySelector<HTMLElement>('.council-composition-slot'); if (slot) replaceCompositionSlot(slot, sessionId);
        return;
      }
      const remove = target.closest<HTMLButtonElement>('[data-remove-criterion-index]');
      if (remove) {
        const slot = panel?.querySelector<HTMLElement>('.council-composition-slot');
        const removeButtons = slot ? [...slot.querySelectorAll<HTMLButtonElement>('[data-remove-criterion-index]')] : [];
        const removedIndex = removeButtons.indexOf(remove);
        const fallback = removeButtons[removedIndex + 1] ?? removeButtons[removedIndex - 1] ?? slot?.querySelector<HTMLElement>('[data-add-criterion="user"]');
        const fallbackSelector = fallback ? compositionNodeSelector(fallback) : null;
        const state = compositionState(sessionId), removeIndex = Number(remove.dataset.removeCriterionIndex);
        if (Number.isInteger(removeIndex) && removeIndex >= 0 && removeIndex < state.value.criteria.length) state.value.criteria.splice(removeIndex, 1);
        if (slot) replaceCompositionSlot(slot, sessionId, fallbackSelector);
        return;
      }
      const option = target.closest<HTMLElement>('[data-value], [data-reviewers-source]');
      if (!option) return;
      const key = option.closest<HTMLElement>('[data-field]')!.dataset.field as Field;
      const current = shownConfig(sessionId);
      if (option.dataset.reviewersSource === 'selected') {
        const draft = selection(sessionId);
        reviewerDraft.set(sessionId, { ...draft, selected: true, ids: [...draft.ids] });
        failures.delete(sessionId);
        // Configure once the user has actually chosen a valid roster; no partial request.
        if (current && draft.ids.length >= 2) {
          try { void apply(sessionId, selectionConfig(sessionId), remember); }
          catch (cause) { selectedError(sessionId, String(cause)); }
        } else paint(sessionId);
        return;
      }
      const value = Number(option.dataset.value);
      if (key === 'reviewers') {
        const draft = selection(sessionId);
        reviewerDraft.set(sessionId, { ...draft, selected: false, ids: [...draft.ids] });
      }
      // While Council is off, a reviewer-count click only sets what starts next.
      if (current) apply(sessionId, { ...current, [key]: value, ...(key === 'reviewers' ? { reviewerBotIds: undefined, optionalSkillIds: undefined, requiredCheckSkillIds: undefined, reviewPermissions: undefined } : {}) }, remember);
      else { saveCouncilDefaults({ ...councilDefaults(), [key]: value }); paint(sessionId); }
    };
    const onKey = (event: KeyboardEvent) => {
      // An IME cancelling its composition (in the prompt, say) keeps its Escape.
      if (event.key === 'Escape' && !isImeKey(event)) { event.preventDefault(); event.stopPropagation(); close(true); return; }
      // A segmented row is one Tab stop: arrows, Home and End move focus along it.
      const option = (event.target as Element | null)?.closest?.<HTMLElement>('.council-panel .ds-seg > button');
      const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
      if (option && (step || event.key === 'Home' || event.key === 'End')) {
        const row = [...option.parentElement!.querySelectorAll<HTMLElement>(':scope > button:not([hidden])')], at = row.indexOf(option);
        const next = row[event.key === 'Home' ? 0 : event.key === 'End' ? row.length - 1 : (at + step! + row.length) % row.length];
        // Focus only; Enter or Space picks (buttons do), so arrowing never changes the council.
        event.preventDefault(); for (const button of row) button.tabIndex = button === next ? 0 : -1; next.focus(); return;
      }
      // Tab and Shift+Tab always move inside the panel, in document order (the switch, each row's stop, the footer
      // checkbox), wrapping like a dialog: WebKit would otherwise skip plain checkboxes and leave the panel.
      if (event.key !== 'Tab' || !panel?.isConnected) return;
      const controls = [...panel.querySelectorAll<HTMLElement>('input:not(:disabled), select:not(:disabled), textarea:not(:disabled), button:not(:disabled)')].filter(control => control.tabIndex !== -1 && control.offsetParent !== null);
      if (!controls.length) return;
      event.preventDefault();
      const at = controls.indexOf(document.activeElement as HTMLElement), dir = event.shiftKey ? -1 : 1;
      controls[at < 0 ? event.shiftKey ? controls.length - 1 : 0 : (at + dir + controls.length) % controls.length].focus();
    };
    const onOutside = (event: PointerEvent) => {
      const target = event.target as Element;
      // Any Council opener (the composer button, or the strip's Lower target or Raise max rounds) toggles the panel itself.
      if (panel && !panel.contains(target) && !target.closest('[data-action="council"]')) close();
    };
    panel.addEventListener('change', onChange);
    panel.addEventListener('input', event => {
      const target = event.target as HTMLElement;
      if (target.closest('.council-composition-slot') && updateCompositionField(sessionId, target)) compositionStatus(sessionId);
    });
    panel.addEventListener('click', onClick);
    document.addEventListener('keydown', onKey, true);
    document.addEventListener('pointerdown', onOutside, true);
    window.addEventListener('resize', place);
    cleanup = () => { document.removeEventListener('keydown', onKey, true); document.removeEventListener('pointerdown', onOutside, true); window.removeEventListener('resize', place); };
  }

  /** The strip's Keep fixing or Accept and finish (Council stays on; the strip reads "Accepted at …"). While one runs all three
   *  actions are busy (still focusable); a failure shows once, in the strip's alert, and focus returns to the button
   *  pressed; done, focus goes to the strip's disclosure (or the prompt once the strip is gone). */
  async function act(sessionId: string, action: 'keep' | 'accept', pressed: HTMLElement): Promise<void> {
    if (actions.get(sessionId)?.busy) return;
    actions.set(sessionId, { busy: true, error: '' }); refresh(sessionId);
    let error = '';
    try {
      if (action === 'keep') await host.keepFixing(sessionId);
      else { const team = await host.call<TeamSnapshot | null>('architect_council_accept', { sessionId }); if (team) host.updated(team); }
    } catch (cause) {
      error = `${action === 'keep' ? 'Keep fixing' : 'Accept and finish'} did not go through: ${cause instanceof Error ? cause.message : String(cause)}`;
    }
    // A sent follow-up reads as fixing until native's state changes (which clears this entry).
    actions.set(sessionId, { busy: false, error, sent: action === 'keep' && !error }); refresh(sessionId);
    // After the composer's own redraw (a send rebuilds it), so focus lands on what stays.
    await new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)));
    const again = pressed.isConnected ? pressed : document.querySelector<HTMLElement>(`#composer .council-strip [data-action="${pressed.dataset.action}"]`);
    (error ? again : document.querySelector<HTMLElement>('#composer .council-strip-toggle') ?? document.querySelector<HTMLElement>('#prompt'))?.focus({ preventScroll: true });
  }
  /** The disclosure: the reviewer rows, remembered per thread; collapsed by default. */
  function toggle(sessionId: string): void {
    if (expanded.has(sessionId)) expanded.delete(sessionId); else expanded.add(sessionId);
    refresh(sessionId);
  }
  /** A settled strip's ×: hidden until the next cycle. */
  function dismiss(sessionId: string): void {
    const team = host.team(sessionId);
    if (team) dismissed.add(cycleKey(team));
    refresh(sessionId);
    document.querySelector<HTMLElement>('#prompt')?.focus({ preventScroll: true });
  }
  /** "Turn on for new threads": Council on for a new thread (its team starting, or its first send), with the defaults. */
  /** Resolves to the reason it failed, or '' (also when there was nothing to do). Classic team setup starts no automatic team. */
  const enableForNewThread = async (sessionId: string): Promise<string> => {
    if (!councilForNewThreads() || host.classicSetup()) return '';
    // Team startup and the first send can request Council concurrently. Join the
    // already-started request so the send observes its refusal instead of treating
    // an in-flight setup as a successful no-op.
    const inFlight = pending.get(sessionId);
    if (inFlight) {
      await inFlight.tail;
      return failures.get(sessionId) ?? '';
    }
    if (host.team(sessionId)?.config.council) return '';
    await apply(sessionId, councilDefaults(), false);
    return failures.get(sessionId) ?? '';
  };
  return { button, strip, open, close, sync, act, toggle, dismiss, enableForNewThread };
}

/** Per thread: a strip action under way (all three are busy), its failure, and a Keep fixing already sent. */
const actions = new Map<string, { busy: boolean; error: string; sent?: boolean }>();
/** Threads whose strip shows its reviewer rows; cycles whose settled strip was dismissed. */
const expanded = new Set<string>(), dismissed = new Set<string>();
const cycleKey = (team: TeamSnapshot): string => `${team.sessionId}:${team.councilSince ?? 0}`;

const MARK_PULSE = '<span class="council-pulse" aria-hidden="true"></span>';
/** A seat's score or state, in words for every reader, plus its mark. The shape says what the tone says. */
type SeatState = 'passed' | 'below' | 'failed' | 'stopped' | 'attention' | 'reviewing' | 'quiet';
/** A seat's score or state, in words for every reader, plus its mark. Shape and tone both differ by state, so colour never
 *  carries it alone: a check (saver) at target, a warning (full) below it, a plain cross (no tone) for a reviewer that failed. */
function seatStatus(seat: CouncilSeatWorker, target: number, active: boolean, asking: (sessionId: string) => boolean, first?: number, { lowest = false, accepted = false } = {}): { html: string; spoken: string; tone: string; mark: string; scored: boolean; state: SeatState } {
  const score = seat.council.score;
  if (typeof score === 'number' && Number.isFinite(score)) {
    const passed = score >= target, moved = first !== undefined && formatScore(first) !== formatScore(score);
    // Settled: first → final, the first muted; just the final when they match.
    const html = `${moved ? `<span class="council-first">${formatScore(first!)} → </span>` : ''}${formatScore(score)}`;
    // Below target: the round's lowest seat carries the warning (muted once accepted); the others a quiet open ring.
    // Accepted: every mark neutral (a plain check at target, a ring below), so no warning sits under "Accepted".
    // Below target and not its complete round's lowest (or accepted): a small muted ring, apart from pending reviewers (pulse
    // or clock) and from the lowest's warning.
    const mark = passed ? icon('check') : lowest && !accepted ? icon('alert') : '<span class="council-ring" aria-hidden="true"></span>', tone = accepted ? '' : passed ? 'success' : lowest ? 'full' : '';
    return { html, spoken: `${moved ? `first ${formatScore(first!)}, final ` : ''}${formatScore(score)}, ${passed ? 'at target' : 'below target'}`, tone, mark, scored: true, state: passed ? 'passed' : 'below' };
  }
  const [word, state]: [string, SeatState] = seat.status === 'failed' ? ['Failed', 'failed'] : seat.status === 'stopped' ? ['Stopped', 'stopped'] : seat.status === 'completed' ? ['No score', 'failed']
    : !active ? ['Paused', 'quiet'] : seat.status === 'waiting' || asking(seat.sessionId) ? ['Needs attention', 'attention'] : seat.status === 'running' ? ['Reviewing', 'reviewing'] : ['Queued', 'quiet'];
  const marks: Record<SeatState, string> = { passed: '', below: '', failed: icon('close'), stopped: icon('stop'), attention: icon('alert'), reviewing: MARK_PULSE, quiet: icon('clock') };
  return { html: escape(word), spoken: word.toLowerCase(), tone: state === 'reviewing' ? 'team' : state === 'attention' ? 'full' : '', mark: marks[state], scored: false, state };
}

const lowestOf = ({ status }: CouncilProgress, round: CouncilRound): number | null => status.round === round.number && status.lowest !== null ? status.lowest : round.lowest;
/** The target each cycle started with (by thread and cycle), as first seen here: a pass after lowering it says so. Kept for
 *  this window only (after a reload a pass reads plainly). */
const firstTargets = new Map<string, number>();
function lowered(team: TeamSnapshot): boolean {
  const target = team.councilStatus?.target ?? team.config.council?.target;
  if (target === undefined) return false;
  const key = `${team.sessionId}:${team.councilSince ?? 0}`, first = firstTargets.get(key);
  if (first === undefined) { firstTargets.set(key, target); return false; }
  return target < first;
}
/** The round whose rows show: the latest, or while the team rests with an unscored latest round, the last scored one. */
function shownRound(progress: CouncilProgress, team: TeamSnapshot): CouncilRound | undefined {
  const resting = !teamActive(team) || councilAsking(progress.status) || settled(progress.state);
  return resting && progress.latest?.lowest === null ? [...progress.rounds].reverse().find(round => round.lowest !== null) ?? progress.latest : progress.latest;
}
/** One short row per reviewer (shared by the strip and the Agents flyout): mark, lens, score; first → final once settled.
 *  A row opens that reviewer, like a worker row. */
function rowsHtml(progress: CouncilProgress, team: TeamSnapshot): string {
  const round = shownRound(progress, team);
  if (!round) return '';
  const { state, status } = progress, ended = settled(state) || finishedBelow(progress, team);
  const firsts = ended ? new Map(firstAndFinal(progress.rounds).map(({ lens, first }) => [lens, first])) : null;
  return round.seats.map(seat => {
    const lens = seat.council.lens.trim() || seat.name, lowest = seatScore(seat) !== null && seatScore(seat) === round.lowest && !provisional(progress, round);
    const reviewerLabel = seat.reviewerBinding?.reviewerBotId ? `${seat.name} · ${lens}` : lens;
    const skillReceipt = seat.reviewerBinding?.snapshot?.skills;
    const supplied = skillReceipt?.pins ?? [], skipped = skillReceipt?.skipped ?? [];
    const observed = seat.reviewerBinding?.skillEvidence ?? {};
    const skillSummary = [
      ...supplied.map(pin => {
        const evidence = observed[pin.id];
        const matches = evidence?.contentSha256 === pin.contentSha256
          && evidence?.artifactRevision === seat.reviewerBinding?.artifactRevision;
        const retrieved = matches && evidence?.invoked;
        const verified = retrieved && evidence?.checkExecuted && evidence?.evidenceVerified
          && evidence?.checkKind === 'pinned_input_integrity';
        return `${pin.id}: supplied; native retrieval ${retrieved ? 'recorded' : 'not verified'}; pinned input integrity ${verified ? 'verified' : pin.checkRequired ? 'required; not verified' : 'not verified'}; SHA-256 ${pin.contentSha256}`;
      }),
      ...skipped.map(skill => `${skill.id}: skipped (${skill.reason})`),
    ];
    const skillNote = skillSummary.length ? `<small class="council-row-skills" title="${escape(skillSummary.join('; '))}">${escape(`Skills: ${supplied.length} supplied, ${skipped.length} skipped`)}${skipped.length ? ` · ${escape(skipped.map(skill => `${skill.id}: ${skill.reason}`).join('; '))}` : ''}</small>` : '';
    const s = seatStatus(seat, status.target, teamActive(team), () => false, firsts?.get(lens), { lowest, accepted: state === 'accepted' });
    return `<button type="button" class="council-row" data-id="${escape(seat.id)}" data-action="open-team-worker" data-worker-id="${escape(seat.id)}" aria-label="${escape(`${reviewerLabel}${skillSummary.length ? `; ${skillSummary.join('; ')}` : ''}: ${s.spoken}. Open reviewer`)}"><span class="council-row-mark"${s.tone ? ` data-state-tone="${s.tone}"` : ''}>${s.mark}</span><span class="council-row-lens">${escape(reviewerLabel)}${skillNote}</span><span class="council-row-score"${s.scored ? ` data-scored="${formatScore(seatScore(seat)!)}"` : ''}>${s.html}</span></button>`;
  }).join('');
}
/** The strip's state line: the state, then the lowest against the target ("so far" while the round is open). */
function stateLine(progress: CouncilProgress, team: TeamSnapshot): { text: string; score: string } {
  const round = shownRound(progress, team), lowest = round ? lowestOf(progress, round) : null, target = formatScore(progress.status.target);
  const sofar = round && provisional(progress, round) ? ' <small>so far</small>' : '';
  return { text: councilStateText(progress, team, lowered(team)), score: lowest === null ? `Target ${target}` : `${formatScore(lowest)} / ${target}${sofar}` };
}
/** The Agents flyout's Council: the state line and the rows, no actions (they live in the composer's strip). */
export function councilSummaryHtml(native: TeamSnapshot): string {
  const team = shownTeam(native), progress = councilProgress(team);
  if (!progress) return '';
  const hasProjection = Boolean(team.councilFindings || team.councilFindingDispositions || team.councilChildReceipts || team.councilReviewReceipts || team.councilUsage);
  if (!progress.rounds.length && !progress.council.composition && !hasProjection) return '';
  const { text, score } = stateLine(progress, team);
  const composition = progress.council.composition;
  const compositionSummary = composition
    ? `Schema v${composition.schemaVersion} · ${councilCompositionMode(composition) ?? 'Unavailable'} · ${composition.strategy.replaceAll('_', ' ')} strategy · ${composition.criteriaSource} criteria`
    : '';
  const ledgerHtml = councilLedgerHtmlForTeam(team);
  return `<section class="council-summary" data-session-id="${escape(team.sessionId)}" aria-label="Council"><p class="council-summary-state"><span>${escape(text)}</span><span class="council-strip-score">${score}</span></p>${compositionSummary ? `<small class="council-summary-composition">${escape(compositionSummary)}</small>` : ''}<div class="council-rows">${rowsHtml(progress, team)}</div>${ledgerHtml}</section>`;
}

/** The current-cycle read-only ledger shared by Workers and Shared Channel report drilldowns. */
export function councilLedgerHtmlForTeam(native: TeamSnapshot): string {
  const team = shownTeam(native), progress = councilProgress(team);
  const hasProjection = Boolean(team.councilFindings || team.councilFindingDispositions || team.councilChildReceipts || team.councilReviewReceipts || team.councilUsage);
  const composition = progress?.council.composition;
  return councilReadOnlyPresenterHtml({
    ...councilLedgerForCurrentCycle(team),
    ...(progress ? { progress } : {}),
    workers: team.workers,
    status: team.councilStatus,
    ...(!composition && !hasProjection ? { unavailableMessage: 'This legacy Council configuration does not produce composition findings, receipts or usage.' } : {}),
  });
}
/** The composer's strip: shown while Council is on and the cycle has a round or is asking, until a settled strip is
 *  dismissed. One line (glyph, state, lowest / target, disclosure), the reviewers when expanded, and what to do next. */
function stripHtml(native: TeamSnapshot): string {
  const team = shownTeam(native), progress = councilProgress(team);
  if (!progress) return '';
  const asks = councilAsking(progress.status), finished = finishedBelow(progress, team), ended = settled(progress.state) || finished;
  const attention = progress.state === 'blocked' || progress.state === 'failed'
    || progress.state === 'inconclusive' || progress.state === 'stale';
  if ((!progress.rounds.length && !asks && !attention) || (ended && dismissed.has(cycleKey(team)))) return '';
  const id = team.sessionId, open = expanded.has(id), { text, score } = stateLine(progress, team), { busy = false, error = '' } = actions.get(id) ?? {};
  const off = busy ? ' aria-disabled="true" aria-busy="true"' : '';
  const panelButton = (field: 'target' | 'maxRounds', label: string, cls = 'ds-btn') => `<button type="button" class="${cls}" data-action="council" data-council-focus="${field}" aria-haspopup="dialog" aria-expanded="${expandedOpener === `${id}:${field}`}"${off}>${label}</button>`;
  // What to do next: asking, the three ways on; at the limit, raise it; finished or stopped, a short hint.
  const halted = !teamActive(team) && team.status !== 'completed' && team.status !== 'ready' && !ended && !asks;
  const canLower = COUNCIL_TARGETS.some(target => target < progress.council.target);
  const used = Math.max(progress.status.round, progress.latest?.number ?? 0);
  const canRaise = COUNCIL_ROUNDS.some(rounds => rounds > used);
  const next = asks ? `<span class="council-strip-actions" role="group" aria-label="Council needs you"><button type="button" class="ds-btn-primary" data-action="council-keep-fixing"${off}>Keep fixing</button>${canLower ? panelButton('target', 'Lower target') : ''}<button type="button" class="ds-btn" data-action="council-accept"${off}>Accept and finish</button></span>`
    : progress.state === 'limit' && canRaise ? `<span class="council-strip-actions">${panelButton('maxRounds', 'Raise max rounds')}</span>`
    : finished ? '<span class="council-strip-hint">Send a message to continue, or lower the target.</span>'
    : attention ? '<span class="council-strip-hint">Review the Council notice before continuing.</span>'
    : halted ? '<span class="council-strip-hint">Send a message to continue.</span>' : '';
  const close = ended ? '<button type="button" class="icon-button council-strip-dismiss" data-action="council-dismiss" aria-label="Dismiss Council result" data-tip="Dismiss">' + icon('close') + '</button>' : '';
  const tone = asks || attention || progress.state === 'limit' || finished ? 'full' : progress.state === 'passed' ? 'success' : '';
  const nativeNotice = attention && progress.status.notice
    ? `<p class="council-strip-error" role="alert">${escape(progress.status.notice)}</p>` : '';
  return `<section class="council-strip" data-session-id="${escape(id)}" data-state="${progress.state}" aria-label="Council"><div class="council-strip-head"><button type="button" class="council-strip-toggle" data-action="council-strip-toggle" aria-expanded="${open}" aria-controls="council-rows-${escape(id)}"><span class="council-glyph" aria-hidden="true">${COUNCIL_ICON}</span><span class="council-strip-state"${tone ? ` data-state-tone="${tone}"` : ''}>${escape(text)}</span><span class="council-strip-score">${score}</span>${icon('chevron')}</button>${next || close ? `<span class="council-strip-end">${next}${close}</span>` : ''}</div>${nativeNotice}${error ? `<p class="council-strip-error" role="alert">${escape(error)}</p>` : ''}<div class="council-rows" id="council-rows-${escape(id)}"${open ? '' : ' hidden'}>${open ? rowsHtml(progress, team) : ''}</div></section>`;
}
const STRIP_EXIT_MS = 120;
/** Draws the strip in place (morph keeps focus), with its motion: it enters and leaves with a short fade and a few pixels of
 *  rise (never the height of anything else), its state cross-fades, and a score lands. */
function paintStrip(slot: HTMLElement, team: TeamSnapshot | null, known: boolean): void {
  const html = team ? stripHtml(team) : '', id = slot.dataset.sessionId!, shown = slot.querySelector<HTMLElement>(':scope > .council-strip');
  if (!html && !shown) {
    // A settled empty state must count as a real baseline for the next round.
    if (team || known) motion(id, 'strip', '', null, () => []);
    return;
  }
  if (!html && shown) {
    if (shown.dataset.leaving) return;
    const out = motion(id, 'strip', '', shown, element => [element.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'translateY(4px)' }], { duration: STRIP_EXIT_MS, easing: 'ease-in', fill: 'forwards' })]);
    if (!out.length) { slot.replaceChildren(); return; }
    shown.dataset.leaving = 'true'; shown.inert = true;
    setTimeout(() => { if (shown.dataset.leaving && shown.parentElement === slot) slot.replaceChildren(); }, STRIP_EXIT_MS);
    return;
  }
  if (shown?.dataset.leaving) slot.replaceChildren();
  morph(slot, html);
  const strip = slot.querySelector<HTMLElement>(':scope > .council-strip');
  if (!strip) return;
  loaded(id);
  const entered = motion(id, 'strip', 'shown', strip, element => [element.animate([{ opacity: 0, transform: 'translateY(4px)' }, { opacity: 1, transform: 'none' }], { duration: 180, easing: 'ease-out' })]);
  // The entrance carries the children, but still establish their baseline for the next real change.
  const state = strip.querySelector<HTMLElement>('.council-strip-state');
  motion(id, 'strip-state', state?.textContent ?? '', entered.length ? null : state, element => fade(element, 180, 0.4));
  for (const score of strip.querySelectorAll<HTMLElement>('.council-row-score')) motion(id, `score:${score.closest<HTMLElement>('.council-row')!.dataset.id}`, score.dataset.scored ?? '', entered.length ? null : score, element => element.dataset.scored ? land(element) : []);
}

/* Motion: one gate for every Council animation. An animation plays for a change seen while its thread is the one shown,
 * and has been for 500 ms (so a launch, a reload or a thread switch plays nothing), in a visible, ready window without
 * reduced motion, on an element on screen. A change is known by what it is (thread, subject, new value), never by the
 * element: a node rebuilt mid-animation joins it at its elapsed time, and one rebuilt after it plays nothing. Presentation
 * only, through the Web Animations API on opacity, transform and scale. */
let shownThread = '', shownAt = 0;
/** When the shown thread's team was first known (or its card first drawn): the quiet window runs from the later of that and
 *  the thread showing, so a slow first load plays nothing either. */
const loadedAt = new Map<string, number>();
function threadShown(sessionId: string): void {
  if (sessionId !== shownThread) { shownThread = sessionId; shownAt = performance.now(); loadedAt.clear(); }
}
function loaded(sessionId: string): void {
  if (!loadedAt.has(sessionId)) loadedAt.set(sessionId, performance.now());
}
const QUIET_MS = 500;
const lastSeen = new Map<string, string>();
const played = new Map<string, { at: number; end: number; prev: string | undefined; on: WeakSet<Element> }>();
type Play = (element: HTMLElement, prev: string | undefined) => Animation[];
const moving = (): boolean => document.documentElement.hasAttribute('data-ready') && !document.hidden && !reducedMotion();
const onScreen = (element: Element): boolean => { const rect = element.getBoundingClientRect(); return element.isConnected && rect.width > 0 && rect.bottom > 0 && rect.top < innerHeight; };
const endOf = (animation: Animation): number => Number(animation.effect?.getComputedTiming().endTime) || 0;
/** Records `value` for the thread's `subject` and plays `play` on `element` when it is a change the gate lets through.
 *  `fromNothing`: a subject first seen counts as a change (a card appearing); otherwise the first sight is a baseline. */
function motion(sessionId: string, subject: string, value: string, element: HTMLElement | null | undefined, play: Play, { fromNothing = false } = {}): Animation[] {
  const slot = `${sessionId}\n${subject}`, key = `${slot}\n${value}`, prev = lastSeen.get(slot), now = performance.now();
  lastSeen.set(slot, value);
  if (!element) return [];
  if (prev !== value && (prev !== undefined || fromNothing)) {
    played.delete(key);
    if (sessionId !== shownThread || now - Math.max(shownAt, loadedAt.get(sessionId) ?? Infinity) < QUIET_MS || !moving() || !onScreen(element)) return [];
    const animations = play(element, prev);
    for (const [old, change] of played) if (change.end < now) played.delete(old);
    if (animations.length) played.set(key, { at: now, end: now + Math.max(...animations.map(endOf)), prev, on: new WeakSet([element]) });
    return animations;
  }
  // The same change on a rebuilt node: it joins the running animation where it is, or stays still.
  const change = played.get(key);
  if (!change || change.on.has(element) || now >= change.end || !moving()) return [];
  change.on.add(element);
  const animations = play(element, change.prev);
  for (const animation of animations) animation.currentTime = now - change.at;
  return animations;
}
/** `from`: a kept node that changes cross-fades from part-way (never blinking to nothing); a new one fades from 0. */
function fade(element: Element | null | undefined, duration = 160, from = 0): Animation[] {
  return element ? [element.animate([{ opacity: from }, { opacity: 1 }], { duration, easing: 'ease-out' })] : [];
}
/** A small scale pop on a short spring (a touch of overshoot), for a cue that needs noticing. */
function pop(element: Element): Animation[] {
  const { keyframes, duration } = springKeyframes({ response: 0.22, damping: 0.72 }, progress => ({ scale: String(0.9 + 0.1 * progress) }));
  return [element.animate(keyframes, { duration, easing: 'linear' })];
}
/** A score lands once: it fades and scales in on a short spring from 0.94 (about 300 ms); its ink changes by the score's
 *  CSS colour transition. The number is final from the first frame, so a screen reader never hears a count. */
function land(score: HTMLElement): Animation[] {
  const { keyframes, duration } = springKeyframes({ response: 0.22 }, progress => ({ opacity: Math.min(1, progress * 1.6), scale: String(0.94 + 0.06 * progress) }));
  return [score.animate(keyframes, { duration, easing: 'linear' })];
}
