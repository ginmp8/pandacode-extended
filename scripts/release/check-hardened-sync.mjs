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
  const incremental = meta.incremental_upstream_review;
  assert(incremental?.from === '0.3.47' && incremental?.through === meta.tracked_upstream_version
    && incremental.extended_base_commit === '430909614339c58ac4a5beea4baa2707f2af27da'
    && incremental.upstream_from_commit === '4187553cb48db96657609948f6b0904556845d45'
    && incremental.upstream_through_commit === '9cc1a9a58d259f7316e4f3ad621e8ca120358d89', 'Incremental upstream identity drift');
  assert(incremental.source_note === 'releases/0.3.48.md'
    && incremental.source_note_blob_sha === '639bd02d0f5513aad54bd253c6f496676b824962'
    && incremental.coverage_document === 'docs/upstream-integration-0.5.1.md', 'Incremental upstream evidence path drift');
  const incrementNote = await read(incremental.source_note);
  const noteBlob = createHash('sha1').update('blob ' + Buffer.byteLength(incrementNote) + '\0').update(incrementNote).digest('hex');
  assert(noteBlob === incremental.source_note_blob_sha, 'Incremental upstream source note changed');
  const incrementalCoverage = await read(incremental.coverage_document);
  for (const identity of [incremental.extended_base_commit, incremental.upstream_from_commit, incremental.upstream_through_commit, incremental.source_note_blob_sha]) {
    assert(incrementalCoverage.includes(identity), 'Incremental coverage omits source identity');
  }
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
  // Supplemental historical review never rewrites the newer integration or the
  // pinned legacy review. Hash every original source note, including source-only .36.
  const historical = meta.historical_upstream_review;
  assert(historical?.from === meta.source_base_upstream_version && historical.through === range.from, 'Historical coverage range drift');
  assert(historical.extended_base_commit === '7f14199973d7bfc62dfb07fa08cb1e3bbd328c5e'
    && historical.upstream_from_commit === '70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb'
    && historical.upstream_through_commit === 'bca0b9f1f03a1cc3f8a8e6330881c34103f14a46', 'Historical coverage commit drift');
  assert(historical.coverage_document === 'docs/upstream-parity-0.3.28-0.3.42.md'
    && historical.release_evidence === 'docs/upstream-release-evidence-0.3.28-0.3.42.json', 'Historical coverage path drift');
  const evidenceText = (await read(historical.release_evidence)).replaceAll('\r\n', '\n');
  assert(createHash('sha256').update(evidenceText).digest('hex') === historical.release_evidence_sha256, 'Historical release evidence digest drift');
  const evidence = JSON.parse(evidenceText);
  assert(evidence.schema_version === 1 && evidence.repository === 'https://github.com/aicodeking/pandacode'
    && evidence.upstream_from_commit === historical.upstream_from_commit
    && evidence.upstream_through_commit === historical.upstream_through_commit, 'Historical release evidence identity drift');
  assert(Array.isArray(evidence.notes) && evidence.notes.length === 15, 'Historical release evidence coverage gap');
  for (let offset = 0; offset < 15; offset++) {
    const note = evidence.notes[offset];
    const upstream = `0.3.${28 + offset}`;
    assert(note.version === upstream && note.source_path === `releases/${upstream}.md`, 'Historical release evidence sequence drift');
    const noteText = (await read(note.source_path)).replaceAll('\r\n', '\n');
    assert(createHash('sha256').update(noteText).digest('hex') === note.source_sha256, 'Historical source note changed: ' + upstream);
    if (upstream === '0.3.36') {
      assert(note.publication === 'source-only' && !note.published_at && !note.release_url && !note.github_body_sha256, 'Source-only release claims publication');
    } else {
      assert(note.publication === 'github-release' && note.release_url === `https://github.com/aicodeking/pandacode/releases/tag/v${upstream}`
        && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(note.published_at ?? '')
        && /^[a-f0-9]{64}$/.test(note.github_body_sha256 ?? ''), 'Published release evidence missing: ' + upstream);
    }
  }
  const historicalCoverage = await read(historical.coverage_document);
  for (const identity of [historical.extended_base_commit, historical.upstream_from_commit, historical.upstream_through_commit, historical.release_evidence_sha256]) {
    assert(historicalCoverage.includes(identity), 'Historical coverage omits source identity');
  }
  // The earlier audit supplements both existing reviews. Its 0.2.x notes have
  // source evidence, but no publication record in the verified public inventory.
  const early = meta.early_upstream_review;
  assert(early?.from === '0.2.0' && early.through === '0.3.27', 'Early coverage range drift');
  assert(early.extended_base_commit === '68e4d0615ec864a2dd9e9630e0e3410432e27207'
    && early.upstream_snapshot_commit === '612ca79c8e7db74461040bc27ddf1a3dcebb32ab', 'Early coverage commit drift');
  assert(early.coverage_document === 'docs/upstream-parity-before-0.3.28.md'
    && early.release_evidence === 'docs/upstream-release-evidence-before-0.3.28.json', 'Early coverage path drift');
  const earlyText = (await read(early.release_evidence)).replaceAll('\r\n', '\n');
  assert(createHash('sha256').update(earlyText).digest('hex') === early.release_evidence_sha256, 'Early release evidence digest drift');
  const earlyEvidence = JSON.parse(earlyText);
  assert(earlyEvidence.schema_version === 1 && earlyEvidence.repository === 'https://github.com/aicodeking/pandacode'
    && earlyEvidence.upstream_snapshot_commit === early.upstream_snapshot_commit, 'Early release evidence identity drift');
  const earlyVersions = [...Array.from({ length: 49 }, (_, patch) => `0.2.${patch}`), ...Array.from({ length: 28 }, (_, patch) => `0.3.${patch}`)];
  assert(Array.isArray(earlyEvidence.notes) && earlyEvidence.notes.length === earlyVersions.length, 'Early release evidence coverage gap');
  for (const [offset, upstream] of earlyVersions.entries()) {
    const note = earlyEvidence.notes[offset];
    assert(note.version === upstream && note.source_path === `releases/${upstream}.md`, 'Early release evidence sequence drift');
    const noteText = (await read(note.source_path)).replaceAll('\r\n', '\n');
    assert(createHash('sha256').update(noteText).digest('hex') === note.source_sha256, 'Early source note changed: ' + upstream);
    if (upstream.startsWith('0.2.')) {
      assert(note.publication === 'source-only' && !note.published_at && !note.release_url && !note.github_body_sha256, 'Early source-only note claims publication');
    } else {
      assert(note.publication === 'github-release' && note.release_url === `https://github.com/aicodeking/pandacode/releases/tag/v${upstream}`
        && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(note.published_at ?? '')
        && /^[a-f0-9]{64}$/.test(note.github_body_sha256 ?? ''), 'Early published release evidence missing: ' + upstream);
    }
  }
  const earlyCoverage = await read(early.coverage_document);
  for (const identity of [early.extended_base_commit, early.upstream_snapshot_commit, early.release_evidence_sha256]) {
    assert(earlyCoverage.includes(identity), 'Early coverage omits source identity');
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
  const references = new Set([...earlyVersions, ...evidence.notes.map(note => note.version)]);
  const sourceOnly = new Set([...earlyEvidence.notes, ...evidence.notes].filter(note => note.publication === 'source-only').map(note => note.version));
  for (let patch = first; patch <= last; patch++) references.add(`${major}.${minor}.${patch}`);
  for (const upstream of references) {
    assert(rootFiles.includes(upstream + '.md') && (sourceOnly.has(upstream) || Boolean(dates[upstream])), 'Missing upstream historical reference ' + upstream);
  }
  return { version: v, display: meta.display_version, upstream: meta.tracked_upstream_version, sourceBase: meta.source_base_upstream_version, upstreamNotes: references.size, extendedNotes: extendedFiles.length };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  check().then(result => console.log(JSON.stringify(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
