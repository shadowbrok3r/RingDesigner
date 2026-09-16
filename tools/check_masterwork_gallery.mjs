// Exercise every gallery view in isolated Chromium, including phone layout.
// node tools/check_masterwork_gallery.mjs COLLECTION OUTPUT
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, access} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import path from 'node:path';
const [root, output] = process.argv.slice(2).map(x=>path.resolve(x));
await mkdir(output,{recursive:true});
const server=createServer(async(req,res)=>{
  try {
    const rel=decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/^\//,'')||'index.html';
    const file=path.resolve(root,rel);
    if(!file.startsWith(root+path.sep))throw Error('Invalid path');
    res.setHeader('Content-Type',file.endsWith('.html')?'text/html':file.endsWith('.png')?'image/png':file.endsWith('.gif')?'image/gif':file.endsWith('.svg')?'image/svg+xml':'application/octet-stream');
    res.end(await readFile(file));
  }catch{res.writeHead(404);res.end('Missing');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const browser=spawn('/usr/bin/chromium',['--headless','--no-sandbox','--disable-dev-shm-usage',`--user-data-dir=${output}/profile`,'--remote-debugging-port=0','about:blank'],{stdio:['ignore','ignore','pipe']});
let log='', socket;browser.stderr.on('data',x=>log+=x);
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const errors=[];
try {
  let endpoint;
  for(let i=0;i<200;i++){endpoint=log.match(/DevTools listening on (ws:\/\/[^\s]+)/)?.[1];if(endpoint)break;await pause(100);}
  if(!endpoint)throw Error(log);
  const targets=await(await fetch(`http://127.0.0.1:${new URL(endpoint).port}/json`)).json();
  socket=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise((ok,bad)=>{socket.onopen=ok;socket.onerror=bad;});
  let serial=0;const pending=new Map();
  socket.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params);};
  const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));setTimeout(()=>{if(pending.delete(id))reject(Error(method+' timed out'));},30000).unref();});
  const evaluate=async expression=>{const r=await cdp('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
  const shot=async name=>{const r=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(path.join(output,name),Buffer.from(r.data,'base64'));};
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1100,deviceScaleFactor:1,mobile:false});
  await cdp('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
  await pause(1000);
  await evaluate('Promise.all([...document.querySelectorAll("img.ring")].map(i=>i.decode()))');
  await shot('desktop.png');
  const views=[];
  for(const ring of ['solstice','nocturne']){
    for(const view of ['hero','seal','cheek','palm','reverse','structure','pattern','turntable']){
      const value=await evaluate(`(async()=>{const a=document.querySelector('#${ring}'),b=a.querySelector('[data-view="${view}"]');b.click();const img=a.querySelector('.ring');await img.decode();return {ring:'${ring}',view:'${view}',width:img.naturalWidth,pressed:b.getAttribute('aria-pressed'),selected:a.querySelectorAll('[aria-pressed="true"]').length};})()`);
      if(!value.width || value.pressed!=='true' || value.selected!==1)throw Error(JSON.stringify(value));
      views.push(value);
    }
    await evaluate(`document.querySelector('#${ring} [data-view="seal"]').click();document.querySelector('#${ring}').scrollIntoView()`);
    await pause(300);await shot(ring+'-seal.png');
  }
  const links=await evaluate('[...document.querySelectorAll("a[href]")].map(a=>a.getAttribute("href"))');
  for(const href of links)await access(path.join(root,decodeURIComponent(href)));
  await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});
  await evaluate('document.querySelectorAll("[data-view=hero]").forEach(b=>b.click());window.scrollTo(0,0)');
  await pause(300);await shot('phone.png');
  const layout=await evaluate('({viewport:innerWidth,document:document.documentElement.scrollWidth,smallestButton:Math.min(...[...document.querySelectorAll("button")].map(b=>b.getBoundingClientRect().height))})');
  if(layout.document>layout.viewport || layout.smallestButton<44)throw Error(JSON.stringify(layout));
  if(errors.length)throw Error(JSON.stringify(errors));
  await writeFile(path.join(output,'results.json'),JSON.stringify({views,local_links:links.length,layout,errors},null,2));
  console.log('PASS: 16 views, local downloads, 390 px layout, no browser errors');
}finally{socket?.close();browser.kill();server.close();}
