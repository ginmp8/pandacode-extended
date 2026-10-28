/** Browser preview for PandaCode Connect: in-memory sample account and computers, no network. Native builds use src-tauri/src/remote. */
import type { RemoteDevice, RemoteSignIn, RemoteStatus, RemoteTarget } from './remote-types';
import { downgradePreviewFullGrants, revokePreviewGrant } from './preview-grants';

const THIS = 'dev-this';
let signedIn = false, hostEnabled = false, allowFullAccess = false, directRoutes = false;
let devices: RemoteDevice[] = [], cancelSignIn: (() => void) | undefined;
const offline = new Set<string>();
const revoked = new Set<string>();
const trusted = new Set<string>(['dev-studio', 'dev-laptop']);
const status = (): RemoteStatus => ({ configured: true, signedIn, account: signedIn ? { login: 'octocat' } : undefined, device: { id: THIS, name: 'This Mac', fingerprint: '3fa91c07d2b64e584a810ea7b9c3d25f' }, hostEnabled, allowFullAccess, directRoutes, connected: signedIn });
const sampleDevices = (): RemoteDevice[] => [
  { id: THIS, name: 'This Mac', platform: 'macOS', fingerprint: '3fa91c07d2b64e584a810ea7b9c3d25f', host: hostEnabled, online: true, trusted: true, lastSeen: Date.now(), current: true },
  { id: 'dev-studio', name: 'Studio iMac', platform: 'macOS', fingerprint: '9b02e4aa71c3d6053f102e49a8410b67', host: true, online: !offline.has('dev-studio'), trusted: trusted.has('dev-studio'), lastSeen: Date.now(), current: false },
  { id: 'dev-nas', name: 'Home NAS', platform: 'Linux', fingerprint: '17aa5e90b3c4d286ea5bb53d782f9104', host: true, online: true, trusted: trusted.has('dev-nas'), lastSeen: Date.now(), current: false },
  { id: 'dev-laptop', name: 'Old laptop', platform: 'Windows', fingerprint: 'c4d81f3e0a9957b2736ca19d52e54086', host: true, online: false, trusted: trusted.has('dev-laptop'), lastSeen: Date.now() - 86_400_000, current: false },
].filter(device => !revoked.has(device.id));
export function invokeRemote<T>(command: string, args: Record<string, any>, emit: (name: string, payload: unknown) => void): Promise<T> {
  return Promise.resolve().then(() => handle(command, args, emit) as T);
}
function handle(command: string, args: Record<string, any>, emit: (name: string, payload: unknown) => void): unknown {
  const w = window as any;
  switch (command) {
    case 'remote_status':
      w.__previewAllowFullAccess = allowFullAccess;
      if (w.__remoteStatusFailures > 0) { w.__remoteStatusFailures--; throw new Error('The relay is unavailable.'); }
      w.__remoteOnline = (id: string, online: boolean) => setPreviewOnline(id, online, emit);
      w.__remoteRevoke = (id: string) => { revoked.add(id); devices = sampleDevices(); emit('remote://presence', devices); };
      return status();
    case 'remote_sign_in_start': return { userCode: 'WDJB-MJHT', verificationUri: 'https://github.com/login/device', expiresIn: 900 } satisfies RemoteSignIn;
    case 'remote_sign_in_finish': return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { cancelSignIn = undefined; signedIn = true; devices = sampleDevices(); emit('remote://status', status()); resolve(status()); }, (window as any).__signInDelay ?? 400);
      cancelSignIn = () => { clearTimeout(timer); reject(new Error('Sign-in cancelled')); };
    });
    case 'remote_sign_in_cancel': cancelSignIn?.(); cancelSignIn = undefined; return {};
    case 'remote_sign_out': signedIn = false; hostEnabled = false; devices = []; emit('remote://status', status()); return status();
    case 'remote_set_host_enabled': hostEnabled = Boolean(args.enabled); devices = sampleDevices(); emit('remote://presence', devices); return status();
    case 'remote_set_full_access': allowFullAccess = Boolean(args.enabled); w.__previewAllowFullAccess = allowFullAccess; if (!allowFullAccess) downgradePreviewFullGrants(); emit('remote://status', status()); return status();
    case 'remote_set_direct_routes': directRoutes = Boolean(args.enabled); emit('remote://status', status()); return status();
    case 'remote_devices':
      if (w.__remoteDevicesFailures > 0) { w.__remoteDevicesFailures--; throw new Error('The device list is unavailable.'); }
      return devices = signedIn ? sampleDevices() : [];
    case 'remote_set_trusted': {
      const device = sampleDevices().find(d => d.id === args.deviceId && !d.current);
      if (args.trusted) {
        if (!device || !args.expectedFingerprint || args.expectedFingerprint !== device.fingerprint) throw new Error("This computer's key changed or has not been verified.");
        trusted.add(args.deviceId);
      } else {
        trusted.delete(args.deviceId);
        revokePreviewGrant(args.deviceId);
      }
      ((window as any).__trustCalls ??= []).push({ ...args });
      return devices = sampleDevices();
    }
    case 'remote_revoke_device': revoked.add(args.deviceId); trusted.delete(args.deviceId); revokePreviewGrant(args.deviceId); devices = sampleDevices(); emit('remote://presence', devices); return {};
    // Other trusted computers with remote control on; preview.ts mocks remote_repo_check and remote_start_task.
    case 'remote_targets': return (signedIn ? sampleDevices() : []).filter(d => !d.current && d.host && d.trusted).map((d): RemoteTarget => ({ deviceId: d.id, name: d.name, platform: d.platform, online: d.online, lastSeen: d.lastSeen ?? null }));
    default: throw new Error(`Unknown preview command: ${command}`);
  }
}
/** The host's start stages as `remote://start-progress` events, like `remote/start.rs` sends them.
 * Tests steer it: `__remoteStartStep` (ms per step, default 250), `__remoteStartManual` (each
 * step waits for `__remoteStartNext()`), `__remoteStartFail` (an error text sent at preparing). */
const activeStarts = new Map<string, () => void>();
/** A start whose reply was lost waits here until cancelled (or `undefined` once it attached). */
export function holdPreviewRemoteStart(sessionId: string, cancel: (() => void) | undefined): void {
  if (cancel) activeStarts.set(sessionId, () => { activeStarts.delete(sessionId); cancel(); }); else activeStarts.delete(sessionId);
}
export function cancelPreviewRemoteStart(sessionId: string): boolean {
  const cancel = activeStarts.get(sessionId); if (!cancel) return false;
  cancel(); return true;
}
export async function previewRemoteStart(args: Record<string, any>, emit: (name: string, payload: unknown) => void): Promise<void> {
  const w = window as any, repo = args.repo ?? 'orbit-labs/orbit', key = String(args.sessionId ?? args.startId ?? '');
  const step = () => new Promise<void>((resolve, reject) => {
    const cancel = () => { activeStarts.delete(key); if (w.__remoteStartManual) w.__remoteStartNext = undefined; reject(new Error('Remote start cancelled.')); };
    const next = () => { if (activeStarts.get(key) === cancel) activeStarts.delete(key); resolve(); };
    activeStarts.set(key, cancel);
    if (w.__remoteStartManual) w.__remoteStartNext = next; else setTimeout(next, w.__remoteStartStep ?? 250);
  });
  const send = (stage: string, percent: number | null = null, detail: string | null = null) => emit('remote://start-progress', { deviceId: args.deviceId, startId: args.startId ?? null, repo, stage, percent, detail });
  try {
    await step();
    for (const [stage, percent] of [['finding', null], ['downloading', null], ['downloading', 12], ['downloading', 58], ['downloading', 100], ['preparing', null]] as const) {
      send(stage, percent); await step();
    }
    const failure = w.__remoteStartFail;
    if (failure) { w.__remoteStartFail = undefined; send('error', null, failure); throw new Error(failure); }
    // `__remoteStartBusy`: the host's checkout was busy, so it reports the worktree it used instead.
    send('starting', null, w.__remoteStartBusy ?? null); await step();
  } finally { activeStarts.delete(key); }
}
/** Test hook: a computer going offline or coming back. */
export function setPreviewOnline(id: string, online: boolean, emit: (name: string, payload: unknown) => void): void {
  if (online) offline.delete(id); else offline.add(id);
  devices = sampleDevices(); emit('remote://presence', devices);
}
