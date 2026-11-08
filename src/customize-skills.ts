/* Standalone Customize -> Skills workspace. Owns no package storage or harness writes:
 * the native skills_* commands share one library with Settings -> Skills & rules. */
import './customize-skills.css';
import { escape, icon, markdownHtml } from './dom';
import type { SkillItem, CatalogSkill } from './skills-library';

type SavedSource = {
  id: string; repo: string; path: string; gitRef: string | null;
  commit: string; selectedPaths: string[]; savedAt: number;
};
type SourcePreview = {
  repo: string; path: string; gitRef: string | null; commit: string; skills: CatalogSkill[];
};
type SourceImport = { source: SavedSource; imported: string[]; skipped: {path: string; name: string; reason: string}[] };
type SyncReport = {
  checkedAt: number; installed: string[]; updated: string[]; skipped: string[];
  events: { id: string; status: string; oldCommit: string | null; newCommit: string | null; impactedHarnesses: string[]; detail: string }[];
};
type Policy = { autoOnStart: boolean };
type PreviewFile = { path: string; text: string | null; size: number; binary: boolean };
type Phase = 'queued' | 'fetching' | 'validating' | 'installing' | 'installed' | 'updated' | 'unchanged' | 'skipped' | 'failed' | 'cancelled';
type PackageProgress = { path: string; name: string; phase: Phase; detail: string | null };
type ImportSnapshot = { repo: string; path: string; gitRef: string | null; commit: string; paths: string[]; allowReplace?: boolean };
type Operation = {
  id: string; kind: 'github' | 'zip' | 'sync'; label: string; source: string; revision: string | null;
  state: 'running' | 'success' | 'partial' | 'failed';
  total: number; completed: number; packages: Map<string, PackageProgress>;
  snapshot?: ImportSnapshot; detail: string;
};
type Detail = { kind: 'installed'; id: string; file: string } | { kind: 'remote'; path: string; file: string };
export type CustomizeSkillsHost = {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  subscribe(listener: (event: any) => void): Promise<() => void>;
  chooseZip(): Promise<string | null>;
  close(): void;
};
const message = (error: unknown): string => error instanceof Error ? error.message : String(error);
const initialPolicy: Policy = { autoOnStart: false };
const plural = (amount: number, word: string) => `${amount} ${word}${amount === 1 ? '' : 's'}`;
const shaShort = (sha: string | null | undefined) => sha ? sha.slice(0, 12) : 'Not pinned';
const isTerminal = (phase: Phase) => ['installed', 'updated', 'unchanged', 'skipped', 'failed', 'cancelled'].includes(phase);
const stateLabel = (phase: Phase) => ({ queued:'Queued', fetching:'Downloading', validating:'Validating', installing:'Installing', installed:'Installed', updated:'Updated', unchanged:'Unchanged', skipped:'Skipped', failed:'Failed', cancelled:'Cancelled' } as Record<Phase,string>)[phase];

export function mountCustomizeSkills(host: HTMLElement, options: CustomizeSkillsHost): {dispose(): void} {
  let installed: SkillItem[] = [];
  let sources: SavedSource[] = [];
  let policy = initialPolicy;
  let tab: 'installed' | 'sources' = 'installed';
  let addOpen = false, githubForm = false;
  let githubUrl = '', githubRef = '', search = '', discoverySearch = '';
  let scan: SourcePreview | null = null;
  let chosen = new Set<string>();
  let editingSource: string | null = null, replaceConflicts = false;
  let detail: Detail | null = null;
  let markdownMode: 'rendered' | 'raw' = 'rendered';
  let previewText: PreviewFile | null = null;
  let previewSerial = 0;
  let pendingRemove: string | null = null;
  let busy = false, error = '', note = '', disposed = false;
  let operation: Operation | null = null;
  let unsub: (() => void) | undefined;
  const esc = (s: unknown) => escape(s);

  function operationHtml(): string {
    if (!operation) return '';
    const op = operation;
    const values = [...op.packages.values()];
    const count = (phase: Phase) => values.filter(entry => entry.phase === phase).length;
    const successes = count('installed') + count('updated');
    const failures = count('failed'), skipped = count('skipped'), unchanged = count('unchanged');
    const title = op.state === 'running' ? op.label
      : op.state === 'success' ? 'Completed successfully'
      : op.state === 'partial' ? 'Completed with errors' : 'Operation failed';
    const processing = values.find(entry => !isTerminal(entry.phase));
    const canRetry = op.kind === 'github' && op.state !== 'running' && op.snapshot
      && values.some(entry => entry.phase === 'failed' || entry.phase === 'skipped');
    return `<section class="customize-operation ds-card" aria-label="Skill operation status" aria-live="off">
      <div class="customize-operation-title">
        <div><h3>${esc(title)}</h3><p>${esc(op.source)}${op.revision ? ` · <code>${esc(shaShort(op.revision))}</code>` : ''}</p></div>
        <span class="customize-operation-count" aria-live="polite">${op.completed} of ${op.total} processed</span>
      </div>
      <progress max="${Math.max(1, op.total)}" value="${op.completed}" aria-label="Packages processed"></progress>
      <p class="customize-help" role="status" aria-live="polite">${esc(op.state === 'running' ? processing ? `${processing.name}: ${stateLabel(processing.phase)}` : 'Preparing the next package…' : `${successes} installed or updated · ${unchanged} unchanged · ${skipped} skipped · ${failures} failed`)}</p>
      ${op.detail ? `<p class="customize-operation-detail" role="alert">${esc(op.detail)}</p>` : ''}
      <ul class="customize-operation-packages" aria-label="Per-package results">${values.map(item => `<li><span class="customize-package-state" data-phase="${item.phase}">${esc(stateLabel(item.phase))}</span><span class="customize-package-name"><strong>${esc(item.name)}</strong><small>${esc(item.path)}${item.detail ? ` · ${esc(item.detail)}` : ''}</small></span></li>`).join('')}</ul>
      <div class="customize-operation-actions">
        <button type="button" class="secondary-button" data-customize="view-installed">View installed skills</button>
        ${canRetry ? `<button type="button" class="secondary-button" data-customize="retry-failed">Retry failed (${failures + skipped})</button>` : ''}
      </div>
    </section>`;
  }

  function renderOperation(): void {
    const slot = host.querySelector<HTMLElement>('#customize-operation-slot');
    if (slot) slot.innerHTML = operationHtml();
  }

  function installedHtml(): string {
    const visible = installed.filter(item => [item.name, item.id, item.description, item.source.location]
      .some(value => value.toLowerCase().includes(search.trim().toLowerCase())));
    const card = (item: SkillItem) => {
      const type = item.source.kind === 'github' ? 'GitHub' : item.source.kind === 'zip' ? 'ZIP' : 'Local';
      const active = item.activations?.length ?? 0;
      return `<li class="customize-skill-row">
        <button type="button" class="customize-skill-main" data-customize="inspect-installed" data-id="${esc(item.id)}" aria-label="Inspect ${esc(item.name)}">
          <span class="customize-skill-icon">${icon('spark')}</span>
          <span class="customize-skill-description"><strong>${esc(item.name)}</strong><span>${esc(item.description || item.id)}</span><small>${esc(item.id)} · ${esc(type)}${item.source.commit ? ` · ${esc(shaShort(item.source.commit))}` : ''}</small></span>
        </button>
        <span class="customize-skill-trailing">
          ${item.findings?.length ? `<small class="customize-warning">${plural(item.findings.length, 'safety finding')}</small>` : ''}
          ${active ? `<small>${plural(active, 'activation')}</small>` : ''}
          ${item.source.kind === 'github' ? `<button type="button" class="secondary-button" data-customize="sync-skill" data-id="${esc(item.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Sync</button>` : ''}
        </span>
      </li>`;
    };
    return `<div class="customize-controls"><label class="customize-search">${icon('search')}<input name="installed-search" aria-label="Search installed skills" placeholder="Search installed skills" type="search" value="${esc(search)}" autocomplete="off"></label><span class="customize-summary">${plural(visible.length,'skill')}</span></div>
      ${visible.length ? `<ul class="customize-skill-list" aria-label="Installed skills">${visible.map(card).join('')}</ul>` : `<div class="customize-empty"><h3>${installed.length ? 'No matching skills' : 'No skills installed'}</h3><p>${installed.length ? 'Try a different search.' : 'Add a ZIP archive or discover skills from GitHub to get started.'}</p></div>`}`;
  }

  function sourcesHtml(): string {
    const rows = sources.map(source => `<li class="customize-source-row">
      <div class="customize-source-identity">
        <strong>${esc(source.repo)}</strong><small>${esc(source.path || 'Repository root')} · ${esc(source.gitRef ?? 'Default branch')}</small>
        <small>${plural(source.selectedPaths.length, 'tracked skill')} · commit <code>${esc(shaShort(source.commit))}</code></small>
      </div>
      <div class="customize-source-actions">
        <button type="button" class="secondary-button" data-customize="edit-source" data-id="${esc(source.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Edit selection</button>
        <button type="button" class="secondary-button" data-customize="sync-source" data-id="${esc(source.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Sync source</button>
        <button type="button" class="secondary-button" data-customize="remove-source" data-id="${esc(source.id)}">${pendingRemove === source.id ? 'Confirm remove' : 'Remove source'}</button>
      </div>
    </li>`).join('');
    return `<section class="customize-sources-section">
      <div class="customize-source-settings">
        <div><h3>Automatic synchronization</h3><p>Check previously imported GitHub skills once when PandaCode starts. No continuous polling or new-skill installation.</p></div>
        <label class="customize-switch"><input type="checkbox" data-customize-toggle="startup-sync" ${policy.autoOnStart ? 'checked' : ''} ${busy ? 'disabled' : ''}>On startup</label>
      </div>
      <div class="customize-source-heading"><h3>Saved GitHub sources</h3><button type="button" class="secondary-button" data-customize="sync-all" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Sync all GitHub skills</button></div>
      ${rows ? `<ul class="customize-source-list">${rows}</ul>` : '<div class="customize-empty">No saved sources. Import selected GitHub skills to save a source.</div>'}
    </section>`;
  }

  function discoveryHtml(): string {
    if (!scan) return '';
    const query = discoverySearch.trim().toLowerCase();
    const shown = scan.skills.filter(entry => [entry.name, entry.path, entry.description].some(value => value.toLowerCase().includes(query)));
    return `<section class="customize-discovery ds-card" aria-label="Discovered GitHub skills">
      <div class="customize-discovery-heading"><div><h3>Found ${plural(scan.skills.length, 'skill')}</h3>
        <p>${esc(scan.repo)}${scan.path ? ` / ${esc(scan.path)}` : ''} · <code>${esc(scan.commit)}</code></p></div>
        <button type="button" class="secondary-button" data-customize="hide-discovery">Close list</button>
      </div>
      <div class="customize-controls">
        <label class="customize-search">${icon('search')}<input type="search" name="discovery-search" aria-label="Filter discovered skills" placeholder="Filter discovered skills" value="${esc(discoverySearch)}"></label>
        <button type="button" class="secondary-button" data-customize="select-all" ${busy ? 'disabled' : ''}>Select shown</button>
        <button type="button" class="secondary-button" data-customize="select-none" ${busy ? 'disabled' : ''}>Clear selection</button>
      </div>
      <div class="customize-discovery-scroll"><ul class="customize-discovery-list">${shown.map(entry => {
        const installedItem = installed.find(item => item.id === entry.id);
        const conflict = installedItem && (installedItem.source.location !== `${scan!.repo}/${entry.path}` || installedItem.source.gitRef !== scan!.gitRef);
        return `<li><label class="customize-discovery-choice"><input type="checkbox" data-select-skill="${esc(entry.path)}" ${chosen.has(entry.path) ? 'checked' : ''} ${busy ? 'disabled' : ''}><span><strong>${esc(entry.name)}</strong><small>${esc(entry.description)}</small><code>${esc(entry.path || '(repository root)')}</code>${conflict ? `<small class="customize-warning">Installed from another source; import requires a deliberate replacement.</small>` : ''}</span></label>
          <button type="button" class="secondary-button" data-customize="inspect-remote" data-path="${esc(entry.path)}">Preview</button></li>`;
      }).join('')}</ul></div>
      <div class="customize-discovery-footer"><span>${plural(chosen.size, 'skill')} selected</span>
        <div class="customize-discovery-actions">
          ${scan.skills.some(item => chosen.has(item.path) && installed.some(current => current.id === item.id &&
            (current.source.location !== `${scan!.repo}/${item.path}` || current.source.gitRef !== scan!.gitRef))) ?
            `<label class="customize-confirm-replace"><input type="checkbox" name="replace-conflicts" ${replaceConflicts ? 'checked' : ''}>I approve replacing conflicting installed skills</label>` : ''}
          ${editingSource ? `<button type="button" class="secondary-button" data-customize="save-source-selection" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Save tracked selection</button>` : ''}
          <button type="button" class="customize-primary" data-customize="import-selected" ${!chosen.size || busy || operation?.state === 'running' || (scan.skills.some(item => chosen.has(item.path) && installed.some(current => current.id === item.id && (current.source.location !== `${scan!.repo}/${item.path}` || current.source.gitRef !== scan!.gitRef))) && !replaceConflicts) ? 'disabled' : ''}>Import selected (${chosen.size})</button>
        </div>
      </div>
    </section>`;
  }

  function detailHtml(): string {
    if (!detail) return '';
    const item = detail.kind === 'installed' ? installed.find(item => item.id === (detail as {id:string}).id) : scan?.skills.find(skill => skill.path === (detail as any).path);
    if (!item) return '';
    const files = detail.kind === 'installed' ? (item as SkillItem).files : ((item as CatalogSkill).filePaths?.length ? (item as CatalogSkill).filePaths! : ['SKILL.md']);
    const selectedFile = detail.file;
    const source = detail.kind === 'installed' ? (item as SkillItem).source : {kind:'github',location:scan!.repo, gitRef:scan!.gitRef,commit:scan!.commit};
    return `<section class="customize-detail ds-card" aria-label="Read-only skill preview">
      <div class="customize-detail-heading">
        <div><p class="customize-eyebrow">${detail.kind === 'remote' ? 'GITHUB PREVIEW · NOT INSTALLED' : 'INSTALLED PACKAGE'}</p>
        <h3>${esc(item.name)}</h3><p>${esc(item.description)}</p><small>${esc(source.location)} · ${esc(shaShort(source.commit))}</small></div>
        <button type="button" class="secondary-button" data-customize="close-detail">Close preview</button>
      </div>
      <div class="customize-preview-shell">
        <nav class="customize-file-tree" aria-label="Package files">
          <strong>Files</strong>
          ${files.map(path => `<button type="button" data-customize="preview-file" data-file="${esc(path)}" class="${selectedFile === path ? 'selected' : ''}">${icon('file')}<span>${esc(path)}</span></button>`).join('')}
        </nav>
        <div class="customize-preview-main">
          <div class="customize-preview-toolbar"><span>${esc(detail.file)}</span><div class="customize-mode-switch">
            <button type="button" data-customize="preview-mode" data-mode="rendered" aria-pressed="${markdownMode === 'rendered'}">Rendered</button>
            <button type="button" data-customize="preview-mode" data-mode="raw" aria-pressed="${markdownMode === 'raw'}">Raw</button>
          </div></div>
          <div id="customize-preview-content" class="customize-preview-body" role="region" aria-label="Skill file content">Loading file…</div>
        </div>
      </div>
    </section>`;
  }

  function githubFormHtml(): string {
    if (!githubForm) return '';
    return `<form class="customize-github-form ds-card" id="customize-github-form">
      <div class="customize-section-head"><div><h3>Add from GitHub</h3><p>Scan every SKILL.md in the repository or the directory linked in a tree URL. Nothing is installed until you choose packages.</p></div><button type="button" class="secondary-button" data-customize="close-github-form">Cancel</button></div>
      <label>GitHub URL<input type="url" required name="github-url" spellcheck="false" autocomplete="off" placeholder="https://github.com/owner/repo" value="${esc(githubUrl)}"></label>
      <label>Branch or full commit SHA <span class="customize-optional">Optional</span><input type="text" name="github-ref" spellcheck="false" autocomplete="off" placeholder="Default branch" value="${esc(githubRef)}"></label>
      <div class="customize-form-actions"><button type="submit" class="customize-primary" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Scan repository</button></div>
    </form>`;
  }

  function draw(): void {
    if (disposed) return;
    const active = document.activeElement as HTMLInputElement | null;
    const activeName = active && host.contains(active) ? active.name : '';
    const cursor = activeName && typeof active?.selectionStart === 'number' ? active.selectionStart : -1;
    host.innerHTML = `<div class="customize-page">
      <header class="customize-page-header">
        <div><p class="customize-eyebrow">PERSONALIZATION</p><h1>Customize</h1><p>Manage extensions for your workspace.</p></div>
        <button type="button" class="secondary-button" data-customize="close">${icon('back')}<span>Back to workspace</span></button>
      </header>
      <div class="customize-page-layout">
        <nav class="customize-category-nav" aria-label="Customization categories">
          <span class="customize-nav-heading">Categories</span><button type="button" aria-current="page">${icon('spark')}<span>Skills</span></button>
        </nav>
        <section class="customize-page-body">
          <div class="customize-toolbar">
            <div><h2>Skills</h2><p>Browse, review, import and synchronize your skills. Installed skills remain inactive until you enable them for a harness.</p></div>
            <div class="customize-toolbar-actions"><button type="button" class="secondary-button" data-customize="refresh" aria-label="Refresh skills">${icon('refresh')}<span>Refresh</span></button>
              <div class="customize-add-area"><button type="button" class="customize-primary" data-customize="toggle-add" aria-haspopup="menu" aria-expanded="${addOpen}">${icon('plus')}<span>Add</span>${icon('down')}</button>
                ${addOpen ? `<div class="customize-add-menu" role="menu" aria-label="Add a skill"><button type="button" role="menuitem" data-customize="add-zip">${icon('archive')}<span>ZIP archive</span></button><button type="button" role="menuitem" data-customize="add-github">${icon('github')}<span>GitHub link</span></button></div>` : ''}
              </div>
            </div>
          </div>
          <nav class="customize-tabs" aria-label="Skills pages"><button type="button" data-customize="tab-installed" aria-current="${tab === 'installed' ? 'page' : 'false'}">Installed <small>${installed.length}</small></button>
            <button type="button" data-customize="tab-sources" aria-current="${tab === 'sources' ? 'page' : 'false'}">GitHub sources <small>${sources.length}</small></button></nav>
          ${error ? `<p class="customize-error" role="alert">${esc(error)}</p>` : ''}
          ${note ? `<p class="customize-notice" role="status">${esc(note)}</p>` : ''}
          ${githubFormHtml()}
          ${scan ? discoveryHtml() : ''}
          <div id="customize-operation-slot">${operationHtml()}</div>
          ${tab === 'installed' ? installedHtml() : sourcesHtml()}
          ${detailHtml()}
        </section>
      </div>
    </div>`;
    if (activeName) {
      const next = [...host.querySelectorAll<HTMLInputElement>('input[name]')].find(input => input.name === activeName);
      if (next) { next.focus({ preventScroll: true }); if (cursor >= 0 && next.type !== 'checkbox' && next.type !== 'search') next.setSelectionRange(cursor,cursor); }
    }
    if (detail) void loadPreview();
  }

  async function refresh(): Promise<void> {
    try {
      const [items, saved, current] = await Promise.all([
        options.call<SkillItem[]>('skills_list'), options.call<SavedSource[]>('skills_source_list'),
        options.call<Policy>('skills_sync_policy'),
      ]);
      if (disposed) return;
      installed = items; sources = saved; policy = current;
    } catch (cause) { error = message(cause); }
    draw();
  }

  async function loadPreview(): Promise<void> {
    if (!detail) return;
    const current = detail, serial = ++previewSerial;
    const slot = host.querySelector<HTMLElement>('#customize-preview-content');
    if (!slot) return;
    slot.textContent = 'Loading file…';
    try {
      const value = current.kind === 'installed'
        ? await options.call<PreviewFile>('skills_preview_file', {id:current.id,file:current.file})
        : await options.call<PreviewFile>('skills_github_preview_file', {repo:scan!.repo,path:current.path,commit:scan!.commit,file:current.file});
      if (disposed || serial !== previewSerial || !slot.isConnected) return;
      previewText = value;
      if (value.binary || value.text === null) {
        slot.textContent = `This file cannot be rendered as text (${value.size} bytes).`; return;
      }
      if (markdownMode === 'raw' || !/\.md$/i.test(current.file)) {
        slot.innerHTML = `<pre><code>${esc(value.text)}</code></pre>`;
      } else {
        slot.innerHTML = await markdownHtml(value.text);
      }
    } catch (cause) {
      if (!disposed && serial === previewSerial) { slot.textContent = message(cause); slot.setAttribute('role','alert'); }
    }
  }

  function beginOperation(kind: Operation['kind'], label: string, source: string, revision: string | null, entries: {path: string; name: string}[], snapshot?: ImportSnapshot): Operation {
    const op: Operation = {id:crypto.randomUUID(),kind,label,source,revision,state:'running',total:entries.length,completed:0,
      packages:new Map(entries.map(entry => [entry.path,{...entry,phase:'queued',detail:null}])),snapshot,detail:''};
    operation = op; note = ''; error = ''; draw(); return op;
  }

  async function importSelected(snapshot: ImportSnapshot): Promise<void> {
    if (busy || operation?.state === 'running') return;
    const packages = snapshot.paths.map(path => {
      const skill = scan?.skills.find(item => item.path === path);
      return {path,name:skill?.name ?? path};
    });
    const op = beginOperation('github','Importing selected skills',snapshot.repo,snapshot.commit,packages,snapshot);
    busy = true;
    try {
      const result = await options.call<SourceImport>('skills_source_import_tracked',{
        repo:snapshot.repo,path:snapshot.path,gitRef:snapshot.gitRef,commit:snapshot.commit,
        selectedPaths:[...snapshot.paths],operationId:op.id,allowReplace:!!snapshot.allowReplace,
      });
      for (const fail of result.skipped) {
        const item = op.packages.get(fail.path);
        if (item && !isTerminal(item.phase)) { item.phase='failed'; item.detail=fail.reason; }
      }
      for (const entry of op.packages.values()) if (!isTerminal(entry.phase)) entry.phase='installed';
      op.completed = op.total;
      op.state = result.skipped.length ? 'partial' : 'success';
      note = `Saved ${result.source.repo}: ${plural(result.imported.length,'skill')} installed. Skills are inactive until enabled for a harness.`;
    } catch (cause) {
      op.detail = message(cause);
      for (const item of op.packages.values()) if (!isTerminal(item.phase)) { item.phase='failed'; item.detail=op.detail; }
      op.completed = op.total; op.state = 'failed';
    } finally { busy = false; await refresh(); }
  }

  async function importZip(): Promise<void> {
    if (busy || operation?.state === 'running') return;
    addOpen = false;
    let path: string | null;
    try { path = await options.chooseZip(); }
    catch (cause) { error = message(cause); draw(); return; }
    if (!path) { draw(); return; }
    const filename = path.split(/[\\/]/).pop() || 'Skill archive';
    const op = beginOperation('zip','Importing ZIP package',filename,null,[{path:filename,name:filename}]);
    busy = true;
    try {
      const result = await options.call<{item:SkillItem;replaced:boolean;previousSource:SkillItem['source']|null;impactedHarnesses:string[]}>('skills_import_zip_tracked',{path,operationId:op.id});
      const item = op.packages.get(filename)!;
      item.phase = 'installed'; item.name = result.item.name;
      op.completed = 1; op.state = 'success';
      op.detail = result.replaced
        ? `Replaced previous central library source ${result.previousSource?.location ?? 'unknown'}. ${plural(result.impactedHarnesses.length,'harness')} may require reactivation. Native harness files were not overwritten.`
        : 'Installed into the central library. No harness was activated.';
      note = `${result.replaced ? 'Replaced' : 'Installed'} ${result.item.name}.`;
      detail = {kind:'installed',id:result.item.id,file:'SKILL.md'}; tab='installed';
    } catch (cause) {
      op.completed = 1; op.state = 'failed'; op.detail = message(cause);
      const item = op.packages.get(filename)!; item.phase = 'failed'; item.detail = op.detail;
    } finally { busy=false; await refresh(); }
  }

  async function synchronize(kind: 'skill'|'source'|'all', id?: string): Promise<void> {
    if (busy || operation?.state === 'running') return;
    let entries: {path:string;name:string}[] = [], label = '';
    if (kind === 'skill') { const item=installed.find(x=>x.id===id); if (!item) return; entries=[{path:item.id,name:item.name}];label='Synchronizing skill'; }
    if (kind === 'source') {
      const source=sources.find(x=>x.id===id); if (!source) return;
      entries=installed.filter(item=>item.source.kind==='github' && item.source.gitRef===source.gitRef &&
        source.selectedPaths.some(path=>item.source.location===`${source.repo}/${path}`)).map(item=>({path:item.id,name:item.name}));
      label='Synchronizing saved source';
    }
    if (kind === 'all') {entries=installed.filter(x=>x.kind==='skill'&&x.source.kind==='github').map(x=>({path:x.id,name:x.name}));label='Synchronizing GitHub skills';}
    const op=beginOperation('sync',label,kind==='source'?sources.find(x=>x.id===id)?.repo??'Saved source': 'Installed GitHub skills',null,entries);
    busy=true;
    try {
      const command='skills_sync_tracked';
      const report=await options.call<SyncReport>(command,{scope:kind,id:id??null,operationId:op.id});
      for (const result of report.events) {
        const item=[...op.packages.values()].find(entry=>entry.path===result.id || installed.find(x=>x.id===result.id)?.source.location.endsWith('/'+entry.path));
        if(!item) continue;
        item.phase=result.status==='updated'?'updated':result.status==='error'?'failed':result.detail.startsWith('Already at')?'unchanged':'skipped';
        item.detail=result.detail;
      }
      for(const item of op.packages.values()) if(!isTerminal(item.phase)) item.phase='skipped';
      op.completed=op.total;op.state=report.events.some(x=>x.status==='error')?'partial':'success';
      op.detail=`${plural(report.updated.length,'skill')} updated. Native harness activations stay unchanged until reactivated.`;
    } catch(cause) {
      op.detail=message(cause);op.completed=op.total;op.state='failed';
      for(const item of op.packages.values()) {item.phase='failed';item.detail=op.detail;}
    } finally {busy=false; await refresh();}
  }

  async function scanGithub(): Promise<void> {
    if (busy || operation?.state==='running') return;
    if(!/^https:\/\/github\.com\/[^/?#]+\/[^/?#]+(?:\/.*)?$/i.test(githubUrl.trim())){
      error='Enter an https://github.com/owner/repo URL.';draw();return;
    }
    busy=true;error='';note='Scanning repository for SKILL.md packages…';draw();
    try {
      const result=await options.call<SourcePreview>('skills_source_preview',{repo:githubUrl.trim(),path:'',gitRef:githubRef.trim()||null});
      scan=result;chosen=new Set();editingSource=null;replaceConflicts=false;discoverySearch='';githubForm=false;note=`Found ${plural(result.skills.length,'skill')} at ${shaShort(result.commit)}. Select packages to import.`;
    } catch(cause){error=message(cause);note='';}
    finally{busy=false;draw();}
  }

  async function editSource(id:string):Promise<void> {
    const source=sources.find(s=>s.id===id); if(!source)return;
    githubUrl=`https://github.com/${source.repo}${source.path?`/tree/${source.gitRef??'main'}/${source.path}`:''}`;
    githubRef=source.gitRef??'';busy=true;error='';note='Scanning saved source…';draw();
    try {
      scan=await options.call<SourcePreview>('skills_source_preview',{repo:source.repo,path:source.path,gitRef:source.gitRef});
      chosen=new Set(source.selectedPaths);editingSource=id;replaceConflicts=false;discoverySearch='';note='Save tracked selection to change automatic update scope without uninstalling packages.';
    } catch(cause){error=message(cause);}
    finally{busy=false;draw();}
  }

  function handleProgress(value:any):void {
    if (!operation || operation.state !== 'running' || value.operationId !== operation.id) return;
    const item=operation.packages.get(value.path);
    if(!item || value.total!==operation.total || typeof value.completed!=='number' || value.completed < operation.completed || value.completed>operation.total) return;
    if(isTerminal(item.phase)) return;
    const allowed=['fetching','validating','installing','installed','updated','unchanged','skipped','failed','cancelled'];
    if(!allowed.includes(value.phase))return;
    item.phase=value.phase as Phase;
    item.detail=typeof value.detail==='string'?value.detail:null;
    operation.completed=Math.max(operation.completed,value.completed);
    renderOperation();
  }

  function selectSkillPath(path:string, checked:boolean):void {
    if(checked)chosen.add(path);else chosen.delete(path);
    draw();
  }

  const click=(event:Event)=>{
    const button=(event.target as HTMLElement).closest<HTMLElement>('[data-customize]');
    if(!button || !host.contains(button))return;
    const action=button.dataset.customize, id=button.dataset.id??'', path=button.dataset.path??'';
    switch(action){
      case 'close': options.close();break;
      case 'toggle-add':addOpen=!addOpen;draw();break;
      case 'add-zip':void importZip();break;
      case 'add-github':addOpen=false;githubForm=true;scan=null;editingSource=null;replaceConflicts=false;detail=null;draw();break;
      case 'close-github-form':githubForm=false;draw();break;
      case 'tab-installed':tab='installed';pendingRemove=null;draw();break;
      case 'tab-sources':tab='sources';pendingRemove=null;draw();break;
      case 'refresh':error='';void refresh();break;
      case 'inspect-installed':detail={kind:'installed',id,file:'SKILL.md'};markdownMode='rendered';draw();break;
      case 'inspect-remote':detail={kind:'remote',path,file:'SKILL.md'};markdownMode='rendered';draw();break;
      case 'close-detail':detail=null;previewSerial++;draw();break;
      case 'preview-file':if(detail){detail={...detail,file:button.dataset.file??'SKILL.md'};draw();}break;
      case 'preview-mode':markdownMode=button.dataset.mode==='raw'?'raw':'rendered';draw();break;
      case 'hide-discovery':scan=null;chosen.clear();editingSource=null;draw();break;
      case 'select-all':if(scan)for(const entry of scan.skills)if([entry.name,entry.path,entry.description].some(s=>s.toLowerCase().includes(discoverySearch.toLowerCase())))chosen.add(entry.path);draw();break;
      case 'select-none':chosen.clear();draw();break;
      case 'import-selected':if(scan&&chosen.size){const snap={repo:scan.repo,path:scan.path,gitRef:scan.gitRef,commit:scan.commit,paths:[...chosen],allowReplace:replaceConflicts};void importSelected(snap);}break;
      case 'save-source-selection':
        if(editingSource)void (async()=>{try{
          const source=await options.call<SavedSource>('skills_source_set_selection',{id:editingSource,selectedPaths:[...chosen]});
          note=`Tracking updated for ${source.repo}; installed packages were not deleted.`;
          error='';await refresh();
        }catch(cause){error=message(cause);draw();}})();break;
      case 'retry-failed':if(operation?.snapshot){const snap={...operation.snapshot,paths:[...operation.packages.values()].filter(entry=>entry.phase==='failed'||entry.phase==='skipped').map(entry=>entry.path)};void importSelected(snap);}break;
      case 'view-installed':tab='installed';search='';draw();break;
      case 'sync-skill':void synchronize('skill',id);break;
      case 'sync-source':void synchronize('source',id);break;
      case 'sync-all':void synchronize('all');break;
      case 'edit-source':void editSource(id);break;
      case 'remove-source':
        if(pendingRemove!==id){pendingRemove=id;draw();break;}
        pendingRemove=null;void (async()=>{try{sources=await options.call<SavedSource[]>('skills_source_remove',{id});note='Source configuration removed. Installed packages were kept.';error='';}catch(cause){error=message(cause);}draw();})();break;
    }
  };
  const input=(event:Event)=>{
    const target=event.target as HTMLInputElement;
    if(target.name==='installed-search'){search=target.value;draw();}
    if(target.name==='discovery-search'){discoverySearch=target.value;draw();}
    if(target.name==='replace-conflicts')replaceConflicts=target.checked;
    if(target.name==='github-url')githubUrl=target.value;
    if(target.name==='github-ref')githubRef=target.value;
  };
  const change=(event:Event)=>{
    const target=event.target as HTMLInputElement;
    if(target.dataset.selectSkill!==undefined){selectSkillPath(target.dataset.selectSkill,target.checked);return;}
    if(target.name==='replace-conflicts'){replaceConflicts=target.checked;draw();return;}
    if(target.dataset.customizeToggle==='startup-sync'){
      const current=target.checked;
      void (async()=>{try{policy=await options.call<Policy>('skills_sync_set_global',{enabled:current});note=current?'Startup sync enabled for previously imported skills.':'Startup sync disabled.';error='';}
        catch(cause){error=message(cause);}draw();})();
    }
  };
  const submit=(event:Event)=>{if((event.target as HTMLElement).id==='customize-github-form'){event.preventDefault();void scanGithub();}};
  host.addEventListener('click',click);
  host.addEventListener('input',input);
  host.addEventListener('change',change);
  host.addEventListener('submit',submit);
  void options.subscribe(handleProgress).then(cleanup=>{if(disposed)cleanup();else unsub=cleanup;}).catch(()=>{});
  draw();
  void refresh();
  return {dispose(){
    disposed=true;previewSerial++;unsub?.();
    host.removeEventListener('click',click);host.removeEventListener('input',input);
    host.removeEventListener('change',change);host.removeEventListener('submit',submit);
    host.replaceChildren();
  }};
}
