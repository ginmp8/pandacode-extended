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

  test('channel Council keeps keyboard focus within the popover at narrow widths', async ({ page }) => {
    test.setTimeout(60_000);
    await page.setViewportSize({ width: 320, height: 460 });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const trigger = composer.locator('.bc-council-trigger');
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await trigger.click();
    await expect(panel).toBeVisible();

    const first = panel.getByRole('switch', { name: 'Council' });
    // The final file-scope input is conditional on selecting configured bots.
    await panel.locator('input[name="councilMode"][value="selected"]').check();
    const last = panel.locator('input[name="councilFilePath"]');
    await expect(last).toBeVisible();
    await first.focus();
    await page.keyboard.press('Shift+Tab');
    await expect(last).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(first).toBeFocused();

    // Conditional user-defined criteria participate in the same dialog focus
    // order rather than trapping the keyboard on a hidden field.
    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    const criterionId = panel.locator('input[name="councilCriterionId"]').first();
    await expect(criterionId).toBeVisible();
    await criterionId.focus();
    await page.keyboard.press('Tab');
    await expect(panel.locator('input[name="councilCriterionLens"]').first()).toBeFocused();

    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
    await expect(trigger).toHaveAttribute('aria-expanded', 'false');
  });

  test('channel Council retains the open popover, keyboard focus and scroll during an unrelated channel update', async ({ page }) => {
    test.setTimeout(60_000);
    await page.setViewportSize({ width: 320, height: 460 });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    const lens = panel.locator('input[name="councilCriterionLens"]').first();
    await lens.fill('Project privacy boundary');
    await lens.focus();
    await lens.evaluate(element => (element as HTMLInputElement).setSelectionRange(2, 9));
    const scrollTop = await panel.evaluate(element => {
      element.scrollTop = Math.min(80, element.scrollHeight - element.clientHeight);
      return element.scrollTop;
    });
    expect(scrollTop).toBeGreaterThan(0);

    const previous = await composer.elementHandle();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    await expect.poll(async () => page.evaluate(old =>
      document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);

    const restored = page.locator('#shared-channel-composer');
    const restoredPanel = restored.getByRole('dialog', { name: 'Council' });
    await expect(restoredPanel).toBeVisible();
    await expect(restored.getByRole('button', { name: 'Council: on' })).toHaveAttribute('aria-expanded', 'true');
    await expect(restoredPanel.locator('input[name="councilCriterionLens"]').first()).toHaveValue('Project privacy boundary');
    await expect(restoredPanel.locator('input[name="councilCriterionLens"]').first()).toBeFocused();
    // An unrelated channel refresh must not move the caret or overwrite a
    // partially edited required review criterion.
    await expect.poll(() => restoredPanel.locator('input[name="councilCriterionLens"]').first()
      .evaluate(element => [(element as HTMLInputElement).selectionStart, (element as HTMLInputElement).selectionEnd]))
      .toEqual([2, 9]);
    await expect.poll(() => restoredPanel.evaluate(element => element.scrollTop)).toBe(scrollTop);
    await page.keyboard.press('Escape');
    await expect(restoredPanel).toBeHidden();
    await expect(restored.getByRole('button', { name: 'Council: on' })).toBeFocused();
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




  // Rotate the three criteria origins across the strategies for each mode.
  // These tests exercise the actual composer submit path with an intercepted
  // preview command; native callback and permission tests remain separate.
  const channelModes = [
    { mode: 'M1', roles: [], automatic: 0 },
    { mode: 'M2', roles: ['direct', 'direct'], automatic: 0 },
    { mode: 'M3', roles: ['direct'], automatic: 1 },
    { mode: 'M4', roles: ['nested', 'nested'], automatic: 0 },
    { mode: 'M5', roles: ['nested'], automatic: 1 },
    { mode: 'M6', roles: ['direct', 'nested'], automatic: 0 },
    { mode: 'M7', roles: ['direct', 'nested'], automatic: 1 },
  ] as const;
  const origins = ['architect', 'user', 'hybrid'] as const;
  const strategies = ['parallel', 'escalated', 'two_phase'] as const;
  for (const [modeIndex, mode] of channelModes.entries()) for (const [strategyIndex, strategy] of strategies.entries()) {
    const origin = origins[(modeIndex + strategyIndex) % origins.length];
    test(`shared Channel Council matrix ${mode.mode}/${strategy}/${origin} submits explicit sources`, async ({ page }) => {
      test.setTimeout(60_000);
      await page.route('**/src/preview.ts*', async route => {
        const response = await route.fetch();
        const source = await response.text();
        const body = source.replace(
          /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
          "if (command === 'bot_shared_channel_post') (window.__channelCouncilPosts ??= []).push(structuredClone(args));\n  if (command === 'bots_list' || command.startsWith('bot_'))",
        );
        if (body === source) throw new Error('Channel Council preview command interceptor was not installed.');
        await route.fulfill({ response, body });
      });
      await showBots(page);
      await sidebar(page).getByRole('button', { name: '#launch' }).click();
      let composer = page.locator('#shared-channel-composer');
      await composer.getByRole('button', { name: 'Council: off' }).click();
      let panel = composer.getByRole('dialog', { name: 'Council' });
      await panel.getByRole('switch', { name: 'Council' }).check();
      const selectedIds: string[] = [];
      if (mode.roles.length) {
        await panel.getByRole('radio', { name: 'Bots' }).check();
        const bots = panel.locator('input[name="councilReviewer"]');
        for (const [index, role] of mode.roles.entries()) {
          // The first fixture channel member is its lead/executor.
          const bot = bots.nth(index + 1);
          await bot.check();
          selectedIds.push(await bot.inputValue());
          if (role === 'nested') await panel.locator('select[name="councilReviewerRole"]').nth(index + 1).selectOption('nested');
        }
        if (mode.automatic) await panel.getByRole('spinbutton', { name: 'Additional automatic reviewers' }).fill(String(mode.automatic));
      }
      if (mode.mode === 'M3' && strategy === 'parallel') {
        const role = panel.locator('select[name="councilReviewerRole"]').nth(1);
        const leaves = panel.locator('input[name="councilChildReviewers"]').nth(1);
        await role.selectOption('nested');
        await leaves.fill('1');
        expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(false);
        await role.selectOption('direct');
        await expect(leaves).toBeDisabled();
        await expect(leaves).toHaveValue('1');
        expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(true);
        await role.selectOption('nested');
        await expect(leaves).toBeEnabled();
        expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(false);
        await role.selectOption('direct');
      }
      await panel.getByRole('combobox', { name: 'Review strategy' }).selectOption(strategy);
      const immediate = panel.getByRole('checkbox', { name: 'Require immediate S2 escalation' });
      if (strategy === 'escalated') {
        await expect(immediate).toBeChecked();
        await immediate.focus();
        await page.keyboard.press('Space');
        await expect(immediate).not.toBeChecked();
      } else await expect(immediate).toBeHidden();
      const source = panel.getByRole('combobox', { name: 'Criteria source' });
      if (mode.mode === 'M1' && strategy === 'parallel') {
        await source.selectOption('user');
        const target = panel.locator('input[name="councilCriterionTarget"]').first();
        await target.fill('0');
        expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(false);
        await source.selectOption('architect');
        await expect(target).toBeDisabled();
        await expect(target).toHaveValue('0');
        expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(true);
      }
      await source.selectOption(origin);
      if (origin !== 'architect') {
        await panel.getByRole('textbox', { name: 'Criterion ID' }).fill('security');
        await panel.getByRole('textbox', { name: 'Review lens' }).fill('Permission boundary');
        await panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).check();
      }
      await composer.getByRole('textbox', { name: 'Message channel' }).fill('Review this task.');
      if (mode.mode === 'M7' && strategy === 'escalated') {
        // A native trigger may need a second round even with immediate escalation off.
        const rounds = panel.getByRole('spinbutton', { name: 'Custom max rounds' });
        await rounds.fill('1');
        await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
        expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
        await rounds.fill('10');
        const childRounds = panel.locator('input[name="councilChildMaxRounds"]').nth(2);
        await childRounds.fill('1');
        await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
        expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
        await childRounds.fill('10');
        // Three required criteria fit the three root seats, but not the two child leaves.
        for (const id of ['integrity', 'reliability']) {
          await panel.getByRole('button', { name: 'Add criterion' }).click();
          await panel.getByRole('textbox', { name: 'Criterion ID' }).last().fill(id);
          await panel.getByRole('textbox', { name: 'Review lens' }).last().fill(id);
        }
        await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
        expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
        await panel.getByRole('button', { name: 'Remove criterion' }).last().click();
        await panel.getByRole('button', { name: 'Remove criterion' }).last().click();
        await page.keyboard.press('Escape');
        const previous = await composer.elementHandle();
        await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
        await expect.poll(async () => page.evaluate(old => document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);
        composer = page.locator('#shared-channel-composer');
        await composer.getByRole('button', { name: 'Council: on' }).click();
        panel = composer.getByRole('dialog', { name: 'Council' });
        await expect(panel.getByRole('checkbox', { name: 'Require immediate S2 escalation' })).not.toBeChecked();
      }
      await composer.locator('button[type=submit]').click();
      await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
      const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0].council);
      expect(posted.strategy ?? 'parallel').toBe(strategy);
      expect(posted.forceEscalation ?? false).toBe(false);
      expect(posted.automaticReviewers ?? 0).toBe(mode.automatic);
      expect(posted.reviewers).toBe(0);
      expect(posted.reviewerBotIds ?? []).toEqual(mode.roles.flatMap((role, index) => role === 'direct' ? [selectedIds[index]] : []));
      expect(posted.nestedBotReviewers ?? []).toEqual(mode.roles.flatMap((role, index) => role === 'nested'
        ? [{ botId: selectedIds[index], childReviewers: 2, childTarget: 9, childMaxRounds: 10 }] : []));
      expect(posted.criteriaSource ?? 'architect').toBe(origin);
      expect(posted.criteria ?? []).toEqual(origin === 'architect' ? [] : [
        { id: 'security', lens: 'Permission boundary', target: 9, required: true, evidenceRequired: true },
      ]);
    });
  }

  test('Channel Council denies the implicit lead executor before dispatch but permits explicit other executors', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const source = await response.text();
      const body = source.replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
        "if (command === 'bot_shared_channel_post') (window.__channelCouncilPosts ??= []).push(structuredClone(args));\n  if (command === 'bots_list' || command.startsWith('bot_'))",
      );
      if (body === source) throw new Error('Channel Council preview command interceptor was not installed.');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('radio', { name: 'Bots' }).check();
    await panel.getByRole('checkbox', { name: 'Scout', exact: true }).check();
    await panel.getByRole('checkbox', { name: 'Writer', exact: true }).check();
    const prompt = composer.getByRole('textbox', { name: 'Message channel' });
    await prompt.fill('Review this task.');
    await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
    await expect(page.getByText('A message executor cannot also be one of its Council reviewers.', { exact: true })).toBeVisible();
    expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
    await expect(prompt).toHaveValue('Review this task.');
    await prompt.fill('@Ledger Review this task.');
    await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
    const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0]);
    expect(posted.recipients).toHaveLength(1);
    expect(posted.council.reviewerBotIds).toHaveLength(2);
    expect(posted.council.reviewerBotIds).not.toContain(posted.recipients[0]);
  });

  test('Channel Council opt-out sends an ordinary message despite an invalid inactive draft', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const source = await response.text();
      const body = source.replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
        "if (command === 'bot_shared_channel_post') (window.__channelCouncilPosts ??= []).push(structuredClone(args));\n  if (command === 'bots_list' || command.startsWith('bot_'))",
      );
      if (body === source) throw new Error('Channel Council preview command interceptor was not installed.');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('textbox', { name: 'Message channel' }).fill('Send without Council.');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    const enabled = panel.getByRole('switch', { name: 'Council' });
    await enabled.check();
    const target = panel.getByRole('spinbutton', { name: 'Custom target score' });
    await target.fill('0');
    expect(await composer.evaluate(form => (form as HTMLFormElement).checkValidity())).toBe(false);
    await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
    expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
    await enabled.uncheck();
    await expect(target).toHaveValue('0');
    await composer.evaluate(form => (form as HTMLFormElement).requestSubmit());
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
    const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0]);
    expect(posted.text).toBe('Send without Council.');
    expect(posted.council).toBeUndefined();
  });

  test('shared Channel submits M3 hybrid seats, S2 and frozen user criteria through the existing native request', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
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

  test('Channel restores the rejected Council request before an explicit retry', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const source = await response.text();
      const body = source.replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
        "if (command === 'bot_shared_channel_post') { const posts = (window.__channelCouncilPosts ??= []); posts.push(structuredClone(args)); if (posts.length === 1) throw new Error('Council admission rejected by test host'); }\n  if (command === 'bots_list' || command.startsWith('bot_'))",
      );
      if (body === source) throw new Error('Channel Council preview command interceptor was not installed.');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('radio', { name: 'Bots' }).check();
    await panel.getByRole('checkbox', { name: 'Writer', exact: true }).check();
    await panel.getByRole('spinbutton', { name: 'Additional automatic reviewers' }).fill('1');
    await panel.getByRole('combobox', { name: 'Review strategy' }).selectOption('escalated');
    await panel.getByRole('checkbox', { name: 'Require immediate S2 escalation' }).uncheck();
    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    await panel.getByRole('textbox', { name: 'Criterion ID' }).fill('security');
    await panel.getByRole('textbox', { name: 'Review lens' }).fill('Permission boundary');
    await panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).check();
    await page.keyboard.press('Escape');
    const prompt = composer.getByRole('textbox', { name: 'Message channel' });
    await prompt.fill('@Ledger Review this artifact.');
    await composer.locator('button[type=submit]').click();
    await expect(page.getByText('Council admission rejected by test host', { exact: true })).toBeVisible();
    await expect(prompt).toHaveValue('@Ledger Review this artifact.');
    await expect(composer.getByRole('button', { name: 'Council: on' })).toBeVisible();
    expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);

    // A same-Channel refresh cannot discard the restored draft.
    const previous = await composer.elementHandle();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    await expect.poll(async () => page.evaluate(old => document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);
    await composer.getByRole('button', { name: 'Council: on' }).click();
    await expect(panel.getByRole('checkbox', { name: 'Writer', exact: true })).toBeChecked();
    await expect(panel.getByRole('checkbox', { name: 'Require immediate S2 escalation' })).not.toBeChecked();
    await expect(panel.getByRole('textbox', { name: 'Criterion ID' })).toHaveValue('security');
    await expect(panel.getByRole('checkbox', { name: 'Require independently verified native evidence' })).toBeChecked();
    await page.keyboard.press('Escape');
    await composer.locator('button[type=submit]').click();
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(2);
    const posts = await page.evaluate(() => (window as any).__channelCouncilPosts);
    expect(posts[0].council).toMatchObject({ strategy: 'escalated', forceEscalation: false, criteriaSource: 'user' });
    expect(posts[1]).toEqual(posts[0]);
    await expect(composer.getByRole('button', { name: 'Council: off' })).toBeVisible();
    await expect(prompt).toHaveValue('');
  });

  test('shared Channel preserves and validates multiple user-required criteria across refresh', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
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
    await panel.locator('input[name="councilReviewer"]').nth(1).check();
    await panel.getByRole('spinbutton', { name: 'Additional automatic reviewers' }).fill('1');
    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('hybrid');

    await panel.getByRole('textbox', { name: 'Criterion ID' }).fill('security');
    await panel.getByRole('textbox', { name: 'Review lens' }).fill('Authorization boundary');
    await panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).check();
    await panel.getByRole('button', { name: 'Add criterion' }).click();
    await expect(panel.locator('[data-council-criterion-row]')).toHaveCount(2);
    await panel.getByRole('textbox', { name: 'Criterion ID' }).nth(1).fill('security');
    await panel.getByRole('textbox', { name: 'Review lens' }).nth(1).fill('Data integrity');
    await composer.getByRole('textbox', { name: 'Message channel' }).fill('Review both required gates');
    await composer.locator('button[type=submit]').click();
    expect(await page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(0);
    await expect(panel).toBeVisible();

    await panel.getByRole('textbox', { name: 'Criterion ID' }).nth(1).fill('data_integrity');
    await page.keyboard.press('Escape');
    const previous = await composer.elementHandle();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', {}));
    await expect.poll(async () => page.evaluate(old => document.querySelector('#shared-channel-composer') !== old, previous)).toBe(true);
    composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: on' }).click();
    panel = composer.getByRole('dialog', { name: 'Council' });
    await expect(panel.locator('[data-council-criterion-row]')).toHaveCount(2);
    await expect(panel.getByRole('textbox', { name: 'Criterion ID' }).nth(0)).toHaveValue('security');
    await expect(panel.getByRole('textbox', { name: 'Criterion ID' }).nth(1)).toHaveValue('data_integrity');
    await expect(panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).nth(0)).toBeChecked();
    await expect(panel.getByRole('checkbox', { name: 'Require independently verified native evidence' }).nth(1)).not.toBeChecked();
    await expect(panel.getByRole('status')).toContainText('2 required criteria (not yet verified)');
    await composer.locator('button[type=submit]').click();
    await expect.poll(() => page.evaluate(() => (window as any).__channelCouncilPosts?.length ?? 0)).toBe(1);
    const posted = await page.evaluate(() => (window as any).__channelCouncilPosts[0].council);
    expect(posted.criteriaSource).toBe('hybrid');
    expect(posted.criteria).toEqual([
      { id: 'security', lens: 'Authorization boundary', target: 9, required: true, evidenceRequired: true },
      { id: 'data_integrity', lens: 'Data integrity', target: 9, required: true, evidenceRequired: false },
    ]);
  });

  test('shared Channel preserves nested reviewer settings on refresh and serializes a single child seat', async ({ page }) => {
    test.setTimeout(60_000);
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(
        /if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
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
      sessionId: 'channel-root', cycleStartedAt: 1000, teamRevision: 9,
      status: { state: 'passed', round: 1, lowest: 9.7, target: 9, stale: false },
      verifiedPass: false, criteria: [],
      reportedFindings: [{
        schemaVersion: 1, id: 'channel:report:1', cycleId: 'channel-root:1000',
        round: 1, workerId: 'a', reportMessageId: 11, lens: 'Security',
        severity: 'high', category: 'authorization', title: 'Channel finding',
        explanation: 'Unverified reviewer report', artifactRevision: 'revision-9',
        verifiedEvidenceRefs: [], evidenceVerified: false, locationVerified: false,
      }],
      composedUsage: { uniqueAttempts: 3, queueMeasuredAttempts: 0, spanMeasuredAttempts: 0,
        cumulativeQueueWaitMs: null, cumulativeObservedSpanMs: null,
        tokenCount: null, costAmount: null, costCurrency: null, usageProvenance: 'unknown' },
      retiredChildAttempts: 2,
      reservedChildren: [{ botId: 'security-child', childSessionId: 'channel-child-one',
        rootCycleId: 'channel-root:1000', requiredLeafReviewers: 2, status: 'reviewing' }],
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
      const body = (await response.text()).replace(/if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
        'if (command === "bot_shared_channel_council_inspect" && args.channelId === "channel-one" && args.deliveryId === "delivery-one") { ' +
        'if (window.__channelCouncilInspectionError) throw new Error(window.__channelCouncilInspectionError); return ' +
        '(window.__channelCouncilInspectorOverride ?? ' + JSON.stringify(inspection) + '); } if (command === "bots_list" || command.startsWith("bot_"))');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    // Channel rendering is async at narrow widths; fixture insertion must
    // target the real, visible thread rather than racing the navigation.
    await expect(page.locator('.conversation-pane.channel-thread .bc-history')).toBeVisible();
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
            cycleId: 'cycle-one', sessionId: 'channel-root', state: 'Passed',
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
    await expect(dialog.getByRole('region', { name: 'Review round 1' })
      .getByText('Tokens unknown', { exact: false })).toBeVisible();
    await expect(dialog.locator('.council-composed-usage-breakdown'))
      .toContainText('Current root and child reservation attempts: 1');
    await expect(dialog.locator('.council-composed-usage-breakdown'))
      .toContainText('Retired child attempts (included in total): 2');
    await expect(dialog.getByText('<script>alert(1)</script>', { exact: false })).toBeVisible();
    await expect(dialog.locator('script')).toHaveCount(0);
    const child = dialog.locator('.council-child-reservations details[data-council-child-key="channel-root:1000:channel-child-one"]');
    await child.locator('summary').click();
    await expect(child).toHaveAttribute('open', '');
    const refresh = dialog.getByRole('button', { name: 'Refresh native evidence' });
    await refresh.click();
    await expect(refresh).toBeEnabled();
    await expect(child).toHaveAttribute('open', '');
    // The Channel and direct chat use the same task-scoped reviewer findings
    // disclosure: a native update must not lose keyboard focus on the same revision.
    const reviewer = dialog.locator('.council-reviewer-findings[data-council-reviewer-key]');
    const reviewerSummary = reviewer.locator(':scope > summary');
    await reviewerSummary.focus();
    await page.keyboard.press('Enter');
    await reviewerSummary.focus();
    await details.evaluate(node => node.dispatchEvent(new Event('council-evidence-invalidated')));
    await expect(reviewerSummary).toBeFocused();
    await expect(reviewer).toHaveAttribute('open', '');
    await expect(reviewer).toContainText('Channel finding');

    // Even with the same channel delivery and child reservation, a newer native
    // artifact revision must not reopen the earlier revision's drill-down.
    await page.evaluate(next => { (window as any).__channelCouncilInspectorOverride = next; }, {
      ...inspection, teamRevision: 10,
    });
    await refresh.click();
    await expect(refresh).toBeEnabled();
    await expect(child).not.toHaveAttribute('open', '');
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    // A Channel notification invalidates the read-only native approval display
    // without relying on a user clicking Refresh. Other Channels cannot trigger
    // a read of this delivery or silently replace the inspected native scope.
    const accepted = { ...inspection, teamRevision: 10, verifiedPass: true,
      verifiedCompletion: { sessionId: 'channel-root', cycleId: 'channel-root:1000',
        teamRevision: 10, round: 1, lowest: 9.7 } };
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, accepted);
    await refresh.click();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    // A conflicting first-wave receipt must revoke the displayed root verdict
    // without navigating away, even when its root completion still matches.
    await page.evaluate(snapshot => {
      (window as any).__channelCouncilInspectorOverride = snapshot;
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    }, { ...accepted, reservedChildren: [{
      ...accepted.reservedChildren[0],
      verifiedChildReceipt: { rootCycleId: 'channel-root:1000', childSessionId: 'channel-child-one',
        botId: 'foreign-owner', objectiveSha256: 'objective', policySha256: 'policy',
        childConfigSha256: 'config', lowestScore: 9.7, leafWorkerIds: ['leaf-a', 'leaf-b'] },
    }] });
    await expect(dialog).toContainText('Conflicting child receipts in this inspection');
    await expect(dialog).not.toContainText('Verified native pass');
    await expect(dialog.locator('.council-verified-completion')).toHaveCount(0);
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, accepted);
    await refresh.click();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    // A native cycle-wide S2 conflict may reject the entire inspection rather
    // than returning a child projection. The shared view must fail closed.
    await page.evaluate(() => {
      (window as any).__channelCouncilInspectionError =
        'child_council_stale: unexpected triggered S2 receipt /private/channel-child';
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    });
    await expect(dialog.getByRole('alert')).toHaveText(
      'Council child evidence is stale or conflicting. Native approval is blocked; refresh after recovery.');
    await expect(dialog).not.toContainText('/private/channel-child');
    await expect(dialog).not.toContainText('Verified native pass');
    await expect(dialog.locator('.council-verified-completion')).toHaveCount(0);
    await page.evaluate(() => { delete (window as any).__channelCouncilInspectionError; });
    await refresh.click();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    // Missing required phase evidence is pending, not a conflicting receipt.
    await page.evaluate(snapshot => {
      (window as any).__channelCouncilInspectorOverride = snapshot;
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    }, { ...accepted, strategy: 'escalated', forceEscalation: true });
    await expect(dialog).toContainText('Required child phase receipts are pending');
    await expect(dialog).not.toContainText('Conflicting child receipts in this inspection');
    await expect(dialog).not.toContainText('Verified native pass');
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, accepted);
    await refresh.click();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'other-channel' }));
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    await refresh.focus();
    await page.evaluate(snapshot => {
      (window as any).__channelCouncilInspectorOverride = snapshot;
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    }, { ...inspection, teamRevision: 11, status: { ...inspection.status, state: 'reviewing', stale: true } });
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    await expect(dialog.getByText('Stale revision', { exact: false })).toBeVisible();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toHaveCount(0);
    await expect(refresh).toBeFocused();
    // A collapsed inspector must not retain a private snapshot in the DOM.
    await details.locator(':scope > summary').click();
    await expect(details.locator('.council-inspection-body')).toBeEmpty();
    await page.evaluate(snapshot => {
      (window as any).__channelCouncilInspectorOverride = snapshot;
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    }, { ...inspection, reportedFindings: [], reservedChildren: [{ botId: 'Foreign child bot',
      childSessionId: 'foreign-child', rootCycleId: 'other-channel:1000', requiredLeafReviewers: 2, status: 'reviewing' }] });
    await expect(details.locator('.council-inspection-body')).toBeEmpty();
    await details.locator(':scope > summary').click();
    await expect(dialog.getByRole('alert')).toContainText('1 foreign-cycle or malformed child Council reservation(s) omitted');
    await expect(dialog).not.toContainText('Foreign child bot');
    await expect(dialog).not.toContainText('Channel finding');
    await expect(dialog).not.toContainText('Verified native pass');
    // The browser must use the channel's delivery identity rather than allowing
    // an arbitrary DOM session id to select an unrelated Council.
    await expect(page.getByRole('button', { name: 'Inspect Council evidence' }))
      .toHaveAttribute('data-delivery-id', 'delivery-one');
    await expect(page.getByRole('button', { name: 'Inspect Council evidence' }))
      .not.toHaveAttribute('data-session-id', /.+/);
    // The inspection belongs to its original Channel delivery even after
    // navigating back to the ordinary conversation. A later notification from
    // that Channel must revoke a cached pass; other Channels must not.
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, accepted);
    await refresh.click();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    await page.evaluate(() => {
      const back = document.querySelector<HTMLButtonElement>('[data-action="close-shared-channel"]');
      if (!back) throw new Error('Channel back navigation is unavailable');
      back.click();
    });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole('textbox', { name: 'Message channel' })).toHaveCount(0);
    await page.evaluate(() => (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'other-channel' }));
    await expect(dialog.getByText('Verified native pass', { exact: false })).toBeVisible();
    await page.evaluate(next => {
      (window as any).__channelCouncilInspectorOverride = next;
      (window as any).__previewEmit('bot-shared-channel-changed', { channelId: 'channel-one' });
    }, { ...inspection, teamRevision: 12, status: { ...inspection.status, state: 'reviewing', stale: true } });
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    await expect(dialog.getByText('Verified native pass', { exact: false })).toHaveCount(0);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('textbox', { name: 'Message channel' })).toHaveCount(0);
  });


  test('Channel native inspection never replaces its bound delivery with a foreign root result', async ({ page }) => {
    const own = {
      sessionId: 'channel-root', cycleStartedAt: 1000, teamRevision: 7,
      status: { state: 'reviewing', round: 1, lowest: null, target: 9, stale: false },
      verifiedPass: false, criteria: [], attempts: [], reservedChildren: [],
    };
    const foreign = {
      ...own, sessionId: 'other-channel-root', cycleStartedAt: 2000,
      status: { state: 'passed', round: 1, lowest: 9.9, target: 9, stale: false },
      verifiedPass: true,
      verifiedCompletion: { sessionId: 'other-channel-root', cycleId: 'other-channel-root:2000',
        teamRevision: 7, round: 1, lowest: 9.9 },
      reportedFindings: [{ schemaVersion: 1, id: 'private:1', cycleId: 'other-channel-root:2000',
        round: 1, workerId: 'private', reportMessageId: 4, lens: 'Security',
        severity: 'high', category: 'privacy', title: 'Other Channel secret finding',
        explanation: 'Another delivery', artifactRevision: 'other-revision',
        verifiedEvidenceRefs: [], evidenceVerified: false, locationVerified: false }],
    };
    await page.route('**/src/preview.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replace(/if\s*\(\s*command\s*===\s*['"]bots_list['"]\s*\|\|\s*command\.startsWith\(\s*['"]bot_['"]\s*\)\s*\)/,
        'if (command === "bot_shared_channel_council_inspect") return ' +
        '(window.__channelCouncilInspectorOverride ?? ' + JSON.stringify(own) + '); if (command === "bots_list" || command.startsWith("bot_"))');
      await route.fulfill({ response, body });
    });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    await expect(page.locator('.conversation-pane.channel-thread .bc-history')).toBeVisible();
    await page.evaluate(async () => {
      const { sharedChannelThreadHtml } = await import('/src/bot-shared-channels.ts');
      const wrapper = document.createElement('div');
      wrapper.innerHTML = sharedChannelThreadHtml({
        id: 'channel-one', name: 'launch', memberIds: [], leadBotId: null,
        archived: false, updatedAt: 1, activeBotIds: [],
        messages: [{ id: 'delivery-one', kind: 'assigned', authorBotId: null,
          text: '', status: 'Completed', createdAt: 1,
          council: { cycleId: 'cycle-one', sessionId: 'channel-root', state: 'Reviewing',
            round: 1, target: 9, lowest: null, artifactRevision: 'artifact', reviewers: [] } }],
      }, []);
      const assignment = wrapper.querySelector('.bc-assigned');
      const history = document.querySelector('.bc-history');
      if (!assignment || !history) throw new Error('Channel fixture is missing');
      history.append(assignment);
    });
    // A foreign response on the FIRST native read must never become the bound
    // Channel task or display its private findings/approval.
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, foreign);
    await page.getByRole('button', { name: 'Inspect Council evidence' }).click();
    const dialog = page.getByRole('dialog', { name: 'Council inspection' });
    await expect(dialog.getByRole('alert')).toContainText('Council evidence does not match this task');
    await expect(dialog).not.toContainText('Other Channel secret finding');
    await expect(dialog).not.toContainText('Verified native pass');
    const refresh = dialog.getByRole('button', { name: 'Refresh native evidence' });
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, own);
    await refresh.click();
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    await expect(dialog.getByRole('alert')).toHaveCount(0);
    // The same binding must remain in force across subsequent refreshes.
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, foreign);
    await refresh.click();
    await expect(dialog.getByRole('alert')).toContainText('Council evidence does not match this task');
    await expect(dialog).not.toContainText('Other Channel secret finding');
    await expect(dialog).not.toContainText('Verified native pass');
    await page.evaluate(snapshot => { (window as any).__channelCouncilInspectorOverride = snapshot; }, own);
    await refresh.click();
    await expect(dialog.getByText('Native pass not verified', { exact: false })).toBeVisible();
    await expect(dialog.getByRole('alert')).toHaveCount(0);
  });

  test('bot collaboration: narrow Channel Council keeps long reviewer identities and inspection actions within the thread', async ({ page }) => {
    test.setTimeout(60_000);
    await page.setViewportSize({ width: 320, height: 640 });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    // Channel rendering is async at narrow widths; fixture insertion must
    // target the real, visible thread rather than racing the navigation.
    await expect(page.locator('.conversation-pane.channel-thread .bc-history')).toBeVisible();
    await page.evaluate(async () => {
      const { sharedChannelThreadHtml } = await import('/src/bot-shared-channels.ts');
      const longName = 'SpecialistSecurityReviewerWithAnUnbrokenDisplayName'.repeat(4);
      const wrapper = document.createElement('div');
      wrapper.innerHTML = sharedChannelThreadHtml({
        id: 'channel-one', name: 'launch', memberIds: [], leadBotId: null,
        archived: false, updatedAt: 1, activeBotIds: [],
        messages: [{
          id: 'long-council-delivery', kind: 'assigned', authorBotId: null,
          text: '', status: 'Completed', createdAt: 1,
          council: {
            cycleId: 'cycle-one', sessionId: 'task-owned-channel-session',
            state: 'Reviewing', round: 1, target: 9, lowest: null,
            artifactRevision: 'frozen-revision', reviewers: [{
              botId: 'security-bot', name: longName, lens: longName,
              round: 1, score: null, skills: [],
            }],
          },
        }],
      }, []);
      const assignment = wrapper.querySelector('.bc-assigned');
      const history = document.querySelector('.bc-history');
      if (!assignment || !history) throw new Error('Channel assignment fixture is missing');
      history.append(assignment);
    });
    const assignment = page.locator('.bc-history .bc-assigned[data-message-id="long-council-delivery"]');
    const inspector = assignment.getByRole('button', { name: 'Inspect Council evidence' });
    await expect(assignment.locator('.bc-council-reviewers')).toContainText('SpecialistSecurityReviewer');
    await expect(inspector).toBeVisible();
    await expect(assignment.getByRole('button', { name: 'View Council report' })).toBeVisible();
    const overflow = await assignment.evaluate(element => {
      const history = element.closest<HTMLElement>('.bc-history')!;
      const summary = element.querySelector<HTMLElement>('.bc-council-summary')!;
      const parentBounds = history.getBoundingClientRect();
      const assignmentBounds = element.getBoundingClientRect();
      return {
        assignment: element.scrollWidth - element.clientWidth,
        summary: summary.scrollWidth - summary.clientWidth,
        left: parentBounds.left - assignmentBounds.left,
        right: assignmentBounds.right - parentBounds.right,
      };
    });
    expect(overflow.assignment).toBeLessThanOrEqual(1);
    expect(overflow.summary).toBeLessThanOrEqual(1);
    expect(overflow.left).toBeLessThanOrEqual(1);
    expect(overflow.right).toBeLessThanOrEqual(1);
    await inspector.focus();
    await expect(inspector).toBeFocused();
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
