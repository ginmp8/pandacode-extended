/** Explicit reader choices, independent of automatic live trace choreography. */
export type DisclosureKind = 'turn' | 'group' | 'tool' | 'raw';
const STORAGE_KEY = 'pandacode.traceDisclosure.v1';
const MAX_ENTRIES = 1024;
const MAX_STORED_BYTES = 250_000;
let choices: Map<string, boolean> | undefined;

function identity(sessionId: string, kind: DisclosureKind, key: string): string | undefined {
  if (!sessionId || !key || sessionId.length > 512 || key.length > 1024) return;
  return JSON.stringify([sessionId, kind, key]);
}

function load(): Map<string, boolean> {
  if (choices) return choices;
  choices = new Map();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw || raw.length > MAX_STORED_BYTES) return choices;
    const entries: unknown = JSON.parse(raw);
    if (!Array.isArray(entries)) return choices;
    for (const entry of entries.slice(-MAX_ENTRIES)) {
      if (!Array.isArray(entry) || entry.length !== 2 || typeof entry[0] !== 'string' ||
          entry[0].length > 1600 || typeof entry[1] !== 'boolean') continue;
      choices.set(entry[0], entry[1]);
    }
  } catch { /* Storage may be blocked or contain older invalid data. */ }
  return choices;
}

export function disclosureChoice(sessionId: string, kind: DisclosureKind, key: string): boolean | undefined {
  const id = identity(sessionId, kind, key);
  return id === undefined ? undefined : load().get(id);
}

export function rememberDisclosure(sessionId: string, kind: DisclosureKind, key: string, open: boolean): void {
  const id = identity(sessionId, kind, key);
  if (id === undefined) return;
  const saved = load();
  saved.delete(id);
  saved.set(id, open);
  while (saved.size > MAX_ENTRIES) saved.delete(saved.keys().next().value!);
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify([...saved])); }
  catch { /* Preserve the in-memory choice when persistence is unavailable. */ }
}
