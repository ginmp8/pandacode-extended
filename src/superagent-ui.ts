import './superagent.css';
import { escape } from './dom';
import { botAvatarHtml } from './bot-avatars';

// This interface only invokes native policy/authorization decisions. It cannot
// grant tools, infer work, start tasks on import or bypass Council review.
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
interface Bot { id: string; name: string; paused: boolean; avatar?: string; color?: string }
interface Config {
  id: string; name: string; coordinatorBotId: string; workerBotId: string;
  projectId?: string; enabled: boolean; policyRevision: number;
  ticketCount?: number; runningCount?: number; blockedCount?: number; needsAttentionCount?: number;
  queuedCount?: number; completedCount?: number;
  waitingApprovalCount?: number; waitingReviewCount?: number; lastActivityAt?: number;
  workerCount?: number; reviewerCount?: number;
  manualEnableBlocker?: string | null;
  policyEditBlocker?: string | null;
}
interface TeamRoster { orchestrationId:string; policyRevision:number; coordinatorBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; maxWorkerAttempts:number; purpose:string; strategy:string; executionReady:boolean }
interface EffectiveSkill {
  id:string; libraryStatus:string; safetyStatus:string;
  harnessEligibility:string; invocationEvidence:string;
}
interface EffectiveMember {
  role:string; position:number; botId:string; name:string;
  provider:string; model:string; mode:string; reasoningEffort:string; projectId:string;
  paused:boolean; present?:boolean; skills:EffectiveSkill[]; configurationIssues?:string[];
}
interface EffectiveTeam {
  orchestrationId:string; policyRevision:number; enabled:boolean;
  executionReady:boolean; grantsIssued:boolean; manualAdmissionBlockers:string[]; members:EffectiveMember[];
}
interface Ticket { id: string; orchestrationId: string; objective: string; state: string; revision: number; evidence: string | null }
interface DemandIndex { id: string; orchestrationId: string; objective: string;
  state: string; revision: number; updatedAt: number; projectId?: string; archived?: boolean }
interface DemandQuery { limit?: number; beforeUpdatedAt?: number; beforeId?: string;
  orchestrationId?: string; stateFilter?: string; searchQuery?: string; projectId?: string; archived?: boolean }
interface Preview { wouldCreate: string[]; unchanged: string[]; importedPresetsDisabled: boolean; reviewDigest: string }
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
  canStageBlocked?: boolean; allowedTools: string[]; allowedSideEffects: boolean; attemptBudgetMs: number;
}
interface Activity { id: number; kind: string; detail: string; at: number }
interface ActivityPage { ticketId: string; items: Activity[]; hasMore: boolean; nextCursor: number | null }
interface MemberActivityCursor { createdAt: number; assignmentId: string }
interface MemberActivityItem {
  assignmentId: string; ticketId: string; orchestrationId: string; botId: string;
  sessionId: string; role: 'coordinator' | 'worker'; attempt: number;
  assignmentState: string; ticketState: string; createdAt: number; updatedAt: number;
  hasResult: boolean; chatAvailable: boolean;
}
interface MemberActivityPage {
  orchestrationId: string; botId: string; items: MemberActivityItem[];
  assignmentCount: number; demandCount: number; attentionDemandCount: number;
  hasMore: boolean; nextCursor: MemberActivityCursor | null;
}
interface AssignmentChatLink {
  orchestrationId: string; ticketId: string; assignmentId: string;
  botId: string; sessionId: string;
}
interface CouncilObservation {
  nativeStatus: string; nativeRound: number; nativeStale: boolean;
  nativePassVerified: boolean; ticketArtifactBound: boolean;
}
interface AdmittedMemberProfile {
  role: string; position: number; botId: string; projectId: string;
  provider: string; model: string; mode: string; reasoningEffort: string;
  assignedSkillIds: string[];
}
interface AdmittedTeamProfile {
  policyRevision: number; capturedAt: number; members: AdmittedMemberProfile[];
}
interface TicketDetails {
  ticket: Ticket; mandate: Mandate; assignments: Assignment[];
  admittedProfile?: AdmittedTeamProfile | null;
  admittedStrategy?: {policyRevision:number;purpose:string;strategy:string;capturedAt:number} | null;
  currentTeamPolicyRevision?: number;
  teamPolicyChangedSinceAdmission?: boolean;
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
interface LinkedActivityCursor { at: number; sourceId: string; deliveryId: string }
interface LinkedActivityPage { decisions: SourceDecision[]; nextCursor?: LinkedActivityCursor | null }
interface SourceIntegrity {
  sourceId: string; repository: string; repositoryId?: number | null; actorId?: number | null;
  mode: 'off' | 'observe';
  policyRevision: number; cursorKind: string; lastSequence: number;
  receiptCount: number; permittedReceiptCount: number;
  maxReceiptSequence: number; cursorConsistent: boolean;
  receiptCapacityReached: boolean;
  historyGapCursor?: number | null; historyGapDetectedAt?: number | null;
  historyGapReason?: 'cursor_missing' | 'window_unverified' | null;
  historyGapOccurrences?: number;
  historyGapVerifiedAt?: number | null; historyGapVerifiedSequence?: number | null;
  backgroundSelected: boolean; nextAction: string;
  observationalOnly: boolean;
}

type NativeAssignmentChatRoute = {
  teamId: string; ticketId: string; assignmentId: string; botId: string; sessionId: string;
};
type OpenNativeAssignmentChat = (route: NativeAssignmentChatRoute) => Promise<void>;

export function mountSuperagent(
  host: HTMLElement, call: Call, openAssignmentChat?: OpenNativeAssignmentChat,
): { refresh(): Promise<void> } {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let allDemands: DemandIndex[] = [], allDemandsLoaded = false, allDemandsHasMore = false;
  let demandTeamFilter = '', demandProjectFilter = '', demandStateFilter = '', demandSearch = '', demandSearchDraft = '';
let demandArchived = false;
  let teamSearch = '', teamStatusFilter: 'all' | 'enabled' | 'disabled' = 'all';
  let reviewQueue: DemandIndex[] = [], reviewQueueLoaded = false, reviewQueueHasMore = false;
  let reviewTeamFilter = '';
  // Human rejection is scoped to one native review receipt, never to a board label.
  let globalRejection: {id:string; teamId:string; revision:number; evidence:string | null; filter:string} | null = null;
  let globalAcceptance: {id:string; teamId:string; revision:number; evidence:string; filter:string} | null = null;
  let globalDemandLayout: 'board' | 'list' = 'board';
  let selected = '', busy = false, loading = true, error = '', feedback = '', importText = '';
  let workspaceView: 'teams' | 'runs' | 'review' | 'sources' | 'portable' = 'teams';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let demandObjective = '', demandMode: 'coordinated' | 'direct' = 'coordinated';
  let selectedDemandWorkerId = '';
  let demandPreflight: ManualPreflight | null = null;
  let preflightObjective = '', preflightMode: 'coordinated' | 'direct' = 'coordinated';
  // Persist the user's editable preset draft across asynchronous list refreshes.
  let draftPresetName = '', draftPresetTemplate = '', draftWorkerAttemptLimit = 2;
  let draftPresetPurpose = '', draftPresetStrategy = 'balanced';
  // Team roles are an unsaved draft until native create confirms the exact roster.
  let draftCoordinatorBotId = '', draftWorkerBotId = '';
  let draftAdditionalWorkerIds: string[] = [], draftReviewerBotIds: string[] = [];
  let draftMembersExpanded = false;
  let rejectingTicket = '', inspectedTicket = '';
  let inspectedAssignmentId = '';
  let activityMode: 'all' | 'compact' | 'json' = 'all';
  let activityFilter = '';
  let olderActivity: Activity[] = [], olderActivityTicketId = '', olderActivityHasMore = true;
  let ticketDetails: TicketDetails | null = null;
  let effectReceipts: EffectReceipt[] = [], effectReceiptsLoaded = false;
  let runGraph: RunGraph | null = null, runGraphLoaded = false;
  let roleRoster: TeamRoster | null = null;
  // Draft order is committed only by the native revision-guarded roster update.
  let workerOrder: string[] = [];
  // Keep unsaved reviewer choices across Worker selection and reorder renders.
  let reviewerDraft: string[] = [];
  let effectiveTeam: EffectiveTeam | null = null;
  let memberActivityTeamId = '', memberActivityBotId = '';
  let memberActivityItems: MemberActivityItem[] = [];
  let memberActivityCounts: Pick<MemberActivityPage, 'assignmentCount' | 'demandCount' | 'attentionDemandCount'> | null = null;
  let memberActivityHasMore = false, memberActivityCursor: MemberActivityCursor | null = null;
  const clearMemberActivity = () => {
    memberActivityTeamId = ''; memberActivityBotId = '';
    memberActivityItems = []; memberActivityCounts = null;
    memberActivityHasMore = false; memberActivityCursor = null;
  };
  let sources: EventSource[] = [], sourcesLoaded = false, decisionSourceId = '';
  let decisions: SourceDecision[] = [], threadLinks: ThreadLink[] = [];
  let sourceIntegrity: SourceIntegrity | null = null;
  let linkedDecisions: SourceDecision[] = [], linkedDecisionsLoaded = false;
  let linkedDecisionsHasMore = false;
  const safe = (value: unknown) => escape(String(value ?? ''));
  // SQLite lower() folds ASCII only. Match the native literal-substring filter
  // without locale-sensitive case conversion that would reject valid UTF-8.
  const nativeCaseFold = (value: string): string =>
    value.replace(/[A-Z]/g, character => character.toLowerCase());
  const isDemandStateMatch = (filter: string | undefined, state: string): boolean =>
    !filter || filter === state ||
      (filter === 'needs_attention' &&
        ['blocked','failed','interrupted','inconclusive'].includes(state));
  const fetchDemandPage = async (query: DemandQuery): Promise<DemandIndex[]> => {
    const nativeQuery: DemandQuery = demandArchived ? {...query, archived:true} : query;
    const page = await call<DemandIndex[]>('superagent_all_demands', {query:nativeQuery});
    // A first page is untrusted until its native filter scope and identities
    // match this request. Never expose foreign rows or their Stop controls.
    // Continuations additionally validate their cursor against the last row.
    if (!query.beforeId) {
      const seen = new Set<string>();
      const searchNeedle = query.searchQuery ? nativeCaseFold(query.searchQuery.trim()) : '';
      if (!Array.isArray(page) || page.length > (query.limit ?? 200) ||
          page.some((item, index) => {
            if (!item || typeof item.id !== 'string' ||
                !/^[A-Za-z0-9_-]{1,128}$/.test(item.id) ||
                typeof item.orchestrationId !== 'string' ||
                !/^[A-Za-z0-9_-]{1,128}$/.test(item.orchestrationId) ||
                !Number.isSafeInteger(item.updatedAt) || item.updatedAt <= 0 ||
                !Number.isSafeInteger(item.revision) || item.revision < 0 ||
                typeof item.objective !== 'string' ||
                typeof item.state !== 'string' ||
                (query.searchQuery && !nativeCaseFold(item.objective).includes(searchNeedle)) ||
                (index > 0 && !(item.updatedAt < page[index - 1].updatedAt ||
                  (item.updatedAt === page[index - 1].updatedAt && item.id < page[index - 1].id))) ||
                (query.orchestrationId && item.orchestrationId !== query.orchestrationId) ||
                !isDemandStateMatch(query.stateFilter, item.state) ||
                (demandArchived ? item.archived !== true : item.archived === true) ||
                (query.projectId && item.projectId !== query.projectId) ||
                seen.has(item.id)) return true;
            seen.add(item.id);
            return false;
          })) {
        throw new Error('Native Demand index returned invalid, duplicate or out-of-scope records; refresh the index.');
      }
    } else if (!Array.isArray(page) || page.length > (query.limit ?? 200) ||
        page.some(item => !item || typeof item.id !== 'string' ||
          !/^[A-Za-z0-9_-]{1,128}$/.test(item.id) ||
          typeof item.orchestrationId !== 'string' ||
          !/^[A-Za-z0-9_-]{1,128}$/.test(item.orchestrationId) ||
          typeof item.objective !== 'string' ||
          typeof item.state !== 'string' || !item.state ||
          !Number.isSafeInteger(item.revision) || item.revision < 0 ||
          !Number.isSafeInteger(item.updatedAt) || item.updatedAt <= 0 ||
          (item.projectId !== undefined && typeof item.projectId !== 'string') ||
          (demandArchived ? item.archived !== true : item.archived === true))) {
      // Continuations get ordering, overlap and filter checks at the caller,
      // but still require valid native Ticket metadata before exposing actions.
      throw new Error('Native Demand continuation has malformed Ticket identity or revision; refresh the index.');
    }
    return page;
  };
  // Team-scoped review rows come from native SQLite before LIMIT; the
  // renderer validates response identity before exposing Open Demand controls.
  const fetchReviewPage = async (teamId: string, cursor?: DemandIndex): Promise<DemandIndex[]> => {
    const page = await call<DemandIndex[]>(
      teamId ? 'superagent_team_review_inbox' : 'superagent_review_inbox',
      {limit: 200, ...(teamId ? {orchestrationId: teamId} : {}),
        ...(cursor ? {beforeUpdatedAt: cursor.updatedAt, beforeId: cursor.id} : {})},
    );
    const known = new Set((cursor ? reviewQueue : []).map(item => item.id));
    const descends = (newer: DemandIndex, older: DemandIndex) =>
      older.updatedAt < newer.updatedAt ||
      (older.updatedAt === newer.updatedAt && older.id < newer.id);
    if (!Array.isArray(page) || page.length > 200 ||
        page.some((item, index) => {
          if (!item || typeof item.id !== 'string' ||
              !/^[A-Za-z0-9_-]{1,128}$/.test(item.id) ||
              typeof item.orchestrationId !== 'string' ||
              !/^[A-Za-z0-9_-]{1,128}$/.test(item.orchestrationId) ||
              (teamId && item.orchestrationId !== teamId) ||
              !['waiting_approval','waiting_review'].includes(item.state) ||
              typeof item.objective !== 'string' ||
              !Number.isSafeInteger(item.revision) || item.revision < 0 ||
              !Number.isSafeInteger(item.updatedAt) || item.updatedAt <= 0 ||
              known.has(item.id) ||
              ((cursor || index > 0) &&
                !descends(index === 0 ? cursor! : page[index - 1], item))) return true;
          known.add(item.id);
          return false;
        })) {
      throw new Error('Native review inbox returned invalid, overlapping or out-of-scope decisions; refresh before acting.');
    }
    return page;
  };
  // Require a native Team+bot match on EVERY page before exposing chat routes.
  // A reused bot ID cannot make another Team's assignments visible.
  const fetchMemberActivity = async (
    teamId: string, botId: string, before?: MemberActivityCursor,
  ): Promise<MemberActivityPage> => {
    const page = await call<MemberActivityPage>('superagent_member_activity_page', {
      orchestrationId: teamId, botId, limit: 25, ...(before ? {before} : {}),
    });
    const ids = new Set<string>();
    const ordered = (older: MemberActivityCursor, newer: MemberActivityCursor): boolean =>
      older.createdAt < newer.createdAt ||
      (older.createdAt === newer.createdAt && older.assignmentId < newer.assignmentId);
    if (!page || page.orchestrationId !== teamId || page.botId !== botId ||
        !Array.isArray(page.items) || page.items.length > 25 ||
        !Number.isSafeInteger(page.assignmentCount) || page.assignmentCount < page.items.length ||
        !Number.isSafeInteger(page.demandCount) || page.demandCount < 0 ||
        page.demandCount > page.assignmentCount ||
        !Number.isSafeInteger(page.attentionDemandCount) || page.attentionDemandCount < 0 ||
        page.attentionDemandCount > page.demandCount ||
        typeof page.hasMore !== 'boolean' ||
        page.items.some((entry, index) => {
          if (!entry || entry.orchestrationId !== teamId || entry.botId !== botId ||
              typeof entry.ticketId !== 'string' || !entry.ticketId ||
              typeof entry.assignmentId !== 'string' || !entry.assignmentId ||
              typeof entry.sessionId !== 'string' || !entry.sessionId ||
              !['worker','coordinator'].includes(entry.role) ||
              !Number.isSafeInteger(entry.attempt) || entry.attempt < 1 ||
              !Number.isSafeInteger(entry.createdAt) || entry.createdAt <= 0 ||
              !Number.isSafeInteger(entry.updatedAt) || entry.updatedAt <= 0 ||
              typeof entry.ticketState !== 'string' ||
              typeof entry.assignmentState !== 'string' ||
              typeof entry.hasResult !== 'boolean' || typeof entry.chatAvailable !== 'boolean' ||
              ids.has(entry.assignmentId)) return true;
          ids.add(entry.assignmentId);
          const newer = index === 0 ? before : page.items[index - 1];
          return !!newer && !ordered(entry, newer);
        }) ||
        (page.hasMore &&
          (!page.items.length || !page.nextCursor ||
            page.nextCursor.createdAt !== page.items[page.items.length - 1].createdAt ||
            page.nextCursor.assignmentId !== page.items[page.items.length - 1].assignmentId)) ||
        (!page.hasMore && page.nextCursor !== null)) {
      throw new Error('Native Team member activity returned invalid or out-of-scope history.');
    }
    return page;
  };
  const current = () => presets.find(item => item.id === selected);
  // Native eligibility must agree between the directory and Team inspector.
  // Neither view issues grants or overrides provider/Council policy.
  const teamActivationBlocker = (team: Config): string | null =>
    team.enabled ? null : team.manualEnableBlocker ||
      ((team.reviewerCount ?? 0) > 0
        ? 'Configured multi-member Teams cannot run without native scoped admission.'
        : null);
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
    const nativeCount = (n: number | undefined, label: string): string =>
      typeof n === 'number' && Number.isSafeInteger(n) && n >= 0 ? n + ' ' + label : label + ' unavailable';
    const roleSummary = (item:Config): string =>
      typeof item.workerCount === 'number' && typeof item.reviewerCount === 'number' &&
      Number.isInteger(item.workerCount) && Number.isInteger(item.reviewerCount) &&
      item.workerCount >= 1 && item.reviewerCount >= 0
        ? '1 Coordinator · ' + item.workerCount + ' Workers · ' + item.reviewerCount + ' Reviewers'
        : 'Member counts unavailable';
    // The native Ticket's admitted project can outlive changes to its Team.
    // Keep historical projects found in loaded pages and exact manual filters available.
    const projectChoices = [...new Set([
      ...presets.map(item => item.projectId),
      ...allDemands.map(item => item.projectId),
      demandProjectFilter,
    ].filter((id): id is string => typeof id === 'string' && id.length > 0))].sort();
    const visibleTeams = presets.filter(item =>
      item.name.toLowerCase().includes(teamSearch.trim().toLowerCase()) &&
      (teamStatusFilter === 'all' || item.enabled === (teamStatusFilter === 'enabled')));
    const list = visibleTeams.map(item =>
      '<li class="sa-item"><div><strong>' + safe(item.name) + '</strong><small>' +
      safe(botName(item.coordinatorBotId)) + ' / ' + safe(botName(item.workerBotId)) +
      '</small><small class="sa-role-counts" aria-label="Configured member counts">' +
      safe(roleSummary(item)) + '</small><small class="sa-counts" aria-label="Task counts">' +
      safe(nativeCount(item.ticketCount,'tasks')) + ' · ' +
      safe(nativeCount(item.runningCount,'running')) + ' · ' +
      safe(nativeCount(item.blockedCount,'blocked')) + '</small>' +
      '<button type="button" class="sa-attention-counts" data-sa-attention-team="' +
      safe(item.id) + '" aria-label="Show Demands needing attention for ' + safe(item.name) + '"' +
      (!Number.isSafeInteger(item.needsAttentionCount) || (item.needsAttentionCount ?? 0) < 1
        ? ' disabled' : '') + '>' +
      safe(nativeCount(item.needsAttentionCount,'needing attention')) + '</button>' +
      '<small class="sa-lifecycle-counts" aria-label="Native queued and completed Demand counts">' +
      safe(nativeCount(item.queuedCount,'queued')) + ' · ' +
      safe(nativeCount(item.completedCount,'completed')) + '</small>' +
      '<small class="sa-review-counts" aria-label="Native pending review counts">' +
      (Number.isSafeInteger(item.waitingApprovalCount) && Number.isSafeInteger(item.waitingReviewCount) &&
       (item.waitingApprovalCount ?? -1) >= 0 && (item.waitingReviewCount ?? -1) >= 0
        ? safe(item.waitingApprovalCount) + ' awaiting human approval · ' +
          safe(item.waitingReviewCount) + ' awaiting Council (not approved)'
        : 'Native review counts unavailable') + '</small>' +
      '<button type="button" data-sa-review-team="' + safe(item.id) +
      '" aria-label="Review pending decisions for ' + safe(item.name) +
      '">Open review inbox</button>' +
      '<small class="sa-last-activity">Last activity: ' + safe(activityTime(item.lastActivityAt)) +
      '</small></div><span>' +
      (item.enabled ? 'Manual' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button>' +
      '<button type="button" data-sa-enable="' + safe(item.id) +
      '" aria-label="' + (item.enabled ? 'Deactivate ' : 'Activate ') +
      safe(item.name) + ' from Teams directory"' +
      (teamActivationBlocker(item) ? ' disabled' : '') + '>' +
      (item.enabled ? 'Disable Team' : 'Enable manual') + '</button>' +
      (teamActivationBlocker(item)
        ? '<small role="status">Manual activation unavailable: ' +
          safe(teamActivationBlocker(item)) + '</small>' : '') +
      '<button type="button" data-sa-assign-team="' + safe(item.id) +
      '" aria-label="Assign demand to ' +
      safe(item.name) + '">Assign demand</button></li>').join('');
    const optionsFor = (selected: string | string[]) => {
      const selectedIds = Array.isArray(selected) ? selected : [selected];
      return bots.map(bot => '<option value="' + safe(bot.id) + '"' +
        (bot.paused ? ' disabled' : '') + (selectedIds.includes(bot.id) ? ' selected' : '') +
        '>' + safe(bot.name) + (bot.paused ? ' (paused)' : '') + '</option>').join('');
    };
    // Native bot identities and shared PandaCode avatars; configuration only.
    // Checkbox names are submitted through the existing native roster API.
    const roleChoices = (name: 'additionalWorkers' | 'reviewers', title: string,
      selectedIds: string[], excludedIds: string[]) =>
      '<fieldset class="sa-member-picker" aria-label="' + safe(title) + '">' +
      '<legend>' + safe(title) + '</legend><div class="sa-member-choice-list">' +
      (bots.filter(bot => !excludedIds.includes(bot.id)).map(bot =>
        '<label class="sa-member-choice"><input type="checkbox" name="' + name +
        '" value="' + safe(bot.id) + '"' +
        (selectedIds.includes(bot.id) ? ' checked' : '') +
        (bot.paused ? ' disabled' : '') + '>' +
        botAvatarHtml(bot.avatar ?? '', bot.name, '', bot.color ?? '') +
        '<span><strong>' + safe(bot.name) + '</strong><small>' +
        (bot.paused ? 'Paused · unavailable' : 'Existing bot') +
        '</small></span></label>').join('') ||
        '<p>No other existing bots available for this role.</p>') +
      '</div></fieldset>';
    const chosen = current();
    // The native directory inspects all durable Demands, not just the latest page.
    const policyEditBlocker = chosen?.policyEditBlocker ||
      (chosen?.enabled ? 'Disable the Team before changing its policy.' :
        tickets.some(ticket => !['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state))
          ? 'Active or reviewable Demands prevent Team policy edits.' : null);
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const latestActivity = ticketDetails?.activity ?? [];
    const newestIds = new Set(latestActivity.map(entry => entry.id));
    const activityTrail = olderActivityTicketId === ticketDetails?.ticket.id
      ? [...latestActivity, ...olderActivity.filter(entry => !newestIds.has(entry.id))]
      : latestActivity;
    const activity = activityTrail.filter(entry => {
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
        '<dt>Session</dt><dd><code>' + safe(attempt.sessionId) + '</code>' +
        (openAssignmentChat
          ? ' <button type="button" data-sa-open-assignment-chat="' + safe(attempt.id) +
            '">Open native chat</button>' : ' · Native chat navigation unavailable') +
        '</dd>' +
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
        '<section class="sa-admitted-profile" role="region" aria-label="Admitted Team configuration">' +
        '<h5>Team configuration at Demand admission</h5>' +
        '<p>Historical native configuration, not provider capability, a tool grant, Skill invocation or Council approval. ' +
        'Editing the reusable Team cannot rewrite this Demand.</p>' +
        (ticketDetails.admittedStrategy
          ? '<p>Admitted Team strategy: ' + safe(ticketDetails.admittedStrategy.strategy) +
            ' · Purpose: ' + safe(ticketDetails.admittedStrategy.purpose || 'not specified') +
            ' · Policy revision ' + safe(ticketDetails.admittedStrategy.policyRevision) + '</p>'
          : '<p>Historical Team strategy unavailable for this legacy Demand.</p>') +
        (ticketDetails.admittedProfile &&
         ticketDetails.admittedProfile.policyRevision === ticketDetails.mandate.policyRevision
          ? '<p>Admitted policy revision ' + ticketDetails.admittedProfile.policyRevision +
            ' · Captured ' + safe(activityTime(ticketDetails.admittedProfile.capturedAt)) +
            ' · Current Team policy revision ' +
            safe(ticketDetails.currentTeamPolicyRevision ?? 'unavailable') + ' · ' +
            (ticketDetails.teamPolicyChangedSinceAdmission === true
              ? 'Team policy changed after admission; original Demand remains pinned.'
              : ticketDetails.teamPolicyChangedSinceAdmission === false
                ? 'Team policy revision unchanged since admission.'
                : 'Native policy drift comparison unavailable.') + '</p>' +
            '<ol>' + ticketDetails.admittedProfile.members.map(member =>
              '<li><strong>' + safe(member.role) + ' ' + (member.position + 1) +
              ' · Bot ID ' + safe(member.botId) + '</strong>' +
              ' · Project ID ' + safe(member.projectId) +
              ' · Harness ' + safe(member.provider) + ' · Model ' + safe(member.model) +
              ' · Mode ' + safe(member.mode) +
              ' · Reasoning ' + safe(member.reasoningEffort || 'not specified') +
              ' · Assigned Skills (not invocation evidence): ' +
              safe(member.assignedSkillIds?.join(', ') || 'none') + '</li>').join('') + '</ol>'
          : '<p>Legacy or inconsistent admission snapshot unavailable; no role, model or Skill provenance inferred.</p>') +
        '</section>' +
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
        '<p class="sa-activity-source" aria-live="polite">' + activity.length +
        ' matching of ' + activityTrail.length + ' loaded native events · ' +
        'latest-50 Ticket snapshot plus explicitly requested older pages. ' +
        'Native audit ID order; no inferred status.</p>' +
        auditContent +
        (latestActivity.length === 50 && olderActivityHasMore
          ? '<button type="button" data-sa-load-older-activity>Load older native events</button>'
          : '') +
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
          '</ol>' : '') +
         (linkedDecisionsLoaded && linkedDecisionsHasMore
           ? '<button type="button" data-sa-load-older-linked-activity>Load older linked activity</button>' : '') +
         '</section>' +
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
          (sources.some(src => src.mode === 'observe' && src.eventKind !== 'check_run.completed')
            ? '<button type="button" data-sa-source-poll-team>Poll all enabled GitHub Events sources</button>'
            : '') +
          '<ul class="sa-tickets sa-source-list">' +
          (sourceRows || '<li class="sa-empty">No event sources configured.</li>') + '</ul>' +
          (sourceIntegrity ? '<section role="region" aria-label="Source integrity">' +
            '<h5>Native source integrity</h5><p>' +
            safe(sourceIntegrity.repository) + ' · ' + safe(sourceIntegrity.mode) +
            ' · Revision ' + safe(sourceIntegrity.policyRevision) + '</p>' +
            '<p>GitHub repository ID: ' + safe(sourceIntegrity.repositoryId ?? 'Not attested') +
            ' · GitHub Events actor ID: ' + safe(sourceIntegrity.actorId ?? 'Not attested') + '</p>' +
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
            '<p role="status">Periodic GitHub Events Observe: ' +
            (sourceIntegrity.backgroundSelected === true
              ? 'selected for this enabled Team (no guarantee of complete delivery)'
              : 'not selected; enable the Team and its Events source for periodic polling') +
            '. Explicit authenticated inspection never grants tools or starts Demands.</p>' +
            (sourceIntegrity.historyGapCursor
              ? (sourceIntegrity.historyGapVerifiedAt
                ? '<p>Historical GitHub continuity warning at cursor ' +
                  safe(sourceIntegrity.historyGapCursor) + ' (' +
                  safe(sourceIntegrity.historyGapReason === 'cursor_missing'
                    ? 'stored cursor absent from bounded feed' : 'bounded feed exhausted') +
                  ') was verified by a later bounded feed through event ' +
                  safe(sourceIntegrity.historyGapVerifiedSequence ?? 'unknown') +
                  '. Original incident retained; no missing events were invented or replayed.</p>'
                : '<p role="alert">GitHub history continuity not verified since cursor ' +
                  safe(sourceIntegrity.historyGapCursor) + ' (' +
                  safe(sourceIntegrity.historyGapReason === 'cursor_missing'
                    ? 'stored cursor absent from bounded feed' : 'bounded feed exhausted') +
                  '). Observed ' + safe(sourceIntegrity.historyGapOccurrences ?? 1) +
                  ' time(s). Operator reconciliation required; no lost events have been inferred or replayed.</p>')
              : '') +
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
    const demandCard = (demand: DemandIndex) => {
      const team = presets.find(item => item.id === demand.orchestrationId);
      return '<li class="sa-demand-card"><div><strong>' + safe(demand.objective) +
        '</strong><small>' + safe(team?.name ?? 'Unknown Team') +
        ' · ' + safe(demand.state) + ' · Revision ' +
        demand.revision + ' · Project ' + safe(demand.projectId ?? 'Unknown') +
        ' · Updated ' + safe(activityTime(demand.updatedAt)) +
        '</small></div><button type="button" data-sa-open-demand="' + safe(demand.id) +
        '">Open demand</button>' +
        (demand.state === 'blocked'
          ? !team?.enabled
            ? '<small>Manual retry unavailable: Team disabled or missing.</small>'
            : (team.reviewerCount ?? 0) > 0
              ? '<small>Manual retry unavailable: configured Council requires native review.</small>'
              : '<button type="button" data-sa-retry-global-demand="' + safe(demand.id) +
                '">Retry demand</button>'
          : '') +
        (demandArchived
          ? '<button type="button" data-sa-restore-global-demand="' + safe(demand.id) +
            '">Restore Demand</button>'
          : ['completed','failed','stopped','interrupted','inconclusive'].includes(demand.state)
            ? '<button type="button" data-sa-archive-global-demand="' + safe(demand.id) +
              '">Delete Demand</button>'
            : '<button type="button" data-sa-stop-global-demand="' + safe(demand.id) +
              '">Cancel Demand</button>') + '</li>';
    };
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
      '<label class="sa-demand-project-filter">Filter by Project<select aria-label="Filter Demands by Project" data-sa-demand-project-filter>' +
      '<option value="">All Projects</option>' +
      projectChoices.map(id => '<option value="' + safe(id) + '"' +
        (demandProjectFilter === id ? ' selected' : '') + '>' + safe(id) + '</option>').join('') +
      '</select></label>' +
      '<form data-sa-form="demand-project-id" role="search" aria-label="Filter by exact admitted project">' +
      '<label>Historical project ID<input type="text" name="projectId" maxlength="128" ' +
      'aria-label="Exact admitted Project ID" value="' + safe(demandProjectFilter) + '"></label>' +
      '<button type="submit">Apply project filter</button></form>' +
      '<label>Visibility<select aria-label="Filter Demands by Visibility" data-sa-demand-visibility-filter>' +
      '<option value="active"' + (!demandArchived ? ' selected' : '') + '>Active</option>' +
      '<option value="archived"' + (demandArchived ? ' selected' : '') + '>Archived</option></select></label>' +
      '<label class="sa-demand-state-filter">Filter by State<select aria-label="Filter Demands by State" data-sa-demand-state-filter>' +
      '<option value="">All States</option>' +
      (['queued','running','waiting_approval','waiting_review','blocked','completed',
        'failed','stopped','interrupted','inconclusive','needs_attention'] as const)
        .map(state => '<option value="' + state + '"' +
          (demandStateFilter === state ? ' selected' : '') + '>' +
          state.split('_').join(' ') + '</option>').join('') +
      '</select></label>' +
      '<form data-sa-form="demand-search" role="search" aria-label="Search native Demands">' +
      '<label>Search objectives<input type="search" name="searchQuery" maxlength="128" ' +
      'aria-label="Search Demands by objective" data-sa-demand-search value="' +
      safe(demandSearchDraft) + '"></label>' +
      '<button type="submit">Search Demands</button></form>' +
      '<p>Native Tickets ' + (demandTeamFilter ? 'for this Team' : 'across Teams') +
      (demandProjectFilter ? ' in native project ' + safe(demandProjectFilter) : '') +
      (demandStateFilter ? ' in state ' + safe(demandStateFilter.split('_').join(' ')) : '') +
      (demandSearch ? ' matching objective "' + safe(demandSearch) + '"' : '') +
      (demandArchived ? ' in archived history' : ' in active history') +
      ', loaded 200 at a time in newest-first order. Search filters native records ' +
      'before pagination (ASCII case-insensitive, literal text). Board counts cover ' +
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
    const preflightCurrent = Boolean(chosen && demandPreflight &&
      demandPreflight.orchestrationId === chosen.id &&
      demandPreflight.policyRevision === chosen.policyRevision &&
      preflightObjective === demandObjective.trim() && preflightMode === demandMode &&
      demandPreflight.selectedRole === (demandMode === 'coordinated' ? 'coordinator' : 'worker') &&
      demandPreflight.selectedBotId === (demandMode === 'coordinated'
        ? chosen.coordinatorBotId :
          ((chosen.workerCount ?? 1) > 1 ? selectedDemandWorkerId : chosen.workerBotId)));
    // A preview is a read-only observation, never a grant or authorization.
    const manualAdmissionReady = Boolean(chosen?.enabled && preflightCurrent &&
      demandPreflight && (demandPreflight.eligible || demandPreflight.canStageBlocked === true) &&
      demandPreflight.allowedTools.length === 0 && !demandPreflight.allowedSideEffects);
    const nativePreflight = preflightCurrent && demandPreflight
      ? '<section class="sa-pane" role="region" aria-label="Native demand preflight">' +
        '<h4>Native demand preflight</h4><p role="status">' +
        (demandPreflight.eligible ? 'Eligible for manual read-only admission now.' :
          demandPreflight.canStageBlocked
            ? 'Busy Worker: a blocked Demand may be durably queued for explicit retry.'
            : 'Not eligible for immediate admission: ' + safe(demandPreflight.reason)) + '</p>' +
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
    // A live native directory projection explains disabled controls. Missing
    // fields from legacy previews retain the conservative multi-member fence.
    const enableBlocker = chosen ? teamActivationBlocker(chosen) : null;
    const active = chosen ? '<section class="sa-pane" aria-label="Selected orchestration"><header><h3>' +
      safe(chosen.name) + '</h3><div class="sa-actions"><button type="button" data-sa-enable="' +
      safe(chosen.id) + '"' +
      (enableBlocker ? ' disabled aria-describedby="sa-native-enable-blocker"' : '') +
      '>' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-duplicate="' + safe(chosen.id) +
      '">Duplicate as new Team</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused</button></div></header>' +
      (enableBlocker ? '<p id="sa-native-enable-blocker" role="status">Manual enable blocked: ' +
        safe(enableBlocker) + ' No task or grant was started.</p>' : '') +
      '<form data-sa-form="edit"><label>Preset title<input name="name" required maxlength="120" value="' + safe(chosen.name) + '"></label>' +
      '<div class="sa-grid"><label>Edit coordinator<select name="coordinatorBotId" required>' +
      optionsFor(chosen.coordinatorBotId) + '</select></label>' +
      '<label>Edit worker<select name="workerBotId" required>' + optionsFor(chosen.workerBotId) +
      '</select></label></div><button class="primary-button" type="submit"' +
      (policyEditBlocker ? ' disabled aria-describedby="sa-policy-edit-blocker"' : '') +
      '>Save preset</button></form>' +
      (policyEditBlocker ? '<p id="sa-policy-edit-blocker" role="status">Team policy edit blocked: ' +
        safe(policyEditBlocker) + '</p>' : '') +
      '<p>Policy revision ' + chosen.policyRevision + ' · Native read-only permission until separately authorized.</p>' +
      '<p>Disabled Teams may change primary roles after settled Demands only when historical native role snapshots are complete and no extra members or GitHub sources remain. Otherwise duplicate the Team.</p>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required>' +
      safe(demandObjective) + '</textarea></label>' +
      '<label>Dispatch<select name="mode"><option value="coordinated"' +
      (demandMode === 'coordinated' ? ' selected' : '') + '>Coordinator, then worker</option>' +
      '<option value="direct"' + (demandMode === 'direct' ? ' selected' : '') +
      '>Worker directly</option></select></label>' +
      ((chosen.workerCount ?? 1) > 1
        ? (roleRoster?.orchestrationId !== chosen.id
          ? '<p>Load the native Team roster before choosing an executor.</p>' +
            '<button type="button" data-sa-role-load>Load configured Workers</button>'
          : '<label>Worker for this Demand<select name="demandWorkerId" required>' +
            '<option value="">Choose a configured Worker</option>' +
            roleRoster.workerBotIds.map(id =>
              '<option value="' + safe(id) + '"' +
              (selectedDemandWorkerId === id ? ' selected' : '') + '>' +
              safe(botName(id)) + '</option>').join('') +
            '</select></label><p>Only explicitly selected direct read-only Workers are supported. ' +
            'Reviewer-backed and coordinated multi-Worker execution remain unavailable.</p>')
        : '') +
      '<button type="button" data-sa-preflight>Review native preflight</button>' +
      '<button class="primary-button" type="submit"' + (manualAdmissionReady ? '' : ' disabled') +
      '>' + (demandPreflight?.canStageBlocked && preflightCurrent
        ? 'Submit busy Demand' : 'Dispatch manually') + '</button></form>' +
      (chosen.enabled && !manualAdmissionReady
        ? '<p role="status">Review an eligible native read-only preflight before dispatch. ' +
          'Changes to the objective, role or Team policy require a new review.</p>'
        : '') + nativePreflight +
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
    const memberActivityView = chosen && effectiveTeam &&
      memberActivityTeamId === chosen.id && memberActivityBotId &&
      effectiveTeam.members.some(member => member.botId === memberActivityBotId)
      ? '<section class="sa-pane" role="region" aria-label="Team member activity">' +
        '<h5>Team activity: ' + safe(botName(memberActivityBotId)) + '</h5>' +
        '<p>Actual native Assignment history for this Team only. A recorded result is not verified delivery or Council approval.</p>' +
        '<p class="sa-member-summary" role="status">' +
        (memberActivityCounts
          ? '<strong>' + memberActivityCounts.assignmentCount + ' assignments</strong> · ' +
            '<strong>' + memberActivityCounts.demandCount + ' demands</strong> · ' +
            '<strong>' + memberActivityCounts.attentionDemandCount + ' needs attention</strong> ' +
            '(entire recorded Team history)'
          : 'Loading native member summary') + '</p>' +
        '<button type="button" data-sa-member-clear>Close activity</button>' +
        '<ol aria-label="Member assignment history">' +
        (memberActivityItems.map(item =>
          '<li>Demand ' + safe(item.ticketId) + ' · ' + safe(item.role) +
          ' · Attempt ' + safe(item.attempt) +
          ' · Assignment ' + safe(item.assignmentState) +
          ' · Demand ' + safe(item.ticketState) +
          ' · ' + safe(activityTime(item.createdAt)) +
          ' · ' + (item.hasResult ? 'Provider result recorded (unverified)' : 'No provider result') +
          ' <button type="button" data-sa-member-open-demand="' + safe(item.assignmentId) +
          '" aria-label="Open demand for assignment ' + safe(item.assignmentId) +
          '">Open demand</button>' +
          (item.chatAvailable && openAssignmentChat
            ? ' <button type="button" data-sa-member-open-chat="' + safe(item.assignmentId) +
              '" aria-label="Open chat for assignment ' + safe(item.assignmentId) + '">Open chat</button>'
            : ' · Native chat unavailable') +
          '</li>').join('') || '<li class="sa-empty">No Team assignments recorded for this bot.</li>') +
        '</ol>' +
        (memberActivityHasMore
          ? '<button type="button" data-sa-member-older>Older activity</button>'
          : '<p>End of recorded Team activity.</p>') + '</section>'
      : '';
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
        ' · No task grant issued</p>' +
        (effectiveTeam.manualAdmissionBlockers.length
          ? '<p>Native manual admission blocked:</p><ul aria-label="Native manual admission blockers">' +
            effectiveTeam.manualAdmissionBlockers.map(reason => '<li>' + safe(reason) + '</li>').join('') + '</ul>'
          : '<p>Native manual topology and role preflight passed. Provider tools, Skills, grants and runtime capacity remain unverified.</p>') +
        '<ol>' + effectiveTeam.members.map(member =>
          '<li><strong>' + safe(member.role) + ' ' + (member.position + 1) +
          ': ' + safe(member.name) + '</strong>' +
          ' · Harness ' + safe(member.provider) + ' · Model ' + safe(member.model) +
          ' · Bot mode ' + safe(member.mode) +
          ' · Reasoning effort ' + safe(member.reasoningEffort || 'not specified') +
          ' · Project ' + safe(member.projectId) +
          ' <button type="button" data-sa-member-activity="' + safe(member.botId) +
          '" aria-label="View activity for ' + safe(member.name) + '">Activity</button>' +
          (member.present === false ? ' · Missing bot (not executable)' :
            member.paused ? ' · Paused' : ' · Not paused (live availability unverified)') +
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
            '<li>' + (member.present === false
              ? 'Skill inventory unavailable for missing bot' : 'No bot-assigned Skills') +
            '</li>') + '</ul></li>').join('') + '</ol>' + memberActivityView + '</section>'
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
              roleChoices('additionalWorkers', 'Additional Workers (up to 7)', workerOrder,
                [chosen.coordinatorBotId, chosen.workerBotId]) +
              '<ol class="sa-worker-order" aria-label="Additional Worker order">' +
              workerOrder.map((id,index) => '<li><span>' + (index + 2) + '. ' + safe(botName(id)) +
                '</span><button type="button" data-sa-worker-move="' + safe(id) +
                '" data-sa-direction="up" aria-label="Move ' + safe(botName(id)) +
                ' up"' + (index === 0 || chosen.enabled ? ' disabled' : '') + '>Up</button>' +
                '<button type="button" data-sa-worker-move="' + safe(id) +
                '" data-sa-direction="down" aria-label="Move ' + safe(botName(id)) +
                ' down"' + (index === workerOrder.length - 1 || chosen.enabled ? ' disabled' : '') +
                '>Down</button></li>').join('') + '</ol>' +
              roleChoices('reviewers', 'Optional Council Reviewers (up to 4)', reviewerDraft,
                [chosen.coordinatorBotId, chosen.workerBotId]) +
              '<p>A bot may have one role only. Workers must share the primary Worker project. A disabled Team may change roles after all demands settle; active/reviewable work and unresolved effects block native saves.</p>' +
              '<button class="primary-button" type="submit"' +
                (policyEditBlocker || roleRoster.policyRevision !== chosen.policyRevision
                   ? ' disabled aria-describedby="sa-role-edit-blocker"' : '') +
                '>Save configured roles</button></form>' +
              (policyEditBlocker ? '<p id="sa-role-edit-blocker" role="status">Team role edit blocked: ' +
                safe(policyEditBlocker) + '</p>' : '') +
              '<form data-sa-form="attempt-budget">' +
                '<label>Maximum attempts per Demand<select name="maxWorkerAttempts">' +
                [1,2,3,4].map(limit => '<option value="' + limit + '"' +
                  (roleRoster?.maxWorkerAttempts === limit ? ' selected' : '') +
                  '>' + limit + '</option>').join('') + '</select></label>' +
                '<p>Native hard cap, including the first Worker attempt. Requires a disabled Team; future Demands only. No automatic retry is enabled.</p>' +
                '<button type="submit"' + (policyEditBlocker ? ' disabled aria-describedby="sa-budget-edit-blocker"' : '') +
                  '>Save Worker attempt budget</button></form>' +
              (policyEditBlocker ? '<p id="sa-budget-edit-blocker" role="status">Team budget edit blocked: ' +
                safe(policyEditBlocker) + '</p>' : '') +
              '<form data-sa-form="team-strategy">' +
                '<label>Team purpose<textarea name="purpose" rows="3" maxlength="400">' +
                  safe(roleRoster.purpose) + '</textarea></label>' +
                '<label>Optimization strategy<select name="strategy">' +
                  ([['balanced','Balanced'],['economy','Economy'],['performance','Performance'],
                    ['security_critical','Security-critical'],['custom','Custom']] as const)
                    .map(([value,label]) => '<option value="' + value + '"' +
                      (roleRoster?.strategy === value ? ' selected' : '') + '>' + label + '</option>').join('') +
                  '</select></label>' +
                '<p>Descriptive Team intent only. Optimization does not change native bot models, tool permissions, costs or required tests and reviews.</p>' +
                '<button type="submit"' + (policyEditBlocker ||
                  roleRoster.policyRevision !== chosen.policyRevision
                  ? ' disabled aria-describedby="sa-strategy-edit-blocker"' : '') +
                  '>Save Team purpose and strategy</button></form>' +
              (policyEditBlocker ? '<p id="sa-strategy-edit-blocker" role="status">Team strategy edit blocked: ' +
                safe(policyEditBlocker) + '</p>' : '') +
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
        '<div class="sa-actions"><button type="button" data-sa-open-runs>Assign demand / Open runs</button>' +
        '<button type="button" data-sa-duplicate="' + safe(chosen.id) +
        '">Duplicate as new Team</button></div></section>'
      : '<section class="sa-pane"><h3>No Teams yet</h3><p>Create a disabled Team from two eligible existing bots. Nothing starts automatically.</p></section>';
    // Capture mutable UI state once so native review row predicates remain type-safe.
    const capturedRejection = globalRejection;
    const capturedAcceptance = globalAcceptance;
    const globalReviewPanel = '<section class="sa-pane sa-cross-team-review" ' +
      'role="region" aria-label="Cross-Team native review inbox"><header>' +
      '<h3>Review inbox · ' + safe(reviewTeamFilter
         ? presets.find(team => team.id === reviewTeamFilter)?.name ?? 'Selected Team'
         : 'All Teams') + '</h3><button type="button" data-sa-load-review-inbox>' +
      'Refresh review inbox</button></header>' +
      '<p>Up to 200 recently updated native Tickets requiring human acceptance or ' +
      'awaiting Council. A waiting Council verdict cannot be manually treated as passed. ' +
      'Open a Demand to inspect its native Team and revision before deciding.</p>' +
      '<label>Filter decisions by Team<select data-sa-review-team-filter ' +
      'aria-label="Filter review inbox by Team"><option value="">All Teams</option>' +
      presets.map(team => '<option value="' + safe(team.id) + '"' +
        (reviewTeamFilter === team.id ? ' selected' : '') + '>' + safe(team.name) +
        '</option>').join('') + '</select></label>' +
      (!reviewQueueLoaded ? '<p role="status">Load native review queue to inspect all Teams.</p>' :
        '<ul class="sa-tickets">' + (reviewQueue.map(item =>
          '<li><div><strong>' + safe(item.objective) + '</strong><small>' +
          safe(presets.find(team => team.id === item.orchestrationId)?.name ?? 'Unknown Team') +
          ' · ' + safe(item.state === 'waiting_approval' ? 'Human decision' :
            'Awaiting native Council (not approved)') + ' · Revision ' +
          item.revision + '</small></div><button type="button" data-sa-open-demand="' +
          safe(item.id) + '">Open demand</button>' +
          (item.state === 'waiting_approval'
            ? '<button type="button" data-sa-global-accept="' + safe(item.id) +
              '">Accept evidence</button><button type="button" data-sa-global-reject="' +
              safe(item.id) + '">Reject evidence</button>' : '') + '</li>').join('') ||
          '<li>No native Tickets currently awaiting a decision.</li>') + '</ul>' +
          (capturedRejection && reviewQueue.some(row => row.id === capturedRejection.id &&
              row.orchestrationId === capturedRejection.teamId &&
              row.revision === capturedRejection.revision && row.state === 'waiting_approval')
            ? '<form data-sa-form="global-review-reject" class="sa-global-review-reject" ' +
              'aria-label="Reject native Demand evidence"><h4>Reject Demand ' +
              safe(capturedRejection.id) + '</h4><p>Original Team ' +
              safe(presets.find(team => team.id === capturedRejection.teamId)?.name ?? 'Unknown') +
              ' · Revision ' + capturedRejection.revision +
              '. This is a negative human decision, not a Council verdict or a retry.</p>' +
              '<p>Native Worker evidence excerpt (up to 2,000 characters):</p>' +
              '<blockquote class="sa-evidence-excerpt">' +
              safe(capturedRejection.evidence?.slice(0, 2000) || 'No evidence recorded') +
              '</blockquote>' +
              (capturedRejection.evidence && capturedRejection.evidence.length > 2000
                ? '<p>Evidence truncated. Open Demand to inspect the complete native record.</p>' : '') +
              '<label>Reason for rejecting evidence<textarea name="reason" rows="3" ' +
              'maxlength="500" required></textarea></label>' +
              '<div class="sa-actions"><button type="submit" class="primary-button">' +
              'Review rejection</button><button type="button" ' +
              'data-sa-global-reject-cancel>Cancel rejection</button></div></form>'
            : '') +
          (capturedAcceptance && reviewQueue.some(row => row.id === capturedAcceptance.id &&
              row.orchestrationId === capturedAcceptance.teamId &&
              row.revision === capturedAcceptance.revision && row.state === 'waiting_approval')
            ? '<form data-sa-form="global-review-accept" class="sa-global-review-accept" ' +
              'aria-label="Accept native Demand evidence"><h4>Accept Demand ' +
              safe(capturedAcceptance.id) + '</h4><p>Original Team ' +
              safe(presets.find(team => team.id === capturedAcceptance.teamId)?.name ?? 'Unknown') +
              ' · Revision ' + capturedAcceptance.revision +
              '. This records explicit human acceptance for a Council-Off Demand only. ' +
              'It is not a native Council verdict and cannot override unresolved effects.</p>' +
              '<p>Native Worker evidence excerpt (up to 2,000 characters):</p>' +
              '<blockquote class="sa-evidence-excerpt">' +
              safe(capturedAcceptance.evidence.slice(0, 2000)) + '</blockquote>' +
              (capturedAcceptance.evidence.length > 2000
                ? '<p>Evidence truncated. Open Demand to inspect the complete native record.</p>' : '') +
              '<div class="sa-actions"><button type="submit" class="primary-button">' +
              'Review acceptance</button><button type="button" ' +
              'data-sa-global-accept-cancel>Cancel acceptance</button></div></form>'
            : '') +
          '<p role="status">' + reviewQueue.length + ' native pending decisions loaded.</p>' +
          (reviewQueueHasMore ? '<button type="button" data-sa-load-older-reviews>' +
            'Load older pending decisions</button>' : '')) +
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
    // A full workspace render must not silently close the expanded member editor.
    const memberEditor = host.querySelector<HTMLDetailsElement>('details.sa-new-members');
    if (memberEditor) draftMembersExpanded = memberEditor.open;
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
          '>Issue Resolution</option>' +
          ['Economy','Balanced','Performance','Security-critical'].map(name =>
            '<option value="' + name + '"' +
            (draftPresetTemplate === name ? ' selected' : '') + '>' + name + '</option>'
          ).join('') + '</select></label>' +
          '<label>Name<input name="name" maxlength="120" placeholder="Software Development" value="' +
          safe(draftPresetName) + '" required></label>' +
          '<div class="sa-grid"><label>Coordinator<select name="coordinatorBotId" required>' +
          '<option value="">Choose bot</option>' + optionsFor(draftCoordinatorBotId) + '</select></label>' +
          '<label>Worker<select name="workerBotId" required><option value="">Choose bot</option>' +
          optionsFor(draftWorkerBotId) + '</select></label></div>' +
          '<details class="sa-new-members"' + (draftMembersExpanded ? ' open' : '') +
          '><summary>Additional Team members (optional)</summary>' +
          '<p>Choose up to seven additional Workers and four independent Reviewers. ' +
          'Each bot has one role, and Workers must share the primary Worker project. ' +
          'Multi-member Teams remain Off until scoped admission is supported.</p>' +
          roleChoices('additionalWorkers', 'Additional Workers', draftAdditionalWorkerIds,
            [draftCoordinatorBotId, draftWorkerBotId]) +
          roleChoices('reviewers', 'Optional Reviewers', draftReviewerBotIds,
            [draftCoordinatorBotId, draftWorkerBotId]) + '</details>' +
          '<label>Maximum attempts per Demand<select name="maxWorkerAttempts">' +
          [1,2,3,4].map(limit => '<option value="' + limit + '"' +
            (draftWorkerAttemptLimit === limit ? ' selected' : '') + '>' +
            limit + '</option>').join('') + '</select></label>' +
          '<label>Team purpose<textarea name="purpose" rows="3" maxlength="400">' +
          safe(draftPresetPurpose) + '</textarea></label>' +
          '<label>Optimization strategy<select name="strategy">' +
          ([['balanced','Balanced'],['economy','Economy'],['performance','Performance'],
            ['security_critical','Security-critical'],['custom','Custom']] as const)
            .map(([value,label]) => '<option value="' + value + '"' +
              (draftPresetStrategy === value ? ' selected' : '') + '>' + label + '</option>').join('') +
          '</select></label>' +
          '<p>Suggestions describe future Demands only; no tools or work are authorized.</p>' +
          '<p>Hard cap including the first attempt. Economy and Security-critical suggest 1; ' +
          'Balanced and Issue Resolution suggest 2; Performance and Software Development suggest 3. ' +
          'No automatic retry, tool grant or execution is enabled.</p>' +
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
    // Native reloads replace the entire workspace. Block stateful actions
    // until the snapshot arrives, but preserve the local-only new-Team draft
    // (example template and name) without granting any native authority.
    if (loading) {
      host.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
        'button, input, select, textarea',
      ).forEach(control => {
        const localDraft = control.matches(
          'form[data-sa-form="create"] [data-sa-template], form[data-sa-form="create"] input[name="name"], form[data-sa-form="create"] select[name="maxWorkerAttempts"], form[data-sa-form="create"] textarea[name="purpose"], form[data-sa-form="create"] select[name="strategy"]',
        );
        control.disabled = !localDraft;
      });
    }
    host.setAttribute('aria-busy', String(busy || loading));
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
      if (snapshot.ticket.id !== id || snapshot.ticket.orchestrationId !== selected) {
        ticketDetails = null; threadLinks = []; inspectedAssignmentId = '';
        throw new Error('Native Ticket identity or Team changed; reload before acting.');
      }
      if (olderActivityTicketId !== id) {
        olderActivity = []; olderActivityHasMore = true; olderActivityTicketId = id;
      }
      ticketDetails = snapshot;
      if (!snapshot.assignments.some(item => item.id === inspectedAssignmentId)) {
        inspectedAssignmentId = '';
      }
      threadLinks = links;
      linkedDecisions = []; linkedDecisionsLoaded = false; linkedDecisionsHasMore = false;
      // Keep the ticket row and its grant/history pinned to one native revision.
      const index = tickets.findIndex(ticket => ticket.id === id);
      if (index >= 0) tickets[index] = snapshot.ticket;
    }
  }
  // Native reloads may overlap a mutation or a view refresh. Serialize reads
  // so an older, slower list cannot overwrite a newer confirmed Team state.
  // A post-mutation reload also waits for any earlier in-flight observation.
  let reloadTail: Promise<void> = Promise.resolve();
  function reload(): Promise<void> {
    const next = reloadTail.then(() => loadNativeSnapshot());
    reloadTail = next.catch(() => {});
    return next;
  }
  // Never treat the create/duplicate ACK as a verified directory state.
  // A stale native read may be repeated; an acknowledged write never is.
  async function reloadCreatedTeam(
    created: Config, expectedRoster?: {workerBotIds:string[]; reviewerBotIds:string[]},
  ): Promise<void> {
    if (!created || typeof created.id !== 'string' ||
        !/^[A-Za-z0-9_-]{1,128}$/.test(created.id) || created.enabled !== false) {
      throw new Error('Native Team creation returned an invalid or enabled identity.');
    }
    selected = created.id;
    teamSearch = ''; teamStatusFilter = 'all';
    await reload();
    let persisted = presets.find(item => item.id === created.id);
    if (!persisted) {
      selected = created.id;
      await reload();
      persisted = presets.find(item => item.id === created.id);
    }
    if (!persisted || persisted.enabled ||
        persisted.coordinatorBotId !== created.coordinatorBotId ||
        persisted.workerBotId !== created.workerBotId) {
      selected = '';
      throw new Error('Native Team creation was acknowledged but its disabled identity could not be verified. Refresh before retrying; the Team may already exist.');
    }
    if (expectedRoster) {
      // A create ACK and a directory row do not prove the requested roster
      // persisted. Recheck native membership before announcing creation.
      let saved: TeamRoster;
      try {
        saved = await call<TeamRoster>('superagent_roster',{orchestrationId:created.id});
      } catch {
        selected = '';
        throw new Error('Native Team creation was acknowledged but its configured roles could not be read. Refresh before retrying; the Team may already exist.');
      }
      if (saved.orchestrationId !== created.id ||
          saved.policyRevision !== persisted.policyRevision ||
          saved.coordinatorBotId !== created.coordinatorBotId ||
          !Array.isArray(saved.workerBotIds) || !Array.isArray(saved.reviewerBotIds) ||
          saved.workerBotIds.length !== expectedRoster.workerBotIds.length ||
          saved.reviewerBotIds.length !== expectedRoster.reviewerBotIds.length ||
          saved.workerBotIds.some((id,index)=>id !== expectedRoster.workerBotIds[index]) ||
          saved.reviewerBotIds.some((id,index)=>id !== expectedRoster.reviewerBotIds[index])) {
        selected = '';
        throw new Error('Native Team creation was acknowledged but its configured roles could not be verified. Refresh before retrying; the Team may already exist.');
      }
    }
    selected = created.id;
  }

  async function loadNativeSnapshot(): Promise<void> {
    loading = true;
    // Mutations invalidate the cross-Team index. Never show stale global
    // status as if the native Tickets had already been re-read.
    allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
    reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
    globalRejection = null; globalAcceptance = null;
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
      if (reviewTeamFilter && !presets.some(config => config.id === reviewTeamFilter)) {
        reviewTeamFilter = '';
        reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
      }
      if (!presets.some(config => config.id === selected)) {
        selected = presets[0]?.id ?? '';
        selectedDemandWorkerId = ''; demandPreflight = null;
        inspectedTicket = ''; threadLinks = [];
        linkedDecisions = []; linkedDecisionsLoaded = false; linkedDecisionsHasMore = false; roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
        sourceIntegrity = null;
      }
      // A failed native read must not leave old ticket approval controls
      // visible underneath a newly selected or reloaded preset.
      tickets = [];
      ticketDetails = null; inspectedAssignmentId = '';
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      // A Demand opened from the native cross-Team index may be older than
      // the selected Team's latest-50 page. Rehydrate only its exact native
      // Ticket identity; never substitute a similarly named or foreign Run.
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
      // Any native refresh invalidates this non-authoritative, time-bound scan.
      effectiveTeam = null; clearMemberActivity();
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
  // Templates suggest only a name and retry cap, never a bot, tool or task.
  host.addEventListener('change', event => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.type === 'checkbox' &&
        ['additionalWorkers', 'reviewers'].includes(target.name)) {
      const form = target.closest<HTMLFormElement>('form[data-sa-form]');
      if (!form || busy) return;
      const checked = Array.from(form.querySelectorAll<HTMLInputElement>(
        'input[type="checkbox"][name="' + target.name + '"]:checked',
      )).map(input => input.value);
      if (form.dataset.saForm === 'create') {
        if (target.name === 'additionalWorkers') draftAdditionalWorkerIds = checked;
        else draftReviewerBotIds = checked;
      } else if (form.dataset.saForm === 'roster' &&
          roleRoster?.orchestrationId === selected) {
        if (target.name === 'reviewers') {
          reviewerDraft = checked;
        } else {
          const focusedId = target.value;
          workerOrder = [...workerOrder.filter(id => checked.includes(id)),
            ...checked.filter(id => !workerOrder.includes(id))];
          render();
          Array.from(host.querySelectorAll<HTMLInputElement>(
            'form[data-sa-form="roster"] input[name="additionalWorkers"]',
          )).find(input => input.value === focusedId)?.focus({preventScroll:true});
        }
      }
      return;
    }
    if (target instanceof HTMLSelectElement &&
        target.closest('form[data-sa-form="create"]') &&
        ['coordinatorBotId', 'workerBotId'].includes(target.name)) {
      if (target.name === 'coordinatorBotId') draftCoordinatorBotId = target.value;
      else draftWorkerBotId = target.value;
      draftAdditionalWorkerIds = draftAdditionalWorkerIds.filter(id =>
        id !== draftCoordinatorBotId && id !== draftWorkerBotId);
      draftReviewerBotIds = draftReviewerBotIds.filter(id =>
        id !== draftCoordinatorBotId && id !== draftWorkerBotId);
      render();
      Array.from(host.querySelectorAll<HTMLSelectElement>(
        'form[data-sa-form="create"] select',
      )).find(input => input.name === target.name)?.focus({preventScroll:true});
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-team-status')) {
      if (!['all','enabled','disabled'].includes(target.value)) return;
      teamStatusFilter = target.value as typeof teamStatusFilter;
      render();
      host.querySelector<HTMLSelectElement>('[data-sa-team-status]')?.focus({preventScroll:true});
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-review-team-filter')) {
      if (busy) { target.value = reviewTeamFilter; return; }
      const teamId = target.value;
      if (teamId && !presets.some(item => item.id === teamId)) {
        target.value = reviewTeamFilter; return;
      }
      reviewTeamFilter = teamId;
      globalRejection = null; globalAcceptance = null;
      reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
      render();
      void work(async () => {
        reviewQueue = await fetchReviewPage(teamId);
        reviewQueueHasMore = reviewQueue.length === 200;
        reviewQueueLoaded = true;
      });
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
          projectId:demandProjectFilter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-demand-project-filter')) {
      if (busy) { target.value = demandProjectFilter; return; }
      const filter = target.value;
      // Previously admitted Demands retain their original project even when
      // the Team has moved to a different project. Both are native-backed IDs.
      if (filter && !presets.some(item => item.projectId === filter)
          && !allDemands.some(item => item.projectId === filter)) {
        target.value = demandProjectFilter; return;
      }
      demandProjectFilter = filter;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
          projectId:filter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target instanceof HTMLSelectElement && target.hasAttribute('data-sa-demand-visibility-filter')) {
      if (busy) { target.value = demandArchived ? 'archived' : 'active'; return; }
      if (target.value !== 'active' && target.value !== 'archived') return;
      demandArchived = target.value === 'archived';
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
          projectId:demandProjectFilter || undefined,
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
        'completed','failed','stopped','interrupted','inconclusive','needs_attention'];
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
          projectId:demandProjectFilter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target instanceof HTMLSelectElement && target.name === 'demandWorkerId' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      selectedDemandWorkerId = target.value;
      demandPreflight = null;
      render();
      host.querySelector<HTMLSelectElement>('form[data-sa-form="run"] select[name="demandWorkerId"]')
        ?.focus({preventScroll:true});
      return;
    }
    if (target instanceof HTMLSelectElement && target.name === 'mode' &&
        target.closest<HTMLFormElement>('form[data-sa-form="run"]')) {
      demandMode = target.value === 'direct' ? 'direct' : 'coordinated';
      demandPreflight = null;
      render();
      host.querySelector<HTMLSelectElement>('form[data-sa-form="run"] select[name="mode"]')
        ?.focus({preventScroll:true});
      return;
    }
    if (target instanceof HTMLSelectElement && target.name === 'maxWorkerAttempts' &&
        target.closest('form[data-sa-form="create"]')) {
      const limit = Number(target.value);
      if ([1,2,3,4].includes(limit)) draftWorkerAttemptLimit = limit;
      return;
    }
    if (target instanceof HTMLSelectElement && target.name === 'strategy' &&
        target.closest('form[data-sa-form="create"]')) {
      draftPresetStrategy = target.value;
      return;
    }
    if (!(target instanceof HTMLSelectElement) || !target.hasAttribute('data-sa-template')) return;
    draftPresetTemplate = target.value;
    draftPresetName = target.value;
    draftWorkerAttemptLimit = ['Economy','Security-critical'].includes(target.value) ? 1 :
      ['Performance','Software Development'].includes(target.value) ? 3 : 2;
    const field = target.form?.querySelector<HTMLInputElement>('input[name="name"]');
    if (field) field.value = draftPresetName;
    const examples: Record<string, {purpose:string;strategy:string}> = {
      'Software Development': {purpose:'Implement software changes with reproducible tests and explicit native scope.',strategy:'balanced'},
      'Issue Resolution': {purpose:'Diagnose and resolve issues with traceable regression evidence.',strategy:'balanced'},
      'Economy': {purpose:'Minimize resource usage without skipping mandatory checks.',strategy:'economy'},
      'Balanced': {purpose:'Balance delivery quality, execution time and resource cost.',strategy:'balanced'},
      'Performance': {purpose:'Prioritize thorough verification within explicit time and cost budgets.',strategy:'performance'},
      'Security-critical': {purpose:'Prioritize scoped permissions and security verification for every change.',strategy:'security_critical'},
    };
    draftPresetPurpose = examples[target.value]?.purpose ?? '';
    draftPresetStrategy = examples[target.value]?.strategy ?? 'balanced';
    const budget = target.form?.querySelector<HTMLSelectElement>('select[name="maxWorkerAttempts"]');
    if (budget) budget.value = String(draftWorkerAttemptLimit);
    const purpose = target.form?.querySelector<HTMLTextAreaElement>('textarea[name="purpose"]');
    if (purpose) purpose.value = draftPresetPurpose;
    const strategy = target.form?.querySelector<HTMLSelectElement>('select[name="strategy"]');
    if (strategy) strategy.value = draftPresetStrategy;
  });
  host.addEventListener('input', event => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.hasAttribute('data-sa-demand-search')) {
      demandSearchDraft = target.value;
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
      const dispatch = host.querySelector<HTMLButtonElement>(
        'form[data-sa-form="run"] button[type="submit"]');
      if (dispatch) dispatch.disabled = true;
    }
    if (target instanceof HTMLInputElement && target.name === 'name' &&
        target.closest<HTMLFormElement>('form[data-sa-form="create"]')) {
      draftPresetName = target.value;
    }
    if (target instanceof HTMLTextAreaElement && target.name === 'purpose' &&
        target.closest<HTMLFormElement>('form[data-sa-form="create"]')) {
      draftPresetPurpose = target.value;
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
        // Native transaction freezes the source revision and ordered roles;
        // no renderer-side read/create gap can silently change the copied Team.
        const created = await call<Config>('superagent_duplicate', {
          sourceId:source.id, expectedRevision:source.policyRevision, name,
        });
        workspaceView = 'teams';
        selectedDemandWorkerId = ''; demandPreflight = null;
        roleRoster = null; workerOrder = []; reviewerDraft = []; sourcesLoaded = false; sources = [];
        await reloadCreatedTeam(created);
        feedback = 'Disabled Team duplicated. No prior Tickets, source grants or approvals were copied.';
      });
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
    if (target.hasAttribute('data-sa-load-older-activity')) {
      const id = inspectedTicket, details = ticketDetails;
      if (!id || details?.ticket.id !== id || details.activity.length !== 50 ||
          !olderActivityHasMore || busy) return;
      const known = [...details.activity, ...(olderActivityTicketId === id ? olderActivity : [])];
      const beforeId = Math.min(...known.map(entry => entry.id));
      if (!Number.isSafeInteger(beforeId) || beforeId <= 0) return;
      void work(async () => {
        const page = await call<ActivityPage>('superagent_activity_page',
          {ticketId:id,beforeId,limit:50});
        if (inspectedTicket !== id || ticketDetails?.ticket.id !== id) return;
        if (page.ticketId !== id || !Array.isArray(page.items) || page.items.length > 50 ||
            page.items.some((entry, index) =>
              !Number.isSafeInteger(entry.id) || entry.id <= 0 ||
              entry.id >= beforeId || (index > 0 && entry.id >= page.items[index-1].id)) ||
            (page.hasMore && (page.items.length === 0 ||
              page.nextCursor !== page.items[page.items.length-1].id)) ||
            (!page.hasMore && page.nextCursor !== null)) {
          throw new Error('Native activity page identity or cursor is inconsistent; reload Ticket.');
        }
        const seen = new Set(known.map(entry => entry.id));
        olderActivity = [...(olderActivityTicketId === id ? olderActivity : []),
          ...page.items.filter(entry => !seen.has(entry.id))];
        olderActivityTicketId = id;
        olderActivityHasMore = page.hasMore;
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
      // A decision staged in another view is no longer an active user intent.
      if (next !== 'review') { globalAcceptance = null; globalRejection = null; }
      render();
      host.querySelector<HTMLButtonElement>('[data-sa-view="' + next + '"]')?.focus({preventScroll:true});
      return;
    }
    if (target.hasAttribute('data-sa-preflight')) {
      const form = host.querySelector<HTMLFormElement>('form[data-sa-form="run"]');
      const chosen = current();
      const objective = form?.querySelector<HTMLTextAreaElement>('textarea[name="objective"]')?.value.trim() ?? '';
      const coordinated = form?.querySelector<HTMLSelectElement>('select[name="mode"]')?.value !== 'direct';
      const isMulti = (chosen?.workerCount ?? 1) > 1;
      if (!chosen || !objective) return;
      if (isMulti && (coordinated || roleRoster?.orchestrationId !== chosen.id ||
          !roleRoster.workerBotIds.includes(selectedDemandWorkerId))) {
        error = 'Choose a configured Worker and direct dispatch for this read-only Demand.';
        render(); return;
      }
      const workerId = isMulti ? selectedDemandWorkerId : chosen.workerBotId;
      demandPreflight = null;
      void work(async () => {
        const snapshot = isMulti
          ? await call<ManualPreflight>('superagent_preflight_worker', {
            orchestrationId:chosen.id,objective,workerBotId:workerId,
          })
          : await call<ManualPreflight>('superagent_preflight', {
            orchestrationId:chosen.id,objective,coordinated,
          });
        if (snapshot.orchestrationId !== chosen.id ||
            snapshot.policyRevision !== chosen.policyRevision ||
            snapshot.selectedRole !== (coordinated ? 'coordinator' : 'worker') ||
            snapshot.selectedBotId !== (coordinated ? chosen.coordinatorBotId : workerId) ||
            snapshot.allowedTools.length !== 0 || snapshot.allowedSideEffects) {
          demandPreflight = null;
          throw new Error('Native preflight identity, policy or read-only scope changed. Refresh the Team.');
        }
        demandPreflight = snapshot;
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
        if (!Array.isArray(receipts) || receipts.length > 50) {
          throw new Error('Invalid native linked source receipt page.');
        }
        if (inspectedTicket === ticketId) {
          linkedDecisions = receipts; linkedDecisionsLoaded = true;
          linkedDecisionsHasMore = receipts.length === 50;
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-older-linked-activity')) {
      const ticketId = inspectedTicket;
      const last = linkedDecisions[linkedDecisions.length - 1];
      if (!ticketId || busy || !linkedDecisionsLoaded || !linkedDecisionsHasMore || !last) return;
      const validKey = (key:string) => /^[A-Za-z0-9_-]{1,128}$/.test(key);
      if (!Number.isSafeInteger(last.at) || last.at <= 0 ||
          !validKey(last.sourceId) || !validKey(last.deliveryId)) {
        error = 'Invalid native linked activity cursor; reload the inspector.';
        render();
        return;
      }
      void work(async () => {
        const before:LinkedActivityCursor = {
          at:last.at, sourceId:last.sourceId, deliveryId:last.deliveryId,
        };
        const page = await call<LinkedActivityPage>('superagent_source_thread_activity_page', {
          ticketId, limit:50, before,
        });
        const older = (a:SourceDecision, b:SourceDecision) =>
          b.at < a.at || (b.at === a.at && (b.sourceId < a.sourceId ||
            (b.sourceId === a.sourceId && b.deliveryId < a.deliveryId)));
        const known = new Set(linkedDecisions.map(row => row.sourceId + ':' + row.deliveryId));
        if (!page || !Array.isArray(page.decisions) || page.decisions.length > 50 ||
            page.decisions.some((row,index) =>
              !row || !Number.isSafeInteger(row.at) || row.at <= 0 ||
              !validKey(row.sourceId) || !validKey(row.deliveryId) ||
              known.has(row.sourceId + ':' + row.deliveryId) ||
              !older(index ? page.decisions[index-1] : last,row))) {
          throw new Error('Native linked activity page overlaps or is out of order.');
        }
        const continuation = page.nextCursor;
        const end = page.decisions[page.decisions.length-1];
        if (continuation && (!end || continuation.at !== end.at ||
            continuation.sourceId !== end.sourceId ||
            continuation.deliveryId !== end.deliveryId)) {
          throw new Error('Native linked activity continuation is inconsistent.');
        }
        if (inspectedTicket === ticketId) {
          linkedDecisions = [...linkedDecisions,...page.decisions];
          linkedDecisionsHasMore = Boolean(continuation);
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-global-accept-cancel')) {
      globalAcceptance = null; render(); return;
    }
    if (target.hasAttribute('data-sa-global-reject-cancel')) {
      globalRejection = null; render(); return;
    }
    if (target.dataset.saGlobalAccept) {
      const row = reviewQueue.find(item => item.id === target.dataset.saGlobalAccept);
      if (!row || row.state !== 'waiting_approval' || busy ||
          (reviewTeamFilter && row.orchestrationId !== reviewTeamFilter)) return;
      void work(async () => {
        const snapshot = await call<TicketDetails>('superagent_snapshot',
          {ticketId:row.id, limit:1});
        if (snapshot.ticket.id !== row.id ||
            snapshot.ticket.orchestrationId !== row.orchestrationId ||
            snapshot.ticket.revision !== row.revision ||
            snapshot.ticket.state !== 'waiting_approval') {
          throw new Error('Native review decision changed; refresh the inbox before accepting.');
        }
        if (snapshot.finalizationGate === 'council_required' ||
            snapshot.finalizationGate === 'effects_unresolved' ||
            snapshot.finalizationGate === 'not_pending') {
          throw new Error('Native finalization gate blocks human acceptance; inspect the Demand.');
        }
        if (!snapshot.ticket.evidence?.trim()) {
          throw new Error('Native Worker evidence is missing; human acceptance is unavailable.');
        }
        globalRejection = null;
        globalAcceptance = {id:row.id, teamId:row.orchestrationId,
          revision:row.revision, evidence:snapshot.ticket.evidence, filter:reviewTeamFilter};
      });
      return;
    }
    if (target.dataset.saGlobalReject) {
      const row = reviewQueue.find(item => item.id === target.dataset.saGlobalReject);
      if (!row || row.state !== 'waiting_approval' || busy ||
          (reviewTeamFilter && row.orchestrationId !== reviewTeamFilter)) return;
      void work(async () => {
        // Index rows are untrusted until an exact native Ticket snapshot agrees.
        const snapshot = await call<TicketDetails>('superagent_snapshot',
          {ticketId:row.id, limit:1});
        if (snapshot.ticket.id !== row.id ||
            snapshot.ticket.orchestrationId !== row.orchestrationId ||
            snapshot.ticket.revision !== row.revision ||
            snapshot.ticket.state !== 'waiting_approval') {
          throw new Error('Native review decision changed; refresh the inbox before rejecting.');
        }
        globalAcceptance = null;
        globalRejection = {id:row.id, teamId:row.orchestrationId,
          revision:row.revision, evidence:snapshot.ticket.evidence, filter:reviewTeamFilter};
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-review-inbox')) {
      void work(async () => {
        globalRejection = null; globalAcceptance = null;
        reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
        reviewQueue = await fetchReviewPage(reviewTeamFilter);
        reviewQueueHasMore = reviewQueue.length === 200;
        reviewQueueLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-older-reviews')) {
      const last = reviewQueue[reviewQueue.length - 1];
      if (busy || !reviewQueueLoaded || !reviewQueueHasMore || !last) return;
      if (!Number.isSafeInteger(last.updatedAt) || last.updatedAt <= 0 ||
          !/^[A-Za-z0-9_-]{1,128}$/.test(last.id)) {
        error = 'Invalid native review cursor. Refresh the inbox.';
        render();
        return;
      }
      void work(async () => {
        const page = await fetchReviewPage(reviewTeamFilter, last);
        reviewQueue = [...reviewQueue, ...page];
        reviewQueueHasMore = page.length === 200;
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
          projectId:demandProjectFilter || undefined,
        });
        allDemands = loaded;
        allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (target.hasAttribute('data-sa-load-older-demands')) {
      const last = allDemands[allDemands.length - 1];
      if (busy || !allDemandsLoaded || !allDemandsHasMore || !last) return;
      if (!Number.isSafeInteger(last.updatedAt) || last.updatedAt <= 0 ||
          !/^[A-Za-z0-9_-]{1,128}$/.test(last.id)) {
        error = 'Invalid native Demand cursor. Refresh the index.';
        render();
        return;
      }
      const team = demandTeamFilter, state = demandStateFilter;
      const project = demandProjectFilter, search = demandSearch, archived = demandArchived;
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, beforeUpdatedAt:last.updatedAt, beforeId:last.id,
          orchestrationId:team || undefined,
          stateFilter:state || undefined,
          searchQuery:search || undefined,
          projectId:project || undefined,
        });
        const descends = (newer:DemandIndex, older:DemandIndex) =>
          older.updatedAt < newer.updatedAt ||
          (older.updatedAt === newer.updatedAt && older.id < newer.id);
        const known = new Set(allDemands.map(item => item.id));
        const pageIds = new Set<string>();
        if (!Array.isArray(loaded) || loaded.length > 200 ||
            loaded.some((item, index) => {
              if (!item || !/^[A-Za-z0-9_-]{1,128}$/.test(item.id) ||
                  !Number.isSafeInteger(item.updatedAt) || item.updatedAt <= 0 ||
                  !descends(index === 0 ? last : loaded[index - 1], item) ||
                  (team && item.orchestrationId !== team) ||
                  !isDemandStateMatch(state, item.state) ||
                  (archived ? item.archived !== true : item.archived === true) ||
                  (project && item.projectId !== project) ||
                  (search && (typeof item.objective !== 'string' ||
                    !nativeCaseFold(item.objective).includes(nativeCaseFold(search)))) ||
                  known.has(item.id) || pageIds.has(item.id)) return true;
              pageIds.add(item.id);
              return false;
            }) ||
            !allDemandsLoaded || allDemands[allDemands.length - 1]?.id !== last.id ||
            allDemands[allDemands.length - 1]?.updatedAt !== last.updatedAt ||
            team !== demandTeamFilter || state !== demandStateFilter ||
            project !== demandProjectFilter || search !== demandSearch ||
            archived !== demandArchived) {
          throw new Error('Native Demand page overlaps, changed scope or is out of order; refresh the index.');
        }
        allDemands = [...allDemands, ...loaded];
        allDemandsHasMore = loaded.length === 200;
      });
      return;
    }
    if (target.dataset.saRetryGlobalDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saRetryGlobalDemand);
      const team = presets.find(item => item.id === demand?.orchestrationId);
      if (!demand || demand.state !== 'blocked' || !team?.enabled ||
          (team.reviewerCount ?? 0) > 0 || busy) return;
      void work(async () => {
        // The cross-Team board is a read-only index, not a retry grant.
        // Pin the original Ticket identity and revision before user consent.
        const snapshot = await call<TicketDetails>('superagent_snapshot',
          {ticketId:demand.id,limit:1});
        const unchanged = (ticket:Ticket) =>
          ticket.id === demand.id && ticket.orchestrationId === demand.orchestrationId &&
          ticket.revision === demand.revision && ticket.state === 'blocked';
        if (!unchanged(snapshot.ticket)) {
          throw new Error('Demand changed since the native page was loaded; refresh before retrying.');
        }
        ask('Retry blocked Demand ' + demand.id + ' at revision ' + demand.revision +
          '? Native admission rechecks the original worker, mandate, project and attempt budget. ' +
          'This explicitly starts another read-only Worker attempt, not an approval.', async () => {
          // Confirmation can remain open while another session changes the Ticket.
          // Native retry itself also rechecks role, mandate, project and lease.
          const fresh = await call<TicketDetails>('superagent_snapshot',
            {ticketId:demand.id,limit:1});
          if (!unchanged(fresh.ticket)) {
            throw new Error('Demand changed before confirmation; refresh before retrying.');
          }
          let retried:Ticket;
          try {
            retried = await call<Ticket>('superagent_retry',{ticketId:demand.id});
          } finally {
            // Native retry may persist a blocked launch failure before returning
            // an error. Never leave the prior board controls as if nothing changed.
            await reload();
            const page = await fetchDemandPage({
              limit:200, orchestrationId:demandTeamFilter || undefined,
              stateFilter:demandStateFilter || undefined,
              searchQuery:demandSearch || undefined,
              projectId:demandProjectFilter || undefined,
            });
            allDemands = page; allDemandsLoaded = true;
            allDemandsHasMore = page.length === 200;
          }
          if (!retried || retried.id !== demand.id ||
              retried.orchestrationId !== demand.orchestrationId ||
              retried.revision <= demand.revision) {
            throw new Error('Native retry returned an inconsistent Ticket; inspect its latest state.');
          }
          feedback = 'Native retry admitted for Demand ' + demand.id +
            '. Inspect the Worker attempt and evidence; completion is not implied.';
        });
      });
      return;
    }

    if (target.dataset.saArchiveGlobalDemand || target.dataset.saRestoreGlobalDemand) {
      const archive = Boolean(target.dataset.saArchiveGlobalDemand);
      const id = target.dataset.saArchiveGlobalDemand ?? target.dataset.saRestoreGlobalDemand;
      const demand = allDemands.find(item => item.id === id);
      if (!demand || busy || Boolean(demand.archived) === archive ||
          !presets.some(item => item.id === demand.orchestrationId)) return;
      void work(async () => {
        const snapshot = await call<TicketDetails>('superagent_snapshot',{
          ticketId:demand.id,limit:1,
        });
        if (snapshot.ticket.id !== demand.id ||
            snapshot.ticket.orchestrationId !== demand.orchestrationId ||
            snapshot.ticket.revision !== demand.revision ||
            snapshot.ticket.state !== demand.state) {
          throw new Error('Demand changed since the native page was loaded; refresh before archive/restore.');
        }
        ask((archive ? 'Delete Demand ' : 'Restore Demand ') + demand.id +
          ' at revision ' + demand.revision +
          '? This only changes visibility; durable Ticket, native chat, receipts and audit are retained. ' +
          'Cancellation does not undo applied Git/file effects.', async () => {
          const updated = await call<Ticket>(archive ?
              'superagent_archive_demand' : 'superagent_restore_demand',{
            ticketId:demand.id,expectedRevision:demand.revision,
          });
          if (!updated || updated.id !== demand.id ||
              updated.orchestrationId !== demand.orchestrationId ||
              updated.revision !== demand.revision + 1) {
            await reload();
            throw new Error('Native archive/restore returned an inconsistent Ticket identity or revision.');
          }
          await reload();
          const page = await fetchDemandPage({
            limit:200, orchestrationId:demandTeamFilter || undefined,
            stateFilter:demandStateFilter || undefined,
            searchQuery:demandSearch || undefined,
            projectId:demandProjectFilter || undefined,
          });
          allDemands = page; allDemandsLoaded = true;
          allDemandsHasMore = page.length === 200;
          feedback = 'Demand visibility changed. Native audit history and effect receipts remain intact.';
        });
      });
      return;
    }
    if (target.dataset.saStopGlobalDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saStopGlobalDemand);
      if (!demand || busy || !presets.some(item => item.id === demand.orchestrationId)) return;
      void work(async () => {
        // A cross-Team page is an observation, not permission. Re-read the
        // canonical Ticket before asking, and CAS the same revision at Stop.
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
        ask('Cancel Demand ' + demand.id + ' at revision ' + demand.revision +
          '? Native revocation stops future work and quarantines pending effects. Applied Git/file changes cannot be undone.', async () => {
          const stopped = await call<Ticket>('superagent_stop',
            {ticketId:demand.id,expectedRevision:snapshot.ticket.revision});
          // The mutating operation is only confirmed by its own native Ticket
          // result. A mismatched reply is not proof that this Demand was stopped.
          if (stopped.id !== demand.id ||
              stopped.orchestrationId !== demand.orchestrationId ||
              stopped.revision !== snapshot.ticket.revision + 1 ||
              stopped.state !== 'stopped') {
            await reload();
            throw new Error('Native Stop returned an inconsistent Ticket identity or revision; inspect the Demand.');
          }
          await reload();
          // Re-read the first page under the *current* filters, rather than
          // showing a stale Stop control or forcing the user to refresh by hand.
          // Never infer settlement from the pre-Stop cross-Team index.
          const loaded = await fetchDemandPage({
            limit:200, orchestrationId:demandTeamFilter || undefined,
            stateFilter:demandStateFilter || undefined,
            searchQuery:demandSearch || undefined,
            projectId:demandProjectFilter || undefined,
          });
          allDemands = loaded;
          allDemandsLoaded = true;
          allDemandsHasMore = loaded.length === 200;
          feedback = 'Demand ' + demand.id + ' stopped by native revocation. Inspect unresolved effects before retry.';
        });
      });
      return;
    }
    if (target.dataset.saOpenDemand) {
      const demand = allDemands.find(item => item.id === target.dataset.saOpenDemand) ??
        reviewQueue.find(item => item.id === target.dataset.saOpenDemand);
      if (!demand || !presets.some(item => item.id === demand.orchestrationId)) return;
      selected = demand.orchestrationId;
      selectedDemandWorkerId = ''; demandPreflight = null;
      workspaceView = 'runs';
      tickets = []; inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
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
        reviewerDraft=[...roleRoster.reviewerBotIds];
      });
      return;
    }
    if (target.dataset.saMemberActivity) {
      const cfg = current(), botId = target.dataset.saMemberActivity;
      if (!cfg || !effectiveTeam || effectiveTeam.orchestrationId !== cfg.id ||
          effectiveTeam.policyRevision !== cfg.policyRevision ||
          !effectiveTeam.members.some(member => member.botId === botId)) return;
      clearMemberActivity();
      memberActivityTeamId = cfg.id; memberActivityBotId = botId;
      void work(async () => {
        const page = await fetchMemberActivity(cfg.id, botId);
        if (selected !== cfg.id || memberActivityBotId !== botId) return;
        memberActivityItems = page.items;
        memberActivityCounts = {
          assignmentCount: page.assignmentCount,
          demandCount: page.demandCount,
          attentionDemandCount: page.attentionDemandCount,
        };
        memberActivityHasMore = page.hasMore;
        memberActivityCursor = page.nextCursor;
      });
      return;
    }
    if (target.hasAttribute('data-sa-member-clear')) {
      clearMemberActivity(); render(); return;
    }
    if (target.hasAttribute('data-sa-member-older')) {
      const teamId = memberActivityTeamId, botId = memberActivityBotId;
      const before = memberActivityCursor;
      if (!teamId || teamId !== selected || !botId || !memberActivityHasMore || !before) return;
      void work(async () => {
        const page = await fetchMemberActivity(teamId, botId, before);
        if (selected !== teamId || memberActivityBotId !== botId) return;
        const seen = new Set(memberActivityItems.map(item => item.assignmentId));
        if (page.items.some(item => seen.has(item.assignmentId))) {
          throw new Error('Native activity page overlaps already inspected Team history.');
        }
        memberActivityItems = [...memberActivityItems, ...page.items];
        memberActivityCounts = {
          assignmentCount: page.assignmentCount,
          demandCount: page.demandCount,
          attentionDemandCount: page.attentionDemandCount,
        };
        memberActivityHasMore = page.hasMore;
        memberActivityCursor = page.nextCursor;
      });
      return;
    }

    if (target.dataset.saMemberOpenDemand) {
      const item = memberActivityItems.find(entry =>
        entry.assignmentId === target.dataset.saMemberOpenDemand);
      const cfg = current();
      if (!item || busy || !cfg || cfg.id !== memberActivityTeamId ||
          item.orchestrationId !== cfg.id || item.botId !== memberActivityBotId) return;
      void work(async () => {
        // A historical activity row is not authority to open an unrelated
        // Ticket. Recheck its immutable Team and exact native Assignment.
        const fresh = await call<TicketDetails>('superagent_snapshot', {
          ticketId:item.ticketId,limit:200,
        });
        if (!fresh?.ticket || !fresh.mandate ||
            fresh.ticket.id !== item.ticketId ||
            fresh.ticket.orchestrationId !== item.orchestrationId ||
            !Array.isArray(fresh.assignments) ||
            !fresh.assignments.some(assignment =>
              assignment.id === item.assignmentId &&
              assignment.botId === item.botId &&
              assignment.sessionId === item.sessionId &&
              assignment.attempt === item.attempt)) {
          throw new Error('Native Demand or Assignment identity changed; refresh member activity.');
        }
        const latest = await call<Ticket[]>('superagent_tickets', {
          orchestrationId:item.orchestrationId,limit:50,
        });
        if (!Array.isArray(latest) ||
            latest.some(ticket => ticket.orchestrationId !== item.orchestrationId)) {
          throw new Error('Native Team Ticket list returned foreign records; navigation denied.');
        }
        const page = await fetchDemandPage({limit:200,orchestrationId:item.orchestrationId});
        if (selected !== item.orchestrationId ||
            memberActivityTeamId !== item.orchestrationId ||
            memberActivityBotId !== item.botId) {
          throw new Error('Team selection changed while opening the Demand; retry from activity.');
        }
        // The selected assignment may be older than the Team's last 50
        // Tickets; inject only its reverified canonical Ticket snapshot.
        tickets = [fresh.ticket, ...latest.filter(ticket => ticket.id !== item.ticketId)];
        inspectedTicket = item.ticketId;
        inspectedAssignmentId = item.assignmentId;
        ticketDetails = null; threadLinks = [];
        await refreshTicketDetails();
        demandTeamFilter = item.orchestrationId;
        demandProjectFilter = ''; demandStateFilter = '';
        demandSearch = ''; demandSearchDraft = '';
        allDemands = page; allDemandsLoaded = true; allDemandsHasMore = page.length === 200;
        workspaceView = 'runs';
        effectiveTeam = null; clearMemberActivity();
      });
      return;
    }
    if (target.dataset.saMemberOpenChat) {
      const item = memberActivityItems.find(entry =>
        entry.assignmentId === target.dataset.saMemberOpenChat);
      if (!item || !item.chatAvailable || !openAssignmentChat ||
          !memberActivityTeamId || memberActivityTeamId !== selected ||
          item.orchestrationId !== selected || item.botId !== memberActivityBotId) return;
      void work(async () => {
        // Recheck the native task thread at navigation time. An immutable
        // Assignment alone cannot authorize a remapped bot conversation.
        const link = await call<AssignmentChatLink>(
          'superagent_member_assignment_chat_link', {
            orchestrationId:item.orchestrationId, ticketId:item.ticketId,
            assignmentId:item.assignmentId, botId:item.botId,
          });
        if (!link || selected !== item.orchestrationId ||
            memberActivityTeamId !== item.orchestrationId ||
            memberActivityBotId !== item.botId ||
            link.orchestrationId !== item.orchestrationId ||
            link.ticketId !== item.ticketId ||
            link.assignmentId !== item.assignmentId ||
            link.botId !== item.botId ||
            link.sessionId !== item.sessionId) {
          throw new Error('Native assignment chat identity changed; refresh Team activity before opening chat.');
        }
        await openAssignmentChat({
          teamId: link.orchestrationId, ticketId: link.ticketId,
          assignmentId: link.assignmentId, botId: link.botId,
          sessionId: link.sessionId,
        });
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
    if (target.dataset.saReviewTeam) {
      if (busy || loading) return;
      const teamId = target.dataset.saReviewTeam;
      if (!presets.some(team => team.id === teamId)) return;
      // A Team shortcut is a read-only navigation request, not an approval.
      // Drop prior Team and decision state before exposing any native controls.
      selected = teamId;
      workspaceView = 'review';
      reviewTeamFilter = teamId;
      globalRejection = null; globalAcceptance = null;
      reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
      tickets = []; inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; clearMemberActivity();
      roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null; selectedDemandWorkerId = ''; demandPreflight = null;
      render();
      void work(async () => {
        const [teamTickets, pending] = await Promise.all([
          call<Ticket[]>('superagent_tickets',{orchestrationId:teamId,limit:50}),
          fetchReviewPage(teamId),
        ]);
        if (selected !== teamId || workspaceView !== 'review' || reviewTeamFilter !== teamId) return;
        if (!Array.isArray(teamTickets) || teamTickets.some(ticket =>
            !ticket || ticket.orchestrationId !== teamId ||
            typeof ticket.id !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(ticket.id))) {
          throw new Error('Native Team Ticket list returned foreign or malformed records; review navigation denied.');
        }
        tickets = teamTickets;
        reviewQueue = pending;
        reviewQueueHasMore = pending.length === 200;
        reviewQueueLoaded = true;
      }).then(() => {
        if (!error && selected === teamId && workspaceView === 'review') {
          host.querySelector<HTMLSelectElement>('[data-sa-review-team-filter]')
            ?.focus({preventScroll:true});
        }
      });
      return;
    }
    if (target.hasAttribute('data-sa-open-runs')) {
      if (busy || loading || !selected || !presets.some(team => team.id === selected)) return;
      const teamId = selected;
      // Team detail navigates to a real native Team-scoped Demand page.
      // Do not reuse another Team's prior query or infer a new admission.
      workspaceView = 'runs';
      demandTeamFilter = teamId; demandProjectFilter = ''; demandStateFilter = '';
      demandSearch = ''; demandSearchDraft = '';
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const page = await fetchDemandPage({limit:200, orchestrationId:teamId});
        if (selected !== teamId || workspaceView !== 'runs' || demandTeamFilter !== teamId) return;
        allDemands = page;
        allDemandsHasMore = page.length === 200;
        allDemandsLoaded = true;
      }).then(() => {
        if (!error && selected === teamId && workspaceView === 'runs') {
          host.querySelector<HTMLTextAreaElement>(
            'form[data-sa-form="run"] textarea[name="objective"]',
          )?.focus({preventScroll:true});
        }
      });
      return;
    }
    if (target.dataset.saSelect || target.dataset.saAssignTeam || target.dataset.saAttentionTeam) {
      if (busy || loading) return;
      const next = target.dataset.saSelect ?? target.dataset.saAssignTeam ??
        target.dataset.saAttentionTeam ?? '';
      if (!presets.some(item => item.id === next)) return;
      const assigning = target.hasAttribute('data-sa-assign-team');
      const attentionOnly = target.hasAttribute('data-sa-attention-team');
      const loadScopedDemands = assigning || attentionOnly;
      workspaceView = 'runs';
      if (loadScopedDemands) {
        // Choose this Team's durable Demand scope; never infer admission.
        demandTeamFilter = next; demandProjectFilter = '';
        demandStateFilter = attentionOnly ? 'needs_attention' : '';
        demandSearch = ''; demandSearchDraft = '';
        allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      }
      const focusObjective = () => {
        if (assigning && !error && selected === next && workspaceView === 'runs') {
          host.querySelector<HTMLTextAreaElement>(
            'form[data-sa-form="run"] textarea[name="objective"]')?.focus();
        }
      };
      const loadTeamDemands = async () => {
        const loaded = await fetchDemandPage({
          limit:200,orchestrationId:next,
          stateFilter:attentionOnly ? 'needs_attention' : undefined,
        });
        if (selected !== next || demandTeamFilter !== next) return;
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      };
      if (selected === next) {
        if (loadScopedDemands) void work(loadTeamDemands).then(focusObjective);
        else render();
        return;
      }
      selected = next;
      clearMemberActivity();
      selectedDemandWorkerId = ''; demandPreflight = null;
      if (assigning) {
        // No cross-Team reuse of another user's unsent objective or task key.
        demandObjective = ''; demandMode = 'coordinated';
        taskKey = crypto.randomUUID();
      }
      tickets = [];
      inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      inspectedAssignmentId = ''; roleRoster = null; workerOrder = []; reviewerDraft = []; effectiveTeam = null;
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      sourceIntegrity = null;
      void work(async () => {
        tickets = await call<Ticket[]>('superagent_tickets',{ orchestrationId:next, limit:50 });
        if (loadScopedDemands) await loadTeamDemands();
      }).then(focusObjective);
      return;
    }
    if (target.dataset.saEnable) {
      const c = presets.find(item => item.id === target.dataset.saEnable);
      if (!c || busy || loading || !Number.isSafeInteger(c.policyRevision) ||
          teamActivationBlocker(c)) return;
      const requestedEnabled = !c.enabled;
      ask(requestedEnabled
        ? 'Enable manual mode for ' + c.name +
          '? Native admission still checks every Demand; this grants no tools or automatic tasks.'
        : 'Disable Team ' + c.name +
          '? Native Off revokes active mandates and quarantines unresolved effects. ' +
          'Already applied external changes cannot be undone.', async () => {
        // The directory and inspector share one revision-guarded local action.
        // A stale confirmation cannot authorize a different policy revision.
        const updated = await call<Config>('superagent_set_enabled', {
          orchestrationId:c.id, expectedRevision:c.policyRevision, enabled:requestedEnabled,
        });
        if (!updated || updated.id !== c.id ||
            updated.enabled !== requestedEnabled ||
            updated.policyRevision !== c.policyRevision + 1) {
          await reload();
          throw new Error('Native Team status response was inconsistent. Refresh before retrying.');
        }
        await reload();
        const persisted = presets.find(item => item.id === c.id);
        if (!persisted || persisted.enabled !== requestedEnabled ||
            persisted.policyRevision !== updated.policyRevision) {
          throw new Error('Native Team status change could not be verified after reload. Inspect the Team before retrying.');
        }
        feedback = requestedEnabled
          ? 'Manual Team enabled. No grant or task was started.'
          : 'Team disabled. Native mandates were revoked; verify any unresolved external effects.';
      });
      return;
    }
    if (target.dataset.saDelete) {
      const c = presets.find(item => item.id === target.dataset.saDelete);
      if (c) ask('Delete unused preset ' + c.name + '? Ticket history cannot be deleted.', async () => {
        await call('superagent_delete',{ orchestrationId:c.id, expectedRevision:c.policyRevision });
        selected = ''; selectedDemandWorkerId = ''; demandPreflight = null;
        await reload();
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
    if (target.hasAttribute('data-sa-source-poll-team')) {
      const teamId = selected;
      if (!teamId || !sourcesLoaded ||
          !sources.some(src => src.mode === 'observe' && src.eventKind !== 'check_run.completed')) return;
      void work(async () => {
        const summary = await call<{
          orchestrationId:string; observationalOnly:boolean; skippedOff:number; skippedCheckRuns:number;
          observed:Array<{scanned:number;recorded:number;duplicates:number;selfIgnored:number;pagesFetched:number}>;
        }>('superagent_source_poll_team_authenticated', {orchestrationId:teamId});
        if (summary.orchestrationId !== teamId || !summary.observationalOnly ||
            !Array.isArray(summary.observed)) {
          throw new Error('Native Team Observe response does not match the selected Team.');
        }
        const counts = summary.observed.reduce((tot, item) => {
          tot.scanned += item.scanned;
          tot.recorded += item.recorded;
          tot.duplicates += item.duplicates;
          tot.selfIgnored += item.selfIgnored;
          tot.pagesFetched += item.pagesFetched;
          return tot;
        }, {scanned:0,recorded:0,duplicates:0,selfIgnored:0,pagesFetched:0});
        feedback = 'Authenticated Team Observe: ' + summary.observed.length + ' source(s), ' +
          counts.scanned + ' scanned across ' + counts.pagesFetched + ' page(s), ' +
          counts.recorded + ' recorded, ' + counts.duplicates + ' duplicate, ' +
          counts.selfIgnored + ' self-authored ignored. ' +
          summary.skippedOff + ' Off and ' + summary.skippedCheckRuns +
          ' Check Run source(s) skipped. No tasks or effects started.';
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
    if (target.dataset.saOpenAssignmentChat) {
      // Navigation is only offered for a currently selected, native Ticket-bound
      // assignment. The host independently rechecks the native bot thread.
      const snapshot = ticketDetails;
      const assignment = snapshot?.assignments.find(item =>
        item.id === target.dataset.saOpenAssignmentChat);
      if (!openAssignmentChat || !snapshot || snapshot.ticket.id !== inspectedTicket ||
          snapshot.ticket.orchestrationId !== selected || !assignment ||
          assignment.id !== inspectedAssignmentId || !assignment.botId ||
          !assignment.sessionId) return;
      void work(() => openAssignmentChat({
        teamId: selected, ticketId: snapshot.ticket.id,
        assignmentId: assignment.id, botId: assignment.botId,
        sessionId: assignment.sessionId,
      }));
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
        const reviewedText = importText;
        const document = JSON.parse(reviewedText);
        const verified = await call<Preview>('superagent_manifest_preview',{ document });
        // The textarea stays editable during native preflight. A late result
        // must not resurrect confirmation for a superseded document.
        if (importText !== reviewedText) {
          inspected = null; preview = null;
          throw new Error('Manifest changed during preview; preview again before importing.');
        }
        if (!verified.importedPresetsDisabled || !/^[a-f0-9]{64}$/.test(verified.reviewDigest)) {
          throw new Error('Native preview did not verify a disabled-only configuration and review digest.');
        }
        inspected = document; preview = verified;
      }); return;
    }
    if (target.hasAttribute('data-sa-import') && inspected) {
      const document = inspected, reviewedText = importText;
      ask('Import these configurations, disabled, without changing existing bots?', async () => {
        // Editing the textarea after preview invalidates an already open
        // confirmation. Never submit a document the user did not review.
        if (inspected !== document || !preview || importText !== reviewedText)
          throw new Error('Manifest changed since preview; preview again before importing.');
        const reviewDigest = preview.reviewDigest;
        const result = await call<ImportResult>('superagent_manifest_import',{
          document, confirmed:true, reviewDigest,
        });
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
    // Enter/submit events must not bypass disabled native-backed controls
    // while hydration or a previous native operation remains unresolved.
    if (loading || busy) return;
    const data = new FormData(form);
    if (form.dataset.saForm === 'demand-project-id') {
      const exact = String(data.get('projectId') ?? '').trim();
      if (exact.length > 128 || (exact && !/^[A-Za-z0-9_-]+$/.test(exact))) {
        error = 'Use an exact Project ID: letters, numbers, hyphens or underscores (max 128).';
        render();
        return;
      }
      demandProjectFilter = exact;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
          projectId:exact || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (form.dataset.saForm === 'demand-search') {
      const next = String(data.get('searchQuery') ?? '').trim();
      if (new TextEncoder().encode(next).length > 128 ||
          Array.from(next).some(char => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)) {
        return;
      }
      demandSearch = next;
      demandSearchDraft = next;
      allDemands = []; allDemandsLoaded = false; allDemandsHasMore = false;
      render();
      void work(async () => {
        const loaded = await fetchDemandPage({
          limit:200, orchestrationId:demandTeamFilter || undefined,
          stateFilter:demandStateFilter || undefined,
          searchQuery:demandSearch || undefined,
          projectId:demandProjectFilter || undefined,
        });
        allDemands = loaded; allDemandsHasMore = loaded.length === 200;
        allDemandsLoaded = true;
      });
      return;
    }
    if (form.dataset.saForm === 'global-review-accept') {
      const currentDecision = globalAcceptance;
      if (!currentDecision || currentDecision.filter !== reviewTeamFilter ||
          workspaceView !== 'review' ||
          !reviewQueue.some(item => item.id === currentDecision.id &&
            item.orchestrationId === currentDecision.teamId &&
            item.revision === currentDecision.revision &&
            item.state === 'waiting_approval')) return;
      ask('Accept native Worker evidence for Demand ' + currentDecision.id +
        ' at revision ' + currentDecision.revision +
        '? This completes the Council-Off Ticket only if all native finalization guards still pass.', async () => {
        if (globalAcceptance !== currentDecision || workspaceView !== 'review' ||
            currentDecision.filter !== reviewTeamFilter) {
          throw new Error('Review selection changed before confirmation.');
        }
        const fresh = await call<TicketDetails>('superagent_snapshot',
          {ticketId:currentDecision.id, limit:1});
        if (fresh.ticket.id !== currentDecision.id ||
            fresh.ticket.orchestrationId !== currentDecision.teamId ||
            fresh.ticket.revision !== currentDecision.revision ||
            fresh.ticket.state !== 'waiting_approval' ||
            fresh.ticket.evidence !== currentDecision.evidence) {
          throw new Error('Native review evidence changed before confirmation; refresh the inbox.');
        }
        if (fresh.finalizationGate === 'council_required' ||
            fresh.finalizationGate === 'effects_unresolved' ||
            fresh.finalizationGate === 'not_pending') {
          throw new Error('Native finalization gate changed; acceptance denied.');
        }
        try {
          const accepted = await call<Ticket>('superagent_accept', {
            ticketId:currentDecision.id, expectedRevision:currentDecision.revision,
          });
          if (accepted.id !== currentDecision.id ||
              accepted.orchestrationId !== currentDecision.teamId ||
              accepted.revision !== currentDecision.revision + 1 ||
              accepted.state !== 'completed') {
            throw new Error('Native acceptance returned an inconsistent Ticket; inspect the latest history.');
          }
          feedback = 'Native human acceptance recorded for Demand ' + currentDecision.id +
            '. No Council verdict or external effect was inferred.';
        } finally {
          // IPC may fail after committing a native decision; never retain an
          // active approval button until the exact authoritative inbox is read.
          globalAcceptance = null;
          reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
          const updated = await fetchReviewPage(reviewTeamFilter);
          reviewQueue = updated;
          reviewQueueHasMore = updated.length === 200;
          reviewQueueLoaded = true;
        }
      });
      return;
    }
    if (form.dataset.saForm === 'global-review-reject') {
      const currentDecision = globalRejection;
      const reason = String(data.get('reason') ?? '').trim();
      if (!currentDecision || currentDecision.filter !== reviewTeamFilter ||
          workspaceView !== 'review' ||
          !reviewQueue.some(item => item.id === currentDecision.id &&
            item.orchestrationId === currentDecision.teamId &&
            item.revision === currentDecision.revision &&
            item.state === 'waiting_approval')) return;
      if (!reason || new TextEncoder().encode(reason).length > 500 ||
          Array.from(reason).some(char => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)) {
        error = 'Enter a nonempty rejection reason of at most 500 bytes without control characters.';
        render(); return;
      }
      ask('Reject native evidence for Demand ' + currentDecision.id +
        ' at revision ' + currentDecision.revision +
        '? This records a negative human decision and blocks the Ticket; it cannot approve or run work.', async () => {
        if (globalRejection !== currentDecision || workspaceView !== 'review' ||
            currentDecision.filter !== reviewTeamFilter) {
          throw new Error('Review selection changed before confirmation.');
        }
        const fresh = await call<TicketDetails>('superagent_snapshot',
          {ticketId:currentDecision.id, limit:1});
        if (fresh.ticket.id !== currentDecision.id ||
            fresh.ticket.orchestrationId !== currentDecision.teamId ||
            fresh.ticket.revision !== currentDecision.revision ||
            fresh.ticket.state !== 'waiting_approval' ||
            fresh.ticket.evidence !== currentDecision.evidence) {
          throw new Error('Native review evidence changed before confirmation; refresh the inbox.');
        }
        try {
          const rejected = await call<Ticket>('superagent_reject', {
            ticketId:currentDecision.id,
            expectedRevision:currentDecision.revision, reason,
          });
          if (rejected.id !== currentDecision.id ||
              rejected.orchestrationId !== currentDecision.teamId ||
              rejected.revision !== currentDecision.revision + 1 ||
              rejected.state !== 'blocked') {
            throw new Error('Native rejection returned an inconsistent Ticket; inspect the latest history.');
          }
          feedback = 'Native evidence rejection recorded for Demand ' + currentDecision.id +
            '. It remains blocked until separately authorized recovery.';
        } finally {
          // Even a failed native call may have committed a decision before an
          // IPC error. Clear stale controls and re-read the native inbox.
          globalRejection = null;
          reviewQueue = []; reviewQueueLoaded = false; reviewQueueHasMore = false;
          const updated = await fetchReviewPage(reviewTeamFilter);
          reviewQueue = updated;
          reviewQueueHasMore = updated.length === 200;
          reviewQueueLoaded = true;
        }
      });
      return;
    }
    if (form.dataset.saForm === 'roster') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id ||
          cfg.enabled || cfg.policyEditBlocker || roleRoster.policyRevision!==cfg.policyRevision) return;
      const workerBotIds=[cfg.workerBotId,...workerOrder];
      // Disabled checked controls are intentionally omitted by FormData.
      // Retain the native roster draft so a paused reviewer is never silently
      // dropped from the persisted Team as a side effect of editing Workers.
      const reviewerBotIds=[...reviewerDraft];
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
    if (form.dataset.saForm === 'team-strategy') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id ||
          roleRoster.policyRevision!==cfg.policyRevision || cfg.enabled || cfg.policyEditBlocker) return;
      const purpose=String(data.get('purpose') ?? '').trim();
      const strategy=String(data.get('strategy') ?? '');
      if ([...purpose].length>400 || /[\x00-\x09\x0b-\x1f\x7f]/.test(purpose) ||
          !['balanced','economy','performance','security_critical','custom'].includes(strategy)) return;
      void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_team_strategy_update',{
          orchestrationId:cfg.id,expectedRevision:cfg.policyRevision,purpose,strategy,
        });
        await reload();
        workspaceView='teams';
      });
      return;
    }
    if (form.dataset.saForm === 'attempt-budget') {
      const cfg=current();
      if (!cfg || !roleRoster || roleRoster.orchestrationId!==cfg.id ||
          roleRoster.policyRevision!==cfg.policyRevision || cfg.enabled || cfg.policyEditBlocker) return;
      const maxWorkerAttempts=Number(data.get('maxWorkerAttempts'));
      if (!Number.isInteger(maxWorkerAttempts) ||
          maxWorkerAttempts<1 || maxWorkerAttempts>4) return;
      void work(async () => {
        roleRoster=await call<TeamRoster>('superagent_worker_attempt_limit_update',{
          orchestrationId:cfg.id,expectedRevision:cfg.policyRevision,maxWorkerAttempts,
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
      if (!existing || existing.enabled || existing.policyEditBlocker) return;
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
      const maxWorkerAttempts=Number(data.get('maxWorkerAttempts'));
      const purpose=String(data.get('purpose') ?? '');
      const strategy=String(data.get('strategy') ?? 'balanced');
      void work(async () => {
        if (purpose.length > 400 || /[\x00-\x09\x0B-\x1F\x7F]/.test(purpose))
          throw new Error('Team purpose must be at most 400 characters and contain no unsafe controls.');
        if (!['balanced','economy','performance','security_critical','custom'].includes(strategy))
          throw new Error('Choose a supported optimization strategy.');
        if (![1,2,3,4].includes(maxWorkerAttempts))
          throw new Error('Choose a Worker-attempt limit from 1 to 4.');
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
        const created = await call<Config>('superagent_create',{
          input,roster,maxWorkerAttempts,purpose,strategy,
        });
        draftPresetName = ''; draftPresetTemplate = ''; draftWorkerAttemptLimit = 2;
        draftPresetPurpose = ''; draftPresetStrategy = 'balanced';
        draftCoordinatorBotId = ''; draftWorkerBotId = '';
        draftAdditionalWorkerIds = []; draftReviewerBotIds = []; draftMembersExpanded = false;
        selectedDemandWorkerId = ''; demandPreflight = null;
        workspaceView = 'runs'; await reloadCreatedTeam(created,roster);
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
      const coordinated = data.get('mode') !== 'direct';
      const isMulti=(selectedConfig?.workerCount ?? 1)>1;
      const workerId=isMulti ? selectedDemandWorkerId : selectedConfig?.workerBotId ?? '';
      const command = coordinated ? 'superagent_submit_coordinated'
        : isMulti ? 'superagent_submit_to_worker' : 'superagent_submit';
      const reviewed = demandPreflight;
      if (isMulti && (coordinated || roleRoster?.orchestrationId !== selectedConfig?.id ||
          !roleRoster?.workerBotIds.includes(workerId))) return;
      if (!selectedConfig?.enabled || !objective || !reviewed ||
          reviewed.orchestrationId !== selectedConfig.id ||
          reviewed.policyRevision !== selectedConfig.policyRevision ||
          reviewed.selectedRole !== (coordinated ? 'coordinator' : 'worker') ||
          reviewed.selectedBotId !== (coordinated ? selectedConfig.coordinatorBotId : workerId) ||
          preflightObjective !== objective ||
          preflightMode !== (coordinated ? 'coordinated' : 'direct') ||
          (!reviewed.eligible && !reviewed.canStageBlocked) ||
          reviewed.allowedSideEffects || reviewed.allowedTools.length !== 0) {
        error = 'Review an eligible native read-only preflight before submitting this Demand.';
        render();
        return;
      }
      const key = taskKey;
      ask((reviewed.canStageBlocked ? 'Submit a manual read-only Demand for busy Team ' :
        'Start this manual read-only Demand for Team ') + selectedConfig.name +
        ' at policy revision ' + reviewed.policyRevision +
        (reviewed.canStageBlocked
          ? '? A blocked Ticket is stored if still busy. If capacity opens before native admission, the Worker may start immediately.'
          : '? Native admission will recheck scope and availability.') +
        ' No write grant is issued.', async () => {
        if (selected !== selectedConfig.id || demandPreflight !== reviewed ||
            (isMulti && selectedDemandWorkerId !== workerId) ||
            demandObjective.trim() !== objective ||
            preflightObjective !== objective ||
            preflightMode !== (coordinated ? 'coordinated' : 'direct')) {
          throw new Error('Demand or Team changed before confirmation. Review native preflight again.');
        }
        // A cached preview never confers authority. Fresh native preflight is
        // followed by the native submit's own authorization and lease checks.
        const latest = isMulti
          ? await call<ManualPreflight>('superagent_preflight_worker', {
            orchestrationId:selectedConfig.id,objective,workerBotId:workerId,
          })
          : await call<ManualPreflight>('superagent_preflight', {
            orchestrationId:selectedConfig.id,objective,coordinated,
          });
        if ((!latest.eligible && !latest.canStageBlocked) ||
            latest.eligible !== reviewed.eligible ||
            Boolean(latest.canStageBlocked) !== Boolean(reviewed.canStageBlocked) ||
            latest.orchestrationId !== reviewed.orchestrationId ||
            latest.policyRevision !== reviewed.policyRevision ||
            latest.projectId !== reviewed.projectId ||
            latest.selectedRole !== reviewed.selectedRole ||
            latest.selectedBotId !== reviewed.selectedBotId ||
            latest.attemptBudgetMs !== reviewed.attemptBudgetMs ||
            latest.allowedTools.length !== 0 || latest.allowedSideEffects) {
          demandPreflight = null;
          throw new Error('Native admission scope or availability changed. Review a fresh preflight.');
        }
        await call(command,{ orchestrationId:selectedConfig.id, objective, requestKey:key,
          ...(isMulti ? {workerBotId:workerId} : {}),
        });
        taskKey = crypto.randomUUID(); demandObjective = ''; demandPreflight = null;
        await reload();
      });
    }
  });
  render();
  void reload();
  return { refresh: reload };
}
