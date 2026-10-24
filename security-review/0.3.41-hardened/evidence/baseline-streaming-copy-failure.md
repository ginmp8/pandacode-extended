# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: message-actions.spec.ts >> conversation message actions >> copies the latest streaming response without replacing its controls or offering an unsaved fork
- Location: ../references/baseline/tests/message-actions.spec.ts:242:3

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: locator.click: Test timeout of 30000ms exceeded.
Call log:
  - waiting for getByRole('log', { name: 'Conversation', exact: true }).locator('[data-message-action="copy"][data-message-key^="commentary:"]')
    - locator resolved to <button type="button" title="Copy message" aria-label="Copy message" data-message-action="copy" data-message-session="session-2" data-message-key="commentary:commentary:-1">…</button>
  - attempting click action
    2 × waiting for element to be visible, enabled and stable
      - element is visible, enabled and stable
      - scrolling into view if needed
      - done scrolling
      - <summary class="tool-group-summary">…</summary> from <details class="tool-group" data-stream-key="group:tool:copy-live-tool" data-tool-group-key="group:tool:copy-live-tool">…</details> subtree intercepts pointer events
    - retrying click action
    - waiting 20ms
    2 × waiting for element to be visible, enabled and stable
      - element is visible, enabled and stable
      - scrolling into view if needed
      - done scrolling
      - <summary class="tool-group-summary">…</summary> from <details class="tool-group" data-stream-key="group:tool:copy-live-tool" data-tool-group-key="group:tool:copy-live-tool">…</details> subtree intercepts pointer events
    - retrying click action
      - waiting 100ms
    53 × waiting for element to be visible, enabled and stable
       - element is visible, enabled and stable
       - scrolling into view if needed
       - done scrolling
       - <summary class="tool-group-summary">…</summary> from <details class="tool-group" data-stream-key="group:tool:copy-live-tool" data-tool-group-key="group:tool:copy-live-tool">…</details> subtree intercepts pointer events
     - retrying click action
       - waiting 500ms
    - waiting for element to be visible, enabled and stable
    - element is visible, enabled and stable
    - scrolling into view if needed
    - done scrolling

```

# Page snapshot

```yaml
- generic [ref=e3]:
  - complementary "Projects and tasks" [ref=e4]:
    - generic [ref=e5]:
      - button "Hide sidebar" [expanded] [ref=e6] [cursor=pointer]
      - generic [ref=e7]: PandaCode
    - generic [ref=e8]:
      - button "New task Ctrl N" [ref=e9] [cursor=pointer]:
        - generic [ref=e10]: New task
        - generic [ref=e11]: Ctrl N
      - button "New chat" [ref=e12] [cursor=pointer]
      - button "Search Ctrl K" [ref=e14] [cursor=pointer]:
        - generic [ref=e15]: Search
        - generic [ref=e16]: Ctrl K
    - generic [ref=e17]:
      - button "Filter projects" [ref=e18] [cursor=pointer]:
        - generic [ref=e19]: All projects
      - button "Import conversations" [ref=e20] [cursor=pointer]
      - button "Open project" [ref=e21] [cursor=pointer]
    - generic [ref=e22]:
      - region "orbit tasks" [ref=e23]:
        - generic [ref=e24]:
          - button "orbit" [expanded] [ref=e25] [cursor=pointer]
          - button "New task in orbit" [ref=e28] [cursor=pointer]
        - generic [ref=e29]:
          - button "Message action fixture Worktree orbit Running 28s" [ref=e30]:
            - generic [ref=e31]:
              - generic [ref=e32]: Message action fixture
              - img "Worktree" [ref=e33]
              - generic [ref=e34]: orbit
              - img "Running" [ref=e35]
              - generic "Working for 28s" [ref=e36]: 28s
          - button "Make search feel instant orbit 5m" [ref=e37]:
            - generic [ref=e38]:
              - generic [ref=e39]: Make search feel instant
              - generic [ref=e40]: orbit
              - 'generic "Last message: 10/25/2026, 9:42:52 AM" [ref=e41]': 5m
          - 'button "Add keyboard shortcuts #131 orbit" [ref=e42]':
            - generic [ref=e43]:
              - generic [ref=e44]: Add keyboard shortcuts
              - 'generic "Issue #131: Keyboard shortcut cheat sheet" [ref=e45]': "#131"
              - generic [ref=e46]: orbit
      - region "pandacode tasks" [ref=e47]:
        - generic [ref=e48]:
          - button "pandacode" [expanded] [ref=e49] [cursor=pointer]:
            - generic [aria-hidden] [ref=e50]: P
          - button "New task in pandacode" [ref=e52] [cursor=pointer]
        - button "Explore the project pandacode 3d" [ref=e54]:
          - generic [ref=e55]:
            - generic [ref=e56]: Explore the project
            - generic [ref=e57]: pandacode
            - 'generic "Last message: 10/22/2026, 9:47:52 AM" [ref=e58]': 3d
    - button "Bots" [ref=e60] [cursor=pointer]
    - generic [ref=e62]:
      - button "Help & support" [ref=e63] [cursor=pointer]
      - button "Agent usage" [ref=e64] [cursor=pointer]
      - button "Review & coordination" [ref=e65] [cursor=pointer]
      - button "Archived tasks" [ref=e66] [cursor=pointer]
      - button "Settings" [ref=e67] [cursor=pointer]
      - button "Dark mode" [pressed] [ref=e68] [cursor=pointer]
    - separator "Resize sidebar" [ref=e69]
  - main [ref=e70]:
    - generic [ref=e71]:
      - button "orbit" [ref=e76] [cursor=pointer]
      - generic [ref=e77]:
        - generic [ref=e78]: Browser preview · sample data
        - group "Workspace view" [ref=e79]:
          - button "Conversation view" [pressed] [ref=e80] [cursor=pointer]:
            - generic [ref=e81]: Conversation
          - button "Board view" [ref=e82] [cursor=pointer]:
            - generic [ref=e83]: Board
          - button "GitHub view" [ref=e84] [cursor=pointer]:
            - generic [ref=e85]: GitHub
        - button "Deploy" [ref=e86] [cursor=pointer]
        - button "Project moodboards" [ref=e88] [cursor=pointer]
        - button "Open browser" [ref=e89] [cursor=pointer]
        - button "Toggle terminal (Ctrl J)" [ref=e90] [cursor=pointer]
        - button "Toggle inspector (Ctrl Shift B)" [ref=e92] [cursor=pointer]
        - button "Task actions" [ref=e93] [cursor=pointer]
    - generic [ref=e95]:
      - log "Conversation" [ref=e96]:
        - article [ref=e97]:
          - generic [ref=e98]:
            - generic [ref=e99]: "Y"
            - strong [ref=e100]: You
            - time [ref=e101]: 10:01 AM
          - generic [ref=e102]: "Keep `main.cpp` in the reply. Do not create a file yet."
          - group "Message actions" [ref=e103]:
            - button "Copy message" [ref=e104] [cursor=pointer]
            - button "Fork from here" [ref=e105] [cursor=pointer]
            - status [ref=e106]
        - group [ref=e107]:
          - generic "Worked for about 0s" [active] [ref=e108] [cursor=pointer]
          - generic [ref=e112]:
            - article [ref=e113]:
              - paragraph [ref=e115]:
                - text: I’ll check
                - strong [ref=e116]: the interface
                - text: before making changes.
              - group "Message actions" [ref=e117]:
                - button "Copy message" [ref=e118] [cursor=pointer]
                - button "Fork from here" [ref=e119] [cursor=pointer]
                - status [ref=e120]
            - group [ref=e121]:
              - generic "Used 1 tool" [ref=e122] [cursor=pointer]
        - article [ref=e129]:
          - generic [ref=e130]:
            - strong [ref=e132]: Codex
            - time [ref=e133]: 10:06 AM
          - generic [ref=e134]:
            - paragraph [ref=e135]: "Here is the code:"
            - generic [ref=e136]:
              - code [ref=e138]: "#include <iostream> int main() { std::cout << \"Hello\"; }"
              - button "Copy code" [ref=e139] [cursor=pointer]
            - paragraph [ref=e143]:
              - link "Reference" [ref=e144] [cursor=pointer]:
                - /url: https://example.com/guide
          - group "Message actions" [ref=e145]:
            - button "Copy message" [ref=e146] [cursor=pointer]
            - button "Fork from here" [ref=e147] [cursor=pointer]
            - status [ref=e148]
        - article [ref=e149]:
          - generic [ref=e150]:
            - generic [ref=e151]: "Y"
            - strong [ref=e152]: You
            - time [ref=e153]: 10:08 AM
          - generic [ref=e154]: Later request must not enter an earlier fork.
          - group "Message actions" [ref=e155]:
            - button "Copy message" [ref=e156] [cursor=pointer]
            - button "Fork from here" [ref=e157] [cursor=pointer]
            - status [ref=e158]
        - article [ref=e159]:
          - generic [ref=e160]:
            - strong [ref=e162]: Codex
            - time [ref=e163]: 10:10 AM
          - paragraph [ref=e165]: Later response must stay in the source.
          - group "Message actions" [ref=e166]:
            - button "Copy message" [ref=e167] [cursor=pointer]
            - button "Fork from here" [ref=e168] [cursor=pointer]
            - status [ref=e169]
        - generic [ref=e170]:
          - group [ref=e171]:
            - generic "Generating response · 28s" [ref=e172] [cursor=pointer]
            - generic [ref=e176]:
              - generic [ref=e177]:
                - article [ref=e178]:
                  - paragraph [ref=e180]:
                    - text: Reading
                    - strong [ref=e181]: the files
                    - text: .
                  - group "Message actions" [ref=e182]:
                    - button "Copy message" [ref=e183] [cursor=pointer]
                    - status [ref=e184]
                - group [ref=e185]:
                  - generic "Read 1 file" [ref=e186] [cursor=pointer]
              - generic [ref=e194]: Generating response
          - article [ref=e195]:
            - generic [ref=e196]:
              - strong [ref=e198]: Codex
              - generic [ref=e199]: Generating
            - paragraph [ref=e201]:
              - text: New
              - strong [ref=e202]: response
              - text: with
              - code [ref=e203]: code
              - text: .
            - group "Message actions" [ref=e204]:
              - button "Copy message" [ref=e205] [cursor=pointer]
              - status [ref=e206]
      - region "Task goal" [ref=e207]:
        - button "Manage task goal" [ref=e208] [cursor=pointer]:
          - generic [ref=e210]:
            - strong [ref=e211]: Source goal
            - generic [ref=e212]: Goal completed · 1,200 tokens · 12s
      - generic [ref=e213]:
        - generic [ref=e214]:
          - generic "/Users/you/Projects/orbit" [ref=e215]: orbit
          - generic "/sample/worktrees/message-actions" [ref=e219]: Worktree
          - button "Branch and worktree actions" [ref=e224] [cursor=pointer]:
            - generic [ref=e225]: feat/instant-search
        - generic [ref=e226]:
          - generic "Browser attachments"
          - textbox "Message your coding agent" [ref=e227]:
            - /placeholder: Queue a follow-up · ⌘↩ steers now
          - generic [ref=e228]:
            - generic [ref=e229]:
              - button "Attach files for context" [ref=e230] [cursor=pointer]
              - button "Choose harness and model" [disabled] [ref=e231]:
                - generic [ref=e233]: GPT-6-Astra · High
              - button "Standard" [disabled] [ref=e235]
              - button "Team" [ref=e237] [cursor=pointer]
              - 'button "Token saver: off" [ref=e239] [cursor=pointer]':
                - generic [ref=e240]: Saver
            - button "Steer the running turn" [ref=e241] [cursor=pointer]:
              - generic [ref=e242]: Steer
            - button "Stop task" [ref=e243] [cursor=pointer]
        - generic "400,000 token context window reported by the harness." [ref=e245]: 400K context
  - complementary "Project inspector" [ref=e246]:
    - navigation "Panel views" [ref=e247]:
      - button "Files" [ref=e248] [cursor=pointer]
      - button "Search" [ref=e249] [cursor=pointer]
      - button "Changes, 3 changed files" [ref=e250] [cursor=pointer]:
        - generic [ref=e251]: "3"
      - button "Browser" [ref=e252] [cursor=pointer]
      - button "Design" [ref=e253] [cursor=pointer]
    - generic [ref=e254]:
      - heading "Files" [level=2] [ref=e255]
      - button "Refresh project" [ref=e256] [cursor=pointer]
    - generic [ref=e257]:
      - generic [ref=e258]: orbit
      - generic [ref=e260]:
        - button "src" [ref=e262] [cursor=pointer]
        - button "public" [ref=e265] [cursor=pointer]
        - generic [ref=e267]:
          - button "index.html" [ref=e268] [cursor=pointer]
          - button "View source of index.html" [ref=e270] [cursor=pointer]
          - button "Attach index.html" [ref=e271] [cursor=pointer]
        - generic [ref=e272]:
          - button "package.json" [ref=e273] [cursor=pointer]
          - button "Attach package.json" [ref=e275] [cursor=pointer]
        - generic [ref=e276]:
          - button "README.md" [ref=e277] [cursor=pointer]
          - button "Attach README.md" [ref=e279] [cursor=pointer]
        - generic [ref=e280]:
          - button "tsconfig.json" [ref=e281] [cursor=pointer]
          - button "Attach tsconfig.json" [ref=e283] [cursor=pointer]
    - separator "Resize right sidebar" [ref=e284]
```

# Test source

```ts
  169 |     expect(await calls(page, 'fork_session_at_message')).toEqual([{ command: 'fork_session_at_message', args: { sessionId, messageId: 204 } }]);
  170 |   });
  171 | 
  172 |   test('starts a ClipboardItem write during the click and copies full stored text after the database responds', async ({ page }) => {
  173 |     await install(page);
  174 |     const fullContent = answer + '\n\n' + 'Long stored response. '.repeat(14000) + 'FULL_COPY_TAIL';
  175 |     await page.evaluate(fullContent => {
  176 |       const fixture = (window as any).__messageActionsTest;
  177 |       fixture.setContent(204, fullContent); fixture.holdContent = true;
  178 |       (window as any).__clipboardWriteStarted = false;
  179 |       Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
  180 |         write: async (items: ClipboardItem[]) => {
  181 |           (window as any).__clipboardWriteStarted = true;
  182 |           const blob = await items[0].getType('text/plain');
  183 |           (window as any).__clipboardWrites.push(await blob.text());
  184 |         },
  185 |       } });
  186 |     }, fullContent);
  187 |     const copy = action(page, 204, 'copy'); await copy.click();
  188 |     await expect.poll(() => page.evaluate(() => typeof (window as any).__messageActionsTest.releaseContent)).toBe('function');
  189 |     expect(await page.evaluate(() => (window as any).__clipboardWriteStarted)).toBe(true);
  190 |     expect(await page.evaluate(() => (window as any).__clipboardWrites)).toEqual([]);
  191 |     await expect(copy).toBeDisabled();
  192 |     await page.evaluate(() => (window as any).__messageActionsTest.releaseContent());
  193 |     await expect.poll(() => page.evaluate(() => (window as any).__clipboardWrites.at(-1)?.length)).toBe(fullContent.length);
  194 |     expect(await page.evaluate(() => (window as any).__clipboardWrites.at(-1))).toBe(fullContent);
  195 |     await expect(copy).toBeEnabled();
  196 |     await expect(copy).toHaveAttribute('aria-label', 'Copied');
  197 |     expect(await calls(page, 'get_message_content')).toEqual([{ command: 'get_message_content', args: { sessionId, messageId: 204 } }]);
  198 |   });
  199 | 
  200 |   test('uses a clipboard fallback when permission is denied and reports failure if no copy method succeeds', async ({ page }) => {
  201 |     await install(page);
  202 |     await page.evaluate(() => {
  203 |       Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
  204 |         writeText: async () => { throw new DOMException('Denied', 'NotAllowedError'); },
  205 |       } });
  206 |       Object.defineProperty(document, 'execCommand', { configurable: true, value: (command: string) => {
  207 |         if (command !== 'copy' || (window as any).__failFallback) return false;
  208 |         (window as any).__clipboardWrites.push((document.activeElement as HTMLTextAreaElement).value);
  209 |         return true;
  210 |       } });
  211 |     });
  212 |     const copy = action(page, 204, 'copy'); await copy.focus(); await page.keyboard.press('Enter');
  213 |     await expect.poll(() => page.evaluate(() => (window as any).__clipboardWrites.at(-1))).toBe(answer);
  214 |     await expect(copy).toBeFocused();
  215 |     await expect(copy).toHaveAttribute('aria-label', 'Copied');
  216 |     await page.evaluate(() => { (window as any).__failFallback = true; });
  217 |     const failedCopy = action(page, 206, 'copy'); await failedCopy.click();
  218 |     await expect(page.locator('#toast')).toContainText('Could not copy this message.');
  219 |     await expect(failedCopy).toBeEnabled();
  220 |     await expect(failedCopy).toHaveAttribute('aria-label', 'Copy message');
  221 |     expect(await page.evaluate(() => (window as any).__clipboardWrites)).toEqual([answer]);
  222 |     await expect(page.locator('textarea[readonly]')).toHaveCount(0);
  223 |   });
  224 | 
  225 |   test('forks at the first user message inside the board without opening the general conversation view', async ({ page }) => {
  226 |     await install(page);
  227 |     await page.getByRole('button', { name: 'Board view', exact: true }).click();
  228 |     const board = page.getByRole('region', { name: 'Project board', exact: true });
  229 |     await board.getByRole('button', { name: 'Open task: Message action fixture', exact: true }).click();
  230 |     await expect(action(page, 201, 'fork')).toBeVisible();
  231 |     await page.screenshot({ path: 'test-results/pandacode-message-actions-board.png', animations: 'disabled' });
  232 |     await action(page, 201, 'fork').click();
  233 |     const forkId = await selectedFork(page), fork = await snapshot(page, forkId);
  234 |     await expect(board).toBeVisible();
  235 |     await expect(page.getByRole('region', { name: 'Task details', exact: true })).toBeVisible();
  236 |     expect(fork.messages.map(message => [message.role, message.content])).toEqual([['user', request]]);
  237 |     await expect(transcript(page)).not.toContainText('Here is the code:');
  238 |     expect(await calls(page, 'send_message')).toEqual([]);
  239 |     expect(await calls(page, 'start_task')).toEqual([]);
  240 |   });
  241 | 
  242 |   test('copies the latest streaming response without replacing its controls or offering an unsaved fork', async ({ page }) => {
  243 |     await install(page);
  244 |     const initial = 'Reading **the files**.';
  245 |     await page.evaluate(({ sessionId, initial }) => {
  246 |       const fixture = (window as any).__messageActionsTest;
  247 |       fixture.emit({ sessionId, kind: 'status', text: 'Starting agent' });
  248 |       fixture.emit({ sessionId, kind: 'text', text: initial });
  249 |     }, { sessionId, initial });
  250 |     const live = page.locator('#stream-region .live-message');
  251 |     await expect(live).toBeVisible();
  252 |     await expect(live.locator('[data-message-action="fork"]')).toHaveCount(0);
  253 |     await action(page, 'live', 'copy').click();
  254 |     await expect.poll(() => page.evaluate(() => (window as any).__clipboardWrites.at(-1))).toBe(initial);
  255 |     await page.evaluate(() => { (window as any).__liveCopyNode = document.querySelector('.live-message [data-message-action="copy"]'); });
  256 |     await page.evaluate(sessionId => {
  257 |       const fixture = (window as any).__messageActionsTest;
  258 |       fixture.emit({ sessionId, kind: 'tool', text: 'Read', tool: { id: 'copy-live-tool', name: 'Read', status: 'completed', input: 'README.md', output: 'Read complete.' } });
  259 |       fixture.emit({ sessionId, kind: 'message_start', text: '' });
  260 |       fixture.emit({ sessionId, kind: 'text', text: 'New **response** with `code`.' });
  261 |     }, sessionId);
  262 |     await expect(live.locator('.live-text')).toHaveText('New response with code.');
  263 |     expect(await page.evaluate(() => (window as any).__liveCopyNode === document.querySelector('.live-message [data-message-action="copy"]'))).toBe(true);
  264 |     await action(page, 'live', 'copy').click();
  265 |     await expect.poll(() => page.evaluate(() => (window as any).__clipboardWrites.at(-1))).toBe('New **response** with `code`.');
  266 |     const earlier = transcript(page).locator('[data-message-action="copy"][data-message-key^="commentary:"]');
  267 |     await expect(earlier).toHaveCount(1);
  268 |     await revealHistory(page);
> 269 |     await earlier.click();
      |                   ^ Error: locator.click: Test timeout of 30000ms exceeded.
  270 |     await expect.poll(() => page.evaluate(() => (window as any).__clipboardWrites.at(-1))).toBe(initial);
  271 |     expect(await calls(page, 'fork_session_at_message')).toEqual([]);
  272 |   });
  273 | 
  274 |   test('keeps the source on a failed fork and prevents duplicate requests while a retry is pending', async ({ page }) => {
  275 |     await install(page); const original = await snapshot(page);
  276 |     await page.evaluate(() => { (window as any).__messageActionsTest.failFork = true; });
  277 |     await action(page, 204, 'fork').click();
  278 |     await expect(page.locator('#toast')).toContainText('Could not save this fork. Try again.');
  279 |     await expect(prompt(page)).toHaveAttribute('data-session-id', sessionId);
  280 |     expect(await snapshot(page)).toEqual(original);
  281 |     await page.evaluate(() => {
  282 |       const fixture = (window as any).__messageActionsTest;
  283 |       fixture.failFork = false; fixture.holdFork = true;
  284 |     });
  285 |     const fork = action(page, 204, 'fork');
  286 |     await fork.click();
  287 |     await expect.poll(() => page.evaluate(() => typeof (window as any).__messageActionsTest.releaseFork)).toBe('function');
  288 |     await expect(fork).toBeDisabled();
  289 |     await fork.evaluate(button => button.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  290 |     expect(await calls(page, 'fork_session_at_message')).toHaveLength(2);
  291 |     await page.evaluate(() => (window as any).__messageActionsTest.releaseFork());
  292 |     await selectedFork(page);
  293 |     expect(await calls(page, 'fork_session_at_message')).toHaveLength(2);
  294 |     expect(await snapshot(page)).toEqual(original);
  295 |   });
  296 | });
  297 | 
  298 | // Your messages sit flush with the right edge of the transcript column, at any window width.
  299 | for (const width of [1600, 1000]) {
  300 |   test(`user bubbles align to the column's right edge at ${width}px`, async ({ page }) => {
  301 |     await page.setViewportSize({ width, height: 900 });
  302 |     await page.goto('/');
  303 |     await expect(page.locator('.task-item.selected')).toHaveAttribute('data-id', 'session-1');
  304 |     const bubble = page.locator('#transcript .user-message').first();
  305 |     await expect(bubble).toBeVisible();
  306 |     const edges = await page.evaluate(() => {
  307 |       const transcript = document.querySelector('#transcript')!;
  308 |       const agent = transcript.querySelector('.agent-message')!.getBoundingClientRect();
  309 |       const user = transcript.querySelector('.user-message')!.getBoundingClientRect();
  310 |       return { agentRight: agent.right, userRight: user.right, userLeft: user.left, agentLeft: agent.left };
  311 |     });
  312 |     expect(Math.abs(edges.userRight - edges.agentRight)).toBeLessThanOrEqual(1);
  313 |     expect(edges.userLeft).toBeGreaterThan(edges.agentLeft);
  314 |     // Bubble text reads left to right, and "Worked for" starts at the column edge like replies.
  315 |     expect(await bubble.locator('.message-content').evaluate(el => getComputedStyle(el).textAlign)).toBe('left');
  316 |     const activity = page.locator('#transcript>.turn-activity').first();
  317 |     const [box, summary] = await Promise.all([activity.boundingBox(), activity.locator('.turn-activity-summary').boundingBox()]);
  318 |     expect(Math.abs(summary!.x - box!.x)).toBeLessThanOrEqual(1);
  319 |     // The live "Working…" row of a running turn starts there too.
  320 |     await page.getByRole('textbox', { name: 'Message your coding agent', exact: true }).fill('Keep going.');
  321 |     await page.keyboard.press('Enter');
  322 |     const live = page.locator('#stream-region>.turn-activity');
  323 |     await expect(live).toBeVisible();
  324 |     const [liveBox, liveSummary, column] = await Promise.all([live.boundingBox(), live.locator('.turn-activity-summary').boundingBox(), activity.boundingBox()]);
  325 |     expect(Math.abs(liveSummary!.x - liveBox!.x)).toBeLessThanOrEqual(1);
  326 |     expect(Math.abs(liveSummary!.x - column!.x)).toBeLessThanOrEqual(1);
  327 |   });
  328 | }
  329 | 
```