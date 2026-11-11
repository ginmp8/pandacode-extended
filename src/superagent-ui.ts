import './superagent.css';
import { escape } from './dom';

// This interface only invokes native policy/authorization decisions. It cannot
// grant tools, infer work, start tasks on import or bypass Council review.
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
interface Bot { id: string; name: string; paused: boolean }
interface Config {
  id: string; name: string; coordinatorBotId: string; workerBotId: string;
  enabled: boolean; policyRevision: number;
}
interface Ticket { id: string; objective: string; state: string; revision: number; evidence: string | null }
interface Preview { wouldCreate: string[]; unchanged: string[]; importedPresetsDisabled: boolean }

export function mountSuperagent(host: HTMLElement, call: Call): void {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let selected = '', busy = false, loading = true, error = '', importText = '';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  const safe = (value: unknown) => escape(String(value ?? ''));
  const current = () => presets.find(item => item.id === selected);
  const botName = (id: string) => bots.find(bot => bot.id === id)?.name ?? 'Missing bot';
  function render(): void {
    if (!host.isConnected) return;
    const list = presets.map(item =>
      '<li class="sa-item"><div><strong>' + safe(item.name) + '</strong><small>' +
      safe(botName(item.coordinatorBotId)) + ' / ' + safe(botName(item.workerBotId)) +
      '</small></div><span>' + (item.enabled ? 'Manual' : 'Off') + '</span>' +
      '<button type="button" data-sa-select="' + safe(item.id) + '" aria-current="' +
      (item.id === selected) + '">Details</button></li>').join('');
    const choices = bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const chosen = current();
    const active = chosen ? '<section class="sa-pane" aria-label="Selected orchestration"><header><h3>' +
      safe(chosen.name) + '</h3><div class="sa-actions"><button type="button" data-sa-enable="' +
      safe(chosen.id) + '">' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused</button></div></header>' +
      '<p>Policy revision ' + chosen.policyRevision + ' · Native read-only permission until separately authorized.</p>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required></textarea></label>' +
      '<label>Dispatch<select name="mode"><option value="coordinated">Coordinator, then worker</option>' +
      '<option value="direct">Worker directly</option></select></label>' +
      '<button class="primary-button" type="submit"' + (chosen.enabled ? '' : ' disabled') +
      '>Dispatch manually</button></form>' +
      '<div class="sa-title"><h4>Tickets</h4><button type="button" data-sa-refresh>Refresh</button></div>' +
      '<ul class="sa-tickets">' + (tickets.map(ticket =>
        '<li><div><strong>' + safe(ticket.objective) + '</strong><small>' + safe(ticket.state) + '</small></div>' +
        (ticket.state === 'waiting_approval'
          ? '<button type="button" data-sa-accept="' + safe(ticket.id) + '">Accept evidence</button>' : '') +
        '</li>').join('') || '<li class="sa-empty">No tickets yet.</li>') + '</ul></section>' : '';
    host.innerHTML = '<div class="sa-ui">' +
      '<p class="sa-intro">Native SuperAgent uses existing bots. Imported presets stay Off, and all task execution is manual.</p>' +
      (loading ? '<p role="status">Loading orchestrations…</p>' : '') +
      (error ? '<p role="alert" class="sa-error">' + safe(error) + '</p>' : '') +
      (confirm ? '<section role="alertdialog" aria-label="Confirm orchestration action" class="sa-confirm"><p>' +
        safe(confirm.message) + '</p><div class="sa-actions"><button type="button" data-sa-cancel>Cancel</button>' +
        '<button class="primary-button" type="button" data-sa-confirm>Confirm</button></div></section>' : '') +
      '<section class="sa-pane"><h3>Orchestrations</h3><ul class="sa-list">' +
      (list || '<li class="sa-empty">No orchestrations created.</li>') + '</ul></section>' +
      '<section class="sa-pane"><h3>New manual preset</h3><form data-sa-form="create">' +
      '<label>Name<input name="name" maxlength="120" placeholder="Software Development" required></label>' +
      '<div class="sa-grid"><label>Coordinator<select name="coordinatorBotId" required>' +
      '<option value="">Choose bot</option>' + choices + '</select></label>' +
      '<label>Worker<select name="workerBotId" required><option value="">Choose bot</option>' +
      choices + '</select></label></div><button class="primary-button" type="submit"' +
      (bots.length < 2 ? ' disabled' : '') + '>Create disabled preset</button></form></section>' +
      active +
      '<section class="sa-pane"><h3>Portable configuration</h3>' +
      '<p>Configuration only: no credentials, memories, conversations, approvals or live tasks.</p>' +
      '<label>Companion manifest JSON<textarea data-sa-manifest rows="5" spellcheck="false">' +
      safe(importText) + '</textarea></label>' +
      '<div class="sa-actions"><button type="button" data-sa-export>Export</button>' +
      '<button type="button" data-sa-preview>Preview import</button></div>' +
      (preview ? '<p role="status">' + preview.wouldCreate.length + ' new; ' +
        preview.unchanged.length + ' unchanged. Imported presets will be disabled.</p>' +
        '<button class="primary-button" type="button" data-sa-import>Confirm import</button>' : '') +
      '</section></div>';
    host.setAttribute('aria-busy', String(busy));
  }
  async function reload(): Promise<void> {
    loading = true; render();
    try {
      const [known, configs] = await Promise.all([call<Bot[]>('bots_list'), call<Config[]>('superagent_list')]);
      bots = known; presets = configs;
      if (!presets.some(config => config.id === selected)) selected = presets[0]?.id ?? '';
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      error = '';
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { loading = false; render(); }
  }
  async function work(action: () => Promise<void>): Promise<void> {
    if (busy) return;
    busy = true; error = '';
    try { await action(); } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { busy = false; render(); }
  }
  function ask(message: string, action: () => Promise<void>): void {
    confirm = { message, action }; render();
    host.querySelector<HTMLButtonElement>('[data-sa-cancel]')?.focus();
  }
  host.addEventListener('input', event => {
    const target = event.target;
    if (target instanceof HTMLTextAreaElement && target.hasAttribute('data-sa-manifest')) {
      importText = target.value; preview = null; inspected = null;
    }
  });
  host.addEventListener('click', event => {
    const target = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('button') : null;
    if (!target || !host.contains(target) || target.disabled) return;
    if (target.hasAttribute('data-sa-cancel')) { confirm = null; render(); return; }
    if (target.hasAttribute('data-sa-confirm')) {
      const action = confirm?.action; confirm = null; if (action) void work(action);
      return;
    }
    if (target.dataset.saSelect) {
      selected = target.dataset.saSelect; void work(async () => {
        tickets = await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 });
      });
      return;
    }
    if (target.dataset.saEnable) {
      const c = presets.find(item => item.id === target.dataset.saEnable);
      if (c) ask((c.enabled ? 'Disable ' : 'Enable manual mode for ') + c.name + '?', async () => {
        await call('superagent_set_enabled',{ orchestrationId:c.id, expectedRevision:c.policyRevision, enabled:!c.enabled });
        await reload();
      });
      return;
    }
    if (target.dataset.saDelete) {
      const c = presets.find(item => item.id === target.dataset.saDelete);
      if (c) ask('Delete unused preset ' + c.name + '? Ticket history cannot be deleted.', async () => {
        await call('superagent_delete',{ orchestrationId:c.id, expectedRevision:c.policyRevision });
        selected = ''; await reload();
      });
      return;
    }
    if (target.hasAttribute('data-sa-refresh')) { void work(reload); return; }
    if (target.hasAttribute('data-sa-export')) {
      void work(async () => {
        importText = JSON.stringify(await call<unknown>('superagent_manifest_export'),null,2);
        preview = null; inspected = null;
      }); return;
    }
    if (target.hasAttribute('data-sa-preview')) {
      void work(async () => {
        inspected = JSON.parse(importText);
        preview = await call<Preview>('superagent_manifest_preview',{ document:inspected });
      }); return;
    }
    if (target.hasAttribute('data-sa-import') && inspected) {
      const document = inspected;
      ask('Import these configurations, disabled, without changing existing bots?', async () => {
        await call('superagent_manifest_import',{ document, confirmed:true });
        preview = null; inspected = null; await reload();
      }); return;
    }
    if (target.dataset.saAccept) {
      const t = tickets.find(item => item.id === target.dataset.saAccept);
      if (t?.state === 'waiting_approval') ask('Accept the recorded worker evidence at ticket revision ' + t.revision + '?', async () => {
        await call('superagent_accept',{ ticketId:t.id, expectedRevision:t.revision }); await reload();
      });
    }
  });
  host.addEventListener('submit', event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !host.contains(form)) return;
    event.preventDefault();
    const data = new FormData(form);
    if (form.dataset.saForm === 'create') {
      const input = { name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? '') };
      void work(async () => {
        if (!input.name || !input.coordinatorBotId || !input.workerBotId ||
          input.coordinatorBotId === input.workerBotId) throw new Error('Choose two different existing bots.');
        const created = await call<Config>('superagent_create',{ input });
        selected = created.id; await reload();
      });
    }
    if (form.dataset.saForm === 'run') {
      const selectedConfig = current(), objective = String(data.get('objective') ?? '').trim();
      const command = data.get('mode') === 'direct' ? 'superagent_submit' : 'superagent_submit_coordinated';
      if (!selectedConfig?.enabled || !objective) return;
      const key = taskKey;
      void work(async () => {
        await call(command,{ orchestrationId:selectedConfig.id, objective, requestKey:key });
        taskKey = crypto.randomUUID(); await reload();
      });
    }
  });
  render();
  void reload();
}
