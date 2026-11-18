# V5c pre-registration: holdover time to breach with orbit-period and flicker terms (roadmap 0.42 T1)

**Status: APPROVED by the owner, 16 Nov 2026.** (Was PROPOSED; approval recorded in `docs/evidence/BARS-TO-GREEN-0.42.md` before any new evaluation data were generated or read.) Written on 2026-11-15 (21:40 UTC) and committed alone, before any code of
the method below exists and before any byte of data set D-R3 has been downloaded or read, and
before seed D-S3 has been generated. It takes effect on the owner's approval (dated 2026-11-16).
V5 (`RESULTS.md`) and V5b (`V5b_RESULTS.md`) stay published as missed whatever happens here. A bar
may be tightened later, never loosened. The result goes in `V5c_RESULTS.md` next to this file,
met or missed.

## 0. What was seen first (disclosure)

* V5 and V5b were read in full: their bars, per-clock figures and post hoc readings. Their four
  data sets (D-R, D-R2, D-S, D-S2) are the **development data** of V5c.
* D-R3 (section 3): only the directory listing of the BKG mirror for GPS weeks 2387 and 2388 was
  read on 2026-11-15 (file names and sizes). No clock file of that period has been downloaded,
  opened or cut by anyone working on Kshana or Kshana Pro, as far as the repositories show: the
  only IGS final clock periods named in this repository are 2025-08-17 to 2025-08-30 (DOY 229 to
  242) and 2025-09-01 to 2025-09-14, plus one file of DOY 055 (searched 2026-11-15).
* D-S3: seed 20261120 appears nowhere in this repository (searched 2026-11-15).

## 1. Diagnosis of the misses (from the published results only)

| Row | Bar missed | Value | Bar |
|---|---|---|---|
| V5 D-R | B5 false alarms | 0.5289 | <= 0.25 |
| V5 D-S | B1 coverage, B2 early warning, B4 factor 2 | 0.6733, 0.4730, 0.2928 | 0.90 to 0.99, >= 0.90, >= 0.60 |
| V5b D-R2 | B1 coverage, B5 false alarms | 0.8935, 0.4897 | 0.90 to 0.99, <= 0.25 |

1. **Synthetic misses (V5 D-S) are explained and were fixed by V5b.** V5's handover state came
   from a 48 h least-squares fit whose white-residual covariance under-states its error on
   random-walk-dominated clocks (V5 `RESULTS.md`). V5b's three-state Kalman handover passed every
   bar on a new synthetic seed (D-S2: 0.9033, 0.9153, 0.5561, 0.8148, 0.1441). V5c keeps that
   handover unchanged and re-tests it on a new seed.
2. **Real-data coverage (V5b D-R2 B1) is wrong in both directions, clock by clock.** Under-covered:
   G25 0.65, G26 0.76, G24 0.79, G03 0.89 (8 runs). Over-covered: G32 0.993, G09 0.986, G08 0.979.
   A single three-term noise model (white phase, white FM, random-walk FM) is too narrow on some
   clocks and too wide on others; the overall 0.8935 is an average of the two errors.
3. **Real-data false alarms (B5) concentrate on a few clocks and repeat across both periods.**
   V5: G25 30/50, G30 30/36, G09 20/20. V5b: G30 24/38, G24 19/28, G09 16/16, G26 12/12. G09
   flags every one of its non-breach pairs in both periods while its interval is over-covered
   (1.000 and 0.986): its predicted spread at 48 h is too wide, so the breach probability passes
   0.5 inside the horizon although the error stays below the limit.
4. **Hypothesis for 2 and 3 (unverified; to be checked on development data only).** IGS
   satellite clock estimates carry periodic terms at the GPS orbital period (about 11.97 h) and
   its harmonics (Senior, Ray and Beard, "Characterization of periodic variations in the GPS
   satellite clocks", GPS Solutions 12, 2008, 211-225), and rubidium clocks show flicker
   frequency noise at averaging times of hours. Over a 48 h window a 12 h periodic term raises the
   Allan variance at 3 to 6 h, and the three-term fit can only absorb it as random-walk FM, whose
   prediction variance grows as `u^3`: too wide at 48 h on clocks with a strong periodic term
   (G09 fits that pattern). A clock dominated by flicker FM with a weak periodic term gets a
   random-walk term too small at the short horizons: too narrow (G24, G25, G26 would fit). The
   per-clock split between the two causes is not in the published results.
5. **What B5 needs (structural note, not a change).** For a calibrated, zero-mean normal
   prediction, `T_50 <= H` exactly when `s(H) >= L / 0.674`; B5 counts the pairs flagged that way
   whose actual error stayed below `L`. A calibrated predictor passes B5 only if few pairs sit at a
   breach probability between about 0.5 and 0.8 by the horizon. The only lever that leaves the
   bar unchanged is a sharper predictive distribution: removing variance that is deterministic
   (periodic) rather than random. That may not be enough (section 7).

## 2. The method (fixed now; `kshana-pro/holdover/v3`, `HoldoverModel::fit_harmonic`)

Everything as V5b section 2 (window `W = 48 h`, grid, the three-state Kalman handover, the drift
offset in the noise fit, `R_eff`), with three changes. Phases are relative to the window's first
sample; `v = t - t_h`.

1. **Periodic terms.** Before step 2 of V5b, fit by least squares over the window
   `x(t) = a + b v + c v^2 / 2 + sum_{k=1,2} (alpha_k cos(2 pi k v / P) + beta_k sin(2 pi k v / P))`
   with `P = 43 082 s` (half a sidereal day: the GPS orbital period, a constant of the
   constellation, never fitted). The harmonic part `h(t)` is subtracted from the window's phase;
   V5b's steps 2 and 3 (noise fit, Kalman handover) run on `x - h`. The coefficient covariance
   `C_h` is the least-squares covariance scaled by `kappa = N / N_eff`, `N_eff = W / tau_c`,
   `tau_c` the first lag at which the autocorrelation of the least-squares residuals falls below
   `1/e` (at least `tau0`), so that correlated residuals do not shrink it.
2. **Flicker frequency term.** The Allan model becomes
   `sigma_y^2(tau) = 3 R / tau^2 + q_wf / tau + F + q_rw tau / 3` (`F >= 0`, the flicker floor),
   fitted exactly as V5b's (relative least squares, drift offset fixed, exhaustive over the 16
   non-negative subsets).
3. **Prediction.** `m(u) = dh u^2 / 2 + h(t_h + u) - h(t_h)`;
   `s(u)^2 = (V5b's s(u)^2) + g_h C_h g_h^T + F u^2`, `g_h` the gradient of
   `h(t_h + u) - h(t_h)` with respect to `(alpha, beta)`. The `F u^2` term is the usual
   `x_rms(u) ~ u sigma_y(u)` approximation for flicker FM; it is an approximation and is stated
   as one in every output.

Probabilities, `T_warn`, `T_50`: as V5. No parameter above is tuned on D-R3 or D-S3. The servo
record path (class or datasheet prior) is unchanged and not scored.

**Development (allowed, published before scoring).** The code is run on D-R, D-R2, D-S and D-S2
(all already published) only to check that it implements this text; the table is committed as
`V5c_dev.json` before D-R3 is downloaded. No constant above may change after this file is
committed. If the development run shows the method cannot pass, the owner may withdraw V5c
before D-R3 is read; a withdrawal is published in `V5c_RESULTS.md` with the development numbers.

## 3. Data

### D-R3: real recorded clocks (public; new download, owner approval needed)

IGS final combined clocks at 30 s, `IGS0OPSFIN_20252780000_01D_30S_CLK.CLK.gz` to
`IGS0OPSFIN_20252910000_01D_30S_CLK.CLK.gz` (2025-10-05 to 2025-10-18, GPS weeks 2387 and 2388),
from the BKG mirror of the IGS products, `https://igs.bkg.bund.de/root_ftp/IGS/products/2387/` and
`.../2388/`: 14 files of 3.0 to 3.1 MB (listing of 2026-11-15), about 43 MB. Terms: IGS data and
products are openly available with attribution (as the V5 and V5b fixtures). The same eleven GPS
Block IIF clocks (G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32), decimated to 300 s with the
same cutter and layout as `tests/fixtures/timing_v5b_igs/` (header noise values written by the
cutter are not read by the predictor). Committed as `tests/fixtures/timing_v5c_igs/clocks.txt`
with a `NOTICE.md`; its SHA-256 is written into `V5c_RESULTS.md` before any scoring. A clock
missing from the files is reported and left out; gaps follow V5's exclusion rule.

Handovers (7.00 d to 11.75 d every 0.25 d), window, horizon (48 h), coverage horizons (1, 2, 4, 8,
12, 24, 48 h) and limits (1, 3, 10 ns): as V5's D-R.

### D-S3: synthetic mixed-class fleet (SYNTHETIC; not a measurement)

As V5b's D-S2 in every respect, seed **20261120**.

## 4. Bars (unchanged)

V5 section 4, word for word in effect: B1 coverage `0.90 <= c <= 0.99`; B2 early warning
`>= 0.90`; B3 median `T_warn / T_act >= 0.20`; B4 `T_50` within a factor 2 `>= 0.60`; B5
false-alarm share `<= 0.25`; B2 to B4 need 30 breach pairs, B5 30 non-breach pairs; a
not-assessable B2, B3 or B4 counts as not passed. **V5c passes only if B1 to B4 pass, and B5
passes or is not assessable, on D-R3 and D-S3 both.** T1 may be called claimed only if V5c
passes; V5 and V5b remain published misses.

## 5. Scoring

```
KSHANA_PRO_V5C_SCORE=1 cargo test --release --test timing_v5 v5c_scored_as_pre_registered_and_published -- --nocapture
```

run once; it writes `V5c_results.json`. The test then recomputes and compares on every run
(the same pattern as V5b; the scoring goes in the existing `tests/timing_v5.rs`, so the README's
integration-test count is unchanged).

## 6. What counts as a miss, and publication

Any bar failing, or any of B2 to B4 not assessable, on either data set is a miss. The result is
published in `V5c_RESULTS.md` with every bar, the counts and the per-clock figures, whatever it
is, and `docs/TIMING-ASSURANCE.md` keeps its "not validated" wording unless V5c passes.

## 7. Feasibility (for the owner; not part of the method)

* Effort: about 2 to 3 days (harmonic and flicker extension of `src/timing/model.rs`, cutter run,
  scoring in the existing test). Disk: 43 MB download, about 0.5 MB committed.
* Risk: B1 moderate (the hypothesis in 1.4 is unverified). **B5 high:** even a calibrated
  predictor misses B5 when many (run, limit) pairs have a breach probability between 0.5 and
  0.8 at 48 h; with limits of 1, 3 and 10 ns on rubidium satellite clocks that may be the case.
  If V5c meets B1 to B4 and misses B5 again, the honest reading is that B5 as written is not
  reachable on this clock population, and that is published, not re-scored.
