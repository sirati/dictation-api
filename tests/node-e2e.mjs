import { fileURLToPath } from "node:url";
import { Client, Provider } from '../bindings/node/dist/src-js/index.js';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import assert from 'node:assert/strict';
const home = await mkdtemp(join(tmpdir(), 'dictation-node-'));
const executable = process.platform === 'win32' ? '../target/debug/dictation-broker.exe' : '../target/debug/dictation-broker';
const child = spawn(fileURLToPath(new URL(executable, import.meta.url)), ['run'], {env:{...process.env,DICTATION_API_HOME:home,DICTATION_API_NO_NATIVE_FOCUS:"1",DICTATION_API_NO_DBUS:"1"},stdio:'inherit'});
const connections = [];
try {
  const endpoint=join(home,'endpoint.json');
  for(let tries=0;;tries++){try{await readFile(endpoint);break;}catch(e){if(tries>100)throw e;await new Promise(r=>setTimeout(r,20));}}
  const a=await Client.connect({id:'node-a',name:'Node A',icon:null,process_id:process.pid},endpoint);connections.push(a);
  const b=await Client.connect({id:'node-b',name:'Node B',icon:null,process_id:process.pid},endpoint);connections.push(b);
  const p=await Provider.connect('Node dictation provider',endpoint);connections.push(p);
  const old=await a.activate();await b.activate();await a.deactivate(old);
  const session=await p.begin({level:true,spectrum:true});
  assert.equal((await b.nextEvent()).kind,'offer');await a.activate();
  await b.respond(session,true,{level:true,spectrum:false});
  const ready=await p.nextEvent();assert.equal(ready.application.id,'node-b');assert.deepEqual(ready.audio,{level:true,spectrum:false});
  await p.audio(session,{level:0.8,spectrum:null});assert.equal((await b.nextEvent()).frame.level,0.8);
  await p.complete(session,'node native binding',true);
  assert.deepEqual((await b.nextEvent()).transcript,{text:'node native binding',submit:true});
  await b.acknowledge(session,true);assert.equal((await p.nextEvent()).applied,true);
  assert.equal(await a.nextEvent(20),null);
  console.log('Node/Electron Node-API socket E2E passed');
} finally {
  for(const c of connections)c.close();
  child.kill();await new Promise(resolve=>child.once('exit',resolve));await rm(home,{recursive:true});
}
