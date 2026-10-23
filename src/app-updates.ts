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
  readonly automatic: boolean;
  readonly supported: boolean;
  subscribe(listener: () => void): () => void;
  setAutomatic(on: boolean): void;
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
