import test from 'node:test';
import assert from 'node:assert/strict';

const memory = new Map();
globalThis.localStorage = { getItem: key => memory.get(key) ?? null, setItem: (key, value) => memory.set(key, String(value)), removeItem: key => memory.delete(key) };
const { COUNCIL_KEY, COUNCIL_COMPOSITION_MODES, acceptsCouncilSnapshot, councilCompositionMode, councilCycleId, councilLedgerForCurrentCycle, newCouncilComposition, validateCouncilComposition, withCouncilComposition, seatVerdict, formatScore, finishedBelow, outOfTen, councilAsking, councilDefaults, councilNotice, councilProgress, councilRounds, councilForNewThreads, councilStateText, firstAndFinal, provisional, saveCouncilDefaults, scoreVerdict, setCouncilForNewThreads } = await import('../src/council.ts');
const { createPreviewArchitect } = await import('../src/preview-architect.ts');

test('composition derives all seven source modes and preserves the additive schema shape', () => {
  const direct = (botId) => ({ botId, mode: 'direct', required: true });
  const nested = botId => ({ botId, mode: 'nested', required: true, nested: { template: 'explicit_task', childCriteriaSource: 'architect', childReviewerCount: 2, target: 9, maxRounds: 3, escalation: 'always' } });
  const modes = [
    ['M1', true, []], ['M2', false, [direct('a')]], ['M3', true, [direct('a')]],
    ['M4', false, [nested('a')]], ['M5', true, [nested('a')]],
    ['M6', false, [direct('a'), nested('b')]], ['M7', true, [direct('a'), nested('b')]],
  ];
  assert.equal(COUNCIL_COMPOSITION_MODES.length, 7);
  for (const [expected, automatic, selected] of modes) {
    const composition = { ...newCouncilComposition(), automatic: { enabled: automatic, count: automatic ? 'auto' : 0 }, selected };
    assert.equal(councilCompositionMode(composition), expected);
  }
  const empty = newCouncilComposition();
  assert.equal(councilCompositionMode(empty), 'M1');
  assert.deepEqual(Object.keys(empty).sort(), ['automatic', 'criteria', 'criteriaSource', 'limits', 'schemaVersion', 'selected', 'strategy']);
  assert.equal(empty.schemaVersion, 1);
  assert.equal(empty.strategy, 'parallel');
  assert.deepEqual(Object.keys(withCouncilComposition({ target: 9, maxRounds: 10, reviewers: 0 }, empty).composition), Object.keys(empty));
});

test('composition validation accepts all strategy and source-mode combinations and rejects unknown strategies', () => {
  const composition = newCouncilComposition();
  assert.deepEqual(validateCouncilComposition(composition, 9), []);
  assert.match(validateCouncilComposition({ ...composition, limits: { ...composition.limits, maxEstimatedCostMicros: 0 } }, 9)[0], /positive whole number/);
  const direct = (botId) => ({ botId, mode: 'direct', required: true });
  const nested = botId => ({ botId, mode: 'nested', required: true, nested: { template: 'explicit_task', childCriteriaSource: 'architect', childReviewerCount: 2, target: 9, maxRounds: 3, escalation: 'always' } });
  const modes = [
    { automatic: { enabled: true, count: 'auto' }, selected: [] },
    { automatic: { enabled: false, count: 0 }, selected: [direct('a'), direct('b')] },
    { automatic: { enabled: true, count: 'auto' }, selected: [direct('a')] },
    { automatic: { enabled: false, count: 0 }, selected: [nested('a'), nested('b')] },
    { automatic: { enabled: true, count: 'auto' }, selected: [nested('a')] },
    { automatic: { enabled: false, count: 0 }, selected: [direct('a'), nested('b')] },
    { automatic: { enabled: true, count: 'auto' }, selected: [direct('a'), nested('b')] },
  ];
  for (const strategy of ['parallel', 'escalated', 'two_phase']) {
    for (const mode of modes) {
      assert.deepEqual(validateCouncilComposition({ ...composition, ...mode, strategy }, 9), [], `${strategy} with ${mode.selected.length} selected reviewers`);
    }
  }
  assert.match(validateCouncilComposition({ ...composition, strategy: 'unknown' }, 9).join(' '), /supported Council execution strategy/);
  const selectedOnly = { ...composition, automatic: { enabled: false, count: 0 }, selected: [
    { botId: 'bot-a', mode: 'nested', required: true, nested: { template: 'explicit_task', childCriteriaSource: 'user', childReviewerCount: 2, target: 9, maxRounds: 3, escalation: 'on_trigger' } },
    { botId: 'bot-b', mode: 'direct', required: true },
  ], criteriaSource: 'user', criteria: [
    { id: 'security', lens: 'Security', description: 'Check the security boundary.', required: true, target: 9, evidenceRequired: true, source: 'user' },
  ] };
  assert.deepEqual(validateCouncilComposition(selectedOnly, 9), []);
  assert.match(validateCouncilComposition({ ...selectedOnly, selected: selectedOnly.selected.slice(0, 1) }, 9).join(' '), /At least 2 root reviewers/);
});

let nextId = 0;
const seat = (round, score, over = {}) => ({ id: `w${++nextId}`, status: score === undefined ? 'running' : 'completed', readOnly: true, createdAt: 100 * round, updatedAt: 100 * round + 50, council: { round, lens: `lens ${nextId}`, score: score ?? null, startedAt: 100 * round }, ...over });
const writer = (over = {}) => ({ id: `w${++nextId}`, status: 'completed', readOnly: false, createdAt: 10, updatedAt: 20, ...over });
const team = (workers, council = { target: 9, maxRounds: 3, reviewers: 0 }, extra = {}) => ({ workers, config: { council }, ...extra });
/** A snapshot carrying the status native sends: the webview reads it, never derives it. */
const native = (status, workers = [], council = { target: 9.5, maxRounds: 10, reviewers: 0 }) =>
  councilProgress(team(workers, council, { councilStatus: { round: 1, lowest: null, target: council.target, stale: false, notice: null, ...status } }));

test('rounds group the current cycle, oldest first, and drop unscored abandoned or older reviewers', () => {
  const rounds = councilRounds(team([
    seat(2, 9), seat(1, 8), seat(2, 7.5), seat(1, undefined, { abandoned: true, status: 'stopped' }), seat(1, 3, { createdAt: 5 }), writer({ createdAt: 500 }),
  ], undefined, { councilSince: 50 }));
  assert.deepEqual(rounds.map(round => round.number), [1, 2]);
  assert.equal(rounds[0].seats.length, 1);
  assert.equal(rounds[1].lowest, 7.5);
  assert.deepEqual(councilRounds({ workers: [writer()], config: {} }), []);
});

test('native ledger projection is pinned to the current root cycle and its correlated child cycles', () => {
  const root = 'session:200', child = 'child-session:9', old = 'session:100';
  const finding = (id, cycleId) => ({ id, cycleId, roundId: `${cycleId}:1`, seatId: `seat-${id}`, artifactDigest: `artifact-${id}`, criterionId: 'security', severity: 'high', category: 'Security', summary: id, evidenceRefs: [], locationStatus: 'missing' });
  const team = {
    sessionId: 'session', councilSince: 200,
    councilFindings: [finding('root', root), finding('child', child), finding('old', old)],
    councilFindingDispositions: [
      { findingId: 'root', disposition: 'addressed' }, { findingId: 'child', disposition: 'waived_by_authorized_human' }, { findingId: 'old', disposition: 'disputed' }, { findingId: 'orphan', disposition: 'addressed' },
    ],
    councilChildReceipts: [
      { rootCycleId: root, childCycleId: child }, { rootCycleId: old, childCycleId: 'old-child:1' },
    ],
    councilReviewReceipts: [
      { cycleId: root, requestId: 'root' }, { cycleId: child, requestId: 'child' }, { cycleId: old, requestId: 'old' },
    ],
    councilUsage: [
      { rootCycleId: root, childCycleId: null, id: 'root' }, { rootCycleId: root, childCycleId: child, id: 'child' }, { rootCycleId: old, childCycleId: 'old-child:1', id: 'old' },
    ],
  };
  const projection = councilLedgerForCurrentCycle(team);
  assert.equal(projection.cycleId, root);
  assert.deepEqual(projection.findings.map(item => item.id), ['root', 'child']);
  assert.deepEqual(projection.findingDispositions.map(item => item.findingId), ['root', 'child']);
  assert.deepEqual(projection.childReceipts.map(item => item.childCycleId), [child]);
  assert.deepEqual(projection.reviewReceipts.map(item => item.requestId), ['root', 'child']);
  assert.deepEqual(projection.usage.map(item => item.id), ['root', 'child']);
});

test('snapshot updates require the active session, non-older cycle, and non-older revision', () => {
  const current = { sessionId: 'session', revision: 8, councilSince: 200 };
  assert.equal(councilCycleId(current), 'session:200');
  assert.equal(acceptsCouncilSnapshot({ ...current, revision: 9 }, current, 'session'), true, 'newer revision in the same cycle');
  assert.equal(acceptsCouncilSnapshot({ ...current, councilFindings: [] }, current, 'session'), true, 'same revision may carry a refreshed projection');
  assert.equal(acceptsCouncilSnapshot({ ...current, revision: 12, councilSince: 100 }, current, 'session'), false, 'a larger revision cannot revive an earlier cycle');
  assert.equal(acceptsCouncilSnapshot({ ...current, revision: 7 }, current, 'session'), false, 'older revision');
  assert.equal(acceptsCouncilSnapshot({ ...current, sessionId: 'other' }, current, 'session'), false, 'wrong task');
  assert.equal(acceptsCouncilSnapshot({ ...current, revision: 9, councilSince: 300 }, current, 'session'), true, 'newer cycle and revision');
});

test('a scored seat stays in its round even if abandoned', () => {
  const [round] = councilRounds(team([seat(1, 8.4, { abandoned: true }), seat(1, 9)]));
  assert.equal(round.seats.length, 2);
  assert.equal(round.lowest, 8.4);
});

test("native's councilStatus is the only state; the rounds still come from the workers", () => {
  assert.equal(councilProgress({ workers: [], config: {} }), null, 'no council, no progress');
  const progress = native({ state: 'fixing', lowest: 9, target: 9, stale: true }, [seat(1, 9), seat(1, 9.5)], { target: 9, maxRounds: 3, reviewers: 0 });
  assert.equal(progress.state, 'fixing', 'native saw an edit the webview cannot, like the lead\'s own');
  assert.equal(progress.rounds.length, 1);
  const unsent = councilProgress(team([seat(1, 8), seat(1, 9)]));
  assert.equal(unsent.state, 'waiting', 'the app never derives a state native did not send');
  assert.equal(unsent.rounds.length, 1, 'its rounds still show');
});

test('state text reads as the design says', () => {
  const text = (status, council) => councilStateText(native(status, [], council));
  assert.equal(text({ state: 'waiting', round: 0 }), 'Waiting for reviewers');
  assert.equal(text({ state: 'reviewing', round: 2 }), 'Round 2 of 10 · reviewing');
  assert.equal(text({ state: 'reviewing', round: 2 }, { target: 9.5, maxRounds: 25, reviewers: 0 }), 'Round 2 of 25 · reviewing');
  assert.equal(text({ state: 'dispatch', round: 2 }), 'Round 2 of 10 · waiting for the lead');
  assert.equal(text({ state: 'fixing', lowest: 8 }), 'Round 1 of 10 · fixing');
  assert.equal(text({ state: 'fixing', round: 3, lowest: 8.3, stale: true }), 'Round 3 of 10 · fixing', 'a stale round below target is the normal fixing loop');
  assert.equal(text({ state: 'fixing', lowest: 9.5, stale: true }), 'Changed after passing at 9.5: fixing');
  assert.equal(text({ state: 'passed', lowest: 9.5 }), 'Passed at 9.5');
  assert.equal(text({ state: 'limit', lowest: 8.7, round: 3 }), 'Stopped at round limit (3 of 10)', 'used of the limit');
  const edited = text({ state: 'limit', lowest: 9.5, round: 3, stale: true });
  assert.equal(edited, 'Stopped at round limit: changed after passing at 9.5 (3 of 10)');
  assert.doesNotMatch(edited, /9\.5 of 9\.5/);
});

test('a team that is not running never reads as reviewing or fixing', () => {
  const text = (status, run) => councilStateText(native(status), run);
  const open = { state: 'dispatch', round: 2, lowest: 8.3 };
  assert.equal(text(open, { status: 'paused' }), 'Paused · round 2 of 10 · lowest so far 8.3 · target 9.5');
  assert.equal(text(open, { status: 'paused', error: 'Team stopped. Resume explicitly to retry unfinished work.' }), 'Stopped · round 2 of 10 · lowest so far 8.3 · target 9.5');
  assert.equal(text(open, { status: 'interrupted' }), 'Interrupted · round 2 of 10 · lowest so far 8.3 · target 9.5');
  assert.equal(text(open, { status: 'failed' }), 'Failed · round 2 of 10 · lowest so far 8.3 · target 9.5');
  assert.equal(text({ state: 'fixing', lowest: 8.8 }, { status: 'completed' }), 'Finished below target', 'the footer carries the numbers');
  assert.equal(text({ state: 'reviewing' }, { status: 'completed' }), 'Finished before any score');
  assert.equal(text({ state: 'passed', lowest: 9.5 }, { status: 'completed' }), 'Passed at 9.5', 'a settled state stays as it is');
  assert.equal(text({ state: 'waiting', round: 0 }, { status: 'running' }), 'Waiting for reviewers');
  assert.equal(text({ state: 'waiting', round: 0 }, { status: 'completed' }), 'No changes to review yet');
  assert.equal(text({ state: 'waiting', round: 0 }, { status: 'paused' }), 'Paused before the first round');
});

test("a round's lowest is so far only while native is still on that round", () => {
  const workers = [seat(1, 8.3), seat(1, 9), seat(2, 9.2), seat(2, undefined)];
  const progress = native({ state: 'reviewing', round: 2, lowest: 9.2 }, workers);
  assert.equal(provisional(progress, progress.rounds[1]), true);
  assert.equal(provisional(progress, progress.rounds[0]), false, 'round 1 is a result');
  assert.equal(provisional(native({ state: 'fixing', round: 2, lowest: 9.2 }, workers), progress.rounds[1]), false);
});

test("asking is exactly native's state 'asking'; its score lives in native's notice", () => {
  const refusal = 'Council round 3: lowest 8.3, target 9.5. Fix the blockers with workers, then dispatch a new round with councilLens.';
  const notice = 'The lead is asking how to continue: lowest score 8.3, target 9.5, after round 3.';
  const asking = native({ state: 'asking', round: 3, lowest: 8.3, notice });
  assert.equal(councilAsking(asking.status), true);
  assert.equal(councilStateText(asking, { status: 'ready', error: refusal }), 'Needs you', 'the score is not said twice');
  assert.equal(councilNotice({ councilStatus: asking.status }), notice);
  assert.equal(councilNotice({ councilStatus: { ...asking.status, notice: null } }), 'The lead is asking how to continue.', 'a fallback, never the refusal');
  const fixing = native({ state: 'fixing', round: 3, lowest: 8.3 });
  assert.equal(councilAsking(fixing.status), false, 'a refusal-shaped error alone asks nothing');
  assert.equal(councilNotice({ councilStatus: fixing.status }), null);
  const paused = native({ state: 'fixing', round: 3, lowest: 8.3, notice: 'Council has not passed yet: 8.3 of 9.5.' });
  assert.equal(councilNotice({ councilStatus: paused.status }), 'Council has not passed yet: 8.3 of 9.5.', 'a refusal outside asking is shown as its notice too');
  assert.equal(councilStateText(native({ state: 'accepted', round: 3, lowest: 8.3 })), 'Accepted at 8.3');
});

test('a score reads against the target in words and tone, never success below it', () => {
  assert.deepEqual(scoreVerdict(8.25, 9.5), { text: 'Scored 8.3 · below 9.5', tone: 'below' });
  assert.deepEqual(scoreVerdict(9.5, 9.5), { text: 'Scored 9.5 · at target', tone: 'at' });
  assert.deepEqual(scoreVerdict(8.3, undefined), { text: 'Scored 8.3', tone: '' }, 'no target known (Council off): neutral');
});

test('defaults are remembered and validated on read, field by field', () => {
  memory.clear();
  assert.deepEqual(councilDefaults(), { target: 9, maxRounds: 10, reviewers: 0 });
  saveCouncilDefaults({ target: 9.5, maxRounds: 25, reviewers: 3 });
  assert.deepEqual(councilDefaults(), { target: 9.5, maxRounds: 25, reviewers: 3 });
  memory.set(COUNCIL_KEY, JSON.stringify({ target: 9.5, maxRounds: 0, reviewers: 3 }));
  assert.deepEqual(councilDefaults(), { target: 9.5, maxRounds: 10, reviewers: 3 }, 'a value not offered falls back to the default');
  memory.set(COUNCIL_KEY, JSON.stringify({ target: 42, maxRounds: 5, reviewers: 'many' }));
  assert.deepEqual(councilDefaults(), { target: 9, maxRounds: 5, reviewers: 0 });
  memory.set(COUNCIL_KEY, '{broken');
  assert.deepEqual(councilDefaults(), { target: 9, maxRounds: 10, reviewers: 0 });
  memory.set(COUNCIL_KEY, 'null');
  assert.deepEqual(councilDefaults(), { target: 9, maxRounds: 10, reviewers: 0 });
  saveCouncilDefaults({ target: 7, maxRounds: 10, reviewers: 0 });
  assert.equal(councilDefaults().target, 7, 'a target of 7 is offered');
});

test('"Turn on for new threads" is remembered with the defaults, and saving defaults keeps it', () => {
  memory.clear();
  assert.equal(councilForNewThreads(), false, 'off unless chosen');
  setCouncilForNewThreads(true);
  saveCouncilDefaults({ target: 8.5, maxRounds: 5, reviewers: 2 });
  assert.equal(councilForNewThreads(), true);
  assert.deepEqual(councilDefaults(), { target: 8.5, maxRounds: 5, reviewers: 2 });
  setCouncilForNewThreads(false);
  assert.equal(councilForNewThreads(), false);
  assert.equal(councilDefaults().target, 8.5, 'turning it off keeps the defaults');
});

test("the preview's pending setup keeps the thread's newest council, like native with_council", t => {
  memory.clear();
  const previous = { window: globalThis.window };
  globalThis.window = { setTimeout: () => 0, clearTimeout() {} };
  t.after(() => { globalThis.window = previous.window; });
  const parent = { id: 'parent', title: 'New conversation', status: 'idle', taskStage: 'pending', archived: false, provider: 'codex', model: 'sample-model', reasoningEffort: null, updatedAt: 0 };
  const api = createPreviewArchitect({
    parent(id) { if (id !== 'parent') throw Error('Task not found'); return parent; },
    assertIdle() {}, append: (sessionId, role, content) => ({ id: 1, sessionId, role, content, createdAt: 0 }),
    finishTiming() {}, finishChanges() {}, nextMessageId: () => 1, models: () => [{ id: 'sample-model' }], emit() {}, agent() {},
  });
  const setup = { auto: true, workers: [], maxParallel: 0, maxRounds: 8, maxAssignments: 24, workspace: 'shared', summaryLimit: 3072 };
  api.configure('parent', setup);
  api.setCouncil('parent', { target: 9, maxRounds: 3, reviewers: 0 });
  api.start('parent', 'Build it');
  api.configure('parent', { ...setup, maxParallel: 2 });
  api.setCouncil('parent', { target: 8, maxRounds: 3, reviewers: 0 });
  const paused = api.control('parent', 'pause');
  assert.equal(paused.config.maxParallel, 2, 'the pending setup applied');
  assert.deepEqual(paused.config.council, { target: 8, maxRounds: 3, reviewers: 0 }, 'and the council set during the run survived it');
});

test("first-vs-final reads each lens's first score this cycle and its latest", () => {
  const lens = (round, name, score) => seat(round, score, { council: { round, lens: name, score, startedAt: 100 * round } });
  const rounds = councilRounds(team([lens(1, 'Correctness', 8.3), lens(1, 'Security', 9.1), lens(2, 'Correctness', 9.5), lens(2, 'Speed', 9.7), lens(2, 'Security', undefined)]));
  assert.deepEqual(firstAndFinal(rounds), [
    { lens: 'Correctness', first: 8.3, final: 9.5 },
    { lens: 'Security', first: 9.1, final: 9.1 },
    { lens: 'Speed', first: 9.7, final: 9.7 },
  ]);
});

test('every score and target reads with one decimal, in columns and in words', () => {
  assert.equal(formatScore(9), '9.0');
  assert.equal(formatScore(8.25), '8.3');
  assert.equal(formatScore(10), '10.0');
  assert.equal(outOfTen(9.6, 9.5), '9.6 out of 10 (target 9.5)');
  assert.equal(outOfTen(9, 9), '9.0 out of 10 (target 9.0)');
});

test('finished below target: a team done with the work under target and nothing running or asked', () => {
  const below = native({ state: 'fixing', lowest: 8.8 });
  assert.equal(finishedBelow(below, { status: 'completed' }), true);
  assert.equal(finishedBelow(below, { status: 'running' }), false, 'still running');
  assert.equal(finishedBelow(native({ state: 'passed', lowest: 9.5 }), { status: 'completed' }), false, 'passed');
  assert.equal(finishedBelow(native({ state: 'asking', lowest: 8.8 }), { status: 'completed' }), false, 'asking is not finished');
  assert.equal(finishedBelow(native({ state: 'fixing', lowest: 9.6, stale: true }), { status: 'completed' }), false, 'changed after passing is its own end');
});

test('seat verdicts: at target everywhere, and only a complete round\'s lowest warns', () => {
  const low = seat(1, 8.3), mid = seat(1, 8.8), high = seat(1, 9.6), open = seat(2, 8.1), pending = seat(2);
  const status = (state, round) => ({ state, round, lowest: 8.1, target: 9.5, stale: false, notice: null });
  const snapshot = (state, round) => team([low, mid, high, open, pending], { target: 9.5, maxRounds: 10, reviewers: 0 }, { councilSince: 0, councilStatus: status(state, round) });
  const done = snapshot('fixing', 2);
  assert.equal(seatVerdict(done, high).tone, 'at');
  assert.equal(seatVerdict(done, low).tone, 'below', "round 1's lowest");
  assert.equal(seatVerdict(done, mid).tone, '', 'below target, not the lowest: neutral');
  assert.match(seatVerdict(done, mid).text, /below 9.5/, 'still says below in words');
  assert.equal(seatVerdict(done, open).tone, 'below', 'round 2, complete');
  assert.equal(seatVerdict(snapshot('reviewing', 2), open).tone, '', 'an open round\'s lowest waits');
  assert.equal(seatVerdict(done, pending), null, 'no score yet');
  // The target is native's when it has one.
  assert.equal(seatVerdict(team([mid], { target: 9.5, maxRounds: 10, reviewers: 0 }, { councilStatus: { ...status('fixing', 1), target: 8.5 } }), mid).tone, 'at');
});
