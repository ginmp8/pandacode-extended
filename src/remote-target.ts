import { escape, icon } from './dom';
import type { RemoteRepoCheck, RemoteTarget } from './remote-types';
import type { CloudTarget } from './cloud-agents';
import type { Session } from './types';
// Start progress for a New task sent to another computer; importing it here loads its strip actions.
export { cancelledRemoteStart, clearRemoteStart, remoteStart, remoteStartAttached, remoteStartStrip, startStageText, trackRemoteStart } from './remote-progress';

/* The context bar's computer choice (PandaCode Connect). A new task can start on another
 * signed-in computer; a task that runs there shows that computer, locked. */

export const LOCAL = 'local';
/** Opens Settings → Remote access when no other computer can take a task. */
export const CONNECT_SETUP = 'connect-setup';

export function lastSeenText(target: Pick<RemoteTarget, 'lastSeen'>): string {
  const raw = target.lastSeen;
  const time = typeof raw === 'number' ? (raw < 1e12 ? raw * 1000 : raw) : typeof raw === 'string' ? Date.parse(raw) : NaN;
  if (!Number.isFinite(time)) return 'last seen unknown';
  const elapsed = Math.max(0, Date.now() - time);
  if (elapsed < 60_000) return 'last seen just now';
  if (elapsed < 3_600_000) return `last seen ${Math.floor(elapsed / 60_000)}m ago`;
  if (elapsed < 86_400_000) return `last seen ${Math.floor(elapsed / 3_600_000)}h ago`;
  return `last seen ${new Date(time).toLocaleDateString()}`;
}
export function offlineText(target: RemoteTarget): string { return `Offline · ${lastSeenText(target)}`; }


/** The control beside Local / Worktree; empty when there is nothing to choose. */
export function computerControl(session: Session, fresh: boolean, targets: RemoteTarget[], chosen: string, chosenName?: string): string {
  if (session.remoteDevice) {
    const name = escape(session.remoteDevice.name), target = targets.find(item => item.deviceId === session.remoteDevice!.id);
    const offline = target && !target.online;
    const status = offline ? ` · ${escape(offlineText(target))}` : '';
    return `<span class="composer-computer composer-computer-fixed" aria-label="Computer: ${name} (fixed for this task)${status}" title="This task runs on ${name}. Start a new task to use another computer.${status}">${icon('monitor')}<span>on ${name}${offline ? ' · Offline' : ''}</span></span>`;
  }
  if (!fresh || (!targets.length && chosen === LOCAL)) return '';
  const target = targets.find(item => item.deviceId === chosen);
  const name = chosen === LOCAL ? 'This computer' : target?.name ?? chosenName ?? 'That computer';
  const status = chosen === LOCAL ? '' : !target ? ' · Not paired' : !target.online ? ` · ${offlineText(target)}` : '';
  return `<button type="button" id="composer-computer" class="composer-computer" data-action="computer-menu" aria-label="Computer: ${escape(name + status)}" aria-haspopup="dialog" aria-expanded="false" title="Where this task runs">${icon('monitor')}<span>${escape(name)}${target && !target.online ? ' · Offline' : !target && chosen !== LOCAL ? ' · Not paired' : ''}</span>${icon('down')}</button>`;
}

export function computerItems(targets: RemoteTarget[], chosen: string, clouds: CloudTarget[] = []) {
  return [
    { id: LOCAL, label: 'This computer', icon: 'monitor', selected: chosen === LOCAL },
    ...targets.map(target => ({ id: target.deviceId, label: target.name, description: target.online ? target.platform : offlineText(target), icon: 'monitor', selected: target.deviceId === chosen, disabled: !target.online })),
    ...clouds.map(target => ({ id: `cloud:${target.provider}`, label: target.label, description: 'Cloud agent machine', icon: 'cloud', selected: chosen === `cloud:${target.provider}`, disabled: !target.ready })),
  ];
}

/** Why GitHub is behind this computer, as one sentence, or '' when it isn't. */
export function behindText(check: RemoteRepoCheck): string {
  const reasons = [
    check.ahead > 0 ? `${check.ahead} unpushed ${check.ahead === 1 ? 'commit' : 'commits'}` : '',
    check.dirty ? 'uncommitted changes' : '',
    check.pushed ? '' : `branch ${check.branch} not pushed`,
  ].filter(Boolean);
  return reasons.length ? `GitHub is behind this computer: ${reasons.join(', ')}.` : '';
}

export function behindDialog(name: string, check: RemoteRepoCheck): string {
  return `<form id="remote-behind-form"><p class="dialog-description">${escape(behindText(check))} ${escape(name)} will start from GitHub’s ${escape(check.branch)}.</p><div class="dialog-footer"><button type="button" class="secondary-button" data-action="close-dialog">Cancel</button><button type="submit" class="primary-button">Continue</button></div></form>`;
}
