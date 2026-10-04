import { chromium } from 'playwright';
import { spawn } from 'child_process';
const port = 8796;
const srv = spawn('/opt/homebrew/bin/python3', ['-m','http.server',String(port)], {cwd:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo', stdio:'ignore'});
await new Promise(r=>setTimeout(r,1200));
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const pages = (process.argv[2]||'capabilities,evidence,developers,editions').split(',');
const tag = process.argv[3]||'';
const out = '/Users/chak/Code/kshana-site-next/explorations/2026-09-27-audit/shots/r5/';
for (const pg of pages){ for (const [w,h,cs] of [[1440,900,'dark'],[1440,900,'light'],[390,844,'dark']]){
  const ctx = await b.newContext({ viewport:{width:w,height:h}, colorScheme:cs });
  const p = await ctx.newPage(); const errs=[];
  p.on('console',m=>{if(m.type()==='error')errs.push(m.text())}); p.on('pageerror',e=>errs.push(String(e)));
  await p.goto(`http://localhost:${port}/${pg}.html`); await p.waitForTimeout(4000);
  const f1 = `${out}${pg}${tag}-${w}x${h}-${cs}.png`;
  await p.screenshot({path:f1, timeout:300000});
  if (w===1440 && cs==='dark'){ await p.waitForTimeout(2000); const a=await p.screenshot({clip:{x:720,y:60,width:720,height:800}, timeout:300000}); await p.waitForTimeout(2000); const c=await p.screenshot({clip:{x:720,y:60,width:720,height:800}, timeout:300000});
    console.log(pg,'live(right half differs 2s apart):', !a.equals(c)); }
  console.log(pg,w,cs,'errors',errs.length, errs.slice(0,3).join(' | '));
  await ctx.close(); }}
await b.close(); srv.kill();
