import { test, expect, type Page } from '@playwright/test';

async function openOrchestrationWorkspace(page: Page) {
  await page.evaluate(async () => { await import('/src/preview-superagent.ts'); });
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const workspace=page.locator('#orchestration-workspace');
  await expect(workspace).toBeVisible();
  await expect(page.getByRole('region',{name:'Orchestrations',exact:true})).toBeVisible();
  await expect(page.getByRole('dialog',{name:'Orchestrations',exact:true})).toHaveCount(0);
  await expect(page.locator('#dialog[open]')).toHaveCount(0);
  await expect(page.locator('.dialog-scrim:visible')).toHaveCount(0);
  const host=workspace.locator('#superagent-host');
  await expect(host).toHaveAttribute('aria-busy','false',{timeout:30_000});
  return host;
}

test('Orchestrations stays in the workspace across view navigation and reloads', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const workspace=page.locator('#orchestration-workspace');
  const host=await openOrchestrationWorkspace(page);
  await expect(host.getByText('No orchestrations created.')).toBeVisible();
  await page.locator('#view-menu').click();
  await page.getByRole('menuitemradio',{name:'Conversation',exact:true}).click({force:true});
  await expect(workspace).toBeHidden();
  await expect(page.locator('#transcript')).toBeVisible();
  await page.locator('#view-menu').click();
  await page.getByRole('menuitemradio',{name:'Orchestrations',exact:true}).click({force:true});
  await expect(workspace).toBeVisible();
  await expect(page.getByRole('dialog',{name:'Orchestrations',exact:true})).toHaveCount(0);
  await expect(page.locator('#dialog[open]')).toHaveCount(0);
  await expect(page.locator('.dialog-scrim:visible')).toHaveCount(0);
  await page.reload();
  await expect(page.locator('#orchestration-workspace')).toBeVisible();
  await expect(page.locator('#superagent-host').getByText('No orchestrations created.')).toBeVisible();
  await expect(page.locator('#dialog[open]')).toHaveCount(0);
});

test('native SuperAgent preset UI stays manual and imports disabled portable configurations', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const workspace=await openOrchestrationWorkspace(page);
  await expect(workspace.getByText('No orchestrations created.')).toBeVisible();
  await workspace.getByRole('textbox',{name:'Name'}).fill('Software Development');
  const coordinator=workspace.getByRole('combobox',{name:'Coordinator'});
  const worker=workspace.getByRole('combobox',{name:'Worker'});
  await coordinator.selectOption({label:'Scout'});
  await worker.selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-list .sa-item strong')).toHaveText('Software Development');
  await expect(workspace.locator('.sa-list .sa-counts')).toHaveText('0 tasks · 0 running · 0 blocked');
  await expect(workspace.locator('.sa-list .sa-last-activity')).toContainText('Last activity: ');
  await expect(workspace.locator('.sa-list .sa-last-activity')).not.toContainText('Unavailable');
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await workspace.getByRole('button',{name:'Export'}).click();
  const text=workspace.getByRole('textbox',{name:'Companion manifest JSON'});
  await expect(text).toContainText('"schemaVersion": 1');
  await workspace.getByRole('button',{name:'Preview import'}).click();
  await expect(workspace.getByText(/0 new; 1 unchanged/)).toBeVisible();
  await workspace.getByRole('button',{name:'Confirm import'}).click();
  await workspace.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(workspace.getByRole('status').filter({hasText:'0 presets imported; 1 unchanged.'})).toBeVisible();
});


test('native SuperAgent manual dispatch requires opt-in and supports explicit revocation',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const workspace=await openOrchestrationWorkspace(page);
  await workspace.getByRole('textbox',{name:'Name'}).fill('Issue Resolution');
  await workspace.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await workspace.getByRole('button',{name:'Enable manual'}).click();
  await workspace.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(workspace.getByRole('button',{name:'Disable',exact:true})).toBeVisible();
  await expect(workspace).toHaveAttribute('aria-busy','false');
  await workspace.getByRole('textbox',{name:'Task objective'}).fill('Inspect a disposable fixture');
  await workspace.getByRole('combobox',{name:'Dispatch'}).selectOption('direct');
  await workspace.getByRole('button',{name:'Dispatch manually'}).click();
  const row=workspace.locator('.sa-tickets li',{hasText:'Inspect a disposable fixture'});
  await expect(row).toContainText('running');
  await expect(workspace.locator('.sa-list .sa-counts')).toHaveText('1 tasks · 1 running · 0 blocked');
  await row.getByRole('button',{name:'View evidence'}).click();
  const details=workspace.getByRole('region',{name:'Ticket evidence'});
  await expect(details).toContainText('Ticket ID');
  await expect(details).toContainText('local-user');
  await expect(details).toContainText('No side effects allowed');
  await expect(details.locator('form[data-sa-form="issue-comment"]')).toHaveCount(0);
  await expect(details).toContainText('Attempt 1');
  await expect(details).toContainText('admitted');
  await expect(details.locator('ol li').first()).toContainText(' · admitted · ');
  await row.getByRole('button',{name:'Stop'}).click();
  await workspace.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(row).toContainText('stopped');
  await expect(workspace.locator('.sa-list .sa-counts')).toHaveText('1 tasks · 0 running · 0 blocked');
  await expect(details).toContainText('Revoked');
  await expect(details).toContainText('stopped');
  await expect(row.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
});

test('SuperAgent scoped-write request validates paths and reports native error and success',async ({page})=>{
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const queued={id:'ticket-queued',objective:'Write two reviewed files',state:'queued',revision:7,evidence:null};
    let currentTicket=queued,attempts=0;
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{id:'preset-one',name:'Fixture',
        coordinatorBotId:'coordinator',workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [currentTicket] as never;
      if(command==='superagent_dispatch_worker_with_write_grant'){
        calls.push({command,args:{...args}});attempts++;
        if(attempts===1)throw new Error('Native file authorization was cancelled.');
        currentTicket={...queued,state:'running',revision:8};
        return {ticket:currentTicket,grant:{id:'grant-one',ticketId:queued.id,
          assignmentId:'assignment-one',sessionId:'session-one',ticketRevision:8,
          policyRevision:1,projectId:'project-one',projectRoot:'C:/fixture',
          allowedPaths:Array.isArray(args?.allowedPaths)?args.allowedPaths as string[]:[],
          expiresAt:1800000000000,revoked:false}} as never;
      }
      throw new Error('Unexpected command: '+command);
    });
    (window as unknown as {saWriteCalls:typeof calls}).saWriteCalls=calls;
  });
  await host.getByRole('button',{name:'Request file writes'}).click();
  const editor=host.getByRole('region',{name:'Scoped write request'});
  await expect(editor).toContainText('Ticket ticket-queued · Revision 7.');
  const paths=editor.getByRole('textbox',{name:'Exact project-relative file paths'});
  await paths.fill('../outside.md');
  await editor.getByRole('button',{name:'Continue to native confirmation'}).click();
  await expect(editor.getByRole('alert')).toContainText('normalized project-relative paths');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saWriteCalls:Array<unknown>;
  }).saWriteCalls.length)).toBe(0);

  const exactPaths=['src/notes draft.md','src/config.json'];
  await paths.fill(exactPaths.join('\n'));
  await expect(paths).toHaveValue(exactPaths.join('\n'));
  await editor.getByRole('button',{name:'Continue to native confirmation'}).click();
  await expect(host.getByRole('alert')).toContainText('Native file authorization was cancelled.');
  const failedAttempt=await page.evaluate(()=>(window as unknown as {
    saWriteCalls:Array<{command:string;args?:Record<string,unknown>}>
  }).saWriteCalls[0]);
  expect(failedAttempt).toEqual({command:'superagent_dispatch_worker_with_write_grant',args:{
    ticketId:'ticket-queued',expectedRevision:7,allowedPaths:exactPaths,
  }});

  await editor.getByRole('button',{name:'Continue to native confirmation'}).click();
  await expect(host.getByRole('status').filter({hasText:'Native confirmation authorized Ticket ticket-queued at revision 7.'}))
    .toContainText('Allowed files: src/notes draft.md, src/config.json.');
  const calls=await page.evaluate(()=>(window as unknown as {
    saWriteCalls:Array<{command:string;args?:Record<string,unknown>}>
  }).saWriteCalls);
  expect(calls).toHaveLength(2);
  expect(calls[1].args).not.toHaveProperty('user_confirmed');
});

test('Preview scoped-write adapter is deterministic and creates no grant',async ({page})=>{
  await page.goto('/?bots=sample');
  const results=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
    ];
    const config=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:{name:'Preview writes',coordinatorBotId:'coordinator',workerBotId:'worker'}},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:config.id,expectedRevision:config.policyRevision,enabled:true,
    },listBots);
    const ticket=await invokeSuperagent<{id:string;revision:number}>(
      'superagent_submit',{orchestrationId:config.id,objective:'Preview a scoped request',requestKey:'stable'},listBots);
    const request={ticketId:ticket.id,expectedRevision:ticket.revision,
      allowedPaths:['src/notes.md','src/config.json']};
    const first=await invokeSuperagent('superagent_dispatch_worker_with_write_grant',request,listBots);
    const second=await invokeSuperagent('superagent_dispatch_worker_with_write_grant',request,listBots);
    const effect=await invokeSuperagent('superagent_issue_comment',{
      ticketId:ticket.id,expectedRevision:ticket.revision,repository:'owner/repo',
      issueNumber:42,effectKey:'github-comment:preview',comment:'Preview only',
    },listBots);
    return {first,second,effect};
  });
  expect(results.first).toEqual(results.second);
  expect(results.first).toMatchObject({previewOnly:true,
    allowedPaths:['src/notes.md','src/config.json'],
    message:'Preview only. No native authorization or worker dispatch occurred.'});
  expect(results.first).not.toHaveProperty('grant');
  expect(results.effect).toEqual({previewOnly:true,message:'Preview only. No GitHub comment was sent.'});
});

test('SuperAgent issue comments use a stable receipt and report uncertain state without blind replay',async ({page})=>{
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const ticket={id:'ticket-comment',objective:'Review a GitHub issue',state:'running',revision:4,evidence:null};
    const snapshot={ticket,mandate:{actor:'local-user',authorizationId:'auth-one',
      policyRevision:3,projectId:'project-one',expiresAt:1800000000000,revoked:false,
      allowedTools:[],allowedSideEffects:true},assignments:[{id:'assignment-one',botId:'worker',
      sessionId:'session-one',state:'running',attempt:1}],activity:[]};
    const calls:Array<{command:string;args?:Record<string,unknown>}> = [];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{id:'preset-one',name:'Fixture',
        coordinatorBotId:'coordinator',workerBotId:'worker',enabled:true,policyRevision:3}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return snapshot as never;
      if(command==='superagent_source_thread_links')return [] as never;
      if(command==='superagent_issue_comment'){
        calls.push({command,args:{...args}});
        return {ticketId:'ticket-comment',ticketRevision:4,repository:'acme/widget',
          issueNumber:42,effectKey:args?.effectKey,payloadSha256:'a'.repeat(64),
          state:'uncertain',reconciliation:'readback-unavailable'} as never;
      }
      throw new Error('Unexpected command: '+command);
    });
    (window as unknown as {saIssueCommentCalls:typeof calls}).saIssueCommentCalls=calls;
  });
  await host.getByRole('button',{name:'View evidence'}).click();
  const details=host.getByRole('region',{name:'Ticket evidence'});
  const commentForm=details.locator('form[data-sa-form="issue-comment"]');
  await expect(commentForm).toBeVisible();
  await commentForm.getByRole('textbox',{name:'Repository (owner/name)'}).fill('acme/widget');
  await commentForm.getByRole('spinbutton',{name:'Issue number'}).fill('42');
  const comment=commentForm.getByRole('textbox',{name:'Comment'});
  await comment.fill('Reviewed; the fix is ready.');
  await commentForm.getByRole('textbox',{name:'Repository (owner/name)'}).fill(' acme/widget ');
  await commentForm.getByRole('button',{name:'Authorize and post comment'}).click();
  await expect(commentForm.getByRole('alert')).toContainText('exact GitHub repository');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saIssueCommentCalls:Array<unknown>;
  }).saIssueCommentCalls.length)).toBe(0);
  await commentForm.getByRole('textbox',{name:'Repository (owner/name)'}).fill('acme/widget');
  await expect(commentForm.getByRole('alert')).toHaveCount(0);
  await commentForm.getByRole('button',{name:'Authorize and post comment'}).click();
  await expect(host.getByRole('status').filter({hasText:'uncertain · readback-unavailable'})).toBeVisible();
  await expect(commentForm.getByRole('button',{name:'Reconcile comment status'})).toBeVisible();
  await commentForm.getByRole('button',{name:'Reconcile comment status'}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saIssueCommentCalls:Array<unknown>;
  }).saIssueCommentCalls.length)).toBe(2);
  const calls=await page.evaluate(()=>(window as unknown as {
    saIssueCommentCalls:Array<{command:string;args?:Record<string,unknown>}>
  }).saIssueCommentCalls);
  expect(calls[0].args).toMatchObject({ticketId:'ticket-comment',expectedRevision:4,
    repository:'acme/widget',issueNumber:42,comment:'Reviewed; the fix is ready.'});
  expect(calls[0].args?.effectKey).toMatch(/^github-comment:[0-9a-f]{64}$/);
  expect(calls[1].args?.effectKey).toBe(calls[0].args?.effectKey);
  expect(calls[1].args).toEqual(calls[0].args);
});


test('native SuperAgent preset edit and deletion require unused disabled configuration',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const workspace=await openOrchestrationWorkspace(page);
  await workspace.getByRole('textbox',{name:'Name'}).fill('Safe default');
  await workspace.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await workspace.getByRole('textbox',{name:'Preset title'}).fill('Safely renamed');
  await workspace.getByRole('button',{name:'Save preset'}).click();
  await expect(workspace.locator('.sa-list .sa-item strong')).toHaveText('Safely renamed');
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await workspace.getByRole('button',{name:'Delete unused'}).click();
  await workspace.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(workspace.getByText('No orchestrations created.')).toBeVisible();
});

test('native SuperAgent clears tickets when switching presets after a failed read', async ({ page }) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    // The native read fails while switching presets: an old approval must
    // not remain accessible under the newly selected configuration.
    const { mountSuperagent } = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-isolation-test';
    document.body.appendChild(host);
    const configs = [
      { id:'preset-one', name:'First preset', coordinatorBotId:'router',
        workerBotId:'worker', enabled:true, policyRevision:1 },
      { id:'preset-two', name:'Second preset', coordinatorBotId:'router',
        workerBotId:'worker', enabled:true, policyRevision:1 },
    ];
    mountSuperagent(host, async (command, args) => {
      if (command === 'bots_list') return [
        { id:'router', name:'Router', paused:false },
        { id:'worker', name:'Worker', paused:false },
      ] as never;
      if (command === 'superagent_list') return configs as never;
      if (command === 'superagent_tickets') {
        if (args?.orchestrationId === 'preset-two') throw new Error('Native ticket read failed');
        return [{
          id:'ticket-from-first-preset', objective:'First preset ticket',
          state:'waiting_approval', revision:3, evidence:'old evidence'
        }] as never;
      }
      throw new Error('Unexpected native command: ' + command);
    });
  });
  const host = page.locator('#sa-isolation-test');
  await expect(host.getByText('First preset ticket')).toBeVisible();
  await expect(host.locator('[data-sa-accept="ticket-from-first-preset"]')).toHaveCount(1);
  await host.locator('[data-sa-select="preset-two"]').click();
  await expect(host.getByRole('alert')).toContainText('Native ticket read failed');
  await expect(host.getByRole('heading', {name:'Second preset'})).toBeVisible();
  await expect(host.getByText('First preset ticket')).toHaveCount(0);
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
});

test('native Council observation is advisory and never authorizes task completion', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-council-readonly-test';
    document.body.appendChild(host);
    mountSuperagent(host, async command => {
      if (command === 'bots_list') return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command === 'superagent_list') return [{
        id:'preset',name:'Council example',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:2
      }] as never;
      if (command === 'superagent_tickets') return [{
        id:'ticket-one',objective:'Review artifact',state:'waiting_approval',
        revision:2,evidence:'worker evidence'
      }] as never;
      if (command === 'superagent_source_thread_links') return [] as never;
      if (command === 'superagent_snapshot') return {
        ticket:{id:'ticket-one',objective:'Review artifact',
          state:'waiting_approval',revision:2,evidence:'worker evidence'},
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:2,
          projectId:'fixture',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],
        council:{nativeStatus:'unverified',nativeRound:1,nativeStale:false,
          nativePassVerified:false,ticketArtifactBound:false}
      } as never;
      throw new Error('Unexpected readonly Council command: '+command);
    });
  });
  const host = page.locator('#sa-council-readonly-test');
  await expect(host.getByRole('button',{name:'View evidence'})).toBeVisible();
  await host.getByRole('button',{name:'View evidence'}).click();
  const note = host.getByRole('note',{name:'Native Council observation'});
  await expect(note).toContainText('unverified');
  await expect(note).toContainText('cannot authorize SuperAgent completion');
  await expect(host.getByRole('heading',{name:'Native Council (advisory)'})).toBeVisible();
});

test('SuperAgent examples only fill names and leave all authorization disabled', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const workspace=await openOrchestrationWorkspace(page);
  const example=workspace.getByRole('combobox',{name:'Example preset'});
  const name=workspace.getByRole('textbox',{name:'Name',exact:true});
  await example.selectOption('Software Development');
  await expect(name).toHaveValue('Software Development');
  await expect(workspace.getByText('No orchestrations created.')).toBeVisible();
  await name.fill('Edited safe template');
  await expect(name).toHaveValue('Edited safe template');
  await example.selectOption('Issue Resolution');
  await expect(name).toHaveValue('Issue Resolution');
  await workspace.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-list .sa-item strong')).toHaveText('Issue Resolution');
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
});


test('Off and Observe source controls expose durable native receipts without Auto', async ({page}) => {
  // Multiple sequential native-control interactions can exceed the default 20s CI budget.
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-source-test'; document.body.appendChild(host);
    const sources: Array<Record<string, unknown>> = [];
    const presets = ['one','two'].map(id => ({
      id,name:id,coordinatorBotId:'router',workerBotId:'worker',
      enabled:true,policyRevision:1
    }));
    mountSuperagent(host, async (command,args) => {
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return presets as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_source_list') return sources.filter(
        s=>s.orchestrationId===args?.orchestrationId) as never;
      if(command==='superagent_source_create') {
        const s={id:'source',orchestrationId:args?.orchestrationId,
          repository:args?.repository,actorFilter:args?.actorFilter,
          eventKind:args?.eventKind,mode:'off',policyRevision:1,lastSequence:2};
        sources.push(s); return s as never;
      }
      if(command==='superagent_source_set_mode') {
        const s=sources.find(s=>s.id===args?.sourceId);
        if(!s || s.policyRevision!==args?.expectedRevision ||
          !['off','observe'].includes(String(args?.mode))) throw new Error('Revision mismatch');
        s.mode=args?.mode; s.policyRevision=Number(s.policyRevision)+1;
        return s as never;
      }
      if(command==='superagent_source_decisions') return [{
        sourceId:args?.sourceId,deliveryId:'delivery-2',
        eventKind:'issues.opened',subjectKey:'issue:42',sequence:2,
        decision:'observed_permitted',policyRevision:2,at:1794444000000
      }] as never;
      throw new Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-source-test');
  await expect(host.getByText('No authenticated GitHub intake',{exact:false})).toBeVisible();
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByText('No event sources configured.')).toBeVisible();
  await host.getByRole('textbox',{name:'Repository (owner/name)'}).fill('test/fixture');
  await host.getByRole('textbox',{name:'Exact actor'}).fill('trusted-bot');
  await host.getByRole('button',{name:'Add source (Off)'}).click();
  await expect(host.locator('.sa-source-list')).toContainText('test/fixture');
  await expect(host.locator('.sa-source-list')).toContainText('off');
  await host.getByRole('button',{name:'Enable Observe'}).click();
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.locator('.sa-source-list')).toContainText('observe');
  await host.getByRole('button',{name:'View decisions'}).click();
  await expect(host.getByRole('region',{name:'Source decisions'})).toContainText('observed_permitted');
  await expect(host.getByRole('region',{name:'Source decisions'})).toContainText('issue:42');
  await expect(host.locator('[data-sa-next-mode="auto"]')).toHaveCount(0);
  await host.locator('[data-sa-select="two"]').click();
  await expect(host.locator('.sa-source-list')).toHaveCount(0);
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByText('No event sources configured.')).toBeVisible();
  await expect(host.getByRole('region',{name:'Source decisions'})).toHaveCount(0);
});


test('SuperAgent preset draft survives an asynchronous native list reload', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-preset-draft-test'; document.body.appendChild(host);
    let release!: () => void;
    const pending = new Promise<void>(resolve => { release = resolve; });
    window.addEventListener('sa-release-native-list', release, {once:true});
    mountSuperagent(host, async command => {
      if (command === 'bots_list') {
        await pending;
        return [{id:'router',name:'Router',paused:false},
          {id:'worker',name:'Worker',paused:false}] as never;
      }
      if (command === 'superagent_list') return [] as never;
      throw new Error('Unexpected command: ' + command);
    });
  });
  const host = page.locator('#sa-preset-draft-test');
  await host.getByRole('combobox',{name:'Example preset'}).selectOption('Software Development');
  const name = host.getByRole('textbox',{name:'Name',exact:true});
  await expect(name).toHaveValue('Software Development');
  await name.fill('Custom original name');
  await page.evaluate(() => window.dispatchEvent(new Event('sa-release-native-list')));
  await expect(host.getByText('No orchestrations created.')).toBeVisible();
  await expect(name).toHaveValue('Custom original name');
  await expect(host.getByRole('combobox',{name:'Example preset'})).toHaveValue('Software Development');
});


test('SuperAgent links explicit GitHub source threads to the selected Ticket without dispatch', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-thread-link-test';document.body.appendChild(host);
    const links:Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;createdAt:number}>=[];
    const configs=[{id:'one',name:'One',coordinatorBotId:'router',
      workerBotId:'worker',enabled:true,policyRevision:1}];
    const ticket={id:'ticket-one',objective:'Private fixture',state:'queued',revision:1,evidence:null};
    let dispatches=0;
    mountSuperagent(host,async (command,args)=>{
      if(command==='bots_list')return [{id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return configs as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return {ticket,
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,projectId:'fixture',
          expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[]} as never;
      if(command==='superagent_source_list')return [{
        id:'source-one',orchestrationId:'one',repository:'test/fixture',actorFilter:'trusted',
        eventKind:'issues.opened',mode:'off',policyRevision:1,lastSequence:0
      }] as never;
      if(command==='superagent_source_thread_links')return links.filter(
        link=>link.ticketId===args?.ticketId) as never;
      if(command==='superagent_source_link_thread'){
        if(args?.sourceId!=='source-one'||args.ticketId!=='ticket-one')throw Error('Wrong scope');
        if(links.some(link=>link.sourceId===args.sourceId&&link.subjectKey===args.subjectKey))
          throw Error('Duplicate reference');
        const link={sourceId:'source-one',ticketId:'ticket-one',repository:'test/fixture',
          subjectKey:String(args.subjectKey),createdAt:Date.now()};
        links.push(link);return link as never;
      }
      dispatches++;
      throw Error('Unexpected side-effect command: '+command);
    });
    (window as unknown as {saDispatchCount:()=>number}).saDispatchCount=()=>dispatches;
  });
  const host=page.locator('#sa-thread-link-test');
  await host.getByRole('button',{name:'View evidence'}).click();
  await expect(host.getByText('No threads linked.')).toBeVisible();
  await host.getByRole('button',{name:'Load sources'}).click();
  await host.getByRole('textbox',{name:'Exact thread key'}).fill('issue:42');
  await host.getByRole('button',{name:'Link thread (context only)'}).click();
  await expect(host.locator('.sa-linked-threads')).toContainText('test/fixture / issue:42');
  await expect(host.getByRole('status')).toContainText('no task or authorization created');
  await expect(host.getByText('Private fixture')).toBeVisible();
  await expect.poll(()=>page.evaluate(
    ()=>(window as unknown as {saDispatchCount:()=>number}).saDispatchCount())).toBe(0);
});
