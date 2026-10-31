import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

test('Expo config deep-merge dependency preserves values without prototype poisoning', () => {
  const merge = require('deepmerge') as (a: unknown, b: unknown) => Record<string, unknown>;
  const input = JSON.parse('{"ios":{"bundleIdentifier":"com.example.app"},"__proto__":{"injectedByConfig":true}}');
  const actual = merge({ ios: { supportsTablet: false } }, input);
  assert.deepEqual(actual.ios, { supportsTablet: false, bundleIdentifier: 'com.example.app' });
  assert.equal(Object.getPrototypeOf(actual), Object.prototype);
  assert.equal(({} as Record<string, unknown>).injectedByConfig, undefined);
  assert.equal(actual.injectedByConfig, undefined);
  const craftedTarget = JSON.parse('{"__proto__":{"elevatedByTarget":true},"ios":{"name":"example"}}');
  const fromTarget = merge(craftedTarget, { ios: { supportsTablet: false } });
  assert.equal(Object.getPrototypeOf(fromTarget), Object.prototype);
  assert.equal(({} as Record<string, unknown>).elevatedByTarget, undefined);
  assert.deepEqual(fromTarget.ios, { name: 'example', supportsTablet: false });
});

test('patched glob and minimatch retain basic Expo tool patterns', () => {
  const { globSync } = require('glob');
  const { minimatch } = require('minimatch');
  assert.equal(minimatch('src/app/index.tsx', '**/*.tsx'), true);
  assert.equal(minimatch('src/app/index.tsx', '**/*.ts'), false);
  const directory = mkdtempSync(join(tmpdir(), 'pandacode-extended-glob-'));
  try {
    writeFileSync(join(directory, 'sample.ts'), 'export const value = 1;');
    assert.deepEqual(globSync('*.ts', { cwd: directory }), ['sample.ts']);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('updated zod and node-forge retain basic validation and hashing', () => {
  const { z } = require('zod');
  assert.equal(z.object({ name: z.string().min(1) }).safeParse({ name: 'PandaCode' }).success, true);
  assert.equal(z.object({ name: z.string().min(1) }).safeParse({ name: '' }).success, false);
  const forge = require('node-forge');
  const digest = forge.md.sha256.create();
  digest.update('test');
  assert.equal(digest.digest().toHex(), '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08');
});
