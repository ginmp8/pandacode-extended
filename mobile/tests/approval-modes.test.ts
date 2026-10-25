/// <reference types="node" />
import assert from 'node:assert/strict';
import test from 'node:test';
import { permissionModes, permissionNotice, supportsPermissionMode } from '../src/ui/approval-modes.ts';

test('only Claude-family harnesses offer the explicit review mode', () => {
  for (const provider of ['claude', 'glm', 'mimo']) {
    const modes = permissionModes(provider);
    assert.equal(modes[0].id, 'ask');
    assert.equal(modes.find(mode => mode.id === 'auto')?.label, 'Auto-approve edits');
    assert.deepEqual(modes.find(mode => mode.id === 'review'), { id: 'review', label: 'Approve for me', available: true });
    assert.equal(supportsPermissionMode(provider, 'review'), true);
    assert.match(permissionNotice(provider, 'review')!, /Full access grant locally/);
  }
});

test('a saved review choice stays visible but cannot launch on another harness', () => {
  for (const provider of ['codex', 'kimi', 'opencode', 'antigravity', 'grok', 'pi', 'omp', 'deepseek', 'commandcode', 'cline', 'kilo', 'github-copilot', 'muse', 'devin', 'unknown']) {
    assert.equal(permissionModes(provider).some(mode => mode.id === 'review'), false, provider);
    const saved = permissionModes(provider, 'review').find(mode => mode.id === 'review');
    assert.equal(saved?.available, false, provider);
    assert.match(saved!.label, /unavailable/);
    assert.equal(supportsPermissionMode(provider, 'review'), false, provider);
    assert.match(permissionNotice(provider, 'review')!, /Choose another/);
  }
});

test('review never changes the existing modes or becomes a default or Full access alias', () => {
  assert.deepEqual(permissionModes('devin').map(mode => mode.id), ['ask', 'auto', 'plan', 'full']);
  for (const mode of ['ask', 'auto', 'plan', 'full']) {
    assert.equal(supportsPermissionMode('claude', mode), true);
    assert.equal(permissionNotice('claude', mode), undefined);
  }
  assert.equal(supportsPermissionMode('claude', ''), false);
  assert.equal(supportsPermissionMode('claude', 'automatic'), false);
});
