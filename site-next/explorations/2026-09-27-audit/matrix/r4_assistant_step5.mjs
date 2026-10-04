import { chromium } from 'playwright'; import fs from 'fs';
const OUT=process.argv[2]; const port=process.env.PORT||8751;
const b=await chromium.launch();
for (const [w,h] of [[1440,900],[390,844]]) for (const theme of ['dark','light']) {
  const ctx=await b.newContext({viewport:{width:w,height:h},colorScheme:theme,reducedMotion:'reduce'});
  await ctx.addInitScript(t=>{try{localStorage.setItem('theme',t)}catch(e){}},theme);
  const p=await ctx.newPage(); const errs=[]; p.on('pageerror',e=>errs.push(String(e)));
  await p.goto(`http://localhost:${port}/index.html`); await p.evaluate(t=>document.documentElement.setAttribute('data-theme',t),theme);
  await p.locator('#mcpChat').scrollIntoViewIfNeeded();
  await p.click('#mp-t4'); await p.waitForTimeout(500);
  const m=await p.evaluate(()=>{const s=document.querySelector('#mp-p4');return {sh:s.scrollHeight,ch:s.clientHeight,status:document.querySelector('#mcpStatus').textContent,live:document.querySelector('#mcpStatus').getAttribute('aria-live'),yl:[...document.querySelectorAll('.mp-yl')].map(e=>e.textContent),ax:document.querySelector('.mp-ax')?.textContent}});
  await p.locator('#mcpChat').screenshot({path:`${OUT}/assistant-review-step5-${w}-${theme}.png`});
  // keyboard: ArrowLeft from tab 5
  await p.focus('#mp-t4'); await p.keyboard.press('ArrowLeft'); const k=await p.evaluate(()=>document.activeElement.id);
  console.log(JSON.stringify({w,theme,errs,...m,afterLeft:k})); await ctx.close();
}
await b.close();
