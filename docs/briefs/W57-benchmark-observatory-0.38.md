# W57 brief: Kshana 0.38 WS-F, open benchmark and interference observatory

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md "Scope extension" WS-F, strategy memo
moves 2 and 8 in full (including the risks), docs/data/INTERFERENCE-DATA-SOURCES.md in kshana.

## Branch you own
- kshana: feat/bench-observatory-0.38 from origin/main.

## Items
F1 kshana-bench v1: bench/ directory with SPEC.md (versioned), split manifests (SHA-256, fetch scripts, never the
   data), metric code via impairment-eval, tasks (a) detection on receiver logs, (b) detection on IQ, (c) timing
   holdover prediction, (d) out-of-distribution optimism gap; baseline scores; a sealed holdout (hash committed,
   contents not); results JSON schema; a static leaderboard page (deliver the page source to W53's Studio/site via
   the coordinator, or as web-ready HTML under bench/site/). Mark the spec/manifests licence as "CC-BY-4.0 / CC0
   proposed, owner decision pending" until the coordinator confirms. Task (a) should use W54's Jammertest scorer
   (kshana feat/referee-0.38) once pushed: merge it into your branch then, don't duplicate it.
   Per-dataset licence review first (TEXBAT/OAKBAT/JammerTest/DataPort/LuGRE): reference, never copy.
F2 Observatory: docs/observatory/METHOD.md (NACp-vs-almanac-HDOP method, cited), a reproducible daily product
   command producing GeoJSON + PNG + incident-report JSON from interference-map, a 30-day Baltic back-catalogue run
   from adsb.lol with a sensitivity study. Publish only degraded / not degraded / not observed; >=24 h delay; coarse
   cells; NO attribution of causes; OpenSky excluded (licence). Large outputs stay out of git (manifest + hashes).

## Done when
Commands on every surface (or N/A with reason); docs; pre-registered bars for any baseline that claims a number;
gates green; report per common rules.
