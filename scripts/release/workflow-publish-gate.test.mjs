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
  assert.equal(jobCondition('npm'), "if: github.event_name == 'workflow_dispatch' && inputs.publish == true && github.ref_type == 'tag'");
});
