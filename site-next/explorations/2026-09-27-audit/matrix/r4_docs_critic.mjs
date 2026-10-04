// R4 docs reviewer: shots + metrics. usage: PORT=.. node r4_docs_review.mjs OUTDIR pages sizes
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2]; const pages = (process.argv[3]||'docs/index').split(',');
const sizes = (process.argv[4]||'1440x900,1920x1080,768x1024,390x844').split(',').map(s=>s.split('x').map(Number));
const port = process.env.PORT||8793; const full = process.env.FULL==='1';
fs.mkdirSync(OUT,{recursive:true});
const browser = await chromium.launch();
for (const p0 of pages) for (const [w,h] of sizes) for (const theme of ['dark','light']) {
  const [p,hash] = p0.split('#');
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme });
  await ctx.addInitScript(t=>{ try{localStorage.setItem('theme',t);}catch(e){} }, theme);
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error') errs.push(m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(`http://127.0.0.1:${port}/${p}.html${hash?'#'+hash:''}`, { waitUntil:'load' });
  await page.waitForTimeout(900);
  const f = `${OUT}/${p.replace(/\//g,'_')}${hash?'_'+hash:''}-${w}x${h}-${theme}.png`;
  await page.screenshot({ path:f, fullPage: full || p.endsWith('index') });
  const m = await page.evaluate(()=>{
    const de=document.documentElement; const out={sw:de.scrollWidth,cw:de.clientWidth,theme:de.dataset.theme};
    const para=[...document.querySelectorAll('.prose>p')].filter(x=>x.textContent.length>200)[0];
    if(para){ const r=para.getBoundingClientRect(); const cs=getComputedStyle(para); const c=document.createElement('canvas').getContext('2d'); c.font=`${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`; const avg=c.measureText('the quick brown fox jumps over the lazy dog, ').width/45; out.cpl=Math.round(r.width/avg); out.pw=Math.round(r.width); out.fs=cs.fontSize; out.fam=cs.fontFamily.slice(0,40);}
    const wide=[...document.querySelectorAll('body *')].filter(e=>{const r=e.getBoundingClientRect(); return r.right>de.clientWidth+1 && r.width>0 && !e.closest('pre,.tbl,[aria-hidden=true]')}).slice(0,5).map(e=>e.tagName+'.'+e.className+' '+Math.round(e.getBoundingClientRect().right));
    out.wide=wide; return out; });
  console.log(JSON.stringify({f:f.split('/').pop(), errs: errs.slice(0,4), ...m}));
  await ctx.close();
}
await browser.close();
