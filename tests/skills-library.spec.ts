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
        mountSkillsLibrary(document.getElementById('host'), { call, projects: () => [{ id: 'orbit', name: 'Orbit' }], sessionId: () => 'fixture-task' });
      </script></body></html>`,
  }));
  await page.goto('/skills-library-harness.html');
  await expect(page.getByRole('list', { name: 'Library' })).toBeVisible();
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

test('catalog discovery is explicit and imports selected nested packages without activation', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Catalog repository', exact: true }).fill('acme/skills');
  await page.getByRole('button', { name: 'Add catalog', exact: true }).click();
  const catalog = page.locator('.skills-catalog');
  await expect(catalog).toContainText('skills/nested/review');
  await expect(page.getByRole('list', { name: 'Library', exact: true })).not.toContainText('Catalog review');
  await page.getByRole('checkbox', { name: 'Select Catalog review from acme/skills', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected (1)', exact: true }).click();
  await expect(item(page, 'Catalog review')).toContainText('acme/skills/skills/nested/review @ aaaaaaa');
  await expect(item(page, 'Catalog review').getByRole('checkbox', { name: 'Catalog review: Claude Code · User' })).not.toBeChecked();
  await expect(page.getByRole('list', { name: 'Library', exact: true })).not.toContainText('Catalog test plan');
});

test('catalog refresh marks updates; disabled sources do not import; removal preserves imported copies', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Catalog repository', exact: true }).fill('acme/skills');
  await page.getByRole('button', { name: 'Add catalog', exact: true }).click();
  await page.getByRole('button', { name: 'Select all', exact: true }).click();
  await page.getByRole('button', { name: 'Import selected (2)', exact: true }).click();
  await expect(item(page, 'Catalog review')).toBeVisible();
  await page.getByRole('button', { name: 'Refresh catalog', exact: true }).click();
  await expect(page.locator('.skills-catalog')).toContainText('Update available');
  await page.getByRole('combobox', { name: 'State of acme/skills' }).selectOption('disabled');
  await expect(page.getByRole('button', { name: 'Select all', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Remove catalog', exact: true }).click();
  await expect(page.locator('.skills-catalog')).toHaveCount(0);
  await expect(item(page, 'Catalog review')).toBeVisible();
});

test('bot and project selections are separate, persisted, and require review after import updates', async ({ page }) => {
  await open(page);
  const panel = page.locator('.skills-scopes');
  await panel.getByRole('checkbox', { name: 'Assign Release notes', exact: true }).check();
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel).toContainText('Selection saved for future turns');
  await panel.getByRole('combobox', { name: 'Skill assignment scope' }).selectOption('bot');
  await expect(panel.getByRole('checkbox', { name: 'Assign Release notes', exact: true })).not.toBeChecked();
  await panel.getByRole('checkbox', { name: 'Assign Release notes', exact: true }).check();
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel).toContainText('Selection saved for future turns');
  await expect(item(page, 'Release notes').getByRole('checkbox', { name: 'Release notes: Claude Code · User' })).not.toBeChecked();
  await panel.getByRole('combobox', { name: 'Skill assignment scope' }).selectOption('project');
  await expect(panel.getByRole('checkbox', { name: 'Assign Release notes', exact: true })).toBeChecked();
  const pins = await page.evaluate(() => (window as any).__skills.call('skills_scope_get', { target: 'project', targetId: 'orbit' }));
  expect(pins).toHaveLength(1); expect(pins[0].sha256).toHaveLength(64);
});

test('scope safety findings require explicit acknowledgement and edited imports stay untouched', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Folder or file path' }).fill('/Users/you/skills/setup');
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  const panel = page.locator('.skills-scopes');
  await panel.getByRole('checkbox', { name: 'Assign setup', exact: true }).check();
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel.getByRole('alert')).toContainText('Review the safety findings');
  await panel.getByRole('checkbox', { name: 'Assign setup', exact: true }).check();
  await panel.getByRole('checkbox', { name: 'I reviewed and accept the safety findings of the selected versions', exact: true }).check();
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel).toContainText('Selection saved for future turns');
  await page.getByRole('textbox', { name: 'Catalog repository', exact: true }).fill('acme/skills');
  await page.getByRole('button', { name: 'Add catalog', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Select Catalog review from acme/skills', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected (1)', exact: true }).click();
  await expect(item(page, 'Catalog review')).toBeVisible();
  await page.evaluate(() => (window as any).__skills.editLibrary('catalog-review'));
  await page.getByRole('checkbox', { name: 'Select Catalog review from acme/skills', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected (1)', exact: true }).click();
  await expect(page.locator('.skills-catalogs')).toContainText('edited library files; not overwritten');
});

test('a catalog update leaves the selected hash stale until the user saves the new version', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Catalog repository', exact: true }).fill('acme/skills');
  await page.getByRole('button', { name: 'Add catalog', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Select Catalog review from acme/skills', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected (1)', exact: true }).click();
  const panel = page.locator('.skills-scopes');
  await panel.getByRole('checkbox', { name: 'Assign Catalog review', exact: true }).check();
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel.getByRole('list', { name: 'Effective skills' })).toContainText('project · prompt');
  const before = await page.evaluate(() => (window as any).__skills.call('skills_scope_get', { target: 'project', targetId: 'orbit' }));
  await page.getByRole('button', { name: 'Refresh catalog', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Select Catalog review from acme/skills', exact: true }).check();
  await page.getByRole('button', { name: 'Import selected (1)', exact: true }).click();
  await expect(panel).toContainText('New version needs review and Save');
  await expect(panel.getByRole('list', { name: 'Effective skills' })).toContainText('skipped');
  const unchanged = await page.evaluate(() => (window as any).__skills.call('skills_scope_get', { target: 'project', targetId: 'orbit' }));
  expect(unchanged).toEqual(before);
  await panel.getByRole('button', { name: 'Save skill selection', exact: true }).click();
  await expect(panel.getByRole('list', { name: 'Effective skills' })).toContainText('project · prompt');
  const after = await page.evaluate(() => (window as any).__skills.call('skills_scope_get', { target: 'project', targetId: 'orbit' }));
  expect(after[0].sha256).not.toEqual(before[0].sha256);
});
