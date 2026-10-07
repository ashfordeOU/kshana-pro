# Re-port runbook: rebuild kshana.dev at a release-bump commit (Linux, cloud container)

Use this at every engine release bump (first: 0.33.0) so the whole site — the changelog
page, every page's version chip and footer, `llms*.txt`, the Studio's recordings and the
run-provenance stamps — moves to the new version in one pass. It condenses
`ASSEMBLY-RUNBOOK.md` (steps 1–7) and `PORT-RUNBOOK.md` for a Linux container, with
what was proven here on 2026-10-07 marked. Those two files stay the detailed reference.

**The canonical site source is this folder, `site-next/` at the kshana-pro root**
(vendored 2026-10-04). `handover/open-core-0.30/site-next/` is an older copy; do not build
from it.

## What is generated from what

- `web/docs/changelog.html` is **generated**, never hand-edited: `tools/build_docs.mjs`
  (run by `build.py`) renders the public repo's `CHANGELOG.md` from `git show HEAD:` of the
  docs checkout. A passage naming an AI vendor is replaced by a pointer to the file.
- Every page, `llms.txt`, `llms-full.txt`, `channels.json`, `sitemap.xml` and the Studio copy
  are written by `explorations/2026-09-27-combo/build.py` from `src/` plus the engine
  checkout. They are copied into the public repo's `web/` by its `web/tools/port_site.py`,
  which also rewrites `web/PORT-MANIFEST.json` (`web/site.test.mjs` checks every SHA).

## Proven in a cloud container on 2026-10-07 (and what was not)

| Piece | Result |
|---|---|
| `cd site-next/tools && npm ci` (marked 12.0.2, marked-gfm-heading-id 3.1.3, highlight.js 11.9.0) | OK, 1.8 s, Node 22.22 |
| Python | 3.13 works (`build.py` ran to its gates in ~1.4 s: 42 charts from 25 recordings, 27 SVG renders) |
| Playwright + Chromium | OK **only with** `executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"`; Playwright 1.63 asks for a headless shell that is not installed |
| `features/check_nav.mjs` | **Will not launch as written**: it calls `chromium.launch({ channel: "chrome" })` (Google Chrome, macOS). Needs an `executablePath` override on Linux |
| Engine release build, recorders, Studio re-record, full `build.py`, `check.py`, port | **Rehearsed end to end on 2026-10-07** at public `bd05a276` (`main` + the `server.json` fix of public PR #37), in another session whose policy allows the release build: every step green after the fixes folded into the steps below. See "Rehearsal record" at the end |
| `build.py` gate 1 | Refuses on public `main` today: `server.json image ghcr.io/ashfordeou/kshana-mcp:0.31.0 is not tagged 0.32.0`. **`mcp/kshana-mcp/server.json` must carry the bump version before building** |
| `build.py` gate 2 | `recorded runs are not from the site's engine checkout … re-run:` the seven recorders below (they were recorded at a pre-squash commit not on `main`, so they must be re-run from the bump commit) |
| `build.py` gate 3 | `recorded runs are engine 0.31.0, the site is 0.32.0` (Studio recordings and package) |

Measured disk: a release `target/` and the Studio work need several GB; keep at least 15 GB free.

## Inputs

```sh
T=/path/to/kshana          # public checkout AT the release-bump commit, committed (docs render from HEAD),
                           # with CHANGELOG [X.Y.Z], every version-sync surface AND mcp/kshana-mcp/server.json bumped
P=/path/to/kshana-pro      # this repo, a branch you will commit the refreshed recordings to
SN=$P/site-next
C=$SN/explorations/2026-09-27-combo          # the site
S=$SN/explorations/2026-10-01-studio-rebuild # the Studio (src/data/sources.json: studio)
export KSHANA_REPO=$T KSHANA_DOCS_REPO=$T KSHANA_ENGINE_TREE=$T
```

`src/data/sources.json` says `~/Code/kshana`; the three variables override it, so no edit is
needed. `build.py` also reads the public Pro wording from `src/data/pro-power.json`'s
`public_text.checkout` (`~/Code/kshana-wt-pro-public`) at a fixed public commit; without that path
it fails with `Pro wording is not in the public docs/PRO.md or README.md`. The commit is in the
public history, so point the path at T (which must have full history, not a shallow clone):
`mkdir -p ~/Code && ln -sfn $T ~/Code/kshana-wt-pro-public`. Use `python3` (3.10+); the `/opt/homebrew/bin/python3` in older notes is macOS only.

## 1. Build the engine and the WebAssembly package at T

```sh
cd $T && cargo build --release --locked --bin kshana                 # recorders run target/release/kshana
cd $T/mcp/kshana-mcp && cargo build --release --locked        # standalone crate (workspace-excluded); only if the MCP tool list changed (PR 34 adds IQ tools)
cd $T && ./web/build.sh                                              # wasm-pack build into web/pkg (needs wasm-pack + wasm32 target)
```

Toolchain for the WebAssembly build, once per container (rehearsal-proven):

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.93.0          # the pinned toolchain, not the default one
cargo install wasm-pack --version 0.13.1 --locked                    # 97 s; built from source, no downloaded binary
# wasm-pack's own download of binaryen (wasm-opt) fails behind the container's TLS-re-terminating
# proxy ("failed to download ... binaryen-version_117"); fetch the same version with curl, check it
# against the checksum binaryen publishes, and put it on PATH before ./web/build.sh:
B=$HOME/binaryen && mkdir -p $B && cd $B
U=https://github.com/WebAssembly/binaryen/releases/download/version_117/binaryen-version_117-x86_64-linux.tar.gz
curl -sSLO $U && curl -sSLO $U.sha256 && sha256sum -c binaryen-version_117-x86_64-linux.tar.gz.sha256 && tar xzf binaryen-version_117-x86_64-linux.tar.gz
export PATH=$B/binaryen-version_117/bin:$PATH
```

The container-built `kshana_bg.wasm` is not byte-identical to the one CI publishes: it embeds the
Cargo registry path (`/root/.cargo/...` here, `/home/runner/.cargo/...` on CI), about 340 bytes.
The Pages workflow compiles its own, so this only affects the copy committed to the Studio's `pkg/`.

`gen_evidence_data.py` also needs the Cs 5071A record: `cd $T && scripts/fetch_cs5071a.sh`
(it looks in `$T/realdata-cache/cs5071a/`, then `~/Code/kshana`). `gen_publications.py`
builds release examples itself with `CARGO_BUILD_JOBS=6`.

## 2. Install the package into the Studio and re-record it (ASSEMBLY-RUNBOOK steps 1–3)

```sh
sha256sum $T/web/pkg/kshana_bg.wasm                                   # record it in the PR body
for f in kshana.js kshana_bg.wasm kshana.d.ts package.json LICENSE; do cp $T/web/pkg/$f $S/pkg/$f; done
# The five Celeste files live in $S/celeste/, not $S/scenarios/ (selfcheck fails "uses the optional
# preset" if they land there). Update the files the Studio has; list, never copy, a new one: it needs
# a library place first.
for f in $T/scenarios/*.toml; do b=$(basename $f)
  if   [ -f $S/scenarios/$b ]; then cmp -s $f $S/scenarios/$b || cp $f $S/scenarios/
  elif [ -f $S/celeste/$b ];   then cmp -s $f $S/celeste/$b   || cp $f $S/celeste/
  else echo "NEW, not copied: $b"; fi; done
# The Studio's channels.json is hand-kept: every install line names the version (8 strings).
sed -i "s/OLD_VERSION/NEW_VERSION/g" $S/channels.json            # e.g. 0\.32\.0 -> 0.33.0
cd $S && node tools_native.mjs --repo $T --bin $T/target/release/kshana --work /tmp/native-work --jobs 4
cd $S && node tools_record.mjs && node selfcheck.mjs                  # SELFCHECK: all checks passed
cd $S && node tools_bundle.mjs
cp $T/web/capabilities.json $S/data/ && for f in verification-matrix card-matrix-map oracle-references standards-matrix-map; do cp $T/web/data/$f.json $S/data/; done
cd $S && node tools_areas.mjs && node tools_bundle.mjs && node selfcheck.mjs   # data/areas.json + celeste/areas.json carry engine_version: check.py rule 17
```

`selfcheck.mjs` fails if the recordings and the package are different versions. It counts the
module tests from Node's spec reporter; off a terminal (`node --test` writes TAP there) it read
`undefined` and failed with every test green, so it now asks for `--test-reporter=spec`.

## 3. Re-run the site's recorders (ASSEMBLY-RUNBOOK step 5)

```sh
cd $C
python3 src/tools/record_runs.py
# gen_capabilities compares with assets/runs/, which build.py copies from src/data/runs/ later; mirror that copy now:
python3 -c "
import json,os,shutil
for n in json.load(open('src/data/runs/index.json'))['runs']:
    d=os.path.join('assets','runs',n); os.makedirs(d,exist_ok=True)
    shutil.copyfile(f'src/data/runs/{n}/scenario.toml',f'{d}/{n}.toml.txt')
    for f in ('result.json','report.html'): shutil.copyfile(f'src/data/runs/{n}/{f}',f'{d}/{f}')"
for t in gen_waterfall gen_capabilities gen_missions_runs record_cli_session gen_evidence_data gen_publications; do
  python3 src/tools/$t.py || break
done
node src/tools/gen_footprint.mjs && node src/tools/gen_footprint.mjs --check   # the eighth recorder: Missions footprint, run with the Studio's pkg
python3 src/tools/gen_missions_page.py && python3 src/tools/gen_missions_page.py --check
# MCP tool list changed (0.33.0: PR 34)? re-record the session and the Developers data:
python3 $SN/tools/record_mcp_session.py $T/mcp/kshana-mcp/target/release/kshana-mcp $T/scenarios/clock-holdover.toml
python3 src/tools/gen_developers_data.py
```

New kinds, tools or scenario files also need a row in `$SN/features/placements.toml`
(`python3 $SN/features/check_features.py` names each as UNPLACED or UNREGISTERED; run it after
`build.py`, it reads the built pages). At 0.32.0 eleven are UNPLACED: `cli:iq`, the eight
`python.iq_*`/`python.receiver_trust` bindings, `wasm.receiver_trust` and the MCP tool
`assess_receiver_log`. Placing them is an editorial decision; 0.33.0 adds PR 34's MCP IQ tools.

**Every recorder records `git rev-parse HEAD` of T, and `build.py` refuses recordings from any other
commit.** So everything that changes T (the bump itself, the docs prose below, the README images)
must be committed before step 2, and nothing may be committed to T between step 2 and the port.

## 4. Build and check the site

```sh
cd $SN/tools && npm ci
cd $C && python3 build.py          # last line: … N files to publish, M MB
cd $C && python3 check.py          # OK: N pages …  (rule 6: every chart's engine version == Cargo.toml's)
cd $C && python3 check.py --mutate # every mutation RED, unmutated green
cd $SN/features && python3 check_features.py
```

`check.py` rule 17 fails on any public page text that names another engine version, so the bump
commit must also move these (rehearsal at 0.32.0 found them at 0.31.0): the `kshana X` / `engine X`
lines in `docs/ANIMATION.md`, `docs/CAPABILITY.md`, `docs/CONSTELLATION-DESIGN.md`,
`docs/INTEGRITY.md` ("Today (vX)"), `docs/LEO-PASS.md`, `docs/LEO-PNT-FUSION.md`,
`docs/SOLAR-SYSTEM.md`, `docs/examples/multi-tool-pipeline.md`, `docs/tutorials/README.md`, and the
README image titles, regenerated with `KSHANA_BIN=$T/target/release/kshana python3
tools/gen_readme_assets.py` (7 s) plus the README alt texts that quote them. Several of those lines
quote figures measured with that version: re-run them, do not only retype the version.

Browser link check (rehearsal: 779 s). `check_nav.mjs` takes Chromium from `KSHANA_CHROME`
(otherwise macOS Chrome) and Playwright from `KSHANA_PLAYWRIGHT`; keep localhost off the proxy:

```sh
mkdir -p ~/pw && cd ~/pw && npm init -y >/dev/null && PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm i playwright@1.60
python3 $SN/features/serve.py $C 8847 &
NO_PROXY=127.0.0.1,localhost KSHANA_CHROME=/opt/pw-browsers/chromium-1194/chrome-linux/chrome \
  KSHANA_PLAYWRIGHT=~/pw/node_modules python3 $C/check.py --nav
```

A changed number on a page is a revision: keep the previous `.build/viz.json` and
`data/series.json` and diff them, and say what moved in the PR body.

## 5. Port into the public repo, inside the release-bump branch

```sh
cd $T
port() { python3 web/tools/port_site.py --site $C --studio $S "$@"; }
port --check ; port ; port --check                     # last: OK: web/ matches the port
KSHANA_WASM_PKG=$S/pkg ./web/build.sh                  # stage the package, no compile
node web/site.test.mjs && node web/legacy-urls.test.mjs && node web/smoke.mjs
for f in web/studio/lib/*.test.mjs; do node "$f" >/dev/null 2>&1 && echo "ok   $f" || echo "FAIL $f"; done   # each also needs a step in ci.yml
scripts/check-version-sync.sh && scripts/check-no-attribution.sh
grep -c 'id="XYZ0---' web/docs/changelog.html   # the new release heading (e.g. 0330---2026-10-0D) is present
grep -rl '<small>vOLD</small>' web --include=*.html # empty: no page left on the old version chip
```

The port refuses a site whose version is not `Cargo.toml`'s. Commit `web/` in the bump
branch: author `Ashforde OÜ <ashfordegithub@cbaweja.org>`, plain message, no trailers.

## 6. Commit the refreshed sources back to kshana-pro

Steps 2–3 rewrite tracked files in `$S` (package, scenarios, recordings, data) and in
`$C/assets/` and `$C/src/` (recorder outputs). Build outputs are ignored by
`site-next/.gitignore`. Commit the tracked changes to a kshana-pro branch and open a PR,
so the next re-port starts from this release's recordings.

## Known gaps to close (not done here)

- `features/check_nav.mjs` now reads `KSHANA_CHROME`; the `scratch/assembly/*.mjs` browser checks
  still assume macOS Chrome.
- `gen_capabilities.py` should compare with `src/data/runs/` (the source) rather than `assets/runs/`
  (a build output), which would make the mirror step in section 3 unnecessary.
- `pro-power.json`'s public-text checkout should take an environment override, like `_sources.py`.
- `gen_pro_power.py` reads `~/Code/kshana-site-next/scratch/pro` and `~/Code/kshana-pro/...`
  (override with `KSHANA_PRO_SHOWCASE` / `KSHANA_PRO_TEXT_REPO`); it is not in the
  recorder gate list, so it only matters if the Pro page changes.

## Rehearsal record (2026-10-07, 0.32.0 at public `bd05a276`, nothing committed to `web/`)

A full pass in a cloud container, at the 0.32.0 version public `main` already carries, to make
release day a pure re-run. T was public `bd05a276` (`main` + public PR #37) plus one local commit,
never pushed, that bumped the docs prose of section 4 to 0.32.0 and regenerated the README images.

| Step | Seconds | Result |
|---|---|---|
| `cargo install wasm-pack 0.13.1` | 97 | OK |
| engine release build (cold) | 352 | OK |
| `kshana-mcp` release build | 169 | OK |
| `scripts/fetch_cs5071a.sh` | 1 | OK |
| `./web/build.sh` | 359, then 266 | failed on the binaryen download; OK with binaryen 117 on PATH (section 1) |
| Studio: copy, `tools_native` (143 records), `tools_record` + `selfcheck`, bundle, areas | 43 + 48 + 0 + 2 | OK after the section 2 fixes (Celeste routing, `channels.json`, reporter, `tools_areas`) |
| recorders: `record_runs`, mirror, `gen_waterfall`, `gen_capabilities`, `gen_missions_runs`, `record_cli_session`, `gen_evidence_data`, `gen_publications`, `gen_footprint` | 0, 0, 1, 2, 14, 0, 140 cold / 3, 337 cold / 24, 6 | OK after the mirror step and the eighth recorder |
| `record_mcp_session` + `gen_developers_data` | 0 | OK (0.32.0 server; re-record at 0.33.0 after PR 34) |
| `build.py` | 12 | OK after the public-text symlink |
| `check.py` | 12 | OK after the section 4 docs prose |
| `check_features.py` | 21 | UNPLACED=11 only (editorial, above) |
| `check.py --mutate` | 522 | OK |
| `check.py --nav` | 779 | Not conclusive in this container. Two runs of the rehearsal site gave 16 and 17 BAD of about 2088 and shared only 4 of them; the committed 0.32.0 `web/` of public `main`, served the same way, gave 4 BAD. Most entries are timeouts ("did not navigate", a section still "hidden" at 2500 ms). Recurring in both rehearsal runs: desktop `/` link to `/editions`, and `/capabilities` `#constellations` (phone and desktop) and `#campaigns` (desktop; also in the baseline). Look at those before release; treat the rest as noise until a quieter machine says otherwise |
| port `--check`, port, port `--check` | 4 | `OK: web/ matches the port (649 files, v0.32.0)` |
| `KSHANA_WASM_PKG=$S/pkg ./web/build.sh` | 0 | OK |
| `site.test.mjs`, `legacy-urls.test.mjs`, `smoke.mjs`, `studio/lib/*.test.mjs` | 9 | OK |
| `KSHANA_VERSION_SYNC_SITE=strict scripts/check-version-sync.sh`, `check-no-attribution.sh` | 1 | OK, no WARN line: no page left on `<small>v0.31.0</small>`, changelog has `[0.32.0]` |

The port rewrote 400 files under `web/` (+2322/-2258 lines): 229 in `studio/`, 62 in `docs/`, 19 in
`assets/`, plus `index.html`, `missions.html`, `llms*.txt`, `sitemap.xml`, `js/` and `data/`. At a
0.33.0 bump expect at least that, plus the Studio recordings and the PR 34 MCP session.
