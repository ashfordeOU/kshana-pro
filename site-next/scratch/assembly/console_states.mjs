// Capabilities consoles: every design of the constellation designer and every body of the solar-system
// console, one state at a time, with the Studio link its provenance line then carries.
import { createRequire } from "node:module";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const b = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {}); const p = await (await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" })).newPage();
const errs = []; p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); }); p.on("pageerror", (e) => errs.push(String(e)));
await p.goto((process.argv[2] || "http://127.0.0.1:8841/") + "capabilities.html", { waitUntil: "load", timeout: 180000 }); await p.waitForTimeout(3000);
const read = (sel) => p.evaluate((s) => { const e = document.querySelector(s); return e ? { text: e.textContent.replace(/\s+/g, " ").slice(0, 90), links: [...e.querySelectorAll("a")].map((a) => a.getAttribute("href")).filter((h) => /playground/.test(h)) } : null; }, sel);
await p.click("#cct-orbits"); await p.waitForTimeout(1500);
const n = await p.evaluate(() => document.querySelectorAll("#cdList > *").length);
for (let i = 0; i < n; i++) { await p.evaluate((k) => document.querySelectorAll("#cdList > *")[k].click(), i); await p.waitForTimeout(900); console.log("constellation design", i, JSON.stringify(await read('[data-cprov="constellation"]'))); }
await p.click("#cct-solar"); await p.waitForTimeout(2000);
console.log("solar", JSON.stringify(await read('[data-cprov="solar"]')));
const picks = await p.evaluate(() => [...document.querySelectorAll("#ccp-solar button, #ccp-solar [role=tab], #ccp-solar [role=option]")].map((e) => (e.textContent || "").trim().slice(0, 24)));
console.log("solar console controls:", picks.length);
const all = await p.evaluate(() => [...document.querySelectorAll("#ccp-solar .prov")].map((e) => ({ text: e.textContent.replace(/\s+/g, " ").slice(0, 100), links: [...e.querySelectorAll("a")].map((a) => a.getAttribute("href")).filter((h) => /playground/.test(h)) })));
for (const a of all) console.log("solar console provenance:", JSON.stringify(a));
console.log("errors:", errs.length ? errs : "none");
await b.close();
