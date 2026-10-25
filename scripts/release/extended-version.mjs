// New Extended releases use independent three-component SemVer.
// Legacy four-component/hardened note filenames remain readable only as history.
import { version } from './common.mjs';

const CORE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function extendedVersion(value) {
  if (!CORE.test(value ?? '')) throw new Error('Extended release version must be X.Y.Z (SemVer core, no +metadata).');
  return version(value);
}

export function upstreamReference(value) {
  if (!CORE.test(value ?? '')) throw new Error('Tracked PandaCode version must be X.Y.Z.');
  return version(value);
}

export const displayVersion = (extended, original) => extendedVersion(extended) + '+pc.' + upstreamReference(original);
export const releaseNotePath = extended => 'releases/extended/' + extendedVersion(extended) + '.md';
export const releaseDateKey = extended => 'extended/' + extendedVersion(extended);

// Keep original PandaCode dates and Extended dates in distinct namespaces.
export function sortReleaseDates(entries) {
  for (const key of Object.keys(entries)) {
    if (key.startsWith('extended/')) version(key.slice('extended/'.length));
    else upstreamReference(key);
  }
  const byNumber = ([a], [b]) => a.localeCompare(b, 'en', { numeric: true });
  const originals = Object.entries(entries).filter(([key]) => !key.startsWith('extended/')).sort(byNumber);
  const extended = Object.entries(entries).filter(([key]) => key.startsWith('extended/')).sort(byNumber);
  return Object.fromEntries([...originals, ...extended]);
}
