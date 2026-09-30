# Design optimiser: the Pareto front of a design space

Open Kshana scores one scenario that you wrote by hand. `kshana-pro design` answers the
next question, "which design should we fly": you state a design space, constraints and two
or three objectives, and it returns the Pareto front (the designs that no other design
beats on every objective at once), the knee of that front, and where a reference design
sits against it. Every design is one run of the open engine, and every design on the front
is written out as a plain scenario file that the free engine runs to the same numbers.

It works on any open scenario kind and around any central body the open engine knows: the
worked studies cover a constellation around the Moon, one around the Earth, one around Mars
and a link budget.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| CSV | comma-separated values |
| DOP | dilution of precision: how much the satellite geometry multiplies a ranging error |
| EIRP | equivalent isotropic radiated power |
| GPS | Global Positioning System |
| HDOP | horizontal dilution of precision |
| HTML | HyperText Markup Language |
| J2 | the second zonal harmonic of a body's gravity field (its oblateness) |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| MBSE | model-based systems engineering |
| NSGA-II | non-dominated sorting genetic algorithm II (Deb, Pratap, Agarwal and Meyarivan, 2002) |
| PDOP | position dilution of precision |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SVG | Scalable Vector Graphics |
| TOML | Tom's Obvious Minimal Language (the scenario and study file format) |
| ZDT1 | the first Zitzler-Deb-Thiele test problem, a search benchmark with a known front |

## Quick start

```bash
# Every command takes --license <token>; a debug build also accepts --dev.
kshana-pro design scenarios/pro-power/design/lunar-navigation-global.toml --out out/ --dev
```

This writes five things into `out/`:

| File | What it is |
|---|---|
| `design.result.json` | Every evaluated design, the front, the knee, the reference design, the labels and the reproducibility block |
| `design.pareto.svg` | Every evaluated design as a dot, the front as a line, the knee and the reference design marked |
| `design.report.html` | One self-contained page (it fetches nothing): the chart, the front table, the labels, reproducibility |
| `front/index.csv` | One row per front design: its variables, objectives and the two SHA-256 values |
| `front/NNN.toml` | Each front design as a plain open scenario |

`kshana-pro run <study.toml>` runs the same study and prints the JSON, so a study also runs
through the service and the registry like any other scenario kind (`pro-design-optimise`).

If `out/front/` already exists, the `index.csv` and `NNN.toml` files of an earlier run are
replaced and every other file in it is left alone.

The report and the chart show a number with up to four decimals, or with four significant
digits when it is below 0.1 (0.001234, 4e-6), so a small value is never shown as 0. The
JSON and the CSV hold every number in full.

To open a front design in free Kshana, run the file with the open engine:

```bash
kshana out/front/004.toml
```

The result document it writes has the SHA-256 that `front/index.csv` lists for that design.

## What it reads from the open engine

| What | How |
|---|---|
| The result of each design | One call of `kshana::api::run_scenario` on the design's scenario text. The objectives and constraint values are numbers read from that result document. |
| The front and the knee | `kshana::mcda::pareto::pareto_front` and `knee_point`, the open engine's own kernel. |
| The labels | `kshana::verification::verification_matrix`, each run's `label`, and each run's `figure_tiers` block when it has one. |
| The units | Each run's own `units` block. |

Nothing else is computed. There is no cost model: an objective is a number the engine
wrote, or the value of a design variable.

## The study file

A study is a TOML file. A key the format does not have is an error, at every level: a
misspelt table such as `[[constraints]]` stops the study with the name of the key, instead
of being ignored and leaving a front that looks as if the constraint held.

```toml
kind = "pro-design-optimise"
title = "Lunar navigation constellation: fewest satellites against availability, whole Moon"
seed = 1

# A complete open scenario, of any open kind, as a string.
base_scenario = """
kind = "constellation-design"
body = "moon"
...
[[constellation.shell]]
total = 8
planes = 2
phasing = 0
altitude_km = 1500.0
inclination_deg = 45.0
"""

[[variable]]
name = "total"
key = "constellation[0].shell[0].total"
type = "choice"
values = [8, 12, 14, 16, 20, 24]

# ... `planes`, `phasing` and `altitude_km` are declared the same way ...

[[variable]]
name = "inclination_deg"
key = "constellation[0].shell[0].inclination_deg"
type = "real"
min = 45.0
max = 90.0
step = 15.0

[[constraint]]
walker = { total = "total", planes = "planes", phasing = "phasing" }

[[objective]]
name = "satellites"
sense = "min"
path = "total_satellites"

[[objective]]
name = "availability_pct"
sense = "max"
path = "global.availability_pct"

[search]
method = "auto"
budget = 1000

[reference]
label = "Bundled lunar relay design"
scenario = '''
...a complete open scenario...
'''
```

### Top level

| Key | Meaning |
|---|---|
| `kind` | `pro-design-optimise`. `kshana-pro run` finds the study by it; `kshana-pro design` accepts a file without it and refuses any other kind. |
| `title` | Shown on the chart and the report |
| `seed` | Seed of the search's random numbers (default 1). It matters only for NSGA-II. |
| `base_scenario` | A complete open scenario as a TOML string. Every design is this scenario with the variables' values written in. |

### `[[variable]]`

| Key | Meaning |
|---|---|
| `name` | The name used by constraints, objectives, the reference and the outputs |
| `key` | Dotted path of the key in the base scenario, with `[i]` for an element of an array of tables, for example `constellation[0].shell[0].inclination_deg`. The key must already exist in the base scenario. |
| `type` | `real` or `integer` (a range: `min`, `max`, `step`; `step` defaults to 1 for an integer) or `choice` (a list: `values`, of numbers, strings or booleans) |
| `unit` | Optional. Used in the outputs when the engine's result does not state a unit for the key. A unit the engine states is never replaced: if the study writes a different one, the engine's is used and the note in the `units` block says so. |

Every variable is a finite list of levels, so a real variable is searched on a grid. A
value is written in the type the base scenario's key already has: a float key stays a
float and an integer key stays an integer.

### `[[constraint]]`

Each constraint is one of two kinds.

| Form | Meaning |
|---|---|
| `walker = { total = "<variable>", planes = "<variable>", phasing = "<variable>" }` | A design constraint for a Walker constellation pattern: `planes` must divide `total`, and `phasing` must be at least 0 and less than `planes` (`phasing` may be left out). A design that breaks it is **never run**. |
| `result = { path = "<result path>", op = "<=", value = 6.0 }` | A condition on a number of the run's result; `op` is one of `<=`, `<`, `>=`, `>`, `==`, `!=`. A run that breaks it **stays in the evaluation log and is kept off the front**. |

### `[[objective]]`

Two or three objectives. Each has a `name`, a `sense` (`min` or `max`) and exactly one of:

| Key | Meaning |
|---|---|
| `path` | A path into the run's result document, for example `global.pdop.p95` |
| `variable` | The name of a design variable, for example the satellite count |

An objective may also carry a `unit`, used the same way as a variable's: only where the
engine's result states none.

### `[search]`

| Key | Default | Meaning |
|---|---|---|
| `method` | `auto` | `exhaustive`, `nsga2`, or `auto`: exhaustive when the feasible space fits the budget, otherwise NSGA-II |
| `budget` | 2000 | The most engine runs the search may make |
| `population` | 40 | NSGA-II population size |
| `generations` | 50 | NSGA-II generations allowed |
| `threads` | all cores | Worker threads. It changes the run time, never the result. `--threads` on the command line overrides it. |

### `[reference]`

A design to place against the front: a `label` and either `values = { <variable> = <value>, ... }`
(a design of the space) or `scenario = "..."` (a complete open scenario, for a design from
outside the space). The reference is scored by the same engine run as every other design.
When an objective is a design variable and the reference is a full scenario, the value is
read at that variable's key in the reference scenario; for a reference with a different
structure (two shells, say) use a result path such as `total_satellites` as the objective.

## How the search works

- **Exhaustive.** Every feasible design is run, in design order. The result states
  `exhaustive: true` and the front is the front of the stated design space.
- **NSGA-II.** For a space larger than the budget. The search never spends a run on a
  design it has already run: an offspring that repeats one is changed until it is new, or
  replaced by an unexplored design. The result states `exhaustive: false` and the front is
  labelled "best found in N runs"; a better design may exist. If the budget does cover the
  whole feasible space, the search ends having run every feasible design and says so.
- **The front and the knee** come from the open engine's kernel, over every run that was
  scored and broke no result constraint. The knee is the front design farthest from the
  straight line between the two ends of the front, in objective space scaled to the front's
  range (Branke and others, 2004). A front with fewer than three distinct points, or one
  that is a straight line, has no knee (`knee.exists` is false).
- **Deterministic.** The same study file and seed give `design.result.json` byte for byte,
  for any thread count. Designs are evaluated in parallel and merged in design order.
- **A failed run is never dropped.** A design the engine rejects is kept in `evaluated[]`
  with status `failed` and the engine's error, and counted in `work.failed`. A design for
  which the engine wrote `null` for an objective (for example a PDOP percentile when no fix
  ever exists) is kept with status `no-value` and counted in `work.no_value`.

The search kernel is tested against the analytic ZDT1 problem, whose true front is known:
with a population of 100 and 250 generations, no returned point is below the true front and
the mean distance to it is 0.00018 on the development machine (the bar, fixed before the
first run, is 0.05).

## Worked example: a lunar navigation constellation

The studies are in `scenarios/pro-power/design/`; where their inputs come from is in
`scenarios/pro-power/design/PROVENANCE.md`.

`lunar-navigation-global.toml` asks: for a single Walker shell around the Moon, which
designs give the highest availability for the fewest satellites? Availability is the
area-weighted share of time and surface with a position fix and PDOP at or below 6, over
one day, above a 10 degree elevation mask.

The space has 1 152 combinations (6 satellite counts, 6 plane counts, 2 phasings,
4 altitudes, 4 inclinations). The Walker constraint removes 544 of them without a run, and
the other 608 are all run (about 10 seconds on four threads on the development machine). The front has 12 designs at
6 distinct points:

| Satellites | Availability (%) | One design at this point (planes / phasing / altitude / inclination) |
|---|---|---|
| 8 | 19.5446 | 8 / 1 / 8 000 km / 90 deg |
| 12 | 55.4222 | 2 / 1 / 8 000 km / 45 deg |
| 14 | 73.5637 | 7 / 1 / 8 000 km / 45 deg |
| 16 | 85.0404 | 2 / 1 / 8 000 km / 45 deg (the knee) |
| 20 | 93.2496 | 4 / 0 / 8 000 km / 60 deg |
| 24 | 100.0 | seven designs reach it |

Availability is an area-weighted average, so a high value can hide a hole: the
16-satellite knee design leaves regions with no fix at all (its worst-site availability is
0 %), which the open engine's coverage map of `front/004.toml` shows. A result constraint
on `global.worst_site_availability_pct` rules such designs out.

The reference design is the open engine's bundled `lunar-relay-constellation` scenario
(8 satellites in elliptical orbits that linger over the south pole, plus 6 in a circular
shell). Over the whole Moon it scores 14 satellites and 21.3286 % availability, and two
front designs (12 and 14 satellites) do at least as well on both objectives. That is a
statement about whole-Moon coverage, which the bundled design was not drawn for.

`lunar-navigation-south-polar.toml` asks the same question over the region the bundled
design targets, the south polar cap (latitude -90 to -70 degrees). The same 608 designs
give a front of 9 designs at 4 points: 8 satellites 24.5261 %, 12 satellites 69.5825 %,
14 satellites 91.6739 % (the knee) and 16 satellites 100 %. Here the bundled design scores
14 satellites and 96.7241 %: no front design dominates it, and it dominates the best
14-satellite single-shell design. A two-shell design with elliptical orbits is outside
this study's design space, and over the cap it is better than anything inside it at the
same satellite count. Read each chart with its region in mind.

Three more studies show the same tool on other questions:

- `leo-positioning-earth.toml`: a generic LEO positioning shell around the Earth with
  three objectives (satellites, availability, 95th-percentile PDOP) and a result
  constraint (a fix at 99 % or more of the time-space points). 1 782 combinations, 351
  removed by the Walker constraint, 1 431 run, 1 141 of those kept off the front by the
  result constraint; the front has 22 designs and its knee is 168 satellites, 90.0661 %
  availability and a 95th-percentile PDOP of 8.81.
- `mars-navigation-shell.toml`: a Walker shell around Mars, 72 designs, a front of 8.
- `link-budget-trade.toml`: not a constellation at all. EIRP against link margin over the
  `link-budget` kind; the front is the one the link equation predicts (1 dB of margin per
  dB of EIRP, less 10 log10 of the data-rate ratio), which a test checks.

### A budgeted search on the same space

Forcing NSGA-II on the 608-design lunar space shows what "best found" means. With a
budget of 150 runs (a quarter of the space), seeds 1, 2 and 3 found 4, 5 and 3 of the 6
true front points, and the hypervolume of the front found was 0.6217, 0.6347 and 0.6087
against 0.6558 for the true front. With 300 runs they found 5, 6 and 5. With a budget of
700 the search runs all 608 designs and returns the exhaustive front exactly. A
best-found front is a real set of designs, each reproducible; it is not a proof that
nothing better exists.

## The result document

`design.result.json` carries the same stamp as every Pro result, so the MBSE tooling
(`kshana-pro mbse`, see `docs/MBSE.md`) can read it:

| Field | Meaning |
|---|---|
| `schema_version`, `kind`, `overlay`, `overlay_version`, `engine_version`, `seed` | What made the document |
| `scenario_hash` | SHA-256 of the canonical form of the study: the parsed study as JSON with sorted keys, the base scenario parsed too, and the thread count left out. Comments, key order and the number of threads do not change it; any value does. |
| `label` | Starts with MODELLED; says whether the front is exhaustive or best found, and quotes the engine's own label for the base kind |
| `exhaustive`, `front_statement` | Whether every feasible design was run |
| `space` | The variables and their levels, the grid size, and the feasible and infeasible counts |
| `work` | `evaluations`, `cache_hits`, `infeasible`, `failed`, `no_value`, `constraint_violated`, `reference_runs` |
| `evaluated[]` | Every design run: its variables, status, objectives, constraint values, the SHA-256 of its scenario text and of its result document |
| `front[]` | The front designs: variables, objectives, both SHA-256 values and the scenario text |
| `knee` | The knee design, its distance from the chord and whether a knee exists |
| `hypervolume` | The share of the box between the best and the worst value of each objective that the front dominates |
| `reference` | The reference's objectives, whether it is on the front (no front design dominates it), which front designs dominate it and which it dominates |
| `convergence[]` | For NSGA-II: the front size and hypervolume after each generation |
| `units` | For every numeric field: `{unit, provenance, note}`, with provenance `computed`, `input` or `constant` |
| `figure_tiers` | For each headline figure: `{path, requirement, tier}` |
| `not_modelled[]` | What the result does not cover |
| `reproducibility` | The number of engine runs and `run_digest`: SHA-256 over one line per evaluated design, in order, holding the SHA-256 of that run's result document |

## What the label means

Every result is labelled **MODELLED**. A front is a selection among model runs, so it can
never be better founded than the model that scored the designs.

Each headline figure also takes the label of the weakest open capability it rests on, and
is never raised:

- If the run's own `figure_tiers` block names the figure, that label is used.
- Otherwise it is the weakest row of the open engine's verification matrix that names the
  base kind (VALIDATED: checked against an independent external reference; MODELLED: a
  first-principles or published-formula model, tested internally; PARTNER: a partner's
  discipline), lowered further if the run's own label says MODELLED or PARTNER.
- A figure on the front or at the knee is never above MODELLED, because its place there is
  decided by the Pareto kernel, which the engine documents as a modelled closed form.
- A kind that no matrix row names gets no label at all; the figure is listed under
  `untiered` with the reason. A design variable is an input and is also listed there.

For example, the `constellation-design` kind has a MODELLED row (coverage maps of
arbitrary designs around any body), so every figure of a constellation study is MODELLED,
even though the same kind reproduces the published GPS values: through this tool's
evaluator the GPS baseline constellation gives a global HDOP median of 0.94, a 90th
percentile of 1.165 and a 95th percentile of 1.255 (published: 0.94, 1.16 and 1.25) and
a PDOP median of 1.795, and a test holds it to that.

## Limitations

- **Geometry only, for the constellation studies.** The `constellation-design` kind scores
  visibility and DOP. It models no signal power, no satellite health and no terrain.
- **Two-body orbits.** Orbits are Keplerian around a spherical body, with optional secular
  J2 drift. There is no third-body perturbation, which a real lunar frozen-orbit design
  depends on, so a lunar front says which geometry covers best, not which orbit is stable.
- **A front that is not exhaustive is a best-found front.** NSGA-II returns real,
  reproducible designs that no other design it ran beats; a better design may exist.
- **No cost model.** Nothing here prices a satellite, a launch or an altitude. When an
  objective does not penalise a variable, the front drifts to that variable's edge: in the
  lunar study every front design sits at the highest altitude offered, because altitude
  improves coverage and costs nothing in this model. Put what matters into an objective or
  a constraint.
- **The objectives are only as good as the kind that computes them.** The tool reads
  numbers; it cannot make them mean more than the base kind's model does.
- **The design space is the one you state.** A design outside the variables and levels is
  not considered, which is why a reference from outside the space can dominate the front.
- **A grid, not a continuum.** A real variable is searched at its stated step.
- **Size.** A grid of more than 2 000 000 combinations is not enumerated: its feasible
  count is reported as unknown and the search is NSGA-II. The final front is computed over
  every scored design, which takes time that grows with the square of the number of runs.
- **One run per design.** Each objective is a single deterministic run, with no
  uncertainty attached.
