// R4 Missions critical review. usage: node r4_missions_review.mjs OUTDIR [sizes]
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2];
const sizes = (process.argv[3]||'1440x900,1920x1080,768x1024,390x844').split(',').map(s=>s.split('x').map(Number));
const base = 'http://localhost:'+(process.env.PORT||8791)+'/2026-09-27-combo/missions.html';
fs.mkdirSync(OUT,{recursive:true});
const browser = await chromium.launch();
async function diffAt(page, sel){
  const el = await page.$(sel); if(!el) return 'none';
  await el.scrollIntoViewIfNeeded(); await page.waitForTimeout(900);
  const a = await el.screenshot(); await page.waitForTimeout(2000); const b = await el.screenshot();
  let d=0; const n=Math.min(a.length,b.length); for(let i=0;i<n;i+=97) if(a[i]!==b[i]) d++; return d + (a.length!==b.length?'*':'');
}
for (const [w,h] of sizes) for (const theme of ['dark','light']) {
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme });
  await ctx.addInitScript(t=>{ try{localStorage.setItem('theme',t);}catch(e){} }, theme);
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error'||m.type()==='warning') errs.push(m.type()+': '+m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(base, { waitUntil:'load' });
  await page.evaluate(t=>document.documentElement.setAttribute('data-theme',t), theme);
  await page.waitForTimeout(600);
  const motion = {};
  for (const [k,s] of [['hero','.mh'],['matrix','.mh-grid > :last-child'],['sectorChart','.mission:not([hidden]) .sc-chart'],['orbit','.ow-stage'],['owChart','.ow-plot'],['fails','.mx-fgrid'],['footprint','[data-mx=footprint]']]) motion[k]=await diffAt(page,s);
  const H = await page.evaluate(()=>document.documentElement.scrollHeight);
  for (let y=0;y<H;y+=600){ await page.evaluate(y=>scrollTo(0,y),y); await page.waitForTimeout(120); }
  await page.evaluate(()=>scrollTo(0,0)); await page.waitForTimeout(1000);
  await page.screenshot({ path:`${OUT}/missions-${w}x${h}-${theme}.png`, fullPage:true });
  for (const s of ['#sectors','#orbits']) { await page.evaluate(s=>document.querySelector(s).scrollIntoView(),s); await page.waitForTimeout(700); await page.screenshot({path:`${OUT}/vp${s.slice(1)}-${w}x${h}-${theme}.png`}); }
  const m = await page.evaluate(()=>{ const r={}; for (const s of document.querySelectorAll('main > section, body > section, section.mx-sec, section.mh')) { r[s.id||s.className]=Math.round(s.getBoundingClientRect().height); }
    const wide=[...document.querySelectorAll('body *')].filter(e=>{const b=e.getBoundingClientRect(); return b.right>innerWidth+1 && b.width>0 && getComputedStyle(e).position!=='fixed' && !e.closest('[style*=overflow],.ow-list,.m-tabs');}).slice(0,6).map(e=>e.tagName+'.'+e.className);
    const dash=[...document.querySelectorAll('b,output,.v,span')].filter(e=>e.children.length===0 && /^(–|-|NaN|undefined|null|Infinity)$/.test(e.textContent.trim()) && e.offsetParent).slice(0,8).map(e=>e.className+'|'+e.parentElement.className);
    const braces=document.body.innerText.match(/\{\{[^}]*\}\}|NaN|undefined/g);
    return {sw:document.documentElement.scrollWidth,cw:document.documentElement.clientWidth,H:document.documentElement.scrollHeight,sections:r,wide,dash,braces}; });
  console.log(JSON.stringify({w,h,theme,motion,errs:errs.slice(0,6),...m}));
  await ctx.close();
}
await browser.close();
