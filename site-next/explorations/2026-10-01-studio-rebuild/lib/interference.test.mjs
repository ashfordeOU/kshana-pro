// SPDX-License-Identifier: AGPL-3.0-only
// Tests for the interference map page's data model. Run with `node lib/interference.test.mjs`.
import assert from "node:assert/strict";
import fs from "node:fs";
import { parseMapFile, statusCounts, cellRows, cellBounds, groupBySource, describeMap, cellAt, cellsForTable, licenceBlock, statusInfo, STATUSES, showValue } from "./interference.mjs";

const demo = (n) => fs.readFileSync(new URL(`../interference/demo/${n}`, import.meta.url), "utf8");
const a1 = parseMapFile(demo("adsb-2026-03-01.geojson"), "adsb-2026-03-01.geojson");
const a2 = parseMapFile(demo("adsb-2026-03-02.geojson"), "adsb-2026-03-02.geojson");
const s1 = parseMapFile(demo("ais-2026-03-01.geojson"), "ais-2026-03-01.geojson");
for (const r of [a1, a2, s1]) assert.equal(r.error, undefined);

// Licence and attribution come through verbatim, per file.
{
  const m = a1.map;
  assert.equal(m.kind, "adsb"); assert.equal(m.date, "2026-03-01");
  assert.match(m.licence.attribution, /^Synthetic demonstration data/);
  assert.equal(m.licence.licence, "CC0-1.0 (synthetic data)");
  assert.equal(m.licence.coverage.length, 1);
  assert.match(m.licence.notice, /A missing cell was not observed/);
  assert.match(m.method.caveats[0], /^SYNTHETIC FIXTURE/);
  assert.equal(m.method.id, "kshana-interference-map/adsb/v1");
}
// Every cell status is one the page knows, for its own source, and statuses follow the documented rules.
for (const r of [a1, a2, s1]) {
  const known = new Set(STATUSES[r.map.kind].map((s) => s.id));
  assert.ok(r.map.cells.length > 20);
  assert.ok(r.map.cells.every((c) => known.has(c.status)));
  assert.deepEqual(r.map.warnings.filter((w) => /does not know/.test(w)), []);
}
{
  const c = statusCounts(a1.map);
  assert.ok(c.degraded > 0 && c.not_degraded > 0 && c.insufficient_sample > 0);
  assert.equal(c.withheld_day_confounded, 0);
  for (const cell of a1.map.cells) {
    const p = cell.props;
    assert.ok(p.aircraft_observed >= 5, "below the publication minimum a cell is not published");
    if (cell.status === "insufficient_sample") assert.ok(p.aircraft_sampled < 10);
    assert.equal(cell.degraded, cell.status === "degraded");
  }
  assert.equal(a1.map.day.day_confounded, false);
}
// A confounded day declares no cell degraded and says so.
{
  assert.equal(a2.map.day.day_confounded, true);
  const c = statusCounts(a2.map);
  assert.equal(c.degraded, 0); assert.ok(c.withheld_day_confounded > 0);
  assert.ok(a2.map.warnings.some((w) => /confounded/.test(w)));
}
// AIS: a count of one or two is withheld (null) and shown as such; anomalous cells list their detectors.
{
  const c = statusCounts(s1.map);
  assert.ok(c.anomalous > 0 && c.not_anomalous > 0);
  const hot = s1.map.cells.find((x) => x.status === "anomalous");
  assert.ok(hot.props.detectors.length > 0);
  const withheld = s1.map.cells.find((x) => Object.values(x.props.vessels_flagged).some((v) => v === null));
  assert.ok(withheld, "the fixture carries withheld counts");
  assert.ok(cellRows(s1.map, withheld).some(([k, v]) => /^Flagged/.test(k) && v === "1 or 2 (withheld)"));
  assert.ok(s1.map.warnings.some((w) => /on-land detector was off/.test(w)));
  assert.ok(s1.map.cells.every((x) => x.props.vessels_observed >= 5));
}
// ADS-B and AIS are grouped, never merged; each group keeps its files and their own licences.
{
  const groups = groupBySource([s1.map, a2.map, a1.map]);
  assert.deepEqual(groups.map((g) => [g.kind, g.maps.map((m) => m.date)]), [["adsb", ["2026-03-01", "2026-03-02"]], ["ais", ["2026-03-01"]]]);
  assert.equal(groups[0].maps[0].cells.length, a1.map.cells.length, "a file's cells are its own");
}
// Refusals say why.
assert.match(parseMapFile("{", "x").error, /not JSON/);
assert.match(parseMapFile(JSON.stringify({ type: "FeatureCollection", features: [] }), "x").error, /not a Kshana interference map/);
const doc = JSON.parse(demo("ais-2026-03-01.geojson"));
const withSchema = (s) => JSON.stringify({ ...doc, kshana_interference_map: { ...doc.kshana_interference_map, schema: s } });
assert.match(parseMapFile(withSchema("kshana-interference-map/v2"), "x").error, /newer or different/);
assert.match(parseMapFile(withSchema("other/v1"), "x").error, /unknown schema/);
assert.match(parseMapFile(JSON.stringify({ ...doc, kshana_interference_map: { ...doc.kshana_interference_map, source_kind: "radar" } }), "x").error, /neither adsb nor ais/);
// A file with no licence text is shown with a warning, not given one.
{
  const bare = JSON.stringify({ ...doc, kshana_interference_map: { ...doc.kshana_interference_map, data: { dataset: "custom" } } });
  const r = parseMapFile(bare, "bare.geojson");
  assert.equal(r.map.licence.licence, ""); assert.equal(r.map.licence.attribution, "");
  assert.ok(r.map.warnings.some((w) => /no licence or attribution text/.test(w)));
}
// An unknown status is kept, drawn as no call, and reported.
{
  const odd = JSON.parse(demo("adsb-2026-03-01.geojson")); odd.features[0].properties.status = "mystery";
  const r = parseMapFile(JSON.stringify(odd), "odd");
  assert.ok(r.map.warnings.some((w) => /mystery/.test(w)));
  assert.equal(statusInfo("adsb", "mystery").tone, "none");
}
// Geometry lookups.
{
  const cell = a1.map.cells[0], b = cellBounds(cell);
  assert.ok(Math.abs(b.east - b.west - 0.5) < 1e-9 && Math.abs(b.north - b.south - 0.5) < 1e-9);
  assert.equal(cellAt(a1.map, (b.west + b.east) / 2, (b.north + b.south) / 2), 0);
  assert.equal(cellAt(a1.map, 0, 0), -1, "a place with no cell was not observed");
  assert.match(describeMap(a1.map), /ADS-B map for 2026-03-01: \d+ cells drawn.*Cells not drawn were not observed/);
  const t = cellsForTable(a1.map);
  assert.equal(t[0].c.status, "degraded"); assert.equal(t.length, a1.map.cells.length);
  assert.deepEqual(licenceBlock({}, undefined), { dataset: "", name: "", licence: "", licenceUrl: "", attribution: "", coverage: [], notice: "" });
  assert.equal(showValue(null), "none"); assert.equal(showValue({ a: 1 }), '{"a":1}');
}
console.log("interference.test.mjs: ok");
