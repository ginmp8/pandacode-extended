"""Reviewed in-memory DOM fixture. No live HTTP/native/provider validation is implied."""
import argparse, ast, json, pathlib
from playwright.sync_api import sync_playwright
parser = argparse.ArgumentParser()
parser.add_argument('--root', default='.')
parser.add_argument('--chromium', default='/usr/bin/chromium')
parser.add_argument('--output', default='/tmp/panda-dom-evidence')
args = parser.parse_args()
root = pathlib.Path(args.root).resolve(); output = pathlib.Path(args.output); output.mkdir(parents=True, exist_ok=True)
script = (root / 'src-tauri/src/browser/review.js').read_text()
# Read the reviewed fixture literal without importing its live-HTTP test runner.
tree = ast.parse((root / 'scripts/hardened-qa/browser-engine-checks.py').read_text())
html = next(ast.literal_eval(n.value).decode() for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'HTML' for t in n.targets))
checks = []
def passed(name): checks.append(name); print('PASS', name, flush=True)
with sync_playwright() as pw:
    browser = pw.chromium.launch(executable_path=args.chromium, headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
    context = browser.new_context(viewport={'width':1100, 'height':800})
    context.route('**/*', lambda r: r.abort())
    page = context.new_page(); page.set_content(html, wait_until='domcontentloaded')
    page.evaluate('document.querySelector("#save").onclick=()=>document.querySelector("#result").textContent="Saved locally"')
    origin = page.evaluate('location.origin')
    assert origin == 'null', 'This test is deliberately a DOM-only opaque-origin fixture.'
    def run(op, **kw): return json.loads(page.evaluate(script.replace('__PANDACODE_REVIEW_ARGS__', json.dumps({'op':op,'origin':origin,**kw}))))
    def ref(s, selector): return next(n['ref'] for n in s['elements'] if n['selector']==selector)
    s=run('snapshot'); assert s['ok'] and s['token'] and s['elements']
    for secret in ['private-input-value','private-password','private-textarea','private-editable','private-hidden','private-token']: assert secret not in json.dumps(s), secret
    passed('real DOM snapshot excludes hidden, editable and form values')
    assert not run('evaluate',value='alert(1)')['ok']; assert not run('snapshot',origin='https://other.invalid')['ok']; passed('closed operations and origin identity reject invalid calls')
    page.locator('#save').evaluate('(el)=>el.setAttribute("data-changed","yes")')
    assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok']; passed('DOM mutations invalidate snapshot refs')
    s=run('snapshot'); assert not run('click',token=s['token'],ref=ref(s,'#outside'))['ok']; passed('external-anchor interaction is refused')
    s=run('snapshot'); assert run('fill',token=s['token'],ref=ref(s,'#name'),value='Test name')['ok']; assert page.locator('#name').input_value()=='Test name'; passed('non-secret fill updates the real DOM')
    assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok']; passed('mutation consumes its refs')
    s=run('snapshot'); assert run('select',token=s['token'],ref=ref(s,'#choice'),value='b')['ok']; assert page.locator('#choice').input_value()=='b'; passed('select changes the real selected value')
    s=run('snapshot'); assert run('check',token=s['token'],ref=ref(s,'#flag'),value='true')['ok']; assert page.locator('#flag').is_checked(); passed('checkbox changes real state')
    s=run('snapshot'); secret=next(e for e in s['elements'] if e.get('inputType')=='password'); assert not run('fill',token=s['token'],ref=secret['ref'],value='no')['ok']; passed('password-field interaction is refused')
    s=run('snapshot'); assert run('click',token=s['token'],ref=ref(s,'#save'))['ok']; assert run('expect_text',value='Saved locally')['passed']; passed('click and independent text check exercise a DOM-only flow')
    # Synthetic timing data tests classification/redaction only; this is not observed network evidence.
    page.evaluate('''performance.getEntriesByType=()=>[
      {name:'http://localhost:3000/api/items?token=private-query',initiatorType:'fetch',responseStatus:200,duration:12,transferSize:20},
      {name:'http://localhost:3000/api/unknown',initiatorType:'fetch',responseStatus:0,duration:2},
      {name:'http://localhost:3000/api/fail',initiatorType:'fetch',responseStatus:503,duration:2}]''')
    resources=run('resources'); assert 'private-query' not in json.dumps(resources); assert all('?' not in r['url'] for r in resources['resources']); passed('synthetic timing records omit query and body data')
    assert run('expect_resource',value='/api/items',status=200)['passed']; assert not run('expect_resource',value='/api/unknown',status=200)['passed']; assert not run('expect_resource',value='/api/fail',status=200)['passed']; passed('synthetic status records distinguish known, unknown and failure')
    audit=run('audit'); assert {'missing-language','missing-name','image-alt'}.issubset({f['code'] for f in audit['findings']}); passed('real DOM audit finds known fixture issues')
    assert run('observe_start')['observing']; page.evaluate('window.dispatchEvent(new ErrorEvent("error",{message:"token=private-observation"}))'); obs=run('observe_read'); assert obs['errors'] and 'private-observation' not in json.dumps(obs); passed('opt-in error collection redacts token-shaped text')
    count=len(obs['errors']); run('observe_stop'); page.evaluate('window.dispatchEvent(new ErrorEvent("error",{message:"after-stop"}))'); assert len(run('observe_read')['errors'])==count; passed('stopping error observation removes handlers')
    assert not run('expect_element',selector='[')['ok']; assert not run('expect_text',value='Not present')['passed']; passed('bad selector and failed expectation cannot pass')
    page.evaluate('document.body.insertAdjacentHTML("beforeend","<div style=width:1800px>overflow</div>")'); assert any(f['code']=='horizontal-overflow' for f in run('audit')['findings']); passed('real viewport overflow is reported')
    page.set_viewport_size({'width':390,'height':780}); assert run('snapshot')['viewport']['width']==390; page.screenshot(path=str(output/'qa-dom-mobile.png')); passed('responsive DOM snapshot reports actual viewport')
    s=run('snapshot'); run('clear'); assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok']; passed('clear revokes refs')
    s=run('snapshot'); page.evaluate('history.replaceState(null,"","#different-route")'); assert not run('click',token=s['token'],ref=ref(s,'#save'))['ok']; passed('same-document route change revokes old refs without needing a DOM mutation')
    assert not run('scroll',value='constructor')['ok']; passed('prototype names are not valid scroll directions')
    run('observe_start'); marker='gh'+'p_'+'q'*24; page.evaluate('(value)=>window.dispatchEvent(new ErrorEvent("error",{message:value}))',marker); assert marker not in json.dumps(run('observe_read')); run('observe_stop'); passed('unlabelled token-shaped errors are redacted')
    browser.close()
(output/'browser-dom-checks.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'scope':'Chromium set_content DOM fixture; timing records synthetic; no HTTP, Tauri or providers; opaque origin allowed only in direct JS unit fixture, rejected by native validator'},indent=2))
print('TOTAL',len(checks))
