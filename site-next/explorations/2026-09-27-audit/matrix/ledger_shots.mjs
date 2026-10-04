// Ledger + Reproduce-it section shots and heights (task: scrollable ledger, balanced repro).
// node ledger_shots.mjs <tag>   -> shots/ledger/<tag>-*.png, prints heights
import { chromium } from 'playwright';
import { spawn } from 'child_process';
const tag = process.argv[2] || 'x';
const OUT = '/Users/chak/Code/kshana-site-next/explorations/2026-09-27-audit/shots/ledger';
const port = 8797;
const srv = spawn('/opt/homebrew/bin/python3', ['-m','http.server',String(port)], {cwd:'/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo', stdio:'ignore'});
await new Promise(r=>setTimeout(r,1200));
async function shot(p, sel, path){ await p.addStyleTag({content:'#nav,.subnav{visibility:hidden!important}'}); const bb = await p.evaluate(s=>{const r=document.querySelector(s).getBoundingClientRect(); return {x:0,y:r.top+scrollY,width:innerWidth,height:r.height};}, sel); await p.screenshot({path, fullPage:true, clip:bb, animations:'disabled', timeout:300000}); }
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
for (const [w,h,theme] of [[1440,900,'dark'],[1440,900,'light'],[390,844,'dark']]) {
  const ctx = await b.newContext({ viewport:{width:w,height:h}, colorScheme:theme, reducedMotion:'reduce' });
  const p = await ctx.newPage();
  await p.goto(`http://localhost:${port}/evidence.html`); await p.waitForTimeout(2000);
  const m = await p.evaluate(()=>{ const q=s=>document.querySelector(s); const r=s=>{const e=q(s);return e?Math.round(e.getBoundingClientRect().height):null};
    const rf=q('.repro-free'), st=q('.rp-steps');
    return {page:document.documentElement.scrollHeight, hscroll:document.documentElement.scrollWidth>innerWidth, ledger:r('#ledger'), repro:r('#repro'),
      stampH: rf&&Math.round(rf.getBoundingClientRect().height), stepsH: st&&Math.round(st.getBoundingClientRect().height),
      stampBottom: rf&&Math.round(rf.getBoundingClientRect().bottom-q('.repro-grid').getBoundingClientRect().top), stepsBottom: st&&Math.round(st.getBoundingClientRect().bottom-q('.repro-grid').getBoundingClientRect().top)}; });
  console.log(tag, w, theme, JSON.stringify(m));
  const base = `${OUT}/${tag}-${w}-${theme}`;
  await shot(p,'#ledger',`${base}-ledger.png`);
  await shot(p,'#repro',`${base}-repro.png`);
  const sc = await p.$('.lg-scroll');
  if (sc) {
    await sc.evaluate(e => { e.scrollTop = (e.scrollHeight - e.clientHeight) / 2; });
    await p.waitForTimeout(200);
    await shot(p,'#ledger',`${base}-ledger-half.png`);
    // expand a visible row
    const s = await sc.$$('details.lg-row > summary');
    const vis = [];
    for (const x of s) { const ok = await x.evaluate((el)=>{const r=el.getBoundingClientRect(), c=el.closest('.lg-scroll').getBoundingClientRect(); return r.top>=c.top && r.bottom<=c.bottom;}); if (ok) vis.push(x); }
    const pick = vis[vis.length-1] || s[0];
    await pick.click(); await p.waitForTimeout(700);
    const fit = await pick.evaluate(el=>{const d=el.parentElement.getBoundingClientRect(), c=el.closest('.lg-scroll').getBoundingClientRect(); return {dH:Math.round(d.height), rH:Math.round(c.height), topAligned:Math.abs(d.top-c.top)<2, inView: d.top>=c.top-1 && d.bottom<=c.bottom+1, exp: el.getAttribute('aria-expanded') ?? String(el.parentElement.open)}});
    console.log(tag, w, theme, 'expanded', JSON.stringify(fit));
    await shot(p,'#ledger',`${base}-ledger-open.png`);
  }
  await ctx.close();
}
await b.close(); srv.kill();
