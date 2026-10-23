// Regression contract: installation must not re-enable unattended downloads,
// updater signing, mutable Cargo resolution, or npm lifecycle execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, access, rm, stat } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, resolve, dirname, isAbsolute } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const root = process.env.INSTALL_SECURITY_TEST_ROOT || fileURLToPath(new URL('../', import.meta.url));
const { buildPlan } = await import(pathToFileURL(join(root,'scripts/release/build.mjs')).href);
const bundle = plan => JSON.parse(plan.args[plan.args.indexOf('--config')+1]).bundle;
const targets = [
 ['windows-x86_64','win32','x64'], ['windows-aarch64','win32','arm64'],
 ['linux-x86_64','linux','x64'], ['linux-aarch64','linux','arm64'],
 ['darwin-x86_64','darwin','x64'], ['darwin-aarch64','darwin','arm64'],
];
for (const [target,platform,arch] of targets) {
 test(`${target}: inherited update secrets cannot restore updater output and builds keep the lockfile`, () => {
  const inherited = {PATH:'/usr/bin', TAURI_SIGNING_PRIVATE_KEY:'dummy-do-not-use',TAURI_SIGNING_PRIVATE_KEY_PASSWORD:'dummy',tauri_signing_private_key:'dummy-case',PANDACODE_UPDATE_PUBLIC_KEY:'stale'};
  const original = {...inherited};
  const plan=buildPlan(target,inherited,platform,arch);
  assert.equal(bundle(plan).createUpdaterArtifacts,false);
  assert.equal(plan.updater,false);
  for (const key of Object.keys(plan.env)) assert.doesNotMatch(key,/^TAURI_SIGNING_PRIVATE_KEY(?:_PASSWORD)?$/i);
  assert.deepEqual(plan.args.slice(plan.args.indexOf('--')+1),['--locked']);
  assert.deepEqual(inherited,original,'must not modify caller environment');
 });
}
for (const [target,arch] of [['windows-x86_64','x64'],['windows-aarch64','arm64']]) {
 test(`${target}: generated installer neither downloads WebView2 nor requests machine-wide installation`, () => {
  const plan=buildPlan(target,{STATIC_VCRUNTIME:'false'},'win32',arch);
  assert.deepEqual(bundle(plan).windows.webviewInstallMode,{type:'skip'});
  assert.equal(bundle(plan).windows.nsis.installMode,'currentUser');
  assert.equal(plan.env.STATIC_VCRUNTIME,'true');
 });
}
test('direct Tauri builds inherit the same no-download/no-updater installation policy',async () => {
 const config=JSON.parse(await readFile(join(root,'src-tauri/tauri.conf.json'),'utf8'));
 assert.deepEqual(config.bundle.windows?.webviewInstallMode,{type:'skip'});
 assert.equal(config.bundle.windows.nsis.installMode,'currentUser');
 assert.equal(config.bundle.createUpdaterArtifacts,false);
});
test('the disabled manifest CLI cannot recreate latest.json, even with valid inputs',async () => {
 const scratch=await mkdtemp(join(tmpdir(),'pandacode-manifest-'));
 try {
  await writeFile(join(scratch,'PandaCode-Windows-x64-setup.exe.sig'),'offline-test-signature');
  await writeFile(join(scratch,'notes.md'),'Local-only regression fixture');
  const result=spawnSync(process.execPath,[join(root,'scripts/release/updater-manifest.mjs'),scratch,'0.3.28',join(scratch,'notes.md')],{encoding:'utf8',timeout:10000});
  assert.equal(result.error,undefined);
  assert.notEqual(result.status,0);
  assert.match(result.stderr,/disabled/i);
  await assert.rejects(access(join(scratch,'latest.json')),error=>error.code==='ENOENT');
 } finally {await rm(scratch,{recursive:true,force:true});}
});
test('plain offline npm install honors repository policy and cannot execute a lifecycle payload',async () => {
 const scratch=await mkdtemp(join(tmpdir(),'pandacode-lifecycle-'));
 try {
  const config=await readFile(join(root,'.npmrc'),'utf8').catch(error=>{if(error.code==='ENOENT')return '';throw error;});
  await writeFile(join(scratch,'.npmrc'),config);
  await writeFile(join(scratch,'package.json'),JSON.stringify({name:'local-install-policy-test',version:'1.0.0',private:true,scripts:{postinstall:'node postinstall.cjs'}}));
  await writeFile(join(scratch,'postinstall.cjs'),"require('node:fs').writeFileSync('lifecycle-ran','controlled regression sentinel');\n");
  // Use an installed npm only; support npm-run, Windows Node and Unix Node layouts.
  const candidates=[process.env.npm_execpath,
   join(dirname(process.execPath),'node_modules/npm/bin/npm-cli.js'),
   resolve(dirname(process.execPath),'../lib/node_modules/npm/bin/npm-cli.js')];
  let npm;
  for (const candidate of candidates) {
   if (!candidate || !isAbsolute(candidate) || !candidate.endsWith('npm-cli.js')) continue;
   if (await stat(candidate).then(info=>info.isFile(),()=>false)) { npm=candidate; break; }
  }
  assert.ok(npm,'Installed npm CLI not found; use Node.js 22+ with npm and run npm run test:install-security.');
  // Do not let npm-run's inherited config make a missing repository policy pass.
  const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>!/^npm_config_/i.test(key)));
  const result=spawnSync(process.execPath,[npm,'install','--offline','--no-audit','--no-fund'],{cwd:scratch,encoding:'utf8',timeout:20000,env:{...env,npm_config_cache:join(scratch,'cache'),npm_config_userconfig:join(scratch,'empty-user-config'),npm_config_globalconfig:join(scratch,'empty-global-config')}});
  assert.equal(result.error,undefined);
  assert.equal(result.status,0,result.stderr);
  await assert.rejects(access(join(scratch,'lifecycle-ran')),error=>error.code==='ENOENT');
 } finally {await rm(scratch,{recursive:true,force:true});}
});
