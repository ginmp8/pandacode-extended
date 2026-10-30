/* Retain the state contract for existing UI callers; external updates are disabled
 * by this build, regardless of preferences saved by an earlier installation. */
export type UpdateState =
  | { kind: 'off' }
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'current'; checkedAt: number }
  | { kind: 'available'; version: string }
  | { kind: 'downloading'; version: string; percent: number | null }
  | { kind: 'ready'; version: string }
  | { kind: 'installing'; version: string }
  | { kind: 'error'; message: string };

/** Automatic installs when idle; Notify checks and asks first; Off checks only on request. */
export type UpdateMode = 'automatic' | 'notify' | 'off';

export const CHECK_EVERY = 6 * 60 * 60 * 1000;
/** A downloaded update installs after this long without input, when no agent is working. */
export const IDLE_BEFORE_RESTART = 2 * 60 * 1000;

export const UPDATE_POLICY_MESSAGE = 'External updates are disabled in this hardened build. Install reviewed releases manually.';

/** Legacy callers always receive false: this build cannot install external updates. */
export function readyToRestart(_busy: boolean, _lastInput: number, _now = Date.now()): boolean {
  return false;
}

/** Whether a downloaded update restarts PandaCode now: on its own only in Automatic, or once
 * the user clicked Install and restart; never while an agent is working. */
export function restartNow(mode: UpdateMode, approved: boolean, busy: boolean, lastInput: number, now = Date.now()): boolean {
  if (busy) return false;
  return approved || (mode === 'automatic' && readyToRestart(busy, lastInput, now));
}

/** The status-bar prompt for a state: Install and restart for a found update (unless
 * dismissed with Later), Restart to update once downloaded. Null hides it. */
export function updatePrompt(state: UpdateState, installable: boolean, dismissed: string | null): { label: string; title: string; later: boolean } | null {
  switch (state.kind) {
    case 'available':
      if (state.version === dismissed) return null;
      return installable
        ? { label: 'Install and restart', title: `PandaCode ${state.version} is available. Installs when your agents are done.`, later: true }
        : { label: `${state.version} available`, title: `PandaCode ${state.version} is available. Open GitHub Releases to update this copy.`, later: true };
    case 'ready': return { label: 'Restart to update', title: `PandaCode ${state.version} is ready. Restart now, or it restarts on its own when your agents are done.`, later: false };
    case 'installing': return { label: 'Restart to update', title: `Installing ${state.version}…`, later: false };
    default: return null;
  }
}

/** One line for Settings → About. */
export function describeUpdate(state: UpdateState, mode: UpdateMode, now = Date.now(), installable = true): string {
  switch (state.kind) {
    case 'off': return UPDATE_POLICY_MESSAGE;
    case 'checking': return 'Checking GitHub Releases…';
    case 'current': { const minutes = Math.round((now - state.checkedAt) / 60_000); return `Up to date · checked ${minutes < 1 ? 'just now' : minutes < 60 ? `${minutes}m ago` : `${Math.round(minutes / 60)}h ago`}`; }
    case 'available': return installable ? `PandaCode ${state.version} is available.` : `PandaCode ${state.version} is available. Updates for this copy are installed by its package or a new AppImage.`;
    case 'downloading': return `Downloading ${state.version}${state.percent === null ? '…' : ` · ${state.percent}%`}`;
    case 'ready': return `${state.version} is ready. ${mode === 'automatic' ? 'PandaCode restarts when your agents are done and you step away.' : 'Restart to finish.'}`;
    case 'installing': return `Installing ${state.version}…`;
    case 'error': return `Couldn’t check for updates: ${state.message}`;
    default: return mode === 'off' ? 'Automatic updates are off.' : 'Checks GitHub Releases automatically.';
  }
}
