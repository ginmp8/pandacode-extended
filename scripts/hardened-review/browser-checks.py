"""Standalone checks of the real new panel with a bounded native-IPC fixture.
Does not start PandaCode, agents, Tauri or a web server. Uses in-memory HTML
and data-module imports of the reviewed compiled source; network requests are aborted.
Usage: python browser-checks.py COMPILED_ROOT EVIDENCE_DIR
Requires an existing Python Playwright installation and Chromium.
"""
import base64, json, os, pathlib, re, shutil, sys, unittest
from playwright.sync_api import sync_playwright
ROOT = pathlib.Path(sys.argv[1]).resolve()
EVIDENCE = pathlib.Path(sys.argv[2]).resolve(); EVIDENCE.mkdir(parents=True, exist_ok=True)
HTML = r'''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Review panel fixture</title>
<style>:root{--text:#e7e5df;--muted:#b5b1a8;--panel:#20211f;--border:#4b4d47;--accent:#e2ca82;--focus:#e2ca82;--on-accent:#151512;--danger:#f0a599;--mono:monospace}*{box-sizing:border-box}body{margin:0;background:#171816;color:var(--text);font:14px/1.45 system-ui}main{max-width:1050px;margin:30px auto;padding:20px;background:var(--panel);border:1px solid var(--border);border-radius:10px}button,input,select,textarea{font:inherit;color:inherit}button{cursor:pointer;background:transparent;border:1px solid var(--border);border-radius:5px;padding:8px 12px}button:disabled{opacity:.5;cursor:default}.primary-button{background:var(--accent);color:var(--on-accent)}input,select,textarea{background:#171816;border:1px solid var(--border);padding:8px;border-radius:5px}h1{margin:0 0 16px;font-size:22px}</style>
<link rel="stylesheet" href="/src/hardened/review-tools.css"><main><h1>Review &amp; coordination</h1><div id="root"></div></main>
<script type="module">
const task={id:'task',projectId:'project',title:'Parser review',provider:'codex',model:'fixture-model',mode:'plan',status:'idle',archived:false,createdAt:1,updatedAt:1,lastResponseAt:null,resumeId:null,worktreePath:null,taskStage:'pending'};
const providers=[{id:'codex',name:'Codex',installed:true,path:'/fixture/codex',version:'fixture',models:[{id:'fixture-model',name:'Fixture model',description:''}]}];
const config={auto:false,rosterOnly:true,name:'Hardened advisor',workers:[{id:'advisor',name:'Advisor',provider:'codex',model:'fixture-model',reasoningEffort:null}],maxParallel:1,maxRounds:2,maxAssignments:1,workspace:'shared',summaryLimit:2048};
const team={sessionId:'task',config,status:'ready',revision:2,round:0,workers:[],updatedAt:1};
window.fx={task,team,providers,sessions:[task],teams:[team],pending:[],hasMessages:false,projectName:'Demo project',commands:[],insertions:[],opens:[],clipboard:[],profiles:[],refreshes:0};
const call=async(command,args={})=>{const f=window.fx;f.commands.push({command,args:structuredClone(args)});if(f.delay){await new Promise(r=>{f.releases??=[];f.releases.push(r);f.release=()=>f.releases.splice(0).forEach(done=>done());});}if(command==='architect_profiles')return {enabled:false,lastSetup:null,activeProfileId:null,profiles:structuredClone(f.profiles)};
if(command==='architect_save_profile'){f.profiles.push({id:'saved',name:args.name,setup:structuredClone(args.setup),updatedAt:1});return {enabled:false,lastSetup:null,activeProfileId:null,profiles:structuredClone(f.profiles)};}
if(command==='get_task')return structuredClone(f.task);if(command==='get_messages')return {messages:[],hasOlder:f.hasMessages};if(command==='architect_get')return structuredClone(f.team);throw Error('Unexpected authority: '+command);};
if(__HAS_PANEL__){const {mountReviewTools}=await import('/src/hardened/review-panel.js');window.disposeReview=mountReviewTools(document.querySelector('#root'),{
call,snapshot:()=>{const f=window.fx;return {task:f.task,team:f.team,providers:f.providers,sessions:f.sessions,teams:f.teams,pending:f.pending,hasMessages:f.hasMessages,projectName:f.projectName};},
refresh:async()=>{window.fx.refreshes++;},loadProviders:async()=>structuredClone(window.fx.providers),
openTask:async(sessionId,workerId)=>{window.fx.opens.push({sessionId,...(workerId === undefined ? {} : {workerId})});},
insertPrompt:(sessionId,text)=>{if(sessionId!==window.fx.task.id)throw Error('Task changed');window.fx.insertions.push({sessionId,text});},
copyText:async(text)=>{window.fx.clipboard.push(text);}
});}window.fixtureReady=true;
</script></html>'''

class BrowserChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pw=sync_playwright().start()
        binary=os.environ.get('CHROMIUM_EXECUTABLE') or shutil.which('chromium')
        options={'headless':True,'args':['--disable-background-networking','--disable-dev-shm-usage']}
        if binary: options['executable_path']=binary
        if os.environ.get('PANDACODE_TEST_ALLOW_NO_SANDBOX')=='1': options['args'].append('--no-sandbox')
        cls.browser=cls.pw.chromium.launch(**options)
    @classmethod
    def tearDownClass(cls):
        cls.browser.close();cls.pw.stop()
    def setUp(self):
        self.context=self.browser.new_context(viewport={'width':1180,'height':920},service_workers='block')
        self.page=self.context.new_page();self.page.set_default_timeout(3000);self.errors=[]
        self.page.on('pageerror',lambda error:self.errors.append(str(error)))
        def module_url(path):
            path=path.resolve()
            if ROOT not in path.parents: raise ValueError('Module outside compiled fixture')
            code=path.read_text()
            code=re.sub(r"(from\s+['\"])(\.[^'\"]+)(['\"])", lambda m:m[1]+module_url(path.parent/m[2])+m[3], code)
            return 'data:text/javascript;base64,'+base64.b64encode(code.encode()).decode()
        panel=ROOT/'src/hardened/review-panel.js'
        html=HTML.replace('__HAS_PANEL__','true' if panel.exists() else 'false')
        if panel.exists(): html=html.replace('/src/hardened/review-panel.js', module_url(panel))
        css=ROOT/'src/hardened/review-tools.css'
        html=html.replace('<link rel="stylesheet" href="/src/hardened/review-tools.css">','<style>'+(css.read_text() if css.exists() else '')+'</style>')
        self.context.route('**/*',lambda route:route.abort())
        self.page.set_content(html);self.page.wait_for_function('window.fixtureReady===true');self.page.wait_for_load_state('networkidle')

    def tearDown(self):
        self.context.close();self.assertEqual(self.errors,[],'Browser runtime errors')
    def tab(self,name):self.page.get_by_role('tab',name=name,exact=True).click()
    def valid_prompt(self):self.page.get_by_label('Review question',exact=True).fill('Review parser correctness without editing files.')
    def test_01_panel_mounts_without_mutating_native_state(self):
        self.assertEqual(self.page.get_by_role('tab').count(),4)
        self.assertEqual(self.page.evaluate('fx.commands'),[])
        self.assertEqual(self.page.evaluate('fx.insertions'),[])
    def test_02_save_profile_is_opt_in_and_idempotent(self):
        self.valid_prompt();self.page.get_by_role('button',name='Save team profile',exact=True).click()
        self.page.wait_for_function('fx.profiles.length===1')
        self.page.get_by_role('button',name='Save team profile',exact=True).click()
        self.page.wait_for_function('fx.commands.length===3')
        self.assertEqual(self.page.evaluate("fx.commands.filter(c=>c.command==='architect_save_profile').length"),1)
        self.assertEqual(self.page.evaluate('fx.task.mode'),'plan')
        self.assertEqual(self.page.evaluate('fx.insertions'),[])
    def test_03_insert_only_prepares_composer_text_after_native_recheck(self):
        self.valid_prompt();self.page.get_by_role('button',name='Insert prompt',exact=True).click()
        self.page.wait_for_function('fx.insertions.length===1')
        self.assertEqual(sorted(self.page.evaluate('fx.commands.map(c=>c.command)')),['architect_get','get_messages','get_task'])
        self.assertIn('parser correctness',self.page.evaluate('fx.insertions[0].text'))
    def test_04_permission_change_after_preflight_is_refused(self):
        self.valid_prompt();self.page.evaluate("fx.task.mode='full'")
        self.page.get_by_role('button',name='Insert prompt',exact=True).click()
        self.page.get_by_role('status').filter(has_text='Read only').wait_for()
        self.assertEqual(self.page.evaluate('fx.insertions'),[])
    def test_05_missing_model_never_falls_back(self):
        self.page.evaluate('fx.providers[0].models=[]')
        self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.page.wait_for_function('fx.refreshes===2')
        self.valid_prompt()
        self.assertTrue(self.page.get_by_role('button',name='Insert prompt',exact=True).is_disabled())
        self.assertTrue(self.page.get_by_role('button',name='Save team profile',exact=True).is_disabled())
    def test_06_handoff_requires_preview_confirmation_and_redacts_common_secrets(self):
        self.tab('Handoff');self.page.get_by_label('Goal',exact=True).fill('Review parser')
        self.page.get_by_label('Selected context',exact=True).fill('api_key = "DEMO_PRIVATE_VALUE"')
        copy=self.page.get_by_role('button',name='Copy handoff',exact=True)
        self.assertTrue(copy.is_disabled())
        self.page.get_by_label('I reviewed this preview for sensitive information.',exact=True).check();copy.click()
        self.page.wait_for_function('fx.clipboard.length===1')
        result=self.page.evaluate('fx.clipboard[0]');self.assertNotIn('DEMO_PRIVATE_VALUE',result);self.assertIn('REDACTED',result)
        self.page.get_by_label('Selected context',exact=True).fill('Updated context')
        self.assertTrue(copy.is_disabled())
        self.assertEqual(self.page.evaluate('fx.commands'),[])
    def test_07_attention_navigation_is_not_a_second_approval_path(self):
        self.page.evaluate("fx.pending=[{sessionId:'task',approvals:2,questions:0}]")
        self.tab('Attention');self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.page.get_by_role('button',name='Open task: Parser review',exact=True).click()
        self.assertEqual(self.page.evaluate('fx.opens'),[{'sessionId':'task'}])
        self.assertEqual(self.page.evaluate('fx.commands'),[])
    def test_08_attention_treats_worker_names_as_text_and_routes_to_parent(self):
        self.page.evaluate("fx.team.workers=[{id:'w',sessionId:'worker',name:'<img src=x onerror=alert(1)>',status:'failed',dependsOn:[],files:[],readOnly:true}]")
        self.tab('Attention');self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.assertEqual(self.page.locator('#root img').count(),0)
        self.page.get_by_role('button',name='Open worker: <img src=x onerror=alert(1)>',exact=True).click()
        self.assertEqual(self.page.evaluate('fx.opens'),[{'sessionId':'task','workerId':'w'}])
    def test_09_inspector_is_a_read_only_projection(self):
        self.page.evaluate("fx.team.workers=[{id:'w',sessionId:'worker',name:'Reviewer',status:'queued',dependsOn:['missing'],files:['src/parser.ts'],readOnly:true}]")
        self.tab('Team inspector');self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.assertIn('Missing: missing',self.page.get_by_role('table').inner_text())
        self.assertIn('Waiting for a dependency',self.page.get_by_role('table').inner_text())
        self.assertEqual(self.page.evaluate('fx.commands'),[])
    def test_10_keyboard_tabs_preserve_drafts(self):
        self.valid_prompt();self.tab('Handoff');self.page.get_by_label('Goal',exact=True).fill('Keep this draft')
        self.tab('Review presets');self.assertIn('parser',self.page.get_by_label('Review question',exact=True).input_value())
        self.page.get_by_role('tab',name='Review presets',exact=True).press('ArrowRight')
        self.assertEqual(self.page.get_by_role('tab',name='Team inspector',exact=True).get_attribute('aria-selected'),'true')
        self.tab('Handoff');self.assertEqual(self.page.get_by_label('Goal',exact=True).input_value(),'Keep this draft')
    def test_11_disposal_blocks_late_insertion_and_detached_controls(self):
        self.valid_prompt();self.page.evaluate('fx.delay=true')
        self.page.get_by_role('button',name='Insert prompt',exact=True).click()
        self.page.wait_for_function('typeof fx.release === "function"')
        self.page.evaluate('window.disposeReview(); fx.delay=false; fx.release();')
        self.page.wait_for_timeout(100)
        self.assertEqual(self.page.evaluate('fx.insertions'),[])
        self.assertEqual(self.page.locator('#root').inner_text(),'')
    def test_12_no_background_polling_and_resolved_attention_disappears(self):
        self.page.evaluate("fx.pending=[{sessionId:'task',approvals:1,questions:0}]");self.tab('Attention')
        self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.page.get_by_role('button',name='Open task: Parser review',exact=True).wait_for()
        self.page.evaluate('fx.pending=[]');self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.page.get_by_text('No attention items in the loaded snapshot.',exact=True).wait_for()
        n=self.page.evaluate('fx.refreshes');self.page.wait_for_timeout(300);self.assertEqual(self.page.evaluate('fx.refreshes'),n)
    def test_13_visual_desktop_and_compact(self):
        self.valid_prompt();self.page.screenshot(path=str(EVIDENCE/'review-desktop.png'),full_page=True)
        self.page.set_viewport_size({'width':600,'height':900});self.page.screenshot(path=str(EVIDENCE/'review-compact.png'),full_page=True)
        self.assertLessEqual(self.page.evaluate('document.documentElement.scrollWidth'),600)

if __name__=='__main__':
    unittest.main(argv=[sys.argv[0],*sys.argv[3:]],verbosity=2)
