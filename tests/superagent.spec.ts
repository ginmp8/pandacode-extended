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
  await workspace.getByRole('textbox',{name:'Name',exact:true}).fill('Software Development');
  const coordinator=workspace.getByRole('combobox',{name:'Coordinator'});
  const worker=workspace.getByRole('combobox',{name:'Worker 1',exact:true});
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
  await workspace.getByRole('textbox',{name:'Name',exact:true}).fill('Issue Resolution');
  await workspace.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker 1',exact:true}).selectOption({label:'Writer'});
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
  await expect(details.getByRole('heading',{name:'Native activity',exact:true}).locator('xpath=following-sibling::ol[1] li').first()).toContainText(' · admitted · ');
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

test('Preview adapter snapshots ordered Team roles at Ticket admission',async ({page})=>{
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker-one',name:'Worker One',paused:false,projectId:'preview-project'},
      {id:'worker-two',name:'Worker Two',paused:false,projectId:'preview-project'},
      {id:'reviewer',name:'Reviewer',paused:false,projectId:'preview-project'},
    ];
    const created=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:{name:'Preview Team',coordinatorBotId:'coordinator',
        workerBotId:'worker-one',workerBotIds:['worker-one','worker-two'],reviewerBotIds:['reviewer']}},listBots);
    const updated=await invokeSuperagent<{policyRevision:number;teamVersion:number;workerBotIds:string[]}>(
      'superagent_update',{orchestrationId:created.id,expectedRevision:created.policyRevision,
        input:{name:'Preview Team',coordinatorBotId:'coordinator',workerBotId:'worker-two',
          workerBotIds:['worker-two','worker-one'],reviewerBotIds:['reviewer']}},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:created.id,expectedRevision:updated.policyRevision,enabled:true,
    },listBots);
    const ticket=await invokeSuperagent<{id:string}>(
      'superagent_submit',{orchestrationId:created.id,objective:'Preview ordered roles'},listBots);
    const snapshot=await invokeSuperagent<{teamSnapshot:Record<string,any>}>(
      'superagent_snapshot',{ticketId:ticket.id},listBots);
    return {updated,snapshot:snapshot.teamSnapshot};
  });
  expect(result.updated.teamVersion).toBe(2);
  expect(result.updated.workerBotIds).toEqual(['worker-two','worker-one']);
  expect(result.snapshot).toMatchObject({
    provenance:'native',teamVersion:2,workerBotIds:['worker-two','worker-one'],reviewerBotIds:['reviewer'],
    roles:[
      {role:'coordinator',ordinal:0,botId:'coordinator'},
      {role:'worker',ordinal:0,botId:'worker-two'},
      {role:'worker',ordinal:1,botId:'worker-one'},
      {role:'reviewer',ordinal:0,botId:'reviewer'},
    ],
  });
});


test('SuperAgent issue comments use a stable receipt and report uncertain state without blind replay',async ({page})=>{
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const ticket={id:'ticket-comment',objective:'Review a GitHub issue',state:'running',revision:4,evidence:null};
    const snapshot={ticket,teamSnapshot:{snapshotVersion:1,provenance:'native',orchestrationId:'preset-one',
      teamVersion:2,name:'Issue Team',projectId:'project-one',coordinatorBotId:'coordinator',
      workerBotIds:['worker','worker-two'],reviewerBotIds:['reviewer'],
      policy:{policyRevision:3,autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,councilEnabled:false},
      roles:[
        {role:'coordinator',ordinal:0,botId:'coordinator',effectiveConfig:{name:'Coordinator'},effectiveConfigSha256:'coordinator-hash'},
        {role:'worker',ordinal:0,botId:'worker',effectiveConfig:{name:'Worker'},effectiveConfigSha256:'worker-hash'},
        {role:'worker',ordinal:1,botId:'worker-two',effectiveConfig:{name:'Worker Two'},effectiveConfigSha256:'worker-two-hash'},
        {role:'reviewer',ordinal:0,botId:'reviewer',effectiveConfig:{name:'Reviewer'},effectiveConfigSha256:'reviewer-hash'},
      ]},mandate:{actor:'local-user',authorizationId:'auth-one',
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
  await expect(details).toContainText('Saved Team snapshot');
  await expect(details).toContainText('Issue Team / 2');
  await expect(details).toContainText('worker-two');
  await expect(details).toContainText('reviewer-hash');
  await details.getByText('Inspect exact saved Team payload').click();
  await expect(details.locator('pre')).toContainText('"provenance": "native"');
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
  await workspace.getByRole('textbox',{name:'Name',exact:true}).fill('Safe default');
  await workspace.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker 1',exact:true}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  const selected=workspace.getByRole('region',{name:'Selected orchestration',exact:true});
  await selected.getByRole('textbox',{name:'Team name',exact:true}).fill('Safely renamed');
  await selected.getByRole('button',{name:'Save Team version',exact:true}).click();
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
        teamSnapshot:{snapshotVersion:1,provenance:'native',orchestrationId:'preset',teamVersion:1,
          name:'Council example',projectId:'fixture',coordinatorBotId:'router',workerBotIds:['worker'],
          reviewerBotIds:[],policy:{policyRevision:1,autonomy:'off',maxConcurrentTasks:1,
            maxAttemptsPerAssignment:2,councilEnabled:false},
          roles:[{role:'coordinator',ordinal:0,botId:'router',effectiveConfig:null,effectiveConfigSha256:null},
            {role:'worker',ordinal:0,botId:'worker',effectiveConfig:null,effectiveConfigSha256:null}]},
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
  await workspace.getByRole('combobox',{name:'Worker 1',exact:true}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-list .sa-item strong')).toHaveText('Issue Resolution');
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
});


test('Team Builder creates and edits ordered worker and reviewer roles', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker-one',name:'Worker One',paused:false,projectId:'project-one'},
      {id:'worker-two',name:'Worker Two',paused:false,projectId:'project-one'},
      {id:'foreign-worker',name:'Foreign Worker',paused:false,projectId:'project-two'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    let config:Record<string,any>|null=null;
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return config?[{...config,ticketCount:0,runningCount:0,blockedCount:0}] as never:[] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'){
        calls.push({command,args:{...args}});
        const input=args?.input as Record<string,any>;
        config={id:'team-one',...input,teamVersion:1,projectId:'project-one',enabled:false,policyRevision:1};
        return config as never;
      }
      if(command==='superagent_update'){
        calls.push({command,args:{...args}});
        const input=args?.input as Record<string,any>;
        config={...config,...input,workerBotId:input.workerBotId,
          teamVersion:Number(config?.teamVersion)+1,policyRevision:Number(config?.policyRevision)+1};
        return config as never;
      }
      throw new Error('Unexpected Team Builder command: '+command);
    });
    (window as unknown as {saTeamCalls:typeof calls}).saTeamCalls=calls;
  });
  const team=host;
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('Ordered Team');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  const workerOne=host.getByRole('combobox',{name:'Worker 1',exact:true});
  await expect(workerOne.locator('option[value="foreign-worker"]')).toHaveAttribute('disabled','');
  await workerOne.selectOption('worker-one');
  await host.getByRole('button',{name:'Add worker'}).click();
  await host.getByRole('combobox',{name:'Worker 2'}).selectOption('worker-two');
  await host.getByRole('button',{name:'Add reviewer'}).click();
  await host.getByRole('combobox',{name:'Reviewer 1'}).selectOption('reviewer-one');
  await host.getByRole('button',{name:'Add reviewer'}).click();
  await host.getByRole('combobox',{name:'Reviewer 2'}).selectOption('reviewer-two');
  await host.getByRole('button',{name:'Move worker 2 up'}).click();
  await host.getByRole('button',{name:'Move reviewer 2 up'}).click();
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(team.locator('.sa-list')).toContainText('Worker Two, Worker One');
  let calls=await page.evaluate(()=>(window as unknown as {
    saTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saTeamCalls());
  expect(calls[0].args?.input).toMatchObject({
    name:'Ordered Team',coordinatorBotId:'coordinator',workerBotId:'worker-two',
    workerBotIds:['worker-two','worker-one'],reviewerBotIds:['reviewer-two','reviewer-one'],
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  await selected.getByRole('button',{name:'Move worker 2 up'}).click();
  await selected.getByRole('button',{name:'Move reviewer 2 up'}).click();
  await selected.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  calls=await page.evaluate(()=>(window as unknown as {
    saTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saTeamCalls());
  expect(calls[1]).toMatchObject({command:'superagent_update',args:{
    orchestrationId:'team-one',expectedRevision:1,input:{
      workerBotId:'worker-one',workerBotIds:['worker-one','worker-two'],
      reviewerBotIds:['reviewer-one','reviewer-two'],
    },
  }});
});

test('Team Builder removes worker and reviewer roles while retaining the required worker', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker-one',name:'Worker One',paused:false,projectId:'project-one'},
      {id:'worker-two',name:'Worker Two',paused:false,projectId:'project-one'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    let config:Record<string,any>={id:'team-removal',name:'Removable roles',
      projectId:'project-one',
      coordinatorBotId:'coordinator',workerBotId:'worker-one',
      workerBotIds:['worker-one','worker-two'],reviewerBotIds:['reviewer-one','reviewer-two'],
      enabled:false,policyRevision:4,teamVersion:1};
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [{...config,ticketCount:0,runningCount:0,blockedCount:0}] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_update'){
        calls.push({command,args:{...args}});
        const input=args?.input as Record<string,any>;
        config={...config,...input,teamVersion:Number(config.teamVersion)+1,
          policyRevision:Number(config.policyRevision)+1};
        return config as never;
      }
      throw new Error('Unexpected Team removal command: '+command);
    });
    (window as unknown as {saRemovalCalls:typeof calls}).saRemovalCalls=calls;
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  const workers=selected.getByRole('group',{name:'Workers (ordered)',exact:true});
  await workers.getByRole('button',{name:'Remove worker 1'}).click();
  await expect(workers.getByRole('combobox',{name:'Worker 1',exact:true})).toHaveValue('worker-two');
  await expect(workers.getByRole('button',{name:'Remove worker 1'})).toHaveCount(0);

  const reviewers=selected.getByRole('group',{name:'Reviewers (ordered)',exact:true});
  await reviewers.getByRole('button',{name:'Remove reviewer 1'}).click();
  await reviewers.getByRole('button',{name:'Remove reviewer 1'}).click();
  await expect(reviewers).toContainText('No reviewers selected.');
  await selected.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');

  const calls=await page.evaluate(()=>(window as unknown as {
    saRemovalCalls:Array<{command:string;args?:Record<string,any>}>
  }).saRemovalCalls);
  expect(calls).toHaveLength(1);
  expect(calls[0]).toMatchObject({command:'superagent_update',args:{
    orchestrationId:'team-removal',expectedRevision:4,input:{
      workerBotId:'worker-two',workerBotIds:['worker-two'],reviewerBotIds:[],
    },
  }});
});

test('Team Builder keeps an edit draft and shows native rejection feedback', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker-one',name:'Worker One',paused:false,projectId:'project-one'},
      {id:'worker-two',name:'Worker Two',paused:false,projectId:'project-one'},
      {id:'reviewer',name:'Reviewer',paused:false,projectId:'project-one'},
    ];
    const config={id:'team-native-blocked',name:'Original Team',coordinatorBotId:'coordinator',
      projectId:'project-one',
      workerBotId:'worker-one',workerBotIds:['worker-one','worker-two'],reviewerBotIds:['reviewer'],
      enabled:false,policyRevision:9};
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [{...config,ticketCount:0,runningCount:0,blockedCount:0}] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_update'){
        calls.push({command,args:{...args}});
        throw new Error('Native policy blocked this Team update: revision changed.');
      }
      throw new Error('Unexpected blocked edit command: '+command);
    });
    (window as unknown as {saBlockedEditCalls:typeof calls}).saBlockedEditCalls=calls;
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  const editor=selected.locator('form[data-sa-form="edit"]');
  await editor.getByRole('textbox',{name:'Team name',exact:true}).fill('Edited but rejected');
  await editor.getByRole('button',{name:'Move worker 2 up'}).click();
  await editor.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  await expect(host.getByRole('alert')).toHaveText('Native policy blocked this Team update: revision changed.');
  await expect(editor.getByRole('textbox',{name:'Team name',exact:true})).toHaveValue('Edited but rejected');
  await expect(selected.getByRole('combobox',{name:'Worker 1',exact:true})).toHaveValue('worker-two');
  await expect(selected.getByRole('combobox',{name:'Worker 2',exact:true})).toHaveValue('worker-one');
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText('Original Team');
  const calls=await page.evaluate(()=>(window as unknown as {
    saBlockedEditCalls:Array<{command:string;args?:Record<string,any>}>
  }).saBlockedEditCalls);
  expect(calls).toEqual([{command:'superagent_update',args:{
    orchestrationId:'team-native-blocked',expectedRevision:9,input:{
      name:'Edited but rejected',coordinatorBotId:'coordinator',
      workerBotIds:['worker-two','worker-one'],reviewerBotIds:['reviewer'],workerBotId:'worker-two',
    },
  }}]);
});

test('Team Builder disables roles from another project', async ({page}) => {
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker-one',name:'Worker One',paused:false,projectId:'project-one'},
      {id:'worker-other',name:'Worker Other Project',paused:false,projectId:'project-two'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-other',name:'Reviewer Other Project',paused:false,projectId:'project-two'},
    ];
    mountSuperagent(host,async(command)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [] as never;
      if(command==='superagent_tickets')return [] as never;
      throw new Error('Unexpected project compatibility command: '+command);
    });
  });

  await host.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption('coordinator');
  const workers=host.getByRole('group',{name:'Workers (ordered)',exact:true});
  const worker=workers.getByRole('combobox',{name:'Worker 1',exact:true});
  await expect(worker.locator('option[value="worker-other"]')).toHaveAttribute('disabled','');
  await expect(worker.locator('option[value="worker-one"]')).not.toHaveAttribute('disabled','');
  await host.getByRole('button',{name:'Add reviewer',exact:true}).click();
  const reviewers=host.getByRole('group',{name:'Reviewers (ordered)',exact:true});
  await expect(reviewers.getByRole('combobox',{name:'Reviewer 1',exact:true})
    .locator('option[value="reviewer-other"]')).toHaveAttribute('disabled','');
});

test('Ticket evidence reports a missing saved Team snapshot without using the current Team', async ({page}) => {
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const ticket={id:'ticket-without-snapshot',objective:'Inspect saved evidence',
      state:'queued',revision:1,evidence:null};
    mountSuperagent(host,async command=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'current-worker',name:'Current Team Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{id:'team-current',name:'Mutable Current Team',
        coordinatorBotId:'coordinator',workerBotId:'current-worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return {
        ticket,mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,
          projectId:'fixture',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},assignments:[],activity:[],
      } as never;
      if(command==='superagent_source_thread_links')return [] as never;
      throw new Error('Unexpected missing-snapshot command: '+command);
    });
  });
  await host.getByRole('button',{name:'View evidence'}).click();
  const details=host.getByRole('region',{name:'Ticket evidence'});
  const snapshot=details.getByRole('region',{name:'Saved Team snapshot'});
  await expect(snapshot.getByRole('heading',{name:'Saved Team snapshot unavailable'})).toBeVisible();
  await expect(snapshot).toContainText('native Ticket response did not include teamSnapshot');
  await expect(snapshot).toContainText('The current Team configuration was not substituted.');
  await expect(snapshot).not.toContainText('Mutable Current Team');
  await expect(snapshot).not.toContainText('current-worker');
});

test('Preview Ticket evidence labels null profile and hash fields as unavailable', async ({page}) => {
  await page.goto('/?bots=sample');
  await openOrchestrationWorkspace(page);
  const previewRoles=await page.evaluate(async () => {
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const bots=[
      {id:'preview-coordinator',name:'Preview Coordinator',paused:false,projectId:'preview-project'},
      {id:'preview-worker',name:'Preview Worker',paused:false,projectId:'preview-project'},
    ];
    const listBots=async()=>bots;
    const created=await invokeSuperagent<{id:string;policyRevision:number}>('superagent_create',{
      input:{name:'Null profile evidence',coordinatorBotId:'preview-coordinator',
        workerBotId:'preview-worker',workerBotIds:['preview-worker'],reviewerBotIds:[]},
    },listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:created.id,expectedRevision:created.policyRevision,enabled:true,
    },listBots);
    const ticket=await invokeSuperagent<{id:string}>('superagent_submit',{
      orchestrationId:created.id,objective:'Render preview profile evidence',
    },listBots);
    const details=await invokeSuperagent<Record<string,any>>('superagent_snapshot',
      {ticketId:ticket.id},listBots);
    const configs=await invokeSuperagent<Array<Record<string,any>>>('superagent_list',{},listBots);
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    mountSuperagent(host,async command=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return configs as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return details as never;
      if(command==='superagent_source_thread_links')return [] as never;
      throw new Error('Unexpected Preview evidence command: '+command);
    });
    return (details.teamSnapshot as {roles:Array<Record<string,unknown>>}).roles;
  });
  expect(previewRoles).toEqual([
    {role:'coordinator',ordinal:0,botId:'preview-coordinator',effectiveConfig:null,effectiveConfigSha256:null},
    {role:'worker',ordinal:0,botId:'preview-worker',effectiveConfig:null,effectiveConfigSha256:null},
  ]);

  const host=page.locator('#superagent-host');
  await host.getByRole('button',{name:'View evidence'}).click();
  const snapshot=host.getByRole('region',{name:'Saved Team snapshot'});
  await expect(snapshot.getByRole('heading',{name:'Saved Team snapshot'})).toBeVisible();
  await expect(snapshot.locator('ol')).toContainText('profile evidence unavailable');
  await expect(snapshot.locator('ol')).not.toContainText('profile SHA-256');
  await snapshot.getByText('Inspect exact saved Team payload').click();
  await expect(snapshot.locator('pre')).toContainText('"effectiveConfig": null');
  await expect(snapshot.locator('pre')).toContainText('"effectiveConfigSha256": null');
  await expect(snapshot.locator('pre')).not.toContainText('"effectiveConfigSha256": "');
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
        sourceId:args?.sourceId,deliveryId:'gh-issues-edited-preview',
        actor:'trusted-bot',
        eventKind:sources.find(s=>s.id===args?.sourceId)?.eventKind ?? 'issues.edited',
        subjectKey:'issue:42',sequence:2,
        decision:'observed_permitted',policyRevision:2,at:1794444000000
      }] as never;
      throw new Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-source-test');
  await expect(host.getByText('Fetch once is a user-clicked read',{exact:false})).toBeVisible();
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByText('No event sources configured.')).toBeVisible();
  const eventKind=host.getByRole('combobox',{name:'Event class'});
  const eventKinds=await eventKind.locator('option').evaluateAll(options=>(options as HTMLOptionElement[]).map(option=>option.value));
  expect(eventKinds).toEqual(['issues.opened','issues.edited','issue_comment.created',
    'pull_request.opened','pull_request_review.submitted','check_run.completed']);
  await eventKind.selectOption('issues.edited');
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
    const links:Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;
      actorFilter:string;eventKind:string;mode:'off'|'observe';createdAt:number}>=[];
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
        teamSnapshot:{snapshotVersion:1,provenance:'native',orchestrationId:'one',teamVersion:1,
          name:'One',projectId:'fixture',coordinatorBotId:'router',workerBotIds:['worker'],reviewerBotIds:[],
          policy:{policyRevision:1,autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,councilEnabled:false},
          roles:[{role:'coordinator',ordinal:0,botId:'router',effectiveConfig:null,effectiveConfigSha256:null},
            {role:'worker',ordinal:0,botId:'worker',effectiveConfig:null,effectiveConfigSha256:null}]},
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
          actorFilter:'trusted',eventKind:'issues.opened',mode:'off' as const,
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
  await expect(host.getByText('Off · no GitHub request')).toBeVisible();
  await expect(host.getByRole('button',{name:/Fetch one latest matching GitHub event/})).toHaveCount(0);
  await expect.poll(()=>page.evaluate(
    ()=>(window as unknown as {saDispatchCount:()=>number}).saDispatchCount())).toBe(0);
});


test('manual Observe fetch supports issues.edited and check_run.completed with dedup receipts and no effects', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-observe-fetch-test';document.body.appendChild(host);
    const sources=[
      {id:'source-edit',orchestrationId:'one',repository:'test/fixture',actorFilter:'trusted-editor',
        eventKind:'issues.edited',mode:'observe' as const,policyRevision:2,lastSequence:0},
      {id:'source-check',orchestrationId:'one',repository:'test/fixture',actorFilter:'trusted-checker',
        eventKind:'check_run.completed',mode:'observe' as const,policyRevision:2,lastSequence:0},
    ];
    const ticket={id:'ticket-one',objective:'Private fixture',state:'queued',revision:1,evidence:null};
    const links=[
      {sourceId:'source-edit',ticketId:'ticket-one',repository:'test/fixture',
        subjectKey:'issue:42',actorFilter:'trusted-editor',eventKind:'issues.edited',mode:'observe',createdAt:1},
      {sourceId:'source-check',ticketId:'ticket-one',repository:'test/fixture',
        subjectKey:'pull:43',actorFilter:'trusted-checker',eventKind:'check_run.completed',mode:'observe',createdAt:2},
    ];
    const fetchCalls:Array<{sourceId:unknown;subjectKey:unknown}>=[];
    const seen=new Set<string>();
    const receipts:Array<Record<string,unknown>>=[];
    let launches=0;
    mountSuperagent(host,async (command,args)=>{
      if(command==='bots_list')return [{id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [{id:'one',name:'One',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return {ticket,
        teamSnapshot:{snapshotVersion:1,provenance:'legacy-unknown',orchestrationId:'one',teamVersion:3,
          name:'Legacy Team (historical configuration unknown)',projectId:'fixture',
          coordinatorBotId:'legacy-unknown-coordinator:one:3',workerBotIds:['worker-old'],reviewerBotIds:[],
          policy:{policyRevision:1,autonomy:'unknown',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,councilEnabled:false},
          roles:[{role:'coordinator',ordinal:0,botId:'legacy-unknown-coordinator:one:3',
            effectiveConfig:null,effectiveConfigSha256:null},
            {role:'worker',ordinal:0,botId:'worker-old',effectiveConfig:null,effectiveConfigSha256:null}]},
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,projectId:'fixture',
          expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[]} as never;
      if(command==='superagent_source_thread_links')return links as never;
      if(command==='superagent_source_list')return sources as never;
      if(command==='superagent_source_fetch_thread'){
        fetchCalls.push({sourceId:args?.sourceId,subjectKey:args?.subjectKey});
        const source=sources.find(item=>item.id===args?.sourceId);
        if(!source)throw new Error('Unknown source');
        const subjectKey=String(args?.subjectKey);
        const key=source.id+':'+subjectKey;
        const duplicate=seen.has(key);
        if(!duplicate){
          seen.add(key);
          receipts.push({sourceId:source.id,
            deliveryId:source.eventKind==='issues.edited'?'gh-issues-edited-abc123':'gh-check_run-completed-9876',
            actor:source.actorFilter,eventKind:source.eventKind,subjectKey,
            sequence:source.eventKind==='issues.edited'?1738555506789:9876,
            decision:'observed_permitted',policyRevision:source.policyRevision,at:Date.now()});
        }
        return {decision:'observed_permitted',duplicate} as never;
      }
      if(command==='superagent_source_decisions')return receipts.filter(row=>row.sourceId===args?.sourceId) as never;
      launches++;
      throw new Error('Unexpected launch/effect command: '+command);
    });
    (window as unknown as {saObserveSnapshot:()=>unknown;saLaunches:()=>number}).saObserveSnapshot=()=>({fetchCalls,receipts});
    (window as unknown as {saObserveSnapshot:()=>unknown;saLaunches:()=>number}).saLaunches=()=>launches;
  });
  const host=page.locator('#sa-observe-fetch-test');
  await host.getByRole('button',{name:'View evidence'}).click();
  const evidence=host.getByRole('region',{name:'Ticket evidence'});
  await expect(evidence).toContainText('legacy-unknown');
  await expect(evidence).toContainText('worker-old');
  await expect(evidence.getByRole('note')).toContainText('not recovered facts');
  const editFetch=host.getByRole('button',{name:'Fetch one latest matching GitHub event for test/fixture issue:42'});
  const checkFetch=host.getByRole('button',{name:'Fetch one latest matching GitHub event for test/fixture pull:43'});
  await expect(editFetch).toBeVisible();
  await expect(checkFetch).toBeVisible();
  await editFetch.click();
  const decisions=host.getByRole('region',{name:'Source decisions'});
  await expect(decisions).toContainText('issues.edited');
  await expect(decisions).toContainText('observed_permitted');
  await editFetch.click();
  await checkFetch.click();
  await expect(decisions).toContainText('check_run.completed');
  await checkFetch.click();
  const snapshot=await page.evaluate(()=>(window as unknown as {saObserveSnapshot:()=>{
    fetchCalls:Array<{sourceId:string;subjectKey:string}>;receipts:Array<Record<string,unknown>>
  }}).saObserveSnapshot());
  expect(snapshot.fetchCalls).toEqual([
    {sourceId:'source-edit',subjectKey:'issue:42'},
    {sourceId:'source-edit',subjectKey:'issue:42'},
    {sourceId:'source-check',subjectKey:'pull:43'},
    {sourceId:'source-check',subjectKey:'pull:43'},
  ]);
  expect(snapshot.receipts.map(receipt=>receipt.eventKind).sort()).toEqual(['check_run.completed','issues.edited']);
  expect(snapshot.receipts.map(receipt=>receipt.decision)).toEqual(['observed_permitted','observed_permitted']);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {saLaunches:()=>number}).saLaunches())).toBe(0);
});

test('SuperAgent Preview lists manual Observe support without sending or recording events', async ({page}) => {
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    return await invokeSuperagent<{previewOnly:boolean;message:string}>(
      'superagent_source_fetch_thread',{},async()=>[]);
  });
  expect(result).toMatchObject({previewOnly:true});
  expect(result.message).toContain('issues.edited');
  expect(result.message).toContain('check_run.completed');
  expect(result.message).toContain('No GitHub request was sent');
  expect(result.message).toContain('no event or decision receipt was stored');
});
