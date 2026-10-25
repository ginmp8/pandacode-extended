// Update a new Extended technical version. Upstream references are reviewed separately.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { root, cargoVersion, json, cargoLockVersion } from './common.mjs';
import { extendedVersion, displayVersion, releaseDateKey, releaseNotePath, sortReleaseDates } from './extended-version.mjs';

const next = extendedVersion(process.argv[2]);
const metaPath = resolve(root, 'EXTENDED-RELEASE.json');
const metadata = await json(metaPath);
const previous = await json(resolve(root, 'package.json'));
if (previous.version !== metadata.extended_version) {
  throw new Error('Extended metadata and package.json already disagree; reconcile before changing version.');
}
const full = displayVersion(next, metadata.tracked_upstream_version);
for (const path of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) {
  const absolute = resolve(root, path);
  const value = await json(absolute);
  value.version = path === 'src-tauri/tauri.conf.json' ? cargoVersion(next) : next;
  if (value.packages?.['']) value.packages[''].version = next;
  await writeFile(absolute, JSON.stringify(value, null, 2) + '\n');
}
const cargoPath = resolve(root, 'src-tauri/Cargo.toml');
const cargo = await readFile(cargoPath, 'utf8');
await writeFile(cargoPath, cargo.replace(/(\[package\][\s\S]*?\nversion = )"[^"]+"/, '$1"' + cargoVersion(next) + '"'));
const lockPath = resolve(root, 'src-tauri/Cargo.lock');
const lock = await readFile(lockPath, 'utf8');
await writeFile(lockPath, cargoLockVersion(lock, next));
metadata.extended_version = next;
metadata.display_version = full;
metadata.release_note = releaseNotePath(next);
await writeFile(metaPath, JSON.stringify(metadata, null, 2) + '\n');
const datesPath = resolve(root, 'website/src/data/release-dates.json');
const dates = await json(datesPath);
const key = releaseDateKey(next);
const today = new Date().toISOString().slice(0, 10);
dates[key] ??= today;
await writeFile(datesPath, JSON.stringify(sortReleaseDates(dates), null, 2) + '\n');
console.log('Extended version set to ' + full + '. Write ' + metadata.release_note + ', validate, then tag v' + next + ' only when publishing.');
