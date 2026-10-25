import test from 'node:test';
import assert from 'node:assert/strict';
import { check } from './check-hardened-sync.mjs';

test('Hardened version, explicit upstream ancestry, layout patch and release history stay consistent', async () => {
  assert.deepEqual(await check(), { version: '0.3.42-hardened', upstream: '0.3.42', sourceBase: '0.3.28', upstreamNotes: 14 });
});
