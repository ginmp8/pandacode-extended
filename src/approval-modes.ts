import type { Mode, ProviderId } from './types';

export interface ApprovalMode { id: Mode; label: string; description: string; icon: string; disabled?: boolean; }

/** Labels describe the native policy for each harness, not a simulated approval layer. */
export function approvalModes(provider: ProviderId, current?: Mode): ApprovalMode[] {
  const reviews = ['claude', 'glm', 'mimo'].includes(provider);
  const review: ApprovalMode[] = reviews || current === 'review' ? [{
    id: 'review', label: 'Approve for me', icon: 'spark', disabled: !reviews,
    description: reviews
      ? 'Let Claude Code review tool requests using its auto mode. Requires a recent Claude Code CLI; some actions may still ask you.'
      : 'This harness has no agent-reviewed approvals for this mode. Choose another permission mode before continuing.',
  }] : [];
  // omp is a Pi fork driven through the same bridge, so it shares Pi's policy.
  const pi = provider === 'pi' || provider === 'omp' ? (provider === 'omp' ? 'omp' : 'Pi') : '';
  const standard: ApprovalMode = { id: 'ask', label: 'Standard', icon: 'check', description: provider === 'codex'
    ? 'Work in this project. Ask you when an action needs extra permission.'
    : provider === 'github-copilot'
    ? 'Use Copilot Agent mode with Allow All off. PandaCode asks before permission requests.'
    : provider === 'grok'
    ? 'Grok asks before edits and commands, whatever its own config says.'
    : provider === 'devin'
    ? 'Devin’s Code mode: edits inside this project apply; commands it does not judge safe ask you.'
    : pi
    ? `${pi} runs without its own approvals; PandaCode asks before edits, commands and other non-read tools.`
    : provider === 'deepseek'
    ? 'DeepSeek edits and runs commands inside this project freely; it asks you before going outside it.'
    : provider === 'cline'
    ? 'Cline acts; reads and searches run, PandaCode asks before edits, commands and other tools.'
    : provider === 'kilo'
    ? 'Kilo reads and searches; PandaCode asks before edits, commands and other tools.'
    : provider === 'muse'
    ? 'Muse asks before actions its rules do not allow. Its own reviewer may approve low-risk ones.'
    : 'Use the harness’s normal rules. Ask you when approval is needed.' };
  const full: ApprovalMode = { id: 'full', label: 'Full access', icon: 'unlock', description: provider === 'antigravity'
    ? 'Automatically approve permission requests. Server restrictions still apply.'
    : provider === 'codex'
    ? 'Skip approval prompts and sandbox limits. Allow file and network access.'
    : provider === 'grok'
    ? 'Always approve: Grok runs every tool without asking (its --always-approve).'
    : pi
    ? `${pi} runs every tool without asking.`
    : provider === 'deepseek'
    ? 'No sandbox: DeepSeek runs every tool without asking.'
    : provider === 'cline'
    ? 'Cline acts with its auto-approve on: every tool runs without asking.'
    : provider === 'muse'
    ? 'No approvals and no sandbox: Muse runs every tool without asking.'
    : provider === 'devin'
    ? 'Devin’s Bypass Permissions mode: every tool runs without asking, unless your organization requires approval.'
    : provider === 'github-copilot'
    ? 'Use Copilot Agent mode with Allow All on. Copilot automatically approves tool, path, and URL requests.'
    : 'Run tools without approval prompts, including outside this project.' };
  if (provider === 'github-copilot') return [standard, ...review,
    { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Use Copilot Plan mode. Read, search, and think requests are allowed; other permission requests are declined.' },
    full];
  // Devin's Code mode already accepts edits, so it has no separate Accept edits choice.
  if (provider === 'devin') return [standard, ...review,
    { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Devin’s Plan mode: it investigates and writes a plan; commands ask you, and edits wait until you approve the plan.' },
    full];
  return [standard, provider === 'muse'
    ? { id: 'auto', label: 'Accept edits', icon: 'edit', description: 'Work freely inside the sandbox. Muse asks only to leave it.' }
    : provider === 'deepseek'
    ? { id: 'auto', label: 'Accept edits', icon: 'edit', description: 'Like Standard, and edits outside this project are approved too. Other actions still ask.' }
    : ['claude', 'glm', 'mimo', 'opencode', 'grok', 'pi', 'omp', 'commandcode', 'cline', 'kilo'].includes(provider)
    ? { id: 'auto', label: 'Accept edits', icon: 'edit', description: 'Allow file edits automatically. Other actions may still ask you.' }
    : provider === 'kimi'
    ? { id: 'auto', label: 'Ask when needed', icon: 'spark', description: 'Allow routine tools automatically. Ask you about sensitive actions.' }
    : provider === 'codex' ? { id: 'auto', label: 'Approve for me', icon: 'spark', description: 'Let Codex review approval requests. Keep the workspace sandbox.' }
    : { id: 'auto', label: 'Accept edits', icon: 'edit', description: 'Automatically approve edit requests offered by the server.' }, ...review,
  provider === 'codex'
    ? { id: 'plan', label: 'Read only', icon: 'plan', description: 'Inspect the project. Block writes and permission escalation.' }
    : provider === 'grok'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Read-only: Grok investigates and proposes a plan; edits and commands are declined.' }
    : pi
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: `Read-only: ${pi} may use read-only tools; everything else is declined.` }
    : provider === 'deepseek'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Read-only sandbox: DeepSeek cannot write files and proposes a plan; requests to escalate are declined.' }
    : provider === 'cline'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Cline’s Plan mode: it explores and proposes a plan; file edits are declined, commands still ask.' }
    : provider === 'kilo'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Read-only: Kilo investigates and proposes a plan; edits, commands, and other state-changing permissions are declined.' }
    : provider === 'muse'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Read-only: Muse cannot write files or run shell commands, and proposes a plan.' }
    : provider === 'commandcode'
    ? { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Read-only: Command Code researches and plans; edits and commands are declined.' }
    : { id: 'plan', label: 'Plan mode', icon: 'plan', description: 'Explore and plan changes before implementation.' }, full];
}

export function approvalMode(provider: ProviderId, mode: Mode): ApprovalMode {
  const modes = approvalModes(provider, mode);
  return modes.find(item => item.id === mode) ?? modes[0];
}
