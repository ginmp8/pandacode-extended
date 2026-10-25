import test from 'node:test';
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';

const notes = new URL('../releases/', import.meta.url);
const extended = new URL('../releases/extended/', import.meta.url);
const datesFile = new URL('../website/src/data/release-dates.json', import.meta.url);
const historicalUndated = new Set(['extended/0.3.35-hardened.3', 'extended/0.3.35-hardened.4']);
const legacy = ['0.3.35-hardened.3', '0.3.35-hardened.4', '0.3.35.3', '0.3.35.4', '0.3.41-hardened', '0.3.42-hardened'];

test('original releases remain at their original paths and every Extended history entry moves into its own folder', async () => {
  const root = (await readdir(notes)).filter(name => name.endsWith('.md'));
  const local = (await readdir(extended)).filter(name => name.endsWith('.md'));
  assert.ok(root.length > 0);
  assert.ok(root.every(name => /^\d+\.\d+\.\d+\.md$/.test(name)), 'root contains an Extended release or invalid filename');
  assert.ok(root.includes('0.3.42.md'));
  assert.ok(local.includes('0.4.0.md'));
  assert.deepEqual([...local].sort(), [...legacy.map(version => version + '.md'), '0.4.0.md'].sort(), 'Extended history must contain exactly the reviewed legacy notes and the current release');
  for (const v of legacy) {
    assert.ok(local.includes(v + '.md'), 'missing migrated local history ' + v);
    assert.ok(!root.includes(v + '.md'), 'legacy local release still in original namespace ' + v);
  }
  assert.equal(new Set([...root.map(name => 'upstream/' + name), ...local.map(name => 'extended/' + name)]).size, root.length + local.length);
});

test('every dated source note has a uniquely namespaced ISO date, without inventing historical release dates', async () => {
  const dates = JSON.parse(await readFile(datesFile, 'utf8'));
  const upstream = (await readdir(notes)).filter(name => name.endsWith('.md')).map(name => name.slice(0, -3));
  const local = (await readdir(extended)).filter(name => name.endsWith('.md')).map(name => 'extended/' + name.slice(0, -3));
  const keys = [...upstream, ...local];
  assert.deepEqual(keys.filter(key => !historicalUndated.has(key) && !/^\d{4}-\d{2}-\d{2}$/.test(dates[key] ?? '')), []);
  assert.deepEqual(Object.keys(dates).filter(key => !keys.includes(key)), [], 'orphan/ambiguous release date key');
  assert.deepEqual(local.filter(key => !dates[key]).sort(), [...historicalUndated].sort());
  assert.equal(dates['0.3.42'], '2026-10-25');
  assert.equal(dates['extended/0.4.0'], '2026-10-25');
  assert.equal(dates['0.4.0'], undefined, 'Extended and original releases must not share an unqualified date key');
});
