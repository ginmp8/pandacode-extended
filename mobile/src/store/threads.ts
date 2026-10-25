// Thread transcripts on disk (expo-file-system): one file per device+session under
// cache/threads/ plus index.json (LRU order, sizes, newest ids). Writes are debounced and run off
// the render path; demo mode never touches the disk. Pure logic: thread-cache.ts.
import { Directory, File, Paths } from 'expo-file-system';
import type { Message } from '../connect/types.ts';
import { decodeThread, encodeThread, evict, parseIndex, touch, type CacheIndex, type CachedThread } from './thread-cache.ts';
import { useConnect } from './index.ts';

const SAVE_MS = 1500;

const dir = () => new Directory(Paths.cache, 'threads');
const indexFile = () => new File(dir(), 'index.json');
const demo = () => useConnect.getState().demo;

let index: CacheIndex = {};
let loaded: Promise<void> | null = null;
let epoch = 0;
let writes: Promise<void> = Promise.resolve();
let clearing: Promise<void> = Promise.resolve();
const timers = new Map<string, ReturnType<typeof setTimeout>>();

function serialize(run: () => Promise<void>): Promise<void> {
  const next = writes.then(run);
  writes = next.catch(() => undefined);
  return next;
}

function loadIndex(): Promise<void> {
  const started = epoch;
  const cleared = clearing;
  loaded ??= (async () => {
    await cleared;
    if (started !== epoch) return;
    try {
      const f = indexFile();
      const stored = f.exists ? parseIndex(await f.text()) : {};
      if (started === epoch) index = { ...stored, ...index };
    } catch { /* unreadable: start empty */ }
  })();
  return loaded;
}

async function writeIndex() {
  if (!dir().exists) dir().create({ intermediates: true });
  const f = indexFile();
  if (!f.exists) f.create();
  await f.write(JSON.stringify(index));
}

/** The newest id cached for a thread (prefetch skips threads already current). */
export const cachedLastId = (key: string): number | undefined => index[key]?.lastId;

/** The cached thread, or null; marks it recently used. */
export async function readThread(key: string): Promise<CachedThread | null> {
  if (demo()) return null;
  const started = epoch;
  await loadIndex();
  if (started !== epoch) return null;
  const entry = index[key];
  if (!entry) return null;
  try {
    const f = new File(dir(), entry.file);
    const thread = f.exists ? decodeThread(await f.text()) : null;
    if (started !== epoch) return null;
    if (!thread) throw new Error('unreadable');
    index = touch(index, key, { at: Date.now() });
    return thread;
  } catch {
    if (started !== epoch) return null;
    const { [key]: _, ...rest } = index;
    index = rest;
    return null;
  }
}

/** Saves the thread SAVE_MS after its last change; `get` reads the messages at that moment. */
export function saveThread(key: string, get: () => { messages?: Message[]; hasOlder: boolean }) {
  if (demo()) return;
  const started = epoch;
  clearTimeout(timers.get(key));
  timers.set(key, setTimeout(() => {
    timers.delete(key);
    if (started === epoch) void write(key, get(), started);
  }, SAVE_MS));
}

async function write(key: string, { messages, hasOlder }: { messages?: Message[]; hasOlder: boolean }, started: number) {
  if (demo() || !messages?.length) return;
  try {
    await loadIndex();
    if (started !== epoch) return;
    await serialize(async () => {
      if (started !== epoch || demo()) return;
      if (!dir().exists) dir().create({ intermediates: true });
      const text = encodeThread(messages, hasOlder);
      index = touch(index, key, { at: Date.now(), bytes: text.length, lastId: messages[messages.length - 1].id });
      const f = new File(dir(), index[key].file);
      if (!f.exists) f.create();
      await f.write(text);
      if (started !== epoch) return;
      const { index: kept, drop } = evict(index);
      index = kept;
      for (const name of drop) {
        const old = new File(dir(), name);
        if (old.exists) old.delete();
      }
      await writeIndex();
    });
  } catch { /* a cache: the next copy writes it again */ }
}

/** Invalidate pending reads immediately; remove files after native writes finish. */
export function clearThreads(): Promise<void> {
  epoch++;
  timers.forEach(clearTimeout);
  timers.clear();
  index = {};
  loaded = null;
  clearing = serialize(async () => {
    if (dir().exists) dir().delete();
  });
  return clearing;
}
