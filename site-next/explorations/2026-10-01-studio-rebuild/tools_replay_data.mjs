// SPDX-License-Identifier: AGPL-3.0-only
// Writes the Bridge replay's data (replay/data/) from the engine's own WebAssembly package, so the replay
// shows what the engine and the logs say and invents nothing.
//
//   node tools_replay_data.mjs [--pkg <wasm-pack output folder>]      (default: ./pkg)
//
// What it does, exactly:
//   1. for each Training scenario in training/scenarios/, runs the engine's nmea_training (the scenario's own
//      seed), checks that the instructor log it writes is byte-identical to training/demo/<name>.instructor.json,
//      and keeps the NMEA stream only in memory;
//   2. scores that stream with the engine's receiver_trust under trust/demo/session.toml (the same vessel
//      settings and thresholds as the Maritime trust demo; nothing is tuned to the scenario) and writes the result,
//      reduced, to replay/data/<name>.trust.json;
//   3. reads each log's sky and sensors (lib/gsv.mjs parseAux: GSV satellites, HDT heading, RMC course and speed),
//      one epoch in five, and writes replay/data/<name>.aux.json; the two Trust demos get theirs from
//      trust/demo/tallinn-helsinki.nmea and from the masked-sky edit of it (lib/trust-geometry-demo.mjs).
// Synthetic text, not a measurement. Re-run it with the release engine at port time; lib/replay.test.mjs checks
// the files against the logs.
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { parseAux } from "./lib/gsv.mjs";
import { maskedPhases, DEMO_PHASES } from "./lib/trust-geometry-demo.mjs";

const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const here = new URL(".", import.meta.url).pathname;
const pkg = arg("pkg") || join(here, "pkg");
const out = join(here, "replay", "data");
mkdirSync(out, { recursive: true });
const mod = await import(new URL(`file://${join(pkg, "kshana.js")}`).href);
await mod.default({ module_or_path: readFileSync(join(pkg, "kshana_bg.wasm")) });
const version = mod.version();
const round = (_, v) => (typeof v === "number" && !Number.isInteger(v) ? Number(v.toFixed(6)) : v);
const STEP_S = 5;

const aux = (id, source, nmea) => {
  const a = parseAux(nmea, { stepS: STEP_S });
  const o = { id, recorded_with: `kshana ${version}`, source, step_s: STEP_S, n: a.n, t: a.t, hdt: a.hdt, cog: a.cog, sog: a.sog, sky: a.sky };
  writeFileSync(join(out, `${id}.aux.json`), JSON.stringify(o, round));
  console.log(`${id}: ${a.n} sky epochs`);
};

const session = readFileSync(join(here, "trust", "demo", "session.toml"), "utf8");
for (const file of readdirSync(join(here, "training", "scenarios")).filter((f) => f.endsWith(".toml")).sort()) {
  const id = file.replace(/\.toml$/, "");
  const r = JSON.parse(mod.nmea_training(readFileSync(join(here, "training", "scenarios", file), "utf8"), NaN));
  if (r.log_json !== readFileSync(join(here, "training", "demo", `${id}.instructor.json`), "utf8")) { console.error(`${id}: the engine's instructor log differs from training/demo/${id}.instructor.json; re-record the demo first`); process.exit(1); }
  const toml = session.replace(/^(\s*)path\s*=\s*"[^"\n]*"/m, (_, sp) => `${sp}text = '''\n${r.nmea}'''`).replace(/^name\s*=.*$/m, `name = "Synthetic training scenario ${id}"`);
  const t = mod.receiver_trust(toml), j = JSON.parse(JSON.parse(t).json);
  console.log(`${id}: ${JSON.parse(t).summary}`);
  writeFileSync(join(out, `${id}.trust.json`), JSON.stringify({
    recorded_with: `kshana ${version} receiver_trust (WebAssembly) over the engine's own nmea_training stream for ${id}, scored under trust/demo/session.toml; reduced to what the replay reads (state, alarms, position, score with its deductions, the sky's geometry label)`,
    advisory: j.advisory, label: j.label, score_model: j.score_model, monitors_run: j.monitors_run, states: j.states, events: j.events,
    epochs: j.epochs.map((e) => ({ t_s: e.t_s, state: e.state, alarms: e.alarms, marine: e.marine && { position: e.marine.position, ratios: [] }, score: e.score, geometry: e.geometry && { status: e.geometry.status, reasons: e.geometry.reasons, source: e.geometry.source, n_used: e.geometry.n_used, hdop: e.geometry.hdop, pdop: e.geometry.pdop, vdop: e.geometry.vdop, open_fraction: null, max_azimuth_gap_deg: e.geometry.max_azimuth_gap_deg, age_s: e.geometry.age_s, reported_hdop: e.geometry.reported_hdop, reported_derived_mismatch: e.geometry.reported_derived_mismatch, in_view: e.geometry.in_view, signal_loss: e.geometry.signal_loss, label: e.geometry.label } })),
  }, round));
  aux(id, `nmea_training ${id}, seed from the scenario`, r.nmea);
}
const nmea = readFileSync(join(here, "trust", "demo", "tallinn-helsinki.nmea"), "utf8");
aux("tallinn-helsinki", "trust/demo/tallinn-helsinki.nmea", nmea);
aux("masked-sky", "trust/demo/tallinn-helsinki.nmea with the western half of the sky masked from 600 s (lib/trust-geometry-demo.mjs)", maskedPhases(nmea, DEMO_PHASES));
