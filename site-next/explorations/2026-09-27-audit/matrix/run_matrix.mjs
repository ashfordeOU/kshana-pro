#!/usr/bin/env node
// Display-matrix audit for the redesigned Kshana site (GOAL.md, "Display matrix" standing rule).
//
//   node run_matrix.mjs --quick                 key sizes, Chromium, a page subset
//   node run_matrix.mjs --full                  everything, three engines, 4 contexts at a time
//   node run_matrix.mjs --fixtures              mutation fixtures + clean control, catch table
//   options: --browsers=chromium,webkit,firefox  --pages=index,docs/*,studio*  --groups=sweep,themes
//            --conc=4  --sweep-step=10  --out=DIR  --no-ios  --no-shots  --site=DIR (under explorations/)
//
// Output: out/<stamp>/report.json, report.md, crops/, shots/. Exit 1 on any issue or tool error.
import { chromium, webkit, firefox } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn, execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as px from './lib/pix.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const EXPL = path.resolve(HERE, '..', '..'); // explorations/
const argv = process.argv.slice(2);
const opt = (k, d) => { const a = argv.find((x) => x.startsWith(`--${k}=`)); return a ? a.slice(k.length + 3) : d; };
const flag = (k) => argv.includes(`--${k}`);
// --probe=lib/probe-r4.js runs another probe file (the in-page code is read at every context creation,
// so a probe edit reaches a run already in flight; a separate file keeps that run on one version).
const PROBE = path.resolve(HERE, opt('probe', path.join('lib', 'probe.js')));
const MODE = flag('full') ? 'full' : flag('fixtures') ? 'fixtures' : 'quick';
const FULLISH = MODE !== 'quick';
const SITE = opt('site', '2026-09-27-combo');
const CONC = +opt('conc', 4);
const SWEEP_STEP = +opt('sweep-step', MODE === 'quick' ? 40 : 10);
const BROWSERS = opt('browsers', MODE === 'full' ? 'chromium,webkit,firefox' : 'chromium').split(',');
const PAGE_FILTER = opt('pages', '');
const GROUP_FILTER = opt('groups', '');
const STAMP = new Date().toISOString().replace(/[-:]/g, '').replace(/\..*/, '').replace('T', '-');
const OUT = path.resolve(opt('out', path.join(HERE, 'out', (MODE === 'fixtures' ? 'fixtures-' : MODE + '-') + STAMP)));
const CROPS = path.join(OUT, 'crops'), SHOTS = path.join(OUT, 'shots');
fs.mkdirSync(CROPS, { recursive: true });
fs.mkdirSync(SHOTS, { recursive: true });
const ENGINES = { chromium, webkit, firefox };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const LOG = fs.createWriteStream(path.join(OUT, 'progress.log'));
const log = (...a) => { const s = `[${new Date().toISOString().slice(11, 19)}] ${a.join(' ')}`; console.log(s); LOG.write(s + '\n'); };

// ---------------------------------------------------------------- the matrix
const FIXED = [
  ['phone', 320, 568], ['phone', 360, 740], ['phone', 375, 667], ['phone', 390, 844], ['phone', 414, 896], ['phone', 430, 932],
  ['tablet', 768, 1024], ['tablet', 820, 1180], ['tablet', 1024, 1366],
  ['laptop', 1280, 720], ['laptop', 1280, 800], ['laptop', 1366, 768], ['laptop', 1440, 900], ['laptop', 1536, 864],
  ['desktop', 1920, 1080], ['desktop', 2560, 1440], ['desktop', 3840, 2160],
  ['ultrawide', 3440, 1440],
];
// Both orientations of every fixed device, plus the browser full-screen window (no chrome).
const DEVICES = [];
for (const [cls, w, h] of FIXED) {
  for (const [W, H, o] of [[w, h, w < h ? 'portrait' : 'landscape'], [h, w, w < h ? 'landscape' : 'portrait']]) {
    if (!DEVICES.some((d) => d.w === W && d.h === H)) DEVICES.push({ cls, w: W, h: H, o, touch: cls === 'phone' || cls === 'tablet', phone: cls === 'phone' });
  }
}
DEVICES.push({ cls: 'fullscreen-window', w: 1512, h: 982, o: 'landscape', touch: false, phone: false }, { cls: 'fullscreen-window', w: 1728, h: 1117, o: 'landscape', touch: false, phone: false }, { cls: 'fullscreen-window', w: 1920, h: 1200, o: 'landscape', touch: false, phone: false });
const QUICK_DEV = [[390, 844], [844, 390], [768, 1024], [1440, 900], [1920, 1080]];
const KEY_SIZES = [[390, 844], [768, 1024], [1440, 900], [1920, 1080]];
const HEIGHTS = [400, 500, 600, 800, 1600], HEIGHT_W = [1024, 1440];
const ZOOMS = [0.5, 0.8, 1, 1.25, 1.5, 2, 3, 4];
const THEME_STATES = {
  'system-light': { colorScheme: 'light', stamp: null },
  'system-dark': { colorScheme: 'dark', stamp: null },
  'stamp-light': { colorScheme: 'dark', stamp: 'light' }, // the stamp must beat the OS setting
  'stamp-dark': { colorScheme: 'light', stamp: 'dark' },
};
const CVDS = ['protanopia', 'deuteranopia', 'tritanopia'];
const STUDIO_VIEWS = ['', 'scenario=jamming-demo&tab=signal', 'scenario=telecom-prtc-holdover-24h&tab=masks', 'scenario=ephemeris&tab=ground', 'scenario=orbit-sgp4-gps&tab=orbit', 'scenario=integrity-raim&tab=timeseries'];

function siteViews() {
  const root = path.join(EXPL, SITE);
  const views = [];
  for (const p of ['index', 'missions', 'capabilities', 'evidence', 'developers', 'editions']) views.push({ id: p, path: `/${SITE}/${p}.html` });
  for (const f of fs.readdirSync(path.join(root, 'docs')).filter((f) => f.endsWith('.html')).sort()) views.push({ id: 'docs/' + f.replace(/\.html$/, ''), path: `/${SITE}/docs/${f}` });
  for (const q of STUDIO_VIEWS) views.push({ id: 'studio' + (q ? '?' + q.replace(/scenario=|tab=/g, '').replace('&', '/') : ''), path: `/${SITE}/playground/index.html` + (q ? '?' + q : ''), studio: true });
  return views;
}
function fixtureViews() {
  const dir = path.join(HERE, 'fixtures');
  return fs.readdirSync(dir).filter((f) => f.endsWith('.html')).sort().map((f) => {
    const src = fs.readFileSync(path.join(dir, f), 'utf8');
    const m = src.match(/<meta name="mx-expect" content="([^"]*)" data-target="([^"]*)"(?: data-why="([^"]*)")?/);
    return { id: 'fixture/' + f.replace(/\.html$/, ''), path: '/' + path.relative(EXPL, path.join(dir, f)).split(path.sep).join('/'), expect: m ? m[1].split(',').filter(Boolean) : [], target: m ? m[2] : '', why: m ? m[3] || '' : '' };
  });
}
const QUICK_PAGES = ['index', 'missions', 'capabilities', 'evidence', 'developers', 'editions', 'docs/index', 'docs/scenario-reference', 'studio', 'studio?jamming-demo/signal'];
const globMatch = (id, pat) => new RegExp('^' + pat.split('*').map((s) => s.replace(/[.?+^$()[\]{}|\\/]/g, '\\$&')).join('.*') + '$').test(id);
let VIEWS = MODE === 'fixtures' ? fixtureViews() : siteViews();
if (PAGE_FILTER) VIEWS = VIEWS.filter((v) => PAGE_FILTER.split(',').some((p) => globMatch(v.id, p)));
else if (MODE === 'quick') VIEWS = VIEWS.filter((v) => QUICK_PAGES.includes(v.id));

// ---------------------------------------------------------------- server
async function freePort() { return new Promise((res) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); }); }
const PORT = await freePort();
const BASE = `http://127.0.0.1:${PORT}`;
const server = spawn('python3', ['-m', 'http.server', String(PORT), '--bind', '127.0.0.1', '--directory', EXPL], { stdio: 'ignore' });
for (let i = 0; i < 50; i++) { try { const r = await fetch(BASE + '/'); if (r.ok) break; } catch { await sleep(100); } }
const cleanup = () => { try { server.kill(); } catch { /* gone */ } };
process.on('exit', cleanup);
process.on('SIGINT', () => { cleanup(); process.exit(130); });

// ---------------------------------------------------------------- browsers
const launched = {};
async function browserFor(name) {
  if (!launched[name]) {
    // Metal-backed ANGLE: WebGL (the globe) renders on the GPU and screenshots are ~10x faster
    // than SwiftShader on a loaded machine.
    const args = name === 'chromium' ? ['--use-gl=angle', '--use-angle=metal', '--ignore-gpu-blocklist'] : [];
    launched[name] = ENGINES[name].launch({ headless: true, args });
  }
  return launched[name];
}
// The site's web fonts come from a third party (Google Fonts). Each engine's responses are fetched
// once (up to four tries) and replayed byte for byte, as a browser with a warm cache would: a
// network blip during a run of several hours otherwise reads as console errors and, worse, as
// layout findings measured in the fallback font (the clean control failed that way, 2026-09-30).
const FONT_CACHE = new Map();
const FONT_STATS = { fetched: 0, replayed: 0, failed: 0 };
async function fontRoute(bname, route) {
  const key = bname + ' ' + route.request().url();
  let hit = FONT_CACHE.get(key);
  if (hit) FONT_STATS.replayed++;
  for (let i = 0; i < 4 && !hit; i++) {
    try { const r = await route.fetch({ timeout: 20000 }); if (r.ok()) { const h = { ...r.headers() }; delete h['content-encoding']; delete h['content-length']; delete h['transfer-encoding']; hit = { status: r.status(), headers: h, body: await r.body() }; FONT_CACHE.set(key, hit); FONT_STATS.fetched++; } else await sleep(1500 * (i + 1)); }
    catch (e) { await sleep(1500 * (i + 1)); }
  }
  if (hit) return route.fulfill({ status: hit.status, headers: hit.headers, body: hit.body });
  FONT_STATS.failed++;
  return route.continue();
}
async function newCtx(bname, o = {}) {
  const b = await browserFor(bname);
  const co = {
    viewport: o.viewport || { width: 1440, height: 900 },
    deviceScaleFactor: o.dsf || 1,
    hasTouch: !!o.touch,
    colorScheme: o.colorScheme || 'light',
    reducedMotion: o.reducedMotion || 'no-preference',
    serviceWorkers: 'block',
  };
  if (o.touch && bname !== 'firefox') co.isMobile = true;
  if (o.forcedColors) co.forcedColors = o.forcedColors;
  const ctx = await b.newContext(co);
  await ctx.addInitScript({ path: PROBE });
  await ctx.route(/^https:\/\/fonts\.(googleapis|gstatic)\.com\//, (route) => fontRoute(bname, route).catch(() => route.continue().catch(() => { /* page gone */ })));
  if (o.stamp) await ctx.addInitScript((t) => { try { localStorage.setItem('kshana-theme', t); } catch (e) { /* storage blocked */ } }, o.stamp);
  return ctx;
}

// ---------------------------------------------------------------- page helpers
const IGNORED_REQ = /favicon|net::ERR_ABORTED|NS_BINDING_ABORTED|cancelled|Load request cancelled/i;
// Resource hints are advisory: WebKit logs a failed <link rel=preconnect> as a console error
// when the network hiccups (seen on the clean control, 2026-09-28), with no effect on the page.
const IGNORED_CONSOLE = /Failed to preconnect to /i;
async function openView(ctx, view, job) {
  const page = await ctx.newPage();
  page.__errors = [];
  page.on('console', (m) => { if (m.type() === 'error' && !IGNORED_CONSOLE.test(m.text())) page.__errors.push('console: ' + m.text().slice(0, 240)); });
  page.on('pageerror', (e) => page.__errors.push('pageerror: ' + String(e.message || e).slice(0, 240)));
  page.on('requestfailed', (r) => { const f = (r.failure() && r.failure().errorText) || ''; const t = r.url() + ' ' + f; if (!IGNORED_REQ.test(t)) page.__errors.push('requestfailed: ' + r.url().slice(0, 160) + ' ' + f); });
  page.on('response', (r) => { if (r.status() >= 400 && !/favicon/.test(r.url())) page.__errors.push(`http ${r.status()}: ${r.url().slice(0, 160)}`); });
  page.setDefaultTimeout(30000);
  try { await page.goto(BASE + view.path, { waitUntil: 'load', timeout: 60000 }); }
  catch (e) { page.__errors.push('goto: ' + e.message.split('\n')[0]); }
  await settle(page, view);
  return page;
}
// Wait until the page is at rest: fonts loaded, the Studio's engine run finished (never
// measure the Studio mid-run), and finite entrance animations done.
async function settle(page, view, maxAnim = 3000) {
  await page.evaluate(() => document.fonts && document.fonts.ready.then(() => 1)).catch(() => {});
  if (view.studio) await studioIdle(page);
  await page.evaluate((ms) => Promise.race([Promise.all(document.getAnimations().filter((a) => { const t = a.effect && a.effect.getComputedTiming(); return !t || t.iterations !== Infinity; }).map((a) => a.finished.catch(() => {}))), new Promise((r) => setTimeout(r, ms))]), maxAnim).catch(() => {});
}
async function studioIdle(page, timeout = 90000) {
  await page.waitForFunction(() => {
    const b = document.getElementById('run');
    const tabs = document.getElementById('tabs');
    const said = ['notice', 'error'].some((id) => { const e = document.getElementById(id); return e && !e.hidden && e.textContent.trim(); });
    return (!b || !b.disabled) && ((tabs && tabs.children.length > 0) || said);
  }, null, { timeout }).catch(() => { page.__errors.push('studio: engine run did not finish in ' + timeout / 1000 + ' s'); });
}
const settleAnims = (page, ms) => page.evaluate((m) => Promise.race([Promise.all(document.getAnimations().filter((a) => { const t = a.effect && a.effect.getComputedTiming(); return !t || t.iterations !== Infinity; }).map((a) => a.finished.catch(() => {}))), new Promise((r) => setTimeout(r, m))]), ms).catch(() => {});
const frames = (page, n = 2) => page.evaluate((k) => new Promise((r) => { let i = 0; const f = () => (++i >= k ? r() : requestAnimationFrame(f)); requestAnimationFrame(f); }), n).catch(() => {});
async function resizeTo(page, view, w, h) {
  await page.setViewportSize({ width: w, height: h });
  await frames(page, 2);
  await sleep(90);
  // A resize can start CSS transitions (an off-canvas drawer sliding shut at a breakpoint);
  // measuring mid-transition reported the Studio library half-way off screen.
  await settleAnims(page, 1000);
  if (view.studio) await studioIdle(page, 30000);
}
// Scroll the whole document once so scroll-triggered reveals and lazy content run.
async function scrollThrough(page, stepMs = 30) {
  const H = await page.evaluate(() => document.documentElement.scrollHeight).catch(() => 0);
  const vh = page.viewportSize().height;
  const stride = Math.max(vh * 0.9, H / 300);
  for (let y = 0; y < H; y += stride) { await page.evaluate((yy) => window.scrollTo(0, yy), y).catch(() => {}); await sleep(stepMs); }
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight)).catch(() => {});
  await sleep(stepMs * 3);
  await page.evaluate(() => window.scrollTo(0, 0)).catch(() => {});
  await page.evaluate(() => Promise.race([Promise.all(document.getAnimations().filter((a) => { const t = a.effect && a.effect.getComputedTiming(); return !t || t.iterations !== Infinity; }).map((a) => a.finished.catch(() => {}))), new Promise((r) => setTimeout(r, 2500))])).catch(() => {});
}
const shot = async (page, clip) => px.decode(await page.screenshot({ type: 'png', scale: 'css', clip, animations: 'allow', caret: 'hide', timeout: 30000 }));
async function vpShot(page) { const v = page.viewportSize(); return shot(page, { x: 0, y: 0, width: v.width, height: v.height }); }

// ---------------------------------------------------------------- issues and crops
const ALL_ISSUES = [], TOOL_ERRORS = [], NOTES = [], RUNS = {};
const cropBySig = new Map();
let cropCount = 0;
const CROP_CAP = MODE === 'full' ? 12000 : 4000;
const addRuns = (group, n) => { RUNS[group] = (RUNS[group] || 0) + n; };
class Job {
  constructor(bname, view, group, label) { this.bname = bname; this.view = view; this.group = group; this.label = label; this.crops = 0; this.issues = []; }
  base(cond) {
    return Object.assign({ page: this.view.id, view: this.view.path, browser: this.bname, group: this.group }, cond);
  }
  // Record issues from one measurement; crop each new signature from the live page.
  async add(page, cond, list, cropNow = true) {
    const fresh = [];
    for (const it of list) {
      const iss = Object.assign(this.base(cond), { category: it.cat, selector: it.sel, text: it.text || '', section: it.sec || '', detail: it.detail || '', other: it.other || undefined, rect: it.rect || null });
      const sig = [iss.page, iss.browser, iss.category, iss.selector, iss.other || ''].join('|');
      iss.sig = sig;
      if (cropBySig.has(sig)) iss.crop = cropBySig.get(sig);
      else if (cropNow && page && iss.rect) fresh.push(iss);
      this.issues.push(iss);
    }
    for (const iss of fresh) {
      if (cropBySig.has(iss.sig)) { iss.crop = cropBySig.get(iss.sig); continue; }
      if (this.crops >= 80 || cropCount >= CROP_CAP) continue;
      iss.crop = await cropIssue(page, iss.rect).catch(() => null);
      if (iss.crop) { cropBySig.set(iss.sig, iss.crop); this.crops++; }
    }
  }
  errors(page, cond) {
    if (!page || !page.__errors) return;
    const seen = new Set();
    for (const e of page.__errors.splice(0)) {
      if (seen.has(e)) continue;
      seen.add(e);
      const iss = Object.assign(this.base(cond), { category: 'console', selector: 'window', text: '', detail: e, rect: null });
      iss.sig = [iss.page, iss.browser, 'console', e.replace(/:\d+:\d+/g, '')].join('|');
      iss.crop = cropBySig.get(iss.page + '|' + iss.browser + '|pagecrop') || null;
      this.issues.push(iss);
    }
  }
}
async function cropIssue(page, r) {
  const v = page.viewportSize();
  const sy0 = await page.evaluate(() => [scrollX, scrollY]);
  let sy = sy0[1];
  if (r.y < sy || r.y + Math.min(r.h, v.height) > sy + v.height) {
    await page.evaluate((y) => window.scrollTo(window.scrollX, y), Math.max(0, r.y - 60));
    await sleep(60);
    sy = await page.evaluate(() => scrollY);
  }
  const x = Math.max(0, Math.min(v.width - 8, r.x - sy0[0] - 16)), y = Math.max(0, Math.min(v.height - 8, r.y - sy - 16));
  const w = Math.max(8, Math.min(v.width - x, r.w + 32, 1400)), h = Math.max(8, Math.min(v.height - y, r.h + 32, 900));
  const name = String(++cropCount).padStart(5, '0') + '.png';
  await page.screenshot({ path: path.join(CROPS, name), clip: { x, y, width: w, height: h }, scale: 'css', timeout: 20000 });
  if (sy !== sy0[1]) await page.evaluate((p) => window.scrollTo(p[0], p[1]), sy0);
  return 'crops/' + name;
}
const sizeOf = (page) => { const v = page.viewportSize(); return `${v.width}x${v.height}`; };

// ---------------------------------------------------------------- pixel graders
// Text contrast on the real rendered background: glyphs hidden, background sampled behind
// every text box (so images, gradients and translucent layers count), glyph colour from CSS.
async function pixelContrast(page, forced = false) {
  const boxes = await page.evaluate(() => __mx.textBoxes());
  if (!boxes.length) return { issues: [], measured: new Set() };
  let bgImg = null, img = null;
  if (forced) img = await vpShot(page);
  else {
    await page.evaluate(() => __mx.hideText(true));
    await frames(page, 1);
    bgImg = await vpShot(page);
    await page.evaluate(() => __mx.hideText(false));
  }
  const out = [], measured = new Set();
  for (const b of boxes) {
    const rs = [];
    if (forced && b.text.replace(/[^\p{L}\p{N}]/gu, '').length < 2) continue; // punctuation only: too few pixels to read
    // Forced colours: the ground comes from pixels (the modal colour of the text box). The glyph
    // is read two ways and the text fails only when both fail. Pixels alone under-read small
    // text: anti-aliased 10 px glyphs never reach their full colour (#8088A6 SVG ticks read as
    // #6B728C, 4.25:1 instead of 5.8:1) and colour emoji are not the text colour, which filled
    // the baseline with false failures. The computed colour alone is not always the used one
    // either: Chromium forces link text to LinkText at paint time while getComputedStyle keeps
    // the author colour (the header's Studio link computes #07080E and paints white).
    if (forced) for (const [x, y, w, h] of b.rects) { const g = px.glyphContrast(img, x, y, w, h); if (g) { const fg = px.blend(b.fg, g.bg); const rCss = px.ratio(fg, g.bg); rs.push({ r: Math.max(rCss, g.r), bg: g.bg, fg, rCss, rPix: g.r, pfg: g.fg }); } }
    else for (const [x, y] of b.pts) {
      const bg = px.px(bgImg, x, y);
      rs.push({ r: px.ratio(px.blend(b.fg, bg), bg), bg });
    }
    if (!rs.length) continue;
    rs.sort((a, c) => a.r - c.r);
    // The lowest third of the samples decides: text over an image fails where the image is
    // worst, but one sample landing on a neighbouring line or border does not.
    const pick = forced ? rs[rs.length - 1] : rs[Math.floor((rs.length - 1) / 3)];
    measured.add(b.sel);
    const need = b.large ? 3 : 4.5;
    if (pick.r + 0.005 < need) {
      const [x, y, w, h] = b.rects[0];
      const fgTxt = forced ? `computed glyph colour ${px.hex(pick.fg)} gives ${pick.rCss.toFixed(2)}:1, rendered glyph pixels ${px.hex(pick.pfg)} give ${pick.rPix.toFixed(2)}:1,` : `text ${px.hex(b.fg)}${b.fg[3] < 1 ? ' @' + b.fg[3].toFixed(2) : ''}`;
      out.push({ cat: 'contrast-text', sel: b.sel, text: b.text, sec: b.sec, detail: `${pick.r.toFixed(2)}:1 (needs ${need}:1), ${fgTxt} on rendered ${px.hex(pick.bg)} [pixels]${b.shadow ? ' (has text-shadow, not counted)' : ''}`, rect: { x: Math.round(x + b.sx), y: Math.round(y + b.sy), w: Math.round(w), h: Math.round(h) } });
    }
  }
  return { issues: out, measured };
}
// Non-text contrast (WCAG 1.4.11) for chart marks, legend swatches and icon-only controls,
// sampled from pixels: the marks are hidden and the ground behind them is read back.
async function markContrast(page, forced = false) {
  const groups = await page.evaluate(() => __mx.marks());
  if (!groups.length) return [];
  // Where each tagged mark sits, read before and after the two captures: a mark that moved in
  // between (a replay dot riding its line, a pulse ring) was sampled at points it had left.
  const boxes = () => page.evaluate(() => { const o = {}; document.querySelectorAll('[data-mx-mark]').forEach((e) => { const r = e.getBoundingClientRect(), g = e.getAttribute('data-mx-mark'); (o[g] = o[g] || []).push([r.x, r.y, r.width, r.height]); }); return o; }).catch(() => ({}));
  const before = await boxes();
  await page.evaluate(() => __mx.hideText(true));
  await frames(page, 1);
  const withMarks = await vpShot(page);
  await page.evaluate(() => __mx.hideMarks(true));
  await frames(page, 1);
  const without = await vpShot(page);
  await page.evaluate(() => { __mx.hideMarks(false); __mx.hideText(false); });
  const after = await boxes();
  const moved = new Set(Object.keys(before).filter((g) => { const a = before[g], b = after[g] || []; return a.length !== b.length || a.some((r, i) => r.some((v, j) => Math.abs(v - b[i][j]) > 1)); }));
  const sy = await page.evaluate(() => [scrollX, scrollY]);
  const out = [];
  for (const g of groups) {
    if (moved.has(g.id)) { MOVED.count++; continue; }
    const what = g.kind === 'chart' ? 'chart mark' : g.kind === 'swatch' ? 'legend swatch' : 'icon';
    const many = `${g.n} element${g.n > 1 ? 's' : ''} in ${g.container}`;
    if (g.region) {
      // Small graphics (icons, swatches): judged on every pixel that changes when hidden.
      const [x, y, w, h] = g.region;
      const rect = { x: Math.round(x + sy[0]), y: Math.round(y + sy[1]), w: Math.round(w), h: Math.round(h) };
      const m = px.regionMarkContrast(withMarks, without, x, y, w, h);
      if (m.changed < 3) out.push({ cat: 'invisible-mark', sel: g.sel, text: g.text, sec: g.container, detail: `${what} is not visible against the rendered ground ${px.hex(m.bg)} (${many})`, rect });
      else if (m.r + 0.005 < 3) out.push({ cat: 'contrast-nontext', sel: g.sel, text: g.text, sec: g.container, detail: `${what} rendered ${px.hex(m.c)} on ${px.hex(m.bg)} is ${m.r.toFixed(2)}:1 (needs 3:1; ${many})`, rect });
      continue;
    }
    if (!g.points.length) continue;
    let maxDiff = 0, best = 1;
    const rs = [];
    for (const [x, y] of g.points) {
      const bg = px.px(without, x, y), shown = px.px(withMarks, x, y);
      maxDiff = Math.max(maxDiff, px.diff(bg, shown));
      best = Math.max(best, px.ratio(shown, bg));
      rs.push({ r: px.ratio(px.blend(g.colour, bg), bg), bg });
    }
    rs.sort((a, b) => a.r - b.r);
    const med = rs[Math.floor(rs.length / 2)];
    const ratioUsed = forced ? best : med.r; // forced colours: the CSS colour is not the used colour
    const xs = g.points.map((p) => p[0]), ys = g.points.map((p) => p[1]);
    const rect = { x: Math.round(Math.min(...xs) + sy[0] - 4), y: Math.round(Math.min(...ys) + sy[1] - 4), w: Math.round(Math.max(...xs) - Math.min(...xs) + 8), h: Math.round(Math.max(...ys) - Math.min(...ys) + 8) };
    if (maxDiff < 10 && process.env.MX_DEBUG_MARKS) { try { const d = process.env.MX_DEBUG_MARKS; fs.mkdirSync(d, { recursive: true }); const k = Date.now() + '-' + Math.random().toString(36).slice(2, 6); fs.writeFileSync(path.join(d, k + '.json'), JSON.stringify({ sel: g.sel, colour: g.colour, points: g.points, scroll: sy, vp: page.viewportSize(), url: page.url(), samples: g.points.map(([x, y]) => [px.px(withMarks, x, y), px.px(without, x, y)]) })); fs.writeFileSync(path.join(d, k + '.with.png'), await page.screenshot({ type: 'png' })); } catch (e) { /* debug only */ } }
    if (maxDiff < 10) out.push({ cat: 'invisible-mark', sel: g.sel, text: g.text, sec: g.container, detail: `${what} ${px.hex(g.colour)} is not visible against the rendered ground ${px.hex(med.bg)} (${many})`, rect });
    else if (ratioUsed + 0.005 < 3) out.push({ cat: 'contrast-nontext', sel: g.sel, text: g.text, sec: g.container, detail: `${what} ${forced ? 'rendered' : px.hex(g.colour) + (g.colour[3] < 0.99 ? ' @' + g.colour[3].toFixed(2) : '')} on ${px.hex(med.bg)} is ${ratioUsed.toFixed(2)}:1 (needs 3:1; ${many})`, rect });
  }
  return out;
}
// Canvases and SVG charts on screen: blank (one flat colour) or content with no contrast.
async function graphicsCheck(page, img, when) {
  const cvs = await page.evaluate(() => __mx.canvases());
  const out = [];
  const v = page.viewportSize();
  for (const c of cvs) {
    if (!c.vis || !c.inView) continue;
    const [x, y, w, h] = c.rect;
    const X = Math.max(0, x), Y = Math.max(0, y), W = Math.min(v.width, x + w) - X, H = Math.min(v.height, y + h) - Y;
    if (W < 20 || H < 20) continue;
    const st = px.regionStats(img, X, Y, W, H);
    if (st.modalShare >= 0.995 || st.maxContrast < 1.15) out.push({ cat: 'canvas-blank', sel: c.sel, text: c.text, sec: c.sec, detail: `canvas ${Math.round(w)}×${Math.round(h)} shows one flat colour ${st.modal ? px.hex(st.modal) : ''}${when ? ' ' + when : ''}`, rect: c.doc });
    else if (st.maxContrast < 3 && st.modalShare > 0.6) out.push({ cat: 'contrast-nontext', sel: c.sel, text: c.text, sec: c.sec, detail: `canvas content reaches only ${st.maxContrast.toFixed(2)}:1 against its ground ${px.hex(st.modal)}${when ? ' ' + when : ''}`, rect: c.doc });
    // Backing store out of step with the displayed size: stretched or stale after a resize.
    if (c.bw > 0 && c.bh > 0 && c.cw > 0 && c.ch > 0) {
      const ar = (c.bw / c.bh) / (c.cw / c.ch);
      if (Math.abs(ar - 1) > 0.04 || c.bw < c.cw * 0.6) out.push({ cat: 'stale-size', sel: c.sel, text: c.text, sec: c.sec, detail: `canvas backing ${c.bw}×${c.bh} for a ${Math.round(c.cw)}×${Math.round(c.ch)} box${when ? ' ' + when : ''}`, rect: c.doc });
    }
  }
  const svgs = await page.evaluate(() => __mx.svgCharts());
  for (const s of svgs) {
    if (!s.vis || !s.inView) continue;
    const [x, y, w, h] = s.rect;
    const X = Math.max(0, x), Y = Math.max(0, y), W = Math.min(v.width, x + w) - X, H = Math.min(v.height, y + h) - Y;
    if (W < 30 || H < 30) continue;
    const st = px.regionStats(img, X, Y, W, H);
    if (st.modalShare >= 0.997) out.push({ cat: 'svg-blank', sel: s.sel, text: s.text, sec: '-', detail: `chart ${Math.round(w)}×${Math.round(h)} renders as one flat colour${when ? ' ' + when : ''}`, rect: s.doc });
  }
  return out;
}
// Choose viewport windows for pixel sampling: top, end, and everywhere CSS cannot answer
// (text on images/gradients/translucent layers) plus every chart and canvas.
async function sampleWindows(page, extraRects, cap) {
  const info = await page.evaluate(() => ({ H: document.documentElement.scrollHeight, vh: innerHeight, charts: __mx.svgCharts().filter((s) => s.vis).map((s) => s.doc.y), cvs: __mx.canvases().filter((c) => c.vis).map((c) => c.doc.y), legends: [...document.querySelectorAll('[class*="legend"],[class*="swatch"]')].filter((e) => e.getClientRects().length).map((e) => e.getBoundingClientRect().top + scrollY) }));
  const ys = [...extraRects.map((r) => r.y), ...info.charts, ...info.cvs, ...info.legends].filter((y) => y >= 0).sort((a, b) => a - b);
  const wins = [0];
  const end = Math.max(0, info.H - info.vh);
  for (const y of ys) { const top = Math.min(end, Math.max(0, y - 60)); if (!wins.some((w) => y >= w + 20 && y < w + info.vh - 40)) wins.push(top); }
  if (!wins.includes(end)) wins.push(end);
  let chosen = [...new Set(wins)].sort((a, b) => a - b);
  let covered = chosen.length;
  if (chosen.length > cap) { const k = chosen.length / cap; chosen = Array.from({ length: cap }, (_, i) => chosen[Math.floor(i * k)]); chosen[cap - 1] = end; }
  return { windows: chosen, wanted: covered, H: info.H, vh: info.vh };
}
const UNSTABLE = { count: 0 };
const MOVED = { count: 0 };
async function windowPass(job, page, cond, cap, what = { text: true, marks: true, graphics: true, occluded: true }, forced = false) {
  const css = await page.evaluate(() => __mx.contrastCSS());
  const { windows, wanted, H } = await sampleWindows(page, css.unknown, cap);
  const measured = new Set();
  const pixIssues = [];
  for (const y of windows) {
    await page.evaluate((yy) => window.scrollTo(0, yy), y);
    await sleep(120);
    await settleAnims(page, 1500);
    await frames(page, 2);
    const edge = y <= 0 ? 'top' : y >= H - page.viewportSize().height - 2 ? 'end' : '';
    const list = [];
    // Pixel verdicts are taken twice when the first one fails, and only a failure seen both times
    // counts: a page that animates (an auto-playing step player, a typing terminal, a pulse ring, a
    // line fading in) otherwise pairs one frame's colour with another frame's ground.
    const stable = async (fn) => {
      const a = await fn();
      if (!a.length) return a;
      await sleep(450); await frames(page, 2);
      const again = new Set((await fn()).map((x) => x.cat + '|' + x.sel));
      const keep = a.filter((x) => again.has(x.cat + '|' + x.sel));
      UNSTABLE.count += a.length - keep.length;
      return keep;
    };
    if (what.text) {
      const pc = await pixelContrast(page, forced);
      pc.measured.forEach((s) => measured.add(s));
      let first = pc.issues; // the first measurement is the one already taken
      list.push(...(await stable(async () => { if (first) { const f = first; first = null; return f; } return (await pixelContrast(page, forced)).issues; })));
    }
    if (what.marks) list.push(...(await stable(() => markContrast(page, forced))));
    if (what.graphics) list.push(...(await graphicsCheck(page, await vpShot(page), '')));
    if (what.occluded) list.push(...(await page.evaluate((e) => __mx.layout({ scope: 'viewport', edge: e, checks: ['occluded'] }), edge)).issues);
    pixIssues.push(...list);
    await job.add(page, cond, list);
  }
  // CSS verdicts for text never measured by pixels (outside the sampled windows).
  if (what.text && !forced) await job.add(page, cond, css.issues.filter((i) => !measured.has(i.sel)));
  if (wanted > windows.length) NOTES.push(`${job.view.id} ${job.bname} ${cond.condition}: pixel sampling visited ${windows.length} of ${wanted} wanted windows (cap ${cap}); CSS contrast covered the rest`);
  return windows.length;
}

// ---------------------------------------------------------------- condition groups
const THEME_BTN = '#themeBtn, #btn-theme';
const G = {};

// Fixed devices, both orientations: whole-document layout, CSS contrast, touch targets.
G.devices = async (job, kind) => {
  const list = (MODE === 'quick' ? DEVICES.filter((d) => QUICK_DEV.some(([w, h]) => d.w === w && d.h === h)) : DEVICES).filter((d) => (kind === 'touch') === d.touch);
  if (!list.length) return;
  const ctx = await newCtx(job.bname, { viewport: { width: list[0].w, height: list[0].h }, touch: kind === 'touch' });
  try {
    const page = await openView(ctx, job.view, job);
    await scrollThrough(page);
    if (kind !== 'touch') {
      const cond = { condition: 'static', size: sizeOf(page), theme: 'system-light', zoom: '100%', motion: 'no-preference' };
      await job.add(page, cond, await page.evaluate(() => [...__mx.hoverOnly(), ...__mx.colourOnly()]));
      addRuns('static', 1);
      // Page thumbnail for console issues, which have no element.
      const name = String(++cropCount).padStart(5, '0') + '.png';
      await page.screenshot({ path: path.join(CROPS, name), scale: 'css' }).catch(() => {});
      cropBySig.set(job.view.id + '|' + job.bname + '|pagecrop', 'crops/' + name);
    }
    for (const d of list) {
      await resizeTo(page, job.view, d.w, d.h);
      const cond = { condition: `device ${d.cls} ${d.o}`, size: `${d.w}x${d.h}`, theme: 'system-light', zoom: '100%', motion: 'no-preference' };
      const L = await page.evaluate((o) => __mx.layout(o), { scope: 'document', phone: d.phone, checks: ['hscroll', 'outside', 'overlap', 'clipped', 'touch'] });
      const C = await page.evaluate(() => __mx.contrastCSS());
      await job.add(page, cond, [...L.issues, ...C.issues]);
      job.errors(page, cond);
      addRuns('devices', 1);
    }
  } finally { await ctx.close(); }
};

// Continuous width sweep: every SWEEP_STEP px from 280 to 2560 at one height, resized live.
G.sweep = async (job, h) => {
  const ctx = await newCtx(job.bname, { viewport: { width: 280, height: h } });
  try {
    const page = await openView(ctx, job.view, job);
    await scrollThrough(page, 15);
    const hits = new Map();
    for (let w = 280; w <= 2560; w += SWEEP_STEP) {
      await page.setViewportSize({ width: w, height: h });
      await frames(page, 2);
      await settleAnims(page, 600); // breakpoint transitions finish before the page is judged
      if (job.view.studio) await studioIdle(page, 20000);
      const L = await page.evaluate(() => __mx.layout({ scope: 'document', checks: ['hscroll', 'outside', 'overlap', 'clipped'] })).catch((e) => ({ issues: [], err: e.message }));
      addRuns('sweep', 1);
      for (const it of L.issues) {
        const k = [it.cat, it.sel, it.other || ''].join('|');
        let e = hits.get(k);
        if (!e) {
          e = { it, ranges: [] };
          hits.set(k, e);
          // Crop the first appearance while the page is at this width.
          const tmp = new Job(job.bname, job.view, job.group);
          tmp.crops = job.crops;
          await tmp.add(page, {}, [it]);
          job.crops = tmp.crops;
          e.crop = tmp.issues[0].crop;
        }
        const last = e.ranges[e.ranges.length - 1];
        if (last && last[1] === w - SWEEP_STEP) last[1] = w; else e.ranges.push([w, w]);
      }
      if (page.__errors.length) job.errors(page, { condition: `sweep h=${h}`, size: `${w}x${h}`, theme: 'system-light', zoom: '100%', motion: 'no-preference' });
    }
    for (const { it, ranges, crop } of hits.values()) {
      const rs = ranges.map(([a, b]) => (a === b ? `${a}` : `${a}–${b}`)).join(', ');
      const cond = { condition: `sweep h=${h}`, size: `${rs} x${h}`, theme: 'system-light', zoom: '100%', motion: 'no-preference', widths: ranges };
      await job.add(null, cond, [Object.assign({}, it, { detail: it.detail + ` [at widths ${rs}]` })], false);
      job.issues[job.issues.length - 1].crop = crop || null;
    }
  } finally { await ctx.close(); }
};

// Short and tall windows: fixed bars must not hide content, anchors must not land under them.
G.heights = async (job) => {
  const hs = MODE === 'quick' ? [400, 1600] : HEIGHTS;
  const ctx = await newCtx(job.bname, { viewport: { width: 1440, height: 900 } });
  try {
    const page = await openView(ctx, job.view, job);
    await scrollThrough(page);
    const anchors = (await page.evaluate(() => __mx.anchorTargets())).slice(0, MODE === 'quick' ? 8 : 25);
    for (const w of HEIGHT_W) for (const h of hs) {
      await resizeTo(page, job.view, w, h);
      const cond = { condition: 'height', size: `${w}x${h}`, theme: 'system-light', zoom: '100%', motion: 'no-preference' };
      const docH = await page.evaluate(() => document.documentElement.scrollHeight);
      const stops = [0, docH - h, ...[0.25, 0.5, 0.75].map((f) => Math.round(docH * f))].filter((v, i, a) => v >= 0 && a.indexOf(v) === i);
      for (const y of stops) {
        await page.evaluate((yy) => window.scrollTo(0, yy), y);
        await sleep(100);
        await settleAnims(page, 1000); // scroll-triggered crossfades (the home page's story beats) finish first
        const edge = y <= 0 ? 'top' : y >= docH - h - 2 ? 'end' : '';
        const L = await page.evaluate((e) => __mx.layout({ scope: 'viewport', edge: e, checks: ['overlap', 'occluded', 'clipped'] }), edge);
        await job.add(page, cond, L.issues);
      }
      await page.evaluate((yy) => window.scrollTo(0, yy), Math.round(docH * 0.4));
      await sleep(150);
      const cov = await page.evaluate(() => __mx.chromeCoverage());
      if (cov.frac > 0.4) await job.add(page, cond, [{ cat: 'chrome-coverage', sel: cov.who[0] || 'body', text: '', sec: '-', detail: `fixed/sticky chrome covers ${Math.round(cov.frac * 100)}% of a ${h}px-high window (${cov.who.join(', ')})`, rect: { x: 0, y: Math.round(docH * 0.4), w, h } }]);
      const bad = [];
      for (const id of anchors) { const r = await page.evaluate((i) => __mx.checkAnchor(i), id); if (r) bad.push(r); }
      await job.add(page, cond, bad);
      await page.evaluate(() => window.scrollTo(0, 0));
      job.errors(page, cond);
      addRuns('heights', 1);
    }
  } finally { await ctx.close(); }
};

// Live resizing with the page (and a Studio run) open.
G.resize = async (job) => {
  const seq = MODE === 'quick' ? [[1440, 900], [390, 844], [1920, 1080], [1024, 768]] : [[1440, 900], [390, 844], [1024, 768], [1920, 1080], [768, 1024], [2560, 1440], [320, 568], [1280, 800]];
  const ctx = await newCtx(job.bname, { viewport: { width: seq[0][0], height: seq[0][1] } });
  try {
    const page = await openView(ctx, job.view, job);
    await scrollThrough(page);
    if (job.view.studio) {
      // Start a run and resize while it is in flight.
      await page.evaluate(() => { const b = document.getElementById('run'); if (b && !b.disabled) b.click(); });
      await sleep(150);
    }
    let prev = null;
    for (let i = 0; i < seq.length; i++) {
      const [w, h] = seq[i];
      await page.setViewportSize({ width: w, height: h });
      await frames(page, 3);
      await sleep(350);
      if (job.view.studio) await studioIdle(page, 60000);
      const cond = { condition: i === 0 ? 'resize (initial)' : `resize ${prev} -> ${w}x${h}`, size: `${w}x${h}`, theme: 'system-light', zoom: '100%', motion: 'no-preference' };
      const when = i === 0 ? 'before any resize' : `after resizing ${prev} → ${w}x${h}`;
      const list = [];
      // Visit each canvas and chart, since graphics below the fold matter too.
      const targets = await page.evaluate(() => [...__mx.canvases().filter((c) => c.vis).map((c) => c.doc.y), ...__mx.svgCharts().filter((s) => s.vis).slice(0, 8).map((s) => s.doc.y)]);
      const ys = [...new Set(targets.map((y) => Math.max(0, y - 40)))].sort((a, b) => a - b);
      const visited = [];
      for (const y of ys.slice(0, 10)) {
        if (visited.some((v) => y >= v && y < v + h - 120)) continue;
        visited.push(y);
        await page.evaluate((yy) => window.scrollTo(0, yy), y);
        await sleep(200);
        await frames(page, 2);
        list.push(...(await graphicsCheck(page, await vpShot(page), when)));
      }
      const L = await page.evaluate(() => __mx.layout({ scope: 'document', checks: ['hscroll'] }));
      list.push(...L.issues);
      const seen = new Set();
      await job.add(page, cond, list.filter((x) => { const k = x.cat + x.sel; if (seen.has(k)) return false; seen.add(k); return true; }));
      job.errors(page, cond);
      addRuns('resize', 1);
      prev = `${w}x${h}`;
      await page.evaluate(() => window.scrollTo(0, 0));
    }
  } finally { await ctx.close(); }
};

// Browser zoom 50–400 % (CSS viewport = window / zoom at deviceScaleFactor = zoom), the
// 320 CSS px reflow case, text-only 200 %, and WCAG 1.4.12 text spacing.
G.zoom = async (job) => {
  const zs = MODE === 'quick' ? [2, 4] : ZOOMS;
  const runs = zs.map((z) => ({ label: `zoom ${Math.round(z * 100)}%`, zoom: `${Math.round(z * 100)}%`, vp: { width: Math.round(1280 / z), height: Math.round(800 / z) }, dsf: z }));
  runs.push({ label: 'reflow 320 CSS px (1280 at 400%)', zoom: '400%', vp: { width: 320, height: 256 }, dsf: 4 });
  for (const r of runs) {
    const ctx = await newCtx(job.bname, { viewport: r.vp, dsf: r.dsf });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 15);
      const cond = { condition: r.label, size: `${r.vp.width}x${r.vp.height}`, theme: 'system-light', zoom: r.zoom, motion: 'no-preference' };
      const L = await page.evaluate(() => __mx.layout({ scope: 'document', checks: ['hscroll', 'outside', 'overlap', 'clipped'] }));
      await job.add(page, cond, L.issues);
      if (r.dsf >= 2) {
        const docH = await page.evaluate(() => document.documentElement.scrollHeight);
        await page.evaluate((y) => window.scrollTo(0, y), Math.round(docH * 0.4));
        await sleep(150);
        const cov = await page.evaluate(() => __mx.chromeCoverage());
        if (cov.frac > 0.5) await job.add(page, cond, [{ cat: 'chrome-coverage', sel: cov.who[0] || 'body', text: '', sec: '-', detail: `at ${r.zoom} zoom the fixed/sticky chrome covers ${Math.round(cov.frac * 100)}% of the window (${cov.who.join(', ')})`, rect: { x: 0, y: Math.round(docH * 0.4), w: r.vp.width, h: r.vp.height } }]);
      }
      job.errors(page, cond);
      addRuns('zoom', 1);
    } catch (e) {
      if (/deviceScaleFactor/i.test(e.message)) NOTES.push(`${job.bname}: zoom ${r.zoom} unsupported (${e.message.split('\n')[0]})`); else throw e;
    } finally { await ctx.close(); }
  }
};
G.text = async (job) => {
  const runs = [
    { label: 'text-only 200%', vp: [1280, 800], fn: 'scale' },
    { label: 'text spacing (WCAG 1.4.12)', vp: [1280, 800], fn: 'spacing' },
    { label: 'text spacing (WCAG 1.4.12)', vp: [390, 844], fn: 'spacing', touch: true },
  ];
  if (MODE === 'quick') runs.splice(2, 1);
  for (const r of runs) {
    const ctx = await newCtx(job.bname, { viewport: { width: r.vp[0], height: r.vp[1] }, touch: r.touch });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 15);
      if (r.fn === 'scale') await page.evaluate(() => __mx.scaleText(2)); else await page.evaluate(() => __mx.textSpacing());
      await frames(page, 3);
      await sleep(300);
      const cond = { condition: r.label, size: `${r.vp[0]}x${r.vp[1]}`, theme: 'system-light', zoom: r.fn === 'scale' ? 'text 200%' : '100%', motion: 'no-preference' };
      const L = await page.evaluate(() => __mx.layout({ scope: 'document', checks: ['hscroll', 'outside', 'overlap', 'clipped'] }));
      await job.add(page, cond, L.issues);
      job.errors(page, cond);
      addRuns('text', 1);
    } finally { await ctx.close(); }
  }
};

// Fullscreen API on the globe, planets, charts and the Studio: enter, check, exit, check.
G.fullscreen = async (job) => {
  const ctx = await newCtx(job.bname, { viewport: { width: 1440, height: 900 } });
  try {
    const page = await openView(ctx, job.view, job);
    await scrollThrough(page);
    const n = await page.evaluate((studio) => {
      const vis = (e) => e && e.getClientRects().length && __mx.effOpacity(e) > 0.05 && e.getBoundingClientRect().width >= 100;
      const t = [];
      document.querySelectorAll('canvas').forEach((c) => { if (vis(c) && t.length < 3) t.push(c); });
      [...document.querySelectorAll('svg')].filter((s) => (s.getAttribute('role') === 'img' || /chart/.test(s.getAttribute('class') || '')) && vis(s)).slice(0, 2).forEach((s) => t.push(s));
      if (studio) ['results', 'views'].forEach((id) => { const e = document.getElementById(id); if (vis(e) && !t.includes(e)) t.push(e); });
      t.forEach((e, i) => e.setAttribute('data-mx-fs', String(i)));
      return t.length;
    }, !!job.view.studio);
    const enabled = await page.evaluate(() => document.fullscreenEnabled !== false && !!Element.prototype.requestFullscreen);
    if (n && !enabled) { NOTES.push(`${job.bname}: Fullscreen API not available in headless mode; fullscreen checks skipped for ${job.view.id}`); return; }
    for (let i = 0; i < n; i++) {
      const sel = `[data-mx-fs="${i}"]`;
      const d0 = await page.evaluate((s) => { const e = document.querySelector(s); if (!e) return null; e.scrollIntoView({ block: 'center' }); const r = e.getBoundingClientRect(); return { sel: __mx.cssPath(e), text: (e.getAttribute('aria-label') || '').slice(0, 50), w: r.width, h: r.height, tag: e.tagName.toLowerCase() }; }, sel);
      if (!d0) continue; // the page replaced the element since it was listed
      await sleep(200);
      const cond = { condition: `fullscreen ${d0.tag}`, size: '1440x900', theme: 'system-light', zoom: '100%', motion: 'no-preference' };
      // A real click gives the request the user activation browsers require.
      await page.evaluate((s) => { const b = document.createElement('button'); b.id = '__mx_fs'; b.textContent = 'fs'; b.style.cssText = 'position:fixed;left:0;top:0;width:40px;height:40px;z-index:2147483647;opacity:0.01'; b.onclick = () => { window.__mxFsErr = ''; document.querySelector(s).requestFullscreen().catch((e) => { window.__mxFsErr = String(e); }); }; document.body.appendChild(b); }, sel);
      await page.click('#__mx_fs', { timeout: 5000 }).catch(() => {});
      await sleep(900);
      // On a loaded machine the request can take longer to settle either way (Firefox headless
      // rejected after the 900 ms mark and was reported as a failure with no error text).
      await page.waitForFunction((s) => document.fullscreenElement === document.querySelector(s) || !!window.__mxFsErr, sel, { timeout: 3000 }).catch(() => {});
      await page.evaluate(() => document.getElementById('__mx_fs')?.remove());
      const st = await page.evaluate((s) => { const e = document.querySelector(s); if (!e) return { on: false, err: 'element replaced', w: 0, h: 0, vw: innerWidth, vh: innerHeight }; const r = e.getBoundingClientRect(); return { on: document.fullscreenElement === e, err: window.__mxFsErr || '', w: r.width, h: r.height, vw: innerWidth, vh: innerHeight }; }, sel);
      const list = [];
      const rect = { x: 0, y: 0, w: 400, h: 300 };
      if (!st.on) {
        if ((/not supported|denied|permission/i.test(st.err) || !st.err) && job.bname !== 'chromium') NOTES.push(`${job.bname}: fullscreen request refused in headless mode (${st.err.slice(0, 80)}) on ${job.view.id}`);
        else list.push({ cat: 'fullscreen', sel: d0.sel, text: d0.text, sec: '-', detail: `requestFullscreen did not enter full screen ${st.err ? '(' + st.err.slice(0, 100) + ')' : ''}`, rect });
      } else {
        await frames(page, 3);
        const img = await vpShot(page);
        const whole = px.regionStats(img, 0, 0, st.vw, st.vh);
        if (whole.modalShare >= 0.995) list.push({ cat: 'fullscreen', sel: d0.sel, text: d0.text, sec: '-', detail: `in full screen the ${d0.tag} shows one flat colour`, rect });
        if (st.w < st.vw * 0.9 && st.h < st.vh * 0.9) list.push({ cat: 'fullscreen', sel: d0.sel, text: d0.text, sec: '-', detail: `in full screen the ${d0.tag} stays ${Math.round(st.w)}×${Math.round(st.h)} in a ${st.vw}×${st.vh} screen`, rect });
        list.push(...(await graphicsCheck(page, img, 'in full screen')).filter((x) => x.cat !== 'svg-blank'));
        // Record (and crop) while still in full screen, so the evidence shows that state.
        await job.add(page, cond, list.splice(0).map((x) => Object.assign(x, { rect: { x: 0, y: 0, w: st.vw, h: st.vh } })));
        await page.evaluate(() => document.exitFullscreen().catch(() => {}));
        await sleep(700);
        const back = await page.evaluate((s) => { const e = document.querySelector(s); if (!e) return { off: !document.fullscreenElement, w: 0, h: 0, gone: true }; e.scrollIntoView({ block: 'center' }); const r = e.getBoundingClientRect(); return { off: !document.fullscreenElement, w: r.width, h: r.height }; }, sel);
        await sleep(200);
        if (!back.off) list.push({ cat: 'fullscreen', sel: d0.sel, text: d0.text, sec: '-', detail: 'exitFullscreen did not leave full screen', rect });
        else if (Math.abs(back.w - d0.w) > 2 || Math.abs(back.h - d0.h) > 2) list.push({ cat: 'fullscreen', sel: d0.sel, text: d0.text, sec: '-', detail: `after leaving full screen the ${d0.tag} is ${Math.round(back.w)}×${Math.round(back.h)}, was ${Math.round(d0.w)}×${Math.round(d0.h)}`, rect });
        list.push(...(await graphicsCheck(page, await vpShot(page), 'after leaving full screen')));
      }
      const seen = new Set();
      await job.add(page, cond, list.filter((x) => { const k = x.cat + x.sel + x.detail.slice(0, 20); if (seen.has(k)) return false; seen.add(k); return true; }));
      job.errors(page, cond);
      addRuns('fullscreen', 2);
    }
  } finally { await ctx.close(); }
};

// All theme states at one size: pixel-sampled text contrast, non-text contrast, blank
// graphics, fixed-bar occlusion. At 1440 wide the theme toggle is also exercised mid-page.
G.themes = async (job, sz) => {
  const [w, h] = sz;
  const touch = w < 700;
  const cap = MODE === 'quick' ? 6 : 14;
  for (const [state, t] of Object.entries(THEME_STATES)) {
    const ctx = await newCtx(job.bname, { viewport: { width: w, height: h }, colorScheme: t.colorScheme, stamp: t.stamp, touch });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 20);
      const cond = { condition: `theme ${state}`, size: `${w}x${h}`, theme: state, zoom: '100%', motion: 'no-preference' };
      const ts = await page.evaluate(() => __mx.themeState());
      const wantDark = t.stamp ? t.stamp === 'dark' : t.colorScheme === 'dark';
      const bg = px.lum(mxParse(ts.bg) || [255, 255, 255]);
      if ((wantDark && bg > 0.4) || (!wantDark && bg < 0.2)) await job.add(page, cond, [{ cat: 'theme-state', sel: 'body', text: '', sec: '-', detail: `expected a ${wantDark ? 'dark' : 'light'} page for ${state}, body background is ${ts.bg} (data-theme="${ts.stamp}")`, rect: { x: 0, y: 0, w: 300, h: 200 } }]);
      await windowPass(job, page, cond, cap);
      job.errors(page, cond);
      addRuns('themes', 1);
    } finally { await ctx.close(); }
  }
  if (w < 1000) return;
  for (const start of ['system-light', 'system-dark']) {
    const t = THEME_STATES[start];
    const ctx = await newCtx(job.bname, { viewport: { width: w, height: h }, colorScheme: t.colorScheme });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 20);
      const hasBtn = await page.evaluate((s) => !!document.querySelector(s), THEME_BTN);
      if (!hasBtn) { if (!job.view.id.startsWith('fixture')) NOTES.push(`${job.view.id}: no theme toggle found`); await ctx.close(); continue; }
      const docH = await page.evaluate(() => document.documentElement.scrollHeight);
      await page.evaluate((y) => window.scrollTo(0, y), Math.round(docH * 0.4));
      await sleep(200);
      for (const dir of [start === 'system-light' ? 'light->dark' : 'dark->light', start === 'system-light' ? 'dark->light' : 'light->dark']) {
        const before = await page.evaluate(() => __mx.themeState());
        await page.evaluate((s) => { const b = document.querySelector(s); b.click(); }, THEME_BTN);
        await sleep(600);
        await frames(page, 3);
        const after = await page.evaluate(() => __mx.themeState());
        const cond = { condition: `theme toggle ${dir} mid-page`, size: `${w}x${h}`, theme: dir.split('->')[1] + ' (toggled)', zoom: '100%', motion: 'no-preference' };
        const l0 = px.lum(mxParse(before.bg) || [255, 255, 255]), l1 = px.lum(mxParse(after.bg) || [255, 255, 255]);
        const wantDark = dir.endsWith('dark');
        if ((wantDark && !(l1 < l0 - 0.2)) || (!wantDark && !(l1 > l0 + 0.2))) await job.add(page, cond, [{ cat: 'theme-toggle', sel: THEME_BTN, text: '', sec: '-', detail: `toggle ${dir}: body background went ${before.bg} → ${after.bg}`, rect: { x: 0, y: Math.round(docH * 0.4), w: 300, h: 200 } }]);
        // Grade the viewport the reader is looking at, plus the charts, in the new theme.
        const y0 = await page.evaluate(() => scrollY);
        const list = [];
        const pc = await pixelContrast(page);
        list.push(...pc.issues, ...(await markContrast(page)), ...(await graphicsCheck(page, await vpShot(page), 'after the theme toggle')));
        await job.add(page, cond, list);
        await windowPass(job, page, cond, Math.max(3, Math.floor(cap / 2)), { text: false, marks: true, graphics: true, occluded: false });
        await page.evaluate((y) => window.scrollTo(0, y), y0);
        job.errors(page, cond);
        addRuns('themes', 1);
      }
    } finally { await ctx.close().catch(() => {}); }
  }
};
function mxParse(c) { const m = c && c.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number); if (p.length > 3 && p[3] === 0) return null; return p.slice(0, 3); }

// Colour-vision simulation: chart palettes must stay distinguishable (CIEDE2000) under
// protanopia, deuteranopia and tritanopia unless the series differ by more than colour.
const DE_MIN = { normal: 10, protanopia: 8, deuteranopia: 8, tritanopia: 8 };
G.cvd = async (job) => {
  for (const scheme of ['light', 'dark']) {
    const ctx = await newCtx(job.bname, { viewport: { width: 1440, height: 900 }, colorScheme: scheme });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 20);
      const pal = await page.evaluate(() => __mx.palettes());
      const bgc = mxParse((await page.evaluate(() => __mx.themeState())).bg) || [255, 255, 255];
      await page.evaluate((svg) => { document.body.insertAdjacentHTML('beforeend', svg); }, px.filterSvg());
      for (const vision of ['normal', ...CVDS]) {
        const cond = { condition: vision === 'normal' ? 'palette (normal vision)' : `colour vision ${vision}`, size: '1440x900', theme: 'system-' + scheme, zoom: '100%', motion: 'no-preference', vision };
        const list = [];
        for (const p of pal) {
          const cols = [];
          for (const s of [...p.series.map((x) => ({ ...x, src: 'series' })), ...p.swatches.map((x) => ({ ...x, src: 'legend' }))]) {
            const flat = px.blend(s.c, bgc);
            if (cols.some((c) => px.de2000(c.flat, flat) < 2)) continue;
            cols.push({ flat, dash: s.dash, shape: s.shape, src: s.src, orig: s.c });
          }
          for (let i = 0; i < cols.length; i++) for (let j = i + 1; j < cols.length; j++) {
            const A = cols[i], B = cols[j];
            if (A.src === B.src && (A.dash !== B.dash || A.shape !== B.shape)) continue; // told apart by more than colour
            const a = px.simulate(A.flat, vision), b = px.simulate(B.flat, vision);
            const de = px.de2000(a, b);
            if (de < DE_MIN[vision]) list.push({ cat: vision === 'normal' ? 'palette' : 'cvd', sel: p.legend && p.series.length === 0 ? p.legend : p.sel, text: p.text, sec: '-', detail: `${px.hex(A.flat)} and ${px.hex(B.flat)} are ΔE2000 ${de.toFixed(1)} apart${vision === 'normal' ? '' : ' under ' + vision} (needs ≥ ${DE_MIN[vision]}; simulated ${px.hex(a)} vs ${px.hex(b)})`, rect: p.rect });
          }
        }
        const seen = new Set();
        const uniq = list.filter((x) => { const k = x.sel + x.detail.slice(0, 20); if (seen.has(k)) return false; seen.add(k); return true; });
        // Render the page through the same matrix for the crop and the evidence shot.
        if (vision !== 'normal') await page.evaluate((v) => { document.documentElement.style.filter = `url(#mx-${v})`; }, vision);
        await frames(page, 2);
        await job.add(page, cond, uniq);
        if (vision !== 'normal' && pal.length && scheme === 'light') {
          const r = pal[0].rect;
          await page.evaluate((y) => window.scrollTo(0, y), Math.max(0, r.y - 80));
          await sleep(120);
          const dir = path.join(SHOTS, 'cvd');
          fs.mkdirSync(dir, { recursive: true });
          await page.screenshot({ path: path.join(dir, `${job.view.id.replace(/[/?]/g, '_')}-${job.bname}-${vision}.png`), scale: 'css' }).catch(() => {});
        }
        await page.evaluate(() => { document.documentElement.style.filter = ''; });
        addRuns('cvd', 1);
      }
      job.errors(page, { condition: 'colour vision', size: '1440x900', theme: 'system-' + scheme, zoom: '100%', motion: 'no-preference' });
    } finally { await ctx.close(); }
  }
};

// Forced colours (Windows high-contrast), Chromium only.
G.forced = async (job) => {
  for (const scheme of ['dark', 'light']) {
    const ctx = await newCtx(job.bname, { viewport: { width: 1440, height: 900 }, colorScheme: scheme, forcedColors: 'active' });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 20);
      const cond = { condition: `forced-colors active (${scheme})`, size: '1440x900', theme: `forced-${scheme}`, zoom: '100%', motion: 'no-preference' };
      await windowPass(job, page, cond, MODE === 'quick' ? 4 : 8, undefined, true);
      await keyboardPass(job, page, cond, MODE === 'quick' ? 20 : 40);
      job.errors(page, cond);
      addRuns('forced', 1);
    } finally { await ctx.close(); }
  }
};

// Reduced motion on and off: text visible in one state must be visible in the other.
G.motion = async (job) => {
  const sizes = MODE === 'quick' ? [[1440, 900]] : [[1440, 900], [390, 844]];
  for (const [w, h] of sizes) {
    const sets = {};
    const pages = {};
    const ctxs = [];
    try {
      for (const rm of ['no-preference', 'reduce']) {
        const ctx = await newCtx(job.bname, { viewport: { width: w, height: h }, reducedMotion: rm, touch: w < 700 });
        ctxs.push(ctx);
        const page = await openView(ctx, job.view, job);
        await scrollThrough(page, 80);
        await sleep(400);
        sets[rm] = await page.evaluate(() => __mx.visibleTexts());
        pages[rm] = page;
        const cond = { condition: `motion ${rm}`, size: `${w}x${h}`, theme: 'system-light', zoom: '100%', motion: rm };
        const L = await page.evaluate(() => __mx.layout({ scope: 'document', checks: ['hscroll', 'overlap', 'clipped'] }));
        await job.add(page, cond, L.issues);
        job.errors(page, cond);
        addRuns('motion', 1);
      }
      for (const [on, off] of [['no-preference', 'reduce'], ['reduce', 'no-preference']]) {
        const list = [];
        for (const [k, v] of Object.entries(sets[on])) {
          const o = sets[off][k];
          if (v.vis && o && !o.vis) list.push({ cat: 'motion-hidden', sel: o.sel, text: o.text, sec: '-', detail: off === 'reduce' ? 'visible with motion, hidden when prefers-reduced-motion: reduce' : 'visible with reduced motion, still hidden after its animation with motion on', rect: o.rect });
        }
        const cond = { condition: `motion ${off} vs ${on}`, size: `${w}x${h}`, theme: 'system-light', zoom: '100%', motion: off };
        await job.add(pages[off], cond, list.slice(0, 60));
      }
    } finally { for (const c of ctxs) await c.close(); }
  }
};

// Keyboard only: every Tab stop visibly focused (pixel diff against the unfocused state,
// indicator ≥ 3:1 against what it replaced), not hidden behind fixed bars, and no traps.
async function keyboardPass(job, page, cond, cap) {
  await page.evaluate(() => { window.scrollTo(0, 0); if (document.activeElement) document.activeElement.blur(); });
  await page.mouse.move(1, 1).catch(() => {});
  const total = await page.evaluate(() => __mx.focusables());
  const key = job.bname === 'webkit' ? 'Alt+Tab' : 'Tab';
  const seq = [];
  const list = [];
  let wraps = 0, visited = new Set(), stuck = 0, lastKey = null;
  const n = Math.min(cap, total + 3);
  for (let i = 0; i < n; i++) {
    await page.keyboard.press(key);
    await frames(page, 2);
    await sleep(30);
    const f = await page.evaluate(() => __mx.focusInfo());
    if (f.none) { wraps++; if (visited.size) break; continue; }
    if (f.key === lastKey && f.isLast) break; // end of the page: focus went to the browser UI
    if (f.key === lastKey) { stuck++; if (stuck >= 2) { list.push({ cat: 'focus-trap', sel: f.sel, text: f.text, sec: f.sec, detail: 'Tab does not move focus away from this element', rect: f.doc }); break; } continue; }
    stuck = 0;
    lastKey = f.key;
    if (visited.has(f.key)) {
      // Back at an element already visited without leaving the page: a cycle. Only a trap if
      // most of the page's focusable elements were never reached.
      if (visited.size < total * 0.8 && total - visited.size > 3) list.push({ cat: 'focus-trap', sel: f.sel, text: f.text, sec: f.sec, detail: `focus cycles back here after ${visited.size} of ${total} focusable elements`, rect: f.doc });
      break;
    }
    visited.add(f.key);
    seq.push(f.sel);
    if (!f.visible) { list.push({ cat: 'focus-visible', sel: f.sel, text: f.text, sec: f.sec, detail: 'receives keyboard focus while invisible (zero size, hidden or transparent)', rect: f.doc }); continue; }
    if (!f.inView) { list.push({ cat: 'focus-visible', sel: f.sel, text: f.text, sec: f.sec, detail: 'focused element is outside the viewport and was not scrolled into view', rect: f.doc }); continue; }
    if (f.obscured) { list.push({ cat: 'focus-obscured', sel: f.sel, text: f.text, sec: f.sec, detail: `focused element entirely hidden behind ${f.coveredBy} (WCAG 2.4.11)`, rect: f.doc }); continue; }
    const v = page.viewportSize();
    const [x, y, w, h] = f.rect;
    const X = Math.max(0, Math.floor(x - 8)), Y = Math.max(0, Math.floor(y - 8));
    const W = Math.min(v.width, Math.ceil(x + w + 8)) - X, H = Math.min(v.height, Math.ceil(y + h + 8)) - Y;
    if (W < 4 || H < 4) continue;
    const A = await shot(page, { x: X, y: Y, width: W, height: H });
    await page.evaluate(() => { const a = document.activeElement; window.__mxLast = a; a.blur(); });
    await frames(page, 2);
    await sleep(30);
    const B = await shot(page, { x: X, y: Y, width: W, height: H });
    await page.evaluate(() => { const a = window.__mxLast; if (a) a.focus({ preventScroll: true }); });
    let changed = 0, strong = 0, best = 1;
    for (let yy = 0; yy < Math.min(A.h, B.h); yy++) for (let xx = 0; xx < Math.min(A.w, B.w); xx++) {
      const i = (yy * A.w + xx) * 4, j = (yy * B.w + xx) * 4;
      const a = [A.d[i], A.d[i + 1], A.d[i + 2]], b = [B.d[j], B.d[j + 1], B.d[j + 2]];
      if (px.diff(a, b) > 24) { changed++; const r = px.ratio(a, b); if (r >= 3) strong++; if (r > best) best = r; }
    }
    const perim = 2 * (w + h);
    if (changed < Math.min(perim * 0.5, 40)) list.push({ cat: 'focus-visible', sel: f.sel, text: f.text, sec: f.sec, detail: `no visible focus indicator (${changed} px change on focus; outline ${f.outline})`, rect: f.doc });
    else if (strong < Math.min(perim * 0.5, 40)) list.push({ cat: 'focus-contrast', sel: f.sel, text: f.text, sec: f.sec, detail: `focus indicator too faint: best ${best.toFixed(2)}:1 against the unfocused state (needs 3:1 over ≥ ${Math.round(Math.min(perim * 0.5, 40))} px)`, rect: f.doc });
  }
  await job.add(page, cond, list);
  return seq.length;
}
G.keyboard = async (job) => {
  for (const scheme of MODE === 'quick' ? ['light'] : ['light', 'dark']) {
    const ctx = await newCtx(job.bname, { viewport: { width: 1440, height: 900 }, colorScheme: scheme });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 15);
      const cond = { condition: 'keyboard only', size: '1440x900', theme: 'system-' + scheme, zoom: '100%', motion: 'no-preference' };
      await keyboardPass(job, page, cond, MODE === 'quick' ? 40 : 160);
      job.errors(page, cond);
      addRuns('keyboard', 1);
    } finally { await ctx.close(); }
  }
};

// Evidence: every page at the key sizes in both themes (full page, first 16000 px).
G.shots = async (job) => {
  const dir = path.join(SHOTS, job.view.id.replace(/[/?]/g, '_'));
  fs.mkdirSync(dir, { recursive: true });
  for (const [w, h] of KEY_SIZES) for (const scheme of ['light', 'dark']) {
    const ctx = await newCtx(job.bname, { viewport: { width: w, height: h }, colorScheme: scheme, touch: w < 1000 });
    try {
      const page = await openView(ctx, job.view, job);
      await scrollThrough(page, 20);
      const docH = await page.evaluate(() => document.documentElement.scrollHeight);
      await page.screenshot({ path: path.join(dir, `${job.bname}-${w}x${h}-${scheme}.jpg`), type: 'jpeg', quality: 60, scale: 'css', fullPage: true, clip: { x: 0, y: 0, width: w, height: Math.min(docH, 16000) }, timeout: 60000 }).catch(async () => {
        await page.screenshot({ path: path.join(dir, `${job.bname}-${w}x${h}-${scheme}.jpg`), type: 'jpeg', quality: 60, scale: 'css' }).catch(() => {});
      });
    } finally { await ctx.close(); }
  }
};

// ---------------------------------------------------------------- plan and run
function plan() {
  const jobs = [];
  const want = (g) => !GROUP_FILTER || GROUP_FILTER.split(',').includes(g);
  for (const bname of BROWSERS) for (const view of VIEWS) {
    const J = (group, label, fn) => { if (want(group)) jobs.push({ bname, view, group, label, fn }); };
    J('devices', 'devices desktop', (j) => G.devices(j, 'desktop'));
    J('devices', 'devices touch', (j) => G.devices(j, 'touch'));
    J('sweep', 'sweep h=900', (j) => G.sweep(j, 900));
    J('sweep', 'sweep h=500', (j) => G.sweep(j, 500));
    J('heights', 'heights', (j) => G.heights(j));
    J('resize', 'resize', (j) => G.resize(j));
    J('zoom', 'zoom', (j) => G.zoom(j));
    J('text', 'text', (j) => G.text(j));
    J('fullscreen', 'fullscreen', (j) => G.fullscreen(j));
    J('themes', 'themes 1440x900', (j) => G.themes(j, [1440, 900]));
    J('themes', 'themes 390x844', (j) => G.themes(j, [390, 844]));
    J('cvd', 'cvd', (j) => G.cvd(j));
    if (bname === 'chromium') J('forced', 'forced-colors', (j) => G.forced(j));
    J('motion', 'motion', (j) => G.motion(j));
    J('keyboard', 'keyboard', (j) => G.keyboard(j));
    if (bname === 'chromium' && !flag('no-shots') && MODE !== 'fixtures') J('shots', 'screenshots', (j) => G.shots(j));
  }
  // Long jobs first so the tail is short; interleave engines.
  const weight = { sweep: 10, themes: 6, zoom: 5, devices: 5, keyboard: 4, motion: 3, heights: 3, forced: 3, shots: 3, resize: 2, cvd: 2, text: 2, fullscreen: 1 };
  const pageWeight = (v) => (/changelog|capability-reference|validation|overview|scenarios|verification|scenario-reference|capability-map|modelled/.test(v.id) ? 3 : v.studio ? 2 : 1);
  jobs.sort((a, b) => weight[b.group] * pageWeight(b.view) - weight[a.group] * pageWeight(a.view));
  return jobs;
}

async function runJobs(jobs) {
  let next = 0, done = 0;
  const t0 = Date.now();
  const timeoutMs = (Number(opt('job-timeout', 0)) || (MODE === 'quick' ? 12 : 40)) * 60e3; // --job-timeout=MIN for a loaded machine
  async function worker(wid) {
    while (next < jobs.length) {
      const spec = jobs[next++];
      const job = new Job(spec.bname, spec.view, spec.group, spec.label);
      const t = Date.now();
      let timer;
      try {
        await Promise.race([spec.fn(job), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('job timed out after ' + timeoutMs / 60e3 + ' min')), timeoutMs); })]);
      } catch (e) {
        TOOL_ERRORS.push({ page: spec.view.id, browser: spec.bname, group: spec.group, label: spec.label, error: String(e && e.message || e).split('\n').slice(0, 3).join(' | ') });
        log(`  ! ${spec.bname} ${spec.view.id} ${spec.label}: ${String(e.message || e).split('\n')[0]}`);
      } finally { clearTimeout(timer); }
      ALL_ISSUES.push(...job.issues);
      done++;
      const el = (Date.now() - t0) / 1000;
      log(`[${done}/${jobs.length}] w${wid} ${spec.bname} ${spec.view.id} ${spec.label}: ${job.issues.length} findings, ${((Date.now() - t) / 1000).toFixed(0)} s (elapsed ${(el / 60).toFixed(1)} min, eta ${((el / done) * (jobs.length - done) / 60).toFixed(0)} min)`);
    }
  }
  await Promise.all(Array.from({ length: Math.min(CONC, jobs.length) }, (_, i) => worker(i + 1)));
}

// iOS Simulator Safari smoke run: open each main page in the booted simulator, screenshot,
// check it rendered. No DOM access, so this is a smoke test, not a matrix run.
async function iosSmoke() {
  const res = { tried: false, device: null, results: [], note: '' };
  if (flag('no-ios') || MODE === 'fixtures') { res.note = 'skipped by option'; return res; }
  let devs;
  try { devs = JSON.parse(execFileSync('xcrun', ['simctl', 'list', 'devices', 'available', '-j'], { encoding: 'utf8', timeout: 30000 })); }
  catch (e) { res.note = 'xcrun simctl unavailable: WebKit mobile emulation only (touch contexts in the WebKit runs)'; return res; }
  const all = Object.entries(devs.devices).filter(([rt]) => /iOS/.test(rt)).flatMap(([rt, ds]) => ds.map((d) => ({ ...d, rt })));
  const d = all.find((x) => x.name === 'iPhone 17 Pro' && x.state === 'Booted') || all.find((x) => x.name === 'iPhone 17 Pro') || all.find((x) => /iPhone/.test(x.name));
  if (!d) { res.note = 'no iPhone simulator: WebKit mobile emulation only'; return res; }
  res.tried = true;
  res.device = `${d.name} (${d.rt.split('.').pop()})`;
  const booted = d.state === 'Booted';
  try {
    if (!booted) { spawnSync('xcrun', ['simctl', 'boot', d.udid], { timeout: 120000 }); spawnSync('xcrun', ['simctl', 'bootstatus', d.udid, '-b'], { timeout: 240000 }); }
    const dir = path.join(SHOTS, 'ios-simulator');
    fs.mkdirSync(dir, { recursive: true });
    await sleep(8000);
    // Reference: the home screen before Safari opens. A page shot that matches it, or matches the
    // previous page, means Safari never showed the page.
    const homeF = path.join(dir, '_home.png');
    spawnSync('xcrun', ['simctl', 'io', d.udid, 'screenshot', homeF], { timeout: 30000 });
    let home = null, prev = null;
    try { home = px.decode(fs.readFileSync(homeF)); } catch { /* no reference */ }
    const pages = VIEWS.filter((v) => !v.id.startsWith('docs/') || v.id === 'docs/index').filter((v) => !v.studio || v.id === 'studio');
    for (const v of pages) {
      spawnSync('xcrun', ['simctl', 'openurl', d.udid, BASE + v.path], { timeout: 30000 });
      await sleep(v.studio ? 40000 : 25000); // the simulator is slow on a loaded machine
      const f = path.join(dir, v.id.replace(/[/?]/g, '_') + '.png');
      spawnSync('xcrun', ['simctl', 'io', d.udid, 'screenshot', f], { timeout: 30000 });
      let ok = false, detail = '';
      try {
        const img = px.decode(fs.readFileSync(f));
        const st = px.regionStats(img, 0, img.h * 0.15, img.w, img.h * 0.7);
        const dHome = home ? px.meanDiff(img, home) : 255, dPrev = prev ? px.meanDiff(img, prev) : 255;
        ok = st.modalShare < 0.97 && st.distinct > 3 && dHome > 15 && dPrev > 4;
        detail = `modal colour share ${(st.modalShare * 100).toFixed(1)}%, ${st.distinct} colour clusters, differs from home screen by ${dHome.toFixed(0)} and from the previous page by ${dPrev.toFixed(0)}`;
        prev = img;
      } catch (e) { detail = 'no screenshot: ' + e.message; }
      res.results.push({ page: v.id, ok, shot: path.relative(OUT, f), detail });
      if (!ok) ALL_ISSUES.push({ page: v.id, view: v.path, browser: 'ios-simulator-safari', group: 'ios', condition: 'iOS Simulator Safari smoke', size: d.name, theme: 'system', zoom: '100%', motion: 'system', category: 'ios-smoke', selector: 'page', text: '', detail: 'page did not render in the iOS Simulator (' + detail + ')', crop: path.relative(OUT, f), sig: v.id + '|ios|smoke' });
    }
  } finally { if (!booted) spawnSync('xcrun', ['simctl', 'shutdown', d.udid], { timeout: 60000 }); }
  return res;
}

// ---------------------------------------------------------------- report
function report(meta) {
  const uniq = new Map();
  for (const i of ALL_ISSUES) { const k = i.sig; if (!uniq.has(k)) uniq.set(k, { ...i, occurrences: 0, conditions: new Set() }); const u = uniq.get(k); u.occurrences += i._w || 1; for (const c of i._conds || [`${i.condition} @ ${i.size}`]) u.conditions.add(c); }
  const U = [...uniq.values()].map((u) => ({ ...u, conditions: [...u.conditions].slice(0, 12), nConditions: u.conditions.size }));
  const byCat = {}, occCat = {}, byBrowser = {}, byGroup = {}, byPage = {};
  for (const u of U) { byCat[u.category] = (byCat[u.category] || 0) + 1; byBrowser[u.browser] = (byBrowser[u.browser] || 0) + 1; byPage[u.page] = (byPage[u.page] || 0) + 1; }
  for (const i of ALL_ISSUES) { occCat[i.category] = (occCat[i.category] || 0) + (i._w || 1); byGroup[i.group] = (byGroup[i.group] || 0) + (i._w || 1); }
  const occTotal = ALL_ISSUES.reduce((a, i) => a + (i._w || 1), 0);
  const norm = (u) => `${u.category}: ${u.detail.replace(/\[at widths[^\]]*\]/, '').replace(/#[0-9a-f]{6}/gi, '#…').replace(/rgb\([^)]*\)/g, 'rgb(…)').replace(/\d+(\.\d+)?/g, 'N').replace(/"[^"]*"/g, '"…"').replace(/\([^)]*\)/g, '(…)').slice(0, 110)}`;
  const types = {};
  for (const u of U) { const k = norm(u); types[k] = types[k] || { n: 0, pages: new Set(), ex: u }; types[k].n++; types[k].pages.add(u.page); }
  const top = Object.entries(types).sort((a, b) => b[1].n - a[1].n).slice(0, 10).map(([k, v]) => ({ type: k, unique: v.n, pages: v.pages.size, example: `${v.ex.page} ${v.ex.browser} ${v.ex.selector} — ${v.ex.detail}`.slice(0, 220) }));
  const runs = Object.values(RUNS).reduce((a, b) => a + b, 0);
  NOTES.push(`web fonts: ${FONT_STATS.fetched} responses fetched from fonts.googleapis.com / fonts.gstatic.com, ${FONT_STATS.replayed} replayed from the run's cache, ${FONT_STATS.failed} could not be fetched in four tries`);
  if (MOVED.count) NOTES.push(`mark contrast: ${MOVED.count} mark groups moved between the two captures (replay dots, pulse rings) and were not judged`);
  if (UNSTABLE.count) NOTES.push(`pixel contrast: ${UNSTABLE.count} first-capture failures were not seen again 450 ms later (animated states) and are not counted`);
  const json = { meta: { ...meta, matrixRuns: runs, runsByGroup: RUNS, uniqueIssues: U.length, occurrences: occTotal, byCategory: byCat, occurrencesByCategory: occCat, byBrowser, byGroup, byPage, topTypes: top }, toolErrors: TOOL_ERRORS, notes: [...new Set(NOTES)], issues: U.map(({ sig, _w, _conds, ...r }) => r) };
  fs.writeFileSync(path.join(OUT, 'report.json'), JSON.stringify(json, null, 1));
  const L = [];
  L.push(`# Display matrix report (${meta.mode})`, '');
  L.push(`- Site: \`${meta.site}\`, ${meta.views} page views, engines: ${meta.browsers.join(', ')}`);
  L.push(`- Started ${meta.started}, took ${(meta.durationSec / 60).toFixed(1)} min with ${CONC} parallel browser contexts`);
  if (meta.merged) L.push(`- Merged: ${meta.merged}`);
  L.push(`- Matrix size: **${runs.toLocaleString('en')} page×condition runs** (${Object.entries(RUNS).map(([k, v]) => `${k} ${v.toLocaleString('en')}`).join(', ')})`);
  L.push(`- Result: **${U.length.toLocaleString('en')} unique issues** (page + engine + category + element), ${occTotal.toLocaleString('en')} occurrences across conditions; ${TOOL_ERRORS.length} tool errors`);
  if (meta.ios) L.push(`- iOS: ${meta.ios.tried ? `iOS Simulator Safari smoke on ${meta.ios.device}: ${meta.ios.results.filter((r) => r.ok).length}/${meta.ios.results.length} pages rendered (screenshots in shots/ios-simulator)` : meta.ios.note}`);
  L.push('', '## Issues per category', '', '| Category | Unique | Occurrences |', '|---|---:|---:|');
  for (const [k, v] of Object.entries(byCat).sort((a, b) => b[1] - a[1])) L.push(`| ${k} | ${v} | ${occCat[k]} |`);
  if (!U.length) L.push('| (none) | 0 | 0 |');
  L.push('', '## Per engine', '', '| Engine | Unique issues |', '|---|---:|');
  for (const [k, v] of Object.entries(byBrowser)) L.push(`| ${k} | ${v} |`);
  L.push('', '## Occurrences per condition group', '', '| Group | Runs | Occurrences |', '|---|---:|---:|');
  for (const g of Object.keys({ ...RUNS, ...byGroup })) L.push(`| ${g} | ${RUNS[g] || 0} | ${byGroup[g] || 0} |`);
  L.push('', '## Top 10 issue types', '', '| # | Type | Unique | Pages | Example |', '|---:|---|---:|---:|---|');
  top.forEach((t, i) => L.push(`| ${i + 1} | ${t.type.replace(/\|/g, '\\|')} | ${t.unique} | ${t.pages} | ${t.example.replace(/\|/g, '\\|')} |`));
  L.push('', '## Pages with most issues', '', '| Page | Unique issues |', '|---|---:|');
  for (const [k, v] of Object.entries(byPage).sort((a, b) => b[1] - a[1]).slice(0, 25)) L.push(`| ${k} | ${v} |`);
  if (meta.fixtures) {
    L.push('', '## Mutation fixtures', '', '| Fixture | Injected defect | Expected category | Result | Evidence |', '|---|---|---|---|---|');
    for (const f of meta.fixtures) L.push(`| ${f.id} | ${f.why} | ${f.expect.join(', ') || '(none: control)'} | ${f.caught ? '**CAUGHT**' : f.control ? (f.pass ? '**PASS (0 issues)**' : `**FAIL (${f.count} issues)**`) : '**MISSED**'} | ${(f.evidence || '').replace(/\|/g, '\\|').slice(0, 200)} |`);
  }
  if (TOOL_ERRORS.length) { L.push('', '## Tool errors', ''); for (const t of TOOL_ERRORS.slice(0, 50)) L.push(`- ${t.browser} ${t.page} ${t.label}: ${t.error}`); }
  const notes = [...new Set(NOTES)];
  if (notes.length) { L.push('', '## Notes', ''); for (const n of notes.slice(0, 80)) L.push(`- ${n}`); if (notes.length > 80) L.push(`- … ${notes.length - 80} more in report.json`); }
  L.push('', '## Issues (first 400 unique, most widespread first)', '', '| Page | Engine | Category | Condition @ size | Theme | Zoom | Element | Detail | Crop |', '|---|---|---|---|---|---|---|---|---|');
  const esc = (s) => String(s || '').replace(/\|/g, '\\|').replace(/\n/g, ' ');
  for (const u of U.sort((a, b) => b.nConditions - a.nConditions).slice(0, 400)) L.push(`| ${esc(u.page)} | ${u.browser} | ${u.category} | ${esc(u.condition)} @ ${esc(u.size)}${u.nConditions > 1 ? ` (+${u.nConditions - 1} more)` : ''} | ${u.theme} | ${u.zoom} | \`${esc(u.selector).slice(0, 90)}\`${u.text ? ' "' + esc(u.text).slice(0, 30) + '"' : ''} | ${esc(u.detail).slice(0, 180)} | ${u.crop ? `[crop](${u.crop})` : '-'} |`);
  L.push('', `Screenshots: \`shots/<page>/chromium-<size>-<theme>.jpg\` (every page at ${KEY_SIZES.map(([w, h]) => `${w}×${h}`).join(', ')} in light and dark); colour-vision renders in \`shots/cvd/\`.`);
  fs.writeFileSync(path.join(OUT, 'report.md'), L.join('\n') + '\n');
  return json;
}

// ---------------------------------------------------------------- main
const t0 = Date.now();
const started = new Date().toISOString();
const jobs = plan();
log(`display matrix ${MODE}: ${VIEWS.length} views × ${BROWSERS.join('/')} → ${jobs.length} jobs, ${CONC} at a time, server ${BASE}, out ${OUT}`);
await runJobs(jobs);
for (const b of Object.values(launched)) { try { (await b).close(); } catch { /* closed */ } }
const ios = MODE === 'full' || flag('ios') ? await iosSmoke() : { tried: false, note: 'not run in this mode (use --full or --ios)' };
let fixtures = null;
if (MODE === 'fixtures') {
  fixtures = VIEWS.map((v) => {
    const mine = ALL_ISSUES.filter((i) => i.page === v.id);
    if (!v.expect.length) return { id: v.id, why: v.why || 'clean control', expect: [], control: true, pass: mine.length === 0, count: mine.length, evidence: mine.slice(0, 3).map((i) => `${i.category} ${i.selector} ${i.detail}`).join(' ; ') };
    const hit = mine.filter((i) => v.expect.includes(i.category) && (!v.target || [i.selector, i.other, i.detail].join(' ').includes(v.target)));
    return { id: v.id, why: v.why, expect: v.expect, target: v.target, caught: hit.length > 0, evidence: hit.length ? `${hit[0].category} @ ${hit[0].condition} ${hit[0].size} ${hit[0].theme}: ${hit[0].selector} — ${hit[0].detail}` : `(no ${v.expect.join('/')} issue on ${v.target}; ${mine.length} other findings)`, other: mine.length - hit.length };
  });
}
// --merge-base=DIR --merge-drop=g1,g2: keep an earlier run's issues and runs except the dropped
// groups (which this run re-measured), so one group can be re-run after a detector fix.
let merged = null;
const MB = opt('merge-base', '');
if (MB) {
  const base = JSON.parse(fs.readFileSync(path.join(path.resolve(MB), 'report.json'), 'utf8'));
  const drop = new Set(opt('merge-drop', '').split(',').filter(Boolean));
  for (const i of base.issues) {
    if (drop.has(i.group)) continue;
    ALL_ISSUES.push({ ...i, sig: [i.page, i.browser, i.category, i.selector, i.other || ''].join('|') + (i.category === 'console' ? '|' + i.detail : ''), _w: i.occurrences || 1, _conds: i.conditions });
  }
  for (const [g, n] of Object.entries(base.meta.runsByGroup)) if (!drop.has(g)) addRuns(g, n);
  merged = `base run ${MB} (${base.meta.mode}, ${(base.meta.durationSec / 60).toFixed(1)} min, started ${base.meta.started}) with groups [${[...drop].join(', ')}] re-measured in this run`;
  for (const n of base.notes || []) NOTES.push(n);
  for (const t of base.toolErrors || []) if (!drop.has(t.group)) TOOL_ERRORS.push(t);
  if (base.meta.ios) var baseIos = base.meta.ios;
}
const meta = { mode: MODE, site: SITE, started, merged, finished: new Date().toISOString(), durationSec: Math.round((Date.now() - t0) / 1000), browsers: BROWSERS, views: VIEWS.length, viewIds: VIEWS.map((v) => v.id), conc: CONC, sweepStep: SWEEP_STEP, ios: (typeof baseIos !== 'undefined' && !ios.tried) ? baseIos : ios, fixtures };
const rep = report(meta);
log(`done in ${(meta.durationSec / 60).toFixed(1)} min: ${rep.meta.matrixRuns} runs, ${rep.meta.uniqueIssues} unique issues (${rep.meta.occurrences} occurrences), ${TOOL_ERRORS.length} tool errors → ${path.join(OUT, 'report.md')}`);
if (fixtures) {
  for (const f of fixtures) log(`  ${f.control ? (f.pass ? 'PASS  ' : 'FAIL  ') : f.caught ? 'CAUGHT' : 'MISSED'} ${f.id}  ${f.evidence.slice(0, 160)}`);
  const ok = fixtures.every((f) => (f.control ? f.pass : f.caught));
  cleanup();
  process.exit(ok ? 0 : 1);
}
cleanup();
process.exit(rep.meta.uniqueIssues || TOOL_ERRORS.length ? 1 : 0);
