// Headless check of the ported site as served from the staged deploy tree.
//   node check_site.mjs [base=http://127.0.0.1:8861/] [siteDir] [repoWebDir]
// Every page x {light,dark} x {390,1440}: console errors, failed requests, screenshot.
// Every internal link: target loads and its anchor exists in the loaded DOM.
// The Studio: WASM engine goes live and runs a scenario. Every old address lands.
import { createRequire } from "node:module";
import { readFileSync, mkdirSync, writeFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
const HOME = process.env.HOME;
const require = createRequire(join(HOME, "Code/kshana-site-next/explorations/2026-09-27-audit/matrix/package.json"));
const { chromium } = require("playwright");

const BASE = process.argv[2] || "http://127.0.0.1:8861/";
const SITE = process.argv[3] || join(HOME, "Code/kshana-site-next/scratch/port/pages-mirror/site");
const WEB = process.argv[4] || join(HOME, "Code/kshana-wt-site/web");
const OUT = join(HOME, "Code/kshana-site-next/scratch/port");
const SHOTS = join(OUT, "shots");
mkdirSync(SHOTS, { recursive: true });

const walk = (dir, base = "") => readdirSync(join(dir, base), { withFileTypes: true }).flatMap((e) => {
  const rel = base ? `${base}/${e.name}` : e.name;
  return e.isDirectory() ? walk(dir, rel) : [rel];
});
const pages = walk(SITE).filter((f) => f.endsWith(".html") && !f.startsWith("assets/") && !f.startsWith("pkg/")).sort();
const reports = walk(SITE).filter((f) => f.endsWith(".html") && f.startsWith("assets/")).sort();
const combos = [["light", 1440, 900], ["dark", 1440, 900], ["light", 390, 844], ["dark", 390, 844]];
const CONC = Number(process.env.CONC || 3);

const browser = await chromium.launch();
const problems = [];
const domIds = new Map(); // page -> Set(ids) from the live DOM (1440 light)
const domLinks = new Map(); // page -> [href]
let loads = 0;
const thirdParty = new Set(); // every host other than the site's own that any page asked for

async function visit(rel, theme, w, h, shot) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, reducedMotion: "reduce" });
  await ctx.addInitScript((t) => { try { localStorage.setItem("kshana-theme", t); } catch {} }, theme);
  const page = await ctx.newPage();
  const errs = [];
  page.on("console", (m) => { if (m.type() === "error") errs.push(`console: ${m.text().slice(0, 200)}`); });
  page.on("pageerror", (e) => errs.push(`pageerror: ${String(e).slice(0, 200)}`));
  page.on("requestfailed", (r) => errs.push(`requestfailed: ${r.url().slice(0, 140)} ${r.failure()?.errorText || ""}`));
  page.on("response", (r) => { if (r.status() >= 400) errs.push(`http ${r.status()}: ${r.url().slice(0, 140)}`); });
  page.on("request", (r) => { const u = r.url(); if (/^https?:/.test(u) && !u.startsWith(BASE)) { thirdParty.add(new URL(u).host); errs.push(`third-party request: ${u.slice(0, 140)}`); } });
  const url = BASE + rel;
  try {
    await page.goto(url, { waitUntil: "load", timeout: 120000 });
    await page.waitForLoadState("networkidle", { timeout: 60000 }).catch(() => {});
    await page.waitForTimeout(600);
    loads += 1;
    const applied = await page.evaluate(() => document.documentElement.getAttribute("data-theme") || document.documentElement.dataset.theme || "");
    if (applied !== theme && !rel.startsWith("assets/")) errs.push(`theme: asked ${theme}, page has data-theme="${applied}"`);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    if (overflow > 2) errs.push(`horizontal overflow ${overflow}px at ${w}`);
    if (theme === "light" && w === 1440) {
      domIds.set(rel, new Set(await page.evaluate(() => [...document.querySelectorAll("[id]")].map((e) => e.id))));
      domLinks.set(rel, await page.evaluate(() => [...document.querySelectorAll("a[href]")].map((a) => a.getAttribute("href"))));
    }
    if (shot) await page.screenshot({ path: join(SHOTS, `${rel.replace(/[\/]/g, "__").replace(/\.html$/, "")}--${theme}-${w}.png`), timeout: 60000 }).catch((e) => errs.push(`screenshot: ${String(e).slice(0, 80)}`));
  } catch (e) {
    errs.push(`load: ${String(e).slice(0, 200)}`);
  }
  await ctx.close();
  for (const e of errs) problems.push(`${rel} [${theme} ${w}] ${e}`);
}

async function pool(jobs) {
  let i = 0;
  await Promise.all(Array.from({ length: CONC }, async () => { while (i < jobs.length) { const j = jobs[i++]; await j(); } }));
}

// ---- 1. every page, four ways
const t0 = Date.now();
await pool(pages.flatMap((rel) => combos.map(([theme, w, h]) => () => visit(rel, theme, w, h, true))));
console.log(`pages: ${pages.length} x ${combos.length} = ${loads} loads in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
// engine reports shipped as assets: load once each
await pool(reports.map((rel) => () => visit(rel, "light", 1440, 900, false)));

// ---- 2. every internal link from the live DOM
const linkProblems = [];
const seen = new Set();
let nLinks = 0;
for (const [rel, hrefs] of domLinks) {
  for (const href of hrefs) {
    if (!href || /^(mailto:|tel:|javascript:)/.test(href)) continue;
    const u = new URL(href, BASE + rel);
    if (u.origin !== new URL(BASE).origin) continue;
    nLinks += 1;
    let path = decodeURIComponent(u.pathname.slice(1)) || "index.html";
    if (path.endsWith("/")) path += "index.html";
    const key = path + u.hash;
    if (seen.has(key)) continue;
    seen.add(key);
    let ok = true;
    try { readFileSync(join(SITE, path)); } catch { ok = false; }
    if (!ok) { linkProblems.push(`${rel}: ${href} -> ${path} is not in the deploy tree`); continue; }
    const frag = decodeURIComponent(u.hash.slice(1));
    if (frag && path.endsWith(".html") && !frag.startsWith("s=")) {
      const ids = domIds.get(path);
      if (ids && !ids.has(frag)) linkProblems.push(`${rel}: ${href} -> no element with id "${frag}" in ${path}`);
    }
  }
}
console.log(`links: ${nLinks} internal links in the live DOM, ${seen.size} unique targets, ${linkProblems.length} broken`);

// ---- 3. the Studio: live WASM engine, runs a scenario (each theme and width)
const studio = [];
for (const [theme, w, h] of combos) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme });
  await ctx.addInitScript((t) => { try { localStorage.setItem("kshana-theme", t); } catch {} }, theme);
  const page = await ctx.newPage();
  const errs = [];
  const wasm = [];
  page.on("console", (m) => { if (m.type() === "error") errs.push(m.text().slice(0, 200)); });
  page.on("pageerror", (e) => errs.push(String(e).slice(0, 200)));
  page.on("request", (r) => { const u = r.url(); if (/^https?:/.test(u) && !u.startsWith(BASE)) { thirdParty.add(new URL(u).host); errs.push(`third-party request: ${u.slice(0, 120)}`); } });
  page.on("response", (r) => { if (r.url().endsWith(".wasm")) wasm.push(`${r.status()} ${r.url().replace(BASE, "")}`); if (r.status() >= 400) errs.push(`http ${r.status()} ${r.url().slice(0, 120)}`); });
  const res = { theme, w };
  try {
    await page.goto(BASE + "playground/index.html?scenario=clock-holdover&tab=overview", { waitUntil: "load", timeout: 120000 });
    await page.waitForFunction(() => document.getElementById("engine-chip")?.dataset.mode && document.getElementById("engine-chip").dataset.mode !== "loading", null, { timeout: 180000 });
    res.mode = await page.evaluate(() => document.getElementById("engine-chip").dataset.mode);
    res.label = await page.evaluate(() => document.getElementById("engine-label").textContent);
    await page.waitForFunction(() => /^Ran locally|^Recorded run shown|^Run failed/.test(document.getElementById("status").textContent), null, { timeout: 180000 });
    res.first = await page.evaluate(() => document.getElementById("status").textContent);
    // a second, explicit run of a different scenario through the UI's Run button
    await page.goto(BASE + "playground/index.html?scenario=integrity-raim&tab=overview", { waitUntil: "load", timeout: 120000 });
    await page.waitForFunction(() => /^Ran locally|^Recorded run shown|^Run failed/.test(document.getElementById("status").textContent), null, { timeout: 180000 });
    // Below 860 px the Studio shows one pane at a time; Run lives in the Scenario pane.
    if (!(await page.isVisible("#run"))) await page.click('button[data-pane="editor"]');
    await page.click("#run");
    await page.waitForFunction(() => /run 2\b/.test(document.getElementById("status").textContent) || /^Run failed/.test(document.getElementById("status").textContent), null, { timeout: 180000 });
    res.second = await page.evaluate(() => document.getElementById("status").textContent);
    res.summary = (await page.evaluate(() => document.getElementById("summary").textContent)).slice(0, 160);
    res.errorShown = await page.evaluate(() => !document.getElementById("error").hidden);
    // the 3-D orbit view loads three.js and OrbitControls: they must come from this site
    await page.goto(BASE + "playground/index.html?scenario=orbit-sgp4-gps&tab=orbit", { waitUntil: "load", timeout: 120000 });
    await page.waitForFunction(() => /^Ran locally|^Run failed/.test(document.getElementById("status").textContent), null, { timeout: 180000 });
    await page.waitForFunction(() => !!document.querySelector("#v-orbit canvas"), null, { timeout: 60000 }).catch(() => {});
    res.orbit = await page.evaluate(() => ({ tab: document.querySelector('#tabs [aria-selected="true"]')?.dataset.tab, canvas: !!document.querySelector("#v-orbit canvas") }));
    await page.screenshot({ path: join(SHOTS, `studio-run--${theme}-${w}.png`), timeout: 60000 }).catch(() => {});
  } catch (e) { res.fail = String(e).slice(0, 240); }
  res.wasm = wasm; res.errors = errs;
  studio.push(res);
  await ctx.close();
}

// ---- 4. every old address
const legacy = JSON.parse(readFileSync(join(WEB, "tools/legacy-urls.json"), "utf8"));
const old = [];
{
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  for (const c of legacy.cases) {
    const page = await ctx.newPage();
    const r = { url: c.url, expect: c.to || "(stays)" };
    try {
      if (!c.to && !/(\/|\.html|\.svg|\.png|\.txt|\.xml|\.json|[#?].*)$/.test(c.url)) {
        // not a document a browser displays (a .wasm, a .toml): fetch it
        const got = await ctx.request.get(BASE + c.url.slice(1));
        r.status = got.status(); r.final = c.url; r.ok = got.status() === 200 && (await got.body()).length > 100;
        old.push(r); await page.close(); continue;
      }
      const resp = await page.goto(BASE + c.url.slice(1), { waitUntil: "load", timeout: 120000 });
      await page.waitForTimeout(400);
      r.status = resp ? resp.status() : null;
      const fin = page.url().replace(BASE, "/");
      r.final = fin;
      if (c.to) {
        r.ok = fin === "/" + c.to;
        const frag = c.to.includes("#") ? c.to.split("#")[1] : "";
        if (r.ok && frag && !frag.startsWith("s=")) r.ok = await page.evaluate((id) => !!document.getElementById(id), frag);
        if (r.ok && frag.startsWith("s=")) {
          await page.waitForFunction(() => /shared scenario|^Ran locally|needs the live engine/.test(document.getElementById("status")?.textContent || "") || !document.getElementById("error")?.hidden, null, { timeout: 120000 }).catch(() => {});
          r.studio = await page.evaluate(() => `${document.getElementById("status").textContent} | toml: ${document.getElementById("toml").value.trim()}`);
          r.ok = /seed = 42/.test(r.studio); // the payload arrived; this sample is not a whole scenario, so it cannot run
        }
        if (r.ok && c.url.includes("embed=1")) {
          r.ok = await page.evaluate(() => document.body.classList.contains("embed"));
          const want = new URLSearchParams(c.to.split("?")[1]).get("tab");
          if (r.ok && want && c.to.includes("scenario=")) {
            await page.waitForFunction(() => /^Ran locally|^Run failed/.test(document.getElementById("status")?.textContent || ""), null, { timeout: 180000 });
            r.studio = "tab " + await page.evaluate(() => document.querySelector('#tabs [aria-selected="true"]')?.dataset.tab);
            r.ok = r.studio === "tab " + want;
          }
        }
      } else {
        r.ok = r.status === 200 && fin === c.url.replace(/^\/index\.html$/, "/index.html");
        if (c.id) r.ok = r.ok && await page.evaluate((id) => !!document.getElementById(id), c.id);
      }
    } catch (e) { r.ok = false; r.err = String(e).slice(0, 160); }
    old.push(r);
    await page.close();
  }
  // A real share link, as the old Share button made it: a whole scenario in the fragment.
  {
    const toml = readFileSync(join(SITE, "scenarios/clock-holdover.toml"), "utf8");
    const frag = "#s=" + Buffer.from(toml, "utf8").toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    const page = await ctx.newPage();
    const r = { url: "/#s=<clock-holdover.toml, " + toml.length + " chars>", expect: "playground/index.html#s=…, runs" };
    try {
      await page.goto(BASE + frag, { waitUntil: "load", timeout: 120000 });
      await page.waitForFunction(() => /^Ran locally|^Run failed/.test(document.getElementById("status")?.textContent || ""), null, { timeout: 180000 });
      r.final = page.url().replace(BASE, "/").slice(0, 40) + "…";
      r.studio = await page.evaluate(() => document.getElementById("status").textContent);
      r.ok = page.url().startsWith(BASE + "playground/index.html#s=") && /^Ran locally/.test(r.studio) && (await page.evaluate(() => document.getElementById("toml").value)) === toml;
    } catch (e) { r.ok = false; r.err = String(e).slice(0, 160); }
    old.push(r);
    await page.close();
  }
  await ctx.close();
}

// ---- 5. a missing address, at the root and three folders deep: status 404, the site's own page
const notFound = [];
for (const [theme, w, h] of combos) {
  for (const path of ["no-such-page", "docs/old/removed/page.html"]) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, reducedMotion: "reduce" });
    await ctx.addInitScript((t) => { try { localStorage.setItem("kshana-theme", t); } catch {} }, theme);
    const page = await ctx.newPage();
    const errs = [];
    page.on("console", (m) => { if (m.type() === "error" && !/404 \(Not Found\)/.test(m.text())) errs.push(m.text().slice(0, 160)); });
    page.on("pageerror", (e) => errs.push(String(e).slice(0, 160)));
    page.on("requestfailed", (r) => errs.push(`requestfailed ${r.url().slice(0, 120)}`));
    page.on("request", (r) => { const u = r.url(); if (/^https?:/.test(u) && !u.startsWith(BASE)) { thirdParty.add(new URL(u).host); errs.push(`third-party request: ${u.slice(0, 120)}`); } });
    const bad = [];
    page.on("response", (r) => { if (r.status() >= 400 && r.url() !== BASE + path) bad.push(`${r.status()} ${r.url().replace(BASE, "/")}`); });
    const r = { path, theme, w };
    try {
      const resp = await page.goto(BASE + path, { waitUntil: "load", timeout: 120000 });
      await page.waitForLoadState("networkidle", { timeout: 60000 }).catch(() => {});
      await page.evaluate(() => document.fonts.ready);
      r.status = resp.status();
      r.h1 = await page.evaluate(() => document.querySelector("h1")?.textContent || "");
      r.styled = await page.evaluate(() => getComputedStyle(document.querySelector("h1")).fontFamily.includes("Geist") && document.querySelector(".nav").getBoundingClientRect().height > 30 && [...document.fonts].some((f) => f.status === "loaded"));
      r.icons = await page.evaluate(() => { const u = document.querySelector(".btn svg use"); return !!u && u.getBoundingClientRect().width > 4; });
      r.links = await page.evaluate(() => [...document.querySelectorAll("#main a")].map((a) => a.href));
      r.overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      await page.screenshot({ path: join(SHOTS, `404-${path.includes("/") ? "deep" : "root"}--${theme}-${w}.png`), timeout: 60000 }).catch(() => {});
      await page.click("#main a.btn-ink");
      await page.waitForLoadState("load");
      r.home = page.url();
      r.ok = r.status === 404 && /No page at this address/.test(r.h1) && r.styled && r.icons && r.overflow <= 2 && bad.length === 0 && errs.length === 0 && r.home === BASE + "index.html"
        && r.links.join() === [BASE + "index.html", BASE + "docs/index.html", BASE + "playground/index.html"].join();
    } catch (e) { r.ok = false; r.err = String(e).slice(0, 200); }
    r.errs = errs; r.bad = bad;
    notFound.push(r);
    await ctx.close();
  }
}
await browser.close();

const report = { thirdPartyHosts: [...thirdParty].sort(), notFound, base: BASE, pages: pages.length, combos: combos.map((c) => `${c[0]}-${c[1]}`), loads, problems, links: { total: nLinks, unique: seen.size, broken: linkProblems }, studio, old };
writeFileSync(join(OUT, "browser-check.json"), JSON.stringify(report, null, 1));
console.log(`page problems: ${problems.length}`);
for (const p of problems.slice(0, 60)) console.log("  " + p);
for (const p of linkProblems.slice(0, 40)) console.log("  LINK " + p);
for (const s of studio) console.log(`studio ${s.theme}-${s.w}: mode=${s.mode} | ${s.label} | ${s.first} | ${s.second} | wasm=${s.wasm.join(",")} | orbit=${JSON.stringify(s.orbit)} | errors=${s.errors.length}${s.errors.length ? " " + s.errors.slice(0, 3).join(" ; ") : ""}${s.fail ? " | FAIL " + s.fail : ""}`);
for (const n of notFound) console.log(`404 ${n.ok ? "OK  " : "FAIL"} /${n.path} [${n.theme} ${n.w}] status=${n.status} styled=${n.styled} icons=${n.icons} home=${(n.home || "").replace(BASE, "/")}${n.bad.length ? " bad=" + n.bad.join(",") : ""}${n.errs.length ? " errs=" + n.errs.join(" | ") : ""}${n.err ? " " + n.err : ""}`);
console.log(`third-party hosts requested by any page: ${[...thirdParty].sort().join(", ") || "none"}`);
for (const o of old) console.log(`old ${o.ok ? "OK  " : "FAIL"} ${o.url} -> ${o.final}${o.studio ? " [" + o.studio + "]" : ""}${o.err ? " " + o.err : ""}`);
const bad = problems.length + linkProblems.length + studio.filter((s) => s.mode !== "live" || s.fail || s.errors.length || !/^Ran locally/.test(s.second || "") || !(s.orbit && s.orbit.tab === "orbit" && s.orbit.canvas)).length + old.filter((o) => !o.ok).length
  + notFound.filter((n) => !n.ok).length + thirdParty.size;
console.log(bad ? `BROWSER CHECK: ${bad} problem(s)` : "BROWSER CHECK: clean");
process.exit(bad ? 1 : 0);
