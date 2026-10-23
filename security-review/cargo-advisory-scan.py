"""Read-only OSV audit; uploads only public crates.io names and locked versions."""
import concurrent.futures
import datetime
import hashlib
import json
import pathlib
import tomllib
import urllib.request

EVIDENCE = pathlib.Path(__file__).resolve().parent
LOCK = EVIDENCE.parent / 'src-tauri/Cargo.lock'
lock_bytes = LOCK.read_bytes()
all_packages = tomllib.loads(lock_bytes.decode())['package']
registry_packages = [p for p in all_packages if p.get('source', '').startswith('registry+')]
# Keep querying the public upstream identity of the local backport. A scanner
# result disappearing merely because Cargo switched to a path is not a fix.
local_backports = [p for p in all_packages if p['name'] == 'glib' and 'source' not in p]
packages = registry_packages + local_backports

def request(url, value=None):
    data = None if value is None else json.dumps(value).encode()
    req = urllib.request.Request(url, data=data,
        headers={'Content-Type': 'application/json', 'User-Agent': 'PandaCode-read-only-dependency-audit'})
    with urllib.request.urlopen(req, timeout=60) as response:
        return json.load(response)

matches = []
for start in range(0, len(packages), 200):
    batch = packages[start:start + 200]
    result = request('https://api.osv.dev/v1/querybatch', {
        'queries': [{'package': {'ecosystem': 'crates.io', 'name': p['name']},
                     'version': p['version']} for p in batch]})
    for package, entry in zip(batch, result['results'], strict=True):
        if entry.get('vulns'):
            matches.append({'name': package['name'], 'version': package['version'],
                            'source_type': 'registry' if 'source' in package else 'local-backport',
                            'advisories': [v['id'] for v in entry['vulns']]})

ids = sorted({advisory for match in matches for advisory in match['advisories']})
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    details = dict(zip(ids, executor.map(
        lambda advisory: request('https://api.osv.dev/v1/vulns/' + advisory), ids), strict=True))
report = {'checked_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'lock_sha256': hashlib.sha256(lock_bytes).hexdigest(),
          'registry_packages': len(registry_packages),
          'local_backports_queried': [{'name': p['name'], 'version': p['version']} for p in local_backports],
          'queries': len(packages), 'matches': matches, 'advisories': details}
output = EVIDENCE / 'cargo-osv-results-recheck.json'
output.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k: v for k, v in report.items() if k != 'advisories'}, indent=2))
for advisory in ids:
    item = details[advisory]
    print(json.dumps({'id': advisory, 'summary': item.get('summary'),
        'withdrawn': item.get('withdrawn'), 'database_specific': item.get('database_specific'),
        'severity': item.get('severity'), 'affected': item.get('affected')}, indent=2))
