import { expect, test } from '@playwright/test';

test('a stale displayed Connect key is rejected before trust is granted', async ({ page }) => {
  await page.goto('/');
  await page.locator('#navigation .task-item').first().waitFor();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Remote access' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Remote access' });
  await panel.getByRole('button', { name: 'Sign in with GitHub' }).click();
  await expect(panel.getByRole('button', { name: 'Sign out' })).toBeVisible();
  const row = panel.locator('.remote-device[data-device="dev-nas"]');
  const trust = panel.getByRole('checkbox', { name: 'Trusted: Home NAS' });
  await expect(trust).not.toBeChecked();
  await expect(row.locator('code')).toHaveText('17aa 5e90 b3c4 d286 ea5b b53d 782f 9104');
  await row.evaluate(element => { (element as HTMLElement).dataset.fingerprint = '00000000000000000000000000000000'; });
  // Exactly one user action: check() may retry after the rejected control is rerendered.
  await trust.evaluate(element => (element as HTMLInputElement).click());
  await expect(trust).not.toBeChecked();
  await expect(panel.getByRole('alert')).toContainText('key changed');
  expect(await page.evaluate(() => (window as any).__trustCalls ?? [])).toHaveLength(0);
  await row.evaluate(element => { (element as HTMLElement).dataset.fingerprint = '17aa5e90b3c4d286ea5bb53d782f9104'; });
  await trust.evaluate(element => (element as HTMLInputElement).click());
  await expect(trust).toBeChecked();
  await expect.poll(() => page.evaluate(() => ((window as any).__trustCalls ?? []).at(-1))).toEqual({
    deviceId: 'dev-nas', trusted: true, expectedFingerprint: '17aa5e90b3c4d286ea5bb53d782f9104',
  });
});

test('pairing approval sends the displayed full fingerprint, blocking requires no new key', async ({ page }) => {
  await page.goto('/');
  await page.locator('#navigation .task-item').first().waitFor();
  await page.evaluate(async () => {
    const path = '/src/remote-pairing-prompt.ts';
    const { mountPairingPrompt } = await import(/* @vite-ignore */ path);
    const fake: any = { handlers: {}, calls: [] };
    (window as any).__securityPairing = fake;
    mountPairingPrompt({
      call: async (name: string, args: unknown) => { fake.calls.push({ name, args }); },
      listen: async (name: string, cb: (data: unknown) => void) => { fake.handlers[name] = cb; return () => {}; },
      notify: () => {},
      openUrl: async () => {},
    } as any);
  });
  await page.evaluate(() => (window as any).__securityPairing.handlers['remote://device-pending']({
    deviceId: 'phone', name: 'Pixel', fingerprint: '0123456789abcdef0123456789abcdef',
  }));
  await expect(page.locator('.pairing-prompt[data-device="phone"] code')).toHaveText('0123 4567 89ab cdef 0123 4567 89ab cdef');
  await page.locator('.pairing-prompt[data-device="phone"]').getByRole('button', { name: 'Approve' }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__securityPairing.calls)).toEqual([{
    name: 'remote_set_device_grant',
    args: { deviceId: 'phone', scope: 'control', expectedFingerprint: '0123456789abcdef0123456789abcdef' },
  }]);
  await page.evaluate(() => (window as any).__securityPairing.handlers['remote://device-pending']({
    deviceId: 'phone2', name: 'Second', fingerprint: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  }));
  await page.locator('.pairing-prompt[data-device="phone2"]').getByRole('button', { name: 'Block' }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__securityPairing.calls.length)).toBe(2);
  expect(await page.evaluate(() => (window as any).__securityPairing.calls[1])).toEqual({
    name: 'remote_set_device_grant', args: { deviceId: 'phone2', scope: 'blocked' },
  });
});

test('the local direct route setting is opt-in and can be disabled again', async ({ page }) => {
  await page.goto('/');
  await page.locator('#navigation .task-item').first().waitFor();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Remote access' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Remote access' });
  await panel.getByRole('button', { name: 'Sign in with GitHub' }).click();
  await expect(panel.getByRole('button', { name: 'Sign out' })).toBeVisible();
  const direct = panel.getByRole('checkbox', { name: 'Use direct LAN/Tailscale connections' });
  await expect(direct).not.toBeChecked();
  await direct.check();
  await expect(direct).toBeChecked();
  await direct.uncheck();
  await expect(direct).not.toBeChecked();
});


test('removing and restoring Trusted never silently restores a previous Full grant', async ({ page }) => {
  await page.goto('/');
  await page.locator('#navigation .task-item').first().waitFor();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Remote access' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Remote access' });
  await panel.getByRole('button', { name: 'Sign in with GitHub' }).click();
  await expect(panel.getByRole('button', { name: 'Sign out' })).toBeVisible();
  const row = panel.locator('.remote-device[data-device="dev-studio"]');
  const trusted = row.getByRole('checkbox', { name: 'Trusted: Studio iMac' });
  await expect(trusted).toBeChecked();
  await expect(row.getByRole('combobox', { name: 'Access for Studio iMac' })).toHaveValue('full');
  await trusted.uncheck();
  await expect(row.getByRole('button', { name: 'Approve Studio iMac' })).toBeVisible();
  await trusted.check();
  await expect(row.getByRole('button', { name: 'Approve Studio iMac' })).toBeVisible();
  await row.getByRole('combobox', { name: 'Approve Studio iMac as' }).selectOption('control');
  await row.getByRole('button', { name: 'Approve Studio iMac' }).click();
  await expect(row.getByRole('combobox', { name: 'Access for Studio iMac' })).toHaveValue('control');
});

test('remote Full scopes require the separate local opt-in and are downgraded when it is turned off', async ({ page }) => {
  await page.goto('/');
  await page.locator('#navigation .task-item').first().waitFor();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Remote access' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Remote access' });
  await panel.getByRole('button', { name: 'Sign in with GitHub' }).click();
  await expect(panel.getByRole('button', { name: 'Sign out' })).toBeVisible();
  const studio = panel.locator('.remote-device[data-device="dev-studio"]');
  const select = studio.getByRole('combobox', { name: 'Access for Studio iMac' });
  await expect(select).toHaveValue('full');
  const full = panel.getByRole('checkbox', { name: 'Full access for remote tasks' });
  await full.check();
  await full.uncheck();
  await expect(select).toHaveValue('control');
  await full.check();
  await expect(select).toHaveValue('control');
  await select.selectOption('full');
  await expect(select).toHaveValue('full');
});
