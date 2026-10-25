import { defineConfig } from 'vite';
// Keep optimized dependencies local to each checkout, including worktrees that
// share installed packages. Concurrent preview servers must not rewrite each
// other's dependency hashes.
export default defineConfig({ clearScreen: false, cacheDir: '.vite-cache', optimizeDeps: { include: ['docx-preview', 'pdfjs-dist/legacy/build/pdf.mjs', 'xlsx'] }, server: { port: 1420, strictPort: true }, build: { target: 'es2022', assetsInlineLimit: 0 } });
