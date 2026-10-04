// R4 shared owner (second pass): theme cycle, light nav over a dark instrument, Studio link, ledger/standards at 200 % text.
import { chromium } from 'playwright';
const base = `http://localhost:${process.env.PORT || 8797}/`; const OUT = process.argv[2];
const b = await chromium.launch(); const res = {};
{ const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: 'light' }); const p = await ctx.newPage();
  await p.goto(base + 'capabilities.html', { waitUntil: 'load' }); await p.waitForTimeout(500);
  const seq = [await p.evaluate(() => document.documentElement.getAttribute('data-theme'))];
  for (let i = 0; i < 3; i++) { await p.click('#themeBtn'); seq.push(await p.evaluate(() => [document.documentElement.getAttribute('data-theme'), (localStorage.getItem('kshana-theme') || '-'), getComputedStyle(document.documentElement).backgroundColor].join('|'))); }
  res.cycle = seq;
  await p.evaluate(() => localStorage.removeItem('kshana-theme')); await p.reload({ waitUntil: 'load' }); await p.waitForTimeout(400);
  // scroll a dark window under the nav
  const sel = await p.evaluate(() => { const c = [...document.querySelectorAll('.window,.viz-plot,canvas')].find(e => e.getBoundingClientRect().height > 300); if (!c) return null; scrollTo(0, c.getBoundingClientRect().top + scrollY + 40); return c.className || c.tagName; });
  await p.waitForTimeout(700);
  res.navOverDark = await p.evaluate(() => { const n = document.getElementById('nav'); return { cls: n.className, bg: getComputedStyle(n).backgroundColor }; });
  await p.screenshot({ path: `${OUT}/nav-light-over-dark-capabilities.png`, clip: { x: 0, y: 0, width: 1440, height: 110 } });
  const r = await p.request.get(base + 'playground/index.html'); res.studio = r.status();
  await ctx.close(); }
for (const pg of ['evidence', 'capabilities']) { const ctx = await b.newContext({ viewport: { width: 1280, height: 800 } }); const p = await ctx.newPage();
  await p.goto(base + pg + '.html', { waitUntil: 'load' }); await p.waitForTimeout(500);
  await p.evaluate(() => { const els = [...document.querySelectorAll('.sd-list *, .lg-compact *')]; const fs = els.map(e => parseFloat(getComputedStyle(e).fontSize)); els.forEach((e, i) => e.style.setProperty('font-size', fs[i] * 2 + 'px', 'important')); });
  await p.waitForTimeout(300);
  res[pg + '-overlaps'] = await p.evaluate(() => { const out = []; for (const row of document.querySelectorAll('.sd, .lg-dom, details.lg-row>summary')) { const kids = [...row.children].filter(k => k.getBoundingClientRect().width); for (let i = 0; i < kids.length; i++) for (let j = i + 1; j < kids.length; j++) { const a = kids[i].getBoundingClientRect(), c = kids[j].getBoundingClientRect(); if (a.left < c.right - 1 && c.left < a.right - 1 && a.top < c.bottom - 1 && c.top < a.bottom - 1) out.push(row.className + ':' + kids[i].tagName + '/' + kids[j].tagName); } } return out.slice(0, 8); });
  const sd = await p.$('.sd-list'); if (sd) { await sd.scrollIntoViewIfNeeded(); await sd.screenshot({ path: `${OUT}/sd-200-${pg}.png` }); }
  await ctx.close(); }
await b.close(); console.log(JSON.stringify(res, null, 1));
