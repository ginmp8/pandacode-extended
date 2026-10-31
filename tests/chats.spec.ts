import { expect, test, type Page } from '@playwright/test';

const chats = (page: Page) => page.getByRole('region', { name: 'Chats', exact: true });
const prompt = (page: Page) => page.getByRole('textbox', { name: 'Message your coding agent', exact: true });
const row = (page: Page, id: string) => page.locator(`.task-item[data-id="${id}"]`);
const menu = (page: Page) => page.getByRole('dialog', { name: 'Task actions', exact: true });

async function probe(page: Page): Promise<void> {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch();
    const source = await response.text();
    const marker = /switch\s*\(command\)\s*\{/;
    expect(source).toMatch(marker);
    const body = source.replace(marker, `
      const chatTestWindow = window;
      (chatTestWindow.__chatCalls ??= []).push({ command, args: structuredClone(args) });
      if (command === 'chat_create' && chatTestWindow.__holdChatCreate) {
        chatTestWindow.__holdChatCreate = false;
        await new Promise(resolve => { chatTestWindow.__releaseChatCreate = resolve; });
      }
      if (command === 'list_projects' && chatTestWindow.__holdChatProjects) {
        chatTestWindow.__holdChatProjects = false;
        await new Promise(resolve => { chatTestWindow.__releaseChatProjects = resolve; });
      }
      if (command === 'list_projects' && chatTestWindow.__failChatProjectsOnce) {
        chatTestWindow.__failChatProjectsOnce = false;
        throw new Error('Chat project refresh failed.');
      }
      if (command === 'get_task' && chatTestWindow.__holdChatNavigation) {
        chatTestWindow.__holdChatNavigation = false;
        chatTestWindow.__heldChatNavigationId = args.sessionId;
        await new Promise(resolve => { chatTestWindow.__releaseChatNavigation = resolve; });
      }
      if (command === 'get_task' && chatTestWindow.__failChatNavigationOnce) {
        chatTestWindow.__failChatNavigationOnce = false;
        throw new Error('Chat navigation failed.');
      }
      if (command === 'list_projects' && chatTestWindow.__failChatProjectsRemaining > 0) {
        chatTestWindow.__failChatProjectsRemaining--;
        throw new Error('Chat project refresh failed.');
      }
      if (command === 'get_task' && args.sessionId === chatTestWindow.__chatTestTargetId && chatTestWindow.__failChatNavigationRemaining > 0) {
        chatTestWindow.__failChatNavigationRemaining--;
        throw new Error('Chat navigation failed.');
      }
      if (command === 'delete_session' && chatTestWindow.__holdChatDelete) {
        chatTestWindow.__holdChatDelete = false;
        await new Promise(resolve => { chatTestWindow.__releaseChatDelete = resolve; });
      }
      if (command === 'delete_session' && chatTestWindow.__failChatDeleteOnce) {
        chatTestWindow.__failChatDeleteOnce = false;
        throw new Error('Chat deletion failed.');
      }
      if (command === 'delete_session' && chatTestWindow.__rejectManagedChatId === args.sessionId) {
        throw new Error('This chat is managed by a team. Delete its parent task instead.');
      }
      if (command === 'chat_cleanup' && chatTestWindow.__failChatCleanup) {
        throw new Error('Hidden project cleanup failed.');
      }
      if (command === 'chat_cleanup' && chatTestWindow.__returnChatCleanupFalse) return false;
      switch (command) {
    `);
    await route.fulfill({ response, body: body + `
      window.__chatPreviewUrl = import.meta.url;
      window.__chatTestSetProtection = (id, status, goalActive = false) => {
        const chat = session(id);
        chat.status = status;
        chat.goalState = goalActive ? { threadId: id, objective: 'Protected goal', status: 'active',
          tokenBudget: null, tokensUsed: 0, timeUsedSeconds: 0, createdAt: Date.now(), updatedAt: Date.now() } : null;
        emit('sessions-changed', undefined);
      };
    ` });
  });
  await page.goto('/');
  await expect(page.locator('.task-item.selected')).toHaveCount(1, { timeout: 20_000 });
  await expect(prompt(page)).toBeEnabled({ timeout: 20_000 });
}

async function calls(page: Page, command: string): Promise<any[]> {
  return page.evaluate(command => ((window as any).__chatCalls ?? []).filter((entry: any) => entry.command === command), command);
}

async function snapshot(page: Page): Promise<{ sessions: any[]; projects: any[] }> {
  return page.evaluate(async () => {
    const preview = await import((window as any).__chatPreviewUrl);
    return { sessions: await preview.invoke('list_sessions', {}), projects: await preview.invoke('list_projects', {}) };
  });
}

async function newChat(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'New chat', exact: true }).click();
  await expect(chats(page).locator('.task-item.selected')).toHaveCount(1, { timeout: 20_000 });
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeEnabled({ timeout: 20_000 });
  await expect(prompt(page)).toBeEnabled({ timeout: 20_000 });
}

async function commandNewChat(page: Page, waitForReady = true): Promise<void> {
  await page.keyboard.press(`${process.platform === 'darwin' ? 'Meta' : 'Control'}+k`);
  await page.getByRole('button', { name: 'New chat (no project)', exact: true }).click();
  if (!waitForReady) return;
  await expect(chats(page).locator('.task-item.selected')).toHaveCount(1, { timeout: 20_000 });
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeEnabled({ timeout: 20_000 });
  await expect(prompt(page)).toBeEnabled({ timeout: 20_000 });
}

async function deleteChatFromRow(page: Page, id: string, keyboard = false): Promise<void> {
  if (keyboard) {
    await row(page, id).focus();
    await page.keyboard.press('Shift+F10');
  } else await row(page, id).click({ button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Delete chat…', exact: true }).click();
}

test.beforeEach(async ({ page }) => {
  await probe(page);
});

test('a new chat lists under Finished, not a project folder, without project affordances', async ({ page }) => {
  await newChat(page);
  await expect(page.locator('.folder-heading', { hasText: 'Chat' })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'What’s on your mind?' })).toBeVisible();
  await expect(page.locator('#breadcrumb')).toHaveText('Chat');
  await expect(page.locator('#deploy-button')).toBeHidden();
  await expect(page.locator('#composer-area .composer-context')).toBeHidden();
  await expect(page.locator('#prompt')).toHaveAttribute('placeholder', 'Ask anything…');

  // Leaving the chat brings the project affordances back.
  await page.locator('#navigation .task-item[data-id="session-1"]').click();
  await expect(page.locator('#deploy-button')).toBeVisible();
});

test('deleting a chat removes it and its hidden project', async ({ page }) => {
  await newChat(page);
  const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await page.getByRole('button', { name: 'Task actions', exact: true }).click();
  await page.getByRole('dialog', { name: 'Task actions', exact: true }).getByRole('menuitem', { name: 'Delete task…', exact: true }).click();
  await page.getByRole('dialog', { name: 'Delete task?', exact: true }).getByRole('button', { name: 'Delete task', exact: true }).click();
  await expect(page.locator(`#navigation .task-item[data-id="${id}"]`)).toHaveCount(0);
  // The project filter lists real projects only.
  await page.locator('#project-filter').click();
  await expect(page.getByRole('menuitemradio', { name: /orbit/ })).toBeVisible();
  await expect(page.getByRole('menuitemradio', { name: /^Chat\b/ })).toHaveCount(0);
});

test('repeated New chat reuses the selected untouched chat', async ({ page }) => {
  const before = await snapshot(page);
  await newChat(page);
  const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await newChat(page);
  await expect(chats(page).locator('.task-item.selected')).toHaveAttribute('data-id', id!);
  expect(await calls(page, 'chat_create')).toHaveLength(1);
  const after = await snapshot(page);
  expect(after.sessions).toHaveLength(before.sessions.length + 1);
  expect(after.projects).toHaveLength(before.projects.length + 1);
});

test('rapid activations from the palette and sidebar share one create through navigation', async ({ page }) => {
  const before = await snapshot(page);
  await page.evaluate(() => {
    const w = window as any;
    w.__holdChatCreate = true; w.__holdChatProjects = true; w.__holdChatNavigation = true;
  });
  await commandNewChat(page, false);
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseChatCreate)).toBe('function');
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeDisabled();
  await commandNewChat(page, false);
  expect(await calls(page, 'chat_create')).toHaveLength(1);

  await page.evaluate(() => (window as any).__releaseChatCreate());
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseChatProjects)).toBe('function');
  await commandNewChat(page, false);
  expect(await calls(page, 'chat_create')).toHaveLength(1);

  await page.evaluate(() => (window as any).__releaseChatProjects());
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseChatNavigation)).toBe('function');
  await commandNewChat(page, false);
  expect(await calls(page, 'chat_create')).toHaveLength(1);
  await page.evaluate(() => (window as any).__releaseChatNavigation());

  await expect(chats(page).locator('.task-item.selected')).toHaveCount(1);
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeEnabled();
  const after = await snapshot(page);
  expect(after.sessions).toHaveLength(before.sessions.length + 1);
  expect(after.projects).toHaveLength(before.projects.length + 1);
});

test('project refresh and navigation failures retry the same created chat', async ({ page }) => {
  const before = await snapshot(page);
  const projectReads = (await calls(page, 'list_projects')).length;
  const taskReads = (await calls(page, 'get_task')).length;
  await page.evaluate(() => {
    const w = window as any;
    w.__failChatProjectsOnce = true; w.__failChatNavigationOnce = true;
  });
  await newChat(page);
  expect(await calls(page, 'chat_create')).toHaveLength(1);
  expect(await calls(page, 'list_projects')).toHaveLength(projectReads + 2);
  expect(await calls(page, 'get_task')).toHaveLength(taskReads + 2);
  const after = await snapshot(page);
  expect(after.sessions).toHaveLength(before.sessions.length + 1);
  expect(after.projects).toHaveLength(before.projects.length + 1);
});

test('drafts and prompt attachments make New chat create a separate conversation and stay with the old one', async ({ page }) => {
  await newChat(page);
  const firstId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Keep this unsent chat draft');
  await page.getByRole('button', { name: 'Attach files for context' }).click();
  const attachMenu = page.getByRole('dialog', { name: 'Attach', exact: true });
  await attachMenu.getByRole('menuitem', { name: /Attach files…/ }).click();
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(2);
  await newChat(page);
  const ids = await chats(page).locator('.task-item').evaluateAll(nodes => nodes.map(node => (node as HTMLElement).dataset.id));
  expect(ids).toHaveLength(2);
  expect(ids).toContain(firstId);
  await row(page, firstId!).click();
  await expect(prompt(page)).toHaveValue('Keep this unsent chat draft');
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(2);
  expect(await calls(page, 'chat_create')).toHaveLength(2);
});

test('a chat with saved messages starts a separate conversation without stopping its agent', async ({ page }) => {
  await newChat(page);
  const firstId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Save this conversation first');
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(page.locator('#transcript')).toContainText('Save this conversation first');
  await newChat(page);
  const nextId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  expect(nextId).not.toBe(firstId);
  // A running chat moves to Active, and to Needs review when its turn finishes.
  // Both conversations must remain visible exactly once across those sections.
  await expect(row(page, firstId!)).toHaveCount(1);
  await expect(row(page, firstId!)).toBeVisible();
  await expect(row(page, nextId!)).toHaveCount(1);
  await expect(row(page, nextId!)).toBeVisible();
  expect(await calls(page, 'chat_create')).toHaveLength(2);
  expect(await calls(page, 'stop_session')).toEqual([]);
});

test('unselected chat deletion honors Escape and preserves a selected chat', async ({ page }) => {
  await newChat(page);
  const targetId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Keep the target draft until deletion is confirmed');
  await newChat(page);
  const otherChatId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Keep the other chat draft');
  const target = await snapshot(page).then(data => data.sessions.find(task => task.id === targetId));

  await deleteChatFromRow(page, targetId!);
  const dialog = page.getByRole('dialog', { name: 'Delete chat?', exact: true });
  await expect(dialog.locator('.delete-task-name')).toHaveText(target.title);
  expect(await calls(page, 'delete_session')).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(row(page, targetId!)).toBeVisible();
  expect(await calls(page, 'delete_session')).toEqual([]);

  await deleteChatFromRow(page, targetId!);
  await page.getByRole('dialog', { name: 'Delete chat?', exact: true }).getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(row(page, targetId!)).toHaveCount(0);
  await expect(row(page, otherChatId!)).toBeVisible();
  await expect(row(page, otherChatId!)).toHaveAttribute('aria-current', 'page');
  expect((await calls(page, 'delete_session')).map(entry => entry.args.sessionId)).toEqual([targetId]);
  expect((await calls(page, 'chat_cleanup')).map(entry => entry.args.projectId)).toEqual([target.projectId]);
  await row(page, otherChatId!).click();
  await expect(prompt(page)).toHaveValue('Keep the other chat draft');

  await newChat(page);
  const thirdChatId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Keep the selected chat while another is deleted');
  const other = await snapshot(page).then(data => data.sessions.find(task => task.id === otherChatId));
  await deleteChatFromRow(page, otherChatId!);
  const changedSelection = page.getByRole('dialog', { name: 'Delete chat?', exact: true });
  await row(page, 'session-2').evaluate(element => element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true })));
  await expect(row(page, 'session-2')).toHaveAttribute('aria-current', 'page');
  await changedSelection.getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(row(page, otherChatId!)).toHaveCount(0);
  await expect(row(page, thirdChatId!)).toBeVisible();
  await expect(row(page, 'session-2')).toHaveAttribute('aria-current', 'page');
  expect((await calls(page, 'delete_session')).map(entry => entry.args.sessionId)).toEqual([targetId, otherChatId]);
  expect((await calls(page, 'chat_cleanup')).map(entry => entry.args.projectId)).toEqual([target.projectId, other.projectId]);
  await row(page, thirdChatId!).click();
  await expect(prompt(page)).toHaveValue('Keep the selected chat while another is deleted');
});

test('chat deletion errors keep the target and skip cleanup until a successful retry', async ({ page }) => {
  await newChat(page);
  const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  const target = await snapshot(page).then(data => data.sessions.find(task => task.id === id));
  await deleteChatFromRow(page, id!);
  await page.evaluate(() => { (window as any).__failChatDeleteOnce = true; });
  const dialog = page.getByRole('dialog', { name: 'Delete chat?', exact: true });
  await dialog.getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(dialog.getByRole('alert')).toHaveText('Chat deletion failed.');
  await expect(row(page, id!)).toBeVisible();
  expect(await calls(page, 'chat_cleanup')).toEqual([]);
  await dialog.getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(row(page, id!)).toHaveCount(0);
  expect((await calls(page, 'delete_session')).map(entry => entry.args.sessionId)).toEqual([id, id]);
  expect((await calls(page, 'chat_cleanup')).map(entry => entry.args.projectId)).toEqual([target.projectId]);
});

test('pinned chat supports keyboard Delete chat and confirmed deletion', async ({ page }) => {
  await newChat(page);
  const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await row(page, id!).focus();
  await page.keyboard.press('Shift+F10');
  await menu(page).getByRole('menuitem', { name: 'Pin task', exact: true }).click();
  await expect(page.locator('#navigation')).toHaveAttribute('aria-busy', 'false');
  const pinned = page.locator('[data-thread-group="pinned"]');
  await expect(row(page, id!)).toBeVisible();
  await expect(pinned.locator('.task-item')).toHaveCount(1);
  await row(page, id!).focus();
  await page.keyboard.press('ContextMenu');
  await menu(page).getByRole('menuitem', { name: 'Delete chat…', exact: true }).click();
  await page.getByRole('dialog', { name: 'Delete chat?', exact: true }).getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(row(page, id!)).toHaveCount(0);
  await expect(pinned.locator('.task-item')).toHaveCount(0);
});

test('persistent refresh and navigation failures recover the same created chat on later clicks', async ({ page }) => {
  const before = await snapshot(page);
  await page.evaluate(() => { (window as any).__failChatProjectsRemaining = 2; });
  await page.getByRole('button', { name: 'New chat', exact: true }).click();
  await expect(page.locator('#toast')).toContainText('Chat project refresh failed.');
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeEnabled();
  expect(await calls(page, 'chat_create')).toHaveLength(1);

  const partial = await snapshot(page);
  const created = partial.sessions.find(task => !before.sessions.some(existing => existing.id === task.id));
  expect(created).toBeTruthy();
  await page.evaluate(id => {
    const w = window as any;
    w.__chatTestTargetId = id;
    w.__failChatNavigationRemaining = 2;
  }, created.id);
  await page.getByRole('button', { name: 'New chat', exact: true }).click();
  await expect(page.locator('#toast')).toContainText('Chat navigation failed.');
  await expect(page.getByRole('button', { name: 'New chat', exact: true })).toBeEnabled();
  expect(await calls(page, 'chat_create')).toHaveLength(1);

  await newChat(page);
  await expect(chats(page).locator('.task-item.selected')).toHaveAttribute('data-id', created.id);
  expect(await calls(page, 'chat_create')).toHaveLength(1);
  const after = await snapshot(page);
  expect(after.sessions).toHaveLength(before.sessions.length + 1);
  expect(after.projects).toHaveLength(before.projects.length + 1);
});

test('deletion pending during selection and draft changes affects only the captured chat', async ({ page }) => {
  await newChat(page);
  const targetId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Draft on chat to remove');
  await newChat(page);
  const otherId = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await prompt(page).fill('Preserve this other draft');
  await deleteChatFromRow(page, targetId!);
  await page.evaluate(() => { (window as any).__holdChatDelete = true; });
  const dialog = page.getByRole('dialog', { name: 'Delete chat?', exact: true });
  await dialog.getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseChatDelete)).toBe('function');
  await expect(dialog.getByRole('button', { name: 'Delete chat', exact: true })).toBeDisabled();

  // A native session event or a programmatic navigation can race the pending IPC.
  // Dispatch directly because the confirmation dialog normally blocks pointer input behind it.
  await row(page, 'session-2').evaluate(element => element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true })));
  await expect(row(page, 'session-2')).toHaveAttribute('aria-current', 'page');
  await expect(prompt(page)).toBeEnabled();
  await prompt(page).evaluate((element: HTMLTextAreaElement) => {
    element.value = 'Draft changed while the deletion was pending';
    element.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.evaluate(() => (window as any).__releaseChatDelete());
  await expect(row(page, targetId!)).toHaveCount(0);
  await expect(row(page, 'session-2')).toHaveAttribute('aria-current', 'page');
  await expect(prompt(page)).toHaveValue('Draft changed while the deletion was pending');
  await row(page, otherId!).click();
  await expect(prompt(page)).toHaveValue('Preserve this other draft');
  expect((await calls(page, 'delete_session')).map(entry => entry.args.sessionId)).toEqual([targetId]);
  expect(await calls(page, 'stop_session')).toEqual([]);
});

for (const failure of ['rejected', 'not-cleaned'] as const) {
  test(`a ${failure} hidden-project cleanup warns after committed deletion without retrying the native delete`, async ({ page }) => {
    await newChat(page);
    const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
    const projectId = (await snapshot(page)).sessions.find(task => task.id === id)!.projectId;
    await page.evaluate(failure => {
      const w = window as any;
      if (failure === 'rejected') w.__failChatCleanup = true;
      else w.__returnChatCleanupFalse = true;
    }, failure);
    await deleteChatFromRow(page, id!);
    await page.getByRole('dialog', { name: 'Delete chat?', exact: true }).getByRole('button', { name: 'Delete chat', exact: true }).click();
    await expect(row(page, id!)).toHaveCount(0);
    await expect(page.locator('#toast')).toContainText('hidden project cleanup failed');
    expect((await calls(page, 'delete_session')).map(entry => entry.args.sessionId)).toEqual([id]);
    expect((await calls(page, 'chat_cleanup')).map(entry => entry.args.projectId)).toEqual([projectId]);
    expect((await snapshot(page)).projects.some(project => project.id === projectId)).toBe(true);
    await expect(page.getByRole('dialog', { name: 'Delete chat?', exact: true })).toBeHidden();
  });
}

test('the sidebar disables deletion for running, awaiting-approval and active-goal chats', async ({ page }) => {
  await newChat(page);
  const id = (await chats(page).locator('.task-item.selected').getAttribute('data-id'))!;
  for (const status of ['running', 'waiting'] as const) {
    await page.evaluate(({ id, status }) => (window as any).__chatTestSetProtection(id, status), { id, status });
    await expect(row(page, id).locator('.task-status.running')).toBeVisible();
    await row(page, id).click({ button: 'right' });
    await expect(menu(page).getByRole('menuitem', { name: 'Delete chat…', exact: true })).toBeDisabled();
    await page.keyboard.press('Escape');
  }
  await page.evaluate(id => (window as any).__chatTestSetProtection(id, 'idle', true), id);
  // Wait for the sidebar's asynchronous session reload rather than guessing a delay.
  await expect.poll(async () => {
    await row(page, id).click({ button: 'right' });
    const blocked = await menu(page).getByRole('menuitem', { name: 'Delete chat…', exact: true }).isDisabled();
    await page.keyboard.press('Escape');
    return blocked;
  }).toBe(true);
  expect(await calls(page, 'delete_session')).toEqual([]);
});

test('a team-managed chat rejected by native deletion stays visible without cleanup', async ({ page }) => {
  await newChat(page);
  const id = await chats(page).locator('.task-item.selected').getAttribute('data-id');
  await page.evaluate(id => { (window as any).__rejectManagedChatId = id; }, id);
  await deleteChatFromRow(page, id!);
  const dialog = page.getByRole('dialog', { name: 'Delete chat?', exact: true });
  await dialog.getByRole('button', { name: 'Delete chat', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('managed by a team');
  await expect(row(page, id!)).toBeVisible();
  expect(await calls(page, 'chat_cleanup')).toEqual([]);
  await expect(dialog.getByRole('button', { name: 'Delete chat', exact: true })).toBeEnabled();
});

test('Go to anything starts a chat, and an empty chat can move to a project', async ({ page }) => {
  await commandNewChat(page);

  await page.getByRole('button', { name: 'Move to project…' }).click();
  await page.getByRole('option', { name: /orbit/ }).click();
  await expect(chats(page)).toHaveCount(0);
  await expect(page.locator('.folder-group[aria-label="orbit tasks"] .task-item.selected')).toHaveCount(1, { timeout: 20_000 });
});
