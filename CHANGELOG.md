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
