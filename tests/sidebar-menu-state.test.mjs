import test from 'node:test';
import assert from 'node:assert/strict';
import { sidebarMenuChoiceIsCurrent, sidebarRowCanReorder } from '../src/sidebar-menu-state.ts';

const opened = { pinned: false, stage: 'review', unread: true };

test('menu actions apply when their observed task state is still current', () => {
  for (const choice of ['pin', 'stage', 'unread']) {
    assert.equal(sidebarMenuChoiceIsCurrent(choice, opened, { ...opened }), true, choice);
  }
});

test('a later pin from another event cannot be inverted by a stale Pin menu', () => {
  assert.equal(sidebarMenuChoiceIsCurrent('pin', opened, { ...opened, pinned: true }), false);
});

test('a later stage transition cannot be overwritten by an old Settle menu', () => {
  assert.equal(sidebarMenuChoiceIsCurrent('stage', opened, { ...opened, stage: 'pending' }), false);
  assert.equal(sidebarMenuChoiceIsCurrent('stage', opened, { ...opened, stage: 'completed' }), false);
});

test('a later read decision is not toggled by a stale Mark as read menu', () => {
  assert.equal(sidebarMenuChoiceIsCurrent('unread', opened, { ...opened, unread: false }), false);
});

test('unrelated field updates do not invalidate independent menu choices', () => {
  assert.equal(sidebarMenuChoiceIsCurrent('pin', opened, { ...opened, stage: 'completed' }), true);
  assert.equal(sidebarMenuChoiceIsCurrent('stage', opened, { ...opened, unread: false }), true);
  assert.equal(sidebarMenuChoiceIsCurrent('unread', opened, { ...opened, pinned: true }), true);
});

test('a stale reorder menu cannot modify automatically ordered or non-draggable rows', () => {
  assert.equal(sidebarRowCanReorder('all', true), true);
  assert.equal(sidebarRowCanReorder('chats', true), true);
  assert.equal(sidebarRowCanReorder('all', false), false);
  for (const group of ['active', 'review', 'completed', undefined]) {
    assert.equal(sidebarRowCanReorder(group, true), false, String(group));
  }
});
