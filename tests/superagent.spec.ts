import { test, expect, type Page, type Locator } from '@playwright/test';

// This suite loads the complete PandaCode preview, including cold Vite modules.
// The default 20-second test budget can expire before the existing 30-second
// native-readiness assertion completes. Keep all functional assertions intact.
test.describe.configure({ timeout: 60_000 });

/**
 * The v2 SuperAgent workspace has no manual-preset or manual-dispatch UI.
 * These tests exercise the new Team -> Demand -> native Run journey, with a
 * strict preview-only native contract. Real provider delivery is a separate
 * native integration acceptance gate and is never asserted by this fixture.
 */

async function openWorkspace(page: Page): Promise<Locator> {
  await page.evaluate(async () => { await import('/src/preview-superagent.ts'); });
  const bar = page.locator('.sidebar-bots-bar');
  if (await bar.getAttribute('aria-pressed') !== 'true') await bar.click();
  const sidebar = page.getByRole('region', {name:'Bots', exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button', {name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem', {name:'Orchestrations'}).click();
  const host = page.locator('#superagent-workspace #superagent-host');
  await expect(page.getByRole('region', {name:'Orchestrations', exact:true})).toBeVisible();
  await expect(page.getByRole('dialog', {name:'Orchestrations', exact:true})).toHaveCount(0);
  await expect(host).toHaveAttribute('aria-busy', 'false', {timeout:30_000});
  return host;
}

async function completeWizard(host: Locator, name: string, mode: 'approval'|'full_auto' = 'approval') {
  await host.getByRole('button', {name:'New Team', exact:true}).click();
  const builder=host.getByRole('region', {name:'Guided Team Builder'});
  await expect(builder).toContainText('Step 1 of 5');
  await builder.getByRole('textbox', {name:'Team name'}).fill(name);
  await builder.getByRole('button', {name:'Next'}).click();
  await builder.getByRole('combobox', {name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.getByRole('combobox', {name:'Primary Worker'}).selectOption({label:'Writer'});
  await builder.getByRole('button', {name:'Next'}).click();
  await expect(builder).toContainText('Skill presence alone does not prove');
  await builder.getByRole('button', {name:'Next'}).click();
  await builder.getByRole('radio', {name:mode==='full_auto' ? /Full Auto/ : /With Approval/}).check();
  return builder;
}

async function openTeam(host: Locator, name: string): Promise<void> {
  await host.getByRole('button', {name:'Teams', exact:true}).click();
  const row=host.locator('.sa-teams-panel .sa-item').filter({hasText:name});
  await expect(row).toBeVisible();
  await row.getByRole('button', {name:'Details'}).click();
  await expect(host.locator('.sa-subtabs').getByRole('button', {name:'Execution and review'})).toBeVisible();
}

test('Orchestrations keeps the reference page and five-step wizard layout', async ({page}) => {
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  await expect(host.locator('.sa-pagehead h1')).toHaveText('Orchestrations');
  await expect(host.locator('.sa-navigation')).toBeVisible();
  await expect(host.locator('.sa-navigation').getByRole('button',{name:'Teams'})).toBeVisible();
  await host.getByRole('button',{name:'New Team'}).click();
  const wizard=host.getByRole('region',{name:'Guided Team Builder'});
  await expect(wizard.locator('.sa-wizard-layout .sa-step-nav')).toBeVisible();
  await expect(wizard).toContainText('Step 1 of 5');
  await expect(host.locator('.sa-teams-panel')).toHaveCount(0);
  await wizard.getByRole('button',{name:'Cancel team setup'}).click();
  await expect(host.getByText('Create your first Team')).toBeVisible();
});

/** A narrow, revision-guarded fake native service. No provider, file or GitHub
 * effect is performed; every mutation is recorded so incorrect dispatch fails. */
async function mountNativeFixture(page: Page, options: {
  reviewers?: number; mode?: 'approval'|'full_auto'|'off'; enabled?: boolean;
  withTicket?: boolean; ticketState?: string;
}={}) {
  await page.evaluate(async opts => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-v2-fixture';
    host.style.cssText='position:fixed;inset:0;z-index:10000;overflow:auto;background:var(--bg);padding:20px';
    document.body.appendChild(host);
    const bots=[
      {id:'coord',name:'Coordinator',paused:false,projectId:'project-1',provider:'codex',model:'test-coord',mode:'ask',skills:[]},
      {id:'worker',name:'Worker',paused:false,projectId:'project-1',provider:'codex',model:'test-worker',mode:'ask',skills:['native-test']},
      {id:'rev1',name:'Reviewer One',paused:false,projectId:'review-1',provider:'claude',model:'review-a',mode:'ask',skills:[]},
      {id:'rev2',name:'Reviewer Two',paused:false,projectId:'review-2',provider:'claude',model:'review-b',mode:'ask',skills:[]},
    ];
    const requestedReviewers=opts.reviewers ?? 2;
    const cfg={
      id:'team-1',name:'Native Team',coordinatorBotId:'coord',workerBotId:'worker',
      workerBotIds:['worker'],reviewerBotIds:['rev1','rev2'].slice(0,requestedReviewers),
      councilRequired:opts.mode==='full_auto',enabled:opts.enabled===true,
      policyRevision:1,teamVersion:1,workerCount:1,reviewerCount:requestedReviewers,
      ticketCount:0,runningCount:0,blockedCount:0,
    };
    let mode=opts.mode ?? 'approval';
    let ticket: {id:string;orchestrationId:string;objective:string;state:string;revision:number;evidence:null}|null =
      opts.withTicket ? {
        id:'demand-1',orchestrationId:cfg.id,objective:'Fix the native example',
        state:opts.ticketState ?? 'running',revision:2,evidence:null,
      } : null;
    const mutations:Array<{command:string;args:Record<string,unknown>}>=[];
    const reads:Array<{command:string;args:Record<string,unknown>}>=[];
    (window as unknown as {saFixtureReads:typeof reads}).saFixtureReads=reads;
    (window as unknown as {saFixtureMutations:typeof mutations;saFixtureEvents:unknown[]})
      .saFixtureMutations=mutations;
    (window as unknown as {saFixtureEvents:unknown[]}).saFixtureEvents=[];
    host.addEventListener('superagent-open-bot-chat',event=>{
      (window as unknown as {saFixtureEvents:unknown[]}).saFixtureEvents.push(
        (event as CustomEvent).detail
      );
    });
    const policy=()=>({
      orchestrationId:cfg.id,policyRevision:cfg.policyRevision,teamVersion:cfg.teamVersion,
      settings:{priority:'balanced',attemptTimeoutMs:300_000,subagentDelegationEnabled:false},
      executionMode:mode,maxAttemptsPerAssignment:2,maxConcurrentTasks:1,
      executableParallelWorkers:1,tokenMeteringAvailable:false,
      pricedCostMeteringAvailable:false,nestedChildAdmissionAvailable:true,enableBlockers:[],
    });
    const roster=()=>({
      orchestrationId:cfg.id,policyRevision:cfg.policyRevision,coordinatorBotId:cfg.coordinatorBotId,
      workerBotIds:[...cfg.workerBotIds],reviewerBotIds:[...cfg.reviewerBotIds],
      councilRequired:cfg.councilRequired,executionReady:true,
    });
    const snapshot=(id:string)=>{
      const isChild=id==='child-1';
      const entry=isChild ? {id:'child-1',orchestrationId:cfg.id,objective:'Implement isolated native WorkItem',
        state:'running',revision:1,evidence:null} : ticket;
      if(!entry)throw Error('No native Ticket matches the requested identity');
      return {ticket:entry,
        mandate:{actor:'local-user',authorizationId:'fixture',policyRevision:cfg.policyRevision,
          projectId:'project-1',expiresAt:Date.now()+100000,revoked:false,
          allowedTools:[],allowedSideEffects:false},
        assignments:[{id:isChild?'child-attempt':'attempt-1',botId:'worker',
          sessionId:isChild?'child-session-1':'worker-session-1',
          state:'running',attempt:1,createdAt:Date.now(),updatedAt:Date.now()}],
        attemptInsights:[{assignmentId:isChild?'child-attempt':'attempt-1',
          currentAssignment:true,hasResult:false,mandateRevoked:false,
          runBudgetExpired:false,remainingRunBudgetMs:20_000}],
        activity:[],teamSnapshot:null,finalizationGate:'not_pending',
      };
    };
    mountSuperagent(host,async(command,args={})=>{
      if(command==='bots_list')return bots as never;
      if(command==='superagent_list')return [{...cfg,ticketCount:ticket?1:0,runningCount:ticket?.state==='running'?1:0}] as never;
      if(command==='superagent_tickets')return (ticket&&args.orchestrationId===cfg.id?[ticket]:[]) as never;
      if(command==='superagent_strategy_get')return policy() as never;
      if(command==='superagent_roster')return roster() as never;
      if(command==='superagent_all_demands')return (ticket?[{id:ticket.id,
        orchestrationId:ticket.orchestrationId,objective:ticket.objective,
        state:ticket.state,revision:ticket.revision,updatedAt:Date.now(),
        projectId:'project-1'}]:[]) as never;
      if(command==='superagent_review_inbox')return (ticket&&
        ['waiting_review','waiting_approval'].includes(ticket.state)
        ?[{...ticket,updatedAt:Date.now(),projectId:'project-1'}]:[]) as never;
      if(command==='superagent_source_thread_links')return [] as never;
      if(command==='superagent_snapshot')return snapshot(String(args.ticketId)) as never;
      if(command==='superagent_demand_plan')return {
        ticketId:'demand-1',ticketRevision:ticket?.revision ?? 2,
        teamVersion:cfg.teamVersion,policyRevision:cfg.policyRevision,
        planSha256:'native-plan',state:'active',
        items:[{id:'item-1',key:'task-1',ordinal:0,workerBotId:'worker',
          objective:'Implement isolated native WorkItem',dependsOn:[],state:'running',
          childTicketId:'child-1',childTicketState:'running',lastReason:null,
          requiresWrite:false,writeAuthorized:false}],
      } as never;
      if(command==='superagent_roster_update'){
        if(cfg.policyRevision!==args.expectedRevision||cfg.enabled)
          throw Error('Stale native roster policy revision or active Team');
        const input=args.input as {workerBotIds:string[];reviewerBotIds:string[];councilRequired:boolean};
        if(input.councilRequired && input.reviewerBotIds.length<2)
          throw Error('Independent Council needs two reviewers');
        mutations.push({command,args});
        cfg.reviewerBotIds=[...input.reviewerBotIds];cfg.reviewerCount=cfg.reviewerBotIds.length;
        cfg.councilRequired=input.councilRequired;cfg.policyRevision++;cfg.teamVersion++;
        return roster() as never;
      }
      if(command==='superagent_strategy_update'){
        if(cfg.policyRevision!==args.expectedRevision||cfg.enabled)throw Error('Stale Team strategy revision');
        const input=args.input as {executionMode:string};
        if(input.executionMode==='full_auto'&&(!cfg.councilRequired||cfg.reviewerBotIds.length<2))
          throw Error('Full Auto requires independent Council');
        if(input.executionMode==='approval'&&cfg.councilRequired)
          throw Error('With Approval requires human acceptance');
        mutations.push({command,args});
        mode=input.executionMode as typeof mode;
        cfg.policyRevision++;cfg.teamVersion++;
        return policy() as never;
      }
      if(command==='superagent_set_enabled'){
        if(cfg.policyRevision!==args.expectedRevision)throw Error('Stale Team enabled revision');
        if(args.enabled&&mode==='off')throw Error('An explicit execution mode is required');
        mutations.push({command,args});cfg.enabled=Boolean(args.enabled);cfg.policyRevision++;
        return cfg as never;
      }
      if(command==='superagent_preflight'){
        reads.push({command,args});
        return {
          orchestrationId:cfg.id,policyRevision:cfg.policyRevision,
          projectId:'project-1',selectedBotId:'coord',selectedRole:'coordinator',
          eligible:false,reason:'Legacy manual runner cannot admit the selected multi-worker Team.',
          allowedTools:[],allowedSideEffects:false,attemptBudgetMs:300_000,
        } as never;
      }
      if(command==='superagent_start_demand'){
        if(!cfg.enabled||mode==='off')throw Error('Team is not enabled');
        mutations.push({command,args});
        ticket={id:'demand-1',orchestrationId:cfg.id,
          objective:String(args.objective),state:'running',revision:2,evidence:null};
        return ticket as never;
      }
      if(command==='superagent_stop'){
        if(!ticket||ticket.revision!==args.expectedRevision)throw Error('Stale Demand');
        mutations.push({command,args});ticket.state='stopped';ticket.revision++;
        return ticket as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  },options);
  const host=page.locator('#sa-v2-fixture');
  await expect(host).toHaveAttribute('aria-busy','false',{timeout:30_000});
  return host;
}

test('Team details use native bot inspector and five reference tabs',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await host.locator('[data-sa-select="team-1"]').click();
  await expect(host.locator('.sa-roster-panel')).toBeVisible();
  await expect(host.locator('.sa-member-inspector')).toBeVisible();
  await expect(host.locator('.sa-subtabs').getByRole('button',{name:'GitHub sources'})).toBeVisible();
  await expect(host.locator('.sa-subtabs').getByRole('button',{name:'Import / export'})).toBeVisible();
  await host.locator('[data-sa-member-inspect="worker"]').click();
  await expect(host.locator('.sa-member-inspector')).toContainText('test-worker');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Import / export'}).click();
  await expect(host.locator('[data-sa-manifest]')).toBeVisible();
});

test('empty Teams view provides an intentional, accessible first-use path',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  await expect(host.getByRole('heading',{name:'Teams',exact:true})).toBeVisible();
  await expect(host.getByText('Create your first Team')).toBeVisible();
  await expect(host.getByRole('button',{name:'New Team',exact:true})).toBeEnabled();
  await expect(host.locator('form[data-sa-form="create"]')).toHaveCount(0);
  await expect(host.getByText('Dispatch manually')).toHaveCount(0);
  await expect(host.getByRole('navigation',{name:'Orchestration views'})).toBeVisible();
});

test('Team search keeps focus while the directory rerenders',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  // The reference first-use screen has no list controls until a Team exists.
  // Seed one through the same native preview journey before testing focus.
  const builder=await completeWizard(host,'Software Search Team');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  const search=host.getByRole('searchbox',{name:'Search Teams'});
  await search.fill('So');
  await expect(search).toBeFocused();
  await search.press('f');
  await expect(search).toHaveValue('Sof');
});

test('five-step setup validates native bot identities and does not create a task',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  await host.getByRole('button',{name:'New Team',exact:true}).click();
  const builder=host.getByRole('region',{name:'Guided Team Builder'});
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder.getByRole('alert')).toContainText('Choose a Team name');
  await builder.getByRole('textbox',{name:'Team name'}).fill('Evidence-first Team');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder.getByRole('alert')).toContainText('Choose an existing Coordinator');
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.getByRole('combobox',{name:'Primary Worker'}).selectOption({label:'Writer'});
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Skill presence alone does not prove');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder.getByRole('radio',{name:/With Approval/})).toBeChecked();
});

test('With Approval Team persists its native execution mode and remains Off',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  const builder=await completeWizard(host,'Approval Delivery Team');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('With Approval');
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  await expect(host.getByRole('region',{name:'Guided Team Builder'})).toHaveCount(0);
  await openTeam(host,'Approval Delivery Team');
  await expect(host.getByText('With Approval · human acceptance')).toBeVisible();
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  await expect(host.locator('form[data-sa-form="strategy-update"] input[value="approval"]')).toBeChecked();
  await expect(host.locator('form[data-sa-form="strategy-update"]')).toBeVisible();
});

test('Full Auto wizard leads to two independent reviewers rather than granting hidden access',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  const builder=await completeWizard(host,'Review-gated Team','full_auto');
  await expect(builder.getByRole('button',{name:'Choose independent reviewers'})).toBeVisible();
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder.getByRole('alert')).toContainText('at least two selected Reviewers');
  await builder.getByRole('button',{name:'Choose independent reviewers'}).click();
  await expect(builder).toContainText('Step 2 of 5');
  await expect(host.locator('.sa-teams-panel .sa-item')).toHaveCount(0);
});

test('an enabled Team admits a Demand through the new screen and opens its native Run',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  const builder=await completeWizard(host,'Software Delivery');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  await openTeam(host,'Software Delivery');
  await host.getByRole('button',{name:'Enable Team'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await host.getByRole('button',{name:'Assign Demand',exact:true}).click();
  await expect(host.getByRole('heading',{name:'Assign a Demand'})).toBeVisible();
  await expect(host.getByRole('button',{name:'Start Demand'})).toBeEnabled();
  await expect(host.getByRole('combobox',{name:'Team for Demand'})).toHaveValue(
    await host.getByRole('combobox',{name:'Team for Demand'}).inputValue()
  );
  await host.getByRole('textbox',{name:'Task objective'}).fill('Fix issue and check native evidence');
  await host.getByRole('button',{name:'Start Demand'}).click();
  await expect(host.getByRole('region',{name:'Selected native Demand'}))
    .toContainText('Fix issue and check native evidence');
  await expect(host.getByRole('region',{name:'Selected native Demand'}))
    .toContainText('running');
  await expect(host.locator('form[data-sa-form="create"]')).toHaveCount(0);
});

test('Demand preflight reads native manual eligibility without granting or starting work',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{enabled:true});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.getByRole('button',{name:'New Demand'}).click();
  await expect(host.locator('form[data-sa-form="run"]')).toContainText('each write-required WorkItem must select a nonempty subset');
  const objective=host.getByRole('textbox',{name:'Task objective'});
  await objective.fill('Make an authorized source change');
  await host.getByRole('button',{name:'Review native preflight'}).click();
  const preview=host.getByRole('region',{name:'Native demand preflight'});
  await expect(preview).toContainText('Manual admission preview');
  await expect(preview).toContainText('Legacy manual runner cannot admit');
  await expect(preview).toContainText('not Full Auto readiness');
  const evidence=await page.evaluate(()=>({
    reads:(window as unknown as {saFixtureReads:Array<{command:string;args:Record<string,unknown>}>}).saFixtureReads,
    mutations:(window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations,
  }));
  expect(evidence.reads).toEqual([{
    command:'superagent_preflight',
    args:{orchestrationId:'team-1',objective:'Make an authorized source change',coordinated:true},
  }]);
  expect(evidence.mutations).toEqual([]);
  await objective.fill('Change the outcome');
  await expect(preview).toHaveCount(0);
  await host.getByRole('button',{name:'Review native preflight'}).click();
  await expect(preview).toContainText('Manual admission preview');
  expect(await page.evaluate(()=>(
    window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations.length)).toBe(0);
});

test('Run board separates navigation from task admission and retains Board/List options',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await expect(host.getByRole('heading',{name:'Demands / Runs',exact:true})).toBeVisible();
  await expect(host.getByRole('button',{name:'New Demand'})).toBeVisible();
  await expect(host.getByRole('region',{name:'All Demands across Teams'})).toBeVisible();
  await expect(host.locator('form[data-sa-form="run"]')).toHaveCount(0);
});

test('native execution mode switch enables required Council before writing Full Auto',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/Full Auto/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  await expect(host).toContainText('Full Auto');
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>})
      .saFixtureMutations);
  expect(calls.map(entry=>entry.command)).toEqual(['superagent_roster_update','superagent_strategy_update']);
  expect(calls[0].args.expectedRevision).toBe(1);
  expect(calls[1].args.expectedRevision).toBe(2);
  expect((calls[1].args.input as {executionMode:string}).executionMode).toBe('full_auto');
});

test('switching Full Auto back to With Approval removes Council before changing mode',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full_auto'});
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/With Approval/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const commands=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string}>}).saFixtureMutations.map(x=>x.command));
  expect(commands).toEqual(['superagent_roster_update','superagent_strategy_update']);
  await expect(form.getByRole('radio',{name:/With Approval/})).toBeChecked();
});

test('Full Auto configuration rejects insufficient reviewers without native mutation',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{reviewers:1});
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/Full Auto/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host.getByRole('alert')).toContainText('needs two independent reviewer bots');
  await expect(host.getByRole('region',{name:'Configured Team roles'})).toBeVisible();
  expect(await page.evaluate(()=>(
    window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations.length)).toBe(0);
});

test('native run inspector opens the exact assigned bot conversation',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await expect(host.locator('[data-sa-open-demand="demand-1"]')).toBeVisible();
  await host.locator('[data-sa-open-demand="demand-1"]').click();
  await expect(host.getByRole('region',{name:'Selected native Demand'}))
    .toContainText('Fix the native example');
  await host.locator('[data-sa-inspect-assignment="attempt-1"]').click();
  await host.getByRole('button',{name:'Open Worker chat'}).click();
  const events=await page.evaluate(()=>
    (window as unknown as {saFixtureEvents:unknown[]}).saFixtureEvents);
  expect(events).toEqual([{sessionId:'worker-session-1',botId:'worker'}]);
});

test('a native child WorkItem opens its own bot session, not a separate SuperAgent chat',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.locator('[data-sa-open-demand="demand-1"]').click();
  await host.getByRole('button',{name:'Inspect WorkItems'}).click();
  await host.getByRole('button',{name:'Open Worker chat'}).click();
  await expect.poll(async()=>page.evaluate(()=>
    (window as unknown as {saFixtureEvents:unknown[]}).saFixtureEvents.length)).toBe(1);
  const events=await page.evaluate(()=>
    (window as unknown as {saFixtureEvents:unknown[]}).saFixtureEvents);
  expect(events).toEqual([{sessionId:'child-session-1',botId:'worker'}]);
});

test('review inbox shows pending Council as pending, not as accepted evidence',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,ticketState:'waiting_review'});
  await host.getByRole('button',{name:'Review inbox'}).click();
  await expect(host.getByRole('region',{name:'Cross-Team native review inbox'}))
    .toContainText('Awaiting native Council (not approved)');
  await expect(host.getByRole('button',{name:'Accept evidence'})).toHaveCount(0);
});

test('a native Stop rechecks the exact Ticket revision and records no other effects',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.locator('[data-sa-open-demand="demand-1"]').click();
  await host.getByRole('button',{name:'Stop',exact:true}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>})
      .saFixtureMutations);
  expect(calls.map(x=>x.command)).toEqual(['superagent_stop']);
  expect(calls[0].args.ticketId).toBe('demand-1');
  expect(calls[0].args.expectedRevision).toBe(2);
});

test('native Team settings and Demand pages remain reachable in a compact viewport',async({page})=>{
  await page.setViewportSize({width:390,height:844});
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  await host.getByRole('button',{name:'New Team',exact:true}).click();
  await expect(host.getByRole('region',{name:'Guided Team Builder'})).toBeVisible();
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await expect(host.getByRole('button',{name:'New Demand'})).toBeVisible();
  await expect(host.locator('form[data-sa-form="create"]')).toHaveCount(0);
});

test('native bot Conversations identifies SuperAgent threads and never calls them removed channels',async({page})=>{
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {renderPanel}=await import('/src/bots-panels.ts');
    const element=document.createElement('div');
    element.id='sa-bot-native-history-test';
    document.body.appendChild(element);
    const detail={
      bot:{id:'worker',name:'Worker'},
      threads:[
        {channelId:'superagent',externalId:'demand-child-7f0c1d2a',sessionId:'native-worker-session',
         updatedAt:Date.now()},
        {channelId:'desktop',externalId:'desktop',sessionId:'original-bot-chat',
         updatedAt:Date.now()-1000},
        {channelId:'schedule',externalId:'schedule-1',sessionId:'latest-scheduled-chat',
         updatedAt:Date.now()+1000},
      ],
      channels:[],
    };
    element.innerHTML=renderPanel('conversations',detail as never,
      {running:()=>false} as never);
  });
  const section=page.getByRole('region',{name:'SuperAgent tasks'});
  await expect(section).toContainText('SuperAgent task');
  await expect(section).not.toContainText('Removed channel');
  await expect(section.locator('[data-open-task="native-worker-session"]')).toHaveText('Open chat');
  const ordinary=page.getByRole('region',{name:'Other bot conversations'});
  await expect(ordinary).toContainText('This app');
  await expect(ordinary.locator('.bots-items li').first()).toContainText('This app');
  await expect(ordinary.locator('[data-open-task="latest-scheduled-chat"]'))
    .toHaveText('Open chat');
});
