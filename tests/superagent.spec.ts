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
  await expect(dialog.locator('.sa-list .sa-item strong')).toHaveText('Issue Resolution');
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
  await expect(host.getByText('Explicit authenticated gh Events polling',{exact:false})).toBeVisible();
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
  await expect(host.getByRole('status')).toContainText('no task or authorization created');
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
        return {scanned:3,recorded:1,duplicates:2} as never;
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
  await expect(host.getByRole('status').filter({hasText:'3 scanned, 1 recorded, 2 duplicate'})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {saPollCalls:string[]}).saPollCalls);
  expect(calls.filter(c=>c==='superagent_source_poll_authenticated')).toHaveLength(1);
  expect(calls.filter(c=>c==='superagent_submit'||c==='superagent_dispatch_worker')).toHaveLength(0);
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
  await expect(host.getByText('Policy revision 2',{exact:false})).toBeVisible();
  const calls=await page.evaluate(()=>(window as unknown as {saRosterCalls:string[]}).saRosterCalls);
  expect(calls.filter(value=>value==='superagent_roster_update')).toHaveLength(1);
  expect(calls.filter(value=>value==='superagent_submit' || value==='superagent_set_enabled')).toHaveLength(0);
});
