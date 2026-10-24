import { expect, test, type Page } from '@playwright/test';

const connection = (id: string, provider = 'claude') => ({ id, provider, name: id === 'work' ? 'Work account' : 'Other account', args: [], env: {}, secretEnv: [], configDir: '/tmp/pandacode-fixture-' + id, revision: 1 });
async function seeded(page: Page, options: { limited?: boolean; currentMissing?: boolean; mismatched?: boolean } = {}) {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch();
    let source = await response.text();
    source = source.replace(/switch\s*\(command\)\s*\{/, `
      (window.__connection041Calls ??= []).push({ command, args: structuredClone(args) });
      if (!window.__connection041Seeded) {
        window.__connection041Seeded = true;
        previewConnections = { connections: ${JSON.stringify([connection('work'), connection('other', 'codex')])}, tasks: {} };
        Object.assign(sessions[0], ${JSON.stringify(options.limited ? { limitedAt: 1, limitedUntil: null, resumeId: 'old-account-session' } : {})});
        providers.find(p => p.id === 'antigravity').installed = false;
        ${options.currentMissing ? "providers.find(p => p.id === 'claude').installed = false;" : ''}
      }
      if (command === 'agent_usage' && args.connection) return {
        provider: args.provider, connectionId: ${options.mismatched ? "'wrong-account'" : 'args.connection'}, status: 'ready',
        checkedAt: Date.now(), stale: false, plan: 'Work plan', windows: [{ label: 'Work window', usedPercent: 73 }]
      };
      if (command === 'send_message') { session(args.sessionId).status = 'running'; return; }
      switch (command) {`);
    await route.fulfill({ response, body: source });
  });
  await page.goto('/');
  await expect(page.locator('#prompt')).toBeVisible();
}

test('usage displays a named connection separately from default account limits', async ({ page }) => {
  await seeded(page);
  await page.locator('#agent-usage-button').click();
  const usage = page.getByRole('dialog', { name: 'Agent usage', exact: true });
  await usage.getByRole('tab', { name: 'Claude', exact: true }).click();
  const account = usage.locator('.usage-connection[data-connection="work"]');
  await expect(account).toContainText('Work account');
  await expect(account.getByRole('progressbar', { name: 'Work account · Work window' })).toHaveAttribute('aria-valuenow', '73');
  await expect(usage.locator('.usage-connection-default').first()).toHaveText('Default setup');
});

test('an account identifier mismatch cannot render another account limits', async ({ page }) => {
  await seeded(page, { mismatched: true });
  await page.locator('#agent-usage-button').click();
  const usage = page.getByRole('dialog', { name: 'Agent usage', exact: true });
  await usage.getByRole('tab', { name: 'Claude', exact: true }).click();
  const account = usage.locator('.usage-connection[data-connection="work"]');
  await expect(account).toContainText('Could not read limits for this connection. Try Refresh.');
  await expect(account.getByRole('progressbar')).toHaveCount(0);
});

test('a limit offers only compatible connections and switches before sending explicitly', async ({ page }) => {
  await seeded(page, { limited: true });
  const banner = page.getByRole('region', { name: 'Usage limit', exact: true });
  await expect(banner.getByRole('button', { name: 'Continue on Work account', exact: true })).toBeVisible();
  await expect(banner.getByRole('button', { name: 'Continue on Other account', exact: true })).toHaveCount(0);
  const calls = () => page.evaluate(() => (window as any).__connection041Calls.filter((c: any) => ['connection_assign', 'send_message'].includes(c.command)));
  expect(await calls()).toEqual([]);
  await banner.getByRole('button', { name: 'Continue on Work account', exact: true }).click();
  await expect.poll(async () => (await calls()).map((c: any) => c.command)).toEqual(['connection_assign', 'send_message']);
  expect((await calls())[0].args).toEqual({ sessionId: 'session-1', connectionId: 'work' });
  await expect(page.locator('#model-picker-trigger')).toContainText('Work account');
});

test('uninstalled noncurrent providers are absent from the model picker', async ({ page }) => {
  await seeded(page);
  await page.locator('#model-picker-trigger').click();
  const picker = page.getByRole('dialog', { name: 'Harness and model', exact: true });
  await expect(picker.getByRole('region', { name: 'Antigravity', exact: true })).toHaveCount(0);
  await expect(picker.getByRole('region', { name: 'Claude Code', exact: true })).toBeVisible();
});

test('an uninstalled current provider remains visible and cannot be selected', async ({ page }) => {
  await seeded(page, { currentMissing: true });
  await page.locator('#model-picker-trigger').click();
  const group = page.getByRole('dialog', { name: 'Harness and model', exact: true }).getByRole('region', { name: 'Claude Code', exact: true });
  await expect(group).toContainText('Unavailable');
  for (const button of await group.getByRole('button').all()) await expect(button).toBeDisabled();
});
