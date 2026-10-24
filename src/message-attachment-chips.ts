import { escape, icon } from './dom';
import type { MessageAttachment } from './message-attachments';
const ICONS = { file: 'file', document: 'file', folder: 'folder', image: 'camera', github: 'issue', raw: 'attach' };
export const sentImageKey = (sessionId: string, messageId: number, imageId: string): string => JSON.stringify([sessionId, messageId, imageId]);
export function attachmentChips(items: MessageAttachment[], sessionId: string, messageId: number, images: Map<string, string | null>): string {
  if (!items.length) return '';
  return `<div class="message-attachments">${items.map((item, index) => {
    const key = item.imageId ? sentImageKey(sessionId, messageId, item.imageId) : '';
    const thumbnail = images.get(key);
    const lead = item.kind === 'image' && item.imageId
      ? `<img data-sent-image="${escape(key)}" ${thumbnail ? `src="${escape(thumbnail)}"` : ''} alt="" width="36" height="36"${thumbnail ? '' : ' hidden'}>${thumbnail ? '' : icon('camera')}`
      : icon(item.kind === 'github' && item.detail.startsWith('Pull') ? 'pull' : ICONS[item.kind]);
    return `<button type="button" class="prompt-chip prompt-chip-${item.kind}" data-action="open-message-attachment" data-message-session="${escape(sessionId)}" data-message-id="${messageId}" data-index="${index}" aria-label="Open ${escape(item.name)}" title="${escape(item.path || item.url || item.detail)}">${lead}<span class="prompt-chip-text"><span class="prompt-chip-name">${escape(item.name)}</span><small>${escape(item.detail)}</small></span></button>`;
  }).join('')}</div>`;
}
