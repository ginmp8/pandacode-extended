import test from 'node:test';
import assert from 'node:assert/strict';
import { extendedVersion, upstreamReference, displayVersion, releaseNotePath, releaseDateKey, sortReleaseDates } from './extended-version.mjs';

test('Extended technical version is a strict SemVer core, while the display metadata identifies tracked PandaCode', () => {
  assert.equal(extendedVersion('0.4.0'), '0.4.0');
  assert.equal(displayVersion('0.4.0', '0.3.42'), '0.4.0+pc.0.3.42');
  assert.equal(releaseNotePath('0.4.1'), 'releases/extended/0.4.1.md');
  assert.equal(releaseDateKey('0.4.1'), 'extended/0.4.1');
  for (const bad of ['0.4.0.1', '0.4.0+pc.0.3.42', '0.4.0-rc.1', '01.4.0', '0.4.0/../../x', undefined, '0.4.0;echo x']) {
    assert.throws(() => extendedVersion(bad), /SemVer core/);
  }
  assert.equal(upstreamReference('0.3.42'), '0.3.42');
  assert.throws(() => upstreamReference('0.3.42-hardened'), /Tracked PandaCode/);
});

test('dates are independently keyed and numerically sorted in both namespaces', () => {
  assert.deepEqual(Object.keys(sortReleaseDates({
    'extended/0.4.1': '2026-10-27',
    '0.3.42': '2026-10-25',
    'extended/0.3.35-hardened.3': '2026-10-24',
    '0.3.9': '2026-10-21',
    'extended/0.4.0': '2026-10-25',
    '0.4.0': '2026-11-01',
  })), ['0.3.9', '0.3.42', '0.4.0', 'extended/0.3.35-hardened.3', 'extended/0.4.0', 'extended/0.4.1']);
  for (const bad of ['../secret', 'extended/../secret', 'extended/0.4.0/extra', 'extended/0.4.0+pc.0.3.42']) {
    assert.throws(() => sortReleaseDates({ [bad]: '2026-10-25' }));
  }
});
