// SPDX-License-Identifier: AGPL-3.0-only
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { createGeometryAdapter, chooseBackend, featuresFor, buildInterimToml, parseInterimNmea, interimInstant, buildRequest, skylineRequest, parseGeometry, parseMonteCarlo, parsePresets, parseProfile, INTERIM_SKYLINES, SKYLINE_LABELS, GEOMETRY_FNS, INTERIM_EPOCH } from "./geometry-adapter.mjs";
import { parseState, SKYLINE_PRESETS, DEFAULTS } from "./geometry-url.mjs";
import { emptyTable, brush, elAt, isOpen, BINS } from "./skyline-edit.mjs";
import { ENGINE_FNS } from "./engine.mjs";

const fake = (fns, impl = {}) => ({ available: true, version: "9.9.9", has: (f) => fns.includes(f), call: async (f, ...a) => impl[f](...a) });
const state = (q = "") => parseState(q).state;

test("the backend is chosen from what the engine has, and features follow it", () => {
  assert.equal(chooseBackend(null), "none");
  assert.equal(chooseBackend({ available: false, has: () => true }), "none");
  assert.equal(chooseBackend(fake(["nmea_training"])), "interim");
  assert.equal(chooseBackend(fake(["nmea_training", "sky_geometry"])), "geometry");
  const fi = featuresFor("interim", fake(["nmea_training"]));
  assert.equal(fi.systems.leo, false);
  assert.equal(fi.skyline, false);
  assert.equal(fi.monteCarlo, false);
  const fg = featuresFor("geometry", fake(GEOMETRY_FNS));
  assert.ok(fg.systems.leo && fg.skyline && fg.canyon && fg.monteCarlo && fg.svg && fg.tle);
  assert.equal(featuresFor("geometry", fake(["sky_geometry"])).monteCarlo, false, "each optional function is checked on its own");
});

test("the 0.36 names are the design note's, and ENGINE_FNS does not list them before the package has them", () => {
  assert.deepEqual([...GEOMETRY_FNS], ["sky_geometry", "sky_monte_carlo", "skyline_presets", "skyline_profile", "skyplot_svg"]);
  // Remove this assertion in the commit that adds them to ENGINE_FNS with a 0.36 package.
  assert.ok(GEOMETRY_FNS.every((f) => !ENGINE_FNS.includes(f)));
});

test("every preset has a label and an interim outline below 90 degrees", () => {
  for (const s of SKYLINE_PRESETS) {
    assert.ok(SKYLINE_LABELS[s], s);
    const t = parseProfile(INTERIM_SKYLINES[s].map((p) => [p.az, p.el]));
    assert.ok(t && !isOpen(t), s);
  }
});

test("the interim scenario carries numbers and fixed names only, at the reference epoch plus the offset", () => {
  assert.equal(interimInstant(3600), "2026-10-12T01:00:00Z");
  assert.equal(interimInstant(0), INTERIM_EPOCH);
  const t = buildInterimToml(state("?site=89.9,-12.5,0&mask=45&sys=G,C,L:xona-pulsar&seed=77&t=60"));
  assert.match(t, /lat_deg = 89\n/);
  assert.match(t, /lon_deg = -12.5\n/);
  assert.match(t, /systems = \["gps", "beidou"\]/);
  assert.match(t, /seed = 77\n/);
  assert.match(t, /start_utc = "2026-10-12T00:01:00Z"/);
  assert.throws(() => buildInterimToml({ ...DEFAULTS, sys: ["gps"], lon: NaN }));
});

const NMEA = [
  "$PKSHT,TRAINING,SYNTHETIC,NOT-FOR-NAVIGATION*3F",
  "$GNGSA,A,3,01,08,,,,,,,,,,,1.8,1.0,1.4,1*00",
  "$GNGSA,A,3,24,,,,,,,,,,,,1.8,1.0,1.4,3*00",
  "$GPGSV,1,1,03,01,83,154,46,08,36,185,42,14,10,000,36,1*00",
  "$GAGSV,1,1,02,24,53,073,43,09,75,227,45,7*00",
  "$GAGSV,1,1,01,99,91,000,43,7*00",
  "$GLGSV,1,1,01,05,10,010,30,1*00",
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

test("the interim compute says what needs engine 0.36", async () => {
  let got;
  const eng = fake(["nmea_training"], { nmea_training: (toml, seed) => { got = { toml, seed }; return JSON.stringify({ nmea: NMEA }); } });
  const a = createGeometryAdapter(eng);
  const r = await a.compute(state("?sys=G,E,L:iridium-stl&sky=p:fjord&seed=5"));
  assert.equal(got.seed, 5);
  assert.equal(r.backend, "interim");
  assert.equal(r.ellipse, null);
  assert.equal(r.openFraction, null);
  assert.ok(r.missing.includes("GDOP") && r.missing.includes("the Monte Carlo cloud") && r.missing.includes("the skyline"));
  assert.ok(r.notes.some((n) => /LEO: needs engine 0\.36/.test(n)) && r.notes.some((n) => /skyline is drawn but not applied/.test(n)));
  assert.equal((await a.compute(state("?sys=L:iridium-stl"))).sats.length, 0);
  assert.equal(await a.monteCarlo(state()), null);
  assert.equal(await a.svg(state()), null);
  assert.equal(await a.profile({ kind: "canyon" }), null);
  const p = await a.preset("fjord");
  assert.match(p.provenance, /Illustrative/);
  assert.ok(isOpen((await a.preset("__proto__")).table));
  await assert.rejects(() => createGeometryAdapter(fake(["nmea_training"], { nmea_training: () => "not json" })).compute(state()), /not JSON/);
});

test("no engine: an empty result that says why", async () => {
  const r = await createGeometryAdapter(null).compute(state());
  assert.equal(r.backend, "none");
  assert.match(r.notes[0], /not running/);
});

test("the 0.36 request: site, offset, systems, mask, UERE, and the skyline as the engine's [skyline] table", () => {
  const s = state("?site=1,2,3&t=60&sys=G,L:xona-pulsar&mask=7&uere=2&sky=c:35,18,24,40,1.5");
  assert.deepEqual(buildRequest(s), { site: { lat_deg: 1, lon_deg: 2, height_m: 3 }, t_offset_s: 60, systems: ["gps"], mask_deg: 7, sigma_uere_m: 2, leo_preset: "xona-pulsar", skyline: { street_az_deg: 35, width_m: 18, height_left_m: 24, height_right_m: 40, antenna_m: 1.5 } });
  assert.deepEqual(skylineRequest({ kind: "preset", name: "fjord" }), { preset: "fjord" });
  assert.equal(skylineRequest({ kind: "none" }), null);
  const tab = skylineRequest({ kind: "table", table: brush(emptyTable(), 10, 20) }).table;
  assert.equal(tab.length, BINS);
  assert.deepEqual(tab[10], [10, 20]);
  const withTle = buildRequest(s, "1 25544U ...");
  assert.deepEqual(withTle.systems, []);
  assert.equal(withTle.leo_preset, undefined);
  assert.equal(withTle.tle, "1 25544U ...");
});

test("the 0.36 replies are checked field by field", async () => {
  const reply = {
    satellites: [{ system: "gps", id: 3, az_deg: 370, el_deg: 40, visible: true }, { system: "leo", id: "L12", az_deg: 10, el_deg: 15, visible: true, masked_by_skyline: true }, { system: "evil", az_deg: 1, el_deg: 1 }, { system: "__proto__", az_deg: 1, el_deg: 1 }, { system: "gps", az_deg: "x", el_deg: 1 }],
    dop: { gdop: 2.1, pdop: 1.9, hdop: 1.1, vdop: 1.5, tdop: "1" },
    ellipse: { semi_major_m: 2.6, semi_minor_m: 1.4, orientation_deg: 33 },
    skyline: { open_fraction: 0.62, provenance: "illustrative dimensions" },
    notes: ["a", 5],
  };
  const r = parseGeometry(reply, "0.36.0");
  assert.deepEqual(r.sats.map((x) => [x.sys, x.id, x.az, x.used, x.masked]), [["gps", "3", 10, true, false], ["leo", "L12", 10, false, true]]);
  assert.equal(r.nMasked, 1);
  assert.equal(r.dop.tdop, null);
  assert.deepEqual(r.ellipse, { semiMajorM: 2.6, semiMinorM: 1.4, orientationDeg: 33 });
  assert.equal(r.openFraction, 0.62);
  assert.deepEqual(r.notes, ["Skyline: illustrative dimensions", "a"]);
  assert.throws(() => parseGeometry(null));
  assert.throws(() => parseGeometry([]));
  assert.equal(parseGeometry({ open_fraction: 2 }).openFraction, null);

  const mc = parseMonteCarlo({ seed: 9, n: 3, samples_enu_m: [[1, 2, 3], [0.5, -1, 0], [1, "x", 0], "no"], rms_horizontal_m: 2.2, inside_95_fraction: 0.951 });
  assert.equal(mc.points.length, 2);
  assert.equal(mc.inside95, 0.951);
  assert.equal(mc.samples, 3);
  assert.throws(() => parseMonteCarlo("x"));

  const presets = parsePresets({ presets: [{ name: "fjord", table: Array(BINS).fill(21.8), provenance: "p" }, { name: "evil", table: Array(BINS).fill(1) }] });
  assert.deepEqual(Object.keys(presets), ["fjord"]);
  assert.equal(elAt(presets.fjord.table, 100), 22);
  assert.equal(parseProfile({ table: "x" }), null);

  const calls = [];
  const eng = fake(["nmea_training", ...GEOMETRY_FNS], {
    sky_geometry: (j) => { calls.push(["g", JSON.parse(j)]); return JSON.stringify(reply); },
    sky_monte_carlo: (j, seed, n) => { calls.push(["mc", seed, n]); return JSON.stringify({ seed, n, samples_enu_m: [[1, 1, 0]] }); },
    skyline_presets: () => JSON.stringify([{ name: "fjord", table: [[0, 2], [90, 22], [180, 2], [270, 22]], provenance: "engine" }]),
    skyline_profile: (j) => { calls.push(["p", JSON.parse(j)]); return JSON.stringify({ table: Array(BINS).fill(30) }); },
    skyplot_svg: () => "<svg/>",
  });
  const a = createGeometryAdapter(eng);
  const s = state("?mc=500&seed=4");
  assert.equal((await a.compute(s)).ellipse.semiMajorM, 2.6);
  assert.equal((await a.monteCarlo(s)).points.length, 1);
  assert.deepEqual(calls[1], ["mc", 4, 500]);
  assert.equal(await a.monteCarlo(state("?mc=0")), null, "mc=0 switches the cloud off");
  const p = await a.preset("fjord");
  assert.equal(p.provenance, "engine");
  assert.equal(elAt(p.table, 90), 22);
  assert.equal(elAt(await a.profile({ kind: "canyon", streetAz: 0, widthM: 20, leftM: 30, rightM: 30, antennaM: 1.5 }), 5), 30);
  assert.equal(await a.svg(s), "<svg/>");
});

// Against the real engine package, when it is checked out (not an LFS pointer).
const PKG = new URL("../pkg/", import.meta.url);
const havePkg = (() => { try { return statSync(new URL("kshana_bg.wasm", PKG)).size > 1024; } catch { return false; } })();
test("against the real engine package", { skip: !havePkg && "pkg/kshana_bg.wasm is not checked out" }, async () => {
  const mod = await import(new URL("kshana.js", PKG).href);
  await mod.default({ module_or_path: readFileSync(new URL("kshana_bg.wasm", PKG)) });
  // The page's engine only offers the functions ENGINE_FNS lists (lib/engine-page.mjs); do the same here.
  const eng = { available: true, version: mod.version(), has: (f) => ENGINE_FNS.includes(f) && typeof mod[f] === "function", call: async (f, ...a) => mod[f](...a) };
  const a = createGeometryAdapter(eng);
  assert.ok(["interim", "geometry"].includes(a.backend));
  const r = await a.compute(state("?mask=10"));
  assert.ok(r.sats.length >= 12, `only ${r.sats.length} satellites`);
  assert.ok(r.sats.every((x) => x.el >= 0 && x.el <= 90 && x.az >= 0 && x.az < 360));
  assert.ok(r.nUsed >= 4);
  assert.ok(r.dop.pdop > 0 && r.dop.hdop > 0 && r.dop.vdop > 0 && r.dop.pdop >= r.dop.hdop);
  assert.deepEqual(await a.compute(state("?mask=10")), r, "the same state gives the same answer");
  assert.notDeepEqual((await a.compute(state("?mask=10&t=10800"))).sats, r.sats, "the sky moves with the time offset");
});
