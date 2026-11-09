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
        const listeners = new Set();
        window.__skills = skills;
        window.__closeCount = 0;
        window.__progressEvents = [];
        mountCustomizeSkills(document.getElementById('host'), {
          call: async (command, args = {}) => {
            if (command === 'skills_source_import_tracked' && window.__scenario === 'hold-import') {
              return await new Promise(resolve => { window.__releaseImport = () => resolve(skills.call(command, args)); });
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
          close: () => window.__closeCount++,
        });
      </script></body></html>`,
  }));
  await page.goto('/customize-skills-harness.html');
  await expect(page.getByRole('heading', {name:'Customize'})).toBeVisible();
  await expect(page.getByRole('heading', {name:'Skills'})).toBeVisible();
}

test('Customize navigation remains separate from the Bots bar', async ({page}) => {
  // The full application preview loads multiple independent panels. Waiting for
  // DOM readiness plus the observable navigation control avoids idle media/font
  // requests counting against this test's existing, unchanged timeout.
  await page.goto('/', {waitUntil:'domcontentloaded'});
  const customize = page.getByRole('button', {name:'Customize', exact:true});
  await expect(customize).toBeVisible();
  await page.locator('.sidebar').screenshot({path:'test-results/customize-navigation-before.png',animations:'disabled'});
  await expect(page.locator('.sidebar-customize + #sidebar-bots-panel + #sidebar-bots')).toHaveCount(1);
  await customize.click();
  await expect(page.getByRole('heading', {name:'Customize',exact:true})).toBeVisible();
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
