import { expect, test, type Page } from '@playwright/test';

/* Isolate Customize from the chat runtime while exercising the same browser preview library. */
async function openCustomize(page: Page, slow = false): Promise<void> {
  await page.route('**/customize-skills-harness.html*', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html lang="en"><head><link rel="stylesheet" href="/src/styles.css"><link rel="stylesheet" href="/src/design-system.css"></head>
      <body><div id="host"></div><script type="module">
        import { mountCustomizeSkills } from '/src/customize-skills.ts';
        import { createPreviewSkills } from '/src/preview-skills.ts';
        const skills = createPreviewSkills();
        const listeners = new Set();
        window.__skills = skills;
        window.__closeCount = 0;
        window.__progressEvents = [];
        window.__trySkillRequests = [];
        window.__holdSkillsList = new URLSearchParams(location.search).has('slow');
        mountCustomizeSkills(document.getElementById('host'), {
          call: async (command, args = {}) => {
            if (command === 'skills_list' && window.__holdSkillsList) {
              await new Promise(resolve => {window.__releaseSkillsList = () => {window.__holdSkillsList=false;resolve();};});
            }
            if (command === 'skills_source_import_tracked' && window.__scenario === 'hold-import') {
              return await new Promise(resolve => { window.__releaseImport = () => resolve(skills.call(command, args)); });
            }
            if (command === 'skills_source_preview' && window.__scenario === 'hold-scan') {
              return await new Promise(resolve => {window.__releaseScan = () => resolve(skills.call(command,args));});
            }
            if (command === 'skills_sync_tracked' && window.__scenario === 'hold-sync') {
              return await new Promise(resolve => {window.__releaseSync = () => resolve(skills.call(command,args));});
            }
            if (command === 'skills_preview_file' && window.__scenario === 'hold-preview') {
              return await new Promise(resolve => {window.__releasePreview = () => resolve(skills.call(command,args));});
            }
            if (command === 'skills_sync_tracked' && window.__scenario === 'sync-busy') {
              return {checkedAt:Date.now(),installed:[],updated:[],skipped:['A skill synchronization is already running.'],events:[]};
            }
            if (command === 'skills_sync_tracked' && window.__scenario === 'sync-revisions') {
              const skill=skills.call('skills_list').find(item=>item.kind==='skill'&&item.source.kind==='github');
              return {checkedAt:Date.now(),installed:[],updated:[skill.id],skipped:[],events:[{id:skill.id,status:'updated',
                oldCommit:'a'.repeat(40),newCommit:'b'.repeat(40),impactedHarnesses:['codex'],detail:'Updated the central library.'}]};
            }
            if (command === 'skills_preview_file' && window.__scenario === 'malicious') {
              const text = '<img src="https://example.org/tracker.png"><scr'+'ipt>window.bad=1</scr'+'ipt>';
              return {path:args.file,text,size:text.length,binary:false};
            }
            if (command === 'skills_source_import_tracked' && window.__scenario === 'partial') {
              const request = args.request, paths=request.selectedPaths;
              const preview=skills.call('skills_source_preview',{repo:request.repo,path:request.path,gitRef:request.gitRef});
              const failedPath=paths.length===3 ? paths[1] : null;
              const successful=paths.filter(path=>path!==failedPath);
              const persisted=skills.call('skills_source_import_tracked',{request:{...request,selectedPaths:successful}});
              const results=[];
              for(let index=0;index<paths.length;index++){
                const path=paths[index];
                const found=preview.skills.find(skill=>skill.path===path);
                const common={operationId:request.operationId,path,name:found?.name??path,total:paths.length,completed:index,detail:null};
                for(const listener of listeners)listener({...common,phase:'fetching'});
                await new Promise(resolve=>setTimeout(resolve,20));
                const final={...common,completed:index+1,phase:path===failedPath?'failed':'installed',
                  detail:path===failedPath?'GitHub rate limit: retry this package.':null};
                for(const listener of listeners)listener(final);
                window.__progressEvents.push(final);
                results.push(final);
              }
              return {...persisted,results,
                skipped:failedPath?[{path:failedPath,name:preview.skills.find(skill=>skill.path===failedPath)?.name??failedPath,reason:'GitHub rate limit: retry this package.'}]:[]};
            }
            return skills.call(command, args);
          },
          subscribe: async listener => { listeners.add(listener); return () => listeners.delete(listener); },
          chooseZip: async () => '/Users/you/Downloads/skill.zip',
          trySkill: async (id, harness) => {window.__trySkillRequests.push({id,harness});},
          close: () => window.__closeCount++,
        });
      </script></body></html>`,
  }));
  await page.goto(slow ? '/customize-skills-harness.html?slow=1' : '/customize-skills-harness.html');
  await expect(page.getByRole('heading', {name:'Customize'})).toBeVisible();
  await expect(page.getByRole('heading', {name:'Skills'})).toBeVisible();
  const category = page.getByRole('navigation', {name:'Customization categories'}).getByRole('button', {name:'Skills'});
  await expect(category).toBeVisible();
  const geometry = await category.evaluate(button => {
    const title = button.querySelector('span')!;
    return {left:button.getBoundingClientRect().left,iconLeft:button.querySelector('svg')!.getBoundingClientRect().left,textLeft:title.getBoundingClientRect().left};
  });
  expect(geometry.iconLeft).toBeLessThan(geometry.textLeft);
  expect(geometry.iconLeft-geometry.left).toBeLessThan(30);
  if(!slow) await expect(page.locator('.customize-skill-icon img').first()).toHaveAttribute('src', /^data:image\/svg\+xml,/);

}

test('Customize follows Search inside the top actions card and stays separate from Bots', async ({page}) => {
  // Full application startup on CI may exceed the default test budget; preserve every assertion.
  test.setTimeout(60_000);
  // The full application preview loads multiple independent panels. Waiting for
  // DOM readiness plus the observable navigation control avoids idle media/font
  // requests counting against this test's existing, unchanged timeout.
  await page.goto('/', {waitUntil:'domcontentloaded'});
  const customize = page.getByRole('button', {name:'Customize', exact:true});
  await expect(customize).toBeVisible();
  await page.locator('.sidebar').screenshot({path:'test-results/customize-navigation-before.png',animations:'disabled'});
  await expect(page.locator('.sidebar-actions > .sidebar-customize-action')).toHaveCount(1);
  await expect(page.locator('.sidebar-actions > button').last()).toHaveAttribute('data-action', 'toggle-customize');
  await expect(page.locator('.sidebar-actions > .search-trigger + .sidebar-customize-action')).toHaveCount(1);
  await expect(page.locator('#sidebar-bots-panel + #sidebar-bots')).toHaveCount(1);
  await customize.click();
  await expect(page.getByRole('heading', {name:'Customize',exact:true})).toBeVisible();
  await expect(customize).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.workspace-header')).toBeHidden();
  await expect(page.locator('.conversation-pane')).toBeHidden();
  await page.screenshot({path:'test-results/customize-navigation-after.png',fullPage:true,animations:'disabled'});
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
  // The global input rule assigns width:100%; checkboxes must override it
  // or they consume the row and crush skill names into vertical letters.
  const geometry = await page.locator('.customize-discovery-list > li').first().evaluate(row => {
    const checkbox = row.querySelector<HTMLInputElement>('input[type=checkbox]')!;
    const label = row.querySelector<HTMLElement>('.customize-discovery-choice > span')!;
    return {
      checkboxWidth: checkbox.getBoundingClientRect().width,
      labelWidth: label.getBoundingClientRect().width,
      rowHeight: row.getBoundingClientRect().height,
    };
  });
  expect(geometry.checkboxWidth).toBeLessThan(32);
  expect(geometry.labelWidth).toBeGreaterThan(180);
  expect(geometry.rowHeight).toBeLessThan(120);
  await page.screenshot({path:'test-results/customize-github-discovery.png',fullPage:true});
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
  await expect(page.getByRole('region',{name:'Read-only skill preview'})).toHaveCount(0);
  await page.screenshot({path:'test-results/customize-zip-installed.png',fullPage:true});
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'ZIP archive'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('ZIP replacement requires confirmation');
  await page.getByRole('button',{name:'Confirm ZIP replacement'}).click();
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

test('three selected skills show partial failure, truthful progress and selective retry', async ({page}) => {
  await openCustomize(page);
  await page.evaluate(() => { (window as any).__scenario='partial'; });
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/acme/pstack');
  await page.getByRole('button',{name:'Scan repository'}).click();
  const choices=page.getByRole('region',{name:'Discovered GitHub skills'}).getByRole('checkbox');
  for(let i=0;i<3;i++)await choices.nth(i).check();
  await page.getByRole('button',{name:'Import selected (3)'}).click();
  const progress=page.getByRole('region',{name:'Skill operation status'});
  await expect(progress).toContainText('Completed with errors');
  await expect(progress).toContainText('3 of 3 processed');
  await expect(progress).toContainText('GitHub rate limit');
  await expect(page.getByRole('button',{name:'Retry failed (1)'})).toBeVisible();
  const observed=await page.evaluate(() => (window as any).__progressEvents.map((event:any)=>event.completed));
  expect(observed).toEqual([1,2,3]);
  await page.evaluate(() => { (window as any).__scenario='success'; });
  await page.getByRole('button',{name:'Retry failed (1)'}).click();
  await expect(progress).toContainText('Completed successfully');
  await expect(progress).toContainText('1 of 1 processed');
});

test('aggregate-only completion is not falsely described as streamed progress', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/acme/pstack');
  await page.getByRole('button',{name:'Scan repository'}).click();
  const choices=page.getByRole('region',{name:'Discovered GitHub skills'}).getByRole('checkbox');
  for(let i=0;i<3;i++)await choices.nth(i).check();
  await page.getByRole('button',{name:'Import selected (3)'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Native incremental progress events were not received');
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed with errors');
  await expect(page.getByRole('region',{name:'Skill operation status'})).not.toContainText('Completed successfully');
});

test('read-only preview cannot load remote resources or execute scripts', async ({page}) => {
  await openCustomize(page);
  await page.evaluate(() => { (window as any).__scenario='malicious'; });
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  await expect(detail.getByRole('region',{name:'Skill file content'})).toBeVisible();
  await expect(detail.locator('script,img,iframe,pandacode-ui')).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).bad)).toBeUndefined();
});

test('a busy native synchronization cannot be reported as successful', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button',{name:'Scan repository'}).click();
  await page.getByRole('region',{name:'Discovered GitHub skills'}).getByRole('checkbox').nth(1).check();
  await page.getByRole('button',{name:'Import selected (1)'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
  await page.evaluate(() => { (window as any).__scenario='sync-busy'; });
  await page.getByRole('button',{name:'GitHub sources'}).click();
  await page.getByRole('button',{name:'Sync all GitHub skills'}).click();
  const status=page.getByRole('region',{name:'Skill operation status'});
  await expect(status).toContainText('Operation failed');
  await expect(status).toContainText('A skill synchronization is already running.');
  await expect(status).not.toContainText('Completed successfully');
});

test('manual synchronization reports old and new revisions and harness impact', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button',{name:'Scan repository'}).click();
  await page.getByRole('region',{name:'Discovered GitHub skills'}).getByRole('checkbox').nth(1).check();
  await page.getByRole('button',{name:'Import selected (1)'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
  await page.evaluate(() => { (window as any).__scenario='sync-revisions'; });
  await page.getByRole('button',{name:'GitHub sources'}).click();
  await page.getByRole('button',{name:'Sync all GitHub skills'}).click();
  const status=page.getByRole('region',{name:'Skill operation status'});
  await expect(status).toContainText('Old revision: aaaaaaaaaaaa');
  await expect(status).toContainText('New revision: bbbbbbbbbbbb');
  await expect(status).toContainText('Harnesses to review: codex');
});

test('a source cannot be removed while its import is in flight', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button',{name:'Scan repository'}).click();
  await page.getByRole('region',{name:'Discovered GitHub skills'}).getByRole('checkbox').nth(1).check();
  await page.getByRole('button',{name:'Import selected (1)'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
  await page.getByRole('button',{name:'GitHub sources'}).click();
  await page.getByRole('button',{name:'Edit selection'}).click();
  await expect(page.getByRole('button',{name:'Import selected (1)'})).toBeEnabled();
  await page.evaluate(() => { (window as any).__scenario='hold-import'; });
  await page.getByRole('button',{name:'Import selected (1)'}).click();
  await page.getByRole('button',{name:'GitHub sources'}).click();
  const remove=page.getByRole('button',{name:'Remove source'});
  await expect(remove).toBeDisabled();
  await page.evaluate(() => (window as any).__releaseImport());
  await expect(remove).toBeEnabled();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
});

test('checkbox selection preserves keyboard focus after the workspace redraw', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button', {name:'Add',exact:true}).click();
  await page.getByRole('menuitem', {name:'GitHub link'}).click();
  await page.getByRole('textbox', {name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button', {name:'Scan repository'}).click();
  const checkbox = page.getByRole('region', {name:'Discovered GitHub skills'})
    .getByRole('checkbox').nth(1);
  await checkbox.focus();
  await page.keyboard.press('Space');
  await expect(checkbox).toBeChecked();
  await expect(checkbox).toBeFocused();
  await page.keyboard.press('Space');
  await expect(checkbox).not.toBeChecked();
  await expect(checkbox).toBeFocused();
});

test('initial library load shows progress and never flashes an empty library', async ({page}) => {
  await openCustomize(page,true);
  const loading=page.getByRole('status',{name:'Loading installed skills'});
  await expect(loading).toBeVisible();
  await expect(page.getByText('No skills installed')).toHaveCount(0);
  await page.evaluate(() => (window as any).__releaseSkillsList());
  await expect(loading).toBeHidden();
  await expect(page.getByRole('button',{name:'Inspect Release notes'})).toBeVisible();
});

test('installed skill can be enabled and disabled with visible feedback', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const toggle=detail.getByRole('checkbox',{name:'Enable Release notes for Codex'});
  await expect(toggle).not.toBeChecked();
  await expect(detail.getByRole('button',{name:'Try in chat'})).toBeDisabled();
  await toggle.check();
  await expect(toggle).toBeChecked();
  await expect(page.locator('.customize-toast')).toContainText('enabled for Codex');
  await expect(detail.getByRole('button',{name:'Try in chat'})).toBeEnabled();
  await toggle.uncheck();
  await expect(toggle).not.toBeChecked();
  await expect(page.locator('.customize-toast')).toContainText('disabled for Codex');
  await expect(detail.getByRole('button',{name:'Try in chat'})).toBeDisabled();
});

test('Try in chat uses the enabled harness and never submits the skill automatically', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  await detail.getByRole('checkbox',{name:'Enable Release notes for Claude Code'}).check();
  await detail.getByRole('button',{name:'Try in chat'}).click();
  await expect.poll(() => page.evaluate(() => (window as any).__trySkillRequests)).toEqual([{id:'release-notes',harness:'claude'}]);
});

test('uninstall requires confirmation and refuses skills still activated', async ({page}) => {
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const toggle=detail.getByRole('checkbox',{name:'Enable Release notes for Codex'});
  await toggle.check();
  await detail.getByRole('button',{name:'Uninstall',exact:true}).click();
  await expect(detail).toContainText('Disable all 1 activation');
  await expect(detail.getByRole('button',{name:'Confirm uninstall'})).toBeDisabled();
  await detail.getByRole('button',{name:'Cancel uninstall'}).click();
  await toggle.uncheck();
  await detail.getByRole('button',{name:'Uninstall',exact:true}).click();
  await expect(detail.getByRole('button',{name:'Confirm uninstall'})).toBeEnabled();
  await detail.getByRole('button',{name:'Confirm uninstall'}).click();
  await expect(page.getByRole('button',{name:'Inspect Release notes'})).toHaveCount(0);
  await expect(page.locator('.customize-toast')).toContainText('uninstalled from the PandaCode library');
});

test('safety findings require a separate explicit consent to activate', async ({page}) => {
  await openCustomize(page);
  await page.evaluate(() => (window as any).__skills.importFolder('/tmp/setup','skill'));
  await page.getByRole('button',{name:'Refresh skills'}).click();
  await page.getByRole('button',{name:'Inspect setup'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const toggle=detail.getByRole('checkbox',{name:'Enable setup for Codex'});
  await toggle.click(); // Safety consent intentionally rejects an immediate checked state.
  await expect(toggle).not.toBeChecked();
  await expect(detail).toContainText('safety finding');
  await detail.getByRole('button',{name:'I reviewed the findings. Enable anyway'}).click();
  await expect(toggle).toBeChecked();
  await expect(page.locator('.customize-toast')).toContainText('setup enabled for Codex');
});

test('Try in chat opens a new projectless chat with a prefilled skill command', async ({page}) => {
  test.setTimeout(60_000);
  await page.goto('/',{waitUntil:'domcontentloaded'});
  await page.getByRole('button',{name:'Customize',exact:true}).click();
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  await detail.getByRole('checkbox',{name:'Enable Release notes for Codex'}).check();
  await detail.getByRole('button',{name:'Try in chat'}).click();
  await expect(page.locator('.customize-error')).toHaveCount(0);
  await expect(page.locator('#customize-hub')).toBeHidden();
  await expect(page.locator('#prompt')).toHaveValue('/release-notes ');
  await expect(page.locator('#prompt')).toBeFocused();
});

test('search typing keeps the caret at the end instead of reversing characters',async({page})=>{
  await openCustomize(page);
  const search=page.getByRole('searchbox',{name:'Search installed skills'});
  await search.pressSequentially('release');
  await expect(search).toHaveValue('release');
  expect(await search.evaluate(element=>(element as HTMLInputElement).selectionStart)).toBe('release'.length);
  await expect(page.getByRole('button',{name:'Inspect Release notes'})).toBeVisible();
});

test('skill details open in a solid right inspector with the Files row tree',async({page})=>{
  await openCustomize(page);
  await expect(page.locator('.customize-skill-row').getByRole('link',{name:'View source'})).toHaveCount(0);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const dialog=page.getByRole('region',{name:'Read-only skill preview'});
  await expect(dialog).toBeVisible();
  await expect(page.locator('.customize-detail-dialog')).toHaveCount(0);
  await expect(dialog.locator('#customize-preview-content')).toContainText('Release notes');
  await page.screenshot({path:'test-results/customize-installed-side-panel.png',animations:'disabled'});
  const box = await dialog.boundingBox(), main = await page.locator('.customize-page').boundingBox();
  expect(box).not.toBeNull();
  expect(main).not.toBeNull();
  expect(Math.abs(main!.x + main!.width - box!.x)).toBeLessThan(3);
  expect(await dialog.evaluate(element => getComputedStyle(element).backgroundColor)).toMatch(/^rgb\(/);
  await expect(dialog.getByRole('link',{name:'View source'})).toHaveAttribute('href',/github\.com\/anthropics\/skills\/tree\//);
  const tree=dialog.getByRole('tree',{name:'Package file tree'});
  await expect(tree).toBeVisible();
  await expect(tree.getByRole('button',{name:'SKILL.md'})).toBeVisible();
  const rootRow = tree.locator('.file-row:has(> button[data-file="SKILL.md"])');
  await expect(rootRow).toHaveAttribute('style','--depth:0');
  await expect(tree.locator('.file-row .file-twisty').first()).toBeVisible();
  await expect(tree.getByRole('button',{name:'Collapse assets folder'})).toBeVisible();
  await expect(tree.getByRole('button',{name:'icon.svg'})).toBeVisible();
  const nestedRow = tree.locator('.file-row:has(> button[data-file="assets/icon.svg"])');
  await expect(nestedRow).toHaveAttribute('style','--depth:1');
  await tree.getByRole('button',{name:'Collapse assets folder'}).click();
  await expect(tree.getByRole('button',{name:'icon.svg'})).toHaveCount(0);
  await tree.getByRole('button',{name:'Expand assets folder'}).click();
  await tree.getByRole('button',{name:'icon.svg'}).click();
  await expect(dialog.locator('#customize-preview-content')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Inspect Release notes'})).toBeFocused();
});

test('unmanaged harness copies require confirmation and retain a backup on replace',async({page})=>{
  await openCustomize(page);
  await page.evaluate(()=>(window as any).__skills.plantUnmanaged('release-notes','codex','user'));
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const dialog=page.getByRole('region',{name:'Read-only skill preview'});
  const toggle=dialog.getByRole('checkbox',{name:'Enable Release notes for Codex'});
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(dialog).toContainText('An unmanaged skill folder already exists');
  await dialog.getByRole('button',{name:'Back up and replace'}).click();
  await expect(toggle).toBeChecked();
  await expect(dialog.locator('.customize-dialog-toast')).toContainText('Original skill backed up at');
  await expect(page.locator('.customize-skill-activity').first()).toHaveClass(/customize-active-text/);
});

test('GitHub scanning shows a spinner and the current action before results arrive',async({page})=>{
  await openCustomize(page);
  await page.evaluate(() => { (window as any).__scenario='hold-scan'; });
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button',{name:'Scan repository'}).click();
  const activity=page.getByRole('status',{name:'Skill action in progress'});
  await expect(activity).toContainText('Scanning GitHub');
  await expect(activity.locator('.customize-spinner')).toBeVisible();
  await expect(page.getByRole('button',{name:'Scan repository'})).toBeDisabled();
  await page.evaluate(() => (window as any).__releaseScan());
  await expect(activity).toHaveCount(0);
  await expect(page.getByRole('region',{name:'Discovered GitHub skills'})).toBeVisible();
});

test('manual GitHub sync shows a loading state for the entire external operation',async({page})=>{
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'GitHub link'}).click();
  await page.getByRole('textbox',{name:'GitHub URL'}).fill('https://github.com/you/agent-skills');
  await page.getByRole('button',{name:'Scan repository'}).click();
  const found=page.getByRole('region',{name:'Discovered GitHub skills'});
  await found.getByRole('checkbox').first().check();
  await found.getByRole('checkbox',{name:'I approve replacing conflicting installed skills'}).check();
  await found.getByRole('button',{name:'Import selected (1)'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed');
  await page.evaluate(() => { (window as any).__scenario='hold-sync'; });
  await page.getByRole('button',{name:'GitHub sources'}).click();
  await page.getByRole('button',{name:'Sync all GitHub skills'}).click();
  const activity=page.getByRole('status',{name:'Skill action in progress'});
  await expect(activity).toContainText('Synchronizing GitHub skills');
  await expect(page.getByRole('button',{name:'Sync all GitHub skills'})).toBeDisabled();
  await page.evaluate(() => (window as any).__releaseSync());
  await expect(activity).toHaveCount(0);
});

test('skill file preview renders an explicit loading state while the file is read',async({page})=>{
  await openCustomize(page);
  await page.evaluate(() => { (window as any).__scenario='hold-preview'; });
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  await expect(detail.locator('.customize-preview-loading .customize-spinner')).toBeVisible();
  await page.evaluate(() => (window as any).__releasePreview());
  await expect(detail.locator('.customize-preview-loading')).toHaveCount(0);
  await expect(detail.locator('#customize-preview-content')).toContainText('Release notes');
});

test('the skill inspector becomes a full-width pane on narrow viewports',async({page})=>{
  await page.setViewportSize({width:900,height:750});
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  await expect(detail).toBeVisible();
  await expect(page.locator('.customize-page')).toBeHidden();
  const box=await detail.boundingBox();
  expect(box!.width).toBeGreaterThan(850);
  await detail.getByRole('button',{name:'Close Release notes'}).click();
  await expect(page.locator('.customize-page')).toBeVisible();
});

test('each opened skill has a closable tab with its own selected file and preview mode', async ({page}) => {
  await page.setViewportSize({width:1450,height:850});
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'ZIP archive'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');

  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const tabs=detail.getByRole('tablist',{name:'Open skill previews'});
  await expect(tabs.getByRole('tab',{name:'Release notes'})).toHaveAttribute('aria-selected','true');
  await expect(detail.getByRole('button',{name:'Close preview'})).toHaveCount(0);
  await detail.getByRole('tree',{name:'Package file tree'}).getByRole('button',{name:'icon.svg'}).click();
  await expect(detail.locator('.customize-preview-toolbar')).toContainText('assets/icon.svg');
  await detail.getByRole('button',{name:'Raw'}).click();
  await expect(detail.getByRole('button',{name:'Raw'})).toHaveAttribute('aria-pressed','true');

  await page.getByRole('button',{name:'Inspect ZIP sample'}).click();
  await expect(tabs.getByRole('tab')).toHaveCount(2);
  await expect(tabs.getByRole('tab',{name:'ZIP sample'})).toHaveAttribute('aria-selected','true');
  await page.screenshot({path:'test-results/customize-skill-tabs.png',animations:'disabled'});
  await expect(detail.locator('.customize-preview-toolbar')).toContainText('SKILL.md');
  await expect(detail.getByRole('button',{name:'Rendered'})).toHaveAttribute('aria-pressed','true');

  await tabs.getByRole('tab',{name:'Release notes'}).click();
  await expect(detail.locator('.customize-preview-toolbar')).toContainText('assets/icon.svg');
  await expect(detail.getByRole('button',{name:'Raw'})).toHaveAttribute('aria-pressed','true');
  await expect(tabs.getByRole('tab',{name:'Release notes'})).toHaveAttribute('aria-selected','true');
  await tabs.getByRole('button',{name:'Close ZIP sample'}).click();
  await expect(tabs.getByRole('tab')).toHaveCount(1);
  await expect(tabs.getByRole('tab',{name:'Release notes'})).toHaveAttribute('aria-selected','true');
  await tabs.getByRole('button',{name:'Close Release notes'}).click();
  await expect(detail).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Inspect Release notes'})).toBeFocused();
});

test('long skill names are truncated in inspector tabs but retain full accessible names', async ({page}) => {
  await openCustomize(page);
  const fullName='An extraordinarily long skill name for comprehensive interface validation';
  await page.evaluate(name => {
    const skills=(window as any).__skills;
    const original=skills.call.bind(skills);
    skills.call=(command:string,args:Record<string,unknown> = {}) => {
      const result=original(command,args);
      return command==='skills_list'
        ? result.map((item:any) => item.id==='release-notes' ? {...item,name} : item)
        : result;
    };
  },fullName);
  await page.getByRole('button',{name:'Refresh skills'}).click();
  await page.getByRole('button',{name:`Inspect ${fullName}`}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const tab=detail.getByRole('tab',{name:fullName});
  await expect(tab).toHaveAttribute('title',fullName);
  const visibleLabel=await tab.locator('.customize-skill-tab-label').textContent();
  expect(Array.from(visibleLabel??'')).toHaveLength(23);
  expect(visibleLabel?.endsWith('…')).toBe(true);
  await expect(detail.getByRole('button',{name:`Close ${fullName}`})).toBeVisible();
});

test('clicking a skill does not depress its card or select its text', async ({page}) => {
  await openCustomize(page);
  const card=page.getByRole('button',{name:'Inspect Release notes'});
  expect(await card.evaluate(element => getComputedStyle(element).userSelect)).toBe('none');
  await card.hover();
  const box=await card.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x + 40,box!.y + box!.height/2);
  const before=await card.evaluate(element => getComputedStyle(element).backgroundColor);
  await page.mouse.down();
  const pressed=await card.evaluate(element => ({
    background:getComputedStyle(element).backgroundColor,
    scale:getComputedStyle(element).scale,
    transform:getComputedStyle(element).transform,
  }));
  expect(pressed.background).toBe(before);
  expect(pressed.scale).toBe('1');
  expect(pressed.transform).toBe('none');
  await page.mouse.up();
  await card.click({clickCount:3});
  expect(await page.evaluate(() => window.getSelection()?.toString() ?? '')).toBe('');
});

test('skill inspector resizes by drag and keyboard, persists its width, and resets on double-click', async ({page}) => {
  await page.setViewportSize({width:1500,height:860});
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const separator=detail.getByRole('separator',{name:'Resize skill inspector'});
  await expect(separator).toBeVisible();
  const initial=await detail.boundingBox();
  const grip=await separator.boundingBox();
  expect(initial).not.toBeNull();
  expect(grip).not.toBeNull();
  await page.mouse.move(grip!.x + grip!.width/2,grip!.y + 90);
  await page.mouse.down();
  await page.mouse.move(grip!.x + grip!.width/2 - 130,grip!.y + 90,{steps:5});
  await page.mouse.up();
  const expanded=await detail.boundingBox();
  expect(expanded!.width).toBeGreaterThan(initial!.width + 90);
  await page.screenshot({path:'test-results/customize-resized-inspector.png',animations:'disabled'});
  const saved=await page.evaluate(() => localStorage.getItem('pandacode.customizeSkillsInspectorWidth'));
  expect(Number(saved)).toBeGreaterThan(initial!.width + 90);
  await separator.focus();
  await separator.press('ArrowRight');
  const keyboard=await detail.boundingBox();
  expect(keyboard!.width).toBeLessThan(expanded!.width);
  await expect(separator).toHaveAttribute('aria-valuenow',String(Math.round(keyboard!.width)));
  await separator.dblclick();
  const reset=await detail.boundingBox();
  expect(Math.abs(reset!.width - initial!.width)).toBeLessThan(4);
  expect(await page.evaluate(() => localStorage.getItem('pandacode.customizeSkillsInspectorWidth'))).toBeNull();
});


test('detail controls preserve panel scroll across redraws, confirmations and async activation', async ({page}) => {
  await page.setViewportSize({width:1420,height:640});
  await openCustomize(page);
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const scroller=detail.locator('.customize-side-content');
  await expect(detail.locator('#customize-preview-content')).toContainText('Release notes');
  const scroll=()=>scroller.evaluate(element=>element.scrollTop);
  const mode=detail.getByRole('button',{name:'Raw',exact:true});
  await mode.scrollIntoViewIfNeeded();
  await scroller.evaluate(element=>{element.scrollTop=140;});
  const beforeMode=await scroll();
  expect(beforeMode).toBeGreaterThan(60);
  await mode.click();
  await expect(mode).toHaveAttribute('aria-pressed','true');
  await expect(mode).toBeFocused();
  await expect.poll(scroll).toBe(beforeMode);
  await detail.getByRole('button',{name:'Rendered',exact:true}).click();
  await expect.poll(scroll).toBe(beforeMode);

  const toggle=detail.getByRole('checkbox',{name:'Enable Release notes for Codex'});
  await toggle.scrollIntoViewIfNeeded();
  const beforeToggle=await scroll();
  expect(beforeToggle).toBeGreaterThan(100);
  await toggle.check();
  await expect(toggle).toBeChecked();
  await expect(detail.locator('.customize-dialog-toast')).toContainText('enabled for Codex');
  await expect.poll(scroll).toBe(beforeToggle);

  const uninstall=detail.getByRole('button',{name:'Uninstall',exact:true});
  await uninstall.scrollIntoViewIfNeeded();
  const beforeConfirm=await scroll();
  expect(beforeConfirm).toBeGreaterThan(100);
  await uninstall.click();
  await expect(detail.getByRole('button',{name:'Cancel uninstall'})).toBeVisible();
  await expect.poll(scroll).toBe(beforeConfirm);
  await detail.getByRole('button',{name:'Cancel uninstall'}).click();
  await expect.poll(scroll).toBe(beforeConfirm);
});

test('switching skill tabs restores their independent scroll positions after refresh', async ({page}) => {
  await page.setViewportSize({width:1450,height:650});
  await openCustomize(page);
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByRole('menuitem',{name:'ZIP archive'}).click();
  await expect(page.getByRole('region',{name:'Skill operation status'})).toContainText('Completed successfully');
  await page.getByRole('button',{name:'Inspect Release notes'}).click();
  const detail=page.getByRole('region',{name:'Read-only skill preview'});
  const scroll=()=>detail.locator('.customize-side-content').evaluate(element=>element.scrollTop);
  await detail.locator('.customize-side-content').evaluate(element=>{element.scrollTop=225;});
  const releaseScroll=await scroll();
  expect(releaseScroll).toBeGreaterThan(100);

  await page.getByRole('button',{name:'Inspect ZIP sample'}).click();
  await expect(detail.getByRole('tab',{name:'ZIP sample'})).toHaveAttribute('aria-selected','true');
  await expect(scroll()).resolves.toBe(0);
  await detail.locator('.customize-side-content').evaluate(element=>{element.scrollTop=150;});
  const zipScroll=await scroll();
  expect(zipScroll).toBeGreaterThan(50);
  await detail.getByRole('tab',{name:'Release notes'}).click();
  await expect.poll(scroll).toBe(releaseScroll);
  await page.getByRole('button',{name:'Refresh skills'}).click();
  await expect.poll(scroll).toBe(releaseScroll);
  await detail.getByRole('tab',{name:'ZIP sample'}).click();
  await expect.poll(scroll).toBe(zipScroll);
  await detail.getByRole('button',{name:'Close ZIP sample'}).click();
  await page.getByRole('button',{name:'Inspect ZIP sample'}).click();
  await expect(scroll()).resolves.toBe(0);
});

