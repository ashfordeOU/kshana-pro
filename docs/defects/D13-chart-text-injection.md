# D13: scenario text reaches chart markup unescaped (DOM script execution in the Studio)

Status: fix in progress (2026-10-08). PRIVATE: this file is the only place the exploit detail is
written down. The public PR and commits say only "escape scenario strings in chart SVG; sanitise
adopted SVG in the Studio; harden URL-state and kind lookups".

## What

Engine chart builders write some scenario strings into SVG text without escaping. The Studio
adopts engine chart markup into the page (`DOMParser` as `image/svg+xml`, then `importNode`), so
an event-handler attribute or script in that markup runs in the kshana.dev origin.

## How a reader is reached

The Studio opens `/studio/#s=<url-safe base64 of a scenario TOML>` and runs it in the browser
engine straight away (app.js, the `shared` branch of the start-up routine; no click). A link
whose scenario carries markup in a free-text field is therefore enough. Nothing is sent to a
server; the page's own origin is the target (browser storage, a page that looks like ours).

## Proof (2026-10-08, W6)

1. Every free-text field of the bundled scenarios, one field at a time, given a well-formed
   payload (`</text><image href="x" onerror="..."/><text>`): the clock `id` leaked into the chart in
   11 scenarios (clock-ensemble, clock-holdover, clock-holdover-labsr, imu-deadreckoning,
   orbit-gnss-challenged, orbit-molniya, orbit-multignss, orbit-real-tle, orbit-rinex,
   orbit-sgp4-gps, timetransfer: `src/report.rs`, `ensemble.rs`, `timetransfer.rs`,
   `inertial/mod.rs`), and the sweep chart's `scale` (`src/sweep.rs`) in sweep-clock-stability.
2. Four of those charts, put through the Studio's own adopt logic in Chromium: the handler ran.
3. After the fix the same inputs come out escaped, and the sanitiser removes the handler even
   from markup that is not escaped (`site-next/scratch/port/check_svgsafe.mjs`: 53 inputs inert,
   5 controls ran their handler untouched).

## Root cause

No shared escape in `crate::chart`; ten modules carried private copies, escaping only `& < >`
(not quotes) where they escaped at all, and the helpers `frame_open`, `y_axis` and `panel_axes`
wrote their text verbatim and relied on every caller to escape.

## Fix

- Engine: one `chart::esc` (`& < > " '`, control characters dropped); the three helpers escape
  their own text and the callers that pre-escaped no longer do (no double escaping); the private
  copies are gone; the eight clock-id sites and the sweep strings are escaped.
  `tests/chart_text_escaped.rs` re-runs each bundled scenario with markup in each string that
  reaches the chart.
- Studio (defence in depth, `lib/svgsafe.mjs`): refuse declarations before parsing; strip
  scripts, embedded documents, handlers, outside links, reaching styles and animations aimed at
  handlers or links from what is adopted.
- Same change: URL-state keeps only the documented parameters; kind-keyed tables answer only for
  their own keys (`lib/own.mjs`); scenario text is fetched only for a catalogued name.

## Release

Engine: 0.33.1 (coordinator decision 2026-10-08). The Studio side lives in the kshana-pro source
(`site-next/explorations/2026-10-01-studio-rebuild`) and is ported into `web/studio`.

## Not proven

Only text that the bundled scenarios carry into a chart was probed. A string reaching a chart by a
path no bundled scenario takes is covered by the helpers' escaping and by the Studio sanitiser, not
by a test. The CodeQL XML-entity alerts on `svgNode` and the request-forgery alerts are not
exploitable as reported (see the triage message of the same date); the entity guard and the
catalogue check are cheap hardening.
