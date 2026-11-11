import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const ci = readFileSync(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8');
const platforms = ci.split('\n  rust-platforms:')[1];

test('release-profile audio smoke reaches all upstream platforms with required macOS capture', () => {
  assert.ok(platforms, 'Rust platform matrix must be present');
  assert.match(platforms, /if: runner\.os == 'macOS' \|\| matrix\.target == 'windows-x86_64' \|\| matrix\.target == 'linux-aarch64'/);
  assert.match(platforms, /PANDACODE_REQUIRE_REAL_MIC: \$\{\{ runner\.os == 'macOS' && '1' \|\| '' \}\}/);
  assert.match(platforms, /timeout-minutes: 70/, 'Extended cold macOS build allowance must remain');
  assert.match(platforms, /name: Release startup smoke tests \(Windows arm64\)/);
});

test('Extended CI keeps security jobs, native dependency recovery and protected push runs', () => {
  assert.match(ci, /name: Dependency audit \(npm lockfiles\)/);
  assert.match(ci, /name: RustSec audit \(Cargo\.lock\)/);
  assert.match(ci, /branches: \[main, base\/development\]/);
  assert.match(ci, /APT::Update::Error-Mode=any/);
  assert.match(ci, /cancel-in-progress: \$\{\{ github\.event_name != 'push' \}\}/);
  assert.equal((ci.match(/timeout-minutes: 10\n\s*run: \|/g) || []).length, 2);
});
