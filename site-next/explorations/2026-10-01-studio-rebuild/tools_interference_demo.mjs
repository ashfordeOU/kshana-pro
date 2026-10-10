// SPDX-License-Identifier: AGPL-3.0-only
// Builds interference/demo/ (the interference map page's SYNTHETIC demo), two parts:
//   1. copies the CLI's own synthetic sample outputs (examples/interference-map/output/*.geojson of the
//      public repository), unchanged, so the demo shows exactly what `kshana interference-map` writes;
//   2. writes one extra file the samples do not cover, a CONFOUNDED ADS-B day (adsb-custom-2026-03-02.geojson), built
//      from the copied ADS-B sample's own method, grid and data blocks (methods v2), with
//      a fixed seed, so the page's day picker and the confounded-day wording can be shown.
// Every number is made up: no tracking source, partner data or real day is involved, and each file's data
// block says so. Format: kshana-interference-map/v1, format_version 1 (docs/INTERFERENCE-MAP.md).
//   node tools_interference_demo.mjs --repo <checkout holding examples/interference-map>
import { writeFileSync, mkdirSync } from "node:fs";
const here = new URL(".", import.meta.url).pathname;
const CELL = 0.5;
let seed = 20260301;
const rnd = () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; };
const ri = (a, b) => a + Math.floor(rnd() * (b - a + 1));
const r4 = (x) => Math.round(x * 1e4) / 1e4;
const feature = (i, j, props) => {
  const s = -90 + i * CELL, w = -180 + j * CELL, n = s + CELL, e = w + CELL;
  return { type: "Feature", properties: { ...props, cell_i: i, cell_j: j }, geometry: { type: "Polygon", coordinates: [[[w, s], [e, s], [e, n], [w, n], [w, s]]] } };
};
// The same made-up open-ocean region as the CLI's synthetic sample (cell rows 238 to 243, columns 257 to 267).
const rows = [], cols = [];
for (let i = 238; i <= 243; i++) rows.push(i);
for (let j = 257; j <= 267; j++) cols.push(j);
const centre = (i, j) => [-90 + (i + 0.5) * CELL, -180 + (j + 0.5) * CELL];
const dist = (i, j, la, lo) => { const [a, b] = centre(i, j); return Math.hypot(a - la, (b - lo) * 0.5); };
function adsbDay(date, confounded, tmpl) {
  const raw = [];
  for (const i of rows) for (const j of cols) {
    if (rnd() < 0.22 + (i === rows[0] || i === rows[rows.length - 1] ? 0.2 : 0)) continue; // not observed
    const observed = ri(2, 60);
    if (observed < 5) continue; // suppressed below the publication minimum
    const sampled = observed < 14 ? ri(0, 9) : ri(10, observed);
    const hot = dist(i, j, 30.25, -49.25) < 0.8;
    const bg = confounded ? 0.2 : 0.04;
    const share = sampled ? Math.min(1, Math.max(0, bg + (hot ? 0.5 : 0) + (rnd() - 0.5) * 0.1)) : 0;
    const affected = Math.round(share * sampled);
    raw.push({ i, j, observed, sampled, affected, share: sampled ? affected / sampled : 0 });
  }
  const full = raw.filter((c) => c.sampled >= 10).map((c) => c.share).sort((a, b) => a - b);
  const median = full.length >= 5 ? (full.length % 2 ? full[(full.length - 1) / 2] : (full[full.length / 2 - 1] + full[full.length / 2]) / 2) : null;
  const dayConfounded = median !== null && median >= 0.15;
  const feats = raw.map((c) => {
    let status = "not_degraded";
    const exceeds = c.sampled >= 10 && c.share >= 0.3 && median !== null && c.share >= median + 0.15;
    if (c.sampled < 10) status = "insufficient_sample"; else if (exceeds && dayConfounded) status = "withheld_day_confounded"; else if (exceeds) status = "degraded";
    // Methods v2 withhold counts below the publication minimum of 5: fewer than 5 sampled aircraft publishes nulls, never zero.
    const w = c.sampled < 5;
    return feature(c.i, c.j, { aircraft_observed: c.observed, aircraft_sampled: w ? null : c.sampled, aircraft_affected: w ? null : c.affected, affected_share: w ? null : r4(c.share), status, degraded: status === "degraded" });
  });
  const t = tmpl.kshana_interference_map;
  return { type: "FeatureCollection", kshana_interference_map: {
    schema: t.schema, format_version: t.format_version, source_kind: "adsb", date, grid: t.grid,
    method: { ...t.method, input_stats: { ...t.method.input_stats, note: "synthetic fixture built from the sample file's method block; no input rows" }, caveats: ["SYNTHETIC FIXTURE: every number in this file is made up for the Studio demonstration.", ...t.method.caveats] },
    day: { background_evaluated: median !== null, background_share: median === null ? null : r4(median), day_confounded: dayConfounded, cells_published: feats.length, cells_suppressed_below_min_distinct: 0 },
    data: t.data, kshana_version: t.kshana_version, notice: t.notice,
  }, features: feats };
}

import { copyFileSync, readdirSync, readFileSync } from "node:fs";
const ri2 = process.argv.indexOf("--repo");
if (ri2 < 0 || !process.argv[ri2 + 1]) { console.error("usage: node tools_interference_demo.mjs --repo <checkout>"); process.exit(2); }
const samples = process.argv[ri2 + 1] + "/examples/interference-map/output/";
mkdirSync(here + "interference/demo", { recursive: true });
for (const f of readdirSync(samples).filter((x) => x.endsWith(".geojson")).sort()) { copyFileSync(samples + f, here + "interference/demo/" + f); console.log("copied", f); }
const tmpl = JSON.parse(readFileSync(samples + "adsb-custom-2026-03-01.geojson", "utf8"));
const doc = adsbDay("2026-03-02", true, tmpl);
writeFileSync(here + "interference/demo/adsb-custom-2026-03-02.geojson", JSON.stringify(doc));
console.log("adsb-custom-2026-03-02.geojson", doc.features.length, "cells (confounded: " + doc.kshana_interference_map.day.day_confounded + ")");
// the CLI sample's synthetic CSV inputs, for the "Build a map here" panel (engine interference_map), plus a short route
mkdirSync(here + "interference/demo/input", { recursive: true });
for (const f of ["adsb.csv", "ais.csv", "land.geojson"]) copyFileSync(process.argv[ri2 + 1] + "/examples/interference-map/input/" + f, here + "interference/demo/input/" + f);
writeFileSync(here + "interference/demo/input/route.csv", "lat,lon\n30.2,-50.0\n30.3,-48.75\n30.4,-47.5\n");
console.log("copied interference/demo/input/ (adsb.csv, ais.csv, land.geojson, route.csv)");
