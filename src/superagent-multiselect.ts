import { botAvatarHtml } from './bot-avatars';
import { escape } from './dom';

export interface MultiSelectBot {
  id: string;
  name: string;
  paused: boolean;
  avatar?: string;
  color?: string;
  projectId?: string;
  provider?: string;
  model?: string;
}

interface MultiSelectOptions {
  name: string;
  label: string;
  bots: MultiSelectBot[];
  selectedIds: string[];
  limit: number;
  disabled?: boolean;
  unavailable?: (bot: MultiSelectBot) => string;
}

function avatar(bot: MultiSelectBot): string {
  return botAvatarHtml(bot.avatar ?? '', bot.name, 'sa-multi-avatar', bot.color ?? '');
}

/** Reusable, form-local picker. Native bot identities and images come from bots_list. */
export function renderBotMultiSelect(options: MultiSelectOptions): string {
  const { name, label, bots, selectedIds, limit, disabled = false, unavailable } = options;
  const selected = selectedIds.map(id => bots.find(bot => bot.id === id))
    .filter((bot): bot is MultiSelectBot => bot !== undefined);
  const names = selected.map(bot => bot.name);
  const caption = names.length ? names.slice(0, 2).join(', ') +
    (names.length > 2 ? ' +' + (names.length - 2) : '') : 'Select existing bots';
  const faces = selected.slice(0, 3).map(avatar).join('');
  const rows = bots.map(bot => {
    const checked = selectedIds.includes(bot.id);
    const reason = bot.paused ? 'Paused' : unavailable?.(bot) ?? '';
    const blocked = disabled || (!checked && (Boolean(reason) || selectedIds.length >= limit));
    const status = disabled ? 'Team enabled' : reason || (!checked && selectedIds.length >= limit ? 'Limit reached' : '');
    const searchable = (bot.name + ' ' + (bot.provider ?? '') + ' ' + (bot.model ?? '')).toLocaleLowerCase();
    return '<label class="sa-multi-option' + (checked ? ' is-selected' : '') +
      (blocked ? ' is-disabled' : '') + '" data-sa-multi-row data-sa-multi-search-text="' +
      escape(searchable) + '">' +
      '<input type="checkbox" data-sa-multi-option data-sa-multi-scope="' + escape(name) +
      '" name="' + escape(name) + '" value="' + escape(bot.id) + '"' +
      (checked ? ' checked' : '') + (blocked ? ' disabled' : '') + '>' + avatar(bot) +
      '<span class="sa-multi-option-text"><strong>' + escape(bot.name) + '</strong><small>' +
      escape([bot.provider, bot.model].filter(Boolean).join(' · ') || 'Existing bot') +
      '</small></span>' + (status ? '<small class="sa-multi-reason">' + escape(status) + '</small>' : '') +
      '</label>';
  }).join('');
  return '<div class="sa-multi-field">' +
    '<span class="sa-multi-label" id="sa-multi-label-' + escape(name) + '">' + escape(label) + '</span>' +
    '<details class="sa-multi" data-sa-multi="' + escape(name) + '">' +
    '<summary aria-label="' + escape(label) + ': ' + selected.length + ' selected">' +
    '<span class="sa-multi-faces" aria-hidden="true">' + faces + '</span>' +
    '<span class="sa-multi-value">' + escape(caption) + '</span>' +
    '<span class="sa-multi-count">' + selected.length + ' / ' + limit + '</span>' +
    '<span class="sa-multi-chevron" aria-hidden="true"></span></summary>' +
    '<div class="sa-multi-popover">' +
    '<div class="sa-multi-search"><input type="search" data-sa-multi-query ' +
    'aria-label="Search ' + escape(label) + '" placeholder="Search bots..." autocomplete="off"></div>' +
    '<div class="sa-multi-options">' + (rows || '<p class="sa-multi-empty">No available bots</p>') +
    '<p class="sa-multi-empty" data-sa-multi-empty hidden>No matching bots</p></div>' +
    '<footer class="sa-multi-footer"><span>' + selected.length + ' of ' + limit + ' selected</span>' +
    '<div><button type="button" data-sa-multi-clear="' + escape(name) + '"' +
    (!selectedIds.length || disabled ? ' disabled' : '') + '>Clear</button>' +
    '<button type="button" data-sa-multi-done="' + escape(name) + '">Done</button></div>' +
    '</footer></div></details></div>';
}

export function filterBotMultiSelect(search: HTMLInputElement): void {
  const picker = search.closest<HTMLDetailsElement>('details[data-sa-multi]');
  if (!picker) return;
  const query = search.value.trim().toLocaleLowerCase();
  let visible = 0;
  picker.querySelectorAll<HTMLElement>('[data-sa-multi-row]').forEach(row => {
    const matches = (row.dataset.saMultiSearchText ?? '').includes(query);
    row.hidden = !matches;
    if (matches) visible++;
  });
  const empty = picker.querySelector<HTMLElement>('[data-sa-multi-empty]');
  if (empty) empty.hidden = visible !== 0;
}

export function captureBotMultiSelectState(host: HTMLElement): Map<string, {
  open: boolean; search: string; scroll: number;
}> {
  const state = new Map<string, { open: boolean; search: string; scroll: number }>();
  host.querySelectorAll<HTMLDetailsElement>('details[data-sa-multi]').forEach(details => {
    const key = details.dataset.saMulti;
    if (!key) return;
    state.set(key, {
      open: details.open,
      search: details.querySelector<HTMLInputElement>('[data-sa-multi-query]')?.value ?? '',
      scroll: details.querySelector<HTMLElement>('.sa-multi-options')?.scrollTop ?? 0,
    });
  });
  return state;
}

export function restoreBotMultiSelectState(host: HTMLElement, state: ReturnType<typeof captureBotMultiSelectState>): void {
  host.querySelectorAll<HTMLDetailsElement>('details[data-sa-multi]').forEach(details => {
    const previous = state.get(details.dataset.saMulti ?? '');
    if (!previous) return;
    details.open = previous.open;
    const search = details.querySelector<HTMLInputElement>('[data-sa-multi-query]');
    if (search) { search.value = previous.search; filterBotMultiSelect(search); }
    const list = details.querySelector<HTMLElement>('.sa-multi-options');
    if (list) list.scrollTop = previous.scroll;
  });
}
