// Retained filename for the existing test: validate the active Extended release,
// without rewriting legacy HARDENED-REVISION.json provenance.
import { readFile, readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';
import { displayVersion, releaseDateKey, releaseNotePath, extendedVersion } from './extended-version.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = async path => readFile(resolve(root, path), 'utf8');
const json = async path => JSON.parse(await read(path));
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const legacy = ['0.3.35-hardened.3', '0.3.35-hardened.4', '0.3.35.3', '0.3.35.4', '0.3.41-hardened', '0.3.42-hardened'];

export async function check() {
  const history = await json('HARDENED-REVISION.json');
  const meta = await json('EXTENDED-RELEASE.json');
  const pkg = await json('package.json');
  const lock = await json('package-lock.json');
  const tauri = await json('src-tauri/tauri.conf.json');
  const cargo = await read('src-tauri/Cargo.toml');
  const cargoLock = await read('src-tauri/Cargo.lock');
  const dates = await json('website/src/data/release-dates.json');
  const css = await read('src/styles.css');
  const workers = await read('src/team-progress.css');
  const spec = await read('tests/architect.spec.ts');
  const v = extendedVersion(pkg.version);
  assert(meta.schema_version === 1 && meta.product === 'PandaCode Extended', 'Extended metadata contract drift');
  assert(v === meta.extended_version, 'Extended metadata/npm version drift');
  assert(meta.display_version === displayVersion(v, meta.tracked_upstream_version), 'Extended display version drift');
  assert(meta.release_note === releaseNotePath(v), 'Extended note path drift');
  assert(meta.full_upstream_parity === false && meta.upstream_code_rebased === false, 'Unsupported upstream parity claim');
  assert(meta.tracked_upstream_version === history.tracked_upstream_version, 'Tracked upstream reference drift');
  assert(meta.source_base_upstream_version === history.source_base_upstream_version && meta.source_base_upstream_version === '0.3.28', 'Historical source ancestry drift');
  assert(history.upstream_code_rebased === false && history.feature_parity_with_upstream_0_3_42 === false, 'Legacy provenance claim drift');
  assert([lock.version, lock.packages?.['']?.version].every(x => x === v), 'npm package/lock version drift');
  assert(tauri.version === v, 'Tauri version drift');
  assert(cargo.includes('[package]\nname = "pandacode"\nversion = "' + v + '"'), 'Cargo.toml version drift');
  assert(cargoLock.includes('name = "pandacode"\nversion = "' + v + '"'), 'Cargo.lock version drift');
  assert(tauri.identifier === 'ai.pandacode.desktop', 'App data identity changed without migration');
  assert(css.includes('overflow-y:auto;overflow-x:hidden;'), 'Missing transcript overflow fix');
  assert(workers.includes('grid-template-columns:minmax(0,1fr)'), 'Missing shrinkable team-grid fix');
  assert(spec.includes("test('a long worker line never makes a narrow thread scroll sideways'"), 'Missing UI regression test');
  const rootFiles = (await readdir(resolve(root, 'releases'))).filter(name => name.endsWith('.md'));
  const extendedFiles = (await readdir(resolve(root, 'releases/extended'))).filter(name => name.endsWith('.md'));
  assert(rootFiles.every(name => /^\d+\.\d+\.\d+\.md$/.test(name)), 'Local notes leaked into the upstream root');
  assert(rootFiles.includes(meta.tracked_upstream_version + '.md'), 'Missing tracked upstream reference');
  assert(extendedFiles.includes(v + '.md'), 'Missing active Extended note');
  assert(!rootFiles.includes('0.3.42.1.md') && !extendedFiles.includes('0.3.42.1.md') && !dates['extended/0.3.42.1'], 'Unpublished 0.3.42.1 candidate must not be in the release history');
  assert(Boolean((await read(meta.release_note)).trim()) && Boolean(dates[releaseDateKey(v)]), 'Missing active Extended release note/date');
  for (const older of legacy) {
    assert(!rootFiles.includes(older + '.md') && extendedFiles.includes(older + '.md'), 'Legacy release note misplaced: ' + older);
  }
  for (let minor = 29; minor <= 42; minor++) {
    const upstream = '0.3.' + minor;
    assert(rootFiles.includes(upstream + '.md') && Boolean(dates[upstream]), 'Missing upstream historical reference ' + upstream);
  }
  return { version: v, display: meta.display_version, upstream: meta.tracked_upstream_version, sourceBase: meta.source_base_upstream_version, upstreamNotes: 14, extendedNotes: extendedFiles.length };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  check().then(result => console.log(JSON.stringify(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
