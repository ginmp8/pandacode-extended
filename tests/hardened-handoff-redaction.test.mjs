import test from 'node:test';
import assert from 'node:assert/strict';
import { buildHandoff, redactForHandoff } from '../src/hardened/review-model.ts';

test('private-key text without an end marker is not included in the handoff', () => {
  const result = redactForHandoff('Context\n-----BEGIN PRIVATE KEY-----\nSYNTHETIC_SECRET_MATERIAL');
  assert.ok(!result.includes('SYNTHETIC_SECRET_MATERIAL'));
  assert.match(result, /REDACTED/);
});
test('checkpoint redaction happens before clipping a long private-key block', () => {
  const result = buildHandoff({ task: { id: 'task', title: 'Review', provider: 'codex', mode: 'plan' }, projectName: 'Demo',
    team: { sessionId: 'task', workers: [{ name: 'Reviewer', checkpoint: { summary: '-----BEGIN PRIVATE KEY-----\n' + 'SYNTHETIC_SECRET_MATERIAL\n'.repeat(40) + '-----END PRIVATE KEY-----', nextSteps: '', blocker: null } }] } },
    { goal: 'Review parser' }, true);
  assert.ok(!result.includes('SYNTHETIC_SECRET_MATERIAL'));
  assert.match(result, /REDACTED/);
});
