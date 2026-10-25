import test from 'node:test';
import assert from 'node:assert/strict';
import { compareVersion, checkUpstream } from './check-upstream-current.mjs';

test('compares upstream semver, not lexicographic strings', () => {
  assert.equal(compareVersion('v0.3.42', '0.3.41'), 1);
  assert.equal(compareVersion('v0.3.42', '0.3.42'), 0);
  assert.equal(compareVersion('v0.3.9', '0.3.42'), -1);
  assert.throws(() => compareVersion('v0.3.43-rc1', '0.3.42'));
});
test('reports whether a later upstream release needs manual reconciliation', async () => {
  const fetcher = async () => ({ ok: true, json: async () => ({ tag_name: 'v0.3.48' }) });
  assert.deepEqual(await checkUpstream(fetcher), { latest: 'v0.3.48', tracked: '0.3.47', behind: true });
  const same = async () => ({ ok: true, json: async () => ({ tag_name: 'v0.3.47' }) });
  assert.equal((await checkUpstream(same)).behind, false);
});
test('fails rather than interpreting malformed or unavailable upstream metadata as current', async () => {
  await assert.rejects(checkUpstream(async () => ({ ok: false, status: 403 })), /HTTP 403/);
  await assert.rejects(checkUpstream(async () => ({ ok: true, json: async () => ({ tag_name: 'not-a-tag' }) })), /Unexpected/);
});
