// Quick visual peek for the r4 full-matrix fix pass: node r4_full_peek.mjs <page> <outdir> [w,h,theme,scrollFrac ...]
import { chromium } from 'playwright';
import fs from 'node:fs';
const [page = 'index.html', out = '/tmp/peek', ...specs] = process.argv.slice(2);
fs.mkdirSync(out, { recursive: true });
const b = await chromium.launch();
for (const sp of (specs.length ? specs : ['1440,900,dark,0', '390,844,light,0'])) {
  const [w, h, theme, frac = '0', sel = ''] = sp.split(',');
  const ctx = await b.newContext({ viewport: { width: +w, height: +h }, colorScheme: theme });
  const p = await ctx.newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(String(e))); p.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()); });
  await p.goto('http://127.0.0.1:8811/2026-09-27-combo/' + page, { waitUntil: 'networkidle' });
  await p.waitForTimeout(1500);
  if (sel) await p.evaluate((s) => document.querySelector(s).scrollIntoView({ block: 'start' }), sel);
  else await p.evaluate((f) => scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * f), +frac);
  await p.waitForTimeout(2500);
  const f = `${out}/${page.replace(/[\/.]/g, '_')}-${w}x${h}-${theme}-${frac}${sel ? sel.replace(/[^a-z0-9]/gi, '') : ''}.png`;
  await p.screenshot({ path: f });
  console.log(f, errs.length ? 'ERR ' + errs.join(' | ') : '');
  await ctx.close();
}
await b.close();
