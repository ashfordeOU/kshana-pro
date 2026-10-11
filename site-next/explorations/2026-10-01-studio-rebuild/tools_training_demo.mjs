// SPDX-License-Identifier: AGPL-3.0-only
// Rebuilds training/demo/ (the training debrief page's SYNTHETIC demo) from a kshana CLI.
//
//   node tools_training_demo.mjs --kshana <path-to-kshana-binary> --repo <checkout holding scenarios/training>
//
// For each scenario in <repo>/scenarios/training/*.toml it runs
//   kshana nmea-scenario <scenario>.toml --out <scratch>/<name>.nmea --instructor-log <scratch>/<name>
// and copies <name>.instructor.json, unchanged, to training/demo/ and the scenario file to training/scenarios/. The NMEA streams themselves are not kept.
// Everything is synthetic: invented positions, dates and tracks, no real vessel, operator or recording.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, copyFileSync, mkdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const bin = arg("kshana"), repo = arg("repo");
if (!bin || !repo) { console.error("usage: node tools_training_demo.mjs --kshana <binary> --repo <checkout>"); process.exit(2); }
const here = new URL(".", import.meta.url).pathname;
const dir = join(repo, "scenarios", "training");
const work = mkdtempSync(join(tmpdir(), "training-demo-"));
mkdirSync(join(here, "training", "demo"), { recursive: true });
console.log(execFileSync(bin, ["--version"], { encoding: "utf8" }).trim());
for (const f of readdirSync(dir).filter((x) => x.endsWith(".toml")).sort()) {
  const name = f.replace(/\.toml$/, "");
  execFileSync(bin, ["nmea-scenario", join(dir, f), "--out", join(work, name + ".nmea"), "--instructor-log", join(work, name)], { cwd: repo, stdio: ["ignore", "ignore", "inherit"] });
  mkdirSync(join(here, "training", "scenarios"), { recursive: true });
  copyFileSync(join(dir, f), join(here, "training", "scenarios", f)); // the scenario files, for the "generate here" panel
  copyFileSync(join(work, name + ".instructor.json"), join(here, "training", "demo", name + ".instructor.json"));
  console.log("wrote training/demo/" + name + ".instructor.json", statSync(join(here, "training", "demo", name + ".instructor.json")).size, "bytes");
}
