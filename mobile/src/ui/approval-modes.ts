export interface PermissionMode { id: string; label: string; available: boolean }

const standardModes = [
  { id: 'ask', label: 'Standard' },
  { id: 'auto', label: 'Auto-approve edits' },
  { id: 'plan', label: 'Plan only' },
  { id: 'full', label: 'Full access' },
];

/** Preserve the selected mode across harness changes; unsupported review never falls back. */
export function supportsPermissionMode(provider: string, mode: string): boolean {
  return mode === 'review' ? ['claude', 'glm', 'mimo'].includes(provider) : standardModes.some(choice => choice.id === mode);
}

export function permissionModes(provider: string, current?: string): PermissionMode[] {
  const choices: PermissionMode[] = standardModes.map(mode => ({ ...mode, available: true }));
  const reviews = supportsPermissionMode(provider, 'review');
  if (reviews || current === 'review') choices.splice(2, 0, {
    id: 'review', label: reviews ? 'Approve for me' : 'Approve for me (unavailable)', available: reviews,
  });
  return choices;
}

export function permissionNotice(provider: string, mode: string): string | undefined {
  if (mode !== 'review') return undefined;
  return supportsPermissionMode(provider, mode)
    ? 'Claude Code reviews tool requests in its auto mode. Enable the Full access grant locally on the computer to use this mode remotely.'
    : 'This harness cannot use Approve for me. Choose another permission mode before continuing.';
}
