import {readdirSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
const groups=process.argv[2]==='release'?['scripts/release','packages/npx']:['tests'];
const excluded=process.argv[2]==='available'?['bundle.test.mjs','file-icons.test.mjs']:[];
if(excluded.length)console.log('NOT RUN: '+excluded.join(', ')+' (dist and material-icon-theme unavailable; tests not modified).');
const files=groups.flatMap(d=>readdirSync(`/work/${d}`).filter(f=>f.endsWith('.test.mjs')&&!excluded.includes(f)).sort().map(f=>`/work/${d}/${f}`));
const result=spawnSync(process.execPath,['--test',...files],{cwd:'/work',encoding:'utf8',timeout:75000,maxBuffer:8*1024*1024});
process.stdout.write(result.stdout||'');process.stderr.write(result.stderr||'');
if(result.error){console.error(result.error.message);process.exitCode=2;}else{process.exitCode=result.status??2;}
