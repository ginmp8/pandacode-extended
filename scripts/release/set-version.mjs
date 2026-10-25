import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { root, version, cargoVersion, json, cargoLockVersion } from './common.mjs';

const next = version(process.argv[2]);
for (const path of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) {
  const absolute = resolve(root, path);
  const value = await json(absolute);
  value.version = next;
  if (value.packages?.['']) value.packages[''].version = next;
  await writeFile(absolute, `${JSON.stringify(value, null, 2)}\n`);
}
const cargoPath = resolve(root, 'src-tauri/Cargo.toml');
const cargo = await readFile(cargoPath, 'utf8');
await writeFile(cargoPath, cargo.replace(/(\[package\][\s\S]*?\nversion = )"[^"]+"/, `$1"${cargoVersion(next)}"`));
const lockPath = resolve(root, 'src-tauri/Cargo.lock');
const lock = await readFile(lockPath, 'utf8');
await writeFile(lockPath, cargoLockVersion(lock, next));
// The website changelog (/changelog) dates each release from this map.
const datesPath = resolve(root, 'website/src/data/release-dates.json');
const dates = await json(datesPath);
const today = new Date().toISOString().slice(0, 10);
const entries = { ...dates, [next]: dates[next] ?? today };
const releaseVersion = (value) => {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?(?:-(hardened))?$/.exec(value);
  if (!match) throw new Error(`Invalid release date key: ${value}`);
  return [Number(match[1]), Number(match[2]), Number(match[3]), Number(match[4] ?? 0), match[5] ? 1 : 0];
};
const orderedDates = Object.fromEntries(Object.entries(entries).sort(([left], [right]) => {
  const a = releaseVersion(left), b = releaseVersion(right);
  for (let index = 0; index < a.length; index++) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
}));
await writeFile(datesPath, `${JSON.stringify(orderedDates, null, 2)}\n`);
console.log(`Release version set to ${next}. Commit these files, then tag v${next}.`);
