// SPDX-License-Identifier: AGPL-3.0-only
// Rebuilds trust/demo/ (the Maritime trust view's synthetic demo) from a kshana CLI.
//
//   node tools_trust_demo.mjs --kshana <path-to-kshana-binary> --repo <checkout holding examples/maritime-trust>
//
// What it does, exactly:
//   1. copies <repo>/examples/maritime-trust/{session.toml,tallinn-helsinki.nmea} to a scratch folder;
//   2. runs `kshana receiver-trust session.toml` there (the CLI writes session.result.json next to it);
//   3. reduces the result to the fields the view reads (score_model, monitors_run, states, events and,
//      per epoch, t_s, state, alarms, marine.position, marine.ratios, score), 5 MB down to about 1.4 MB;
//   4. copies <repo>/examples/maritime-trust/tallinn-helsinki.truth.csv unchanged;
//   5. also copies the session settings and the NMEA log, unchanged, for the "Score a log here" panel;
//   6. writes both to trust/demo/ and prints the CLI's version and summary line.
// The demo is synthetic text made by the engine's own generator: not a measurement. Re-run it with the
// release engine at port time; lib/trust.test.mjs then checks the new files.
import { execFileSync } from "node:child_process";
import { mkdtempSync, copyFileSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const bin = arg("kshana"), repo = arg("repo");
if (!bin || !repo) { console.error("usage: node tools_trust_demo.mjs --kshana <binary> --repo <checkout>"); process.exit(2); }
const here = new URL(".", import.meta.url).pathname;
const src = join(repo, "examples", "maritime-trust");
const work = mkdtempSync(join(tmpdir(), "trust-demo-"));
for (const f of ["session.toml", "tallinn-helsinki.nmea"]) copyFileSync(join(src, f), join(work, f));
const version = execFileSync(bin, ["--version"], { encoding: "utf8" }).trim();
const summary = execFileSync(bin, ["receiver-trust", "session.toml"], { cwd: work, encoding: "utf8" }).trim().split("\n").pop();
const j = JSON.parse(readFileSync(join(work, "session.result.json"), "utf8"));
const out = {
  recorded_with: "kshana receiver-trust (CLI), reduced to the fields the Maritime trust view reads",
  advisory: j.advisory, label: j.label, score_model: j.score_model, monitors_run: j.monitors_run, states: j.states, events: j.events,
  epochs: j.epochs.map((e) => ({ t_s: e.t_s, state: e.state, alarms: e.alarms, marine: e.marine && { position: e.marine.position, ratios: e.marine.ratios }, score: e.score })),
};
mkdirSync(join(here, "trust", "demo"), { recursive: true });
writeFileSync(join(here, "trust", "demo", "tallinn-helsinki.result.json"), JSON.stringify(out));
copyFileSync(join(src, "tallinn-helsinki.truth.csv"), join(here, "trust", "demo", "tallinn-helsinki.truth.csv"));
// the raw NMEA log and the session settings, for the "Score a log here" panel (engine receiver_trust_replay)
for (const f of ["session.toml", "tallinn-helsinki.nmea"]) copyFileSync(join(src, f), join(here, "trust", "demo", f));
console.log(`${version}\n${summary}\nwrote trust/demo/ (${out.epochs.length} epochs)`);
