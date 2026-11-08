import { expect, test, type Page } from '@playwright/test';

/* Settings → Skills & rules, mounted on its own page over the preview library
 * (src/preview-skills.ts), so it runs whether or not Settings has wired the section yet. */
async function open(page: Page): Promise<void> {
  await page.route('**/skills-library-harness.html', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html><head><link rel="stylesheet" href="/src/styles.css"></head><body><main id="host" class="settings-page-root"></main>
      <script type="module">
        import { mountSkillsLibrary } from '/src/skills-library.ts';
        import { createPreviewSkills } from '/src/preview-skills.ts';
        const skills = createPreviewSkills();
        window.__skills = skills;
        const call = async (command, args = {}) => skills.call(command, args);
        mountSkillsLibrary(document.getElementById('host'), { call, projects: () => [{ id: 'orbit', name: 'Orbit' }] });
      </script></body></html>`,
  }));
  await page.goto('/skills-library-harness.html');
  await expect(page.getByRole('list', { name: 'Library' })).toBeVisible();
}
/** The real Settings sidebar, content column and card tokens (not the isolated import harness). */
async function openWithinSettings(page: Page): Promise<void> {
  await page.route('**/skills-layout-harness.html', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html data-theme="dark"><head>
      <link rel="stylesheet" href="/src/styles.css">
      <link rel="stylesheet" href="/src/design-system.css">
      <link rel="stylesheet" href="/src/settings.css">
    </head><body><div class="settings-page-root" style="--sidebar-width:220px;min-height:100vh">
      <aside class="settings-nav"></aside>
      <div class="settings-content"><section class="settings-panel settings-panel-paged">
        <div class="settings-panel-body"><div id="skills-host"></div></div>
      </section></div>
    </div><script type="module">
      import { mountSkillsLibrary } from '/src/skills-library.ts';
      import { createPreviewSkills } from '/src/preview-skills.ts';
      const preview = createPreviewSkills();
      mountSkillsLibrary(document.getElementById('skills-host'), { call: (command, args = {}) => preview.call(command, args), projects: () => [] });
    </script></body></html>`,
  }));
  await page.goto('/skills-layout-harness.html');
  await expect(page.getByRole('navigation', { name: 'Skills Library pages' })).toBeVisible();
}

const item = (page: Page, name: string) => page.getByRole('list', { name: 'Library' }).locator('.skills-item', { hasText: name });
const status = (page: Page, row: string, column: number) => page.getByRole('table', { name: 'Skills by harness' }).getByRole('row', { name: new RegExp(row) }).locator('td').nth(column);

test.describe('Skills & rules', () => {
for (const theme of ['light', 'dark']) {
  test(`import controls stay readable and full height in ${theme}`, async ({ page }) => {
    await open(page);
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const form = page.getByRole('form', { name: 'Import a skill or rule' });
    const path = form.getByRole('textbox', { name: 'Folder or file path' });
    expect.soft((await path.boundingBox())!.height).toBeGreaterThanOrEqual(32);
    await expect(form.getByRole('combobox')).toHaveCount(3);
    for (const select of await form.getByRole('combobox').all()) {
      const colors = await select.locator('option').evaluateAll(options => options.map(option => {
        const probe = document.createElement('span');
        probe.style.cssText = 'background:var(--panel);color:var(--text)';
        option.closest('form')!.append(probe);
        const expected = getComputedStyle(probe);
        const actual = getComputedStyle(option);
        const colors = { background: actual.backgroundColor, text: actual.color, panel: expected.backgroundColor, foreground: expected.color };
        probe.remove();
        return colors;
      }));
      for (const color of colors) {
        expect(color.background).toBe(color.panel);
        expect(color.text).toBe(color.foreground);
      }
    }
    await path.fill('/path/to/skill-folder');
    await form.screenshot({ path: `test-results/skills-import-${theme}.png` });
    await form.getByRole('combobox', { name: 'Kind', exact: true }).selectOption('rule');
    await expect(form.getByRole('combobox', { name: 'Kind', exact: true })).toHaveValue('rule');
    await form.getByRole('combobox', { name: 'Source', exact: true }).selectOption('github');
    for (const input of await form.getByRole('textbox').all()) {
      expect((await input.boundingBox())!.height).toBeGreaterThanOrEqual(32);
    }
    await form.getByRole('combobox', { name: 'Project for project scope' }).selectOption('orbit');
    await expect(form.getByRole('combobox', { name: 'Project for project scope' })).toHaveValue('orbit');
  });
}

test('activating a skill for Claude Code and Codex shows in the matrix, and drift is reported', async ({ page }) => {
  await open(page);
  await page.getByRole('checkbox', { name: 'Release notes: Claude Code · User' }).check();
  await expect(page.getByRole('status')).toHaveText('Active for Claude Code.');
  await page.getByRole('checkbox', { name: 'Release notes: Codex · User' }).check();
  await expect(status(page, 'Release notes', 0)).toHaveText('Active');
  await expect(status(page, 'Release notes', 1)).toHaveText('Active');
  await expect(page.locator('.skills-unmanaged')).toContainText('~/.claude/skills/pdf-tools');

  await page.evaluate(() => (window as any).__skills.edit('release-notes', 'claude', 'user'));
  await page.getByRole('combobox', { name: 'Project for project scope' }).selectOption('orbit');
  await expect(status(page, 'Release notes', 0)).toHaveText('Edited on disk');
  await page.getByRole('checkbox', { name: 'Release notes: Claude Code · User' }).uncheck();
  await expect(page.getByRole('status')).toContainText('Left 1 edited file in place');
  await expect(page.getByRole('button', { name: 'Remove Release notes' })).toBeEnabled();
  await page.getByRole('button', { name: 'Remove Release notes' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Turn Release notes off everywhere' })).toBeVisible();
});

test('project-scope rules and safety findings need acknowledgement before activation', async ({ page }) => {
  await open(page);
  await page.getByRole('combobox', { name: 'Project for project scope' }).selectOption('orbit');
  await page.getByRole('checkbox', { name: 'No force push: Codex · Project' }).check();
  await expect(page.getByRole('status')).toHaveText('Active for Codex.');

  await page.getByRole('textbox', { name: 'Folder or file path' }).fill('/Users/you/skills/setup');
  await page.getByRole('button', { name: 'Import' }).click();
  await expect(page.getByRole('status')).toHaveText('Imported setup. 1 safety finding to review.');
  await expect(page.getByRole('list', { name: 'Safety findings for setup' })).toContainText('Download piped into a shell');
  const toggle = page.getByRole('checkbox', { name: 'setup: Claude Code · User' });
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await item(page, 'setup').getByRole('button', { name: 'Activate anyway' }).click();
  await expect(page.getByRole('checkbox', { name: 'setup: Claude Code · User' })).toBeChecked();
});

test('GitHub import validates the repository and records the commit', async ({ page }) => {
  await open(page);
  await page.getByRole('combobox', { name: 'Source' }).selectOption('github');
  await page.getByRole('button', { name: 'Import' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'owner/name' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Repository', exact: true }).fill('acme/skills');
  await page.getByRole('textbox', { name: 'Path in repository' }).fill('skills/pdf');
  await page.getByRole('button', { name: 'Import' }).click();
  await expect(item(page, 'pdf')).toContainText('GitHub · acme/skills/skills/pdf @ a1b2c3d');
  await expect(page.getByRole('textbox', { name: 'Repository', exact: true })).toHaveValue('');
});

test('discovery registers only skill repositories, shows name conflicts, and importing never activates', async ({ page }) => {
  await open(page);
  await page.evaluate(() => (window as any).__skills.failNext('GitHub\'s rate limit was reached. Try again later.'));
  await page.getByRole('button', { name: 'Find my skill repositories' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'rate limit' })).toBeVisible();

  await page.getByRole('button', { name: 'Find my skill repositories' }).click();
  await expect(page.getByRole('status')).toHaveText('Added you/agent-skills.');
  const sources = page.getByRole('list', { name: 'Catalog sources' });
  await expect(sources.locator('.skills-catalog')).toHaveCount(1);
  await expect(sources).not.toContainText('you/skills-wishlist');
  const source = page.locator('.skills-catalog[data-repo="you/agent-skills"]');
  await expect(source).toContainText('Discovered');
  await expect(source).toContainText('Private');
  await expect(source.locator('.skills-catalog-skill', { hasText: 'Release notes' })).toContainText('Same name in anthropics/skills/release-notes');

  await page.getByRole('checkbox', { name: 'Select Changelog from you/agent-skills' }).check();
  await page.getByRole('button', { name: 'Import selected from you/agent-skills' }).click();
  await expect(page.getByRole('status')).toHaveText('Imported 1 skill. Nothing is active until you turn it on.');
  await expect(item(page, 'Changelog')).toContainText('GitHub · you/agent-skills/skills/changelog @ 5e6f7a8');
  await expect(page.getByRole('checkbox', { name: 'Changelog: Claude Code · User' })).not.toBeChecked();
  await expect(source.locator('.skills-catalog-skill', { hasText: 'Changelog' })).toContainText('In the library');

  await page.getByRole('button', { name: 'Import all from you/agent-skills' }).click();
  await expect(page.getByRole('status')).toContainText('Imported 2 skills. Skipped 1: release-notes is already in the library from anthropics/skills/release-notes.');
  await expect(item(page, 'Release notes')).toContainText('anthropics/skills/release-notes');

  await page.getByRole('button', { name: 'Ignore you/agent-skills' }).click();
  await expect(page.getByRole('list', { name: 'Ignored sources' })).toContainText('you/agent-skills');
  await page.getByRole('button', { name: 'Find my skill repositories' }).click();
  await expect(page.getByRole('status')).toHaveText('No new skill repositories found.');
  await expect(page.getByRole('list', { name: 'Catalog sources' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Stop ignoring you/agent-skills' }).click();
  await page.getByRole('button', { name: 'Disable you/agent-skills' }).click();
  await expect(source).toContainText('Disabled');
  await expect(page.getByRole('button', { name: 'Import all from you/agent-skills' })).toHaveCount(0);
});

test('source scan is ephemeral; selected skills are saved by repository, folder and commit', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Import from GitHub', exact: true }).click();
  await expect(page.getByRole('list', { name: 'Saved GitHub skill sources' })).toHaveCount(0);
  await page.getByRole('textbox', { name: 'Source repository' }).fill('https://github.com/ginmp8/rhapsodia');
  await page.getByRole('textbox', { name: 'Skills folder path' }).fill('skills');
  await page.getByRole('button', { name: 'List skills' }).click();
  await expect(page.getByRole('status')).toContainText('Found 4 skills');
  const choices = page.getByRole('list', { name: 'Select GitHub skills' });
  await expect(choices.getByRole('checkbox')).toHaveCount(4);
  await expect(page.getByRole('list', { name: 'Saved GitHub skill sources' })).toHaveCount(0);
  await page.getByRole('checkbox', { name: 'Select software-improvement-loop from ginmp8/rhapsodia' }).check();
  await page.getByRole('button', { name: 'Import selected and save source' }).click();
  await expect(page.getByRole('status')).toContainText('Imported 1 skill and saved ginmp8/rhapsodia/skills.');
  const saved = page.getByRole('list', { name: 'Saved GitHub skill sources' });
  await expect(saved.getByRole('listitem')).toHaveCount(1);
  await expect(saved).toContainText('1 selected skill');
  await expect(saved.locator('a[href*="github.com/ginmp8/rhapsodia/tree/"]')).toHaveCount(1);
  await page.getByRole('button', { name: 'Library & activation' }).click();
  await expect(item(page, 'software-improvement-loop')).toContainText('ginmp8/rhapsodia/skills/software-improvement-loop');
  await expect(page.getByRole('list', { name: 'Library' })).not.toContainText('systematic-debugging');
  await expect(page.getByRole('checkbox', { name: 'software-improvement-loop: Claude Code · User' })).not.toBeChecked();
});

test('saved source selections can be edited or removed without deleting imported skills', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Import from GitHub' }).click();
  await page.getByRole('textbox', { name: 'Source repository' }).fill('ginmp8/rhapsodia');
  await page.getByRole('textbox', { name: 'Skills folder path' }).fill('skills');
  await page.getByRole('button', { name: 'List skills' }).click();
  await page.getByRole('checkbox', { name: 'Select archify from ginmp8/rhapsodia', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected and save source' }).click();
  await page.getByRole('button', { name: 'Edit selection' }).click();
  await expect(page.getByRole('checkbox', { name: 'Select archify from ginmp8/rhapsodia', exact: true })).toBeChecked();
  await page.getByRole('checkbox', { name: 'Select systematic-debugging from ginmp8/rhapsodia' }).check();
  await page.getByRole('button', { name: 'Import selected and save source' }).click();
  const saved = page.getByRole('list', { name: 'Saved GitHub skill sources' });
  await expect(saved).toContainText('2 selected skills');
  await expect(saved.getByRole('listitem')).toHaveCount(1);
  await page.getByRole('button', { name: 'Remove source config' }).click();
  await expect(page.getByRole('button', { name: 'Confirm removal' })).toBeVisible();
  await page.getByRole('button', { name: 'Confirm removal' }).click();
  await expect(page.getByRole('list', { name: 'Saved GitHub skill sources' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Library & activation' }).click();
  await expect(item(page, 'archify')).toBeVisible();
  await expect(item(page, 'systematic-debugging')).toBeVisible();
});

test('multiple folders in the same repository remain separate saved configurations', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Import from GitHub' }).click();
  const repo = page.getByRole('textbox', { name: 'Source repository' });
  const path = page.getByRole('textbox', { name: 'Skills folder path' });
  await repo.fill('ginmp8/rhapsodia');
  await path.fill('skills/archify');
  await page.getByRole('button', { name: 'List skills' }).click();
  await expect(page.getByRole('list', { name: 'Select GitHub skills' }).getByRole('checkbox')).toHaveCount(1);
  await page.getByRole('button', { name: 'Select all' }).click();
  await page.getByRole('button', { name: 'Import selected and save source' }).click();
  await path.fill('skills/systematic-debugging');
  await page.getByRole('button', { name: 'List skills' }).click();
  await page.getByRole('button', { name: 'Select all' }).click();
  await page.getByRole('button', { name: 'Import selected and save source' }).click();
  const saved = page.getByRole('list', { name: 'Saved GitHub skill sources' });
  await expect(saved.getByRole('listitem')).toHaveCount(2);
  await expect(saved).toContainText('skills/archify');
  await expect(saved).toContainText('skills/systematic-debugging');
  await page.getByRole('button', { name: 'Synchronization' }).click();
  await expect(page.getByRole('list', { name: 'Synchronization sources' }).getByRole('button', { name: 'Sync selected skills' })).toHaveCount(2);
  await page.getByRole('list', { name: 'Synchronization sources' }).getByRole('button', { name: 'Sync selected skills' }).first().click();
  await expect(page.getByRole('status')).toContainText('Updated 0 skills from saved source.');
});

test('synchronization page offers per-skill and bulk manual sync', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Synchronization' }).click();
  await expect(page.getByRole('list', { name: 'Skill synchronization' })).toContainText('Release notes');
  await page.getByRole('button', { name: 'Sync this skill' }).click();
  await expect(page.getByRole('status')).toContainText('already at the current ref');
  await page.getByRole('button', { name: 'Sync all installed skills' }).click();
  await expect(page.getByRole('status')).toContainText('Updated 0 skills.');
  await page.getByRole('checkbox', { name: 'Automatically synchronize imported skills when PandaCode starts' }).check();
  await expect(page.getByRole('status')).toHaveText('Automatic synchronization enabled for imported skills.');
  await page.getByRole('combobox', { name: 'Startup updates for Release notes' }).selectOption('no');
  await expect(page.getByRole('status')).toHaveText('Updated startup policy for release-notes.');
});

test('an exact GitHub tree URL imports one complete recommended skill by reference', async ({ page }) => {
  await open(page);
  await page.getByRole('combobox', { name: 'Source' }).selectOption('github');
  await page.getByRole('textbox', { name: 'Repository', exact: true }).fill('https://github.com/ginmp8/rhapsodia/tree/main/skills/archify');
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Imported 1 skill.');
  await expect(item(page, 'archify')).toContainText('ginmp8/rhapsodia/skills/archify');
  await expect(page.getByRole('checkbox', { name: 'archify: Claude Code · User' })).not.toBeChecked();
});

test('collection browsing is scoped and refuses an ambiguous tree ref without a ref field', async ({ page }) => {
  await open(page);
  await page.getByRole('combobox', { name: 'Source' }).selectOption('github');
  const repo = page.getByRole('textbox', { name: 'Repository', exact: true });
  await repo.fill('https://github.com/ginmp8/rhapsodia/tree/feature/custom/skills');
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('exact ref separately');
  await repo.fill('https://github.com/ginmp8/rhapsodia/tree/main/skills');
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Use catalog selection or Import all');
  const catalog = page.getByRole('list', { name: 'Catalog sources' });
  await expect(catalog).toContainText('software-improvement-loop');
  await catalog.getByRole('button', { name: 'Import all from ginmp8/rhapsodia' }).click();
  await expect(page.getByRole('status')).toContainText('Nothing is active until you turn it on.');
});

test('a pstack-sized collection is scanned as a catalog and imports past 64 files in total', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Catalog repository' }).fill('acme/nope');
  await page.getByRole('button', { name: 'Scan' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'not found, or your GitHub login cannot read it' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Catalog repository' }).fill('https://github.com/acme/pstack');
  await page.getByRole('button', { name: 'Scan' }).click();
  await expect(page.getByRole('status')).toContainText('Found 40 skills in acme/pstack.');
  await expect(page.getByRole('list', { name: 'Skills in acme/pstack' }).getByRole('listitem')).toHaveCount(40);
  await page.getByRole('button', { name: 'Import all from acme/pstack' }).click();
  await expect(page.getByRole('status')).toHaveText('Imported 40 skills. Nothing is active until you turn it on.');
  await expect(page.getByRole('list', { name: 'Library' }).locator('.skills-item')).toHaveCount(42);
  await page.getByRole('button', { name: 'Refresh acme/pstack' }).click();
  await expect(page.getByRole('status')).toHaveText('Refreshed acme/pstack. Library copies stay as imported.');
});
test('GitHub import and sync spacing follows Settings in light and dark themes', async ({ page }) => {
  await page.setViewportSize({ width: 1120, height: 900 });
  await openWithinSettings(page);
  for (const theme of ['light', 'dark']) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.getByRole('button', { name: 'Import from GitHub', exact: true }).click();
    const tabs = page.getByRole('navigation', { name: 'Skills Library pages' });
    const positions = await tabs.locator('button').evaluateAll(buttons => buttons.map(button => {
      const bounds = button.getBoundingClientRect();
      return { left: bounds.left, right: bounds.right };
    }));
    expect(positions).toHaveLength(3);
    expect(positions[1].left - positions[0].right).toBeLessThanOrEqual(12);
    expect(positions[2].left - positions[1].right).toBeLessThanOrEqual(12);
    await expect(tabs.getByRole('button', { name: 'Import from GitHub' })).toHaveAttribute('aria-pressed', 'true');
    const controls = page.locator('.skills-source-controls');
    const [outer, repo, path, scan] = await Promise.all([
      controls.boundingBox(),
      controls.getByRole('textbox', { name: 'Source repository' }).boundingBox(),
      controls.getByRole('textbox', { name: 'Skills folder path' }).boundingBox(),
      controls.getByRole('button', { name: 'List skills' }).boundingBox(),
    ]);
    expect(repo!.x - outer!.x).toBeGreaterThanOrEqual(16);
    expect(path!.x).toBe(repo!.x);
    expect(scan!.x - outer!.x).toBeGreaterThanOrEqual(16);
    await page.getByRole('button', { name: 'Synchronization', exact: true }).click();
    const sync = page.locator('.skills-sync-controls');
    const [syncBox, toggleBox, syncButtonBox] = await Promise.all([
      sync.boundingBox(),
      sync.getByRole('checkbox', { name: 'Automatically synchronize imported skills when PandaCode starts' }).boundingBox(),
      sync.getByRole('button', { name: 'Sync all installed skills' }).boundingBox(),
    ]);
    expect(toggleBox!.x - syncBox!.x).toBeGreaterThanOrEqual(16);
    expect(syncBox!.x + syncBox!.width - syncButtonBox!.x - syncButtonBox!.width).toBeGreaterThanOrEqual(16);
    await expect(page.getByText('Sources', { exact: true })).toHaveClass(/settings-heading/);
  }
});

test('new Skills Library views stay inside the Settings panel at a narrow window', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 900 });
  await openWithinSettings(page);
  const panel = page.locator('.settings-panel');
  for (const name of ['Import from GitHub', 'Synchronization']) {
    await page.getByRole('button', { name, exact: true }).click();
    const bounds = await panel.boundingBox();
    const tabs = await page.getByRole('navigation', { name: 'Skills Library pages' }).boundingBox();
    expect(tabs!.x).toBeGreaterThanOrEqual(bounds!.x - 1);
    expect(tabs!.x + tabs!.width).toBeLessThanOrEqual(bounds!.x + bounds!.width + 1);
    const overflow = await page.locator('#skills-host').evaluate(host => host.scrollWidth - host.clientWidth);
    expect(overflow).toBeLessThanOrEqual(1);
  }
});

});
