import { expect, test, type Page } from '@playwright/test';

const sidebar = (page: Page) => page.getByRole('region', { name: 'Bots', exact: true });

async function showBots(page: Page): Promise<void> {
  await page.goto('/?bots=sample');
  const bar = page.locator('.sidebar-bots-bar');
  if (await bar.getAttribute('aria-pressed') !== 'true') await bar.click();
  await expect(sidebar(page)).toBeVisible();
}

test.describe('shared bot channels', () => {

  test('shared channel composition derives all seven A/D/N mode readouts', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();

    const modes = [
      { id: 'M1', automatic: true, branches: [] },
      { id: 'M2', automatic: false, branches: ['direct'] },
      { id: 'M3', automatic: true, branches: ['direct'] },
      { id: 'M4', automatic: false, branches: ['nested'] },
      { id: 'M5', automatic: true, branches: ['nested'] },
      { id: 'M6', automatic: false, branches: ['direct', 'nested'] },
      { id: 'M7', automatic: true, branches: ['direct', 'nested'] },
    ] as const;
    const automatic = panel.locator('[data-bc-automatic-enabled]');
    const rows = panel.locator('[data-bc-reviewer-row]');
    await expect(panel.locator('[data-bc-source-mode]')).toHaveCount(7);
    for (const mode of modes) {
      await automatic.setChecked(mode.automatic);
      for (let index = 0; index < await rows.count(); index++) {
        const row = rows.nth(index);
        const reviewer = row.locator('[data-bc-reviewer]');
        if (await reviewer.isChecked()) await reviewer.uncheck();
      }
      for (const [index, branch] of mode.branches.entries()) {
        const row = rows.nth(index);
        await row.locator('[data-bc-reviewer]').check();
        await row.getByRole('combobox', { name: 'Reviewer branch' }).selectOption(branch);
      }

      const sources = [mode.automatic && 'A', mode.branches.includes('direct') && 'D', mode.branches.includes('nested') && 'N'].filter(Boolean).join(' + ');
      await expect(panel.locator('[data-bc-source-summary]')).toHaveText(`Sources ${sources} · ${mode.id}`);
      await expect(panel.locator('[data-bc-source-mode][aria-current="true"]')).toHaveCount(1);
      await expect(panel.locator(`[data-bc-source-mode="${mode.id}"]`)).toHaveAttribute('aria-current', 'true');
    }
  });

  test('shared Council state ignores a higher-revision event from an earlier cycle', async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem('pandacode.teamClassic', 'false'));
    await showBots(page);
    await expect.poll(() => page.evaluate(() => typeof (window as any).__previewEmit), { timeout: 15_000 }).toBe('function');
    await expect.poll(() => page.evaluate(() => (window as any).__previewListenerCount?.('architect-event') ?? 0), { timeout: 15_000 }).toBeGreaterThan(0);
    const sessionId = await page.locator('#prompt').getAttribute('data-session-id');
    if (!sessionId) throw new Error('Expected the active conversation prompt to identify its session.');
    const current = {
      sessionId,
      config: { workers: [], maxParallel: 0, maxRounds: 1, maxAssignments: 0, workspace: 'shared', summaryLimit: 0, rosterOnly: true, name: 'Current cycle team' },
      status: 'running', round: 1, revision: 20, workers: [], councilSince: 200, updatedAt: 1_800_000_001_000,
    };
    const emit = (snapshot: unknown) => page.evaluate(value => (window as any).__previewEmit('architect-event', value), snapshot);
    const team = page.locator('#team-trigger');
    await emit(current);
    await expect(team).toHaveAttribute('aria-label', 'Current cycle team: on');

    await emit({ ...current, config: { ...current.config, name: 'Stale cycle team' }, status: 'completed', revision: 99, councilSince: 100 });
    await expect(team).toHaveAttribute('aria-label', 'Current cycle team: on');

    await emit({ ...current, config: { ...current.config, name: 'Next cycle team' }, revision: 100, councilSince: 300 });
    await expect(team).toHaveAttribute('aria-label', 'Next cycle team: on');
  });

  test('bot collaboration: Council opens from the channel footer, keeps settings in the form and closes with Escape', async ({ page }) => {
    // The full preview initializes channel state before exercising the Council controls.
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    const trigger = composer.locator('.bc-council-trigger');
    const panel = composer.getByRole('dialog', { name: 'Council' });

    await expect(trigger).toBeVisible({ timeout: 15_000 });
    await expect(panel).toBeHidden();
    await expect(composer.locator('.bc-council-controls')).toBeHidden();
    await trigger.click();
    await expect(panel).toBeVisible();
    await expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const enabled = panel.getByRole('switch', { name: 'Council' });
    await expect(enabled).toBeFocused();
    await expect(panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' })).toBeVisible();
    await expect(panel.getByRole('radio')).toHaveCount(0);
    await page.keyboard.press('Tab');
    await expect(panel.getByRole('button', { name: '7.0', exact: true })).toBeFocused();
    await enabled.focus();
    await enabled.check();
    await panel.getByRole('button', { name: '8.5', exact: true }).click();
    await panel.getByRole('button', { name: '5', exact: true }).click();
    await expect(trigger).toHaveAttribute('aria-label', 'Council: on');
    await expect(panel.getByRole('button', { name: '8.5', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const values = await composer.evaluate(form => {
      const data = new FormData(form as HTMLFormElement);
      return { enabled: data.get('councilEnabled'), target: data.get('councilTarget'), rounds: data.get('councilMaxRounds'), hasComposition: data.has('councilComposition') };
    });
    expect(values).toEqual({ enabled: 'on', target: '8.5', rounds: '5', hasComposition: false });

    // The number fields still permit values outside the presets.
    await panel.getByRole('spinbutton', { name: 'Custom target score' }).fill('6.5');
    await expect(panel.getByRole('button', { name: '8.5', exact: true })).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
    await expect(trigger).toHaveAttribute('aria-expanded', 'false');
    await expect(trigger).toHaveAttribute('aria-label', 'Council: on');
    await prompt.fill('Keep the message composer independent');
    await expect(prompt).toHaveValue('Keep the message composer independent');
    await expect(composer.locator('.bc-council-controls')).toBeHidden();
  });

  test('shared channel submission preserves schema-v1 Council composition', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    const councilTrigger = composer.locator('.bc-council-trigger');
    const reviewerRows = panel.locator('[data-bc-reviewer-row]');
    const reviewers = panel.locator('input[name="councilReviewer"]');
    const configureComposition = async (strategy: string) => {
      await panel.getByRole('switch', { name: 'Council' }).check();
      await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
      await panel.getByRole('combobox', { name: 'Automatic reviewer count' }).selectOption('2');
      await reviewers.nth(0).check();
      await reviewers.nth(1).check();
      await reviewerRows.nth(0).getByRole('combobox', { name: 'Reviewer branch' }).selectOption('direct');
      const nestedPolicy = reviewerRows.nth(1).locator('[data-bc-nested-policy]');
      await reviewerRows.nth(1).getByRole('combobox', { name: 'Reviewer branch' }).selectOption('nested');
      await nestedPolicy.getByRole('combobox', { name: 'Task template' }).selectOption('validated_default');
      await nestedPolicy.getByRole('combobox', { name: 'Child criteria' }).selectOption('hybrid');
      await nestedPolicy.getByRole('spinbutton', { name: 'Child reviewers' }).fill('3');
      await nestedPolicy.getByRole('spinbutton', { name: 'Child target' }).fill('8.5');
      await nestedPolicy.getByRole('spinbutton', { name: 'Child max rounds' }).fill('4');
      await nestedPolicy.getByRole('combobox', { name: 'Escalation' }).selectOption('on_trigger');
      await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
      const criterion = panel.locator('[data-bc-criterion]').first();
      await criterion.getByRole('textbox', { name: 'Criterion ID' }).fill('channel-review');
      await criterion.getByRole('textbox', { name: 'Lens' }).fill('correctness');
      await criterion.getByRole('textbox', { name: 'Description' }).fill('Check correctness and evidence.');
      await criterion.getByRole('spinbutton', { name: 'Target' }).fill('8.5');
      const strategyPicker = panel.getByRole('combobox', { name: 'Execution strategy' });
      await expect(strategyPicker).toHaveValue('parallel');
      await expect(panel.getByRole('option', { name: 'S1 · Parallel' })).toBeEnabled();
      await expect(panel.getByRole('option', { name: 'S2 · Escalated / on-demand' })).toBeEnabled();
      await expect(panel.getByRole('option', { name: 'S3 · Two-phase' })).toBeEnabled();
      await strategyPicker.selectOption(strategy);
      const selectedBotIds = await reviewers.evaluateAll(inputs => inputs.filter(input => (input as HTMLInputElement).checked).map(input => (input as HTMLInputElement).value));
      return {
        selectedBotIds,
        composition: {
          schemaVersion: 1,
          automatic: { enabled: true, count: 2 },
          selected: [
            { botId: selectedBotIds[0], mode: 'direct', required: true },
            { botId: selectedBotIds[1], mode: 'nested', required: true, nested: { template: 'validated_default', childCriteriaSource: 'hybrid', childReviewerCount: 3, target: 8.5, maxRounds: 4, escalation: 'on_trigger' } },
          ],
          criteriaSource: 'user',
          criteria: [{ id: 'channel-review', lens: 'correctness', description: 'Check correctness and evidence.', required: true, target: 8.5, evidenceRequired: true, source: 'user' }],
          strategy,
          limits: { maxDepth: 2, maxRootSeats: 8, maxConcurrentLeaves: 64, maxLeafReviewsPerRound: 128, timeoutMs: 86_400_000, maxEstimatedCostMicros: 1_000_000 },
        },
      };
    };
    const posts = () => page.evaluate(() => (window as any).__previewSharedChannelPosts ?? []);
    for (const [index, value] of ['parallel', 'escalated', 'two_phase'].entries()) {
      if (index > 0) {
        await councilTrigger.click();
        await expect(panel).toBeVisible();
      }
      const { selectedBotIds, composition } = await configureComposition(value);
      await prompt.fill(`Preserve the ${value} Council plan`);
      await composer.locator('button[type="submit"]').click();
      await expect(panel).toBeHidden();
      await expect.poll(async () => (await posts()).length).toBe(index + 1);
      await expect(composer.locator('button[type="submit"]')).toBeEnabled();
      expect((await posts())[index]).toEqual({
        channelId: 'sample-launch',
        text: `Preserve the ${value} Council plan`,
        council: { target: 9, maxRounds: 10, reviewers: 2, reviewerBotIds: selectedBotIds, composition: { ...composition, strategy: value } },
      });
    }
  });

  test('shared channel legacy Council submission omits absent composition', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('button', { name: '8.5', exact: true }).click();
    await panel.getByRole('button', { name: '5', exact: true }).click();
    await prompt.fill('Keep the legacy Council payload');
    await composer.locator('button[type="submit"]').click();

    const posts = () => page.evaluate(() => (window as any).__previewSharedChannelPosts ?? []);
    await expect.poll(async () => (await posts()).length).toBe(1);
    expect((await posts())[0]).toEqual({
      channelId: 'sample-launch',
      text: 'Keep the legacy Council payload',
      council: { target: 8.5, maxRounds: 5, reviewers: 0 },
    });
  });

  test('bot collaboration: Council restores selection after a channel refresh without showing settings inline', async ({ page }) => {
    // The full preview initializes channel state before exercising the Council controls.
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('button', { name: '9.5', exact: true }).click();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    const reviewers = panel.locator('input[name="councilReviewer"]');
    await expect(reviewers.first()).toBeVisible();
    await reviewers.first().check();
    await page.keyboard.press('Escape');

    const previous = await composer.elementHandle();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    await expect.poll(async () => page.evaluate(old => document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);
    const restored = page.locator('#shared-channel-composer');
    await expect(restored.getByRole('button', { name: 'Council: on' })).toBeVisible();
    await expect(restored.locator('.bc-council-panel')).toBeHidden();
    await restored.getByRole('button', { name: 'Council: on' }).click();
    const restoredPanel = restored.getByRole('dialog', { name: 'Council' });
    await expect(restoredPanel.getByRole('switch', { name: 'Council' })).toBeChecked();
    await expect(restoredPanel.getByRole('button', { name: '9.5', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect(restoredPanel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' })).toBeChecked();
    await expect(restoredPanel.locator('input[name="councilReviewer"]').first()).toBeChecked();
  });

  test('renders isolated reviewer skill provenance with keyboard-readable details', async ({ page }) => {
    await page.goto('/?bots=sample');
    const html = await page.evaluate(async () => {
      const module = '/src/bot-shared-channels.ts';
      const { sharedChannelThreadHtml } = await import(module);
      return sharedChannelThreadHtml({
        id: 'channel-one', name: 'isolated', memberIds: [], leadBotId: null,
        archived: false, updatedAt: 1, activeBotIds: [],
        messages: [{
          id: 'delivery-one', kind: 'assigned', authorBotId: null,
          text: '', status: 'Completed', createdAt: 1,
          council: {
            cycleId: 'cycle-one', sessionId: 'private-session-one',
            state: 'Passed', round: 1, target: 9, lowest: 9.5,
            artifactRevision: 'frozen-revision',
            reviewers: [{
              botId: 'selected-bot', name: 'Selected bot', lens: 'security',
              round: 1, score: 9.5,
              skills: [{
                id: 'skill-a', status: 'not_verified',
                sha256: 'a'.repeat(64), sourceKind: 'github',
                sourceRef: 'main', sourceCommit: 'c'.repeat(40),
                required: true, nativeRetrieval: true,
                nativeCheckVerified: false,
                correctiveAction: 'Dispatch a new authorized Council round.',
              }],
            }],
          },
        }],
      }, []);
    });
    await page.setContent(html);
    const receipt = page.getByRole('note', { name: /skill-a.*not_verified/i });
    await expect(receipt).toBeVisible();
    await expect(receipt).toHaveAttribute('tabindex', '0');
    await expect(receipt).toHaveAttribute('title', /SHA-256 a{64}/);
    await expect(receipt).toContainText('Skills: 1');
    await expect(page.locator('.bc-council-summary')).toContainText('Selected bot');
    expect(html).not.toContain('sensitive/path');
  });

  test('switches directly from a shared channel to a bot conversation', async ({ page }) => {
    // Sample bot bootstrapping may leave no initial conversation selected.
    // Open a real bot conversation before asserting its composer and switching
    // to the channel; keep the complete navigation and draft-retention checks.
    test.setTimeout(60_000);
    await showBots(page);
    const initialBot = sidebar(page).getByRole('button', { name: /^Chat with Scout/ });
    await expect(initialBot).toBeVisible();
    await initialBot.click();
    await expect(page.locator('#composer-area #composer')).toBeVisible();
    const pageErrors: string[] = [];
    page.on('pageerror', error => pageErrors.push(error.message));
    const channel = sidebar(page).getByRole('button', { name: '#launch' });
    await expect(channel).toBeVisible();
    await channel.click();
    await expect(page.locator('.conversation-pane.channel-thread .bc-channel')).toBeVisible();

    // The new bot conversation must replace the channel without clicking Back.
    await page.getByRole('textbox', { name: 'Message channel' }).fill('Unsent channel draft');
    const bot = sidebar(page).getByRole('button', { name: /^Chat with Scout/ });
    await expect(bot).toBeVisible();
    await bot.click();
    // A positive destination assertion must pass before checking that the channel
    // class went away; otherwise a blank intermediate view could pass that check.
    await expect(page.locator('#breadcrumb .bot-crumb')).toContainText('Scout');
    await expect(page.locator('.conversation-pane')).not.toHaveClass(/channel-thread/);
    await expect(page.locator('#transcript .bc-channel')).toHaveCount(0);
    await expect(page.locator('#composer #prompt')).toBeVisible();
    await expect(sidebar(page).locator('.channel-row[aria-current="page"]')).toHaveCount(0);

    // The channel keeps a draft that was not sent before the switch.
    // Re-resolve the current channel row after the bot switch; the sidebar can
    // replace its nodes asynchronously, so clicking a detached old element is inert.
    await channel.click();
    await expect(page.getByRole('textbox', { name: 'Message channel' })).toHaveValue('Unsent channel draft');
    expect(pageErrors).toEqual([]);
  });

  test('creates a channel from the sidebar and opens its settings', async ({ page }) => {
    test.setTimeout(60_000);
    await showBots(page);
    await page.getByRole('button', { name: 'Bot actions' }).click();
    await page.getByRole('menu', { name: 'Bot actions' }).getByRole('menuitem', { name: 'New channel' }).click();

    const dialog = page.getByRole('dialog', { name: 'New channel' });
    await dialog.getByRole('textbox', { name: 'Channel name' }).fill('release-test');
    await dialog.getByRole('checkbox', { name: 'Scout' }).check();
    await dialog.getByRole('checkbox', { name: 'Writer' }).check();
    await dialog.getByRole('button', { name: 'Create channel' }).click();

    await expect(sidebar(page).getByRole('group', { name: 'Channels' })).toContainText('#release-test');
    const channel = page.locator('.conversation-pane.channel-thread .bc-channel');
    await expect(page.locator('#breadcrumb')).toContainText('#release-test');
    await expect(channel.getByText('Add bots in channel settings to start work.')).toHaveCount(0);
    await channel.getByRole('button', { name: 'Channel settings' }).click();
    await expect(page.getByRole('dialog', { name: 'Channel settings' })).toBeVisible();
  });

  test('an empty channel explains how to start and an archived channel is preview only', async ({ page }) => {
    test.setTimeout(60_000);
    await showBots(page);
    await page.getByRole('button', { name: 'Bot actions' }).click();
    await page.getByRole('menu', { name: 'Bot actions' }).getByRole('menuitem', { name: 'New channel' }).click();
    const create = page.getByRole('dialog', { name: 'New channel' });
    await create.getByRole('textbox', { name: 'Channel name' }).fill('notes');
    await create.getByRole('button', { name: 'Create channel' }).click();
    const channel = page.locator('.conversation-pane.channel-thread .bc-channel');
    await expect(channel).toContainText('Add bots in channel settings to start work.');
    await expect(channel.locator('textarea')).toHaveCount(0);
    await page.getByRole('button', { name: 'Back to conversation' }).click();

    const row = sidebar(page).getByRole('button', { name: '#notes' });
    await row.click({ button: 'right' });
    await page.getByRole('menuitem', { name: 'Delete' }).click();
    await expect(sidebar(page).getByRole('group', { name: 'Deleted channels' })).toContainText('#notes');
    await sidebar(page).getByRole('button', { name: '#notes' }).click();
    const archived = page.locator('.conversation-pane.channel-thread .bc-channel');
    await expect(archived).toContainText('Deleted channel. Preview only.');
    await expect(archived.locator('textarea')).toHaveCount(0);
    await expect(archived.getByRole('button', { name: 'Channel settings' })).toHaveCount(0);
  });

  test('routes to at most two mentioned bots and lets the user stop and reassign', async ({ page }) => {
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const channel = page.locator('.conversation-pane.channel-thread .bc-channel');
    await page.getByRole('textbox', { name: 'Message channel' }).fill('@Scout and @Writer and @Ledger, review launch.');
    await page.locator('#shared-channel-composer button[type=submit]').click();
    await expect(channel.locator('.bc-assigned')).toHaveCount(2);
    await expect(channel.locator('.bc-assigned')).toContainText(['Assigned to Scout', 'Assigned to Writer']);
    await expect(channel.locator('.bc-working-status')).toHaveText('Scout and Writer are working');
    await page.locator('#composer-area').getByRole('button', { name: 'Stop' }).click();
    await expect(page.locator('#composer-area .bc-channel-actions')).toContainText('This message stopped.');
    await page.locator('#composer-area').getByRole('button', { name: /Reassign/ }).click();
    await page.locator('#composer-area .bc-reassign-menu').getByRole('button', { name: 'Ledger' }).click();
    await expect(channel.locator('.bc-assigned').last()).toContainText('Assigned to Ledger');
    await expect(channel.locator('.bc-working-status')).toHaveText('Ledger is working');
  });

  test('keeps an open Reassign menu through a refresh and names who Stop stops', async ({ page }) => {
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    await page.getByRole('textbox', { name: 'Message channel' }).fill('@Writer draft the post.');
    await page.getByRole('textbox', { name: 'Message channel' }).press('Enter');
    const actions = page.locator('#composer-area .bc-channel-actions');
    await expect(actions.getByRole('button', { name: 'Stop Writer' })).toBeVisible();
    await expect(actions.getByRole('status')).toHaveCount(0);
    await actions.getByRole('button', { name: 'Stop Writer' }).click();
    await actions.getByRole('button', { name: /Reassign/ }).click();
    const menu = page.locator('#composer-area .bc-reassign-menu');
    await expect(menu).toBeVisible();
    const before = await page.evaluate(() => document.querySelector('#composer-area .bc-reassign-menu'));
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    // The refresh rewrote the actions (a new menu element), and the menu is still open.
    await expect.poll(() => page.evaluate(old => document.querySelector('#composer-area .bc-reassign-menu') !== old, before)).toBe(true);
    await expect(menu).toBeVisible();
    await expect(actions.getByRole('button', { name: /Reassign/ })).toHaveAttribute('aria-expanded', 'true');
  });

  test('a failed post can be continued, and removing a member stops its work', async ({ page }) => {
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const channel = page.locator('.conversation-pane.channel-thread .bc-channel');
    const actions = page.locator('#composer-area .bc-channel-actions');
    await expect(channel.locator('.bc-history')).toBeVisible();
    await page.getByRole('textbox', { name: 'Message channel' }).fill('[post-fails] ship it');
    await page.getByRole('textbox', { name: 'Message channel' }).press('Enter');
    await expect(actions).toContainText('This message failed.');
    await actions.getByRole('button', { name: 'Continue' }).click();
    await expect(channel.locator('.bc-working-status')).toHaveText('Scout is working');
    await expect(channel.locator('.bc-assigned')).toHaveCount(2);

    await channel.getByRole('button', { name: 'Channel settings' }).click();
    const settings = page.getByRole('dialog', { name: 'Channel settings' });
    await settings.getByRole('checkbox', { name: 'Scout' }).uncheck();
    await settings.getByRole('button', { name: 'Save' }).click();
    await expect(actions).toContainText('This message stopped.');
    await expect(channel.locator('.bc-working')).toHaveCount(0);
    await expect(channel.locator('.bc-assigned').last()).toContainText('Stopped');
  });

  test('uses the channel lead when a post has no bot mention', async ({ page }) => {
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const channel = page.locator('.conversation-pane.channel-thread .bc-channel');
    await expect(sidebar(page).locator('.channel-row[aria-current="page"]')).toHaveText('#launch');
    await expect(sidebar(page).locator('.bot-row.selected')).toHaveCount(0);
    await expect(page.locator('#shared-channel-composer .send-button')).toBeVisible();
    await expect(channel.locator('.bc-history')).toBeVisible();
    // Enter sends, like the task composer.
    await page.getByRole('textbox', { name: 'Message channel' }).fill('Review launch.');
    await page.getByRole('textbox', { name: 'Message channel' }).press('Enter');
    await expect(channel.locator('.bc-assigned')).toHaveCount(1);
    await expect(channel.locator('.bc-assigned')).toContainText('Assigned to Scout');
  });
});
