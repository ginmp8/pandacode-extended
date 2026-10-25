// Retained filename for the existing test: validate the active Extended release,
// without rewriting legacy HARDENED-REVISION.json provenance.
import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';
import { displayVersion, releaseDateKey, releaseNotePath, extendedVersion, upstreamReference } from './extended-version.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const legacy = ['0.3.35-hardened.3', '0.3.35-hardened.4', '0.3.35.3', '0.3.35.4', '0.3.41-hardened', '0.3.42-hardened'];
// The prior review is evidence about its exact bytes, not mutable current metadata.
// Normalize checkout line endings so Git's Windows autocrlf does not change its identity.
const historicalReviewSha256 = 'e64c3e5e51e4b161aaf4fbc7b2a540bda1487117f54b8724d3547f69b59e5ecf';

export async function check(sourceRoot = root) {
  const read = async path => readFile(resolve(sourceRoot, path), 'utf8');
  const json = async path => JSON.parse(await read(path));
  const historicalText = await read('HARDENED-REVISION.json');
  assert(createHash('sha256').update(historicalText.replaceAll('\r\n', '\n')).digest('hex') === historicalReviewSha256, 'Historical review evidence changed');
  const history = JSON.parse(historicalText);
  const meta = await json('EXTENDED-RELEASE.json');
  const pkg = await json('package.json');
  const lock = await json('package-lock.json');
  const tauri = await json('src-tauri/tauri.conf.json');
  const mobile = await json('mobile/package.json');
  const mobileLock = await json('mobile/package-lock.json');
  const mobileApp = await json('mobile/app.json');
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
  assert(history.tracked_upstream_version === '0.3.42', 'Historical tracked reference drift');
  assert(meta.source_base_upstream_version === history.source_base_upstream_version && meta.source_base_upstream_version === '0.3.28', 'Historical source ancestry drift');
  assert(history.upstream_code_rebased === false && history.feature_parity_with_upstream_0_3_42 === false, 'Legacy provenance claim drift');
  const range = meta.reviewed_upstream_range;
  assert(range?.from === history.tracked_upstream_version && range.through === meta.tracked_upstream_version, 'Reviewed upstream range drift');
  const [major, minor, first] = upstreamReference(range.from).split('.').map(Number);
  const [lastMajor, lastMinor, last] = upstreamReference(range.through).split('.').map(Number);
  assert(major === lastMajor && minor === lastMinor && last >= first && last - first <= 1000, 'Invalid reviewed upstream range');
  const integration = meta.integration_provenance;
  assert(/^[a-f0-9]{40}$/.test(integration?.extended_base_commit ?? '') && /^[a-f0-9]{40}$/.test(integration?.upstream_commit ?? ''), 'Missing integration commit identities');
  assert(/^[a-f0-9]{64}$/.test(integration?.reference_archive_sha256 ?? ''), 'Missing reference archive identity');
  assert(Number.isSafeInteger(integration?.reference_files_verified) && integration.reference_files_verified > 0, 'Missing reference comparison coverage');
  // Product-only patches retain the reviewed source evidence from their integration release.
  assert(/^docs\/upstream-integration-(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\.md$/.test(integration.coverage_document ?? ''), 'Integration coverage path drift');
  const coverage = await read(integration.coverage_document);
  for (const identity of [integration.extended_base_commit, integration.upstream_commit, integration.reference_archive_sha256]) {
    assert(coverage.includes(identity), 'Integration coverage omits source identity');
  }
  assert([lock.version, lock.packages?.['']?.version].every(x => x === v), 'npm package/lock version drift');
  assert(tauri.version === v, 'Tauri version drift');
  assert([mobile.version, mobileLock.version, mobileLock.packages?.['']?.version, mobileApp.expo?.version].every(x => x === v), 'Mobile version drift');
  assert(cargo.includes('[package]\nname = "pandacode"\nversion = "' + v + '"'), 'Cargo.toml version drift');
  assert(cargoLock.includes('name = "pandacode"\nversion = "' + v + '"'), 'Cargo.lock version drift');
  assert(tauri.identifier === 'ai.pandacode.desktop', 'App data identity changed without migration');
  assert(css.includes('overflow-y:auto;overflow-x:hidden;'), 'Missing transcript overflow fix');
  assert(workers.includes('grid-template-columns:minmax(0,1fr)'), 'Missing shrinkable team-grid fix');
  assert(spec.includes("test('a long worker line never makes a narrow thread scroll sideways'"), 'Missing UI regression test');
  const rootFiles = (await readdir(resolve(sourceRoot, 'releases'))).filter(name => name.endsWith('.md'));
  const extendedFiles = (await readdir(resolve(sourceRoot, 'releases/extended'))).filter(name => name.endsWith('.md'));
  assert(rootFiles.every(name => /^\d+\.\d+\.\d+\.md$/.test(name)), 'Local notes leaked into the upstream root');
  assert(rootFiles.includes(meta.tracked_upstream_version + '.md'), 'Missing tracked upstream reference');
  assert(extendedFiles.includes(v + '.md'), 'Missing active Extended note');
  const extendedDateKeys = new Set(extendedFiles.map(name => 'extended/' + name.slice(0, -3)));
  assert(Object.keys(dates).every(key => !key.startsWith('extended/') || extendedDateKeys.has(key)), 'An Extended release date has no matching release note');
  assert(Boolean((await read(meta.release_note)).trim()) && Boolean(dates[releaseDateKey(v)]), 'Missing active Extended release note/date');
  for (const older of legacy) {
    assert(!rootFiles.includes(older + '.md') && extendedFiles.includes(older + '.md'), 'Legacy release note misplaced: ' + older);
  }
  const references = new Set(Array.from({ length: 14 }, (_, i) => '0.3.' + (29 + i)));
  for (let patch = first; patch <= last; patch++) references.add(`${major}.${minor}.${patch}`);
  for (const upstream of references) {
    assert(rootFiles.includes(upstream + '.md') && Boolean(dates[upstream]), 'Missing upstream historical reference ' + upstream);
  }
  return { version: v, display: meta.display_version, upstream: meta.tracked_upstream_version, sourceBase: meta.source_base_upstream_version, upstreamNotes: references.size, extendedNotes: extendedFiles.length };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  check().then(result => console.log(JSON.stringify(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
