import { subscribe } from './bridge';
import { escape, icon } from './dom';

/* PandaCode Connect start progress (docs/remote-access.md): while `remote_start_task`
 * waits for another computer to find or clone the project, the host reports its stages
 * (`task.progress`, re-emitted here as `remote://start-progress`) and the New task shows
 * them in a strip above the composer or in the pending transcript turn. A
 * failure stays there with Retry until the next send or Dismiss. */

export type RemoteStartStage = 'connecting' | 'finding' | 'downloading' | 'preparing' | 'starting' | 'error';
/** `remote://start-progress`, validated natively (`remote/start.rs` `progress_payload`). */
export interface RemoteStartProgress { deviceId: string; startId?: string | null; repo?: string | null; stage: Exclude<RemoteStartStage, 'connecting'>; percent?: number | null; detail?: string | null }
/** `remote://start-note`, emitted when a lost start reply is recovered and linked. */
export interface RemoteStartNote { sessionId: string; note: string }
export interface RemoteStart { sessionId: string; startId: string; deviceId: string; device: string; repo: string | null; stage: RemoteStartStage; percent: number | null; detail: string | null; note?: string; retry?: () => void; onStage?: (start: RemoteStart) => void; cancelling?: boolean; waiting?: boolean; mirrorId?: string; settle?: (error?: Error) => void }

const EVENT = 'remote://start-progress';
const NOTE_EVENT = 'remote://start-note';
const STAGES: readonly RemoteStartStage[] = ['connecting', 'finding', 'downloading', 'preparing', 'starting', 'error'];
const starts = new Map<string, RemoteStart>();
let listening: Promise<void> | undefined;
let unlisten: (() => void)[] = [];
let listenerGeneration = 0;
let observer: MutationObserver | undefined;

export function startStageText(start: Pick<RemoteStart, 'stage' | 'repo' | 'percent' | 'detail'>): string {
  const repo = start.repo ?? 'the repository';
  switch (start.stage) {
    case 'connecting': return 'Connecting to computer';
    case 'finding': return start.repo ? `Finding ${repo}` : 'Finding the project';
    case 'downloading': return `Downloading ${repo}${start.percent == null ? '' : ` · ${Math.round(start.percent)}%`}`;
    case 'preparing': return 'Preparing checkout';
    case 'starting': return start.detail || 'Starting agent';
    case 'error': return `Could not start: ${start.detail || 'the other computer did not answer'}`;
  }
}

export const remoteStart = (sessionId: string): RemoteStart | undefined => starts.get(sessionId);

/** The strip above a New task's composer while it starts on another computer, or ''.
 * A start whose stages show in the transcript's turn (`onStage`) shows here only its failure. */
export function remoteStartStrip(sessionId: string): string {
  const start = starts.get(sessionId); if (!start || (start.onStage && start.stage !== 'error')) return '';
  const failed = start.stage === 'error', determinate = start.stage === 'downloading' && start.percent != null;
  const lead = failed ? icon('alert') : '<span class="support-spinner" aria-hidden="true"></span>';
  const step = failed ? '' : ` aria-description="Step ${STAGES.indexOf(start.stage) + 1} of ${STAGES.length - 1}"`;
  const bar = determinate ? `<progress class="remote-start-bar" max="100" value="${Math.round(start.percent!)}" aria-label="Download progress"></progress>` : '';
  const actions = failed ? `<button type="button" class="remote-start-retry" data-remote-start="retry">${icon('refresh')}<span>Retry</span></button><button type="button" class="remote-start-dismiss" data-remote-start="dismiss" aria-label="Dismiss" title="Dismiss">${icon('close')}</button>` : '';
  return `<div class="remote-start-strip" data-stage="${start.stage}"><span class="remote-start-status" role="${failed ? 'alert' : 'status'}"${step}>${lead}<span class="remote-start-text"><span class="remote-start-device">${escape(start.device)}</span> · ${escape(startStageText(start))}</span></span>${bar}${actions}</div>`;
}

/** Redraws the strip in place (never the composer), for the task whose prompt is shown. */
export function syncRemoteStartStrip(): void {
  const composer = document.querySelector('#composer-area #composer'), old = document.querySelector<HTMLElement>('#composer-area .remote-start-strip');
  const id = document.querySelector<HTMLTextAreaElement>('#prompt')?.dataset.sessionId;
  const markup = id ? remoteStartStrip(id) : '';
  if (old) { if (old.outerHTML !== markup) { if (markup) old.outerHTML = markup; else old.remove(); } }
  else if (markup && composer) composer.insertAdjacentHTML('beforebegin', markup);
  watch();
}

/** Keeps the strip when the composer area re-renders, only while there is one to show. */
function watch(): void {
  const area = document.querySelector('#composer-area');
  if (!starts.size || !area) { observer?.disconnect(); observer = undefined; return; }
  if (observer) return;
  observer = new MutationObserver(() => {
    const id = document.querySelector<HTMLTextAreaElement>('#prompt')?.dataset.sessionId;
    if (id && starts.has(id) !== Boolean(document.querySelector('#composer-area .remote-start-strip'))) syncRemoteStartStrip();
  });
  observer.observe(area, { childList: true });
}

/** Each attempt has its own ID, including retries of the same task. Legacy hosts
 * without an ID can update only an unambiguous active attempt on that computer. */
export function onRemoteStartProgress(event: RemoteStartProgress): void {
  if (!event || !STAGES.includes(event.stage)) return;
  const candidates = [...starts.values()].filter(item => item.deviceId === event.deviceId);
  const active = candidates.filter(item => item.stage !== 'error');
  const start = event.startId != null
    ? candidates.find(item => item.startId === event.startId)
    : active.length === 1 ? active[0] : undefined;
  if (!start || start.deviceId !== event.deviceId) return;
  // A frame that arrives late never moves a start backwards, except into an error.
  if (event.stage !== 'error' && STAGES.indexOf(event.stage) < STAGES.indexOf(start.stage)) return;
  Object.assign(start, { stage: event.stage, repo: event.repo ?? start.repo, percent: event.stage === 'downloading' ? event.percent ?? null : null, detail: event.stage === 'error' || event.stage === 'starting' ? event.detail ?? null : null });
  start.onStage?.(start);
  // A start being cancelled, or one whose reply was lost, ends with this frame.
  if (event.stage === 'error' && (start.cancelling || start.waiting)) start.settle?.(new Error(start.detail || 'the other computer did not answer'));
  syncRemoteStartStrip();
}
const waitingFor = (start: RemoteStart): boolean => start.stage === 'starting' && Boolean(start.detail?.startsWith('Waiting for'));

/** A recovered note belongs only to the remote attempt that is currently awaiting its link. */
export function onRemoteStartNote(event: RemoteStartNote): void {
  if (!event || typeof event.sessionId !== 'string' || !event.sessionId || typeof event.note !== 'string' || !event.note.trim()) return;
  const start = starts.get(event.sessionId) ?? [...starts.values()].find(item => item.mirrorId === event.sessionId);
  if (!start || !waitingFor(start)) return;
  // The link can race the lost-reply result. Remember an early note, but settle
  // only once the waiting continuation has installed its own promise.
  start.note = event.note;
  if (!start.waiting) return;
  // The host has linked this mirror. The result carries the note to sendRemote's
  // existing composer-note path, which survives the task selection/redraw.
  start.settle?.();
}

function hasActiveStart(): boolean { return [...starts.values()].some(start => start.stage !== 'error'); }

/** Subscribe only while a start is in flight and dispose both event listeners at idle. */
async function listen(): Promise<void> {
  if (unlisten.length) return;
  if (listening) return listening;
  const generation = ++listenerGeneration;
  const pending = (async () => {
    const stops: (() => void)[] = [];
    try {
      // bridge.ts has no typed overloads for these remote start events.
      const sub = subscribe as unknown as (name: string, callback: (event: any) => void) => Promise<() => void>;
      stops.push(await sub(EVENT, onRemoteStartProgress));
      stops.push(await sub(NOTE_EVENT, onRemoteStartNote));
    } catch {
      stops.forEach(stop => stop());
      if (generation === listenerGeneration) listening = undefined;
      return;
    }
    if (generation !== listenerGeneration || !hasActiveStart()) stops.forEach(stop => stop());
    else unlisten = stops;
  })();
  listening = pending;
  await pending;
}

function stopListeningIfIdle(): void {
  if (hasActiveStart()) return;
  // Invalidate a still-pending subscribe too; it will dispose its handlers on arrival.
  listenerGeneration++;
  unlisten.splice(0).forEach(stop => stop());
  listening = undefined;
}

const message = (error: unknown): string => error instanceof Error ? error.message : typeof error === 'string' ? error : 'the other computer did not answer';

/** Runs `start` (the `remote_start_task` call, given the start id to pass as `startId`)
 * with progress shown for `sessionId`. Resolves or rejects like `start`; a failure stays
 * as the strip's error, whose Retry calls `retry` (default: send the composer again).
 * With `onStage`, each stage goes to it instead of the strip, which then shows only a failure. */
export async function trackRemoteStart<T>(sessionId: string, target: { deviceId: string; name: string }, start: (startId: string) => Promise<T>, retry?: () => void, onStage?: (start: RemoteStart) => void): Promise<T> {
  const entry: RemoteStart = { sessionId, startId: crypto.randomUUID(), deviceId: target.deviceId, device: target.name, repo: null, stage: 'connecting', percent: null, detail: null, retry, onStage };
  // Settled early by a cancellation, by the error frame of a start being cancelled, and,
  // once the reply was lost, by the task attaching or failing.
  let ended: Promise<void> = new Promise((resolve, reject) => { entry.settle = error => error ? reject(error) : resolve(); });
  ended.catch(() => undefined);
  starts.set(sessionId, entry);
  syncRemoteStartStrip();
  try {
    await listen();
    if (starts.get(sessionId) !== entry) throw new Error('Start cancelled.');
    // Reuse the unique start ID for cancellation and a reply lost after host admission.
    const call = start(entry.startId);
    call.catch(() => undefined);
    const result = await Promise.race([call, ended.then(() => call)]);
    if (waitingFor(entry) && starts.get(sessionId) === entry) {
      entry.waiting = true;
      const mirror = (result as { sessionId?: unknown } | null)?.sessionId;
      if (typeof mirror === 'string') entry.mirrorId = mirror;
      ended = new Promise((resolve, reject) => { entry.settle = error => error ? reject(error) : resolve(); });
      ended.catch(() => undefined);
      // A linked host can publish its note before the lost-reply result resolves.
      if (entry.note) entry.settle?.();
      await ended;
    }
    if (starts.get(sessionId) === entry) starts.delete(sessionId);
    return entry.note && result && typeof result === 'object' && !Array.isArray(result)
      ? Object.assign({}, result, { note: entry.note }) as T
      : result;
  } catch (error) {
    if (starts.get(sessionId) === entry) {
      if (entry.cancelling) starts.delete(sessionId);
      else Object.assign(entry, { stage: 'error', percent: null, waiting: false, detail: entry.stage === 'error' && entry.detail ? entry.detail : message(error) });
    }
    throw error;
  } finally { entry.settle = undefined; stopListeningIfIdle(); syncRemoteStartStrip(); }
}

/** The task of a start whose reply was lost spoke (its first agent event): it attached. */
export function remoteStartAttached(sessionId: string): void {
  const start = starts.get(sessionId) ?? [...starts.values()].find(item => item.mirrorId === sessionId);
  if (start?.waiting) start.settle?.();
}

/** Ends a start the viewer cancelled (`remote_cancel_start` answered `cancelled: true`). */
export function cancelledRemoteStart(sessionId: string): void {
  const start = starts.get(sessionId);
  if (start?.cancelling) start.settle?.(new Error('Start cancelled.'));
}

export function clearRemoteStart(sessionId: string): void {
  const start = starts.get(sessionId);
  if (start && starts.delete(sessionId)) { start.settle?.(new Error('Start cancelled.')); stopListeningIfIdle(); syncRemoteStartStrip(); }
}

document.addEventListener('click', event => {
  const button = (event.target as Element | null)?.closest?.<HTMLElement>('[data-remote-start]');
  const id = document.querySelector<HTMLTextAreaElement>('#prompt')?.dataset.sessionId;
  if (!button || !id) return;
  event.preventDefault(); event.stopPropagation();
  const start = starts.get(id);
  clearRemoteStart(id);
  if (button.dataset.remoteStart !== 'retry') return;
  if (start?.retry) start.retry();
  else document.querySelector<HTMLFormElement>('#composer')?.requestSubmit();
});
