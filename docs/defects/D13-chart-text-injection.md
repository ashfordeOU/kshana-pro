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

---

# DRAFT security advisory (nothing public until the 0.33.1 deploy to kshana.dev is confirmed)

Founder decision 2026-10-08: publish a GitHub security advisory once the fix is live.
Drafted by W6 from the repository history; every claim below was checked against git unless marked.

**Title:** Script execution in the Kshana Studio through scenario text carried into generated chart SVG

**Summary.** The engine writes some strings that come from a scenario (for example a clock's `id`,
a sweep's `scale`) into the SVG charts it generates without escaping them. The Studio on kshana.dev
runs a scenario given in the address (`/studio/#s=...`) as soon as the page opens and adds the
resulting chart to the page, so a crafted link makes the page run script chosen by whoever wrote
the link, in the kshana.dev origin. Fixed in 0.33.1 (engine escaping, and the Studio now reduces
any adopted markup to drawing).

**Affected.**
- The Studio (kshana.dev/studio): every deployment from **v0.29.2** (2026-10-01; `web/studio/app.js`
  first adopts engine chart markup into the page with `DOMParser` and `importNode`) through the
  deployment of 0.33.1. The in-browser playground before it showed the chart as an image
  (`<img src="blob:...">`, which does not run handlers); I found no earlier insertion of engine chart
  markup into the page (history searched for `DOMParser`, `importNode`, `innerHTML` on `web/`; not
  a proof of absence). The Studio is a website, not a package: there is nothing for a user to upgrade.
- The engine, as a library and CLI (crates.io `kshana`, PyPI `kshana`): versions **0.1.0 through
  0.33.0** write the unescaped text (the clock-id line first appears in `0a989efd`, released in
  v0.1.0). Impact there is limited to opening a generated `*.chart.svg` that was made from an
  untrusted scenario in a browser as a document of its own; no program of ours does that.
- Last release before the fix: v0.32.0 (v0.33.0 was not released when this was drafted).

**Patched:** engine 0.33.1; Studio from the deploy that ships the same change.

**Severity (CVSS 3.1): 6.1 Medium, `CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N`.**
Reasoning: reached through a link (AV:N), needs no preparation (AC:L), no account (PR:N), the reader
must open the link (UI:R). Scope changed because the script runs in the browser's kshana.dev
origin, a different authority from the engine that wrote the text. Confidentiality and integrity are
Low because the origin holds only what the Studio keeps in the reader's own browser (saved runs and
settings; no sign-in, no server-side data) and the page can be made to say things; availability is
unaffected. A reader who scores scope as unchanged for this class of issue gets 4.7
(`.../S:U/C:L/I:L/A:N`). CWE-79 (DOM-based cross-site scripting) and CWE-116 (improper encoding or
escaping of output).

**Impact.** Client side only. No server, build or release system is involved, no credentials or
keys are held by the site, and nothing is sent to us by the link (the scenario travels in the
address fragment, which browsers do not send). An attacker who gets a reader to open a crafted link
can run script in the kshana.dev origin: read what the Studio stored in that browser, change what
the page shows (including making a result look like ours), or send the reader elsewhere.
Not exploited to our knowledge (no log exists for it; the fragment never reaches a server).

**Workarounds.** None needed for users of the CLI or library. Studio users: reload kshana.dev after
the deploy; until then, do not open Studio links (`#s=...`) from sources you do not trust.

**Not part of this advisory.** Four related static-analysis alerts (XML entity expansion in the chart
parser, property injection from the address, a method lookup by kind, request construction from
state) were triaged: none is exploitable as reported; the cheap guards were added in the same change.

**Credits.** Ashforde OÜ.

**Open points before publishing.** (1) Confirm the deploy of the Studio change to kshana.dev, not
just the engine release. (2) Ecosystem entries: crates.io `kshana` `>= 0.1.0, < 0.33.1` and PyPI
`kshana` `>= 0.1.0, < 0.33.1` (the PyPI/npm packages carry the same engine; confirm the first version
of each that shipped the chart text, which I did not check per package). (3) Decide the CVSS scope
reading above. (4) Checked: the clock-id line is in v0.1.0; the sweep chart text (`sweep.rs`, commit `01796136`) first ships in v0.4.0. The lower bound 0.1.0 therefore covers both.
