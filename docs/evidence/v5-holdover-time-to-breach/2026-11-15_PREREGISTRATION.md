# V5 pre-registration: holdover time-to-breach prediction bars on recorded clocks

Written and committed on **2026-11-15 (08:50 UTC)**, alone, before the predictor (roadmap 0.42
T1) is written and before any scoring code runs. It fixes what is predicted, how, the metrics, the
pass thresholds and the data. A bar may be tightened after the run, never loosened. A miss is
published with its numbers (in `RESULTS.md` next to this file), not tuned away. T1
("holdover and time-to-breach for every clock in a fleet") is called **claimed** only if every
bar below passes on both data sets.

## 1. What is predicted

A clock enters holdover at time `t_h`: from then on its time is the linear extrapolation of the
state it had at `t_h` (phase and frequency, as a frozen servo keeps them). The **holdover time
error** after `u` seconds is

```
TE(u) = x(t_h + u) - (a + b u)
```

where `x` is the clock's recorded phase against a better reference and `a`, `b` are the phase
and frequency the predictor estimated at `t_h`. The predictor gives, from data before `t_h`
only, a normal distribution `TE(u) ~ N(m(u), s(u)^2)` for every `u` up to the horizon `H`, and
from it, for a breach limit `L` (user-set: e.g. 100 us, 1.1 us, 1 ns),

* `P(u) = P(|TE(u)| >= L)`,
* **T_warn(L)**: the first grid time `u` with `P(u) >= 0.05` (the warning time),
* **T_50(L)**: the first grid time `u` with `P(u) >= 0.5` (the median time-to-breach).

Either is "beyond the horizon" when the condition never holds up to `H`.

## 2. The predictor (fixed now; the code must implement exactly this)

Training: the `N` samples of the window `[t_h - W, t_h]`, `W = 48 h`, sample interval `tau0`.

1. Least-squares fit `x(t) = a + b v + c v^2 / 2`, `v = t - t_h`, over the window. Residuals
   `r`; residual variance `sr2 = sum(r^2) / (N - 3)`; parameter covariance `C = sr2 (A^T A)^-1`.
2. Overlapping Allan variance of the residuals at `m = 1, 2, 4, ...` (powers of two) while
   `m <= N / 8`, `tau = m tau0`. Fit `sigma^2(tau) = 3 R / tau^2 + q_wf / tau + q_rw tau / 3`
   with `R, q_wf, q_rw >= 0` by least squares on relative residuals
   (`sum(((model - sigma^2) / sigma^2)^2)`), exhaustively over the non-negative subsets.
3. Prediction for `u > 0`:
   * `m(u) = c u^2 / 2`
   * `s(u)^2 = C_aa + (C_bb + q_wf / W + q_rw W / 3) u^2 + C_cc u^4 / 4 + q_wf u + q_rw u^3 / 3 + R`
   (`C_aa`, `C_bb`, `C_cc` the diagonal of `C`; cross terms left out).
4. Probabilities from the normal distribution; times on the grid `u = k tau0`, `k = 1 .. H / tau0`.

No parameter of the predictor is tuned on either data set. The clock-noise values printed in the
real data file's headers (fitted by others on the first half) are **not** used.

## 3. Data

### D-R: real recorded clocks (public)

`tests/fixtures/timing_v5_igs/clocks.txt` (SHA-256
`d813620393b4b4905aaedc4eaff8a5f2aad29a2a356b54cb2f06ad9810055de3`): International GNSS Service
(IGS) final combined satellite clocks, 2025-09-01 to 2025-09-14, the eleven GPS Block IIF
satellites G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32 (rubidium clocks), decimated to 300 s,
as cut by the open engine's test suite (same file, same hash). Public, IGS terms (open use with
attribution). These are **free-running clocks; no GNSS removal happened**: the holdover is
**emulated** by freezing the estimated state at `t_h`, which is how a frozen servo behaves on
the same oscillator. Honest note: this file has been used by other open-engine tests (clock
filter tuning); the holdover predictor has never been run on it.

* Handover times `t_h` = 7.00 d, 7.25 d, ..., 11.75 d from the first epoch (20 per clock).
* `W = 48 h`, `H = 48 h`, grid `tau0 = 300 s`.
* Coverage horizons: 1, 2, 4, 8, 12, 24, 48 h.
* Limits `L`: 1 ns, 3 ns, 10 ns. (The regulatory-scale limits, 100 us or 1.1 us, are never reached
  by these clocks in 48 h, so they would score nothing; they are used on D-S.)
* A run whose window or horizon includes a missing epoch is excluded and counted (G24 has one
  gap of three epochs).

### D-S: synthetic mixed-class fleet (labelled SYNTHETIC; not a measurement)

Generated inside the scoring test with a fixed seed (`20261115`, the crate's own generator) and
written nowhere else: twenty clocks, five each of the open engine's classes TCXO, OCXO, RAFS and
CSAC, using each class's white-FM and random-walk-FM levels (`ClockClass::psds`, random-run term
set to zero), a deterministic drift drawn uniformly in +-1e-14/s (TCXO), +-1e-15/s (OCXO, CSAC),
+-1e-16/s (RAFS), an initial frequency offset drawn in +-1e-9, and white phase noise of 1 ns
(a counter). `tau0 = 10 s`, four days per clock.

* Handover times `t_h` = 48 h, 54 h, 60 h, 66 h, 72 h (5 per clock).
* `W = 48 h`, `H = 24 h`, grid `tau0 = 10 s`.
* Coverage horizons: 1, 2, 4, 8, 12, 24 h.
* Limits `L`: 100 ns, 1.1 us, 100 us (user-set values in the style of PRTC-class and RTS 25
  limits; using them asserts nothing about any standard).

## 4. Metrics and bars (each data set scored separately)

Actual breach time `T_act(L)`: the first grid `u <= H` with `|TE(u)| >= L`; "no breach" if none.
A *pair* is one (run, `L`).

| Bar | Metric | Pass |
|---|---|---|
| B1 | Coverage of the 95 % interval: share of (run, coverage horizon) points with `|TE(u) - m(u)| <= 1.96 s(u)` | `0.90 <= coverage <= 0.99` |
| B2 | Early warning: among pairs with a breach, share with `T_warn <= T_act` (no warning within `H` counts as late) | `>= 0.90` |
| B3 | Not uselessly early: median over pairs with a breach of `T_warn / T_act` (no warning: `H / T_act`) | `>= 0.20` |
| B4 | Median accuracy: among pairs with a breach, share with `0.5 <= T_50 / T_act <= 2` (`T_50` beyond `H` taken as `H`) | `>= 0.60` |
| B5 | False alarms: among pairs with no breach, share with `T_50 <= H` | `<= 0.25` |

Assessability: B2 to B4 need at least 30 pairs with a breach and B5 at least 30 pairs without;
a bar with fewer is "not assessable". A not-assessable B2, B3 or B4 counts as **not passed**
(T1 cannot be claimed on too little data). A not-assessable B5 is reported and does not block.

**V5 passes** only if B1 to B4 pass, and B5 passes or is not assessable, on **D-R and D-S both**.

## 5. What will be published

`RESULTS.md` beside this file: the code commit scored, every bar's number and verdict per data
set, the counts (runs, exclusions, pairs), and per-clock figures for D-R; the test that produces
them (`tests/timing_v5.rs`) prints the same table. The result is published whatever it is.
