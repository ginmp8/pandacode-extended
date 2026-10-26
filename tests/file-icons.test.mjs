import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { fileIconId, fileIconUrl } from '../src/file-icons.ts';

test('files and folders get the Material Icon Theme icon VS Code would show', () => {
  for (const [name, isDir, expected] of [
    ['Start-SzG-Collector-v22.exe', false, 'exe'], ['SzDuilibEx.dll', false, 'dll'], ['SzG-Technical-Overview.md', false, 'markdown'],
    ['更新日志.txt', false, 'document'], ['package.json', false, 'nodejs'], ['types.d.ts', false, 'typescript-def'], ['App.TSX', false, 'react_ts'],
    ['no-extension', false, 'file'], ['.gitignore', false, 'git'],
    ['json', true, 'folder-json'], ['lua', true, 'folder-lua'], ['Bin', true, 'folder-dist'], ['SzG-Appendices', true, 'folder'],
  ]) assert.equal(fileIconId(name, isDir), expected, name);
});

test('names matching Object.prototype keys fall back to the defaults instead of throwing', () => {
  for (const name of ['constructor', '__proto__', 'tostring', 'a.constructor', 'hasownproperty']) {
    assert.equal(fileIconId(name, true), 'folder', name);
    assert.equal(fileIconId(name, false), 'file', name);
    assert.ok(fileIconUrl(name, false));
  }
});

test('the light theme uses light variants where they exist and dark icons otherwise', () => {
  assert.equal(fileIconId('page.jinja', false, true), 'jinja_light');
  assert.equal(fileIconId('page.jinja', false, false), 'jinja');
  assert.equal(fileIconId('app.exe', false, true), 'exe');
});

test('every resolved icon points at a shipped SVG', () => {
  for (const [name, isDir] of [['a.exe', false], ['x.unknown', false], ['src', true], ['Login', true]]) {
    assert.ok(existsSync(fileURLToPath(fileIconUrl(name, isDir))), name);
  }
});

// npm run build runs before npm test in CI; this ensures native installers ship
// every icon referenced by the installed manifest, not just icons used by tests.
test('production build includes every Material Icon Theme SVG', () => {
  const manifest = JSON.parse(readFileSync(new URL('../node_modules/material-icon-theme/dist/material-icons.json', import.meta.url), 'utf8'));
  const files = new Set(Object.values(manifest.iconDefinitions).map(definition => definition.iconPath.split('/').pop()));
  for (const filename of files) {
    assert.ok(existsSync(new URL(`../dist/material-icons/${filename}`, import.meta.url)), `missing packaged icon ${filename}`);
  }
});
