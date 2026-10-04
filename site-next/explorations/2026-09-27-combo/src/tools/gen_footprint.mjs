// Recompute the Missions jamming footprint (FOOTPRINT in src/js/missions.mjs) with the package the
// Studio runs (src/data/sources.json: studio, then its pkg/): one run of the Studio's
// scenarios/maritime-strait-jamming.toml per grid cell, with only [receiver] lat_deg and lon_deg
// changed, to the same three decimals the page's "open this run" link hands the Studio, so a cell
// shows exactly what its link reproduces. Writes js10 (mean J/S in tenths of a dB), av (tracking
// availability under jamming x1000) and the engine version.
// Run: node src/tools/gen_footprint.mjs [--check]   (Node.js 18 or newer; from the site folder)
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
const SITE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const cfg = JSON.parse(fs.readFileSync(path.join(SITE, "src/data/sources.json"), "utf8"));
const S = path.resolve(SITE, process.env.KSHANA_STUDIO || cfg.studio) + "/";
const M = path.join(SITE, "src/js/missions.mjs");
const k = await import(S + "pkg/kshana.js");
k.initSync({ module: fs.readFileSync(S + "pkg/kshana_bg.wasm") });
const g = await import(S + "lib/guided.mjs");
const src = fs.readFileSync(M, "utf8");
const i0 = src.indexOf("const FOOTPRINT = {"), blk = src.slice(i0, src.indexOf("};", i0));
const num = (key) => Number(blk.match(new RegExp("\\b" + key + ":\\s*([-0-9.]+)"))[1]);
const F = { file: blk.match(/file: "([^"]+)"/)[1], jlat: num("jlat"), jlon: num("jlon"), step: num("step"), nx: num("nx"), ny: num("ny") };
const toml = fs.readFileSync(S + "scenarios/" + F.file + ".toml", "utf8");
const kmLat = 111.2, kmLon = 111.32 * Math.cos(F.jlat * Math.PI / 180);  // as cellLatLon() in missions.mjs
const js10 = [], av = [];
for (let j = -F.ny; j <= F.ny; j++) for (let i = -F.nx; i <= F.nx; i++) {
  let t = g.patchSectionScalar(toml, "receiver", "lat_deg", (F.jlat + j * F.step / kmLat).toFixed(3));
  t = g.patchSectionScalar(t, "receiver", "lon_deg", (F.jlon + i * F.step / kmLon).toFixed(3));
  const r = JSON.parse(k.run(t)).fom;
  js10.push(Math.round(r.mean_js_db * 10)); av.push(Math.round(r.availability_under_jamming * 1000));
}
let s = src.replace(/(const FOOTPRINT = \{\n  engine: ")[^"]+"/, `$1${k.version()}"`);
s = s.replace(/\bjs10: \[[^\]]+\]/, `js10: [${js10}]`).replace(/\bav: \[[^\]]+\]/, `av: [${av}]`);
if (process.argv.includes("--check")) {
  if (s !== src) { console.error("src/js/missions.mjs FOOTPRINT is stale: run node src/tools/gen_footprint.mjs"); process.exit(1); }
  console.log(`FOOTPRINT matches engine ${k.version()} (${js10.length} cells)`);
} else {
  fs.writeFileSync(M, s);
  console.log(`wrote FOOTPRINT: engine ${k.version()}, ${js10.length} cells`);
}
