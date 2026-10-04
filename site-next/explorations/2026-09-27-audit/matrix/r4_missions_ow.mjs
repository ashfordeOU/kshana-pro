import { chromium } from 'playwright';
const OUT=process.argv[2]; const W=+(process.argv[3]||1440), Hh=+(process.argv[4]||900);
const browser = await chromium.launch();
for (const theme of ['dark','light']) {
  const ctx = await browser.newContext({ viewport:{width:W,height:Hh}, colorScheme: theme });
  const page = await ctx.newPage(); const errs=[]; page.on('pageerror',e=>errs.push(String(e))); page.on('console',m=>{if(m.type()==='error')errs.push(m.text())});
  await page.goto('http://localhost:8791/2026-09-27-combo/missions.html'); await page.evaluate(t=>document.documentElement.setAttribute('data-theme',t),theme);
  await page.evaluate(()=>document.querySelector('#orbits .ow').scrollIntoView()); await page.waitForTimeout(2500);
  for (const r of ['constellation-multi-gnss-coverage','leo-pnt-mega-shell','lunar-relay-constellation','mars-orbit-pnt']) {
    await page.click(`[data-ow-run="${r}"]`); await page.waitForTimeout(2200);
    const a = await page.$eval('[data-ow-sout]',e=>e.textContent); const ro1=await page.$eval('[data-ow-ro]',e=>e.innerText.replace(/\n/g,' '));
    await page.waitForTimeout(2000);
    const b = await page.$eval('[data-ow-sout]',e=>e.textContent); const ro2=await page.$eval('[data-ow-ro]',e=>e.innerText.replace(/\n/g,' '));
    await (await page.$('.ow')).screenshot({path:`${OUT}/ow-${r}-${W}-${theme}.png`});
    console.log(theme,r,'slider',a,'->',b,'|',ro1===ro2?'readouts STATIC':'readouts move');
  }
  console.log(errs);
  await ctx.close();
}
await browser.close();
