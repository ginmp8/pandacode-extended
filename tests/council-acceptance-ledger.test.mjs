import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

test('Council #147 acceptance ledger lists every requirement without claiming unexecuted passes', () => {
  const ledger = readFileSync(new URL('../docs/council-147-acceptance-ledger.md', import.meta.url), 'utf8');
  const ids = [...ledger.matchAll(/^\| (AC-\d{2}) \|/gm)].map(match => match[1]);
  const expected = Array.from({ length: 40 }, (_, i) => `AC-${String(i + 1).padStart(2, '0')}`);
  assert.deepEqual(ids, expected, 'every AC must appear exactly once and in the frozen issue order');
  assert.match(ledger, /IMPLEMENTATION UNVERIFIED/);
  assert.match(ledger, /exact remote PR HEAD/);
  assert.match(ledger, /positive.*standalone task-client terminal/);
  assert.match(ledger, /No row below claims a passing validation/);
});
