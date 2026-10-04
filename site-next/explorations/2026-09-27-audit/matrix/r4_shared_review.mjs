// R4 shared-shell review: every page's nav, search, theme (3 states), footer, links, Studio link,
// the shared kshana-mcp player on Home + Developers, nav fit at text-only 200 %. usage: node r4_shared_review.mjs OUTDIR
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2]; fs.mkdirSync(OUT, { recursive: true });
const port = process.env.PORT || 8797, base = `http://localhost:${port}/`;
const pages = ['index','missions','capabilities','evidence','developers','editions','docs/index'];
const browser = await chromium.launch();
const res = {};
for (const p of pages) for (const [w,h] of [[1440,900],[390,844]]) {
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme:'dark' });
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error') errs.push(m.text().slice(0,160)); }); page.on('pageerror', e=>errs.push(String(e).slice(0,160)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(base+p+'.html', { waitUntil:'load' }); await page.waitForTimeout(700);
  const r = {};
  // theme: system (no stamp) -> toggle -> toggle
  r.theme0 = await page.evaluate(()=>document.documentElement.getAttribute('data-theme'));
  await page.click('#themeBtn'); r.theme1 = await page.evaluate(()=>document.documentElement.getAttribute('data-theme'));
  await page.click('#themeBtn'); r.theme2 = await page.evaluate(()=>document.documentElement.getAttribute('data-theme'));
  const bg = async()=>page.evaluate(()=>getComputedStyle(document.body).backgroundColor);
  r.bg2 = await bg(); await page.click('#themeBtn'); r.bg3 = await bg();
  // search palette
  await page.click('#kbtn'); await page.waitForTimeout(250);
  r.palette = await page.evaluate(()=>{ const d=document.getElementById('palette'); return !!d && (d.open || getComputedStyle(d).display!=='none'); });
  await page.keyboard.type('holdover'); await page.waitForTimeout(250);
  r.hits = await page.evaluate(()=>document.querySelectorAll('#palette [role=option], #palette li a, #palette a').length);
  await page.keyboard.press('Escape'); await page.waitForTimeout(150);
  r.paletteClosed = await page.evaluate(()=>{ const d=document.getElementById('palette'); return !d || !(d.open || getComputedStyle(d).display!=='none' && !d.hidden); });
  // menu at phone
  if (w < 1180) { await page.click('#menuBtn'); r.menuOpen = await page.evaluate(()=>getComputedStyle(document.getElementById('navLinks')).display!=='none'); await page.click('#menuBtn'); }
  const m = await page.evaluate(()=>{
    const links=[...document.querySelectorAll('#navLinks a')].map(a=>a.getAttribute('href'));
    const cur=document.querySelector('#navLinks a[aria-current="page"]'); const st=document.querySelector('.nav .pill');
    const foot=document.querySelectorAll('footer a').length;
    const nr=document.querySelector('.nav-right').getBoundingClientRect().right;
    return { links, current: cur&&cur.textContent.trim(), studio: st&&st.getAttribute('href'), foot, navRightOut: Math.round(nr-document.documentElement.clientWidth),
      hscroll: document.documentElement.scrollWidth>document.documentElement.clientWidth, compact: document.getElementById('nav').className };
  });
  Object.assign(r, m);
  // player
  if (p==='index'||p==='developers') {
    await page.evaluate(()=>{ const s=document.getElementById('mcpStage'); s.scrollIntoView({block:'center'}); });
    await page.waitForTimeout(900);
    r.player = await page.evaluate(()=>{ const t=document.querySelectorAll('#mcpSteps [role=tab]'); const ps=[...document.querySelectorAll('.mp-panel')]; const st=document.getElementById('mcpStage');
      return { tabs:t.length, panels:ps.length, stageH:Math.round(st.getBoundingClientRect().height), overflow: ps.map(p=>{const was=p.hidden; p.hidden=false; const o=p.scrollHeight-p.clientHeight; p.hidden=was; return o;}), lede: (document.getElementById('mcpLede')||{}).textContent, axis: !!document.querySelector('.mp-ax'), headDisplay:getComputedStyle(document.querySelector('.mp-head')).display }; });
    await page.click('#mcpNext'); r.afterNext = await page.evaluate(()=>document.querySelector('#mcpSteps [aria-selected=true]').id);
    await page.locator('#mcpStage').screenshot({ path:`${OUT}/player-${p.replace('/','_')}-${w}.png` });
  }
  r.errs = errs;
  res[`${p}@${w}`] = r; await ctx.close();
}
// text-only 200 % at 1280x800 (matrix method: double every computed font size) + on-dark nav in light theme on Editions
for (const p of pages) {
  const ctx = await browser.newContext({ viewport:{width:1280,height:800} }); const page = await ctx.newPage();
  await page.goto(base+p+'.html', { waitUntil:'load' }); await page.waitForTimeout(500);
  await page.evaluate(()=>{ const els=[...document.querySelectorAll('body *')]; const fs=els.map(e=>parseFloat(getComputedStyle(e).fontSize)); els.forEach((e,i)=>{ if(!e.closest('svg')) e.style.setProperty('font-size',(fs[i]*2)+'px','important'); }); });
  await page.waitForTimeout(500);
  res[`${p}@text200`] = await page.evaluate(()=>({ navRightOut: Math.round(document.querySelector('.nav-right').getBoundingClientRect().right-document.documentElement.clientWidth), cls: document.getElementById('nav').className }));
  await page.screenshot({ path:`${OUT}/text200-${p.replace('/','_')}.png`, clip:{x:0,y:0,width:1280,height:120} });
  await ctx.close();
}
{
  const ctx = await browser.newContext({ viewport:{width:1440,height:900}, colorScheme:'light' }); const page = await ctx.newPage();
  await page.goto(base+'editions.html', { waitUntil:'load' }); await page.waitForTimeout(400);
  await page.evaluate(()=>{ const d=document.querySelector('[data-nav-dark]'); if(d) scrollTo(0, d.getBoundingClientRect().top+scrollY-10); }); await page.waitForTimeout(600);
  res['editions@ondark-light'] = await page.evaluate(()=>{ const a=document.querySelector('#navLinks a[aria-current="page"]'), k=document.querySelector('#kbtn kbd'); const c=e=>{const s=getComputedStyle(e);return s.color+' / '+s.backgroundColor}; return { nav: document.getElementById('nav').className, cur: a&&c(a), kbd: k&&c(k) }; });
  await page.screenshot({ path:`${OUT}/editions-ondark-light.png`, clip:{x:0,y:0,width:1440,height:90} });
  await ctx.close();
}
await browser.close();
fs.writeFileSync(`${OUT}/result.json`, JSON.stringify(res,null,1));
console.log(JSON.stringify(res,null,0));
