import { expect, test, type Page } from '@playwright/test';

const settings = (page: Page) => page.getByRole('dialog', { name: 'Scout settings' });
async function createBot(page: Page, mode = 'ask'): Promise<void> {
  await page.goto('/');
  await page.locator('.sidebar-bots-bar').click();
  await page.getByRole('region', { name: 'Bots', exact: true }).getByRole('button', { name: 'Add bot', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Bots' });
  await dialog.getByRole('button', { name: /Researcher/ }).click();
  await dialog.getByRole('textbox', { name: 'Name', exact: true }).fill('Scout');
  await dialog.getByRole('textbox', { name: 'Soul', exact: true }).fill('Precise, candid and warm.');
  await dialog.getByRole('combobox', { name: 'Permissions' }).selectOption(mode);
  await dialog.getByRole('button', { name: 'Create bot' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#breadcrumb .bot-crumb')).toContainText('Scout');
}
async function openSettings(page: Page): Promise<void> {
  await page.locator('#breadcrumb').getByRole('button', { name: 'Scout settings', exact: true }).click();
  await expect(settings(page)).toBeVisible();
}
async function closeSettings(page: Page): Promise<void> {
  await page.keyboard.press('Escape');
  await expect(settings(page)).toBeHidden();
}

test('personality imports, saves and removes without changing instructions', async ({ page }) => {
  await createBot(page); await openSettings(page);
  const soul = settings(page).getByRole('textbox', { name: 'Soul', exact: true });
  const instructions = await settings(page).getByRole('textbox', { name: 'Instructions', exact: true }).inputValue();
  await expect(soul).toHaveValue('Precise, candid and warm.');
  await settings(page).locator('[data-soul-file]').setInputFiles({ name: 'soul.md', mimeType: 'text/markdown', buffer: Buffer.from('\uFEFF# Scout\nWarm, focused — never vague.') });
  await expect(soul).toHaveValue('# Scout\nWarm, focused — never vague.');
  await settings(page).getByRole('button', { name: 'Save', exact: true }).click();
  await closeSettings(page); await openSettings(page);
  await expect(soul).toHaveValue('# Scout\nWarm, focused — never vague.');
  await expect(settings(page).getByRole('textbox', { name: 'Instructions', exact: true })).toHaveValue(instructions);
  await settings(page).getByRole('button', { name: 'Remove personality' }).click();
  await expect(soul).toHaveValue('');
  await settings(page).getByRole('button', { name: 'Save', exact: true }).click();
  await closeSettings(page); await openSettings(page);
  await expect(soul).toHaveValue('');
});

test('late personality imports cannot overwrite a closed and reopened profile', async ({ page }) => {
  await createBot(page); await openSettings(page);
  await page.evaluate(() => {
    const original = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = function () {
      if (this.name !== 'delayed-soul.md') return original.call(this);
      return new Promise<ArrayBuffer>(resolve => { (window as any).__finishSoulRead = () => resolve(new TextEncoder().encode('Stale imported personality').buffer); });
    };
  });
  await settings(page).locator('[data-soul-file]').setInputFiles({ name: 'delayed-soul.md', mimeType: 'text/markdown', buffer: Buffer.from('Stale imported personality') });
  await closeSettings(page); await openSettings(page);
  const soul = settings(page).getByRole('textbox', { name: 'Soul', exact: true });
  await soul.fill('Current unsaved personality');
  await page.evaluate(() => (window as any).__finishSoulRead());
  await expect(soul).toHaveValue('Current unsaved personality');
});

test('a stale bot detail response cannot change the target of a profile save', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.task-item[data-id="session-1"]')).toBeVisible();
  const ids = await page.evaluate(async () => {
    const { invoke } = await import('/src/preview.ts');
    const { mountBots } = await import('/src/bots.ts');
    const input = { role: '', instructions: '', goals: '', soul: 'Original personality', avatar: '', provider: 'claude', model: 'default', mode: 'ask', heartbeatMinutes: 0 };
    const first = await invoke<any>('bot_create', { input: { ...input, name: 'Alpha' } });
    const second = await invoke<any>('bot_create', { input: { ...input, name: 'Beta' } });
    const host = document.createElement('div'); host.id = 'bot-race-fixture';
    host.style.cssText = 'position:fixed;inset:0;z-index:100000;background:var(--surface);overflow:auto'; document.body.append(host);
    let delayBeta = true;
    mountBots(host, {
      call: async (command: string, args: any = {}) => {
        const result = await invoke<any>(command, args);
        if (command === 'bot_detail' && args.botId === second.id && delayBeta) {
          delayBeta = false;
          await new Promise<void>(resolve => { (window as any).__releaseBotDetail = resolve; });
        }
        return result;
      },
      providers: await invoke<any[]>('list_providers', {}), defaultModel: () => 'default', loadModels: async () => [],
      running: () => false, openTask() {}, openChat() {}, notify() {}, initialBot: first.id,
    });
    return { first: first.id, second: second.id };
  });
  const fixture = page.locator('#bot-race-fixture');
  await expect(fixture.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('Alpha');
  await fixture.locator(`[data-bot="${ids.second}"]`).click();
  await expect.poll(() => page.evaluate(() => Boolean((window as any).__releaseBotDetail))).toBe(true);
  await fixture.locator(`[data-bot="${ids.first}"]`).click();
  await expect(fixture.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('Alpha');
  await fixture.getByRole('textbox', { name: 'Soul', exact: true }).fill('Alpha changed this personality');
  await page.evaluate(async () => { (window as any).__releaseBotDetail(); await new Promise(resolve => setTimeout(resolve, 0)); });
  await fixture.getByRole('button', { name: 'Save', exact: true }).click();
  await expect.poll(() => page.evaluate(async id => { const { invoke } = await import('/src/preview.ts'); return (await invoke<any>('bot_detail', { botId: id })).bot.soul; }, ids.first)).toBe('Alpha changed this personality');
  const untouched = await page.evaluate(async id => { const { invoke } = await import('/src/preview.ts'); return (await invoke<any>('bot_detail', { botId: id })).bot; }, ids.second);
  expect(untouched.name).toBe('Beta');
  expect(untouched.soul).toBe('Original personality');
});

test('bot settings save pinned Skills and configured MCP names independently', async ({ page }) => {
  await createBot(page); await openSettings(page);
  await settings(page).getByRole('tab', { name: 'Skills & MCP', exact: true }).click();
  await settings(page).getByRole('checkbox', { name: 'Assign Release notes' }).check();
  await settings(page).getByRole('button', { name: 'Save skill selection' }).click();
  await expect(settings(page)).toContainText('Selection saved for future turns.');
  await settings(page).getByRole('checkbox', { name: 'Use MCP Preview documentation' }).check();
  await settings(page).getByRole('button', { name: 'Save MCP selection' }).click();
  await closeSettings(page); await openSettings(page);
  await settings(page).getByRole('tab', { name: 'Skills & MCP', exact: true }).click();
  await expect(settings(page).getByRole('checkbox', { name: 'Assign Release notes' })).toBeChecked();
  await expect(settings(page).getByRole('checkbox', { name: 'Use MCP Preview documentation' })).toBeChecked();
  await expect(settings(page)).toContainText('Selected version');
  const saved = await page.evaluate(async () => {
    const { invoke } = await import('/src/preview.ts');
    const [bot] = await invoke<any[]>('bots_list', {});
    return { bot, pins: await invoke<any[]>('skills_scope_get', { target: 'bot', targetId: bot.id }) };
  });
  expect(saved.pins).toHaveLength(1);
  expect(saved.pins[0]).toMatchObject({ id: 'release-notes', sha256: expect.any(String) });
  expect(saved.bot.mcp).toEqual(['Preview documentation']);
  expect(saved.bot).not.toHaveProperty('skills');
});

test('a project task binds the bot natively while keeping its own permissions and transcript', async ({ page }) => {
  await createBot(page, 'full');
  await page.locator('.sidebar-bots-bar').click();
  await page.locator('.task-item[data-id="session-1"]').click();
  await page.locator('.new-task-button').click();
  await expect(page.getByRole('heading', { name: /^What should we build in/ })).toBeVisible();
  const project = await page.locator('#welcome-project').textContent();
  const taskId = await page.locator('#prompt').getAttribute('data-session-id');
  const original = await page.evaluate(async sessionId => { const { invoke } = await import('/src/preview.ts'); return invoke<any>('get_task', { sessionId }); }, taskId);
  await page.getByRole('button', { name: 'Bot: No bot', exact: true }).click();
  await page.getByRole('menuitemradio', { name: 'Scout', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Bot: Scout', exact: true })).toBeVisible();
  await expect(page.locator('#welcome-project')).toHaveText(project!);
  await page.locator('#prompt').fill('Review this project with the selected profile');
  await page.locator('#prompt').press('Enter');
  await expect(page.locator('[aria-label="Bot: Scout (fixed for this task)"]')).toBeVisible();
  await expect(page.locator('#transcript .user-message').last()).toContainText('Review this project with the selected profile');
  await expect(page.locator('#transcript .user-message').last()).not.toContainText('Bot brief:');
  const sent = await page.evaluate(async id => {
    const { invoke } = await import('/src/preview.ts');
    const tasks = await invoke<any[]>('list_sessions', {});
    return { task: tasks.find(task => task.id === id), prompt: (window as any).__botTaskPrompts?.find((entry: any) => entry.sessionId === id) };
  }, taskId);
  expect(sent.task.mode).toBe('ask');
  expect(sent.task.projectId).toBe(original.projectId);
  expect(sent.task.worktreePath).toBe(original.worktreePath);
  expect(sent.prompt.text).toContain('Precise, candid and warm.');
  expect(sent.prompt.text).not.toContain('pandacode_bot_notify');
});
