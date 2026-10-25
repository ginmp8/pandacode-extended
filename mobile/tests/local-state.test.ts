import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import test from 'node:test';

function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
const storage = new Map<string, string>();
const adapter = {
  get: async (key: string) => storage.get(key) ?? null,
  set: async (key: string, value: string) => { storage.set(key, value); },
  remove: async (key: string) => { storage.delete(key); },
  send: async (_device: string, _session: string, _text: string) => ({ queued: false }),
};
(globalThis as any).__localAdapter = adapter;
const hooks = registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith('/connect/client.ts')) return { format: 'module', shortCircuit: true, source: `const a = globalThis.__localAdapter; export const connect = { sendMessage: (...args) => a.send(...args), loadMessages: async () => {} };` };
    if (url.endsWith('/connect/secrets.ts')) return { format: 'module', shortCircuit: true, source: `const a = globalThis.__localAdapter; export const get = key => a.get(key); export const set = (key, value) => a.set(key, value); export const remove = key => a.remove(key);` };
    if (url.endsWith('/connect/config.ts')) return { format: 'module', shortCircuit: true, source: `export const build = null;` };
    return nextLoad(url, context);
  },
});
const { useConnect, initialState } = await import('../src/store/index.ts');
const local = await import('../src/store/local.ts');
hooks.deregister();
const key = btoa(String.fromCharCode(...new Uint8Array(32).fill(7)));
const entry = { id: 'm1', deviceId: 'computer', sessionId: 'task', text: 'Account one private prompt', createdAt: 1 };
const tick = () => new Promise<void>(resolve => setImmediate(resolve));
async function setup() {
  await local.clearLocal();
  useConnect.setState({ ...initialState(), ready: true, signedIn: true, connected: true, account: { id: 'gh:1', login: 'one', avatarUrl: '' },
    devices: [{ id: 'computer', name: 'Computer', publicKey: key, host: true, online: true, platform: 'linux', lastSeen: 0 }], pins: { computer: key } }, true);
  local.useLocal.setState({ loaded: true, outbox: [entry] });
}

test('an outbox delivery is durably marked uncertain before the network send', async () => {
  await setup();
  let sent = 0;
  adapter.send = async () => {
    const saved = JSON.parse(storage.get('inbox.outbox')!);
    assert.equal(saved.accountId, 'gh:1');
    assert.match(saved.entries[0].error, /Delivery was started/);
    sent++;
    return { queued: false };
  };
  await local.flushOutbox();
  await tick();
  assert.equal(sent, 1);
  assert.deepEqual(local.useLocal.getState().outbox, []);
  assert.deepEqual(JSON.parse(storage.get('inbox.outbox')!).entries, []);
});

test('a failed acknowledgement save retains the uncertain marker for the next launch', async () => {
  await setup();
  const original = adapter.set;
  adapter.send = async () => {
    adapter.set = async () => { throw new Error('Keychain unavailable'); };
    return { queued: false };
  };
  try {
    await local.flushOutbox();
    await tick();
    assert.match(JSON.parse(storage.get('inbox.outbox')!).entries[0].error, /Delivery was started/);
    local.useLocal.setState({ outbox: [] });
    await local.loadLocal();
    let repeated = 0;
    adapter.send = async () => { repeated++; return { queued: false }; };
    await local.flushOutbox();
    assert.equal(repeated, 0, 'an uncertain prior send is never retried automatically');
  } finally { adapter.set = original; }
});

test('account switching stops an active flush before it sends the next old prompt', async () => {
  await setup();
  local.useLocal.setState({ outbox: [entry, { ...entry, id: 'm2', text: 'second old prompt', createdAt: 2 }] });
  const sent = deferred<void>(), answer = deferred<{ queued: boolean }>();
  let sends = 0;
  adapter.send = async () => { sends++; sent.resolve(); return answer.promise; };
  const flushing = local.flushOutbox();
  await sent.promise;
  useConnect.setState({ account: { id: 'gh:2', login: 'two', avatarUrl: '' } });
  answer.resolve({ queued: false });
  await flushing;
  assert.equal(sends, 1);
  assert.deepEqual(local.useLocal.getState().outbox, []);
  await local.loadLocal();
  assert.deepEqual(local.useLocal.getState().outbox, []);
});

test('repeated local loads do not duplicate a persisted outbox entry', async () => {
  await setup();
  storage.set('inbox.outbox', JSON.stringify({ accountId: 'gh:1', entries: [entry, entry] }));
  local.useLocal.setState({ outbox: [] });
  await Promise.all([local.loadLocal(), local.loadLocal()]);
  assert.deepEqual(local.useLocal.getState().outbox.map(item => item.id), ['m1']);
});
