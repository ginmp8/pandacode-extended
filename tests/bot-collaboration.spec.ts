import { expect, test } from '@playwright/test';

test.describe('bot collaboration', () => {
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
    await fixture.locator('input[name="councilEnabled"]').check();
    await expect(fixture.locator('input[name="councilEnabled"]')).toBeChecked();
    await expect(fixture.locator('input[name="councilTarget"]')).toHaveValue('9');
    await expect(fixture.locator('input[name="councilMaxRounds"]')).toHaveValue('10');
    await expect(fixture.locator('input[name="councilReviewer"]')).toHaveCount(2);
    await fixture.locator('input[name="councilMode"][value="selected"]').check();
    await expect(fixture.locator('input[name="councilScopedFile"]')).toHaveCount(1);
    await expect(fixture.locator('input[name="councilFilePath"]')).toHaveCount(1);

    await fixture.locator('input[name="councilTarget"]').fill('8.5');
    await fixture.locator('input[name="councilMaxRounds"]').fill('4');
    await fixture.locator('input[name="councilReviewer"][value="reviewer"]').check();
    await fixture.locator('input[name="councilScopedFile"]').check();
    await fixture.locator('input[name="councilFilePath"]').fill('src/entry.ts');
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
