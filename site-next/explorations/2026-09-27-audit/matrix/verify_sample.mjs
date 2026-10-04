#!/usr/bin/env node
// Re-open sampled matrix findings in a real browser and collect evidence for a human verdict.
//   node verify_sample.mjs SAMPLE.json OUTDIR [--site=2026-09-27-combo]
// SAMPLE.json: { category: [issue, ...] } as found in report.json. Writes OUTDIR/facts.jsonl (resumable) and
// one PNG per finding (the element region, at 2x) for the verdict.
import { chromium, webkit, firefox } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as px from './lib/pix.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const EXPL = path.resolve(HERE, '..', '..');
const [sampleFile, outDir] = process.argv.slice(2);
fs.mkdirSync(outDir, { recursive: true });
const S = JSON.parse(fs.readFileSync(sampleFile, 'utf8'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const port = await new Promise((res) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); });
const server = spawn('/opt/homebrew/bin/python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1', '--directory', EXPL], { stdio: 'ignore' });
process.on('exit', () => server.kill());
await sleep(800);
const BASE = `http://127.0.0.1:${port}`;
const ENG = { chromium, webkit, firefox };
const browsers = {};

function ctxOpts(i) {
  const c = i.condition;
  let w = 1440, h = 900;
  const m = String(i.size).match(/^(\d+)x(\d+)/);
  if (m) { w = +m[1]; h = +m[2]; }
  if (i.widths) { w = i.widths[0][0]; h = +(c.match(/h=(\d+)/) || [0, 900])[1]; }
  const o = { viewport: { width: w, height: h }, deviceScaleFactor: 2, colorScheme: /dark/.test(i.theme) ? 'dark' : 'light' };
  if (/forced/.test(c)) o.forcedColors = 'active';
  if (/phone|tablet/.test(c) || (/spacing/.test(c) && w < 700)) { o.hasTouch = true; if (i.browser !== 'firefox') o.isMobile = true; }
  return o;
}

const factsFile = path.join(outDir, 'facts.jsonl');
const done = new Set(fs.existsSync(factsFile) ? fs.readFileSync(factsFile, 'utf8').trim().split('\n').filter(Boolean).map((l) => { const o = JSON.parse(l); return o.cat + '|' + o.k; }) : []);
const only = (process.argv.find((a) => a.startsWith('--only=')) || '').slice(7);
for (const [cat, list] of Object.entries(S)) {
  if (only && !only.split(',').includes(cat)) continue;
  for (let k = 0; k < list.length; k++) {
    if (done.has(cat + '|' + k)) continue;
    const i = list[k];
    const b = browsers[i.browser] || (browsers[i.browser] = await ENG[i.browser].launch({ headless: true, args: i.browser === 'chromium' ? ['--use-gl=angle', '--use-angle=metal', '--ignore-gpu-blocklist'] : [] }));
    const ctx = await b.newContext(ctxOpts(i));
    await ctx.addInitScript({ path: path.join(HERE, 'lib', 'probe.js') });
    const page = await ctx.newPage();
    page.setDefaultTimeout(30000);
    const kill = setTimeout(() => { ctx.close().catch(() => {}); }, 150000);
    const f = { cat, k, page: i.page, browser: i.browser, condition: i.condition, size: i.size, selector: i.selector, detail: i.detail };
    try {
      await page.goto(BASE + i.view, { waitUntil: 'load', timeout: 60000 });
      await Promise.race([page.evaluate(() => document.fonts.ready), sleep(8000)]);
      if (i.view.includes('playground')) await page.waitForFunction(() => { const r = document.getElementById('run'); return r && !r.disabled; }, null, { timeout: 90000 }).catch(() => {});
      await page.evaluate(async () => { const H = document.documentElement.scrollHeight; for (let y = 0; y < H; y += innerHeight) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 20)); } scrollTo(0, 0); });
      await sleep(1500);
      if (/text-only/.test(i.condition)) await page.evaluate(() => __mx.scaleText(2));
      if (/spacing/.test(i.condition)) await page.evaluate(() => __mx.textSpacing());
      await sleep(300);
      const info = await page.evaluate(({ sel, other }) => {
        const el = document.querySelector(sel);
        if (!el) return { missing: true };
        const d = (e) => {
          const r = e.getBoundingClientRect(), s = getComputedStyle(e);
          const chain = [];
          for (let a = e; a && a.nodeType === 1; a = a.parentElement) { const t = getComputedStyle(a); if (t.transform !== 'none' || t.position === 'fixed' || t.visibility !== 'visible' || a.hasAttribute('aria-hidden') || a.hasAttribute('inert') || a.hidden) chain.push(`${a.tagName.toLowerCase()}${a.id ? '#' + a.id : ''}${a.className && typeof a.className === 'string' ? '.' + a.className.split(' ')[0] : ''}{pos:${t.position},tf:${t.transform.slice(0, 40)},vis:${t.visibility},ah:${a.getAttribute('aria-hidden')},inert:${a.hasAttribute('inert')},hidden:${a.hidden}}`); }
          const rg = document.createRange(); rg.selectNodeContents(e);
          return { rect: [r.left + scrollX, r.top + scrollY, r.width, r.height].map(Math.round), textRects: [...rg.getClientRects()].slice(0, 4).map((q) => [q.left + scrollX, q.top + scrollY, q.width, q.height].map(Math.round)), opacity: __mx.effOpacity(e), color: s.color, fill: s.fill, font: s.fontSize + '/' + s.lineHeight, pos: s.position, tabindex: e.tabIndex, chain: chain.slice(0, 5), text: (e.innerText || e.textContent || '').trim().slice(0, 60) };
        };
        const o = other ? document.querySelector(other) : null;
        return { el: d(el), other: o ? d(o) : null, vw: document.documentElement.clientWidth, docW: document.documentElement.scrollWidth, rootOverflowX: getComputedStyle(document.documentElement).overflowX + '/' + getComputedStyle(document.body).overflowX };
      }, { sel: i.selector, other: i.other || null });
      Object.assign(f, info);
      if (!info.missing) {
        if (cat === 'focus-visible' || cat === 'hover-only') {
          await page.keyboard.press('Tab');
          const r = await page.evaluate((sel) => { const e = document.querySelector(sel); e.scrollIntoView({ block: 'center' }); e.focus(); const q = e.getBoundingClientRect(); return { focused: document.activeElement === e, opacityFocused: getComputedStyle(e).opacity, effFocused: __mx.effOpacity(e), outline: getComputedStyle(e).outline, fv: e.matches(':focus-visible'), rect: [q.left, q.top, q.width, q.height] }; }, i.selector);
          await sleep(400); // any transition finishes
          r.opacityFocusedAfter400ms = await page.evaluate((sel) => getComputedStyle(document.querySelector(sel)).opacity, i.selector);
          const [x, y, w, h] = r.rect;
          await page.screenshot({ path: path.join(outDir, `${cat}-${k}-focused.png`), clip: { x: Math.max(0, x - 200), y: Math.max(0, y - 20), width: Math.min(420, w + 240), height: h + 40 } });
          const hd = await page.evaluate((sel) => { const e = document.querySelector(sel); const hh = e.closest('h2,h3'); const q = (hh || e).getBoundingClientRect(); return [q.left + 10, q.top + q.height / 2]; }, i.selector);
          await page.evaluate(() => document.activeElement.blur());
          await page.mouse.move(hd[0], hd[1]);
          await sleep(400);
          r.opacityHovered = await page.evaluate((sel) => getComputedStyle(document.querySelector(sel)).opacity, i.selector);
          f.focus = r;
        } else {
          const R = info.other ? [Math.min(info.el.rect[0], info.other.rect[0]), Math.min(info.el.rect[1], info.other.rect[1]), 0, 0] : info.el.rect.slice();
          if (info.other) { R[2] = Math.max(info.el.rect[0] + info.el.rect[2], info.other.rect[0] + info.other.rect[2]) - R[0]; R[3] = Math.max(info.el.rect[1] + info.el.rect[3], info.other.rect[1] + info.other.rect[3]) - R[1]; }
          await page.evaluate((y) => scrollTo(0, Math.max(0, y - 80)), R[1]);
          await sleep(300);
          const sy = await page.evaluate(() => scrollY);
          const vp = page.viewportSize();
          const clip = { x: Math.max(0, R[0] - 24), y: Math.max(0, R[1] - sy - 24), width: 0, height: 0 };
          clip.width = Math.max(40, Math.min(vp.width - clip.x, R[2] + 48));
          clip.height = Math.max(20, Math.min(vp.height - clip.y, R[3] + 48, 600));
          if (cat === 'outside-viewport') { clip.x = 0; clip.width = vp.width; clip.y = Math.max(0, R[1] - sy - 24); clip.height = Math.max(40, Math.min(vp.height - clip.y, Math.max(R[3], 60) + 48, 500)); }
          const buf = await page.screenshot({ path: path.join(outDir, `${cat}-${k}.png`), clip, scale: 'css' });
          if (cat === 'contrast-text') {
            const img = px.decode(buf);
            const tr = info.el.textRects[0] || info.el.rect;
            const g = px.glyphContrast(img, tr[0] - clip.x, tr[1] - sy - clip.y, tr[2], tr[3]);
            if (g) f.pixels = { bg: px.hex(g.bg), fg3rd: px.hex(g.fg), r3rd: +g.r.toFixed(2) };
          }
        }
      }
    } catch (e) { f.error = String(e.message).split('\n')[0]; }
    clearTimeout(kill);
    fs.appendFileSync(factsFile, JSON.stringify(f) + '\n');
    console.log(JSON.stringify({ cat, k, page: i.page, b: i.browser, miss: f.missing, err: f.error }));
    await ctx.close().catch(() => {});
  }
}
for (const b of Object.values(browsers)) await b.close();
server.kill();
