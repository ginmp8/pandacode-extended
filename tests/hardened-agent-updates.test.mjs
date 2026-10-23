import test from 'node:test';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';

// CSS has no behavior in this controller. Load the actual TypeScript controller
// under Node to exercise its public API without a browser, IPC or network.
const hooks = registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith('/src/agent-updates.css')) return { format: 'module', source: '', shortCircuit: true };
    return nextLoad(url, context);
  },
});
const { initializeAgentUpdates } = await import('../src/agent-updates.ts');
hooks.deregister();

test('agent update controller shows policy without checks, timers or installation', async () => {
  const originalFetch = globalThis.fetch;
  const originalTimeout = globalThis.setTimeout, originalInterval = globalThis.setInterval;
  const forbidden = () => { throw new Error('The disabled updater must not perform this operation'); };
  const listeners = new Map();
  let html = '', closed = 0, focused = 0, opened = 0;
  const button = {
    addEventListener: (name, callback) => listeners.set(name, callback),
    removeEventListener: name => listeners.delete(name),
    focus: () => { focused++; },
  };
  const host = {
    modal: (title, body) => { assert.equal(title, 'Agent updates'); html = body; opened++; return { querySelector: () => button }; },
    closeModal: () => { closed++; },
    providersChanged: forbidden,
    providers: forbidden,
    report: forbidden,
  };
  globalThis.fetch = forbidden;
  globalThis.setTimeout = forbidden;
  globalThis.setInterval = forbidden;
  try {
    const controller = initializeAgentUpdates(host);
    assert.equal(opened, 0);
    await controller.show();
    assert.match(html, /External agent update checks and installers are disabled/);
    assert.doesNotMatch(html, /agent-update-button|agent-updates-check/);
    assert.equal(focused, 1);
    listeners.get('click')();
    assert.equal(closed, 1);
    await controller.show();
    assert.equal(listeners.size, 1);
    controller.dispose();
    assert.equal(listeners.size, 0);
    await controller.show();
    assert.equal(opened, 2);
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.setTimeout = originalTimeout;
    globalThis.setInterval = originalInterval;
  }
});
