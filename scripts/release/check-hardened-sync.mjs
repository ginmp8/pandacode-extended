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
  assert(/^\d+\.\d+\.\d+-hardened$/.test(v), 'Unexpected product version');
  assert([lock.version, lock.packages?.['']?.version, tauri.version, meta.product_version, meta.release_display_version].every(x => x === v), 'Manifest version drift');
  assert(new RegExp('\\[package\\][\\s\\S]*?\\nversion = "' + v + '"').test(cargo), 'Cargo.toml version drift');
  assert(cargoLock.includes('name = "pandacode"\nversion = "' + v + '"'), 'Cargo.lock version drift');
  assert(meta.upstream_version === v.replace('-hardened', '') && meta.tracked_upstream_version === meta.upstream_version, 'Reference version drift');
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
  assert(Boolean((await read(`releases/${v}.md`)).trim()) && Boolean(dates[v]), 'Missing local Hardened release note/date');
  return { version:v, upstream:meta.upstream_version, sourceBase:meta.source_base_upstream_version, upstreamNotes:14 };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  check().then(result => console.log(JSON.stringify(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
