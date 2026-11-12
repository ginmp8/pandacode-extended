/* Standalone Customize -> Skills workspace. Owns no package storage or harness writes:
 * the native skills_* commands share one library with Settings -> Skills & rules. */
import './customize-skills.css';
import { escape, fileIcon, icon, markdownHtml } from './dom';
import { switchedOff, type SkillItem, type CatalogSkill, type SkillMatrix } from './skills-library';
import { SKILLS_CHANGED_EVENT, type SkillInventory } from './skill-manager-model';

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
type PreviewMode = 'rendered' | 'raw';
type Detail = { kind: 'installed'; id: string; file: string; mode: PreviewMode } | { kind: 'remote'; path: string; file: string; mode: PreviewMode };
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
  host.classList.add('customize-skills-root');
  let installed: SkillItem[] = [];
  let installedIcons: Record<string,string> = {};
  let sources: SavedSource[] = [];
  let policy = initialPolicy;
  let matrix: SkillMatrix = {project:null,harnesses:[]};
  let managerOff: ReadonlyMap<string,string> = new Map();
  let managerReadSerial = 0;
  let syncReport: SyncReport = {checkedAt:0,installed:[],updated:[],skipped:[],events:[]};
  let changingRefSource: string | null = null;
  let tab: 'installed' | 'sources' = 'installed';
  let addOpen = false, githubForm = false;
  let githubUrl = '', githubRef = '', search = '', discoverySearch = '';
  let scan: SourcePreview | null = null;
  let chosen = new Set<string>();
  let editingSource: string | null = null, replaceConflicts = false;
  let detail: Detail | null = null;
  const openedDetails: Detail[] = [];
  // The DOM is recreated on every state change. Remember the scroll position of
  // each open skill independently, including the nested file tree and preview.
  type DetailScroll = { content: number; files: number; preview: number; file: string };
  const scrollByDetail = new Map<string, DetailScroll>();
  let drawnDetailKey: string | null = null;
  let drawnDetailFile: string | null = null;
  function rememberDetailScroll(): void {
    if (!drawnDetailKey || !drawnDetailFile) return;
    const content = host.querySelector<HTMLElement>('.customize-side-content');
    if (!content) return;
    scrollByDetail.set(drawnDetailKey, {
      content: content.scrollTop,
      files: host.querySelector<HTMLElement>('.customize-file-list')?.scrollTop ?? 0,
      preview: host.querySelector<HTMLElement>('#customize-preview-content')?.scrollTop ?? 0,
      file: drawnDetailFile,
    });
  }
  function restoreDetailScroll(): void {
    if (!detail) return;
    const saved = scrollByDetail.get(detailKey(detail));
    if (!saved) return;
    const content = host.querySelector<HTMLElement>('.customize-side-content');
    const files = host.querySelector<HTMLElement>('.customize-file-list');
    const preview = host.querySelector<HTMLElement>('#customize-preview-content');
    if (content) content.scrollTop = saved.content;
    if (files) files.scrollTop = saved.files;
    if (preview && saved.file === detail.file) preview.scrollTop = saved.preview;
  }
  // Keep each tab's file and preview mode while switching between skills.
  const detailKey = (value: Detail) => value.kind === 'installed' ? `installed:${value.id}` : `remote:${value.path}`;
  let inspectorWidth: number | null = null;
  const inspectorWidthKey = 'pandacode.customizeSkillsInspectorWidth';
  try {
    const raw = localStorage.getItem(inspectorWidthKey);
    const saved = raw === null ? NaN : Number(raw);
    if (Number.isFinite(saved) && saved >= 320 && saved <= 4096) inspectorWidth = saved;
  } catch { /* Resizing remains available without storage. */ }
  if (inspectorWidth !== null) host.style.setProperty('--customize-inspector-width', `${inspectorWidth}px`);
  type ResizeDrag = { id: number; handle: HTMLElement; startX: number; startWidth: number; priorWidth: number | null };
  let resizeDrag: ResizeDrag | null = null;
  let previewText: PreviewFile | null = null;
  let previewSerial = 0;
  let pendingRemove: string | null = null;
  let busy = false, busyLabel = '', error = '', note = '', disposed = false;
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
  const offInSkills = (item: SkillItem): boolean => managerOff.has(item.name) || managerOff.has(item.id);
  async function readManager(): Promise<ReadonlyMap<string,string> | null> {
    try {
      return switchedOff(await options.call<SkillInventory>('skill_manager_list'), '');
    } catch {
      // Preserve last known switches. Native send-time enforcement still applies.
      return null;
    }
  }
  const onManagerChange = () => {
    const serial = ++managerReadSerial;
    void readManager().then(next => {
      if (!disposed && serial === managerReadSerial && next !== null) { managerOff = next; draw(); }
    });
  };

  function success(text: string): void {
    toast = text;
    if (toastTimer !== undefined) clearTimeout(toastTimer);
    draw();
    toastTimer = setTimeout(() => { if (!disposed && toast === text) { toast = ''; draw(); } }, 5500);
  }

  function activityHtml(): string {
    if (!busy && (!refreshing || firstLoad)) return '';
    const label = busy ? busyLabel || 'Working on skills…' : 'Refreshing installed skills…';
    return `<div class="customize-activity" role="status" aria-label="Skill action in progress" aria-live="polite">
      <span class="customize-spinner" aria-hidden="true"></span><span>${esc(label)}</span>
    </div>`;
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
    pendingActivation=null; pendingReplacement=null; busy=true;
    busyLabel=`${enabled?'Enabling':'Disabling'} ${item.name} for ${harnessLabel(harness)}…`; error=''; draw();
    try {
      const change = enabled
        ? await options.call<{kept:string[]}>('skills_activate', {id,harness,scope:'user',acknowledge,replaceExisting})
        : await options.call<{kept:string[]}>('skills_deactivate', {id,harness,scope:'user'});
      await refresh();
      const kept = change.kept?.length
        ? enabled ? ` ${change.kept.join(' ')}.`
          : ` ${plural(change.kept.length,'edited file')} preserved on disk.`
        : '';
      success(`${item.name} ${enabled?'enabled':'disabled'} for ${harnessLabel(harness)}.${kept}${enabled && offInSkills(item) ? ' Still switched off in Settings → Tools → Skills.' : ''}`);
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
    busy=true; busyLabel=`Uninstalling ${item.name}…`; error=''; draw();
    try {
      await options.call<SkillItem[]>('skills_remove',{id});
      pendingUninstall=null; pendingActivation=null;
      await refresh();
      success(`${item.name} uninstalled from the PandaCode library.`);
    } catch (cause) { error=message(cause); }
    finally { busy=false; draw(); }
  }

  async function tryInstalledSkill(id: string): Promise<void> {
    if (busy || operation?.state === 'running') return;
    const item=installed.find(entry=>entry.id===id);
    const available=item && !offInSkills(item) ? matrix.harnesses.filter(row=>row.installed && currentUserActivation(item,row.harness)) : [];
    const harness=available.find(row=>row.harness===tryHarness)?.harness ?? available[0]?.harness;
    if (!item || !harness) return;
    busy=true; busyLabel=`Opening ${item.name} in chat…`; error=''; draw();
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

  /** Use the inspector Files row grammar: a flat, depth-indented, collapsible tree. */
  function filesTreeHtml(paths: string[], selected: string): string {
    type Node = { name: string; path: string; children: Map<string, Node>; isFile: boolean };
    const root: Node = {name:'',path:'',children:new Map(),isFile:false};
    for (const relative of paths) {
      const segments = relative.split('/').filter(Boolean);
      let parent = root;
      for (const [index, name] of segments.entries()) {
        const path = segments.slice(0,index+1).join('/');
        let child = parent.children.get(name);
        if (!child) { child={name,path,children:new Map(),isFile:false};parent.children.set(name,child); }
        if (index === segments.length-1) child.isFile = true;
        parent = child;
      }
    }
    const render = (nodes: Map<string,Node>, depth: number): string =>
      [...nodes.values()].sort((a,b)=>Number(a.isFile)-Number(b.isFile)||a.name.localeCompare(b.name))
        .map(node => {
          const folder = !node.isFile || node.children.size > 0;
          const open = !collapsedFiles.has(node.path);
          const action = folder ? 'toggle-file-folder' : 'preview-file';
          const label = folder ? ` aria-label="${open?'Collapse':'Expand'} ${esc(node.name)} folder"` : '';
          const expanded = folder ? ` aria-expanded="${open}"` : '';
          const selectedState = folder ? '' : ` aria-selected="${selected===node.path}"`;
          const row = `<div class="file-row ${!folder&&selected===node.path?'customize-file-selected':''}" role="treeitem" aria-level="${depth+1}"${expanded}${selectedState} style="--depth:${depth}">
            <button type="button" data-customize="${action}" data-path="${esc(node.path)}" data-file="${folder?'':esc(node.path)}" title="${esc(node.path)}"${label}${expanded}>
              ${folder?icon('chevron','file-twisty'):'<span class="file-twisty"></span>'}
              ${fileIcon(node.name,folder)}<span>${esc(node.name)}</span>
            </button>
          </div>`;
          return row + (folder&&open ? render(node.children,depth+1) : '');
        }).join('');
    return `<div class="file-list customize-file-list" role="tree" aria-label="Package file tree">${render(root.children,0)}</div>`;
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


  function selectDetail(next: Detail): void {
    const key = detailKey(next);
    const existing = openedDetails.find(value => detailKey(value) === key);
    if (!existing) openedDetails.push(next);
    if (detail && detailKey(detail) === key) return;
    detail = existing ?? next;
    pendingActivation = null; pendingReplacement = null; pendingUninstall = null;
    collapsedFiles.clear(); tryHarness = ''; previewSerial++;
    draw();
    host.querySelector<HTMLButtonElement>('.customize-skill-tab.selected .customize-skill-tab-button')?.focus({preventScroll:true});
  }

  function replaceActiveDetail(next: Detail): void {
    const index = openedDetails.findIndex(value => detailKey(value) === detailKey(next));
    if (index >= 0) openedDetails[index] = next;
    detail = next;
    draw();
  }

  function closeSkillDetail(key = detail ? detailKey(detail) : ''): void {
    const index = openedDetails.findIndex(value => detailKey(value) === key);
    if (index < 0) return;
    const closing = openedDetails[index];
    const wasActive = detail !== null && detailKey(detail) === key;
    openedDetails.splice(index, 1);
    if (wasActive) {
      detail = openedDetails[index] ?? openedDetails[index - 1] ?? null;
      pendingActivation = null; pendingReplacement = null; pendingUninstall = null;
      collapsedFiles.clear(); previewSerial++;
    }
    draw();
    scrollByDetail.delete(key);
    if (detail) host.querySelector<HTMLButtonElement>('.customize-skill-tab.selected .customize-skill-tab-button')?.focus({preventScroll:true});
    else if (wasActive) {
      const selector = closing.kind === 'installed'
        ? `[data-customize="inspect-installed"][data-id="${CSS.escape(closing.id)}"]`
        : `[data-customize="inspect-remote"][data-path="${CSS.escape(closing.path)}"]`;
      host.querySelector<HTMLButtonElement>(selector)?.focus({preventScroll:true});
    }
  }

  function forgetRemoteDetails(): void {
    const wasRemote = detail?.kind === 'remote';
    for (let index = openedDetails.length - 1; index >= 0; index--) {
      if (openedDetails[index].kind === 'remote') openedDetails.splice(index, 1);
    }
    if (wasRemote) {
      detail = openedDetails.at(-1) ?? null;
      pendingActivation = null; pendingReplacement = null; pendingUninstall = null;
      collapsedFiles.clear(); previewSerial++;
    }
  }

  function pruneUninstalledTabs(): void {
    const remaining = new Set(installed.map(item => item.id));
    const lostActive = detail?.kind === 'installed' && !remaining.has(detail.id);
    for (let index = openedDetails.length - 1; index >= 0; index--) {
      const value = openedDetails[index];
      if (value.kind === 'installed' && !remaining.has(value.id)) openedDetails.splice(index, 1);
    }
    if (lostActive) {
      detail = openedDetails.at(-1) ?? null;
      pendingActivation = null; pendingReplacement = null; pendingUninstall = null;
      collapsedFiles.clear(); previewSerial++;
    }
  }

  function inspectorWidthRange(): {min: number; max: number} {
    return {min: 320, max: Math.max(320, host.clientWidth - 440)};
  }

  function updateResizeHandle(): void {
    const handle = host.querySelector<HTMLElement>('.customize-side-resize');
    const pane = host.querySelector<HTMLElement>('.customize-detail-side');
    if (!handle || !pane) return;
    const bounds = inspectorWidthRange();
    handle.setAttribute('aria-valuemin', String(bounds.min));
    handle.setAttribute('aria-valuemax', String(bounds.max));
    handle.setAttribute('aria-valuenow', String(Math.round(pane.getBoundingClientRect().width)));
  }

  function applyInspectorWidth(value: number | null, persist = false): void {
    if (value === null) {
      inspectorWidth = null;
      host.style.removeProperty('--customize-inspector-width');
    } else {
      const {min, max} = inspectorWidthRange();
      inspectorWidth = Math.round(Math.min(max, Math.max(min, value)));
      host.style.setProperty('--customize-inspector-width', `${inspectorWidth}px`);
    }
    if (persist) {
      try {
        if (inspectorWidth === null) localStorage.removeItem(inspectorWidthKey);
        else localStorage.setItem(inspectorWidthKey, String(inspectorWidth));
      } catch { /* The selected width still applies in the current session. */ }
    }
    updateResizeHandle();
  }

  function finishResize(commit: boolean): void {
    if (!resizeDrag) return;
    const finished = resizeDrag;
    resizeDrag = null;
    host.classList.remove('customize-side-resizing');
    if (finished.handle.isConnected && finished.handle.hasPointerCapture(finished.id)) finished.handle.releasePointerCapture(finished.id);
    if (commit) applyInspectorWidth(inspectorWidth, true);
    else applyInspectorWidth(finished.priorWidth);
  }

  function resizePointerDown(event: PointerEvent): void {
    const handle = event.target instanceof Element ? event.target.closest<HTMLElement>('.customize-side-resize') : null;
    if (!handle || !host.contains(handle) || resizeDrag || event.button !== 0 || !event.isPrimary || matchMedia('(max-width:1100px)').matches) return;
    event.preventDefault();
    const pane = host.querySelector<HTMLElement>('.customize-detail-side');
    if (!pane) return;
    resizeDrag = {id:event.pointerId, handle, startX:event.clientX, startWidth:pane.getBoundingClientRect().width, priorWidth:inspectorWidth};
    handle.setPointerCapture(event.pointerId);
    handle.focus({preventScroll:true});
    host.classList.add('customize-side-resizing');
  }
  function resizePointerMove(event: PointerEvent): void {
    if (!resizeDrag || resizeDrag.id !== event.pointerId) return;
    applyInspectorWidth(resizeDrag.startWidth + resizeDrag.startX - event.clientX);
  }
  function resizePointerUp(event: PointerEvent): void { if (resizeDrag?.id === event.pointerId) finishResize(true); }
  function resizePointerCancel(event: PointerEvent): void { if (resizeDrag?.id === event.pointerId) finishResize(false); }
  function resetInspectorWidth(event: MouseEvent): void {
    if (!(event.target instanceof Element) || !event.target.closest('.customize-side-resize')) return;
    event.preventDefault();
    finishResize(false);
    applyInspectorWidth(null, true);
  }

  function installedHtml(): string {

    const visible = installed.filter(item => item.kind==='skill' && [item.name, item.id, item.description, item.source.location]
      .some(value => value.toLowerCase().includes(search.trim().toLowerCase())));
    const card = (item: SkillItem) => {
      const type = item.source.kind === 'github' ? 'GitHub' : item.source.kind === 'zip' ? 'ZIP' : 'Local';
      const active = item.activations?.length ?? 0;
      const statuses = matrix.harnesses.flatMap(row=>row.cells.filter(cell=>cell.id===item.id && cell.status!=='off')
        .map(cell=>({label:`${harnessLabel(row.harness)}: ${cell.status}`, live:cell.status==='active'||cell.status==='prompt'})));
      const off=offInSkills(item);
      const live=statuses.some(status=>status.live) && !off;
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
          ${off ? '<small class="customize-warning">Off in Settings → Tools → Skills</small>' : ''}
          ${statuses.length ? `<small class="customize-harness-statuses">${statuses.map(status=>`<span class="${status.live && !off?'customize-active-text':''}">${esc(status.label)}</span>`).join('<span aria-hidden="true"> · </span>')}</small>` : ''}</span>
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
    const switchedOffInSkills = offInSkills(item);
    const available = selectable.filter(row=>row.installed && !switchedOffInSkills && currentUserActivation(item,row.harness));
    const picked = available.find(row=>row.harness===tryHarness)?.harness ?? available[0]?.harness ?? '';
    const harnesses = selectable.map(row => {
      const enabled = item.activations.some(a=>a.harness===row.harness && a.scope==='user');
      const state = row.cells.find(c=>c.id===item.id && c.scope==='user')?.status ?? 'off';
      const status = !row.installed ? 'Harness unavailable' : !enabled ? 'Not enabled' :
        state === 'active' || state === 'prompt' ? (switchedOffInSkills ? 'Installed; off in Skills' : 'Enabled') :
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
        `Disable all ${plural(item.activations.length,'activation')} before uninstalling. Rules & library can turn off legacy activations everywhere, including project scopes.` :
        'This removes the skill package from the central library. Saved GitHub sources remain configured, but removed skills are not automatically reinstalled.'}</p>
      <div class="customize-inline-actions">
        <button type="button" class="secondary-button" data-customize="cancel-uninstall">Cancel uninstall</button>
        <button type="button" class="customize-danger" data-customize="confirm-uninstall" data-id="${esc(item.id)}" ${item.activations.length || busy ? 'disabled':''}>Confirm uninstall</button>
      </div>
    </div>` : '';
    return `<div class="customize-lifecycle" aria-label="Manage installed skill">
      <div class="customize-section-head"><div><h3>Activation by harness</h3><p>Manage user-scope library copies here. For project-specific installs and switches, use Settings → Tools → Skills.</p></div></div>
      ${switchedOffInSkills ? '<p class="customize-help" role="status">Switched off in Settings → Tools → Skills for all projects. The harness copy stays installed, but Try in chat is unavailable until you turn the skill back on there.</p>' : ''}
      ${harnesses ? `<div class="customize-harness-grid">${harnesses}</div>` : '<p class="customize-help">No supported harness is installed. Install a harness to enable this skill.</p>'}
      ${projectCount ? `<p class="customize-help">${plural(projectCount,'legacy project activation')} recorded. Use Rules & library to turn off legacy activations everywhere.</p>` : ''}
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


  function skillTabsHtml(): string {
    const tabs = openedDetails.map(value => {
      const key = detailKey(value);
      const item = value.kind === 'installed'
        ? installed.find(skill => skill.id === value.id)
        : scan?.skills.find(skill => skill.path === value.path);
      const name = item?.name ?? (value.kind === 'installed' ? value.id : value.path);
      const characters = Array.from(name);
      const label = characters.length > 22 ? characters.slice(0, 22).join('') + '…' : name;
      const selected = detail !== null && detailKey(detail) === key;
      return `<div class="customize-skill-tab${selected ? ' selected' : ''}" data-skill-tab="${esc(key)}">
        <button type="button" role="tab" class="customize-skill-tab-button${selected ? ' selected' : ''}"
          data-customize="activate-skill-tab" data-key="${esc(key)}" aria-controls="customize-skill-panel-content"
          aria-selected="${selected}" aria-label="${esc(name)}" title="${esc(name)}" tabindex="${selected ? 0 : -1}">
          ${icon(value.kind === 'installed' ? 'skill' : 'github')}<span class="customize-skill-tab-label" aria-hidden="true">${esc(label)}</span>
        </button>
        <button type="button" class="customize-skill-tab-close" data-customize="close-skill-tab"
          data-key="${esc(key)}" aria-label="Close ${esc(name)}" title="Close ${esc(name)}" tabindex="-1">${icon('close')}</button>
      </div>`;
    }).join('');
    return `<div class="customize-side-tabbar">
      <nav class="customize-skill-tabs" role="tablist" aria-label="Open skill previews">${tabs}</nav>
    </div>`;
  }

  function detailHtml(): string {
    if (!detail) return '';
    const item = detail.kind === 'installed'
      ? installed.find(entry => entry.id === (detail as {id:string}).id)
      : scan?.skills.find(entry => entry.path === (detail as {path:string}).path);
    if (!item) return '';
    const files = detail.kind === 'installed'
      ? (item as SkillItem).files
      : ((item as CatalogSkill).filePaths?.length ? (item as CatalogSkill).filePaths! : ['SKILL.md']);
    const source = detail.kind === 'installed'
      ? (item as SkillItem).source
      : {kind:'github',location:scan!.repo,gitRef:scan!.gitRef,commit:scan!.commit};
    const sourceUrl = githubSourceLink(source);
    return `<aside id="customize-skill-detail-panel" class="customize-detail-side" role="region" aria-label="Read-only skill preview">
      <div class="customize-side-resize" role="separator" tabindex="0" aria-orientation="vertical" aria-label="Resize skill inspector"
        aria-description="Use Left and Right arrows to resize. Hold Shift for larger steps. Home and End select the minimum and maximum widths. Double-click to reset."
        aria-controls="customize-skill-detail-panel" title="Resize skill inspector · Double-click to reset"></div>
      ${skillTabsHtml()}
      <header class="customize-side-heading">
        <div><p class="customize-eyebrow">${detail.kind === 'remote' ? 'GITHUB PREVIEW · NOT INSTALLED' : 'INSTALLED PACKAGE'}</p>
          <h2>${esc(item.name)}</h2></div>
      </header>
      <div id="customize-skill-panel-content" class="customize-side-content" role="tabpanel" aria-label="${esc(item.name)}">
        ${activityHtml()}
        <p class="customize-side-description">${esc(item.description)}</p>
        <div class="customize-side-source"><small>${esc(source.location)} · ${esc(shaShort(source.commit))}</small>
          ${sourceUrl ? `<a class="customize-source-link" href="${esc(sourceUrl)}" target="_blank" rel="noopener noreferrer">View source</a>` : ''}</div>
        <section class="customize-package-files" aria-label="Package files">
          <div class="file-folder-path"><span>Files</span><small>${plural(files.length,'file')}</small></div>
          ${filesTreeHtml(files,detail.file)}
        </section>
        <section class="customize-preview-main" aria-label="Selected package file">
          <div class="customize-preview-toolbar"><span title="${esc(detail.file)}">${esc(detail.file)}</span>
            <div class="customize-mode-switch">
              <button type="button" data-customize="preview-mode" data-mode="rendered" aria-pressed="${detail.mode==='rendered'}">Rendered</button>
              <button type="button" data-customize="preview-mode" data-mode="raw" aria-pressed="${detail.mode==='raw'}">Raw</button>
            </div>
          </div>
          <div id="customize-preview-content" class="customize-preview-body" role="region" aria-label="Skill file content">
            <span class="customize-spinner" aria-hidden="true"></span>Loading file…
          </div>
        </section>
        ${detail.kind === 'installed' ? lifecycleHtml(item as SkillItem) : ''}
        ${toast ? `<div class="customize-dialog-toast" role="status">${esc(toast)}</div>` : ''}
        ${error ? `<p class="customize-dialog-error" role="alert">${esc(error)}</p>` : ''}
      </div>
    </aside>`;
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
    if (resizeDrag) finishResize(false); // A full redraw must not orphan an active pointer capture.
    const hubScrollTop = host.scrollTop;
    rememberDetailScroll();
    host.classList.toggle('customize-detail-open', !!detail);
    const active = document.activeElement as HTMLInputElement | null;
    const withinHost = !!active && host.contains(active);
    const activeName = withinHost ? active.name : '';
    const activeSkillPath = withinHost ? active.dataset.selectSkill : undefined;
    const activeAction = withinHost ? active.dataset.customize : undefined;
    const activeHarness = withinHost ? active.dataset.customizeHarness : undefined;
    const activeHarnessId = withinHost ? active.dataset.id : undefined;
    const activeActionId = withinHost ? active.dataset.id : undefined;
    const activePath = withinHost ? active.dataset.file || active.dataset.path : undefined;
    const activeTabKey = withinHost ? active.dataset.key : undefined;
    const activeMode = withinHost ? active.dataset.mode : undefined;
    const cursor = activeName && typeof active?.selectionStart === 'number' ? active.selectionStart : -1;
    const previousTabScroll = host.querySelector<HTMLElement>('.customize-skill-tabs')?.scrollLeft ?? 0;
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
          ${detail ? '' : activityHtml()}
          ${note ? `<p class="customize-notice" role="status">${esc(note)}</p>` : ''}
          ${githubFormHtml()}
          ${scan ? discoveryHtml() : ''}
          <div id="customize-operation-slot">${operationHtml()}</div>
          ${firstLoad || loadFailed ? `<section class="customize-loading" role="status" aria-label="${refreshing ? 'Loading installed skills' : 'Unable to load installed skills'}">
            ${refreshing ? '<span class="customize-spinner" aria-hidden="true"></span><strong>Loading installed skills…</strong><p>Reading your library and harness activations.</p><div class="customize-skeleton"></div><div class="customize-skeleton"></div>' : '<strong>Unable to load the skill library.</strong><p>Use Refresh to retry. Installed skills have not been changed.</p>'}
          </section>` : (tab === 'installed' ? installedHtml() : sourcesHtml())}
        </section>
      </div>
    </div>${detailHtml()}`;
    drawnDetailKey = detail ? detailKey(detail) : null;
    drawnDetailFile = detail?.file ?? null;
    const strip = host.querySelector<HTMLElement>('.customize-skill-tabs');
    if (strip) {
      strip.scrollLeft = previousTabScroll;
      const selectedTab = strip.querySelector<HTMLElement>('.customize-skill-tab.selected');
      if (selectedTab) {
        const visible = strip.getBoundingClientRect(), chosenTab = selectedTab.getBoundingClientRect();
        if (chosenTab.left < visible.left) strip.scrollLeft -= visible.left - chosenTab.left;
        else if (chosenTab.right > visible.right) strip.scrollLeft += chosenTab.right - visible.right;
      }
    }
    const next = activeHarness !== undefined
      ? [...host.querySelectorAll<HTMLInputElement>('[data-customize-harness]')].find(input => input.dataset.customizeHarness === activeHarness && input.dataset.id === activeHarnessId)
      : activeSkillPath !== undefined
      ? [...host.querySelectorAll<HTMLInputElement>('[data-select-skill]')].find(input => input.dataset.selectSkill === activeSkillPath)
      : activeAction
        ? [...host.querySelectorAll<HTMLElement>('[data-customize]')].find(button => button.dataset.customize === activeAction && (activePath !== undefined ? (button.dataset.file || button.dataset.path) === activePath : activeTabKey !== undefined ? button.dataset.key === activeTabKey : activeMode !== undefined ? button.dataset.mode === activeMode : button.dataset.id === activeActionId))
        : activeName
          ? [...host.querySelectorAll<HTMLInputElement>('input[name]')].find(input => input.name === activeName)
          : undefined;
    if (next && !next.hasAttribute('disabled')) {
      next.focus({ preventScroll: true });
      if (cursor >= 0 && next instanceof HTMLInputElement && next.type !== 'checkbox')
        next.setSelectionRange(cursor, cursor);
    }
    restoreDetailScroll();
    host.scrollTop = hubScrollTop;
    updateResizeHandle();
    if (detail) void loadPreview();
  }

  async function refresh(): Promise<void> {
    const serial = ++refreshSerial;
    const managerSerial = ++managerReadSerial;
    refreshing = true;
    draw();
    try {
      const [items, saved, current, statuses, recent, icons, managerSwitches] = await Promise.all([
        options.call<SkillItem[]>('skills_list'), options.call<SavedSource[]>('skills_source_list'),
        options.call<Policy>('skills_sync_policy'),
        options.call<SkillMatrix>('skills_matrix').catch(()=>({project:null,harnesses:[]} as SkillMatrix)),
        options.call<SyncReport>('skills_sync_report').catch(()=>({checkedAt:0,installed:[],updated:[],skipped:[],events:[]} as SyncReport)),
        options.call<Record<string,string>>('skills_icons').catch(()=>({})),
        readManager(),
      ]);
      if (disposed || serial !== refreshSerial) return;
      installed=items; installedIcons=icons; sources=saved; policy=current; matrix=statuses; syncReport=recent;
      if (managerSerial === managerReadSerial && managerSwitches !== null) managerOff=managerSwitches;
      pruneUninstalledTabs();
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
    const content = slot.closest<HTMLElement>('.customize-side-content');
    const contentBeforeLoad = content?.scrollTop ?? 0;
    slot.innerHTML = '<span class="customize-preview-loading"><span class="customize-spinner" aria-hidden="true"></span>Loading file…</span>';
    try {
      const value = current.kind === 'installed'
        ? await options.call<PreviewFile>('skills_preview_file', {id:current.id,file:current.file})
        : await options.call<PreviewFile>('skills_github_preview_file', {repo:scan!.repo,path:current.path,commit:scan!.commit,file:current.file});
      if (disposed || serial !== previewSerial || !slot.isConnected) return;
      previewText = value;
      if (value.binary || value.text === null) {
        slot.textContent = `This file cannot be rendered as text (${value.size} bytes).`; return;
      }
      if (current.mode === 'raw' || !/\.md$/i.test(current.file)) {
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
    } finally {
      if (!disposed && serial === previewSerial && slot.isConnected) {
        const saved = scrollByDetail.get(detailKey(current));
        // The loading placeholder can temporarily shrink the scrollable content.
        // Restore only if the user did not scroll while the file was loading.
        if (saved && saved.file === current.file && slot.scrollTop === 0) slot.scrollTop = saved.preview;
        if (saved && content && content.scrollTop === contentBeforeLoad) content.scrollTop = saved.content;
      }
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
    busy = true; busyLabel=`Importing ${plural(packages.length,'skill')} from GitHub…`; draw();
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
    addOpen = false; busy=true; busyLabel='Selecting a ZIP archive…'; draw();
    let path: string | null;
    try { path = selectedPath ?? await options.chooseZip(); }
    catch (cause) { busy=false; error = message(cause); draw(); return; }
    if (!path) { busy=false; draw(); return; }
    pendingZipReplacement = null;
    const filename = path.split(/[\\/]/).pop() || 'Skill archive';
    const op = beginOperation('zip','Importing ZIP package',filename,null,[{path:filename,name:filename}]);
    busy = true; busyLabel=`Installing ${filename}…`; draw();
    try {
      const result = await options.call<{item:SkillItem;replaced:boolean;previousSource:SkillItem['source']|null;impactedHarnesses:string[]}>('skills_import_zip_tracked',{path,operationId:op.id,allowReplace:confirmed});
      const item = op.packages.get(filename)!;
      item.phase = result.replaced ? 'replaced' : 'installed'; item.name = result.item.name;
      op.completed = 1; op.state = 'success';
      op.detail = result.replaced
        ? `Replaced previous central library source ${result.previousSource?.location ?? 'unknown'}. ${plural(result.impactedHarnesses.length,'harness')} may require reactivation. Native harness files were not overwritten.`
        : 'Installed into the central library. No harness was activated.';
      note = `${result.replaced ? 'Replaced' : 'Installed'} ${result.item.name}.`;
      // Installing does not open a blocking dialog; users can inspect the new row explicitly.
      tab='installed';
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
    busy=true; busyLabel=`${label}…`; draw();
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
    busy=true;busyLabel='Scanning GitHub for SKILL.md packages…';error='';note='Scanning repository for SKILL.md packages…';draw();
    try {
      const changing=changingRefSource?sources.find(source=>source.id===changingRefSource):null;
      const result=await options.call<SourcePreview>('skills_source_preview',{repo:githubUrl.trim(),path:changing?.path??'',gitRef:githubRef.trim()||null});
      forgetRemoteDetails(); scan=result;chosen=new Set(changing?.selectedPaths.filter(path=>result.skills.some(skill=>skill.path===path))??[]);
      editingSource=null;replaceConflicts=false;discoverySearch='';githubForm=false;
      note=`Found ${plural(result.skills.length,'skill')} at ${shaShort(result.commit)}. Select packages to import.`;
    } catch(cause){error=message(cause);note='';}
    finally{busy=false;draw();}
  }

  async function editSource(id:string):Promise<void> {
    const source=sources.find(s=>s.id===id); if(!source)return;
    changingRefSource=null;
    githubUrl=`https://github.com/${source.repo}${source.path?`/tree/${source.gitRef??'main'}/${source.path}`:''}`;
    githubRef=source.gitRef??'';busy=true;busyLabel='Scanning saved GitHub source…';error='';note='Scanning saved source…';draw();
    try {
      const sourcePreview=await options.call<SourcePreview>('skills_source_preview',{repo:source.repo,path:source.path,gitRef:source.gitRef});
      forgetRemoteDetails(); scan=sourcePreview;
      chosen=new Set(source.selectedPaths);editingSource=id;replaceConflicts=false;discoverySearch='';note='Save tracked selection to change automatic update scope without uninstalling packages.';
    } catch(cause){error=message(cause);}
    finally{busy=false;draw();}
  }

  function changeSourceRef(id:string):void {
    const source=sources.find(source=>source.id===id);
    if(!source)return;
    changingRefSource=id;editingSource=null;forgetRemoteDetails();scan=null;chosen.clear();
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
      case 'add-github':addOpen=false;githubForm=true;forgetRemoteDetails();scan=null;editingSource=null;changingRefSource=null;replaceConflicts=false;draw();break;
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
      case 'inspect-installed':selectDetail({kind:'installed',id,file:'SKILL.md',mode:'rendered'});break;
      case 'inspect-remote':selectDetail({kind:'remote',path,file:'SKILL.md',mode:'rendered'});break;
      case 'activate-skill-tab':{
        const tabToActivate = openedDetails.find(value => detailKey(value) === button.dataset.key);
        if (tabToActivate) selectDetail(tabToActivate);
        break;
      }
      case 'close-skill-tab':closeSkillDetail(button.dataset.key);break;
      case 'preview-file':if(detail)replaceActiveDetail({...detail,file:button.dataset.file??'SKILL.md'});break;
      case 'preview-mode':if(detail)replaceActiveDetail({...detail,mode:button.dataset.mode==='raw'?'raw':'rendered'});break;
      case 'hide-discovery':forgetRemoteDetails();scan=null;chosen.clear();editingSource=null;changingRefSource=null;draw();break;
      case 'select-all':if(scan)for(const entry of scan.skills)if([entry.name,entry.path,entry.description].some(s=>s.toLowerCase().includes(discoverySearch.toLowerCase())))chosen.add(entry.path);draw();break;
      case 'select-none':chosen.clear();draw();break;
      case 'import-selected':if(scan&&chosen.size){const snap={repo:scan.repo,path:scan.path,gitRef:scan.gitRef,commit:scan.commit,paths:[...chosen],allowReplace:replaceConflicts};void importSelected(snap);}break;
      case 'save-source-selection':
        if(editingSource && !busy && operation?.state!=='running')void (async()=>{
          busy=true;busyLabel='Saving tracked GitHub selection…';draw();
          try{
            const source=await options.call<SavedSource>('skills_source_set_selection',{id:editingSource,selectedPaths:[...chosen]});
            note=`Tracking updated for ${source.repo}; installed packages were not deleted.`;
            error='';await refresh();
          }catch(cause){error=message(cause);}finally{busy=false;draw();}
        })();break;
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
        pendingRemove=null;void (async()=>{
          const existingBusy=busy; if(!existingBusy){busy=true;busyLabel='Removing GitHub source…';draw();}
          try{sources=await options.call<SavedSource[]>('skills_source_remove',{id});note='Source configuration removed. Installed packages were kept.';error='';}
          catch(cause){error=message(cause);}finally{if(!existingBusy)busy=false;draw();}
        })();break;
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
      void (async()=>{
        busy=true;busyLabel='Updating startup synchronization…';draw();
        try{policy=await options.call<Policy>('skills_sync_set_global',{enabled:current});note=current?'Startup sync enabled for previously imported skills.':'Startup sync disabled.';error='';}
        catch(cause){error=message(cause);}finally{busy=false;draw();}
      })();
    }
  };
  const submit=(event:Event)=>{if((event.target as HTMLElement).id==='customize-github-form'){event.preventDefault();void scanGithub();}};

  const keydown=(event:KeyboardEvent)=>{
    const target = event.target instanceof HTMLElement ? event.target : null;
    if (target?.matches('.customize-side-resize')) {
      if (event.altKey || event.ctrlKey || event.metaKey || matchMedia('(max-width:1100px)').matches) return;
      if (event.key === 'Escape' && resizeDrag) { event.preventDefault(); finishResize(false); return; }
      const {min,max} = inspectorWidthRange();
      const current = host.querySelector<HTMLElement>('.customize-detail-side')?.getBoundingClientRect().width ?? min;
      const step = event.shiftKey ? 32 : 8;
      const width = event.key === 'ArrowLeft' ? current + step : event.key === 'ArrowRight' ? current - step
        : event.key === 'Home' ? min : event.key === 'End' ? max : null;
      if (width !== null) { event.preventDefault(); applyInspectorWidth(width,true); }
      return;
    }
    if (target?.matches('.customize-skill-tab-button') && !event.altKey && !event.ctrlKey && !event.metaKey) {
      const tabs = [...host.querySelectorAll<HTMLButtonElement>('.customize-skill-tab-button')];
      const index = tabs.indexOf(target as HTMLButtonElement);
      if (index >= 0) {
        if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); closeSkillDetail(target.dataset.key); return; }
        const next = event.key === 'ArrowRight' ? tabs[(index+1)%tabs.length] : event.key === 'ArrowLeft' ? tabs[(index-1+tabs.length)%tabs.length]
          : event.key === 'Home' ? tabs[0] : event.key === 'End' ? tabs.at(-1) : undefined;
        if (next) { event.preventDefault(); const found=openedDetails.find(value => detailKey(value) === next.dataset.key); if (found) selectDetail(found); }
      }
    }
    if (event.key === 'Escape' && detail) { event.preventDefault(); closeSkillDetail(); }
  };
  window.addEventListener(SKILLS_CHANGED_EVENT,onManagerChange);
  host.addEventListener('keydown',keydown);
  host.addEventListener('pointerdown',resizePointerDown);
  host.addEventListener('pointermove',resizePointerMove);
  host.addEventListener('pointerup',resizePointerUp);
  host.addEventListener('pointercancel',resizePointerCancel);
  host.addEventListener('dblclick',resetInspectorWidth);
  host.addEventListener('click',click);
  host.addEventListener('input',input);
  host.addEventListener('change',change);
  host.addEventListener('submit',submit);
  void options.subscribe(handleProgress).then(cleanup=>{if(disposed)cleanup();else unsub=cleanup;}).catch(()=>{});
  draw();
  void refresh();
  return {refresh,closeDetail:()=>{
    if(detail) closeSkillDetail();
  },dispose(){
    disposed=true;previewSerial++;unsub?.();
    finishResize(false);
    host.classList.remove('customize-skills-root','customize-detail-open','customize-side-resizing');
    host.style.removeProperty('--customize-inspector-width');
    if(toastTimer !== undefined)clearTimeout(toastTimer);
    window.removeEventListener(SKILLS_CHANGED_EVENT,onManagerChange);
    host.removeEventListener('keydown',keydown);
    host.removeEventListener('pointerdown',resizePointerDown);
    host.removeEventListener('pointermove',resizePointerMove);
    host.removeEventListener('pointerup',resizePointerUp);
    host.removeEventListener('pointercancel',resizePointerCancel);
    host.removeEventListener('dblclick',resetInspectorWidth);
    host.removeEventListener('click',click);host.removeEventListener('input',input);
    host.removeEventListener('change',change);host.removeEventListener('submit',submit);
    host.replaceChildren();
  }};
}
