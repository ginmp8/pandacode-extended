import assert from 'node:assert/strict';
import test from 'node:test';
import { SecurityWrites, SESSION_ENDED } from '../src/connect/security-writes.ts';

test('deletion waits for already-started key/pin/replay writes, then erases them', async () => {
  for (const name of ['token', 'pins', 'replay']) {
    const writes = new SecurityWrites();
    const store = new Map<string, string>();
    let finish!: () => void;
    const held = writes.write(writes.version, async () => {
      await new Promise<void>(resolve => { finish = resolve; });
      store.set(name, 'old-session');
    });
    const rejected = assert.rejects(held, { message: SESSION_ENDED });
    await Promise.resolve();
    const deletion = writes.transition(async () => { store.clear(); });
    assert.equal(writes.active(writes.version), false);
    finish();
    await Promise.all([deletion, rejected]);
    assert.equal(store.size, 0, `${name} cannot reappear after wipe resolves`);
    assert.equal(writes.active(writes.version), true);
  }
});

test('sign-out cancels queued writes and prevents late old-session refreshes', async () => {
  const writes = new SecurityWrites();
  const old = writes.version;
  const events: string[] = [];
  const queued = writes.write(old, async () => { events.push('save old token'); });
  const rejected = assert.rejects(queued, { message: SESSION_ENDED });
  await writes.transition(async () => { events.push('clear token'); });
  await rejected;
  await assert.rejects(writes.write(old, async () => { events.push('late refresh'); }), { message: SESSION_ENDED });
  await writes.write(writes.version, async () => { events.push('new sign-in'); });
  assert.deepEqual(events, ['clear token', 'new sign-in']);
});

test('a failed clear stays paused and two transitions are serialized', async () => {
  const writes = new SecurityWrites();
  await assert.rejects(writes.transition(async () => { throw new Error('Keychain unavailable'); }), /Keychain/);
  assert.equal(writes.active(writes.version), false);
  await assert.rejects(writes.write(writes.version, async () => undefined), { message: SESSION_ENDED });
  const events: number[] = [];
  await Promise.all([
    writes.transition(async () => { events.push(1); }),
    writes.transition(async () => { events.push(2); }),
  ]);
  assert.deepEqual(events, [1, 2]);
});
