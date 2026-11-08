// Records REAL engine outputs (kshana WASM, run under Node) for the Studio's offline fallback.
// Run: node tools_record.mjs [names...]   (writes recorded/*.json.gz + recorded/index.json, and
// celeste/recorded/ for the optional group when the celeste/ folder is present).
// A scenario the browser build cannot run is taken from native/<name>.json when tools_native.mjs
// has recorded its whole run there; its index entry then says source "native".
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync, unlinkSync } from "node:fs";
import { gunzipSync, gzipSync } from "node:zlib";
// Each recording is gzip-compressed (level 9, no timestamp: the same run gives the same bytes); the
// index names the .json.gz file and the Studio reads it through lib/packs.mjs, as it reads native/.
const gz = (rec) => gzipSync(Buffer.from(JSON.stringify(rec), "utf8"), { level: 9 });
// The plain recorded/<name>.json of an earlier layout goes when its .json.gz is written.
const dropPlain = (path) => { if (existsSync(path)) unlinkSync(path); };
const here = new URL(".", import.meta.url).pathname;
const m = await import(here + "pkg/kshana.js");
m.initSync({ module: readFileSync(here + "pkg/kshana_bg.wasm") });
// An export larger than this is not recorded (null = "available with the live engine").
const EXPORT_CAP = 150_000;
// Optional file names on the command line record only those (the index keeps every other entry).
// The engine commit these recordings are stamped with (a full 40-hex SHA of the rc the package was built from):
// --engine-commit <sha> or KSHANA_ENGINE_COMMIT. selfcheck.mjs --final refuses a recording without one.
const argv = process.argv.slice(2);
let engineCommit = process.env.KSHANA_ENGINE_COMMIT || null;
{ const i = argv.indexOf("--engine-commit"); if (i >= 0) { engineCommit = argv[i + 1]; argv.splice(i, 2); } }
if (engineCommit && !/^[0-9a-f]{40}$/.test(engineCommit)) { console.error("--engine-commit must be a full 40-character lowercase hex SHA"); process.exit(2); }
const only = argv.map((x) => (x.endsWith(".toml") ? x : x + ".toml"));
const groups = [{ dir: "scenarios", out: "recorded", native: "native" }];
if (existsSync(here + "celeste")) groups.push({ dir: "celeste", out: "celeste/recorded", native: "celeste/native" });
let total = 0, failed = 0, native = 0;
for (const g of groups) {
  mkdirSync(here + g.out, { recursive: true });
  const index = { engine_version: m.version(), ...(engineCommit ? { engine_commit: engineCommit } : {}), recorded_utc: new Date().toISOString(), runs: {} };
  if (only.length) { try { Object.assign(index.runs, JSON.parse(readFileSync(here + g.out + "/index.json", "utf8")).runs); } catch { /* first run */ } }
  for (const f of readdirSync(here + g.dir).filter((x) => x.endsWith(".toml") && (!only.length || only.includes(x))).sort()) {
    const toml = readFileSync(here + g.dir + "/" + f, "utf8");
    const name = f.replace(/\.toml$/, ".json");
    const out = name + ".gz";
    let all;
    const t0 = performance.now();
    try { all = JSON.parse(m.run_all(toml)); }
    catch (e) {
      const error = String(e).split("\n")[0].slice(0, 300);
      // The native engine's recording of the same file, if there is one.
      let nat = null;
      try { nat = JSON.parse(gunzipSync(readFileSync(here + g.native + "/" + name + ".gz")).toString("utf8")); } catch { /* none */ }
      if (nat && nat.json && nat.toml === toml) {
        const rec = { engine_version: nat.engine_version, source: "native", platform: nat.platform, command: nat.command, file: f, toml, summary: nat.summary, json: nat.json, svg: nat.svg || "", csv: nat.csv || null, exports: {} };
        writeFileSync(here + g.out + "/" + out, gz(rec));
        dropPlain(here + g.out + "/" + name);
        index.runs[f] = { file: out, bytes: JSON.stringify(rec).length, source: "native", browser_error: error };
        native++;
      } else { index.runs[f] = { error }; failed++; }
      continue;
    }
    const ms = Math.round(performance.now() - t0);
    const exports = {};
    for (const [k, fn] of [["sp3", m.export_sp3], ["omm", m.export_omm], ["oem", m.export_oem]]) {
      try { const t = fn(toml); if (t && t.trim()) exports[k] = t.length <= EXPORT_CAP ? t : null; } catch { /* kind yields no such export */ }
    }
    const rec = { engine_version: m.version(), file: f, toml, summary: all.summary, json: all.json, svg: all.svg, csv: all.csv || null, exports };
    writeFileSync(here + g.out + "/" + out, gz(rec));
    dropPlain(here + g.out + "/" + name);
    index.runs[f] = { file: out, bytes: JSON.stringify(rec).length, ms };
    total++;
  }
  index.runs = Object.fromEntries(Object.entries(index.runs).sort(([a], [b]) => a.localeCompare(b)));
  writeFileSync(here + g.out + "/index.json", JSON.stringify(index, null, 1));
}
console.log(`${total} recorded from the browser build; ${native} taken from the native engine; ${failed} not runnable`);
