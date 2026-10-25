// Connect secrets in the OS secure store (Keychain / Android Keystore): relay token,
// X25519 private key, device id, and the account shown while signed in.
import * as SecureStore from 'expo-secure-store';
import { decodeSecret, generateSecret, uuid } from './crypto.ts';
import { build } from './config.ts';
import { parsePins, type Pins } from './trust.ts';
import type { Account } from './types.ts';

const KEYS = { token: 'connect.token', key: 'connect.key', device: 'connect.device', account: 'connect.account', pins: 'connect.pins', replay: 'connect.replay' } as const;
const OPTIONS: SecureStore.SecureStoreOptions = { keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY };

export const get = (name: string) => SecureStore.getItemAsync(name, OPTIONS);
export const set = (name: string, value: string) => SecureStore.setItemAsync(name, value, OPTIONS);
export const remove = (name: string) => SecureStore.deleteItemAsync(name, OPTIONS);

export const getPins = async (): Promise<Pins> => parsePins(await get(KEYS.pins));
export const setPins = (pins: Pins) => set(KEYS.pins, JSON.stringify(parsePins(JSON.stringify(pins))));
export const replayStore = { read: () => get(KEYS.replay), write: (value: string) => set(KEYS.replay, value) };

/** Bind credentials to their configured relay; rebuilding for another relay cannot leak an old token. */
export async function getToken(): Promise<string | null> {
  const raw = await get(KEYS.token);
  if (raw === null) return null;
  let value: { relay?: unknown; token?: unknown };
  try { value = JSON.parse(raw); } catch { throw new Error('Saved sign-in is unreadable. Sign out before signing in again.'); }
  if (!value || typeof value.token !== 'string' || value.relay !== build?.relay) throw new Error('Saved sign-in belongs to a different relay. Sign out before signing in again.');
  return value.token;
}
export const setToken = (token: string) => {
  if (!build) throw new Error('Configure Connect before saving a sign-in.');
  return set(KEYS.token, JSON.stringify({ relay: build.relay, token }));
};

export async function getAccount(): Promise<Account | null> {
  const raw = await get(KEYS.account);
  if (raw === null) return null;
  let value: Account;
  try { value = JSON.parse(raw); } catch { throw new Error('Saved account data is unreadable.'); }
  if (!value || typeof value.id !== 'string' || typeof value.login !== 'string') throw new Error('Saved account data is invalid.');
  return value;
}
export const setAccount = (account: Account) => set(KEYS.account, JSON.stringify(account));

/** Signing out drops the token and account; the device id and key stay for the next sign-in. */
export const clearSession = () => Promise.all([remove(KEYS.token), remove(KEYS.account)]);

export async function getDeviceId(): Promise<string> {
  const id = await get(KEYS.device);
  if (id !== null && !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(id)) throw new Error('The saved device identity is invalid. It has not been replaced.');
  return id ?? newDeviceId();
}

/** A fresh device id only when none exists, or following explicit account deletion. */
export async function newDeviceId(): Promise<string> {
  const id = uuid();
  await set(KEYS.device, id);
  return id;
}

let keyLock: Promise<Uint8Array> | null = null;

/** After account deletion: token, account, device id and key go; the next sign-in makes new ones. */
export async function wipe(): Promise<void> {
  await Promise.all(Object.values(KEYS).map(remove));
  keyLock = null;
}

/** A malformed/unreadable existing key fails closed; only a genuinely missing key is generated. */
export function getSecret(): Promise<Uint8Array> {
  keyLock ??= (async () => {
    const raw = await get(KEYS.key);
    if (raw !== null) {
      const existing = decodeSecret(raw);
      if (!existing) throw new Error('The saved device key is invalid. It has not been replaced.');
      return existing;
    }
    if (await get(KEYS.device) !== null) throw new Error('This phone has a saved identity but its private key is missing. It has not been replaced.');
    const fresh = generateSecret();
    await set(KEYS.key, fresh);
    return decodeSecret(fresh)!;
  })().catch(error => {
    keyLock = null;
    throw error;
  });
  return keyLock;
}
