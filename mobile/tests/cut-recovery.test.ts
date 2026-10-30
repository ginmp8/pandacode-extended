/// <reference types="node" />
import assert from 'node:assert/strict';
import test from 'node:test';
import { newestCutId } from '../src/connect/cut-recovery.ts';

test('prioritizes the newest cut message when an older cached page was also truncated', () => {
  const history = [{ id: 4, cut: true }, { id: 7, cut: false }, { id: 10, cut: true }, { id: 12, cut: false }];
  assert.equal(newestCutId(history), 10);
  assert.deepEqual(history.map(message => message.cut), [true, false, true, false], 'selection is read-only');
});

test('does not refetch when no messages are cut', () => {
  assert.equal(newestCutId([{ id: 4 }, { id: 8, cut: false }]), undefined);
  assert.equal(newestCutId([]), undefined);
  assert.equal(newestCutId(undefined), undefined);
});

test('selects an oversized most-recent message before older truncated messages', () => {
  assert.equal(newestCutId([{ id: 1, cut: true }, { id: 80, cut: true }, { id: 100, cut: true }]), 100);
});
