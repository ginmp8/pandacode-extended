import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import test, { after } from 'node:test';
import { pairKey, publicKey, seal } from '../src/connect/crypto.ts';
import { parseToken } from '../src/connect/token.ts';

const phoneId = '3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f';
const hostId = 'eb52f4a1-a4ec-46ad-bb49-81dbe0148081';
const phoneSecret = new Uint8Array(32).fill(7), hostSecret = new Uint8Array(32).fill(8);
const host = { id: hostId, publicKey: publicKey(hostSecret), name: 'Computer', platform: 'linux', online: true, host: false, lastSeen: 0 };
const token = (extra = 0) => `${Buffer.from(JSON.stringify({ acct: 'gh:1', dev: phoneId, iat: Math.floor(Date.now() / 1000), exp: Math.floor(Date.now() / 1000) + 30 * 86400 + extra })).toString('base64url')}.signature`;
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }

const platform = {
  values: new Map<string, string>(),
  holds: new Map<string, { started: ReturnType<typeof deferred<void>>; finish: ReturnType<typeof deferred<void>> }>(),
  readHolds: new Map<string, { started: ReturnType<typeof deferred<void>>; finish: ReturnType<typeof deferred<void>> }>(),
  handlers: null as any, refresh: false, devices: Promise.resolve([] as unknown[]), thread: Promise.resolve(null as unknown),
  parseToken,
  async get(name: string) {
    const hold = this.readHolds.get(name);
    if (hold) { hold.started.resolve(); await hold.finish.promise; this.readHolds.delete(name); }
    return this.values.get(name) ?? null;
  },
  async set(name: string, value: string) {
    const hold = this.holds.get(name);
    if (hold) { hold.started.resolve(); await hold.finish.promise; this.holds.delete(name); }
    this.values.set(name, value);
  },
  async remove(name: string) { this.values.delete(name); },
  hold(name: string) { const hold = { started: deferred<void>(), finish: deferred<void>() }; this.holds.set(name, hold); return hold; },
  holdRead(name: string) { const hold = { started: deferred<void>(), finish: deferred<void>() }; this.readHolds.set(name, hold); return hold; },
};
(globalThis as any).__clientPlatform = platform;
(globalThis as any).__DEV__ = false;
const mocks: Record<string, string> = {
  'expo-constants': `export default { expoConfig: { extra: { relayUrl: 'https://relay.example', githubClientId: 'client' } } };`,
  'expo-crypto': `export const getRandomValues = value => globalThis.crypto.getRandomValues(value);`,
  'expo-secure-store': `const p = globalThis.__clientPlatform; export const AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY = 7; export const getItemAsync = name => p.get(name); export const setItemAsync = (name, value) => p.set(name, value); export const deleteItemAsync = name => p.remove(name);`,
  'expo-file-system': `export const Paths = { cache: '/cache', document: '/document' }; export class File { exists = false; create() {} delete() {} async text() { return ''; } async write() {} }`,
  'react-native': `export const AppState = { currentState: 'active', addEventListener: () => ({ remove() {} }) }; export const Platform = { OS: 'ios' };`,
};
const sourceMocks: Record<string, string> = {
  '/connect/socket.ts': `export class RelaySocket { open = false; connecting = false; constructor(handlers) { this.handlers = handlers; globalThis.__clientPlatform.handlers = handlers; } start() { this.open = true; this.handlers.onOpen(); } stop() { if (this.open) { this.open = false; this.handlers.onClose(); } } dispose() { this.stop(); } send() { return true; } }`,
  '/connect/auth.ts': `const p = globalThis.__clientPlatform; export const SIGNED_OUT = 'Signed out'; export class SignedOut extends Error {} export const parseToken = p.parseToken; export const expired = value => (parseToken(value)?.exp ?? 0) <= Date.now() / 1000; export const needsRefresh = () => p.refresh; export const refreshToken = async () => p.freshToken; export const listDevices = async () => p.devices; export const revokeDevice = async () => {}; export const startSignIn = async () => ({}); export const finishSignIn = async () => p.signIn; export const deleteAccount = async () => { p.handlers.onRefused(p.sessionToken, true); return { ok: true }; };`,
  '/store/icons.ts': `const s = { cache: {} }; export const useIcons = { getState: () => s, setState: update => Object.assign(s, typeof update === 'function' ? update(s) : update) }; export const loadIcons = async () => {}; export const saveIcons = async () => {}; export const resetIcons = () => { s.cache = {}; }; export const clearIcons = async () => resetIcons();`,
  '/store/threads.ts': `export const cachedLastId = () => undefined; export const clearThreads = async () => {}; export const readThread = async () => globalThis.__clientPlatform.thread; export const saveThread = () => {};`,
  '/store/attachment-files.ts': `export const cachedBlob = () => undefined; export const clearBlobs = async () => {}; export const loadBlobs = async () => {}; export const partial = () => ({ size: 0, append: async () => {}, reset: async () => {}, finish: async () => '/cache/file.png' });`,
  '/connect/inbox-file.ts': `export const clearInbox = async () => {}; export const readInbox = async () => null; export const saveInboxSoon = () => {};`,
};
const hooks = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier in mocks) return { url: `test-client:${specifier}`, shortCircuit: true };
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    const native = mocks[url.replace(/^test-client:/, '')];
    const local = Object.entries(sourceMocks).find(([suffix]) => url.endsWith(suffix))?.[1];
    if (native || local) return { format: 'module', source: native ?? local!, shortCircuit: true };
    return nextLoad(url, context);
  },
});
const secrets = await import('../src/connect/secrets.ts');
const { useConnect } = await import('../src/store/index.ts');
let sequence = 0;
let client: any;
async function setup(pinned = false) {
  if (client) await client.signOut();
  await secrets.wipe();
  platform.holds.clear();
  platform.readHolds.clear();
  platform.refresh = false;
  platform.devices = Promise.resolve([]);
  platform.thread = Promise.resolve(null);
  const currentToken = token();
  Object.assign(platform, { sessionToken: currentToken, freshToken: token(3600) });
  Object.assign(platform, { signIn: { token: currentToken, account: { id: 'gh:1', login: 'user', avatarUrl: '' }, deviceId: phoneId } });
  platform.values.set('connect.token', JSON.stringify({ relay: 'https://relay.example', token: currentToken }));
  platform.values.set('connect.account', JSON.stringify({ id: 'gh:1', login: 'user', avatarUrl: '' }));
  platform.values.set('connect.device', phoneId);
  platform.values.set('connect.key', btoa(String.fromCharCode(...phoneSecret)));
  if (pinned) platform.values.set('connect.pins', JSON.stringify({ [hostId]: host.publicKey }));
  client = (await import(new URL(`../src/connect/client.ts?lifecycle=${++sequence}`, import.meta.url).href)).connect;
  return client;
}
const tick = () => new Promise<void>(resolve => setImmediate(resolve));
async function presence() { platform.handlers.onText(JSON.stringify({ kind: 'presence', devices: [host] })); await tick(); }

test('client deletion drains an in-flight explicit trust write before wiping the pin', async () => {
  const c = await setup();
  await c.init();
  await presence();
  const hold = platform.hold('connect.pins');
  const trust = c.setTrusted(hostId, host.publicKey, true);
  const rejected = assert.rejects(trust, /session has ended/);
  await hold.started.promise;
  const deletion = c.deleteAccount();
  await tick();
  assert.equal(useConnect.getState().signedIn, false);
  hold.finish.resolve();
  await Promise.all([deletion, rejected]);
  assert.equal(platform.values.has('connect.pins'), false);
  assert.equal(platform.values.has('connect.token'), false);
  assert.equal(platform.values.has('connect.account'), false);
  assert.notEqual(platform.values.get('connect.device'), phoneId);
  assert.deepEqual(useConnect.getState().pins, {});
});

test('client sign-out cannot be undone by a delayed token refresh write', async () => {
  const c = await setup();
  platform.refresh = true;
  const hold = platform.hold('connect.token');
  await c.init();
  await hold.started.promise;
  const signOut = c.signOut();
  hold.finish.resolve();
  await signOut;
  await tick();
  assert.equal(platform.values.has('connect.token'), false);
  assert.equal(platform.values.has('connect.account'), false);
  assert.equal(useConnect.getState().signedIn, false);
});

test('client deletion drains an authenticated replay receipt before wiping it', async () => {
  const c = await setup(true);
  await c.init();
  await presence();
  const hold = platform.hold('connect.replay');
  const key = pairKey(hostSecret, hostId, publicKey(phoneSecret), phoneId);
  const body = seal(key, hostId, phoneId, JSON.stringify({ type: 'event', name: 'agent.events', data: { events: [] }, ts: Date.now(), nonce: 'fresh-replay-test-00001' }));
  platform.handlers.onText(JSON.stringify({ from: hostId, to: phoneId, body }));
  await hold.started.promise;
  const deletion = c.deleteAccount();
  await tick();
  hold.finish.resolve();
  await deletion;
  await tick();
  assert.equal(platform.values.has('connect.replay'), false);
  assert.equal(useConnect.getState().signedIn, false);
  assert.equal(useConnect.getState().error, null, 'a stale old-session receipt failure cannot poison the new identity');
});

test('old HTTP device lists and cached transcripts cannot repopulate signed-out state', async () => {
  const c = await setup();
  await c.init();
  const devices = deferred<unknown[]>();
  const transcript = deferred<unknown>();
  platform.devices = devices.promise;
  platform.thread = transcript.promise;
  const discovery = c.refreshDevices();
  const cached = c.peek(hostId, 'private-task');
  await c.signOut();
  devices.resolve([host]);
  transcript.resolve({ messages: [{ id: 1, content: 'private old account transcript' }], hasOlder: false });
  await Promise.all([discovery, cached]);
  assert.deepEqual(useConnect.getState().devices, []);
  assert.deepEqual(useConnect.getState().messages, {});
});

test('cancelling during the final trust read erases the just-saved sign-in', async () => {
  const c = await setup();
  await c.init();
  await c.signOut();
  const code = await c.signInStart();
  const hold = platform.holdRead('connect.pins');
  const finishing = c.signInFinish(code);
  const rejected = assert.rejects(finishing, /cancelled/);
  await hold.started.promise;
  assert.equal(platform.values.has('connect.token'), true, 'the cancellation is after credential persistence');
  c.signInCancel();
  hold.finish.resolve();
  await rejected;
  assert.equal(platform.values.has('connect.token'), false);
  assert.equal(platform.values.has('connect.account'), false);
  assert.equal(useConnect.getState().signedIn, false);
});

after(async () => { if (client) await client.signOut(); hooks.deregister(); });
