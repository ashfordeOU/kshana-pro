# Sources: where the Kshana site, brand and report sources live

Written 2026-10-04, at engine release 0.31.0. This file maps every source the public site
kshana.dev is built from, and says how the public repository is refreshed from it.

Until this date the site source lived only in `~/Code/kshana-site-next` on the founder's Mac, so
no other machine could rebuild the site. It now lives here, in the private repository, and any
session that can clone `ashfordeOU/kshana-pro` can rebuild and re-port the site.

## The boundary

- **This repository (private, proprietary)** holds the *sources*: the site's page templates and
  build, the Kshana Studio source, the design tokens, the brand masters, the report theme notes and
  the Pro sample assets.
- **The public repository `ashfordeOU/kshana` (AGPL-3.0)** receives only the *ported output* in its
  `web/` folder (deployed to kshana.dev by `.github/workflows/pages.yml`). Its `web/PORT-MANIFEST.json`
  pins every ported file by SHA-256, and `web/site.test.mjs` fails if `web/` drifts from it.
- Nothing in this repository is copied into the public repository except through that port.
  The public engine stays AGPL-3.0 and never depends on anything here.

## Map

| Folder here | What it is | Where it ends up in the public repo |
|---|---|---|
| `site-next/` | The site source, canonical. The same layout as the Mac folder `~/Code/kshana-site-next`, so every runbook path still applies. Build outputs and `node_modules` are not tracked (`site-next/.gitignore`) | `web/` (via `web/tools/port_site.py`) |
| `site-next/explorations/2026-09-27-combo/` | The kshana.dev site: `src/` (page templates, CSS, JS, data, recorders in `src/tools/`), `build.py`, `check.py`, `assets/` (imagery, brand mask, Pro samples, and the recorders' engine runs) | `web/*.html`, `web/css/`, `web/js/`, `web/data/`, `web/assets/`, `web/docs/` |
| `site-next/explorations/2026-10-01-studio-rebuild/` | Kshana Studio (Simple and Advanced views), with its WebAssembly package `pkg/`, recordings `recorded/` and `native/`, and scenario copies | `web/studio/` |
| `site-next/tools/` | `build_docs.mjs` (Markdown to docs pages), `build_viz.mjs` (charts through the Studio's view code), `record_mcp_session.py` | used by `build.py` |
| `site-next/features/` | The feature register: `gen_features.py`, `check_features.py`, `placements.toml` | checks only |
| `site-next/scratch/{port,assembly,actlocal}/` | The browser and local-runner check scripts the runbooks call | checks only |
| `site-next/*.md` | `HANDOVER.md`, `GOAL.md` (tracker), `PORT-RUNBOOK.md`, `ASSEMBLY-RUNBOOK.md`, `RELEASE-RUNBOOK.md`, `LOCAL-RUNNERS.md` | — |
| `design-tokens/` | `tokens.css` and `theme.css`, as links to the single source in `site-next/.../src/`. Edit them there (or here; it is the same file) | `web/tokens.css`, `web/theme.css`, `web/studio/tokens.css` (the Studio copy differs only by its font path, `../fonts/`) |
| `brand/` | The Kshana brand masters: the founder's mark (SVG, PNG), wordmark, logo, banner, favicon, the mark's alpha mask (link), the home-screen icon (link), the social card (SVG source and PNG) and the OFL fonts (Unbounded, Geist, Geist Mono). `SHA256SUMS` lists them | `docs/assets/`, `web/favicon.svg`, `web/og-card.*`, `tools/readme-fonts/`, `web/assets/brand/` |
| `report-templates/` | `report-theme.css`, a mirror of the run report's stylesheet (the `STYLE` constant in the public `src/advanced_report.rs`, with its SHA-256), and `samples/`, the nine run reports kshana.dev publishes, as the 0.31.0 engine wrote them | the engine writes reports itself; the site publishes the recorded ones under `web/assets/` |
| `pro-assets/` | The Pro mission-dossier sample (PDF and two page images), as links to `site-next/.../assets/pro/` | `web/assets/pro/` |

Heavy binary media (`*.wasm`, images, PDFs, fonts) are stored with Git LFS (`.gitattributes`).
Run `git lfs install` once, then clone or pull as usual.

`handover/open-core-0.30/site-next/` is the 2026-10-01 snapshot made for the 0.30 cloud work. It
is history; `site-next/` here is the current source.

## Rebuild and re-port the public site from here

What a fresh machine needs: this repository (with LFS), the public repository, Python 3.11 or newer,
Node.js 20 or newer, a Rust toolchain and `wasm-pack` (only to re-record), Playwright with Chromium
(only for the browser checks and screenshots).

```sh
git clone https://github.com/ashfordeOU/kshana-pro.git ~/Code/kshana-pro      # this repo (git lfs install first)
git clone https://github.com/ashfordeOU/kshana.git ~/Code/kshana              # the public repo
cd ~/Code/kshana && git worktree add -b site/<topic> ~/Code/kshana-wt-site origin/main
cd ~/Code/kshana-pro/site-next/tools && npm ci                                 # the docs renderer's pinned packages
export KSHANA_REPO=~/Code/kshana-wt-site KSHANA_DOCS_REPO=~/Code/kshana-wt-site  # src/data/sources.json defaults to ~/Code/kshana
```

1. **New engine version:** build the release binary in the worktree (`cargo build --release --bin
   kshana`), and a WebAssembly package from the same commit (`wasm-pack build --target web --release
   -- --features wasm`). Install the package into the Studio and re-record it
   (`ASSEMBLY-RUNBOOK.md`, steps 1 to 3: `tools_native.mjs`, `tools_record.mjs`, `tools_areas.mjs`,
   `tools_bundle.mjs`, then `selfcheck.mjs` must pass).
2. **Re-run the site's recorders** at the worktree's committed HEAD (each refuses an unclean tree):
   `src/tools/record_runs.py`, `gen_waterfall.py`, `gen_capabilities.py`, `gen_missions_runs.py`,
   `record_cli_session.py`, `gen_publications.py`, `gen_evidence_data.py`, `gen_missions_page.py`,
   `gen_developers_data.py`, and `node src/tools/gen_footprint.mjs`. (`gen_capabilities.py`
   compares against `assets/runs/`, which `build.py` copies from `src/data/runs/` only after its own
   staleness check: copy the three files of each run by hand first, as the build does.)
3. **Build and check:** `python3 build.py && python3 check.py` (expect `OK: … pages …`), then
   `check.py --mutate` and `features/check_features.py`.
4. **Port:** in the worktree, `python3 web/tools/port_site.py --site
   ~/Code/kshana-pro/site-next/explorations/2026-09-27-combo --studio
   ~/Code/kshana-pro/site-next/explorations/2026-10-01-studio-rebuild` (twice: the second run must
   write nothing), then `node web/site.test.mjs`, `node web/legacy-urls.test.mjs`, `node web/smoke.mjs`
   (after `KSHANA_WASM_PKG=<the package> ./web/build.sh`) and `scripts/check-version-sync.sh`.
5. Commit with the `ashfordeOU` identity and no trailers; open a pull request; never push a tag from
   a site branch.

Every step, with the expected output, is in `site-next/PORT-RUNBOOK.md` and
`site-next/ASSEMBLY-RUNBOOK.md`; `site-next/HANDOVER.md` has the hard rules.

## Keeping this repository the source

- Edit the site in `site-next/` here, commit and push, then port. A change made directly in the
  public `web/` is overwritten by the next port and fails `web/site.test.mjs` against the manifest.
- The public repo owns a few `web/` files the port does not manage: `web/og-card.*` (drawn by the
  public `tools/gen_og_card.py`), `web/favicon.svg`, `web/capabilities.json`, `web/fonts/`,
  `web/vendor/` and `web/tools/`. When you change one of those, refresh its copy in `brand/` too.
