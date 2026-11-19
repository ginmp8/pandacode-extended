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

async function completeWizard(host: Locator, name: string, mode: 'approval'|'full' = 'approval') {
  await host.getByRole('button', {name:'New Team', exact:true}).click();
  const builder=host.getByRole('region', {name:'Guided Team Builder'});
  await expect(builder).toContainText('Step 1 of 5');
  await builder.getByRole('textbox', {name:'Team name'}).fill(name);
  await builder.getByRole('button', {name:'Next'}).click();
  await builder.getByRole('combobox', {name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.locator('details[data-sa-multi="builderWorkers"] summary').click();
  await builder.locator('details[data-sa-multi="builderWorkers"]').getByRole('checkbox',{name:/Writer/}).check();
  await builder.locator('details[data-sa-multi="builderWorkers"]').getByRole('button',{name:'Done'}).click();
  await builder.getByRole('button', {name:'Next'}).click();
  await expect(builder).toContainText('Skill presence alone does not prove');
  await builder.getByRole('button', {name:'Next'}).click();
  await builder.getByRole('radio', {name:mode==='full' ? /Full Access/ : /With Approval/}).check();
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
  reviewers?: number; council?: boolean; mode?: 'approval'|'full'|'off'; enabled?: boolean;
  withTicket?: boolean; ticketState?: string; blockedHasPlan?: boolean; revoked?: boolean;
  withOtherTeam?: boolean;
}={}) {
  await page.evaluate(async opts => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-v2-fixture';
    host.style.cssText='position:fixed;inset:0;z-index:10000;overflow:auto;background:var(--bg);padding:20px';
    document.body.appendChild(host);
    const bots=[
      {id:'coord',name:'Coordinator',avatar:'panda:0',paused:false,projectId:'project-1',provider:'codex',model:'test-coord',mode:'ask',skills:[]},
      {id:'worker',name:'Worker',avatar:'panda:1',paused:false,projectId:'project-1',provider:'codex',model:'test-worker',mode:'ask',skills:['native-test']},
      {id:'worker2',name:'Worker Two',avatar:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/Vy8AAAAASUVORK5CYII=',paused:false,projectId:'project-1',provider:'codex',model:'test-worker-2',mode:'ask',skills:[]},
      {id:'other',name:'Other Project Worker',avatar:'panda:3',paused:false,projectId:'project-2',provider:'codex',model:'other',mode:'ask',skills:[]},
      {id:'paused',name:'Paused Worker',avatar:'panda:2',paused:true,projectId:'project-1',provider:'codex',model:'paused',mode:'ask',skills:[]},
      {id:'rev1',name:'Reviewer One',avatar:'panda:4',paused:false,projectId:'review-1',provider:'claude',model:'review-a',mode:'ask',skills:[]},
      {id:'rev2',name:'Reviewer Two',avatar:'panda:5',paused:false,projectId:'review-2',provider:'claude',model:'review-b',mode:'ask',skills:[]},
    ];
    const requestedReviewers=opts.reviewers ?? 2;
    const cfg={
      id:'team-1',name:'Native Team',coordinatorBotId:'coord',workerBotId:'worker',
      workerBotIds:['worker'],reviewerBotIds:['rev1','rev2'].slice(0,requestedReviewers),
      councilRequired:requestedReviewers>0,enabled:opts.enabled===true,
      policyRevision:1,teamVersion:1,workerCount:1,reviewerCount:requestedReviewers,
      ticketCount:0,runningCount:0,blockedCount:0,
    };
    let mode=opts.mode ?? 'approval';
    let archivedTicket=false;
    const otherCfg={...cfg,id:'team-2',name:'Other Private Team',ticketCount:1};
    const otherTicket=opts.withOtherTeam ? {
      id:'demand-other',orchestrationId:'team-2',objective:'Private demand from another Team',
      state:'completed',revision:1,evidence:null,
    } : null;
    let ticket: {id:string;orchestrationId:string;objective:string;state:string;revision:number;evidence:null}|null =
      opts.withTicket ? {
        id:'demand-1',orchestrationId:cfg.id,objective:'Fix the native example',
        state:opts.ticketState ?? 'running',revision:2,evidence:null,
      } : null;
    const mutations:Array<{command:string;args:Record<string,unknown>}>=[];
    const reads:Array<{command:string;args:Record<string,unknown>}>=[];
    const demandQueries:Array<Record<string,unknown>>=[];
    (window as unknown as {saFixtureDemandQueries:typeof demandQueries})
      .saFixtureDemandQueries=demandQueries;
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
      return {ticket:entry,archived:!isChild && archivedTicket,
        mandate:{actor:'local-user',authorizationId:'fixture',policyRevision:cfg.policyRevision,
          projectId:'project-1',expiresAt:Date.now()+100000,revoked:opts.revoked===true,
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
      if(command==='superagent_list')return [
        {...cfg,ticketCount:ticket?1:0,runningCount:ticket?.state==='running'?1:0},
        ...(otherTicket?[otherCfg]:[]),
      ] as never;
      if(command==='superagent_tickets')return (
        args.orchestrationId===cfg.id && ticket && !archivedTicket ? [ticket] :
        args.orchestrationId==='team-2' && otherTicket ? [otherTicket] : []
      ) as never;
      if(command==='superagent_strategy_get')return policy() as never;
      if(command==='superagent_roster')return roster() as never;
      if(command==='superagent_capability_candidates'){
        reads.push({command,args});
        if(args.orchestrationId!==cfg.id)throw Error('Wrong Team');
        return {orchestrationId:cfg.id,policyRevision:cfg.policyRevision,
          observedAt:Date.now(),excludedByPrivacyPreference:1,truncated:false,
          selectionApplied:false,authorizationGranted:false,
          disclaimer:'Declared metadata only',candidates:[{
            botId:'worker2',name:'Worker Two',suggestedRole:'worker',
            alreadyAssigned:false,declaredMatchScore:3,matchedTerms:['testing'],
            evidenceSources:['assigned_skill_reference'],status:'declared_match',
            projectScope:{projectId:'project-1',status:'project_reference_only'},
            permissionMode:{value:'ask',status:'not_an_execution_grant'},
            availability:{status:'not_paused_runtime_unknown',paused:false},
            executionEligibility:'not_verified',sourceConfidence:'declarations_only',
          }]} as never;
      }
      if(command==='superagent_all_demands'){
        const query=(args.query ?? args) as {orchestrationId?:string;archived?:boolean};
        demandQueries.push(query as Record<string,unknown>);
        const items=[
          ...(ticket?[{id:ticket.id,orchestrationId:ticket.orchestrationId,
            objective:ticket.objective,state:ticket.state,revision:ticket.revision,
            updatedAt:Date.now(),projectId:'project-1',archived:archivedTicket}]:[]),
          ...(otherTicket?[{id:otherTicket.id,orchestrationId:otherTicket.orchestrationId,
            objective:otherTicket.objective,state:otherTicket.state,
            revision:otherTicket.revision,updatedAt:Date.now()-1000,
            projectId:'project-2',archived:false}]:[]),
        ];
        return items.filter(item=>
          (!query.orchestrationId || item.orchestrationId===query.orchestrationId)
          && item.archived===Boolean(query.archived)) as never;
      }
      if(command==='superagent_review_inbox')return (ticket&&
        ['waiting_review','waiting_approval'].includes(ticket.state)
        ?[{...ticket,updatedAt:Date.now(),projectId:'project-1'}]:[]) as never;
      if(command==='superagent_source_thread_links')return [] as never;
      if(command==='superagent_snapshot')return snapshot(String(args.ticketId)) as never;
      if(command==='superagent_demand_plan'){
        if(ticket?.state==='blocked' && !opts.blockedHasPlan) return null as never;
        return {
        ticketId:'demand-1',ticketRevision:ticket?.revision ?? 2,
        teamVersion:cfg.teamVersion,policyRevision:cfg.policyRevision,
        planSha256:'native-plan',state:'active',
        items:[{id:'item-1',key:'task-1',ordinal:0,workerBotId:'worker',
          objective:'Implement isolated native WorkItem',dependsOn:[],
          state:ticket?.state==='blocked'?'blocked':'running',
          childTicketId:'child-1',childTicketState:'running',
          lastReason:ticket?.state==='blocked'?'Native Worker delivery failed':null,
          requiresWrite:false,writeAuthorized:false}],
        } as never;
      }
      if(command==='superagent_roster_update'){
        if(cfg.policyRevision!==args.expectedRevision||cfg.enabled)
          throw Error('Stale native roster policy revision or active Team');
        const input=args.input as {workerBotIds:string[];reviewerBotIds:string[];councilRequired:boolean};
        if(input.councilRequired !== (input.reviewerBotIds.length > 0) || input.reviewerBotIds.length === 1)
          throw Error('Configured Council requires two or more reviewers');
        mutations.push({command,args});
        cfg.workerBotIds=[...input.workerBotIds];cfg.workerCount=cfg.workerBotIds.length;
        cfg.reviewerBotIds=[...input.reviewerBotIds];cfg.reviewerCount=cfg.reviewerBotIds.length;
        cfg.councilRequired=input.councilRequired;cfg.policyRevision++;cfg.teamVersion++;
        return roster() as never;
      }
      if(command==='superagent_strategy_update'){
        if(cfg.policyRevision!==args.expectedRevision||cfg.enabled)throw Error('Stale Team strategy revision');
        const input=args.input as {executionMode:string};
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
      if(command==='superagent_retry'){
        if(!ticket||ticket.state!=='blocked')throw Error('Only a blocked native Demand can retry');
        mutations.push({command,args});ticket.state='running';ticket.revision++;
        return ticket as never;
      }
      if(command==='superagent_accept'||command==='superagent_reject'){
        if(!ticket||ticket.state!=='waiting_approval'||ticket.revision!==args.expectedRevision)
          throw Error('Stale native approval');
        if(command==='superagent_reject'&&!String(args.reason??'').trim())
          throw Error('Rejecting evidence requires a reason');
        mutations.push({command,args});
        ticket.state=command==='superagent_accept'?'completed':'blocked';
        ticket.revision++;
        return ticket as never;
      }
      if(command==='superagent_stop'){
        if(!ticket||ticket.revision!==args.expectedRevision ||
            ['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state))
          throw Error('Stale or terminal Demand');
        mutations.push({command,args});ticket.state='stopped';ticket.revision++;
        return ticket as never;
      }
      if(command==='superagent_archive_demand'){
        if(!ticket||ticket.revision!==args.expectedRevision ||
            !['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state))
          throw Error('Cancel Demand before deleting');
        if(archivedTicket)throw Error('Demand already archived');
        mutations.push({command,args});archivedTicket=true;
        return undefined as never;
      }
      if(command==='superagent_restore_demand'){
        if(!ticket||ticket.revision!==args.expectedRevision || !archivedTicket)
          throw Error('Stale archived Demand');
        mutations.push({command,args});archivedTicket=false;
        return undefined as never;
      }
      throw Error('Unexpected native command: '+command);
    });
  },options);
  const host=page.locator('#sa-v2-fixture');
  await expect(host).toHaveAttribute('aria-busy','false',{timeout:30_000});
  return host;
}

test('Team wizard selects peer Workers from multiple projects with native bot images',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await host.getByRole('button',{name:'New Team',exact:true}).click();
  const wizard=host.getByRole('region',{name:'Guided Team Builder'});
  await wizard.getByRole('textbox',{name:'Team name'}).fill('Image-aware Team');
  await wizard.getByRole('button',{name:'Next'}).click();
  await wizard.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Coordinator'});
  await wizard.locator('details[data-sa-multi="builderWorkers"] summary').click();
  await wizard.locator('details[data-sa-multi="builderWorkers"]').locator('input[data-sa-multi-option][value="worker"]').check();
  await wizard.locator('details[data-sa-multi="builderWorkers"]').getByRole('button',{name:'Done'}).click();
  const workers=wizard.locator('details[data-sa-multi="builderWorkers"]');
  await expect(workers.locator('select[multiple]')).toHaveCount(0);
  await workers.locator('summary').click();
  await expect(workers.getByRole('checkbox',{name:/Paused Worker/})).toBeDisabled();
  await expect(workers.getByRole('checkbox',{name:/Other Project Worker/})).toBeEnabled();
  const search=workers.getByRole('searchbox',{name:/Search Workers/});
  await search.fill('worker two');
  await workers.getByRole('checkbox',{name:/Worker Two/}).check();
  await expect(workers).toHaveAttribute('open','');
  await expect(workers.getByRole('searchbox')).toHaveValue('worker two');
  await expect(workers.locator('.sa-multi-faces img')).toHaveAttribute('src',/^data:image\/png;base64,/);
  await expect(workers.locator('.sa-multi-count')).toContainText('2 / 16');
  await workers.getByRole('button',{name:'Done'}).click();
  await expect(workers).not.toHaveAttribute('open','');
  const council=wizard.locator('details[data-sa-multi="builderReviewers"]');
  await council.locator('summary').click();
  await expect(council.getByRole('checkbox',{name:/Worker Two/})).toBeDisabled();
  await council.getByRole('checkbox',{name:/Reviewer One/}).check();
  await council.getByRole('checkbox',{name:/Reviewer Two/}).check();
  await expect(council.locator('.sa-multi-faces svg')).toHaveCount(2);
  await council.getByRole('button',{name:'Done'}).click();
  await wizard.getByRole('button',{name:'Next'}).click();
  await expect(wizard.getByText(/Worker Two/).first()).toBeVisible();
});

test('capability matching stages a Worker only after explicit native roster save',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Members and roles'}).click();
  const load=host.getByRole('button',{name:'Load native members'});
  if(await load.count())await load.click();
  const fit=host.getByRole('region',{name:'Bot capability matching'});
  await expect(fit).toBeVisible();
  await fit.getByRole('textbox',{name:'Work objective'}).fill('Rust testing');
  await fit.getByRole('button',{name:'Compare current capabilities'}).click();
  await expect(fit).toContainText('Worker Two');
  await expect(fit).toContainText('Execution eligibility not verified');
  expect(await page.evaluate(()=>(window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations.length)).toBe(0);
  await fit.getByRole('button',{name:'Stage as Worker'}).click();
  const workers=host.locator('details[data-sa-multi="workers"]');
  await expect(workers.locator('input[data-sa-multi-option][value="worker2"]')).toBeChecked();
  expect(await page.evaluate(()=>(window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations.length)).toBe(0);
  await host.getByRole('button',{name:'Save configured roles'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>}).saFixtureMutations);
  expect(calls.at(-1)?.command).toBe('superagent_roster_update');
  expect((calls.at(-1)?.args.input as {workerBotIds:string[]}).workerBotIds).toEqual(['worker','worker2']);
});

test('native Team role picker preserves checked bots and saves ordered IDs only on submit',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Members and roles'}).click();
  const load=host.getByRole('button',{name:'Load native members'});
  if (await load.count()) await load.click();
  const workers=host.locator('details[data-sa-multi="workers"]');
  await expect(workers).toBeVisible();
  await workers.locator('summary').click();
  await workers.getByRole('checkbox',{name:/Worker Two/}).check();
  await expect(workers.getByRole('checkbox',{name:/Worker Two/})).toBeChecked();
  await expect(workers).toHaveAttribute('open','');
  await expect(workers.locator('.sa-multi-faces img')).toHaveCount(1);
  await workers.getByRole('button',{name:'Done'}).click();
  const council=host.locator('details[data-sa-multi="reviewers"]');
  await council.locator('summary').click();
  await expect(workers).not.toHaveAttribute('open','');
  await expect(council.getByRole('checkbox',{name:/Worker Two/})).toBeDisabled();
  await expect(council.getByRole('checkbox',{name:/Reviewer One/})).toBeChecked();
  await council.getByRole('button',{name:'Done'}).click();
  // A closed native <details> hides its inputs from the accessibility tree.
  // Assert the actual checked input without reopening the picker.
  await expect(workers.locator('input[data-sa-multi-option][value="worker2"]')).toBeChecked();
  expect(await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:unknown[]}).saFixtureMutations.length)).toBe(0);
  await host.getByRole('button',{name:'Save configured roles'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>})
      .saFixtureMutations);
  expect(calls.at(-1)?.command).toBe('superagent_roster_update');
  expect((calls.at(-1)?.args.input as {workerBotIds:string[]}).workerBotIds)
    .toEqual(['worker','worker2']);
  expect((calls.at(-1)?.args.input as {reviewerBotIds:string[]}).reviewerBotIds)
    .toEqual(['rev1','rev2']);
});

test('Team details expose native bot inspector and scoped Demand tasks tab',async({page})=>{
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
  await expect(builder.getByRole('alert')).toContainText('Choose one Coordinator and at least one Worker');
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.locator('details[data-sa-multi="builderWorkers"] summary').click();
  await builder.locator('details[data-sa-multi="builderWorkers"]').getByRole('checkbox',{name:/Writer/}).check();
  await builder.locator('details[data-sa-multi="builderWorkers"]').getByRole('button',{name:'Done'}).click();
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

test('Full Access wizard allows zero reviewers and configures native Full Access',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await openWorkspace(page);
  const builder=await completeWizard(host,'Access Team','full');
  await expect(builder).toContainText('Configured reviewers are always invoked');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Step 5 of 5');
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  await expect(host.getByRole('region',{name:'Guided Team Builder'})).toHaveCount(0);
  await openTeam(host,'Access Team');
  await expect(host).toContainText('Full Access');
  await expect(host.locator('.sa-team-policy'))
    .toContainText('Native Full Access; no separate WorkItem permission');
  await expect(host.locator('.sa-team-policy')).not.toContainText('Scoped write grants');
  await host.getByRole('button',{name:'Assign Demand',exact:true}).click();
  await expect(host.locator('.sa-admission-summary'))
    .toContainText('Native Full Access · no separate WorkItem grant');
  await expect(host.locator('.sa-admission-summary'))
    .toContainText('Off · zero reviewers allowed');
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
  await expect(host.locator('form[data-sa-form="run"]')).toContainText('existing native bot/chat permissions');
  await expect(host.getByRole('combobox',{name:'Native Demand test command'})).toHaveCount(0);
  await expect(host.locator('[name="initialAllowedPaths"]')).toHaveCount(0);
  const objective=host.getByRole('textbox',{name:'Task objective'});
  await objective.fill('Make an authorized source change');
  await host.getByRole('button',{name:'Review native preflight'}).click();
  const preview=host.getByRole('region',{name:'Native demand preflight'});
  await expect(preview).toContainText('Manual admission preview');
  await expect(preview).toContainText('Legacy manual runner cannot admit');
  await expect(preview).toContainText('not Full Access readiness');
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

test('native Full Access mode does not modify Council settings',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/Full Access/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  await expect(host).toContainText('Full Access');
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>})
      .saFixtureMutations);
  expect(calls.map(entry=>entry.command)).toEqual(['superagent_strategy_update']);
  expect(calls[0].args.expectedRevision).toBe(1);
  expect((calls[0].args.input as {executionMode:string}).executionMode).toBe('full');
});

test('switching Full Access back to With Approval leaves independent Council off',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full'});
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/With Approval/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const commands=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string}>}).saFixtureMutations.map(x=>x.command));
  expect(commands).toEqual(['superagent_strategy_update']);
  await expect(form.getByRole('radio',{name:/With Approval/})).toBeChecked();
});

test('native Full Access configuration accepts zero Council reviewers',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{reviewers:0});
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Execution and review'}).click();
  const form=host.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('radio',{name:/Full Access/}).check();
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const calls=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string}>}).saFixtureMutations.map(x=>x.command));
  expect(calls).toEqual(['superagent_strategy_update']);
  await expect(form.getByRole('radio',{name:/Full Access/})).toBeChecked();
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

test('Team tasks board shows only its own native Demands and per-Demand WorkItems',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,withOtherTeam:true});
  await host.locator('[data-sa-team-tasks="team-1"]').click();
  const team=host.getByRole('region',{name:'Team Demands and Tasks'});
  await expect(team).toBeVisible();
  await expect(team).toContainText('Fix the native example');
  await expect(team).not.toContainText('Private demand from another Team');
  await expect(host.locator('.sa-subtabs').getByRole('button',{name:'Demands & Tasks'}))
    .toHaveAttribute('aria-current','page');
  await team.locator('[data-sa-team-demand-tasks="demand-1"]').click();
  await expect(team.getByRole('region',{name:'Demand task timeline'}))
    .toContainText('Implement isolated native WorkItem');
  await expect(team.getByRole('region',{name:'Demand task timeline'}))
    .toContainText('Worker: Worker');
  const queries=await page.evaluate(()=>
    (window as unknown as {saFixtureDemandQueries:Array<Record<string,unknown>>})
      .saFixtureDemandQueries);
  expect(queries.some(q=>q.orchestrationId==='team-1' && q.archived===false)).toBe(true);
  await team.getByRole('button',{name:'Open full Demand timeline'}).click();
  const details=host.getByRole('region',{name:'Selected native Demand'});
  await expect(details).toContainText('Fix the native example');
  await details.getByRole('button',{name:'Back to Team Tasks'}).click();
  await expect(host.getByRole('region',{name:'Team Demands and Tasks'})).toBeVisible();
});

test('Delete Demand cancels an active native Ticket and archives it reversibly',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.locator('[data-sa-team-tasks="team-1"]').click();
  const team=host.getByRole('region',{name:'Team Demands and Tasks'});
  await team.getByRole('button',{name:'Delete Demand'}).click();
  const confirmation=host.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(confirmation).toContainText('reversible archive');
  await confirmation.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host).toHaveAttribute('aria-busy','false');
  const first=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>})
      .saFixtureMutations);
  expect(first.map(x=>x.command)).toEqual([
    'superagent_stop','superagent_archive_demand',
  ]);
  expect(first[0].args).toEqual({ticketId:'demand-1',expectedRevision:2});
  expect(first[1].args).toEqual({ticketId:'demand-1',expectedRevision:3});
  await expect(team).not.toContainText('Fix the native example');
  await team.getByRole('button',{name:'Archived (restore)'}).click();
  await expect(team.getByRole('button',{name:'Restore Demand'})).toBeVisible();
  await team.getByRole('button',{name:'Restore Demand'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(team).toContainText('Fix the native example');
  await team.getByRole('button',{name:'Active Demands'}).click();
  await expect(team).toContainText('Fix the native example');
  const all=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string}>})
      .saFixtureMutations.map(x=>x.command));
  expect(all).toEqual(['superagent_stop','superagent_archive_demand',
    'superagent_restore_demand']);
  expect(all).not.toContain('superagent_start_demand');
});

test('Cancel Demand is available in its Team board, not only the global board',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.locator('[data-sa-team-tasks="team-1"]').click();
  const team=host.getByRole('region',{name:'Team Demands and Tasks'});
  await team.getByRole('button',{name:'Cancel Demand'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(team).toContainText('stopped');
  await expect(team.getByRole('button',{name:'Cancel Demand'})).toHaveCount(0);
  await expect(team.getByRole('button',{name:'Delete Demand'})).toBeVisible();
});

test('a native Stop rechecks the exact Ticket revision and records no other effects',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{withTicket:true,enabled:true});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.locator('[data-sa-open-demand="demand-1"]').click();
  await host.getByRole('button',{name:'Cancel Demand',exact:true}).click();
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

test('Team details expose the native enable toggle beside Configure on every tab',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full'});
  await openTeam(host,'Native Team');
  const header=host.locator('.sa-team-detail-title .sa-detail-actions');
  await expect(header.getByRole('button',{name:'Enable Team'})).toBeVisible();
  await host.locator('.sa-subtabs').getByRole('button',{name:'Members and roles'}).click();
  await expect(header.getByRole('button',{name:'Enable Team'})).toBeVisible();
  await header.getByRole('button',{name:'Enable Team'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(header.getByRole('button',{name:'Disable Team'})).toBeVisible();
  const writes=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>}).saFixtureMutations);
  expect(writes).toEqual([{command:'superagent_set_enabled',
    args:{orchestrationId:'team-1',expectedRevision:1,enabled:true}}]);
});

test('Worker checkbox and Council selection have consistent label alignment',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full'});
  await openTeam(host,'Native Team');
  await host.locator('.sa-subtabs').getByRole('button',{name:'Members and roles'}).click();
  const workersPicker=host.locator('details[data-sa-multi="workers"]');
  await expect(workersPicker).toBeVisible();
  await host.getByRole('button',{name:'Teams',exact:true}).click();
  await host.getByRole('button',{name:'New Team',exact:true}).click();
  const builder=host.getByRole('region',{name:'Guided Team Builder'});
  await builder.getByRole('textbox',{name:'Team name'}).fill('Checkbox alignment Team');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Coordinator'});
  await builder.locator('details[data-sa-multi="builderWorkers"] summary').click();
  await builder.locator('details[data-sa-multi="builderWorkers"]').locator('input[data-sa-multi-option][value="worker"]').check();
  // Both role pickers should align the checkbox alongside its label.
  const workerRow=builder.locator('details[data-sa-multi="builderWorkers"] .sa-multi-option').first();
  const reviewerRow=builder.locator('details[data-sa-multi="builderReviewers"] .sa-multi-option').first();
  await expect(workerRow).toHaveCSS('flex-direction','row');
  await expect(workerRow).toHaveCSS('align-items','center');
  await expect(reviewerRow).toHaveCSS('flex-direction','row');
  await expect(reviewerRow).toHaveCSS('align-items','center');
  await builder.locator('details[data-sa-multi="builderWorkers"]').getByRole('button',{name:'Done'}).click();
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('radio',{name:/Full Access/}).check();
  await expect(builder.getByRole('radio',{name:/Full Access/})).toBeChecked();
  await expect(builder).toContainText('Configured reviewers are always invoked');
});

test('Team Builder validation is a highlighted visible warning, not muted footer text',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page);
  await host.getByRole('button',{name:'New Team',exact:true}).click();
  const builder=host.getByRole('region',{name:'Guided Team Builder'});
  await builder.getByRole('button',{name:'Next'}).click();
  const warning=builder.getByRole('alert');
  await expect(warning).toContainText('Choose a Team name');
  const visual=await warning.evaluate(el=>({
    background:getComputedStyle(el).backgroundColor,
    left:getComputedStyle(el).borderLeftWidth,
  }));
  expect(visual.background).not.toBe('rgba(0, 0, 0, 0)');
  expect(parseFloat(visual.left)).toBeGreaterThanOrEqual(2);
});

test('blocked Demand offers genuine Retry and a fresh-run alternative',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full',enabled:true,withTicket:true,ticketState:'blocked'});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.getByRole('button',{name:'Resolve demand'}).click();
  const attention=host.getByRole('region',{name:'Demand needs attention'});
  await expect(attention.getByRole('button',{name:'Retry current Demand'})).toBeVisible();
  await expect(attention.getByRole('button',{name:'Start new Demand'})).toBeVisible();
  await attention.getByRole('button',{name:'Retry current Demand'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  const writes=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>}).saFixtureMutations);
  expect(writes.map(x=>x.command)).toEqual(['superagent_retry']);
});

test('blocked DAG and a revoked mandate cannot expose invalid native Retry',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{
    mode:'full',enabled:true,withTicket:true,ticketState:'blocked',blockedHasPlan:true,
  });
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.getByRole('button',{name:'Resolve demand'}).click();
  const panel=host.getByRole('region',{name:'Demand needs attention'});
  await expect(panel.getByRole('button',{name:'Retry current Demand'})).toHaveCount(0);
  await expect(panel.getByRole('button',{name:'Start new Demand'})).toBeVisible();
  await expect(panel).toContainText('Native retry is not available');
  await expect(panel).toContainText('Blocked WorkItem task-1: Native Worker delivery failed');
});

test('Needs attention explains revoked native authorization without offering Retry',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{
    mode:'full',enabled:true,withTicket:true,ticketState:'blocked',revoked:true,
  });
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.getByRole('button',{name:'Resolve demand'}).click();
  const attention=host.getByRole('region',{name:'Demand needs attention'});
  await expect(attention).toContainText('original native authorization was revoked');
  await expect(attention.getByRole('button',{name:'Retry current Demand'})).toHaveCount(0);
  await expect(attention.getByRole('button',{name:'Start new Demand'})).toBeVisible();
});

test('failed Demand cannot be falsely approved and pre-fills a separately authorized new run',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{mode:'full',enabled:true,withTicket:true,ticketState:'failed'});
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.getByRole('button',{name:'Resolve demand'}).click();
  const attention=host.getByRole('region',{name:'Demand needs attention'});
  await expect(attention.getByRole('button',{name:'Retry current Demand'})).toHaveCount(0);
  await expect(attention.getByRole('button',{name:'Start new Demand'})).toBeVisible();
  await attention.getByRole('button',{name:'Start new Demand'}).click();
  await expect(host.getByRole('heading',{name:'Assign a Demand'})).toBeVisible();
  await expect(host.getByRole('textbox',{name:'Task objective'})).toHaveValue('Fix the native example');
  const writes=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string}>}).saFixtureMutations);
  expect(writes).toEqual([]);
});

test('With Approval exposes a native reasoned rejection without self-approval',async({page})=>{
  await page.goto('/?bots=sample');
  const host=await mountNativeFixture(page,{
    mode:'approval',enabled:true,withTicket:true,ticketState:'waiting_approval',
  });
  await host.getByRole('button',{name:'Demands / Runs',exact:true}).click();
  await host.locator('[data-sa-open-demand="demand-1"]').click();
  const details=host.getByRole('region',{name:'Selected native Demand'});
  await expect(details.getByRole('button',{name:'Accept evidence'})).toBeVisible();
  await details.getByRole('button',{name:'Reject evidence'}).click();
  await details.getByRole('textbox',{name:'Rejection reason'}).fill('Native test evidence is missing');
  await details.getByRole('button',{name:'Submit rejection'}).click();
  const actions=await page.evaluate(()=>
    (window as unknown as {saFixtureMutations:Array<{command:string;args:Record<string,unknown>}>}).saFixtureMutations);
  expect(actions).toEqual([{command:'superagent_reject',args:{
    ticketId:'demand-1',expectedRevision:2,reason:'Native test evidence is missing',
  }}]);
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
