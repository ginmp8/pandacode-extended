/* Settings → Remote access: sign in with GitHub, allow this computer to be driven, and
 * manage the account's devices and what each may do (scoped pairing, remote/grants.rs).
 * Nothing reaches the network until the user signs in. */
import './remote-settings.css';
import { escape, icon } from './dom';
import { infoMark } from './tooltip';
import { APPROVE_SCOPES, SCOPE_HELP, SCOPE_LABELS, groupFingerprint, type DeviceGrant, type GrantScope, type PairCode } from './remote-pairing-prompt';
import type { RemoteDevice, RemoteHost, RemoteSignIn, RemoteStatus } from './remote-types';

const TRUST_TIP = 'Both computers must trust each other before one can drive the other. Compare the fingerprint shown on the other computer with the one here before trusting it. If its key changes, you need to trust it again.';
const ALLOW_TIP = 'Remote prompts follow this computer’s approval settings; they get no more power than prompts typed here.';
const FULL_ACCESS_TIP = 'Off by default. Enable here only if trusted computers may start, continue or configure Full access or Claude Code Approve for me tasks on this computer. Turning this off blocks new remote actions; already running work can still be stopped.';

export function mountRemoteSettings(root: HTMLElement, host: RemoteHost): () => void {
  let status: RemoteStatus | undefined, devices: RemoteDevice[] = [], grants: DeviceGrant[] = [], error = '', statusError = '', devicesError = '', grantsError = '', devicesLoaded = false, loading = true, code: RemoteSignIn | undefined, expires = 0, signingIn = false, disposed = false, attempt = 0;
  let pairCode: PairCode | undefined, codeScope: GrantScope = 'control', pairTarget = '', pairInput = '', pairing = false, expiry: number | undefined;
  const picks: Record<string, string> = {};
  const message = (e: unknown) => e instanceof Error ? e.message : String(e);
  const fail = (e: unknown) => { error = message(e); render(); };
  const failure = (title: string, detail: string) => `<div class="remote-load-error" role="alert">${icon('alert')}<span><strong>${escape(title)}</strong><small>${escape(detail)}</small></span><button type="button" class="secondary-button" data-retry>Retry</button></div>`;
  const grantOf = (id: string) => grants.find(grant => grant.deviceId === id);

  const optionList = (selected: GrantScope, block = false): string =>
    [...APPROVE_SCOPES, ...(block ? [{ id: 'blocked' as GrantScope, label: 'Blocked' }] : [])]
      .map(option => `<option value="${option.id}"${selected === option.id ? ' selected' : ''}>${option.label}</option>`).join('');
  const codeText = (digits: string) => digits.slice(0, 3) + (digits.length > 3 ? ' ' + digits.slice(3, 6) : '');

  function deviceRow(device: RemoteDevice): string {
    const grant = grantOf(device.id);
    const pending = !device.current && grant?.scope === 'pending';
    const waiting = pending && grant?.requested;
    const scope = grant?.scope ?? (device.trusted ? 'control' : 'pending');
    const name = escape(device.name);
    const key = escape(groupFingerprint(grant?.fingerprint || device.fingerprint));
    const actions = device.current ? '' : pending
      ? `<select data-pick="${escape(device.id)}" aria-label="Approve ${name} as">${optionList((picks[device.id] as GrantScope) || 'control')}</select><button type="button" class="primary-button" data-approve="${escape(device.id)}" aria-label="Approve ${name}">Approve</button><button type="button" class="secondary-button" data-block="${escape(device.id)}" aria-label="Block ${name}">Block</button>`
      : `<select data-scope="${escape(device.id)}" aria-label="Access for ${name}"${grant ? '' : ' disabled'}>${optionList(scope, true)}</select><button type="button" class="secondary-button" data-revoke="${escape(device.id)}" aria-label="Revoke ${name}">Revoke</button>`;
    const trustToggle = device.current ? '' : `<label class="remote-trust"><span>Trusted</span><input type="checkbox" class="settings-switch" data-trust="${escape(device.id)}" aria-label="Trusted: ${name}" ${device.trusted ? 'checked' : ''}></label>`;
    return `<div class="setting-row remote-device${waiting ? ' remote-pending' : ''}" data-device="${escape(device.id)}" data-fingerprint="${escape(grant?.fingerprint || device.fingerprint)}">
      <div class="remote-device-identity"><strong><span class="remote-dot" data-online="${device.online}" role="img" aria-label="${device.online ? 'Online' : 'Offline'}"></span>${name}${device.current ? ' <span class="remote-badge">This computer</span>' : ''}</strong><small>${waiting ? 'asks for access · ' : pending ? 'not approved · ' : ''}${escape(device.platform)} · key <code>${key}</code></small></div>
      ${device.current ? '' : `<div class="remote-device-actions">${trustToggle}${actions}</div>`}</div>`;
  }

  function body(): string {
    if (!status) {
      if (statusError) return failure('Couldn’t load remote access.', statusError);
      if (loading) return '<p class="dialog-loading" role="status">Loading…</p>';
    }
    if (!status) return failure('Couldn’t load remote access.', 'Try again.');
    if (!status.configured) return '<p class="remote-note" role="status">Remote access is not configured in this build.</p>';
    if (!status.signedIn) {
      const flow = code ? `<div class="remote-code"><p>Enter this code at <strong>${escape(code.verificationUri)}</strong>:</p><div class="remote-code-row"><output class="remote-user-code" aria-label="Device code">${escape(code.userCode)}</output><button type="button" class="secondary-button" data-copy> ${icon('copy')}Copy code</button></div><p class="remote-note" role="status">Waiting for you to approve in the browser… The code expires at ${new Date(expires).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}.</p><button type="button" class="secondary-button" data-cancel>Cancel</button></div>` : '';
      return `<div class="setting-row"><div><strong>PandaCode Connect</strong><small>Sign in with GitHub to reach your other computers. Task content is end-to-end encrypted; the relay sees only ciphertext.</small></div><button type="button" class="secondary-button" data-sign-in ${signingIn ? 'disabled' : ''}>${icon('github')}Sign in with GitHub</button></div>${flow}`;
    }
    const ordered = [...devices].sort((a, b) => Number(grantOf(b.id)?.scope === 'pending' && grantOf(b.id)?.requested) - Number(grantOf(a.id)?.scope === 'pending' && grantOf(a.id)?.requested));
    const pairable = devices.filter(device => !device.current && device.host);
    if (!pairable.some(device => device.id === pairTarget)) pairTarget = pairable[0]?.id ?? '';
    const pairingForm = `<div class="remote-pairing-form">
      <h3 class="settings-heading">Pair another computer</h3>
      <div class="setting-row"><label for="remote-pair-target">Computer to pair with</label><div class="remote-device-actions">
      <select id="remote-pair-target" aria-label="Computer to pair with">${pairable.map(device => `<option value="${escape(device.id)}"${device.id === pairTarget ? ' selected' : ''}>${escape(device.name)}</option>`).join('')}</select>
      <input id="remote-pair-input" class="remote-pair-input" type="text" inputmode="numeric" autocomplete="off" maxlength="7" aria-label="Pairing code from the other computer" placeholder="000 000" value="${escape(codeText(pairInput))}">
      <button type="button" class="secondary-button" data-pair ${pairInput.length !== 6 || pairing || !pairTarget ? 'disabled' : ''}>Pair</button></div></div>
      ${status.hostEnabled ? `<div class="setting-row"><div><strong>Allow pairing with this computer</strong><small>A six-digit, single-use code is valid for 10 minutes.</small></div><div class="remote-device-actions"><select id="remote-code-scope" aria-label="Access the pairing code gives">${optionList(codeScope)}</select><button type="button" class="secondary-button" data-code-new>Show pairing code</button></div></div>${pairCode ? `<div class="remote-code"><output aria-label="Pairing code" class="remote-user-code">${escape(codeText(pairCode.code))}</output><small>Gives ${escape(SCOPE_LABELS[pairCode.scope].toLowerCase())} once. Expires at ${new Date(pairCode.expiresAt).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}.</small><button type="button" class="secondary-button" data-code-cancel>Cancel</button></div>` : ''}` : ''}
    </div>`;
    return `<div class="setting-row"><div><strong>${escape(status.account?.login ?? 'Signed in')}</strong><small>Signed in with GitHub · ${status.connected ? 'connected' : 'connecting…'} · key <code class="remote-key">${escape(groupFingerprint(status.device.fingerprint))}</code></small></div><button type="button" class="secondary-button" data-sign-out>Sign out</button></div>
      <div class="setting-row"><div><strong>Allow remote control of this computer ${infoMark(ALLOW_TIP, 'About remote control')}</strong><small>Your other signed-in computers can see and drive tasks here.</small></div><input type="checkbox" class="settings-switch" id="remote-host" aria-label="Allow remote control of this computer" ${status.hostEnabled ? 'checked' : ''}></div>
      <div class="setting-row"><div><strong>Full access for remote tasks ${infoMark(FULL_ACCESS_TIP, 'About Full access for remote tasks')}</strong><small>Separately authorize Full access on this computer; disabling removes stored Full grants.</small></div><input type="checkbox" class="settings-switch" id="remote-full-access" aria-label="Full access for remote tasks" ${status.allowFullAccess ? 'checked' : ''}></div>
      <div class="setting-row"><div><strong>Use direct LAN/Tailscale connections</strong><small>Optional: probe private addresses only for devices trusted on this computer; relay remains the fallback.</small></div><input type="checkbox" class="settings-switch" id="remote-direct-routes" aria-label="Use direct LAN/Tailscale connections" ${status.directRoutes ? 'checked' : ''}></div>
      <h3 class="settings-heading">Devices ${infoMark(SCOPE_HELP, 'About device access')}</h3>
      ${devicesError ? failure('Couldn’t load devices.', devicesError) : grantsError ? failure('Couldn’t load device access.', grantsError) : ''}
      <div class="settings-group remote-devices">${ordered.length ? ordered.map(deviceRow).join('') : '<p class="remote-note">No devices yet.</p>'}</div>
      ${pairingForm}`;
  }

  function render(): void {
    if (disposed) return;
    const active = document.activeElement instanceof HTMLElement && root.contains(document.activeElement) ? document.activeElement : undefined;
    const keep = active?.id, caret = active instanceof HTMLInputElement ? active.selectionStart : null;
    main.innerHTML = `<div class="settings-group remote-settings">${body()}</div>${error ? `<p class="remote-error" role="alert">${escape(error)}</p>` : ''}`;
    if (keep) { const again = main.querySelector<HTMLElement>(`#${CSS.escape(keep)}`); again?.focus(); if (again instanceof HTMLInputElement && caret !== null) again.setSelectionRange(caret, caret); }
  }
  const apply = (next: RemoteStatus) => { status = next; if (next.signedIn) { code = undefined; signingIn = false; } else { pairCode = undefined; grants = []; } };
  async function loadGrants(): Promise<void> {
    grantsError = '';
    if (!status?.signedIn) { grants = []; return; }
    try { grants = await host.call<DeviceGrant[]>('remote_device_grants'); }
    catch (e) { grants = []; grantsError = message(e); }
  }
  async function loadDevices(): Promise<void> {
    devicesLoaded = false; devicesError = '';
    if (!status?.signedIn) { devices = []; devicesLoaded = true; return; }
    try { devices = await host.call<RemoteDevice[]>('remote_devices'); devicesLoaded = true; }
    catch (e) { devicesError = message(e); }
    await loadGrants();
  }
  async function load(): Promise<void> {
    loading = true; statusError = ''; devicesError = ''; error = ''; render();
    try { apply(await host.call<RemoteStatus>('remote_status')); }
    catch (e) { status = undefined; statusError = message(e); }
    if (status) await loadDevices();
    loading = false; render();
  }
  async function signIn(): Promise<void> {
    if (signingIn) return;
    error = ''; devicesError = ''; signingIn = true;
    const mine = ++attempt;
    try {
      code = await host.call<RemoteSignIn>('remote_sign_in_start'); expires = Date.now() + code.expiresIn * 1000; render();
      void host.openUrl(code.verificationUri).catch(() => undefined);
      apply(await host.call<RemoteStatus>('remote_sign_in_finish'));
      await loadDevices();
    } catch (e) {
      if (mine !== attempt) return;
      code = undefined; signingIn = false; error = message(e);
    }
    render();
  }
  function cancelSignIn(): void {
    attempt++; code = undefined; signingIn = false; error = ''; render();
    void host.call('remote_sign_in_cancel').catch(fail);
  }
  function setGrant(deviceId: string, scope: string): void {
    error = '';
    const expectedFingerprint = root.querySelector<HTMLElement>(`.remote-device[data-device="${CSS.escape(deviceId)}"]`)?.dataset.fingerprint;
    void host.call('remote_set_device_grant', { deviceId, scope, ...(scope === 'blocked' || scope === 'pending' ? {} : { expectedFingerprint }) }).then(() => loadGrants()).then(() => render(), e => { error = message(e); void loadGrants().then(render); });
  }
  function showCode(next: PairCode | undefined): void {
    pairCode = next; window.clearTimeout(expiry);
    if (next) expiry = window.setTimeout(() => { if (pairCode === next) { pairCode = undefined; render(); } }, Math.max(0, next.expiresAt - Date.now()));
    render();
  }
  root.addEventListener('click', event => {
    const target = (event.target as HTMLElement).closest<HTMLElement>('button');
    if (!target) return;
    const row = target.closest<HTMLElement>('.remote-device');
    if (target.hasAttribute('data-retry')) void load();
    else if (target.hasAttribute('data-sign-in')) void signIn();
    else if (target.hasAttribute('data-cancel')) cancelSignIn();
    else if (target.hasAttribute('data-copy') && code) void navigator.clipboard.writeText(code.userCode).then(() => host.notify('Code copied'), () => host.notify('Couldn’t copy; select the code instead'));
    else if (target.hasAttribute('data-sign-out')) void host.call<RemoteStatus>('remote_sign_out').then(next => { apply(next); devices = []; devicesLoaded = true; devicesError = ''; error = ''; render(); }, fail);
    else if (target.dataset.revoke) void host.call('remote_revoke_device', { deviceId: target.dataset.revoke }).then(() => { devices = devices.filter(d => d.id !== target.dataset.revoke); render(); }, fail);
    else if (target.dataset.approve) setGrant(target.dataset.approve, row?.querySelector<HTMLSelectElement>('select[data-pick]')?.value ?? 'control');
    else if (target.dataset.block) setGrant(target.dataset.block, 'blocked');
    else if (target.hasAttribute('data-code-new')) void host.call<PairCode>('remote_pair_code', { scope: codeScope }).then(showCode, fail);
    else if (target.hasAttribute('data-code-cancel')) { showCode(undefined); void host.call('remote_pair_code_cancel').catch(fail); }
    else if (target.hasAttribute('data-pair') && !pairing) {
      const name = devices.find(device => device.id === pairTarget)?.name ?? 'that computer';
      error = ''; pairing = true; render();
      void host.call<{ scope: GrantScope }>('remote_pair_redeem', { deviceId: pairTarget, code: pairInput }).then(result => { pairInput = ''; host.notify(`Paired with ${name}: ${(SCOPE_LABELS[result.scope] ?? result.scope).toLowerCase()}`); }, e => { error = message(e); }).finally(() => { pairing = false; render(); });
    }
  });
  root.addEventListener('change', event => {
    const control = event.target as HTMLInputElement | HTMLSelectElement;
    if (control.dataset.scope) setGrant(control.dataset.scope, control.value);
    else if (control.dataset.pick) picks[control.dataset.pick] = control.value;
    else if (control.id === 'remote-code-scope') codeScope = control.value as GrantScope;
    else if (control.id === 'remote-pair-target') pairTarget = control.value;
  });
  root.addEventListener('input', event => {
    const input = event.target as HTMLInputElement;
    if (input.id === 'remote-pair-input') {
      pairInput = input.value.replace(/\D/g, '').slice(0, 6);
      input.value = codeText(pairInput);
      input.setSelectionRange(input.value.length, input.value.length);
      const pair = main.querySelector<HTMLButtonElement>('[data-pair]');
      if (pair) pair.disabled = pairInput.length !== 6 || pairing || !pairTarget;
    }
    else if (input.dataset.trust) void host.call<RemoteDevice[]>('remote_set_trusted', { deviceId: input.dataset.trust, trusted: input.checked, ...(input.checked ? { expectedFingerprint: input.closest<HTMLElement>('.remote-device')?.dataset.fingerprint } : {}) }) .then(next => { devices = next; void loadGrants().then(render); }, e => { input.checked = !input.checked; fail(e); });
    else if (input.id === 'remote-host') void host.call<RemoteStatus>('remote_set_host_enabled', { enabled: input.checked }).then(next => { apply(next); render(); }, e => { input.checked = !input.checked; fail(e); });
    else if (input.id === 'remote-full-access') void host.call<RemoteStatus>('remote_set_full_access', { enabled: input.checked }).then(next => { apply(next); void loadGrants().then(render); }, e => { input.checked = !input.checked; fail(e); });
    else if (input.id === 'remote-direct-routes') void host.call<RemoteStatus>('remote_set_direct_routes', { enabled: input.checked }).then(next => { apply(next); render(); }, e => { input.checked = !input.checked; fail(e); });
  });
  const unlisten: Promise<() => void>[] = [
    host.listen('remote://status', next => { if (!signingIn || next.signedIn) { apply(next); render(); } }),
    host.listen('remote://presence', next => { if (Array.isArray(next)) { devices = next; devicesLoaded = true; devicesError = ''; void loadGrants().then(render); } }),
    host.listen('remote://device-grants', next => { if (Array.isArray(next)) { grants = next; grantsError = ''; render(); } }),
    host.listen('remote://pair-code-used', () => { if (pairCode) { host.notify('Pairing code used'); showCode(undefined); } }),
  ];
  root.innerHTML = '<div class="remote-main"></div><section class="remote-diagnostics-section" hidden><h3 class="settings-heading">Diagnostics</h3><div class="settings-group remote-diagnostics-body"></div></section>';
  const main = root.querySelector<HTMLElement>('.remote-main')!;
  const section = root.querySelector<HTMLElement>('.remote-diagnostics-section')!;
  void import('./remote-diagnostics').then(module => {
    const mount = (module as { mountRemoteDiagnostics?: (container: HTMLElement) => void }).mountRemoteDiagnostics;
    if (disposed || typeof mount !== 'function') return;
    mount(section.querySelector<HTMLElement>('.remote-diagnostics-body')!);
    section.hidden = false;
  }).catch(() => undefined);
  void load();
  return () => { disposed = true; window.clearTimeout(expiry); for (const u of unlisten) void u.then(fn => fn(), () => undefined); };
}
