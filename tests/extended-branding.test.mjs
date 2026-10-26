import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = async path => readFile(new URL('../' + path, import.meta.url), 'utf8');

test('Extended desktop branding retains stable package and application identity', async () => {
  const pkg = JSON.parse(await source('package.json'));
  const native = JSON.parse(await source('src-tauri/tauri.conf.json'));
  const cargo = await source('src-tauri/Cargo.toml');
  const page = await source('index.html');
  const main = await source('src/main.ts');
  assert.equal(native.productName, 'PandaCode Extended');
  assert.equal(native.app.windows[0].title, 'PandaCode Extended');
  assert.equal(native.identifier, 'ai.pandacode.desktop');
  assert.equal(native.version, '0.6.0');
  assert.equal(pkg.name, 'pandacode');
  assert.equal(pkg.version, '0.6.0');
  assert.match(cargo, /name = "pandacode"/);
  assert.match(page, /<title>PandaCode Extended<\/title>/);
  assert.match(main, /class="brand-name"[^>]*>PandaCode Extended<\/span>/);
  const metadata = JSON.parse(await source('EXTENDED-RELEASE.json'));
  assert.equal(metadata.extended_version, '0.6.0');
  assert.equal(metadata.display_version, '0.6.0+pc.0.3.48');
  const about = await source('src/about-settings.ts');
  assert.match(about, /value === extendedRelease\.extended_version/);
});

test('Extended About page separately attributes the original PandaCode project', async () => {
  const about = await source('src/about-settings.ts');
  const support = await source('src/support-settings.ts');
  const tray = await source('src/tray-popover.ts');
  const nativeTray = await source('src-tauri/src/tray.rs');
  assert.match(about, /About PandaCode Extended/);
  assert.match(about, /extendedRepository: 'https:\/\/github\.com\/ginmp8\/pandacode-extended'/);
  assert.match(about, /extendedIssues: 'https:\/\/github\.com\/ginmp8\/pandacode-extended\/issues'/);
  assert.match(about, /https:\/\/github\.com\/aicodeking\/pandacode/);
  assert.match(about, /Extended issues/);
  assert.match(about, /Upstream repository/);
  assert.match(about, /Upstream releases/);
  assert.match(support, /PandaCode Extended:/);
  assert.match(tray, /PandaCode Extended/);
  assert.match(nativeTray, /Open PandaCode Extended/);
});
