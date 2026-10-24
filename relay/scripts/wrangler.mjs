/** No npx, network fallback or shell command evaluation. Supply a vetted,
 * already-installed Wrangler CLI, with exact version 4.151.0.
 */
import { existsSync, realpathSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { isAbsolute, resolve } from 'node:path';

const input = process.argv.slice(2);
const operation = input.join(' ');
const allowed = new Set(['dev', 'deploy', 'secret put SESSION_SECRET', 'secret put GITHUB_CLIENT_ID', 'secret put GITHUB_CLIENT_SECRET']);
if (!allowed.has(operation)) {
  console.error('Only reviewed Wrangler dev, deploy and explicit relay secret-put operations are supported.');
  process.exit(2);
}
const configured = process.env.PANDACODE_WRANGLER_PATH;
if (configured && !isAbsolute(configured)) {
  console.error('PANDACODE_WRANGLER_PATH must be an absolute path to a reviewed Wrangler .js entrypoint.');
  process.exit(2);
}
const candidate = configured ?? resolve('node_modules', 'wrangler', 'bin', 'wrangler.js');
if (!existsSync(candidate) || !candidate.endsWith('.js')) {
  console.error('Wrangler is not installed locally. Set PANDACODE_WRANGLER_PATH to a reviewed Wrangler 4.151.0 .js entrypoint. No packages will be downloaded.');
  process.exit(2);
}
const cli = realpathSync(candidate);
const env = { ...process.env, WRANGLER_SEND_METRICS: 'false', CI: '1' };
const version = spawnSync(process.execPath, [cli, '--version'], { encoding: 'utf8', timeout: 15_000, maxBuffer: 64 * 1024, env });
if (version.error || version.status !== 0 || !/(?:^|\s)4\.151\.0(?:\s|$)/.test(`${version.stdout ?? ''} ${version.stderr ?? ''}`)) {
  console.error('Wrangler must be exactly version 4.151.0; refusing to run an unreviewed version.');
  process.exit(2);
}
const result = spawnSync(process.execPath, [cli, ...input], { stdio: 'inherit', timeout: 1_860_000, env });
if (result.error) {
  console.error('Wrangler failed or exceeded its deadline; remote operation outcome may be indeterminate.');
  process.exit(1);
}
process.exit(result.status ?? 1);
