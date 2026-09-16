// Actual Chromium worker + responsive canvas smoke check. No npm dependencies.
// node tools/check_portable_browser.mjs DIST FIXTURE OUT
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, readdir, stat} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import path from 'node:path';
const [dist, fixture, output] = process.argv.slice(2).map(x => path.resolve(x));
const started = Date.now();
await mkdir(output, {recursive:true});
const server = createServer(async (req,res) => {
  try {
    const relative = decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/^\//,'') || 'index.html';
    const file = path.resolve(dist,relative);
    if (!file.startsWith(dist+path.sep)) throw Error('Invalid path');
    const bytes=await readFile(file);
    res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':file.endsWith('.html')?'text/html':'application/octet-stream');
    res.end(bytes);
  } catch { res.writeHead(404);res.end('Missing'); }
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url=`http://127.0.0.1:${server.address().port}/`;
const browser=spawn('/usr/bin/chromium',['--headless','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader',`--user-data-dir=${output}/chromium-profile`,'--remote-debugging-port=0','about:blank'],{stdio:['ignore','ignore','pipe']});
let log=''; browser.stderr.on('data',b=>log+=b);
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const errors=[];
let socket;
try {
  let endpoint;
  for(let i=0;i<200;i++){endpoint=log.match(/DevTools listening on (ws:\/\/[^\s]+)/)?.[1];if(endpoint)break;await pause(100);}
  if(!endpoint)throw Error(log);
  const port=new URL(endpoint).port;
  const targets=await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  socket=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise((ok,bad)=>{socket.onopen=ok;socket.onerror=bad;});
  let sequence=0;const pending=new Map();
  socket.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}else if(m.method==='Runtime.exceptionThrown'){errors.push(m.params);}};
  const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));setTimeout(()=>{if(pending.delete(id))reject(Error(`Timed out: ${method}`));},120000).unref();});
  const evaluate=async expression=>{const r=await cdp('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
  const click=async(x,y)=>{await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',clickCount:1});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',clickCount:1});await pause(600);};
  const shot=async name=>{const r=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(path.join(output,name),Buffer.from(r.data,'base64'));};
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1100,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.navigate',{url});await pause(10000);
  await shot('guided.png');await click(205,30);await shot('workshop-empty.png');
  await click(52,111);await pause(10000);await shot('workshop-preview.png');
  const file=JSON.parse(await readFile(fixture,'utf8'));
  const design=file.design??file;
  // Verify the independent worker protocol, including a graph-backed CAD design.
  const result=await evaluate(`new Promise((resolve,reject)=>{
    const w=new Worker('workshop-worker_loader.js');
    const job=${JSON.stringify({id:1,key:123,design,stage:'Nominal',action:'Inspect'})};
    const result={}; let phase=0;
    const timeout=setTimeout(()=>{w.terminate();reject(Error('Worker timeout'));},90000);
    let ticks=0;const heart=setInterval(()=>ticks++,10);
    const fail=message=>{clearTimeout(timeout);clearInterval(heart);w.terminate();reject(Error(message));};
    w.onerror=e=>fail(e.message);
    w.onmessage=e=>{
      if(e.data==='ready'){result.ready=true;w.postMessage(JSON.stringify(job));return;}
      if(typeof e.data!=='string'){
        const bytes=new Uint8Array(e.data.bytes);
        if(phase!==2 || bytes[0]!==80 || bytes[1]!==75)return fail('Diagnostic ZIP buffer missing');
        result.zipBytes=bytes.length;result.binaryKey=e.data.key;result.ticks=ticks;
        clearTimeout(timeout);clearInterval(heart);w.terminate();resolve(result);return;
      }
      const done=JSON.parse(e.data);
      if(phase===0){
        if(done.result.Err)return fail(done.result.Err);
        result.faces=done.result.Ok.View.shape.faces.length;result.stage=done.result.Ok.View.stage;
        phase=1;job.id++;job.action={Pattern:{diagnostic:false}};w.postMessage(JSON.stringify(job));
      }else if(phase===1){
        if(!done.result.Err?.includes('obstructed'))return fail('Blocked production pattern was not rejected');
        result.productionRefused=true;phase=2;job.id++;job.action={Pattern:{diagnostic:true}};w.postMessage(JSON.stringify(job));
      }else fail('Expected a transferred binary artifact');
    };
  })`);
  if(!result.faces || !result.ready || !result.productionRefused || result.ticks<1 || result.binaryKey!=='123')throw Error(JSON.stringify(result));
  await cdp('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:path.join(output,'downloads'),eventsEnabled:true});
  await click(712,172); await shot('workshop-files.png');
  await click(645,324); await pause(4000); await shot('workshop-download.png');
  const downloaded = [];
  for (const name of await readdir(path.join(output,'downloads'))) {
    const file = path.join(output,'downloads',name);
    if (name.endsWith('.zip') && (await stat(file)).mtimeMs >= started) {
      const data=await readFile(file);
      if (data[0]!==80 || data[1]!==75) throw Error('Download is not a ZIP');
      downloaded.push({name,bytes:data.length});
    }
  }
  if(!downloaded.length) throw Error('Workshop did not deliver a new ZIP download');
  await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});await pause(1200);await shot('workshop-phone.png');
  await cdp('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:1});
  await cdp('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:107,y:630}]});
  await cdp('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});await pause(800);await shot('workshop-phone-cad.png');
  if(errors.length)throw Error(JSON.stringify(errors));
  await writeFile(path.join(output,'results.json'),JSON.stringify({worker:result,downloaded,errors},null,2));
  console.log(JSON.stringify({worker:result,output}));
} finally {
  socket?.close();browser.kill('SIGTERM');server.close();
  await writeFile(path.join(output,'chromium.log'),log);
}
