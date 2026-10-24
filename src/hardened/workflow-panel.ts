import { buildWorkflow, prepareWorkflow, saveWorkflow, workflows, type WorkflowHost, type WorkflowInput } from './workflow-library.ts';
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') => { const el = document.createElement(tag); el.textContent = text; el.className = cls; return el; };
/** Present existing engine capabilities without another scheduler or execution path. */
export function mountWorkflowLibrary(root: HTMLElement, owner: WorkflowHost): () => void {
  const life = new AbortController(), taskId = owner.current()?.id ?? '';
  let disposed = false, busy = false, serial = 0;
  const host: WorkflowHost = { ...owner, active: () => !disposed && owner.active() && owner.current()?.id === taskId };
  const status = node('p', '', 'review-status'); status.setAttribute('role', 'status');
  const announce = (message: string) => { if (!disposed) status.textContent = message.slice(0, 800); };
  const intro = node('p', 'Prepare one bounded prompt or save a local project recipe. Saving writes the selected scope and criteria to .pandacode/recipes; it never queues a task. Sending uses the original composer and approval controls. Tools unavailable in a provider must be reported as not-run.', 'review-note');
  const grid = node('div', '', 'review-grid');
  function field<T extends HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(label: string, input: T): T { const wrap = node('div', '', 'review-field'), caption = node('label', label); input.id = `workflow-${++serial}`; caption.htmlFor = input.id; wrap.append(caption, input); grid.append(wrap); return input; }
  const kind = field('Workflow', node('select'));
  for (const item of workflows) { const option = node('option', item.name); option.value = item.id; kind.append(option); }
  const origin = field('Approved site origin (browser workflows)', node('input')); origin.placeholder = 'http://localhost:3000'; origin.maxLength = 2048; origin.autocomplete = 'off';
  const goal = field('Workflow goal', node('textarea')); goal.maxLength = 800; goal.rows = 3;
  const scope = field('Files or user flow in scope', node('textarea')); scope.maxLength = 1000; scope.rows = 3;
  const acceptance = field('Independent expected results', node('textarea')); acceptance.maxLength = 800; acceptance.rows = 3;
  const details = node('details'); details.open = true; details.append(node('summary', 'Exact prompt / recipe preview'));
  const preview = node('pre', '', 'review-preview'); details.append(preview);
  const confirm = node('input'); confirm.type = 'checkbox';
  confirm.id = `workflow-confirm-${++serial}`;
  const confirmation = node('div', '', 'review-field'), confirmationLabel = node('label', 'I reviewed this exact scope, expectations and information to be shared'); confirmationLabel.htmlFor = confirm.id;
  confirmation.append(confirm, confirmationLabel);
  const actions = node('div', '', 'review-actions'); const buttons: HTMLButtonElement[] = [];
  const input = (): WorkflowInput => ({ kind: kind.value, goal: goal.value, scope: scope.value, acceptance: acceptance.value, origin: origin.value });
  const changed = () => { confirm.checked = false; try { const recipe = buildWorkflow(input()); preview.textContent = JSON.stringify(recipe, null, 2); } catch (error) { preview.textContent = error instanceof Error ? error.message : String(error); } origin.disabled = !workflows.find(w => w.id === kind.value)?.browser; };
  for (const control of [kind, origin, goal, scope, acceptance]) control.addEventListener('input', changed, { signal: life.signal });
  const task = owner.current(); if (!task) announce('Select an idle local project task to use these workflows.');
  async function perform(action: (request: WorkflowInput) => Promise<void>) {
    if (disposed || busy) return;
    if (!confirm.checked) { announce('Review the preview and confirm this exact scope first.'); return; }
    if (!host.active() || !taskId) { announce('The task changed. Reopen the library.'); return; }
    const request = input(); confirm.checked = false; busy = true; buttons.forEach(b => b.disabled = true);
    try { await action(request); } catch (error) { announce(error instanceof Error ? error.message : String(error)); }
    finally { if (!disposed) { busy = false; buttons.forEach(b => b.disabled = false); } }
  }
  function button(label: string, action: () => void, primary = false) { const b = node('button', label, primary ? 'primary-button' : 'secondary-button'); b.type = 'button'; b.addEventListener('click', action, { signal: life.signal }); buttons.push(b); return b; }
  actions.append(button('Prepare in composer', () => { void perform(async request => { await prepareWorkflow(host, taskId, request); announce('Prepared in the original composer. Nothing was sent.'); }); }, true),
    button('Save project recipe', () => { void perform(async request => { const result = await saveWorkflow(host, taskId, buildWorkflow(request)); announce(result.created ? 'Project recipe saved. Open Recipes for the original compatibility preview and explicit Run action.' : 'An identical recipe already exists; no file was changed.'); }); }));
  const navigation = node('div', '', 'review-actions');
  for (const [view, label] of [['browser','Open existing browser'],['recipes','Open original Recipes']] as const) navigation.append(button(label, () => {
    if (!host.active() || !host.open) { announce('Select the original project task before opening this view.'); return; }
    void Promise.resolve(host.open(view)).catch(error => announce(String(error)));
  }));
  root.append(intro, grid, details, confirmation, actions, navigation, status); changed();
  return () => { if (disposed) return; disposed = true; life.abort(); root.replaceChildren(); };
}
