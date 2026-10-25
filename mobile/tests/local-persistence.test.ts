import assert from 'node:assert/strict';
import test from 'node:test';
import { LocalPersistence, type LocalData, type LocalStore } from '../src/store/local-persistence.ts';

const data: LocalData = { snoozed: {}, pins: { task: true }, outbox: [{ id: 'm1', deviceId: 'computer', sessionId: 'task', text: 'Private prompt', createdAt: 1 }] };
function backend() {
  const values = new Map<string, string>();
  return { values, async get(key: string) { return values.get(key) ?? null; }, async set(key: string, value: string) { values.set(key, value); }, async remove(key: string) { values.delete(key); } };
}

test('stored outbox and preferences are readable only by the account that saved them', async () => {
  const store = backend(), local = new LocalPersistence(store);
  local.account('gh:1');
  await local.save(data);
  assert.deepEqual(await local.load(), data);
  local.account(null);
  assert.equal(await local.load(), null);
  local.account('gh:2');
  assert.deepEqual(await local.load(), { snoozed: {}, pins: {}, outbox: [] });
  local.account('gh:1');
  assert.deepEqual(await local.load(), data, 'ordinary sign-out can retain this account’s own outbox');
  store.values.set('inbox.outbox', JSON.stringify(data.outbox));
  assert.deepEqual((await local.load())?.outbox, [], 'unowned legacy data is never automatically delivered');
});

test('clear drains writes already started and cancels stale queued saves', async () => {
  const store = backend();
  let release!: () => void;
  let started!: () => void;
  const began = new Promise<void>(resolve => { started = resolve; });
  const gate = new Promise<void>(resolve => { release = resolve; });
  const adapter: LocalStore = { ...store, async set(key, value) { started(); await gate; await store.set(key, value); } };
  const local = new LocalPersistence(adapter);
  local.account('gh:1');
  const writing = local.save(data);
  await began;
  const queued = local.save(data);
  const clearing = local.clear();
  release();
  await Promise.all([writing, queued, clearing]);
  assert.equal(store.values.size, 0);
});

test('a late secure-store read cannot restore erased or previous-account prompts', async () => {
  const store = backend();
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const local = new LocalPersistence({ ...store, async get(key) { const snapshot = await store.get(key); await gate; return snapshot; } });
  local.account('gh:1');
  await local.save(data);
  const reading = local.load();
  await local.clear();
  local.account('gh:2');
  release();
  assert.equal(await reading, null);
  assert.equal(store.values.size, 0);
});

test('queued work from an old account cannot overwrite a new account snapshot', async () => {
  const store = backend(), local = new LocalPersistence(store);
  local.account('gh:1');
  const old = local.save(data);
  local.account('gh:2');
  const fresh = { snoozed: {}, pins: {}, outbox: [] };
  await Promise.all([old, local.save(fresh)]);
  assert.deepEqual(await local.load(), fresh);
  assert.equal(JSON.parse(store.values.get('inbox.outbox')!).accountId, 'gh:2');
});

test('storage erasure failure remains observable and blocks automatic saves', async () => {
  const store = backend();
  const local = new LocalPersistence({ ...store, async remove() { throw new Error('Keychain locked'); } });
  local.account('gh:1');
  await local.save(data);
  await assert.rejects(local.clear(), /Keychain locked/);
  assert.equal(local.current(local.version), false);
  assert.equal(await local.load(), null);
});
