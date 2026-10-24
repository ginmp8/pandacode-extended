import './browser-qa.css';
import { exportReport, needsInteraction, parseFlow, reviewOrigin, runFlow, type FlowReport, type ReviewReply, type ReviewRequest } from './browser-flow.ts';
import type { BrowserScreenshot } from '../types';
export interface BrowserQAHost {
  taskId: string; status(): { url: string; title: string }; active(): boolean;
  review(request: ReviewRequest): Promise<ReviewReply>;
  capture(): Promise<BrowserScreenshot>;
  resize(width: number | null, height: number | null): Promise<{ width: number; height: number }>;
}
export interface BrowserQA { toggle(): void; resize(origin: string, width: number, height: number): Promise<unknown>; dispose(): void; }
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') => { const el = document.createElement(tag); el.textContent = text; el.className = cls; return el; };
let sequence = 0;
/** UI-only workbench, instantiated on demand. No persistent store or network client. */
export function mountBrowserQA(parent: HTMLElement, host: BrowserQAHost): BrowserQA {
  const root = node('section', '', 'hardened-browser-qa'); root.setAttribute('aria-label', 'Browser QA'); root.hidden = true;
  parent.querySelector('.browser-location')?.after(root); if (!root.isConnected && !root.parentElement) parent.prepend(root);
  const life = new AbortController(), signal = life.signal, id = `browser-qa-${++sequence}`;
  let disposed = false, busy = false, epoch = 0, busyEpoch = 0, selected = 0, snapshot: ReviewReply | undefined, evidence = '', report: FlowReport | undefined, run: AbortController | undefined, observationOrigin = '';
  let before: BrowserScreenshot | undefined, after: BrowserScreenshot | undefined;
  const origin = () => { if (!host.active()) throw new Error('Open a loaded browser in the same task first.'); return new URL(host.status().url).origin; };
  const valid = () => !disposed && host.active();
  const status = node('p', '', 'qa-status'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const uiCurrent = () => !disposed && !root.hidden && (!busy || busyEpoch === epoch);
  const announce = (text: string) => { if (uiCurrent()) status.textContent = text.slice(0, 700); };
  const buttons: HTMLButtonElement[] = [];
  function button(label: string, action: () => void, type = 'secondary-button', track = true) { const b = node('button', label, type); b.type = 'button'; b.addEventListener('click', action, { signal }); if (track) buttons.push(b); return b; }
  function field<T extends HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(label: string, control: T, into: HTMLElement): T { const wrap = node('div', '', 'qa-field'), caption = node('label', label); control.id = `${id}-${into.querySelectorAll('input,select,textarea').length}-${sequence++}`; caption.htmlFor = control.id; wrap.append(caption, control); into.append(wrap); return control; }
  function display(value: unknown) {
    if (!uiCurrent()) return;
    evidence = JSON.stringify(value, null, 2).slice(0, 60000); output.textContent = evidence; confirmCopy.checked = false;
  }
  async function work(action: () => Promise<void>) {
    if (disposed || busy || root.hidden) return; busy = true; busyEpoch = epoch; buttons.forEach(b => b.disabled = true);
    try { await action(); } catch (error) { announce(error instanceof Error ? error.message : String(error)); }
    finally { if (!disposed) { busy = false; buttons.forEach(b => b.disabled = false); } }
  }
  async function review(op: string, extra: Partial<ReviewRequest> = {}) {
    const expected = origin(), ticket = epoch, result = await host.review({ ...extra, op, origin: expected });
    if (!uiCurrent() || ticket !== epoch || !valid() || new URL(host.status().url).origin !== expected) throw new Error('Task or browser origin changed. The result was discarded.');
    if (result.ok !== true) throw new Error(result.error || 'No valid browser evidence was returned.');
    if (!result.url || new URL(result.url).origin !== expected) throw new Error('Browser origin changed.');
    return result;
  }
  const heading = node('div', '', 'qa-heading'); heading.append(node('strong', 'Browser QA'), button('Close QA', () => close(), 'secondary-button', false));
  const note = node('p', 'Local checks on the visible page. Page content is untrusted; browser actions can change application data. No headers, response bodies or form values are collected.', 'qa-note');
  const tabs = node('div', '', 'qa-tabs'); tabs.setAttribute('role', 'tablist'); tabs.setAttribute('aria-label', 'Browser QA sections');
  const labels = ['Inspect', 'Interactions', 'Scenarios', 'Screenshots'];
  const panes = labels.map((label, i) => {
    const b = button(label, () => activate(i), '', false); b.setAttribute('role', 'tab'); b.id = `${id}-tab-${i}`; b.setAttribute('aria-controls', `${id}-pane-${i}`); tabs.append(b);
    const pane = node('section', '', 'qa-pane'); pane.id = `${id}-pane-${i}`; pane.setAttribute('role', 'tabpanel'); pane.setAttribute('aria-labelledby', b.id); return pane;
  });
  function activate(i: number) { selected = i; [...tabs.children].forEach((t, n) => { t.setAttribute('aria-selected', String(n === i)); (t as HTMLElement).tabIndex = n === i ? 0 : -1; panes[n].hidden = n !== i; }); }
  tabs.addEventListener('keydown', e => { let i = selected; if (e.key === 'ArrowRight') i = (i + 1) % labels.length; else if (e.key === 'ArrowLeft') i = (i + labels.length - 1) % labels.length; else return; e.preventDefault(); activate(i); (tabs.children[i] as HTMLElement).focus(); }, { signal });
  const inspectActions = node('div', '', 'qa-actions'); panes[0].append(inspectActions);
  for (const [op, label] of [['snapshot','Snapshot'],['audit','Layout and labels'],['resources','Network timing'],['observe_read','Read runtime errors']]) {
    inspectActions.append(button(label, () => void work(async () => {
      const result = await review(op); if (disposed) return; display(result);
      if (op === 'snapshot') { snapshot = result; targets.replaceChildren(); for (const item of (result.elements ?? []).slice(0, 200)) { const option = node('option', `${item.selector} (${String((item as Record<string, unknown>).name ?? '').slice(0, 80)})`); option.value = item.ref; targets.append(option); } allowAction.checked = false; }
      announce(op === 'audit' ? 'Heuristic findings collected. This is not a complete accessibility audit.' : 'Evidence collected. Review it before copying.');
    })));
  }
  const monitoring = node('div', '', 'qa-actions'); panes[0].append(monitoring);
  monitoring.append(button('Observe errors (10 min)', () => void work(async () => { const expected = origin(); const result = await review('observe_start'); if (!disposed) { observationOrigin = expected; display(result); announce('Only new errors and unhandled rejections are observed. Console methods are not intercepted.'); } })),
    button('Stop observing', () => void work(async () => { display(await review('observe_stop')); observationOrigin = ''; announce('Runtime error observation stopped.'); })));
  const sizing = node('div', '', 'qa-actions'); panes[0].append(sizing);
  const size = field('Viewport target', node('select'), sizing);
  for (const [value, label] of [['fit','Fit panel'],['390x640','Phone 390 x 640'],['768x900','Tablet 768 x 900'],['1280x720','Desktop 1280 x 720']]) { const o = node('option', label); o.value = value; size.append(o); }
  sizing.append(button('Apply viewport', () => void work(async () => {
    const expected = origin(), parts = size.value === 'fit' ? [null, null] : size.value.split('x').map(Number);
    const actual = await host.resize(parts[0], parts[1]);
    if (!uiCurrent() || !valid() || origin() !== expected) return;
    snapshot = undefined; allowAction.checked = false;
    announce(`Actual viewport: ${Math.round(actual.width)} x ${Math.round(actual.height)}. Targets are clamped to the visible panel; this is not device emulation.`);
  })));
  panes[1].append(node('p', 'Take a Snapshot first. Any DOM change expires its refs. Actions are dispatched once, without automatic retry; verify their effects afterward.', 'qa-note'));
  const targets = field('Element from latest snapshot', node('select'), panes[1]);
  const value = field('Non-secret test value', node('input'), panes[1]); value.maxLength = 2048; value.autocomplete = 'off';
  const allowAction = node('input'); allowAction.type = 'checkbox'; field('I authorize this interaction on the current page', allowAction, panes[1]);
  for (const control of [targets, value]) control.addEventListener('input', () => { allowAction.checked = false; }, { signal });
  const interactionActions = node('div', '', 'qa-actions'); panes[1].append(interactionActions);
  for (const op of ['click','focus','fill','select','check']) interactionActions.append(button(op[0].toUpperCase() + op.slice(1), () => void work(async () => {
    if (!allowAction.checked || !snapshot?.token || !targets.value) throw new Error('Take a new snapshot and explicitly authorize the selected action.');
    const captured = { token: snapshot.token, ref: targets.value, value: value.value }; allowAction.checked = false; snapshot = undefined;
    display(await review(op, captured)); announce('DOM action dispatched. Take another snapshot or run an assertion to verify the result.');
  })));
  for (const dir of ['up','down']) interactionActions.append(button(`Scroll ${dir}`, () => void work(async () => { if (!allowAction.checked) throw new Error('Authorize the page interaction first.'); allowAction.checked = false; display(await review('scroll', { value: dir })); })));
  panes[2].append(node('p', 'Finite JSON scenarios: 32 steps, 45 seconds, no scripts. Expected text/status must come from your requirements, not from the result being tested. A failed or unknown assertion stops the run.', 'qa-note'));
  const scenario = field('Scenario JSON', node('textarea'), panes[2]); scenario.rows = 6; scenario.maxLength = 65536; scenario.spellcheck = false;
  const init = () => { let expected = 'http://localhost:3000'; try { expected = origin(); } catch {} scenario.value = JSON.stringify({ version: 1, origin: expected, steps: [{ op: 'expect_text', value: 'REPLACE WITH EXPECTED UI TEXT' }, { op: 'expect_resource', value: '/api/health', status: 200 }] }, null, 2); };
  init();
  const allowFlow = node('input'); allowFlow.type = 'checkbox'; field('I authorize the interactions in this exact scenario', allowFlow, panes[2]);
  scenario.addEventListener('input', () => { allowFlow.checked = false; report = undefined; }, { signal });
  const flowActions = node('div', '', 'qa-actions'); panes[2].append(flowActions);
  flowActions.append(button('Validate scenario', () => void work(async () => { const flow = parseFlow(scenario.value); if (flow.origin !== origin()) throw new Error('Scenario origin differs from the current page.'); announce(`${flow.steps.length} steps. ${needsInteraction(flow) ? 'Interaction confirmation is required.' : 'No mutating steps.'} This validates structure, not application behavior.`); })),
    button('Run checks', () => void work(async () => {
      const plan = parseFlow(scenario.value), allowed = allowFlow.checked; allowFlow.checked = false; run = new AbortController(); report = undefined;
      const expected = origin(); announce('Running bounded checks...');
      const result = await runFlow(plan, { taskId: host.taskId, origin: expected, interactive: allowed, signal: run.signal,
        active: () => valid() && new URL(host.status().url).origin === expected,
        review: host.review, capture: host.capture, resize: (w, h) => host.resize(w, h),
        progress: (n, total) => announce(`Checked ${n} of ${total} steps.`),
      });
      run = undefined; if (!uiCurrent()) return; report = result; evidence = exportReport(result); output.textContent = evidence; confirmCopy.checked = false;
      announce(`Scenario ${result.status}: ${result.steps.length} recorded steps. This conclusion applies only to the supplied checks.`);
    }), 'primary-button'), button('Stop checks', () => { run?.abort(); announce('Stop requested. An action already dispatched is not rolled back.'); }, 'secondary-button', false));
  panes[3].append(node('p', 'Capture a before/after viewport for manual comparison. Images may contain private information; they are not sent to an agent or copied with text reports.', 'qa-note'));
  const captures = node('div', '', 'qa-captures'), images = [node('img'), node('img')]; images[0].alt = 'Reference viewport capture'; images[1].alt = 'Current viewport capture'; images.forEach(img => { img.hidden = true; captures.append(img); });
  const captureActions = node('div', '', 'qa-actions'); panes[3].append(captureActions, captures);
  for (const [index, label] of [[0,'Capture reference'],[1,'Capture current']] as const) captureActions.append(button(label, () => void work(async () => {
    const expected = origin(); const shot = await host.capture();
    if (!uiCurrent() || !valid() || origin() !== expected || new URL(shot.url).origin !== expected) throw new Error('Task or origin changed during capture.');
    if (!shot.dataUrl?.startsWith('data:image/png;base64,') || shot.dataUrl.length > 8 * 1024 * 1024) throw new Error('Screenshot format or size is invalid.');
    if (index === 0) before = shot; else after = shot; images[index].src = shot.dataUrl; images[index].hidden = false;
    announce(before && after ? 'Two viewport captures are visible for manual comparison. No automated visual-equivalence verdict is implied.' : 'Viewport captured locally.');
  })));
  const details = node('details', '', 'qa-evidence'); details.append(node('summary', 'Collected evidence (review before sharing)'));
  const output = node('pre'); output.setAttribute('aria-label', 'Browser QA evidence'); details.append(output);
  const confirmCopy = node('input'); confirmCopy.type = 'checkbox'; field('I reviewed the evidence and approve copying it', confirmCopy, details);
  details.append(button('Copy evidence', () => void work(async () => { if (!evidence || !confirmCopy.checked) throw new Error('Review the collected evidence and confirm copying first.'); const text = evidence; confirmCopy.checked = false; await navigator.clipboard.writeText(text); announce('Evidence copied. Clipboard history may retain it outside PandaCode.'); })));
  root.append(heading, note, tabs, ...panes, details, status); activate(0);
  function clearPrivate() { snapshot = undefined; before = after = undefined; evidence = ''; report = undefined; output.textContent = ''; value.value = ''; scenario.value = ''; targets.replaceChildren(); images.forEach(img => { img.removeAttribute('src'); img.hidden = true; }); confirmCopy.checked = allowAction.checked = allowFlow.checked = false; }
  function close() {
    epoch++; run?.abort(); root.hidden = true; clearPrivate();
    let expected = observationOrigin; observationOrigin = '';
    try { if (host.active()) expected = expected || origin(); } catch { /* closed browser */ }
    if (expected && host.active() && new URL(host.status().url).origin === expected) void host.review({ op: 'clear', origin: expected }).catch(() => {});
    void host.resize(null, null).catch(() => {});
  }
  return {
    toggle() { if (disposed) return; if (!root.hidden) close(); else { epoch++; root.hidden = false; init(); root.querySelector<HTMLButtonElement>('[role=tab]')?.focus(); } },
    async resize(expected, width, height) {
      reviewOrigin(expected);
      if (!Number.isInteger(width) || width < 320 || width > 1920 || !Number.isInteger(height) || height < 240 || height > 1440) throw new Error('Viewport must be 320-1920 by 240-1440.');
      if (origin() !== expected) throw new Error('Browser origin changed.');
      const actual = await host.resize(width, height); if (!valid() || origin() !== expected) throw new Error('Browser changed during resize.');
      return { requested: { width, height }, actual, exact: Math.abs(actual.width - width) <= 2 && Math.abs(actual.height - height) <= 2, note: 'Panel bounds apply. Not device/touch emulation.' };
    },
    dispose() { if (disposed) return; close(); disposed = true; life.abort(); root.remove(); },
  };
}
