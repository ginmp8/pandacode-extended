import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import test from 'node:test';
import { connectBuild } from '../src/connect/build-policy.ts';
import { parseToken } from '../src/connect/token.ts';

test('Connect requires an explicit HTTPS origin and OAuth client', () => {
  assert.equal(connectBuild('', '').build, null);
  assert.equal(connectBuild('https://relay.example', '').build, null);
  assert.deepEqual(connectBuild(' https://Relay.Example:443/ ', 'client_123').build, { relay: 'https://relay.example', clientId: 'client_123' });
  for (const url of ['http://relay.example', 'https://user:password@relay.example', 'https://relay.example/path', 'https://relay.example?q=1', 'https://relay.example#fragment', 'https:////relay.example', 'https://relay.example\\', 'https://relay.example extra']) assert.equal(connectBuild(url, 'client').build, null, url);
});

test('the distributable has Extended identity, no hosted project ownership or update destination', () => {
  const config = JSON.parse(readFileSync(new URL('../app.json', import.meta.url), 'utf8')).expo;
  const packageJson = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(config.name, 'PandaCode Extended');
  assert.equal(config.version, packageJson.version);
  assert.equal(config.owner, undefined);
  assert.equal(config.extra.eas, undefined);
  assert.equal(config.extra.relayUrl, '');
  assert.equal(config.extra.githubClientId, '');
  assert.equal(config.updates.enabled, false);
  assert.equal(config.ios.bundleIdentifier, 'ai.pandacode.extended.mobile');
  assert.equal(config.android.package, 'ai.pandacode.extended.mobile');
  assert.equal(existsSync(new URL('../eas.json', import.meta.url)), false);
  assert.equal(packageJson.scripts['submit:ios'], undefined);
  assert.equal(packageJson.scripts['build:ios'], undefined);
});

test('local token claims must contain bounded identity and integral timestamps', () => {
  const good = { acct: 'gh:123', dev: '3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f', iat: 1_000, exp: 2_000 };
  const token = (claims: unknown) => `${Buffer.from(JSON.stringify(claims)).toString('base64url')}.signature`;
  assert.deepEqual(parseToken(token(good)), good);
  for (const change of [{ acct: '' }, { dev: 'other' }, { iat: -1 }, { iat: 1.5 }, { exp: Infinity }, { exp: 999 }, { exp: Number.MAX_SAFE_INTEGER + 1 }]) assert.equal(parseToken(token({ ...good, ...change })), null);
  for (const bad of ['', 'a.b.c', token(good) + '.extra', 'x'.repeat(4097)]) assert.equal(parseToken(bad), null);
});
