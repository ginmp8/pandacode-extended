import test from 'node:test';
import assert from 'node:assert/strict';
import { approvalModes, approvalMode } from '../src/approval-modes.ts';
import { advertisedModelsOnly, modelChoices } from '../src/model-catalog.ts';
import { historicalProvider } from '../src/provider-handoff.ts';
import { buildReview, reviewProblems } from '../src/hardened/review-model.ts';

test('Devin offers its three native permission choices without silently widening legacy tasks', () => {
  assert.deepEqual(approvalModes('devin').map(mode => mode.id), ['ask', 'plan', 'full']);
  assert.match(approvalMode('devin', 'ask').description, /Code mode/);
  assert.match(approvalMode('devin', 'plan').description, /approve the plan/);
  assert.match(approvalMode('devin', 'full').description, /Bypass Permissions/);
  assert.equal(approvalMode('devin', 'auto').id, 'ask');
  // The existing stricter Copilot plan description must not widen to fetch.
  assert.doesNotMatch(approvalMode('github-copilot', 'plan').description, /fetch/);
});

test('Devin keeps advertised models and each model-specific reasoning catalog', () => {
  assert.equal(advertisedModelsOnly('devin'), true);
  assert.equal(advertisedModelsOnly('github-copilot'), true);
  assert.equal(advertisedModelsOnly('claude'), false);
  const catalog = [
    { id: 'swe-2-high', name: 'SWE-2', description: '', reasoningEfforts: [{ id: 'high', name: 'High' }], defaultReasoningEffort: 'high' },
    { id: 'text-only', name: 'Text only model', description: 'Text only', reasoningEfforts: [] },
  ];
  const choices = modelChoices('devin', catalog, 'swe-2-high');
  assert.deepEqual(choices.map(model => model.id), ['default', 'swe-2-high', 'text-only']);
  assert.deepEqual(choices[1].reasoningEfforts, catalog[0].reasoningEfforts);
  assert.equal(choices[2].description, 'Text only');
  // A saved, unavailable ID remains visible rather than replacing historical state.
  assert.equal(modelChoices('devin', catalog, 'previous-model').at(-1).id, 'previous-model');
});

test('saved Devin responses retain their author after switching harness', () => {
  const message = { id: 1, sessionId: 'task', role: 'assistant', content: 'Review result', createdAt: 1, authorProvider: 'devin' };
  assert.equal(historicalProvider(message, [message], 'codex'), 'devin');
});

test('Extended manual review accepts installed Devin roles without relaxing admission', () => {
  const request = { kind: 'advisor', provider: 'devin', model: 'swe-2-high', question: 'Review this change', files: 'src/flow.ts', acceptance: 'Report confirmed regressions' };
  const { setup, prompt } = buildReview(request);
  const task = { id: 't', projectId: 'p', provider: 'devin', model: 'swe-2-high', mode: 'plan', status: 'idle', taskStage: 'pending', archived: false };
  const team = { sessionId: 't', status: 'ready', round: 0, workers: [], pendingSetup: null, config: structuredClone(setup.config) };
  const providers = [{ id: 'devin', name: 'Devin CLI', installed: true, models: [{ id: 'swe-2-high', name: 'SWE-2' }] }];
  assert.equal(setup.architect.provider, 'devin');
  assert.equal(setup.config.workers[0].provider, 'devin');
  assert.match(prompt, /NOT isolated/);
  const context = { task, team, providers, hasMessages: false };
  assert.deepEqual(reviewProblems(context, request), []);
  assert.match(reviewProblems({ ...context, task: { ...task, mode: 'full' } }, request).join(' '), /plan/);
  assert.match(reviewProblems({ ...context, providers: [{ ...providers[0], installed: false }] }, request).join(' '), /installed devin/);
});
