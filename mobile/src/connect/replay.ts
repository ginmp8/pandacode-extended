import { Replay } from './rpc.ts';

export interface ReplayCheckpoint { timestamp: number; receivedAt: number }
export interface ReplayStore { read(): Promise<string | null>; write(value: string): Promise<void> }

/** A compact secure checkpoint makes previously accepted frames fail across restarts.
 * Within a launch, out-of-order frames are allowed in the normal replay window. After
 * a restart, frames at/before the prior high watermark are deliberately refused. */
export class PersistentReplay {
  private readonly replay = new Replay();
  private checkpoint: ReplayCheckpoint = { timestamp: 0, receivedAt: 0 };
  private cutoff = 0;
  private ready = false;
  private failed = false;
  private readonly store: ReplayStore;

  constructor(store: ReplayStore) { this.store = store; }

  async load(): Promise<void> {
    const raw = await this.store.read();
    if (raw !== null) {
      let value: ReplayCheckpoint;
      try { value = JSON.parse(raw); } catch { throw new Error('The saved replay checkpoint is unreadable.'); }
      if (!value || !Number.isSafeInteger(value.timestamp) || value.timestamp < 0 || !Number.isSafeInteger(value.receivedAt) || value.receivedAt < 0) {
        throw new Error('The saved replay checkpoint is invalid.');
      }
      this.checkpoint = value;
      this.cutoff = value.timestamp;
    }
    this.ready = true;
  }

  /** Caller serializes frames and revalidates the current pin after awaiting persistence. */
  async check(timestamp: unknown, nonce: unknown, now = Date.now()): Promise<boolean> {
    if (!this.ready || this.failed) throw new Error('Replay protection is unavailable. Reopen the app after repairing secure storage.');
    if (!Number.isSafeInteger(now) || now < this.checkpoint.receivedAt) return false;
    if (typeof timestamp !== 'number' || timestamp <= this.cutoff || !this.replay.check(timestamp, nonce, now)) return false;
    const next = { timestamp: Math.max(this.checkpoint.timestamp, timestamp), receivedAt: Math.max(this.checkpoint.receivedAt, now) };
    try { await this.store.write(JSON.stringify(next)); }
    catch {
      this.failed = true;
      throw new Error('Could not persist replay protection. Connect has stopped without accepting the frame.');
    }
    this.checkpoint = next;
    return true;
  }
}
