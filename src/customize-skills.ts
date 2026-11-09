/* Standalone Customize -> Skills workspace. Owns no package storage or harness writes:
 * the native skills_* commands share one library with Settings -> Skills & rules. */
import './customize-skills.css';
import { escape, icon, markdownHtml } from './dom';
import type { SkillItem, CatalogSkill, SkillMatrix } from './skills-library';

type SavedSource = {
  id: string; repo: string; path: string; gitRef: string | null;
  commit: string; selectedPaths: string[]; savedAt: number;
  lastSyncedAt?: number|null; lastSyncStatus?: string|null; lastSyncCommit?: string|null;
};
type SourcePreview = {
  repo: string; path: string; gitRef: string | null; commit: string; skills: CatalogSkill[];
};
type SourceImport = { source: SavedSource; imported: string[]; skipped: {path: string; name: string; reason: string}[];
  results?: {path:string;name:string;phase:Phase;completed:number;total:number;detail:string|null}[] };
type SyncReport = {
  checkedAt: number; installed: string[]; updated: string[]; skipped: string[];
  events: { id: string; status: string; oldCommit: string | null; newCommit: string | null; impactedHarnesses: string[]; detail: string }[];
};
type Policy = { autoOnStart: boolean };
type PreviewFile = { path: string; text: string | null; size: number; binary: boolean };
type Phase = 'queued' | 'fetching' | 'validating' | 'installing' | 'installed' | 'replaced' | 'updated' | 'unchanged' | 'skipped' | 'failed' | 'cancelled';
type PackageProgress = { path: string; name: string; phase: Phase; detail: string | null };
type ImportSnapshot = { repo: string; path: string; gitRef: string | null; commit: string; paths: string[]; allowReplace?: boolean };
type Operation = {
  id: string; kind: 'github' | 'zip' | 'sync'; label: string; source: string; revision: string | null;
  state: 'running' | 'success' | 'partial' | 'failed';
  total: number; completed: number; packages: Map<string, PackageProgress>;
  snapshot?: ImportSnapshot; detail: string; progressSeen: boolean;
};
type Detail = { kind: 'installed'; id: string; file: string } | { kind: 'remote'; path: string; file: string };
export type CustomizeSkillsHost = {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  subscribe(listener: (event: any) => void): Promise<() => void>;
  chooseZip(): Promise<string | null>;
  /** Create a projectless chat using an enabled harness; prefill /skillname without sending. */
  trySkill(id: string, harness: string): Promise<void>;
  close(): void;
};
const message = (error: unknown): string => error instanceof Error ? error.message : String(error);
const initialPolicy: Policy = { autoOnStart: false };
const plural = (amount: number, word: string) => `${amount} ${word}${amount === 1 ? '' : 's'}`;
const shaShort = (sha: string | null | undefined) => sha ? sha.slice(0, 12) : 'Not pinned';
const sourceLocation = (repo:string,path:string) => path ? `${repo}/${path}` : repo;
const HARNESS_LABELS: Record<string,string> = { claude:'Claude Code', codex:'Codex', opencode:'OpenCode', 'github-copilot':'GitHub Copilot', glm:'GLM', mimo:'MiMo', deepseek:'DeepSeek', kimi:'Kimi', cline:'Cline', kilo:'Kilo', pi:'Pi' };
const harnessLabel = (harness: string) => HARNESS_LABELS[harness] ?? harness;
const isTerminal = (phase: Phase) => ['installed', 'replaced', 'updated', 'unchanged', 'skipped', 'failed', 'cancelled'].includes(phase);
const stateLabel = (phase: Phase) => ({ queued:'Queued', fetching:'Downloading', validating:'Validating', installing:'Installing', installed:'Installed', replaced:'Replaced', updated:'Updated', unchanged:'Unchanged', skipped:'Skipped', failed:'Failed', cancelled:'Cancelled' } as Record<Phase,string>)[phase];

/** SVGs belong to imported packages: render only strictly validated, inert image markup. */
function skillIconUrl(source: string | undefined): string | null {
  if (!source || source.length > 65_536) return null;
  const doc = new DOMParser().parseFromString(source, 'image/svg+xml');
  if (doc.querySelector('parsererror') || doc.documentElement.localName !== 'svg') return null;
  const tags = new Set(['svg','g','path','rect','circle','ellipse','line','polyline','polygon','title','desc']);
  const attrs = new Set(['xmlns','viewBox','width','height','version','fill','fill-rule','stroke','stroke-width','stroke-linecap','stroke-linejoin','stroke-miterlimit','opacity','d','x','y','x1','y1','x2','y2','cx','cy','r','rx','ry','points','transform']);
  let nodes = 0, safe = true;
  const inspect = (element: Element): void => {
    if (++nodes > 256 || !tags.has(element.localName)) safe = false;
    for (const attr of Array.from(element.attributes)) {
      if (attr.name === 'xmlns') {
        if (attr.value !== 'http://www.w3.org/2000/svg') safe = false;
        continue;
      }
      if (!attrs.has(attr.name) || /url\s*\(|javascript\s*:|https?\s*:|data\s*:|@import/i.test(attr.value)) safe = false;
    }
    for (const child of Array.from(element.children)) inspect(child);
  };
  inspect(doc.documentElement);
  return safe ? 'data:image/svg+xml,' + encodeURIComponent(new XMLSerializer().serializeToString(doc.documentElement)) : null;
}

export function mountCustomizeSkills(host: HTMLElement, options: CustomizeSkillsHost): {dispose(): void;refresh():Promise<void>;closeDetail():void} {
  let installed: SkillItem[] = [];
  let installedIcons: Record<string,string> = {};
  let sources: SavedSource[] = [];
  let policy = initialPolicy;
  let matrix: SkillMatrix = {project:null,harnesses:[]};
  let syncReport: SyncReport = {checkedAt:0,installed:[],updated:[],skipped:[],events:[]};
  let changingRefSource: string | null = null;
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
  let pendingZipReplacement: string | null = null;
  let unsub: (() => void) | undefined;
  let firstLoad = true, refreshing = false, loadFailed = false, refreshSerial = 0;
  let toast = '', toastTimer: ReturnType<typeof setTimeout> | undefined;
  let pendingActivation: {id:string;harness:string}|null = null;
  let pendingReplacement: {id:string;harness:string;acknowledge:boolean}|null = null;
  const collapsedFiles = new Set<string>();
  let pendingUninstall: string|null = null, tryHarness = '';
  const esc = (s: unknown) => escape(s);

  function success(text: string): void {
    toast = text;
    if (toastTimer !== undefined) clearTimeout(toastTimer);
    draw();
    toastTimer = setTimeout(() => { if (!disposed && toast === text) { toast = ''; draw(); } }, 5500);
  }

  function currentUserActivation(item: SkillItem, harness: string): boolean {
    const activation = item.activations.find(a => a.harness === harness && a.scope === 'user');
    const cell = matrix.harnesses.find(row=>row.harness===harness)?.cells.find(c=>c.id===item.id&&c.scope==='user');
    return !!activation && activation.sha256 === item.sha256
      && (!item.findings.length || item.acknowledged === item.sha256)
      && (cell?.status === 'active' || cell?.status === 'prompt');
  }

  async function setHarnessEnabled(id: string, harness: string, enabled: boolean, acknowledge = false, replaceExisting = false): Promise<void> {
    if (busy || operation?.state === 'running') return;
    const item = installed.find(entry => entry.id === id);
    if (!item || !matrix.harnesses.some(row=>row.harness===harness)) return;
    if (enabled && item.findings.length && item.acknowledged !== item.sha256 && !acknowledge) {
      pendingActivation={id,harness}; draw(); return;
    }
    pendingActivation=null; pendingReplacement=null; busy=true; error=''; draw();
    try {
      const change = enabled
        ? await options.call<{kept:string[]}>('skills_activate', {id,harness,scope:'user',acknowledge,replaceExisting})
        : await options.call<{kept:string[]}>('skills_deactivate', {id,harness,scope:'user'});
      await refresh();
      const kept = change.kept?.length
        ? enabled ? ` ${change.kept.join(' ')}.`
          : ` ${plural(change.kept.length,'edited file')} preserved on disk.`
        : '';
      success(`${item.name} ${enabled?'enabled':'disabled'} for ${harnessLabel(harness)}.${kept}`);
    } catch (cause) {
      const detail=message(cause);
      if(enabled && !replaceExisting && detail.includes('already exists and PandaCode did not create it.')) {
        pendingReplacement={id,harness,acknowledge};
        error='';
      } else error=detail;
    }
    finally {
      busy=false; draw();
      if (detail?.kind==='installed' && detail.id===id) {
        host.querySelector<HTMLInputElement>(`[data-customize-harness="${CSS.escape(harness)}"][data-id="${CSS.escape(id)}"]`)?.focus({preventScroll:true});
      }
    }
  }

  async function uninstall(id: string): Promise<void> {
    if (busy || operation?.state === 'running') return;
    const item=installed.find(entry=>entry.id===id);
    if (!item || item.activations.length || pendingUninstall!==id) return;
    busy=true; error=''; draw();
    try {
      await options.call<SkillItem[]>('skills_remove',{id});
      pendingUninstall=null; pendingActivation=null; detail=null; previewSerial++;
      await refresh();
      success(`${item.name} uninstalled from the PandaCode library.`);
    } catch (cause) { error=message(cause); }
    finally { busy=false; draw(); }
  }

  async function tryInstalledSkill(id: string): Promise<void> {
    if (busy || operation?.state === 'running') return;
    const item=installed.find(entry=>entry.id===id);
    const available=item ? matrix.harnesses.filter(row=>row.installed && currentUserActivation(item,row.harness)) : [];
    const harness=available.find(row=>row.harness===tryHarness)?.harness ?? available[0]?.harness;
    if (!item || !harness) return;
    busy=true; error=''; draw();
    try { await options.trySkill(id,harness); }
    catch(cause) { error=message(cause); }
    finally { busy=false; if(!disposed)draw(); }
  }

  function operationHtml(): string {
    if (!operation) return '';
    const op = operation;
    const values = [...op.packages.values()];
    const count = (phase: Phase) => values.filter(entry => entry.phase === phase).length;
    const successes = count('installed') + count('replaced') + count('updated');
    const failures = count('failed'), skipped = count('skipped'), unchanged = count('unchanged');
    const title = op.state === 'running' ? op.label
      : op.state === 'success' ? 'Completed successfully'
      : op.state === 'partial' ? 'Completed with errors' : 'Operation failed';
    const processing = values.find(entry => !isTerminal(entry.phase));
    const canConfirmZip = op.kind === 'zip' && op.state === 'failed' && !!pendingZipReplacement;
    const retryable = values.filter(entry => entry.phase === 'failed');
    const canRetry = op.state !== 'running' && retryable.length > 0
      && (op.kind === 'sync' || (op.kind === 'github' && !!op.snapshot));
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
        ${canConfirmZip ? '<button type="button" class="customize-primary" data-customize="confirm-zip-replace">Confirm ZIP replacement</button>' : ''}
        ${canRetry ? `<button type="button" class="secondary-button" data-customize="retry-failed">Retry failed (${retryable.length})</button>` : ''}
      </div>
    </section>`;
  }

  function renderOperation(): void {
    const slot = host.querySelector<HTMLElement>('#customize-operation-slot');
    if (slot) slot.innerHTML = operationHtml();
  }

  /** Build an accessible directory tree without altering the original package paths. */
  function filesTreeHtml(paths: string[], selected: string): string {
    type Node = { name: string; path: string; folders: Map<string,Node>; file: boolean };
    const root: Node = {name:'',path:'',folders:new Map(),file:false};
    for(const relative of paths) {
      const segments=relative.split('/').filter(Boolean);
      let branch=root;
      for(const [index,name] of segments.entries()) {
        const path=segments.slice(0,index+1).join('/');
        let child=branch.folders.get(name);
        if(!child) {child={name,path,folders:new Map(),file:false};branch.folders.set(name,child);}
        if(index===segments.length-1) child.file=true;
        branch=child;
      }
    }
    const render = (nodes: Map<string,Node>, nested=false): string => {
      const ordered=[...nodes.values()].sort((a,b)=>Number(a.file)-Number(b.file)||a.name.localeCompare(b.name));
      return `<ul role="${nested?'group':'tree'}" ${nested?'':'aria-label="Package file tree"'}>${ordered.map(node=>{
        if(node.file) return `<li role="treeitem" aria-selected="${selected===node.path}">
          <button type="button" data-customize="preview-file" data-file="${esc(node.path)}"
            class="customize-file-entry ${selected===node.path?'selected':''}" title="${esc(node.path)}">
            ${icon('file')}<span>${esc(node.name)}</span>
          </button>
        </li>`;
        const open=!collapsedFiles.has(node.path);
        return `<li role="treeitem" aria-expanded="${open}">
          <button type="button" class="customize-folder-entry" data-customize="toggle-file-folder" data-path="${esc(node.path)}" aria-label="${open?'Collapse':'Expand'} ${esc(node.name)} folder">
            <span class="customize-tree-caret" aria-hidden="true">${open?'⌄':'›'}</span>${icon('folder')}<span>${esc(node.name)}</span>
          </button>
          ${open?render(node.folders,true):''}
        </li>`;
      }).join('')}</ul>`;
    };
    return render(root.folders);
  }

  function githubSourceLink(source: {kind:string;location:string;gitRef?:string|null;commit?:string|null}): string | null {
    if(source.kind!=='github')return null;
    const parts=source.location.split('/');
    if(parts.length<2||parts.some(part=>!/^[a-zA-Z0-9._-]+$/.test(part)))return null;
    const root=`https://github.com/${encodeURIComponent(parts[0])}/${encodeURIComponent(parts[1])}`;
    const revision=source.commit||source.gitRef;
    return revision
      ? `${root}/tree/${encodeURIComponent(revision)}${parts.length>2?'/'+parts.slice(2).map(encodeURIComponent).join('/'):''}`
      : root;
  }

  function dismissDetail(): void {
    const old=detail;
    detail=null;pendingActivation=null;pendingReplacement=null;pendingUninstall=null;collapsedFiles.clear();previewSerial++;
    draw();
    const selector=old?.kind==='installed'
      ? `[data-customize="inspect-installed"][data-id="${CSS.escape(old.id)}"]`
      : old ? `[data-customize="inspect-remote"][data-path="${CSS.escape(old.path)}"]` : '';
    if(selector)host.querySelector<HTMLButtonElement>(selector)?.focus({preventScroll:true});
  }

  function installedHtml(): string {

    const visible = installed.filter(item => item.kind==='skill' && [item.name, item.id, item.description, item.source.location]
      .some(value => value.toLowerCase().includes(search.trim().toLowerCase())));
    const card = (item: SkillItem) => {
      const type = item.source.kind === 'github' ? 'GitHub' : item.source.kind === 'zip' ? 'ZIP' : 'Local';
      const active = item.activations?.length ?? 0;
      const statuses = matrix.harnesses.flatMap(row=>row.cells.filter(cell=>cell.id===item.id && cell.status!=='off')
        .map(cell=>({label:`${harnessLabel(row.harness)}: ${cell.status}`, live:cell.status==='active'||cell.status==='prompt'})));
      const live=statuses.some(status=>status.live);
      const missingPeers = (item.requiredSkills??[]).filter(id=>!installed.some(entry=>entry.id===id));
      const last = syncReport.events.find(event=>event.id===item.id);
      const update = !last ? 'Update status not checked'
        : last.status==='updated' ? 'Updated on last sync'
        : last.status==='unchanged' ? 'Up to date at last check'
        : last.status==='error' ? 'Update failed' : 'Update skipped';
       const iconUrl = skillIconUrl(installedIcons[item.id]);
      return `<li class="customize-skill-row">
        <button type="button" class="customize-skill-main" data-customize="inspect-installed" data-id="${esc(item.id)}" aria-label="Inspect ${esc(item.name)}">
          <span class="customize-skill-icon">${iconUrl ? `<img src="${esc(iconUrl)}" alt="" loading="lazy">` : icon('skill')}</span>
          <span class="customize-skill-description"><span class="customize-skill-title"><strong>${esc(item.name)}</strong>${item.findings?.length ? `<small class="customize-warning">${plural(item.findings.length, 'safety finding')}</small>` : ''}</span><span>${esc(item.description || item.id)}</span><small>${esc(item.id)} · ${esc(type)}${item.source.commit ? ` · ${esc(shaShort(item.source.commit))}` : ''}${item.source.gitRef ? ` · ref ${esc(item.source.gitRef)}` : ''}</small>
          ${item.source.kind==='github' ? `<small>${esc(update)}</small>` : ''}
          ${missingPeers.length ? `<small class="customize-warning">Missing dependencies: ${esc(missingPeers.join(', '))}</small>` : ''}
          ${statuses.length ? `<small class="customize-harness-statuses">${statuses.map(status=>`<span class="${status.live?'customize-active-text':''}">${esc(status.label)}</span>`).join('<span aria-hidden="true"> · </span>')}</small>` : ''}</span>
        </button>
        <span class="customize-skill-trailing">
          <small class="customize-skill-activity ${active && live ? 'customize-active-text' : ''}">${active ? plural(active,'activation') : 'Not enabled'}</small>
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
        ${source.lastSyncedAt
          ? `<small>Last sync: ${esc(source.lastSyncStatus??'checked')} · ${esc(new Date(source.lastSyncedAt).toLocaleString())}${source.lastSyncCommit ? ` · ${esc(shaShort(source.lastSyncCommit))}` : ''}</small>`
          : '<small>Not synchronized yet</small>'}
      </div>
      <div class="customize-source-actions">
        <button type="button" class="secondary-button" data-customize="edit-source" data-id="${esc(source.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Edit selection</button>
        <button type="button" class="secondary-button" data-customize="change-source-ref" data-id="${esc(source.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Change ref</button>
        <button type="button" class="secondary-button" data-customize="sync-source" data-id="${esc(source.id)}" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Sync source</button>
        <button type="button" class="secondary-button" data-customize="remove-source" data-id="${esc(source.id)}" ${operation?.state === 'running' && operation.kind === 'github' && operation.snapshot?.repo === source.repo && operation.snapshot?.path === source.path && operation.snapshot?.gitRef === source.gitRef ? 'disabled title="Finish importing this source before removing it"' : ''}>${pendingRemove === source.id ? 'Confirm remove' : 'Remove source'}</button>
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
        const conflict = installedItem && (installedItem.source.location !== `${sourceLocation(scan!.repo,entry.path)}` || installedItem.source.gitRef !== scan!.gitRef);
        return `<li><label class="customize-discovery-choice"><input type="checkbox" data-select-skill="${esc(entry.path)}" ${chosen.has(entry.path) ? 'checked' : ''} ${busy ? 'disabled' : ''}><span><strong>${esc(entry.name)}</strong><small>${esc(entry.description)}</small><code>${esc(entry.path || '(repository root)')}</code>${conflict ? `<small class="customize-warning">Installed from another source; import requires a deliberate replacement.</small>` : ''}</span></label>
          <button type="button" class="secondary-button" data-customize="inspect-remote" data-path="${esc(entry.path)}">Preview</button></li>`;
      }).join('')}</ul></div>
      <div class="customize-discovery-footer"><span>${plural(chosen.size, 'skill')} selected</span>
        <div class="customize-discovery-actions">
          ${scan.skills.some(item => chosen.has(item.path) && installed.some(current => current.id === item.id &&
            (current.source.location !== `${sourceLocation(scan!.repo,item.path)}` || current.source.gitRef !== scan!.gitRef))) ?
            `<label class="customize-confirm-replace"><input type="checkbox" name="replace-conflicts" ${replaceConflicts ? 'checked' : ''}>I approve replacing conflicting installed skills</label>` : ''}
          ${editingSource ? `<button type="button" class="secondary-button" data-customize="save-source-selection" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Save tracked selection</button>` : ''}
          <button type="button" class="customize-primary" data-customize="import-selected" ${!chosen.size || busy || operation?.state === 'running' || (scan.skills.some(item => chosen.has(item.path) && installed.some(current => current.id === item.id && (current.source.location !== `${sourceLocation(scan!.repo,item.path)}` || current.source.gitRef !== scan!.gitRef))) && !replaceConflicts) ? 'disabled' : ''}>Import selected (${chosen.size})</button>
        </div>
      </div>
    </section>`;
  }


  function lifecycleHtml(item: SkillItem): string {
    const selectable = matrix.harnesses.filter(row => row.installed || item.activations.some(a => a.harness === row.harness));
    const projectCount = item.activations.filter(a=>a.scope==='project').length;
    const available = selectable.filter(row=>row.installed && currentUserActivation(item,row.harness));
    const picked = available.find(row=>row.harness===tryHarness)?.harness ?? available[0]?.harness ?? '';
    const harnesses = selectable.map(row => {
      const enabled = item.activations.some(a=>a.harness===row.harness && a.scope==='user');
      const state = row.cells.find(c=>c.id===item.id && c.scope==='user')?.status ?? 'off';
      const status = !row.installed ? 'Harness unavailable' : !enabled ? 'Not enabled' :
        state === 'active' || state === 'prompt' ? 'Enabled' :
        state === 'outdated' ? 'Update requires reactivation' : state === 'drifted' ? 'Edited on disk' : 'Missing harness files';
      return `<div class="customize-harness-row">
        <label><span><strong>${esc(harnessLabel(row.harness))}</strong><small>${esc(status)}</small></span>
          <input type="checkbox" data-customize-harness="${esc(row.harness)}" data-id="${esc(item.id)}"
          aria-label="Enable ${esc(item.name)} for ${esc(harnessLabel(row.harness))}" ${enabled?'checked':''}
          ${busy || operation?.state==='running' || (!row.installed && !enabled) ? 'disabled':''}></label>
        ${enabled && state !== 'active' && state !== 'prompt' && row.installed ?
          `<button type="button" class="secondary-button" data-customize="reactivate-skill" data-id="${esc(item.id)}" data-harness="${esc(row.harness)}" ${busy?'disabled':''}>Reactivate</button>` : ''}
      </div>`;
    }).join('');
    const consent = pendingActivation?.id === item.id ? `<div class="customize-skill-consent" role="alert">
      <p class="customize-warning">Review ${plural(item.findings.length,'safety finding')} before enabling ${esc(item.name)} for ${esc(harnessLabel(pendingActivation.harness))}.</p>
      <ul>${item.findings.map(f=>`<li>${esc(f.rule)} · ${esc(f.file)}:${f.line}</li>`).join('')}</ul>
      <div class="customize-inline-actions"><button type="button" class="secondary-button" data-customize="cancel-activation">Cancel</button>
        <button type="button" class="customize-primary" data-customize="confirm-activation" data-id="${esc(item.id)}" data-harness="${esc(pendingActivation.harness)}">I reviewed the findings. Enable anyway</button></div>
      </div>` : '';
    const replacement = pendingReplacement?.id === item.id ? `<div class="customize-replace-confirm" role="alert">
      <strong>Replace the existing ${esc(harnessLabel(pendingReplacement.harness))} skill?</strong>
      <p>An unmanaged skill folder already exists at this harness location. PandaCode will move it to a backup outside the harness skill directory before installing ${esc(item.name)}. The original files will not be deleted.</p>
      <div class="customize-inline-actions">
        <button type="button" class="secondary-button" data-customize="cancel-replacement">Cancel</button>
        <button type="button" class="customize-primary" data-customize="confirm-replacement" data-id="${esc(item.id)}" data-harness="${esc(pendingReplacement.harness)}">Back up and replace</button>
      </div>
    </div>` : '';
    const confirm = pendingUninstall === item.id ? `<div class="customize-uninstall-confirm" role="alert">
      <strong>Uninstall ${esc(item.name)}?</strong>
      <p>${item.activations.length ?
        `Disable all ${plural(item.activations.length,'activation')} (including project scopes in Settings → Skills & rules) before uninstalling.` :
        'This removes the skill package from the central library. Saved GitHub sources remain configured, but removed skills are not automatically reinstalled.'}</p>
      <div class="customize-inline-actions">
        <button type="button" class="secondary-button" data-customize="cancel-uninstall">Cancel uninstall</button>
        <button type="button" class="customize-danger" data-customize="confirm-uninstall" data-id="${esc(item.id)}" ${item.activations.length || busy ? 'disabled':''}>Confirm uninstall</button>
      </div>
    </div>` : '';
    return `<div class="customize-lifecycle" aria-label="Manage installed skill">
      <div class="customize-section-head"><div><h3>Activation by harness</h3><p>Enable at user scope for new chats and tasks. Project-specific activations remain managed in Settings → Skills & rules.</p></div></div>
      ${harnesses ? `<div class="customize-harness-grid">${harnesses}</div>` : '<p class="customize-help">No supported harness is installed. Install a harness to enable this skill.</p>'}
      ${projectCount ? `<p class="customize-help">${plural(projectCount,'project activation')} configured separately in Settings.</p>` : ''}
      ${consent}
      ${replacement}
      <div class="customize-skill-footer">
        <button type="button" class="customize-danger" data-customize="uninstall-skill" data-id="${esc(item.id)}" ${busy || operation?.state==='running'?'disabled':''}>Uninstall</button>
        <div class="customize-try-controls">
          ${available.length > 1 ? `<label>Test with <select name="try-harness" aria-label="Harness for test chat">
            ${available.map(row=>`<option value="${esc(row.harness)}" ${row.harness===picked?'selected':''}>${esc(harnessLabel(row.harness))}</option>`).join('')}
          </select></label>` : (picked ? `<span>${esc(harnessLabel(picked))}</span>` : '')}
          <button type="button" class="customize-primary" data-customize="try-skill" data-id="${esc(item.id)}" ${!picked || busy || operation?.state==='running'?'disabled':''}>${icon('chat')}<span>Try in chat</span></button>
        </div>
      </div>
      ${!picked ? '<p class="customize-help">Enable this skill for an installed harness at user scope to test it in a new chat.</p>' : '<p class="customize-help">Creates a new chat with /skillname prefilled. It does not send or run the skill automatically.</p>'}
      ${confirm}
    </div>`;
  }

  function detailHtml(): string {
    if (!detail) return '';
    const item = detail.kind === 'installed' ? installed.find(item => item.id === (detail as {id:string}).id) : scan?.skills.find(skill => skill.path === (detail as any).path);
    if (!item) return '';
    const files = detail.kind === 'installed' ? (item as SkillItem).files : ((item as CatalogSkill).filePaths?.length ? (item as CatalogSkill).filePaths! : ['SKILL.md']);
    const selectedFile = detail.file;
    const source = detail.kind === 'installed' ? (item as SkillItem).source : {kind:'github',location:scan!.repo, gitRef:scan!.gitRef,commit:scan!.commit};
    const sourceUrl = githubSourceLink(source);
    return `<dialog class="customize-detail-dialog" aria-label="Skill details">
      <section class="customize-detail ds-card" aria-label="Read-only skill preview">
      <div class="customize-detail-heading">
        <div><p class="customize-eyebrow">${detail.kind === 'remote' ? 'GITHUB PREVIEW · NOT INSTALLED' : 'INSTALLED PACKAGE'}</p>
        <h3>${esc(item.name)}</h3><p>${esc(item.description)}</p><small>${esc(source.location)} · ${esc(shaShort(source.commit))}</small>
          ${sourceUrl ? `<a class="customize-source-link" href="${esc(sourceUrl)}" target="_blank" rel="noopener noreferrer">View source</a>` : ''}</div>
        <button type="button" class="secondary-button" data-customize="close-detail">Close preview</button>
      </div>
      ${detail.kind === 'installed' ? lifecycleHtml(item as SkillItem) : ''}
      <div class="customize-preview-shell">
        <nav class="customize-file-tree" aria-label="Package files">
          <strong>Files</strong>
          ${filesTreeHtml(files, selectedFile)}
        </nav>
        <div class="customize-preview-main">
          <div class="customize-preview-toolbar"><span>${esc(detail.file)}</span><div class="customize-mode-switch">
            <button type="button" data-customize="preview-mode" data-mode="rendered" aria-pressed="${markdownMode === 'rendered'}">Rendered</button>
            <button type="button" data-customize="preview-mode" data-mode="raw" aria-pressed="${markdownMode === 'raw'}">Raw</button>
          </div></div>
          <div id="customize-preview-content" class="customize-preview-body" role="region" aria-label="Skill file content">Loading file…</div>
        </div>
      </div>
      ${toast ? `<div class="customize-dialog-toast" role="status">${esc(toast)}</div>` : ''}
      ${error ? `<p class="customize-dialog-error" role="alert">${esc(error)}</p>` : ''}
      </section>
    </dialog>`;
  }

  function githubFormHtml(): string {
    if (!githubForm) return '';
    return `<form class="customize-github-form ds-card" id="customize-github-form">
      <div class="customize-section-head"><div><h3>Add from GitHub</h3><p>Scan every SKILL.md in the repository or the directory linked in a tree URL. Nothing is installed until you choose packages.</p></div><button type="button" class="secondary-button" data-customize="close-github-form">Cancel</button></div>
      <label>GitHub URL<input type="url" required name="github-url" spellcheck="false" autocomplete="off" placeholder="https://github.com/owner/repo" value="${esc(githubUrl)}" ${changingRefSource ? 'readonly' : ''}></label>
      <label>Branch or full commit SHA <span class="customize-optional">Optional</span><input type="text" name="github-ref" spellcheck="false" autocomplete="off" placeholder="Default branch" value="${esc(githubRef)}"></label>
      <div class="customize-form-actions"><button type="submit" class="customize-primary" ${busy || operation?.state === 'running' ? 'disabled' : ''}>Scan repository</button></div>
    </form>`;
  }

  function draw(): void {
    if (disposed) return;
    const active = document.activeElement as HTMLInputElement | null;
    const withinHost = !!active && host.contains(active);
    const activeName = withinHost ? active.name : '';
    const activeSkillPath = withinHost ? active.dataset.selectSkill : undefined;
    const activeAction = withinHost ? active.dataset.customize : undefined;
    const activeHarness = withinHost ? active.dataset.customizeHarness : undefined;
    const activeHarnessId = withinHost ? active.dataset.id : undefined;
    const activeActionId = withinHost ? active.dataset.id : undefined;
    const cursor = activeName && typeof active?.selectionStart === 'number' ? active.selectionStart : -1;
    host.innerHTML = `<div class="customize-page">
      <header class="customize-page-header">
        <div><p class="customize-eyebrow">PERSONALIZATION</p><h1>Customize</h1><p>Manage extensions for your workspace.</p></div>
        <button type="button" class="secondary-button" data-customize="close">${icon('back')}<span>Back to workspace</span></button>
      </header>
      <div class="customize-page-layout">
        <nav class="customize-category-nav" aria-label="Customization categories">
          <span class="customize-nav-heading">Categories</span><button type="button" aria-current="page">${icon('skill')}<span>Skills</span></button>
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
          <nav class="customize-tabs" aria-label="Skills pages"><button type="button" data-customize="tab-installed" aria-current="${tab === 'installed' ? 'page' : 'false'}">Installed <small>${installed.filter(item=>item.kind==='skill').length}</small></button>
            <button type="button" data-customize="tab-sources" aria-current="${tab === 'sources' ? 'page' : 'false'}">GitHub sources <small>${sources.length}</small></button></nav>
          ${toast ? `<div class="customize-toast" role="status">${icon('check')}<span>${esc(toast)}</span><button type="button" data-customize="dismiss-toast" aria-label="Dismiss notification">${icon('close')}</button></div>` : ''}
          ${error ? `<p class="customize-error" role="alert">${esc(error)}</p>` : ''}
          ${refreshing && !firstLoad ? '<p class="customize-refreshing" role="status">Refreshing skills…</p>' : ''}
          ${note ? `<p class="customize-notice" role="status">${esc(note)}</p>` : ''}
          ${githubFormHtml()}
          ${scan ? discoveryHtml() : ''}
          <div id="customize-operation-slot">${operationHtml()}</div>
          ${firstLoad || loadFailed ? `<section class="customize-loading" role="status" aria-label="${refreshing ? 'Loading installed skills' : 'Unable to load installed skills'}">
            ${refreshing ? '<span class="customize-spinner" aria-hidden="true"></span><strong>Loading installed skills…</strong><p>Reading your library and harness activations.</p><div class="customize-skeleton"></div><div class="customize-skeleton"></div>' : '<strong>Unable to load the skill library.</strong><p>Use Refresh to retry. Installed skills have not been changed.</p>'}
          </section>` : (tab === 'installed' ? installedHtml() : sourcesHtml())}
          ${detailHtml()}
        </section>
      </div>
    </div>`;
    const next = activeHarness !== undefined
      ? [...host.querySelectorAll<HTMLInputElement>('[data-customize-harness]')].find(input => input.dataset.customizeHarness === activeHarness && input.dataset.id === activeHarnessId)
      : activeSkillPath !== undefined
      ? [...host.querySelectorAll<HTMLInputElement>('[data-select-skill]')].find(input => input.dataset.selectSkill === activeSkillPath)
      : activeAction
        ? [...host.querySelectorAll<HTMLElement>('[data-customize]')].find(button => button.dataset.customize === activeAction && button.dataset.id === activeActionId)
        : activeName
          ? [...host.querySelectorAll<HTMLInputElement>('input[name]')].find(input => input.name === activeName)
          : undefined;
    if (next && !next.hasAttribute('disabled')) {
      next.focus({ preventScroll: true });
      if (cursor >= 0 && next instanceof HTMLInputElement && next.type !== 'checkbox')
        next.setSelectionRange(cursor, cursor);
    }
    if (detail) {
      const modal=host.querySelector<HTMLDialogElement>('.customize-detail-dialog');
      if(modal) {
        modal.addEventListener('cancel',event=>{event.preventDefault();dismissDetail();},{once:true});
        modal.showModal();
      }
      void loadPreview();
    }
  }

  async function refresh(): Promise<void> {
    const serial = ++refreshSerial;
    refreshing = true;
    draw();
    try {
      const [items, saved, current, statuses, recent, icons] = await Promise.all([
        options.call<SkillItem[]>('skills_list'), options.call<SavedSource[]>('skills_source_list'),
        options.call<Policy>('skills_sync_policy'),
        options.call<SkillMatrix>('skills_matrix').catch(()=>({project:null,harnesses:[]} as SkillMatrix)),
        options.call<SyncReport>('skills_sync_report').catch(()=>({checkedAt:0,installed:[],updated:[],skipped:[],events:[]} as SyncReport)),
        options.call<Record<string,string>>('skills_icons').catch(()=>({})),
      ]);
      if (disposed || serial !== refreshSerial) return;
      installed=items; installedIcons=icons; sources=saved; policy=current; matrix=statuses; syncReport=recent;
      loadFailed=false;
    } catch (cause) {
      if (disposed || serial !== refreshSerial) return;
      error=message(cause);
      if (firstLoad || loadFailed) loadFailed=true;
    } finally {
      if (!disposed && serial === refreshSerial) {
        firstLoad=false; refreshing=false; draw();
      }
    }
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
        // Skill instructions are untrusted data, never interactive message/UI code.
        const inert = value.text.replace(/^(\s*`{3,})\s*(ui|chart|svg|html|mermaid)\b/gim, '$1text');
        const purify = (await import('dompurify')).default;
        slot.innerHTML = purify.sanitize(await markdownHtml(inert), {
          FORBID_TAGS: ['pandacode-ui','pandacode-code','img','iframe','form','input','button','video','audio','object','embed','svg','math'],
          FORBID_ATTR: ['src','srcset','style','id','name','onload','onclick'],
          ALLOW_DATA_ATTR: false,
        });
        for (const anchor of slot.querySelectorAll<HTMLAnchorElement>('a[href]')) {
          if (!/^https:\/\//i.test(anchor.href)) anchor.removeAttribute('href');
          else {anchor.target='_blank';anchor.rel='noopener noreferrer';}
        }
      }
    } catch (cause) {
      if (!disposed && serial === previewSerial) { slot.textContent = message(cause); slot.setAttribute('role','alert'); }
    }
  }

  function beginOperation(kind: Operation['kind'], label: string, source: string, revision: string | null, entries: {path: string; name: string}[], snapshot?: ImportSnapshot): Operation {
    const op: Operation = {id:crypto.randomUUID(),kind,label,source,revision,state:'running',total:entries.length,completed:0,
      packages:new Map(entries.map(entry => [entry.path,{...entry,phase:'queued',detail:null}])),snapshot,detail:'',progressSeen:false};
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
      const result = await options.call<SourceImport>('skills_source_import_tracked',{request:{
        repo:snapshot.repo,path:snapshot.path,gitRef:snapshot.gitRef,commit:snapshot.commit,
        selectedPaths:[...snapshot.paths],operationId:op.id,allowReplace:!!snapshot.allowReplace,
      }});
      for (const resultItem of result.results??[]) {
        const item = op.packages.get(resultItem.path);
        if (item && isTerminal(resultItem.phase)) {
          item.phase=resultItem.phase; item.detail=resultItem.detail;
        }
      }
      for (const fail of result.skipped) {
        const item = op.packages.get(fail.path);
        if (item && !isTerminal(item.phase)) { item.phase='failed'; item.detail=fail.reason; }
      }
      // Compatibility with the original aggregate-only preview backend: use
      // only named confirmed successes, never invent intermediate stages.
      for (const entry of op.packages.values()) {
        if (!isTerminal(entry.phase) && result.imported.includes(entry.name)) entry.phase='installed';
        if (!isTerminal(entry.phase)) {entry.phase='failed';entry.detail='No terminal outcome was returned for this package.';}
      }
      op.completed=[...op.packages.values()].filter(item=>isTerminal(item.phase)).length;
      const failures=[...op.packages.values()].filter(item=>item.phase==='failed').length;
      op.state = failures || result.skipped.length ? (failures===op.total?'failed':'partial') : 'success';
      if (!op.progressSeen && op.total > 1) {
        // Final native results are authoritative for installed package contents,
        // but missing incremental events violate the progress acceptance contract.
        op.state = 'partial';
        op.detail = 'Native incremental progress events were not received. Package outcomes are final, but progress delivery failed; verify the installed skills.';
      }
      if (changingRefSource && !failures && !result.skipped.length) {
        if (changingRefSource!==result.source.id) {
          await options.call('skills_source_remove',{id:changingRefSource});
        }
        changingRefSource=null;
      }
      note = `Saved ${result.source.repo}: ${plural(result.imported.length,'skill')} installed or updated. Skills remain inactive until enabled for a harness.`;
    } catch (cause) {
      op.detail = message(cause);
      for (const item of op.packages.values()) if (!isTerminal(item.phase)) { item.phase='failed'; item.detail=op.detail; }
      op.completed = op.total; op.state = 'failed';
    } finally { busy = false; await refresh(); }
  }

  async function importZip(selectedPath?: string, confirmed = false): Promise<void> {
    if (busy || operation?.state === 'running') return;
    addOpen = false;
    let path: string | null;
    try { path = selectedPath ?? await options.chooseZip(); }
    catch (cause) { error = message(cause); draw(); return; }
    if (!path) { draw(); return; }
    pendingZipReplacement = null;
    const filename = path.split(/[\\/]/).pop() || 'Skill archive';
    const op = beginOperation('zip','Importing ZIP package',filename,null,[{path:filename,name:filename}]);
    busy = true;
    try {
      const result = await options.call<{item:SkillItem;replaced:boolean;previousSource:SkillItem['source']|null;impactedHarnesses:string[]}>('skills_import_zip_tracked',{path,operationId:op.id,allowReplace:confirmed});
      const item = op.packages.get(filename)!;
      item.phase = result.replaced ? 'replaced' : 'installed'; item.name = result.item.name;
      op.completed = 1; op.state = 'success';
      op.detail = result.replaced
        ? `Replaced previous central library source ${result.previousSource?.location ?? 'unknown'}. ${plural(result.impactedHarnesses.length,'harness')} may require reactivation. Native harness files were not overwritten.`
        : 'Installed into the central library. No harness was activated.';
      note = `${result.replaced ? 'Replaced' : 'Installed'} ${result.item.name}.`;
      detail = {kind:'installed',id:result.item.id,file:'SKILL.md'}; tab='installed';
    } catch (cause) {
      op.completed = 1; op.state = 'failed'; op.detail = message(cause);
      if(op.detail.startsWith('ZIP replacement requires confirmation:')) pendingZipReplacement = path;
      const item = op.packages.get(filename)!; item.phase = 'failed'; item.detail = op.detail;
    } finally { busy=false; await refresh(); }
  }

  async function synchronize(kind: 'skill'|'source'|'all'|'ids', id?: string, retryIds: string[] = []): Promise<void> {
    if (busy || operation?.state === 'running') return;
    let entries: {path:string;name:string}[] = [], label = '';
    if (kind === 'skill') { const item=installed.find(x=>x.id===id); if (!item) return; entries=[{path:item.id,name:item.name}];label='Synchronizing skill'; }
    if (kind === 'source') {
      const source=sources.find(x=>x.id===id); if (!source) return;
      entries=installed.filter(item=>item.source.kind==='github' && item.source.gitRef===source.gitRef &&
        source.selectedPaths.some(path=>item.source.location===sourceLocation(source.repo,path))).map(item=>({path:item.id,name:item.name}));
      label='Synchronizing saved source';
    }
    if (kind === 'all') {entries=installed.filter(x=>x.kind==='skill'&&x.source.kind==='github').map(x=>({path:x.id,name:x.name}));label='Synchronizing GitHub skills';}
    if (kind === 'ids') {entries=installed.filter(x=>retryIds.includes(x.id)&&x.source.kind==='github').map(x=>({path:x.id,name:x.name}));label='Retrying failed skill syncs';}
    if (!entries.length) {note='No GitHub skills are selected for synchronization.';draw();return;}
    const op=beginOperation('sync',label,kind==='source'?sources.find(x=>x.id===id)?.repo??'Saved source': 'Installed GitHub skills',null,entries);
    busy=true;
    try {
      const command='skills_sync_tracked';
      const report=await options.call<SyncReport>(command,{scope:kind,id:id??null,ids:kind==='ids'?retryIds:null,operationId:op.id});
      const returned = new Set<string>();
      for (const result of report.events) {
        const item = op.packages.get(result.id);
        if (!item) continue;
        returned.add(result.id);
        item.phase = result.status==='updated' ? 'updated' : result.status==='error' ? 'failed'
          : result.status==='unchanged' || result.detail.startsWith('Already at') ? 'unchanged' : 'skipped';
        const previous = result.oldCommit ? shaShort(result.oldCommit) : 'unavailable';
        const current = result.newCommit ? shaShort(result.newCommit) : 'unavailable';
        const harnesses = result.impactedHarnesses.length
          ? ` · Harnesses to review: ${result.impactedHarnesses.join(', ')}` : '';
        item.detail = `Old revision: ${previous} · New revision: ${current} · ${result.detail}${harnesses}`;
      }
      // The native terminal report is authoritative: an empty or incomplete
      // report must not turn a rejected/competing synchronization into success.
      for (const item of op.packages.values()) if (!returned.has(item.path)) {
        item.phase = 'failed';
        item.detail = report.skipped.join('; ') || 'No terminal synchronization result was returned for this skill.';
      }
      op.completed = op.total;
      const failed = [...op.packages.values()].filter(item=>item.phase==='failed').length;
      op.state = failed===op.total ? 'failed' : failed ? 'partial' : 'success';
      op.detail = `${plural(report.updated.length,'skill')} updated${failed ? ` · ${plural(failed,'skill')} failed` : ''}. Native harness activations stay unchanged until reactivated.`;
      if(!op.progressSeen&&op.total>1)op.detail+=' Incremental progress events were not received; results are from the final report.';
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
      const changing=changingRefSource?sources.find(source=>source.id===changingRefSource):null;
      const result=await options.call<SourcePreview>('skills_source_preview',{repo:githubUrl.trim(),path:changing?.path??'',gitRef:githubRef.trim()||null});
      scan=result;chosen=new Set(changing?.selectedPaths.filter(path=>result.skills.some(skill=>skill.path===path))??[]);
      editingSource=null;replaceConflicts=false;discoverySearch='';githubForm=false;
      note=`Found ${plural(result.skills.length,'skill')} at ${shaShort(result.commit)}. Select packages to import.`;
    } catch(cause){error=message(cause);note='';}
    finally{busy=false;draw();}
  }

  async function editSource(id:string):Promise<void> {
    const source=sources.find(s=>s.id===id); if(!source)return;
    changingRefSource=null;
    githubUrl=`https://github.com/${source.repo}${source.path?`/tree/${source.gitRef??'main'}/${source.path}`:''}`;
    githubRef=source.gitRef??'';busy=true;error='';note='Scanning saved source…';draw();
    try {
      scan=await options.call<SourcePreview>('skills_source_preview',{repo:source.repo,path:source.path,gitRef:source.gitRef});
      chosen=new Set(source.selectedPaths);editingSource=id;replaceConflicts=false;discoverySearch='';note='Save tracked selection to change automatic update scope without uninstalling packages.';
    } catch(cause){error=message(cause);}
    finally{busy=false;draw();}
  }

  function changeSourceRef(id:string):void {
    const source=sources.find(source=>source.id===id);
    if(!source)return;
    changingRefSource=id;editingSource=null;scan=null;detail=null;chosen.clear();
    githubUrl=`https://github.com/${source.repo}`;githubRef=source.gitRef??'';
    githubForm=true;
    note='Enter the new branch or full SHA, scan, and explicitly import the selected skills to change this source revision.';
    error='';draw();
  }

  function handleProgress(value:any):void {
    if (!operation || operation.state !== 'running' || value.operationId !== operation.id) return;
    const item=operation.packages.get(value.path);
    if(!item || value.total!==operation.total || typeof value.completed!=='number' || value.completed < operation.completed || value.completed>operation.total) return;
    if(isTerminal(item.phase)) return;
    const allowed=['fetching','validating','installing','installed','replaced','updated','unchanged','skipped','failed','cancelled'];
    if(!allowed.includes(value.phase))return;
    const rank:Record<Phase,number>={queued:0,fetching:1,validating:2,installing:3,installed:4,replaced:4,updated:4,unchanged:4,skipped:4,failed:4,cancelled:4};
    if(rank[value.phase as Phase]<rank[item.phase])return;
    operation.progressSeen=true;
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
    const backdrop=event.target;
    if(backdrop instanceof HTMLDialogElement && backdrop.classList.contains('customize-detail-dialog')) {
      const rect=backdrop.getBoundingClientRect();
      const point=event as MouseEvent;
      if(point.clientX<rect.left||point.clientX>rect.right||point.clientY<rect.top||point.clientY>rect.bottom) dismissDetail();
      return;
    }
    const button=(event.target as HTMLElement).closest<HTMLElement>('[data-customize]');
    if(!button || !host.contains(button))return;
    const action=button.dataset.customize, id=button.dataset.id??'', path=button.dataset.path??'';
    switch(action){
      case 'close': options.close();break;
      case 'toggle-add':addOpen=!addOpen;draw();break;
      case 'add-zip':void importZip();break;
      case 'confirm-zip-replace':
        if(pendingZipReplacement) {const selected=pendingZipReplacement;pendingZipReplacement=null;void importZip(selected,true);}
        break;
      case 'add-github':addOpen=false;githubForm=true;scan=null;editingSource=null;changingRefSource=null;replaceConflicts=false;detail=null;draw();break;
      case 'close-github-form':githubForm=false;changingRefSource=null;draw();break;
      case 'tab-installed':tab='installed';pendingRemove=null;draw();break;
      case 'tab-sources':tab='sources';pendingRemove=null;draw();break;
      case 'refresh':error='';void refresh();break;
      case 'dismiss-toast':toast='';draw();break;
      case 'uninstall-skill':pendingUninstall=id;pendingActivation=null;draw();break;
      case 'cancel-uninstall':pendingUninstall=null;draw();break;
      case 'confirm-uninstall':void uninstall(id);break;
      case 'cancel-activation':pendingActivation=null;draw();break;
      case 'confirm-activation':if(pendingActivation?.id===id && pendingActivation.harness===button.dataset.harness) void setHarnessEnabled(id,pendingActivation.harness,true,true);break;
      case 'cancel-replacement':pendingReplacement=null;draw();break;
      case 'confirm-replacement':
        if(pendingReplacement?.id===id && pendingReplacement.harness===button.dataset.harness) {
          const decision=pendingReplacement;
          void setHarnessEnabled(id,decision.harness,true,decision.acknowledge,true);
        }
        break;
      case 'toggle-file-folder':{
        if(!path)break;
        if(collapsedFiles.has(path))collapsedFiles.delete(path);else collapsedFiles.add(path);
        draw();break;
      }
      case 'reactivate-skill':void setHarnessEnabled(id,button.dataset.harness??'',true);break;
      case 'try-skill':void tryInstalledSkill(id);break;
      case 'inspect-installed':detail={kind:'installed',id,file:'SKILL.md'};markdownMode='rendered';pendingActivation=null;pendingReplacement=null;pendingUninstall=null;collapsedFiles.clear();tryHarness='';draw();break;
      case 'inspect-remote':detail={kind:'remote',path,file:'SKILL.md'};markdownMode='rendered';pendingUninstall=null;pendingActivation=null;pendingReplacement=null;collapsedFiles.clear();draw();break;
      case 'close-detail':dismissDetail();break;
      case 'preview-file':if(detail){detail={...detail,file:button.dataset.file??'SKILL.md'};draw();}break;
      case 'preview-mode':markdownMode=button.dataset.mode==='raw'?'raw':'rendered';draw();break;
      case 'hide-discovery':scan=null;chosen.clear();editingSource=null;changingRefSource=null;draw();break;
      case 'select-all':if(scan)for(const entry of scan.skills)if([entry.name,entry.path,entry.description].some(s=>s.toLowerCase().includes(discoverySearch.toLowerCase())))chosen.add(entry.path);draw();break;
      case 'select-none':chosen.clear();draw();break;
      case 'import-selected':if(scan&&chosen.size){const snap={repo:scan.repo,path:scan.path,gitRef:scan.gitRef,commit:scan.commit,paths:[...chosen],allowReplace:replaceConflicts};void importSelected(snap);}break;
      case 'save-source-selection':
        if(editingSource)void (async()=>{try{
          const source=await options.call<SavedSource>('skills_source_set_selection',{id:editingSource,selectedPaths:[...chosen]});
          note=`Tracking updated for ${source.repo}; installed packages were not deleted.`;
          error='';await refresh();
        }catch(cause){error=message(cause);draw();}})();break;
      case 'retry-failed':
        if(operation?.kind==='github'&&operation.snapshot){
          const snap={...operation.snapshot,paths:[...operation.packages.values()].filter(entry=>entry.phase==='failed').map(entry=>entry.path)};
          if(snap.paths.length)void importSelected(snap);
        }else if(operation?.kind==='sync'){
          const ids=[...operation.packages.values()].filter(entry=>entry.phase==='failed').map(entry=>entry.path);
          if(ids.length)void synchronize('ids',undefined,ids);
        }
        break;
      case 'view-installed':tab='installed';search='';draw();break;
      case 'sync-skill':void synchronize('skill',id);break;
      case 'sync-source':void synchronize('source',id);break;
      case 'sync-all':void synchronize('all');break;
      case 'edit-source':void editSource(id);break;
      case 'change-source-ref':changeSourceRef(id);break;
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
    if(target.dataset.customizeHarness!==undefined) { void setHarnessEnabled(target.dataset.id??'',target.dataset.customizeHarness,target.checked); return; }
    if(target.name==='try-harness'){tryHarness=target.value;return;}
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
  return {refresh,closeDetail:()=>{
    if(detail) dismissDetail();
  },dispose(){
    disposed=true;previewSerial++;unsub?.();
    host.querySelector<HTMLDialogElement>('.customize-detail-dialog')?.close();
    if(toastTimer !== undefined)clearTimeout(toastTimer);
    host.removeEventListener('click',click);host.removeEventListener('input',input);
    host.removeEventListener('change',change);host.removeEventListener('submit',submit);
    host.replaceChildren();
  }};
}
