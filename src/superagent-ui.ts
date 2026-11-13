import './superagent.css';
import { escape } from './dom';

// This interface only invokes native policy/authorization decisions. It cannot
// grant tools, infer work, start tasks on import or bypass Council review.
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
interface Bot { id: string; name: string; paused: boolean }
interface Config {
  id: string; name: string; coordinatorBotId: string; workerBotId: string;
  workerBotIds?: string[]; reviewerBotIds?: string[]; teamVersion?: number;
  enabled: boolean; policyRevision: number;
  ticketCount?: number; runningCount?: number; blockedCount?: number; lastActivityAt?: number;
}
interface Ticket { id: string; objective: string; state: string; revision: number; evidence: string | null }
interface TeamSnapshot {
  [key: string]: unknown;
  snapshotVersion?: unknown; provenance?: unknown; orchestrationId?: unknown;
  teamVersion?: unknown; name?: unknown; projectId?: unknown; coordinatorBotId?: unknown;
  workerBotIds?: unknown; reviewerBotIds?: unknown; policy?: unknown; roles?: unknown;
}
interface TeamDraft {
  name: string; coordinatorBotId: string; workerBotIds: string[]; reviewerBotIds: string[];
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
interface Assignment { id: string; botId: string; sessionId: string; state: string; attempt: number }
interface Activity { id: number; kind: string; detail: string; at: number }
interface CouncilObservation {
  nativeStatus: string; nativeRound: number; nativeStale: boolean;
  nativePassVerified: boolean; ticketArtifactBound: boolean;
}
interface TicketDetails {
  ticket: Ticket; mandate: Mandate; assignments: Assignment[];
  activity: Activity[]; teamSnapshot?: TeamSnapshot | null; council?: CouncilObservation | null;
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
  sourceId: string; ticketId: string; repository: string; subjectKey: string;
  actorFilter: string; eventKind: string; mode: 'off' | 'observe'; createdAt: number;
}
interface SourceDecision {
  sourceId: string; deliveryId: string; actor: string; eventKind: string; subjectKey: string;
  decision: string; sequence: number; policyRevision: number; at: number;
}
interface ObserveFetchDecision { decision: string; duplicate: boolean }
interface PreviewObserveFetch { previewOnly: true; message: string }
const OBSERVE_EVENT_KINDS = [
  'issues.opened','issues.edited','issue_comment.created','pull_request.opened',
  'pull_request_review.submitted','check_run.completed',
] as const;
const OBSERVE_FETCH_KINDS = new Set<string>(OBSERVE_EVENT_KINDS);
const MAX_TEAM_WORKERS = 16;
const MAX_TEAM_REVIEWERS = 8;

async function issueCommentEffectKey(
  ticketId: string, repository: string, issueNumber: number, comment: string,
): Promise<string> {
  const payload = new TextEncoder().encode(JSON.stringify([ticketId, repository, issueNumber, comment]));
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', payload));
  return 'github-comment:' + Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}

export function mountSuperagent(host: HTMLElement, call: Call): void {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let selected = '', busy = false, loading = true, error = '', feedback = '', importText = '';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let writeGrantTicket = '', writeGrantRevision: number | null = null;
  let writePathsDraft = '', writePathsError = '';
  let commentRepository = '', commentIssueNumber = '', commentDraft = '';
  let issueCommentError = '', issueCommentStatus: IssueCommentStatus | null = null;
  // Persist the user's editable preset draft across asynchronous list refreshes.
  let rejectingTicket = '', inspectedTicket = '';
  let ticketDetails: TicketDetails | null = null;
  let draftPresetName = '', draftPresetTemplate = '', draftCoordinatorBotId = '';
  let draftWorkerBotIds = [''], draftReviewerBotIds: string[] = [];
  const editTeamDrafts = new Map<string, TeamDraft>();
  let sources: EventSource[] = [], sourcesLoaded = false, decisionSourceId = '';
  let decisions: SourceDecision[] = [], threadLinks: ThreadLink[] = [];
  const safe = (value: unknown) => escape(String(value ?? ''));
  const current = () => presets.find(item => item.id === selected);
  const botName = (id: string) => bots.find(bot => bot.id === id)?.name ?? 'Missing bot';
  const activityTime = (timestamp?: number): string => typeof timestamp === 'number' &&
    Number.isFinite(timestamp) && timestamp > 0
    ? new Date(timestamp).toLocaleString() : 'Unavailable';
  const readTeamDraft = (form: HTMLFormElement): TeamDraft => {
    const data = new FormData(form);
    return {
      name: String(data.get('name') ?? '').trim(),
      coordinatorBotId: String(data.get('coordinatorBotId') ?? ''),
      workerBotIds: data.getAll('workerBotIds').map(String),
      reviewerBotIds: data.getAll('reviewerBotIds').map(String),
    };
  };
  const rememberTeamDraft = (form: HTMLFormElement, draft: TeamDraft): void => {
    if (form.dataset.saForm === 'create') {
      draftPresetName = draft.name;
      draftCoordinatorBotId = draft.coordinatorBotId;
      draftWorkerBotIds = draft.workerBotIds.length ? draft.workerBotIds : [''];
      draftReviewerBotIds = draft.reviewerBotIds;
    } else if (form.dataset.saForm === 'edit' && form.dataset.orchestrationId) {
      editTeamDrafts.set(form.dataset.orchestrationId, draft);
    }
  };
  const validateTeamDraft = (draft: TeamDraft): TeamDraft => {
    if (!draft.name || !draft.coordinatorBotId || !draft.workerBotIds.length ||
        draft.workerBotIds.some(id => !id) || draft.reviewerBotIds.some(id => !id)) {
      throw new Error('Choose a Team name, one coordinator, and at least one worker.');
    }
    if (draft.workerBotIds.length > MAX_TEAM_WORKERS || draft.reviewerBotIds.length > MAX_TEAM_REVIEWERS) {
      throw new Error('A Team supports up to 16 workers and 8 reviewers.');
    }
    const roleIds = [draft.coordinatorBotId, ...draft.workerBotIds, ...draft.reviewerBotIds];
    if (new Set(roleIds).size !== roleIds.length) throw new Error('A bot can have only one role in a Team.');
    return draft;
  };
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
    const list = presets.map(item =>
      {
      const workerIds = item.workerBotIds?.length ? item.workerBotIds : [item.workerBotId];
      const reviewerIds = item.reviewerBotIds ?? [];
      const canEdit = !item.enabled && (item.ticketCount ?? 0) === 0;
      const roles = safe(botName(item.coordinatorBotId)) + ' · Workers: ' +
        safe(workerIds.map(botName).join(', ')) + (reviewerIds.length
          ? ' · Reviewers: ' + safe(reviewerIds.map(botName).join(', ')) : '');
      return '<li class="sa-item"><div><strong>' + safe(item.name) + '</strong><small>' +
      roles +
      '</small><small class="sa-counts" aria-label="Task counts">' +
      (item.ticketCount ?? 0) + ' tasks · ' + (item.runningCount ?? 0) +
      ' running · ' + (item.blockedCount ?? 0) + ' blocked</small>' +
      '<small class="sa-last-activity">Last activity: ' + safe(activityTime(item.lastActivityAt)) +
      '</small></div><span>' +
      (item.enabled ? 'Manual' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button>' + (canEdit
        ? '<button type="button" data-sa-edit-team="' + safe(item.id) + '">Edit Team</button>' : '') +
      '</li>';
      }
    ).join('');
    const optionsFor = (botId: string) => bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + (bot.id === botId ? ' selected' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const roleEditor = (kind: 'worker' | 'reviewer', ids: string[]) => {
      const title = kind === 'worker' ? 'Workers (ordered)' : 'Reviewers (ordered)';
      const max = kind === 'worker' ? MAX_TEAM_WORKERS : MAX_TEAM_REVIEWERS;
      const rows = ids.map((id, index) => {
        const label = kind === 'worker' ? 'Worker ' + (index + 1) : 'Reviewer ' + (index + 1);
        const name = kind === 'worker' ? 'workerBotIds' : 'reviewerBotIds';
        return '<li><label>' + label + '<select name="' + name + '" data-sa-team-role="' + kind + '" required>' +
          '<option value="">Choose bot</option>' + optionsFor(id) + '</select></label><div class="sa-actions">' +
          '<button type="button" data-sa-role-action="up" data-sa-role="' + kind + '" data-sa-role-index="' + index + '" aria-label="Move ' +
          label.toLowerCase() + ' up"' + (index === 0 ? ' disabled' : '') + '>Move up</button>' +
          '<button type="button" data-sa-role-action="down" data-sa-role="' + kind + '" data-sa-role-index="' + index + '" aria-label="Move ' +
          label.toLowerCase() + ' down"' + (index === ids.length - 1 ? ' disabled' : '') + '>Move down</button>' +
          (kind === 'worker' && ids.length === 1 ? '' : '<button type="button" data-sa-role-action="remove" data-sa-role="' +
            kind + '" data-sa-role-index="' + index + '" aria-label="Remove ' + label.toLowerCase() + '">Remove</button>') +
          '</div></li>';
      }).join('');
      return '<div class="sa-team-role-editor" role="group" aria-label="' + title + '"><h4>' + title + '</h4><ol>' +
        (rows || '<li>No reviewers selected.</li>') + '</ol><button type="button" data-sa-role-add="' + kind + '"' +
        (ids.length >= max ? ' disabled' : '') + '>Add ' + kind + '</button></div>';
    };
    const chosen = current();
    const selectedTeamDraft = chosen ? editTeamDrafts.get(chosen.id) ?? {
      name: chosen.name,
      coordinatorBotId: chosen.coordinatorBotId,
      workerBotIds: chosen.workerBotIds?.length ? chosen.workerBotIds : [chosen.workerBotId],
      reviewerBotIds: chosen.reviewerBotIds ?? [],
    } : null;
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const writeTicket = tickets.find(ticket => ticket.id === writeGrantTicket);
    const canComment = !!inspected && ticketDetails?.ticket.id === inspected.id &&
      inspected.state === 'running' && !ticketDetails.mandate.revoked &&
      ticketDetails.mandate.allowedSideEffects;
    const commentStatus = issueCommentStatus && inspected &&
      issueCommentStatus.ticketId === inspected.id &&
      issueCommentStatus.ticketRevision === inspected.revision ? issueCommentStatus : null;
    const details = inspected && ticketDetails
      ? '<section class="sa-pane" role="region" aria-label="Ticket evidence">' +
        '<h4>Ticket evidence</h4><p>Read-only native snapshot. Model output is not task approval.</p>' +
        renderTeamSnapshot(ticketDetails.teamSnapshot) +
        '<dl><dt>Ticket ID</dt><dd><code>' + safe(inspected.id) + '</code></dd>' +
        '<dt>State / revision</dt><dd>' + safe(inspected.state) + ' / ' + inspected.revision + '</dd>' +
        '<dt>Actor</dt><dd>' + safe(ticketDetails.mandate.actor) + '</dd>' +
        '<dt>Authorization</dt><dd><code>' + safe(ticketDetails.mandate.authorizationId) + '</code></dd>' +
        '<dt>Project / policy</dt><dd>' + safe(ticketDetails.mandate.projectId) + ' / ' +
        ticketDetails.mandate.policyRevision + '</dd>' +
        '<dt>Mandate</dt><dd>' + (ticketDetails.mandate.revoked ? 'Revoked' : 'Issued') +
        ' · ' + (ticketDetails.mandate.allowedSideEffects ? 'Scoped side effects' : 'No side effects allowed') +
        ' · Tools: ' + safe(ticketDetails.mandate.allowedTools.join(', ') || 'none') + '</dd>' +
        '<dt>Expires</dt><dd>' + safe(new Date(ticketDetails.mandate.expiresAt).toLocaleString()) + '</dd></dl>' +
        '<h5>Assignment attempts</h5><ul>' +
        (ticketDetails.assignments.map(item => '<li>Attempt ' + item.attempt + ' · ' +
          safe(item.state) + ' · Bot ' + safe(botName(item.botId)) + ' · Session <code>' +
          safe(item.sessionId) + '</code></li>').join('') || '<li>No worker assigned.</li>') +
        '</ul><h5>Native activity</h5><ol>' +
        (ticketDetails.activity.map(entry => '<li>' + safe(activityTime(entry.at)) + ' · ' +
          safe(entry.kind) + ' · ' + safe(entry.detail) + '</li>').join('') ||
          '<li>No activity recorded.</li>') +
        '</ol><h5>Linked GitHub threads</h5><ul class="sa-linked-threads">' +
        (threadLinks.map(link => '<li><span>' + safe(link.repository) + ' / ' +
          safe(link.subjectKey) + ' · ' + safe(link.eventKind) + ' · actor ' +
          safe(link.actorFilter) + '</span>' + (link.mode === 'observe'
            ? (OBSERVE_FETCH_KINDS.has(link.eventKind)
              ? '<button type="button" data-sa-fetch-thread="' + safe(link.sourceId) +
              '" data-sa-subject-key="' + safe(link.subjectKey) + '" aria-label="Fetch one latest matching GitHub event for ' +
                safe(link.repository) + ' ' + safe(link.subjectKey) + '">Fetch once</button>'
              : '<small>Manual fetch unavailable for this event class.</small>')
            : '<small>Off · no GitHub request</small>') + '</li>').join('') ||
          '<li>No threads linked.</li>') + '</ul>' +
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
        (sourcesLoaded && sources.length
          ? '<form data-sa-form="thread-link"><label>Event source<select name="sourceId" required>' +
            sources.map(src => '<option value="' + safe(src.id) + '">' +
              safe(src.repository) + ' · ' + safe(src.eventKind) + '</option>').join('') +
            '</select></label><label>Exact thread key<input name="subjectKey" required ' +
            'maxlength="24" placeholder="issue:42 or pull:123"></label>' +
            '<button type="submit">Link thread (context only)</button></form>'
          : '<p>Load event sources below before linking a thread.</p>') +
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
      '<button type="button" data-sa-source-decisions="' + safe(src.id) +
      '">View decisions</button></li>').join('');
    const sourcePanel = '<section class="sa-event-sources" aria-label="GitHub event sources">' +
      '<h4>GitHub event sources</h4><p>Off/Observe only. Fetch once is a user-clicked read of one linked thread using your signed-in <code>gh</code> CLI. No polling, Auto tasks or external writes; Off makes no request.</p>' +
      (!sourcesLoaded ? '<button type="button" data-sa-load-sources>Load sources</button>'
        : '<form data-sa-form="source"><div class="sa-grid">' +
          '<label>Repository (owner/name)<input name="repository" maxlength="200" required></label>' +
          '<label>Exact actor<input name="actorFilter" maxlength="80" required></label></div>' +
          '<p>For <code>check_run.completed</code>, use the owning GitHub App slug as the actor.</p>' +
          '<label>Event class<select name="eventKind">' +
          OBSERVE_EVENT_KINDS
            .map(kind => '<option value="' + kind + '">' + kind + '</option>').join('') +
          '</select></label><button type="submit">Add source (Off)</button></form>' +
          '<ul class="sa-tickets sa-source-list">' +
          (sourceRows || '<li class="sa-empty">No event sources configured.</li>') + '</ul>' +
          (decisionSourceId ? '<div role="region" aria-label="Source decisions">' +
            '<h5>Recorded decisions</h5><ol>' +
            (decisions.map(d => '<li>' + safe(activityTime(d.at)) + ' · ' +
              safe(d.decision) + ' · ' + safe(d.eventKind) + ' · actor ' + safe(d.actor) + ' · ' +
              safe(d.subjectKey) + ' · Delivery ' + safe(d.deliveryId) +
              ' · Revision ' + d.policyRevision + '</li>').join('') ||
              '<li>No recorded events.</li>') + '</ol></div>' : '')) + '</section>';
    const active = chosen ? '<section class="sa-pane" aria-label="Selected orchestration"><header><h3>' +
      safe(chosen.name) + '</h3><div class="sa-actions"><button type="button" data-sa-enable="' +
      safe(chosen.id) + '">' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused</button></div></header>' +
      '<form data-sa-form="edit" data-orchestration-id="' + safe(chosen.id) + '"><label>Team name<input name="name" required maxlength="120" value="' + safe(selectedTeamDraft?.name) + '"></label>' +
      '<div class="sa-grid"><label>Coordinator<select name="coordinatorBotId" required>' +
      '<option value="">Choose bot</option>' + optionsFor(selectedTeamDraft?.coordinatorBotId ?? '') + '</select></label></div>' +
      roleEditor('worker', selectedTeamDraft?.workerBotIds ?? []) + roleEditor('reviewer', selectedTeamDraft?.reviewerBotIds ?? []) +
      '<button class="primary-button" type="submit"' +
      (chosen.enabled || (chosen.ticketCount ?? 0) > 0 ? ' disabled' : '') + '>Save Team version</button></form>' +
      (chosen.ticketCount ? '<p>Team roles lock after its first Ticket.</p>' : '') +
      '<p>Policy revision ' + chosen.policyRevision + ' · Native read-only permission until separately authorized.</p>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required></textarea></label>' +
      '<label>Dispatch<select name="mode"><option value="coordinated">Coordinator, then worker</option>' +
      '<option value="direct">Worker directly</option></select></label>' +
      '<button class="primary-button" type="submit"' + (chosen.enabled ? '' : ' disabled') +
      '>Dispatch manually</button></form>' +
      '<div class="sa-title"><h4>Tickets</h4><button type="button" data-sa-refresh>Refresh</button></div>' +
      '<ul class="sa-tickets">' + (tickets.map(ticket =>
        '<li><div><strong>' + safe(ticket.objective) + '</strong><small>' + safe(ticket.state) +
        ' · Revision ' + ticket.revision + '</small></div>' +
        '<div class="sa-ticket-actions"><button type="button" data-sa-inspect-ticket="' + safe(ticket.id) +
        '" aria-expanded="' + (ticket.id === inspectedTicket) + '">View evidence</button>' +
        (ticket.state === 'waiting_approval'
          ? '<button type="button" data-sa-accept="' + safe(ticket.id) + '">Accept evidence</button>' +
            '<button type="button" data-sa-reject="' + safe(ticket.id) + '">Reject evidence</button>' : '') +
        (ticket.state === 'queued' ? '<button type="button" data-sa-dispatch="' + safe(ticket.id) + '">Dispatch worker</button>' +
          '<button type="button" data-sa-write-request="' + safe(ticket.id) + '">Request file writes</button>' : '') +
        (ticket.state === 'blocked' ? '<button type="button" data-sa-retry="' + safe(ticket.id) + '">Retry manually</button>' : '') +
        (!['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)
          ? '<button type="button" data-sa-stop="' + safe(ticket.id) + '">Stop</button>' : '') +
        '</div>' +
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
        details + sourcePanel + '</section>' : '';
    host.innerHTML = '<div class="sa-ui">' +
      '<p class="sa-intro">Native SuperAgent uses existing bots. Imported presets stay Off, and all task execution is manual.</p>' +
      (loading ? '<p role="status">Loading orchestrations…</p>' : '') +
      (error ? '<p role="alert" class="sa-error">' + safe(error) + '</p>' : '') +
      (feedback ? '<p role="status" class="sa-feedback">' + safe(feedback) + '</p>' : '') +
      (confirm ? '<section role="alertdialog" aria-label="Confirm orchestration action" class="sa-confirm"><p>' +
        safe(confirm.message) + '</p><div class="sa-actions"><button type="button" data-sa-cancel>Cancel</button>' +
        '<button class="primary-button" type="button" data-sa-confirm>Confirm</button></div></section>' : '') +
      '<section class="sa-pane"><h3>Orchestrations</h3><ul class="sa-list">' +
      (list || '<li class="sa-empty">No orchestrations created.</li>') + '</ul></section>' +
      '<section class="sa-pane"><h3>Team Builder</h3>' +
      '<p>Membership is saved in order. The existing manual dispatch path does not schedule multiple Workers, and Reviewer membership does not run Council review.</p>' +
      '<form data-sa-form="create">' +
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
      '<option value="">Choose bot</option>' + optionsFor(draftCoordinatorBotId) + '</select></label></div>' +
      roleEditor('worker', draftWorkerBotIds) + roleEditor('reviewer', draftReviewerBotIds) +
      '<button class="primary-button" type="submit"' +
      (bots.length < 2 ? ' disabled' : '') + '>Create disabled preset</button></form></section>' +
      active +
      '<section class="sa-pane"><h3>Portable configuration</h3>' +
      '<p>Configuration only: no credentials, memories, conversations, approvals or live tasks.</p>' +
      '<label>Companion manifest JSON<textarea data-sa-manifest rows="5" spellcheck="false">' +
      safe(importText) + '</textarea></label>' +
      '<div class="sa-actions"><button type="button" data-sa-export>Export</button>' +
      '<button type="button" data-sa-preview>Preview import</button></div>' +
      (preview ? '<p role="status">' + preview.wouldCreate.length + ' new; ' +
        preview.unchanged.length + ' unchanged. Imported presets will be disabled.</p>' +
        '<button class="primary-button" type="button" data-sa-import>Confirm import</button>' : '') +
      '</section></div>';
    host.setAttribute('aria-busy', String(busy));
  }
  async function refreshTicketDetails(): Promise<void> {
    const id = inspectedTicket;
    if (!id || !tickets.some(ticket => ticket.id === id)) {
      inspectedTicket = ''; ticketDetails = null; threadLinks = []; issueCommentStatus = null; return;
    }
    const previousRevision = ticketDetails?.ticket.id === id ? ticketDetails.ticket.revision : null;
    // Use existing native read endpoints, not model-generated summaries. A
    // revision change refreshes the snapshot before a later human decision.
    const [snapshot, links] = await Promise.all([
      call<TicketDetails>('superagent_snapshot', { ticketId:id, limit:50 }),
      call<ThreadLink[]>('superagent_source_thread_links', { ticketId:id }),
    ]);
    if (inspectedTicket === id) {
      if (previousRevision !== null && previousRevision !== snapshot.ticket.revision) issueCommentStatus = null;
      ticketDetails = snapshot;
      threadLinks = links;
      // Keep the ticket row and its grant/history pinned to one native revision.
      const index = tickets.findIndex(ticket => ticket.id === id);
      if (index >= 0) tickets[index] = snapshot.ticket;
    }
  }
  async function reload(): Promise<void> {
    loading = true;
    // Keep updated controls hidden until the mutating operation has settled.
    // Exposing them mid-reload would allow silently discarded submissions.
    if (!busy) render();
    try {
      const [known, configs] = await Promise.all([call<Bot[]>('bots_list'), call<Config[]>('superagent_list')]);
      bots = known; presets = configs;
      if (!presets.some(config => config.id === selected)) {
        selected = presets[0]?.id ?? '';
        inspectedTicket = ''; threadLinks = [];
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      }
      // A failed native read must not leave old ticket approval controls
      // visible underneath a newly selected or reloaded preset.
      tickets = [];
      ticketDetails = null;
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      await refreshTicketDetails();
      if (sourcesLoaded && selected) {
        sources = await call<EventSource[]>('superagent_source_list',{ orchestrationId:selected });
        if (!sources.some(item => item.id === decisionSourceId)) {
          decisionSourceId = ''; decisions = [];
        } else {
          decisions = await call<SourceDecision[]>('superagent_source_decisions',
            {sourceId:decisionSourceId,limit:50});
        }
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
    if (!(target instanceof HTMLSelectElement)) return;
    if (target.hasAttribute('data-sa-template')) {
      draftPresetTemplate = target.value;
      draftPresetName = target.value;
      const field = target.form?.querySelector<HTMLInputElement>('input[name="name"]');
      if (field) field.value = draftPresetName;
    }
    const form = target.closest<HTMLFormElement>('form[data-sa-form="create"],form[data-sa-form="edit"]');
    if (form) rememberTeamDraft(form, readTeamDraft(form));
  });
  host.addEventListener('input', event => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.name === 'name') {
      const form = target.closest<HTMLFormElement>('form[data-sa-form="create"],form[data-sa-form="edit"]');
      if (form) {
        const draft = readTeamDraft(form);
        draft.name = target.value.trim();
        rememberTeamDraft(form, draft);
      }
    }
    if (target instanceof HTMLTextAreaElement && target.hasAttribute('data-sa-manifest')) {
      importText = target.value; preview = null; inspected = null;
    }
    if (target instanceof HTMLTextAreaElement && target.hasAttribute('data-sa-write-paths')) {
      writePathsDraft = target.value; writePathsError = '';
    }
    if (target instanceof HTMLInputElement && target.form?.dataset.saForm === 'issue-comment') {
      if (target.name === 'repository') commentRepository = target.value;
      if (target.name === 'issueNumber') commentIssueNumber = target.value;
      issueCommentError = ''; issueCommentStatus = null;
      target.form?.querySelector('[data-sa-comment-error]')?.remove();
      target.form?.querySelector('[data-sa-comment-status]')?.remove();
    }
    if (target instanceof HTMLTextAreaElement && target.form?.dataset.saForm === 'issue-comment') {
      commentDraft = target.value; issueCommentError = ''; issueCommentStatus = null;
      target.form?.querySelector('[data-sa-comment-error]')?.remove();
      target.form?.querySelector('[data-sa-comment-status]')?.remove();
    }
  });
  host.addEventListener('click', event => {
    const target = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('button') : null;
    if (!target || !host.contains(target) || target.disabled) return;
    if (target.hasAttribute('data-sa-cancel')) { confirm = null; render(); return; }
    if (target.hasAttribute('data-sa-write-cancel')) {
      writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = ''; writePathsError = '';
      render(); return;
    }
    if (target.hasAttribute('data-sa-confirm')) {
      const action = confirm?.action; confirm = null; if (action) void work(action);
      return;
    }
    if (target.dataset.saRoleAdd || target.dataset.saRoleAction) {
      const form = target.closest<HTMLFormElement>('form[data-sa-form="create"],form[data-sa-form="edit"]');
      const kind = target.dataset.saRole ?? target.dataset.saRoleAdd;
      if (!form || (kind !== 'worker' && kind !== 'reviewer')) return;
      const draft = readTeamDraft(form);
      const ids = kind === 'worker' ? draft.workerBotIds : draft.reviewerBotIds;
      const max = kind === 'worker' ? MAX_TEAM_WORKERS : MAX_TEAM_REVIEWERS;
      if (target.dataset.saRoleAdd) {
        if (ids.length < max) ids.push('');
      } else {
        const index = Number(target.dataset.saRoleIndex);
        const action = target.dataset.saRoleAction;
        if (!Number.isInteger(index) || index < 0 || index >= ids.length) return;
        if (action === 'remove') {
          if (kind === 'reviewer' || ids.length > 1) ids.splice(index, 1);
        } else if (action === 'up' && index > 0) {
          [ids[index - 1], ids[index]] = [ids[index], ids[index - 1]];
        } else if (action === 'down' && index < ids.length - 1) {
          [ids[index + 1], ids[index]] = [ids[index], ids[index + 1]];
        }
      }
      if (kind === 'worker') draft.workerBotIds = ids;
      else draft.reviewerBotIds = ids;
      rememberTeamDraft(form, draft);
      render();
      return;
    }
    if (target.dataset.saSelect) {
      if (busy || loading || selected === target.dataset.saSelect) return;
      const next = target.dataset.saSelect;
      selected = next;
      tickets = [];
      writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = ''; writePathsError = '';
      inspectedTicket = ''; ticketDetails = null; threadLinks = [];
      sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
      void work(async () => {
        tickets = await call<Ticket[]>('superagent_tickets',{ orchestrationId:next, limit:50 });
      });
      return;
    }
    if (target.dataset.saEditTeam) {
      const team = presets.find(item => item.id === target.dataset.saEditTeam);
      if (!team || team.enabled || (team.ticketCount ?? 0) > 0) return;
      editTeamDrafts.set(team.id, {
        name: team.name,
        coordinatorBotId: team.coordinatorBotId,
        workerBotIds: team.workerBotIds?.length ? [...team.workerBotIds] : [team.workerBotId],
        reviewerBotIds: [...(team.reviewerBotIds ?? [])],
      });
      if (selected !== team.id) {
        selected = team.id;
        tickets = []; inspectedTicket = ''; ticketDetails = null; threadLinks = [];
        writeGrantTicket = ''; writeGrantRevision = null; writePathsDraft = ''; writePathsError = '';
        sourcesLoaded = false; sources = []; decisionSourceId = ''; decisions = [];
        void work(async () => {
          tickets = await call<Ticket[]>('superagent_tickets',{ orchestrationId:team.id, limit:50 });
          await refreshTicketDetails();
        });
      } else render();
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
    if (target.dataset.saSourceDecisions) {
      const id = target.dataset.saSourceDecisions;
      if (sources.some(item => item.id === id)) void work(async () => {
        const loaded = await call<SourceDecision[]>('superagent_source_decisions',
          {sourceId:id,limit:50});
        decisionSourceId = id; decisions = loaded;
      });
      return;
    }
    if (target.dataset.saFetchThread) {
      const sourceId = target.dataset.saFetchThread;
      const subjectKey = target.dataset.saSubjectKey ?? '';
      const link = threadLinks.find(item => item.sourceId === sourceId && item.subjectKey === subjectKey);
      if (!link || link.mode !== 'observe') {
        feedback = 'This source is Off or the linked thread is no longer available; no GitHub request was made.';
        render();
        return;
      }
      void work(async () => {
        const fetched = await call<ObserveFetchDecision | PreviewObserveFetch>(
          'superagent_source_fetch_thread', {sourceId,subjectKey});
        if ('previewOnly' in fetched) {
          feedback = fetched.message;
          return;
        }
        sources = await call<EventSource[]>('superagent_source_list', {orchestrationId:selected});
        sourcesLoaded = true;
        decisionSourceId = sourceId;
        decisions = await call<SourceDecision[]>('superagent_source_decisions',
          {sourceId,limit:50});
        feedback = fetched.duplicate
          ? 'This GitHub event was already recorded. No Ticket was created or launched.'
          : 'Recorded ' + fetched.decision + ' for this one linked thread. No Ticket was created or launched.';
      });
      return;
    }
    if (target.hasAttribute('data-sa-refresh')) { void work(reload); return; }
    if (target.hasAttribute('data-sa-close-details')) {
      inspectedTicket = ''; ticketDetails = null; issueCommentStatus = null; render(); return;
    }
    if (target.dataset.saInspectTicket) {
      const id = target.dataset.saInspectTicket;
      if (inspectedTicket === id) {
        inspectedTicket = ''; ticketDetails = null; issueCommentStatus = null; render();
      } else {
        inspectedTicket = id; ticketDetails = null; issueCommentStatus = null;
        commentRepository = ''; commentIssueNumber = ''; commentDraft = ''; issueCommentError = '';
        void work(refreshTicketDetails);
      }
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
      if (!existing || existing.enabled || (existing.ticketCount ?? 0) > 0) return;
      const draft = readTeamDraft(form);
      void work(async () => {
        const team = validateTeamDraft(draft);
        const input = { ...team, workerBotId:team.workerBotIds[0] };
        await call('superagent_update',{ orchestrationId:existing.id, expectedRevision:existing.policyRevision, input });
        editTeamDrafts.delete(existing.id);
        await reload();
      });
      return;
    }
    if (form.dataset.saForm === 'create') {
      const draft = readTeamDraft(form);
      void work(async () => {
        const team = validateTeamDraft(draft);
        const input = { ...team, workerBotId:team.workerBotIds[0] };
        const created = await call<Config>('superagent_create',{ input });
        draftPresetName = ''; draftPresetTemplate = ''; draftCoordinatorBotId = '';
        draftWorkerBotIds = ['']; draftReviewerBotIds = [];
        selected = created.id; await reload();
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
      const command = data.get('mode') === 'direct' ? 'superagent_submit' : 'superagent_submit_coordinated';
      if (!selectedConfig?.enabled || !objective) return;
      const key = taskKey;
      void work(async () => {
        await call(command,{ orchestrationId:selectedConfig.id, objective, requestKey:key });
        taskKey = crypto.randomUUID(); await reload();
      });
    }
  });
  render();
  void reload();
}
