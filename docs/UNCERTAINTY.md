# Uncertainty and sensitivity at scale, over any scenario kind

`kshana-pro uncertainty` answers the two questions that follow every simulation result:
**how sure are we**, and **what drives it**.

The open Kshana engine gives one number for one scenario, or, with its
`campaign [monte_carlo]`, the scatter of that number over random seeds. A Kshana Pro
study goes one step further. You state how uncertain the *inputs* are. The study runs
the open engine over a seeded sample of those inputs, thousands of times if needed, and
reports for every output you watch:

- the mean, the standard deviation and the 5th, 50th and 95th percentiles, with a
  confidence interval on the mean;
- the **probability of meeting a limit**, given the stated input uncertainty, with its
  confidence interval;
- for a time series, the **band** between the 5th and the 95th percentile at every time
  step, and the median;
- a **ranking of the inputs** by how much of the output's variance each one explains;
- how each of those estimates settled as the runs accumulated.

A study can be stopped and resumed, and its result is the same file, byte for byte,
whatever the number of threads or interruptions.

The engine is used exactly as it is, through its public interface. Nothing here adds or
changes a model, so a figure is as trustworthy as the scenario kind it comes from and
never more: a MODELLED kind stays MODELLED however many runs are made.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| C/N0 | carrier-to-noise density ratio |
| dB | decibel |
| Eb/N0 | energy per bit to noise density ratio |
| EIRP | equivalent isotropically radiated power |
| G/T | receive gain-to-noise-temperature ratio (figure of merit) |
| GNSS | Global Navigation Satellite System |
| GPS | Global Positioning System |
| HTML | HyperText Markup Language |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| LHS | Latin hypercube sampling |
| PNT | positioning, navigation and timing |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SISRE | signal-in-space range error |
| SRC | standardised regression coefficient |
| SVG | Scalable Vector Graphics |
| TOML | Tom's Obvious Minimal Language (the scenario and study file format) |

## Quick start

```bash
# A study with a checkpoint: it can be stopped and resumed.
kshana-pro uncertainty scenarios/pro-power/uncertainty/uq-link-budget-margin.toml \
    --out out/uq-link --license <token>

# Stop after 500 new runs, continue later.
kshana-pro uncertainty study.toml --out out/study --max-runs 500 --license <token>
kshana-pro uncertainty study.toml --out out/study --resume --license <token>

# The same study in memory, with no checkpoint; prints the result document.
kshana-pro run study.toml --license <token>
```

A debug build also accepts `--dev` in place of `--license <token>`.

Files written to `--out`:

| File | Content |
|---|---|
| `uq.result.json` | every number, with its unit, its tier and the reproducibility stamp |
| `uq.bands.svg` | the 5th to 95th percentile band and the median of each series output, with the base run and the limit |
| `uq.sensitivity.svg` | ranked bars: the sensitivity of each output to each input, with confidence intervals |
| `uq.convergence.svg` | each estimate as the runs accumulated |
| `uq.report.html` | a self-contained page: the probability of meeting each limit, the tables behind the charts, what is not modelled, the reproducibility stamp |
| `runs.jsonl` | the checkpoint: one line per finished realisation |

## 1. The study file

A study is a TOML file with `kind = "pro-uncertainty"`. The five worked studies are in
`scenarios/pro-power/uncertainty/`.

```toml
kind = "pro-uncertainty"
title = "X-band link margin under stated input uncertainty"
seed = 20260930

base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[input]]
name = "eirp"
key = "eirp_dbw"
unit = "dBW"
dist = "normal"
mean = 55.0
std = 1.0
source = "assumed: 1 dB (one standard deviation) on the transmit chain, no unit measured"

[[output]]
name = "margin"
path = "margin_db"
limit = { op = "ge", value = 91.0 }

[sampling]
method = "lhs"
n = 8192

[sensitivity]
method = "sobol"
```

### Top level

| Field | Required | Meaning |
|---|---|---|
| `kind` | yes | `"pro-uncertainty"` |
| `title` | yes | printed on every output |
| `seed` | yes | fixes the sampling design and every bootstrap; the same seed gives the same study |
| `base_scenario` | yes | a complete open scenario of **any** kind, as TOML text; a `campaign` is allowed. It must run as written: it is run once, unchanged (the *base run*), before anything else |

A misspelled field name anywhere in the file is an error, never silently ignored.

### `[[input]]`: one uncertain input

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | letters, digits, `_` or `-`; unique |
| `key` | yes | dotted path into the base scenario. A segment that is a whole number indexes an array of tables: `fusion.system.1.sisre_m` is the second `[[fusion.system]]`. The key must already exist and hold a number; a mistyped key is refused, it never creates a field the engine would ignore |
| `dist` | yes | `normal`, `uniform`, `lognormal`, `triangular` or `discrete` |
| distribution parameters | yes | see below; a parameter the distribution does not take is refused |
| `source` | **yes** | where this uncertainty comes from: a datasheet, a measurement, a requirement. If you have none, write the word `assumed` (alone or in a sentence); the input is then printed as **ASSUMED** in the result, the summary, every chart and the report |
| `unit` | no | the input's unit, for display (the engine reports units of results, not of scenario keys) |

| `dist` | Parameters | Value at cumulative probability `u` |
|---|---|---|
| `normal` | `mean`, `std` (not negative) | `mean + std * z(u)`, `z` the inverse of the standard normal distribution (Wichura's algorithm AS 241) |
| `uniform` | `min`, `max` | `min + (max - min) * u` |
| `lognormal` | `median` (positive), `ln_std` (not negative): the standard deviation of the natural logarithm | `median * exp(ln_std * z(u))` |
| `triangular` | `min`, `mode`, `max` | the inverse of the triangular distribution |
| `discrete` | `values`, optional `weights` (equal when left out) | the value whose cumulative weight first exceeds `u`. A key that holds a whole number can only be varied this way, with whole-number values |

A spread of zero is allowed: the input is then constant, and its sensitivity is reported
as null with the reason, not as zero. A distribution that would give a value that is not a
finite number (a `uniform` or `triangular` range too wide to represent, a tail that
overflows) is refused before anything runs.

### `[[output]]`: one watched output

Either a scalar or a series. Output names are letters, digits, `_` or `-`.

| Field | Meaning |
|---|---|
| `name` | required, unique |
| `path` | a scalar result path, in the open campaign's grammar: dotted field names; `name[i]` takes element `i` of an array and counts from the end when `i` is negative, so `classical.series[-1].error_ns` is the last row |
| `series` and `time` | instead of `path`: two paths with one `[]` segment each, for example `classical.series[].error_ns` and `classical.series[].t`. The output is then a band over time |
| `limit` | optional `{ op, value }`, the value in the output's own unit. `op` is `lt`, `le`, `gt`, `ge`, or, for a symmetric guard, `abs_lt` or `abs_le` (the magnitude is below, or at most, the value) |
| `unit` | only for a path the base kind's own units block does not describe. Otherwise the unit is read from the base run; a declared `unit` that differs from it is refused, so a result never carries a unit the engine did not report |

### `[sampling]`

| Field | Default | Meaning |
|---|---|---|
| `method` | `lhs` | `lhs` (Latin hypercube sampling) or `random` |
| `n` | required, at least 2 | the size of the sample the statistics are taken over. One study makes at most 10,000,000 engine runs (`n`, or `n * (k + 2)` with `sobol`): every finished realisation is held in memory until the result is written |
| `vary_seed` | `false` | realisation `k` runs with the engine seed `base + k`, `base` being the base scenario's own seed: the open campaign's rule |
| `seed_key` | `seed` | the scenario key that holds the engine seed |
| `keep_samples` | `true` | keep each realisation's value of each scalar output in the result |

A study must vary something: at least one `[[input]]`, or `vary_seed = true`.

### `[sensitivity]`

| `method` | Cost in engine runs | What it gives |
|---|---|---|
| `none` (default) | `n` | no ranking |
| `src` | `n` (no extra runs) | standardised regression coefficients and the R-squared of the linear fit |
| `sobol` | `n * (k + 2)` for `k` inputs | first-order and total Sobol' indices with 95 % confidence intervals |

The base run is one more run in every case. For three inputs and `n = 8192`, `sobol`
costs 8192 × 5 = 40,960 runs.

### `[run]`

`threads`: worker threads. It is not part of the study's identity and cannot change a
result. The command-line `--threads` overrides it; with neither, the machine's
parallelism is used, at most eight.

## 2. How the sample is drawn

- **Latin hypercube sampling** (McKay, Beckman and Conover, 1979): each input's range is
  cut into `n` strata of equal probability; every stratum is used exactly once, in a
  random order that is independent from one input to the next. It covers each input's
  range evenly, which plain random sampling only does on average.
- **Plain random sampling**: independent uniform numbers.

Either way the sample is on the unit cube first and is then mapped through each input's
inverse distribution function. Each input has its own random stream, derived from the
study `seed`, so the design is a pure function of `(seed, method, n, number of inputs)`
and adding an input leaves the columns of the others unchanged.

**Correlated inputs** (roadmap F8a). By default inputs are independent. A
`[[correlation]]` table correlates two of them:

```toml
[[correlation]]
inputs = ["eirp", "losses"]
rho = 0.7
source = "assumed: amplifier drive and pointing loss share a thermal cause"
```

`rho` is the correlation of the two inputs' normal scores (a Gaussian copula): each
row of the unit design is turned into normal scores, multiplied by the Cholesky factor
of the correlation matrix, and turned back into the unit interval before each input's
inverse distribution function. Each input keeps its stated distribution exactly; for
two normal inputs `rho` is their ordinary correlation, and for other shapes the
ordinary correlation is a little smaller in magnitude. Every pair names a `source`, as
an input does; an assumed one is printed as ASSUMED in the summary. Correlations that
cannot hold together (a matrix that is not positive definite) are refused. With
correlations, the Latin hypercube's even coverage applies to the normal scores before
mixing, not to each input after it, and the sensitivity methods (`src`, `sobol`) are
refused, since both assume independent inputs; the spread, the limit probability and
the bands are computed as usual.

## 3. What is computed

### Per scalar output

Over the realisations that gave a value:

- the mean; the standard deviation `std` (divisor `n`, the open campaign's definition)
  and `std_sample` (divisor `n - 1`); the smallest and largest value;
- the 5th, 50th and 95th percentiles, by nearest rank;
- a 95 % confidence interval on the mean: percentile bootstrap, 2000 resamples, fixed
  seed;
- `convergence`: the same statistics over the first 16, 32, 64, ... realisations and over
  all of them;
- `nominal`: the output in the base run.

These are computed exactly as the open `campaign [monte_carlo]` computes them. See
section 6.

### Probability of meeting a limit

For an output with a `limit`, the share of realisations whose value meets it, reported
as the *probability of meeting the limit, given the stated input uncertainty*, with a
Wilson score 95 % interval (Wilson, 1927).

The share is taken over **all** realisations of the sample. A realisation whose engine
run failed, or that gave no value, counts as *not* meeting the limit.
`probability_among_evaluated` gives the share among the realisations that did give a
value, so the two readings are both on the page.

### Bands

For a series output: at every time step, the 5th, 50th and 95th percentile (nearest
rank) over the realisations, the base run's own series, and, with a `limit`, the share
of realisations meeting it at that step. The time axis is the base run's. A realisation
whose series has another length than the base run's does not contribute to the band.

### Sensitivity: Sobol' indices (`method = "sobol"`)

For an output `Y` and independent inputs `X_1 .. X_k`:

- the **first-order index** `S_i = Var(E[Y | X_i]) / Var(Y)` is the share of the output
  variance that would disappear if input `i` were known exactly;
- the **total index** `ST_i = E[Var(Y | all inputs but i)] / Var(Y)` is the share that
  remains as long as input `i` is unknown, its interactions with the other inputs
  included.

The design is Saltelli's: two independent samples `A` and `B` of `n` rows, and for each
input `i` the matrix `A_B(i)`, which is `A` with column `i` taken from `B`. That is
`n * (k + 2)` runs. The estimators are:

- first order, Saltelli and others (2010), Table 2:
  `V_i = (1/n) Σ f(B)_j (f(A_B(i))_j - f(A)_j)`;
- total, Jansen (1999): `VT_i = (1/2n) Σ (f(A)_j - f(A_B(i))_j)²`;

each divided by the variance of the `2n` values of `A` and `B` together, after centring
the outputs on their mean.

Each index carries a 95 % confidence interval: a percentile bootstrap over the rows of
the design, 1000 resamples, fixed seed. The inputs are **ranked** by total index.
`interaction_share` is one minus the sum of the first-order indices: the share of the
variance that no single input explains alone.

Estimates are not clamped. An input with no effect has indices that scatter around zero
and can read slightly below zero; its interval then contains zero.

The statistics of section 3 are taken over matrix `A` (the first `n` realisations).

With `vary_seed`, the engine seed behaves as one more column that always comes from `A`:
row `j` of `A` and of every `A_B(i)` runs with seed `base + j`, row `j` of `B` with
`base + n + j`. An index therefore never mixes in the noise of a changed seed, and the
seed's own run-to-run effect appears in `interaction_share`. The seed is not ranked.

### Sensitivity: standardised regression coefficients (`method = "src"`)

The output is regressed on the inputs by ordinary least squares, on the same `n`
realisations. For each input the result gives:

- `coefficient`: the slope, in output units per input unit;
- `src`: the slope times `std(input) / std(output)`, the standardised coefficient;
- `rank`: by magnitude of `src`.

and for the fit, `r_squared`. When the response is linear and the inputs independent,
`src²` equals the first-order Sobol' index and the squares sum to one.

**When R-squared is below 0.7 the ranking is not reliable, and the result says so in
words**, in `statement`, on the chart and in the report, and `reliable` is `false`. A
low R-squared means the output does not respond linearly to the inputs: an input that
changes the *spread* of an output rather than its level, for example, is invisible to a
regression. Use `sobol` then.

### When there is nothing to attribute

- An output that does not vary over the sample has no indices: every entry is `null`
  with `reason`, never `0`.
- An input that takes a single value in the sample has no index: `null` with `reason`.
  The other inputs keep theirs.

## 4. Stop and resume

`runs.jsonl` holds one JSON document per line.

Line 1, the header:

```json
{"engine_version":"0.29.0","kshana_pro_uncertainty_checkpoint":1,"overlay_version":"0.2.0","runs_total":2000,"study_hash":"a084b040…f5e570"}
```

Every further line, one finished realisation: its `index`, the `inputs` vector, the
engine `seed` (or null), the scalar `outputs`, the `series` values, the `result_sha256`
of the engine's result document, and `error` (null, or why the run failed).

- Lines are appended **in realisation order**, whatever the thread count.
- `--max-runs N` stops cleanly after `N` new runs. No result file is written until the
  last realisation is finished.
- `--resume` skips the realisations already in the file and appends the rest.
- The header carries the **study hash**: the SHA-256 of the canonical form of the parsed
  study (every default filled in, the base scenario as parsed so its comments and
  layout do not count, keys sorted, and without `[run]`). If the study changed since the
  file was started, `--resume` is refused and says so. Nothing is overwritten.
- A different engine version is refused too: runs of two engine versions are never
  mixed in one study.
- If the process died in the middle of a write, the last line is cut short. `--resume`
  detects it, drops it, truncates the file back to the last whole line and runs that
  realisation again, and says so. A line that does not parse anywhere else means the
  file was damaged or edited, and the resume is refused.
- Without `--resume`, an existing `runs.jsonl` is never overwritten.
- A new study does not start in a directory that holds result files (`uq.result.json`
  and the others) but no `runs.jsonl`: those files belong to another run, and a stopped
  run writes no result that would replace them.

A study that was stopped and resumed, any number of times, ends with the same six files
as an uninterrupted one, byte for byte. `tests/uncertainty.rs` checks exactly that.

## 5. The result document

`uq.result.json` carries the Pro result contract:

| Field | Content |
|---|---|
| `schema_version`, `kind`, `overlay`, `overlay_version`, `engine_version` | what wrote the document |
| `seed` | the study seed |
| `scenario_hash` | the study hash (section 4) |
| `label` | starts with `MODELLED`; names the ASSUMED inputs |
| `base` | the base scenario's kind, its own label, its scenario hash and the SHA-256 of the base run's result |
| `inputs[]` | each input: key, distribution, `source`, `source_class` (`ASSUMED` or `STATED`), its base value, and the mean, spread and range of the values actually drawn |
| `sampling`, `runs` | the design and the run counts; `runs.failed` and `runs.failures[]` |
| `outputs.<name>` | section 3, per scalar output |
| `bands.<name>` | section 3, per series output |
| `sensitivity` | the method, its cost, and per scalar output the indices or coefficients |
| `units` | for **every** numeric field: `{unit, provenance, note}`, keyed by result path in the open engine's own path grammar. Each output's unit is read from the base run's own units block |
| `figure_tiers.figures[]` | per output: `base_tier`, the tier the open engine grants the figure for one run, and `tier`, the tier the statistic carries. `tier` is never above `MODELLED` |
| `not_modelled[]` | what the study leaves out |
| `reproducibility` | the run count and `run_digest`: the SHA-256 over every realisation's result hash, in order |

### Failed runs

A realisation whose engine run fails (an input that reaches a value the engine refuses)
is recorded with its error in `runs.jsonl`, counted in `runs.failed`, and listed in
`runs.failures[]`. It is never dropped and never counted as meeting a limit. Every
statistic is over the successful realisations only, and the summary line and the report
say so whenever a run failed. In a Sobol' design a failed run removes its whole row
from the index estimate; `n_rows` says how many rows were used. If every realisation fails, the
study still finishes: each statistic, coefficient and index is null with its reason, and
the probability of meeting a limit is 0.

## 6. How this relates to `campaign [monte_carlo]` and to `pro-batch`

| | Varies | Reports |
|---|---|---|
| open `campaign [monte_carlo]` | the engine seed | per metric: mean, standard deviation, 5th, 50th and 95th percentiles, bootstrap interval on the mean |
| `pro-batch` | the engine seed, in parallel | one metric: mean, standard deviation, smallest, largest, 5th and 95th percentiles |
| `pro-uncertainty` | the engine seed **and the inputs**, in parallel, resumable | all of the above for any number of outputs, plus the probability of meeting a limit, bands over time, convergence, and the ranking of the inputs |

**The same seeds give the same samples.** A study with no `[[input]]` and
`vary_seed = true` runs exactly the scenarios the open campaign runs: realisation `k`
with seed `base + k`. With the study `seed` set to the campaign's `seed`, the bootstrap
stream is the campaign's too. `uq-clock-holdover-seeds.toml` is that study for the open
scenario `campaign-monte-carlo-clock-holdover`, and `tests/uncertainty.rs` runs both and
requires the 200 samples, the mean, the standard deviation, the three percentiles and
the bootstrap interval to be equal bit for bit. Free and Pro never disagree on a number;
Pro starts where the campaign stops.

`pro-batch` is unchanged.

## 7. Worked examples

All numbers below are from runs of the study files in
`scenarios/pro-power/uncertainty/` on open engine 0.28.0 (the 75-kind build), made with
a debug build of Kshana Pro 0.1.0, which embeds the engine optimised. For Kshana Pro
0.2.0 on open engine 0.29.0 the X-band link margin (8192 × 5 runs), the clock holdover
with an uncertain `q_wf` (2000 runs, also stopped at 700 and resumed: the same six files)
and the low-Earth-orbit chain (6144 runs) were run again and gave the same figures at the
precision stated here; their run digests changed, because every result document states
the engine version. The base
scenarios are the open engine's own public example files, copied unchanged into
`scenarios/pro-power/uncertainty/engine/` with their provenance.

### 7.1 A link margin, against its closed form

`uq-link-budget-margin.toml`: the open `link-budget` example (X band, 2000 km, 1 Mbit/s)
with the EIRP uncertain by 1.0 dB, the G/T by 0.5 dB and the lumped losses by 0.5 dB
(normal, one standard deviation; all three ASSUMED). The margin is linear in the three,
so everything has a closed form.

| Quantity | Closed form | Study (`n` = 8192, 40,960 runs) |
|---|---|---|
| mean margin | 92.1245 dB (the base run) | 92.1245 dB |
| standard deviation | √1.5 = 1.2247 dB | 1.2249 dB |
| probability that the margin is at least 91 dB | 0.8207 | 0.8198 (95 % interval 0.8114 to 0.8280) |
| first-order index: EIRP, G/T, losses | 2/3, 1/6, 1/6 | 0.673, 0.162, 0.163 |
| total index: EIRP, G/T, losses | 2/3, 1/6, 1/6 | 0.669, 0.162, 0.167 |
| share left to interactions | 0 | 0.0016 |

With `method = "src"` on the 8192 plain runs: slopes +1, +1 and −1 dB per dB and
R-squared 1, to rounding.

Study hash `57847df040e032373f9bbedf306c6f6c383075b2d91d9e87386e049591b3d6e1`, seed
20260930.

### 7.2 The open campaign, reproduced, then taken further

`uq-clock-holdover-seeds.toml`: a chip-scale atomic clock disciplined to GNSS for 600 s,
then coasting to 3600 s, over 200 engine seeds. No input is varied.

| Final time error | Open `campaign [monte_carlo]` | Pro study |
|---|---|---|
| mean | −0.4536 ns | −0.4536 ns |
| standard deviation | 14.9265 ns | 14.9265 ns |
| 5th, 50th, 95th percentile | −24.947, 0.858, 24.449 ns | −24.947, 0.858, 24.449 ns |
| 95 % interval on the mean | −2.518 to 1.590 ns | −2.518 to 1.590 ns |

(Equal bit for bit, not only to the digits shown. The closed form for the standard
deviation is √(q_wf × 3010 s) = 16.459 ns; 14.93 ns is the sample value over these 200
seeds.)

From the same 200 runs the study adds: the band of the whole time-error curve (5th to
95th percentile −24.95 to 24.45 ns at 3600 s); the probability of ending inside the
scenario's 20 ns guard, 0.830 (95 % interval 0.772 to 0.876, 166 of 200); and the
running estimate at 16, 32, 64, 128 and 200 runs.

Study hash `bf7e0f4daf6e8ec99c935cf6d5344f83dc9187c21f76f67459bde205b8409bdc`, seed
20260928 (the campaign's).

### 7.3 The same clock with an uncertain noise level

`uq-clock-holdover-qwf.toml`: the clock's white-frequency-noise intensity `q_wf` is no
longer the datasheet value 9e-20 s²/s but uniform from half to three times that value
(ASSUMED), over 2000 runs with the engine seed varied.

Given `q_wf`, the final error is normal with variance `q_wf × 3010 s`, so over the
mixture its variance is `mean(q_wf) × 3010 s` and its standard deviation
√(1.575e-19 × 3010) s = 21.77 ns.

| Final time error | Fixed `q_wf` (7.2) | Uncertain `q_wf` |
|---|---|---|
| standard deviation, closed form | 16.46 ns | 21.77 ns |
| standard deviation, study | 14.93 ns (200 runs) | 21.47 ns (2000 runs) |
| 5th to 95th percentile | −24.9 to 24.4 ns | −36.0 to 35.9 ns |
| probability of ending inside the 20 ns guard | 0.830 | 0.683 (95 % interval 0.662 to 0.703) |

`uq.bands.svg` draws the time-error curve the open engine draws as one line, as the band
it really is, against the ±20 ns guard.

This study also shows the regression's blind spot. `q_wf` changes the *spread* of the
final error, not its level, so the linear fit explains nothing (R-squared 0.0004) and
the result states that the ranking is NOT reliable rather than printing a coefficient as
if it meant something.

Study hash `a084b04062aca73095920d198ef1667d1ba7ca096742997ffbd9320986f5e570`, seed
20260930.

### 7.4 A navigation chain in low Earth orbit

`uq-leo-pnt-chain.toml`: the open `leo-pnt-end-to-end` example (a generic 1080 km
constellation: signal, pass link budget, navigation message, fused positioning), with
the orbit-and-clock error of the broadcast message uniform from 0.10 m to 0.40 m and the
transmit EIRP normal with a standard deviation of 1 dB (both ASSUMED), 64 runs,
`method = "src"`.

Direct runs of the open engine show that the peak C/N0 moves exactly 1 dB per dB of
EIRP and not at all with the orbit-and-clock error, and that the broadcast SISRE follows
the orbit-and-clock error alone. The study finds the same:

| Output | Slope against EIRP | Slope against the orbit-and-clock error | R-squared |
|---|---|---|---|
| peak C/N0 | 1 dB-Hz per dB | 0 | 1 |
| broadcast SISRE | 0 | 1 m per m | 1 |

`uq-leo-pnt-chain-sobol.toml` adds the SISRE of GPS and of Galileo as inputs (uniform,
0.4 m to 0.8 m and 0.2 m to 0.4 m, both ASSUMED) and asks which of the four inputs drives
the position error, with and without the LEO layer, using Sobol' indices over
1024 × (4 + 2) = 6144 runs of the whole chain:

| Output | Mean | 5th to 95th percentile | Input | First-order index (95 % interval) | Total index (95 % interval) |
|---|---|---|---|---|---|
| fused position error (GPS, Galileo and LEO) | 0.431 m | 0.247 m to 0.604 m | orbit-and-clock error | 0.988 (0.919 to 1.059) | 0.989 (0.932 to 1.049) |
| | | | EIRP | 0.001 (−0.003 to 0.005) | 0.002 (0.002 to 0.002) |
| | | | SISRE of Galileo | 0.001 (−0.001 to 0.002) | 0.000 |
| | | | SISRE of GPS | 0.000 (−0.001 to 0.001) | 0.000 |
| position error of GPS and Galileo alone | 1.604 m | 1.539 m to 1.674 m | SISRE of GPS | 0.713 (0.648 to 0.776) | 0.679 (0.635 to 0.728) |
| | | | SISRE of Galileo | 0.330 (0.288 to 0.376) | 0.320 (0.295 to 0.346) |
| | | | orbit-and-clock error, EIRP | 0 | 0 |
| peak C/N0 | 60.73 dB-Hz | 59.09 to 62.37 dB-Hz | EIRP | 1.007 (0.915 to 1.091) | 1.006 (0.947 to 1.066) |
| | | | the other three | 0 | 0 |

Read across the rows: without the LEO layer the position error is set by the range errors
of GPS (about two thirds of its variance) and Galileo (about one third). With the LEO
layer the fused error is set almost entirely by the LEO orbit-and-clock error, and the
range errors of GPS and Galileo no longer matter to it. The probability that the fused
error stays at or below 0.6 m is 0.937 (95 % interval 0.920 to 0.950, 959 of 1024),
given the stated input uncertainty. An index estimate is not clamped, so the index of a
sole driver scatters around 1 and its interval straddles it.

Study hash `a96a8850a373f4e6fbb2eae8258a7f10a5e929352d32841525ee98518082ac65`, seed
20260930. This run was stopped after 2000 realisations and resumed.

The 0.6 m limit on the fused error in these two studies is an example requirement, not a
value from any specification.

## 8. How the estimators are checked

`tests/uncertainty.rs` holds every estimator to a value that does not come from the
code under test:

| Check | Reference |
|---|---|
| Sobol' first-order and total indices | the Ishigami function's closed form (a = 7, b = 0.1): 0.3139, 0.4424, 0 and 0.5576, 0.4424, 0.2437, each within 0.02 and inside its own interval at `n` = 65,536 (327,680 evaluations); and over twelve seeds at `n` = 4096, the closed-form value lies inside the reported 95 % interval in at least 65 of the 72 cases (72 were measured) |
| mean, standard deviation, probability, indices, coefficients | the link margin's closed form (7.1), with tolerances derived from `n` |
| samples, statistics, bootstrap interval | the open engine's own `campaign [monte_carlo]`, bit for bit (7.2) |
| spread under input uncertainty, band | the mixture's closed form (7.3); the band at the last step equals the scalar output's percentiles |
| a new scenario kind | direct runs of the open engine (7.4) |
| inverse normal distribution | published quantiles, and an independent numerical integral |
| Wilson interval | Newcombe (1998), 81 of 263: 0.2553 to 0.3662 |

## Licence

`kshana-pro uncertainty` and the `pro-uncertainty` kind need the Enterprise tier, or the
`pro-uncertainty` feature named on the licence. With no licence both fail closed, before
anything runs. For access, write to contact@ashforde.org.

## Limitations

- **Correlation is pairwise and Gaussian.** `[[correlation]]` sets the correlation of
  normal scores between pairs of inputs (a Gaussian copula). Tail dependence beyond a
  Gaussian copula, and correlations that change with the operating point, are not
  modelled. A pair with no `[[correlation]]` is independent.
- **No sensitivity indices for correlated inputs.** The regression ranking and the
  Sobol' indices assume independent inputs, so a study with correlations is refused
  unless `sensitivity.method` is `none`.
- **The result is conditional on the stated distributions.** The study cannot check that
  an input really is normal with that standard deviation. An input whose `source` is
  `assumed` is printed as ASSUMED everywhere, so that a reader cannot miss it.
- **A MODELLED kind stays MODELLED however many runs are made.** More runs narrow the
  sampling error of a statistic. They do not change how well the base scenario kind
  matches reality. `figure_tiers` keeps the base kind's tier beside every output and
  never raises it.
- **Sensitivity is for scalar outputs.** A series output gets a band, not indices. To
  rank the drivers of a curve, watch a scalar taken from it (its last value, its root
  mean square).
- **Five distribution shapes**: normal, uniform, lognormal, triangular, discrete.
- **The engine seed is not ranked.** With `vary_seed`, its effect is part of the variance
  that no listed input explains.
- **The regression ranking needs a near-linear response.** Below R-squared 0.7 it is
  flagged as not reliable; use `sobol`.
- **Bootstrap intervals on Sobol' indices resample design rows as if they were
  independent.** For a Latin hypercube they are slightly negatively dependent, so the
  intervals tend to be a little wide rather than narrow. Over twelve seeds of the
  Ishigami check at `n` = 4096 they contained the closed-form value in 72 of 72 cases (70 of 72 with other bootstrap
  streams). A
  95 % interval still misses one time in twenty: with the study seed 20260930, one of the
  six Ishigami intervals missed its closed-form value at `n` = 16,384 and another at
  32,768, and all six contained it at 65,536.
- **Numbers for a customer or a publication come from a release build.** A development
  build embeds the engine optimised but differs from a release build in the last bits
  of some floating-point results (see `Cargo.toml`).

## References

- M. D. McKay, R. J. Beckman and W. J. Conover, "A Comparison of Three Methods for
  Selecting Values of Input Variables in the Analysis of Output from a Computer Code",
  Technometrics 21(2), 1979, pp. 239-245. (Latin hypercube sampling.)
- A. Saltelli, P. Annoni, I. Azzini, F. Campolongo, M. Ratto and S. Tarantola, "Variance
  based sensitivity analysis of model output. Design and estimator for the total
  sensitivity index", Computer Physics Communications 181(2), 2010, pp. 259-270. (The
  design and the first-order estimator.)
- M. J. W. Jansen, "Analysis of variance designs for model output", Computer Physics
  Communications 117(1-2), 1999, pp. 35-43. (The total-order estimator.)
- I. M. Sobol', "Global sensitivity indices for nonlinear mathematical models and their
  Monte Carlo estimates", Mathematics and Computers in Simulation 55(1-3), 2001,
  pp. 271-280. (The indices.)
- T. Ishigami and T. Homma, "An importance quantification technique in uncertainty
  analysis for computer models", Proceedings of the First International Symposium on
  Uncertainty Modeling and Analysis, 1990, pp. 398-403. (The test function.)
- A. Saltelli and others, "Global Sensitivity Analysis: The Primer", Wiley, 2008.
  (Standardised regression coefficients and their limits.)
- M. J. Wichura, "Algorithm AS 241: The Percentage Points of the Normal Distribution",
  Applied Statistics 37(3), 1988, pp. 477-484.
- E. B. Wilson, "Probable Inference, the Law of Succession, and Statistical Inference",
  Journal of the American Statistical Association 22(158), 1927, pp. 209-212.
- R. G. Newcombe, "Two-sided confidence intervals for the single proportion: comparison
  of seven methods", Statistics in Medicine 17(8), 1998, pp. 857-872.
