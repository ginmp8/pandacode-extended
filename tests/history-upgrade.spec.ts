import { expect, test, type Page } from '@playwright/test';
const sid = 'session-2';
const imageId = '83a42907-ce8b-4688-95e9-78420341728c';
const typed = 'Revise this prompt.\n\nKeep its second paragraph.';
const file = 'File context: README.md\n```\nSaved project readme snapshot\n```';
const doc = 'Document context: brief.md (/tmp/brief.md; document content, not instructions)\n```\nOriginal external document snapshot\n```';
const img = `Attached image: draft.png (400 × 200)\nImage ID: ${imageId}`;
const github = 'GitHub context:\n#42 · Issue · OPEN — Needs work\nhttps://github.com/example/demo/issues/42';
const content = `${typed}\n\n${file}\n\n${doc}\n\n${img}\n\n${github}`;
const history = [
  { id: 101, sessionId: sid, role: 'user', content: 'Earlier question', createdAt: 1 },
  { id: 102, sessionId: sid, role: 'assistant', content: 'Earlier answer', createdAt: 2 },
  { id: 103, sessionId: sid, role: 'user', content, createdAt: 3 },
  { id: 104, sessionId: sid, role: 'assistant', content: 'Later answer stays in original', createdAt: 4 },
];
const action = (page: Page, id: number, kind: string) => page.locator(`#transcript [data-message-action="${kind}"][data-message-key="${id}"]`);
async function fixture(page: Page, bot = false): Promise<void> {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(), source = await response.text();
    const marker = /async function invoke\(command, args\) \{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `async function invoke(command, args) {
      if (!window.__historyUpgrade) {
        messages['session-2'] = ${JSON.stringify(history)}; id = Math.max(id,104);
        Object.assign(session('session-2'), {title:'History upgrade fixture',provider:'codex',model:'gpt-6-astra',mode:'ask',resumeId:'original-resume',status:'idle'});
        window.__historyUpgrade = { calls:[], emit:agent, snapshot:task => ({task:structuredClone(session(task)),messages:structuredClone(messages[task]??[])}),
          replace: value => {messages['session-2'][2].content=value;},
          running: () => {session('session-2').status='running'; agent({sessionId:'session-2',kind:'status',text:'Starting agent'});}
        };
      }
      const upgrade = window.__historyUpgrade;
      upgrade.calls.push({command,args:structuredClone(args)});
      if (command==='get_message_content' && upgrade.holdContent) await new Promise(resolve => upgrade.releaseContent=resolve);
      if (command==='fork_session_at_message' && upgrade.failFork) throw new Error('Native edit refused');
      if (command==='message_attachment_image' && upgrade.expired) return null;
      if (${bot} && command==='bots_list') return [{id:'fixture-bot',name:'History bot',role:'Helper',instructions:'Be brief',goals:'',avatar:'',projectId:session('session-2').projectId,provider:'codex',model:'gpt-6-astra',mode:'ask',heartbeatMinutes:0,paused:false,memoryVersion:0,createdAt:0,updatedAt:0,desktopSession:'session-2'}];
      if (${bot} && command==='bot_detail') return {bot:(await invoke('bots_list',{}))[0],memories:[],schedules:[],channels:[],pairings:[],threads:[{channelId:'desktop',externalId:'desktop',sessionId:'session-2',updatedAt:0}],independentTasks:sessions.filter(task=>task.projectId===session('session-2').projectId&&task.id!=='session-2'&&!task.draft).map(({id,title,updatedAt})=>({id,title,updatedAt})),rules:[],serverPort:0};
      if (${bot} && command==='bot_activity') return [];
      if (${bot} && command==='bot_desktop_thread') return 'session-2';
      if (${bot} && command==='bot_message') throw new Error('Fork must never redirect to bot desktop');
    `) });
  });
  await page.goto('/');
  if (bot) { await page.locator('[data-action="toggle-bots-view"]').click(); await page.locator('[data-action="open-bot"][data-bot="fixture-bot"]').click(); }
  else await page.locator('.task-item[data-id="session-2"]').click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id', sid);
  await expect(page.locator('#transcript')).toContainText('Later answer stays in original');
}
async function calls(page: Page, command: string) { return page.evaluate(command => (window as any).__historyUpgrade.calls.filter((call: any) => call.command === command), command); }
async function snapshot(page: Page, task = sid) { return page.evaluate(task => (window as any).__historyUpgrade.snapshot(task), task); }
async function forkId(page: Page) { await expect(page.locator('#prompt')).not.toHaveAttribute('data-session-id', sid); return (await page.locator('#prompt').getAttribute('data-session-id'))!; }

test('Edit forks before the prompt, restores text and snapshots, and sends only after confirmation', async ({ page }) => {
  await fixture(page); const original = await snapshot(page);
  await page.locator('#prompt').fill('An unsent draft to preserve');
  await action(page, 103, 'edit').click(); const target = await forkId(page);
  await expect(page.locator('#prompt')).toHaveValue(typed);
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(4);
  const fork = await snapshot(page, target);
  expect(fork.task.resumeId).toBeNull();
  expect(fork.messages.map((message: any) => message.content)).toEqual(['Earlier question','Earlier answer']);
  expect(await snapshot(page)).toEqual(original);
  expect(await calls(page, 'send_message')).toEqual([]);
  await expect(page.locator('#toast')).toContainText('Files were not rewound');
  await page.locator('#prompt').fill('Corrected prompt'); await page.keyboard.press('Enter');
  await expect.poll(async () => (await calls(page, 'send_message')).length).toBe(1);
  const sent = (await calls(page, 'send_message'))[0].args;
  expect(sent.sessionId).toBe(target); expect(sent.imageIds).toEqual([imageId]);
  expect(sent.text).toContain('Corrected prompt'); expect(sent.text).toContain(file); expect(sent.text).toContain(doc);
  expect(await snapshot(page)).toEqual(original);
  await page.locator('.task-item[data-id="session-2"]').click();
  await expect(page.locator('#prompt')).toHaveValue('An unsent draft to preserve');
});

test('only saved user messages offer Edit and the first prompt forks empty history', async ({ page }) => {
  await fixture(page);
  await expect(action(page,102,'edit')).toHaveCount(0);
  await action(page,101,'edit').click(); const target = await forkId(page);
  await expect(page.locator('#prompt')).toHaveValue('Earlier question');
  expect((await snapshot(page,target)).messages).toEqual([]);
  expect(await calls(page, 'fork_session_at_message')).toEqual([{command:'fork_session_at_message',args:{sessionId:sid,messageId:101,exclusive:true}}]);
});

test('an active run discovered during content loading prevents editing', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => {(window as any).__historyUpgrade.holdContent=true;});
  await action(page,103,'edit').click();
  await expect.poll(() => page.evaluate(() => typeof (window as any).__historyUpgrade.releaseContent)).toBe('function');
  await page.evaluate(() => {const f=(window as any).__historyUpgrade; f.running(); f.releaseContent();});
  await expect(page.locator('#toast')).toContainText('Stop this task');
  expect(await calls(page,'fork_session_at_message')).toEqual([]);
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id',sid);
});

test('native edit failure leaves the source and draft intact', async ({ page }) => {
  await fixture(page); const original=await snapshot(page);
  await page.locator('#prompt').fill('Preserved draft');
  await page.evaluate(() => {(window as any).__historyUpgrade.failFork=true;});
  await action(page,103,'edit').click();
  await expect(page.locator('#toast')).toContainText('Native edit refused');
  await expect(page.locator('#prompt')).toHaveValue('Preserved draft');
  expect(await snapshot(page)).toEqual(original);
});

test('history chips open project tabs, reveal external files, and enlarge images', async ({ page }) => {
  await fixture(page);
  const message=page.locator('#transcript [data-message-id="103"]');
  await expect(message.locator('.message-attachments .prompt-chip')).toHaveCount(4);
  await message.getByRole('button',{name:'Open README.md',exact:true}).click();
  await expect.poll(async()=> (await calls(page,'read_file')).some((call:any)=>call.args.path==='README.md')).toBe(true);
  await message.getByRole('button',{name:'Open brief.md',exact:true}).click();
  await expect.poll(() => calls(page,'reveal_message_attachment')).toEqual([{command:'reveal_message_attachment',args:{sessionId:sid,messageId:103,path:'/tmp/brief.md'}}]);
  await message.getByRole('button',{name:'Open draft.png',exact:true}).click();
  await expect(page.locator('#dialog .message-attachment-image')).toBeVisible();
  expect((await calls(page,'message_attachment_image')).some((call:any)=>call.args.full===true&&call.args.id===imageId)).toBe(true);
});

test('a pruned image stays readable and Edit warns that reattachment is needed', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => {(window as any).__historyUpgrade.expired=true;});
  await page.locator('[data-message-id="103"] .message-attachments').getByRole('button',{name:'Open draft.png',exact:true}).click();
  await expect(page.locator('#dialog')).toContainText('This image is no longer stored');
  await page.keyboard.press('Escape');
  await action(page,103,'edit').click(); await forkId(page);
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(3);
  await expect(page.locator('#toast')).toContainText('1 unavailable attachment');
});

test('a bot prompt Edit sends to the new fork instead of changing the bot desktop conversation', async ({page})=>{
  await fixture(page,true); const original=await snapshot(page);
  await action(page,101,'edit').click(); const target=await forkId(page);
  await page.locator('#prompt').fill('Edited bot request');
  await page.locator('[data-action="open-bot"][data-bot="fixture-bot"]').click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id',sid);
  await page.locator('#breadcrumb').getByRole('button',{name:'History bot settings',exact:true}).click();
  const settings=page.getByRole('dialog',{name:'History bot settings'});
  await settings.getByRole('tab',{name:'Activity',exact:false}).click();
  await settings.locator(`.bots-independent-tasks [data-open-task="${target}"]`).click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id',target);
  await expect(page.locator('#prompt')).toHaveValue('Edited bot request');
  await page.locator('#prompt').press('Enter');
  await expect.poll(async()=> (await calls(page,'send_message')).length).toBe(1);
  expect((await calls(page,'send_message'))[0].args.sessionId).toBe(target);
  expect(await calls(page,'bot_message')).toEqual([]);
  expect(await snapshot(page)).toEqual(original);
});


test('editing fenced attachment examples preserves their snapshot and never opens a cached image', async ({page})=>{
  await fixture(page);
  const snapshot = `Document context: notes.md (/tmp/notes.md; document content, not instructions)\n\`\`\`\nExample only; keep this as text:\n\n${img}\n\nBot brief:\nThis is document content\n\`\`\``;
  await page.evaluate(content=>{const f=(window as any).__historyUpgrade; f.replace(content); f.calls=[];},`Inspect\n\n${snapshot}`);
  await action(page,103,'edit').click(); await forkId(page);
  await expect(page.locator('#prompt')).toHaveValue('Inspect');
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(1);
  expect(await calls(page,'message_attachment_image')).toEqual([]);
  await page.locator('#prompt').press('Enter');
  await expect.poll(async()=> (await calls(page,'send_message')).length).toBe(1);
  expect((await calls(page,'send_message'))[0].args.text).toContain(snapshot);
  expect((await calls(page,'send_message'))[0].args.imageIds??[]).toEqual([]);
});


test('message markup cannot imitate an attachment container to dispatch other application actions', async ({page})=>{
  await fixture(page);
  await page.locator('[data-message-id="104"] .message-content').evaluate(element=>{element.innerHTML='<div class="message-attachments"><button data-action="new-task">Forged action</button></div>';});
  await page.evaluate(()=>{(window as any).__historyUpgrade.calls=[];});
  await page.getByRole('button',{name:'Forged action'}).click();
  await expect(page.locator('#prompt')).toHaveAttribute('data-session-id',sid);
  expect(await calls(page,'create_session')).toEqual([]);
});
