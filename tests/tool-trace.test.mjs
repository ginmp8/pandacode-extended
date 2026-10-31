import test from 'node:test';
import assert from 'node:assert/strict';
import { describeTool, mcpAction, mcpCard, summarizeTools, summaryIcons, toolPreview, traceRowHtml, shellCommand, toolKind } from '../src/tool-trace.ts';
import { richCard } from '../src/tool-cards.ts';
import { renderToolActivity } from '../src/tool-activity.ts';
import { PANDACODE_TOOLS } from '../src/tool-trace-pandacode.ts';

const tool = (name, input, extra = {}) => ({ name, status: 'completed', input: typeof input === 'string' ? input : JSON.stringify(input), output: '', ...extra });

test('describes each kind as a verb, chip and object', () => {
  const cases = [
    [tool('Bash', { command: 'npm test -- --runInBand' }), 'shell', 'Ran', 'npm', 'test -- --runInBand'],
    [tool('Command', '/bin/zsh -lc "git status --short"\nWorking directory: /repo'), 'shell', 'Ran', 'git', 'status --short'],
    [tool('pandacode_vm_exec', { name: 'vm', command: 'uname -a' }), 'shell', 'Ran', 'uname', '-a · on vm'],
    [tool('Read', { file_path: '/repo/src/main.ts' }), 'read', 'Read', 'main.ts', ''],
    [tool('read', { path: 'src/dom.ts' }), 'read', 'Read', 'dom.ts', ''],
    [tool('Write', { file_path: 'a/b.md', content: 'one\ntwo' }), 'write', 'Wrote', 'b.md', '2 lines'],
    [tool('Grep', { pattern: 'icon\\(', path: 'src' }), 'search', 'Searched', 'src', 'for icon\\('],
    [tool('Glob', { pattern: '**/*.ts' }), 'search', 'Searched', 'workspace', 'for files matching **/*.ts'],
    [tool('WebSearch', { query: 'tauri updater' }), 'web', 'Searched', 'Web', 'for tauri updater'],
    [tool('WebFetch', { url: 'https://www.example.com/a', prompt: 'Summarize' }), 'fetch', 'Fetched', 'example.com', 'Summarize'],
    [tool('TodoWrite', { todos: [{ content: 'a', status: 'pending' }, { content: 'b', status: 'completed' }] }), 'todo', 'Updated', undefined, 'the plan · 2 steps'],
    [tool('Task', { description: 'Find callers', subagent_type: 'Explore' }), 'agent', 'Started', 'Explore', 'Find callers'],
    [tool('Skill', { skill: 'hyperframes' }), 'skill', 'Loaded skill', 'hyperframes', ''],
    [tool('Skill', { skill: 'simplify', args: 'src/lib' }), 'skill', 'Loaded skill', 'simplify', 'src/lib'],
    [tool('mcp__github__list_issues', {}), 'mcp', 'Read the', 'GitHub', 'issues'],
    [tool('mcp__slack__search_messages', { query: 'messages since yesterday' }), 'mcp', 'Searched', 'Slack', 'for messages since yesterday'],
    [tool('mcp__claude_ai_Granola__get_meetings', { query: 'notes from today’s calls' }, { status: 'running' }), 'mcp', 'Read the', 'Granola', 'notes from today’s calls'],
    [tool('mcp__acme__deploy_site', {}), 'mcp', 'Used', 'acme', 'deploy site'],
    [tool('pandacode_dispatch_workers', { workers: [{}, {}] }), 'mcp', 'Dispatched', 'PandaCode', '2 workers'],
    [tool('pandacode_architect_pandacode_title', { title: 'Traces' }), 'mcp', 'Named', 'PandaCode', 'the task “Traces”'],
    [tool('mcp__pandacode_architect__pandacode_dispatch_workers', { workers: [{}] }), 'mcp', 'Dispatched', 'PandaCode', '1 worker'],
    [tool('Write', { file_path: 'one.md', content: 'x' }), 'write', 'Wrote', 'one.md', '1 line'],
    [tool('Frobnicate', { path: 'x/y' }), 'tool', 'Used', 'Frobnicate', 'x/y'],
  ];
  for (const [input, kind, verb, chip, object] of cases) {
    const trace = describeTool(input);
    assert.equal(trace.kind, kind, input.name);
    assert.equal(trace.verb, verb, input.name);
    assert.equal(trace.chip?.label, chip, input.name);
    assert.equal(trace.object, object, input.name);
  }
  const edit = describeTool(tool('Edit', { file_path: 'src/x.ts', old_string: 'a', new_string: 'b\nc' }));
  assert.deepEqual([edit.kind, edit.verb, edit.chip.label, edit.chip.title, edit.added, edit.removed], ['edit', 'Edited', 'x.ts', 'src/x.ts', 2, 1]);
  const multi = describeTool(tool('edit', { path: 'y.ts', edits: [{ oldText: 'a', newText: 'b' }, { oldText: 'c\nd', newText: '' }] }));
  assert.deepEqual([multi.added, multi.removed], [1, 3]);
  assert.equal(shellCommand("bash -lc 'ls -la'", {}), 'ls -la');
  assert.equal(toolKind('MultiEdit'), 'edit');
  assert.equal(describeTool(tool('mcp__slack__search_messages', {})).icon, 'search');
  assert.equal(describeTool(tool('mcp__linear__list_issues', {})).icon, 'file');
  assert.match(describeTool(tool('mcp__slack__search_messages', {})).chip.svg, /brand-icon/);
  assert.deepEqual(['search_messages', 'slack_search_public', 'list_pull_requests', 'get_meetings', 'deploy_site'].map(mcpAction), ['search', 'search', 'read', 'read', undefined]);
  assert.match(traceRowHtml(describeTool(tool('mcp__gmail__search_threads', { query: 'unread threads' }))), /tool-trace-verb">Searched<\/span><span class="tool-trace-chip" title="mcp__gmail__search_threads"><svg class="brand-icon"[^]*<b>Gmail<\/b><\/span><span class="tool-trace-object">for unread threads</);
});

test('rows read as finished sentences even while running', () => {
  assert.equal(describeTool(tool('Bash', { command: 'ls' }, { status: 'running' })).verb, 'Ran');
  assert.equal(describeTool(tool('Read', { file_path: 'a' }, { status: 'running' })).verb, 'Read');
  assert.equal(describeTool(tool('WebSearch', { query: 'q' }, { status: 'running' })).verb, 'Searched');
  assert.equal(describeTool(tool('Skill', { skill: 's' }, { status: 'running' })).verb, 'Loaded skill');
  // Only brand marks sit inside the chip; a line icon would repeat the row's kind icon.
  assert.match(traceRowHtml(describeTool(tool('Read', { file_path: 'src/a.ts' }))), /tool-trace-chip" title="src\/a\.ts"><b>a\.ts<\/b>/);
  assert.equal(describeTool(tool('pandacode_dispatch_workers', {}, { status: 'running' })).verb, 'Dispatched');
});

test('summaries pluralise, merge verbs and report running and failures', () => {
  const bash = tool('Bash', { command: 'ls' }), read = tool('Read', { file_path: 'a' });
  // Two counts at most, then `and N more`, so the folded summary stays on one line.
  assert.equal(summarizeTools([bash, bash, bash, read, read, tool('Edit', {})]), 'Ran 3 commands, read 2 files and 1 more step');
  assert.equal(summarizeTools([tool('Grep', {}), tool('WebSearch', {}), read, read, read]), 'Ran 2 searches and read 3 files');
  assert.equal(summarizeTools([{ ...bash, status: 'running' }, bash], { live: true }), 'Running 2 commands…');
  // A saved record stuck at `running` still reads as finished.
  assert.equal(summarizeTools([{ ...bash, status: 'running' }, bash]), 'Ran 2 commands');
  assert.equal(summarizeTools([bash, { ...read, status: 'failed' }]), 'Ran 1 command and read 1 file · 1 failed');
  assert.equal(summarizeTools([undefined]), 'Used 1 tool');
  assert.equal(summarizeTools([tool('TodoWrite', {})]), 'Updated the plan');
  const brief = ['mcp__slack__search_messages', 'mcp__granola__get_meetings', 'mcp__gmail__search_threads', 'mcp__github__list_pull_requests', 'mcp__linear__list_issues'].map(name => tool(name, {}));
  assert.equal(summarizeTools(brief), 'Ran 2 searches and read 3 sources');
  assert.equal(summarizeTools([...brief, read]), 'Ran 2 searches, read 3 sources and 1 more step');
  assert.equal(summarizeTools([{ ...brief[0], status: 'running' }, brief[1]], { live: true }), 'Running 1 search and reading 1 source…');
  const marks = summaryIcons(brief);
  assert.equal(marks.length, 3);
  assert.ok(marks.every(mark => mark.includes('brand-icon')));
  assert.equal(summaryIcons([bash, bash, read]).length, 2);
});

test('MCP results become brand rows: marks, title, meta, time, body and +N more', () => {
  const json = value => JSON.stringify(value);
  const slack = mcpCard(tool('mcp__slack__search_messages', {}, { output: json({ messages: [
    { user: 'Jane Doe', channel: '#support', ts: '9:41', text: 'Two tickets.' },
    { user: { name: 'Sam Lee' }, channel: '#deploys', ts: '11:02', text: 'fix: retry' },
    { user: 'Alex Kim', channel: '#general', ts: '14:15', text: 'Huddle at 3?' },
    { user: 'Priya Shah', channel: '#design', ts: '15:30', text: 'Mocks' },
  ] }) }));
  assert.match(slack, /class="tool-trace-card is-rows" data-trace-kind="mcp" data-brand="slack"/);
  assert.match(slack, /trace-mark is-avatar">J<\/span><span class="trace-row-head"><b>Jane Doe<\/b><span class="trace-row-meta">#support<\/span><\/span><span class="trace-row-time">9:41<\/span><span class="trace-row-body">Two tickets.<\/span>/);
  assert.match(slack, /<b>Sam Lee<\/b>/);
  assert.ok(!slack.includes('Priya'));
  assert.match(slack, /tool-trace-more">\+1 more</);
  const gmail = mcpCard(tool('mcp__gmail__search_threads', {}, { output: json([{ type: 'text', text: json({ threads: [{ subject: 'Re: Signed contract', from: 'Jane Doe', time: '8:12', unread: true, snippet: 'Attached' }, { subject: 'Invoice', from: 'Billing', unread: false }] }) }]) }));
  assert.match(gmail, /<li class="trace-row" data-unread><span class="trace-mark is-unread" data-unread><\/span><span class="trace-row-head"><b>Re: Signed contract<\/b><span class="trace-row-meta">Jane Doe/);
  assert.match(gmail, /<span class="trace-mark is-unread"><\/span><span class="trace-row-head"><b>Invoice<\/b>/);
  const github = mcpCard(tool('mcp__github__list_pull_requests', {}, { output: json({ pull_requests: [{ title: 'Retry', identifier: 'acme/app#128', state: 'merged', time: '2h' }, { title: 'Export', identifier: 'acme/app#131', state: 'open' }] }) }));
  assert.match(github, /trace-mark is-pull" data-state="merged"/);
  assert.match(github, /data-state="open"[^]*<b>Export<\/b><span class="trace-row-meta">acme\/app#131/);
  const linear = mcpCard(tool('mcp__linear__list_issues', {}, { output: json({ issues: [{ title: 'CSV', identifier: 'ENG-142', state: 'In Progress' }, { title: 'Dupes', identifier: 'ENG-139', status: 'Todo' }, { title: 'Ship', identifier: 'ENG-120', state: 'Done' }] }) }));
  assert.deepEqual([...linear.matchAll(/is-issue" data-state="(\w+)"/g)].map(match => match[1]), ['progress', 'todo', 'done']);
  const granola = mcpCard(tool('mcp__granola__get_meetings', {}, { output: json({ meetings: [{ title: 'Kickoff', author: 'Jane, Sam', time: '10:00' }] }) }));
  assert.match(granola, /<span class="trace-mark"><svg class="brand-icon"[^]*<b>Kickoff<\/b><span class="trace-row-meta">Jane, Sam/);
  // Unparseable or list-free output falls through to the rich card or the excerpt.
  const plain = tool('mcp__slack__search_messages', {}, { output: 'no results' });
  assert.equal(mcpCard(plain), '');
  assert.equal(mcpCard(tool('mcp__x__get', {}, { output: '{"ok":true}' })), '');
  assert.equal(mcpCard(tool('Bash', { command: 'ls' }, { output: '[{"title":"x"}]' })), '');
  assert.ok(toolPreview(plain).length > 0);
});

test('previews prefer the MCP card, then the rich card inside the shared card chrome', () => {
  for (const input of [
    tool('Command', 'ls', { output: 'Exit code: 2\nOutput:\nline 1' }),
    tool('Read', 'src/a.ts', { output: 'one\ntwo\nthree' }),
    tool('Edit', { file_path: 'a', old_string: 'x', new_string: 'y' }),
    tool('TodoWrite', { todos: [{ content: 'Ship', status: 'completed' }] }),
  ]) {
    assert.ok(richCard(input), input.name);
    assert.equal(toolPreview(input), `<div class="tool-trace-card" data-trace-kind="${toolKind(input.name)}">${richCard(input)}</div>`, input.name);
  }
  const mcp = tool('mcp__linear__list_issues', {}, { output: '{"issues":[{"title":"CSV","state":"todo"}]}' });
  assert.equal(toolPreview(mcp), mcpCard(mcp));
  // PandaCode's own tools and docs servers use the rich card first (workers with prompts, docs with URLs).
  for (const own of [
    tool('mcp__pandacode_architect__pandacode_dispatch_workers', { workers: [{ name: 'A', prompt: 'Do it', model: 'opus' }] }, { output: '{"workers":[{"name":"A","status":"started"}]}' }),
    tool('mcp__docs__search_docs', { query: 'index' }, { output: '{"results":[{"title":"Search index","url":"https://docs.example.com/search-index"}]}' }),
  ]) {
    assert.ok(richCard(own), own.name);
    assert.equal(toolPreview(own), `<div class="tool-trace-card" data-trace-kind="mcp">${richCard(own)}</div>`, own.name);
  }
  assert.equal(toolPreview(tool('Read', 'a', { output: '[Non-text content omitted]' })).includes('Non-text'), false);
});

test('chips, objects and cards escape tool data', () => {
  const unsafe = '<img src=x onerror=alert(1)>';
  const html = traceRowHtml(describeTool(tool(unsafe, { path: unsafe })))
    + traceRowHtml(describeTool(tool('Read', { file_path: `/a/${unsafe}"` })))
    + toolPreview(tool('Bash', { command: unsafe }, { output: unsafe }))
    + toolPreview(tool('WebSearch', {}, { output: `Links: [{"title":"${unsafe.replace(/"/g, '')}","url":"javascript:${unsafe}"}]` }))
    + toolPreview(tool('Grep', {}, { output: unsafe }))
    + toolPreview(tool('TodoWrite', { todos: [{ content: unsafe, status: '"><b>' }] }));
  assert.ok(!html.includes('<img'));
  assert.ok(!/data-status="">/.test(html));
  assert.match(html, /&lt;img/);
  const card = mcpCard(tool('mcp__slack__search_messages', {}, { output: JSON.stringify({ messages: [{ user: unsafe, channel: unsafe, ts: unsafe, text: unsafe, state: unsafe }] }) }));
  assert.ok(card.includes('&lt;img') && !card.includes('<img'));
});

import { failedVerb } from '../src/tool-trace.ts';

test('failed rows lead with the failure', () => {
  const cases = { 'Creating task': "Couldn't create task", Running: "Couldn't run", 'Taking screenshot of': "Couldn't take screenshot of", 'Double-clicking': "Couldn't double-click",
    Cancelling: "Couldn't cancel", Controlling: "Couldn't control", Dragging: "Couldn't drag", Installing: "Couldn't install", 'Shutting down': "Couldn't shut down",
    'Queuing message for': "Couldn't queue message for", 'Going back': "Couldn't go back", Using: "Couldn't use", 'Messaging the user': "Couldn't message the user", Pressing: "Couldn't press" };
  for (const [running, failed] of Object.entries(cases)) assert.equal(failedVerb(running), failed);
  for (const name of PANDACODE_TOOLS) assert.match(describeTool(tool('pandacode_' + name, {}, { status: 'failed' })).verb, /^Couldn't [a-z]/, name);
  const html = renderToolActivity({ id: '1', name: 'pandacode_vm_manage', status: 'failed', input: '{"action":"create","name":"x"}', output: 'No disk' }, 'k');
  assert.ok(!html.includes('tool-activity-status'));
  assert.match(html, /is-error">No disk</);
});

test('badges separate success, running, neutral and failure', () => {
  const html = toolPreview(tool('pandacode_list_tasks', {}, { output: JSON.stringify({ tasks: ['completed', 'running', 'queued', 'failed', 'stopped'].map((status, i) => ({ id: String(i), title: 't', status })) }) }));
  assert.match(html, /tool-trace-badge">completed</);
  assert.match(html, /is-live">running</);
  assert.match(html, /is-quiet">queued</);
  assert.match(html, /is-failed">failed</);
  assert.match(html, /is-quiet">stopped</);
});

test('cards do not repeat the row, and full data appears only when the card is partial', () => {
  const card = (name, input, output = '') => toolPreview(tool(name, input, { output }));
  assert.equal(card('pandacode_vm_control', { name: 'vm', action: 'click', x: 1, y: 2 }), '');
  assert.match(card('pandacode_vm_control', { name: 'vm', action: 'scroll', direction: 'down', amount: 5 }), /<dt>amount<\/dt><dd>5<\/dd>/);
  assert.ok(!card('pandacode_vm_control', { name: 'vm', action: 'scroll', direction: 'down', amount: 5 }).includes('<dt>direction'));
  assert.equal(card('pandacode_bot_remember', { content: 'short' }), '');
  assert.equal(card('pandacode_bot_notify', { text: 'short' }), '');
  assert.ok(!card('pandacode_bot_schedule', { title: 'Digest', prompt: 'Summarize', kind: 'daily', timeOfDay: '08:00' }).includes('<dt>kind'));
  assert.equal(card('pandacode_vm_screenshot', { name: 'vm' }), '');
  assert.ok(!card('pandacode_track_files', { paths: ['a'] }, '{"captureId":"cap-1"}').includes('Capture cap-1'));
  const mcp = card('mcp__github__list_issues', { repo: 'a/b' }, '{"total":1}');
  assert.ok(!mcp.includes('tool-trace-chip'));
  assert.match(mcp, /<h5 class="tool-trace-label">Arguments<\/h5>/);
  assert.ok(!card('Task', { subagent_type: 'Explore', prompt: 'p' }, 'r').includes('tool-trace-chip'));
  const activity = (name, input, output) => renderToolActivity({ id: '1', name, status: 'completed', input: JSON.stringify(input), output }, 'k');
  assert.ok(!activity('pandacode_vm_list', {}, '{"vms":[]}').includes('tool-trace-raw'));
  assert.match(activity('mcp__x__y', {}, 'line\n'.repeat(20)), /tool-trace-raw/);
  // Rich cards for built-in tools are excerpts and always keep the raw record.
  assert.match(activity('Bash', { command: 'ls' }, 'a'), /tool-trace-raw/);
  assert.match(activity('Bash', { command: 'ls' }, Array.from({ length: 20 }, (_, i) => `line ${i}`).join('\n')), /tool-trace-raw/);
  assert.match(activity('Read', { file_path: 'a' }, Array.from({ length: 400 }, (_, i) => `x ${i}`).join('\n')), /tool-trace-raw/);
  assert.match(activity('Read', { file_path: 'a' }, 'one\ntwo'), /tool-trace-raw/);
  assert.match(activity('pandacode_open_terminal', {}, ''), /data-static><div class="tool-trace-summary"/);
  assert.ok(!activity('pandacode_open_terminal', {}, '').includes('<details'));
  assert.ok(!renderToolActivity({ id: '1', name: 'Bash', status: 'running', input: '{"command":"ls"}', output: '' }, 'k').includes('Waiting for output'));
  assert.match(toolPreview(tool('pandacode_vm_screenshot', { name: 'vm' }, { output: 'Screenshot 1280×800' })), /^<p data-trace-kind="mcp" class="tool-trace-inline tool-trace-note">Screenshot 1280×800<\/p>$/);
  assert.equal(card('pandacode_set_task_stage', { taskId: 't', stage: 'review' }, '{"id":"t","title":"T","stage":"review"}'), '<!--trace-complete-->');
  assert.match(activity('pandacode_set_task_stage', { taskId: 't', stage: 'review' }, '{"id":"t","title":"T","stage":"review"}'), /data-static/);
  const one = card('pandacode_create_task', { title: 'T', description: 'Do it' }, '{"id":"t","title":"T","description":"Do it"}');
  assert.equal(one.match(/Do it/g)?.length, 1);
  assert.ok(!activity('pandacode_open_terminal', {}, '').includes('No output was returned'));
  assert.ok(!activity('Bash', { command: 'true' }, '').includes('No output was returned'));
});

test('summaries keep finished work in past tense, name PandaCode families and count nonzero exits', () => {
  const bash = tool('Bash', { command: 'ls' });
  assert.equal(summarizeTools([bash, { ...bash, status: 'running' }], { live: true }), 'Ran 1 command · 1 running');
  assert.equal(summarizeTools([tool('pandacode_send_team_message', {}), tool('pandacode_read_team_mailbox', {}), tool('pandacode_vm_list', {})]), 'Coordinated 2 team steps and operated 1 VM action');
  assert.equal(summarizeTools([tool('pandacode_cloud_cli', {}), bash]), 'Ran 1 cloud command and 1 command');
  const failing = tool('Bash', { command: 'npm test' }, { output: 'Exit code: 1\nOutput:\nfail' });
  assert.equal(summarizeTools([failing, bash]), 'Ran 1 command · 1 failed');
  const trace = describeTool(failing);
  assert.equal(trace.exit, 1);
  assert.match(traceRowHtml(trace), /is-failed">exit 1</);
  assert.match(renderToolActivity({ id: '1', ...failing }, 'k'), /data-tool-failed/);
  assert.ok(!renderToolActivity({ id: '1', ...bash, output: 'Exit code: 0\nOutput:\nok' }, 'k').includes('data-tool-failed'));
});

test('search keeps line numbers; generic arrays of objects become item lists; directory marks you', () => {
  assert.match(toolPreview(tool('Grep', { pattern: 'x' }, { output: 'src/a.ts:12:const x = 1' })), /tc-gutter">12</);
  // A recognizable collection becomes brand rows; other JSON keeps the argument/result tables.
  const mcp = toolPreview(tool('mcp__github__list_issues', {}, { output: JSON.stringify({ total: 2, items: [{ number: 812, title: 'Rows wrap' }, { number: 815, title: 'Dark chips' }] }) }));
  assert.match(mcp, /trace-row-head"><b>Rows wrap<\/b>/);
  assert.ok(!mcp.includes('2 items'));
  const generic = toolPreview(tool('mcp__x__get_config', {}, { output: JSON.stringify({ name: 'app', regions: ['us', 'eu'] }) }));
  assert.match(generic, /<dt>regions<\/dt><dd>us, eu<\/dd>/);
  const dir = toolPreview(tool('pandacode_team_directory', {}, { output: JSON.stringify({ you: 'w1', workers: [{ id: 'w1', name: 'Traces' }, { id: 'w2', name: 'Audit' }] }) }));
  assert.ok(!dir.includes('You are'));
  assert.match(dir, /Traces<\/b><span class="tool-trace-badge is-quiet">you</);
});

test('subagent cards: compact while running, result first with the prompt behind a disclosure', () => {
  const running = toolPreview(tool('Task', { subagent_type: 'Explore', prompt: 'Find callers' }, { status: 'running' }));
  assert.equal(running, '');
  const withModel = toolPreview(tool('Task', { subagent_type: 'Explore', prompt: 'Find callers', model: 'haiku' }, { status: 'running' }));
  assert.match(withModel, /haiku<\/span><span class="tool-trace-badge is-live">running</);
  assert.ok(!withModel.includes('Find callers'));
  const done = toolPreview(tool('Task', { subagent_type: 'Explore', prompt: 'Find callers' }, { output: 'Found 3' }));
  assert.ok(done.indexOf('Found 3') < done.indexOf('tool-trace-aside'));
  assert.match(done, /<details class="tool-trace-aside"><summary>.*Prompt<\/summary><p class="tool-trace-prompt">Find callers<\/p><\/details>/);
});

test('vm_exec rows name their machine; created tasks keep the input description', async () => {
  const { toolPreview } = await import('../src/tool-trace.ts');
  const vm = describeTool(tool('pandacode_vm_exec', { name: 'ubuntu-dev', command: 'uname -a' }));
  assert.equal(vm.object, '-a · on ubuntu-dev');
  const created = toolPreview(tool('pandacode_create_task', { title: 'Polish', description: 'Tighten the onboarding copy' }, { output: JSON.stringify({ task: { id: 't1', title: 'Polish' } }) }));
  assert.match(created, /Tighten the onboarding copy/);
});
