# Provenance of the MBSE fixtures

Every file under `runs/` and `engine/` was written by the open Kshana engine or its
repository generators. None was edited by hand. The tests in `tests/mbse.rs` check the
SHA-256 values below against the files, and check each result file against the
`result_sha256` its own report records.

## Engine

- Binary: `kshana 0.28.0` (the batch A+B+C integration build, 75 scenario kinds),
  SHA-256 `2a25a2e68e58207b6c8fb113ed3aa906056daadfd958ac3ee6e8c2d916eb16be`,
  platform macOS arm64. The build did not record a git commit (the engine's report
  says so in `reproducibility.git_commit_note`).
- Produced on 2026-09-29, one run per directory, each started in its own directory.

## Runs (`runs/<name>/`)

Each directory holds the scenario file and the two documents the engine wrote for it,
`<name>.result.json` and `<name>.report.json`. The HTML report and SVG chart the engine
also writes are not kept.

| Directory | Scenario source | Command | Scenario SHA-256 | Result SHA-256 |
|---|---|---|---|---|
| `clock-holdover` | `kshana example clock-holdover` | `kshana clock-holdover.toml` | `430c364d…7d52` | `ee5ebb0a…9483` |
| `clock-holdover-retuned` | `clock-holdover` with the classical clock's `q_wf` changed from `9.0e-20` to `1.6e-19` (σ_y(1 s) 3e-10 → 4e-10) and the first comment line saying so | `kshana clock-holdover-retuned.toml` | `eaf64ef5…515f` | `aa2f8305…8dea` |
| `integrity-raim` | `kshana example integrity-raim` | `kshana integrity-raim.toml` | `0bf9f03f…b4a3` | `0a35cad3…7353` |
| `campaign-jam-spoof-holdover-integrity` | `kshana example campaign-jam-spoof-holdover-integrity` | `kshana campaign-jam-spoof-holdover-integrity.toml` | `cec1e10f…ee5f` | `3ab00422…341a` |
| `leo-pnt-end-to-end` | `kshana example leo-pnt-end-to-end` | `kshana leo-pnt-end-to-end.toml` | `4a0286b5…ebce` | `f3545413…f788` |

Re-running `kshana clock-holdover.toml` with the same binary gave a byte-identical
result file (checked with `cmp` when the fixture was made).

## Engine artefacts (`engine/`)

| File | Source | SHA-256 |
|---|---|---|
| `field-units-schema.json` | `docs/field-units-schema.json` of the open repository at commit `da55ae630b16dc93dc81119627b2445396782e3d` (the documentation branch for the same 75-kind engine), written by `cargo test --test field_units_global zzz_emit_field_units_schema -- --ignored` | `8aa887e6dd7518407ebc982917de7705dceec58b45eefd701e0d96399068a5b7` |
| `verification-matrix.json` | `web/data/verification-matrix.json` at the same commit, written by `cargo run --bin gen_validation_artifacts` (223 rows: 83 VALIDATED, 136 MODELLED, 4 PARTNER) | `c17a81fa180eeed2c55067440e47b14441d935897c251ccea47a4580ec40a8ce` |
| `verification-matrix-v0.23.0.json` | `web/data/verification-matrix.json` at tag `v0.23.0` (commit `e6508d531c14d16fe10e516140bca37845b11ec3`); used for the capability-label change-impact test, because five rows moved from MODELLED to VALIDATED after it, among them "IEEE-1139 power-law clock noise + flicker-FM floor" | `a23f8e478457478a84d49b605607fcb9a2b0c14c2853c07abd1fa9b68e7541da` |

Every capability label in the five run reports matches `verification-matrix.json`
(checked when the fixture was made; the test `run_labels_match_the_matrix` keeps it so).

## Hand-written inputs

`requirements.csv` and the three `trace*.toml` files are the worked example's inputs,
written for these runs. `requirements.reqif` and `model.golden.sysml` are outputs of
`kshana-pro mbse import --to reqif` and `kshana-pro mbse sysml`, kept as golden files;
the tests regenerate them and compare (set `KSHANA_PRO_UPDATE_GOLDEN=1` to rewrite).
