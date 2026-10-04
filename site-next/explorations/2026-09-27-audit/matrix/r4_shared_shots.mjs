// R4 shared owner: section shots of the shared player (Home #assistant, Developers #start) + page tops, dark/light.
import { chromium } from 'playwright';
const OUT='../shots/r4/shared', base='http://localhost:8797/';
const b=await chromium.launch();
for (const [p,sel] of [['index','#assistant'],['developers','#start'],['editions','#nav']]) for (const [w,h] of [[1440,900],[390,844]]) for (const t of ['dark','light']) {
  const c=await b.newContext({viewport:{width:w,height:h},colorScheme:t}); const pg=await c.newPage();
  await pg.goto(base+p+'.html',{waitUntil:'load'}); await pg.evaluate(t=>document.documentElement.setAttribute('data-theme',t),t);
  await pg.evaluate(async()=>{for(let y=0;y<document.documentElement.scrollHeight;y+=800){scrollTo(0,y);await new Promise(r=>setTimeout(r,40));}});
  if (sel!=='#nav') { await pg.evaluate(s=>{const e=document.querySelector(s);scrollTo(0,e.getBoundingClientRect().top+scrollY-64)},sel); await pg.waitForTimeout(1200); await pg.locator(sel).screenshot({path:`${OUT}/${p}-${sel.slice(1)}-${w}x${h}-${t}.png`}); }
  else { await pg.evaluate(()=>scrollTo(0,0)); await pg.waitForTimeout(600); await pg.screenshot({path:`${OUT}/${p}-top-${w}x${h}-${t}.png`}); }
  await c.close();
}
await b.close(); console.log('ok');
