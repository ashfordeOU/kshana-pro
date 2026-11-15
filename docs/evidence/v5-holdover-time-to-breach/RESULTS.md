# V5 results: holdover time-to-breach prediction bars (scored 2026-11-15)

**Outcome: V5 MISSED.** Four bars missed (one on the real data, three on the synthetic data).
Under the pre-registration, T1 ("holdover and time to breach for every clock") is **not
claimed**. This page stays published as it is; nothing below was tuned after the run.

* Pre-registration: `2026-11-15_PREREGISTRATION.md` (commit 855436be, pushed before any
  predictor or scoring code existed).
* Predictor scored: `src/timing/model.rs` as first committed with this file
  (`HoldoverModel::fit`, the method of pre-registration section 2).
* Scoring: `tests/timing_v5.rs`; the numbers are `results.json`, which the test recomputes and
  compares on every run (`cargo test --test timing_v5 -- --nocapture` prints the table).

## Bars

### D-R: real recorded clocks (IGS, eleven GPS IIF rubidium clocks; holdover emulated)

220 runs (11 clocks x 20 handovers), 0 excluded (the one gap of G24 falls outside every scored
span), 1,540 coverage points, 418 (run, limit) pairs with a breach within 48 h, 242 without.

| Bar | Metric | Value | Pass if | Verdict |
|---|---|---|---|---|
| B1 | 95 % interval coverage | 0.9766 | 0.90 to 0.99 | pass |
| B2 | Early warning share | 0.9952 | >= 0.90 | pass |
| B3 | Median T_warn / T_act | 0.2382 | >= 0.20 | pass |
| B4 | T_50 within a factor 2 | 0.6316 | >= 0.60 | pass |
| B5 | False-alarm share | 0.5289 | <= 0.25 | **MISS** |

### D-S: synthetic mixed-class fleet (SYNTHETIC, not a measurement)

100 runs (20 clocks x 5 handovers), 0 excluded, 600 coverage points, 222 pairs with a breach
within 24 h, 78 without.

| Bar | Metric | Value | Pass if | Verdict |
|---|---|---|---|---|
| B1 | 95 % interval coverage | 0.6733 | 0.90 to 0.99 | **MISS** |
| B2 | Early warning share | 0.4730 | >= 0.90 | **MISS** |
| B3 | Median T_warn / T_act | 1.8903 | >= 0.20 | pass |
| B4 | T_50 within a factor 2 | 0.2928 | >= 0.60 | **MISS** |
| B5 | False-alarm share | 0.1282 | <= 0.25 | pass |

Per-clock figures for both sets are in `results.json`.

## Implementation notes (no effect on the method)

* D-S: "four days per clock" is generated as four days spanned, 34,561 samples at 10 s, so the
  last handover (72 h) has its full 24 h horizon.
* The real data file's header noise values are not read by the predictor.

## What the miss shows (post hoc diagnosis, written after the scoring; not a claim)

Looked at after the bars were scored, on the same data, so it explains the miss and decides
nothing about it:

1. **The handover state comes from a 48 h quadratic fit, and its error is under-stated.** For
   a clock dominated by random-walk frequency noise (every synthetic class at 48 h; less so the
   rubidium satellite clocks), the fitted phase and frequency at the window's end are off by
   about the residual's size, because the residuals are strongly correlated. The least-squares
   covariance assumes white residuals and so reports a far smaller error. On the synthetic set
   this shows as a holdover error that is already large at the first step (a constant offset
   the interval does not cover): coverage 0.67, early warning 0.47.
2. **B5 on the real data.** With the interval slightly wide (coverage 0.977), the median time to
   breach often falls inside 48 h for pairs whose actual error stayed below the limit; G09,
   G25 and G30 account for most of these.

A successor method (a three-state Kalman filter for the handover state, with its full
covariance) is pre-registered separately as V5b, on data V5 did not use, before it is scored.
Its outcome is published beside this one and does not change this result.
