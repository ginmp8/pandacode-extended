import { test, expect, type Page } from '@playwright/test';

async function openOrchestrationWorkspace(page: Page) {
  await page.evaluate(async () => { await import('/src/preview-superagent.ts'); });
  const bar=page.locator('.sidebar-bots-bar');
  if(await bar.getAttribute('aria-pressed')!=='true')await bar.click();
  const sidebar=page.getByRole('region',{name:'Bots',exact:true});
  await expect(sidebar).toBeVisible();
  await sidebar.getByRole('button',{name:'Bot actions'}).click();
  await sidebar.getByRole('menuitem',{name:'Orchestrations'}).click();
  const workspace=page.locator('#superagent-workspace');
  await expect(workspace).toBeVisible();
  await expect(page.getByRole('region',{name:'Orchestrations',exact:true})).toBeVisible();
  await expect(page.getByRole('dialog',{name:'Orchestrations',exact:true})).toHaveCount(0);
  await expect(page.locator('#dialog[open]')).toHaveCount(0);
  await expect(page.locator('.dialog-scrim:visible')).toHaveCount(0);
  const host=workspace.locator('#superagent-host');
  await expect(host).toHaveAttribute('aria-busy','false',{timeout:30_000});
  return host;
}


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
  await dialog.getByRole('textbox',{name:'Name',exact:true}).fill('Software Development');
  const coordinator=dialog.getByRole('combobox',{name:'Coordinator'});
  const worker=dialog.getByRole('combobox',{name:'Worker'});
  await coordinator.selectOption({label:'Scout'});
  await worker.selectOption({label:'Writer'});
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
  await expect(text).toContainText('"schemaVersion": 5');
  await dialog.getByRole('button',{name:'Preview import'}).click();
  await expect(dialog.getByText(/0 new; 1 unchanged/)).toBeVisible();
  await dialog.getByRole('button',{name:'Confirm import'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(dialog.getByRole('status').filter({hasText:'0 presets imported; 1 unchanged.'})).toBeVisible();
  await dialog.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
});



test('duplicating a native Team retains its roles but never its saved runtime authority', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  const create=host.locator('form[data-sa-form="create"]');
  await create.getByRole('textbox',{name:'Name',exact:true}).fill('Source Team');
  await create.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await create.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await create.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(host.locator('.sa-list .sa-item')).toHaveCount(1);

  await host.getByRole('button',{name:'Duplicate as new Team'}).click();
  const prompt=host.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(prompt).toContainText('Tickets, source subscriptions, permissions and approvals are not copied');
  await prompt.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.locator('.sa-list .sa-item')).toHaveCount(2);
  await expect(host.locator('.sa-list')).toContainText('Source Team copy');
  await expect(host.getByRole('status').filter({hasText:'Disabled Team duplicated.'})).toBeVisible();
  await expect(host.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(host.locator('.sa-list .sa-counts')).toHaveText([
    '0 tasks · 0 running · 0 blocked',
    '0 tasks · 0 running · 0 blocked',
  ]);

  await host.getByRole('button',{name:'Load native members'}).click();
  await expect(host.getByRole('region',{name:'Configured Team roles'}))
    .toContainText('Scout');
  await expect(host.getByRole('region',{name:'Configured Team roles'}))
    .toContainText('Writer');
});

test('versioned Team strategies keep unsupported budgets visible and prevent silent enable', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  const create=host.locator('form[data-sa-form="create"]');
  await create.getByRole('textbox',{name:'Name',exact:true}).fill('Economy Strategy');
  await create.getByRole('combobox',{name:'Coordinator',exact:true}).selectOption({label:'Scout'});
  await create.getByRole('combobox',{name:'Worker',exact:true}).selectOption({label:'Writer'});
  await create.getByRole('button',{name:'Create disabled preset'}).click();
  const strategy=host.getByRole('region',{name:'Team runtime strategy'});
  await strategy.getByRole('button',{name:'Inspect native policy'}).click();
  await expect(strategy).toContainText('Native executable parallel Workers: 1');
  await expect(strategy).toContainText('Real token metering: Not supported');
  const form=strategy.locator('form[data-sa-form="strategy-update"]');
  await form.getByRole('combobox',{name:'Optimization priority'}).selectOption('cost');
  await form.getByRole('spinbutton',{name:'Maximum concurrent tasks (1–8)'}).fill('2');
  await form.getByRole('spinbutton',{name:'Maximum attempts per Worker (1–8)'}).fill('3');
  await form.getByRole('spinbutton',{name:'Attempt timeout (milliseconds, 60000–1800000)'}).fill('300000');
  await form.getByRole('button',{name:'Save future-Team policy'}).click();
  await expect(host.getByRole('status').filter({hasText:'Strategy saved for future Tickets.'})).toBeVisible();
  await strategy.getByRole('button',{name:'Inspect native policy'}).click();
  await expect(strategy).toContainText('Team version 2');
  await expect(strategy).toContainText('Parallel worker execution has no admitted isolated scheduler yet.');
  await expect(form.getByRole('combobox',{name:'Optimization priority'})).toHaveValue('cost');
  await expect(form.getByRole('spinbutton',{name:'Maximum attempts per Worker (1–8)'})).toHaveValue('3');
  await host.getByRole('button',{name:'Enable manual'}).click();
  await host.getByRole('alertdialog',{name:'Confirm orchestration action'})
    .getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('alert')).toContainText('Requested Team policy cannot run');
  await expect(host.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await host.getByRole('button',{name:'Portable configuration'}).click();
  await host.getByRole('button',{name:'Export'}).click();
  const portable = host.getByRole('textbox',{name:'Companion manifest JSON'});
  await expect(portable).toContainText('"schemaVersion": 5');
  await expect(portable).toContainText('"priority": "cost"');
  await expect(portable).toContainText('"maxConcurrentTasks": 2');
  await expect(portable).toContainText('"attemptTimeoutMs": 300000');
  await host.getByRole('button',{name:'Preview import'}).click();
  await expect(host.getByText(/0 new; 1 unchanged/)).toBeVisible();
});

test('GitHub Auto requires explicit bounded source consent and never grants writes', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const existing=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');
    host.id='superagent-host';existing.replaceWith(host);
    let revision=0;
    let mode:'off'|'staging'|'active'='off';
    const commands:string[]=[];
    const source={id:'source-1',orchestrationId:'team-1',repository:'owner/repo',
      actorFilter:'trusted-bot',eventKind:'issues.opened',
      mode:'observe',policyRevision:2,lastSequence:0};
    const policy=()=>({sourceId:source.id,mode,policyRevision:revision,
      sourcePolicyRevision:source.policyRevision,maxEventsPerHour:4,maxEventsPerDay:20,
      activationAt:0,staged:0,dispatched:0,uncertain:0,
      autoWritesAllowed:false,pollingGapFree:false});
    mountSuperagent(host,async(command,args) => {
      commands.push(command);
      if(command==='bots_list')return [
        {id:'lead',name:'Coordinator',paused:false,projectId:'project'},
        {id:'worker',name:'Worker',paused:false,projectId:'project'},
      ] as never;
      if(command==='superagent_list')return [{
        id:'team-1',name:'Review Team',coordinatorBotId:'lead',workerBotId:'worker',
        enabled:true,policyRevision:1,projectId:'project',
        workerCount:1,reviewerCount:0,
      }] as never;
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_source_list')return [source] as never;
      if(command==='superagent_auto_policy_get')return policy() as never;
      if(command==='superagent_auto_policy_update'){
        if(args?.sourceId!==source.id || args?.expectedRevision!==revision)
          throw Error('Source revision changed.');
        const input=args?.input as {mode:'off'|'staging'|'active'};
        mode=input.mode;revision++;
        return policy() as never;
      }
      throw Error('Unexpected privileged command: '+command);
    });
    Object.assign(window,{autoCommandLog:()=>commands.slice()});
  });
  const host=page.locator('#superagent-host');
  await expect(host).toHaveAttribute('aria-busy','false');
  await host.getByRole('button',{name:'Sources',exact:true}).click();
  await host.getByRole('button',{name:'Load sources'}).click();
  await host.getByRole('button',{name:'Configure GitHub Auto'}).click();
  const auto=host.getByRole('region',{name:'GitHub Auto source consent'});
  await expect(auto).toContainText('Mode off');
  await expect(auto).toContainText('Bounded authenticated polling is not gap-free');
  const form=auto.locator('form[data-sa-form="auto-policy"]');
  await form.getByRole('combobox',{name:'Native Auto mode'}).selectOption('staging');
  await form.getByRole('spinbutton',{name:'Maximum events per hour (1–20)'}).fill('3');
  await form.getByRole('spinbutton',{name:'Maximum events per day (1–100)'}).fill('15');
  await form.getByRole('button',{name:'Save native Auto consent'}).click();
  const approval=host.getByRole('alertdialog',{name:'Confirm orchestration action'});
  await expect(approval).toContainText('without launching a Worker');
  await approval.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(host.getByRole('status').filter({hasText:'GitHub Auto staging saved'})).toBeVisible();
  await expect(auto).toContainText('Mode staging');
  const commands=await page.evaluate(()=>
    (window as unknown as {autoCommandLog:()=>string[]}).autoCommandLog());
  expect(commands).toContain('superagent_auto_policy_update');
  expect(commands).not.toContain('superagent_submit');
  expect(commands).not.toContain('superagent_issue_comment');
  expect(commands).not.toContain('superagent_dispatch_worker');
});

test('guided SuperAgent Team Builder validates five native-backed stages and saves Off', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  const host=await openOrchestrationWorkspace(page);
  await host.getByRole('button',{name:'Guided Team setup (five steps)'}).click();
  const builder=host.getByRole('region',{name:'Guided Team Builder'});
  await expect(builder).toContainText('Step 1 of 5');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder.getByRole('alert')).toContainText('Choose a Team name');
  await builder.getByRole('textbox',{name:'Team name'}).fill('Team via guided setup');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Step 2 of 5');
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.getByRole('combobox',{name:'Primary Worker'}).selectOption({label:'Writer'});
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Step 3 of 5');
  await expect(builder).toContainText('Skill presence alone does not prove');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Step 4 of 5');
  await expect(builder).toContainText('Saved execution mode:');
  await builder.getByRole('button',{name:'Next'}).click();
  await expect(builder).toContainText('Step 5 of 5');
  await expect(builder).toContainText('no new task or tool grants');
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  await expect(host.locator('.sa-list .sa-item strong'))
    .toHaveText('Team via guided setup',{timeout:15_000});
  await expect(host.getByRole('region',{name:'Guided Team Builder'})).toHaveCount(0);
  await expect(host.getByRole('button',{name:'Enable manual'})).toBeVisible();
});

test('guided Team Builder scopes Workers to the primary checkout without restricting reviewers', async ({page}) => {
  await page.goto('/?bots=sample');
  await openOrchestrationWorkspace(page);
  await page.evaluate(async () => {
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const previous=document.querySelector<HTMLElement>('#superagent-host')!;
    const host=document.createElement('div');
    host.id='superagent-host'; previous.replaceWith(host);
    const bots=[
      {id:'coordinator',name:'Coordinator',paused:false,projectId:'coord-project'},
      {id:'worker-one',name:'Primary Worker',paused:false,projectId:'project-one'},
      {id:'worker-two',name:'Same Project Worker',paused:false,projectId:'project-one'},
      {id:'foreign-worker',name:'Different Project Worker',paused:false,projectId:'project-two'},
      {id:'independent-reviewer',name:'Independent Reviewer',paused:false,projectId:'council-project'},
    ];
    mountSuperagent(host, async(command) => {
      if(command==='bots_list') return bots as never;
      if(command==='superagent_list') return [] as never;
      if(command==='superagent_tickets') return [] as never;
      throw Error('Unexpected native command: '+command);
    });
  });
  const host=page.locator('#superagent-host');
  await expect(host).toHaveAttribute('aria-busy','false');
  await host.getByRole('button',{name:'Guided Team setup (five steps)'}).click();
  const builder=host.getByRole('region',{name:'Guided Team Builder'});
  await builder.getByRole('textbox',{name:'Team name'}).fill('Project scoped Team');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption('coordinator');
  await builder.getByRole('combobox',{name:'Primary Worker'}).selectOption('worker-one');
  const workers=builder.locator('select[name="builderWorkers"]');
  await expect(workers.locator('option[value="foreign-worker"]')).toBeDisabled();
  await expect(workers.locator('option[value="worker-two"]')).toBeEnabled();
  const reviewers=builder.locator('select[name="builderReviewers"]');
  await expect(reviewers.locator('option[value="independent-reviewer"]')).toBeEnabled();
  await expect(builder.getByRole('combobox',{name:'Coordinator'})).toHaveValue('coordinator');
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
  await dialog.getByRole('textbox',{name:'Name',exact:true}).fill('Issue Resolution');
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
  await dialog.getByRole('button',{name:'Dispatch manually'}).click();
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
  await expect(details.getByRole('heading',{name:'Native activity',exact:true}).locator('xpath=following-sibling::ol[1] li').first()).toContainText(' · admitted · ');
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
  await dialog.getByRole('textbox',{name:'Name',exact:true}).fill('Safe default');
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
  await host.getByRole('textbox',{name:'Repository (owner/name)'}).fill('test/fixture');
  await host.getByRole('textbox',{name:'Exact actor'}).fill('trusted-bot');
  await host.getByRole('button',{name:'Add source (Off)'}).click();
  await expect(host.locator('.sa-source-list')).toContainText('test/fixture');
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
    const ticket={id:'ticket-one',objective:'Private fixture',state:'queued',revision:1,evidence:null};
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
    const ticket={id:'audit-ticket',objective:'Review bounded evidence',state:'waiting_approval',revision:3,evidence:'evidence'};
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
          input.reviewerBotIds.length!==2)throw Error('Role order validation failed');
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
  await form.locator('select[name="additionalWorkers"]').selectOption(['worker-b','worker-c']);
  await form.getByRole('button',{name:'Move Worker C up'}).click();
  await expect(form.locator('.sa-worker-order li').first()).toContainText('Worker C');
  await form.locator('select[name="reviewers"]').selectOption(['review-a','review-b']);
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
        return {ticket:run,mandate:{actor:'local-user',authorizationId:'native',
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
          return [demand('a-0','team-a',1900),demand('b-0','team-b',2000)] as never;
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
          demand('match-a','team-a','Needle hidden past page 200'),
          demand('match-b','team-b','NEEDLE in another Team'),
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
  await host.getByRole('button',{name:'Review native preflight'}).click();
  const panel = host.getByRole('region',{name:'Native demand preflight'});
  await expect(panel).toContainText('Eligible for manual read-only admission now.');
  await expect(panel).toContainText('Router');
  await expect(panel).toContainText('isolated-project / 2');
  await expect(panel).toContainText('none / no side effects');
  await expect(panel).toContainText('not a grant, Ticket or guaranteed reservation');
  await expect(host.getByRole('textbox',{name:'Task objective'})).toHaveValue('Inspect changes');
  const calls = await page.evaluate(() =>
    (window as unknown as {saPreflightCalls:()=>string[]}).saPreflightCalls());
  expect(calls.filter(name=>name==='superagent_preflight')).toHaveLength(1);
  expect(calls.filter(name=>name.includes('submit')||name.includes('dispatch'))).toHaveLength(0);
  await host.getByRole('textbox',{name:'Task objective'}).fill('Changed objective');
  await expect(panel).toHaveCount(0);
});

test('native-backed attempt inspector shows recorded sessions and run budgets without granting effects', async ({page}) => {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => {
    const {mountSuperagent} = await import('/src/superagent-ui.ts');
    const host=document.createElement('div');
    host.id='sa-native-attempt-test'; document.body.appendChild(host);
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
        state:'waiting_approval',revision:4,updatedAt:1794400000000},
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
        grantsIssued:false,members:[
          {role:'coordinator',position:0,botId:'lead',name:'Lead',provider:'codex',
            model:'gpt-main',mode:'plan',projectId:'p',paused:false,skills:[],configurationIssues:[]},
          {role:'worker',position:0,botId:'worker',name:'Worker',provider:'claude',
            model:'sonnet',mode:'plan',projectId:'p',paused:false,skills:[
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
  await expect(view).toContainText('Skill quality · Library installed · Safety clean');
  await expect(view).toContainText('Skill removed · Library missing');
  await expect(view.getByText('Assigned Skill removed is missing: install or unassign it in Skills Manager.')).toBeVisible();
  await expect(view).toContainText('No local configuration issues detected. Harness/tool eligibility unverified.');
  await expect(view).toContainText('Harness eligibility not_verified');
  await expect(view).toContainText('Multi-role execution unavailable');
  await expect(view).toContainText('No task grant issued');
  const calls=await page.evaluate(()=>
    (window as unknown as {effectiveCalls:()=>string[]}).effectiveCalls());
  expect(calls.filter(c=>c==='superagent_effective_team')).toHaveLength(1);
  expect(calls.some(c=>/submit|dispatch|accept|reserve|enable|activate/.test(c))).toBe(false);
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
  await host.getByRole('button',{name:'Demands / Runs'}).click();
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
  await host.getByRole('button',{name:'Demands / Runs'}).click();
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


