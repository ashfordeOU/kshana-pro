// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the eLoran/GNSS simulator page (Enterprise, feature pro-eloran), built on
// the Pro service's /eloran routes (src/serve/eloran.rs): the scenario editor (GET
// /eloran/example, POST /eloran/validate), the chain map and table from the GRI database (GET
// /eloran/gri, GET /eloran/gri/{chain} for a [[chains]] snippet), run controls (POST
// /eloran/run: a synchronous run, or a job on a service with a data directory) and the
// receiver results (POST /eloran/rx). Hardware output is not offered: it stays on the command
// line (kshana-pro eloran stream), conducted use only. The models are pure and tested in
// eloran.test.mjs; the view draws them with the map maths of geo.mjs.
import { fitView, PROJECTION, latLonText } from "./geo.mjs";

const STATUS_TONE = { operational: "ok", testbed: "ok", planned: "warn", unknown: "none", terminated: "none", never_built: "none" };
const CONF_TONE = { verified: "ok", "single-source": "warn", unverified: "bad" };
const num = (v) => (v === null || v === undefined || v === "" ? NaN : Number(v));
const fx = (v, d = 2) => (Number.isFinite(num(v)) ? num(v).toFixed(d) : "—");

/** The answer to POST /eloran/validate (a proApi result) as { ok, tone, text, hash }. */
export function validateModel(r) {
  if (r && r.ok && r.value && r.value.ok) {
    const v = r.value;
    const chains = Array.isArray(v.chains) ? v.chains.map((c) => `${c.name} (GRI ${Math.round(c.gri_us / 10)}, ${c.stations} stations)`).join(", ") : "";
    const big = Number(v.samples_total) > Number(v.synchronous_limit);
    return { ok: true, tone: "ok", hash: String(v.scenario_sha256), text: `Valid · SHA-256 ${v.scenario_sha256}`, detail: `${v.duration_s} s at ${Number(v.fs_hz) / 1e6} MHz, ${v.channels} channel(s); ${chains}${big ? ". Larger than a synchronous run: the service needs a data directory to run it as a job." : ""}` };
  }
  const why = (r && r.value && r.value.error) || (r && r.error) || "the scenario could not be checked";
  return { ok: false, tone: "bad", hash: "", text: String(why), detail: "" };
}

/** The GRI database (GET /eloran/gri) as table rows, filtered by region, status and text. */
export function griRows(list, { region = "", status = "", search = "" } = {}) {
  const chains = list && Array.isArray(list.chains) ? list.chains : [];
  const q = String(search || "").trim().toLowerCase();
  return chains
    .filter((c) => (!region || c.region === region) && (!status || c.status === status))
    .filter((c) => !q || [c.id, c.name, c.region, c.system, String(c.gri), ...(c.countries || []), ...(c.stations || []).map((s) => s.name)].some((x) => String(x || "").toLowerCase().includes(q)))
    .map((c) => {
      const stations = (c.stations || []).map((s) => ({ id: String(s.id), name: String(s.name || s.id), role: String(s.role || ""), pos: Number.isFinite(num(s.lat_deg)) && Number.isFinite(num(s.lon_deg)) ? { lat: num(s.lat_deg), lon: num(s.lon_deg) } : null }));
      return {
        id: String(c.id), gri: Number(c.gri), name: String(c.name), region: String(c.region || ""), system: String(c.system || ""),
        status: String(c.status || "unknown"), statusTone: STATUS_TONE[c.status] || "none",
        confidence: String(c.confidence || "unverified"), confidenceTone: CONF_TONE[c.confidence] || "none",
        complete: !!c.complete, era: [c.operational_from, c.operational_to].some(Boolean) ? `${c.operational_from || "?"} – ${c.operational_to || "today"}` : "",
        stations, placed: stations.filter((s) => s.pos).length,
      };
    })
    .sort((a, b) => a.region.localeCompare(b.region) || a.gri - b.gri || a.id.localeCompare(b.id));
}

/** The regions and statuses in the database, for the filters. */
export function griFacets(list) {
  const chains = list && Array.isArray(list.chains) ? list.chains : [];
  return { regions: [...new Set(chains.map((c) => c.region).filter(Boolean))].sort(), statuses: [...new Set(chains.map((c) => c.status).filter(Boolean))].sort() };
}

const tomlStr = (s) => `"${String(s).replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;

/** The `key` of each table under a header line `[[name]]` in a TOML text (first value only). */
function keysUnder(text, name, key) {
  const out = new Set();
  const head = new RegExp(`^\\s*\\[\\[${name.replace(/[.]/g, "\\.")}\\]\\]\\s*$`, "m");
  for (const p of String(text || "").split(head).slice(1)) {
    const body = p.split(/^\s*\[/m)[0];
    const m = body.match(new RegExp(`^\\s*${key}\\s*=\\s*"([^"]*)"`, "m"));
    if (m) out.add(m[1]);
  }
  return out;
}

/** The time references a scenario text defines in [[time_refs]] (their `id`s). */
export const definedTimeRefs = (text) => keysUnder(text, "time_refs", "id");
/** The chain names a scenario text defines in [[chains]]. */
export const definedChains = (text) => keysUnder(text, "chains", "name");

/**
 * Add a GRI database snippet ([[chains]] ...) to the end of a scenario text. A chain already in
 * the text (the same `name`) is not added twice. A time reference the snippet's stations use and
 * the scenario does not define gets a [[time_refs]] entry with a zero offset (to be edited), so
 * the result validates. Returns { text, added, duplicate, timeRefs }.
 */
export function insertSnippet(text, snippet) {
  const src = String(text || "");
  const sn = String(snippet || "");
  if (!/^\s*\[\[chains\]\]/m.test(sn)) return { text: src, added: false, duplicate: false, timeRefs: [] };
  const name = (sn.match(/^\s*name\s*=\s*"([^"]*)"/m) || [])[1];
  if (name && definedChains(src).has(name)) {
    return { text: src, added: false, duplicate: true, timeRefs: [] };
  }
  const have = definedTimeRefs(src);
  const used = [...new Set([...sn.matchAll(/^\s*time_ref\s*=\s*"([^"]+)"/gm)].map((m) => m[1]))];
  const missing = used.filter((r) => !have.has(r));
  const refs = missing.map((r) => `[[time_refs]]\nid = ${tomlStr(r)}\noffset_ns = 0.0   # added with the chain: set the reference's offset\n`).join("\n");
  const sep = src.length && !src.endsWith("\n") ? "\n\n" : src.length ? "\n" : "";
  return { text: `${src}${sep}${refs ? refs + "\n" : ""}${sn.endsWith("\n") ? sn : sn + "\n"}`, added: true, duplicate: false, timeRefs: missing };
}

const STAT_ROWS = [
  ["samples_per_channel", "Samples per channel", 0],
  ["channels", "Channels", 0],
  ["pulses", "Pulses", 0],
  ["truth_records", "Truth rows", 0],
  ["peak_abs", "Peak (full scale)", 3],
  ["clipped_samples", "Clipped samples", 0],
  ["realtime_factor", "Real-time factor", 1],
  ["wall_s", "Wall time (s)", 2],
];
const PREVIEW_COLS = ["chain", "station", "group", "toa_s", "delay_ns", "asf_ns", "ecd_us", "field_uv_m", "snr_db", "time_ref_offset_ns", "distance_km"];

/**
 * A run's answer (POST /eloran/run, or a job's result) as the page draws it. A job ticket
 * (no statistics yet) is { queued: true, runId }.
 */
export function runModel(rec) {
  if (!rec || typeof rec !== "object") return null;
  const runId = String(rec.run_id || rec.job_id || "");
  if (!rec.stats) return { queued: true, runId, mode: String(rec.mode || "job"), state: String(rec.state || "queued") };
  const st = rec.stats;
  const pv = rec.truth_preview || {};
  const cols = Array.isArray(pv.columns) ? pv.columns : [];
  const keep = PREVIEW_COLS.map((c) => cols.indexOf(c)).filter((i) => i >= 0);
  const idx = keep.length ? keep : cols.map((_, i) => i);
  return {
    queued: false, runId, mode: String(rec.mode || ""), hash: String(rec.scenario_sha256 || ""), label: String(rec.evidence_label || ""),
    stats: STAT_ROWS.filter(([k]) => st[k] !== undefined).map(([k, label, d]) => [label, d ? fx(st[k], d) : String(st[k])]),
    clipped: Number(st.clipped_samples) > 0,
    files: Object.entries(rec.files || {}).filter(([, v]) => v).map(([k, v]) => [k, String(v)]),
    samplesSha: rec.artefacts && rec.artefacts.samples_sha256 ? String(rec.artefacts.samples_sha256) : "",
    hasRecording: !!(rec.files && rec.files.samples),
    preview: { columns: idx.map((i) => cols[i]), rows: (pv.rows || []).map((r) => idx.map((i) => (r[i] === undefined ? "" : String(r[i])))), total: Number(pv.total) || 0 },
  };
}

/** The receiver report (kshana-eloran-rx) as tables: per station, the fix, time per reference. */
export function rxModel(rep) {
  if (!rep || rep.format !== "kshana-eloran-rx") throw new Error("not a receiver report");
  const tc = rep.truth_comparison || null;
  const stations = (tc ? tc.stations : []).map((s) => {
    const rms = num(s.toa_rms_ns), cyc = Number(s.cycle_errors) || 0;
    return { key: `${s.chain}/${s.station}${s.channel !== null && s.channel !== undefined ? `/${s.channel}` : ""}`, chain: String(s.chain), station: String(s.station), n: Number(s.n) || 0,
      toaRms: fx(rms), toaMean: fx(s.toa_mean_ns), toaMax: fx(s.toa_max_abs_ns), ecdRms: fx(s.ecd_rms_err_us, 3), cycleErrors: cyc,
      tone: cyc > 0 ? "bad" : rms > 30 ? "warn" : "ok" };
  });
  const p = rep.position_solution || {};
  const perr = tc ? num(tc.position_error_m) : NaN;
  const fix = p.error !== undefined
    ? { ok: false, text: `No position fix: ${p.error}`, tone: "bad" }
    : { ok: true, lat: num(p.lat_deg), lon: num(p.lon_deg), hdop: fx(p.hdop), converged: !!p.converged, errorM: Number.isFinite(perr) ? fx(perr) : "", tone: !p.converged ? "warn" : Number.isFinite(perr) && perr > 100 ? "warn" : "ok",
      text: `${latLonText(num(p.lat_deg), num(p.lon_deg), 6)} · HDOP ${fx(p.hdop)}${Number.isFinite(perr) ? ` · error ${fx(perr)} m` : ""}${p.converged ? "" : " · not converged"}` };
  return {
    runId: String(rep.run_id || ""), tracked: Number(rep.stations_tracked) || 0, measurements: Number(rep.measurements) || 0, label: String(rep.evidence_label || ""),
    hasTruth: !!tc, stations, fix,
    biases: Array.isArray(p.clock_bias_ns) ? p.clock_bias_ns.map((b) => (Array.isArray(b) ? [String(b[0]), fx(b[1])] : ["", fx(b)])) : [],
    byRef: (rep.time_by_reference || []).map((r) => ({ ref: String(r.time_ref), offset: fx(r.ref_minus_rx_ns), stations: Number(r.stations) || 0 })),
    perStation: (rep.time_solution || []).map((s) => ({ key: `${s.chain}/${s.station}`, ref: String(s.time_ref), offset: fx(s.ref_minus_rx_ns), std: fx(s.std_ns), n: Number(s.n) || 0 })),
    cycleErrors: stations.reduce((n, s) => n + s.cycleErrors, 0),
  };
}

/** The state of the eLoran page. */
export function createEloran() {
  return { text: "", validation: null, gri: null, filter: { region: "", status: "", search: "" }, sel: "", run: null, rx: null, rxOpts: { per_loop: false, cri_blanking: true }, busy: "" };
}

// ------------------------------------------------------------------ the view (DOM)
const SVGNS = "http://www.w3.org/2000/svg";
function svg(tag, attrs = {}, ...kids) {
  const n = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined && v !== null) n.setAttribute(k, String(v));
  for (const c of kids) if (c) n.append(c.nodeType ? c : document.createTextNode(String(c)));
  return n;
}
const toneVar = (t) => ({ ok: "var(--itg)", warn: "var(--nav)", bad: "var(--int)" })[t] || "var(--ink-3)";

/** The chain map: every listed chain's stations; the selected chain's larger and named. */
export function chainMap(rows, { w = 640, h = 320, selected, onSelect } = {}) {
  const sel = rows.find((r) => r.id === selected);
  const pts = (sel ? sel.stations : rows.flatMap((r) => r.stations)).map((s) => s.pos).filter(Boolean);
  const v = fitView(pts, w, h, { pad: 28, minSpanM: 200000 });
  const root = svg("svg", { viewBox: `0 0 ${w} ${h}`, class: "pro-map", role: "img", "aria-label": sel ? `Map of chain ${sel.name}` : `Map of ${rows.length} chains` });
  root.append(svg("rect", { x: 0, y: 0, width: w, height: h, class: "pro-map-ground" }));
  if (!v) {
    root.append(svg("text", { x: w / 2, y: h / 2, "text-anchor": "middle", class: "pro-map-note" }, "No station of these chains has a position in the database."));
    return root;
  }
  for (const r of rows) {
    const on = r.id === selected;
    for (const s of r.stations) {
      if (!s.pos) continue;
      const [x, y] = v.project(s.pos.lon, s.pos.lat);
      if (x < -20 || y < -20 || x > w + 20 || y > h + 20) continue;
      const g = svg("g", { class: `pro-pin ${on ? "sel" : ""}`, "aria-label": `${r.name} ${s.name}` },
        svg("circle", { cx: x, cy: y, r: on ? 6 : 3, fill: toneVar(r.statusTone), stroke: on ? "var(--ink)" : "none", "stroke-width": 1.5 }),
        on ? svg("text", { x: x + 9, y: y + 4, class: "pro-pin-label" }, `${s.name}${s.role === "master" ? " (M)" : ""}`) : null);
      if (onSelect) g.addEventListener("click", () => onSelect(r.id));
      root.append(g);
    }
  }
  root.append(svg("line", { x1: 16, y1: h - 18, x2: 16 + v.scale.px, y2: h - 18, class: "pro-scale" }), svg("text", { x: 16, y: h - 24, class: "pro-map-note" }, v.scale.label));
  root.append(svg("text", { x: w - 12, y: h - 12, "text-anchor": "end", class: "pro-map-note" }, PROJECTION));
  return root;
}

/** A table of { columns, rows } (text cells). */
export function textTable(h, columns, rows, { mono = true } = {}) {
  return h("div", { class: "wrap" }, h("table", { class: "pro-table" },
    h("thead", {}, h("tr", {}, ...columns.map((c) => h("th", { text: c })))),
    h("tbody", {}, ...rows.map((r) => h("tr", {}, ...r.map((c) => h("td", { class: mono ? "mono" : null, text: c })))))));
}

const sleep = (ms) => new Promise((res) => setTimeout(res, ms));

/**
 * The actions of the page over the app's service calls (proApi). `E` is createEloran();
 * `say(tone, text)` shows a message; `redraw()` draws again; `wait(ms)` paces the job poll.
 */
export function eloranActions(papi, E, { say, redraw, wait = sleep, pollLimit = 900 }) {
  const act = async (busy, fn) => { E.busy = busy; redraw(); try { await fn(); } finally { E.busy = ""; redraw(); } };
  return {
    redraw,
    example: async (force = false) => {
      const r = await papi.get("/eloran/example");
      if (r.ok && typeof r.value === "string") { if (force || !E.text.trim()) { E.text = r.value; E.validation = null; } }
      else say("bad", r.error || "The service gave no example scenario.");
      redraw();
    },
    gri: async () => {
      const r = await papi.get("/eloran/gri");
      if (r.ok && r.value && Array.isArray(r.value.chains)) E.gri = r.value; else say("bad", r.error || "The service gave no GRI database.");
      redraw();
    },
    validate: () => act("validate", async () => { E.validation = validateModel(await papi.post("/eloran/validate", E.text)); }),
    insert: (id) => act("insert", async () => {
      E.sel = id;
      const r = await papi.get(`/eloran/gri/${encodeURIComponent(id)}`);
      const c = r.ok && r.value && Array.isArray(r.value.chains) ? r.value.chains[0] : null;
      if (!c) return say("bad", r.error || `No entry ${id}.`);
      if (!c.snippet) return say("bad", `${id} cannot be added to a scenario: ${c.snippet_error || "the database entry is incomplete"}.`);
      const ins = insertSnippet(E.text, c.snippet);
      if (ins.duplicate) return say("warn", `${c.summary.name} is already in the scenario.`);
      E.text = ins.text;
      E.validation = null;
      say("ok", `Added ${c.summary.name} (GRI ${c.summary.gri}) from the GRI database${ins.timeRefs.length ? `, with the time reference${ins.timeRefs.length > 1 ? "s" : ""} ${ins.timeRefs.join(", ")} at a zero offset (edit it)` : ""}. Validate the scenario before running it.`);
    }),
    run: (format = "") => act("run", async () => {
      E.rx = null;
      const r = await papi.post(`/eloran/run${format ? `?format=${encodeURIComponent(format)}` : ""}`, E.text);
      if (!r.ok) return say("bad", r.error);
      const m = runModel(r.value);
      E.run = r.value;
      if (!m || !m.queued) return say("ok", `Run ${m ? m.runId.slice(0, 12) : ""} finished.`);
      say("ok", `Queued as job ${m.runId.slice(0, 12)}; waiting for it.`);
      for (let i = 0; i < pollLimit; i++) {
        const s = await papi.get(`/jobs/${m.runId}`);
        if (!s.ok) return say("bad", s.error);
        if (s.value.state === "failed") return say("bad", `The run failed: ${s.value.error || "no reason given"}`);
        if (s.value.state === "succeeded") {
          const res = await papi.get(`/jobs/${m.runId}/result`);
          if (!res.ok) return say("bad", res.error);
          E.run = { ...res.value, run_id: m.runId };
          return say("ok", `Job ${m.runId.slice(0, 12)} finished.`);
        }
        E.run = { ...r.value, state: s.value.state };
        redraw();
        await wait(1000);
      }
      say("warn", "The job is still running; look at the service's job board, or run again later (the same scenario is the same job).");
    }),
    rx: () => act("rx", async () => {
      const id = E.run && (E.run.run_id || E.run.job_id);
      if (!id) return say("bad", "Run the scenario first.");
      const r = await papi.post("/eloran/rx", { run: id, per_loop: !!E.rxOpts.per_loop, cri_blanking: E.rxOpts.cri_blanking !== false });
      if (!r.ok) return say("bad", r.error);
      try { E.rx = rxModel(r.value); say("ok", `Receiver: ${E.rx.tracked} station(s) tracked, ${E.rx.measurements} measurements.`); } catch (e) { say("bad", String(e.message || e)); }
    }),
  };
}

/** The eLoran page. `env`: { h, page, btn, chip, note (a node or null), E, A (eloranActions) }. */
export function eloranPage({ h, page, btn, chip, note, E, A }) {
  const busy = (k) => E.busy === k;
  // (a) the scenario editor
  const v = E.validation;
  const editor = h("div", { class: "card" },
    h("div", { class: "lic-head" }, h("h2", { text: "Scenario" }), v ? chip(v.ok ? "VALID" : "INVALID", v.tone) : chip("NOT CHECKED", "none")),
    h("label", { for: "pro-eloran-text", class: "mut", text: "Scenario (TOML)" }),
    h("textarea", { id: "pro-eloran-text", class: "pro-editor", rows: "22", spellcheck: "false", oninput: (e) => { E.text = e.target.value; E.validation = null; } }, E.text),
    h("div", { class: "row" }, btn(busy("validate") ? "Validating…" : "Validate", A.validate, true), btn("Load the example", () => A.example(true))),
    v ? h("p", { class: `lic-result ${v.tone}`, role: "status" }, h("span", { class: "mono", text: v.text }), v.detail ? h("span", { class: "mut", text: ` ${v.detail}` }) : null) : null);

  // (b) the chain map and table
  const rows = griRows(E.gri, E.filter);
  const facets = griFacets(E.gri);
  const pick = (id, label, key, options) => h("label", { class: "row" }, h("span", { class: "mut", text: label }),
    h("select", { id, class: "pro-select", onchange: (e) => { E.filter[key] = e.target.value; A.redraw(); } },
      h("option", { value: "", text: "All" }), ...options.map((o) => h("option", { value: o, text: o, selected: E.filter[key] === o || null }))));
  const chains = h("div", { class: "card" },
    h("div", { class: "lic-head" }, h("h2", { text: "Chains from the GRI database" }), h("span", { class: "mut", text: E.gri ? `${rows.length} of ${E.gri.count} · updated ${E.gri.updated || "?"}` : "reading…" })),
    h("div", { class: "row" }, pick("pro-eloran-region", "Region", "region", facets.regions), pick("pro-eloran-status", "Status", "status", facets.statuses),
      h("input", { class: "pro-input", placeholder: "Search: name, GRI, country, station", value: E.filter.search, onchange: (e) => { E.filter.search = e.target.value; A.redraw(); } })),
    chainMap(rows, { selected: E.sel, onSelect: (id) => { E.sel = id; A.redraw(); } }),
    h("div", { class: "wrap pro-scroll" }, h("table", { class: "pro-table" },
      h("thead", {}, h("tr", {}, ...["GRI", "Chain", "Region", "Status", "Confidence", "Stations", "Operated", ""].map((t) => h("th", { text: t })))),
      h("tbody", {}, ...rows.map((r) => h("tr", { class: r.id === E.sel ? "sel" : null, onclick: () => { E.sel = r.id; A.redraw(); } },
        h("td", { class: "mono", text: String(r.gri) }),
        h("td", {}, h("b", { text: r.name }), h("div", { class: "mono mut", text: r.id })),
        h("td", { text: r.region }),
        h("td", {}, chip(r.status.toUpperCase().replace("_", " "), r.statusTone)),
        h("td", {}, chip(r.confidence.toUpperCase(), r.confidenceTone)),
        h("td", { class: "mono", text: `${r.stations.length}${r.complete ? "" : " (incomplete)"}` }),
        h("td", { class: "mono", text: r.era || "—" }),
        h("td", {}, btn("Insert", (e) => { if (e && e.stopPropagation) e.stopPropagation(); A.insert(r.id); }))))))),
    h("p", { class: "mut", text: "Insert adds the chain's [[chains]] entry, with its sources and confidence as comments, to the end of the scenario. Historic geometry is copied unchanged from the database." }));

  // (c) run controls and statistics
  const m = runModel(E.run);
  const fmt = h("select", { id: "pro-eloran-format", class: "pro-select", "aria-label": "Sample format", onchange: (e) => { E.format = e.target.value; } },
    ...[["", "Scenario's format"], ["ri16", "ri16 (16-bit)"], ["rf32", "rf32 (float)"], ["none", "Truth only"]].map(([val, t]) => h("option", { value: val, text: t, selected: (E.format || "") === val || null })));
  const run = h("div", { class: "card" },
    h("div", { class: "lic-head" }, h("h2", { text: "Run" }), m ? chip(m.queued ? String(m.state || "queued").toUpperCase() : (m.label || "DONE"), m.queued ? "warn" : "ok") : null),
    h("div", { class: "row" }, btn(busy("run") ? "Running…" : "Run the scenario", () => A.run(E.format || ""), true), fmt),
    h("p", { class: "mut", text: "Writes the SigMF recording, the truth table and the run record on the service. Hardware output stays on the command line (kshana-pro eloran stream), into cables or antenna simulators only." }),
    m && !m.queued ? [
      h("dl", { class: "lic-rows" }, h("dt", { text: "Run" }), h("dd", { class: "mono", text: m.runId }), h("dt", { text: "Scenario SHA-256" }), h("dd", { class: "mono", text: m.hash }),
        ...m.stats.flatMap(([k, val]) => [h("dt", { text: k }), h("dd", { class: "mono", text: val })]),
        ...m.files.flatMap(([k, val]) => [h("dt", { text: `File · ${k}` }), h("dd", { class: "mono", text: val })]),
        m.samplesSha ? [h("dt", { text: "Recording SHA-256" }), h("dd", { class: "mono", text: m.samplesSha })] : null),
      m.clipped ? h("p", { class: "lic-warning warn", text: "Some samples clipped: lower output_scale_uv_m or the interference level." }) : null,
      h("h3", { text: `Truth (first ${m.preview.rows.length} of ${m.preview.total} rows)` }),
      textTable(h, m.preview.columns, m.preview.rows)] : m ? h("p", { class: "mut", text: `Job ${m.runId} is ${m.state}.` }) : h("p", { class: "empty", text: "No run yet." }));

  // (d) receiver results
  const R = E.rx;
  const check = (key, label) => h("label", { class: "row" }, h("input", { type: "checkbox", checked: (key === "cri_blanking" ? E.rxOpts[key] !== false : !!E.rxOpts[key]) || null, onchange: (e) => { E.rxOpts[key] = !!e.target.checked; } }), label);
  const canRx = !!(m && !m.queued && m.hasRecording);
  const rx = h("div", { class: "card" },
    h("div", { class: "lic-head" }, h("h2", { text: "Receiver" }), R ? chip(R.cycleErrors ? `${R.cycleErrors} CYCLE ERRORS` : "NO CYCLE ERRORS", R.cycleErrors ? "bad" : "ok") : null),
    h("div", { class: "row" }, h("button", { class: canRx ? "btn primary" : "btn", type: "button", disabled: canRx ? null : true, onclick: A.rx }, busy("rx") ? "Receiving…" : "Run the receiver on the recording"), check("per_loop", "Per loop (crossed loops)"), check("cri_blanking", "Cross-rate blanking")),
    m && !m.queued && !m.hasRecording ? h("p", { class: "mut", text: "This run kept no recording (truth only): run with a sample format to use the receiver." }) : null,
    R ? [
      h("p", { class: "mut", text: `${R.tracked} station(s) tracked, ${R.measurements} measurements. ${R.label}` }),
      R.hasTruth ? textTable(h, ["Station", "n", "ToA error RMS (ns)", "Mean (ns)", "Max |error| (ns)", "ECD RMS error (µs)", "Cycle errors"], R.stations.map((s) => [s.key, String(s.n), s.toaRms, s.toaMean, s.toaMax, s.ecdRms, String(s.cycleErrors)])) : h("p", { class: "mut", text: "No truth comparison in this report." }),
      h("h3", { text: "Position" }), h("p", { class: `lic-result ${R.fix.tone}`, text: R.fix.text }),
      R.biases.length ? textTable(h, ["Clock bias vs", "ns"], R.biases) : null,
      h("h3", { text: "Time offsets per reference (reference − receiver)" }),
      textTable(h, ["Reference", "Offset (ns)", "Stations"], R.byRef.map((r) => [r.ref, r.offset, String(r.stations)])),
      R.perStation.length ? textTable(h, ["Station", "Reference", "Offset (ns)", "Std (ns)", "n"], R.perStation.map((s) => [s.key, s.ref, s.offset, s.std, String(s.n)])) : null] : h("p", { class: "empty", text: "No receiver results yet." }));

  return page("pro-eloran", "Kshana Enterprise · eLoran/GNSS simulator", "eLoran simulator",
    "Write a scenario, add real chains from the GRI database, run it on your Kshana Pro service and read the recording back with the software receiver against the simulator's truth. Every result is MODELLED.",
    note, h("div", { class: "pro-split pro-eloran" }, h("div", { class: "pro-col" }, editor, run, rx), h("div", { class: "pro-col" }, chains)));
}
