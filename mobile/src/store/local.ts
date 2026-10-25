// This phone's own inbox state: snoozes, phone-only pins and the outbox. Kept in SecureStore
// (the app's only persistent store), separate from the Connect state client.ts owns.
import { useEffect } from 'react';
import { create } from 'zustand';
import { connect } from '../connect/client.ts';
import { uuid } from '../connect/crypto.ts';
import { useShallow } from 'zustand/react/shallow';
import { get, remove, set } from '../connect/secrets.ts';
import { activeSnoozes, type OutboxEntry } from './inbox.ts';
import { useConnect } from './index.ts';
import { applyFlush, flush, isUnsent } from './outbox.ts';
import { LocalPersistence } from './local-persistence.ts';
import { trusted } from '../connect/trust.ts';

interface LocalState {
  loaded: boolean;
  snoozed: Record<string, number>;
  pins: Record<string, boolean>;
  outbox: OutboxEntry[];
  /** Prompts this phone steered (trimmed text → when), to label their bubbles. Memory only:
   * stored messages don't mark steers. */
  steers: Record<string, number>;
}

const empty = (): LocalState => ({ loaded: false, snoozed: {}, pins: {}, outbox: [], steers: {} });
export const useLocal = create<LocalState>(empty);

/** Demo mode's seeded snoozes and outbox are never saved or sent. */
const demo = () => useConnect.getState().demo;
const persistence = new LocalPersistence({ get, set, remove });
const owner = () => { const state = useConnect.getState(); return state.signedIn && !state.demo ? state.account?.id ?? null : null; };
const syncOwner = () => persistence.account(owner());
syncOwner();
useConnect.subscribe((state, previous) => {
  if (state.account?.id === previous.account?.id && state.signedIn === previous.signedIn && state.demo === previous.demo) return;
  syncOwner();
  useLocal.setState(empty());
});
const persistenceError = (error: unknown) => useConnect.setState({ error: error instanceof Error ? `Could not save local inbox data: ${error.message}` : 'Could not save local inbox data.' });

const save = () => {
  if (demo()) return Promise.resolve();
  syncOwner();
  const { snoozed, pins, outbox } = useLocal.getState();
  return persistence.save({ snoozed, pins, outbox });
};
const patch = (update: (s: LocalState) => Partial<LocalState>) => { useLocal.setState(update); void save().catch(persistenceError); };

export async function loadLocal() {
  syncOwner();
  const version = persistence.version;
  const stored = await persistence.load();
  if (demo() || !persistence.current(version)) return;
  useLocal.setState(s => ({
    loaded: true,
    snoozed: { ...stored?.snoozed, ...s.snoozed },
    pins: { ...stored?.pins, ...s.pins },
    outbox: [...new Map([...(stored?.outbox ?? []), ...s.outbox].map(entry => [entry.id, entry])).values()],
  }));
}

export const snooze = (key: string, until: number) => patch(s => ({ snoozed: { ...activeSnoozes(s.snoozed, Date.now()), [key]: until } }));
export const unsnooze = (key: string) => patch(s => ({ snoozed: Object.fromEntries(Object.entries(s.snoozed).filter(([k]) => k !== key)) }));
export const togglePin = (key: string) => patch(s => {
  const { [key]: on, ...rest } = s.pins;
  return { pins: on ? rest : { ...rest, [key]: true } };
});
export const markSteered = (text: string) => useLocal.setState(s => ({ steers: { ...s.steers, [text.trim()]: Date.now() } }));
export const removeOutbox = (id: string) => patch(s => ({ outbox: s.outbox.filter(e => e.id !== id) }));

/** Sends a message, then copies it back: the host saves a sent prompt before it answers, but
 * otherwise the phone copies messages only at the run's end. A queued one isn't saved yet. */
async function deliver(deviceId: string, sessionId: string, text: string): Promise<{ queued: boolean }> {
  const result = await connect.sendMessage(deviceId, sessionId, text);
  if (!result.queued) void connect.loadMessages(deviceId, sessionId).catch(() => undefined);
  return result;
}

/** Sends a message; if the computer can't be reached it waits in the outbox instead. */
export async function sendOrQueue(deviceId: string, sessionId: string, text: string): Promise<{ queued: boolean; outbox: boolean }> {
  const version = persistence.version;
  // Earlier unsent messages for this task go first, so this one waits behind them.
  if (useLocal.getState().outbox.some(e => e.deviceId === deviceId && e.sessionId === sessionId)) {
    enqueue(deviceId, sessionId, text);
    return { queued: false, outbox: true };
  }
  try {
    return { ...(await deliver(deviceId, sessionId, text)), outbox: false };
  } catch (error) {
    if (!persistence.current(version)) throw new Error('The account changed before this message could be queued.');
    if (!isUnsent(error)) throw error;
    enqueue(deviceId, sessionId, text);
    return { queued: false, outbox: true };
  }
}

function enqueue(deviceId: string, sessionId: string, text: string) {
  patch(s => ({ outbox: [...s.outbox, { id: uuid(), deviceId, sessionId, text, createdAt: Date.now() }] }));
}

/** Account deletion: snoozes, pins and the outbox, in memory and stored. */
export async function clearLocal(): Promise<void> {
  useLocal.setState({ ...empty(), loaded: true });
  await persistence.clear();
}

let flushing = false;
export async function flushOutbox() {
  if (flushing || demo()) return;
  const version = persistence.version;
  flushing = true;
  try {
    const { connected } = useConnect.getState();
    const entries = useLocal.getState().outbox;
    if (!connected || !entries.length) return;
    const result = await flush(entries, {
      online: id => {
        const current = useConnect.getState();
        const device = current.devices.find(d => d.id === id);
        return persistence.current(version) && current.connected && !!device?.online && trusted(device, current.pins);
      },
      send: async entry => {
        if (!persistence.current(version)) throw new Error('The account changed before delivery.');
        // Persist an uncertain-delivery marker BEFORE sending. A crash or failed
        // acknowledgement write must not automatically resend a delivered prompt.
        useLocal.setState(s => ({ outbox: s.outbox.map(e => e.id === entry.id ? { ...e, error: 'Delivery was started. Check the task before retrying.' } : e) }));
        await save();
        if (!persistence.current(version)) throw new Error('The account changed before delivery.');
        try { return await deliver(entry.deviceId, entry.sessionId, entry.text); }
        catch (error) {
          if (persistence.current(version) && isUnsent(error)) {
            useLocal.setState(s => ({ outbox: s.outbox.map(e => e.id === entry.id ? { ...e, error: undefined } : e) }));
            await save();
          }
          throw error;
        }
      },
    });
    if (!persistence.current(version)) return;
    if (result.sent.length || Object.keys(result.failed).length) {
      patch(s => ({ outbox: applyFlush(s.outbox, result) }));
      for (const key of new Set(entries.filter(e => result.sent.includes(e.id)).map(e => `${e.deviceId}\n${e.sessionId}`))) {
        const [deviceId, sessionId] = key.split('\n');
        void connect.loadMessages(deviceId, sessionId).catch(() => undefined);
      }
    }
  } finally {
    flushing = false;
  }
}

/** Clears an entry's error and tries the outbox again. */
export function retryOutbox(id: string) {
  patch(s => ({ outbox: s.outbox.map(e => (e.id === id ? { ...e, error: undefined } : e)) }));
  void flushOutbox();
}

/** Loads local state (again after leaving demo mode), then flushes whenever the relay reconnects or a computer comes online. */
export function useOutboxFlusher() {
  const loaded = useLocal(s => s.loaded);
  const { ready, demoOn, accountId } = useConnect(useShallow(s => ({ ready: s.ready, demoOn: s.demo, accountId: s.account?.id })));
  const signal = useConnect(s => (s.connected ? s.devices.filter(d => d.online && trusted(d, s.pins)).map(d => d.id).sort().join(',') : ''));
  useEffect(() => {
    if (!ready || demoOn) return;
    useLocal.setState(empty());
    void loadLocal().catch(persistenceError);
  }, [ready, demoOn, accountId]);
  useEffect(() => { if (loaded && signal) void flushOutbox(); }, [loaded, signal]);
}
