/* Explicit GitHub discovery/import inside the existing Skills & rules settings. */
import { escape } from './dom';
import type { SkillsLibraryHost, SkillItem } from './skills-library';
export type CatalogSkill = { path: string; id: string; name: string; description: string; files: number; bytes: number; imported: string | null; conflicts: string[] };
export type SkillCatalog = { repo: string; gitRef: string | null; commit: string; state: 'enabled' | 'disabled' | 'ignored'; scannedAt: number; skills: CatalogSkill[] };
export type SkillCatalogs = { sources: SkillCatalog[] };

export function mountSkillCatalogs(host: HTMLElement, options: SkillsLibraryHost, changed: () => void): () => void {
  let catalogs: SkillCatalogs = { sources: [] }, busy = false, disposed = false, note = '', error = '';
  const selected = new Map<string, Set<string>>();
  const text = (error: unknown) => error instanceof Error ? error.message : String(error);
  async function refresh(): Promise<void> {
    try { catalogs = await options.call<SkillCatalogs>('skills_catalogs'); } catch (e) { error = text(e); }
    if (!disposed) draw();
  }
  async function run(work: () => Promise<void>): Promise<void> {
    busy = true; note = ''; error = ''; draw();
    try { await work(); } catch (e) { error = text(e); }
    busy = false; if (!disposed) await refresh();
  }
  function draw(): void {
    const repo = host.querySelector<HTMLInputElement>('[name="catalogRepo"]')?.value ?? '';
    const ref = host.querySelector<HTMLInputElement>('[name="catalogRef"]')?.value ?? '';
    host.innerHTML = `<h3 class="settings-heading">GitHub catalogs</h3><div class="settings-group skills-catalogs">
      <p class="skills-note">Scan a repository for nested SKILL.md packages. Import only what you select, then activate or assign it separately.</p>
      <form class="skills-import-row" aria-label="Add a skill catalog"><input name="catalogRepo" aria-label="Catalog repository" placeholder="owner/name" value="${escape(repo)}" autocomplete="off" spellcheck="false" required><input name="catalogRef" aria-label="Catalog branch, tag or commit" placeholder="Default branch" value="${escape(ref)}" autocomplete="off" spellcheck="false"><button class="secondary-button" type="submit" ${busy ? 'disabled' : ''}>Add catalog</button></form>
      <p class="skills-note" aria-live="polite">${escape(busy ? 'Reading catalog…' : note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p>
      ${catalogs.sources.length ? catalogs.sources.map(source => {
        const chosen = selected.get(source.repo) ?? new Set();
        return `<details class="skills-catalog" data-repo="${escape(source.repo)}" open><summary><strong>${escape(source.repo)}</strong> · ${source.skills.length} skills · ${source.state}</summary>
          <p class="skills-source">Pinned commit: <code>${escape(source.commit)}</code>${source.gitRef ? ` · ${escape(source.gitRef)}` : ''}</p>
          <div class="skills-import-row"><select data-state aria-label="${escape(`State of ${source.repo}`)}" ${busy ? 'disabled' : ''}>${['enabled', 'disabled', 'ignored'].map(state => `<option value="${state}" ${source.state === state ? 'selected' : ''}>${state.charAt(0).toUpperCase() + state.slice(1)}</option>`).join('')}</select>
          <button type="button" class="secondary-button" data-action="refresh" ${busy || source.state !== 'enabled' ? 'disabled' : ''}>Refresh catalog</button><button type="button" class="secondary-button" data-action="remove" ${busy ? 'disabled' : ''}>Remove catalog</button></div>
          <ul class="skills-catalog-list">${source.skills.map(skill => `<li><label class="skills-toggle"><input type="checkbox" data-path="${escape(skill.path)}" aria-label="${escape(`Select ${skill.name} from ${source.repo}`)}" ${chosen.has(skill.path) ? 'checked' : ''} ${busy || source.state !== 'enabled' ? 'disabled' : ''}><span><strong>${escape(skill.name)}</strong><small>${escape(skill.path || '(repository root)')} · ${skill.files} files · ${skill.imported ? skill.imported === source.commit ? 'Imported' : 'Update available' : 'Available'}</small><small>${escape(skill.description)}</small>${skill.conflicts.length ? `<small class="skills-error">Name conflict: ${escape(skill.conflicts.join(', '))}</small>` : ''}</span></label></li>`).join('')}</ul>
          <div class="skills-import-row"><button type="button" class="secondary-button" data-action="all" ${busy || source.state !== 'enabled' ? 'disabled' : ''}>Select all</button><button type="button" class="secondary-button" data-action="import" ${busy || source.state !== 'enabled' || !chosen.size ? 'disabled' : ''}>Import selected (${chosen.size})</button></div></details>`;
      }).join('') : '<p class="skills-empty">No catalogs. Add a repository to discover its skills.</p>'}</div>`;
  }
  const submit = (event: Event) => {
    event.preventDefault(); if (busy) return;
    const data = new FormData(event.target as HTMLFormElement);
    const repo = String(data.get('catalogRepo') ?? '').trim();
    const gitRef = String(data.get('catalogRef') ?? '').trim() || null;
    void run(async () => { catalogs = await options.call('skills_catalog_add', { repo, gitRef }); note = 'Catalog indexed. No skill was imported or activated.'; });
  };
  const change = (event: Event) => {
    const target = event.target as HTMLInputElement | HTMLSelectElement;
    const repo = target.closest<HTMLElement>('[data-repo]')?.dataset.repo;
    if (!repo || busy) return;
    if (target instanceof HTMLInputElement && target.hasAttribute('data-path')) {
      const paths = selected.get(repo) ?? new Set<string>();
      if (target.checked) paths.add(target.dataset.path!); else paths.delete(target.dataset.path!);
      selected.set(repo, paths); draw();
    } else if (target.hasAttribute('data-state')) void run(async () => { await options.call('skills_catalog_state', { repo, state: target.value }); });
  };
  const click = (event: Event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-action]');
    const repo = button?.closest<HTMLElement>('[data-repo]')?.dataset.repo;
    if (!button || !repo || busy) return;
    const source = catalogs.sources.find(s => s.repo === repo); if (!source) return;
    if (button.dataset.action === 'all') { selected.set(repo, new Set(source.skills.map(s => s.path))); draw(); return; }
    if (button.dataset.action === 'refresh') void run(async () => { await options.call('skills_catalog_refresh', { repo }); note = 'Index refreshed. Library copies and assignments are unchanged.'; });
    if (button.dataset.action === 'remove') void run(async () => { await options.call('skills_catalog_state', { repo, state: null }); selected.delete(repo); note = 'Catalog removed. Imported skills are still in the library.'; });
    if (button.dataset.action === 'import') void run(async () => {
      const result = await options.call<{ items: SkillItem[]; imported: string[]; skipped: { name: string; reason: string }[] }>('skills_catalog_import', { repo, paths: [...(selected.get(repo) ?? [])] });
      note = `Imported ${result.imported.length}. Nothing activated.${result.skipped.length ? ` Skipped: ${result.skipped.map(s => `${s.name}: ${s.reason}`).join(' ')}` : ''}`;
      selected.delete(repo); changed();
    });
  };
  host.addEventListener('submit', submit); host.addEventListener('change', change); host.addEventListener('click', click);
  draw(); void refresh();
  return () => { disposed = true; host.removeEventListener('submit', submit); host.removeEventListener('change', change); host.removeEventListener('click', click); };
}
