#!/usr/bin/env node
// Navigation check for the built kshana.dev: every link lands where it says, on phone and desktop.
//
// For every page of the site and the docs (found by following links from the home page and the docs
// index), in a phone viewport (iPhone 13) and a desktop one (1440 x 900):
//   ANCHOR   every in-page link with a #fragment, clicked after scrolling to the middle of the page:
//            2.5 s after the click, the target's top must sit just under the page's header offset
//            (html{scroll-padding-top}: 84 px on the site, so 60 to 130 px from the top of the screen;
//            0 on an engine run report). Where the window cannot scroll further (a target near the top
//            or the very bottom of a page) it only has to be in view below the header.
//   CROSS    every link from one page to a #fragment on another page, loaded fresh: the same rule.
//   PAGE     every link to another page without a fragment, clicked from a page scrolled down:
//            the new page must stay at the top (scroll position 0) 1.5 s after it loads. This catches
//            scroll restoration, focus() without preventScroll, and tabs that scroll on load.
// Links are followed as written, so clean URLs (/evidence#research, /docs/changelog) are exercised
// the way the host serves them: serve the build with features/serve.py (it resolves /x to x.html).
//
// Usage (needs a served build):
//   python3 features/serve.py explorations/2026-09-27-combo 8847 &      (a preview that serves clean URLs)
//   node features/check_nav.mjs [base-url] [--only phone|desktop] [--page /evidence] [--jobs N] [--verbose]
//   python3 explorations/2026-09-27-combo/check.py --nav [base-url]     (same thing, from the site check)
// base-url defaults to http://127.0.0.1:8847. Exit 1 if any link is BAD. Playwright is loaded from
// this tree's node_modules if present, else from $KSHANA_PLAYWRIGHT (a node_modules directory), else
// from promo/2026-10-01-video/node_modules. It drives the installed Chrome (channel "chrome").
import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
async function loadPlaywright() {
  try { return await import("playwright"); } catch { /* fall through */ }
  for (const dir of [process.env.KSHANA_PLAYWRIGHT, path.join(HERE, "..", "promo", "2026-10-01-video", "node_modules"), path.join(HERE, "..", "scratch", "navcheck", "node_modules")]) {
    if (dir && existsSync(path.join(dir, "playwright"))) return createRequire(path.join(dir, "x.js"))("playwright");
  }
  throw new Error("playwright not found: set KSHANA_PLAYWRIGHT to a node_modules directory that has it");
}
const { chromium, devices } = await loadPlaywright();

const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args.splice(i, 2)[1] : null; };
const flag = (k) => { const i = args.indexOf(k); if (i >= 0) { args.splice(i, 1); return true; } return false; };
const ONLY = opt("--only"), ONE_RAW = opt("--page"), JOBS = Number(opt("--jobs") || 6), VERBOSE = flag("--verbose");
const BASE = (args[0] || "http://127.0.0.1:8847").replace(/\/$/, "");
const TASK_MS = 90000;
// One key per page whatever the spelling: "/", "/index.html" -> "/index"; "/evidence.html", "/evidence" -> "/evidence"; "/docs/" -> "/docs/index".
const key = (p) => { try { p = decodeURI(p); } catch { /* as is */ } if (p.endsWith("/")) p += "index"; return p.replace(/\.html$/, ""); };
const isPage = (p) => !p.startsWith("/studio/") && !p.startsWith("/playground/") && (/\.html$/.test(p) || p.endsWith("/") || !/\.[a-z0-9]+$/i.test(p.split("/").pop()));
const ONE = ONE_RAW ? key(ONE_RAW) : null;

// Linux: KSHANA_CHROME names a Chromium binary (e.g. /opt/pw-browsers/chromium-1194/chrome-linux/chrome); otherwise macOS Chrome.
const browser = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : { channel: "chrome" });
const VIEWPORTS = [["phone", { ...devices["iPhone 13"] }], ["desktop", { viewport: { width: 1440, height: 900 } }]].filter(([n]) => !ONLY || n === ONLY);
const newCtx = async (opts) => { const c = await browser.newContext(opts); c.setDefaultTimeout(15000); c.setDefaultNavigationTimeout(30000); return c; };

// ------------------------------------------------------------------ discover pages and links
async function collect(page) {
  return page.evaluate(() => [...document.querySelectorAll("a[href]")].filter((a) => !a.hasAttribute("download")).map((a) => {
    const u = new URL(a.href, location.href);
    return { path: u.pathname, hash: u.hash, origin: u.origin, visible: a.checkVisibility ? a.checkVisibility({ visibilityProperty: true }) : !!(a.offsetParent || a.getClientRects().length), raw: a.getAttribute("href") };
  }).filter((l) => l.origin === location.origin && !l.raw.startsWith("javascript:")));
}
const pageUrl = {}; // key -> the path as the site links it (first seen)
async function discover() {
  const ctx = await newCtx({ viewport: { width: 1440, height: 900 } });
  const p = await ctx.newPage();
  const links = {}, queue = ["/", "/docs/"], seen = new Set();
  pageUrl[key("/")] = "/"; pageUrl[key("/docs/")] = "/docs/";
  while (queue.length) {
    const cur = queue.shift(), k = key(cur);
    if (seen.has(k)) continue;
    seen.add(k);
    const r = await p.goto(BASE + cur, { waitUntil: "load" }).catch(() => null);
    if (!r || !r.ok()) { links[k] = null; continue; }
    links[k] = await collect(p);
    for (const l of links[k]) {
      if (!isPage(l.path)) continue;
      const t = key(l.path);
      if (!pageUrl[t]) pageUrl[t] = l.path;
      if (!seen.has(t)) queue.push(l.path);
    }
  }
  await ctx.close();
  return links;
}

// ------------------------------------------------------------------ measuring
// Where the target is 250, 1000 and 2500 ms after the action, and whether the window is at its first
// or last scroll position. When nothing moved between 250 and 1000 ms the 1000 ms reading is final.
async function sampleTarget(p, hash, t0) {
  const out = [];
  for (const t of [250, 1000, 2500]) {
    if (out.length === 2 && out[0].top === out[1].top) { out.push(out[1]); break; }
    const wait = t - (Date.now() - t0); if (wait > 0) await p.waitForTimeout(wait);
    out.push(await p.evaluate((h) => {
      let id; try { id = decodeURIComponent(h.slice(1)); } catch { id = h.slice(1); }
      const el = document.getElementById(id) || document.getElementsByName(id)[0];
      if (!el) return { top: "missing" };
      const r = el.getBoundingClientRect();
      if (!r.width && !r.height) return { top: "hidden" };
      // Measured in the visual viewport (what the reader sees): a page wider than a phone is zoomed
      // out, and its layout viewport is taller than the screen.
      const v = window.visualViewport || { offsetTop: 0, pageTop: scrollY, height: innerHeight };
      const se = document.scrollingElement;
      const pad = parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop) || 0;
      return { top: Math.round(r.top - v.offsetTop), pad, vh: Math.round(v.height), atStart: v.pageTop <= 1, atEnd: v.pageTop + v.height >= se.scrollHeight - 2 };
    }, hash));
  }
  return out;
}
function judge(s) {
  const f = s[s.length - 1];
  if (typeof f.top !== "number") return false;
  // The page's own header offset (html{scroll-padding-top}: 84 px on the site, 0 on a run report).
  if (f.top >= f.pad - 24 && f.top <= f.pad + 46) return true;
  if (f.atStart && f.top >= 0 && f.top < f.vh - 20) return true; // a target at the top of the page (a skip link to <main>)
  return f.atEnd && f.top >= Math.max(0, f.pad - 24) && f.top < f.vh - 20; // the window cannot scroll further
}

const results = [];
const log = (ok, line) => { results.push({ ok, line }); if (!ok || VERBOSE) console.log(`${ok ? "OK " : "BAD"} ${line}`); };

// Mark the first visible link that goes to page key k with hash h ("" for none); returns whether one was found.
async function mark(p, k, h) {
  return p.evaluate(([k, h]) => {
    const key = (q) => { try { q = decodeURI(q); } catch (e) { /* as is */ } if (q.endsWith("/")) q += "index"; return q.replace(/\.html$/, ""); };
    document.querySelectorAll("[data-navcheck]").forEach((x) => x.removeAttribute("data-navcheck"));
    const a = [...document.querySelectorAll("a[href]")].find((a) => {
      // rendered, not inside a closed <details> or a hidden panel
      if (a.hasAttribute("download") || !(a.checkVisibility ? a.checkVisibility({ visibilityProperty: true }) : (a.offsetParent || a.getClientRects().length))) return false;
      const u = new URL(a.href, location.href);
      return u.origin === location.origin && key(u.pathname) === k && u.hash === h;
    });
    if (a) a.setAttribute("data-navcheck", "1");
    return !!a;
  }, [k, h]);
}

async function inPage(p, name, k, anchors) {
  const url = pageUrl[k];
  await p.goto("about:blank");
  await p.goto(BASE + url, { waitUntil: "load" }); await p.waitForTimeout(800);
  for (const h of anchors) {
    if (key(new URL(p.url()).pathname) !== k) await p.goto(BASE + url, { waitUntil: "load" });
    await p.evaluate(() => scrollTo({ top: document.scrollingElement.scrollHeight / 2, behavior: "instant" })); await p.waitForTimeout(100);
    if (!(await mark(p, k, h))) continue; // not shown in this viewport (a desktop-only table of contents, for example)
    const a = p.locator("[data-navcheck]");
    try { await a.scrollIntoViewIfNeeded({ timeout: 1500 }); } catch { /* clicked as is */ }
    const t0 = Date.now();
    // A skip link sits off screen until it has keyboard focus: follow it the way a keyboard user does.
    try { await a.click({ timeout: 2000 }); } catch {
      try { await a.focus(); await p.keyboard.press("Enter"); } catch { log(false, `${name} ${url} click ${h}: link could not be followed`); continue; }
    }
    if (key(new URL(p.url()).pathname) !== k) { log(false, `${name} ${url} click ${h}: left the page for ${p.url()}`); continue; }
    const s = await sampleTarget(p, h, t0);
    log(judge(s), `${name} ${url} click ${h} top@250/1000/2500ms=${s.map((x) => x.top).join("/")}`);
  }
}
async function crossPage(p, name, target, hash, from) {
  await p.goto("about:blank"); // a fresh load, never a same-document jump
  const t0 = Date.now();
  const r = await p.goto(BASE + target + hash, { waitUntil: "load" }).catch(() => null);
  if (!r || !r.ok()) { log(false, `${name} ${pageUrl[from]} -> ${target}${hash}: not served (${r ? r.status() : "no response"})`); return; }
  const s = await sampleTarget(p, hash, t0);
  log(judge(s), `${name} ${pageUrl[from]} -> ${target}${hash} top@250/1000/2500ms=${s.map((x) => x.top).join("/")}`);
}
async function pageLink(p, name, from, target) {
  await p.goto("about:blank");
  await p.goto(BASE + pageUrl[from], { waitUntil: "load" });
  await p.evaluate(() => scrollTo({ top: Math.min(3000, document.scrollingElement.scrollHeight), behavior: "instant" })); await p.waitForTimeout(200);
  if (!(await mark(p, key(target), ""))) return;
  // a link that opens a new tab (an engine report): load its address in this tab instead
  if (await p.locator("[data-navcheck]").evaluate((a) => a.target && a.target !== "_self")) {
    const r = await p.goto(BASE + target, { waitUntil: "load" }).catch(() => null);
    if (!r || !r.ok()) { log(false, `${name} ${pageUrl[from]} page link -> ${target}: not served`); return; }
    await p.waitForTimeout(1500);
    const y0 = await p.evaluate(() => Math.round(scrollY));
    log(y0 === 0, `${name} ${pageUrl[from]} page link (new tab) -> ${target} y=${y0}`);
    return;
  }
  try { await Promise.all([p.waitForURL((u) => key(u.pathname) === key(target), { timeout: 15000 }), p.locator("[data-navcheck]").click({ timeout: 3000 })]); }
  catch { log(false, `${name} ${pageUrl[from]} page link -> ${target}: did not navigate`); return; }
  await p.waitForLoadState("load"); await p.waitForTimeout(1500);
  const y = await p.evaluate(() => Math.round(scrollY));
  log(y === 0, `${name} ${pageUrl[from]} page link -> ${target} y=${y}`);
}

// n workers, each with one tab; a task that throws or hangs gets a new tab and one retry.
async function pool(ctx, tasks, n) {
  let i = 0;
  await Promise.all(Array.from({ length: Math.min(n, tasks.length) }, async () => {
    let p = await ctx.newPage();
    while (i < tasks.length) {
      const t = tasks[i++];
      for (let attempt = 0; attempt < 2; attempt++) {
        let timer;
        try {
          await Promise.race([t(p), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error(`no result in ${TASK_MS / 1000} s`)), TASK_MS); })]);
          clearTimeout(timer); break;
        } catch (e) {
          clearTimeout(timer);
          await p.close().catch(() => {}); p = await ctx.newPage();
          if (attempt) log(false, `error: ${t.label}: ${String(e).split("\n")[0]}`);
        }
      }
    }
    await p.close().catch(() => {});
  }));
}

// ------------------------------------------------------------------ run
const links = await discover();
const pages = Object.keys(links).filter((k) => links[k] && (!ONE || k === ONE)).sort();
for (const [k, l] of Object.entries(links)) if (l === null) log(false, `discover ${pageUrl[k] || k}: not served`);
const cross = new Map(), targets = new Map();
for (const from of Object.keys(links)) for (const l of links[from] || []) {
  if (!isPage(l.path)) continue;
  const t = key(l.path);
  if (t === from || (ONE && t !== ONE)) continue;
  if (l.hash.length > 1) { if (!cross.has(t + l.hash)) cross.set(t + l.hash, [l.path, l.hash, from]); }
  else if (!targets.has(t)) targets.set(t, [l.path, from]);
}
console.log(`navigation check: ${BASE} · ${pages.length} pages · ${cross.size} cross-page fragments · ${targets.size} page targets · ${VIEWPORTS.map(([n]) => n).join(" + ")}`);
for (const [name, opts] of VIEWPORTS) {
  const ctx = await newCtx(opts);
  const tasks = [];
  for (const k of pages) {
    const anchors = [...new Set(links[k].filter((l) => isPage(l.path) && key(l.path) === k && l.hash.length > 1).map((l) => l.hash))];
    // long pages (the changelog) in slices, so one task never runs past the time limit
    for (let i = 0; i < anchors.length; i += 12) {
      const part = anchors.slice(i, i + 12);
      tasks.push(Object.assign((p) => inPage(p, name, k, part), { label: `${name} ${pageUrl[k]} ${part[0]}…` }));
    }
  }
  for (const [t, h, from] of cross.values()) tasks.push(Object.assign((p) => crossPage(p, name, t, h, from), { label: `${name} ${t}${h}` }));
  for (const [t, from] of targets.values()) tasks.push(Object.assign((p) => pageLink(p, name, from, t), { label: `${name} -> ${t}` }));
  await pool(ctx, tasks, JOBS);
  await ctx.close();
}
await browser.close();
const summary = VIEWPORTS.map(([n]) => { const r = results.filter((x) => x.line.startsWith(n + " ")); return `${n}: ${r.filter((x) => !x.ok).length} BAD of ${r.length}`; }).join(" · ");
const other = results.filter((x) => !x.ok && !VIEWPORTS.some(([n]) => x.line.startsWith(n + " "))).length;
const bad = results.filter((x) => !x.ok).length;
console.log(`${bad ? "FAILED" : "OK"}: ${summary}${other ? ` · ${other} discovery or browser errors` : ""}`);
process.exit(bad ? 1 : 0);
