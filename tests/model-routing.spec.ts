import { expect, test, type Page } from '@playwright/test';

// Settings → Agents → Model routing, and the uncapped model catalog sent with an automatic team.
const settings = (page: Page) => page.getByRole('region', { name: 'Settings', exact: true });

async function install(page: Page, init?: () => void): Promise<void> {
  if (init) await page.addInitScript(init);
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(); const source = await response.text();
    const marker = /async function invoke\(command, args\) \{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `async function invoke(command, args) {
      (window.__routing ??= { calls: [] }).calls.push({ command, args: structuredClone(args) });
      // Test hook: \`__routingFail[command] = message\` rejects that command until cleared.
      const failure = command === 'model_routing_configure' && window.__routingWriteFailures?.length
        ? window.__routingWriteFailures.shift()
        : command === 'model_routing_status' && window.__routingStatusFailures?.length
          ? window.__routingStatusFailures.shift() : window.__routingFail?.[command];
      // Test hook: \`__routingDelay[command] = ms\` holds that command back before it runs.
      const delay = command === 'model_routing_configure' && window.__routingWriteDelays?.length
        ? window.__routingWriteDelays.shift()
        : command === 'model_routing_route' && window.__routingPreviewDelays?.length
          ? window.__routingPreviewDelays.shift() : window.__routingDelay?.[command];
      if (delay) await new Promise(resolve => setTimeout(resolve, delay));
      // Deterministic test hold: release native calls in a chosen order, without timer races.
      if (window.__routingHold?.[command]) await new Promise(resolve => (window.__routingReleases ??= []).push(resolve));
      if (failure) throw new Error(failure);
      return invokePreview(command, args);
    }
    async function invokePreview(command, args) {`) });
  });
  await page.goto('/');
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id', 'session-2');
}
const calls = (page: Page, command: string): Promise<any[]> => page.evaluate(command => (window as any).__routing.calls.filter((entry: any) => entry.command === command), command);
async function openAgents(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await settings(page).getByRole('tab', { name: 'Agent teams', exact: true }).click();
}
async function closeSettings(page: Page): Promise<void> {
  await settings(page).getByRole('button', { name: 'Back to the workspace', exact: true }).click();
  await expect(settings(page)).toBeHidden();
}

test('the section renders, settings persist and the profiler downloads', async ({ page }) => {
  await install(page);
  await openAgents(page);
  const toggle = settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true });
  await expect(toggle).toBeChecked();
  await expect(settings(page).getByText('Not downloaded', { exact: true })).toBeVisible();
  await expect(settings(page).getByRole('radio', { name: 'Balanced' })).toHaveAttribute('aria-checked', 'true');
  const minP = settings(page).getByRole('spinbutton', { name: /Minimum chance/ });
  await expect(minP).toBeHidden();
  await settings(page).getByRole('radio', { name: 'Cheap' }).click();
  await expect(minP).toBeVisible();
  await settings(page).getByRole('radio', { name: 'Cheap' }).press('ArrowRight');
  await expect(minP).toBeHidden();
  await settings(page).getByRole('radio', { name: 'Balanced' }).press('ArrowLeft');
  await expect(minP).toBeVisible();
  await toggle.uncheck();
  await settings(page).getByRole('button', { name: 'Download', exact: true }).click();
  await expect(settings(page).getByText('Ready', { exact: true })).toBeVisible();
  await expect(minP).toHaveValue('50');
  await expect(minP).toHaveAttribute('min', '30');
  await expect(minP).toHaveAttribute('max', '90');
  await minP.fill('99'); await minP.blur();
  await expect(minP).toHaveValue('90');
  await minP.fill('10'); await minP.blur();
  await expect(minP).toHaveValue('30');
  // Reopen: the preview keeps the configuration.
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await openAgents(page);
  await expect(settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true })).not.toBeChecked();
  await expect(settings(page).getByRole('radio', { name: 'Cheap' })).toHaveAttribute('aria-checked', 'true');
  await expect(settings(page).getByRole('spinbutton', { name: /Minimum chance/ })).toHaveValue('30');
  await expect(settings(page).getByRole('textbox', { name: 'Task to route' })).toBeDisabled();
  await expect(settings(page).getByText('Ready', { exact: true })).toBeVisible();
});

test('Settings retries an initial transient routing status read without user action', async ({ page }) => {
  await install(page);
  const before = (await calls(page, 'model_routing_status')).length;
  await page.evaluate(() => { (window as any).__routingStatusFailures = ['Temporary status read failure', null]; });
  await openAgents(page);
  await expect(settings(page).getByText('Routing status unavailable; retrying…')).toBeVisible();
  await expect.poll(async () => (await calls(page, 'model_routing_status')).length).toBeGreaterThan(before + 1);
  await expect(settings(page).getByRole('radio', { name: 'Balanced' })).toHaveAttribute('aria-checked', 'true');
  await expect(settings(page).getByText('Not downloaded', { exact: true })).toBeVisible();
});

test('a failed save cannot swallow the only initial status retry', async ({ page }) => {
  await install(page);
  // Startup may have read status already; hold and count only this Settings mount.
  const before = (await calls(page, 'model_routing_status')).length;
  await page.evaluate(() => {
    (window as any).__routingHold = { model_routing_status: true };
    (window as any).__routingStatusFailures = ['Initial status read failed', null];
    (window as any).__routingWriteFailures = ['Mode save failed'];
  });
  await openAgents(page);
  await expect.poll(async () => (await calls(page, 'model_routing_status')).length).toBe(before + 1);
  // The write supersedes the held read's generation, but its own save also fails.
  await settings(page).getByRole('radio', { name: 'Cheap' }).click();
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(1);
  await page.evaluate(() => {
    (window as any).__routingHold.model_routing_status = false;
    (window as any).__routingReleases.shift()();
  });
  await expect(settings(page).getByText('Routing status unavailable; retrying…')).toBeVisible();
  await expect.poll(async () => (await calls(page, 'model_routing_status')).length).toBe(before + 2);
  await expect(settings(page).getByRole('radio', { name: 'Balanced' })).toHaveAttribute('aria-checked', 'true');
  await expect(settings(page).getByRole('button', { name: 'Route', exact: true })).toBeEnabled();
});

test('routing preview is disabled until the initial native status has loaded', async ({ page }) => {
  await install(page);
  await page.evaluate(() => { (window as any).__routingDelay = { model_routing_status: 1300 }; });
  await openAgents(page);
  const routeButton = settings(page).getByRole('button', { name: 'Route', exact: true });
  const task = settings(page).getByRole('textbox', { name: 'Task to route' });
  await expect(routeButton).toBeDisabled();
  await expect(task).toBeDisabled();
  await expect(task).toBeEnabled();
  await expect(routeButton).toBeEnabled();
});

test('rapid Settings changes persist in user order even when the first native write is slow', async ({ page }) => {
  await install(page);
  await openAgents(page);
  await page.evaluate(() => { (window as any).__routingWriteDelays = [450, 0]; });
  const modes = settings(page);
  await modes.getByRole('radio', { name: 'Cheap' }).click();
  await modes.getByRole('radio', { name: 'Best' }).click();
  await expect(modes.getByRole('radio', { name: 'Best' })).toHaveAttribute('aria-checked', 'true');
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(2);
  // Check after the delayed write has had enough time to finish; a stale write must not win.
  await page.waitForTimeout(550);
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await openAgents(page);
  await expect(settings(page).getByRole('radio', { name: 'Best' })).toHaveAttribute('aria-checked', 'true');
});

test('a later Team mode change wins over an in-flight Settings write after closing Settings', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Balanced team');
  await openAgents(page);
  const before = (await calls(page, 'model_routing_configure')).length;
  await page.evaluate(() => { (window as any).__routingWriteDelays = [1200, 0]; });
  await settings(page).getByRole('radio', { name: 'Cheap' }).click();
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(before + 1);
  await closeSettings(page);
  await pickTeam(page, 'Best team');
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(before + 2);
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await openAgents(page);
  await expect(settings(page).getByRole('radio', { name: 'Best' })).toHaveAttribute('aria-checked', 'true');
});

test('a failed Settings write never blocks a later Team preference save', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Balanced team');
  await openAgents(page);
  const before = (await calls(page, 'model_routing_configure')).length;
  await page.evaluate(() => {
    (window as any).__routingWriteDelays = [550, 0];
    (window as any).__routingWriteFailures = ['Simulated settings save failure', null];
  });
  await settings(page).getByRole('radio', { name: 'Cheap' }).click();
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(before + 1);
  await closeSettings(page);
  await pickTeam(page, 'Best team');
  await expect.poll(async () => (await calls(page, 'model_routing_configure')).length).toBe(before + 2);
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await openAgents(page);
  await expect(settings(page).getByRole('radio', { name: 'Best' })).toHaveAttribute('aria-checked', 'true');
});

test('Try it shows a pick and alternatives', async ({ page }) => {
  await install(page);
  await openAgents(page);
  await settings(page).getByRole('textbox', { name: 'Task to route' }).fill('Fix the failing login test');
  await settings(page).getByRole('button', { name: 'Route', exact: true }).click();
  const rows = settings(page).getByRole('list', { name: 'Routed models' }).getByRole('listitem');
  await expect(rows).toHaveCount(5);
  await expect(rows.first()).toContainText('Pick');
  await expect(settings(page).getByText(/debugging/)).toBeVisible();
  expect((await calls(page, 'model_routing_route')).at(-1).args.pool.length).toBeGreaterThan(24);
});

test('turning routing off discards an in-flight preview instead of showing a stale recommendation', async ({ page }) => {
  await install(page);
  await openAgents(page);
  await page.evaluate(() => { (window as any).__routingDelay = { model_routing_route: 750 }; });
  await settings(page).getByRole('textbox', { name: 'Task to route' }).fill('Fix the failing login test');
  await settings(page).getByRole('button', { name: 'Route', exact: true }).click();
  await expect(settings(page).locator('.routing-result')).toHaveText('Routing…');
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);
  await settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true }).uncheck();
  await expect(settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true })).not.toBeChecked();
  await page.waitForTimeout(850);
  await expect(settings(page).locator('.routing-result')).toBeHidden();
  await expect(settings(page).getByRole('button', { name: 'Route', exact: true })).toBeDisabled();
});

test('editing the task immediately invalidates an in-flight preview for the previous text', async ({ page }) => {
  await install(page);
  await openAgents(page);
  await page.evaluate(() => { (window as any).__routingDelay = { model_routing_route: 750 }; });
  const task = settings(page).getByRole('textbox', { name: 'Task to route' });
  const result = settings(page).locator('.routing-result');
  await task.fill('Fix the failing login test');
  await settings(page).getByRole('button', { name: 'Route', exact: true }).click();
  await expect(result).toHaveText('Routing…');
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);
  await task.fill('Write release notes for the next version');
  await expect(result).toBeHidden();
  await page.waitForTimeout(850);
  await expect(result).toBeHidden();
});

test('editing a task allows an immediate new preview without an old request unlocking it', async ({ page }) => {
  await install(page);
  await openAgents(page);
  const task = settings(page).getByRole('textbox', { name: 'Task to route' });
  const routeButton = settings(page).getByRole('button', { name: 'Route', exact: true });
  const result = settings(page).locator('.routing-result');
  await page.evaluate(() => { (window as any).__routingHold = { model_routing_route: true }; });
  await task.fill('Fix the failing login test');
  await routeButton.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);
  await task.fill('Write release notes for the next version');
  // First native call remains held: a new task must be routable immediately.
  await expect(result).toBeHidden();
  await expect(routeButton).toBeEnabled();
  await routeButton.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(2);
  // Release the stale call while the newer one remains in flight.
  await page.evaluate(() => (window as any).__routingReleases.shift()());
  await expect(routeButton).toBeDisabled();
  await expect(result).toHaveText('Routing…');
  await page.evaluate(() => (window as any).__routingReleases.shift()());
  await expect(result).toContainText('Difficulty');
  await expect(routeButton).toBeEnabled();
});

test('preview limits obsolete concurrent native routing requests', async ({ page }) => {
  await install(page);
  await openAgents(page);
  const task = settings(page).getByRole('textbox', { name: 'Task to route' });
  const button = settings(page).getByRole('button', { name: 'Route', exact: true });
  await page.evaluate(() => { (window as any).__routingHold = { model_routing_route: true }; });
  await task.fill('First independent request');
  await button.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);
  await task.fill('Second independent request');
  await button.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(2);
  await task.fill('Third independent request');
  await expect(button).toBeDisabled();
  // An old request finishing frees capacity for the current input; the other request remains in flight.
  await page.evaluate(() => (window as any).__routingReleases.shift()());
  await expect(button).toBeEnabled();
  await button.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(3);
  await page.evaluate(() => (window as any).__routingReleases.shift()());
  await expect(button).toBeDisabled();
  await page.evaluate(() => (window as any).__routingReleases.shift()());
  await expect(button).toBeEnabled();
});

test('a failed obsolete preview cannot overwrite a newer recommendation', async ({ page }) => {
  await install(page);
  await openAgents(page);
  const task = settings(page).getByRole('textbox', { name: 'Task to route' });
  const button = settings(page).getByRole('button', { name: 'Route', exact: true });
  const result = settings(page).locator('.routing-result');
  await page.evaluate(() => {
    (window as any).__routingHold = { model_routing_route: true };
    (window as any).__routingFail = { model_routing_route: 'Stale preview failed' };
  });
  await task.fill('Fix the obsolete login task');
  await button.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);

  // The first response remains held and will fail, but the latest task succeeds.
  await task.fill('Review the current release task');
  await page.evaluate(() => {
    (window as any).__routingHold.model_routing_route = false;
    delete (window as any).__routingFail.model_routing_route;
  });
  await button.click();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(2);
  await expect(result).toContainText('Difficulty');

  // A rejected obsolete request must not replace the newer valid recommendation.
  await page.evaluate(async () => {
    (window as any).__routingReleases.shift()();
    await new Promise(resolve => setTimeout(resolve, 0));
  });
  await expect(result).toContainText('Difficulty');
  await expect(result).not.toContainText('Stale preview failed');
  await expect(button).toBeEnabled();
});

test('changing mode hides the older preview before the delayed native save completes', async ({ page }) => {
  await install(page);
  await openAgents(page);
  await page.evaluate(() => {
    (window as any).__routingDelay = { model_routing_route: 350 };
    (window as any).__routingWriteDelays = [900];
  });
  await settings(page).getByRole('textbox', { name: 'Task to route' }).fill('Fix the failing login test');
  await settings(page).getByRole('button', { name: 'Route', exact: true }).click();
  await expect(settings(page).locator('.routing-result')).toHaveText('Routing…');
  await settings(page).getByRole('radio', { name: 'Cheap' }).click();
  // The old route finishes before the mode save. It must not show under the new selection.
  await page.waitForTimeout(500);
  await expect(settings(page).locator('.routing-result')).toBeHidden();
  await expect(settings(page).getByRole('radio', { name: 'Cheap' })).toHaveAttribute('aria-checked', 'true');
});

test('a routing status update cannot re-enable Route during a pending preview', async ({ page }) => {
  await install(page);
  await openAgents(page);
  const routeButton = settings(page).getByRole('button', { name: 'Route', exact: true });
  await page.evaluate(() => { (window as any).__routingDelay = { model_routing_route: 1200 }; });
  await settings(page).getByRole('textbox', { name: 'Task to route' }).fill('Fix the failing login test');
  await routeButton.click();
  await expect(routeButton).toBeDisabled();
  // Profiler status changes while the same task and routing policy remain selected.
  // A status-only render must not unlock a pending preview.
  const download = settings(page).getByRole('button', { name: 'Download', exact: true });
  await expect(download).toBeVisible();
  await download.click();
  await expect(settings(page).getByText('Ready', { exact: true })).toBeVisible();
  await expect(routeButton).toBeDisabled();
  await expect.poll(async () => (await calls(page, 'model_routing_route')).length).toBe(1);
  await expect(routeButton).toBeEnabled();
});

test('the catalog keeps every harness and long shared prefixes', async ({ page }) => {
  await install(page);
  const result = await page.evaluate(async () => {
    const { buildTeamCatalog } = await import('/src/team-catalog.ts');
    const long = 'x'.repeat(90);
    const models = (prefix: string, n: number) => Array.from({ length: n }, (_, i) => ({ id: `${prefix}${i}`, name: `M${i}` }));
    const harnesses = [
      { id: 'opencode', name: 'OpenCode', models: [...models('provider/', 700), { id: `${long}a`, name: 'A' }, { id: `${long}b`, name: 'B' }, { id: 'provider/1', name: 'dup' }] },
      { id: 'glm', name: 'GLM', models: models('glm-', 2) }, { id: 'grok', name: 'Grok', models: models('grok-', 2) }, { id: 'muse', name: 'Muse', models: models('muse-', 2) },
      { id: 'kimi', name: 'Kimi', models: models('kimi-', 100) },
    ];
    const { catalog, roster } = buildTeamCatalog(harnesses as any);
    return { count: catalog.length, ids: new Set(catalog.map(m => m.id)).size, providers: [...new Set(catalog.map(m => m.provider))], longBoth: catalog.filter(m => m.model.startsWith(long)).length, dups: catalog.filter(m => m.provider === 'opencode' && m.model === 'provider/1').length, valid: catalog.every(m => /^[a-z0-9_-]{1,100}$/.test(m.id)), roster: roster.length, rosterInCatalog: roster.every(r => catalog.some(c => c.id === r.id)) };
  });
  expect(result.count).toBeLessThanOrEqual(1000);
  expect(result.ids).toBe(result.count);
  expect(result.providers).toEqual(expect.arrayContaining(['opencode', 'glm', 'grok', 'muse', 'kimi']));
  expect(result.longBoth).toBe(2);
  expect(result.dups).toBe(1);
  expect(result.valid).toBe(true);
  expect(result.roster).toBeLessThanOrEqual(24);
  expect(result.rosterInCatalog).toBe(true);
});

test('invalid catalog entries cannot disable routing for the valid installed models', async ({ page }) => {
  await install(page);
  const result = await page.evaluate(async () => {
    const { buildTeamCatalog } = await import('/src/team-catalog.ts');
    const harnesses = [{ id: 'codex', name: 'Codex', models: [
      { id: 'valid-model', name: 'Valid' },
      { id: 'bad\u0000model', name: 'NUL' },
      { id: 'x'.repeat(201), name: 'Oversized' },
      { id: 'malformed-name', name: 'Invalid\u0000name' },
      { id: 'also-valid', name: 'Valid 2' },
    ] }];
    const { catalog } = buildTeamCatalog(harnesses as any);
    return catalog.map(p => p.model);
  });
  expect(result).toEqual(['valid-model', 'also-valid']);
});

test('the cap keeps every harness represented', async ({ page }) => {
  await install(page);
  const providers = await page.evaluate(async () => {
    const { buildTeamCatalog } = await import('/src/team-catalog.ts');
    const models = (prefix: string, n: number) => Array.from({ length: n }, (_, i) => ({ id: `${prefix}${i}`, name: `M${i}` }));
    const { catalog } = buildTeamCatalog([{ id: 'opencode', name: 'OpenCode', models: models('p/', 2000) }, { id: 'glm', name: 'GLM', models: models('glm-', 3) }] as any);
    return { count: catalog.length, glm: catalog.filter(m => m.provider === 'glm').length };
  });
  expect(providers).toEqual({ count: 1000, glm: 3 });
});

// The composer's Team button opens No team · Cheap · Balanced · Best; the kind is Panda Router's global mode.
const teamButton = (page: Page) => page.locator('#team-trigger');
const kind = (page: Page, name: string) => page.getByRole('menuitemradio', { name, exact: true });
async function pickTeam(page: Page, name: string): Promise<void> {
  await teamButton(page).click();
  await kind(page, name).click();
  await expect(kind(page, name)).toBeHidden();
}

test('Team opens a menu of team kinds, turns the team on with the uncapped catalog and prepares routing', async ({ page }) => {
  await install(page);
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  await expect(teamButton(page)).toHaveAttribute('aria-haspopup', 'dialog');
  await expect(teamButton(page)).not.toHaveAttribute('data-on');
  await teamButton(page).click();
  await expect(teamButton(page)).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('menuitemradio')).toHaveText([/No team/, /Cheap team/, /Balanced team/, /Best team/]);
  await expect(kind(page, 'No team')).toHaveAttribute('aria-checked', 'true');
  await kind(page, 'Cheap team').click();
  await expect(teamButton(page)).toHaveAccessibleName('Cheap team');
  await expect(teamButton(page)).toHaveAttribute('data-on', '');
  expect((await calls(page, 'model_routing_configure')).at(-1).args).toEqual({ enabled: true, mode: 'cheapest' });
  const { config } = (await calls(page, 'architect_configure')).at(-1).args;
  expect(config.auto).toBe(true);
  expect(config.workers.length).toBeLessThanOrEqual(24);
  expect(config.catalog.length).toBeGreaterThan(config.workers.length);
  await expect.poll(async () => (await calls(page, 'model_routing_prepare')).length).toBeGreaterThan(0);
  // Another kind only changes the mode; the team stays as it is.
  const configures = (await calls(page, 'architect_configure')).length;
  await teamButton(page).click();
  await expect(kind(page, 'Cheap team')).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('ArrowDown');
  await kind(page, 'Best team').press('Enter');
  await expect(teamButton(page)).toHaveAccessibleName('Best team');
  expect(await calls(page, 'architect_configure')).toHaveLength(configures);
  // Picking the current kind only closes the menu.
  const modes = (await calls(page, 'model_routing_configure')).length;
  await pickTeam(page, 'Best team');
  expect(await calls(page, 'model_routing_configure')).toHaveLength(modes);
  // No team turns the team off and leaves the mode for next time.
  await pickTeam(page, 'No team');
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  await expect(teamButton(page)).not.toHaveAttribute('data-on');
  expect((await calls(page, 'architect_configure')).at(-1).args.config).toBeNull();
  expect(await calls(page, 'model_routing_configure')).toHaveLength(modes);
});

test('the last team kind is remembered for new tasks and across reloads, and follows Settings both ways', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Best team');
  await page.locator('.task-item[data-id="session-1"]').click();
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  await teamButton(page).click();
  await expect(kind(page, 'Best team')).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByRole('menuitemradio', { checked: true })).toHaveText(/No team/);
  await page.keyboard.press('Escape');
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(teamButton(page)).toHaveAccessibleName('Best team');
  await openAgents(page);
  await expect(settings(page).getByRole('radio', { name: 'Best' })).toHaveAttribute('aria-checked', 'true');
  // A change in Settings reaches the button without a reload; the modal makes the composer inert, so read its text.
  await settings(page).getByRole('radio', { name: 'Balanced' }).click();
  await expect(teamButton(page)).toHaveText('Balanced team');
  // With routing off the lead picks models, and the button says only Team.
  await settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true }).uncheck();
  await expect(teamButton(page)).toHaveText('Team');
  await closeSettings(page);
  await teamButton(page).click();
  await expect(kind(page, 'Team')).toHaveAttribute('aria-checked', 'true');
  // A kind turns routing back on.
  await kind(page, 'Cheap team').click();
  await expect(teamButton(page)).toHaveAccessibleName('Cheap team');
  expect((await calls(page, 'model_routing_configure')).at(-1).args).toEqual({ enabled: true, mode: 'cheapest' });
  await openAgents(page);
  await expect(settings(page).getByRole('checkbox', { name: 'Pick worker models with Panda Router', exact: true })).toBeChecked();
  await expect(settings(page).getByRole('radio', { name: 'Cheap' })).toHaveAttribute('aria-checked', 'true');
});

test('a remote task never shows the Team menu', async ({ page }) => {
  await install(page);
  await expect(teamButton(page)).toBeVisible();
  await page.evaluate(() => (window as any).__previewMirrorTeam());
  await page.locator('.task-item[data-id="mirror-team"]').click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id', 'mirror-team');
  await expect(teamButton(page)).toHaveCount(0);
});

test('an unknown mode reads Team and checks no kind', async ({ page }) => {
  await install(page, () => localStorage.setItem('pandacode.preview.routing', JSON.stringify({ mode: 'fastest' })));
  await teamButton(page).click();
  await kind(page, 'No team').click();
  await expect(kind(page, 'No team')).toBeHidden();
  await teamButton(page).click();
  await expect(page.getByRole('menuitemradio', { checked: true })).toHaveText(/No team/);
  await expect(page.getByRole('menuitemradio')).toHaveCount(4);
});

test('a refused mode change after the team turned on shows the menu error and keeps the team on', async ({ page }) => {
  await install(page);
  await page.evaluate(() => { (window as any).__routingFail = { model_routing_configure: 'Routing settings could not be saved.' }; });
  await teamButton(page).click();
  await kind(page, 'Cheap team').click();
  await expect(page.locator('.choice-menu-error')).toHaveText('Routing settings could not be saved.');
  // The team turned on first; the mode stays Balanced.
  expect((await calls(page, 'architect_configure')).at(-1).args.config.auto).toBe(true);
  await expect(teamButton(page)).toHaveAttribute('data-on', '');
  await expect(teamButton(page)).toHaveText('Balanced team');
});

test('a team that cannot turn on leaves the global mode alone', async ({ page }) => {
  await install(page);
  await page.evaluate(() => { (window as any).__routingFail = { architect_configure: 'The team could not be saved.' }; });
  await teamButton(page).click();
  await kind(page, 'Best team').click();
  await expect(page.locator('.choice-menu-error')).toHaveText('The team could not be saved.');
  expect(await calls(page, 'model_routing_configure')).toHaveLength(0);
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  await expect(teamButton(page)).not.toHaveAttribute('data-on');
});

test('with routing off the menu offers a lead-picks team, and kinds say they turn routing on', async ({ page }) => {
  await install(page, () => localStorage.setItem('pandacode.preview.routing', JSON.stringify({ enabled: false })));
  await teamButton(page).click();
  await expect(page.getByRole('menuitemradio')).toHaveText([/No team/, /Team/, /Cheap team/, /Balanced team/, /Best team/]);
  // The kinds keep their own details; the group heading says they turn routing on.
  await expect(page.getByRole('group', { name: 'Worker models for every team · turns routing on' })).toBeVisible();
  await expect(kind(page, 'Cheap team')).toHaveAccessibleDescription('Cheapest model likely to pass');
  await expect(kind(page, 'Best team')).toHaveAccessibleDescription('Strongest model for each worker');
  await expect(kind(page, 'Team')).toHaveAccessibleDescription('The lead picks models');
  // The lead-picks team leaves routing off.
  await kind(page, 'Team').click();
  await expect(teamButton(page)).toHaveAttribute('data-on', '');
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  expect(await calls(page, 'model_routing_configure')).toHaveLength(0);
  // A kind turns routing on, keeping the team.
  const configures = (await calls(page, 'architect_configure')).length;
  await pickTeam(page, 'Cheap team');
  await expect(teamButton(page)).toHaveAccessibleName('Cheap team');
  expect((await calls(page, 'model_routing_configure')).at(-1).args).toEqual({ enabled: true, mode: 'cheapest' });
  expect(await calls(page, 'architect_configure')).toHaveLength(configures);
  await teamButton(page).click();
  await expect(kind(page, 'Team')).toHaveCount(0);
  await expect(kind(page, 'Balanced team')).toHaveAccessibleDescription('Best value for the cost');
});

test('a status read during a pending change never overrides it', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Cheap team');
  await page.evaluate(() => { (window as any).__routingDelay = { model_routing_configure: 800 }; });
  await teamButton(page).click();
  await kind(page, 'Balanced team').click();
  // The read starts after the change and answers first, with the old mode.
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
  await page.waitForTimeout(300);
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
  await page.evaluate(() => { delete (window as any).__routingDelay; window.dispatchEvent(new Event('focus')); });
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
});

test('without a routing status the menu still offers every choice, and recovers on the next render or focus', async ({ page }) => {
  await install(page, () => { (window as any).__routingFail = { model_routing_status: 'Router unavailable.' }; });
  await teamButton(page).click();
  await expect(page.getByRole('menuitemradio')).toHaveText([/No team/, /Team/, /Cheap team/, /Balanced team/, /Best team/]);
  await expect(page.getByRole('group', { name: 'Worker models for every team · turns routing on' })).toBeVisible();
  await kind(page, 'Team').click();
  await expect(teamButton(page)).toHaveAccessibleName('Team');
  await expect(teamButton(page)).toHaveAttribute('data-on', '');
  expect((await calls(page, 'architect_configure')).at(-1).args.config.auto).toBe(true);
  expect(await calls(page, 'model_routing_configure')).toHaveLength(0);
  await page.evaluate(() => { delete (window as any).__routingFail; window.dispatchEvent(new Event('focus')); });
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
  // A render recovers too, and once loaded renders ask nothing.
  await page.evaluate(() => { (window as any).__routingFail = { model_routing_status: 'Router unavailable.' }; });
  await page.reload();
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id', 'session-2');
  await page.evaluate(() => { delete (window as any).__routingFail; });
  // Renders retry a failed load at most every 3 s (ROUTING_RETRY_MS).
  await page.waitForTimeout(3100);
  await page.locator('.task-item[data-id="session-1"]').click();
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
  const loads = (await calls(page, 'model_routing_status')).length;
  await page.locator('.task-item[data-id="session-1"]').click();
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(teamButton(page)).toHaveAccessibleName('Balanced team');
  expect(await calls(page, 'model_routing_status')).toHaveLength(loads);
});

test('a routing change keeps the prompt focus and caret', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Cheap team');
  const prompt = page.locator('#prompt');
  await prompt.fill('hello world'); await prompt.focus();
  await prompt.evaluate((el: HTMLTextAreaElement) => el.setSelectionRange(5, 5));
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('pandacode:routing', { detail: { enabled: true, mode: 'quality', minP: 0.5, profiler: 'ready', error: null } })));
  await expect(teamButton(page)).toHaveAccessibleName('Best team');
  expect(await prompt.evaluate((el: HTMLTextAreaElement) => [document.activeElement === el, el.selectionStart, el.value])).toEqual([true, 5, 'hello world']);
});

test('the Team button keeps its label and fits the toolbar from wide to the narrowest window', async ({ page }) => {
  await install(page);
  await pickTeam(page, 'Balanced team');
  for (const theme of ['light', 'dark']) {
    if (await page.locator('html').getAttribute('data-theme') !== theme) await page.locator('[data-action="toggle-theme"]').click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    // 860 is the app's minimum window width (tauri.conf.json); 600 is the browser preview's narrow layout (icon only).
    for (const width of [1280, 860, 600]) {
      await page.setViewportSize({ width, height: 800 });
      const layout = await page.evaluate(() => {
        const toolbar = document.querySelector('.composer-toolbar')!, tools = document.querySelector('.composer-tools')!.getBoundingClientRect();
        const button = document.querySelector('#team-trigger')!, box = button.getBoundingClientRect(), mode = document.querySelector('#mode-picker-trigger')!.getBoundingClientRect();
        const label = button.querySelector('span')!;
        return { toolsRight: tools.right, right: box.right, overflow: toolbar.scrollWidth - toolbar.clientWidth, center: box.top + box.height / 2, modeCenter: mode.top + mode.height / 2, shown: label.offsetWidth > 0, fits: label.scrollWidth <= label.clientWidth };
      });
      expect(layout.shown).toBe(width > 650);
      expect(layout.fits).toBe(true);
      expect(layout.right).toBeLessThanOrEqual(layout.toolsRight);
      expect(layout.overflow).toBeLessThanOrEqual(0);
      expect(Math.abs(layout.center - layout.modeCenter)).toBeLessThanOrEqual(2);
    }
  }
});
