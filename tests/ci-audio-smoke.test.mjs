import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const ci = readFileSync(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8');
const platforms = ci.split('\n  rust-platforms:')[1];

test('release-profile audio smoke reaches all upstream platforms with required macOS capture', () => {
  assert.ok(platforms, 'Rust platform matrix must be present');
  assert.match(platforms, /if: runner\.os == 'macOS' \|\| matrix\.target == 'windows-x86_64' \|\| matrix\.target == 'linux-aarch64'/);
  assert.match(platforms, /PANDACODE_REQUIRE_REAL_MIC: \$\{\{ runner\.os != 'Windows' && '1' \|\| '' \}\}/);
  assert.match(platforms, /timeout-minutes: 70/, 'Extended cold macOS build allowance must remain');
  assert.match(platforms, /name: Release startup smoke tests \(Windows arm64\)/);
});

test('release-profile audio smoke runs the named ignored Rust test exactly', () => {
  const exactAudioCommand = 'cargo test --locked --release --manifest-path src-tauri/Cargo.toml --lib voice::tests::real_audio::real_audio_lists_opens_and_records -- --exact --ignored --nocapture';
  assert.equal(ci.split(exactAudioCommand).length - 1, 2, 'both Linux and matrix release jobs select the exact audio test');
  assert.equal(ci.split("grep -q 'test result: ok. 1 passed'").length - 1, 2, 'both release jobs reject an exact filter that runs zero tests');
  assert.equal((ci.match(/libasound2-plugins/g) || []).length >= 2, true, 'Linux runners install ALSA plugins for JACK fallback');
  assert.doesNotMatch(ci, /--lib real_audio -- --ignored/, 'broad filters could pass after a test rename');
  assert.match(ci, /Acquire::Retries=3/, 'keep upstream apt fetch retry count');
});

test('Extended CI keeps security jobs, native dependency recovery and protected push runs', () => {
  assert.match(ci, /name: Dependency audit \(npm lockfiles\)/);
  assert.match(ci, /name: RustSec audit \(Cargo\.lock\)/);
  assert.match(ci, /branches: \[main, base\/development\]/);
  assert.match(ci, /APT::Update::Error-Mode=any/);
  assert.match(ci, /cancel-in-progress: \$\{\{ github\.event_name == 'pull_request' \}\}/);
  assert.equal((ci.match(/timeout-minutes: 10\n\s*run: \|/g) || []).length, 2);
});
