// Kshana Studio first-time-user baseline: load timing, density, wayfinding evidence,
// phone-layout defects, newcomer task click counts, console errors and failed requests.
// Read-only: it only loads pages in a headless Chromium and writes into this folder.
//
// Rerun (regression check):
//   cd ~/Code/kshana-site-next/research/studio-ux-baseline
//   node baseline.mjs                      # everything (about 15 minutes, throttled runs are slow)
//   PHASES=load node baseline.mjs          # only the load timing
//   PHASES=views,tasks node baseline.mjs   # only density/phone checks and the click tasks
// Environment:
//   STUDIO   base URL of the Studio (default http://127.0.0.1:8853/)
//   SITE     base URL of the site (default http://127.0.0.1:8841/), used for the site-path deep links
//   REPS     unthrottled repetitions per load cell (default 3; the median is reported)
//   PW_DIR   a folder whose node_modules holds playwright (default: search the npx cache)
// Output: results.json and *.jpg in this folder. The report STUDIO-UX-BASELINE.md cites both.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const STUDIO = (process.env.STUDIO || "http://127.0.0.1:8853/").replace(/\/?$/, "/");
const SITE = (process.env.SITE || "http://127.0.0.1:8841/").replace(/\/?$/, "/");
const REPS = +(process.env.REPS || 3);
const PHASES = (process.env.PHASES || "views,tasks,site,load,waitshots").split(",");

function loadPlaywright() {
  const npx = path.join(process.env.HOME, ".npm/_npx");
  const tries = [process.env.PW_DIR, ...(fs.existsSync(npx) ? fs.readdirSync(npx).map((d) => path.join(npx, d)) : [])].filter(Boolean);
  for (const dir of tries) {
    try {
      const pw = createRequire(path.join(dir, "noop.js"))("playwright");
      if (fs.existsSync(pw.chromium.executablePath())) return pw; // only a copy whose browser is installed
    } catch { /* next */ }
  }
  throw new Error("playwright not found: run `npx -y playwright@1.62.1 --version` once, or set PW_DIR");
}
const { chromium } = loadPlaywright();

// Entries: the bare landing plus real deep links copied from the site build (explorations/2026-09-27-combo).
// `from` says where on the site each link appears and how many times it is linked.
export const ENTRIES = [
  { id: "landing", q: "", from: "bare Studio URL (site nav, docs/studio.html)" },
  { id: "dl-waterfall", q: "?scenario=l-band-waterfall-jamming&tab=spectrum&play=1", from: "home page hero, index.html" },
  { id: "dl-raim", q: "?scenario=integrity-raim&tab=overview", from: "most-linked deep link on the site (13 links)" },
  { id: "dl-lunar-time", q: "?scenario=lunar-time-budget&tab=overview", from: "lunar capability/mission cards (11 links)" },
  { id: "dl-clock-exports", q: "?scenario=clock-holdover&tab=exports", from: "home page + developers 'exports' links (7 links)" },
];

const DEVICES = {
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 },
  phone: {
    viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true,
    userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
  },
};
// Chrome DevTools "Fast 3G" preset (latency 150 ms x 3.75, 1.6 Mbit/s down x 0.9, 750 kbit/s up x 0.9) plus 4x CPU slowdown.
const NETS = {
  none: null,
  fast3g: { latency: 562.5, downloadThroughput: Math.round((1.6e6 / 8) * 0.9), uploadThroughput: Math.round((750e3 / 8) * 0.9), cpu: 4 },
};

async function shot(page, name, full = false) {
  await page.screenshot({ path: path.join(HERE, `${name}.jpg`), type: "jpeg", quality: 80, fullPage: full });
  return `${name}.jpg`;
}

// Installed before any page script: records the first moment each milestone is true (performance.now() ms).
const POLLER = () => {
  const M = (window.__ux = { marks: {}, lcp: 0 });
  const mark = (k, v) => { if (!(k in M.marks)) M.marks[k] = Math.round(performance.now()); if (v !== undefined && !((k + "Value") in M)) M[k + "Value"] = v; };
  try { new PerformanceObserver((l) => { for (const e of l.getEntries()) M.lcp = Math.round(e.startTime); }).observe({ type: "largest-contentful-paint", buffered: true }); } catch {}
  const g = (id) => document.getElementById(id);
  const inView = (el) => { if (!el || !el.getClientRects().length) return false; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 20 && r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth; };
  const tick = () => {
    const start = g("start"), chip = g("engine-chip"), kp = g("kpis-wrap"), figs = g("figs"), dash = g("dash"), err = g("error");
    if (start && !start.hidden && g("start-tiles") && g("start-tiles").children.length) mark("startShown");
    if (chip && chip.dataset.mode && chip.dataset.mode !== "loading") mark("engineReady", chip.dataset.mode);
    const resultReady = kp && !kp.hidden && figs && figs.children.length > 0;
    if (resultReady) mark("resultReady");
    if (resultReady && dash && !dash.hidden) {
      const anyView = [...document.querySelectorAll("#views .view:not([hidden])")].some(inView);
      if (inView(kp) || anyView || inView(g("headline"))) mark("resultVisible");
    }
    if (err && !err.hidden && err.textContent.trim()) mark("error", err.textContent.trim().slice(0, 300));
  };
  setInterval(tick, 40);
  document.addEventListener("DOMContentLoaded", tick);
};

// Runs inside the page: density, phone defects and wayfinding evidence for what is on screen now.
const MEASURE = () => {
  const vw = innerWidth, vh = innerHeight;
  const clipRect = (el) => {
    const r = el.getBoundingClientRect(); const rect = { l: r.left, t: r.top, r: r.right, b: r.bottom };
    for (let a = el.parentElement; a && a !== document.documentElement; a = a.parentElement) {
      const cs = getComputedStyle(a);
      if (cs.overflowX !== "visible" || cs.overflowY !== "visible") {
        const ar = a.getBoundingClientRect();
        if (cs.overflowX !== "visible") { rect.l = Math.max(rect.l, ar.left); rect.r = Math.min(rect.r, ar.right); }
        if (cs.overflowY !== "visible") { rect.t = Math.max(rect.t, ar.top); rect.b = Math.min(rect.b, ar.bottom); }
      }
      if (cs.position === "fixed") break;
    }
    return rect;
  };
  const visible = (el) => {
    if (!el.checkVisibility || !el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) return false;
    const r = el.getBoundingClientRect(); if (r.width < 1 || r.height < 1) return false;
    const c = clipRect(el); return c.r > c.l && c.b > c.t;
  };
  const inViewport = (el) => { const c = clipRect(el); return c.r > 0 && c.l < vw && c.b > 0 && c.t < vh && c.r > c.l && c.b > c.t; };
  const label = (el) => (el.getAttribute("aria-label") || el.textContent || el.getAttribute("placeholder") || el.title || el.id || el.tagName).replace(/\s+/g, " ").trim().slice(0, 48);
  const IA = 'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], [role="menuitem"], [role="slider"]';
  const all = [...document.querySelectorAll(IA)].filter((el) => !el.parentElement.closest(IA) || el.matches("input,select,textarea"));
  const rendered = (el) => { if (!el.checkVisibility || !el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) return false; const r = el.getBoundingClientRect(); return r.width >= 1 && r.height >= 1; };
  const vis = all.filter(rendered); // rendered anywhere on the page, including inside scrolled-away panels
  const above = all.filter(visible).filter(inViewport); // actually on screen now
  const inputs = (list) => list.filter((el) => el.matches("input:not([type=hidden]):not([type=search]), select, textarea"));
  const searches = (list) => list.filter((el) => el.matches("input[type=search]"));
  const blocks = new Set(); let words = 0; let wordsPage = 0;
  const tw = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, { acceptNode: (n) => (n.nodeValue.trim() ? 1 : 2) });
  const visCache = new Map();
  const isVis = (el) => { if (!visCache.has(el)) visCache.set(el, visible(el)); return visCache.get(el); };
  for (let n; (n = tw.nextNode());) {
    const p = n.parentElement; if (!p || p.closest("script,style,noscript,template,textarea,pre.code-hl")) continue;
    if (!rendered(p)) continue;
    const w = n.nodeValue.trim().split(/\s+/).length; wordsPage += w;
    if (!isVis(p)) continue;
    const rg = document.createRange(); rg.selectNodeContents(n);
    const rr = [...rg.getClientRects()]; const pc = clipRect(p);
    if (!rr.some((r) => r.bottom > Math.max(0, pc.t) && r.top < Math.min(vh, pc.b) && r.right > Math.max(0, pc.l) && r.left < Math.min(vw, pc.r))) continue;
    words += w;
    let b = p; while (b && b !== document.body && getComputedStyle(b).display === "inline") b = b.parentElement;
    blocks.add(b);
  }
  const se = document.scrollingElement;
  const hScroll = se.scrollWidth > vw + 1;
  const small = vis.filter((el) => { const r = el.getBoundingClientRect(); return r.width < 44 || r.height < 44; })
    .map((el) => { const r = el.getBoundingClientRect(); return { label: label(el), w: Math.round(r.width), h: Math.round(r.height), above: inViewport(el) }; });
  const offscreen = all.filter((el) => el.checkVisibility && el.checkVisibility({ checkVisibilityCSS: true }))
    .filter((el) => { const r = el.getBoundingClientRect(); return r.width > 0 && (r.left >= vw || r.right <= 0); })
    .map((el) => ({ label: label(el), left: Math.round(el.getBoundingClientRect().left) }));
  const overflowRight = [...document.querySelectorAll("body *")].filter((el) => {
    const r = el.getBoundingClientRect(); if (r.width < 1 || r.right <= vw + 1) return false;
    if (!el.checkVisibility || !el.checkVisibility({ checkVisibilityCSS: true })) return false;
    for (let a = el.parentElement; a; a = a.parentElement) { if (getComputedStyle(a).overflowX !== "visible") return false; }
    return true;
  }).slice(0, 15).map((el) => ({ tag: el.tagName.toLowerCase(), id: el.id, cls: String(el.className).slice(0, 40), right: Math.round(el.getBoundingClientRect().right) }));
  const clipped = [...document.querySelectorAll("body *")].filter((el) => {
    if (![...el.childNodes].some((c) => c.nodeType === 3 && c.nodeValue.trim())) return false;
    if (!isVis(el)) return false; const cs = getComputedStyle(el);
    return (cs.overflowX === "hidden" || cs.overflowX === "clip" || cs.textOverflow === "ellipsis") && el.scrollWidth > el.clientWidth + 1;
  }).slice(0, 25).map((el) => ({ text: label(el), id: el.id, cls: String(el.className).slice(0, 30), shown: el.clientWidth, needed: el.scrollWidth }));
  const fixed = [...document.querySelectorAll("body *")].filter((el) => { const p = getComputedStyle(el).position; return (p === "fixed" || p === "sticky") && isVis(el); })
    .map((el) => { const r = el.getBoundingClientRect(); return { id: el.id || String(el.className).split(" ")[0], pos: getComputedStyle(el).position, top: Math.round(r.top), h: Math.round(Math.min(r.bottom, vh) - Math.max(r.top, 0)) }; })
    .filter((x) => x.h > 0);
  // Content that needs a sideways swipe: horizontally scrollable boxes whose content is wider than the box.
  const sideways = [...document.querySelectorAll("body *")].filter((el) => { const ox = getComputedStyle(el).overflowX; return (ox === "auto" || ox === "scroll") && el.scrollWidth > el.clientWidth + 4 && isVis(el); })
    .map((el) => ({ id: el.id || String(el.className).split(" ")[0], shown: el.clientWidth, content: el.scrollWidth, hiddenPct: Math.round(100 * (1 - el.clientWidth / el.scrollWidth)) }));
  const bar = document.getElementById("bar"); const headerH = bar ? Math.round(bar.getBoundingClientRect().height) : null;
  const g = (id) => document.getElementById(id);
  const txt = (el) => (el && isVis(el) ? el.textContent.replace(/\s+/g, " ").trim() : null);
  const h1s = [...document.querySelectorAll("h1")].filter(isVis).map((e) => ({ text: e.textContent.trim(), inView: inViewport(e) }));
  const cur = document.querySelector("#steps [aria-current=step]");
  const nextBtn = [...document.querySelectorAll(".is-next")].find(isVis);
  const tabSel = document.querySelector("#tabs [aria-selected=true]");
  return {
    url: location.pathname + location.search, vw, vh, scrollHeight: Math.max(se.scrollHeight, (g("results") || se).scrollHeight),
    controls: { visibleOnPage: vis.length, aboveFold: above.length, disabledAboveFold: above.filter((e) => e.disabled).length },
    inputs: { visibleOnPage: inputs(vis).length, aboveFold: inputs(above).length, searchBoxesAboveFold: searches(above).length },
    text: { blocksAboveFold: blocks.size, wordsAboveFold: words, wordsOnPage: wordsPage },
    phone: { hScroll, scrollWidth: se.scrollWidth, smallTargets: small.length, smallTargetsAboveFold: small.filter((s) => s.above).length, smallSample: small.slice(0, 40), offscreen, overflowRight, clipped, fixed, sideways, headerH },
    way: {
      title: document.title, crumbs: txt(g("crumbs")), h1: h1s, status: txt(g("status")), stepCurrent: cur ? cur.textContent.trim() : null,
      nextHighlighted: nextBtn ? label(nextBtn) : null, nextInView: nextBtn ? inViewport(nextBtn) : false,
      firstHint: txt(g("first-hint")), question: txt(g("sc-question")), tabSelected: tabSel ? tabSel.textContent.trim() : null,
      tabs: [...document.querySelectorAll("#tabs [role=tab]")].filter(isVis).map((t) => t.textContent.trim()),
      engine: txt(g("engine-label")), error: txt(g("error")), notice: txt(g("notice")),
    },
  };
};

function median(a) { const v = a.filter((x) => x != null).sort((x, y) => x - y); return v.length ? v[Math.floor((v.length - 1) / 2)] : null; }

async function newPage(browser, devName, net) {
  const ctx = await browser.newContext({ ...DEVICES[devName], serviceWorkers: "block" });
  await ctx.addInitScript(POLLER);
  const page = await ctx.newPage();
  const log = { console: [], pageErrors: [], failed: [], http4xx5xx: [], bytes: 0, requests: 0, wasmFetches: 0 };
  page.on("console", (m) => { if (m.type() === "error" || m.type() === "warning") log.console.push({ type: m.type(), text: m.text().slice(0, 300) }); });
  page.on("pageerror", (e) => log.pageErrors.push(String(e).slice(0, 300)));
  ctx.on("requestfailed", (r) => log.failed.push({ url: r.url().slice(0, 160), err: (r.failure() || {}).errorText }));
  ctx.on("response", (r) => { if (r.status() >= 400) log.http4xx5xx.push({ url: r.url().slice(0, 160), status: r.status() }); });
  ctx.on("requestfinished", async (r) => { log.requests++; if (/\.wasm/.test(r.url())) log.wasmFetches++; try { const s = await r.sizes(); log.bytes += s.responseBodySize + s.responseHeadersSize; } catch {} });
  if (net) {
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: net.latency, downloadThroughput: net.downloadThroughput, uploadThroughput: net.uploadThroughput });
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: net.cpu });
  }
  return { ctx, page, log };
}

const marksOf = (page) => page.evaluate(() => window.__ux && window.__ux.marks).catch(() => null);
async function waitMarks(page, keys, timeout) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeout) {
    const m = await marksOf(page);
    if (m && (m.error !== undefined || keys.every((k) => m[k] !== undefined))) return m;
    await page.waitForTimeout(250);
  }
  return (await marksOf(page)) || {};
}

async function loadOnce(browser, entry, devName, netName, base = STUDIO) {
  const { ctx, page, log } = await newPage(browser, devName, NETS[netName]);
  const timeout = netName === "none" ? 60000 : 240000;
  const tNav = Date.now();
  await page.goto(base + entry.q, { waitUntil: "commit", timeout });
  const want = entry.id === "landing" ? ["startShown", "engineReady", "resultReady"] : ["engineReady", "resultVisible"];
  const marks = await waitMarks(page, want, timeout);
  const paint = await page.evaluate(() => Object.fromEntries(performance.getEntriesByType("paint").map((e) => [e.name, Math.round(e.startTime)]))).catch(() => ({}));
  const extra = await page.evaluate(() => ({ lcp: window.__ux.lcp, mode: window.__ux.engineReadyValue, error: window.__ux.errorValue || null })).catch(() => ({}));
  await page.waitForTimeout(500);
  const res = { entry: entry.id, device: devName, net: netName, wallMs: Date.now() - tNav, fp: paint["first-paint"] ?? null, fcp: paint["first-contentful-paint"] ?? null, lcp: extra.lcp || null,
    startShown: marks.startShown ?? null, engineReady: marks.engineReady ?? null, engineMode: extra.mode || null,
    resultReady: marks.resultReady ?? null, resultVisible: marks.resultVisible ?? null, error: extra.error,
    bytes: log.bytes, requests: log.requests, wasmFetches: log.wasmFetches, log };
  await ctx.close();
  return res;
}

async function phaseLoad(browser, out) {
  out.load = [];
  for (const netName of Object.keys(NETS)) for (const devName of Object.keys(DEVICES)) for (const entry of ENTRIES) {
    const reps = netName === "none" ? REPS : 1;
    const runs = [];
    for (let i = 0; i < reps; i++) runs.push(await loadOnce(browser, entry, devName, netName));
    const keys = ["fp", "fcp", "lcp", "startShown", "engineReady", "resultReady", "resultVisible"];
    const cell = { entry: entry.id, device: devName, net: netName, reps, ...Object.fromEntries(keys.map((k) => [k, median(runs.map((r) => r[k]))])),
      engineMode: runs[0].engineMode, error: runs.find((r) => r.error)?.error || null, bytes: runs[0].bytes, requests: runs[0].requests, wasmFetches: runs[0].wasmFetches,
      console: runs[0].log.console, pageErrors: runs[0].log.pageErrors, failed: runs[0].log.failed, http4xx5xx: runs[0].log.http4xx5xx, raw: runs.map(({ log, ...r }) => r) };
    out.load.push(cell);
    console.log(`load ${devName}/${netName}/${entry.id}: fcp ${cell.fcp} engine ${cell.engineReady} result ${cell.resultVisible ?? cell.resultReady} ms (${cell.bytes} B, ${cell.requests} req, wasm x${cell.wasmFetches})`);
    save(out);
  }
}

async function openReady(browser, devName, q, base = STUDIO) {
  const { ctx, page, log } = await newPage(browser, devName, null);
  await page.goto(base + q, { waitUntil: "commit" });
  await waitMarks(page, q ? ["engineReady", "resultVisible"] : ["startShown", "engineReady", "resultReady"], 60000);
  await page.waitForTimeout(1200);
  return { ctx, page, log };
}
const measure = (page) => page.evaluate(MEASURE);
const slimLog = (log) => ({ console: log.console, pageErrors: log.pageErrors, failed: log.failed, http4xx5xx: log.http4xx5xx });

async function phaseViews(browser, out) {
  out.views = [];
  const rec = async (page, devName, view, note = "") => {
    const m = await measure(page);
    const s = await shot(page, `${devName}-${view}`);
    out.views.push({ device: devName, view, note, shot: s, ...m });
    console.log(`view ${devName}/${view}: ${m.controls.aboveFold} controls, ${m.inputs.aboveFold} inputs, ${m.text.wordsAboveFold} words above fold; hScroll ${m.phone.hScroll}`);
    save(out);
    return out.views[out.views.length - 1];
  };
  for (const devName of Object.keys(DEVICES)) {
    const phone = devName === "phone";
    let { ctx, page, log } = await openReady(browser, devName, "");
    const v0 = await rec(page, devName, "landing-start", "bare URL, first visit, no stored preferences");
    await shot(page, `${devName}-landing-start-full`, true);
    await page.click("#start-suggest button[data-file='clock-holdover.toml']").catch(() => {});
    await page.evaluate(() => { window.__ux.marks = {}; });
    await waitMarks(page, ["resultVisible"], 30000); await page.waitForTimeout(1500);
    await rec(page, devName, "dash-clock-holdover", "after clicking the Good first run 'Hold time when GNSS is lost'");
    await shot(page, `${devName}-dash-clock-holdover-full`, true);
    if (phone) await page.click("#mob [data-mstep=set]").catch(() => {}); else await page.click("#btn-params").catch(() => {});
    await page.waitForTimeout(900);
    await rec(page, devName, "params-drawer", "Parameters opened on clock-holdover (Guided view)");
    await page.click("#pv-all").catch(() => {}); await page.waitForTimeout(500);
    await rec(page, devName, "params-all-fields", "Parameters, All fields view");
    await page.click("#params-close").catch(() => {}); await page.waitForTimeout(500);
    if (phone) await page.click("#mob [data-mstep=choose]").catch(() => {}); else await page.click("#btn-lib").catch(() => {});
    await page.waitForTimeout(900);
    await rec(page, devName, "library", phone ? "phone bottom bar 'Choose'" : "'Library' button in the run bar");
    v0.log = slimLog(log);
    await ctx.close();
    for (const entry of ENTRIES.filter((e) => e.id !== "landing")) {
      ({ ctx, page, log } = await openReady(browser, devName, entry.q));
      const v = await rec(page, devName, entry.id, `deep link ${entry.q} (${entry.from})`);
      v.log = slimLog(log);
      await shot(page, `${devName}-${entry.id}-full`, true);
      if (entry.id === "dl-raim") {
        const tabs = await page.locator("#tabs [role=tab]").allTextContents();
        for (let i = 0; i < tabs.length; i++) {
          await page.locator("#tabs [role=tab]").nth(i).click().catch(() => {}); await page.waitForTimeout(900);
          const name = tabs[i].trim().replace(/[^a-z0-9]+/gi, "-").toLowerCase();
          await rec(page, devName, `raim-tab-${name}`, `RAIM result, tab ${i + 1} of ${tabs.length}`);
        }
      }
      await ctx.close();
    }
  }
}

// Newcomer tasks from the bare landing. Each step: what is clicked, whether the newcomer must guess, and scroll needed.
const TASKS = [
  { id: "T1-gnss-coverage", goal: "See where in the world GNSS satellites are visible (multi-constellation coverage)",
    steps: [{ sel: "#start-suggest button[data-file='constellation-multi-gnss-coverage.toml']", what: "Good first run: 'Map the coverage of four GNSS constellations'", guess: false }] },
  { id: "T2-propagate-orbit", goal: "Propagate a satellite orbit and see its track",
    steps: [
      { sel: ".start-tile[data-domain=orbits]", what: "Domain tile 'Orbits & GNSS geometry' (one of 16 tiles)", guess: "'Orbits & GNSS geometry', 'Constellation design' and 'Mission analysis & space operations' are all plausible" },
      { sel: ".start-hit[data-file='orbit-sgp4-gps.toml']", what: "Scenario 'SGP4 orbits' (one of 8)", guess: "a newcomer must know that SGP4 is an orbit propagator; 'Ground track' is equally plausible" },
      { sel: "#tabs [role=tab]:has-text('Orbit')", what: "Result tab showing the orbit", guess: "the orbit picture is not the default panel", optional: true },
    ] },
  { id: "T3-lunar-navigation", goal: "Open a lunar navigation scenario and read its result",
    steps: [
      { sel: ".start-tile[data-domain=deepspace]", what: "Domain tile 'Moon, cislunar & Mars'", guess: false },
      { sel: ".start-hit[data-file='lunanet-araim.toml']", what: "Scenario 'Lunar integrity' (one of 22, none marked as a starting point)", guess: "22 lunar, cislunar and Mars scenarios with no recommended first pick" },
    ] },
];

async function phaseTasks(browser, out) {
  out.tasks = [];
  for (const devName of Object.keys(DEVICES)) for (const task of TASKS) {
    const { ctx, page } = await openReady(browser, devName, "");
    const steps = []; let clicks = 0, fail = null;
    for (const [i, st] of task.steps.entries()) {
      const loc = page.locator(st.sel).first();
      if (!(await loc.count())) { if (st.optional) { steps.push({ what: st.what, found: false }); continue; } fail = `step ${i + 1} not found: ${st.sel}`; break; }
      const pre = await loc.evaluate((el) => { const r = el.getBoundingClientRect(); return { top: Math.round(r.top), h: Math.round(r.height), w: Math.round(r.width), vh: innerHeight }; });
      const scrollNeeded = Math.max(0, pre.top + pre.h - pre.vh);
      await loc.scrollIntoViewIfNeeded();
      await page.evaluate(() => { window.__ux.marks = {}; });
      const t0 = await page.evaluate(() => performance.now());
      await loc.click(); clicks++;
      await page.waitForTimeout(700);
      const step = { n: clicks, what: st.what, guess: st.guess, optional: !!st.optional, scrollNeededPx: scrollNeeded, target: { w: pre.w, h: pre.h } };
      const last = i === task.steps.length - 1 || (task.steps[i + 1] && task.steps[i + 1].optional);
      if (last && !st.optional) {
        const m = await waitMarks(page, ["resultVisible"], 30000);
        step.resultMsAfterClick = m.resultVisible != null ? Math.round(m.resultVisible - t0) : null;
        step.resultReadyMsAfterClick = m.resultReady != null ? Math.round(m.resultReady - t0) : null;
      }
      step.shot = await shot(page, `${devName}-${task.id}-step${clicks}`);
      steps.push(step);
    }
    const m = await measure(page);
    out.tasks.push({ device: devName, id: task.id, goal: task.goal, clicksToFirstResult: task.steps.filter((s) => !s.optional).length, clicksTotal: clicks, fail, steps,
      after: { way: m.way, inputsAboveFold: m.inputs.aboveFold, controlsAboveFold: m.controls.aboveFold } });
    console.log(`task ${devName}/${task.id}: ${clicks} clicks ${fail || ""}`);
    save(out);
    await ctx.close();
  }
  const searches = [["T1-gnss-coverage", "coverage"], ["T2-propagate-orbit", "orbit"], ["T3-lunar-navigation", "lunar"]];
  out.taskSearch = [];
  for (const [id, q] of searches) for (const devName of Object.keys(DEVICES)) {
    const { ctx, page } = await openReady(browser, devName, "");
    await page.click("#start-search"); await page.fill("#start-search", q); await page.waitForTimeout(1200);
    const r = await page.evaluate(() => ({ hits: document.querySelectorAll("#start-results .start-hit:not(.field)").length, fieldHits: document.querySelectorAll("#start-results .start-hit.field").length, domainHits: document.querySelectorAll("#start-results .start-dom").length, first: ((document.querySelector("#start-results .start-hit b") || {}).textContent) || null, heads: [...document.querySelectorAll("#start-results h3")].map((x) => x.textContent) }));
    r.shot = await shot(page, `${devName}-search-${q}`);
    out.taskSearch.push({ id, device: devName, query: q, ...r });
    await ctx.close();
  }
  save(out);
}

// What a real site visitor gets: the same entries through the site's own copy of the Studio.
async function phaseSite(browser, out) {
  out.siteCopy = [];
  for (const entry of ENTRIES) for (const devName of Object.keys(DEVICES)) {
    const r = await loadOnce(browser, entry, devName, "none", SITE + "playground/index.html");
    out.siteCopy.push({ entry: entry.id, device: devName, engineReady: r.engineReady, engineMode: r.engineMode, resultVisible: r.resultVisible, resultReady: r.resultReady, startShown: r.startShown, error: r.error, failed: r.log.failed, http4xx5xx: r.log.http4xx5xx, console: r.log.console, pageErrors: r.log.pageErrors });
    console.log(`site ${devName}/${entry.id}: engine ${r.engineReady} result ${r.resultVisible} ${r.error || ""}`);
    save(out);
  }
}

// What a phone visitor on a slow link sees while waiting: screenshots at 5 s and 20 s on the most-linked deep link.
async function phaseWaitShots(browser, out) {
  const { ctx, page } = await newPage(browser, "phone", NETS.fast3g);
  await page.goto(STUDIO + ENTRIES[2].q, { waitUntil: "commit" });
  out.waitShots = [];
  for (const t of [5000, 20000]) {
    await page.waitForTimeout(t - (out.waitShots.length ? 5000 : 0));
    const status = await page.evaluate(() => ({ status: (document.getElementById("status") || {}).textContent, chip: (document.getElementById("engine-label") || {}).textContent })).catch(() => ({}));
    out.waitShots.push({ atMs: t, ...status, shot: await shot(page, `phone-fast3g-dl-raim-at-${t / 1000}s`) });
  }
  await ctx.close();
  save(out);
}

const OUT = path.join(HERE, "results.json");
function save(out) { fs.writeFileSync(OUT, JSON.stringify(out, null, 1)); }

const prev = fs.existsSync(OUT) ? JSON.parse(fs.readFileSync(OUT, "utf8")) : {};
const out = { ...prev, studio: STUDIO, siteBase: SITE, ranAt: new Date().toISOString(), entries: ENTRIES, nets: NETS,
  devices: Object.fromEntries(Object.entries(DEVICES).map(([k, v]) => [k, { ...v.viewport, dpr: v.deviceScaleFactor, touch: !!v.hasTouch }])) };
const browser = await chromium.launch({ headless: true });
try {
  if (PHASES.includes("views")) await phaseViews(browser, out);
  if (PHASES.includes("tasks")) await phaseTasks(browser, out);
  if (PHASES.includes("site")) await phaseSite(browser, out);
  if (PHASES.includes("load")) await phaseLoad(browser, out);
  if (PHASES.includes("waitshots")) await phaseWaitShots(browser, out);
} finally {
  save(out);
  await browser.close();
}
console.log("wrote", OUT);
