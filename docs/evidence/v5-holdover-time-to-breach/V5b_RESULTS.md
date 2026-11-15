# V5b results: the successor holdover predictor (scored 2026-11-15)

**Outcome: V5b MISSED.** Two bars missed on the real data (B1 narrowly, B5 clearly); every bar
passed on the synthetic data. Under both pre-registrations, T1 is **not claimed**. V5's own
miss (`RESULTS.md`) is unchanged. Nothing below was tuned after the run.

* Pre-registration: `2026-11-15_V5b_PREREGISTRATION.md` (commit 00a8a478, pushed before the
  successor predictor existed or ran on these data).
* Predictor scored: `HoldoverModel::fit_kalman` in `src/timing/model.rs` as first committed
  with this file (the open engine's three-state clock Kalman filter for the handover state).
* Scoring: `tests/timing_v5.rs` (`v5b_scored_as_pre_registered_and_published`); numbers in
  `V5b_results.json`, recomputed and compared on every run.

## D-R2: real recorded clocks (IGS, 2025-08-17 to 2025-08-30, data V5 did not use)

220 runs planned; 12 excluded (G03's one-day gap), 208 scored; 1,456 coverage points; 381
pairs with a breach within 48 h, 243 without.

| Bar | Metric | Value | Pass if | Verdict |
|---|---|---|---|---|
| B1 | 95 % interval coverage | 0.8935 | 0.90 to 0.99 | **MISS** |
| B2 | Early warning share | 0.9501 | >= 0.90 | pass |
| B3 | Median T_warn / T_act | 0.4387 | >= 0.20 | pass |
| B4 | T_50 within a factor 2 | 0.8268 | >= 0.60 | pass |
| B5 | False-alarm share | 0.4897 | <= 0.25 | **MISS** |

Per clock (coverage; early warning; false alarms of pairs without a breach): G25 0.65, 0.79,
8/31; G26 0.76, 1.00, 12/12; G24 0.79, 0.88, 19/28; G03 0.89, 0.60, 9/9 (8 runs); the other
seven clocks 0.91 to 0.99 coverage and 0.94 to 1.00 early warning. Full table in
`V5b_results.json`.

## D-S2: synthetic mixed-class fleet (SYNTHETIC, seed 20261116)

100 runs, 0 excluded, 600 coverage points, 189 pairs with a breach within 24 h, 111 without.

| Bar | Metric | Value | Pass if | Verdict |
|---|---|---|---|---|
| B1 | 95 % interval coverage | 0.9033 | 0.90 to 0.99 | pass |
| B2 | Early warning share | 0.9153 | >= 0.90 | pass |
| B3 | Median T_warn / T_act | 0.5561 | >= 0.20 | pass |
| B4 | T_50 within a factor 2 | 0.8148 | >= 0.60 | pass |
| B5 | False-alarm share | 0.1441 | <= 0.25 | pass |

## Reading (written after the scoring; not a claim)

* Against V5 the successor fixes what V5's diagnosis found: on clocks that follow the noise model
  (the synthetic set) the interval is calibrated and the warning comes before the breach. On the
  real data, early warning (0.95) and the median time (0.83 within a factor 2) are much better
  than V5's (0.63).
* The real rubidium clocks are not fully described by three noise terms: four of the eleven
  (G24, G25, G26, and G03 on its few runs) are under-covered, which is where coverage falls just
  below 0.90. Orbit-period terms and flicker frequency noise, not modelled, are the likely
  causes; that is a hypothesis to test, not a finding.
* B5 missed on real data in both V5 and V5b. As written, B5 counts every pair whose median time
  falls inside the horizon but whose actual error stayed below the limit; a calibrated predictor
  can miss it whenever the breach probability by the horizon is near one half. Any change to that
  bar would be a new pre-registration with its reason stated, on new data; it is not changed here.

The fleet assessment (`pro-timing-holdover`) uses the successor predictor for free-running
records, because it is the better-founded of the two; every result's label says that neither V5
nor V5b passed.
