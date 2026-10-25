import assert from 'node:assert/strict';
import test from 'node:test';
import { admitReply, changedPins, parsePins, requireTrusted, trusted, type Pins } from '../src/connect/trust.ts';
import { fingerprint } from '../src/connect/crypto.ts';

const key = (value: number) => btoa(String.fromCharCode(...new Uint8Array(32).fill(value)));
const peer = { id: 'computer-1', publicKey: key(7) };

test('presence and account membership grant no authority; only the exact pinned key does', () => {
  assert.equal(trusted(peer, {}), false);
  assert.throws(() => requireTrusted(peer, {}), /Trust this computer/);
  const pins = changedPins({}, peer, peer.publicKey, true);
  assert.equal(trusted(peer, pins), true);
  assert.doesNotThrow(() => requireTrusted(peer, pins));
  assert.equal(trusted({ ...peer, publicKey: key(8) }, pins), false);
  assert.throws(() => requireTrusted({ ...peer, publicKey: key(8) }, pins), /different key/);
  const removed = changedPins(pins, peer, peer.publicKey, false);
  assert.equal(trusted(peer, removed), false);
  assert.equal(trusted(peer, pins), true, 'edits do not mutate a prior state snapshot');
  assert.match(fingerprint(peer.publicKey), /^[0-9a-f]{32}$/, 'all 128 fingerprint bits are displayed');
});

test('a key changed after the confirmation opened cannot be authorized', () => {
  assert.throws(() => changedPins({}, { ...peer, publicKey: key(8) }, peer.publicKey, true), /different key/);
  assert.throws(() => changedPins({}, peer, 'broken key', true), /different key/);
});

test('missing pins are empty; damaged or oversized saved trust fails closed', () => {
  assert.deepEqual(parsePins(null), {});
  assert.deepEqual(parsePins(JSON.stringify({ [peer.id]: peer.publicKey })), { [peer.id]: peer.publicKey });
  for (const bad of ['not json', 'null', '[]', '1', '{"computer-1":"bad"}', `{"__proto__":"${peer.publicKey}"}`]) assert.throws(() => parsePins(bad));
  const full = Object.fromEntries(Array.from({ length: 64 }, (_, i) => [`device-${i}`, peer.publicKey]));
  assert.throws(() => changedPins(full, peer, peer.publicKey, true), /limit/);
  assert.throws(() => parsePins(JSON.stringify({ ...full, [peer.id]: peer.publicKey })), /invalid/);
});

test('the replay write is reached only for a current pin and its result is revalidated', async () => {
  let pins: Pins = { [peer.id]: peer.publicKey };
  let currentPeer: typeof peer | undefined = peer;
  let active = true;
  let receipts = 0;
  const current = () => ({ peer: currentPeer, pins, active });
  const receipt = async () => { receipts++; return true; };
  assert.equal(await admitReply(peer, current, receipt), 'accepted');
  assert.equal(receipts, 1);
  pins = {};
  assert.equal(await admitReply(peer, current, receipt), 'unauthorized');
  assert.equal(receipts, 1, 'untrusted peers cannot consume the replay ledger');
  pins = { [peer.id]: peer.publicKey };
  assert.equal(await admitReply(peer, current, async () => { pins = {}; return true; }), 'unauthorized');
  pins = { [peer.id]: peer.publicKey };
  assert.equal(await admitReply(peer, current, async () => { currentPeer = { ...peer, publicKey: key(8) }; return true; }), 'unauthorized');
  currentPeer = peer;
  assert.equal(await admitReply(peer, current, async () => { active = false; return true; }), 'unauthorized');
  active = true;
  assert.equal(await admitReply(peer, current, async () => false), 'stale');
  await assert.rejects(admitReply(peer, current, async () => { throw new Error('disk failed'); }), /disk failed/);
});
