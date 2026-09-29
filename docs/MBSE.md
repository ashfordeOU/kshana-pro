# Model-based systems engineering (MBSE) and programme tooling

`kshana-pro mbse` turns the outputs of the open Kshana engine into systems-engineering
evidence. It reads requirements, checks each one against the result documents the
engine wrote, and produces a verification cross-reference matrix (VCRM), a Systems
Modeling Language version 2 (SysML v2) model, a change-impact report and an evidence
pack. It never re-runs or re-computes anything: every number it reports was read from
a file the open engine produced.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| CSV | comma-separated values (RFC 4180) |
| ECSS | European Cooperation for Space Standardization |
| GNSS | Global Navigation Satellite System |
| HTML | HyperText Markup Language |
| ISQ | International System of Quantities (the SysML v2 quantity library) |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| MBSE | model-based systems engineering |
| OMG | Object Management Group |
| PNT | positioning, navigation and timing |
| PPP | precise point positioning |
| RAIM | receiver autonomous integrity monitoring |
| ReqIF | Requirements Interchange Format (OMG) |
| RMS | root mean square |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SI | International System of Units |
| SISRE | signal-in-space range error |
| SysML v2 | Systems Modeling Language, version 2 (OMG) |
| TOML | Tom's Obvious Minimal Language (the scenario and trace file format) |
| VCRM | verification cross-reference matrix |

## What it reads from the open engine

This is the complete list. Nothing else is read.

| File the open engine writes | What is read |
|---|---|
| `<name>.result.json` (one per run) | any numeric or boolean field a criterion names, through the engine's own path grammar (`kshana::field_schema`); the `units` block (`units.<path>.unit`), used when the schema has no entry; the reproducibility stamp `scenario_hash`, `seed`, `engine_version`, `schema_version`; the run-level `label`; `figure_tiers.figures[].{path, requirement, tier}`; `kind` when there is no report |
| `<name>.report.json` (one per run, optional but recommended) | `kind`; `reproducibility.{engine_version, seed, scenario_sha256, result_scenario_hash, result_sha256, platform.os, platform.arch, command}`; `capabilities.rows[].{requirement, label, role, used_by}`; `capabilities.figure_tiers[]` when the result has no `figure_tiers` |
| `<name>.toml` (the scenario file the run used, optional) | its SHA-256, compared with the report's `scenario_sha256`; its `kind`; its tables named for clocks, receivers (`receiver`, `user`, `antenna`, `site`), constellations (`constellation`, `shell`, `satellite`, `system`, `gnss`), jammers and spoofers, with their scalar keys, for the SysML v2 parts; for change impact, every key and value |
| `docs/field-units-schema.json` (open repository) | `schema` (must be `kshana-field-units`); `kinds.<kind>.fields.<path>.{unit, provenance, evidence_tier}` |
| `web/data/verification-matrix.json` (open repository) | `rows[].{requirement, capability, status, module}`: the VALIDATED, MODELLED or PARTNER label of each capability |

Campaign results are result documents of the `campaign` kind and are read the same way,
for example `timeline.channels.time_error_ns.values[]` or `timeline.events[].t_s`.

## Quick start

```bash
# Every command takes --license <token>; a debug build also accepts --dev.
kshana-pro mbse vcrm tests/fixtures/mbse/trace.toml --dev
kshana-pro mbse pack tests/fixtures/mbse/trace.toml --out out/ --dev
```

The worked example in `tests/fixtures/mbse/` uses real runs of the open engine
(`kshana 0.28.0`) of four scenarios: clock holdover, RAIM integrity, a chained jamming,
spoofing and holdover campaign, and an end-to-end LEO PNT chain. How each file was made
is in `tests/fixtures/mbse/PROVENANCE.md`.

## 1. Requirements

Each requirement has an identifier, its text, a verification method (test, analysis,
inspection or demonstration, the four methods of ECSS-E-ST-10-02C) and an acceptance
criterion. It may also list the runs that verify it and pin the verification-matrix
capabilities its evidence rests on.

### CSV

A header row, then one requirement per row. Column names are matched without regard to
case:

| Column | Also accepted | Content |
|---|---|---|
| `id` | | identifier, unique |
| `text` | `requirement` | the requirement text |
| `method` | `verification`, `verification_method` | test, analysis, inspection or demonstration |
| `criterion` | `acceptance`, `acceptance_criterion` | the acceptance criterion (may be empty) |
| `runs` | `run` | run identifiers, separated by `;` (optional) |
| `capability` | `capabilities` | verification-matrix capability names, separated by `;` (optional) |

### ReqIF (the subset read)

Every `SPEC-OBJECT` becomes one requirement, in document order. Attribute values are
matched to their attribute definitions, and the definitions are recognised by
`LONG-NAME`, ignoring case, spaces, dots, hyphens and underscores:

| Field | Accepted `LONG-NAME` |
|---|---|
| id | `ReqIF.ForeignID`, `ID`, `Identifier`, `ReqID` (otherwise the `SPEC-OBJECT` identifier) |
| text | `ReqIF.Text`, `Text`, `Description` |
| method | `Verification Method`, `Method`, `Verification` |
| criterion | `Acceptance Criterion`, `Criterion`, `Acceptance` |
| runs | `Kshana Runs` |
| capabilities | `Kshana Capabilities` |

String, integer, real, boolean and date values are read from `THE-VALUE`; XHTML values
from the text inside `THE-VALUE`; enumeration values from the `LONG-NAME` of the
referenced enumeration value. Specification hierarchies, relations, relation groups and
tool extensions are not read. The reader accepts no internal document type definition
and expands no external entity.

The export (`import --to reqif`) writes a complete ReqIF 1.2 document in the element
order the ReqIF schema fixes. Every `LAST-CHANGE` and the header `CREATION-TIME` carry
the `--utc` stamp, or `1970-01-01T00:00:00Z` when none is given, so two exports of the
same requirements are identical.

### SysML v2 (the subset read)

Each `requirement def` becomes a requirement: its identifier is the short name (the
name in `<...>`), else the name; its text is its `doc` comment; its method, criterion
and pinned capabilities are the string attributes `verificationMethod`,
`acceptanceCriterion` and `kshanaCapabilities`. A `verification` usage whose objective
`verify`s a requirement usage links the run the verification usage is named after.
Anything outside the subset (listed under section 3) is refused with its line number.

## 2. Acceptance criteria

A criterion compares values read from a run's `result.json`:

```text
classical.fom.timing_p95_ns <= 20 ns
classical.fom.holdover_s >= 30 min
abs_max(quantum.series[].error_ns) <= 1 ns
classical.series.error_ns.max <= 50 ns
epochs.hpl_m < al_h_m for all epochs
fraction(epochs.available == true) >= 95 %
abs(stanford.points.error_m) < stanford.points.pl_m for all points
min(timeline.events[].t_s) <= 30 min
ppp.cases[1].median_convergence_min <= 10 min
quantum.filter_health.consistent == true
```

Grammar:

```text
criterion  := clause ( "and" clause )*
clause     := operand CMP operand [ "for" ("all" | "any") [NAME] ]
CMP        := "<" | "<=" | ">" | ">=" | "==" | "!="
operand    := NUMBER [UNIT] | "true" | "false" | PATH
            | FUNC "(" operand ")"
            | ("fraction" | "count") "(" operand CMP operand ")"
FUNC       := max | min | mean | rms | abs_max | p95 | sum | count | len
            | first | last | abs | fraction
PATH       := SEG ( "." SEG )*     SEG := NAME | NAME "[]" | NAME "[" INDEX "]"
```

- **Paths** use the open engine's path grammar: an array adds one segment and its `[]`
  is optional, so `epochs.hpl_m` and `epochs[].hpl_m` are the same column. A path whose
  last segment is an aggregate name and that does not exist as written is that
  aggregate over its parent: `classical.series.error_ns.max`.
- **Arrays** must be reduced with an aggregate or quantified with `for all` or
  `for any`. Comparing a whole array with a number without saying which is reported as
  not verified. `for all epochs` also checks that an operand really runs over `epochs`.
  Null elements are skipped and counted in the evidence.
- **`p95`** is the nearest-rank 95th percentile, the definition the engine's own
  `timing_p95_ns` uses. `fraction(...)` is dimensionless and `count(...)` is a count.
- **Units.** A number's unit is converted into the field's unit before comparing, so
  `30 min` against a field in `s` compares with 1800 s. A number with no unit is
  accepted only against a dimensionless or count field. Two fields are compared only
  when their units have the same dimension. The field's unit comes from the field-units
  schema, else from the result's own `units` block; the evidence names which. Units
  follow the engine's house spelling (SI symbols, `^` for powers, `*` and `/`,
  `count`, `1` for dimensionless, and the older dimensionless spellings). Logarithmic
  units (`dB`, `dBW`, `dB-Hz` and so on) compare only with the identical unit and are
  never scaled.

A criterion is **met**, **not met** or **not verified**. It is not verified when a
field is missing, has no unit or a unit that is not a unit, when the units do not match,
when an array is not reduced, or when the run's files fail the integrity checks below.
If one clause is false, the criterion is not met even when another clause could not be
checked.

## 3. The trace file and the VCRM

```toml
title = "Timing and integrity programme"
requirements = "requirements.csv"                          # .csv, .reqif / .xml, or .sysml
field_units_schema = "engine/field-units-schema.json"      # optional
verification_matrix = "engine/verification-matrix.json"    # optional

[[run]]
id = "RUN-CLOCK"
scenario = "runs/clock-holdover/clock-holdover.toml"       # optional
result = "runs/clock-holdover/clock-holdover.result.json"
report = "runs/clock-holdover/clock-holdover.report.json"  # optional

[[link]]                     # optional: a requirement's own runs column also links
requirement = "CLK-001"
runs = ["RUN-CLOCK"]

[[sign_off]]                 # optional: rows of the evidence pack's sign-off table
role = "Prepared by"
name = ""
```

Paths are relative to the trace file. `kshana-pro mbse trace` prints the links with each
run's stamp and verdict. `kshana-pro mbse vcrm` evaluates every criterion against every
linked run and prints one line per requirement and run (`--format md`, `csv` or
`json`). `--fail-on-open` exits with status 1 unless every requirement is met, for use
in continuous integration.

**Evidence** in each line: the run's `scenario_hash`, `seed`, `engine_version` and
`schema_version`, and for every field read, its path, the value compared, its unit and
where the unit was read. For an array it gives how many elements satisfied the
comparison and the first failing (or closest) element.

**Integrity checks.** When the report is present, the SHA-256 of the scenario file must
equal the report's `scenario_sha256` (so a scenario edited after the run is caught) and
the SHA-256 of the result file must equal the report's `result_sha256`. The result and
report must agree on the engine version. Any failure makes every verdict on that run
not verified and adds an open item.

**Basis.** Every verdict carries the label of the evidence it rests on: VALIDATED,
MODELLED or PARTNER. The label is the weakest of:

1. the capabilities the requirement pins, read from the verification matrix (else the
   run's report);
2. otherwise, per field read: the figure's tier in the result's `figure_tiers`; else the
   field's `evidence_tier` in the field-units schema (`modelled` counts as MODELLED,
   `validated` as VALIDATED, an echoed input neither raises nor lowers it); else the
   run's basis: a MODELLED run-level `label`, and the capabilities the run's report says
   its kind exercised (the reproducibility row every run shares is left out), plus any
   partner-owned capability the run relies on.

A verdict on a MODELLED or PARTNER basis is flagged next to the verdict and listed as an
open item. **Other open items:** a requirement with no traced run; a requirement with no
criterion (verify it by its method and record the result in the sign-off table); a
requirement whose method is test, inspection or demonstration (ECSS-E-ST-10-02C counts
simulation as analysis, so the method itself still needs its own record); every not met
and not verified line; a capability whose label in a run's report differs from the
programme's verification matrix.

On the worked example the matrix has 15 requirements: 12 met (9 of them on a MODELLED or
PARTNER basis), 2 not met and 1 not verified. The two not-met lines are real findings
in the engine's output: the RAIM run's horizontal protection level exceeds its 40 m
alert limit at 9 of 361 epochs, and in the campaign the clock time error exceeds its
50 ns guard at 183 of 458 timeline steps, from t = 2140 s in the holdover phase to
t = 3960 s at the end of the integrity-alarm phase.

## 4. SysML v2 export

`kshana-pro mbse sysml <trace.toml>` writes one package with:

- `Systems`: a `part def` per kind of system the scenarios simulate (`Clock`,
  `Receiver`, `Constellation`, `Jammer`, `Spoofer`), with an attribute per scenario key.
  A plain time, length, frequency or speed unit becomes an ISQ quantity
  (`ISQ::DurationValue`, `LengthValue`, `FrequencyValue`, `SpeedValue`) with values in
  coherent SI units (`[SI::s]`, `[SI::m]`, `[SI::Hz]`); other numbers are
  `ScalarValues::Real` (or `Integer` for a count). Each attribute's `doc` says its unit
  and where the unit was read: the field-units schema, or the key's unit suffix
  (`_km`, `_deg`, `_ns`, ...).
- `Scenarios`: a part per run holding the system instances it simulated, with their
  values.
- `RequirementDefinitions` and `Requirements`: a `requirement def` per requirement (text
  as `doc`, method and criterion as string attributes) and a usage of each.
- `Verification`: a `verification def KshanaRun` with the stamp attributes, and a
  verification per run whose objective `verify`s each linked requirement, with the
  verdict and basis as a `doc`.
- A `satisfy` relationship from each requirement to the run's system, only where the run
  met it. Where it did not, a comment says so.

Before writing, the export is run through the subset checker; `sysml --check <file>`
checks any file. The checker covers exactly these productions of the SysML v2 textual
notation: packages, imports, `part`, `requirement`, `verification` and `attribute`
definitions and usages, typing (`:`), specialisation (`:>`), redefinition (`:>>`),
attribute values (numbers with an optional unit in brackets, strings, booleans),
`subject`, `objective`, `verify`, `satisfy ... by ...`, `doc` and comments. It checks the
lexical form of names, strings and numbers, refuses a reserved word used as an unquoted
name, and resolves every name used for typing, satisfy, verify and units to an element
in the file or to one of the standard-library names the export uses.

Round trips: CSV to ReqIF and back, and CSV to SysML v2 and back, are lossless for the
requirement fields. A requirements-only SysML model (`import --to sysml`) has no verify
relationships, so run links are carried only by the full export.

## 5. Change impact

```bash
# Two traces: the same programme on two engine versions, or after a scenario change.
kshana-pro mbse impact --old old/trace.toml --new new/trace.toml
# Two scenario files, against a trace.
kshana-pro mbse impact trace.toml --scenario-old a.toml --scenario-new b.toml
```

With two traces, a requirement needs re-verification when a linked run's
`scenario_hash` changed (for a kind whose result carries none, its scenario SHA-256),
its `engine_version` or `seed` changed, a capability label its run rests on changed
between the two verification matrices or reports, a pinned capability's label changed,
or its criterion, method or links changed. Both matrices are evaluated and each
requirement shows its verdict before and after.

With two scenario files, the files are compared key by key after parsing, so comments
and layout do not count, and the changed keys are listed. Every requirement linked to a
run of the old file needs re-verification. The new `scenario_hash` exists only once the
engine has run the new file.

## 6. Evidence pack

`kshana-pro mbse pack <trace.toml> --out <dir> [--utc <iso>]` writes
`evidence-pack.html` and `evidence-pack.json`: the VCRM, a traceability graph
(requirements to runs, green met, red not met, amber not verified, dashed where the
basis is MODELLED or PARTNER), the reproducibility record of every run (files and their
SHA-256, the stamp and where each value was read, the integrity result), the open items,
a sign-off table and the SHA-256 of every input. The page loads nothing from the
network. `pack_sha256` is the SHA-256 of the pack's content, so the same inputs always
give the same hash; the `--utc` issue stamp is kept outside it.

## Licence

The commands need the `pro-mbse` feature, which the Enterprise tier carries (the same
feature the `pro-mbse` scenario pack uses; a licence can also grant it by name).
Without a licence the binary runs at Evaluation tier and refuses. `--dev` switches the
check off in debug builds only; a release build refuses it.

## Limitations

- The SysML v2 checker implements the subset above, transcribed from the OMG
  specification; it is not the OMG pilot implementation, and the export has not been
  loaded into a SysML v2 tool. The standard-library names it uses (`ScalarValues`,
  `ISQ`, `SI`) are checked against a fixed list, not against the library itself.
- The ReqIF export has not been validated against the ReqIF XML schema with a schema
  validator, nor imported into a requirements tool; the reader has been tested on its
  own export and on a hand-written document in the style of other tools' exports.
- Criteria compare values; there is no arithmetic between fields (a ratio of two fields
  cannot be written; `fraction(...)` covers the common case).
- The engine stamps `scenario_hash` only for some kinds. For the others (the integrity
  and LEO chain runs in the worked example) the stamp says so, and the report's scenario
  SHA-256 stands in for it in evidence and change impact.
- SysML parts come from scenario tables whose names denote a system; other tables are
  left out, and a unit given only by a key suffix is a naming convention, not a
  statement by the engine. The export says which applies to each attribute.
- Simulation evidence counts as analysis. For a test, inspection or demonstration
  requirement a met criterion supports the requirement but does not replace the test,
  inspection or demonstration; the pack lists it as an open item.
