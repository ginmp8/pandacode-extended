import { expect, test, type Page } from '@playwright/test';

/** Test the real UI module, with only native file dialogs and backend calls isolated. */
async function mount(page: Page, mode: 'import' | 'export') {
  await page.evaluate(async requestedMode => {
    const w = window as any;
    const host = document.createElement('section');
    host.id = 'manifest-test-host';
    document.body.append(host);
    w.__manifestCalls = [];
    w.__manifestApplied = 0;
    w.__manifestRegistry = { version: 1, sources: [] };
    const sampleBots = [{ id: 'bot-scout', name: 'Scout' }, { id: 'bot-writer', name: 'Writer' }];
    const clone = (value: unknown) => structuredClone(value);
    const backend = async (command: string, args: Record<string, any> = {}) => {
      w.__manifestCalls.push({ command, args: clone(args) });
      if (command === 'bots_list') return clone(sampleBots);
      if (command === 'bot_manifest_import') {
        if (w.__rejectManifest) throw new Error('The manifest is invalid.');
        return { backupPath: '/app-data/bot-backups/bots-verified.json', lines: ['"Scout" updated'], failed: 0 };
      }
      if (command === 'bot_manifest_export_file') {
        return { document: { bots: [] }, omittedSharedChannels: ['requires-other-bots'] };
      }
      if (command === 'bot_manifest_export_folder') {
        return (args.botIds as string[]).map((id: string) => '/saved/' + id + '.json');
      }
      if (command === 'bot_manifest_sources_list') return clone(w.__manifestRegistry);
      if (command === 'bot_manifest_sources_save') {
        w.__manifestRegistry = clone(args.registry);
        return clone(w.__manifestRegistry);
      }
      if (command === 'bot_manifest_sources_check' || command === 'bot_manifest_sources_sync') {
        for (const source of w.__manifestRegistry.sources) {
          source.lastCheckedSha = '0123456789abcdef0123456789abcdef01234567';
          if (command === 'bot_manifest_sources_sync') {
            source.lastAppliedSha = source.lastCheckedSha;
          }
        }
        return { sources: w.__manifestRegistry.sources.map((source: any) => ({
          id: source.id, repository: source.repository, ref: source.ref,
          commit: source.lastCheckedSha, paths: source.manifests, hashes: ['sha256'],
        })), backupPath: command === 'bot_manifest_sources_sync' ? '/app-data/bot-backups/sync.json' : null,
        lines: ['Git manifests validated.'], failed: 0, applied: command === 'bot_manifest_sources_sync' };
      }
      throw new Error('Unexpected backend call: ' + command);
    };
    const { mountBotManifests } = await import('/src/bot-manifests.ts');
    await mountBotManifests(host, requestedMode, {
      call: backend,
      onApplied: async () => { w.__manifestApplied += 1; },
    });
  }, mode);
  return page.locator('#manifest-test-host');
}

test.beforeEach(async ({ page }) => {
  // Native filesystem dialogs cannot run in the browser preview; replace only
  // their import with deterministic destinations. Backend behavior stays mocked separately.
  await page.route('**/src/bot-manifests.ts*', async route => {
    const response = await route.fetch();
    const source = await response.text();
    const dialogImport = /import\s*\{\s*open\s*,\s*save\s*\}\s*from\s*["'][^"']+["'];?/;
    expect(source).toMatch(dialogImport);
    await route.fulfill({ response, body: source.replace(dialogImport,
      "const open = async () => '/mock/exports'; const save = async () => '/mock/bots.json';") });
  });
  await page.goto('/');
});

test.describe('bot manifests', () => {
  test('selects one, multiple, and separate bot JSON exports with no state mutation', async ({ page }) => {
    const host = await mount(page, 'export');
    await expect(host.getByRole('checkbox', { name: /Select all/ })).toBeChecked();
    await host.getByRole('checkbox', { name: /Select all/ }).uncheck();
    await host.getByRole('checkbox', { name: 'Scout' }).check();
    await host.getByRole('button', { name: /Export selected bots/ }).click();
    await expect(host.getByRole('status')).toContainText('Configuration saved');
    let exports = await page.evaluate(() => (window as any).__manifestCalls.filter((entry: any) =>
      entry.command === 'bot_manifest_export_file'));
    expect(exports).toHaveLength(1);
    expect(exports[0].args.botIds).toEqual(['bot-scout']);
    await expect(host.getByRole('status')).toContainText('requires-other-bots');
    await host.getByRole('checkbox', { name: /Select all/ }).check();
    await host.getByRole('button', { name: /Export selected bots/ }).click();
    await expect.poll(async () => page.evaluate(() => (window as any).__manifestCalls.filter((entry: any) =>
      entry.command === 'bot_manifest_export_file').length)).toBe(2);
    exports = await page.evaluate(() => (window as any).__manifestCalls.filter((entry: any) =>
      entry.command === 'bot_manifest_export_file'));
    expect(exports[1].args.botIds).toEqual(['bot-scout', 'bot-writer']);
    await host.getByRole('radio', { name: /separate JSON file per bot/i }).check();
    await host.getByRole('button', { name: /Export selected bots/ }).click();
    await expect(host.getByRole('status')).toContainText('Exported 2 bot manifest');
    const folders = await page.evaluate(() => (window as any).__manifestCalls.filter((entry: any) =>
      entry.command === 'bot_manifest_export_folder'));
    expect(folders[0].args.botIds).toEqual(['bot-scout', 'bot-writer']);
    expect(await page.evaluate(() => (window as any).__manifestApplied)).toBe(0);
  });

  test('rejects malformed local JSON and shows the verified backup after a successful import', async ({ page }) => {
    const host = await mount(page, 'import');
    await host.getByRole('button', { name: /Import bots/ }).click();
    await expect(host.getByRole('status')).toContainText('Choose a JSON file');
    await host.locator('[data-manifest-local]').setInputFiles({
      name: 'bad.json', mimeType: 'application/json', buffer: Buffer.from('{broken'),
    });
    await host.getByRole('button', { name: /Import bots/ }).click();
    await expect(host.getByRole('status')).toContainText('Operation failed');
    expect(await page.evaluate(() => (window as any).__manifestCalls.some((entry: any) =>
      entry.command === 'bot_manifest_import'))).toBe(false);
    await host.locator('[data-manifest-local]').setInputFiles({
      name: 'bots.json', mimeType: 'application/json',
      buffer: Buffer.from('{"bots":[{"name":"Scout"}]}'),
    });
    await host.getByRole('button', { name: /Import bots/ }).click();
    await expect(host.getByRole('status')).toContainText('/app-data/bot-backups/bots-verified.json');
    expect(await page.evaluate(() => (window as any).__manifestApplied)).toBe(1);
  });

  test('supports multiple Git sources, check/sync, and retains applied revisions', async ({ page }) => {
    const host = await mount(page, 'import');
    await host.getByRole('button', { name: 'Git sources' }).click();
    await host.getByRole('button', { name: 'Add source' }).click();
    await host.getByRole('button', { name: 'Add source' }).click();
    const rows = host.locator('[data-source-row]');
    await rows.nth(0).getByRole('textbox', { name: 'Source ID' }).fill('team');
    await rows.nth(0).getByRole('textbox', { name: 'GitHub repository' }).fill('https://github.com/team/config');
    await rows.nth(0).getByRole('textbox', { name: /Branch, tag or commit/ }).fill('main');
    await rows.nth(0).getByRole('textbox', { name: /Manifest paths/ }).fill('bots/one.json');
    await rows.nth(1).getByRole('textbox', { name: 'Source ID' }).fill('release');
    await rows.nth(1).getByRole('textbox', { name: 'GitHub repository' }).fill('https://github.com/team/releases');
    await rows.nth(1).getByRole('textbox', { name: /Manifest paths/ }).fill('bots/two.json');
    await host.getByRole('button', { name: 'Save sources' }).click();
    await expect(host.getByRole('status')).toContainText('saved');
    await host.getByRole('button', { name: 'Check for updates' }).click();
    await expect(host.getByRole('status')).toContainText('Checked: team');
    await host.getByRole('button', { name: 'Sync now' }).click();
    await expect(host.getByRole('status')).toContainText('Synced: team');
    await expect(host.getByRole('status')).toContainText('/app-data/bot-backups/sync.json');
    await expect(rows).toHaveCount(2);
    await expect(host.locator('.bm-source-state')).toContainText([
      'Checked: 0123456789ab', 'Checked: 0123456789ab',
    ]);
    expect(await page.evaluate(() => (window as any).__manifestApplied)).toBe(1);
  });

  test('reports backend import failure without claiming applied changes', async ({ page }) => {
    const host = await mount(page, 'import');
    await page.evaluate(() => { (window as any).__rejectManifest = true; });
    await host.locator('[data-manifest-local]').setInputFiles({
      name: 'bots.json', mimeType: 'application/json', buffer: Buffer.from('{"bots":[]}'),
    });
    await host.getByRole('button', { name: /Import bots/ }).click();
    await expect(host.getByRole('status')).toContainText('The manifest is invalid');
    expect(await page.evaluate(() => (window as any).__manifestApplied)).toBe(0);
  });
});
