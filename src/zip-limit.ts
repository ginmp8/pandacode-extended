/** Early parser preflight. The native reader additionally counts actual inflated
 * bytes, validates CRCs and rejects ambiguous paths before returning the archive. */
export const DOCUMENT_ARCHIVE_BYTES = 64 * 1024 * 1024;
const ENTRY_BYTES = 16 * 1024 * 1024;
const ENTRIES = 2048;

export function checkUnzippedSize(bytes: ArrayBuffer, required = false): void {
  const view = new DataView(bytes);
  const zip = bytes.byteLength >= 4 && view.getUint32(0, true) === 0x04034b50;
  if (!zip && !required) return;
  const damaged = () => new Error('The document archive is damaged or unsupported.');
  let end = -1;
  for (let i = bytes.byteLength - 22; i >= Math.max(0, bytes.byteLength - 65_557); i--) {
    if (view.getUint32(i, true) === 0x06054b50 && i + 22 + view.getUint16(i + 20, true) === bytes.byteLength) { end = i; break; }
  }
  if (end < 0 || view.getUint16(end + 4, true) || view.getUint16(end + 6, true)) throw damaged();
  const entries = view.getUint16(end + 10, true), size = view.getUint32(end + 12, true);
  if (!entries || entries > ENTRIES || view.getUint16(end + 8, true) !== entries) throw damaged();
  let offset = view.getUint32(end + 16, true), total = 0;
  if (offset + size !== end) throw damaged();
  const names = new Set<string>();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  for (let n = 0; n < entries; n++) {
    if (offset + 46 > end || view.getUint32(offset, true) !== 0x02014b50) throw damaged();
    const flags = view.getUint16(offset + 8, true), method = view.getUint16(offset + 10, true);
    const packed = view.getUint32(offset + 20, true), expanded = view.getUint32(offset + 24, true);
    const nameLength = view.getUint16(offset + 28, true), local = view.getUint32(offset + 42, true);
    const next = offset + 46 + nameLength + view.getUint16(offset + 30, true) + view.getUint16(offset + 32, true);
    if (flags & 1 || ![0, 8].includes(method) || next > end || local + 30 > offset || view.getUint32(local, true) !== 0x04034b50) throw damaged();
    if (expanded > ENTRY_BYTES || (total += expanded) > DOCUMENT_ARCHIVE_BYTES) throw new Error('The document unpacks beyond the 64 MiB preview limit.');
    const start = local + 30 + view.getUint16(local + 26, true) + view.getUint16(local + 28, true);
    if (start + packed > view.getUint32(end + 16, true)) throw damaged();
    const name = decoder.decode(new Uint8Array(bytes, offset + 46, nameLength));
    if (!name || name.startsWith('/') || /[\\\0]/.test(name) || name.split('/').includes('..') || names.has(name)) throw damaged();
    names.add(name); offset = next;
  }
  if (offset !== end) throw damaged();
}
