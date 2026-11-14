import { expect, test, type Page } from '@playwright/test';

const sidebar = (page: Page) => page.getByRole('region', { name: 'Bots', exact: true });

async function showBots(page: Page): Promise<void> {
  await page.goto('/?bots=sample');
  const bar = page.locator('.sidebar-bots-bar');
  if (await bar.getAttribute('aria-pressed') !== 'true') await bar.click();
  await expect(sidebar(page)).toBeVisible();
}

test.describe('shared bot channels', () => {

  test('bot collaboration: Council opens from the channel footer, keeps settings in the form and closes with Escape', async ({ page }) => {
    // The full preview initializes channel state before exercising the Council controls.
    test.setTimeout(60_000);
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
    await enabled.check();
    await panel.getByRole('button', { name: '8.5', exact: true }).click();
    await panel.getByRole('button', { name: '5', exact: true }).click();
    await expect(trigger).toHaveAttribute('aria-label', 'Council: on');
    await expect(panel.getByRole('button', { name: '8.5', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const values = await composer.evaluate(form => {
      const data = new FormData(form as HTMLFormElement);
      return { enabled: data.get('councilEnabled'), target: data.get('councilTarget'), rounds: data.get('councilMaxRounds') };
    });
    expect(values).toEqual({ enabled: 'on', target: '8.5', rounds: '5' });

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

  test('bot collaboration: Council restores selection after a channel refresh without showing settings inline', async ({ page }) => {
    // The full preview initializes channel state before exercising the Council controls.
    test.setTimeout(60_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('button', { name: '9.5', exact: true }).click();
    await panel.getByRole('radio', { name: 'Bots' }).check();
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
    await expect(restoredPanel.getByRole('radio', { name: 'Bots' })).toBeChecked();
    await expect(restoredPanel.locator('input[name="councilReviewer"]').first()).toBeChecked();
  });



  test('shared Channel submits M3 hybrid seats, S2 and frozen user criteria through the existing native request', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(
        "if (command === 'bots_list' || command.startsWith('bot_'))",
        "if (command === 'bot_shared_channel_post') (window.__channelCouncilPosts ??= []).push(structuredClone(args));\n  if (command === 'bots_list' || command.startsWith('bot_'))",
      );
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('radio', { name: 'Bots' }).check();
    const bots = panel.locator('input[name="councilReviewer"]');
    await bots.nth(1).check();
    await panel.getByRole('spinbutton', { name: 'Additional automatic reviewers' }).fill('1');
    await expect(panel.getByRole('status')).toContainText('2 root seats (1 selected bots + 1 automatic)');
    await panel.getByRole('combobox', { name: 'Review strategy' }).selectOption('escalated');
    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    await panel.getByRole('textbox', { name: 'Criterion ID' }).fill('security');
    await panel.getByRole('textbox', { name: 'Review lens' }).fill('Permission boundary');
    await panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).check();
    const selectedId = await bots.nth(1).inputValue();
    await composer.getByRole('textbox', { name: 'Message channel' }).fill('Review this artifact.');
    await composer.locator('button[type=submit]').click();
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
    const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0].council);
    expect(posted).toMatchObject({
      target: 9, maxRounds: 10, reviewers: 0,
      reviewerBotIds: [selectedId], automaticReviewers: 1,
      strategy: 'escalated', forceEscalation: true,
      criteriaSource: 'user', criteria: [{ id: 'security', lens: 'Permission boundary',
        target: 9, required: true, evidenceRequired: true }],
    });
    expect(posted.nestedBotReviewers).toBeUndefined();
  });

  test('shared Channel preserves nested reviewer settings on refresh and serializes a single child seat', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(
        "if (command === 'bots_list' || command.startsWith('bot_'))",
        "if (command === 'bot_shared_channel_post') (window.__channelCouncilPosts ??= []).push(structuredClone(args));\n  if (command === 'bots_list' || command.startsWith('bot_'))",
      );
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    let composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    let panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('radio', { name: 'Bots' }).check();
    const bots = panel.locator('input[name="councilReviewer"]');
    await bots.nth(1).check();
    await bots.nth(2).check();
    const childId = await bots.nth(2).inputValue();
    await panel.locator('select[name="councilReviewerRole"]').nth(2).selectOption('nested');
    await panel.locator('input[name="councilChildReviewers"]').nth(2).fill('3');
    await panel.getByRole('combobox', { name: 'Review strategy' }).selectOption('two_phase');
    await page.keyboard.press('Escape');
    const previous = await composer.elementHandle();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    await expect.poll(() => page.evaluate(old => document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);
    composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: on' }).click();
    panel = composer.getByRole('dialog', { name: 'Council' });
    await expect(panel.locator('select[name="councilReviewerRole"]').nth(2)).toHaveValue('nested');
    await expect(panel.locator('input[name="councilChildReviewers"]').nth(2)).toHaveValue('3');
    await expect(panel.getByRole('combobox', { name: 'Review strategy' })).toHaveValue('two_phase');
    await expect(panel.getByRole('status')).toContainText('2 root seats (2 selected bots + 0 automatic)');
    await composer.getByRole('textbox', { name: 'Message channel' }).fill('Check isolation.');
    await composer.locator('button[type=submit]').click();
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
    const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0].council);
    expect(posted.strategy).toBe('two_phase');
    expect(posted.automaticReviewers).toBeUndefined();
    expect(posted.reviewerBotIds).toHaveLength(1);
    expect(posted.nestedBotReviewers).toEqual([{ botId: childId, childReviewers: 3, childTarget: 9, childMaxRounds: 10 }]);
  });

  test('channel Council opens the shared native inspector without navigating or granting a pass', async ({ page }) => {
    test.setTimeout(60_000);
    await page.setViewportSize({ width: 360, height: 640 });
    const inspection = {
      cycleStartedAt: 1000, teamRevision: 9,
      status: { state: 'passed', round: 1, lowest: 9.7, target: 9, stale: false },
      verifiedPass: false, criteria: [],
      attempts: [{
        workerId: 'a', round: 1, lens: '<script>alert(1)</script>',
        reviewerSource: 'direct_bot', name: 'Review', status: 'completed',
        score: 9.7, abandoned: false, provider: 'codex', model: 'native',
        attempt: 1, createdAt: 1000, startedAt: 1001, lastUpdatedAt: 1020,
        tokenCount: null, costAmount: null, costCurrency: null, usageProvenance: 'unknown',
      }],
    };
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(/switch\s*\(command\)\s*\{/,
        'if (command === "architect_council_inspect" && args.sessionId === "channel-cycle-task") return ' +
        JSON.stringify(inspection) + '; switch (command) {');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    await page.evaluate(async () => {
      const { sharedChannelThreadHtml } = await import('/src/bot-shared-channels.ts');
      const wrapper = document.createElement('div');
      wrapper.innerHTML = sharedChannelThreadHtml({
        id: 'channel-one', name: 'launch', memberIds: [], leadBotId: null,
        archived: false, updatedAt: 1, activeBotIds: [],
        messages: [{
          id: 'delivery-one', kind: 'assigned', authorBotId: null,
          text: '', status: 'Completed', createdAt: 1,
          council: {
            cycleId: 'cycle-one', sessionId: 'channel-cycle-task', state: 'Passed',
            round: 1, target: 9, lowest: 9.7, artifactRevision: 'artifact',
            reviewers: [],
          },
        }],
      }, []);
      const assignment = wrapper.querySelector('.bc-assigned');
      const history = document.querySelector('.bc-history');
      if (!assignment || !history) throw new Error('Channel fixture is missing');
      history.append(assignment);
    });
    await expect(page.getByRole('button', { name: 'View Council report' })).toBeVisible();
    await page.getByRole('button', { name: 'Inspect Council evidence' }).click();
    const dialog = page.getByRole('dialog', { name: 'Council inspection' });
    await expect(dialog).toBeVisible();
    const details = dialog.locator('.council-inspection');
    await expect(details).toHaveAttribute('open', '');
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    await expect(dialog.getByText('Tokens unknown', { exact: false })).toBeVisible();
    await expect(dialog.getByText('<script>alert(1)</script>', { exact: false })).toBeVisible();
    await expect(dialog.locator('script')).toHaveCount(0);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('textbox', { name: 'Message channel' })).toBeVisible();
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
    await showBots(page);
    await page.getByRole('button', { name: 'Add bot or channel' }).click();
    await page.getByRole('menu', { name: 'Create' }).getByRole('menuitem', { name: 'New channel' }).click();

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
    await showBots(page);
    await page.getByRole('button', { name: 'Add bot or channel' }).click();
    await page.getByRole('menu', { name: 'Create' }).getByRole('menuitem', { name: 'New channel' }).click();
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
