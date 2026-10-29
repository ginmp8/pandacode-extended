/// <reference types="node" />
import assert from 'node:assert/strict';
import test from 'node:test';
import { startNote } from '../src/connect/start-note.ts';

test('a worktree fallback note is one short line, and older hosts send none', () => {
  assert.equal(startNote({ sessionId: 's' }), undefined);
  assert.equal(startNote(null), undefined);
  assert.equal(startNote({ note: 42 }), undefined);
  assert.equal(startNote({ note: '  ' }), undefined);
  assert.equal(startNote({ sessionId: 's', worktree: true, note: 'Started in a worktree: Studio has an agent working in orbit.\nmore' }), 'Started in a worktree: Studio has an agent working in orbit.');
  assert.equal(startNote({ note: 'x'.repeat(500) })?.length, 300);
});

test('untrusted remote fallback explanations cannot inject a second line', () => {
  const prefix = 'Started in a worktree: Studio is busy.';
  for (const separator of ['\r', '\r\n', '\u2028', '\u2029']) {
    assert.equal(startNote({ note: prefix + separator + '/private/host/workspace' }), prefix);
  }
});
