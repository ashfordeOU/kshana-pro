import { chromium } from 'playwright';
import { spawn } from 'child_process';
const port = 8795;
const srv = spawn('/opt/homebrew/bin/python3', ['-m','http.server',String(port)], {cwd:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo', stdio:'ignore'});
await new Promise(r=>setTimeout(r,1200));
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const pages = (process.argv[2]||'capabilities,evidence,developers,editions,missions,index').split(',');
for (const pg of pages){ for (const [w,h] of [[1440,900],[390,844]]){
  const p = await (await b.newContext({ viewport:{width:w,height:h}, colorScheme:'dark' })).newPage();
  await p.goto(`http://localhost:${port}/${pg}.html`); await p.waitForTimeout(2500);
  const r = await p.evaluate(()=>({H:document.documentElement.scrollHeight, hs: document.documentElement.scrollWidth>innerWidth, secs:[...document.querySelectorAll('main > section, body > section, section[id], section.page-hero')].filter(s=>!s.parentElement.closest('section')).map(s=>(s.id||s.className.split(' ')[0])+':'+Math.round(s.getBoundingClientRect().height))}));
  console.log(pg, w, r.H, 'hscroll', r.hs, w===1440? r.secs.join(' '):'');
  await p.close(); }}
await b.close(); srv.kill();
