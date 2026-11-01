import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = path => readFile(resolve(root, path), 'utf8');
const json = async path => JSON.parse(await read(path));
const gitSha = content => {
  const bytes = Buffer.from(content, 'utf8');
  return createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');
};
function cargoVersion(content, packageName) {
  const sections = content.split(/(?=^\[(?:\[)?[a-z][^\n]*\])/m);
  const section = sections.find(s => s.startsWith(packageName ? '[[package]]' : '[package]') && (!packageName || new RegExp(`^name = "${packageName}"$`, 'm').test(s)));
  return section?.match(/^version = "([^"]+)"/m)?.[1];
}
const semver = /^\d+\.\d+\.\d+$/;
const cmp = (a, b) => a.split('.').map(Number).reduce((r, n, i) => r || n - Number(b.split('.')[i]), 0);

test('application identity matches pinned upstream source release', async () => {
  const source = await json('docs/extended/upstream/source-baseline.json');
  assert.equal(source.source_fork, 'ginmp8/pandacode');
  assert.match(source.version, semver);
  assert.match(source.release_sha, /^[a-f0-9]{40}$/);
  assert.match(source.tree_sha, /^[a-f0-9]{40}$/);
  const pkg = await json('package.json');
  const lock = await json('package-lock.json');
  const tauri = await json('src-tauri/tauri.conf.json');
  const cargo = await read('src-tauri/Cargo.toml');
  const cargoLock = await read('src-tauri/Cargo.lock');
  const values = {
    'package.json': pkg.version,
    'package-lock.json root': lock.version,
    'package-lock.json packages[""]': lock.packages?.['']?.version,
    'src-tauri/Cargo.toml': cargoVersion(cargo),
    'src-tauri/Cargo.lock pandacode': cargoVersion(cargoLock, 'pandacode'),
    'src-tauri/tauri.conf.json': tauri.version,
  };
  for (const [path, actual] of Object.entries(values)) assert.equal(actual, source.version, `${path} must match upstream ${source.version}`);
});

test('historical release notes and dates retain original upstream Git blobs', async () => {
  const source = await json('docs/extended/upstream/source-baseline.json');
  const releases = (await readdir(resolve(root, 'releases'))).filter(x => x.endsWith('.md') && semver.test(x.slice(0, -3))).map(x => x.slice(0, -3)).sort(cmp);
  assert.equal(releases.at(-1), source.version, 'latest historical release should match imported upstream source version');
  for (const [version, hash] of Object.entries(source.release_notes)) {
    assert.match(hash, /^[a-f0-9]{40}$/);
    const note = await read(`releases/${version}.md`);
    assert.ok(note.trim());
    assert.equal(gitSha(note), hash, `release notes ${version} must match pinned original`);
  }
  const dates = await read('website/src/data/release-dates.json');
  assert.match(JSON.parse(dates)[source.version], /^\d{4}-\d{2}-\d{2}$/);
  assert.equal(gitSha(dates), source.release_dates_sha, 'release dates must match pinned original');
});
