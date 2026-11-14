import type { Bot } from './bots';
import { botAvatarHtml } from './bot-avatars';
import { escape, icon } from './dom';
import { WORKING_LABELS } from './bot-thread';
import type { CouncilComposition, CouncilCriteriaSource, CouncilReviewerMode, CouncilStrategy, TeamSnapshot } from './architect-types';
import { councilCompositionValidationIssues } from './council';
import { councilLedgerHtmlForTeam } from './council-panel';

export interface SharedChannelSummary {
  id: string;
  name: string;
  memberIds: string[];
  leadBotId: string | null;
  archived: boolean;
  updatedAt: number;
}
export interface SharedChannelMessage {
  id: string;
  kind: 'user' | 'reply' | 'assigned' | 'bot_post' | 'bot_request' | 'direct_delegation';
  authorBotId: string | null;
  recipientIds?: string[];
  text: string;
  status: 'Queued' | 'In progress' | 'Completed' | 'Failed' | 'Stopped' | null;
  createdAt: number;
  council?: ChannelCouncilView;
}
export interface ChannelCouncilView {
  cycleId: string;
  sessionId: string;
  state: string;
  round: number;
  target: number;
  lowest: number | null;
  artifactRevision: string;
  notice?: string | null;
  reviewers: {
    botId: string | null; name: string; lens: string; round: number; score: number | null;
    skills?: {
      id: string; status: string; sha256?: string | null; sourceKind?: string | null;
      sourceRef?: string | null; sourceCommit?: string | null; required: boolean;
      nativeRetrieval: boolean; nativeCheckVerified: boolean; correctiveAction?: string | null;
    }[];
  }[];
  followupStatus?: string | null;
  followupUrl?: string | null;
  followupError?: string | null;
  acceptanceReason?: string | null;
  acceptedBy?: string | null;
}
export interface SharedChannelDetail extends SharedChannelSummary {
  messages: SharedChannelMessage[];
  activeBotIds: string[];
  /** Older history exists beyond the latest page returned. */
  hasMore?: boolean;
}
export interface SharedChannelUiOptions {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  bots: Bot[];
  modal(title: string, body: string, cls?: string): HTMLDialogElement;
  closeModal(): void;
  changed(): Promise<void>;
  openChannel(channelId: string): void;
  notify(text: string): void;
  loadCouncilSnapshot?: (sessionId: string) => Promise<TeamSnapshot | null>;
  bindCouncilEvidence?: (host: HTMLElement, sessionId: string) => void;
}

const channelName = (name: string) => `#${name.replace(/^#+/, '')}`;
const CHANNEL_COUNCIL_LIMITS = Object.freeze({
  maxDepth: 2, maxRootSeats: 8, maxConcurrentLeaves: 64,
  maxLeafReviewsPerRound: 128, timeoutMs: 86_400_000, maxEstimatedCostMicros: 1_000_000,
});

function faces(ids: string[], bots: Bot[], cls = ''): string {
  const members = ids.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot));
  if (!members.length) return `<span class="bc-empty-face" aria-hidden="true">${icon('bot')}</span>`;
  const shown = members.length > 4 ? members.slice(0, 3) : members;
  return `<span class="bc-faces ${cls}" aria-label="${escape(members.map(bot => bot.name).join(', '))}">${shown.map(bot => botAvatarHtml(bot.avatar, bot.name, 'bc-face', bot.color)).join('')}${members.length > 4 ? `<span class="bc-face-count">+${members.length - 3}</span>` : ''}</span>`;
}

export function channelFaceCluster(ids: string[], bots: Bot[]): string { return faces(ids, bots, 'bc-cluster'); }

export function createChannelBody(bots: Bot[]): string {
  return `<form class="bc-create-form"><p class="dialog-description">A shared conversation for a small group of bots.</p>
    <label class="field-label" for="bc-new-name">Channel name</label><input id="bc-new-name" name="name" type="text" maxlength="48" autocomplete="off" required placeholder="e.g. launch">
    <fieldset class="bc-member-list"><legend>Bots</legend>${bots.map(bot => `<label><input type="checkbox" name="member" value="${escape(bot.id)}">${botAvatarHtml(bot.avatar, bot.name, 'bc-pick-face', bot.color)}<span>${escape(bot.name)}</span></label>`).join('')}</fieldset>
    <div class="dialog-footer"><button type="button" class="secondary-button" data-shared-action="cancel">Cancel</button><button type="submit" class="primary-button">Create channel</button></div></form>`;
}

export function openCreateChannel(options: SharedChannelUiOptions): void {
  const dialog = options.modal('New channel', createChannelBody(options.bots), 'bot-shared-dialog');
  const form = dialog.querySelector<HTMLFormElement>('.bc-create-form')!;
  form.addEventListener('submit', event => {
    event.preventDefault();
    const data = new FormData(form), name = String(data.get('name') ?? '').trim().replace(/^#+/, '');
    if (!name) return;
    const memberIds = [...form.querySelectorAll<HTMLInputElement>('input[name=member]:checked')].map(input => input.value);
    void options.call<SharedChannelSummary>('bot_shared_channel_create', { name, memberIds }).then(async channel => {
      await options.changed();
      options.closeModal();
      options.openChannel(channel.id);
    }).catch(error => options.notify(error instanceof Error ? error.message : String(error)));
  });
  dialog.querySelector('[data-shared-action="cancel"]')?.addEventListener('click', options.closeModal);
}

function messageHtml(message: SharedChannelMessage, bots: Bot[]): string {
  if (message.kind === 'assigned' || message.kind === 'direct_delegation') {
    const bot = bots.find(item => item.id === message.authorBotId);
    const description = message.kind === "direct_delegation"
      ? `${bot?.name ?? "Bot"}: ${message.text}` : `Assigned to ${bot?.name ?? "bot"}`;
    const council = message.council;
    // Show only provenance safe for a channel: no local materialization paths,
    // private reviewer instructions or credentials. Details remain accessible
    // by keyboard/assistive technology without a second Customize registry.
    const reviewerSummary = council?.reviewers.map(reviewer => {
      const skills = reviewer.skills ?? [];
      const receipts = skills.map(skill => [
        skill.id, skill.status, skill.sha256 ? `SHA-256 ${skill.sha256}` : '',
        skill.sourceKind ?? '', skill.sourceRef ?? '', skill.sourceCommit ?? '',
        skill.nativeRetrieval ? 'retrieval recorded' : 'retrieval not verified',
        skill.nativeCheckVerified ? 'native check verified' : 'native check not verified',
        skill.correctiveAction ?? '',
      ].filter(Boolean).join(' · '));
      const skillNote = receipts.length
        ? ` <span class="bc-council-skill-receipts" role="note" tabindex="0" aria-label="${escape(receipts.join('; '))}" title="${escape(receipts.join('; '))}">Skills: ${skills.length}</span>`
        : '';
      return `${escape(reviewer.name)} (${escape(reviewer.lens ?? 'review')})${reviewer.score == null ? '' : ` ${reviewer.score.toFixed(1)}`}${skillNote}`;
    }).join(' · ') ?? '';
    const verdict = council ? `<span class="bc-council-summary" aria-label="Delivery Council status"><strong>Council: ${escape(council.state)}</strong>${council.round ? ` · round ${council.round}` : ''}${council.lowest != null ? ` · lowest ${council.lowest.toFixed(1)}/10` : ''}<span class="bc-council-reviewers">${reviewerSummary}</span>${council.notice ? `<span class="bc-council-notice">${escape(council.notice)}</span>` : ''}${council.acceptanceReason ? `<span class="bc-council-acceptance">Accepted by local user: ${escape(council.acceptanceReason)}</span>` : ''}${council.followupStatus ? `<span class="bc-council-followup">GitHub follow-up: ${escape(council.followupStatus)}${council.followupError ? ` · ${escape(council.followupError)}` : ''}${council.followupUrl?.startsWith('https://github.com/') ? ` · <a href="${escape(council.followupUrl)}" target="_blank" rel="noopener noreferrer">Open issue</a>` : ''}</span>` : ''}${council.sessionId ? `<button type="button" class="secondary-button bc-council-report" data-action="channel-council-report" data-session-id="${escape(council.sessionId)}">View Council report</button>` : ''}${council.round > 0 && ['Limit reached', 'Needs correction', 'Inconclusive'].includes(council.state) && message.status !== 'Stopped' && message.status !== 'Failed' ? `<button type="button" class="secondary-button" data-action="channel-council-accept" data-delivery-id="${escape(message.id)}">Accept with reason</button>` : ''}</span>` : '';
    return `<div class="bc-assigned" role="note" data-message-id="${escape(message.id)}"><span>${escape(description)}</span>${message.status ? `<small>${escape(message.status)}</small>` : ""}${verdict}</div>`;
  }
  const bot = message.authorBotId ? bots.find(item => item.id === message.authorBotId) : undefined;
  const name = bot?.name ?? (message.kind === 'user' ? 'You' : 'Bot');
  const targets = (message.recipientIds ?? []).map(id => bots.find(item => item.id === id)?.name ?? 'Bot');
  const label = message.kind === 'bot_request' ? `Requested ${targets.join(' and ')}`
    : message.kind === 'bot_post' ? 'Channel update' : '';
  const marker = message.kind === 'bot_request' ? `<small class="bc-message-status">${escape([label, message.status].filter(Boolean).join(' · '))}</small>`
    : label ? `<small class="bc-message-status">${escape(label)}</small>`
    : message.status === 'Failed' || message.status === 'Stopped' ? `<small class="bc-message-status">${escape(message.status)}</small>` : '';
  return `<article class="bc-message ${message.kind === 'user' ? 'from-user' : 'from-bot'}" data-message-id="${escape(message.id)}" ${bot?.color ? `style="--bot-color:${escape(bot.color)}"` : ''}>
    ${bot ? botAvatarHtml(bot.avatar, bot.name, 'bc-message-avatar', bot.color) : ''}<div class="bc-message-main"><header><strong>${escape(name)}</strong>${marker}</header><div class="bc-message-text" data-message-id="${escape(message.id)}"></div></div></article>`;
}

function workingHtml(channel: SharedChannelDetail, bots: Bot[]): string {
  const ids = channel.activeBotIds;
  if (!ids.length) return '';
  const names = ids.map(id => bots.find(bot => bot.id === id)?.name).filter(Boolean) as string[];
  const label = names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names.at(-1)} are working` : `${names[0] ?? 'A bot'} is working`;
  return `<div class="bc-working bot-working" data-shimmer><span class="bc-working-faces">${faces(ids, bots)}</span><span class="bot-working-label">${escape(WORKING_LABELS[0])}</span><span class="bc-working-status sr-only" role="status">${escape(label)}</span></div>`;
}

export function sharedChannelThreadHtml(channel: SharedChannelDetail, bots: Bot[]): string {
  const members = channel.memberIds.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot));
  const empty = channel.messages.length === 0;
  const messages = channel.messages.map(message => messageHtml(message, bots)).join('');
  return `<section class="bc-channel" data-channel-id="${escape(channel.id)}">
    <header class="bc-header"><span class="bc-title">${channelFaceCluster(channel.memberIds, bots)}<span class="bc-member-names">${escape(members.map(bot => bot.name).join(', ') || 'No members')}</span></span>${channel.archived ? '' : `<button type="button" class="icon-button bc-settings" data-action="shared-channel-settings" aria-label="Channel settings">${icon('settings')}</button>`}</header>
    ${channel.archived ? '<div class="bc-archived" role="status">Deleted channel. Preview only.</div>' : ''}
    <div class="bc-history" aria-label="Channel messages">${channel.hasMore ? '<button type="button" class="secondary-button bc-earlier" data-shared-action="earlier">Show earlier messages</button>' : ''}${empty ? `<div class="bc-empty">${channelFaceCluster([], bots)}<p>${members.length ? 'No messages yet. @mention a bot or write to the lead.' : 'Add bots in channel settings to start work.'}</p></div>` : messages}${workingHtml(channel, bots)}</div>
  </section>`;
}

export function sharedChannelActionsHtml(channel: SharedChannelDetail, bots: Bot[]): string {
  if (channel.archived) return '<div class="bc-channel-actions"><span role="status">Deleted channel · preview only</span></div>';
  // A stopped or failed post can run again; the thread above already says who is working.
  const stopped = [...channel.messages].reverse().find(message => (message.status === 'Stopped' || message.status === 'Failed') && message.kind === 'user');
  const members = channel.memberIds.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot));
  const queued = channel.messages.filter(message => message.kind === 'assigned' && message.status === 'Queued').map(message => message.authorBotId ?? '');
  const busy = [...new Set([...channel.activeBotIds, ...queued])].map(id => bots.find(bot => bot.id === id)?.name ?? 'bot');
  const stop = busy.length ? `<button type="button" class="secondary-button" data-shared-action="stop">${escape(`Stop ${busy.length > 1 ? `${busy.slice(0, -1).join(', ')} and ${busy.at(-1)}` : busy[0]}`)}</button>` : '';
  const stoppedRow = stopped && members.length ? `<span>${stopped.status === 'Failed' ? 'This message failed.' : 'This message stopped.'}</span><button type="button" class="secondary-button" data-shared-action="continue" data-message="${escape(stopped.id)}">Continue</button><button type="button" class="secondary-button" data-shared-action="reassign-toggle" aria-expanded="false">Reassign ${icon('down')}</button><div class="bc-reassign-menu" hidden>${members.map(bot => `<button type="button" data-shared-action="reassign" data-message="${escape(stopped.id)}" data-bot="${escape(bot.id)}">${botAvatarHtml(bot.avatar, bot.name, 'bc-pick-face', bot.color)}${escape(bot.name)}</button>`).join('')}</div>` : '';
  return stop || stoppedRow ? `<div class="bc-channel-actions" role="group" aria-label="Channel activity">${stop}${stoppedRow}</div>` : '';
}


/** Preserve unsent, per-message Council inputs across a channel refresh only. */
export function restoreSharedChannelCouncilInputs(previous: HTMLFormElement | null, next: HTMLFormElement | null): void {
  if (!previous || !next || previous.dataset.channelId !== next.dataset.channelId) return;
  const oldRows = [...previous.querySelectorAll<HTMLElement>('[data-bc-criterion]')];
  const nextList = next.querySelector<HTMLElement>('[data-bc-criteria-list]');
  const template = next.querySelector<HTMLTemplateElement>('[data-bc-criterion-template]');
  if (nextList && template) {
    while (next.querySelectorAll('[data-bc-criterion]').length < oldRows.length) {
      const row = template.content.firstElementChild?.cloneNode(true);
      if (row) nextList.append(row);
    }
    while (next.querySelectorAll('[data-bc-criterion]').length > oldRows.length && oldRows.length > 0) {
      next.querySelector('[data-bc-criterion]:last-of-type')?.remove();
    }
    reindexChannelCriteria(previous);
    reindexChannelCriteria(next);
  }
  const oldInputs = [...previous.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>('[name^="council"]')];
  const restored = new Set<Element>();
  for (const input of next.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>('[name^="council"]')) {
    const key = input.dataset.councilDraftKey;
    const old = oldInputs.find(item => !restored.has(item) && (key
      ? item.dataset.councilDraftKey === key
      : item.name === input.name && item.tagName === input.tagName
        && (!(item instanceof HTMLInputElement) || !(input instanceof HTMLInputElement)
          || (item.type !== 'checkbox' && item.type !== 'radio') || item.value === input.value)));
    if (!old) continue;
    restored.add(old);
    if (input instanceof HTMLInputElement && (input.type === 'checkbox' || input.type === 'radio') && old instanceof HTMLInputElement) input.checked = old.checked;
    else input.value = old.value;
  }
}

function reindexChannelCriteria(form: HTMLFormElement): void {
  for (const [index, row] of [...form.querySelectorAll<HTMLElement>('[data-bc-criterion]')].entries()) {
    for (const control of row.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('[data-bc-criterion-field]')) {
      control.dataset.councilDraftKey = `criterion-${index}-${control.dataset.bcCriterionField}`;
    }
  }
}

function sharedChannelCouncilPreview(form: HTMLFormElement, composition: CouncilComposition): string {
  const reviewerRows = [...form.querySelectorAll<HTMLElement>('[data-bc-reviewer-row]')];
  const selectedLines = composition.selected.map(reviewer => {
    const row = reviewerRows.find(item => item.querySelector<HTMLInputElement>('[data-bc-reviewer]')?.value === reviewer.botId);
    const name = row?.querySelector<HTMLElement>('.council-composition-bot strong')?.textContent?.trim() || reviewer.botId;
    if (reviewer.mode !== 'nested') return `D · Direct review: ${name}`;
    const raw = (key: string) => {
      const value = row?.querySelector<HTMLInputElement | HTMLSelectElement>(`[data-bc-nested-field="${key}"]`)?.value.trim() ?? '';
      return value || 'not set';
    };
    return `N · Child Council: ${name} (template ${raw('template')}; criteria ${raw('childCriteriaSource')}; reviewers ${raw('childReviewerCount')}; target ${raw('target')}; max rounds ${raw('maxRounds')}; escalation ${raw('escalation')})`;
  });
  const automaticCount = composition.automatic.enabled
    ? form.querySelector<HTMLSelectElement>('[data-bc-automatic-count]')?.value || 'not set'
    : 'off';
  const hasDirect = composition.selected.some(reviewer => reviewer.mode === 'direct');
  const hasNested = composition.selected.some(reviewer => reviewer.mode === 'nested');
  const sourceKey = `${composition.automatic.enabled ? 'A' : ''}${hasDirect ? 'D' : ''}${hasNested ? 'N' : ''}`;
  const modeId = ({ A: 'M1', D: 'M2', AD: 'M3', N: 'M4', AN: 'M5', DN: 'M6', ADN: 'M7' } as Record<string, string>)[sourceKey] ?? 'none';
  const strategy = ({ parallel: 'S1 · Parallel', escalated: 'S2 · Escalated / on-demand', two_phase: 'S3 · Two-phase' } as Record<string, string>)[composition.strategy] ?? composition.strategy;
  const criteriaSource = composition.criteriaSource === 'architect' ? 'Architect'
    : composition.criteriaSource === 'hybrid' ? 'Hybrid (Architect + User)' : 'User';
  const criteriaLines = composition.criteria.map((criterion, index) => {
    const rawTarget = criterionRowsRawValue(form, index, 'target') || 'not set';
    return `  ${criterion.id || 'ID not set'} · ${criterion.lens || 'lens not set'} · target ${rawTarget}/10 · ${criterion.required ? 'required' : 'optional'} · evidence required: ${criterion.evidenceRequired ? 'yes' : 'no'}\n    ${criterion.description || 'Description not set'}`;
  });
  const lines = [
    'Draft only · configuration preview, not a review result',
    'Schema version: 1',
    `Reviewer sources: ${sourceKey ? `${sourceKey} · ${modeId}` : 'none selected'}`,
    ...(composition.automatic.enabled ? [`A · Automatic Architect reviewers: ${automaticCount === 'auto' ? 'Auto' : automaticCount}`] : ['A · Automatic Architect reviewers: off']),
    ...selectedLines,
    `Strategy: ${strategy}`,
    `Criteria source: ${criteriaSource}`,
    ...(composition.criteriaSource !== 'user' ? ['Architect-provided criteria are not included in this composer preview.'] : []),
    composition.criteria.length ? 'User criteria (draft):' : 'User criteria: none configured',
    ...criteriaLines,
    'Plan limits: depth 2; root seats 8; concurrent leaves 64; leaf reviews per round 128; timeout 24 hours; estimated cost cap 1,000,000 micros.',
    'Evidence status: not evaluated in this preview.',
  ];
  return lines.join('\n');
}

function criterionRowsRawValue(form: HTMLFormElement, index: number, name: string): string {
  return [...form.querySelectorAll<HTMLElement>('[data-bc-criterion]')][index]
    ?.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[data-bc-criterion-field="${name}"]`)?.value.trim() ?? '';
}

function channelCriterionMarkup(): string {
  return `<fieldset class="council-criterion" data-bc-criterion>
    <legend>User criterion</legend>
    <label>Criterion ID<input type="text" name="councilCriterionId" data-bc-criterion-field="id" maxlength="80" value="criterion-1"></label>
    <label>Lens<input type="text" name="councilCriterionLens" data-bc-criterion-field="lens" maxlength="80" value="correctness"></label>
    <label>Description<textarea name="councilCriterionDescription" data-bc-criterion-field="description" rows="2" maxlength="4096">Check correctness and evidence.</textarea></label>
    <label>Target<input type="number" name="councilCriterionTarget" data-bc-criterion-field="target" min="1" max="10" step="0.5" value="8.5"></label>
    <label><input type="checkbox" name="councilCriterionRequired" data-bc-criterion-field="required" checked> Required for acceptance</label>
    <label><input type="checkbox" name="councilCriterionEvidence" data-bc-criterion-field="evidenceRequired" checked> Require verified evidence</label>
    <button type="button" class="ds-btn council-remove-criterion" data-bc-remove-criterion>Remove criterion</button>
  </fieldset>`;
}

export function sharedChannelComposerHtml(channel: SharedChannelDetail, bots: Bot[] = []): string {
  if (channel.archived || !channel.memberIds.length) return '';
  const members = channel.memberIds.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot));
  const options = members.map(bot => `<div class="council-composition-reviewer" data-bc-reviewer-row>
    <label class="council-composition-bot"><input name="councilReviewer" type="checkbox" value="${escape(bot.id)}" data-bc-reviewer><span><strong>${escape(bot.name)}</strong><small>Channel member</small></span></label>
    <label class="council-composition-reviewer-mode">Reviewer branch<select name="councilReviewerMode" data-bc-reviewer-mode data-council-draft-key="reviewer-${escape(bot.id)}-mode" disabled><option value="direct" selected>Direct review</option><option value="nested">Child Council</option></select></label>
    <fieldset class="council-nested-fields" data-bc-nested-policy hidden>
      <legend>Child Council policy</legend>
      <label>Task template<select name="councilNestedTemplate" data-bc-nested-field="template" data-council-draft-key="reviewer-${escape(bot.id)}-template"><option value="explicit_task" selected>Explicit task</option><option value="validated_default">Validated default</option></select></label>
      <label>Child criteria<select name="councilNestedCriteriaSource" data-bc-nested-field="childCriteriaSource" data-council-draft-key="reviewer-${escape(bot.id)}-criteria"><option value="architect" selected>Architect</option><option value="user">User</option><option value="hybrid">Hybrid</option></select></label>
      <label>Child reviewers<input type="number" name="councilNestedReviewerCount" data-bc-nested-field="childReviewerCount" data-council-draft-key="reviewer-${escape(bot.id)}-count" min="2" max="8" step="1" value="2"></label>
      <label>Child target<input type="number" name="councilNestedTarget" data-bc-nested-field="target" data-council-draft-key="reviewer-${escape(bot.id)}-target" min="1" max="10" step="0.5" value="9"></label>
      <label>Child max rounds<input type="number" name="councilNestedMaxRounds" data-bc-nested-field="maxRounds" data-council-draft-key="reviewer-${escape(bot.id)}-rounds" min="1" max="100" step="1" value="10"></label>
      <label>Escalation<select name="councilNestedEscalation" data-bc-nested-field="escalation" data-council-draft-key="reviewer-${escape(bot.id)}-escalation"><option value="always" selected>Always</option><option value="on_trigger">On trigger</option></select></label>
    </fieldset>
  </div>`).join('');
  const legacyOptions = members.map(bot => `<label><input type="checkbox" name="councilReviewer" value="${escape(bot.id)}" data-bc-legacy-reviewer data-council-draft-key="legacy-reviewer-${escape(bot.id)}" disabled>${botAvatarHtml(bot.avatar, bot.name, 'bc-pick-face', bot.color)}<span>${escape(bot.name)}</span></label>`).join('');
  const sourceModes = [
    ['M1', 'A · automatic'], ['M2', 'D · direct'], ['M3', 'A + D'],
    ['M4', 'N · nested'], ['M5', 'A + N'], ['M6', 'D + N'], ['M7', 'A + D + N'],
  ].map(([id, label]) => `<li data-bc-source-mode="${id}"><strong>${id}</strong><span>${label}</span></li>`).join('');
  return `<form id="shared-channel-composer" data-channel-id="${escape(channel.id)}" class="composer bc-composer" novalidate><textarea id="shared-channel-prompt" name="text" aria-label="Message channel" aria-description="Enter to send. Shift Enter for a new line." rows="2" placeholder="Message ${escape(channelName(channel.name))}… @mention up to two bots"></textarea>
    <div class="composer-toolbar"><button type="button" class="bc-council-trigger composer-council-button" data-action="channel-council" aria-label="Council: off" aria-haspopup="dialog" aria-expanded="false" aria-controls="bc-council-panel" title="Council settings"><span class="council-glyph"><svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v16M8 20h8M5 7h14M5 7l-3 6.5a3 3 0 0 0 6 0zM19 7l-3 6.5a3 3 0 0 0 6 0z"/></svg></span></button><span class="bc-composer-spacer"></span><button type="submit" class="send-button" title="Send message" aria-label="Send message">${icon('arrow')}</button></div>
    <p class="sr-only" data-bc-post-status role="status" aria-live="polite" aria-atomic="true"></p>
    <section id="bc-council-panel" class="floating-menu council-panel bc-council-panel" role="dialog" aria-labelledby="bc-council-title" popover="auto">
      <div class="bc-council-controls">
        <div class="council-panel-head"><label id="bc-council-title" for="bc-council-enabled">Council</label><input id="bc-council-enabled" class="ds-toggle council-switch" type="checkbox" role="switch" name="councilEnabled" aria-describedby="bc-council-about"></div>
        <p id="bc-council-about">Reviewers score this message until they reach your target. These settings apply only to this message.</p>
        <fieldset class="bc-council-options"><legend class="sr-only">Council settings for this message</legend>
        <div class="council-field"><span id="bc-council-target-label">Target score</span><div class="bc-council-value-row"><div class="ds-seg" role="group" aria-labelledby="bc-council-target-label"><button type="button" data-council-preset="target" data-value="7" aria-pressed="false">7.0</button><button type="button" data-council-preset="target" data-value="8" aria-pressed="false">8.0</button><button type="button" data-council-preset="target" data-value="8.5" aria-pressed="false">8.5</button><button type="button" data-council-preset="target" data-value="9" aria-pressed="true">9.0</button><button type="button" data-council-preset="target" data-value="9.5" aria-pressed="false">9.5</button><button type="button" data-council-preset="target" data-value="10" aria-pressed="false">10.0</button></div><input type="number" name="councilTarget" aria-label="Custom target score" title="Target score (1–10)" min="1" max="10" step="0.5" value="9"></div></div>
        <div class="council-field"><span id="bc-council-rounds-label">Max rounds</span><div class="bc-council-value-row"><div class="ds-seg" role="group" aria-labelledby="bc-council-rounds-label"><button type="button" data-council-preset="rounds" data-value="3" aria-pressed="false">3</button><button type="button" data-council-preset="rounds" data-value="5" aria-pressed="false">5</button><button type="button" data-council-preset="rounds" data-value="10" aria-pressed="true">10</button><button type="button" data-council-preset="rounds" data-value="25" aria-pressed="false">25</button></div><input type="number" name="councilMaxRounds" aria-label="Custom max rounds" title="Max rounds (1–100)" min="1" max="100" step="1" value="10"></div></div>
        <fieldset class="bc-council-mode-group" data-bc-legacy-controls>
          <legend>Legacy reviewer selection</legend>
          <div class="bc-council-modes" role="group" aria-label="Legacy Council reviewer source">
            <label><input type="radio" name="councilMode" value="auto" data-council-draft-key="legacy-mode-auto" checked>Automatic native reviewers</label>
            <label><input type="radio" name="councilMode" value="selected" data-council-draft-key="legacy-mode-selected">Selected channel bots</label>
          </div>
          <div class="bc-council-reviewer-options" role="group" aria-label="Legacy selected channel bots" data-bc-legacy-reviewer-options>
            <p>Choose 2–8 members other than any bot assigned this message.</p>${legacyOptions || '<p>No channel members are available for selection.</p>'}
          </div>
        </fieldset>
        <section class="council-composition-editor" aria-labelledby="bc-council-composition-title">
          <h3 id="bc-council-composition-title">Reviewer composition</h3>
          <label class="council-composition-toggle"><input type="checkbox" name="councilCompositionEnabled" data-bc-composition-enabled data-council-draft-key="composition-enabled"><span>Use a schema v1 composition plan</span></label>
          <p class="council-composition-help">When this is off, Council keeps its legacy automatic reviewer behavior. Composition reviewers are limited to this channel’s members.</p>
          <div class="council-composition-body" data-bc-composition-body hidden>
            <p class="council-composition-mode-readout" role="status" data-bc-source-summary>Choose one or more reviewer sources.</p>
            <ol class="council-composition-modes" aria-label="Derived reviewer sources">${sourceModes}</ol>
            <label class="council-composition-toggle"><input type="checkbox" name="councilAutomaticEnabled" data-bc-automatic-enabled data-council-draft-key="automatic-enabled" checked><span>Include automatic Architect reviewers (A)</span></label>
            <label class="council-composition-control">Automatic reviewer count<select name="councilAutomaticCount" data-bc-automatic-count data-council-draft-key="automatic-count"><option value="auto" selected>Auto</option>${Array.from({ length: 8 }, (_, index) => `<option value="${index + 1}">${index + 1}</option>`).join('')}</select></label>
            <div class="council-composition-bots" role="group" aria-label="Selected channel-member reviewers" data-bc-reviewer-group>${options || '<p>No channel members are available for reviewer selection.</p>'}</div>
            <label class="council-composition-control">Criteria source<select name="councilCriteriaSource" data-bc-criteria-source data-council-draft-key="criteria-source"><option value="architect" selected>Architect</option><option value="user">User</option><option value="hybrid">Hybrid</option></select></label>
            <p class="council-composition-help">Architect criteria come from the lead’s plan. User criteria are explicit review gates; hybrid combines both.</p>
            <div class="council-composition-criteria" data-bc-criteria-editor hidden>
              <div data-bc-criteria-list>${channelCriterionMarkup()}</div>
              <template data-bc-criterion-template>${channelCriterionMarkup()}</template>
              <button type="button" class="ds-btn" data-bc-add-criterion>Add user criterion</button>
            </div>
            <label class="council-composition-control">Execution strategy<select name="councilStrategy" data-bc-strategy data-council-draft-key="strategy"><option value="parallel" selected>S1 · Parallel</option><option value="escalated">S2 · Escalated / on-demand</option><option value="two_phase">S3 · Two-phase</option></select></label>
            <p class="council-composition-help">Choose an execution strategy independently of the reviewer sources. Native controls scheduling and validates the submitted plan.</p>
            <section class="council-plan-preview" data-bc-plan-preview aria-label="Schema v1 plan preview" hidden>
              <h4>Schema v1 plan preview</h4>
              <p class="council-composition-help">Draft only. This is the configuration being sent; it does not show review results or verified evidence.</p>
              <pre data-bc-plan-preview-text style="box-sizing:border-box;min-width:0;max-width:100%;white-space:pre-wrap;overflow-wrap:anywhere"></pre>
            </section>
          </div>
        </section>
        <p class="council-composition-help" data-bc-validation-summary role="alert" hidden></p>
        <div class="bc-council-followup"><label><input type="checkbox" name="councilGithubIssue"> After Council approval, create one GitHub issue with accepted findings</label>
          <label>Authorized repository (owner/name) <input type="text" name="councilGithubRepo" maxlength="201" placeholder="owner/repository" autocomplete="off"></label>
          <small>Uses the existing signed-in GitHub CLI. A failed or uncertain external write is reported separately; it never changes the Council verdict.</small></div>
        <div class="bc-council-selected-permissions"><label><input type="checkbox" name="councilScopedFile"> Allow one exact native file-inspection action</label><label>Authorized relative file path <input type="text" name="councilFilePath" maxlength="240" placeholder="src/example.ts" autocomplete="off"></label><small>Read-only by default. This does not authorize shell commands or checkout writes.</small></div>
        </fieldset>
      </div>
    </section></form>`;
}

/** Build the schema-v1 draft from the visible channel controls; absent means keep the legacy Council payload. */
export function sharedChannelCouncilComposition(form: HTMLFormElement): CouncilComposition | null {
  if (!form.querySelector<HTMLInputElement>('[data-bc-composition-enabled]')?.checked) return null;
  const automaticEnabled = form.querySelector<HTMLInputElement>('[data-bc-automatic-enabled]')?.checked ?? false;
  const countValue = form.querySelector<HTMLSelectElement>('[data-bc-automatic-count]')?.value ?? 'auto';
  const automaticCount = automaticEnabled ? countValue === 'auto' ? 'auto' : Number(countValue) : 0;
  const selected = [...form.querySelectorAll<HTMLInputElement>('[data-bc-reviewer]:checked')].map(input => {
    const row = input.closest<HTMLElement>('[data-bc-reviewer-row]');
    const mode = (row?.querySelector<HTMLSelectElement>('[data-bc-reviewer-mode]')?.value ?? 'direct') as CouncilReviewerMode;
    if (mode !== 'nested') return { botId: input.value, mode, required: true as const };
    const field = (name: string): string => row?.querySelector<HTMLInputElement | HTMLSelectElement>(`[data-bc-nested-field="${name}"]`)?.value ?? '';
    return {
      botId: input.value, mode, required: true as const,
      nested: {
        template: field('template') as 'explicit_task' | 'validated_default',
        childCriteriaSource: field('childCriteriaSource') as CouncilCriteriaSource,
        childReviewerCount: Number(field('childReviewerCount')),
        target: Number(field('target')),
        maxRounds: Number(field('maxRounds')),
        escalation: field('escalation') as 'always' | 'on_trigger',
      },
    };
  });
  const criteriaSource = (form.querySelector<HTMLSelectElement>('[data-bc-criteria-source]')?.value ?? 'architect') as CouncilCriteriaSource;
  const criteria = criteriaSource === 'architect' ? [] : [...form.querySelectorAll<HTMLElement>('[data-bc-criterion]')].map(row => {
    const value = (name: string): string => row.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[data-bc-criterion-field="${name}"]`)?.value ?? '';
    return {
      id: value('id'), lens: value('lens'), description: value('description'),
      required: row.querySelector<HTMLInputElement>('[data-bc-criterion-field="required"]')?.checked ?? false,
      target: Number(value('target')),
      evidenceRequired: row.querySelector<HTMLInputElement>('[data-bc-criterion-field="evidenceRequired"]')?.checked ?? false,
      source: 'user' as const,
    };
  });
  const strategy = (form.querySelector<HTMLSelectElement>('[data-bc-strategy]')?.value ?? 'parallel') as CouncilStrategy;
  return {
    schemaVersion: 1,
    automatic: { enabled: automaticEnabled, count: automaticCount },
    selected,
    criteriaSource,
    criteria,
    strategy,
    limits: { ...CHANNEL_COUNCIL_LIMITS },
  };
}

export interface SharedChannelCouncilIssue { field: HTMLElement | null; message: string }

/** Validate the visible draft and point each failure back to its source field. Native remains authoritative. */
export function sharedChannelCouncilValidationIssues(
  form: HTMLFormElement,
  composition: CouncilComposition | null,
  target: number,
  maxRounds: number,
): SharedChannelCouncilIssue[] {
  const issues: SharedChannelCouncilIssue[] = [];
  const field = (selector: string): HTMLElement | null => form.querySelector<HTMLElement>(selector);
  const add = (control: HTMLElement | null, message: string) => issues.push({ field: control, message });
  const roundsInput = field('[name="councilMaxRounds"]');
  if (!Number.isInteger(maxRounds) || maxRounds < 1 || maxRounds > 100) add(roundsInput, 'Council max rounds must be an integer from 1 to 100.');
  if (!composition) {
    if (!Number.isFinite(target) || target < 1 || target > 10) add(field('[name="councilTarget"]'), 'Council target must be between 1 and 10.');
    return issues;
  }

  const reviewerRows = [...form.querySelectorAll<HTMLElement>('[data-bc-reviewer-row]')];
  const reviewerGroup = field('[data-bc-reviewer-group]');
  const automaticCount = field('[data-bc-automatic-count]');
  const criterionRows = [...form.querySelectorAll<HTMLElement>('[data-bc-criterion]')];
  const reviewerField = (index: number, suffix = ''): HTMLElement | null => {
    const reviewer = composition.selected[index];
    const row = reviewerRows.find(item => item.querySelector<HTMLInputElement>('[data-bc-reviewer]')?.value === reviewer?.botId);
    if (suffix.startsWith('nested.')) return row?.querySelector<HTMLElement>(`[data-bc-nested-field="${suffix.slice(7)}"]`) ?? row?.querySelector<HTMLElement>('[data-bc-nested-policy]') ?? reviewerGroup;
    if (suffix === 'nested') return row?.querySelector<HTMLElement>('[data-bc-nested-policy]') ?? reviewerGroup;
    if (suffix === 'mode') return row?.querySelector<HTMLElement>('[data-bc-reviewer-mode]') ?? reviewerGroup;
    if (suffix === 'botId') return row?.querySelector<HTMLElement>('[data-bc-reviewer]') ?? reviewerGroup;
    return row ?? reviewerGroup;
  };
  const coreField = (path: string, message: string): HTMLElement | null => {
    if (path === 'strategy') return field('[data-bc-strategy]');
    if (path === 'criteriaSource') return field('[data-bc-criteria-source]');
    if (path === 'councilTarget') return field('[name="councilTarget"]');
    if (path === 'automatic.count') return automaticCount;
    if (path === 'reviewers') {
      if (message.includes('At least 2 root reviewers')) {
        const sourceFields: HTMLElement[] = [];
        if (composition.automatic.enabled && automaticCount) sourceFields.push(automaticCount);
        for (const reviewer of composition.selected) {
          const row = reviewerRows.find(item => item.querySelector<HTMLInputElement>('[data-bc-reviewer]')?.value === reviewer.botId);
          const branch = row?.querySelector<HTMLElement>('[data-bc-reviewer-mode]');
          if (branch) sourceFields.push(branch);
        }
        if (sourceFields.length) {
          for (const source of sourceFields) add(source, 'This A/D/N selection provides fewer than 2 root reviewer seats. Add another direct or nested reviewer, or increase automatic reviewers.');
          return null;
        }
        return field('[data-bc-source-summary]');
      }
      return reviewerGroup;
    }
    if (path === 'criteria') {
      const required = criterionRows.find(row => !row.querySelector<HTMLInputElement>('[data-bc-criterion-field="required"]')?.checked)
        ?.querySelector<HTMLElement>('[data-bc-criterion-field="required"]');
      return message.includes('required criterion') ? required ?? field('[data-bc-criteria-list]') : field('[data-bc-criteria-list]');
    }
    if (path === 'schemaVersion') return field('[data-bc-composition-enabled]');
    if (path.startsWith('limits.')) return field('[data-bc-plan-preview]');
    const reviewerMatch = /^reviewers\.(\d+)\.(.+)$/.exec(path);
    if (reviewerMatch) return reviewerField(Number(reviewerMatch[1]), reviewerMatch[2]);
    const criterionMatch = /^criteria\.(\d+)\.(.+)$/.exec(path);
    if (criterionMatch) {
      const row = criterionRows[Number(criterionMatch[1])];
      return row?.querySelector<HTMLElement>(`[data-bc-criterion-field="${criterionMatch[2]}"]`) ?? field('[data-bc-criteria-list]');
    }
    return null;
  };
  for (const issue of councilCompositionValidationIssues(composition, target)) {
    const mapped = coreField(issue.field, issue.message);
    if (mapped) add(mapped, issue.message);
  }

  for (const [index, reviewer] of composition.selected.entries()) {
    if (!['direct', 'nested'].includes(reviewer.mode)) add(reviewerField(index, 'mode'), 'Choose a direct or nested reviewer branch.');
    if (reviewer.mode !== 'nested' || !reviewer.nested) continue;
    const row = reviewerRows.find(item => item.querySelector<HTMLInputElement>('[data-bc-reviewer]')?.value === reviewer.botId);
    const nestedField = (name: string) => row?.querySelector<HTMLElement>(`[data-bc-nested-field="${name}"]`) ?? row?.querySelector<HTMLElement>('[data-bc-nested-policy]') ?? reviewerGroup;
    if (!['explicit_task', 'validated_default'].includes(reviewer.nested.template)) add(nestedField('template'), 'Choose a supported child task template.');
    if (!['architect', 'user', 'hybrid'].includes(reviewer.nested.childCriteriaSource)) add(nestedField('childCriteriaSource'), 'Choose a supported child criteria source.');
    if (!['always', 'on_trigger'].includes(reviewer.nested.escalation)) add(nestedField('escalation'), 'Choose a supported escalation setting.');
  }
  return issues;
}

/** Render the client-side issues next to their controls; edits clear or update the failed state. */
export function showSharedChannelCouncilValidation(form: HTMLFormElement, issues: SharedChannelCouncilIssue[], focusFirst = false): void {
  const summary = form.querySelector<HTMLElement>('[data-bc-validation-summary]');
  for (const control of form.querySelectorAll<HTMLElement>('[data-bc-validation-linked]')) {
    const previousId = control.dataset.bcValidationLinked;
    const describedBy = (control.getAttribute('aria-describedby') ?? '').split(/\s+/).filter(id => id && id !== previousId);
    if (describedBy.length) control.setAttribute('aria-describedby', describedBy.join(' '));
    else control.removeAttribute('aria-describedby');
    control.removeAttribute('aria-invalid');
    if (control.dataset.bcValidationTabindex === 'added') control.removeAttribute('tabindex');
    delete control.dataset.bcValidationLinked;
    delete control.dataset.bcValidationTabindex;
  }
  for (const error of form.querySelectorAll<HTMLElement>('[data-bc-inline-error]')) error.remove();
  if (summary) {
    summary.hidden = issues.length === 0;
    summary.textContent = issues.length ? `Council settings need attention: ${issues.map(issue => issue.message).join(' ')}` : '';
  }
  form.dataset.bcValidationVisible = String(issues.length > 0);
  const grouped = new Map<HTMLElement, string[]>();
  for (const issue of issues) {
    if (!issue.field) continue;
    grouped.set(issue.field, [...(grouped.get(issue.field) ?? []), issue.message]);
  }
  for (const [index, [control, messages]] of [...grouped.entries()].entries()) {
    const label = control.closest('label');
    const error = document.createElement('small');
    error.className = 'council-composition-help';
    error.dataset.bcInlineError = '';
    error.id = `bc-council-validation-${index}`;
    error.textContent = [...new Set(messages)].join(' ');
    if (label) label.append(error);
    else control.insertAdjacentElement('afterend', error);
    control.setAttribute('aria-invalid', 'true');
    if (!control.hasAttribute('tabindex') && !control.matches('input,select,textarea,button,[tabindex]')) {
      control.setAttribute('tabindex', '-1');
      control.dataset.bcValidationTabindex = 'added';
    }
    const describedBy = (control.getAttribute('aria-describedby') ?? '').split(/\s+/).filter(Boolean);
    control.setAttribute('aria-describedby', [...describedBy, error.id].join(' '));
    control.dataset.bcValidationLinked = error.id;
  }
  if (focusFirst && issues.length) {
    const panel = form.querySelector<HTMLElement>('.bc-council-panel');
    if (panel && !panel.matches(':popover-open')) panel.showPopover();
    const first = issues.find(issue => issue.field)?.field;
    first?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    first?.focus({ preventScroll: true });
  }
}

/** Compatibility helper for callers that only need the first composition-level message. */
export function sharedChannelCouncilCompositionError(composition: CouncilComposition, target: number): string | null {
  const issues = sharedChannelCouncilValidationIssues(document.createElement('form'), composition, target, 10);
  return issues[0]?.message ?? null;
}

function addChannelCriterion(form: HTMLFormElement): HTMLElement | null {
  const template = form.querySelector<HTMLTemplateElement>('[data-bc-criterion-template]');
  const list = form.querySelector<HTMLElement>('[data-bc-criteria-list]');
  if (!template || !list || list.querySelectorAll('[data-bc-criterion]').length >= 64) return null;
  const row = template.content.firstElementChild?.cloneNode(true) as HTMLElement | null;
  if (!row) return null;
  const used = new Set([...list.querySelectorAll<HTMLInputElement>('[data-bc-criterion-field="id"]')].map(input => input.value));
  let id = 1;
  while (used.has(`criterion-${id}`)) id++;
  const idInput = row.querySelector<HTMLInputElement>('[data-bc-criterion-field="id"]');
  if (idInput) idInput.value = `criterion-${id}`;
  list.append(row);
  reindexChannelCriteria(form);
  return row;
}

/** Bind the per-message Council panel without changing the channel's native submission contract. */
let activeCouncilPlacement: (() => void) | null = null;
export function bindSharedChannelCouncil(form: HTMLFormElement | null): void {
  if (activeCouncilPlacement) window.removeEventListener('resize', activeCouncilPlacement);
  activeCouncilPlacement = null;
  if (!form) return;
  const panel = form.querySelector<HTMLElement>('.bc-council-panel');
  const trigger = form.querySelector<HTMLButtonElement>('.bc-council-trigger');
  if (!panel || !trigger) return;
  const sync = () => {
    const enabled = form.querySelector<HTMLInputElement>('input[name="councilEnabled"]')?.checked ?? false;
    trigger.dataset.on = enabled ? 'true' : 'false';
    trigger.setAttribute('aria-label', enabled ? 'Council: on' : 'Council: off');
    for (const choice of panel.querySelectorAll<HTMLButtonElement>('button[data-council-preset]')) {
      const name = choice.dataset.councilPreset === 'target' ? 'councilTarget' : 'councilMaxRounds';
      const input = form.elements.namedItem(name) as HTMLInputElement | null;
      choice.setAttribute('aria-pressed', String(Number(input?.value) === Number(choice.dataset.value)));
    }
    const compositionEnabled = form.querySelector<HTMLInputElement>('[data-bc-composition-enabled]')?.checked ?? false;
    const compositionBody = form.querySelector<HTMLElement>('[data-bc-composition-body]');
    if (compositionBody) { compositionBody.hidden = !compositionEnabled; compositionBody.style.display = compositionEnabled ? 'grid' : 'none'; }
    const legacyControls = form.querySelector<HTMLElement>('[data-bc-legacy-controls]');
    const legacySelected = form.querySelector<HTMLInputElement>('input[name="councilMode"][value="selected"]')?.checked ?? false;
    if (legacyControls) { legacyControls.hidden = compositionEnabled; legacyControls.style.display = compositionEnabled ? 'none' : 'grid'; }
    for (const mode of form.querySelectorAll<HTMLInputElement>('input[name="councilMode"]')) mode.disabled = compositionEnabled;
    for (const reviewer of form.querySelectorAll<HTMLInputElement>('[data-bc-legacy-reviewer]')) reviewer.disabled = compositionEnabled || !legacySelected;
    const automaticEnabled = compositionEnabled && (form.querySelector<HTMLInputElement>('[data-bc-automatic-enabled]')?.checked ?? false);
    const count = form.querySelector<HTMLSelectElement>('[data-bc-automatic-count]');
    if (count) count.disabled = !automaticEnabled;
    const reviewerRows = [...form.querySelectorAll<HTMLElement>('[data-bc-reviewer-row]')];
    const selectedRows: HTMLElement[] = [];
    let direct = false, nested = false;
    for (const row of reviewerRows) {
      const selected = row.querySelector<HTMLInputElement>('[data-bc-reviewer]')?.checked ?? false;
      const mode = row.querySelector<HTMLSelectElement>('[data-bc-reviewer-mode]');
      if (mode) mode.disabled = !compositionEnabled || !selected;
      const isNested = selected && mode?.value === 'nested';
      const nestedFields = row.querySelector<HTMLElement>('[data-bc-nested-policy]');
      if (nestedFields) { nestedFields.hidden = !compositionEnabled || !isNested; nestedFields.style.display = compositionEnabled && isNested ? 'grid' : 'none'; }
      for (const field of row.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-bc-nested-field]')) field.disabled = !compositionEnabled || !isNested;
      if (!selected || !compositionEnabled) continue;
      selectedRows.push(row);
      if (isNested) nested = true;
      else direct = true;
    }
    const scopedPermissions = form.querySelector<HTMLElement>('.bc-council-selected-permissions');
    if (scopedPermissions) scopedPermissions.style.display = compositionEnabled
      ? selectedRows.length ? 'grid' : 'none'
      : legacySelected ? 'grid' : 'none';
    const sourceKey = `${automaticEnabled ? 'A' : ''}${direct ? 'D' : ''}${nested ? 'N' : ''}`;
    const derived = ({ A: 'M1', D: 'M2', AD: 'M3', N: 'M4', AN: 'M5', DN: 'M6', ADN: 'M7' } as Record<string, string>)[sourceKey];
    const sourceSummary = form.querySelector<HTMLElement>('[data-bc-source-summary]');
    if (sourceSummary) sourceSummary.textContent = derived ? `Sources ${[automaticEnabled && 'A', direct && 'D', nested && 'N'].filter(Boolean).join(' + ')} · ${derived}` : 'Choose one or more reviewer sources.';
    for (const item of form.querySelectorAll<HTMLElement>('[data-bc-source-mode]')) item.setAttribute('aria-current', String(item.dataset.bcSourceMode === derived));
    const criteriaSource = form.querySelector<HTMLSelectElement>('[data-bc-criteria-source]')?.value ?? 'architect';
    const criteriaEditable = compositionEnabled && criteriaSource !== 'architect';
    const criteriaEditor = form.querySelector<HTMLElement>('[data-bc-criteria-editor]');
    if (criteriaEditor) { criteriaEditor.hidden = !criteriaEditable; criteriaEditor.style.display = criteriaEditable ? 'grid' : 'none'; }
    for (const control of form.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('[data-bc-criterion-field]')) control.disabled = !criteriaEditable;
    const criterionRows = form.querySelectorAll('[data-bc-criterion]');
    const addCriterion = form.querySelector<HTMLButtonElement>('[data-bc-add-criterion]');
    if (addCriterion) addCriterion.disabled = !criteriaEditable || criterionRows.length >= 64;
    for (const remove of form.querySelectorAll<HTMLButtonElement>('[data-bc-remove-criterion]')) remove.disabled = !criteriaEditable || criterionRows.length <= 1;
    const preview = form.querySelector<HTMLElement>('[data-bc-plan-preview]');
    const previewText = form.querySelector<HTMLElement>('[data-bc-plan-preview-text]');
    if (preview) preview.hidden = !compositionEnabled;
    if (compositionEnabled && previewText) {
      const composition = sharedChannelCouncilComposition(form);
      if (composition) previewText.textContent = sharedChannelCouncilPreview(form, composition);
    }
    if (form.dataset.bcValidationVisible === 'true') {
      if (!enabled) showSharedChannelCouncilValidation(form, []);
      else {
        const composition = sharedChannelCouncilComposition(form);
        const target = Number((form.elements.namedItem('councilTarget') as HTMLInputElement | null)?.value ?? 9);
        const rounds = Number((form.elements.namedItem('councilMaxRounds') as HTMLInputElement | null)?.value ?? 10);
        showSharedChannelCouncilValidation(form, sharedChannelCouncilValidationIssues(form, composition, target, rounds));
      }
    }
  };
  panel.addEventListener('click', event => {
    const target = event.target as Element;
    if (target.closest('[data-bc-add-criterion]')) {
      const row = addChannelCriterion(form);
      sync();
      row?.querySelector<HTMLInputElement>('[data-bc-criterion-field="id"]')?.focus({ preventScroll: true });
      return;
    }
    const removeCriterion = target.closest<HTMLButtonElement>('[data-bc-remove-criterion]');
    if (removeCriterion) {
      const list = form.querySelector<HTMLElement>('[data-bc-criteria-list]');
      if (list && list.querySelectorAll('[data-bc-criterion]').length > 1) removeCriterion.closest('[data-bc-criterion]')?.remove();
      reindexChannelCriteria(form);
      sync();
      return;
    }
    const choice = (event.target as Element).closest<HTMLButtonElement>('button[data-council-preset]');
    if (!choice) return;
    const name = choice.dataset.councilPreset === 'target' ? 'councilTarget' : 'councilMaxRounds';
    const input = form.elements.namedItem(name) as HTMLInputElement | null;
    if (!input) return;
    input.value = choice.dataset.value ?? input.value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  panel.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      panel.hidePopover();
      trigger.focus({ preventScroll: true });
      return;
    }
    if (event.key !== 'Tab') return;
    const focusable = [...panel.querySelectorAll<HTMLElement>('a[href],button:not([disabled]),input:not([type="hidden"]),select,textarea,[tabindex]:not([tabindex="-1"])')]
      .filter(element => !element.matches(':disabled') && element.tabIndex >= 0 && element.getClientRects().length > 0);
    if (!focusable.length) { event.preventDefault(); return; }
    const first = focusable[0], last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });
  const place = () => positionSharedChannelCouncilPanel(trigger, panel);
  panel.addEventListener('toggle', event => {
    const open = event.newState === 'open';
    trigger.setAttribute('aria-expanded', String(open));
    if (open) {
      if (activeCouncilPlacement) window.removeEventListener('resize', activeCouncilPlacement);
      activeCouncilPlacement = place;
      window.addEventListener('resize', place);
      place();
    } else if (activeCouncilPlacement === place) {
      window.removeEventListener('resize', place);
      activeCouncilPlacement = null;
    }
  });
  form.addEventListener('change', sync);
  form.addEventListener('input', sync);
  reindexChannelCriteria(form);
  sync();
}

/** Keep the floating Council panel within the viewport, including after a window resize. */
function positionSharedChannelCouncilPanel(trigger: HTMLElement, panel: HTMLElement): void {
  const rect = trigger.getBoundingClientRect();
  const width = Math.max(0, Math.min(380, window.innerWidth - 24));
  const natural = Math.min(560, panel.scrollHeight);
  const above = rect.top - 12, below = window.innerHeight - rect.bottom - 12;
  const placeAbove = above >= natural || above > below;
  const room = Math.max(0, placeAbove ? above : below);
  const height = Math.min(560, room);
  const top = placeAbove ? Math.max(12, rect.top - 10 - Math.min(natural, height)) : rect.bottom + 10;
  panel.style.width = width + 'px';
  panel.style.left = Math.max(12, Math.min(window.innerWidth - width - 12, rect.left + rect.width / 2 - width / 2)) + 'px';
  panel.style.top = top + 'px';
  // Keep the popover inside the viewport even before the resize event is delivered.
  // The dynamic viewport cap responds to resizing independently of the JS re-anchor.
  panel.style.maxHeight = `min(${height}px, max(0px, calc(100dvh - ${top}px - 12px)))`;
}

/** Show Council above the channel tray as a non-modal popover, like the direct-chat Council. */
export function toggleSharedChannelCouncil(trigger: HTMLElement): void {
  const panel = trigger.closest<HTMLFormElement>('#shared-channel-composer')?.querySelector<HTMLElement>('.bc-council-panel');
  if (!panel) return;
  if (panel.matches(':popover-open')) { panel.hidePopover(); trigger.focus({ preventScroll: true }); return; }
  panel.showPopover();
  positionSharedChannelCouncilPanel(trigger, panel);
  trigger.setAttribute('aria-expanded', 'true');
  panel.querySelector<HTMLElement>('.council-switch')?.focus({ preventScroll: true });
}

async function editChannel(options: SharedChannelUiOptions, channel: SharedChannelDetail): Promise<void> {
  const selected = new Set(channel.memberIds);
  const fields = options.bots.map(bot => `<label><input type="checkbox" name="member" value="${escape(bot.id)}" ${selected.has(bot.id) ? 'checked' : ''}>${botAvatarHtml(bot.avatar, bot.name, 'bc-pick-face', bot.color)}<span>${escape(bot.name)}</span></label>`).join('');
  const dialog = options.modal('Channel settings', `<form class="bc-settings-form"><label class="field-label" for="bc-edit-name">Name</label><input id="bc-edit-name" name="name" value="${escape(channel.name)}" maxlength="48" required>
    <fieldset class="bc-member-list"><legend>Members</legend>${fields || '<p>No bots are available.</p>'}</fieldset><label class="field-label" for="bc-lead">Lead</label><select id="bc-lead" name="lead">${channel.memberIds.map(id => options.bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot)).map((bot, index) => `<option value="${escape(bot.id)}" ${bot.id === channel.leadBotId || (!channel.leadBotId && index === 0) ? 'selected' : ''}>${escape(bot.name)}</option>`).join('')}</select>
    <div class="dialog-footer"><button type="button" class="secondary-button" data-shared-action="settings-cancel">Cancel</button><button type="submit" class="primary-button">Save</button></div></form>`, 'bot-shared-dialog');
  const form = dialog.querySelector<HTMLFormElement>('.bc-settings-form')!;
  form.addEventListener('submit', event => {
    event.preventDefault();
    const data = new FormData(form), memberIds = [...form.querySelectorAll<HTMLInputElement>('input[name=member]:checked')].map(input => input.value);
    const requestedLead = String(data.get('lead') ?? '');
    const leadBotId = memberIds.includes(requestedLead) ? requestedLead : memberIds[0] ?? null;
    const save = form.querySelector<HTMLButtonElement>('button[type="submit"]'); if (save) save.disabled = true;
    void options.call('bot_shared_channel_update', { channelId: channel.id, name: String(data.get('name') ?? ''), memberIds, leadBotId }).then(async () => {
      options.closeModal();
      await options.changed();
    }).catch(error => options.notify(error instanceof Error ? error.message : String(error))).finally(() => { if (save?.isConnected) save.disabled = false; });
  });
  dialog.querySelector('[data-shared-action="settings-cancel"]')?.addEventListener('click', options.closeModal);
}

export async function openSharedChannel(channelId: string, options: SharedChannelUiOptions): Promise<void> {
  options.openChannel(channelId);
}

/** Use the same current-cycle ledger presenter as the Workers inspector, with no editable controls. */
export function sharedChannelCouncilReportHtml(team: TeamSnapshot): string {
  const status = team.councilStatus;
  const score = status?.lowest == null ? 'lowest score Unknown' : `lowest score ${status.lowest.toFixed(1)} / ${status.target.toFixed(1)}`;
  return `<section class="bc-council-report" data-session-id="${escape(team.sessionId)}" aria-label="Council report"><p class="bc-council-report-state"><strong>Council: ${escape(status?.state ?? 'Unknown')}</strong> · ${score}</p>${councilLedgerHtmlForTeam(team)}</section>`;
}

/** Open the report from a channel without leaving its current conversation route. */
export async function openSharedChannelCouncilReport(sessionId: string, options: SharedChannelUiOptions): Promise<void> {
  if (!options.loadCouncilSnapshot) { options.notify('This Council delivery report is unavailable.'); return; }
  let team: TeamSnapshot | null;
  try { team = await options.loadCouncilSnapshot(sessionId); }
  catch { options.notify('This Council delivery report is unavailable.'); return; }
  if (!team || team.sessionId !== sessionId) { options.notify('This Council delivery report is unavailable.'); return; }
  const dialog = options.modal('Council report', sharedChannelCouncilReportHtml(team), 'bot-shared-dialog');
  const report = dialog.querySelector<HTMLElement>('.bc-council-report');
  if (report) options.bindCouncilEvidence?.(report, sessionId);
}

export async function openSharedChannelSettings(channelId: string, options: SharedChannelUiOptions): Promise<void> {
  try { await editChannel(options, await options.call<SharedChannelDetail>('bot_shared_channel_detail', { channelId })); }
  catch (error) { options.notify(error instanceof Error ? error.message : String(error)); }
}


/** This is a local user action, not a model instruction or a bot tool. */
export function openSharedChannelCouncilAcceptance(deliveryId: string, options: SharedChannelUiOptions): void {
  const dialog = options.modal('Accept Council exception', `<form class="bc-council-accept-form">
    <p class="dialog-description">The Council has not passed. Accept this review as a user exception only with an explicit reason. The result will be attributed to the local user.</p>
    <label for="bc-council-accept-reason">Acceptance reason</label>
    <textarea id="bc-council-accept-reason" name="reason" required minlength="8" maxlength="1024" rows="4" placeholder="Explain why you accept this result despite the Council findings"></textarea>
    <div class="dialog-footer"><button type="button" class="secondary-button" data-shared-action="accept-cancel">Cancel</button>
    <button type="submit" class="primary-button">Accept with reason</button></div>
  </form>`, 'bot-shared-dialog');
  const form = dialog.querySelector<HTMLFormElement>('.bc-council-accept-form')!;
  form.addEventListener('submit', event => {
    event.preventDefault();
    const reason = String(new FormData(form).get('reason') ?? '').trim();
    if (reason.length < 8 || reason.length > 1024) { options.notify('Enter an acceptance reason (8–1024 characters).'); return; }
    const submit = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    if (submit) submit.disabled = true;
    void options.call('bot_shared_channel_council_accept', { deliveryId, reason }).then(async () => {
      options.closeModal();
      await options.changed();
    }).catch(error => options.notify(error instanceof Error ? error.message : String(error)))
      .finally(() => { if (submit?.isConnected) submit.disabled = false; });
  });
  dialog.querySelector('[data-shared-action="accept-cancel"]')?.addEventListener('click', options.closeModal);
}
