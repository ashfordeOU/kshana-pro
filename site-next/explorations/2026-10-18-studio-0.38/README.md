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

The view draws the engine's result for the two bundled `holdover-test` scenarios, recorded with the native engine from kshana `feat/timing-standards-0.38` at 760fc54d (interim; the release re-record replaces them). The time-error curve is drawn only from the result's own `record.series` ({time_s, time_error_ns}, at most 3000 points, with `series_decimation` stating the rule); both bundled records are 601 samples, so nothing is decimated. The page has no parser of its own for a record: `lib/holdover.test.mjs` checks that the engine's series equals the record read from the scenario text (to rounding in the unit conversion).

- `holdover-engine-series-{example,ptp4l}-*` are the view on the recorded results as the engine wrote them, at 1440 and 390 px, light and dark. The earlier no-series and shape-fixture screenshots are removed; `REVIEW/fixtures/holdover-test-*.with-series.result.json` stay as shape fixtures for the unit tests (the two series shapes, pairs and arrays).

Words on the view: "measured against the mask"; MODELLED.

## Site: Benchmark page with W57's fragment

`site-benchmark-w57-fragment-*` is `/benchmark` built in a scratch copy with kshana `feat/bench-observatory-0.38` @ 1df6db2 `bench/site/leaderboard.html` embedded (engine 96a59e44 otherwise, gates skipped in the copy). `check.py` passed on that build (113 pages). The fragment is regenerated when W57 re-scores under spec 2.0.0; nothing on the page is typed by hand except the hero and the "what a row means" text.

## Tools: Jammertest card (wasm)

The Tools page has a Jammertest tab: import the plan (`jammertest_import`) and score a log against it (`jammertest_score`), both run in the browser from the 0.38 package. The committed `pkg/` is still the 0.37 package, so on it the card says plainly that the engine lacks the function; the screenshots `tools-jammertest-synthetic-*` were taken with a package built from `feat/referee-0.38` @ `eb45325` swapped into a scratch copy of the Studio (not committed). `jammertest-smoke.mjs` drives it; `jammertest-fixture/mk.py` writes the synthetic NMEA log, catalogue and plan (one real C/N0 drop in slot one, nothing in slot two, so one detected and one missed). The data are invented and are not Jammertest data; the card's MODELLED label and the advisory statement are shown with the result. SBF is accepted in this card only: no Trust "open a file" wasm entry reads SBF.

## Receiver and sensor inputs (W56's machine inputs, kshana `feat/machines-0.38` @ 6ec3cbc1)

- **Trust, "Generic receiver-trust run":** a log-format picker (NMEA, RTKLIB `.pos`, NovAtel OEM7, RTCM 3 MSM, GNSS-SDR JSON lines, Septentrio SBF, u-blox UBX, RINEX, Android), guessed from the first bytes (NovAtel, UBX, SBF, RTCM 3) then the file name, and an optional IMU file (`[log.imu]`, csv or euroc, heading at the first sample, clock offset, axis). Binary formats go to the engine as base64 through `receiver_trust`, text formats as text; nothing is uploaded. "Score as a vessel" adds `[platform] kind = "vessel"` so the run can be shown in the replay below with the receiver's-own-flag strip. The session is built by `lib/trust-open.mjs` (tested).
- **Tools, "IMU fit":** `imu_fit` on a stationary log, with the fitted table, the log's SHA-256 and the engine's notes; MODELLED, and the card says the estimator is checked on synthetic logs only.
- **Tools, telemetry:** MQTT as a fifth format (`trust_telemetry` format `mqtt`: topic, tab, payload per line; nothing is published from the page).
- Screenshots `trust-novatel-*` and `tools-imufit-*` were taken with a package built from `6ec3cbc1` swapped into a scratch copy of the Studio (not committed). The NovAtel log is invented and written to W56's test-line layout (`machines-fixture/novatel_mk.py`; BESTPOS, RANGE and RXSTATUS once a second, receiver jammer flag 120 to 180 s, C/N0 down 14 dB from 90 to 150 s); the IMU, `.pos` and MQTT inputs are in `others_mk.py`. `machines-smoke.mjs` drives them. RTCM 3 and Septentrio SBF were exercised too (`trust-rtcm3-synthetic-*`, `trust-sbf-synthetic-*`) with invented binary logs written by `machines-fixture/rtcm_sbf_mk.py` in the layouts the engine's own tests use (not captures from a receiver): both sniffed from their first bytes, both read, and the monitors alarm at the start of the 14 dB drop (70 s and 80 s).

## Interference observatory viewer (Studio page `observatory/`, per W57's `docs/design/W57-observatory-viewer-spec.md` @ 868fba8e)

Open-a-file only: the person loads the files `kshana interference-map daily` wrote for a day (`.geojson`, `.incidents.json`, `.manifest.json`, `.png`; one or several days together). The page makes no request of its own (checked by `selfcheck`), fetches no data, uploads nothing and ships **no observatory product**. It reads, in `lib/observatory.mjs` (tested):

- the three states only; a feature with any other state, or an incident file whose `cause` is not exactly `not attributed`, is refused with a reason, so the page can show no cause or actor;
- counts the file withholds (null) read "withheld", never 0; a cell not in the file is not drawn; the summary is shown as written, and a mismatch with the cells is said;
- "Not observed is not clear." in the legend and in the advisory block; the release rule (at least 24 h after the day ends, 0.5-degree cells, not a live view); a day-wide-effect day says no cell could be called, not that the sky was clean; a day with no degraded cells says exactly that;
- the dataset block with licence and attribution, the licence address as a link only when it is https, the file's own notice, and a files block that recomputes each opened file's SHA-256 (Web Crypto) against the manifest ("matches", "does not match", "not opened");
- the cells as an SVG (plate carrée, no base map, degraded solid red, not degraded solid blue, not observed grey diagonal hatch, a title per cell), an incident table whose rows outline their cells, a cell table with a state filter that is the accessible equivalent, and the PNG as a picture when opened.

`observatory-fixture/mk.py` writes three INVENTED days (a pseudo-random pattern on a Baltic-shaped box, dataset "synthetic sample"; day 2 a day-wide-effect day, day 3 without a degraded cell) only so the page can be reviewed; they are not observatory output and are not shipped with the page. `observatory-smoke.mjs` drives the page against them (34 checks at 1440 and 390 px, light and dark: hashes match, an altered file does not, a cause-naming incident file is refused, no request to any other address, no horizontal page scroll). Screenshots are `observatory-synthetic-*` (clipped to the top of the page).

For integration: the kshana side lists the Studio pages in `web/tools/port_site.py` (STUDIO_PAGES) and `web/site.test.mjs` (VIEWS); `observatory` needs adding to both when the page is ported.

## 0.38.0 integration (kshana `feat/studio-replay-0.38`, merged with `integ/0.38.0` @ c979ccc8)

`integration-0.38.0/` holds the review screenshots of the **ported** site, served from kshana's `web/` (home, capabilities, evidence, developers, benchmark, replay, optical, holdover, observatory empty, and observatory with the three invented days of `observatory-fixture/mk.py`), each at 1440 and 390 px, light and dark; `shoot.mjs` takes them and reports page errors, third-party requests and horizontal page scroll (none found). The Studio package is the 0.38.0 WebAssembly build of the merged tree (wasm-opt run from the npm `binaryen` 117 package because the release download is not reachable from here), the native recordings were made with the release `kshana` of the same tree, and the browser and native recordings carry one `engine_commit` (c979ccc8…); `selfcheck.mjs --final` passes. The site was rebuilt against the same tree with the recorders in `src/tools/`; `check.py` and `check_features.py` pass (errors 0).

## Integration pass 2 (engine a9978e3f, 0.38.0 with gnss-ssv and lunar-infrastructure)

`integration-0.38.0/` was re-shot from the rebuilt site: home, capabilities, evidence, developers, benchmark, replay, optical, holdover, observatory, the Space-user GNSS page and the Lunar infrastructure page, at 1440 and 390 px, light and dark. `shoot.mjs` runs against the built site folder served locally. The site pages load web fonts and a chart library from public hosts as designed; the Studio pages make no third-party request and no page scrolls sideways.
