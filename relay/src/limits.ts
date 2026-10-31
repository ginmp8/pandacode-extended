export const MAX_DEVICES = 10;
export const MAX_FRAME_BYTES = 256 * 1024;
/** Preserve correlation for small throttled frames without parsing large payloads. */
export const RATE_LIMITED_PARSE_CHARS = 4096;
export const RATE_PER_SEC = 20;
export const RATE_BURST = 100;
export const TOKEN_TTL_SECONDS = 30 * 24 * 60 * 60;
export const MAX_NAME_CHARS = 64;
export const MAX_PLATFORM_CHARS = 32;
export const MAX_REF_CHARS = 64;
/** A socket with no ping answered (or no join) for this long is dead. Clients ping every 30 s. */
export const STALE_MS = 75_000;

/** Opt-in push registrations and notifications remain strictly bounded. */
export const MAX_PUSH_TOKEN_CHARS = 512;
export const MAX_PUSH_TOKENS_PER_DEVICE = 3;
export const NOTIFY_COALESCE_MS = 30_000;
export const MAX_NOTIFY_CHARS = 256;

/** Read no more than `limit` bytes, including when Content-Length is absent or false.
 * Never call request.json()/arrayBuffer() on an unbounded untrusted stream.
 */
export async function readJsonLimited(request: Request, limit: number): Promise<{ tooLarge: boolean; value: unknown | null }> {
  if (!request.body) return { tooLarge: false, value: null };
  const reader = request.body.getReader();
  const parts: Uint8Array[] = [];
  let used = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value.byteLength > limit - used) {
        void reader.cancel().catch(() => {});
        return { tooLarge: true, value: null };
      }
      used += value.byteLength;
      parts.push(value);
    }
    const bytes = new Uint8Array(used);
    let offset = 0;
    for (const part of parts) { bytes.set(part, offset); offset += part.byteLength; }
    return { tooLarge: false, value: JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: false }).decode(bytes)) };
  } catch {
    return { tooLarge: false, value: null };
  } finally {
    reader.releaseLock();
  }
}
