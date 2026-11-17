import './superagent.css';
import { escape } from './dom';
import { GuidedTeamBuilder } from './superagent-team-builder';

// This interface only invokes native policy/authorization decisions. It cannot
// grant tools, infer work, start tasks on import or bypass Council review.
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
interface Bot { id: string; name: string; paused: boolean;
  provider?: string; model?: string; mode?: string; projectId?: string; skills?: string[] }
interface Config {
  id: string; name: string; coordinatorBotId: string; workerBotId: string;
  enabled: boolean; policyRevision: number; councilRequired?:boolean;
  workerBotIds?:string[]; reviewerBotIds?:string[];
  ticketCount?: number; runningCount?: number; blockedCount?: number; lastActivityAt?: number;
  workerCount?: number; reviewerCount?: number;
}
interface StrategySettings {
  priority:string; attemptTimeoutMs:number; tokenCeiling?:number;
  estimatedCostCeilingMinor?:number; subagentDelegationEnabled:boolean;
}
interface StrategyView {
  orchestrationId:string; policyRevision:number; teamVersion:number;
  settings:StrategySettings; executionMode: 'off' | 'approval' | 'full_auto'; maxAttemptsPerAssignment:number; maxConcurrentTasks:number;
  executableParallelWorkers:number; tokenMeteringAvailable:boolean;
  pricedCostMeteringAvailable:boolean; nestedChildAdmissionAvailable:boolean;
  enableBlockers:string[];
}
interface TeamRoster { orchestrationId:string; policyRevision:number; coordinatorBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; councilRequired?:boolean; executionReady:boolean }
interface EffectiveSkill {
  id:string; libraryStatus:string; safetyStatus:string;
  harnessEligibility:string; invocationEvidence:string;
}
interface EffectiveMember {
  role:string; position:number; botId:string; name:string;
  provider:string; model:string; mode:string; reasoningEffort?:string; projectId:string;
  paused:boolean; skills:EffectiveSkill[]; configurationIssues?:string[];
}
interface EffectiveTeam {
  orchestrationId:string; policyRevision:number; enabled:boolean;
  executionReady:boolean; grantsIssued:boolean; members:EffectiveMember[];
  manualAdmissionBlockers?:string[];
}
interface TeamSnapshot {
  [key: string]: unknown;
  snapshotVersion?: unknown; provenance?: unknown; orchestrationId?: unknown;
  teamVersion?: unknown; name?: unknown; projectId?: unknown; coordinatorBotId?: unknown;
  workerBotIds?: unknown; reviewerBotIds?: unknown; policy?: unknown; roles?: unknown;
}
interface Ticket { id: string; orchestrationId: string; objective: string; state: string; revision: number; evidence: string | null }
interface WorkItemEvidence {
  id:string; key:string; ordinal:number; workerBotId:string; objective:string;
  dependsOn:string[]; state:string; childTicketId:string|null;
  childTicketState:string|null; lastReason:string|null;
  requiresWrite:boolean; writeAuthorized:boolean;
  evidenceSha256?:string|null; approvalKind?:string|null; evidenceVerifiedAt?:number|null;
}
interface DeliveryObservation {
  ticketId:string; ticketRevision:number; ticketState:string; deliveryState:string;
  totalWorkItems:number; verifiedWorkItems:number; unresolvedExternalEffects:number;
  appliedScopedFileWrites?:number; unresolvedScopedFileWrites?:number;
  repositoryHeadSha:string|null; worktreeStatusSha256:string|null;
  worktreeDiffSha256:string|null; observedStatusEntries:number|null;
  checkoutObservation:string; sourceAttribution:string;
  testExecutionVerified:boolean; ciResultVerified:boolean;
  externalDeliveryVerified:boolean; checkedAt:number; readOnly:boolean;
}
interface ObservedChildNode {
  id:string;parentId:string|null;rootAssignmentId:string;rootSessionId:string;
  rootWorkerBotId:string;name:string;reportedStatus:string;depth:number;
  missingParent:boolean;ancestryCycleOrOverflow:boolean;toolCount:number;
  startedAt:number;updatedAt:number;independentlyVerified:false;
  nativeChildGrantPresent:false;
}
interface ObservedChildTree {
  ticketId:string;teamVersion:number;configuredWorkers:number;councilReviewers:number;
  observedChildren:number;reportedRunning:number;reportedWaiting:number;
  reportedNested:number;hasUnresolvedLineage:boolean;truncated:boolean;
  canSpawnNativeChildren:false;providerStatusIsNotVerification:true;
  nodes:ObservedChildNode[];
}
interface DemandPlanEvidence {
  ticketId:string; ticketRevision:number; teamVersion:number; policyRevision:number;
  planSha256:string; state:string; items:WorkItemEvidence[];
}

/// Read-only interpretation of native dependency evidence. This is never
/// a scheduling decision, approval or substitute for server-side admission.
function workItemProgressHint(item:WorkItemEvidence, items:WorkItemEvidence[]):string {
  if(item.state==='completed') return item.evidenceSha256
    ? 'Verified native completion: exact Worker result and attributed acceptance recorded.'
    : 'Native approval receipt missing; predecessor cannot unlock dependencies.';
  if(item.requiresWrite && item.childTicketState==='completed' && !item.evidenceSha256) {
    return 'Write-required WorkItem needs an applied native file-write receipt from its final Worker attempt and a matching approval before dependencies can advance.';
  }
  if(item.state==='blocked') return 'Blocked: native recovery required.';
  if(item.state==='waiting_review') return 'Waiting for native review; dependencies cannot advance.';
  if(item.state==='running') return 'Native Worker assignment running.';
  const waiting=item.dependsOn.flatMap(key=>{
    const predecessor=items.find(candidate=>candidate.key===key);
    return predecessor?.state==='completed' ? [] :
      [key+' ('+(predecessor?.state??'unknown')+')'];
  });
  if(waiting.length) return 'Waiting for prerequisite Tickets: '+waiting.join(', ');
  if(item.requiresWrite && !item.writeAuthorized) {
    return 'Awaiting local authorization of exact file paths.';
  }
  return 'Pending native Worker admission and checkout availability checks.';
}
interface NativeSubagentPolicy {
  rootTicketId:string; teamVersion:number; policyRevision:number;
  maxDepth:number; maxChildren:number; maxTotal:number; revoked:boolean;
}
interface NativeSubagentNode {
  id:string; rootTicketId:string; parentTicketId:string;
  parentAssignmentId:string; childTicketId:string|null;
  key:string; workerBotId:string; depth:number; objective:string;
  state:string; reason:string|null;
}
interface NativeSubagentTree {
  rootTicketId:string; policy:NativeSubagentPolicy|null;
  nodes:NativeSubagentNode[];
}
interface DemandIndex { id: string; orchestrationId: string; objective: string;
  state: string; revision: number; updatedAt: number; projectId?: string }
interface DemandQuery {
  limit?: number; beforeUpdatedAt?: number; beforeId?: string;
  orchestrationId?: string; stateFilter?: string; searchQuery?: string;
  projectId?: string;
}
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
interface Activity { id: number; kind: string; detail: string; at: number }
interface CouncilObservation {
  nativeStatus: string; nativeRound: number; nativeStale: boolean;
  nativePassVerified: boolean; ticketArtifactBound: boolean;
}
interface TicketDetails {
  ticket: Ticket; mandate: Mandate; assignments: Assignment[];
  attemptInsights?: AssignmentInsight[];
  activity: Activity[]; teamSnapshot?: TeamSnapshot | null; council?: CouncilObservation | null;
  finalizationGate?: 'not_pending' | 'native_recheck_required' | 'council_required' | 'effects_unresolved';
  unresolvedExternalEffects?: number;
}
interface IssueCommentStatus {
  ticketId: string; ticketRevision: number; repository: string; issueNumber: number;
  effectKey: string; payloadSha256: string; state: 'pending' | 'uncertain' | 'applied';
  reconciliation: string;
}
interface PreviewIssueComment { previewOnly: true; message: string }
type IssueCommentResult = IssueCommentStatus | PreviewIssueComment;
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

interface AutoSourceStatus {
  sourceId:string; enabled:boolean; mode:string;
  repository:string; actorFilter:string; eventKind:string;
  maxDailyTasks:number|null; usedLastDay:number;
  expiresAt:number|null; operatorLogin:string|null;
  pending:number; blocked:number;
}

interface SourceIntegrity {
  sourceId: string; repository: string; mode: 'off' | 'observe';
  policyRevision: number; cursorKind: string; lastSequence: number;
  receiptCount: number; permittedReceiptCount: number;
  maxReceiptSequence: number; cursorConsistent: boolean;
  receiptCapacityReached: boolean; nextAction: string;
  observationalOnly: boolean;
}

async function issueCommentEffectKey(
  ticketId: string, repository: string, issueNumber: number, comment: string,
): Promise<string> {
  const payload = new TextEncoder().encode(JSON.stringify([ticketId, repository, issueNumber, comment]));
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', payload));
  return 'github-comment:' + Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}

export function mountSuperagent(host: HTMLElement, call: Call): { refresh(): Promise<void> } {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let allDemands: DemandIndex[] = [], allDemandsLoaded = false, allDemandsHasMore = false;
  let demandTeamFilter = '', demandStateFilter = '', demandSearch = '', demandSearchDraft = '';
  let demandProjectFilter = '', demandProjectDraft = '';
  let teamSearch = '', teamStatusFilter: 'all' | 'enabled' | 'disabled' = 'all';
  let reviewQueue: DemandIndex[] = [], reviewQueueLoaded = false;
  let globalDemandLayout: 'board' | 'list' = 'board';
  let selected = '', busy = false, loading = true, error = '', feedback = '', importText = '';
  let workspaceView: 'teams' | 'team' | 'runs' | 'new-demand' | 'run' | 'review' | 'sources' | 'portable' = 'teams';
  let teamPanel: 'overview' | 'members' | 'execution' = 'overview';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let commentRepository = '', commentIssueNumber = '', commentDraft = '';
  // Never reuse a potentially private GitHub comment draft in another Ticket.
  // Preserve fields across native refreshes of the *same* Ticket only.
  let commentDraftTicketId = '';
  let issueCommentError = '', issueCommentStatus: IssueCommentStatus | null = null;
  // Read-only draft PR target preflight; never a publication or durable permission.
  let draftPrRepository = '', draftPrHead = '', draftPrBase = '';
  let draftPrError = '', draftPrVerifiedSha = '', draftPrVerifiedRevision = -1;
  let demandObjective = '';
  let workPlan: DemandPlanEvidence | null = null, workPlanLoaded = false;
  let workWriteItemKey = '', workWritePaths = '';
  let subagentTree:NativeSubagentTree|null = null, subagentTreeLoaded = false;
  let subagentDepth=2, subagentPerParent=2, subagentTotal=8;
  let initialDemandWritePaths = '';
  const guidedBuilder = new GuidedTeamBuilder();
  let rejectingTicket = '', inspectedTicket = '';
  let inspectedAssignmentId = '';
  let activityMode: 'all' | 'compact' | 'json' = 'all';
  let activityFilter = '';
  let ticketDetails: TicketDetails | null = null;
  let effectReceipts: EffectReceipt[] = [], effectReceiptsLoaded = false;
  let runGraph: RunGraph | null = null, runGraphLoaded = false;
  let delivery:DeliveryObservation|null=null, deliveryLoaded=false;
  let providerChildren:ObservedChildTree|null=null, providerChildrenLoaded=false;
  let roleRoster: TeamRoster | null = null;
  let teamStrategy: StrategyView | null = null;
  // Draft order is committed only by the native revision-guarded roster update.
  let workerOrder: string[] = [];
  let reviewerDraft: string[] = [];
  let effectiveTeam: EffectiveTeam | null = null;
  let sources: EventSource[] = [], sourcesLoaded = false, decisionSourceId = '';
  let decisions: SourceDecision[] = [], threadLinks: ThreadLink[] = [];
  let sourceIntegrity: SourceIntegrity | null = null;
  let autoStatus: AutoSourceStatus | null = null, autoDailyLimit = 3;
  let linkedDecisions: SourceDecision[] = [], linkedDecisionsLoaded = false;
  const safe = (value: unknown) => escape(String(value ?? ''));
  const fetchDemandPage = (query: DemandQuery) =>
    call<DemandIndex[]>('superagent_all_demands', {
      query: {...query, ...(demandProjectFilter ? {projectId:demandProjectFilter} : {})},
    });
  function captureFocusedControl(): {
    selector: string; index: number; selectionStart: number | null;
    selectionEnd: number | null; selectionDirection: 'forward' | 'backward' | 'none' | null;
  } | null {
    const focused = document.activeElement;
    if (!(focused instanceof HTMLElement) || !host.contains(focused)) return null;
    const dataAttributes = Array.from(focused.attributes)
      .filter(attribute => attribute.name.startsWith('data-sa-'));
    const selector = focused.id
      ? '#' + CSS.escape(focused.id)
      : focused.tagName.toLowerCase() + dataAttributes
        .map(attribute => '[' + attribute.name + '="' + CSS.escape(attribute.value) + '"]').join('');
    if (!focused.id && !dataAttributes.length) return null;
    const index = Array.from(host.querySelectorAll<HTMLElement>(selector)).indexOf(focused);
    if (index < 0) return null;
    let selectionStart: number | null = null, selectionEnd: number | null = null;
    let selectionDirection: 'forward' | 'backward' | 'none' | null = null;
    if (focused instanceof HTMLInputElement || focused instanceof HTMLTextAreaElement) {
      try {
        selectionStart = focused.selectionStart;
        selectionEnd = focused.selectionEnd;
        selectionDirection = focused.selectionDirection;
      } catch { /* Some input types do not expose text selection. */ }
    }
    return { selector, index, selectionStart, selectionEnd, selectionDirection };
  }
  function restoreFocusedControl(snapshot: ReturnType<typeof captureFocusedControl>): void {
    if (!snapshot) return;
    const focused = host.querySelectorAll<HTMLElement>(snapshot.selector)[snapshot.index];
    if (!focused) return;
    focused.focus({ preventScroll:true });
    if (snapshot.selectionStart !== null && snapshot.selectionEnd !== null &&
        (focused instanceof HTMLInputElement || focused instanceof HTMLTextAreaElement)) {
      try { focused.setSelectionRange(snapshot.selectionStart, snapshot.selectionEnd, snapshot.selectionDirection ?? undefined); }
      catch { /* A changing control may no longer support a selection range. */ }
    }
  }
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
  function renderTeamSnapshot(snapshot: TeamSnapshot | null | undefined): string {
    if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot)) {
      return '<section class="sa-pane sa-team-snapshot" aria-label="Saved Team snapshot">' +
        '<h5>Saved Team snapshot unavailable</h5><p>The native Ticket response did not include <code>teamSnapshot</code>. The current Team configuration was not substituted.</p></section>';
    }
    const rawProvenance = typeof snapshot.provenance === 'string' ? snapshot.provenance : '';
    const provenance = rawProvenance || 'missing';
    const provenanceNote = rawProvenance === 'native'
      ? 'Native: this immutable Team snapshot was captured for this Ticket.'
      : rawProvenance === 'legacy-current'
        ? 'Legacy-current: backfilled from the mutable Team configuration observed during upgrade; it may not match the exact historical admission state.'
        : rawProvenance === 'legacy-unknown'
          ? 'Legacy-unknown: historical role/profile data was not recorded. Placeholder identities and missing profile evidence are not recovered facts or dispatch authority.'
          : 'Unrecognized or missing provenance. Treat historical Team details as unverified.';
    const roles = Array.isArray(snapshot.roles) ? snapshot.roles : [];
    const roleRows = roles.map((entry: unknown) => {
      const role = entry && typeof entry === 'object' && !Array.isArray(entry)
        ? entry as Record<string, unknown> : {};
      const roleName = typeof role.role === 'string' ? role.role : 'Unknown role';
      const ordinal = typeof role.ordinal === 'number' && Number.isFinite(role.ordinal)
        ? ' · ordinal ' + role.ordinal : '';
      const botId = typeof role.botId === 'string' ? role.botId : 'Unknown bot identity';
      const digest = typeof role.effectiveConfigSha256 === 'string' && role.effectiveConfigSha256
        ? ' · profile SHA-256 ' + safe(role.effectiveConfigSha256)
        : role.effectiveConfig === null ? ' · profile evidence unavailable' : '';
      return '<li><strong>' + safe(roleName) + '</strong>' + safe(ordinal) + ' · ' +
        safe(botName(botId)) + ' <code>' + safe(botId) + '</code>' + digest + '</li>';
    }).join('');
    let payload = 'Unable to serialize the returned snapshot.';
    try { payload = JSON.stringify(snapshot, null, 2) ?? 'null'; } catch { /* malformed preview only */ }
    const policy = snapshot.policy && typeof snapshot.policy === 'object' && !Array.isArray(snapshot.policy)
      ? snapshot.policy as Record<string, unknown> : null;
    return '<section class="sa-pane sa-team-snapshot" aria-label="Saved Team snapshot">' +
      '<h5>Saved Team snapshot</h5><p>Exact immutable <code>teamSnapshot</code> returned with this Ticket; the current Team is never substituted.</p>' +
      '<dl><dt>Provenance</dt><dd>' + safe(provenance) + '</dd>' +
      '<dt>Team / version</dt><dd>' + safe(snapshot.name ?? 'Unknown Team') + ' / ' +
      safe(snapshot.teamVersion ?? 'unknown') + ' · snapshot schema ' + safe(snapshot.snapshotVersion ?? 'unknown') + '</dd>' +
      (policy ? '<dt>Saved policy</dt><dd>Revision ' + safe(policy.policyRevision ?? 'unknown') +
        ' · Autonomy ' + safe(policy.autonomy ?? 'unknown') + ' · Council ' +
        safe(policy.councilEnabled ?? 'unknown') + '</dd>' : '') +
      '</dl><p role="note">' + safe(provenanceNote) + '</p>' +
      '<h6>Saved role order</h6><ol>' + (roleRows || '<li>No role entries were returned.</li>') +
      '</ol><details><summary>Inspect exact saved Team payload</summary><pre>' + safe(payload) +
      '</pre></details></section>';
  }
  function render(): void {
    if (!host.isConnected) return;
    const focusedControl = captureFocusedControl();
    // A different Ticket is a different confidentiality boundary. Native
    // refreshes can repaint the workspace without clearing the same draft.
    if (commentDraftTicketId !== inspectedTicket) {
      commentDraftTicketId = inspectedTicket;
      commentRepository = '';
      commentIssueNumber = '';
      commentDraft = '';
      issueCommentError = '';
      issueCommentStatus = null;
      draftPrRepository = ''; draftPrHead = ''; draftPrBase = '';
      draftPrError = ''; draftPrVerifiedSha = ''; draftPrVerifiedRevision = -1;
    }
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
      (item.enabled ? 'Enabled' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button></li>').join('');
    const optionsFor = (botId: string) => bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + (bot.id === botId ? ' selected' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const choices = optionsFor('');
    const chosen = current();
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const canComment = !!inspected && ticketDetails?.ticket.id === inspected.id &&
      inspected.state === 'running' && !ticketDetails.mandate.revoked &&
      ticketDetails.mandate.allowedSideEffects;
    const commentStatus = issueCommentStatus && inspected &&
      issueCommentStatus.ticketId === inspected.id &&
      issueCommentStatus.ticketRevision === inspected.revision ? issueCommentStatus : null;
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
        '</dd></dl><div class="sa-actions"><button type="button" class="sa-open-chat" data-sa-open-assignment-chat="' + safe(attempt.sessionId) + '">Open ' + safe(botName(attempt.botId)) + ' chat</button>' +
        '<button type="button" data-sa-close-assignment>Close attempt</button></div></section>'
      : '';
    const deliveryPanel = '<section class="sa-pane" role="region" aria-label="Engineering delivery evidence">' +
      '<h5>Engineering delivery evidence</h5>' +
      '<p>Read-only native state and bounded Git observations. Git HEAD and a Worker response do not prove tests, CI or a delivered PR.</p>' +
      '<button type="button" data-sa-inspect-delivery>Inspect native delivery</button>' +
      (deliveryLoaded && delivery?.ticketId===inspectedTicket
        ? '<dl><dt>Native delivery state</dt><dd>' + safe(delivery.deliveryState) + '</dd>' +
          '<dt>Verified WorkItems</dt><dd>' + safe(delivery.verifiedWorkItems) +
            ' / ' + safe(delivery.totalWorkItems) + '</dd>' +
          '<dt>Unresolved external effects</dt><dd>' + safe(delivery.unresolvedExternalEffects) + '</dd>' +
          '<dt>Applied native scoped file writes</dt><dd>' +
            safe(delivery.appliedScopedFileWrites ?? 'Unavailable') + '</dd>' +
          '<dt>Unresolved native scoped file writes</dt><dd>' +
            safe(delivery.unresolvedScopedFileWrites ?? 'Unavailable') + '</dd>' +
          '<dt>Git observation</dt><dd>' + safe(delivery.checkoutObservation) + '</dd>' +
          '<dt>Git HEAD (unattributed)</dt><dd><code>' +
            safe(delivery.repositoryHeadSha ?? 'unavailable') + '</code></dd>' +
          '<dt>Source attribution</dt><dd>' + safe(delivery.sourceAttribution) + '</dd>' +
          '<dt>Native tests / CI / publication</dt><dd>' +
            (delivery.testExecutionVerified ? 'Tests verified' : 'Tests unverified') + ' · ' +
            (delivery.ciResultVerified ? 'CI verified' : 'CI unverified') + ' · ' +
            (delivery.externalDeliveryVerified ? 'Publication verified' : 'Publication unverified') +
          '</dd></dl><p>Git digests are snapshots, not task-attributed commits or delivery approval.</p>'
        : '<p>Not inspected for this Ticket.</p>') + '</section>';
    const demandPlanPanel = '<section class="sa-pane" role="region" aria-label="Demand WorkItems">' +
      '<h5>Native dependency plan</h5><p>Each WorkItem has a separate native Ticket and review gate. ' +
      'No Coordinator text can issue write authorization.</p>' +
      '<button type="button" data-sa-load-work-plan>Inspect WorkItems</button>' +
      (workPlanLoaded && workPlan?.ticketId === inspectedTicket
        ? '<p role="status">Plan ' + safe(workPlan.state) +
          ' · Team version ' + safe(workPlan.teamVersion) +
          ' · SHA-256 ' + safe(workPlan.planSha256) + '</p>' +
          (workPlan.state === 'active' && tickets.some(ticket =>
            ticket.id === workPlan?.ticketId && ticket.state === 'queued') &&
            workPlan.items.filter(item => item.requiresWrite && !item.writeAuthorized &&
              item.state === 'pending' && !item.childTicketId).length > 1
            ? '<p>Authorize the SAME exact file paths for every pending write WorkItem ' +
              'with one native confirmation. Each child keeps a separate immutable grant; ' +
              'other files, shell and GitHub actions remain denied.</p>' +
              '<button type="button" data-sa-authorize-demand-work>' +
              'Authorize shared paths for pending WorkItems</button>' : '') +
          '<ol class="sa-tickets">' + workPlan.items.map(item =>
            '<li><strong>' + safe(item.key) + '</strong> · ' + safe(item.state) +
            ' · Worker ' + safe(botName(item.workerBotId)) +
            '<p>' + safe(item.objective) + '</p>' +
            '<small>Prerequisites: ' + safe(item.dependsOn.join(', ') || 'none') +
            ' · Child Ticket: ' + safe(item.childTicketId ?? 'not admitted') +
            ' · Native state: ' + safe(item.childTicketState ?? 'not started') +
            (item.lastReason ? ' · ' + safe(item.lastReason) : '') + '</small>' +
            (item.evidenceSha256
              ? '<small>Native ' + safe(item.approvalKind ?? 'approval') +
                ' · Evidence SHA-256 <code>' + safe(item.evidenceSha256) + '</code>' +
                ' · Reviewed ' + safe(activityTime(item.evidenceVerifiedAt ?? undefined)) + '</small>'
              : item.childTicketState === 'completed'
                ? '<small>Approval receipt unavailable; dependencies remain gated</small>' : '') +
            '<small class="sa-item-progress" aria-label="WorkItem progress">' +
              safe(workItemProgressHint(item, workPlan?.items ?? [])) + '</small>' +
            (item.requiresWrite && !item.writeAuthorized && item.state === 'pending'
              ? ' <button type="button" data-sa-authorize-work="' + safe(item.key) +
                '">Authorize exact file paths</button>' : '') +
            (item.requiresWrite && item.writeAuthorized
              ? ' <small>Exact file scope confirmed locally</small>' : '') +
            (item.childTicketId
              ? ' <button type="button" data-sa-open-workitem-chat="' +
                safe(item.childTicketId) + '">Open Worker chat</button>'
              : '') +
            '</li>').join('') + '</ol>' +
          ((workWriteItemKey === '*' && workPlan.items.filter(item =>
            item.requiresWrite && item.state === 'pending' &&
            !item.writeAuthorized && !item.childTicketId).length > 1) ||
            (workWriteItemKey !== '*' && workPlan.items.some(item =>
              item.key === workWriteItemKey && item.requiresWrite &&
              item.state === 'pending' && !item.writeAuthorized))
            ? '<form data-sa-form="work-write-scope" aria-label="WorkItem file authorization">' +
              '<p>Native local-user confirmation required for ' +
              (workWriteItemKey === '*'
                ? workPlan.items.filter(item => item.requiresWrite && !item.writeAuthorized &&
                    item.state === 'pending' && !item.childTicketId).length +
                  ' pending WorkItems. The same exact file paths will be granted to each.'
                : safe(workWriteItemKey) + '.') +
              ' Other tool and GitHub rights remain denied.</p>' +
              '<label>Exact project-relative paths, one per line' +
              '<textarea name="allowedPaths" rows="4" spellcheck="false" required>' +
              safe(workWritePaths) + '</textarea></label>' +
              '<button type="submit" class="primary-button">Review native authorization</button>' +
              '<button type="button" data-sa-work-write-cancel>Cancel</button></form>' : '')
        : workPlanLoaded ? '<p>No native WorkItem plan belongs to this Ticket.</p>'
          : '<p>Plan not loaded.</p>') +
      '</section>';
    const effectiveWorkerIds=ticketDetails?.teamSnapshot?.workerBotIds;
    const isRootDemand=Boolean(ticketDetails?.activity.some(entry =>
      entry.kind==='demand_auto_plan_requested') &&
      Array.isArray(effectiveWorkerIds) && effectiveWorkerIds.length>=2 &&
      ticketDetails && ['running','queued'].includes(ticketDetails.ticket.state));
    const providerChildPanel = '<section class="sa-pane" role="region" aria-label="Provider subagent hierarchy">' +
      '<h5>Provider-reported subagents</h5>' +
      '<p>Actual recorded provider child events. These are not independent native Workers, Council seats or tool permissions.</p>' +
      '<button type="button" data-sa-load-provider-children>Inspect actual provider children</button>' +
      (providerChildrenLoaded && providerChildren?.ticketId===inspectedTicket
        ? '<p>Configured Workers ' + safe(providerChildren.configuredWorkers) +
          ' · Council Reviewers ' + safe(providerChildren.councilReviewers) +
          ' · Observed children ' + safe(providerChildren.observedChildren) +
          ' · Running ' + safe(providerChildren.reportedRunning) +
          ' · Waiting ' + safe(providerChildren.reportedWaiting) +
          ' · Nested ' + safe(providerChildren.reportedNested) + '</p>' +
          (providerChildren.hasUnresolvedLineage
            ? '<p role="status">Provider ancestry is missing or cyclic. No native authority inferred.</p>' : '') +
          (providerChildren.truncated ? '<p>Provider observation capped at 256 children.</p>' : '') +
          '<ol aria-label="Observed provider subagents">' +
          (providerChildren.nodes.map(child=>'<li>' +
            '<strong>' + safe(child.name) + '</strong>' +
            ' · Provider state ' + safe(child.reportedStatus) +
            ' · Parent ' + safe(child.parentId ?? 'root') +
            ' · Depth ' + safe(child.depth) +
            ' · Root Worker ' + safe(botName(child.rootWorkerBotId)) +
            ' · Reported tools ' + safe(child.toolCount) +
            (child.missingParent ? ' · Missing parent' : '') +
            (child.ancestryCycleOrOverflow ? ' · Invalid ancestry' : '') +
            ' · Native grant: none · Independent verification: none</li>').join('') ||
            '<li>No provider subagents observed.</li>') + '</ol>'
        : '<p>Provider child events not loaded.</p>') + '</section>';
    const subagentPanel = '<section class="sa-pane" role="region" aria-label="Native Worker subagents">' +
      '<h5>Worker-created subagents</h5><p>Optional, finite read-only native child delegation. ' +
      'A local user must authorize the exact root Demand and child limits. ' +
      'Children retain independent Tickets and cannot inherit file, shell, MCP or GitHub rights.</p>' +
      '<button type="button" data-sa-load-subagents>Inspect native descendant tree</button>' +
      (subagentTreeLoaded && subagentTree?.rootTicketId===inspectedTicket
        ? '<p role="status">' +
          (subagentTree.policy
            ? 'Enabled · depth ≤ ' + subagentTree.policy.maxDepth +
              ' · children per Worker ≤ ' + subagentTree.policy.maxChildren +
              ' · total descendants ≤ ' + subagentTree.policy.maxTotal +
              (subagentTree.policy.revoked ? ' · revoked' : '')
            : 'No native descendant policy has been issued') + '</p>' +
          '<ol class="sa-tickets">' +
          subagentTree.nodes.map(node=>
            '<li><strong>' + safe(node.key) + '</strong> · depth ' + node.depth +
            ' · ' + safe(node.state) +
            '<p>' + safe(node.objective) + '</p>' +
            '<small>Parent Ticket ' + safe(node.parentTicketId) +
            ' · Worker ' + safe(botName(node.workerBotId)) +
            ' · Child Ticket ' + safe(node.childTicketId ?? 'not admitted') +
            (node.reason ? ' · ' + safe(node.reason) : '') +
            '</small></li>').join('') +
          '</ol>' +
          (!subagentTree.policy && isRootDemand
            ? '<form data-sa-form="enable-subagents">' +
              '<p>Only the native local confirmation dialog can authorize a bounded descendant policy.</p>' +
              '<label>Maximum nesting depth (1–3)<input name="maxDepth" type="number" min="1" max="3" value="' +
              safe(subagentDepth) + '" required></label>' +
              '<label>Maximum children per Worker (1–4)<input name="maxChildren" type="number" min="1" max="4" value="' +
              safe(subagentPerParent) + '" required></label>' +
              '<label>Maximum total descendants (1–24)<input name="maxTotal" type="number" min="1" max="24" value="' +
              safe(subagentTotal) + '" required></label>' +
              '<button type="submit" class="primary-button">Review bounded subagent authorization</button>' +
              '</form>' : '')
        : subagentTreeLoaded ? '<p>No native descendants are associated with this Ticket.</p>'
          : '<p>Descendant tree not loaded.</p>') +
      '</section>';
    const details = inspected && ticketDetails
      ? '<section class="sa-pane" role="region" aria-label="Ticket evidence">' +
        '<h4>Ticket evidence</h4><p>Read-only native snapshot. Model output is not task approval.</p>' +
        renderTeamSnapshot(ticketDetails.teamSnapshot) +
        deliveryPanel + demandPlanPanel + subagentPanel + providerChildPanel +
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
            '. Council evidence is authoritative only for an exact Ticket, artifact and verified native cycle; unbound or stale results cannot approve a task. Such unverified evidence cannot authorize SuperAgent completion.</p></section>'
          : '') +
        (canComment
          ? '<section class="sa-effect-editor" aria-label="Explicit GitHub effect"><h5>GitHub issue comment</h5>' +
            '<p>Available only for this running Ticket’s active side-effect mandate. The native confirmation shows the exact target and comment.</p>' +
            '<form data-sa-form="issue-comment"><div class="sa-grid">' +
            '<label>Repository (owner/name)<input name="repository" maxlength="200" required value="' +
            safe(commentRepository) + '"></label>' +
            '<label>Issue number<input name="issueNumber" type="number" min="1" step="1" inputmode="numeric" required value="' +
            safe(commentIssueNumber) + '"></label></div>' +
            '<label>Comment<textarea name="comment" rows="4" maxlength="60000" required>' +
            safe(commentDraft) + '</textarea></label>' +
            (issueCommentError ? '<p role="alert" data-sa-comment-error class="sa-write-path-error">' + safe(issueCommentError) + '</p>' : '') +
            (commentStatus ? '<p role="status" data-sa-comment-status>Effect ' + safe(commentStatus.state) +
              ' · ' + safe(commentStatus.reconciliation) + ' · SHA-256 ' +
              safe(commentStatus.payloadSha256) + '</p>' : '') +
            '<button type="submit" class="primary-button">' +
            (commentStatus?.state === 'applied' ? 'Check existing comment' :
              commentStatus?.state === 'pending' ? 'Check pending comment' :
                commentStatus?.state === 'uncertain' ? 'Reconcile comment status' :
                  'Authorize and post comment') + '</button></form></section>'
          : '') +
        (canComment
          ? '<section class="sa-effect-editor" role="region" aria-label="Draft PR target preflight">' +
            '<h5>Draft pull request target (read-only)</h5>' +
            '<p>Verify the active Worker checkout is clean, its head is already pushed, and the base exists in the same repository. No pull request is created and no permission is granted.</p>' +
            '<form data-sa-form="draft-pr-preflight"><label>GitHub repository (owner/name)' +
            '<input name="repository" maxlength="200" required value="' + safe(draftPrRepository) + '"></label>' +
            '<div class="sa-grid"><label>Pushed head branch<input name="head" maxlength="200" required value="' +
            safe(draftPrHead) + '"></label><label>Base branch<input name="base" maxlength="200" required value="' +
            safe(draftPrBase) + '"></label></div>' +
            (draftPrError ? '<p role="alert" data-sa-draft-pr-error>' + safe(draftPrError) + '</p>' : '') +
            (draftPrVerifiedSha && draftPrVerifiedRevision === inspected.revision
              ? '<p role="status" data-sa-draft-pr-sha>Verified remote head SHA: <code>' +
                safe(draftPrVerifiedSha) + '</code>. Read-only verification only; creating a PR remains unavailable.</p>' : '') +
            '<button type="submit">Verify pushed branch (read-only)</button></form></section>'
          : '') +
        '<button type="button" data-sa-close-details>Close evidence</button></section>'
      : '';
    const sourceRows = sources.map(src => {
      const activeAuto = autoStatus?.sourceId === src.id ? autoStatus : null;
      return '<li><div><strong>' + safe(src.repository) + '</strong><small>' +
      safe(src.eventKind) + ' · ' + safe(src.actorFilter) + ' · Cursor ' +
      src.lastSequence + ' · Revision ' + src.policyRevision + '</small></div>' +
      '<span>' + safe(activeAuto?.mode ?? src.mode) + '</span>' +
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
      '">Inspect source health</button>' +
      (src.mode === 'observe' && src.eventKind === 'issues.opened'
        ? '<button type="button" data-sa-auto-status="' + safe(src.id) +
          '">Inspect read-only Auto policy</button>' : '') +
      '</li>';
    }).join('');
    const sourcePanel = '<section class="sa-event-sources" aria-label="GitHub event sources">' +
      '<h4>GitHub event sources</h4><p>Authenticated GitHub Events polling is bounded and read-only in Observe mode. Off and authenticated Observe remain available. Read-only Auto for exact issues.opened events requires separate local native confirmation and expires within 24 hours. A source grant never authorizes files, GitHub comments, pushes, merges, or releases.</p>' +
      (!sourcesLoaded ? '<button type="button" data-sa-load-sources>Load sources</button>'
        : '<form data-sa-form="source"><div class="sa-grid">' +
          '<label>Repository (owner/name)<input name="repository" maxlength="200" required></label>' +
          '<label>Exact actor<input name="actorFilter" maxlength="80" required></label></div>' +
          '<label>Event class<select name="eventKind">' +
          ['issues.opened','issues.edited','issues.closed','issues.reopened',
            'issues.assigned','issues.unassigned','issues.labeled','issues.unlabeled',
            'issue_comment.created','pull_request.opened',
            'pull_request.closed','pull_request.reopened','pull_request.merged',
            'pull_request.assigned','pull_request.unassigned',
            'pull_request.labeled','pull_request.unlabeled',
            'pull_request_review.submitted',
            'pull_request_review.updated','pull_request_review.dismissed',
            'pull_request_review_comment.created','check_run.completed']
            .map(kind => '<option value="' + kind + '">' + kind + '</option>').join('') +
          '</select></label><button type="submit">Add source (Off)</button></form>' +
          '<ul class="sa-tickets sa-source-list">' +
          (sourceRows || '<li class="sa-empty">No event sources configured.</li>') + '</ul>' +
          (autoStatus && sources.some(src => src.id === autoStatus?.sourceId)
            ? '<section class="sa-pane" role="region" aria-label="Authenticated GitHub Auto policy">' +
              '<h5>Read-only GitHub Auto grant</h5><p>' +
              safe(autoStatus.repository) + ' · ' + safe(autoStatus.eventKind) +
              ' · Actor ' + safe(autoStatus.actorFilter) +
              ' · Mode ' + safe(autoStatus.mode) +
              '</p><p>Intake in rolling day: ' + autoStatus.usedLastDay +
              ' · Pending: ' + autoStatus.pending + ' · Blocked: ' + autoStatus.blocked +
              ' · Expires: ' + safe(autoStatus.expiresAt
                ? activityTime(autoStatus.expiresAt) : 'Not authorized') + '</p>' +
              (autoStatus.enabled
                ? '<button type="button" data-sa-auto-revoke="' + safe(autoStatus.sourceId) +
                  '">Revoke Auto and stop associated native Tickets</button>'
                : '<form data-sa-form="auto-authorization">' +
                  '<p>Explicit local-user confirmation authorizes one repository, exact actor, issues.opened, read-only work, and a finite budget. Issue text never grants tools.</p>' +
                  '<label>Maximum new Demands per rolling 24h' +
                  '<input name="maxDailyTasks" type="number" min="1" max="20" required value="' +
                  safe(autoDailyLimit) + '"></label>' +
                  '<button type="submit" class="primary-button">Review read-only Auto grant</button>' +
                  '</form>') +
              '</section>' : '') +
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
      demand.revision + ' · Admitted project ' + safe(demand.projectId ?? 'Unknown') +
      ' · Updated ' + safe(activityTime(demand.updatedAt)) +
      '</small></div><button type="button" data-sa-open-demand="' + safe(demand.id) +
      '">Open demand</button>' +
      (!['completed','failed','stopped','interrupted','inconclusive'].includes(demand.state)
        ? '<button type="button" data-sa-stop-global-demand="' + safe(demand.id) +
          '">Stop demand</button>' : '') + '</li>';
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
      '<form data-sa-form="demand-project-filter" role="search" aria-label="Filter native Demands by admitted project">' +
      '<label>Admitted project ID<input type="text" name="projectId" maxlength="128" ' +
      'aria-label="Exact admitted Project ID" data-sa-demand-project-input ' +
      'value="' + safe(demandProjectDraft) + '"></label>' +
      '<button type="submit">Filter Project</button></form>' +
      '<label class="sa-demand-state-filter">Filter by State<select aria-label="Filter Demands by State" data-sa-demand-state-filter>' +
      '<option value="">All States</option>' +
      (['queued','running','waiting_approval','waiting_review','blocked','completed',
        'failed','stopped','interrupted','inconclusive'] as const)
        .map(state => '<option value="' + state + '"' +
          (demandStateFilter === state ? ' selected' : '') + '>' +
          state.split('_').join(' ') + '</option>').join('') +
      '</select></label>' +
      '<form data-sa-form="demand-search" role="search" aria-label="Search native Demands">' +
      '<label>Search objectives<input type="search" name="searchQuery" aria-label="Search Demands by objective" data-sa-demand-search maxlength="128" ' +
      'placeholder="Literal objective fragment" value="' + safe(demandSearchDraft) + '"></label>' +
      '<button type="submit">Search Demands</button></form>' +
      '<p>Native Tickets ' + (demandTeamFilter ? 'for this Team' : 'across Teams') +
      (demandProjectFilter ? ' admitted to project ' + safe(demandProjectFilter) : '') +
      (demandStateFilter ? ' in state ' + safe(demandStateFilter) : '') +
      (demandSearch ? ' matching objective "' + safe(demandSearch) + '"' : '') +
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
    const teamEditor = chosen ? '<section class="sa-pane" aria-label="Team identity"><h3>Team identity</h3>' +
      '<form data-sa-form="edit"><label>Team name<input name="name" required maxlength="120" value="' + safe(chosen.name) + '"></label>' +
      '<div class="sa-grid"><label>Coordinator<select name="coordinatorBotId" required>' +
      optionsFor(chosen.coordinatorBotId) + '</select></label>' +
      '<label>Primary Worker<select name="workerBotId" required>' +
      optionsFor(chosen.workerBotId) + '</select></label></div>' +
      '<button class="primary-button" type="submit">Save Team identity</button></form>' +
      '<p>Identity changes apply to future Demands only. Existing native bot sessions and approvals remain attached to their original Tickets.</p>' +
      '<div class="sa-actions"><button type="button" data-sa-duplicate="' + safe(chosen.id) +
      '">Duplicate Team</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused Team</button></div></section>' : '';
    const admission = chosen ? '<section class="sa-pane" aria-label="Native Demand and WorkItems">' +
      '<header><h3>New Demand · ' + safe(chosen.name) + '</h3>' +
      '<button type="button" data-sa-return-team>Team settings</button></header>' +
      '<label>Team<select data-sa-admission-team aria-label="Team for Demand">' +
      presets.map(team => '<option value="' + safe(team.id) + '"' +
        (team.id === chosen.id ? ' selected' : '') + '>' +
        safe(team.name) + (team.enabled ? '' : ' (Off)') + '</option>').join('') +
      '</select></label>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required>' +
      safe(demandObjective) + '</textarea></label>' +
      '<p class="sa-execution-explainer"><strong>Native Team orchestration</strong> · ' +
      (teamStrategy?.orchestrationId === chosen.id
        ? (teamStrategy.executionMode === 'full_auto' ? 'Full Auto · native Council'
          : teamStrategy.executionMode === 'approval' ? 'With Approval · human decision'
          : 'Execution policy not configured')
        : 'Inspect the Team execution policy before starting') +
      '. The Coordinator plans dependencies and the scheduler dispatches them to existing bot conversations.</p>' +
      '<label>Optional exact files for this Demand (one project-relative file per line)' +
        '<textarea name="initialAllowedPaths" rows="3" autocomplete="off" ' +
        'placeholder="src/feature.ts">' + safe(initialDemandWritePaths) + '</textarea></label>' +
        '<p>Native local confirmation occurs once before Coordinator planning. ' +
        'Every eligible write-required WorkItem receives the SAME exact paths ' +
        'as an immutable child grant. Leave blank for read-only planning and ' +
        'later per-WorkItem authorization. Shell, MCP and GitHub writes stay denied.</p>' +
      '<button class="primary-button" type="submit"' +
      (chosen.enabled && teamStrategy?.orchestrationId === chosen.id &&
        teamStrategy.executionMode !== 'off' ? '' : ' disabled') +
      '>Start Demand</button></form>' +
      '<p class="sa-mode-note">Native admission checks the Team policy, bot eligibility and every scoped permission before dispatch.</p></section>' : '';
    const active = chosen && inspectedTicket ? '<section class="sa-pane sa-run-detail" aria-label="Selected native Demand">' +
      '<div class="sa-run-head"><button type="button" data-sa-back-runs>All Demands</button>' +
      '<p class="sa-eyebrow">NATIVE DEMAND / RUN</p><h2>' +
        safe(tickets.find(ticket => ticket.id === inspectedTicket)?.objective ?? 'Loading native Demand') + '</h2>' +
      '<p>Team ' + safe(chosen.name) + ' · ' + safe(tickets.find(ticket => ticket.id === inspectedTicket)?.state ?? 'Loading') +
        ' · Ticket ' + safe(inspectedTicket) + '</p></div>' +
      '<ol class="sa-run-milestones" aria-label="Native Run lifecycle">' +
        '<li><span>01 · Admission</span><strong>' +
          (ticketDetails?.ticket.id === inspectedTicket ? 'Native Ticket recorded' : 'Loading native record') +
          '</strong></li>' +
        '<li><span>02 · Assignment</span><strong>' +
          (ticketDetails?.ticket.id === inspectedTicket
            ? ticketDetails.assignments.length + ' recorded attempts' : 'Not inspected') +
          '</strong></li>' +
        '<li><span>03 · WorkItems</span><strong>' +
          (workPlanLoaded && workPlan?.ticketId === inspectedTicket
            ? workPlan.items.length + ' native WorkItems' : 'Not inspected') +
          '</strong></li>' +
        '<li><span>04 · Review</span><strong>' +
          (ticketDetails?.ticket.id === inspectedTicket
            ? ticketDetails.finalizationGate === 'council_required' ? 'Council required' :
              ticketDetails.ticket.state === 'waiting_approval' ? 'Human approval pending' :
              ticketDetails.ticket.state === 'waiting_review' ? 'Native review pending' :
              'No verified verdict shown'
            : 'Not inspected') +
          '</strong></li>' +
        '<li><span>05 · Delivery</span><strong>' +
          (deliveryLoaded && delivery?.ticketId === inspectedTicket
            ? safe(delivery.deliveryState) : 'Not inspected') +
          '</strong></li></ol>' +
      '<div class="sa-title"><h4>Native Ticket</h4><button type="button" data-sa-refresh>Refresh</button></div>' +
      '<ul class="sa-tickets">' + (tickets.filter(ticket => ticket.id === inspectedTicket).map(ticket =>
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
        (ticket.state === 'blocked' ? '<button type="button" data-sa-retry="' +
          safe(ticket.id) + '">Retry blocked Demand</button>' : '') +
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
        (effectiveTeam.enabled ? 'Team enabled' : 'Team Off') + ' · ' +
        (effectiveTeam.executionReady ? 'Native role checks passed' : 'Native role checks require attention') +
        ' · No task grant issued</p>' +
        (effectiveTeam.manualAdmissionBlockers
          ? (effectiveTeam.manualAdmissionBlockers.length
            ? '<p>Native admission blocked:</p><ul aria-label="Native admission blockers">' +
              effectiveTeam.manualAdmissionBlockers.map(reason => '<li>' + safe(reason) + '</li>').join('') + '</ul>'
            : '<p>Native role preflight passed. Provider tools, Skills, grants and runtime capacity remain unverified.</p>')
          : '<p>Native admission preflight was not returned; execution readiness is unverified.</p>') +
        '<ol>' + effectiveTeam.members.map(member =>
          '<li><strong>' + safe(member.role) + ' ' + (member.position + 1) +
          ': ' + safe(member.name) + '</strong>' +
          ' · Harness ' + safe(member.provider) + ' · Model ' + safe(member.model) +
          ' · Bot mode ' + safe(member.mode) +
          ' · Reasoning effort ' + safe(member.reasoningEffort || 'not specified') +
          ' · Project ' + safe(member.projectId) +
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
    const strategyPane = chosen
      ? '<section class="sa-pane" aria-label="Team runtime strategy">' +
        '<header><h3>Resource strategy and native capability</h3>' +
        '<button type="button" data-sa-strategy-load>Inspect native policy</button></header>' +
        (!teamStrategy || teamStrategy.orchestrationId !== chosen.id ||
          teamStrategy.policyRevision !== chosen.policyRevision
          ? '<p>Load the immutable Team policy to see cost, latency, attempts and executable limits.</p>'
          : '<p>Team version ' + teamStrategy.teamVersion +
            ' · Native executable parallel Workers: ' + teamStrategy.executableParallelWorkers +
            ' · Real token metering: ' + (teamStrategy.tokenMeteringAvailable ? 'Available' : 'Not supported') +
            ' · Priced cost metering: ' + (teamStrategy.pricedCostMeteringAvailable ? 'Available' : 'Not supported') +
            ' · Nested delegation: ' + (teamStrategy.nestedChildAdmissionAvailable ? 'Available' : 'Not supported') + '</p>' +
            (teamStrategy.enableBlockers.length
              ? '<p role="status">Enable blockers: ' + safe(teamStrategy.enableBlockers.join('; ')) + '</p>' : '') +
            '<form data-sa-form="strategy-update">' +
            '<fieldset class="sa-mode-choices"><legend>Execution mode</legend>' +
            '<label class="sa-mode-option"><input type="radio" name="executionMode" value="approval"' +
              (teamStrategy.executionMode === 'approval' ? ' checked' : '') +
              '><span><strong>With Approval</strong><small>WorkItems pause for native human acceptance. Council automatic finalization must be disabled.</small></span></label>' +
            '<label class="sa-mode-option"><input type="radio" name="executionMode" value="full_auto"' +
              (teamStrategy.executionMode === 'full_auto' ? ' checked' : '') +
              '><span><strong>Full Auto</strong><small>Native dependency dispatch with two or more Council reviewers; only a verified pass settles each WorkItem.</small></span></label>' +
            '</fieldset>' +
            '<p class="sa-mode-note">The Team must be Off to change modes. Full Auto requires at least two independent native Council reviewers; With Approval waits for a human decision. Neither mode grants additional tools or file/GitHub rights.</p>' +
            (roleRoster?.orchestrationId === chosen.id && roleRoster.reviewerBotIds.length < 2
              ? '<div class="sa-mode-requirement" role="note">Full Auto needs two independent reviewers. ' +
                '<button type="button" data-sa-edit-reviewers>Configure reviewers</button></div>'
              : '') +
            '<label>Optimization priority<select name="priority">' +
            (['balanced','cost','latency','throughput','quality'] as const).map(priority =>
              '<option value="' + priority + '"' +
              (teamStrategy?.settings.priority === priority ? ' selected' : '') + '>' +
              priority + '</option>').join('') + '</select></label>' +
            '<label>Maximum concurrent tasks (1–8)<input type="number" name="maxConcurrentTasks" min="1" max="8" required value="' +
              teamStrategy.maxConcurrentTasks + '"></label>' +
            '<label>Maximum attempts per Worker (1–8)<input type="number" name="maxAttemptsPerAssignment" min="1" max="8" required value="' +
              teamStrategy.maxAttemptsPerAssignment + '"></label>' +
            '<label>Attempt timeout (milliseconds, 60000–1800000)<input type="number" name="attemptTimeoutMs" min="60000" max="1800000" required value="' +
              teamStrategy.settings.attemptTimeoutMs + '"></label>' +
            '<label>Token ceiling (optional; requires native metering)<input type="number" name="tokenCeiling" min="1" placeholder="Not configured" value="' +
              safe(teamStrategy.settings.tokenCeiling ?? '') + '"></label>' +
            '<label>Estimated cost ceiling, minor units (optional; requires verified pricing)<input type="number" name="estimatedCostCeilingMinor" min="1" placeholder="Not configured" value="' +
              safe(teamStrategy.settings.estimatedCostCeilingMinor ?? '') + '"></label>' +
            '<label><input type="checkbox" name="subagentDelegationEnabled"' +
              (teamStrategy.settings.subagentDelegationEnabled ? ' checked' : '') +
              '> Request delegated subagents (requires separate native authorization)</label>' +
            '<p>Only the native runtime issues grants. Strategy preferences never override Council, checkout isolation or tool enforcement. Unsupported ceilings and parallelism keep the Team Off.</p>' +
            '<button class="primary-button" type="submit"' +
              (chosen.enabled ? ' disabled' : '') + '>Save future-Team policy</button></form>') +
        '</section>' : '';
    const teamRoles = chosen
      ? '<section class="sa-pane" aria-label="Configured Team roles"><h3>Configure Team roles</h3>' +
        '<p>Coordinator, Workers and independent Council Reviewers reuse their native bot profiles, models, reasoning effort, permissions and Skills. Conflicting checkout writes remain serialized.</p>' +
        (!roleRoster || roleRoster.orchestrationId !== chosen.id
          ? '<button type="button" data-sa-role-load>Load native members</button>'
          : '<p>Coordinator: ' + safe(botName(roleRoster.coordinatorBotId)) +
            ' · Primary Worker: ' + safe(botName(roleRoster.workerBotIds[0])) +
            ' · Policy revision ' + roleRoster.policyRevision + '</p>' +
            '<form data-sa-form="roster">' +
              '<label>Additional Workers (up to 15)<select name="additionalWorkers" data-sa-worker-select multiple size="5">' +
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
              '<label>Optional Council Reviewers (up to 8)<select name="reviewers" data-sa-reviewer-select multiple size="5">' +
              roleOptions(reviewerDraft) + '</select></label>' +
              '<label><input type="checkbox" name="councilRequired"' +
              (roleRoster.councilRequired ? ' checked disabled' : ' disabled') +
              '> Council review is managed by the selected execution mode</label>' +
              '<p>Full Auto requires at least two reviewers. With Approval uses human acceptance. Change the mode in Execution and review; each native Ticket retains its frozen reviewers.</p>' +
              '<button type="button" data-sa-edit-mode>Configure execution mode</button>' +
              '<p>Saving roles appends a future-only Team version. Active Tickets retain their frozen roster, Council policy, mandate and grants. A saved primary identity cannot change after the Team has Ticket history.</p>' +
              '<button class="primary-button" type="submit"' +
                '>Save configured roles</button></form>' +
              '<p role="status">Native role configuration is saved per Team version. ' +
                (roleRoster.councilRequired ? 'Independent Council is required.' : 'Human approval policy is available.') +
                ' Execution eligibility and any active file grants are verified during native admission.</p>' +
            '<button type="button" data-sa-effective-load>Inspect effective policy and Skills</button>' +
            effectivePolicy) +
        '</section>' : '';
    const overview = chosen
      ? '<section class="sa-pane" aria-label="Team overview"><h3>Selected Team</h3>' +
        '<p>' + safe(chosen.name) + ' · ' + (chosen.enabled ? 'Enabled' : 'Off') +
        ' · Policy revision ' + chosen.policyRevision + '</p>' +
        '<dl><dt>Coordinator (read-only)</dt><dd>' + safe(botName(chosen.coordinatorBotId)) + '</dd>' +
        '<dt>Worker</dt><dd>' + safe(botName(chosen.workerBotId)) + '</dd>' +
        '<dt>Configured members</dt><dd>' + safe(roleSummary(chosen)) + '</dd>' +
        '<dt>Review policy</dt><dd>' + (chosen.councilRequired ? 'Native Council required' : 'Council Off') + '</dd>' +
        '<dt>Execution policy</dt><dd>' +
        (teamStrategy?.orchestrationId === chosen.id
          ? (teamStrategy.executionMode === 'full_auto' ? 'Full Auto · independent Council' :
            teamStrategy.executionMode === 'approval' ? 'With Approval · human acceptance' :
            'Choose an execution mode')
          : 'Loading native policy') +
        '</dd></dl>' +
        '<div class="sa-roster-summary"><h4>Existing bots</h4><ul>' +
          ([{role:'Coordinator',id:chosen.coordinatorBotId},
            ...(chosen.workerBotIds?.length ? chosen.workerBotIds : [chosen.workerBotId])
              .map((id,index)=>({role:'Worker ' + (index+1),id})),
            ...(chosen.reviewerBotIds ?? []).map((id,index)=>({role:'Reviewer ' + (index+1),id}))])
            .map(member=>{
              const bot=bots.find(candidate=>candidate.id===member.id);
              return '<li><span><small>' + safe(member.role) + '</small><strong>' +
                safe(bot?.name ?? member.id) + '</strong></span><span><small>' +
                safe(bot?.provider ?? 'Unknown harness') + '</small><strong>' +
                safe(bot?.model ?? 'Unavailable model') + '</strong></span></li>';
            }).join('') +
          '</ul><button type="button" data-sa-team-tab="members">Review native bot settings and roles</button></div>' +
        '<div class="sa-actions"><button type="button" data-sa-enable="' + safe(chosen.id) + '"' +
        '>' + (chosen.enabled ? 'Disable Team' : 'Enable Team') + '</button>' +
        '<button type="button" data-sa-view="runs">View runs</button></div>' +
        (teamStrategy?.orchestrationId === chosen.id &&
          teamStrategy.executionMode === 'full_auto' && (chosen.reviewerCount ?? chosen.reviewerBotIds?.length ?? 0) < 2
          ? '<p role="note">Full Auto requires at least two independent reviewers before the Team can be enabled.</p>' : '') +
        '</section>'
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
      '<p class="sa-nav-label">Workspace</p>' +
      ([
        ['teams','Teams'],['runs','Demands / Runs'],['review','Review inbox'],
      ] as const).map(([id,label]) =>
        '<button type="button" data-sa-view="' + id + '" aria-current="' +
        ((workspaceView === id || (workspaceView === 'team' && id === 'teams') ||
          (id === 'runs' && (workspaceView === 'new-demand' || workspaceView === 'run'))) ? 'page' : 'false') +
        '">' + label + '</button>').join('') +
      '<p class="sa-nav-label sa-nav-separator">Team settings</p>' +
      ([
        ['sources','GitHub sources'],['portable','Import / export'],
      ] as const).map(([id,label]) =>
        '<button type="button" data-sa-view="' + id + '" aria-current="' +
        (workspaceView === id ? 'page' : 'false') + '"' +
        (!presets.length ? ' disabled' : '') + '>' + label + '</button>').join('') +
      '</nav>';
    const directory = '<div class="sa-section-head"><div><h2>Teams</h2>' +
      '<p>Each Team is a reusable SuperAgent made from existing PandaCode bots.</p></div>' +
      '<button class="primary-button" type="button" data-sa-builder-open>New Team</button></div>' +
      (!presets.length ? '<section class="sa-pane sa-first-team"><h3>Create your first Team</h3>' +
        '<p>Assign a Coordinator, Workers and optional independent Reviewers. ' +
        'Choose Full Auto or With Approval before enabling the Team.</p>' +
        '<p>Creating a Team does not run a task or grant additional permissions.</p>' +
      '<ol class="sa-onboarding-flow" aria-label="From request to reviewed delivery">' +
        '<li><strong>1. Configure the Team</strong><span>Assign your existing bots and select an execution mode.</span></li>' +
        '<li><strong>2. Assign a Demand</strong><span>Describe the outcome and review native permissions.</span></li>' +
        '<li><strong>3. Review evidence</strong><span>Inspect Worker chats, Council findings or human decisions.</span></li>' +
      '</ol></section>' : '') +
      guidedBuilder.render(bots) +
      '<section class="sa-pane sa-teams-panel" aria-label="Saved Teams">' +
      '<div class="sa-list-controls"><label>Search Teams<input type="search" data-sa-team-search value="' +
      safe(teamSearch) + '" placeholder="Search teams"></label>' +
      '<label>Team status<select data-sa-team-status>' +
      '<option value="all"' + (teamStatusFilter === 'all' ? ' selected' : '') + '>All Teams</option>' +
      '<option value="enabled"' + (teamStatusFilter === 'enabled' ? ' selected' : '') + '>Enabled</option>' +
      '<option value="disabled"' + (teamStatusFilter === 'disabled' ? ' selected' : '') + '>Off</option>' +
      '</select></label></div>' +
      '<p class="sa-list-caption" role="status">' + visibleTeams.length + ' of ' +
        presets.length + ' Teams</p>' +
      '<ul class="sa-list">' + (list || '<li class="sa-empty">' +
      (presets.length ? 'No Teams match these filters.' : 'No Teams created yet.') +
      '</li>') + '</ul></section>';
    const teamView = chosen
      ? '<div class="sa-team-detail-head"><button type="button" data-sa-team-back>All Teams</button>' +
        '<div class="sa-team-detail-title"><div><h2>' + safe(chosen.name) + '</h2>' +
        '<p>Policy revision ' + chosen.policyRevision + ' · ' +
          (chosen.enabled ? 'Enabled' : 'Off') + ' · ' +
          (teamStrategy?.orchestrationId === chosen.id
            ? (teamStrategy.executionMode === 'full_auto' ? 'Full Auto' :
              teamStrategy.executionMode === 'approval' ? 'With Approval' :
              'Execution mode not configured')
            : 'Loading native policy') + '</p></div>' +
        '<button class="primary-button" type="button" data-sa-create-demand>Assign Demand</button>' +
        '</div></div>' +
        '<nav class="sa-subtabs" aria-label="Team details">' +
        ([
          ['overview','Overview'],['members','Members and roles'],['execution','Execution and review'],
        ] as const).map(([id,label]) =>
          '<button type="button" data-sa-team-tab="' + id + '" aria-current="' +
          (teamPanel === id ? 'page' : 'false') + '">' + label + '</button>').join('') +
        '</nav>' +
        (teamPanel === 'overview' ? overview :
          teamPanel === 'members' ? teamEditor + teamRoles : strategyPane)
      : '<section class="sa-pane"><h3>No Team selected</h3>' +
        '<p>Return to Teams and create or select a Team.</p></section>';
    host.innerHTML = '<div class="sa-ui">' +
      (loading ? '<p role="status">Loading orchestrations…</p>' : '') +
      (error ? '<p role="alert" class="sa-error">' + safe(error) + '</p>' : '') +
      (feedback ? '<p role="status" class="sa-feedback">' + safe(feedback) + '</p>' : '') +
      (confirm ? '<section role="alertdialog" aria-label="Confirm orchestration action" class="sa-confirm"><p>' +
        safe(confirm.message) + '</p><div class="sa-actions"><button type="button" data-sa-cancel>Cancel</button>' +
        '<button class="primary-button" type="button" data-sa-confirm>Confirm</button></div></section>' : '') +
      '<div class="sa-layout">' + navigation + '<div class="sa-stage">' +
      (workspaceView === 'teams' ? directory : '') +
      (workspaceView === 'team' ? teamView : '') +
      (workspaceView === 'runs'
        ? '<div class="sa-section-head"><div><h2>Demands / Runs</h2>' +
          '<p>Track native WorkItems, approvals and evidence across Teams.</p></div>' +
          '<button type="button" class="primary-button" data-sa-create-demand>New Demand</button></div>' +
          globalDemands : '') +
      (workspaceView === 'new-demand'
        ? '<div class="sa-section-head"><div><h2>Assign a Demand</h2>' +
          '<p>Describe the expected result, then review the native execution policy.</p></div></div>' +
          (admission || '<section class="sa-pane">Create a Team before admitting work.</section>') : '') +
      (workspaceView === 'run'
        ? active || '<section class="sa-pane">Open a native Demand from Demands / Runs.</section>' : '') +
      (workspaceView === 'review' ? reviewPanel : '') +
      (workspaceView === 'sources' ? (chosen ? '<section class="sa-pane">' + sourcePanel + '</section>'
        : '<section class="sa-pane"><p>Select a Team to configure its GitHub sources.</p></section>') : '') +
      (workspaceView === 'portable'
        ? '<section class="sa-pane"><h3>Import / export</h3>' +
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
    // During native hydration, local-only draft fields remain editable.
    // Stateful commands and controls must stay inert until the snapshot loads.
    if (loading) {
      host.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
        'button, input, select, textarea',
      ).forEach(control => { control.disabled = true; });
    }
    host.setAttribute('aria-busy', String(busy || loading));
    restoreFocusedControl(focusedControl);
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
    delivery=null; deliveryLoaded=false;
    providerChildren=null;providerChildrenLoaded=false;
    const [snapshot, links] = await Promise.all([
      call<TicketDetails>('superagent_snapshot', { ticketId:id, limit:50 }),
      call<ThreadLink[]>('superagent_source_thread_links', { ticketId:id }),
    ]);
    if (inspectedTicket === id) {
      if (snapshot.ticket.id !== id || snapshot.ticket.orchestrationId !== selected) {
        ticketDetails = null; threadLinks = []; inspectedAssignmentId = '';
        throw new Error('Native Ticket identity or Team changed; reload before acting.');
      }
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
        teamStrategy = null;
        inspectedTicket = ''; threadLinks = [];
        linkedDecisions = []; linkedDecisionsLoaded = false; roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
        sourceIntegrity = null;
        autoStatus = null;
      }
      // A failed native read must not leave old ticket approval controls
      // visible underneath a newly selected or reloaded preset.
      tickets = [];
      ticketDetails = null; inspectedAssignmentId = '';
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      // An inspected cross-Team Demand can fall outside this Team's latest-50
      // page. Rehydrate its exact native identity before showing actions.
      if (inspectedTicket && !tickets.some(ticket => ticket.id === inspectedTicket)) {
        const historical = await call<TicketDetails>('superagent_snapshot', {
          ticketId:inspectedTicket, limit:50,
        });
        if (historical.ticket.id !== inspectedTicket ||
            historical.ticket.orchestrationId !== selected) {
          inspectedTicket = '';
          throw new Error('Native demand ownership changed; reload the index.');
        }
        tickets = [historical.ticket, ...tickets];
      }
      await refreshTicketDetails();
      if (roleRoster && selected) {
        roleRoster = await call<TeamRoster>('superagent_roster',{orchestrationId:selected});
        workerOrder = roleRoster.workerBotIds.slice(1);
        reviewerDraft = [...roleRoster.reviewerBotIds];
      }
      // Policy and roster always come from the native revisioned Team; no
      // local template or cached UI selection becomes execution authority.
      if (selected) {
        const [policy, roster] = await Promise.all([
          call<StrategyView>('superagent_strategy_get',{orchestrationId:selected}),
          call<TeamRoster>('superagent_roster',{orchestrationId:selected}),
        ]);
        const currentConfig = presets.find(config => config.id === selected);
        if (currentConfig && policy.policyRevision === currentConfig.policyRevision &&
            roster.policyRevision === currentConfig.policyRevision) {
          teamStrategy = policy; roleRoster = roster;
          workerOrder = roster.workerBotIds.slice(1);
          reviewerDraft = [...roster.reviewerBotIds];
        } else {
          teamStrategy = null; roleRoster = null;
        }
      } else {
        teamStrategy = null; roleRoster = null;
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
        autoStatus = null;
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
  host.addEventListener('change', event => {
    const target = event.target;
    if (target instanceof Element && guidedBuilder.handleChange(target)) {
      render();
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-reviewer-select')) {
      if (busy || !roleRoster || roleRoster.orchestrationId !== selected) return;
      reviewerDraft = [...target.selectedOptions].map(option => option.value);
      render();
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-worker-select')) {
      if (busy || !roleRoster || roleRoster.orchestrationId !== selected) return;
      const next = Array.from(target.selectedOptions, option => option.value);
      workerOrder = [...workerOrder.filter(id => next.includes(id)),
        ...next.filter(id => !workerOrder.includes(id))];
      render();
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-admission-team')) {
      if (busy || loading || !presets.some(team => team.id === target.value)) return;
      selected = target.value; inspectedTicket = ''; ticketDetails = null;
      roleRoster = null; teamStrategy = null;
      void work(reload);
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
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:filter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
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
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:filter || undefined,
          searchQuery:demandSearch || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
  });
  host.addEventListener('input', event => {
    const target = event.target;
    if (target instanceof Element && guidedBuilder.handleInput(target)) return;
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-demand-search')) {
      demandSearchDraft = target.value;
      return;
    }
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-demand-project-input')) {
      demandProjectDraft = target.value;
      return;
    }
    if (target instanceof HTMLInputElement &&
        target.closest('form[data-sa-form="draft-pr-preflight"]')) {
      if (target.name === 'repository') draftPrRepository = target.value;
      if (target.name === 'head') draftPrHead = target.value;
      if (target.name === 'base') draftPrBase = target.value;
      draftPrError = ''; draftPrVerifiedSha = ''; draftPrVerifiedRevision = -1;
      host.querySelector('[data-sa-draft-pr-error]')?.remove();
      host.querySelector('[data-sa-draft-pr-sha]')?.remove();
      return;
    }
    if ((target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) &&
        target.closest('form[data-sa-form="issue-comment"]')) {
      // Persist the user's in-progress local draft before any native refresh
      // replaces the form. No model, external effect or repository call runs.
      if (commentDraftTicketId !== inspectedTicket) return;
      if (target.name === 'repository') commentRepository = target.value;
      if (target.name === 'issueNumber') commentIssueNumber = target.value;
      if (target.name === 'comment') commentDraft = target.value;
      issueCommentError = '';
      host.querySelector('[data-sa-comment-error]')?.remove();
      return;
    }
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-team-search')) {
      teamSearch = target.value;
      const selection = target.selectionStart;
      render();
      const replacement = host.querySelector<HTMLInputElement>('[data-sa-team-search]');
      replacement?.focus({preventScroll:true});
      if (replacement && selection !== null) replacement.setSelectionRange(selection, selection);
      return;
    }
    if (target instanceof HTMLTextAreaElement && target.name === 'initialAllowedPaths' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      initialDemandWritePaths = target.value;
      return;
    }
    if (target instanceof HTMLTextAreaElement && target.name === 'objective' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      demandObjective = target.value;
      host.querySelector('[aria-label="Native demand preflight"]')?.remove();
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
    if (target.dataset.saOpenAssignmentChat) {
      const assignment = ticketDetails?.assignments.find(item =>
        item.sessionId === target.dataset.saOpenAssignmentChat &&
        item.id === inspectedAssignmentId);
      if (!assignment || ticketDetails?.ticket.id !== inspectedTicket) return;
      host.dispatchEvent(new CustomEvent('superagent-open-bot-chat',{
        detail:{sessionId:assignment.sessionId, botId:assignment.botId},
      }));
      return;
    }
    if (target.dataset.saOpenWorkitemChat) {
      const childId = target.dataset.saOpenWorkitemChat;
      const item = workPlan?.ticketId === inspectedTicket &&
        workPlan.items.find(entry => entry.childTicketId === childId);
      if (!item || !childId) return;
      void work(async () => {
        const snapshot = await call<TicketDetails>('superagent_snapshot',{
          ticketId:childId, limit:10,
        });
        if (snapshot.ticket.id !== childId || !workPlan?.items.some(entry =>
          entry.childTicketId === childId && entry.workerBotId === item.workerBotId)) {
          throw new Error('Native WorkItem identity changed; reload its plan.');
        }
        const assignment = snapshot.assignments.find(entry =>
          entry.botId === item.workerBotId && Boolean(entry.sessionId));
        if (!assignment) throw new Error('This WorkItem has no native bot conversation yet.');
        host.dispatchEvent(new CustomEvent('superagent-open-bot-chat',{
          detail:{sessionId:assignment.sessionId, botId:assignment.botId},
        }));
      });
      return;
    }
    if (guidedBuilder.handleClick(target, bots)) {
      render();
      host.querySelector<HTMLElement>('[aria-label="Team setup step"] h4')
        ?.focus({preventScroll:true});
      return;
    }
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
    if (target.hasAttribute('data-sa-return-team')) {
      workspaceView = 'team'; render(); return;
    }
    if (target.hasAttribute('data-sa-back-runs')) {
      workspaceView = 'runs'; inspectedTicket = ''; ticketDetails = null;
      render();
      if (!allDemandsLoaded)
        host.querySelector<HTMLButtonElement>('[data-sa-load-all-demands]')?.click();
      return;
    }
    if (target.hasAttribute('data-sa-create-demand')) {
      workspaceView = presets.length ? 'new-demand' : 'teams';
      render();
      if (presets.length) {
        host.querySelector<HTMLTextAreaElement>('form[data-sa-form="run"] textarea[name="objective"]')
          ?.focus({preventScroll:true});
      }
      return;
    }
    if (target.hasAttribute('data-sa-team-back')) {
      workspaceView = 'teams'; render(); return;
    }
    if (target.hasAttribute('data-sa-edit-reviewers')) {
      teamPanel = 'members'; render();
      host.querySelector<HTMLSelectElement>('[data-sa-reviewer-select]')?.focus({preventScroll:true});
      return;
    }
    if (target.hasAttribute('data-sa-edit-mode')) {
      teamPanel = 'execution'; render();
      host.querySelector<HTMLInputElement>('input[name="executionMode"]:checked')?.focus({preventScroll:true});
      return;
    }
    if (target.dataset.saTeamTab) {
      const tab = target.dataset.saTeamTab;
      if (!['overview','members','execution'].includes(tab)) return;
      teamPanel = tab as typeof teamPanel; render(); return;
    }
    if (target.dataset.saView) {
      const next = target.dataset.saView;
      if (!['teams','runs','review','sources','portable'].includes(next)) return;
      workspaceView = next as typeof workspaceView;
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-view="' + next + '"]')?.focus({preventScroll:true});
      if (next === 'runs' && !allDemandsLoaded)
        host.querySelector<HTMLButtonElement>('[data-sa-load-all-demands]')?.click();
      if (next === 'review' && !reviewQueueLoaded)
        host.querySelector<HTMLButtonElement>('[data-sa-load-review-inbox]')?.click();
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
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
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
        const loaded = await fetchDemandPage({
          limit:200, beforeUpdatedAt:last.updatedAt, beforeId:last.id,
          orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
        });
        const seen = new Set(allDemands.map(item => item.id));
        allDemands = [...allDemands, ...loaded.filter(item => !seen.has(item.id))];
        allDemandsHasMore = loaded.length === 200;
      });
      return;
    }
    if (target.dataset.saStopGlobalDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saStopGlobalDemand);
      if (!demand || busy || !presets.some(item => item.id === demand.orchestrationId)) return;
      void work(async () => {
        // A cross-Team index is observation only. Re-read native ownership and
        // revision, then commit Stop under the identical revision guard.
        const snapshot = await call<TicketDetails>('superagent_snapshot',
          {ticketId:demand.id,limit:1});
        if (snapshot.ticket.id !== demand.id ||
            snapshot.ticket.orchestrationId !== demand.orchestrationId ||
            snapshot.ticket.revision !== demand.revision ||
            snapshot.ticket.state !== demand.state) {
          throw new Error('Demand changed since the native page was loaded; refresh before stopping.');
        }
        if (['completed','failed','stopped','interrupted','inconclusive']
          .includes(snapshot.ticket.state)) return;
        ask('Stop Demand ' + demand.id + ' at revision ' + demand.revision +
          '? Native Stop revokes its mandate and quarantines pending effects.', async () => {
          await call('superagent_stop',
            {ticketId:demand.id,expectedRevision:snapshot.ticket.revision});
          await reload();
          feedback = 'Demand ' + demand.id + ' stopped by native revocation. Refresh Demands for current states.';
        });
      });
      return;
    }
    if (target.dataset.saOpenDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saOpenDemand) ??
        reviewQueue.find(item => item.id === target.dataset.saOpenDemand);
      if (!demand || !presets.some(item => item.id === demand.orchestrationId)) return;
      selected = demand.orchestrationId;
      workspaceView = 'run';
      tickets = []; inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; workerOrder = []; reviewerDraft = [];
      effectiveTeam = null; teamStrategy = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null;
      void work(async () => {
        const [loaded, policy, roster] = await Promise.all([
          call<Ticket[]>('superagent_tickets',{
            orchestrationId:demand.orchestrationId,limit:50,
          }),
          call<StrategyView>('superagent_strategy_get',{orchestrationId:demand.orchestrationId}),
          call<TeamRoster>('superagent_roster',{orchestrationId:demand.orchestrationId}),
        ]);
        const cfg=presets.find(item=>item.id===demand.orchestrationId);
        if (selected!==demand.orchestrationId || !cfg ||
            policy.policyRevision!==cfg.policyRevision ||
            roster.policyRevision!==cfg.policyRevision) {
          throw new Error('Native Team policy changed while opening the Demand; reload.');
        }
        tickets=loaded; teamStrategy=policy; roleRoster=roster;
        workerOrder=roster.workerBotIds.slice(1); reviewerDraft=[...roster.reviewerBotIds];
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
        reviewerDraft=[...roleRoster.reviewerBotIds];
      });
      return;
    }
    if (target.hasAttribute('data-sa-strategy-load')) {
      const cfg = current();
      if (cfg) void work(async () => {
        const snapshot = await call<StrategyView>('superagent_strategy_get',{
          orchestrationId:cfg.id,
        });
        if (selected !== cfg.id || snapshot.orchestrationId !== cfg.id ||
            snapshot.policyRevision !== cfg.policyRevision) {
          throw new Error('Team strategy changed. Reload before editing.');
        }
        teamStrategy = snapshot;
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
    if (target.dataset.saSelect) {
      if (busy || loading) return;
      workspaceView = 'team'; teamPanel = 'overview';
      if (selected === target.dataset.saSelect) { render(); return; }
      const next = target.dataset.saSelect;
      selected = next;
      tickets = [];
      inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; effectiveTeam = null; teamStrategy = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null;
      void work(async () => {
        const [loaded, policy, roster] = await Promise.all([
          call<Ticket[]>('superagent_tickets',{ orchestrationId:next, limit:50 }),
          call<StrategyView>('superagent_strategy_get',{orchestrationId:next}),
          call<TeamRoster>('superagent_roster',{orchestrationId:next}),
        ]);
        if (selected !== next) return;
        tickets = loaded; teamStrategy = policy; roleRoster = roster;
        workerOrder = roster.workerBotIds.slice(1); reviewerDraft = [...roster.reviewerBotIds];
      });
      return;
    }
    if (target.dataset.saEnable) {
      const c = presets.find(item => item.id === target.dataset.saEnable);
      if (c && !c.enabled && (teamStrategy?.orchestrationId !== c.id ||
          teamStrategy.executionMode === 'off')) {
        teamPanel = 'execution';
        error = 'Select and save With Approval or Full Auto before enabling this Team.';
        render(); return;
      }
      if (c) ask((c.enabled ? 'Disable ' : 'Enable the configured mode for ') + c.name + '?', async () => {
        await call('superagent_set_enabled',{ orchestrationId:c.id, expectedRevision:c.policyRevision, enabled:!c.enabled });
        await reload();
      });
      return;
    }
    if (target.dataset.saDuplicate) {
      const source = presets.find(item => item.id === target.dataset.saDuplicate);
      if (!source || busy) return;
      const base = source.name.trim() + ' copy';
      let name = base;
      for (let index = 2; presets.some(item => item.name.toLowerCase() === name.toLowerCase()); index++) {
        name = base + ' ' + index;
      }
      if (new TextEncoder().encode(name).length > 120) {
        error = 'The Team name is too long to duplicate. Rename the source Team first.';
        render(); return;
      }
      ask('Create a disabled copy of ' + source.name +
        ' with its existing role roster? Tickets, source subscriptions, permissions and approvals are not copied.', async () => {
        // Source revision and frozen role snapshot are checked inside one
        // native transaction. Renderer reads cannot grant or duplicate authority.
        const created = await call<Config>('superagent_duplicate', {
          sourceId:source.id, expectedRevision:source.policyRevision, name,
        });
        if (created.enabled || created.id === source.id) {
          throw new Error('Native duplication failed to create an independent disabled Team.');
        }
        selected = created.id; workspaceView = 'teams';
        roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
        await reload();
        feedback = 'Disabled Team duplicated. Native roles were retained; no tasks or permissions copied.';
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
    if (target.dataset.saAutoStatus) {
      const sourceId = target.dataset.saAutoStatus;
      if (sources.some(src => src.id === sourceId && src.mode === 'observe' &&
          src.eventKind === 'issues.opened')) void work(async () => {
        const found = await call<AutoSourceStatus>('superagent_auto_status',{sourceId});
        if (sources.some(src => src.id === sourceId) && found.sourceId === sourceId) {
          autoStatus=found;
        }
      });
      return;
    }
    if (target.dataset.saAutoRevoke) {
      const sourceId = target.dataset.saAutoRevoke;
      const src = sources.find(src => src.id === sourceId);
      if (!src || !autoStatus?.enabled || autoStatus.sourceId !== sourceId) return;
      ask('Revoke this exact read-only GitHub Auto source? All pending admissions will be blocked; existing matching native Tickets are stopped, and ordinary Observe receipts remain intact.',async () => {
        const outcome=await call<AutoSourceStatus>('superagent_auto_revoke',{
          sourceId,expectedRevision:src.policyRevision,
        });
        autoStatus=outcome;
        await reload();
        autoStatus=await call<AutoSourceStatus>('superagent_auto_status',{sourceId});
        feedback='Auto source revoked natively. Pending intake blocked and live tasks stopped; historical receipts remain.';
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
    if (target.hasAttribute('data-sa-load-subagents')) {
      const ticketId=inspectedTicket;
      if (ticketId && ticketDetails?.ticket.id===ticketId) void work(async () => {
        const result=await call<NativeSubagentTree>('superagent_subagents',{ticketId});
        if (inspectedTicket===ticketId && result.rootTicketId===ticketId) {
          subagentTree=result; subagentTreeLoaded=true;
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-provider-children')) {
      const ticketId=inspectedTicket;
      if(!ticketId || !ticketDetails || ticketDetails.ticket.id!==ticketId)return;
      void work(async()=>{
        const observed=await call<ObservedChildTree>('superagent_observed_child_tree',{ticketId});
        const pinnedVersion=ticketDetails?.teamSnapshot?.teamVersion;
        if(observed.ticketId!==ticketId || inspectedTicket!==ticketId ||
            (typeof pinnedVersion==='number' && observed.teamVersion!==pinnedVersion)){
          throw new Error('Provider child stream changed before native inspection.');
        }
        providerChildren=observed;providerChildrenLoaded=true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-inspect-delivery')) {
      const ticketId=inspectedTicket;
      if (!ticketId || !ticketDetails || ticketDetails.ticket.id!==ticketId) return;
      void work(async()=>{
        const observed=await call<DeliveryObservation>('superagent_delivery_inspect',{ticketId});
        if(observed.ticketId!==ticketId || observed.readOnly!==true)
          throw new Error('Native delivery inspection did not match this Ticket.');
        if(inspectedTicket===ticketId){delivery=observed;deliveryLoaded=true;}
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-work-plan')) {
      const ticketId = inspectedTicket;
      const selectedTicket = tickets.find(ticket => ticket.id === ticketId);
      if (ticketId && selectedTicket && ticketDetails?.ticket.id === ticketId) void work(async () => {
        const plan = await call<DemandPlanEvidence | null>('superagent_demand_plan',{ticketId});
        if (inspectedTicket === ticketId) {
          if (plan && plan.ticketId !== ticketId) {
            throw new Error('Native WorkItem plan belongs to a different Ticket.');
          }
          workPlan = plan;
          workPlanLoaded = true;
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-authorize-demand-work')) {
      const ticket=tickets.find(ticket=>ticket.id===inspectedTicket);
      const eligible=workPlan?.items.filter(item=>item.requiresWrite &&
        item.state==='pending' && !item.writeAuthorized && !item.childTicketId) ?? [];
      if (ticket?.state==='queued' && workPlan?.ticketId===ticket.id &&
          workPlan.state==='active' && eligible.length>1) {
        workWriteItemKey='*'; workWritePaths=''; render();
        host.querySelector<HTMLTextAreaElement>(
          'form[data-sa-form="work-write-scope"] textarea')?.focus();
      }
      return;
    }
    if (target.dataset.saAuthorizeWork) {
      const item = workPlan?.items.find(item => item.key === target.dataset.saAuthorizeWork);
      const ticket = tickets.find(ticket => ticket.id === inspectedTicket);
      if (item?.requiresWrite && item.state === 'pending' && !item.writeAuthorized &&
          ticket?.state === 'queued' && workPlan?.state === 'active' &&
          workPlan.ticketId === ticket.id) {
        workWriteItemKey = item.key; workWritePaths = ''; render();
        host.querySelector<HTMLTextAreaElement>('form[data-sa-form="work-write-scope"] textarea')?.focus();
      }
      return;
    }
    if (target.hasAttribute('data-sa-work-write-cancel')) {
      workWriteItemKey = ''; workWritePaths = ''; render();
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
      subagentTree = null; subagentTreeLoaded = false;
      workPlan = null; workPlanLoaded = false;
      delivery=null; deliveryLoaded=false;
      providerChildren=null;providerChildrenLoaded=false;
      workWriteItemKey = ''; workWritePaths = '';
      const id = target.dataset.saInspectTicket;
      if (inspectedTicket === id) {
        if (workspaceView === 'run') void work(refreshTicketDetails);
        else { inspectedTicket = ''; ticketDetails = null; inspectedAssignmentId = ''; render(); }
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
        await call('superagent_stop',{ ticketId:t.id, expectedRevision:t.revision }); await reload();
      });
    }
  });
  host.addEventListener('submit', event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !host.contains(form)) return;
    event.preventDefault();
    // A synthetic submit or Enter must never bypass native hydration or busy gates.
    if (loading || busy) return;
    const data = new FormData(form);
    if (form.dataset.saForm === 'auto-authorization') {
      const status=autoStatus;
      const src=sources.find(item => item.id === status?.sourceId);
      const maxDailyTasks=Number(data.get('maxDailyTasks'));
      if (!status || !src || src.mode !== 'observe' ||
          src.eventKind !== 'issues.opened' || status.enabled ||
          !Number.isInteger(maxDailyTasks) || maxDailyTasks<1 || maxDailyTasks>20) {
        error='Read-only Auto requires an unchanged Observe issues.opened source and a daily limit from 1 to 20.';
        render(); return;
      }
      autoDailyLimit=maxDailyTasks;
      void work(async () => {
        const result=await call<AutoSourceStatus>('superagent_auto_authorize',{
          sourceId:src.id,expectedRevision:src.policyRevision,maxDailyTasks,
        });
        if (result.sourceId!==src.id || !result.enabled || result.maxDailyTasks!==maxDailyTasks) {
          throw new Error('The native Auto grant does not match the inspected source policy.');
        }
        autoStatus=result;
        await reload();
        autoStatus=await call<AutoSourceStatus>('superagent_auto_status',{sourceId:src.id});
        feedback='Local GitHub Auto grant authorized for bounded read-only issue intake. Explicit file-write confirmations remain required.';
      });
      return;
    }
    if (form.dataset.saForm === 'enable-subagents') {
      const ticket=ticketDetails?.ticket;
      const tree=subagentTree;
      const currentWorkerIds=ticketDetails?.teamSnapshot?.workerBotIds;
      const eligible=Boolean(ticketDetails?.activity.some(entry =>
        entry.kind==='demand_auto_plan_requested') &&
        Array.isArray(currentWorkerIds) && currentWorkerIds.length>=2);
      const maxDepth=Number(data.get('maxDepth'));
      const maxChildren=Number(data.get('maxChildren'));
      const maxTotal=Number(data.get('maxTotal'));
      if (!ticket || inspectedTicket!==ticket.id || !eligible ||
          !['running','queued'].includes(ticket.state) || tree?.policy ||
          !Number.isInteger(maxDepth) || maxDepth<1 || maxDepth>3 ||
          !Number.isInteger(maxChildren) || maxChildren<1 || maxChildren>4 ||
          !Number.isInteger(maxTotal) || maxTotal<1 || maxTotal>24 ||
          maxChildren>maxTotal) {
        error='Choose an active, multiworker Demand and finite nesting limits.';
        render(); return;
      }
      subagentDepth=maxDepth; subagentPerParent=maxChildren; subagentTotal=maxTotal;
      const ticketId=ticket.id, expectedRevision=ticket.revision;
      void work(async () => {
        const policy=await call<NativeSubagentPolicy>('superagent_enable_subagents',{
          ticketId,expectedRevision,maxDepth,maxChildren,maxTotal,
        });
        if (policy.rootTicketId!==ticketId || policy.revoked) {
          throw new Error('Native descendant policy does not belong to the selected Demand.');
        }
        await reload();
        if (inspectedTicket===ticketId) {
          subagentTree=await call<NativeSubagentTree>('superagent_subagents',{ticketId});
          subagentTreeLoaded=true;
        }
        feedback='Native subagent policy authorized. Worker-created children remain bounded, read-only, and subject to their own review.';
      });
      return;
    }
    if (form.dataset.saForm === 'work-write-scope') {
      const ticketId = inspectedTicket;
      const plan = workPlan;
      const itemKey = workWriteItemKey;
      const batch = itemKey === '*';
      const ticket = tickets.find(ticket => ticket.id === ticketId);
      workWritePaths = String(data.get('allowedPaths') ?? '');
      const paths = workWritePaths.split(/\r\n?|\n/).filter(Boolean);
      const pendingItems = plan?.items.filter(item => item.state === 'pending' &&
        item.requiresWrite && !item.writeAuthorized && !item.childTicketId) ?? [];
      const selectedItems = batch ? pendingItems :
        pendingItems.filter(item => item.key === itemKey);
      if (!plan || !ticket || ticket.state !== 'queued' || plan.state !== 'active' ||
          plan.ticketId !== ticketId || plan.ticketRevision !== ticket.revision ||
          selectedItems.length !== (batch ? pendingItems.length : 1) ||
          (batch && selectedItems.length < 2) ||
          paths.length === 0 || paths.length > 16) {
        error = 'A current, pending write scope and 1 to 16 exact paths are required.';
        render(); return;
      }
      void work(async () => {
        const command = batch
          ? 'superagent_authorize_demand_work_item_writes'
          : 'superagent_authorize_work_item_write';
        const args = batch
          ? {ticketId, expectedRevision:plan.ticketRevision, allowedPaths:paths}
          : {ticketId, expectedRevision:plan.ticketRevision, itemKey, allowedPaths:paths};
        const result = await call<DemandPlanEvidence>(command, args);
        if (result.ticketId !== ticketId || selectedItems.some(selected =>
          !result.items.some(item => item.key === selected.key &&
            item.requiresWrite && item.writeAuthorized))) {
          throw new Error('Native file permission did not match the selected WorkItems.');
        }
        workWriteItemKey = ''; workWritePaths = '';
        await reload();
        if (inspectedTicket === ticketId) {
          workPlan = await call<DemandPlanEvidence | null>('superagent_demand_plan',{ticketId});
          workPlanLoaded = true;
        }
        feedback = batch
          ? 'One native confirmation authorized ' + selectedItems.length +
            ' pending WorkItems, each with the same exact file paths and separate grants.'
          : 'Exact WorkItem file scope confirmed natively. The scheduler will admit the eligible Worker without broader tool rights.';
      });
      return;
    }
    if (form.dataset.saForm === 'draft-pr-preflight') {
      const ticketId = inspectedTicket;
      const ticket = tickets.find(item => item.id === ticketId);
      const snapshot = ticketDetails;
      const repository = String(data.get('repository') ?? '');
      const head = String(data.get('head') ?? '');
      const base = String(data.get('base') ?? '');
      draftPrRepository = repository; draftPrHead = head; draftPrBase = base;
      draftPrVerifiedSha = ''; draftPrVerifiedRevision = -1;
      const parts = repository.split('/');
      const validRepository = repository.length <= 200 && parts.length === 2 &&
        parts.every(part => part.length > 0 && part !== '.' && part !== '..' &&
          /^[A-Za-z0-9_.-]+$/.test(part));
      const validBranch = (value: string) => value.length > 0 && value.length <= 200 &&
        /^[A-Za-z0-9_./-]+$/.test(value) && !value.startsWith('-') &&
        !value.endsWith('.') && !value.endsWith('/') &&
        !value.includes('..') && !value.includes('//');
      if (!validRepository || !validBranch(head) || !validBranch(base) || head === base) {
        draftPrError = 'Enter an exact owner/name repository and distinct valid head/base branches.';
        render(); return;
      }
      if (!ticket || ticket.state !== 'running' || !snapshot ||
          snapshot.ticket.id !== ticketId || snapshot.ticket.revision !== ticket.revision ||
          snapshot.mandate.revoked || !snapshot.mandate.allowedSideEffects) {
        draftPrError = 'The Ticket has no current side-effect mandate; refresh evidence.';
        render(); return;
      }
      const expectedRevision = snapshot.ticket.revision;
      draftPrError = '';
      void work(async () => {
        await refreshTicketDetails();
        const refreshed = ticketDetails;
        const currentTicket = tickets.find(item => item.id === ticketId);
        if (inspectedTicket !== ticketId || !refreshed || !currentTicket ||
            refreshed.ticket.id !== ticketId || refreshed.ticket.revision !== expectedRevision ||
            currentTicket.revision !== expectedRevision ||
            refreshed.ticket.state !== 'running' || refreshed.mandate.revoked ||
            !refreshed.mandate.allowedSideEffects) {
          throw new Error('The Ticket or its mandate changed; review native evidence again.');
        }
        const sha = await call<string>('superagent_verify_draft_pull_request_target', {
          ticketId, expectedRevision, repository, head, base,
        });
        if (!/^[0-9a-fA-F]{40}([0-9a-fA-F]{24})?$/.test(sha)) {
          throw new Error('GitHub did not return a valid remote head commit SHA.');
        }
        if (inspectedTicket !== ticketId || draftPrRepository !== repository ||
            draftPrHead !== head || draftPrBase !== base) return;
        draftPrVerifiedSha = sha;
        draftPrVerifiedRevision = expectedRevision;
        feedback = 'Read-only target verification succeeded for ' + repository + ':' + head +
          '. No branch was pushed, pull request created, or permission granted.';
      });
      return;
    }
    if (form.dataset.saForm === 'issue-comment') {
      const ticketId = inspectedTicket;
      const ticket = tickets.find(item => item.id === ticketId);
      const snapshot = ticketDetails;
      const repository = String(data.get('repository') ?? '');
      const issueText = String(data.get('issueNumber') ?? '');
      const issueNumber = Number(issueText);
      const comment = String(data.get('comment') ?? '');
      commentRepository = repository; commentIssueNumber = issueText; commentDraft = comment;
      const parts = repository.split('/');
      const validRepository = parts.length === 2 && parts.every(part => part.length > 0 &&
        part !== '.' && part !== '..' && /^[A-Za-z0-9_.-]+$/.test(part));
      let validationError = '';
      if (!validRepository) validationError = 'Enter one exact GitHub repository as owner/name.';
      else if (!/^[1-9][0-9]*$/.test(issueText) || !Number.isSafeInteger(issueNumber)) {
        validationError = 'Enter a positive whole issue number.';
      } else if (!comment.trim() || new TextEncoder().encode(comment).length > 60_000 ||
          comment.includes('\0') || comment.includes('<!-- pandacode-superagent-effect:')) {
        validationError = 'Enter a nonempty comment under 60 KB without a PandaCode effect marker.';
      }
      if (validationError) {
        issueCommentError = validationError; render(); return;
      }
      if (!ticket || ticket.state !== 'running' || !snapshot || snapshot.ticket.id !== ticketId ||
          snapshot.ticket.revision !== ticket.revision || snapshot.mandate.revoked ||
          !snapshot.mandate.allowedSideEffects) {
        issueCommentError = 'This Ticket has no current side-effect authorization. Refresh evidence before continuing.';
        render(); return;
      }
      const expectedRevision = snapshot.ticket.revision;
      issueCommentError = '';
      void work(async () => {
        await refreshTicketDetails();
        const refreshed = ticketDetails;
        const currentTicket = tickets.find(item => item.id === ticketId);
        if (!refreshed || !currentTicket || refreshed.ticket.revision !== expectedRevision ||
            currentTicket.revision !== expectedRevision) {
          throw new Error('This Ticket changed. Review its updated evidence before continuing.');
        }
        if (refreshed.ticket.state !== 'running' || refreshed.mandate.revoked ||
            !refreshed.mandate.allowedSideEffects) {
          throw new Error('The Ticket is no longer running under an active side-effect mandate.');
        }
        const effectKey = await issueCommentEffectKey(ticketId, repository, issueNumber, comment);
        const result = await call<IssueCommentResult>('superagent_issue_comment', {
          ticketId, expectedRevision, repository, issueNumber, effectKey, comment,
        });
        if ('previewOnly' in result) {
          issueCommentStatus = null;
          feedback = result.message;
          return;
        }
        if (result.ticketId !== ticketId || result.ticketRevision !== expectedRevision ||
            result.repository !== repository || result.issueNumber !== issueNumber ||
            result.effectKey !== effectKey) {
          throw new Error('The native effect result did not match this Ticket and exact comment.');
        }
        issueCommentStatus = result;
        feedback = result.state === 'applied' && result.reconciliation === 'exact-readback'
          ? 'GitHub comment applied and verified by exact read-back.'
          : 'GitHub comment state: ' + result.state + ' (' + result.reconciliation + '). Repeating the same comment checks its saved receipt and does not blindly post again.';
      });
      return;
    }
    if (form.dataset.saForm === 'demand-project-filter') {
      const next = String(data.get('projectId') ?? '').trim();
      if (next.length > 128 || (next !== '' && !/^[A-Za-z0-9_-]+$/.test(next))) {
        error = 'Project filter requires an exact native ID (letters, digits, hyphen or underscore).';
        render(); return;
      }
      demandProjectFilter = next; demandProjectDraft = next;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
        });
        allDemands = loaded;
        allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (form.dataset.saForm === 'demand-search') {
      const next = String(data.get('searchQuery') ?? '').trim();
      if (new TextEncoder().encode(next).length > 128 || /[\x00-\x1f\x7f]/.test(next)) {
        error = 'Search requires at most 128 characters without control characters.';
        render(); return;
      }
      demandSearch = next; demandSearchDraft = next;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined, searchQuery:demandSearch || undefined,
        });
        allDemands = loaded;
        allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (form.dataset.saForm === 'strategy-update') {
      const cfg = current();
      const loaded = teamStrategy;
      if (!cfg || !loaded || cfg.id !== loaded.orchestrationId ||
          cfg.policyRevision !== loaded.policyRevision || cfg.enabled) return;
      const optionalPositive = (name: string): number | undefined => {
        const raw = String(data.get(name) ?? '').trim();
        if (!raw) return undefined;
        const value = Number(raw);
        return Number.isSafeInteger(value) && value > 0 ? value : NaN;
      };
      const input = {
        priority:String(data.get('priority') ?? ''),
        maxConcurrentTasks:Number(data.get('maxConcurrentTasks')),
        maxAttemptsPerAssignment:Number(data.get('maxAttemptsPerAssignment')),
        attemptTimeoutMs:Number(data.get('attemptTimeoutMs')),
        tokenCeiling:optionalPositive('tokenCeiling'),
        estimatedCostCeilingMinor:optionalPositive('estimatedCostCeilingMinor'),
        subagentDelegationEnabled:data.has('subagentDelegationEnabled'),
        executionMode:String(data.get('executionMode') ?? ''),
      };
      if (!['approval','full_auto'].includes(input.executionMode) ||
          !['balanced','cost','latency','throughput','quality'].includes(input.priority) ||
          !Number.isInteger(input.maxConcurrentTasks) || input.maxConcurrentTasks < 1 || input.maxConcurrentTasks > 8 ||
          !Number.isInteger(input.maxAttemptsPerAssignment) || input.maxAttemptsPerAssignment < 1 ||
          input.maxAttemptsPerAssignment > 8 || !Number.isInteger(input.attemptTimeoutMs) ||
          input.attemptTimeoutMs < 60000 || input.attemptTimeoutMs > 1800000 ||
          Number.isNaN(input.tokenCeiling) || Number.isNaN(input.estimatedCostCeilingMinor)) {
        error = 'Correct the bounded strategy settings before saving.';
        render(); return;
      }
      const roster = roleRoster;
      if (!roster || roster.orchestrationId !== cfg.id ||
          roster.policyRevision !== cfg.policyRevision) {
        error = 'The native roster is stale. Refresh this Team before updating its execution mode.';
        render(); return;
      }
      if (input.executionMode === 'full_auto' && roster.reviewerBotIds.length < 2) {
        error = 'Full Auto needs two independent reviewer bots. Configure them in Members and roles before saving.';
        teamPanel = 'members'; render(); return;
      }
      void work(async () => {
        let expectedRevision = loaded.policyRevision;
        const requiresCouncil = input.executionMode === 'full_auto';
        let councilChanged = false;
        try {
          if (roster.councilRequired !== requiresCouncil) {
            const changed = await call<TeamRoster>('superagent_roster_update',{
              orchestrationId:cfg.id, expectedRevision,
              input:{workerBotIds:[...roster.workerBotIds],
                reviewerBotIds:[...roster.reviewerBotIds],
                councilRequired:requiresCouncil},
            });
            if (changed.orchestrationId !== cfg.id ||
                changed.councilRequired !== requiresCouncil ||
                changed.policyRevision <= expectedRevision) {
              throw new Error('Native Council policy did not confirm the requested mode transition.');
            }
            expectedRevision = changed.policyRevision;
            councilChanged = true;
          }
          const saved = await call<StrategyView>('superagent_strategy_update',{
            orchestrationId:cfg.id, expectedRevision, input,
          });
          if (saved.executionMode !== input.executionMode) {
            throw new Error('Native execution policy did not confirm the selected mode.');
          }
          teamStrategy = saved;
          await reload();
          teamPanel = 'execution';
          feedback = saved.enableBlockers.length
            ? 'Execution mode saved, but the Team remains Off: ' + saved.enableBlockers.join('; ')
            : 'Execution mode saved. Enable the Team to admit new Demands.';
        } catch (cause) {
          if (councilChanged) {
            await reload();
            throw new Error('Council settings were updated safely, but execution mode was not. ' +
              'The Team remains Off; review the current native policy and retry. ' +
              (cause instanceof Error ? cause.message : String(cause)));
          }
          throw cause;
        }
      });
      return;
    }
    if (form.dataset.saForm === 'roster') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id) return;
      const workerBotIds=[cfg.workerBotId,...workerOrder];
      const reviewerBotIds=[...reviewerDraft];
      // Freeze the visible policy before async work; reload may clear the
      // selected roster while the native compare-and-swap request is pending.
      const councilRequired = roleRoster.councilRequired;
      void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_roster_update',{
          orchestrationId:cfg.id, expectedRevision:cfg.policyRevision,
          input:{workerBotIds,reviewerBotIds,councilRequired},
        });
        await reload();
        workspaceView='team';
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
      if (!existing) return;
      // A name/primary edit must not discard additional saved role identities.
      // Native compare-and-swap appends the future-only policy version.
      const input = {
        name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? ''),
        workerBotIds:[
          String(data.get('workerBotId') ?? ''),
          ...(existing.workerBotIds ?? [existing.workerBotId]).slice(1)
            .filter(id => id !== String(data.get('workerBotId') ?? '')),
        ],
        reviewerBotIds:[...(existing.reviewerBotIds ?? [])],
        councilRequired:existing.councilRequired === true,
      };
      void work(async () => {
        await call('superagent_update',{ orchestrationId:existing.id, expectedRevision:existing.policyRevision, input });
        await reload();
      });
    }
    if (form.dataset.saForm === 'guided-builder') {
      const validated = guidedBuilder.validatedInput(bots);
      if (!validated) { render(); return; }
      const {executionMode, ...input} = validated;
      void work(async () => {
        const created = await call<Config>('superagent_create',{ input });
        selected = created.id;
        // Save the versioned execution contract before the Team can be enabled.
        // If this second write fails, the newly created Team remains safely Off.
        const policy = await call<StrategyView>('superagent_strategy_get',{
          orchestrationId:created.id,
        });
        await call('superagent_strategy_update',{
          orchestrationId:created.id, expectedRevision:policy.policyRevision,
          input:{
            priority:policy.settings.priority,
            attemptTimeoutMs:policy.settings.attemptTimeoutMs,
            maxAttemptsPerAssignment:policy.maxAttemptsPerAssignment,
            maxConcurrentTasks:policy.maxConcurrentTasks,
            tokenCeiling:policy.settings.tokenCeiling,
            estimatedCostCeilingMinor:policy.settings.estimatedCostCeilingMinor,
            subagentDelegationEnabled:policy.settings.subagentDelegationEnabled,
            executionMode,
          },
        });
        guidedBuilder.reset();
        workspaceView = 'teams';
        await reload();
        feedback = 'Team and ' + (executionMode === 'full_auto' ? 'Full Auto' : 'With Approval') +
          ' policy saved Off. Enable the Team to start a native Demand.';
      });
      return;
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
      if (!selectedConfig?.enabled || !objective ||
          teamStrategy?.orchestrationId !== selectedConfig.id ||
          teamStrategy.executionMode === 'off') return;
      const key = taskKey;
      initialDemandWritePaths = String(data.get('initialAllowedPaths') ?? '');
      const initialPaths = initialDemandWritePaths.split(/\r\n?|\n/).filter(Boolean);
      if (initialPaths.length > 16 || new Set(initialPaths).size !== initialPaths.length) {
        error = 'Supply at most 16 unique exact project-relative paths.'; render(); return;
      }
      void work(async () => {
        const admitted = await call<Ticket>('superagent_start_demand',{
          orchestrationId:selectedConfig.id, objective, requestKey:key,
          ...(initialPaths.length ? {allowedPaths:initialPaths} : {}),
        });
        if (!admitted.id || admitted.orchestrationId !== selectedConfig.id)
          throw new Error('Native Demand admission returned a mismatched Team or missing Ticket.');
        taskKey = crypto.randomUUID(); demandObjective = '';
        initialDemandWritePaths = '';
        inspectedTicket = admitted.id;
        workspaceView = 'run';
        await reload();
      });
    }
  });
  render();
  void reload();
  return { refresh: reload };
}
