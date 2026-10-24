import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import vm from 'node:vm';
const ts=createRequire(import.meta.url)('typescript');
// Execute the reviewed entrypoint, not main.ts module initialization. Native activity
// and browser RPC are explicit boundary doubles; this does not validate Tauri itself.
function fixture() {
 const source=readFileSync(new URL('../src/main.ts',import.meta.url),'utf8');
 const ast=ts.createSourceFile('main.ts',source,ts.ScriptTarget.ES2022,true);
 const fn=ast.statements.find(s=>ts.isFunctionDeclaration(s)&&s.name?.text==='performAgentUiAction');assert.ok(fn);
 const f={active:true,visible:true,calls:[],operations:[],result:{ok:true,url:'http://localhost:3000'},state:{sessionId:'task',inspector:true,inspectorTab:'browser'}};
 const panel={review:async args=>{f.operations.push({op:'review',args});if(f.wait)await f.wait;return f.result;},resizeReview:async(origin,width,height)=>{f.operations.push({op:'resize',origin,width,height});return{actual:{width,height}};}};
 const scope={state:f.state,browserPanel:panel,transcriptVisible:()=>f.visible,call:async(c,a)=>{assert.equal(c,'agent_ui_request_active');f.calls.push(a.requestId);return f.active;}};
 vm.createContext(scope);vm.runInContext(ts.transpileModule(fn.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText,scope,{timeout:1000});
 f.scope=scope;f.run=(action,args={})=>scope.performAgentUiAction({requestId:'request',sessionId:'task',action,args});return f;
}
test('approved foreground browser review reaches the existing owner and returns evidence',async()=>{
 const f=fixture();const result=await f.run('browser_review',{op:'audit',origin:'http://localhost:3000'});assert.equal(result,f.result);assert.equal(f.operations.length,1);assert.equal(f.operations[0].args.op,'audit');assert.ok(f.calls.length>=2);
});
test('inactive requests, foreign tasks and hidden browsers never dispatch review',async()=>{
 for(const change of [f=>f.active=false,f=>f.state.sessionId='other',f=>f.visible=false,f=>f.state.inspector=false,f=>f.state.inspectorTab='files',f=>f.scope.browserPanel=undefined]){
  const f=fixture();change(f);await assert.rejects(f.run('browser_review',{op:'snapshot'}));assert.equal(f.operations.length,0);
 }
});
test('late evidence is discarded after cancellation or browser ownership change',async()=>{
 for(const change of [f=>f.active=false,f=>f.state.sessionId='other',f=>f.scope.browserPanel={review:async()=>({})}]){
  const f=fixture();let release;f.wait=new Promise(r=>release=r);const pending=f.run('browser_review',{op:'audit'});
  while(!f.operations.length)await new Promise(r=>setImmediate(r));change(f);release();await assert.rejects(pending);assert.equal(f.operations.length,1);
 }
});
test('responsive review uses the selected panel and does not start a task or server',async()=>{
 const f=fixture();const result=await f.run('browser_viewport',{origin:'http://localhost:3000',width:390,height:640});assert.equal(result.actual.width,390);assert.equal(f.operations.length,1);assert.equal(f.operations[0].op,'resize');assert.ok(f.calls.every(x=>x==='request'));
});
