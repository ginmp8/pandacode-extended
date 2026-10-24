/** 0.3.35.4: native admission remains fail-closed; manual multi-harness roles use
 * existing profile/scheduler only. Do not execute any provider or project scripts. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const feature = () => import('../src/hardened/review-model.ts');
const catalog = [
  { id: 'codex', name: 'Codex', installed: true, models: [{ id: 'c-model', name: 'Codex model' }] },
  { id: 'claude', name: 'Claude', installed: true, models: [{ id: 'a-model', name: 'Claude model' }] },
  { id: 'kimi', name: 'Kimi', installed: false, models: [{ id: 'k-model', name: 'Kimi model' }] },
];
const request = (extra = {}) => ({
  kind: 'committee', provider: 'claude', model: 'a-model',
  question: 'Review one flow', acceptance: 'Report risky state transitions', files: 'src/flow.ts',
  reviewers: [{ id: 'correctness', provider: 'codex', model: 'c-model' }, { id: 'risk', provider: 'claude', model: 'a-model' }],
  ...extra,
});
const task = (extra = {}) => ({
  id: 't1', projectId: 'p', provider: 'claude', model: 'a-model', mode: 'plan', status: 'idle',
  taskStage: 'pending', archived: false, resumeId: null, lastResponseAt: null, ...extra,
});
const team = (setup, extra = {}) => ({
  sessionId: 't1', status: 'ready', round: 0, workers: [], pendingSetup: null,
  config: structuredClone(setup.config), ...extra,
});

test('committee roles preserve selected providers and fixed native budgets', async () => {
  const { buildReview } = await feature();
  const p = buildReview(request());
  assert.deepEqual(p.setup.architect, { provider: 'claude', model: 'a-model', reasoningEffort: null });
  assert.deepEqual(p.setup.config.workers.map(w => [w.id, w.provider, w.model]),
    [['correctness', 'codex', 'c-model'], ['risk', 'claude', 'a-model']]);
  assert.deepEqual([p.setup.config.maxParallel, p.setup.config.maxAssignments, p.setup.config.maxRounds], [2, 2, 2]);
  assert.equal(p.setup.config.rosterOnly, true);
  assert.equal(p.setup.config.auto, false);
  assert.match(p.prompt, /NOT isolated/);
  assert.match(p.prompt, /codex:c-model/);
  assert.match(p.prompt, /Do not require consensus/);
  assert.doesNotMatch(p.prompt, /Use Paseo/i);
});

test('only exact fixed role roster is accepted, with no unregistered harness injection', async () => {
  const { buildReview } = await feature();
  for (const invalid of [
    { provider: 'evil-cli' },
    { reviewers: [] },
    { reviewers: [{ id: 'risk', provider: 'codex', model: 'c-model' }, request().reviewers[1]] },
    { reviewers: [{ ...request().reviewers[0], provider: 'evil-cli' }, request().reviewers[1]] },
    { reviewers: [{ ...request().reviewers[0], model: '' }, request().reviewers[1]] },
  ]) assert.throws(() => buildReview(request(invalid)), /provider|roster|model|registered/i);
});

test('manual preparation requires exact team and installed coordinator and reviewer models', async () => {
  const { buildReview, reviewProblems } = await feature();
  const req = request(), setup = buildReview(req).setup;
  const ctx = { task: task(), team: team(setup), providers: catalog, hasMessages: false };
  assert.deepEqual(reviewProblems(ctx, req), []);
  const changed = team(setup);
  changed.config.workers[0].provider = 'kimi';
  assert.match(reviewProblems({ ...ctx, team: changed }, req).join(' '), /exact bounded review profile/i);
  assert.ok(reviewProblems({ ...ctx, task: task({ provider: 'codex' }) }, req).length);
  assert.ok(reviewProblems({ ...ctx, providers: catalog.filter(p => p.id !== 'codex') }, req).length);
  assert.ok(reviewProblems({ ...ctx, hasMessages: true }, req).length);
  assert.ok(reviewProblems({ ...ctx, task: task({ mode: 'full' }) }, req).length);
});

test('unproven native containment cannot be presented as a running isolated review', () => {
  const native = readFileSync(new URL('../src-tauri/src/review.rs', import.meta.url), 'utf8');
  assert.match(native, /status: "containment_unproven"/);
  assert.match(native, /ready: false/);
  assert.match(native, /proven_providers: Vec::new\(\)/);
  assert.doesNotMatch(native, /Command::new\(|\.spawn\(|\.run\(|Server::start\(/);
  const ui = readFileSync(new URL('../src/hardened/review-panel.ts', import.meta.url), 'utf8');
  assert.match(ui, /No isolated start action is implemented/);
  assert.match(ui, /containment_unproven means this review is NOT isolated/);
  assert.match(ui, /Refresh containment status/);
});
