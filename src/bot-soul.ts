/** Personality files are copied into the profile once, never linked or watched. */
export const MAX_SOUL_BYTES = 64 * 1024;

export async function readSoulFile(file: Pick<File, 'size' | 'arrayBuffer'>): Promise<string> {
  if (file.size > MAX_SOUL_BYTES) throw new Error('Personality must be under 64 KB. Choose a smaller soul.md.');
  const bytes = await file.arrayBuffer();
  if (bytes.byteLength > MAX_SOUL_BYTES) throw new Error('Personality must be under 64 KB. Choose a smaller soul.md.');
  try { return new TextDecoder('utf-8', { fatal: true }).decode(bytes).replace(/^\uFEFF/, ''); }
  catch { throw new Error('Choose a UTF-8 text or Markdown personality file.'); }
}
