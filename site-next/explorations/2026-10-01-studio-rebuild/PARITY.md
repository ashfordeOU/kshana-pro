# Playground parity: live kshana.dev playground → redesigned playground

Every feature of the live playground (`~/Code/kshana/web/index.html`, `app.js` and its
`*.mjs` modules) mapped to where it lives in the redesign. `selfcheck.mjs` parses the
**Check** column of this table (`file: token`) and fails if the token is missing, so the
table cannot claim a feature the code does not have.

Engine: the WebAssembly (WASM) build of the integrated 75-kind engine (`pkg/`, built with
wasm-pack from the release tree, the same flags as `web/build.sh`); it reports the release version.
The package's JavaScript interface is the published one (run, summary, chart, table, SP3, OMM
and OEM exports), so the engine's run report, its animation and its CZML, KML, GeoJSON, STK and
SigMF exports have no browser writer: the Studio shows the files the native command-line engine
wrote for each bundled scenario (`native/`, made by `tools_native.mjs`) and labels them recorded.

## Parity (live feature → new UI)

| # | Live feature | Live source | New location | Check |
|---|---|---|---|---|
| 1 | Engine off the main thread (module Web Worker, main-thread fallback) | `engine.mjs`, `engine-worker.mjs` | Engine chip in the top bar; `createEngineClient` reused unchanged | `app.js: createEngineClient(` |
| 2 | Cancel a long run; busy label "Running… N s" | `engine.mjs` `busyLabel`, `cancel` | Run bar: Cancel button, status line, progress bar | `app.js: busyLabel(` |
| 3 | Supersede a run when inputs change mid-run | `app.js` job queue | Same job model (`SUPERSEDE_MS`) | `app.js: SUPERSEDE_MS` |
| 4 | Scenario picker (62 entries in a select) | `app.js` `SCENARIOS` | Library rail: all 163 bundled files by mission domain, with search | `app.js: groupedLibrary(` |
| 5 | Full TOML editor | `#toml` textarea | Scenario source panel with syntax highlighting | `index.html: id="toml"` |
| 6 | Guided sliders (≤6 knobs per scenario) | `guided.mjs` | Parameters → Guided | `app.js: knobsForToml(` |
| 7 | One-line summary | `#summary` | Headline card | `index.html: id="summary"` |
| 8 | Figures-of-merit table with validated / modelled tier | `tabs.mjs` `buildFomRows`, `report.mjs` `fomTier` | Key figure cards + Overview table | `app.js: buildFomRows(` |
| 9 | Time-series chart (engine SVG) with hover read-out | `hover.mjs` | Time series → Engine chart | `app.js: attachChartHover(` |
| 10 | Stability (Allan deviation, log axes) with hover | `app.js` `adevSvg` | Stability view (log-log, crosshair) | `app.js: renderStability` |
| 11 | Kalman filter-health cards (NIS/NEES vs 95% χ²) | `app.js` `renderFilterHealth` | Overview → Filter health | `app.js: filter_health` |
| 12 | 3-D orbit view | `orbit3d.mjs` | 3-D orbit view: Three.js, drag to rotate; SVG view as fallback and download | `app.js: orbit3dSvg(` |
| 13 | Parameter sweep, up to 60 runs, line chart | `sweep.mjs` | Sweep view | `app.js: sweepValues(` |
| 14 | Multi-run overlay compare (pin up to 4, best per metric, deltas) | `overlay.mjs`, `compare.mjs` | Compare view + Pin button + history Pin | `app.js: overlayRows(` |
| 15 | Downloadable offline HTML report | `report.mjs` | Exports & report → Reproducible report; palette command | `app.js: buildReportHtml(` |
| 16 | Chart downloads (SVG and 2× PNG, provenance-stamped filenames) | `chartdl.mjs` | Download row under every chart | `app.js: svgToPngBlob(` |
| 17 | Standards exports: SP3, CCSDS OMM, CCSDS OEM, CSV table | `app.js` `EXPORTERS` | Exports & report cards | `app.js: export_sp3` |
| 18 | Full JSON with copy and download | `#json` | JSON view (highlighted), Copy, Download | `index.html: id="json-download"` |
| 19 | Share link carrying the exact scenario | `share.mjs` `encodeFragment` | Share menu → Copy share link | `app.js: encodeFragment(` |
| 20 | Open a shared link | `share.mjs` `decodeFragment` | Loads as "Shared scenario" | `app.js: decodeFragment(` |
| 21 | Embed mode `?embed=1&scenario=…&tab=…&seed=…` | `embed.mjs` | Same URL contract; chrome hidden | `app.js: embedConfig(` |
| 22 | Guided tour (spotlight, keyboard, focus trap) | `tour.mjs` | Tour button; own steps, `placeTooltip` reused | `app.js: placeTooltip(` |
| 23 | Validation counts from the generated ledger | `counts.mjs` | Opening screen, capability-map legend (whole-engine line) | `app.js: matrixCounts(` |
| 24 | Validated/modelled honesty wording | page copy | Overview card note, headline badges, footer | `index.html: not yet oracle-checked` |
| 25 | Nothing uploaded; runs locally | page copy | Engine chip, tour, meta description | `index.html: Nothing is uploaded` |
| 26 | Print keeps the content | `bindPrintExpansion` | Print stylesheet shows every view | `app.css: @media print` |

Not carried over, by design: the capability explorer, validation ledger table, standards
grid, MCP (Model Context Protocol) section, citation and editions. They are landing-page
content and live on the combo landing page (`../2026-09-27-combo/`); the playground links
to the ledger. The live page's language switcher only translates those landing sections,
so it goes with them. `smoke.mjs` is the live site's page smoke test and is not a
playground feature.

## Beyond parity (new)

| # | New feature | Where | Check |
|---|---|---|---|
| N1 | Every numeric field as a stepper (log-scale ×/÷ for noise coefficients), synced both ways with the text, inline comments kept | Parameters → All fields | `app.js: numericFields(` |
| N2 | Signal & band view: effective C/N0 waterfall per satellite and epoch, tracking threshold, jammer band and carrier when the scenario states them | Signal & band | `app.js: signalHeatmapSvg(` |
| N3 | Holdover view: drag a timing budget, read time-to-breach per clock off the run's own record, next to the engine's own figure | Holdover | `app.js: timeToThreshold(` |
| N4 | Telecom timing masks: MTIE and TDEV on log axes with each ITU-T check's limit, verdict chips, budgets table | Timing masks | `app.js: masksModel(` |
| N5 | Ground track map with visible-pass highlighting | Ground track | `app.js: groundTrackSvg(` |
| N6 | Interactive themed time series with crosshair and GNSS-denied shading | Time series → Interactive | `app.js: lineChartSvg(` |
| N7 | Two-parameter sweep as a heat map | Sweep → Second parameter | `app.js: heatGridSvg(` |
| N8 | Difference trace B − A in compare | Compare | `app.js: B − A` |
| N9 | Run history for the session: restore or pin any earlier run | History drawer | `app.js: restoreRun(` |
| N10 | Command palette (⌘K / Ctrl-K): scenarios, views, commands | Top bar | `app.js: renderPalette(` |
| N11 | Keyboard: ⌘↵ run, / search, [ and ] switch views, arrow keys in tabs | Global | `app.js: cycleTab(` |
| N12 | Every published figure with its unit, note and provenance from the engine's `units` block | Overview | `app.js: keyFigures(` |
| N13 | Recorded-run fallback when WebAssembly is blocked (real engine outputs for 162 of 163 scenarios, clearly labelled; the two the browser build cannot run are recorded with the native engine) | Automatic; `?engine=recorded` forces it | `app.js: recordedEngine(` |
| N14 | Light and dark themes with a toggle; charts stay on dark "space" surfaces in both | Top bar | `app.js: toggleTheme(` |
| N15 | Phone layout at 390 px: the same layers in one column, a 56 px header, a back link that names where it leads | Every screen | `index.html: id="task-back"` |
| N16 | Copy embed code (iframe snippet) and preview embed | Share menu | `app.js: copyEmbedCode(` |
| N17 | Scenarios that cannot run in a browser are labelled with the reason | Library | `app.js: NOT_IN_BROWSER` |
| N18 | Reproducible report also covers non-clock kinds (key figures with tiers, signal, sweep charts) | Exports & report | `app.js: keyFigures(r, 24)` |
| N19 | Spectrum: the waterfall (frequency across, time down) with a replay, the receiver's bands at the replay time, the spectrum along the row on show; one waterfall per band panel | Spectrum | `app.js: stageSpectrum(` |
| N20 | Solar system: plan view of every body and its orbit track, the inner planets, and each planet's moons; links with their light time | Solar system | `app.js: stageSolar(` |
| N21 | Constellation design around any body: a coverage or dilution-of-precision map with the satellites replayed along their ground tracks | Coverage | `app.js: stageCoverage(` |
| N22 | Campaigns: the mission timeline (every channel on one clock, phases and events) with a replay; sweep, Monte Carlo (histogram) and composition tables | Campaign | `app.js: stageTimeline(` |
| N23 | Low Earth orbit kinds: signal design, pass and link, navigation message, positioning, precise positioning, non-terrestrial network positioning, as charts and tables read from the result | Signal design, Pass & link, Navigation message, Positioning | `app.js: panelNode(` |
| N24 | End-to-end chain: the stages in hand-off order with each one's figures | End-to-end chain | `app.js: stageChain(` |
| N25 | The engine's own animation (HTML player and animated SVG), recorded with the native engine | Animation | `app.js: renderAnimation(` |
| N26 | The engine's own run report, recorded with the native engine | Engine report | `app.js: renderEngineReport(` |
| N27 | Interoperability exports (CZML, KML, GeoJSON, STK ephemeris, SigMF) recorded with the native engine, each downloadable, with the GeoJSON drawn on a map and the engine's reason where a format does not apply | Exports & report | `app.js: renderInterop(` |
| N28 | A scenario the browser build cannot run is shown as a run recorded with the native engine, labelled as recorded | Automatic | `app.js: showNativeRun(` |
| N29 | An optional group of scenarios in its own folder, which can be deleted without touching anything else | Library | `app.js: registerGroup(` |
| N30 | Every value of the newer views records the result path it shows, so the screen can be checked against the engine output | All capability views | `app.js: "data-path": path` |
| N31 | Guided sliders for the newer kinds, each bound to one field of the scenario; array-of-tables fields in All fields | Parameters | `app.js: K.guidedFor(` |
| N32 | Deep links: `?scenario=<name>&tab=<view>` plus `play`, `frame`, `field`, `view` | URL | `app.js: S.link = {` |
| N33 | Recorded runs and native recordings are read from one file per scenario, or from a few gzip packs on a host that limits the number of files (`tools_bundle.mjs --to <folder>` writes the packs; the index entry names the pack) | Automatic | `app.js: createPackReader(` |

## Rebuild (2026-10-01): where the parity rows now live

The approved Studio redesign (`../2026-10-01-studio-ux`, founder "approved" 2026-10-01) replaces the
dashboard layout. Every row above keeps its feature; the table says where each moved.

| # | Rebuild feature | Where | Check |
|---|---|---|---|
| R1 | Opens on a real result: the four-constellation coverage map from the recorded run, satellites moving | Opening screen, hero | `app.js: paintHero(` |
| R2 | Capability map: one tile per area with its scenario count, a recorded preview and a two-part evidence bar (validated / modelled) counted from the verification matrix | Opening screen | `app.js: evidenceBar(` |
| R3 | A plain sentence of what the result means, from the result data (lib/meaning.mjs) | Task screen, top | `app.js: plainMeaning(` |
| R4 | At most five inputs by default, ranked by lib/inputs.mjs; the rest under Advanced settings | Task screen, side | `app.js: rankInputs(` |
| R5 | How this is computed: the scenario's own method note, its evidence level and references | Task screen, collapsed | `index.html: id="how"` |
| R6 | For researchers: command line, Python, exports, share, citation with the DOI, the scenario file | Task screen, collapsed | `index.html: id="research"` |
| R7 | The address is the state: Back and Forward, and every site link (`?scenario=…&tab=…`) | Everywhere | `app.js: parseState(` |
| R8 | Abbreviations spelled out at first use on each screen (lib/abbr.mjs) | Task screen and hero | `app.js: spellOut(` |
| R9 | Recorded result first, labelled, with an "Engine loading" note, then the live run | Every task link | `app.js: preview: !S.engine` |

The library rail, the step bar, the parameters drawer, the History and Tour header buttons and
the phone's bottom bar are gone: an area's list and the full list (`?domain=`, `?browse=1`) replace
the rail; Run again replaces the steps; History, Tour and Compare are in the search box (⌘K) and
under "For researchers". Kshana Pro is linked from the footer and from "For researchers" only.

## Two views (0.29.3)

| # | Feature | Where | Check |
|---|---|---|---|
| R10 | Simple view (default) and Advanced view (the full dashboard, `advanced/`), one switch in each header and "Open in Advanced view" under "For researchers"; the scenario, the panel and any edit travel in the link; the choice is remembered per browser | Header, For researchers | `app.js: switchToAdvanced(` |
| R11 | The first paint: the recorded answer of a task link, and the opening screen's answer, before the modules load (`data/firstpaint*.json`, written by `tools_areas.mjs`) | Every task link, the opening screen | `index.html: data/firstpaint.json` |
| T1 | The 0.35 pages are reachable from both views: header links and the Views menu (Simple), the Views menu (Advanced), and an "Open your own files" row on the opening screen | Header, opening screen | `index.html: tools/#evidence` |
| T2 | Advanced view carries the same entry points to trust, interference map, training debrief and tools | Advanced header | `advanced/index.html: adv-views` |
| T3 | Tools page: evidence-pack check, test-bench export, compliance mapping, OSNMA check, command-line cards and the "Use from your AI assistant" panel, engine-backed and client-side | Tools | `tools/tools.js: osnma_verify` |
| T4 | Trust, training and interference pages run the engine in the browser (score an NMEA log, generate a training stream, build an interference map and route exposure) beside opening files | Those pages | `trust/trust.js: receiver_trust_replay` |
