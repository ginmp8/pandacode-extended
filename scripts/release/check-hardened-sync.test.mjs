import test from 'node:test';
import assert from 'node:assert/strict';
import { cp, mkdtemp, mkdir, readFile, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { check } from './check-hardened-sync.mjs';

test('Extended core/display version, historical ancestry, layout patch and separated release histories stay consistent', async () => {
  assert.deepEqual(await check(), { version: '0.5.0', display: '0.5.0+pc.0.3.47', upstream: '0.3.47', sourceBase: '0.3.28', upstreamNotes: 19, extendedNotes: 8 });
});

const source = new URL('../../', import.meta.url);
const fixturePaths = ['HARDENED-REVISION.json', 'EXTENDED-RELEASE.json', 'package.json', 'package-lock.json',
  'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'website/src/data/release-dates.json',
  'src/styles.css', 'src/team-progress.css', 'tests/architect.spec.ts', 'docs/upstream-integration-0.5.0.md', 'releases'];

async function fixture(run) {
  const directory = await mkdtemp(join(tmpdir(), 'extended-release-'));
  try {
    for (const path of fixturePaths) {
      await mkdir(dirname(join(directory, path)), { recursive: true });
      await cp(new URL(path, source), join(directory, path), { recursive: true });
    }
    await check(directory);
    await run(directory);
  } finally { await rm(directory, { recursive: true, force: true }); }
}

async function editJson(directory, path, edit) {
  const absolute = join(directory, path);
  const value = JSON.parse(await readFile(absolute, 'utf8'));
  edit(value);
  await writeFile(absolute, JSON.stringify(value, null, 2) + '\n');
}

test('new upstream tracking cannot rewrite historical evidence or claim an unverified rebase', async () => {
  for (const [path, edit, expected] of [
    ['HARDENED-REVISION.json', value => { value.tracked_upstream_version = '0.3.47'; }, /Historical review evidence changed/],
    ['EXTENDED-RELEASE.json', value => { value.source_base_upstream_version = '0.3.47'; }, /Historical source ancestry drift/],
    ['EXTENDED-RELEASE.json', value => { value.full_upstream_parity = true; }, /Unsupported upstream parity claim/],
    ['EXTENDED-RELEASE.json', value => { value.upstream_code_rebased = true; }, /Unsupported upstream parity claim/],
  ]) {
    await fixture(async directory => {
      await editJson(directory, path, edit);
      await assert.rejects(check(directory), expected);
    });
  }
});

test('a tracked upstream advance requires bounded source identities, range and coverage', async () => {
  for (const [edit, expected] of [
    [value => { value.reviewed_upstream_range.through = '0.3.46'; }, /Reviewed upstream range drift/],
    [value => { value.integration_provenance.upstream_commit = 'not-a-commit'; }, /Missing integration commit identities/],
    [value => { value.integration_provenance.reference_archive_sha256 = 'not-a-digest'; }, /Missing reference archive identity/],
    [value => { value.integration_provenance.reference_files_verified = 0; }, /Missing reference comparison coverage/],
    [value => { value.integration_provenance.coverage_document = '../outside.md'; }, /Integration coverage path drift/],
  ]) {
    await fixture(async directory => {
      await editJson(directory, 'EXTENDED-RELEASE.json', edit);
      await assert.rejects(check(directory), expected);
    });
  }
  await fixture(async directory => {
    await writeFile(join(directory, 'docs/upstream-integration-0.5.0.md'), '# Missing source identities\n');
    await assert.rejects(check(directory), /Integration coverage omits source identity/);
  });
});

test('version disagreement and gaps in the integrated release history fail the gate', async () => {
  await fixture(async directory => {
    await editJson(directory, 'src-tauri/tauri.conf.json', value => { value.version = '0.4.0'; });
    await assert.rejects(check(directory), /Tauri version drift/);
  });
  await fixture(async directory => {
    await unlink(join(directory, 'releases/0.3.44.md'));
    await assert.rejects(check(directory), /Missing upstream historical reference 0\.3\.44/);
  });
  await fixture(async directory => {
    await editJson(directory, 'website/src/data/release-dates.json', value => { delete value['0.3.46']; });
    await assert.rejects(check(directory), /Missing upstream historical reference 0\.3\.46/);
  });
});

test('the historical evidence identity is unchanged by Windows checkout line endings', async () => {
  await fixture(async directory => {
    const path = join(directory, 'HARDENED-REVISION.json');
    await writeFile(path, (await readFile(path, 'utf8')).replaceAll('\n', '\r\n'));
    await assert.doesNotReject(check(directory));
  });
});
