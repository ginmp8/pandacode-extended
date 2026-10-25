import test from 'node:test';
import assert from 'node:assert/strict';
import { check } from './check-hardened-sync.mjs';

test('Extended core/display version, historical ancestry, layout patch and separated release histories stay consistent', async () => {
  assert.deepEqual(await check(), { version: '0.4.0', display: '0.4.0+pc.0.3.42', upstream: '0.3.42', sourceBase: '0.3.28', upstreamNotes: 14, extendedNotes: 8 });
});
