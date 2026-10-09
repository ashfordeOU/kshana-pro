// SPDX-License-Identifier: AGPL-3.0-only
// Tests for the maritime trust view's data model (trust.mjs). Run with `node lib/trust.test.mjs`.
import assert from "node:assert/strict";
import fs from "node:fs";
import {
  sniff, parseLines, parseTruth, parseTrustCsv, parseNmeaFixes, nmeaChecksumOk, enOffsetM, distanceM, niceLength,
  makeProjection, deriveGate, nearestIndex, joinFixes, joinTruth, buildRun, reasonsAt, epochsAboveOnset, stepIndex,
  describeEpoch, pkshtFor, bandRuns, monitorInfo, KNOWN_MONITORS, MONITOR_INFO,
} from "./trust.mjs";

// A result in the shape `kshana receiver-trust` writes for a vessel: ten epochs, calibrating, then nominal,
// degraded, untrusted and a recovery.
const model = { nominal_min: 90, degraded_min: 55, onset_ratio: 0.5, full_ratio: 1.5, evidence_hold_s: 10, weights: { kinematic: 60, "heading-course": 40, "cn0-spread": 30 } };
const mk = (t, state, score, ded, pos) => ({
  t_s: t, state, alarms: ded.filter((d) => d[1] >= 1).map((d) => d[0]),
  marine: pos ? { position: pos, stats: {}, ratios: ded.map((d) => [d[0], d[1]]) } : undefined,
  score: score == null ? undefined : { score, band: state, deductions: ded.map((d) => ({ monitor: d[0], ratio: d[1], points: d[2] })) },
});
const result = {
  score_model: model, monitors_run: ["kinematic", "heading-course", "cn0-spread", "agc"], states: {}, events: [{ label: "drag", onset_s: 5, end_s: null }],
  epochs: [
    mk(0, "calibrating", null, [], [59.0, 24.0]), mk(1, "calibrating", null, [], [59.001, 24.0]),
    mk(2, "nominal", 100, [], [59.002, 24.0]), mk(3, "nominal", 95, [["agc", 0.4, 0]], [59.003, 24.0]),
    mk(4, "degraded", 70, [["cn0-spread", 1.1, 30]], [59.004, 24.0]), mk(5, "untrusted", 30, [["heading-course", 2, 40], ["cn0-spread", 1.6, 30]], [59.005, 24.001]),
    mk(6, "untrusted", 20, [["heading-course", 2, 40], ["kinematic", 1.2, 40]], [59.006, 24.002]), mk(7, "nominal", 100, [], [59.007, 24.002]),
    mk(8, "nominal", 100, [], [59.008, 24.002]), mk(9, "nominal", 100, [], [59.009, 24.002]),
  ],
};

// Sniffing by content.
assert.equal(sniff(JSON.stringify(result, null, 2)), "result");
assert.equal(sniff('{"seq":1,"t_s":0,"state":"calibrating","score":null}\n{"seq":2,"t_s":1}'), "lines");
assert.equal(sniff("$GPGGA,080000.00,5927.2,N,02446.2,E,1,12,0.9,16.7,M,21.0,M,,*5B"), "nmea");
assert.equal(sniff("t_s,true_lat_deg,true_lon_deg\n0,59,24"), "truth");
assert.equal(sniff("t_s,state,n_sats,alarms\n"), "csv");
assert.equal(sniff("  "), "empty");
assert.equal(sniff("hello"), "unknown");

// A batch result.
{
  const { run, error } = buildRun({ result });
  assert.equal(error, undefined);
  assert.equal(run.epochs.length, 10);
  assert.equal(run.hasPosition, true);
  assert.equal(run.firstDeduction, 3);
  assert.equal(run.firstUntrusted, 5);
  assert.equal(run.gateDerived, true);
  assert.deepEqual(run.events, [{ label: "drag", onset_s: 5, end_s: null }]);
  assert.deepEqual(run.strips, ["kinematic", "heading-course", "cn0-spread", "agc"], "strips are the monitors that ran, in the documented order");
  // Reasons: largest first with the weight each monitor carries; faint = ran and cost nothing; not-run listed apart.
  const r = reasonsAt(run, 5);
  assert.deepEqual(r.ded.map((d) => [d.monitor, d.points, d.weight]), [["heading-course", 40, 40], ["cn0-spread", 30, 30]]);
  assert.deepEqual(r.faint.sort(), ["agc", "kinematic"]);
  assert.ok(r.notRun.includes("osnma") && !r.notRun.includes("agc"), "a monitor that did not run is never shown as passing");
  assert.equal(r.notRun.length + run.monitorsRun.length, KNOWN_MONITORS.length);
  // Above onset (ratio > onset_ratio): agc 0.4 is below it.
  assert.deepEqual(epochsAboveOnset(run, "cn0-spread"), [4, 5]);
  assert.deepEqual(epochsAboveOnset(run, "agc"), []);
  // Stepping honours the band filter.
  assert.equal(stepIndex(run, 0, 1, false), 1);
  assert.equal(stepIndex(run, 0, 1, true), 4);
  assert.equal(stepIndex(run, 6, 1, true), 6, "no later degraded or untrusted epoch: stay");
  assert.equal(stepIndex(run, 4, -1, true), 4);
  // Gate: untrusted at 5 and 6; released 30 s after leaving the band.
  const g = deriveGate(run.epochs, 30);
  assert.deepEqual(g.slice(0, 5), ["off", "off", "passed", "passed", "passed"]);
  assert.deepEqual(g.slice(5), ["withheld", "withheld", "withheld", "withheld", "withheld"], "held until gate_release_s has passed");
  const g2 = deriveGate(run.epochs, 1);
  assert.deepEqual(g2.slice(5), ["withheld", "withheld", "withheld", "passed", "passed"]);
  assert.match(describeEpoch(run, 5), /untrusted.*heading-course 40\.0 points.*gate withheld \(derived\)/);
}

// Runs by band share their joints so the line has no gaps.
{
  const { run } = buildRun({ result });
  const runs = bandRuns(run, false);
  assert.deepEqual(runs.map((r) => r.band), ["calibrating", "nominal", "degraded", "untrusted", "nominal"]);
  assert.equal(runs[1].idx[0], runs[0].idx[runs[0].idx.length - 1]);
  assert.deepEqual(bandRuns(run, true).map((r) => r.band), ["degraded", "untrusted"]);
}

// Refusals say why.
assert.match(buildRun({ result: { epochs: [] } }).error, /score_model|epochs/);
assert.match(buildRun({ result: { epochs: [{ t_s: 0, state: "nominal" }] } }).error, /static-platform/);
assert.match(buildRun({}).error, /Nothing to draw/);
assert.match(buildRun({ lines: [] }).error, /no epochs/);

// Live JSON lines (schema 1): no position, a gate, and the model is assumed.
{
  const lines = [
    { seq: 1, t_s: 0, time: "2025-06-14T08:00:00.000Z", state: "calibrating", score: null, deductions: [], alarms: [], gate: "off", note: null },
    { seq: 2, t_s: 1, time: "2025-06-14T08:00:01.000Z", state: "nominal", score: 100, deductions: [], alarms: [], gate: "passed", note: null },
    { seq: 3, t_s: 2, time: "2025-06-14T08:00:02.000Z", state: "untrusted", score: 23.4, deductions: [{ monitor: "heading-course", ratio: 2, points: 40 }], alarms: ["heading-course"], gate: "withheld", note: null },
    { seq: 4, t_s: 3, time: "2025-06-14T08:00:03.000Z", state: "untrusted", score: 20, deductions: [{ monitor: "future-monitor", ratio: 3, points: 20 }], alarms: ["future-monitor"], gate: "withheld", note: null },
  ];
  const p = parseLines(lines.map((l) => JSON.stringify(l)).join("\n") + "\nnot json\n");
  assert.equal(p.epochs.length, 4); assert.equal(p.skipped, 1);
  // The track comes from the NMEA the stream was read from, joined on time.
  const nmea = ["$GPGGA,080000.00,5927.29982,N,02446.20166,E,1,12,0.9,16.7,M,21.0,M,,", "$GPGGA,080001.00,5927.30464,N,02446.20304,E,1,12,0.9,16.7,M,21.0,M,,", "$GPGGA,080002.00,5927.30866,N,02446.20648,E,0,12,0.9,16.5,M,21.0,M,,", "$GPGGA,080003.00,5927.31333,N,02446.20820,E,1,12,0.9,16.0,M,21.0,M,,"]
    .map((b) => { let c = 0; for (let i = 1; i < b.length; i++) c ^= b.charCodeAt(i); return `${b}*${c.toString(16).toUpperCase().padStart(2, "0")}`; }).join("\r\n");
  const fixes = parseNmeaFixes(nmea);
  assert.equal(fixes.fixes.length, 4);
  assert.equal(fixes.fixes[2].valid, false, "a fix the receiver flags invalid is not drawn");
  assert.ok(Math.abs(fixes.fixes[0].lat - (59 + 27.29982 / 60)) < 1e-9 && Math.abs(fixes.fixes[0].lon - (24 + 46.20166 / 60)) < 1e-9);
  const { run } = buildRun({ lines: p.epochs, nmea: fixes });
  assert.equal(run.gateDerived, false, "the stream's own gate is used");
  assert.equal(run.epochs[2].pos, null, "the invalid-quality fix leaves no point");
  assert.ok(run.epochs[3].pos && run.epochs[0].pos);
  assert.equal(run.model.assumed, true);
  assert.equal(run.monitorsRun, null);
  assert.deepEqual(reasonsAt(run, 3).notRun, [], "not stated, so nothing is claimed as not run");
  assert.ok(run.strips.includes("future-monitor"), "an unknown monitor name is a monitor");
  assert.match(monitorInfo("future-monitor"), /does not have a description/);
  assert.ok(run.notes.some((n) => /does not state which monitors ran/.test(n)));
  assert.match(pkshtFor(run.epochs[2], run), /^\$PKSHT,1,080002\.00,23\.4,U,W,heading-course:40\.0\*[0-9A-F]{2}$/);
  // Later schema versions add a position key at the end.
  const v11 = buildRun({ lines: p.epochs.map((e, i) => ({ ...e, position: [59 + i * 0.001, 24] })) }).run;
  assert.equal(v11.hasPosition, true); assert.deepEqual(v11.epochs[2].pos, [59.002, 24]);
}

// NMEA: a corrupt checksum is skipped, southern and western hemispheres are signed.
{
  assert.equal(nmeaChecksumOk("$GPGGA,080000.00,5927.29982,N,02446.20166,E,1,12,0.9,16.7,M,21.0,M,,*5B"), true);
  assert.equal(nmeaChecksumOk("$GPGGA,080000.00,5927.29982,N,02446.20166,E,1,12,0.9,16.7,M,21.0,M,,*5C"), false);
  const body = "GPGGA,010203.50,3355.50000,S,15112.00000,W,1,08,1.0,5.0,M,0.0,M,,";
  let c = 0; for (const ch of body) c ^= ch.charCodeAt(0);
  const f = parseNmeaFixes(`$${body}*${c.toString(16).toUpperCase()}\n$${body}*00\n`);
  assert.equal(f.fixes.length, 1); assert.equal(f.skipped, 1);
  assert.ok(f.fixes[0].lat < 0 && f.fixes[0].lon < 0 && Math.abs(f.fixes[0].tod - (3723.5)) < 1e-9);
}

// Truth: joined by time; the offset is the engine's tangent-plane distance.
{
  const truth = parseTruth("t_s,true_lat_deg,true_lon_deg\n0,59.0,24.0\n1,59.001,24.0\n2,59.002,24.0\n3,59.003,24.0\n4,59.004,24.0\n5,59.005,24.0\n");
  assert.equal(truth.t.length, 6);
  const { run } = buildRun({ result, truth });
  assert.equal(run.hasTruth, true);
  assert.equal(run.epochs[5].offsetM > 50 && run.epochs[5].offsetM < 90, true, "0.001 degree of longitude at 59 N is about 57 m");
  assert.equal(run.epochs[3].offsetM, 0);
  assert.equal(run.epochs[8].truth, undefined, "no truth beyond the file");
  assert.match(buildRun({ result, truth: parseTruth("t_s,true_lat_deg,true_lon_deg\n500,1,1\n") }).run.notes.join(" "), /match no epoch/);
}

// Geometry.
{
  const [e, n] = enOffsetM(59, 24, 59, 24.001);
  assert.ok(Math.abs(n) < 1e-9 && e > 56 && e < 58);
  assert.ok(Math.abs(distanceM([59, 24], [59.001, 24]) - 111.4) < 0.5);
  assert.equal(niceLength(7300), 5000); assert.equal(niceLength(1999), 1000); assert.equal(niceLength(0), 0);
  const pr = makeProjection([[59, 24], [59.01, 24.02]]);
  assert.ok(pr.widthM > 1100 && pr.widthM < 1200 && pr.heightM > 1100 && pr.heightM < 1130);
  assert.equal(makeProjection([null]), null);
  assert.equal(nearestIndex([0, 1, 2, 3], 1.4), 1); assert.equal(nearestIndex([0, 1, 2, 3], 9, 0.5), -1);
}

// trust.csv: scores and reasons only.
{
  const csv = "t_s,state,n_sats,cn0_mean_dbhz,cn0_drop_db,agc,agc_z,jam_ind,position_offset_m,raim_stat,raim_thr,clock_innov_ns,clock_bound_ns,alarms,score,score_reasons\n0,calibrating,8,,,,,,,,,,,,,\n1,untrusted,8,,,,,,,,,,,heading-course;cn0-spread,23.4,heading-course:40.0;cn0-spread:30.0\n";
  const c = parseTrustCsv(csv);
  assert.equal(c.epochs.length, 2); assert.equal(c.epochs[0].score, null);
  assert.deepEqual(c.epochs[1].deductions, [{ monitor: "heading-course", ratio: null, points: 40 }, { monitor: "cn0-spread", ratio: null, points: 30 }]);
  assert.deepEqual(c.epochs[1].alarms, ["heading-course", "cn0-spread"]);
  const { run } = buildRun({ csv: c });
  assert.equal(run.hasPosition, false);
}

// Every monitor named in the documentation has a description (16).
assert.equal(KNOWN_MONITORS.length, 16);
for (const m of KNOWN_MONITORS) assert.ok(MONITOR_INFO[m].length > 20, m);

// The recorded demo, when it is present: the engine's own result for the synthetic ferry log.
const demo = new URL("../trust/demo/tallinn-helsinki.result.json", import.meta.url);
const demoTruth = new URL("../trust/demo/tallinn-helsinki.truth.csv", import.meta.url);
if (fs.existsSync(demo)) {
  const { run, error } = buildRun({ result: JSON.parse(fs.readFileSync(demo, "utf8")), truth: parseTruth(fs.readFileSync(demoTruth, "utf8")) });
  assert.equal(error, undefined);
  assert.ok(run.hasPosition && run.hasTruth);
  assert.equal(run.epochs[0].state, "calibrating");
  assert.ok(run.firstUntrusted > 1500 / 1, "the drag-off begins at 1500 s, so nothing before it is untrusted");
  assert.ok(run.epochs.slice(0, 1500).every((e) => e.state !== "untrusted"), "no untrusted epoch before the drag-off");
  assert.ok(run.epochs[2900].offsetM > 100, "reported and true positions have separated by the end");
  assert.ok(run.epochs[1000].offsetM < 30, "and are close before the drag-off");
  assert.ok(run.epochs.every((e) => e.pos), "every epoch has a reported position");
}
console.log("trust.test.mjs: ok");
