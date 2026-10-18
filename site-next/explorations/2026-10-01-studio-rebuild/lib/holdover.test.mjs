// SPDX-License-Identifier: AGPL-3.0-only
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SCENARIOS, readHoldover, readRecordFromToml, seriesFor, teSvg, curvesSvg, stepRows, holdoverRows } from "./holdover.mjs";

const here = new URL("../", import.meta.url).pathname;
const load = (id) => JSON.parse(readFileSync(here + `recorded/${id}.json`, "utf8"));
const models = Object.fromEntries(SCENARIOS.map((s) => [s.id, readHoldover(load(s.id)).model]));

test("both holdover-test recordings open and carry the engine's numbers unchanged", () => {
  assert.equal(SCENARIOS.length, 2);
  for (const sc of SCENARIOS) {
    const doc = load(sc.id), raw = JSON.parse(doc.json), { model, error } = readHoldover(doc);
    assert.equal(error, undefined, sc.id);
    assert.equal(model.steps.length, 3); assert.deepEqual(model.steps.map((s) => s.name), ["baseline", "holdover", "recovery"]);
    assert.equal(model.holdover.maxNs, raw.holdover.te_at_max_abs_ns); assert.equal(model.test.removedS, raw.test.gnss_removed_s); assert.equal(model.envelope.tExceedS, raw.holdover_envelope.time_to_exceed_s);
    assert.equal(model.masks[0].checks[0].limitNs, raw.masks[0].checks[0].limit_ns); assert.equal(model.verdicts.holdover_masks, raw.verdicts.holdover_masks);
    assert.match(model.label, /^MODELLED/); assert.match(model.label, /measured against the mask/); assert.ok(model.notCovered.length >= 4);
    assert.match(model.test.statedBy, /not verified by Kshana/);
  }
});

test("the operator's record is re-read from the scenario text and used only when it matches the engine's record block", () => {
  for (const sc of SCENARIOS) {
    const m = models[sc.id], { series, from } = seriesFor(m);
    assert.ok(series, sc.id); assert.equal(from, "the scenario's own record");
    assert.equal(series.t.length, m.record.n); assert.equal(series.t[series.t.length - 1] - series.t[0], m.record.lengthS);
  }
  const m = models["holdover-test-example"], s = seriesFor(m).series;
  // the series the engine hashed is the one drawn: its own sha is over the scored record (checked here as the same record, not re-derived)
  assert.ok(s.te.some((v) => Math.abs(v) > 200), "the example's holdover reaches the drift the scenario states");
  const raw = JSON.parse(load("holdover-test-example").json);
  assert.equal(Math.max(...s.te.map(Math.abs)), raw.steps[1].max_abs_te_ns > raw.recovery.peak_abs_te_ns ? raw.steps[1].max_abs_te_ns : Math.max(raw.steps[1].max_abs_te_ns, raw.recovery.peak_abs_te_ns), "the largest sample drawn is the largest the engine scored");
});
test("a record that differs from the engine's block in count, length or interval is not drawn, and the reason is said", () => {
  const m = structuredClone(models["holdover-test-example"]);
  const bad = m.toml.replace(/\[0, [^\]]*\],/, "");
  assert.match(seriesFor(m, bad).why, /samples where the engine scored 601/);
  assert.equal(seriesFor(m, "kind = \"holdover-test\"\n").series, null);
  const shifted = structuredClone(m); shifted.record.lengthS += 1;
  assert.match(seriesFor(shifted).why, /a length of/);
});
test("a series the result itself carries is preferred, in either shape", () => {
  const m = structuredClone(models["holdover-test-example"]), s = readRecordFromToml(m.toml);
  m.record.series = { t: s.t, te: s.te };
  assert.equal(seriesFor(m).from, "the result");
  const raw = JSON.parse(load("holdover-test-example").json); raw.record.series = { time_s: s.t, time_error_ns: s.te };
  const { model } = readHoldover({ ...load("holdover-test-example"), json: JSON.stringify(raw) });
  assert.equal(seriesFor(model).from, "the result");
});
test("the time-error chart marks the phases, the operator's two times, the mask limits and the exceedances, from the result", () => {
  for (const sc of SCENARIOS) {
    const m = models[sc.id], svg = teSvg(m, seriesFor(m).series);
    for (const p of ["Baseline: locked", "Holdover: GNSS removed", "Recovery: GNSS restored"]) assert.ok(svg.includes(p), p);
    assert.match(svg, /GNSS removed \(stated by the operator\)/); assert.match(svg, /GNSS restored \(stated by the operator\)/);
    assert.match(svg, /prtc-a ±100 ns/); assert.match(svg, /class="ho-line"/); assert.match(svg, /Recovery band ±100 ns/);
    assert.match(svg, /ePRTC-A holdover envelope first exceeded/);
  }
  assert.match(teSvg(models["holdover-test-example"], null), /No time-error curve/);
});
test("MTIE and TDEV are drawn from the engine's curves with the mask limits as markers", () => {
  const m = models["holdover-test-example"];
  for (const w of ["mtie", "tdev"]) { const svg = curvesSvg(m, w); assert.match(svg, /ho-cv/); assert.match(svg, /ho-mk/); assert.ok(svg.includes("baseline") && svg.includes("holdover")); }
  assert.match(curvesSvg({ curves: [], masks: [] }, "mtie"), /No MTIE curve/);
});
test("the tables say what the result says, with units and without a verdict of the page's own", () => {
  const m = models["holdover-test-ptp4l"];
  assert.equal(stepRows(m).length, 3); assert.ok(holdoverRows(m).some(([k, v]) => /Recovery/.test(k) && /inside ±100 ns/.test(v)));
  assert.ok(holdoverRows(m).some(([k, v]) => /envelope/.test(k) && /FAIL/.test(v)));
  assert.match(readHoldover({ json: "{}" }).error, /not a holdover-test result/); assert.match(readHoldover({ json: "{" }).error, /not JSON/);
});
