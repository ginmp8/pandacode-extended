import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import vm from 'node:vm';
import { reviewProblems } from '../src/hardened/review-model.ts';
const ts = createRequire(import.meta.url)('typescript');

// Execute only the reviewed integration function, not PandaCode's module startup.
// The native transport and original view are boundary doubles; this is not Tauri proof.
async function fixture() {
  const source = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8');
  const ast = ts.createSourceFile('main.ts', source, ts.ScriptTarget.ES2022, true);
  const fn = ast.statements.find(s => ts.isFunctionDeclaration(s) && s.name?.text === 'openReviewTools');
  assert.ok(fn, 'The sidebar must expose the review-tools integration');
  const task = { id: 'task', projectId: 'project', title: 'Review', provider: 'codex', model: 'fixture-model', mode: 'plan', status: 'idle', archived: false };
  const team = { sessionId: 'task', revision: 1, status: 'ready', round: 0, workers: [],
    config: { auto: false, rosterOnly: true, name: 'Hardened advisor', workspace: 'shared', maxParallel: 1, maxRounds: 2, maxAssignments: 1, summaryLimit: 1024,
      workers: [{ id: 'advisor', name: 'Advisor', provider: 'codex', model: 'fixture-model', reasoningEffort: null }] } };
  const request = { kind: 'advisor', provider: 'codex', model: 'fixture-model', question: 'Review only', files: '', acceptance: '' };
  const providers = [{ id: 'codex', installed: true, models: [{ id: 'fixture-model' }] }];
  const text = { isConnected: true, dataset: { sessionId: 'task' }, value: '', events: [], dispatchEvent(e) { this.events.push(e.type); }, focus() {} };
  const root = { isConnected: true, textContent: '' };
  const dialog = { open: true, querySelector: () => root };
  const f = { calls: [], opens: [], text, state: { sessionId: 'task', sessions: [task], providers: [], projects: [{ id: 'project', name: 'Demo' }], messages: [], messagesLoading: false, hasOlder: false, streams: new Map(), attachments: [], browserAttachments: new Map(), moodboardAttachments: new Map() },
    teams: new Map([['task', team]]), deletedSessions: new Set(), running: false, images: [], catalog: [], task, team };
  const scope = { state: f.state, teams: f.teams, deletedSessions: f.deletedSessions,
    session: () => f.state.sessions.find(s => s.id === f.state.sessionId), project: () => ({ name: 'Demo' }),
    isRunning: () => f.running, isChatProject: () => false, botProjects: new Set(),
    call: async (command, args) => { f.calls.push({ command, args }); if (command === 'get_task') return structuredClone(f.task); if (command === 'architect_get') return structuredClone(f.team); if (command === 'get_messages') return { hasOlder: false, messages: [] }; if (command === 'list_providers') return f.catalog; if (command === 'list_provider_models') return [{ id: 'fixture-model' }]; throw Error(`Unexpected native call: ${command}`); },
    modal: () => dialog, closeModal: () => { dialog.open = false; },
    attachments: { list: () => f.images },
    document: { querySelector: () => text }, navigator: { clipboard: { writeText: async s => { f.copied = s; } } },
    Event: class { constructor(type) { this.type = type; } },
    selectSession: async id => { f.opens.push(id); f.state.sessionId = id; },
    ensureArchitectView: async () => ({ openWorker: id => { f.workerOpened = id; } }),
    require: name => {
      if (name === './hardened/review-model') return { reviewProblems };
      assert.equal(name, './hardened/review-panel');
      return { mountReviewTools: (_root, host) => { f.host = host; return () => {}; } };
    },
    reviewToolsCleanup: undefined, structuredClone, Map, Set,
  };
  const code = ts.transpileModule(fn.getText(ast), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
  vm.createContext(scope); vm.runInContext(code, scope, { timeout: 1000 });
  await scope.openReviewTools(); assert.ok(f.host); f.request = request; f.providers = providers; return f;
}
test('opening and refreshing review tools never invokes a mutating native command', async () => {
  const f = await fixture(); assert.equal(f.calls.length, 0);
  await f.host.refresh(); assert.deepEqual(f.calls.map(c => c.command).sort(), ['architect_get', 'get_task']);
  assert.equal(f.state.sessions[0], f.task, 'Read-only refresh does not replace global task state');
  f.catalog = [{ id: 'codex', installed: true, models: [] }];
  const catalog = await f.host.loadProviders(); assert.equal(catalog[0].models[0].id, 'fixture-model');
  assert.ok(f.calls.every(c => ['get_task', 'architect_get', 'list_providers', 'list_provider_models'].includes(c.command)));
});
test('draft insertion preserves user work and refuses running, non-plan or attached contexts', async () => {
  for (const mutate of [f => { f.text.value = 'User draft'; }, f => { f.running = true; }, f => { f.task.mode = 'full'; }, f => { f.state.attachments.push('private.txt'); }, f => { f.images.push({ id: 'image' }); }, f => { f.state.browserAttachments.set('task', [{}]); }, f => { f.state.moodboardAttachments.set('task', [{}]); }, f => { f.text.dataset.sessionId = 'other'; }, f => { f.deletedSessions.add('task'); }]) {
    const f = await fixture(); mutate(f); const before = f.text.value;
    await assert.rejects(async () => f.host.insertPrompt('task', 'Review only'));
    assert.equal(f.text.value, before); assert.equal(f.calls.length, 0);
  }
  const f = await fixture(); await f.host.insertPrompt('task', 'Review only', f.request, f.providers);
  assert.equal(f.text.value, 'Review only'); assert.deepEqual(f.text.events, ['input']);
  assert.deepEqual(f.calls.map(c => c.command).sort(), ['architect_get', 'get_messages', 'get_task']);
});
test('attention navigation refuses a worker outside its parent task', async () => {
  const f = await fixture(); await assert.rejects(() => f.host.openTask('task', 'foreign-worker'));
  assert.deepEqual(f.opens, []); await assert.rejects(() => f.host.openTask('unknown'));
  f.team.workers.push({ id: 'worker' });
  await f.host.openTask('task', 'worker'); assert.deepEqual(f.opens, ['task']); assert.equal(f.workerOpened, 'worker');
});
