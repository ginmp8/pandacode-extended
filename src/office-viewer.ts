// Local, read-only previews. Native admission validates workspace ownership and
// archive expansion; renderers never receive a network URL or agent credentials.
import './office-viewer.css';
import { escape } from './dom';
import { checkUnzippedSize } from './zip-limit';
import { docxStaging } from './office-safety';
import type { SheetReply, SheetPreview } from './document-sheet-worker';

export type OfficeKind = 'pdf' | 'docx' | 'sheet' | 'image';
const PDF_PAGES = 5000, CANVAS_PIXELS = 16_000_000;
const aborted = (signal?: AbortSignal) => { if (signal?.aborted) throw new DOMException('Preview closed', 'AbortError'); };

export async function renderOffice(host: HTMLElement, kind: OfficeKind, bytes: ArrayBuffer, path = '', signal?: AbortSignal): Promise<() => void> {
  aborted(signal);
  if (bytes.byteLength > 64 * 1024 * 1024) throw new Error('The file exceeds the 64 MiB preview limit.');
  host.innerHTML = '<div class="office-viewer"><div class="inspector-empty" role="status">Opening…</div></div>';
  const viewer = host.firstElementChild as HTMLElement;
  viewer.addEventListener('click', event => {
    const anchor = event.composedPath().find((target): target is HTMLAnchorElement => target instanceof HTMLAnchorElement && target.hasAttribute('href'));
    if (anchor && !/^https?:\/\//i.test(anchor.getAttribute('href') ?? '')) event.preventDefault();
  });
  if (kind === 'image') return renderImage(viewer, bytes, path, signal);
  if (kind === 'pdf') return renderPdf(viewer, bytes, signal);
  checkUnzippedSize(bytes, kind === 'docx' || /\.(?:xlsx|xlsm|ods)$/i.test(path));
  if (kind === 'docx') return renderDocx(viewer, bytes, signal);
  return renderSheet(viewer, bytes, signal);
}

async function renderPdf(viewer: HTMLElement, bytes: ArrayBuffer, signal?: AbortSignal): Promise<() => void> {
  const pdfjs = await import('pdfjs-dist/legacy/build/pdf.mjs');
  aborted(signal);
  pdfjs.GlobalWorkerOptions.workerSrc = new URL('../node_modules/pdfjs-dist/legacy/build/pdf.worker.min.mjs', import.meta.url).href;
  const task = pdfjs.getDocument({
    data: new Uint8Array(bytes.slice(0)), enableXfa: false,
    useWorkerFetch: false, useWasm: false, disableFontFace: true, useSystemFonts: true,
    maxImageSize: CANVAS_PIXELS, canvasMaxAreaInBytes: CANVAS_PIXELS * 4,
  });
  let disposed = false;
  let observer: IntersectionObserver | undefined;
  const renders = new Map<HTMLElement, { cancel(): void }>();
  const visible = new Set<HTMLElement>();
  let drawing = 0;
  const dispose = () => {
    if (disposed) return;
    disposed = true; observer?.disconnect(); visible.clear();
    for (const render of renders.values()) render.cancel();
    renders.clear(); signal?.removeEventListener('abort', dispose); void task.destroy().catch(() => {});
  };
  signal?.addEventListener('abort', dispose, { once: true });
  try {
    const doc = await task.promise;
    aborted(signal);
    const first = (await doc.getPage(1)).getViewport({ scale: 1 });
    if (![first.width, first.height].every(value => Number.isFinite(value) && value > 0)) throw new Error('Invalid PDF page dimensions.');
    const count = Math.min(doc.numPages, PDF_PAGES);
    viewer.dataset.kind = 'pdf';
    viewer.innerHTML = `<div class="office-bar">${doc.numPages.toLocaleString()} page${doc.numPages === 1 ? '' : 's'}${count < doc.numPages ? `<span>Showing the first ${count.toLocaleString()}.</span>` : ''}</div><div class="pdf-pages"></div>`;
    const pages = viewer.querySelector<HTMLElement>('.pdf-pages')!;
    async function drawPage(slot: HTMLElement): Promise<void> {
      drawing++; slot.dataset.drawing = 'true';
      try {
        const page = await doc.getPage(Number(slot.dataset.page));
        if (disposed || !visible.has(slot)) return;
        const natural = page.getViewport({ scale: 1 });
        if (![natural.width, natural.height].every(value => Number.isFinite(value) && value > 0)) throw new Error('Invalid page dimensions');
        const width = Math.max(1, Math.min(pages.clientWidth - 32, 1100));
        const viewport = page.getViewport({ scale: width / natural.width });
        const ratio = Math.min(window.devicePixelRatio || 1, Math.sqrt(CANVAS_PIXELS / (viewport.width * viewport.height)));
        const canvas = document.createElement('canvas');
        canvas.width = Math.max(1, Math.floor(viewport.width * ratio)); canvas.height = Math.max(1, Math.floor(viewport.height * ratio));
        slot.style.aspectRatio = `${natural.width} / ${natural.height}`; slot.replaceChildren(canvas);
        const render = page.render({ canvas, viewport, transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0] });
        renders.set(slot, render); await render.promise;
        if (!visible.has(slot)) { slot.replaceChildren(); page.cleanup(); }
      } catch {
        if (!disposed && visible.has(slot)) { slot.classList.add('failed'); slot.textContent = 'This page could not be rendered.'; }
      } finally { renders.delete(slot); delete slot.dataset.drawing; drawing--; pump(); }
    }
    function pump(): void {
      if (disposed) return;
      for (const slot of visible) {
        if (drawing >= 2) break;
        if (!slot.hasChildNodes() && !slot.dataset.drawing) void drawPage(slot);
      }
    }
    observer = new IntersectionObserver(entries => {
      for (const entry of entries) {
        const slot = entry.target as HTMLElement;
        if (entry.isIntersecting) visible.add(slot);
        else { visible.delete(slot); renders.get(slot)?.cancel(); slot.replaceChildren(); }
      }
      pump();
    }, { root: viewer.closest('.file-view-body'), rootMargin: '400px 0px' });
    for (let number = 1; number <= count; number++) {
      const slot = document.createElement('div'); slot.className = 'pdf-page'; slot.dataset.page = String(number);
      slot.setAttribute('aria-label', `Page ${number}`); slot.style.aspectRatio = `${first.width} / ${first.height}`;
      pages.append(slot); observer.observe(slot);
    }
    return dispose;
  } catch (error) { dispose(); throw error; }
}

async function renderDocx(viewer: HTMLElement, bytes: ArrayBuffer, signal?: AbortSignal): Promise<() => void> {
  const { parseAsync, renderDocument } = await import('docx-preview');
  aborted(signal);
  const staging = docxStaging();
  const options = { inWrapper: true, useBase64URL: true, ignoreFonts: true, ignoreLastRenderedPageBreak: true, experimental: false, renderAltChunks: false, renderComments: false, h: staging.h };
  const doc = await parseAsync(bytes, options);
  aborted(signal);
  const nodes = await renderDocument(doc, options);
  aborted(signal);
  const clean = staging.sanitize(nodes);
  viewer.dataset.kind = 'docx'; viewer.innerHTML = '<div class="docx-host"></div>';
  const shadow = viewer.firstElementChild!.attachShadow({ mode: 'open' });
  const frame = document.createElement('style');
  frame.textContent = ':host{display:block;contain:layout paint;overflow:clip}.docx-wrapper{background:transparent;padding:24px 16px}.docx-wrapper>section.docx{max-width:100%;box-sizing:border-box;margin-bottom:16px}img{max-width:100%;height:auto}';
  shadow.append(frame, clean);
  return () => shadow.replaceChildren();
}

function parseSheets(bytes: ArrayBuffer, signal?: AbortSignal): Promise<SheetPreview[]> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('./document-sheet-worker.ts', import.meta.url), { type: 'module' });
    let timer: ReturnType<typeof setTimeout>;
    const clean = () => { clearTimeout(timer); worker.terminate(); signal?.removeEventListener('abort', cancel); };
    const cancel = () => { clean(); reject(new DOMException('Preview closed', 'AbortError')); };
    if (signal?.aborted) { cancel(); return; }
    signal?.addEventListener('abort', cancel, { once: true });
    timer = setTimeout(() => { clean(); reject(new Error('The spreadsheet took too long to parse.')); }, 10_000);
    worker.onerror = () => { clean(); reject(new Error('The spreadsheet could not be parsed.')); };
    worker.onmessage = (event: MessageEvent<SheetReply>) => { clean(); if ('error' in event.data) reject(new Error(event.data.error)); else resolve(event.data.sheets); };
    const copy = bytes.slice(0); worker.postMessage(copy, [copy]);
  });
}

async function renderSheet(viewer: HTMLElement, bytes: ArrayBuffer, signal?: AbortSignal): Promise<() => void> {
  const sheets = await parseSheets(bytes, signal); aborted(signal);
  viewer.dataset.kind = 'sheet';
  const columnName = (column: number) => { let name = ''; for (let n = column + 1; n > 0; n = Math.floor((n - 1) / 26)) name = String.fromCharCode(65 + (n - 1) % 26) + name; return name; };
  const show = (index: number) => {
    const sheet = sheets[index]; if (!sheet) return;
    const head = `<tr><th></th>${Array.from({ length: sheet.columns }, (_, col) => `<th>${columnName(sheet.startColumn + col)}</th>`).join('')}</tr>`;
    const body = sheet.rows.map((row, index) => `<tr><th>${sheet.startRow + index + 1}</th>${row.map(cell => `<td>${escape(cell)}</td>`).join('')}</tr>`).join('');
    const tabs = sheets.map((sheet, i) => `<button type="button" data-sheet="${i}" aria-pressed="${i === index}">${escape(sheet.name)}</button>`).join('');
    const limit = sheet.truncated ? '<span>Showing up to the first 2,000 rows and 200 columns, within a 100,000-cell document limit.</span>' : '';
    viewer.innerHTML = `<div class="office-bar sheet-tabs" role="group" aria-label="Sheets">${tabs}${limit}</div><div class="sheet-scroll"><table class="sheet-grid" aria-label="${escape(sheet.name)}"><thead>${head}</thead><tbody>${body}</tbody></table></div>`;
  };
  viewer.addEventListener('click', event => { const tab = (event.target as Element).closest<HTMLElement>('[data-sheet]'); if (tab) show(Number(tab.dataset.sheet)); });
  show(0); return () => viewer.replaceChildren();
}

async function renderImage(viewer: HTMLElement, bytes: ArrayBuffer, path: string, signal?: AbortSignal): Promise<() => void> {
  if (bytes.byteLength > 8 * 1024 * 1024) throw new Error('Image previews are limited to 8 MiB.');
  const types: Record<string, string> = { png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp', avif: 'image/avif', bmp: 'image/bmp', ico: 'image/x-icon', svg: 'image/svg+xml' };
  const type = types[path.split('.').pop()!.toLowerCase()];
  let content: BlobPart = bytes;
  if (type === 'image/svg+xml') {
    const { default: purify } = await import('dompurify');
    const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    const clean = purify.sanitize(source, { ALLOWED_TAGS: ['svg', 'g', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon', 'text', 'tspan', 'defs', 'clipPath', 'linearGradient', 'radialGradient', 'stop', 'marker', 'title', 'desc'], FORBID_ATTR: ['style', 'href', 'xlink:href'] });
    // Disallow paint-server URLs (including escaped CSS) as well as links.
    const svg = new DOMParser().parseFromString(clean, 'image/svg+xml').documentElement;
    if (svg.localName !== 'svg') throw new Error('The SVG could not be decoded.');
    for (const node of [svg, ...svg.querySelectorAll('*')]) for (const attr of [...node.attributes]) if (/url\s*\(|\\/i.test(attr.value)) node.removeAttribute(attr.name);
    for (const dimension of ['width', 'height']) { const value = parseFloat(svg.getAttribute(dimension) ?? ''); if (!Number.isFinite(value) || value > 4096 || value <= 0) svg.setAttribute(dimension, '1024'); }
    content = new XMLSerializer().serializeToString(svg);
  }
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(new Error('The image could not be read.'));
    reader.readAsDataURL(new Blob([content], { type }));
  });
  aborted(signal);
  const image = new Image(); image.alt = path.split('/').pop() ?? path; image.src = url;
  await image.decode(); aborted(signal);
  if (image.naturalWidth * image.naturalHeight > 32_000_000) { image.src = ''; throw new Error('Image previews are limited to 32 million pixels.'); }
  viewer.dataset.kind = 'image';
  viewer.innerHTML = `<div class="office-bar sheet-tabs">${image.naturalWidth.toLocaleString()} × ${image.naturalHeight.toLocaleString()} px<button type="button" data-zoom aria-pressed="false">Actual size</button></div><div class="image-stage"></div>`;
  const zoom = viewer.querySelector<HTMLElement>('[data-zoom]')!;
  const toggle = () => zoom.setAttribute('aria-pressed', String(viewer.toggleAttribute('data-actual')));
  image.title = 'Click to toggle actual size'; image.addEventListener('click', toggle); zoom.addEventListener('click', toggle);
  viewer.querySelector('.image-stage')!.append(image);
  return () => { image.src = ''; viewer.replaceChildren(); };
}
