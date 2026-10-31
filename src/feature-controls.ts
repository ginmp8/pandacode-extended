import { call } from './bridge';

type Flag = 'agentUpdateChecks' | 'cloudCredentialTransfer' | 'agentCloudManagement' | 'externalMcpServers' | 'botsPaused';
type Flags = Record<Flag, boolean>;
type DirectRoutes = { directRoutes: boolean };

const CONTROLS: { id: Flag; title: string; detail: string }[] = [
  { id: 'agentUpdateChecks', title: 'Check agent updates automatically', detail: 'Off: check installed agents only when you click Check updates. Installation remains manual.' },
  { id: 'cloudCredentialTransfer', title: 'Transfer local credentials to cloud agents', detail: 'Off: prevent new cloud agent starts from copying local login files, API keys and GitHub CLI tokens.' },
  { id: 'agentCloudManagement', title: 'Allow agent VM and cloud management', detail: 'Off: agents may inspect existing machines but cannot control VMs or use deployment tools.' },
  { id: 'externalMcpServers', title: 'Allow external MCP servers in Claude and Codex', detail: 'Off: isolate MCP configuration in PandaCode-launched Claude and Codex runs. Other harnesses keep native settings.' },
  { id: 'botsPaused', title: 'Pause all bots and external channels', detail: 'Stop bot tasks and hold check-ins, schedules and external messaging without changing individual bot settings.' },
];

export function mountFeatureControls(host: HTMLElement): () => void {
  let disposed = false;
  let flags: Flags | null = null;
  let direct: boolean | null = null;
  let pending = false;
  let error = '';

  const render = (focus = ''): void => {
    if (disposed) return;
    host.replaceChildren();
    const group = document.createElement('div');
    group.className = 'settings-group';
    const items = [...CONTROLS, { id: 'directRoutes' as const, title: 'Use direct LAN / Tailscale routes', detail: 'When Connect is enabled, allow authenticated direct connections. Off uses the relay only.' }];
    for (const item of items) {
      const row = document.createElement('div');
      row.className = 'setting-row';
      const summary = document.createElement('div');
      const title = document.createElement('strong');
      title.textContent = item.title;
      const description = document.createElement('small');
      description.textContent = item.detail;
      summary.append(title, description);
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.className = 'settings-switch';
      input.setAttribute('aria-label', item.title);
      if (item.id === 'directRoutes') {
        input.dataset.directRoutes = '';
        input.checked = direct ?? false;
        input.disabled = direct === null || pending;
      } else {
        input.dataset.feature = item.id;
        input.checked = flags?.[item.id] ?? false;
        input.disabled = flags === null || pending;
      }
      row.append(summary, input);
      group.append(row);
    }
    if (error) {
      const notice = document.createElement('p');
      notice.className = 'settings-embed-error';
      notice.setAttribute('role', 'alert');
      notice.textContent = error;
      group.append(notice);
    }
    host.append(group);
    if (focus) host.querySelector<HTMLInputElement>('input[data-feature="' + focus + '"],input[data-direct-routes]')?.focus({ preventScroll: true });
  };

  const reload = async (): Promise<void> => {
    const [features, routes] = await Promise.allSettled([
      call<Flags>('feature_controls_get'),
      call<DirectRoutes>('remote_direct_routes_status'),
    ]);
    if (disposed) return;
    if (features.status === 'fulfilled') flags = features.value;
    if (routes.status === 'fulfilled') direct = routes.value.directRoutes;
    error = features.status === 'rejected' || routes.status === 'rejected' ? 'Some feature controls could not be loaded.' : '';
    render();
  };

  const change = async (event: Event): Promise<void> => {
    const input = event.target instanceof HTMLInputElement ? event.target : null;
    if (!input || pending) return;
    const id = input.dataset.feature as Flag | undefined;
    const route = input.hasAttribute('data-direct-routes');
    if (!id && !route) return;
    pending = true;
    error = '';
    const enabled = input.checked;
    render();
    try {
      if (route) {
        direct = (await call<DirectRoutes>('remote_set_direct_routes', { enabled })).directRoutes;
      } else if (id) {
        flags = await call<Flags>('feature_controls_set', { key: id, enabled });
      }
    } catch (failure) {
      error = failure instanceof Error ? failure.message : String(failure);
    } finally {
      pending = false;
      render(id || 'directRoutes');
    }
  };

  host.addEventListener('change', change);
  render();
  void reload();
  return () => { disposed = true; host.removeEventListener('change', change); host.replaceChildren(); };
}
