// Attachment bytes on disk (expo-file-system): cache/attachments/<hash>.<ext> plus index.json (LRU
// order and sizes, capped at ATTACHMENT_BYTES). A download writes <hash>.part as chunks arrive, so
// a failed one resumes where it stopped; finishing renames it. Pure logic: attachments.ts.
import { Directory, File, Paths } from 'expo-file-system';
import { evict } from './thread-cache.ts';
import { ATTACHMENT_BYTES, blobName, parseBlobIndex, type BlobIndex } from './attachments.ts';

const dir = () => new Directory(Paths.cache, 'attachments');
const indexFile = () => new File(dir(), 'index.json');

let index: BlobIndex = {};
let loaded: Promise<void> | null = null;
let epoch = 0;
let writes: Promise<void> = Promise.resolve();
let clearing: Promise<void> = Promise.resolve();

function serialize<T>(run: () => Promise<T>): Promise<T> {
  const next = writes.then(run);
  writes = next.then(() => undefined, () => undefined);
  return next;
}

/** Reads the index once per launch. */
export function loadBlobs(): Promise<void> {
  const started = epoch;
  const cleared = clearing;
  loaded ??= (async () => {
    await cleared;
    if (started !== epoch) return;
    try {
      const f = indexFile();
      const stored = f.exists ? parseBlobIndex(await f.text()) : {};
      if (started === epoch) index = { ...stored, ...index };
    } catch { /* unreadable: start empty */ }
  })();
  return loaded;
}

function ensureDir() {
  if (!dir().exists) dir().create({ intermediates: true });
}

async function writeIndex() {
  ensureDir();
  const f = indexFile();
  if (!f.exists) f.create();
  await f.write(JSON.stringify(index));
}

/** The cached file's URI, marked recently used; undefined if it isn't (or no longer is) on disk. */
export function cachedBlob(key: string): string | undefined {
  const entry = index[key];
  if (!entry) return undefined;
  const f = new File(dir(), entry.file);
  if (f.exists) {
    index = { ...index, [key]: { ...entry, at: Date.now() } };
    return f.uri;
  }
  const { [key]: _, ...rest } = index;
  index = rest;
  return undefined;
}

/** Every handle belongs to this cache lifetime; an old download cannot alter a new one. */
export function partial(key: string) {
  const started = epoch;
  const f = new File(dir(), blobName(key));
  const current = () => {
    if (started !== epoch) throw new Error('The attachment cache was cleared.');
  };
  return {
    get size() { current(); return f.exists ? f.size ?? 0 : 0; },
    append(base64: string): Promise<void> {
      return serialize(async () => {
        current();
        ensureDir();
        if (!f.exists) f.create();
        await f.write(base64, { encoding: 'base64', append: true });
        current();
      });
    },
    reset(): Promise<void> {
      return serialize(async () => {
        if (started === epoch && f.exists) f.delete();
      });
    },
    /** Moves a completed download only after all chunk writes, then records its URI. */
    finish(mime: string, bytes: number): Promise<string> {
      return serialize(async () => {
        current();
        const name = blobName(key, mime);
        const target = new File(dir(), name);
        if (target.exists) target.delete();
        await f.move(target);
        current();
        const { index: kept, drop } = evict({ ...index, [key]: { file: name, at: Date.now(), bytes } }, ATTACHMENT_BYTES);
        index = kept;
        for (const file of drop) {
          try {
            const old = new File(dir(), file);
            if (old.exists) old.delete();
          } catch { /* gone already */ }
        }
        await writeIndex();
        current();
        return target.uri;
      });
    },
  };
}

/** Invalidate handles immediately; erase only after writes and moves have settled. */
export function clearBlobs(): Promise<void> {
  epoch++;
  index = {};
  loaded = null;
  clearing = serialize(async () => {
    if (dir().exists) dir().delete();
  });
  return clearing;
}
