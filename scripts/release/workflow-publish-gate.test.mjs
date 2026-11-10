import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const workflow = readFileSync(new URL('../../.github/workflows/release.yml', import.meta.url), 'utf8');

function jobCondition(job) {
  const lines = workflow.split(/\r?\n/);
  const start = lines.indexOf(`  ${job}:`);
  assert.notEqual(start, -1, `Missing release workflow job: ${job}`);
  const endOffset = lines.slice(start + 1).findIndex(line => /^  [a-zA-Z][\w-]*:$/.test(line));
  const end = endOffset === -1 ? lines.length : start + 1 + endOffset;
  const condition = lines.slice(start + 1, end).find(line => /^    if: /.test(line));
  assert.ok(condition, `Missing publication guard on ${job}`);
  return condition.trim();
}

test('publishing requires an explicit manual workflow dispatch', () => {
  assert.equal(jobCondition('publish'), "if: github.event_name == 'workflow_dispatch' && inputs.publish == true");
  assert.doesNotMatch(workflow, /^  npm:\s*$/m, 'Extended release workflow must not contain an npm publish job');
  assert.doesNotMatch(workflow, /\bnpm publish\b/, 'Extended release workflow must not publish packages to npm');
});


test('Extended and upstream release tag identities are resolved separately', () => {
  assert.match(workflow, /node scripts\/release\/resolve-release-identity\.mjs/);
  assert.match(workflow, /RELEASE_TAG: \$\{\{ needs\.prepare\.outputs\.release_tag \}\}/);
  assert.match(workflow, /RELEASE_NOTES: \$\{\{ needs\.prepare\.outputs\.release_notes \}\}/);
  assert.match(workflow, /gh release create "\$RELEASE_TAG"/);
  assert.match(workflow, /updater-manifest\.mjs assets "\$VERSION" "\$RELEASE_NOTES" "\$RELEASE_TAG"/);
});


test('manual publishing requires main push CI for the exact commit', () => {
  const prepare = workflow.split('  prepare:')[1].split('\n  build:')[0];
  assert.match(prepare, /actions: read/);
  assert.match(prepare, /Require a green CI run for this commit/);
  assert.match(prepare, /if: github\.event_name == 'workflow_dispatch' && inputs\.publish == true/);
  assert.match(prepare, /gh run list .*--workflow ci\.yml --commit "\$GITHUB_SHA"/);
  assert.match(prepare, /--event push --branch main\b/);
  assert.match(prepare, /status=\$\(jq -r/);
  assert.match(prepare, /conclusion=\$\(jq -r/);
  assert.match(prepare, /url=\$\(jq -r/);
  assert.match(prepare, /case "\$conclusion" in/);
  assert.match(prepare, /cancelled\)/);
  assert.match(prepare, /Re-run CI for this commit/);
  assert.match(prepare, /No CI run from a push to main/);
});

test('failed or malformed CI lookup aborts manual publication with a diagnostic', () => {
  const prepare = workflow.split('  prepare:')[1].split('\n  build:')[0];
  assert.match(prepare, /if \[ -z "\$runs" \] \|\| ! jq -e 'type == "array"'/);
  assert.match(prepare, /CI lookup for \$GITHUB_SHA failed: gh did not return a list of runs/);
});

test('nonzero CI lookup exit status blocks manual publication', () => {
  const prepare = workflow.split('  prepare:')[1].split('\n  build:')[0];
  assert.match(prepare, /if ! runs=\$\(gh run list/);
  assert.match(prepare, /echo "\$lookup_failed"/);
  assert.match(prepare, /exit 1/);
});

test('main-push CI is not cancelled by later pushes', () => {
  const ci = readFileSync(new URL('../../.github/workflows/ci.yml', import.meta.url), 'utf8');
  assert.match(ci, /cancel-in-progress: \$\{\{ github\.event_name == 'pull_request' \}\}/);
});
