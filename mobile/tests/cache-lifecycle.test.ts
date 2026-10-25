import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import test from 'node:test';
import { encodeThread, fileName } from '../src/store/thread-cache.ts';
import { blobName } from '../src/store/attachments.ts';

type Operation = 'read' | 'write' | 'move';
const files = new Map<string, string>();
const directories = new Set(['cache']);
const holds = new Map<string, { entered(): void; wait: Promise<void> }>();
const uri = (...parts: (string | { uri: string })[]) => parts.map(p => typeof p === 'string' ? p : p.uri).join('/');

function hold(operation: Operation, path: string) {
  let entered!: () => void, release!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; });
  const wait = new Promise<void>(resolve => { release = resolve; });
  holds.set(`${operation}:${path}`, { entered, wait });
  return { started, release };
}

async function pause(operation: Operation, path: string) {
  const key = `${operation}:${path}`;
  const held = holds.get(key);
  if (!held) return;
  holds.delete(key);
  held.entered();
  await held.wait;
}

// Match SDK 58: file reads, writes and moves are asynchronous; creation and
// deletion are synchronous. Held operations model native work completing later.
class Directory {
  uri: string;
  constructor(...parts: (string | { uri: string })[]) { this.uri = uri(...parts); }
  get exists() { return directories.has(this.uri); }
  create() { directories.add(this.uri); }
  delete() {
    for (const name of files.keys()) if (name.startsWith(`${this.uri}/`)) files.delete(name);
    for (const name of directories) if (name === this.uri || name.startsWith(`${this.uri}/`)) directories.delete(name);
  }
}

class File {
  uri: string;
  constructor(...parts: (string | { uri: string })[]) { this.uri = uri(...parts); }
  get exists() { return files.has(this.uri); }
  get size() { return files.get(this.uri)?.length ?? 0; }
  create() { files.set(this.uri, ''); }
  delete() { files.delete(this.uri); }
  async text() {
    const text = files.get(this.uri);
    if (text === undefined) throw new Error('missing file');
    await pause('read', this.uri);
    return text;
  }
  async write(text: string, options?: { append?: boolean; encoding?: string }) {
    const path = this.uri;
    await pause('write', path);
    const value = options?.encoding === 'base64' ? Buffer.from(text, 'base64').toString('utf8') : text;
    files.set(path, `${options?.append ? files.get(path) ?? '' : ''}${value}`);
  }
  async move(target: File) {
    const path = this.uri;
    await pause('move', path);
    const content = files.get(path);
    if (content === undefined) throw new Error('missing partial');
    files.set(target.uri, content);
    files.delete(path);
    this.uri = target.uri;
  }
}

(globalThis as any).__mobileCacheFiles = { File, Directory, Paths: { cache: 'cache' } };
const hooks = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === 'expo-file-system') return { url: 'test-cache:files', shortCircuit: true };
    if (specifier === './index.ts' && context.parentURL?.includes('/src/store/')) return { url: 'test-cache:state', shortCircuit: true };
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === 'test-cache:files') return { format: 'module', shortCircuit: true,
      source: 'export const { File, Directory, Paths } = globalThis.__mobileCacheFiles;' };
    if (url === 'test-cache:state') return { format: 'module', shortCircuit: true,
      source: 'export const useConnect = { getState: () => ({ demo: false }) };' };
    return nextLoad(url, context);
  },
});
const threads = await import('../src/store/threads.ts');
const icons = await import('../src/store/icons.ts');
const blobs = await import('../src/store/attachment-files.ts');
const inbox = await import('../src/connect/inbox-file.ts');
hooks.deregister();

const key = 'host/task';
const transcript = [{ id: 1, role: 'user', content: 'Private task from the previous account', createdAt: 1, turnDurationMs: null }];
const threadPath = `cache/threads/${fileName(key)}`;
const iconPath = 'cache/project-icons.json';
const icon = { url: 'data:image/png;base64,aGVsbG8=', at: 1 };
const tick = () => new Promise<void>(resolve => setImmediate(resolve));

test.beforeEach(async () => {
  await Promise.all([threads.clearThreads(), icons.clearIcons(), blobs.clearBlobs(), inbox.clearInbox()]);
  files.clear();
  holds.clear();
});

function seedThread() {
  directories.add('cache/threads');
  files.set('cache/threads/index.json', JSON.stringify({ [key]: { file: fileName(key), at: 1, bytes: 10, lastId: 1 } }));
  files.set(threadPath, encodeThread(transcript, false));
}

test('a transcript read that finishes after cleanup cannot restore the old account', async () => {
  seedThread();
  const held = hold('read', threadPath);
  const reading = threads.readThread(key);
  await held.started;
  await threads.clearThreads();
  held.release();
  assert.equal(await reading, null);
  assert.equal(threads.cachedLastId(key), undefined);
  assert.equal(files.size, 0);
});

test('a pending index read cannot start a transcript write after cleanup', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  seedThread();
  const held = hold('read', 'cache/threads/index.json');
  threads.saveThread(key, () => ({ messages: transcript, hasOlder: false }));
  t.mock.timers.tick(1500);
  await held.started;
  await threads.clearThreads();
  held.release();
  await tick();
  assert.equal(files.size, 0);
  assert.equal(threads.cachedLastId(key), undefined);
});

test('transcript cleanup waits for a native write before removing the files', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const held = hold('write', threadPath);
  threads.saveThread(key, () => ({ messages: transcript, hasOlder: false }));
  t.mock.timers.tick(1500);
  await held.started;
  let settled = false;
  const clearing = Promise.resolve(threads.clearThreads()).then(() => { settled = true; });
  await tick();
  const settledEarly = settled;
  held.release();
  await clearing;
  await tick();
  assert.equal(settledEarly, false, 'cleanup cannot report success before the write finishes');
  assert.equal(files.size, 0);
});

test('project-icon reads and writes cannot repopulate a cleared cache', async () => {
  files.set(iconPath, JSON.stringify({ [key]: icon }));
  const readHold = hold('read', iconPath);
  const reading = icons.loadIcons();
  await readHold.started;
  await icons.clearIcons();
  readHold.release();
  await reading;
  assert.deepEqual(icons.useIcons.getState().cache, {});

  icons.useIcons.setState({ cache: { [key]: icon } });
  const writeHold = hold('write', iconPath);
  const writing = icons.saveIcons();
  await writeHold.started;
  let settled = false;
  const clearing = icons.clearIcons().then(() => { settled = true; });
  await tick();
  const settledEarly = settled;
  writeHold.release();
  await Promise.all([writing, clearing]);
  assert.equal(settledEarly, false);
  assert.equal(files.has(iconPath), false);
  assert.deepEqual(icons.useIcons.getState().cache, {});
});

test('inbox reads finishing after cleanup return no previous-account data', async () => {
  files.set('cache/inbox.json', 'old account inbox');
  const held = hold('read', 'cache/inbox.json');
  const reading = inbox.readInbox();
  await held.started;
  await inbox.clearInbox();
  held.release();
  assert.equal(await reading, null);
});

test('inbox cleanup drains active writes and cancels scheduled writes', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const held = hold('write', 'cache/inbox.json');
  inbox.saveInboxSoon(() => 'old account inbox');
  t.mock.timers.tick(1000);
  await held.started;
  inbox.saveInboxSoon(() => 'scheduled old inbox');
  let settled = false;
  const clearing = Promise.resolve(inbox.clearInbox()).then(() => { settled = true; });
  await tick();
  const settledEarly = settled;
  held.release();
  await clearing;
  t.mock.timers.tick(1000);
  await tick();
  assert.equal(settledEarly, false);
  assert.equal(files.has('cache/inbox.json'), false);
});

test('attachment cleanup drains writes and invalidates old partial handles', async () => {
  await blobs.loadBlobs();
  const file = blobs.partial(key);
  const held = hold('write', `cache/attachments/${blobName(key)}`);
  const writing = Promise.resolve(file.append(Buffer.from('old image').toString('base64'))).catch(error => error);
  await held.started;
  let settled = false;
  const clearing = Promise.resolve(blobs.clearBlobs()).then(() => { settled = true; });
  await tick();
  const settledEarly = settled;
  held.release();
  await Promise.all([writing, clearing]);
  assert.equal(settledEarly, false);
  assert.equal(files.size, 0);
  await assert.rejects(Promise.resolve().then(() => file.append('YQ==')), /cleared/);
});

test('an old partial cannot delete a new download with the same key', async () => {
  const old = blobs.partial(key);
  await old.append('b2xk');
  await blobs.clearBlobs();
  const fresh = blobs.partial(key);
  await fresh.append('bmV3');
  await old.reset();
  const uri = await fresh.finish('image/png', 3);
  assert.equal(files.get(uri), 'new');
  assert.equal(blobs.cachedBlob(key), uri);
});

test('attachment completion waits for the native move and cannot finish across cleanup', async () => {
  const file = blobs.partial(key);
  await file.append('aW1hZ2U=');
  const held = hold('move', `cache/attachments/${blobName(key)}`);
  const finishing = file.finish('image/png', 5).catch(error => error);
  await held.started;
  const clearing = blobs.clearBlobs();
  held.release();
  const result = await finishing;
  await clearing;
  assert.match(result.message, /cleared/);
  assert.equal(files.size, 0);
  assert.equal(blobs.cachedBlob(key), undefined);
});
