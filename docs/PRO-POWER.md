# Kshana Pro power features: design optimiser, uncertainty, mission dossier

Open Kshana answers "what does this one scenario give". The three features on this page
answer the questions a programme asks next, over the same open engine and on any open
scenario kind:

| Question | Command | Guide |
|---|---|---|
| Which design should we fly? | `kshana-pro design` | [DESIGN-OPTIMISER.md](DESIGN-OPTIMISER.md) |
| How sure are we, and what drives the result? | `kshana-pro uncertainty` | [UNCERTAINTY.md](UNCERTAINTY.md) |
| Does the mission meet its requirements, and can we hand that over? | `kshana-pro mission build` / `mission check` | [MISSION-DOSSIER.md](MISSION-DOSSIER.md) |

None of them adds or changes a physical model. Every number comes from a run of the open
engine through its public interface (`kshana::api::run_scenario`), so a figure is as
trustworthy as the open scenario kind it comes from, and never more. Every output is
labelled MODELLED and carries the engine's own VALIDATED / MODELLED / PARTNER tier for
each figure, never a higher one. Each feature works on any system: none depends on the
optional low-Earth-orbit workshop preset of the open engine, and each was tested with
that preset deleted from the engine (the mission dossier before its last three fixes,
which did not touch how scenarios are run).

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| CSV | comma-separated values |
| HTML | HyperText Markup Language |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| NSGA-II | non-dominated sorting genetic algorithm II |
| PDF | Portable Document Format |
| ReqIF | Requirements Interchange Format |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SVG | Scalable Vector Graphics |
| SysML v2 | Systems Modeling Language, version 2 |
| TOML | Tom's Obvious Minimal Language (the scenario and study file format) |

All three need the Enterprise tier (or the feature named on the licence). Without a
licence the binary runs at the Evaluation tier and refuses; `--dev` switches the check off
in a debug build only, and a release build refuses it. For access, write to
contact@ashforde.org.

## 1. Design optimiser

**What it does.** You state a design space over a base scenario of any open kind (each
variable a scenario key with its levels or a stepped range), constraints, and two or three
objectives. It returns the Pareto front (the designs no other design beats on every
objective at once), the knee of that front, and where a reference design sits against it.
A Walker pattern constraint removes impossible constellations without a run; a condition
on a result field keeps a run off the front. The search is exhaustive when the feasible
space fits the run budget, and a seeded NSGA-II otherwise; a front that is not exhaustive
is labelled "best found in N runs", never "optimal". The same study and seed give the same
bytes for any thread count.

**Command.**

```bash
kshana-pro design <study.toml> --out <dir> [--threads <n>] [--license <token>]
kshana-pro run <study.toml> [--license <token>]      # same study, prints the JSON
```

Scenario kind: `pro-design-optimise`.

**Inputs.** One study file (`kind = "pro-design-optimise"`): the base scenario as a string,
`[[variable]]`, `[[constraint]]`, `[[objective]]` (a result field or a design variable,
minimised or maximised), `[search]` (budget, seed, population) and an optional
`[reference]`. An unknown key anywhere is an error.

**Outputs.** `design.result.json` (every evaluated design, the front, the knee, the
reference, a unit for every numeric field, the figure tiers, what is not modelled, the
reproducibility digest), `design.pareto.svg`, a self-contained `design.report.html`,
`front/index.csv`, and `front/NNN.toml`: each front design as a plain open scenario that
the free engine runs to the result whose SHA-256 the index lists.

**Worked studies** (`scenarios/pro-power/design/`): a lunar navigation constellation over
the whole Moon and over the south polar cap, a LEO positioning shell, a Mars shell, and a
link budget. Measured and independently re-checked: the 608-design whole-Moon space has a
front of 12 designs at 6 points (8 satellites at 19.5446 % availability up to 24 satellites
at 100 %), equal to a brute-force front over 608 scenarios run with the free engine.

**Honestly not done.**
- No cost model: when an objective does not penalise a variable, the front drifts to that
  variable's edge (every lunar front design sits at the highest altitude offered).
- Constellation studies score geometry only, on two-body orbits with optional J2 drift: no
  signal power, no satellite health, no terrain, no third-body perturbation.
- A grid, not a continuum; a space above 2 000 000 combinations is searched by NSGA-II only
  and its feasible count is reported as unknown.
- One deterministic run per design, with no uncertainty attached (use the uncertainty
  feature on a chosen design).
- The knee is always named when it exists, even on a nearly straight front where it is
  barely one; its distance is in the result.
- The chart becomes small at phone width; the report was checked in one browser engine
  only.

## 2. Uncertainty and sensitivity

**What it does.** You name any open scenario, state how uncertain its inputs are (normal,
uniform, lognormal, triangular or discrete, each with its source; `assumed` is allowed and
printed as ASSUMED on every output) and list the outputs to watch. It runs the open engine
over a seeded Latin hypercube or random sample and reports, per output: mean, standard
deviation, 5th / 50th / 95th percentiles with a bootstrap interval on the mean, the
probability of meeting a limit with a Wilson interval, the percentile band of a time
series at every step, how the estimates settled, and a ranking of the inputs by
first-order and total Sobol' indices or standardised regression coefficients (flagged as
not reliable below an R-squared of 0.7). A constant output or input gives null with the
reason, never zero. With the engine seed varied and no input, a study reproduces the open
`campaign [monte_carlo]` bit for bit, so free and Pro never disagree on a number.

**Command.**

```bash
kshana-pro uncertainty <study.toml> --out <dir> [--resume] [--max-runs <n>] [--threads <n>] [--license <token>]
kshana-pro run <study.toml> [--license <token>]      # in memory, no checkpoint
```

Scenario kind: `pro-uncertainty`.

**Inputs.** One study file (`kind = "pro-uncertainty"`): the base scenario, `[[input]]`
(key, distribution, source), `[[output]]` (result path, optional limit), `[sampling]`,
`[sensitivity]`, `[run]`. A declared unit that differs from the engine's, a distribution
that can give a non-finite value, and a study above 10 000 000 engine runs are refused
before anything runs.

**Outputs.** `uq.result.json`, `uq.bands.svg`, `uq.sensitivity.svg`, `uq.convergence.svg`,
a self-contained `uq.report.html`, and the checkpoint `runs.jsonl` (one line per finished
realisation). A stopped and resumed study ends with the same files, byte for byte, as an
uninterrupted one. A failed engine run is recorded and counted, and never counts as
meeting a limit.

**Worked studies** (`scenarios/pro-power/uncertainty/`): a link margin against its closed
form, the open clock-holdover campaign reproduced and then taken further, the same clock
with an uncertain noise level, and the LEO positioning chain with regression and with
Sobol' sensitivity. The estimators are checked against the Ishigami function's closed form
(all six indices within 0.02; the intervals contained the closed form in 72 of 72 cases
over twelve seeds).

**Honestly not done.**
- Inputs are independent: correlation between inputs is not modelled.
- The result is conditional on the stated distributions, which the tool cannot check.
- More runs narrow the sampling error, not the model error: a MODELLED kind stays MODELLED.
- Sensitivity is for scalar outputs; a series gets a band, not indices. The engine seed
  is not ranked.
- A value edited inside a checkpoint line that still parses is accepted on resume and does
  not change the run digest.
- The report pages were not reviewed in a browser matrix.

## 3. Mission dossier

**What it does.** One programme file (a requirements file and a list of scenarios of any
kind, optional headline figures and charts, an optional `pro-trade-study`, sign-off rows)
becomes one dossier in one command. Every scenario is run; every requirement is checked
against the real runs with a unit-checked, machine-readable criterion; the verification
matrix lists each verdict with its evidence (scenario hash, seed, engine version, the
fields and values read) and its basis, and flags every met requirement that rests on a
MODELLED or PARTNER basis. Test, inspection and demonstration requirements stay open items
however the run went. It composes the existing requirements tooling (`kshana-pro mbse`)
and computes no result of its own. `mission check` re-hashes a delivered dossier and, with
`--rerun`, runs every scenario again and compares.

**Command.**

```bash
kshana-pro mission build <programme.toml> --out <dir> [--utc <iso>] [--license <token>]
kshana-pro mission check <dir> [--rerun] [--license <token>]
```

Licence feature: `pro-mission-dossier`. It is a command, not a scenario kind.

**Inputs.** The programme file (TOML) and a requirements file in CSV, ReqIF or SysML v2
text. Run identifiers that differ only in letter case, scenario values that are not
finite numbers and criterion limits that are not finite are refused before anything runs.

**Outputs.** `dossier.pdf` (A4: cover, MODELLED banner, verdict summary, matrix, one sheet
per run with vector charts, trade, open items, reproducibility record, sign-off,
limitations), `dossier.html` (self-contained, no network request, no script),
`dossier.json` (with the dossier hash), `runs/<id>/` (each scenario unchanged, its result,
chart and, for an open kind, the engine's report), `trace.toml`, the evidence pack,
`manifest.json` (every file with its SHA-256) and `ledger.json` (a hash-chained record).

**Worked programme** (`scenarios/pro-power/mission/programme.toml`): twelve illustrative
requirements over five open-engine runs (a LEO positioning chain, a lunar relay
constellation, the L band under jamming, a jam-spoof-holdover campaign, a Mars orbiter)
and one lunar relay trade. Result: 7 met, 4 not met, 1 not verified, 15 open items; every
value was recomputed from the free engine's own output files.

**Honestly not done.**
- The dossier hash, manifest and ledger are not signed: they prove the files agree with
  each other, not who issued them.
- Simulation evidence counts as analysis; it does not replace a test, inspection or
  demonstration.
- The PDF uses the built-in fonts and shows ASCII only; the HTML and JSON keep the text.
- A data file a scenario names by relative path is not copied into the dossier; a
  scenario that fails mid-build leaves earlier runs under `runs/` with no dossier.
- The trade section shows what the `pro-trade-study` pack returns; the free engine
  re-runs each variant, not the trade.
- SysML v2 requirements input through this command was not exercised by the independent
  check; only Chromium on macOS was used for the HTML.
- Nothing here is a certification, a qualification or an acceptance by any authority.

## 4. What holds for all three

- **Verification.** Each feature was built on its own branch, then checked by a separate
  reviewer who re-ran every claim, compared outputs with the free engine run on its own,
  tried adversarial inputs, mutated the code to prove the tests fail, and fixed what they
  found with a test for each defect. Test files: `tests/design_optimise.rs`,
  `tests/uncertainty.rs`, `tests/mission_dossier.rs`.
- **Not yet a release.** Every number above comes from a debug build of Kshana Pro, which
  embeds the open engine optimised. No release build and no signed licence token were
  exercised; only the refusal without a licence and the development switch were. Numbers
  for a customer come from a release build.
- **Development engine pin.** On this branch Pro builds against an unpublished 75-kind
  open engine by path (see the README, "Build & run"). A release must first move to the
  published engine version that carries those kinds and re-run every check with `--locked`.
