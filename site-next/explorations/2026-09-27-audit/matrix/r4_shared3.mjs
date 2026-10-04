// R4 shared owner (third pass): every page together. Nav links, command palette search, theme cycle
// (three states), footer, every same-site link and #anchor, the Studio links, console errors, horizontal scroll.
// Usage: node r4_shared3.mjs OUTDIR  (serves the combo site itself on a free port)
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
const SITE = new URL('../../2026-09-27-combo/', import.meta.url).pathname;
const OUT = process.argv[2]; fs.mkdirSync(OUT, { recursive: true });
const PORT = 8800 + Math.floor(Math.random() * 150);
const srv = spawn('/opt/homebrew/bin/python3', ['-m', 'http.server', String(PORT), '--bind', '127.0.0.1', '-d', SITE], { stdio: 'ignore' });
await new Promise((r) => setTimeout(r, 900));
const base = `http://127.0.0.1:${PORT}/`;
const PAGES = ['index', 'capabilities', 'missions', 'evidence', 'developers', 'editions', 'docs/index', 'docs/glossary', 'docs/capability-reference', 'docs/scenario-reference'];
const b = await chromium.launch();
const res = { pages: {}, links: {}, anchors: [], studio: {} };
const idsCache = new Map();
async function idsOf(ctx, path) {
  if (idsCache.has(path)) return idsCache.get(path);
  const r = await ctx.request.get(base + path); const t = r.ok() ? await r.text() : '';
  const ids = new Set([...t.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
  idsCache.set(path, { status: r.status(), ids }); return idsCache.get(path);
}
const allLinks = new Map();
for (const pg of PAGES) {
  const r = { errors: [], http: [] };
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: 'dark' });
  const p = await ctx.newPage();
  p.on('console', (m) => { if (m.type() === 'error') r.errors.push(m.text().slice(0, 160)); });
  p.on('pageerror', (e) => r.errors.push('pageerror: ' + String(e).slice(0, 160)));
  p.on('response', (x) => { if (x.status() >= 400 && x.url().startsWith(base)) r.http.push(x.status() + ' ' + x.url().slice(base.length)); });
  await p.goto(base + pg + '.html', { waitUntil: 'load' }); await p.waitForTimeout(700);
  // theme: three states via the button, then back to system
  const seq = [];
  for (let i = 0; i < 3; i++) { await p.click('#themeBtn'); await p.waitForTimeout(120); seq.push(await p.evaluate(() => (document.documentElement.getAttribute('data-theme') || 'system') + ':' + getComputedStyle(document.body).backgroundColor)); }
  r.theme = seq;
  // nav + footer
  r.nav = await p.evaluate(() => [...document.querySelectorAll('#navLinks a, .nav a.brand, nav a')].map((a) => a.getAttribute('href')).filter(Boolean).slice(0, 20));
  r.current = await p.evaluate(() => document.querySelector('#navLinks a[aria-current="page"]')?.textContent.trim() || null);
  r.footer = await p.evaluate(() => { const f = document.querySelector('footer'); return f ? { links: f.querySelectorAll('a').length, credit: /NASA/.test(f.textContent) } : null; });
  // palette search
  await p.evaluate(() => window.scrollTo(0, 0));
  await p.keyboard.press(process.platform === 'darwin' ? 'Meta+k' : 'Control+k'); await p.waitForTimeout(250);
  let palOpen = await p.evaluate(() => { const x = document.getElementById('palette'); return !!x && (x.open || !x.hidden && getComputedStyle(x).display !== 'none'); });
  if (!palOpen) { await p.click('#kbtn').catch(() => {}); await p.waitForTimeout(250); palOpen = await p.evaluate(() => { const x = document.getElementById('palette'); return !!x && (x.open || getComputedStyle(x).display !== 'none'); }); }
  await p.fill('#palInput', 'holdover').catch(() => {}); await p.waitForTimeout(250);
  r.palette = { open: palOpen, results: await p.evaluate(() => document.querySelectorAll('#palList .pal-item').length), count: await p.evaluate(() => document.getElementById('palCount')?.textContent || '') };
  await p.keyboard.press('Escape');
  // horizontal scroll at 390 and 320
  for (const w of [390, 320]) { await p.setViewportSize({ width: w, height: 844 }); await p.waitForTimeout(250); r['hscroll' + w] = await p.evaluate(() => document.documentElement.scrollWidth - innerWidth); }
  // links
  const links = await p.evaluate(() => [...document.querySelectorAll('a[href]')].map((a) => a.href));
  for (const l of links) if (l.startsWith(base)) { if (!allLinks.has(l)) allLinks.set(l, new Set()); allLinks.get(l).add(pg); }
  res.pages[pg] = r; await ctx.close();
}
// resolve every same-site link and anchor
const ctx = await b.newContext();
for (const [l, from] of allLinks) {
  const u = new URL(l); let path = u.pathname.slice(1) || 'index.html'; if (path.endsWith('/')) path += 'index.html';
  const got = await idsOf(ctx, path);
  if (got.status >= 400) res.links[l.slice(base.length)] = { status: got.status, from: [...from].slice(0, 3) };
  else if (u.hash && u.hash.length > 1 && path.endsWith('.html') && !path.startsWith('playground/')) {
    const id = decodeURIComponent(u.hash.slice(1)); if (!got.ids.has(id)) res.anchors.push({ link: l.slice(base.length), from: [...from].slice(0, 3) });
  }
}
res.linkCount = allLinks.size;
// Studio: open three recorded-run links as a reader would and check it runs clean
const studio = [...allLinks.keys()].filter((l) => l.includes('playground/index.html?scenario=')).slice(0, 3);
studio.unshift(base + 'playground/index.html');
for (const l of studio) {
  const c = await b.newContext({ viewport: { width: 1440, height: 900 } }); const p = await c.newPage(); const errs = [];
  p.on('pageerror', (e) => errs.push(String(e).slice(0, 160))); p.on('console', (m) => { if (m.type() === 'error') errs.push(m.text().slice(0, 160)); });
  const rr = await p.goto(l, { waitUntil: 'load' }); await p.waitForTimeout(2500);
  res.studio[l.slice(base.length)] = { status: rr.status(), errors: errs.slice(0, 4), title: await p.title() };
  if (l.endsWith('index.html')) await p.screenshot({ path: `${OUT}/studio-1440.png` });
  await c.close();
}
// abbreviation keys, both themes and phone width
for (const [w, h] of [[1440, 900], [390, 844]]) for (const scheme of ['dark', 'light']) {
  const c = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme }); const p = await c.newPage();
  await p.goto(base + 'evidence.html', { waitUntil: 'load' }); await p.waitForTimeout(500);
  const sd = await p.$('#standards .sd-list'); if (sd) { await sd.scrollIntoViewIfNeeded(); await p.waitForTimeout(300); await sd.screenshot({ path: `${OUT}/standards-${w}-${scheme}.png` }); }
  await p.goto(base + 'capabilities.html', { waitUntil: 'load' }); await p.waitForTimeout(500);
  const ok = await p.evaluate(() => { const d = document.querySelector('details.cx-row'); if (!d) return false; d.open = true; d.scrollIntoView({ block: 'center' }); return true; });
  if (ok) { await p.waitForTimeout(300); const d = await p.$('details.cx-row[open]'); if (d && await d.isVisible()) await d.screenshot({ path: `${OUT}/explorer-row-${w}-${scheme}.png` }); }
  await c.close();
}
await b.close(); srv.kill();
fs.writeFileSync(`${OUT}/result.json`, JSON.stringify(res, null, 1));
const bad = Object.entries(res.pages).filter(([, r]) => r.errors.length || r.http.length || r.hscroll390 > 0 || r.hscroll320 > 0 || !r.palette.open || !r.palette.results || !r.footer);
console.log(JSON.stringify({ links: res.linkCount, brokenLinks: Object.keys(res.links).length, brokenAnchors: res.anchors.length, pagesWithProblems: bad.map(([k, r]) => ({ k, errors: r.errors.slice(0, 3), http: r.http.slice(0, 3), h390: r.hscroll390, h320: r.hscroll320, palette: r.palette, footer: r.footer })), theme: Object.fromEntries(Object.entries(res.pages).map(([k, r]) => [k, r.theme.map((x) => x.split(':')[0]).join('>')])), studio: res.studio }, null, 1));
