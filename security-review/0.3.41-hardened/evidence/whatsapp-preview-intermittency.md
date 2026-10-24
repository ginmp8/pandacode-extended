# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bots.spec.ts >> bots >> WhatsApp setup ends with copyable webhook details built from the tunnel address
- Location: tests/bots.spec.ts:317:3

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator: getByRole('dialog', { name: 'Scout settings' }).locator('.bots-channel .bots-guide').locator('code').filter({ hasText: '/whatsapp/' })
Expected pattern: /^https:\/\/quiet-river\.trycloudflare\.com\/whatsapp\/[0-9a-f-]{36}$/
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toHaveText" getByRole('dialog', { name: 'Scout settings' }).locator('.bots-channel .bots-guide').locator('code').filter({ hasText: '/whatsapp/' }) with timeout 5000ms
  - waiting for getByRole('dialog', { name: 'Scout settings' }).locator('.bots-channel .bots-guide').locator('code').filter({ hasText: '/whatsapp/' })

```

```yaml
- complementary "Projects and tasks":
  - button "Hide sidebar" [expanded]
  - text: PandaCode
  - region "Bots":
    - searchbox "Search bots"
    - button "Add bot"
    - list:
      - listitem:
        - button "Chat with Scout": Scout Digs into questions and reports back
        - button "Scout settings"
  - button "Bots 1" [pressed]
  - button "Help & support"
  - button "Agent usage"
  - button "Review & coordination"
  - button "Archived tasks"
  - button "Settings"
  - button "Dark mode" [pressed]
  - separator "Resize sidebar"
- main:
  - strong: Scout
  - text: Ready
  - button "Pause Scout"
  - button "Scout settings": Settings
  - text: Browser preview · sample data
  - group "Workspace view":
    - button "Conversation view" [pressed]: Conversation
    - button "Board view": Board
    - button "GitHub view": GitHub
  - button "Deploy"
  - button "Project moodboards"
  - button "Open browser"
  - button "Toggle terminal (Ctrl J)"
  - button "Toggle inspector (Ctrl Shift B)"
  - button "Task actions"
  - log "Conversation":
    - heading "Chat with Scout" [level=1]
    - paragraph: Digs into questions and reports back
    - button "What can you help me with?"
    - button "Remember that I prefer short, direct answers."
    - button "Every weekday at 9, send me a summary of what changed."
    - group "Set up Scout":
      - button "Instructions"
      - button "Memory"
      - button "Schedules"
      - button "Channels"
  - textbox "Message your coding agent":
    - /placeholder: Message Scout…
  - button "Attach files for context"
  - button "Choose harness and model": Claude Sonnet 5
  - button "Standard"
  - button "Team"
  - 'button "Token saver: off"': Saver
  - button "Send message"
  - text: 200K context
- complementary "Project inspector":
  - navigation "Panel views":
    - button "Files"
    - button "Search"
    - button "Changes, 3 changed files": "3"
    - button "Browser"
    - button "Design"
  - heading "Files" [level=2]
  - button "Refresh project"
  - text: Scout
  - button "src"
  - button "public"
  - button "index.html"
  - button "View source of index.html"
  - button "Attach index.html"
  - button "package.json"
  - button "Attach package.json"
  - button "README.md"
  - button "Attach README.md"
  - button "tsconfig.json"
  - button "Attach tsconfig.json"
  - separator "Resize right sidebar"
- dialog "Scout settings":
  - heading "Scout settings" [level=2]
  - button "Close dialog"
  - heading "Scout" [level=3]
  - text: Ready
  - paragraph: Digs into questions and reports backClaude Code
  - button "Open chat"
  - button "Pause"
  - button "More"
  - alert: Use the https:// address your tunnel or Tailscale shows, without a path or query.
  - tablist "Bot settings":
    - tab "Profile"
    - tab "Channels 1 waiting" [selected]: Channels 1
    - tab "Schedules"
    - tab "Memory"
    - tab "Activity"
  - tabpanel "Channels 1 waiting":
    - heading "Channels" [level=2]
    - paragraph: Talk to it from a browser, Telegram, Slack, Discord or WhatsApp.
    - list:
      - listitem:
        - text: WhatsApp Preview Business (+1 555 0100) Preview · not connected
        - checkbox "WhatsApp on" [checked]
        - button "Remove WhatsApp"
        - group:
          - text: Finish setting up
          - list:
            - listitem:
              - text: "Give Meta a way in. Meta delivers messages over HTTPS, so this computer needs a public address. The quickest is a free Cloudflare tunnel; run these in Terminal and keep the second one open: Install"
              - code: brew install cloudflared
              - button "Copy"
              - text: Start
              - code: cloudflared tunnel --url http://127.0.0.1:47321
              - button "Copy"
              - text: It prints an address ending in
              - code: trycloudflare.com
              - text: ". That address changes each time the tunnel starts; for a fixed one, use Tailscale Funnel instead: Or"
              - code: tailscale funnel --bg 47321
              - button "Copy"
            - listitem:
              - text: Paste that https address
              - textbox "Paste that https address Save":
                - /placeholder: https://….trycloudflare.com
                - text: http://not-secure.example
              - button "Save"
            - listitem:
              - text: "In Meta, open WhatsApp → Configuration → Webhook → Edit and paste: (the callback URL appears here after you save the address) Verify token"
              - code: pandacode-preview0123456789
              - button "Copy"
              - text: Press Verify and save, then under Webhook fields press Manage and subscribe to messages. The status above turns to “Webhook verified”.
            - listitem:
              - button "Message your WhatsApp number"
            - listitem: Send it any message. It replies with a four-digit code, which also appears under People below. Press Approve and you’re talking.
    - heading "People About pairing" [level=4]:
      - text: People
      - img "About pairing"
    - list:
      - listitem:
        - text: Sample sender WhatsApp · code
        - code: "4821"
        - button "Approve"
        - button "Block"
    - heading "Add another channel" [level=4]
    - group "Add a channel":
      - button "Web chat A private chat page on this computer Instant":
        - text: Web chat A private chat page on this computer
        - emphasis: Instant
      - button "Telegram Message it from your phone 2 min":
        - text: Telegram Message it from your phone
        - emphasis: 2 min
      - button "Slack DM it or @mention it in channels 4 min":
        - text: Slack DM it or @mention it in channels
        - emphasis: 4 min
      - button "Discord DM it or @mention it in a server 4 min":
        - text: Discord DM it or @mention it in a server
        - emphasis: 4 min
      - button "WhatsApp Via Meta’s Cloud API and a tunnel 15 min":
        - text: WhatsApp Via Meta’s Cloud API and a tunnel
        - emphasis: 15 min
```

# Test source

```ts
  241 |     await form.getByLabel('Bot token').fill('123456');
  242 |     await form.getByRole('button', { name: 'Test' }).click();
  243 |     await expect(form.locator('.bots-check.fail')).toContainText('copy the whole line BotFather sent');
  244 |     await form.getByLabel('Bot token').fill('123456789:AAE1234567890abcdefghijklmnopqrstu');
  245 |     await form.getByRole('button', { name: 'Test' }).click();
  246 |     await expect(form.locator('.bots-check.ok')).toContainText('@preview_pandacode_bot');
  247 |     await form.getByRole('button', { name: 'Connect Telegram' }).click();
  248 |     const card = panel(page).locator('.bots-channel');
  249 |     await expect(card).toContainText('@preview_pandacode_bot');
  250 |     await expect(panel(page)).not.toContainText('AAE1234567890');
  251 |     const guide = card.locator('details.bots-guide');
  252 |     await expect(guide).toHaveAttribute('open', '');
  253 |     await guide.getByRole('button', { name: 'Open @preview_pandacode_bot in Telegram' }).click();
  254 |     await expect.poll(async () => (await opened(page)).at(-1)).toBe('https://t.me/preview_pandacode_bot');
  255 |     const pending = panel(page).locator('.bots-pairing.pending');
  256 |     await expect(pending).toContainText('4821');
  257 |     await expect(tab(page, 'Channels')).toContainText('1');
  258 |     await pending.getByRole('button', { name: 'Approve' }).click();
  259 |     await expect(panel(page).locator('.bots-pairing')).toContainText('approved');
  260 |     await expect(card.locator('details.bots-guide')).not.toHaveAttribute('open', '');
  261 |     await panel(page).getByRole('button', { name: 'Remove Telegram' }).click();
  262 |     await panel(page).getByRole('alertdialog', { name: 'Remove this channel?' }).getByRole('button', { name: 'Remove', exact: true }).click();
  263 |     await expect(panel(page).locator('.bots-channel')).toHaveCount(0);
  264 |   });
  265 | 
  266 |   test('Slack setup creates the app from a prefilled manifest and spots swapped tokens', async ({ page }) => {
  267 |     await captureOutside(page);
  268 |     await newBot(page);
  269 |     await openSettings(page, 'Channels');
  270 |     await panel(page).getByRole('button', { name: /^Slack/ }).click();
  271 |     const form = panel(page).locator('.bots-connect');
  272 |     await form.getByRole('button', { name: 'Create Slack app' }).click();
  273 |     await expect.poll(async () => (await opened(page)).length).toBe(1);
  274 |     const url = new URL((await opened(page))[0]);
  275 |     expect(url.origin + url.pathname).toBe('https://api.slack.com/apps');
  276 |     const manifest = JSON.parse(url.searchParams.get('manifest_json')!);
  277 |     expect(manifest.display_information.name).toBe('Scout');
  278 |     expect(manifest.settings.socket_mode_enabled).toBe(true);
  279 |     expect(manifest.settings.event_subscriptions.bot_events).toEqual(['app_mention', 'message.im']);
  280 |     expect(manifest.oauth_config.scopes.bot).toEqual(expect.arrayContaining(['chat:write', 'app_mentions:read', 'im:history', 'users:read']));
  281 |     await form.getByRole('button', { name: 'Copy manifest instead' }).click();
  282 |     await expect.poll(async () => (await copied(page)).length).toBe(1);
  283 |     expect(JSON.parse((await copied(page))[0])).toEqual(manifest);
  284 |     await form.getByLabel('Bot token').fill('xapp-1-A');
  285 |     await form.getByLabel('App-level token').fill('xoxb-1-B');
  286 |     await form.getByRole('button', { name: 'Connect Slack' }).click();
  287 |     await expect(panel(page).getByRole('alert').first()).toContainText('swapped');
  288 |     // Typed values survive the panel re-rendering.
  289 |     await tab(page, 'Memory').click();
  290 |     await tab(page, 'Channels').click();
  291 |     await expect(form.getByLabel('Bot token')).toHaveValue('xapp-1-A');
  292 |     await form.getByLabel('Bot token').fill('xoxb-1-B');
  293 |     await form.getByLabel('App-level token').fill('xapp-1-A');
  294 |     await form.getByRole('button', { name: 'Connect Slack' }).click();
  295 |     await expect(panel(page).locator('.bots-channel')).toContainText('@scout in Preview Workspace');
  296 |     await expect(panel(page).locator('.bots-guide')).toContainText('/invite');
  297 |   });
  298 | 
  299 |   test('Discord setup offers the server invite and warns when Message Content is off', async ({ page }) => {
  300 |     await captureOutside(page);
  301 |     await newBot(page);
  302 |     await openSettings(page, 'Channels');
  303 |     await panel(page).getByRole('button', { name: /^Discord/ }).click();
  304 |     const form = panel(page).locator('.bots-connect');
  305 |     await expect(form.locator('.bots-steps > li')).toHaveCount(4);
  306 |     await form.getByLabel('Bot token').fill('client-secret-by-mistake');
  307 |     await form.getByRole('button', { name: 'Test' }).click();
  308 |     await expect(form.locator('.bots-check.fail')).toContainText('Client Secret');
  309 |     await form.getByLabel('Bot token').fill(`${'M'.repeat(24)}.${'G'.repeat(6)}.${'x'.repeat(38)}`);
  310 |     await form.getByRole('button', { name: 'Connect Discord' }).click();
  311 |     const card = panel(page).locator('.bots-channel');
  312 |     await expect(card.locator('.bots-warning')).toContainText('Message Content intent is off');
  313 |     await card.getByRole('button', { name: 'Add to a server' }).click();
  314 |     await expect.poll(async () => (await opened(page)).at(-1) ?? '').toContain('discord.com/oauth2/authorize?client_id=1234567890&scope=bot');
  315 |   });
  316 | 
  317 |   test('WhatsApp setup ends with copyable webhook details built from the tunnel address', async ({ page }) => {
  318 |     await captureOutside(page);
  319 |     await newBot(page);
  320 |     await openSettings(page, 'Channels');
  321 |     await panel(page).getByRole('button', { name: /^WhatsApp/ }).click();
  322 |     const form = panel(page).locator('.bots-connect');
  323 |     await expect(form.locator('.bots-steps > li')).toHaveCount(5);
  324 |     await form.getByLabel('Phone number ID').fill('+351 912 345 678');
  325 |     await form.getByLabel('Access token').fill('EAAG-preview');
  326 |     await form.getByLabel('App secret').fill('preview-secret');
  327 |     await form.getByRole('button', { name: 'Test' }).click();
  328 |     await expect(form.locator('.bots-check.fail')).toContainText('not the phone number itself');
  329 |     await form.getByLabel('Phone number ID').fill('123456789012345');
  330 |     await form.getByRole('button', { name: 'Connect WhatsApp' }).click();
  331 |     const guide = panel(page).locator('.bots-channel .bots-guide');
  332 |     await expect(guide).toContainText('cloudflared tunnel --url http://127.0.0.1:47321');
  333 |     await expect(guide).toContainText('the callback URL appears here after you save the address');
  334 |     await expect(guide).toContainText('pandacode-preview0123456789');
  335 |     await guide.getByPlaceholder('https://….trycloudflare.com').fill('http://not-secure.example');
  336 |     await guide.getByRole('button', { name: 'Save' }).click();
  337 |     await expect(panel(page).getByRole('alert').first()).toContainText('https://');
  338 |     await guide.getByPlaceholder('https://….trycloudflare.com').fill('https://quiet-river.trycloudflare.com/');
  339 |     await guide.getByRole('button', { name: 'Save' }).click();
  340 |     const callback = guide.locator('code', { hasText: '/whatsapp/' });
> 341 |     await expect(callback).toHaveText(/^https:\/\/quiet-river\.trycloudflare\.com\/whatsapp\/[0-9a-f-]{36}$/);
      |                            ^ Error: expect(locator).toHaveText(expected) failed
  342 |   });
  343 | 
  344 |   test('web chat is one click, and phone access builds a private phone link', async ({ page }) => {
  345 |     await captureOutside(page);
  346 |     await newBot(page);
  347 |     await openSettings(page, 'Channels');
  348 |     await panel(page).getByRole('button', { name: /^Web chat/ }).click();
  349 |     await panel(page).getByRole('button', { name: 'Turn on web chat' }).click();
  350 |     const card = panel(page).locator('.bots-channel');
  351 |     await expect(card).toContainText('Ready on this computer');
  352 |     await card.getByText('How to use').click();
  353 |     await card.getByRole('button', { name: 'Open web chat' }).click();
  354 |     await expect.poll(async () => (await opened(page)).at(-1) ?? '').toMatch(/^http:\/\/127\.0\.0\.1:47321\/chat\/[0-9a-f-]{36}#preview-token$/);
  355 |     await card.getByRole('checkbox', { name: /Use from my phone/ }).check();
  356 |     await expect(card).toContainText('tailscale serve --bg 47321');
  357 |     await card.getByPlaceholder('https://your-mac.tailnet.ts.net').fill('https://my-mac.tailnet.ts.net');
  358 |     await card.getByRole('button', { name: 'Save' }).click();
  359 |     await card.getByRole('button', { name: 'Copy link for my phone' }).click();
  360 |     await expect.poll(async () => (await copied(page)).at(-1)).toMatch(/^https:\/\/my-mac\.tailnet\.ts\.net\/chat\/[0-9a-f-]{36}#preview-token$/);
  361 |     await card.getByRole('button', { name: 'New link' }).click();
  362 |     await panel(page).getByRole('alertdialog', { name: 'Create a new web chat link?' }).getByRole('button', { name: 'New link' }).click();
  363 |     await expect(page.locator('#toast')).toHaveText('New link copied');
  364 |     await expect(tab(page, 'Channels')).toContainText('1');
  365 |     await tab(page, 'Activity').click();
  366 |     await expect(panel(page).getByRole('heading', { name: 'Conversations', exact: true })).toBeVisible();
  367 |     await expect(panel(page).getByRole('button', { name: 'Open web chat' })).toBeVisible();
  368 |   });
  369 | 
  370 |   test('rules validate patterns and list allow, ask and deny', async ({ page }) => {
  371 |     await newBot(page);
  372 |     await openSettings(page, 'Activity');
  373 |     await expect(panel(page).getByRole('heading', { name: /^Rules(?: |$)/ })).toBeVisible();
  374 |     await panel(page).getByRole('textbox', { name: 'Pattern' }).fill('*');
  375 |     await panel(page).getByRole('button', { name: 'Add rule' }).click();
  376 |     await expect(panel(page).getByRole('alert')).toContainText('at least two characters');
  377 |     await panel(page).getByRole('textbox', { name: 'Pattern' }).fill('git push*');
  378 |     await panel(page).getByRole('combobox', { name: 'Decision' }).selectOption('deny');
  379 |     await panel(page).getByRole('button', { name: 'Add rule' }).click();
  380 |     await expect(panel(page).locator('.bots-chip.deny')).toBeVisible();
  381 |     await panel(page).getByRole('button', { name: 'Delete rule git push*' }).click();
  382 |     await expect(panel(page).getByText('No rules.', { exact: false })).toBeVisible();
  383 |   });
  384 | 
  385 |   test('settings: pause, profile, activity, reset and delete in the modal', async ({ page }) => {
  386 |     await newBot(page);
  387 |     await openSettings(page);
  388 |     await panel(page).getByRole('button', { name: 'Pause' }).click();
  389 |     await expect(panel(page).getByText('Paused. Schedules', { exact: false })).toBeVisible();
  390 |     await panel(page).getByRole('button', { name: 'Resume' }).click();
  391 |     await expect(panel(page).locator('.bots-banner')).toHaveCount(0);
  392 |     await expect(tab(page, 'Profile')).toHaveAttribute('aria-selected', 'true');
  393 |     await expect(panel(page).getByRole('button', { name: 'Cancel' })).toHaveCount(0);
  394 |     await panel(page).getByRole('textbox', { name: 'Job' }).fill('Finds answers');
  395 |     await panel(page).getByRole('button', { name: 'Save' }).click();
  396 |     await expect(panel(page).locator('.bp-meta')).toContainText('Finds answers');
  397 |     await tab(page, 'Activity').click();
  398 |     await expect(panel(page).locator('.bots-activity')).toContainText('Settings changed');
  399 |     await expect(panel(page).locator('.bots-activity')).toContainText('Created Scout');
  400 |     // Destructive actions wait one level down, in More.
  401 |     await expect(panel(page).getByRole('button', { name: 'Delete bot' })).toHaveCount(0);
  402 |     await panel(page).getByRole('button', { name: 'More', exact: true }).click();
  403 |     await panel(page).getByRole('menuitem', { name: 'Reset bot' }).click();
  404 |     await panel(page).getByRole('alertdialog', { name: 'Reset Scout?' }).getByRole('button', { name: 'Reset', exact: true }).click();
  405 |     await expect(panel(page).locator('.bots-activity')).toContainText('Reset');
  406 |     await panel(page).getByRole('button', { name: 'More', exact: true }).click();
  407 |     await panel(page).getByRole('menuitem', { name: 'Delete bot' }).click();
  408 |     await panel(page).getByRole('alertdialog', { name: 'Delete Scout?' }).getByRole('button', { name: 'Delete', exact: true }).click();
  409 |     await expect(panel(page)).toBeHidden();
  410 |     await expect(sidebar(page).getByRole('button', { name: 'Add a bot' })).toBeVisible();
  411 |     await expect(crumb(page)).toHaveCount(0);
  412 |   });
  413 | 
  414 |   test('Open chat in settings returns to the conversation', async ({ page }) => {
  415 |     await newBot(page);
  416 |     await showTasksView(page);
  417 |     await page.locator('#navigation .task-item', { hasText: 'Explore the project' }).click();
  418 |     await showBotsView(page);
  419 |     await sidebar(page).getByRole('button', { name: 'Scout settings' }).click();
  420 |     await panel(page).getByRole('button', { name: 'Open chat' }).click();
  421 |     await expect(panel(page)).toBeHidden();
  422 |     await expect(crumb(page)).toContainText('Scout');
  423 |   });
  424 | 
  425 |   test('bots are reachable from Go to anything', async ({ page }) => {
  426 |     await page.goto('/');
  427 |     await page.locator('.search-trigger').click();
  428 |     await page.getByRole('textbox', { name: 'Search tasks and commands' }).fill('bots');
  429 |     await page.locator('#command-results').getByRole('button', { name: 'Bots' }).click();
  430 |     await expect(bots(page)).toBeVisible();
  431 |   });
  432 | });
  433 | 
```