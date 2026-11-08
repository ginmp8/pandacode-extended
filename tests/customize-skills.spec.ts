import { expect, test, type Page } from '@playwright/test';

/* Isolate Customize from the chat runtime while exercising the same browser preview library. */
async function openCustomize(page: Page): Promise<void> {
  await page.route('**/customize-skills-harness.html', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html lang="en"><head><link rel="stylesheet" href="/src/styles.css"><link rel="stylesheet" href="/src/design-system.css"></head>
      <body><div id="host"></div><script type="module">
        import { mountCustomizeSkills } from '/src/customize-skills.ts';
        import { createPreviewSkills } from '/src/preview-skills.ts';
        const skills = createPreviewSkills();
        window.__skills = skills;
        window.__closeCount = 0;
        mountCustomizeSkills(document.getElementById('host'), {
          call: async (command, args = {}) => skills.call(command, args),
          subscribe: async () => () => {},
          chooseZip: async () => '/Users/you/Downloads/skill.zip',
          close: () => window.__closeCount++,
        });
      </script></body></html>`,
  }));
  await page.goto('/customize-skills-harness.html');
  await expect(page.getByRole('heading', {name:'Customize'})).toBeVisible();
  await expect(page.getByRole('heading', {name:'Skills'})).toBeVisible();
}

test('Customize navigation remains separate from the Bots bar', async ({page}) => {
  await page.goto('/');
  const customize = page.getByRole('button', {name:'Customize', exact:true});
  await expect(customize).toBeVisible();
  await expect(page.locator('.sidebar-customize + #sidebar-bots-panel + #sidebar-bots')).toHaveCount(1);
  await customize.click();
  await expect(page.getByRole('heading', {name:'Customize',exact:true})).toBeVisible();
  await page.getByRole('button', {name:'Back to workspace'}).click();
  await expect(page.locator('#customize-hub')).toBeHidden();
});

test('GitHub Add has only URL and optional ref, then imports only selected skills', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'GitHub link'}).click();
  const form = page.locator('#customize-github-form');
  await expect(form.locator('input')).toHaveCount(2);
  await form.getByRole('textbox', {name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await form.getByRole('button', {name:'Scan repository'}).click();
  await expect(page.getByRole('region',{name:'Discovered GitHub skills'})).toContainText('Found 3 skills');
  await expect(page.getByRole('button', {name:'Import selected (0)'})).toBeDisabled();
  await page.getByRole('region', {name:'Discovered GitHub skills'}).getByRole('checkbox').nth(1).check();
  await expect(page.getByRole('button', {name:'Import selected (1)'})).toBeEnabled();
  await page.getByRole('button', {name:'Import selected (1)'}).click();
  await expect(page.getByRole('region', {name:'Skill operation status'})).toContainText('Completed successfully');
  await expect(page.getByRole('region', {name:'Skill operation status'})).toContainText('1 of 1 processed');
  await page.getByRole('button', {name:'GitHub sources'}).click();
  await expect(page.getByText('you/agent-skills',{exact:true}).first()).toBeVisible();
});

test('ZIP import reports replacement without changing harness activation', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'ZIP archive'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
  await expect(page.getByText('ZIP sample',{exact:true}).first()).toBeVisible();
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'ZIP archive'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Replaced previous central library source');
});

test('Preview is read-only, supports raw text and local packaged files', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button', {name:'Inspect Release notes'}).click();
  const detail = page.getByRole('region',{name:'Read-only skill preview'});
  await expect(detail).toContainText('INSTALLED PACKAGE');
  await expect(detail.getByRole('button',{name:'SKILL.md'})).toBeVisible();
  await detail.getByRole('button',{name:'Raw'}).click();
  await expect(detail.locator('#customize-preview-content pre')).toBeVisible();
  await detail.getByRole('button',{name:'Rendered'}).click();
  await expect(detail.locator('#customize-preview-content')).toContainText('Release notes');
});

test('a conflicting installed skill requires explicit replacement approval', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'GitHub link'}).click();
  await page.getByRole('textbox', {name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button', {name:'Scan repository'}).click();
  const found = page.getByRole('region', {name:'Discovered GitHub skills'});
  await found.getByRole('checkbox').first().check();
  await expect(found.getByRole('button', {name:'Import selected (1)'})).toBeDisabled();
  await found.getByRole('checkbox', {name:'I approve replacing conflicting installed skills'}).check();
  await expect(found.getByRole('button', {name:'Import selected (1)'})).toBeEnabled();
});
