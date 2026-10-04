// Layout fingerprint of every page: which font each text element resolved to and its box.
//   node font_metrics.mjs <out.json>
import { createRequire } from "node:module";
import { readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const HOME = process.env.HOME;
const require = createRequire(join(HOME, "Code/kshana-site-next/explorations/2026-09-27-audit/matrix/package.json"));
const { chromium } = require("playwright");
const BASE = "http://127.0.0.1:8861/";
const SITE = join(HOME, "Code/kshana-site-next/scratch/port/pages-mirror/site");
const walk = (dir, base = "") => readdirSync(join(dir, base), { withFileTypes: true }).flatMap((e) => { const rel = base ? `${base}/${e.name}` : e.name; return e.isDirectory() ? walk(dir, rel) : [rel]; });
const pages = walk(SITE).filter((f) => f.endsWith(".html") && !f.startsWith("assets/") && !f.startsWith("pkg/") && f !== "404.html").sort();
const browser = await chromium.launch();
const out = {};
const hosts = new Set();
let i = 0;
async function one(rel, w, h, theme) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, reducedMotion: "reduce" });
  await ctx.addInitScript((t) => { try { localStorage.setItem("kshana-theme", t); } catch {} }, theme);
  const page = await ctx.newPage();
  page.on("request", (r) => { const u = new URL(r.url()); if (u.protocol.startsWith("http") && u.host !== "127.0.0.1:8861") hosts.add(u.host); });
  await page.goto(BASE + rel, { waitUntil: "load", timeout: 120000 });
  await page.waitForLoadState("networkidle", { timeout: 60000 }).catch(() => {});
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(500);
  out[`${rel}|${theme}|${w}`] = await page.evaluate(() => {
    const els = [...document.querySelectorAll("h1,h2,h3,.lede,.eyebrow,.nav-links a,.brand,.btn,.foot a,code,.pill,p")].slice(0, 120);
    const rows = els.map((e) => { const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return [e.tagName, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10, c.fontFamily.split(",")[0].replace(/["']/g, ""), c.fontWeight, c.color]; });
    const faces = [...document.fonts].filter((f) => f.status === "loaded").map((f) => `${f.family.replace(/["']/g, "")} ${f.weight}`).sort();
    return { rows, faces: [...new Set(faces)], height: document.documentElement.scrollHeight, bg: getComputedStyle(document.body).backgroundColor };
  });
  await ctx.close();
}
const jobs = pages.flatMap((rel) => [[rel, 1440, 900, "light"], [rel, 1440, 900, "dark"], [rel, 390, 844, "light"], [rel, 390, 844, "dark"]]);
await Promise.all(Array.from({ length: 3 }, async () => { while (i < jobs.length) { const j = jobs[i++]; try { await one(...j); } catch (e) { out[`${j[0]}|${j[3]}|${j[1]}`] = { error: String(e).slice(0, 120) }; } } }));
await browser.close();
writeFileSync(process.argv[2], JSON.stringify({ hosts: [...hosts].sort(), out }, null, 0));
console.log(`${Object.keys(out).length} page views, third-party hosts requested: ${[...hosts].sort().join(", ") || "none"}`);
