# kshana-site-next

Separate project for the kshana.dev redesign. Nothing here touches `~/Code/kshana/web/`
(the live site) until the new site is proven.

## explorations/2026-09-27-directions

Four direction mockups (A Live Instrument, B Signal Field, C Mission Time, D Proof Journal)
as Design-canvas artboards. These files are canonical; the canvas is a published copy:
https://claude.ai/artifact/8ohxNMkWumfw9benoZAtMX

Numbers in the mockups are real (verification matrix 66/104/4 of 174, published validation
results and their test files). Charts and readouts that stand in for a live engine run are
labelled "illustrative".

## explorations/2026-09-27-round2 (after round 1 was rejected as flat wireframes)

Three full landing-page concepts, one self-contained `index.html` each (Three.js 0.160 and
GSAP 3.12.5 from CDNs). Shared facts brief: `BRIEF.md`. Files are canonical; published copies:
- Aurora (Apple-style, 3D Earth, pinned story): https://claude.ai/artifact/2o1vPKA88DpKwN8fB3tKuD
- Spectrum (colourful SaaS, live waterfall): https://claude.ai/artifact/3MqJDtDFG3GKpeTF5tpF2S
- Observatory (mission control, command palette): https://claude.ai/artifact/H7t3zadmEE5PanUiGdRrMU

## explorations/2026-09-27-playground — Playground (2026-09-27)

The redesigned playground, a working app that pairs with the combo landing page and shares
its design tokens verbatim (`tokens.css`, identical to `../2026-09-27-combo/tokens.css`).
It runs the REAL engine: `pkg/` is the published npm package `kshana@0.28.0` (WebAssembly).
Run it locally:

    cd explorations/2026-09-27-playground && python3 -m http.server 18731
    # open http://127.0.0.1:18731/   (add ?engine=recorded to see the fallback)
    node selfcheck.mjs               # parity table, module tests, hosts, names, catalogue, recordings

- `index.html`, `app.css`, `app.js`: the app (DOM layer).
- `lib/`: the live site's pure modules copied unchanged from `~/Code/kshana/web` (engine,
  share, guided, sweep, overlay, compare, report, chartdl, orbit3d, tour, tabs, hover, embed,
  counts) with their tests, plus new pure modules `catalog.mjs`, `params.mjs`, `views.mjs`
  with tests. `counts.test.mjs` is adapted: its README/index.html pins belong to the repo.
- `scenarios/`, `data/`: copied from the repo (78 scenario files; verification matrix).
- `recorded/`: real engine outputs for 76 of the 78 scenarios, made by `node tools_record.mjs`
  with the same WebAssembly build, shown with a "Recorded run" label when a host blocks
  WebAssembly. Exports over 150 kB are not recorded (they say so).
- `PARITY.md`: every live playground feature mapped to the new UI, plus what is new. The
  self-check fails if a row's feature is missing from the code.

## explorations/2026-09-27-combo (Combo, 2026-09-27)

One landing page that fuses Aurora (cinematic 3-D globe and pinned scroll story), Spectrum
(colour-coded failure domains and the live waterfall) and Observatory (telemetry, mission console,
command palette, evidence tags), plus founder additions: missions by sector, roles, ways to use it,
band atlas, clock ladder, timing wall, capability atlas, Earth-Moon-Mars scroll and a free-versus-Pro
editions section with contact-only paid tiers. Built, not hand-edited:

    cd explorations/2026-09-27-combo && python3 build.py && python3 check.py

- `index.html`: the built page (self-contained; Three.js 0.160 and GSAP 3.12.5 from CDNs).
- `src/page.html`, `src/style.css`, `src/app.js`, `src/globe.mjs`: the sources `build.py` inlines.
- `build.py`: reads every count and dataset from `~/Code/kshana` (read-only) at build time: version,
  63 kinds, 77 scenario files, the 174-row ledger, 46 capability cards, 13 standards, clock presets,
  ITU-T masks, and the committed `clock-holdover` result the playground section shows as real output.
- `check.py`: tag balance, allowed hosts, forbidden words and prices, number tracing, coverage markers,
  scenario names.
- `COVERAGE.md`: every live-site need mapped to the section that covers it, gaps closed, verified
  bands and clocks, and engine feature gaps proposed (not shown as existing).
- `tokens.css`: the unified design tokens, shared verbatim with `2026-09-27-playground`.

## Published preview (2026-09-27)
Combo landing page + redesigned playground as ONE artifact (playground served under `playground/`):
https://claude.ai/artifact/2rsxG7ZAcjhuuzP7nnTWVN
Staged from the scratchpad: combo `index.html` with `../2026-09-27-playground/` rewritten to `playground/`,
plus the playground folder minus `selfcheck.mjs`, `tools_record.mjs`, `PARITY.md`, `*.test.mjs` and `pkg/kshana.d.ts`.
Downloads (report, PNG, CSV) do nothing inside the artifact viewer; they work when the site is served normally.

## Multi-page site (2026-09-27, after the overlap audit)

`explorations/2026-09-27-combo/` is now a multi-page site: `index.html` (Home), `missions.html`,
`capabilities.html`, `evidence.html`, `developers.html`, `editions.html`, `docs/` (generated from the
repository's git-tracked Markdown at HEAD) and `playground/` (a synced copy of
`../2026-09-27-playground` without its dev files). One shared shell (`src/shell.html`, `site.css`,
`site.js`, `tokens.css`); each page loads only its own module (`js/*.mjs`).

    cd explorations/2026-09-27-combo && python3 build.py && python3 check.py   # also: check.py --mutate

Navigation check (every link lands where it says, phone and desktop), against a served build. Pages
link each other by clean addresses (`/evidence#research`), which Python's plain `http.server` does not
resolve, so serve the build with `features/serve.py` (it maps `/x` to `x.html`, like the host):

    python3 features/serve.py explorations/2026-09-27-combo 8847 &
 It follows every page of the site and the docs, clicks every in-page
`#` link from mid-page, loads every cross-page `page.html#anchor` link fresh, and clicks every page link
from a scrolled page. An anchor must settle 60 to 130 px from the top (the 84 px header offset), a page
link must land at the top. Exit 1 on any BAD line; takes several minutes:

    cd explorations/2026-09-27-combo && python3 check.py --nav [http://127.0.0.1:8847]
    node features/check_nav.mjs [base-url] [--only phone|desktop] [--page /missions.html] [--verbose]

Playwright comes from `$KSHANA_PLAYWRIGHT` (a `node_modules` folder) or `promo/2026-10-01-video/node_modules`,
and drives the installed Chrome.

- Every chart is a recorded engine v0.28.0 run, rendered by the playground's own view code
  (`../../tools/build_viz.mjs`), labelled with engine version, scenario and seed, and linked to that
  run and view in the playground. No "Illustrative" stand-ins remain.
- Install commands come from `channels.json`, generated from the repo manifests and cross-checked
  against the READMEs. Docs: `../../tools/build_docs.mjs` (marked 12, highlight.js 11, pinned in
  `tools/package.json`).
- `PUBLISH.txt`: the exact file list to publish (the whole folder minus sources and build tools).
- Audit: `explorations/2026-09-27-audit/` (AUDIT.md, IMAGERY.md, PROGRESS.md, shots/, tool/).
- `src/legacy/`: the old single-page sources, kept for reference; not built.
