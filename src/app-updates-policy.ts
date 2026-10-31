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

export const CHECK_EVERY = 6 * 60 * 60 * 1000;
/** A downloaded update installs after this long without input, when no agent is working. */
export const IDLE_BEFORE_RESTART = 2 * 60 * 1000;

export const UPDATE_POLICY_MESSAGE = 'External updates are disabled in this hardened build. Install reviewed releases manually.';

/** Legacy callers always receive false: this build cannot install external updates. */
export function readyToRestart(_busy: boolean, _lastInput: number, _now = Date.now()): boolean {
  return false;
}

/** One line for Settings → About. */
export function describeUpdate(state: UpdateState, automatic: boolean | 'automatic' | 'notify' | 'off', now = Date.now()): string {
  // Pre-hardening callers used string update modes; 'notify' must not be
  // treated as automatic merely because it is truthy.
  const auto = automatic === true || automatic === 'automatic';
  switch (state.kind) {
    case 'off': return UPDATE_POLICY_MESSAGE;
    case 'checking': return 'Checking GitHub Releases…';
    case 'current': { const minutes = Math.round((now - state.checkedAt) / 60_000); return `Up to date · checked ${minutes < 1 ? 'just now' : minutes < 60 ? `${minutes}m ago` : `${Math.round(minutes / 60)}h ago`}`; }
    case 'available': return `PandaCode ${state.version} is available.`;
    case 'downloading': return `Downloading ${state.version}${state.percent === null ? '…' : ` · ${state.percent}%`}`;
    case 'ready': return `${state.version} is ready. ${auto ? 'PandaCode restarts when your agents are done and you step away.' : 'Restart to finish.'}`;
    case 'installing': return `Installing ${state.version}…`;
    case 'error': return `Couldn’t check for updates: ${state.message}`;
    default: return automatic === false || automatic === 'off' ? 'Automatic updates are off.' : 'Checks GitHub Releases automatically.';
  }
}
