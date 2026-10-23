import { expect, test, type Page } from '@playwright/test';

const dialog = (page: Page) => page.getByRole('dialog', { name: 'Agent updates', exact: true });
const calls = (page: Page) => page.evaluate(() => (window as any).__agentUpdates.calls);
async function open(page: Page): Promise<void> {
  await page.clock.install();
  await page.addInitScript(() => {
    (window as any).__agentUpdates = { calls: [] };
    localStorage.setItem('pandacode.autoUpdate', 'on');
    localStorage.setItem('pandacode.agent-updates.dismissed', '{}');
  });
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(), source = await response.text();
    const marker = /switch\s*\(command\)\s*\{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `
      if (['check_agent_updates', 'update_agent'].includes(command)) {
        window.__agentUpdates.calls.push({ command, args });
        throw new Error('External updates are disabled.');
      }
      switch (command) {`) });
  });
  await page.goto('/');
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeEnabled();
}
async function manual(page: Page): Promise<void> {
  await page.keyboard.press('Meta+,');
  await page.getByRole('region', { name: 'Settings', exact: true }).getByRole('tab', { name: 'Agents', exact: true }).click();
  await page.getByRole('button', { name: 'Update policy', exact: true }).click();
  await expect(dialog(page)).toBeVisible();
}

test.describe('Hardened agent update policy', () => {
  test('startup, elapsed time and focus never check or install updates', async ({ page }) => {
    await open(page);
    await page.clock.fastForward(24 * 60 * 60 * 1000);
    await page.evaluate(() => {
      window.dispatchEvent(new Event('focus'));
      document.dispatchEvent(new Event('visibilitychange'));
    });
    expect(await calls(page)).toEqual([]);
    await expect(page.getByRole('region', { name: 'Agent updates', exact: true })).toHaveCount(0);
  });

  test('manual update entry explains policy without contacting a server', async ({ page }) => {
    await open(page); await manual(page);
    await expect(dialog(page)).toContainText('disabled in this hardened build');
    await expect(dialog(page).getByRole('button', { name: /^Update / })).toHaveCount(0);
    expect(await calls(page)).toEqual([]);
    await dialog(page).getByRole('button', { name: 'Done', exact: true }).click();
    await expect(dialog(page)).toBeHidden();
  });

  test('an old enabled preference does not restore updates after reopening', async ({ page }) => {
    await open(page);
    await page.reload();
    await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeEnabled();
    await page.clock.fastForward(9000);
    expect(await calls(page)).toEqual([]);
    await manual(page);
    await expect(dialog(page)).toContainText('Install reviewed agent versions manually outside PandaCode.');
  });
});
