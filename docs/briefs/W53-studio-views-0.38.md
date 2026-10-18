# W53 brief: Kshana 0.38 WS-A, the Studio views from the promo, built for real (+ D2)

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md section WS-A.
Reference look: kshana-pro docs/handover-0.37/video/kshana-0.36-0.37-promo-branded-poster.png and the MP4 there.

## Branches you own
- kshana-pro: site/studio-0.38, created from origin/port/0.37.0 (the Studio/site source; site-next/ runbooks:
  PORT-RUNBOOK.md, RE-PORT.md). Use add_repo for kshana-pro push access if needed.
- kshana: feat/studio-replay-0.38 from origin/main, ONLY for engine/wasm changes the views need (e.g. A6 export,
  exposing skyline/sky data to wasm). Don't edit kshana web/ by hand: it is produced by web/tools/port_site.py at
  integration.

## Items (plan IDs)
A1 Bridge replay: two runs side by side on one shared mission clock (Trust + Training), per panel track, score,
   band and verdict; play/pause/step/speed; keyboard accessible.
A2 Live skyplot in Trust and Training synced to the cursor, from the log's GSV; masked/skyline sectors shaded.
   The masked-sky demo must show the half sky blanking at T+600 s. The browser invents no engine numbers.
A3 Optional offline coastline under the track: Natural Earth (public domain) or interference-map fetch-land output,
   bundled small; nothing fetched at runtime without consent.
A4 Vessel glyph + heading (true vs reported) and an error ring in metres, matching the instructor JSON.
A5 Optical PNT page /studio/optical/ for the two 0.37 optical-atmosphere scenarios ("model output" labels).
A6 Export the replay (video frames / animated SVG), reusing the engine's --animate where possible.
A7 Kshana brand tokens and logo on all new views (tokens.css / theme.css; mark web/assets/brand/kshana-mark-mask.png).
D2 Site: replace the hand-typed "160 bundled scenarios" (actual 175) with a generated count, wherever it appears.

## Done when
Studio selfcheck and lib tests green; check.py (incl. rule 27: every class has a CSS rule) clean; check_features
clean; Playwright screenshots of every new view at 1440 and 390 px, light and dark, saved on your branch under
site-next/explorations/2026-10-xx-studio-0.38/ and listed in your report for coordinator review BEFORE final push.
Vessel/coast art stays labelled as illustration only where it is not data.
