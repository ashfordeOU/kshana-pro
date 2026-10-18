# Studio 0.38 views: screenshots for review (2026-10-18)

Taken with Playwright (Chromium) from `2026-10-01-studio-rebuild/` served locally, at 1440 px and 390 px wide, light and dark, full page.

| Views | Files |
|---|---|
| Bridge replay, fjord vs drag-off (T+13:20) | `replay-fjord-vs-dragoff-{1440,390}-{light,dark}.png` |
| Bridge replay, half the sky masked vs the plain run (T+10:20, just after the blanking at T+10:00) | `replay-masked-sky-blanking-*` |
| Training debrief with the skyplot, vessel glyph, heading and error ring (coastal drag-off, row 81) | `training-coastal-drag-off-*` |
| Maritime trust with the skyplot, glyph and ring (masked-sky demo, cursor 650 s) | `trust-masked-sky-*` |
| Optical PNT, three sites with diversity | `optical-network-*` |
| Optical PNT, enhancement techniques | `optical-techniques-*` |

`smoke.mjs` is the functional check that ran alongside: play, pause, step, moment steps, keyboard, verdict at the end, the half sky blanking at T+600 s with the same satellites listed, both exports (animated SVG 1.7 MB, frames ZIP 4.6 MB) and both optical scenarios.

All data on these pages is synthetic or recorded engine output; the coast is Natural Earth public-domain land for orientation only.
