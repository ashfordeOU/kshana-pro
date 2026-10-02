# Kshana Studio: first-time-user baseline

Measured 2026-10-01. Read-only measurement, step 1 of the Studio redesign. Every later change should be measured against these numbers by rerunning the same script.

- **What was measured:** the Studio staging build served at `http://127.0.0.1:8853/` (source `explorations/2026-09-30-studio-dash`, engine v0.29.1, live WebAssembly mode). For comparison, the same entry links were also loaded through the site's own copy of the Studio at `http://127.0.0.1:8841/playground/index.html`.
- **How:** headless Chromium (Playwright 1.62.1). Desktop is 1440×900. Phone is 390×844 with touch and device scale factor 3, and an iPhone user agent. Every load used a fresh browser context: empty cache, no stored preferences, service workers blocked. That is a true first visit.
- **Script:** `research/studio-ux-baseline/baseline.mjs`. Rerun it with `node baseline.mjs`, or one phase at a time with `PHASES=load|views|tasks|site|waitshots`. Raw numbers are in `results.json`.
- **Inventory script:** `inventory.mjs` → `inventory.json`. It imports the Studio's own `lib/catalog.mjs`, `lib/guided.mjs`, `lib/params.mjs` and `lib/kinds.mjs`, so its counts are the Studio's own counts.
- **Screenshots:** `research/studio-ux-baseline/*.jpg`, saved as JPEG at quality 80. The names follow `<device>-<view>.jpg`.

Abbreviations used here: GNSS (global navigation satellite system), RAIM (receiver autonomous integrity monitoring), HPL/VPL (horizontal/vertical protection level), SGP4 (Simplified General Perturbations 4, a satellite orbit propagator), WASM (WebAssembly, the browser format the engine is compiled to), TOML (the scenario file format), KPI (key performance indicator, here the "Key figures" cards), CPU (processor), CSV/JSON/SVG/PNG (file formats), DOI (digital object identifier, used to cite a publication).

---

## 1. Headline numbers

| Measure | Desktop | Phone |
|---|---|---|
| Time to first result on a deep link, unthrottled (median of 3) | 0.33–1.42 s | 0.41–1.32 s |
| Time to first result on a deep link, Fast 3G plus 4× CPU slowdown | 107.1–107.8 s | 107.1–107.8 s |
| Time until the engine is ready, Fast 3G plus 4× CPU | 57.1 s | 57.1–57.4 s |
| Bytes downloaded on a cold visit | 18.6 MB | 18.6 MB |
| Times the 8.4 MB engine file is downloaded per visit | 2 | 2 |
| Result shown on the bare landing without a click | none | none |
| Controls on the first screen of the landing | 34 | 13 |
| Words on the first screen of the landing | 533 | 115 |
| Clicks to a first result for 3 newcomer tasks (from landing) | 1 / 2 / 2 (3 to reach the orbit picture) | same, plus 1013–1688 px of scrolling |
| Scenarios / domains exposed | 139 / 16 | same |
| Scenarios whose Parameters panel opens with more than 5 inputs | 34 of 139 | same |

## 2. The three proposed gates

| Gate | Target | Today | Verdict |
|---|---|---|---|
| First result within 3 s on phone | ≤ 3 s | **107.1–107.8 s** on Fast 3G plus 4× CPU (all 4 deep links). 0.41–1.32 s unthrottled on localhost. The bare landing shows no result at all until the visitor clicks. | **Fail** on a realistic phone link (about 36 times over). It passes only on localhost. |
| No more than 2 clicks to first result from any entry | ≤ 2 | Deep links: 0 clicks. Landing: task 1 (GNSS coverage) 1 click, task 3 (lunar) 2 clicks, task 2 (propagate an orbit) 2 clicks to a result but **3** clicks to see the orbit itself. Search route: 1 click, typing, then 1 click. Search only works if the right hit is near the top, and for "orbit" and "coverage" it is not (§5). | **Borderline.** It passes on click count. It fails on "meaningful" for the orbit task, and on phone it needs up to 1688 px of scrolling. |
| No more than 5 inputs visible by default | ≤ 5 | First screen with Parameters closed: 0–2 inputs (desktop 1–2, phone 0–1). After pressing Parameters, the default view has more than 5 inputs for **34 of 139** scenarios: 12 show 6 guided sliders, and 22 open straight into "All fields" with 7–43 inputs. Median 3, maximum 43. The clock-holdover "All fields" view shows 11 inputs on desktop. | **Pass** on the first screen. **Fail** for 24% of scenarios once Parameters is opened. |

How each gate was measured: §3 for time, §5 for clicks, §7 for inputs.

---

## 3. Load timing

**Method.** `addInitScript` installs a 40 ms poller before any page script runs, and records `performance.now()` the first time each milestone becomes true:
- **Start screen shown:** `#start` is visible with its domain tiles drawn.
- **Engine ready:** `#engine-chip[data-mode]` is no longer `loading`.
- **Result computed:** `#kpis-wrap` is visible and its Key figures cards exist, even if they are behind the start screen.
- **Result visible:** the dashboard is shown and the Key figures, a result panel or the run summary is inside the viewport.

First paint and first contentful paint come from the browser's paint timing entries. Largest contentful paint comes from a `PerformanceObserver`. Bytes are the sum of response body plus headers for every finished request.

**Throttling.** "Fast 3G" is Chrome DevTools' preset: 562.5 ms latency, 180 kB/s down and 84 kB/s up, applied with `Network.emulateNetworkConditions`. The 4× CPU slowdown is `Emulation.setCPUThrottlingRate`. Unthrottled cells are the median of 3 runs. Throttled cells are 1 run each, because each takes about 2 minutes.

| Device | Network | Entry | First paint | First contentful paint | Largest contentful paint | Start screen shown | Engine ready | Result computed | Result visible | Bytes | Requests | Engine downloads |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| desktop | unthrottled (median of 3) | landing | 0.63 s | 0.63 s | 0.63 s | 0.69 s | 0.69 s | 0.81 s | none (start screen) | 18.6 MB | 47 | 2 |
| desktop | unthrottled (median of 3) | dl-waterfall | 0.32 s | 0.32 s | 0.32 s | — | 0.38 s | 0.76 s | 0.76 s | 18.6 MB | 47 | 2 |
| desktop | unthrottled (median of 3) | dl-raim | 0.64 s | 0.64 s | 0.64 s | — | 0.70 s | 1.42 s | 1.42 s | 18.6 MB | 47 | 2 |
| desktop | unthrottled (median of 3) | dl-lunar-time | 0.18 s | 0.18 s | 0.34 s | — | 0.25 s | 0.33 s | 0.33 s | 18.6 MB | 47 | 2 |
| desktop | unthrottled (median of 3) | dl-clock-exports | 0.56 s | 0.56 s | 0.77 s | — | 0.61 s | 0.74 s | 0.74 s | 18.5 MB | 46 | 2 |
| phone | unthrottled (median of 3) | landing | 0.52 s | 0.52 s | 0.52 s | 0.57 s | 0.57 s | 0.69 s | none (start screen) | 18.5 MB | 44 | 2 |
| phone | unthrottled (median of 3) | dl-waterfall | 0.22 s | 0.22 s | 0.22 s | — | 0.29 s | 0.66 s | 0.66 s | 18.6 MB | 47 | 2 |
| phone | unthrottled (median of 3) | dl-raim | 0.54 s | 0.54 s | 0.54 s | — | 0.60 s | 1.32 s | 1.32 s | 18.6 MB | 47 | 2 |
| phone | unthrottled (median of 3) | dl-lunar-time | 0.25 s | 0.25 s | 0.25 s | — | 0.33 s | 0.41 s | 0.41 s | 18.6 MB | 47 | 2 |
| phone | unthrottled (median of 3) | dl-clock-exports | 0.25 s | 0.25 s | 0.25 s | — | 0.32 s | 0.42 s | 0.42 s | 18.7 MB | 48 | 2 |
| desktop | Fast 3G + 4x CPU | landing | 2.12 s | 2.12 s | 8.85 s | 8.84 s | 57.15 s | 107.19 s | none (start screen) | 18.6 MB | 47 | 2 |
| desktop | Fast 3G + 4x CPU | dl-waterfall | 1.93 s | 1.93 s | 1.93 s | — | 57.10 s | 107.49 s | 107.49 s | 18.6 MB | 47 | 2 |
| desktop | Fast 3G + 4x CPU | dl-raim | 1.92 s | 1.92 s | 1.92 s | — | 57.18 s | 107.81 s | 107.81 s | 18.6 MB | 47 | 2 |
| desktop | Fast 3G + 4x CPU | dl-lunar-time | 1.92 s | 1.92 s | 107.07 s | — | 57.10 s | 107.07 s | 107.07 s | 18.6 MB | 47 | 2 |
| desktop | Fast 3G + 4x CPU | dl-clock-exports | 2.07 s | 2.07 s | 2.07 s | — | 57.09 s | 107.16 s | 107.16 s | 18.7 MB | 48 | 2 |
| phone | Fast 3G + 4x CPU | landing | 2.62 s | 2.62 s | 2.62 s | 9.09 s | 57.39 s | 107.45 s | none (start screen) | 18.6 MB | 47 | 2 |
| phone | Fast 3G + 4x CPU | dl-waterfall | 1.91 s | 1.91 s | 2.00 s | — | 57.15 s | 107.76 s | 107.76 s | 18.6 MB | 47 | 2 |
| phone | Fast 3G + 4x CPU | dl-raim | 1.94 s | 1.94 s | 1.94 s | — | 57.15 s | 107.82 s | 107.82 s | 18.6 MB | 47 | 2 |
| phone | Fast 3G + 4x CPU | dl-lunar-time | 1.94 s | 1.94 s | 1.94 s | — | 57.12 s | 107.08 s | 107.08 s | 18.6 MB | 47 | 2 |
| phone | Fast 3G + 4x CPU | dl-clock-exports | 1.92 s | 1.92 s | 1.92 s | — | 57.19 s | 107.30 s | 107.30 s | 18.7 MB | 48 | 2 |

Entries (all are real links copied from the site build `explorations/2026-09-27-combo`, which has 197 distinct Studio deep links):

| Id | Query string | Where it appears on the site |
|---|---|---|
| landing | (none) | Studio nav link, docs/studio.html |
| dl-waterfall | `?scenario=l-band-waterfall-jamming&tab=spectrum&play=1` | home page hero |
| dl-raim | `?scenario=integrity-raim&tab=overview` | most-linked deep link (13 links) |
| dl-lunar-time | `?scenario=lunar-time-budget&tab=overview` | lunar capability and mission cards (11 links) |
| dl-clock-exports | `?scenario=clock-holdover&tab=exports` | home page and developers "exports" links (7 links) |

**What the timing shows**
- The engine file `pkg/kshana_bg.wasm` is 8.4 MB uncompressed. Every cold visit downloads it **twice**: once when `app.js` compiles it on the main thread (`bootEngine`, `mod.default()`), and again when the Web Worker (`lib/engine-worker.mjs`, `init()`) loads it. That is 18.6 MB per first visit.
- On Fast 3G, the first download ends at about 57 s, when the engine chip turns "Live". The second download ends at about 107 s, and only then does the first run finish. The total is the same for every entry, so the network dominates.
- The staging server (`python3 -m http.server`) sends no compression and no `Cache-Control` header. For reference, the engine compresses to 2.73 MB with gzip at level 9, and to 1.87 MB with brotli at quality 11 (measured locally). That puts a production-like download at roughly a quarter of today's. This is a measured file size, not a measured production load.
- While the visitor waits, the phone shows a header, the placeholder title "Scenario", an empty chip, Pin/Compare/Share buttons, and the licence and Kshana Pro sales footer. No progress bar or "Loading" text is visible, although `#status` reads "Loading engine…" in the document. See `phone-fast3g-dl-raim-at-5s.jpg` and `phone-fast3g-dl-raim-at-20s.jpg`.
- The scenario's title and question are static data in `lib/catalog.mjs`, but they are only painted after the engine boots. Recorded runs of every bundled scenario also ship in `recorded/`. The Studio uses them only as a fallback when the WebAssembly fails, never as an instant first result.
- The landing start screen appears at 8.8–9.1 s on Fast 3G, after `app.js` (168 kB), the stylesheets and the web fonts load.

## 4. Density (first screen and each main view)

**Method** (`MEASURE` in baseline.mjs):
- **Controls:** `a[href]`, `button`, `input`, `select`, `textarea`, `summary`, and `[role=button|tab|menuitem|slider]`, not counting a control nested in another control.
- **On screen:** visible (CSS visibility, opacity and size) and inside the viewport after clipping by every scrolling ancestor. The app scrolls inside panels, so "above the fold" means "inside the visible part of its panel and the window".
- **Rendered on page:** visible styling anywhere, including content scrolled away.
- **Inputs:** text, number and range inputs, selects and text areas. Search boxes are counted separately in brackets.
- **Text blocks:** distinct non-inline elements that hold on-screen text.
- **Words:** every on-screen text node (any line box inside the viewport) split on whitespace.

| Device | View | Controls on screen | Controls rendered on page | Inputs on screen (search boxes) | Text blocks on screen | Words on screen | Screenshot |
|---|---|---|---|---|---|---|---|
| desktop | landing-start | 34 | 171 | 1 (2) | 97 | 533 | `desktop-landing-start.jpg` |
| desktop | dash-clock-holdover | 44 | 192 | 1 (1) | 146 | 544 | `desktop-dash-clock-holdover.jpg` |
| desktop | params-drawer | 51 | 201 | 5 (2) | 143 | 593 | `desktop-params-drawer.jpg` |
| desktop | params-all-fields | 77 | 227 | 11 (2) | 163 | 586 | `desktop-params-all-fields.jpg` |
| desktop | library | 44 | 192 | 1 (1) | 146 | 544 | `desktop-library.jpg` |
| desktop | dl-waterfall | 44 | 198 | 2 (1) | 156 | 552 | `desktop-dl-waterfall.jpg` |
| desktop | dl-raim | 45 | 179 | 1 (2) | 132 | 540 | `desktop-dl-raim.jpg` |
| desktop | raim-tab-overview | 35 | 179 | 1 (2) | 104 | 454 | `desktop-raim-tab-overview.jpg` |
| desktop | raim-tab-time-series | 35 | 179 | 1 (1) | 94 | 427 | `desktop-raim-tab-time-series.jpg` |
| desktop | raim-tab-sweep | 43 | 187 | 7 (2) | 111 | 490 | `desktop-raim-tab-sweep.jpg` |
| desktop | raim-tab-animation | 31 | 182 | 1 (1) | 65 | 365 | `desktop-raim-tab-animation.jpg` |
| desktop | raim-tab-engine-report | 31 | 182 | 1 (1) | 65 | 371 | `desktop-raim-tab-engine-report.jpg` |
| desktop | raim-tab-exports-report | 39 | 194 | 1 (1) | 121 | 558 | `desktop-raim-tab-exports-report.jpg` |
| desktop | raim-tab-json | 35 | 182 | 1 (2) | 78 | 439 | `desktop-raim-tab-json.jpg` |
| desktop | dl-lunar-time | 41 | 175 | 1 (2) | 114 | 544 | `desktop-dl-lunar-time.jpg` |
| desktop | dl-clock-exports | 46 | 195 | 1 (1) | 128 | 562 | `desktop-dl-clock-exports.jpg` |
| phone | landing-start | 13 | 31 | 0 (1) | 32 | 115 | `phone-landing-start.jpg` |
| phone | dash-clock-holdover | 13 | 33 | 1 (0) | 36 | 129 | `phone-dash-clock-holdover.jpg` |
| phone | params-drawer | 13 | 21 | 4 (1) | 27 | 87 | `phone-params-drawer.jpg` |
| phone | params-all-fields | 32 | 47 | 8 (1) | 40 | 72 | `phone-params-all-fields.jpg` |
| phone | library | 12 | 31 | 0 (0) | 26 | 128 | `phone-library.jpg` |
| phone | dl-waterfall | 23 | 43 | 1 (0) | 43 | 85 | `phone-dl-waterfall.jpg` |
| phone | dl-raim | 19 | 27 | 0 (1) | 40 | 73 | `phone-dl-raim.jpg` |
| phone | raim-tab-overview | 9 | 27 | 0 (1) | 30 | 49 | `phone-raim-tab-overview.jpg` |
| phone | raim-tab-time-series | 14 | 30 | 0 (0) | 32 | 159 | `phone-raim-tab-time-series.jpg` |
| phone | raim-tab-sweep | 16 | 33 | 6 (0) | 22 | 123 | `phone-raim-tab-sweep.jpg` |
| phone | raim-tab-animation | 8 | 28 | 0 (0) | 12 | 82 | `phone-raim-tab-animation.jpg` |
| phone | raim-tab-engine-report | 8 | 28 | 0 (0) | 11 | 88 | `phone-raim-tab-engine-report.jpg` |
| phone | raim-tab-exports-report | 10 | 40 | 0 (0) | 27 | 176 | `phone-raim-tab-exports-report.jpg` |
| phone | raim-tab-json | 11 | 28 | 0 (0) | 16 | 164 | `phone-raim-tab-json.jpg` |
| phone | dl-lunar-time | 19 | 25 | 0 (1) | 40 | 77 | `phone-dl-lunar-time.jpg` |
| phone | dl-clock-exports | 21 | 31 | 0 (0) | 40 | 95 | `phone-dl-clock-exports.jpg` |

**Reading it.** At 1440 px, the desktop first screen of every view carries 31–77 controls and 365–593 words. The always-open 139-item library rail accounts for a large share. The desktop "library" view is identical to the dashboard (44/44 controls, 544/544 words), because the Library button does nothing visible at 1440 px: the rail is already open. Full-page captures are the `*-full.jpg` files.

## 5. Clicks to a first meaningful result (three newcomer tasks)

**Method.** Each task starts from the bare landing in a fresh context. The script clicks the most obvious visible target, records the scroll distance needed to reach it (`top + height − viewport height`), and times the result appearing after the last click. Tasks were chosen from what the Studio actually offers. **There is no GNSS sky plot, and no scenario that takes "my location" as its first input.** So task 1 uses the coverage map, which is the nearest thing.

| Task | Device | Clicks | Steps (scroll needed before the click) | Result after last click | Where a newcomer must guess |
|---|---|---|---|---|---|
| T1 See where GNSS satellites are visible | desktop | 1 | "Good first run: Map the coverage of four GNSS constellations" (0 px) | 0.08 s | none, it is a suggested run. To set "my place", there is no guided control; the user must edit fields. |
| | phone | 1 | same (0 px) | 0.06 s | same |
| T2 Propagate an orbit | desktop | 2 to a result, 3 to the orbit picture | domain tile "Orbits & GNSS geometry" (7 px) → "SGP4 orbits" (0 px) → tab "3-D orbit" | 0.06 s | (1) three domains are plausible: Orbits & GNSS geometry, Constellation design, Mission analysis & space operations. (2) The user must know that "SGP4" means orbit propagation; "Ground track" is equally plausible. (3) The orbit picture is not the default panel. |
| | phone | same | tile needs **1013 px** of scrolling | 0.07 s | same |
| T3 Open a lunar navigation scenario | desktop | 2 | tile "Moon, cislunar & Mars" (**286 px**) → "Lunar integrity" (0 px) | 0.04 s | 22 lunar, cislunar and Mars scenarios, none marked as a starting point. The opened result shows "Samples available 0" with no explanation (`desktop-T3-lunar-navigation-step2.jpg`). |
| | phone | 2 | tile needs **1688 px** of scrolling | 0.05 s | same |

Screenshots: `<device>-T1-gnss-coverage-step1.jpg`, `<device>-T2-propagate-orbit-step{1,2,3}.jpg`, `<device>-T3-lunar-navigation-step{1,2}.jpg`.

**Search route** (type into the start-screen search; results appear after 120 ms):

| Query | Scenario hits | Field hits | First hit shown | Right answer's position |
|---|---|---|---|---|
| coverage | 7 | 0 | "Real two-line elements" | the coverage map is not first (`desktop-search-coverage.jpg`) |
| orbit | 50 (12 shown) | 24 (of 48 scenarios) | **"Jammer margin from received power"** | "SGP4 orbits" is 4th (`desktop-search-orbit.jpg`) |
| lunar | 23 (12 shown) | 8 | "Lunar relay constellation" | acceptable (`desktop-search-lunar.jpg`) |

## 6. Wayfinding: where am I, what am I looking at, what do I do next

Evidence comes from `results.json → views[].way`: breadcrumbs (`#crumbs`), the visible `h1`, the scenario question (`#sc-question`), the step marked `aria-current`, and the control marked `.is-next`. Each cell was scored yes or no against the screenshot.

| View | Where am I? | What am I looking at? | What do I do next? | Evidence |
|---|---|---|---|---|
| Landing, desktop | Yes | Yes | **No** | Crumb "Studio", h1 "What do you want to do?" and a lede are present. No control is marked next. 34 controls compete: a 5-step explainer, search, 4 suggested runs, 16 tiles and a 139-item rail. The History badge already reads "1" on a first visit, because of the hidden default run. |
| Landing, phone | Yes | Yes | Yes | The first suggested run is on screen. The domain tiles start below 1000 px. |
| Deep-link dashboard, desktop | Yes | **No** | Yes | Crumbs "Studio › Integrity & positioning › RAIM integrity › Overview", the h1 and the question are present. But the Key figures do not answer the question: on RAIM, 3 of 5 cards are inputs echoed back (Al h 40 m, Al v 50 m, Alert limit 50 m). "Share or export" is marked next. |
| Deep-link dashboard, phone | Yes | **No** | Yes | The question is hidden on phone (`sc-question` not visible in any phone view). The first Key figure on the waterfall link is an input ("Tracking threshold 25 dB-Hz"). "Share or export" is marked next and is on screen. |
| A result panel after switching tabs (desktop and phone) | Yes | Yes | **No** | Crumbs update to the tab name. Card titles explain the chart. No next action is marked on screen (`.is-next` off-screen in all 14 tab views). |
| Parameters drawer, desktop | Yes | Yes | Yes | "2 Set parameters" plus a hint, but the step bar still says "4 Read results", which contradicts it. Opening the drawer at 1440 px squeezes the header (§9). |
| Parameters drawer, phone | Yes | Yes | **No** | The drawer is a fixed full-screen sheet (844 px tall), so a change and its effect are never on screen together. |
| Library, desktop | **No** | Yes | Yes | At 1440 px the Library button changes nothing: the rail is already open. |
| Choose (library), phone | Yes | Yes | Yes | Returns to the start screen. |

## 7. Inventory

From `inventory.mjs`, which uses the Studio's own modules, and from the data files.

| Item | Count | Source |
|---|---|---|
| Domains with scenarios | 16 (15 built in, plus the optional Celeste group) | `lib/catalog.mjs DOMAINS`, `groups.json` |
| Scenarios in the library | 139 (134 bundled, plus 5 Celeste) | `SCENARIOS` + `celeste/index.json` |
| Scenarios largest per domain | Moon, cislunar & Mars 22; LEO navigation 22; Inertial & alternative navigation 16; LEO missions 15 | `inventory.json perDomain` |
| Scenarios marked "not in browser" | 2 | `NOT_IN_BROWSER` |
| Engine kinds with a purpose-built result view | 13 | `lib/kinds.mjs CAPABILITIES` |
| Product capabilities listed for the site | 46 (17 validated, 29 modelled) | `data/capabilities.json`. **Not loaded by the Studio.** |
| Verification-matrix rows | 223 (83 validated, 136 modelled, 4 partner-owned) | `data/verification-matrix.json`. Loaded only to print a count line in the library footer. |
| Default scenario (runs behind the start screen) | `clock-holdover.toml` | `DEFAULT_SCENARIO` |

**Inputs per scenario.**
- Guided sliders: median 2, mean 2.1, maximum 6. The cap of 6 is in `knobsForToml`.
- All numeric fields: median 13, mean 19.7, maximum 252 (`campaign-jam-spoof-holdover-integrity`, which is one of the four "Good first runs").
- 113 of 139 scenarios have more than 5 numeric fields.
- 46 scenarios have **no** guided sliders, and their Parameters panel opens on "All fields" (`setPview(anyKnob ? "guided" : "all")`). 23 of those show more than 5 inputs.
- 10 scenarios have **zero** tunable fields, including the deep-link target `lunar-time-budget`. For those, step 2 "Set" leads nowhere.
- On the first screen, with Parameters closed, 0–2 inputs are visible. Parameters starts closed below 1680 px wide, and always on phones.

**Importance or "advanced" metadata.** None exists as data.
- `groups.json`, the scenario TOML files and `capabilities.json` have no importance, priority, basic/advanced or level field.
- The only de facto importance signal is code: the 13 candidate guided knobs in `lib/guided.mjs GUIDED_KNOBS`, taken in catalogue order up to 6, and per-kind `guidedFor()` in `lib/kinds.mjs`.
- TOML comment lines (median scenario: 53 lines total) describe fields in prose. They could seed such metadata, but they are not structured.

**Validation status as data.**
- Per figure: yes. Each run's `figure_tiers` block feeds the "Figures of merit" table and the MODELLED/VALIDATED chips on Key figures (`lib/tabs.mjs figureTier`).
- Per capability: no. `capabilities.json` (with `status`) and `oracle-references.json` (31 references) ship in `data/`, but `app.js` never fetches them. Scenarios are not linked to capabilities in the Studio. A researcher cannot see "what Kshana can do and how well it is validated" at a glance inside the Studio.

## 8. Phone problems (390×844, touch)

**Method** (`MEASURE.phone`):
- **Page sideways scroll:** `scrollingElement.scrollWidth > innerWidth`.
- **Boxes needing a sideways swipe:** scrollable boxes (`overflow-x: auto|scroll`) whose content is wider than the box. The figure is the share of content hidden.
- **Small targets:** visible controls narrower or shorter than 44 px.
- **Off-screen controls:** displayed controls whose box lies wholly left or right of the viewport.
- **Clipped text:** `overflow: hidden` or ellipsis with `scrollWidth > clientWidth`.
- **Fixed bars:** `position: fixed` or `sticky` elements, with the height they cover.

| Device | View | Header height | Fixed/sticky bars (height px) | Boxes needing a sideways swipe (share hidden) | Targets under 44 px (on screen) | Off-screen controls | Page sideways scroll |
|---|---|---|---|---|---|---|---|
| phone | landing-start | 133 | mob 65 | none | 3 (2) | 1 | no |
| phone | dash-clock-holdover | 115 | tabs-wrap 56, mob 65 | figs 75%, tabs 63%, ho-chart 40% | 17 (5) | 6 | no |
| phone | params-drawer | 115 | editor-pane 844, mob 65 | none | 11 (3) | 1 | no |
| phone | params-all-fields | 115 | editor-pane 844, mob 65 | none | 21 (11) | 1 | no |
| phone | library | 115 | mob 65 | none | 7 (0) | 1 | no |
| phone | dl-waterfall | 179 | tabs-wrap 56, mob 65 | figs 75%, tabs 59%, subtabs 80%, chart-box 40% | 9 (3) | 14 | no |
| phone | dl-raim | 179 | tabs-wrap 56, mob 65 | figs 70%, tabs 53%, table-scroll 30% | 6 (2) | 4 | no |
| phone | raim-tab-overview | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53%, table-scroll 30% | 13 (1) | 4 | no |
| phone | raim-tab-time-series | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53%, ts-live 40% | 16 (6) | 4 | no |
| phone | raim-tab-sweep | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53% | 19 (8) | 4 | no |
| phone | raim-tab-animation | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53% | 14 (0) | 3 | no |
| phone | raim-tab-engine-report | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53% | 14 (0) | 4 | no |
| phone | raim-tab-exports-report | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53%, table-scroll 14%, geo-chart 40% | 23 (0) | 4 | no |
| phone | raim-tab-json | 147 | tabs-wrap 56, mob 65 | figs 70%, tabs 53%, json 70% | 14 (3) | 4 | no |
| phone | dl-lunar-time | 179 | tabs-wrap 56, mob 65 | figs 51%, tabs 37%, table-scroll 25% | 6 (2) | 2 | no |
| phone | dl-clock-exports | 179 | tabs-wrap 56, mob 65 | figs 75%, tabs 63% | 5 (1) | 6 | no |

**Findings.**
- No page-level sideways scroll and no clipped-text elements were detected.
- **Content hidden sideways instead:** the Key-figures strip hides 51–75% of its cards, the panel tab strip hides 37–63%, and the waterfall sub-tabs hide 80%. Result tables hide 14–30% of their columns (the "Provenance" column is cut in `phone-dl-raim.jpg`).
- The header wraps to 3 rows: brand, then 4 buttons, then breadcrumbs. On deep links it is 179 px tall, 21% of the screen.
- The fixed bottom bar (65 px) plus the sticky tab strip (56 px) cover 121 px, 14% of the screen, at all times on the dashboard.
- Typical small targets: the home link (89×24), search boxes (26 px tall), the Guided / All fields toggle (26 px), "Interactive / Engine chart" (26 px), SVG/PNG download buttons (46×26), the holdover budget slider (22 px tall), All-fields steppers (84×28) and a footer link (16 px tall).
- On phone the Run, Parameters and Library buttons are moved into the bottom bar, so only Pin, Compare and Share remain in the scenario header.

## 9. Researcher layer

| Feature | Present? | Evidence |
|---|---|---|
| Copy this run as a command-line call | **Absent** | No per-run command. The Scenario-file card says "Run it again anywhere with the command-line tool", but gives no command. The footer's install-command chips come from `channels.json`, which returns 404 on 8853, so they are hidden there. They work on the site copy. |
| Copy as Python | **Absent** | No Python snippet anywhere (3 mentions of "python" in app.js, none a snippet). |
| Data export | Present | Exports tab: an HTML reproducible report, the full JSON result, the TOML scenario, and a CSV table where the run has one. Interoperability table: CZML, KML, GeoJSON, STK ephemeris, SigMF. SP3, OMM and OEM for orbit runs. JSON copy/download on the JSON tab. |
| Figure export | Present | SVG and PNG buttons under every chart (`mountTools`). |
| Citation / DOI / "how to cite" | **Absent** | No match for cite, citation, DOI, BibTeX or Zenodo in app.js or index.html. |
| Method and reference text next to results | Partial | Card notes explain each chart. The figure tiers (validated or modelled) are shown with their definition. Some runs carry an "honesty" label paragraph (`desktop-dl-lunar-time.jpg`). The Engine report tab is available. The oracle references (`data/oracle-references.json`, 31 entries) are not shown, and no per-scenario literature reference appears beside results. The source TOML header comments, which do cite sources (for example Kaplan & Hegarty), are only visible by opening the Scenario source editor. |
| Share link / embed | Present | Share menu: copy share link, embed code, embed preview. |
| Compare and sweep | Present | Pin up to 4 runs and compare them. The Sweep tab runs up to 60 engine runs, in 1 or 2 dimensions. |

## 10. Console errors and failed network requests

- Staging (8853), on every load and both devices: **2 console errors, both HTTP 404**:
  - `assets/brand/kshana-mark-mask.png`, requested as `../assets/...` from the brand mark. The file is outside the studio-dash folder, so the logo is blank.
  - `channels.json`, which is missing from staging, so the install-command chips are hidden.
- No uncaught page errors and no failed (aborted) requests.
- Site copy (8841 `/playground/`): 0 console errors, 0 HTTP errors and 0 page errors on all 10 loads (5 entries × 2 devices). Deep-link results appeared 0.6–3.6 s after navigation, unthrottled.

---

## 11. The 10 worst newcomer blockers (ranked)

Each item is "observed problem → likely cause", with a screenshot.

1. **A phone visitor on a slow link waits 107 s for the first result and sees no progress.** `phone-fast3g-dl-raim-at-20s.jpg`. → The 8.4 MB uncompressed engine is downloaded twice, once on the main thread and once in the worker. Staging sends no compression and no caching headers. The title and the recorded result (both static) are held back until the engine boots. The "Loading engine…" status is not visible on phone.
2. **The landing shows no result and offers about five competing ways in at once.** These are a 5-step explainer, search, 4 suggested runs, 16 domain tiles and a 139-item rail: 34 controls and 533 words on the desktop first screen, with nothing marked as next. `desktop-landing-start.jpg`. → The start screen is a catalogue rather than a first result. The default run is computed (0.81 s) but hidden behind it.
3. **Key figures echo inputs instead of answering the scenario's question.** RAIM shows 3 inputs out of 5 cards. The waterfall's first card is the tracking threshold, an input. Lunar integrity leads with the alert limit and range error, both inputs, and then "Samples available 0" with no verdict. `desktop-dl-raim.jpg`, `phone-dl-waterfall.jpg`, `desktop-T3-lunar-navigation-step2.jpg`. → Cards are picked generically from result fields (`lib/kpi.mjs`) with no "answer" field per kind.
4. **On phone the result is pushed down and half-hidden sideways.** The header is 179 px (3 rows), and 14% of the screen is covered by fixed bars. The Key-figures strip hides 51–75% of its cards, the tabs hide 37–63%, and tables are cut. The scenario question is hidden. Deep links show only 73–95 words of screen content. `phone-dl-raim.jpg`. → The desktop layout is stacked rather than re-thought for phone.
5. **Jargon and raw identifiers on first sight.** "Al h", "Al v", "kind: integrity", file paths, "Crossover tau", "HPL / VPL", "SGP4", raw field names (`al_h_m`, `time.step_s`), device codes ("csac-sa45s"). `desktop-dl-raim.jpg`, `desktop-dl-lunar-time.jpg`, `phone-params-all-fields.jpg`. → Labels are derived from engine keys (`keyLabel`), and the developer chips (kind, file) are shown to everyone. This conflicts with the house rule that every abbreviation is spelled out.
6. **Search ranks the wrong scenario first.** "orbit" puts "Jammer margin from received power" first, out of 50 scenarios plus 24 field hits. "coverage" puts "Real two-line elements" first, not the coverage map. `desktop-search-orbit.jpg`. → Library order rather than relevance (`groupedLibrary`), and field matches are mixed in.
7. **Browsing to a task needs domain knowledge and long scrolls, with no starting point per domain.** Three domains are plausible for "orbit". 22 lunar scenarios have no recommended first one. The lunar tile sits 1688 px down on phone. No sky plot or "my location" scenario exists. `phone-T3-lunar-navigation-step1.jpg`, `desktop-T2-propagate-orbit-step1.jpg`. → The taxonomy is by engine domain, not by newcomer question. There is no per-domain "start here" flag.
8. **Parameters are either absent or overwhelming.** 46 of 139 scenarios open straight into raw "All fields" (up to 43 inputs). 10 scenarios have nothing to set, which leaves step 2 a dead end. On phone the sheet covers the whole screen, so cause and effect are never visible together. `desktop-params-all-fields.jpg`, `phone-params-drawer.jpg`. → There is no input-importance metadata; guided knobs come from a fixed list of 13 key names capped at 6.
9. **Researchers cannot see "everything Kshana can do, and how well it is validated" at a glance.** The 46-capability list with validated/modelled status, and the 31 oracle references, ship in `data/` but are never loaded. Validation appears only per figure, after a run. There is no citation/DOI and no copy-as-command or Python. See §7 and §9. → The Studio was built scenario-first, and the capability data lives only on the site pages.
10. **Contradictory or broken state signals.**
    - At 1440 px, opening Parameters squeezes the header: the title wraps, the kind chip wraps to 3 lines, and the "Ran locally at…" status overlaps the step buttons.
    - The step bar says "4 Read results" while the drawer says "2 Set parameters".
    - The Library button does nothing visible at 1440 px.
    - History shows "1" on a first visit.
    - On staging the brand mark is blank (404).

    Screenshots: `desktop-params-drawer.jpg`, `desktop-landing-start.jpg`. → Layout breakpoints assume a viewport of 1680 px or wider when the drawer is open. The hidden default run is counted as a user run.

## 12. Caveats

- All timing is against localhost Python servers. Unthrottled numbers are best-case. The Fast 3G figures model the network and CPU, not a real phone's WebAssembly compile speed. CPU throttling applies to the page; whether Chromium also slows the dedicated worker is not verified.
- Production (kshana.dev) compression and caching were not measured. The `/playground/pkg/kshana_bg.wasm` URL there returned 404, so the deployed layout differs from this staging build. The gzip/brotli sizes above are local file measurements.
- Word and text-block counts are approximate. A text node counts in full if any of its line boxes is on screen.
- The wayfinding scores are a single reviewer's judgement against the recorded evidence. They are not user-tested.
