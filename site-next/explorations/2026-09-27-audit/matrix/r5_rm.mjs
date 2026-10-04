import { chromium } from 'playwright';
import { spawn } from 'child_process';
const srv = spawn('/opt/homebrew/bin/python3', ['-m','http.server','8799'], {cwd:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo', stdio:'ignore'});
await new Promise(r=>setTimeout(r,1000));
const b = await chromium.launch();
for (const [pg,sel] of [['capabilities','#chWin'],['evidence','#ehWin'],['developers','#dtWin'],['editions','#eoWin']]) {
  const ctx = await b.newContext({ viewport:{width:1440,height:900}, colorScheme:'light', reducedMotion:'reduce' }); ctx.setDefaultTimeout(300000);
  const p = await ctx.newPage(); const errs=[]; p.on('pageerror',e=>errs.push(String(e))); p.on('console',m=>{if(m.type()==='error')errs.push(m.text())});
  await p.goto(`http://localhost:8799/${pg}.html`); await p.waitForTimeout(3500);
  const a = await p.locator(sel).screenshot({path:`/Users/chak/Code/kshana-site-next/explorations/2026-09-27-audit/shots/r5/${pg}-reduced-motion-light.png`});
  await p.waitForTimeout(2000); const c = await p.locator(sel).screenshot();
  const info = await p.evaluate(()=>({term:(document.querySelector('#dtCode')||{}).textContent?.length, cells:document.querySelectorAll('.eh-c').length, svg:(document.querySelector('#eoSvg')||{}).childElementCount, counts:[...document.querySelectorAll('[data-to]')].map(e=>e.textContent).join('/')}));
  console.log(pg, 'static under reduced motion:', a.equals(c), JSON.stringify(info), 'errors', errs);
  await ctx.close();
}
await b.close(); srv.kill();
