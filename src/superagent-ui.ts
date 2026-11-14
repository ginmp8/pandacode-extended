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
  settings:StrategySettings; maxAttemptsPerAssignment:number; maxConcurrentTasks:number;
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
  state: string; revision: number; updatedAt: number }
interface DemandQuery {
  limit?: number; beforeUpdatedAt?: number; beforeId?: string;
  orchestrationId?: string; stateFilter?: string; searchQuery?: string;
}
interface Preview { wouldCreate: string[]; unchanged: string[]; importedPresetsDisabled: boolean }
interface ImportResult { created: string[]; unchanged: string[]; backupPath?: string }
interface WorkerWriteDispatchRequest extends Record<string, unknown> {
  ticketId: string; expectedRevision: number; allowedPaths: string[];
}
interface WorkerWriteGrant {
  id: string; ticketId: string; assignmentId: string; sessionId: string;
  ticketRevision: number; policyRevision: number; projectId: string; projectRoot: string;
  allowedPaths: string[]; expiresAt: number; revoked: boolean;
}
interface WorkerWriteDispatch { ticket: Ticket; grant: WorkerWriteGrant }
interface PreviewWorkerWriteDispatch {
  previewOnly: true; ticket: Ticket; allowedPaths: string[]; message: string;
}
type WorkerWriteDispatchResult = WorkerWriteDispatch | PreviewWorkerWriteDispatch;
function isPreviewWorkerWriteDispatch(result: WorkerWriteDispatchResult): result is PreviewWorkerWriteDispatch {
  return 'previewOnly' in result && result.previewOnly === true;
}
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
  let teamSearch = '', teamStatusFilter: 'all' | 'enabled' | 'disabled' = 'all';
  let reviewQueue: DemandIndex[] = [], reviewQueueLoaded = false;
  let globalDemandLayout: 'board' | 'list' = 'board';
  let selected = '', busy = false, loading = true, error = '', feedback = '', importText = '';
  let workspaceView: 'teams' | 'runs' | 'review' | 'sources' | 'portable' = 'teams';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let writeGrantTicket = '', writeGrantRevision: number | null = null;
  let writePathsDraft = '', writePathsError = '';
  let commentRepository = '', commentIssueNumber = '', commentDraft = '';
  // Never reuse a potentially private GitHub comment draft in another Ticket.
  // Preserve fields across native refreshes of the *same* Ticket only.
  let commentDraftTicketId = '';
  let issueCommentError = '', issueCommentStatus: IssueCommentStatus | null = null;
  let demandObjective = '', demandMode: 'coordinated' | 'direct' | 'auto' = 'coordinated';
  let workPlan: DemandPlanEvidence | null = null, workPlanLoaded = false;
  let workWriteItemKey = '', workWritePaths = '';
  let subagentTree:NativeSubagentTree|null = null, subagentTreeLoaded = false;
  let subagentDepth=2, subagentPerParent=2, subagentTotal=8;
  let demandPreflight: ManualPreflight | null = null;
  let preflightObjective = '', preflightMode: 'coordinated' | 'direct' = 'coordinated';
  // Persist the user's editable preset draft across asynchronous list refreshes.
  let draftPresetName = '', draftPresetTemplate = '';
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
  let effectiveTeam: EffectiveTeam | null = null;
  let sources: EventSource[] = [], sourcesLoaded = false, decisionSourceId = '';
  let decisions: SourceDecision[] = [], threadLinks: ThreadLink[] = [];
  let sourceIntegrity: SourceIntegrity | null = null;
  let autoStatus: AutoSourceStatus | null = null, autoDailyLimit = 3;
  let linkedDecisions: SourceDecision[] = [], linkedDecisionsLoaded = false;
  const safe = (value: unknown) => escape(String(value ?? ''));
  const fetchDemandPage = (query: DemandQuery) =>
    call<DemandIndex[]>('superagent_all_demands', { query });
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
    // A different Ticket is a different confidentiality boundary. Native
    // refreshes can repaint the workspace without clearing the same draft.
    if (commentDraftTicketId !== inspectedTicket) {
      commentDraftTicketId = inspectedTicket;
      commentRepository = '';
      commentIssueNumber = '';
      commentDraft = '';
      issueCommentError = '';
      issueCommentStatus = null;
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
      (item.enabled ? 'Manual' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button></li>').join('');
    const optionsFor = (botId: string) => bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + (bot.id === botId ? ' selected' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const choices = optionsFor('');
    const chosen = current();
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const writeTicket = tickets.find(ticket => ticket.id === writeGrantTicket);
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
        '</dd></dl><button type="button" data-sa-close-assignment>Close attempt</button></section>'
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
            '</li>').join('') + '</ol>' +
          (workWriteItemKey && workPlan.items.some(item => item.key === workWriteItemKey &&
            item.requiresWrite && item.state === 'pending' && !item.writeAuthorized)
            ? '<form data-sa-form="work-write-scope" aria-label="WorkItem file authorization">' +
              '<p>Native local-user confirmation required for ' + safe(workWriteItemKey) +
              '. Only exact file paths; other tool and GitHub rights remain denied.</p>' +
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
            '. Council evidence is authoritative only for an exact Ticket, artifact and verified native cycle; unbound or stale results cannot approve a task.</p></section>'
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
      '<h4>GitHub event sources</h4><p>Off and authenticated Observe remain available. Read-only Auto for exact issues.opened events requires separate local native confirmation and expires within 24 hours. A source grant never authorizes files, GitHub comments, pushes, merges, or releases.</p>' +
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
      '<form data-sa-form="demand-search" role="search" aria-label="Search native Demands">' +
      '<label>Search objectives<input type="search" name="searchQuery" aria-label="Search Demands by objective" data-sa-demand-search maxlength="128" ' +
      'placeholder="Literal objective fragment" value="' + safe(demandSearchDraft) + '"></label>' +
      '<button type="submit">Search Demands</button></form>' +
      '<p>Native Tickets ' + (demandTeamFilter ? 'for this Team' : 'across Teams') +
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
      safe(chosen.id) + '">' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-duplicate="' + safe(chosen.id) +
      '">Duplicate as new Team</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
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
      '>Worker directly</option>' +
      '<option value="auto"' + (demandMode === 'auto' ? ' selected' : '') +
      '>Supervised Demand · native WorkItems</option></select></label>' +
      '<button type="button" data-sa-preflight>Review native preflight</button>' +
      '<button class="primary-button" type="submit"' + (chosen.enabled ? '' : ' disabled') +
      '>' + (demandMode === 'auto' ? 'Start supervised Demand' : 'Dispatch manually') +
      '</button></form>' + nativePreflight +
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
        (ticket.state === 'queued' ? '<button type="button" data-sa-dispatch="' + safe(ticket.id) + '">Dispatch worker</button>' +
          '<button type="button" data-sa-write-request="' + safe(ticket.id) + '">Request file writes</button>' : '') +
        (ticket.state === 'blocked' ? '<button type="button" data-sa-retry="' + safe(ticket.id) + '">Retry manually</button>' : '') +
        (!['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)
          ? '<button type="button" data-sa-stop="' + safe(ticket.id) + '">Stop</button>' : '') +
        '</li>').join('') || '<li class="sa-empty">No tickets yet.</li>') + '</ul>' +
        (writeTicket && writeGrantRevision !== null
          ? '<section class="sa-write-editor" role="region" aria-label="Scoped write request"><h4>Request scoped file writes</h4>' +
            '<p>Ticket <code>' + safe(writeTicket.id) + '</code> · Revision ' + writeGrantRevision +
            '. Submitting opens a native confirmation that lists these exact files; only that dialog can authorize the worker.</p>' +
            '<form data-sa-form="write-grant"><label>Exact project-relative file paths<textarea data-sa-write-paths name="allowedPaths" rows="5" spellcheck="false">' +
            safe(writePathsDraft) + '</textarea></label><small>Enter one normalized relative path per line. Blank lines are ignored; path text is preserved exactly. Up to 16 files.</small>' +
            (writePathsError ? '<p class="sa-write-path-error" role="alert">' + safe(writePathsError) + '</p>' : '') +
            '<div class="sa-actions"><button type="submit" class="primary-button">Continue to native confirmation</button>' +
            '<button type="button" data-sa-write-cancel>Cancel</button></div></form></section>' : '') +
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
        '<p>Roles reuse existing bot identities, models, harnesses and Skills. Additional Workers remain unavailable for autonomous parallel dispatch; configured Reviewers can be required through native Council.</p>' +
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
              '<label>Optional Council Reviewers (up to 8)<select name="reviewers" multiple size="5">' +
              roleOptions(roleRoster.reviewerBotIds) + '</select></label>' +
              '<label><input type="checkbox" name="councilRequired"' +
              (roleRoster.councilRequired ? ' checked' : '') +
              '> Require independent native Council review before completion</label>' +
              '<p>Enabling required Council needs at least two eligible reviewer bots. Each native Ticket freezes its policy and reviewers.</p>' +
              '<p>A bot may have one role only. Workers must share the primary Worker project. A disabled Team may change roles after all demands settle; active/reviewable work and unresolved effects block native saves.</p>' +
              '<button class="primary-button" type="submit"' +
                (chosen.enabled || tickets.some(ticket => !['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)) ? ' disabled' : '') +
                '>Save configured roles</button></form>' +
              '<p role="status">' + (roleRoster.executionReady ?
                (roleRoster.councilRequired ?
                'Single-worker manual execution with mandatory native Council review.' :
                'Single-worker manual execution available after explicit enable.') :
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
        '<dt>Review policy</dt><dd>' + (chosen.councilRequired ? 'Native Council required' : 'Council Off') + '</dd>' +
        '<dt>Current native support</dt><dd>Manual worker dispatch with explicitly authorized scoped writes on supported platforms; authenticated GitHub Auto only with an exact native grant.</dd></dl>' +
        '<div class="sa-actions"><button type="button" data-sa-enable="' + safe(chosen.id) + '"' +
        (!chosen.enabled && (chosen.workerBotIds?.length ?? 1) > 1
          ? ' disabled title="Native manual admission supports one executable Worker"' : '') +
        '>' + (chosen.enabled ? 'Disable' : 'Enable manual') + '</button>' +
        '<button type="button" data-sa-open-runs>Assign demand / Open runs</button></div>' +
        (!chosen.enabled && (chosen.workerBotIds?.length ?? 1) > 1
          ? '<p role="note">This Team has multiple configured Workers; native manual execution is unavailable until separately scoped multi-worker admission is supported.</p>' : '') +
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
      ([
        ['teams','Teams'],['runs','Demands / Runs'],['review','Review inbox'],
        ['sources','Sources'],['portable','Portable configuration'],
      ] as const).map(([id,label]) =>
        '<button type="button" data-sa-view="' + id + '" aria-current="' +
        (workspaceView === id ? 'page' : 'false') + '">' + label + '</button>').join('') +
      '</nav>';
    host.innerHTML = '<div class="sa-ui">' +
      '<p class="sa-intro">A Team is a reusable SuperAgent. All Run actions below use native Ticket and Mandate records; scope-bound worker writes and task Council are native policy-gated; GitHub Auto is not yet supported.</p>' +
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
          choices + '</select></label></div><button class="primary-button" type="submit"' +
          (bots.length < 2 ? ' disabled' : '') + '>Create disabled preset</button></form></section>' +
           guidedBuilder.render(bots) + overview + teamRoles + strategyPane
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
    delivery=null; deliveryLoaded=false;
    providerChildren=null;providerChildrenLoaded=false;
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
        teamStrategy = null;
        inspectedTicket = ''; threadLinks = [];
        linkedDecisions = []; linkedDecisionsLoaded = false; roleRoster = null; workerOrder = []; effectiveTeam = null;
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
        sourceIntegrity = null;
        autoStatus = null;
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
      if (teamStrategy && !presets.some(config =>
        config.id === teamStrategy?.orchestrationId &&
        config.policyRevision === teamStrategy?.policyRevision)) {
        teamStrategy = null;
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
  // Templates populate editable names only; no bot, tool or task is selected.
  host.addEventListener('change', event => {
    const target = event.target;
    if (target instanceof Element && guidedBuilder.handleChange(target)) {
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
    if (target instanceof HTMLSelectElement && target.name === 'mode' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      demandMode = target.value === 'auto' ? 'auto' :
        target.value === 'direct' ? 'direct' : 'coordinated';
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
    if (target instanceof Element && guidedBuilder.handleInput(target)) return;
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-demand-search')) {
      demandSearchDraft = target.value;
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
    if (target.dataset.saView) {
      const next = target.dataset.saView;
      if (!['teams','runs','review','sources','portable'].includes(next)) return;
      workspaceView = next as typeof workspaceView;
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-view="' + next + '"]')?.focus({preventScroll:true});
      return;
    }
    if (target.hasAttribute('data-sa-preflight')) {
      if (demandMode === 'auto') {
        feedback = 'Native Demand admission will recheck the saved Team, original project, Coordinator, member profiles and checkout leases before dispatch. Planned file writes require separate local confirmation.';
        render();
        return;
      }
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
      inspectedAssignmentId = ''; roleRoster = null; effectiveTeam = null; teamStrategy = null;
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
        // Read both authoritative native views again before creating a new
        // versioned Team. Neither view is an execution grant.
        const [roster, current] = await Promise.all([
          call<TeamRoster>('superagent_roster',{orchestrationId:source.id}),
          call<Config[]>('superagent_list').then(items => items.find(item => item.id === source.id)),
        ]);
        if (!current || current.policyRevision !== source.policyRevision ||
            current.coordinatorBotId !== source.coordinatorBotId ||
            current.workerBotId !== source.workerBotId ||
            roster.orchestrationId !== source.id ||
            roster.policyRevision !== source.policyRevision ||
            roster.coordinatorBotId !== source.coordinatorBotId ||
            roster.workerBotIds[0] !== source.workerBotId ||
            !roster.workerBotIds.length) {
          throw new Error('Source Team changed. Reload before duplicating.');
        }
        const created = await call<Config>('superagent_create', {input:{
          name, coordinatorBotId:roster.coordinatorBotId,
          workerBotId:roster.workerBotIds[0],
          workerBotIds:[...roster.workerBotIds],
          reviewerBotIds:[...roster.reviewerBotIds],
          councilRequired:roster.councilRequired ?? source.councilRequired ?? false,
        }});
        if (created.enabled || created.id === source.id) {
          throw new Error('Native duplication failed to create an independent disabled Team.');
        }
        selected = created.id; workspaceView = 'teams';
        roleRoster = null; workerOrder = []; effectiveTeam = null;
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
    if (target.hasAttribute('data-sa-write-cancel')) {
      writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = ''; writePathsError = '';
      render(); return;
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
    if (target.dataset.saWriteRequest) {
      const ticket = tickets.find(item => item.id === target.dataset.saWriteRequest);
      if (ticket?.state === 'queued') {
        writeGrantTicket = ticket.id; writeGrantRevision = ticket.revision;
        writePathsDraft = ''; writePathsError = ''; error = ''; feedback = '';
        render();
        host.querySelector<HTMLTextAreaElement>('[data-sa-write-paths]')?.focus();
      }
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
      const ticket = tickets.find(ticket => ticket.id === ticketId);
      workWritePaths = String(data.get('allowedPaths') ?? '');
      const paths = workWritePaths.split(/\r\n?|\n/).filter(Boolean);
      if (!plan || !ticket || ticket.state !== 'queued' || plan.state !== 'active' ||
          plan.ticketId !== ticketId || plan.ticketRevision !== ticket.revision ||
          !plan.items.some(item => item.key === itemKey && item.state === 'pending' &&
            item.requiresWrite && !item.writeAuthorized) || paths.length === 0 || paths.length > 16) {
        error = 'A current, pending WorkItem and 1 to 16 exact paths are required.';
        render(); return;
      }
      void work(async () => {
        const result = await call<DemandPlanEvidence>('superagent_authorize_work_item_write', {
          ticketId, expectedRevision:plan.ticketRevision, itemKey, allowedPaths:paths,
        });
        if (result.ticketId !== ticketId || !result.items.some(item =>
          item.key === itemKey && item.requiresWrite && item.writeAuthorized)) {
          throw new Error('Native file permission did not match the selected WorkItem.');
        }
        workWriteItemKey = ''; workWritePaths = '';
        await reload();
        if (inspectedTicket === ticketId) {
          workPlan = await call<DemandPlanEvidence | null>('superagent_demand_plan',{ticketId});
          workPlanLoaded = true;
        }
        feedback = 'Exact WorkItem file scope confirmed natively. The scheduler will admit the eligible Worker without broader tool rights.';
      });
      return;
    }
    if (form.dataset.saForm === 'write-grant') {
      const ticketId = writeGrantTicket;
      const expectedRevision = writeGrantRevision;
      const ticket = tickets.find(item => item.id === ticketId);
      writePathsDraft = String(data.get('allowedPaths') ?? '');
      const paths = writePathsDraft.split(/\r\n?|\n/).filter(path => path.length > 0);
      let pathError = '';
      if (paths.length === 0 || paths.length > 16) {
        pathError = 'Enter between 1 and 16 exact file paths.';
      } else {
        const unique = new Set<string>();
        for (const path of paths) {
          const invalidSegment = path.split('/').some(part => part === '' || part === '.' || part === '..' ||
            part.toLowerCase() === '.git' || [...part].some(character => {
              const code = character.codePointAt(0) ?? 0;
              return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
            }));
          if (new TextEncoder().encode(path).length > 4096 || path.includes('\\') || path.includes('\0') ||
              path.includes(':') || invalidSegment) {
            pathError = 'Use normalized project-relative paths without traversal, .git, drive prefixes, backslashes, or control characters.';
            break;
          }
          if (unique.has(path)) {
            pathError = 'A write scope cannot repeat a file path.';
            break;
          }
          unique.add(path);
        }
      }
      if (pathError) {
        writePathsError = pathError; render(); return;
      }
      if (!ticket || ticket.state !== 'queued' || expectedRevision === null ||
          ticket.revision !== expectedRevision) {
        writePathsError = 'This Ticket changed after you opened the request. Refresh it and review the paths again.';
        render(); return;
      }
      writePathsError = '';
      const request: WorkerWriteDispatchRequest = {
        ticketId, expectedRevision, allowedPaths: paths,
      };
      void work(async () => {
        const result = await call<WorkerWriteDispatchResult>(
          'superagent_dispatch_worker_with_write_grant', request,
        );
        if (isPreviewWorkerWriteDispatch(result)) {
          writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = '';
          await reload();
          feedback = 'Preview only: no native confirmation was shown, no write grant was created, and no worker was dispatched. Requested files for Ticket ' +
            ticketId + ': ' + paths.join(', ') + '.';
          return;
        }
        if (result.ticket.id !== ticketId || result.grant.ticketId !== ticketId ||
            result.grant.allowedPaths.length !== paths.length ||
            result.grant.allowedPaths.some((path, index) => path !== paths[index])) {
          throw new Error('The native response did not match the requested Ticket and file paths.');
        }
        writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = '';
        await reload();
        feedback = 'Native confirmation authorized Ticket ' + ticketId + ' at revision ' +
          expectedRevision + '. Worker state: ' + result.ticket.state + '. Allowed files: ' +
          result.grant.allowedPaths.join(', ') + '. Grant expires ' + activityTime(result.grant.expiresAt) + '.';
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
      };
      if (!['balanced','cost','latency','throughput','quality'].includes(input.priority) ||
          !Number.isInteger(input.maxConcurrentTasks) || input.maxConcurrentTasks < 1 || input.maxConcurrentTasks > 8 ||
          !Number.isInteger(input.maxAttemptsPerAssignment) || input.maxAttemptsPerAssignment < 1 ||
          input.maxAttemptsPerAssignment > 8 || !Number.isInteger(input.attemptTimeoutMs) ||
          input.attemptTimeoutMs < 60000 || input.attemptTimeoutMs > 1800000 ||
          Number.isNaN(input.tokenCeiling) || Number.isNaN(input.estimatedCostCeilingMinor)) {
        error = 'Correct the bounded strategy settings before saving.';
        render(); return;
      }
      void work(async () => {
        const saved = await call<StrategyView>('superagent_strategy_update',{
          orchestrationId:cfg.id, expectedRevision:loaded.policyRevision, input,
        });
        teamStrategy = saved;
        await reload();
        feedback = saved.enableBlockers.length
          ? 'Strategy saved for future Tickets. This Team remains Off: ' + saved.enableBlockers.join('; ')
          : 'Versioned Team strategy saved. Existing Ticket snapshots were not modified.';
      });
      return;
    }
    if (form.dataset.saForm === 'roster') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id) return;
      const workerBotIds=[cfg.workerBotId,...workerOrder];
      const reviewerBotIds=data.getAll('reviewers').map(String);
      // Freeze the visible policy before async work; reload may clear the
      // selected roster while the native compare-and-swap request is pending.
      const councilRequired = typeof roleRoster.councilRequired === 'boolean'
        ? data.has('councilRequired') : undefined;
      void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_roster_update',{
          orchestrationId:cfg.id, expectedRevision:cfg.policyRevision,
          input:{workerBotIds,reviewerBotIds,
            ...(typeof councilRequired === 'boolean' ? {councilRequired} : {})},
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
    if (form.dataset.saForm === 'guided-builder') {
      const input = guidedBuilder.validatedInput(bots);
      if (!input) { render(); return; }
      void work(async () => {
        const created = await call<Config>('superagent_create',{ input });
        guidedBuilder.reset();
        selected = created.id; workspaceView = 'runs';
        await reload();
        feedback = 'Team saved Off; enable Manual only after native role checks.';
      });
      return;
    }
    if (form.dataset.saForm === 'create') {
      const input = { name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? '') };
      void work(async () => {
        if (!input.name || !input.coordinatorBotId || !input.workerBotId ||
          input.coordinatorBotId === input.workerBotId) throw new Error('Choose two different existing bots.');
        const created = await call<Config>('superagent_create',{ input });
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
      const command = data.get('mode') === 'auto' ? 'superagent_start_demand' :
        data.get('mode') === 'direct' ? 'superagent_submit' : 'superagent_submit_coordinated';
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
