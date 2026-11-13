import { escape } from './dom';
import type { CouncilCriteriaSource, CouncilExecutionStrategy, CouncilInspection, CouncilReportedFinding } from './architect-types';

/** Native durations describe app-observed lifecycle only, not token or provider billing time. */
function observedDuration(label: string, ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return label + ' unknown';
  return label + ' ' + (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : Math.round(ms) + ' ms');
}

/** Safe read-only native projection, reusable without the task orchestrator. */
export function councilInspectionHtml(inspection: CouncilInspection | null): string {
  if (!inspection) return '<p>No task-owned Council is available.</p>';
  // An informational hint never overrides a stale or non-passing native state.
  const verified = inspection.verifiedPass && inspection.status.state === 'passed' && !inspection.status.stale;
  const verdict = inspection.status.state === 'accepted'
    ? 'Accepted by user (not a native pass)'
    : verified ? 'Verified native pass' : 'Native pass not verified';
  const modeLabels: Record<CouncilInspection['mode'], string> = {
    m1: 'Architect', m2: 'Bots', m3: 'Bots + Architect',
    m4: 'Bots with own Architect', m5: 'Bots with own Architect + Architect',
    m6: 'Mixed bots', m7: 'Mixed bots + Architect',
  };
  const nativeMode = inspection.mode && modeLabels[inspection.mode];
  const strategyLabels: Record<CouncilExecutionStrategy, string> = {
    parallel: 'Parallel', escalated: 'Escalated / on-demand', two_phase: 'Two-phase / adversarial',
  };
  const criteriaLabels: Record<CouncilCriteriaSource, string> = {
    architect: 'Architect-defined', user: 'User-defined', hybrid: 'Hybrid',
  };
  const strategyName = inspection.strategy && strategyLabels[inspection.strategy];
  const strategyHtml = strategyName
    ? '<p class="council-native-strategy">Configured native strategy: ' + escape(strategyName) +
      (inspection.strategy === 'escalated' && inspection.forceEscalation ? ' · Explicit fresh-round escalation required' : '') +
      (inspection.criteriaSource && criteriaLabels[inspection.criteriaSource]
        ? ' · Criteria origin: ' + escape(criteriaLabels[inspection.criteriaSource]) : '') + '</p>'
    : '';
  const sourceBreakdown = nativeMode
    ? '<p class="council-native-mode">Native mode: ' + escape(nativeMode) +
      ' · Observed attempts (not a passing quorum): ' +
      escape(String(inspection.attempts.filter(a => a.reviewerSource === 'direct_bot').length)) + ' direct bot, ' +
      escape(String(inspection.attempts.filter(a => a.reviewerSource === 'automatic').length)) + ' automatic</p>'
    : '';
  const reported = (inspection.reportedFindings ?? []).slice(-32);
  const findingRow = (f: CouncilReportedFinding): string =>
    '<li><strong>' + escape(f.severity.toUpperCase() + ' · ' + f.title) +
    '</strong><p>' + escape(f.category + ' · ' + f.lens + ' · ' + f.explanation) +
    '</p><small>Reviewer report only; independent evidence unverified · Round ' +
    escape(String(f.round)) + ' · ' + escape(f.workerId) + '</small></li>';
  const shown = inspection.attempts.slice(-32);
  const rounds = [...new Set(shown.map(a => a.round))];
  const criteria = inspection.criteria.filter(c => c.required);
  const attempts = rounds.map(round => {
    const entries = shown.filter(a => a.round === round).map(a => {
      const score = a.score === null ? 'No native score' : a.score.toFixed(1) + '/10';
      const usage = a.tokenCount === null ? 'Tokens unknown' : a.tokenCount + ' tokens';
      const cost = a.costAmount === null || !a.costCurrency ? 'Cost unknown' : a.costAmount + ' ' + a.costCurrency;
      const observed = observedDuration('Queue', a.queueWaitMs) + ' · ' +
        observedDuration('Observed span (last update)', a.observedSpanMs);
      const attributed = reported.filter(f => f.workerId === a.workerId && f.round === a.round);
      const drilldown = attributed.length
        ? '<details class="council-reviewer-findings"><summary>' +
          escape(String(attributed.length)) + ' reviewer-reported finding(s) (unverified)</summary><ul>' +
          attributed.map(findingRow).join('') + '</ul></details>'
        : '';
      return '<li><strong>' + escape(a.name + ' — ' + a.lens) + '</strong> ' +
        escape(a.reviewerSource.replace('_', ' ') + ' · ' + a.status + ' · ' + score) +
        '<small>Provider: ' + escape(a.provider + ' / ' + a.model + ' · ' + observed +
        ' · ' + usage + ' · ' + cost + ' (' + a.usageProvenance + ')') + '</small>' + drilldown + '</li>';
    }).join('');
    return '<section aria-label="Review round ' + round + '"><h4>Round ' + round +
      '</h4><ul>' + entries + '</ul></section>';
  }).join('');
  // Keep older/orphan reports visible when the corresponding attempt is outside
  // the native 32-attempt display window. Never invent their reviewer linkage.
  const unlinked = reported.filter(f => !shown.some(a => a.workerId === f.workerId && a.round === f.round));
  const findingsHtml = unlinked.length
    ? '<section aria-label="Reviewer-reported findings"><h4>Reviewer-reported findings (unverified)</h4>' +
      '<p>The matching reviewer attempt is not in this display.</p><ul>' +
      unlinked.map(findingRow).join('') + '</ul></section>'
    : '';
  const children = (inspection.reservedChildren ?? []).slice(0,8);
  const childHtml = children.length
    ? '<section aria-label="Task-owned child Councils" class="council-child-reservations">' +
      '<h4>Task-owned child Councils (not approvals)</h4>' +
      '<p>Reservations are not executed reviews or parent Council votes.</p><ul>' +
      children.map(child => '<li><details><summary>' + escape(child.botId) +
        ' · ' + escape(child.status.replaceAll('_', ' ')) + '</summary>' +
        '<p>Configured leaf reviewers: ' + escape(String(child.requiredLeafReviewers)) +
        ' · Task session: ' + escape(child.childSessionId) +
        ' · Parent cycle: ' + escape(child.rootCycleId) +
        '</p></details></li>').join('') + '</ul></section>'
    : '';
  const hidden = inspection.attempts.length - shown.length;
  const usage = inspection.usage;
  const usageHtml = usage
    ? '<p class="council-usage-totals">Native attempts: ' + escape(String(usage.uniqueAttempts)) +
      ' · Cumulative queue ' + escape(observedDuration('', usage.cumulativeQueueWaitMs).trim()) +
      ' (' + escape(String(usage.queueMeasuredAttempts)) + '/' + escape(String(usage.uniqueAttempts)) + ' measured)' +
      ' · Cumulative observed span ' + escape(observedDuration('', usage.cumulativeObservedSpanMs).trim()) +
      ' (' + escape(String(usage.spanMeasuredAttempts)) + '/' + escape(String(usage.uniqueAttempts)) + ' measured)' +
      ' · Provider tokens ' + (usage.tokenCount === null ? 'unknown' : escape(String(usage.tokenCount))) +
      ' · Cost ' + (usage.costAmount === null || !usage.costCurrency ? 'unknown' : escape(String(usage.costAmount) + ' ' + usage.costCurrency)) +
      '</p>'
    : '';
  const completion = verified && inspection.verifiedCompletion
    ? '<p class="council-verified-completion"><strong>Completed native review cycle</strong> · ' +
      escape(inspection.verifiedCompletion.cycleId) + ' · round ' +
      escape(String(inspection.verifiedCompletion.round)) + '</p>' : '';
  return '<div class="council-inspection-detail"><p><strong>' + escape(verdict) +
    '</strong> · ' + (inspection.status.stale ? 'Stale revision' : 'Current native status') +
    ': ' + escape(inspection.status.state) + ' · Revision ' + escape(String(inspection.teamRevision)) +
    '</p><p>' + (criteria.length ? 'Required criteria: ' +
      escape(criteria.map(c => c.lens).join(', ')) : 'Criteria: Architect-defined') + '</p>' +
    completion + sourceBreakdown + strategyHtml + usageHtml + (hidden ? '<p>' + hidden + ' older attempts are not shown.</p>' : '') +
    (attempts || '<p>No reviewer attempts have been admitted.</p>') + childHtml + findingsHtml + '</div>';
}

/** Native lookup on demand; closing and reopening always retrieves fresh evidence. */
export function createCouncilInspector(read: () => Promise<CouncilInspection | null>): HTMLElement {
  const details = document.createElement('details');
  details.className = 'council-inspection';
  details.setAttribute('aria-label', 'Council results');
  details.innerHTML = '<summary>Review evidence and usage</summary><div class="council-inspection-body" aria-live="polite"></div>';
  const body = details.querySelector<HTMLElement>('.council-inspection-body')!;
  let serial = 0;
  details.addEventListener('toggle', () => {
    if (!details.open) { serial++; return; }
    const current = ++serial;
    body.textContent = 'Loading native Council result…';
    void read().then(result => {
      if (serial !== current || !details.open || !details.isConnected) return;
      body.innerHTML = councilInspectionHtml(result);
    }).catch(reason => {
      if (serial !== current || !details.open || !details.isConnected) return;
      body.replaceChildren();
      const error = document.createElement('p');
      error.setAttribute('role', 'alert');
      error.textContent = 'Could not inspect this Council: ' + (reason instanceof Error ? reason.message : String(reason));
      body.append(error);
    });
  });
  return details;
}
