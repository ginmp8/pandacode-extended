import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const catalog = JSON.parse(readFileSync(new URL('../src/recommended-skills.v1.json', import.meta.url), 'utf8'));

test('recommended skill references are versioned, unique and never embed package contents', () => {
  assert.equal(catalog.version, 1);
  assert.equal(catalog.kind, 'github-skill-recommendations');
  assert.equal(catalog.templates.length, 19);
  assert.equal(catalog.templates.filter(entry => !entry.optional).length, 15);
  assert.equal(catalog.templates.filter(entry => entry.optional).length, 4);
  assert.equal(new Set(catalog.templates.map(entry => entry.id)).size, 19);
  for (const entry of catalog.templates) {
    assert.match(entry.id, /^[a-z][a-z0-9-]+$/);
    assert.equal(entry.repository, 'ginmp8/rhapsodia');
    assert.equal(entry.path, `skills/${entry.id}`);
    assert.equal(entry.gitRef, 'main');
    assert.equal(typeof entry.available, 'boolean');
    assert.ok(!('contents' in entry) && !('files' in entry) && !('base64' in entry));
  }
  assert.deepEqual(
    catalog.templates.filter(entry => !entry.available).map(entry => entry.id),
    ['agent-systems-best-practices'],
    'do not pretend that unavailable packages are importable',
  );
  for (const id of ['mago', 'magia', 'nomia']) assert.ok(!catalog.templates.some(entry => entry.id === id));
});
