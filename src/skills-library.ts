/* Settings → Skills & rules (issues #15 and #45): the library, imports, where each item is
 * active per harness and scope, the drift matrix and safety findings. Everything is native
 * (`skills_*` commands in src-tauri/src/skills.rs); this view only draws and asks. Catalogs
 * (#114, #115) index GitHub repositories of skills; importing from one never activates. */
import './skills-library.css';
import { escape } from './dom';
import { infoMark } from './tooltip';

export type SkillKind = 'skill' | 'rule';
export type SkillScope = 'user' | 'project';
export type SkillFinding = { rule: 'secret' | 'approvals' | 'pipe-to-shell'; file: string; line: number; excerpt: string };
export type SkillActivation = { harness: string; scope: SkillScope; project: string | null; surface: 'folder' | 'block' | 'prompt'; sha256: string; files: { path: string; sha256: string }[] };
export type SkillItem = {
  id: string; name: string; kind: SkillKind; description: string; sha256: string; importedAt: number;
  source: { kind: 'folder' | 'github'; location: string; gitRef: string | null; commit: string | null };
  acknowledged: string | null; autoUpdate: boolean | null; autoAvailable?: boolean; requiredSkills?: string[]; fileHashes: Record<string, string>; activations: SkillActivation[]; files: string[]; findings: SkillFinding[];
};
export type SkillCell = { id: string; scope: SkillScope; status: 'active' | 'outdated' | 'drifted' | 'missing' | 'prompt' | 'off'; surface: string };
export type SkillMatrix = {
  project: string | null;
  harnesses: { harness: string; installed: boolean; prompt: boolean; cells: SkillCell[]; unmanaged: { scope: SkillScope; kind: SkillKind; path: string; detail: string }[] }[];
};
export type CatalogSkill = { path: string; id: string; name: string; description: string; files: number; bytes: number; imported: string | null; conflicts: string[] };
export type CatalogSource = {
  repo: string; gitRef: string | null; commit: string | null; origin: 'manual' | 'discovered'; state: 'enabled' | 'disabled' | 'ignored';
  private: boolean; pushedAt: string | null; scannedAt: number; skills: CatalogSkill[];
};
export type SkillCatalogs = { sources: CatalogSource[]; discovery: { at: number; checked: number; registered: string[]; failures: string[]; note: string | null } | null };
export type CatalogImport = { items: SkillItem[]; imported: string[]; skipped: { path: string; name: string; reason: string }[] };
export type SkillSyncPolicy = { autoOnStart: boolean; trustedScopes: { repo: string; path: string; gitRef?: string | null }[] };
export type SkillSyncReport = {
  checkedAt: number; installed?: string[]; updated: string[]; skipped: string[];
  events: { id: string; status: string; oldCommit: string | null; newCommit: string | null; impactedHarnesses: string[]; detail: string }[];
};
/** A named branch containing slashes needs a separate, exact ref; never guess a path split. */
function githubSelection(input: string, pathInput = '', refInput = ''): { repo: string; path: string; gitRef: string | null } {
  let repo = input.trim();
  let path = pathInput.trim().replace(/^\/+|\/+$/g, '');
  let gitRef = refInput.trim() || null;
  if (repo.startsWith('https://')) {
    let url: URL;
    try { url = new URL(repo); } catch { throw new Error('Enter a valid GitHub URL.'); }
    if (url.hostname !== 'github.com' || url.username || url.password || url.port || url.search || url.hash)
      throw new Error('Only github.com URLs without credentials, query or fragment are supported.');
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length < 2) throw new Error('Enter a GitHub repository URL.');
    repo = [parts[0], parts[1].replace(/\.git$/, '')].join('/');
    if (parts.length > 2) {
      if (!['tree', 'blob'].includes(parts[2]) || parts.length < 4)
        throw new Error('Use a repository URL or tree/ref/path or blob/ref/SKILL.md URL.');
      const ref = gitRef || (parts[3] === 'main' || parts[3] === 'master' || /^[a-fA-F0-9]{40}$/.test(parts[3]) ? parts[3] : '');
      if (!ref) throw new Error('Enter the exact ref separately to disambiguate the GitHub URL.');
      const tail = parts.slice(3).join('/');
      if (tail !== ref && !tail.startsWith(ref + '/')) throw new Error('The URL does not match the explicit ref.');
      const urlPath = tail === ref ? '' : tail.slice(ref.length + 1);
      if (path && path !== urlPath) throw new Error('The URL and path fields refer to different locations.');
      path = urlPath;
      gitRef = ref;
      if (parts[2] === 'blob' && !path.endsWith('/SKILL.md') && path !== 'SKILL.md')
        throw new Error('A skill-file URL must end in SKILL.md.');
    }
  } else if (repo.includes('://')) {
    throw new Error('Only https://github.com URLs are supported.');
  }
  path = path.replace(/\/SKILL\.md$/, '').replace(/^SKILL\.md$/, '');
  if (!/^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/.test(repo) || repo.split('/').some(x => x.startsWith('-') || x.startsWith('.')))
    throw new Error('Enter the repository as owner/name.');
  if (path && path.split('/').some(x => !x || x === '..' || x === '.' || x.startsWith('.')))
    throw new Error('Unsafe GitHub path.');
  if (gitRef && (!/^[a-zA-Z0-9_./-]+$/.test(gitRef) || gitRef.includes('..') || gitRef.startsWith('-')))
    throw new Error('Invalid Git reference.');
  return { repo, path, gitRef };
}

export type SavedSkillSource = {
  id: string; repo: string; path: string; gitRef: string | null;
  commit: string; selectedPaths: string[]; savedAt: number;
};
export type GitHubSourcePreview = {
  repo: string; path: string; gitRef: string | null; commit: string; skills: CatalogSkill[];
};
export type ImportedSkillSource = {
  source: SavedSkillSource; imported: string[];
  skipped: { path: string; name: string; reason: string }[];
};
export type SkillsLibraryHost = {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  /** Projects offered for project scope. */
  projects?(): { id: string; name: string }[];
};

const HARNESS_NAMES: Record<string, string> = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', glm: 'GLM', mimo: 'MiMo', deepseek: 'DeepSeek', commandcode: 'Command Code', cline: 'Cline', kilo: 'Kilo', omp: 'OMP', pi: 'Pi', muse: 'Muse Code', zcode: 'ZCode' };
const harnessName = (id: string) => HARNESS_NAMES[id] ?? id.charAt(0).toUpperCase() + id.slice(1);
const FINDING_LABELS: Record<SkillFinding['rule'], string> = { secret: 'Secret-shaped value', approvals: 'Turns off approvals or the sandbox', 'pipe-to-shell': 'Download piped into a shell' };
const STATUS_LABELS: Record<SkillCell['status'], string> = { active: 'Active', outdated: 'Older version', drifted: 'Edited on disk', missing: 'Missing on disk', prompt: 'Via prompt', off: '—' };
const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? '' : 's'}`;
const message = (error: unknown) => error instanceof Error ? error.message : String(error);
const CATALOGS_HELP = 'Catalogs list skills; nothing is turned on until you choose a harness.';

export function mountSkillsLibrary(host: HTMLElement, options: SkillsLibraryHost): () => void {
  let items: SkillItem[] = [];
  let matrix: SkillMatrix = { project: null, harnesses: [] };
  let catalogs: SkillCatalogs = { sources: [], discovery: null };
  let syncPolicy: SkillSyncPolicy = { autoOnStart: false, trustedScopes: [] };
  let syncReport: SkillSyncReport = { checkedAt: 0, updated: [], skipped: [], events: [] };
  let savedSources: SavedSkillSource[] = [];
  let sourcePreview: GitHubSourcePreview | null = null;
  let sourceQuery = { repo: '', path: '', gitRef: '' };
  let sourceSelected = new Set<string>();
  let pendingSourceRemoval: string | null = null;
  /** Catalog skills chosen for import, as `repo\npath`. */
  let selected = new Set<string>();
  const catalogScopes = new Map<string, string>();
  let view: 'library' | 'import' | 'sync' = 'library';
  let projectId = '';
  let source: 'folder' | 'github' = 'folder';
  let pending: { id: string; harness: string; scope: SkillScope } | null = null;
  let note = '', error = '', busy = false, disposed = false, clearFields = false;
  const projects = () => options.projects?.() ?? [];
  const scopeArgs = (scope: SkillScope) => scope === 'project' ? { projectId } : {};

  async function refresh(): Promise<void> {
    try {
      [items, matrix, catalogs, syncPolicy, syncReport, savedSources] = await Promise.all([
        options.call<SkillItem[]>('skills_list'),
        options.call<SkillMatrix>('skills_matrix', projectId ? { projectId } : {}),
        options.call<SkillCatalogs>('skills_catalogs'),
        options.call<SkillSyncPolicy>('skills_sync_policy'),
        options.call<SkillSyncReport>('skills_sync_report'),
        options.call<SavedSkillSource[]>('skills_source_list'),
      ]);
    } catch (failure) { error = message(failure); }
    if (!disposed) draw();
  }
  async function run(work: () => Promise<string | void>): Promise<void> {
    busy = true; error = ''; note = ''; draw();
    try { note = (await work()) ?? ''; } catch (failure) { error = message(failure); }
    busy = false;
    if (!disposed) await refresh();
  }

  /** Harnesses with toggles: the file-based ones, plus any already used through the prompt. */
  const harnesses = () => {
    const ids = new Set(['claude', 'codex', ...matrix.harnesses.map(row => row.harness)]);
    return [...ids].map(id => matrix.harnesses.find(row => row.harness === id) ?? { harness: id, installed: false, prompt: false, cells: [], unmanaged: [] });
  };
  const scopes = (): SkillScope[] => projectId ? ['user', 'project'] : ['user'];
  const active = (item: SkillItem, harness: string, scope: SkillScope) =>
    item.activations.some(a => a.harness === harness && a.scope === scope && (scope === 'user' || a.project === matrix.project));

  function itemHtml(item: SkillItem): string {
    const from = item.source.kind === 'github' ? `GitHub · ${item.source.location}${item.source.commit ? ` @ ${item.source.commit.slice(0, 7)}` : ''}` : item.source.location;
    const toggles = harnesses().flatMap(row => scopes().map(scope => {
      const label = `${harnessName(row.harness)} · ${scope === 'user' ? 'User' : 'Project'}`;
      return `<label class="skills-toggle"><input type="checkbox" data-harness="${escape(row.harness)}" data-scope="${scope}" ${active(item, row.harness, scope) ? 'checked' : ''} ${busy ? 'disabled' : ''} aria-label="${escape(`${item.name}: ${label}`)}"><span>${escape(label)}</span></label>`;
    })).join('');
    const findings = item.findings.length ? `<ul class="skills-findings" aria-label="${escape(`Safety findings for ${item.name}`)}">${item.findings.map(f =>
      `<li><strong>${escape(FINDING_LABELS[f.rule])}</strong> <span>${escape(f.file)}:${f.line}</span><code>${escape(f.excerpt)}</code></li>`).join('')}</ul>` : '';
    const confirm = pending?.id === item.id ? `<div class="skills-confirm" role="alert"><span>${escape(`${item.name} has ${item.findings.length} safety finding${item.findings.length === 1 ? '' : 's'}. Activate it for ${harnessName(pending.harness)} anyway?`)}</span><button type="button" class="secondary-button" data-action="acknowledge">Activate anyway</button><button type="button" class="secondary-button" data-action="cancel">Cancel</button></div>` : '';
    return `<li class="skills-item" data-id="${escape(item.id)}">
      <div class="skills-item-head"><div><strong>${escape(item.name)}</strong><span class="skills-kind">${item.kind === 'rule' ? 'Rule' : 'Skill'}</span><small>${escape(item.description)}</small><small class="skills-source">${escape(from)}</small></div>
        <button type="button" class="secondary-button" data-action="remove" aria-label="${escape(`Remove ${item.name}`)}" ${busy ? 'disabled' : ''}>Remove</button></div>
      ${findings}${confirm}<div class="skills-toggles">${toggles}</div></li>`;
  }

  function matrixHtml(): string {
    const rows = matrix.harnesses;
    if (!rows.length || !items.length) return '<p class="skills-empty">No installed harness found yet, or nothing in the library.</p>';
    const columns = rows.flatMap(row => scopes().map(scope => ({ row, scope })));
    const head = columns.map(({ row, scope }) => `<th scope="col">${escape(harnessName(row.harness))}<small>${scope === 'user' ? 'User' : 'Project'}${row.prompt ? ' · prompt' : ''}</small></th>`).join('');
    const body = items.map(item => `<tr><th scope="row">${escape(item.name)}</th>${columns.map(({ row, scope }) => {
      const cell = row.cells.find(c => c.id === item.id && c.scope === scope);
      const status = cell?.status ?? 'off';
      return `<td class="skills-status" data-status="${status}" title="${escape(cell?.surface ?? '')}">${STATUS_LABELS[status]}</td>`;
    }).join('')}</tr>`).join('');
    const unmanaged = rows.flatMap(row => row.unmanaged.map(u => `<li><strong>${escape(harnessName(row.harness))}</strong> <code>${escape(u.path)}</code> <span>${escape(u.detail)}</span></li>`)).join('');
    const prompt = rows.some(row => row.prompt) ? '<p class="skills-note">Harnesses marked “prompt” have no skill files; PandaCode adds these items to their messages.</p>' : '';
    return `<div class="skills-matrix-wrap"><table class="skills-matrix" aria-label="Skills by harness"><thead><tr><th scope="col">Item</th>${head}</tr></thead><tbody>${body}</tbody></table></div>${prompt}
      ${unmanaged ? `<h4 class="skills-subheading">Found on disk, not managed by PandaCode</h4><ul class="skills-unmanaged">${unmanaged}</ul>` : ''}`;
  }

  function catalogSkillHtml(source: CatalogSource, skill: CatalogSkill): string {
    const key = `${source.repo}\n${skill.path}`;
    const library = skill.imported ? `<small>${skill.imported === source.commit ? 'In the library' : `In the library @ ${escape(skill.imported.slice(0, 7))}; a newer version is upstream`}</small>` : '';
    const conflict = skill.conflicts.length ? `<small class="skills-conflict">Same name in ${skill.conflicts.map(escape).join(', ')}</small>` : '';
    return `<li class="skills-catalog-skill"><label class="skills-toggle"><input type="checkbox" data-select="${escape(key)}" ${selected.has(key) ? 'checked' : ''} ${busy ? 'disabled' : ''} aria-label="${escape(`Select ${skill.name} from ${source.repo}`)}"><span><strong>${escape(skill.name)}</strong> <code>${escape(skill.path || 'root')}</code> · ${plural(skill.files, 'file')}</span></label>
      <small>${escape(skill.description)}</small>${library}${conflict}</li>`;
  }
  function sourceHtml(source: CatalogSource): string {
    const enabled = source.state === 'enabled';
    const repo = escape(source.repo);
    const tags = [source.origin === 'discovered' ? 'Discovered' : 'Added', source.private ? 'Private' : '', enabled ? '' : 'Disabled'].filter(Boolean).map(tag => `<span class="skills-kind">${tag}</span>`).join('');
    const at = source.commit ? ` @ ${escape(source.commit.slice(0, 7))}${source.gitRef ? ` (${escape(source.gitRef)})` : ''}` : '';
    const button = (action: string, label: string, extra = '', name = `${label} ${source.repo}`) =>
      `<button type="button" class="secondary-button" data-action="${action}" data-repo="${repo}" ${extra} aria-label="${escape(name)}" ${busy ? 'disabled' : ''}>${label}</button>`;
    const scope = catalogScopes.get(source.repo) || '';
    const visibleSkills = source.skills.filter(skill => !scope || skill.path === scope || skill.path.startsWith(scope + '/'));
    const body = enabled ? `<p class="skills-note">${scope ? `Browsing ${escape(scope)} · ${plural(visibleSkills.length, 'skill')}` : 'Browsing all indexed skills.'}</p><ul class="skills-catalog-skills" aria-label="${escape(`Skills in ${source.repo}`)}">${visibleSkills.map(skill => catalogSkillHtml(source, skill)).join('')}</ul>
      <div class="skills-confirm">${button('catalog-import', 'Import selected', '', `Import selected from ${source.repo}`)}${button('catalog-import-all', 'Import all', '', `Import all from ${source.repo}`)}</div>` : '';
    return `<li class="skills-item skills-catalog" data-repo="${repo}">
      <div class="skills-item-head"><div><strong>${repo}</strong>${tags}<small>${plural(source.skills.length, 'skill')}${at}</small></div>
        <div class="skills-confirm">${button('catalog-refresh', 'Refresh')}${enabled ? button('catalog-state', 'Disable', 'data-state="disabled"') : button('catalog-state', 'Enable', 'data-state="enabled"')}${button('catalog-state', 'Ignore', 'data-state="ignored"')}</div></div>
      ${body}</li>`;
  }
  function catalogsHtml(): string {
    const shown = catalogs.sources.filter(s => s.state !== 'ignored');
    const ignored = catalogs.sources.filter(s => s.state === 'ignored');
    const found = catalogs.discovery;
    const summary = found ? `<p class="skills-note">Checked ${found.checked} ${found.checked === 1 ? 'repository' : 'repositories'} you own.${found.failures.length ? ` Could not read ${found.failures.map(escape).join('; ')}.` : ''}${found.note ? ` ${escape(found.note)}` : ''}</p>` : '';
    return `<div class="settings-group">
      <form class="skills-import skills-catalog-add" aria-label="Add a skill catalog" novalidate>
        <div class="skills-import-row"><input name="catalogRepo" aria-label="Catalog repository" placeholder="owner/name or github.com/owner/repo/tree/main/skills" spellcheck="false" autocomplete="off"><input name="catalogRef" aria-label="Catalog branch, tag or commit" placeholder="Default branch" spellcheck="false" autocomplete="off"><button type="submit" class="secondary-button" ${busy ? 'disabled' : ''}>Scan</button></div>
        <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="discover" ${busy ? 'disabled' : ''}>Find my skill repositories</button></div>
      </form>
      ${shown.length ? '' : '<p class="skills-empty">No catalogs yet. Scan a repository, or find the skill repositories in your GitHub account.</p>'}
    </div>${summary}
    ${shown.length ? `<ul class="settings-group skills-list" aria-label="Catalog sources">${shown.map(sourceHtml).join('')}</ul>` : ''}
    ${ignored.length ? `<ul class="skills-unmanaged" aria-label="Ignored sources">${ignored.map(s => `<li><code>${escape(s.repo)}</code> <button type="button" class="secondary-button" data-action="catalog-state" data-state="enabled" data-repo="${escape(s.repo)}" aria-label="${escape(`Stop ignoring ${s.repo}`)}" ${busy ? 'disabled' : ''}>Stop ignoring</button></li>`).join('')}</ul>` : ''}`;
  }


  function sourceImportHtml(): string {
    const previews = sourcePreview?.skills ?? [];
    const previewsHtml = previews.map(skill => `<li class="skills-catalog-skill">
      <label class="skills-toggle"><input type="checkbox" data-source-select="${escape(skill.path)}"
        aria-label="${escape(`Select ${skill.name} from ${sourcePreview!.repo}`)}" ${sourceSelected.has(skill.path) ? 'checked' : ''} ${busy ? 'disabled' : ''}>
        <span><strong>${escape(skill.name)}</strong> <code>${escape(skill.path || '(root)')}</code></span></label>
      <small>${escape(skill.description)}</small>
      <small>${plural(skill.files, 'file')} · ${Math.ceil(skill.bytes / 1024)} KiB</small>
    </li>`).join('');
    const cards = savedSources.map(source => {
      const link = `https://github.com/${source.repo}/tree/${source.commit}${source.path ? '/' + source.path : ''}`;
      const removing = pendingSourceRemoval === source.id;
      return `<li class="skills-item" data-source-id="${escape(source.id)}">
        <div class="skills-item-head"><div>
          <strong>${escape(source.repo)}</strong><small><code>${escape(source.path || '(repository root)')}</code></small>
          <small>Ref: ${escape(source.gitRef || 'default branch')} · ${plural(source.selectedPaths.length, 'selected skill')} · ${escape(source.commit.slice(0, 12))}</small>
          <small><a href="${escape(link)}" target="_blank" rel="noopener noreferrer">View saved GitHub path</a></small>
        </div></div>
        <div class="skills-import-row skills-source-actions">
          <button type="button" class="secondary-button" data-action="source-edit" ${busy ? 'disabled' : ''}>Edit selection</button>
          <button type="button" class="secondary-button" data-action="${removing ? 'source-confirm-remove' : 'source-remove'}" ${busy ? 'disabled' : ''}>${removing ? 'Confirm removal' : 'Remove source config'}</button>
          ${removing ? '<button type="button" class="secondary-button" data-action="source-cancel-remove">Cancel</button>' : ''}
        </div>
        ${removing ? '<small>Removing the configuration does not delete imported skills or native harness files.</small>' : ''}
      </li>`;
    }).join('');
    return `<div class="settings-group skills-source-controls">
      <p class="skills-note">Enter a GitHub repository and its skills folder. PandaCode uses the signed-in GitHub CLI to list available skills without cloning or storing unselected repositories. Importing does not activate skills or run scripts.</p>
      <form class="skills-source-form" aria-label="Scan a GitHub skills source" novalidate>
        <label>GitHub repository URL or owner/name
          <input name="sourceRepo" aria-label="Source repository" value="${escape(sourceQuery.repo)}" placeholder="owner/repo" spellcheck="false" autocomplete="off" required>
        </label>
        <label>Skills folder path (optional for repository root)
          <input name="sourcePath" aria-label="Skills folder path" value="${escape(sourceQuery.path)}" placeholder="skills" spellcheck="false" autocomplete="off">
        </label>
        <label>Git ref (branch, tag, or full commit SHA)
          <input name="sourceRef" aria-label="Source Git ref" value="${escape(sourceQuery.gitRef)}" placeholder="Default branch" spellcheck="false" autocomplete="off">
        </label>
        <div class="skills-import-row skills-source-actions">
          <button type="submit" class="secondary-button" ${busy ? 'disabled' : ''}>List skills</button>
        </div>
      </form>
      <p class="skills-message" role="status">${escape(busy ? 'Working…' : note)}</p>
      <p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p>
    </div>
    ${sourcePreview ? `<h3 class="settings-heading">Available skills</h3>
      <section class="settings-group skills-source-results" aria-label="Available GitHub skills">
        <div class="skills-source-summary"><span>${plural(previews.length, 'skill')} at ${escape(sourcePreview.repo)}/${escape(sourcePreview.path || '(root)')} · ${escape(sourcePreview.commit.slice(0, 12))}</span>
          <strong>${plural(sourceSelected.size, 'selected')}</strong></div>
        <div class="skills-import-row skills-source-actions">
          <button type="button" class="secondary-button" data-action="source-select-all" ${busy ? 'disabled' : ''}>Select all</button>
          <button type="button" class="secondary-button" data-action="source-clear" ${busy ? 'disabled' : ''}>Clear selection</button>
        </div>
        <ul class="skills-catalog-skills" aria-label="Select GitHub skills">${previewsHtml}</ul>
        <div class="skills-import-row skills-source-actions">
          <button type="button" class="secondary-button" data-action="source-save-import" ${!sourceSelected.size || busy ? 'disabled' : ''}>Import selected and save source</button>
        </div>
      </section>` : ''}
    <h3 class="settings-heading">Saved sources</h3>
    ${savedSources.length ? `<ul class="settings-group skills-list" aria-label="Saved GitHub skill sources">${cards}</ul>` :
    '<div class="settings-group skills-empty-group"><p class="skills-empty">No saved GitHub skill sources. Scan, select and import a collection to save it.</p></div>'}`;
  }

  function syncHtml(): string {
    const githubItems = items.filter(item => item.kind === 'skill' && item.source.kind === 'github');
    const sourceHtml = savedSources.map(source => {
      const link = `https://github.com/${source.repo}/tree/${source.commit}${source.path ? '/' + source.path : ''}`;
      return `<li class="skills-item" data-source-id="${escape(source.id)}"><strong>${escape(source.repo)}</strong>
        <small>${escape(source.path || '(repository root)')} · ${plural(source.selectedPaths.length, 'selected skill')} · ${escape(source.gitRef || 'default branch')}</small>
        <div class="skills-import-row skills-source-actions">
          <a href="${escape(link)}" target="_blank" rel="noopener noreferrer">View GitHub path</a>
          <button type="button" class="secondary-button" data-action="sync-saved-source" ${busy ? 'disabled' : ''}>Sync selected skills</button>
        </div></li>`;
    }).join('');
    const entries = githubItems.map(item => {
      const report = syncReport.events.find(event => event.id === item.id);
      const reference = item.source.gitRef || '';
      const refName = `Git ref for ${item.name}`;
      return `<li class="skills-item" data-id="${escape(item.id)}"><strong>${escape(item.name)}</strong>
        <small>${escape(item.source.location)} @ ${escape(item.source.commit?.slice(0, 12) || 'unknown')}${item.autoAvailable ? ' · Auto-available on compatible bot turns' : ''}</small>
        ${item.requiredSkills?.length ? `<small>Required library peers: ${escape(item.requiredSkills.join(', '))}</small>` : ''}
        <div class="skills-import-row"><label>${escape(refName)}<input data-ref-input aria-label="${escape(refName)}" value="${escape(reference)}" placeholder="Default branch" spellcheck="false"></label>
          <button type="button" class="secondary-button" data-action="set-ref" ${busy ? 'disabled' : ''}>Save ref</button></div>
        <label>Startup updates for ${escape(item.name)}
          <select data-skill-auto="${escape(item.id)}" aria-label="${escape(`Startup updates for ${item.name}`)}" ${busy ? 'disabled' : ''}>
            <option value="inherit" ${item.autoUpdate === null || item.autoUpdate === undefined ? 'selected' : ''}>Inherit global</option>
            <option value="yes" ${item.autoUpdate === true ? 'selected' : ''}>Enabled (when global sync is on)</option>
            <option value="no" ${item.autoUpdate === false ? 'selected' : ''}>Disabled</option>
          </select></label>
        <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="sync-item" ${busy ? 'disabled' : ''}>Sync this skill</button></div>
        ${report ? `<small>${escape(report.status)} · ${escape(report.detail)}${report.newCommit ? ` · ${escape(report.newCommit.slice(0, 12))}` : ''}</small>` : ''}</li>`;
    }).join('');
    return `<div class="settings-group skills-sync-controls">
      <p class="skills-note">Synchronization updates only skills already imported into the PandaCode library. Startup synchronization is off by default and never installs newly discovered skills. Full commit-SHA pins do not move automatically. Updates replace edited library copies without backups; native harness files remain unchanged.</p>
      <label class="skills-toggle"><input type="checkbox" data-global-sync ${syncPolicy.autoOnStart ? 'checked' : ''} ${busy ? 'disabled' : ''}><span>Automatically synchronize imported skills when PandaCode starts</span></label>
      <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="sync-all" ${busy ? 'disabled' : ''}>Sync all installed skills</button></div>
      <p class="skills-message" role="status">${escape(busy ? 'Working…' : note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p>
      ${syncReport.checkedAt ? `<small>Last synchronization report: ${escape(new Date(syncReport.checkedAt).toLocaleString())} · ${plural(syncReport.installed?.length ?? 0, 'new skill')} installed · ${plural(syncReport.updated.length, 'skill')} updated</small>` : ''}
    </div><h3 class="settings-heading">Sources</h3>
    ${savedSources.length ? `<ul class="settings-group skills-list" aria-label="Synchronization sources">${sourceHtml}</ul>` : '<div class="settings-group skills-empty-group"><p class="skills-empty">No saved GitHub skill sources.</p></div>'}
    <h3 class="settings-heading">Installed skills</h3>
    ${githubItems.length ? `<ul class="settings-group skills-list" aria-label="Skill synchronization">${entries}</ul>` : '<div class="settings-group skills-empty-group"><p class="skills-empty">No GitHub skills installed yet.</p></div>'}`;
  }

  function draw(): void {
    const list = projects();
    const values = clearFields ? {} : Object.fromEntries([...host.querySelectorAll<HTMLInputElement | HTMLSelectElement>('form [name]')].map(field => [field.name, field.value]));
    host.innerHTML = `<nav class="ds-seg skills-view-tabs" aria-label="Skills Library pages">
      <button type="button" data-action="view" data-view="library" aria-current="${view === 'library' ? 'page' : 'false'}" aria-pressed="${view === 'library' ? 'true' : 'false'}">Library &amp; activation</button>
      <button type="button" data-action="view" data-view="import" aria-current="${view === 'import' ? 'page' : 'false'}" aria-pressed="${view === 'import' ? 'true' : 'false'}">Import from GitHub</button>
      <button type="button" data-action="view" data-view="sync" aria-current="${view === 'sync' ? 'page' : 'false'}" aria-pressed="${view === 'sync' ? 'true' : 'false'}">Synchronization</button>
    </nav>${view === 'import' ? sourceImportHtml() : view === 'sync' ? syncHtml() : `<h3 class="settings-heading">Import</h3><div class="settings-group skills-library" aria-label="Skills and rules">
      <form class="skills-import" aria-label="Import a skill or rule" novalidate>
        <div class="skills-import-row">
          <select name="kind" aria-label="Kind"><option value="skill">Skill</option><option value="rule">Rule</option></select>
          <select name="source" aria-label="Source"><option value="folder" ${source === 'folder' ? 'selected' : ''}>Local folder or file</option><option value="github" ${source === 'github' ? 'selected' : ''}>GitHub repository</option></select>
          ${list.length ? `<select name="project" aria-label="Project for project scope"><option value="">User scope only</option>${list.map(p => `<option value="${escape(p.id)}" ${p.id === projectId ? 'selected' : ''}>${escape(p.name)}</option>`).join('')}</select>` : ''}
        </div>
        ${source === 'folder'
          ? '<input name="path" aria-label="Folder or file path" placeholder="/path/to/skill-folder or rule.md" spellcheck="false" autocomplete="off">'
          : '<div class="skills-import-row"><input name="repo" aria-label="Repository" placeholder="owner/name" spellcheck="false" autocomplete="off"><input name="repoPath" aria-label="Path in repository" placeholder="skills/pdf" spellcheck="false" autocomplete="off"><input name="ref" aria-label="Branch, tag or commit" placeholder="Default branch" spellcheck="false" autocomplete="off"></div>'}
        <div class="skills-import-row skills-import-actions"><p class="skills-message" role="status">${escape(busy ? 'Working…' : note)}</p><button type="submit" class="secondary-button" ${busy ? 'disabled' : ''}>Import</button></div>
        <p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p>
      </form>
    </div>
    <div class="settings-heading"><h3>Catalogs</h3>${infoMark(CATALOGS_HELP, 'About catalogs')}<span class="sr-only">${CATALOGS_HELP}</span></div>${catalogsHtml()}
    <h3 class="settings-heading">Library</h3>
    ${items.length ? `<ul class="settings-group skills-list" aria-label="Library">${items.map(itemHtml).join('')}</ul>` : '<p class="skills-empty">The library is empty. Import a skill folder (with a SKILL.md) or a rule (one Markdown file).</p>'}
    <h3 class="settings-heading">By harness</h3>${matrixHtml()}`}`;
    for (const [name, value] of Object.entries(values)) {
      const field = host.querySelector<HTMLInputElement | HTMLSelectElement>(`form [name="${name}"]`);
      if (field && name !== 'source' && name !== 'project') field.value = value;
    }
    clearFields = false;
  }

  function toggle(input: HTMLInputElement): void {
    const id = input.closest<HTMLElement>('.skills-item')!.dataset.id!;
    const { harness, scope } = input.dataset as { harness: string; scope: SkillScope };
    const item = items.find(entry => entry.id === id)!;
    if (!input.checked) {
      void run(async () => {
        const change = await options.call<{ kept: string[] }>('skills_deactivate', { id, harness, scope, ...scopeArgs(scope) });
        return change.kept.length ? `Left ${change.kept.length} edited file${change.kept.length === 1 ? '' : 's'} in place: ${change.kept.join(', ')}` : `${item.name} is off for ${harnessName(harness)}.`;
      });
      return;
    }
    if (item.findings.length && !item.acknowledged) {
      input.checked = false; pending = { id, harness, scope }; draw(); return;
    }
    void activate(id, harness, scope, false);
  }
  function activate(id: string, harness: string, scope: SkillScope, acknowledge: boolean): Promise<void> {
    pending = null;
    return run(async () => {
      await options.call('skills_activate', { id, harness, scope, ...scopeArgs(scope), acknowledge });
      const row = matrix.harnesses.find(r => r.harness === harness);
      return row?.prompt ? `${harnessName(harness)} has no skill files; PandaCode adds this to its messages.` : `Active for ${harnessName(harness)}.`;
    });
  }
  function importItem(form: HTMLFormElement): void {
    const data = new FormData(form);
    const kind = String(data.get('kind') ?? 'skill') as SkillKind;
    const text = (name: string) => String(data.get(name) ?? '').trim();
    if (source === 'folder' && !text('path')) { error = 'Enter a folder or file path.'; draw(); return; }
    if (source === 'github' && !text('repo')) { error = 'Enter the repository as owner/name.'; draw(); return; }
    let selection: ReturnType<typeof githubSelection> | null = null;
    if (source === 'github') {
      try { selection = githubSelection(text('repo'), text('repoPath'), text('ref')); }
      catch (cause) { error = message(cause); draw(); return; }
    }
    const github = selection;
    void run(async () => {
      const indexCollection = github && kind === 'skill' && (
        text('repo').startsWith('https://') || !github.path || github.path === 'skills');
      if (github && indexCollection) {
        const catalog = await options.call<SkillCatalogs>('skills_catalog_add', { repo: github.repo, gitRef: github.gitRef });
        catalogs = catalog;
        const sourceIndex = catalog.sources.find(item => item.repo.toLowerCase() === github.repo.toLowerCase());
        const exact = sourceIndex?.skills.find(skill => skill.path === github.path);
        clearFields = true;
        if (exact) {
          const imported = await options.call<CatalogImport>('skills_catalog_import', { repo: github.repo, paths: [exact.path] });
          return `Imported ${plural(imported.imported.length, 'skill')}.${imported.skipped.length ? ' ' + imported.skipped.map(item => item.reason).join('; ') : ''}`;
        }
        catalogScopes.set(github.repo, github.path);
        const inScope = sourceIndex?.skills.filter(skill => !github.path || skill.path.startsWith(github.path + '/')) ?? [];
        if (!inScope.length) throw new Error(`No valid SKILL.md packages under ${github.repo}/${github.path}.`);
        return `Found ${plural(inScope.length, 'skill')} in ${github.repo}/${github.path}. Use catalog selection or Import all.`;
      }
      const item = source === 'folder'
        ? await options.call<SkillItem>('skills_import_folder', { path: text('path'), kind })
        : await options.call<SkillItem>('skills_import_github', { repo: github!.repo, path: github!.path, gitRef: github!.gitRef, kind });
      clearFields = true;
      return `Imported ${item.name}.${item.findings.length ? ` ${item.findings.length} safety finding${item.findings.length === 1 ? '' : 's'} to review.` : ''}`;
    });
  }

  function addCatalog(form: HTMLFormElement): void {
    const data = new FormData(form);
    const raw = String(data.get('catalogRepo') ?? '').trim();
    if (!raw) { error = 'Enter the repository as owner/name.'; draw(); return; }
    let selection: ReturnType<typeof githubSelection>;
    try { selection = githubSelection(raw, '', String(data.get('catalogRef') ?? '')); }
    catch (cause) { error = message(cause); draw(); return; }
    void run(async () => {
      catalogs = await options.call<SkillCatalogs>('skills_catalog_add', { repo: selection.repo, gitRef: selection.gitRef });
      catalogScopes.set(selection.repo, selection.path);
      clearFields = true;
      const source = catalogs.sources.find(s => s.repo.toLowerCase() === selection.repo.toLowerCase());
      const inScope = source?.skills.filter(skill => !selection.path || skill.path === selection.path || skill.path.startsWith(selection.path + '/')) ?? [];
      return `Found ${plural(inScope.length, 'skill')} in ${selection.repo}${selection.path ? '/' + selection.path : ''}. Select packages below to import.`;
    });
  }

  function scanSource(form: HTMLFormElement): void {
    const formData = new FormData(form);
    const input = (name: string) => String(formData.get(name) || '').trim();
    let query: ReturnType<typeof githubSelection>;
    try {
      query = githubSelection(input('sourceRepo'), input('sourcePath'), input('sourceRef'));
    } catch (cause) { error = message(cause); draw(); return; }
    sourceQuery = { repo: query.repo, path: query.path, gitRef: query.gitRef || '' };
    sourcePreview = null;
    sourceSelected = new Set();
    void run(async () => {
      const preview = await options.call<GitHubSourcePreview>('skills_source_preview', {
        repo: query.repo, path: query.path, gitRef: query.gitRef,
      });
      sourcePreview = preview;
      const saved = savedSources.find(source => source.repo.toLowerCase() === preview.repo.toLowerCase() && source.path === preview.path);
      sourceSelected = new Set((saved?.selectedPaths || []).filter(path => preview.skills.some(skill => skill.path === path)));
      return `Found ${plural(preview.skills.length, 'skill')} in ${preview.repo}/${preview.path || '(root)'}. Select the ones to import.`;
    });
  }

  function catalogAction(action: string, repo: string, state?: string): void {
    if (action === 'catalog-refresh') void run(async () => { await options.call('skills_catalog_refresh', { repo }); return `Refreshed ${repo}. Library copies stay as imported.`; });
    if (action === 'catalog-state' && state) void run(async () => {
      await options.call('skills_catalog_state', { repo, state });
      return state === 'ignored' ? `Ignoring ${repo}. Discovery will not add it again.` : state === 'disabled' ? `${repo} is disabled.` : `${repo} is enabled.`;
    });
    if (action === 'catalog-import' || action === 'catalog-import-all') {
      const prefix = `${repo}\n`;
      const scope = catalogScopes.get(repo) || '';
      const scoped = (path: string) => !scope || path === scope || path.startsWith(scope + '/');
      const paths = action === 'catalog-import'
        ? [...selected].filter(key => key.startsWith(prefix)).map(key => key.slice(prefix.length)).filter(scoped)
        : scope ? catalogs.sources.find(source => source.repo === repo)?.skills.filter(item => scoped(item.path)).map(item => item.path) ?? [] : null;
      if (paths && !paths.length) { error = 'Select skills to import, or choose Import all.'; draw(); return; }
      void run(async () => {
        const result = await options.call<CatalogImport>('skills_catalog_import', { repo, paths });
        selected = new Set([...selected].filter(key => !key.startsWith(prefix)));
        const skipped = result.skipped.length ? ` Skipped ${result.skipped.length}: ${result.skipped.map(s => s.reason).join(' ')}` : '';
        return `Imported ${plural(result.imported.length, 'skill')}.${skipped} Nothing is active until you turn it on.`;
      });
    }
  }

  const onClick = (event: Event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-action]');
    if (!button) return;
    if (button.dataset.action === 'view') {
      view = button.dataset.view as typeof view; note = ''; error = ''; draw(); return;
    }
    if (button.dataset.action === 'source-select-all' && sourcePreview) {
      sourceSelected = new Set(sourcePreview.skills.map(skill => skill.path));
      draw(); return;
    }
    if (button.dataset.action === 'source-clear') {
      sourceSelected = new Set(); draw(); return;
    }
    if (button.dataset.action === 'source-save-import' && sourcePreview) {
      const preview = sourcePreview;
      const paths = [...sourceSelected];
      if (!paths.length) { error = 'Select at least one skill before importing.'; draw(); return; }
      void run(async () => {
        const result = await options.call<ImportedSkillSource>('skills_source_import', {
          repo: preview.repo, path: preview.path, gitRef: preview.gitRef,
          commit: preview.commit, selectedPaths: paths,
        });
        sourceSelected = new Set(result.source.selectedPaths);
        return `Imported ${plural(result.imported.length, 'skill')} and saved ${result.source.repo}/${result.source.path || '(root)'}.${result.skipped.length ? ` Skipped: ${result.skipped.map(s => s.reason).join('; ')}` : ''} Activation is separate.`;
      }); return;
    }
    const sourceId = button.closest<HTMLElement>('[data-source-id]')?.dataset.sourceId;
    if (button.dataset.action === 'sync-saved-source' && sourceId) {
      void run(async () => {
        const report = await options.call<SkillSyncReport>('skills_sync_saved_source', { id: sourceId });
        return `Updated ${plural(report.updated.length, 'skill')} from saved source.${report.skipped.length ? ` Skipped: ${report.skipped.join('; ')}` : ''}`;
      }); return;
    }
    if (button.dataset.action === 'source-edit' && sourceId) {
      const saved = savedSources.find(s => s.id === sourceId);
      if (!saved) return;
      sourceQuery = { repo: saved.repo, path: saved.path, gitRef: saved.gitRef || '' };
      pendingSourceRemoval = null;
      void run(async () => {
        sourcePreview = await options.call<GitHubSourcePreview>('skills_source_preview', sourceQuery);
        sourceSelected = new Set(saved.selectedPaths.filter(path => sourcePreview!.skills.some(skill => skill.path === path)));
        return `Editing selection for ${saved.repo}/${saved.path || '(root)'}.`;
      }); return;
    }
    if (button.dataset.action === 'source-remove' && sourceId) {
      pendingSourceRemoval = sourceId; draw(); return;
    }
    if (button.dataset.action === 'source-cancel-remove') {
      pendingSourceRemoval = null; draw(); return;
    }
    if (button.dataset.action === 'source-confirm-remove' && sourceId && pendingSourceRemoval === sourceId) {
      void run(async () => {
        await options.call('skills_source_remove', { id: sourceId });
        pendingSourceRemoval = null;
        return 'Saved configuration removed. Previously imported library skills remain installed.';
      }); return;
    }
    if (button.dataset.action === 'sync-item' || button.dataset.action === 'sync-all') {
      const id = button.closest<HTMLElement>('[data-id]')?.dataset.id;
      if (button.dataset.action === 'sync-item' && !id) return;
      void run(async () => {
        const report = await options.call<{ updated: string[]; skipped: string[] }>(
          button.dataset.action === 'sync-item' ? 'skills_sync_item' : 'skills_sync_all', id ? { id } : {});
        return `Updated ${plural(report.updated.length, 'skill')}.${report.skipped.length ? ` Skipped: ${report.skipped.join('; ')}` : ''}`;
      });
      return;
    }
    if (button.dataset.action === 'set-ref') {
      const row = button.closest<HTMLElement>('[data-id]');
      const id = row?.dataset.id;
      if (!id) return;
      const gitRef = row.querySelector<HTMLInputElement>('[data-ref-input]')?.value.trim() || null;
      void run(async () => {
        await options.call('skills_sync_set_ref', { id, gitRef });
        return `Saved ref for ${id}. Run Sync this skill to fetch the selected revision.`;
      }); return;
    }
    if (button.dataset.action === 'discover') {
      void run(async () => {
        const found = (await options.call<SkillCatalogs>('skills_discover')).discovery;
        return found?.registered.length ? `Added ${found.registered.join(', ')}.` : 'No new skill repositories found.';
      });
      return;
    }
    if (button.dataset.repo) { catalogAction(button.dataset.action!, button.dataset.repo, button.dataset.state); return; }
    const id = button.closest<HTMLElement>('.skills-item')?.dataset.id;
    if (button.dataset.action === 'remove' && id) void run(async () => { await options.call('skills_remove', { id }); return 'Removed.'; });
    if (button.dataset.action === 'acknowledge' && pending) void activate(pending.id, pending.harness, pending.scope, true);
    if (button.dataset.action === 'cancel') { pending = null; draw(); }
  };
  const onChange = (event: Event) => {
    const target = event.target as HTMLElement;
    if (target instanceof HTMLInputElement && target.dataset.harness) toggle(target);
    if (target instanceof HTMLInputElement && target.hasAttribute('data-global-sync')) {
      void run(async () => { await options.call('skills_sync_set_global', { enabled: target.checked }); return target.checked ? 'Automatic synchronization enabled for imported skills.' : 'Automatic synchronization disabled.'; });
    }
    if (target instanceof HTMLSelectElement && target.dataset.skillAuto) {
      const id = target.dataset.skillAuto;
      const choice = target.value === 'inherit' ? null : target.value === 'yes';
      void run(async () => { await options.call('skills_sync_override', { id, autoUpdate: choice }); return `Updated startup policy for ${id}.`; });
    }
    if (target instanceof HTMLInputElement && target.closest('.skills-source-form')) {
      const form = target.closest<HTMLFormElement>('form')!;
      sourceQuery = {
        repo: form.querySelector<HTMLInputElement>('[name="sourceRepo"]')?.value || '',
        path: form.querySelector<HTMLInputElement>('[name="sourcePath"]')?.value || '',
        gitRef: form.querySelector<HTMLInputElement>('[name="sourceRef"]')?.value || '',
      };
      if (sourcePreview) {
        // Invalidate the previous selection without replacing the form during blur: doing
        // so would cancel a pending click on "List skills" in the same user action.
        sourcePreview = null;
        sourceSelected = new Set();
        const results = host.querySelector('.skills-source-results');
        if (results?.previousElementSibling?.matches('.settings-heading')) results.previousElementSibling.remove();
        results?.remove();
      }
    }
    if (target instanceof HTMLInputElement && target.hasAttribute('data-source-select')) {
      const path = target.dataset.sourceSelect!;
      if (target.checked) sourceSelected.add(path); else sourceSelected.delete(path);
      draw();
    }
    if (target instanceof HTMLInputElement && target.dataset.select) {
      const key = target.dataset.select;
      selected = target.checked ? new Set([...selected, key]) : new Set([...selected].filter(k => k !== key));
    }
    if (target instanceof HTMLSelectElement && target.name === 'source') { source = target.value as typeof source; draw(); }
    if (target instanceof HTMLSelectElement && target.name === 'project') { projectId = target.value; pending = null; void refresh(); }
  };
  const onSubmit = (event: Event) => {
    event.preventDefault();
    const form = event.target as HTMLFormElement;
    if (form.classList.contains('skills-source-form')) scanSource(form);
    else if (form.classList.contains('skills-catalog-add')) addCatalog(form); else importItem(form);
  };
  host.addEventListener('click', onClick);
  host.addEventListener('change', onChange);
  host.addEventListener('submit', onSubmit);
  draw();
  void refresh();
  return () => {
    disposed = true;
    host.removeEventListener('click', onClick);
    host.removeEventListener('change', onChange);
    host.removeEventListener('submit', onSubmit);
  };
}
