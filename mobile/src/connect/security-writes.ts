export const SESSION_ENDED = 'This Connect session has ended. Reopen the task after signing in.';

/** Serialize security writes with sign-out/deletion. A transition invalidates
 * queued work immediately, drains any OS write already in flight, then clears
 * storage. A failed transition remains paused until the app is reopened. */
export class SecurityWrites {
  private generation = 0;
  private paused = false;
  private tail: Promise<void> = Promise.resolve();

  get version(): number { return this.generation; }
  active(version: number): boolean { return version === this.generation && !this.paused; }

  write<T>(version: number, operation: () => Promise<T>): Promise<T> {
    const result = this.tail.then(async () => {
      if (!this.active(version)) throw new Error(SESSION_ENDED);
      const value = await operation();
      if (!this.active(version)) throw new Error(SESSION_ENDED);
      return value;
    });
    this.tail = result.then(() => undefined, () => undefined);
    return result;
  }

  transition(operation: () => Promise<void>): Promise<void> {
    const version = ++this.generation;
    this.paused = true;
    const result = this.tail.then(operation).then(() => {
      if (version === this.generation) this.paused = false;
    });
    this.tail = result.catch(() => undefined);
    return result;
  }
}
