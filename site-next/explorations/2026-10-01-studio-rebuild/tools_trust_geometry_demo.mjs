// SPDX-License-Identifier: AGPL-3.0-only
// Writes trust/demo/masked-sky.result.json: the Maritime trust view's geometry demo (0.36).
//
//   node tools_trust_geometry_demo.mjs --pkg <wasm-pack output folder of an engine with the 0.36 geometry>
//
// What it does, exactly:
//   1. reads trust/demo/tallinn-helsinki.nmea (the synthetic ferry log the main demo uses) and edits it as text
//      the way the engine's own test does (tests/geometry_trust.rs, masked_with(600, 0, "2.9")): from 600 s, the
//      GSV signal strength of every satellite at azimuth 180 degrees or more is blanked (the western half of the
//      sky lost behind, say, a hull or a quay) and GGA reports HDOP 2.9; checksums are recomputed;
//   2. scores it with the engine's receiver_trust (the batch scorer, log inline) under trust/demo/session.toml;
//   3. reduces the result to the fields the view reads (as tools_trust_demo.mjs does) plus the 0.36 geometry:
//      per epoch geometry, degradation and explained_by_geometry; geometry_model and geometry_summary.
// The log keeps the demo's made-up drag-off from 1500 s, so one run shows a masked sky (geometry, from 600 s)
// and an attack-like inconsistency (from 1500 s) side by side. Synthetic text, not a measurement. Re-run it
// with the release engine at port time; lib/trust-geometry.test.mjs then checks the new file.
import { readFileSync, writeFileSync } from "node:fs";
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
  const log = maskedWith(readFileSync(join(demo, "tallinn-helsinki.nmea"), "utf8"), 600, 0, "2.9");
  const toml = session.replace(/^(\s*)path\s*=\s*"[^"\n]*"/m, (_, sp) => `${sp}text = '''\n${log}'''`);
  const r = JSON.parse(mod.receiver_trust(toml));
  const j = JSON.parse(r.json);
  if (!j.geometry_model) { console.error("this engine writes no geometry_model: build it from a branch with the 0.36 geometry"); process.exit(1); }
  const out = {
    recorded_with: `kshana ${mod.version()} receiver_trust (WebAssembly, the 0.36 geometry), reduced to the fields the Maritime trust view reads; log: the main demo with the western half of the sky masked from 600 s`,
    advisory: j.advisory, label: j.label, score_model: j.score_model, monitors_run: j.monitors_run, states: j.states, events: j.events,
    geometry_model: j.geometry_model, geometry_summary: j.geometry_summary,
    epochs: j.epochs.map((e) => ({ t_s: e.t_s, state: e.state, alarms: e.alarms, marine: e.marine && { position: e.marine.position, ratios: e.marine.ratios }, score: e.score, geometry: e.geometry, degradation: e.degradation, explained_by_geometry: e.explained_by_geometry })),
  };
  // Six decimals (about 0.1 m of latitude, far below the drawing's resolution) keep the file small.
  const round = (_, v) => (typeof v === "number" && !Number.isInteger(v) ? Number(v.toFixed(6)) : v);
  writeFileSync(join(demo, "masked-sky.result.json"), JSON.stringify(out, round));
  console.log(`${mod.version()}: ${r.summary}`);
  console.log("geometry_summary", JSON.stringify(j.geometry_summary));
}
