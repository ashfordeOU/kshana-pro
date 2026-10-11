// SPDX-License-Identifier: AGPL-3.0-only
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { createGeometryAdapter, chooseBackend, systemsFor, buildInterimToml, parseInterimNmea, buildGeometryRequest, parseGeometryResult, INTERIM_SKYLINES, SKYLINE_LABELS, GEOMETRY_FNS } from "./geometry-adapter.mjs";
import { DEFAULTS, SKYLINES, parseState } from "./geomstate.mjs";
import { normalise } from "./skyline.mjs";

const fake = (fns, impl = {}) => ({ available: true, version: "9.9.9", has: (f) => fns.includes(f), call: async (f, ...a) => impl[f](...a) });
const state = (q = "") => parseState(q).state;

test("the backend is chosen from what the engine has", () => {
  assert.equal(chooseBackend(null), "none");
  assert.equal(chooseBackend({ available: false, has: () => true }), "none");
  assert.equal(chooseBackend(fake(["nmea_training"])), "interim");
  assert.equal(chooseBackend(fake(["nmea_training", ...GEOMETRY_FNS])), "geometry");
  assert.equal(chooseBackend(fake(["geometry"])), "none", "both 0.36 functions are needed");
  assert.deepEqual(systemsFor("interim").filter((s) => !s.available).map((s) => s.id), ["leo"]);
  assert.ok(systemsFor("geometry").every((s) => s.available));
  assert.ok(systemsFor("none").every((s) => !s.available && s.why));
});

test("every named skyline has a label and an outline that is already normal", () => {
  for (const s of SKYLINES) assert.ok(SKYLINE_LABELS[s], s);
  for (const [name, pts] of Object.entries(INTERIM_SKYLINES)) assert.deepEqual(normalise(pts), pts, name);
});

test("the interim scenario carries numbers and fixed names only", () => {
  const t = buildInterimToml(state("?place=custom&lat=89.9&lon=-12.5&mask=45&sys=gps,leo,glonass&seed=77"));
  assert.match(t, /lat_deg = 89\n/);
  assert.match(t, /lon_deg = -12.5\n/);
  assert.match(t, /systems = \["gps", "glonass"\]/);
  assert.match(t, /seed = 77\n/);
  assert.throws(() => buildInterimToml({ ...DEFAULTS, sys: ["gps"], t: '2026"\n[x]' }));
  assert.throws(() => buildInterimToml({ ...DEFAULTS, sys: ["gps"], lon: NaN }));
});

const NMEA = [
  "$PKSHT,TRAINING,SYNTHETIC,NOT-FOR-NAVIGATION*3F",
  "$GNGSA,A,3,01,08,,,,,,,,,,,1.8,1.0,1.4,1*00",
  "$GNGSA,A,3,24,,,,,,,,,,,,1.8,1.0,1.4,3*00",
  "$GPGSV,1,1,03,01,83,154,46,08,36,185,42,14,10,000,36,1*00",
  "$GAGSV,1,1,02,24,53,073,43,09,75,227,45,7*00",
  "$GAGSV,1,1,01,99,91,000,43,7*00",
  "$XXGSV,1,1,01,05,10,010,30,1*00",
  "$GPGSV,1,1,01,01,<b>,154,46,1*00",
  "$GNGSA,A,3,14,,,,,,,,,,,,9.9,9.9,9.9,1*00",
  "$GPGSV,1,1,01,02,10,010,30,1*00",
].join("\n");

test("the interim reader takes the first epoch's GSV and GSA only, strictly", () => {
  const r = parseInterimNmea(NMEA);
  assert.deepEqual(r.dop, { gdop: null, pdop: 1.8, hdop: 1, vdop: 1.4, tdop: null });
  assert.deepEqual(r.sats.map((s) => `${s.sys}${s.id}:${s.az}/${s.el}:${s.used ? "u" : "-"}`), ["gps1:154/83:u", "gps8:185/36:u", "gps14:0/10:-", "galileo9:227/75:-", "galileo24:73/53:u"]);
  assert.equal(r.nUsed, 3);
  assert.deepEqual(parseInterimNmea("garbage\n$GPGSV,not,a*ZZ").sats, []);
});

test("the interim compute says what it cannot show", async () => {
  let got;
  const eng = fake(["nmea_training"], { nmea_training: (toml, seed) => { got = { toml, seed }; return JSON.stringify({ nmea: NMEA, log_json: "{}" }); } });
  const a = createGeometryAdapter(eng);
  const r = await a.compute(state("?sys=gps,galileo,leo&sky=fjord&seed=5"));
  assert.equal(got.seed, 5);
  assert.equal(r.backend, "interim");
  assert.equal(r.ellipse, null);
  assert.equal(r.cloud, null);
  assert.ok(r.missing.includes("GDOP") && r.missing.includes("the Monte Carlo cloud"));
  assert.ok(r.notes.some((n) => /LEO/.test(n)) && r.notes.some((n) => /skyline/.test(n)));
  assert.equal((await a.compute(state("?sys=leo"))).sats.length, 0);
  const bad = createGeometryAdapter(fake(["nmea_training"], { nmea_training: () => "not json" }));
  await assert.rejects(() => bad.compute(state()), /not JSON/);
  const sky = await a.skyline("fjord");
  assert.equal(sky.source, "illustrative outline");
  assert.deepEqual((await a.skyline("custom")).points, []);
  assert.deepEqual((await a.skyline("__proto__")).points, []);
});

test("no engine: an empty result that says why", async () => {
  const r = await createGeometryAdapter(null).compute(state());
  assert.equal(r.backend, "none");
  assert.match(r.notes[0], /not running/);
});

test("the 0.36 request is built from the state, and the reply is checked field by field", async () => {
  const s = state("?sky=custom&pts=0:10,90:20&sigma=2&seed=9&n=500&sys=gps,leo");
  const req = buildGeometryRequest(s, s.pts);
  assert.deepEqual(req.systems, ["gps", "leo"]);
  assert.deepEqual(req.skyline, [{ az_deg: 0, el_deg: 10 }, { az_deg: 90, el_deg: 20 }]);
  assert.deepEqual(req.monte_carlo, { seed: 9, samples: 500 });
  const reply = {
    sigma_uere_m: 2,
    satellites: [{ system: "gps", id: 3, az_deg: 370, el_deg: 40, used: true }, { system: "leo", id: "L12", az_deg: 10, el_deg: 15, used: false, blocked_by: "skyline" }, { system: "evil", az_deg: 1, el_deg: 1 }, { system: "gps", az_deg: "x", el_deg: 1 }],
    dop: { gdop: 2.1, pdop: 1.9, hdop: 1.1, vdop: 1.5, tdop: "1" },
    horizontal_ellipse: { semi_major_m: 2.6, semi_minor_m: 1.4, orientation_deg: 33 },
    monte_carlo: { seed: 9, rms_horizontal_m: 2.2, points_enu_m: [[1, 2, 3], [0.5, -1, 0], [1, "x", 0], "no"] },
    notes: ["a", 5],
  };
  const r = parseGeometryResult(reply, "0.36.0");
  assert.deepEqual(r.sats.map((x) => [x.sys, x.id, x.az, x.used, x.blocked]), [["gps", "3", 10, true, null], ["leo", "L12", 10, false, "skyline"]]);
  assert.equal(r.dop.tdop, null);
  assert.deepEqual(r.ellipse, { semiMajorM: 2.6, semiMinorM: 1.4, orientationDeg: 33, sigmaM: 2 });
  assert.equal(r.cloud.samples, 2);
  assert.deepEqual(r.notes, ["a"]);
  assert.throws(() => parseGeometryResult(null));
  const eng = fake(["nmea_training", ...GEOMETRY_FNS], { geometry: (j) => { assert.deepEqual(JSON.parse(j), req); return JSON.stringify(reply); }, skyline_preset: () => JSON.stringify({ points: [{ az_deg: 5, el_deg: 7 }, { az_deg: "x" }] }) });
  const a = createGeometryAdapter(eng);
  assert.equal(a.backend, "geometry");
  assert.equal((await a.compute(s, s.pts)).ellipse.semiMajorM, 2.6);
  assert.deepEqual((await a.skyline("fjord")).points, [{ az: 5, el: 7 }]);
});

// Against the real engine package, when it is checked out (not an LFS pointer).
const PKG = new URL("../pkg/", import.meta.url);
const havePkg = (() => { try { return statSync(new URL("kshana_bg.wasm", PKG)).size > 1024; } catch { return false; } })();
test("the interim backend against the real engine package", { skip: !havePkg && "pkg/kshana_bg.wasm is not checked out" }, async () => {
  const mod = await import(new URL("kshana.js", PKG).href);
  await mod.default({ module_or_path: readFileSync(new URL("kshana_bg.wasm", PKG)) });
  const eng = { available: true, version: mod.version(), has: (f) => typeof mod[f] === "function", call: async (f, ...a) => mod[f](...a) };
  const a = createGeometryAdapter(eng);
  assert.ok(a.backend === "interim" || a.backend === "geometry");
  const r = await a.compute(state("?sys=gps,galileo,beidou&mask=10"), []);
  assert.ok(r.sats.length >= 12, `only ${r.sats.length} satellites`);
  assert.ok(r.sats.every((x) => x.el >= 0 && x.el <= 90 && x.az >= 0 && x.az < 360));
  assert.ok(r.nUsed >= 4 && r.nUsed <= 12);
  assert.ok(r.dop.pdop > 0 && r.dop.hdop > 0 && r.dop.vdop > 0 && r.dop.pdop >= r.dop.hdop);
  const again = await a.compute(state("?sys=gps,galileo,beidou&mask=10"), []);
  assert.deepEqual(again, r, "the same state gives the same answer");
  const later = await a.compute(state("?sys=gps,galileo,beidou&mask=10&t=2026-10-12T15:00:00Z"), []);
  assert.notDeepEqual(later.sats, r.sats, "the sky moves with time");
});
