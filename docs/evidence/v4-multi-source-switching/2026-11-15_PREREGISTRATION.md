# V4 pre-registration: multi-source switching bars (roadmap 0.42 F3)

Written and committed on **2026-11-15 (11:20 UTC)**, alone, before the multi-source trust selector
(roadmap 0.42 F3) is written and before any scoring code runs. It fixes the selector's rule, the
data, the injected faults, the metrics and the pass thresholds. A bar may be tightened after the
run, never loosened. A miss is published with its numbers (in `RESULTS.md` next to this file), not
tuned away. F3 ("choose which source to trust second by second, with evidence") is called
**claimed** only if every bar below passes on every data set.

## 0. What the data are, plainly

No public recording exists, to our knowledge, of one site receiving GNSS, OSNMA-authenticated
Galileo, a LEO PNT service, eLoran and PTP at the same time, with labelled faults. V4 therefore
scores on **recorded local clocks** (real oscillators, public IGS products) carrying **modelled
source measurements** (white noise at each source's stated level) and **injected faults**
(stated below, seeded). What is real: the local clock's behaviour, which drives every holdover
decision and every holdover error. What is modelled: each source's noise and each fault. A
synthetic set (D-S) repeats the test with modelled clocks on a one-second grid. A pass supports
the sentence "on recorded clocks with modelled source errors and injected faults, the selector
met the pre-registered switching bars"; it says nothing about any real receiver, network or
service. A real multi-source recording (partner data) remains open.

## 1. Sources and the measurement model

The local clock's true offset from the reference time scale is `x(t)`. Each external source `s`
gives, at each epoch where it is available, a measured offset

```
o_s(t) = x(t) + n_s(t) + f_s(t)
```

with `n_s` white normal noise of standard deviation `sigma_s` (the source's stated level, which
the selector is also told) and `f_s` the injected fault (zero outside a fault). Sources, groups
(sources in one group share a signal path and are never independent of each other) and levels:

| Source | Group | `sigma_s` | Status values |
|---|---|---|---|
| `osnma` (Galileo time, navigation data authenticated by OSNMA) | `gnss` | 15 ns | `ok`, `failed`, `unavailable` |
| `gnss` (open-signal GNSS time, not authenticated) | `gnss` | 15 ns | `ok`, `unavailable` |
| `leo` (a LEO PNT time service) | `leo` | 40 ns | `ok`, `unavailable` |
| `ptp` (PTP from a remote grandmaster over the network) | `ptp` | 50 ns | `ok`, `unavailable` |
| `eloran` | `eloran` | 60 ns | `ok`, `unavailable` |
| `holdover` (the local clock extrapolated, internal) | `holdover` | section 2.3 | always present once anchored |

## 2. The selector (fixed now; the code must implement exactly this)

Epoch grid `tau0` (the data's interval). Constants: `K = 4` (consistency and protection
multiplier), `W = 3600 s` (holdover fit window), `G = 600 s` (holdover guard), `S_H = 200 ns`
(largest holdover sigma that votes), `T_up = max(2 tau0, 10 s)` (hold time before switching to a
better source), `T_osnma = 60 s` (OSNMA recovery time).

### 2.1 Exclusion

At each epoch a source is **excluded** when its status is not `ok`. In addition, when `osnma`
reports `failed`, the whole `gnss` group (`osnma` and `gnss`) is excluded from that epoch until
`osnma` has reported `ok` continuously for `T_osnma`.

### 2.2 Qualification

For each external candidate `c` not excluded: each **other** group with at least one usable
source votes through its representative (its non-excluded source of smallest `sigma`, ties by the
order `osnma, gnss, leo, ptp, eloran`); the `holdover` group votes when it is anchored and
`sigma_h <= S_H`. A vote **agrees** when `|o_c - o_r| <= K sqrt(sigma_c^2 + sigma_r^2)` and
**disagrees** otherwise. `c` **qualifies** when `agree >= 1` and `agree >= disagree`. Its
protection level is `PL_c = K sqrt(sigma_c^2 + sigma_ref^2)` where `sigma_ref` is the smallest
`sigma` among the agreeing representatives.

### 2.3 Holdover

The holdover fit uses the epochs at which the selector's output was an external source, with time
`t <= now - G`, the most recent `W` seconds of them (all such epochs in `[t_e - W, t_e]`, `t_e` the
latest). Weighted equally, a least-squares line `a + b (t - t_e)` through the outputs; `v` the mean
of the stated output variances in the window; `var_a = v (1/N + (t_e - tbar)^2 / Sxx)`,
`var_b = v / Sxx`. With at least 3 epochs and `Sxx > 0` the holdover is **anchored**; its output
at `now` (`u = now - t_e`) is `a + b u`, with

```
sigma_h(u)^2 = var_a + var_b u^2 + q_wf u + q_rw u^3 / 3 + (D_c (u + W/2)^2 / 2)^2
```

`q_wf`, `q_rw` the local clock's class levels from the open engine (`ClockClass::psds`, random-run
term zero; the class is configured, never fitted on the scored data). `D_c` is the class's drift
bound (the error a line fitted over `W` makes when extrapolated over a quadratic drift is at most
`D (u + W/2)^2 / 2`): `tcxo` 1e-14/s, `ocxo` 1e-15/s, `csac` 1e-15/s, `rafs` 1e-16/s, `uso`
1e-15/s, `dsac` 1e-17/s. Its protection level is
`PL_h = K sigma_h`.

### 2.4 Choice and hysteresis

Ranking of qualified candidates: smallest `sigma_s`, ties by the order `osnma, gnss, leo, ptp,
eloran`. The current choice is kept while it qualifies and no better-ranked candidate has
qualified continuously for `T_up`. If the current choice stops qualifying (or is excluded), the
selector switches **at once** to the best-ranked qualified candidate; with none, to `holdover` if
anchored; with no anchor, the output is `none` (no time output). From `holdover` or `none` it
switches at once to the best-ranked qualified candidate (no hold).

Output per epoch: the chosen source, its offset `ô`, its protection level `PL`, the state
(`trusted` for an external source, `holdover`, `none`), and the evidence (every source's status:
excluded with the reason, qualified with its votes, or not qualified with its votes).

## 3. Data

### D-R1: recorded clocks (public)

`tests/fixtures/timing_v5_igs/clocks.txt` (SHA-256
`d813620393b4b4905aaedc4eaff8a5f2aad29a2a356b54cb2f06ad9810055de3`): IGS final combined clocks of
the eleven GPS Block IIF satellites (rubidium), 2025-09-01 to 2025-09-14, 300 s. Each clock is the
local clock `x(t)` of one run, class `rafs`. A clock with a gap is split at the gap into separate
runs (state reset); a run shorter than one day is dropped and counted.

### D-R2: recorded clocks (public)

`tests/fixtures/timing_v5b_igs/clocks.txt` (SHA-256
`3da6d0a5a7facdc1e564fdeed5f5ada890a2f9cdab6a218fa79a83608a3230a4`): the same eleven clocks,
2025-08-17 to 2025-08-30, 300 s; same rules.

Honest note: V5 and V5b used these files to score a holdover predictor; no multi-source selector
has been run on them.

### D-S: synthetic (SYNTHETIC; not a measurement)

Ten local clocks generated inside the scoring test with the crate's own generator
(`timing::model::synth_clock`, seed **20261117**): five `ocxo` and five `rafs` (alternating,
`ocxo` first), each class's white-FM and random-walk-FM levels, a drift uniform in +-1e-15/s, an
initial frequency offset uniform in +-1e-10, no phase noise; one day each at `tau0 = 1 s`.

### Source noise and faults (all data sets)

Generated per run with seed `20261115 + 1000 x data-set number (R1 = 1, R2 = 2, S = 3) + run
index`, the crate's generator, in this order: noise for every source at every epoch (`osnma`,
`gnss`, `leo`, `ptp`, `eloran`), then the LEO pass phase, then the faults type by type in the
table's order. `osnma` and `gnss` noise are independent draws.

* **LEO availability:** passes of 600 s every 1800 s, phase uniform in [0, 1800 s).
* **Faults:** for each type, the number in a run is `floor(rate x days + U)`, `U` uniform in
  [0, 1), `days` the run length; each start uniform in `[0, T - duration]`. Faults of different
  types may overlap.

| Type | Rate per day | Duration | Effect |
|---|---|---|---|
| S1 generated-signal spoof | 1 | 1800 s | `gnss` ramp `f = 2 ns/s x (t - t0)`; `osnma` status `failed` throughout |
| S2 re-broadcast time push | 1 | 1800 s | the same ramp on `gnss` and `osnma`; `osnma` stays `ok` |
| J jamming | 2 | 1200 s | `gnss` and `osnma` `unavailable` |
| P PTP asymmetry | 1 | 3600 s | `ptp` step `+500 ns` |
| L LEO time error | 1 | 3600 s | `leo` step `+300 ns` |
| E eLoran outage | 1 | 3600 s | `eloran` `unavailable` |
| A all external lost | 0.5 | 1800 s | every external source `unavailable` |

A source is **faulty** at an epoch when an S1, S2, P or L fault acts on it (truth label).

## 4. Metrics and bars (each data set scored separately)

Time error `TE = ô - x`. An epoch with output `none` has no `TE`.

| Bar | Metric | Pass |
|---|---|---|
| B1 integrity | Misleading epochs (`|TE| > PL`) as a share of epochs with an output | `<= 0.1 %` |
| B2 deselection | Among fault episodes with an **assessable** epoch (below), the share in which the faulty source is never the output at an assessable epoch | `>= 95 %` |
| B3a accuracy | p95 of `|TE|` over epochs with an output, divided by the oracle's p95 | `<= 1.25` |
| B3b against GNSS-only | p99.9 of `|TE|`: selector below the GNSS-only baseline | strictly below |
| B4 stability | Number of output changes, summed over runs | `<= 2 x transitions + 2 x runs` |
| B5 availability | Among epochs where at least two groups have an available, non-faulty source, the share with an external output | `>= 99 %` |
| B6 evidence | Two runs byte-identical; the evidence chain verifies; every epoch has a record; a one-byte change in the decisions breaks verification | all four |

* **Assessable epoch (B2):** an epoch of the episode at least 3 epochs after the first epoch at
  which the faulty source's true fault error `|f_s|` exceeds `5 sqrt(sigma_s^2 + sigma_r^2)`,
  `sigma_r` the smallest `sigma` among the available, non-faulty sources of other external
  groups at that epoch (an epoch with none is not assessable).
* **Oracle (B3a):** at each epoch the available, non-faulty external source of smallest `sigma`
  (same tie order), else the holdover of section 2.3 fitted on the oracle's own external outputs,
  else no output.
* **GNSS-only (B3b):** `gnss` whenever available, else the holdover of section 2.3 fitted on its
  own external outputs, else no output; it uses no exclusion and no cross-check.
* **Transitions (B4):** the epochs at which any source's availability, status or faulty label
  changes, counted once per epoch.

Assessability: B2 needs at least 30 episodes with an assessable epoch per data set; with fewer it
is "not assessable" and counts as **not passed**.

**V4 passes** only if B1 to B6 pass on **D-R1, D-R2 and D-S**.

## 5. What will be published

`RESULTS.md` beside this file: the code commit scored, every bar's number and verdict per data
set, the counts (runs, dropped runs, epochs, episodes), and the per-type deselection figures. The
test that produces them (`tests/multi_source_v4.rs`) prints the same table and pins the numbers.
The result is published whatever it is.
