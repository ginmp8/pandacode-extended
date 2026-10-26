// The phone's Connect client: a viewer of tasks on the user's computers (it never hosts).
// Owns the relay socket, the pending calls and the store; screens call `connect.*` and read
// state with the hooks in src/store. See README.md beside this file.
import './polyfill.ts';
import { initialState, taskKey, useConnect, type ConnectState } from '../store/index.ts';
import { chunks, dueIcons, mergeIcons } from '../store/icon-cache.ts';
import { clearIcons, loadIcons, resetIcons, saveIcons, useIcons } from '../store/icons.ts';
import { File, Paths } from 'expo-file-system';
import { BUILD_DEMO, DEMO_FILE, demoAtStart } from '../demo.ts';
import { demoCall, type DemoHost } from '../demo-host.ts';
import { delivering, settled, unmatched, type Sent } from '../store/sent.ts';
import { applyTeamEvent, decodeTeam, decodeTeamRun, teamBusy, teamRun, type Team } from '../store/team.ts';
import { applyEvent, ended, fromList, fromSubscribe, idleLive, lastId, mergeMessages, prependMessages, staleLists, withTeamRun } from '../store/reduce.ts';
import { cachedLastId, clearThreads, readThread, saveThread } from '../store/threads.ts';
import { PrefetchQueue, prefetchOrder } from '../store/prefetch.ts';
import { Cursors, followReplay } from '../store/replay.ts';
import { clearBlobs } from '../store/attachment-files.ts';
import type { Variant } from '../store/attachments.ts';
import { fetchAttachment, resetAttachmentRequests, type AttachmentDeps, type Source } from './attachments.ts';
import * as auth from './auth.ts';
import { deleteAccount as deleteAccountSteps } from './delete-account.ts';
import { build, NOT_CONFIGURED } from './config.ts';
import { decodeSecret, fingerprint, open, pairKey, publicKey, seal, uuid } from './crypto.ts';
import { BackgroundGate, LOST, NOT_CONNECTED, OFFLINE, outdated, Pending, RATE, request, STALE, START_TIMEOUT_MS, TIMEOUT_MS, UNKNOWN_METHOD, type Method } from './rpc.ts';
import * as secrets from './secrets.ts';
import { PersistentReplay } from './replay.ts';
import { SecurityWrites, SESSION_ENDED } from './security-writes.ts';
import { admitReply, changedPins, requireTrusted, trusted, NOT_TRUSTED, KEY_CHANGED } from './trust.ts';
import { AppState } from 'react-native';
import { RelaySocket } from './socket.ts';
import { markCurrent, markStartup } from './perf.ts';
import { clearInbox, readInbox, saveInboxSoon } from './inbox-file.ts';
import { decodeInbox, encodeInbox, type RestoredInbox } from '../store/inbox-cache.ts';
import { attempt, configured, stopping, unanswered, unconfigured, unpaused, unstopping } from './optimistic.ts';
import { newestPage, olderByAfterId, pageMessages, type Fetch } from './paging.ts';
import type { Live } from '../store/reduce.ts';
import type { AgentEvent, Bot, Changes, ConfigureParams, Device, Diff, MergeMethod, Message, MessagePage, Provider, ProviderModel, PullDetail, RunFinished, StartParams, SubscribeReply, TaskSummary, TasksList } from './types.ts';

const RENEW_MS = 9 * 60_000;
const REFRESH_MS = 12 * 3600_000;
const MAX_FRAME = 256 * 1024;
const MAX_PAGES = 50;
/** Messages asked for when a thread opens with nothing held, and per older page on scroll. */
const TAIL = 60;
const OLDER = 60;
/** afterId pages a background prefetch may copy. */
const PREFETCH_PAGES = 2;
/** Task lists refresh this often while the app is open (faster while a task runs). */
const LIST_IDLE_MS = 60_000;
const LIST_ACTIVE_MS = 15_000;
/** Presence frames come in bursts; a list younger than this is fresh enough. */
const LIST_FRESH_MS = 5_000;
/** A merge waits on GitHub and the host's git. */
const MERGE_TIMEOUT_MS = 60_000;
/** Calls made while the socket connects (a tap at launch, after foregrounding) wait this long for presence before failing. */
const CONNECT_WAIT_MS = 5_000;

let token: string | null = null;
let secret: Uint8Array | null = null;
let presence = false;
let signInAbort: AbortController | null = null;
let timers: ReturnType<typeof setInterval>[] = [];
/** Per module instance: Fast Refresh re-runs this file with fresh state while the store says ready. */
let initialized = false;
let securityReady = false;
let deletingAccount = false;
let signOutPending: Promise<void> | null = null;
const securityWrites = new SecurityWrites();
const replayStore = {
  read: secrets.replayStore.read,
  write: (value: string) => securityWrites.write(securityWrites.version, () => secrets.replayStore.write(value)),
};
const pending = new Pending();
const gate = new BackgroundGate();
/** Settles when credentials, keys, pins and the replay checkpoint are readable. */
let booted: Promise<void> = Promise.resolve();
/** Calls waiting for the socket's presence frame (see whenConnected). */
let connectWaiters: (() => void)[] = [];
let inbound = new PersistentReplay(replayStore);
let incoming: Promise<void> = Promise.resolve();
let pinWrites: Promise<void> = Promise.resolve();
/** Pair keys by device id, with the public key they were derived from. */
const keys = new Map<string, { publicKey: string; key: Uint8Array }>();
/** Tasks the UI has open, per host; subscribed, renewed, and re-subscribed on presence. */
const watched = new Map<string, Set<string>>();
/** Running tasks per host, subscribed too so the inbox can show what each is doing (at most GLANCE_MAX). */
const glanced = new Map<string, Set<string>>();
const GLANCE_MAX = 20;
/** Every subscribed task of a host: open ones and running ones. */
const subscribed = (host: string) => new Set([...(watched.get(host) ?? []), ...(glanced.get(host) ?? [])]);
/** Per taskKey: the message calls in order (one at a time), and the newest copy (it owns `history`). */
const queues = new Map<string, Promise<unknown>>();
const copies = new Map<string, Promise<unknown>>();
/** Cache reads per taskKey this launch (single flight; the first one fills the store). */
const warming = new Map<string, Promise<void>>();
/** When each thread was last copied in full (prefetch skips fresh ones). */
const copiedAt = new Map<string, number>();
/** Hosts whose task.messages has no tail/beforeId (replies without hasOlder): afterId paging only. */
const legacy = new Set<string>();

const state = () => useConnect.getState();
const patch = (update: Partial<ConnectState> | ((s: ConnectState) => Partial<ConnectState>)) => useConnect.setState(update);

const socket = new RelaySocket({
  token: () => token,
  url: () => `${build!.relay.replace(/^https:/, 'wss:')}/v1/connect?host=0`,
  onOpen: () => {
    markStartup('socket');
    patch({ connected: true, error: null });
    void refreshIfDue();
    timers = [setInterval(() => void subscribeAll(), RENEW_MS), setInterval(() => void refreshIfDue(), REFRESH_MS), setInterval(pollLists, LIST_ACTIVE_MS)];
  },
  onClose: () => {
    timers.forEach(clearInterval);
    timers = [];
    presence = false;
    pending.failAll(LOST);
    patch({ connected: false });
  },
  onText: text => {
    // SecureStore receipt writes complete before any frame takes effect. A queued frame
    // rechecks its current peer pin and session after the asynchronous boundary.
    const receivedToken = token;
    const version = securityWrites.version;
    incoming = incoming.then(() => receivedToken === token && securityWrites.active(version) && current() ? frame(text) : undefined)
      .catch(error => { if (securityWrites.active(version) && current()) securityFailure(error); });
  },
  onRefused: (used, revoked) => {
    if (revoked || auth.expired(used)) return signOutIf(used);
    // A WebSocket upgrade hides its HTTP status; ask the API whether the token still works.
    auth.listDevices(used).catch(error => { if (error instanceof auth.SignedOut) signOutIf(error.token); });
  },
});

// In development Fast Refresh re-runs this module whenever it or an import changes: the new
// instance (token unset, socket closed) would serve every call while the old socket still kept
// the store connected, so each new copy failed with NOT_SIGNED_IN. The newest instance closes the
// previous socket, and init() (re-run by the layout's effect) starts this one.
const instances = globalThis as { __pandacodeSocket?: RelaySocket };
instances.__pandacodeSocket?.dispose();
instances.__pandacodeSocket = socket;

/** The build-time demo applies until the user leaves it once this launch. */
let buildDemo = BUILD_DEMO;

/**
 * Reads secrets and connects when signed in, or enters demo mode. Call once at app start.
 * No socket opens until credentials, explicit pins and the replay checkpoint are readable.
 */
async function init(): Promise<void> {
  if (initialized) return booted;
  initialized = true;
  const version = securityWrites.version;
  booted = (async () => {
    if (demoAtStart(buildDemo, buildDemo ? null : await readDemo())) return seedDemo();
    const [saved, account, key, pins, cached] = await Promise.all([
      secrets.getToken(), secrets.getAccount(), secrets.getSecret(), secrets.getPins(), readInbox(), inbound.load(),
    ]);
    // On first use the key precedes its device id. An existing id with a missing
    // key is incomplete security state and must never trigger silent rekeying.
    const deviceId = await secrets.getDeviceId();
    if (!current() || !securityWrites.active(version)) return;
    const claims = saved ? auth.parseToken(saved) : null;
    if (saved && (!account || claims?.acct !== account.id || claims.dev !== deviceId)) throw new Error('Saved sign-in does not match this account and device. Clear the saved sign-in before continuing.');
    const activeToken = saved && !auth.expired(saved) && build ? saved : null;
    if (!activeToken) await Promise.all([clearIcons(), forgetThreads()]);
    if (!current() || !securityWrites.active(version)) return;
    secret = key;
    securityReady = true;
    token = activeToken;
    const restored = token && account ? decodeInbox(cached, account.id) : null;
    patch(s => ({ ready: true, deviceId, pins, fingerprint: fingerprint(publicKey(key)), signedIn: !!token, account: token ? account : null, ...(restored ? withCached(s, restored) : {}) }));
    markStartup('ready');
    void loadIcons();
    if (token) socket.start();
  })().catch(error => {
    if (!current() || !securityWrites.active(version)) return;
    securityFailure(error);
    token = null;
    initialized = false;
    patch({ ready: true, signedIn: false, account: null });
  });
  await booted;
}

function securityFailure(error: unknown): void {
  securityReady = false;
  socket.stop();
  const message = error instanceof Error ? error.message : 'Connect security state is unavailable.';
  pending.failAll(message);
  patch({ error: message, connected: false });
}

/** The cached inbox under whatever live data already arrived (presence can beat the file). */
function withCached(s: ConnectState, cached: RestoredInbox): Partial<ConnectState> {
  return { devices: s.devices.length ? s.devices : cached.devices, tasks: { ...cached.tasks, ...s.tasks }, bots: { ...cached.bots, ...s.bots } };
}

// The inbox cache follows the store: devices, lists and bots, saved shortly after they change.
useConnect.subscribe((s, prev) => {
  if (s.tasks === prev.tasks && s.devices === prev.devices && s.bots === prev.bots) return;
  if (!current() || s.demo || !token || !s.account) return;
  saveInboxSoon(() => {
    const now = state();
    return current() && !now.demo && token && now.account ? encodeInbox(now.account.id, now.devices, now.tasks, now.bots) : null;
  });
});
/** False in a module instance Fast Refresh replaced. */
function current() { return instances.__pandacodeSocket === socket; }
const ownsSession = (version: number) => current() && securityWrites.active(version);
function requireSession(version: number) {
  if (!ownsSession(version)) throw new Error(SESSION_ENDED);
}

async function signInStart(): Promise<auth.SignInCode> {
  if (!build) throw new Error(NOT_CONFIGURED);
  await init();
  if (!securityReady) throw new Error(state().error ?? 'Connect security state is unavailable.');
  if (deletingAccount) throw new Error('Account deletion is still in progress.');
  if (token) throw new Error('Sign out before signing in to another account.');
  signInAbort?.abort();
  const abort = new AbortController();
  signInAbort = abort;
  const version = securityWrites.version;
  const code = await auth.startSignIn();
  if (abort.signal.aborted || signInAbort !== abort || !securityWrites.active(version)) throw new Error('Sign-in was cancelled.');
  return code;
}

/** Waits for approval on GitHub and registers this phone. */
async function signInFinish(code: auth.SignInCode): Promise<void> {
  if (!securityReady) throw new Error(state().error ?? 'Connect security state is unavailable.');
  if (deletingAccount) throw new Error('Account deletion is still in progress.');
  const abort = signInAbort;
  if (!abort || abort.signal.aborted) throw new Error('Sign-in was cancelled.');
  const version = securityWrites.version;
  const result = await auth.finishSignIn(code, abort.signal);
  if (abort.signal.aborted || signInAbort !== abort || !securityWrites.active(version)) throw new Error('Sign-in was cancelled.');
  const pins = await securityWrites.write(version, async () => {
    try {
      if (abort.signal.aborted) throw new Error('Sign-in was cancelled.');
      await secrets.setToken(result.token);
      if (abort.signal.aborted) throw new Error('Sign-in was cancelled.');
      await secrets.setAccount(result.account);
      if (abort.signal.aborted) throw new Error('Sign-in was cancelled.');
      const savedPins = await secrets.getPins();
      if (abort.signal.aborted) throw new Error('Sign-in was cancelled.');
      return savedPins;
    } catch (error) {
      await secrets.clearSession();
      throw error;
    }
  });
  if (abort.signal.aborted || signInAbort !== abort || !securityWrites.active(version)) throw new Error('Sign-in was cancelled.');
  if (signInAbort === abort) signInAbort = null;
  token = result.token;
  presence = false;
  patch({ signedIn: true, account: result.account, deviceId: result.deviceId, pins, error: null });
  socket.stop();
  socket.start();
}

function signInCancel() {
  signInAbort?.abort();
  signInAbort = null;
}

// ---- Demo mode (src/demo.ts) ----

const demoFile = () => new File(Paths.document, DEMO_FILE);
const readDemo = async () => {
  try {
    const f = demoFile();
    return f.exists ? await f.text() : null;
  } catch { return null; }
};

async function seedDemo() {
  const data = await import('../demo-data.ts');
  data.seedDemo();
  patch({ demo: true, error: null });
}

/** From sign-in: fake computers and tasks, no network, kept across launches. */
async function enterDemo(): Promise<void> {
  signInCancel();
  const f = demoFile();
  if (!f.exists) f.create();
  await f.write('1');
  await seedDemo();
}

/** Back to sign-in with nothing of the demo left. */
async function leaveDemo(): Promise<void> {
  buildDemo = false;
  const f = demoFile();
  if (f.exists) f.delete();
  resetIcons();
  useConnect.setState({ ...initialState(), ready: true }, true);
  initialized = false;
  await init();
}

const demoHost: DemoHost = { get: () => state(), set: patch, later: (run, ms) => { setTimeout(run, ms); }, now: Date.now };

async function signOut(): Promise<void> {
  if (state().demo) return leaveDemo();
  if (signOutPending) return signOutPending;
  const run = clearSignedInSession();
  signOutPending = run;
  try { await run; }
  finally { if (signOutPending === run) signOutPending = null; }
}

async function clearSignedInSession(): Promise<void> {
  const ready = securityReady;
  securityReady = false;
  const clear = securityWrites.transition(async () => {
    await secrets.clearSession();
    await Promise.all([clearIcons(), forgetThreads()]);
    // A receipt interrupted by sign-out may have marked its old checker failed.
    // Its durable write has now drained; reload the preserved checkpoint safely.
    const nextReplay = new PersistentReplay(replayStore);
    await nextReplay.load();
    inbound = nextReplay;
  });
  const version = securityWrites.version;
  resetSession();
  try { await clear; }
  catch (error) { securityFailure(error); throw error; }
  if (current() && securityWrites.active(version)) securityReady = ready;
}

/** Immediately invalidates network/state authority; storage clears follow the write drain. */
function resetSession(forgetIdentity = false) {
  signInCancel();
  socket.stop();
  pending.failAll('Signed out.');
  token = null;
  keys.clear();
  watched.clear();
  glanced.clear();
  presence = false;
  const { deviceId, pins, fingerprint: ownFingerprint } = state();
  useConnect.setState({ ...initialState(), ready: true, ...(forgetIdentity ? {} : { deviceId, pins, fingerprint: ownFingerprint }) }, true);
}

/** A refusal signs out only if it refused the token that is still current. */
function signOutIf(used: string) {
  if (token !== used) return;
  if (deletingAccount) return; // the confirmed deletion path owns the local cleanup
  void signOut().then(() => patch({ error: auth.SIGNED_OUT })).catch(securityFailure);
}

async function refreshIfDue() {
  const old = token;
  const version = securityWrites.version;
  if (!old || !auth.needsRefresh(old)) return;
  try {
    const fresh = await auth.refreshToken(old);
    if (!fresh || token !== old || !securityReady || !securityWrites.active(version)) return;
    await securityWrites.write(version, () => secrets.setToken(fresh));
    if (token === old && securityReady && securityWrites.active(version)) token = fresh;
  } catch (error) {
    if (error instanceof auth.SignedOut) signOutIf(error.token);
    else if (ownsSession(version)) securityFailure(error);
  }
}

// ---- Devices ----

const validDevice = (d: any): d is Device =>
  d && typeof d.id === 'string' && /^[a-zA-Z0-9_-]{1,200}$/.test(d.id) && typeof d.name === 'string' && typeof d.publicKey === 'string' && decodeSecret(d.publicKey) !== null;

/** Presence is discovery only. A removed or changed key loses all pending authority. */
function applyDevices(list: unknown[]) {
  const devices = list.filter(validDevice).slice(0, 64);
  if (new Set(devices.map(d => d.id)).size !== devices.length) return;
  for (const previous of state().devices) {
    const next = devices.find(d => d.id === previous.id);
    if (!next || next.publicKey !== previous.publicKey || !trusted(next, state().pins)) {
      pending.fail(previous.id, undefined, KEY_CHANGED);
      watched.delete(previous.id);
      glanced.delete(previous.id);
    }
  }
  for (const id of keys.keys()) if (!devices.some(d => d.id === id && d.publicKey === keys.get(id)!.publicKey)) keys.delete(id);
  for (const map of [watched, glanced]) for (const id of map.keys()) if (!devices.some(d => d.id === id)) map.delete(id);
  patch({ devices });
}

/** Explicit UI confirmation of the displayed key; never invoked by presence or startup. */
async function setTrusted(deviceId: string, expectedKey: string, allow: boolean): Promise<void> {
  const version = securityWrites.version;
  const previous = pinWrites;
  let release!: () => void;
  pinWrites = new Promise(resolve => { release = resolve; });
  await previous;
  try {
    if (!token || !securityReady || !presence || !current() || !securityWrites.active(version)) throw new Error(NOT_CONNECTED);
    const used = token;
    const device = state().devices.find(d => d.id === deviceId);
    if (!device || deviceId === state().deviceId) throw new Error('That computer is no longer available.');
    const next = changedPins(state().pins, device, expectedKey, allow);
    if (!allow) {
      patch({ pins: next });
      keys.delete(deviceId);
      watched.delete(deviceId);
      glanced.delete(deviceId);
      pending.fail(deviceId, undefined, NOT_TRUSTED);
    }
    try { await securityWrites.write(version, () => secrets.setPins(next)); }
    catch (error) { if (securityWrites.active(version)) securityFailure(error); throw error; }
    if (token !== used || !current()) return;
    if (state().devices.find(d => d.id === deviceId)?.publicKey !== expectedKey) throw new Error(KEY_CHANGED);
    patch({ pins: next });
    if (allow && device.online && device.host) void listTasks(deviceId).catch(() => undefined);
  } finally { release(); }
}

/** The device list over HTTP; only needed before the socket's first presence frame. */
async function refreshDevices(): Promise<void> {
  if (!token || presence) return;
  const used = token;
  const version = securityWrites.version;
  try {
    const devices = await auth.listDevices(used);
    if (!presence && token === used && securityReady && current() && securityWrites.active(version)) applyDevices(devices);
  } catch (error) {
    if (error instanceof auth.SignedOut) signOutIf(error.token);
    throw error;
  }
}

async function revokeDevice(deviceId: string): Promise<void> {
  if (state().demo) {
    if (deviceId === state().deviceId) return leaveDemo();
    return patch(s => ({ devices: s.devices.filter(d => d.id !== deviceId) }));
  }
  if (!token) throw new Error(NOT_SIGNED_IN);
  const used = token;
  await auth.revokeDevice(used, deviceId).catch(error => {
    if (error instanceof auth.SignedOut) signOutIf(error.token);
    throw error;
  });
  if (deviceId === state().deviceId) return signOut();
}

/** Deletes the account and every device from the relay, then wipes this phone's secrets and caches and signs out. */
async function deleteAccount(): Promise<void> {
  if (!token) throw new Error(NOT_SIGNED_IN);
  if (deletingAccount) throw new Error('Account deletion is already in progress.');
  deletingAccount = true;
  const used = token;
  try { await deleteAccountSteps({
    remove: () => auth.deleteAccount(used).catch(error => {
      if (error instanceof auth.SignedOut) signOutIf(error.token);
      throw error;
    }),
    wipe: async () => {
      securityReady = false;
      const clear = securityWrites.transition(async () => {
        await secrets.wipe();
        await Promise.all([clearIcons(), forgetThreads()]);
      });
      resetSession(true);
      secret = null;
      try { await clear; }
      catch (error) { securityFailure(error); throw error; }
    },
    signOut: async () => {
      // The confirmed deletion has erased the identity. Initialize a fresh one
      // only after every old security write and cache operation has drained.
      inbound = new PersistentReplay(replayStore);
      initialized = false;
      await init();
    },
  }); } catch (error) {
    deletingAccount = false;
    if (error instanceof auth.SignedOut) signOutIf(error.token);
    throw error;
  } finally { deletingAccount = false; }
}

// ---- Calls ----

/** Thrown by calls made while the token is still loading or after sign-out; screens don't show it. */
export const NOT_SIGNED_IN = 'Sign in to PandaCode Connect first.';

function callable(deviceId: string): Device {
  if (!token) throw new Error(NOT_SIGNED_IN);
  if (!securityReady) throw new Error(state().error ?? 'Connect security state is unavailable.');
  const s = state();
  if (deviceId === s.deviceId) throw new Error('That is this phone.');
  const device = s.devices.find(d => d.id === deviceId);
  if (!device) throw new Error('That computer is no longer on your account.');
  requireTrusted(device, s.pins);
  if (!device.online) throw new Error(OFFLINE);
  return device;
}

function pairFor(device: Device): Uint8Array {
  requireTrusted(device, state().pins);
  const cached = keys.get(device.id);
  if (cached?.publicKey === device.publicKey) return cached.key;
  const key = pairKey(secret!, state().deviceId, device.publicKey, device.id);
  keys.set(device.id, { publicKey: device.publicKey, key });
  return key;
}

/** Resolves once presence arrived on an open socket, or after `ms` (the call then fails as before). */
function whenConnected(ms = CONNECT_WAIT_MS): Promise<void> {
  if ((presence && socket.open) || !token || !(socket.open || socket.connecting)) return Promise.resolve();
  return new Promise(resolve => {
    const done = () => {
      clearTimeout(timer);
      connectWaiters = connectWaiters.filter(w => w !== done);
      resolve();
    };
    const timer = setTimeout(done, ms);
    connectWaiters.push(done);
  });
}

/**
 * One request to a host. `background` work (prefetch, icons, attachments, glance subscribes, bots)
 * waits its turn: at most one per host in flight, so a tap never queues behind a pile of it.
 */
async function call<T>(deviceId: string, method: Method, params: unknown, timeoutMs = TIMEOUT_MS, background = false): Promise<T> {
  const version = securityWrites.version;
  await booted;
  requireSession(version);
  if (state().demo) return demoCall(demoHost, deviceId, method, params) as Promise<T>;
  await whenConnected();
  requireSession(version);
  const expectedKey = callable(deviceId).publicKey;
  const release = background ? await gate.turn(deviceId) : () => undefined;
  try {
    requireSession(version);
    const reply = await send<T>(deviceId, method, params, timeoutMs);
    requireSession(version);
    const peer = state().devices.find(d => d.id === deviceId);
    if (!peer || peer.publicKey !== expectedKey || !trusted(peer, state().pins)) throw new Error(KEY_CHANGED);
    return reply;
  } finally {
    release();
  }
}

function send<T>(deviceId: string, method: Method, params: unknown, timeoutMs: number): Promise<T> {
  const device = callable(deviceId);
  const message = request(method, params);
  const body = seal(pairFor(device), state().deviceId, device.id, JSON.stringify(message));
  const frame = JSON.stringify({ to: device.id, body, ref: message.id });
  if (frame.length > MAX_FRAME) throw new Error('That request is too large.');
  const reply = pending.wait(device.id, message.id, timeoutMs);
  if (!socket.send(frame)) pending.fail(device.id, message.id, NOT_CONNECTED);
  return reply as Promise<T>;
}

// ---- Incoming frames ----

async function frame(text: string): Promise<void> {
  if (!securityReady || !token || new TextEncoder().encode(text).byteLength > MAX_FRAME) return;
  let value: any;
  try { value = JSON.parse(text); } catch { return; }
  switch (value?.kind) {
    case 'presence':
      if (!Array.isArray(value.devices)) return;
      presence = true;
      markStartup('presence');
      applyDevices(value.devices);
      // Lists first: every online host is asked at once (cached lists count as stale).
      void refreshLists(LIST_FRESH_MS);
      void subscribeAll();
      connectWaiters.forEach(wake => wake());
      return;
    case 'undeliverable':
      if (typeof value.to === 'string') pending.fail(value.to, typeof value.ref === 'string' ? value.ref : undefined, value.reason === 'rate' ? RATE : OFFLINE);
      return;
    case 'error':
      patch({ error: typeof value.error === 'string' ? `Relay: ${value.error.slice(0, 300)}` : 'The relay reported an error.' });
      return;
  }
  const { from, to, body } = value ?? {};
  const s = state();
  if (typeof from !== 'string' || typeof body !== 'string' || to !== s.deviceId || from === s.deviceId) return;
  const device = s.devices.find(d => d.id === from);
  if (!device?.publicKey || !trusted(device, s.pins)) return;
  const receivedToken = token;
  let message: any;
  try { message = JSON.parse(open(pairFor(device), from, to, body)); } catch { return; }
  // Replies and events carry ts + nonce: a stale or repeated one (or an older host's) is dropped.
  if (message?.type !== 'res' && message?.type !== 'event') return;
  const admission = await admitReply(device, () => ({
    peer: state().devices.find(d => d.id === from), pins: state().pins,
    active: current() && securityReady && receivedToken === token,
  }), () => inbound.check(message.ts, message.nonce));
  if (admission !== 'accepted') {
    if (admission === 'stale' && message.type === 'res') pending.resolve(from, { id: message.id, ok: false, error: STALE });
    return;
  }
  if (message?.type === 'res') pending.resolve(from, message);
  else if (message?.type === 'event' && message.name === 'agent.events') {
    events(from, message.data?.events, message.data?.epoch);
    // The host dropped events it could not buffer: hold what comes next and replay from here.
    if (message.data?.lagged === true) {
      cursors.hold(from);
      void subscribeHost(from);
    }
  }
  else if (message?.type === 'event' && message.name === 'run.finished') void finished(from, message.data);
}

const active = (status?: string) => status === 'running' || status === 'waiting';

/** Per host: its event journal's epoch and the newest `seq` seen per task, so a subscribe asks
 * only for what was missed; live events wait while a subscribe's replay is in flight. */
const cursors = new Cursors();

function events(host: string, raw: unknown, epoch?: unknown, live = true) {
  const ids = subscribed(host);
  if (!ids.size || !Array.isArray(raw)) return;
  if (live && cursors.keep(host, epoch, raw as AgentEvent[])) return;
  const list = cursors.fresh(host, epoch, raw as AgentEvent[]);
  const started = new Set<string>();
  patch(s => {
    const live = { ...s.live };
    for (const event of list as AgentEvent[]) {
      if (typeof event?.sessionId !== 'string' || typeof event.kind !== 'string' || !ids.has(event.sessionId)) continue;
      const key = taskKey(host, event.sessionId);
      const before = live[key]?.status;
      live[key] = applyEvent(live[key] ?? idleLive(), { ...event, text: typeof event.text === 'string' ? event.text : '' });
      if (!active(before) && active(live[key].status)) started.add(event.sessionId);
    }
    let teams = s.teams;
    for (const event of list as AgentEvent[]) {
      if (event?.kind !== 'team' || typeof event.sessionId !== 'string' || !ids.has(event.sessionId)) continue;
      const key = taskKey(host, event.sessionId);
      const team = teams[key] === 'outdated' ? undefined : teams[key];
      const next = applyTeamEvent(team, event.data);
      if (next === team || next === undefined) continue;
      teams = { ...teams, [key]: next };
      // Older hosts send no team-settled event: a loaded team's workers say when a held task is done.
      const run = team ? teamRun(next) : undefined;
      if (run && live[key]?.held) live[key] = withTeamRun(live[key], run);
    }
    let sent = s.sent;
    for (const id of started) if (sent[taskKey(host, id)]) sent = { ...sent, [taskKey(host, id)]: delivering(sent[taskKey(host, id)]) };
    return { live, sent, teams, tasks: withStatuses(s, host, live) };
  });
  // A turn that starts on its own (the host delivering a queued prompt after the last run's
  // copy) saved its prompt before its first event: copy it now, not only at this run's end.
  for (const id of started) if (watched.get(host)?.has(id)) void loadMessages(host, id).catch(() => undefined);
}

/** Mirrors live statuses into the host's task list. */
function withStatuses(s: ConnectState, host: string, live: ConnectState['live']): ConnectState['tasks'] {
  const list = s.tasks[host];
  if (!list) return s.tasks;
  let changed = false;
  const tasks = list.tasks.map(t => {
    const status = live[taskKey(host, t.sessionId)]?.status;
    if (!status || status === t.status) return t;
    changed = true;
    return { ...t, status };
  });
  // Unchanged → same object, so a streamed delta doesn't re-render task lists.
  return changed ? { ...s.tasks, [host]: { ...list, tasks } } : s.tasks;
}

async function finished(host: string, data: RunFinished) {
  const version = securityWrites.version;
  if (typeof data?.sessionId !== 'string') return;
  void listTasks(host).catch(() => undefined); // titles, statuses and order
  const key = taskKey(host, data.sessionId);
  if (watched.get(host)?.has(data.sessionId)) {
    await loadMessages(host, data.sessionId).catch(() => undefined); // the next subscribe retries
    // The turn may have opened a pull request (or changed the linked one).
    if (key in state().pulls) void loadPull(host, data.sessionId, true).catch(() => undefined);
  } else if (!state().live[key]) return;
  if (!ownsSession(version)) return;
  // A glanced task (no open thread) ends now too: a newer host keeps listing an architect with a
  // busy team as running, so its list row alone would not settle the finished turn's activity.
  // Newer hosts send the row's status: 'running' while the team works (hold), the final one when
  // it settles (`teamSettled`, possibly after an ordinary run.finished: ending twice is harmless).
  const hold = active(data.status);
  const status = data.teamSettled === true && typeof data.status === 'string' && data.status && !hold ? data.status : data.stopped ? 'stopped' : data.successful ? 'idle' : 'failed';
  patch(s => {
    // A team still working (newer hosts say so; else the loaded team) keeps the task working.
    const listed = data.team !== undefined ? decodeTeamRun(data.team) : teamRun(s.teams[key]);
    const team = hold && !teamBusy(listed) ? { status: 'running', activeWorkers: listed?.activeWorkers ?? 0 } : listed;
    // The host's run ends with its process, background subagents included: only the team holds.
    const live = { ...s.live, [key]: ended(s.live[key], status, team, []) };
    return { live, sent: editSent(s.sent, key, settled), tasks: withStatuses(s, host, live) };
  });
}

// ---- Subscriptions and messages ----

/** Keeps these tasks' events and messages flowing while the UI shows them. */
function watch(deviceId: string, sessionIds: string[]) {
  const ids = watched.get(deviceId) ?? new Set();
  const before = ids.size;
  sessionIds.forEach(id => ids.add(id));
  watched.set(deviceId, ids);
  prefetch.hold('thread');
  if (ids.size !== before) void subscribeHost(deviceId);
}

/** Follows a host's running tasks (from its list) so their activity shows in the inbox. */
function glance(deviceId: string, sessionIds: string[]) {
  const before = glanced.get(deviceId) ?? new Set<string>();
  const next = new Set(sessionIds.slice(0, GLANCE_MAX));
  if (next.size) glanced.set(deviceId, next);
  else glanced.delete(deviceId);
  if ([...next].some(id => !before.has(id) && !watched.get(deviceId)?.has(id))) void subscribeHost(deviceId);
}

function unwatch(deviceId: string, sessionIds: string[]) {
  const ids = watched.get(deviceId);
  sessionIds.forEach(id => ids?.delete(id));
  if (ids && !ids.size) watched.delete(deviceId);
  if (!watched.size) prefetch.release('thread');
}

async function subscribeAll() {
  await Promise.all([...new Set([...watched.keys(), ...glanced.keys()])].map(subscribeHost));
}

/** Per host: the subscribe in flight, and whether the set changed since it was sent. */
const subscribing = new Map<string, Promise<void>>();
const resubscribe = new Set<string>();

/** One subscribe call per host at a time: calls in the same tick share it, and changes made while
 * it is in flight go out in one more call after it. */
function subscribeHost(deviceId: string): Promise<void> {
  const version = securityWrites.version;
  const running = subscribing.get(deviceId);
  if (running) {
    resubscribe.add(deviceId);
    return running;
  }
  const run = (async () => {
    await Promise.resolve(); // the rest of this tick joins in
    if (!ownsSession(version)) return;
    do {
      resubscribe.delete(deviceId);
      await subscribeOnce(deviceId);
    } while (ownsSession(version) && resubscribe.has(deviceId));
  })().finally(() => { if (subscribing.get(deviceId) === run) subscribing.delete(deviceId); });
  subscribing.set(deviceId, run);
  return run;
}

/** Subscribe replies catch up on missed status changes, pending requests and messages. */
async function subscribeOnce(deviceId: string): Promise<void> {
  const version = securityWrites.version;
  const ids = [...subscribed(deviceId)].slice(0, 500);
  if (!ids.length || !socket.open) return;
  let reply: { tasks: SubscribedTask[] };
  try {
    // Only glanced tasks (no open thread on this host): background work.
    reply = await call(deviceId, 'subscribe', { sessionIds: ids }, TIMEOUT_MS, !watched.get(deviceId)?.size);
  } catch {
    return; // offline; the next presence frame or renewal retries
  }
  if (!ownsSession(version)) return;
  const tasks = Array.isArray(reply?.tasks) ? reply.tasks : [];
  patch(s => {
    const live = { ...s.live };
    for (const t of tasks) {
      const key = taskKey(deviceId, t.sessionId);
      live[key] = fromSubscribe(live[key] ?? idleLive(), t.status, t.pending, t, teamRun(s.teams[key]));
    }
    return { live, tasks: withStatuses(s, deviceId, live) };
  });
  // What arrived live meanwhile is newer than the snapshot.
  showHeld();
  // A gap means events were lost: the messages copy catches up.
  for (const t of tasks) if (watched.get(deviceId)?.has(t.sessionId) && (t.gap === true || t.lastId > lastId(state().messages[taskKey(deviceId, t.sessionId)]))) void loadMessages(deviceId, t.sessionId).catch(() => undefined);
}

/** One task's task.messages call with these params; a background copy (prefetch) turns foreground once the thread is open. */
const fetchFor = (deviceId: string, sessionId: string, background = false): Fetch => params =>
  call<MessagePage>(deviceId, 'task.messages', { sessionId, ...params }, TIMEOUT_MS, background && !watched.get(deviceId)?.has(sessionId));

/** Stores a page: after the newest message, before the oldest, or in place of the window (a tail).
 * Prepend and replace also record whether older messages remain. Saved to disk shortly after. */
function store(deviceId: string, sessionId: string, page: Message[], mode: 'append' | 'prepend' | 'replace', reply?: Partial<MessagePage> | null) {
  const key = taskKey(deviceId, sessionId);
  patch(s => {
    const held = s.messages[key] ?? [];
    const merged = mode === 'replace' ? page : mode === 'prepend' ? prependMessages(held, page) : mergeMessages(held, page);
    const { [key]: _, ...rest } = s.older;
    const older = mode === 'append' ? s.older : reply?.hasOlder ? { ...rest, [key]: 'more' as const } : rest;
    return { messages: { ...s.messages, [key]: merged }, older, sent: editSent(s.sent, key, sent => unmatched(sent, merged)), tasks: withTitle(s, deviceId, sessionId, reply?.title) };
  });
  saveThread(key, () => ({ messages: state().messages[key], hasOlder: state().older[key] !== undefined }));
}

/** Runs a task's message calls one at a time, in order. */
function serial<T>(key: string, run: () => Promise<T>): Promise<T> {
  const version = securityWrites.version;
  const next = (queues.get(key) ?? Promise.resolve()).catch(() => undefined).then(() => { requireSession(version); return run(); });
  queues.set(key, next);
  const done = () => { if (queues.get(key) === next) queues.delete(key); };
  next.then(done, done);
  return next;
}

/** Draws the thread from the disk cache when the store holds none of it (press-in, open, prefetch). */
function peek(deviceId: string, sessionId: string): Promise<void> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  if (state().demo || key in state().messages) return Promise.resolve();
  let read = warming.get(key);
  if (!read) {
    read = readThread(key).then(cached => {
      if (!ownsSession(version) || !cached || key in state().messages) return;
      patch(s => ({ messages: { ...s.messages, [key]: cached.messages }, older: cached.hasOlder ? { ...s.older, [key]: 'more' } : s.older }));
    }, () => undefined);
    warming.set(key, read);
  }
  return read;
}

/**
 * Brings a thread up to date. With nothing held (in memory or on disk) the newest page comes
 * first (tail); then the messages after the newest one held, following hasMore. A held window
 * far behind (its first afterId page has more after it) is replaced by the tail. Hosts without
 * tail/beforeId page by afterId from the start, as before. Runs one call at a time per task.
 */
function loadMessages(deviceId: string, sessionId: string, pages = MAX_PAGES, background = false): Promise<void> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  const run = async (): Promise<string> => {
    await peek(deviceId, sessionId);
    requireSession(version);
    const fetch = fetchFor(deviceId, sessionId, background);
    const tail = () => newestPage(fetch, deviceId, legacy, { tail: true, limit: TAIL });
    const held = !!state().messages[key]?.length;
    if (!held) {
      // An older host answers as for afterId 0 (the first page) or refuses (then the loop starts at 0).
      const { page, legacy: old } = await tail();
      requireSession(version);
      // A prefetch learning the host is older stops: its first page is the oldest, not the newest.
      if (old && pages < MAX_PAGES) return 'afterId';
      if (page) store(deviceId, sessionId, pageMessages(page), old ? 'append' : 'replace', page);
      if (page && !page.hasMore) return old ? 'afterId' : 'tail';
    }
    for (let n = 0; n < pages; n++) {
      const reply = await fetch({ afterId: lastId(state().messages[key]) });
      requireSession(version);
      if (n === 0 && held && reply?.hasMore) {
        const { page, legacy: old } = await tail();
        requireSession(version);
        if (!old && page) {
          store(deviceId, sessionId, pageMessages(page), 'replace', page);
          if (!page.hasMore) return 'tail';
          continue;
        }
      }
      const messages = pageMessages(reply);
      store(deviceId, sessionId, messages, 'append', reply);
      if (!reply?.hasMore || !messages.length) return 'afterId';
    }
    return 'afterId';
  };
  const copy = serial(key, run);
  copies.set(key, copy);
  const settle = (error: string | null) => {
    if (!ownsSession(version) || copies.get(key) !== copy) return; // a later copy/session owns the state now
    copies.delete(key);
    patch(s => {
      const { [key]: _, ...history } = s.history;
      return { history: error === null ? history : { ...history, [key]: error } };
    });
  };
  patch(s => ({ history: { ...s.history, [key]: null } }));
  return copy.then(how => {
    requireSession(version);
    copiedAt.set(key, Date.now());
    settle(null);
    markCurrent(key, how);
  }, (error: Error) => {
    settle(error?.message || 'Could not load this task.');
    throw error;
  });
}

/** The page before the oldest message held, when the user scrolls near it. Hosts without
 * beforeId page forward from the start up to it. A failure leaves it to the next scroll. */
function loadOlder(deviceId: string, sessionId: string): Promise<void> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  if (state().older[key] !== 'more') return Promise.resolve();
  const setOlder = (value?: 'more' | 'loading') => {
    if (!ownsSession(version)) return;
    patch(s => {
      const { [key]: _, ...rest } = s.older;
      return { older: value ? { ...rest, [key]: value } : rest };
    });
  };
  setOlder('loading');
  return serial(key, async () => {
    const first = state().messages[key]?.[0]?.id;
    if (!first) return setOlder();
    const fetch = fetchFor(deviceId, sessionId);
    const { page, legacy: old } = await newestPage(fetch, deviceId, legacy, { beforeId: first, limit: OLDER });
    requireSession(version);
    if (!old && page) return store(deviceId, sessionId, pageMessages(page), 'prepend', page);
    const older = await olderByAfterId(fetch, first, page, MAX_PAGES);
    requireSession(version);
    store(deviceId, sessionId, older, 'prepend', { hasOlder: false });
  }).catch(() => { setOlder('more'); });
}

/** Copies threads ahead in the background, one call at a time, held while a thread is open or a prompt sends. */
const prefetch = new PrefetchQueue(async key => {
  const at = key.indexOf('/');
  const [deviceId, sessionId] = [key.slice(0, at), key.slice(at + 1)];
  if (legacy.has(deviceId) && !(key in state().messages) && cachedLastId(key) === undefined) return;
  await loadMessages(deviceId, sessionId, PREFETCH_PAGES, true);
});

/** Queues the threads worth copying ahead from every online host's list: running, waiting, then the most recent. */
function schedulePrefetch() {
  const s = state();
  if (s.demo) return;
  const known = (key: string) => {
    const id = Math.max(lastId(s.messages[key]), cachedLastId(key) ?? 0);
    const at = copiedAt.get(key);
    return id || at ? { lastId: id, at: at ?? 0 } : undefined;
  };
  const open = new Set([...watched].flatMap(([host, ids]) => [...ids].map(id => taskKey(host, id))));
  const lists = Object.entries(s.tasks)
    .filter(([deviceId]) => s.devices.some(d => d.id === deviceId && d.online))
    // Without tail pages a first copy starts at the oldest messages: only cached threads catch up.
    .map(([deviceId, list]) => ({ deviceId, tasks: list.tasks.filter(t => !legacy.has(deviceId) || known(taskKey(deviceId, t.sessionId))) }));
  prefetch.replace(prefetchOrder(lists, known, Date.now()).filter(key => !open.has(key)));
}

/** Sign-out and account deletion: nothing of any thread stays in memory or on disk. */
async function forgetThreads(): Promise<void> {
  prefetch.replace([]);
  resetAttachmentRequests();
  warming.clear();
  copiedAt.clear();
  legacy.clear();
  queues.clear();
  copies.clear();
  listing.clear();
  checking.clear();
  noIcons.clear();
  iconsAsking.clear();
  teamless.clear();
  subscribing.clear();
  resubscribe.clear();
  await Promise.all([clearThreads(), clearBlobs(), clearInbox()]);
}

function withTitle(s: ConnectState, host: string, sessionId: string, title: unknown): ConnectState['tasks'] {
  const list = s.tasks[host];
  if (!list || typeof title !== 'string' || !title) return s.tasks;
  return { ...s.tasks, [host]: { ...list, tasks: list.tasks.map(t => (t.sessionId === sessionId && t.title !== title ? { ...t, title } : t)) } };
}

// ---- Typed methods ----

/** Full lists in flight per host: presence, focus and pull to refresh share one call. */
const listing = new Map<string, Promise<TasksList>>();

function listTasks(deviceId: string, limit?: number): Promise<TasksList> {
  if (limit !== undefined) return fetchList(deviceId, limit);
  const running = listing.get(deviceId);
  if (running) return running;
  const next = fetchList(deviceId).finally(() => { if (listing.get(deviceId) === next) listing.delete(deviceId); });
  listing.set(deviceId, next);
  return next;
}

/** A host's list; then (never before it) glances, icons, bots and prefetch, all background. */
async function fetchList(deviceId: string, limit?: number): Promise<TasksList> {
  const version = securityWrites.version;
  const reply = await call<TasksList>(deviceId, 'tasks.list', limit === undefined ? {} : { limit });
  requireSession(version);
  markStartup('list');
  const list = { tasks: Array.isArray(reply?.tasks) ? reply.tasks : [], projects: Array.isArray(reply?.projects) ? reply.projects : [] };
  patch(s => {
    // A task already waiting can be answered as soon as it opens; a run that ended while the
    // phone wasn't listening stops showing as working.
    const live = { ...s.live };
    for (const t of list.tasks) {
      const key = taskKey(deviceId, t.sessionId);
      const next = fromList(live[key], t.status, Array.isArray(t.pending) ? t.pending : undefined, t, teamRun(s.teams[key]));
      if (next) live[key] = next;
    }
    return { live, tasks: { ...s.tasks, [deviceId]: { ...list, loadedAt: Date.now() } } };
  });
  glance(deviceId, list.tasks.filter(t => active(t.status)).map(t => t.sessionId));
  void projectIcons(deviceId, list.tasks.map(t => t.projectId));
  if (!state().outdated[deviceId]) void listBots(deviceId).catch(() => undefined);
  schedulePrefetch();
  return list;
}

/** Lists the tasks of every online host whose list is older than `maxAgeMs`; failures wait for the next refresh. */
async function refreshLists(maxAgeMs = 0): Promise<void> {
  const s = state();
  if (!token || !socket.open || AppState.currentState !== 'active') return;
  const hosts = s.devices.filter(d => d.host && d.online && d.id !== s.deviceId && trusted(d, s.pins)).map(d => d.id);
  const loadedAt = Object.fromEntries(Object.entries(s.tasks).map(([id, list]) => [id, list.loadedAt]));
  await Promise.all(staleLists(hosts, loadedAt, Date.now(), maxAgeMs).map(id => listTasks(id).catch(() => undefined)));
}

/** Every LIST_ACTIVE_MS tick: lists refresh at that pace while a task runs, else every LIST_IDLE_MS. */
function pollLists() {
  const running = Object.values(state().live).some(l => l.status === 'running' || l.status === 'waiting');
  void refreshLists(running ? LIST_ACTIVE_MS - 1000 : LIST_IDLE_MS - 1000);
}

// Back in the foreground: catch up on lists and subscriptions (a socket that stayed open
// gets no presence frame; one that closed reconnects and catches up on presence).
AppState.addEventListener('change', next => {
  if (next !== 'active' || !socket.open) return;
  void refreshLists(LIST_FRESH_MS);
  void subscribeAll();
});

/** Hosts that answered 'Unknown method.' (older desktops): not asked again this session. */
const noIcons = new Set<string>();
const iconsAsking = new Set<string>();

/** Fetches the icons of these projects that are new or a day old, in batches; failures wait for the next call. */
async function projectIcons(deviceId: string, projectIds: string[]): Promise<void> {
  const version = securityWrites.version;
  await loadIcons();
  if (!ownsSession(version)) return;
  if (noIcons.has(deviceId)) return;
  const due = dueIcons(useIcons.getState().cache, deviceId, projectIds, Date.now()).filter(id => !iconsAsking.has(`${deviceId}/${id}`));
  if (!due.length) return;
  due.forEach(id => iconsAsking.add(`${deviceId}/${id}`));
  try {
    for (const batch of chunks(due)) {
      const reply = await call<{ icons?: unknown }>(deviceId, 'projects.icons', { projectIds: batch }, TIMEOUT_MS, true);
      requireSession(version);
      useIcons.setState(s => ({ cache: mergeIcons(s.cache, deviceId, batch, reply?.icons, Date.now()) }));
    }
  } catch (error) {
    if (ownsSession(version) && (error as Error).message === 'Unknown method.') noIcons.add(deviceId);
  } finally {
    if (ownsSession(version)) {
      due.forEach(id => iconsAsking.delete(`${deviceId}/${id}`));
      void saveIcons();
    }
  }
}

async function listProviders(deviceId: string, force = false): Promise<Provider[]> {
  const version = securityWrites.version;
  const providers = await call<Provider[]>(deviceId, 'providers.list', { force });
  requireSession(version);
  patch(s => ({ providers: { ...s.providers, [deviceId]: providers } }));
  return providers;
}

async function listModels(deviceId: string, provider: string, force = false): Promise<ProviderModel[]> {
  const version = securityWrites.version;
  const models = await call<ProviderModel[]>(deviceId, 'providers.models', { provider, force });
  requireSession(version);
  patch(s => ({ models: { ...s.models, [`${deviceId}/${provider}`]: models } }));
  return models;
}

/** Starts a task on the host (repo OR projectId) and watches it. */
async function startTask(deviceId: string, params: StartParams): Promise<string> {
  const version = securityWrites.version;
  if (!params.repo === !params.projectId) throw new Error('Choose a repository or a project.');
  const { sessionId } = await call<{ sessionId: string }>(deviceId, 'task.start', params, START_TIMEOUT_MS);
  requireSession(version);
  watch(deviceId, [sessionId]);
  patch(s => ({ live: { ...s.live, [taskKey(deviceId, sessionId)]: { ...idleLive(), status: 'running', activity: 'Starting' } } }));
  return sessionId;
}

/** Applies `edit` to one task's sent list; unchanged → same object, an empty list is dropped. */
function editSent(all: ConnectState['sent'], key: string, edit: (sent: Sent[]) => Sent[]): ConnectState['sent'] {
  const list = all[key];
  if (!list) return all;
  const next = edit(list);
  if (next === list) return all;
  const { [key]: _, ...rest } = all;
  return next.length ? { ...rest, [key]: next } : rest;
}

/** Shows `text` in the transcript at once while `send` runs: kept (as queued if the host queued
 * it) until the stored copy replaces it, dropped if the send fails. */
async function optimistic<T>(deviceId: string, sessionId: string, text: string, initial: Sent['state'], send: () => Promise<T>, queued?: (reply: T) => boolean): Promise<T> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  const entry: Sent = { id: uuid(), text, state: initial, after: lastId(state().messages[key]) };
  patch(s => ({ sent: { ...s.sent, [key]: [...(s.sent[key] ?? []), entry] } }));
  prefetch.hold(`send:${entry.id}`);
  const edit = (change: (e: Sent) => Sent | null) =>
    ownsSession(version) && patch(s => ({ sent: editSent(s.sent, key, list => list.flatMap(e => (e.id === entry.id ? change(e) ?? [] : [e]))) }));
  try {
    const reply = await send();
    requireSession(version);
    if (queued?.(reply)) edit(e => ({ ...e, state: 'queued' }));
    return reply;
  } catch (error) {
    edit(() => null);
    throw error;
  } finally {
    prefetch.release(`send:${entry.id}`);
  }
}

const sendMessage = (deviceId: string, sessionId: string, text: string) =>
  optimistic(deviceId, sessionId, text, 'sending', () => call<{ queued: boolean }>(deviceId, 'task.send', { sessionId, text }), reply => !!reply?.queued);
/** Edits one task's live view (and the list's status with it); returns the view before. */
function editLive(deviceId: string, sessionId: string, edit: (live: Live) => Live): Live {
  const key = taskKey(deviceId, sessionId);
  const before = state().live[key] ?? idleLive();
  patch(s => {
    const live = { ...s.live, [key]: edit(s.live[key] ?? idleLive()) };
    return { live, tasks: withStatuses(s, deviceId, live) };
  });
  return before;
}

/** Shows "Stopping…" at once; a refused stop puts the activity back. */
const stopTask = (deviceId: string, sessionId: string) => attempt(() => {
  const version = securityWrites.version;
  const before = editLive(deviceId, sessionId, stopping);
  return () => { if (ownsSession(version)) editLive(deviceId, sessionId, live => unstopping(live, before.activity)); };
}, () => call<object>(deviceId, 'task.stop', { sessionId }));
/** The host saves a steer when the agent accepts it, before it answers. */
async function steerTask(deviceId: string, sessionId: string, text: string) {
  const version = securityWrites.version;
  const reply = await optimistic(deviceId, sessionId, text, 'steering', () => call<object>(deviceId, 'task.steer', { sessionId, text }));
  requireSession(version);
  void loadMessages(deviceId, sessionId).catch(() => undefined);
  return reply;
}
/** The list shows the new settings at once; a refused change puts back what it set. */
function configureTask(deviceId: string, params: ConfigureParams) {
  const version = securityWrites.version;
  const { sessionId, ...change } = params;
  const editTasks = (edit: (tasks: TaskSummary[]) => TaskSummary[]) => ownsSession(version) && patch(s => {
    const list = s.tasks[deviceId];
    return list ? { tasks: { ...s.tasks, [deviceId]: { ...list, tasks: edit(list.tasks) } } } : {};
  });
  return attempt(() => {
    const task = state().tasks[deviceId]?.tasks.find(t => t.sessionId === sessionId);
    editTasks(tasks => configured(tasks, sessionId, change));
    const before = task ? { provider: task.provider, model: task.model, effort: task.effort, mode: task.mode } : null;
    return () => { if (before) editTasks(tasks => unconfigured(tasks, sessionId, change, before)); };
  }, () => call<object>(deviceId, 'task.configure', params));
}

/** An answered request leaves the live view at once (the card collapses), as on the desktop; a
 * refused answer brings it back. */
function answered(deviceId: string, sessionId: string, requestId: string, kind: 'approval_cancelled' | 'input_cancelled', send: () => Promise<object>) {
  const version = securityWrites.version;
  return attempt(() => {
    const before = state().live[taskKey(deviceId, sessionId)];
    const request = before && [...before.approvals, ...before.inputs].find(r => r.requestId === requestId);
    editLive(deviceId, sessionId, live => applyEvent(live, { sessionId, kind, text: '', requestId }));
    return () => { if (ownsSession(version) && before && request) editLive(deviceId, sessionId, live => unanswered(live, request, before)); };
  }, async () => {
    await send();
    return {};
  });
}
const respondApproval = (deviceId: string, sessionId: string, requestId: string, approve: boolean) =>
  answered(deviceId, sessionId, requestId, 'approval_cancelled', () => call(deviceId, 'approval.respond', { sessionId, requestId, approve }));
const respondInput = (deviceId: string, sessionId: string, requestId: string, response: unknown) =>
  answered(deviceId, sessionId, requestId, 'input_cancelled', () => call(deviceId, 'input.respond', { sessionId, requestId, response }));

// ---- Pull requests, changes and bots (newer hosts) ----

/** A call to a method older desktops lack: their 'Unknown method.' becomes an update note, and the host is remembered as outdated. */
async function newer<T>(deviceId: string, method: Method, params: unknown, timeoutMs?: number, background = false): Promise<T> {
  const version = securityWrites.version;
  try {
    return await call<T>(deviceId, method, params, timeoutMs, background);
  } catch (error) {
    requireSession(version);
    if ((error as Error).message !== UNKNOWN_METHOD) throw error;
    patch(s => ({ outdated: { ...s.outdated, [deviceId]: true } }));
    throw new Error(outdated(state().devices.find(d => d.id === deviceId)?.name ?? 'that computer'));
  }
}

const pullOf = (reply: { pull?: unknown } | undefined) => (reply?.pull && typeof reply.pull === 'object' ? reply.pull as PullDetail : null);
const setPull = (key: string, pull: PullDetail | null) => patch(s => ({ pulls: { ...s.pulls, [key]: pull } }));
const checking = new Map<string, Promise<PullDetail | null>>();

/** The task's linked pull request, else one its turns opened (task.pullDetect links it). Cached per task unless `force`. */
function loadPull(deviceId: string, sessionId: string, force = false): Promise<PullDetail | null> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  if (!force && key in state().pulls) return Promise.resolve(state().pulls[key]);
  if (state().outdated[deviceId]) return Promise.resolve(null);
  const run = checking.get(key) ?? (async () => {
    try {
      const pull = pullOf(await newer(deviceId, 'task.pull', { sessionId })) ?? pullOf(await newer(deviceId, 'task.pullDetect', { sessionId }));
      requireSession(version);
      setPull(key, pull);
      return pull;
    } finally {
      if (ownsSession(version)) checking.delete(key);
    }
  })();
  checking.set(key, run);
  return run;
}

async function pullDiff(deviceId: string, sessionId: string, path: string): Promise<Diff> {
  const reply = await newer<{ patch?: string; truncated?: boolean }>(deviceId, 'task.pullDiff', { sessionId, path });
  return { text: typeof reply?.patch === 'string' ? reply.patch : '', truncated: !!reply?.truncated };
}
async function mergePull(deviceId: string, sessionId: string, method: MergeMethod): Promise<void> {
  await newer(deviceId, 'task.pullMerge', { sessionId, method }, MERGE_TIMEOUT_MS);
  await loadPull(deviceId, sessionId, true);
}

async function taskChanges(deviceId: string, sessionId: string): Promise<Changes> {
  const reply = await newer<Partial<Changes>>(deviceId, 'task.changes', { sessionId });
  return { branch: reply?.branch ?? '', ahead: reply?.ahead ?? 0, behind: reply?.behind ?? 0, files: Array.isArray(reply?.files) ? reply.files : [] };
}
async function taskDiff(deviceId: string, sessionId: string, path?: string, staged = false): Promise<Diff> {
  const reply = await newer<{ diff?: string; truncated?: boolean }>(deviceId, 'task.diff', { sessionId, ...(path ? { path } : {}), staged });
  return { text: typeof reply?.diff === 'string' ? reply.diff : '', truncated: !!reply?.truncated };
}

/** One host's bots; an older host has none (and isn't asked again). */
async function listBots(deviceId: string): Promise<Bot[]> {
  const version = securityWrites.version;
  let bots: Bot[];
  try {
    const reply = await newer<{ bots?: Bot[] }>(deviceId, 'bots.list', {}, TIMEOUT_MS, true);
    bots = Array.isArray(reply?.bots) ? reply.bots.filter(b => typeof b?.id === 'string') : [];
  } catch (error) {
    requireSession(version);
    if (!state().outdated[deviceId]) throw error;
    bots = [];
  }
  requireSession(version);
  patch(s => ({ bots: { ...s.bots, [deviceId]: bots } }));
  return bots;
}

const editBot = (deviceId: string, botId: string, change: Partial<Bot>) =>
  patch(s => ({ bots: { ...s.bots, [deviceId]: (s.bots[deviceId] ?? []).map(b => (b.id === botId ? { ...b, ...change } : b)) } }));

/** The bot's desktop conversation (created on first use) → sessionId. */
async function botThread(deviceId: string, botId: string): Promise<string> {
  const version = securityWrites.version;
  const known = state().bots[deviceId]?.find(b => b.id === botId)?.desktopSession;
  if (known) return known;
  const { sessionId } = await newer<{ sessionId: string }>(deviceId, 'bot.thread', { botId });
  requireSession(version);
  editBot(deviceId, botId, { desktopSession: sessionId });
  void listTasks(deviceId).catch(() => undefined);
  return sessionId;
}

/** Sends through the bot (its brief and memories apply), shown at once in its conversation. */
const botMessage = (deviceId: string, botId: string, sessionId: string, text: string) =>
  optimistic(deviceId, sessionId, text, 'sending', () => newer<{ sessionId: string }>(deviceId, 'bot.message', { botId, text }));

/** Shows paused/resumed at once; a refusal puts the bot back. */
async function pauseBot(deviceId: string, botId: string, paused: boolean): Promise<void> {
  const version = securityWrites.version;
  await attempt(() => {
    const bot = state().bots[deviceId]?.find(b => b.id === botId);
    editBot(deviceId, botId, { paused, state: paused ? 'paused' : 'ready' });
    return () => { if (ownsSession(version) && bot) patch(s => ({ bots: { ...s.bots, [deviceId]: unpaused(s.bots[deviceId] ?? [], botId, paused, bot) } })); };
  }, () => newer(deviceId, 'bot.pause', { botId, paused }));
  requireSession(version);
  void listBots(deviceId).catch(() => undefined);
}

// ---- Teams (newer hosts) ----

/** Hosts without task.team, so one 'Unknown method.' doesn't mark pulls and bots outdated too. */
const teamless = new Set<string>();

/** The task's team (null: not an architect task), stored per task; 'outdated' on hosts without task.team. */
async function loadTeam(deviceId: string, sessionId: string): Promise<Team | null | 'outdated'> {
  const version = securityWrites.version;
  const key = taskKey(deviceId, sessionId);
  if (teamless.has(deviceId)) return 'outdated';
  try {
    const team = decodeTeam(await call(deviceId, 'task.team', { sessionId }, TIMEOUT_MS, true));
    requireSession(version);
    patch(s => ({ teams: { ...s.teams, [key]: team } }));
    return team;
  } catch (error) {
    if ((error as Error).message !== UNKNOWN_METHOD) throw error;
    requireSession(version);
    teamless.add(deviceId);
    patch(s => ({ teams: { ...s.teams, [key]: 'outdated' } }));
    return 'outdated';
  }
}

/** A team worker's transcript, oldest first (afterId pages until the host has no more). */
async function workerMessages(deviceId: string, sessionId: string, workerId: string, afterId = 0): Promise<Message[]> {
  let all: Message[] = [];
  let cursor = afterId;
  for (let page = 0; page < MAX_PAGES; page++) {
    const reply = await call<Partial<MessagePage>>(deviceId, 'task.workerMessages', { sessionId, workerId, ...(cursor ? { afterId: cursor } : {}) });
    const fresh = (Array.isArray(reply?.messages) ? reply.messages : []).filter(m => typeof m?.id === 'number' && m.id > cursor);
    all = [...all, ...fresh];
    if (!reply?.hasMore || !fresh.length) return all;
    cursor = fresh[fresh.length - 1].id;
  }
  return all;
}

// ---- Attachments ----

const attachmentDeps: AttachmentDeps = {
  call: (deviceId, method, params) => call(deviceId, method, params, TIMEOUT_MS, true),
  // Below the thread's own message calls: wait for whatever is queued for it now.
  after: (deviceId, sessionId) => (queues.get(taskKey(deviceId, sessionId)) ?? Promise.resolve()).catch(() => undefined),
  // Above prefetch: it holds while any download waits.
  hold: () => prefetch.hold('attachments'),
  release: () => prefetch.release('attachments'),
  demo: id => (state().demo ? import('../demo-images.ts').then(m => m.demoImage(id)) : undefined),
};

/** A message attachment or workspace image as a file:// URI (see ./attachments.ts). */
const attachment = (deviceId: string, sessionId: string, source: Source, variant: Variant) =>
  fetchAttachment(attachmentDeps, deviceId, sessionId, source, variant);

export const connect = {
  init, signInStart, signInFinish, signInCancel, signOut,
  enterDemo, leaveDemo, deleteAccount,
  refreshDevices, revokeDevice, setTrusted,
  watch, unwatch, loadMessages, loadOlder, peek,
  listTasks, refreshLists, listProviders, listModels, startTask, projectIcons,
  sendMessage, stopTask, steerTask, configureTask, respondApproval, respondInput,
  loadPull, pullDiff, mergePull, taskChanges, taskDiff, listBots, botThread, botMessage, pauseBot,
  loadTeam, workerMessages,
  attachment,
};
