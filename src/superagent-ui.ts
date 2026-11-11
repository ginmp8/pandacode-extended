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
interface Mandate {
  actor: string; authorizationId: string; policyRevision: number; projectId: string;
  expiresAt: number; revoked: boolean; allowedTools: string[]; allowedSideEffects: boolean;
}
interface Assignment { id: string; botId: string; sessionId: string; state: string; attempt: number }
interface Activity { id: number; kind: string; detail: string; at: number }
interface TicketDetails { mandate: Mandate; assignments: Assignment[]; activity: Activity[] }

export function mountSuperagent(host: HTMLElement, call: Call): void {
  let bots: Bot[] = [], presets: Config[] = [], tickets: Ticket[] = [];
  let selected = '', busy = false, loading = true, error = '', importText = '';
  let preview: Preview | null = null, inspected: unknown = null;
  let confirm: { message: string; action: () => Promise<void> } | null = null;
  let taskKey = crypto.randomUUID();
  let rejectingTicket = '', inspectedTicket = '';
  let ticketDetails: TicketDetails | null = null;
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
    const optionsFor = (botId: string) => bots.map(bot => '<option value="' + safe(bot.id) + '"' +
      (bot.paused ? ' disabled' : '') + (bot.id === botId ? ' selected' : '') + '>' + safe(bot.name) +
      (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const choices = optionsFor('');
    const chosen = current();
    const inspected = tickets.find(ticket => ticket.id === inspectedTicket);
    const details = inspected && ticketDetails
      ? '<section class="sa-pane" role="region" aria-label="Ticket evidence">' +
        '<h4>Ticket evidence</h4><p>Read-only native snapshot. Model output is not task approval.</p>' +
        '<dl><dt>Ticket ID</dt><dd><code>' + safe(inspected.id) + '</code></dd>' +
        '<dt>State / revision</dt><dd>' + safe(inspected.state) + ' / ' + inspected.revision + '</dd>' +
        '<dt>Actor</dt><dd>' + safe(ticketDetails.mandate.actor) + '</dd>' +
        '<dt>Authorization</dt><dd><code>' + safe(ticketDetails.mandate.authorizationId) + '</code></dd>' +
        '<dt>Project / policy</dt><dd>' + safe(ticketDetails.mandate.projectId) + ' / ' +
        ticketDetails.mandate.policyRevision + '</dd>' +
        '<dt>Mandate</dt><dd>' + (ticketDetails.mandate.revoked ? 'Revoked' : 'Issued') +
        ' · ' + (ticketDetails.mandate.allowedSideEffects ? 'Scoped side effects' : 'No side effects allowed') +
        ' · Tools: ' + safe(ticketDetails.mandate.allowedTools.join(', ') || 'none') + '</dd>' +
        '<dt>Expires</dt><dd>' + safe(new Date(ticketDetails.mandate.expiresAt).toLocaleString()) + '</dd></dl>' +
        '<h5>Assignment attempts</h5><ul>' +
        (ticketDetails.assignments.map(item => '<li>Attempt ' + item.attempt + ' · ' +
          safe(item.state) + ' · Bot ' + safe(botName(item.botId)) + ' · Session <code>' +
          safe(item.sessionId) + '</code></li>').join('') || '<li>No worker assigned.</li>') +
        '</ul><h5>Native activity</h5><ol>' +
        (ticketDetails.activity.map(entry => '<li>' + safe(entry.kind) + ' · ' +
          safe(entry.detail) + '</li>').join('') || '<li>No activity recorded.</li>') +
        '</ol><button type="button" data-sa-close-details>Close evidence</button></section>'
      : '';
    const active = chosen ? '<section class="sa-pane" aria-label="Selected orchestration"><header><h3>' +
      safe(chosen.name) + '</h3><div class="sa-actions"><button type="button" data-sa-enable="' +
      safe(chosen.id) + '">' + (chosen.enabled ? 'Disable' : 'Enable manual') +
      '</button><button type="button" data-sa-delete="' + safe(chosen.id) + '"' +
      (chosen.enabled ? ' disabled' : '') + '>Delete unused</button></div></header>' +
      '<form data-sa-form="edit"><label>Preset title<input name="name" required maxlength="120" value="' + safe(chosen.name) + '"></label>' +
      '<div class="sa-grid"><label>Edit coordinator<select name="coordinatorBotId" required>' +
      optionsFor(chosen.coordinatorBotId) + '</select></label>' +
      '<label>Edit worker<select name="workerBotId" required>' + optionsFor(chosen.workerBotId) +
      '</select></label></div><button class="primary-button" type="submit"' +
      (chosen.enabled ? ' disabled' : '') + '>Save preset</button></form>' +
      '<p>Policy revision ' + chosen.policyRevision + ' · Native read-only permission until separately authorized.</p>' +
      '<form data-sa-form="run"><label>Task objective<textarea name="objective" rows="3" maxlength="16000" required></textarea></label>' +
      '<label>Dispatch<select name="mode"><option value="coordinated">Coordinator, then worker</option>' +
      '<option value="direct">Worker directly</option></select></label>' +
      '<button class="primary-button" type="submit"' + (chosen.enabled ? '' : ' disabled') +
      '>Dispatch manually</button></form>' +
      '<div class="sa-title"><h4>Tickets</h4><button type="button" data-sa-refresh>Refresh</button></div>' +
      '<ul class="sa-tickets">' + (tickets.map(ticket =>
        '<li><div><strong>' + safe(ticket.objective) + '</strong><small>' + safe(ticket.state) +
        ' · Revision ' + ticket.revision + '</small></div>' +
        '<button type="button" data-sa-inspect-ticket="' + safe(ticket.id) +
        '" aria-expanded="' + (ticket.id === inspectedTicket) + '">View evidence</button>' +
        (ticket.state === 'waiting_approval'
          ? '<button type="button" data-sa-accept="' + safe(ticket.id) + '">Accept evidence</button>' +
            '<button type="button" data-sa-reject="' + safe(ticket.id) + '">Reject evidence</button>' : '') +
        (ticket.state === 'queued' ? '<button type="button" data-sa-dispatch="' + safe(ticket.id) + '">Dispatch worker</button>' : '') +
        (ticket.state === 'blocked' ? '<button type="button" data-sa-retry="' + safe(ticket.id) + '">Retry manually</button>' : '') +
        (!['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)
          ? '<button type="button" data-sa-stop="' + safe(ticket.id) + '">Stop</button>' : '') +
        '</li>').join('') || '<li class="sa-empty">No tickets yet.</li>') + '</ul>' +
        (rejectingTicket ? '<form data-sa-form="reject"><label>Rejection reason<textarea name="reason" maxlength="500" required></textarea></label>' +
          '<div class="sa-actions"><button type="submit" class="primary-button">Submit rejection</button>' +
          '<button type="button" data-sa-reject-cancel>Cancel</button></div></form>' : '') +
        details + '</section>' : '';
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
  async function refreshTicketDetails(): Promise<void> {
    const id = inspectedTicket;
    if (!id || !tickets.some(ticket => ticket.id === id)) {
      inspectedTicket = ''; ticketDetails = null; return;
    }
    // Use existing native read endpoints, not model-generated summaries. A
    // revision change refreshes the snapshot before a later human decision.
    const [mandate, assignments, activity] = await Promise.all([
      call<Mandate>('superagent_mandate', { ticketId:id }),
      call<Assignment[]>('superagent_assignments', { ticketId:id, limit:50 }),
      call<Activity[]>('superagent_activity', { ticketId:id, limit:50 }),
    ]);
    if (inspectedTicket === id) ticketDetails = { mandate, assignments, activity };
  }
  async function reload(): Promise<void> {
    loading = true;
    // Keep updated controls hidden until the mutating operation has settled.
    // Exposing them mid-reload would allow silently discarded submissions.
    if (!busy) render();
    try {
      const [known, configs] = await Promise.all([call<Bot[]>('bots_list'), call<Config[]>('superagent_list')]);
      bots = known; presets = configs;
      if (!presets.some(config => config.id === selected)) selected = presets[0]?.id ?? '';
      tickets = selected ? await call<Ticket[]>('superagent_tickets',{ orchestrationId:selected, limit:50 }) : [];
      await refreshTicketDetails();
      error = '';
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { loading = false; if (!busy) render(); }
  }
  async function work(action: () => Promise<void>): Promise<void> {
    if (busy) return;
    busy = true; error = '';
    host.setAttribute('aria-busy', 'true');
    // Preserve any form fields while the native operation is in flight,
    // but prevent a second click from being silently discarded.
    host.querySelectorAll<HTMLButtonElement>('button').forEach(button => { button.disabled = true; });
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
      selected = target.dataset.saSelect;
      inspectedTicket = ''; ticketDetails = null;
      void work(async () => {
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
    if (target.hasAttribute('data-sa-close-details')) {
      inspectedTicket = ''; ticketDetails = null; render(); return;
    }
    if (target.dataset.saInspectTicket) {
      const id = target.dataset.saInspectTicket;
      if (inspectedTicket === id) {
        inspectedTicket = ''; ticketDetails = null; render();
      } else {
        inspectedTicket = id; ticketDetails = null;
        void work(refreshTicketDetails);
      }
      return;
    }
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
      return;
    }
    if (target.dataset.saReject) { rejectingTicket = target.dataset.saReject; render(); return; }
    if (target.hasAttribute('data-sa-reject-cancel')) { rejectingTicket = ''; render(); return; }
    if (target.dataset.saDispatch) {
      const t = tickets.find(item => item.id === target.dataset.saDispatch);
      if (t?.state === 'queued') ask('Dispatch the worker using the original coordinator evidence?', async () => {
        await call('superagent_dispatch_worker',{ ticketId:t.id }); await reload();
      });
      return;
    }
    if (target.dataset.saRetry) {
      const t = tickets.find(item => item.id === target.dataset.saRetry);
      if (t?.state === 'blocked') ask('Retry the original mandate with unchanged permissions?', async () => {
        await call('superagent_retry',{ ticketId:t.id }); await reload();
      });
      return;
    }
    if (target.dataset.saStop) {
      const t = tickets.find(item => item.id === target.dataset.saStop);
      if (t) ask('Stop the task and revoke its authorization?', async () => {
        await call('superagent_stop',{ ticketId:t.id }); await reload();
      });
    }
  });
  host.addEventListener('submit', event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !host.contains(form)) return;
    event.preventDefault();
    const data = new FormData(form);
    if (form.dataset.saForm === 'edit') {
      const existing = current();
      if (!existing || existing.enabled) return;
      const input = {
        name:String(data.get('name') ?? '').trim(),
        coordinatorBotId:String(data.get('coordinatorBotId') ?? ''),
        workerBotId:String(data.get('workerBotId') ?? ''),
      };
      void work(async () => {
        await call('superagent_update',{ orchestrationId:existing.id, expectedRevision:existing.policyRevision, input });
        await reload();
      });
    }
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
    if (form.dataset.saForm === 'reject') {
      const ticket = tickets.find(item => item.id === rejectingTicket);
      const reason = String(data.get('reason') ?? '').trim();
      if (ticket?.state !== 'waiting_approval' || !reason) return;
      void work(async () => {
        await call('superagent_reject',{ ticketId:ticket.id, expectedRevision:ticket.revision, reason });
        rejectingTicket = ''; await reload();
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
