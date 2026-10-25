import test from 'node:test';
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join, relative } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const excludedDirectories = new Set([
  '.git', '.astro', '.cache', 'node_modules', 'dist', 'target', 'vendor',
  'security-review', 'release', 'test-results', 'playwright-report',
]);
const textFile = /\.(?:md|mjs|js|ts|tsx|rs|toml|json|jsonc|yaml|yml|html|css|astro|sh|xml|txt|lock|ps1|py|sql|svg|bat)$/i;
const obsolete = /0\.3\.42(?:\.1|\+1|-hardened\.1)/;

// Prevent retired intermediate candidate identifiers from reappearing in
// maintained source, docs and tests. Keep third-party and immutable audit
// evidence out of scope; original upstream release notes are also read-only.
test('active Extended source and documentation use the current 0.4.0 release identity', async () => {
  const matches = [];
  const visit = async directory => {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!excludedDirectories.has(entry.name)) await visit(join(directory, entry.name));
        continue;
      }
      if (!entry.isFile()) continue;
      const path = join(directory, entry.name);
      const name = relative(root, path).replaceAll('\\', '/');
      if (name.startsWith('releases/') && !name.startsWith('releases/extended/')) continue;
      if (!textFile.test(name) && !['LICENSE', '.gitignore', '.npmrc'].includes(entry.name)) continue;
      const source = await readFile(path, 'utf8');
      if (obsolete.test(name) || obsolete.test(source)) matches.push(name);
    }
  };
  await visit(root);
  assert.deepEqual(matches, [], 'Retired release identifiers remain in maintained source or documentation');
});
