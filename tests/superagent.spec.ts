import { test, expect } from '@playwright/test';
test('native SuperAgent preset UI stays manual and imports disabled portable configurations', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const dialog=page.getByRole('region',{name:'Orchestrations'});
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('No orchestrations created.')).toBeVisible();
  await dialog.getByRole('textbox',{name:'Name'}).fill('Software Development');
  const coordinator=dialog.getByRole('combobox',{name:'Coordinator'});
  const worker=dialog.getByRole('combobox',{name:'Worker'});
  await coordinator.selectOption({label:'Scout'});
  await worker.selectOption({label:'Writer'});
  // The separate attempt budget must not ambiguously match the Worker selector.
  await expect(dialog.getByRole('combobox',{name:'Maximum attempts per Demand',exact:true}))
    .toHaveValue('2');
  // The directory is independent of the unsaved Team: redraws retain role selections.
  await dialog.locator('details.sa-new-members summary').click();
  await dialog.getByRole('combobox',{name:'Team status'}).selectOption('disabled');
  await expect(dialog.locator('details.sa-new-members')).toHaveAttribute('open','');
  await dialog.getByRole('searchbox',{name:'Search Teams'}).fill('not-yet-created');
  await expect(coordinator.locator('option:checked')).toHaveText('Scout');
  await expect(worker.locator('option:checked')).toHaveText('Writer');
  await dialog.getByRole('searchbox',{name:'Search Teams'}).fill('');
  await dialog.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(dialog.locator('.sa-list .sa-item strong')).toHaveText('Software Development');
  await expect(dialog.locator('.sa-list .sa-counts')).toHaveText('0 tasks · 0 running · 0 blocked');
  await expect(dialog.locator('.sa-list .sa-last-activity')).toContainText('Last activity: ');
  await expect(dialog.locator('.sa-list .sa-last-activity')).not.toContainText('Unavailable');
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await dialog.getByRole('button',{name:'Portable configuration'}).click();
  await dialog.getByRole('button',{name:'Export'}).click();
  const text=dialog.getByRole('textbox',{name:'Companion manifest JSON'});
  await expect(text).toContainText('"schemaVersion": 1');
  await dialog.getByRole('button',{name:'Preview import'}).click();
  await expect(dialog.getByText(/0 new; 1 unchanged/)).toBeVisible();
  await dialog.getByRole('button',{name:'Confirm import'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(dialog.getByRole('status').filter({hasText:'0 presets imported; 1 unchanged.'})).toBeVisible();
  await dialog.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
});



test('explicit secondary Worker selection uses native preflight and read-only task admission', async ({page}) => {
  // CI hydration, native roster loading and confirm/dispatch may exceed the short default.
  // Keep every original assertion and action; allow the same budget as adjacent integration tests.
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-direct-worker-test';
    document.body.appendChild(host);
    const calls:Array<{command:string;args:unknown}>=[];
    (window as unknown as {saDirectCalls:typeof calls}).saDirectCalls=calls;
    const team={id:'team',name:'Team',coordinatorBotId:'coordinator',
      workerBotId:'worker-a',enabled:true,policyRevision:2,workerCount:2,reviewerCount:0};
    const other={...team,id:'another-team',name:'Other team'};
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,args});
      if(command==='bots_list') return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker-a',name:'Worker A',paused:false},
        {id:'worker-b',name:'Worker B',paused:false},
      ] as never;
      if(command==='superagent_list') return [team,other] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_roster') return {
        orchestrationId:(args as {orchestrationId:string}).orchestrationId,
        policyRevision:2,coordinatorBotId:'coordinator',
        workerBotIds:['worker-a','worker-b'],reviewerBotIds:[],
        maxWorkerAttempts:2,purpose:'',strategy:'balanced',executionReady:false,
      } as never;
      if(command==='superagent_preflight_worker') return {
        orchestrationId:'team',policyRevision:2,projectId:'project',
        selectedBotId:(args as {workerBotId:string}).workerBotId,selectedRole:'worker',
        eligible:true,reason:null,canStageBlocked:false,
        allowedTools:[],allowedSideEffects:false,attemptBudgetMs:60000,
      } as never;
      if(command==='superagent_submit_to_worker') return {
        id:'task',orchestrationId:'team',objective:'Inspect safely',state:'running',
        revision:1,evidence:null,
      } as never;
      throw Error('Unexpected command '+command);
    });
  });
  const host=page.locator('#sa-direct-worker-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await host.getByRole('button',{name:'Load configured Workers'}).click();
  const workers=host.getByRole('combobox',{name:'Worker for this Demand'});
  await workers.selectOption('worker-b');
  await host.getByRole('textbox',{name:'Task objective'}).fill('Inspect safely');
  await host.getByRole('combobox',{name:'Dispatch'}).selectOption('direct');
  await host.getByRole('button',{name:'Review native preflight'}).click();
  await expect(host.getByRole('region',{name:'Native demand preflight'}))
    .toContainText('Worker B');
  await host.getByRole('button',{name:'Dispatch manually'}).click();
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  const calls=await page.evaluate(()=>
    (window as unknown as {saDirectCalls:Array<{command:string;args:unknown}>}).saDirectCalls);
  const submitted=calls.filter(call=>call.command==='superagent_submit_to_worker');
  expect(submitted).toHaveLength(1);
  expect((submitted[0].args as {workerBotId:string}).workerBotId).toBe('worker-b');
  expect(calls.some(call=>call.command==='superagent_submit'
    || call.command==='superagent_submit_coordinated')).toBe(false);
  // Choosing a different Team must never carry an old Worker selection forward,
  // even when that same bot is a valid member of both native rosters.
  await host.locator('[data-sa-select="another-team"]').click();
  await host.getByRole('button',{name:'Load configured Workers'}).click();
  await expect(host.getByRole('combobox',{name:'Worker for this Demand'})).toHaveValue('');
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
});

test('new multi-role Team draft survives workspace redraw and submits exact native roster', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-new-team-draft';
    document.body.appendChild(host);
    const configs: Array<Record<string, unknown>> = [];
    (window as unknown as {saCreateTeamPayload: unknown}).saCreateTeamPayload = null;
    mountSuperagent(host, async (command, args) => {
      if (command === 'bots_list') return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Primary Worker',paused:false},
        {id:'worker-b',name:'Additional Worker',paused:false,avatar:'panda:2'},
        {id:'reviewer',name:'Reviewer',paused:false},
      ] as never;
      if (command === 'superagent_list') return [...configs] as never;
      if (command === 'superagent_tickets') return [] as never;
      if (command === 'superagent_roster') return {
        orchestrationId:'created-team',policyRevision:1,coordinatorBotId:'coordinator',
        workerBotIds:['worker','worker-b'],reviewerBotIds:['reviewer'],
        maxWorkerAttempts:3,purpose:'Prioritize thorough verification within explicit time and cost budgets.',
        strategy:'performance',executionReady:false,
      } as never;
      if (command === 'superagent_create') {
        (window as unknown as {saCreateTeamPayload: unknown}).saCreateTeamPayload = args;
        const input = args?.input as Record<string, string>;
        const cfg = {id:'created-team',...input,enabled:false,policyRevision:1,
          workerCount:2,reviewerCount:1};
        configs.push(cfg);
        return cfg as never;
      }
      throw Error('Unexpected command: ' + command);
    });
  });
  const host = page.locator('#sa-new-team-draft');
  await expect(host.getByRole('button',{name:'Create disabled preset'})).toBeEnabled();
  await host.getByRole('combobox',{name:'Example preset'}).selectOption('Economy');
  await expect(host.getByRole('combobox',{name:'Maximum attempts per Demand'})).toHaveValue('1');
  await host.getByRole('combobox',{name:'Example preset'}).selectOption('Performance');
  await expect(host.getByRole('combobox',{name:'Maximum attempts per Demand'})).toHaveValue('3');
  await expect(host.getByRole('combobox',{name:'Optimization strategy'})).toHaveValue('performance');
  await expect(host.getByRole('textbox',{name:'Team purpose'}))
    .toHaveValue('Prioritize thorough verification within explicit time and cost budgets.');
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('Persisted roles');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await host.getByRole('combobox',{name:'Worker',exact:true}).selectOption('worker');
  await host.locator('details.sa-new-members summary').click();
  await host.getByRole('group',{name:'Additional Workers'}).getByRole('checkbox',{name:/Additional Worker/}).check();
  await host.getByRole('group',{name:'Optional Reviewers'}).getByRole('checkbox',{name:/Reviewer/}).check();
  // The same existing bot appears in both eligible role pickers; each
  // renders its native Panda avatar without duplicating the selected role.
  await expect(host.getByRole('group',{name:'Additional Workers'})
    .locator('.sa-member-choice .bot-avatar.has-art svg')).toHaveCount(1);
  await expect(host.getByRole('group',{name:'Optional Reviewers'})
    .locator('.sa-member-choice .bot-avatar.has-art svg')).toHaveCount(1);
  await host.getByRole('combobox',{name:'Team status'}).selectOption('enabled');
  await host.getByRole('searchbox',{name:'Search Teams'}).fill('another Team');
  await expect(host.locator('details.sa-new-members')).toHaveAttribute('open','');
  await expect(host.locator('input[name="additionalWorkers"][value="worker-b"]')).toBeChecked();
  await expect(host.locator('input[name="reviewers"][value="reviewer"]')).toBeChecked();
  await expect(host.locator('select[name="coordinatorBotId"]')).toHaveValue('coordinator');
  await expect(host.locator('select[name="workerBotId"]')).toHaveValue('worker');
  await expect(host.locator('select[name="maxWorkerAttempts"]')).toHaveValue('3');
  await expect(host.getByRole('combobox',{name:'Optimization strategy'})).toHaveValue('performance');
  await expect(host.getByRole('textbox',{name:'Team purpose'}))
    .toHaveValue('Prioritize thorough verification within explicit time and cost budgets.');
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.getByRole('button',{name:'Demands / Runs'})).toHaveAttribute('aria-current','page');
  const payload = await page.evaluate(() =>
    (window as unknown as {saCreateTeamPayload: Record<string, unknown>}).saCreateTeamPayload);
  expect(payload).toEqual({
    input:{name:'Persisted roles',coordinatorBotId:'coordinator',workerBotId:'worker'},
    roster:{workerBotIds:['worker','worker-b'],reviewerBotIds:['reviewer']},
    maxWorkerAttempts:3,
    purpose:'Prioritize thorough verification within explicit time and cost budgets.',
    strategy:'performance',
  });
});


test('new multi-role Team rejects a stale native roster after creation without repeating the write', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-stale-created-roster';
    document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {saStaleRosterCalls:string[]}).saStaleRosterCalls=calls;
    let created:Record<string,unknown>|null=null;
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
        {id:'reviewer',name:'Reviewer',paused:false},
      ] as never;
      if(command==='superagent_list')return (created ? [created] : []) as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'){
        const input=args?.input as {name:string;coordinatorBotId:string;workerBotId:string};
        created={...input,id:'stale-roster-team',enabled:false,policyRevision:1,
          workerCount:1,reviewerCount:0};
        return created as never;
      }
      if(command==='superagent_roster')return {
        orchestrationId:'stale-roster-team',policyRevision:1,
        coordinatorBotId:'coordinator',workerBotIds:['worker'],
        reviewerBotIds:[],maxWorkerAttempts:2,purpose:'',strategy:'balanced',
        executionReady:true,
      } as never;
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-stale-created-roster');
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('Reviewed Team');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await host.getByRole('combobox',{name:'Worker',exact:true}).selectOption('worker');
  await host.locator('details.sa-new-members summary').click();
  await host.getByRole('group',{name:'Optional Reviewers'})
    .getByRole('checkbox',{name:'Reviewer'}).check();
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.getByRole('alert')).toContainText('configured roles could not be verified');
  const calls=await page.evaluate(()=>
    (window as unknown as {saStaleRosterCalls:string[]}).saStaleRosterCalls);
  expect(calls.filter(command=>command==='superagent_create')).toHaveLength(1);
  expect(calls.filter(command=>command==='superagent_roster')).toHaveLength(1);
  expect(calls).not.toContain('superagent_submit');
});


test('Team directory Assign demand opens native scoped Runs without inherited drafts or dispatch', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-assign-demand-route';document.body.appendChild(host);
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    (window as unknown as {saAssignRouteCalls:typeof calls}).saAssignRouteCalls=calls;
    const teams=[
      {id:'team-a',name:'Design Team',coordinatorBotId:'coordinator',
        workerBotId:'worker',enabled:false,policyRevision:1,workerCount:1,reviewerCount:0},
      {id:'team-b',name:'Build Team',coordinatorBotId:'coordinator',
        workerBotId:'worker',enabled:false,policyRevision:1,workerCount:1,reviewerCount:0},
    ];
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,args});
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return teams as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        if((args?.query as {orchestrationId?:string} | undefined)?.orchestrationId!=='team-b')
          throw Error('Native Demand query was not Team scoped');
        return [{id:'demand-b',orchestrationId:'team-b',objective:'Persisted build activity',
          state:'blocked',revision:2,updatedAt:1000,projectId:'project-b'}] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-assign-demand-route');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('textbox',{name:'Task objective'}).fill('Draft for Design Team only');
  await host.getByRole('button',{name:'Teams',exact:true}).click();
  await host.getByRole('button',{name:'Assign demand to Build Team'}).click();
  await expect(host.getByRole('button',{name:'Demands / Runs'})).toHaveAttribute('aria-current','page');
  await expect(host.getByRole('region',{name:'Selected orchestration'})).toContainText('Build Team');
  await expect(host.getByRole('combobox',{name:'Filter Demands by Team'})).toHaveValue('team-b');
  await expect(host.getByRole('textbox',{name:'Task objective'})).toHaveValue('');
  await expect(host.getByRole('textbox',{name:'Task objective'})).toBeFocused();
  await expect(host.getByRole('region',{name:'All Demands across Teams'}))
    .toContainText('Persisted build activity');
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  const calls=await page.evaluate(() =>
    (window as unknown as {saAssignRouteCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saAssignRouteCalls);
  expect(calls.filter(item=>item.command==='superagent_all_demands')).toEqual([
    {command:'superagent_all_demands',args:{query:{limit:200,orchestrationId:'team-b'}}},
  ]);
  expect(calls.some(item=>/superagent_(submit|preflight|set_enabled|dispatch)/.test(item.command)))
    .toBe(false);
});


test('Team detail opens scoped native Demand history without dispatch or inherited filters', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-team-open-runs';document.body.appendChild(host);
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    (window as unknown as {saTeamOpenRunsCalls:typeof calls}).saTeamOpenRunsCalls=calls;
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,args});
      if(command==='bots_list')return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-open',name:'Reusable Team',coordinatorBotId:'router',
          workerBotId:'worker',enabled:false,policyRevision:1,projectId:'project'},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        if((args?.query as {orchestrationId?:string} | undefined)?.orchestrationId!=='team-open')
          throw Error('Native Demand query crossed the selected Team');
        return [{id:'existing-demand',orchestrationId:'team-open',
          objective:'Existing native Demand',state:'blocked',revision:2,
          updatedAt:1000,projectId:'project'}] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-team-open-runs');
  await host.getByRole('button',{name:'Assign demand / Open runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await expect(host.getByRole('button',{name:'Demands / Runs'})).toHaveAttribute('aria-current','page');
  await expect(global.getByRole('combobox',{name:'Filter Demands by Team'})).toHaveValue('team-open');
  await expect(global).toContainText('Existing native Demand');
  await expect(global).toContainText('1 native Demands loaded.');
  await expect(host.getByRole('textbox',{name:'Task objective'})).toBeFocused();
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  const calls=await page.evaluate(()=>
    (window as unknown as {saTeamOpenRunsCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saTeamOpenRunsCalls);
  expect(calls.filter(item=>item.command==='superagent_all_demands')).toEqual([
    {command:'superagent_all_demands',args:{query:{limit:200,orchestrationId:'team-open'}}},
  ]);
  expect(calls.some(item=>/superagent_(submit|preflight|stop|accept|grant|dispatch)/.test(item.command)))
    .toBe(false);
});

test('Team attention shortcut queries only native needs-attention states and rejects a stale mixed page',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-team-attention-scope';document.body.appendChild(host);
    const queries:Record<string,unknown>[]=[];
    let returnForeignState=false;
    (window as unknown as {saAttentionQueries:typeof queries;saAttentionInvalid:()=>void})
      .saAttentionQueries=queries;
    (window as unknown as {saAttentionInvalid:()=>void}).saAttentionInvalid=()=>{
      returnForeignState=true;
    };
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Other Team',coordinatorBotId:'coordinator',
          workerBotId:'worker',enabled:false,policyRevision:1,needsAttentionCount:0},
        {id:'team-b',name:'Build Team',coordinatorBotId:'coordinator',
          workerBotId:'worker',enabled:false,policyRevision:1,needsAttentionCount:2},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query=args?.query as Record<string,unknown> | undefined;
        queries.push({...query});
        if(query?.orchestrationId!=='team-b'||query?.stateFilter!=='needs_attention')
          throw Error('Attention shortcut omitted the native Team/state filter.');
        if(returnForeignState)return [{
          id:'completed-b',orchestrationId:'team-b',objective:'Already complete',
          state:'completed',revision:1,updatedAt:1003,
        }] as never;
        return [
          {id:'blocked-b',orchestrationId:'team-b',objective:'Blocked deployment',
            state:'blocked',revision:1,updatedAt:1002},
          {id:'failed-b',orchestrationId:'team-b',objective:'Failed regression',
            state:'failed',revision:1,updatedAt:1001},
        ] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-team-attention-scope');
  await host.getByRole('button',{name:'Show Demands needing attention for Build Team'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await expect(global.getByRole('combobox',{name:'Filter Demands by Team'})).toHaveValue('team-b');
  await expect(global).toContainText('Blocked deployment');
  await expect(global).toContainText('Failed regression');
  await expect(global).toContainText('2 native Demands loaded.');
  await page.evaluate(()=>(window as unknown as {saAttentionInvalid:()=>void}).saAttentionInvalid());
  await global.getByRole('button',{name:'Refresh selected Team'}).click();
  await expect(host.getByRole('alert')).toContainText('out-of-scope records');
  await expect(global.getByRole('button',{name:'Open demand'})).toHaveCount(0);
  const queries=await page.evaluate(()=>
    (window as unknown as {saAttentionQueries:Record<string,unknown>[]}).saAttentionQueries);
  expect(queries).toEqual([
    {limit:200,orchestrationId:'team-b',stateFilter:'needs_attention'},
    {limit:200,orchestrationId:'team-b',stateFilter:'needs_attention'},
  ]);
});

test('Team review shortcut opens an isolated native decision inbox without approving anything',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-team-review-route';document.body.appendChild(host);
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    let invalidPage=false;
    (window as unknown as {saReviewRouteCalls:typeof calls;saReviewRouteInvalid:()=>void})
      .saReviewRouteCalls=calls;
    (window as unknown as {saReviewRouteInvalid:()=>void}).saReviewRouteInvalid=()=>{
      invalidPage=true;
    };
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,args});
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Unrelated Team',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:false,policyRevision:1,waitingApprovalCount:1,waitingReviewCount:0},
        {id:'team-b',name:'Review Team',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:false,policyRevision:1,waitingApprovalCount:1,waitingReviewCount:1},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_team_review_inbox'){
        if(args?.orchestrationId!=='team-b'||args?.limit!==200)
          throw Error('Review shortcut was not scoped to the selected Team.');
        return [{
          id:invalidPage?'foreign-review':'review-b',
          orchestrationId:invalidPage?'team-a':'team-b',
          objective:'Verify release evidence',state:'waiting_approval',
          revision:3,updatedAt:2000,
        }] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-team-review-route');
  await host.getByRole('button',{name:'Review pending decisions for Review Team'}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await expect(host.getByRole('button',{name:'Review inbox',exact:true}))
    .toHaveAttribute('aria-current','page');
  await expect(inbox.getByRole('combobox',{name:'Filter review inbox by Team'}))
    .toHaveValue('team-b');
  await expect(inbox).toContainText('Verify release evidence');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(1);
  await page.evaluate(()=>(window as unknown as {saReviewRouteInvalid:()=>void}).saReviewRouteInvalid());
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(host.getByRole('alert')).toContainText('out-of-scope decisions');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(0);
  const calls=await page.evaluate(()=>
    (window as unknown as {saReviewRouteCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saReviewRouteCalls);
  expect(calls.filter(entry=>entry.command==='superagent_team_review_inbox')
    .map(entry=>entry.args)).toEqual([
      {limit:200,orchestrationId:'team-b'},
      {limit:200,orchestrationId:'team-b'},
    ]);
  expect(calls.some(entry=>/superagent_(accept|reject|submit|grant|dispatch|stop|retry)/.test(entry.command)))
    .toBe(false);
});

test('Team strategy changes through native CAS and persists after refresh',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-team-strategy-test';document.body.appendChild(host);
    const team={id:'team',name:'Reusable',coordinatorBotId:'coordinator',workerBotId:'worker',
      enabled:false,policyRevision:2};
    let roster={orchestrationId:'team',policyRevision:2,coordinatorBotId:'coordinator',
      workerBotIds:['worker'],reviewerBotIds:[],maxWorkerAttempts:2,
      purpose:'',strategy:'balanced',executionReady:true};
    const calls:unknown[]=[];
    (window as unknown as {saStrategyCalls:unknown[]}).saStrategyCalls=calls;
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [{id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [team] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_roster')return roster as never;
      if(command==='superagent_team_strategy_update'){
        calls.push(args);
        if(args?.orchestrationId!=='team' || args?.expectedRevision!==2)
          throw Error('Unscoped native strategy mutation');
        roster={...roster,policyRevision:3,purpose:String(args.purpose),strategy:String(args.strategy)};
        team.policyRevision=3;
        return roster as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-team-strategy-test');
  await host.getByRole('button',{name:'Teams',exact:true}).click();
  await host.getByRole('button',{name:'Load native members'}).click();
  await host.locator('form[data-sa-form="team-strategy"] textarea[name="purpose"]')
    .fill('Automate reliable tests');
  await host.locator('form[data-sa-form="team-strategy"] select[name="strategy"]')
    .selectOption('performance');
  await host.getByRole('button',{name:'Save Team purpose and strategy'}).click();
  await expect(host.locator('form[data-sa-form="team-strategy"] textarea[name="purpose"]'))
    .toHaveValue('Automate reliable tests');
  await expect(host.locator('form[data-sa-form="team-strategy"] select[name="strategy"]'))
    .toHaveValue('performance');
  const calls=await page.evaluate(()=>(window as unknown as {saStrategyCalls:unknown[]}).saStrategyCalls);
  expect(calls).toEqual([{orchestrationId:'team',expectedRevision:2,
    purpose:'Automate reliable tests',strategy:'performance'}]);
});

test('native policy blocker prevents edits hidden by the paginated Ticket view',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-policy-blocked';document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {nativePolicyCalls:string[]}).nativePolicyCalls=calls;
    const team={id:'team',name:'Reusable',coordinatorBotId:'coordinator',workerBotId:'worker',
      enabled:false,policyRevision:4,policyEditBlocker:'Active or reviewable Demands prevent Team policy edits.'};
    const mounted=mountSuperagent(host,async command=>{
      calls.push(command);
      if(command==='bots_list')return [{id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [team] as never;
      if(command==='superagent_tickets')return [{id:'recent',orchestrationId:'team',objective:'Closed',
        state:'completed',revision:1}] as never;
      if(command==='superagent_roster')return {orchestrationId:'team',policyRevision:4,
        coordinatorBotId:'coordinator',workerBotIds:['worker'],reviewerBotIds:[],
        maxWorkerAttempts:2,executionReady:true} as never;
      throw Error('Unexpected native command '+command);
    });
    (window as unknown as {liftBlocker:()=>Promise<void>}).liftBlocker=async()=>{
      team.policyEditBlocker='';await mounted.refresh();
    };
  });
  const host=page.locator('#sa-policy-blocked');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByRole('button',{name:'Save preset'})).toBeDisabled();
  await expect(host.getByText('Team policy edit blocked:',{exact:false})).toContainText('Active or reviewable');
  await host.locator('form[data-sa-form="edit"]').dispatchEvent('submit');
  await host.getByRole('button',{name:'Teams',exact:true}).click();
  await host.getByRole('button',{name:'Load native members'}).click();
  await expect(host.getByRole('button',{name:'Save configured roles'})).toBeDisabled();
  await expect(host.getByRole('button',{name:'Save Worker attempt budget'})).toBeDisabled();
  await host.locator('form[data-sa-form="attempt-budget"]').dispatchEvent('submit');
  const calls=await page.evaluate(()=>(window as unknown as {nativePolicyCalls:string[]}).nativePolicyCalls);
  expect(calls).not.toContain('superagent_update');
  expect(calls).not.toContain('superagent_roster_update');
  expect(calls).not.toContain('superagent_worker_attempt_limit_update');
  await page.evaluate(async()=>await (window as unknown as {liftBlocker:()=>Promise<void>}).liftBlocker());
  await expect(host.getByRole('button',{name:'Save configured roles'})).toBeEnabled();
  await expect(host.getByRole('button',{name:'Save Worker attempt budget'})).toBeEnabled();
});

test('native SuperAgent manual dispatch requires opt-in and supports explicit revocation',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const dialog=page.getByRole('region',{name:'Orchestrations'});
  await dialog.getByRole('textbox',{name:'Name'}).fill('Issue Resolution');
  await dialog.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await dialog.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await dialog.getByRole('button',{name:'Create disabled preset'}).click();
  // Wait for the native create ACK and the Runs view, not the click itself.
  // The CI host can complete a native reload after the usual 5s assertion window.
  await expect(dialog.locator('.sa-list .sa-item strong')).toHaveText('Issue Resolution', {timeout:15_000});
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await dialog.getByRole('button',{name:'Enable manual'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Disable',exact:true})).toBeVisible();
  await expect(dialog.locator('#superagent-host')).toHaveAttribute('aria-busy','false');
  await dialog.getByRole('textbox',{name:'Task objective'}).fill('Inspect a disposable fixture');
  await dialog.getByRole('combobox',{name:'Dispatch'}).selectOption('direct');
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await dialog.getByRole('button',{name:'Review native preflight'}).click();
  await expect(dialog.getByRole('region',{name:'Native demand preflight'}))
    .toContainText('Eligible for manual read-only admission now.');
  await dialog.getByRole('button',{name:'Dispatch manually'}).click();
  await expect(dialog.getByRole('alertdialog',{name:'Confirm orchestration action'}))
    .toContainText('No write grant is issued.');
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  const row=dialog.locator('.sa-tickets li',{hasText:'Inspect a disposable fixture'});
  await expect(row).toContainText('running');
  await expect(dialog.locator('.sa-list .sa-counts')).toHaveText('1 tasks · 1 running · 0 blocked');
  await row.getByRole('button',{name:'View evidence'}).click();
  const details=dialog.getByRole('region',{name:'Ticket evidence'});
  await expect(details).toContainText('Ticket ID');
  await expect(details).toContainText('local-user');
  await expect(details).toContainText('No side effects allowed');
  await expect(details).toContainText('Attempt 1');
  await expect(details).toContainText('admitted');
  await expect(details.locator('ol li').first()).toContainText(' · admitted · ');
  await row.getByRole('button',{name:'Stop'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(row).toContainText('stopped');
  await expect(dialog.locator('.sa-list .sa-counts')).toHaveText('1 tasks · 0 running · 0 blocked');
  await expect(details).toContainText('Revoked');
  await expect(details).toContainText('stopped');
  await expect(row.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
});


test('native SuperAgent preset edit and deletion require unused disabled configuration',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const dialog=page.getByRole('region',{name:'Orchestrations'});
  await dialog.getByRole('textbox',{name:'Name'}).fill('Safe default');
  await dialog.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await dialog.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await dialog.getByRole('button',{name:'Create disabled preset'}).click();
  // Wait for the committed Team and navigation before interacting with the edit form.
  await expect(dialog.locator('.sa-list .sa-item strong'))
    .toHaveText('Safe default',{timeout:15_000});
  await expect(dialog.getByRole('button',{name:'Demands / Runs'}))
    .toHaveAttribute('aria-current','page');
  await dialog.getByRole('textbox',{name:'Preset title'}).fill('Safely renamed');
  await dialog.getByRole('button',{name:'Save preset'}).click();
  await expect(dialog.locator('.sa-list .sa-item strong')).toHaveText('Safely renamed');
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await dialog.getByRole('button',{name:'Delete unused'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(dialog.getByText('No orchestrations created.')).toBeVisible();
});

test('settled Team role edit uses one native revision-bound mutation without dispatch', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-team-role-reuse';
    document.body.appendChild(host);
    const calls:Array<{command:string;args?:Record<string,unknown>}>=[];
    (window as unknown as {saRoleReuseCalls:typeof calls}).saRoleReuseCalls=calls;
    const cfg={id:'reusable',name:'Existing Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',policyRevision:4,enabled:false,ticketCount:1};
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,args});
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{...cfg}] as never;
      if(command==='superagent_tickets')return [
        {id:'past',orchestrationId:'reusable',objective:'Finished Demand',state:'stopped',revision:3}
      ] as never;
      if(command==='superagent_update') {
        if(args?.orchestrationId!=='reusable'||args?.expectedRevision!==4)
          throw Error('Native policy revision was not preserved');
        const input=args?.input as {name:string;coordinatorBotId:string;workerBotId:string};
        if(input.name!=='Rotated Team'||input.coordinatorBotId!=='worker'||
            input.workerBotId!=='coordinator')throw Error('Invalid role mutation payload');
        Object.assign(cfg,input,{policyRevision:5});
        return {...cfg} as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-team-role-reuse');
  // The editable Team details are deliberately in the first-class Runs view,
  // not the Teams creation screen. Navigate as a user would.
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByText('Disabled Teams may change primary roles', {exact:false})).toBeVisible();
  await host.getByRole('textbox',{name:'Preset title'}).fill('Rotated Team');
  await host.getByRole('combobox',{name:'Edit coordinator'}).selectOption('worker');
  await host.getByRole('combobox',{name:'Edit worker'}).selectOption('coordinator');
  await host.getByRole('button',{name:'Save preset'}).click();
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText('Rotated Team');
  await expect(host.getByRole('region',{name:'Selected orchestration'}))
    .toContainText('Policy revision 5');
  const calls=await page.evaluate(() =>
    (window as unknown as {saRoleReuseCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saRoleReuseCalls);
  expect(calls.filter(item=>item.command==='superagent_update')).toHaveLength(1);
  expect(calls.filter(item=>item.command==='superagent_submit'||
    item.command==='superagent_set_enabled'||item.command==='superagent_dispatch_worker'))
    .toHaveLength(0);
});

test('native Team hydration holds controls until the current projection is ready', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-delayed-native-hydration';
    document.body.appendChild(host);
    let release!:()=>void;
    const pending=new Promise<void>(resolve => { release=resolve; });
    (window as unknown as {releaseTeamHydration:()=>void}).releaseTeamHydration=release;
    mountSuperagent(host,async command => {
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list') {
        await pending;
        return [{id:'team',name:'Native Team',coordinatorBotId:'coordinator',
          workerBotId:'worker',enabled:false,policyRevision:3}] as never;
      }
      if(command==='superagent_tickets')return [] as never;
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-delayed-native-hydration');
  await expect(host).toHaveAttribute('aria-busy','true');
  await expect(host.getByRole('button',{name:'Demands / Runs'})).toBeDisabled();
  await expect(host.getByRole('searchbox',{name:'Search Teams'})).toBeDisabled();
  await expect(host.getByRole('button',{name:'Create disabled preset'})).toBeDisabled();
  await expect(host.getByRole('combobox',{name:'Example preset'})).toBeEnabled();
  await expect(host.getByRole('textbox',{name:'Name',exact:true})).toBeEnabled();
  // Synthetic submit must not admit/create a Team during native hydration.
  await host.locator('form[data-sa-form="create"]').dispatchEvent('submit');
  await expect(host.getByRole('alert')).toHaveCount(0);
  await page.evaluate(() =>
    (window as unknown as {releaseTeamHydration:()=>void}).releaseTeamHydration());
  await expect(host).toHaveAttribute('aria-busy','false');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByRole('textbox',{name:'Preset title'})).toHaveValue('Native Team');
  await expect(host.getByRole('combobox',{name:'Edit coordinator'})).toHaveValue('coordinator');
});

test('native SuperAgent clears tickets when switching presets after a failed read', async ({ page }) => {
  test.setTimeout(60_000);
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
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByText('First preset ticket')).toBeVisible();
  await expect(host.locator('[data-sa-accept="ticket-from-first-preset"]')).toHaveCount(1);
  await expect(host.locator('.sa-directory .sa-item')).toHaveCount(2);
  await host.locator('.sa-directory .sa-item').filter({hasText:'Second preset'})
    .getByRole('button',{name:'Details'}).click();
  await expect(host.getByRole('alert')).toContainText('Native ticket read failed');
  await expect(host.getByRole('heading', {name:'Second preset'})).toBeVisible();
  await expect(host.getByText('First preset ticket')).toHaveCount(0);
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
});

test('native finalization snapshot blocks unsafe accept controls without executing them', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-finalization-native'; document.body.appendChild(host);
    (window as unknown as {finalizationActions:string[]}).finalizationActions = [];
    mountSuperagent(host, async command => {
      if(command==='bots_list') return [{id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Test Team',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:2,
      }] as never;
      if(command==='superagent_tickets') return [{
        id:'ticket',orchestrationId:'team',objective:'Review real evidence',
        state:'waiting_approval',revision:3,evidence:'worker evidence',
      }] as never;
      if(command==='superagent_source_thread_links') return [] as never;
      if(command==='superagent_snapshot') return {
        ticket:{id:'ticket',orchestrationId:'team',objective:'Review real evidence',
          state:'waiting_approval',revision:3,evidence:'worker evidence'},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:2,
          projectId:'project',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],finalizationGate:'council_required',
        unresolvedExternalEffects:0,
      } as never;
      (window as unknown as {finalizationActions:string[]}).finalizationActions.push(command);
      throw new Error('Unapproved action: '+command);
    });
  });
  const host = page.locator('#sa-finalization-native');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  await expect(host.getByRole('region',{name:'Ticket evidence'}))
    .toContainText('Blocked: native Council task review required');
  await expect(host.getByRole('button',{name:'Accept evidence'})).toBeDisabled();
  await expect(host.getByRole('button',{name:'Accept evidence'}))
    .toHaveAttribute('aria-describedby','sa-finalization-blocked');
  const actions=await page.evaluate(() =>
    (window as unknown as {finalizationActions:string[]}).finalizationActions);
  expect(actions).not.toContain('superagent_accept');
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
        id:'ticket-one',orchestrationId:'preset',objective:'Review artifact',state:'waiting_approval',
        revision:2,evidence:'worker evidence'
      }] as never;
      if (command === 'superagent_source_thread_links') return [] as never;
      if (command === 'superagent_snapshot') return {
        ticket:{id:'ticket-one',orchestrationId:'preset',objective:'Review artifact',
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
  await host.getByRole('button',{name:'Demands / Runs'}).click();
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
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true') await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const dialog=page.getByRole('region',{name:'Orchestrations'});
  const example=dialog.getByRole('combobox',{name:'Example preset'});
  const name=dialog.getByRole('textbox',{name:'Name',exact:true});
  await example.selectOption('Software Development');
  await expect(name).toHaveValue('Software Development');
  await expect(dialog.getByText('No orchestrations created.')).toBeVisible();
  await name.fill('Edited safe template');
  await expect(name).toHaveValue('Edited safe template');
  await example.selectOption('Issue Resolution');
  await expect(name).toHaveValue('Issue Resolution');
  await dialog.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await dialog.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await dialog.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(dialog.locator('.sa-list .sa-item strong')).toHaveText('Issue Resolution');
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
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
      if(command==='superagent_source_integrity') {
        const src = sources.find(item => item.id === args?.sourceId);
        if (!src) throw new Error('Unknown native source');
        return {
          sourceId:src.id,repository:src.repository,mode:src.mode,
          policyRevision:src.policyRevision,cursorKind:'github_event_identity_high_watermark',
          lastSequence:src.lastSequence,receiptCount:1,permittedReceiptCount:1,
          maxReceiptSequence:3,cursorConsistent:false,receiptCapacityReached:false,
          nextAction:'investigate_cursor_receipt_mismatch',observationalOnly:true
        } as never;
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
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await expect(host.getByText('Authenticated GitHub Events polling is bounded',{exact:false})).toBeVisible();
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByText('No event sources configured.')).toBeVisible();
  // These actions are surfaced by the native GitHub Events adapter, not
  // inferred from issue/review text or mistaken for Council acceptance.
  const kinds = host.getByRole('combobox',{name:'Event class'});
  await expect(kinds.locator('option[value="pull_request.closed"]')).toHaveCount(1);
  await expect(kinds.locator('option[value="pull_request_review.submitted"]')).toHaveCount(1);
  await expect(kinds.locator('option[value="pull_request_review_comment.created"]')).toHaveCount(1);
  await expect(kinds.locator('option[value="pull_request_review.dismissed"]')).toHaveCount(1);
  // New source classes are native-backed and stay Off until explicitly enabled.
  for(const kind of [
    'issues.assigned','issues.unassigned','issues.labeled','issues.unlabeled',
    'pull_request.merged','pull_request.assigned','pull_request.unassigned',
    'pull_request.labeled','pull_request.unlabeled'
  ]) {
    await expect(kinds.locator('option[value="'+kind+'"]')).toHaveCount(1);
  }
  await kinds.selectOption('pull_request.merged');
  await host.getByRole('textbox',{name:'Repository (owner/name)'}).fill('test/fixture');
  await host.getByRole('textbox',{name:'Exact actor'}).fill('trusted-bot');
  await host.getByRole('button',{name:'Add source (Off)'}).click();
  await expect(host.locator('.sa-source-list')).toContainText('test/fixture');
  await expect(host.locator('.sa-source-list')).toContainText('pull_request.merged');
  await expect(host.locator('.sa-source-list')).toContainText('off');
  await host.getByRole('button',{name:'Enable Observe'}).click();
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.locator('.sa-source-list')).toContainText('observe');
  await host.getByRole('button',{name:'Inspect source health'}).click();
  const integrity = host.getByRole('region',{name:'Source integrity'});
  await expect(integrity).toContainText('GitHub Events high-watermark');
  await expect(integrity).toContainText('investigate_cursor_receipt_mismatch');
  await expect(integrity).toContainText('Cursor behind durable receipts');
  await expect(integrity).toContainText('Observation only.');
  await expect(host.locator('[data-sa-next-mode="auto"]')).toHaveCount(0);
  await host.getByRole('button',{name:'View decisions'}).click();
  await expect(host.getByRole('region',{name:'Source decisions'})).toContainText('observed_permitted');
  await expect(host.getByRole('region',{name:'Source decisions'})).toContainText('issue:42');
  await expect(host.locator('[data-sa-next-mode="auto"]')).toHaveCount(0);
  // Details and Assign demand are different actions; a Team selector must
  // resolve to exactly one control even when both are available.
  await expect(host.locator('[data-sa-select="two"]')).toHaveCount(1);
  await expect(host.getByRole('button',{name:'Assign demand to two'})).toHaveCount(1);
  await host.locator('[data-sa-select="two"]').click();
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await expect(host.locator('.sa-source-list')).toHaveCount(0);
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByText('No event sources configured.')).toBeVisible();
  await expect(host.getByRole('region',{name:'Source decisions'})).toHaveCount(0);
  await expect(host.getByRole('region',{name:'Source integrity'})).toHaveCount(0);
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
  await expect(host).toHaveAttribute('aria-busy','true');
  await expect(host.getByRole('button',{name:'Create disabled preset'})).toBeDisabled();
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
  // Multiple native-backed tab changes can exceed the default timeout on
  // loaded CI browsers. Preserve every assertion and side-effect denial.
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-thread-link-test';document.body.appendChild(host);
    const links:Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;createdAt:number}>=[];
    const configs=[{id:'one',name:'One',coordinatorBotId:'router',
      workerBotId:'worker',enabled:true,policyRevision:1}];
    const ticket={id:'ticket-one',orchestrationId:'one',objective:'Private fixture',state:'queued',revision:1,evidence:null};
    let dispatches=0, unlinks=0;
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
      if(command==='superagent_source_thread_activity')return (
        links.some(link=>link.ticketId===args?.ticketId && link.subjectKey==='issue:42')
          ? [{sourceId:'source-one',deliveryId:'verified-501',eventKind:'issues.opened',
              subjectKey:'issue:42',decision:'observed_permitted',sequence:501,
              policyRevision:1,at:Date.now()}] : []) as never;
      if(command==='superagent_source_link_thread'){
        if(args?.sourceId!=='source-one'||args.ticketId!=='ticket-one')throw Error('Wrong scope');
        if(links.some(link=>link.sourceId===args.sourceId&&link.subjectKey===args.subjectKey))
          throw Error('Duplicate reference');
        const link={sourceId:'source-one',ticketId:'ticket-one',repository:'test/fixture',
          subjectKey:String(args.subjectKey),createdAt:Date.now()};
        links.push(link);return link as never;
      }
      if(command==='superagent_source_link_issue_pull'){
        if(args?.sourceId!=='source-one'||args.ticketId!=='ticket-one'||
            args.issueKey!=='issue:42'||args.pullKey!=='pull:77'||
            !links.some(link=>link.subjectKey===args.issueKey))throw Error('Invalid pairing');
        const link={sourceId:'source-one',ticketId:'ticket-one',repository:'test/fixture',
          subjectKey:'pull:77',createdAt:Date.now()};
        links.push(link);return link as never;
      }
      if(command==='superagent_source_unlink_thread'){
        if(args?.sourceId!=='source-one' || args.ticketId!=='ticket-one' ||
            !['issue:42','pull:77'].includes(String(args.subjectKey))) throw Error('Wrong unlink scope');
        const index=links.findIndex(link=>link.sourceId===args.sourceId &&
          link.ticketId===args.ticketId && link.subjectKey===args.subjectKey);
        if(index<0) throw Error('Missing native link');
        links.splice(index,1); unlinks++; return undefined as never;
      }
      dispatches++;
      throw Error('Unexpected side-effect command: '+command);
    });
    (window as unknown as {saDispatchCount:()=>number;saUnlinkCount:()=>number}).saDispatchCount=()=>dispatches;
    (window as unknown as {saUnlinkCount:()=>number}).saUnlinkCount=()=>unlinks;
  });
  const host=page.locator('#sa-thread-link-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  await expect(host.getByText('No threads linked.')).toBeVisible();
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await host.getByRole('button',{name:'Load sources'}).click();
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('textbox',{name:'Exact thread key'}).fill('issue:42');
  await host.getByRole('button',{name:'Link thread (context only)'}).click();
  await expect(host.locator('.sa-linked-threads')).toContainText('test/fixture / issue:42');
  await expect(host.locator('.sa-feedback[role="status"]')).toContainText('no task or authorization created');
  await host.getByRole('button',{name:'Load linked source decisions'}).click();
  const activity=host.getByRole('region',{name:'Observed linked GitHub activity'});
  await expect(activity).toContainText('verified-501');
  await expect(activity).toContainText('observed_permitted');
  await host.getByRole('textbox',{name:'Exact PR thread key'}).fill('pull:77');
  await host.getByRole('button',{name:'Associate PR with linked issue (context only)'}).click();
  await expect(host.locator('.sa-linked-threads')).toContainText('pull:77');
  await expect(host.locator('.sa-feedback[role="status"]')).toContainText('GitHub relationship is not verified');
  await expect(host.getByRole('button',{name:'Unlink test/fixture / issue:42'})).toBeVisible();
  await host.getByRole('button',{name:'Unlink test/fixture / issue:42'}).click();
  await expect(host.getByText('Unlink this exact GitHub source thread?',{exact:false})).toBeVisible();
  await host.getByRole('button',{name:'Confirm'}).click();
  await expect(host.locator('.sa-linked-threads')).toContainText('pull:77');
  await host.getByRole('button',{name:'Unlink test/fixture / pull:77'}).click();
  await host.getByRole('button',{name:'Confirm'}).click();
  await expect(host.locator('.sa-linked-threads')).toContainText('No threads linked.');
  await host.getByRole('button',{name:'Load linked source decisions'}).click();
  await expect(activity).toContainText('No authenticated Observe decisions match linked threads.');
  await expect.poll(()=>page.evaluate(
    ()=>(window as unknown as {saUnlinkCount:()=>number}).saUnlinkCount())).toBe(2);
  await expect(host.getByText('Private fixture')).toBeVisible();
  await expect.poll(()=>page.evaluate(
    ()=>(window as unknown as {saDispatchCount:()=>number}).saDispatchCount())).toBe(0);
});

test('SuperAgent inspector pages verified linked history without duplicate tasks or repeated receipts', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-paged-linked-history';document.body.appendChild(host);
    const ticket={id:'ticket',orchestrationId:'team',objective:'Keep history private',
      state:'queued',revision:1,evidence:null};
    const calls:string[]=[];
    const decisions=Array.from({length:52},(_,index)=>({
      sourceId:'source',deliveryId:'receipt-'+String(52-index).padStart(3,'0'),
      eventKind:'issues.opened',subjectKey:'issue:42',decision:'observed_permitted',
      sequence:52-index,policyRevision:1,at:1700000000000,
    }));
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [{id:'a',name:'Coordinator',paused:false},
        {id:'b',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [{id:'team',name:'Team',coordinatorBotId:'a',
        workerBotId:'b',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [ticket] as never;
      if(command==='superagent_snapshot')return {ticket,
        mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,projectId:'fixture',
          expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[]} as never;
      if(command==='superagent_source_thread_links')return [{
        sourceId:'source',ticketId:'ticket',repository:'test/fixture',
        subjectKey:'issue:42',createdAt:1700000000000}] as never;
      if(command==='superagent_source_thread_activity')return decisions.slice(0,50) as never;
      if(command==='superagent_source_thread_activity_page'){
        const before=args?.before as {at:number;sourceId:string;deliveryId:string};
        if(args?.ticketId!=='ticket'||before?.deliveryId!=='receipt-003'||
            before?.sourceId!=='source'||before?.at!==1700000000000)
          throw Error('Invalid native continuation request');
        return {decisions:decisions.slice(50),nextCursor:null} as never;
      }
      throw Error('Unexpected command: '+command);
    });
    (window as unknown as {saLinkedCalls:()=>string[]}).saLinkedCalls=()=>calls;
  });
  const host=page.locator('#sa-paged-linked-history');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  await host.getByRole('button',{name:'Load linked source decisions'}).click();
  const region=host.getByRole('region',{name:'Observed linked GitHub activity'});
  await expect(region).toContainText('receipt-052');
  await expect(region).not.toContainText('receipt-002');
  await region.getByRole('button',{name:'Load older linked activity'}).click();
  await expect(region).toContainText('receipt-001');
  await expect(region.locator('ol li')).toHaveCount(52);
  await expect(region.getByRole('button',{name:'Load older linked activity'})).toHaveCount(0);
  const calls=await page.evaluate(
    ()=>(window as unknown as {saLinkedCalls:()=>string[]}).saLinkedCalls());
  expect(calls.filter(command=>command==='superagent_source_thread_activity_page')).toHaveLength(1);
  expect(calls.filter(command=>command==='superagent_source_thread_activity')).toHaveLength(1);
  expect(calls.some(command=>command.includes('dispatch')||
    command.includes('grant')||command.includes('admit'))).toBe(false);
});

test('Bots Orchestrations is a persistent main workspace with native-backed Teams and Runs', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  if (await bar.getAttribute('aria-pressed') !== 'true') await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const workspace=page.getByRole('region',{name:'Orchestrations'});
  await expect(workspace).toBeVisible();
  await expect(page.getByRole('dialog',{name:'Orchestrations'})).toHaveCount(0);
  await expect(workspace.getByRole('navigation',{name:'Orchestration views'})).toBeVisible();
  await expect(workspace.getByRole('complementary',{name:'Teams directory'})).toContainText('No orchestrations created.');
  await workspace.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(workspace.getByText('Select a Team from the directory')).toBeVisible();
  await workspace.getByRole('button',{name:'Teams',exact:true}).click();
  await workspace.getByRole('textbox',{name:'Name',exact:true}).fill('Persistent Team');
  await workspace.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await workspace.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await workspace.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-list')).toContainText('Persistent Team');
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await workspace.getByRole('button',{name:'Back to Bots'}).click();
  await expect(workspace).toBeHidden();
  const orchestrationItem=sidebar.getByRole('menuitem',{name:'Orchestrations'});
  if (!await orchestrationItem.isVisible()) await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await expect(orchestrationItem).toBeVisible();
  await orchestrationItem.click();
  await expect(workspace).toBeVisible();
  await expect(workspace.locator('.sa-list')).toContainText('Persistent Team');
  await expect(workspace.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
});

test('Ticket activity offers native All, Compact and JSON views without invented evidence', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-audit-native-fixture';document.body.appendChild(host);
    const ticket={id:'audit-ticket',orchestrationId:'audit-team',objective:'Review bounded evidence',state:'waiting_approval',revision:3,evidence:'evidence'};
    mountSuperagent(host,async command => {
      if (command==='bots_list')return [{id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}] as never;
      if (command==='superagent_list')return [{id:'audit-team',name:'Audit Team',coordinatorBotId:'router',workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if (command==='superagent_tickets')return [ticket] as never;
      if (command==='superagent_source_thread_links')return [] as never;
      if (command==='superagent_snapshot')return {
        ticket,mandate:{actor:'local-user',authorizationId:'manual',policyRevision:1,projectId:'fixture',expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[
          {id:3,at:1794444000000,kind:'turn_settled',detail:'Evidence recorded'},
          {id:2,at:1794443999000,kind:'turn_settled',detail:'Evidence recorded'},
          {id:1,at:1794443998000,kind:'admitted',detail:'Manual native grant'},
        ]
      } as never;
      throw new Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-audit-native-fixture');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  const details=host.getByRole('region',{name:'Ticket evidence'});
  const modes=details.getByRole('navigation',{name:'Activity display'});
  await expect(modes.getByRole('button',{name:'All'})).toHaveAttribute('aria-pressed','true');
  await expect(details.locator('.sa-audit-list li')).toHaveCount(3);
  await modes.getByRole('button',{name:'Compact'}).click();
  await expect(details.locator('.sa-audit-list li')).toHaveCount(2);
  await expect(details.getByText('2 consecutive native events',{exact:false})).toBeVisible();
  await modes.getByRole('button',{name:'JSON'}).click();
  await expect(details.locator('.sa-audit-json')).toContainText('"id": 3');
  await expect(details.locator('.sa-audit-json')).not.toContainText('another-ticket');
  await details.getByRole('searchbox',{name:'Filter native activity'}).fill('admitted');
  await expect(details.locator('.sa-audit-json')).toContainText('"kind": "admitted"');
  await expect(details.locator('.sa-audit-json')).not.toContainText('turn_settled');
  await modes.getByRole('button',{name:'All'}).click();
  await expect(details.locator('.sa-audit-list li')).toHaveCount(1);
  await expect(details.getByText('Manual native grant',{exact:false})).toBeVisible();
});

test('Off source never polls GitHub and Observe only records authenticated native decisions',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-poll-source';document.body.appendChild(host);
    const src={id:'source-1',orchestrationId:'preset',repository:'owner/repo',actorFilter:'trusted-bot',
      eventKind:'issues.opened',mode:'off',policyRevision:1,lastSequence:0};
    const calls:string[]=[];
    (window as unknown as {saPollCalls:string[]}).saPollCalls=calls;
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [{id:'a',name:'A',paused:false},{id:'b',name:'B',paused:false}] as never;
      if(command==='superagent_list')return [{id:'preset',name:'Observed Team',coordinatorBotId:'a',
        workerBotId:'b',enabled:false,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_source_list')return [src] as never;
      if(command==='superagent_source_set_mode'){
        src.mode=String(args?.mode);src.policyRevision+=1;return {...src} as never;
      }
      if(command==='superagent_source_poll_authenticated'){
        if(src.mode!=='observe')throw new Error('Poll attempted outside Observe');
        return {sourceId:src.id,repository:src.repository,scanned:3,recorded:1,duplicates:2,
          selfIgnored:0,pagesFetched:1,skippedBeforeActivation:0,lastSequence:5,boundedWindow:true} as never;
      }
      throw new Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-poll-source');
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByRole('button',{name:'Poll authenticated GitHub'})).toHaveCount(0);
  await host.getByRole('button',{name:'Enable Observe'}).click();
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('button',{name:'Poll authenticated GitHub'})).toBeVisible();
  await host.getByRole('button',{name:'Poll authenticated GitHub'}).click();
  await expect(host.getByRole('status').filter({hasText:'3 scanned across 1 page(s), 1 recorded, 0 self-authored ignored, 2 duplicate'})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {saPollCalls:string[]}).saPollCalls);
  expect(calls.filter(c=>c==='superagent_source_poll_authenticated')).toHaveLength(1);
  expect(calls.filter(c=>c==='superagent_submit'||c==='superagent_dispatch_worker')).toHaveLength(0);
});

test('Team Observe batches only enabled authenticated GitHub Events sources without work', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-team-observe';
    document.body.appendChild(host);
    const sources = [
      {id:'opened',orchestrationId:'team-1',repository:'owner/repo',actorFilter:'trusted-bot',
        eventKind:'issues.opened',mode:'observe',policyRevision:2,lastSequence:0},
      {id:'comment',orchestrationId:'team-1',repository:'owner/repo',actorFilter:'trusted-bot',
        eventKind:'issue_comment.created',mode:'observe',policyRevision:2,lastSequence:0},
      {id:'checks',orchestrationId:'team-1',repository:'owner/repo',actorFilter:'trusted-bot',
        eventKind:'check_run.completed',mode:'observe',policyRevision:2,lastSequence:0},
      {id:'off',orchestrationId:'team-1',repository:'owner/repo',actorFilter:'trusted-bot',
        eventKind:'issues.closed',mode:'off',policyRevision:1,lastSequence:0},
    ];
    const called:string[] = [];
    (window as unknown as {saTeamObserveCalls:string[]}).saTeamObserveCalls = called;
    mountSuperagent(host, async (command,args) => {
      if(command==='bots_list') return [
        {id:'a',name:'A',paused:false},{id:'b',name:'B',paused:false}
      ] as never;
      if(command==='superagent_list') return [{
        id:'team-1',name:'Observe Team',coordinatorBotId:'a',workerBotId:'b',
        enabled:false,policyRevision:1,
      }] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_source_list') return sources as never;
      called.push(command);
      if(command==='superagent_source_poll_team_authenticated') {
        if(args?.orchestrationId!=='team-1') throw Error('Cross-Team Observe denied');
        return {
          orchestrationId:'team-1',observationalOnly:true,skippedOff:1,skippedCheckRuns:1,
          observed:[
            {scanned:3,recorded:1,duplicates:2,selfIgnored:0,pagesFetched:1},
            {scanned:2,recorded:2,duplicates:0,selfIgnored:1,pagesFetched:1},
          ],
        } as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host = page.locator('#sa-team-observe');
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await host.getByRole('button',{name:'Load sources'}).click();
  await host.getByRole('button',{name:'Poll all enabled GitHub Events sources'}).click();
  await expect(host.getByRole('status').filter({
    hasText:'2 source(s), 5 scanned across 2 page(s), 3 recorded, 2 duplicate, 1 self-authored ignored',
  })).toBeVisible();
  await expect(host.getByRole('status').filter({
    hasText:'1 Off and 1 Check Run source(s) skipped. No tasks or effects started.',
  })).toBeVisible();
  const called = await page.evaluate(
    () => (window as unknown as {saTeamObserveCalls:string[]}).saTeamObserveCalls,
  );
  expect(called).toEqual(['superagent_source_poll_team_authenticated']);
});

test('Check Run observation stays Off until enabled and never launches a Demand', async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-check-run';document.body.appendChild(host);
    const src={id:'check-source',orchestrationId:'preset',repository:'owner/repo',
      actorFilter:'trusted-bot',eventKind:'check_run.completed',mode:'off',
      policyRevision:1,lastSequence:0};
    const calls:{command:string;runId?:number}[]=[];
    (window as unknown as {saCheckCalls:{command:string;runId?:number}[]}).saCheckCalls=calls;
    mountSuperagent(host,async(command,args)=>{
      calls.push({command,runId:Number(args?.checkRunId)});
      if(command==='bots_list')return [
        {id:'a',name:'A',paused:false},{id:'b',name:'B',paused:false}
      ] as never;
      if(command==='superagent_list')return [{
        id:'preset',name:'Checks Team',coordinatorBotId:'a',workerBotId:'b',
        enabled:false,policyRevision:1
      }] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_source_list')return [src] as never;
      if(command==='superagent_source_set_mode'){
        src.mode=String(args?.mode);src.policyRevision+=1;return {...src} as never;
      }
      if(command==='superagent_source_observe_check_run'){
        if(src.mode!=='observe'||args?.checkRunId!==240)
          throw new Error('Check Run observation attempted without exact native source/input');
        return {sourceId:src.id,deliveryId:'checkrun_240',decision:'observed_permitted',
          policyRevision:src.policyRevision,sequence:1,at:Date.now(),duplicate:false} as never;
      }
      if(command==='superagent_source_decisions')return [{
        sourceId:src.id,deliveryId:'checkrun_240',actor:'trusted-bot',
        eventKind:'check_run.completed',subjectKey:'pull:42',
        decision:'observed_permitted',sequence:1,policyRevision:src.policyRevision,
        at:Date.now(),checkConclusion:'failure',
        checkHeadSha:'0123456789012345678901234567890123456789',
        checkCompletedAt:Date.now()
      }] as never;
      throw new Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-check-run');
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await host.getByRole('button',{name:'Load sources'}).click();
  await expect(host.getByRole('button',{name:'Observe authenticated Check Run'})).toHaveCount(0);
  await host.getByRole('button',{name:'Enable Observe'}).click();
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('button',{name:'Poll authenticated GitHub'})).toHaveCount(0);
  const input=host.getByRole('textbox',{name:'Check Run ID'});
  await input.fill('0');
  await host.getByRole('button',{name:'Observe authenticated Check Run'}).click();
  await expect(host.locator('.sa-feedback')).toContainText('Enter a positive, bounded Check Run ID');
  await host.getByRole('textbox',{name:'Check Run ID'}).fill('240');
  await host.getByRole('button',{name:'Observe authenticated Check Run'}).click();
  await expect(host.locator('.sa-feedback')).toContainText('observed_permitted');
  await host.getByRole('button',{name:'View decisions'}).click();
  const timeline=host.getByRole('region',{name:'Source decisions'});
  await expect(timeline).toContainText('Observed Check: failure');
  await expect(timeline).toContainText('Head 012345678901');
  const calls=await page.evaluate(()=>(window as unknown as {
    saCheckCalls:{command:string;runId?:number}[]
  }).saCheckCalls);
  expect(calls.filter(c=>c.command==='superagent_source_observe_check_run')).toEqual([
    {command:'superagent_source_observe_check_run',runId:240}
  ]);
  expect(calls.filter(c=>c.command==='superagent_submit'||
    c.command==='superagent_dispatch_worker'||c.command==='superagent_source_poll_authenticated'
  )).toHaveLength(0);
});

test('multi-member Team Builder updates settled history safely without launching work',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-configured-roster';document.body.appendChild(host);
    const calls:string[]=[];(window as unknown as {saRosterCalls:string[]}).saRosterCalls=calls;
    const config={id:'team',name:'Security-critical Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',policyRevision:1,enabled:false,ticketCount:1};
    let roster={orchestrationId:'team',policyRevision:1,coordinatorBotId:'coordinator',
      workerBotIds:['worker'],reviewerBotIds:[],executionReady:true};
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker A',paused:false},
        {id:'worker-b',name:'Worker B',paused:false},
        {id:'worker-c',name:'Worker C',paused:false},
        {id:'review-a',name:'Reviewer A',paused:false},
        {id:'review-b',name:'Reviewer B',paused:false},
      ] as never;
      if(command==='superagent_list')return [{...config}] as never;
      if(command==='superagent_tickets')return [{id:'settled-ticket',state:'completed',objective:'Previous completed demand',revision:1}] as never;
      if(command==='superagent_roster')return {...roster} as never;
      if(command==='superagent_roster_update') {
        if(args?.expectedRevision!==config.policyRevision)throw Error('Native policy revision mismatch');
        const input=args?.input as {workerBotIds:string[];reviewerBotIds:string[]};
        if(input.workerBotIds.join(',')!=='worker,worker-c,worker-b' ||
          input.reviewerBotIds.join(',')!=='review-a,review-b')throw Error('Role order validation failed');
        config.policyRevision++;
        roster={...roster,workerBotIds:input.workerBotIds,reviewerBotIds:input.reviewerBotIds,
          policyRevision:config.policyRevision,executionReady:false};
        return {...roster} as never;
      }
      throw Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-configured-roster');
  await host.getByRole('button',{name:'Load native members'}).click();
  const form=host.locator('form[data-sa-form="roster"]');
  // Reviewer draft must survive selecting and reordering Workers before Save.
  await form.locator('input[name="reviewers"][value="review-a"]').check();
  await form.locator('input[name="reviewers"][value="review-b"]').check();
  await form.locator('input[name="additionalWorkers"][value="worker-b"]').check();
  await form.locator('input[name="additionalWorkers"][value="worker-c"]').check();
  await expect(form.locator('input[name="reviewers"]:checked')).toHaveCount(2);
  await form.getByRole('button',{name:'Move Worker C up'}).click();
  await expect(form.locator('input[name="reviewers"]:checked')).toHaveCount(2);
  await expect(form.locator('.sa-worker-order li').first()).toContainText('Worker C');
  await expect(form.locator('input[name="reviewers"][value="review-a"]')).toBeChecked();
  await expect(form.locator('input[name="reviewers"][value="review-b"]')).toBeChecked();
  await expect(form.getByRole('button',{name:'Save configured roles'})).toBeEnabled();
  await form.getByRole('button',{name:'Save configured roles'}).click();
  await expect(host.getByText('Multi-member execution is disabled',{exact:false})).toBeVisible();
  await expect(host.getByRole('region',{name:'Configured Team roles'}))
    .toContainText('Policy revision 2');
  await expect(host.locator('.sa-worker-order li').first()).toContainText('Worker C');
  const calls=await page.evaluate(()=>(window as unknown as {saRosterCalls:string[]}).saRosterCalls);
  expect(calls.filter(value=>value==='superagent_roster_update')).toHaveLength(1);
  expect(calls.filter(value=>value==='superagent_submit' || value==='superagent_set_enabled')).toHaveLength(0);
});


test('paused reviewer remains in the native Team edit payload until explicitly changed', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-paused-reviewer';
    document.body.appendChild(host);
    const calls:unknown[]=[];
    (window as unknown as {saPausedReviewerCalls:unknown[]}).saPausedReviewerCalls=calls;
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'co',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
        {id:'worker-b',name:'Additional Worker',paused:false},
        {id:'review',name:'Paused Reviewer',paused:true,avatar:'panda:2'},
      ] as never;
      if(command==='superagent_list')return [{
        id:'team',name:'Review Team',coordinatorBotId:'co',workerBotId:'worker',
        enabled:false,policyRevision:1,workerCount:1,reviewerCount:1,
      }] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_roster')return {
        orchestrationId:'team',policyRevision:1,coordinatorBotId:'co',
        workerBotIds:['worker'],reviewerBotIds:['review'],maxWorkerAttempts:2,
        executionReady:false,
      } as never;
      if(command==='superagent_roster_update'){
        calls.push(args);
        throw Error('Paused member must be reviewed before a native policy change.');
      }
      throw Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-paused-reviewer');
  await host.getByRole('button',{name:'Load native members'}).click();
  const form=host.locator('form[data-sa-form="roster"]');
  const paused=form.locator('input[name="reviewers"][value="review"]');
  await expect(paused).toBeChecked();
  await expect(paused).toBeDisabled();
  await form.locator('input[name="additionalWorkers"][value="worker-b"]').check();
  await form.getByRole('button',{name:'Save configured roles'}).click();
  await expect(host.getByRole('alert')).toContainText('Paused member must be reviewed');
  const attempts=await page.evaluate(()=>
    (window as unknown as {saPausedReviewerCalls:Array<{input:{reviewerBotIds:string[];workerBotIds:string[]}}>})
      .saPausedReviewerCalls);
  expect(attempts).toHaveLength(1);
  expect(attempts[0].input.reviewerBotIds).toEqual(['review']);
  expect(attempts[0].input.workerBotIds).toEqual(['worker','worker-b']);
});

test('cross-Team Demands index is native-backed and opens only the original Team Ticket',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-cross-team-runs';document.body.appendChild(host);
    const calls:string[]=[];(window as unknown as {saCrossCalls:string[]}).saCrossCalls=calls;
    const configs=[
      {id:'team-a',name:'Economy',coordinatorBotId:'router',workerBotId:'worker',
        enabled:true,policyRevision:2},
      {id:'team-b',name:'Performance',coordinatorBotId:'router',workerBotId:'worker',
        enabled:true,policyRevision:3},
    ];
    const run={id:'demand-b',orchestrationId:'team-b',objective:'Inspect build logs',
      state:'waiting_approval',revision:4,updatedAt:1794470400000,evidence:'private'};
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'router',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return configs as never;
      if(command==='superagent_tickets')return [] as never; // older than local Team page
      if(command==='superagent_all_demands')return [{
        id:run.id,orchestrationId:run.orchestrationId,objective:run.objective,
        state:run.state,revision:run.revision,updatedAt:run.updatedAt,
      }] as never;
      if(command==='superagent_snapshot') {
        if(args?.ticketId!==run.id)throw Error('Cross-Team Ticket leak');
        const wrongOwner=(window as unknown as {saCrossWrongOwner?:boolean}).saCrossWrongOwner;
        return {ticket:wrongOwner?{...run,orchestrationId:'team-a'}:run,mandate:{actor:'local-user',authorizationId:'native',
          projectId:'project-b',policyRevision:3,expiresAt:1800000000000,
          revoked:false,allowedTools:[],allowedSideEffects:false},
          assignments:[],activity:[],council:null} as never;
      }
      if(command==='superagent_source_thread_links')return [] as never;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-cross-team-runs');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(global).toContainText('Performance');
  await expect(global).toContainText('Inspect build logs');
  const board=global.getByRole('group',{name:'Demand status board'});
  await expect(board).toBeVisible();
  await expect(board.getByRole('region',{name:'Awaiting decision Demands'})).toContainText('Inspect build logs');
  await expect(board.getByRole('region',{name:'Working Demands'})).toContainText('No Demands in this state.');
  await global.getByRole('button',{name:'List',exact:true}).click();
  await expect(board).toHaveCount(0);
  await expect(global.locator('.sa-global-demand-list')).toContainText('Performance');
  await global.getByRole('button',{name:'Board',exact:true}).click();
  await expect(board).toBeVisible();
  await global.getByRole('button',{name:'Open demand'}).click();
  await expect(host.getByRole('region',{name:'Selected orchestration'}))
    .toContainText('Performance');
  await expect(host.getByRole('region',{name:'Ticket evidence'}))
    .toContainText('Ticket ID');
  const calls=await page.evaluate(()=>(window as unknown as {saCrossCalls:string[]}).saCrossCalls);
  expect(calls.filter(value=>value==='superagent_all_demands')).toHaveLength(1);
  expect(calls.filter(value=>value==='superagent_accept'||value==='superagent_submit')).toHaveLength(0);
  // A full native refresh must not evict an opened Run outside the Team's
  // bounded latest-50 page, or silently discard its revision-bound inspector.
  const team=host.getByRole('region',{name:'Selected orchestration'});
  await team.getByRole('button',{name:'Refresh'}).click();
  await expect(host.getByRole('region',{name:'Ticket evidence'})).toContainText('demand-b');
  await expect(team.getByRole('button',{name:'Accept evidence'})).toBeVisible();
  // A same-ID snapshot owned by another Team is never a valid substitute.
  await page.evaluate(() => {
    (window as unknown as {saCrossWrongOwner:boolean}).saCrossWrongOwner=true;
  });
  await team.getByRole('button',{name:'Refresh'}).click();
  await expect(host.getByRole('alert')).toContainText('Native demand ownership changed');
  await expect(host.getByRole('region',{name:'Ticket evidence'})).toHaveCount(0);
  await expect(team.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
  const after=await page.evaluate(()=>(window as unknown as {saCrossCalls:string[]}).saCrossCalls);
  expect(after.filter(value=>value==='superagent_accept'||value==='superagent_submit')).toHaveLength(0);
});


test('global Demand Stop checks native ownership/revision before confirmed revocation', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-global-stop-fixture';
    document.body.appendChild(host);
    type Trace = {command:string;args?:Record<string,unknown>};
    const trace:Trace[] = [];
    (window as unknown as {saGlobalStopTrace:Trace[];saGlobalStopStale:boolean}).saGlobalStopTrace=trace;
    (window as unknown as {saGlobalStopStale:boolean}).saGlobalStopStale=true;
    let demand = {id:'run-stop',orchestrationId:'team-b',objective:'Stop bounded work',
      state:'running',revision:6,updatedAt:1794470400000,projectId:'project-b',evidence:null};
    const teams=[{id:'team-a',name:'Economy',coordinatorBotId:'lead',workerBotId:'worker',
      enabled:true,policyRevision:2},{id:'team-b',name:'Performance',
      coordinatorBotId:'lead',workerBotId:'worker',enabled:true,policyRevision:3}];
    mountSuperagent(host, async (command,args) => {
      trace.push({command,args});
      if (command==='bots_list') return [
        {id:'lead',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command==='superagent_list') return teams as never;
      if (command==='superagent_tickets') return [] as never;
      if (command==='superagent_all_demands') return [{...demand}] as never;
      if (command==='superagent_snapshot') {
        if (args?.ticketId!==demand.id) throw Error('Wrong Ticket inspected');
        const stale=(window as unknown as {saGlobalStopStale:boolean}).saGlobalStopStale;
        return {ticket:stale?{...demand,revision:demand.revision+1}:{...demand},
          mandate:{actor:'local-user',authorizationId:'native',projectId:'project-b',
            policyRevision:3,expiresAt:1800000000000,revoked:false,
            allowedTools:[],allowedSideEffects:false},
          assignments:[],activity:[],council:null} as never;
      }
      if (command==='superagent_stop') {
        if (args?.ticketId!==demand.id || args?.expectedRevision!==demand.revision)
          throw Error('Native revision guard not supplied');
        demand={...demand,state:'stopped',revision:demand.revision+1};
        return {...demand} as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-global-stop-fixture');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(global.getByRole('button',{name:'Cancel Demand'})).toBeVisible();
  await global.getByRole('button',{name:'Cancel Demand'}).click();
  await expect(host.getByRole('alert')).toContainText('Demand changed since the native page');
  await expect(host.getByRole('alertdialog')).toHaveCount(0);
  let trace=await page.evaluate(()=>
    (window as unknown as {saGlobalStopTrace:{command:string}[]}).saGlobalStopTrace);
  expect(trace.filter(item=>item.command==='superagent_stop')).toHaveLength(0);
  await page.evaluate(() => {
    (window as unknown as {saGlobalStopStale:boolean}).saGlobalStopStale=false;
  });
  await global.getByRole('button',{name:'Cancel Demand'}).click();
  const confirm=host.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(confirm).toContainText('revision 6');
  trace=await page.evaluate(()=>
    (window as unknown as {saGlobalStopTrace:{command:string}[]}).saGlobalStopTrace);
  expect(trace.filter(item=>item.command==='superagent_stop')).toHaveLength(0);
  await confirm.getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('status').filter({hasText:'run-stop stopped'})).toBeVisible();
  // The UI must replace the stale cross-Team index with a fresh native
  // result without an extra operator click; Stop must no longer be offered.
  await expect(global).toContainText('Stop bounded work');
  await expect(global).toContainText('stopped');
  await expect(global.getByRole('button',{name:'Cancel Demand'})).toHaveCount(0);
  trace=await page.evaluate(()=>
    (window as unknown as {saGlobalStopTrace:{command:string;args?:Record<string,unknown>}[]}).saGlobalStopTrace);
  expect(trace.filter(item=>item.command==='superagent_stop')).toHaveLength(1);
  expect(trace.find(item=>item.command==='superagent_stop')?.args)
    .toMatchObject({ticketId:'run-stop',expectedRevision:6});
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(global.getByRole('button',{name:'Cancel Demand'})).toHaveCount(0);
  await expect(global).toContainText('stopped');
});

test('cross-Team blocked Demand retry requires current native identity, confirmation and bounded admission', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div'); host.id = 'sa-global-retry-fixture';
    document.body.appendChild(host);
    type Mode = 'stale-first' | 'stale-confirm' | 'native-denied' | 'valid';
    type Trace = {command:string;args?:Record<string,unknown>};
    let mode:Mode = 'stale-first', reads = 0, enabled = true;
    const trace:Trace[] = [];
    (window as unknown as {saRetryTrace:Trace[]}).saRetryTrace = trace;
    (window as unknown as {
      saRetryMode:(next:Mode)=>void;saRetryEnabled:(next:boolean)=>void;
    }).saRetryMode = next => {mode = next;reads = 0;};
    (window as unknown as {saRetryEnabled:(next:boolean)=>void}).saRetryEnabled =
      next => {enabled = next;};
    let demand = {id:'retry-7',orchestrationId:'team-b',objective:'Recover scoped Worker',
      state:'blocked',revision:7,updatedAt:1794470400000,projectId:'project-b',evidence:null};
    mountSuperagent(host,async(command,args)=>{
      trace.push({command,args});
      if(command==='bots_list')return [
        {id:'lead',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:false,policyRevision:2},
        {id:'team-b',name:'Performance',coordinatorBotId:'lead',workerBotId:'worker',
          enabled,reviewerCount:0,policyRevision:3},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands')return [{...demand}] as never;
      if(command==='superagent_snapshot'){
        if(args?.ticketId!==demand.id)throw Error('Wrong native Ticket identity');
        reads++;
        const stale = mode==='stale-first' || (mode==='stale-confirm' && reads>1);
        return {ticket:{...demand,revision:demand.revision+(stale?1:0)},
          mandate:{actor:'local-user',authorizationId:'native',projectId:'project-b',
            policyRevision:3,expiresAt:1800000000000,revoked:false,
            allowedTools:[],allowedSideEffects:false},
          assignments:[],activity:[],council:null} as never;
      }
      if(command==='superagent_retry'){
        if(mode==='native-denied')throw Error('Original mandate expired; native retry denied.');
        if(args?.ticketId!==demand.id)throw Error('Incorrect native retry Ticket');
        demand={...demand,state:'running',revision:demand.revision+1};
        return {...demand} as never;
      }
      throw Error('Unexpected native operation: '+command);
    });
  });
  const host=page.locator('#sa-global-retry-fixture');
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  const retry=global.getByRole('button',{name:'Retry demand'});
  await expect(retry).toBeVisible();
  await retry.click();
  await expect(host.getByRole('alert')).toContainText('Demand changed since the native page');
  await expect(host.getByRole('alertdialog')).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {saRetryMode:(v:string)=>void})
    .saRetryMode('stale-confirm'));
  await retry.click();
  const confirm=host.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(confirm).toContainText('revision 7');
  await expect(confirm).toContainText('read-only Worker attempt');
  await confirm.getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('Demand changed before confirmation');
  let calls=await page.evaluate(()=>
    (window as unknown as {saRetryTrace:{command:string;args?:Record<string,unknown>}[]}).saRetryTrace);
  expect(calls.filter(call=>call.command==='superagent_retry')).toHaveLength(0);
  await page.evaluate(()=>(window as unknown as {saRetryMode:(v:string)=>void})
    .saRetryMode('native-denied'));
  await retry.click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('Original mandate expired');
  await expect(retry).toBeVisible();
  await page.evaluate(()=>(window as unknown as {saRetryMode:(v:string)=>void})
    .saRetryMode('valid'));
  await retry.click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(global).toContainText('running');
  await expect(global.getByRole('button',{name:'Retry demand'})).toHaveCount(0);
  await expect(host.getByRole('status').filter({hasText:'Native retry admitted'})).toBeVisible();
  calls=await page.evaluate(()=>
    (window as unknown as {saRetryTrace:{command:string;args?:Record<string,unknown>}[]})
      .saRetryTrace);
  expect(calls.filter(call=>call.command==='superagent_retry')).toHaveLength(2);
  expect(calls.find(call=>call.command==='superagent_retry')?.args)
    .toMatchObject({ticketId:'retry-7'});
  expect(calls.some(call=>/grant|accept|effect_reserve|council_pass/.test(call.command)))
    .toBe(false);
});

test('cross-Team native Demand pages load older Tickets without inferring new work', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-paged-demands'; document.body.appendChild(host);
    const pages: Record<string,unknown>[]=[];
    (window as unknown as {saDemandPages:Record<string,unknown>[]}).saDemandPages=pages;
    const makeDemand=(index:number)=>({
      id:'demand-'+index,orchestrationId:'team',objective:'Ticket '+index,
      state:'blocked',revision:1,updatedAt:2000-index,
    });
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [{id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [{id:'team',name:'Delivery Team',
        coordinatorBotId:'router',workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query = args?.query as Record<string, unknown> | undefined;
        if (!query) throw Error('Native Demand query envelope required');
        pages.push({...query});
        if(query?.beforeId==='demand-199'&&query?.beforeUpdatedAt===1801)
          return [makeDemand(200),makeDemand(201)] as never;
        if(!query?.beforeId)return Array.from({length:200},(_,index)=>makeDemand(index)) as never;
        throw Error('Native pagination cursor mismatch');
      }
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-paged-demands');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(global).toContainText('200 native Demands loaded.');
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(global).toContainText('202 native Demands loaded.');
  await expect(global.getByRole('button',{name:'Load older Demands'})).toHaveCount(0);
  await global.getByRole('button',{name:'List',exact:true}).click();
  await expect(global.locator('.sa-global-demand-list li')).toHaveCount(202);
  await expect(global).toContainText('Ticket 201');
  const cursors=await page.evaluate(()=>
    (window as unknown as {saDemandPages:Record<string,unknown>[]}).saDemandPages);
  expect(cursors).toHaveLength(2);
  expect(cursors[1]).toMatchObject({limit:200,beforeUpdatedAt:1801,beforeId:'demand-199'});
});

test('cross-Team Demand pagination rejects overlapping, reordered and foreign Team receipts', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-invalid-demand-pages';
    document.body.appendChild(host);
    type Mode = 'duplicate' | 'out-of-order' | 'foreign' | 'invalid-revision' | 'invalid-objective' | 'valid';
    (window as unknown as {saDemandPageMode:Mode}).saDemandPageMode='duplicate';
    const calls:string[]=[];
    (window as unknown as {saDemandPageCalls:string[]}).saDemandPageCalls=calls;
    const demand=(index:number)=>({
      id:'demand-'+index,orchestrationId:'team',objective:'Task '+index,
      state:'blocked',revision:1,updatedAt:2000-index,projectId:'p',
    });
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{
        id:'team',name:'Delivery Team',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:1,projectId:'p',
      }] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query=args?.query as Record<string,unknown> | undefined;
        if(query?.orchestrationId!=='team')return [] as never;
        if(!query.beforeId)return Array.from({length:200},(_,i)=>demand(i)) as never;
        if(query.beforeId!=='demand-199' || query.beforeUpdatedAt!==1801)
          throw Error('Incorrect native keyset cursor');
        const mode=(window as unknown as {saDemandPageMode:Mode}).saDemandPageMode;
        if(mode==='duplicate')return [demand(198)] as never;
        if(mode==='out-of-order')return [{...demand(200),updatedAt:1802}] as never;
        if(mode==='foreign')return [{...demand(200),orchestrationId:'other'}] as never;
        if(mode==='invalid-revision')return [{...demand(200),revision:-1}] as never;
        if(mode==='invalid-objective')return [{...demand(200),objective:null}] as never;
        return [demand(200)] as never;
      }
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-invalid-demand-pages');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await global.getByRole('combobox',{name:'Filter Demands by Team'}).selectOption('team');
  await expect(global).toContainText('200 native Demands loaded.');
  for(const mode of ['duplicate','out-of-order','foreign','invalid-revision','invalid-objective'] as const){
    await page.evaluate(value=>{
      (window as unknown as {saDemandPageMode:string}).saDemandPageMode=value;
    },mode);
    await global.getByRole('button',{name:'Load older Demands'}).click();
    await expect(host.getByRole('alert')).toContainText(mode.startsWith('invalid-')
      ? 'Native Demand continuation has malformed Ticket' : 'Native Demand page overlaps');
    await expect(global).toContainText('200 native Demands loaded.');
  }
  await page.evaluate(()=>{
    (window as unknown as {saDemandPageMode:string}).saDemandPageMode='valid';
  });
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(global).toContainText('201 native Demands loaded.');
  await expect(global.getByRole('button',{name:'Load older Demands'})).toHaveCount(0);
  const calls=await page.evaluate(()=>
    (window as unknown as {saDemandPageCalls:string[]}).saDemandPageCalls);
  expect(calls.filter(command=>command==='superagent_all_demands')).toHaveLength(7);
  expect(calls.filter(command=>command==='superagent_stop'||command==='superagent_submit')).toHaveLength(0);
});

test('native Demand first page denies foreign Team, state, project and duplicate receipts', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div'); host.id = 'sa-first-page-boundary';
    document.body.appendChild(host);
    type Mode = 'foreign-team' | 'wrong-state' | 'wrong-project' | 'duplicate' | 'valid';
    let mode: Mode = 'foreign-team';
    const calls: string[] = [];
    (window as unknown as {setSaFirstPageMode:(next:Mode)=>void}).setSaFirstPageMode =
      next => { mode = next; };
    (window as unknown as {saFirstPageCalls:string[]}).saFirstPageCalls = calls;
    mountSuperagent(host, async (command, args) => {
      calls.push(command);
      if (command === 'bots_list') return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command === 'superagent_list') return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:false,policyRevision:2},
        {id:'team-b',name:'Performance',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:false,policyRevision:2},
      ] as never;
      if (command === 'superagent_tickets') return [] as never;
      if (command === 'superagent_all_demands') {
        const query = args?.query as Record<string,unknown> | undefined;
        if (query?.orchestrationId !== 'team-a') return [] as never;
        const row = {id:'demand-a',orchestrationId:'team-a',objective:'Native scoped demand',
          state:'blocked',projectId:'project-a',revision:2,updatedAt:2000};
        if (mode === 'foreign-team') return [{...row,orchestrationId:'team-b'}] as never;
        if (mode === 'wrong-state') return [{...row,state:'completed'}] as never;
        if (mode === 'wrong-project') return [{...row,projectId:'project-b'}] as never;
        if (mode === 'duplicate') return [row,row] as never;
        return [row] as never;
      }
      throw Error('Unexpected native operation: '+command);
    });
  });
  const host = page.locator('#sa-first-page-boundary');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global = host.getByRole('region',{name:'All Demands across Teams'});
  const rejected = 'Native Demand index returned invalid, duplicate or out-of-scope records';
  await global.getByRole('combobox',{name:'Filter Demands by Team'}).selectOption('team-a');
  await expect(host.getByRole('alert')).toContainText(rejected);
  await expect(global.getByRole('button',{name:'Open demand'})).toHaveCount(0);
  await page.evaluate(() =>
    (window as unknown as {setSaFirstPageMode:(mode:'wrong-state')=>void})
      .setSaFirstPageMode('wrong-state'));
  await global.getByRole('combobox',{name:'Filter Demands by State'}).selectOption('blocked');
  await expect(host.getByRole('alert')).toContainText(rejected);
  await page.evaluate(() =>
    (window as unknown as {setSaFirstPageMode:(mode:'wrong-project')=>void})
      .setSaFirstPageMode('wrong-project'));
  await global.getByRole('textbox',{name:'Exact admitted Project ID'}).fill('project-a');
  await global.getByRole('button',{name:'Apply project filter'}).click();
  await expect(host.getByRole('alert')).toContainText(rejected);
  await page.evaluate(() =>
    (window as unknown as {setSaFirstPageMode:(mode:'duplicate')=>void})
      .setSaFirstPageMode('duplicate'));
  await global.getByRole('button',{name:'Refresh selected Team'}).click();
  await expect(host.getByRole('alert')).toContainText(rejected);
  await expect(global.getByRole('button',{name:'Cancel Demand'})).toHaveCount(0);
  await page.evaluate(() =>
    (window as unknown as {setSaFirstPageMode:(mode:'valid')=>void})
      .setSaFirstPageMode('valid'));
  await global.getByRole('button',{name:'Refresh selected Team'}).click();
  await expect(global).toContainText('1 native Demands loaded.');
  await expect(global).toContainText('Native scoped demand');
  const calls = await page.evaluate(() =>
    (window as unknown as {saFirstPageCalls:string[]}).saFirstPageCalls);
  expect(calls.filter(command => command === 'superagent_all_demands')).toHaveLength(5);
  expect(calls.some(command => /submit|dispatch|stop|grant|accept/.test(command))).toBe(false);
});


test('Demand search rejects out-of-scope native rows and unordered first pages without exposing controls', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-demand-search-identity';
    document.body.appendChild(host);
    type Mode='wrong-first'|'unordered-first'|'valid-first'|'wrong-next'|'valid-next';
    let mode:Mode='wrong-first';
    const calls:string[]=[];
    (window as unknown as {saSetSearchMode:(value:Mode)=>void}).saSetSearchMode =
      value => { mode=value; };
    (window as unknown as {saScopeCommands:string[]}).saScopeCommands=calls;
    const demand=(index:number,objective:string)=>({
      id:'demand-'+index,orchestrationId:'team',objective,state:'blocked',
      revision:1,updatedAt:2000-index,projectId:'project',
    });
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [{
        id:'team',name:'Search Team',coordinatorBotId:'lead',
        workerBotId:'worker',enabled:false,policyRevision:1,projectId:'project',
      }] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query=args?.query as Record<string,unknown> | undefined;
        if(query?.searchQuery!=='NEEDLE') return [] as never;
        if(!query.beforeId) {
          if(mode==='wrong-first')return [demand(0,'Unrelated task')] as never;
          if(mode==='unordered-first')return [
            demand(1,'Needle older'),demand(0,'NEEDLE newer'),
          ] as never;
          return Array.from({length:200},(_,i)=>demand(i,'Needle '+i)) as never;
        }
        if(query.beforeId!=='demand-199'||query.beforeUpdatedAt!==1801)
          throw Error('Wrong native continuation cursor');
        if(mode==='wrong-next')return [demand(200,'Unrelated next page')] as never;
        return [demand(200,'Needle page 201')] as never;
      }
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-demand-search-identity');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  const search=global.getByRole('searchbox',{name:'Search Demands by objective'});
  await search.fill('NEEDLE');
  await search.press('Enter');
  const scopeError='Native Demand index returned invalid, duplicate or out-of-scope records';
  await expect(host.getByRole('alert')).toContainText(scopeError);
  await expect(global.getByRole('button',{name:'Cancel Demand'})).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {saSetSearchMode:(value:string)=>void})
    .saSetSearchMode('unordered-first'));
  await search.press('Enter');
  await expect(host.getByRole('alert')).toContainText(scopeError);
  await expect(global.getByRole('button',{name:'Open demand'})).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {saSetSearchMode:(value:string)=>void})
    .saSetSearchMode('valid-first'));
  await search.press('Enter');
  await expect(global).toContainText('200 native Demands loaded.');
  await page.evaluate(()=>(window as unknown as {saSetSearchMode:(value:string)=>void})
    .saSetSearchMode('wrong-next'));
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(host.getByRole('alert')).toContainText('Native Demand page overlaps');
  await expect(global).toContainText('200 native Demands loaded.');
  await page.evaluate(()=>(window as unknown as {saSetSearchMode:(value:string)=>void})
    .saSetSearchMode('valid-next'));
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(global).toContainText('201 native Demands loaded.');
  const calls=await page.evaluate(()=>
    (window as unknown as {saScopeCommands:string[]}).saScopeCommands);
  expect(calls.filter(command=>command==='superagent_all_demands')).toHaveLength(5);
  expect(calls.some(command=>/submit|dispatch|stop|accept|grant|reserve/.test(command))).toBe(false);
});

test('native Team filter pages independently and resets pagination when changing Teams', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-filtered-demands';
    document.body.appendChild(host);
    const queries:Record<string,unknown>[]=[];
    (window as unknown as {saFilteredQueries:Record<string,unknown>[]}).saFilteredQueries=queries;
    const demand=(id:string,team:string,at:number)=>({
      id,orchestrationId:team,objective:'Task '+id,state:'blocked',revision:1,updatedAt:at,
    });
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:true,policyRevision:2},
        {id:'team-b',name:'Performance',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:true,policyRevision:2},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query = args?.query as Record<string, unknown> | undefined;
        if (!query) throw Error('Native Demand query envelope required');
        queries.push({...query});
        if(query?.stateFilter==='running')return [] as never;
        if(query?.stateFilter && query.stateFilter!=='blocked')
          throw Error('Invalid native Demand state');
        if(query?.orchestrationId==='team-b'){
          if(query?.beforeId==='b-199' && query?.beforeUpdatedAt===1801)
            return [demand('b-200','team-b',1800)] as never;
          if(!query?.beforeId)return Array.from({length:200},(_,i)=>
            demand('b-'+i,'team-b',2000-i)) as never;
        }
        if(query?.orchestrationId==='team-a' && !query?.beforeId)
          return [demand('a-0','team-a',1900)] as never;
        if(!query?.orchestrationId && !query?.beforeId)
          return [demand('b-0','team-b',2000),demand('a-0','team-a',1900)] as never;
        throw Error('Invalid native Team-scoped cursor');
      }
      throw Error('Unexpected native read: '+command);
    });
  });
  const global=page.locator('#sa-filtered-demands')
    .getByRole('region',{name:'All Demands across Teams'});
  await page.locator('#sa-filtered-demands')
    .getByRole('button',{name:'Demands / Runs'}).click();
  const filter=global.getByRole('combobox',{name:'Filter Demands by Team'});
  await filter.selectOption('team-b');
  await expect(global).toContainText('200 native Demands loaded.');
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(global).toContainText('201 native Demands loaded.');
  await filter.selectOption('team-a');
  await expect(global).toContainText('1 native Demands loaded.');
  await expect(global).toContainText('Task a-0');
  await expect(global).not.toContainText('Task b-200');
  await filter.selectOption('');
  await expect(global).toContainText('2 native Demands loaded.');
  const state=global.getByRole('combobox',{name:'Filter Demands by State'});
  await state.selectOption('running');
  await expect(global).toContainText('0 native Demands loaded.');
  await state.selectOption('blocked');
  await expect(global).toContainText('2 native Demands loaded.');
  const queries=await page.evaluate(()=>
    (window as unknown as {saFilteredQueries:Record<string,unknown>[]}).saFilteredQueries);
  expect(queries).toMatchObject([
    {limit:200,orchestrationId:'team-b'},
    {limit:200,orchestrationId:'team-b',beforeId:'b-199',beforeUpdatedAt:1801},
    {limit:200,orchestrationId:'team-a'},
    {limit:200,orchestrationId:undefined},
    {limit:200,orchestrationId:undefined,stateFilter:'running'},
    {limit:200,orchestrationId:undefined,stateFilter:'blocked'},
  ]);
  expect(queries).toHaveLength(6);
});



test('native project filter preserves immutable Ticket scope across pages and other Teams', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-project-filter';
    document.body.appendChild(host);
    const queries:Record<string,unknown>[]=[];
    (window as unknown as {saProjectQueries:Record<string,unknown>[]}).saProjectQueries=queries;
    const item=(id:string,team:string,at:number)=>({
      id,orchestrationId:team,objective:'Scoped '+id,state:'blocked',
      projectId:team==='team-a'?'project-a':'project-b',
      revision:0,updatedAt:at,
    });
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Team A',coordinatorBotId:'lead',workerBotId:'worker',
          projectId:'project-a',enabled:false,policyRevision:1},
        {id:'team-b',name:'Team B',coordinatorBotId:'lead',workerBotId:'worker',
          projectId:'project-b',enabled:false,policyRevision:1},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands') {
        const query=args?.query as Record<string,unknown>;
        queries.push({...query});
        if(query.projectId==='project-a') {
          if(query.beforeId==='a-199' && query.beforeUpdatedAt===1801)
            return [item('a-200','team-a',1800)] as never;
          if(!query.beforeId)
            return Array.from({length:200},(_,i)=>item('a-'+i,'team-a',2000-i)) as never;
        }
        if(query.projectId==='project-b' && !query.beforeId)
          return [item('b-0','team-b',2000)] as never;
        if(!query.projectId && !query.beforeId)
          return [item('b-0','team-b',2000),item('a-0','team-a',2000)] as never;
        throw Error('Unsupported native project cursor');
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-project-filter');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  const project=global.getByRole('combobox',{name:'Filter Demands by Project'});
  await project.selectOption('project-a');
  await expect(global).toContainText('200 native Demands loaded.');
  await global.getByRole('button',{name:'Load older Demands'}).click();
  await expect(global).toContainText('201 native Demands loaded.');
  await project.selectOption('project-b');
  await expect(global).toContainText('1 native Demands loaded.');
  await expect(global).toContainText('Scoped b-0');
  await expect(global).not.toContainText('Scoped a-200');
  await project.selectOption('');
  await expect(global).toContainText('2 native Demands loaded.');
  const queries=await page.evaluate(()=>
    (window as unknown as {saProjectQueries:Record<string,unknown>[]}).saProjectQueries);
  expect(queries).toMatchObject([
    {limit:200,projectId:'project-a'},
    {limit:200,projectId:'project-a',beforeId:'a-199',beforeUpdatedAt:1801},
    {limit:200,projectId:'project-b'},
    {limit:200,projectId:undefined},
  ]);
  expect(queries).toHaveLength(4);
});

test('historical native Ticket project stays visible and filterable after Team changes', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-historical-project';
    document.body.appendChild(host);
    const queries:Record<string,unknown>[]=[];
    (window as unknown as {saHistoricalQueries:Record<string,unknown>[]}).saHistoricalQueries=queries;
    const archived={id:'old-run',orchestrationId:'team',objective:'Historical run',
      state:'blocked',revision:2,updatedAt:2000,projectId:'archived-project'};
    const recent={id:'new-run',orchestrationId:'team',objective:'Recent run',
      state:'running',revision:1,updatedAt:3000,projectId:'current-project'};
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team',name:'Updated Team',coordinatorBotId:'lead',workerBotId:'worker',
          projectId:'current-project',enabled:false,policyRevision:2},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands'){
        const query=args?.query as Record<string,unknown>;
        queries.push({...query});
        if(query.projectId==='archived-project')return [archived] as never;
        if(query.projectId==='unknown-old-project')return [] as never;
        if(!query.projectId)return [recent,archived] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-historical-project');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  await global.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(global).toContainText('Project archived-project');
  await global.getByRole('combobox',{name:'Filter Demands by Project'})
    .selectOption('archived-project');
  await expect(global).toContainText('1 native Demands loaded.');
  await expect(global).toContainText('Historical run');
  await expect(global).not.toContainText('Recent run');
  await global.getByRole('textbox',{name:'Exact admitted Project ID'}).fill('unknown-old-project');
  await global.getByRole('button',{name:'Apply project filter'}).click();
  await expect(global).toContainText('0 native Demands loaded.');
  expect(await page.evaluate(() =>
    (window as unknown as {saHistoricalQueries:Record<string,unknown>[]}).saHistoricalQueries
  )).toMatchObject([
    {limit:200,projectId:undefined},
    {limit:200,projectId:'archived-project'},
    {limit:200,projectId:'unknown-old-project'},
  ]);
});

test('native objective search filters durable Demands before pagination without dispatch', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-native-objective-search';
    document.body.appendChild(host);
    const queries:Record<string,unknown>[]=[];
    (window as unknown as {saSearchQueries:Record<string,unknown>[]}).saSearchQueries=queries;
    const demand=(id:string,team:string,objective:string)=>({
      id,orchestrationId:team,objective,state:'blocked',revision:1,updatedAt:2000,
    });
    mountSuperagent(host,async(command,args) => {
      if(command==='bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:false,policyRevision:1},
        {id:'team-b',name:'Security',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:false,policyRevision:1},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_all_demands') {
        const query = args?.query as Record<string, unknown> | undefined;
        if (!query) throw Error('Native Demand query envelope required');
        queries.push({...query});
        if(query?.searchQuery==='NEEDLE')return [
          demand('match-b','team-b','NEEDLE in another Team'),
          demand('match-a','team-a','Needle hidden past page 200'),
        ] as never;
        if(query?.searchQuery)throw Error('Native objective filter mismatch');
        return [demand('recent','team-a','Recent unrelated work')] as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-native-objective-search');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const global=host.getByRole('region',{name:'All Demands across Teams'});
  const search=global.getByRole('searchbox',{name:'Search Demands by objective'});
  await search.fill('NEEDLE');
  await search.press('Enter');
  await expect(global).toContainText('2 native Demands loaded.');
  await expect(global).toContainText('Needle hidden past page 200');
  await expect(global).not.toContainText('Recent unrelated work');
  await expect(global).toContainText('matching objective "NEEDLE"');
  await global.getByRole('button',{name:'List',exact:true}).click();
  await expect(global.locator('.sa-global-demand-list li')).toHaveCount(2);
  await search.fill('');
  await global.getByRole('button',{name:'Search Demands'}).click();
  await expect(global).toContainText('Recent unrelated work');
  await expect(global).not.toContainText('matching objective "NEEDLE"');
  const queries=await page.evaluate(()=>
    (window as unknown as {saSearchQueries:Record<string,unknown>[]}).saSearchQueries);
  expect(queries).toMatchObject([
    {limit:200,searchQuery:'NEEDLE'},
    {limit:200,searchQuery:undefined},
  ]);
  expect(queries).toHaveLength(2);
});

test('manual demand preflight shows native read-only scope and never admits work', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-native-preflight-test'; document.body.appendChild(host);
    const calls: string[] = [];
    mountSuperagent(host, async (command, args) => {
      calls.push(command);
      if (command === 'bots_list') return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command === 'superagent_list') return [{id:'team',name:'Manual Team',
        coordinatorBotId:'router',workerBotId:'worker',enabled:true,policyRevision:2}] as never;
      if (command === 'superagent_tickets') return [] as never;
      if (command === 'superagent_preflight') {
        if (args?.orchestrationId !== 'team' || args?.objective !== 'Inspect changes')
          throw Error('Wrong preflight request');
        return {orchestrationId:'team',policyRevision:2,projectId:'isolated-project',
          selectedBotId:args.coordinated ? 'router' : 'worker',
          selectedRole:args.coordinated ? 'coordinator' : 'worker',
          eligible:true,reason:null,allowedTools:[],allowedSideEffects:false,
          attemptBudgetMs:1800000} as never;
      }
      throw Error('Unexpected command '+command);
    });
    (window as unknown as {saPreflightCalls:()=>string[]}).saPreflightCalls = () => calls;
  });
  const host = page.locator('#sa-native-preflight-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('textbox',{name:'Task objective'}).fill('Inspect changes');
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await host.getByRole('button',{name:'Review native preflight'}).click();
  const panel = host.getByRole('region',{name:'Native demand preflight'});
  await expect(panel).toContainText('Eligible for manual read-only admission now.');
  await expect(panel).toContainText('Router');
  await expect(panel).toContainText('isolated-project / 2');
  await expect(panel).toContainText('none / no side effects');
  await expect(panel).toContainText('not a grant, Ticket or guaranteed reservation');
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeEnabled();
  await expect(host.getByRole('textbox',{name:'Task objective'})).toHaveValue('Inspect changes');
  const calls = await page.evaluate(() =>
    (window as unknown as {saPreflightCalls:()=>string[]}).saPreflightCalls());
  expect(calls.filter(name=>name==='superagent_preflight')).toHaveLength(1);
  expect(calls.filter(name=>name.includes('submit')||name.includes('dispatch'))).toHaveLength(0);
  await host.getByRole('textbox',{name:'Task objective'}).fill('Changed objective');
  await expect(panel).toHaveCount(0);
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
});

test('manual Demand cannot dispatch a blocked or revision-stale native preflight', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-preflight-revalidation-test'; document.body.appendChild(host);
    const calls:string[]=[];
    let inspected=0;
    mountSuperagent(host, async (command) => {
      calls.push(command);
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Revision-sensitive Team',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:4,
      }] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_preflight') {
        inspected++;
        return {
          orchestrationId:'team',policyRevision:inspected===3?5:4,
          projectId:'safe-project',selectedBotId:'worker',selectedRole:'worker',
          eligible:inspected!==1,reason:inspected===1?'Worker occupied':null,
          allowedTools:[],allowedSideEffects:false,attemptBudgetMs:1800000,
        } as never;
      }
      throw Error('Unexpected native operation '+command);
    });
    (window as unknown as {saNativeAdmissionCalls:()=>string[]}).saNativeAdmissionCalls=()=>calls;
  });
  const host=page.locator('#sa-preflight-revalidation-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('textbox',{name:'Task objective'}).fill('Inspect a bounded fixture');
  await host.getByRole('combobox',{name:'Dispatch'}).selectOption('direct');
  const submit=host.getByRole('button',{name:'Dispatch manually'});
  await expect(submit).toBeDisabled();
  await host.getByRole('button',{name:'Review native preflight'}).click();
  await expect(host.getByRole('region',{name:'Native demand preflight'}))
    .toContainText('Not eligible for immediate admission: Worker occupied');
  await expect(submit).toBeDisabled();
  await host.getByRole('button',{name:'Review native preflight'}).click();
  await expect(submit).toBeEnabled();
  await submit.click();
  await expect(host.getByRole('alertdialog',{name:'Confirm orchestration action'}))
    .toContainText('No write grant is issued.');
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('alert')).toContainText(
    'Native admission scope or availability changed. Review a fresh preflight.');
  await expect(submit).toBeDisabled();
  const calls=await page.evaluate(() =>
    (window as unknown as {saNativeAdmissionCalls:()=>string[]}).saNativeAdmissionCalls());
  expect(calls.filter(command=>command==='superagent_preflight')).toHaveLength(3);
  expect(calls.filter(command=>command.includes('submit'))).toHaveLength(0);
});

test('manual Demand preserves native blocked-Ticket staging without granting write authority', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-stage-blocked-test'; document.body.appendChild(host);
    const calls:string[]=[];
    const tickets:Array<Record<string,unknown>>=[];
    mountSuperagent(host, async (command) => {
      calls.push(command);
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Queue Team',coordinatorBotId:'router',workerBotId:'worker',
        enabled:true,policyRevision:3,
      }] as never;
      if(command==='superagent_tickets') return tickets as never;
      if(command==='superagent_preflight') return {
        orchestrationId:'team',policyRevision:3,projectId:'project',
        selectedBotId:'worker',selectedRole:'worker',
        eligible:false,canStageBlocked:true,reason:'Worker or checkout occupied',
        allowedTools:[],allowedSideEffects:false,attemptBudgetMs:1800000,
      } as never;
      if(command==='superagent_submit') {
        tickets.push({id:'blocked-ticket',orchestrationId:'team',
          objective:'Queue a second task',state:'blocked',revision:1,evidence:null});
        return tickets[0] as never;
      }
      throw Error('Unexpected native command '+command);
    });
    (window as unknown as {saBlockedCalls:()=>string[]}).saBlockedCalls=()=>calls;
  });
  const host=page.locator('#sa-stage-blocked-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('textbox',{name:'Task objective'}).fill('Queue a second task');
  await host.getByRole('combobox',{name:'Dispatch'}).selectOption('direct');
  await expect(host.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await host.getByRole('button',{name:'Review native preflight'}).click();
  await expect(host.getByRole('region',{name:'Native demand preflight'}))
    .toContainText('blocked Demand may be durably queued');
  await host.getByRole('button',{name:'Submit busy Demand'}).click();
  await expect(host.getByRole('alertdialog',{name:'Confirm orchestration action'}))
    .toContainText('may start immediately');
  const before=await page.evaluate(() =>
    (window as unknown as {saBlockedCalls:()=>string[]}).saBlockedCalls());
  expect(before.filter(name=>name==='superagent_submit')).toHaveLength(0);
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.locator('.sa-tickets')).toContainText('blocked');
  const calls=await page.evaluate(() =>
    (window as unknown as {saBlockedCalls:()=>string[]}).saBlockedCalls());
  expect(calls.filter(name=>name==='superagent_preflight')).toHaveLength(2);
  expect(calls.filter(name=>name==='superagent_submit')).toHaveLength(1);
});

test('native-backed attempt inspector shows recorded sessions and run budgets without granting effects', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-native-attempt-test'; document.body.appendChild(host);
    (window as unknown as {saChatTargets: unknown[]}).saChatTargets = [];
    mountSuperagent(host, async command => {
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Team fixture',coordinatorBotId:'router',workerBotId:'worker',
        enabled:true,policyRevision:2
      }] as never;
      if(command==='superagent_tickets') return [{
        id:'ticket',orchestrationId:'team',objective:'Inspect native attempt',
        state:'running',revision:3,evidence:null
      }] as never;
      if(command==='superagent_source_thread_links') return [] as never;
      if(command==='superagent_snapshot') return {
        ticket:{id:'ticket',orchestrationId:'team',objective:'Inspect native attempt',
          state:'running',revision:3,evidence:null},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:2,
          projectId:'project',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[{id:'attempt-one',botId:'worker',sessionId:'session-one',
          state:'running',attempt:1,createdAt:1794400000000,updatedAt:1794400000200}],
        attemptInsights:[{assignmentId:'attempt-one',currentAssignment:true,
          hasResult:false,mandateRevoked:false,runBudgetExpired:false,
          remainingRunBudgetMs:240000}],
        activity:[]
      } as never;
      throw new Error('Unexpected native command: '+command);
    }, async route => {
      (window as unknown as {saChatTargets: unknown[]}).saChatTargets.push(route);
    });
  });
  const host=page.locator('#sa-native-attempt-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  await host.getByRole('button',{name:'Inspect attempt 1'}).click();
  const inspector=host.getByRole('region',{name:'Native assignment inspector'});
  await expect(inspector).toContainText('session-one');
  await expect(inspector).toContainText('240 seconds remain');
  await expect(inspector).toContainText('Recorded (not a tool grant)');
  await expect(inspector).toContainText('Not recorded');
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
  await inspector.getByRole('button',{name:'Open native chat'}).click();
  expect(await page.evaluate(() =>
    (window as unknown as {saChatTargets: unknown[]}).saChatTargets)).toEqual([
    {teamId:'team',ticketId:'ticket',assignmentId:'attempt-one',botId:'worker',sessionId:'session-one'}
  ]);
  await expect(host).toHaveAttribute('aria-busy','false');
  // Forged data attributes must not navigate to an unverified attempt.
  await page.evaluate(() => {
    const host = document.querySelector('#sa-native-attempt-test')!;
    const forged = document.createElement('button');
    forged.dataset.saOpenAssignmentChat = 'other-ticket-attempt';
    host.append(forged); forged.click(); forged.remove();
  });
  expect(await page.evaluate(() =>
    (window as unknown as {saChatTargets: unknown[]}).saChatTargets)).toHaveLength(1);
  await inspector.getByRole('button',{name:'Close attempt'}).click();
  await expect(inspector).toHaveCount(0);
});


test('native effect receipt inspection is lazy, task-scoped and cannot dispatch', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-effect-inspector-test'; document.body.appendChild(host);
    const seen:string[] = [];
    mountSuperagent(host, async command => {
      seen.push(command);
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Recovery Team',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:2
      }] as never;
      if(command==='superagent_tickets') return [{
        id:'ticket',orchestrationId:'team',objective:'Inspect reconciliation',
        state:'interrupted',revision:7,evidence:null
      }] as never;
      if(command==='superagent_source_thread_links') return [] as never;
      if(command==='superagent_snapshot') return {
        ticket:{id:'ticket',orchestrationId:'team',objective:'Inspect reconciliation',
          state:'interrupted',revision:7,evidence:null},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:2,
          projectId:'project',expiresAt:1800000000000,revoked:true,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[]
      } as never;
      if(command==='superagent_effect_receipts') return [
        {effectKey:'github:issue:42',payloadSha256:'f'.repeat(64),
          state:'uncertain',createdAt:1794400000000,updatedAt:1794400000100}
      ] as never;
      throw Error('Unexpected native command: '+command);
    });
    (window as unknown as {saEffectCalls:()=>string[]}).saEffectCalls=()=>seen;
  });
  const host=page.locator('#sa-effect-inspector-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  const region=host.getByRole('region',{name:'Native effect receipts'});
  await expect(region).toContainText('Receipts not loaded.');
  await region.getByRole('button',{name:'Inspect receipts'}).click();
  await expect(region).toContainText('github:issue:42');
  await expect(region).toContainText('uncertain');
  await expect(region).toContainText('independent provider reconciliation');
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
  const seen=await page.evaluate(() =>
    (window as unknown as {saEffectCalls:()=>string[]}).saEffectCalls());
  expect(seen.filter(value=>value==='superagent_effect_receipts')).toHaveLength(1);
  expect(seen.some(value=>/submit|dispatch|accept|reserve|acknowledge/.test(value))).toBe(false);
});


test('native cross-Team review inbox exposes pending states without synthetic Council approval', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-global-review-test'; document.body.appendChild(host);
    const seen:string[]=[];
    const demands=[
      {id:'needs-human',orchestrationId:'team-a',objective:'Evidence needs human',
        state:'waiting_approval',revision:4,updatedAt:1794400000300},
      {id:'awaits-council',orchestrationId:'team-b',objective:'Real Council unavailable',
        state:'waiting_review',revision:8,updatedAt:1794400000200}
    ];
    mountSuperagent(host, async command => {
      seen.push(command);
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return [
        {id:'team-a',name:'Economy',coordinatorBotId:'router',
          workerBotId:'worker',enabled:true,policyRevision:2},
        {id:'team-b',name:'Security',coordinatorBotId:'router',
          workerBotId:'worker',enabled:true,policyRevision:2}
      ] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_review_inbox') return demands as never;
      if(command==='superagent_snapshot') return {
        ticket:{id:'awaits-council',orchestrationId:'team-b',
          objective:'Real Council unavailable',state:'waiting_review',
          revision:8,evidence:null},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:2,
          projectId:'project',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],council:null
      } as never;
      if(command==='superagent_source_thread_links') return [] as never;
      throw Error('Unexpected native command: '+command);
    });
    (window as unknown as {saReviewCommands:()=>string[]}).saReviewCommands=()=>seen;
  });
  const host=page.locator('#sa-global-review-test');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(inbox).toContainText('Human decision');
  await expect(inbox).toContainText('Awaiting native Council (not approved)');
  await expect(inbox.locator('[data-sa-accept]')).toHaveCount(0);
  await inbox.getByRole('button',{name:'Open demand'}).nth(1).click();
  await expect(host.getByRole('region',{name:'Ticket evidence'}))
    .toContainText('awaits-council');
  await expect(host.getByRole('region',{name:'Selected orchestration'}))
    .toContainText('Security');
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
  const commands=await page.evaluate(()=>
    (window as unknown as {saReviewCommands:()=>string[]}).saReviewCommands());
  expect(commands.filter(c=>c==='superagent_review_inbox')).toHaveLength(1);
  expect(commands.some(c=>/submit|dispatch|accept|reserve/.test(c))).toBe(false);
});

test('cross-Team review inbox pages native decisions without implicit approval', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-review-pages';document.body.appendChild(host);
    const at=1794400000000;
    const rows=Array.from({length:204},(_,index)=>{
      const id='review-'+String(204-index).padStart(3,'0');
      return {id,orchestrationId:index%2?'team-a':'team-b',
        objective:'Native decision '+id,
        state:index%2?'waiting_approval':'waiting_review',
        revision:2,updatedAt:at,projectId:'project'};
    });
    const seen:Array<{command:string;beforeId?:unknown;beforeUpdatedAt?:unknown}>=[];
    (window as unknown as {saReviewPageCalls:typeof seen}).saReviewPageCalls=seen;
    mountSuperagent(host,async (command,args)=>{
      seen.push({command,beforeId:args?.beforeId,beforeUpdatedAt:args?.beforeUpdatedAt});
      if(command==='bots_list')return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'router',
          workerBotId:'worker',enabled:true,policyRevision:1},
        {id:'team-b',name:'Security',coordinatorBotId:'router',
          workerBotId:'worker',enabled:true,policyRevision:1}
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_review_inbox'){
        if(args?.beforeId===undefined)return rows.slice(0,200) as never;
        if(args.beforeId!=='review-005'||args.beforeUpdatedAt!==at||args.limit!==200)
          throw Error('Incorrect native keyset cursor');
        return rows.slice(200) as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-review-pages');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(200);
  await inbox.getByRole('button',{name:'Load older pending decisions'}).click();
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(204);
  await expect(inbox.getByRole('button',{name:'Load older pending decisions'})).toHaveCount(0);
  await expect(inbox).toContainText('204 native pending decisions loaded');
  await expect(inbox).toContainText('Awaiting native Council (not approved)');
  await expect(inbox.locator('[data-sa-accept]')).toHaveCount(0);
  const calls=await page.evaluate(()=>
    (window as unknown as {saReviewPageCalls:Array<{command:string;beforeId?:unknown;beforeUpdatedAt?:unknown}>})
      .saReviewPageCalls);
  expect(calls.filter(item=>item.command==='superagent_review_inbox')).toEqual([
    {command:'superagent_review_inbox',beforeId:undefined,beforeUpdatedAt:undefined},
    {command:'superagent_review_inbox',beforeId:'review-005',beforeUpdatedAt:1794400000000},
  ]);
  expect(calls.some(item=>/submit|dispatch|accept|reserve|grant/.test(item.command))).toBe(false);
});


test('Team review inbox uses native scoped keyset pages and rejects foreign Team receipts', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-team-review-test'; document.body.appendChild(host);
    const rows = Array.from({length:201},(_,i)=>({
      id:'review-'+String(201-i).padStart(3,'0'),orchestrationId:'team-a',
      objective:'Team A pending decision '+i,state:i%2?'waiting_review':'waiting_approval',
      revision:2,updatedAt:1794400200000-i,
    }));
    const calls: Array<{command:string;args?:Record<string,unknown>}> = [];
    let invalid = true;
    (window as unknown as {saReviewTeamTest:{
      calls:typeof calls; setInvalid:(value:boolean)=>void
    }}).saReviewTeamTest = {calls,setInvalid:value=>{invalid=value;}};
    mountSuperagent(host,async <T>(command:string,args?:Record<string,unknown>):Promise<T> => {
      calls.push({command,args});
      if(command==='bots_list')return [
        {id:'router',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as T;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Team A',coordinatorBotId:'router',
          workerBotId:'worker',enabled:false,policyRevision:1},
        {id:'team-b',name:'Team B',coordinatorBotId:'router',
          workerBotId:'worker',enabled:false,policyRevision:1},
      ] as T;
      if(command==='superagent_tickets')return [] as T;
      if(command==='superagent_team_review_inbox'){
        if(args?.orchestrationId!=='team-a'||args?.limit!==200)
          throw Error('Expected native Team review scope');
        if(!args.beforeId)return rows.slice(0,200) as T;
        if(args.beforeId!=='review-002'||args.beforeUpdatedAt!==1794400200000-199)
          throw Error('Invalid review cursor');
        return [{...rows[200],orchestrationId:invalid?'team-b':'team-a'}] as T;
      }
      if(command==='superagent_review_inbox')return [{
        id:'review-b',orchestrationId:'team-b',objective:'Team B pending',
        state:'waiting_review',revision:1,updatedAt:1794400200000,
      }] as T;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-team-review-test');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('combobox',{name:'Filter review inbox by Team'}).selectOption('team-a');
  await expect(inbox).toContainText('200 native pending decisions loaded.');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(200);
  await inbox.getByRole('button',{name:'Load older pending decisions'}).click();
  await expect(host.getByRole('alert')).toContainText('out-of-scope');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(200);
  await page.evaluate(()=>{
    (window as unknown as {saReviewTeamTest:{setInvalid:(value:boolean)=>void}})
      .saReviewTeamTest.setInvalid(false);
  });
  await inbox.getByRole('button',{name:'Load older pending decisions'}).click();
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(201);
  await expect(inbox.getByRole('button',{name:'Load older pending decisions'})).toHaveCount(0);
  await inbox.getByRole('combobox',{name:'Filter review inbox by Team'}).selectOption('');
  await expect(inbox).toContainText('Team B pending');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(1);
  const calls=await page.evaluate(()=>
    (window as unknown as {saReviewTeamTest:{
      calls:Array<{command:string;args?:Record<string,unknown>}>
    }}).saReviewTeamTest.calls);
  expect(calls.filter(c=>c.command==='superagent_team_review_inbox')).toHaveLength(3);
  expect(calls.filter(c=>c.command==='superagent_review_inbox')).toHaveLength(1);
  expect(calls.some(c=>/accept|submit|dispatch|grant|effect/.test(c.command))).toBe(false);
});


test('global native Review inbox refuses unordered first-page receipts before exposing decisions', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-review-order-test';document.body.appendChild(host);
    const calls:string[]=[];
    let unordered=true;
    (window as unknown as {saReviewOrder:{
      calls:string[];repair:()=>void;
    }}).saReviewOrder={calls,repair:()=>{unordered=false;}};
    const newest={id:'review-new',orchestrationId:'team-a',
      objective:'Newest pending Council review',state:'waiting_review',
      revision:2,updatedAt:1794400200000};
    const older={id:'review-old',orchestrationId:'team-b',
      objective:'Older human decision',state:'waiting_approval',
      revision:2,updatedAt:1794400100000};
    mountSuperagent(host,async command=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Team A',coordinatorBotId:'router',
          workerBotId:'worker',enabled:false,policyRevision:1},
        {id:'team-b',name:'Team B',coordinatorBotId:'router',
          workerBotId:'worker',enabled:false,policyRevision:1}
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_review_inbox')
        return (unordered?[older,newest]:[newest,older]) as never;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-review-order-test');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(host.getByRole('alert')).toContainText('out-of-scope decisions');
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {
    saReviewOrder:{repair:()=>void}
  }).saReviewOrder.repair());
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(inbox.getByRole('button',{name:'Open demand'})).toHaveCount(2);
  await expect(inbox).toContainText('2 native pending decisions loaded');
  const calls=await page.evaluate(()=>(window as unknown as {
    saReviewOrder:{calls:string[]}
  }).saReviewOrder.calls);
  expect(calls.filter(command=>command==='superagent_review_inbox')).toHaveLength(2);
  expect(calls.some(command=>/submit|dispatch|accept|reject|grant|reserve/.test(command))).toBe(false);
});

test('native Run graph is lazy, revision-bound and read-only', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-run-graph-test'; document.body.appendChild(host);
    const seen:string[]=[];
    mountSuperagent(host, async command => {
      seen.push(command);
      if(command==='bots_list') return [
        {id:'router',name:'Router',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Graph Team',coordinatorBotId:'router',
        workerBotId:'worker',enabled:true,policyRevision:2
      }] as never;
      if(command==='superagent_tickets') return [{
        id:'ticket',orchestrationId:'team',objective:'View actual assignment',
        state:'running',revision:4,evidence:null
      }] as never;
      if(command==='superagent_source_thread_links') return [] as never;
      if(command==='superagent_snapshot') return {
        ticket:{id:'ticket',orchestrationId:'team',objective:'View actual assignment',
          state:'running',revision:4,evidence:null},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:2,
          projectId:'project',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[{id:'asgn',botId:'worker',sessionId:'session',attempt:1,
          state:'running'}],activity:[]
      } as never;
      if(command==='superagent_graph') return {
        ticketId:'ticket',ticketRevision:4,truncatedAttempts:false,
        nodes:[
          {id:'ticket:ticket',kind:'ticket',state:'running',label:'Demand / Ticket',botId:null,attempt:null,active:false},
          {id:'assignment:asgn',kind:'assignment',state:'running',label:'Attempt 1',botId:'worker',attempt:1,active:true}
        ],
        edges:[{from:'ticket:ticket',to:'assignment:asgn',relation:'owns_attempt'}]
      } as never;
      throw Error('Unexpected command: '+command);
    });
    (window as unknown as {saGraphCalls:()=>string[]}).saGraphCalls=()=>seen;
  });
  const host=page.locator('#sa-run-graph-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  const graph=host.getByRole('region',{name:'Native run graph'});
  await expect(graph).toContainText('Graph not loaded.');
  await graph.getByRole('button',{name:'Inspect graph'}).click();
  await expect(graph).toContainText('Ticket owns attempt');
  await expect(graph).toContainText('Active native assignment');
  await expect(graph.getByRole('listitem')).toHaveCount(2);
  await graph.getByRole('button',{name:'Inspect graph attempt 1'}).click();
  const inspector=host.getByRole('region',{name:'Native assignment inspector'});
  await expect(inspector).toContainText('session');
  await expect(inspector).toContainText('Attempt 1');
  await expect(inspector).toBeFocused();
  await inspector.getByRole('button',{name:'Close attempt'}).click();
  await expect(inspector).toHaveCount(0);
  await expect(host.locator('[data-sa-accept]')).toHaveCount(0);
  const calls=await page.evaluate(()=>
    (window as unknown as {saGraphCalls:()=>string[]}).saGraphCalls());
  expect(calls.filter(c=>c==='superagent_graph')).toHaveLength(1);
  expect(calls.some(c=>/submit|dispatch|accept|reserve/.test(c))).toBe(false);
});


test('Team effective policy view is native-backed and never grants or launches tasks', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-effective-test'; document.body.appendChild(host);
    const seen:string[]=[];
    (window as unknown as {effectiveCalls:()=>string[]}).effectiveCalls=()=>seen;
    const config={id:'team',name:'Effective policy',coordinatorBotId:'lead',
      workerBotId:'worker',enabled:false,policyRevision:3,ticketCount:0};
    mountSuperagent(host,async(command) => {
      seen.push(command);
      if(command==='bots_list') return [
        {id:'lead',name:'Lead',paused:false},
        {id:'worker',name:'Worker',paused:false},
        {id:'second',name:'Second',paused:false}
      ] as never;
      if(command==='superagent_list') return [config] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_roster') return {
        orchestrationId:'team',policyRevision:3,coordinatorBotId:'lead',
        workerBotIds:['worker','second'],reviewerBotIds:[],executionReady:false
      } as never;
      if(command==='superagent_effective_team') return {
        orchestrationId:'team',policyRevision:3,enabled:false,executionReady:false,
        grantsIssued:false,manualAdmissionBlockers:[
          'Team is disabled; explicitly enable it before admitting a Demand.',
          'Multi-role Team requires native scoped role admission; manual dispatch is unavailable.'
        ],members:[
          {role:'coordinator',position:0,botId:'lead',name:'Lead',provider:'codex',
            model:'gpt-main',mode:'plan',reasoningEffort:'medium',projectId:'p',paused:false,skills:[],configurationIssues:[]},
          {role:'worker',position:0,botId:'worker',name:'Worker',provider:'claude',
            model:'sonnet',mode:'plan',reasoningEffort:'high',projectId:'p',paused:false,skills:[
              {id:'quality',libraryStatus:'installed',safetyStatus:'clean',
                harnessEligibility:'not_verified',invocationEvidence:'not_verified'},
              {id:'removed',libraryStatus:'missing',safetyStatus:'unknown',
                harnessEligibility:'not_verified',invocationEvidence:'not_verified'}
            ],configurationIssues:[
              'Assigned Skill removed is missing: install or unassign it in Skills Manager.'
            ]}
        ]
      } as never;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-effective-test');
  const roles=host.getByRole('region',{name:'Configured Team roles'});
  await roles.getByRole('button',{name:'Load native members'}).click();
  await roles.getByRole('button',{name:'Inspect effective policy and Skills'}).click();
  const view=roles.getByRole('region',{name:'Native effective Team policy'});
  await expect(view).toContainText('Harness claude');
  await expect(view).toContainText('Model sonnet');
  await expect(view).toContainText('Reasoning effort high');
  await expect(view).toContainText('Skill quality · Library installed · Safety clean');
  await expect(view).toContainText('Skill removed · Library missing');
  await expect(view.getByText('Assigned Skill removed is missing: install or unassign it in Skills Manager.')).toBeVisible();
  await expect(view).toContainText('No local configuration issues detected. Harness/tool eligibility unverified.');
  await expect(view).toContainText('Harness eligibility not_verified');
  await expect(view).toContainText('Multi-role execution unavailable');
  await expect(view.getByRole('list',{name:'Native manual admission blockers'}))
    .toContainText('Multi-role Team requires native scoped role admission');
  await expect(view).toContainText('Native manual admission blocked');
  await expect(view.getByRole('list',{name:'Native manual admission blockers'}))
    .toContainText('Team is disabled; explicitly enable it before admitting a Demand.');
  await expect(view).toContainText('No task grant issued');
  const calls=await page.evaluate(()=>
    (window as unknown as {effectiveCalls:()=>string[]}).effectiveCalls());
  expect(calls.filter(c=>c==='superagent_effective_team')).toHaveLength(1);
  expect(calls.some(c=>/submit|dispatch|accept|reserve|enable|activate/.test(c))).toBe(false);
});

test('Team member Activity pages are native-backed, Team-scoped and open the original bot chat only after recheck', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-member-activity-test'; document.body.appendChild(host);
    const seen: {command:string;args?:Record<string,unknown>}[]=[];
    const routes: unknown[]=[];
    let staleChatLink = false;
    (window as unknown as {saRejectChatLink:()=>void}).saRejectChatLink = () => { staleChatLink = true; };
    (window as unknown as {saMemberActivityEvidence:()=>unknown}).saMemberActivityEvidence=
      ()=>({seen,routes});
    mountSuperagent(host, async(command,args) => {
      seen.push({command,args});
      if (command==='bots_list') return [
        {id:'lead',name:'Lead',paused:false},
        {id:'worker',name:'Worker',paused:false}
      ] as never;
      if (command==='superagent_list') return [
        {id:'team',name:'Scoped Team',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:false,policyRevision:3,ticketCount:2}
      ] as never;
      if (command==='superagent_tickets') return [] as never;
      if (command==='superagent_roster') return {
        orchestrationId:'team',policyRevision:3,coordinatorBotId:'lead',
        workerBotIds:['worker'],reviewerBotIds:[],executionReady:true
      } as never;
      if (command==='superagent_effective_team') return {
        orchestrationId:'team',policyRevision:3,enabled:false,executionReady:true,
        grantsIssued:false,manualAdmissionBlockers:[],members:[
          {role:'coordinator',position:0,botId:'lead',name:'Lead',
            provider:'codex',model:'model',mode:'plan',reasoningEffort:'',
            projectId:'lead-project',paused:false,skills:[],configurationIssues:[]},
          {role:'worker',position:0,botId:'worker',name:'Worker',
            provider:'codex',model:'model',mode:'plan',reasoningEffort:'',
            projectId:'worker-project',paused:false,skills:[],configurationIssues:[]}
        ]
      } as never;
      if (command==='superagent_member_activity_page') {
        if (args?.orchestrationId!=='team') throw Error('wrong Team requested');
        if (args?.botId==='lead') return {
          orchestrationId:'unrelated-team',botId:'lead',items:[{
            assignmentId:'foreign',ticketId:'foreign-ticket',orchestrationId:'unrelated-team',
            botId:'lead',sessionId:'foreign-session',role:'coordinator',attempt:1,
            assignmentState:'finished',ticketState:'completed',createdAt:800,updatedAt:800,
            hasResult:true,chatAvailable:true
          }],hasMore:false,nextCursor:null
        } as never;
        if (!args?.before) return {
          orchestrationId:'team',botId:'worker',items:[{
            assignmentId:'a-new',ticketId:'ticket-new',orchestrationId:'team',
            botId:'worker',sessionId:'missing-session',role:'worker',attempt:2,
            assignmentState:'finished',ticketState:'blocked',createdAt:500,updatedAt:501,
            hasResult:false,chatAvailable:false
          }],hasMore:true,nextCursor:{createdAt:500,assignmentId:'a-new'}
        } as never;
        return {
          orchestrationId:'team',botId:'worker',items:[{
            assignmentId:'a-old',ticketId:'ticket-old',orchestrationId:'team',
            botId:'worker',sessionId:'real-session',role:'worker',attempt:1,
            assignmentState:'finished',ticketState:'waiting_approval',createdAt:499,updatedAt:499,
            hasResult:true,chatAvailable:true
          }],hasMore:false,nextCursor:null
        } as never;
      }
      if (command==='superagent_member_assignment_chat_link') {
        if (args?.orchestrationId!=='team' || args?.ticketId!=='ticket-old' ||
            args?.assignmentId!=='a-old' || args?.botId!=='worker')
          throw Error('Chat link escaped native Team/Assignment scope');
        return {orchestrationId:'team',ticketId:'ticket-old',assignmentId:'a-old',
          botId:'worker',sessionId:staleChatLink?'remapped-session':'real-session'} as never;
      }
      if (command==='superagent_snapshot') {
        if (args?.ticketId!=='ticket-old') throw Error('wrong Ticket requested');
        return {
          ticket:{id:'ticket-old',orchestrationId:'team',objective:'Historical native demand',
            revision:2,state:'waiting_approval',evidence:'Worker evidence'},
          mandate:{actor:'operator',authorizationId:'manual',policyRevision:3,
            projectId:'worker-project',expiresAt:1800000000000,revoked:false,
            allowedTools:[],allowedSideEffects:false},
          assignments:[{id:'a-old',botId:'worker',sessionId:'real-session',attempt:1,
            state:'finished',createdAt:499,updatedAt:499}],activity:[],
        } as never;
      }
      if (command==='superagent_source_thread_links') return [] as never;
      if (command==='superagent_all_demands') {
        if ((args?.query as {orchestrationId?:string} | undefined)?.orchestrationId!=='team')
          throw Error('Historical Demand query crossed Team boundary');
        return [{id:'ticket-old',orchestrationId:'team',objective:'Historical native demand',
          state:'waiting_approval',revision:2,projectId:'worker-project',updatedAt:499}] as never;
      }
      throw Error('Unexpected command: '+command);
    }, async route => { routes.push(route); });
  });
  const host=page.locator('#sa-member-activity-test');
  const roles=host.getByRole('region',{name:'Configured Team roles'});
  await roles.getByRole('button',{name:'Load native members'}).click();
  await roles.getByRole('button',{name:'Inspect effective policy and Skills'}).click();
  await roles.getByRole('button',{name:'View activity for Worker'}).click();
  const activity=roles.getByRole('region',{name:'Team member activity'});
  await expect(activity).toContainText('ticket-new');
  await expect(activity).toContainText('Native chat unavailable');
  await expect(activity.getByRole('button',{name:'Open chat for assignment a-new'})).toHaveCount(0);
  // A historical row cannot navigate if native ownership/assignment proof is missing.
  await activity.getByRole('button',{name:'Open demand for assignment a-new'}).click();
  await expect(host).toContainText('wrong Ticket requested');
  await expect(host.getByRole('region',{name:'Ticket evidence'})).toHaveCount(0);
  await activity.getByRole('button',{name:'Older activity'}).click();
  await expect(activity).toContainText('ticket-old');
  await activity.getByRole('button',{name:'Open chat for assignment a-old'}).click();
  await expect.poll(async () => page.evaluate(() => {
    const evidence=(window as unknown as {saMemberActivityEvidence:()=>{
      routes:Array<{teamId:string;ticketId:string;assignmentId:string;botId:string;sessionId:string}>
    }}).saMemberActivityEvidence();
    return evidence.routes.length;
  })).toBe(1);
  const evidence=await page.evaluate(() =>
    (window as unknown as {saMemberActivityEvidence:()=>{
      seen:Array<{command:string;args?:Record<string,unknown>}>;routes:unknown[]
    }}).saMemberActivityEvidence());
  expect(evidence.routes[0]).toEqual({
    teamId:'team',ticketId:'ticket-old',assignmentId:'a-old',
    botId:'worker',sessionId:'real-session',
  });
  await page.evaluate(() =>
    (window as unknown as {saRejectChatLink:()=>void}).saRejectChatLink());
  await activity.getByRole('button',{name:'Open chat for assignment a-old'}).click();
  await expect(host).toContainText(
    'Native assignment chat identity changed; refresh Team activity before opening chat.');
  const blocked = await page.evaluate(() =>
    (window as unknown as {saMemberActivityEvidence:()=>{
      routes: unknown[]
    }}).saMemberActivityEvidence().routes.length);
  expect(blocked).toBe(1, 'Stale native links cannot dispatch chat navigation');
  // page.evaluate returns a snapshot: re-read the command log after the denied
  // second click so the assertion cannot accidentally inspect stale evidence.
  const refreshedEvidence = await page.evaluate(() =>
    (window as unknown as {saMemberActivityEvidence:()=>{
      seen:Array<{command:string;args?:Record<string,unknown>}>;
    }}).saMemberActivityEvidence());
  expect(refreshedEvidence.seen.filter(call=>call.command==='superagent_member_assignment_chat_link'))
    .toHaveLength(2);
  expect(evidence.seen.filter(call=>call.command==='superagent_member_activity_page'))
    .toHaveLength(2);
  expect(evidence.seen.some(call => /submit|dispatch|accept|grant|enable|reserve/.test(call.command)))
    .toBe(false);
  await roles.getByRole('button',{name:'View activity for Lead'}).click();
  await expect(host).toContainText('invalid or out-of-scope history');
  await expect(roles.getByRole('region',{name:'Team member activity'}))
    .not.toContainText('foreign-ticket');
  // An older Assignment outside the Team's latest 50 Tickets resolves to
  // the original canonical Demand and selects its native attempt inspector.
  await roles.getByRole('button',{name:'View activity for Worker'}).click();
  await roles.getByRole('region',{name:'Team member activity'})
    .getByRole('button',{name:'Older activity'}).click();
  await roles.getByRole('region',{name:'Team member activity'})
    .getByRole('button',{name:'Open demand for assignment a-old'}).click();
  await expect(host.getByRole('button',{name:'Demands / Runs'}))
    .toHaveAttribute('aria-current','page');
  await expect(host.getByRole('combobox',{name:'Filter Demands by Team'}))
    .toHaveValue('team');
  await expect(host.getByRole('region',{name:'Ticket evidence'}))
    .toContainText('ticket-old');
  await expect(host.getByRole('button',{name:'Inspect attempt 1'}))
    .toHaveAttribute('aria-expanded','true');
  const finalEvidence=await page.evaluate(() =>
    (window as unknown as {saMemberActivityEvidence:()=>{
      seen:Array<{command:string;args?:Record<string,unknown>}>;routes:unknown[]
    }}).saMemberActivityEvidence());
  expect(finalEvidence.seen.some(call=>call.command==='superagent_all_demands' &&
    (call.args?.query as {orchestrationId?:string} | undefined)?.orchestrationId==='team')).toBe(true);
  expect(finalEvidence.seen.some(call=>
    /submit|dispatch|accept|grant|enable|reserve|stop/.test(call.command))).toBe(false);
});

test('native Team directory counts configured roles and filters without creating tasks', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-directory-filter-test'; document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {teamDirectoryCalls:()=>string[]}).teamDirectoryCalls=()=>calls;
    const teams=[
      {id:'economy',name:'Economy',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:false,policyRevision:1,workerCount:1,reviewerCount:0},
      {id:'security',name:'Security critical',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:true,policyRevision:4,workerCount:3,reviewerCount:2},
      {id:'balanced',name:'Balanced',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:false,policyRevision:2,workerCount:2,reviewerCount:1},
    ];
    mountSuperagent(host,async(command) => {
      calls.push(command);
      if(command==='bots_list') return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return teams as never;
      if(command==='superagent_tickets') return [] as never;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-directory-filter-test');
  const directory=host.getByRole('complementary',{name:'Teams directory'});
  await expect(directory.locator('.sa-item')).toHaveCount(3);
  await expect(directory.locator('.sa-role-counts').nth(1))
    .toHaveText('1 Coordinator · 3 Workers · 2 Reviewers');
  const search=directory.getByRole('searchbox',{name:'Search Teams'});
  await search.fill('SECURITY');
  await expect(search).toBeFocused();
  await expect(directory.locator('.sa-item strong')).toHaveText('Security critical');
  await expect(directory).toContainText('1 of 3 native Teams displayed');
  await search.fill('');
  await directory.getByRole('combobox',{name:'Team status'}).selectOption('disabled');
  await expect(directory.locator('.sa-item strong')).toHaveText(['Economy','Balanced']);
  await directory.getByRole('combobox',{name:'Team status'}).selectOption('all');
  await expect(directory.locator('.sa-item')).toHaveCount(3);
  const calls=await page.evaluate(() =>
    (window as unknown as {teamDirectoryCalls:()=>string[]}).teamDirectoryCalls());
  expect(calls.every(command=>['bots_list','superagent_list','superagent_tickets'].includes(command))).toBe(true);
});

test('duplicate Team uses native atomic roster creation without copying source authority', async ({page}) => {
  test.setTimeout(90_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const workspace=page.getByRole('region',{name:'Orchestrations'});
  await workspace.getByRole('button',{name:'Teams',exact:true}).click();
  const form=workspace.locator('form[data-sa-form="create"]');
  await form.getByRole('textbox',{name:'Name',exact:true}).fill('Review Fixture');
  await form.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await form.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await form.locator('summary').click();
  await form.getByRole('group',{name:'Optional Reviewers'}).getByRole('checkbox',{name:/Ledger/}).check();
  await form.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-list .sa-item')).toHaveCount(1);
  await workspace.getByRole('button',{name:'Teams',exact:true}).click();
  await workspace.getByRole('region',{name:'Team overview'})
    .getByRole('button',{name:'Duplicate as new Team'}).click();
  const approval=workspace.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(approval).toContainText('Tickets, source subscriptions, permissions and approvals are not copied');
  await approval.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(workspace.locator('.sa-list .sa-item')).toHaveCount(2);
  await expect(workspace.locator('.sa-list')).toContainText('Review Fixture copy');
  await expect(workspace.getByRole('status').filter({hasText:'Disabled Team duplicated.'})).toBeVisible();
  await workspace.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await expect(workspace.getByRole('region',{name:'Selected orchestration'}))
    .toContainText('Review Fixture copy');
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeDisabled();
  await workspace.getByRole('button',{name:'Teams',exact:true}).click();
  await workspace.getByRole('button',{name:'Load native members'}).click();
  await expect(workspace.getByRole('region',{name:'Configured Team roles'})).toContainText('Ledger');
  await expect(workspace.locator('.sa-list .sa-item').filter({hasText:'Review Fixture copy'}))
    .toContainText('0 tasks · 0 running · 0 blocked');
});

test('Team duplication rejects a stale native revision without creating a preset', async ({page}) => {
  test.setTimeout(45_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-duplicate-stale';
    document.body.appendChild(host);
    const calls: Array<{command:string;args?:Record<string,unknown>}>=[];
    (window as unknown as {saDuplicateCalls:typeof calls}).saDuplicateCalls=calls;
    mountSuperagent(host,async (command,args) => {
      calls.push({command,args});
      if(command==='bots_list') return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list') return [{
        id:'source',name:'Existing Team',coordinatorBotId:'lead',
        workerBotId:'worker',enabled:false,policyRevision:7
      }] as never;
      if(command==='superagent_tickets') return [] as never;
      if(command==='superagent_duplicate') {
        if(args?.sourceId!=='source'||args?.expectedRevision!==7||
            args?.name!=='Existing Team copy') throw Error('Incorrect native CAS input');
        throw Error('Source Team policy revision changed; reload before duplicating.');
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-duplicate-stale');
  await host.getByRole('button',{name:'Duplicate as new Team'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('alert')).toContainText('Source Team policy revision changed');
  await expect(host.locator('.sa-list .sa-item')).toHaveCount(1);
  const calls=await page.evaluate(() =>
    (window as unknown as {saDuplicateCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saDuplicateCalls);
  expect(calls.filter(item=>item.command==='superagent_duplicate')).toHaveLength(1);
  expect(calls.every(item=>item.command!=='superagent_create'&&
    item.command!=='superagent_roster_update')).toBe(true);
});

test('native Team enable blocker is visible and cannot be bypassed by the workspace', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-manual-enable-denied';
    document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {saEnableDenialCalls:string[]}).saEnableDenialCalls=calls;
    mountSuperagent(host, async command => {
      calls.push(command);
      if (command==='bots_list') return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if (command==='superagent_list') return [{
        id:'team',name:'Paused policy Team',coordinatorBotId:'coordinator',
        workerBotId:'worker',enabled:false,policyRevision:9,
        workerCount:1,reviewerCount:0,
        manualEnableBlocker:'Bot permission mode is not supported by PandaCode.',
      }] as never;
      if (command==='superagent_tickets') return [] as never;
      throw Error('Unexpected command: '+command);
    });
  });
  const host=page.locator('#sa-manual-enable-denied');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const enable=host.getByRole('button',{name:'Enable manual'});
  await expect(enable).toBeDisabled();
  await expect(enable).toHaveAttribute('aria-describedby','sa-native-enable-blocker');
  await expect(host.getByRole('status').filter({hasText:'Manual enable blocked:'}))
    .toContainText('Bot permission mode is not supported by PandaCode.');
  expect(await page.evaluate(() =>
    (window as unknown as {saEnableDenialCalls:string[]}).saEnableDenialCalls))
    .toEqual(['bots_list','superagent_list','superagent_tickets']);
});

test('new Team persists optional reviewers as one disabled role configuration', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const bar=page.locator('.sidebar-bots-bar');
  await expect(bar).toBeVisible({timeout:30_000});
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const workspace=page.getByRole('region',{name:'Orchestrations'});
  // Teams is the default workspace surface; await native form rendering.
  const form=workspace.locator('form[data-sa-form="create"]');
  await expect(form).toBeVisible();
  await form.getByRole('textbox',{name:'Name',exact:true}).fill('Security Review Team');
  await form.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await form.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await form.locator('summary').click();
  await form.getByRole('group',{name:'Optional Reviewers'}).getByRole('checkbox',{name:/Ledger/}).check();
  await form.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(workspace.locator('.sa-role-counts')).toContainText('1 Coordinator · 1 Workers · 1 Reviewers');
  await workspace.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await expect(workspace.getByRole('button',{name:'Enable manual'})).toBeDisabled();
  await workspace.getByRole('button',{name:'Teams',exact:true}).click();
  await workspace.getByRole('button',{name:'Load native members'}).click();
  await expect(workspace.getByRole('region',{name:'Configured Team roles'})).toContainText('Ledger');
});


test('desktop native Ticket activity pages are keyset-bound, filterable and never launch work', async ({page}) => {
  // Two explicit keyset fetches and full inspector rendering exceed the generic
  // 20s browser-test budget on loaded CI runners. Keep every assertion intact.
  test.setTimeout(45_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div'); host.id='sa-older-audit'; document.body.appendChild(host);
    const seen:Array<{command:string;beforeId?:number}>=[];
    (window as unknown as {saOlderAuditCalls:typeof seen}).saOlderAuditCalls=seen;
    mountSuperagent(host, async (command,args) => {
      seen.push({command,beforeId:Number(args?.beforeId)});
      if(command==='bots_list')return [{id:'router',name:'Router',paused:false},
        {id:'worker',name:'Worker',paused:false}] as never;
      if(command==='superagent_list')return [{id:'team',name:'History Team',
        coordinatorBotId:'router',workerBotId:'worker',enabled:true,policyRevision:1}] as never;
      if(command==='superagent_tickets')return [{id:'ticket',orchestrationId:'team',
        objective:'Audit history',state:'running',revision:2,evidence:null}] as never;
      if(command==='superagent_source_thread_links')return [] as never;
      if(command==='superagent_snapshot')return {
        ticket:{id:'ticket',orchestrationId:'team',objective:'Audit history',
          state:'running',revision:2,evidence:null},
        mandate:{actor:'operator',authorizationId:'manual',policyRevision:1,
          projectId:'project',expiresAt:1800000000000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],
        activity:Array.from({length:50},(_,i)=>({id:105-i,kind:'native',
          detail:'audit-'+(105-i),at:1800000000000-i})),
        finalizationGate:'not_pending',unresolvedExternalEffects:0
      } as never;
      if(command==='superagent_activity_page') {
        const before=Number(args?.beforeId);
        if(before===56)return {ticketId:'ticket',
          items:Array.from({length:50},(_,i)=>({id:55-i,kind:'native',
            detail:'audit-'+(55-i),at:1800000000000-50-i})),
          hasMore:true,nextCursor:6} as never;
        if(before===6)return {ticketId:'ticket',
          items:Array.from({length:5},(_,i)=>({id:5-i,kind:'native',
            detail:'audit-'+(5-i),at:1800000000000-100-i})),
          hasMore:false,nextCursor:null} as never;
        throw Error('Unexpected keyset cursor: '+before);
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-older-audit');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  await expect(host.locator('.sa-audit-list li')).toHaveCount(50);
  await host.getByRole('button',{name:'Load older native events'}).click();
  await expect(host.locator('.sa-audit-list li')).toHaveCount(100);
  await host.getByRole('button',{name:'Load older native events'}).click();
  await expect(host.locator('.sa-audit-list li')).toHaveCount(105);
  await expect(host.getByRole('button',{name:'Load older native events'})).toHaveCount(0);
  await host.getByRole('button',{name:'Json'}).click();
  await expect(host.locator('.sa-audit-json')).toContainText('audit-1');
  await host.getByRole('searchbox',{name:'Filter native activity'}).fill('audit-1');
  await expect(host.locator('.sa-activity-source')).toContainText('matching of 105 loaded native events');
  const calls=await page.evaluate(()=>
    (window as unknown as {saOlderAuditCalls:Array<{command:string;beforeId?:number}>}).saOlderAuditCalls);
  expect(calls.filter(item=>item.command==='superagent_activity_page').map(item=>item.beforeId))
    .toEqual([56,6]);
  expect(calls.some(item=>/submit|dispatch|accept|stop|grant|reserve/.test(item.command))).toBe(false);
});


test('Demand inspector preserves native admitted Team identity across revisions without exposing paths', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-admission-history-test'; document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {admissionHistoryCalls:()=>string[]}).admissionHistoryCalls=()=>calls;
    const ticket={id:'frozen-ticket',orchestrationId:'team',objective:'Historical policy',
      state:'completed',revision:5,evidence:'recorded'};
    mountSuperagent(host,async(command) => {
      calls.push(command);
      if(command==='bots_list') return [
        {id:'lead',name:'Renamed Lead',paused:false},
        {id:'new-worker',name:'Replacement Worker',paused:false},
      ] as never;
      if(command==='superagent_list') return [{
        id:'team',name:'Updated Team',coordinatorBotId:'lead',workerBotId:'new-worker',
        enabled:false,policyRevision:5,
      }] as never;
      if(command==='superagent_tickets') return [ticket] as never;
      if(command==='superagent_source_thread_links') return [] as never;
      if(command==='superagent_snapshot') return {
        ticket,
        mandate:{actor:'operator',authorizationId:'frozen',policyRevision:2,
          projectId:'project-old',expiresAt:1800000000000,revoked:true,
          allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],
        currentTeamPolicyRevision:5,teamPolicyChangedSinceAdmission:true,
        admittedProfile:{policyRevision:2,capturedAt:1800000000000,members:[
          {role:'coordinator',position:0,botId:'lead',projectId:'project-old',
            provider:'codex',model:'original-lead-model',mode:'plan',reasoningEffort:'high',
            assignedSkillIds:[],projectPath:'/private/hidden-coordinator'},
          {role:'worker',position:0,botId:'former-worker',projectId:'project-old',
            provider:'claude',model:'original-worker-model',mode:'plan',reasoningEffort:'medium',
            assignedSkillIds:['verification'],projectPath:'/private/hidden-worker'},
        ]},
        finalizationGate:'not_pending',unresolvedExternalEffects:0,
      } as never;
      throw Error('Unexpected native call: '+command);
    });
  });
  const host=page.locator('#sa-admission-history-test');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await host.getByRole('button',{name:'View evidence'}).click();
  const pane=host.getByRole('region',{name:'Admitted Team configuration'});
  await expect(pane).toContainText('Admitted policy revision 2');
  await expect(pane).toContainText('Current Team policy revision 5');
  await expect(pane).toContainText('Team policy changed after admission');
  await expect(pane).toContainText('Bot ID former-worker');
  await expect(pane).toContainText('Model original-worker-model');
  await expect(pane).toContainText('Assigned Skills (not invocation evidence): verification');
  await expect(pane).not.toContainText('Replacement Worker');
  await expect(host).not.toContainText('/private/hidden-worker');
  await expect(host).not.toContainText('/private/hidden-coordinator');
  const calls=await page.evaluate(() =>
    (window as unknown as {admissionHistoryCalls:()=>string[]}).admissionHistoryCalls());
  expect(calls.some(command=>/submit|dispatch|accept|effect|grant|reserve/.test(command))).toBe(false);
});


test('cross-Team native review rejection is evidence-bound, revision-guarded and never a Council pass', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-global-rejection-test';document.body.appendChild(host);
    type Mode='stale-first'|'stale-confirm'|'native-denied'|'valid';
    type Trace={command:string;args?:Record<string,unknown>};
    const trace:Trace[]=[];
    let mode:Mode='stale-first',reads=0;
    let decision={id:'human-review',orchestrationId:'team-b',objective:'Review release evidence',
      state:'waiting_approval',revision:7,updatedAt:2000,
      evidence:'Evidence <img src=x onerror=alert(1)> not executable'};
    (window as unknown as {saGlobalReviewTest:{trace:Trace[];setMode:(next:Mode)=>void}})
      .saGlobalReviewTest={trace,setMode:next=>{mode=next;reads=0;}};
    mountSuperagent(host,async(command,args)=>{
      trace.push({command,args});
      if(command==='bots_list')return [
        {id:'lead',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:true,policyRevision:2},
        {id:'team-b',name:'Performance',coordinatorBotId:'lead',workerBotId:'worker',
          enabled:true,policyRevision:3},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_review_inbox')return [
        ...(decision.state==='waiting_approval'?[{...decision}]:[]),
        {id:'council-review',orchestrationId:'team-a',objective:'Council pending',
          state:'waiting_review',revision:4,updatedAt:1000},
      ] as never;
      if(command==='superagent_snapshot'){
        if(args?.ticketId!=='human-review')throw Error('Foreign Ticket read');
        reads++;
        const stale=mode==='stale-first'||(mode==='stale-confirm'&&reads>=2);
        return {ticket:{...decision,revision:decision.revision+(stale?1:0)},
          mandate:{actor:'operator',authorizationId:'manual',policyRevision:3,projectId:'project',
            expiresAt:1800000000000,revoked:false,allowedTools:[],allowedSideEffects:false},
          assignments:[],activity:[]} as never;
      }
      if(command==='superagent_reject'){
        if(mode==='native-denied')throw Error('Native revision CAS denied');
        if(args?.ticketId!==decision.id||args?.expectedRevision!==decision.revision||
            args?.reason!=='Incorrect evidence')throw Error('Unexpected native rejection');
        decision={...decision,state:'blocked',revision:decision.revision+1};
        return {...decision} as never;
      }
      throw Error('Unexpected native operation: '+command);
    });
  });
  const host=page.locator('#sa-global-rejection-test');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(inbox.getByRole('button',{name:'Reject evidence'})).toHaveCount(1);
  await inbox.getByRole('button',{name:'Reject evidence'}).click();
  await expect(host.getByRole('alert')).toContainText('Native review decision changed');
  await expect(inbox.getByRole('form',{name:'Reject native Demand evidence'})).toHaveCount(0);
  await page.evaluate(()=>
    (window as unknown as {saGlobalReviewTest:{setMode:(next:string)=>void}})
      .saGlobalReviewTest.setMode('stale-confirm'));
  await inbox.getByRole('button',{name:'Reject evidence'}).click();
  const form=inbox.getByRole('form',{name:'Reject native Demand evidence'});
  await expect(form).toContainText('Performance');
  await expect(form).toContainText('Evidence <img src=x onerror=alert(1)> not executable');
  await expect(form.locator('img')).toHaveCount(0);
  await form.getByRole('textbox',{name:'Reason for rejecting evidence'}).fill('Incorrect evidence');
  await form.getByRole('button',{name:'Review rejection'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('evidence changed before confirmation');
  let calls=await page.evaluate(()=>
    (window as unknown as {saGlobalReviewTest:{trace:{command:string}[]}})
      .saGlobalReviewTest.trace);
  expect(calls.some(item=>item.command==='superagent_reject')).toBe(false);
  await form.getByRole('button',{name:'Cancel rejection'}).click();
  await page.evaluate(()=>
    (window as unknown as {saGlobalReviewTest:{setMode:(next:string)=>void}})
      .saGlobalReviewTest.setMode('native-denied'));
  await inbox.getByRole('button',{name:'Reject evidence'}).click();
  await form.getByRole('textbox',{name:'Reason for rejecting evidence'}).fill('Incorrect evidence');
  await form.getByRole('button',{name:'Review rejection'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('Native revision CAS denied');
  await expect(inbox.getByRole('button',{name:'Reject evidence'})).toHaveCount(1);
  await page.evaluate(()=>
    (window as unknown as {saGlobalReviewTest:{setMode:(next:string)=>void}})
      .saGlobalReviewTest.setMode('valid'));
  await inbox.getByRole('button',{name:'Reject evidence'}).click();
  await form.getByRole('textbox',{name:'Reason for rejecting evidence'}).fill('Incorrect evidence');
  await form.getByRole('button',{name:'Review rejection'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(inbox.getByRole('button',{name:'Reject evidence'})).toHaveCount(0);
  await expect(inbox).toContainText('Awaiting native Council (not approved)');
  await expect(host.getByRole('status').filter({hasText:'Native evidence rejection recorded'})).toBeVisible();
  calls=await page.evaluate(()=>
    (window as unknown as {saGlobalReviewTest:{trace:{command:string;args?:Record<string,unknown>}[]}})
      .saGlobalReviewTest.trace);
  expect(calls.filter(item=>item.command==='superagent_reject')).toHaveLength(2);
  expect(calls.filter(item=>item.command==='superagent_reject')[1]?.args)
    .toMatchObject({ticketId:'human-review',expectedRevision:7,reason:'Incorrect evidence'});
  expect(calls.some(item=>/superagent_accept|dispatch|effect_reserve|council_pass/.test(item.command)))
    .toBe(false);
});

test('cross-Team native human acceptance rechecks evidence and Council/effect gates', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-global-acceptance-test';
    document.body.appendChild(host);
    type Mode='stale-first'|'stale-confirm'|'council-required'|'effects-unresolved'|'native-denied'|'valid';
    type Trace={command:string;args?:Record<string,unknown>};
    const trace:Trace[]=[];
    let mode:Mode='stale-first', reads=0;
    let decision={id:'human-review',orchestrationId:'team-b',
      objective:'Review durable evidence',state:'waiting_approval',
      revision:7,updatedAt:2000,
      evidence:'Evidence <img src=x onerror=alert(1)> is inert'};
    (window as unknown as {saGlobalAcceptTest:{trace:Trace[];setMode:(value:Mode)=>void}})
      .saGlobalAcceptTest={trace,setMode:value=>{mode=value;reads=0;}};
    mountSuperagent(host,async(command,args)=>{
      trace.push({command,args});
      if(command==='bots_list')return [
        {id:'lead',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-a',name:'Economy',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:true,policyRevision:2},
        {id:'team-b',name:'Performance',coordinatorBotId:'lead',
          workerBotId:'worker',enabled:true,policyRevision:3},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_review_inbox')return [
        ...(decision.state==='waiting_approval'?[{...decision}]:[]),
        {id:'council-review',orchestrationId:'team-a',objective:'Council pending',
          state:'waiting_review',revision:4,updatedAt:1000},
      ] as never;
      if(command==='superagent_snapshot'){
        if(args?.ticketId!==decision.id)throw Error('Foreign Ticket read');
        reads++;
        const stale=mode==='stale-first'||(mode==='stale-confirm'&&reads>=2);
        return {ticket:{...decision,revision:decision.revision+(stale?1:0)},
          mandate:{actor:'operator',authorizationId:'manual',policyRevision:3,
            projectId:'project',expiresAt:1800000000000,revoked:false,
            allowedTools:[],allowedSideEffects:false},
          assignments:[],activity:[],
          finalizationGate:mode==='council-required'?'council_required':
            mode==='effects-unresolved'?'effects_unresolved':'native_recheck_required',
        } as never;
      }
      if(command==='superagent_accept'){
        if(mode==='native-denied')throw Error('Native mandate CAS denied');
        if(mode!=='valid'||args?.ticketId!==decision.id||
            args?.expectedRevision!==decision.revision)throw Error('Unexpected native acceptance');
        decision={...decision,state:'completed',revision:decision.revision+1};
        return {...decision} as never;
      }
      throw Error('Unexpected native operation: '+command);
    });
  });
  const host=page.locator('#sa-global-acceptance-test');
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  const inbox=host.getByRole('region',{name:'Cross-Team native review inbox'});
  await inbox.getByRole('button',{name:'Refresh review inbox'}).click();
  await expect(inbox.getByRole('button',{name:'Accept evidence'})).toHaveCount(1);
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  await expect(host.getByRole('alert')).toContainText('Native review decision changed');
  await expect(inbox.getByRole('form',{name:'Accept native Demand evidence'})).toHaveCount(0);
  const setMode=async(mode:string)=>page.evaluate(value=>
    (window as unknown as {saGlobalAcceptTest:{setMode:(value:string)=>void}})
      .saGlobalAcceptTest.setMode(value),mode);
  await setMode('council-required');
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  await expect(host.getByRole('alert')).toContainText('Native finalization gate blocks');
  await setMode('effects-unresolved');
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  await expect(host.getByRole('alert')).toContainText('Native finalization gate blocks');
  await setMode('stale-confirm');
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  const form=inbox.getByRole('form',{name:'Accept native Demand evidence'});
  await expect(form).toContainText('Performance');
  await expect(form).toContainText('Evidence <img src=x onerror=alert(1)> is inert');
  await expect(form.locator('img')).toHaveCount(0);
  await form.getByRole('button',{name:'Review acceptance'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('evidence changed before confirmation');
  let calls=await page.evaluate(()=>
    (window as unknown as {saGlobalAcceptTest:{trace:{command:string;args?:Record<string,unknown>}[]}}).saGlobalAcceptTest.trace);
  expect(calls.some(item=>item.command==='superagent_accept')).toBe(false);
  await form.getByRole('button',{name:'Cancel acceptance'}).click();
  await setMode('native-denied');
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  await form.getByRole('button',{name:'Review acceptance'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('alert')).toContainText('Native mandate CAS denied');
  await expect(inbox.getByRole('button',{name:'Accept evidence'})).toHaveCount(1);
  await setMode('valid');
  await inbox.getByRole('button',{name:'Accept evidence'}).click();
  await form.getByRole('button',{name:'Review acceptance'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(inbox.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
  await expect(inbox).toContainText('Awaiting native Council (not approved)');
  await expect(host.getByRole('status').filter({hasText:'Native human acceptance recorded'})).toBeVisible();
  calls=await page.evaluate(()=>
    (window as unknown as {saGlobalAcceptTest:{trace:{command:string;args?:Record<string,unknown>}[]}}).saGlobalAcceptTest.trace);
  expect(calls.filter(item=>item.command==='superagent_accept')).toHaveLength(2);
  expect(calls.filter(item=>item.command==='superagent_accept')[1]?.args)
    .toMatchObject({ticketId:'human-review',expectedRevision:7});
  expect(calls.some(item=>/superagent_reject|dispatch|effect_reserve|council_pass/.test(item.command)))
    .toBe(false);
});


test('Team directory uses durable native human and Council pending counts independently of paging', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-pending-counts';document.body.appendChild(host);
    const calls:string[]=[];(window as unknown as {saPendingCalls:string[]}).saPendingCalls=calls;
    mountSuperagent(host,async command=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'a',name:'Coordinator',paused:false},{id:'b',name:'Worker',paused:false}
      ] as never;
      if(command==='superagent_list')return [
        {id:'team-1',name:'Needs review',coordinatorBotId:'a',workerBotId:'b',enabled:false,
          policyRevision:1,ticketCount:260,runningCount:2,blockedCount:3,
          queuedCount:8,completedCount:40,needsAttentionCount:5,waitingApprovalCount:4,waitingReviewCount:2},
        {id:'team-2',name:'No reviews',coordinatorBotId:'a',workerBotId:'b',enabled:false,
          policyRevision:1,ticketCount:1,runningCount:0,blockedCount:0,
          queuedCount:0,completedCount:1,needsAttentionCount:0,waitingApprovalCount:0,waitingReviewCount:0},
      ] as never;
      if(command==='superagent_tickets')return [] as never;
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-pending-counts');
  await expect(host.locator('.sa-list .sa-review-counts')).toHaveText([
    '4 awaiting human approval · 2 awaiting Council (not approved)',
    '0 awaiting human approval · 0 awaiting Council (not approved)',
  ]);
  await expect(host.locator('.sa-list .sa-counts').first()).toContainText('260 tasks');
  await expect(host.locator('.sa-list .sa-attention-counts')).toHaveText([
    '5 needing attention','0 needing attention',
  ]);
  await expect(host.locator('.sa-list .sa-lifecycle-counts')).toHaveText([
    '8 queued · 40 completed','0 queued · 1 completed',
  ]);
  await host.getByRole('button',{name:'Review inbox',exact:true}).click();
  await expect(host.locator('.sa-list .sa-review-counts').first())
    .toContainText('2 awaiting Council (not approved)');
  const calls=await page.evaluate(()=> (window as unknown as {saPendingCalls:string[]}).saPendingCalls);
  expect(calls).toEqual(['bots_list','superagent_list','superagent_tickets']);
});


test('overlapping native Team refreshes cannot replace the newest directory with an older response',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-refresh-order';document.body.appendChild(host);
    const first={id:'first',name:'First Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',enabled:false,policyRevision:1};
    const second={...first,id:'second',name:'Second Team'};
    let listReads=0,releaseSlow:(()=>void)|undefined,completed=false;
    const mounted=mountSuperagent(host,async command=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list'){
        listReads++;
        if(listReads===2){
          await new Promise<void>(resolve=>{releaseSlow=resolve;});
          return [first] as never;
        }
        return (listReads>=3?[first,second]:[first]) as never;
      }
      if(command==='superagent_tickets')return [] as never;
      throw Error('Unexpected native command '+command);
    });
    (window as unknown as {saRefreshControl:{
      queue:()=>void;hasSlow:()=>boolean;release:()=>void;completed:()=>boolean;
    }}).saRefreshControl={
      queue:()=>{void Promise.all([mounted.refresh(),mounted.refresh()])
        .then(()=>{completed=true;});},
      hasSlow:()=>Boolean(releaseSlow),
      release:()=>{if(!releaseSlow)throw Error('Slow native read not started');releaseSlow();},
      completed:()=>completed,
    };
  });
  const host=page.locator('#sa-refresh-order');
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText('First Team');
  await page.evaluate(()=>(window as unknown as {saRefreshControl:{queue:()=>void}})
    .saRefreshControl.queue());
  await expect.poll(()=>page.evaluate(()=>
    (window as unknown as {saRefreshControl:{hasSlow:()=>boolean}})
      .saRefreshControl.hasSlow())).toBe(true);
  await page.evaluate(()=>(window as unknown as {saRefreshControl:{release:()=>void}})
    .saRefreshControl.release());
  await expect.poll(()=>page.evaluate(()=>
    (window as unknown as {saRefreshControl:{completed:()=>boolean}})
      .saRefreshControl.completed())).toBe(true);
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText(['First Team','Second Team']);
});

test('native Team creation clears stale directory filters after creation is acknowledged',async ({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-create-filter';document.body.appendChild(host);
    const teams:Array<Record<string,unknown>>=[];
    const commands:string[]=[];
    (window as unknown as {saFilterCommands:string[]}).saFilterCommands=commands;
    mountSuperagent(host,async(command,args)=>{
      commands.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [...teams] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'){
        const input=(args as {input:{name:string;coordinatorBotId:string;workerBotId:string}}).input;
        const created={...input,id:'new-team',enabled:false,policyRevision:1};
        teams.push(created);
        return created as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-create-filter');
  await expect(host.getByText('No orchestrations created.')).toBeVisible();
  await host.getByRole('searchbox',{name:'Search Teams'}).fill('not-this-team');
  await host.getByRole('combobox',{name:'Team status'}).selectOption('enabled');
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('New Team');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await host.getByRole('combobox',{name:'Worker',exact:true}).selectOption('worker');
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText('New Team');
  await expect(host.getByRole('searchbox',{name:'Search Teams'})).toHaveValue('');
  await expect(host.getByRole('combobox',{name:'Team status'})).toHaveValue('all');
  await expect(host.getByRole('button',{name:'Enable manual'})).toBeVisible();
  const calls=await page.evaluate(()=>
    (window as unknown as {saFilterCommands:string[]}).saFilterCommands);
  expect(calls.filter(command=>command==='superagent_create')).toHaveLength(1);
  expect(calls).not.toContain('superagent_submit');
});

test('new Team ACK retries a stale directory read without duplicating creation',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-create-ack';document.body.appendChild(host);
    let reads=0,writes=0;
    let created:Record<string,unknown>|null=null;
    (window as unknown as {saCreateAckCounts:()=>{reads:number;writes:number}}).saCreateAckCounts=
      ()=>({reads,writes});
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list'){
        reads++;
        return (created && reads>=3 ? [created] : []) as never;
      }
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'){
        writes++;
        const input=args?.input as {name:string;coordinatorBotId:string;workerBotId:string};
        created={...input,id:'verified-new-team',enabled:false,policyRevision:1};
        return created as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-create-ack');
  await expect(host.getByText('No orchestrations created.')).toBeVisible();
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('Verified Team');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await host.getByRole('combobox',{name:'Worker',exact:true}).selectOption('worker');
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.locator('.sa-list .sa-item strong')).toHaveText('Verified Team');
  await expect(host.getByRole('button',{name:'Demands / Runs'}))
    .toHaveAttribute('aria-current','page');
  const counts=await page.evaluate(()=>
    (window as unknown as {saCreateAckCounts:()=>{reads:number;writes:number}})
      .saCreateAckCounts());
  expect(counts).toEqual({reads:3,writes:1});
});

test('missing native Team after a successful ACK exposes an error without retrying the write',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-create-missing';document.body.appendChild(host);
    const commands:string[]=[];
    (window as unknown as {saMissingCommands:string[]}).saMissingCommands=commands;
    mountSuperagent(host,async(command,args)=>{
      commands.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list'||command==='superagent_tickets')return [] as never;
      if(command==='superagent_create'){
        const input=args?.input as {name:string;coordinatorBotId:string;workerBotId:string};
        return {...input,id:'persisted-but-unlisted',enabled:false,policyRevision:1} as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-create-missing');
  await expect(host.getByText('No orchestrations created.')).toBeVisible();
  await host.getByRole('textbox',{name:'Name',exact:true}).fill('Unlisted Team');
  await host.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await host.getByRole('combobox',{name:'Worker',exact:true}).selectOption('worker');
  await host.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.getByRole('alert'))
    .toContainText('its disabled identity could not be verified');
  await expect(host.locator('.sa-list .sa-item strong')).toHaveCount(0);
  const commands=await page.evaluate(()=>
    (window as unknown as {saMissingCommands:string[]}).saMissingCommands);
  expect(commands.filter(command=>command==='superagent_create')).toHaveLength(1);
  expect(commands.filter(command=>command==='superagent_list')).toHaveLength(3);
});

test('portable confirmation rejects a manifest changed after native preview',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-preview-bound';document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {saPreviewCommands:string[]}).saPreviewCommands=calls;
    mountSuperagent(host,async command=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [] as never;
      if(command==='superagent_manifest_preview')return {
        wouldCreate:['Reviewed Team'],unchanged:[],importedPresetsDisabled:true,
        reviewDigest:'a'.repeat(64),
      } as never;
      if(command==='superagent_manifest_import')throw Error('Unreviewed import reached native mutation.');
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-preview-bound');
  await host.getByRole('button',{name:'Portable configuration'}).click();
  const manifest=host.getByRole('textbox',{name:'Companion manifest JSON'});
  await manifest.fill('{"schemaVersion":1,"orchestrations":[]}');
  await host.getByRole('button',{name:'Preview import'}).click();
  await expect(host.getByRole('button',{name:'Confirm import'})).toBeVisible();
  await host.getByRole('button',{name:'Confirm import'}).click();
  await expect(host.getByRole('alertdialog',{name:'Confirm orchestration action'}))
    .toBeVisible();
  await manifest.fill('{"schemaVersion":1,"orchestrations":[{"name":"Changed"}]}');
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('alert')).toContainText('Manifest changed since preview');
  const calls=await page.evaluate(()=>
    (window as unknown as {saPreviewCommands:string[]}).saPreviewCommands);
  expect(calls.filter(command=>command==='superagent_manifest_preview')).toHaveLength(1);
  expect(calls).not.toContain('superagent_manifest_import');
});

test('portable preview invalidates a late response after editing the manifest', async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-preview-race';
    document.body.appendChild(host);
    const commands:string[]=[];
    let completePreview:()=>void=()=>{};
    const release=new Promise<void>(resolve=>{completePreview=resolve;});
    (window as unknown as {saFinishPreview:()=>void}).saFinishPreview=completePreview;
    (window as unknown as {saPreviewRaceCommands:string[]}).saPreviewRaceCommands=commands;
    mountSuperagent(host,async command=>{
      commands.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [] as never;
      if(command==='superagent_manifest_preview'){
        await release;
        return {wouldCreate:['Original Team'],unchanged:[],importedPresetsDisabled:true,
          reviewDigest:'a'.repeat(64)} as never;
      }
      if(command==='superagent_manifest_import')throw Error('Unreviewed manifest reached native import.');
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-preview-race');
  await host.getByRole('button',{name:'Portable configuration'}).click();
  const manifest=host.getByRole('textbox',{name:'Companion manifest JSON'});
  await manifest.fill('{"schemaVersion":1,"orchestrations":[]}');
  await host.getByRole('button',{name:'Preview import'}).click();
  await manifest.fill('{"schemaVersion":1,"orchestrations":[{"name":"Different"}]}');
  await page.evaluate(()=>(window as unknown as {saFinishPreview:()=>void}).saFinishPreview());
  await expect(host.getByRole('alert')).toContainText('Manifest changed during preview');
  await expect(host.getByRole('button',{name:'Confirm import'})).toHaveCount(0);
  const commands=await page.evaluate(()=>
    (window as unknown as {saPreviewRaceCommands:string[]}).saPreviewRaceCommands);
  expect(commands.filter(command=>command==='superagent_manifest_preview')).toHaveLength(1);
  expect(commands).not.toContain('superagent_manifest_import');
});

test('confirmed portable Team import forwards the current native-reviewed plan digest',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-reviewed-import';document.body.appendChild(host);
    const imports:Array<{document:unknown;confirmed:unknown;reviewDigest:unknown}>=[];
    (window as unknown as {saReviewedImports:typeof imports}).saReviewedImports=imports;
    mountSuperagent(host,async(command,args)=>{
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [] as never;
      if(command==='superagent_manifest_preview')return {
        wouldCreate:['Reviewed Team'],unchanged:[],importedPresetsDisabled:true,
        reviewDigest:'f'.repeat(64),
      } as never;
      if(command==='superagent_manifest_import'){
        imports.push({document:args?.document,confirmed:args?.confirmed,
          reviewDigest:args?.reviewDigest});
        if(args?.reviewDigest!=='f'.repeat(64))throw Error('Native review identity missing.');
        return {created:['Reviewed Team'],unchanged:[]} as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host=page.locator('#sa-reviewed-import');
  await host.getByRole('button',{name:'Portable configuration'}).click();
  const manifest=host.getByRole('textbox',{name:'Companion manifest JSON'});
  await manifest.fill('{"schemaVersion":1,"orchestrations":[]}');
  await host.getByRole('button',{name:'Preview import'}).click();
  await expect(host.getByRole('button',{name:'Confirm import'})).toBeVisible();
  await host.getByRole('button',{name:'Confirm import'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm'}).click();
  await expect(host.getByRole('status').filter({hasText:'1 presets imported; 0 unchanged.'}))
    .toBeVisible();
  const received=await page.evaluate(()=>
    (window as unknown as {saReviewedImports:unknown[]}).saReviewedImports);
  expect(received).toEqual([{
    document:{schemaVersion:1,orchestrations:[]},
    confirmed:true,reviewDigest:'f'.repeat(64),
  }]);
});


test('Team attention badge opens a native pre-paginated four-state Demand scope without dispatch', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-attention-nav';
    document.body.appendChild(host);
    const calls: Array<{command:string;args?:Record<string,unknown>}> = [];
    (window as unknown as {saAttentionCalls:typeof calls}).saAttentionCalls = calls;
    mountSuperagent(host, async (command,args) => {
      calls.push({command,args});
      if (command === 'bots_list') return [
        {id:'co',name:'Coordinator',paused:false},{id:'wo',name:'Worker',paused:false},
      ] as never;
      if (command === 'superagent_list') return [
        {id:'attention-team',name:'Repair Team',coordinatorBotId:'co',workerBotId:'wo',
          enabled:false,policyRevision:1,needsAttentionCount:4},
      ] as never;
      if (command === 'superagent_tickets') return [] as never;
      if (command === 'superagent_all_demands') {
        const query = args?.query as {orchestrationId?:string;stateFilter?:string} | undefined;
        if (query?.orchestrationId !== 'attention-team' ||
            query.stateFilter !== 'needs_attention') {
          throw Error('Unscoped native attention Demand request');
        }
        return ['blocked','failed','interrupted','inconclusive'].map((state,index) => ({
          id:'attention-'+index,orchestrationId:'attention-team',
          objective:'Repair '+state,state,revision:1,updatedAt:4000-index,
          projectId:'project',
        })) as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host = page.locator('#sa-attention-nav');
  await host.getByRole('button',{name:'Show Demands needing attention for Repair Team'}).click();
  const runs = host.getByRole('region',{name:'All Demands across Teams'});
  await expect(host.getByRole('button',{name:'Demands / Runs'})).toHaveAttribute('aria-current','page');
  await expect(runs.getByRole('combobox',{name:'Filter Demands by Team'})).toHaveValue('attention-team');
  await expect(runs.getByRole('combobox',{name:'Filter Demands by State'})).toHaveValue('needs_attention');
  await expect(runs).toContainText('4 native Demands loaded.');
  for (const state of ['blocked','failed','interrupted','inconclusive']) {
    await expect(runs).toContainText('Repair '+state);
  }
  const calls = await page.evaluate(() =>
    (window as unknown as {saAttentionCalls:Array<{command:string;args?:Record<string,unknown>}>})
      .saAttentionCalls);
  expect(calls.filter(item=>item.command==='superagent_all_demands')).toEqual([
    {command:'superagent_all_demands',
      args:{query:{limit:200,orchestrationId:'attention-team',stateFilter:'needs_attention'}}},
  ]);
  expect(calls.some(item=>/submit|dispatch|accept|grant|reserve|effect/.test(item.command)))
    .toBe(false);
});

test('Team directory activation uses native CAS without selecting or executing other Teams', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host = document.createElement('div');
    host.id = 'sa-team-directory-activation';
    document.body.appendChild(host);
    const teams = [
      {id:'team-a',name:'Routine Team',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:false,policyRevision:1,workerCount:1,reviewerCount:0},
      {id:'team-b',name:'Build Team',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:false,policyRevision:4,workerCount:1,reviewerCount:0},
      {id:'team-c',name:'Review Team',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:false,policyRevision:2,workerCount:1,reviewerCount:1,
        manualEnableBlocker:'Native Council admission not available'},
    ];
    const calls:Array<{command:string;args?:Record<string,unknown>}> = [];
    let rejectNext = false;
    (window as unknown as {
      saDirectoryActivationCalls:typeof calls;
      saDirectoryActivationReject:()=>void;
    }).saDirectoryActivationCalls = calls;
    (window as unknown as {saDirectoryActivationReject:()=>void})
      .saDirectoryActivationReject = () => { rejectNext = true; };
    mountSuperagent(host, async(command,args) => {
      calls.push({command,args});
      if(command === 'bots_list')return [
        {id:'lead',name:'Lead',paused:false},{id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command === 'superagent_list')return teams.map(team=>({...team})) as never;
      if(command === 'superagent_tickets')return [] as never;
      if(command === 'superagent_set_enabled'){
        if(rejectNext){rejectNext=false;throw Error('Native policy revision conflict');}
        const team = teams.find(item=>item.id === args?.orchestrationId);
        if(!team || args?.expectedRevision !== team.policyRevision ||
            args?.enabled === team.enabled)throw Error('Native policy revision conflict');
        team.enabled = args?.enabled === true;
        team.policyRevision++;
        return {...team} as never;
      }
      throw Error('Unexpected native command '+command);
    });
  });
  const host = page.locator('#sa-team-directory-activation');
  const enable = () => host.getByRole('button',{
    name:'Activate Build Team from Teams directory',
  });
  const disable = () => host.getByRole('button',{
    name:'Deactivate Build Team from Teams directory',
  });
  await expect(host.getByRole('button',{
    name:'Activate Review Team from Teams directory',
  })).toBeDisabled();
  await enable().click();
  await expect(host.getByRole('alertdialog')).toContainText('grants no tools or automatic tasks');
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(disable()).toBeVisible();
  // The Teams workspace renders the selected Team as its overview;
  // 'Selected orchestration' is the separate Runs inspector.
  await expect(host.getByRole('region',{name:'Team overview'}))
    .toContainText('Routine Team');
  await disable().click();
  await expect(host.getByRole('alertdialog')).toContainText('Already applied external changes cannot be undone');
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(enable()).toBeVisible();
  await expect(host.getByRole('status').filter({hasText:'Native mandates were revoked'}))
    .toBeVisible();
  await enable().click();
  await page.evaluate(() => (window as unknown as {
    saDirectoryActivationReject:()=>void;
  }).saDirectoryActivationReject());
  await host.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('alert')).toContainText('Native policy revision conflict');
  await expect(enable()).toBeVisible();
  const calls=await page.evaluate(() => (window as unknown as {
    saDirectoryActivationCalls:Array<{command:string;args?:Record<string,unknown>}>;
  }).saDirectoryActivationCalls);
  expect(calls.filter(item=>item.command==='superagent_set_enabled').map(item=>item.args))
    .toEqual([
      {orchestrationId:'team-b',expectedRevision:4,enabled:true},
      {orchestrationId:'team-b',expectedRevision:5,enabled:false},
      {orchestrationId:'team-b',expectedRevision:6,enabled:true},
    ]);
  expect(calls.some(item=>/superagent_(submit|preflight|grant|dispatch|retry|stop|accept)/.test(item.command)))
    .toBe(false);
});

test('archive and restore use real native Demand commands, CAS and separate visibility views',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-visibility-fixture';document.body.appendChild(host);
    const calls:string[]=[];
    (window as unknown as {saVisibilityCalls:string[]}).saVisibilityCalls=calls;
    let archived=false,revision=3;
    const team={id:'archive-team',name:'Archive Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',enabled:false,policyRevision:1,workerCount:1,reviewerCount:0};
    const ticket=()=>({id:'demand-archive',orchestrationId:team.id,
      objective:'Keep durable history',state:'stopped',revision,evidence:null});
    mountSuperagent(host,async(command,args)=>{
      calls.push(command);
      if(command==='bots_list')return [
        {id:'coordinator',name:'Coordinator',paused:false},
        {id:'worker',name:'Worker',paused:false},
      ] as never;
      if(command==='superagent_list')return [team] as never;
      if(command==='superagent_tickets')return (archived?[]:[ticket()]) as never;
      if(command==='superagent_all_demands'){
        const query=args?.query as {archived?:boolean} | undefined;
        return (Boolean(query?.archived)===archived ? [{...ticket(),archived,
          updatedAt:1000+revision,projectId:'project-1'}] : []) as never;
      }
      if(command==='superagent_snapshot')return {
        ticket:ticket(),mandate:{actor:'local-user',authorizationId:'native-test',
          policyRevision:1,projectId:'project-1',expiresAt:9999999999999,
          revoked:true,allowedTools:[],allowedSideEffects:false},
        assignments:[],activity:[],
      } as never;
      if(command==='superagent_archive_demand'||command==='superagent_restore_demand'){
        if(args?.ticketId!=='demand-archive'||args?.expectedRevision!==revision)
          throw Error('Stale or cross-Demand visibility request');
        if((command==='superagent_archive_demand')===archived)
          throw Error('Unexpected visibility transition');
        archived=command==='superagent_archive_demand';revision++;
        return ticket() as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#sa-visibility-fixture');
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  const runs=host.getByRole('region',{name:'All Demands across Teams'});
  await runs.getByRole('button',{name:'Refresh all Teams'}).click();
  await expect(runs.getByRole('button',{name:'Delete Demand'})).toBeVisible();
  await runs.getByRole('button',{name:'Delete Demand'}).click();
  await expect(host.getByRole('alertdialog',{name:'Confirm orchestration action'}))
    .toContainText('durable Ticket, native chat, receipts and audit are retained');
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(runs.getByRole('button',{name:'Delete Demand'})).toHaveCount(0);
  await runs.getByRole('combobox',{name:'Filter Demands by Visibility'}).selectOption('archived');
  await expect(runs.getByRole('button',{name:'Restore Demand'})).toBeVisible();
  await runs.getByRole('button',{name:'Restore Demand'}).click();
  await host.getByRole('alertdialog').getByRole('button',{name:'Confirm'}).click();
  await expect(runs.getByRole('button',{name:'Restore Demand'})).toHaveCount(0);
  await runs.getByRole('combobox',{name:'Filter Demands by Visibility'}).selectOption('active');
  await expect(runs.getByRole('button',{name:'Delete Demand'})).toBeVisible();
  const calls=await page.evaluate(()=>
    (window as unknown as {saVisibilityCalls:string[]}).saVisibilityCalls);
  expect(calls.filter(name=>name==='superagent_archive_demand')).toHaveLength(1);
  expect(calls.filter(name=>name==='superagent_restore_demand')).toHaveLength(1);
  expect(calls).not.toContain('superagent_submit');
});
