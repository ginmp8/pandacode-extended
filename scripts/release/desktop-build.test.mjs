import test from 'node:test';
import assert from 'node:assert/strict';
import { buildPlan, builtArtifact, checkVersions, checksumLine, macosSigning, releaseAssetName } from './build.mjs';
import { appImageBundleOptions } from './common.mjs';

const env = { PANDACODE_LICENSE_PUBLIC_KEY: 'stale', PANDACODE_UPDATE_PUBLIC_KEY: 'stale', TAURI_SIGNING_PRIVATE_KEY: 'stale', APPLE_ID: 'someone', APPLE_SIGNING_IDENTITY: 'Developer ID Application: Someone', PATH: 'test-path' };
const config = plan => JSON.parse(plan.args[plan.args.indexOf('--config') + 1]);

test('builds require the native host and never forward old license keys or Apple variables to Windows and Linux', () => {
  for (const [target, platform] of [['windows-x86_64', 'win32'], ['windows-aarch64', 'win32'], ['linux-x86_64', 'linux']]) {
    const arch = target === 'windows-aarch64' ? 'arm64' : 'x64';
    const plan = buildPlan(target, env, platform, arch);
    for (const name of ['APPLE_ID', 'APPLE_SIGNING_IDENTITY', 'PANDACODE_LICENSE_PUBLIC_KEY', 'PANDACODE_UPDATE_PUBLIC_KEY']) assert.equal(plan.env[name], undefined, name);
    assert.equal(plan.env.PATH, 'test-path');
    assert.equal(plan.signing.mode, 'unsigned');
    // Local-only builds neither inherit updater signing secrets nor produce update artifacts.
    assert.equal(plan.env.TAURI_SIGNING_PRIVATE_KEY, undefined);
    assert.equal(config(plan).bundle.createUpdaterArtifacts, false);
    assert.equal(config(buildPlan(target, { PATH: 'p' }, platform, arch)).bundle.createUpdaterArtifacts, false);
    assert.equal(config(plan).plugins, undefined);
    assert.throws(() => buildPlan(target, env, 'darwin', 'arm64'), /native/);
  }
  assert.throws(() => buildPlan('darwin-aarch64', env, 'darwin', 'x64'), /native/);
  assert.throws(() => buildPlan('darwin-x86_64', env, 'darwin', 'arm64'), /native/);
  assert.ok(buildPlan('darwin-x86_64', { PATH: 'p' }, 'darwin', 'x64').args.includes('x86_64-apple-darwin'));
  assert.equal(env.APPLE_ID, 'someone', 'the caller environment is unchanged');
});

test('Windows installs per user without downloading WebView2 and links the C++ runtime statically', () => {
  const plan = buildPlan('windows-x86_64', { ...env, STATIC_VCRUNTIME: 'false' }, 'win32', 'x64');
  assert.equal(plan.env.STATIC_VCRUNTIME, 'true');
  assert.deepEqual(config(plan).bundle.windows.webviewInstallMode, { type: 'skip' });
  assert.equal(config(plan).bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(buildPlan('linux-x86_64', { STATIC_VCRUNTIME: 'false' }, 'linux', 'x64').env.STATIC_VCRUNTIME, 'false');
});

test('Linux bundles the external-link helper without changing Mac or Windows bundles', () => {
  const expected = { linux: { appimage: { files: { '/usr/bin/xdg-open': '/usr/bin/xdg-open' } } } };
  assert.deepEqual(appImageBundleOptions('linux-x86_64'), expected);
  assert.deepEqual(appImageBundleOptions('linux-aarch64'), expected);
  assert.deepEqual(config(buildPlan('linux-x86_64', env, 'linux', 'x64')).bundle.linux, expected.linux);
  assert.deepEqual(appImageBundleOptions('darwin-aarch64'), {});
  assert.equal(config(buildPlan('windows-x86_64', env, 'win32', 'x64')).bundle.linux, undefined);
});

test('macOS falls back to an ad-hoc signature and drops partial Apple credentials', () => {
  const plan = buildPlan('darwin-aarch64', { APPLE_ID: 'someone', PATH: 'p' }, 'darwin', 'arm64');
  assert.equal(plan.signing.mode, 'adhoc');
  assert.equal(plan.env.APPLE_SIGNING_IDENTITY, '-');
  assert.equal(plan.env.APPLE_ID, undefined);
  assert.equal(config(plan).bundle.macOS.signingIdentity, '-');
  assert.ok(plan.args.includes('app,dmg'));
});

test('Developer ID signing requires a real identity and notarization credentials', () => {
  const identity = 'Developer ID Application: Example (TEAM123)';
  assert.throws(() => macosSigning({ APPLE_SIGNING_IDENTITY: 'Apple Development: Someone' }), /Developer ID/);
  assert.throws(() => macosSigning({ APPLE_SIGNING_IDENTITY: identity }), /notarized/);
  assert.deepEqual(macosSigning({ APPLE_SIGNING_IDENTITY: identity, APPLE_API_KEY: 'k', APPLE_API_ISSUER: 'i', APPLE_API_KEY_PATH: '/k.p8' }), { mode: 'developer-id', identity });
  const plan = buildPlan('darwin-aarch64', { APPLE_SIGNING_IDENTITY: identity, APPLE_ID: 'a', APPLE_PASSWORD: 'p', APPLE_TEAM_ID: 't' }, 'darwin', 'arm64');
  assert.equal(plan.signing.mode, 'developer-id');
  assert.equal(plan.env.APPLE_ID, 'a');
});

test('Extended 0.5.1 packaging only accepts matching Extended-named Tauri bundles', () => {
  for (const [target, suffix] of [
    ['windows-x86_64', '_x64-setup.exe'],
    ['windows-aarch64', '_arm64-setup.exe'],
    ['linux-x86_64', '_amd64.AppImage'],
    ['linux-aarch64', '_aarch64.AppImage'],
    ['darwin-aarch64', '_aarch64.dmg'],
    ['darwin-x86_64', '_x64.dmg'],
  ]) {
    const spaced = `PandaCode Extended_0.5.1${suffix}`;
    const underscored = `PandaCode_Extended_0.5.1${suffix}`;
    assert.equal(builtArtifact([spaced, underscored], '0.5.1', target), spaced);
    assert.equal(builtArtifact([underscored], '0.5.1', target), underscored);
    assert.throws(() => builtArtifact([spaced], '0.3.42', target), /did not produce/);
    assert.throws(() => builtArtifact([`PandaCode_0.5.1${suffix}`], '0.5.1', target), /did not produce/);
  }
});

test('release builds require one matching three-component Extended SemVer', async () => {
  await assert.doesNotReject(checkVersions('0.5.1'));
  await assert.rejects(checkVersions('0.4.0'), /disagree on the version/);
  await assert.rejects(checkVersions('0.3.42'), /disagree on the version/);
  await assert.rejects(checkVersions('0.4.0.1'), /SemVer core/);
  await assert.rejects(checkVersions('0.4.0+pc.0.3.42'), /SemVer core/);
});

test('legacy release asset names stay reproducible and new Extended assets use their own identity', () => {
  assert.equal(releaseAssetName('0.3.34-hardened', 'PandaCode-Windows-x64-setup.exe'), 'PandaCode-0.3.34.hardened-Windows-x64-setup.exe');
  assert.equal(releaseAssetName('0.3.35-hardened.4', 'PandaCode-Windows-x64-setup.exe'), 'PandaCode-0.3.35.hardened.4-Windows-x64-setup.exe');
  assert.equal(releaseAssetName('1.2.3', 'PandaCode-Windows-x64-setup.exe'), 'PandaCode-Extended-1.2.3-Windows-x64-setup.exe');
  assert.equal(releaseAssetName('0.4.0', 'PandaCode-Windows-x64-setup.exe'), 'PandaCode-Extended-0.4.0-Windows-x64-setup.exe');
  assert.equal(releaseAssetName('0.5.1', 'PandaCode-Windows-x64-setup.exe'), 'PandaCode-Extended-0.5.1-Windows-x64-setup.exe');
});

test('checksum sidecars use the sha256sum format', () => {
  assert.equal(checksumLine('a'.repeat(64), 'PandaCode-Linux-x86_64.AppImage'), `${'a'.repeat(64)}  PandaCode-Linux-x86_64.AppImage\n`);
  assert.throws(() => checksumLine('nothex', 'x'), /SHA-256/);
});
