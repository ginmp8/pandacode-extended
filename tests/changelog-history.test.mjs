import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

test('website loads both release folders and explicitly namespaces origin/anchors', async () => {
  const lib = await readFile(new URL('../website/src/lib/changelog.ts', import.meta.url), 'utf8');
  const page = await readFile(new URL('../website/src/pages/changelog.astro', import.meta.url), 'utf8');
  assert.match(lib, /glob\('\.\.\/\.\.\/\.\.\/releases\/\*\.md'/);
  assert.match(lib, /glob\('\.\.\/\.\.\/\.\.\/releases\/extended\/\*\.md'/);
  assert.match(lib, /origin === 'extended' \? 'extended\/' \+ version/);
  assert.match(lib, /dateOrder/);
  assert.match(page, /vextended-/);
  assert.match(page, /release\.displayVersion/);
  assert.doesNotMatch(page, /Hardened source|local Hardened source revisions/);
  const source = (await readdir(new URL('../releases/', import.meta.url))).filter(s => s.endsWith('.md'));
  const extended = (await readdir(new URL('../releases/extended/', import.meta.url))).filter(s => s.endsWith('.md'));
  assert.ok(source.length > 0 && extended.length > 0);
});
