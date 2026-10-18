// SPDX-License-Identifier: AGPL-3.0-only
// Interim recordings of the two optical-atmosphere scenarios WITH the 0.38 time series, from a native kshana build,
// until the release package re-records the whole Studio (RERECORD.md step 4). The recorded entry keeps the shape
// tools_record.mjs writes, and says it is native and from which engine commit.
//
//   node tools_record_optical_series.mjs --dir <folder holding optical-atmosphere-*.result.json / .chart.svg / .toml / *.stdout
//        written by `kshana scenarios/optical-atmosphere-network.toml` and -techniques.toml> --engine-commit <40-hex SHA>
//
// Checks first that the result is the recorded 0.37 one with the series and its units added, nothing else changed.
import { readFileSync, writeFileSync } from "node:fs";
const arg = (n) => { const i = process.argv.indexOf("--" + n); return i > 0 ? process.argv[i + 1] : null; };
const dir = arg("dir"), commit = arg("engine-commit");
if (!dir || !/^[0-9a-f]{40}$/.test(commit || "")) { console.error("usage: node tools_record_optical_series.mjs --dir <folder> --engine-commit <40-hex SHA>"); process.exit(2); }
const here = new URL(".", import.meta.url).pathname;
for (const n of ["network", "techniques"]) {
  const file = `optical-atmosphere-${n}.toml`, stem = `optical-atmosphere-${n}`;
  const old = JSON.parse(readFileSync(`${here}recorded/${stem}.json`, "utf8"));
  const json = readFileSync(`${dir}/${stem}.result.json`, "utf8"), nj = JSON.parse(json), oj = JSON.parse(old.json);
  for (const s of nj.sites) delete s.timeseries;
  nj.units = Object.fromEntries(Object.entries(nj.units).filter(([k]) => !k.includes("timeseries")));
  if (JSON.stringify(nj) !== JSON.stringify(oj)) { console.error(`${stem}: the new result differs from the recorded one by more than the series`); process.exit(1); }
  const stdout = readFileSync(`${dir}/${n}.stdout`, "utf8");
  if (!stdout.includes(old.summary)) { console.error(`${stem}: the summary line changed`); process.exit(1); }
  const rec = { engine_version: old.engine_version, source: "native", platform: "linux x86_64 (debug build)", command: `kshana scenarios/${file}`, engine_commit: commit,
    note: "interim recording with the 0.38 time series from a native build; the release package re-records it", file, toml: readFileSync(`${dir}/${file}`, "utf8"), summary: old.summary, json, svg: readFileSync(`${dir}/${stem}.chart.svg`, "utf8"), csv: null, exports: {} };
  writeFileSync(`${here}recorded/${stem}.json`, JSON.stringify(rec));
  console.log(`${stem}: recorded with the series (${Math.round(JSON.stringify(rec).length / 1024)} kB)`);
}
