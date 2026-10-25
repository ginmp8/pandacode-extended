// Presence describes availability. Only an explicitly pinned public key grants trust.
import { decodeSecret } from './crypto.ts';
import type { Device } from './types.ts';

export type Pins = Record<string, string>;
export const NOT_TRUSTED = 'Trust this computer in Computers after comparing its fingerprint on both devices.';
export const KEY_CHANGED = 'This computer has a different key. Compare its fingerprint and explicitly trust the new key.';
const validId = (id: string) => /^[a-zA-Z0-9_-]{1,200}$/.test(id) && !['__proto__', 'constructor', 'prototype'].includes(id);

export function parsePins(raw: string | null): Pins {
  if (raw === null) return {};
  let value: unknown;
  try { value = JSON.parse(raw); } catch { throw new Error('Saved device trust is unreadable. No computer has been authorized.'); }
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length > 64) throw new Error('Saved device trust is invalid.');
  const result: Pins = {};
  for (const [id, key] of Object.entries(value)) {
    if (!validId(id) || typeof key !== 'string' || !decodeSecret(key)) throw new Error('Saved device trust is invalid.');
    result[id] = key;
  }
  return result;
}

export function trusted(device: Pick<Device, 'id' | 'publicKey'>, pins: Pins): boolean {
  return Object.hasOwn(pins, device.id) && pins[device.id] === device.publicKey && decodeSecret(device.publicKey) !== null;
}

export function requireTrusted(device: Pick<Device, 'id' | 'publicKey'>, pins: Pins): void {
  if (!trusted(device, pins)) throw new Error(Object.hasOwn(pins, device.id) ? KEY_CHANGED : NOT_TRUSTED);
}

/** The key displayed by the confirmation must still be the key presence lists now. */
export function changedPins(pins: Pins, device: Pick<Device, 'id' | 'publicKey'>, expectedKey: string, allow: boolean): Pins {
  if (!validId(device.id) || !decodeSecret(expectedKey) || device.publicKey !== expectedKey) throw new Error(KEY_CHANGED);
  const next = { ...pins };
  if (allow) next[device.id] = expectedKey;
  else delete next[device.id];
  if (Object.keys(next).length > 64) throw new Error('The trusted-device limit was reached. Remove an unused trust entry first.');
  return next;
}

/** Authorize before and after the asynchronous durable replay receipt. A revoked,
 * replaced or signed-out peer cannot apply a response when storage finishes later. */
export async function admitReply(
  peer: Pick<Device, 'id' | 'publicKey'>,
  current: () => { peer: Pick<Device, 'id' | 'publicKey'> | undefined; pins: Pins; active: boolean },
  replay: () => Promise<boolean>,
): Promise<'accepted' | 'stale' | 'unauthorized'> {
  const authorized = () => {
    const now = current();
    return now.active && now.peer?.id === peer.id && now.peer.publicKey === peer.publicKey && trusted(now.peer, now.pins);
  };
  if (!authorized()) return 'unauthorized';
  const fresh = await replay();
  if (!authorized()) return 'unauthorized';
  return fresh ? 'accepted' : 'stale';
}
