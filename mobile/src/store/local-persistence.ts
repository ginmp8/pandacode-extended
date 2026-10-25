import { activeSnoozes, type OutboxEntry } from './inbox.ts';
import { parseOutbox } from './outbox.ts';

const KEY = 'inbox.local', OUTBOX = 'inbox.outbox';
export interface LocalData { snoozed: Record<string, number>; pins: Record<string, boolean>; outbox: OutboxEntry[] }
export interface LocalStore { get(key: string): Promise<string | null>; set(key: string, value: string): Promise<void>; remove(key: string): Promise<void> }
const empty = (): LocalData => ({ snoozed: {}, pins: {}, outbox: [] });

/** Account-bound local data with the same drain-before-erase rule as credentials. */
export class LocalPersistence {
  private owner: string | null = null;
  private generation = 0;
  private writes: Promise<void> = Promise.resolve();
  private blocked = false;
  private clearVersion = 0;
  private readonly storage: LocalStore;
  constructor(storage: LocalStore) { this.storage = storage; }
  get version() { return this.generation; }
  current(version: number) { return version === this.generation && !this.blocked; }

  account(owner: string | null): void {
    if (owner === this.owner) return;
    this.owner = owner;
    this.generation++;
  }

  async load(): Promise<LocalData | null> {
    const version = this.generation, accountId = this.owner;
    if (!accountId || this.blocked) return null;
    const [raw, rawOutbox] = await Promise.all([this.storage.get(KEY), this.storage.get(OUTBOX)]);
    if (!this.current(version)) return null;
    const read = (value: string | null) => { try { return JSON.parse(value ?? 'null'); } catch { return null; } };
    const preferences = read(raw), pending = read(rawOutbox);
    const result = empty();
    if (preferences?.accountId === accountId) {
      const record = (value: unknown) => value && typeof value === 'object' && !Array.isArray(value) ? value : {};
      result.snoozed = activeSnoozes(record(preferences.snoozed) as Record<string, number>, Date.now());
      result.pins = Object.fromEntries(Object.entries(record(preferences.pins)).filter(([, value]) => value === true));
    }
    if (pending?.accountId === accountId) result.outbox = parseOutbox(JSON.stringify(pending.entries));
    return result;
  }

  save(data: LocalData): Promise<void> {
    const version = this.generation, accountId = this.owner;
    if (!accountId) return Promise.resolve();
    const preferences = JSON.stringify({ accountId, snoozed: data.snoozed, pins: data.pins });
    const outbox = JSON.stringify({ accountId, entries: data.outbox });
    const next = this.writes.then(async () => {
      if (!this.current(version)) return;
      await Promise.all([this.storage.set(KEY, preferences), this.storage.set(OUTBOX, outbox)]);
    });
    this.writes = next.catch(() => undefined);
    return next;
  }

  clear(): Promise<void> {
    ++this.generation;
    const clearVersion = ++this.clearVersion;
    this.blocked = true;
    const next = this.writes.then(async () => {
      await Promise.all([this.storage.remove(KEY), this.storage.remove(OUTBOX)]);
      if (clearVersion === this.clearVersion) this.blocked = false;
    });
    this.writes = next.catch(() => undefined);
    return next;
  }
}
