# V3c results: the M4 position-offset bar, relative-motion test (16 November 2026)

Pre-registration: `2026-11-16_V3c_PREREGISTRATION.md` (approved by the owner 16 Nov 2026, approval
commit 60c91516). Order kept: the development **recall ceiling** (`V3c_dev_ceiling.json`) was
committed first (2f4119ca), from the generator's truth only, before any V3c code existed; then the
method `kshana-pro/m4-offset/v5` (`src/integrity/offset_v5.rs`), the development table and the
chosen pair (`V3c_dev.json`, 62d18b05), before seed 7000 was generated. Scored once on seeds 7000
to 7009 (20 000 vessels): `V3c_results.json`. SYNTHETIC: the open M4 generator of kshana 0.41.0,
MODELLED.

## Verdict: V3c NOT MET (V3-B4 missed; every guard met)

| Bar | Bar as registered | Scored (7000-7009) | Outcome |
|---|---|---|---|
| V3-B4 | offset recall >= 0.85 at 5 km, >= 0.98 at 10, 20, 50 km | 0.650 / 0.775 / 0.796 / 0.761 (2 km, no bar: 0.014) | **MISSED** |
| V3-G1 | false alarms <= 0.01 | 0.0059 (95 % CI 0.0048 to 0.0073) | Met |
| V3-G2 | DARK recall >= 0.95, precision >= 0.80 | 0.980, 0.917 | Met |
| V3-G3 | GAP recall >= 0.85 | 1.000 | Met |
| V3-G4 | IDENTITY-DUPLICATE recall >= 0.95 | 1.000 | Met |
| V3-G5 | determinism | same classes, flags and findings on a repeat (`v3c_g5_g6_*`) | Met |
| V3-G6 | stage 1 the open engine's byte for byte | `decide(scene, &M4Params::v3())` unchanged; stage 2 changes only NORMAL to POSITION-OFFSET and clears only support fixes' flags | Met |

Parameters (chosen on development seeds by V3's rule): m = 4, T_span = 1200 s. Stage-2
findings on the scored seeds: 218.

Confusion matrix (rows truth, columns decided; NORMAL, DARK, GAP, POSITION-OFFSET, IDENTITY-DUPLICATE):

| Truth \ decided | NORMAL | DARK | GAP | OFFSET | ID-DUP |
|---|---|---|---|---|---|
| NORMAL | 15905 | 5 | 0 | 90 | 0 |
| DARK | 28 | 1372 | 0 | 0 | 0 |
| GAP | 0 | 0 | 600 | 0 | 0 |
| POSITION-OFFSET | 442 | 119 | 0 | 839 | 0 |
| IDENTITY-DUPLICATE | 0 | 0 | 0 | 0 | 600 |

## Recall ceiling (development seeds 70 to 79, generator truth; published before the method)

Share of offset vessels with at least `m` post-onset RF fixes of their own spanning `T_span`:

| m, T_span | 2 km | 5 km | 10 km | 20 km | 50 km |
|---|---|---|---|---|---|
| 3, 1200 s | 0.982 | 0.986 | 0.989 | 0.986 | 0.975 |
| 3, 1800 s | 0.982 | 0.986 | 0.989 | 0.986 | 0.975 |
| 4, 1200 s | 0.896 | 0.904 | 0.843 | 0.886 | 0.868 |
| 4, 1800 s | 0.896 | 0.904 | 0.843 | 0.886 | 0.868 |

At every pair the ceiling is below 0.98 at 50 km (0.975 at m = 3; 0.868 at m = 4), so by the
pre-registration V3-B4 is **not reachable on this generator with a method of this kind**. The owner
directed the scored run anyway (16 Nov), and it is published here. At the chosen m = 4 the ceiling
itself (0.84 to 0.90) is below every 0.98 bar.

## Development table (seeds 70 to 79)

| m | T_span | FA | recall 5 / 10 / 20 / 50 km |
|---|---|---|---|
| 3 | 1200 s | 0.0312 | 0.750 / 0.786 / 0.818 / 0.814 |
| 3 | 1800 s | 0.0312 | 0.750 / 0.786 / 0.818 / 0.814 |
| 4 | 1200 s | 0.0070 | 0.668 / 0.754 / 0.729 / 0.743 |
| 4 | 1800 s | 0.0070 | 0.668 / 0.754 / 0.729 / 0.743 |

No pair met FA <= 0.005, so the lowest-FA pair was taken (m = 4; `T_span` 1200 and 1800 s gave
identical results, ties to the smaller).

## Reading (post hoc, not a re-score)

* The relative-motion test and exclusivity did what section 1 expected for false alarms: 0.0059
  against V3b's 0.072 and V3's 0.132, inside the 0.01 guard.
* Recall fell against V3b (0.829 / 0.857 / 0.829 / 0.896): m = 4 was forced by the false-alarm
  rule and its ceiling is 0.84 to 0.90. Even at m = 3 the development recall was 0.75 to 0.82,
  under its own ceiling (0.975 to 0.989); where the remaining loss sits (relative-motion
  rejections, exclusivity, contradiction) was not separated and would need a new
  pre-registration, not a re-score.
* 119 offset vessels were decided DARK (8.5 %), 442 NORMAL.
* On the development seeds, no method that needs at least three post-onset fixes of the vessel can
  reach 0.98 at 50 km on this generator (ceiling 0.975); a successor would need a generator with
  more RF accesses after onset, or a different kind of evidence.

The E5 product keeps classes from the open M4 v3 decision, and stage-2 hits stay "offset
candidates for review". V3 and V3b stay published as not met, as do the open engine's M4 v1 to
v3 rows.

