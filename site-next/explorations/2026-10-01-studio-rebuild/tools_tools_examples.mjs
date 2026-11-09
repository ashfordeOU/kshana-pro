// SPDX-License-Identifier: AGPL-3.0-only
// Rebuilds the Tools page's example packs and recorded CLI samples from a kshana CLI (0.35 or later).
//
//   node tools_tools_examples.mjs --kshana <path-to-kshana-binary> --repo <checkout holding examples/maritime-trust>
//
// Writes, all synthetic and all made by the engine's own generators:
//   tools/examples/evidence/   a signed evidence pack of the first 60 s of the synthetic maritime demo log,
//                              signed with a key made for this run (the private key is deleted at the end
//                              and never written into the Studio; index.json carries the PUBLIC key, which is
//                              what a verifier pins), plus index.json {files, tamper, public_key}
//   tools/examples/compliance/ two small synthetic scenario results and their scenario files + index.json
//   tools/samples/            recorded real CLI output shown on the CLI cards (telemetry, IQ command list,
//                              Kshana Connect's conformance check and adapter registry)
// OSNMA samples are not made here: they come from the engine repo's examples/osnma/ (see tools_osnma_examples.mjs).
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, copyFileSync, readFileSync, writeFileSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const bin = arg("kshana"), repo = arg("repo");
if (!bin || !repo) { console.error("usage: node tools_tools_examples.mjs --kshana <binary> --repo <checkout>"); process.exit(2); }
const here = new URL(".", import.meta.url).pathname;
const run = (args, cwd) => execFileSync(bin, args, { cwd, encoding: "utf8", maxBuffer: 1 << 28 });
const work = mkdtempSync(join(tmpdir(), "tools-examples-"));
const version = run(["--version"]).trim().replace(/^kshana /, "");
const src = join(repo, "examples", "maritime-trust");
for (const f of ["session.toml", "tallinn-helsinki.nmea"]) copyFileSync(join(src, f), join(work, f));

// 1. evidence pack
run(["evidence", "keygen", "--out", "demo.key"], work);
const pubHex = readFileSync(join(work, "demo.key.pub"), "utf8").trim().split(/\s+/).find((w) => /^[0-9a-f]{64}$/.test(w));
if (!pubHex) throw new Error("could not read the public key from demo.key.pub");
run(["receiver-trust", "evidence", "session.toml", "--from", "0", "--to", "60", "--key", "demo.key", "--out", "pack", "--title", "Synthetic demonstration pack", "--created-utc", "2026-01-01T00:00:00Z"], work);
const verdict = JSON.parse(run(["evidence", "verify", "pack", "--pubkey", "demo.key.pub", "--json"], work));
const ev = join(here, "tools", "examples", "evidence");
rmSync(ev, { recursive: true, force: true }); mkdirSync(ev, { recursive: true });
const packFiles = readdirSync(join(work, "pack")).sort();
for (const f of packFiles) copyFileSync(join(work, "pack", f), join(ev, f));
writeFileSync(join(ev, "index.json"), JSON.stringify({
  note: "A synthetic evidence pack: the first 60 s of the synthetic maritime demo log, signed with a key made only for this example.",
  recorded_with: `kshana ${version}`, files: packFiles, tamper: "epochs.json", public_key: pubHex }, null, 2) + "\n");

// 2. compliance example
const cp = join(here, "tools", "examples", "compliance");
rmSync(cp, { recursive: true, force: true }); mkdirSync(cp, { recursive: true });
const names = ["integrity-raim", "jamming-demo"];
for (const n of names) { copyFileSync(join(here, "scenarios", n + ".toml"), join(work, n + ".toml")); run([n + ".toml"], work); }
const cfiles = [];
for (const n of names) for (const f of [n + ".toml", n + ".result.json"]) { copyFileSync(join(work, f), join(cp, f)); cfiles.push(f); }
writeFileSync(join(cp, "index.json"), JSON.stringify({
  note: "Two synthetic scenario runs (integrity, jamming) and their scenario files. The mapping reads what each run evidenced; it never says a framework is met.",
  recorded_with: `kshana ${version}`, files: cfiles }, null, 2) + "\n");

// 3. recorded CLI samples
const sm = join(here, "tools", "samples"); mkdirSync(sm, { recursive: true });
run(["receiver-trust", "session.toml"], work);
writeFileSync(join(sm, "telemetry-metrics.txt"), run(["trust-telemetry", "--result", "session.result.json", "--print-metrics"], work));
writeFileSync(join(sm, "telemetry-cef.txt"), run(["trust-telemetry", "--result", "session.result.json", "--print-syslog", "--format", "cef"], work));
writeFileSync(join(sm, "telemetry-leef.txt"), run(["trust-telemetry", "--result", "session.result.json", "--print-syslog", "--format", "leef"], work));
const iq = spawnSync(bin, ["iq", "--help"], { encoding: "utf8" });
writeFileSync(join(sm, "iq-help.txt"), (iq.stdout || "") + (iq.stderr || ""));
// Kshana Connect: the conformance set and the adapter registry, as the repository holds them.
writeFileSync(join(sm, "connect-check.txt"), run(["connect", "check", "conformance/connect"], repo));
writeFileSync(join(sm, "connect-adapters.txt"), run(["connect", "adapters", "list", "adapters"], repo));
rmSync(work, { recursive: true, force: true });
console.log(`kshana ${version}: evidence ok=${verdict.ok}; wrote tools/examples/{evidence,compliance} and tools/samples`);
