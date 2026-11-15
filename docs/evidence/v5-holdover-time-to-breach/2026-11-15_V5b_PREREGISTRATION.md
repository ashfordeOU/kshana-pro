# V5b pre-registration: the successor holdover predictor, on data V5 did not use

Written and committed on **2026-11-15 (09:15 UTC)**, alone, after V5 was scored and missed
(`RESULTS.md`, commit 8628188e) and **before** the successor predictor is written or run on the
data below. V5's result stands unchanged whatever happens here. The design below was informed by
the post hoc diagnosis of V5 on V5's data (stated in `RESULTS.md`); it is scored only on data
that V5 did not touch and on a new synthetic seed. Bars may be tightened later, never loosened;
a miss is published in `V5b_RESULTS.md` with its numbers.

## 1. What is predicted

Exactly as V5 (pre-registration section 1): the holdover time error
`TE(u) = x(t_h + u) - (xh + yh u)` with `xh`, `yh` the predictor's phase and frequency at the
handover, a normal prediction `N(m(u), s(u)^2)`, the warning time `T_warn` (breach probability
first >= 0.05) and the median time `T_50` (>= 0.5) on the grid `u = k tau0`.

## 2. The successor predictor (fixed now)

Training window `[t_h - W, t_h]`, `W = 48 h`, `N` samples at `tau0`; all phases are taken
relative to the window's first sample.

1. **Drift** `D`: least-squares quadratic over the window (as V5); its drift coefficient.
2. **Noise:** overlapping Allan variance of the **raw** window phase (no detrending) at
   `m = 1, 2, 4, ...` while `m <= N / 8`; fit `3 R / tau^2 + q_wf / tau + q_rw tau / 3`, with the
   known drift term `D^2 tau^2 / 2` added as a fixed offset, by least squares on relative residuals
   with all three terms non-negative (exhaustive over subsets, as V5).
   `R_eff = max(R, (1e-13 s)^2 / 12)`.
3. **Handover state:** the open engine's three-state clock Kalman filter
   (`kshana::clock_state::ClockState3`, phase, frequency, drift; process noise `q_wf`, `q_rw`,
   random-run zero), started at the window's first sample with state `(0, y0, 0)`, `y0` the
   least-squares slope over the window's first quarter, and diagonal covariance
   `(s_x^2, (1e-8)^2, (1e-12 /s)^2)`, `s_x = 10 sd(first-quarter line residuals) + sqrt(R_eff)`;
   then predict and update with every sample (measurement variance `R_eff`) up to `t_h`. The
   result: state `(xh, yh, dh)` and covariance `P`.
4. **Prediction:** `m(u) = dh u^2 / 2`;
   `s(u)^2 = g P g^T + q_wf u + q_rw u^3 / 3 + R_eff`, `g = (1, u, u^2 / 2)`.

The servo-record path (a class or datasheet prior) is unchanged and not scored here.

## 3. Data

### D-R2: real recorded clocks (public), not used by V5

`tests/fixtures/timing_v5b_igs/clocks.txt` (SHA-256
`3da6d0a5a7facdc1e564fdeed5f5ada890a2f9cdab6a218fa79a83608a3230a4`): IGS final combined
satellite clocks, **2025-08-17 to 2025-08-30**, the same eleven GPS IIF rubidium clocks, 300 s.
Free-running; holdover emulated. Honest note: the open engine's clock-filter tests used this file;
neither the V5 nor the V5b predictor has been run on it. G03 has one gap of 288 epochs.

Handovers, window, horizon, coverage horizons, limits (1, 3, 10 ns) and the exclusion rule: as
V5's D-R.

### D-S2: synthetic mixed-class fleet (SYNTHETIC), new seed

As V5's D-S (four days spanned, 34,561 samples at 10 s per clock; same classes, drifts, offsets
and phase noise, same handovers, window, horizon and limits), generated with seed **20261116**.

## 4. Metrics and bars

Identical to V5 section 4 (B1 to B5, the same thresholds and assessability rules). **V5b passes**
only if B1 to B4 pass, and B5 passes or is not assessable, on D-R2 and D-S2 both.

## 5. What a pass would and would not mean

A pass validates the successor predictor on these two data sets (real rubidium satellite clocks
with emulated holdover; synthetic clocks). It does not change V5's published miss, and it is not
evidence for servo-record (prior-based) clocks, other oscillator types in real data, or real
GNSS-removal holdover runs.
