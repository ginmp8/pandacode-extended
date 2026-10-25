// Parsing is isolated from the UI and terminated on cancellation or the deadline.
// Sheet cells are returned as text; formulas, macros, links and HTML are not run.
import { read, utils } from 'xlsx';

export type SheetPreview = { name: string; rows: string[][]; columns: number; startRow: number; startColumn: number; truncated: boolean };
export type SheetReply = { sheets: SheetPreview[] } | { error: string };
const ROWS = 2000, COLUMNS = 200, SHEETS = 32, CELLS = 100_000, CELL_CHARS = 4096;

self.onmessage = (event: MessageEvent<ArrayBuffer>) => {
  try {
    const bytes = new Uint8Array(event.data);
    if (bytes.byteLength > 64 * 1024 * 1024) throw new Error('The spreadsheet exceeds the preview limit.');
    const names = read(bytes, { type: 'array', bookSheets: true, bookProps: true }).SheetNames;
    if (!names.length || names.length > SHEETS) throw new Error('Preview supports up to 32 sheets.');
    const book = read(bytes, { type: 'array', dense: true, cellDates: true, sheetRows: ROWS + 1, cellFormula: false, cellHTML: false, bookVBA: false, sheets: names });
    let remaining = CELLS, textBytes = 0;
    const sheets = names.map(name => {
      const sheet = book.Sheets[name];
      const range = utils.decode_range(sheet['!ref'] ?? 'A1');
      const full = utils.decode_range(String(sheet['!fullref'] ?? sheet['!ref'] ?? 'A1'));
      if (![range.s.c, range.s.r, range.e.c, range.e.r].every(Number.isSafeInteger) || range.s.c < 0 || range.s.r < 0) throw new Error('The spreadsheet has an invalid range.');
      const columns = Math.min(Math.max(1, range.e.c - range.s.c + 1), COLUMNS);
      const count = Math.min(Math.max(0, range.e.r - range.s.r + 1), ROWS, Math.floor(remaining / columns));
      remaining -= count * columns;
      const rows: string[][] = [];
      for (let row = range.s.r; row < range.s.r + count; row++) {
        const values: string[] = [];
        for (let col = range.s.c; col < range.s.c + columns; col++) {
          const cell = sheet['!data']?.[row]?.[col] ?? sheet[utils.encode_cell({ r: row, c: col })];
          const value = cell ? String(utils.format_cell(cell)).slice(0, CELL_CHARS) : '';
          textBytes += value.length * 2;
          if (textBytes > 8 * 1024 * 1024) throw new Error('The spreadsheet text exceeds the preview limit.');
          values.push(value);
        }
        rows.push(values);
      }
      return { name, rows, columns, startRow: range.s.r, startColumn: range.s.c, truncated: full.e.r - full.s.r + 1 > count || full.e.c - full.s.c + 1 > columns };
    });
    self.postMessage({ sheets } satisfies SheetReply);
  } catch { self.postMessage({ error: 'The spreadsheet is damaged or exceeds the bounded preview limits.' } satisfies SheetReply); }
};
