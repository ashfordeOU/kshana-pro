# Changelog

All notable changes to the private Kshana Pro overlay. Each lane keeps its entry in its
own bullet so parallel work merges cleanly.

## [Unreleased]

- **Pro now builds on the integrated open engine: 75 open scenario kinds (was 63), all
  reachable through Pro. DEVELOPMENT PIN, not a release pin.** On branch `feat/pro-power`
  the `kshana` dependency points by path at a dedicated, read-only engine worktree
  (`../kshana-wt-pro-engine`, branch `feat/pro-engine-base` of the open repository, commit
  `da55ae6`). That engine adds twelve kinds to the published 0.28.0: `spectrum`,
  `solar-system`, `body-pnt`, `constellation-design`, `campaign`, `leo-signal`,
  `leo-pass`, `leo-navmsg`, `leo-pvt`, `leo-ppp`, `ntn-positioning` and `leo-pnt-chain`
  (spectrum and waterfall, the solar system and positioning around any body,
  constellation design, campaigns, and the low-Earth-orbit positioning, navigation and
  timing (LEO-PNT) chain; both lists were counted with each binary's `kinds --json`). The
  engine's animation, report and interoperability-export commands are command-line
  features of the open binary, not scenario kinds, and are not re-exposed by the
  `kshana-pro` command line here. The tree still reports version 0.28.0 but is
  not the 0.28.0 on crates.io. **Release step owed, not done:** before any Pro release,
  replace the path with the published engine version that carries these kinds, run
  `cargo update -p kshana`, and re-run the whole check set with `--locked`; a path pin
  builds only on a machine that has the worktree. No overlay source needed changing to
  compile: the engine's public seams (`api`, `registry`, `fom_label`, the model traits)
  are unchanged for the overlay's uses. New: `kshana-pro kinds --json` prints the open
  and Pro catalogues with `open_count` and `pro_count` (the open half is read from the
  embedded engine, never re-listed), and `tests/open_engine_reachable.rs` runs every
  open kind from one of the engine's own generic scenario files three ways (open engine,
  Pro dispatcher with no licence, open ∪ Pro registry by id) and requires the same result
  document byte for byte. It never uses the engine's optional Celeste preset files, so it
  passes with that preset absent. The README states nineteen integration tests (was
  eighteen). The sixteen Pro packs are unchanged and all still build and run
  (`registry::tests::every_pro_kind_builds_and_runs_through_the_registry`).
  Development and test builds now compile the embedded engine at optimisation level 3
  (`[profile.dev.package.kshana]`), so the library tests take about half a minute instead
  of about nine. Measured on the engine's 138 scenario files (its optional preset's five
  included) against the open release binary: `kshana-pro run` from a release build gave
  the same result document byte for byte on all 138; from a development build it matched
  136 and differed in the last bits of one field in each of two files
  (`combined-altpnt`, `gravity-map-nav`; relative 3e-15 to 6e-15). A number for a
  customer or a publication comes from a release build.

- **Model-based systems engineering (MBSE) and programme tooling over the open engine's
  own outputs (`kshana-pro mbse import|trace|vcrm|sysml|impact|pack`).** Requirements are
  imported from comma-separated values (CSV), a documented subset of the Requirements
  Interchange Format (ReqIF 1.2) and Systems Modeling Language v2 (SysML v2) text. Each
  carries a verification method (test, analysis, inspection or demonstration) and an
  acceptance criterion: a machine-checkable expression over the fields of an open-engine
  `result.json`, with units, aggregates (`max`, `p95`, `fraction`, ...) and `for all` /
  `for any` quantifiers over arrays. A trace file links requirements to runs; each
  criterion is evaluated against the real result document and unit-checked through the
  engine's field-units schema (`kinds.<kind>.fields.<path>.unit`), with the result's own
  `units` block as fallback, and a unit mismatch is reported as not verified. The
  verification cross-reference matrix (VCRM) records, per requirement and run, the
  verdict (met / not met / not verified), the run's `scenario_hash`, `seed`,
  `engine_version` and `schema_version`, every field path and value read, and the
  VALIDATED / MODELLED / PARTNER label the verdict rests on (from `figure_tiers`, the
  schema's `evidence_tier`, the report's `capabilities.rows` and the verification
  matrix), so a MODELLED or PARTNER basis is flagged. A scenario or result file whose
  SHA-256 no longer matches its report, or a scenario file whose `kind` or `seed`
  disagrees with the run's, makes the run's verdicts not verified. The SysML
  v2 export (part definitions for the simulated clocks, receivers, constellations and
  jammers with units from the schema, requirement definitions, verification cases per
  run, `satisfy` only where met) passes an in-repository checker for the grammar subset
  it uses, documented in `docs/MBSE.md`. Change impact lists the requirements to
  re-verify between two traces (scenario hash, engine version, seed or capability label
  changed, or, when no stamp tells the runs apart, the result file's SHA-256) or two
  scenario files (keys compared after parsing). The evidence pack is one
  offline HTML page plus JSON with the VCRM, a traceability graph, the reproducibility
  record of every run, open items and a sign-off table, hashed for reproducibility. The
  commands need the `pro-mbse` feature (Enterprise tier), gated exactly like `ci` and
  `study`. Tests: `tests/mbse.rs` (22 tests) over real `kshana 0.28.0` runs of
  clock-holdover, integrity-raim, campaign-jam-spoof-holdover-integrity,
  leo-pnt-end-to-end and a re-specified clock-holdover, committed with their provenance
  in `tests/fixtures/mbse/`, plus unit tests in each new module. The README states
  eighteen integration tests (was seventeen).

- **Study dossier (`kshana-pro study --dossier`), and the README pack count fixed.**
  One scenario now becomes a reproducible evidence dossier, written as JSON
  (`dossier.json`), a self-contained HTML page (`dossier.html`) and the exact scenario
  text run (`scenario.run.toml`). The page is laid out like a European Space Agency
  (ESA) technical note: identification, scope, applicable and reference documents,
  method, results, limitations. It records the open engine and Pro versions, the SHA-256
  of the input, the engine's scenario hash, and a SHA-256 result hash over the whole
  dossier; the same scenario gives the same hash and byte-identical files (an optional
  `--utc` issue stamp is kept outside the hash). Every figure carries the VALIDATED or
  MODELLED tier the open verification matrix grants it, read through
  `kshana::fom_label::tier_for`; a figure with no owning row is MODELLED. A
  one-at-a-time sensitivity sweep re-runs the scenario with each key parameter changed
  alone and reports each figure's change and elasticity. For `clock` scenarios the
  dossier adds the Pro holdover band and its maximum time interval error (MTIE) check
  against the scenario's own time-error budget and the published clock-class limits
  already in `twin::holdover_uq`: the enhanced primary reference time clock (ePRTC) 30 ns
  and primary reference time clock (PRTC-A) 100 ns class limits, and the conventional
  1.5 µs / 24 h holdover budget (an industry convention, not an ITU-T clock-class limit). No new mask values
  were added: the MTIE check already existed. The run's behaviour is read against the
  four levels of the US Department of Homeland Security (DHS) Resilient PNT Conformance
  Framework (RPCF) v2.0, Table 1, with each of its eight minimum requirements recorded
  as not assessed where a simulation cannot verify it, and Prevent / Respond / Recover
  readings are offered for later mapping to IEEE P1952, whose draft is not public. All
  of it is labelled "tentative, simulation-derived, not a certified level". The Goal
  Structuring Notation (GSN) argument, Verification Control Document (VCD) and W3C-PROV
  record are built with the existing L3 types; a `pro-trade-study` scenario also gets
  the trade's own gate, GSN, VCD and provenance appended unchanged. The command needs
  the Pro tier (`pro-study-dossier`) and fails closed without a licence. Worked example:
  `scenarios/dossier-holdover-csac.toml`. The README said seven Pro packs; the registry
  holds sixteen, and the README and `docs/OVERLAY-ARCHITECTURE.md` now say so. The new
  `tests/readme_counts.rs` fails if either count, or the README's integration-test
  count, stops matching the registry or `tests/`. Tests: `tests/study_dossier.rs`
  (determinism, tier split, the binary end to end and its licence gate) and unit tests
  in `src/study_dossier.rs`.

- **Design optimiser: the Pareto front of a design space, over any open kind and around
  any body (`kshana-pro design <study.toml> --out <dir>`, new pack `pro-design-optimise`).**
  A study states variables written into a base scenario of any open kind, constraints
  (a Walker pattern constraint that removes infeasible designs without a run, and conditions
  on result fields that keep a run off the front) and two or three objectives, each a field
  of the engine's result or a design variable; there is no cost model. Every design is one
  run of the open engine through `kshana::api::run_scenario`; the front and its knee come
  from the open kernel `kshana::mcda::pareto`. The search is exhaustive when the feasible
  space fits the budget and a seeded NSGA-II (non-dominated sorting genetic algorithm II)
  otherwise; a front that is not exhaustive is labelled "best found in N runs". The same
  study and seed give the same bytes for any thread count. Outputs: `design.result.json`
  (with the Pro result stamp: `scenario_hash`, `units` for every numeric field,
  `figure_tiers`, `not_modelled`, `reproducibility.run_digest`), `design.pareto.svg`,
  a self-contained `design.report.html`, `front/index.csv` and `front/NNN.toml`, each front
  design as a plain open scenario that the free engine runs to the same result document.
  Every result is labelled MODELLED, and each figure takes the label of the weakest open
  capability it rests on. Five worked studies in `scenarios/pro-power/design/` (a lunar
  navigation constellation over the whole Moon and over the south polar cap, a low-Earth-orbit
  positioning shell, a Mars shell, a link budget), with provenance. Measured: the 608-design
  lunar space has a front of 12 designs at 6 points (8 satellites 19.5446 % availability
  to 24 satellites 100 %), equal to a brute-force front over scenarios the test writes
  itself; through the evaluator the GPS (Global Positioning System) baseline constellation
  reads the published horizontal dilution of precision (median 0.94, 95th percentile 1.255
  against 1.25 published); on the analytic ZDT1 benchmark the search kernel's mean distance
  to the true front is 0.00018 against a bar of 0.05 fixed beforehand (the first run missed
  the bar at 0.177 because the crossover lacked its exchange step; the operator was
  corrected and the bar was not moved). The feature falls to the default, fail-closed
  licence tier. Guide: `docs/DESIGN-OPTIMISER.md`. Tests: `tests/design_optimise.rs`
  (the README now states twenty integration tests and seventeen Pro packs).

- **Design optimiser: five defects found by an independent verification, each fixed with a
  test.** (1) A key the study format does not have was ignored, so a misspelt table
  (`[[constraints]]` for `[[constraint]]`) dropped the constraint and the study still
  returned a front; an unknown key is now an error at every level, on `kshana-pro design`
  and on `kshana-pro run`, and a file of another `kind` is refused. (2) A `unit` written in
  the study file replaced the unit the engine's own result states, so `unit = "m"` could
  relabel a number the engine gives in kilometres; the engine's unit is now always used,
  the study's only where the engine states none (as the guide already said), and the
  `units` note records a disagreement. (3) `kshana-pro design --out <dir>` deleted the
  whole `<dir>/front/` folder before writing; it now removes only `index.csv` and
  `NNN.toml` files of an earlier run. (4) The report and the chart rounded to four
  decimals, so a value of 0.000004 read "0"; a number below 0.1 now keeps four significant
  digits. (5) The README still said sixteen packs in one sentence; it says seventeen and
  `tests/readme_counts.rs` now reads that sentence too. None of this changes a published
  number: the five worked studies give the same `design.result.json`, chart, report and
  front files byte for byte, with the same study hashes. The test that keeps the optional
  low-Earth-orbit preset out of the feature's scenarios now looks at file names inside the
  scenario folder, not at the path the repository is checked out in.

- **Uncertainty and sensitivity at scale, over any scenario kind (`kshana-pro
  uncertainty`, kind `pro-uncertainty`).** A study names any open scenario, states how
  uncertain its inputs are (normal, uniform, lognormal, triangular or discrete, each with
  the source of that uncertainty; the word `assumed` is allowed and is then printed as
  ASSUMED on every output) and lists the outputs to watch. It runs the open engine over a
  seeded Latin hypercube or plain random sample and reports, per output: the mean, the
  standard deviation, the 5th, 50th and 95th percentiles and a bootstrap interval on the
  mean, the running estimate as runs accumulate, the probability of meeting a limit with
  a Wilson interval ("given the stated input uncertainty"), and for a time series the
  percentile band at every step. Sensitivity is either standardised regression
  coefficients from the same runs, flagged in words as not reliable when the R-squared of
  the linear fit is below 0.7, or first-order (Saltelli and others 2010) and total-order
  (Jansen 1999) Sobol' indices with bootstrap intervals, from `n * (k + 2)` runs for `k`
  inputs. A constant output or input gives null with the reason, never zero. The result
  is byte-identical for any thread count. The command appends one line per finished
  realisation to `runs.jsonl`, stops cleanly at `--max-runs` and continues with
  `--resume`, refuses a study that changed since the checkpoint was started, and repairs
  a last line cut short by re-running that realisation; a stopped and resumed study ends
  with the same six files as an uninterrupted one. A failed engine run is recorded and
  counted, never dropped, and never counts as meeting a limit. With no input and the
  engine seed varied, a study reproduces the open `campaign [monte_carlo]` bit for bit
  (samples, statistics and bootstrap interval), so free and Pro never disagree on a
  number. Every result is labelled MODELLED, carries a unit for every numeric field and
  the base kind's tier for every output, never a higher one, and lists what is not
  modelled (correlation between inputs first). `pro-batch` is unchanged. Needs the
  Enterprise tier (feature `pro-uncertainty`); fails closed without a licence. New:
  `src/batch/uq/`, `src/packs/uncertainty_pack.rs`, five studies and their engine
  scenario copies with provenance in `scenarios/pro-power/uncertainty/`, the guide
  `docs/UNCERTAINTY.md`. Tests: `tests/uncertainty.rs` (15 tests: the Ishigami closed
  form, a link margin's closed form, the open campaign reproduced, the mixture closed
  form for an uncertain clock noise level, the low-Earth-orbit navigation chain against
  direct engine runs, thread-count independence, stop and resume, failed runs,
  adversarial studies, the licence gate) and unit tests in each module. The README now
  states seventeen Pro packs (was sixteen) and twenty integration tests (was nineteen).
  Hardened after an independent review: a declared output `unit` that differs from the
  unit the base kind reports is refused (it was taken as given); a distribution that
  gives a value that is not a finite number is refused before anything runs; a study
  whose every realisation fails finishes with null and a reason under `src` (it
  panicked); one study makes at most 10,000,000 engine runs (a very large `n` exhausted
  memory); a new study does not start beside the result files of another run. No
  published number changes.

- **Mission dossier (`kshana-pro mission build` and `mission check`): requirements to
  runs to verification matrix to a branded PDF, in one command.** A programme file
  (TOML: title, reference, issue, classification, a requirements file in comma-separated
  values (CSV), Requirements Interchange Format (ReqIF) or Systems Modeling Language
  version 2 (SysML v2), a list of scenarios of any kind with optional headline figures
  and charts, optional links, an optional `pro-trade-study`, sign-off rows) becomes one
  deliverable. Every scenario's exact bytes are run through the gated dispatcher and
  written under `runs/<id>/` as the open engine's own command line writes them, with the
  open engine's report (`kshana::advanced_report::build`), so the requirements tooling's
  integrity checks hold; a Pro kind is written without a report and the dossier says its
  integrity rests on the result hash alone. The trace file is written and the
  verification cross-reference matrix and evidence pack are built by the existing
  model-based systems engineering (MBSE) code (`mbse::trace`, `mbse::pack`); no second
  requirements engine was written. Outputs: `dossier.json` (with `dossier_sha256` over
  the canonical body; the `--utc` stamp is outside the hash), `dossier.html`
  (self-contained, no network, print stylesheet), `dossier.pdf` (A4, brand colours,
  cover, the MODELLED banner with "not a certification", tables with repeated headers,
  vector charts, page numbers), `runs/`, `trace.toml`, the evidence pack, `manifest.json`
  and a hash-chained `ledger.json` (`study_ledger`). A figure is only a scalar the
  result's own `units` block marks `provenance = "computed"`; inputs and constants are
  listed as inputs. `mission check <dir>` re-hashes every file and names what changed;
  `--rerun` runs every stored scenario again and compares. Needs the
  `pro-mission-dossier` feature (Enterprise tier, or granted by name); a Pro kind in a
  programme still needs its own tier. Worked example in `scenarios/pro-power/mission/`:
  five open-engine runs on public data (a low Earth orbit navigation chain, a lunar
  constellation, the L band under interference, a chained campaign, Mars orbit
  navigation) and one trade study; twelve requirements give 7 met (all 7 on a MODELLED
  or PARTNER basis), 4 not met and 1 not verified, with 15 open items. Checked against
  the free engine: with the open release binary all five result files and reports are
  byte-identical to the dossier's; with an unoptimised debug build of the open engine
  four are, and the low Earth orbit chain differs in 33 floating-point fields by a
  relative 4.8e-5 at most, with no verdict changed. Guide: `docs/MISSION-DOSSIER.md`.
  Tests: `tests/mission_dossier.rs` (every expected value is also read from a direct
  open-engine run) and unit tests in `src/mission/`. The README states twenty
  integration tests (was nineteen). **Two revisions of existing output.**
  (1) `kshana-pro study --dossier`: when a result's `units` block carries provenance,
  the default figures are only the `computed` fields, so for kinds other than `clock`
  echoed inputs and constants are no longer listed as figures (measured on four
  scenarios: lunar constellation 30 figures to 18, low Earth orbit chain 33 to 16,
  spectrum 25 to 7, Mars unchanged at 12). The clock worked example is unchanged: the
  dossier hash of `scenarios/dossier-holdover-csac.toml` is `d5e3acdc…9d3e` before and
  after, and its three files are byte-identical. (2) `kshana-pro report --pdf` and the
  Pack-A PDF: `report_pdf::Layout::fill_rect` drew the banner with `Op::DrawRectangle`,
  which `printpdf` 0.9.1 writes as a clipping path, so the banner background was never
  painted and everything after the banner on its page was clipped away; it now draws a
  filled polygon. The footer separator, a middle dot the built-in fonts showed as two
  wrong characters, is now a vertical bar. These PDFs now show their full first page;
  apart from that separator their text is unchanged.

- **Mission dossier and acceptance criteria: three refusals that protect a verdict.**
  (1) `kshana-pro mission build` refuses run identifiers that differ only in letter
  case, and any spelling of `TRADE` for a scenario. Each identifier becomes a directory
  under `runs/`; on a file system that ignores case (the default on macOS and Windows)
  `RUN-A` and `run-a` shared one directory, the second run overwrote the first one's
  files, and the build returned success with the first run's verdict read from the
  second run's result (measured on two Mars runs with seeds 7 and 99: a limit of 1.9 m
  that seed 7 meets at 1.8827 m was reported not met at 2.0928 m, and `mission check`
  then failed on its own dossier with 10 problems). (2) It refuses a scenario that
  holds a value that is not a finite number (`nan`, `inf`), naming the key, also inside
  a variant of a trade study: the engine takes such a value (measured on the open engine's
  `clock-holdover.toml`: the classical clock's holdover is 2610 s with the 20 ns
  threshold and reads 6600 s with `threshold_ns = nan`, which is never crossed), the
  result echoes it as `null`, and no sheet showed it. (3) **A revision of existing
  behaviour in `kshana-pro mbse`:** a criterion number must be finite. A literal too
  large for a double (`<= 1e999 m`) parsed to infinity and the criterion read as met
  for every value; it now does not parse, and the requirements file does not load.
  Tests: `run_ids_that_differ_only_in_letter_case_are_refused` and
  `values_that_are_not_finite_numbers_are_refused` in `tests/mission_dossier.rs`, and
  unit tests in `src/mission/programme.rs` and `src/mbse/criterion.rs`. Three more
  tests close gaps a mutation check found (the code was right, no test held it):
  `the_summary_counts_each_label_as_the_matrix_carries_it` (a PARTNER verdict counted as
  MODELLED went unnoticed), `markup_in_programme_text_is_escaped_in_the_html` (an
  unescaped page went unnoticed) and `a_ledger_from_another_dossier_fails_the_check`.
  **A changed number in `mission check`'s output:** the count of files re-hashed is
  now of distinct files. A file listed by both `dossier.json` and `manifest.json` was
  counted twice, so the worked example printed 65 for the 35 files it holds (30 of them
  listed twice); it now prints 35. Every comparison is still made.
