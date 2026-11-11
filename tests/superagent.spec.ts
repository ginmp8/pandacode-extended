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
  const dialog=page.getByRole('dialog',{name:'Orchestrations'});
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
  await expect(dialog.getByRole('button',{name:'Enable manual'})).toBeVisible();
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await dialog.getByRole('button',{name:'Export'}).click();
  const text=dialog.getByRole('textbox',{name:'Companion manifest JSON'});
  await expect(text).toContainText('"schemaVersion": 1');
  await dialog.getByRole('button',{name:'Preview import'}).click();
  await expect(dialog.getByText(/0 new; 1 unchanged/)).toBeVisible();
  await dialog.getByRole('button',{name:'Confirm import'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
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
  const dialog=page.getByRole('dialog',{name:'Orchestrations'});
  await dialog.getByRole('textbox',{name:'Name'}).fill('Issue Resolution');
  await dialog.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await dialog.getByRole('combobox',{name:'Worker'}).selectOption({label:'Writer'});
  await dialog.getByRole('button',{name:'Create disabled preset'}).click();
  await expect(dialog.getByRole('button',{name:'Dispatch manually'})).toBeDisabled();
  await dialog.getByRole('button',{name:'Enable manual'}).click();
  await dialog.getByRole('button',{name:'Confirm',exact:true}).click();
  await expect(dialog.getByRole('button',{name:'Disable'})).toBeVisible();
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
  const dialog=page.getByRole('dialog',{name:'Orchestrations'});
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
