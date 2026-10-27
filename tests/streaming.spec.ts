import { expect, test, type Page } from '@playwright/test';

type View = 'conversation' | 'board';
const prompt = (page: Page) => page.getByRole('textbox', { name: 'Message your coding agent', exact: true });
const live = (page: Page) => page.locator('#stream-region .live-text');
const transcript = (page: Page) => page.getByRole('log', { name: 'Conversation', exact: true });
const liveGroups = (page: Page) => page.locator('#stream-region details.tool-group');
const liveSequence = (page: Page) => page.locator('#stream-region .live-text, #stream-region [data-commentary], #stream-region details.tool-group').evaluateAll(nodes => nodes.map(node => {
  if (!node.matches('.tool-group')) return node.textContent?.trim();
  const count = node.querySelectorAll('.tool-activity').length;
  return `${count} tool${count === 1 ? '' : 's'}`;
}).filter(Boolean));

async function installControlledStreaming(page: Page): Promise<void> {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch();
    const source = await response.text();
    const marker = /switch \(command\) \{/;
    expect(source).toMatch(marker);
    const body = source.replace(marker, `
      if (command === 'get_messages' && args.sessionId === window.__streamFailMessages) throw new Error('Saved response is temporarily unavailable.');
      if (command === 'get_messages' && args.sessionId === window.__streamHoldMessages) {
        await new Promise(resolve => { window.__releaseStreamMessages = resolve; });
      }
      if (command === 'send_message') {
        const s = session(args.sessionId);
        s.status = 'running'; s.taskStage = 'in_progress'; s.updatedAt = Date.now();
        if (s.title === 'New conversation') s.title = args.text.slice(0, 48);
        messages[s.id] ??= [];
        messages[s.id].push({ id: ++id, sessionId: s.id, role: 'user', content: args.text, createdAt: Date.now() });
        let pending = '', liveTool, toolIndex = 0;
        const recordedTools = new Map();
        const persist = () => {
          if (!pending) return;
          messages[s.id].push({ id: ++id, sessionId: s.id, role: 'assistant', content: pending, createdAt: Date.now() });
          pending = '';
        };
        window.__streamFixtures ??= {};
        window.__streamFixtures[s.id] = {
          push(text) { pending += text; s.lastResponseAt = Date.now(); agent({ sessionId: s.id, kind: 'text', text }); },
          messageStart() { persist(); agent({ sessionId: s.id, kind: 'message_start', text: '' }); },
          startTool() {
            persist();
            liveTool = { id: 'controlled-background-' + s.id + '-' + ++toolIndex, name: 'Bash', status: 'running', input: 'npm test', output: '' };
            const liveToolMessage = { id: ++id, sessionId: s.id, role: 'tool', content: encodeTool(liveTool), createdAt: Date.now() };
            recordedTools.set(liveTool.id, { tool: liveTool, message: liveToolMessage });
            messages[s.id].push(liveToolMessage);
            agent({ sessionId: s.id, kind: 'tool', text: 'Bash', tool: { ...liveTool } });
            return liveTool.id;
          },
          updateTool(status, toolId = liveTool.id) {
            const record = recordedTools.get(toolId);
            record.tool.status = status; record.tool.output += 'More command output. ';
            record.message.content = encodeTool(record.tool);
            agent({ sessionId: s.id, kind: 'tool', text: 'Bash output', tool: { ...record.tool } });
          },
          tool(overrides = {}) {
            persist();
            const activity = { id: 'controlled-read-' + s.id + '-' + ++toolIndex, name: 'Read', status: 'completed', input: 'README.md', output: 'Controlled fixture output.' };
            Object.assign(activity, overrides);
            messages[s.id].push({ id: ++id, sessionId: s.id, role: 'tool', content: encodeTool(activity), createdAt: Date.now() });
            agent({ sessionId: s.id, kind: 'tool', text: 'Read completed', tool: activity });
            return activity.id;
          },
          done() {
            persist(); finishTiming(s.id); s.status = 'idle'; s.taskStage = 'review';
            agent({ sessionId: s.id, kind: 'done', text: '' });
          }
        };
        window.__latestStreamId = s.id;
        agent({ sessionId: s.id, kind: 'status', text: 'Starting agent' });
        return undefined;
      }
      switch (command) {`);
    await route.fulfill({ response, body });
  });
}

async function startTurn(page: Page, view: View, expandActivity = true): Promise<string> {
  if (view === 'board') {
    await page.getByRole('button', { name: 'Board view', exact: true }).click();
    const board = page.getByRole('region', { name: 'Project board', exact: true });
    await board.getByRole('button', { name: 'Add task', exact: true }).click();
    await page.locator('#dialog').getByRole('textbox', { name: 'Task title', exact: true }).fill('Controlled streaming response');
    await page.locator('#dialog').getByRole('textbox', { name: 'Task description', exact: true }).fill('Controlled streaming response');
    await page.locator('#dialog').getByRole('button', { name: 'Add to pending', exact: true }).click();
    await page.getByRole('region', { name: 'Task details', exact: true }).getByRole('button', { name: 'Start task', exact: true }).click();
  } else {
    await page.getByRole('button', { name: /^New task (?:⌘|Ctrl) N$/ }).click();
    await expect(page.getByRole('heading', { name: /^What should we build in/ })).toBeVisible();
    await expect(prompt(page)).toBeEnabled();
    await prompt(page).fill('Controlled streaming response');
    await prompt(page).press('Enter');
  }
  await expect.poll(() => page.evaluate(() => (window as any).__latestStreamId ?? '')).not.toBe('');
  await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeVisible();
  if (expandActivity) {
    const activity = page.locator('#stream-region .turn-activity');
    await expect(activity).toHaveCount(1);
    if (!await activity.evaluate((node: HTMLDetailsElement) => node.open)) await activity.locator('.turn-activity-summary').click();
  }
  return page.evaluate(() => (window as any).__latestStreamId);
}

async function push(page: Page, id: string, text: string): Promise<void> {
  await page.evaluate(({ id, text }) => (window as any).__streamFixtures[id].push(text), { id, text });
}

async function finish(page: Page, id: string): Promise<void> {
  await page.evaluate(id => (window as any).__streamFixtures[id].done(), id);
  await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeHidden();
  await expect(prompt(page)).toBeEnabled();
}

test.describe('streamed responses', () => {
  test.beforeEach(async ({ page }) => {
    await installControlledStreaming(page);
    await page.goto('/');
    await expect(page.locator('.task-item.selected .task-title')).toHaveText('Make search feel instant');
  });

  for (const fixture of [
    { name: 'Skill', input: JSON.stringify({ skill: 'simplify', args: 'Only show after disclosure' }), output: 'Launching skill: simplify' },
    { name: 'unknown_tool', input: 'Only show after disclosure', output: '' },
  ]) {
    test(`${fixture.name} without a card keeps raw details closed until keyboard disclosure`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: 'reduce' });
      const id = await startTurn(page, 'conversation');
      const key = await page.evaluate(({ id, fixture }) => (window as any).__streamFixtures[id].tool(fixture), { id, fixture });
      const tool = transcript(page).locator(`details.tool-trace[data-tool-key="tool:${key}"]`);
      const raw = tool.locator('details.tool-trace-raw');
      await expect(tool).toHaveCount(1);
      await expect(tool.locator('.tool-trace-card')).toHaveCount(0);
      await expect(tool).not.toHaveAttribute('open', '');
      await expect(raw).not.toHaveAttribute('open', '');
      await expect(raw.locator('[data-tool-section="input"] pre')).toBeHidden();
      await tool.locator(':scope > summary').focus();
      await tool.locator(':scope > summary').press('Enter');
      await expect(tool).toHaveAttribute('open', '');
      await expect(raw.locator('[data-tool-section="input"] pre')).toBeHidden();
      await raw.locator(':scope > summary').focus();
      await raw.locator(':scope > summary').press('Enter');
      await expect(raw.locator('[data-tool-section="input"] pre')).toBeVisible();
      await expect(raw.locator('[data-tool-section="input"] pre')).toHaveText(fixture.input);
      await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
      await expect(raw).toHaveAttribute('open', '');
      await finish(page, id);
      await expect(tool).toHaveAttribute('open', '');
      await expect(raw).toHaveAttribute('open', '');
      await expect(raw.locator('[data-tool-section="input"] pre')).toBeVisible();
    });
  }

  test('opening raw details on an automatic tool card pins the reader choice across later results', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const id = await startTurn(page, 'conversation');
    const key = await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
    const tool = transcript(page).locator(`details.tool-trace[data-tool-key="tool:${key}"]`);
    const raw = tool.locator('details.tool-trace-raw');
    await expect(tool).toHaveAttribute('open', '');
    await expect(raw).not.toHaveAttribute('open', '');
    await expect(raw.locator('[data-tool-section="output"] pre')).toBeHidden();
    await raw.locator(':scope > summary').focus();
    await raw.locator(':scope > summary').press('Enter');
    await expect(raw.locator('[data-tool-section="output"] pre')).toBeVisible();
    await page.evaluate(id => { for (let i = 0; i < 4; i++) (window as any).__streamFixtures[id].tool(); }, id);
    await expect(page.locator('#stream-region .tool-activity')).toHaveCount(5);
    await expect(tool).toHaveAttribute('open', '');
    await expect(raw).toHaveAttribute('open', '');
    await finish(page, id);
    await expect(raw.locator('[data-tool-section="output"] pre')).toBeVisible();
  });

  test('another tool updating preserves an unchanged live summary during a pointer click', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const id = await startTurn(page, 'conversation');
    const second = await page.evaluate(id => {
      const fixture = (window as any).__streamFixtures[id];
      fixture.startTool(); return fixture.startTool();
    }, id);
    const tools = page.locator('#stream-region details.tool-trace');
    await expect(tools).toHaveCount(2);
    const first = tools.first(), summary = first.locator(':scope > summary');
    await expect(first).not.toHaveAttribute('open', '');
    await summary.evaluate(summary => {
      const tool = summary.parentElement!;
      const saved = (window as any).__unchangedLiveTool = { summary, tool, detached: false };
      new MutationObserver(records => {
        for (const record of records) for (const removed of record.removedNodes) {
          if (removed === tool || removed.contains(summary)) saved.detached = true;
        }
      }).observe(document.querySelector('#stream-region .live-tools')!, { childList: true, subtree: true });
    });
    const box = (await summary.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.evaluate(({ id, second }) => (window as any).__streamFixtures[id].updateTool('completed', second), { id, second });
    await expect(tools.nth(1)).toHaveAttribute('data-tool-status', 'completed');
    expect(await summary.evaluate(summary => {
      const saved = (window as any).__unchangedLiveTool;
      return { sameSummary: summary === saved.summary, sameTool: summary.parentElement === saved.tool, connected: saved.summary.isConnected, detached: saved.detached };
    })).toEqual({ sameSummary: true, sameTool: true, connected: true, detached: false });
    await page.mouse.up();
    await expect(first).toHaveAttribute('open', '');
    await page.evaluate(({ id, second }) => (window as any).__streamFixtures[id].updateTool('completed', second), { id, second });
    await expect(first).toHaveAttribute('open', '');
  });

  test('switching reduced motion on settles tool expansion, row entry and header animations already running', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    const id = await startTurn(page, 'conversation');
    await page.evaluate(() => {
      const query = matchMedia('(prefers-reduced-motion: reduce)');
      const snapshot = () => document.querySelector('#stream-region')!.getAnimations({ subtree: true })
        .filter(animation => animation.playState === 'running').map(animation => animation.constructor.name);
      (window as any).__traceMotionSnapshot = snapshot;
      query.addEventListener('change', () => {
        if (query.matches) requestAnimationFrame(() => { (window as any).__traceMotionAfter = snapshot(); });
      });
    });
    await page.evaluate(id => (window as any).__streamFixtures[id].tool({
      name: 'mcp__docs__search_docs', input: JSON.stringify({ query: 'motion preference' }),
      output: JSON.stringify({ results: Array.from({ length: 6 }, (_, i) => ({ title: 'Source ' + i, url: 'https://docs.example.com/' + i, snippet: 'A result row.' })) }),
    }), id);
    await page.waitForFunction(() => {
      const running = (window as any).__traceMotionSnapshot();
      if (running.includes('Animation')) { (window as any).__traceMotionBefore = running; return true; }
      return false;
    });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await expect.poll(() => page.evaluate(() => (window as any).__traceMotionAfter)).toEqual([]);
    expect(await page.evaluate(() => (window as any).__traceMotionBefore)).toContain('Animation');
    const tool = page.locator('#stream-region details.tool-trace');
    await expect(tool).toHaveAttribute('open', '');
    expect(await tool.locator('.tool-activity-body').evaluate(node => (node as HTMLElement).style.overflow)).toBe('');
    await expect(tool.locator('.tc-row')).toHaveCount(6);
    // Turning the preference off permits future motion, without restarting old animations.
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    expect(await tool.evaluate(node => node.getAnimations({ subtree: true }).filter(animation => animation.constructor.name === 'Animation' && animation.playState === 'running').length)).toBe(0);
  });

  for (const folding of ['card', 'turn'] as const) {
    test(`switching reduced motion on completes an in-flight tool ${folding} collapse without clipped content`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      const id = await startTurn(page, 'conversation');
      await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
      await expect(page.locator('#stream-region details.tool-trace')).toHaveAttribute('open', '');
      await expect.poll(() => page.locator('#stream-region').evaluate(root => root.getAnimations({ subtree: true }).filter(animation => animation.constructor.name === 'Animation' && animation.playState === 'running').length)).toBe(0);
      await page.evaluate(async folding => {
        const motion = await import('/src/trace-motion.ts');
        const activity = document.querySelector<HTMLDetailsElement>('#stream-region .live-activity')!;
        const target = folding === 'turn' ? activity : activity.querySelector<HTMLDetailsElement>('details.tool-trace')!;
        const content = target.querySelector<HTMLElement>(':scope > .tool-activity-body, :scope > .turn-activity-body')!;
        const query = matchMedia('(prefers-reduced-motion: reduce)');
        query.addEventListener('change', () => {
          if (query.matches) requestAnimationFrame(() => {
            (window as any).__foldingTraceAfter = { open: target.open, closing: target.hasAttribute('data-closing'), overflow: content.style.overflow, height: content.style.height,
              running: target.getAnimations({ subtree: true }).filter(animation => animation.playState === 'running').length };
          });
        });
        if (folding === 'turn') motion.foldTurn(activity);
        else motion.choreograph(activity.querySelector<HTMLElement>('.live-tools')!, activity.dataset.turnKey!, { answering: true, expandAll: false });
        (window as any).__foldingTraceBefore = { closing: target.hasAttribute('data-closing'), running: target.getAnimations({ subtree: true }).some(animation => animation.constructor.name === 'Animation' && animation.playState === 'running') };
      }, folding);
      expect(await page.evaluate(() => (window as any).__foldingTraceBefore)).toEqual({ closing: true, running: true });
      await page.emulateMedia({ reducedMotion: 'reduce' });
      // A frozen first-frame snapshot cannot pass just because the old animation
      // eventually reached its natural end during an assertion retry.
      await expect.poll(() => page.evaluate(() => (window as any).__foldingTraceAfter))
        .toEqual({ open: false, closing: false, overflow: '', height: '', running: 0 });
    });
  }

  for (const author of ['user', 'agent']) {
    test(`history handoff preserves selection in an unchanged historical ${author} message without detaching it`, async ({ page }) => {
      if (author === 'agent') {
        // The default sample ends in tool activity, so its assistant messages
        // are nested narration. Persist a real completed response first.
        const earlier = await startTurn(page, 'conversation');
        await push(page, earlier, 'An earlier assistant response remains available for selection.');
        await expect(live(page)).toHaveText('An earlier assistant response remains available for selection.');
        await finish(page, earlier);
      }
      const historical = transcript(page).locator(`:scope > .${author}-message[data-message-id]`).first();
      await expect(historical).toBeVisible();
      await expect(historical).toHaveAttribute('data-message-id', /^[1-9]\d*$/);
      // A second turn uses the same session id; wait for its new fixture rather
      // than accepting the id left behind by the previous completed turn.
      await page.evaluate(() => { delete (window as any).__latestStreamId; });
      await prompt(page).fill('Complete a turn while an earlier message is being read');
      await prompt(page).press('Enter');
      await expect.poll(() => page.evaluate(() => (window as any).__latestStreamId ?? '')).not.toBe('');
      const id = await page.evaluate(() => (window as any).__latestStreamId);
      await push(page, id, 'FINAL_RESPONSE_FOR_HISTORY_HANDOFF');
      await expect(live(page)).toHaveText('FINAL_RESPONSE_FOR_HISTORY_HANDOFF');
      await historical.scrollIntoViewIfNeeded();
      const selected = await historical.evaluate(article => {
        const walker = document.createTreeWalker(article.querySelector('.message-content')!, NodeFilter.SHOW_TEXT);
        let node = walker.nextNode() as Text | null;
        while (node && !node.textContent?.trim()) node = walker.nextNode() as Text | null;
        if (!node) throw new Error('Historical fixture has no selectable text.');
        const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, Math.min(18, node.length));
        const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
        const saved = (window as any).__historicalSelection = { article, node, text: selection.toString(), detached: false };
        new MutationObserver(records => {
          for (const record of records) for (const removed of record.removedNodes) {
            if (removed === article || removed.contains(article) || removed.contains(node)) saved.detached = true;
          }
        }).observe(document.querySelector('#transcript')!, { childList: true, subtree: true });
        return saved.text;
      });
      expect(selected.length).toBeGreaterThan(0);
      await page.evaluate(id => { (window as any).__streamHoldMessages = id; (window as any).__streamFixtures[id].done(); }, id);
      await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseStreamMessages)).toBe('function');
      expect(await page.evaluate(() => ({ selected: window.getSelection()?.toString(), detached: (window as any).__historicalSelection.detached }))).toEqual({ selected, detached: false });
      await page.evaluate(() => { (window as any).__streamHoldMessages = null; (window as any).__releaseStreamMessages(); });
      await expect(prompt(page)).toBeEnabled();
      await expect(transcript(page).locator('.agent-message .message-content').filter({ hasText: 'FINAL_RESPONSE_FOR_HISTORY_HANDOFF' })).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message').filter({ hasText: 'FINAL_RESPONSE_FOR_HISTORY_HANDOFF' })).toHaveAttribute('data-message-id', /^[1-9]\d*$/);
      expect(await historical.evaluate(article => {
        const saved = (window as any).__historicalSelection;
        return { sameArticle: article === saved.article, nodeConnected: saved.node.isConnected, selected: window.getSelection()?.toString(), detached: saved.detached };
      })).toEqual({ sameArticle: true, nodeConnected: true, selected, detached: false });
      await page.locator('#navigation .task-item', { hasText: 'Explore the project' }).click();
      await expect(page.locator('#navigation .task-item.selected .task-title')).toHaveText('Explore the project');
      await expect(transcript(page)).toContainText('The project is a small native workspace');
      expect(await page.evaluate(() => (window as any).__historicalSelection.article.isConnected)).toBe(false);
      expect(await page.evaluate(() => window.getSelection()?.toString())).not.toBe(selected);
    });
  }

  for (const content of ['commentary', 'raw output']) {
    test(`history handoff preserves selection inside expanded historical ${content} without detaching its turn`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: 'reduce' });
      const earlier = await startTurn(page, 'conversation');
      await push(page, earlier, 'Historical commentary stays readable during later work.');
      await expect(live(page)).toHaveText('Historical commentary stays readable during later work.');
      await page.evaluate(id => (window as any).__streamFixtures[id].tool({ name: 'Bash', input: 'printf "recorded output"', output: 'Historical raw output stays readable during later work.' }), earlier);
      await expect(page.locator('#stream-region .tool-activity')).toHaveCount(1);
      await push(page, earlier, 'The earlier turn is complete.');
      await expect(live(page)).toHaveText('The earlier turn is complete.');
      await finish(page, earlier);
      const historicalTurn = transcript(page).locator(':scope > details.turn-activity').first();
      await expect(historicalTurn.locator('.agent-message[data-message-id]').first()).toHaveAttribute('data-message-id', /^[1-9]\d*$/);
      await page.evaluate(() => { delete (window as any).__latestStreamId; });
      await prompt(page).fill('Complete another turn while earlier activity is being read');
      await prompt(page).press('Enter');
      await expect.poll(() => page.evaluate(() => (window as any).__latestStreamId ?? '')).not.toBe('');
      const id = await page.evaluate(() => (window as any).__latestStreamId);
      await push(page, id, 'FINAL_RESPONSE_AFTER_HISTORICAL_ACTIVITY');
      await expect(live(page)).toHaveText('FINAL_RESPONSE_AFTER_HISTORICAL_ACTIVITY');
      if (!await historicalTurn.evaluate((turn: HTMLDetailsElement) => turn.open)) await historicalTurn.locator(':scope > summary').click();
      const target = content === 'commentary' ? historicalTurn.locator('.agent-message .message-content').first()
        : historicalTurn.locator('[data-tool-section="output"] pre').first();
      if (content === 'raw output') {
        const group = historicalTurn.locator('.tool-group').first();
        if (!await group.evaluate((details: HTMLDetailsElement) => details.open)) await group.locator(':scope > summary').click();
        const tool = group.locator('details.tool-trace').first();
        if (!await tool.evaluate((details: HTMLDetailsElement) => details.open)) await tool.locator(':scope > summary').click();
        const raw = tool.locator('.tool-trace-raw');
        if (!await raw.evaluate((details: HTMLDetailsElement) => details.open)) await raw.locator(':scope > summary').click();
      }
      await expect(target).toBeVisible();
      const selected = await target.evaluate(target => {
        const walker = document.createTreeWalker(target, NodeFilter.SHOW_TEXT);
        let node = walker.nextNode() as Text | null;
        while (node && !node.textContent?.trim()) node = walker.nextNode() as Text | null;
        if (!node) throw new Error('Historical activity fixture has no selectable text.');
        const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, Math.min(18, node.length));
        const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
        const turn = target.closest('.turn-activity')!;
        const saved = (window as any).__historicalActivitySelection = { target, turn, node, text: selection.toString(), detached: false };
        new MutationObserver(records => {
          for (const record of records) for (const removed of record.removedNodes) {
            if (removed === turn || removed.contains(turn) || removed.contains(node)) saved.detached = true;
          }
        }).observe(document.querySelector('#transcript')!, { childList: true, subtree: true });
        return saved.text;
      });
      expect(selected.length).toBeGreaterThan(0);
      await page.evaluate(id => { (window as any).__streamHoldMessages = id; (window as any).__streamFixtures[id].done(); }, id);
      await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseStreamMessages)).toBe('function');
      expect(await page.evaluate(() => {
        const saved = (window as any).__historicalActivitySelection;
        return { selected: window.getSelection()?.toString(), detached: saved.detached, connected: saved.node.isConnected };
      })).toEqual({ selected, detached: false, connected: true });
      await page.evaluate(() => { (window as any).__streamHoldMessages = null; (window as any).__releaseStreamMessages(); });
      await expect(transcript(page).locator('.agent-message').filter({ hasText: 'FINAL_RESPONSE_AFTER_HISTORICAL_ACTIVITY' })).toHaveAttribute('data-message-id', /^[1-9]\d*$/);
      await expect(target).toBeVisible();
      expect(await target.evaluate(target => {
        const saved = (window as any).__historicalActivitySelection;
        return { sameTarget: target === saved.target, sameTurn: target.closest('.turn-activity') === saved.turn, selected: window.getSelection()?.toString(), detached: saved.detached, connected: saved.node.isConnected };
      })).toEqual({ sameTarget: true, sameTurn: true, selected, detached: false, connected: true });
    });
  }

  test('history handoff preserves composer typing and caret when no transcript text is selected', async ({ page }) => {
    const id = await startTurn(page, 'conversation');
    await push(page, id, 'A completed response alongside a follow-up draft.');
    await expect(live(page)).toHaveText('A completed response alongside a follow-up draft.');
    await prompt(page).fill('A draft follow-up that must not be sent yet.');
    await prompt(page).evaluate((textarea: HTMLTextAreaElement) => {
      window.getSelection()?.removeAllRanges();
      textarea.focus(); textarea.setSelectionRange(8, 17);
    });
    await finish(page, id);
    await expect(transcript(page).locator('.agent-message').filter({ hasText: 'A completed response alongside a follow-up draft.' })).toHaveAttribute('data-message-id', /^[1-9]\d*$/);
    await expect(prompt(page)).toBeFocused();
    await expect(prompt(page)).toHaveValue('A draft follow-up that must not be sent yet.');
    expect(await prompt(page).evaluate((textarea: HTMLTextAreaElement) => [textarea.selectionStart, textarea.selectionEnd])).toEqual([8, 17]);
  });

  test('live tool activity stays open regardless of Settings and preserves manual disclosure choices during updates', async ({ page }) => {
    const id = await startTurn(page, 'conversation', false);
    const activity = page.locator('#stream-region .turn-activity');
    const tools = activity.locator('details.tool-trace');
    const openTools = () => tools.evaluateAll(list => list.flatMap((tool, i) => (tool as HTMLDetailsElement).open ? [i] : []));
    await expect(activity).toHaveAttribute('open', '');
    await page.evaluate(id => { const fixture = (window as any).__streamFixtures[id]; for (let i = 0; i < 6; i++) fixture.tool(); }, id);
    await expect(liveGroups(page)).toHaveCount(2);
    // Live steps hang off the rail: every group is open, its summary hidden, and the two newest results show their cards.
    await expect(activity.locator('.tool-group[open]')).toHaveCount(2);
    await expect(liveGroups(page).first().locator('.tool-group-summary')).toBeHidden();
    await expect.poll(openTools).toEqual([4, 5]);
    await page.keyboard.press('Meta+,');
    await page.getByRole('checkbox', { name: 'Expand tool activity', exact: true }).check();
    await page.getByRole('button', { name: 'Back to the workspace', exact: true }).click();
    await expect(activity).toHaveAttribute('open', '');
    await expect(activity.locator('.tool-group[open]')).toHaveCount(2);
    // A step the reader opens by hand stays open through new text and new tools.
    await tools.first().locator(':scope > summary').click();
    await expect(tools.first()).toHaveAttribute('open', '');
    await push(page, id, 'Another response.');
    await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
    await expect(liveGroups(page)).toHaveCount(3);
    await expect(tools.first()).toHaveAttribute('open', '');
    await expect(liveGroups(page).last()).toHaveAttribute('open', '');
    await page.keyboard.press('Meta+,');
    await page.getByRole('checkbox', { name: 'Expand tool activity', exact: true }).uncheck();
    await page.getByRole('button', { name: 'Back to the workspace', exact: true }).click();
    await expect(activity).toHaveAttribute('open', '');
    await expect(tools.first()).toHaveAttribute('open', '');
    await push(page, id, 'Final answer stays visible.');
    await expect(live(page)).toBeVisible();
    await expect(tools.first()).toHaveAttribute('open', '');
    await finish(page, id);
    await expect(transcript(page).getByText('Final answer stays visible.', { exact: true })).toBeVisible();
    await startTurn(page, 'conversation', false);
    await expect(page.locator('#stream-region .turn-activity')).toHaveAttribute('open', '');
  });

  test('keeps the latest streaming narrative before tools when older activity is trimmed', async ({ page }) => {
    const id = await startTurn(page, 'conversation');
    await push(page, id, 'I’m checking all project files.');
    await expect(live(page)).toHaveText('I’m checking all project files.');
    const originalResponse = await live(page).elementHandle();
    await page.evaluate(id => {
      const fixture = (window as any).__streamFixtures[id];
      for (let i = 0; i < 35; i++) fixture.tool();
    }, id);
    await expect(page.locator('#stream-region .activity-limit')).toBeVisible();
    await expect(live(page)).toBeVisible();
    expect(await originalResponse!.evaluate(node => node === document.querySelector('#stream-region .live-text'))).toBe(true);
    await expect.poll(async () => (await liveSequence(page))[0]).toBe('I’m checking all project files.');
    const retainedTools = await page.locator('#stream-region .tool-activity').count();
    expect(retainedTools).toBeGreaterThan(0);
    expect(retainedTools).toBeLessThanOrEqual(30);
    await expect(page.locator('#stream-region .live-text, #stream-region [data-commentary]').filter({ hasText: 'I’m checking all project files.' })).toHaveCount(1);
    await finish(page, id);
    await expect(transcript(page).locator('.tool-activity')).toHaveCount(35);
  });

  test('open links and code spans render at once instead of waiting for their closing syntax', async ({ page }) => {
    const id = await startTurn(page, 'conversation', false);
    await push(page, id, 'Read [the guide');
    await expect(live(page).locator('.md-open-link')).toHaveText('the guide');
    await expect(live(page)).toHaveText('Read the guide');
    await push(page, id, '](https://exa');
    await expect(live(page)).toHaveText('Read the guide');
    await push(page, id, 'mple.com) then run `npm te');
    await expect(live(page).locator('a')).toHaveAttribute('href', 'https://example.com');
    await expect(live(page).locator('code')).toHaveText('npm te');
    await push(page, id, 'st`.\n\n```\nconst a = list[0');
    await expect(live(page).locator('pre')).toContainText('list[0');
    await finish(page, id);
  });

  for (const view of ['conversation', 'board'] as const) {
    test(`keeps ${view} streaming narratives chronological around compact tool groups and late updates`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'Inspecting **the project**.');
      await expect(live(page)).toHaveText('Inspecting the project.');
      const generating = page.locator('#stream-region .streaming-label');
      await expect(generating).toBeVisible();
      const originalResponse = await live(page).elementHandle();
      const firstToolId = await page.evaluate(id => {
        const fixture = (window as any).__streamFixtures[id];
        const first = fixture.startTool(); fixture.tool();
        return first;
      }, id);
      await expect(liveGroups(page)).toHaveCount(1);
      // A live group with a running call reads in the present tense.
      await expect(liveGroups(page).first().locator('.tool-group-label')).toHaveText('Running 1 command and reading 1 file…');
      // Live groups are the open step rail; the header carries the running sentence instead of a group summary.
      await expect(liveGroups(page).first()).toHaveAttribute('open', '');
      await expect(liveGroups(page).first().locator('.tool-group-summary')).toBeHidden();
      await expect(live(page)).toBeVisible();
      await expect(generating).toBeHidden();
      await expect(page.locator('#stream-region .turn-activity')).toHaveAttribute('open', '');
      await expect.poll(() => liveSequence(page)).toEqual(['Inspecting the project.', '2 tools']);

      await push(page, id, 'Found **one** issue.');
      await expect(live(page)).toHaveText('Found one issue.');
      await expect(generating).toBeVisible();
      await expect(page.locator('#stream-region [data-commentary]')).toBeVisible();
      await expect(page.locator('#stream-region [data-commentary]')).toHaveText('Inspecting the project.');
      const thirdToolId = await page.evaluate(id => (window as any).__streamFixtures[id].startTool(), id);
      await expect(liveGroups(page)).toHaveCount(2);
      await expect(liveGroups(page).last().locator('.tool-group-label')).toHaveText('Running 1 command…');
      await expect(liveGroups(page).last()).toHaveAttribute('open', '');
      await expect(generating).toBeHidden();
      await expect.poll(() => liveSequence(page)).toEqual(['Inspecting the project.', '2 tools', 'Found one issue.', '1 tool']);
      expect(await originalResponse!.evaluate(node => node === document.querySelector('#stream-region .live-text'))).toBe(true);

      const firstGroup = liveGroups(page).first();
      const firstTool = page.locator(`#stream-region .tool-activity[data-tool-key="tool:${firstToolId}"]`);
      await firstTool.locator(':scope > summary').click();
      await firstTool.locator('.tool-trace-raw > summary').click();
      await page.evaluate(({ id, toolId }) => (window as any).__streamFixtures[id].updateTool('running', toolId), { id, toolId: firstToolId });
      await expect(firstGroup).toHaveAttribute('open', '');
      await expect(firstTool).toHaveAttribute('open', '');
      await expect(firstTool.locator('[data-tool-section="output"] pre')).toHaveText('More command output.');
      await expect(firstTool.locator('[data-tool-section="output"] pre')).toBeVisible();
      await expect(page.locator('#stream-region .tool-activity')).toHaveCount(3);
      await expect.poll(() => liveSequence(page)).toEqual(['Inspecting the project.', '2 tools', 'Found one issue.', '1 tool']);

      await push(page, id, 'The fix is ready.');
      await expect(live(page)).toHaveText('The fix is ready.');
      await expect(generating).toBeVisible();
      await expect(page.locator('#stream-region [data-commentary]')).toHaveCount(2);
      for (const commentary of await page.locator('#stream-region [data-commentary]').all()) await expect(commentary).toBeVisible();
      await page.evaluate(({ id, firstToolId, thirdToolId }) => {
        const fixture = (window as any).__streamFixtures[id];
        fixture.updateTool('completed', firstToolId); fixture.updateTool('completed', thirdToolId);
      }, { id, firstToolId, thirdToolId });
      await expect(firstTool).toHaveAttribute('data-tool-status', 'completed');
      await expect(firstGroup).toHaveAttribute('open', '');
      await expect(firstTool).toHaveAttribute('open', '');
      await expect(liveGroups(page).last()).toHaveAttribute('open', '');
      await expect.poll(() => liveSequence(page)).toEqual(['Inspecting the project.', '2 tools', 'Found one issue.', '1 tool', 'The fix is ready.']);
      await expect(page.locator('#stream-region .tool-activity')).toHaveCount(3);

      await finish(page, id);
      await expect(generating).toBeHidden();
      await expect(transcript(page).locator('.tool-activity')).toHaveCount(3);
      await expect(transcript(page).locator('.tool-group')).toHaveCount(2);
      for (const text of ['Inspecting the project.', 'Found one issue.', 'The fix is ready.']) {
        await expect(transcript(page).locator('.message-content, [data-commentary]').filter({ hasText: text })).toHaveCount(1);
      }
      await expect(transcript(page).locator('.message-content').filter({ hasText: 'The fix is ready.' })).toBeVisible();
      const savedTool = transcript(page).locator(`.tool-activity[data-tool-key="tool:${firstToolId}"]`);
      await expect(savedTool).toHaveAttribute('open', '');
      await expect(savedTool.locator('[data-tool-section="output"] pre')).toBeVisible();
      if (view === 'board') {
        for (const text of ['Inspecting the project.', 'Found one issue.']) {
          await expect(transcript(page).locator('.message-content').filter({ hasText: text })).toBeVisible();
        }
      }
    });

    test(`splits ${view} streaming tools into batches of five and starts a new group after a response`, async ({ page, browserName }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'I’m checking the project files.');
      await expect(live(page)).toBeVisible();
      await page.evaluate(id => {
        const fixture = (window as any).__streamFixtures[id];
        for (let i = 0; i < 6; i++) fixture.tool();
      }, id);
      await expect(liveGroups(page).locator('.tool-group-label')).toHaveText(['Read 5 files', 'Read 1 file']);
      await expect.poll(() => liveSequence(page)).toEqual(['I’m checking the project files.', '5 tools', '1 tool']);

      await push(page, id, 'The files look good. I’m checking one last detail.');
      await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
      await expect(liveGroups(page).locator('.tool-group-label')).toHaveText(['Read 5 files', 'Read 1 file', 'Read 1 file']);
      await expect.poll(() => liveSequence(page)).toEqual(['I’m checking the project files.', '5 tools', '1 tool', 'The files look good. I’m checking one last detail.', '1 tool']);
      await expect(page.locator('#stream-region .tool-activity')).toHaveCount(7);
      for (const group of await liveGroups(page).all()) await expect(group).toHaveAttribute('open', '');
      await expect(page.locator('#stream-region [data-commentary]')).toBeVisible();
      await expect(live(page)).toBeVisible();
      if (view === 'board') {
        // Narration lines up with the turn header the step rail hangs from.
        const groupLeft = (await page.locator('#stream-region .turn-activity-summary').boundingBox())!.x;
        for (const text of [page.locator('#stream-region [data-commentary] p').first(), live(page).locator('p').first()]) {
          expect(Math.abs((await text.boundingBox())!.x - groupLeft)).toBeLessThanOrEqual(1);
        }
      }
      await page.screenshot({ path: `/private/tmp/pandacode-chronological-tools-${view}-${browserName}.png` });
      await finish(page, id);
      await expect(transcript(page).locator('.tool-group-label')).toHaveText(['Read 5 files', 'Read 1 file', 'Read 1 file']);
      await expect.poll(() => transcript(page).locator('.turn-activity-body .message-content, .turn-activity-body .tool-group').evaluateAll(nodes => nodes.map(node => node.matches('.tool-group')
        ? node.querySelector('.tool-group-label')?.textContent
        : node.textContent?.trim()))).toEqual(['I’m checking the project files.', 'Read 5 files', 'Read 1 file', 'The files look good. I’m checking one last detail.', 'Read 1 file']);
    });

    test(`shows partial Markdown in ${view} before completion and keeps the response and dropdown steady`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, '**Visible** response');
      await expect(live(page)).toBeVisible();
      await expect(live(page).locator('strong')).toHaveText('Visible');
      await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeVisible();
      await page.getByRole('button', { name: 'Filter projects', exact: true }).click();
      const menu = page.getByRole('dialog', { name: 'Filter projects', exact: true });
      await expect(menu).toHaveAttribute('data-popover-state', 'open');
      await page.evaluate(() => {
        const output = document.querySelector('.live-text')!;
        const selected = output.querySelector('strong')!.firstChild!;
        const range = document.createRange(); range.setStart(selected, 0); range.setEnd(selected, 7);
        const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
        (window as any).__streamPresentation = {
          output, selected, article: output.closest('article'), composer: document.querySelector('#composer'),
          menu: document.querySelector('.choice-menu'), rows: [...document.querySelectorAll('.choice-menu-option')]
        };
      });
      await push(page, id, '\n\nThe update is `ready`.');
      await expect(live(page).locator('code')).toHaveText('ready');
      await expect(live(page)).toContainText('Visible response');
      await expect(menu).toHaveAttribute('data-popover-state', 'open');
      const stable = await page.evaluate(() => {
        const saved = (window as any).__streamPresentation;
        const rows = [...document.querySelectorAll('.choice-menu-option')];
        return {
          output: saved.output === document.querySelector('.live-text'),
          article: saved.article === document.querySelector('.live-text')?.closest('article'),
          composer: saved.composer === document.querySelector('#composer'),
          menu: saved.menu === document.querySelector('.choice-menu'),
          rows: rows.length === saved.rows.length && rows.every((row, index) => row === saved.rows[index]),
          selectedText: window.getSelection()?.toString(),
          selectedNode: saved.selected.isConnected,
        };
      });
      expect(stable).toEqual({ output: true, article: true, composer: true, menu: true, rows: true, selectedText: 'Visible', selectedNode: true });
      await page.keyboard.press('Escape');
      await finish(page, id);
      await expect(transcript(page).locator('.agent-message')).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message strong').filter({ hasText: 'Visible' })).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message code')).toHaveText('ready');
    });

    test(`keeps the last ${view} chunk visible when completion arrives before saved history`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'The first part is visible. ');
      await expect(live(page)).toContainText('The first part is visible.');
      await page.evaluate(id => {
        (window as any).__streamHoldMessages = id;
        (window as any).__streamFixtures[id].push('This final part must stay visible.');
        (window as any).__streamFixtures[id].done();
      }, id);
      await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseStreamMessages)).toBe('function');
      await expect(transcript(page)).toContainText('The first part is visible. This final part must stay visible.');
      await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeHidden();
      await page.evaluate(() => { (window as any).__streamHoldMessages = null; (window as any).__releaseStreamMessages(); });
      await expect(transcript(page).locator('.agent-message')).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message .message-content')).toHaveText('The first part is visible. This final part must stay visible.');
      await expect(prompt(page)).toBeEnabled();
    });

    test(`keeps separate assistant messages in ${view} without repeating their saved snapshots`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'First assistant message.');
      await expect(live(page)).toHaveText('First assistant message.');
      await page.evaluate(id => {
        (window as any).__streamFixtures[id].messageStart();
        return new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      }, id);
      await expect(live(page)).toBeVisible();
      await expect(live(page)).toHaveText('First assistant message.');
      await expect(page.locator('#stream-region [data-commentary]')).toHaveCount(0);
      await push(page, id, 'Second assistant message.');
      await expect(live(page)).toHaveText('Second assistant message.');
      await expect(transcript(page)).toContainText('First assistant message.');
      await finish(page, id);
      await expect(transcript(page).locator('.agent-message')).toHaveCount(2);
      await expect(transcript(page).locator('.agent-message .message-content')).toHaveText(['First assistant message.', 'Second assistant message.']);
    });

    test(`keeps the latest ${view} message visible through tools until new response text replaces it`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'Inspecting **the project**.');
      await expect(live(page)).toHaveText('Inspecting the project.');
      await page.evaluate(() => {
        (window as any).__retainedResponse = {
          article: document.querySelector('#stream-region .live-message'),
          output: document.querySelector('#stream-region .live-text'),
        };
      });
      await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
      await expect(page.locator('#stream-region .live-tools .tool-activity')).toHaveCount(1);
      await expect(live(page)).toBeVisible();
      await expect(live(page)).toHaveText('Inspecting the project.');
      await expect(page.locator('#stream-region [data-commentary]')).toHaveCount(0);
      await page.evaluate(id => {
        const fixture = (window as any).__streamFixtures[id];
        fixture.messageStart(); fixture.messageStart(); fixture.push('');
        return new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      }, id);
      await expect(live(page)).toBeVisible();
      await expect(live(page)).toHaveText('Inspecting the project.');
      await expect(page.locator('#stream-region [data-commentary]')).toHaveCount(0);
      expect(await page.evaluate(() => {
        const saved = (window as any).__retainedResponse;
        return saved.article === document.querySelector('#stream-region .live-message')
          && saved.output === document.querySelector('#stream-region .live-text');
      })).toBe(true);

      await push(page, id, 'Found **one** issue.');
      await expect(live(page)).toHaveText('Found one issue.');
      await expect(live(page).locator('strong')).toHaveText('one');
      await expect(page.locator('#stream-region [data-commentary]')).toHaveCount(1);
      await expect(page.locator('#stream-region [data-commentary]')).toHaveText('Inspecting the project.');
      expect(await page.evaluate(() => {
        const saved = (window as any).__retainedResponse;
        return saved.article === document.querySelector('#stream-region .live-message')
          && saved.output === document.querySelector('#stream-region .live-text');
      })).toBe(true);
      await push(page, id, ' Fixing it now.');
      await expect(live(page)).toHaveText('Found one issue. Fixing it now.');
      await finish(page, id);
      await expect(transcript(page).locator('.agent-message .message-content').filter({ hasText: 'Found one issue. Fixing it now.' })).toHaveCount(1);
    });

    test(`finalizes a retained ${view} message once when work ends without another response`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'The update is ready for review.');
      await expect(live(page)).toHaveText('The update is ready for review.');
      await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
      await expect(page.locator('#stream-region .live-tools .tool-activity')).toHaveCount(1);
      await expect(live(page)).toHaveText('The update is ready for review.');
      await page.evaluate(id => {
        (window as any).__streamHoldMessages = id;
        (window as any).__streamFixtures[id].done();
      }, id);
      await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseStreamMessages)).toBe('function');
      await expect(transcript(page).locator('.message-content, [data-commentary]').filter({ hasText: 'The update is ready for review.' })).toHaveCount(1);
      await page.evaluate(() => { (window as any).__streamHoldMessages = null; (window as any).__releaseStreamMessages(); });
      await expect(prompt(page)).toBeEnabled();
      await expect(transcript(page).locator('.message-content, [data-commentary]').filter({ hasText: 'The update is ready for review.' })).toHaveCount(1);
    });

    test(`keeps the completed ${view} response readable and retries a failed history handoff`, async ({ page }) => {
      const id = await startTurn(page, view);
      await push(page, id, 'The completed response must remain readable.');
      await expect(live(page)).toHaveText('The completed response must remain readable.');
      await page.evaluate(id => {
        (window as any).__streamFailMessages = id;
        (window as any).__streamFixtures[id].done();
      }, id);
      const retry = page.getByRole('button', { name: 'Retry loading response', exact: true });
      await expect(retry).toBeVisible();
      // A recorded turn boundary can move the final text into the completed
      // transcript before the authoritative history handoff succeeds.
      const retained = transcript(page).locator('.agent-message .message-content').filter({ hasText: 'The completed response must remain readable.' });
      await expect(retained).toHaveCount(1);
      await expect(retained).toBeVisible();
      await expect(retained).toHaveText('The completed response must remain readable.');
      await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeHidden();
      await expect(prompt(page)).toBeDisabled();
      await page.evaluate(() => { (window as any).__streamFailMessages = null; });
      await retry.click();
      await expect(retry).toBeHidden();
      await expect(prompt(page)).toBeEnabled();
      await expect(transcript(page).locator('.agent-message')).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message .message-content')).toHaveText('The completed response must remain readable.');
    });

    test(`keeps the ${view} response intact when a running tool sends late output and completes`, async ({ page }) => {
      const id = await startTurn(page, view);
      await page.evaluate(id => (window as any).__streamFixtures[id].startTool(), id);
      await push(page, id, 'The response began while the command was running. ');
      await expect(live(page)).toHaveText('The response began while the command was running.');
      await page.evaluate(id => (window as any).__streamFixtures[id].updateTool('running'), id);
      await expect(live(page)).toBeVisible();
      await expect(live(page)).toContainText('The response began while the command was running.');
      await page.evaluate(id => (window as any).__streamFixtures[id].updateTool('completed'), id);
      await expect(live(page)).toBeVisible();
      await expect(live(page)).toContainText('The response began while the command was running.');
      await push(page, id, 'The same response continues after it completes.');
      await finish(page, id);
      await expect(transcript(page).locator('.agent-message')).toHaveCount(1);
      await expect(transcript(page).locator('.agent-message .message-content')).toHaveText('The response began while the command was running. The same response continues after it completes.');
    });
  }

  test('keeps receiving text when switching views and tasks without mixing their responses', async ({ page }) => {
    const id = await startTurn(page, 'conversation');
    await push(page, id, 'First stream segment. ');
    await expect(live(page)).toContainText('First stream segment.');
    await page.getByRole('button', { name: 'Board view', exact: true }).click();
    await push(page, id, 'Arrived with the panel closed. ');
    await page.locator(`[data-action="board-open"][data-id="${id}"]`).click();
    await expect(live(page)).toContainText('First stream segment. Arrived with the panel closed.');
    await page.getByRole('button', { name: 'Open task: Explore the project', exact: true }).click();
    await push(page, id, 'Arrived in the background. ');
    await expect(transcript(page)).not.toContainText('First stream segment.');
    await expect(transcript(page)).toContainText('The project is a small native workspace');
    await page.getByRole('button', { name: 'Conversation view', exact: true }).click();
    await page.locator(`.task-item[data-id="${id}"]`).click();
    await expect(live(page)).toContainText('First stream segment. Arrived with the panel closed. Arrived in the background.');
    await push(page, id, 'Final segment.');
    await finish(page, id);
    await expect(transcript(page).locator('.agent-message')).toHaveCount(1);
    await expect(transcript(page).locator('.agent-message .message-content')).toHaveText('First stream segment. Arrived with the panel closed. Arrived in the background. Final segment.');
  });

  test('shows streamed Board comments around tool work and finalizes each comment once', async ({ page }) => {
    const id = await startTurn(page, 'board');
    await push(page, id, 'Inspecting the project.');
    await expect(live(page)).toHaveText('Inspecting the project.');
    await page.evaluate(id => (window as any).__streamFixtures[id].tool(), id);
    await expect(live(page)).toBeVisible();
    await expect(live(page)).toHaveText('Inspecting the project.');
    await expect(transcript(page).locator('[data-commentary]')).toHaveCount(0);
    await push(page, id, 'Found **one** issue.');
    await expect(live(page).locator('strong')).toHaveText('one');
    await expect(transcript(page).locator('[data-commentary]')).toHaveCount(1);
    await expect(transcript(page).locator('[data-commentary]')).toContainText('Inspecting the project.');
    await expect(transcript(page).locator('[data-commentary]')).toBeVisible();
    await finish(page, id);
    await expect(transcript(page).locator('.agent-message')).toHaveCount(2);
    await expect(transcript(page).locator('.agent-message .message-content')).toHaveText(['Inspecting the project.', 'Found one issue.']);
  });

  test('sanitizes untrusted streamed Markdown before completion and keeps external links isolated', async ({ page }) => {
    const id = await startTurn(page, 'conversation');
    // Raw HTML is never interpreted (the Markdown engine keeps it as text); Markdown links are.
    await push(page, id, '<img src="x" onerror="window.__unsafeStream = true"><button data-action="archive-task">Archive task</button><input id="prompt" name="title"><a id="dialog" data-action="archive-task" href="javascript:window.__unsafeStream=true">Unsafe link</a><script>window.__unsafeStream = true</script>\n\n[Safe link](https://example.com) and [Unsafe markdown](javascript:window.__unsafeStream=true)');
    await expect(live(page)).toContainText('<script>window.__unsafeStream = true</script>');
    const safeLink = live(page).getByRole('link', { name: 'Safe link', exact: true });
    await expect(safeLink).toBeVisible();
    await expect(safeLink).toHaveAttribute('href', 'https://example.com');
    await expect(safeLink).toHaveAttribute('target', '_blank');
    await expect(safeLink).toHaveAttribute('rel', 'noopener noreferrer');
    await expect(live(page).locator('img,script,button,input,form,[data-action],[id],[name],[onclick],[onerror]')).toHaveCount(0);
    await expect(live(page).locator('a[href^="javascript"]')).toHaveCount(0);
    expect(await page.evaluate(() => (window as any).__unsafeStream)).toBeUndefined();
    await expect(page.getByRole('button', { name: 'Stop task', exact: true })).toBeVisible();
    await finish(page, id);
    await expect(transcript(page).locator('.agent-message .message-content').getByRole('link', { name: 'Safe link', exact: true })).toBeVisible();
    expect(await page.evaluate(() => (window as any).__unsafeStream)).toBeUndefined();
  });
});

test('response file links resolve in batches, preserve web links and remain steady while streaming', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.task-item.selected .task-title')).toHaveText('Make search feel instant');
  await page.evaluate(async () => {
    // Vite may keep a timestamped dependency after another test edits the app.
    const source = await (await fetch('/src/dom.ts')).text();
    const files = await import(source.match(/from ["']([^"']*file-links\.ts[^"']*)["']/)![1]);
    const { streamMarkdown } = await import('/src/streaming-markdown.ts');
    const fixture = (window as any).__fileLinks = { calls: [] as any[], clicks: [] as any[], current: { projectId: 'alpha', sessionId: 'task-one' } };
    files.configureFileLinks({
      context: () => fixture.current,
      resolve: async (context: any, paths: string[]) => {
        fixture.calls.push({ context, paths });
        return paths.filter(path => ['index.html', 'logo.svg', 'src/app.ts', '/workspace/index.html', 'docs/design notes.md'].includes(path)).map(input => ({ input, path: input, previewable: /\.(html|svg)$/.test(input) }));
      },
      open: (context: any, file: any) => { fixture.clicks.push({ context, file }); },
    });
    const target = document.createElement('div'); target.id = 'file-link-fixture'; document.body.append(target);
    fixture.target = target; fixture.stream = streamMarkdown;
    fixture.value = 'Open `index.html`, logo.svg and [source](src/app.ts:12), [local page](file:///workspace/index.html) and [design notes](docs/design%20notes.md). Keep [the docs](https://example.com/guide.html) and missing.html.\n\n```js\nconst file = "index.html";\n```';
    streamMarkdown(target, fixture.value);
  });
  const output = page.locator('#file-link-fixture');
  await expect(output.locator('.workspace-file-link')).toHaveCount(5);
  await expect(output.locator('a[href="https://example.com/guide.html"]')).toHaveAttribute('target', '_blank');
  await expect(output.locator('pre .workspace-file-link')).toHaveCount(0);
  await expect(output).toContainText('missing.html');
  await page.evaluate(() => {
    const f = (window as any).__fileLinks;
    f.link = f.target.querySelector('.workspace-file-link');
    f.current = { projectId: 'beta', sessionId: 'task-two' };
    for (let i = 0; i < 20; i++) f.stream(f.target, f.value + '\n\nMore text ' + i);
  });
  await expect(output).toContainText('More text 19');
  expect(await page.evaluate(() => (window as any).__fileLinks.link === document.querySelector('#file-link-fixture .workspace-file-link'))).toBe(true);
  await output.locator('.workspace-file-link').first().click();
  const result = await page.evaluate(() => ({ calls: (window as any).__fileLinks.calls, clicks: (window as any).__fileLinks.clicks }));
  expect(result.calls).toHaveLength(1);
  expect(result.calls[0].context).toEqual({ projectId: 'alpha', sessionId: 'task-one' });
  expect(result.clicks[0].context).toEqual({ projectId: 'alpha', sessionId: 'task-one' });
  expect(result.clicks[0].file.path).toBe('index.html');
});

test('Markdown local links stay inert until resolution and stale file resolutions cannot relink a reused task', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.task-item.selected .task-title')).toHaveText('Make search feel instant');
  await page.evaluate(async () => {
    // Vite may keep a timestamped dependency after another test edits the app.
    const source = await (await fetch('/src/dom.ts')).text();
    const files = await import(source.match(/from ["']([^"']*file-links\.ts[^"']*)["']/)![1]); const { markdown } = await import('/src/dom.ts');
    const target = document.createElement('div'); target.id = 'file-link-fixture'; document.body.append(target);
    const f = (window as any).__fileLinks = { target, files, markdown, calls: [] as any[], releases: [] as any[], clicks: [] as any[] };
    files.configureFileLinks({ context: () => ({ projectId: 'alpha', sessionId: 'one' }),
      resolve: (context: any, paths: string[]) => new Promise(resolve => { f.calls.push({ context, paths }); f.releases.push(() => resolve(paths.map(input => ({ input, path: input, previewable: true })))); }),
      open: (context: any, file: any) => { f.clicks.push({ context, file }); },
    });
    await markdown(target, '[Preview](index.html) [Unsafe](file:///etc/passwd) [Traversal](../secret.html) <a data-workspace-file="index.html" href="javascript:alert(1)">Forged</a>');
  });
  const output = page.locator('#file-link-fixture');
  await expect.poll(() => page.evaluate(() => (window as any).__fileLinks.calls.length)).toBe(1);
  await expect(output.locator('a[href^="file:"], a[href="index.html"], [data-workspace-file]')).toHaveCount(0);
  await page.evaluate(async () => {
    const f = (window as any).__fileLinks;
    f.files.bindFileLinkContext(f.target, { projectId: 'beta', sessionId: 'two' });
    await f.markdown(f.target, 'Other thread logo.svg');
    f.releases[0]();
  });
  await expect(output.locator('.workspace-file-link')).toHaveCount(0);
  await expect(output).toHaveText('Other thread logo.svg');
  await expect.poll(() => page.evaluate(() => (window as any).__fileLinks.calls.length)).toBe(2);
  await page.evaluate(() => (window as any).__fileLinks.releases[1]());
  await expect(output.locator('.workspace-file-link')).toHaveCount(1);
  await output.locator('.workspace-file-link').click();
  expect(await page.evaluate(() => (window as any).__fileLinks.clicks[0].context)).toEqual({ projectId: 'beta', sessionId: 'two' });
});

test('identical streamed filenames relink to the newly selected worker context', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.task-item.selected .task-title')).toHaveText('Make search feel instant');
  await page.evaluate(async () => {
    // Vite may keep a timestamped dependency after another test edits the app.
    const source = await (await fetch('/src/dom.ts')).text();
    const files = await import(source.match(/from ["']([^"']*file-links\.ts[^"']*)["']/)![1]); const { streamMarkdown } = await import('/src/streaming-markdown.ts');
    const target = document.createElement('div'); target.id = 'file-link-fixture'; document.body.append(target);
    const f = (window as any).__fileLinks = { target, files, streamMarkdown, clicks: [] as any[] };
    files.configureFileLinks({ context: () => ({ projectId: 'alpha', sessionId: 'worker-one' }),
      resolve: async (_: any, paths: string[]) => paths.map(input => ({ input, path: input, previewable: true })),
      open: (context: any, file: any) => { f.clicks.push({ context, file }); },
    });
    streamMarkdown(target, 'Open `index.html`.');
  });
  const output = page.locator('#file-link-fixture');
  await expect(output.locator('.workspace-file-link')).toHaveCount(1);
  await page.evaluate(() => {
    const f = (window as any).__fileLinks;
    f.files.bindFileLinkContext(f.target, { projectId: 'alpha', sessionId: 'worker-two' });
    f.streamMarkdown(f.target, 'Open `index.html`.');
  });
  await expect(output.locator('.workspace-file-link')).toHaveCount(1);
  await output.locator('.workspace-file-link').click();
  expect(await page.evaluate(() => (window as any).__fileLinks.clicks[0].context.sessionId)).toBe('worker-two');
});

test('completion refresh links a file that the agent mentioned before creating it', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.task-item.selected .task-title')).toHaveText('Make search feel instant');
  await page.evaluate(async () => {
    const source = await (await fetch('/src/dom.ts')).text();
    const files = await import(source.match(/from ["']([^"']*file-links\.ts[^"']*)["']/)![1]);
    const { markdown } = await import('/src/dom.ts');
    const target = document.createElement('div'); target.id = 'file-link-fixture'; document.body.append(target);
    const f = (window as any).__fileLinks = { files, target, exists: false, calls: 0, context: { projectId: 'alpha', sessionId: 'one' } };
    files.configureFileLinks({ context: () => f.context,
      resolve: async (_: any, paths: string[]) => { f.calls++; return f.exists ? paths.map(input => ({ input, path: input, previewable: true })) : []; },
      open: () => {},
    });
    await markdown(target, 'I am creating `index.html`.');
  });
  await expect.poll(() => page.evaluate(() => (window as any).__fileLinks.calls)).toBe(1);
  await expect(page.locator('#file-link-fixture .workspace-file-link')).toHaveCount(0);
  await page.evaluate(() => {
    const f = (window as any).__fileLinks; f.exists = true; f.files.refreshFileLinks(f.context);
  });
  await expect(page.locator('#file-link-fixture .workspace-file-link')).toHaveCount(1);
  expect(await page.evaluate(() => (window as any).__fileLinks.calls)).toBe(2);
});
