/* Release notes for /changelog, read at build time from upstream releases/*.md and Extended releases/extended/*.md
 * (the same files the release runbook publishes). Notes use a small subset of Markdown:
 * an optional "# Heading", paragraphs, "- " bullets (an indented "- " nests one level) with
 * wrapped continuation lines, **bold**, `code` and [links](https://…) (http/https only). The signing boilerplate repeated on every release is dropped here
 * and stated once on the page. */
import dates from '../data/release-dates.json';
import extendedMetadata from '../../../EXTENDED-RELEASE.json';

export type Item = { html: string; children: readonly string[] };
export type Block = { kind: 'paragraph'; html: string } | { kind: 'list'; items: readonly Item[] };
export type Release = { key: string; version: string; displayVersion: string; origin: 'upstream' | 'extended'; date?: string; title?: string; blocks: readonly Block[] };

const upstreamSources = import.meta.glob('../../../releases/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const extendedSources = import.meta.glob('../../../releases/extended/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const BOILERPLATE = /^This is an (?:Apple silicon )?Alpha preview\b/;

const escape = (text: string) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
export const inline = (text: string) => escape(text)
  .replace(/`([^`]+)`/g, '<code>$1</code>')
  .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
  .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');

// Sort newest date first, then numeric version; independent source keys avoid collision.
function compareReleases(a: Release, b: Release): number {
  const dateOrder = (b.date ?? '').localeCompare(a.date ?? '');
  if (dateOrder) return dateOrder;
  const left = a.version.match(/\d+/g)?.map(Number) ?? [];
  const right = b.version.match(/\d+/g)?.map(Number) ?? [];
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    const difference = (right[i] ?? 0) - (left[i] ?? 0);
    if (difference) return difference;
  }
  if (a.origin !== b.origin) return a.origin === 'extended' ? -1 : 1;
  return a.key.localeCompare(b.key);
}

/** Top-level bullets, each with its indented sub-bullets; other lines continue the last one. */
function listItems(chunk: string): Item[] {
  const raw = chunk.split('\n').reduce<{ text: string; children: string[] }[]>((items, line) => {
    const last = items.at(-1);
    if (line.startsWith('- ')) return [...items, { text: line.slice(2), children: [] }];
    if (!last) return items;
    const nested = line.match(/^\s+- (.*)$/)?.[1];
    const children = nested !== undefined ? [...last.children, nested]
      : last.children.length ? [...last.children.slice(0, -1), `${last.children.at(-1)} ${line.trim()}`] : last.children;
    const text = nested === undefined && !last.children.length ? `${last.text} ${line.trim()}` : last.text;
    return [...items.slice(0, -1), { text, children }];
  }, []);
  return raw.map(item => ({ html: inline(item.text), children: item.children.map(inline) }));
}

/** Groups lines into blocks: blank lines end a block, and so does the start of a list. */
export function parse(source: string): { title?: string; blocks: Block[] } {
  const lines = source.replace(/\r\n/g, '\n').trim().split('\n');
  const heading = lines[0]?.match(/^#\s+(.+)$/)?.[1];
  const title = heading && !/^PandaCode(?: Extended)? \d/.test(heading) ? heading : undefined;
  // A list may follow its lead-in paragraph without a blank line between them.
  const leadIn = (chunk: string): string[] => {
    const at = chunk.search(/\n- /);
    return at > 0 && !chunk.startsWith('- ') ? [chunk.slice(0, at), chunk.slice(at + 1)] : [chunk];
  };
  const chunks = (heading ? lines.slice(1) : lines).join('\n').split(/\n\s*\n/)
    .map(chunk => chunk.trim()).filter(Boolean).flatMap(leadIn);
  const blocks = chunks.flatMap((chunk): Block[] => {
    if (chunk.startsWith('- ')) return [{ kind: 'list', items: listItems(chunk) }];
    const text = chunk.replace(/\s*\n\s*/g, ' ');
    return BOILERPLATE.test(text) ? [] : [{ kind: 'paragraph', html: inline(text) }];
  });
  return { title, blocks };
}

function collect(sourceNotes: Record<string, string>, origin: Release['origin']): Release[] {
  return Object.entries(sourceNotes).map(([path, source]) => {
    const filename = path.split('/').at(-1) ?? '';
    const version = filename.endsWith('.md') ? filename.slice(0, -3) : '';
    const pattern = origin === 'upstream'
      ? /^\d+\.\d+\.\d+$/
      : /^\d+\.\d+\.\d+(?:\.\d+)?(?:-hardened(?:\.\d+)?)?$/;
    if (!pattern.test(version)) throw new Error('Unexpected release note filename: ' + path);
    const key = origin === 'extended' ? 'extended/' + version : version;
    const displayVersion = origin === 'extended' && version === extendedMetadata.extended_version
      ? extendedMetadata.display_version : version;
    return { key, version, displayVersion, origin, date: (dates as Record<string, string>)[key], ...parse(source) };
  });
}

export const releases: readonly Release[] = [
  ...collect(upstreamSources, 'upstream'),
  ...collect(extendedSources, 'extended'),
].sort(compareReleases);

export const formatDate = (date: string) => new Date(`${date}T12:00:00Z`).toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric', timeZone: 'UTC' });
