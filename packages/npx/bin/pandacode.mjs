#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { install, supportedPlatforms } from '../installer.mjs';

const args = process.argv.slice(2);
if (args.includes('--help') || args.includes('-h')) {
  console.log(`Usage: node packages/npx/bin/pandacode.mjs [--help] [--version]

Remote installation is disabled in this hardened build.
Build the reviewed source locally using CONTRIBUTING.md.
Do not use npx pandacode: it installs the upstream, unhardened release.

  --version   print this source package's version`);
} else if (args.includes('--version') || args.includes('-v')) {
  console.log(JSON.parse(readFileSync(new URL('../package.json', import.meta.url))).version);
} else {
  install({ dryRun: args.includes('--dry-run') }).catch(error => {
    console.error(`pandacode: ${error.message}`);
    process.exit(1);
  });
}
