# Display matrix audit

`run_matrix.mjs` implements the "Display matrix" standing rule in `GOAL.md`. It serves the built
site read-only with `python3 -m http.server` on a free port, drives Chromium, WebKit and Firefox
through Playwright, and grades every page and Studio view under every display condition the rule
lists. It never writes anything under the site.

## Usage

```sh
cd explorations/2026-09-27-audit/matrix
npm install                       # playwright 1.60.0 (matches chromium-1223, webkit-2287, firefox-1522 in ~/Library/Caches/ms-playwright)
node run_matrix.mjs --quick       # 10 key pages, key sizes, Chromium only, 40 px sweep (a few minutes)
node run_matrix.mjs --full        # everything, three engines, 4 browser contexts at a time, iOS Simulator smoke
node run_matrix.mjs --fixtures    # the mutation fixtures + clean control, prints the catch table
```

Options: `--browsers=chromium,webkit,firefox`, `--pages=index,docs/*,studio*` (glob on view ids),
`--groups=sweep,themes,...` (condition groups below), `--conc=4`, `--sweep-step=10`, `--out=DIR`,
`--no-ios`, `--ios` (run the simulator smoke outside `--full`), `--no-shots`, `--site=DIR`,
`--job-timeout=MIN` (per-job limit; default 40 in full mode, 12 in quick; raise it on a loaded machine).
`--probe=FILE` runs another in-page probe file (default `lib/probe.js`). The probe is read at every
browser-context creation, so editing `lib/probe.js` changes a run already in flight; put a new version
in its own file until running jobs finish.

Output goes to `out/<mode>-<stamp>/`:

- `report.json`: every issue with page, view URL, engine, size, theme, zoom, motion, condition, category, CSS selector, text, detail, document rect and screenshot crop; plus counts per category, engine, group and page, the top issue types, tool errors and notes.
- `report.md`: the same counts in tables, the top 10 issue types, the first 400 unique issues with crop links, and (fixtures mode) the catch table.
- `crops/`: one crop per unique issue (page + engine + category + element), taken from the live page at the moment of detection. Console errors link a thumbnail of the page.
- `shots/<page>/chromium-<W>x<H>-<light|dark>.jpg`: every page at 390×844, 768×1024, 1440×900 and 1920×1080 in both themes (full page, first 16,000 px). `shots/cvd/` holds renders through the colour-vision filters; `shots/ios-simulator/` the simulator screenshots.
- `progress.log`.

The exit code is 1 when there is any issue or tool error (fixtures mode: 1 unless every fixture is caught and the control is clean).

Counting: a **run** is one page×condition×engine measurement (one width of the sweep, one device size, one theme state, one zoom level, one colour-vision simulation...). An **issue** is unique per page + engine + category + element; its **occurrences** are the runs that saw it.

## The matrix

Views: the six top pages, every `docs/*.html`, and six Studio views (the default plus five scenario/tab deep links). Each view runs these groups in each engine:

| Group | What runs | Checks |
|---|---|---|
| devices | the fixed device list in both orientations (phones, tablets, laptops, desktops, 3840×2160, 3440×1440) plus full-screen browser windows 1512×982, 1728×1117, 1920×1200; phones and tablets in a touch/mobile context | whole-document layout, CSS contrast, 24×24 touch targets on phone sizes; once per view: hover-only reveals and colour-only status |
| sweep | every 10 px from 280 to 2560 at heights 900 and 500, resized live in one page | horizontal scroll, content outside the viewport, overlap, clipped text; hits are merged into width ranges |
| heights | heights 400, 500, 600, 800, 1600 at widths 1024 and 1440 | viewport overlap/occlusion by fixed bars at top, 25/50/75 % and end; in-page anchors landing under the fixed bar; fixed chrome covering over 40 % of the window |
| resize | 1440×900 → 390×844 → 1024×768 → 1920×1080 → 768×1024 → 2560×1440 → 320×568 → 1280×800 with the page open; on Studio views a run is started and the window is resized while it is in flight | every canvas and SVG chart visited: blank, stale backing size, horizontal scroll, console errors |
| zoom | browser zoom 50, 80, 100, 125, 150, 200, 300, 400 % emulated as CSS viewport = 1280×800 / zoom at deviceScaleFactor = zoom (what browser zoom does to layout); plus 320×256 CSS px (1280 at 400 %) reflow | horizontal scroll, outside viewport, overlap, clipping; fixed chrome over 50 % at 200 % and above |
| text | text-only 200 % (every font size and pixel line-height doubled) at 1280×800; WCAG 1.4.12 text spacing (line-height 1.5, letter 0.12 em, word 0.16 em, paragraph 2 em) at 1280×800 and 390×844 | clipping, overlap, horizontal scroll, outside viewport |
| fullscreen | the Fullscreen API on up to three canvases (globe, planets), two SVG charts and, in the Studio, the results pane: enter through a real click, check, exit, check | entered, fills the screen, not blank, canvas backing resized, restored to its original size after exit, no errors |
| themes | at 1440×900 and 390×844: system light (no stamp, prefers light), system dark (no stamp, prefers dark), `data-theme="light"` under prefers dark, `data-theme="dark"` under prefers light; at 1440 also the toggle light→dark→light and dark→light→dark mid-page | theme actually applied; text contrast on rendered pixels; 3:1 non-text contrast; invisible marks; blank canvases/charts; fixed-bar occlusion |
| cvd | light and dark, normal vision + protanopia, deuteranopia, tritanopia | chart palettes and legends distinguishable (CIEDE2000) |
| forced | forced-colors: active under dark and light (Chromium only) | pixel-only text contrast, marks, blank graphics, a keyboard pass |
| motion | prefers-reduced-motion reduce and no-preference at 1440×900 and 390×844 | layout in both; any text visible in one state and hidden in the other |
| keyboard | Tab through every focusable element (cap 160) in light and dark | focus visible, focus indicator contrast, focus hidden behind fixed bars (2.4.11), traps |
| shots | Chromium, the key sizes in both themes | evidence only, no checks |

After the engines, `--full` opens the top pages, `docs/index` and the Studio in Safari on the iOS Simulator (`xcrun simctl`, iPhone 17 Pro). That is a smoke test: Safari has to show the page (screenshot differs from the home screen and from the previous page, and is not one flat colour). There is no DOM access there, so the matrix itself runs on desktop WebKit with a touch/mobile context for phones and tablets ("WebKit mobile emulation").

## What each check measures

Detectors come from `../tool/audit.mjs` and keep its false-positive refinements: text and controls inside a fixed or sticky layer are not compared with content scrolling under it; full-width fixed bars are judged only at the document's ends; visually hidden screen-reader text (`clip: rect(0…)`, `clip-path: inset(50%)`, 1 px boxes) is ignored; text over a decorative graphic is fine when the text is painted on top; the Studio is never measured mid-run (every measurement waits for the Run button to be enabled and a result or an explanation to be showing). All in-page code is `lib/probe.js`; pixel maths is `lib/pix.mjs`.

- **hscroll**: the document is wider than the viewport. The causes are named: elements that stick out past the right edge while their parent does not, and that no inner clipping or scrolling box contains. In a phone or tablet context the same overflow widens the layout viewport (`innerWidth` grows past the screen) even when the root clips overflow, so the page pans sideways: that is reported as hscroll too, naming the elements that stick out past their own parent (aria-hidden ones included), not the fluid blocks and fixed bars that merely stretch with the widened viewport.
- **outside-viewport**: the root hides horizontal overflow (`overflow-x: clip/hidden` on html or body), so an element past the edge is simply lost. Ignored: elements parked entirely off to the left (skip links, a closed left drawer) and `aria-hidden`/`inert` subtrees. A panel parked past the right edge is still reported: even `position: fixed` it widens the document (`scrollWidth`), and in a mobile context it widens the layout viewport so the whole page pans sideways (checked 2026-09-28: a 200 px fixed panel at `translateX(100%)` made `innerWidth` 443 on a 320 px phone). Every resize waits for running CSS transitions and finite animations (up to 1 s, 0.6 s per sweep step), so a drawer sliding shut at a breakpoint is not measured half-way.
- **overlap**: two content boxes (text runs measured per line with Range rects, controls, graphics) from unrelated elements in the same layer intersect by at least 3×3 px. A text box is the font's content area, taller than the ink, so text is judged on its ink band (the box less 15 % of its height at top and bottom): tightly set lines whose boxes share a few pixels with no glyph touching are not reported.
- **clipped**: a box with `overflow: hidden/clip`, `text-overflow: ellipsis` or line clamping whose text runs past its edge.
- **occluded**: a fixed or sticky element covers text (hit-tested), except full-width bars away from the document ends.
- **anchor-hidden**: jumping to an in-page anchor puts its target under the fixed top bar.
- **chrome-coverage**: fixed and sticky chrome covers more than 40 % of a short window (50 % when zoomed to 200 % or more).
- **contrast-text** (WCAG 1.4.3): 4.5:1, or 3:1 for text ≥ 24 px or bold ≥ 18.66 px. In theme runs the background is read from pixels: glyphs are hidden with `-webkit-text-fill-color: transparent` (borders and currentColor icons stay), the viewport is captured, and up to six points per text line are sampled where hit-testing confirms the element is on top; the lowest third of those samples decides, so text on images, gradients and translucent layers is judged where it is worst. The glyph colour comes from CSS with the element's effective opacity. Windows are visited at the top, the end, every place CSS cannot answer (image, gradient, translucent or positioned layer, SVG text) and every chart, canvas and legend, up to 14 windows per state; everything else gets the CSS verdict (first opaque background, alpha-composited). Detail lines say `[pixels]` or `[CSS]`. In forced-colors mode the ground comes from pixels (the modal colour of the text box) and the glyph is read two ways: the computed colour (Chromium holds the forced system colour there, or the author colour where `forced-color-adjust` is `none`/`preserve-parent-color`, as for SVG text) and the third-strongest glyph pixel; text fails only when both fail. Pixels alone under-read small anti-aliased text (10 px ticks in #8088A6 read as #6B728C, 4.25:1 instead of 5.8:1) and colour emoji; the computed colour alone misses link text, which Chromium paints in LinkText while `getComputedStyle` keeps the author colour. The detail line prints both.
- **contrast-nontext** (WCAG 1.4.11): 3:1. SVG chart marks (lines, points, bars; not gridlines, backgrounds, halos or marks under 35 % opacity) are grouped by chart and colour; the marks are hidden and the ground behind them sampled at points along each path, and the mark colour (with its opacity) is composited on that ground. Legend swatches and icon-only controls are judged from pixels alone: every pixel that changes when the graphic is hidden, 90th-percentile contrast against what is behind it. Canvases: the contrast the top 0.5 % of pixels reach against the canvas's most common colour.
- **Stable pixel verdicts**: when the pixel text-contrast or mark-contrast pass (contrast-text, contrast-nontext, invisible-mark) fails in a window, the window is measured again 450 ms later and only failures seen both times count. Pages that animate (auto-playing players, typing terminals, pulse rings, fades) otherwise pair one frame's colour with another frame's ground. The report's notes give the number dropped.
- **Marks that move or sit under a fixed layer**: each mark's box is read before and after the two captures; a mark group that moved more than 1 px in between (a replay dot riding its line, a pulse ring) is not judged, and the report's notes give the number skipped. Sample points under a fixed or sticky layer the mark is not part of (the site header) are dropped: hiding the mark changes nothing there, which is not "invisible".
- **invisible-mark**: a mark, swatch or icon that changes no pixel (or under 10 levels) when hidden.
- **canvas-blank / svg-blank**: 99.5 % (canvas) or 99.7 % (SVG) of the element's pixels are within 12 levels of one colour.
- **stale-size**: a canvas backing store whose aspect ratio is more than 4 % off its displayed box, or under 60 % of its displayed width (stretched or not re-sized after a resize or full screen).
- **fullscreen**: request refused, element not filling the screen, one flat colour in full screen, or not restored after exit.
- **theme-state / theme-toggle**: the body background is not dark (or light) for the requested state, or the toggle did not change it.
- **palette / cvd**: every pair of series colours in a chart and its legend (flattened on the page background) must be at least ΔE2000 10 apart for normal vision and 8 apart after simulation, using the Machado, Oliveira and Fernandes (2009) matrices at full severity in linear RGB. Pairs told apart by dash pattern or mark shape are exempt. The same matrices are applied to the rendered page as SVG `feColorMatrix` filters for the crops and `shots/cvd/`. Heatmaps (over 150 cells) are continuous scales and are not checked here.
- **motion-hidden**: after a slow scroll through the whole page, a text element visible under one prefers-reduced-motion setting is hidden (opacity, visibility, clipping by an `overflow: hidden/clip` ancestor) under the other. A scrolling ancestor (`overflow: auto/scroll`) does not count as hiding: what it shows depends on its scroll position, not on motion (from `lib/probe-r4.js`).
- **focus-visible / focus-contrast / focus-obscured / focus-trap** (2.4.7, 1.4.11, 2.4.11, 2.1.2): for each Tab stop the element's box plus 8 px is captured focused and unfocused. Fewer than min(half the perimeter, 40) pixels changing is "no visible indicator"; fewer than that many changing with 3:1 against the unfocused pixel is "too faint". Obscured means every hit-test point inside the element lands on a fixed layer. A trap is Tab not moving, or focus cycling back before 80 % of the page's focusable elements were reached. WebKit uses Alt+Tab (Safari's Tab skips links).
- **touch-target** (2.5.8, phones only): under 24×24 CSS px and closer than the spacing exception allows (a 24 px circle on its centre hits another target or another small target's circle). Links inside a sentence are exempt.
- **hover-only**: a same-origin CSS rule reveals a descendant on `:hover` with no `:focus`/`:focus-within` twin and no `(hover: hover)` guard.
- **colour-only**: an element with a status class (good, warn, critical, pass, fail...) that has no text, label or title and sits in a textless parent.
- **console**: console errors, uncaught exceptions, failed requests and HTTP 4xx/5xx responses during any run.
- **ios-smoke**: the simulator did not show the page.

## Proof: mutation fixtures

`fixtures/make_fixtures.py` builds the fixtures from `fixtures/base/`, a frozen copy of the site's `docs/index.html` with its CSS and JS, so they stay put while the site changes. Every fixture adds the same harness section (a two-series SVG chart with a legend, a canvas, buttons, a label box, a revealed paragraph) in its correct form; `00-control.html` is only that and must come out with zero issues. The base carries two patches for real site defects that would otherwise make the control fail, both also reported in the baseline: the header's Studio button overflows at 280–360 px, and at text-only 200 % the header runs 376 px past the edge. The harness is also written the correct way for forced colours (`forced-color-adjust: none` on swatches) and full screen (a ResizeObserver redraws the canvas): the first control run caught both, which is why they are there.

| Fixture | Injected defect | Must report |
|---|---|---|
| 01-hscroll-390 | a 600 px fixed-width element | hscroll or outside-viewport on `#fx-wide` (this site's body has `overflow-x: clip`, so it is content lost past the edge rather than a scrollbar) |
| 01b-hscroll-390-unclipped | the same element with the root's overflow clipping removed | hscroll on `#fx-wide` |
| 02-overlap-768 | two captions overlapping between 700 and 800 px | overlap on `#fx-b` |
| 03-grey-dark-only | `#3A3F4F` text in the dark theme only | contrast-text on `#fx-grey` |
| 04-line-equals-dark-bg | a chart line stroked in the dark background colour | invisible-mark or contrast-nontext on `#fx-line2` |
| 05-canvas-blank-after-resize | a canvas cleared on resize and never redrawn | canvas-blank on `#fx-canvas` |
| 06-focus-outline-removed | `outline: none` on a button's focus | focus-visible or focus-contrast on `#fx-btn` |
| 07-touch-16px | two 16 px buttons 2 px apart | touch-target on `#fx-t1` |
| 08-clip-at-200-text | a 22 px fixed-height box | clipped on `#fx-clip` |
| 09-legend-deuteranopia | red/green series and legend | cvd on `#fx-chart` |
| 10-visible-only-with-motion | a paragraph at opacity 0 that only an animation (never run under reduced motion) reveals | motion-hidden on `#fx-reveal` |
| 11-focus-trap | a two-button widget whose last button sends Tab back to its first | focus-trap on `#fx-trap-a` |
| 12-hover-only | a card action revealed by `:hover` with its `:focus-within` twin removed | hover-only on `#fx-more` |
| 13-phone-pan-aria-hidden | an `aria-hidden` decoration hanging 140 px past the right edge: harmless on desktop (the root clips it) but a phone's layout viewport widens to 443 px and the page pans sideways | hscroll on `#fx-deco` (phone and tablet contexts; Chromium only, since only its `isMobile` emulation widens the layout viewport) |

The harness also carries correct patterns that must not be reported, so the control fails if a false-positive cause comes back: the page's skip link, an `aria-hidden` decoration hanging past the left edge, a closed `<details>` holding a link, two SVG legend rows set 9 px apart at 10 px (boxes touch, ink does not), 10 px SVG tick labels in `--ink-3` (forced colours must not under-read them), a card action revealed on `:hover` and `:focus-within`, and a fixed panel parked off the left edge by a transform that slides in above 1500 px, like the Studio library (a 0.3 s transition runs at every breakpoint crossing, so a measurement taken mid-slide would report it).

A fixture counts as caught only when an issue of the expected category names the injected element. Results of the last fixtures run are in the catch table of `out/fixtures-*/report.md`.

## Checking the checker

`node verify_sample.mjs SAMPLE.json OUTDIR [--only=category]` re-opens sampled findings (a `{category: [issue, ...]}` file cut from `report.json`) in the same engine, size, theme and text condition, and records what a human needs for a verdict: the element's and the other element's rects, computed colours, opacity when focused and when its heading is hovered, the transform/fixed/aria-hidden chain, and a 2x screenshot of the region. Results append to `OUTDIR/facts.jsonl`, so a stopped run resumes. `TOOL-VERIFICATION.md` has the 2026-09-28 fixture catch table, the false-positive sample and the before/after quick counts.

## Web fonts

The site loads its fonts from Google Fonts. Each engine's responses from `fonts.googleapis.com` and `fonts.gstatic.com` are fetched once per run (up to four tries) and replayed byte for byte to every later page, as a browser with a warm cache would. Without this a network blip during a run of several hours shows up as console errors and as layout findings measured in the fallback font. The report's notes give the number fetched, replayed and failed.

## Known limits

- Browser zoom is emulated (CSS viewport divided by the zoom, deviceScaleFactor multiplied), which is what zoom does to layout; browser UI and minimum-font-size settings are not modelled. Text-only zoom doubles every computed font size except inside SVG (SVG text scales with its graphic).
- Contrast is judged for text drawn by the DOM. Text painted into a canvas, and text in images, is not read. Text shadows are removed before sampling and not credited.
- The sampling windows are capped (14 per theme state, 8 in forced colours); text outside them gets the CSS verdict, which cannot see images or gradients behind text. The report notes every page where the cap cut windows.
- SVG heatmaps (over 150 rect cells) are skipped for per-mark contrast and colour-vision checks: cells are judged against each other, not against a ground, and their scales are continuous.
- "Chart" means an SVG with `role="img"`, a chart/plot/graph class, or at least six shapes. Marks under 35 % opacity count as decoration. Gridlines are recognised by class name only.
- Canvas charts are judged as a whole (blank, weak content, stale size); individual canvas series are not separated, so canvases are not in the colour-vision check.
- Forced colours are Chromium-only (Playwright emulation). The Fullscreen API in headless WebKit and Firefox may refuse or be missing; that is recorded as a note, not an issue.
- Scroll-triggered reveals are triggered by one scroll through the page, with up to 300 stops; on longer pages the stride grows and a reveal between stops could be missed.
- The keyboard pass caps at 160 Tab stops (40 in forced colours) and does not open menus or dialogs; traps inside widgets that only open on click are not reached.
- Touch targets use the 2.5.8 spacing exception with bounding boxes; the "equivalent control elsewhere" and "essential" exceptions need a human.
- Hover-only and colour-only are static heuristics (same-origin stylesheets, status class names).
- Counts are per element: one CSS rule applied to every heading anchor on 50 pages in 3 engines is ~1,300 unique issues. Read the top issue types table for root causes.
- The iOS Simulator run is a smoke test: it proves Safari renders each page, nothing more.
- The machine this was built on ran at a load average near 100 during development; timings in reports reflect that.
