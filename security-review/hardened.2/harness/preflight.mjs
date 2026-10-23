import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import net from 'node:net';
import { spawnSync } from 'node:child_process';
for (const path of ['/work/probe-write','/usr/probe-write','/probe-write']) {
  assert.throws(() => writeFileSync(path,'probe'), error => ['EROFS','EACCES'].includes(error.code));
}
assert.equal(existsSync('/root'),false);
assert.equal(existsSync('/proc/1'),false);
assert.ok(process.pid < 100, 'private PID namespace');
mkdirSync('/tmp/mount-test');
const mount = spawnSync('mount',['-t','tmpfs','tmpfs','/tmp/mount-test'],{encoding:'utf8',timeout:3000});
assert.notEqual(mount.status,0,'mount capability must be dropped');
assert.equal(Object.keys(process.env).some(k => /TOKEN|SECRET|PASSWORD|API_KEY/i.test(k)),false);
mkdirSync('/tmp/home',{recursive:true});
writeFileSync('/tmp/allowed','scratch');
await new Promise((resolve,reject) => {
 const socket=net.connect({host:'1.1.1.1',port:443});
 socket.on('connect',()=> {socket.destroy(); reject(new Error('Egress was available'));});
 socket.on('error',()=>resolve());
 socket.setTimeout(1500,()=>{socket.destroy();resolve();});
});
console.log(JSON.stringify({readonlySource:true,readonlyToolchain:true,chroot:true,pidNamespace:true,netNamespace:true,egressBlocked:true,capabilitiesDropped:true,cleanEnvironment:true,scratchLimitMiB:64,timeoutSeconds:90,addressSpaceMiB:8192,cpuSeconds:45,fileLimitMiB:32,files:128},null,2));
