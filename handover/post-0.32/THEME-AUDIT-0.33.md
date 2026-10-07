# Theme audit for 0.33.0: what on kshana.dev, the Studio, the reports and the READMEs is not on the current theme

**Audited:** 2026-10-07, live https://kshana.dev (the 0.32.0 deploy) and public `main` at `58957f8e`;
site source = kshana-pro `site-next/` (canonical). Read-only: nothing in either repo was changed
except this file and its evidence folder.
**Evidence:** `theme-audit-0.33/` next to this file (20 JPEGs, 900 px wide, 1.2 MB total).

## 0. Which theme is "current" — the question this audit cannot settle

Two earlier audits in `salvage/` disagree:

- `site-audit-A.md`: the current site theme is **Observatory** = `web/tokens.css` + `web/theme.css`
  (theme.css header: "the Observatory system (founder's choice, 2026-09-28)"). This matches what
  is in the canonical source today: `site-next/explorations/2026-09-27-combo/src/theme.css` is
  byte-identical to the live `web/theme.css`.
- `site-theme-audit.md`: the target is the **Ashforde gold/Garamond** theme. That theme exists, but
  only in the Pro document generators (kshana-pro `src/report_brand.rs`, `src/brand_pdf.rs`,
  `src/report_pdf.rs`, `papers/*/report.html`) and the Pro dossier sample on the Editions page. It
  was never in the site source.

**This audit measures against Observatory** (the theme the site source declares). If the target is
Ashforde gold instead, every surface below is OLD — section 6 gives that count — and the fix is
a site-wide redesign, not a clean-up.

Five theme systems are in play, oldest first:

| Name used here | Where it is defined | Signature |
|---|---|---|
| **Warm-dark (July)** | public `src/chart.rs` + ~35 engine modules; `tools/gen_validation_figures.py` ("dark brand palette", 2026-07-02); the diagrams' Mermaid styling (not in either repo; baked into the SVGs) | near-black `#0c0b08`/`#11100b`, gold-tan `#e0bd84`/`#c79e63`, cream `#f1ece2`, Helvetica / sans-serif |
| **Warm paper report** | public `src/advanced_report.rs` (mirrored in kshana-pro `report-templates/report-theme.css`) | cream `#fbfaf7`, brown accent `#9a5b22`/`#e0a36a`, monospace + system-ui |
| **Combo** (2026-09-27) | `tokens.css` alone: six domain colours `--int --spf --tim --orb --itg --nav`, "space" panels `--space*` (dark in both themes), `--r-card` 28 px, `--sh-*` shadows, Bricolage Grotesque display | domain reds/purples/blues `#B8262B #6236E0 #1C4FE0 #006C7D #0A6F38 #7C4800`, series `#FF5B54 #A58BFF #6A98FF #2CD0DE #3DDC84 #FFB224`, space `#05060C`/`#EEF0FA`/`#8088A6` |
| **Observatory** (2026-09-28, current) | `theme.css` over tokens.css, site pages only | blue-grey ground `#E8ECF3` / `#060A14`, accents `--cyan --magenta --lime --amber --coral` `#066A86 #A8247F #2F6E0A #764600 #B42D1B`, `--panel` glass, Geist + Unbounded |
| **Ashforde gold** (Pro documents) | kshana-pro `src/report_brand.rs`, `brand_pdf.rs` | gold `#B6985A`, ink `#11161E`, paper `#F6F4F0`, Cormorant / EB Garamond |

`theme.css` says it re-points the shared token names "so pages not yet rebuilt follow the same
base". That is the root cause of the founder's report: the Observatory rebuild was **partial**.
Sections built from combo components (space panels, domain colours, combo cards and charts) still
render that way on an Observatory page, and they read as an older theme beside the rebuilt ones.

## 1. Method

- A Playwright audit with Chromium 141 at 1280×900, run in both light and dark, over the 7 top-level
  pages plus 4 docs pages (the other 57 docs pages share the same template). For every top-level
  section it reads the computed `color`, `background-color`, `border-color`, `fill` and `stroke` of
  up to 4,000 elements, then classifies each colour as combo / Observatory / warm / gold by exact hex.
  It also records the heading font and screenshots the section.
- **Verdict rule:** OLD when the combo count exceeds the Observatory count and is at least 25;
  MIXED when combo is at least 25 but Observatory is higher; otherwise CURRENT. The Studio is OLD
  (combo) by construction: it loads tokens.css without theme.css.
- I checked each OLD/MIXED verdict by eye on its screenshot. The verdict is mechanical; the
  screenshots are the evidence.
- **Source attribution:** a static pass over `site-next/.../src/**/*.css` counts each rule's
  `var(--combo…)` vs `var(--observatory…)` references plus hard-coded combo hexes, per selector.
- For images, a palette grep of every SVG plus a visual check of rendered samples. Raster provenance
  comes from `SHOTS.json`, `MANIFEST.json` and `rendered-from.json`.

Per-file combo/Observatory variable references in the site source (`src/`):

| File | combo refs | Observatory refs | Reading |
|---|---|---|---|
| `site.css` | 108 | 145 | shared components; the combo-only rules are `.hx-space` (9/0), `.pro-band` (8/0), `.nav.on-dark` (7/1), `.repro` (7/0), `.evi-n`, `.chat`, `.pb-*`, `.viz-plot`, `.cfg*`, `.st-*`, `.sd.ok`, `.ba-map` |
| `css/pro-power.css` | 25 | 1 | **entirely combo**: `.pw`, `.pw-more`, `.pw-principles`, `.pw-q`, `.pw-ladder`, `.pw-not`, `.pw-terms`, `.pw-tiles`, `.pw-trio-tabs` |
| `css/capabilities.css` | 51 | 199 | `.journey` (12/0), `.ss-*` / `.ch-*` / `.cz-*` / `.cw-*` console HUDs |
| `css/missions.css` | 24 | 145 | `.mx-chart` (16/0), `.mx-readout` |
| `css/evidence.css` | 18 | 147 | `.pub-*` research plots (`.pub-svg` 6/0) |
| `css/developers.css` | 6 | 52 | `.dt-*` terminal |
| `css/editions.css` | 2 | 69 | dark-theme overrides only |
| `css/docs.css` | 0 | 84 | (one hard-coded `#6A2CD6`, a near-copy of `--spf`) |
| `css/home.css`, `home-waterfall.css`, `light.css` | 1 / 0 / 1 | 159 / 30 / 32 | CURRENT |

## 2. Live site, section by section (light and dark counts are combo/Observatory hits)

| Page | # | Section (id / heading) | Light combo/obs | Dark combo/obs | Heading font | Verdict | Source of the combo styling | Evidence |
|---|---|---|---|---|---|---|---|---|
| / | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| / | 1 | top — Rehearse the minute GNSS goes dark. | 0/60 | 0/123 | Geist | CURRENT |  |  |
| / | 2 | proof | 0/2 | 0/23 |  | CURRENT |  |  |
| / | 3 | lband — Watch a jammer take the band. | 0/8 | 0/129 | Geist | CURRENT |  |  |
| / | 4 | timeline — 1/6Nominal | 0/18 | 0/185 | Geist | CURRENT |  |  |
| / | 5 | capabilities — One engine for the whole failure chain. | 0/1044 | 0/2418 | Geist | CURRENT |  |  |
| / | 6 | assistant — Ask your AI assistant. The engine answers. | 0/10 | 10/134 | Geist | CURRENT |  |  |
| / | 7 | story — A lie that looks like a fix. | 0/11 | 0/94 | Geist | CURRENT |  |  |
| / | 8 | pro-band — Free for everyone. Pro when the answer has to st | 938/13 | 938/55 | Geist | OLD | site.css .pro-band/.pb-* + css/pro-power.css .pw-tiles (space tiles) | [home--8-light.jpg](theme-audit-0.33/home--8-light.jpg) |
| / | 9 | closing — Find where your timing breaks, before it does. | 0/0 | 0/7 | Geist | CURRENT |  |  |
| / | 10 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /capabilities | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| /capabilities | 1 | page-hero — One engine for the whole failure chain. | 34/9 | 36/74 | Geist | OLD | site.css .hx-space (dark 'space' hero panel) | [capabilities--1-light.jpg](theme-audit-0.33/capabilities--1-light.jpg) |
| /capabilities | 2 | waterfall — The whole L band, second by second. | 7/213 | 7/513 | Geist | CURRENT |  |  |
| /capabilities | 3 | console — Six consoles, one engine. | 520/128 | 970/2225 | Geist | OLD | css/capabilities.css .ss-*/.ch-*/.cz-*/.cw-* HUDs + .journey (12 combo / 0 obs) | [capabilities--3-light.jpg](theme-audit-0.33/capabilities--3-light.jpg) |
| /capabilities | 4 | build — Change any run, in your browser. | 9/16 | 11/204 | Geist | CURRENT |  |  |
| /capabilities | 5 | reference — Every scenario, capability and standard. | 65/8 | 66/1402 | Geist | OLD | site.css domain dots (--itg/--spf/--int) | [capabilities--5-light.jpg](theme-audit-0.33/capabilities--5-light.jpg) |
| /capabilities | 6 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /developers | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| /developers | 1 | page-hero — One engine. Every door in. | 31/51 | 32/135 | Geist | MIXED | site.css .hx-space; css/developers.css .dt-* | [developers--1-light.jpg](theme-audit-0.33/developers--1-light.jpg) |
| /developers | 2 | start — Install in one line. Or none. | 0/37 | 19/580 | Geist | CURRENT |  |  |
| /developers | 3 | ways — Eleven ways in. | 9/22 | 11/420 | Geist | CURRENT |  |  |
| /developers | 4 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /editions | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| /editions | 1 | page-hero — Free to build on. Pro when it has to stand up in | 0/68 | 1/340 | Geist | CURRENT |  |  |
| /editions | 2 | editions — Compare the editions. | 0/20 | 0/139 | Geist | CURRENT |  |  |
| /editions | 3 | pro — Free runs one scenario. Pro answers the programm | 2859/0 | 2859/1 | Geist | OLD | css/pro-power.css .pw* (25 combo / 1 obs) + site.css .hx-space | [editions--3-light.jpg](theme-audit-0.33/editions--3-light.jpg) |
| /editions | 4 | custom-why — When you need an answer, not a tool. | 0/3 | 0/22 | Geist | CURRENT |  |  |
| /editions | 5 | install-core | 0/11 | 0/118 |  | CURRENT |  |  |
| /editions | 6 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /evidence | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| /evidence | 1 | page-hero — Every capability wears its evidence. | 0/131 | 1/458 | Geist | CURRENT |  |  |
| /evidence | 2 | results — Checked against someone else's answer. | 0/146 | 0/1565 | Geist | CURRENT |  |  |
| /evidence | 3 | matrix — One cell per capability. | 0/131 | 0/431 | Geist | CURRENT |  |  |
| /evidence | 4 | ledger — All 249 rows, one line each. | 4/15 | 4/4007 | Geist | CURRENT |  |  |
| /evidence | 5 | repro — Rerun it. Get the same numbers. | 25/2 | 25/72 | Geist | OLD | site.css .repro (7/0) | [evidence--5-light.jpg](theme-audit-0.33/evidence--5-light.jpg) |
| /evidence | 6 | standards — Speaks the formats you already file. | 26/1 | 26/126 | Geist | OLD | site.css .sd.ok (--itg) | [evidence--6-light.jpg](theme-audit-0.33/evidence--6-light.jpg) |
| /evidence | 7 | next — Same evidence, packaged or done for you. | 0/5 | 0/26 | Geist | CURRENT |  |  |
| /evidence | 8 | cite — Published research. | 410/60 | 429/780 | Geist | OLD | css/evidence.css .pub-* (.pub-svg 6/0) | [evidence--8-light.jpg](theme-audit-0.33/evidence--8-light.jpg) |
| /evidence | 9 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /missions | 0 | nav | 0/0 | 0/30 |  | CURRENT |  |  |
| /missions | 1 | top — Your mission. Its failure. A scenario that shows | 0/46 | 3/250 | Geist | CURRENT |  |  |
| /missions | 2 | sectors — Ten sectors. One engine. | 312/432 | 366/2891 | Geist | MIXED | css/missions.css .mx-chart (16/0) | [missions--2-light.jpg](theme-audit-0.33/missions--2-light.jpg) |
| /missions | 3 | orbits — Any constellation, around any body. | 0/20 | 0/323 | Geist | CURRENT |  |  |
| /missions | 4 | fails — Six ways GNSS fails, and who it hits. | 262/12 | 281/493 | Geist | OLD | css/missions.css .mx-chart + site.css space cards | [missions--4-light.jpg](theme-audit-0.33/missions--4-light.jpg) |
| /missions | 5 | shows — Four pictures every sector asks for. | 1875/8 | 1888/1105 | Geist | OLD | css/missions.css .mx-chart/.mx-readout (space map) | [missions--5-light.jpg](theme-audit-0.33/missions--5-light.jpg) |
| /missions | 6 | who — A way in for every role. | 0/9 | 0/97 | Geist | CURRENT |  |  |
| /missions | 7 | honest | 0/0 | 0/6 |  | CURRENT |  |  |
| /missions | 8 | foot — Product | 0/1 | 0/34 | Geist Mono | CURRENT |  |  |
| /studio/ | 0 | bar | 1/0 | 28/0 |  | OLD (combo) | Studio | studio--0-light.jpg (not committed) |
| /studio/ | 1 | home — What would you like to find out? | 2680/0 | 3084/0 | Bricolage Grotesque | OLD (combo) | Studio: explorations/2026-10-01-studio-rebuild (tokens.css only, Bricolage) | [studio--1-light.jpg](theme-audit-0.33/studio--1-light.jpg) |
| /studio/ | 2 | res-foot | 0/0 | 23/0 |  | OLD (combo) | Studio | studio--2-light.jpg (not committed) |
| /docs/agency-orbit-validation | 0 | nav | 0/0 | — |  | CURRENT |  |  |
| /docs/agency-orbit-validation | 1 | docs — Get started | 0/1 | — | Geist Mono | CURRENT |  |  |
| /docs/agency-orbit-validation | 2 | foot — Product | 0/1 | — | Geist Mono | CURRENT |  |  |
| /docs/animation | 0 | nav | 0/0 | — |  | CURRENT |  |  |
| /docs/animation | 1 | docs — Get started | 6/1 | — | Geist Mono | CURRENT |  |  |
| /docs/animation | 2 | foot — Product | 0/1 | — | Geist Mono | CURRENT |  |  |
| /docs/araim-reference | 0 | nav | 0/0 | — |  | CURRENT |  |  |
| /docs/araim-reference | 1 | docs — Get started | 0/15 | — | Geist Mono | CURRENT |  |  |
| /docs/araim-reference | 2 | foot — Product | 0/1 | — | Geist Mono | CURRENT |  |  |
| /docs/architecture | 0 | nav | 0/0 | — |  | CURRENT |  |  |
| /docs/architecture | 1 | docs — Get started | 14/23 | — | Geist Mono | CURRENT |  |  |
| /docs/architecture | 2 | foot — Product | 0/1 | — | Geist Mono | CURRENT |  |  |

Docs pages: the 4 sampled (`agency-orbit-validation`, `animation`, `araim-reference`, `architecture`)
are CURRENT. The template is Observatory (`css/docs.css` 0 combo refs). The only stray is the
hard-coded `#6A2CD6` purple in a few docs callouts (6–14 hits per page).

What the OLD sections look like (see the screenshots):
- **Editions › "Free runs one scenario. Pro answers…"** (`#pro`): a full-width dark indigo/purple
  "space" block with lavender `#C3A6FF` / `#1E1446` chips and combo series colours. It is the
  strongest visual break on the site. Home's `#pro-band` ends in the same dark tiles.
- **Missions › "Six ways GNSS fails" / "Four pictures every sector asks for" / sectors**: black
  space-panel charts (`.mx-chart`) with combo series red/blue inside otherwise Observatory cards.
- **Capabilities › hero panel, "Six consoles, one engine"**: dark space HUDs plus domain-coloured
  top rules (red / purple / blue per failure domain) on the "Six ways GNSS fails" cards. The
  reference atlas uses domain-colour dots (`--itg` green, `--spf` purple).
- **Evidence › "Rerun it" (dark code panel), "Speaks the formats" (domain-green checks),
  "Published research"** (dark bar plots with combo lavender/blue).
- **Developers › hero terminal**: MIXED; the space terminal sits inside an Observatory hero.
- **Studio**: the whole app is combo (tokens.css only, Bricolage Grotesque headings, domain
  colours, space panels). tokens.css says this split is "by design", but beside the site it reads
  as the old theme.

## 3. Studio, reports, OG card, README and docs images

| Item | Theme shown | Generator (and where its source lives) | Verdict |
|---|---|---|---|
| Studio (`/studio/`, `web/studio/`) | Combo | kshana-pro `site-next/explorations/2026-10-01-studio-rebuild` (`app.css`, `studio.css`, tokens.css); ported by `port_site.py --studio` | **OLD** (combo) |
| Studio chart exports ("SVG", the Report view) | Warm-dark | engine `chart_svg` (public `src/chart.rs` + kind modules, via WASM) | **OLD** (warm-dark) |
| 9 run reports `web/assets/{missions-runs,waterfall,runs}/**/report.html` (linked as "Report" from site cards) | Warm paper page + warm-dark embedded chart | public `src/advanced_report.rs` (+ `src/chart.rs`); recorded by the site recorders; mirror in kshana-pro `report-templates/` | **OLD** (warm paper + warm-dark) |
| Every engine `*.chart.svg` | Warm-dark | public `src/chart.rs` and about 35 kind modules with hard-coded `#0c0b08` / `#e0bd84` / `#c79e63` (e.g. `sweep.rs`, `raim.rs`, `pvt.rs`, `lunar_*.rs`, `leo_*`) | **OLD** (warm-dark) |
| `web/og-card.svg` / `.png` (the social card) | Observatory-dark (navy ground, Unbounded wordmark, cyan eyebrow); refreshed 2026-10-07 | public `tools/gen_og_card.py` (master in kshana-pro `brand/`) | CURRENT ([evidence](theme-audit-0.33/web_og-card.jpg)). The kshana-pro `brand/og-card.svg` master is also Observatory-dark, but its content differs from the public card: re-sync it |
| README `docs/assets/readme/*-{light,dark}.svg` (hero, architecture, campaign-timeline, coverage-map, flow-×4, lband-waterfall, leo-pass, research, solar-system, logo) | Observatory palette; most regenerated 2026-10-07 | public `tools/gen_readme_assets.py` (`MANIFEST.json`) | CURRENT ([hero](theme-audit-0.33/readme_hero-light.jpg)). The light variants carry a few combo hexes (1–23 each), which is minor |
| README `docs/assets/readme/site/site-strip-{light,dark}.jpg` | Real screenshots of the 0.31.0 site: Home hero (CURRENT), Evidence research (OLD section), **Editions "Same engine, amplified" (OLD section)** | public `tools/capture_site_shots.mjs` + `tools/readme_shots.py` (`SHOTS.json`) | **Re-capture** after the site fix (2 of 3 panels show OLD sections; stamps say v0.31.0) |
| README `docs/assets/readme/studio/studio-{home,task,advanced}-{light,dark}.jpg` | Real screenshots of the Studio (combo), v0.31.0 | public `tools/capture_studio_shots.mjs` + `readme_shots.py` | **OLD** (combo) — re-capture after any Studio restyle; version-stale either way |
| README demo charts `docs/assets/{clock-holdover,inertial-deadreckoning,orbit-gnss-challenged}.svg` | Warm-dark | engine `cargo run -- scenarios/<name>.toml` (`src/chart.rs`); guarded by `tests/published_figures_still_reproduce.rs` | **OLD** (warm-dark) ([evidence](theme-audit-0.33/assets_clock-holdover.jpg)) |
| `docs/assets/{clock-ensemble-band,sweep-clock-stability}.svg` | Warm-dark (sans-serif) | engine chart output (June) | **OLD** (warm-dark) |
| `docs/assets/figures/{validation-breakdown,oracle-kind-stacked,scenario-fom,sgp4-regime-bars,domain-coverage-map}.{svg,png}` (4 in README) | Warm-dark ("dark brand palette", Helvetica) | public `tools/gen_validation_figures.py` (`BG #0c0b08`, `TITLE #e0bd84`) | **OLD** (warm-dark) ([evidence](theme-audit-0.33/figures_validation-breakdown.jpg)) |
| `docs/assets/diagrams/{validation-provenance,system-overview,engine-flow,module-map,distribution}.{svg,png}` (1 in README) | Warm-dark Mermaid (`#11100b` boxes, `#c79e63` borders, Helvetica) | sources `docs/diagrams/*.mmd`; rendered with mermaid-cli, then `tools/render-diagram.sh` (PNG + `rendered-from.json`). **The Mermaid theme config is in neither repo** | **OLD** (warm-dark) ([1](theme-audit-0.33/diagrams_validation-provenance.jpg), [2](theme-audit-0.33/diagrams_system-overview.jpg)) |
| `paper/crossover/{clock,inertial}.png` (README) | matplotlib dark, warm-tan title; footer **"Kshana v0.20.0"** | public `paper/crossover/render_{clock,inertial}.py` from `*.json` | **OLD** (warm-dark, and version-stale) ([evidence](theme-audit-0.33/paper_crossover_clock.jpg)) |
| `docs/assets/kshana-{banner,mark,wordmark}.svg`, `kshana-logo.png` (June) | Brand marks, palette-neutral | kshana-pro `brand/` masters | n/a (marks) |
| Editions › Pro dossier sample (`web/assets/pro/mission-dossier*`) | **Ashforde gold** (Pro document style) | kshana-pro Pro generators; `pro-assets/` | Deliberate: it is a Pro document |
| `theme-color` meta / `apple-touch-icon` | absent on every page | page `<head>` from `site-next/.../src/shell.html` | Gap, not a theme mismatch (noted in site-audit-A) |

## 4. Where each OLD item's source lives, and the fix

| OLD item | Source | Fix type |
|---|---|---|
| Editions `#pro`, Home `#pro-band` tiles | kshana-pro `site-next/explorations/2026-09-27-combo/src/css/pro-power.css` (`.pw*`), `src/site.css` (`.pro-band`, `.pb-*`, `.hx-space`) | **Source change** (CSS → Observatory tokens: `--panel`, `--cyan/--magenta`, `--shadow`, `--r`); then build + port |
| Capabilities hero, console HUDs, journey, reference dots | `src/css/capabilities.css` (`.journey`, `.ss-*`, `.ch-*`, `.cz-*`, `.cw-*`), `src/site.css` (`.hx-space`, domain dots) | **Source change** |
| Missions sectors / fails / shows charts | `src/css/missions.css` (`.mx-chart`, `.mx-readout`) and the chart colours the site's `build.py` / `tools/build_viz.mjs` emit from the Studio view code | **Source change** (CSS + viz series mapping) |
| Evidence repro / standards / research | `src/site.css` (`.repro`, `.sd.ok`, `.evi-n`), `src/css/evidence.css` (`.pub-*`) | **Source change** |
| Developers hero terminal | `src/site.css` `.hx-space`, `src/css/developers.css` `.dt-*` | **Source change** |
| Docs `#6A2CD6` callouts | `src/css/docs.css` | **Source change** (one-line token swap) |
| Studio (whole app) | kshana-pro `site-next/explorations/2026-10-01-studio-rebuild` (`app.css`, `studio.css`, its tokens.css; it loads no theme.css) | **Source change, needs a decision**: load Observatory theme.css + Unbounded in the Studio, or keep the combo Studio deliberately |
| Engine charts (`chart.svg`, Studio exports, report charts, README demo charts) | public `src/chart.rs` + ~35 kind modules | **Public engine change** (one shared chart palette module), then **regenerate** README demo charts + the `published_figures_still_reproduce` goldens, report exports, and Studio recordings |
| Run report pages (9) | public `src/advanced_report.rs` | **Public engine change**, then **regenerate** (site recorders, then port) |
| Validation figures | public `tools/gen_validation_figures.py` | **Public tool change**, then **regenerate** |
| Mermaid diagrams | `docs/diagrams/*.mmd` + an untracked Mermaid theme | **Add a tracked Mermaid config** with Observatory colours, then **regenerate** via `tools/render-diagram.sh` |
| Paper crossover PNGs | public `paper/crossover/render_*.py` | **Public tool change**, then **regenerate** (also fixes the v0.20.0 stamp) |
| README site strip + Studio shots | `tools/capture_site_shots.mjs`, `capture_studio_shots.mjs`, `readme_shots.py` | **Regenerate** after the site/Studio fix is ported |

## 5. Counts

- **Live site, 7 top-level pages, 52 sections** (nav and footer counted per page): **37 CURRENT,
  2 MIXED, 10 OLD, plus the 3 Studio sections (combo)**. The OLD sections are on Home (1),
  Capabilities (3), Editions (1), Evidence (3) and Missions (2); the MIXED ones are on
  Developers (1) and Missions (1).
- **Docs pages:** 61, one Observatory template. CURRENT apart from one hard-coded colour.
- **Studio:** 1 app, entirely combo.
- **Images referenced from the four READMEs:** 32 distinct local references, plus 1 external
  registry badge:
  - CURRENT: 13 Observatory SVGs, including the logo.
  - OLD:
    - 3 Studio screenshots, combo theme;
    - 1 site strip, 2 of whose 3 panels show OLD sections;
    - 3 engine demo charts, warm-dark;
    - 4 figures, each as SVG and PNG (8 references), warm-dark;
    - 1 diagram as SVG and PNG (2 references), warm-dark;
    - 2 paper PNGs, warm-dark and stamped v0.20.0.

  In total **19 OLD image references**.
- **Other generated outputs:** 9 run reports (OLD), every engine `chart.svg` (OLD), og-card (CURRENT),
  4 more docs diagrams and 3 more figures (OLD), 2 more docs engine charts (OLD).

## 6. If the target is Ashforde gold instead

Then nothing is CURRENT: all 52 sections, all 61 docs pages, the Studio, the og-card and all 32
README images are OLD. Only the Pro dossier sample is on target. That would need new site tokens and
fonts, a new wordmark treatment, re-rendering every image, and a decision on the
"Kshana · क्षण · the precise instant" name line. `site-theme-audit.md` §11 has the full list. It is
a redesign, not a fix, so the plan below assumes Observatory.

## 7. Fix plan (assuming Observatory)

**A. Site-source PR in kshana-pro (W4), CSS only, no new design:**
1. `css/pro-power.css` (`.pw*`) and `site.css` `.pro-band` / `.pb-*`: move the Pro blocks from the
   dark indigo space treatment onto Observatory surfaces (`--panel`, `--bg-3`, accents
   `--magenta` for Pro and `--cyan`), keeping one dark "instrument" surface only where the content
   is a chart.
2. `site.css` `.hx-space`, `.repro`, `.chat`, `.cfg*`, `.st-*`, `.viz-plot`, `.ba-map`: define
   **one** Observatory instrument-dark surface in `theme.css` (e.g. `--inst`, `--inst-ink`,
   `--inst-line`, light and dark) and point every space panel at it, instead of the combo
   `--space*`.
3. Map the six domain colours onto the Observatory accents in `theme.css` (`--int`→coral,
   `--spf`→magenta, `--tim`→cyan-blue, `--orb`→cyan, `--itg`→lime, `--nav`→amber, and their
   `-card` / `s-*` series), so every remaining `var(--int…)` (missions `.mx-chart`, capabilities
   `.journey` / HUDs, evidence `.pub-*` / `.sd.ok`, reference dots) follows Observatory with no
   per-rule edits. Then fix the near-duplicate hard-codes (`#C9321F #6B4FD8 #2F55C8 #3552A8
   #1F7A3E #7E4B00 #6A2CD6 #C3A6FF #1E1446 #1A1240`) to tokens.
4. Studio (only if the founder wants it on Observatory): load `theme.css` + the Unbounded/Geist
   font file in `studio-rebuild` and drop Bricolage; the domain-colour mapping from step 3 then
   applies.
5. Tidy: stale `0.29.2` comments; the tokens.css header still documents Bricolage as the display font.
6. Verify without a full rebuild: serve public `web/` with the changed CSS swapped in, re-run this
   audit's Playwright pass (light + dark) and screenshot every section. Target: 0 OLD and 0 MIXED.
   Then a full `build.py` + `check.py` run (W2's re-port runbook) carries it into `web/`.

**B. Regenerated in the 0.33.0 release (W2), after A merges:**
1. The re-port (`site-next/RE-PORT.md`) carries the CSS into `web/`. Re-capture the README site
   strip and Studio shots with `tools/capture_{site,studio}_shots.mjs` + `readme_shots.py`, from the
   ported site.
2. Public source changes, decided first and then made by whoever holds public push:
   - one shared chart palette for `src/chart.rs` and the ~35 kind modules;
   - an Observatory restyle of `src/advanced_report.rs`;
   - the palette in `tools/gen_validation_figures.py`;
   - a tracked Mermaid config;
   - `paper/crossover/render_*.py`.

   Each needs its regeneration in the same bump:
   - the README demo charts and their `published_figures_still_reproduce` goldens;
   - the docs figures and diagrams (`render-diagram.sh`, `rendered-from.json`);
   - the paper PNGs;
   - the run reports and Studio recordings, through the recorders.

   **Decision needed:** whether engine charts and reports keep a separate "document" look (warm
   paper / warm-dark, as now) or follow the site. Changing `src/chart.rs` changes every published
   chart's bytes, which is a revision under the repo's reproducibility rules.
3. Re-sync kshana-pro `brand/og-card.*` with the public og-card (same theme; the content differs).
