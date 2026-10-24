/** Finite browser scenarios. This is not an agent scheduler or a network proxy. */
export type ReviewReply = { ok?: boolean; url?: string; passed?: boolean; error?: string; token?: string; elements?: { selector: string; ref: string }[]; [key: string]: unknown };
export type ReviewRequest = { op: string; origin: string; selector?: string; value?: string; status?: number; token?: string; ref?: string };
export type FlowStep = { op: string; selector?: string; value?: string; status?: number; timeoutMs?: number; width?: number; height?: number };
export interface BrowserFlow { version: 1; origin: string; steps: FlowStep[]; }
export interface FlowReport { version: 1; taskId: string; origin: string; status: 'passed' | 'failed' | 'cancelled'; elapsedMs: number; steps: { index: number; op: string; status: 'passed' | 'failed'; detail: string }[]; }
export interface FlowHost {
  taskId: string; origin: string; interactive: boolean; active(): boolean; signal?: AbortSignal;
  review(request: ReviewRequest): Promise<ReviewReply>;
  capture?(): Promise<{ url: string }>;
  resize?(width: number, height: number): Promise<{ width: number; height: number }>;
  now?(): number; delay?(ms: number): Promise<void>;
  progress?(completed: number, total: number): void;
}
const mutating = new Set(['click', 'fill', 'select', 'check', 'focus', 'scroll', 'resize']);
const fields: Record<string, string[]> = {
  snapshot: [], audit: [], resources: [], screenshot: [],
  expect_text: ['value'], expect_title: ['value'], expect_url: ['value'], expect_element: ['selector'], expect_resource: ['value', 'status'],
  wait_text: ['value', 'timeoutMs'], click: ['selector'], fill: ['selector', 'value'], select: ['selector', 'value'], check: ['selector', 'value'], focus: ['selector'], scroll: ['value'], resize: ['width', 'height'],
};
function object(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }
export function reviewOrigin(value: string): string {
  if (typeof value !== 'string' || value.length > 2048 || /[\u0000-\u0020]/.test(value)) throw new Error('Use an explicit HTTP(S) origin.');
  const u = new URL(value);
  if (!['http:', 'https:'].includes(u.protocol) || u.username || u.password || u.pathname !== '/' || u.search || u.hash) throw new Error('Use an HTTP(S) origin only, without credentials, path, query or fragment.');
  return u.origin;
}
export function parseFlow(text: string): BrowserFlow {
  if (new TextEncoder().encode(text).length > 65536) throw new Error('Scenario exceeds 64 KiB.');
  const raw: unknown = JSON.parse(text);
  if (!object(raw) || raw.version !== 1 || Object.keys(raw).some(k => !['version', 'origin', 'steps'].includes(k)) || typeof raw.origin !== 'string') throw new Error('Invalid scenario version or fields.');
  const origin = reviewOrigin(raw.origin);
  if (!Array.isArray(raw.steps) || !raw.steps.length || raw.steps.length > 32) throw new Error('A scenario needs 1 to 32 steps.');
  const steps: FlowStep[] = raw.steps.map((s: unknown) => {
    if (!object(s) || typeof s.op !== 'string' || !Object.hasOwn(fields, s.op) || Object.keys(s).some(k => k !== 'op' && !fields[s.op as string].includes(k))) throw new Error('Unknown scenario operation or fields.');
    for (const k of fields[s.op]) {
      if (k === 'timeoutMs') continue;
      if (!(k in s)) throw new Error(`Missing ${k} for ${s.op}.`);
    }
    for (const k of ['value', 'selector']) if (k in s && (typeof s[k] !== 'string' || s[k].length > 2048 || s[k].includes('\0') || k === 'selector' && !s[k].trim())) throw new Error('Invalid scenario text or selector.');
    if (s.op.startsWith('expect_') && 'value' in s && !String(s.value).trim() || s.op === 'wait_text' && !String(s.value).trim()) throw new Error('Expectation cannot be empty.');
    if ('status' in s && (!Number.isInteger(s.status) || Number(s.status) < 100 || Number(s.status) > 599)) throw new Error('Expected HTTP status must be 100 to 599.');
    if ('timeoutMs' in s && (!Number.isInteger(s.timeoutMs) || Number(s.timeoutMs) < 100 || Number(s.timeoutMs) > 5000)) throw new Error('Wait must be 100 to 5000 ms.');
    if (s.op === 'resize' && (!Number.isInteger(s.width) || !Number.isInteger(s.height) || Number(s.width) < 320 || Number(s.width) > 1920 || Number(s.height) < 240 || Number(s.height) > 1440)) throw new Error('Viewport must be 320-1920 by 240-1440.');
    if (s.op === 'scroll' && !['up','down','top','bottom'].includes(String(s.value))) throw new Error('Invalid scroll direction.');
    if (s.op === 'check' && !['true','false'].includes(String(s.value))) throw new Error('Checkbox value must be true or false.');
    return { ...s } as FlowStep;
  });
  return { version: 1, origin, steps };
}
export const needsInteraction = (flow: BrowserFlow): boolean => flow.steps.some(s => mutating.has(s.op));
function clean(value: unknown): string { return String(value ?? '').replace(/https?:\/\/[^\s"<>]+/g, s => { try { const u = new URL(s); return u.origin + u.pathname; } catch { return '[URL]'; } }).replace(/\b((?:token|password|secret|api[_-]?key|authorization)\s*[:=]\s*)[^\s,;]+/gi, '$1[redacted]').replace(/Bearer\s+\S+/gi, 'Bearer [redacted]').slice(0, 500); }
export async function runFlow(input: BrowserFlow, host: FlowHost): Promise<FlowReport> {
  const flow = parseFlow(JSON.stringify(input));
  if (flow.origin !== reviewOrigin(host.origin)) throw new Error('Scenario origin differs from the selected browser.');
  if (needsInteraction(flow) && !host.interactive) throw new Error('Explicit interactive consent is required before any mutating step.');
  const now = host.now ?? (() => performance.now()), start = now(), delay = host.delay ?? (ms => new Promise(resolve => setTimeout(resolve, ms)));
  const report: FlowReport = { version: 1, taskId: host.taskId, origin: flow.origin, status: 'passed', elapsedMs: 0, steps: [] };
  const active = () => { try { return !host.signal?.aborted && host.active(); } catch { return false; } };
  const guard = () => { if (!active()) throw new Error('Review cancelled or task changed.'); if (now() - start >= 45000) throw new Error('Scenario deadline exceeded (45 seconds).'); };
  const checkReply = (reply: ReviewReply): ReviewReply => {
    if (!reply || reply.ok !== true) throw new Error(clean(reply?.error) || 'Browser did not return valid evidence.');
    if (!reply.url || new URL(reply.url).origin !== flow.origin) throw new Error('Browser origin changed.');
    return reply;
  };
  // A timeout stops waiting; it cannot roll back an action already dispatched to a page.
  const bounded = <T>(action: () => Promise<T>): Promise<T> => new Promise((resolve, reject) => {
    guard();
    let finished = false;
    const finish = (error: Error | undefined, value?: T) => {
      if (finished) return; finished = true; clearTimeout(timer); host.signal?.removeEventListener('abort', abort);
      if (error) reject(error); else resolve(value as T);
    };
    const abort = () => finish(new Error('Review cancelled or task changed.'));
    const timer = setTimeout(() => finish(new Error('Scenario deadline exceeded (45 seconds).')), Math.max(1, 45000 - (now() - start)));
    host.signal?.addEventListener('abort', abort, { once: true });
    if (host.signal?.aborted) { abort(); return; }
    try { action().then(value => finish(undefined, value), error => finish(error instanceof Error ? error : new Error(String(error)))); }
    catch (error) { finish(error instanceof Error ? error : new Error(String(error))); }
  });
  const review = async (request: ReviewRequest) => { guard(); const reply = await bounded(() => host.review(request)); guard(); return checkReply(reply); };
  for (let i = 0; i < flow.steps.length; i++) {
    const step = flow.steps[i];
    if (!active()) { report.status = 'cancelled'; break; }
    try {
      guard(); let detail = 'Observed operation completed.';
      const base: ReviewRequest = { op: step.op, origin: flow.origin };
      if (step.selector !== undefined) base.selector = step.selector;
      if (step.value !== undefined) base.value = step.value;
      if (step.status !== undefined) base.status = step.status;
      if (step.op === 'screenshot') {
        await review({ op: 'snapshot', origin: flow.origin });
        if (!host.capture) throw new Error('Screenshot capture is unavailable.');
        guard(); const capture = await bounded(() => host.capture!()); guard();
        if (new URL(capture.url).origin !== flow.origin) throw new Error('Browser origin changed during capture.');
        detail = 'Viewport captured; image content is not included in this text report.';
      } else if (step.op === 'resize') {
        await review({ op: 'snapshot', origin: flow.origin });
        if (!host.resize) throw new Error('Viewport resizing is unavailable.');
        guard(); const actual = await bounded(() => host.resize!(step.width!, step.height!)); guard();
        if (Math.abs(actual.width - step.width!) > 2 || Math.abs(actual.height - step.height!) > 2) throw new Error('Requested viewport does not fit the available panel. Enlarge it or use a smaller target.');
        detail = `Viewport observed at ${Math.round(actual.width)} x ${Math.round(actual.height)}.`;
      } else if (step.op === 'wait_text') {
        const until = now() + (step.timeoutMs ?? 3000); let passed = false;
        do {
          const r = await review({ op: 'expect_text', origin: flow.origin, value: step.value });
          if (r.passed === true) { passed = true; break; }
          if (now() >= until) break;
          await bounded(() => delay(Math.min(250, Math.max(1, until - now()))));
        } while (active());
        guard(); if (!passed) throw new Error('Expected text was not observed within the bounded wait.');
        detail = 'Expected text observed within the wait deadline.';
      } else if (['click','fill','select','check','focus'].includes(step.op)) {
        const s = await review({ op: 'snapshot', origin: flow.origin });
        const matches = s.elements?.filter(e => e.selector === step.selector) ?? [];
        if (matches.length !== 1 || !s.token) throw new Error('Selector must identify one element from the latest snapshot.');
        await review({ op: step.op, origin: flow.origin, token: s.token, ref: matches[0].ref, ...(step.value === undefined ? {} : { value: step.value }) });
        detail = 'DOM action dispatched once; later assertions establish the result.';
      } else {
        const result = await review(base);
        if (step.op.startsWith('expect_') && result.passed !== true) throw new Error('Expected condition was not observed; unknown is not a pass.');
        if (step.op === 'audit') detail = 'Heuristic findings collected; this step does not certify accessibility.';
      }
      report.steps.push({ index: i + 1, op: step.op, status: 'passed', detail }); host.progress?.(i + 1, flow.steps.length);
    } catch (error) {
      report.status = active() ? 'failed' : 'cancelled';
      report.steps.push({ index: i + 1, op: step.op, status: 'failed', detail: clean(error instanceof Error ? error.message : error) }); break;
    }
  }
  report.elapsedMs = Math.max(0, Math.round(now() - start)); return report;
}
export function exportReport(report: FlowReport): string {
  return JSON.stringify({ version: 1, taskId: report.taskId, origin: reviewOrigin(report.origin), status: report.status, elapsedMs: report.elapsedMs,
    steps: report.steps.slice(0, 32).map(s => ({ index: s.index, op: s.op, status: s.status, detail: clean(s.detail) })),
    scope: 'Finite in-page checks, not proof of complete UI/API correctness. No input values, response bodies or screenshot bytes included.' }, null, 2);
}
