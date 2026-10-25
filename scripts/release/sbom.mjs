// Explicit local inventory only. Uses already-installed scanners and an existing
// Grype database; never installs tools, updates a database, or publishes artifacts.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { copyFile, lstat, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { digest, root, targets } from './common.mjs';
import { releaseAssetName } from './build.mjs';
import { extendedVersion } from './extended-version.mjs';

const execute = promisify(execFile);
const MAX_OUTPUT = 32 * 1024 * 1024;
const SEVERITIES = ['Critical', 'High', 'Medium', 'Low', 'Negligible', 'Unknown'];
const json = text => { try { return JSON.parse(text); } catch { return undefined; } };
const safe = value => String(value).replace(/[|`<>\r\n]/g, ' ').slice(0, 300);

export function packagedAssets(names, releaseVersion) {
  const version = extendedVersion(releaseVersion);
  const wanted = new Map(Object.values(targets).map(target => [releaseAssetName(version, target.asset), `${target.platform}/${target.arch}`]));
  return names.filter(name => wanted.has(name)).sort().map(name => ({ name, platform: wanted.get(name) }));
}

export function scannerEnvironment(inherited) {
  // An inherited scanner option must not override the explicit offline policy,
  // load an unrelated template/configuration, or filter out vulnerability results.
  const env = Object.fromEntries(Object.entries(inherited).filter(([name]) => !/^(SYFT|GRYPE)_/i.test(name)));
  if (inherited.GRYPE_DB_CACHE_DIR) env.GRYPE_DB_CACHE_DIR = inherited.GRYPE_DB_CACHE_DIR;
  return {
    ...env, SYFT_CHECK_FOR_APP_UPDATE: 'false', GRYPE_CHECK_FOR_APP_UPDATE: 'false', GRYPE_DB_AUTO_UPDATE: 'false',
    SYFT_GOLANG_SEARCH_REMOTE_LICENSES: 'false', SYFT_GOLANG_USE_PACKAGES_LIB: 'false', SYFT_JAVA_USE_NETWORK: 'false',
    SYFT_JAVASCRIPT_SEARCH_REMOTE_LICENSES: 'false', SYFT_PYTHON_SEARCH_REMOTE_LICENSES: 'false', SYFT_CPP_VCPKG_ALLOW_GIT_CLONE: 'false',
  };
}

export function inventoryStatus({ bom, error }) {
  if (error) return { status: 'incomplete', components: 0, reason: error };
  if (bom?.bomFormat !== 'CycloneDX' || !Array.isArray(bom.components) || !bom.components.length) {
    return { status: 'incomplete', components: 0, reason: 'Missing, malformed or empty CycloneDX inventory' };
  }
  return { status: 'ok', components: bom.components.length };
}

export function scanStatus(inventory, { report, error }) {
  if (inventory.status !== 'ok') return { status: 'not-scanned', reason: 'Inventory incomplete' };
  if (error || !Array.isArray(report?.matches)) return { status: 'failed', reason: error ?? 'Missing or malformed Grype report' };
  const severities = Object.fromEntries(SEVERITIES.map(severity => [severity, 0]));
  for (const match of report.matches) {
    const severity = match?.vulnerability?.severity;
    severities[SEVERITIES.includes(severity) ? severity : 'Unknown']++;
  }
  return { status: 'scanned', findings: report.matches.length, severities };
}

export function coverage({ version, commit, scanners, dbBuilt, entries, notes }) {
  const incomplete = entries.filter(entry => entry.inventory.status !== 'ok' || entry.scan.status !== 'scanned').map(entry => entry.name);
  const hasInstaller = entries.some(entry => entry.kind === 'installer');
  return { version, commit, scanners, vulnerabilityDbBuilt: dbBuilt ?? 'unknown', entries, notes,
    complete: hasInstaller && incomplete.length === 0, hasInstaller, incomplete };
}

export function markdown(report) {
  const rows = report.entries.map(entry => `| \`${safe(entry.name)}\` | ${safe(entry.platform)} | ${entry.kind} | ${entry.inventory.status} (${entry.inventory.components}) | ${entry.scan.status}${entry.scan.findings === undefined ? '' : ` (${entry.scan.findings} findings)`} |`);
  return [
    `# Local inventory: PandaCode Extended ${report.version}`, '',
    `Source commit: \`${report.commit}\`. Database built: ${safe(report.vulnerabilityDbBuilt)}.`, '',
    report.complete ? 'All listed inventories were identified and scanned. Findings still require review.' : '**Coverage is incomplete.** Empty/failed inventories and unavailable scans are not clean results.', '',
    '| Subject | Platform | Kind | Inventory | Scan |', '| --- | --- | --- | --- | --- |', ...rows, '',
    'Full hashes, scanner versions, failure reasons and severity counts are in `SBOM-COVERAGE.json`.', '',
    '## Limits', '', ...report.notes.map(note => `- ${note}`), '',
  ].join('\n');
}

const NOTES = [
  'Only installers actually present in the input directory are listed; this is not coverage of all release platforms.',
  'Compressed installers may yield no identifiable components. That result remains incomplete.',
  'Lockfile inventories describe source dependencies, not proven contents of a native installer.',
  'The website, relay, mobile application, private disabled npx package and host WebView/OS libraries are outside this desktop inventory.',
  'Scanners are already installed locally. Application/database updates and remote enrichment are disabled; a missing or stale database is not silently refreshed.',
  'Findings are reported for review. This helper does not sign, attest, upload or publish anything, and is not a security certification.',
];

export async function generate({ assets, out, version, commit, sourceRoot = root, runner = execute, inherited = process.env }) {
  extendedVersion(version);
  if (!/^[a-f0-9]{40}$/.test(commit ?? '')) throw new Error('Supply the full 40-character source commit.');
  const pkg = json(await readFile(resolve(sourceRoot, 'package.json'), 'utf8'));
  if (pkg?.version !== version) throw new Error('The source package version does not match the inventory version.');
  const selected = packagedAssets(await readdir(assets), version);
  // A fresh directory prevents old scanner output from being mistaken for this run.
  await mkdir(out);
  const config = resolve(out, 'scanner-policy.yaml');
  await writeFile(config, 'check-for-app-update: false\nenrich: []\ndb:\n  auto-update: false\n  validate-age: true\n  require-update-check: false\n');
  const env = scannerEnvironment(inherited);
  const tool = async (name, args) => {
    try {
      const result = await runner(name, ['--config', config, ...args], { cwd: sourceRoot, env, timeout: 300_000, maxBuffer: MAX_OUTPUT, encoding: 'utf8' });
      const value = json(result.stdout);
      return value === undefined ? { error: `${name} returned invalid JSON` } : { value };
    } catch (error) {
      // Do not copy arbitrary scanner stderr (which can contain environment or
      // document data) into a release report.
      return { error: `${name} failed (${safe(error.code ?? (error.killed ? 'timeout' : 'execution error'))})` };
    }
  };
  const syft = await tool('syft', ['version', '-o', 'json']);
  const grype = await tool('grype', ['version', '-o', 'json']);
  const db = await tool('grype', ['db', 'status', '-o', 'json']);
  const subjects = selected.map(entry => ({ ...entry, kind: 'installer', path: resolve(assets, entry.name) }));
  // Isolate exact lockfiles; scanning dir:. would also inventory unrelated nested
  // packages, build output, private checkouts and the report being generated.
  for (const [name, files] of [
    ['extended-npm-lock', ['package.json', 'package-lock.json']],
    ['extended-cargo-lock', ['src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']],
  ]) {
    const directory = resolve(out, name);
    await mkdir(directory);
    const hashes = {};
    for (const file of files) {
      await copyFile(resolve(sourceRoot, file), resolve(directory, file.split('/').at(-1)));
      hashes[file] = await digest(resolve(directory, file.split('/').at(-1)));
    }
    subjects.push({ name, kind: 'source dependencies', platform: 'all', directory, sourceHashes: hashes });
  }
  const entries = [];
  for (const subject of subjects) {
    const regular = !subject.path || (await lstat(subject.path)).isFile();
    const source = subject.path ? `file:${subject.path}` : `dir:${subject.directory}`;
    const result = regular ? await tool('syft', ['scan', source, '-o', 'cyclonedx-json', '-q']) : { error: 'Installer is not a regular file' };
    const inventory = inventoryStatus({ bom: result.value, error: result.error });
    const sbom = `${subject.name}.cdx.json`;
    if (inventory.status === 'ok') await writeFile(resolve(out, sbom), JSON.stringify(result.value, null, 2) + '\n');
    const scanResult = inventory.status === 'ok'
      ? db.error ? { error: 'Local vulnerability database is unavailable' } : await tool('grype', [`sbom:${resolve(out, sbom)}`, '-o', 'json', '-q'])
      : {};
    const scan = scanStatus(inventory, { report: scanResult.value, error: scanResult.error });
    if (scan.status === 'scanned') await writeFile(resolve(out, `${subject.name}.grype.json`), JSON.stringify(scanResult.value, null, 2) + '\n');
    entries.push({ name: subject.name, kind: subject.kind, platform: subject.platform, sourceHashes: subject.sourceHashes,
      sha256: subject.path && regular ? await digest(subject.path) : undefined, sbom: inventory.status === 'ok' ? sbom : undefined, inventory, scan });
  }
  const report = coverage({ version, commit, scanners: { syft: syft.value?.version ?? 'unavailable', grype: grype.value?.version ?? 'unavailable' },
    dbBuilt: db.value?.built, entries, notes: NOTES });
  await writeFile(resolve(out, 'SBOM-COVERAGE.json'), JSON.stringify(report, null, 2) + '\n');
  await writeFile(resolve(out, 'SBOM-COVERAGE.md'), markdown(report));
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [assets, out, version, commit] = process.argv.slice(2);
  if (!assets || !out || !version || !commit) {
    console.error('Usage: sbom.mjs <existing-installers-dir> <new-output-dir> <X.Y.Z> <full-source-commit>');
    process.exitCode = 1;
  } else generate({ assets: resolve(assets), out: resolve(out), version, commit }).then(report => {
    console.log(`Local coverage ${report.complete ? 'complete for listed subjects' : 'incomplete'}; ${report.entries.length} subjects. See SBOM-COVERAGE.json.`);
    if (!report.complete) process.exitCode = 2;
  }).catch(error => { console.error(error.message); process.exitCode = 1; });
}
