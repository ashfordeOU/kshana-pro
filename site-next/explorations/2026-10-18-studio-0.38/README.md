# Studio 0.38 views: screenshots for review (2026-10-18)

Taken with Playwright (Chromium) from `2026-10-01-studio-rebuild/` served locally, at 1440 px and 390 px wide, light and dark, full page.

| Views | Files |
|---|---|
| Bridge replay, fjord vs drag-off (T+13:20) | `replay-fjord-vs-dragoff-{1440,390}-{light,dark}.png` |
| Bridge replay, half the sky masked vs the plain run (T+10:20, just after the blanking at T+10:00) | `replay-masked-sky-blanking-*` |
| Training debrief with the skyplot, vessel glyph, heading and error ring (coastal drag-off, row 81) | `training-coastal-drag-off-*` |
| Maritime trust with the skyplot, glyph and ring (masked-sky demo, cursor 650 s) | `trust-masked-sky-*` |
| Optical PNT, three sites with diversity and the engine's seeded link-margin time series (interim recording, see below) | `optical-network-*` |
| Optical PNT, enhancement techniques | `optical-techniques-*` |

`smoke.mjs` is the functional check that ran alongside: play, pause, step, moment steps, keyboard, verdict at the end, the half sky blanking at T+600 s with the same satellites listed, both exports (animated SVG 1.7 MB, frames ZIP 4.6 MB) and both optical scenarios.

All data on these pages is synthetic or recorded engine output; the coast is Natural Earth public-domain land for orientation only.

## Changes after the coordinator's review

- The coast is hidden for a run whose true track runs over land at 1:10m (the Tallinn-Helsinki, masked-sky and fjord logs), with the one-line note "coast hidden: this log's invented positions fall on land".
- Where a log lists satellites and hears none in some direction, the track carries a halo and a label from the log ("no signal from the west, T+6:00 to T+8:40 (from the log)"; a blocked sky and a jammed signal look the same in a log, so the words say "no signal", not "blocked").
- The lone Site diversity card spans its row. The optical page now draws link margin against time from the engine's new seeded `timeseries` (kshana feat/studio-replay-0.38). The two recordings under `recorded/` are interim: from a native debug build at the engine commit named in each file (`tools_record_optical_series.mjs` checks that nothing but the series and its units changed against the 0.37 recordings); the release package re-records them at integration.

## Trust: receiver self-report and Jammertest scorecard (shape fixtures)

`trust-self-report-shape-fixture-*` and `trust-jammertest-scorecard-shape-fixture-*` draw the new Trust lanes from hand-built files that mirror the engine's `SelfReportEpoch`/`SelfReportSummary` and `Scorecard` structs (feat/referee-0.38). They exist to check the drawing; the numbers in them are invented and say so in their own label. Fixtures are in `2026-10-01-studio-rebuild/REVIEW/fixtures/` (not ported).

## Site: Benchmark page stub

`site-benchmark-stub-*` is the new `/benchmark` site page in its "no results published yet" state (built in a scratch copy against engine 96a59e44 with the data-staleness gates skipped; `check.py` passes on that build, rules 27 and 28 included). When the engine checkout carries `bench/site/leaderboard.html` (a fragment: no html/head/body/script) `build.py` embeds it in the board section; a whole document or a script stops the build.

## Holdover test view (W55's `holdover-test` kind)

The view draws the engine's result for the two bundled `holdover-test` scenarios, recorded with the native engine from kshana `feat/timing-standards-0.38` at a396a84c (interim; the release re-record replaces them). The time-error curve is drawn only from `record.series`, which W55 has been asked to write into the result; until it does:

- `holdover-recorded-no-series-example-1440-light.png` is the view on the recorded result as it is: no curve, and the page says why.
- `holdover-shape-fixture-*` are **shape fixtures**: the same results with `record.series` filled from the scenario's own record by a review script (`REVIEW/fixtures/holdover-test-*.with-series.result.json`). They show the drawing, not engine output.

Words on the view: "measured against the mask"; MODELLED.
