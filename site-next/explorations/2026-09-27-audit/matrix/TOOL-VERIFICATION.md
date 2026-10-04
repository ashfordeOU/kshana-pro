# Display matrix: tool verification, 2026-09-28

This checks `run_matrix.mjs` itself: do the mutation fixtures get caught, is the clean control
clean, and how many of the baseline's biggest categories are false positives. Nothing under the
site was edited. The machine ran at a load average of 140 to 230 throughout, so timings are
slow and animation timing is less stable than usual.

## 1. Mutation fixtures

The fixture set gained three defects: `11-focus-trap`, `12-hover-only` and
`13-phone-pan-aria-hidden`. Two of them, a focus trap and a hover-only control, were named in the
brief and had no fixture. The harness now also carries correct patterns that must not be
reported (see "False-positive guards" below).

Before = the tool as the previous agent left it (`out/fx-verify-1`, Chromium, 14 views, 7,840
runs). After = the fixed tool (`out/fx-verify-3`, Chromium, 15 views). A fixture counts as
caught only when an issue of the expected category names the injected element.

| Fixture | Injected defect | Expected | Before | After | Evidence (after) |
|---|---|---|---|---|---|
| 00-control | the harness in its correct form | 0 issues | PASS | PASS | 0 issues |
| 01-hscroll-390 | 600 px fixed-width strip | hscroll / outside-viewport | CAUGHT | CAUGHT | outside-viewport `div#fx-wide`, 357 px outside at 280–640 and 830–920 px |
| 01b-hscroll-390-unclipped | the same strip, root overflow visible | hscroll | CAUGHT | CAUGHT | hscroll `div#fx-wide`, document 637 px wide in a 280 px viewport |
| 02-overlap-768 | two captions overlap at 700–800 px | overlap | CAUGHT | CAUGHT | `p#fx-a` overlaps `p#fx-b` by 186×17 px at 700–800 px |
| 03-grey-dark-only | #3A3F4F text in the dark theme only | contrast-text | CAUGHT | CAUGHT | `p#fx-grey` 1.91:1 on #07080E (system-dark 390×844) |
| 04-line-equals-dark-bg | chart line stroked in the dark background colour | invisible-mark / contrast-nontext | CAUGHT | CAUGHT | invisible-mark `polyline#fx-line2`, #07080E on #07080E |
| 05-canvas-blank-after-resize | canvas cleared on resize, never redrawn | canvas-blank | CAUGHT | CAUGHT | `canvas#fx-canvas` one flat colour after 1440×900 → 390×844 |
| 06-focus-outline-removed | `outline:none` on a button's focus | focus-visible / focus-contrast | CAUGHT | CAUGHT | `button#fx-btn` 0 px change on focus |
| 07-touch-16px | two 16 px buttons 2 px apart | touch-target | CAUGHT | CAUGHT | `button#fx-t1` 16×16 px, closer than 24 px (phone 320×568) |
| 08-clip-at-200-text | 22 px fixed-height box | clipped | CAUGHT | CAUGHT | `div#fx-clip` content 260×40 in a 260×22 box (text-only 200 %) |
| 09-legend-deuteranopia | red/green series and legend | cvd | CAUGHT | CAUGHT | `svg#fx-chart` ΔE2000 2.7 under deuteranopia |
| 10-visible-only-with-motion | paragraph revealed only by an animation | motion-hidden | CAUGHT | CAUGHT | `p#fx-reveal` hidden under reduced motion |
| 11-focus-trap (new) | last widget button sends Tab back to the first | focus-trap | CAUGHT | CAUGHT | `button#fx-trap-a`: focus cycles back after 66 of 128 focusable elements |
| 12-hover-only (new) | card action revealed by `:hover`, `:focus-within` twin removed | hover-only | CAUGHT | CAUGHT | `span#fx-more` revealed only by `.fx-card:hover .fx-more`; also focus-visible: its link takes focus while invisible |
| 13-phone-pan-aria-hidden (new) | aria-hidden decoration 140 px past the right edge | hscroll | not run (see below) | CAUGHT | `div#fx-deco`: on a phone the page is 443 px wide for a 320 px screen |

Before the fix, fixture 13's defect showed up as outside-viewport on `header#nav` (123 px
outside at 320×568): the right symptom, but the wrong element. The header is only stretched by
the widened layout viewport. The fixed tool names the decoration.

WebKit and Firefox run of the same fixtures with the fixed tool: see "Other engines" at the end.

### False-positive guards (the control's new content)

The control must stay at zero while it holds these correct patterns: the page's skip link, an
aria-hidden decoration hanging past the left edge, a closed `<details>` holding a link, two SVG
legend rows set 9 px apart at 10 px (their boxes touch but the glyphs do not), 10 px SVG tick
labels in `--ink-3`, a card action revealed on `:hover` and `:focus-within`, and a fixed panel
parked off the left edge that slides in above 1500 px (a 0.3 s transition, like the Studio
library).

With that control, the tool as the previous agent left it fails with 3 issues, all false
(`out/fx-control-prefix`). A fourth row comes from an earlier draft of the control, in which
the decoration hung right:

| Pre-fix finding on the control | Cause | Fix |
|---|---|---|
| overlap: `text.fx-tick` "trace one" overlaps "trace two" by 39×3 px, at 300 and 330–2560 px | text boxes are the font's content area, taller than the ink | overlap judges text on its ink band |
| outside-viewport: `aside#fx-drawer` 53–196 px outside at 1500–1600 px | measured while the panel was still sliding in after the resize | every resize waits for transitions |
| chrome-coverage: header + parked panel "cover 60 %" at 400 % zoom | a panel parked off screen by a transform was counted as covering rows | only the on-screen width counts |
| (with the decoration on the right) outside-viewport `header#nav` at 320×568 | phone layout viewport widened; symptom named | the widened viewport is reported as hscroll, naming the cause |

The fixed tool passes this control with 0 issues (`out/fx-verify-3`).

## 2. False positives in the baseline

Sample: 20 findings per category, drawn with a fixed seed (20260928) from
`out/full-baseline/report.json`. `verify_sample.mjs` opened each one in the same engine, size,
theme and text condition, on the current combo build. It recorded rects, computed colours,
opacity when focused (and 400 ms later) and when the heading is hovered, and the
transform/fixed/aria-hidden chain, and took a 2x screenshot. A few elements are no longer in
the current build (the home page was rebuilt). Those were judged on the baseline's own crop.

| Category | Baseline unique | Sampled | Real | False | Cause of the false ones | Fix |
|---|---:|---:|---:|---:|---|---|
| focus-visible | 1,292 | 20 | 20 | 0 | none. All 20 are `a.anchor` heading permalinks: real Tab stops (tabIndex 0) that are still at opacity 0 when focused, even 400 ms later, and opacity 1 only on heading hover. `.prose .anchor{opacity:0}` has no `:focus` rule. The ring is drawn on an invisible element | none needed. The pass grades only elements that really receive Tab focus, so non-focusable elements and content inside a closed `<details>` cannot appear (the guard control proves it) |
| contrast-text | 337 | 20 | 0 | 20 | all 20 are forced-colors (Chromium). The glyph colour was read from pixels, and anti-aliased 8–10.5 px text never reaches its colour. SVG ticks are `fill:#8088A6` (5.77:1 on #05060C) but read as #636982–#6E758F (2.7–4.4:1). Colour emoji (🟢, #019901) were read as text colour | forced colours: the ground still comes from pixels, the glyph is read from both the computed colour and the pixels, and the text fails only when both fail. The computed colour alone would miss links: Chromium paints link text in LinkText while `getComputedStyle` keeps the author colour (the header's Studio link computes #07080E and paints white) |
| overlap | 229 | 20 | 16 | 4 | 2 were text boxes sharing 3 px with no glyph contact (tightly stacked SVG legend rows; a large numeral over its caption). 1 was the Studio search `kbd` vs input, measured while the library drawer slid shut after a resize (settled, they are 8 px apart). 1 was a home telemetry card over the globe: gone from the current build, and the baseline crop shows the text painted on top | ink band for text (box less 15 % of its height at top and bottom); every resize and sweep step waits for running transitions and finite animations (1 s / 0.6 s cap) |
| outside-viewport | 215 | 20 | 18 | 2 | both are Studio `aside#library` at 864×1536 and 1080×1920 portrait, measured mid-slide. Settled, the drawer sits at `translateX(-306px)` with its right edge at −6 px, which is already ignored as parked left | the same transition wait |
| hover-only | 179 | 20 | 20 | 0 | none. All are `.prose h2:hover .anchor` / `h3:hover`. Hovered opacity 1, focused opacity 0 | none needed |

The 18 real outside-viewport findings are 15 × the header's `div.nav-right` running 86 px past a
280–360 px window (still true on the current build: the Studio pill ends at 366 px), 2 long URLs
with no `overflow-wrap`, and 2 × `header#nav` at 320×568. The last two are real but misattributed:
on a phone something else widens the layout viewport and the header only stretches with it. The
fixed tool reports that as hscroll and names the element that sticks out.

Counting note (not a false positive): 1,289 of the 1,292 focus-visible issues and all 179
hover-only issues come from one CSS rule, repeated per heading, per page and per engine. The
README now says so under Known limits.

Supplementary sample: 20 contrast-text findings from `out/full-baseline-merged` (after the
previous agent's own forced-colors change). 18 are still forced-colors under-reads of the same
kind. 2 do not reproduce: one selector now names a different element, and one background read
as #C2C2C6 is light (#F4F4FA) on the current build.

### Other false-positive causes found on the way

- `chrome-coverage` counted fixed elements parked off screen horizontally. The panel at
  `translateX(-100%)` counted as covering its rows.
- A phone's widened layout viewport was blamed on whichever full-width block stretched with it.
- The whole `#beats` story on the home page (a crossfade at the 920 px breakpoint) was reported
  as 56 overlaps. Two frames after the resize all four beats sit at opacity 0.7. Settled, they
  are at 0. This is the transition wait again. The heights group measured right after each
  scroll and caught the same crossfade intermittently. It now settles too, and three repeated
  runs give identical results.
- In WebKit and Firefox the clean control failed twice for reasons outside the page: a WebKit
  "Failed to preconnect to fonts.googleapis.com" TLS hiccup, and a Firefox fullscreen request
  that settled after the 900 ms mark. Both are fixed. The control now passes in all three
  engines (`out/fx-control-3engines`).

### Strictness kept

- A panel parked past the **right** edge is still reported. I tried exempting transformed
  off-canvas panels, then found that on a 320 px phone such a panel widens `innerWidth` to 443 px
  and the whole page pans sideways. That is a real defect, so the exemption was dropped.
- An aria-hidden element is skipped as content, but if it widens a phone's layout viewport it is
  named (fixture 13).
- The overlap ink band still catches every real collision in the sample. Examples: "Signals" /
  "2", "Clocks" / "13", "…nar" / "CHECKED", the "0.10 m" / "10 cm" scale labels at text 200 %,
  a legend label running into a tick under text spacing, and fixture 02's 17 px caption overlap.
- On the same snapshot, every text-200 % overlap that disappeared was checked on its crop. All 17
  are stacked lines or pills beside headings with no glyph contact.

## 3. Quick run on the current combo build: before and after

The live build was rebuilt while this ran (`index.html`, `theme.css`, the playground and more
changed between 22:42 and 23:20 local). For a fair comparison, the build was copied at 23:27 to
`2026-09-27-audit/combo-snap-tmp` (deleted afterwards), and both tool versions ran `--quick` on that copy at the same
time. Each run: 10 views, Chromium, 1,638 page×condition runs.

| Category | Before (pre-fix tool) | After (fixed tool) |
|---|---:|---:|
| overlap | 122 | 45 |
| contrast-text | 66 | 59 |
| clipped | 26 | 25 |
| invisible-mark | 15 | 16 |
| contrast-nontext | 12 | 11 |
| outside-viewport | 8 | 8 |
| cvd | 5 | 6 |
| hscroll | 4 | 4 |
| touch-target | 3 | 3 |
| motion-hidden | 2 | 2 |
| stale-size | 2 | 2 |
| chrome-coverage | 2 | 2 |
| hover-only | 1 | 1 |
| **total unique** | **268** | **184** |
| occurrences | 761 | 653 |

Unique issues: **268 → 184** (occurrences 761 → 653). 93 issues went away and 9 appeared.

What went away:
- 64 were the home page's `#beats` story, measured mid-crossfade. That is 56 in the 920 px sweep
  step and 8 at 1024/1440×1600.
- 26 were other overlaps: 17 at text 200 % and the rest SVG labels stacked 3 px apart. Every
  crop was checked: no glyph contact.
- 5 were forced-colors contrast under-reads.
- The rest were single findings that moved between neighbouring elements.

What appeared:
- 5 are the settled `#beats` state at 1024×1600. The previous beat, "A lie that looks like a
  fix.", stays visible at about 30 % opacity behind "Then time starts to drift.". The crop shows
  the ghosted text, and three repeated runs report it identically. This is a real defect that
  mid-transition sampling used to hide.
- 4 are marks on animated SVG (a pulse ring and the jamming footprint's cvd pairs), which flicker
  between runs.

Findings that move between runs come from infinite SVG animations and pixel-sampling windows.
The underlying defects are still reported on neighbouring elements: for example, the
`a.prov-open` links at 4.24–4.33:1 are in both runs.

For reference, quick runs on the live build as it stood at each moment: pre-fix 230 unique
(`out/quick-before`, build of 22:42) and fixed 195 unique (`out/quick-after`, rebuilt build).
The site changed between them, so they are not a like-for-like comparison.

## Changes to the tool

- `run_matrix.mjs`
  - Forced-colors text contrast: pixel ground, glyph read from both the computed colour and the
    glyph pixels, and a failure only when both fail. The detail line prints both.
  - `resizeTo()` waits up to 1 s for running transitions and finite animations. Every sweep step
    waits up to 0.6 s, and every heights-group scroll stop waits up to 1 s.
  - Fullscreen: waits up to 3 s more for the request to settle. In WebKit and Firefox, a request
    that neither enters nor rejects is a note, not an issue. Firefox headless had been rejected
    after the 900 ms mark and reported as "did not enter full screen" with no error text.
  - Console: `Failed to preconnect to …` is ignored. WebKit logs a failed resource hint as an
    error when the network hiccups; this happened on the clean control and has no effect on the
    page.
- `lib/probe.js`
  - Overlap judges text on its ink band.
  - chrome-coverage counts only the on-screen width.
  - Phone/tablet contexts whose layout viewport widened (`innerWidth` > screen + 24 px) report
    hscroll, naming the elements that stick out past their own parent (aria-hidden ones
    included), not blocks that merely span the widened viewport. The document-overflow branch
    and outside-viewport stand down in that state, so the header is no longer blamed.
- `fixtures/make_fixtures.py`: fixtures 11, 12 and 13; the false-positive guards in the harness;
  `build()` takes an optional harness-CSS transform.
- `verify_sample.mjs` (new): re-opens sampled findings and records evidence, resumable
  (`facts.jsonl`).
- `README.md`: the detector descriptions, fixture table, guard list, verifier and limits,
  updated to match.

Run outputs: `out/fx-verify-1` (fixtures, pre-fix), `out/fx-control-prefix` (guarded control,
pre-fix tool), `out/fx-verify-3` (fixtures, fixed), `out/fx-verify-2-wkff` (fixtures, WebKit and
Firefox, fixed), `out/quick-before-snap` and `out/quick-after-snap-final` (quick, same
snapshot).

## Other engines

`out/fx-verify-2-wkff` (WebKit + Firefox, 15 views, 16,740 runs, fixed tool at that point):
fixtures 01–12 are all caught in both engines, including the new focus-trap (WebKit tabs with
Alt+Tab) and hover-only fixtures.

- Fixture 13 is not caught there, and that is expected. Only Chromium's `isMobile` emulation
  widens the layout viewport. WebKit's emulation does not, and Firefox has no `isMobile`. It is
  caught in Chromium.
- The control failed in that run for the two outside-the-page reasons above (WebKit preconnect
  TLS error, Firefox fullscreen timing).
- After those fixes, `out/fx-control-3engines` ran the control and fixture 05 in Chromium,
  WebKit and Firefox. The control passes with 0 issues, and fixture 05 is caught as canvas-blank
  in all three.

The Chromium catch table above is from `out/fx-verify-3`. The later changes (heights scroll
settle, fullscreen wait, preconnect filter) touch no fixture's detection path, and the control
re-run covers them.

## 4. Full-run round, 2026-09-29: two false-positive causes fixed, and why

The quick run on all three engines (`out/r4-fx-q-all-1`, 4,857 runs, 243 unique) was read
finding by finding before any page was changed. Two causes produced findings that were not
defects. Each was checked against the page before the tool was changed.

### 4.1 Motion comparison counted a scroll box's unscrolled rows as hidden

58 of 61 motion-hidden findings were WebKit rows of the docs "On this page" list
(`nav.docs-toc > a.l2`) on `docs/scenario-reference`: "visible with motion, hidden when
prefers-reduced-motion: reduce". The list is `position:sticky; overflow:auto`, and the docs
script scrolls it to keep the section in view in sight (`toc.scrollTop = …`,
`tools/build_docs.mjs` line 382). Which rows are inside the box depends only on where that
scroll ends up after the tool's slow scroll, and WebKit's timing differed between the two
motion settings. Every row is reachable by scrolling the list; nothing is hidden by motion.

Fix (`lib/probe-r4.js`): `visibleRect(el, r, { scrollers: false })` treats only
`overflow: hidden/clip` ancestors as hiding their content in the motion comparison; an
`auto/scroll` ancestor no longer does. Every other check still clips by scroll boxes. Fixture 10
(opacity revealed only by an animation) is still caught.

### 4.2 Pixel contrast paired one frame's colour with another frame's ground

Several findings sat on components that animate by design:

| Finding | What was on screen | Evidence |
|---|---|---|
| Developers `button#mp-t1 > span.n` "#ffffff on #e8f0e6 1.16:1" | the auto-playing step player moved from step 1 to 2 between the glyph read and the glyph-hidden capture: white is the selected-step colour, #e8f0e6 the lime "past" ground | in each settled state the pairs are dark-on-cyan / white-on-teal / ink-2-on-lime-soft (site.css 560–563), all above 4.5:1 |
| Developers `code#dtCode … span.dt-cmd` "#eef0fa on #3ddcf7" (Firefox) | the terminal's block caret passing over the typed command | the caret moves on every frame; the text settles on the panel |
| Home `div#hLog … span` "@0.55" and "@0.21" (Firefox) | log lines caught inside their 0.5 s `logIn` fade | `home.css` line 75; the settled lines are opaque |
| Home `div#storyCopy` "@0.87/0.84" | the scroll story's copy caught mid cross-fade | `home.mjs` sets its opacity from the scroll position |
| Home `g.ring-anim > circle.pulse`, Evidence `circle.hr-d`, Missions replay dot | expanding pulse rings and moving replay dots | their position and opacity change on every frame |
| Editions `svg#eoSvg` marks | the ring steps to a new kind every 0.9–2.8 s and redraws its highlighted links | screenshot `scratchpad/peek/eo-light.png`: every mark is clearly visible in the light theme |

Fix (`run_matrix.mjs`, `windowPass`): when the pixel text-contrast or mark-contrast pass
reports failures, it waits 450 ms and measures again, and only a failure seen both times is
counted. The number dropped is printed in the report's notes. A static defect fails both times,
so every fixture is still caught.

### 4.3 Other findings judged outside the pages

| Finding | Cause | Evidence |
|---|---|---|
| console 404 for `docs/docs.js`, `docs/index.html`, `docs/scenario-reference.html`, `assets/brand/kshana-mark.svg`, `playground/lib/views.mjs`; docs/index "expected a dark page" in the same run | other agents rebuilt the site while the matrix was running: `build.py` rewrites `docs/` and the assets, and for a moment those files do not exist | all of these files exist and load on the built site; `check.py` passes its link check; the theme-state finding came from the same moment the page itself returned 404 |
| "requestFullscreen did not enter full screen" (Chromium, no error text) and `canvas.owm-cv` stale-size in full screen | measured at a load average of 180–240; the request and the `fullscreenchange` redraw had not settled inside the waits | re-run one by one with the tool's own button method (`scratchpad/fs.mjs`): all three canvases enter full screen at 1440×900 with 1440×900 backing |
| Hero windows `figure#chWin/#dtWin/#eoWin/#ehWin` "content 306 in a 166 box" | measured while the page owners were rebuilding those heroes | at 280, 320, 520 and 1040 px all four are `overflow: visible` with `scrollWidth == clientWidth` (`scratchpad/win.mjs`) |

### 4.4 Proof after the change

`out/r4-fx-fixtures-1` (Chromium, 15 views, 8,400 runs, `--probe=lib/probe-r4.js` and the
stability check): the control passes with 0 issues, and all 14 fixtures (01–13 and 01b) are
caught by the expected category on the injected element.

`--probe=FILE` (new): the in-page code is read at every browser-context creation, so an edit to
`lib/probe.js` would reach a full run already in flight. The full "before" run kept the
unchanged `lib/probe.js`; the changed probe is `lib/probe-r4.js` until that run finished.
