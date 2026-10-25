import assert from 'node:assert/strict';
import test from 'node:test';
import { filterInbox, inboxProjects, projectKey } from '../src/store/inbox-filter.ts';
import { botRows, inboxSections, type InboxInput } from '../src/store/inbox.ts';
import type { TaskSummary } from '../src/connect/types.ts';

const task = (sessionId: string, status = 'idle', extra: Partial<TaskSummary> = {}): TaskSummary => ({
  sessionId, title: sessionId, status, projectId: 'p', projectName: 'Website', provider: 'claude', model: 'default',
  effort: null, mode: 'ask', pinned: false, updatedAt: 900, lastPromptAt: null, ...extra,
});
function fixture() {
  const input: InboxInput = {
    hosts: [{ id: 'mac', name: 'Studio' }, { id: 'pc', name: 'Office' }],
    tasks: { mac: { tasks: [task('build', 'running'), task('approve', 'waiting'), task('docs', 'idle', { projectId: 'docs', projectName: 'Docs' })], projects: [] },
      pc: { tasks: [task('other')], projects: [] } },
    live: {}, snoozed: {}, pins: {}, outbox: [{ id: 'o', deviceId: 'mac', sessionId: 'build', text: 'queued prompt', createdAt: 950 }], now: 1000,
  };
  const inbox = inboxSections(input);
  const bots = botRows(input.hosts, { mac: [{ id: 'helper', name: 'Helper', avatar: null, state: 'ready', paused: false, desktopSession: null, projectId: 'bot-ws' }] }, input.tasks);
  return { inbox, bots };
}

test('computer and project filters compose without merging status or unsent sections', () => {
  const { inbox, bots } = fixture();
  const selected = filterInbox(inbox, bots, { query: '', deviceId: 'mac', project: projectKey('mac', 'p') });
  assert.deepEqual(selected.inbox.running.map(r => r.task.sessionId), ['build']);
  assert.deepEqual(selected.inbox.needs.map(r => r.task.sessionId), ['approve']);
  assert.deepEqual(selected.inbox.unsent.map(r => [r.outboxId, r.text]), [['o', 'queued prompt']]);
  assert.deepEqual(selected.inbox.recent, []);
  assert.deepEqual(selected.bots, []);
  assert.equal(inbox.recent.length, 2, 'view filtering does not mutate source state');
});

test('search covers task project/computer names and bots; clearing restores every row', () => {
  const { inbox, bots } = fixture();
  assert.deepEqual(filterInbox(inbox, bots, { query: ' DOCS ', deviceId: '', project: '' }).inbox.recent.map(r => r.task.sessionId), ['docs']);
  assert.deepEqual(filterInbox(inbox, bots, { query: 'office', deviceId: '', project: '' }).inbox.recent.map(r => r.task.sessionId), ['other']);
  assert.deepEqual(filterInbox(inbox, bots, { query: 'HELP', deviceId: '', project: '' }).bots.map(r => r.bot.name), ['Helper']);
  const none = filterInbox(inbox, bots, { query: 'no matching name', deviceId: '', project: '' });
  assert.ok(Object.values(none.inbox).every(rows => rows.length === 0));
  assert.deepEqual(none.bots, []);
  assert.deepEqual(filterInbox(inbox, bots, { query: '', deviceId: '', project: '' }), { inbox, bots });
});

test('project options distinguish equal IDs on separate hosts and include bot workspaces', () => {
  const { inbox, bots } = fixture();
  const options = inboxProjects(inbox, bots, '');
  assert.ok(options.some(p => p.value === projectKey('mac', 'p') && p.label === 'Website · Studio'));
  assert.ok(options.some(p => p.value === projectKey('pc', 'p') && p.label === 'Website · Office'));
  assert.deepEqual(inboxProjects(inbox, bots, 'pc'), [{ value: projectKey('pc', 'p'), label: 'Website' }]);
  assert.equal(inboxProjects(inbox, bots, 'mac').length, 3);
  assert.deepEqual(filterInbox(inbox, bots, { query: '', deviceId: 'mac', project: projectKey('mac', 'bot-ws') }).bots.map(r => r.bot.id), ['helper']);
  assert.ok(Object.values(filterInbox(inbox, bots, { query: '', deviceId: 'pc', project: projectKey('mac', 'p') }).inbox).every(rows => !rows.length));
});
