import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
const path = new URL('../src/hardened/workflow-library.ts', import.meta.url);
async function feature() { assert.ok(existsSync(path), 'Workflow library is not implemented'); return import(path.href); }
const input = { kind: 'visual-review', goal: 'Review the settings form', scope: 'src/settings.ts', acceptance: 'Labels are present and success is visible', origin: 'http://localhost:3000' };
test('library supplies separate UI, integration, bug and code workflows without executable payloads', async () => {
 const { workflows, buildWorkflow } = await feature();
 assert.equal(new Set(workflows.map(w => w.id)).size, workflows.length);
 for (const id of ['visual-review','integration-smoke','reproduce-bug','contract-review','implementation-review','parallel-research']) {
  const recipe = buildWorkflow({ ...input, kind:id }); assert.equal(recipe.steps.length,1); assert.equal(recipe.steps[0].kind,'prompt'); assert.ok(recipe.steps[0].text.length<=4000); assert.equal(recipe.steps[0].command,undefined);
 }
});
test('workflows require bounded explicit criteria and reject malformed origins', async () => {
 const {buildWorkflow} = await feature();
 for (const patch of [{kind:'unknown'}, {goal:''}, {acceptance:''}, {scope:'x'.repeat(1001)}, {origin:'file:///tmp/app'}, {origin:'https://user:password@site.test'}]) assert.throws(()=>buildWorkflow({...input,...patch}));
 const recipe=buildWorkflow(input); assert.match(recipe.steps[0].text,/Review the settings form/); assert.match(recipe.steps[0].text,/not.*certif|not.*proof/i);
});
test('inserting a prepared workflow does not call a send, queue, permission or execution command', async () => {
 const {prepareWorkflow}=await feature(); const calls=[], inserted=[];
 const host={ current:()=>({id:'task',projectId:'project',provider:'codex',mode:'ask',archived:false,status:'idle'}), active:()=>true,
  call:async(name,args)=>{calls.push(name); if(name==='get_task')return host.current(); if(name==='list_providers')return[{id:'codex',installed:true}]; throw Error(name); },
  insert:(id,text)=>inserted.push({id,text}) };
 await prepareWorkflow(host,'task',input); assert.equal(inserted.length,1); assert.deepEqual(calls.sort(),['get_task','list_providers']); assert.equal(inserted[0].id,'task');
});
test('task switches, missing provider, remote/archived tasks and read-only browser modes block insertion', async () => {
 const {prepareWorkflow}=await feature();
 for(const patch of [{id:'other'},{archived:true},{remoteDevice:{id:'remote'}},{mode:'plan'},{status:'running'}]){
  let inserts=0; const task={id:'task',projectId:'project',provider:'codex',mode:'ask',archived:false,status:'idle',...patch};
  const host={current:()=>task,active:()=>true,call:async n=>n==='get_task'?task:[{id:'codex',installed:true}],insert:()=>{inserts++;}};
  await assert.rejects(prepareWorkflow(host,'task',input)); assert.equal(inserts,0);
 }
 let inserted=false; const task={id:'task',projectId:'project',provider:'codex',mode:'ask',status:'idle'};
 await assert.rejects(prepareWorkflow({current:()=>task,active:()=>true,call:async n=>n==='get_task'?task:[],insert:()=>{inserted=true;}},'task',input)); assert.equal(inserted,false);
});
test('recipe save checks for collisions and never overwrites a different existing recipe',async()=>{
 const {buildWorkflow,saveWorkflow}=await feature();const recipe=buildWorkflow(input);let writes=0;
 const host={current:()=>({id:'task',projectId:'project'}),active:()=>true,call:async(n)=>{if(n==='recipes_list')return[{...recipe,steps:[{kind:'prompt',text:'User-owned content'}]}];writes++;},insert:()=>{}};
 await assert.rejects(saveWorkflow(host,'task',recipe));assert.equal(writes,0);
 host.call=async(n)=>{if(n==='recipes_list')return[{...recipe,scope:'project'}];writes++;};
 assert.equal((await saveWorkflow(host,'task',recipe)).created,false); assert.equal(writes,0);
});
test('recipe save is explicit and bounded to the same local project after asynchronous reads',async()=>{
 const {buildWorkflow,saveWorkflow}=await feature();let active=true,writes=[];
 const host={current:()=>({id:'task',projectId:'project'}),active:()=>active,call:async(n,args)=>{if(n==='recipes_list'){active=false;return [];}writes.push({n,args});},insert:()=>{}};
 await assert.rejects(saveWorkflow(host,'task',buildWorkflow(input)));assert.equal(writes.length,0);
 active=true;host.call=async(n,args)=>{if(n==='recipes_list')return[];writes.push({n,args});};
 assert.equal((await saveWorkflow(host,'task',buildWorkflow(input))).created,true);assert.equal(writes[0].n,'recipes_save');assert.equal(writes[0].args.scope,'project');assert.equal(writes.length,1);
});

test('browser workflows reference the actual PandaCode tool names, not Paseo aliases', async () => {
 const {buildWorkflow}=await feature();
 const text=buildWorkflow(input).steps[0].text;
 assert.match(text,/pandacode_capture_browser/);
 assert.doesNotMatch(text,/\bscreenshot_browser\b/);
});
