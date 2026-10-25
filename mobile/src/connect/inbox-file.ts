// The inbox cache file (src/store/inbox-cache.ts): read once at launch, written SAVE_MS after the
// last change, deleted on sign-out and account deletion. Off the render path; a cache, so errors are dropped.
import { File, Paths } from 'expo-file-system';

const SAVE_MS = 1000;
const file = () => new File(Paths.cache, 'inbox.json');
let timer: ReturnType<typeof setTimeout> | null = null;
let epoch = 0;
let writes: Promise<void> = Promise.resolve();
let clearing: Promise<void> = Promise.resolve();

function serialize(run: () => Promise<void>): Promise<void> {
  const next = writes.then(run);
  writes = next.catch(() => undefined);
  return next;
}

export async function readInbox(): Promise<string | null> {
  const started = epoch;
  try {
    await clearing;
    if (started !== epoch) return null;
    const f = file();
    const text = f.exists ? await f.text() : null;
    return started === epoch ? text : null;
  } catch { return null; }
}

/** `get` runs when the timer fires; null skips the write (signed out, demo). */
export function saveInboxSoon(get: () => string | null) {
  const started = epoch;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    if (started !== epoch) return;
    const text = get();
    if (text === null) return;
    void serialize(async () => {
      if (started !== epoch) return;
      const f = file();
      if (!f.exists) f.create();
      await f.write(text);
    }).catch(() => undefined); // a cache: rewritten on the next change
  }, SAVE_MS);
}

/** Erasure resolves after pending native writes, while stale reads stop immediately. */
export function clearInbox(): Promise<void> {
  epoch++;
  if (timer) clearTimeout(timer);
  timer = null;
  clearing = serialize(async () => {
    const f = file();
    if (f.exists) f.delete();
  });
  return clearing;
}
