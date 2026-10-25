// Project icons in memory (zustand) and in a small cache file (expo-file-system), so tiles draw
// at once on the next launch. client.ts fetches them (`connect.projectIcons`); see icon-cache.ts.
import { File, Paths } from 'expo-file-system';
import { create } from 'zustand';
import { iconKey, parseIcons, type IconCache } from './icon-cache.ts';
import { useConnect } from './index.ts';

export const useIcons = create<{ cache: IconCache }>(() => ({ cache: {} }));

const file = () => new File(Paths.cache, 'project-icons.json');

/** Demo mode keeps its seeded icons out of the cache file. */
const demo = () => useConnect.getState().demo;

let loaded: Promise<void> | null = null;
let epoch = 0;
let writes: Promise<void> = Promise.resolve();
let clearing: Promise<void> = Promise.resolve();

function serialize(run: () => Promise<void>): Promise<void> {
  const next = writes.then(run);
  writes = next.catch(() => undefined);
  return next;
}

/** Reads the cache file once; entries fetched meanwhile win. */
export function loadIcons(): Promise<void> {
  if (demo()) return Promise.resolve();
  const started = epoch;
  const cleared = clearing;
  loaded ??= (async () => {
    await cleared;
    if (started !== epoch) return;
    try {
      const f = file();
      if (!f.exists) return;
      const stored = parseIcons(await f.text());
      if (started === epoch) useIcons.setState(s => ({ cache: { ...stored, ...s.cache } }));
    } catch { /* unreadable: start empty */ }
  })();
  return loaded;
}

export async function saveIcons(): Promise<void> {
  if (demo()) return;
  const started = epoch;
  try {
    await serialize(async () => {
      if (started !== epoch || demo()) return;
      const f = file();
      if (!f.exists) f.create();
      await f.write(JSON.stringify(useIcons.getState().cache));
    });
  } catch { /* a cache: the next fetch fills it again */ }
}

/** Forgets the icons in memory (leaving demo mode); the next load reads the file again. */
export function resetIcons() {
  epoch++;
  loaded = null;
  useIcons.setState({ cache: {} });
}

/** Account deletion: memory and the cache file. */
export async function clearIcons(): Promise<void> {
  resetIcons();
  clearing = serialize(async () => {
    const f = file();
    if (f.exists) f.delete();
  });
  await clearing;
}

/** The project's icon as a data: URL, or null for the coloured initial. */
export const useProjectIcon = (deviceId: string | undefined, projectId: string | undefined) =>
  useIcons(s => (deviceId && projectId ? s.cache[iconKey(deviceId, projectId)]?.url ?? null : null));
