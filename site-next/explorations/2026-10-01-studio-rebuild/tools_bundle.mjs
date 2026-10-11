// Bundles the Studio's many small files into few, for a host that limits the number of files.
//
//   node tools_bundle.mjs
//       writes scenarios/index.json here: every bundled scenario file in one JSON, so a host
//       serves one file instead of one per scenario.
//   node tools_bundle.mjs --to <published Studio folder> [--recorded-packs 12] [--native-packs 20]
//       also writes, into <folder>, the recorded runs and the native engine's recordings as
//       packs: recorded/pack-NN.json.gz and native/pack-NN.json.gz (gzip of one JSON object
//       keyed by the loose file's name), and an index.json in each whose entries name their
//       pack. The same for every optional group listed in groups.json (one pack each).
//       The loose files here are not touched: they stay the layout a host without a file
//       limit serves, and the Studio reads whichever the index names (lib/packs.mjs).
//
// The site build runs the second form when it copies the Studio. Output is deterministic:
// the same recordings give byte-identical packs.
import { statSync, readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { gunzipSync, gzipSync } from "node:zlib";
import { join } from "node:path";
import { planPacks } from "./lib/packs.mjs";

const here = new URL(".", import.meta.url).pathname;
const args = process.argv.slice(2);
const opt = (name, dflt) => { const i = args.indexOf(name); return i < 0 ? dflt : args[i + 1]; };
const TO = opt("--to", null);
const PREVIEW_ROOT = opt("--preview-root", null);
const MAX = { recorded: parseInt(opt("--recorded-packs", "12"), 10), native: parseInt(opt("--native-packs", "20"), 10) };

// 1. Scenario files.
const dir = here + "scenarios/";
const files = readdirSync(dir).filter((f) => f.endsWith(".toml")).sort();
const scenarioBundle = JSON.stringify(Object.fromEntries(files.map((f) => [f, readFileSync(dir + f, "utf8")])));
writeFileSync(dir + "index.json", scenarioBundle);
console.log(`${files.length} scenario files bundled`);
if (!TO) process.exit(0);

// 2. Recorded runs and native recordings, as packs, into the published folder.
mkdirSync(join(TO, "scenarios"), { recursive: true });
writeFileSync(join(TO, "scenarios", "index.json"), scenarioBundle);
const groups = [""];
const WITHOUT = (opt("--without", "") || "").split(",").filter(Boolean);
if (existsSync(here + "groups.json")) for (const g of JSON.parse(readFileSync(here + "groups.json", "utf8")).groups) if (existsSync(here + g + "index.json") && !WITHOUT.includes(g)) groups.push(g);
let nPacks = 0, nRecords = 0, bytesOut = 0;
for (const g of groups) {
  for (const kind of ["recorded", "native"]) {
    const src = here + g + kind + "/";
    if (!existsSync(src + "index.json")) continue;
    const index = JSON.parse(readFileSync(src + "index.json", "utf8"));
    const text = (file) => { const b = readFileSync(src + file); return (b[0] === 0x1f && b[1] === 0x8b ? gunzipSync(b) : b).toString("utf8"); };
    const items = Object.entries(index.runs).filter(([, e]) => e && !e.error && e.file).map(([, e]) => ({ name: e.file, text: text(e.file) }));
    for (const it of items) it.bytes = it.text.length;
    const plan = planPacks(items, g ? 1 : MAX[kind]);
    const byName = new Map(items.map((it) => [it.name, it.text]));
    const packOf = new Map();
    const out = join(TO, g, kind);
    mkdirSync(out, { recursive: true });
    plan.forEach((names, i) => {
      const packName = `pack-${String(i).padStart(2, "0")}.json.gz`;
      // Members are spliced in as the recorder wrote them (no re-serialising of a record).
      const body = "{" + names.map((n) => JSON.stringify(n) + ":" + byName.get(n)).join(",") + "}";
      const gz = gzipSync(Buffer.from(body, "utf8"), { level: 9 });
      writeFileSync(join(out, packName), gz);
      for (const n of names) packOf.set(n, packName);
      nPacks++; bytesOut += gz.length;
    });
    for (const e of Object.values(index.runs)) if (e && e.file && packOf.has(e.file)) e.pack = packOf.get(e.file);
    writeFileSync(join(out, "index.json"), JSON.stringify(index, null, 1));
    nRecords += items.length;
  }
}
console.log(`${nRecords} recordings packed into ${nPacks} files, ${(bytesOut / 1e6).toFixed(1)} MB (${groups.length - 1} optional group(s))`);

// 3. The claude.ai preview host's limits (255 files, 64 MB, 15 MB a file) are not kshana.dev's (Pages:
// 1 GB, 100 MB a file) but are still enforced for the preview publish: give the tree to be published
// with --preview-root and this fails when it is over any of them.
if (PREVIEW_ROOT) {
  const walk = (d) => readdirSync(d, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? walk(join(d, e.name)) : [join(d, e.name)]);
  const all = walk(PREVIEW_ROOT);
  const sizes = all.map((f) => statSync(f).size);
  const total = sizes.reduce((a, b) => a + b, 0);
  const bad = [];
  if (all.length > 255) bad.push(`${all.length} files, over 255`);
  if (total > 64e6) bad.push(`${(total / 1e6).toFixed(1)} MB, over 64 MB`);
  all.forEach((f, i) => { if (sizes[i] > 15e6) bad.push(`${f} over 15 MB`); });
  if (bad.length) { console.error("preview host limits exceeded: " + bad.join("; ")); process.exit(1); }
  console.log(`preview limits ok: ${all.length} files, ${(total / 1e6).toFixed(1)} MB`);
}
