// Self-check for the redesigned playground. Run: node selfcheck.mjs
// 1. every PARITY.md row's Check token is present in the named file;
// 2. every lib module test passes (node --test);
// 3. no external host outside the allowed CDNs, fonts and project links;
// 4. no forbidden names; no invented pricing;
// 5. every numeric claim the app states about the catalogue matches the files on disk;
// 6. the recorded fallback covers every scenario with a run, and says which engine made each;
// 7. the native engine's recordings cover every scenario, and the optional group is self-contained.
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
const here = new URL(".", import.meta.url).pathname;
const read = (f) => readFileSync(here + f, "utf8");
let fails = 0;
const ok = (cond, msg) => { if (!cond) { fails++; console.log("FAIL", msg); } };

// 0. Git LFS: the package (pkg/*.wasm) and the native recordings (native/*.gz) are LFS objects in
// kshana-pro. A clone that never ran `git lfs pull` holds 130-byte pointer files instead, and the
// module tests then fail deep inside zlib ("incorrect header check"). Say so up front instead.
{
  const { readdirSync: ls, statSync, openSync, readSync, closeSync } = await import("node:fs");
  const pointers = [];
  for (const dir of ["pkg", "native"]) {
    let names = []; try { names = ls(here + dir); } catch { continue; }
    for (const f of names) {
      const p = `${here}${dir}/${f}`; if (!statSync(p).isFile() || statSync(p).size > 1024) continue;
      const b = Buffer.alloc(40), fd = openSync(p, "r"); readSync(fd, b, 0, 40, 0); closeSync(fd);
      if (b.toString("utf8").startsWith("version https://git-lfs")) pointers.push(`${dir}/${f}`);
    }
  }
  if (pointers.length) {
    console.log(`FAIL Git LFS objects are not checked out (${pointers.length} pointer files, e.g. ${pointers[0]}).`);
    console.log("     Run, from the kshana-pro checkout:  git lfs install && git lfs pull --include='site-next/**'");
    process.exit(1);
  }
}

// 1. parity
const parity = read("PARITY.md");
const rows = [...parity.matchAll(/`((?:app\.js|index\.html|app\.css|advanced\/index\.html|tools\/tools\.js|trust\/trust\.js)): ([^`]+)`/g)];
for (const [, file, token] of rows) ok(read(file).includes(token), `PARITY check missing: ${file}: ${token}`);
console.log(`parity: ${rows.length} checks`);

// 2. module tests
try {
  const out = execFileSync("node", ["--test", "--test-reporter=spec", ...readdirSync(here + "lib").filter((f) => f.endsWith(".test.mjs")).map((f) => "lib/" + f)], { cwd: here, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  const pass = (out.match(/^ℹ pass (\d+)/m) || [])[1], fail = (out.match(/^ℹ fail (\d+)/m) || [])[1];
  ok(fail === "0", `module tests failed: ${fail}`);
  console.log(`module tests: ${pass} passed, ${fail} failed`);
} catch (e) { ok(false, "module tests did not run cleanly: " + String(e.stdout || e).slice(-400)); }

// 3. external hosts
const allowed = /^(cdn\.jsdelivr\.net|fonts\.googleapis\.com|fonts\.gstatic\.com|kshana\.dev|www\.w3\.org|github\.com)$/;
const own = ["index.html", "app.js", "app.css", "studio.css", "lib/catalog.mjs", "lib/params.mjs", "lib/views.mjs", "lib/kinds.mjs", "lib/stages.mjs", "lib/meaning.mjs", "lib/inputs.mjs", "lib/urlstate.mjs", "lib/areas.mjs", "lib/abbr.mjs", "tools_areas.mjs", "data/areas.json", "advanced/index.html", "advanced.js", "advanced/dash.css", "advanced/advanced.css", "lib/meaning-kinds.mjs", "geometry/index.html", "geometry/geometry.js", "geometry/geometry.css", "lunar/index.html", "lunar/lunar.js", "lunar/lunar.css", "lib/lunar-station.mjs", "lib/geometry-url.mjs", "lib/skyline-edit.mjs", "lib/skyplot.mjs", "lib/geometry-adapter.mjs"];
for (const f of own) for (const m of read(f).matchAll(/https?:\/\/([a-z0-9.-]+)/gi)) ok(allowed.test(m[1]), `${f}: unexpected host ${m[1]}`);
ok(/three@0\.160\.0/.test(read("index.html")), "Three.js is pinned to 0.160.0");
// The Geometry view takes no remote data: its script and modules make no request of their own (the engine is
// started by lib/engine-page.mjs from this folder), and every number comes from the engine through the adapter.
for (const f of ["geometry/geometry.js", "lib/geometry-url.mjs", "lib/skyline-edit.mjs", "lib/skyplot.mjs", "lib/geometry-adapter.mjs", "lib/trust-geometry.mjs", "lib/trust-geometry-demo.mjs"]) ok(!/\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|sendBeacon|import\s*\(/.test(read(f)), `${f}: makes no request`);
// The lunar station view fetches only this kind's two fixed files from this site (lib/lunar-station.mjs names them),
// uses generic names only, and carries the honesty block.
{
  const js = read("lunar/lunar.js"), lib = read("lib/lunar-station.mjs"), html = read("lunar/index.html");
  ok(!/\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|sendBeacon|import\s*\(/.test(lib), "lib/lunar-station.mjs: makes no request");
  ok([...js.matchAll(/\bfetch\s*\(([^)]*)\)/g)].every((m) => /^(RECORDED_FILE|SCENARIO_FILE)$/.test(m[1].trim())) && !/XMLHttpRequest|WebSocket|EventSource|sendBeacon|import\s*\(/.test(js), "lunar/lunar.js: fetches only the kind's two fixed files");
  ok(/export const SCENARIO_FILE = "scenarios\/lunar-station-coverage\.toml";/.test(lib) && /export const RECORDED_FILE = "recorded\/lunar-station-coverage\.json";/.test(lib), "the two fixed files are literal same-origin paths");
  for (const f of ["lunar/index.html", "lunar/lunar.js", "lunar/lunar.css"]) ok(!/nova\s*moon|\bESA\b|European Space Agency|Moonlight/i.test(read(f)), `${f}: generic names only`);
  ok(html.includes('<base href="../">') && /<b>MODELLED\.<\/b> <b>Under the stated public assumptions, not a statement about the performance of any real system\.<\/b> <b>Not affiliated with or endorsed by any agency\.<\/b>/.test(html), "lunar/index.html resolves from the Studio folder and carries the honesty block");
}
ok(read("geometry/index.html").includes('<base href="../">') && read("geometry/index.html").includes("Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.") && /representative sky, not the sky of that date/i.test(read("geometry/index.html")), "geometry/index.html resolves from the Studio folder and carries the advisory statement and the representative-sky notice");

// 4. names and pricing
for (const f of [...own, "PARITY.md", "tokens.css"]) {
  const t = read(f);
  ok(!/aperio/i.test(t), `${f}: names Aperio`);
  ok(!/\bclaude\b|anthropic/i.test(t), `${f}: names an AI vendor`);
  ok(!/[€$£]\s?\d|\bprice\b|\bpricing\b|per seat|\/month/i.test(t), `${f}: mentions a price`);
}
ok(/mailto:contact@ashforde\.org/.test(read("index.html")), "Pro call to action is a mailto");

// 5. catalogue numbers
const files = readdirSync(here + "scenarios").filter((f) => f.endsWith(".toml"));
const catalog = read("lib/catalog.mjs");
const listed = [...catalog.matchAll(/^\s*\["([a-z0-9.-]+\.toml)",/gm)].map((m) => m[1]);
ok(listed.length === files.length, `library lists ${listed.length}, disk has ${files.length}`);
ok(parity.includes(`all ${files.length} bundled files`), `PARITY states the file count (${files.length})`);

// 6. recorded fallback
const idx = JSON.parse(read("recorded/index.json"));
const runnable = Object.values(idx.runs).filter((r) => !r.error).length;
// The recordings must be the package's own: re-record (tools_native.mjs, then tools_record.mjs) after a package swap.
const pkgVersion = JSON.parse(read("pkg/package.json")).version;
ok(idx.engine_version === pkgVersion, `the recorded runs are engine ${idx.engine_version}, the package is ${pkgVersion}`);
// The install commands the Studio shows (the site build overwrites this file with its own copy).
ok(JSON.parse(read("channels.json")).version === pkgVersion, `channels.json is for ${JSON.parse(read("channels.json")).version}, the package is ${pkgVersion}`);
ok(/url\(assets\/brand\/kshana-mark-mask\.png\)/.test(read("index.html")) && readdirSync(here + "assets/brand").includes("kshana-mark-mask.png"), "the brand mark ships inside the Studio folder");
ok(JSON.parse(read("native/index.json")).engine_version === pkgVersion, `the native recordings are engine ${JSON.parse(read("native/index.json")).engine_version}, the package is ${pkgVersion}`);
// The commit stamp (selfcheck.mjs --final, the release gate): both recordings name the same full engine SHA.
{
  const natIdx = JSON.parse(read("native/index.json"));
  const stamped = /^[0-9a-f]{40}$/.test(idx.engine_commit || "") && idx.engine_commit === natIdx.engine_commit;
  if (process.argv.includes("--final")) ok(stamped, `recorded/index.json engine_commit ${idx.engine_commit} and native/index.json engine_commit ${natIdx.engine_commit} must be the same full SHA`);
  else if (!stamped) console.log("note: recordings carry no matching engine_commit (required by --final)");
}
ok(parity.includes(`${runnable} of ${files.length} scenarios`), `PARITY states recorded coverage (${runnable} of ${files.length})`);
for (const [f, r] of Object.entries(idx.runs)) if (!r.error) { const rec = JSON.parse(read("recorded/" + r.file)); ok(rec.toml === read("scenarios/" + f), `recorded ${f} matches the bundled scenario text`); }

ok(Object.entries(idx.runs).filter(([, r]) => r.source === "native").every(([f]) => /RECORDED_NATIVELY = \[[^\]]*"/.test(catalog) && catalog.includes(`"${f}"`)), "a natively recorded run is listed as such");

// 7. native recordings and the optional group
const nat = JSON.parse(read("native/index.json"));
const single = files.filter((f) => !f.endsWith(".suite.toml"));
ok(single.every((f) => nat.runs[f] && !nat.runs[f].error), "every bundled scenario has a native recording");
ok(single.every((f) => nat.runs[f] && nat.runs[f].report), "every bundled scenario has the engine's report");
for (const f of ["index.html", "app.css", "studio.css", "lib/catalog.mjs", "lib/kinds.mjs", "lib/stages.mjs", "lib/views.mjs", "lib/meaning.mjs", "lib/inputs.mjs", "lib/urlstate.mjs", "lib/areas.mjs", "lib/meaning-kinds.mjs", "data/areas.json", "advanced/index.html", "advanced.js", "PARITY.md"]) ok(!/celeste/i.test(read(f)), `${f}: names the optional group (it must live only in its own folder and the loader)`);
for (const f of files) ok(!/=\s*"[^"]*celeste/i.test(read("scenarios/" + f)), `scenarios/${f}: uses the optional preset`);
ok(!/celeste/i.test(read("app.js")), "app.js: names the optional group (groups.json is the only place that lists it)");
// The Advanced view shares the engine, the worker, the modules and the data: no second copy.
const adv = read("advanced/index.html");
ok(/<base href="\.\.\/">/.test(adv) && /src="advanced\.js"/.test(adv), "advanced/index.html resolves from the Studio folder and runs advanced.js");
ok(!readdirSync(here + "advanced").some((f) => /^(pkg|lib|recorded|scenarios|data|native)$/.test(f) || f.endsWith(".mjs") || f.endsWith(".wasm")), "advanced/ holds no copy of the engine, modules or data");
ok(read("advanced.js").includes('new URL("./pkg/kshana_bg.wasm", import.meta.url)') && read("advanced.js").includes('new URL("./lib/engine-worker.mjs", import.meta.url)'), "advanced.js loads the one engine and worker from the Studio folder");
ok(!/Kshana Pro</.test(adv.split("</header>")[0]) && !/Kshana Pro</.test(read("index.html").split("</header>")[0]), "no Kshana Pro link in either header (footer and For researchers only)");
for (const g of JSON.parse(read("groups.json")).groups) { const gi = JSON.parse(read(g + "index.json")); ok(gi.scenarios.every((x) => readdirSync(here + g).includes(x[0])), `${g}: every listed scenario file is in the folder`); ok(Object.keys(JSON.parse(read(g + "recorded/index.json")).runs).length === gi.scenarios.length, `${g}: every scenario of the group is recorded`); }
console.log(`native recordings: ${Object.keys(nat.runs).length}; with an animation ${Object.values(nat.runs).filter((r) => r.animation).length}; with an export ${Object.values(nat.runs).filter((r) => (r.exports || []).length).length}`);


// 8. no recorded or native record shows a Kshana version other than the package's (a stale record would
// otherwise carry an old engine's name in its chart text, report footer or export header)
{
  const { gunzipSync } = await import("node:zlib");
  const stale = [];
  const scan = (name, text) => { for (const m of text.matchAll(/Kshana,? v?(\d+\.\d+\.\d+)/g)) if (m[1] !== pkgVersion) { stale.push(`${name}: ${m[0]}`); break; } };
  for (const dir of ["native", "celeste/native"]) if (existsSync(here + dir)) for (const f of readdirSync(here + dir).filter((x) => x.endsWith(".gz"))) scan(`${dir}/${f}`, gunzipSync(readFileSync(here + dir + "/" + f)).toString("utf8"));
  for (const dir of ["recorded", "celeste/recorded"]) if (existsSync(here + dir)) for (const f of readdirSync(here + dir).filter((x) => x.endsWith(".json") && x !== "index.json")) scan(`${dir}/${f}`, readFileSync(here + dir + "/" + f, "utf8"));
  ok(stale.length === 0, `records that name another Kshana version than ${pkgVersion}: ${stale.slice(0, 5).join("; ")}${stale.length > 5 ? ` (+${stale.length - 5} more)` : ""}`);
}

console.log(fails ? `SELFCHECK: ${fails} failure(s)` : "SELFCHECK: all checks passed");
process.exit(fails ? 1 : 0);
