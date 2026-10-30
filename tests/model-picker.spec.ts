import { expect, test, type Page } from '@playwright/test';

const trigger = (page: Page) => page.locator('#model-picker-trigger');
const picker = (page: Page) => page.getByRole('dialog', { name: 'Harness and model', exact: true });
const search = (page: Page) => picker(page).getByRole('combobox', { name: 'Search harnesses and models', exact: true });
const task = (page: Page) => page.evaluate(async () => {
  const { invoke } = await import((window as any).__pandacodePreviewUrl);
  const id = document.querySelector<HTMLElement>('#prompt')?.dataset.sessionId;
  return invoke('get_task', { sessionId: id });
});
const activeOption = (page: Page) => picker(page).locator('[role="option"][aria-selected="true"]');

async function open(page: Page, url = '/?teams'): Promise<void> {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, body: `${await response.text()}\nwindow.__pandacodePreviewUrl = import.meta.url;\n` });
  });
  await page.goto(url);
  await expect(page.locator('.task-item.selected')).toHaveAttribute('data-id', 'session-1');
  await trigger(page).click();
  await expect(picker(page)).toBeVisible();
}

// Issue #123: the picker lists only installed harnesses. The preview marks
// Antigravity and Oh My Pi as not installed.
test('remote harness discovery errors never expose private diagnostics', async ({ page }) => {
  await page.goto('/?teams');
  await page.evaluate(async () => {
    // Use the real picker with a rejected remote discovery request, not mocked markup.
    const modulePath = '/src/model-picker.ts';
    const { openModelPicker } = await import(modulePath);
    const anchor = document.createElement('button');
    anchor.id = 'remote-harness-error-trigger';
    anchor.style.cssText = 'position:fixed;top:16px;left:16px';
    document.body.append(anchor);
    (window as any).__closeRemoteHarnessPicker = openModelPicker({
      anchor,
      providers: [{ id: 'codex', name: 'Codex', installed: true, models: [] }],
      current: { provider: 'codex', model: 'default' },
      canSwitchProvider: true,
      onSelect: async () => {},
      loadProviders: async () => { throw new Error('ENOENT /private/credentials?api_key=DO_NOT_DISPLAY'); },
    });
  });
  await expect(picker(page)).toContainText('Could not load harnesses: Provider unavailable');
  await expect(picker(page)).not.toContainText('DO_NOT_DISPLAY');
  await expect(picker(page)).not.toContainText('/private/credentials');
  await page.evaluate(() => (window as any).__closeRemoteHarnessPicker?.());
});

test('HTTP 403 and 429 discovery errors give actionable, non-sensitive labels', async ({ page }) => {
  await page.goto('/?teams');
  for (const { detail, label } of [
    { detail: 'HTTP 403 Forbidden; bearer DO_NOT_DISPLAY', label: 'Access denied' },
    { detail: 'HTTP 403 Forbidden; please verify credentials DO_NOT_DISPLAY', label: 'Access denied' },
    { detail: 'HTTP 429 Too Many Requests; token DO_NOT_DISPLAY', label: 'Rate limited' },
    { detail: 'HTTP 429 Too Many Requests; credential rotation DO_NOT_DISPLAY', label: 'Rate limited' },
  ]) {
    await page.evaluate(async reason => {
      const { openModelPicker } = await import('/src/model-picker.ts');
      const anchor = document.createElement('button');
      anchor.style.cssText = 'position:fixed;top:16px;left:16px';
      document.body.append(anchor);
      (window as any).__closeDiscoveryPicker = openModelPicker({
        anchor,
        providers: [{ id: 'codex', name: 'Codex', installed: true, models: [] }],
        current: { provider: 'codex', model: 'default' },
        canSwitchProvider: true,
        onSelect: async () => {},
        loadProviders: async () => { throw new Error(reason); },
      });
    }, detail);
    await expect(picker(page)).toContainText(`Could not load harnesses: ${label}`);
    await expect(picker(page)).not.toContainText('DO_NOT_DISPLAY');
    await page.evaluate(() => (window as any).__closeDiscoveryPicker?.());
  }
});

test('the current custom model is not advertised by another named connection', async ({ page }) => {
  await page.goto('/?teams');
  await page.evaluate(async () => {
    const modulePath = '/src/model-picker.ts';
    const { openModelPicker } = await import(modulePath);
    const anchor = document.createElement('button');
    anchor.id = 'named-connection-picker-trigger';
    anchor.style.cssText = 'position:fixed;top:16px;left:16px';
    document.body.append(anchor);
    (window as any).__closeConnectionPicker = openModelPicker({
      anchor,
      providers: [{ id: 'codex', name: 'Codex', installed: true, models: [] }],
      current: { provider: 'codex', model: 'private-model-a' },
      currentConnection: 'account-a',
      connections: [
        { id: 'account-a', name: 'Account A', provider: 'codex' },
        { id: 'account-b', name: 'Account B', provider: 'codex' },
      ],
      canSwitchProvider: true,
      onSelect: async () => {},
    });
  });
  const menu = picker(page);
  await expect(menu.locator('[role="option"][data-model="private-model-a"]')).toHaveCount(1);
  await menu.getByRole('tab', { name: 'Account B', exact: true }).click();
  await expect(menu.locator('[role="option"][data-model="private-model-a"]')).toHaveCount(0);
  await menu.getByRole('tab', { name: 'Codex', exact: true }).click();
  await expect(menu.locator('[role="option"][data-model="private-model-a"]')).toHaveCount(0);
  await menu.getByRole('tab', { name: 'Account A', exact: true }).click();
  await expect(menu.locator('[role="option"][data-model="private-model-a"]')).toHaveCount(1);
  await expect(menu.locator('[role="option"][data-model="private-model-a"]')).toHaveAttribute('aria-current', 'true');
  await page.evaluate(() => (window as any).__closeConnectionPicker?.());
});

test('Recent remembers the named connection when choosing its model again', async ({ page }) => {
  await page.goto('/?teams');
  await page.evaluate(async () => {
    const modulePath = '/src/model-picker.ts';
    const { openModelPicker } = await import(modulePath);
    const anchor = document.createElement('button');
    anchor.id = 'recent-connection-trigger';
    anchor.style.cssText = 'position:fixed;top:16px;left:16px';
    document.body.append(anchor);
    (window as any).__openNamedRecent = (connection: string) => openModelPicker({
      anchor,
      providers: [{ id: 'codex', name: 'Codex', installed: true, models: [{ id: 'model-a', name: 'Model A' }, { id: 'model-b', name: 'Model B' }] }],
      connections: [
        { id: 'account-a', name: 'Account A', provider: 'codex' },
        { id: 'account-b', name: 'Account B', provider: 'codex' },
      ],
      current: { provider: 'codex', model: 'default' },
      currentConnection: connection,
      canSwitchProvider: true,
      onSelect: async (provider: string, model: string, selectedConnection: string | null) => {
        (window as any).__lastNamedRecent = [provider, model, selectedConnection];
      },
    });
    (window as any).__openNamedRecent('account-a');
  });
  await picker(page).getByRole('option', { name: 'Account A · Model A', exact: true }).click();
  await expect(picker(page)).toHaveCount(0);
  await page.evaluate(() => (window as any).__openNamedRecent('account-b'));
  await picker(page).getByRole('tab', { name: 'Recent', exact: true }).click();
  await picker(page).getByRole('option', { name: 'Account A · Model A', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__lastNamedRecent)).toEqual(['codex', 'model-a', 'account-a']);
});

test('the harness picker hides harnesses that are not installed', async ({ page }) => {
  await page.goto('/');
  await page.locator('.new-task-button').click();
  await page.getByRole('button', { name: 'Choose harness and model', exact: true }).click();
  await expect(picker(page).getByRole('tab', { name: 'Grok Build' })).toBeVisible();
  await expect(picker(page).locator('[data-provider="antigravity"], [data-provider="omp"]')).toHaveCount(0);
  await expect(picker(page)).not.toContainText('Antigravity');
  await expect(picker(page)).not.toContainText('Oh My Pi');
});

test('opens on the current harness with one-line rows, a scrollable rail and a focused search', async ({ page }) => {
  await open(page);
  await expect(search(page)).toBeFocused();
  await expect(search(page)).toHaveAttribute('placeholder', /^Search \d+ models, \d+ harnesses…$/);
  const box = (await picker(page).boundingBox())!;
  expect(box.width).toBe(460);
  expect(box.height).toBeLessThanOrEqual(440);
  const rail = picker(page).getByRole('tablist', { name: 'Harnesses' });
  await expect(rail.getByRole('tab', { name: 'Claude Code', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(rail.getByRole('tab', { name: 'Teams, 2' })).toBeVisible();
  // Every harness is reachable: the last one scrolls into view.
  await rail.getByRole('tab').last().scrollIntoViewIfNeeded();
  await expect(rail.getByRole('tab').last()).toBeInViewport();
  const list = picker(page).getByRole('listbox');
  await expect(list.getByRole('option').first()).toHaveAccessibleName('Claude Code · Default');
  await expect(list.locator('[aria-current="true"]')).toHaveCount(1);
  await expect(activeOption(page)).toHaveAttribute('aria-current', 'true');
  await expect(search(page)).toHaveAttribute('aria-activedescendant', (await activeOption(page).getAttribute('id'))!);
  for (const height of await list.getByRole('option').evaluateAll(rows => rows.map(row => row.getBoundingClientRect().height))) expect(height).toBe(28);
  // The harness logo shows once, in the heading.
  await expect(list.locator('.model-menu-option .provider-logo')).toHaveCount(0);
  // Instant: no transitions or running animations in the panel.
  expect(await picker(page).evaluate(panel => [panel, ...panel.querySelectorAll('*')].filter(node => getComputedStyle(node).transitionDuration.split(',').some(value => parseFloat(value) > 0)).length)).toBe(0);
});

test('search flattens matches across harnesses and the rail becomes a hit map', async ({ page }) => {
  await open(page);
  await page.keyboard.type('sonne');
  await expect(search(page)).toHaveValue('sonne');
  const rail = picker(page).getByRole('tablist');
  await expect(rail.getByRole('tab', { name: 'Codex', exact: true })).toHaveClass(/is-dim/);
  await expect(rail.getByRole('tab', { name: 'Claude Code', exact: true }).locator('.model-menu-count')).toHaveText('1');
  await expect(activeOption(page)).toHaveAccessibleName('Claude Code · Claude Sonnet 5');
  // One custom row, for the viewed harness, after the real hits.
  await expect(picker(page).getByRole('option', { name: /^Use "/ })).toHaveCount(1);
  await expect(picker(page).getByRole('option').last()).toHaveAccessibleName('Use "sonne" with Claude Code');
  await expect(picker(page).locator('.model-menu-empty')).toHaveCount(0);
  // A query that names a harness offers no custom row.
  await search(page).fill('codex');
  await expect(picker(page).getByRole('option', { name: /^Use "/ })).toHaveCount(0);
  await search(page).fill('no-such-thing-at-all');
  await expect(picker(page).locator('.model-menu-empty')).toHaveText('No matching models.');
  await expect(picker(page).getByRole('option')).toHaveCount(1);
  await expect(picker(page).getByRole('option').first()).toHaveAccessibleName('Use "no-such-thing-at-all" with Claude Code');
  await search(page).fill('Ship');
  await expect(picker(page).getByRole('option').first()).toHaveAccessibleName('Agent team · Ship crew');
});

test('keyboard: arrows move rows, left/right change harness, Enter picks, Tab cycles without closing', async ({ page }) => {
  await open(page);
  await page.keyboard.press('ArrowRight');
  await expect(picker(page).getByRole('tab', { name: 'Codex', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(activeOption(page)).toHaveAccessibleName('Codex · Default');
  await page.keyboard.press('ArrowUp');
  const last = await activeOption(page).getAttribute('aria-label');
  await page.keyboard.press('ArrowDown');
  await expect(activeOption(page)).toHaveAccessibleName('Codex · Default');
  expect(last).not.toBe('Codex · Default');
  await page.keyboard.press('End');
  await expect(activeOption(page)).toHaveAccessibleName(last!);
  // Tab: search → rail → effort → search, never closing.
  await page.keyboard.press('Tab');
  await expect(picker(page).getByRole('tab', { name: 'Codex', exact: true })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(picker(page).getByRole('radio', { name: 'Default', exact: true })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(search(page)).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(picker(page).getByRole('radio', { name: 'Default', exact: true })).toBeFocused();
  await expect(picker(page)).toBeVisible();
  // Typing anywhere goes to search.
  await page.keyboard.type('astra');
  await expect(search(page)).toHaveValue('astra');
  await expect(activeOption(page)).toHaveAccessibleName('Codex · GPT-6-Astra');
  await page.keyboard.press('Enter');
  await expect(picker(page)).toHaveCount(0);
  await expect(trigger(page)).toContainText('GPT-6-Astra');
  expect(await task(page)).toMatchObject({ provider: 'codex', model: 'gpt-6-astra' });
});

test('reasoning effort save errors redact provider details and preserve the selected effort', async ({ page }) => {
  await page.goto('/?teams');
  await page.evaluate(async () => {
    const { openModelPicker } = await import('/src/model-picker.ts');
    const anchor = document.createElement('button');
    anchor.style.cssText = 'position:fixed;top:16px;left:16px';
    document.body.append(anchor);
    (window as any).__closeEffortErrorPicker = openModelPicker({
      anchor,
      providers: [{ id: 'codex', name: 'Codex', installed: true, models: [{
        id: 'sample-model', name: 'Sample Model',
        reasoningEfforts: [{ id: 'high', name: 'High', description: 'High effort' }],
      }] }],
      current: { provider: 'codex', model: 'sample-model', reasoningEffort: null },
      canSwitchProvider: true,
      onSelect: async () => {},
      onEffortSelect: async () => { throw new Error('HTTP 429 Too Many Requests; INTERNAL_MARKER_SHOULD_NOT_RENDER'); },
    });
  });
  const group = picker(page).getByRole('radiogroup', { name: 'Effort' });
  await group.getByRole('radio', { name: 'High', exact: true }).click();
  await expect(picker(page).getByRole('alert')).toHaveText('Rate limited');
  await expect(picker(page)).not.toContainText('INTERNAL_MARKER_SHOULD_NOT_RENDER');
  await expect(group.getByRole('radio', { name: 'Default', exact: true })).toHaveAttribute('aria-checked', 'true');
  await page.evaluate(() => (window as any).__closeEffortErrorPicker?.());
});

test('effort is a segmented radiogroup that saves on choice', async ({ page }) => {
  await open(page);
  const group = picker(page).getByRole('radiogroup', { name: 'Effort' });
  await expect(group.getByRole('radio', { name: 'Default', exact: true })).toHaveAttribute('aria-checked', 'true');
  await expect(group.locator('[data-model-default]')).toHaveText('Medium');
  await expect(group.locator('.segment-thumb')).toHaveCount(1);
  await group.getByRole('radio', { name: 'High', exact: true }).click();
  await expect(group.getByRole('radio', { name: 'High', exact: true })).toHaveAttribute('aria-checked', 'true');
  await expect.poll(async () => (await task(page)).reasoningEffort).toBe('high');
  await expect(picker(page).locator('.model-menu-effort-hint')).toHaveText('Sample high effort.');
  await expect(trigger(page).locator('.model-effort')).toHaveText('High');
  // Arrow keys step and save.
  await group.getByRole('radio', { name: 'High', exact: true }).focus();
  await page.keyboard.press('ArrowLeft');
  await expect.poll(async () => (await task(page)).reasoningEffort).toBe('medium');
  await page.keyboard.press('Home');
  await expect.poll(async () => (await task(page)).reasoningEffort).toBeNull();
  await expect(trigger(page).locator('.model-effort')).toBeHidden();
  await expect(picker(page)).toBeVisible();
});

test('teams: picking one sets the team, a plain model clears it', async ({ page }) => {
  await open(page);
  await picker(page).getByRole('tab', { name: 'Teams, 2' }).click();
  const ship = picker(page).getByRole('option', { name: 'Agent team · Ship crew', exact: true });
  await expect(ship).toContainText('3 workers');
  await expect(ship.locator('.provider-logo')).toHaveCount(3);
  await ship.click();
  await expect(picker(page)).toHaveCount(0);
  await expect(trigger(page)).toContainText('Ship crew');
  await expect(trigger(page).locator('.model-menu-stack .provider-logo')).toHaveCount(3);
  await trigger(page).click();
  await expect(picker(page).getByRole('tab', { name: 'Teams, 2' })).toHaveAttribute('aria-selected', 'true');
  await expect(picker(page).getByRole('option', { name: 'Agent team · Ship crew', exact: true })).toHaveAttribute('aria-current', 'true');
  await expect(picker(page).locator('.model-menu-settings')).toContainText('Set per worker');
  await expect(picker(page).getByRole('radiogroup')).toHaveCount(0);
  await picker(page).getByRole('tab', { name: 'Claude Code', exact: true }).click();
  await picker(page).getByRole('option', { name: 'Claude Code · Claude Opus 5', exact: true }).click();
  await expect(picker(page)).toHaveCount(0);
  await expect(trigger(page)).not.toContainText('Ship crew');
  await expect(trigger(page)).toContainText('Claude Opus 5');
  await expect(trigger(page).locator('.model-menu-stack')).toHaveCount(0);
});

test('Esc clears the query first, then closes and returns focus to the trigger', async ({ page }) => {
  await open(page);
  await page.keyboard.type('opus');
  await page.keyboard.press('Escape');
  await expect(search(page)).toHaveValue('');
  await expect(picker(page)).toBeVisible();
  await expect(picker(page).getByRole('tab', { name: 'Claude Code', exact: true })).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Escape');
  await expect(picker(page)).toHaveCount(0);
  await expect(trigger(page)).toBeFocused();
});

test('the model picker stays inside a 320px viewport with search and keyboard selection', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 640 });
  await open(page);
  const menu = picker(page);
  await expect.poll(async () => {
    const box = await menu.boundingBox();
    return box !== null && box.x >= 0 && box.x + box.width <= 320;
  }).toBe(true);
  await expect(search(page)).toBeFocused();
  const rail = menu.getByRole('tablist', { name: 'Harnesses' });
  await expect(rail.getByRole('tab', { name: 'Claude Code', exact: true })).toBeVisible();
  await search(page).fill('opus');
  // A team whose description mentions Opus can legitimately appear before the model.
  // Exercise actual keyboard navigation to the requested model instead of assuming row zero.
  const target = menu.getByRole('option', { name: 'Claude Code · Claude Opus 5', exact: true });
  await expect(target).toBeVisible();
  const targetIndex = await menu.locator('.model-menu-option:not(:disabled)').evaluateAll(rows =>
    rows.findIndex(row => row.getAttribute('aria-label') === 'Claude Code · Claude Opus 5'));
  expect(targetIndex).toBeGreaterThanOrEqual(0);
  await search(page).press('Home');
  for (let index = 0; index < targetIndex; index++) await search(page).press('ArrowDown');
  await expect(activeOption(page)).toHaveAccessibleName('Claude Code · Claude Opus 5');
  await search(page).press('Enter');
  await expect(menu).toHaveCount(0);
  await expect(trigger(page)).toContainText('Claude Opus 5');
});

test('narrow windows collapse the rail to logos', async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 800 });
  await open(page);
  const tab = picker(page).getByRole('tab', { name: 'Claude Code', exact: true });
  expect((await tab.boundingBox())!.width).toBeLessThan(50);
  await expect(tab.locator('.model-menu-tab-label')).toBeHidden();
});

test('model selection failure does not expose provider credentials or local paths', async ({ page }) => {
  await page.goto('/?teams');
  await page.evaluate(async () => {
    const { openModelPicker } = await import('/src/model-picker.ts');
    const anchor = document.createElement('button');
    anchor.style.cssText = 'position:fixed;top:16px;left:16px';
    document.body.append(anchor);
    (window as any).__closeSelectionErrorPicker = openModelPicker({
      anchor,
      providers: [{ id: 'codex', name: 'Codex', installed: true, models: [{ id: 'model-a', name: 'Model A' }] }],
      current: { provider: 'codex', model: 'default' },
      canSwitchProvider: true,
      onSelect: async () => { throw new Error('HTTP 401 /private/credentials?token=DO_NOT_DISPLAY'); },
    });
  });
  await picker(page).getByRole('option', { name: 'Codex · Model A', exact: true }).click();
  await expect(picker(page).locator('.model-menu-error')).toHaveText('Login required');
  await expect(picker(page)).not.toContainText('DO_NOT_DISPLAY');
  await expect(picker(page)).not.toContainText('/private/credentials');
  await page.evaluate(() => (window as any).__closeSelectionErrorPicker?.());
});
