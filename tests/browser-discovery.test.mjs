import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('every required Connect and trace browser suite is collected by the canonical configuration', () => {
  const required = ['agent-traces', 'remote-security', 'remote-pairing', 'remote-diagnostics', 'remote-connect-ux']
    .map(name => `${name}.spec.ts`);
  const root = fileURLToPath(new URL('../', import.meta.url));
  const cli = fileURLToPath(new URL('../node_modules/@playwright/test/cli.js', import.meta.url));
  const result = spawnSync(process.execPath, [cli, 'test', ...required, '--list', '--reporter=json'], {
    cwd: root, encoding: 'utf8', timeout: 60_000, maxBuffer: 8 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout || String(result.error));
  const report = JSON.parse(result.stdout);
  assert.deepEqual(report.errors, [], 'discovery must not ignore load errors');
  const files = new Set();
  const visit = suites => {
    for (const suite of suites) {
      for (const spec of suite.specs ?? []) if (spec.tests?.length) files.add(spec.file.split(/[\\/]/).at(-1));
      visit(suite.suites ?? []);
    }
  };
  visit(report.suites);
  for (const file of required) assert.ok(files.has(file), `${file} must execute in CI, not just appear in its command`);
});
