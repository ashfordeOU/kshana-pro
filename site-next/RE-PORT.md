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
| Engine release build, recorders, Studio re-record, full `build.py`, `check.py`, port | **Not run here.** The release `cargo build` was refused by this session's permission policy, and every later step needs that binary |
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
needed. Use `python3` (3.10+); the `/opt/homebrew/bin/python3` in older notes is macOS only.

## 1. Build the engine and the WebAssembly package at T

```sh
cd $T && cargo build --release --locked --bin kshana                 # recorders run target/release/kshana
cd $T/mcp/kshana-mcp && cargo build --release --locked        # standalone crate (workspace-excluded); only if the MCP tool list changed (PR 34 adds IQ tools)
cd $T && ./web/build.sh                                              # wasm-pack build into web/pkg (needs wasm-pack + wasm32 target)
```

`gen_evidence_data.py` also needs the Cs 5071A record: `cd $T && scripts/fetch_cs5071a.sh`
(it looks in `$T/realdata-cache/cs5071a/`, then `~/Code/kshana`). `gen_publications.py`
builds release examples itself with `CARGO_BUILD_JOBS=6`.

## 2. Install the package into the Studio and re-record it (ASSEMBLY-RUNBOOK steps 1–3)

```sh
sha256sum $T/web/pkg/kshana_bg.wasm                                   # record it in the PR body
for f in kshana.js kshana_bg.wasm kshana.d.ts package.json LICENSE; do cp $T/web/pkg/$f $S/pkg/$f; done
for f in $T/scenarios/*.toml; do cmp -s "$f" "$S/scenarios/$(basename $f)" || cp "$f" $S/scenarios/; done
cd $S && node tools_native.mjs --repo $T --bin $T/target/release/kshana --work /tmp/native-work --jobs 4
cd $S && node tools_record.mjs && node selfcheck.mjs                  # SELFCHECK: all checks passed
cd $S && node tools_bundle.mjs
cp $T/web/capabilities.json $S/data/ && for f in verification-matrix card-matrix-map oracle-references standards-matrix-map; do cp $T/web/data/$f.json $S/data/; done
```

`selfcheck.mjs` fails if the recordings and the package are different versions.

## 3. Re-run the site's recorders (ASSEMBLY-RUNBOOK step 5)

```sh
cd $C
for t in record_runs gen_waterfall gen_capabilities gen_missions_runs record_cli_session gen_evidence_data gen_publications; do
  python3 src/tools/$t.py || break
done
python3 src/tools/gen_missions_page.py && python3 src/tools/gen_missions_page.py --check
# MCP tool list changed (0.33.0: PR 34)? re-record the session and the Developers data:
python3 $SN/tools/record_mcp_session.py $T/mcp/kshana-mcp/target/release/kshana-mcp $T/scenarios/clock-holdover.toml
python3 src/tools/gen_developers_data.py
```

New kinds, tools or scenario files also need a row in `$SN/features/placements.toml`
(`python3 $SN/features/check_features.py` names each as UNPLACED or UNREGISTERED).

## 4. Build and check the site

```sh
cd $SN/tools && npm ci
cd $C && python3 build.py          # last line: … N files to publish, M MB
cd $C && python3 check.py          # OK: N pages …  (rule 6: every chart's engine version == Cargo.toml's)
cd $C && python3 check.py --mutate # every mutation RED, unmutated green
cd $SN/features && python3 check_features.py
```

Optional browser link check (needs the `executablePath` fix above in `features/check_nav.mjs`):
`python3 $SN/features/serve.py $C 8847 & python3 $C/check.py --nav`.

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

- `features/check_nav.mjs` and the `scratch/assembly/*.mjs` browser checks assume macOS
  Chrome; add `executablePath` from an env var for Linux.
- `gen_pro_power.py` reads `~/Code/kshana-site-next/scratch/pro` and `~/Code/kshana-pro/...`
  (override with `KSHANA_PRO_SHOWCASE` / `KSHANA_PRO_TEXT_REPO`); it is not in the
  recorder gate list, so it only matters if the Pro page changes.
