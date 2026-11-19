/** Read-only capability inspection UI. Native configuration owns the truth and permissions. */
import { escape } from './dom';
import type { BotDetail } from './bots';
import type { PanelContext } from './bots-panels';

interface Fact { value: string | null; status: string; configured?: boolean; }
interface NamedStatus { id?: string; name?: string; status: string; }
interface Profile {
  botId: string;
  name: string;
  role: Fact;
  summary: Fact;
  instructions: Fact;
  provider: Fact;
  model: Fact;
  permissionMode: Fact;
  projectScope: { projectId: string; status: string };
  availability: { status: string; paused: boolean };
  skills: NamedStatus[];
  mcp: NamedStatus[];
  tools: NamedStatus[];
  visibility: string;
  observedAt: number;
  configurationUpdatedAt: number;
}
interface Reply {
  topic: string;
  fields: Record<string, unknown>;
  status: string;
  executed: boolean;
}
let current: {
  botId: string; revision: number; profile: Profile | null;
  loading: boolean; error: string;
} = { botId: '', revision: -1, profile: null, loading: false, error: '' };
let reply: { botId: string; value: Reply | null } = { botId: '', value: null };

function load(context: PanelContext, botId: string, revision: number): void {
  current = { botId, revision, profile: null, loading: true, error: '' };
  void context.call<Profile>('bot_capability_profile', { botId }).then(profile => {
    if (current.botId !== botId || current.revision !== revision) return;
    current = { botId, revision, profile, loading: false, error: '' };
    context.rerender();
  }).catch(cause => {
    if (current.botId !== botId || current.revision !== revision) return;
    current = { botId, revision, profile: null, loading: false,
      error: cause instanceof Error ? cause.message : String(cause) };
    context.rerender();
  });
}

function item(label: string, value: string, status = ''): string {
  return '<li><span class="bots-item-main"><b>' + escape(label) + '</b>' +
    '<small>' + escape(value) + (status ? ' · ' + escape(status) : '') +
    '</small></span></li>';
}

export function renderCapabilityPanel(detail: BotDetail, context: PanelContext): string {
  const bot = detail.bot;
  if (current.botId !== bot.id || current.revision !== bot.updatedAt) {
    reply = { botId: '', value: null };
    load(context, bot.id, bot.updatedAt);
  }
  const p = current.profile;
  if (!p) {
    return '<section aria-label="Capability profile"><p role="status">' +
      (current.loading ? 'Reading current native bot configuration…' : 'Capability profile unavailable: ' + escape(current.error)) +
      '</p><button type="button" class="secondary-button" data-capabilities-refresh>Retry</button></section>';
  }
  const published = p.summary.value || '';
  const answer = reply.botId === bot.id ? reply.value : null;
  return '<section aria-label="Capability profile" class="bots-capability-profile">' +
    '<p>Read-only native configuration. Not an execution grant or Council verdict. ' +
    'Private instructions, credentials, memories, and chat history are restricted.</p>' +
    '<p><button type="button" class="secondary-button" data-capabilities-refresh>Refresh profile</button>' +
    ' <small>Read at ' + escape(new Date(p.observedAt).toLocaleString()) + '</small></p>' +
    '<ul class="bots-items">' +
    item('Declared role', p.role.value || 'Not declared', p.role.status) +
    item('Published summary', published || 'Not published', p.summary.status) +
    item('Private instructions', p.instructions.configured ? 'Configured' : 'Not configured', 'restricted') +
    item('Provider and model', (p.provider.value || 'Unknown') + ' / ' + (p.model.value || 'Unknown'), 'current configuration') +
    item('Permission mode', p.permissionMode.value || 'Unknown', 'runtime authorization still required') +
    item('Project scope', p.projectScope.projectId, p.projectScope.status) +
    item('Availability', p.availability.status, p.availability.paused ? 'Paused' : 'Live capacity not verified') +
    item('Assigned skills', p.skills.map(s => s.id).filter(Boolean).join(', ') || 'None', 'invocation not verified') +
    item('MCP references', p.mcp.map(s => s.name).filter(Boolean).join(', ') || 'None', 'harness not verified') +
    item('Native bot tools', p.tools.map(s => s.name).filter(Boolean).join(', ') || 'None', 'runtime-scoped') +
    '</ul>' +
    '<form class="bots-capability-settings"><h4>Discovery visibility</h4>' +
    '<label class="bots-field"><span>Who can discover this bot</span><select name="visibility">' +
    '<option value="local_bots"' + (p.visibility === 'local_bots' ? ' selected' : '') + '>Local bots may inspect</option>' +
    '<option value="owner_only"' + (p.visibility === 'owner_only' ? ' selected' : '') + '>Only you may inspect</option>' +
    '</select></label>' +
    '<label class="bots-field"><span>Public capability summary (optional)</span>' +
    '<textarea name="summary" maxlength="500" rows="2" placeholder="Describe work this bot can help with (do not include secrets)">' +
    escape(published) + '</textarea></label>' +
    '<button type="submit" class="secondary-button">Save discovery preferences</button></form>' +
    '<form class="bots-capability-question"><h4>Ask about capabilities</h4>' +
    '<label class="bots-field"><span>Question</span><input name="question" maxlength="400" required ' +
    'placeholder="What skills are assigned?"></label>' +
    '<button type="submit" class="secondary-button">Ask without executing</button></form>' +
    (answer ? '<section aria-label="Capability answer" role="region">' +
      '<h4>' + escape(answer.topic) + '</h4><p>' + escape(answer.status) + ' · no task executed</p>' +
      '<pre>' + escape(JSON.stringify(answer.fields, null, 2)) + '</pre></section>' : '') +
    '</section>';
}

export function capabilityClick(target: HTMLElement, context: PanelContext): boolean {
  if (!target.hasAttribute('data-capabilities-refresh')) return false;
  const bot = context.detail()?.bot;
  if (bot) {
    reply = { botId: '', value: null };
    load(context, bot.id, bot.updatedAt);
    context.rerender();
  }
  return true;
}

export function capabilitySubmit(form: HTMLFormElement, context: PanelContext): boolean {
  if (!form.matches('.bots-capability-settings, .bots-capability-question')) return false;
  const bot = context.detail()?.bot;
  if (!bot) return true;
  const data = new FormData(form);
  const text = (name: string) => String(data.get(name) || '').trim();
  if (form.matches('.bots-capability-settings')) {
    void context.work(async () => {
      await context.call('bot_capability_preferences_set', {
        botId: bot.id, visibility: text('visibility'), summary: text('summary'),
      });
      current = { botId: '', revision: -1, profile: null, loading: false, error: '' };
      await context.reload();
      context.notify('Capability preferences saved');
    });
  } else {
    const question = text('question');
    if (!question) return true;
    void context.work(async () => {
      reply = { botId: bot.id, value: await context.call<Reply>(
        'bot_capability_question', { botId: bot.id, question }) };
      context.rerender();
    });
  }
  return true;
}
