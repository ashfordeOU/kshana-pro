// Records what only the NATIVE command-line engine can make, for the Studio's "recorded" views:
// the engine's own run report, its animation (--animate svg,html), its interoperability exports
// (--export all) and, for a kind the browser build cannot run, the whole run.
// Run: node tools_native.mjs --repo <kshana checkout> [--bin <kshana binary>] [--work <scratch dir>] [--jobs 4] [names...]
// Writes native/<scenario>.json.gz (gzip: the packs are large and text) + native/index.json, and
// celeste/native/ for the optional group.
//
// Every scenario is run from a scratch working directory as `kshana scenarios/<file>.toml`, the
// command a reader types at the repository root, so the recorded report's reproducibility record
// names that command and no path of the recording machine.
import { readFileSync, readdirSync, writeFileSync, mkdirSync, copyFileSync, existsSync, symlinkSync, rmSync, statSync } from "node:fs";
import { execFile } from "node:child_process";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

const here = new URL(".", import.meta.url).pathname;
const args = process.argv.slice(2);
const opt = (name, dflt) => { const i = args.indexOf(name); if (i < 0) return dflt; const v = args[i + 1]; args.splice(i, 2); return v; };
const REPO = opt("--repo", null);
if (!REPO) { console.error("usage: node tools_native.mjs --repo <kshana checkout> [--bin <binary>] [--work <dir>] [--jobs 4] [names...]"); process.exit(2); }
const BIN = opt("--bin", join(REPO, "target/release/kshana"));
const WORK = opt("--work", join(here, "..", "..", "scratch", "studio-next", "native-work"));
const JOBS = parseInt(opt("--jobs", "4"), 10);
const only = args.map((x) => x.replace(/\.toml$/, ""));

// Size limits of one packed record. A file over FILE_CAP, or one that would take the record's
// exports over TOTAL_CAP, is listed with its size and the command that makes it, not stored.
const FILE_CAP = 760_000, TOTAL_CAP = 900_000, ANIM_CAP = 900_000, REPORT_CAP = 900_000;
// Stored first when the total runs out: the map formats a browser can draw or a reader opens directly.
const PRIORITY = ["geojson", "sigmf", "czml", "kml", "stk"];
const TEXT_EXT = /\.(czml|kml|geojson|e|sigmf-meta)$/;
const FORMAT_OF = (name) => (name.endsWith(".czml") ? "czml" : name.endsWith(".kml") ? "kml" : name.endsWith(".geojson") ? "geojson" : name.endsWith(".e") ? "stk" : /\.sigmf-(meta|data)$/.test(name) ? "sigmf" : null);

const run = (argv) => new Promise((resolve) => {
  execFile(BIN, argv, { cwd: WORK, encoding: "utf8", maxBuffer: 1 << 28, timeout: 900_000 }, (err, stdout, stderr) => {
    resolve({ rc: err ? (typeof err.code === "number" ? err.code : 1) : 0, stdout: stdout || "", stderr: stderr || "" });
  });
});
const scrub = (s) => String(s).split(WORK + "/").join("").split(WORK).join(".");

rmSync(WORK, { recursive: true, force: true });
mkdirSync(join(WORK, "scenarios"), { recursive: true });
// Repository folders a scenario may read by a relative path (read only).
for (const d of ["tests", "data", "examples"]) if (existsSync(join(REPO, d))) symlinkSync(join(REPO, d), join(WORK, d));

const groups = [{ dir: "scenarios", out: "native" }];
if (existsSync(here + "celeste")) groups.push({ dir: "celeste", out: "celeste/native" });
const version = (await run(["--version"])).stdout.trim().replace(/^kshana\s+/, "");
const jobs = [];
for (const g of groups) {
  mkdirSync(here + g.out, { recursive: true });
  for (const f of readdirSync(here + g.dir).filter((x) => x.endsWith(".toml") && !x.endsWith(".suite.toml")).sort()) {
    const name = f.replace(/\.toml$/, "");
    if (only.length && !only.includes(name)) continue;
    copyFileSync(here + g.dir + "/" + f, join(WORK, "scenarios", f));
    jobs.push({ g, f, name });
  }
}

async function record({ g, f, name }) {
  const rel = `scenarios/${f}`;
  const produced = () => readdirSync(join(WORK, "scenarios")).filter((x) => x.startsWith(name + ".") && x !== f);
  const read = (x) => readFileSync(join(WORK, "scenarios", x));
  const rec = { engine_version: version, source: "native", platform: `${process.platform} ${process.arch}`, file: f, toml: readFileSync(here + g.dir + "/" + f, "utf8") };

  // 1. Which interoperability formats apply, and why not (no run).
  const plan = await run([rel, "--export", "list"]);
  rec.export_plan = plan.stdout.split("\n").filter(Boolean).map((l) => {
    const m = l.match(/^(\w+): (applies|does not apply)(?::\s*(.*))?$/);
    return m ? { format: m[1], applies: m[2] === "applies", reason: m[3] || "" } : { format: "", applies: false, reason: scrub(l) };
  });
  // 2. The exports.
  const ex = await run([rel, "--export", "all"]);
  rec.exports = { command: `kshana ${rel} --export all`, files: [] };
  if (ex.rc === 0) {
    let total = 0;
    const size = (x) => statSync(join(WORK, "scenarios", x)).size;
    const files = produced().filter((x) => FORMAT_OF(x)).sort((a, b) => PRIORITY.indexOf(FORMAT_OF(a)) - PRIORITY.indexOf(FORMAT_OF(b)) || size(a) - size(b) || a.localeCompare(b));
    for (const x of files) {
      const buf = read(x);
      const entry = { name: x, format: FORMAT_OF(x), bytes: buf.length };
      const stored = TEXT_EXT.test(x) ? buf.length : Math.ceil(buf.length / 3) * 4;
      if (buf.length <= FILE_CAP && total + stored <= TOTAL_CAP) {
        if (TEXT_EXT.test(x)) entry.text = buf.toString("utf8"); else entry.base64 = buf.toString("base64");
        total += stored;
      }
      rec.exports.files.push(entry);
    }
    rec.exports.files.sort((a, b) => a.name.localeCompare(b.name));
  } else rec.exports.error = scrub((ex.stderr || ex.stdout).trim().split("\n").pop() || "export failed");
  // 3. The animation.
  const an = await run([rel, "--animate", "svg,html"]);
  rec.animation = { command: `kshana ${rel} --animate svg,html` };
  if (an.rc === 0) {
    for (const [k, x] of [["svg", `${name}.animation.svg`], ["html", `${name}.animation.html`]]) {
      if (!existsSync(join(WORK, "scenarios", x))) continue;
      const t = read(x).toString("utf8");
      rec.animation[k + "_bytes"] = t.length;
      rec.animation[k] = t.length <= ANIM_CAP ? t : null;
    }
  } else rec.animation.error = scrub((an.stderr || an.stdout).trim().split("\n").pop() || "no animation").replace(/^error:\s*/, "");
  // 4. The plain run, last, so the report on disk is the plain command's.
  const plain = await run([rel]);
  rec.command = `kshana ${rel}`;
  if (plain.rc !== 0) rec.error = scrub((plain.stderr || plain.stdout).trim().split("\n").pop() || "run failed");
  else {
    rec.summary = plain.stdout.split("\n")[0].replace(/^scenario\s+/, "");
    rec.summary_line = plain.stdout.split("\n")[0];
    const get = (x) => (existsSync(join(WORK, "scenarios", x)) ? read(x).toString("utf8") : null);
    const report = get(`${name}.report.html`);
    rec.report_bytes = report ? report.length : 0;
    rec.report_html = report && report.length <= REPORT_CAP ? report : null;
    rec.result_bytes = (get(`${name}.result.json`) || "").length;
    // The whole run is kept only where the browser build cannot make it (the Studio then shows it as a recorded run).
    rec.json = get(`${name}.result.json`);
    rec.svg = get(`${name}.chart.svg`);
    rec.csv = get(`${name}.table.csv`);
  }
  for (const k of ["report_html", "json", "svg"]) if (typeof rec[k] === "string" && rec[k].includes(WORK)) throw new Error(`${f}: ${k} names the recording directory`);
  return rec;
}

const index = { engine_version: version, source: "native", platform: `${process.platform} ${process.arch}`, recorded_utc: new Date().toISOString(), runs: {} };
const celIndex = { ...index, runs: {} };
if (only.length) for (const [ix, p] of [[index, "native/index.json"], [celIndex, "celeste/native/index.json"]]) { try { Object.assign(ix.runs, JSON.parse(readFileSync(here + p, "utf8")).runs); } catch { /* first run */ } }
let next = 0, done = 0;
const keepRun = new Set((process.env.KSHANA_NATIVE_FULL || "lunar-llr-datum").split(",").filter(Boolean));
async function worker() {
  while (next < jobs.length) {
    const j = jobs[next++];
    const rec = await record(j);
    if (!keepRun.has(j.name)) { delete rec.json; delete rec.svg; delete rec.csv; }
    const text = JSON.stringify(rec);
    const gz = gzipSync(Buffer.from(text, "utf8"), { level: 9 });
    writeFileSync(here + j.g.out + "/" + j.name + ".json.gz", gz);
    (j.g.out === "native" ? index : celIndex).runs[j.f] = rec.error ? { error: rec.error } : {
      file: j.name + ".json.gz", bytes: gz.length, unpacked_bytes: text.length, report: rec.report_html !== null, animation: !!(rec.animation.html || rec.animation.svg),
      animation_reason: rec.animation.error || "", exports: rec.exports.files.map((x) => x.format).filter((v, i, a) => a.indexOf(v) === i).sort(),
      export_files: rec.exports.files.length, export_files_stored: rec.exports.files.filter((x) => "text" in x || "base64" in x).length, full_run: keepRun.has(j.name),
    };
    done++;
    if (done % 10 === 0 || done === jobs.length) console.log(`${done}/${jobs.length}`);
  }
}
await Promise.all(Array.from({ length: JOBS }, worker));
const sortRuns = (ix) => { ix.runs = Object.fromEntries(Object.entries(ix.runs).sort(([a], [b]) => a.localeCompare(b))); return ix; };
writeFileSync(here + "native/index.json", JSON.stringify(sortRuns(index), null, 1));
if (groups.length > 1) writeFileSync(here + "celeste/native/index.json", JSON.stringify(sortRuns(celIndex), null, 1));
const all = [...Object.values(index.runs), ...Object.values(celIndex.runs)];
console.log(`${all.length} native records; ${all.filter((r) => r.error).length} failed; ${all.filter((r) => r.animation).length} with an animation; ${all.filter((r) => r.exports && r.exports.length).length} with an export`);
