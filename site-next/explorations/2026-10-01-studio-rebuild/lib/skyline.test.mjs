// SPDX-License-Identifier: AGPL-3.0-only
import { test } from "node:test";
import assert from "node:assert/strict";
import { normalise, outlineAt, outline, addPoint, movePoint, removePoint, keyEdit, describe, wrapAz, MAX_SKY_POINTS, MIN_GAP_DEG } from "./skyline.mjs";

const P = (...xs) => xs.map(([az, el]) => ({ az, el }));

test("normalise sorts, rounds, wraps, clamps, merges close vertices and drops junk", () => {
  assert.deepEqual(normalise(P([200, 10], [-10, 95], [370, -3], [200.04, 20])), P([10, 0], [200, 20], [350, 90]));
  assert.deepEqual(normalise([null, { az: NaN, el: 1 }, { az: 1, el: Infinity }, "x"]), []);
  assert.deepEqual(normalise(P([0, 5], [359.5, 9])), P([359.5, 9]), "vertices closer than the gap across north merge");
  assert.equal(normalise(Array.from({ length: 200 }, (_, i) => ({ az: i * 1.5, el: 5 }))).length, MAX_SKY_POINTS);
});

test("the outline interpolates linearly and wraps through north", () => {
  const p = P([90, 10], [270, 30]);
  assert.equal(outlineAt(p, 90), 10);
  assert.equal(outlineAt(p, 180), 20);
  assert.equal(outlineAt(p, 0), 20);
  assert.equal(outlineAt(p, 315), 25);
  assert.equal(outlineAt([], 10), 0);
  assert.equal(outlineAt(P([5, 12]), 200), 12);
  assert.equal(outline(p, 90).length, 4);
});

test("adding: placed in order, refused when too close or full", () => {
  let r = addPoint(P([10, 5], [200, 5]), 100, 30);
  assert.deepEqual(r.points.map((q) => q.az), [10, 100, 200]);
  assert.equal(r.index, 1);
  assert.equal(addPoint(r.points, 100.5, 3).index, -1);
  assert.equal(addPoint(r.points, 9.6 + 360, 3).index, -1);
  const full = Array.from({ length: MAX_SKY_POINTS }, (_, i) => ({ az: i * 5, el: 1 }));
  assert.equal(addPoint(full, 2.5, 1).index, -1);
});

test("dragging a vertex never passes its neighbours and clamps elevation", () => {
  const p = P([10, 5], [100, 20], [200, 5]);
  assert.deepEqual(movePoint(p, 1, 150, 95)[1], { az: 150, el: 90 });
  assert.equal(movePoint(p, 1, 250, 20)[1].az, 200 - MIN_GAP_DEG);
  assert.equal(movePoint(p, 1, 5, 20)[1].az, 10 + MIN_GAP_DEG);
  // The first vertex can move back through north, as far as the last vertex allows.
  const q = movePoint(p, 0, 350, 5);
  assert.deepEqual(q.map((x) => x.az), [100, 200, 350]);
  assert.deepEqual(movePoint(p, 0, 190, 5).map((x) => x.az), [99, 100, 200], "stops one gap before the next vertex");
  assert.deepEqual(movePoint(p, 0, 190.5, 5).map((x) => x.az), [100, 200, 201], "the nearer way round (west, through north) wins, and stops one gap after the last vertex");
  assert.deepEqual(movePoint(p, 0, 230, 5).map((x) => x.az), [100, 200, 230], "west through north");
  assert.equal(movePoint(p, 7, 1, 1), p, "an index out of range changes nothing");
});

test("removing", () => {
  assert.deepEqual(removePoint(P([1, 1], [2, 2]), 0), P([2, 2]));
  assert.deepEqual(removePoint(P([1, 1]), 5), P([1, 1]));
});

test("keyboard editing", () => {
  const p = P([10, 5], [100, 20], [200, 5]);
  assert.equal(keyEdit(p, 1, "ArrowUp").points[1].el, 21);
  assert.equal(keyEdit(p, 1, "ArrowDown", true).points[1].el, 15);
  assert.equal(keyEdit(p, 1, "ArrowRight").points[1].az, 101);
  assert.equal(keyEdit(p, 1, "ArrowLeft", true).points[1].az, 95);
  const del = keyEdit(p, 2, "Delete");
  assert.equal(del.points.length, 2);
  assert.equal(del.index, 1);
  assert.equal(keyEdit(P([5, 5]), 0, "Backspace").index, -1);
  assert.equal(keyEdit(p, 0, "End").index, 2);
  assert.equal(keyEdit(p, 0, "a"), null);
  assert.equal(keyEdit(p, 9, "ArrowUp"), null);
  // Moving the first vertex left across north keeps following it.
  const r = keyEdit(P([0.5, 7], [100, 20], [200, 5]), 0, "ArrowLeft");
  assert.deepEqual(r.points[r.index], { az: 359.5, el: 7 });
});

test("text alternative", () => {
  assert.match(describe([]), /Open horizon/);
  assert.equal(describe(P([10, 5], [100, 20])), "Skyline of 2 points; highest 20° at azimuth 100°.");
  assert.equal(wrapAz(-0.04), 0);
});
