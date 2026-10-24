import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
const file=new URL('../src/hardened/browser-flow.ts',import.meta.url);
async function feature(){assert.ok(existsSync(file),'The bounded browser flow feature is not implemented');return import(file.href);}
const plan=(steps, extra={})=>JSON.stringify({version:1,origin:'http://localhost:3000',steps,...extra});
const scope={taskId:'task-a',origin:'http://localhost:3000',interactive:false};
test('flow parser accepts explicit checks and refuses executable or unknown instructions',async()=>{
 const {parseFlow}=await feature();
 assert.equal(parseFlow(plan([{op:'expect_text',value:'Ready'}])).steps.length,1);
 for(const steps of [[{op:'evaluate',value:'process.exit()'}],[{op:'click',selector:'#save',script:'alert(1)'}],[{op:'wait_text',value:'Ready',timeoutMs:6000}],[{op:'expect_resource',value:'/api',status:0}]])assert.throws(()=>parseFlow(plan(steps)));
 for(const origin of ['file:///etc/passwd','https://user:pass@local.test','http://localhost:3000/path','http://localhost:3000?token=x'])assert.throws(()=>parseFlow(plan([{op:'snapshot'}],{origin})));
 assert.throws(()=>parseFlow(plan(Array(33).fill({op:'snapshot'}))),/32/);
 assert.throws(()=>parseFlow(plan([])),/step/i);
});
test('mutating flows require explicit consent and do not call transport when refused',async()=>{
 const {parseFlow,runFlow}=await feature();let calls=0;
 await assert.rejects(runFlow(parseFlow(plan([{op:'click',selector:'#save'}])),{...scope,review:async()=>{calls++;return{};},active:()=>true}),/consent|interactive/i);
 assert.equal(calls,0);
});
test('failed assertion stops a flow instead of reporting a successful run',async()=>{
 const {parseFlow,runFlow}=await feature();let calls=0;
 const report=await runFlow(parseFlow(plan([{op:'expect_text',value:'Ready'},{op:'expect_title',value:'Settings'}])),{...scope,active:()=>true,review:async()=>{calls++;return {ok:false,url:'http://localhost:3000/',passed:false};}});
 assert.equal(report.status,'failed');assert.equal(calls,1);assert.equal(report.steps.length,1);
});
test('task changes and explicit cancellation never dispatch the next mutation',async()=>{
 const {parseFlow,runFlow}=await feature();let active=true,calls=0;const stop=new AbortController();
 const report=await runFlow(parseFlow(plan([{op:'expect_text',value:'Ready'},{op:'click',selector:'#save'}])),{...scope,interactive:true,signal:stop.signal,active:()=>active,review:async()=>{calls++;active=false;return {ok:true,passed:true,url:'http://localhost:3000/'};}});
 assert.equal(report.status,'cancelled');assert.equal(calls,1);
 stop.abort();calls=0;assert.equal((await runFlow(parseFlow(plan([{op:'snapshot'}])),{...scope,active:()=>true,signal:stop.signal,review:async()=>{calls++;return{};}})).status,'cancelled');assert.equal(calls,0);
});
test('action refs are obtained from fresh snapshots and a stale failure is not retried',async()=>{
 const {parseFlow,runFlow}=await feature();const seen=[];
 const report=await runFlow(parseFlow(plan([{op:'click',selector:'#save'}])),{...scope,interactive:true,active:()=>true,review:async r=>{seen.push(r);if(r.op==='snapshot')return{ok:true,url:'http://localhost:3000/',token:'snapshot-token',elements:[{selector:'#save',ref:'e1'}]};throw new Error('Snapshot is stale');}});
 assert.equal(report.status,'failed');assert.equal(seen.length,2);assert.equal(seen[1].ref,'e1');assert.equal(seen[1].token,'snapshot-token');
});
test('flows refuse changed origins even when a preceding result otherwise passed',async()=>{
 const {parseFlow,runFlow}=await feature();let calls=0;
 const report=await runFlow(parseFlow(plan([{op:'expect_text',value:'Ready'},{op:'click',selector:'#save'}])),{...scope,interactive:true,active:()=>true,review:async()=>{calls++;return{ok:true,passed:true,url:'https://other.test/'};}});
 assert.equal(report.status,'failed');assert.equal(calls,1);
});
test('flow export contains summaries, not input values, raw network bodies or query secrets',async()=>{
 const {parseFlow,runFlow,exportReport}=await feature();
 const report=await runFlow(parseFlow(plan([{op:'expect_text',value:'user-private'}])),{...scope,active:()=>true,review:async()=>({ok:true,passed:true,url:'http://localhost:3000/?secret=private',body:'never-export'})});
 const text=exportReport(report);assert.equal(report.status,'passed');for(const secret of ['user-private','secret=','never-export'])assert.ok(!text.includes(secret));
});
test('resource expectations require a real known matching HTTP status and no API request is synthesized',async()=>{
 const {parseFlow,runFlow}=await feature();const seen=[];
 const report=await runFlow(parseFlow(plan([{op:'expect_resource',value:'/api/items',status:200}])),{...scope,active:()=>true,review:async r=>{seen.push(r);return{ok:true,passed:false,url:scope.origin};}});
 assert.equal(report.status,'failed');assert.equal(seen[0].status,200);assert.equal(seen[0].op,'expect_resource');
});
test('waiting uses bounded reads and honours cancellation without busy looping',async()=>{
 const {parseFlow,runFlow}=await feature();let now=0,calls=0;
 const report=await runFlow(parseFlow(plan([{op:'wait_text',value:'Ready',timeoutMs:500}])),{...scope,active:()=>true,now:()=>now,delay:async ms=>{now+=ms;},review:async()=>{calls++;return{ok:true,passed:false,url:scope.origin};}});
 assert.equal(report.status,'failed');assert.ok(calls>=2 && calls<=4);assert.ok(now<=750);
});

test('abort releases a stalled host call without dispatching another step', async () => {
  const {parseFlow,runFlow}=await feature();
  const controller = new AbortController(); let calls = 0;
  const pending = runFlow(parseFlow(JSON.stringify({ version: 1, origin: 'http://localhost:3000', steps: [{ op: 'snapshot' }, { op: 'audit' }] })), {
    taskId: 'task', origin: 'http://localhost:3000', interactive: false, active: () => true, signal: controller.signal,
    review: () => { calls++; return new Promise(() => {}); },
  });
  controller.abort();
  const result = await Promise.race([pending, new Promise(resolve => setTimeout(() => resolve({ status: 'stalled' }), 100))]);
  assert.equal(result.status, 'cancelled'); assert.equal(calls, 1);
});

test('the global deadline also bounds a stalled host call', async () => {
  const {parseFlow,runFlow}=await feature();
  let clockCalls = 0;
  const result = await runFlow(parseFlow(JSON.stringify({ version: 1, origin: 'http://localhost:3000', steps: [{ op: 'snapshot' }] })), {
    taskId: 'task', origin: 'http://localhost:3000', interactive: false, active: () => true,
    now: () => ++clockCalls === 1 ? 0 : 44990,
    review: () => new Promise(() => {}),
  });
  assert.equal(result.status, 'failed'); assert.match(result.steps[0].detail, /deadline/i);
});

test('a browser disappearing while its state is read produces cancellation evidence',async()=>{
 const {parseFlow,runFlow}=await feature();let gone=false;
 const report=await runFlow(parseFlow(plan([{op:'snapshot'},{op:'audit'}])),{...scope,active:()=>{if(gone)throw Error('Browser closed');return true;},review:async()=>{gone=true;return{ok:true,url:scope.origin};}});
 assert.equal(report.status,'cancelled');assert.equal(report.steps.length,1);
});
