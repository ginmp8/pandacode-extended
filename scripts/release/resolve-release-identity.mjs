// Keep upstream application/build identity separate from Extended distribution tags.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { version } from './common.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));

export function resolveReleaseIdentity({ appVersion, refType, refName, hasNotes }) {
  const app = version(appVersion);
  const releaseTag = refType === 'tag' ? refName : `v${app}`;
  if (typeof releaseTag !== 'string' || !releaseTag.startsWith('v')) throw new Error('Release tag must be vX.Y.Z.');
  const release = version(releaseTag.slice(1));
  const extended = release !== app;
  const notes = extended ? `releases/extended/${release}.md` : `releases/${release}.md`;
  if (!hasNotes(notes)) throw new Error(`Release notes ${notes} are missing or empty.`);
  return {
    version: app,
    release_version: release,
    release_notes: notes,
    release_tag: releaseTag,
    release_title: `${extended ? 'PandaCode Extended' : 'PandaCode'} ${release}`,
  };
}

function main() {
  const appVersion = JSON.parse(readFileSync(resolve(root, 'src-tauri/tauri.conf.json'), 'utf8')).version;
  const identity = resolveReleaseIdentity({
    appVersion,
    refType: process.env.GITHUB_REF_TYPE,
    refName: process.env.GITHUB_REF_NAME,
    hasNotes: path => {
      try { return Boolean(readFileSync(resolve(root, path), 'utf8').trim()); }
      catch (error) { if (error.code === 'ENOENT') return false; throw error; }
    },
  });
  for (const [key, value] of Object.entries(identity)) console.log(`${key}=${value}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(`::error::${error.message}`); process.exitCode = 1; }
}
