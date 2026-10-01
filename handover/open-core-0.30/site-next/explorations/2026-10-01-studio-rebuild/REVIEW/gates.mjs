// Studio rebuild gates, adapted from research/studio-ux-baseline/baseline.mjs (same browser,
// same devices, same Fast 3G + 4x CPU preset, same "result visible" idea) for the new screens.
// Read-only: it loads pages in a headless Chromium and writes REVIEW/gates.json.
//
//   cd explorations/2026-10-01-studio-rebuild && python3 -m http.server 8877 &   (or the LAN server)
//   node REVIEW/gates.mjs                 # every gate (about 6 minutes; the throttled loads are slow)
//   PHASES=load,clicks,inputs,phone,links,throttle node REVIEW/gates.mjs
// Environment: STUDIO (default http://127.0.0.1:8877/), REPS (default 3), PW_DIR (playwright).
//
// Gates (from the founder's brief, 2026-10-01):
//   load      first visible result <= 1.5 s unthrottled, every entry, desktop and phone (median of REPS)
//   throttle  on Fast 3G + 4x CPU a recorded result shows, labelled, before the engine is ready
//   clicks    <= 2 clicks to a first result from any entry
//   inputs    <= 5 inputs visible by default on every scenario (all of them, by script)
//   phone     header <= 64 px; tap targets >= 44 px; no page or box hiding content sideways
//   links     every Studio link the site build uses opens the right scenario and view
//   errors    0 console errors, page errors or failed requests across every load above
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.dirname(HERE);
const STUDIO = (process.env.STUDIO || "http://127.0.0.1:8877/").replace(/\/?$/, "/");
const REPS = +(process.env.REPS || 3);
const PHASES = (process.env.PHASES || "load,clicks,inputs,phone,links,throttle").split(",");

function loadPlaywright() {
  const npx = path.join(process.env.HOME, ".npm/_npx");
  const tries = [process.env.PW_DIR, ...(fs.existsSync(npx) ? fs.readdirSync(npx).map((d) => path.join(npx, d)) : [])].filter(Boolean);
  for (const dir of tries) {
    try { const pw = createRequire(path.join(dir, "noop.js"))("playwright"); if (fs.existsSync(pw.chromium.executablePath())) return pw; } catch { /* next */ }
  }
  throw new Error("playwright not found: run `npx -y playwright@1.62.1 --version` once, or set PW_DIR");
}
const { chromium } = loadPlaywright();

const DEVICES = {
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 },
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1" },
};
const FAST3G = { latency: 562.5, downloadThroughput: Math.round((1.6e6 / 8) * 0.9), uploadThroughput: Math.round((750e3 / 8) * 0.9), cpu: 4 };
// The baseline's entries (real links from the site build) plus the opening screen's questions.
const ENTRIES = [
  { id: "landing", q: "" },
  { id: "dl-waterfall", q: "?scenario=l-band-waterfall-jamming&tab=spectrum&play=1" },
  { id: "dl-raim", q: "?scenario=integrity-raim&tab=overview" },
  { id: "dl-lunar-time", q: "?scenario=lunar-time-budget&tab=overview" },
  { id: "dl-clock-exports", q: "?scenario=clock-holdover&tab=exports" },
];

// "Result visible": on the opening screen the hero map and its answer; on a task screen the
// plain answer and its key figures (or, for a result without cards, the answer) in the viewport.
const PROBE = () => {
  window.__t = {};
  const mark = (k) => { if (!(k in window.__t)) window.__t[k] = performance.now(); };
  const inView = (el) => { if (!el) return false; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.top < innerHeight && r.bottom > 0; };
  const tick = () => {
    const scr = document.body && document.body.dataset.screen;
    if (scr === "home" && !/^Loading/.test((document.getElementById("hero-answer") || {}).textContent || "Loading") && document.querySelector("#hero-figs div") && inView(document.getElementById("hero-answer"))) mark("answer");
    if (scr === "task" && document.querySelector("#figs .kf") && inView(document.getElementById("m-big"))) mark("answer");
    if (scr === "home" && document.querySelector("#hero-map svg") && !/^Loading/.test((document.getElementById("hero-answer") || {}).textContent || "Loading") && inView(document.getElementById("hero-map"))) mark("result");
    if (scr === "task") {
      const big = document.getElementById("m-big");
      if (big && big.textContent && !/^Opening/.test(big.textContent) && inView(big)) { mark("result"); const c = document.querySelector("#m-chips .chip-s2"); if (c) window.__t.label = window.__t.label || c.textContent; }
    }
    const chip = document.getElementById("engine-chip");
    if (chip && chip.dataset.mode && chip.dataset.mode !== "loading") mark("engine");
    if (!("result" in window.__t) || !("engine" in window.__t)) setTimeout(tick, 30);
  };
  document.addEventListener("DOMContentLoaded", tick);
};

const errors = [], harness = [];
// Playwright's serviceWorkers: "block" (a true first visit, as in the baseline) replaces
// navigator.serviceWorker in every frame; inside the engine report's sandboxed frame that
// replacement throws. With service workers allowed the same pages log nothing, so these are
// kept apart as harness noise, not page errors.
// The probe itself (addInitScript) is injected into every frame, the sandboxed report frame
// included, where Chromium blocks it and logs "Blocked script execution in 'about:srcdoc'".
// Loaded without the probe (REVIEW/shots.mjs) the same views log nothing.
const HARNESS = /Failed to read the 'serviceWorker' property from 'Navigator'|Blocked script execution in 'about:srcdoc'/;
async function newPage(browser, device, opts = {}) {
  const ctx = await browser.newContext({ ...DEVICES[device], serviceWorkers: "block", colorScheme: opts.theme || "light" });
  const page = await ctx.newPage();
  await page.addInitScript(PROBE);
  const where = () => page.url().replace(STUDIO, "");
  page.on("console", (m) => { if (m.type() === "error") (HARNESS.test(m.text()) ? harness : errors).push({ device, at: where(), text: m.text() }); });
  page.on("pageerror", (e) => (HARNESS.test(e.message) ? harness : errors).push({ device, at: where(), text: "page error: " + e.message }));
  page.on("requestfailed", (r) => { if (!/net::ERR_ABORTED/.test(r.failure()?.errorText || "")) errors.push({ device, at: where(), text: `request failed: ${r.url()} ${r.failure()?.errorText}` }); });
  page.on("response", (r) => { if (r.status() >= 400) errors.push({ device, at: where(), text: `HTTP ${r.status()} ${r.url()}` }); });
  return { ctx, page };
}
const waitResult = (page, timeout = 30000) => page.waitForFunction(() => window.__t && "result" in window.__t, null, { timeout });
const median = (xs) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]; };
const out = { studio: STUDIO, when: new Date().toISOString(), gates: {} };

const browser = await chromium.launch();

// ---------------------------------------------------------------- load
if (PHASES.includes("load")) {
  const rows = [];
  for (const device of ["desktop", "phone"]) for (const e of ENTRIES) {
    const ts = [];
    for (let i = 0; i < REPS; i++) {
      const { ctx, page } = await newPage(browser, device);
      await page.goto(STUDIO + "index.html" + e.q);
      await waitResult(page);
      ts.push(await page.evaluate(() => window.__t.result));
      await ctx.close();
    }
    rows.push({ device, entry: e.id, first_result_ms: Math.round(median(ts)), runs: ts.map(Math.round) });
  }
  out.gates.load = { target: "<= 1500 ms", rows, pass: rows.every((r) => r.first_result_ms <= 1500) };
  console.log("load", JSON.stringify(rows.map((r) => `${r.device}/${r.entry} ${r.first_result_ms}`)), out.gates.load.pass ? "PASS" : "FAIL");
}

// ---------------------------------------------------------------- clicks
if (PHASES.includes("clicks")) {
  const paths = [];
  const run = async (device, name, steps) => {
    const { ctx, page } = await newPage(browser, device);
    await page.goto(STUDIO + "index.html");
    await page.waitForSelector("#tiles .tile");
    let clicks = 0;
    for (const s of steps) {
      if (s.type) { await page.fill(s.type, s.text); await page.waitForTimeout(400); continue; }
      await page.locator(s.click).first().scrollIntoViewIfNeeded();
      await page.locator(s.click).first().click();
      clicks++;
      await page.waitForTimeout(150);
    }
    await page.waitForFunction(() => document.body.dataset.screen === "task" && !/^Opening/.test(document.getElementById("m-big").textContent || "") && document.getElementById("m-big").textContent, null, { timeout: 20000 });
    const got = await page.evaluate(() => ({ url: location.search, big: document.getElementById("m-big").textContent }));
    paths.push({ device, task: name, clicks, ...got });
    await ctx.close();
  };
  for (const device of ["desktop", "phone"]) {
    await run(device, "question: can I trust my fix", [{ click: '#questions a[data-file="integrity-raim.toml"]' }]);
    await run(device, "hero: open this result", [{ click: "#hero-open" }]);
    await run(device, "tile: Moon and Mars, then lunar integrity", [{ click: '#tiles a[data-domain="deepspace"]' }, { click: '#start-results a[data-file="lunanet-araim.toml"]' }]);
    await run(device, "tile: orbits, then SGP4 orbits", [{ click: '#tiles a[data-domain="orbits"]' }, { click: '#start-results a[data-file="orbit-sgp4-gps.toml"]' }]);
    await run(device, "browse all, search coverage, first hit", [{ click: "#tiles a.tile.all" }, { type: "#start-search", text: "coverage" }, { click: "#start-results a.start-hit" }]);
  }
  out.gates.clicks = { target: "<= 2 clicks (deep links: 0)", paths, max: Math.max(...paths.map((p) => p.clicks)), pass: paths.every((p) => p.clicks <= 2) };
  console.log("clicks", JSON.stringify(paths.map((p) => `${p.device}/${p.task}: ${p.clicks} -> ${p.url}`)), out.gates.clicks.pass ? "PASS" : "FAIL");
}

// ---------------------------------------------------------------- inputs on every scenario
// Inputs: text, number and range inputs, selects and text areas that are rendered visible
// anywhere on the page (not in a closed section), search boxes excepted.
const COUNT_INPUTS = () => [...document.querySelectorAll("input,select,textarea")].filter((el) => {
  if (el.type === "search" || el.type === "hidden" || el.type === "checkbox" || el.type === "radio") return false;
  if (!el.getClientRects().length) return false;
  // Closed <details> keep a layout box in current Chromium (content-visibility), so check them.
  for (let d = el.closest("details"); d; d = d.parentElement && d.parentElement.closest("details")) if (!d.open && !el.closest("summary")) return false;
  const cs = getComputedStyle(el);
  return cs.visibility !== "hidden" && cs.display !== "none";
}).map((el) => el.id || el.getAttribute("aria-label") || el.type);
if (PHASES.includes("inputs")) {
  const all = await (async () => {
    const { ctx, page } = await newPage(browser, "desktop");
    await page.goto(STUDIO + "index.html");
    await page.waitForSelector("#tiles .tile");
    await page.waitForFunction(() => document.querySelector('.tile.all b') && /139|\d{3}/.test(document.querySelector('.tile.all b').textContent));
    const names = await page.evaluate(async () => (await import("./lib/catalog.mjs")).SCENARIOS.map((s) => s[0]));
    const rows = [];
    for (const device of ["desktop", "phone"]) {
      const { ctx: c2, page: p2 } = device === "desktop" ? { ctx, page } : await newPage(browser, "phone");
      if (device === "phone") { await p2.goto(STUDIO + "index.html"); await p2.waitForSelector("#tiles .tile"); }
      for (const f of names) {
        await p2.evaluate((href) => { const a = document.createElement("a"); a.href = href; document.body.append(a); a.click(); a.remove(); }, `?scenario=${f.replace(/\.toml$/, "")}`);
        await p2.waitForFunction((file) => document.title !== "Kshana Studio" && document.body.dataset.screen === "task" && !/^Opening/.test(document.getElementById("m-big").textContent || "") && (document.getElementById("m-big").textContent || document.getElementById("error").textContent), f, { timeout: 30000 }).catch(() => {});
        await p2.waitForTimeout(250);
        const ins = await p2.evaluate(COUNT_INPUTS);
        rows.push({ device, file: f, visible_inputs: ins.length, inputs: ins });
      }
      if (device === "phone") await c2.close();
    }
    await ctx.close();
    return rows;
  })();
  const over = all.filter((r) => r.visible_inputs > 5);
  out.gates.inputs = { target: "<= 5 per scenario", scenarios: new Set(all.map((r) => r.file)).size, checked: all.length, max: Math.max(...all.map((r) => r.visible_inputs)), over, histogram: all.reduce((m, r) => ((m[r.visible_inputs] = (m[r.visible_inputs] || 0) + 1), m), {}), pass: !over.length };
  console.log("inputs", `${out.gates.inputs.scenarios} scenarios, ${all.length} checks, max ${out.gates.inputs.max}, over 5: ${over.length}`, JSON.stringify(out.gates.inputs.histogram), out.gates.inputs.pass ? "PASS" : "FAIL");
  fs.writeFileSync(path.join(HERE, "inputs-per-scenario.json"), JSON.stringify(all, null, 1));
}

// ---------------------------------------------------------------- phone layout
const PHONE_MEASURE = () => {
  const vis = (el) => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && r.right > 0 && r.left < innerWidth && cs.visibility !== "hidden" && inOpen(el); };
  const inOpen = (el) => { for (let d = el.closest("details"); d; d = d.parentElement && d.parentElement.closest("details")) if (!d.open && !(el.closest("summary") && el.closest("summary").parentElement === d)) return false; return true; }; // the skip link waits off screen until focused
  const ctl = [...document.querySelectorAll("a[href],button,input,select,textarea,summary,[role=tab],[role=button]")].filter((el) => vis(el) && !el.closest("svg") && !(el.parentElement && el.parentElement.closest("a[href],button,summary")));
  const small = ctl.filter((el) => { const r = el.getBoundingClientRect(); return r.width < 44 || r.height < 44; }).map((el) => { const r = el.getBoundingClientRect(); return `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : el.className && typeof el.className === "string" ? "." + el.className.split(" ")[0] : ""} "${(el.textContent || el.getAttribute("aria-label") || "").trim().slice(0, 30)}" ${Math.round(r.width)}x${Math.round(r.height)}`; });
  const boxes = [...document.querySelectorAll("*")].filter((el) => { const cs = getComputedStyle(el); return /(auto|scroll)/.test(cs.overflowX) && el.scrollWidth > el.clientWidth + 1 && vis(el); }).map((el) => `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : "." + String(el.className).split(" ")[0]} ${el.scrollWidth}/${el.clientWidth}`);
  return { header: Math.round(document.getElementById("bar").getBoundingClientRect().height), pageSideways: document.scrollingElement.scrollWidth > innerWidth, boxes, smallTargets: small, controls: ctl.length };
};
if (PHASES.includes("phone")) {
  const rows = [];
  const views = [
    { id: "landing", q: "" }, { id: "domain", q: "?domain=deepspace" }, { id: "browse-search", q: "?browse=1&q=orbit" },
    ...ENTRIES.slice(1), { id: "coverage", q: "?scenario=constellation-multi-gnss-coverage" }, { id: "campaign", q: "?scenario=campaign-jam-spoof-holdover-integrity" },
    { id: "raim-open-all", q: "?scenario=integrity-raim", open: true },
    { id: "landing-engine-loading", q: "", hold: true }, { id: "raim-engine-loading", q: "?scenario=integrity-raim", hold: true },
    { id: "raim-sweep", q: "?scenario=integrity-raim&tab=sweep" }, { id: "raim-json", q: "?scenario=integrity-raim&tab=json" }, { id: "raim-animation", q: "?scenario=integrity-raim&tab=animation" },
    { id: "raim-report", q: "?scenario=integrity-raim&tab=report" }, { id: "telecom-masks", q: "?scenario=telecom-prtc-holdover-24h&tab=masks" }, { id: "solar", q: "?scenario=solar-system-tour" },
    { id: "leo-chain", q: "?scenario=leo-pnt-end-to-end" }, { id: "orbit-3d", q: "?scenario=orbit-sgp4-gps&tab=orbit" },
  ];
  for (const v of views) {
    const { ctx, page } = await newPage(browser, "phone");
    if (v.hold) await page.route("**/pkg/kshana_bg.wasm", () => new Promise(() => {})); // the engine never arrives: the loading state
    await page.goto(STUDIO + "index.html" + v.q);
    await page.waitForFunction(() => window.__t && ("result" in window.__t || document.body.dataset.screen === "browse"), null, { timeout: 30000 }).catch(() => {});
    await page.waitForTimeout(1500);
    if (v.open) await page.evaluate(() => { for (const d of document.querySelectorAll("details")) d.open = true; });
    await page.waitForTimeout(200);
    rows.push({ view: v.id, ...(await page.evaluate(PHONE_MEASURE)) });
    await ctx.close();
  }
  const pass = rows.every((r) => r.header <= 64 && !r.pageSideways && !r.boxes.length && !r.smallTargets.length);
  out.gates.phone = { target: "header <= 64 px, no sideways content, targets >= 44 px", rows, pass };
  for (const r of rows) console.log("phone", r.view, `header ${r.header}`, r.pageSideways ? "PAGE-SIDEWAYS" : "", r.boxes.length ? "boxes " + r.boxes.join(", ") : "", r.smallTargets.length ? `small ${r.smallTargets.length}: ${r.smallTargets.slice(0, 6).join("; ")}` : "targets ok");
  console.log("phone", pass ? "PASS" : "FAIL");
}

// ---------------------------------------------------------------- site links
if (PHASES.includes("links")) {
  const combo = path.join(ROOT, "..", "2026-09-27-combo");
  const found = new Set();
  const walk = (d) => { for (const f of fs.readdirSync(d)) { const p = path.join(d, f); const st = fs.statSync(p); if (st.isDirectory()) walk(p); else if (/\.(html|js|json|txt|xml)$/.test(f)) for (const m of fs.readFileSync(p, "utf8").matchAll(/(?:playground|studio)\/(?:index\.html)?(\?[^"'\s<>)\\]*)/g)) found.add(m[1].replace(/&amp;/g, "&")); } };
  walk(combo);
  const links = [...found].filter((q) => q.length > 1 && !/\.\.\./.test(q)).sort();
  const TAB_ALIAS = { fom: "overview", figures: "overview", chart: "timeseries", "engine-chart": "timeseries", export: "exports", animations: "animation" };
  const { ctx, page } = await newPage(browser, "desktop");
  const rows = [];
  for (const q of links) {
    const p = new URLSearchParams(q);
    const want = p.get("scenario") ? (/\.toml$/.test(p.get("scenario")) ? p.get("scenario") : p.get("scenario") + ".toml") : null;
    const wantTab = p.get("tab") ? TAB_ALIAS[p.get("tab")] || p.get("tab") : null;
    await page.goto(STUDIO + "index.html" + q);
    if (want) await page.waitForFunction(() => document.body.dataset.screen === "task" && /\S/.test(document.getElementById("m-big").textContent || "") && !/^Opening/.test(document.getElementById("m-big").textContent), null, { timeout: 30000 }).catch(() => {});
    await page.waitForTimeout(want ? 900 : 400);
    const got = await page.evaluate(() => ({ screen: document.body.dataset.screen, url: location.search, tab: (document.querySelector('#tabs [aria-selected="true"]') || {}).dataset?.tab || null, tabs: [...document.querySelectorAll("#tabs [data-tab]")].map((b) => b.dataset.tab), title: document.title }));
    const gotScenario = new URLSearchParams(got.url).get("scenario");
    const okScenario = !want || (got.screen === "task" && gotScenario && gotScenario + ".toml" === want);
    const okTab = !wantTab || got.tab === wantTab || (!got.tabs.includes(wantTab));
    rows.push({ link: q, want, wantTab, ...got, ok: okScenario && okTab && got.tab === (wantTab && got.tabs.includes(wantTab) ? wantTab : got.tab), tabOffered: wantTab ? got.tabs.includes(wantTab) : null });
  }
  await ctx.close();
  const bad = rows.filter((r) => !r.ok);
  const notOffered = rows.filter((r) => r.wantTab && r.tabOffered === false);
  out.gates.links = { target: "every site link opens its scenario and view", total: rows.length, bad, tab_not_offered: notOffered, pass: !bad.length };
  console.log("links", `${rows.length} links, ${bad.length} wrong, ${notOffered.length} ask for a view the scenario does not offer`, out.gates.links.pass ? "PASS" : "FAIL");
  for (const b of bad.slice(0, 10)) console.log("  BAD", b.link, "->", b.url, b.tab);
  for (const n of notOffered.slice(0, 10)) console.log("  view not offered", n.link, "->", n.tab, "of", n.tabs.join(","));
}

// ---------------------------------------------------------------- the Advanced view
// The full dashboard (advanced/): 0 errors, the right scenario, and on a phone a usable layout:
// one header row of 64 px or less, nothing off-screen, no page or strip hiding content sideways.
if (PHASES.includes("advanced")) {
  const rows = [];
  const urls = ["advanced/", "advanced/?scenario=integrity-raim&tab=timeseries", "advanced/?scenario=l-band-waterfall-jamming&tab=spectrum&play=1", "advanced/?scenario=clock-holdover&tab=exports", "advanced/?scenario=constellation-multi-gnss-coverage", "advanced/?scenario=campaign-jam-spoof-holdover-integrity", "index.html?scenario=telecom-prtc-holdover-24h&view=advanced"];
  for (const device of ["desktop", "phone"]) for (const u of urls) {
    const { ctx, page } = await newPage(browser, device);
    await page.goto(STUDIO + u);
    await page.waitForFunction(() => location.pathname.endsWith("/advanced/") && document.getElementById("kpis-wrap") && (!document.getElementById("kpis-wrap").hidden || !document.getElementById("start").hidden), null, { timeout: 30000 }).catch(() => {});
    await page.waitForTimeout(2500);
    const m = await page.evaluate(() => {
      const vis = (el) => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && cs.visibility !== "hidden" && cs.display !== "none"; };
      const ctl = [...document.querySelectorAll("a[href],button,input,select,textarea,[role=tab]")].filter((el) => vis(el) && !el.classList.contains("skip") && !el.closest(".lib,.edit,.pal,.drawer,[hidden]"));
      const off = ctl.filter((el) => { const r = el.getBoundingClientRect(); return r.right <= 0 || r.left >= innerWidth; }).map((el) => `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""} "${(el.textContent || "").trim().slice(0, 24)}"`);
      const boxes = [...document.querySelectorAll("*")].filter((el) => { const cs = getComputedStyle(el); return /(auto|scroll)/.test(cs.overflowX) && el.scrollWidth > el.clientWidth + 1 && vis(el) && !el.closest(".lib,.edit,.pal,.drawer,.code,textarea"); }).map((el) => `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : "." + String(el.className).split(" ")[0]} ${el.scrollWidth}/${el.clientWidth}`);
      return { path: location.pathname + location.search, scenario: new URLSearchParams(location.search).get("scenario"), title: document.title, header: Math.round(document.getElementById("bar").getBoundingClientRect().height), pageSideways: document.scrollingElement.scrollWidth > innerWidth, off, boxes, switchShown: vis(document.getElementById("view-switch")) };
    });
    rows.push({ device, url: u, ...m });
    await ctx.close();
  }
  const pass = rows.every((r) => r.switchShown && (r.device === "desktop" || (r.header <= 64 && !r.pageSideways && !r.off.length && !r.boxes.length)));
  out.gates.advanced = { target: "0 errors; the switch on every page; on a phone header <= 64 px, nothing off-screen or hidden sideways", rows, pass };
  for (const r of rows) console.log("advanced", r.device, r.url, `header ${r.header}`, r.switchShown ? "switch" : "NO-SWITCH", r.pageSideways ? "PAGE-SIDEWAYS" : "", r.off.length ? "off " + r.off.join(", ") : "", r.boxes.length ? "boxes " + r.boxes.join(", ") : "", "|", r.title);
  console.log("advanced", pass ? "PASS" : "FAIL");
}

// ---------------------------------------------------------------- throttled
if (PHASES.includes("throttle")) {
  const rows = [];
  for (const [device, e] of [["phone", ENTRIES[0]], ["phone", ENTRIES[2]], ["desktop", ENTRIES[1]]]) {
    const { ctx, page } = await newPage(browser, device);
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: FAST3G.latency, downloadThroughput: FAST3G.downloadThroughput, uploadThroughput: FAST3G.uploadThroughput });
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: FAST3G.cpu });
    await page.goto(STUDIO + "index.html" + e.q, { timeout: 180000 });
    await waitResult(page, 180000);
    const t1 = await page.evaluate(() => ({ ...window.__t, recordedNote: (document.getElementById("hero-rec") || {}).textContent, loadNote: !(document.getElementById("hero-load") || { hidden: true }).hidden, chip: document.getElementById("engine-label").textContent }));
    await page.screenshot({ path: path.join(HERE, `throttled-${device}-${e.id}-first-result.jpg`), quality: 80, type: "jpeg" });
    await page.waitForFunction(() => "engine" in window.__t, null, { timeout: 240000 }).catch(() => {});
    const t2 = await page.evaluate(() => window.__t);
    rows.push({ device, entry: e.id, first_answer_ms: t1.answer ? Math.round(t1.answer) : null, first_result_ms: Math.round(t1.result), engine_ready_ms: t2.engine ? Math.round(t2.engine) : null, label: t1.label || t1.recordedNote, engine_chip_at_result: t1.chip, before_engine: !t2.engine || t1.result < t2.engine });
    await ctx.close();
  }
  out.gates.throttle = { target: "a labelled recorded result before the engine is ready (Fast 3G + 4x CPU)", rows, pass: rows.every((r) => r.before_engine && /Recorded/.test(r.label || "")) };
  console.log("throttle", JSON.stringify(rows), out.gates.throttle.pass ? "PASS" : "FAIL");
}

await browser.close();
out.gates.errors = { target: "0", count: errors.length, errors: errors.slice(0, 50), harness_noise: harness.length, pass: !errors.length };
console.log("errors", errors.length, errors.slice(0, 8).map((e) => `${e.device} ${e.at}: ${e.text}`).join(" | "));
const file = path.join(HERE, `gates${process.env.PHASES ? "-" + PHASES.join("-") : ""}.json`);
fs.writeFileSync(file, JSON.stringify(out, null, 1));
console.log("wrote", file);
