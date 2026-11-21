# Independent verification evidence: requirements coverage and verification status

Module `src/isvv.rs`, command `kshana-pro isvv` (`src/cli/isvv_cmd.rs`), tests in
`tests/avionics.rs` (module `isvv`). Feature `pro-isvv-evidence` (Pro).

**What already existed.** `kshana-pro avionics evidence` (Enterprise) maps an applicant's
requirements to the built-in avionics procedures and their signed verdicts. It covers only those
procedures. This command is the generic version for any project's own tests.

## Inputs

`request.toml`:

```toml
title = "Unit software, release 2.3"
requirements = "requirements.csv"   # id[,text][,method][,tests]
trace = "trace.csv"                 # requirement,test   (optional with a tests column)
results = ["junit-unit.xml", "junit-integration.xml", "manual.csv"]
baseline = "../release-2.2-pack"    # optional: a previous signed pack, or its report.json
```

* **Requirements**: CSV with an `id` column. `text`, `method` and `tests` are optional. Quoted
  fields may hold commas. A `method` other than test (`A`, `I`, `R`, or analysis, inspection,
  review) marks a requirement verified another way. Such a requirement is listed but not
  scored, unless tests are traced to it. `tests` holds trace patterns separated by `;`.
* **Trace map**: CSV `requirement,test`. A test pattern is a test name and may use `*`
  (`log.Commands.*`).
* **Results**: JUnit XML (`testsuite`/`testcase`). A test is named `classname.name`, or
  `suite.name` when there is no classname. A `failure` child marks it failed, `error` marks it
  errored and `skipped` marks it skipped. Comments and CDATA are ignored. CSV
  `test,outcome[,message]` works for manual or other runners. When a test appears more than
  once, the worst outcome stands.

## Output

`kshana-pro isvv run request.toml --out <dir> --signer <name> --key-file <file> [--trust <hex>]`
writes and signs `report.json` (`kshana-pro-isvv-coverage/1`), `report.html` and every input with
its SHA-256 (`inputs/`).

| status | meaning |
|---|---|
| `passed` | every trace pattern matched a test in the results; none failed or was skipped |
| `failed` | at least one traced test failed or errored |
| `partial` | some traced tests passed; others were not found or were skipped |
| `not-run` | traced, but no traced test passed (not found, or only skipped) |
| `untraced` | verified by test, but no trace entry names it (an untested requirement) |
| `other-method` | verified by analysis, inspection or review, with no test traced |

The report also lists **orphan tests** (results that no trace entry claims) and **trace
errors** (entries naming a requirement that is not in the list). With a baseline it gives the
**change since the baseline**: requirement status changes, new and removed requirements, test
regressions (was not failing, now fails), fixes, and new and removed tests. A baseline given as
a signed pack directory has its signature checked against `--trust`, or against any key when
none is given. A failure is reported in the delta and does not stop the run.

`kshana-pro isvv synth <dir>` writes SYNTHETIC records and two requests: a baseline run and a
current run against it.

MODELLED: a tally of the supplied records. No test is re-run. Whether a requirement is met is
for the project's verification authority to decide. No project's requirement text is shipped.
