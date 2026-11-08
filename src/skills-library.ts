/* Settings → Skills & rules (issues #15 and #45): the library, imports, where each item is
 * active per harness and scope, the drift matrix and safety findings. Everything is native
 * (`skills_*` commands in src-tauri/src/skills.rs); this view only draws and asks. Catalogs
 * (#114, #115) index GitHub repositories of skills; importing from one never activates. */
import './skills-library.css';
import { escape } from './dom';
import { infoMark } from './tooltip';
import recommendations from './recommended-skills.v1.json';

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
  /** Catalog skills chosen for import, as `repo\npath`. */
  let selected = new Set<string>();
  let recommendedSelected = new Set<string>();
  const catalogScopes = new Map<string, string>();
  let view: 'library' | 'recommended' | 'sync' = 'library';
  let projectId = '';
  let source: 'folder' | 'github' = 'folder';
  let pending: { id: string; harness: string; scope: SkillScope } | null = null;
  let note = '', error = '', busy = false, disposed = false, clearFields = false;
  const projects = () => options.projects?.() ?? [];
  const scopeArgs = (scope: SkillScope) => scope === 'project' ? { projectId } : {};

  async function refresh(): Promise<void> {
    try {
      [items, matrix, catalogs, syncPolicy, syncReport] = await Promise.all([
        options.call<SkillItem[]>('skills_list'),
        options.call<SkillMatrix>('skills_matrix', projectId ? { projectId } : {}),
        options.call<SkillCatalogs>('skills_catalogs'),
        options.call<SkillSyncPolicy>('skills_sync_policy'),
        options.call<SkillSyncReport>('skills_sync_report'),
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


  function recommendedHtml(): string {
    const groups = [...new Set(recommendations.templates.map(entry => entry.group))];
    const source = catalogs.sources.find(entry => entry.repo === 'ginmp8/rhapsodia');
    const groupHtml = groups.map(group => {
      const entries = recommendations.templates.filter(entry => entry.group === group);
      return `<h3 class="settings-heading">${escape(group)}</h3><section class="settings-group skills-recommended-group">
        <ul class="skills-catalog-skills">${entries.map(entry => {
          const path = `ginmp8/rhapsodia/${entry.path}`;
          const installed = items.find(item => item.source.location.toLowerCase() === path.toLowerCase());
          const newest = source?.skills.find(skill => skill.path === entry.path);
          const status = !entry.available ? 'Unavailable in the current RhapsodIA source' : installed
            ? (newest && newest.imported !== source?.commit ? 'An update may be available' : 'Installed') : 'Not installed';
          return `<li class="skills-catalog-skill"><label class="skills-toggle">
            <input type="checkbox" data-recommended="${escape(entry.id)}" ${recommendedSelected.has(entry.id) ? 'checked' : ''} ${!entry.available || busy ? 'disabled' : ''}
              aria-label="${escape(`Select recommended ${entry.id}`)}">
            <span><strong>${escape(entry.id)}</strong> ${entry.optional ? '<span class="skills-kind">Optional</span>' : ''}<code>${escape(entry.path)}</code></span></label>
            <small>${escape(status)}</small></li>`;
        }).join('')}</ul></section>`;
    }).join('');
    return `<div class="settings-group skills-recommended-controls"><p class="skills-note">These are metadata references to public RhapsodIA skills, not preinstalled packages. Import requires the GitHub CLI. No scripts run on import and no bot is activated automatically.</p>
      <label>Git ref (branch, tag, or immutable full commit SHA)
        <input name="recommendedRef" aria-label="Recommended source ref" value="main" spellcheck="false" autocomplete="off">
      </label><div class="skills-import-row skills-import-actions">
        <button type="button" class="secondary-button" data-action="recommend-import" ${busy ? 'disabled' : ''}>Install selected</button>
        <button type="button" class="secondary-button" data-action="recommend-all" ${busy ? 'disabled' : ''}>Install all primary skills</button>
      </div><p class="skills-message" role="status">${escape(busy ? 'Working…' : note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p></div>
      ${groupHtml}`;
  }

  function syncHtml(): string {
    const githubItems = items.filter(item => item.kind === 'skill' && item.source.kind === 'github');
    const sources = [...new Set([
      ...githubItems.map(item => item.source.location.split('/').slice(0, 2).join('/')),
      ...syncPolicy.trustedScopes.map(scope => scope.repo),
    ])].sort();
    const sourceHtml = sources.map(repo => {
      const scopes = syncPolicy.trustedScopes.filter(scope => scope.repo.toLowerCase() === repo.toLowerCase());
      const rootTrusted = scopes.some(scope => !scope.path);
      const partial = scopes.some(scope => !!scope.path);
      const ref = catalogs.sources.find(source => source.repo.toLowerCase() === repo.toLowerCase())?.gitRef
        || githubItems.find(item => item.source.location.startsWith(repo + '/'))?.source.gitRef || '';
      const scopeLines = scopes.map(scope => `<li><code>${escape(scope.path || '(repository root)')}</code> @ ${escape(scope.gitRef || 'default branch')}
        <button type="button" class="secondary-button" data-action="revoke-path" data-repo="${escape(repo)}" data-path="${escape(scope.path)}" ${busy ? 'disabled' : ''}>Revoke collection trust</button></li>`).join('');
      return `<li class="skills-item" data-repo="${escape(repo)}"><strong>${escape(repo)}</strong>
        <small>${rootTrusted ? 'Repository root trusted' : partial ? 'Only selected collection paths trusted' : 'No startup trust'}</small>
        <div class="skills-import-row skills-import-actions">
          <button type="button" class="secondary-button" data-action="sync-source" data-repo="${escape(repo)}" ${busy ? 'disabled' : ''}>Sync this source</button>
          <button type="button" class="secondary-button" data-action="trust-source" data-repo="${escape(repo)}" data-git-ref="${escape(ref)}" data-trusted="${rootTrusted ? 'false' : 'true'}" ${busy ? 'disabled' : ''}>${rootTrusted ? 'Revoke root trust' : 'Trust repository root'}</button>
        </div>
        <label>Authorize a collection path (empty means root)
          <input data-trust-path aria-label="${escape(`Authorized collection path for ${repo}`)}" placeholder="skills/engineering" spellcheck="false">
        </label>
        <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="trust-path" data-repo="${escape(repo)}" data-git-ref="${escape(ref)}" ${busy ? 'disabled' : ''}>Trust collection</button></div>
        ${scopeLines ? `<ul class="skills-unmanaged" aria-label="${escape(`Trusted paths in ${repo}`)}">${scopeLines}</ul>` : ''}
      </li>`;
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
            <option value="yes" ${item.autoUpdate === true ? 'selected' : ''}>Enabled (trusted sources only)</option>
            <option value="no" ${item.autoUpdate === false ? 'selected' : ''}>Disabled</option>
          </select></label>
        <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="sync-item" ${busy ? 'disabled' : ''}>Sync this skill</button></div>
        ${report ? `<small>${escape(report.status)} · ${escape(report.detail)}${report.newCommit ? ` · ${escape(report.newCommit.slice(0, 12))}` : ''}</small>` : ''}</li>`;
    }).join('');
    return `<div class="settings-group skills-sync-controls">
      <p class="skills-note">Startup checks run without blocking the application. Updates and discovery are off by default. Explicitly trusting a repository or collection authorizes the automatic installation of new valid, unflagged skills in that exact scope the next time startup checks are enabled. Check your trust scope carefully; use a full commit SHA to pin source content.  Automatic checks do not move full-SHA pins. Synchronization overwrites local PandaCode library edits without a backup; it never silently overwrites harness-managed or manual files. Later updates to previously acknowledged skills may add new warnings.</p>
      <label class="skills-toggle"><input type="checkbox" data-global-sync ${syncPolicy.autoOnStart ? 'checked' : ''} ${busy ? 'disabled' : ''}><span>Automatically check trusted skills when PandaCode starts</span></label>
      <div class="skills-import-row skills-import-actions"><button type="button" class="secondary-button" data-action="sync-all" ${busy ? 'disabled' : ''}>Sync all installed skills</button></div>
      <p class="skills-message" role="status">${escape(busy ? 'Working…' : note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p>
      ${syncReport.checkedAt ? `<small>Last synchronization report: ${escape(new Date(syncReport.checkedAt).toLocaleString())} · ${plural(syncReport.installed?.length ?? 0, 'new skill')} installed · ${plural(syncReport.updated.length, 'skill')} updated</small>` : ''}
    </div><h3 class="settings-heading">Sources</h3>
    ${sources.length ? `<ul class="settings-group skills-list" aria-label="Synchronization sources">${sourceHtml}</ul>` : '<div class="settings-group skills-empty-group"><p class="skills-empty">No GitHub sources installed.</p></div>'}
    <h3 class="settings-heading">Installed skills</h3>
    ${githubItems.length ? `<ul class="settings-group skills-list" aria-label="Skill synchronization">${entries}</ul>` : '<div class="settings-group skills-empty-group"><p class="skills-empty">No GitHub skills installed yet.</p></div>'}`;
  }

  function draw(): void {
    const list = projects();
    const values = clearFields ? {} : Object.fromEntries([...host.querySelectorAll<HTMLInputElement | HTMLSelectElement>('form [name]')].map(field => [field.name, field.value]));
    host.innerHTML = `<nav class="ds-seg skills-view-tabs" aria-label="Skills Library pages">
      <button type="button" data-action="view" data-view="library" aria-current="${view === 'library' ? 'page' : 'false'}" aria-pressed="${view === 'library' ? 'true' : 'false'}">Library &amp; activation</button>
      <button type="button" data-action="view" data-view="recommended" aria-current="${view === 'recommended' ? 'page' : 'false'}" aria-pressed="${view === 'recommended' ? 'true' : 'false'}">Recommended skills</button>
      <button type="button" data-action="view" data-view="sync" aria-current="${view === 'sync' ? 'page' : 'false'}" aria-pressed="${view === 'sync' ? 'true' : 'false'}">Synchronization</button>
    </nav>${view === 'recommended' ? recommendedHtml() : view === 'sync' ? syncHtml() : `<h3 class="settings-heading">Import</h3><div class="settings-group skills-library" aria-label="Skills and rules">
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
    if (button.dataset.action === 'recommend-import' || button.dataset.action === 'recommend-all') {
      const gitRef = host.querySelector<HTMLInputElement>('[name="recommendedRef"]')?.value.trim() || 'main';
      const paths = recommendations.templates.filter(entry => entry.available &&
        (button.dataset.action === 'recommend-all' ? !entry.optional : recommendedSelected.has(entry.id))).map(entry => entry.path);
      if (!paths.length) { error = 'Select at least one available recommended skill.'; draw(); return; }
      void run(async () => {
        await options.call('skills_catalog_add', { repo: 'ginmp8/rhapsodia', gitRef });
        const result = await options.call<CatalogImport>('skills_catalog_import', { repo: 'ginmp8/rhapsodia', paths });
        recommendedSelected = new Set();
        return `Imported ${plural(result.imported.length, 'recommended skill')}.${result.skipped.length ? ` Skipped: ${result.skipped.map(entry => entry.reason).join('; ')}` : ''} Activation is separate.`;
      });
      return;
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
    if (button.dataset.action === 'sync-source' && button.dataset.repo) {
      void run(async () => {
        const result = await options.call<SkillSyncReport>('skills_sync_source', { repo: button.dataset.repo });
        return `Updated ${plural(result.updated.length, 'skill')}. ${result.skipped.join('; ')}`;
      }); return;
    }
    if (button.dataset.action === 'trust-source' && button.dataset.repo) {
      void run(async () => {
        await options.call('skills_sync_trust', { repo: button.dataset.repo, path: '', gitRef: button.dataset.gitRef || null, trusted: button.dataset.trusted === 'true' });
        return button.dataset.trusted === 'true' ? 'Repository root explicitly trusted for startup checks.' : 'Repository root trust revoked.';
      }); return;
    }
    if (button.dataset.action === 'trust-path' && button.dataset.repo) {
      const sourceRow = button.closest<HTMLElement>('li[data-repo]');
      const path = sourceRow?.querySelector<HTMLInputElement>('[data-trust-path]')?.value.trim() || '';
      void run(async () => {
        await options.call('skills_sync_trust', { repo: button.dataset.repo, path, gitRef: button.dataset.gitRef || null, trusted: true });
        return `Authorized ${button.dataset.repo}/${path || '(root)'} for startup discovery.`;
      }); return;
    }
    if (button.dataset.action === 'revoke-path' && button.dataset.repo) {
      void run(async () => {
        await options.call('skills_sync_trust', { repo: button.dataset.repo, path: button.dataset.path || '', trusted: false });
        return 'Collection trust revoked.';
      }); return;
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
      void run(async () => { await options.call('skills_sync_set_global', { enabled: target.checked }); return target.checked ? 'Startup checks enabled for trusted sources.' : 'Startup checks disabled globally.'; });
    }
    if (target instanceof HTMLSelectElement && target.dataset.skillAuto) {
      const id = target.dataset.skillAuto;
      const choice = target.value === 'inherit' ? null : target.value === 'yes';
      void run(async () => { await options.call('skills_sync_override', { id, autoUpdate: choice }); return `Updated startup policy for ${id}.`; });
    }
    if (target instanceof HTMLInputElement && target.dataset.recommended) {
      const id = target.dataset.recommended;
      recommendedSelected = target.checked ? new Set([...recommendedSelected, id]) : new Set([...recommendedSelected].filter(value => value !== id));
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
    if (form.classList.contains('skills-catalog-add')) addCatalog(form); else importItem(form);
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
