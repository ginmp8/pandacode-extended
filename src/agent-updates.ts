import './agent-updates.css';
import type { Provider, ProviderId } from './types';

export interface AgentUpdate {
  provider: ProviderId;
  name: string;
  providers: ProviderId[];
  installedVersion: string | null;
  latestVersion: string | null;
  status: 'available' | 'current' | 'unsupported' | 'error';
  method: string | null;
  canUpdate: boolean;
  message: string | null;
  checkedAt: number;
  docsUrl?: string | null;
}
export interface AgentUpdateProgress { provider: ProviderId; phase: 'updating' | 'verifying' | 'completed' | 'error'; message?: string | null; }
export interface AgentUpdatesHost {
  modal(title: string, body: string, className?: string): HTMLDialogElement;
  closeModal(): void;
  providersChanged(): Promise<void>;
  providers(): Provider[];
  report(error: unknown): void;
}
export interface AgentUpdatesController { show(): Promise<void>; dispose(): void; }

/** No metadata requests or installers run in this build, including on startup,
 * focus, visibility changes, or an old UI calling the retained controller. */
export function initializeAgentUpdates(host: AgentUpdatesHost): AgentUpdatesController {
  let disposed = false;
  let close: HTMLButtonElement | null = null;
  const done = () => host.closeModal();
  return {
    async show() {
      if (disposed) return;
      close?.removeEventListener('click', done);
      const dialog = host.modal('Agent updates', `<section class="agent-updates"><p class="agent-updates-status" role="status">External agent update checks and installers are disabled in this hardened build. Install reviewed agent versions manually outside PandaCode.</p><div class="dialog-footer"><button type="button" class="secondary-button agent-updates-close">Done</button></div></section>`, 'agent-updates-dialog');
      close = dialog.querySelector<HTMLButtonElement>('.agent-updates-close');
      close?.addEventListener('click', done);
      close?.focus({ preventScroll: true });
    },
    dispose() {
      disposed = true;
      close?.removeEventListener('click', done);
      close = null;
    },
  };
}
