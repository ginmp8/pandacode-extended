/* Bot/project selections are content-pinned, separate from file-based harness activation. */
import { escape } from './dom';
import type { SkillItem, SkillsLibraryHost } from './skills-library';
export type SkillPin = { id: string; sha256: string };
export function mountSkillScopes(host: HTMLElement, options: SkillsLibraryHost, fixed?: { target: 'bot' | 'project'; id: string }): () => void {
  let items: SkillItem[] = [], bots: { id: string; name: string }[] = [], pins: SkillPin[] = [];
  let target: 'bot' | 'project' = fixed?.target ?? 'project', targetId = fixed?.id ?? '', chosen = new Set<string>();
  let effective: { id: string; name: string; provenance: string; delivery: string; reason: string | null }[] = [], effectiveError = '';
  let busy = false, disposed = false, note = '', error = '', generation = 0;
  const targets = () => target === 'bot' ? bots : options.projects?.() ?? [];
  const message = (e: unknown) => e instanceof Error ? e.message : String(e);
  async function refresh(): Promise<void> {
    const request = ++generation;
    try {
      const [nextItems, nextBots] = await Promise.all([options.call<SkillItem[]>('skills_list'), options.call<{ id: string; name: string }[]>('bots_list')]);
      if (disposed || request !== generation) return;
      items = nextItems.filter(i => i.kind === 'skill'); bots = nextBots;
      if (!fixed && !targets().some(t => t.id === targetId)) targetId = targets()[0]?.id ?? '';
      const nextPins = targetId ? await options.call<SkillPin[]>('skills_scope_get', { target, targetId }) : [];
      if (disposed || request !== generation) return;
      pins = nextPins; chosen = new Set(pins.map(p => p.id));
      effective = []; effectiveError = '';
      const sessionId = options.sessionId?.();
      if (sessionId) {
        try {
          const resolution = await options.call<{ items: typeof effective }>('skills_task_resolve', { sessionId });
          if (request === generation) effective = resolution.items;
        } catch (e) { if (request === generation) effectiveError = message(e); }
      }
    } catch (e) { if (request === generation) error = message(e); }
    if (!disposed && request === generation) draw();
  }
  function draw(): void {
    host.innerHTML = `<h3 class="settings-heading">${fixed ? 'Selected skills' : 'Bot & project skills'}</h3><div class="settings-group skills-scopes">
      <p class="skills-note">Select the exact skill versions for this bot or project. Tasks merge both scopes by id. Scoped additions use bounded context; existing native activations keep their directories.</p>
      <form aria-label="Assign skills to a bot or project"><div class="skills-import-row" ${fixed ? 'hidden' : ''}><select name="scopeTarget" aria-label="Skill assignment scope" ${busy ? 'disabled' : ''}><option value="project" ${target === 'project' ? 'selected' : ''}>Project</option><option value="bot" ${target === 'bot' ? 'selected' : ''}>Bot</option></select><select name="scopeId" aria-label="Skill assignment target" ${busy || !targets().length ? 'disabled' : ''}>${targets().map(t => `<option value="${escape(t.id)}" ${t.id === targetId ? 'selected' : ''}>${escape(t.name)}</option>`).join('')}</select></div>
      ${targetId ? `<ul class="skills-catalog-list">${items.map(item => {
        const pin = pins.find(p => p.id === item.id), stale = pin && pin.sha256 !== item.sha256;
        return `<li><label class="skills-toggle"><input type="checkbox" data-skill="${escape(item.id)}" aria-label="${escape(`Assign ${item.name}`)}" ${chosen.has(item.id) ? 'checked' : ''} ${busy ? 'disabled' : ''}><span><strong>${escape(item.name)}</strong><small>${escape(item.description)}</small><small>${stale ? 'New version needs review and Save' : pin ? 'Selected version' : 'Available'} · ${escape(item.sha256.slice(0, 12))}${item.findings.length ? ` · ${item.findings.length} safety findings — review in the Skills library` : ''}</small></span></label></li>`;
      }).join('')}${pins.filter(p => !items.some(i => i.id === p.id)).map(pin => `<li class="skills-error">Missing: ${escape(pin.id)}. Save to remove this stale selection.</li>`).join('')}</ul><label class="skills-toggle"><input type="checkbox" name="acknowledge" ${busy ? 'disabled' : ''}><span>I reviewed and accept the safety findings of the selected versions</span></label><button type="submit" class="secondary-button" ${busy ? 'disabled' : ''}>Save skill selection</button>` : '<p class="skills-empty">Create a bot or add a project to assign skills.</p>'}
      <p class="skills-note" aria-live="polite">${escape(busy ? 'Saving…' : note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p></form>
      ${options.sessionId?.() ? `<h4 class="skills-subheading">Current task: effective skills</h4><button type="button" class="secondary-button" data-scope-refresh ${busy ? 'disabled' : ''}>Refresh current task</button>${effectiveError ? `<p class="skills-error">${escape(effectiveError)}</p>` : effective.length ? `<ul class="skills-catalog-list" aria-label="Effective skills">${effective.map(item => `<li><strong>${escape(item.name)}</strong> · ${escape(item.provenance)} · ${escape(item.delivery)}${item.reason ? `<small>${escape(item.reason)}</small>` : ''}</li>`).join('')}</ul>` : '<p class="skills-empty">No assigned skills apply to this task.</p>'}` : ''}</div>`;
  }
  const change = (event: Event) => {
    const input = event.target as HTMLInputElement | HTMLSelectElement; if (busy || disposed) return;
    if (!fixed && input.name === 'scopeTarget') { target = input.value as typeof target; targetId = ''; note = ''; error = ''; void refresh(); }
    else if (!fixed && input.name === 'scopeId') { targetId = input.value; note = ''; error = ''; void refresh(); }
    else if (input instanceof HTMLInputElement && input.dataset.skill) { if (input.checked) chosen.add(input.dataset.skill); else chosen.delete(input.dataset.skill); }
  };
  const submit = async (event: Event) => {
    event.preventDefault(); event.stopPropagation(); if (busy || disposed || !targetId) return;
    const acknowledge = host.querySelector<HTMLInputElement>('[name="acknowledge"]')?.checked ?? false;
    const ids = [...chosen].filter(id => items.some(i => i.id === id));
    busy = true; note = ''; error = ''; draw();
    try { await options.call('skills_scope_set', { target, targetId, ids, acknowledge }); note = 'Selection saved for future turns. Existing tasks keep their history.'; }
    catch (e) { error = message(e); }
    busy = false; if (!disposed) await refresh();
  };
  const click = (event: Event) => { if (!busy && (event.target as HTMLElement).closest('[data-scope-refresh]')) void refresh(); };
  host.addEventListener('change', change); host.addEventListener('submit', submit); host.addEventListener('click', click);
  draw(); void refresh();
  return () => { disposed = true; generation++; host.removeEventListener('change', change); host.removeEventListener('submit', submit); host.removeEventListener('click', click); };
}
