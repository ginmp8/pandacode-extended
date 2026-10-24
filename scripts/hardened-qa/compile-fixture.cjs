// Development-only transpilation, no project hooks, plugins or config execution.
const fs = require('node:fs'), path = require('node:path');
const ts = require(process.env.TYPESCRIPT_PATH || 'typescript');
const [root, out] = process.argv.slice(2); if (!root || !out) throw Error('Usage: node compile-fixture.cjs ROOT OUTPUT');
const files = ['browser-flow.ts','browser-qa.ts','workflow-library.ts','workflow-panel.ts','review-panel.ts','review-actions.ts','review-model.ts'];
for(const name of files) {
 const input=path.join(root,'src/hardened',name); if(!fs.existsSync(input)) throw Error('Feature module not implemented: '+name);
 const result=ts.transpileModule(fs.readFileSync(input,'utf8'),{fileName:input,reportDiagnostics:true,compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}});
 const errors=(result.diagnostics||[]).filter(d=>d.category===ts.DiagnosticCategory.Error);if(errors.length)throw Error(ts.formatDiagnosticsWithColorAndContext(errors,{getCanonicalFileName:s=>s,getCurrentDirectory:()=>root,getNewLine:()=> '\n'}));
 const target=path.join(out,'src/hardened',name.replace(/\.ts$/,'.js'));fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,result.outputText.replace(/^import\s+['"][^'"]+\.css['"];?\s*$/gm,'').replace(/(from\s+['"]\.[^'"]+)\.ts(['"])/g,'$1.js$2'));
}
for (const name of ['team-mode','worker-scheduling']) {
 const input=path.join(root,'src',name+'.ts'),target=path.join(out,'src',name+'.js');
 fs.writeFileSync(target,ts.transpileModule(fs.readFileSync(input,'utf8'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText);
}
for(const name of ['browser-qa.css','review-tools.css'])fs.copyFileSync(path.join(root,'src/hardened',name),path.join(out,'src/hardened',name));
console.log('Compiled reviewed fixture modules; this is not a complete app build.');
