import { preview } from './bridge';
import { icon } from './dom';
import { appUpdates, describeUpdate } from './app-updates';

/** Public upstream project links. Opening one is user-initiated; no update checks run. */
export const PROJECT_LINKS = {
  repository: 'https://github.com/aicodeking/pandacode',
  issues: 'https://github.com/aicodeking/pandacode/issues',
  releases: 'https://github.com/aicodeking/pandacode/releases',
  license: 'https://github.com/aicodeking/pandacode/blob/main/LICENSE',
} as const;

const link = (label: string, url: string) =>
  `<button type="button" class="secondary-button" data-action="open-url" data-url="${url}">${label}${icon('forward')}</button>`;

/** The About group: app version, license, disabled update policy, and upstream links. */
export function aboutSettingsHtml(): string {
  return `<div class="settings-group settings-about" aria-labelledby="settings-about-heading">
    <div class="setting-row"><div><strong id="settings-about-heading">About PandaCode — 0.3.34.hardened</strong><small class="settings-about-version">Version …</small></div></div>
    <div class="setting-row"><div><strong>License</strong><small>Open source (MIT-based). No account, no telemetry.</small></div>${link('PandaCode License', PROJECT_LINKS.license)}</div>
    <div class="setting-row settings-updates"><div><strong>Updates</strong><small class="settings-update-status" aria-live="polite">External updates are disabled in this hardened build. Install reviewed releases manually.</small></div><div class="settings-row-actions"><button type="button" class="secondary-button settings-update-action" hidden></button>${link('Upstream releases', PROJECT_LINKS.releases)}</div></div>
    <div class="setting-row settings-updates-auto" hidden><div><strong>Automatic updates disabled</strong><small>This build cannot download or install external app updates.</small></div><input type="checkbox" class="settings-switch" data-update-automatic aria-label="Install updates automatically" disabled></div>
    <div class="setting-row"><div><strong>Source code</strong><small>This is a fork of PandaCode. Upstream downloads do not include this security revision.</small></div><div class="settings-row-actions">${link('GitHub repository', PROJECT_LINKS.repository)}${link('Issues', PROJECT_LINKS.issues)}</div></div>
  </div>`;
}

/** Reads the bundle version from Tauri; the browser preview has no app bundle. */
async function appVersion(): Promise<string> {
  if (preview) return 'Preview';
  return (await import('@tauri-apps/api/app')).getVersion();
}

export function mountAboutSettings(host: HTMLElement): () => void {
  const version = host.querySelector<HTMLElement>('.settings-about-version');
  let disposed = false;
  const cleanupUpdates = mountUpdates(host);
  if (version) {
    appVersion()
      .then(value => { if (!disposed) version.textContent = `Version ${value.replace(/-hardened$/, '.hardened')}`; })
      .catch(() => { if (!disposed) version.textContent = 'Version unavailable'; });
  }
  return () => { disposed = true; cleanupUpdates(); };
}

/** The Updates rows follow the updater: status, Check now / Restart now, and the automatic switch. */
function mountUpdates(host: HTMLElement): () => void {
  const updates = appUpdates();
  const status = host.querySelector<HTMLElement>('.settings-update-status')!;
  const action = host.querySelector<HTMLButtonElement>('.settings-update-action')!;
  const auto = host.querySelector<HTMLInputElement>('[data-update-automatic]')!;
  if (!updates?.supported) {
    status.textContent = describeUpdate({ kind: 'off' }, false);
    host.querySelector<HTMLElement>('.settings-updates-auto')?.remove();
    return () => {};
  }
  const paint = () => {
    const state = updates.state;
    status.textContent = describeUpdate(state, updates.automatic);
    auto.checked = updates.automatic;
    const restartable = state.kind === 'ready' || state.kind === 'available';
    action.hidden = state.kind === 'downloading' || state.kind === 'installing';
    action.disabled = state.kind === 'checking';
    action.textContent = restartable ? (state.kind === 'ready' ? 'Restart now' : 'Install') : 'Check now';
  };
  const click = () => { void (updates.state.kind === 'ready' || updates.state.kind === 'available' ? updates.restart() : updates.check()); };
  const change = () => updates.setAutomatic(auto.checked);
  action.addEventListener('click', click);
  auto.addEventListener('change', change);
  const unsubscribe = updates.subscribe(paint);
  paint();
  return () => { unsubscribe(); action.removeEventListener('click', click); auto.removeEventListener('change', change); };
}
