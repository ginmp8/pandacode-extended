import { expect, test } from '@playwright/test';

test.describe('bot collaboration', () => {
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
