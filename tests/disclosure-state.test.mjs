import test from 'node:test';
import assert from 'node:assert/strict';
const data = new Map();
globalThis.localStorage = {
  getItem: key => data.get(key) ?? null,
  setItem: (key, value) => { data.set(key, value); },
};
const fresh = suffix => import(`../src/disclosure-state.ts?${suffix}`);

test('manual disclosure survives module reload and stays scoped by task, key and kind', async () => {
  const first = await fresh('session-a');
  assert.equal(first.disclosureChoice('a', 'tool', 'tool:t1'), undefined);
  first.rememberDisclosure('a', 'tool', 'tool:t1', true);
  first.rememberDisclosure('a', 'raw', 'tool:t1', false);
  const second = await fresh('session-b');
  assert.equal(second.disclosureChoice('a', 'tool', 'tool:t1'), true);
  assert.equal(second.disclosureChoice('a', 'raw', 'tool:t1'), false);
  assert.equal(second.disclosureChoice('b', 'tool', 'tool:t1'), undefined);
  assert.equal(second.disclosureChoice('a', 'tool', 'tool:t2'), undefined);
  second.rememberDisclosure('a', 'tool', 'tool:t1', false);
  assert.equal((await fresh('session-c')).disclosureChoice('a', 'tool', 'tool:t1'), false);
});

test('corrupt storage and invalid keys fail closed without dropping UI choice', async () => {
  data.set('pandacode.traceDisclosure.v1', '{invalid-json');
  const api = await fresh('corrupt');
  assert.equal(api.disclosureChoice('valid', 'turn', 'turn:a'), undefined);
  api.rememberDisclosure('', 'turn', 'turn:a', true);
  api.rememberDisclosure('valid', 'turn', '', true);
  assert.equal(data.get('pandacode.traceDisclosure.v1'), '{invalid-json');
  api.rememberDisclosure('valid', 'turn', 'turn:a', true);
  assert.equal(api.disclosureChoice('valid', 'turn', 'turn:a'), true);
});

test('bounded retention prevents unbounded storage growth', async () => {
  data.clear();
  const api = await fresh('bounded');
  for (let i = 0; i < 1030; i++) api.rememberDisclosure('s', 'tool', 'key:' + i, true);
  const stored = JSON.parse(data.get('pandacode.traceDisclosure.v1'));
  assert.equal(stored.length, 1024);
  assert.equal(api.disclosureChoice('s', 'tool', 'key:0'), undefined);
  assert.equal(api.disclosureChoice('s', 'tool', 'key:1029'), true);
});

test('blocked persistence retains state in memory for current session', async () => {
  data.clear();
  const api = await fresh('blocked');
  globalThis.localStorage.setItem = () => { throw new Error('disabled'); };
  api.rememberDisclosure('s', 'group', 'group:x', true);
  assert.equal(api.disclosureChoice('s', 'group', 'group:x'), true);
});
