import { chromium } from 'playwright';
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const p = await (await b.newContext({ viewport:{width:1440,height:900}, colorScheme:'dark' })).newPage();
const errs=[]; p.on('console',m=>{if(m.type()==='error')errs.push(m.text())}); p.on('pageerror',e=>errs.push(String(e)));
await p.goto('http://localhost:8731/index.html'); await p.waitForTimeout(15000);
for (let k=0;k<9;k++){ await p.screenshot({path:`../shots/observatory-review/sample-${k}.png`, clip:{x:0,y:60,width:1440,height:840}});
  const info = await p.evaluate(()=>({clk:document.querySelector('#hClock').textContent, ph:document.querySelector('#hPhase').textContent, vals:[...document.querySelectorAll('.hero-panels [data-v]')].map(e=>e.textContent), st:[...document.querySelectorAll('.hero-panels [data-state]')].map(e=>e.textContent), flip:[...document.querySelectorAll('.glabel.on')].map(l=>l.className+' '+Math.round(l.getBoundingClientRect().right)), panelsL:Math.round(document.querySelector('.hero-panels').getBoundingClientRect().left), lastLog:[...document.querySelectorAll('#hLog .log-line')].slice(-2).map(e=>e.textContent), logBottomVisible: (()=>{const L=document.querySelector('#hLog');const r=L.getBoundingClientRect();const ls=[...L.querySelectorAll('.log-line:not(.caret-line)')];const last=ls.at(-1);return last? last.getBoundingClientRect().bottom<=r.bottom+1:null})() }));
  console.log(k, JSON.stringify(info)); await p.waitForTimeout(4700); }
console.log('errors', errs); await b.close();
