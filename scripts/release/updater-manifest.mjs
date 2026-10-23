// Offline signature discovery remains available; writing an update feed is disabled.
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { targets } from './common.mjs';

export async function signaturesIn(directory) {
  const signatures = {};
  for (const [target, config] of Object.entries(targets)) {
    try { signatures[target] = await readFile(resolve(directory, `${config.updaterAsset}.sig`), 'utf8'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return signatures;
}

async function main() {
  throw new Error('Updater manifest generation is disabled in this local-only hardened distribution.');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
