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

async function mountCouncilTicket(page: Page, options: {
  hostId: string; councilEnabled: boolean; councilRequired: boolean;
  council: {
    nativeStatus: string; nativeRound: number; nativeStale: boolean;
    nativePassVerified: boolean; ticketArtifactBound: boolean;
  } | null;
}) {
  await page.evaluate(async options => {
    const { mountSuperagent } = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = options.hostId;
    document.body.appendChild(host);
    const ticket = {
      id:'ticket-one', objective:'Review artifact',
      state:options.councilRequired ? 'waiting_review' : 'waiting_approval',
      revision:2, evidence:'worker evidence',
    };
    const policy = {
      policyRevision:1, autonomy:'off', maxConcurrentTasks:1, maxAttemptsPerAssignment:2,
      councilEnabled:options.councilEnabled, councilRequired:options.councilRequired,
    };
    mountSuperagent(host, async command => {
      if (command === 'bots_list') return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command === 'superagent_list') return [{
        id:'preset',name:'Council example',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:2,
      }] as never;
      if (command === 'superagent_tickets') return [ticket] as never;
      if (command === 'superagent_source_thread_links') return [] as never;
      if (command === 'superagent_work_items') return [] as never;
      if (command === 'superagent_snapshot') return {
        ticket,
        teamSnapshot:{snapshotVersion:1,provenance:'native',orchestrationId:'preset',teamVersion:1,
          name:'Council example',projectId:'fixture',coordinatorBotId:'router',workerBotIds:['worker'],
          reviewerBotIds:[],policy,
          roles:[{role:'coordinator',ordinal:0,botId:'router',effectiveConfig:null,effectiveConfigSha256:null},
            {role:'worker',ordinal:0,botId:'worker',effectiveConfig:null,effectiveConfigSha256:null}]},
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:2,
          projectId:'fixture',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],council:options.council,
      } as never;
      throw new Error('Unexpected Council command: ' + command);
    });
  }, options);
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
  const plan=details.getByRole('region',{name:'Plan / Readiness'});
  await expect(plan.locator('.sa-work-items-list .sa-item')).toContainText('Inspect a disposable fixture');
  await expect(plan.locator('.sa-work-items-list .sa-item')).toContainText('Ordinal 0 · State running');
  await expect(plan.locator('.sa-work-items-list .sa-item')).toContainText('Readiness Unavailable');
  await expect(plan.locator('.sa-work-items-list .sa-item')).toContainText('Prerequisite links unavailable');
  await expect(details).toContainText('admitted');
  await expect(details.getByRole('heading',{name:'Native activity',exact:true}).locator('xpath=following-sibling::ol[1]/li').first()).toContainText(' · admitted · ');
  await row.getByRole('button',{name:'Stop'}).click();
  await workspace.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(row).toContainText('stopped');
  await expect(workspace.locator('.sa-list .sa-counts')).toHaveText('1 tasks · 0 running · 0 blocked');
  await expect(details).toContainText('Revoked');
  await expect(details).toContainText('stopped');
  await expect(plan.locator('.sa-work-items-list .sa-item')).toContainText('Ordinal 0 · State blocked');
  await expect(row.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
});

test('SuperAgent scoped-write request validates paths and reports native error and success',async ({page})=>{
  test.setTimeout(60_000);
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

test('Preview retry keeps a stopped Ticket terminal and its WorkItem blocked',async ({page})=>{
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'preview-project'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'preview-project'},
    ];
    const config=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:{name:'Preview retry',coordinatorBotId:'coordinator',workerBotId:'worker',
        workerBotIds:['worker'],reviewerBotIds:['reviewer-one','reviewer-two'],councilRequired:true}},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:config.id,expectedRevision:config.policyRevision,enabled:true,
    },listBots);
    const ticket=await invokeSuperagent<{id:string;revision:number}>(
      'superagent_submit',{orchestrationId:config.id,objective:'Retry one recorded objective'},listBots);
    const initialItems=await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_work_items',{ticketId:ticket.id},listBots);
    const stopped=await invokeSuperagent<{state:string;revision:number}>(
      'superagent_stop',{ticketId:ticket.id},listBots);
    const stoppedItems=await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_work_items',{ticketId:ticket.id},listBots);
    const assignmentsBeforeRetry=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_assignments',{ticketId:ticket.id},listBots)).map(assignment=>({...assignment}));
    let retryError='';
    try { await invokeSuperagent('superagent_retry',{ticketId:ticket.id},listBots); }
    catch(cause) { retryError=cause instanceof Error?cause.message:String(cause); }
    const ticketsAfterRetry=await invokeSuperagent<Array<{id:string;state:string;revision:number}>>(
      'superagent_tickets',{orchestrationId:config.id},listBots);
    const workItemsAfterRetry=await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_work_items',{ticketId:ticket.id},listBots);
    const assignmentsAfterRetry=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_assignments',{ticketId:ticket.id},listBots)).map(assignment=>({...assignment}));
    const snapshot=await invokeSuperagent<Record<string,any>>(
      'superagent_snapshot',{ticketId:ticket.id},listBots);
    return {ticketId:ticket.id,initialItems,stoppedState:stopped.state,
      stoppedRevision:stopped.revision,stoppedItems,retryError,
      stateAfterRetry:ticketsAfterRetry.find(item=>item.id===ticket.id)?.state,
      revisionAfterRetry:ticketsAfterRetry.find(item=>item.id===ticket.id)?.revision,
      workItemsAfterRetry,assignmentsBeforeRetry,assignmentsAfterRetry,
      snapshotHasCouncil:'council' in snapshot};
  });

  expect(result.initialItems).toHaveLength(1);
  expect(result.initialItems[0]).toMatchObject({ticketId:result.ticketId,
    objective:'Retry one recorded objective',state:'running',ordinal:0});
  expect(result.stoppedState).toBe('stopped');
  expect(result.retryError).toBe('Only a blocked SuperAgent ticket can be retried manually.');
  expect(result.stoppedItems[0]).toMatchObject({id:result.initialItems[0].id,state:'blocked'});
  expect(result.stateAfterRetry).toBe('stopped');
  expect(result.revisionAfterRetry).toBe(result.stoppedRevision);
  expect(result.workItemsAfterRetry).toHaveLength(1);
  expect(result.workItemsAfterRetry[0]).toMatchObject({id:result.initialItems[0].id,
    objective:'Retry one recorded objective',state:'blocked',ordinal:0});
  expect(result.workItemsAfterRetry[0]).not.toHaveProperty('dependsOn');
  expect(result.workItemsAfterRetry[0]).not.toHaveProperty('council');
  expect(result.assignmentsBeforeRetry).toHaveLength(1);
  expect(result.assignmentsAfterRetry).toEqual(result.assignmentsBeforeRetry);
  expect(result.assignmentsAfterRetry[0]).toMatchObject({attempt:1,state:'stopped'});
  expect(result.snapshotHasCouncil).toBe(false);
});

test('Preview coordinated submission fails closed and preserves direct preview',async ({page})=>{
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
    ];
    const config=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:{name:'Preview coordinated admission',
        coordinatorBotId:'coordinator',workerBotId:'worker'}},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:config.id,expectedRevision:config.policyRevision,enabled:true,
    },listBots);
    const directTicket=await invokeSuperagent<Record<string,any>>('superagent_submit',{
      orchestrationId:config.id,objective:'Keep direct preview available',
    },listBots);
    const ticketsBefore=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_tickets',{orchestrationId:config.id},listBots)).map(ticket=>({...ticket}));
    const assignmentsBefore=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_assignments',{ticketId:directTicket.id},listBots)).map(assignment=>({...assignment}));
    const workItemsBefore=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_work_items',{ticketId:directTicket.id},listBots)).map(item=>({...item}));
    let coordinatedSubmitError='';
    try {
      await invokeSuperagent('superagent_submit_coordinated',{
        orchestrationId:config.id,objective:'Plan a bounded preview task',
      },listBots);
    } catch(cause) { coordinatedSubmitError=cause instanceof Error?cause.message:String(cause); }
    const ticketsAfter=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_tickets',{orchestrationId:config.id},listBots)).map(ticket=>({...ticket}));
    const assignmentsAfter=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_assignments',{ticketId:directTicket.id},listBots)).map(assignment=>({...assignment}));
    const workItemsAfter=(await invokeSuperagent<Array<Record<string,any>>>(
      'superagent_work_items',{ticketId:directTicket.id},listBots)).map(item=>({...item}));
    return {directTicket,ticketsBefore,assignmentsBefore,workItemsBefore,
      coordinatedSubmitError,ticketsAfter,assignmentsAfter,workItemsAfter};
  });

  expect(result.coordinatedSubmitError).toBe(
    'Preview cannot run the Coordinator. Coordinated Ticket submission is unavailable.');
  expect(result.directTicket).toMatchObject({state:'running',revision:0,evidence:null});
  expect(result.ticketsBefore).toHaveLength(1);
  expect(result.ticketsAfter).toEqual(result.ticketsBefore);
  expect(result.assignmentsBefore).toHaveLength(1);
  expect(result.assignmentsBefore[0]).toMatchObject({botId:'worker',attempt:1,state:'running'});
  expect(result.assignmentsAfter).toEqual(result.assignmentsBefore);
  expect(result.workItemsBefore).toHaveLength(1);
  expect(result.workItemsBefore[0]).toMatchObject({objective:'Keep direct preview available',state:'running',
    evidenceContract:'Preview-only informational description: record the observable result for "Keep direct preview available".'});
  expect(result.workItemsAfter).toEqual(result.workItemsBefore);
});

test('Coordinated preview submission shows a visible error without creating a Ticket',async ({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
    ];
    const config=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:{name:'Preview confirmation',coordinatorBotId:'coordinator',workerBotId:'worker'}},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:config.id,expectedRevision:config.policyRevision,enabled:true,
    },listBots);
    const host=document.createElement('div');host.id='sa-preview-dispatch-confirmation';document.body.appendChild(host);
    const call=async<T>(command:string,args:Record<string,unknown>={})=>{
      if(command==='bots_list')return await listBots() as T;
      return await invokeSuperagent<T>(command,args,listBots);
    };
    mountSuperagent(host,call);
    (window as unknown as {saPreviewDispatchState:()=>Promise<Record<string,any>>}).saPreviewDispatchState=async()=>({
      tickets:await invokeSuperagent<Array<Record<string,any>>>('superagent_tickets',
        {orchestrationId:config.id},listBots),
      presets:await invokeSuperagent<Array<Record<string,any>>>('superagent_list',{},listBots),
    });
  });

  const host=page.locator('#sa-preview-dispatch-confirmation');
  const runForm=host.locator('form[data-sa-form="run"]');
  await expect(runForm).toBeVisible();
  await runForm.getByRole('textbox',{name:'Task objective'}).fill('Preview confirmation objective');
  await runForm.getByRole('combobox',{name:'Dispatch'}).selectOption('coordinated');
  await runForm.getByRole('button',{name:'Dispatch manually'}).click();
  await expect(host.getByRole('alert')).toContainText('Preview cannot run the Coordinator.');
  await expect(host.locator('.sa-tickets')).toContainText('No tickets yet.');
  const state=await page.evaluate(()=>(window as unknown as {
    saPreviewDispatchState:()=>Promise<Record<string,any>>
  }).saPreviewDispatchState());
  expect(state.tickets).toEqual([]);
  expect(state.presets[0]).toMatchObject({ticketCount:0,runningCount:0,blockedCount:0});
});

test('Preview retry creates a fresh correction item after human rejection',async ({page})=>{
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {preparePreviewRetryState}=await import('/src/preview-superagent.ts');
    const objective='Correct the recorded preview result';
    const originalItem={id:'original-item',ticketId:'ticket-rejected',objective,
      evidenceContract:'Check and report the corrected observable result.',
      state:'completed',ordinal:0,createdAt:1,updatedAt:2};
    const originalAssignment={id:'original-assignment',botId:'worker',sessionId:'worker-session',
      attempt:1,state:'finished'};
    const transition=preparePreviewRetryState({ticketId:'ticket-rejected',objective,
      workerBotId:'worker',maxAttemptsPerAssignment:2,items:[originalItem],
      assignments:[originalAssignment],latestActivity:{id:2,kind:'human_rejected',
        detail:'Preview-only state transition',at:3},now:4});
    let unrelatedCompletedItemError='';
    try {
      preparePreviewRetryState({ticketId:'ticket-rejected',objective,workerBotId:'worker',
        maxAttemptsPerAssignment:2,items:[originalItem],assignments:[originalAssignment],
        latestActivity:{id:3,kind:'manual_retry',detail:'not a rejection',at:5},now:6});
    } catch(cause) { unrelatedCompletedItemError=cause instanceof Error?cause.message:String(cause); }
    return {objective,originalItem,originalAssignment,transition,unrelatedCompletedItemError};
  });

  expect(result.unrelatedCompletedItemError).toContain('explicitly rejected completed WorkItem');
  expect(result.originalItem).toMatchObject({objective:'Correct the recorded preview result',
    state:'completed',ordinal:0,createdAt:1,updatedAt:2});
  expect(result.originalAssignment).toMatchObject({attempt:1,state:'finished'});
  expect(result.transition.workItems).toHaveLength(2);
  expect(result.transition.workItems[0]).toEqual(result.originalItem);
  expect(result.transition.workItems[1]).toMatchObject({ticketId:'ticket-rejected',
    objective:'Correct the recorded preview result',
    evidenceContract:'Check and report the corrected observable result.',
    state:'running',ordinal:1,createdAt:4,updatedAt:4});
  expect(result.transition.workItems[1].id).not.toBe(result.originalItem.id);
  expect(result.transition.assignment).toMatchObject({botId:'worker',sessionId:'preview-session-ticket-rejected-attempt-2',
    attempt:2,state:'running'});
  expect(result.transition.assignment.id).not.toBe(result.originalAssignment.id);
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
  test.setTimeout(60_000);
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
      if(command==='superagent_work_items')return [] as never;
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
  await expect(commentForm.getByRole('spinbutton',{name:'Issue number'})).toHaveValue('42');
  await expect(comment).toHaveValue('Reviewed; the fix is ready.');
  await commentForm.getByRole('button',{name:'Authorize and post comment'}).click();
  await expect(host.getByRole('status').filter({hasText:'uncertain · readback-unavailable'})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saIssueCommentCalls:Array<unknown>;
  }).saIssueCommentCalls.length)).toBe(1);
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

test('Team Builder saves a future-only Team version for an enabled Team with Ticket history', async ({page}) => {
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
    const ticket={id:'ticket-running',objective:'Active Ticket',state:'running',revision:2,evidence:null};
    let config:Record<string,any>={id:'team-future-version',name:'Enabled Team',
      coordinatorBotId:'coordinator',workerBotId:'worker-one',workerBotIds:['worker-one'],
      reviewerBotIds:['reviewer'],enabled:true,policyRevision:6,teamVersion:3,
      projectId:'project-one',councilRequired:false};
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [{...config,ticketCount:1,runningCount:1,
        blockedCount:0}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_update'){
        calls.push({command,args:{...args}});
        const input=args?.input as Record<string,any>;
        config={...config,...input,teamVersion:Number(config.teamVersion)+1,
          policyRevision:Number(config.policyRevision)+1};
        return config as never;
      }
      throw new Error('Unexpected enabled Team edit command: '+command);
    });
    (window as unknown as {saFutureVersionCalls:typeof calls}).saFutureVersionCalls=calls;
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await expect(host.locator('.sa-list .sa-item .sa-counts')).toHaveText('1 tasks · 1 running · 0 blocked');
  await expect(host.getByText('Active Ticket', {exact:true})).toBeVisible();
  await expect(host.getByRole('button',{name:'Edit Team'})).toBeVisible();
  await host.getByRole('button',{name:'Edit Team'}).click();
  const edit=selected.locator('form[data-sa-form="edit"]');
  await expect(edit.getByRole('button',{name:'Save Team version',exact:true})).toBeEnabled();
  const contract='Saving creates a new Team version used by future tasks. Active Tickets keep their saved Team snapshot, grants and roles.';
  await expect(selected.getByText(contract,{exact:true})).toBeVisible();
  await edit.getByRole('textbox',{name:'Team name',exact:true}).fill('Enabled Team renamed');
  await edit.getByRole('button',{name:'Add worker'}).click();
  await edit.getByRole('combobox',{name:'Worker 2'}).selectOption('worker-two');
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  await expect(selected.getByRole('textbox',{name:'Team name',exact:true})).toHaveValue('Enabled Team renamed');
  const calls=await page.evaluate(()=>(window as unknown as {
    saFutureVersionCalls:Array<{command:string;args?:Record<string,unknown>}>
  }).saFutureVersionCalls);
  expect(calls).toEqual([{command:'superagent_update',args:{
    orchestrationId:'team-future-version',expectedRevision:6,input:{
      name:'Enabled Team renamed',coordinatorBotId:'coordinator',
      workerBotIds:['worker-one','worker-two'],reviewerBotIds:['reviewer'],
      workerBotId:'worker-one',councilRequired:false,
    },
  }}]);
  await expect(host.getByText('Active Ticket', {exact:true})).toBeVisible();
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

test('required Council reports a bound verified native review without an advisory claim', async ({page}) => {
  await page.goto('/?bots=sample');
  await mountCouncilTicket(page, {
    hostId:'sa-council-required-bound', councilEnabled:true, councilRequired:true,
    council:{nativeStatus:'passed',nativeRound:2,nativeStale:false,
      nativePassVerified:true,ticketArtifactBound:true},
  });
  const host = page.locator('#sa-council-required-bound');
  await expect(host.getByRole('button',{name:'View evidence'})).toBeVisible();
  await host.getByRole('button',{name:'View evidence'}).click();
  const note = host.getByRole('note',{name:'Native Council observation'});
  await expect(host.getByRole('heading',{name:'Native Council',exact:true})).toBeVisible();
  await expect(note).toContainText('Policy: Council required. Review target binding: Bound to this Ticket artifact.');
  await expect(note).toContainText('Native team status: passed · Round 2. Verified native pass: Yes.');
  await expect(note).toContainText('Required Council gates completion for this Ticket.');
  await expect(note).toContainText('The artifact is bound as the native review target; a native Council pass is verified.');
  await expect(note).not.toContainText('(advisory)');
  await expect(note).not.toContainText('Not bound to this ticket artifact');
});

test('required Council reports an unproven task binding as Unknown, not advisory', async ({page}) => {
  await page.goto('/?bots=sample');
  await mountCouncilTicket(page, {
    hostId:'sa-council-required-unbound', councilEnabled:true, councilRequired:true,
    council:{nativeStatus:'unverified',nativeRound:1,nativeStale:false,
      nativePassVerified:false,ticketArtifactBound:false},
  });
  const host = page.locator('#sa-council-required-unbound');
  await host.getByRole('button',{name:'View evidence'}).click();
  const note = host.getByRole('note',{name:'Native Council observation'});
  await expect(note).toContainText('Policy: Council required. Review target binding: Not proven (Unknown).');
  await expect(note).toContainText('Native team status: unverified · Round 1. Verified native pass: No.');
  await expect(note).toContainText('Neither a verified native Council pass nor a bound review target is proven');
  await expect(note).not.toContainText('(advisory)');
});

test('required Council reports an unavailable native observation instead of advisory', async ({page}) => {
  await page.goto('/?bots=sample');
  await mountCouncilTicket(page, {
    hostId:'sa-council-required-unavailable', councilEnabled:true, councilRequired:true, council:null,
  });
  const host = page.locator('#sa-council-required-unavailable');
  await host.getByRole('button',{name:'View evidence'}).click();
  const note = host.getByRole('note',{name:'Native Council observation'});
  await expect(note).toContainText('Policy: Council required.');
  await expect(note).toContainText('native Council observation is unavailable');
  await expect(note).toContainText('a bound native review target is Unknown');
  await expect(note).not.toContainText('(advisory)');
});

test('a snapshot with only councilEnabled true is treated as required, never advisory', async ({page}) => {
  await page.goto('/?bots=sample');
  await mountCouncilTicket(page, {
    hostId:'sa-council-enabled-only', councilEnabled:true, councilRequired:false,
    council:{nativeStatus:'unverified',nativeRound:1,nativeStale:true,
      nativePassVerified:false,ticketArtifactBound:false},
  });
  const host = page.locator('#sa-council-enabled-only');
  await host.getByRole('button',{name:'View evidence'}).click();
  const note = host.getByRole('note',{name:'Native Council observation'});
  await expect(host.getByRole('heading',{name:'Native Council',exact:true})).toBeVisible();
  await expect(note).toContainText('Policy: Council required. Review target binding: Not proven (Unknown).');
  await expect(note).toContainText('Native team status: unverified · Round 1 · stale. Verified native pass: No.');
  await expect(note).toContainText('Required Council gates completion for this Ticket.');
  await expect(note).not.toContainText('(advisory)');
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
  }).saTeamCalls);
  expect(calls).toHaveLength(1);
  expect(calls[0].command).toBe('superagent_create');
  expect(calls[0].args?.input).toMatchObject({
    name:'Ordered Team',coordinatorBotId:'coordinator',workerBotId:'worker-two',
    workerBotIds:['worker-two','worker-one'],reviewerBotIds:['reviewer-two','reviewer-one'],
    councilRequired:false,
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  await selected.getByRole('button',{name:'Move worker 2 up'}).click();
  await selected.getByRole('button',{name:'Move reviewer 2 up'}).click();
  await selected.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  calls=await page.evaluate(()=>(window as unknown as {
    saTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saTeamCalls);
  expect(calls).toHaveLength(2);
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
      enabled:false,policyRevision:9,councilRequired:false};
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
      councilRequired:false,
    },
  }}]);
});

test('Team Builder sends required Council policy with create and update and shows the saved state', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker',name:'Worker',paused:false,projectId:'project-one'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    let config:Record<string,any>|null=null;
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return config
        ? [{...config,ticketCount:0,runningCount:0,blockedCount:0}] as never : [] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'||command==='superagent_update'){
        calls.push({command,args:{...args}});
        const input=args?.input as Record<string,any>;
        config=command==='superagent_create'
          ? {id:'team-council',...input,teamVersion:1,projectId:'project-one',enabled:false,policyRevision:1}
          : {...config,...input,teamVersion:Number(config?.teamVersion)+1,
            policyRevision:Number(config?.policyRevision)+1};
        return config as never;
      }
      throw new Error('Unexpected Council Team command: '+command);
    });
    (window as unknown as {saCouncilTeamCalls:typeof calls}).saCouncilTeamCalls=calls;
  });
  const create=host.locator('form[data-sa-form="create"]');
  const createCouncil=create.getByRole('checkbox',{name:'Require native Council review'});
  await expect(createCouncil).not.toBeChecked();
  await expect(create.getByRole('combobox',{name:'Coordinator'})
    .locator('option[value="coordinator"]')).toBeAttached();
  await create.getByRole('textbox',{name:'Name',exact:true}).fill('Council Team');
  await create.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await create.getByRole('combobox',{name:'Worker 1',exact:true}).selectOption('worker');
  await create.getByRole('button',{name:'Add reviewer'}).click();
  await create.getByRole('combobox',{name:'Reviewer 1'}).selectOption('reviewer-one');
  await create.getByRole('button',{name:'Add reviewer'}).click();
  await create.getByRole('combobox',{name:'Reviewer 2'}).selectOption('reviewer-two');
  await createCouncil.check();
  await create.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.locator('.sa-list .sa-item [data-sa-council-state]')).toHaveText('Council required');
  let calls=await page.evaluate(()=>(window as unknown as {
    saCouncilTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saCouncilTeamCalls);
  expect(calls).toHaveLength(1);
  expect(calls[0]).toMatchObject({command:'superagent_create',args:{input:{
    name:'Council Team',coordinatorBotId:'coordinator',workerBotId:'worker',
    workerBotIds:['worker'],reviewerBotIds:['reviewer-one','reviewer-two'],
    councilRequired:true,
  }}});

  await host.getByRole('button',{name:'Edit Team'}).click();
  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  const edit=selected.locator('form[data-sa-form="edit"]');
  const editCouncil=edit.getByRole('checkbox',{name:'Require native Council review'});
  await expect(editCouncil).toBeChecked();
  await edit.getByRole('textbox',{name:'Team name',exact:true}).fill('Council Team renamed');
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  calls=await page.evaluate(()=>(window as unknown as {
    saCouncilTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saCouncilTeamCalls);
  expect(calls).toHaveLength(2);
  expect(calls[1]).toMatchObject({command:'superagent_update',args:{
    orchestrationId:'team-council',expectedRevision:1,input:{
      name:'Council Team renamed',councilRequired:true,
      reviewerBotIds:['reviewer-one','reviewer-two'],
    },
  }});
  await expect(host.locator('.sa-list .sa-item [data-sa-council-state]')).toHaveText('Council required');

  await editCouncil.uncheck();
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  calls=await page.evaluate(()=>(window as unknown as {
    saCouncilTeamCalls:Array<{command:string;args?:Record<string,any>}>
  }).saCouncilTeamCalls);
  expect(calls).toHaveLength(3);
  expect(calls[2]).toMatchObject({command:'superagent_update',args:{
    orchestrationId:'team-council',expectedRevision:2,input:{
      name:'Council Team renamed',councilRequired:false,
      reviewerBotIds:['reviewer-one','reviewer-two'],
    },
  }});
  await expect(host.locator('.sa-list .sa-item [data-sa-council-state]')).toHaveText('Council off');
});

test('Team Builder field-validates required Council with fewer than two reviewers before create or update', async ({page}) => {
  // This regression covers three live invalid-form cycles. CI exhausted the
  // whole-test deadline during the final edit toggle after the locator resolved.
  test.setTimeout(120_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker',name:'Worker',paused:false,projectId:'project-one'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    const config={id:'team-existing',name:'Existing Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',workerBotIds:['worker'],reviewerBotIds:['reviewer-one'],
      enabled:false,policyRevision:3,teamVersion:1,projectId:'project-one',councilRequired:false};
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [{...config,ticketCount:0,runningCount:0,blockedCount:0}] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'||command==='superagent_update'){
        calls.push({command,args:{...args}});
        throw new Error('Invalid Council requirement must not reach native.');
      }
      throw new Error('Unexpected invalid Council command: '+command);
    });
    (window as unknown as {saInvalidCouncilCalls:typeof calls}).saInvalidCouncilCalls=calls;
  });
  const create=host.locator('form[data-sa-form="create"]');
  await expect(create.getByRole('combobox',{name:'Coordinator'})
    .locator('option[value="coordinator"]')).toBeAttached();
  await create.getByRole('textbox',{name:'Name',exact:true}).fill('Invalid Council Team');
  await create.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await create.getByRole('combobox',{name:'Worker 1',exact:true}).selectOption('worker');
  await create.getByRole('button',{name:'Add reviewer'}).click();
  await create.getByRole('combobox',{name:'Reviewer 1'}).selectOption('reviewer-one');
  const createCouncil=create.getByRole('checkbox',{name:'Require native Council review'});
  await createCouncil.check();
  await create.getByRole('button',{name:'Create disabled preset'}).click();
  const createError=create.locator('[data-sa-council-error]');
  await expect(createError).toHaveText('Require native Council review needs between two and eight distinct reviewers.');
  await expect(createError).toHaveAttribute('role','alert');
  await expect(createCouncil).toHaveAttribute('aria-invalid','true');
  await expect(createCouncil).toHaveAttribute('aria-describedby',/sa-create-council-error/);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saInvalidCouncilCalls:Array<unknown>;
  }).saInvalidCouncilCalls.length)).toBe(0);
  await createCouncil.uncheck();
  await expect(create.locator('[data-sa-council-error]')).toHaveCount(0);
  await expect(createCouncil).not.toHaveAttribute('aria-invalid','true');

  // A duplicate reviewer is not a distinct native reviewer and must fail in the
  // form before any create call can reach native policy.
  await create.getByRole('button',{name:'Add reviewer'}).click();
  await create.getByRole('combobox',{name:'Reviewer 2'}).selectOption('reviewer-one');
  await createCouncil.check();
  await create.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(create.locator('[data-sa-council-error]'))
    .toHaveText('Require native Council review needs between two and eight distinct reviewers.');
  await expect(createCouncil).toHaveAttribute('aria-invalid','true');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saInvalidCouncilCalls:Array<unknown>;
  }).saInvalidCouncilCalls.length)).toBe(0);

  await host.getByRole('button',{name:'Edit Team'}).click();
  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  const edit=selected.locator('form[data-sa-form="edit"]');
  const editCouncil=edit.getByRole('checkbox',{name:'Require native Council review'});
  await expect(editCouncil).not.toBeChecked();
  await editCouncil.check();
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  const editError=edit.locator('[data-sa-council-error]');
  await expect(editError).toHaveText('Require native Council review needs between two and eight distinct reviewers.');
  await expect(editCouncil).toHaveAttribute('aria-invalid','true');
  await expect(editCouncil).toHaveAttribute('aria-describedby',/sa-edit-council-error/);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saInvalidCouncilCalls:Array<unknown>;
  }).saInvalidCouncilCalls.length)).toBe(0);
  await expect(edit.getByRole('combobox',{name:'Reviewer 1',exact:true})).toHaveValue('reviewer-one');
});

test('Team Builder clears the Council error and saves once the second distinct reviewer is added', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker',name:'Worker',paused:false,projectId:'project-one'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    let config:Record<string,any>={id:'team-fix-council',name:'Fix Council',
      coordinatorBotId:'coordinator',workerBotId:'worker',workerBotIds:['worker'],
      reviewerBotIds:['reviewer-one'],enabled:false,policyRevision:5,teamVersion:1,
      projectId:'project-one',councilRequired:false};
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
      throw new Error('Unexpected Council fix command: '+command);
    });
    (window as unknown as {saCouncilFixCalls:typeof calls}).saCouncilFixCalls=calls;
  });

  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  const edit=selected.locator('form[data-sa-form="edit"]');
  const editCouncil=edit.getByRole('checkbox',{name:'Require native Council review'});
  await expect(editCouncil).not.toBeChecked();
  await editCouncil.check();
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  const error=edit.locator('[data-sa-council-error]');
  await expect(error).toHaveText('Require native Council review needs between two and eight distinct reviewers.');
  await expect(editCouncil).toHaveAttribute('aria-invalid','true');
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saCouncilFixCalls:Array<unknown>;
  }).saCouncilFixCalls.length)).toBe(0);

  await edit.getByRole('button',{name:'Add reviewer'}).click();
  await edit.getByRole('combobox',{name:'Reviewer 2'}).selectOption('reviewer-two');
  await expect(edit.locator('[data-sa-council-error]')).toHaveCount(0);
  await expect(editCouncil).not.toHaveAttribute('aria-invalid','true');
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saCouncilFixCalls:Array<unknown>;
  }).saCouncilFixCalls.length)).toBe(1);
  await expect(host.locator('.sa-list .sa-item [data-sa-council-state]')).toHaveText('Council required');
  const calls=await page.evaluate(()=>(window as unknown as {
    saCouncilFixCalls:Array<{command:string;args?:Record<string,any>}>
  }).saCouncilFixCalls);
  expect(calls).toHaveLength(1);
  expect(calls[0]).toMatchObject({command:'superagent_update',args:{
    orchestrationId:'team-fix-council',expectedRevision:5,input:{
      name:'Fix Council',coordinatorBotId:'coordinator',workerBotId:'worker',
      workerBotIds:['worker'],reviewerBotIds:['reviewer-one','reviewer-two'],
      councilRequired:true,
    },
  }});
});

test('Team Builder leaves an unknown saved Council state unchanged and omits it from the update payload', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');host.id='superagent-host';previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'project-one'},
      {id:'worker',name:'Worker',paused:false,projectId:'project-one'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'project-one'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'project-one'},
    ];
    // A list record from an older/native shape may omit councilRequired entirely.
    let config:Record<string,any>={id:'team-unknown-council',name:'Unknown Council',
      coordinatorBotId:'coordinator',workerBotId:'worker',workerBotIds:['worker'],
      reviewerBotIds:['reviewer-one','reviewer-two'],enabled:false,policyRevision:7,
      teamVersion:2,projectId:'project-one'};
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
      throw new Error('Unexpected unknown Council command: '+command);
    });
    (window as unknown as {saUnknownCouncilCalls:typeof calls}).saUnknownCouncilCalls=calls;
  });

  const state=host.locator('.sa-list .sa-item [data-sa-council-state]');
  await expect(state).toHaveText('Council state unavailable');
  const selected=host.getByRole('region',{name:'Selected orchestration',exact:true});
  await host.getByRole('button',{name:'Edit Team'}).click();
  const edit=selected.locator('form[data-sa-form="edit"]');
  const editCouncil=edit.getByRole('checkbox',{name:'Require native Council review'});
  await expect(editCouncil).not.toBeChecked();
  await edit.getByRole('textbox',{name:'Team name',exact:true}).fill('Unknown Council renamed');
  await edit.getByRole('button',{name:'Save Team version',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {
    saUnknownCouncilCalls:Array<unknown>;
  }).saUnknownCouncilCalls.length)).toBe(1);
  await expect(state).toHaveText('Council state unavailable');
  await expect(editCouncil).not.toBeChecked();

  const calls=await page.evaluate(()=>(window as unknown as {
    saUnknownCouncilCalls:Array<{command:string;args?:Record<string,any>}>
  }).saUnknownCouncilCalls);
  expect(calls).toHaveLength(1);
  const input=calls[0].args?.input as Record<string,unknown>;
  expect(input).toMatchObject({
    name:'Unknown Council renamed',workerBotId:'worker',
    reviewerBotIds:['reviewer-one','reviewer-two'],
  });
  expect(Object.prototype.hasOwnProperty.call(input,'councilRequired')).toBe(false);
});

test('Team Builder disables roles from another project', async ({page}) => {
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
  test.setTimeout(60_000);
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
      if(command==='superagent_work_items')return [] as never;
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
  test.setTimeout(60_000);
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
      if(command==='superagent_work_items')return [] as never;
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


test('Ticket Plan and Readiness shows native WorkItems with loading, empty and error states', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-work-items-test';document.body.appendChild(host);
    const ticket={id:'ticket-work-items',objective:'Render ordered WorkItems',state:'running',
      revision:1,activeAssignmentId:'assignment-first',evidence:null};
    const state:{mode:'items'|'empty'|'error';calls:Array<Record<string,unknown>>;
      waitForItems:boolean;releaseItems?:()=>void}={mode:'items',calls:[],waitForItems:true};
    mountSuperagent(host,async (command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{id:'preset-one',name:'Work Item Team',
        coordinatorBotId:'coordinator',workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return {ticket,teamSnapshot:null,
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,projectId:'fixture',
          expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
        assignments:[{id:'assignment-first',ticketId:'ticket-work-items',botId:'worker',
          sessionId:'session-first',state:'running',attempt:1,workItemId:'item-first',
          result:null,createdAt:1,updatedAt:2}],activity:[]} as never;
      if(command==='superagent_source_thread_links')return [] as never;
      if(command==='superagent_work_items'){
        state.calls.push({...args});
        if(state.mode==='error')throw new Error('Native WorkItem read failed.');
        if(state.mode==='empty')return [] as never;
        // Deliberately reversed: the native order is authoritative and the
        // section still renders ordinal 0 before later ordinals.
        const rows=[
          {id:'item-fourth',ticketId:'ticket-work-items',objective:'Fourth bounded slice',
            evidenceContract:'Record why this slice is not ready to begin.',
            state:'blocked',ordinal:3,dependsOnIds:[],readiness:'not_ready',createdAt:1,updatedAt:2},
          {id:'item-third',ticketId:'ticket-work-items',objective:'Third bounded slice',
            evidenceContract:'Record why this slice is still waiting.',
            state:'queued',ordinal:2,dependsOnIds:['item-first'],readiness:'dependency_blocked',createdAt:1,updatedAt:2},
          {id:'item-second',ticketId:'ticket-work-items',objective:'Second bounded slice',
            evidenceContract:'Record the completed second slice and its output.',
            state:'queued',ordinal:1,dependsOnIds:[],readiness:'ready',createdAt:1,updatedAt:2},
          {id:'item-first',ticketId:'ticket-work-items',objective:'First bounded slice',
            evidenceContract:'Show the first slice has produced its expected result.',
            state:'running',ordinal:0,dependsOnIds:[],readiness:'active',createdAt:1,updatedAt:2},
        ];
        if(state.waitForItems)return await new Promise(resolve=>{
          state.releaseItems=()=>resolve(rows as never);
        }) as never;
        return rows as never;
      }
      throw new Error('Unexpected WorkItem evidence command: '+command);
    });
    (window as unknown as {saWorkItemsState:typeof state}).saWorkItemsState=state;
  });

  const host=page.locator('#sa-work-items-test');
  const setMode=(mode:'items'|'empty'|'error')=>page.evaluate(mode=>{
    const state=(window as unknown as {saWorkItemsState:{mode:string;waitForItems:boolean}}).saWorkItemsState;
    state.mode=mode;state.waitForItems=false;
  },mode);
  const readCalls=()=>page.evaluate(()=>(window as unknown as {
    saWorkItemsState:{calls:Array<Record<string,unknown>>}
  }).saWorkItemsState.calls);

  await host.getByRole('button',{name:'View evidence'}).click();
  const details=host.getByRole('region',{name:'Ticket evidence'});
  const workItems=details.getByRole('region',{name:'Plan / Readiness'});
  await expect(workItems.locator('[data-sa-work-items-loading]'))
    .toHaveText('Loading WorkItems for this Ticket…');
  await expect(workItems.locator('[data-sa-plan-body]')).toBeVisible();
  await workItems.locator('summary').click();
  await expect(workItems.locator('[data-sa-plan-body]')).toBeHidden();
  await workItems.locator('summary').click();
  await expect(workItems.locator('[data-sa-plan-body]')).toBeVisible();
  expect(await readCalls()).toEqual([{ticketId:'ticket-work-items'}]);
  await page.evaluate(()=>{
    const state=(window as unknown as {saWorkItemsState:{waitForItems:boolean;releaseItems?:()=>void}}).saWorkItemsState;
    state.waitForItems=false;state.releaseItems?.();
  });
  await expect(workItems.getByRole('heading',{name:'WorkItem List',exact:true})).toBeVisible();
  await expect(workItems).toContainText('This list does not authorize or dispatch work.');
  await expect(workItems).toContainText('Evidence contracts are informational descriptions, not proof that work is complete.');
  await expect(workItems).toContainText('Prerequisite IDs and readiness come from native dependency rows and the active Assignment binding');
  const rows=workItems.locator('.sa-work-items-list .sa-item');
  await expect(rows).toHaveCount(4);
  await expect(rows.nth(0)).toContainText('First bounded slice');
  await expect(rows.nth(0)).toContainText('Ordinal 0 · State running · Readiness Active');
  await expect(rows.nth(0)).toContainText('Prerequisites: none');
  await expect(rows.nth(0)).toContainText('Evidence contract (descriptive only; not verified evidence): Show the first slice has produced its expected result.');
  await expect(rows.nth(1)).toContainText('Second bounded slice');
  await expect(rows.nth(1)).toContainText('Ordinal 1 · State queued · Readiness Ready');
  await expect(rows.nth(1)).toContainText('Prerequisites: none');
  await expect(rows.nth(1)).toContainText('Evidence contract (descriptive only; not verified evidence): Record the completed second slice and its output.');
  await expect(rows.nth(2)).toContainText('Third bounded slice');
  await expect(rows.nth(2)).toContainText('Ordinal 2 · State queued · Readiness Dependency blocked');
  await expect(rows.nth(2)).toContainText('Depends on: First bounded slice · ID item-first');
  await expect(rows.nth(3)).toContainText('Ordinal 3 · State blocked · Readiness Not ready');
  await expect(workItems.getByRole('button')).toHaveCount(0);

  await details.getByRole('button',{name:'Close evidence'}).click();
  await setMode('error');
  await host.getByRole('button',{name:'View evidence'}).click();
  const errorDetails=host.getByRole('region',{name:'Ticket evidence'});
  await expect(errorDetails.getByRole('region',{name:'Plan / Readiness'}).getByRole('alert'))
    .toHaveText('Work items unavailable: Native WorkItem read failed.');
  await expect(errorDetails.locator('.sa-work-items-list')).toHaveCount(0);
  await expect(errorDetails).toContainText('Ticket ID');

  await errorDetails.getByRole('button',{name:'Close evidence'}).click();
  await setMode('empty');
  await host.getByRole('button',{name:'View evidence'}).click();
  const emptyDetails=host.getByRole('region',{name:'Ticket evidence'});
  const emptyItems=emptyDetails.getByRole('region',{name:'Plan / Readiness'});
  await expect(emptyItems.getByText('No WorkItems recorded for this Ticket.',{exact:true})).toBeVisible();
  await expect(emptyItems.getByRole('alert')).toHaveCount(0);
  await expect(emptyItems.locator('.sa-item')).toHaveCount(0);
  expect(await readCalls()).toEqual([
    {ticketId:'ticket-work-items'},{ticketId:'ticket-work-items'},{ticketId:'ticket-work-items'},
  ]);
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
      if(command==='superagent_work_items')return [] as never;
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
      if(command==='superagent_work_items')return [] as never;
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

test('Preview SuperAgent round-trips required and off Council policies with reviewer-count validation', async ({page}) => {
  await page.goto('/?bots=sample');
  const result=await page.evaluate(async()=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
      {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
      {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'preview-project'},
      {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'preview-project'},
    ];
    const team=(name:string,reviewers:string[],councilRequired?:boolean)=>({
      name,coordinatorBotId:'coordinator',workerBotId:'worker',workerBotIds:['worker'],
      reviewerBotIds:reviewers,
      ...(councilRequired===undefined?{}:{councilRequired}),
    });
    const required=await invokeSuperagent<{id:string;policyRevision:number;councilRequired:boolean}>(
      'superagent_create',{input:team('Preview Council Required',['reviewer-one','reviewer-two'],true)},listBots);
    const listedRequired=await invokeSuperagent<Array<{id:string;councilRequired:boolean}>>(
      'superagent_list',{},listBots);
    const preserved=await invokeSuperagent<{policyRevision:number;councilRequired:boolean}>(
      'superagent_update',{orchestrationId:required.id,expectedRevision:required.policyRevision,
        input:team('Preview Council Required',['reviewer-one','reviewer-two'])},listBots);
    const listedPreserved=await invokeSuperagent<Array<{id:string;councilRequired:boolean}>>(
      'superagent_list',{},listBots);
    const off=await invokeSuperagent<{policyRevision:number;councilRequired:boolean}>(
      'superagent_update',{orchestrationId:required.id,expectedRevision:preserved.policyRevision,
        input:team('Preview Council Required',['reviewer-one'],false)},listBots);
    const listedOff=await invokeSuperagent<Array<{id:string;councilRequired:boolean}>>(
      'superagent_list',{},listBots);
    let tooFewOnCreate='',tooFewOnUpdate='';
    try {
      await invokeSuperagent('superagent_create',
        {input:team('Preview Council Too Few',['reviewer-one'],true)},listBots);
    } catch(error) { tooFewOnCreate=error instanceof Error?error.message:String(error); }
    try {
      await invokeSuperagent('superagent_update',{orchestrationId:required.id,
        expectedRevision:off.policyRevision,
        input:team('Preview Council Required',['reviewer-one'],true)},listBots);
    } catch(error) { tooFewOnUpdate=error instanceof Error?error.message:String(error); }
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:required.id,expectedRevision:off.policyRevision,enabled:true},listBots);
    const offTicket=await invokeSuperagent<{id:string}>('superagent_submit',{
      orchestrationId:required.id,objective:'Preview off Council admission'},listBots);
    const offSnapshot=await invokeSuperagent<{teamSnapshot:{policy:Record<string,unknown>}}>(
      'superagent_snapshot',{ticketId:offTicket.id},listBots);
    const requiredTeam=await invokeSuperagent<{id:string;policyRevision:number}>(
      'superagent_create',{input:team('Preview Council Admission',['reviewer-one','reviewer-two'],true)},listBots);
    await invokeSuperagent('superagent_set_enabled',{
      orchestrationId:requiredTeam.id,expectedRevision:requiredTeam.policyRevision,enabled:true},listBots);
    const requiredTicket=await invokeSuperagent<{id:string}>('superagent_submit',{
      orchestrationId:requiredTeam.id,objective:'Preview required Council admission'},listBots);
    const requiredSnapshot=await invokeSuperagent<{teamSnapshot:{policy:Record<string,unknown>}}>(
      'superagent_snapshot',{ticketId:requiredTicket.id},listBots);
    return {required,listedRequired,preserved,listedPreserved,off,listedOff,
      tooFewOnCreate,tooFewOnUpdate,
      offPolicy:offSnapshot.teamSnapshot.policy,
      requiredPolicy:requiredSnapshot.teamSnapshot.policy};
  });
  expect(result.required.councilRequired).toBe(true);
  expect(result.listedRequired.find(config=>config.id===result.required.id)?.councilRequired).toBe(true);
  expect(result.preserved.councilRequired).toBe(true);
  expect(result.listedPreserved.find(config=>config.id===result.required.id)?.councilRequired).toBe(true);
  expect(result.off.councilRequired).toBe(false);
  expect(result.listedOff.find(config=>config.id===result.required.id)?.councilRequired).toBe(false);
  expect(result.tooFewOnCreate).toContain('between two and eight');
  expect(result.tooFewOnUpdate).toContain('between two and eight');
  expect(result.offPolicy).toMatchObject({councilRequired:false,councilEnabled:false});
  expect(result.requiredPolicy).toMatchObject({councilRequired:true,councilEnabled:true});
});

test('Preview manifest round-trip preserves required Council and reviewer roles', async ({page}) => {
  await page.goto('/?bots=sample');
  const bots=[
    {id:'coordinator',name:'Coordinator',paused:false,projectId:'preview-project'},
    {id:'worker',name:'Worker',paused:false,projectId:'preview-project'},
    {id:'reviewer-one',name:'Reviewer One',paused:false,projectId:'preview-project'},
    {id:'reviewer-two',name:'Reviewer Two',paused:false,projectId:'preview-project'},
  ];
  const v1=await page.evaluate(async bots=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>bots;
    await invokeSuperagent('superagent_create',{input:{name:'Preview Council Off',
      coordinatorBotId:'coordinator',workerBotId:'worker',workerBotIds:['worker'],
      reviewerBotIds:['reviewer-one'],councilRequired:false}},listBots);
    return await invokeSuperagent<Record<string,any>>('superagent_manifest_export',{},listBots);
  },bots);
  expect(v1.schemaVersion).toBe(1);
  expect(v1.orchestrations).toEqual([{name:'Preview Council Off',coordinatorBot:'Coordinator',
    workerBot:'Worker',enabled:false,autonomy:'off',maxConcurrentTasks:1,
    maxAttemptsPerAssignment:2,council:{enabled:false}}]);

  const exported=await page.evaluate(async bots=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>bots;
    await invokeSuperagent('superagent_create',{input:{name:'Preview Council Required',
      coordinatorBotId:'coordinator',workerBotId:'worker',workerBotIds:['worker'],
      reviewerBotIds:['reviewer-one','reviewer-two'],councilRequired:true}},listBots);
    return await invokeSuperagent<Record<string,any>>('superagent_manifest_export',{},listBots);
  },bots);
  expect(exported.schemaVersion).toBe(2);
  const presets=exported.orchestrations as Array<Record<string,any>>;
  expect(presets.find(preset=>preset.name==='Preview Council Required')).toEqual({
    name:'Preview Council Required',coordinatorBot:'Coordinator',workers:['Worker'],
    reviewers:['Reviewer One','Reviewer Two'],
    policy:{autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
      councilEnabled:true,councilRequired:true},
  });
  expect(presets.find(preset=>preset.name==='Preview Council Off')).toEqual({
    name:'Preview Council Off',coordinatorBot:'Coordinator',workers:['Worker'],
    reviewers:['Reviewer One'],
    policy:{autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
      councilEnabled:false,councilRequired:false},
  });

  // Reloading discards the in-memory preview store, so the import is a fresh store.
  await page.reload();
  const restored=await page.evaluate(async ({bots,exported})=>{
    const {invokeSuperagent}=await import('/src/preview-superagent.ts');
    const listBots=async()=>bots;
    const preview=await invokeSuperagent<Record<string,any>>('superagent_manifest_preview',
      {document:exported},listBots);
    const imported=await invokeSuperagent<Record<string,any>>('superagent_manifest_import',
      {document:exported,confirmed:true},listBots);
    const configs=await invokeSuperagent<Array<Record<string,any>>>('superagent_list',{},listBots);
    const requiredConfig=configs.find(config=>config.name==='Preview Council Required')!;
    await invokeSuperagent('superagent_set_enabled',{orchestrationId:requiredConfig.id,
      expectedRevision:requiredConfig.policyRevision,enabled:true},listBots);
    const ticket=await invokeSuperagent<{id:string}>('superagent_submit',{
      orchestrationId:requiredConfig.id,objective:'Preview restored Council'},listBots);
    const snapshot=await invokeSuperagent<Record<string,any>>('superagent_snapshot',
      {ticketId:ticket.id},listBots);
    const invalid=structuredClone(exported);
    const required=(invalid.orchestrations as Array<Record<string,any>>)
      .find(preset=>preset.name==='Preview Council Required')!;
    required.reviewers=['Reviewer One'];
    let error='';
    try {
      await invokeSuperagent('superagent_manifest_preview',{document:invalid},listBots);
    } catch(cause) { error=cause instanceof Error?cause.message:String(cause); }
    return {preview,imported,configs,error,
      requiredPolicy:snapshot.teamSnapshot.policy,
      requiredReviewers:snapshot.teamSnapshot.reviewerBotIds};
  },{bots,exported});
  expect(restored.preview).toEqual({wouldCreate:['Preview Council Off','Preview Council Required'],
    unchanged:[],importedPresetsDisabled:true});
  expect(restored.imported).toEqual({created:['Preview Council Off','Preview Council Required'],
    unchanged:[]});
  const requiredConfig=restored.configs.find(config=>config.name==='Preview Council Required')!;
  expect(requiredConfig).toMatchObject({coordinatorBotId:'coordinator',workerBotId:'worker',
    workerBotIds:['worker'],reviewerBotIds:['reviewer-one','reviewer-two'],
    councilRequired:true,enabled:false});
  const offConfig=restored.configs.find(config=>config.name==='Preview Council Off')!;
  expect(offConfig).toMatchObject({coordinatorBotId:'coordinator',workerBotId:'worker',
    workerBotIds:['worker'],reviewerBotIds:['reviewer-one'],councilRequired:false,enabled:false});
  expect(restored.requiredPolicy).toMatchObject({councilEnabled:true,councilRequired:true});
  expect(restored.requiredReviewers).toEqual(['reviewer-one','reviewer-two']);
  expect(restored.error).toContain('between two and eight distinct reviewers');
});
