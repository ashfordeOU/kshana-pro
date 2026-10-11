// SPDX-License-Identifier: AGPL-3.0-only
import { test } from "node:test";
import assert from "node:assert/strict";
import { parseState, serializeState, normaliseState, readTime, readPoints, DEFAULTS, PLACES, SYSTEMS, MAX_SKY_POINTS, MAX_QUERY } from "./geomstate.mjs";

test("the bare address is the default state, and the default state is the bare address", () => {
  const { state, notes } = parseState("");
  assert.deepEqual(notes, []);
  assert.equal(state.place, DEFAULTS.place);
  assert.deepEqual(state.sys, [...DEFAULTS.sys]);
  assert.equal(serializeState(state), "");
});

test("a full state round-trips through the address", () => {
  const s = { place: "custom", lat: -33.8568, lon: 151.2153, h: 42.5, t: "2027-03-01T06:30:15Z", sys: ["gps", "leo", "galileo"], mask: 12.5, sky: "custom", pts: [{ az: 0, el: 10 }, { az: 90.5, el: 35 }, { az: 270, el: 60.2 }], sigma: 1.75, seed: 424242, n: 2500 };
  const q = serializeState(s);
  const back = parseState(q);
  assert.deepEqual(back.notes, []);
  assert.deepEqual(back.state, { ...s, sys: ["gps", "galileo", "leo"] });
  assert.equal(serializeState(back.state), q, "serialising is stable");
});

test("a named place implies its coordinates, and moving off it makes the place custom", () => {
  assert.equal(serializeState({ ...DEFAULTS, place: "geiranger", ...PLACES.geiranger }), "?place=geiranger");
  const r = parseState("?place=geiranger");
  assert.equal(r.state.lat, PLACES.geiranger.lat);
  assert.equal(parseState("?place=geiranger&lat=10").state.place, "custom");
  const h = parseState("?place=geiranger&h=300").state;
  assert.equal(h.place, "geiranger");
  assert.equal(h.h, 300);
});

test("hostile and malformed values are refused, each with a note, and the default stands", () => {
  const bad = [
    ["lat=1e2", "lat"], ["lat=91", "lat"], ["lat=NaN", "lat"], ["lat=Infinity", "lat"], ["lat=0x10", "lat"], ["lat= 5", "lat"], ["lat=5.123456", "lat"],
    ["mask=46", "mask"], ["sigma=0", "sigma"], ["seed=-1", "seed"], ["seed=1.5", "seed"], ["n=99999", "n"],
    ["t=2026-02-30T00:00:00Z", "t"], ["t=2026-10-12T12:00:00%2B02:00", "t"], ["t=1999-01-01T00:00:00Z", "t"], ["t=<script>", "t"],
    ["sys=gps,javascript:alert(1)", "sys"], ["sys=__proto__", "sys"], ["place=constructor", "place"], ["place=__proto__", "place"],
    ["sky=../../etc/passwd", "sky"], ["sky=custom&pts=1:2;3:4", "pts"], ["sky=custom&pts=360:10", "pts"], ["sky=custom&pts=10:91", "pts"],
    ["sky=custom&pts=" + encodeURIComponent('"><svg onload=alert(1)>'), "pts"],
  ];
  for (const [q, key] of bad) {
    const { state, notes } = parseState("?" + q);
    assert.ok(notes.length >= 1, `${q}: no note`);
    if (key !== "pts" && key !== "place") assert.deepEqual(state[key], key === "sys" ? [...DEFAULTS.sys] : DEFAULTS[key], q);
    if (key === "pts") assert.deepEqual(state.pts, []);
    if (key === "place") assert.equal(state.place, DEFAULTS.place);
  }
});

test("unknown keys are ignored, a repeated key keeps its first value, and an over-long address is not read", () => {
  assert.deepEqual(parseState("?theme=dark&foo=bar").notes, []);
  const r = parseState("?mask=10&mask=20");
  assert.equal(r.state.mask, 10);
  assert.equal(r.notes.length, 1);
  const long = parseState("?mask=10&x=" + "a".repeat(MAX_QUERY));
  assert.equal(long.state.mask, DEFAULTS.mask);
  assert.equal(long.notes.length, 1);
});

test("skyline points: limits, and they are dropped unless the skyline is custom", () => {
  assert.equal(readPoints(Array.from({ length: MAX_SKY_POINTS + 1 }, (_, i) => `${i}:5`).join(",")), null);
  assert.equal(readPoints(Array.from({ length: MAX_SKY_POINTS }, (_, i) => `${i}:5`).join(",")).length, MAX_SKY_POINTS);
  assert.deepEqual(readPoints(""), []);
  assert.deepEqual(parseState("?sky=fjord&pts=10:20").state.pts, []);
});

test("an empty constellation list is kept (the view says nothing is selected), and the order is canonical", () => {
  assert.deepEqual(parseState("?sys=").state.sys, []);
  assert.deepEqual(parseState("?sys=leo,gps").state.sys, ["gps", "leo"]);
  assert.ok(SYSTEMS.every((s) => /^[a-z]+$/.test(s)));
});

test("times: only whole-second UTC instants that exist", () => {
  assert.equal(readTime("2026-10-12T12:00:00Z"), "2026-10-12T12:00:00Z");
  assert.equal(readTime("2028-02-29T00:00:00Z"), "2028-02-29T00:00:00Z");
  assert.equal(readTime("2027-02-29T00:00:00Z"), null);
  assert.equal(readTime("2026-10-12T24:00:00Z"), null);
  assert.equal(readTime("2026-10-12T12:00:00.5Z"), null);
});

test("normaliseState clamps a page-built state to what a link could carry", () => {
  const s = normaliseState({ ...DEFAULTS, mask: 80, sigma: 2.346, sys: ["gps", "nope"] });
  assert.equal(s.mask, DEFAULTS.mask);
  assert.equal(s.sigma, 2.35);
  assert.deepEqual(s.sys, ["gps"]);
});
