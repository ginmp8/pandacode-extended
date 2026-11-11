import { expect, test } from '@playwright/test';

test.describe('bot collaboration', () => {
  test('channel Council controls stay compact and aligned in desktop and narrow layouts', async ({ page }) => {
    await page.setViewportSize({ width: 1060, height: 780 });
    await page.goto('/tests/fixtures/bot-manifests.html');
    await page.addStyleTag({ url: '/src/styles.css' });
    await page.addStyleTag({ url: '/src/bot-shared-channels.css' });
    await page.evaluate(async () => {
      const { sharedChannelComposerHtml } = await import('/src/bot-shared-channels.ts');
      document.body.innerHTML = sharedChannelComposerHtml({
        id: 'lab-security', name: 'lab-security', memberIds: ['executor', 'reviewer'],
        archived: false, messages: [], activeBotIds: [],
      } as any, [
        { id: 'executor', name: 'Executor' },
        { id: 'reviewer', name: 'Reviewer' },
      ] as any);
      document.querySelector<HTMLFormElement>('#shared-channel-composer')!.style.cssText = 'max-width:830px;margin:24px auto';
    });
    const form = page.locator('#shared-channel-composer');
    const toggle = form.locator('input[name=councilEnabled]');
    await expect(toggle).toBeVisible();
    const dims = await toggle.evaluate(element => {
      const rect = element.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    });
    expect(dims.width).toBeGreaterThanOrEqual(30);
    expect(dims.width).toBeLessThanOrEqual(40);
    expect(dims.height).toBeLessThanOrEqual(23);
    await toggle.check();
    const options = form.locator('.bc-council-options');
    await expect(options).toBeVisible();
    for (const name of ['councilGithubIssue', 'councilMode']) {
      const control = form.locator(name === 'councilMode' ? 'input[name=councilMode][value=selected]' : `input[name=${name}]`);
      const width = await control.evaluate(element => element.getBoundingClientRect().width);
      expect(width, name).toBeLessThanOrEqual(20);
    }
    const group = form.locator('.bc-council-modes');
    const layout = await group.evaluate(element => {
      const labels = [...element.querySelectorAll('label')];
      const a = labels[0].getBoundingClientRect(), b = labels[1].getBoundingClientRect();
      const parent = element.getBoundingClientRect();
      return { sameRow: Math.abs(a.top - b.top) < 2, contained: a.left >= parent.left && b.right <= parent.right };
    });
    expect(layout.sameRow).toBe(true);
    expect(layout.contained).toBe(true);
    await form.locator('input[name=councilMode][value=selected]').check();
    await expect(form.locator('.bc-council-reviewer-options')).toBeVisible();
    await expect(form.locator('.bc-council-selected-permissions')).toBeVisible();
    await page.setViewportSize({ width: 480, height: 640 });
    const overflow = await form.evaluate(element => {
      const root = element.getBoundingClientRect(), panel = element.querySelector('.bc-council-options')!.getBoundingClientRect();
      return { root: root.width, panel: panel.width, right: panel.right, viewport: window.innerWidth, height: panel.height, footer: element.querySelector('.send-button')!.getBoundingClientRect().bottom };
    });
    expect(overflow.panel).toBeLessThanOrEqual(overflow.root);
    expect(overflow.right).toBeLessThanOrEqual(overflow.viewport);
    expect(overflow.height).toBeLessThanOrEqual(340);
    expect(overflow.footer).toBeLessThanOrEqual(640);
    await expect(form.locator('.send-button')).toBeVisible();
  });

  test('Council opt-in is isolated to one message and statuses are per-delivery', async ({ page }) => {
    await page.goto('/tests/fixtures/bot-manifests.html');
    await page.evaluate(async () => {
      const { sharedChannelThreadHtml, sharedChannelComposerHtml } = await import('/src/bot-shared-channels.ts');
      const bots = [
        { id: 'executor', name: 'Executor', avatar: 'panda:0', color: '' },
        { id: 'reviewer', name: 'Reviewer', avatar: 'panda:1', color: '' },
      ] as any;
      const channel = {
        id: 'quality', name: 'quality', memberIds: ['executor', 'reviewer'],
        leadBotId: 'executor', archived: false, updatedAt: 1, activeBotIds: [],
        messages: [
          { id: 'one', kind: 'assigned', authorBotId: 'executor', text: 'Assigned', status: 'In progress', createdAt: 1,
            council: { cycleId: 'isolated-one', sessionId: 'council-task-one', state: 'Reviewing', round: 1, target: 9, lowest: null,
              artifactRevision: '1:2', reviewers: [{ botId: 'reviewer', name: 'Reviewer', score: null }] } },
          { id: 'two', kind: 'assigned', authorBotId: 'reviewer', text: 'Assigned', status: 'Completed', createdAt: 2 },
          { id: 'exception', kind: 'assigned', authorBotId: 'executor', text: 'Accepted', status: 'Completed', createdAt: 3,
            council: { cycleId: 'accepted-one', sessionId: 'accepted-session', state: 'Accepted by user', round: 1, target: 9,
              lowest: 7.5, artifactRevision: '1:3', reviewers: [], acceptedBy: 'local_user', acceptanceReason: 'Accepted documented limitation',
              followupStatus: 'succeeded', followupUrl: 'https://github.com/owner/repo/issues/42' } },
          { id: 'limit', kind: 'assigned', authorBotId: 'executor', text: 'At limit', status: 'In progress', createdAt: 4,
            council: { cycleId: 'limit-one', sessionId: 'limit-session', state: 'Limit reached', round: 1, target: 9, lowest: 7,
              artifactRevision: '1:4', reviewers: [] } },
        ],
      } as any;
      const root = document.createElement('div');
      root.id = 'channel-council-fixture';
      root.innerHTML = sharedChannelThreadHtml(channel, bots) + sharedChannelComposerHtml(channel, bots);
      document.body.append(root);
    });
    const fixture = page.locator('#channel-council-fixture');
    await expect(fixture.locator('input[name="councilEnabled"]')).not.toBeChecked();
    await expect(fixture.locator('.bc-assigned[data-message-id="one"]')).toContainText('Council: Reviewing');
    await expect(fixture.locator('.bc-assigned[data-message-id="one"]')).toContainText('Reviewer');
    await expect(fixture.locator('.bc-assigned[data-message-id="one"] [data-action="channel-council-report"]')).toHaveAttribute('data-session-id', 'council-task-one');
    await expect(fixture.locator('.bc-assigned[data-message-id="two"]')).not.toContainText('Council:');
    await expect(fixture.locator('.bc-assigned[data-message-id="exception"]')).toContainText('Accepted by local user: Accepted documented limitation');
    await expect(fixture.locator('.bc-assigned[data-message-id="exception"] a[href="https://github.com/owner/repo/issues/42"]')).toHaveCount(1);
    await expect(fixture.locator('.bc-assigned[data-message-id="exception"] [data-action="channel-council-accept"]')).toHaveCount(0);
    await expect(fixture.locator('.bc-assigned[data-message-id="limit"] [data-action="channel-council-accept"]')).toHaveAttribute('data-delivery-id', 'limit');
    await fixture.locator('input[name="councilEnabled"]').check();
    await expect(fixture.locator('input[name="councilEnabled"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilTarget"]')).toHaveValue('9');
    await expect(fixture.locator('input[name="councilMaxRounds"]')).toHaveValue('10');
    await expect(fixture.locator('input[name="councilReviewer"]')).toHaveCount(2);
    await fixture.locator('input[name="councilMode"][value="selected"]').check();
    await expect(fixture.locator('input[name="councilScopedFile"]')).toHaveCount(1);
    await expect(fixture.locator('input[name="councilFilePath"]')).toHaveCount(1);
    await expect(fixture.locator('input[name="councilGithubIssue"]')).toHaveCount(1);
    await expect(fixture.locator('input[name="councilGithubRepo"]')).toHaveCount(1);

    await fixture.locator('input[name="councilTarget"]').fill('8.5');
    await fixture.locator('input[name="councilMaxRounds"]').fill('4');
    await fixture.locator('input[name="councilReviewer"][value="reviewer"]').check();
    await fixture.locator('input[name="councilScopedFile"]').check();
    await fixture.locator('input[name="councilFilePath"]').fill('src/entry.ts');
    await fixture.locator('input[name="councilGithubIssue"]').check();
    await fixture.locator('input[name="councilGithubRepo"]').fill('owner/repo');
    await page.evaluate(async () => {
      const { sharedChannelComposerHtml, restoreSharedChannelCouncilInputs } = await import('/src/bot-shared-channels.ts');
      const parent = document.getElementById('channel-council-fixture')!;
      const old = parent.querySelector<HTMLFormElement>('form')!;
      const render = (id: string) => {
        const holder = document.createElement('div');
        holder.innerHTML = sharedChannelComposerHtml({
          id, name: id, memberIds: ['executor', 'reviewer'], archived: false,
        } as any, [{ id: 'executor', name: 'Executor' }, { id: 'reviewer', name: 'Reviewer' }] as any);
        return holder.querySelector<HTMLFormElement>('form')!;
      };
      const refreshed = render('quality');
      restoreSharedChannelCouncilInputs(old, refreshed);
      old.replaceWith(refreshed);
    });
    await expect(fixture.locator('input[name="councilEnabled"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilMode"][value="selected"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilTarget"]')).toHaveValue('8.5');
    await expect(fixture.locator('input[name="councilMaxRounds"]')).toHaveValue('4');
    await expect(fixture.locator('input[name="councilReviewer"][value="reviewer"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilScopedFile"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilFilePath"]')).toHaveValue('src/entry.ts');
    await expect(fixture.locator('input[name="councilGithubIssue"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilGithubRepo"]')).toHaveValue('owner/repo');
    await page.evaluate(async () => {
      const { sharedChannelComposerHtml, restoreSharedChannelCouncilInputs } = await import('/src/bot-shared-channels.ts');
      const old = document.querySelector<HTMLFormElement>('#channel-council-fixture form')!;
      const holder = document.createElement('div');
      holder.innerHTML = sharedChannelComposerHtml({
        id: 'other', name: 'other', memberIds: ['executor','reviewer'], archived: false,
      } as any, [{ id: 'executor', name: 'Executor' }, { id: 'reviewer', name: 'Reviewer' }] as any);
      const next = holder.querySelector<HTMLFormElement>('form')!;
      restoreSharedChannelCouncilInputs(old, next);
      old.replaceWith(next);
    });
    await expect(fixture.locator('input[name="councilEnabled"]')).not.toBeChecked();
    await expect(fixture.locator('input[name="councilGithubIssue"]')).not.toBeChecked();
    await expect(fixture.locator('input[name="councilGithubRepo"]')).toHaveValue('');
  });

  test('renders attributed bot posts, explicit recipients, and direct delegation status in one channel', async ({ page }) => {
    await page.goto('/tests/fixtures/bot-manifests.html');
    await page.evaluate(async () => {
      const { sharedChannelThreadHtml } = await import('/src/bot-shared-channels.ts');
      const bots = [
        { id: 'author', name: 'Author', avatar: 'panda:0', color: '' },
        { id: 'reviewer', name: 'Reviewer', avatar: 'panda:1', color: '' },
      ] as any;
      const channel = {
        id: 'quality', name: 'quality', memberIds: ['author', 'reviewer'],
        leadBotId: 'author', archived: false, updatedAt: 1, activeBotIds: [],
        messages: [
          { id: 'post', kind: 'bot_post', authorBotId: 'author', text: '@Reviewer status update', status: 'Completed', createdAt: 1 },
          { id: 'request', kind: 'bot_request', authorBotId: 'author', recipientIds: ['reviewer'], text: 'Please review', status: 'Queued', createdAt: 2 },
          { id: 'direct', kind: 'direct_delegation', authorBotId: 'author', text: 'Direct review was requested', status: 'In progress', createdAt: 3 },
        ],
      } as any;
      const host = document.createElement('div');
      host.id = 'bot-collaboration-fixture';
      host.innerHTML = sharedChannelThreadHtml(channel, bots);
      document.body.append(host);
    });
    const channel = page.locator('#bot-collaboration-fixture');
    await expect(channel.locator('article.bc-message[data-message-id="post"]')).toContainText('Author');
    await expect(channel.locator('article.bc-message[data-message-id="post"]')).toContainText('Channel update');
    await expect(channel.locator('article.bc-message[data-message-id="request"]')).toContainText('Requested Reviewer · Queued');
    await expect(channel.locator('.bc-assigned[data-message-id="direct"]')).toContainText('Direct review was requested');
    await expect(channel.locator('.bc-assigned[data-message-id="direct"]')).toContainText('In progress');
    await expect(channel.locator('.bc-message')).toHaveCount(2);
    await expect(channel.locator('.bc-assigned')).toHaveCount(1);
  });
});
