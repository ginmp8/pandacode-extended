import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { coverage, generate, inventoryStatus, packagedAssets, scanStatus } from './sbom.mjs';

test('inventory only selects this Extended version installers, without updater archives', () => {
  assert.deepEqual(packagedAssets(['PandaCode-macOS-arm64.dmg', 'PandaCode-Extended-0.4.0-macOS-arm64.dmg',
    'PandaCode-Extended-0.5.0-macOS-arm64.dmg', 'PandaCode-Extended-0.5.0-macOS-arm64.app.tar.gz',
    'PandaCode-Extended-0.5.0-Linux-x86_64.AppImage', 'latest.json'], '0.5.0'), [
    { name: 'PandaCode-Extended-0.5.0-Linux-x86_64.AppImage', platform: 'linux/x64' },
    { name: 'PandaCode-Extended-0.5.0-macOS-arm64.dmg', platform: 'darwin/arm64' },
  ]);
  assert.throws(() => packagedAssets([], '0.5.0+pc.0.3.47'), /SemVer core/);
});

test('malformed or empty inventory/scan data never means clean', () => {
  const base = { bomFormat: 'CycloneDX', specVersion: '1.6', version: 1 };
  for (const bom of [undefined, {}, { components: [{}] }, { ...base, components: [] }, { ...base, components: [null] }, { ...base, components: [{}] }]) {
    assert.equal(inventoryStatus({ bom }).status, 'incomplete');
  }
  const inventory = inventoryStatus({ bom: { ...base, components: [{ name: 'fixture', type: 'library' }] } });
  const descriptor = { name: 'grype', version: 'fixture' };
  for (const report of [undefined, {}, { matches: null }, { matches: 'not an array' }, { matches: [] }, { descriptor, matches: [null] }, { descriptor, matches: [{}] }]) {
    assert.equal(scanStatus(inventory, { report }).status, 'failed');
  }
  assert.equal(scanStatus({ status: 'incomplete' }, { report: { matches: [] } }).status, 'not-scanned');
  const artifact = { name: 'fixture', type: 'npm' };
  const report = scanStatus(inventory, { report: { descriptor, matches: [
    { vulnerability: { id: 'fixture-advisory-a', severity: 'High' }, artifact },
    { vulnerability: { id: 'fixture-advisory-b', severity: 'Unspecified' }, artifact },
  ] } });
  assert.equal(report.findings, 2);
  assert.equal(report.severities.High, 1);
  assert.equal(report.severities.Unknown, 1);
  assert.equal(scanStatus(inventory, { report: { descriptor, matches: [] } }).findings, 0);
});

test('dependency-only output does not imply installer coverage', () => {
  assert.equal(coverage({ entries: [], notes: [] }).complete, false);
  assert.equal(coverage({ entries: [{ name: 'lock', kind: 'source dependencies', inventory: { status: 'ok' }, scan: { status: 'scanned' } }], notes: [] }).complete, false);
});

test('the complete local flow overrides inherited network options and records scanner failure honestly', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'extended-sbom-'));
  try {
    const sourceRoot = join(scratch, 'source');
    const assets = join(scratch, 'assets');
    const out = join(scratch, 'report');
    await mkdir(join(sourceRoot, 'src-tauri'), { recursive: true });
    await mkdir(assets);
    await writeFile(join(sourceRoot, 'package.json'), '{"version":"0.5.0"}');
    for (const path of ['package-lock.json', 'src-tauri/Cargo.lock', 'src-tauri/Cargo.toml']) await writeFile(join(sourceRoot, path), 'fixture');
    await writeFile(join(assets, 'PandaCode-Extended-0.5.0-Linux-x86_64.AppImage'), 'fixture installer');
    const calls = [];
    const runner = async (name, args, options) => {
      calls.push({ name, args });
      assert.equal(options.env.SYFT_CHECK_FOR_APP_UPDATE, 'false');
      assert.equal(options.env.GRYPE_CHECK_FOR_APP_UPDATE, 'false');
      assert.equal(options.env.GRYPE_DB_AUTO_UPDATE, 'false');
      assert.equal(options.env.SYFT_JAVASCRIPT_SEARCH_REMOTE_LICENSES, 'false');
      assert.equal(options.env.SYFT_ENRICH, undefined);
      assert.equal(options.env.GRYPE_ONLY_FIXED, undefined);
      assert.equal(options.env.GRYPE_DB_CACHE_DIR, '/existing/local/db');
      assert.match(await readFile(args[1], 'utf8'), /enrich: \[\]/);
      assert.equal(options.timeout, 300_000);
      if (args.includes('version')) return { stdout: '{"version":"fixture"}' };
      if (args.includes('status')) return { stdout: '{"built":"2026-10-25T00:00:00Z"}' };
      if (name === 'syft') return { stdout: '{"bomFormat":"CycloneDX","specVersion":"1.6","version":1,"components":[{"name":"fixture","type":"library"}]}' };
      return { stdout: 'malformed output must not become zero findings' };
    };
    const report = await generate({ sourceRoot, assets, out, version: '0.5.0', commit: '1'.repeat(40), runner,
      inherited: { PATH: '/installed/tools', SYFT_ENRICH: 'all', SYFT_CHECK_FOR_APP_UPDATE: 'true', GRYPE_DB_AUTO_UPDATE: 'true', GRYPE_ONLY_FIXED: 'true', GRYPE_DB_CACHE_DIR: '/existing/local/db' } });
    assert.equal(report.complete, false);
    assert.equal(report.entries.length, 3);
    assert.ok(report.entries.every(entry => entry.scan.status === 'failed'));
    assert.ok(calls.every(call => !call.args.includes('update') && !call.args.includes('db:pull')));
    assert.match(await readFile(join(out, 'SBOM-COVERAGE.md'), 'utf8'), /Coverage is incomplete/);
    assert.equal(report.entries[0].sha256.length, 64);
    assert.deepEqual(Object.keys(report.entries[1].sourceHashes), ['package.json', 'package-lock.json']);
    await assert.rejects(generate({ sourceRoot, assets, out, version: '0.5.0', commit: '1'.repeat(40), runner }), /EEXIST/);
  } finally { await rm(scratch, { recursive: true, force: true }); }
});
