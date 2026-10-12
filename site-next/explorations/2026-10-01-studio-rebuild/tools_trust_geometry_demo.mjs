// SPDX-License-Identifier: AGPL-3.0-only
// Writes the Maritime trust view's geometry demo (0.36) and its review fixture.
//
//   node tools_trust_geometry_demo.mjs --pkg <wasm-pack output folder of an engine with the 0.36 geometry>
//
// What it does, exactly:
//   1. reads trust/demo/tallinn-helsinki.nmea (the synthetic ferry log the main demo uses) and edits it as text
//      (lib/trust-geometry-demo.mjs maskedWith, checksums recomputed);
//   2. scores each edited log with the engine's receiver_trust (the batch scorer, log inline) under
//      trust/demo/session.toml;
//   3. reduces each result to the fields the view reads (as tools_trust_demo.mjs does) plus the 0.36 geometry:
//      per epoch geometry, and the class on every deduction; geometry_model and geometry_summary.
// Files written:
//   trust/demo/masked-sky.result.json — the engine test's own edit (tests/geometry_trust.rs, masked_with(600, 0,
//     "2.9")): from 600 s every satellite at azimuth 180 degrees or more loses its signal (the western half of the
//     sky behind a hull or a quay) and GGA reports HDOP 2.9. The demo's made-up drag-off from 1500 s stays, so one
//     run shows a degraded sky and an attack-like inconsistency side by side.
//   REVIEW/fixtures/poor-geometry.result.json — a review fixture, not published (REVIEW/ is not ported): only
//     azimuths below 120 degrees kept from 600 s, so four satellites are left, the geometry is poor and costs
//     points, and GGA keeps stating HDOP 0.9, which the derived HDOP contradicts. Epochs to 1000 s.
// Synthetic text, not a measurement. Re-run it with the release engine at port time; lib/trust-geometry.test.mjs
// then checks the demo.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { maskedWith } from "./lib/trust-geometry-demo.mjs";

const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const here = new URL(".", import.meta.url).pathname;
const demo = join(here, "trust", "demo");

if (import.meta.url === `file://${process.argv[1]}`) {
  const pkg = arg("pkg");
  if (!pkg) { console.error("usage: node tools_trust_geometry_demo.mjs --pkg <wasm-pack output folder>"); process.exit(2); }
  const mod = await import(new URL(`file://${join(pkg, "kshana.js")}`).href);
  await mod.default({ module_or_path: readFileSync(join(pkg, "kshana_bg.wasm")) });
  const session = readFileSync(join(demo, "session.toml"), "utf8");
  const nmea = readFileSync(join(demo, "tallinn-helsinki.nmea"), "utf8");
  // Six decimals (about 0.1 m of latitude, far below the drawing's resolution) keep the files small.
  const round = (_, v) => (typeof v === "number" && !Number.isInteger(v) ? Number(v.toFixed(6)) : v);
  const record = (log, what, keep = () => true) => {
    const toml = session.replace(/^(\s*)path\s*=\s*"[^"\n]*"/m, (_, sp) => `${sp}text = '''\n${log}'''`);
    const r = JSON.parse(mod.receiver_trust(toml));
    const j = JSON.parse(r.json);
    if (!j.geometry_model) { console.error("this engine writes no geometry_model: build it from a branch with the 0.36 geometry"); process.exit(1); }
    console.log(`${what}: ${r.summary}\n  geometry_summary ${JSON.stringify(j.geometry_summary)}`);
    return JSON.stringify({
      recorded_with: `kshana ${mod.version()} receiver_trust (WebAssembly, the 0.36 geometry), reduced to the fields the Maritime trust view reads; log: ${what}`,
      advisory: j.advisory, label: j.label, score_model: j.score_model, monitors_run: j.monitors_run, states: j.states, events: j.events,
      geometry_model: j.geometry_model, geometry_summary: j.geometry_summary,
      epochs: j.epochs.filter(keep).map((e) => ({ t_s: e.t_s, state: e.state, alarms: e.alarms, marine: e.marine && { position: e.marine.position, ratios: e.marine.ratios }, score: e.score, geometry: e.geometry })),
    }, round);
  };
  writeFileSync(join(demo, "masked-sky.result.json"), record(maskedWith(nmea, 600, 0, "2.9"), "the main demo with the western half of the sky masked from 600 s"));
  mkdirSync(join(here, "REVIEW", "fixtures"), { recursive: true });
  writeFileSync(join(here, "REVIEW", "fixtures", "poor-geometry.result.json"),
    record(maskedWith(nmea, 600, 0, "0.9", 120), "the main demo with only azimuths below 120 degrees kept from 600 s (review fixture)", (e) => e.t_s <= 1000));
}
