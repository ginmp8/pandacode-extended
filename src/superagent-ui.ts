import './superagent.css';
import { escape } from './dom';

// This interface only invokes native policy/authorization decisions. It cannot
// grant tools, infer work, start tasks on import or bypass Council review.
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
interface Bot { id: string; name: string; paused: boolean }
interface Config {
  id: string; name: string; coordinatorBotId: string; workerBotId: string;
  enabled: boolean; policyRevision: number;
  ticketCount?: number; runningCount?: number; blockedCount?: number; lastActivityAt?: number;
  workerCount?: number; reviewerCount?: number;
}
interface TeamRoster { orchestrationId:string; policyRevision:number; coordinatorBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; executionReady:boolean }
interface EffectiveSkill {
  id:string; libraryStatus:string; safetyStatus:string;
  harnessEligibility:string; invocationEvidence:string;
}
interface EffectiveMember {
  role:string; position:number; botId:string; name:string;
  provider:string; model:string; mode:string; projectId:string;
  paused:boolean; skills:EffectiveSkill[]; configurationIssues?:string[];
}
interface EffectiveTeam {
  orchestrationId:string; policyRevision:number; enabled:boolean;
  executionReady:boolean; grantsIssued:boolean; members:EffectiveMember[];
}
interface Ticket { id: string; orchestrationId: string; objective: string; state: string; revision: number; evidence: string | null }
interface DemandIndex { id: string; orchestrationId: string; objective: string;
  state: string; revision: number; updatedAt: number }
interface Preview { wouldCreate: string[]; unchanged: string[]; importedPresetsDisabled: boolean }
interface ImportResult { created: string[]; unchanged: string[]; backupPath?: string }
interface Mandate {
  actor: string; authorizationId: string; policyRevision: number; projectId: string;
  expiresAt: number; revoked: boolean; allowedTools: string[]; allowedSideEffects: boolean;
}
interface Assignment { id: string; botId: string; sessionId: string; state: string;
  attempt: number; createdAt?: number; updatedAt?: number }
interface EffectReceipt {
  effectKey: string; payloadSha256: string; state: 'pending' | 'uncertain' | 'applied';
  createdAt: number; updatedAt: number;
}
interface AssignmentInsight {
  assignmentId: string; currentAssignment: boolean; hasResult: boolean;
  mandateRevoked: boolean; runBudgetExpired: boolean; remainingRunBudgetMs: number | null;
}
interface ManualPreflight {
  orchestrationId: string; policyRevision: number; projectId: string;
  selectedBotId: string; selectedRole: string; eligible: boolean; reason: string | null;
  allowedTools: string[]; allowedSideEffects: boolean; attemptBudgetMs: number;
}
interface Activity { id: number; kind: string; detail: string; at: number }
interface CouncilObservation {
  nativeStatus: string; nativeRound: number; nativeStale: boolean;
  nativePassVerified: boolean; ticketArtifactBound: boolean;
}
interface TicketDetails {
  ticket: Ticket; mandate: Mandate; assignments: Assignment[];
  attemptInsights?: AssignmentInsight[];
  activity: Activity[]; council?: CouncilObservation | null;
  finalizationGate?: 'not_pending' | 'native_recheck_required' | 'council_required' | 'effects_unresolved';
  unresolvedExternalEffects?: number;
}
interface EventSource {
  id: string; orchestrationId: string; repository: string; actorFilter: string;
  eventKind: string; mode: 'off' | 'observe'; policyRevision: number; lastSequence: number;
}
interface ThreadLink {
  sourceId: string; ticketId: string; repository: string; subjectKey: string; createdAt: number;
}
interface RunGraphNode {
  id: string; kind: string; state: string; label: string;
  botId: string | null; attempt: number | null; active: boolean;
}
interface RunGraphEdge { from: string; to: string; relation: string }
interface RunGraph {
  ticketId: string; ticketRevision: number; truncatedAttempts: boolean;
  nodes: RunGraphNode[]; edges: RunGraphEdge[];
}
interface SourceDecision {
  sourceId: string; deliveryId: string; eventKind: string; subjectKey: string;
  decision: string; sequence: number; policyRevision: number; at: number;
  checkConclusion?: string; checkHeadSha?: string; checkCompletedAt?: number;
}
interface SourceIntegrity {
  sourceId: string; repository: string; mode: 'off' | 'observe';
  policyRevision: number; cursorKind: string; lastSequence: number;
  receiptCount: number; permittedReceiptCount: number;
  maxReceiptSequence: number; cursorConsistent: boolean;
  receiptCapacityReached: boolean; nextAction: string;
  observationalOnly: boolean;
}

export function mountSuperagent(host: HTMLElement, call: Call): { refresh(): Promise<void> } {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let allDemands: DemandIndex[] = [], allDemandsLoaded = false, allDemandsHasMore = false;
  let demandTeamFilter = '', demandStateFilter = '';
  let teamSearch = '', teamStatusFilter: 'all' | 'enabled' | 'disabled' = 'all';
  let reviewQueue: DemandIndex[] = [], reviewQueueLoaded = false;
  let globalDemandLayout: 'board' | 'list' = 'board';
  let selected = '', busy = false, loading = true, error = '', feedback = '', importText = '';
  let workspaceView: 'teams' | 'runs' | 'review' | 'sources' | 'portable' = 'teams';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let demandObjective = '', demandMode: 'coordinated' | 'direct' = 'coordinated';
  let demandPreflight: ManualPreflight | null = null;
  let preflightObjective = '', preflightMode: 'coordinated' | 'direct' = 'coordinated';
  // Persist the user's editable preset draft across asynchronous list refreshes.
  let draftPresetName = '', draftPresetTemplate = '';
  let rejectingTicket = '', inspectedTicket = '';
  let inspectedAssignmentId = '';
  let activityMode: 'all' | 'compact' | 'json' = 'all';
  let activityFilter = '';
  let ticketDetails: TicketDetails | null = null;
  let effectReceipts: EffectReceipt[] = [], effectReceiptsLoaded = false;
  let runGraph: RunGraph | null = null, runGraphLoaded = false;
  let roleRoster: TeamRoster | null = null;
  // Draft order is committed only by the native revision-guarded roster update.
  let workerOrder: string[] = [];
  let effectiveTeam: EffectiveTeam | null = null;
  let sources: EventSource[] = [], sourcesLoaded = false, decisionSourceId = '';
  let decisions: SourceDecision[] = [], threadLinks: ThreadLink[] = [];
  let sourceIntegrity: SourceIntegrity | null = null;
  let linkedDecisions: SourceDecision[] = [], linkedDecisionsLoaded = false;
  const safe = (value: unknown) => escape(String(value ?? ''));
  const current = () => presets.find(item => item.id === selected);
  const botName = (id: string) => bots.find(bot => bot.id === id)?.name ?? 'Missing bot';
  const activityTime = (timestamp?: number): string => typeof timestamp === 'number' &&
    Number.isFinite(timestamp) && timestamp > 0
    ? new Date(timestamp).toLocaleString() : 'Unavailable';
  const observedCheck = (entry: SourceDecision): string =>
    entry.eventKind !== 'check_run.completed' ? '' :
    entry.checkConclusion && entry.checkHeadSha && entry.checkCompletedAt
      ? ' · Observed Check: ' + safe(entry.checkConclusion) +
        ' · Head ' + safe(entry.checkHeadSha.slice(0, 12)) +
        ' · Completed ' + safe(activityTime(entry.checkCompletedAt))
      : ' · Check outcome unavailable (legacy receipt)';
  function render(): void {
    if (!host.isConnected) return;
    const roleSummary = (item:Config): string =>
      typeof item.workerCount === 'number' && typeof item.reviewerCount === 'number' &&
      Number.isInteger(item.workerCount) && Number.isInteger(item.reviewerCount) &&
      item.workerCount >= 1 && item.reviewerCount >= 0
        ? '1 Coordinator · ' + item.workerCount + ' Workers · ' + item.reviewerCount + ' Reviewers'
        : 'Member counts unavailable';
    const visibleTeams = presets.filter(item =>
      item.name.toLowerCase().includes(teamSearch.trim().toLowerCase()) &&
      (teamStatusFilter === 'all' || item.enabled === (teamStatusFilter === 'enabled')));
    const list = visibleTeams.map(item =>
      '<li class="sa-item"><div><strong>' + safe(item.name) + '</strong><small>' +
      safe(botName(item.coordinatorBotId)) + ' / ' + safe(botName(item.workerBotId)) +
      '</small><small class="sa-role-counts" aria-label="Configured member counts">' +
      safe(roleSummary(item)) + '</small><small class="sa-counts" aria-label="Task counts">' +
      (item.ticketCount ?? 0) + ' tasks · ' + (item.runningCount ?? 0) +
      ' running · ' + (item.blockedCount ?? 0) + ' blocked</small>' +
      '<small class="sa-last-activity">Last activity: ' + safe(activityTime(item.lastActivityAt)) +
      '</small></div><span>' +
      (item.enabled ? 'Manual' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button></li>').join('');
    const optionsFor = (botId: string) => bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + (bot.id === botId ? ' selected' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const choices = optionsFor('');
    const chosen = current();
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const activity = (ticketDetails?.activity ?? []).filter(entry => {
      const needle = activityFilter.trim().toLowerCase();
      return !needle || (entry.kind + ' ' + entry.detail + ' ' + entry.id).toLowerCase().includes(needle);
    });
    const compactGroups = activity.reduce<Array<{first:Activity; count:number; last:Activity}>>((groups, event) => {
      const tail = groups[groups.length - 1];
      if (tail && tail.first.kind === event.kind && tail.first.detail === event.detail) {
        tail.count += 1; tail.last = event;
      } else {
        groups.push({first:event,count:1,last:event});
      }
      return groups;
    }, []);
    const auditContent = activityMode === 'json'
      ? '<pre class="sa-audit-json"><code>' + safe(JSON.stringify(activity, null, 2)) + '</code></pre>'
      : '<ol class="sa-audit-list">' +
        (activityMode === 'compact'
          ? compactGroups.map(group => '<li>' + safe(activityTime(group.first.at)) + ' · ' +
              safe(group.first.kind) + ' · ' + safe(group.first.detail) +
              (group.count > 1 ? ' · ' + group.count + ' consecutive native events' : '') +
              '</li>').join('')
          : activity.map(entry => '<li>' + safe(activityTime(entry.at)) + ' · ' +
              safe(entry.kind) + ' · ' + safe(entry.detail) + ' · Event ' + entry.id +
              '</li>').join('')) +
        (!activity.length ? '<li>No matching native events.</li>' : '') + '</ol>';
    const attempt = ticketDetails?.assignments.find(item => item.id === inspectedAssignmentId);
    const insight = ticketDetails?.attemptInsights?.find(item => item.assignmentId === inspectedAssignmentId);
    const attemptDetails = attempt
      ? '<section class="sa-assignment-inspector" role="region" aria-label="Native assignment inspector" tabindex="-1">' +
        '<h5>Attempt ' + attempt.attempt + ' · ' + safe(attempt.state) + '</h5>' +
        '<p>Native Ticket/Assignment snapshot; not provider liveness, a review verdict or a grant.</p>' +
        '<dl><dt>Bot</dt><dd>' + safe(botName(attempt.botId)) + '</dd>' +
        '<dt>Session</dt><dd><code>' + safe(attempt.sessionId) + '</code></dd>' +
        '<dt>Current Ticket assignment</dt><dd>' +
        (insight ? (insight.currentAssignment ? 'Yes' : 'No') : 'Unavailable') + '</dd>' +
        '<dt>Native mandate</dt><dd>' +
        (insight ? (insight.mandateRevoked ? 'Revoked' : 'Recorded (not a tool grant)') : 'Unavailable') + '</dd>' +
        '<dt>Provider result</dt><dd>' +
        (insight ? (insight.hasResult ? 'Recorded (content hidden)' : 'Not recorded') : 'Unknown') + '</dd>' +
        '<dt>Started / updated</dt><dd>' + safe(activityTime(attempt.createdAt)) +
        ' / ' + safe(activityTime(attempt.updatedAt)) + '</dd>' +
        '<dt>Attempt run budget</dt><dd>' +
        (!insight ? 'Unavailable' : insight.runBudgetExpired ? 'Expired' :
          insight.remainingRunBudgetMs === null ? 'Not running' :
          Math.max(0, Math.floor(insight.remainingRunBudgetMs / 1000)) + ' seconds remain') +
        '</dd></dl><button type="button" data-sa-close-assignment>Close attempt</button></section>'
      : '';
    const details = inspected && ticketDetails
      ? '<section class="sa-pane" role="region" aria-label="Ticket evidence">' +
        '<h4>Ticket evidence</h4><p>Read-only native snapshot. Model output is not task approval.</p>' +
        '<dl><dt>Ticket ID</dt><dd><code>' + safe(inspected.id) + '</code></dd>' +
        '<dt>State / revision</dt><dd>' + safe(inspected.state) + ' / ' + inspected.revision + '</dd>' +
        '<dt>Actor</dt><dd>' + safe(ticketDetails.mandate.actor) + '</dd>' +
        '<dt>Authorization</dt><dd><code>' + safe(ticketDetails.mandate.authorizationId) + '</code></dd>' +
        '<dt>Project / policy</dt><dd>' + safe(ticketDetails.mandate.projectId) + ' / ' +
        ticketDetails.mandate.policyRevision + '</dd>' +
        '<dt>Mandate</dt><dd>' + (ticketDetails.mandate.revoked ? 'Revoked' : 'Issued') +
        ' · ' + (ticketDetails.mandate.allowedSideEffects ? 'Scoped side effects' : 'No side effects allowed') +
        ' · Tools: ' + safe(ticketDetails.mandate.allowedTools.join(', ') || 'none') + '</dd>' +
        '<dt>Expires</dt><dd>' + safe(new Date(ticketDetails.mandate.expiresAt).toLocaleString()) + '</dd>' +
        '<dt>Finalization</dt><dd id="sa-finalization-blocked">' +
        (ticketDetails.finalizationGate === 'council_required'
          ? 'Blocked: native Council task review required; no verified task-bound approval.'
          : ticketDetails.finalizationGate === 'effects_unresolved'
            ? 'Blocked: ' + (ticketDetails.unresolvedExternalEffects ?? 0) +
              ' unresolved external effects require authoritative reconciliation.'
            : ticketDetails.finalizationGate === 'native_recheck_required'
              ? 'Manual decision possible only after native authorization recheck.'
              : 'No finalization decision pending.') + '</dd></dl>' +
        '<h5>Assignment attempts</h5><ul>' +
        (ticketDetails.assignments.map(item => '<li>Attempt ' + item.attempt + ' · ' +
          safe(item.state) + ' · Bot ' + safe(botName(item.botId)) + ' · Session <code>' +
          safe(item.sessionId) + '</code> <button type="button" data-sa-inspect-assignment="' +
          safe(item.id) + '" aria-expanded="' + (item.id === inspectedAssignmentId) +
          '">Inspect attempt ' + item.attempt + '</button></li>').join('') ||
          '<li>No worker assigned.</li>') +
        '</ul>' + attemptDetails +
        '<section class="sa-run-graph" role="region" aria-label="Native run graph">' +
        '<h5>Execution graph</h5><p>Native Ticket ownership edges only. ' +
        'No inferred collaboration, child delegation or Council approval.</p>' +
        '<button type="button" data-sa-load-run-graph>Inspect graph</button>' +
        (runGraphLoaded && runGraph && runGraph.ticketId === inspected.id &&
         runGraph.ticketRevision === ticketDetails.ticket.revision
          ? '<ol class="sa-run-graph-nodes">' +
            (runGraph.nodes.map(node => '<li><strong>' + safe(node.label) + '</strong>' +
              ' · ' + safe(node.state) +
              (node.botId ? ' · Bot ' + safe(botName(node.botId)) : '') +
              (node.active ? ' · Active native assignment' : '') +
              '<small>' + (node.kind === 'ticket' ? 'Root Ticket' :
                runGraph!.edges.find(edge => edge.to === node.id)?.relation === 'owns_attempt'
                  ? 'Ticket owns attempt' : 'Ticket awaits decision') +
              '</small>' +
              (node.kind === 'assignment' && ticketDetails?.assignments.some(item =>
                node.id === 'assignment:' + item.id &&
                node.botId === item.botId && node.attempt === item.attempt)
                ? ' <button type="button" data-sa-graph-inspect-assignment="' +
                  safe(node.id.slice('assignment:'.length)) + '">' +
                  'Inspect graph attempt ' + node.attempt + '</button>' : '') +
              '</li>').join('') || '<li>No durable nodes.</li>') +
            '</ol>' + (runGraph.truncatedAttempts
              ? '<p>Only the latest 50 attempts are shown.</p>' : '') :
            '<p>Graph not loaded.</p>') + '</section>' +
        '<section class="sa-effect-receipts" role="region" aria-label="Native effect receipts">' +
        '<h5>External effect receipts</h5><p>Read-only native identity and state. ' +
        'Pending may still be running; uncertain requires independent provider reconciliation. ' +
        'Neither state permits replay, approval or assumed success.</p>' +
        '<button type="button" data-sa-load-effect-receipts>Inspect receipts</button>' +
        (effectReceiptsLoaded ? '<ol>' +
          (effectReceipts.map(item => '<li><code>' + safe(item.effectKey) + '</code> · ' +
            safe(item.state) + ' · SHA-256 <code>' + safe(item.payloadSha256) +
            '</code> · Updated ' + safe(activityTime(item.updatedAt)) + '</li>').join('') ||
            '<li>No native external effect receipts for this Ticket.</li>') + '</ol>' :
            '<p>Receipts not loaded.</p>') + '</section>' +
        '<h5>Native activity</h5>' +
        '<nav class="sa-activity-tabs" aria-label="Activity display">' +
        (['all','compact','json'] as const).map(mode =>
          '<button type="button" data-sa-activity-mode="' + mode + '" aria-pressed="' +
          (mode === activityMode) + '">' + mode[0].toUpperCase() + mode.slice(1) +
          '</button>').join('') + '</nav>' +
        '<label class="sa-activity-filter">Filter native activity<input type="search" data-sa-activity-filter value="' +
        safe(activityFilter) + '" placeholder="Event type, detail or ID"></label>' +
        '<p class="sa-activity-source">' + activity.length + ' native events in view · ' +
        'bounded latest-50 Ticket snapshot, native audit order. No inferred status.</p>' +
        auditContent +
        '<h5>Linked GitHub threads (context references only)</h5><ul class="sa-linked-threads">' +
        (threadLinks.map(link => '<li>' + safe(link.repository) + ' / ' +
          safe(link.subjectKey) + ' <button type="button" data-sa-unlink-source="' +
          safe(link.sourceId) + '" data-sa-unlink-thread="' + safe(link.subjectKey) +
          '" aria-label="Unlink ' + safe(link.repository) + ' / ' +
          safe(link.subjectKey) + '">Unlink</button></li>').join('') ||
          '<li>No threads linked.</li>') + '</ul>' +
        '<section class="sa-linked-activity" role="region" aria-label="Observed linked GitHub activity">' +
        '<h5>Observed linked GitHub activity</h5><p>Read-only, bounded native Observe receipts for explicitly linked threads. ' +
        'Event text is not loaded and no additional work is inferred.</p>' +
        '<button type="button" data-sa-load-linked-activity>Load linked source decisions</button>' +
        (linkedDecisionsLoaded ? '<ol>' +
          (linkedDecisions.map(entry => '<li>' + safe(activityTime(entry.at)) + ' · ' +
            safe(entry.eventKind) + ' · ' + safe(entry.subjectKey) + ' · ' +
            safe(entry.decision) + observedCheck(entry) +
            ' · Delivery ' + safe(entry.deliveryId) + '</li>').join('') || '<li>No authenticated Observe decisions match linked threads.</li>') +
          '</ol>' : '') + '</section>' +
        (sourcesLoaded && sources.length
          ? '<form data-sa-form="thread-link"><label>Event source<select name="sourceId" required>' +
            sources.map(src => '<option value="' + safe(src.id) + '">' +
              safe(src.repository) + ' · ' + safe(src.eventKind) + '</option>').join('') +
            '</select></label><label>Exact thread key<input name="subjectKey" required ' +
            'maxlength="24" placeholder="issue:42 or pull:123"></label>' +
            '<button type="submit">Link thread (context only)</button></form>'
          : '<p>Load event sources below before linking a thread.</p>') +
        (sourcesLoaded && threadLinks.some(link => link.subjectKey.startsWith('issue:'))
          ? '<form data-sa-form="issue-pull-link"><p>Operator-asserted PR association; ' +
            'not GitHub-verified and not a permission grant.</p>' +
            '<label>Linked issue<select name="issueLink" required>' +
            threadLinks.filter(link => link.subjectKey.startsWith('issue:')).map(link =>
              '<option value="' + safe(link.sourceId + '|' + link.subjectKey) + '">' +
              safe(link.repository) + ' / ' + safe(link.subjectKey) + '</option>').join('') +
            '</select></label><label>Exact PR thread key<input name="pullKey" required ' +
            'maxlength="24" placeholder="pull:123"></label>' +
            '<button type="submit">Associate PR with linked issue (context only)</button></form>'
          : '') +
        (ticketDetails.council
          ? '<section class="sa-council-observation" role="note" aria-label="Native Council observation">' +
            '<h5>Native Council (advisory)</h5><p>Native team status: ' +
            safe(ticketDetails.council.nativeStatus) + ' · Round ' +
            safe(ticketDetails.council.nativeRound) +
            (ticketDetails.council.nativeStale ? ' · stale' : '') +
            '. Not bound to this ticket artifact; cannot authorize SuperAgent completion.</p></section>'
          : '') +
        '<button type="button" data-sa-close-details>Close evidence</button></section>'
      : '';
    const sourceRows = sources.map(src =>
      '<li><div><strong>' + safe(src.repository) + '</strong><small>' +
      safe(src.eventKind) + ' · ' + safe(src.actorFilter) + ' · Cursor ' +
      src.lastSequence + ' · Revision ' + src.policyRevision + '</small></div>' +
      '<span>' + safe(src.mode) + '</span>' +
      '<button type="button" data-sa-source-mode="' + safe(src.id) +
      '" data-sa-next-mode="' + (src.mode === 'off' ? 'observe' : 'off') + '">' +
      (src.mode === 'off' ? 'Enable Observe' : 'Turn Off') + '</button>' +
      (src.mode === 'observe'
        ? (src.eventKind === 'check_run.completed'
          ? '<label>Check Run ID <input data-sa-check-run-id="' + safe(src.id) +
            '" inputmode="numeric" pattern="[1-9][0-9]*" maxlength="16" aria-label="Check Run ID" /></label>' +
            '<button type="button" data-sa-source-check-run="' + safe(src.id) +
            '">Observe authenticated Check Run</button>'
          : '<button type="button" data-sa-source-poll="' + safe(src.id) +
            '">Poll authenticated GitHub</button>') : '') +
      '<button type="button" data-sa-source-decisions="' + safe(src.id) +
      '">View decisions</button>' +
      '<button type="button" data-sa-source-health="' + safe(src.id) +
      '">Inspect source health</button></li>').join('');
    const sourcePanel = '<section class="sa-event-sources" aria-label="GitHub event sources">' +
      '<h4>GitHub event sources</h4><p>Off/Observe only. Authenticated GitHub Events polling is bounded; Checks API requires an explicit Check Run ID and a unique matching PR head. Neither is a gap-free subscription or an authorization to launch tasks or external writes.</p>' +
      (!sourcesLoaded ? '<button type="button" data-sa-load-sources>Load sources</button>'
        : '<form data-sa-form="source"><div class="sa-grid">' +
          '<label>Repository (owner/name)<input name="repository" maxlength="200" required></label>' +
          '<label>Exact actor<input name="actorFilter" maxlength="80" required></label></div>' +
          '<label>Event class<select name="eventKind">' +
          ['issues.opened','issues.edited','issues.closed','issues.reopened',
            'issue_comment.created','pull_request.opened',
            'pull_request.closed','pull_request.reopened','pull_request_review.submitted',
            'pull_request_review.updated','pull_request_review.dismissed',
            'pull_request_review_comment.created','check_run.completed']
            .map(kind => '<option value="' + kind + '">' + kind + '</option>').join('') +
          '</select></label><button type="submit">Add source (Off)</button></form>' +
          '<ul class="sa-tickets sa-source-list">' +
          (sourceRows || '<li class="sa-empty">No event sources configured.</li>') + '</ul>' +
          (sourceIntegrity ? '<section role="region" aria-label="Source integrity">' +
            '<h5>Native source integrity</h5><p>' +
            safe(sourceIntegrity.repository) + ' · ' + safe(sourceIntegrity.mode) +
            ' · Revision ' + safe(sourceIntegrity.policyRevision) + '</p>' +
            '<p>Cursor type: ' +
            safe(sourceIntegrity.cursorKind === 'local_check_run_observation_count'
              ? 'Local Check Run observation count' : 'GitHub Events high-watermark') +
            ' · Stored cursor: ' + safe(sourceIntegrity.lastSequence) +
            ' · Highest durable receipt: ' + safe(sourceIntegrity.maxReceiptSequence) +
            '</p><p>Decisions: ' + safe(sourceIntegrity.receiptCount) +
            ' · Permitted: ' + safe(sourceIntegrity.permittedReceiptCount) +
            ' · ' + (sourceIntegrity.receiptCapacityReached
              ? 'Receipt capacity reached — operator intervention required'
              : 'Receipt capacity available') + '</p>' +
            '<p>' + (sourceIntegrity.cursorConsistent
              ? 'Cursor is consistent with stored receipts.'
              : 'Cursor behind durable receipts — new observations denied until reconciled.') +
            ' Native next action: ' + safe(sourceIntegrity.nextAction) + '</p>' +
            '<p>Observation only. No GitHub subscription completeness, worker admission, Council pass, or Auto authorization is implied.</p>' +
            '</section>' : '') +
          (decisionSourceId ? '<div role="region" aria-label="Source decisions">' +
            '<h5>Recorded decisions</h5><ol>' +
            (decisions.map(d => '<li>' + safe(activityTime(d.at)) + ' · ' +
              safe(d.decision) + ' · ' + safe(d.eventKind) + ' · ' +
              safe(d.subjectKey) + observedCheck(d) +
              ' · Delivery ' + safe(d.deliveryId) +
              ' · Revision ' + d.policyRevision + '</li>').join('') ||
              '<li>No recorded events.</li>') + '</ol></div>' : '')) + '</section>';
    // One native read of the bounded cross-Team Demand index powers both
    // accessible presentations. A board column is not a scheduler state.
    const demandCard = (demand: DemandIndex) =>
      '<li class="sa-demand-card"><div><strong>' + safe(demand.objective) +
      '</strong><small>' + safe(presets.find(item => item.id === demand.orchestrationId)?.name ??
        'Unknown Team') + ' · ' + safe(demand.state) + ' · Revision ' +
      demand.revision + ' · Updated ' + safe(activityTime(demand.updatedAt)) +
      '</small></div><button type="button" data-sa-open-demand="' + safe(demand.id) +
      '">Open demand</button></li>';
    const demandGroups = [
      {label:'Working',states:['queued','running']},
      {label:'Awaiting decision',states:['waiting_approval','waiting_review']},
      {label:'Needs attention',states:['blocked','failed','interrupted','inconclusive']},
      {label:'Finished or stopped',states:['completed','stopped']},
    ];
    const unmatchedDemands = allDemands.filter(demand =>
      !demandGroups.some(group => group.states.includes(demand.state)));
    const demandBoard = '<div class="sa-demand-board" role="group" aria-label="Demand status board">' +
      demandGroups.concat(unmatchedDemands.length ? [{label:'Other states',states:[]}] : [])
        .map(group => {
          const entries = group.states.length ? allDemands.filter(demand =>
            group.states.includes(demand.state)) : unmatchedDemands;
          return '<section class="sa-demand-column" aria-label="' + safe(group.label) + ' Demands">' +
            '<h4>' + safe(group.label) + ' <span class="sa-demand-count">' +
            entries.length + '</span></h4><ul class="sa-tickets">' +
            (entries.map(demandCard).join('') || '<li class="sa-empty">No Demands in this state.</li>') +
            '</ul></section>';
        }).join('') + '</div>';
    const globalDemands = '<section class="sa-pane sa-global-demands" aria-label="All Demands across Teams">' +
      '<header><h3>All Demands / Runs</h3><button type="button" data-sa-load-all-demands>' +
      (demandTeamFilter ? 'Refresh selected Team' : 'Refresh all Teams') + '</button></header>' +
      '<label class="sa-demand-team-filter">Filter by Team<select aria-label="Filter Demands by Team" data-sa-demand-team-filter>' +
      '<option value="">All Teams</option>' +
      presets.map(item => '<option value="' + safe(item.id) + '"' +
        (demandTeamFilter === item.id ? ' selected' : '') + '>' + safe(item.name) + '</option>').join('') +
      '</select></label>' +
      '<label class="sa-demand-state-filter">Filter by State<select aria-label="Filter Demands by State" data-sa-demand-state-filter>' +
      '<option value="">All States</option>' +
      (['queued','running','waiting_approval','waiting_review','blocked','completed',
        'failed','stopped','interrupted','inconclusive'] as const)
        .map(state => '<option value="' + state + '"' +
          (demandStateFilter === state ? ' selected' : '') + '>' +
          state.split('_').join(' ') + '</option>').join('') +
      '</select></label>' +
      '<p>Native Tickets ' + (demandTeamFilter ? 'for this Team' : 'across Teams') +
      (demandStateFilter ? ' in state ' + safe(demandStateFilter) : '') +
      ', loaded 200 at a time in newest-first order. Board counts cover ' +
      'only loaded pages, not all saved Demands. Open a Demand to act within its Team.</p>' +
      (!allDemandsLoaded ? '<p role="status">Load Demands for the selected Team scope.</p>' :
        '<div class="sa-demand-layout-toggle" role="group" aria-label="Demand presentation">' +
        (['board','list'] as const).map(layout =>
          '<button type="button" data-sa-global-demand-layout="' + layout +
          '" aria-pressed="' + (globalDemandLayout === layout) + '">' +
          (layout === 'board' ? 'Board' : 'List') + '</button>').join('') + '</div>' +
        (globalDemandLayout === 'board' ? demandBoard :
          '<ul class="sa-tickets sa-global-demand-list">' +
          (allDemands.map(demandCard).join('') ||
          '<li class="sa-empty">No native Demands / Runs found.</li>') + '</ul>') +
        '<p role="status">' + allDemands.length + ' native Demands loaded.</p>' +
        (allDemandsHasMore ? '<button type="button" data-sa-load-older-demands>Load older Demands</button>' : '')) +
      '</section>';
    const nativePreflight = demandPreflight && demandPreflight.orchestrationId === selected &&
      preflightObjective === demandObjective.trim() && preflightMode === demandMode
      ? '<section class="sa-pane" role="region" aria-label="Native demand preflight">' +
        '<h4>Native demand preflight</h4><p role="status">' +
        (demandPreflight.eligible ? 'Eligible for manual read-only admission now.' :
          'Not eligible for immediate admission: ' + safe(demandPreflight.reason)) + '</p>' +
        '<dl><dt>Selected role and bot</dt><dd>' + safe(demandPreflight.selectedRole) +
        ' · ' + safe(botName(demandPreflight.selectedBotId)) + '</dd>' +
        '<dt>Team project / policy revision</dt><dd>' + safe(demandPreflight.projectId) +
        ' / ' + demandPreflight.policyRevision + '</dd>' +
        '<dt>Authorized tools / effects</dt><dd>' +
        safe(demandPreflight.allowedTools.join(', ') || 'none') + ' / ' +
        (demandPreflight.allowedSideEffects ? 'scoped' : 'no side effects') + '</dd>' +
        '<dt>Attempt time budget</dt><dd>' +
        safe(String(Math.floor(demandPreflight.attemptBudgetMs / 60000))) + ' minutes</dd></dl>' +
        '<p>This is a read-only snapshot, not a grant, Ticket or guaranteed reservation. ' +
        'Native admission rechecks all policy, availability and checkout limits.</p></section>' : '';
    const active = chosen ? '<section class="sa-pane" aria-label="Selected orchestration"><header><h3>' +
      safe(chosen.name) + '</h3><div class="sa-actions"><button type="button" data-sa-enable="' +
      safe(chosen.id) + '"' +
      (!chosen.enabled && ((chosen.workerCount ?? 1)>1 || (chosen.reviewerCount ?? 0)>0)
        ? ' disabled title="Multi-member execution requires native scoped admission"'
        : '') + '>' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused</button></div></header>' +
      '<form data-sa-form="edit"><label>Preset title<input name="name" required maxlength="120" value="' + safe(chosen.name) + '"></label>' +
      '<div class="sa-grid"><label>Edit coordinator<select name="coordinatorBotId" required>' +
      optionsFor(chosen.coordinatorBotId) + '</select></label>' +
      '<label>Edit worker<select name="workerBotId" required>' + optionsFor(chosen.workerBotId) +
      '</select></label></div><button class="primary-button" type="submit"' +
      (chosen.enabled ? ' disabled' : '') + '>Save preset</button></form>' +
      '<p>Policy revision ' + chosen.policyRevision + ' · Native read-only permission until separately authorized.</p>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required>' +
      safe(demandObjective) + '</textarea></label>' +
      '<label>Dispatch<select name="mode"><option value="coordinated"' +
      (demandMode === 'coordinated' ? ' selected' : '') + '>Coordinator, then worker</option>' +
      '<option value="direct"' + (demandMode === 'direct' ? ' selected' : '') +
      '>Worker directly</option></select></label>' +
      '<button type="button" data-sa-preflight>Review native preflight</button>' +
      '<button class="primary-button" type="submit"' + (chosen.enabled ? '' : ' disabled') +
      '>Dispatch manually</button></form>' + nativePreflight +
      '<div class="sa-title"><h4>Tickets</h4><button type="button" data-sa-refresh>Refresh</button></div>' +
      '<ul class="sa-tickets">' + (tickets.map(ticket =>
        '<li><div><strong>' + safe(ticket.objective) + '</strong><small>' + safe(ticket.state) +
        ' · Revision ' + ticket.revision + '</small></div>' +
        '<button type="button" data-sa-inspect-ticket="' + safe(ticket.id) +
        '" aria-expanded="' + (ticket.id === inspectedTicket) + '">View evidence</button>' +
        (ticket.state === 'waiting_approval'
          ? '<button type="button" data-sa-accept="' + safe(ticket.id) + '"' +
            (ticketDetails?.ticket.id === ticket.id &&
            (ticketDetails.finalizationGate === 'council_required' ||
             ticketDetails.finalizationGate === 'effects_unresolved')
              ? ' disabled aria-describedby="sa-finalization-blocked"' : '') +
            '>Accept evidence</button>' +
            '<button type="button" data-sa-reject="' + safe(ticket.id) + '">Reject evidence</button>' : '') +
        (ticket.state === 'queued' ? '<button type="button" data-sa-dispatch="' + safe(ticket.id) + '">Dispatch worker</button>' : '') +
        (ticket.state === 'blocked' ? '<button type="button" data-sa-retry="' + safe(ticket.id) + '">Retry manually</button>' : '') +
        (!['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)
          ? '<button type="button" data-sa-stop="' + safe(ticket.id) + '">Stop</button>' : '') +
        '</li>').join('') || '<li class="sa-empty">No tickets yet.</li>') + '</ul>' +
        (rejectingTicket ? '<form data-sa-form="reject"><label>Rejection reason<textarea name="reason" maxlength="500" required></textarea></label>' +
          '<div class="sa-actions"><button type="submit" class="primary-button">Submit rejection</button>' +
          '<button type="button" data-sa-reject-cancel>Cancel</button></div></form>' : '') +
        details + '</section>' : '';
    const roleOptions = (selectedRoles:string[]) => bots
      .filter(bot => chosen && bot.id !== chosen.coordinatorBotId && bot.id !== chosen.workerBotId)
      .map(bot => '<option value="' + safe(bot.id) + '"' +
        (selectedRoles.includes(bot.id) ? ' selected' : '') +
        (bot.paused ? ' disabled' : '') + '>' + safe(bot.name) +
        (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const effectivePolicy = chosen && effectiveTeam &&
      effectiveTeam.orchestrationId === chosen.id &&
      effectiveTeam.policyRevision === chosen.policyRevision
      ? '<section role="region" aria-label="Native effective Team policy">' +
        '<h4>Effective Team roles and Skills</h4><p>Native configuration snapshot, not admission. ' +
        'Library status is distinct from provider eligibility and verified invocation. ' +
        'No role, model or Skill grants tools.</p>' +
        '<p>Policy revision ' + effectiveTeam.policyRevision + ' · ' +
        (effectiveTeam.enabled ? 'Team enabled for manual checks' : 'Team Off') + ' · ' +
        (effectiveTeam.executionReady ? 'Single-worker topology' : 'Multi-role execution unavailable') +
        ' · No task grant issued</p><ol>' +
        effectiveTeam.members.map(member =>
          '<li><strong>' + safe(member.role) + ' ' + (member.position + 1) +
          ': ' + safe(member.name) + '</strong>' +
          ' · Harness ' + safe(member.provider) + ' · Model ' + safe(member.model) +
          ' · Bot mode ' + safe(member.mode) + ' · Project ' + safe(member.projectId) +
          (member.paused ? ' · Paused' : ' · Not paused (live availability unverified)') +
          '<section class="sa-member-issues" aria-label="Local configuration checks">' +
          (member.configurationIssues?.length
            ? '<p>Native configuration issues:</p><ul>' +
              member.configurationIssues.map(issue => '<li>' + safe(issue) + '</li>').join('') + '</ul>'
            : '<p>No local configuration issues detected. Harness/tool eligibility unverified.</p>') +
          '</section><ul>' + (member.skills.map(skill =>
            '<li>Skill ' + safe(skill.id) + ' · Library ' + safe(skill.libraryStatus) +
            ' · Safety ' + safe(skill.safetyStatus) +
            ' · Harness eligibility ' + safe(skill.harnessEligibility) +
            ' · Invocation/evidence ' + safe(skill.invocationEvidence) + '</li>').join('') ||
            '<li>No bot-assigned Skills</li>') + '</ul></li>').join('') + '</ol></section>'
      : '';
    const teamRoles = chosen
      ? '<section class="sa-pane" aria-label="Configured Team roles"><h3>Configure Team roles</h3>' +
        '<p>Roles reuse existing bot identities, models, harnesses and Skills. Additional Workers and optional Reviewers are configuration only until native role admission is implemented.</p>' +
        (!roleRoster || roleRoster.orchestrationId !== chosen.id
          ? '<button type="button" data-sa-role-load>Load native members</button>'
          : '<p>Coordinator: ' + safe(botName(roleRoster.coordinatorBotId)) +
            ' · Primary Worker: ' + safe(botName(roleRoster.workerBotIds[0])) +
            ' · Policy revision ' + roleRoster.policyRevision + '</p>' +
            '<form data-sa-form="roster">' +
              '<label>Additional Workers (up to 7)<select name="additionalWorkers" data-sa-worker-select multiple size="5">' +
              roleOptions(workerOrder) + '</select></label>' +
              '<ol class="sa-worker-order" aria-label="Additional Worker order">' +
              workerOrder.map((id,index) => '<li><span>' + (index + 2) + '. ' + safe(botName(id)) +
                '</span><button type="button" data-sa-worker-move="' + safe(id) +
                '" data-sa-direction="up" aria-label="Move ' + safe(botName(id)) +
                ' up"' + (index === 0 || chosen.enabled ? ' disabled' : '') + '>Up</button>' +
                '<button type="button" data-sa-worker-move="' + safe(id) +
                '" data-sa-direction="down" aria-label="Move ' + safe(botName(id)) +
                ' down"' + (index === workerOrder.length - 1 || chosen.enabled ? ' disabled' : '') +
                '>Down</button></li>').join('') + '</ol>' +
              '<label>Optional Council Reviewers (up to 4)<select name="reviewers" multiple size="5">' +
              roleOptions(roleRoster.reviewerBotIds) + '</select></label>' +
              '<p>A bot may have one role only. Workers must share the primary Worker project. A disabled Team may change roles after all demands settle; active/reviewable work and unresolved effects block native saves.</p>' +
              '<button class="primary-button" type="submit"' +
                (chosen.enabled || tickets.some(ticket => !['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)) ? ' disabled' : '') +
                '>Save configured roles</button></form>' +
              '<p role="status">' + (roleRoster.executionReady ?
                'Single-worker manual execution available after explicit enable.' :
                'Multi-member execution is disabled until native scoped admission is available.') + '</p>' +
            '<button type="button" data-sa-effective-load>Inspect effective policy and Skills</button>' +
            effectivePolicy) +
        '</section>' : '';
    const overview = chosen
      ? '<section class="sa-pane" aria-label="Team overview"><h3>Selected Team</h3>' +
        '<p>' + safe(chosen.name) + ' · ' + (chosen.enabled ? 'Manual read-only' : 'Disabled / Off') +
        ' · Policy revision ' + chosen.policyRevision + '</p>' +
        '<dl><dt>Coordinator (read-only)</dt><dd>' + safe(botName(chosen.coordinatorBotId)) + '</dd>' +
        '<dt>Worker</dt><dd>' + safe(botName(chosen.workerBotId)) + '</dd>' +
        '<dt>Configured members</dt><dd>' + safe(roleSummary(chosen)) + '</dd>' +
        '<dt>Current native support</dt><dd>One worker, manual dispatch; scoped writes and automatic GitHub intake unsupported.</dd></dl>' +
        '<button type="button" data-sa-open-runs>Assign demand / Open runs</button></section>'
      : '<section class="sa-pane"><h3>No Teams yet</h3><p>Create a disabled Team from two eligible existing bots. Nothing starts automatically.</p></section>';
    const globalReviewPanel = '<section class="sa-pane sa-cross-team-review" ' +
      'role="region" aria-label="Cross-Team native review inbox"><header>' +
      '<h3>Review inbox · All Teams</h3><button type="button" data-sa-load-review-inbox>' +
      'Refresh review inbox</button></header>' +
      '<p>Up to 200 recently updated native Tickets requiring human acceptance or ' +
      'awaiting Council. A waiting Council verdict cannot be manually treated as passed. ' +
      'Open a Demand to inspect its native Team and revision before deciding.</p>' +
      (!reviewQueueLoaded ? '<p role="status">Load native review queue to inspect all Teams.</p>' :
        '<ul class="sa-tickets">' + (reviewQueue.map(item =>
          '<li><div><strong>' + safe(item.objective) + '</strong><small>' +
          safe(presets.find(team => team.id === item.orchestrationId)?.name ?? 'Unknown Team') +
          ' · ' + safe(item.state === 'waiting_approval' ? 'Human decision' :
            'Awaiting native Council (not approved)') + ' · Revision ' +
          item.revision + '</small></div><button type="button" data-sa-open-demand="' +
          safe(item.id) + '">Open demand</button></li>').join('') ||
          '<li>No native Tickets currently awaiting a decision.</li>') + '</ul>') +
      '</section>';
    const selectedTeamReview = chosen
      ? '<section class="sa-pane" aria-label="Review inbox"><h3>Review inbox · ' + safe(chosen.name) +
        '</h3><p>Only native Tickets awaiting explicit human evidence acceptance; advisory Council is not a verified pass.</p>' +
        '<ul class="sa-tickets">' +
        (tickets.filter(ticket => ticket.state === 'waiting_approval').map(ticket =>
          '<li><div><strong>' + safe(ticket.objective) + '</strong><small>Ticket ' +
          safe(ticket.id) + ' · Revision ' + ticket.revision + '</small></div>' +
          '<button type="button" data-sa-inspect-ticket="' + safe(ticket.id) + '">View evidence</button>' +
          '<button type="button" data-sa-accept="' + safe(ticket.id) + '"' +
            (ticketDetails?.ticket.id === ticket.id &&
            (ticketDetails.finalizationGate === 'council_required' ||
             ticketDetails.finalizationGate === 'effects_unresolved')
              ? ' disabled aria-describedby="sa-finalization-blocked"' : '') +
            '>Accept evidence</button>' +
          '<button type="button" data-sa-reject="' + safe(ticket.id) + '">Reject evidence</button></li>'
        ).join('') || '<li class="sa-empty">No Tickets awaiting human approval for this Team.</li>') +
        '</ul>' +
        (rejectingTicket ? '<form data-sa-form="reject"><label>Rejection reason<textarea name="reason" maxlength="500" required></textarea></label>' +
          '<div class="sa-actions"><button type="submit" class="primary-button">Submit rejection</button>' +
          '<button type="button" data-sa-reject-cancel>Cancel</button></div></form>' : '') +
        details + '</section>'
      : '<section class="sa-pane"><p>Select a Team to inspect its native human review queue.</p></section>';
    const reviewPanel = globalReviewPanel + selectedTeamReview;
    const navigation = '<nav class="sa-navigation" aria-label="Orchestration views">' +
      ([
        ['teams','Teams'],['runs','Demands / Runs'],['review','Review inbox'],
        ['sources','Sources'],['portable','Portable configuration'],
      ] as const).map(([id,label]) =>
        '<button type="button" data-sa-view="' + id + '" aria-current="' +
        (workspaceView === id ? 'page' : 'false') + '">' + label + '</button>').join('') +
      '</nav>';
    host.innerHTML = '<div class="sa-ui">' +
      '<p class="sa-intro">A Team is a reusable SuperAgent. All Run actions below use native Ticket and Mandate records; writes, automatic GitHub dispatch and Council verdicts are not yet supported.</p>' +
      (loading ? '<p role="status">Loading orchestrations…</p>' : '') +
      (error ? '<p role="alert" class="sa-error">' + safe(error) + '</p>' : '') +
      (feedback ? '<p role="status" class="sa-feedback">' + safe(feedback) + '</p>' : '') +
      (confirm ? '<section role="alertdialog" aria-label="Confirm orchestration action" class="sa-confirm"><p>' +
        safe(confirm.message) + '</p><div class="sa-actions"><button type="button" data-sa-cancel>Cancel</button>' +
        '<button class="primary-button" type="button" data-sa-confirm>Confirm</button></div></section>' : '') +
      navigation +
      '<div class="sa-layout"><aside class="sa-directory" aria-label="Teams directory">' +
      '<section class="sa-pane"><h3>Teams</h3>' +
      '<label>Search Teams<input type="search" data-sa-team-search value="' +
      safe(teamSearch) + '" placeholder="Team name"></label>' +
      '<label>Team status<select data-sa-team-status>' +
      '<option value="all"' + (teamStatusFilter === 'all' ? ' selected' : '') + '>All statuses</option>' +
      '<option value="enabled"' + (teamStatusFilter === 'enabled' ? ' selected' : '') + '>Manual enabled</option>' +
      '<option value="disabled"' + (teamStatusFilter === 'disabled' ? ' selected' : '') + '>Disabled / Off</option>' +
      '</select></label><p role="status">' + visibleTeams.length + ' of ' +
      presets.length + ' native Teams displayed. Selection remains open when filtered out.</p>' +
      '<ul class="sa-list">' + (list || '<li class="sa-empty">' +
      (presets.length ? 'No Teams match these filters.' : 'No orchestrations created.') +
      '</li>') + '</ul></section>' +
      '</aside><div class="sa-stage">' +
      (workspaceView === 'teams'
        ? '<section class="sa-pane"><h3>New manual preset</h3><form data-sa-form="create">' +
          '<label>Example preset<select data-sa-template aria-label="Example preset">' +
          '<option value="">Custom</option>' +
          '<option value="Software Development"' +
          (draftPresetTemplate === 'Software Development' ? ' selected' : '') +
          '>Software Development</option>' +
          '<option value="Issue Resolution"' +
          (draftPresetTemplate === 'Issue Resolution' ? ' selected' : '') +
          '>Issue Resolution</option></select></label>' +
          '<label>Name<input name="name" maxlength="120" placeholder="Software Development" value="' +
          safe(draftPresetName) + '" required></label>' +
          '<div class="sa-grid"><label>Coordinator<select name="coordinatorBotId" required>' +
          '<option value="">Choose bot</option>' + choices + '</select></label>' +
          '<label>Worker<select name="workerBotId" required><option value="">Choose bot</option>' +
          choices + '</select></label></div>' +
          '<details class="sa-new-members"><summary>Additional Team members (optional)</summary>' +
          '<p>Choose up to seven additional Workers and four independent Reviewers. ' +
          'Each bot has one role, and Workers must share the primary Worker project. ' +
          'Multi-member Teams remain Off until scoped admission is supported.</p>' +
          '<label>Additional Workers<select name="additionalWorkers" multiple size="4">' +
          choices + '</select></label>' +
          '<label>Optional Reviewers<select name="reviewers" multiple size="4">' +
          choices + '</select></label></details>' +
          '<button class="primary-button" type="submit"' +
          (bots.length < 2 ? ' disabled' : '') + '>Create disabled preset</button></form></section>' + overview + teamRoles
        : '') +
      (workspaceView === 'runs' ? globalDemands + (active ||
        '<section class="sa-pane"><p>Select a Team from the directory to assign and inspect native Tickets.</p></section>') : '') +
      (workspaceView === 'review' ? reviewPanel : '') +
      (workspaceView === 'sources' ? (chosen ? '<section class="sa-pane">' + sourcePanel + '</section>'
        : '<section class="sa-pane"><p>Select a Team to configure its Off/Observe GitHub sources.</p></section>') : '') +
      (workspaceView === 'portable'
        ? '<section class="sa-pane"><h3>Portable configuration</h3>' +
          '<p>Configuration only: no credentials, memories, conversations, approvals or live tasks.</p>' +
          '<label>Companion manifest JSON<textarea data-sa-manifest rows="5" spellcheck="false">' +
          safe(importText) + '</textarea></label>' +
          '<div class="sa-actions"><button type="button" data-sa-export>Export</button>' +
          '<button type="button" data-sa-preview>Preview import</button></div>' +
          (preview ? '<p role="status">' + preview.wouldCreate.length + ' new; ' +
            preview.unchanged.length + ' unchanged. Imported presets will be disabled.</p>' +
            '<button class="primary-button" type="button" data-sa-import>Confirm import</button>' : '') +
          '</section>' : '') +
      '</div></div></div>';
    host.setAttribute('aria-busy', String(busy));
  }
  async function refreshTicketDetails(): Promise<void> {
    const id = inspectedTicket;
    if (!id || !tickets.some(ticket => ticket.id === id)) {
      inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; return;
    }
    // Use existing native read endpoints, not model-generated summaries. A
    // revision change refreshes the snapshot before a later human decision.
    effectReceipts = []; effectReceiptsLoaded = false;
    runGraph = null; runGraphLoaded = false;
    const [snapshot, links] = await Promise.all([
      call<TicketDetails>('superagent_snapshot', { ticketId:id, limit:50 }),
      call<ThreadLink[]>('superagent_source_thread_links', { ticketId:id }),
    ]);
    if (inspectedTicket === id) {
      ticketDetails = snapshot;
      if (!snapshot.assignments.some(item => item.id === inspectedAssignmentId)) {
        inspectedAssignmentId = '';
      }
      threadLinks = links;
      linkedDecisions = []; linkedDecisionsLoaded = false;
      // Keep the ticket row and its grant/history pinned to one native revision.
      const index = tickets.findIndex(ticket => ticket.id === id);
      if (index >= 0) tickets[index] = snapshot.ticket;
    }
  }
  async function reload(): Promise<void> {
    loading = true;
    // Mutations invalidate the cross-Team index. Never show stale global
    // status as if the native Tickets had already been re-read.
    allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
    reviewQueue = []; reviewQueueLoaded = false;
    demandPreflight = null;
    // Keep updated controls hidden until the mutating operation has settled.
    // Exposing them mid-reload would allow silently discarded submissions.
    if (!busy) render();
    try {
      const [known, configs] = await Promise.all([call<Bot[]>('bots_list'), call<Config[]>('superagent_list')]);
      bots = known; presets = configs;
      if (demandTeamFilter && !presets.some(config => config.id === demandTeamFilter)) {
        demandTeamFilter = '';
      }
      if (!presets.some(config => config.id === selected)) {
        selected = presets[0]?.id ?? '';
        inspectedTicket = ''; threadLinks = [];
        linkedDecisions = []; linkedDecisionsLoaded = false; roleRoster = null; workerOrder = []; effectiveTeam = null;
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
        sourceIntegrity = null;
      }
      // A failed native read must not leave old ticket approval controls
      // visible underneath a newly selected or reloaded preset.
      tickets = [];
      ticketDetails = null; inspectedAssignmentId = '';
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      await refreshTicketDetails();
      if (roleRoster && selected) {
        roleRoster = await call<TeamRoster>('superagent_roster',{orchestrationId:selected});
        workerOrder = roleRoster.workerBotIds.slice(1);
      }
      // Any native refresh invalidates this non-authoritative, time-bound scan.
      effectiveTeam = null;
      if (sourcesLoaded && selected) {
        sources = await call<EventSource[]>('superagent_source_list',{ orchestrationId:selected });
        if (!sources.some(item => item.id === decisionSourceId)) {
          decisionSourceId = ''; decisions = [];
        } else {
          decisions = await call<SourceDecision[]>('superagent_source_decisions',
            {sourceId:decisionSourceId,limit:50});
        }
        // Native policies and cursors are mutable; never keep a prior
        // scan visible as current after a reload or a Team change.
        sourceIntegrity = null;
      }
      error = '';
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { loading = false; if (!busy) render(); }
  }
  async function work(action: () => Promise<void>): Promise<void> {
    if (busy) return;
    busy = true; error = ''; feedback = '';
    host.setAttribute('aria-busy', 'true');
    // Preserve any form fields while the native operation is in flight,
    // but prevent a second click from being silently discarded.
    host.querySelectorAll<HTMLButtonElement>('button').forEach(button => { button.disabled = true; });
    try { await action(); } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { busy = false; render(); }
  }
  function ask(message: string, action: () => Promise<void>): void {
    confirm = { message, action }; render();
    host.querySelector<HTMLButtonElement>('[data-sa-cancel]')?.focus();
  }
  // Templates populate editable names only; no bot, tool or task is selected.
  host.addEventListener('change', event => {
    const target = event.target;
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-worker-select')) {
      if (busy || !roleRoster || roleRoster.orchestrationId !== selected) return;
      const next = Array.from(target.selectedOptions, option => option.value);
      workerOrder = [...workerOrder.filter(id => next.includes(id)),
        ...next.filter(id => !workerOrder.includes(id))];
      render();
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-team-status')) {
      if (!['all','enabled','disabled'].includes(target.value)) return;
      teamStatusFilter = target.value as typeof teamStatusFilter;
      render();
      host.querySelector<HTMLSelectElement>('[data-sa-team-status]')?.focus({preventScroll:true});
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-demand-team-filter')) {
      if (busy) { target.value = demandTeamFilter; return; }
      const filter = target.value;
      if (filter && !presets.some(item => item.id === filter)) {
        target.value = demandTeamFilter; return;
      }
      demandTeamFilter = filter;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await call<DemandIndex[]>('superagent_all_demands',{
          limit:200, orchestrationId:filter || undefined,
          stateFilter:demandStateFilter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-demand-state-filter')) {
      if (busy) { target.value = demandStateFilter; return; }
      const filter = target.value;
      const allowed = ['queued','running','waiting_approval','waiting_review','blocked',
        'completed','failed','stopped','interrupted','inconclusive'];
      if (filter && !allowed.includes(filter)) {
        target.value = demandStateFilter; return;
      }
      demandStateFilter = filter;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await call<DemandIndex[]>('superagent_all_demands',{
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:filter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target instanceof HTMLSelectElement && target.name === 'mode' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      demandMode = target.value === 'direct' ? 'direct' : 'coordinated';
      demandPreflight = null;
      host.querySelector('[aria-label="Native demand preflight"]')?.remove();
      return;
    }
    if (!(target instanceof HTMLSelectElement) || !target.hasAttribute('data-sa-template')) return;
    draftPresetTemplate = target.value;
    draftPresetName = target.value;
    const field = target.form?.querySelector<HTMLInputElement>('input[name="name"]');
    if (field) field.value = draftPresetName;
  });
  host.addEventListener('input', event => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-team-search')) {
      teamSearch = target.value;
      const selection = target.selectionStart;
      render();
      const replacement = host.querySelector<HTMLInputElement>('[data-sa-team-search]');
      replacement?.focus({preventScroll:true});
      if (replacement && selection !== null) replacement.setSelectionRange(selection, selection);
      return;
    }
    if (target instanceof HTMLTextAreaElement && target.name === 'objective' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      demandObjective = target.value;
      demandPreflight = null;
      host.querySelector('[aria-label="Native demand preflight"]')?.remove();
    }
    if (target instanceof HTMLInputElement && target.name === 'name' &&
        target.closest<HTMLFormElement>('form[data-sa-form="create"]')) {
      draftPresetName = target.value;
    }
    if (target instanceof HTMLTextAreaElement && target.hasAttribute('data-sa-manifest')) {
      importText = target.value; preview = null; inspected = null;
    }
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-activity-filter')) {
      activityFilter = target.value;
      const selection = target.selectionStart;
      render();
      const replacement = host.querySelector<HTMLInputElement>('[data-sa-activity-filter]');
      replacement?.focus({preventScroll:true});
      if (replacement && selection !== null) replacement.setSelectionRange(selection, selection);
    }
  });
  host.addEventListener('click', event => {
    const target = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('button') : null;
    if (!target || !host.contains(target) || target.disabled) return;
    if (target.dataset.saWorkerMove) {
      if (busy || roleRoster?.orchestrationId !== selected || current()?.enabled) return;
      const index = workerOrder.indexOf(target.dataset.saWorkerMove);
      const offset = target.dataset.saDirection === 'up' ? -1 : 1;
      const next = index + offset;
      if (index < 0 || next < 0 || next >= workerOrder.length) return;
      [workerOrder[index], workerOrder[next]] = [workerOrder[next], workerOrder[index]];
      const movedId = target.dataset.saWorkerMove, direction = target.dataset.saDirection;
      render();
      Array.from(host.querySelectorAll<HTMLButtonElement>('[data-sa-worker-move]'))
        .find(button => button.dataset.saWorkerMove === movedId &&
          button.dataset.saDirection === direction)?.focus({preventScroll:true});
      return;
    }
    if (target.hasAttribute('data-sa-cancel')) { confirm = null; render(); return; }
    if (target.hasAttribute('data-sa-confirm')) {
      const action = confirm?.action; confirm = null; if (action) void work(action);
      return;
    }
    if (target.dataset.saUnlinkSource && target.dataset.saUnlinkThread) {
      const sourceId=target.dataset.saUnlinkSource;
      const subjectKey=target.dataset.saUnlinkThread;
      const ticketId=inspectedTicket;
      if (!ticketId || !ticketDetails || !threadLinks.some(link =>
          link.ticketId===ticketId && link.sourceId===sourceId &&
          link.subjectKey===subjectKey)) return;
      ask('Unlink this exact GitHub source thread? Stored authenticated receipts will remain; other explicit links may still show the same thread.', async () => {
        await call('superagent_source_unlink_thread',{sourceId,ticketId,subjectKey});
        await refreshTicketDetails();
        feedback='Source thread unlinked. Stored authenticated receipts were not deleted.';
      });
      return;
    }
    if (target.dataset.saActivityMode) {
      const mode = target.dataset.saActivityMode;
      if (!['all','compact','json'].includes(mode)) return;
      activityMode = mode as typeof activityMode;
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-activity-mode="' + mode + '"]')?.focus({preventScroll:true});
      return;
    }
    if (target.dataset.saGlobalDemandLayout) {
      const layout = target.dataset.saGlobalDemandLayout;
      if (layout !== 'board' && layout !== 'list') return;
      globalDemandLayout = layout;
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-global-demand-layout="' + layout + '"]')
        ?.focus({preventScroll:true});
      return;
    }
    if (target.dataset.saView) {
      const next = target.dataset.saView;
      if (!['teams','runs','review','sources','portable'].includes(next)) return;
      workspaceView = next as typeof workspaceView;
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-view="' + next + '"]')?.focus({preventScroll:true});
      return;
    }
    if (target.hasAttribute('data-sa-preflight')) {
      const form = host.querySelector<HTMLFormElement>('form[data-sa-form="run"]');
      const chosen = current();
      const objective = form?.querySelector<HTMLTextAreaElement>('textarea[name="objective"]')?.value.trim() ?? '';
      const coordinated = form?.querySelector<HTMLSelectElement>('select[name="mode"]')?.value !== 'direct';
      if (!chosen || !objective) return;
      demandPreflight = null;
      void work(async () => {
        demandPreflight = await call<ManualPreflight>('superagent_preflight', {
          orchestrationId: chosen.id, objective, coordinated,
        });
        demandObjective = objective;
        preflightObjective = objective;
        preflightMode = coordinated ? 'coordinated' : 'direct';
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-run-graph')) {
      const ticketId = inspectedTicket;
      if (ticketId && ticketDetails) void work(async () => {
        const graph = await call<RunGraph>('superagent_graph',{ticketId});
        if (inspectedTicket !== ticketId) return;
        if (graph.ticketRevision !== ticketDetails?.ticket.revision) {
          throw new Error('Native Ticket revision changed; refresh the Inspector before viewing its graph.');
        }
        runGraph = graph; runGraphLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-linked-activity')) {
      const ticketId = inspectedTicket;
      if (ticketId && ticketDetails) void work(async () => {
        const receipts = await call<SourceDecision[]>('superagent_source_thread_activity',
          {ticketId, limit:50});
        if (inspectedTicket === ticketId) {
          linkedDecisions = receipts; linkedDecisionsLoaded = true;
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-review-inbox')) {
      void work(async () => {
        reviewQueue = []; reviewQueueLoaded = false;
        reviewQueue = await call<DemandIndex[]>('superagent_review_inbox',{limit:200});
        reviewQueueLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-all-demands')) {
      void work(async () => {
        allDemands = [];
        allDemandsLoaded = false; allDemandsHasMore = false;
        const loaded = await call<DemandIndex[]>('superagent_all_demands',{
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
        });
        allDemands = loaded;
        allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-older-demands')) {
      const last = allDemands[allDemands.length - 1];
      if (!allDemandsLoaded || !allDemandsHasMore || !last) return;
      void work(async () => {
        const loaded = await call<DemandIndex[]>('superagent_all_demands',{
          limit:200, beforeUpdatedAt:last.updatedAt, beforeId:last.id,
          orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
        });
        const seen = new Set(allDemands.map(item => item.id));
        allDemands = [...allDemands, ...loaded.filter(item => !seen.has(item.id))];
        allDemandsHasMore = loaded.length === 200;
      });
      return;
    }
    if (target.dataset.saOpenDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saOpenDemand) ??
        reviewQueue.find(item => item.id === target.dataset.saOpenDemand);
      if (!demand || !presets.some(item => item.id === demand.orchestrationId)) return;
      selected = demand.orchestrationId;
      workspaceView = 'runs';
      tickets = []; inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; workerOrder = []; effectiveTeam = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null;
      void work(async () => {
        tickets = await call<Ticket[]>('superagent_tickets',{
          orchestrationId:demand.orchestrationId,limit:50,
        });
        // The global index can include an older Ticket outside the Team's
        // latest-50 page. Resolve it through the native immutable Ticket
        // identity and verify its Team before exposing any approval controls.
        if (!tickets.some(item => item.id === demand.id)) {
          const detail = await call<TicketDetails>('superagent_snapshot',{
            ticketId:demand.id,limit:50,
          });
          if (detail.ticket.orchestrationId !== demand.orchestrationId) {
            throw new Error('Native demand ownership changed; reload the index.');
          }
          tickets = [detail.ticket,...tickets];
        }
        inspectedTicket = demand.id;
        await refreshTicketDetails();
      });
      return;
    }
    if (target.hasAttribute('data-sa-role-load')) {
      if (selected) void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_roster',{orchestrationId:selected});
        workerOrder=roleRoster.workerBotIds.slice(1);
      });
      return;
    }
    if (target.hasAttribute('data-sa-effective-load')) {
      const cfg=current();
      if (cfg && roleRoster?.orchestrationId===cfg.id) void work(async () => {
        const snapshot=await call<EffectiveTeam>('superagent_effective_team',
          {orchestrationId:cfg.id});
        if (snapshot.orchestrationId!==cfg.id ||
            snapshot.policyRevision!==cfg.policyRevision) {
          effectiveTeam=null;
          throw new Error('Team policy changed during inspection; reload before using this view.');
        }
        effectiveTeam=snapshot;
      });
      return;
    }
    if (target.hasAttribute('data-sa-open-runs')) {
      workspaceView = 'runs'; render();
      host.querySelector<HTMLButtonElement>('[data-sa-view="runs"]')?.focus({preventScroll:true});
      return;
    }
    if (target.dataset.saSelect) {
      if (busy || loading) return;
      workspaceView = 'runs';
      if (selected === target.dataset.saSelect) { render(); return; }
      const next = target.dataset.saSelect;
      selected = next;
      tickets = [];
      inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; effectiveTeam = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null;
      void work(async () => {
        tickets = await call<Ticket[]>('superagent_tickets',{ orchestrationId:next, limit:50 });
      });
      return;
    }
    if (target.dataset.saEnable) {
      const c = presets.find(item => item.id === target.dataset.saEnable);
      if (c) ask((c.enabled ? 'Disable ' : 'Enable manual mode for ') + c.name + '?', async () => {
        await call('superagent_set_enabled',{ orchestrationId:c.id, expectedRevision:c.policyRevision, enabled:!c.enabled });
        await reload();
      });
      return;
    }
    if (target.dataset.saDelete) {
      const c = presets.find(item => item.id === target.dataset.saDelete);
      if (c) ask('Delete unused preset ' + c.name + '? Ticket history cannot be deleted.', async () => {
        await call('superagent_delete',{ orchestrationId:c.id, expectedRevision:c.policyRevision });
        selected = ''; await reload();
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-sources')) {
      if (selected) void work(async () => {
        sources = await call<EventSource[]>('superagent_source_list',{orchestrationId:selected});
        sourcesLoaded = true;
      });
      return;
    }
    if (target.dataset.saSourceMode) {
      const src = sources.find(item => item.id === target.dataset.saSourceMode);
      const mode = target.dataset.saNextMode;
      if (src && (mode === 'off' || mode === 'observe'))
        ask('Set this source to ' + mode + '? This never launches tasks.',async () => {
          await call('superagent_source_set_mode',
            {sourceId:src.id,expectedRevision:src.policyRevision,mode});
          await reload();
        });
      return;
    }
    if (target.dataset.saSourceCheckRun) {
      const selected = sources.find(item => item.id === target.dataset.saSourceCheckRun);
      const input = target.closest('li')?.querySelector<HTMLInputElement>('[data-sa-check-run-id]');
      const raw = input?.value.trim() || '';
      if (!selected || selected.mode !== 'observe' || selected.eventKind !== 'check_run.completed') return;
      const runId = Number(raw);
      if (!/^[1-9][0-9]{0,15}$/.test(raw) || !Number.isSafeInteger(runId)) {
        feedback = 'Enter a positive, bounded Check Run ID from the configured repository.';
        render();
        return;
      }
      void work(async () => {
        const receipt = await call<{decision:string;duplicate:boolean}>(
          'superagent_source_observe_check_run', {sourceId:selected.id,checkRunId:runId});
        feedback = 'Authenticated Checks API: ' + receipt.decision +
          (receipt.duplicate ? ' (duplicate)' : '') +
          '. Context receipt only; no task, approval or external effect started.';
        await reload();
      });
      return;
    }
    if (target.dataset.saSourcePoll) {
      const currentSource = sources.find(item => item.id === target.dataset.saSourcePoll);
      if (currentSource?.mode === 'observe') void work(async () => {
        const summary = await call<{
          scanned:number;recorded:number;duplicates:number;selfIgnored:number;pagesFetched:number
        }>('superagent_source_poll_authenticated', {sourceId:currentSource.id});
        feedback = 'Authenticated bounded poll: ' + summary.scanned + ' scanned across ' +
          summary.pagesFetched + ' page(s), ' + summary.recorded + ' recorded, ' +
          summary.selfIgnored + ' self-authored ignored, ' + summary.duplicates +
          ' duplicate. No tasks or effects started. This is not a gap-free subscription.';
        await reload();
      });
      return;
    }
    if (target.dataset.saSourceHealth) {
      const sourceId = target.dataset.saSourceHealth;
      if (sources.some(item => item.id === sourceId)) void work(async () => {
        const diagnosis = await call<SourceIntegrity>('superagent_source_integrity', {sourceId});
        if (sources.some(item => item.id === sourceId)) sourceIntegrity = diagnosis;
      });
      return;
    }
    if (target.dataset.saSourceDecisions) {
      const id = target.dataset.saSourceDecisions;
      if (sources.some(item => item.id === id)) void work(async () => {
        const loaded = await call<SourceDecision[]>('superagent_source_decisions',
          {sourceId:id,limit:50});
        decisionSourceId = id; decisions = loaded;
      });
      return;
    }
    if (target.hasAttribute('data-sa-refresh')) { void work(reload); return; }
    if (target.hasAttribute('data-sa-load-effect-receipts')) {
      const ticketId = inspectedTicket;
      if (!ticketId || !ticketDetails || ticketDetails.ticket.id !== ticketId) return;
      void work(async () => {
        const nativeReceipts = await call<EffectReceipt[]>('superagent_effect_receipts',
          {ticketId, limit:50});
        if (inspectedTicket !== ticketId) return;
        effectReceipts = nativeReceipts; effectReceiptsLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-close-details')) {
      inspectedTicket = ''; ticketDetails = null; inspectedAssignmentId = '';
      render(); return;
    }
    if (target.hasAttribute('data-sa-close-assignment')) {
      inspectedAssignmentId = ''; render(); return;
    }
    if (target.dataset.saGraphInspectAssignment) {
      const assignmentId = target.dataset.saGraphInspectAssignment;
      // Graph data is a projection, never permission. Only navigate to an
      // attempt confirmed by this Ticket's same-revision native snapshot.
      const snapshot = ticketDetails;
      const node = runGraph?.nodes.find(item =>
        item.kind === 'assignment' && item.id === 'assignment:' + assignmentId);
      const assignment = snapshot?.assignments.find(item => item.id === assignmentId);
      if (!runGraphLoaded || !snapshot || !assignment || !node || !runGraph ||
          runGraph.ticketId !== inspectedTicket ||
          runGraph.ticketRevision !== snapshot.ticket.revision ||
          snapshot.ticket.id !== inspectedTicket ||
          node.botId !== assignment.botId || node.attempt !== assignment.attempt) return;
      inspectedAssignmentId = assignmentId;
      render();
      host.querySelector<HTMLElement>('.sa-assignment-inspector')?.focus({preventScroll:true});
      return;
    }
    if (target.dataset.saInspectAssignment) {
      if (ticketDetails?.ticket.id !== inspectedTicket ||
          !ticketDetails.assignments.some(item => item.id === target.dataset.saInspectAssignment)) return;
      inspectedAssignmentId = inspectedAssignmentId === target.dataset.saInspectAssignment
        ? '' : target.dataset.saInspectAssignment;
      render();
      return;
    }
    if (target.dataset.saInspectTicket) {
      const id = target.dataset.saInspectTicket;
      if (inspectedTicket === id) {
        inspectedTicket = ''; ticketDetails = null; inspectedAssignmentId = '';
        render();
      } else {
        inspectedTicket = id; ticketDetails = null; inspectedAssignmentId = '';
        void work(refreshTicketDetails);
      }
      return;
    }
    if (target.hasAttribute('data-sa-export')) {
      void work(async () => {
        importText = JSON.stringify(await call<unknown>('superagent_manifest_export'),null,2);
        preview = null; inspected = null;
      }); return;
    }
    if (target.hasAttribute('data-sa-preview')) {
      void work(async () => {
        inspected = JSON.parse(importText);
        preview = await call<Preview>('superagent_manifest_preview',{ document:inspected });
      }); return;
    }
    if (target.hasAttribute('data-sa-import') && inspected) {
      const document = inspected;
      ask('Import these configurations, disabled, without changing existing bots?', async () => {
        const result = await call<ImportResult>('superagent_manifest_import',{ document, confirmed:true });
        preview = null; inspected = null; await reload();
        feedback = result.created.length + ' presets imported; ' + result.unchanged.length +
          ' unchanged.' + (result.backupPath ? ' Recovery snapshot: ' + result.backupPath : '');
      }); return;
    }
    if (target.dataset.saAccept) {
      const t = tickets.find(item => item.id === target.dataset.saAccept);
      if (t?.state === 'waiting_approval') ask('Accept the recorded worker evidence at ticket revision ' + t.revision + '?', async () => {
        await call('superagent_accept',{ ticketId:t.id, expectedRevision:t.revision }); await reload();
      });
      return;
    }
    if (target.dataset.saReject) { rejectingTicket = target.dataset.saReject; render(); return; }
    if (target.hasAttribute('data-sa-reject-cancel')) { rejectingTicket = ''; render(); return; }
    if (target.dataset.saDispatch) {
      const t = tickets.find(item => item.id === target.dataset.saDispatch);
      if (t?.state === 'queued') ask('Dispatch the worker using the original coordinator evidence?', async () => {
        await call('superagent_dispatch_worker',{ ticketId:t.id }); await reload();
      });
      return;
    }
    if (target.dataset.saRetry) {
      const t = tickets.find(item => item.id === target.dataset.saRetry);
      if (t?.state === 'blocked') ask('Retry the original mandate with unchanged permissions?', async () => {
        await call('superagent_retry',{ ticketId:t.id }); await reload();
      });
      return;
    }
    if (target.dataset.saStop) {
      const t = tickets.find(item => item.id === target.dataset.saStop);
      if (t) ask('Stop the task and revoke its authorization?', async () => {
        await call('superagent_stop',{ ticketId:t.id }); await reload();
      });
    }
  });
  host.addEventListener('submit', event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !host.contains(form)) return;
    event.preventDefault();
    const data = new FormData(form);
    if (form.dataset.saForm === 'roster') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id) return;
      const workerBotIds=[cfg.workerBotId,...workerOrder];
      const reviewerBotIds=data.getAll('reviewers').map(String);
      void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_roster_update',{
          orchestrationId:cfg.id, expectedRevision:cfg.policyRevision,
          input:{workerBotIds,reviewerBotIds},
        });
        await reload();
        workspaceView='teams';
      });
      return;
    }
    if (form.dataset.saForm === 'thread-link') {
      const ticketId = inspectedTicket;
      const sourceId = String(data.get('sourceId') ?? '');
      const subjectKey = String(data.get('subjectKey') ?? '').trim();
      if (!ticketId || !ticketDetails || !sources.some(src => src.id === sourceId) ||
          !/^(issue|pull):[1-9][0-9]{0,11}$/.test(subjectKey)) return;
      void work(async () => {
        await call('superagent_source_link_thread', {sourceId,ticketId,subjectKey});
        await refreshTicketDetails();
        feedback = 'Thread linked as context only; no task or authorization created.';
      });
      return;
    }
    if (form.dataset.saForm === 'issue-pull-link') {
      const ticketId=inspectedTicket;
      const selectedLink=String(data.get('issueLink') ?? '');
      const issueLink=threadLinks.find(link => link.ticketId===ticketId &&
        link.subjectKey.startsWith('issue:') &&
        link.sourceId+'|'+link.subjectKey===selectedLink);
      const pullKey=String(data.get('pullKey') ?? '').trim();
      if (!ticketId || !issueLink || !/^pull:[1-9][0-9]{0,11}$/.test(pullKey)) return;
      void work(async () => {
        await call('superagent_source_link_issue_pull',{
          sourceId:issueLink.sourceId,ticketId,issueKey:issueLink.subjectKey,pullKey,
        });
        await refreshTicketDetails();
        feedback='PR associated as operator context only; GitHub relationship is not verified.';
      });
      return;
    }
    if (form.dataset.saForm === 'source') {
      const orchestrationId = selected;
      const repository = String(data.get('repository') ?? '').trim();
      const actorFilter = String(data.get('actorFilter') ?? '').trim();
      const eventKind = String(data.get('eventKind') ?? '');
      if (!sourcesLoaded || !orchestrationId || !repository || !actorFilter) return;
      void work(async () => {
        await call('superagent_source_create',{orchestrationId,repository,actorFilter,eventKind});
        await reload();
      });
      return;
    }
    if (form.dataset.saForm === 'edit') {
      const existing = current();
      if (!existing || existing.enabled) return;
      const input = {
        name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? ''),
      };
      void work(async () => {
        await call('superagent_update',{ orchestrationId:existing.id, expectedRevision:existing.policyRevision, input });
        await reload();
      });
    }
    if (form.dataset.saForm === 'create') {
      const input = { name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? '') };
      const additionalWorkers=data.getAll('additionalWorkers').map(String);
      const reviewerBotIds=data.getAll('reviewers').map(String);
      void work(async () => {
        if (!input.name || !input.coordinatorBotId || !input.workerBotId)
          throw new Error('Choose a Team name, Coordinator and primary Worker.');
        if (additionalWorkers.length>7 || reviewerBotIds.length>4)
          throw new Error('Choose up to seven additional Workers and four Reviewers.');
        const ids=[input.coordinatorBotId,input.workerBotId,...additionalWorkers,...reviewerBotIds];
        if(new Set(ids).size!==ids.length)
          throw new Error('Each Team bot must have exactly one role.');
        const roster=additionalWorkers.length || reviewerBotIds.length
          ? {workerBotIds:[input.workerBotId,...additionalWorkers],reviewerBotIds}
          : undefined;
        const created = await call<Config>('superagent_create',{ input,roster });
        draftPresetName = ''; draftPresetTemplate = '';
        selected = created.id; workspaceView = 'runs'; await reload();
      });
    }
    if (form.dataset.saForm === 'reject') {
      const ticket = tickets.find(item => item.id === rejectingTicket);
      const reason = String(data.get('reason') ?? '').trim();
      if (ticket?.state !== 'waiting_approval' || !reason) return;
      void work(async () => {
        await call('superagent_reject',{ ticketId:ticket.id, expectedRevision:ticket.revision, reason });
        rejectingTicket = ''; await reload();
      });
    }
    if (form.dataset.saForm === 'run') {
      const selectedConfig = current(), objective = String(data.get('objective') ?? '').trim();
      const command = data.get('mode') === 'direct' ? 'superagent_submit' : 'superagent_submit_coordinated';
      if (!selectedConfig?.enabled || !objective) return;
      const key = taskKey;
      void work(async () => {
        await call(command,{ orchestrationId:selectedConfig.id, objective, requestKey:key });
        taskKey = crypto.randomUUID(); demandObjective = ''; demandPreflight = null;
        await reload();
      });
    }
  });
  render();
  void reload();
  return { refresh: reload };
}
