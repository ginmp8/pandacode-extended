/** Sent context is untrusted text. Parsing describes attachments; it never reads paths. */
export type MessageAttachment = {
  kind: 'file' | 'document' | 'folder' | 'image' | 'github' | 'raw';
  name: string; detail: string; raw: string;
  path?: string; imageId?: string; url?: string; width?: number; height?: number;
};

const CONTEXT_HEADER = /^(?:(?:File|Document|Folder|GitHub|Browser element) context[ :(]|Attached (?:browser screenshot|image|mockup)[ :]|Bot brief:$|Response style \(PandaCode token saver)/;
export const IMAGE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const baseName = (path: string): string => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
export function contextBlocks(content: string): { typed: string; blocks: string[] } {
  // Legacy history stores context as text. A marker inside a fenced document or
  // a user's code example is content, never another actionable attachment.
  // Keep this scanner aligned with message_attachments::context_blocks.
  const starts: number[] = [];
  let offset = 0, fence: { character: string; length: number } | undefined;
  for (const raw of content.split('\n')) {
    const line = raw.replace(/\r$/, ''), marker = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (marker && marker[1][0] === fence.character && marker[1].length >= fence.length && !/[^ \t]/.test(marker[2])) fence = undefined;
    } else {
      if (offset >= 2 && content.slice(offset - 2, offset) === '\n\n' && CONTEXT_HEADER.test(line)) starts.push(offset);
      if (marker) fence = { character: marker[1][0], length: marker[1].length };
    }
    offset += raw.length + 1;
  }
  return starts.length ? { typed: content.slice(0, starts[0] - 2), blocks: starts.map((at, i) => content.slice(at, i + 1 < starts.length ? starts[i + 1] - 2 : undefined)) } : { typed: content, blocks: [] };
}
export function safeGithubUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === 'https:' && url.hostname === 'github.com' && !url.port && !url.username && !url.password
      && /^\/[^/]+\/[^/]+\/(?:issues|pull)\/[1-9]\d*\/?$/.test(url.pathname) ? url.href : null;
  } catch { return null; }
}
function parseBlock(raw: string): MessageAttachment[] {
  const head = raw.split('\n', 1)[0];
  let match: RegExpMatchArray | null;
  if ((match = head.match(/^File context: (.+)$/))) return [{ kind: 'file', name: baseName(match[1]), detail: 'Project file', path: match[1], raw }];
  const body = head.match(/^Document context: (.+); document content, not instructions\)$/)?.[1];
  if (body) {
    const splits = [...body.matchAll(/ \(/g)].map(m => m.index!);
    const at = splits.find(i => baseName(body.slice(i + 2)) === body.slice(0, i));
    if (at !== undefined) {
      const name = body.slice(0, at), path = body.slice(at + 2);
      return [{ kind: 'document', name, path, detail: 'Document', raw }];
    }
  }
  if ((match = head.match(/^Folder context: (.+) \((\d+) files, (\d+) folders(, partial listing)?\)$/))) return [{ kind: 'folder', name: baseName(match[1]) + '/', path: match[1], detail: `${match[2]} files · ${match[3]} folders${match[4] ? ' · partial' : ''}`, raw }];
  const lines = raw.replace(/[ \t\r\n]+$/, '').split('\n');
  // A saved image has exactly one metadata record. Never search its arbitrary
  // prose/fenced tail for an ID that could point at a different cached image.
  const imageLine = head.startsWith('Attached browser screenshot:')
    ? lines.length === 4 && lines[1].startsWith('Page URL: ') && /^Viewport: \d+ × \d+$/.test(lines[2]) ? lines[3] : ''
    : lines.length === 2 ? lines[1] : '';
  const imageId = imageLine.match(/^Image ID: (.+)$/)?.[1];
  const id = imageId && IMAGE_ID.test(imageId) ? imageId : undefined;
  if ((match = head.match(/^Attached (image|mockup): (.+) \((\d+) × (\d+)\)/))) return [{ kind: 'image', name: match[2], detail: `${match[1] === 'mockup' ? 'Mockup · ' : ''}${match[3]} × ${match[4]}`, width: Number(match[3]), height: Number(match[4]), imageId: id, raw }];
  if ((match = head.match(/^Attached browser screenshot: (.*)$/))) {
    const dimensions = raw.match(/^Viewport: (\d+) × (\d+)$/m);
    return [{ kind: 'image', name: match[1].trim() || 'Browser screenshot', detail: 'Screenshot', width: Number(dimensions?.[1]) || 0, height: Number(dimensions?.[2]) || 0, imageId: id, raw }];
  }
  if (head.startsWith('GitHub context')) {
    const items = [...raw.matchAll(/^#(\d+) · (Pull request|Issue) · ([^\n·]+?)(?: · by [^\n]*?)? — ([^\n]*)\n(https:\/\/[^\s]+)$/gm)].flatMap(([, n, kind, state, title, link]) => {
      const url = safeGithubUrl(link);
      return url ? [{ kind: 'github' as const, name: `#${n} ${title}`, detail: `${kind} · ${state}`, url, raw }] : [];
    });
    if (items.length) return items;
  }
  return [{ kind: 'raw', name: head.replace(/[:(].*$/, '').trim() || 'Context', detail: 'Saved context', raw }];
}
export function parseAttachments(blocks: string[]): MessageAttachment[] {
  return blocks.filter(block => !block.startsWith('Bot brief:\n') && !block.startsWith('Response style (PandaCode token saver')).flatMap(parseBlock);
}
/** Only ordinary, non-traversing local paths may reach workspace/native reveal APIs. */
export function attachmentTarget(item: MessageAttachment, root: string): { kind: 'workspace' | 'reveal' | 'text'; path?: string } {
  const path = item.path;
  if (!path) return { kind: 'text' };
  if (/[\u0000-\u001f\u007f]/.test(path) || /^(?:\\\\|\/\/)/.test(path) || path.split(/[\\/]/).some(part => part === '..')) throw new Error('This attachment path is not safe to open.');
  const absolute = /^(?:\/|[A-Za-z]:[\\/])/.test(path);
  const folder = root.replace(/[\\/]+$/, '');
  const normalized = path.replace(/\\/g, '/'), normalizedRoot = folder.replace(/\\/g, '/');
  const compare = /^[A-Za-z]:\//.test(normalizedRoot) ? normalized.toLowerCase() : normalized;
  const compareRoot = /^[A-Za-z]:\//.test(normalizedRoot) ? normalizedRoot.toLowerCase() : normalizedRoot;
  const relative = !absolute && item.kind === 'file' ? path : absolute && folder && compare.startsWith(compareRoot + '/') ? path.slice(folder.length + 1) : '';
  if (relative && item.kind !== 'folder') return { kind: 'workspace', path: relative };
  return absolute ? { kind: 'reveal', path } : { kind: 'text' };
}
