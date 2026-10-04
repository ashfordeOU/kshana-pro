// Build-time extracts of REAL engine output for every chart on the site.
// Reads the playground's recorded runs (engine v0.28.0, produced by tools_record.mjs from the
// bundled scenarios) and computes each chart's model with the playground's own view code
// (lib/views.mjs), so a site chart and the playground view it links to draw the same data.
// Usage: node build_viz.mjs <playground dir> <out json> [<requests json> <land json>]
// With a requests file ([{id,w,h,variant}]) it also pre-renders each requested chart as a
// static SVG with the playground's own renderer, so the page shows it before any script runs.
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const [pgDir, outFile, reqFile, landFile] = process.argv.slice(2);
const V = await import(pathToFileURL(path.join(pgDir, "lib", "views.mjs")).href);
const recDir = path.join(pgDir, "recorded");

const cache = new Map();
function rec(file) {
  if (cache.has(file)) return cache.get(file);
  const p = path.join(recDir, `${file}.json`);
  if (!fs.existsSync(p)) throw new Error(`no recording for ${file}`);
  const a = JSON.parse(fs.readFileSync(p, "utf8"));
  const r = typeof a.json === "string" ? JSON.parse(a.json) : a.json;
  const out = { a, r };
  cache.set(file, out);
  return out;
}

// Keep a series' shape while bounding its size: every k-th point plus the last, and every
// point where the value changes sharply (so a step or a spike is never smoothed away).
function thin(points, max = 260) {
  if (!Array.isArray(points) || points.length <= max) return points;
  const k = Math.ceil(points.length / max);
  const out = [];
  for (let i = 0; i < points.length; i++) if (i % k === 0 || i === points.length - 1) out.push(points[i]);
  return out;
}
const round = (v, sig = 5) => (typeof v === "number" && Number.isFinite(v) ? Number(v.toPrecision(sig)) : v);
const rp = (pts) => pts.map(([x, y]) => [round(x, 7), round(y)]);
function lineModel(m, max) {
  return { ...m, series: m.series.map((s) => ({ ...s, points: rp(thin(s.points, max)) })) };
}
function meta(file, tab) {
  const { a, r } = rec(file);
  const tomlSeed = (a.toml || "").match(/^\s*seed\s*=\s*(\d+)/m);
  return {
    file,
    tab,
    engine: a.engine_version,
    seed: typeof r.seed === "number" ? r.seed : tomlSeed ? Number(tomlSeed[1]) : null,
    hash: typeof r.scenario_hash === "string" ? r.scenario_hash.slice(0, 12) : null,
    summary: a.summary || null,
  };
}

const V_OUT = {};
function line(id, file, tab, opts = {}) {
  const { a, r } = rec(file);
  const m = V.seriesModel(r, a.toml);
  if (!m) throw new Error(`${id}: ${file} has no series model`);
  V_OUT[id] = { id, kind: "line", ...meta(file, tab), model: lineModel(m, opts.max || 240), opts: opts.svg || {} };
}
function adev(id, files, tab) {
  // One or more runs' Allan-deviation curves on one chart; each curve names its run.
  const series = [];
  const seen = new Set();
  for (const f of files) {
    const { r } = rec(f);
    for (const c of V.adevCurves(r)) {
      const key = c.label;
      if (seen.has(key)) continue;
      seen.add(key);
      series.push({ ...c, label: c.label, from: f, points: rp(c.points) });
    }
  }
  const colors = ["var(--s-tim)", "var(--s-nav)", "var(--s-orb)", "var(--s-spf)", "var(--s-itg)", "var(--s-int)"];
  series.forEach((s, i) => { s.color = colors[i % colors.length]; });
  V_OUT[id] = { id, kind: "line", ...meta(files[0], tab), files, model: { title: "Allan deviation", series, xLabel: "averaging time τ (s)", yLabel: "σy(τ)", unit: "", logX: true, logY: true, outages: [] }, opts: {} };
}
function heat(id, file, tab) {
  const { a, r } = rec(file);
  const s = V.signalModel(r, a.toml);
  if (!s) throw new Error(`${id}: ${file} has no signal model`);
  // Compact the cells to the two C/N0 fields, J/S and status.
  const cells = s.cells.map((row) => Object.fromEntries(Object.entries(row).map(([prn, x]) => [prn, {
    cn0_effective_dbhz: round(x.cn0_effective_dbhz, 4), cn0_nominal_dbhz: round(x.cn0_nominal_dbhz, 4), js_db: round(x.js_db, 4), status: x.status,
  }])));
  const epochs = Array.isArray(r.epochs) ? r.epochs.map((e) => ({ t: e.t ?? e.t_s, visible: e.visible, tracking: e.tracking, state: e.gnss_state || null })) : [];
  V_OUT[id] = { id, kind: "heat", ...meta(file, tab), model: { ...s, cells }, epochs, fom: r.fom || null };
}
function ground(id, file, tab) {
  const { r } = rec(file);
  const g = V.groundTrack(r);
  if (!g) throw new Error(`${id}: no ground track`);
  V_OUT[id] = { id, kind: "ground", ...meta(file, tab), model: { ...g, points: thin(g.points, 400).map((p) => ({ lat: round(p.lat, 5), lon: round(p.lon, 6), visible: p.visible })) }, source: r.source || null };
}
function masks(id, file, tab) {
  const { r } = rec(file);
  const m = V.masksModel(r);
  if (!m) throw new Error(`${id}: no masks model`);
  V_OUT[id] = { id, kind: "mask", ...meta(file, tab), model: { ...m, mtie: rp(m.mtie), tdev: rp(m.tdev) }, oscillator: r.oscillator ? { model: r.oscillator.model, class: r.oscillator.class, preset: r.oscillator.preset, evidence: r.oscillator.evidence } : null };
}
// Inertial coasting through a GNSS outage, on the axis that shows it: time since GNSS was lost,
// position error on a log scale (a tactical-grade and a flight-controller IMU differ by two orders
// of magnitude, which a linear axis flattens into an empty chart). The position budget is drawn as
// a flat series (a log chart has no threshold line) and each IMU's engine-reported holdover
// (fom.holdover_s, seconds inside the budget) is marked where it leaves the budget.
function coast(id, file, tab, lead = 20) {
  const { a, r } = rec(file);
  const m = V.seriesModel(r, a.toml);
  if (!m || !m.outages || !m.outages.length) throw new Error(`${id}: ${file} has no outage to coast through`);
  const [t0, t1] = m.outages[0];
  const budget = m.threshold;
  const series = m.series.map((s) => ({ ...s, points: s.points.filter((p) => p[0] >= t0 - lead && p[0] <= t1 && p[1] > 0).map((p) => [p[0] - t0, p[1]]) }));
  const blocks = [r.quantum, r.classical].filter(Boolean);
  const marks = [];
  blocks.forEach((b, i) => {
    const h = b.fom && b.fom.holdover_s;
    const ser = series.find((s) => s.label === (b.spec && b.spec.id));
    if (!ser || !Number.isFinite(h) || !Number.isFinite(budget)) return;
    const inside = h < t1 - t0 - 1e-6;
    if (inside) marks.push({ x: h, y: budget, label: `${round(h, 3)} s`, color: ser.color });
  });
  if (Number.isFinite(budget)) series.push({ label: `${budget} m position budget`, color: "var(--s-int)", points: [[-lead, budget], [t1 - t0, budget]] });
  const model = { title: "Position error since GNSS was lost", xLabel: "time since GNSS was lost (s)", yLabel: "position error (m), log scale", unit: "m",
    series: series.map((s) => ({ ...s, points: rp(thin(s.points, 240)) })), threshold: null, outages: [[0, t1 - t0]], logY: true };
  const hold = blocks.map((b) => ({ id: b.spec && b.spec.id, holdover_s: b.fom && b.fom.holdover_s, outage_s: t1 - t0, fused_outage_rms_m: b.fused_outage_rms_m ?? null, free_outage_rms_m: b.free_outage_rms_m ?? null }));
  V_OUT[id] = { id, kind: "line", ...meta(file, tab), model, opts: { logY: true, marks }, figures: { budget_m: budget, imus: hold } };
}
function figures(id, file, tab, pick) {
  const { r } = rec(file);
  V_OUT[id] = { id, kind: "figures", ...meta(file, tab), figures: pick(r) };
}

// ---------------------------------------------------------------- the site's charts
// Home: the flagship waterfall and the six failure cards.
heat("wf-jamming", "jamming-demo", "signal");
line("fail-int", "jamming-demo", "timeseries");
line("fail-spf", "spoof-meaconing", "timeseries");
line("fail-tim", "clock-holdover", "timeseries");
ground("fail-orb", "ephemeris", "ground");
line("fail-itg", "integrity-raim", "timeseries");
coast("fail-nav", "small-uas-jammed-nav", "timeseries");
// Missions: one real run per sector.
line("sec-defence", "spoof-attack", "timeseries");
line("sec-space", "orbit-gnss-challenged", "timeseries");
line("sec-aviation", "araim-gps-galileo", "timeseries");
masks("sec-telecom", "telecom-prtc-holdover-24h", "masks");
line("sec-cni", "clock-ensemble", "timeseries");
line("sec-research", "sweep-clock-stability", "timeseries");
line("sec-lunar", "lunanet-araim", "timeseries");
line("sec-mars", "mars-pnt-lmo", "timeseries");
line("sec-rail-nearest", "terrain-slam", "timeseries");
// Maritime and road/rail sectors, once their scenarios are committed and recorded.
for (const [id, f] of [["sec-maritime", "maritime-strait-jamming"], ["sec-automotive", "automotive-urban-canyon"]]) {
  if (fs.existsSync(path.join(recDir, `${f}.json`))) line(id, f, "timeseries");
}
line("sec-maritime-nearest", "imu-deadreckoning", "timeseries");
// Capabilities: clocks, timing wall, journey.
adev("clk-adev", ["clock-holdover", "clock-holdover-labsr"], "stability");
line("clk-hold-a", "clock-holdover", "holdover");
line("clk-hold-b", "orbit-molniya", "holdover");
line("clk-slot", "slot-timing-ocxo-leo", "holdover");
masks("tw-mtie", "telecom-prtc-holdover-24h", "masks");
figures("tw-lunar", "lunar-time-offset", "overview", (r) => ({
  secular_rate_us_per_day: round(r.secular_rate_us_per_day), band_low: r.band_low, band_high: r.band_high,
  self_potential_us_per_day: round(r.self_potential_us_per_day), kinetic_us_per_day: round(r.kinetic_us_per_day),
  topographic_spread_ns_per_day: round(r.topographic_spread_ns_per_day),
}));
figures("clk-lunar-classes", "lunar-time-budget", "overview", (r) => (r.clock_crossovers || []).map((c) => ({
  clock: c.clock, noise_type: c.noise_type, sigma_y_one_s: round(c.sigma_y_one_s), x_one_day_s: round(c.x_one_day_s),
})));
figures("tw-telecom-budgets", "telecom-prtc-holdover-24h", "masks", (r) => ({
  budgets: r.budgets, envelope: r.holdover_envelope, oscillator: { model: r.oscillator.model, class: r.oscillator.class, evidence: r.oscillator.evidence },
}));
figures("slot-result", "slot-timing-ocxo-leo", "holdover", (r) => ({ result: r.result, slot: { guard_ns: r.slot.guard_ns, elapsed_since_sync_s: r.slot.elapsed_since_sync_s }, oscillator: { source_detail: r.oscillator.source_detail, sigma_y_1s: round(r.oscillator.sigma_y_1s) } }));
heat("band-l1", "jamming-demo", "signal");
heat("band-lunar-s", "lunar-jamming", "signal");
ground("jr-earth", "ephemeris", "ground");
line("jr-moon", "lunanet-araim", "timeseries");
line("jr-mars", "mars-pnt-surface", "timeseries");
// Home hero: the ISS orbit the ephemeris scenario propagates (GCRS, km), replayed on the globe.
figures("hero-track", "ephemeris", "orbit", (r) => r.samples.filter((_, i) => i % 3 === 0).map((p) => p.gcrs_r_m.map((v) => round(v / 1000, 6))));
// Developers: what an assistant receives from the MCP server's run_scenario.
figures("mcp-imu", "imu-deadreckoning", "timeseries", (r) => ({
  threshold_m: r.threshold_m,
  quantum: { id: r.quantum.spec.id, fom: r.quantum.fom },
  classical: { id: r.classical.spec.id, fom: r.classical.fom },
}));
// Editions and the Pro band: the open-engine foundations each deliverable builds on.
adev("pro-twin", ["clock-holdover"], "stability");
line("pro-trade", "sweep-clock-stability", "timeseries");
figures("pro-repro", "clock-holdover", "json", (r) => ({ scenario_hash: r.scenario_hash, seed: r.seed, engine_version: r.engine_version, schema_version: r.schema_version, threshold_ns: r.threshold_ns }));

// The engine at work (Capabilities): five recorded runs, each with the engine's own figures.
line("eng-quantum", "imu-deadreckoning", "timeseries");
const specFom = (r, keys) => ["quantum", "classical"].filter((k) => r[k]).map((k) => ({ id: r[k].spec.id, ...Object.fromEntries(keys.map((x) => [x, round(r[k].fom[x])])) }));
figures("eng-fom-hold", "clock-holdover", "holdover", (r) => ({ threshold_ns: r.threshold_ns, clocks: specFom(r, ["holdover_s", "timing_p95_ns"]) }));
figures("eng-fom-itg", "integrity-raim", "timeseries", (r) => ({ al_h_m: r.al_h_m, al_v_m: r.al_v_m, samples_available: r.samples_available, samples_total: r.samples_total }));
figures("eng-fom-quantum", "imu-deadreckoning", "timeseries", (r) => ({ threshold_m: r.threshold_m, imus: specFom(r, ["holdover_s", "pos_p95_m"]) }));

fs.writeFileSync(outFile, JSON.stringify(V_OUT));
console.log(`viz: ${Object.keys(V_OUT).length} charts from ${cache.size} recordings, ${(fs.statSync(outFile).size / 1024).toFixed(0)} KB`);

// ---------------------------------------------------------------- static SVG renders
if (reqFile) {
  const reqs = JSON.parse(fs.readFileSync(reqFile, "utf8"));
  const land = landFile ? JSON.parse(fs.readFileSync(landFile, "utf8")) : [];
  const svgs = {};
  for (const q of reqs) {
    const v = V_OUT[q.id];
    if (!v) throw new Error(`requested chart ${q.id} is not defined in build_viz.mjs`);
    const key = `${q.id}|${q.w}|${q.h}|${q.variant || ""}`;
    let svg = "";
    if (v.kind === "line") svg = V.lineChartSvg(v.model, { ...(v.opts || {}), w: q.w, h: q.h }).svg;
    else if (v.kind === "heat") svg = V.signalHeatmapSvg(v.model, { w: q.w, titles: false, field: q.variant === "nominal" ? "cn0_nominal_dbhz" : "cn0_effective_dbhz" });
    else if (v.kind === "ground") svg = V.groundTrackSvg(v.model, { w: q.w, land });
    else if (v.kind === "mask") {
      const mm = v.model;
      const metric = q.variant === "tdev" ? "tdev" : "mtie";
      const pts = metric === "tdev" ? mm.tdev : mm.mtie;
      svg = V.lineChartSvg({ title: metric.toUpperCase(), series: [{ label: `${metric.toUpperCase()} (ns)`, color: metric === "tdev" ? "var(--s-spf)" : "var(--s-tim)", points: pts }], xLabel: "observation interval τ (s)", yLabel: `${metric.toUpperCase()} (ns)`, unit: "ns" }, { logX: true, logY: true, w: q.w, h: q.h }).svg;
    }
    if (!svg) throw new Error(`chart ${q.id} rendered empty`);
    svgs[key] = svg;
  }
  const out = JSON.parse(fs.readFileSync(outFile, "utf8"));
  fs.writeFileSync(outFile, JSON.stringify({ charts: out, svgs }));
  console.log(`svg: ${Object.keys(svgs).length} static renders`);
}
