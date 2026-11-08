import './bot-manifests.css';
import { escape } from './dom';
import { open, save } from '@tauri-apps/plugin-dialog';

type Backend = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type BotOption = { id: string; name: string };
type Source = {
  id: string; repository: string; ref: string; manifests: string[]; autoSync: boolean;
  lastCheckedSha?: string | null; lastAppliedSha?: string | null; lastAppliedHash?: string | null;
};
type Registry = { version: number; sources: Source[] };
type ExportResult = { omittedSharedChannels: string[]; document: unknown };
type FolderExportResult = { paths: string[]; omittedSharedChannels: string[] };
type ImportResult = { backupPath: string; lines: string[]; failed: number };
type SyncResult = ImportResult & { applied: boolean; sources: { id: string; commit: string; paths: string[]; hashes: string[] }[]; backupPath: string | null };

const text = (value: unknown): string => value instanceof Error ? value.message : String(value);
const jsonFilter = [{ name: 'JSON manifests', extensions: ['json'] }];
const briefSha = (value?: string | null): string => value ? value.slice(0, 12) : 'Not checked';
const omissionNotice = (channels: string[]): string => channels.length
  ? ' Shared channels omitted because their full membership is not in each exported file: ' + channels.join(', ') + '.'
  : '';

export async function mountBotManifests(
  host: HTMLElement,
  mode: 'import' | 'export',
  { call, onApplied }: { call: Backend; onApplied: () => Promise<void> | void },
): Promise<void> {
  let busy = false;
  let registry: Registry = { version: 1, sources: [] };
  let activeTab: 'local' | 'git' = 'local';
  const bots = mode === 'export' ? await call<BotOption[]>('bots_list') : [];
  if (!host.isConnected) return;

  function status(message: string, problem = false): void {
    const display = host.querySelector<HTMLElement>('[data-manifest-status]');
    if (!display) return;
    display.textContent = message;
    display.dataset.error = String(problem);
    display.hidden = false;
  }

  const description = '<p class="bm-note">Portable configuration only. Conversations, memories, workspaces, credentials and running tasks are excluded. A JSON import creates a verified local backup before making any changes.</p>';

  function renderExport(selected = new Set(bots.map(bot => bot.id))): void {
    host.innerHTML = '<div class="bm-panel">' + description +
      '<fieldset class="bm-bots"><legend>Select bots</legend>' +
      '<label class="bm-choice"><input type="checkbox" data-select-all ' +
      (selected.size === bots.length && bots.length ? 'checked' : '') + ' ' +
      (!bots.length ? 'disabled' : '') + '> Select all</label>' +
      (bots.length ? bots.map(bot => '<label class="bm-choice"><input type="checkbox" data-select-bot="' +
        escape(bot.id) + '" ' + (selected.has(bot.id) ? 'checked' : '') + '> ' +
        escape(bot.name) + '</label>').join('') : '<p>No bots to export.</p>') +
      '</fieldset><fieldset class="bm-formats"><legend>Export format</legend>' +
      '<label><input type="radio" name="bot-export-format" value="combined" checked> One JSON file for selected bots</label>' +
      '<label><input type="radio" name="bot-export-format" value="separate"> A separate JSON file per bot (choose a folder)</label>' +
      '</fieldset><div class="dialog-footer"><button type="button" class="primary-button" data-manifest-export ' +
      (!bots.length ? 'disabled' : '') + '>Export selected bots…</button></div>' +
      '<p role="status" data-manifest-status hidden></p></div>';
  }

  const sourceRow = (source: Source, i: number): string =>
    '<fieldset class="bm-source" data-source-row="' + i + '"><legend>Source ' + (i + 1) +
    '</legend><label>Source ID<input type="text" data-source-field="id" value="' + escape(source.id) +
    '" required maxlength="100" placeholder="team-bots"></label>' +
    '<label>GitHub repository<input type="url" data-source-field="repository" value="' +
    escape(source.repository) + '" required placeholder="https://github.com/team/bots-config"></label>' +
    '<label>Branch, tag or commit<input type="text" data-source-field="ref" value="' + escape(source.ref) +
    '" required placeholder="main"></label><label>Manifest paths (one per line)<textarea data-source-field="manifests" rows="2" required placeholder="bots/developer.json">' +
    escape(source.manifests.join('\n')) + '</textarea></label>' +
    '<p class="bm-source-state">Checked: ' + escape(briefSha(source.lastCheckedSha)) +
    ' · Applied: ' + escape(briefSha(source.lastAppliedSha)) + '</p>' +
    '<button type="button" class="secondary-button" data-source-remove="' + i + '">Remove source</button></fieldset>';

  function renderImport(): void {
    host.innerHTML = '<div class="bm-panel">' + description +
      '<div class="bm-tabs" role="group" aria-label="Import source">' +
      '<button type="button" data-manifest-tab="local" aria-pressed="' + (activeTab === 'local') + '">Local JSON file</button>' +
      '<button type="button" data-manifest-tab="git" aria-pressed="' + (activeTab === 'git') + '">Git sources</button></div>' +
      (activeTab === 'local' ?
        '<section><label class="bm-file">Select a UTF-8 JSON manifest<input type="file" accept=".json,application/json" data-manifest-local></label>' +
        '<p class="bm-note">Existing bots are updated by name; omitted bots are never deleted. A failed bot does not undo other successful bots.</p>' +
        '<div class="dialog-footer"><button type="button" class="primary-button" data-manifest-apply>Import bots…</button></div></section>'
        : '<section><p>Sources are read-only. Each Git ref is pinned to a commit before its JSON files are loaded. Private GitHub repositories require an existing <code>GH_TOKEN</code> or <code>GITHUB_TOKEN</code> in the application environment.</p>' +
        registry.sources.map(sourceRow).join('') +
        '<button type="button" class="secondary-button" data-source-add>Add source</button>' +
        '<div class="dialog-footer bm-actions"><button type="button" class="secondary-button" data-sources-save>Save sources</button>' +
        '<button type="button" class="secondary-button" data-sources-check>Check for updates</button>' +
        '<button type="button" class="primary-button" data-sources-sync>Sync now</button></div>' +
        '<p class="bm-note">No automatic Git polling is enabled. Sync is explicit and never pushes to a repository.</p></section>') +
      '<p role="status" data-manifest-status hidden></p></div>';
  }

  if (mode === 'export') renderExport();
  else {
    registry = await call<Registry>('bot_manifest_sources_list');
    if (!host.isConnected) return;
    renderImport();
  }

  function selectIds(): string[] {
    return Array.from(host.querySelectorAll<HTMLInputElement>('[data-select-bot]:checked'))
      .map(input => input.dataset.selectBot!).filter(Boolean);
  }

  function syncSourceInputs(): void {
    for (const row of host.querySelectorAll<HTMLElement>('[data-source-row]')) {
      const index = Number(row.dataset.sourceRow);
      const source = registry.sources[index];
      if (!source) continue;
      const field = (name: string) => row.querySelector<HTMLInputElement | HTMLTextAreaElement>('[data-source-field="' + name + '"]')?.value ?? '';
      source.id = field('id').trim();
      source.repository = field('repository').trim();
      source.ref = field('ref').trim();
      source.manifests = field('manifests').split(/[\r\n,]+/).map(s => s.trim()).filter(Boolean);
    }
  }

  async function saveSources(): Promise<void> {
    syncSourceInputs();
    registry = await call<Registry>('bot_manifest_sources_save', { registry });
  }

  async function runAction(action: string): Promise<void> {
    if (busy) return;
    busy = true;
    const buttons = host.querySelectorAll<HTMLButtonElement>('button');
    buttons.forEach(button => { button.disabled = true; });
    try {
      if (action === 'export') {
        const botIds = selectIds();
        if (!botIds.length) throw new Error('Select at least one bot.');
        const separate = host.querySelector<HTMLInputElement>('input[name=bot-export-format]:checked')?.value === 'separate';
        if (separate) {
          const folder = await open({ title: 'Export individual bot manifests', directory: true, multiple: false });
          if (typeof folder !== 'string') return;
          const result = await call<FolderExportResult>('bot_manifest_export_folder', { botIds, folder });
          status('Exported ' + result.paths.length + ' bot manifest(s) to ' + folder + '.' +
            omissionNotice(result.omittedSharedChannels));
        } else {
          const path = await save({ title: 'Export bot configuration', defaultPath: 'bots.json', filters: jsonFilter });
          if (!path) return;
          const result = await call<ExportResult>('bot_manifest_export_file', { botIds, path });
          status('Configuration saved to ' + path + '.' + omissionNotice(result.omittedSharedChannels));
        }
      } else if (action === 'import') {
        const file = host.querySelector<HTMLInputElement>('[data-manifest-local]')?.files?.[0];
        if (!file) throw new Error('Choose a JSON file.');
        if (file.size > 1024 * 1024) throw new Error('The JSON manifest exceeds 1 MB.');
        const contents = new TextDecoder('utf-8', { fatal: true }).decode(await file.arrayBuffer());
        const document: unknown = JSON.parse(contents);
        const result = await call<ImportResult>('bot_manifest_import', { document });
        await onApplied();
        status('Backup: ' + result.backupPath + '. ' + result.lines.join(' · ') +
          (result.failed ? ' (' + result.failed + ' failed)' : ' (completed)'), result.failed > 0);
      } else if (action === 'save') {
        await saveSources();
        status('Git source configuration saved.');
      } else if (action === 'check' || action === 'sync') {
        if (!registry.sources.length) throw new Error('Add at least one Git source.');
        await saveSources();
        const result = await call<SyncResult>(
          action === 'check' ? 'bot_manifest_sources_check' : 'bot_manifest_sources_sync');
        registry = await call<Registry>('bot_manifest_sources_list');
        renderImport();
        if (action === 'sync' && result.backupPath) await onApplied();
        const revisions = result.sources.map(source => source.id + ' @ ' + briefSha(source.commit)).join(', ');
        status((action === 'check' ? 'Checked: ' : 'Synced: ') + revisions + '. ' +
          (result.backupPath ? 'Backup: ' + result.backupPath + '. ' : '') +
          result.lines.join(' · '), result.failed > 0);
      }
    } catch (error) {
      status('Operation failed: ' + text(error), true);
    } finally {
      busy = false;
      host.querySelectorAll<HTMLButtonElement>('button').forEach(button => { button.disabled = false; });
    }
  }

  host.addEventListener('change', event => {
    const target = event.target as HTMLInputElement;
    if (target.matches('[data-select-all]')) {
      host.querySelectorAll<HTMLInputElement>('[data-select-bot]').forEach(bot => { bot.checked = target.checked; });
    } else if (target.matches('[data-select-bot]')) {
      const all = host.querySelector<HTMLInputElement>('[data-select-all]');
      if (all) all.checked = selectIds().length === bots.length;
    }
  });
  host.addEventListener('click', event => {
    const target = (event.target as HTMLElement).closest<HTMLButtonElement>('button');
    if (!target || busy) return;
    if (target.dataset.manifestTab === 'local' || target.dataset.manifestTab === 'git') {
      syncSourceInputs();
      activeTab = target.dataset.manifestTab;
      renderImport();
    } else if (target.hasAttribute('data-source-add')) {
      syncSourceInputs();
      registry.sources.push({ id: '', repository: '', ref: 'main', manifests: [], autoSync: false });
      renderImport();
    } else if (target.hasAttribute('data-source-remove')) {
      syncSourceInputs();
      registry.sources.splice(Number(target.dataset.sourceRemove), 1);
      renderImport();
    } else if (target.hasAttribute('data-manifest-export')) void runAction('export');
    else if (target.hasAttribute('data-manifest-apply')) void runAction('import');
    else if (target.hasAttribute('data-sources-save')) void runAction('save');
    else if (target.hasAttribute('data-sources-check')) void runAction('check');
    else if (target.hasAttribute('data-sources-sync')) void runAction('sync');
  });
}
