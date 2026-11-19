// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// node --test apps/desktop-pro/pro/desktop/*.test.mjs
// The Pro work models (0.42 U6, U19, U27, U28) and their registration on a fake shell.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { fitView, scaleBar, haversine, merc } from "./geo.mjs";
import { fleetModel, trustOf, posOf, ageText } from "./fleet.mjs";
import { replayModel, trustAt, trustBands, positionAt, frameAt, stepCursor, MAX_GAP_S } from "./replay.mjs";
import { newReview, replayItems, scorecardItems, decide, summary, reviewRecord, availableSections, buildReport, esc } from "./review.mjs";
import { scorecardModel, areaModel, timingModel, trustRunsModel, durText } from "./products.mjs";
import { accountModel, libraryModel, auditModel, auditCsv, wallModel } from "./enterprise.mjs";
import { proApi, auditPath } from "./service.mjs";
import { registerWork, WORK, workBadge, createWork } from "./work.mjs";
import { rxModel, benchModel } from "./eloran.mjs";

const NOW = 1_800_000_000;
const here = dirname(fileURLToPath(import.meta.url));
const SCORECARD = JSON.parse(readFileSync(join(here, "../../../../report-templates/samples/receiver-scorecard-public/receiver-scorecard.json"), "utf8"));

test("geo: Web Mercator fit keeps every point inside, scale bars are round", () => {
  const pts = [{ lat: 59.44, lon: 24.77 }, { lat: 60.17, lon: 24.94 }, { lat: 1000, lon: 0 }];
  const v = fitView(pts, 600, 400, { pad: 20 });
  for (const p of pts.slice(0, 2)) {
    const [x, y] = v.project(p.lon, p.lat);
    assert.ok(x >= 19.9 && x <= 580.1 && y >= 19.9 && y <= 380.1, `${x},${y}`);
  }
  // North is up.
  assert.ok(v.project(24.94, 60.17)[1] < v.project(24.77, 59.44)[1]);
  assert.equal(fitView([], 10, 10), null);
  assert.deepEqual(scaleBar(10, 120), { px: 100, label: "1 km" });
  assert.equal(scaleBar(3, 120).label, "200 m");
  assert.ok(Math.abs(haversine(0, 0, 0, 1) - 111319.5) < 1);
  assert.ok(Math.abs(merc(180, 0)[0] - 20037508.34) < 0.01);
});

test("fleet (U6): states, trust, positions and alarms from GET /fleet/stations; worst first", () => {
  const fleet = {
    limit: 5, active: 3,
    stations: [
      { id: "a", name: "Alpha", tenant: "t", state: "live", last_seen_unix: NOW - 30, health: { trust: 0.95, lat: 59.4, lon: 24.7 } },
      { id: "b", name: "Bravo", tenant: "t", state: "stale", last_seen_unix: NOW - 600, health: { trust: 0.9 } },
      { id: "c", name: "Charlie", tenant: "t", state: "live", last_seen_unix: NOW - 10, health: { trust_state: "UNTRUSTED", lat_deg: 60, lon_deg: 25 } },
      { id: "d", name: "Delta", tenant: "t", state: "registered", health: null },
      { id: "e", name: "Echo", tenant: "t", state: "retired", retired: true, health: { trust: 0.1 } },
    ],
  };
  const m = fleetModel(fleet, NOW);
  assert.deepEqual(m.stations.map((s) => s.id), ["b", "c", "d", "a", "e"]);
  assert.deepEqual(m.counts, { live: 2, stale: 1, registered: 1, retired: 1 });
  assert.equal(m.capacity, "3 of 5 Stations in use");
  assert.equal(m.placed, 2);
  assert.equal(m.alarms, 2, "stale Bravo and untrusted Charlie; a retired Station raises none");
  assert.equal(m.stations.find((s) => s.id === "d").trust.text, "not reported");
  assert.equal(m.stations.find((s) => s.id === "d").ageText, "never reported");
  assert.equal(ageText(600), "10 min ago");
  assert.deepEqual(trustOf({ trust: 0.6 }).tone, "warn");
  assert.equal(trustOf({ trust: 7 }).state, "UNKNOWN", "an out-of-range trust is not drawn as a value");
  assert.equal(posOf({ lat: 91, lon: 0 }), null);
  assert.equal(fleetModel(null, NOW).stations.length, 0);
});

const T0 = 1_778_738_400; // 2026-05-14T06:00:00Z
function replayDoc(ok = true) {
  const pts = (dlat) => Array.from({ length: 120 }, (_, i) => ({ t: T0 + 10 * i, lat: 55.2 + dlat + 0.001 * i, lon: 2.5 + 0.002 * i, flagged: i > 40 && i < 60 }));
  return {
    format: "kshana-pro-incident-replay/1", case_id: "C-1", title: "Synthetic", status: "final",
    verification: { ok, trusted: ok, problems: ok ? [] : ["timeline.csv changed"], signer: "Issuer", key_id: "k1" },
    window: { start: T0, end: T0 + 1200 },
    verdict: { jamming: { status: "indicated", confidence: "medium" }, spoofing: { status: "not-indicated" }, sentence: "Jamming is indicated." },
    tracks: [
      { source: "Bridge GNSS", class: "measured", kind: "estimated", points: pts(0) },
      { source: "AIS feed", class: "reported", kind: "reported", points: pts(0.01).filter((_, i) => i % 3 === 0) },
    ],
    episodes: [
      { id: "F1", category: "jamming", confidence: "medium", source: "Bridge GNSS", start: T0 + 300, end: T0 + 720, statement: "C/N0 fell on every satellite" },
      { id: "F2", category: "degradation-not-attributed", confidence: "low", source: "Bridge GNSS", start: T0 + 900, end: T0 + 960, statement: "" },
    ],
    area: { bbox: [55.2, 55.4, 2.5, 2.8] }, limits: ["V2 not met: no detection-rate claim."], label: "MODELLED",
  };
}

test("replay (U19): reported vs estimated on one cursor, trust shading from the findings", () => {
  const m = replayModel(replayDoc());
  assert.equal(m.tracks.length, 2);
  assert.deepEqual(m.tracks.map((t) => t.kind), ["estimated", "reported"]);
  assert.equal(trustAt(m, T0 + 100).level, "trusted");
  assert.equal(trustAt(m, T0 + 400).level, "untrusted");
  assert.equal(trustAt(m, T0 + 930).level, "degraded");
  const bands = trustBands(m, 120);
  assert.deepEqual(bands.map((b) => b.level), ["trusted", "untrusted", "trusted", "degraded", "trusted"]);
  assert.equal(bands[0].from, m.t0);
  assert.equal(bands.at(-1).to, m.t1);
  // The last fix at or before t; none before the first, none across a gap.
  const est = m.tracks[0];
  assert.equal(positionAt(est, T0 - 1), null);
  assert.equal(positionAt(est, T0 + 15).t, T0 + 10);
  assert.equal(positionAt(est, T0 + 1190 + MAX_GAP_S + 1), null);
  const f = frameAt(m, T0 + 300);
  assert.ok(f.gap > 1000 && f.gap < 1200, `${f.gap} m: 0.01 degree of latitude`);
  assert.equal(stepCursor(m, T0, "j"), T0);
  assert.equal(stepCursor(m, T0, "l"), T0 + 10);
  assert.equal(stepCursor(m, T0, "End"), T0 + 1200);
  const bad = replayModel(replayDoc(false));
  assert.equal(bad.verified, false);
  assert.deepEqual(bad.problems, ["timeline.csv changed"]);
  assert.throws(() => replayModel({ format: "x" }));
});

test("review (U27): every finding decided, disputes need a note, the record is signed only when complete", () => {
  const m = replayModel(replayDoc());
  let r = newReview({ kind: "incident-forensics", id: m.caseId, title: m.title }, replayItems(m));
  assert.equal(summary(r).pending, 2);
  assert.throws(() => reviewRecord(r, { reviewer: "Ana", at: NOW }), /pending/);
  assert.throws(() => decide(r, "F1", "disputed", " "), /note/);
  assert.throws(() => decide(r, "nope", "accepted"));
  r = decide(r, "F1", "accepted");
  r = decide(r, "F2", "disputed", "Fjord shadowing, as the master said");
  assert.deepEqual(summary(r), { total: 2, pending: 0, accepted: 1, disputed: 1, complete: true });
  assert.throws(() => reviewRecord(r, { reviewer: "", at: NOW }));
  const rec = reviewRecord(r, { reviewer: "Ana", at: NOW });
  assert.equal(rec.format, "kshana-pro-desktop-review/1");
  assert.match(rec.note, /not the claim-grade expert review/);
  const sc = scorecardItems(SCORECARD);
  assert.equal(sc.length, SCORECARD.totals.events);
});

test("report builder (U27): sections from what is open, escaped, labels and limits carried", () => {
  const m = replayModel(replayDoc());
  m.title = "<script>x</script>";
  const card = scorecardModel(SCORECARD);
  let rv = newReview({ kind: "incident-forensics", id: "C-1" }, replayItems(m));
  rv = decide(decide(rv, "F1", "accepted"), "F2", "accepted");
  const src = { replay: m, review: rv, scorecard: card, area: null };
  assert.deepEqual(availableSections(src).map((s) => s.id), ["verdict", "verification", "scorecard", "review", "limits"]);
  assert.deepEqual(availableSections({}), []);
  assert.throws(() => buildReport(src, [], { title: "T", at: NOW }));
  assert.throws(() => buildReport(src, ["area"], { title: "T", at: NOW }), /nothing open/);
  const { html, json } = buildReport(src, ["verdict", "scorecard", "limits"], { title: "Case <C-1> & co", author: "Ana", at: NOW });
  assert.ok(html.includes("Case &lt;C-1&gt; &amp; co") && !html.includes("<C-1>"));
  assert.ok(html.includes("V2 not met"));
  assert.ok(html.includes(esc(card.wording)));
  assert.ok(!/<script/i.test(html));
  const rec = JSON.parse(json);
  assert.deepEqual(Object.keys(rec.sections), ["verdict", "scorecard", "limits"]);
  for (const w of ["compliant", "conformant", "certified"]) assert.ok(!html.toLowerCase().includes(w), w);
});

test("products: the scorecard, area trust (no emitter positions), timing fleet, multi-source trust", () => {
  const c = scorecardModel(SCORECARD);
  assert.equal(c.levels[0].tone, "ok");
  assert.equal(c.cases.length, 11);
  assert.throws(() => scorecardModel({}));
  const picture = {
    title: "Gulf", label: "MODELLED", positions: { emitter_positions: "withheld", withheld: 2, reason: "F2 gate not met" }, wide_area: [{}],
    areas: [
      { id: "x", name: "Port", current: "TRUSTED", sources: { "st-1": 10, "st-2": 4 }, first_g: 0, last_g: 99, timeline: [[0, "TRUSTED", 50], [50, "DEGRADED", 50]], events: [] },
      { id: "y", name: "Approach", current: "UNTRUSTED", sources: { "rx-9": 3 }, first_g: 0, last_g: 99, timeline: [[0, "UNTRUSTED", 100]], events: [{ id: "e1", worst: "UNTRUSTED", duration_s: 60, basis: "network", corroborated: false, sources: ["rx-9"], detections: [{ lat_deg: 1 }] }] },
    ],
  };
  const fleet = { stations: [{ id: "st-1", health: { lat: 59, lon: 24 } }, { id: "st-2", health: { lat: 61, lon: 26 } }] };
  const a = areaModel(picture, fleet);
  assert.equal(a.worst, "UNTRUSTED");
  assert.equal(a.areas[0].id, "y");
  assert.equal(a.areas[0].pos, null, "an area without placed Stations is listed, not placed");
  assert.deepEqual(a.areas[1].pos, { lat: 60, lon: 25 });
  assert.equal(a.areas[1].timeline[1].from, 0.5);
  assert.match(a.positions, /withheld/);
  assert.ok(!JSON.stringify(a).includes("lat_deg"), "no emitter position reaches the model");
  const t = timingModel({ title: "Fleet", counts: { ok: 1, act: 1 }, order: ["c2", "c1"], clocks: [
    { id: "c1", status: "ok", breaches: [{ name: "1.5 us", t_warn_s: null }] },
    { id: "c2", status: "act", vendor: "V", model: "M", breaches: [{ name: "1.5 us", t_warn_s: 7500 }, { name: "100 ns", t_warn_s: 3000 }] },
  ] });
  assert.deepEqual(t.clocks.map((x) => [x.id, x.tone, x.warnText]), [["c2", "bad", "50 min"], ["c1", "ok", "no limit reached within the horizon"]]);
  assert.equal(durText(7500), "2 h 05 min");
  const tr = trustRunsModel({ format: "kshana-pro-trust-evidence/1", epochs: 100, label: "MODELLED", counts: { trusted: 80, holdover: 15, none: 5, by_source: [["osnma", 80]] },
    switches: [{ t_s: 0, from: "none", to: "osnma", reason: "qualified" }, { t_s: 600, from: "osnma", to: "holdover", reason: "osnma failed" }] });
  assert.deepEqual(tr.runs.map((r) => [r.source, r.state, r.to]), [["osnma", "trusted", 600], ["holdover", "holdover", null]]);
  assert.equal(tr.switches, 1);
  assert.throws(() => trustRunsModel({ format: "x" }));
});

test("enterprise (U28): account and roles, team library, audit log, watch wall", () => {
  assert.equal(accountModel(null, null).signedIn, false);
  const acc = accountModel({ id: "alice", tenant: "site-a", role: "submitter", source: "oidc", edition: "enterprise", enterprise: ["pro-watch-wall"] }, { connected: true, base: "https://k.example" });
  assert.equal(acc.source, "single sign-on");
  assert.deepEqual(acc.can, { submit: true, admin: false, wall: true, audit: false, library: true });
  const lib = libraryModel([{ name: "b", op: "run", saved_utc: "2027-01-01T00:00:00Z", saved_by: "x", request: { scenario: "kind = \"link-budget\"\n" } }, { name: "a", op: "run", saved_utc: "2027-02-01T00:00:00Z" }]);
  assert.deepEqual(lib.map((s) => [s.name, s.kind]), [["a", ""], ["b", "link-budget"]]);
  const ex = { format: "kshana-pro-audit-export/1", count: 2, chain: { verified: true, lines: 9, files: 1 }, lines_sha256: "ab",
    lines: [{ t: 1, who: "a", tenant: "t", role: "admin", source: "key", method: "GET", route: "/jobs", status: 200, ms: 1, sha256: "x" }, { t: 2, who: "b, \"q\"", tenant: "t", role: "reader", source: "oidc", method: "POST", route: "/jobs", status: 403, ms: 1, sha256: "y" }] };
  const am = auditModel(ex);
  assert.equal(am.rows[0].t, 2);
  assert.equal(am.refused, 1);
  assert.ok(am.chainOk);
  assert.equal(auditModel(ex, { who: "a" }).rows.length, 1);
  assert.ok(auditCsv(am).includes('"b, ""q"""'));
  assert.equal(auditModel({ ...ex, chain: { verified: false, first_bad: "2027-01-01.jsonl:3" } }).chainText, "Chain broken at 2027-01-01.jsonl:3.");
  assert.equal(auditPath(NOW, 7), "/audit/export?since=2027-01-09&until=2027-01-15");
  const wm = wallModel({ format: "kshana-pro-watch-wall/1", tenant: "t", at_unix: NOW, alarm: true, jobs: { succeeded: 2 },
    fleet: { stations: [{ name: "A", state: "live" }, { name: "B", state: "stale" }] }, area: { configured: false }, timing: { configured: true, clocks: [{ id: "c", status: "watch" }] } });
  assert.deepEqual(wm.tiles.map((t) => [t.name, t.tone]), [["B", "bad"], ["c", "warn"], ["A", "ok"]]);
  assert.deepEqual(wm.notConfigured, ["area console"]);
});

test("service: the page reaches the service only through the app, and a refusal is an error", async () => {
  const sent = [];
  const invoke = async (cmd, a) => { sent.push([cmd, a]); if (a.args[1] === "/nope") return { status: 403, json: { error: "below admin" } }; return { status: 200, json: { ok: 1 } }; };
  const p = proApi(invoke);
  assert.deepEqual(await p.get("/whoami"), { ok: true, value: { ok: 1 }, status: 200, error: "" });
  const r = await p.get("/nope");
  assert.equal(r.ok, false);
  assert.match(r.error, /403: below admin/);
  await p.post("/library/x/run", { a: 1 });
  assert.deepEqual(sent.at(-1), ["engine_call", { name: "pro_service", args: ["POST", "/library/x/run", "{\"a\":1}"] }]);
  const out = proApi(null);
  assert.equal((await out.replay("/x")).ok, false);
});

test("work views: registered on the shell with editions and sign-in badges; no DOM at registration", async () => {
  const calls = { views: new Map(), sections: [], groups: [], shown: [], refreshes: 0 };
  const shell = {
    version: 1, view: "home",
    registerView: (id, spec) => { calls.views.set(id, spec); return () => calls.views.delete(id); },
    registerNavSection: (s) => { calls.sections.push(s); return () => {}; },
    registerPaletteGroup: (g) => { calls.groups.push(g); return () => {}; },
    show: (id) => calls.shown.push(id), refresh: () => { calls.refreshes++; }, toast: () => {},
  };
  const invoked = [];
  const papi = proApi(async (cmd, a) => { invoked.push(a.name); if (a.name === "pro_service_status") return { connected: true, base: "https://k.example", credential: "sso" }; if (a.name === "pro_service") return { status: 200, json: { id: "alice", tenant: "t", role: "admin", source: "oidc", edition: "enterprise", enterprise: ["pro-watch-wall", "pro-audit-export"] } }; return null; });
  const S = { view: { unlocked: true, edition: "Pro" } };
  const pending = [];
  const { W, off } = registerWork(shell, { h: () => { throw new Error("no DOM at registration"); }, ic: () => null, S, papi, now: () => NOW, later: (fn) => pending.push(fn) });
  assert.deepEqual([...calls.views.keys()], WORK.map((w) => w.id));
  assert.equal(off.length, WORK.length + 2);
  // Before the connection is read: Pro work needs sign-in, Enterprise work needs Enterprise.
  const badges = () => Object.fromEntries(calls.sections[0].items().map((i) => [i.id, i.badge]));
  assert.equal(badges()["pro-replay"], null);
  assert.equal(badges()["pro-fleet"], "SIGN IN");
  assert.equal(badges()["pro-wall"], "ENTERPRISE");
  await Promise.all(pending.map((f) => f()));
  assert.deepEqual(invoked, ["pro_service_status", "pro_service"]);
  assert.equal(W.me.id, "alice");
  assert.equal(badges()["pro-fleet"], null);
  S.view = { unlocked: true, edition: "Enterprise" };
  assert.equal(badges()["pro-wall"], null);
  S.view = { unlocked: false, edition: "Evaluation" };
  assert.equal(badges()["pro-fleet"], "PRO");
  assert.equal(workBadge(WORK[0], { unlocked: true, edition: "Pro" }, createWork()), "SIGN IN");
  const pal = calls.groups[0].items();
  assert.ok(pal.some((i) => i.id === "pro-wall" && i.tag === "ENTERPRISE"));
  pal.find((i) => i.id === "pro-replay").onSelect();
  assert.deepEqual(calls.shown, ["pro-replay"]);
});

// ------------------------------------------------------------------ render smoke test
// A minimal document (elements with attributes, children, text and listeners) so every work
// view's render path runs in node: a typo in a render would throw here.
function fakeDocument() {
  class El {
    constructor(tag) { this.tag = tag; this.attrs = {}; this.children = []; this.listeners = {}; this.style = {}; this._text = ""; this.className = ""; this.nodeType = 1; this.dataset = {}; }
    setAttribute(k, v) { this.attrs[k] = String(v); }
    getAttribute(k) { return this.attrs[k]; }
    append(...c) { for (const x of c) this.children.push(typeof x === "string" ? { nodeType: 3, text: x } : x); }
    replaceChildren(...c) { this.children = []; this.append(...c); }
    addEventListener(e, f) { this.listeners[e] = f; }
    set textContent(t) { this._text = String(t); this.children = []; }
    get textContent() { return this._text + this.children.map((c) => (c.nodeType === 3 ? c.text : c.textContent)).join(""); }
    getBoundingClientRect() { return { left: 0, width: 100 }; }
    setPointerCapture() {}
  }
  return { createElement: (t) => new El(t), createElementNS: (_, t) => new El(t), createTextNode: (t) => ({ nodeType: 3, text: String(t) }), getElementById: () => null };
}

test("work views: every view renders, signed in or not, with data or without", async () => {
  globalThis.document = fakeDocument();
  const { h, ic } = await import("./ui.mjs");
  const views = new Map();
  const shell = { version: 1, view: "x", registerView: (id, s) => { views.set(id, s); return () => {}; }, registerNavSection: () => () => {}, registerPaletteGroup: () => () => {}, show: () => {}, refresh: () => {}, toast: () => {} };
  const answers = {
    "/whoami": { id: "alice", tenant: "t", role: "admin", source: "oidc", edition: "enterprise", enterprise: ["pro-watch-wall", "pro-audit-export"] },
    "/fleet/stations": { limit: 3, active: 1, stations: [{ id: "a", name: "A", state: "live", last_seen_unix: NOW, health: { trust: 0.9, lat: 59, lon: 24 } }] },
    "/area/picture": { title: "Area", positions: { emitter_positions: "withheld" }, areas: [{ id: "x", name: "X", current: "DEGRADED", sources: { a: 1 }, first_g: 0, last_g: 9, timeline: [[0, "DEGRADED", 10]], events: [] }] },
    "/timing/assessment": { title: "Fleet", counts: { ok: 1 }, clocks: [{ id: "c", status: "ok", breaches: [] }] },
    "/library": [{ name: "s", op: "run", saved_utc: "2027-01-01T00:00:00Z", request: {} }],
    "/wall/state": { format: "kshana-pro-watch-wall/1", tenant: "t", at_unix: NOW, alarm: false, jobs: {}, fleet: { stations: [] }, area: { configured: false }, timing: { configured: false } },
  };
  const papi = proApi(async (cmd, a) => {
    if (a.name === "pro_service_status") return { connected: true, base: "https://k" };
    if (a.name === "pro_incident_replay") return replayDoc();
    if (a.name === "pro_service") {
      const p = a.args[1];
      if (p.startsWith("/audit/export")) return { status: 200, json: { format: "kshana-pro-audit-export/1", chain: { verified: true }, lines: [{ t: 1, who: "a", tenant: "t", role: "admin", source: "key", method: "GET", route: "/", status: 200, ms: 1, sha256: "x" }] } };
      if (p === "/eloran/example") return { status: 200, text: "[scene]\nduration_s = 1.0\n" };
      if (p === "/eloran/gri") return { status: 200, json: { count: 1, updated: "2026-10-01", chains: [{ id: "uk-6731", gri: 6731, name: "Anthorn", region: "Europe", status: "operational", confidence: "verified", complete: true, stations: [{ id: "M", name: "Anthorn", role: "master", lat_deg: 54.9, lon_deg: -3.3 }] }] } };
      return { status: 200, json: answers[p] };
    }
    return null;
  });
  const S = { view: { unlocked: true, edition: "Enterprise" } };
  const pending = [];
  const { W } = registerWork(shell, { h, ic, S, papi, now: () => NOW, later: (fn) => pending.push(fn) });
  const ctx = { runs: [{ title: "card", result: JSON.stringify(SCORECARD) }] };
  const renderAll = () => { for (const [id, v] of views) { const el = document.createElement("div"); v.render(el, ctx); assert.ok(el.children.length, id); } };
  renderAll(); // before anything is read
  while (pending.length) await pending.shift()();
  renderAll(); // signed in: each service view starts its read
  while (pending.length) await pending.shift()();
  renderAll(); // with the service's answers
  for (const k of ["fleet", "area", "timing", "library", "wall", "audit"]) assert.ok(W[k], `${k}: ${JSON.stringify(W.msg[k])}`);
  assert.equal(W.eloran.gri.count, 1, JSON.stringify(W.msg.eloran));
  assert.match(W.eloran.text, /\[scene\]/);
  W.eloran.sel = "uk-6731";
  W.eloran.validation = { ok: false, tone: "bad", text: "scenario: bad", detail: "" };
  W.eloran.run = { format: "kshana-eloran-run", run_id: "a".repeat(64), scenario_sha256: "b".repeat(64), stats: { pulses: 1, clipped_samples: 2 }, files: { samples: "/r.sigmf-data" }, truth_preview: { columns: ["chain", "toa_s"], rows: [["A", "0.1"]], total: 1 } };
  W.eloran.rx = rxModel({ format: "kshana-eloran-rx", stations_tracked: 1, measurements: 2, position_solution: { error: "too few" }, time_by_reference: [{ time_ref: "UTC(A)", ref_minus_rx_ns: 1, stations: 1 }], truth_comparison: { stations: [{ chain: "A", station: "M", n: 2, toa_rms_ns: 1, cycle_errors: 0 }], position_error_m: null } });
  W.eloran.monitor = "<!doctype html><title>m</title>";
  W.eloran.bench.result = benchModel({
    assessment: { ready: true, counts: { pass: 1 }, findings: [{ role: "eloran_source", device: "rp", check: "band", status: "pass", detail: "ok" }], runnable: ["pps_jitter"], not_runnable: [["dynamic_range", "needs a spectrum analyser"]] },
    wiring_md: "| 1 | a | b | coax | n |",
    twins: { rp: { levels: [{ level_dbfs: 0, vpeak_50ohm: 1, delay_bias_ns: 0, ecd_us: 0, snr_db: 80 }] }, rf: { error: "cannot output 90–110 kHz" } },
  });
  {
    const el = document.createElement("div");
    views.get("pro-eloran").render(el, ctx);
    const t = el.textContent;
    assert.ok(!t.includes("[object"), "a node was stringified into the eLoran page");
    assert.ok(t.includes("Spec twin: rp") && t.includes("80.0 dB") && t.includes("cannot output 90–110 kHz"), "bench card renders the twins");
  }
  W.packDir = "/packs/c1";
  W.replay = replayModel(replayDoc());
  W.cursor = T0 + 400;
  W.review = newReview({ kind: "incident-forensics", id: "C-1" }, replayItems(W.replay));
  W.report.sections = ["verdict"];
  renderAll(); // with a replay, a review and a scorecard open
  S.view = { unlocked: false, edition: "Evaluation" };
  renderAll(); // locked
  if (W.wallTimer) clearInterval(W.wallTimer);
  delete globalThis.document;
});
