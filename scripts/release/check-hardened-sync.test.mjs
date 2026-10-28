import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { cp, mkdtemp, mkdir, readFile, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { check } from './check-hardened-sync.mjs';

test('Extended core/display version, historical ancestry, layout patch and separated release histories stay consistent', async () => {
  assert.deepEqual(await check(), { version: '0.7.0', display: '0.7.0+pc.0.3.50', upstream: '0.3.50', sourceBase: '0.3.28', upstreamNotes: 100, extendedNotes: 12 });
});

const source = new URL('../../', import.meta.url);
const fixturePaths = ['HARDENED-REVISION.json', 'EXTENDED-RELEASE.json', 'package.json', 'package-lock.json',
  'mobile/package.json', 'mobile/package-lock.json', 'mobile/app.json',
  'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'website/src/data/release-dates.json',
  'src/styles.css', 'src/team-progress.css', 'tests/architect.spec.ts', 'docs/upstream-integration-0.5.0.md',
  'docs/upstream-parity-0.3.28-0.3.42.md', 'docs/upstream-release-evidence-0.3.28-0.3.42.json',
  'docs/upstream-parity-before-0.3.28.md', 'docs/upstream-release-evidence-before-0.3.28.json', 'docs/upstream-integration-0.5.1.md', 'docs/upstream-integration-0.6.0.md', 'releases'];

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
    await editJson(directory, 'EXTENDED-RELEASE.json', value => { value.incremental_upstream_review.upstream_through_commit = '0'.repeat(40); });
    await assert.rejects(check(directory), /Incremental upstream identity drift/);
  });
  await fixture(async directory => {
    await writeFile(join(directory, 'releases/0.3.48.md'), '# Changed source note\\n');
    await assert.rejects(check(directory), /Incremental upstream source note changed/);
  });
  await fixture(async directory => {
    await writeFile(join(directory, 'docs/upstream-integration-0.6.0.md'), '# Missing source identities\\n');
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

test('mobile manifests cannot silently diverge from the Extended release', async () => {
  for (const [path, edit] of [
    ['mobile/package.json', value => { value.version = '1.0.0'; }],
    ['mobile/package-lock.json', value => { value.packages[''].version = '0.4.0'; }],
    ['mobile/app.json', value => { value.expo.version = '0.5.0+pc.0.3.47'; }],
  ]) {
    await fixture(async directory => {
      await editJson(directory, path, edit);
      await assert.rejects(check(directory), /Mobile version drift/);
    });
  }
});

test('the version command advances desktop and mobile together without rewriting published history', async () => {
  await fixture(async directory => {
    for (const name of ['set-version.mjs', 'common.mjs', 'extended-version.mjs']) {
      const target = join(directory, 'scripts/release', name);
      await mkdir(dirname(target), { recursive: true });
      await cp(new URL(name, import.meta.url), target);
    }
    const historical = await readFile(join(directory, 'HARDENED-REVISION.json'), 'utf8');
    const published = await readFile(join(directory, 'releases/extended/0.4.0.md'), 'utf8');
    execFileSync(process.execPath, [join(directory, 'scripts/release/set-version.mjs'), '1.2.3'], { stdio: 'pipe' });
    for (const path of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json', 'mobile/package.json', 'mobile/package-lock.json']) {
      const value = JSON.parse(await readFile(join(directory, path), 'utf8'));
      assert.equal(value.version, '1.2.3', path);
      if (value.packages?.['']) assert.equal(value.packages[''].version, '1.2.3', path);
    }
    const app = JSON.parse(await readFile(join(directory, 'mobile/app.json'), 'utf8'));
    assert.equal(app.expo.version, '1.2.3');
    assert.equal(app.expo.updates.enabled, false);
    assert.equal(await readFile(join(directory, 'HARDENED-REVISION.json'), 'utf8'), historical);
    assert.equal(await readFile(join(directory, 'releases/extended/0.4.0.md'), 'utf8'), published);
    // Follow the documented workflow: add the new product note, retaining reviewed upstream evidence.
    await writeFile(join(directory, 'releases/extended/1.2.3.md'), '# Extended 1.2.3\n\nProduct-only patch.\n');
    const metadata = JSON.parse(await readFile(join(directory, 'EXTENDED-RELEASE.json'), 'utf8'));
    assert.equal(metadata.integration_provenance.coverage_document, 'docs/upstream-integration-0.6.0.md');
    assert.deepEqual(await check(directory), { version: '1.2.3', display: '1.2.3+pc.0.3.50', upstream: '0.3.50', sourceBase: '0.3.28', upstreamNotes: 100, extendedNotes: 13 });
  });
});

test('the historical evidence identity is unchanged by Windows checkout line endings', async () => {
  await fixture(async directory => {
    for (const file of ['HARDENED-REVISION.json', 'docs/upstream-release-evidence-0.3.28-0.3.42.json', 'docs/upstream-release-evidence-before-0.3.28.json', 'releases/0.2.4.md', 'releases/0.3.27.md', 'releases/0.3.28.md', 'releases/0.3.36.md']) {
      const path = join(directory, file);
      await writeFile(path, (await readFile(path, 'utf8')).replaceAll('\n', '\r\n'));
    }
    await assert.doesNotReject(check(directory));
  });
});

test('historical review cannot silently omit a release or rewrite its frozen evidence', async () => {
  for (const [edit, expected] of [
    [value => { delete value.historical_upstream_review; }, /Historical coverage range drift/],
    [value => { value.historical_upstream_review.from = '0.3.29'; }, /Historical coverage range drift/],
    [value => { value.historical_upstream_review.extended_base_commit = value.integration_provenance.extended_base_commit; }, /Historical coverage commit drift/],
    [value => { value.historical_upstream_review.release_evidence = '../outside.json'; }, /Historical coverage path drift/],
  ]) {
    await fixture(async directory => {
      await editJson(directory, 'EXTENDED-RELEASE.json', edit);
      await assert.rejects(check(directory), expected);
    });
  }
  await fixture(async directory => {
    await writeFile(join(directory, 'releases/0.3.28.md'), '# Rewritten reference\n');
    await assert.rejects(check(directory), /Historical source note changed: 0\.3\.28/);
  });
  await fixture(async directory => {
    await editJson(directory, 'docs/upstream-release-evidence-0.3.28-0.3.42.json', value => { value.notes.splice(8, 1); });
    await assert.rejects(check(directory), /Historical release evidence digest drift/);
  });
});

test('early historical review cannot omit its range, change its baseline or rewrite original notes', async () => {
  for (const [edit, expected] of [
    [value => { delete value.early_upstream_review; }, /Early coverage range drift/],
    [value => { value.early_upstream_review.from = '0.3.0'; }, /Early coverage range drift/],
    [value => { value.early_upstream_review.extended_base_commit = value.historical_upstream_review.extended_base_commit; }, /Early coverage commit drift/],
    [value => { value.early_upstream_review.release_evidence = '../outside.json'; }, /Early coverage path drift/],
  ]) {
    await fixture(async directory => {
      await editJson(directory, 'EXTENDED-RELEASE.json', edit);
      await assert.rejects(check(directory), expected);
    });
  }
  for (const version of ['0.2.4', '0.3.27']) {
    await fixture(async directory => {
      await writeFile(join(directory, `releases/${version}.md`), '# Rewritten reference\n');
      await assert.rejects(check(directory), new RegExp(`Early source note changed: ${version.replaceAll('.', '\\.')}`));
    });
  }
});

test('early source-only evidence cannot invent publication or silently drop a note even with a new digest', async () => {
  for (const [edit, expected] of [
    [value => { value.notes.splice(4, 1); }, /Early release evidence coverage gap/],
    [value => { value.notes[0].published_at = '2026-10-01T00:00:00Z'; }, /Early source-only note claims publication/],
    [value => { value.notes[49].release_url = 'https://example.invalid/release'; }, /Early published release evidence missing/],
  ]) {
    await fixture(async directory => {
      const path = 'docs/upstream-release-evidence-before-0.3.28.json';
      await editJson(directory, path, edit);
      const digest = createHash('sha256').update(await readFile(join(directory, path))).digest('hex');
      await editJson(directory, 'EXTENDED-RELEASE.json', value => { value.early_upstream_review.release_evidence_sha256 = digest; });
      await assert.rejects(check(directory), expected);
    });
  }
});
