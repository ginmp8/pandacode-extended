import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { checkUnzippedSize } from '../src/zip-limit.ts';
import { workspaceImagePath } from '../src/file-links.ts';

const array = bytes => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
test('real Word and spreadsheet fixture archives pass the parser preflight', async () => {
  for (const name of ['sample.docx', 'sample.xlsx']) checkUnzippedSize(array(await readFile(new URL(`./fixtures/${name}`, import.meta.url))), true);
});
test('truncated, over-budget and inconsistent archives are rejected before parsing', async () => {
  const bytes = await readFile(new URL('./fixtures/sample.docx', import.meta.url));
  assert.throws(() => checkUnzippedSize(array(bytes.subarray(0, -4)), true));
  assert.throws(() => checkUnzippedSize(array(Buffer.from('not ZIP')), true));
  const changed = Buffer.from(bytes), central = changed.indexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02]));
  assert.ok(central >= 0); changed.writeUInt32LE(0xffffffff, central + 24);
  assert.throws(() => checkUnzippedSize(array(changed), true), /preview limit/);
  const inconsistent = Buffer.from(bytes); inconsistent.writeUInt32LE(0xffffffff, central + 42);
  assert.throws(() => checkUnzippedSize(array(inconsistent), true));
});
test('document image paths resolve against their folder and cannot escape the workspace', () => {
  assert.equal(workspaceImagePath('../images/chart.png', 'docs/guide.md'), 'images/chart.png');
  assert.equal(workspaceImagePath('/images/chart.png', 'docs/guide.md'), 'images/chart.png');
  for (const path of ['../../private.png', 'https://example.com/a.png', '//example.com/a.png', '%2f%2fexample.com/a.png', 'data:image/png;base64,AAAA', '../.git/private.png', '..\\private.png', '../bad%00.png', '../picture.svg']) assert.equal(workspaceImagePath(path, 'docs/guide.md'), undefined, path);
});
