import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = async path => readFile(resolve(root, path), 'utf8');
const json = async path => JSON.parse(await read(path));
const assert = (condition, message) => { if (!condition) throw new Error(message); };
export async function check() {
  const meta = await json('HARDENED-REVISION.json');
  const pkg = await json('package.json');
  const lock = await json('package-lock.json');
  const tauri = await json('src-tauri/tauri.conf.json');
  const cargo = await read('src-tauri/Cargo.toml');
  const cargoLock = await read('src-tauri/Cargo.lock');
  const dates = await json('website/src/data/release-dates.json');
  const css = await read('src/styles.css');
  const workers = await read('src/team-progress.css');
  const spec = await read('tests/architect.spec.ts');
  const v = pkg.version;
  const release = /^(\d+\.\d+\.\d+)(?:\.(\d+)|-hardened)$/.exec(v);
  assert(release, 'Unexpected product version');
  const nativeVersion = release[2] ? `${release[1]}+${release[2]}` : v;
  assert([lock.version, lock.packages?.['']?.version].every(x => x === v), 'npm package/lock version drift');
  assert(tauri.version === nativeVersion, 'Tauri version drift');
  assert(new RegExp('\\[package\\][\\s\\S]*?\\nversion = "' + nativeVersion.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '"').test(cargo), 'Cargo.toml version drift');
  assert(new RegExp('name = "pandacode"\\r?\\nversion = "' + nativeVersion.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '"').test(cargoLock), 'Cargo.lock version drift');
  assert(release[1] === meta.upstream_version && meta.upstream_version === meta.tracked_upstream_version && meta.reference_upstream_version === meta.upstream_version, 'Reference version drift');
  assert(meta.product_version === `${meta.upstream_version}-hardened` && meta.product_version === meta.release_display_version && meta.revision === meta.release_display_version, 'Hardened revision metadata drift');
  assert(meta.source_base_upstream_version === '0.3.28' && meta.upstream_code_rebased === false, 'Missing honest base provenance');
  assert(meta.feature_parity_with_upstream_0_3_42 === false, 'False complete-parity claim');
  assert(css.includes('overflow-y:auto;overflow-x:hidden;'), 'Missing transcript overflow fix');
  assert(workers.includes('grid-template-columns:minmax(0,1fr)'), 'Missing shrinkable team-grid fix');
  assert(spec.includes("test('a long worker line never makes a narrow thread scroll sideways'"), 'Missing UI regression test');
  for (let minor = 29; minor <= 42; minor++) {
    const note = `0.3.${minor}`;
    assert(Boolean((await read(`releases/${note}.md`)).trim()), `Missing upstream release note ${note}`);
    assert(Boolean(dates[note]), `Missing upstream date ${note}`);
  }
  const displayVersion = meta.release_display_version;
  assert(Boolean((await read(`releases/${v}.md`)).trim()) && Boolean(dates[v]), 'Missing local Hardened release note/date');
  assert(Boolean((await read(`releases/${displayVersion}.md`)).trim()) && Boolean(dates[displayVersion]), 'Missing metadata Hardened release note/date');
  return { version:v, upstream:meta.upstream_version, sourceBase:meta.source_base_upstream_version, upstreamNotes:14 };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  check().then(result => console.log(JSON.stringify(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
