// Review screenshots of the Studio rebuild: light and dark, desktop (1440x900) and phone (390x844).
//   node REVIEW/shots.mjs        (with the folder served, default http://127.0.0.1:8877/)
// Writes REVIEW/<screen>-<device>-<theme>.jpg (the first screen) and -full.jpg (the whole page)
// for: the opening screen, three task screens, the loading state, "For researchers" opened, the
// switch to the Advanced view, and the Advanced view itself (the full dashboard).
// The loading state is real: the engine file is held back by the browser until the shot is taken.
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const STUDIO = (process.env.STUDIO || "http://127.0.0.1:8877/").replace(/\/?$/, "/");
function loadPlaywright() {
  const npx = path.join(process.env.HOME, ".npm/_npx");
  for (const d of [process.env.PW_DIR, ...(fs.existsSync(npx) ? fs.readdirSync(npx).map((x) => path.join(npx, x)) : [])].filter(Boolean)) {
    try { const pw = createRequire(path.join(d, "noop.js"))("playwright"); if (fs.existsSync(pw.chromium.executablePath())) return pw; } catch { /* next */ }
  }
  throw new Error("playwright not found");
}
const { chromium } = loadPlaywright();
const DEVICES = {
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 },
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
};
const SHOTS = [
  { id: "opening", q: "" },
  { id: "task-raim-integrity", q: "?scenario=integrity-raim" },
  { id: "task-clock-holdover", q: "?scenario=clock-holdover" },
  { id: "task-lband-waterfall", q: "?scenario=l-band-waterfall-jamming&tab=spectrum&play=1" },
  { id: "loading-state", q: "?scenario=integrity-raim", holdEngine: true },
  { id: "loading-opening", q: "", holdEngine: true, only: "desktop" },
  { id: "for-researchers", q: "?scenario=integrity-raim", open: "research" },
  { id: "view-toggle", q: "?scenario=integrity-raim", hover: "#view-switch" },
  { id: "advanced-view-raim", path: "advanced/?scenario=integrity-raim&tab=timeseries", advanced: true },
  { id: "advanced-view-waterfall", path: "advanced/?scenario=l-band-waterfall-jamming&tab=spectrum", advanced: true },
];
const only = process.env.ONLY ? process.env.ONLY.split(",") : null;
const browser = await chromium.launch();
const log = [];
for (const theme of ["light", "dark"]) for (const device of ["desktop", "phone"]) for (const s of SHOTS) {
  if (only && !only.includes(s.id)) continue;
  if (s.only && s.only !== device) continue;
  const ctx = await browser.newContext({ ...DEVICES[device], colorScheme: theme, serviceWorkers: "block" });
  const page = await ctx.newPage();
  const errs = [];
  page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
  page.on("pageerror", (e) => errs.push(e.message));
  let release = null;
  if (s.holdEngine) await page.route("**/pkg/kshana_bg.wasm", (route) => new Promise((res) => { release = () => { route.continue().catch(() => {}); res(); }; }));
  await page.goto(STUDIO + (s.path || "index.html" + s.q));
  if (s.advanced) await page.waitForFunction(() => !document.getElementById("kpis-wrap").hidden, null, { timeout: 30000 });
  else await page.waitForFunction(() => {
    const scr = document.body.dataset.screen;
    if (scr === "home") return !!document.querySelector("#hero-map svg") && document.querySelectorAll("#tiles .tile").length > 10;
    return scr === "task" && document.getElementById("m-big").textContent && !/^Opening/.test(document.getElementById("m-big").textContent);
  }, null, { timeout: 30000 });
  await page.waitForTimeout(s.holdEngine ? 1200 : 3200);
  if (s.hover) { await page.hover(s.hover); await page.focus(s.hover); await page.waitForTimeout(300); }
  if (s.open) {
    await page.evaluate((id) => { const d = document.getElementById(id); d.open = true; d.scrollIntoView({ block: "start" }); window.scrollBy(0, -70); }, s.open);
    await page.waitForTimeout(400);
  }
  const base = path.join(HERE, `${s.id}-${device}-${theme}`);
  await page.screenshot({ path: `${base}.jpg`, type: "jpeg", quality: 82 });
  if (!s.open) { await page.evaluate(() => window.scrollTo(0, 0)); await page.screenshot({ path: `${base}-full.jpg`, type: "jpeg", quality: 78, fullPage: true }); }
  const chip = await page.evaluate(() => document.getElementById("engine-label").textContent);
  log.push(`${path.basename(base)} engine="${chip}" errors=${errs.length}${errs.length ? " " + errs.join(" | ") : ""}`);
  if (release) release();
  await ctx.close();
}
await browser.close();
fs.writeFileSync(path.join(HERE, "shots.log"), log.join("\n") + "\n");
console.log(log.join("\n"));
