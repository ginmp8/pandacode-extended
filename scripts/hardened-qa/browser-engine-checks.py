"""Reviewed local fixture only. Runs real DOM and HTTP assertions, not providers/Tauri."""
import argparse, json, pathlib, threading
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from playwright.sync_api import sync_playwright
p=argparse.ArgumentParser();p.add_argument('--root',default='.');p.add_argument('--chromium',default='/usr/bin/chromium');p.add_argument('--output',default='/tmp/panda-qa-evidence');args=p.parse_args()
root=pathlib.Path(args.root).resolve(); source=root/'src-tauri/src/browser/review.js'
assert source.is_file(), 'The in-page browser review implementation is missing'
script=source.read_text(); output=pathlib.Path(args.output);output.mkdir(parents=True,exist_ok=True)
HTML=b'''<!doctype html><html><head><title>QA fixture</title><style>body{font:16px sans-serif}button,input,select{padding:12px;margin:8px}</style></head><body><h1>Account</h1><p id="result">Ready</p><label for="name">Display name</label><input id="name" value="private-input-value"><input type="password" value="private-password"><textarea>private-textarea</textarea><div contenteditable>private-editable</div><div hidden>private-hidden</div><input id="token" type="hidden" value="private-token"><select id="choice"><option value="a">Alpha</option><option value="b">Beta</option></select><input id="flag" type="checkbox" aria-label="Notifications"><button id="save">Save</button><button id="empty"></button><img src="/broken.png"><a id="outside" href="https://other.invalid/path">External</a><script>document.querySelector('#save').onclick=async()=>{const r=await fetch('/api/items?access_token=private-query');document.querySelector('#result').textContent=(await r.json()).result;};</script></body></html>'''
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_GET(self):
  if self.path.startswith('/api/items'): code=200;data=b'{"result":"Saved"}';mime='application/json'
  elif self.path.startswith('/api/fail'): code=503;data=b'{"error":"unavailable"}';mime='application/json'
  elif self.path.startswith('/broken.png'): code=404;data=b'';mime='image/png'
  else: code=200;data=HTML;mime='text/html'
  self.send_response(code);self.send_header('Content-Type',mime);self.end_headers();self.wfile.write(data)
server=ThreadingHTTPServer(('127.0.0.1',0),Handler);threading.Thread(target=server.serve_forever,daemon=True).start();origin=f'http://127.0.0.1:{server.server_port}'
checks=[]
def record(name): checks.append(name);print('PASS',name,flush=True)
with sync_playwright() as pw:
 browser=pw.chromium.launch(executable_path=args.chromium,headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
 ctx=browser.new_context(viewport={'width':1100,'height':800});ctx.route('**/*',lambda route:route.continue_() if route.request.url.startswith(origin+'/') else route.abort())
 page=ctx.new_page();page.goto(origin);page.wait_for_load_state('networkidle')
 def run(op,**kw):return json.loads(page.evaluate(script.replace('__PANDACODE_REVIEW_ARGS__',json.dumps({'op':op,'origin':origin,**kw}))))
 def ref(snapshot,selector):return next(n['ref'] for n in snapshot['elements'] if n['selector']==selector)
 s=run('snapshot'); assert s['ok'] and s['token'] and s['elements'];raw=json.dumps(s)
 for x in ['private-input-value','private-password','private-textarea','private-editable','private-hidden','private-token']:assert x not in raw,x
 record('snapshot excludes form values, hidden and editable text')
 assert not run('evaluate',value='alert(1)')['ok'];assert not run('snapshot',origin='https://other.invalid')['ok'];record('unknown operation and origin mismatch rejected')
 page.locator('#save').evaluate('(el)=>el.setAttribute("data-changed","yes")');r=run('click',token=s['token'],ref=ref(s,'#save'));assert not r['ok'] and 'stale' in r['error'].lower();record('DOM mutation invalidates refs')
 s=run('snapshot');r=run('click',token=s['token'],ref=ref(s,'#outside'));assert not r['ok'];assert page.url==origin+'/';record('external anchor action denied')
 s=run('snapshot');assert run('fill',token=s['token'],ref=ref(s,'#name'),value='Test name')['ok'];assert page.locator('#name').input_value()=='Test name';record('fill invokes real DOM input setter')
 assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok'];record('action consumes snapshot token')
 s=run('snapshot');assert run('select',token=s['token'],ref=ref(s,'#choice'),value='b')['ok'];assert page.locator('#choice').input_value()=='b';record('select changes real option')
 s=run('snapshot');assert run('check',token=s['token'],ref=ref(s,'#flag'),value='true')['ok'];assert page.locator('#flag').is_checked();record('checkbox action changes state')
 s=run('snapshot');password=next(x for x in s['elements'] if x.get('inputType')=='password');assert not run('fill',token=s['token'],ref=password['ref'],value='no')['ok'];record('credential fields cannot be filled by review tools')
 s=run('snapshot');assert run('click',token=s['token'],ref=ref(s,'#save'))['ok'];page.wait_for_function('document.querySelector("#result").textContent==="Saved"');assert run('expect_text',value='Saved')['passed'];record('browser click executes a real frontend to HTTP backend flow')
 resources=run('resources');r=next(x for x in resources['resources'] if x['path']=='/api/items');assert r['status']==200;assert 'private-query' not in json.dumps(resources);assert '?' not in r['url'];record('network timings and HTTP status contain no query or response body')
 assert run('expect_resource',value='/api/items',status=200)['passed'];assert not run('expect_resource',value='/api/missing',status=200)['passed'];record('resource checks need observed known matching status')
 page.evaluate('fetch("/api/fail").then(r=>r.text())');page.wait_for_function('performance.getEntriesByType("resource").some(e=>e.name.includes("/api/fail"))');assert not run('expect_resource',value='/api/fail',status=200)['passed'];record('real HTTP 503 does not pass a 200 expectation')
 audit=run('audit');codes={f['code'] for f in audit['findings']};assert {'missing-language','missing-name','image-alt'}.issubset(codes);record('bounded audit reports concrete DOM issues')
 assert run('observe_start')['observing'];page.evaluate('window.dispatchEvent(new ErrorEvent("error",{message:"token=private-observation"}))');obs=run('observe_read');assert obs['errors'] and 'private-observation' not in json.dumps(obs);record('opt-in runtime errors are redacted')
 count=len(obs['errors']);run('observe_stop');page.evaluate('window.dispatchEvent(new ErrorEvent("error",{message:"after-stop"}))');assert len(run('observe_read')['errors'])==count;record('stopping observation removes handlers')
 assert not run('expect_element',selector='[')['ok'];assert not run('expect_text',value='Not present')['passed'];record('bad selectors and failed expectations are explicit')
 page.evaluate('document.body.insertAdjacentHTML("beforeend", "<div style=width:1800px>overflow</div>")');assert any(f['code']=='horizontal-overflow' for f in run('audit')['findings']);record('layout overflow is detected in real viewport')
 page.set_viewport_size({'width':390,'height':780});assert run('snapshot')['viewport']['width']==390;page.screenshot(path=str(output/'browser-qa-fixture-mobile.png'));record('responsive snapshot reflects actual viewport')
 s=run('snapshot');run('clear');assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok'];record('clear revokes retained refs and observers')
 browser.close()
server.shutdown();(output/'browser-engine-checks.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'scope':'reviewed real Chromium DOM and local HTTP fixture; not native Tauri'},indent=2));print('TOTAL',len(checks))
