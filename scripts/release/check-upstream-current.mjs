// Read-only upstream version check. Never downloads or executes releases.
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export function parseVersion(value) {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(value);
  if (!match) throw new Error('Unexpected upstream release tag');
  return match.slice(1).map(Number);
}
export function compareVersion(a, b) {
  const left = parseVersion(a), right = parseVersion(b);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return Math.sign(left[i] - right[i]);
  return 0;
}
export async function checkUpstream(fetcher = fetch) {
  const meta = JSON.parse(await readFile(resolve(root, 'HARDENED-REVISION.json'), 'utf8'));
  const response = await fetcher('https://api.github.com/repos/aicodeking/pandacode/releases/latest', {
    headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'pandacode-hardened-version-check' },
    signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`Could not check upstream releases (HTTP ${response.status})`);
  const release = await response.json();
  const tag = release?.tag_name;
  if (typeof tag !== 'string') throw new Error('Upstream release response omitted tag_name');
  const ahead = compareVersion(tag, meta.tracked_upstream_version) > 0;
  return { latest: tag, tracked: meta.tracked_upstream_version, behind: ahead };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  checkUpstream().then(result => {
    console.log(JSON.stringify(result));
    if (result.behind) process.exitCode = 2;
  }).catch(error => { console.error(error.message); process.exitCode = 1; });
}
