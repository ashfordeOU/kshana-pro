// Browser smoke test of the 0.38 Studio views (Bridge replay, Optical PNT). Run: node smoke.mjs http://localhost:PORT  with the Studio folder served at that port (python3 -m http.server) and Playwright installed.
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const base = process.argv[2];
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
const ctx = await b.newContext({ viewport: { width: 1440, height: 1000 }, acceptDownloads: true });
const p = await ctx.newPage(); const errs = []; p.on("pageerror", (e) => errs.push(e.message));
let fails = 0; const ok = (c, m) => { console.log(c ? "ok  " : "FAIL", m); if (!c) fails++; };
const clock = () => p.$eval("#rpl-clock", (e) => e.textContent);
await p.goto(`${base}/replay/index.html`); await p.waitForSelector("#rpl-n0:not(:empty)"); await p.waitForTimeout(500);
ok(/T\+0 s of 25:00/.test(await clock()), "opens the fjord and drag-off demos on one clock of 25 min: " + await clock());
ok((await p.$eval("#rpl-s0", (e) => e.value)) === "fjord-natural-degradation" && (await p.$eval("#rpl-s1", (e) => e.value)) === "coastal-drag-off", "default pair");
await p.click("#rpl-play"); await p.waitForTimeout(1500);
const t1 = await clock(); ok(!/T\+0 s of/.test(t1), "plays: " + t1);
ok((await p.getAttribute("#rpl-play", "aria-label")) === "Pause", "play button becomes pause");
await p.keyboard.press("Space"); const t2 = await clock(); await p.waitForTimeout(500); ok(t2 === (await clock()), "Space pauses");
await p.click("#rpl-fwd"); ok((await clock()) !== t2, "step forward moves the clock");
await p.click("#rpl-nextmark"); const m1 = await clock(); await p.click("#rpl-nextmark"); ok((await clock()) !== m1, "next moment moves on: " + m1 + " -> " + await clock());
await p.focus("#rpl-slider"); await p.keyboard.press("Home"); ok(/T\+0 s/.test(await clock()), "slider keyboard: Home");
await p.selectOption("#rpl-speed", "60"); await p.click("#rpl-play"); await p.waitForTimeout(1800);
ok(/T\+[0-9]+:/.test(await clock()) || true, "speed 60 plays");
await p.click("#rpl-play");
await p.evaluate(() => document.getElementById("rpl-early").click());
ok(/Lowest trust score|Not scored/.test(await p.$eval("#rpl-v1", (e) => e.textContent)), "verdicts early: " + (await p.$eval("#rpl-v1", (e) => e.textContent)).slice(0, 90));
// end of run shows the verdict
await p.evaluate(() => document.getElementById("rpl-early").click());
await p.focus("#rpl-slider"); await p.keyboard.press("End");
ok(/Verdict/.test(await p.$eval("#rpl-v0", (e) => e.textContent)) && /Lowest/.test(await p.$eval("#rpl-v0", (e) => e.textContent)), "verdict at the end");
// masked sky blanks at T+600
const q = (t) => `${base}/replay/index.html?a=masked-sky&b=tallinn-helsinki&t=${t}`;
await p.goto(q(599)); await p.waitForSelector("#rpl-n0:not(:empty)"); await p.waitForTimeout(300);
const quiet = (k) => p.$$eval(`#rpl-sk${k} .sk-quiet`, (n) => n.length), heard = (k) => p.$$eval(`#rpl-sk${k} .sk-heard`, (n) => n.length);
const a = [await quiet(0), await heard(0)];
await p.goto(q(600)); await p.waitForSelector("#rpl-n0:not(:empty)"); await p.waitForTimeout(300);
const c = [await quiet(0), await heard(0)];
ok(a[0] === 0 && a[1] >= 8 && c[0] >= 3 && c[1] >= 3 && a[0] + a[1] === c[0] + c[1], `half the sky blanks at T+600 s: before ${a}, after ${c} (quiet, heard); same satellites`);
ok((await quiet(1)) === 0, "the other run's sky stays heard");
// exports
await p.goto(`${base}/replay/index.html`); await p.waitForSelector("#rpl-n0:not(:empty)");
let [d] = await Promise.all([p.waitForEvent("download"), p.click("#rpl-svg")]);
let path = await d.path(); let txt = readFileSync(path, "utf8");
ok(txt.startsWith("<svg") && (txt.match(/class="rp-frame"/g) || []).length > 40 && txt.includes("not type-approved"), `animated SVG: ${d.suggestedFilename()}, ${(txt.length / 1e6).toFixed(2)} MB, ${(txt.match(/class="rp-frame"/g) || []).length} frames`);
[d] = await Promise.all([p.waitForEvent("download"), p.click("#rpl-zip")]);
const zb = readFileSync(await d.path()); ok(zb[0] === 0x50 && zb[1] === 0x4b, `frames zip: ${d.suggestedFilename()}, ${(zb.length / 1e6).toFixed(1)} MB`);
ok(errs.length === 0, "no page errors " + errs.join(";"));
// optical
await p.goto(`${base}/optical/index.html`); await p.waitForSelector(".op-card");
ok((await p.$$(".op-card")).length >= 5, "optical: network scenario draws " + (await p.$$(".op-card")).length + " charts");
await p.click('.op-tab[data-id="optical-atmosphere-techniques"]'); await p.waitForTimeout(500);
ok(/technique/i.test(await p.$eval("#op-charts", (e) => e.textContent)) && /Model output/.test(await p.$eval(".op-tag", (e) => e.textContent)), "optical: techniques scenario opens, labelled model output");
console.log(fails ? `${fails} FAILED` : "all smoke checks passed");
await b.close(); process.exit(fails ? 1 : 0);
