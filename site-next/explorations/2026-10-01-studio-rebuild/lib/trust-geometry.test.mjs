// SPDX-License-Identifier: AGPL-3.0-only
// Tests for the Maritime trust view's geometry lane (trust-geometry.mjs, and its use in trust.mjs).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { readEpochGeometry, readDegradation, readExplained, readGeometryModel, readGeometrySummary, countGeometry, statusKey, causeKey, geometryAt, describeGeometry, GEO_STATUSES, DEGRADATIONS, UNKNOWN_TEXT, THRESHOLDS } from "./trust-geometry.mjs";
import { buildRun, reasonsAt, describeEpoch, parseLines } from "./trust.mjs";
import { maskedWith, checksum } from "./trust-geometry-demo.mjs";

const g = (o = {}) => ({ status: "good", reasons: [], basis: "sky", n_sats: 13, pdop: 1.43, hdop: 0.8, vdop: 1.19, max_azimuth_gap_deg: 40, age_s: 0, reported_hdop: 0.9, hdop_report_mismatch: false, ...o });
const model = { nominal_min: 90, degraded_min: 55, onset_ratio: 0.5, full_ratio: 1.5, evidence_hold_s: 10, weights: { "loss-of-lock": 25, kinematic: 60 } };
const ep = (t, state, score, ded, geo, degradation, explained) => ({ t_s: t, state, alarms: [], marine: { position: [59 + t / 1000, 24], ratios: ded.map((d) => [d[0], d[1]]) }, score: score == null ? undefined : { score, band: state, deductions: ded.map((d) => ({ monitor: d[0], ratio: d[1], points: d[2] })) }, geometry: geo, degradation, explained_by_geometry: explained });

test("an epoch's geometry is read field by field", () => {
  const r = readEpochGeometry(g({ status: "degraded", reasons: ["sky-blocked", 5, "high-hdop"], max_azimuth_gap_deg: 230, hdop_report_mismatch: true }));
  assert.equal(r.status, "degraded");
  assert.deepEqual(r.reasons, ["sky-blocked", "high-hdop"]);
  assert.equal(r.gapDeg, 230);
  assert.equal(r.mismatch, true);
  assert.equal(readEpochGeometry(null), null, "an engine before 0.36 writes none");
  assert.equal(readEpochGeometry([1]), null);
  const odd = readEpochGeometry({ status: "excellent", pdop: "1" });
  assert.equal(odd.status, "unknown", "a status this page does not know is never read as good");
  assert.equal(odd.oddStatus, "excellent");
  assert.equal(odd.pdop, null);
  assert.equal(readEpochGeometry({ status: "good", n_sats: -3 }).nSats, null);
  assert.equal(readDegradation("geometry"), "geometry");
  assert.equal(readDegradation("__proto__"), null);
  assert.deepEqual(readExplained(["loss-of-lock", 3, null]), ["loss-of-lock"]);
  assert.deepEqual(readExplained("loss-of-lock"), []);
});

test("unknown geometry says so and is never shown as good", () => {
  const e = { geometry: readEpochGeometry(g({ status: "unknown", pdop: null, hdop: null, vdop: null, basis: "none" })), degradation: "none" };
  const a = geometryAt(e);
  assert.equal(a.tone, "unknown");
  assert.match(a.heading, /^Geometry unknown: no GSV or GSA in the log/);
  assert.equal(describeGeometry(e), UNKNOWN_TEXT);
  assert.equal(statusKey(e), "unknown");
  assert.equal(geometryAt({ geometry: null }).tone, "missing");
  assert.equal(statusKey({ geometry: null }), "missing");
  assert.equal(causeKey({ degradation: null }), "missing");
});

test("the panel words: reasons in plain words, the mismatch, what the degradation is put down to", () => {
  const e = { geometry: readEpochGeometry(g({ status: "degraded", reasons: ["sky-blocked"], reported_hdop: 0.9, hdop: 2.9, hdop_report_mismatch: true })), degradation: "geometry-and-inconsistency" };
  const a = geometryAt(e);
  assert.match(a.heading, /one side of the sky blocked/);
  const rows = Object.fromEntries(a.rows);
  assert.match(rows["HDOP the receiver reports"], /disagrees with the sky/);
  assert.match(rows["Degradation put down to"], /poor sky and an attack-like check/);
  assert.equal(describeGeometry(e), "geometry degraded (HDOP 2.9), geometry and inconsistency");
});

test("the model: thresholds in a fixed order with their provenance; the summary", () => {
  const m = readGeometryModel({ pdop_max: 6, hdop_max: 4, min_sats: 5, azimuth_gap_deg: 180, hdop_report_tol: 1, hold_s: 10, provenance: { pdop_max: "GPS SPS PS", hdop_max: "IEC 61108-1", odd: 5 }, explains: "loss-of-lock ..." });
  assert.deepEqual(m.thresholds.map((t) => t.key), THRESHOLDS.map(([k]) => k));
  assert.deepEqual(m.provenance.map((p) => p.key), ["hdop_max", "pdop_max"]);
  assert.equal(m.explains, "loss-of-lock ...");
  assert.equal(readGeometryModel("x"), null);
  const s = readGeometrySummary({ good: 600, degraded: 2401, unknown: 0, attributed_geometry: 950, attributed_inconsistency: 0, attributed_both: 1451, first_degraded_t_s: 600 });
  assert.deepEqual(s, { good: 600, degraded: 2401, unknown: 0, geometry: 950, inconsistency: 0, both: 1451, firstDegradedS: 600 });
});

test("a batch run carries the lane; explained monitors are marked and still cost their points", () => {
  const result = {
    score_model: model, monitors_run: ["loss-of-lock", "kinematic"], states: {}, events: [],
    geometry_model: { pdop_max: 6, hdop_max: 4, min_sats: 5, azimuth_gap_deg: 180, hdop_report_tol: 1, hold_s: 10, provenance: {}, explains: "x" },
    geometry_summary: { good: 1, degraded: 2, unknown: 0, attributed_geometry: 1, attributed_inconsistency: 0, attributed_both: 1 },
    epochs: [
      ep(0, "nominal", 100, [], g(), "none", []),
      ep(1, "degraded", 75, [["loss-of-lock", 1.75, 25]], g({ status: "degraded", reasons: ["sky-blocked"] }), "geometry", ["loss-of-lock"]),
      ep(2, "untrusted", 15, [["kinematic", 2, 60], ["loss-of-lock", 1.75, 25]], g({ status: "degraded", reasons: ["sky-blocked"] }), "geometry-and-inconsistency", ["loss-of-lock"]),
    ],
  };
  const { run } = buildRun({ result });
  assert.equal(run.hasGeometry, true);
  assert.equal(run.geometryModel.thresholds.length, 6);
  assert.equal(run.geometrySummary.both, 1);
  const r = reasonsAt(run, 2);
  assert.deepEqual(r.ded.map((d) => [d.monitor, d.points, d.explained]), [["kinematic", 60, false], ["loss-of-lock", 25, true]]);
  assert.equal(run.epochs[1].score, 75, "the geometry never changes the score");
  assert.match(describeEpoch(run, 1), /geometry degraded/);
  const c = countGeometry(run.epochs);
  assert.equal(c.degraded, 2); assert.equal(c.geometry, 1); assert.equal(c["geometry-and-inconsistency"], 1);
});

test("a run from an older engine has no lane, and nothing is drawn as good", () => {
  const { run } = buildRun({ result: { score_model: model, monitors_run: [], epochs: [ep(0, "nominal", 100, [])] } });
  assert.equal(run.hasGeometry, false);
  assert.equal(run.geometryModel, null);
  assert.equal(run.epochs[0].geometry, null);
  assert.deepEqual(reasonsAt(run, 0).ded, []);
});

test("live lines, schema 1.3: the same fields at the end of each line", () => {
  const text = [
    JSON.stringify({ schema: "1.3", seq: 1, t_s: 0, state: "nominal", score: 100, deductions: [], gate: "passed", position: { lat_deg: 59, lon_deg: 24 }, geometry: g(), degradation: "none", explained_by_geometry: [] }),
    JSON.stringify({ schema: "1.3", seq: 2, t_s: 1, state: "degraded", score: 75, deductions: [{ monitor: "loss-of-lock", ratio: 1.75, points: 25 }], gate: "passed", position: { lat_deg: 59, lon_deg: 24 }, geometry: g({ status: "unknown" }), degradation: "inconsistency", explained_by_geometry: [] }),
  ].join("\n");
  const { run } = buildRun({ lines: parseLines(text).epochs });
  assert.equal(run.hasGeometry, true);
  assert.deepEqual(run.epochs.map(statusKey), ["good", "unknown"]);
  assert.deepEqual(run.epochs.map(causeKey), ["none", "inconsistency"]);
  assert.equal(reasonsAt(run, 1).ded[0].explained, false);
});

test("the demo generator edits the log as the engine's test does", () => {
  const body = (s) => `${s}*${checksum(s.slice(1))}`;
  const log = [
    body("$GPGGA,080000.00,5927.2998,N,02446.2017,E,1,13,0.9,37.7,M,17.8,M,,"),
    body("$GPGSV,1,1,02,01,40,090,45,02,30,200,44"),
    body("$GPGGA,081000.00,5927.2998,N,02446.2017,E,1,13,0.9,37.7,M,17.8,M,,"),
    body("$GPGSV,1,1,02,01,40,090,45,02,30,200,44"),
  ].join("\n");
  const out = maskedWith(log, 600, 0, "2.9").trim().split("\n");
  assert.equal(out[0], log.split("\n")[0], "before 600 s the log is unchanged");
  assert.match(out[2], /,1,13,2\.9,/, "GGA reports HDOP 2.9 from 600 s");
  assert.match(out[3], /^\$GPGSV,1,1,02,01,40,090,45,02,30,200,\*/, "the satellite at azimuth 200 loses its signal; the one at 90 keeps it");
  for (const l of out) assert.equal(l.split("*")[1], checksum(l.slice(1).split("*")[0]), "checksums recomputed");
});

// The recorded demo (tools_trust_geometry_demo.mjs) through the view's own reader.
const DEMO = new URL("../trust/demo/masked-sky.result.json", import.meta.url);
test("the masked-sky demo: a masked sky is geometry, the drag-off is not, and the score is the engine's", { skip: !existsSync(DEMO) && "trust/demo/masked-sky.result.json is not there" }, () => {
  const result = JSON.parse(readFileSync(DEMO, "utf8"));
  const { run, error } = buildRun({ result, demo: true });
  assert.equal(error, undefined);
  assert.equal(run.hasGeometry, true);
  assert.ok(run.epochs.every((e) => e.geometry && GEO_STATUSES.includes(e.geometry.status) && !e.geometry.oddStatus));
  assert.ok(run.epochs.every((e) => DEGRADATIONS.includes(e.degradation)));
  const at = (t) => run.epochs.find((e) => e.t_s === t);
  assert.equal(at(300).geometry.status, "good", "before the mask the sky is good");
  assert.equal(at(700).geometry.status, "degraded");
  assert.ok(at(700).geometry.reasons.includes("sky-blocked"));
  assert.equal(at(700).degradation, "geometry", "the masked sky alone is geometry");
  assert.ok(at(700).explained.includes("loss-of-lock"));
  assert.ok(at(700).deductions.some((d) => d.monitor === "loss-of-lock" && d.points > 0), "an explained monitor still costs its points");
  assert.ok(run.epochs.some((e) => e.t_s > 1500 && /inconsistency/.test(e.degradation)), "the drag-off stays attack-like under the masked sky");
  const c = countGeometry(run.epochs), s = run.geometrySummary;
  assert.equal(c.good, s.good); assert.equal(c.degraded, s.degraded); assert.equal(c.unknown, s.unknown);
  assert.equal(c.geometry, s.geometry); assert.equal(c["geometry-and-inconsistency"], s.both); assert.equal(c.inconsistency, s.inconsistency);
  assert.match(result.advisory, /Advisory software, not type-approved navigation equipment \(IEC 61108, IEC 61162\)/);
  assert.equal(run.geometryModel.thresholds.find((t) => t.key === "hdop_max").value, 4);
  assert.ok(run.geometryModel.provenance.length >= 5 && run.geometryModel.explains.length > 50);
});
