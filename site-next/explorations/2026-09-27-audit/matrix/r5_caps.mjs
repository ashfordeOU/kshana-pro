import { chromium } from 'playwright';
import { spawn } from 'child_process';
const srv = spawn('/opt/homebrew/bin/python3', ['-m','http.server','8798'], {cwd:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo', stdio:'ignore'});
await new Promise(r=>setTimeout(r,1000));
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const ctx = await b.newContext({ viewport:{width:1440,height:900}, colorScheme:'dark' }); ctx.setDefaultTimeout(300000);
const p = await ctx.newPage(); const errs=[];
p.on('console',m=>{if(m.type()==='error')errs.push(m.text())}); p.on('pageerror',e=>errs.push(String(e)));
await p.goto('http://localhost:8798/capabilities.html'); await p.waitForTimeout(3000);
console.log('default H', await p.evaluate(()=>document.documentElement.scrollHeight));
for (const t of await p.$$eval('.cc-tabs [role=tab]', ts=>ts.map(t=>t.id))) {
  await p.click('#'+t); await p.waitForTimeout(700);
  console.log(t, await p.evaluate((id)=>{const pn=document.getElementById(document.getElementById(id).getAttribute('aria-controls'));return 'panel '+Math.round(pn.getBoundingClientRect().height)+' pageH '+document.documentElement.scrollHeight},t));
}
await p.close();
for (const a of ['atlas','bands','clocks','explorer','journey','runs','timing','fails','integrity','constellations','campaigns','customise','studio','standards','holdover','waterfall']) {
  const q = await ctx.newPage(); await q.goto('http://localhost:8798/capabilities.html#'+a); await q.waitForTimeout(1800);
  console.log('#'+a, await q.evaluate((a)=>{const e=document.getElementById(a);const r=e.getBoundingClientRect();return 'visible '+(r.height>0)+' top '+Math.round(r.top)+' panelHidden '+!!(e.closest('.cc-panel')&&e.closest('.cc-panel').hidden)},a));
  await q.close();
}
const q = await ctx.newPage(); await q.goto('http://localhost:8798/capabilities.html'); await q.waitForTimeout(3000);
await q.screenshot({path:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-audit/shots/r5/capabilities-full-1440-dark.jpg', fullPage:true, quality:60, type:'jpeg'});
console.log('errors', errs);
await b.close(); srv.kill();
