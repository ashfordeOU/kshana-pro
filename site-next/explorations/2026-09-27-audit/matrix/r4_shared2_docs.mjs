// R4 shared owner (second pass): docs template features. usage: PORT=.. node r4_shared2_docs.mjs OUTDIR
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2]; fs.mkdirSync(OUT, { recursive: true });
const base = `http://localhost:${process.env.PORT || 8797}/`;
const browser = await chromium.launch();
const res = {};
for (const [w, h] of [[1440, 900], [390, 844]]) for (const theme of ['dark', 'light']) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await ctx.newPage(); const errs = [];
  page.on('console', m => { if (m.type() === 'error') errs.push(m.text().slice(0, 160)); }); page.on('pageerror', e => errs.push(String(e).slice(0, 160)));
  page.on('response', r => { if (r.status() >= 400) errs.push(r.status() + ' ' + r.url()); });
  const r = {};
  await page.goto(base + 'docs/scenario-reference.html', { waitUntil: 'load' }); await page.waitForTimeout(500);
  r.navOpen = await page.evaluate(() => document.querySelector('.docs-nav-d').open);
  r.summaryShown = await page.evaluate(() => getComputedStyle(document.querySelector('.docs-nav-d>summary')).display !== 'none');
  r.order = await page.evaluate(() => ['.docs-search', '.docs-nav-d', '.docs-main'].map(s => Math.round(document.querySelector(s).getBoundingClientRect().top)));
  r.copyBtns = await page.evaluate(() => document.querySelectorAll('.docs-copy').length);
  r.labels = await page.evaluate(() => [...new Set([...document.querySelectorAll('.prose pre')].slice(0, 30).map(p => getComputedStyle(p, '::before').content))]);
  await page.locator('.docs-copy').first().scrollIntoViewIfNeeded(); await page.locator('.docs-copy').first().click(); await page.waitForTimeout(300);
  r.clip = (await page.evaluate(() => navigator.clipboard.readText().catch(() => 'n/a'))).slice(0, 40);
  r.toast = await page.evaluate(() => (document.getElementById('toast') || {}).textContent);
  await page.locator('.docs-copy').first().screenshot({ path: `${OUT}/copy-${w}-${theme}.png` }).catch(() => {});
  // "/" focuses search; typing, arrows, enter
  await page.mouse.click(5, 300); await page.keyboard.press('/');
  r.slashFocus = await page.evaluate(() => document.activeElement.id);
  await page.keyboard.type('sgp4'); await page.waitForTimeout(500);
  r.results = await page.evaluate(() => [...document.querySelectorAll('#docsR a')].slice(0, 4).map(a => a.firstChild.textContent));
  r.count = await page.evaluate(() => document.getElementById('docsCount').textContent);
  r.expanded = await page.evaluate(() => document.getElementById('docsQ').getAttribute('aria-expanded'));
  await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowDown');
  r.active = await page.evaluate(() => document.getElementById('docsQ').getAttribute('aria-activedescendant'));
  await page.screenshot({ path: `${OUT}/search-${w}-${theme}.png` });
  await page.keyboard.press('Escape');
  r.afterEsc = await page.evaluate(() => [document.getElementById('docsQ').value, document.querySelectorAll('#docsR a').length]);
  await page.keyboard.type('holdover'); await page.waitForTimeout(400); await Promise.all([page.waitForURL(u => !String(u).includes('scenario-reference'), { timeout: 5000 }).catch(() => {}), page.keyboard.press('Enter')]); await page.waitForLoadState('load');
  r.enterWent = page.url().replace(base, '');
  // TOC active section (wide only)
  await page.goto(base + 'docs/sgp4-validation.html', { waitUntil: 'load' }); await page.waitForTimeout(300);
  await page.evaluate(() => { const hs = document.querySelectorAll('.prose h2'); if (hs[2]) hs[2].scrollIntoView(); }); await page.waitForTimeout(600);
  r.tocCurrent = await page.evaluate(() => { const a = document.querySelector('.docs-toc a[aria-current="location"]'); return a ? a.textContent : null; });
  r.h2InView = await page.evaluate(() => { const hs = document.querySelectorAll('.prose h2'); return hs[2] ? hs[2].id : null; });
  if (w < 800) { await page.evaluate(() => scrollTo(0, 0)); await page.click('.docs-nav-d>summary'); await page.waitForTimeout(200); r.phoneOpened = await page.evaluate(() => document.querySelector('.docs-nav-d').open); await page.screenshot({ path: `${OUT}/navlist-${w}-${theme}.png` }); }
  await page.goto(base + 'docs/index.html', { waitUntil: 'load' }); await page.waitForTimeout(500);
  r.cards = await page.evaluate(() => document.querySelectorAll('.docs-home h2+ul small').length);
  r.homeNavVisible = await page.evaluate(() => getComputedStyle(document.querySelector('.docs-nav-d')).display);
  await page.screenshot({ path: `${OUT}/docs-index-${w}x${h}-${theme}.png`, fullPage: true });
  r.hscroll = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  r.errs = errs; res[`${w}-${theme}`] = r; await ctx.close();
}
await browser.close();
fs.writeFileSync(`${OUT}/result.json`, JSON.stringify(res, null, 1));
console.log(JSON.stringify(res, null, 1));
