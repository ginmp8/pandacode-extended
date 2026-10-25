import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const read = async path => readFile(new URL('../' + path, import.meta.url), 'utf8');
const extended = 'https://github.com/ginmp8/pandacode-extended';
const upstream = 'https://github.com/aicodeking/pandacode';
const retiredUrl = 'https://github.com/ginmp8/pandacode-hardened';

test('Extended repository, issue and release links target the planned canonical address', async () => {
  const pkg = JSON.parse(await read('package.json'));
  assert.equal(pkg.homepage, extended + '#readme');
  assert.equal(pkg.repository.url, 'git+' + extended + '.git');
  assert.equal(pkg.bugs.url, extended + '/issues');

  const about = await read('src/about-settings.ts');
  assert.ok(about.includes(extended + "'"));
  assert.ok(about.includes(extended + "/issues'"));
  assert.ok(about.includes(upstream + "'"));

  const readme = await read('README.md');
  const history = await read('docs/UPSTREAM-RELEASE-HISTORY.md');
  const changelog = await read('website/src/pages/changelog.astro');
  assert.ok(readme.includes(extended + '/issues'));
  assert.ok(history.includes(extended + '/releases'));
  assert.ok(changelog.includes(extended + '/releases'));
  for (const current of [readme, history, changelog, about, await read('docs/VERSIONING.md')]) {
    assert.ok(!current.includes(retiredUrl), 'maintained URL still points to old repository');
  }
});

test('Extended website source/issue links and original PandaCode download/attribution links remain distinct', async () => {
  const distribution = await read('website/src/lib/distribution.ts');
  const license = await read('website/src/pages/license.astro');
  const releaseTools = await read('scripts/release/common.mjs');
  assert.ok(distribution.includes("const repoUrl = '" + extended + "'"));
  assert.ok(distribution.includes("originalPandaCode = { repo: '" + upstream + "' }"));
  assert.match(distribution, /releases: .*originalPandaCode\.repo.*releases\/latest/);
  assert.match(distribution, /const latest = .*originalPandaCode\.repo.*releases\/latest\/download/);
  assert.ok(license.includes('href={originalPandaCode.repo}'));
  assert.ok(license.includes('href={project.repo}'));
  assert.ok(releaseTools.includes("|| 'ginmp8/pandacode-extended'"));
});
