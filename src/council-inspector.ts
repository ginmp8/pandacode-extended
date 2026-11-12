import { escape } from './dom';
import type { CouncilInspection } from './architect-types';

/** Native durations describe app-observed lifecycle only, not token or provider billing time. */
function observedDuration(label: string, ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return label + ' unknown';
  return label + ' ' + (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : Math.round(ms) + ' ms');
}

/** Safe read-only native projection, reusable without the task orchestrator. */
export function councilInspectionHtml(inspection: CouncilInspection | null): string {
  if (!inspection) return '<p>No task-owned Council is available.</p>';
  const verdict = inspection.status.state === 'accepted'
    ? 'Accepted by user (not a native pass)'
    : inspection.verifiedPass ? 'Verified native pass' : 'Native pass not verified';
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
      return '<li><strong>' + escape(a.name + ' — ' + a.lens) + '</strong> ' +
        escape(a.reviewerSource.replace('_', ' ') + ' · ' + a.status + ' · ' + score) +
        '<small>Provider: ' + escape(a.provider + ' / ' + a.model + ' · ' + observed +
        ' · ' + usage + ' · ' + cost + ' (' + a.usageProvenance + ')') + '</small></li>';
    }).join('');
    return '<section aria-label="Review round ' + round + '"><h4>Round ' + round +
      '</h4><ul>' + entries + '</ul></section>';
  }).join('');
  const hidden = inspection.attempts.length - shown.length;
  const completion = inspection.verifiedPass && inspection.status.state === 'passed' &&
    !inspection.status.stale && inspection.verifiedCompletion
    ? '<p class="council-verified-completion"><strong>Completed native review cycle</strong> · ' +
      escape(inspection.verifiedCompletion.cycleId) + ' · round ' +
      escape(String(inspection.verifiedCompletion.round)) + '</p>' : '';
  return '<div class="council-inspection-detail"><p><strong>' + escape(verdict) +
    '</strong> · ' + (inspection.status.stale ? 'Stale revision' : 'Current native status') +
    ': ' + escape(inspection.status.state) + ' · Revision ' + escape(String(inspection.teamRevision)) +
    '</p><p>' + (criteria.length ? 'Required criteria: ' +
      escape(criteria.map(c => c.lens).join(', ')) : 'Criteria: Architect-defined') + '</p>' +
    completion + (hidden ? '<p>' + hidden + ' older attempts are not shown.</p>' : '') +
    (attempts || '<p>No reviewer attempts have been admitted.</p>') + '</div>';
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
