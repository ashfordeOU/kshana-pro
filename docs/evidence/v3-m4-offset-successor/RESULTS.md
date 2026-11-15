# V3 results: the M4 position-offset bar, stage-2 method `kshana-pro/m4-offset/v4`

Pre-registration: `2026-11-15_PREREGISTRATION.md` (committed alone, `5b3ded07`). Development run on
seeds 50 to 59 and the chosen pair committed in `1ea8071f` before seed 5000 was generated
(`dev.json`). Scored once on seeds 5000 to 5009 (20 000 vessels) by
`KSHANA_PRO_V3_SCORE=1 cargo test --test integrity_v3 v3_scored`; numbers in `results.json`.
MODELLED: synthetic scenes of the open M4 generator. The open engine's M4 v1 to v3 results stay
published as NOT MET.

## Development run (seeds 50 to 59)

No grid pair met FA ≤ 0.005 (FA 0.454, 0.549, 0.140, 0.221 for m = 2 and 3 at 13.816 and
18.421). By the pre-registered rule the lowest-FA pair (m = 3, G_pair = 13.816) was committed and
the miss was expected before the scored run.

## Scored run (seeds 5000 to 5009)

| Bar | Result |
|---|---|
| **V3-B4 position offset** | **NOT MET**: recall 0.861 at 5 km (241 of 280; 95 % 0.815 to 0.899; bar ≥ 0.85, this part met), 0.879, 0.871 and 0.871 at 10, 20 and 50 km (bar ≥ 0.98 each, missed); 0.157 at 2 km (no bar). Stage 1 alone (open M4 v3) on the same seeds: 0.504, 0.679, 0.675, 0.671 |
| V3-G1 false alarms | **NOT MET**: FA 0.132 (2 109 of 16 000 NORMAL vessels; 95 % 0.127 to 0.137), bar ≤ 0.01. Stage 1 alone: 0.0008 |
| V3-G2 dark | MET: recall 0.995, precision 0.963 |
| V3-G3 gaps | MET: recall 1.000 |
| V3-G4 identity duplicate | MET: recall 1.000 |
| V3-G5 determinism | MET: `tests/integrity_v3.rs` decides development seed 50 twice and compares |
| V3-G6 additivity | MET: stage 1 is the pinned open engine's `decide(&scene, &M4Params::v3())`, unchanged; Pro adds stage 2 only |

**V3 is NOT MET** (V3-B4 and V3-G1).

Confusion matrix (rows true, columns decided; NORMAL, DARK, GAP, POSITION-OFFSET,
IDENTITY-DUPLICATE):

| true \ decided | NORMAL | DARK | GAP | OFFSET | DUPLICATE |
|---|---|---|---|---|---|
| NORMAL | 13 891 | 9 | 0 | 2 100 | 0 |
| DARK | 7 | 1 393 | 0 | 0 | 0 |
| GAP | 0 | 0 | 600 | 0 | 0 |
| POSITION-OFFSET | 336 | 45 | 0 | 1 019 | 0 |
| IDENTITY-DUPLICATE | 0 | 0 | 0 | 0 | 600 |

## What it shows (descriptive, after the run)

- The parallel-track test finds far more offsets than stage 1 alone (1 019 against 705 at all
  distances), but 2 100 NORMAL vessels are flagged. The cause, diagnosed from the method text before
  the scored run (`2026-11-15_V3b_PREREGISTRATION.md`, section 0): a vessel's own fix, where AIS
  says it is, can support a hypothesis of up to the pairwise radius (5.26 km), so one stray fix
  3.7 to 5 km away plus the vessel's own fixes passes.
- The successor, V3b, was pre-registered before its code and seeds (`8a6bf4d8`); its result is in
  `V3b_RESULTS.md`. This V3 result stands as published.
