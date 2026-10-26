import { defineConfig } from 'vite';
// Keep optimized dependencies local to each checkout, including worktrees that
// share installed packages. Concurrent preview servers must not rewrite each
// other's dependency hashes.
export default defineConfig({ clearScreen: false, cacheDir: '.vite-cache', optimizeDeps: { include: ['docx-preview', 'pdfjs-dist/legacy/build/pdf.mjs', 'xlsx'] }, server: {
  port: 1420,
  strictPort: true,
  // Pre-transform first-open UI modules so concurrent browser tests do not wait
  // for Vite's on-demand transform waterfall. Browser imports remain lazy.
  warmup: {
    clientFiles: [
      './src/choice-menu.ts',
      './src/agent-usage.ts',
      './src/usage-ledger.ts',
      './src/usage-overview.ts',
      './src/usage-cost-chart.ts',
      './src/usage-heatmap.ts',
    ],
  },
}, build: { target: 'es2022', assetsInlineLimit: 0 } });
