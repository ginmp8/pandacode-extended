#!/usr/bin/env python3
"""Static, offline, read-only verifier for PandaCode selective 0.3.35 tray backport.
Does not run build scripts, Rust/Node application code, plugins, external processes or network.
Supply frozen-evaluation.json and current candidate root. Outputs JSON and fails closed.
"""
from __future__ import annotations
import argparse, hashlib, json, re
from pathlib import Path

def sha(b:bytes)->str:return hashlib.sha256(b).hexdigest()
def verify(root:Path, frozen:dict)->dict:
  checks=[]
  def check(name:str,predicate:bool,detail:str=''):
    checks.append({'check':name,'status':'pass' if predicate else 'fail','detail':detail})
  origin=root.parent/'baseline'
  base_paths={str(x.relative_to(origin)) for x in origin.rglob('*') if x.is_file()}
  cand_paths={str(x.relative_to(root)) for x in root.rglob('*') if x.is_file()}
  changed=sorted(x for x in base_paths&cand_paths if (origin/x).read_bytes()!=(root/x).read_bytes())
  added=sorted(cand_paths-base_paths)
  removed=sorted(base_paths-cand_paths)
  expected_changed={
    'AGENTS.md','HARDENED-REVISION.json','README.md','package-lock.json','package.json',
    'src-tauri/Cargo.lock','src-tauri/Cargo.toml','src-tauri/tauri.conf.json',
    'src-tauri/src/lib.rs','src-tauri/src/tray.rs','src/entry.ts','src/preview.ts'
  }
  expected_added={
    'src/tray-popover.ts','src/tray-popover.css','src-tauri/capabilities/tray.json','tests/tray-popover.spec.ts',
    'security-review/0.3.35-hardened/README.md','security-review/0.3.35-hardened/source-manifest.json',
    'security-review/0.3.35-hardened/static-verification.json','security-review/0.3.35-hardened/verify-source.py',
    'security-review/0.3.35-hardened/frozen-evaluation.json',
    'security-review/0.3.35-hardened/changes.patch',
    'security-review/0.3.35-hardened/changes.json',
    'security-review/0.3.35-hardened/tamper-probe.json'
  }
  check('allowed-edits-only', set(changed)==expected_changed,'unexpected='+str(sorted(set(changed)^expected_changed)))
  check('allowed-additions-only',set(added)<=expected_added,'unexpected='+str(sorted(set(added)-expected_added)))
  check('no-deletions',not removed, 'unexpected='+str(removed))
  for rel,digest in frozen['protected_sha256'].items():
    if rel in ('package-lock.json','src-tauri/Cargo.lock','src-tauri/tauri.conf.json'):
      a=(origin/rel).read_bytes();b=(root/rel).read_bytes()
      check('pin-change-only:'+rel,b==a.replace(b'0.3.34-hardened.1',b'0.3.35-hardened.1'))
    else:
      check('protected-byte-identical:'+rel,sha((root/rel).read_bytes())==digest)
  versions=[json.loads((root/f).read_text())['version'] for f in ('package.json','package-lock.json','src-tauri/tauri.conf.json')]
  cargo=(root/'src-tauri/Cargo.toml').read_text();lock=(root/'src-tauri/Cargo.lock').read_text()
  check('version-sync',versions==['0.3.35-hardened.1']*3 and 'version = "0.3.35-hardened.1"' in cargo and 'version = "0.3.35-hardened.1"' in lock)
  cap=json.loads((root/'src-tauri/capabilities/tray.json').read_text())
  main=json.loads((root/'src-tauri/capabilities/default.json').read_text())
  check('tray-least-privilege',cap.get('webviews')==['tray'] and set(cap.get('permissions',[]))=={'core:event:allow-listen','core:event:allow-unlisten'} and main.get('webviews')==['main'])
  lib=(root/'src-tauri/src/lib.rs').read_text()
  block=lib.split('if invoke.message.webview_ref().label() == "tray" {',1)[-1].split('if invoke.message.webview_ref().label() != "main" {',1)[0]
  acl=['tray_snapshot','tray_open','tray_refresh_usage','tray_show_app','tray_hide','tray_quit','tray_resize']
  exposed=re.findall(r'tray::(tray_[a-z_]+)',block)
  check('tray-command-whitelist',exposed==acl and 'return handler(invoke);' in block and 'Browser content has no application access.' in lib)
  tray=(root/'src-tauri/src/tray.rs').read_text()
  check('native-navigation-bound',all(s in tray for s in ('on_navigation(|url|','url.query_pairs().any(|(key, value)| key == "view" && value == "tray")','"/index.html" | "/"')))
  check('bounded-tray-memory','while unseen.len() > 1024 { unseen.pop_first(); }' in tray)
  check('bounded-refresh',all(s in tray for s in ('const MAX_PROVIDERS: usize = 32;','CHECKING.compare_exchange(0, providers.len()','CHECKING.fetch_sub(1')))
  check('focus-workflow','tray::finished(&app, &session_id, was_stopped, successful);' in lib)
  check('separate-frontend-entry',"view === 'tray'" in (root/'src/entry.ts').read_text())
  ts=(root/'src/tray-popover.ts').read_text()
  check('escaped-user-facing-content',all(s in ts for s in ('escape(task.id)','escape(task.title','escape(sub)','escape(usage.plan)','escape(window.label)')))
  check('numeric-usage-sanitize','Number.isFinite(window.usedPercent)' in ts and 'aria-valuenow' in ts)
  check('tray-ui-tests','tray_open' in (root/'tests/tray-popover.spec.ts').read_text() and 'ArrowDown' in (root/'tests/tray-popover.spec.ts').read_text())
  check('no-updater-regression','tauri-plugin-updater' not in cargo and 'updater:' not in (root/'src-tauri/capabilities/default.json').read_text())
  check('no-install-regression','npm install' not in (root/'src-tauri/src/cloud/cloudflare.rs').read_text() and 'npx' not in (root/'relay/package.json').read_text())
  check('no-cloud-agent-import','mod cloud_agents;' not in lib and not (root/'src-tauri/src/cloud_agents.rs').exists())
  check('npx-remote-install-still-disabled','Disabled remote installer' in (root/'packages/npx/package.json').read_text())
  check('lockfiles-retained',bool((root/'package-lock.json').stat().st_size and (root/'src-tauri/Cargo.lock').stat().st_size))
  check('no-symlinks',not any(f.is_symlink() for f in root.rglob('*')))
  # Scan newly added executable files for likely hardcoded credential formats, return only counts/paths.
  patterns=[re.compile(p) for p in (r'ghp_[A-Za-z0-9]{30,}',r'gho_[A-Za-z0-9]{30,}',r'xox[baprs]-[A-Za-z0-9-]{22,}',r'AKIA[A-Z0-9]{16}',r'-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----')]
  new_code=[x for x in added if x.endswith(('.rs','.ts','.js','.mjs','.py'))]
  secret_hits={x:sum(1 for pattern in patterns if pattern.search((root/x).read_text(errors='replace'))) for x in new_code}
  secret_hits={k:v for k,v in secret_hits.items() if v}
  check('no-likely-credentials-in-new-code',not secret_hits,'paths-counts='+json.dumps(secret_hits))
  return {'source_version':'0.3.35-hardened.1','input_archive_sha256':frozen['inputs'],'checks':checks,'passed':sum(c['status']=='pass' for c in checks),'failed':sum(c['status']=='fail' for c in checks),'modified_files':changed,'added_files':added,'removed_files':removed,'status':'static-pass' if all(c['status']=='pass' for c in checks) else 'static-fail','runtime':{'cargo-check':'not-run (Rust toolchain unavailable)','cargo-test':'not-run (Rust toolchain unavailable and no approved target sandbox)','tsc-full':'attempted-blocked (62 baseline missing-dependency diagnostics, 63 candidate; one new diagnostic is an existing Tauri API module import unavailable without node_modules)','playwright':'not-run (browser dependencies and approved runtime unavailable)','runtime-IPC':'not-run','SCA':'not-run','credential-provider-verification':'not-run','cross-platform':'not-run'}}

def main():
 p=argparse.ArgumentParser();p.add_argument('--root',required=True);p.add_argument('--frozen',required=True);p.add_argument('--output',required=True);args=p.parse_args()
 result=verify(Path(args.root),json.loads(Path(args.frozen).read_text()))
 Path(args.output).parent.mkdir(parents=True,exist_ok=True)
 Path(args.output).write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
 print('STATUS',result['status'],'PASS',result['passed'],'FAIL',result['failed'])
 for c in result['checks']:
   if c['status']=='fail':print('FAIL',c)
 raise SystemExit(0 if result['failed']==0 else 1)
if __name__=='__main__':main()
