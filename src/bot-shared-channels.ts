import type { Bot } from './bots';
import { botAvatarHtml } from './bot-avatars';
import { escape, icon } from './dom';
import { WORKING_LABELS } from './bot-thread';

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
}

const channelName = (name: string) => `#${name.replace(/^#+/, '')}`;

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
  const oldInputs = [...previous.querySelectorAll<HTMLInputElement>('input[name^="council"]')];
  for (const input of next.querySelectorAll<HTMLInputElement>('input[name^="council"]')) {
    const old = oldInputs.find(item => item.name === input.name
      && (item.type === 'checkbox' || item.type === 'radio' ? item.value === input.value : true));
    if (!old) continue;
    if (input.type === 'checkbox' || input.type === 'radio') input.checked = old.checked;
    else input.value = old.value;
  }
}

export function sharedChannelComposerHtml(channel: SharedChannelDetail, bots: Bot[] = []): string {
  if (channel.archived || !channel.memberIds.length) return '';
  const members = channel.memberIds.map(id => bots.find(bot => bot.id === id)).filter((bot): bot is Bot => Boolean(bot));
  const options = members.map(bot => `<label><input name="councilReviewer" type="checkbox" value="${escape(bot.id)}"><span>${escape(bot.name)}</span></label>`).join('');
  return `<form id="shared-channel-composer" data-channel-id="${escape(channel.id)}" class="composer bc-composer"><textarea id="shared-channel-prompt" name="text" aria-label="Message channel" aria-description="Enter to send. Shift Enter for a new line." rows="2" placeholder="Message ${escape(channelName(channel.name))}… @mention up to two bots"></textarea>
    <div class="composer-toolbar"><button type="button" class="bc-council-trigger composer-council-button" data-action="channel-council" aria-label="Council: off" aria-haspopup="dialog" aria-expanded="false" aria-controls="bc-council-panel" title="Council settings"><span class="council-glyph"><svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v16M8 20h8M5 7h14M5 7l-3 6.5a3 3 0 0 0 6 0zM19 7l-3 6.5a3 3 0 0 0 6 0z"/></svg></span></button><span class="bc-composer-spacer"></span><button type="submit" class="send-button" title="Send message" aria-label="Send message">${icon('arrow')}</button></div>
    <section id="bc-council-panel" class="floating-menu council-panel bc-council-panel" role="dialog" aria-labelledby="bc-council-title" popover="auto">
      <div class="bc-council-controls">
        <div class="council-panel-head"><label id="bc-council-title" for="bc-council-enabled">Council</label><input id="bc-council-enabled" class="ds-toggle council-switch" type="checkbox" role="switch" name="councilEnabled" aria-describedby="bc-council-about"></div>
        <p id="bc-council-about">Reviewers score this message until they reach your target. These settings apply only to this message.</p>
        <fieldset class="bc-council-options"><legend class="sr-only">Council settings for this message</legend>
        <div class="council-field"><span id="bc-council-target-label">Target score</span><div class="bc-council-value-row"><div class="ds-seg" role="group" aria-labelledby="bc-council-target-label"><button type="button" data-council-preset="target" data-value="7" aria-pressed="false">7.0</button><button type="button" data-council-preset="target" data-value="8" aria-pressed="false">8.0</button><button type="button" data-council-preset="target" data-value="8.5" aria-pressed="false">8.5</button><button type="button" data-council-preset="target" data-value="9" aria-pressed="true">9.0</button><button type="button" data-council-preset="target" data-value="9.5" aria-pressed="false">9.5</button><button type="button" data-council-preset="target" data-value="10" aria-pressed="false">10.0</button></div><input type="number" name="councilTarget" aria-label="Custom target score" title="Target score (1–10)" min="1" max="10" step="0.5" value="9"></div></div>
        <div class="council-field"><span id="bc-council-rounds-label">Max rounds</span><div class="bc-council-value-row"><div class="ds-seg" role="group" aria-labelledby="bc-council-rounds-label"><button type="button" data-council-preset="rounds" data-value="3" aria-pressed="false">3</button><button type="button" data-council-preset="rounds" data-value="5" aria-pressed="false">5</button><button type="button" data-council-preset="rounds" data-value="10" aria-pressed="true">10</button><button type="button" data-council-preset="rounds" data-value="25" aria-pressed="false">25</button></div><input type="number" name="councilMaxRounds" aria-label="Custom max rounds" title="Max rounds (1–100)" min="1" max="100" step="1" value="10"></div></div>
        <div class="bc-council-mode-group"><span class="bc-council-section-label">Reviewers</span>
          <div class="bc-council-modes ds-seg" role="group" aria-label="Reviewer mode">
            <label><input type="radio" name="councilMode" value="auto" checked><span>Auto</span></label>
            <label><input type="radio" name="councilMode" value="selected"><span>Bots</span></label>
          </div>
        </div>
        <div class="bc-council-reviewer-options"><p>Choose 2–8 members other than any bot assigned this message.</p>${options}</div>
        <div class="bc-council-followup"><label><input type="checkbox" name="councilGithubIssue"> After Council approval, create one GitHub issue with accepted findings</label>
          <label>Authorized repository (owner/name) <input type="text" name="councilGithubRepo" maxlength="201" placeholder="owner/repository" autocomplete="off"></label>
          <small>Uses the existing signed-in GitHub CLI. A failed or uncertain external write is reported separately; it never changes the Council verdict.</small></div>
        <div class="bc-council-selected-permissions"><label><input type="checkbox" name="councilScopedFile"> Allow one exact native file-inspection action</label><label>Authorized relative file path <input type="text" name="councilFilePath" maxlength="240" placeholder="src/example.ts" autocomplete="off"></label><small>Read-only by default. This does not authorize shell commands or checkout writes.</small></div>
        </fieldset>
      </div>
    </section></form>`;
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
  };
  panel.addEventListener('click', event => {
    const choice = (event.target as Element).closest<HTMLButtonElement>('button[data-council-preset]');
    if (!choice) return;
    const name = choice.dataset.councilPreset === 'target' ? 'councilTarget' : 'councilMaxRounds';
    const input = form.elements.namedItem(name) as HTMLInputElement | null;
    if (!input) return;
    input.value = choice.dataset.value ?? input.value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  panel.addEventListener('keydown', event => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    panel.hidePopover();
    trigger.focus({ preventScroll: true });
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
