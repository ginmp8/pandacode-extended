import test from 'node:test';
import assert from 'node:assert/strict';
import { readSoulFile, MAX_SOUL_BYTES } from '../src/bot-soul.ts';

test('personality imports preserve UTF-8 markdown and remove a leading BOM', async () => {
  const file = new Blob(['\uFEFF# Scout\nDirect, warm — precise.']);
  assert.equal(await readSoulFile(file), '# Scout\nDirect, warm — precise.');
  assert.equal((await readSoulFile(new Blob(['a'.repeat(MAX_SOUL_BYTES)]))).length, MAX_SOUL_BYTES);
});

test('personality imports refuse invalid UTF-8 and enforce actual byte size', async () => {
  await assert.rejects(readSoulFile(new Blob([new Uint8Array([0xff])])), /UTF-8/);
  await assert.rejects(readSoulFile(new Blob(['é'.repeat(MAX_SOUL_BYTES / 2 + 1)])), /64 KB/);
  let read = false;
  await assert.rejects(readSoulFile({ size: MAX_SOUL_BYTES + 1, arrayBuffer: async () => { read = true; return new ArrayBuffer(0); } }), /64 KB/);
  assert.equal(read, false, 'oversized files are refused before reading');
  await assert.rejects(readSoulFile({ size: 0, arrayBuffer: async () => new ArrayBuffer(MAX_SOUL_BYTES + 1) }), /64 KB/);
});
