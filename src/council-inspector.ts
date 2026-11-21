import { escape } from './dom';
import type { CouncilCriteriaSource, CouncilExecutionStrategy, CouncilInspection, CouncilReportedFinding } from './architect-types';

/** Native durations describe app-observed lifecycle only, not token or provider billing time. */
function observedDuration(label: string, ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return label + ' unknown';
  return label + ' ' + (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : Math.round(ms) + ' ms');
}

/** Malformed or unobserved native usage must not appear as measured work. */
function nonNegativeCount(value: number | null | undefined): string {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
    ? String(value) : 'unknown';
}

function observedProviderCost(amount: number | null | undefined, currency: string | null | undefined): string {
  return typeof amount === 'number' && Number.isFinite(amount) && amount >= 0
    && typeof currency === 'string' && currency.trim().length > 0
    ? amount + ' ' + currency : 'unknown';
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
  // A historical user exception is not authorization for a newer revision.
  // The read-only presenter must never promote stale acceptance to a current gate.
  const verdict = inspection.status.state === 'accepted' && !inspection.status.stale
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
  // A reserved child is neither a native leaf attempt nor a passing parent
  // seat. Display root reviewer activity separately from the task-owned
  // child reservations, including when M4 has no root reviewer attempts.
  const childReservations = Array.isArray(inspection.reservedChildren) ? inspection.reservedChildren : [];
  const sourceBreakdown = nativeMode
    ? '<p class="council-native-mode">Native mode: ' + escape(nativeMode) +
      ' · Observed root reviewer attempts (not a passing quorum): ' +
      escape(String(inspection.attempts.filter(a => a.reviewerSource === 'direct_bot').length)) + ' direct bot, ' +
      escape(String(inspection.attempts.filter(a => a.reviewerSource === 'automatic').length)) + ' automatic' +
      ' · Task-owned child Council reservations: ' + escape(String(childReservations.length)) +
      ' (each may become at most one composite parent seat after native verification; reservations are not approvals)</p>'
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
    reviewed_file_sha256_verified: 'Native file SHA-256 matched the required digest for the reviewed revision; this does not prove project tests or Council approval',
    reviewed_file_text_contains_verified: 'Native checkout file contained the exact configured UTF-8 fragment when the host checked it; this does not prove project tests, ongoing file immutability or Council approval',
    reviewed_file_json_verified: 'Native checkout JSON syntax was valid when the host checked the frozen file; this does not prove schema compliance, project tests or Council approval',
    reviewed_file_toml_verified: 'Native checkout TOML syntax was valid when the host checked the task-scoped file; this does not prove Cargo checks, project tests, builds or Council approval',
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
      // Malformed or legacy provider data must never look like a valid
      // reviewer score or billable usage in this read-only inspection.
      const score = typeof a.score === 'number' && Number.isFinite(a.score)
        && a.score >= 0 && a.score <= 10 ? a.score.toFixed(1) + '/10' : 'No native score';
      const usage = Number.isSafeInteger(a.tokenCount) && a.tokenCount !== null
        && a.tokenCount >= 0 ? a.tokenCount + ' tokens' : 'Tokens unknown';
      const cost = typeof a.costAmount === 'number' && Number.isFinite(a.costAmount)
        && a.costAmount >= 0 && a.costCurrency
        ? a.costAmount + ' ' + a.costCurrency : 'Cost unknown';
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
  const children = childReservations.slice(0,8);
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
          && ids.length >= 2 && ids.length <= 8 && new Set(ids).size === ids.length
          && ids.every(id => typeof id === 'string' && id.trim().length > 0);
        const escalationReceipt = child.verifiedEscalationReceipt;
        const freshIds = escalationReceipt?.leafWorkerIds ?? [];
        // Correlation is display hygiene, not a second native approval gate.
        // A receipt displayed by the host must refer to the same frozen child
        // and to fresh identities outside the original child review wave.
        // A triggered S2 child can start with a genuine failing first wave.
        // It has no passing first-wave receipt, even after its independently
        // verified second wave succeeds. This is display hygiene only: the
        // native inspector has already revalidated the exact task receipt.
        const triggeredWithoutPassingFirst = !receipt
          && inspection.strategy === 'escalated' && !inspection.forceEscalation
          && child.status === 'child_escalation_evidence_ready_root_pending'
          && (child.automaticEscalationTriggers ?? []).some(reason =>
            ['below_target', 'required_criterion', 'conflicting_verdict', 'required_evidence'].includes(reason));
        const escalationCorrelated = escalationReceipt && (correlated || triggeredWithoutPassingFirst)
          && escalationReceipt.rootCycleId === child.rootCycleId
          && escalationReceipt.childSessionId === child.childSessionId
          && escalationReceipt.botId === child.botId
          && (!receipt || (
            escalationReceipt.objectiveSha256 === receipt.objectiveSha256
            && escalationReceipt.policySha256 === receipt.policySha256
            && escalationReceipt.childConfigSha256 === receipt.childConfigSha256))
          && Number.isFinite(escalationReceipt.lowestScore)
          && escalationReceipt.lowestScore >= 0 && escalationReceipt.lowestScore <= 10
          && Array.isArray(freshIds) && freshIds.length === child.requiredLeafReviewers
          && freshIds.length >= 2 && freshIds.length <= 8
          && new Set(freshIds).size === freshIds.length
          && freshIds.every(id => typeof id === 'string' && id.trim().length > 0 && !ids.includes(id));
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
          && challengeIds.every(id => typeof id === 'string' && id.trim().length > 0 && !ids.includes(id));
        const measured = child.observedLeafUsage;
        const childUsageHtml = measured
          ? '<p>Child-observed native leaf attempts: ' + escape(nonNegativeCount(measured.uniqueAttempts)) +
            ' · Queue ' + escape(observedDuration('', measured.cumulativeQueueWaitMs).trim()) +
            ' (' + escape(nonNegativeCount(measured.queueMeasuredAttempts)) + ' measured)' +
            ' · Observed span ' + escape(observedDuration('', measured.cumulativeObservedSpanMs).trim()) +
            ' (' + escape(nonNegativeCount(measured.spanMeasuredAttempts)) + ' measured)' +
            ' · Settled review window ' + escape(observedDuration('', measured.settledReviewWindowMs).trim()) +
            ' · Provider tokens ' + escape(nonNegativeCount(measured.tokenCount)) +
            ' · Cost ' + escape(observedProviderCost(measured.costAmount, measured.costCurrency)) +
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
            (freshLeafCount > 0
              ? (verified && escalationCorrelated
                ? '. Independent second-wave evidence and root native pass verified.</p>'
                : '. Fresh native second-wave reviewers have been admitted; independent receipts and root approval are still pending.</p>')
              : '. Fresh independent child reviewers are required; admission and root approval remain pending.</p>')
          : '';
        const rootEscalationVerified = Boolean(verified && escalationCorrelated);
        const hasFreshWave = child.status === 'child_escalation_reviewing'
          || child.status === 'child_challenge_reviewing' || freshLeafCount > 0;
        const escalationHtml = hasFreshWave && !s3
          ? '<p role="status">Fresh native child escalation: round 2 admitted ' +
            escape(String(freshLeafCount)) + ' of ' + escape(String(child.requiredLeafReviewers)) +
            ' reviewers. ' + (escalationCorrelated
              ? (rootEscalationVerified
                ? 'Fresh child evidence and root native pass are verified.'
                : 'Fresh child evidence is recorded separately; root approval remains pending.')
              : 'First-wave receipts do not verify second-wave completion or authorize root approval.') + '</p>'
          : '';
        const rootChallengeVerified = Boolean(verified && challengeCorrelated);
        const challengeHtml = hasFreshWave && s3
          ? '<p role="status">Independent S3 child challenge: phase 2 admitted ' +
            escape(String(freshLeafCount)) + ' of ' + escape(String(child.requiredLeafReviewers)) +
            ' reviewers. ' + (challengeCorrelated
              ? (rootChallengeVerified
                ? 'Independent S3 challenge receipt and root native pass are verified.'
                : 'The challenge receipt is authenticated; root approval remains pending.')
              : 'A first-phase score is not independent challenge evidence or terminal approval.') + '</p>'
          : '';
        const challengeReceiptHtml = s3 && challengeReceipt
          ? (challengeCorrelated
            ? '<p>Native S3 challenge child receipt recorded (' +
              (rootChallengeVerified ? 'root native pass verified' : 'root approval pending') +
              '): minimum score ' +
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
            ? '<p>Native second-wave child evidence recorded (' +
              (rootEscalationVerified ? 'root native pass verified' : 'root approval pending') +
              '): minimum score ' + escape(escalationReceipt.lowestScore.toFixed(1)) + '/10 · ' +
              escape(String(freshIds.length)) + ' independent fresh reviewer identifiers.</p>' +
              '<ul aria-label="Fresh child reviewer evidence identifiers">' +
              freshIds.map(id => '<li>' + escape(id) + '</li>').join('') + '</ul>'
            : '<p role="alert">Second-wave child receipt correlation or reviewer identity mismatch; no approval.</p>')
          : hasFreshWave ? '<p>No independent second-wave child receipt is available yet; no parent vote.</p>' : '';
        const leafHtml = leafAttempts.length
          ? '<section aria-label="Child native reviewer attempts"><h5>Native child reviewer attempts (not root votes)</h5><ul>' +
            leafAttempts.map(attempt => {
              const score = attempt.score === null || !Number.isFinite(attempt.score)
                || attempt.score < 0 || attempt.score > 10
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
        return '<li><details data-council-child-key="' + escape(child.rootCycleId + ':' + child.childSessionId) + '"><summary>' + escape(child.botId) +
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
    ? '<p class="council-usage-totals">Native attempts: ' + escape(nonNegativeCount(usage.uniqueAttempts)) +
      ' · Cumulative queue ' + escape(observedDuration('', usage.cumulativeQueueWaitMs).trim()) +
      ' (' + escape(nonNegativeCount(usage.queueMeasuredAttempts)) + '/' + escape(nonNegativeCount(usage.uniqueAttempts)) + ' measured)' +
      ' · Cumulative observed span ' + escape(observedDuration('', usage.cumulativeObservedSpanMs).trim()) +
      ' (' + escape(nonNegativeCount(usage.spanMeasuredAttempts)) + '/' + escape(nonNegativeCount(usage.uniqueAttempts)) + ' measured)' +
      ' · Settled review window ' + escape(observedDuration('', usage.settledReviewWindowMs).trim()) +
      ' (overlap counted once; not provider time)' +
      ' · Provider tokens ' + escape(nonNegativeCount(usage.tokenCount)) +
      ' · Cost ' + escape(observedProviderCost(usage.costAmount, usage.costCurrency)) +
      '</p>'
    : '';
  const composed = inspection.composedUsage;
  const composedHtml = composed && children.length
    ? '<p class="council-composed-usage">Root + child native attempts: ' + escape(nonNegativeCount(composed.uniqueAttempts)) +
      ' · Cumulative queue ' + escape(observedDuration('', composed.cumulativeQueueWaitMs).trim()) +
      ' (' + escape(nonNegativeCount(composed.queueMeasuredAttempts)) + ' measured)' +
      ' · Cumulative observed span ' + escape(observedDuration('', composed.cumulativeObservedSpanMs).trim()) +
      ' (' + escape(nonNegativeCount(composed.spanMeasuredAttempts)) + ' measured)' +
      ' · Settled review window ' + escape(observedDuration('', composed.settledReviewWindowMs).trim()) +
      ' (overlap counted once; not provider time)' +
      ' · Provider tokens ' + escape(nonNegativeCount(composed.tokenCount)) +
      ' · Cost ' + escape(observedProviderCost(composed.costAmount, composed.costCurrency)) +
      ' · Child reservations and parent composite seats excluded.</p>'
    : '';
  const groups = inspection.usageByRoundSource ?? [];
  const groupHtml = groups.length
    ? '<section aria-label="Root usage by round and source" class="council-usage-breakdown">' +
      '<h4>Root-observed usage by round and reviewer source</h4>' +
      '<p>Each native attempt is counted once across these groups. Child reservations are excluded; provider tokens and cost remain unknown without receipts.</p><ul>' +
      groups.map(group => '<li>Round ' + escape(String(group.round)) + ' · ' +
        escape(group.reviewerSource.replace('_', ' ')) +
        ' · Native attempts ' + escape(nonNegativeCount(group.usage.uniqueAttempts)) +
        ' · Queue ' + escape(observedDuration('', group.usage.cumulativeQueueWaitMs).trim()) +
        ' (' + escape(nonNegativeCount(group.usage.queueMeasuredAttempts)) + ' measured)' +
        ' · Observed span ' + escape(observedDuration('', group.usage.cumulativeObservedSpanMs).trim()) +
        ' (' + escape(nonNegativeCount(group.usage.spanMeasuredAttempts)) + ' measured)' +
        ' · Settled review window ' + escape(observedDuration('', group.usage.settledReviewWindowMs).trim()) + '</li>').join('') +
      '</ul></section>'
    : '';
  const allGroups = inspection.composedUsageByRoundSource ?? [];
  const allGroupsHtml = children.length && allGroups.length
    ? '<section aria-label="Root and child usage by round and source" class="council-usage-breakdown">' +
      '<h4>Root and child native usage by round and source</h4>' +
      '<p>Each admitted native leaf or root reviewer is counted once. Child coordinators, composite parent seats and unadmitted reservations are excluded; provider billing remains unknown.</p><ul>' +
      allGroups.map(group => '<li>Round ' + escape(String(group.round)) + ' · ' +
        escape(group.reviewerSource.replace('_', ' ')) +
        ' · Native attempts ' + escape(nonNegativeCount(group.usage.uniqueAttempts)) +
        ' · Queue ' + escape(observedDuration('', group.usage.cumulativeQueueWaitMs).trim()) +
        ' · Observed span ' + escape(observedDuration('', group.usage.cumulativeObservedSpanMs).trim()) +
        ' · Settled review window ' + escape(observedDuration('', group.usage.settledReviewWindowMs).trim()) + '</li>').join('') +
      '</ul></section>'
    : '';
  const completion = verified && inspection.verifiedCompletion
    ? '<p class="council-verified-completion"><strong>Completed native review cycle</strong> · ' +
      escape(inspection.verifiedCompletion.cycleId) + ' · round ' +
      escape(String(inspection.verifiedCompletion.round)) + '</p>' : '';
  return '<div class="council-inspection-detail"><p><strong>' + escape(verdict) +
    '</strong> · ' + (inspection.status.stale ? 'Stale revision' : 'Status at last native inspection') +
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
  const refresh = document.createElement('button');
  refresh.type = 'button';
  refresh.className = 'ds-btn council-inspection-refresh';
  refresh.textContent = 'Refresh native evidence';
  // A collapsed disclosure must never offer an un-focusable refresh as a Tab stop.
  refresh.disabled = true;
  body.before(refresh);
  // Disclosure state belongs to one native root task, cycle, and frozen artifact revision.
  // A child reservation ID can remain unchanged after the artifact is revised.
  const scopeOf = (snapshot: CouncilInspection | null): string | null =>
    snapshot && typeof snapshot.sessionId === 'string' && snapshot.sessionId.trim().length > 0
      && Number.isSafeInteger(snapshot.cycleStartedAt) && Number.isSafeInteger(snapshot.teamRevision)
      ? JSON.stringify([snapshot.sessionId, snapshot.cycleStartedAt, snapshot.teamRevision]) : null;
  let shownScope: string | null = null;
  let serial = 0;
  const load = () => {
    const current = ++serial;
    refresh.disabled = true;
    const priorScope = shownScope;
    const expandedChildren = new Set(Array.from(body.querySelectorAll<HTMLDetailsElement>('.council-child-reservations details[data-council-child-key][open]'))
      .map(child => child.dataset.councilChildKey));
    body.textContent = 'Loading native Council result…';
    // A synchronous read failure must also fail closed and release the button.
    void Promise.resolve().then(read).then(result => {
      if (serial !== current || !details.open || !details.isConnected) return;
      body.innerHTML = councilInspectionHtml(result);
      const nextScope = scopeOf(result);
      // The reservation and the root task/revision must still match. A new
      // artifact revision must not inherit an earlier review's open drill-down.
      if (priorScope !== null && priorScope === nextScope) {
        body.querySelectorAll<HTMLDetailsElement>('.council-child-reservations details[data-council-child-key]')
          .forEach(child => { if (expandedChildren.has(child.dataset.councilChildKey)) child.open = true; });
      }
      shownScope = nextScope;
    }).catch(reason => {
      if (serial !== current || !details.open || !details.isConnected) return;
      shownScope = null;
      body.replaceChildren();
      const error = document.createElement('p');
      error.setAttribute('role', 'alert');
      error.textContent = 'Could not inspect this Council: ' + (reason instanceof Error ? reason.message : String(reason));
      body.append(error);
    }).finally(() => {
      if (serial === current) refresh.disabled = false;
    });
  };
  details.addEventListener('toggle', () => {
    if (!details.open) { serial++; refresh.disabled = true; return; }
    load();
  });
  refresh.addEventListener('click', () => {
    if (details.open) load();
  });
  return details;
}
