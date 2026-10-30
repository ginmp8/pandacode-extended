import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { patchKey, splitPatch, noteChangedPaths, likelyFileNames } from '../src/diff-prefetch.ts';

test('diff prefetch uses bounded basename-only language hints', () => {
  noteChangedPaths(Array.from({ length: 55 }, (_, i) => `src/deep/${i}.ts`));
  assert.equal(likelyFileNames().length, 50);
  assert.deepEqual(likelyFileNames().slice(0, 2), ['0.ts', '1.ts']);
});

test('patch cache key is stable and sensitive to path, length and content', () => {
  const one = patchKey('a.ts', 'a\nb');
  assert.equal(one, patchKey('a.ts', 'a\nb'));
  assert.notEqual(one, patchKey('b.ts', 'a\nb'));
  assert.notEqual(one, patchKey('a.ts', 'a\nc'));
});

test('splitPatch retains separate file sections and does not concatenate unrelated files', () => {
  const patch = [
    'diff --git a/first.ts b/first.ts',
    '--- a/first.ts', '+++ b/first.ts', '@@ -1 +1 @@', '-old', '+new',
    'diff --git a/second.ts b/second.ts',
    '--- a/second.ts', '+++ b/second.ts', '@@ -1 +1 @@', '-before', '+after',
    '',
  ].join('\n');
  const chunks = splitPatch(patch);
  assert.equal(chunks.length, 2);
  assert.equal(chunks[0].continued, false);
  assert.equal(chunks[1].continued, false);
  assert.ok(chunks[0].text.includes('first.ts'));
  assert.ok(chunks[1].text.startsWith('diff --git a/second.ts'));
  assert.ok(!chunks[0].text.includes('second.ts'));
});
