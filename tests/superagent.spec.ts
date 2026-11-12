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
      if(command==='superagent_source_thread_activity')return [{
        sourceId:'source-one',deliveryId:'verified-501',eventKind:'issues.opened',
        subjectKey:'issue:42',decision:'observed_permitted',sequence:501,
        policyRevision:1,at:Date.now()
      }] as never;
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
  await expect(host.getByRole('status')).toContainText('Enter a positive, bounded Check Run ID');
  await host.getByRole('textbox',{name:'Check Run ID'}).fill('240');
  await host.getByRole('button',{name:'Observe authenticated Check Run'}).click();
  await expect(host.getByRole('status')).toContainText('observed_permitted');
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

test('multi-member Team Builder stores native role revisions without launching unapproved work',async({page})=>{
  test.setTimeout(60_000);
  await page.goto('/?bots=sample');
  await page.evaluate(async()=>{
    const {mountSuperagent}=await import('/src/superagent-ui.ts');
    const host=document.createElement('div');host.id='sa-configured-roster';document.body.appendChild(host);
    const calls:string[]=[];(window as unknown as {saRosterCalls:string[]}).saRosterCalls=calls;
    const config={id:'team',name:'Security-critical Team',coordinatorBotId:'coordinator',
      workerBotId:'worker',policyRevision:1,enabled:false,ticketCount:0};
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
      if(command==='superagent_tickets')return [] as never;
      if(command==='superagent_roster')return {...roster} as never;
      if(command==='superagent_roster_update') {
        if(args?.expectedRevision!==config.policyRevision)throw Error('Native policy revision mismatch');
        const input=args?.input as {workerBotIds:string[];reviewerBotIds:string[]};
        if(input.workerBotIds.length!==3||input.reviewerBotIds.length!==2)throw Error('Role validation failed');
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
  await form.locator('select[name="reviewers"]').selectOption(['review-a','review-b']);
  await form.getByRole('button',{name:'Save configured roles'}).click();
  await expect(host.getByText('Multi-member execution is disabled',{exact:false})).toBeVisible();
  await expect(host.getByRole('region',{name:'Configured Team roles'}))
    .toContainText('Policy revision 2');
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
        pages.push({...args});
        if(args?.beforeId==='demand-199'&&args?.beforeUpdatedAt===1801)
          return [makeDemand(200),makeDemand(201)] as never;
        if(!args?.beforeId)return Array.from({length:200},(_,index)=>makeDemand(index)) as never;
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
