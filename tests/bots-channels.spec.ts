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

  test('shared channel schema-v1 preview shows the draft without implying review evidence', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();

    const preview = panel.locator('[data-bc-plan-preview]');
    const previewText = panel.locator('[data-bc-plan-preview-text]');
    await expect(preview).toBeVisible();
    await expect(previewText).toContainText('Draft only · configuration preview, not a review result');
    await expect(previewText).toContainText('Schema version: 1');
    await expect(previewText).toContainText('Reviewer sources: A · M1');
    await expect(previewText).toContainText('A · Automatic Architect reviewers: Auto');
    await expect(previewText).toContainText('Strategy: S1 · Parallel');
    await expect(previewText).toContainText('Architect-provided criteria are not included in this composer preview.');
    await expect(previewText).toContainText('Evidence status: not evaluated in this preview.');

    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    await panel.locator('[data-bc-criterion-field="description"]').fill('Check the acceptance evidence.');
    await expect(previewText).toContainText('Criteria source: User');
    await expect(previewText).toContainText('Check the acceptance evidence.');
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
    const reviewers = panel.locator('[data-bc-reviewer]');
    const configureComposition = async (strategy: string) => {
      await panel.getByRole('switch', { name: 'Council' }).check();
      await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
      await panel.getByRole('combobox', { name: 'Automatic reviewer count' }).selectOption('2');
      await reviewers.nth(1).check();
      await reviewers.nth(2).check();
      await reviewerRows.nth(1).getByRole('combobox', { name: 'Reviewer branch' }).selectOption('direct');
      const nestedPolicy = reviewerRows.nth(2).locator('[data-bc-nested-policy]');
      await reviewerRows.nth(2).getByRole('combobox', { name: 'Reviewer branch' }).selectOption('nested');
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

  test('no-mention M2 composition accepts two non-lead reviewers beside the fallback lead', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await panel.getByRole('checkbox', { name: 'Include automatic Architect reviewers (A)' }).uncheck();
    const rows = panel.locator('[data-bc-reviewer-row]');
    const names = await rows.locator('.council-composition-bot strong').allTextContents();
    expect(names).toEqual(['Scout', 'Writer', 'Ledger', 'Dozy']);
    const reviewers = panel.locator('[data-bc-reviewer]');
    await reviewers.nth(1).check();
    await reviewers.nth(2).check();
    await expect(panel.locator('[data-bc-source-summary]')).toHaveText('Sources D · M2');
    const selected = await reviewers.evaluateAll(inputs => inputs.filter(input => (input as HTMLInputElement).checked).map(input => (input as HTMLInputElement).value));
    await prompt.fill('Review the launch plan without naming an executor');
    expect(await prompt.inputValue()).not.toMatch(/@/);

    await composer.locator('button[type="submit"]').click();
    const posts = () => page.evaluate(() => (window as any).__previewSharedChannelPosts ?? []);
    await expect.poll(async () => (await posts()).length).toBe(1);
    expect((await posts())[0]).toMatchObject({
      channelId: 'sample-launch',
      text: 'Review the launch plan without naming an executor',
      council: {
        reviewers: 0,
        reviewerBotIds: selected,
        composition: {
          schemaVersion: 1,
          automatic: { enabled: false, count: 0 },
          selected: selected.map((botId: string) => ({ botId, mode: 'direct', required: true })),
        },
      },
    });
  });

  test('no-mention M2 composition rejects a selected fallback lead', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await panel.getByRole('checkbox', { name: 'Include automatic Architect reviewers (A)' }).uncheck();
    const rows = panel.locator('[data-bc-reviewer-row]');
    await expect(rows.first().locator('.council-composition-bot strong')).toHaveText('Scout');
    await panel.locator('[data-bc-reviewer]').nth(0).check();
    await panel.locator('[data-bc-reviewer]').nth(1).check();
    await expect(panel.locator('[data-bc-source-summary]')).toHaveText('Sources D · M2');
    await prompt.fill('Review without explicit @mentions');

    await composer.locator('button[type="submit"]').click();
    const posts = () => page.evaluate(() => (window as any).__previewSharedChannelPosts ?? []);
    expect(await posts()).toHaveLength(0);
    await expect(page.locator('#toast')).toContainText('A message executor cannot also be one of its Council reviewers.');
    await expect(composer).toHaveAttribute('aria-busy', 'false');
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

  test('shared channel legacy selected-bot Council submission stays available when composition is off', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.locator('input[name="councilMode"][value="selected"]').check();
    const reviewers = panel.locator('[data-bc-legacy-reviewer]');
    await reviewers.nth(1).check();
    await reviewers.nth(2).check();
    const selectedIds = await reviewers.evaluateAll(inputs => inputs.filter(input => (input as HTMLInputElement).checked).map(input => (input as HTMLInputElement).value));
    await expect(panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' })).not.toBeChecked();
    await prompt.fill('Keep selected legacy reviewers');
    const target = panel.getByRole('spinbutton', { name: 'Custom target score' });
    await target.fill('11');
    await composer.locator('button[type="submit"]').click();
    await expect(target).toHaveAttribute('aria-invalid', 'true');
    await expect(target).toBeFocused();
    const posts = () => page.evaluate(() => (window as any).__previewSharedChannelPosts ?? []);
    expect(await posts()).toHaveLength(0);

    // Correcting the legacy target clears its error without dropping the selected M2 reviewers.
    await target.fill('9');
    await expect(target).not.toHaveAttribute('aria-invalid', 'true');
    await composer.locator('button[type="submit"]').click();

    await expect.poll(async () => (await posts()).length).toBe(1);
    expect((await posts())[0]).toEqual({
      channelId: 'sample-launch',
      text: 'Keep selected legacy reviewers',
      council: { target: 9, maxRounds: 10, reviewers: 0, reviewerBotIds: selectedIds },
    });
  });

  test('shared channel Council submission announces and clears its loading state', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await page.getByRole('textbox', { name: 'Message channel' }).fill('Wait for the channel post');
    await page.evaluate(() => {
      (window as any).__channelPostLoadingState = new Promise(resolve => {
        document.addEventListener('submit', event => {
          if ((event.target as HTMLFormElement | null)?.id !== 'shared-channel-composer') return;
          queueMicrotask(() => {
            const form = document.querySelector<HTMLFormElement>('#shared-channel-composer')!;
            resolve({
              busy: form.getAttribute('aria-busy'),
              promptDisabled: form.querySelector<HTMLTextAreaElement>('#shared-channel-prompt')?.disabled,
              sendDisabled: form.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled,
              liveText: form.querySelector<HTMLElement>('[data-bc-post-status]')?.textContent?.trim(),
              liveRole: form.querySelector<HTMLElement>('[data-bc-post-status]')?.getAttribute('role'),
              liveMode: form.querySelector<HTMLElement>('[data-bc-post-status]')?.getAttribute('aria-live'),
            });
          });
        }, { once: true });
      });
    });
    await composer.locator('button[type="submit"]').click();
    const loading = await page.evaluate(() => (window as any).__channelPostLoadingState);
    expect(loading).toEqual({ busy: 'true', promptDisabled: true, sendDisabled: true, liveText: 'Sending message to channel…', liveRole: 'status', liveMode: 'polite' });
    await expect.poll(() => composer.getAttribute('aria-busy')).toBe('false');
    await expect(composer.getByRole('textbox', { name: 'Message channel' })).toBeEnabled();
    await expect(composer.locator('button[type="submit"]')).toBeEnabled();
  });

  test('shared channel post errors are announced in an assertive live region', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    await page.getByRole('textbox', { name: 'Message channel' }).fill('Try the archived channel');
    await page.evaluate(async () => {
      const { invokeBots } = await import('/src/preview-bots.ts');
      await invokeBots('bot_shared_channel_archive', { channelId: 'sample-launch', archived: true }, {
        invoke: async () => null,
        emit: () => {},
        addProject: () => ({}) as any,
      } as any);
    });

    await page.locator('#shared-channel-composer button[type="submit"]').click();
    const status = page.locator('#shared-channel-composer [data-bc-post-status]');
    await expect(status).toHaveAttribute('role', 'alert');
    await expect(status).toHaveAttribute('aria-live', 'assertive');
    await expect(status).toHaveText('Message not sent. This channel is deleted.');
  });

  test('schema-v1 validation reopens the composition popover after it was closed', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await panel.getByRole('checkbox', { name: 'Include automatic Architect reviewers (A)' }).uncheck();
    const reviewer = panel.locator('[data-bc-reviewer-row]').first();
    await reviewer.locator('[data-bc-reviewer]').check();
    const branch = reviewer.getByRole('combobox', { name: 'Reviewer branch' });
    await prompt.fill('Validate from the closed popover');
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();

    await composer.locator('button[type="submit"]').click();
    await expect(panel).toBeVisible();
    await expect(branch).toHaveAttribute('aria-invalid', 'true');
    await expect(branch).toBeFocused();
    await expect(panel.locator('[data-bc-validation-summary]')).toContainText('A/D/N selection provides fewer than 2 root reviewer seats');
  });

  test('schema-v1 Council popover contains forward and reverse Tab navigation', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    const enabled = panel.getByRole('switch', { name: 'Council' });
    await enabled.check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await panel.locator('[data-bc-reviewer]').first().check();
    const last = panel.locator('input[name="councilFilePath"]');
    await expect(last).toBeVisible();
    await last.focus();
    await page.keyboard.press('Tab');
    await expect(enabled).toBeFocused();
    await page.keyboard.press('Shift+Tab');
    await expect(last).toBeFocused();
  });

  test('shared channel Council submission focuses and scrolls an invalid nested field into view', async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 360, height: 440 });
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await panel.getByRole('combobox', { name: 'Automatic reviewer count' }).selectOption('2');
    const rows = panel.locator('[data-bc-reviewer-row]');
    for (const index of [0, 1]) {
      await rows.nth(index).locator('[data-bc-reviewer]').check();
      await rows.nth(index).getByRole('combobox', { name: 'Reviewer branch' }).selectOption('nested');
    }
    const childTarget = rows.nth(1).locator('[data-bc-nested-field="target"]');
    await childTarget.fill('11');
    await prompt.fill('Show the invalid child target');
    await panel.evaluate(element => { element.scrollTop = 0; });
    const before = await childTarget.evaluate(element => {
      const panel = element.closest<HTMLElement>('.bc-council-panel')!;
      const field = element.getBoundingClientRect(), frame = panel.getBoundingClientRect();
      return field.bottom > frame.bottom || field.top < frame.top;
    });
    expect(before).toBe(true);

    await composer.locator('button[type="submit"]').click();
    await expect(childTarget).toHaveAttribute('aria-invalid', 'true');
    await expect(childTarget).toBeFocused();
    const after = await childTarget.evaluate(element => {
      const panel = element.closest<HTMLElement>('.bc-council-panel')!;
      const field = element.getBoundingClientRect(), frame = panel.getBoundingClientRect();
      const reviewerList = element.closest<HTMLElement>('[data-bc-reviewer-group]');
      return {
        insidePanel: field.top >= frame.top && field.bottom <= frame.bottom,
        scrolled: panel.scrollTop > 0 || Boolean(reviewerList?.scrollTop),
      };
    });
    expect(after).toEqual({ insidePanel: true, scrolled: true });
  });

  test('shared channel Council validation points to A/D/N, strategy, criteria and nested fields, then clears', async ({ page }) => {
    test.setTimeout(120_000);
    await showBots(page);
    await sidebar(page).getByRole('button', { name: '#launch' }).click();
    const composer = page.locator('#shared-channel-composer');
    const prompt = page.getByRole('textbox', { name: 'Message channel' });
    await composer.getByRole('button', { name: 'Council: off' }).click();
    const panel = composer.getByRole('dialog', { name: 'Council' });
    await panel.getByRole('switch', { name: 'Council' }).check();
    await panel.getByRole('checkbox', { name: 'Use a schema v1 composition plan' }).check();
    await prompt.fill('Validate the visible Council plan');

    // A alone with one automatic reviewer seat cannot meet the two-seat minimum.
    await panel.getByRole('combobox', { name: 'Automatic reviewer count' }).selectOption('1');
    await composer.locator('button[type="submit"]').click();
    const automaticCount = panel.getByRole('combobox', { name: 'Automatic reviewer count' });
    await expect(automaticCount).toHaveAttribute('aria-invalid', 'true');
    await expect(panel.locator('[data-bc-validation-summary]')).toContainText('A/D/N selection provides fewer than 2 root reviewer seats');
    await automaticCount.selectOption('2');
    await expect(automaticCount).not.toHaveAttribute('aria-invalid', 'true');
    await expect(panel.locator('[data-bc-validation-summary]')).toBeHidden();

    // D alone with one selected member also needs another root seat.
    await panel.getByRole('checkbox', { name: 'Include automatic Architect reviewers (A)' }).uncheck();
    const reviewers = panel.locator('[data-bc-reviewer]');
    await reviewers.nth(0).check();
    const firstBranch = panel.locator('[data-bc-reviewer-row]').nth(0).getByRole('combobox', { name: 'Reviewer branch' });
    await composer.locator('button[type="submit"]').click();
    await expect(firstBranch).toHaveAttribute('aria-invalid', 'true');
    await reviewers.nth(1).check();
    await expect(firstBranch).not.toHaveAttribute('aria-invalid', 'true');

    // N validation belongs to the selected nested reviewer branch.
    await reviewers.nth(1).uncheck();
    await firstBranch.selectOption('nested');
    await composer.locator('button[type="submit"]').click();
    await expect(firstBranch).toHaveAttribute('aria-invalid', 'true');
    await reviewers.nth(1).check();
    const secondBranch = panel.locator('[data-bc-reviewer-row]').nth(1).getByRole('combobox', { name: 'Reviewer branch' });
    await secondBranch.selectOption('nested');
    await expect(firstBranch).not.toHaveAttribute('aria-invalid', 'true');

    const strategy = panel.getByRole('combobox', { name: 'Execution strategy' });
    await strategy.evaluate(element => {
      const select = element as HTMLSelectElement;
      select.add(new Option('Unsupported strategy', 'unsupported'));
      select.value = 'unsupported';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await composer.locator('button[type="submit"]').click();
    await expect(strategy).toHaveAttribute('aria-invalid', 'true');
    await strategy.selectOption('parallel');
    await expect(strategy).not.toHaveAttribute('aria-invalid', 'true');

    await panel.getByRole('combobox', { name: 'Criteria source' }).selectOption('user');
    const description = panel.locator('[data-bc-criterion-field="description"]');
    await description.fill('');
    await composer.locator('button[type="submit"]').click();
    await expect(description).toHaveAttribute('aria-invalid', 'true');
    await description.fill('Check the published acceptance criteria.');
    await expect(description).not.toHaveAttribute('aria-invalid', 'true');

    const nestedTarget = panel.locator('[data-bc-reviewer-row]').first().locator('[data-bc-nested-field="target"]');
    await nestedTarget.fill('11');
    await composer.locator('button[type="submit"]').click();
    await expect(nestedTarget).toHaveAttribute('aria-invalid', 'true');
    await nestedTarget.fill('8.5');
    await expect(nestedTarget).not.toHaveAttribute('aria-invalid', 'true');
    await expect(panel.locator('[data-bc-validation-summary]')).toBeHidden();
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
    const reviewers = panel.locator('[data-bc-reviewer]');
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
    await expect(restoredPanel.locator('[data-bc-reviewer]').first()).toBeChecked();
  });

  test('channel Council report shows the same current-cycle hierarchy and resolved evidence as Workers', async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem('pandacode.teamClassic', 'false'));
    await page.goto('/?bots=sample&council=ledger');
    await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeVisible();

    const directCycleId = await page.evaluate(async () => {
      const { call } = await import('/src/bridge.ts');
      const snapshot = await call<any>('architect_get', { sessionId: 'session-1' });
      if (!snapshot) return null;
      const { ArchitectView } = await import('/src/architect.ts');
      const view = new ArchitectView({ setup: () => {}, loadModels: async () => [], updated: () => {}, report: () => {} });
      document.body.append(view.host);
      view.setTask(snapshot.sessionId, snapshot, []);
      view.open(false);
      return document.querySelector('.architect-panel .council-ledger')?.getAttribute('data-cycle-id') ?? null;
    });
    expect(directCycleId).toBeTruthy();
    const directLedger = page.locator('.architect-panel .council-ledger');
    await expect(directLedger).toBeVisible();
    await expect(directLedger).toHaveAttribute('data-cycle-id', directCycleId!);
    const directHierarchy = directLedger.locator('.council-inspector-hierarchy');
    const directHierarchyText = await directHierarchy.evaluate(node => node.textContent?.replace(/\s+/g, ' ').trim() ?? '');
    const directRootFinding = directLedger.locator('.council-ledger-finding').filter({ hasText: 'A current root-cycle issue.' });
    await expect(directRootFinding).toHaveCount(1);

    const botBar = page.locator('.sidebar-bots-bar');
    if (await botBar.getAttribute('aria-pressed') !== 'true') await botBar.click();
    const channelButton = page.getByRole('button', { name: '#launch', exact: true });
    await expect(channelButton).toBeVisible();
    await channelButton.click();
    await expect(page.locator('.conversation-pane.channel-thread .bc-channel')).toBeVisible();
    const channelMarkup = await page.evaluate(async cycleId => {
      const { sharedChannelThreadHtml } = await import('/src/bot-shared-channels.ts');
      return sharedChannelThreadHtml({
        id: 'sample-launch', name: 'launch', memberIds: [], leadBotId: null,
        archived: false, updatedAt: 1, activeBotIds: [],
        messages: [{
          id: 'report-delivery', kind: 'assigned', authorBotId: null,
          text: 'Council review completed', status: 'Completed', createdAt: 1,
          council: {
            cycleId, sessionId: 'session-1', state: 'Passed',
            round: 1, target: 9.5, lowest: 9.6, artifactRevision: 'preview-revision-1', reviewers: [],
          },
        }],
      }, []);
    }, directCycleId!);
    await page.locator('.conversation-pane.channel-thread .bc-channel').evaluate((node, html) => { node.outerHTML = html; }, channelMarkup);
    await page.getByRole('button', { name: 'View Council report' }).click();

    const dialog = page.getByRole('dialog', { name: 'Council report' });
    const report = dialog.locator('.bc-council-report');
    const reportLedger = report.locator('.council-ledger');
    await expect(reportLedger).toBeVisible();
    await expect(reportLedger).toHaveAttribute('data-cycle-id', directCycleId!);
    await expect(reportLedger).not.toContainText('earlier-cycle finding');
    const reportHierarchy = reportLedger.locator('.council-inspector-hierarchy');
    const reportHierarchyText = await reportHierarchy.evaluate(node => node.textContent?.replace(/\s+/g, ' ').trim() ?? '');
    expect(reportHierarchyText).toBe(directHierarchyText);
    const rootFinding = reportLedger.locator('.council-ledger-finding').filter({ hasText: 'A current root-cycle issue.' });
    await rootFinding.getByText('Evidence and accepted exceptions', { exact: true }).click();
    const evidenceButton = rootFinding.getByRole('button', { name: 'src/main.ts', exact: true }).first();
    await expect(evidenceButton).toHaveClass(/workspace-file-link/);

    const root = reportHierarchy.locator('[data-level="root-council"]');
    const bot = root.locator('[data-level="bot"]').filter({ hasText: 'Security' }).first();
    await bot.locator(':scope > summary').click();
    const child = bot.locator('[data-level="child-council"]');
    await child.locator(':scope > summary').click();
    const round = child.locator('[data-level="round"]').first();
    await round.locator(':scope > summary').click();
    const reviewer = round.locator('[data-level="reviewer"]').first();
    await reviewer.locator(':scope > summary').click();
    await expect(reviewer.locator(':scope > summary')).toContainText('Score Unknown');
    await expect(reviewer).toContainText('child reviewer scores are not included in this projection');
    const reviewerEvidence = reviewer.locator('.council-inspector-evidence');
    await reviewerEvidence.locator(':scope > summary').click();
    await expect(reviewerEvidence).toContainText('src/main.ts');
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
