# Kshana Pro power features: design optimiser, uncertainty, mission dossier, campaign watch, spectrum coexistence, job service

Open Kshana answers "what does this one scenario give". The six features on this page
answer the questions a programme asks next, over the same open engine and on any open
scenario kind:

| Question | Command | Guide |
|---|---|---|
| Which design should we fly? | `kshana-pro design` | [DESIGN-OPTIMISER.md](DESIGN-OPTIMISER.md) |
| How sure are we, and what drives the result? | `kshana-pro uncertainty` | [UNCERTAINTY.md](UNCERTAINTY.md) |
| Does the mission meet its requirements, and can we hand that over? | `kshana-pro mission build` / `mission check` | [MISSION-DOSSIER.md](MISSION-DOSSIER.md) |
| Did a figure that matters get worse since the last run, or is it noise? | `kshana-pro ci` with `[[watch]]` tables, `ci history verify`, `ci diff` | [CAMPAIGN-WATCH.md](CAMPAIGN-WATCH.md) |
| Does our whole signal plan and constellation coexist with the open navigation signals? | `kshana-pro coexist` | [SPECTRUM-COEXISTENCE.md](SPECTRUM-COEXISTENCE.md) |
| Can the team run all of this as a service on its own network? | `kshana-pro serve --data-dir` | [SERVICE.md](SERVICE.md) |

None of them adds or changes a physical model. Every number comes from a run of the open
engine through its public interface, so a figure is as trustworthy as the open scenario
kind it comes from, and never more. The results carry the engine's own VALIDATED /
MODELLED / PARTNER tier for each figure, never a higher one; the design optimiser,
uncertainty, mission dossier and spectrum coexistence label every output MODELLED;
campaign watch carries each result's own label next to every watched figure; the job
service stores each result as the engine gave it and shows only the tier the result
states. None depends on the optional low-Earth-orbit workshop preset of the open engine.
The first three were tested with that preset deleted from the engine (the mission dossier
before its last three fixes, which did not touch how scenarios are run); for campaign
watch and spectrum coexistence a test scans the feature's files for the preset's name;
the job service runs whatever scenario it is sent.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| C/N0 | carrier-to-noise density ratio |
| CI | continuous integration |
| CSV | comma-separated values |
| GNSS | global navigation satellite system |
| GPS | Global Positioning System |
| HTTP | HyperText Transfer Protocol |
| JSONL | JSON Lines (one JSON document per line) |
| OpenAPI | the OpenAPI Specification, a machine-readable description of an HTTP interface |
| TLS | Transport Layer Security |
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

The design optimiser, uncertainty, the mission dossier and spectrum coexistence need the
Enterprise tier (or the feature named on the licence); campaign watch needs the
`pro-scenario-ci` feature (Pro tier); the job service serves every operation at the tier
of the licence the process runs with. Without a
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

## 4. Campaign watch

**What it does.** It turns a scenario pack into a subscription that keeps proving itself.
A pack manifest carries `[[watch]]` tables: the figures that matter (a result path, an
aggregate such as `max(...)`, a named campaign figure such as a channel's value at the end
of a phase, the first alarm time or a sweep crossing), what worse means for each
(`higher`, `lower` or `target`), a tolerance, and an optional absolute requirement checked
with units. `kshana-pro ci` then fails the pipeline when a watched figure gets worse or a
requirement stops being met. A Monte Carlo figure is decided by a Welch t-test on the
means and an F-test on the variances at a stated significance level (default 0.01), so
another set of seeds is not called a regression and a doubled spread is. A pack without
watch tables runs exactly as before, byte for byte.

**Command.**

```bash
kshana-pro ci --pack <pack.toml> [--baseline <ci.json>] --json <ci.json> [--out <report.html>] \
              [--history <history.jsonl>] [--trend <trend.html>] [--exact] [--license <token>]
kshana-pro ci history verify <history.jsonl>
kshana-pro ci diff <old ci.json> <new ci.json> [--json <out>]
```

Scenario kind of the product: `pro-scenario-ci`.

**Inputs.** A pack manifest (TOML) listing scenarios of any open kind, with `[[watch]]`
tables; optionally the previous passing `ci.json` as the baseline and an existing history.

**Outputs.** `ci.json` (every figure, every watch and its verdict; it is the next run's
baseline), the branded HTML report, the append-only hash-chained `history.jsonl`, a trend
page (`trend.html` with one sparkline per watched figure, and `trend.json`), and the
figure-by-figure diff. Exit code 0 when every watch holds, 1 otherwise. Five subscription
packs ship in `scenarios/packs/` (low Earth orbit, constellations, spectrum, campaigns,
the solar system), each with provenance, and a six-revision demonstration sequence in
`scenarios/pro-power/watch-demo/`.

**Checked at merge.** `campaign-baseline` run twice on the merged branch: 10 watches, PASS
both times, `--exact` found 0 changed results, `ci history verify` reported 2 records with
the chain intact, and `ci diff` reported 0 of 125 figures moved.

**Honestly not done.**
- A watch proves that a MODELLED figure stayed stable or moved; it does not prove the
  figure is true.
- The statistical gate needs the ensemble's mean, standard deviation and count; there is no
  test for correlated realisations or for percentiles, and the F-test assumes roughly
  normal samples.
- A figure with no stated direction is informational and never fails a run.
- The end of a campaign phase is read on the campaign's grid and can differ from the
  engine's carried value by one grid step.
- Criteria compare values; there is no arithmetic between fields.

## 5. Spectrum coexistence

**What it does.** It tests one or more candidate signal plans, each a signal design plus a
constellation, against every open-service GNSS signal the open engine models (GPS L1 C/A
(coarse/acquisition) and L5; Galileo E1, E5a, E5b and the E5 alternative binary offset
carrier signal), in both directions and over a worldwide grid, and gives a verdict per plan
and signal against the limit the study states (`max_degradation_db` with its
`limit_source`; no limit is built in). The free `leo-signal` kind answers this for one
satellite; this answers it for every satellite of the plan in view at once. Every physical
number is read from open `leo-signal` and `constellation-design` runs; the feature adds
only the aggregation and ranks the plans with the open Pareto routine. It refuses any
table named for jammers, spoofers, attacks or threats.

**Command.**

```bash
kshana-pro coexist <study.toml> --out <dir> [--license <token>]
kshana-pro run <study.toml> [--license <token>]      # prints the result JSON
```

Scenario kind: `pro-spectrum-coexistence`.

**Inputs.** One study file (`kind = "pro-spectrum-coexistence"`): the limit and its source,
the victim signals (only the six open-service signals above), the grid, and one `[[plan]]`
per candidate with its signal design and its satellites. A received power of 0 dBW (1 W)
or more is refused as a unit slip.

**Outputs.** `coexistence.result.json` (a MODELLED upper bound, every open run with the
SHA-256 of its scenario and result, a unit for every numeric field),
`coexistence.matrix.svg`, `coexistence.map.svg`, `coexistence.pareto.svg` and a
self-contained `coexistence.report.html`.

**Worked example** (`scenarios/pro-power/coexistence/generic-walker-coexistence.toml`):
four plans on the open engine's generic designs against an illustrative 0.1 dB limit.
**Checked at merge.** Re-run: scenario hash `479364bd…985d4`, 4 of 4 plans compatible; the 288-satellite
L-band plan's worst loss caused is 0.00544 dB, the 144-satellite plan's 0.00288 dB, and
the two C-band plans have no spectral overlap with any victim.

**Honestly not done.**
- An upper bound: every visible satellite at the maximum received power, no
  elevation-dependent power, no receive-antenna pattern.
- Open-service signals only, and only those the open engine models (not GPS L2C).
- Smooth spectra with no spreading-code lines, as in the open `leo-signal` kind.
- No other systems in the sum: other GNSS, other low Earth orbit systems and terrestrial
  services are left out.
- Not a coordination filing and not a regulatory finding.

## 6. On-premises job service

**What it does.** `kshana-pro serve --data-dir <dir>` keeps a queue of jobs on disk, runs
them on a bounded pool of workers, stores each result and its artefacts under a job
identifier (the SHA-256 of the operation and the canonical request, so the same request is
the same job), and records every delivery in a hash-chained ledger. A stop or a crash loses
no work: on restart every unfinished job is queued again, and a finished job is never run
twice. Three operations ship: `run` (any open or Pro scenario), `dossier` (the study
dossier) and `ci` (a Pack C run). The OpenAPI 3.1 document is generated from the same route
table as the router. A self-contained job board page lists the jobs. Standard library
only, no outbound connection, no telemetry.

**Command.**

```bash
kshana-pro serve --addr <host:port> --data-dir <dir> [--workers <n>] \
                 [--api-token-file <path>] [--license <token>]
```

Routes: `POST /jobs`, `GET /jobs`, `GET /jobs/<id>`, `GET /jobs/<id>/result`,
`GET /jobs/<id>/artefacts/<name>`, `GET /ledger`, `GET /ledger/verify`,
`GET /openapi.json`, `GET /` (the job board), beside the existing `GET /health`,
`/version`, `/kinds`, `POST /run` and `POST /study`.

**Inputs.** `{"op": ..., "request": ...}` as JSON (at most 16 MiB, mebibytes): a scenario's
text for `run` and `dossier`; a manifest, its scenario texts and an optional baseline for
`ci`. The entitlement is checked on submission (403, nothing queued) and again when the job
runs.

**Outputs.** Per job under `<data-dir>/jobs/<id>/`: `request.json`, `status.json`,
`result.json` and `artefacts/`; the delivery ledger; the verification report of
`GET /ledger/verify`.

**Checked at merge.** A server on the merged branch with two workers took one `run` job
(the public link-budget scenario from `tests/fixtures/service/`): 202 on submission,
succeeded after one attempt, its result the same JSON document as `kshana-pro run` on the
same file (the command line prints it formatted, so the bytes differ), `GET /ledger/verify`
verified with 3 records and 1 delivered job, the OpenAPI document listed 13 paths, and the
job board answered 200.

**Honestly not done.**
- One request per connection, no keep-alive or streaming; a client polls.
- One entitlement for the whole process; no per-user tier.
- The bearer token is one shared secret, not user management.
- Plain HTTP only: TLS belongs in the site's own reverse proxy.
- One server per data directory; nothing locks it. No cancellation, no time limit, no
  priority, no retention policy.
- The data directory is the only copy of the results and the ledger; back it up.

## 7. What holds for all six

- **Verification.** Each feature was built on its own branch and then checked by a
  separate reviewer, who fixed what they found with a test for each defect. For the first
  three the reviewer re-ran every claim, compared outputs with the free engine run on its
  own, tried adversarial inputs and mutated the code to prove the tests fail. The last
  three passed their independent verification after fixes and were merged together; after
  the merge every test binary passed and each new command was run once for real (the
  "Checked at merge" lines above). Test files: `tests/design_optimise.rs`,
  `tests/uncertainty.rs`, `tests/mission_dossier.rs`, `tests/campaign_watch.rs`,
  `tests/spectrum_coexistence.rs`, `tests/service_jobs.rs`.
- **Not yet a release.** Every number above comes from a debug build of Kshana Pro, which
  embeds the open engine optimised. No release build and no signed licence token were
  exercised; only the refusal without a licence and the development switch were. Numbers
  for a customer come from a release build. Since PRO-14, CI runs `scripts/release-smoke.sh`:
  a release build with a CI-only licence key, a signed Pro token, and one Pro pack whose
  report must equal the debug build's byte for byte. The figures above were not re-taken
  from a release build; the smoke test shows only that the two builds agree on that pack.
- **Development engine pin.** On this branch Pro builds against an unpublished 75-kind
  open engine by path (see the README, "Build & run"). A release must first move to the
  published engine version that carries those kinds and re-run every check with `--locked`.
