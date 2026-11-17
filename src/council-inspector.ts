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
  // Display-only hints are not approval. Require a matching native completion
  // for this exact session/cycle, revision, round and minimum score.
  const proof = inspection.verifiedCompletion;
  const verified = inspection.verifiedPass && inspection.status.state === 'passed' && !inspection.status.stale
    && proof != null && typeof inspection.sessionId === 'string' && inspection.sessionId.length > 0
    && proof.sessionId === inspection.sessionId
    && proof.cycleId === `${proof.sessionId}:${inspection.cycleStartedAt}`
    && proof.teamRevision === inspection.teamRevision
    && proof.round === inspection.status.round
    && Number.isFinite(proof.lowest) && proof.lowest === inspection.status.lowest
    && proof.lowest >= inspection.status.target;
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
    escape(String(f.round)) + ' · ' + escape(f.workerId) +
    ' · Finding ID ' + escape(typeof f.id === 'string' ? f.id : 'unavailable') +
    ' · Artifact revision ' + escape(typeof f.artifactRevision === 'string' ? f.artifactRevision : 'unavailable') +
    ' · ' + (f.locationVerified === true ? 'Location verification indicated' : 'Location unverified') +
    '</small></li>';
  // A truncated visual finding window cannot establish native total counts.
  const triage = inspection.findingTriage;
  const counts = triage && [triage.reported, triage.highPriorityClaims, triage.unverifiedClaims];
  const findingTriageHtml = counts && counts.every(value => Number.isSafeInteger(value) && value >= 0) &&
    triage!.highPriorityClaims <= triage!.reported && triage!.unverifiedClaims <= triage!.reported
    ? '<p class="council-finding-triage">Native current-cycle reviewer claims: ' +
      escape(String(triage!.reported)) + ' · Critical/high reported: ' +
      escape(String(triage!.highPriorityClaims)) + ' · Independently unverified: ' +
      escape(String(triage!.unverifiedClaims)) + ' (not a verified defect or Council pass)</p>'
    : '';
  const history = (inspection.historicalReportedFindings ?? []).slice(-32);
  const historicalHtml = history.length
    ? '<section aria-label="Earlier reviewer-reported findings" class="council-historical-findings">' +
      '<h4>Earlier reviewer reports (unverified history)</h4>' +
      '<p>A newer reviewer turn does not prove these findings were fixed. ' +
      'No automatic resolution or native approval is inferred.</p><ul>' +
      history.map(findingRow).join('') + '</ul></section>'
    : '';
  const shown = inspection.attempts.slice(-32);
  const rounds = [...new Set(shown.map(a => a.round))];
  const criteria = inspection.criteria ?? [];
  // A configured target or evidence obligation is not a verified reviewer result.
  // User and hybrid criteria are user-supplied anchors; other reviewer lenses
  // may be chosen by the Architect and are shown separately under attempts.
  const criteriaHtml = criteria.length
    ? '<section aria-label="Task criteria" class="council-inspection-criteria">' +
      '<h4>Configured task criteria (not verified results)</h4><ul>' +
      criteria.map(c => '<li><strong>' + escape(c.lens) + '</strong> · ' +
        (c.required ? 'Required' : 'Optional') + ' · Target ' +
        (Number.isFinite(c.target) ? escape(c.target.toFixed(1)) + '/10' : 'unknown') +
        ' · Evidence ' + (c.evidenceRequired ? 'required' : 'not explicitly required') +
        ' · ' + (inspection.criteriaSource === 'user' || inspection.criteriaSource === 'hybrid'
          ? 'User-defined criterion' : 'Configured criterion') + '</li>').join('') +
      '</ul><p>Targets and evidence obligations do not establish reviewer coverage or approval.</p></section>'
    : '<p>Criteria: Architect-defined</p>';
  const criterionObservationLabels: Record<NonNullable<CouncilInspection['criterionObservations']>[number]['state'], string> = {
    optional: 'Optional, not a required pass gate',
    awaiting_review: 'No review round admitted',
    missing_root_score: 'No settled scored root reviewer for this lens',
    below_target: 'Observed root reviewer score below required target',
    native_evidence_unverified: 'Independent native criterion evidence not verified; approval blocked',
    pinned_input_integrity_verified: 'Native pinned-input integrity verified for this criterion; this does not prove project tests, artifact correctness or Council approval',
    root_scores_observed: 'Root reviewer scores observed; not independently verified artifact evidence',
  };
  const nativeCriterionObservations = inspection.criterionObservations ?? [];
  const observedCriteriaHtml = nativeCriterionObservations.length
    ? '<section aria-label="Native criterion observations" class="council-criterion-observations">' +
      '<h4>Native criterion observations (not approvals)</h4>' +
      '<p>Root reviewer scores are reports, not artifact verification. A host-verified pinned-input check covers only that named check; child Council receipts are inspected separately. Neither is a Council approval.</p><ul>' +
      nativeCriterionObservations.map(item => '<li><strong>' + escape(item.id) + '</strong> · Round ' +
        escape(String(item.round)) + ' · Scored root reports ' +
        escape(String(item.rootScoredSeats)) + ' · Lowest root score ' +
        (item.lowestRootScore == null || !Number.isFinite(item.lowestRootScore)
          ? 'unknown' : escape(item.lowestRootScore.toFixed(1)) + '/10') + ' · ' +
        escape(criterionObservationLabels[item.state] ?? 'Unrecognized native observation; no approval') + '</li>').join('') +
      '</ul></section>'
    : '';
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
      children.map(child => {
        const receipt = child.verifiedChildReceipt;
        const ids = receipt?.leafWorkerIds ?? [];
        const correlated = receipt && receipt.rootCycleId === child.rootCycleId
          && receipt.childSessionId === child.childSessionId && receipt.botId === child.botId
          && Number.isFinite(receipt.lowestScore) && receipt.lowestScore >= 0 && receipt.lowestScore <= 10
          && Array.isArray(ids) && ids.length === child.requiredLeafReviewers
          && ids.length >= 2 && ids.length <= 8 && new Set(ids).size === ids.length;
        const escalationReceipt = child.verifiedEscalationReceipt;
        const freshIds = escalationReceipt?.leafWorkerIds ?? [];
        // Correlation is display hygiene, not a second native approval gate.
        // A receipt displayed by the host must refer to the same frozen child
        // and to fresh identities outside the original child review wave.
        const escalationCorrelated = escalationReceipt && correlated
          && escalationReceipt.rootCycleId === child.rootCycleId
          && escalationReceipt.childSessionId === child.childSessionId
          && escalationReceipt.botId === child.botId
          && escalationReceipt.objectiveSha256 === receipt?.objectiveSha256
          && escalationReceipt.policySha256 === receipt?.policySha256
          && escalationReceipt.childConfigSha256 === receipt?.childConfigSha256
          && Number.isFinite(escalationReceipt.lowestScore)
          && escalationReceipt.lowestScore >= 0 && escalationReceipt.lowestScore <= 10
          && Array.isArray(freshIds) && freshIds.length === child.requiredLeafReviewers
          && freshIds.length >= 2 && freshIds.length <= 8
          && new Set(freshIds).size === freshIds.length
          && freshIds.every(id => !ids.includes(id));
        const challengeReceipt = child.verifiedChallengeReceipt;
        const challenge = challengeReceipt?.phaseTwo;
        const challengeIds = challenge?.leafWorkerIds ?? [];
        // UI correlation is only display hygiene. The native authorization
        // boundary separately re-derives all receipts, grants and callbacks.
        const challengeCorrelated = challengeReceipt && correlated
          && challengeReceipt.rootCycleId === child.rootCycleId
          && challengeReceipt.childSessionId === child.childSessionId
          && challengeReceipt.botId === child.botId
          && /^[a-f0-9]{64}$/.test(challengeReceipt.phaseOneReceiptSha256 ?? '')
          && challenge?.rootCycleId === child.rootCycleId
          && challenge?.childSessionId === child.childSessionId
          && challenge?.botId === child.botId
          && challenge?.objectiveSha256 === receipt?.objectiveSha256
          && challenge?.policySha256 === receipt?.policySha256
          && challenge?.childConfigSha256 === receipt?.childConfigSha256
          && Number.isFinite(challenge?.lowestScore)
          && (challenge?.lowestScore ?? -1) >= 0 && (challenge?.lowestScore ?? 11) <= 10
          && Array.isArray(challengeIds)
          && challengeIds.length === child.requiredLeafReviewers
          && challengeIds.length >= 2 && challengeIds.length <= 8
          && new Set(challengeIds).size === challengeIds.length
          && challengeIds.every(id => typeof id === 'string' && id.length > 0 && !ids.includes(id));
        const measured = child.observedLeafUsage;
        const childUsageHtml = measured
          ? '<p>Child-observed native leaf attempts: ' + escape(String(measured.uniqueAttempts)) +
            ' · Queue ' + escape(observedDuration('', measured.cumulativeQueueWaitMs).trim()) +
            ' (' + escape(String(measured.queueMeasuredAttempts)) + ' measured)' +
            ' · Observed span ' + escape(observedDuration('', measured.cumulativeObservedSpanMs).trim()) +
            ' (' + escape(String(measured.spanMeasuredAttempts)) + ' measured)' +
            ' · Provider tokens ' + (measured.tokenCount == null ? 'unknown' : escape(String(measured.tokenCount))) +
            ' · Cost ' + (measured.costAmount == null || !measured.costCurrency
              ? 'unknown' : escape(String(measured.costAmount) + ' ' + measured.costCurrency)) +
            '</p>'
          : '<p>No child leaf attempts have been admitted.</p>';
        // An escalated child can have two waves of up to eight native leaves.
        // Show both; a first-wave receipt cannot certify the second wave.
        const leafAttempts = (child.leafAttempts ?? []).slice(-16);
        const freshLeafCount = leafAttempts.filter(attempt => attempt.round === 2).length;
        const s3 = inspection.strategy === 'two_phase';
        const validTriggers = new Set(['below_target', 'high_severity_finding', 'conflicting_verdict',
          'required_criterion', 'required_evidence']);
        const triggers = (child.automaticEscalationTriggers ?? [])
          .filter(reason => validTriggers.has(reason));
        const automaticTriggerHtml = !s3 && inspection.strategy === 'escalated'
          && inspection.forceEscalation !== true && triggers.length > 0
          ? '<p role="status">Authenticated S2 first-wave trigger: ' +
            escape(triggers.join(', ').replaceAll('_', ' ')) +
            '. Fresh independent child reviewers are required; no automatic second wave or root approval is implied.</p>'
          : '';
        const hasFreshWave = child.status === 'child_escalation_reviewing'
          || child.status === 'child_challenge_reviewing' || freshLeafCount > 0;
        const escalationHtml = hasFreshWave && !s3
          ? '<p role="status">Fresh native child escalation: round 2 admitted ' +
            escape(String(freshLeafCount)) + ' of ' + escape(String(child.requiredLeafReviewers)) +
            ' reviewers. ' + (escalationCorrelated
              ? 'Fresh child evidence is recorded separately; root approval remains pending.'
              : 'First-wave receipts do not verify second-wave completion or authorize root approval.') + '</p>'
          : '';
        const challengeHtml = hasFreshWave && s3
          ? '<p role="status">Independent S3 child challenge: phase 2 admitted ' +
            escape(String(freshLeafCount)) + ' of ' + escape(String(child.requiredLeafReviewers)) +
            ' reviewers. ' + (challengeCorrelated
              ? 'The challenge receipt is authenticated; root approval remains pending.'
              : 'A first-phase score is not independent challenge evidence or terminal approval.') + '</p>'
          : '';
        const challengeReceiptHtml = s3 && challengeReceipt
          ? (challengeCorrelated
            ? '<p>Native S3 challenge child receipt recorded (root approval pending): minimum score ' +
              escape((challenge?.lowestScore ?? 0).toFixed(1)) + '/10 · ' +
              escape(String(challengeIds.length)) + ' fresh independent reviewer identifiers.</p>' +
              '<ul aria-label="S3 child challenge reviewer evidence identifiers">' +
              challengeIds.map(id => '<li>' + escape(id) + '</li>').join('') + '</ul>'
            : '<p role="alert">S3 challenge receipt correlation or reviewer identity mismatch; no approval.</p>')
          : hasFreshWave && s3
            ? '<p>No independent S3 challenge receipt is available yet; root approval remains pending.</p>'
            : '';
        const escalationReceiptHtml = !s3 && escalationReceipt
          ? (escalationCorrelated
            ? '<p>Native second-wave child evidence recorded (root approval pending): minimum score ' +
              escape(escalationReceipt.lowestScore.toFixed(1)) + '/10 · ' +
              escape(String(freshIds.length)) + ' independent fresh reviewer identifiers.</p>' +
              '<ul aria-label="Fresh child reviewer evidence identifiers">' +
              freshIds.map(id => '<li>' + escape(id) + '</li>').join('') + '</ul>'
            : '<p role="alert">Second-wave child receipt correlation or reviewer identity mismatch; no approval.</p>')
          : hasFreshWave ? '<p>No independent second-wave child receipt is available yet; no parent vote.</p>' : '';
        const leafHtml = leafAttempts.length
          ? '<section aria-label="Child native reviewer attempts"><h5>Native child reviewer attempts (not root votes)</h5><ul>' +
            leafAttempts.map(attempt => {
              const score = attempt.score === null || !Number.isFinite(attempt.score)
                ? 'No native score' : attempt.score.toFixed(1) + '/10 (reported)';
              return '<li><strong>' + escape(attempt.name + ' — ' + attempt.lens) + '</strong>' +
                '<small>' + escape('Round ' + (attempt.round ?? 'unknown') + ' · ' + attempt.status + ' · ' + score + ' · ' + attempt.workerId) +
                ' · ' + escape(attempt.provider + ' / ' + attempt.model) +
                ' · ' + escape(observedDuration('Queue', attempt.queueWaitMs)) +
                ' · ' + escape(observedDuration('Observed span', attempt.observedSpanMs)) +
                ' · Tokens unknown · Cost unknown</small></li>';
            }).join('') + '</ul></section>'
          : '<p>No native child reviewer attempts are available for drill-down.</p>';
        const receiptHtml = receipt ? (correlated
          ? '<p>' + (hasFreshWave
            ? 'First-wave native child evidence recorded (second-wave approval pending): minimum score '
            : 'Native child evidence recorded (root approval pending): minimum score ') +
            escape(receipt.lowestScore.toFixed(1)) + '/10 · ' +
            escape(String(ids.length)) + ' independent leaf identifiers.</p>' +
            '<ul aria-label="Child reviewer evidence identifiers">' +
            ids.map(id => '<li>' + escape(id) + '</li>').join('') + '</ul>'
          : '<p role="alert">Child evidence receipt correlation or reviewer count mismatch; no approval.</p>')
          : '<p>No verified child receipt is available; no parent vote.</p>';
        return '<li><details><summary>' + escape(child.botId) +
          ' · ' + escape(child.status.replaceAll('_', ' ')) + '</summary>' +
          '<p>Configured leaf reviewers: ' + escape(String(child.requiredLeafReviewers)) +
          ' · Task session: ' + escape(child.childSessionId) +
          ' · Parent cycle: ' + escape(child.rootCycleId) +
          '</p>' + childUsageHtml + leafHtml + automaticTriggerHtml + escalationHtml + challengeHtml +
          receiptHtml + escalationReceiptHtml + challengeReceiptHtml + '</details></li>';
      }).join('') + '</ul></section>'
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
  const composed = inspection.composedUsage;
  const composedHtml = composed && children.length
    ? '<p class="council-composed-usage">Root + child native attempts: ' + escape(String(composed.uniqueAttempts)) +
      ' · Cumulative queue ' + escape(observedDuration('', composed.cumulativeQueueWaitMs).trim()) +
      ' (' + escape(String(composed.queueMeasuredAttempts)) + ' measured)' +
      ' · Cumulative observed span ' + escape(observedDuration('', composed.cumulativeObservedSpanMs).trim()) +
      ' (' + escape(String(composed.spanMeasuredAttempts)) + ' measured)' +
      ' · Provider tokens ' + (composed.tokenCount == null ? 'unknown' : escape(String(composed.tokenCount))) +
      ' · Cost ' + (composed.costAmount == null || !composed.costCurrency
        ? 'unknown' : escape(String(composed.costAmount) + ' ' + composed.costCurrency)) +
      ' · Child reservations and parent composite seats excluded.</p>'
    : '';
  const groups = inspection.usageByRoundSource ?? [];
  const groupHtml = groups.length
    ? '<section aria-label="Root usage by round and source" class="council-usage-breakdown">' +
      '<h4>Root-observed usage by round and reviewer source</h4>' +
      '<p>Each native attempt is counted once across these groups. Child reservations are excluded; provider tokens and cost remain unknown without receipts.</p><ul>' +
      groups.map(group => '<li>Round ' + escape(String(group.round)) + ' · ' +
        escape(group.reviewerSource.replace('_', ' ')) +
        ' · Native attempts ' + escape(String(group.usage.uniqueAttempts)) +
        ' · Queue ' + escape(observedDuration('', group.usage.cumulativeQueueWaitMs).trim()) +
        ' (' + escape(String(group.usage.queueMeasuredAttempts)) + ' measured)' +
        ' · Observed span ' + escape(observedDuration('', group.usage.cumulativeObservedSpanMs).trim()) +
        ' (' + escape(String(group.usage.spanMeasuredAttempts)) + ' measured)</li>').join('') +
      '</ul></section>'
    : '';
  const allGroups = inspection.composedUsageByRoundSource ?? [];
  const allGroupsHtml = children.length && allGroups.length
    ? '<section aria-label="Root and child usage by round and source" class="council-usage-breakdown">' +
      '<h4>Root and child native usage by round and source</h4>' +
      '<p>Each admitted native leaf or root reviewer is counted once. Child coordinators, composite parent seats and unadmitted reservations are excluded; provider billing remains unknown.</p><ul>' +
      allGroups.map(group => '<li>Round ' + escape(String(group.round)) + ' · ' +
        escape(group.reviewerSource.replace('_', ' ')) +
        ' · Native attempts ' + escape(String(group.usage.uniqueAttempts)) +
        ' · Queue ' + escape(observedDuration('', group.usage.cumulativeQueueWaitMs).trim()) +
        ' · Observed span ' + escape(observedDuration('', group.usage.cumulativeObservedSpanMs).trim()) + '</li>').join('') +
      '</ul></section>'
    : '';
  const completion = verified && inspection.verifiedCompletion
    ? '<p class="council-verified-completion"><strong>Completed native review cycle</strong> · ' +
      escape(inspection.verifiedCompletion.cycleId) + ' · round ' +
      escape(String(inspection.verifiedCompletion.round)) + '</p>' : '';
  return '<div class="council-inspection-detail"><p><strong>' + escape(verdict) +
    '</strong> · ' + (inspection.status.stale ? 'Stale revision' : 'Current native status') +
    ': ' + escape(inspection.status.state) + ' · Revision ' + escape(String(inspection.teamRevision)) +
    '</p>' + criteriaHtml + observedCriteriaHtml +
    completion + sourceBreakdown + strategyHtml + usageHtml + composedHtml + groupHtml + allGroupsHtml + (hidden ? '<p>' + hidden + ' older attempts are not shown.</p>' : '') +
    (attempts || '<p>No reviewer attempts have been admitted.</p>') + childHtml + findingTriageHtml + findingsHtml + historicalHtml + '</div>';
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
