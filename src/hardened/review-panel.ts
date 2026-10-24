import './review-tools.css';
import { mountWorkflowLibrary } from './workflow-panel.ts';
import type { WorkflowHost } from './workflow-library.ts';
import { buildReview, buildHandoff, collectAttention, inspectTeam, reviewProblems, type HandoffContext, type HandoffFields, type PendingAttention, type ReviewContext, type ReviewKind, type ReviewRequest } from './review-model.ts';
import { prepareReviewPrompt, saveReviewProfile, type ReviewCall } from './review-actions.ts';
import type { TeamSnapshot } from '../architect-types';
import type { Provider, ProviderId, Session } from '../types';

export interface ReviewReadiness { status: 'containment_unproven' | 'capability_missing' | 'proven'; ready: boolean; provenProviders: ProviderId[]; note: string }
export interface ReviewSnapshot extends ReviewContext, HandoffContext {
  sessions: Session[]; teams: TeamSnapshot[]; pending: PendingAttention[];
}
export interface ReviewToolsHost {
  call: ReviewCall;
  snapshot(): ReviewSnapshot;
  refresh(): Promise<void>;
  loadProviders(selected?: ProviderId[]): Promise<Provider[]>;
  reviewReadiness?: () => Promise<ReviewReadiness>;
  openTask(sessionId: string, workerId?: string): Promise<void>;
  insertPrompt(sessionId: string, text: string, request: ReviewRequest, providers: Provider[]): void | Promise<void>;
  copyText(text: string): Promise<void>;
  workflow?: WorkflowHost;
}
let sequence = 0;
const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', className = ''): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag); node.textContent = text; node.className = className; return node;
};

/** Mounted only on demand; no global listeners, polling, transcript reads or background work. */
export function mountReviewTools(root: HTMLElement, host: ReviewToolsHost): () => void {
  const abort = new AbortController(), signal = abort.signal, id = `hardened-review-${++sequence}`;
  let disposed = false, busy = false, snapshot = host.snapshot(), providers: Provider[] = [], selectedTab = 0;
  const boundTask = snapshot.task?.id ?? '';
  root.classList.add('hardened-review');
  const status = el('p', '', 'review-status'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const announce = (text: string) => { if (!disposed) status.textContent = text.slice(0, 800); };
  const changed = () => (host.snapshot().task?.id ?? '') !== boundTask;
  const request = (): ReviewRequest => ({
    kind: kind.value as ReviewKind, provider: provider.value as ProviderId, model: model.value,
    reviewers: (kind.value === 'committee' ? ['correctness', 'risk'] : ['advisor']).map((id, i) => {
      const chosenProvider = (reviewerProviders[i].value || provider.value) as ProviderId;
      return { id: id as 'advisor' | 'correctness' | 'risk', provider: chosenProvider,
        model: reviewerModels[i].value || (chosenProvider === provider.value ? model.value : '') };
    }),
    question: question.value, files: files.value, acceptance: acceptance.value,
  });
  const context = (): ReviewContext => ({ ...snapshot, providers });
  async function perform(operation: () => Promise<void>): Promise<void> {
    if (disposed || busy) return;
    busy = true; paint();
    try { await operation(); }
    catch (error) { announce(error instanceof Error ? error.message : String(error)); }
    finally { if (!disposed) { busy = false; paint(); } }
  }
  function button(text: string, handler: () => void, primary = false): HTMLButtonElement {
    const node = el('button', text, primary ? 'primary-button' : 'secondary-button'); node.type = 'button';
    node.addEventListener('click', handler, { signal }); return node;
  }
  let fieldNumber = 0;
  function field<T extends HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(parent: HTMLElement, label: string, control: T): T {
    control.id = `${id}-field-${++fieldNumber}`;
    const wrap = el('div', '', 'review-field'), caption = el('label', label);
    caption.htmlFor = control.id; wrap.append(caption, control); parent.append(wrap); return control;
  }
  function checkboxField(parent: HTMLElement, label: string): HTMLInputElement {
    const control = el('input'); control.type = 'checkbox'; control.id = `${id}-field-${++fieldNumber}`;
    const wrap = el('div', '', 'review-field'), caption = el('label', label); caption.htmlFor = control.id;
    wrap.append(control, caption); parent.append(wrap); return control;
  }
  const note = el('p', 'Local tools for the existing PandaCode engine. Nothing starts automatically; approvals remain in the original controls.', 'review-note');
  const toolbar = el('div', '', 'review-toolbar');
  const refreshButton = button('Refresh snapshot', () => { void perform(refresh); });
  toolbar.append(el('span', '', 'review-task-label'), refreshButton);
  const tabs = el('div', '', 'review-tabs'); tabs.setAttribute('role', 'tablist'); tabs.setAttribute('aria-label', 'Review tools');
  const labels = ['Review presets', 'Team inspector', 'Attention', 'Handoff'];
  const panes = labels.map((label, i) => {
    const tab = button(label, () => activate(i)); tab.id = `${id}-tab-${i}`; tab.setAttribute('role', 'tab'); tab.setAttribute('aria-controls', `${id}-pane-${i}`); tabs.append(tab);
    const pane = el('section', '', 'review-pane'); pane.id = `${id}-pane-${i}`; pane.setAttribute('role', 'tabpanel'); pane.setAttribute('aria-labelledby', tab.id); return pane;
  });
  function activate(index: number): void {
    selectedTab = index;
    [...tabs.children].forEach((node, i) => { node.setAttribute('aria-selected', String(i === index)); (node as HTMLElement).tabIndex = i === index ? 0 : -1; panes[i].hidden = i !== index; });
    paintSnapshots();
  }
  tabs.addEventListener('keydown', event => {
    let next = selectedTab;
    if (event.key === 'ArrowRight') next = (next + 1) % labels.length;
    else if (event.key === 'ArrowLeft') next = (next + labels.length - 1) % labels.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = labels.length - 1;
    else return;
    event.preventDefault(); activate(next); (tabs.children[next] as HTMLElement).focus();
  }, { signal });
  root.append(note, toolbar, tabs, ...panes, status);

  const presets = panes[0];
  const isolated = el('section', '', 'review-isolation');
  isolated.append(el('strong', 'Isolated review — native security gate'));
  const isolationState = el('p', 'Checking native containment status...', 'review-note'); isolated.append(isolationState);
  const isolationRefresh = button('Check isolation readiness', () => { void perform(checkReadiness); });
  isolated.append(isolationRefresh); presets.append(isolated);
  presets.append(el('p', 'Manual mode only: save a profile, then use the original model picker on a NEW project task and select Read only. Return here to prepare a NON-ISOLATED prompt; Send remains manual.', 'review-note'));
  presets.append(el('p', 'Saving here does not change defaults. Selecting a team in the original picker retains PandaCode\'s existing default-team behavior. Plan mode, worker readOnly flags and prompts do not isolate files, credentials or network, for ANY provider.', 'review-note'));
  const form = el('div', '', 'review-grid'); presets.append(form);
  const kind = field(form, 'Preset', el('select'));
  for (const [value, label] of [['advisor', 'Advisor - one reviewer'], ['committee', 'Committee - two reviewers']]) { const option = el('option', label); option.value = value; kind.append(option); }
  const provider = field(form, 'Coordinator provider', el('select'));
  const model = field(form, 'Coordinator model', el('select'));
  const reviewerProviders: HTMLSelectElement[] = [], reviewerModels: HTMLSelectElement[] = [];
  const reviewerFields: HTMLElement[] = [];
  for (const i of [0, 1]) {
    const group = el('fieldset', '', 'review-role');
    const legend = el('legend', i === 0 ? 'Reviewer 1' : 'Reviewer 2'); group.append(legend);
    reviewerProviders[i] = field(group, `Reviewer ${i + 1} provider`, el('select'));
    reviewerModels[i] = field(group, `Reviewer ${i + 1} model`, el('select'));
    reviewerFields[i] = group; form.append(group);
  }
  const updateRoleLabels = () => {
    reviewerFields[1].hidden = kind.value !== 'committee';
    (reviewerFields[0].querySelector('legend')!).textContent = kind.value === 'committee' ? 'Correctness reviewer' : 'Advisor';
    (reviewerFields[1].querySelector('legend')!).textContent = 'Risk reviewer';
  };
  updateRoleLabels();
  const profileName = field(form, 'Saved profile name', el('input')); profileName.maxLength = 80; profileName.value = 'Hardened advisor';
  const question = field(form, 'Review question', el('textarea')); question.rows = 3; question.maxLength = 8192;
  const files = field(form, 'Relative file scopes (one per line)', el('textarea')); files.rows = 3; files.maxLength = 8192; files.placeholder = 'src/parser.ts\ntests/parser.test.ts';
  const acceptance = field(form, 'Review acceptance criteria', el('textarea')); acceptance.rows = 3; acceptance.maxLength = 4096;
  const checks = el('ul', '', 'review-checks'); checks.setAttribute('aria-label', 'Preparation checks'); presets.append(checks);
  const previewDetails = el('details'); previewDetails.append(el('summary', 'Prompt preview'));
  const promptPreview = el('pre', '', 'review-preview'); previewDetails.append(promptPreview); presets.append(previewDetails);
  const actions = el('div', '', 'review-actions');
  const saveButton = button('Save team profile', () => { void perform(async () => {
    const plan = buildReview(request());
    const selections = [plan.setup.architect, ...plan.setup.config.workers];
    if (selections.some(choice => !providers.some(p => p.id === choice.provider && p.installed && p.models.some(m => m.id === choice.model)))) {
      throw new Error('Refresh the local catalog and choose available provider models for every role.');
    }
    const result = await saveReviewProfile(host.call, profileName.value, plan.setup);
    announce(result.created ? 'Profile saved. No task, permission or default was changed; select it using the original model picker.' : 'The same profile already exists. Nothing was changed.');
  }); });
  const insertButton = button('Insert prompt', () => { void perform(async () => {
    const taskId = snapshot.task?.id, frozenRequest = request();
    if (!taskId || changed()) throw new Error('The selected task changed. Close and reopen this panel.');
    const roles = buildReview(frozenRequest);
    const selected = [roles.setup.architect.provider, ...roles.setup.config.workers.map(w => w.provider)];
    const freshProviders = await host.loadProviders([...new Set(selected)]);
    if (disposed || changed()) return;
    const text = await prepareReviewPrompt(host.call, taskId, freshProviders, frozenRequest);
    if (disposed) return;
    if (changed()) throw new Error('The selected task changed. Nothing was inserted.');
    await host.insertPrompt(taskId, text, frozenRequest, freshProviders);
    announce('Prompt prepared. Review it and use the normal Send control when ready.');
  }); }, true);
  actions.append(saveButton, insertButton); presets.append(actions);
  kind.addEventListener('change', () => { profileName.value = kind.value === 'committee' ? 'Hardened committee' : 'Hardened advisor'; updateRoleLabels(); paint(); }, { signal });
  provider.addEventListener('change', () => { void perform(refresh); }, { signal });
  for (const input of reviewerProviders) input.addEventListener('change', () => { void perform(refresh); }, { signal });
  for (const input of [model, ...reviewerModels, question, files, acceptance]) input.addEventListener('input', () => paint(), { signal });

  const inspector = panes[1], attention = panes[2];
  for (const pane of [inspector, attention]) pane.addEventListener('click', event => {
    const control = (event.target as Element).closest<HTMLButtonElement>('button[data-review-session]');
    if (control && pane.contains(control)) open(control.dataset.reviewSession!, control.dataset.reviewWorker);
  }, { signal });
  const handoff = panes[3];
  handoff.append(el('p', 'Only the fields below and optional worker checkpoints are copied. No transcript, attachment, environment or credential store is collected. Redaction is best-effort: inspect the preview before copying.', 'review-note'));
  const handoffForm = el('div', '', 'review-grid'); handoff.append(handoffForm);
  const handoffFields: Partial<Record<keyof HandoffFields, HTMLTextAreaElement>> = {};
  for (const [key, label] of [['goal', 'Goal'], ['context', 'Selected context'], ['decisions', 'Decisions and failed attempts'], ['nextSteps', 'Next steps'], ['reportedChecks', 'Reported checks (not verified)'], ['acceptance', 'Acceptance criteria']] as const) {
    const input = field(handoffForm, label, el('textarea')); input.rows = 3; input.maxLength = 4096; handoffFields[key] = input;
    input.addEventListener('input', () => { confirmed.checked = false; paintHandoff(); }, { signal });
  }
  handoffFields.goal!.value = snapshot.task?.title ?? '';
  const include = checkboxField(handoff, 'Include this task\'s worker checkpoints (agent-reported)');
  include.addEventListener('change', () => { confirmed.checked = false; paintHandoff(); }, { signal });
  const handoffPreview = field(handoff, 'Handoff preview', el('textarea')); handoffPreview.readOnly = true; handoffPreview.rows = 10; handoffPreview.className = 'review-preview';
  const confirmed = checkboxField(handoff, 'I reviewed this preview for sensitive information.');
  const handoffError = el('p', '', 'review-note'); handoff.append(handoffError);
  const copyButton = button('Copy handoff', () => { void perform(async () => {
    if (changed()) throw new Error('The selected task changed. Close and reopen this panel before exporting.');
    if (!confirmed.checked || !handoffPreview.value) throw new Error('Review the handoff preview before copying.');
    await host.copyText(handoffPreview.value);
    announce('Handoff copied. No agent was started and no conversation was changed.');
  }); }, true);
  handoff.append(copyButton);
  confirmed.addEventListener('change', () => paintHandoff(), { signal });

  function paintHandoff(): void {
    let text = '';
    try {
      const fields = Object.fromEntries(Object.entries(handoffFields).map(([key, input]) => [key, input!.value])) as unknown as HandoffFields;
      text = buildHandoff(snapshot, fields, include.checked); handoffError.textContent = '';
    } catch (error) { handoffError.textContent = (error as Error).message; }
    if (handoffPreview.value !== text) confirmed.checked = false;
    handoffPreview.value = text;
    copyButton.disabled = busy || changed() || !text || !confirmed.checked;
  }
  function open(sessionId: string, workerId?: string): void {
    void perform(async () => { await host.openTask(sessionId, workerId); });
  }
  function paintSnapshots(): void {
    if (disposed) return;
    if (selectedTab === 1) {
      inspector.replaceChildren(el('p', 'Read-only snapshot. The native scheduler is authoritative and also checks other teams. Refresh explicitly; no polling runs here.', 'review-note'));
      const t = snapshot.team;
      if (!t) { inspector.append(el('p', 'No team loaded for the selected task.')); return; }
      inspector.append(el('p', `Revision ${t.revision} | Lead turns ${t.round}/${t.config.maxRounds} | Parallel limit ${t.config.maxParallel || 'unlimited (existing configuration)'}`));
      const rows = inspectTeam(t);
      if (!rows.length) { inspector.append(el('p', 'No worker assignments yet.')); return; }
      const scroll = el('div', '', 'review-table-scroll'), table = el('table'); table.setAttribute('aria-label', 'Worker dependencies and blockers');
      const head = el('thead'), heading = el('tr');
      for (const label of ['Worker', 'Status', 'Dependencies', 'File scope', 'Reason']) { const cell = el('th', label); cell.scope = 'col'; heading.append(cell); } head.append(heading); table.append(head);
      const body = el('tbody');
      for (const row of rows) {
        const tr = el('tr'), name = el('td'); const control = el('button', row.name, 'secondary-button'); control.type = 'button'; control.dataset.reviewSession = t.sessionId; control.dataset.reviewWorker = row.id; control.setAttribute('aria-label', `Open worker: ${row.name}`); name.append(control); tr.append(name);
        for (const text of [row.status, row.dependencies, row.scopes, row.reason]) tr.append(el('td', text)); body.append(tr);
      }
      table.append(body); scroll.append(table); inspector.append(scroll);
    }
    if (selectedTab === 2) {
      attention.replaceChildren(el('p', 'Up to 200 items from loaded tasks and teams. Pending requests may be incomplete after restart. Opening an item does not approve, answer or retry it.', 'review-note'));
      const items = collectAttention(snapshot.sessions, snapshot.teams, snapshot.pending);
      if (!items.length) attention.append(el('p', 'No attention items in the loaded snapshot.'));
      else {
        const list = el('ul', '', 'review-attention');
        for (const item of items) {
          const li = el('li'); li.append(el('strong', item.title), el('span', `${item.reason}${item.count > 1 ? ` (${item.count})` : ''}`));
          const control = el('button', item.workerId ? 'Open worker' : 'Open task', 'secondary-button'); control.type = 'button'; control.dataset.reviewSession = item.sessionId; if (item.workerId) control.dataset.reviewWorker = item.workerId;
          control.setAttribute('aria-label', `${item.workerId ? 'Open worker' : 'Open task'}: ${item.title}`); li.append(control); list.append(li);
        }
        attention.append(list);
      }
    }
  }
  function paint(): void {
    if (disposed) return;
    toolbar.firstElementChild!.textContent = snapshot.task?.title ?? 'No project task selected';
    refreshButton.disabled = busy;
    let valid = false;
    try { promptPreview.textContent = buildReview(request()).prompt; valid = true; }
    catch (error) { promptPreview.textContent = (error as Error).message; }
    const problems = reviewProblems(context(), request());
    if (changed()) problems.push('The selected task changed. Close and reopen this panel.');
    checks.replaceChildren(...(problems.length ? problems : ['Ready to prepare a prompt. Native state will be rechecked; sending remains manual.']).map(text => el('li', text)));
    let available = false;
    if (valid) {
      const plan = buildReview(request());
      available = [plan.setup.architect, ...plan.setup.config.workers].every(choice =>
        providers.some(p => p.id === choice.provider && p.installed && p.models.some(m => m.id === choice.model)));
    }
    saveButton.disabled = busy || !available || !valid;
    isolationRefresh.disabled = busy;
    insertButton.disabled = busy || problems.length > 0;
    paintHandoff();
  }
  /** Explicit native inventory only; never attempts to launch a harness or a jail. */
  async function checkReadiness(): Promise<void> {
    if (!host.reviewReadiness) {
      isolationState.textContent = 'Native isolation status unavailable. Isolated review is disabled; manual prompts are NOT isolated.';
      return;
    }
    try {
      const report = await host.reviewReadiness();
      if (disposed) return;
      // Display cannot authorize execution. There is intentionally no start button.
      isolationState.textContent = report.ready && report.status === 'proven'
        ? `Native review readiness reported for ${report.provenProviders.join(', ') || 'no providers'}. No isolated start action is implemented in this release.`
        : `${report.status}: ${report.note} Isolated review is disabled.`;
    } catch {
      if (!disposed) { isolationState.textContent = 'Native security status could not be verified. Isolated review is disabled.'; }
    }
  }
  const replaceOptions = (select: HTMLSelectElement, choices: { value: string; label: string }[], preferred: string) => {
    select.replaceChildren();
    for (const choice of choices) {
      const option = el('option', choice.label); option.value = choice.value; select.append(option);
    }
    if (choices.some(choice => choice.value === preferred)) select.value = preferred;
  };
  async function refresh(): Promise<void> {
    const previousProvider = provider.value || snapshot.task?.provider || 'codex';
    const previousModel = model.value || snapshot.task?.model || '';
    const previousReviewers = reviewerProviders.map((p, i) => ({ provider: p.value, model: reviewerModels[i].value }));
    providers = [];
    try {
      const selected = [previousProvider, ...previousReviewers.map(r => r.provider || previousProvider)] as ProviderId[];
      const [catalog] = await Promise.all([host.loadProviders([...new Set(selected)]), host.refresh()]);
      if (disposed) return;
      if (changed()) { announce('The selected task changed. Close and reopen this panel.'); return; }
      providers = catalog; snapshot = host.snapshot();
      replaceOptions(provider, catalog.filter(p => p.installed).map(p => ({ value: p.id, label: p.name })), previousProvider);
      if (!provider.options.length) replaceOptions(provider, [{ value: '', label: 'No installed providers' }], '');
      const lead = catalog.find(p => p.id === provider.value && p.installed);
      replaceOptions(model, (lead?.models ?? []).map(m => ({ value: m.id, label: m.name || m.id })), previousModel);
      if (!model.options.length) replaceOptions(model, [{ value: '', label: 'No available model for coordinator' }], '');
      for (const i of [0, 1]) {
        const old = previousReviewers[i];
        const options = [{ value: '', label: 'Same as coordinator' },
          ...catalog.filter(p => p.installed).map(p => ({ value: p.id, label: p.name }))];
        replaceOptions(reviewerProviders[i], options, old.provider);
        const selectedProvider = reviewerProviders[i].value || provider.value;
        const models = catalog.find(p => p.id === selectedProvider && p.installed)?.models ?? [];
        replaceOptions(reviewerModels[i], [
          ...(selectedProvider === provider.value ? [{ value: '', label: 'Same as coordinator model' }] : []),
          ...models.map(m => ({ value: m.id, label: m.name || m.id })),
        ], old.model);
        if (!reviewerModels[i].options.length) replaceOptions(reviewerModels[i], [{ value: '', label: 'No available reviewer model' }], '');
      }
      announce('Local provider/model catalog refreshed. No agent was started.');
    } finally { if (!disposed) { paint(); paintSnapshots(); } }
  }
  void checkReadiness();
  const workflowSection = el('details'); workflowSection.className = 'workflow-library'; workflowSection.append(el('summary', 'Workflow library: visual, integration and code review'));
  const workflowRoot = el('section'); workflowSection.append(workflowRoot); presets.append(workflowSection);
  const workflowCleanup = host.workflow ? mountWorkflowLibrary(workflowRoot, host.workflow) : undefined;
  if (!host.workflow) workflowRoot.append(el('p', 'Workflow library requires an active local project task.'));
  activate(0); paint(); void perform(refresh);
  return () => { if (disposed) return; disposed = true; workflowCleanup?.(); abort.abort(); root.replaceChildren(); root.classList.remove('hardened-review'); };
}
