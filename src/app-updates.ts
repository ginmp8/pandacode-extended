/* External updates are deliberately unavailable in this hardened build.
 * Keep the controller contract for existing UI callers, but never read an old
 * auto-update preference, load an updater plugin, schedule work, or restart.
 * The native updater plugin and its capabilities are removed separately. */
import type { UpdateState } from './app-updates-policy.ts';
export { describeUpdate, type UpdateState } from './app-updates-policy.ts';

export interface UpdatesHost {
  busy(): boolean;
  notify(text: string): void;
}
export interface AppUpdates {
  readonly state: UpdateState;
  readonly mode: UpdateMode;
  readonly supported: boolean;
  /** False for a Linux run outside an AppImage: updates are reported, never installed. */
  readonly installable: boolean;
  subscribe(listener: () => void): () => void;
  setMode(mode: UpdateMode): void;
  check(): Promise<void>;
  restart(): Promise<void>;
}

let instance: AppUpdates | undefined;
export const appUpdates = (): AppUpdates | undefined => instance;

export function startAppUpdates(_host: UpdatesHost): AppUpdates {
  const state: UpdateState = Object.freeze({ kind: 'off' });
  instance = {
    state,
    automatic: false,
    supported: false,
    subscribe: () => () => {},
    setAutomatic: () => {},
    async check() {},
    async restart() {},
  };
  return instance;
}

/** The header prompt (`#update-status`): Install and restart / Later for a found update,
 * Restart to update once downloaded. Later hides it until a newer version turns up. */
export function mountUpdatePill(pill: HTMLButtonElement, updates: AppUpdates): void {
  const label = pill.querySelector('span')!;
  const later = document.createElement('button');
  later.type = 'button';
  later.className = 'update-status update-later';
  later.textContent = 'Later';
  later.hidden = true;
  pill.after(later);
  let dismissed: string | null = null;
  const paint = () => {
    const state = updates.state;
    const prompt = updatePrompt(state, updates.installable, dismissed);
    pill.hidden = !prompt;
    later.hidden = !prompt?.later;
    if (!prompt) return;
    label.textContent = prompt.label;
    pill.title = prompt.title;
    pill.disabled = state.kind === 'installing';
  };
  updates.subscribe(paint);
  pill.addEventListener('click', () => {
    if (updates.state.kind === 'available' && !updates.installable) void call('open_external_url', { url: PROJECT_LINKS.releases });
    else void updates.restart();
  });
  later.addEventListener('click', () => { if (updates.state.kind === 'available') dismissed = updates.state.version; paint(); });
  paint();
}
