import test from 'node:test';
import assert from 'node:assert/strict';

// The hardened update policy must survive old preferences and calls from stale UI.
const { readyToRestart, describeUpdate, IDLE_BEFORE_RESTART } = await import('../src/app-updates-policy.ts');
const { startAppUpdates, appUpdates } = await import('../src/app-updates.ts');

test('the hardened build never restarts to install an external update', () => {
  const now = 10_000_000;
  assert.equal(readyToRestart(false, now - IDLE_BEFORE_RESTART, now), false);
  assert.equal(readyToRestart(false, now - IDLE_BEFORE_RESTART + 1, now), false);
  assert.equal(readyToRestart(true, now - 60 * 60_000, now), false);
});

test('old automatic-update preferences and direct controller calls cannot enable networking', async () => {
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  const originalFetch = globalThis.fetch;
  const originalTimeout = globalThis.setTimeout, originalInterval = globalThis.setInterval;
  const unexpected = () => { throw new Error('Disabled updates must not access storage, network or timers'); };
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, get: unexpected });
  globalThis.fetch = unexpected;
  globalThis.setTimeout = unexpected;
  globalThis.setInterval = unexpected;
  try {
    const updater = startAppUpdates({ busy: unexpected, notify: unexpected });
    assert.equal(appUpdates(), updater);
    assert.equal(updater.automatic, false);
    assert.equal(updater.supported, false);
    assert.deepEqual(updater.state, { kind: 'off' });
    const unsubscribe = updater.subscribe(unexpected);
    updater.setAutomatic(true);
    await updater.check();
    await updater.restart();
    unsubscribe();
    assert.equal(updater.automatic, false);
    assert.deepEqual(updater.state, { kind: 'off' });
    assert.match(describeUpdate(updater.state, true), /disabled in this hardened build/);
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.setTimeout = originalTimeout;
    globalThis.setInterval = originalInterval;
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage);
    else delete globalThis.localStorage;
  }
});

test('About explains each state in one line', () => {
  const now = Date.parse('2026-10-18T12:00:00Z');
  assert.equal(describeUpdate({ kind: 'current', checkedAt: now - 5 * 60_000 }, 'automatic', now), 'Up to date · checked 5m ago');
  assert.equal(describeUpdate({ kind: 'downloading', version: '0.3.1', percent: 42 }, 'automatic'), 'Downloading 0.3.1 · 42%');
  assert.match(describeUpdate({ kind: 'ready', version: '0.3.1' }, 'automatic'), /^0\.3\.1 is ready\. PandaCode restarts when your agents are done/);
  assert.equal(describeUpdate({ kind: 'ready', version: '0.3.1' }, 'notify'), '0.3.1 is ready. Restart to finish.');
  assert.equal(describeUpdate({ kind: 'idle' }, 'off'), 'Automatic updates are off.');
  assert.equal(describeUpdate({ kind: 'idle' }, 'notify'), 'Checks GitHub Releases automatically.');
  assert.match(describeUpdate({ kind: 'available', version: '0.3.1' }, 'automatic', now, false), /installed by its package or a new AppImage/);
  assert.match(describeUpdate({ kind: 'error', message: 'offline' }, 'automatic'), /offline$/);
});
