import { chromium } from 'playwright';
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:320,height:700}, reducedMotion:'reduce'})).newPage();
await p.goto('http://localhost:8731/index.html',{waitUntil:'domcontentloaded'}); await p.waitForTimeout(1000);
console.log(await p.evaluate(()=>{const e=document.querySelector('.hero-copy'),s=getComputedStyle(e),h=getComputedStyle(document.querySelector('.hero'));return {w:s.width,ml:s.marginLeft,as:s.alignSelf,tr:s.transform,translate:s.translate,scale:s.scale,ai:h.alignItems,hw:h.width,disp:h.display, cls:e.className}}));
await b.close();
