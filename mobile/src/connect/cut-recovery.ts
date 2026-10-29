import type { Message } from './types.ts';

/** Restore the most recent cut message first, so the visible answer is not left truncated. */
export function newestCutId(messages: readonly Pick<Message, 'id' | 'cut'>[] | undefined): number | undefined {
  if (!messages) return undefined;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].cut) return messages[i].id;
  }
  return undefined;
}
