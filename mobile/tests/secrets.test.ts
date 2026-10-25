import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import test from 'node:test';

const backend = {
  values: new Map<string, string>(), writes: [] as string[], readsFail: false, writesFail: false, deletesFail: false,
  async get(name: string) { if (this.readsFail) throw new Error('Keychain locked'); return this.values.get(name) ?? null; },
  async set(name: string, value: string) { if (this.writesFail) throw new Error('Keychain write failed'); this.writes.push(name); this.values.set(name, value); },
  async remove(name: string) { if (this.deletesFail) throw new Error('Keychain delete failed'); this.values.delete(name); },
};
(globalThis as unknown as { __mobileSecureStore: typeof backend }).__mobileSecureStore = backend;

// Exercise the actual secret policy against OS-storage failures without a native
// runtime or a network request. Only platform adapters/config are replaced.
const hooks = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === 'expo-secure-store' || specifier === 'expo-constants') return { url: `test-mobile:${specifier}`, shortCircuit: true };
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === 'test-mobile:expo-secure-store') return {
      format: 'module', shortCircuit: true,
      source: `const store = globalThis.__mobileSecureStore;
        export const AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY = 7;
        export const getItemAsync = name => store.get(name);
        export const setItemAsync = (name, value) => store.set(name, value);
        export const deleteItemAsync = name => store.remove(name);`,
    };
    if (url === 'test-mobile:expo-constants') return {
      format: 'module', shortCircuit: true,
      source: `export default { expoConfig: { extra: { relayUrl: 'https://relay.example', githubClientId: 'client' } } };`,
    };
    return nextLoad(url, context);
  },
});
const secrets = await import('../src/connect/secrets.ts');
hooks.deregister();

test('unreadable or corrupt private keys are preserved; only missing keys are created', async () => {
  backend.values.set('connect.key', 'corrupted');
  await assert.rejects(secrets.getSecret(), /has not been replaced/);
  assert.equal(backend.values.get('connect.key'), 'corrupted');
  assert.equal(backend.writes.length, 0);
  backend.readsFail = true;
  await assert.rejects(secrets.getSecret(), /Keychain locked/);
  assert.equal(backend.writes.length, 0);
  backend.readsFail = false;
  backend.values.delete('connect.key');
  backend.values.set('connect.device', '3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f');
  await assert.rejects(secrets.getSecret(), /private key is missing/);
  assert.equal(backend.values.has('connect.key'), false, 'missing key of an existing device is not an initial install');
  backend.values.delete('connect.device');
  backend.writesFail = true;
  await assert.rejects(secrets.getSecret(), /write failed/);
  backend.writesFail = false;
  const secret = await secrets.getSecret();
  assert.equal(secret.byteLength, 32);
  assert.equal(backend.writes.filter(name => name === 'connect.key').length, 1);
  assert.deepEqual(await secrets.getSecret(), secret);
});

test('saved identity and pins fail closed instead of receiving a replacement', async () => {
  const writesBefore = backend.writes.length;
  backend.values.set('connect.device', 'not-a-uuid');
  await assert.rejects(secrets.getDeviceId(), /not been replaced/);
  backend.values.set('connect.pins', '{damaged');
  await assert.rejects(secrets.getPins(), /unreadable/);
  backend.values.set('connect.account', 'null');
  await assert.rejects(secrets.getAccount(), /invalid/);
  assert.equal(backend.writes.length, writesBefore);
  assert.equal(backend.values.get('connect.pins'), '{damaged');
});

test('tokens remain bound to the explicitly configured relay', async () => {
  backend.values.delete('connect.token');
  assert.equal(await secrets.getToken(), null);
  await secrets.setToken('token');
  assert.equal(await secrets.getToken(), 'token');
  assert.deepEqual(JSON.parse(backend.values.get('connect.token')!), { relay: 'https://relay.example', token: 'token' });
  backend.values.set('connect.token', JSON.stringify({ relay: 'https://other.example', token: 'old-secret' }));
  await assert.rejects(secrets.getToken(), /different relay/);
  backend.values.set('connect.token', 'legacy-unbound-token');
  await assert.rejects(secrets.getToken(), /unreadable/);
});

test('ordinary sign-out preserves identity and pins and reports deletion failures', async () => {
  const key = backend.values.get('connect.key');
  const pins = backend.values.get('connect.pins');
  backend.deletesFail = true;
  await assert.rejects(secrets.clearSession(), /delete failed/);
  backend.deletesFail = false;
  await secrets.clearSession();
  assert.equal(backend.values.has('connect.token'), false);
  assert.equal(backend.values.has('connect.account'), false);
  assert.equal(backend.values.get('connect.key'), key);
  assert.equal(backend.values.get('connect.pins'), pins);
  await secrets.wipe();
  assert.equal(backend.values.size, 0, 'only explicit account deletion erases the complete identity');
});
