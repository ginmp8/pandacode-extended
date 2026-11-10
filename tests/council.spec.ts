import { expect, test, type Page } from '@playwright/test';

const button = (page: Page) => page.locator('.composer-council-button');
const panel = (page: Page) => page.locator('.council-panel');
const toggle = (page: Page) => panel(page).getByRole('switch', { name: 'Council' });
/** Eligible reviewer identities, excluding the current task executor and unavailable bots. */
const eligibleReviewers = (page: Page) => panel(page).locator('.council-bots input[data-reviewer-bot]:not(:disabled)');
const option = (page: Page, field: string, name: string) => panel(page).locator(`[data-field="${field}"]`).getByRole('button', { name, exact: true });
type ReviewerFixture = { ids: string[]; bots: { id: string; name: string; role: string; provider: string; model: string; paused: boolean; skills?: string[]; available?: boolean; reason?: string }[] };

// The first paint can be slow on a busy machine; what is under test is what happens after it.
test.setTimeout(60_000);

/** Records every architect_set_council; `refuse` makes the mock refuse a command, like a native error. */
async function open(page: Page, url = '/', refuse: Record<string, string> = {}, delay = 0, delaySession = '', slow: Record<string, number> = {}, reviewers?: ReviewerFixture): Promise<void> {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch();
    const body = (await response.text()).replace(/switch\s*\(command\)\s*\{/, `
      if (command === 'architect_set_council') (window.__councilCalls ??= []).push(structuredClone(args.council));
      if (command === 'architect_start') (window.__councilStarts ??= []).push(args.text);
      if (command === 'architect_council_accept') (window.__councilAccepts ??= []).push(args.sessionId);
      if (command === 'architect_set_council' && ${delay} && (!${JSON.stringify(delaySession)} || args.sessionId === ${JSON.stringify(delaySession)})) await new Promise(done => setTimeout(done, ${delay}));
      if (${JSON.stringify(slow)}[command]) await new Promise(done => setTimeout(done, ${JSON.stringify(slow)}[command]));
      if (window.__hold?.[command]) await window.__hold[command];
      if (${JSON.stringify(refuse)}[command]) throw new Error(${JSON.stringify(refuse)}[command]);
      if (command === 'architect_get' && ${Boolean(reviewers)}) {
        const team = architect.get(args.sessionId);
        if (team?.config.council) team.config.council.reviewerBotIds = ${JSON.stringify(reviewers?.ids ?? [])};
        return team;
      }
      if (command === 'architect_council_reviewer_options' && ${Boolean(reviewers)}) return ${JSON.stringify(reviewers?.bots ?? [])};
      switch (command) {
    `);
    await route.fulfill({ response, body });
  });
  await page.goto(url);
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeVisible({ timeout: 30_000 });
}
const calls = (page: Page) => page.evaluate(() => (window as any).__councilCalls ?? []);
const strip = (page: Page) => page.locator('#composer .council-strip');
const disclosure = (page: Page) => strip(page).locator('.council-strip-toggle');
const ready = (page: Page) => expect(page.locator('html')).toHaveAttribute('data-ready', '');
/** Past the quiet window after the thread and its actual team have shown. The tray can precede the team snapshot. */
const settle = async (page: Page) => { await expect(button(page)).toBeVisible(); await expect(strip(page)).toBeVisible(); await ready(page); await page.waitForTimeout(700); };

// One describe, so the WebKit config's title grep can select every Council test.
test.describe('Council', () => {
test('selected configured bots are task-scoped, attributed by ID, and automatic reviewers remain the default', async ({ page }) => {
  await open(page, '/?bots=sample');
  await button(page).click();
  await expect(option(page, 'reviewers', 'Auto: the lead picks reviewers for the work')).toHaveAttribute('aria-pressed', 'true');
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  // An unavailable bot may be below the scrollable panel's fold; scrolling must expose it.
  const pausedBot = panel(page).getByRole('checkbox', { name: /Dozy/ });
  await expect(pausedBot).toBeDisabled();
  const pausedName = panel(page).getByText('Dozy');
  await pausedName.scrollIntoViewIfNeeded();
  await expect(pausedName).toBeVisible();
  await expect(panel(page).getByText('Paused')).toBeVisible();
  // The sample's current executor is timing-dependent; never select a forbidden reviewer.
  await expect.poll(async () => eligibleReviewers(page).count()).toBeGreaterThanOrEqual(2);
  await eligibleReviewers(page).nth(0).check();
  await eligibleReviewers(page).nth(1).check();
  expect(await calls(page)).toEqual([]);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page)).length).toBe(1);
  const [selected] = await calls(page);
  expect(selected.reviewerBotIds).toHaveLength(2);
  expect(new Set(selected.reviewerBotIds).size).toBe(2);
  await expect(eligibleReviewers(page).nth(0)).toBeChecked();
  await option(page, 'reviewers', 'Auto: the lead picks reviewers for the work').click();
  await expect.poll(async () => (await calls(page)).length).toBe(2);
  expect((await calls(page))[1].reviewerBotIds).toBeUndefined();
});

test('optional selected-bot skill remains optional after Council configuration is reloaded', async ({ page }) => {
  const bots = [
    { id: 'reviewer-alpha', name: 'Alpha', role: 'QA', provider: 'codex', model: 'gpt-5', paused: false, skills: ['risk-scan'] },
    { id: 'reviewer-beta', name: 'Beta', role: 'QA', provider: 'claude', model: 'sonnet', paused: false, skills: [] },
  ];
  await open(page, '/?bots=sample', {}, 0, '', {}, { ids: bots.map(bot => bot.id), bots });
  await button(page).click();
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  await expect.poll(async () => eligibleReviewers(page).count()).toBe(2);
  await eligibleReviewers(page).nth(0).check();
  await eligibleReviewers(page).nth(1).check();
  await toggle(page).click();
  await expect.poll(async () => (await calls(page)).length).toBeGreaterThanOrEqual(1);
  const optional = panel(page).getByRole('group', { name: 'Optional reviewer skills' }).getByRole('checkbox', { name: 'risk-scan' });
  // Applying the setting replaces the input node; assert the persisted value, not the old element.
  await optional.click();
  await expect.poll(async () => (await calls(page)).at(-1)?.optionalSkillIds).toEqual(['risk-scan']);

  await page.reload();
  // A browser reload restarts the preview app; wait for the same boot-ready
  // composer condition as open() before asserting persisted Council settings.
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeVisible({ timeout: 30_000 });
  // The composer renders before the persisted native team/Council snapshot loads.
  // Wait for that Council state, not merely the composer, before opening its settings.
  await expect(strip(page)).toBeVisible();
  await ready(page);
  await expect(button(page)).toBeVisible();
  await button(page).click();
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  await expect(panel(page).getByRole('group', { name: 'Optional reviewer skills' }).getByRole('checkbox', { name: 'risk-scan' })).toBeChecked();
  await expect(panel(page).getByRole('group', { name: 'Required native skill checks' }).getByRole('checkbox', { name: 'risk-scan' })).not.toBeChecked();
});

test('selected reviewer skill receipts show native supply and optional failures without claiming execution', async ({ page }) => {
  await open(page, '/?council=fixing');
  const status = await page.evaluate(async () => {
    const { councilSummaryHtml } = await import('/src/council-panel.ts');
    const team: any = {
      sessionId: 'skills-seat', status: 'completed', councilSince: 1,
      config: { council: { target: 9, maxRounds: 10, reviewers: 2 } },
      councilStatus: { state: 'passed', round: 1, lowest: 9.2, target: 9, stale: false },
      workers: [{
        id: 'seat-1', name: 'Configured reviewer', status: 'completed', createdAt: 2,
        council: { round: 1, lens: 'Security', score: 9.2 },
        reviewerBinding: {
          reviewerBotId: 'reviewer-1',
          snapshot: { skills: {
            pins: [{
              id: 'required-scan', contentSha256: 'a'.repeat(64),
              supplied: true, invoked: false, checkExecuted: false, evidenceVerified: false, checkRequired: true,
            }],
            skipped: [{ id: 'optional-scan', reason: 'reviewer_skill_unavailable: missing package <untrusted>' }],
          } },
        },
      }],
    };
    const document = new DOMParser().parseFromString(councilSummaryHtml(team), 'text/html');
    const row = document.querySelector('.council-row');
    const detail = row?.querySelector('.council-row-skills');
    return {
      text: detail?.textContent ?? '',
      title: detail?.getAttribute('title') ?? '',
      label: row?.getAttribute('aria-label') ?? '',
      injectedElementCount: document.querySelectorAll('untrusted').length,
    };
  });
  expect(status.text).toContain('1 supplied, 1 skipped');
  expect(status.text).toContain('optional-scan');
  expect(status.text).toContain('reviewer_skill_unavailable');
  expect(status.title).toContain('required-scan: supplied; native retrieval not verified; pinned input integrity required; not verified');
  expect(status.title).toContain('optional-scan: skipped');
  expect(status.label).toContain('optional-scan');
  expect(status.injectedElementCount).toBe(0);
});

test('native reviewer evidence requires matching content and artifact revision', async ({ page }) => {
  await open(page, '/?council=fixing');
  const observed = await page.evaluate(async () => {
    const { councilSummaryHtml } = await import('/src/council-panel.ts');
    const team: any = {
      sessionId: 'native-observed', status: 'completed', councilSince: 1,
      config: { council: { target: 9, maxRounds: 10, reviewers: 2 } },
      councilStatus: { state: 'passed', round: 1, lowest: 9.5, target: 9, stale: false },
      workers: [{
        id: 'observed', name: 'Reviewer', status: 'completed', createdAt: 2,
        council: { round: 1, lens: 'Integrity', score: 9.5 },
        reviewerBinding: {
          reviewerBotId: 'reviewer-bot', artifactRevision: 'cycle:1:2:observed',
          skillEvidence: { 'skill-a': {
            contentSha256: 'c'.repeat(64), artifactRevision: 'cycle:1:2:observed',
            invoked: true, checkExecuted: true, evidenceVerified: true,
            checkKind: 'pinned_input_integrity',
          } },
          snapshot: { skills: { pins: [{
            id: 'skill-a', contentSha256: 'c'.repeat(64), supplied: true, checkRequired: true,
            invoked: false, checkExecuted: false, evidenceVerified: false,
          }], skipped: [] } },
        },
      }],
    };
    const receipt = () => new DOMParser().parseFromString(councilSummaryHtml(team), 'text/html')
      .querySelector('.council-row-skills')?.getAttribute('title') ?? '';
    const valid = receipt();
    team.workers[0].reviewerBinding.skillEvidence['skill-a'].artifactRevision = 'unrelated';
    return { valid, stale: receipt() };
  });
  expect(observed.valid).toContain('native retrieval recorded; pinned input integrity verified');
  expect(observed.stale).toContain('native retrieval not verified; pinned input integrity required; not verified');
});

test('scoped reviewer action is explicitly granted only to one relative file', async ({ page }) => {
  await open(page, '/?bots=sample');
  await button(page).click();
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  await expect.poll(async () => eligibleReviewers(page).count()).toBeGreaterThanOrEqual(2);
  await eligibleReviewers(page).nth(0).check();
  await eligibleReviewers(page).nth(1).check();
  await panel(page).getByRole('checkbox', { name: /Allow one exact native host action/ }).check();
  await expect(panel(page).getByRole('textbox', { name: 'Allowed reviewer file path' })).toBeVisible();
  await panel(page).getByRole('textbox', { name: 'Allowed reviewer file path' }).fill('src/example.ts');
  await toggle(page).click();
  await expect.poll(async () => (await calls(page)).length).toBe(1);
  const config = (await calls(page))[0];
  expect(config.reviewPermissions?.authorizationId).toMatch(/^[a-f0-9-]{36}$/);
  expect(config.reviewPermissions.grants).toEqual([{tool:'pandacode_open_file', action:'call', resourceArgument:'path', resource:'src/example.ts'}]);
  await panel(page).getByRole('button', { name: 'Auto: the lead picks reviewers for the work' }).click();
  await expect.poll(async () => (await calls(page)).length).toBe(2);
  expect((await calls(page))[1].reviewPermissions).toBeUndefined();
});

test('selected reviewers can be chosen using keyboard controls', async ({ page }) => {
  await open(page, '/?bots=sample');
  await button(page).click();
  await option(page, 'reviewers', 'Auto: the lead picks reviewers for the work').focus();
  await page.keyboard.press('End');
  await expect(panel(page).getByRole('button', { name: 'Selected bots' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(panel(page).getByRole('button', { name: 'Selected bots' })).toHaveAttribute('aria-pressed', 'true');
  await expect.poll(async () => eligibleReviewers(page).count()).toBeGreaterThanOrEqual(2);
  const first = eligibleReviewers(page).nth(0);
  await first.focus();
  await page.keyboard.press('Space');
  await expect(first).toBeChecked();
  const second = eligibleReviewers(page).nth(1);
  await second.focus();
  await page.keyboard.press('Space');
  await expect(second).toBeChecked();
  await expect(panel(page).getByText('2 of 2–8 reviewers selected')).toBeVisible();
});

test('selected reviewer mode refuses to launch with fewer than two bots', async ({ page }) => {
  await open(page, '/?bots=sample');
  await button(page).click();
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  await expect.poll(async () => eligibleReviewers(page).count()).toBeGreaterThanOrEqual(2);
  await eligibleReviewers(page).first().check();
  await toggle(page).click();
  await expect(panel(page).getByRole('alert')).toContainText('Select 2 to 8 distinct reviewer bots');
  await expect(toggle(page)).not.toBeChecked();
  expect(await calls(page)).toEqual([]);
});

for (const state of ['paused', 'unavailable', 'missing'] as const) test(`a selected ${state} bot stays removable and cannot bypass reviewer validation`, async ({ page }) => {
  const staleId = 'old<reviewer>', valid = ['ready-one', 'ready-two'].map(id => ({ id, name: id, role: 'Reviewer', provider: 'codex', model: 'gpt-5', paused: false }));
  const stale = { id: staleId, name: 'Old reviewer', role: 'Reviewer', provider: 'codex', model: 'gpt-5', paused: state === 'paused', available: state !== 'unavailable' };
  await open(page, '/?council=fixing', {}, 0, '', {}, { ids: [staleId, ...valid.map(bot => bot.id)], bots: state === 'missing' ? valid : [stale, ...valid] });
  await button(page).click();
  const old = panel(page).getByRole('checkbox', { name: state === 'missing' ? /old<reviewer>/ : /Old reviewer/ });
  await expect(old).toBeChecked();
  await expect(old).toBeEnabled();
  if (state === 'missing') await expect(panel(page).getByText('Missing bot', { exact: true })).toBeVisible();
  await panel(page).getByRole('button', { name: 'Selected bots' }).click();
  await expect(panel(page).getByRole('alert')).toContainText('A selected reviewer bot is unavailable');
  expect(await calls(page)).toEqual([]);
  await old.focus();
  await page.keyboard.press('Space');
  await expect.poll(async () => (await calls(page)).length).toBe(1);
  expect((await calls(page))[0].reviewerBotIds).toEqual(valid.map(bot => bot.id));
  if (state === 'missing') await expect(old).toHaveCount(0);
  else { await expect(old).not.toBeChecked(); await expect(old).toBeDisabled(); }
  await expect(panel(page).getByRole('checkbox', { name: /ready-one/ })).toBeFocused();
  await expect(panel(page).getByText('2 of 2–8 reviewers selected', { exact: true })).toBeVisible();
});

test('while off, a choice only sets what council starts with', async ({ page }) => {
  await open(page);
  await button(page).click();
  await option(page, 'target', '8.5').click();
  await expect(option(page, 'target', '8.5')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('#team-trigger')).toHaveAccessibleName('Team: off');
  expect(await calls(page)).toEqual([]);
  await toggle(page).click();
  await expect.poll(() => calls(page)).toEqual([{ target: 8.5, maxRounds: 10, reviewers: 0 }]);
});

test('a native error shows in the panel and the control returns to the true state', async ({ page }) => {
  await open(page, '/?council', { architect_set_council: 'Council refused by native.' });
  await button(page).click();
  await expect(option(page, 'target', '9.5')).toHaveAttribute('aria-pressed', 'true');
  await option(page, 'target', '8.0').click();
  await expect(panel(page).getByRole('alert')).toHaveText('Council was not changed: Council refused by native.');
  await expect(option(page, 'target', '9.5')).toHaveAttribute('aria-pressed', 'true');
  await expect(option(page, 'target', '8.0')).toHaveAttribute('aria-pressed', 'false');
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
});

test('a refused team leaves the switch off and says why', async ({ page }) => {
  await open(page, '/', { architect_configure: 'Pause the task goal first.' });
  await button(page).click();
  await toggle(page).click();
  await expect(panel(page).getByRole('alert')).toHaveText('Council was not changed: Pause the task goal first.');
  await expect(toggle(page)).not.toBeChecked();
  expect(await calls(page)).toEqual([]);
});

test('Escape closes the panel and returns focus to the button', async ({ page }) => {
  await open(page);
  await button(page).focus();
  await page.keyboard.press('Enter');
  await expect(toggle(page)).toBeFocused();
  await expect(button(page)).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(panel(page)).toHaveCount(0);
  await expect(button(page)).toBeFocused();
  await expect(button(page)).toHaveAttribute('aria-expanded', 'false');
});

test('the open panel follows the thread, and a pending change stays with its own thread', async ({ page }) => {
  await open(page, '/', {}, 1500);
  await button(page).click();
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await expect(panel(page)).toHaveCount(0);
  await expect(button(page)).toHaveAccessibleName('Council: off');
  await button(page).click();
  await expect(toggle(page)).not.toBeChecked();
  await page.keyboard.press('Escape');
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await expect(button(page)).toHaveAccessibleName('Council: on, target 9.0', { timeout: 15_000 });
});

test('Tab wraps inside the panel', async ({ page }) => {
  await open(page);
  await button(page).click();
  await expect(toggle(page)).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  // The last control is the remembered "Turn on for new threads".
  await expect(panel(page).getByLabel('Start new threads with Council on')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(toggle(page)).toBeFocused();
});

test('the panel follows its button when the window resizes', async ({ page }) => {
  await open(page);
  await button(page).click();
  await expect(panel(page)).toBeVisible();
  await page.setViewportSize({ width: 900, height: 800 });
  await expect.poll(async () => {
    const [b, p] = await Promise.all([button(page).boundingBox(), panel(page).boundingBox()]);
    return Math.abs((b!.x + b!.width / 2) - Math.min(900 - 12 - p!.width / 2, Math.max(12 + p!.width / 2, p!.x + p!.width / 2))) < 2 && p!.x + p!.width <= 900 - 11 && p!.y + p!.height <= b!.y;
  }).toBe(true);
});

test('a refusal that lands after the panel closed is read aloud and shown on the next open', async ({ page }) => {
  await open(page, '/?council', { architect_set_council: 'Council refused by native.' });
  // Hold the native reply until the panel is actually closed. A fixed mock delay
  // can expire before Playwright presses Escape on a busy CI runner.
  await page.evaluate(() => {
    const w = window as any;
    w.__hold = w.__hold ?? {};
    w.__hold.architect_set_council = new Promise<void>(resolve => { w.__releaseCouncilSet = resolve; });
  });
  await button(page).click();
  await option(page, 'target', '8.0').click();
  await expect.poll(() => calls(page)).toHaveLength(1);
  await page.keyboard.press('Escape');
  await expect(panel(page)).toHaveCount(0);
  await page.evaluate(() => (window as any).__releaseCouncilSet());
  await expect(page.locator('#composer-live')).toHaveText('Council was not changed: Council refused by native.');
  await button(page).click();
  await expect(panel(page).getByRole('alert')).toHaveText('Council was not changed: Council refused by native.');
  await expect(option(page, 'target', '9.5')).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Escape');
  await button(page).click();
  await expect(panel(page).getByRole('alert')).toBeHidden();
});

test('an IME cancelling its composition keeps its Escape', async ({ page }) => {
  await open(page);
  await button(page).click();
  await expect(panel(page)).toBeVisible();
  const kept = await page.evaluate(() => {
    const event = new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true, cancelable: true });
    document.activeElement!.dispatchEvent(event);
    return !event.defaultPrevented;
  });
  expect(kept).toBe(true);
  await expect(panel(page)).toBeVisible();
});

test("one thread's slow change does not hold up another's, and the copy says when a team turns on", async ({ page }) => {
  await open(page, '/?council', {}, 4000, 'session-1');
  await button(page).click();
  await expect(panel(page).locator('#council-about')).not.toContainText('Starts a team');
  await option(page, 'target', '9.0').click();
  await page.keyboard.press('Escape');
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await button(page).click();
  await expect(panel(page).locator('#council-about')).toHaveText('Reviewers score the work until all reach your target. Starts a team; these become your defaults.');
  await toggle(page).click();
  // Session 1's call is still waiting out its 4 s; this one does not queue behind it.
  await expect(button(page)).toHaveAccessibleName('Council: on, target 9.0', { timeout: 3000 });
});

test('turning Council on with no team redraws the composer once the team starts', async ({ page }) => {
  await open(page);
  // A redraw replaces the composer form (the prompt node itself is kept, for typing focus).
  await page.locator('#composer').evaluate(form => { form.dataset.before = 'yes'; });
  await button(page).click();
  await toggle(page).click();
  await expect(page.locator('#team-trigger')).toHaveAccessibleName(/: on$/);
  await expect(page.locator('#composer')).not.toHaveAttribute('data-before', 'yes');
});

test('the note says what the settings do while Council is off', async ({ page }) => {
  await open(page);
  await button(page).click();
  // The header's description says it, after what Council does.
  await expect(panel(page).locator('#council-about')).toContainText('these become your defaults');
  await expect(panel(page).locator('[data-field="maxRounds"] button')).toHaveText(['3', '5', '10', '25']);
  await toggle(page).click();
  await expect(panel(page).locator('#council-about')).toContainText('Uses more of your plan');
});

test('a native value outside the segment shows as a selected extra option', async ({ page }) => {
  await open(page, '/?council=limit');
  await button(page).click();
  await expect(panel(page).getByText('Max rounds', { exact: true })).toBeVisible();
  const extra = panel(page).locator('[data-field="maxRounds"] [data-extra]');
  await expect(extra).toHaveText('2');
  await expect(extra).toHaveAttribute('aria-pressed', 'true');
  await expect(panel(page).locator('[data-field="maxRounds"] [aria-pressed="true"]')).toHaveCount(1);
  await option(page, 'maxRounds', '5').click();
  await expect(extra).toHaveCount(0);
});

test('targets run from 7 to 10', async ({ page }) => {
  await open(page);
  await button(page).click();
  await expect(panel(page).locator('[data-field="target"] button')).toHaveText(['7.0', '8.0', '8.5', '9.0', '9.5', '10.0']);
});

test('switching threads while the team starts says nothing went wrong', async ({ page }) => {
  await open(page, '/', {}, 0, '', { architect_profiles: 1500 });
  await button(page).click();
  await toggle(page).click();
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await page.waitForTimeout(2500);
  await expect(page.locator('#composer-live')).not.toContainText('could not start');
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await button(page).click();
  await expect(panel(page).getByRole('alert')).toBeHidden();
});

test('a canceled no-team enable clears busy and optimistic state and allows retry', async ({ page }) => {
  await open(page);
  // Hold setup before it can create a team, then cancel it by leaving the thread.
  await page.evaluate(() => {
    (window as any).__hold = { architect_profiles: new Promise<void>(resolve => { (window as any).__releaseProfiles = resolve; }) };
  });
  await button(page).click();
  await toggle(page).click();
  await expect(button(page)).toHaveAttribute('aria-busy', 'true');
  await expect(toggle(page)).toBeChecked();
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await expect(panel(page)).toHaveCount(0);
  await page.evaluate(async () => {
    (window as any).__releaseProfiles();
    await new Promise(resolve => setTimeout(resolve, 100));
  });
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await expect(button(page)).toHaveAttribute('aria-busy', 'false');
  await expect(button(page)).toHaveAccessibleName('Council: off');
  await button(page).click();
  await expect(toggle(page)).not.toBeChecked();
  await expect(panel(page).getByRole('alert')).toBeHidden();
  expect(await calls(page)).toEqual([]);
  await toggle(page).click();
  await expect(button(page)).toHaveAccessibleName('Council: on, target 9.0');
  await expect(button(page)).toHaveAttribute('aria-busy', 'false');
  await expect(toggle(page)).toBeChecked();
  expect(await calls(page)).toEqual([{ target: 9.0, maxRounds: 10, reviewers: 0 }]);
});

test('copy: Auto is the lead picking reviewers, and an open round says so far', async ({ page }) => {
  await open(page, '/?council');
  await button(page).click();
  await expect(panel(page).locator('#council-about')).toContainText('Uses more of your plan.');
  await expect(option(page, 'reviewers', 'Auto: the lead picks reviewers for the work')).toHaveAttribute('data-tip', 'Auto: the lead picks reviewers for the work');
  await expect(panel(page).locator('.council-lowest')).toHaveText(' · lowest so far 9.2');
});

test('"Turn on for new threads" starts Council with the first send of a new thread', async ({ page }) => {
  await open(page);
  await button(page).click();
  await panel(page).getByLabel('Start new threads with Council on').check();
  expect(JSON.parse(await page.evaluate(() => localStorage.getItem('pandacode.council') ?? '{}')).newThreads).toBe(true);
  await page.keyboard.press('Escape');
  await page.locator('.new-task-button').click();
  await composerSettled(page);
  await page.getByRole('textbox', { name: 'Message your coding agent' }).fill('Add a search box');
  await page.getByRole('textbox', { name: 'Message your coding agent' }).press('Enter');
  await expect.poll(() => calls(page)).toEqual([{ target: 9, maxRounds: 10, reviewers: 0 }]);
  await expect.poll(() => page.evaluate(() => (window as any).__councilStarts ?? [])).toEqual(['Add a search box']);
});

test('with Classic team setup, enabling Council on a thread with no team opens that setup', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('pandacode.teamClassic', 'true'));
  await open(page);
  await button(page).click();
  await toggle(page).click();
  await expect(page.locator('.architect-setup-popover')).toBeVisible();
  await expect(panel(page)).toHaveCount(0);
  expect(await calls(page)).toEqual([]);
});

test('panel hierarchy: one description, labels in text ink, a footer checkbox for new threads, and it fits 1280x820', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await open(page, '/?council');
  await button(page).click();
  const intro = await panel(page).locator('#council-about').textContent();
  // What Council does, then what it costs: two sentences, one paragraph.
  expect(intro!.trim().split(/[.!?](\s|$)/).filter(part => part && part.trim()).length).toBe(2);
  expect(await panel(page).locator('p:not(.council-error):not([hidden])').count()).toBe(1);
  const [label, ink, weight] = await panel(page).locator('#council-target').evaluate(node => [getComputedStyle(node).color, getComputedStyle(document.querySelector('.council-panel-head label')!).color, Number(getComputedStyle(node).fontWeight)]);
  expect(label).toBe(ink); expect(weight).toBeGreaterThanOrEqual(500);
  // Saver's footer pattern: a small checkbox row.
  await expect(panel(page).locator('div.council-foot').getByRole('checkbox', { name: 'Start new threads with Council on' })).toBeVisible();
  // Saver's width (its layout width; the opening scale is presentation).
  expect(await panel(page).evaluate(node => node.offsetWidth)).toBe(300);
  // The description is at most two lines, and "10.0" keeps clear of its segment's edges.
  expect(await panel(page).locator('#council-about').evaluate(node => Math.round(node.getBoundingClientRect().height / parseFloat(getComputedStyle(node).lineHeight)))).toBeLessThanOrEqual(2);
  expect(await option(page, 'target', '10.0').evaluate(node => { const range = document.createRange(); range.selectNodeContents(node); const text = range.getBoundingClientRect(), box = node.getBoundingClientRect(); return Math.min(text.left - box.left, box.right - text.right); })).toBeGreaterThanOrEqual(6);
  expect(await panel(page).evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
});

/** A new thread draws its composer twice (at once, then when its page loads); a draft typed in between is lost. Waits
 *  until the prompt stays the same element across two polls. */
const composerSettled = (page: Page) => page.waitForFunction(() => {
  const seen = window as any, now = document.querySelector('#prompt'), same = Boolean(now) && seen.__prompt === now;
  seen.__prompt = now; return same;
}, undefined, { polling: 300 });

async function newThreadsOn(page: Page): Promise<void> {
  await button(page).click();
  await panel(page).getByLabel('Start new threads with Council on').check();
  await page.keyboard.press('Escape');
  await page.locator('.new-task-button').click();
  await composerSettled(page);
}

test('with Classic team setup, a first send starts no automatic team for Council', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('pandacode.teamClassic', 'true'));
  await open(page);
  await newThreadsOn(page);
  await page.getByRole('textbox', { name: 'Message your coding agent' }).fill('Add a search box');
  await page.getByRole('textbox', { name: 'Message your coding agent' }).press('Enter');
  await page.waitForTimeout(800);
  expect(await calls(page)).toEqual([]);
  expect(await page.evaluate(() => (window as any).__councilStarts ?? [])).toEqual([]);
});

test('a second Enter while Council turns on for the first send sends nothing more', async ({ page }) => {
  await open(page);
  await newThreadsOn(page);
  // The team's start waits on a request the test holds, then releases: no timers.
  await page.evaluate(() => { (window as any).__hold = { architect_profiles: new Promise(done => { (window as any).__release = done; }) }; });
  const prompt = page.getByRole('textbox', { name: 'Message your coding agent' });
  await prompt.fill('Add a search box');
  await prompt.press('Enter');
  await prompt.press('Enter');
  await page.evaluate(() => (window as any).__release());
  await expect.poll(() => page.evaluate(() => (window as any).__councilStarts ?? [])).toEqual(['Add a search box']);
  expect(await calls(page)).toHaveLength(1);
});

test('a failed first-send enable is shown, and the message still goes', async ({ page }) => {
  // Every toast shown is recorded, so a toast that auto-hides or is replaced still counts: no timing.
  await page.addInitScript(() => {
    (window as any).__toasts = [];
    new MutationObserver(records => { for (const record of records) for (const node of record.addedNodes) if (node instanceof HTMLElement && node.matches('.toast, .toast *')) (window as any).__toasts.push(node.textContent); })
      .observe(document, { subtree: true, childList: true, characterData: true });
    // A reused toast changes its text in place: read it on any change too.
    new MutationObserver(() => { const text = document.querySelector('.toast')?.textContent; if (text) (window as any).__toasts.push(text); }).observe(document, { subtree: true, childList: true, characterData: true });
  });
  await open(page, '/', { architect_set_council: 'Council is unavailable.' });
  await newThreadsOn(page);
  await page.getByRole('textbox', { name: 'Message your coding agent' }).fill('Add a search box');
  await page.getByRole('textbox', { name: 'Message your coding agent' }).press('Enter');
  await expect.poll(() => page.evaluate(() => (window as any).__toasts.join('\n'))).toContain('Council did not turn on: Council is unavailable.');
  await expect.poll(() => page.evaluate(() => (window as any).__councilStarts ?? [])).toEqual(['Add a search box']);
});

const ready = (page: Page) => expect(page.locator('html')).toHaveAttribute('data-ready', '');

test('each segmented row is one Tab stop: arrows, Home and End move focus only, Enter or Space picks', async ({ page }) => {
  await open(page, '/?council');
  await button(page).click();
  for (const field of ['target', 'maxRounds', 'reviewers']) expect(await panel(page).locator(`[data-field="${field}"] button[tabindex="0"]`).count()).toBe(1);
  await option(page, 'target', '9.5').focus();
  await page.keyboard.press('ArrowRight');
  await expect(option(page, 'target', '10.0')).toBeFocused();
  await expect(option(page, 'target', '10.0')).toHaveAttribute('tabindex', '0');
  await page.keyboard.press('Home');
  await expect(option(page, 'target', '7.0')).toBeFocused();
  await page.keyboard.press('End');
  await expect(option(page, 'target', '10.0')).toBeFocused();
  expect(await calls(page)).toEqual([]);
  await expect(option(page, 'target', '9.5')).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Enter');
  await expect.poll(() => calls(page).then(list => list.at(-1)?.target)).toBe(10);
});

test('the panel places itself again when its content changes', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 640 });
  await open(page);
  await button(page).click();
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
  // Turning on a team redraws the composer asynchronously. Verify placement
  // after the native request and its replacement tray button have settled.
  await expect.poll(() => calls(page).then(list => list.length)).toBe(1);
  await expect(button(page)).toHaveAttribute('aria-busy', 'false');
  await expect.poll(async () => {
    const [p, b] = await Promise.all([panel(page).boundingBox(), button(page).boundingBox()]);
    return Boolean(p && b && p.y >= 0 && p.y + p.height <= b.y);
  }).toBe(true);
});

test('an open tooltip follows the composer button when its name changes', async ({ page }) => {
  await open(page, '/?council');
  await button(page).hover();
  await expect(page.locator('.tip-bubble.tip-visible')).toHaveText('Council: Round 2 of 10 · reviewing');
  await page.evaluate(() => (window as any).__councilSeat({ round: 2, lens: 'User experience', score: 8.6 }));
  // Every seat of round 2 is scored now, so the round is complete (the mock moves on like native).
  await expect(page.locator('.tip-bubble.tip-visible')).toHaveText('Council: Round 2 of 10 · fixing');
});

test('in forced colours a switch shows on and off', async ({ page }) => {
  await page.emulateMedia({ forcedColors: 'active' });
  await open(page, '/?council');
  await button(page).click();
  const on = await toggle(page).evaluate(node => getComputedStyle(node).backgroundColor);
  await toggle(page).click(); await expect(toggle(page)).not.toBeChecked();
  const off = await toggle(page).evaluate(node => getComputedStyle(node).backgroundColor);
  expect(on).not.toBe(off);
  if (process.env.COUNCIL_SHOTS) { await page.waitForTimeout(250); await panel(page).screenshot({ path: '/tmp/pandacode-council/forced-switches.png' }); }
});

/** Every Web Animation a Council element starts: its class, whether it was on screen in a visible window, and the
 *  animation itself, so its start time tells one play (a rebuilt node joining it) from a replay. */
const plays = (page: Page): Promise<{ starts: Record<string, number>; hidden: string[] }> => page.evaluate(async () => {
  await new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)));
  const log = (window as any).__log.splice(0) as { name: string; visible: boolean; animation: Animation }[], starts: Record<string, number[]> = {};
  for (const { name, animation } of log) (starts[name] ??= []).push(Number(animation.startTime ?? 0));
  // A node that joins a running animation starts within a frame or two of it; a replay starts later.
  const separate = (times: number[]) => times.sort((a, b) => a - b).filter((time, index) => index === 0 || time - times[index - 1] > 50).length;
  return { starts: Object.fromEntries(Object.entries(starts).map(([name, times]) => [name, separate(times)])), hidden: log.filter(entry => !entry.visible).map(entry => entry.name) };
});

test('Auto is explained, the off note is plain, and the button is busy while a switch is on its way', async ({ page }) => {
  await open(page, '/?council', {}, 1500);
  await button(page).click();
  await expect(option(page, 'reviewers', 'Auto: the lead picks reviewers for the work')).toHaveAttribute('aria-pressed', 'true');
  await expect(option(page, 'reviewers', 'Auto: the lead picks reviewers for the work')).toHaveAttribute('data-tip', 'Auto: the lead picks reviewers for the work');
  await option(page, 'reviewers', '3').click();
  await expect(button(page)).toHaveAttribute('aria-busy', 'true');
  await expect(button(page)).toHaveAttribute('aria-busy', 'false', { timeout: 5000 });
});

test('Tab and Shift+Tab stay inside the panel through every control, from any of them (WebKit too)', async ({ page }) => {
  await open(page, '/?council');
  await button(page).click();
  await expect(toggle(page)).toBeFocused();
  const where = () => page.evaluate(() => { const active = document.activeElement as HTMLElement; return active.closest('.council-panel') ? (active.classList.contains('council-switch') ? 'switch' : active.classList.contains('council-new-threads') ? 'new' : active.closest<HTMLElement>('[data-field]')!.dataset.field!) : `outside:${active.tagName}`; });
  const order = ['switch', 'target', 'maxRounds', 'reviewers', 'new'];
  const seen: string[] = [];
  for (let step = 0; step < order.length; step++) { seen.push(await where()); await page.keyboard.press('Tab'); }
  expect(seen).toEqual(order);
  expect(await where()).toBe('switch');
  // Record actual focus transitions instead of crossing the Playwright boundary after
  // each key. All transitions still have to follow the exact sequence, including both wraps.
  await page.evaluate(() => {
    (window as any).__councilFocusTrail = [] as string[];
    document.addEventListener('focusin', event => {
      const active = event.target as HTMLElement;
      const field = active.closest<HTMLElement>('[data-field]');
      const location = !active.closest('.council-panel') ? `outside:${active.tagName}`
        : active.classList.contains('council-switch') ? 'switch'
        : active.classList.contains('council-new-threads') ? 'new'
        : field?.dataset.field ?? 'unmapped';
      (window as any).__councilFocusTrail.push(location);
    }, true);
  });
  for (const start of order) {
    for (const key of ['Tab', 'Shift+Tab']) {
      // Start on that control, then traverse two full rounds using real key events.
      for (let step = 0; step < order.length && await where() !== start; step++) await page.keyboard.press('Tab');
      expect(await where()).toBe(start);
      await page.evaluate(() => { (window as any).__councilFocusTrail = []; });
      for (let step = 0; step < order.length * 2; step++) await page.keyboard.press(key);
      const visited: string[] = await page.evaluate(() => (window as any).__councilFocusTrail);
      const direction = key === 'Tab' ? 1 : -1;
      const expected = Array.from({ length: order.length * 2 }, (_, step) =>
        order[(order.indexOf(start) + direction * (step + 1) + order.length * 2) % order.length]);
      expect(visited).toEqual(expected);
      expect(await where()).toBe(start);
    }
  }
  expect(await panel(page).locator('.council-new-threads').getAttribute('tabindex')).toBe('0');
  expect(await toggle(page).getAttribute('tabindex')).toBe('0');
});

test('at 900×420 the composer panel fits above its button without scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 420 });
  await open(page, '/?council');
  await composerSettled(page);
  await button(page).click();
  await expect(panel(page)).toBeVisible();
  expect(await panel(page).evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
});

test('with Council on and nothing reviewed yet, the panel says so quietly', async ({ page }) => {
  await open(page, '/');
  await button(page).click();
  await toggle(page).click();
  await expect(toggle(page)).toBeChecked();
  await expect(panel(page).locator('.council-idle')).toHaveText('No changes to review yet');
  await toggle(page).click();
  await expect(panel(page).locator('.council-idle')).toBeHidden();
});

test('panel readability: the footer is meta size, and the panel is near-opaque over the card', async ({ page }) => {
  await open(page, '/?council=asking');
  await button(page).click();
  const [foot, meta] = await panel(page).locator('.council-new').evaluate(node => { const probe = document.createElement('span'); probe.style.fontSize = 'var(--ds-text-meta)'; node.append(probe); const sizes = [getComputedStyle(node).fontSize, getComputedStyle(probe).fontSize]; probe.remove(); return sizes; });
  expect(foot).toBe(meta);
  const alpha = await panel(page).evaluate(node => { const n = getComputedStyle(node).backgroundColor.match(/[\d.]+/g)!.map(Number); return n.length > 3 ? n[3] : 1; });
  expect(alpha).toBeGreaterThanOrEqual(0.9);
});

test('turning Council on and a refusal are read aloud, even with the panel open', async ({ page }) => {
  await open(page);
  await button(page).click(); await toggle(page).click();
  await expect(page.locator('#composer-live')).toContainText('Council is on. Target 9.0.');
  await open(page, '/?council', { architect_set_council: 'Council refused by native.' });
  await button(page).click();
  await option(page, 'target', '8.0').click();
  await expect(panel(page).locator('.council-error')).toBeVisible();
  await expect(page.locator('#composer-live')).toContainText('Council was not changed: Council refused by native.');
});

for (const [kind, state, score, extra] of [
  ['', 'Round 2 of 10 · reviewing', '9.2 / 9.5 so far', ''],
  ['fixing', 'Round 1 of 10 · fixing', '8.3 / 9.5', ''],
  ['asking', 'Needs you', '8.3 / 9.5', 'actions'],
  ['passed', 'Passed at 9.5', '9.5 / 9.5', 'dismiss'],
  ['accepted', 'Accepted at 8.3', '8.3 / 9.5', 'dismiss'],
  ['limit', 'Stopped at round limit (2 of 2)', '8.7 / 9.5', 'raise'],
  ['finished', 'Finished below target', '8.3 / 9.5', 'hint'],
] as const) {
  test(`the strip in the prompt box reads ${kind || 'reviewing'}`, async ({ page }) => {
    await open(page, `/?council=${kind}`);
    await expect(strip(page)).toHaveAccessibleName('Council');
    await expect(strip(page).locator('.council-strip-state')).toHaveText(state);
    await expect(strip(page).locator('.council-strip-score')).toHaveText(score);
    // Inside the prompt box, above the input.
    expect(await strip(page).evaluate(node => Boolean(node.closest('#composer')) && Boolean(node.compareDocumentPosition(document.querySelector('#prompt')!) & Node.DOCUMENT_POSITION_FOLLOWING))).toBe(true);
    await expect(strip(page).getByRole('button', { name: 'Keep fixing' })).toHaveCount(extra === 'actions' ? 1 : 0);
    await expect(strip(page).getByRole('button', { name: 'Raise max rounds' })).toHaveCount(extra === 'raise' ? 1 : 0);
    await expect(strip(page).getByRole('button', { name: 'Dismiss Council result' })).toHaveCount(extra === 'dismiss' || extra === 'raise' || extra === 'hint' ? 1 : 0);
    if (extra === 'hint') await expect(strip(page).locator('.council-strip-hint')).toHaveText('Send a message to continue, or lower the target.');
    // Collapsed by default; never a card in the thread.
    await expect(disclosure(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(page.locator('.council-card')).toHaveCount(0);
  });
}

test('no strip while Council is off, or on with no round yet', async ({ page }) => {
  await open(page, '/');
  await expect(strip(page)).toHaveCount(0);
  await button(page).click(); await toggle(page).click();
  await expect(button(page)).toHaveAttribute('data-on', '');
  await expect(strip(page)).toHaveCount(0);
});

test('a stopped team: the strip says how to continue', async ({ page }) => {
  await open(page, '/?council');
  await page.locator('#agents-toggle').click();
  await page.locator('.architect-panel').getByRole('button', { name: 'Stop team' }).click();
  await page.keyboard.press('Escape');
  await expect(strip(page).locator('.council-strip-state')).toHaveText(/^Stopped · round 2 of 10/);
  await expect(strip(page).locator('.council-strip-hint')).toHaveText('Send a message to continue.');
});

test('expand and collapse: the reviewers of the latest scored round, remembered per thread', async ({ page }) => {
  await open(page, '/?council=fixing');
  const rows = strip(page).locator('.council-row');
  await expect(rows).toHaveCount(0);
  await disclosure(page).click();
  await expect(disclosure(page)).toHaveAttribute('aria-expanded', 'true');
  await expect(rows).toHaveCount(3);
  await expect(rows.first()).toHaveAccessibleName('Correctness: 8.3, below target. Open reviewer');
  await expect(rows.locator('.council-row-score')).toHaveText(['8.3', '9.1', '8.8']);
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await expect(strip(page)).toHaveCount(0);
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await expect(rows).toHaveCount(3);
  await disclosure(page).click();
  await expect(rows).toHaveCount(0);
});

test('a running round shows a pulse; a settled cycle reads first → final', async ({ page }) => {
  await open(page, '/?council');
  await disclosure(page).click();
  await expect(strip(page).locator('.council-row').nth(2).locator('.council-pulse')).toHaveCount(1);
  await open(page, '/?council=passed');
  await disclosure(page).click();
  await expect(strip(page).locator('.council-row').first().locator('.council-first')).toHaveText('8.3 → ');
});

test('clicking a reviewer row opens that reviewer, like a worker row', async ({ page }) => {
  await open(page, '/?council=fixing');
  await disclosure(page).click();
  await strip(page).locator('.council-row').filter({ hasText: 'Security' }).click();
  await expect(page.locator('.architect-worker-meta strong')).toHaveText('Security');
  await expect(page.locator('.architect-panel').getByRole('button', { name: 'Back to workers', exact: true })).toBeFocused();
});

test('asking: Keep fixing sends once, the strip reads fixing at once, and focus lands on the strip', async ({ page }) => {
  await open(page, '/?council=asking');
  await strip(page).getByRole('button', { name: 'Keep fixing' }).click();
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Round 1 of 10 · fixing');
  await expect(strip(page).getByRole('button', { name: 'Keep fixing' })).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => (window as any).__councilStarts ?? [])).toEqual(['Keep fixing until every reviewer reaches the target.']);
  await expect(disclosure(page)).toBeFocused();
});

test('asking: Accept and finish, then the settled strip can be dismissed until the next cycle', async ({ page }) => {
  await open(page, '/?council=asking');
  await strip(page).getByRole('button', { name: 'Accept and finish' }).click();
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Accepted at 8.3');
  await expect(page.locator('#composer-live')).toContainText('Council: Accepted at 8.3');
  await strip(page).getByRole('button', { name: 'Dismiss Council result' }).click();
  await expect(strip(page)).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeFocused();
  // Still dismissed after the composer draws again (another thread and back).
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await expect(button(page)).toBeVisible();
  await expect(strip(page)).toHaveCount(0);
});

test('an action that fails: all three busy while it runs, then its failure in the strip and focus back on it', async ({ page }) => {
  await open(page, '/?council=asking', { architect_council_accept: 'Native refused.' });
  // Hold the native answer until the intermediate busy state is asserted.
  // A fixed 800ms mock delay raced against slow CI browser instrumentation.
  await page.evaluate(() => {
    const w = window as any;
    w.__hold = w.__hold ?? {};
    w.__hold.architect_council_accept = new Promise<void>(resolve => { w.__releaseCouncilAccept = resolve; });
  });
  const accept = strip(page).getByRole('button', { name: 'Accept and finish' });
  await accept.focus(); await page.keyboard.press('Enter');
  for (const name of ['Keep fixing', 'Lower target', 'Accept and finish']) await expect(strip(page).getByRole('button', { name })).toHaveAttribute('aria-disabled', 'true');
  await page.evaluate(() => (window as any).__releaseCouncilAccept());
  await expect(strip(page).getByRole('alert')).toHaveText('Accept and finish did not go through: Native refused.');
  await expect(accept).toBeFocused();
  await expect(strip(page).getByRole('button', { name: 'Keep fixing' })).not.toHaveAttribute('aria-disabled', 'true');
});

test('Lower target and Raise max rounds open the compact panel from the strip', async ({ page }) => {
  await open(page, '/?council=asking');
  const lower = strip(page).getByRole('button', { name: 'Lower target' });
  await lower.click();
  await expect(panel(page)).toHaveAccessibleName('Lower target');
  await expect(lower).toHaveAttribute('aria-expanded', 'true');
  await expect(panel(page).locator('[data-field="target"] > button:visible')).toHaveText(['7.0', '8.0', '8.5', '9.0']);
  // Lowering to a value still above the lowest keeps focus in the panel.
  await expect(option(page, 'target', '9.0')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(() => calls(page).then(list => list.at(-1)?.target)).toBe(9);
  await expect(option(page, 'target', '8.5')).toBeFocused();
  // Lowering to the lowest passes: the control goes, the panel closes and focus goes to the prompt.
  await page.keyboard.press('ArrowLeft'); await page.keyboard.press('ArrowLeft'); await page.keyboard.press('Enter');
  await expect(strip(page).locator('.council-strip-state')).toHaveText(/^Passed at 8.3/);
  await expect(panel(page)).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeFocused();
  await open(page, '/?council=limit');
  await strip(page).getByRole('button', { name: 'Raise max rounds' }).click();
  await expect(panel(page)).toHaveAccessibleName('Raise max rounds');
  await expect(panel(page).locator('#council-maxRounds')).toHaveText('2 of 2 rounds used');
  await expect(option(page, 'maxRounds', '10')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Round 2 of 10 · fixing');
});

test('at the minimum target there is no empty Lower target picker and acceptance keeps focus', async ({ page }) => {
  await page.route('**/src/preview-architect.ts*', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, body: (await response.text()).replace('target: 9.5, maxRounds:', 'target: 7, maxRounds:') });
  });
  await open(page, '/?council=asking');
  await composerSettled(page);
  await expect(strip(page).getByRole('button', { name: 'Lower target' })).toHaveCount(0);
  await expect(strip(page).getByRole('button', { name: 'Keep fixing' })).toBeEnabled();
  await strip(page).getByRole('button', { name: 'Accept and finish' }).click();
  await expect(disclosure(page)).toBeFocused();
  await expect(panel(page)).toHaveCount(0);
});

for (const used of [25, 30]) test(`at ${used} used rounds there is no empty Raise max rounds picker`, async ({ page }) => {
  await open(page, '/?council=limit');
  await composerSettled(page);
  await page.evaluate(used => (window as any).__councilSeat({ round: 2, lens: 'Correctness', score: 8.7, council: { state: 'limit', round: used } }), used);
  await expect(strip(page).getByRole('button', { name: 'Raise max rounds' })).toHaveCount(0);
  await disclosure(page).focus();
  await page.keyboard.press('Enter');
  await expect(disclosure(page)).toBeFocused();
  await expect(panel(page)).toHaveCount(0);
});

test('the tray button is settings only: no score badge, a cue dot when asking or settled', async ({ page }) => {
  await open(page, '/?council');
  await expect(button(page).locator('b')).toHaveCount(0);
  await expect(button(page)).toHaveAccessibleName('Council: Round 2 of 10 · reviewing');
  const dot = () => button(page).locator('.council-glyph').evaluate(node => getComputedStyle(node, '::after').content !== 'none');
  await expect.poll(dot).toBe(false);
  await open(page, '/?council=asking');
  await expect(button(page)).toHaveAccessibleName('Council needs you');
  await expect.poll(dot).toBe(true);
  await open(page, '/?council=passed');
  await expect.poll(dot).toBe(true);
  await open(page, '/?council=accepted');
  await expect(button(page)).toHaveAccessibleName(/^Council: Accepted/);
  await expect.poll(dot).toBe(false);
});

test('in the thread, reviewers are ordinary worker rows with their verdict as status', async ({ page }) => {
  await open(page, '/?council=fixing');
  await page.locator('.team-progress-toggle').first().click();
  const row = page.locator('.team-progress-worker').filter({ hasText: 'Correctness' });
  await expect(row.locator('.team-progress-status')).toHaveText('Scored 8.3 · below 9.5');
  await expect(row).toHaveAttribute('data-status', 'score-below');
});

test('the Agents flyout shows the state line and reviewer rows, no actions', async ({ page }) => {
  await open(page, '/?council=asking');
  await page.locator('#agents-toggle').click();
  const summary = page.locator('.architect-panel .council-summary');
  await expect(summary.locator('.council-summary-state')).toContainText('Needs you');
  await expect(summary.locator('.council-row')).toHaveCount(3);
  await expect(summary.getByRole('button', { name: /Keep fixing|Accept|Lower target/ })).toHaveCount(0);
  await summary.locator('.council-row').first().click();
  await expect(page.locator('.architect-worker-meta strong')).toHaveText('Correctness');
});

/** Every Web Animation on a Council element (the strip, the tray button), by class. */
async function recordMotion(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    (window as any).__motion = [];
    Element.prototype.animate = function (this: Element, ...args: Parameters<Element['animate']>) {
      if (this.closest('.council-strip, .composer-council-button')) (window as any).__motion.push({ name: this.getAttribute('class') ?? this.tagName, frames: JSON.stringify(args[0]) });
      return animate.apply(this, args);
    };
  });
}
const motion = (page: Page): Promise<{ name: string; frames: string }[]> => page.evaluate(() => (window as any).__motion.splice(0));

for (const known of [false, true]) test(`motion: ${known ? 'a known empty Council enters' : 'an unknown Council hydrates quietly'} after a delayed first strip paint`, async ({ page }) => {
  await recordMotion(page);
  // Isolate the model/paint gap: a slow initial load and a real later round must have different baselines.
  await page.route('**/__council-motion', route => route.fulfill({ contentType: 'text/html', body: '<html data-ready><body><div id="composer"></div></body></html>' }));
  await page.goto('/__council-motion');
  await page.evaluate(async known => {
    const { createCouncil } = await import('/src/council-panel.ts');
    let team: any = null, loaded = known;
    const council = createCouncil({
      team: () => team, loaded: () => loaded, call: async () => null as any,
      startTeam: async () => false, updated: () => {}, keepFixing: async () => {},
      classicSetup: () => false, openClassicSetup: () => {}, announce: () => {},
    });
    document.querySelector('#composer')!.innerHTML = council.button('delayed') + council.strip('delayed');
    council.sync('delayed');
    (window as any).__hydrateCouncil = () => {
      loaded = true;
      team = {
        sessionId: 'delayed', status: 'running', config: { council: { target: 9.5, maxRounds: 10, reviewers: 0 } },
        councilStatus: { state: 'reviewing', round: 1, lowest: null, target: 9.5, stale: false },
        workers: [{ id: 'seat', name: 'Correctness', status: 'running', createdAt: 1, council: { round: 1, lens: 'Correctness', score: null } }],
      };
      council.button('delayed'); // The snapshot is known before the strip's deferred paint.
    };
    (window as any).__paintCouncil = () => council.sync('delayed');
    (window as any).__changeCouncil = () => { team.councilStatus.state = 'fixing'; council.sync('delayed'); };
  }, known);
  await page.waitForTimeout(700);
  await page.evaluate(() => (window as any).__hydrateCouncil());
  await page.waitForTimeout(700);
  await page.evaluate(() => (window as any).__paintCouncil());
  await expect(strip(page)).toBeVisible();
  const initial = await motion(page);
  if (known) expect(initial.filter(entry => entry.name === 'council-strip')).toHaveLength(1);
  else expect(initial).toEqual([]);
  await page.waitForTimeout(250);
  await page.evaluate(() => (window as any).__changeCouncil());
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Round 1 of 10 · fixing');
  expect((await motion(page)).some(entry => entry.name === 'council-strip-state')).toBe(true);
});

test('motion: nothing on load or thread switch; the strip enters with a short fade and rise; the state cross-fades; a score lands; the glyph pops for asking', async ({ page }) => {
  await recordMotion(page);
  await open(page, '/?council=fixing');
  await settle(page);
  expect(await motion(page)).toEqual([]);
  await page.locator('.sidebar').getByText('Refine the navigation').click();
  await page.locator('.sidebar').getByText('Make search feel instant').click();
  await expect(strip(page)).toBeVisible(); await page.waitForTimeout(700);
  expect(await motion(page)).toEqual([]);
  await disclosure(page).click();
  await page.evaluate(() => (window as any).__councilSeat({ round: 2, lens: 'Correctness', score: null }));
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Round 2 of 10 · reviewing');
  let played = await motion(page);
  expect(played.map(entry => entry.name)).toContain('council-strip-state');
  expect(JSON.parse(played.find(entry => entry.name === 'council-strip-state')!.frames)[0].opacity).toBe(0.4);
  await page.evaluate(() => (window as any).__councilSeat({ round: 2, lens: 'Correctness', score: 9.0, council: { state: 'fixing', round: 2, lowest: 9.0 } }));
  await expect(strip(page).locator('.council-row-score').first()).toHaveText('9.0');
  played = await motion(page);
  expect(played.some(entry => entry.name === 'council-row-score' && entry.frames.includes('scale'))).toBe(true);
  await page.evaluate(() => (window as any).__councilAsk());
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Needs you');
  await expect.poll(async () => (await page.evaluate(() => (window as any).__motion as { name: string; frames: string }[])).some(entry => entry.name === 'council-glyph' && entry.frames.includes('scale'))).toBe(true);
});

test('motion: the strip enters when the first round starts, and never animates the composer\'s height', async ({ page }) => {
  await recordMotion(page);
  // A settled, dismissed strip comes back when the next round starts: it enters then.
  await open(page, '/?council=fixing');
  await settle(page);
  await page.evaluate(() => (window as any).__councilAsk());
  await strip(page).getByRole('button', { name: 'Accept and finish' }).click();
  await strip(page).getByRole('button', { name: 'Dismiss Council result' }).click();
  await expect(strip(page)).toHaveCount(0);
  await motion(page);
  await page.evaluate(() => (window as any).__councilSeat({ round: 2, lens: 'Correctness', score: null, council: { state: 'reviewing', round: 2, lowest: null } }));
  await expect(strip(page)).toBeVisible();
  await expect.poll(async () => (await page.evaluate(() => (window as any).__motion as { name: string; frames: string }[])).filter(entry => entry.name === 'council-strip')).toHaveLength(1);
  const [enter] = (await motion(page)).filter(entry => entry.name === 'council-strip');
  const frames = JSON.parse(enter.frames);
  expect(frames[0].opacity).toBe(0); expect(frames[0].transform).toBe('translateY(4px)');
  expect(enter.frames).not.toMatch(/height/);
});

test('reduced motion: nothing animates, and every state still lands', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await recordMotion(page);
  await open(page, '/?council=fixing');
  await settle(page);
  await disclosure(page).click();
  await page.evaluate(() => (window as any).__councilSeat({ round: 2, lens: 'Correctness', score: 9.0, council: { state: 'fixing', round: 2, lowest: 9.0 } }));
  await page.evaluate(() => (window as any).__councilAsk());
  await expect(strip(page).locator('.council-strip-state')).toHaveText('Needs you');
  expect(await motion(page)).toEqual([]);
});

test('keyboard: the strip comes before the input, and its controls in reading order', async ({ page, browserName }) => {
  await open(page, '/?council=asking');
  await expect(strip(page).getByRole('button', { name: 'Keep fixing' })).toBeVisible();
  // In document order: the disclosure, the three actions, then the input.
  expect(await page.evaluate(() => [...document.querySelectorAll('#composer .council-strip button, #prompt')].map(node => node.id === 'prompt' ? 'prompt' : node.classList.contains('council-strip-toggle') ? 'toggle' : node.textContent!.trim()))).toEqual(['toggle', 'Keep fixing', 'Lower target', 'Accept and finish', 'prompt']);
  // WebKit's Tab skips buttons unless Full Keyboard Access is on; the order above is what it follows when it is.
  if (browserName === 'webkit') return;
  await disclosure(page).focus();
  const order: string[] = [];
  for (let step = 0; step < 4; step++) { order.push(await page.evaluate(() => (document.activeElement as HTMLElement).textContent?.trim() || document.activeElement!.getAttribute('aria-label') || '')); await page.keyboard.press('Tab'); }
  expect(order.slice(1)).toEqual(['Keep fixing', 'Lower target', 'Accept and finish']);
  await expect(page.getByRole('textbox', { name: 'Message your coding agent' })).toBeFocused();
});

test('asking is read aloud once, through the composer live region', async ({ page }) => {
  await open(page, '/?council=fixing');
  await settle(page);
  await page.evaluate(() => (window as any).__councilAsk());
  await expect(page.locator('#composer-live')).toHaveText(/^Council: Needs you\. /);
});

for (const width of [1280, 560]) {
  test(`the strip fits the composer at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 860 });
    await open(page, '/?council=asking');
    await expect(strip(page)).toBeVisible();
    const fits = await page.evaluate(() => {
      const box = document.querySelector('#composer')!.getBoundingClientRect();
      return [...document.querySelectorAll('#composer .council-strip *')].every(node => { const rect = node.getBoundingClientRect(); return !rect.width || (rect.left >= box.left - 0.5 && rect.right <= box.right + 0.5); });
    });
    expect(fits).toBe(true);
    if (!process.env.COUNCIL_SHOTS) return;
    for (const theme of ['dark', 'light'] as const) for (const kind of ['', 'asking', 'passed', 'accepted', 'limit', 'finished']) {
      await page.addInitScript(mode => localStorage.setItem('pandacode.theme', mode), theme);
      await open(page, `/?council=${kind}`);
      await page.waitForTimeout(400);
      await page.locator('#composer-area').screenshot({ path: `/tmp/pandacode-council/strip-${theme}-${width}-${kind || 'reviewing'}.png` });
      if (kind === 'passed') { await disclosure(page).click(); await page.waitForTimeout(200); await page.locator('#composer-area').screenshot({ path: `/tmp/pandacode-council/strip-${theme}-${width}-expanded.png` }); }
    }
  });
}
});
