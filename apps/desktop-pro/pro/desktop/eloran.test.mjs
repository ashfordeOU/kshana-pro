// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// node --test apps/desktop-pro/pro/desktop/*.test.mjs
// The eLoran page (Enterprise, pro-eloran): its models of the service's /eloran answers, the
// snippet insertion, and its actions over a fake service (synchronous run, job run, receiver).
import test from "node:test";
import assert from "node:assert/strict";
import { validateModel, griRows, griFacets, insertSnippet, definedTimeRefs, definedChains, runModel, rxModel, createEloran, eloranActions } from "./eloran.mjs";
import { proApi } from "./service.mjs";

const GRI = {
  format: "kshana-eloran-gri-db", updated: "2026-10-01", count: 3,
  chains: [
    { id: "nela-7499", gri: 7499, gri_us: 74990, name: "Lessay", region: "Europe", system: "eloran", status: "terminated", confidence: "verified", complete: true, operational_from: "1995", operational_to: "2015-12-31", countries: ["FR"],
      stations: [{ id: "M", name: "Lessay", role: "master", lat_deg: 49.15, lon_deg: -1.5 }, { id: "X", name: "Soustons", role: "secondary", lat_deg: 43.7, lon_deg: -1.4 }] },
    { id: "korea-9930", gri: 9930, gri_us: 99300, name: "East Asia", region: "Asia", system: "eloran", status: "testbed", confidence: "single-source", complete: false, countries: ["KR"],
      stations: [{ id: "M", name: "Pohang", role: "master", lat_deg: 36.1, lon_deg: 129.3 }, { id: "Y", name: "Unknown", role: "secondary", lat_deg: null, lon_deg: null }] },
    { id: "uk-6731", gri: 6731, gri_us: 67310, name: "Anthorn", region: "Europe", system: "eloran", status: "operational", confidence: "verified", complete: true, countries: ["GB"],
      stations: [{ id: "M", name: "Anthorn", role: "master", lat_deg: 54.9, lon_deg: -3.3 }] },
  ],
};
const SNIPPET = `# Anthorn — kshana-eloran GRI database entry "uk-6731"
[[chains]]
name = "Anthorn"
gri_us = 67310
[[chains.stations]]
id = "M"
name = "Anthorn"
role = "master"
position = { lat_deg = 54.9, lon_deg = -3.3 }
time_ref = "UTC(NPL)"
`;
const SCENARIO = `[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 1.0
fs_hz = 1000000
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 5.0
[[chains]]
name = "chain-A"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
`;
const RECORD = {
  format: "kshana-eloran-run", version: 1, mode: "sync", run_id: "a".repeat(64), scenario_sha256: "b".repeat(64), evidence_label: "MODELLED",
  stats: { samples_per_channel: 1000000, channels: 1, pulses: 96, truth_records: 12, clipped_samples: 0, peak_abs: 0.4321, wall_s: 0.5, realtime_factor: 2.04 },
  files: { samples: "/tmp/r/recording.sigmf-data", metadata: "/tmp/r/recording.sigmf-meta", truth: "/tmp/r/recording.truth.csv", run: "/tmp/r/recording.run.json" },
  artefacts: { samples_sha256: "c".repeat(64) },
  truth_preview: { columns: ["t_emit_s", "chain", "station", "group", "toa_s", "delay_ns", "geom_ns", "ecd_us", "snr_db"], rows: [["0.0", "chain-A", "M", "0", "0.00063", "630.1", "629.0", "0.1", "40.2"]], total: 12 },
};
const REPORT = {
  format: "kshana-eloran-rx", version: 1, run_id: "a".repeat(64), stations_tracked: 2, measurements: 24, evidence_label: "MODELLED",
  time_solution: [{ chain: "chain-A", station: "M", time_ref: "UTC(A)", ref_minus_rx_ns: 50.12, std_ns: 0.4, n: 12 }],
  time_by_reference: [{ time_ref: "UTC(A)", ref_minus_rx_ns: 50.1, stations: 2 }],
  position_solution: { lat_deg: 55.00001, lon_deg: 3.00002, hdop: 1.8, clock_bias_ns: [["UTC(A)", -49.9]], residuals_ns: [0.1], converged: true },
  truth_comparison: {
    stations: [
      { chain: "chain-A", station: "M", channel: null, n: 12, toa_mean_ns: 0.2, toa_std_ns: 0.5, toa_rms_ns: 0.54, toa_max_abs_ns: 1.1, ecd_rms_err_us: 0.002, cycle_errors: 0 },
      { chain: "chain-A", station: "X", channel: null, n: 12, toa_mean_ns: 1.2, toa_std_ns: 0.5, toa_rms_ns: 1.3, toa_max_abs_ns: 2.1, ecd_rms_err_us: 0.004, cycle_errors: 1 },
    ],
    position_error_m: 1.734,
  },
};

test("validate: the hash, or the service's error", () => {
  const ok = validateModel({ ok: true, value: { ok: true, scenario_sha256: "f".repeat(64), duration_s: 1, fs_hz: 2e6, channels: 2, chains: [{ name: "A", gri_us: 67310, stations: 3 }], samples_total: 4e6, synchronous_limit: 8e6 } });
  assert.equal(ok.ok, true);
  assert.equal(ok.hash, "f".repeat(64));
  assert.match(ok.text, /SHA-256 f{64}/);
  assert.match(ok.detail, /A \(GRI 6731, 3 stations\)/);
  assert.doesNotMatch(ok.detail, /data directory/);
  const big = validateModel({ ok: true, value: { ok: true, scenario_sha256: "f", duration_s: 60, fs_hz: 2e6, channels: 2, chains: [], samples_total: 2.4e8, synchronous_limit: 8e6 } });
  assert.match(big.detail, /data directory/);
  const bad = validateModel({ ok: false, value: { ok: false, error: "scenario: missing field `scene`" }, status: 422, error: "422: scenario: missing field `scene`" });
  assert.equal(bad.ok, false);
  assert.equal(bad.text, "scenario: missing field `scene`");
  assert.equal(validateModel({ ok: false, value: null, error: "not in the app" }).text, "not in the app");
});

test("GRI database: rows with status and confidence tones, filters, facets", () => {
  const all = griRows(GRI);
  assert.deepEqual(all.map((r) => r.id), ["korea-9930", "uk-6731", "nela-7499"], "by region, then GRI");
  const k = all[0];
  assert.equal(k.placed, 1, "a station without a position is listed, not placed");
  assert.equal(k.statusTone, "ok");
  assert.equal(k.confidenceTone, "warn");
  assert.equal(all[2].era, "1995 – 2015-12-31");
  assert.equal(all[1].era, "");
  assert.deepEqual(griRows(GRI, { region: "Europe" }).map((r) => r.id), ["uk-6731", "nela-7499"]);
  assert.deepEqual(griRows(GRI, { status: "terminated" }).map((r) => r.id), ["nela-7499"]);
  assert.deepEqual(griRows(GRI, { search: "soustons" }).map((r) => r.id), ["nela-7499"], "a station name finds its chain");
  assert.deepEqual(griRows(GRI, { search: "9930" }).map((r) => r.id), ["korea-9930"]);
  assert.deepEqual(griFacets(GRI), { regions: ["Asia", "Europe"], statuses: ["operational", "terminated", "testbed"] });
  assert.deepEqual(griRows(null), []);
});

test("snippet insertion: appended once, missing time references defined", () => {
  assert.deepEqual([...definedTimeRefs(SCENARIO)], ["UTC(A)"]);
  assert.deepEqual([...definedChains(SCENARIO)], ["chain-A"]);
  const r = insertSnippet(SCENARIO, SNIPPET);
  assert.equal(r.added, true);
  assert.deepEqual(r.timeRefs, ["UTC(NPL)"]);
  assert.ok(r.text.startsWith(SCENARIO));
  assert.match(r.text, /\[\[time_refs\]\]\nid = "UTC\(NPL\)"\noffset_ns = 0\.0/);
  assert.ok(r.text.endsWith(SNIPPET));
  assert.deepEqual([...definedChains(r.text)], ["chain-A", "Anthorn"]);
  assert.deepEqual([...definedTimeRefs(r.text)], ["UTC(A)", "UTC(NPL)"]);
  const again = insertSnippet(r.text, SNIPPET);
  assert.equal(again.duplicate, true);
  assert.equal(again.text, r.text);
  // A scenario that already defines the reference gets no second definition.
  const withRef = SCENARIO + '[[time_refs]]\nid = "UTC(NPL)"\n';
  assert.deepEqual(insertSnippet(withRef, SNIPPET).timeRefs, []);
  assert.equal(insertSnippet(SCENARIO, "not a snippet").added, false);
  assert.equal(insertSnippet("", SNIPPET).text.startsWith("[[time_refs]]"), true);
});

test("run record: statistics, files and a truth preview of chosen columns; a job ticket is queued", () => {
  const m = runModel(RECORD);
  assert.equal(m.queued, false);
  assert.equal(m.runId, "a".repeat(64));
  assert.deepEqual(m.stats.find(([k]) => k === "Peak (full scale)"), ["Peak (full scale)", "0.432"]);
  assert.deepEqual(m.stats.find(([k]) => k === "Real-time factor"), ["Real-time factor", "2.0"]);
  assert.equal(m.clipped, false);
  assert.equal(m.hasRecording, true);
  assert.deepEqual(m.preview.columns, ["chain", "station", "group", "toa_s", "delay_ns", "ecd_us", "snr_db"]);
  assert.deepEqual(m.preview.rows[0], ["chain-A", "M", "0", "0.00063", "630.1", "0.1", "40.2"]);
  assert.equal(m.preview.total, 12);
  assert.equal(m.samplesSha, "c".repeat(64));
  const ticket = runModel({ job_id: "d".repeat(64), run_id: "d".repeat(64), mode: "job", state: "queued" });
  assert.deepEqual(ticket, { queued: true, runId: "d".repeat(64), mode: "job", state: "queued" });
  const truthOnly = runModel({ ...RECORD, files: { samples: null, truth: "/t" } });
  assert.equal(truthOnly.hasRecording, false);
  assert.equal(runModel(null), null);
});

test("receiver report: per-station ToA error and cycle errors, the fix and its error, time per reference", () => {
  const r = rxModel(REPORT);
  assert.equal(r.tracked, 2);
  assert.deepEqual(r.stations.map((s) => [s.key, s.toaRms, s.cycleErrors, s.tone]), [["chain-A/M", "0.54", 0, "ok"], ["chain-A/X", "1.30", 1, "bad"]]);
  assert.equal(r.cycleErrors, 1);
  assert.equal(r.fix.ok, true);
  assert.equal(r.fix.errorM, "1.73");
  assert.match(r.fix.text, /55\.000010° N, 3\.000020° E · HDOP 1\.80 · error 1\.73 m/);
  assert.deepEqual(r.biases, [["UTC(A)", "-49.90"]]);
  assert.deepEqual(r.byRef, [{ ref: "UTC(A)", offset: "50.10", stations: 2 }]);
  assert.equal(r.perStation[0].offset, "50.12");
  const nofix = rxModel({ ...REPORT, position_solution: { error: "fewer than three stations" }, truth_comparison: undefined });
  assert.equal(nofix.fix.ok, false);
  assert.match(nofix.fix.text, /fewer than three/);
  assert.equal(nofix.hasTruth, false);
  assert.throws(() => rxModel({ format: "x" }), /not a receiver report/);
});

/** A fake app: the service answers by method and path; `sent` records every call. */
function fakeApp(answer) {
  const sent = [];
  const papi = proApi(async (cmd, a) => {
    assert.equal(cmd, "engine_call");
    assert.equal(a.name, "pro_service");
    sent.push(a.args);
    return answer(a.args[0], a.args[1], a.args[2]);
  });
  return { papi, sent };
}

test("actions: example and database loaded, validate, insert, synchronous run, receiver", async () => {
  const { papi, sent } = fakeApp((m, p, body) => {
    if (p === "/eloran/example") return { status: 200, text: SCENARIO };
    if (p === "/eloran/gri") return { status: 200, json: GRI };
    if (p === "/eloran/gri/uk-6731") return { status: 200, json: { count: 1, chains: [{ summary: { name: "Anthorn", gri: 6731 }, snippet: SNIPPET }] } };
    if (p === "/eloran/gri/korea-9930") return { status: 200, json: { count: 1, chains: [{ summary: { name: "East Asia", gri: 9930 }, snippet: null, snippet_error: "station Y has no position" }] } };
    if (p === "/eloran/validate") return body.includes("[scene]") ? { status: 200, json: { ok: true, scenario_sha256: "e".repeat(64), duration_s: 1, fs_hz: 1e6, channels: 1, chains: [] } } : { status: 422, json: { ok: false, error: "scenario: bad" } };
    if (p === "/eloran/run") return { status: 200, json: RECORD };
    if (p === "/eloran/rx") return { status: 200, json: REPORT };
    return { status: 404, json: { error: "not found" } };
  });
  const E = createEloran();
  const said = [];
  let draws = 0;
  const A = eloranActions(papi, E, { say: (tone, text) => said.push([tone, text]), redraw: () => { draws++; } });
  await A.example();
  await A.gri();
  assert.equal(E.text, SCENARIO);
  assert.equal(E.gri.count, 3);
  E.text = "edited";
  await A.example();
  assert.equal(E.text, "edited", "the example does not overwrite the user's text");
  await A.validate();
  assert.equal(E.validation.ok, false);
  await A.example(true);
  await A.validate();
  assert.equal(E.validation.hash, "e".repeat(64));
  assert.deepEqual(sent.at(-1), ["POST", "/eloran/validate", SCENARIO], "the TOML goes as it is");
  await A.insert("uk-6731");
  assert.ok(E.text.includes('name = "Anthorn"') && E.text.includes('id = "UTC(NPL)"'));
  assert.equal(E.validation, null, "an edit clears the last check");
  assert.equal(said.at(-1)[0], "ok");
  await A.insert("korea-9930");
  assert.match(said.at(-1)[1], /station Y has no position/);
  await A.insert("uk-6731");
  assert.equal(said.at(-1)[0], "warn");
  await A.run("");
  assert.equal(E.run.run_id, "a".repeat(64));
  await A.run("none");
  assert.equal(sent.at(-1)[1], "/eloran/run?format=none");
  await A.rx();
  assert.deepEqual(JSON.parse(sent.at(-1)[2]), { run: "a".repeat(64), per_loop: false, cri_blanking: true });
  assert.equal(E.rx.tracked, 2);
  assert.equal(E.busy, "");
  assert.ok(draws > 5);
  assert.ok(!sent.some(([, p]) => /stream|soapy|device/.test(p)), "the page never asks for hardware output");
});

test("actions: a job run is polled until it succeeds, a failed job and a refusal are errors", async () => {
  const id = "9".repeat(64);
  let polls = 0;
  const { papi } = fakeApp((m, p) => {
    if (p === "/eloran/run") return { status: 202, json: { job_id: id, run_id: id, mode: "job", state: "queued" } };
    if (p === `/jobs/${id}`) return { status: 200, json: { job_id: id, state: ++polls < 3 ? "running" : "succeeded" } };
    if (p === `/jobs/${id}/result`) return { status: 200, json: { ...RECORD, run_id: undefined, mode: "job" } };
    return { status: 404, json: { error: "not found" } };
  });
  const E = createEloran();
  const said = [];
  const A = eloranActions(papi, E, { say: (tone, text) => said.push([tone, text]), redraw: () => {}, wait: async () => {} });
  await A.run();
  assert.equal(polls, 3);
  assert.equal(E.run.run_id, id, "the job's id is the run's");
  assert.equal(runModel(E.run).queued, false);

  const failing = fakeApp((m, p) => (p === "/eloran/run" ? { status: 202, json: { job_id: id, run_id: id, mode: "job" } } : { status: 200, json: { state: "failed", error: "engine said no" } }));
  const E2 = createEloran();
  const said2 = [];
  await eloranActions(failing.papi, E2, { say: (t, x) => said2.push([t, x]), redraw: () => {}, wait: async () => {} }).run();
  assert.deepEqual(said2.at(-1), ["bad", "The run failed: engine said no"]);

  const refused = fakeApp(() => ({ status: 403, json: { error: "'pro-eloran' requires the 'enterprise' tier" } }));
  const E3 = createEloran();
  const said3 = [];
  const A3 = eloranActions(refused.papi, E3, { say: (t, x) => said3.push([t, x]), redraw: () => {} });
  await A3.run();
  assert.match(said3.at(-1)[1], /403: 'pro-eloran' requires the 'enterprise' tier/);
  await A3.rx();
  assert.match(said3.at(-1)[1], /Run the scenario first/);
});

test("actions: the live monitor page comes back as HTML and is kept for the sandboxed frame", async () => {
  const PAGE = "<!doctype html><title>eLoran Monitor</title>";
  const { papi, sent } = fakeApp((m, p) => {
    if (m === "POST" && p === "/eloran/monitor?format=html") return { status: 200, text: PAGE };
    return { status: 404, json: { error: "not found" } };
  });
  const E = createEloran();
  E.text = SCENARIO;
  const said = [];
  const A = eloranActions(papi, E, { say: (tone, text) => said.push([tone, text]), redraw: () => {} });
  await A.monitor();
  assert.equal(E.monitor, PAGE);
  assert.equal(said.at(-1)[0], "ok");
  assert.equal(sent.at(-1)[2], SCENARIO, "the scenario text is the request body");
  const refused = fakeApp(() => ({ status: 403, json: { error: "this capability is not available under this licence" } }));
  const E2 = createEloran();
  const said2 = [];
  await eloranActions(refused.papi, E2, { say: (t, x) => said2.push([t, x]), redraw: () => {} }).monitor();
  assert.equal(E2.monitor, "");
  assert.equal(said2.at(-1)[0], "bad");
});
