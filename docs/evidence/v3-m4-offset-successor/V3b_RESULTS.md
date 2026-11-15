# V3b results: the M4 position-offset bar, corrected stage-2 method `kshana-pro/m4-offset/v4b`

Pre-registration: `2026-11-15_V3b_PREREGISTRATION.md` (committed alone, `8a6bf4d8`, before its code
and seeds). Development run on seeds 60 to 69 and the chosen pair committed before seed 6000 was
generated (`V3b_dev.json`). Scored once on seeds 6000 to 6009 (20 000 vessels) by
`KSHANA_PRO_V3B_SCORE=1 cargo test --test integrity_v3 v3b_scored`; numbers in `V3b_results.json`.
MODELLED: synthetic scenes. V3 (`RESULTS.md`) stays published as NOT MET.

## Development run (seeds 60 to 69)

No pair met FA ≤ 0.005 (FA 0.427, 0.499, 0.075, 0.092). By the rule the lowest-FA pair (m = 3,
G_pair = 13.816) was committed and the miss was expected before the scored run.

## Scored run (seeds 6000 to 6009)

| Bar | Result |
|---|---|
| **V3-B4 position offset** | **NOT MET**: recall 0.829 at 5 km (95 % 0.779 to 0.871; bar ≥ 0.85), 0.857, 0.829 and 0.896 at 10, 20 and 50 km (bar ≥ 0.98 each); 0.075 at 2 km (no bar). Stage 1 alone: 0.493, 0.632, 0.675, 0.714 |
| V3-G1 false alarms | **NOT MET**: FA 0.072 (1 156 of 16 000; 95 % 0.068 to 0.076), bar ≤ 0.01. Stage 1 alone: 0.0006 |
| V3-G2 dark | MET: recall 0.995, precision 0.958 |
| V3-G3 gaps | MET: recall 1.000 |
| V3-G4 identity duplicate | MET: recall 1.000 |
| V3-G5 determinism | MET (`tests/integrity_v3.rs`) |
| V3-G6 additivity | MET: stage 1 is the open engine's, unchanged |

**V3b is NOT MET** (V3-B4 and V3-G1).

| true \ decided | NORMAL | DARK | GAP | OFFSET | DUPLICATE |
|---|---|---|---|---|---|
| NORMAL | 14 844 | 5 | 0 | 1 151 | 0 |
| DARK | 7 | 1 393 | 0 | 0 | 0 |
| GAP | 0 | 0 | 600 | 0 | 0 |
| POSITION-OFFSET | 368 | 56 | 0 | 976 | 0 |
| IDENTITY-DUPLICATE | 0 | 0 | 0 | 0 | 600 |

## What it shows, and what E5 does with it (descriptive, after the run)

- Excluding the vessel's own fixes halved the false alarms (0.132 to 0.072) but they stay seven
  times over the bar: in 6 h scenes of 2 000 vessels, two or three unassociated fixes (mostly dark
  vessels' and offset vessels' own displaced fixes) often line up at a constant displacement from
  an unrelated AIS track over 10 minutes or more. A successor would need a test of the relative
  motion over the whole support (velocity consistency) or exclusivity between identities competing
  for the same fixes; that is a new pre-registration, not done here.
- **E5 consequence:** neither stage-2 method met its bars, so the E5 scoring never lets stage 2
  change a class. Classes come from the open M4 v3 decision only (FA 0.0006 to 0.0009 on these
  seeds); stage-2 hits are listed as "offset candidates for review", marked as coming from a method
  that missed its bars. The E5 position-offset class therefore carries the open M4 v3 performance,
  which also missed its offset bar (published in the open repository).
