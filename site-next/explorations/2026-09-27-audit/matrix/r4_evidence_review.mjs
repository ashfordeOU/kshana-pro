// R4 Evidence critical review: full shots at 4 sizes x 2 themes, bento viewport shots, motion diff, metrics.
// usage: node r4_evidence_review.mjs OUTDIR [url-path] [sizes]
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2]; const path = process.argv[3]||'2026-09-27-combo/evidence.html';
const sizes = (process.argv[4]||'1440x900,1920x1080,768x1024,390x844').split(',').map(s=>s.split('x').map(Number));
const base = 'http://localhost:'+(process.env.PORT||8771)+'/';
const tag = path.split('/').slice(-2).join('_').replace('.html','');
fs.mkdirSync(OUT,{recursive:true});
const browser = await chromium.launch();
for (const [w,h] of sizes) for (const theme of ['dark','light']) {
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme });
  await ctx.addInitScript(t=>{ try{localStorage.setItem('theme',t);}catch(e){} }, theme);
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error'||m.type()==='warning') errs.push(m.type()+': '+m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(base+path, { waitUntil:'load' });
  await page.evaluate(t=>document.documentElement.setAttribute('data-theme',t), theme);
  await page.waitForTimeout(400);
  // motion diff: hero area 2s apart before scrolling
  const a = await page.screenshot(); await page.waitForTimeout(2000); const b = await page.screenshot();
  let diff=0; const n=Math.min(a.length,b.length); for(let i=0;i<n;i+=97) if(a[i]!==b[i]) diff++;
  const H = await page.evaluate(()=>document.documentElement.scrollHeight);
  for (let y=0;y<H;y+=500){ await page.evaluate(y=>scrollTo(0,y),y); await page.waitForTimeout(150); }
  let bentoMotion=null;
  if (await page.$('#evBento')) {
    await page.evaluate(()=>{document.querySelectorAll('.ev-tile').forEach(t=>t.classList.remove('in'));});
    await page.evaluate(()=>document.querySelector('#results').scrollIntoView());
    await page.waitForTimeout(50);
    await page.evaluate(()=>{document.querySelectorAll('.ev-tile').forEach(t=>t.classList.add('in'));});
    const c = await page.screenshot(); await page.waitForTimeout(2000); const d = await page.screenshot();
    let dd=0; const m=Math.min(c.length,d.length); for(let i=0;i<m;i+=97) if(c[i]!==d[i]) dd++;
    bentoMotion=dd;
    await page.waitForTimeout(3000); const e1 = await page.screenshot(); await page.waitForTimeout(2000); const e2 = await page.screenshot();
    let sd=0; for(let i=0;i<Math.min(e1.length,e2.length);i+=97) if(e1[i]!==e2[i]) sd++; bentoMotion=[dd,sd];
    const spare = await page.evaluate(()=>[...document.querySelectorAll('.ev-vis')].map(v=>{const p=v.previousElementSibling; return Math.round(v.getBoundingClientRect().top-p.getBoundingClientRect().bottom);}));
    bentoMotion.push(spare.join(','));
    await page.screenshot({ path:`${OUT}/${tag}-bento-${w}x${h}-${theme}.png` });
  }
  await page.evaluate(()=>scrollTo(0,0)); await page.waitForTimeout(1200);
  await page.screenshot({ path:`${OUT}/${tag}-${w}x${h}-${theme}.png`, fullPage:true });
  const m = await page.evaluate(()=>{ const r={}; for (const s of document.querySelectorAll('main section, body > section')) { const b=s.getBoundingClientRect(); r[s.id||s.className]=Math.round(b.height); }
    const wide=[...document.querySelectorAll('body *')].filter(e=>{const b=e.getBoundingClientRect(); return b.right>innerWidth+1 && b.width>0 && getComputedStyle(e).position!=='fixed';}).slice(0,5).map(e=>e.tagName+'.'+e.className);
    return {sw:document.documentElement.scrollWidth,cw:document.documentElement.clientWidth,H:document.documentElement.scrollHeight,sections:r,wide}; });
  console.log(JSON.stringify({tag,w,h,theme,heroDiff:diff,bentoMotion,errs:errs.slice(0,6),...m}));
  await ctx.close();
}
await browser.close();
