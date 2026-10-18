// SPDX-License-Identifier: AGPL-3.0-only
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SCENARIOS, readOptical, chartsOf, barsSvg, linesSvg, factRows } from "./optical.mjs";

const here = new URL("../", import.meta.url).pathname;
const load = (id) => JSON.parse(readFileSync(here + `recorded/${id}.json`, "utf8"));

test("both 0.37 optical-atmosphere scenarios open and carry the engine's numbers unchanged", () => {
  assert.equal(SCENARIOS.length, 2);
  for (const sc of SCENARIOS) {
    const doc = load(sc.id), raw = JSON.parse(doc.json), { model, error } = readOptical(doc);
    assert.equal(error, undefined, sc.id);
    assert.equal(model.sites.length, raw.sites.length);
    raw.sites.forEach((s, i) => { assert.equal(model.sites[i].availability, s.availability_pct); assert.equal(model.sites[i].ceiling, s.availability_ceiling_pct); assert.equal(model.sites[i].margins.length, s.margin_for_availability.length); });
    assert.equal(model.link.marginDb, raw.link.margin_no_atmosphere_db);
    assert.match(model.label, /^MODELLED/); assert.match(model.summary, /MODELLED/);
    assert.match(model.sites[0].source, /ILLUSTRATIVE/);
  }
});
test("the network scenario draws diversity, and the techniques scenario draws the techniques", () => {
  const net = chartsOf(readOptical(load("optical-atmosphere-network")).model).map((c) => c.id), tec = chartsOf(readOptical(load("optical-atmosphere-techniques")).model).map((c) => c.id);
  assert.ok(net.includes("diversity") && !net.includes("techniques")); assert.ok(tec.includes("techniques") && !tec.includes("diversity"));
  for (const id of ["avail", "margin", "outage", "timing"]) { assert.ok(net.includes(id) && tec.includes(id), id); }
});
test("charts label every series at the line's end and never use a time axis the result does not have", () => {
  const charts = chartsOf(readOptical(load("optical-atmosphere-network")).model);
  const outage = charts.find((c) => c.id === "outage").svg;
  for (const n of ["northern", "central", "southern"]) assert.ok(outage.includes(`>${n}</text>`), n);
  assert.ok(charts.every((c) => !/time series|against time/i.test(c.title)));
  assert.match(outage, /outage duration, h \(log\)/);
});
test("bars are scaled to the stated maximum and carry their value as text", () => {
  const svg = barsSvg([{ label: "a", value: 50, text: "50.0 %" }], { max: 100 });
  const w = Number(svg.match(/class="op-bar[^"]*" x="\d+" y="\d+" width="([\d.]+)"/)[1]), track = Number(svg.match(/class="op-track" x="\d+" y="\d+" width="([\d.]+)"/)[1]);
  assert.ok(Math.abs(w - track / 2) < 0.1); assert.match(svg, />50\.0 %</);
});
test("an empty chart says so, and a result that is not optical is refused", () => {
  assert.match(linesSvg([], {}), /No data in this result/);
  assert.match(readOptical({ json: JSON.stringify({ kind: "other" }) }).error, /not an optical-atmosphere/);
  assert.match(readOptical({ json: "{" }).error, /not JSON/);
  assert.ok(factRows(readOptical(load("optical-atmosphere-network")).model).length === 8);
});
