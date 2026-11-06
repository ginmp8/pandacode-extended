import assert from 'node:assert/strict';
import test from 'node:test';
import { resolveReleaseIdentity } from './resolve-release-identity.mjs';
import { updaterManifest } from './common.mjs';

const notes = new Set(['releases/0.3.99.md', 'releases/extended/0.10.0.md', 'releases/extended/0.11.0.md']);
const scenario = (refType, refName, appVersion = '0.3.99') => resolveReleaseIdentity({ appVersion, refType, refName, hasNotes: file => notes.has(file) });

test('Extended v0.10.0 tag keeps upstream app version and selects Extended notes', () => {
  assert.deepEqual(scenario('tag', 'v0.10.0'), {
    version: '0.3.99', release_version: '0.10.0', release_notes: 'releases/extended/0.10.0.md',
    release_tag: 'v0.10.0', release_title: 'PandaCode Extended 0.10.0',
  });
});

test('future Extended tags work without hardcoding the upstream application version', () => {
  assert.equal(scenario('tag', 'v0.11.0').release_version, '0.11.0');
});

test('upstream app tags and manual branch builds keep the original release identity', () => {
  for (const [refType, refName] of [['tag', 'v0.3.99'], ['branch', 'base/development']]) {
    const actual = scenario(refType, refName);
    assert.equal(actual.version, '0.3.99');
    assert.equal(actual.release_version, '0.3.99');
    assert.equal(actual.release_notes, 'releases/0.3.99.md');
    assert.equal(actual.release_tag, 'v0.3.99');
  }
});

test('invalid tags, missing release notes and malformed app versions fail closed', () => {
  for (const tag of ['v0.12.0', '../secret', 'v0.10.0; echo x', 'v00.10.0', 'v0.11.0-beta', '0.11.0', 'v1.2.3/../../bad']) {
    assert.throws(() => scenario('tag', tag));
  }
  assert.throws(() => scenario('tag', 'v0.10.0', 'bad'), /SemVer/);
});

test('Extended updater keeps app SemVer but downloads from the Extended GitHub tag', () => {
  const manifest = updaterManifest('0.3.99', 'Extended notes', { 'windows-x86_64': 'sig-win' }, '2026-11-06T00:00:00Z', 'ginmp8/pandacode-extended', 'v0.10.0');
  assert.equal(manifest.version, '0.3.99');
  assert.equal(manifest.platforms['windows-x86_64'].url, 'https://github.com/ginmp8/pandacode-extended/releases/download/v0.10.0/PandaCode-Windows-x64-setup.exe');
  assert.throws(() => updaterManifest('0.3.99', '', { 'windows-x86_64': 'sig' }, undefined, undefined, 'v0.10.0; rm -rf /'), /SemVer/);
});
