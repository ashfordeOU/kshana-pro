import { createRequire } from "node:module";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const [base, dir, out] = process.argv.slice(2);
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
let fails = 0; const ok = (c, m) => { console.log(c ? "ok  " : "FAIL", m); if (!c) fails++; };
for (const [w, h, scheme] of [[1440, 1000, "light"], [1440, 1000, "dark"], [390, 900, "light"], [390, 900, "dark"]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme, acceptDownloads: true });
  const p = await ctx.newPage(); const errs = []; p.on("pageerror", (e) => errs.push(e.message)); p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
  const main = w === 1440 && scheme === "light";
  // Trust: NovAtel
  await p.goto(`${base}/trust/index.html`); await p.waitForSelector("#mt-gen-file", { state: "attached" }); await p.waitForTimeout(1200);
  ok((await p.$$eval("#mt-gen-format option", (o) => o.length)) === 9, "format picker lists 9 formats");
  await p.setInputFiles("#mt-gen-file", `${dir}/../nv/novatel-self-report.log`); await p.waitForTimeout(3000);
  const st = await p.$eval("#mt-gen-status", (e) => e.textContent);
  const fmt = await p.$eval("#mt-gen-format", (e) => e.value);
  const gout = await p.$eval("#mt-gen-out", (e) => e.textContent);
  if (main) { ok(fmt === "novatel", "format sniffed from the bytes, not the .log name: " + fmt); }
  await p.selectOption("#mt-gen-format", "novatel"); await p.check("#mt-gen-vessel"); await p.click("#mt-gen-run"); await p.waitForTimeout(3000);
  const st2 = await p.$eval("#mt-gen-status", (e) => e.textContent), go2 = await p.$eval("#mt-gen-out", (e) => e.textContent);
  if (main) { ok(/Assessed .* as NovAtel/.test(st2), "novatel assessed: " + st2.slice(0, 140)); ok(/own flags/i.test(go2) && /Receiver flags/.test(go2), "self-report summary shown: " + go2.slice(0, 260)); }
  await p.screenshot({ path: `${out}/trust-novatel-generic-${w}-${scheme}.png`, fullPage: true });
  {
    await p.evaluate(() => [...document.querySelectorAll("#mt-gen-out button")].find((x) => /replay below/.test(x.textContent)).click()); await p.waitForTimeout(2500);
    const vis = await p.$eval("#mt-self-card", (e) => !e.hidden);
    if (main) { ok(vis, "the receiver's own flag card appears for the NovAtel result"); }
    await p.evaluate(() => document.getElementById("mt-self-card").scrollIntoView());
    if (vis) { await p.focus("#mt-slider"); await p.keyboard.press("Home"); for (let i = 0; i < 150; i++) await p.keyboard.press("ArrowRight"); await p.waitForTimeout(300);
      const now = await p.$eval("#mt-self-now", (e) => e.textContent); if (main) ok(/flag|alarm|report/i.test(now), "strip at 150 s: " + now.slice(0, 160)); }
    await p.screenshot({ path: `${out}/trust-novatel-self-report-${w}-${scheme}.png`, fullPage: true });
  }
  // Trust: RTKLIB .pos
  if (main) {
    await p.setInputFiles("#mt-gen-file", `${dir}/track.pos`); await p.waitForTimeout(2500);
    const s3 = await p.$eval("#mt-gen-status", (e) => e.textContent);
    ok(await p.$eval("#mt-gen-format", (e) => e.value) === "rtklib-pos", "pos guessed");
    console.log("pos status:", s3.slice(0, 200));
  }
  if (main) for (const [f, want, label] of [["rtcm3-msm4.bin", "rtcm3", "RTCM 3"], ["sbf-synthetic.sbf", "sbf", "Septentrio SBF"]]) {
    await p.goto(`${base}/trust/index.html`); await p.waitForSelector("#mt-gen-file", { state: "attached" }); await p.waitForTimeout(1000);
    await p.setInputFiles("#mt-gen-file", `${dir}/${f}`); await p.waitForTimeout(2500);
    const g = await p.$eval("#mt-gen-format", (e) => e.value), s4 = await p.$eval("#mt-gen-status", (e) => e.textContent), o4 = await p.$eval("#mt-gen-out", (e) => e.textContent);
    ok(g === want && new RegExp(label).test(s4) && /first alarm at \d+/.test(o4), `${label}: sniffed ${g}; ${o4.slice(0, 130)}`);
    await p.screenshot({ path: `${out}/trust-${want}-synthetic-${w}-${scheme}.png`, fullPage: false });
  }
  // Tools: IMU fit
  await p.goto(`${base}/tools/index.html#imufit`); await p.waitForSelector("#if-file", { state: "attached" }); await p.waitForTimeout(1500);
  await p.setInputFiles("#if-file", `${dir}/imu-still.csv`); await p.waitForTimeout(300);
  await p.click("#if-run"); await p.waitForTimeout(2500);
  const fo = await p.$eval("#if-out", (e) => e.textContent);
  if (main) { ok(/MODELLED/.test(fo) && /q_va/.test(fo) && /accel_classical/.test(fo), "imu fit shows rows and the toml: " + fo.slice(0, 200)); }
  await p.screenshot({ path: `${out}/tools-imufit-${w}-${scheme}.png`, fullPage: true });
  // Tools: MQTT
  if (main) {
    await p.goto(`${base}/tools/index.html#cli`); await p.waitForSelector("#te-fmt", { state: "attached" }); await p.waitForTimeout(1200);
    await p.setInputFiles("#te-file", `${dir}/stream.jsonl`); await p.selectOption("#te-fmt", "mqtt");
    await p.evaluate(() => [...document.querySelectorAll(".tl-tele button")].find((x) => x.textContent === "Run").click()); await p.waitForTimeout(2000);
    const t = await p.$eval(".tl-tele .tl-out", (e) => e.textContent);
    ok(/homeassistant\/sensor\/kshana\/score\/config/.test(t) && /kshana\/trust\/state/.test(t), "mqtt lines: " + t.slice(0, 160));
  }
  ok(errs.length === 0, `no page errors ${w} ${scheme} ${errs.join("|").slice(0, 200)}`);
  await ctx.close();
}
await b.close(); process.exit(fails ? 1 : 0);
