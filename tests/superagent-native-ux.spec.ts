import { test, expect, type Page } from '@playwright/test';

async function openNativeOrchestrations(page: Page) {
  await page.goto('/?bots=sample');
  await page.evaluate(async () => { await import('/src/preview-superagent.ts'); });
  const bar = page.locator('.sidebar-bots-bar');
  if (await bar.getAttribute('aria-pressed') !== 'true') await bar.click();
  const bots = page.getByRole('region', { name:'Bots', exact:true });
  await bots.getByRole('button', { name:'Bot actions' }).click();
  await bots.getByRole('menuitem', { name:'Orchestrations' }).click();
  const host = page.locator('#superagent-workspace #superagent-host');
  await expect(host).toHaveAttribute('aria-busy', 'false', { timeout:30_000 });
  return host;
}

test('native Orchestrations first use prioritizes one guided Team creation path', async ({ page }) => {
  test.setTimeout(60_000);
  const host = await openNativeOrchestrations(page);
  await expect(host.getByRole('heading', {
    name:'Your first SuperAgent starts with a Team',
  })).toBeVisible();
  await expect(host.getByRole('navigation', {name:'Orchestration views'})).toBeVisible();
  await expect(host.locator('details.sa-legacy-create')).not.toHaveAttribute('open', '');
  await expect(host.getByRole('complementary', {name:'Orchestrations navigation'})).toBeVisible();
  await page.screenshot({path:'test-results/superagent-native-first-use.png'});
  await host.locator('.sa-native-header [data-sa-builder-open]').click();
  const builder = host.getByRole('region', { name:'Guided Team Builder' });
  await expect(builder).toContainText('Step 1 of 5');
  await expect(builder.locator('.sa-builder-step-list li')).toHaveCount(5);
  await builder.getByRole('textbox', { name:'Team name' }).fill('Native UX Team');
  await builder.getByRole('button', { name:'Next' }).click();
  await builder.getByRole('combobox', { name:'Coordinator' }).selectOption({label:'Scout'});
  await builder.getByRole('combobox', { name:'Primary Worker' }).selectOption({label:'Writer'});
  await builder.getByRole('button', { name:'Next' }).click();
  await builder.getByRole('button', { name:'Next' }).click();
  await builder.getByRole('button', { name:'Next' }).click();
  await expect(builder).toContainText('Step 5 of 5');
  await builder.getByRole('button', { name:'Create Team (disabled)' }).click();

  const overview = host.getByRole('region', { name:'Team overview' });
  await expect(overview).toBeVisible({timeout:20_000});
  await expect(overview).toContainText('Native UX Team');
  await page.screenshot({path:'test-results/superagent-native-team-overview.png'});
  await expect(overview).toContainText('Coordinator');
  await expect(overview).toContainText('Worker 1');
  await expect(overview).toContainText('task needs native admission');
  await expect(host.getByRole('button', { name:'Enable manual' })).toBeVisible();
  await expect(host.getByRole('button', { name:'Demands / Runs' }))
    .toHaveAttribute('aria-current', 'false');
});

test('native team member inspector and global views use existing read-only data', async ({page}) => {
  test.setTimeout(60_000);
  const host = await openNativeOrchestrations(page);
  await host.locator('.sa-native-header [data-sa-builder-open]').click();
  const builder = host.getByRole('region', {name:'Guided Team Builder'});
  await builder.getByRole('textbox',{name:'Team name'}).fill('Review setup');
  await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('combobox',{name:'Coordinator'}).selectOption({label:'Scout'});
  await builder.getByRole('combobox',{name:'Primary Worker'}).selectOption({label:'Writer'});
  for(let step=0;step<3;step++)await builder.getByRole('button',{name:'Next'}).click();
  await builder.getByRole('button',{name:'Create Team (disabled)'}).click();
  await expect(host.getByRole('region',{name:'Team overview'})).toBeVisible({timeout:20_000});
  const members = host.locator('button[data-sa-member-open]');
  await expect(members).toHaveCount(2);
  await members.nth(1).click();
  const inspector = host.getByRole('region',{name:'Member profile'});
  await expect(inspector).toContainText('Writer');
  await expect(inspector).toContainText('Skills');
  await expect(members.nth(1)).toHaveAttribute('aria-pressed','true');

  await host.getByRole('button',{name:'Members and Skills'}).click();
  await expect(host.getByRole('region',{name:'Configured Team roles'})).toBeVisible();
  await host.getByRole('button',{name:'Execution policy'}).click();
  await expect(host.getByRole('region',{name:'Team runtime strategy'})).toBeVisible();
  await host.getByRole('button',{name:'Demands / Runs'}).click();
  await expect(host.getByRole('region',{name:'All Demands across Teams'})).toBeVisible();
  await page.screenshot({path:'test-results/superagent-native-demand-board.png'});
  await host.getByRole('button',{name:'Review inbox'}).click();
  await expect(host.getByRole('region',{name:'Cross-Team native review inbox'})).toBeVisible();
  await host.getByRole('button',{name:'Sources'}).click();
  await expect(host).toContainText('GitHub');
  await host.getByRole('button',{name:'Portable configuration'}).click();
  await expect(host.locator('textarea[data-sa-manifest]')).toBeVisible();
});
