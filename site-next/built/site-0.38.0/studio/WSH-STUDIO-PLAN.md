# WS-H Studio views: plan and what each needs from the result (19 Oct 2026, W53)

Brief: kshana-pro `ws/coordination` docs/briefs/WS-H-gnss-beyond-earth-0.38.md, H3; definition of done G4 and G5. The engine branches
(`feat/ssv-0.38`, `feat/lunar-infra-0.38`) were not on the remote when this was written, so nothing below is bound to a result shape yet.
`tools_newview.mjs` stamps out each page (index.html, page script and CSS, a lib reader with a test) from the shared Studio template, with a
generic reader that shows only what a result holds; each page below then replaces `readView` and `chartsOf` with its own. Rules for every page:
read the recorded result only, copy every number, scale it, draw nothing the engine did not write (no estimated curve, no filled-in gap); the
label the ledger holds (MODELLED unless an external public oracle was used) and the advisory statement on the page; no cause or actor words;
works at 390 px and 1440 px, light and dark, no horizontal page scroll; no request of any kind beyond the Studio's own files.

| Page (folder) | Kind | Question | Charts, each from a stated result field |
|---|---|---|---|
| `ssv` (SSV sky and availability) | `gnss-ssv` | Which navigation signals can a spacecraft in a high orbit use, and when does it have none? | signals in view along the trajectory per constellation (stepped lines, with the >=1 and >=4 lines marked); outage bars (start, duration, longest); main-lobe against sidelobe share; C/N0 along the trajectory; DOP; the ICG SSV availability table beside the engine's, the pre-registered comparison and its verdict as the engine states it; the skyplot at a chosen epoch through `lib/skydraw.mjs` |
| `ssv-weak` (weak-signal and limb) | same kind or its own | What does a grazing signal lose, and can a receiver still track it? | limb-ray delay and loss against tangent height; plasmaspheric TEC observable; acquisition/tracking threshold against integration time, aiding and C/N0 (10 to 25 dB-Hz) |
| `ssv-filter` (onboard filter) | same kind or its own | How well does an onboard filter hold a high orbit? | position and velocity error against time with the filter's own bounds; clock estimate; innovation consistency |
| `formation` | formation result | How well do the spacecraft know where they are relative to each other? | relative position error per pair against time; baseline length; the engine's own consistency statistic |
| `od-consistency` (lunar orbit determination) | the lunar OD kind | Do onboard GNSS orbit determination and independent ground radiometric orbit determination agree in lunar orbit? | the difference of the two orbits against time (radial, along, cross), their covariances, the consistency metrics as written, for ELFO and NRHO |
| `joint-surface-orbital` | the joint processing kind | Can a surface station and an orbiter fix carrier-phase integer ambiguities together? | ambiguity fix rate, time to fix, the position bound with and without the fix; the station and orbiter geometry |
| lunar frame and geodesy (extend `lunar/`) | `lunar-station-*`, frame, space-tie | What does adding a station or co-locating techniques do to the frame? | the information gain per parameter (before and after), libration and Love-number sensitivity (h2, k2, l2), as the engine's FIM results state them |

Needed from W54 and W57 (please relay, per kind): the `kind` string; the scenario file names to bundle; the result JSON field names and units for each series
above, with the sample times; whether a series is decimated and how (the holdover-test `record.series` and `series_decimation` are the pattern); the
label and advisory fields; the ledger rows; and one recorded result of each scenario from the native engine. If a series is not in the result, the page
says so and draws no curve, as the Holdover view does.

Gates per page: reader and chart tests (`lib/<id>.test.mjs`, in `ci.yml`), selfcheck rules (own list, no-request, base href, advisory statement, label
wording), the Studio menu and list links, `port_site.py` STUDIO_PAGES, `site.test.mjs` VIEWS, catalogue rows and recordings, then `tools_areas`,
`tools_bundle`, `tools_links`, `check_features` placements, screenshots at 1440 and 390 px in light and dark. G5: the 0.38 section, capabilities
and evidence pages carry WS-H once the ledger rows exist (counts are generated, never typed).
