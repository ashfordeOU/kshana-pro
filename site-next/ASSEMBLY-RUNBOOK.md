# Assembly runbook: a new engine package into the Studio, the site and the port

Written 2026-09-30 after the Studio swap. Every step below was run that day with the
corrected WebAssembly (browser build) package of `feat/wasm-parity`. Nothing here pushes,
tags or touches the live kshana.dev.

Python is `/opt/homebrew/bin/python3` (the shell's `python3` may be 3.9 and fails).
The machine is usually loaded: no step here compiles anything.

## Where things are

| Thing | Path |
|---|---|
| The Studio the site is built with (the staging copy; the site's build input) | `explorations/2026-09-30-studio-next` |
| The previous Studio, untouched, the fallback | `explorations/2026-09-27-playground` |
| The site (source `src/`, `build.py`, `check.py`; built in place) | `explorations/2026-09-27-combo` |
| What the site reads: the engine checkout, the Studio folder, the checkout of the assistant-server tool list | `explorations/2026-09-27-combo/src/data/sources.json` (`repo`, `docs_repo`, `studio`, `mcp_repo`) |
| Backup taken before the swap (site, old Studio, staging Studio, feature placements) | `explorations/2026-09-27-audit/backup-studio-swap/` |
| Browser checks and their reports | `scratch/assembly/` (`prov_audit.mjs`, `studio_links_check.mjs`, `compare_results.py`, `studio_smoke.mjs`, `reports/`, `shots/`) |
| The port | `~/Code/kshana-wt-site` (branch `feat/site-redesign`), `PORT-RUNBOOK.md` |

To go back to the previous Studio: set `"studio": "../2026-09-27-playground"` in
`sources.json` (or `KSHANA_STUDIO=../2026-09-27-playground`) and rebuild. The page scripts
then need the old loose-file reader: restore the site from the backup instead if that is the aim.

## The one rule that decides how much to redo

The site draws two kinds of numbers:

- **from the Studio's recordings** (42 charts, the Missions sector charts, the six failure
  cards): these follow the package installed in the Studio. Steps 1 to 4.
- **from the native engine, run by the recorders in `src/tools/`** (Home hero and L-band,
  Missions engine-run cards and orbit console, Capabilities runs, Evidence, the command-line
  session): these follow the engine checkout named in `sources.json: repo`. `build.py`
  refuses to build when a recorder's output is from another commit, and names the recorder.
  Step 5.

A package built from a commit that is NOT the site's engine checkout leaves the two kinds
on different engines. That is the state on 2026-09-30: the Studio runs the corrected package
(`feat/wasm-parity` 4fb3791) and the site's recorders ran `feat/docs-v29` 382619f. One
figure differs today because of it: the Missions card `leo-navmsg-fit-interval-trade` shows
1.6 mm and 40.3 mm (worst range error at 60 s and 900 s fit); the corrected engine, and so
the Studio, gives 1.8 mm and 46.6 mm. `scratch/assembly/compare_results.py` lists every such
difference. It closes at step 5, when the site moves to the final tree.

## 1. Install a new package into the Studio

```sh
PKG=~/Code/kshana-site-next/scratch/wasm-parity/pkg        # the new wasm-pack package
S=~/Code/kshana-site-next/explorations/2026-09-30-studio-next
shasum -a 256 $PKG/kshana_bg.wasm                          # note it: it is what you installed
for f in kshana.js kshana_bg.wasm kshana.d.ts package.json LICENSE; do cp $PKG/$f $S/pkg/$f; done
```

Only those five files (a package's `README.md` and `kshana_bg.wasm.d.ts` are not used).

## 2. Re-record the Studio with the new package

The native recordings first (the browser recorder takes the one scenario the browser build
cannot run, `lunar-llr-datum`, from them), with a native engine **built from the same commit
as the package**:

```sh
cd $S
node tools_native.mjs --repo <engine checkout of that commit> --bin <its target/release/kshana> \
     --work ~/Code/kshana-site-next/scratch/assembly/native-work --jobs 4      # about 1 to 3 minutes
node tools_record.mjs                                                            # about 2 to 5 minutes
node selfcheck.mjs                                                               # SELFCHECK: all checks passed
```

Expect `138 native records; 0 failed` and `137 recorded from the browser build; 1 taken
from the native engine; 1 not runnable` (the one is the study suite file, which is not a
scenario). `selfcheck.mjs` fails if the recordings and the package are different versions.
On 2026-09-30 the binary was `scratch/wasm-parity/target/release/kshana` and the checkout
`~/Code/kshana-wt-wasm-parity`.

If the Studio's scenario files are older than the engine's, copy the engine's first
(`cp <checkout>/scenarios/<name>.toml $S/scenarios/`; the five Celeste files live in
`$S/celeste/`), then record. The port says which differ (`note: … Studio scenario file(s) differ`).

## 3. Rebundle

```sh
cd $S && node tools_bundle.mjs            # scenarios/index.json: every scenario file in one JSON
```

The packs of recordings are not kept in the Studio folder. The site build makes them when it
copies the Studio (`tools_bundle.mjs --to <site>/playground`: 12 packs of recorded runs,
20 of native recordings, one each per optional group), because only the preview host limits
the number of files. The Studio reads a pack when its index entry names one and the loose
file otherwise (`lib/packs.mjs`), so the staging copy and the ported kshana.dev serve loose
files and the preview serves packs.

## 4. Rebuild the site and check it

```sh
cd ~/Code/kshana-site-next/explorations/2026-09-27-combo
/opt/homebrew/bin/python3 build.py          # last line: … N files to publish, M MB
/opt/homebrew/bin/python3 check.py          # OK: … (rule 12: at most 255 files, 64 MB, 15 MB a file)
/opt/homebrew/bin/python3 check.py --mutate # every mutation RED (good), unmutated: green
cd ~/Code/kshana-site-next/features && /opt/homebrew/bin/python3 check_features.py    # PASS, 0 errors
```

On 2026-09-30: 217 files, 44.7 MB; 605 Studio links; `check_features`: 599 features,
NO_STUDIO 0.

Without the Celeste group (nothing is deleted; the build leaves the folder out and empties
the group list):

```sh
KSHANA_STUDIO_WITHHOLD=celeste/ /opt/homebrew/bin/python3 build.py && /opt/homebrew/bin/python3 check.py
```

(207 files, 43.9 MB.) To withhold it for good, delete `$S/celeste/` and its entry in
`$S/groups.json`; nothing else names it. Rebuild without the variable to put it back.

What to read in the output:

- `build.py` stops with `recorded runs are not from the site's engine checkout … re-run:` → step 5.
- `build.py` stops with `… has no <tab> view in the Studio` or `… is not in the Studio` → a page
  links a view the new recordings no longer have; fix the link in `src/pages/` or the generator.
- `build.py` stops with `the recorded MCP session lists N tools, the server source M` → step 5d.
- `js/developers-data.mjs is stale` → `/opt/homebrew/bin/python3 src/tools/gen_developers_data.py`.
- A changed number on a page is a revision (rule R4), never silent. Compare before rebuilding:
  keep `.build/viz.json` and `data/series.json` from the previous build and diff them (the
  commands are in `scratch/assembly/` history; on 2026-09-30 the corrected package changed no
  chart and no plotted series).

## 5. When the engine checkout changes (the final tree)

Do this once the final tree exists with a release binary at `<tree>/target/release/kshana`
and a package built from the same commit is installed (steps 1 to 3).

a. Point the site at it: in `src/data/sources.json` set `repo` and `docs_repo` to the tree;
   remove `mcp_repo` if the tree carries `feat/mcp-newkinds` (14 tools; count them with
   `grep -c '#\[tool(' <tree>/mcp/kshana-mcp/src/server.rs`).

b. Re-run the recorders `build.py` names, from the site folder. Each runs the tree's binary
   on the tree's scenario files in a scratch folder:

   ```sh
   cd ~/Code/kshana-site-next/explorations/2026-09-27-combo
   for t in record_runs gen_waterfall gen_capabilities gen_missions_runs record_cli_session; do
     /opt/homebrew/bin/python3 src/tools/$t.py || break
   done
   /opt/homebrew/bin/python3 src/tools/gen_evidence_data.py      # builds a small probe with cargo: run it when the machine allows
   /opt/homebrew/bin/python3 src/tools/gen_missions_page.py      # then --check must exit 0
   ```

c. The Studio's data folder takes the tree's generated files (the build copies them into the
   published copy anyway; this keeps the staging copy telling the same counts):

   ```sh
   T=<tree>; cp $T/web/capabilities.json $S/data/ && for f in verification-matrix card-matrix-map oracle-references standards-matrix-map; do cp $T/web/data/$f.json $S/data/; done
   ```

d. If the assistant server's tool list changed, record the session again with a server binary
   built from the tree, then regenerate the Developers data:

   ```sh
   /opt/homebrew/bin/python3 ~/Code/kshana-site-next/tools/record_mcp_session.py <tree>/mcp/kshana-mcp/target/release/kshana-mcp <tree>/scenarios/clock-holdover.toml
   /opt/homebrew/bin/python3 src/tools/gen_developers_data.py
   ```

e. New tools, kinds or scenario files need a row in `features/placements.toml`
   (`check_features.py` names each one as UNPLACED or UNREGISTERED).

f. A version bump (0.29.0) needs a package of that version in the Studio (steps 1 to 3): rule 6
   of `check.py` requires every chart's engine version to equal `Cargo.toml`'s.

g. Step 4, then list every number that changed against the previous build.

## 6. Browser checks (the site must be served: `python3 -m http.server 8841 --bind 127.0.0.1` from the site folder)

```sh
cd ~/Code/kshana-site-next/scratch/assembly
node prov_audit.mjs              # every provenance line that names a scenario has its Studio link (rendered pages, every tab opened)
node studio_links_check.mjs      # follows every Studio link: scenario, view, run, console, network; about 10 minutes
/opt/homebrew/bin/python3 compare_results.py   # the site's published engine results against the Studio's recordings
node studio_smoke.mjs yes        # the Studio boots, runs and reads a pack; "no" when Celeste is withheld
cd ~/Code/kshana-site-next/explorations/2026-09-27-audit/matrix && node run_matrix.mjs --quick
```

Reports land in `scratch/assembly/reports/` (`studio-links.md` is the link table) and
`matrix/out/quick-<stamp>/`.

## 7. The port

`PORT-RUNBOOK.md` has the full list of checks. The short form, with the new Studio:

```sh
cd ~/Code/kshana-wt-site
port() { /opt/homebrew/bin/python3 web/tools/port_site.py --site ~/Code/kshana-site-next/explorations/2026-09-27-combo --studio ~/Code/kshana-site-next/explorations/2026-09-30-studio-next "$@"; }
port --check        # what would change
port                # ported vX.Y.Z: … N written, M removed
port --check        # OK: web/ matches the port
KSHANA_WASM_PKG=~/Code/kshana-site-next/explorations/2026-09-30-studio-next/pkg ./web/build.sh   # stage the package, no compile
node web/site.test.mjs && node web/legacy-urls.test.mjs && node web/smoke.mjs
for f in web/playground/lib/*.test.mjs; do node "$f" >/dev/null 2>&1 && echo "ok   $f" || echo "FAIL $f"; done
diff <(find web -name '*.test.mjs' -not -path 'web/pkg/*' -not -path 'web/playground/pkg/*' | sort) \
     <(grep -oE '^ +run: node web/[A-Za-z0-9_./-]+\.test\.mjs$' .github/workflows/ci.yml | awk '{print $3}' | sort -u)   # empty: every test has a workflow step
git add -A web .github/workflows/ci.yml && scripts/check-no-attribution.sh
git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com commit -m "site: port the new Studio and the assembled site into web/"
```

The port takes the Studio folder itself, so kshana.dev serves one file per recording (no
packs); both layouts work there because the reader follows the index. A new Studio
`lib/*.test.mjs` file needs a step in `.github/workflows/ci.yml` (the `diff` above names it).
No trailers, no push.

## What is still to do after this runbook's first run (2026-09-30)

- Move the site to the final engine tree (step 5). Until then the site's native-engine
  figures are from `feat/docs-v29` 382619f and the Studio runs `feat/wasm-parity` 4fb3791.
- Version 0.29.0: a package of that version, then steps 1 to 5.
- The port was rehearsed with the site as built from `feat/docs-v29`; the final port builds
  the site from the worktree it ports into (`PORT-RUNBOOK.md`, section 0), which needs step 5
  for that worktree.
- The Rust documentation-sync tests that read `web/` (`PORT-RUNBOOK.md` 2e) were not run on
  2026-09-30 (no compiling while the release gate was running).

## 8. Swap in the dashboard Studio (studio-dash), written 2026-09-30 night; NOT performed

The dashboard Studio (start screen, five steps, key-figure strip, panel grid, parameters drawer,
field finder, phone step bar) is the staging copy `explorations/2026-09-30-studio-dash`. It is
`studio-next` with a new page layout (`index.html`, `dash.css`, `app.js`) and two new modules
(`lib/kpi.mjs`, `lib/finder.mjs`, each with a `*.test.mjs`). Same package, same recordings, same
scenario files, same deep links (`tools_links.mjs` output is identical to studio-next's). The
verification of 2026-09-30 is in `scratch/studio-dash/` (`test/results.json`, `logs/`, `shots/`).
A rehearsal build of a copy of the site with it is in `scratch/studio-dash/combo-trial/`
(`logs/trial-build.log`, `logs/trial-check.log`).

If a new package or re-recording landed in studio-next after 2026-09-30 23:00, carry it over first
(the dash copy has no other changes under `pkg/`, `recorded/`, `native/`, `scenarios/`, `celeste/`, `data/`):

```sh
N=~/Code/kshana-site-next/explorations/2026-09-30-studio-next
D=~/Code/kshana-site-next/explorations/2026-09-30-studio-dash
rsync -a --delete $N/pkg/ $D/pkg/ && rsync -a --delete $N/recorded/ $D/recorded/ && rsync -a --delete $N/native/ $D/native/
rsync -a --delete $N/scenarios/ $D/scenarios/ && rsync -a --delete $N/celeste/ $D/celeste/ && rsync -a $N/data/ $D/data/
cd $D && node selfcheck.mjs && for f in lib/*.test.mjs; do node "$f" >/dev/null 2>&1 && echo "ok   $f" || echo "FAIL $f"; done
```

Then:

```sh
S=~/Code/kshana-site-next/explorations/2026-09-30-studio-dash
cd $S && node tools_bundle.mjs                                   # 1. rebundle scenarios/index.json
cd ~/Code/kshana-site-next/explorations/2026-09-27-combo
cp src/data/sources.json ../2026-09-27-audit/backup-studio-swap/sources.before-dash.json
# 2. point the site at it: in src/data/sources.json set  "studio": "../2026-09-30-studio-dash"
/opt/homebrew/bin/python3 build.py                               # 3. rebuild (last line: N files to publish)
/opt/homebrew/bin/python3 check.py                               # 4. OK, rule 12 (file cap) and rule 16 (Studio links) included
/opt/homebrew/bin/python3 check.py --mutate
cd ~/Code/kshana-site-next/features && /opt/homebrew/bin/python3 check_features.py     # PASS, NO_STUDIO 0
KSHANA_STUDIO_WITHHOLD=celeste/ /opt/homebrew/bin/python3 ../explorations/2026-09-27-combo/build.py   # optional: Celeste withheld, then rebuild without it
cd ~/Code/kshana-site-next/scratch/assembly && node studio_smoke.mjs yes && node studio_links_check.mjs   # 5. browser checks, site on :8841
```

6. Port rerun, with the new Studio folder (section 7, `--studio` changes):

```sh
cd ~/Code/kshana-wt-site
port() { /opt/homebrew/bin/python3 web/tools/port_site.py --site ~/Code/kshana-site-next/explorations/2026-09-27-combo --studio ~/Code/kshana-site-next/explorations/2026-09-30-studio-dash "$@"; }
port --check && port && port --check
for f in web/playground/lib/*.test.mjs; do node "$f" >/dev/null 2>&1 && echo "ok   $f" || echo "FAIL $f"; done
diff <(find web -name '*.test.mjs' -not -path 'web/pkg/*' -not -path 'web/playground/pkg/*' | sort) \
     <(grep -oE '^ +run: node web/[A-Za-z0-9_./-]+\.test\.mjs$' .github/workflows/ci.yml | awk '{print $3}' | sort -u)
```

The `diff` WILL name two new files, `web/playground/lib/kpi.test.mjs` and
`web/playground/lib/finder.test.mjs`: add a `run: node web/playground/lib/kpi.test.mjs` and a
`run: node web/playground/lib/finder.test.mjs` step to `.github/workflows/ci.yml` beside the other
Studio test steps, rerun the `diff` (empty), then the web tests and smoke test of section 7, and commit
as there (ashfordeOU identity, no trailers, no push).

To go back: set `"studio": "../2026-09-30-studio-next"` in `sources.json`, rebuild, check, port again.

Notes for the swap:
- `index.html` still names the web-font and three.js hosts exactly as studio-next does; the port
  rewrites both to local files as before (nothing new is requested from a third-party host).
- build.py copies the whole folder minus tools and tests, so `dash.css`, `lib/kpi.mjs` and
  `lib/finder.mjs` ship: 3 more files than studio-next (217 → 220 in the preview build).
- Links: every `?scenario=…&tab=…` link opens the same scenario with that panel in focus (and, for a
  secondary view such as `report`, `animation`, `exports`, `json`, `sweep`, `orbit`, open on top of
  the grid). Old names `tab=fom`/`figures` map to `overview`, `chart` to `timeseries`. A link with no
  scenario, or `?start=1`, opens the start screen.

Rehearsal, 2026-09-30 23:25 (a copy of the site in `scratch/studio-dash/combo-trial/`, built with
`KSHANA_STUDIO=<studio-dash>`): `check.py` OK, 78 pages, 225 files, 46.0 MB, 602 Studio links, 102 run
visuals; `check.py --mutate` all red, unmutated green; `studio_smoke.mjs yes` on the trial served at
:8855 passes (runs, reads a pack, no console error). The trial's `build.py` had to skip one assertion
unrelated to the Studio (line 1576: "Pro hash … is not printed in the public docs/PRO.md", from the
Pro work in flight at that hour); the live site's build will stop there too until that is settled.
