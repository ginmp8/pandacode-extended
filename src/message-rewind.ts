import type { PromptAttachment } from './attachments';
import { contextBlocks, parseAttachments } from './message-attachments.ts';

export type RewindDraft = { text: string; items: PromptAttachment[]; unavailable: string[] };
/** Restore saved snapshots, without re-reading original files or submitting a task. */
export async function prepareRewindDraft(content: string, image: (id: string) => Promise<string | null>): Promise<RewindDraft> {
  const { typed, blocks } = contextBlocks(content);
  const result: RewindDraft = { text: typed, items: [], unavailable: [] };
  let textBytes = 0, images = 0;
  for (const item of parseAttachments(blocks)) {
    const bytes = new TextEncoder().encode(item.raw).byteLength;
    if (result.items.length >= 24 || bytes > 40 * 1024 || textBytes + bytes > 512 * 1024) { result.unavailable.push(item.name); continue; }
    // A GitHub block can contain several chips, but is restored exactly once.
    if (result.items.some(previous => previous.kind === 'document' && previous.originalContext === item.raw)) continue;
    if (item.kind === 'image') {
      const thumbnail = item.imageId && images < 4 ? await image(item.imageId).catch(() => null) : null;
      if (!item.imageId || !thumbnail || !/^data:image\/(?:png|jpeg);base64,[A-Za-z0-9+/=]+$/.test(thumbnail)) { result.unavailable.push(item.name); continue; }
      images++;
      result.items.push({ kind: 'image', id: item.imageId, name: item.name, source: 'file', width: item.width || 0, height: item.height || 0, thumbnail });
    } else {
      textBytes += bytes;
      result.items.push({ kind: 'document', id: crypto.randomUUID(), name: item.name, path: item.path || '', size: bytes,
        text: '', truncated: false, method: 'history', originalContext: item.raw });
    }
  }
  return result;
}
