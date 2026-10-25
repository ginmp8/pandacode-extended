/* One list of machines for the VM panel: local VMs from Tart and cloud sandboxes from
 * connected providers, with what each can do. Pure, so the panel only renders. */
import type { Vm, VmStatus } from './vm-panel';

export type CloudCapabilities = { start: boolean; stop: boolean; delete: boolean; exec: boolean; lease: boolean };
export type CloudMachine = { id: string; provider: string; name: string; state: string; os: string; desktop: boolean; detail: string; capabilities?: CloudCapabilities };
export const SANDBOX_PROVIDERS = { gcloud: 'Google Cloud VM', e2b: 'E2B', modal: 'Modal', vercel: 'Vercel Sandbox', codesandbox: 'CodeSandbox', runloop: 'Runloop', blaxel: 'Blaxel', tensorlake: 'Tensorlake', freestyle: 'Freestyle', boat: 'boat', instabox: 'InstaBox' } as const;
/** Compatibility for older native snapshots; new native machines declare these directly. */
export function cloudCapabilities(provider: string): CloudCapabilities {
  const known = ['daytona', 'cloudflare', 'aws', 'agentcore', ...Object.keys(SANDBOX_PROVIDERS)].includes(provider);
  return { start: known && !['modal', 'vercel', 'agentcore'].includes(provider), stop: known && !['modal', 'vercel', 'blaxel'].includes(provider), delete: known, exec: known, lease: ['e2b', 'modal', 'vercel', 'codesandbox', 'runloop', 'freestyle', 'boat', 'instabox'].includes(provider) };
}
export type { CloudConnection } from './cloud-settings';
import type { CloudConnection } from './cloud-settings';
export type Machine = {
  /** A local VM's name, or a cloud ID such as `daytona:…`. */
  key: string;
  name: string;
  os: string;
  cloud: boolean;
  running: boolean;
  /** Its screen can be shown and used here now. */
  live: boolean;
  desktop: boolean;
  detail: string;
  /** Local VMs running outside PandaCode must be restarted here to be seen. */
  elsewhere: boolean;
  canStart: boolean;
  canStop: boolean;
  canDelete: boolean;
  canLease: boolean;
  leaseMinutes: number;
  local?: Vm;
};
export type CreateOption = { id: string; label: string; description: string; cloud: boolean };

const osName = (os: string) => os === 'darwin' ? 'macOS' : os === 'windows' ? 'Windows' : 'Linux';
const memory = (mb: number) => mb >= 1024 ? `${Math.round(mb / 1024)} GB` : `${mb} MB`;

export function machines(status: VmStatus | undefined, cloud: CloudMachine[]): Machine[] {
  const local = (status?.vms ?? []).map((vm): Machine => ({
    key: vm.name, name: vm.name, os: vm.os, cloud: false, running: vm.running, live: vm.running && vm.attached, desktop: true,
    detail: `${osName(vm.os)} · ${memory(vm.memoryMb)} · ${vm.running ? (vm.attached ? 'Running' : 'Running elsewhere') : 'Stopped'}`,
    elsewhere: vm.running && !vm.attached, canStart: true, canStop: true, canDelete: true, canLease: false, leaseMinutes: 0, local: vm,
  }));
  const remote = cloud.map((machine): Machine => ({
    key: machine.id, name: machine.name, os: machine.os, cloud: true, running: machine.state === 'running', live: machine.state === 'running' && machine.desktop,
    desktop: machine.desktop, detail: `${machine.detail} · ${machine.state[0].toUpperCase()}${machine.state.slice(1)}`, elsewhere: false,
    canStart: (machine.capabilities ?? cloudCapabilities(machine.provider)).start,
    canStop: (machine.capabilities ?? cloudCapabilities(machine.provider)).stop,
    canDelete: (machine.capabilities ?? cloudCapabilities(machine.provider)).delete,
    canLease: (machine.capabilities ?? cloudCapabilities(machine.provider)).lease,
    leaseMinutes: machine.provider === 'runloop' ? 360 : 30,
  }));
  return [...local, ...remote];
}

/** Things the New menu can make: local images once Tart is set up, and cloud kinds per connected provider. */
export function createOptions(status: VmStatus | undefined, connections: CloudConnection[]): CreateOption[] {
  const local = status?.installed ? status.images.map(image => ({ id: `image:${image.id}`, label: image.name, description: `Local · ${image.downloadGb} GB download · ${memory(image.memoryMb)}`, cloud: false })) : [];
  const on = (provider: CloudConnection['provider']) => connections.some(c => c.provider === provider && c.connected);
  const cloud: CreateOption[] = [
    ...(on('daytona') ? [{ id: 'cloud:daytona', label: 'Daytona desktop', description: 'Linux with a screen · billed by Daytona', cloud: true }] : []),
    ...(connections.some(c => c.provider === 'cloudflare' && c.connected && c.sandboxes) ? [{ id: 'cloud:cloudflare', label: 'Cloudflare container', description: 'Linux, commands only · your Cloudflare account', cloud: true }] : []),
    ...(on('aws') ? [
      { id: 'cloud:aws-linux', label: 'AWS Linux desktop', description: 'Ubuntu on EC2 · billed while running', cloud: true },
      { id: 'cloud:aws-windows', label: 'AWS Windows desktop', description: 'Windows Server on EC2 · about 10 min to set up', cloud: true },
      { id: 'cloud:agentcore', label: 'AWS AgentCore session', description: 'Commands only · ends after an hour', cloud: true },
    ] : []),
    ...Object.entries(SANDBOX_PROVIDERS).filter(([provider]) => on(provider as CloudConnection['provider']) && (provider !== 'vercel' || connections.some(c => c.provider === 'vercel' && c.sandboxes))).map(([provider, label]) => ({
      id: `cloud:${provider}`, label, description: `Commands only · ${provider === 'modal' || provider === 'vercel' ? 'delete only; no resume' : provider === 'blaxel' ? 'automatic standby' : 'your cloud account'} · billed by provider`, cloud: true,
    })),
  ];
  return [...local, ...cloud];
}

export function cloudName(): string {
  return `pandacode-${Math.random().toString(36).slice(2, 8)}`;
}
