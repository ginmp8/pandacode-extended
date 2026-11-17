import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const script = fileURLToPath(new URL('./retry-macos-dmg-build.sh', import.meta.url));
// Only macOS uses this Bash-specific build helper; exercise it on Unix runners.
const shellTest = process.platform === 'win32' ? test.skip : test;

function exercise(mode) {
  const dir = mkdtempSync(resolve(tmpdir(), 'pandacode-dmg-retry-test-'));
  try {
    const bin = resolve(dir, 'bin');
    mkdirSync(bin);
    writeFileSync(resolve(bin, 'node'), [
      '#!/usr/bin/env bash',
      'set -euo pipefail',
      'count=0',
      'if [[ -f "$CASE_DIR/count" ]]; then count="$(cat "$CASE_DIR/count")"; fi',
      'count=$((count + 1))',
      'printf "%s" "$count" > "$CASE_DIR/count"',
      'case "$MOCK_MODE" in',
      '  success) exit 0 ;;',
      '  busy-once) if (( count > 1 )); then exit 0; fi ;;',
      '  generic) echo "error: Rust linker failure" >&2; exit 1 ;;',
      '  always-busy) ;;',
      'esac',
      'echo "failed to bundle project: error running bundle_dmg.sh" >&2',
      "echo \"hdiutil: couldn't eject disk2 - Resource busy\" >&2",
      'exit 1',
    ].join('\n') + '\n');
    writeFileSync(resolve(bin, 'sleep'), '#!/usr/bin/env bash\nexit 0\n');
    chmodSync(resolve(bin, 'node'), 0o755);
    chmodSync(resolve(bin, 'sleep'), 0o755);
    const result = spawnSync('bash', [script], {
      cwd: resolve(script, '../../..'),
      encoding: 'utf8',
      env: {
        ...process.env, MOCK_MODE: mode, CASE_DIR: dir, RUNNER_TEMP: dir,
        PATH: [bin, process.env.PATH].join(delimiter),
      },
    });
    return { ...result, attempts: Number(readFileSync(resolve(dir, 'count'), 'utf8')) };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

shellTest('successful macOS DMG build runs once', () => {
  const result = exercise('success');
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.attempts, 1);
});

shellTest('transient hdiutil busy failure retries and preserves success', () => {
  const result = exercise('busy-once');
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.attempts, 2);
});

shellTest('real compiler failure is not retried', () => {
  const result = exercise('generic');
  assert.notEqual(result.status, 0);
  assert.equal(result.attempts, 1);
});

shellTest('persistently busy DMG fails after three attempts', () => {
  const result = exercise('always-busy');
  assert.notEqual(result.status, 0);
  assert.equal(result.attempts, 3);
});

test('release workflow uses the guarded retry only for macOS', () => {
  const workflow = readFileSync(new URL('../../.github/workflows/release.yml', import.meta.url), 'utf8');
  assert.match(workflow, /runner\.os == 'macOS' && 'bash scripts\/release\/retry-macos-dmg-build\.sh' \|\| 'node scripts\/release\/build\.mjs'/);
});
