"""Real new UI modules with an explicit IPC boundary double; no native execution."""
import base64, functools, json, os, pathlib, re, sys, unittest
from playwright.sync_api import sync_playwright
ROOT=pathlib.Path(sys.argv[1]).resolve(); OUT=pathlib.Path(sys.argv[2]).resolve();OUT.mkdir(parents=True,exist_ok=True)
@functools.lru_cache(maxsize=64)
def module(path):
    p=pathlib.Path(path).resolve()
    if p.suffix == '.ts': p=p.with_suffix('.js')
    if not p.suffix: p=p.with_suffix('.js')
    if ROOT not in p.parents: raise ValueError('Module outside fixture')
    code=p.read_text()
    code=re.sub(r"(from\s+['\"])(\.[^'\"]+)(['\"])",lambda m:m[1]+module(str(p.parent/m[2]))+m[3],code)
    return 'data:text/javascript;base64,'+base64.b64encode(code.encode()).decode()
HTML='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Hardened QA UI fixture</title><style>
:root{--text:#e7e5df;--muted:#b5b1a8;--panel:#20211f;--bg:#171816;--border:#4b4d47;--accent:#e2ca82;--focus:#e2ca82;--on-accent:#151512;--danger:#f0a599;--mono:monospace}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.45 system-ui}main{max-width:1100px;margin:20px auto;padding:16px;background:var(--panel);border:1px solid var(--border);border-radius:8px}button,input,select,textarea{font:inherit;color:inherit}button{cursor:pointer;background:transparent;border:1px solid var(--border);border-radius:4px;padding:8px 12px}button:disabled{opacity:.5}.primary-button{background:var(--accent);color:var(--on-accent)}input,select,textarea{background:var(--bg);border:1px solid var(--border);padding:8px;border-radius:4px;max-width:100%}h1{font-size:22px}#browser{display:flex;flex-direction:column;height:800px;container-type:inline-size;container-name:browser-panel}#workflow{padding:12px}#guest{border:1px dashed var(--border);flex:1;padding:20px;min-height:80px;color:var(--muted)}[hidden]{display:none!important}
__CSS__</style><main><h1>Browser QA</h1><div id="browser"><div class="browser-location">Native browser integration boundary: test fixture</div><div id="guest">Application viewport is provided by the native PandaCode WebView in the product. This fixture tests the controls, not that WebView.</div></div></main><main id="work-main" class="hardened-review"><h1>Workflow library</h1><section id="workflow"></section></main>
<script type="module">
const {mountBrowserQA}=await import('__QA__');const {mountWorkflowLibrary}=await import('__WORK__');
const PNG='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aY1sAAAAASUVORK5CYII=';
window.fx={active:true,url:'http://localhost:3000/settings',task:{id:'task',projectId:'project',provider:'codex',mode:'ask',status:'idle'},calls:[],writes:[],inserts:[],opens:[],copied:[],captures:0,resizes:[],failCheck:false,hold:false,waiters:[],resolve:()=>{fx.hold=false;fx.waiters.splice(0).forEach(r=>r());}};
Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async t=>fx.copied.push(t)}});
const review=async a=>{fx.calls.push(structuredClone(a));if(fx.hold)await new Promise(r=>fx.waiters.push(r));let r={ok:true,url:fx.url};if(a.op==='snapshot')r={...r,token:'one-use-token',text:'Visible evidence',elements:[{selector:'#save',ref:'e1',name:'<img src=x onerror=window.unsafe=true>'},{selector:'#name',ref:'e2',name:'Name'}]};if(a.op==='audit')r.findings=[{code:'missing-name',selector:'#save',message:'Fixture issue'}];if(a.op.startsWith('expect_'))r.passed=!fx.failCheck;if(a.op==='observe_start')r.observing=true;return r;};
window.qa=mountBrowserQA(document.querySelector('#browser'),{taskId:'task',status:()=>({url:fx.url,title:'Fixture'}),active:()=>fx.active,review,capture:async()=>{fx.captures++;return{id:'shot',url:fx.url,title:'Fixture',dataUrl:PNG,width:1,height:1};},resize:async(w,h)=>{fx.resizes.push({w,h});return{width:Math.min(w||900,900),height:Math.min(h||600,600)};}});qa.toggle();
window.workflowDispose=mountWorkflowLibrary(document.querySelector('#workflow'),{current:()=>fx.task,active:()=>fx.active,call:async(c,a)=>{fx.writes.push({c,a});if(c==='get_task')return structuredClone(fx.task);if(c==='list_providers')return[{id:'codex',installed:true}];if(c==='recipes_list')return[];if(c==='recipes_save')return;throw Error('Unexpected command '+c);},insert:(id,text)=>fx.inserts.push({id,text}),open:view=>fx.opens.push(view)});
window.ready=true;
</script></html>'''
class UI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pw=sync_playwright().start();cls.browser=cls.pw.chromium.launch(executable_path=os.environ.get('CHROMIUM_EXECUTABLE','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
    @classmethod
    def tearDownClass(cls):cls.browser.close();cls.pw.stop()
    def setUp(self):
        self.ctx=self.browser.new_context(viewport={'width':1180,'height':1100},service_workers='block');self.ctx.route('**/*',lambda r:r.abort());self.page=self.ctx.new_page();self.page.set_default_timeout(2500);self.errors=[];self.page.on('pageerror',lambda e:self.errors.append(str(e)))
        html=HTML.replace('__QA__',module(str(ROOT/'src/hardened/browser-qa.js'))).replace('__WORK__',module(str(ROOT/'src/hardened/workflow-panel.js'))).replace('__CSS__','\n'.join((ROOT/'src/hardened'/f).read_text() for f in ['browser-qa.css','review-tools.css']))
        self.page.set_content(html);self.page.wait_for_function('window.ready===true');self.q=self.page.get_by_role('region',name='Browser QA',exact=True)
    def tearDown(self):self.ctx.close();self.assertEqual(self.errors,[])
    def tab(self,name):self.q.get_by_role('tab',name=name,exact=True).click()
    def button(self,name):self.q.get_by_role('button',name=name,exact=True).click()
    def scenario(self,steps):self.tab('Scenarios');self.q.get_by_label('Scenario JSON',exact=True).fill(json.dumps({'version':1,'origin':'http://localhost:3000','steps':steps}))
    def workflow(self):
        self.page.get_by_label('Workflow goal',exact=True).fill('Review settings')
        self.page.get_by_label('Files or user flow in scope',exact=True).fill('src/settings.ts')
        self.page.get_by_label('Independent expected results',exact=True).fill('The Saved message is visible')
        self.page.get_by_label('Approved site origin (browser workflows)',exact=True).fill('http://localhost:3000')
    def confirmed(self):self.page.get_by_label('I reviewed this exact scope, expectations and information to be shared',exact=True).check()
    def test_01_opening_has_no_execution_or_mutating_ipc(self):
        self.assertEqual(self.page.evaluate('fx.calls'),[]);self.assertEqual(self.page.evaluate('fx.writes'),[]);self.assertEqual(self.page.evaluate('fx.inserts'),[])
    def test_02_snapshot_is_text_not_injected_html(self):
        self.button('Snapshot');self.page.wait_for_function('fx.calls.length===1');self.assertIn('Visible evidence',self.q.locator('pre').text_content());self.assertEqual(self.q.locator('img[src=x]').count(),0);self.assertIsNone(self.page.evaluate('window.unsafe'))
    def test_03_interaction_needs_fresh_consent_and_edit_resets_it(self):
        self.button('Snapshot');self.tab('Interactions');self.button('Click');self.assertEqual(self.page.evaluate('fx.calls.length'),1)
        consent=self.q.get_by_label('I authorize this interaction on the current page',exact=True);consent.check();self.q.get_by_label('Non-secret test value',exact=True).fill('sample');self.assertFalse(consent.is_checked());consent.check();self.button('Click');self.page.wait_for_function('fx.calls.length===2');self.assertEqual(self.page.evaluate('fx.calls[1].token'),'one-use-token');self.assertFalse(consent.is_checked())
    def test_04_invalid_scenario_never_reaches_browser(self):
        self.scenario([{'op':'evaluate','value':'alert(1)'}]);self.button('Run checks');self.assertEqual(self.page.evaluate('fx.calls'),[]);self.assertIn('Unknown',self.q.get_by_role('status').text_content())
    def test_05_failed_assertion_stops_next_step(self):
        self.page.evaluate('fx.failCheck=true');self.scenario([{'op':'expect_text','value':'Saved'},{'op':'audit'}]);self.button('Run checks');self.page.wait_for_function('fx.calls.length===1');self.assertIn('failed',self.q.get_by_role('status').text_content());self.assertEqual(self.page.evaluate('fx.calls.length'),1)
    def test_06_stop_releases_pending_scenario_without_later_dispatch(self):
        self.page.evaluate('fx.hold=true');self.scenario([{'op':'snapshot'},{'op':'audit'}]);self.button('Run checks');self.page.wait_for_function('fx.waiters.length===1');self.button('Stop checks');self.page.wait_for_function('document.querySelector(".qa-status").textContent.includes("cancelled")');self.page.evaluate('fx.resolve()');self.assertEqual(self.page.evaluate('fx.calls.length'),1)
    def test_07_copy_requires_confirmation(self):
        self.button('Snapshot');self.q.locator('summary').click();self.button('Copy evidence');self.assertEqual(self.page.evaluate('fx.copied'),[]);self.q.get_by_label('I reviewed the evidence and approve copying it',exact=True).check();self.button('Copy evidence');self.assertEqual(self.page.evaluate('fx.copied.length'),1)
    def test_08_closing_discards_late_snapshot_results(self):
        self.page.evaluate('fx.hold=true');self.button('Snapshot');self.page.wait_for_function('fx.waiters.length===1');self.button('Close QA');self.page.evaluate('fx.resolve()');self.page.wait_for_timeout(50);self.assertEqual(self.page.locator('.qa-evidence pre').text_content(),'');self.assertEqual(self.page.get_by_label('Element from latest snapshot',exact=True).locator('option').count(),0)
    def test_09_captures_stay_local_and_do_not_fill_composer(self):
        self.tab('Screenshots');self.button('Capture reference');self.button('Capture current');self.assertEqual(self.page.evaluate('fx.captures'),2);self.assertEqual(self.page.evaluate('fx.inserts'),[]);self.assertEqual(self.q.locator('img:not([hidden])').count(),2)
    def test_10_resize_reports_actual_panel_dimensions(self):
        self.q.get_by_label('Viewport target',exact=True).select_option('1280x720');self.button('Apply viewport');self.assertIn('900 x 600',self.q.get_by_role('status').text_content());self.assertIn('not device emulation',self.q.get_by_role('status').text_content())
    def test_11_workflow_confirmation_and_manual_insert_only(self):
        self.workflow();self.page.get_by_role('button',name='Prepare in composer',exact=True).click();self.assertEqual(self.page.evaluate('fx.writes'),[]);self.confirmed();self.page.get_by_role('button',name='Prepare in composer',exact=True).click();self.page.wait_for_function('fx.inserts.length===1');self.assertEqual(sorted(self.page.evaluate('fx.writes.map(w=>w.c)')),['get_task','list_providers']);self.assertIn('Review settings',self.page.evaluate('fx.inserts[0].text'))
    def test_12_recipe_save_uses_original_project_store_without_queue(self):
        self.workflow();self.confirmed();self.page.get_by_role('button',name='Save project recipe',exact=True).click();self.page.wait_for_function('fx.writes.length===2');self.assertEqual(self.page.evaluate('fx.writes.map(w=>w.c)'),['recipes_list','recipes_save']);self.assertEqual(self.page.evaluate('fx.writes[1].a.scope'),'project');self.assertEqual(self.page.evaluate('fx.inserts'),[])
    def test_13_workflow_edit_resets_confirmation_and_readonly_mode_blocks_browser(self):
        self.workflow();self.confirmed();self.page.get_by_label('Workflow goal',exact=True).fill('Changed');self.assertFalse(self.page.get_by_label('I reviewed this exact scope, expectations and information to be shared',exact=True).is_checked());self.page.evaluate('fx.task.mode="plan"');self.confirmed();self.page.get_by_role('button',name='Prepare in composer',exact=True).click();self.assertEqual(self.page.evaluate('fx.inserts'),[])
    def test_14_open_links_delegate_existing_controls_only(self):
        self.page.get_by_role('button',name='Open existing browser',exact=True).click();self.page.get_by_role('button',name='Open original Recipes',exact=True).click();self.assertEqual(self.page.evaluate('fx.opens'),['browser','recipes']);self.assertEqual(self.page.evaluate('fx.writes'),[])
    def test_15_keyboard_tabs_and_responsive_layout(self):
        self.q.get_by_role('tab',name='Inspect',exact=True).focus();self.page.keyboard.press('ArrowRight');self.assertEqual(self.q.get_by_role('tab',name='Interactions',exact=True).get_attribute('aria-selected'),'true');self.page.screenshot(path=str(OUT/'browser-qa-desktop.png'),full_page=True)
        self.page.set_viewport_size({'width':390,'height':844});self.assertLessEqual(self.page.evaluate('document.documentElement.scrollWidth'),392);self.page.screenshot(path=str(OUT/'browser-qa-mobile.png'),full_page=True)
    def test_16_dispose_removes_controls_and_stale_insertions(self):
        self.page.evaluate('qa.dispose();workflowDispose()');self.assertEqual(self.page.locator('.hardened-browser-qa').count(),0);self.assertEqual(self.page.locator('#workflow').text_content(),'');self.assertEqual(self.page.evaluate('fx.inserts'),[])
if __name__=='__main__':unittest.main(argv=[sys.argv[0],*sys.argv[3:]],verbosity=2)
