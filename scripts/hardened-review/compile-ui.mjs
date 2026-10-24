// Trusted syntax-only transformation: never load project configuration or execute source.
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
const [source, output] = process.argv.slice(2);
if (!source || !output) throw new Error('Usage: node compile-ui.mjs ABSOLUTE_SOURCE_ROOT OUTPUT_DIR');
const require = createRequire(join(source, 'package.json'));
const ts = require('typescript');
const files = ['src/hardened/review-model.ts', 'src/hardened/review-actions.ts', 'src/hardened/review-panel.ts', 'src/hardened/workflow-panel.ts', 'src/hardened/workflow-library.ts', 'src/hardened/browser-flow.ts', 'src/worker-scheduling.ts', 'src/team-mode.ts'];
let errors = 0;
for (const file of files) {
  if (!existsSync(join(source, file))) continue;
  const raw = readFileSync(join(source, file), 'utf8');
  const emitted = ts.transpileModule(raw, { fileName: file, reportDiagnostics: true, compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
  for (const d of emitted.diagnostics ?? []) { if (d.category === ts.DiagnosticCategory.Error) { console.error(file, ts.flattenDiagnosticMessageText(d.messageText, '\n')); errors++; } }
  const code = emitted.outputText.replace(/^import ['"].*\.css['"];?\s*$/gm, '')
    .replace(/(from\s+['"])(\.[^'"]+)(['"])/g, (_, a, b, c) => a + (b.endsWith('.ts') ? b.slice(0, -3) + '.js' : b + (/\.\w+$/.test(b) ? '' : '.js')) + c);
  const target = join(output, file.replace(/\.ts$/, '.js')); mkdirSync(dirname(target), { recursive: true }); writeFileSync(target, code);
}
if (existsSync(join(source, 'src/hardened/review-tools.css'))) {
  mkdirSync(join(output, 'src/hardened'), { recursive: true });
  writeFileSync(join(output, 'src/hardened/review-tools.css'), readFileSync(join(source, 'src/hardened/review-tools.css')));
}
if (errors) process.exitCode = 1;
console.log(JSON.stringify({ transformer: ts.version, syntaxErrors: errors, files: files.filter(f => existsSync(join(source, f))) }));
