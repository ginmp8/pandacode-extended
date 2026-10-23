import ts from '/opt/node/lib/node_modules/typescript/lib/typescript.js';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
for(const f of ['src/about-settings.ts','src/provider-settings.ts']) {
 const result=ts.transpileModule(readFileSync('/work/'+f,'utf8'),{fileName:f,reportDiagnostics:true,compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}});
 const errors=(result.diagnostics||[]).filter(d=>d.category===ts.DiagnosticCategory.Error);
 if(errors.length)throw new Error(JSON.stringify(errors));
 console.log('TypeScript syntax/transpile OK (not full type-check): '+f);
}
for(const f of ['scripts/release/build.mjs','scripts/release/updater-manifest.mjs','scripts/release/desktop-build.test.mjs','scripts/release/ci-startup-smoke.mjs','tests/install-policy.test.mjs','tests/open-source.test.mjs']) {
 const result=spawnSync(process.execPath,['--check','/work/'+f],{encoding:'utf8',timeout:5000});
 if(result.status!==0)throw new Error(f+': '+result.stderr);
 console.log('JavaScript syntax OK: '+f);
}
