// SPDX-License-Identifier: AGPL-3.0-only
// Tests for the tools hub helpers (tools35.mjs). Run with `node lib/tools35.test.mjs`.
import assert from "node:assert/strict";
import { toBase64, fromBase64, packFilesJson, packProblems, parsePublicKey, verdictView, checkEpoch, benchEntries, scenarioKind, complianceRuns, BENCH_KINDS } from "./tools35.mjs";

const te = (s) => new TextEncoder().encode(s);
// base64 round trips and matches Node's
for (const b of [[], [0], [255, 0], [1, 2, 3], [250, 251, 252, 253], Array.from({ length: 300 }, (_, i) => i % 256)]) {
  const u = Uint8Array.from(b);
  assert.equal(toBase64(u), Buffer.from(u).toString("base64"));
  assert.deepEqual([...fromBase64(toBase64(u))], b);
}
assert.throws(() => fromBase64("a$bc"), /not base64/); assert.throws(() => fromBase64("abc"), /not base64/);

// pack files: text as strings, binary as base64, the chosen folder's own name dropped
{
  const j = packFilesJson([{ path: "pack/manifest.json", bytes: te('{"a":1}') }, { path: "pack/log-slice.bin", bytes: Uint8Array.from([0xFF, 0xFE, 0, 1]) }, { path: "pack/summary.html", bytes: te("<p>é</p>") }]);
  assert.deepEqual(Object.keys(j).sort(), ["log-slice.bin", "manifest.json", "summary.html"]);
  assert.equal(j["manifest.json"], '{"a":1}');
  assert.deepEqual(j["log-slice.bin"], { base64: "//4AAQ==" });
  assert.equal(j["summary.html"], "<p>é</p>");
  // loose files keep their names; a BOM or other text that would not survive is sent as bytes
  const k = packFilesJson([{ name: "manifest.json", bytes: te("{}") }, { name: "bom.txt", bytes: Uint8Array.from([0xEF, 0xBB, 0xBF, 0x41]) }]);
  assert.equal(k["manifest.json"], "{}"); assert.deepEqual(k["bom.txt"], { base64: "77u/QQ==" });
  assert.throws(() => packFilesJson([{ name: "../x", bytes: te("") }]), /refused file name/);
  assert.throws(() => packFilesJson([{ name: "a", bytes: te("") }, { name: "a", bytes: te("") }]), /two files are named a/);
  assert.deepEqual(packProblems({ "manifest.json": "x" }), ["manifest.sig is not among the chosen files"]);
  assert.deepEqual(packProblems({ "manifest.json": "x", "manifest.sig": "y" }), []);
}
// public key
assert.deepEqual(parsePublicKey(""), { key: "" });
assert.equal(parsePublicKey("  " + "AB".repeat(32) + "\n").key, "ab".repeat(32));
assert.match(parsePublicKey("abc").error, /64 hex digits/); assert.match(parsePublicKey("zz".repeat(32)).error, /64 hex digits/);
// verdicts
assert.equal(verdictView({ verdict: "verified", message: "m" }).tone, "ok");
assert.equal(verdictView({ verdict: "intact-signer-not-pinned" }).tone, "warn");
assert.equal(verdictView({ verdict: "failed", message: "bad" }).text, "bad");
assert.equal(verdictView(null).tone, "bad");
// epoch
assert.deepEqual(checkEpoch(""), { epoch: "" });
assert.equal(checkEpoch("2025-03-01T10:00:00Z").epoch, "2025-03-01T10:00:00Z");
assert.equal(checkEpoch("2024-02-29T00:00:00").epoch, "2024-02-29T00:00:00");
for (const bad of ["2025-02-29T00:00:00", "1999-01-01T00:00:00", "2100-01-01T00:00:00", "2025-13-01T00:00:00", "2025-01-01T24:00:00", "tomorrow", "2025-01-01 10:00:00"]) assert.ok(checkEpoch(bad).error, bad);
// bench
{
  const e = benchEntries({ files: { "motion.csv": "a", nmea: "b", "events.toml": "c" } }, "run1");
  assert.deepEqual(e.map((x) => x.name), ["run1.events.toml", "run1.motion.csv", "run1.nmea"]);
  assert.deepEqual(benchEntries(null, "x"), []);
  assert.deepEqual([...BENCH_KINDS], ["gnss-ins", "jamming", "gnss-sim"]);
  assert.equal(scenarioKind('# c\nkind = "gnss-ins"\n[x]'), "gnss-ins"); assert.equal(scenarioKind("name = 'x'"), null);
}
// compliance runs pair results with scenarios by name
{
  const r = complianceRuns([{ name: "a.result.json", text: "{}" }, { name: "a.toml", text: "kind='x'" }, { name: "b.result.json", text: "{}" }, { name: "c.toml", text: "" }]);
  assert.deepEqual(r.runs, [{ label: "a", result: "{}", scenario: "kind='x'" }, { label: "b", result: "{}" }]);
  assert.deepEqual(r.unusedScenarios, ["c"]);
}
console.log("tools35.test.mjs: ok");
