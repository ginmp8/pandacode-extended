import assert from 'node:assert/strict';
import test from 'node:test';
import { PersistentReplay, type ReplayStore } from '../src/connect/replay.ts';

const now = 1_000_000;
const nonce = (id: number) => `unique-nonce-${String(id).padStart(8, '0')}`;
function storage(raw: string | null = null) {
  return { raw, writes: 0, readError: false, writeError: false,
    async read() { if (this.readError) throw new Error('read failed'); return this.raw; },
    async write(value: string) { this.writes++; if (this.writeError) throw new Error('write failed'); this.raw = value; },
  };
}

test('receipt persistence completes before acceptance and duplicates are refused', async () => {
  const store = storage();
  const replay = new PersistentReplay(store);
  await assert.rejects(replay.check(now, nonce(1), now), /unavailable/);
  await replay.load();
  assert.equal(await replay.check(now, nonce(1), now), true);
  assert.deepEqual(JSON.parse(store.raw!), { timestamp: now, receivedAt: now });
  assert.equal(await replay.check(now, nonce(1), now + 1), false);
  assert.equal(store.writes, 1);
  assert.equal(await replay.check(now - 1, nonce(2), now + 1), true, 'fresh out-of-order replies work within the launch');
  assert.equal(JSON.parse(store.raw!).timestamp, now, 'the checkpoint never moves backwards');
});

test('a restart refuses every previously accepted timestamp and catches up with fresh frames', async () => {
  const store = storage();
  const first = new PersistentReplay(store);
  await first.load();
  assert.equal(await first.check(now, nonce(1), now), true);
  const restarted = new PersistentReplay(store);
  await restarted.load();
  assert.equal(await restarted.check(now, nonce(1), now + 1), false);
  assert.equal(await restarted.check(now, nonce(2), now + 1), false, 'equal-ms ambiguity fails closed after restart');
  assert.equal(await restarted.check(now + 1, nonce(3), now + 1), true);
  assert.equal(await restarted.check(now + 2, nonce(4), now), false, 'clock rollback cannot reopen the replay window');
});

test('missing state is allowed, corrupted or unreadable state is never reset', async () => {
  for (const raw of ['bad', 'null', '[]', '{}', '{"timestamp":-1,"receivedAt":0}', '{"timestamp":1.5,"receivedAt":0}']) {
    const store = storage(raw);
    const replay = new PersistentReplay(store);
    await assert.rejects(replay.load(), /replay checkpoint/);
    await assert.rejects(replay.check(now, nonce(1), now), /unavailable/);
    assert.equal(store.writes, 0);
    assert.equal(store.raw, raw);
  }
  const store = storage();
  store.readError = true;
  await assert.rejects(new PersistentReplay(store).load(), /read failed/);
});

test('storage failures permanently stop admission for the launch', async () => {
  const store = storage();
  const replay = new PersistentReplay(store);
  await replay.load();
  store.writeError = true;
  await assert.rejects(replay.check(now, nonce(1), now), /without accepting/);
  store.writeError = false;
  await assert.rejects(replay.check(now + 1, nonce(2), now + 1), /unavailable/);
  assert.equal(store.raw, null);
});

test('an in-progress secure write cannot apply its frame early', async () => {
  let finish!: () => void;
  const store: ReplayStore = { read: async () => null, write: () => new Promise(resolve => { finish = resolve; }) };
  const replay = new PersistentReplay(store);
  await replay.load();
  let applied = false;
  const accepted = replay.check(now, nonce(1), now).then(ok => { applied = ok; });
  await Promise.resolve();
  assert.equal(applied, false);
  finish();
  await accepted;
  assert.equal(applied, true);
});
