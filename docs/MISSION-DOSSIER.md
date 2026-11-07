# Mission dossier: requirements to runs to verification matrix to a branded PDF

`kshana-pro mission build` takes one programme (a requirements file and a list of
scenarios of any kind) and delivers one dossier in one command: every scenario is run,
every requirement is checked against the real runs, and the result is a verification
matrix with its open items, one sheet per run, and a branded PDF in which every number
can be re-derived with the free open engine.

The open engine writes one report per run. The pieces that turn runs into programme
evidence already exist in Kshana Pro (`kshana-pro mbse`, see [MBSE.md](MBSE.md)), but
they check result files a user has already produced and traced by hand. This command
composes them: it runs the scenarios, writes the trace, builds the matrix and the
evidence pack with the same requirements tooling, and lays the whole out as the
document a programme hands over. It computes no result of its own.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| A4 | the 210 mm by 297 mm paper size of the International Organization for Standardization standard ISO 216 |
| ASCII | American Standard Code for Information Interchange |
| C/A | coarse/acquisition (the GPS L1 civil signal) |
| C/N0 | carrier-to-noise density ratio |
| CSV | comma-separated values (Request for Comments 4180) |
| ECSS | European Cooperation for Space Standardization |
| GNSS | Global Navigation Satellite System |
| GPS | Global Positioning System |
| HTML | HyperText Markup Language |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| MBSE | model-based systems engineering |
| PDF | Portable Document Format |
| PDOP | position dilution of precision |
| PNT | positioning, navigation and timing |
| PPP | precise point positioning |
| ReqIF | Requirements Interchange Format (Object Management Group) |
| RMS | root mean square |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SVG | Scalable Vector Graphics |
| SysML v2 | Systems Modeling Language, version 2 (Object Management Group) |
| TOML | Tom's Obvious Minimal Language (the scenario and programme file format) |
| UTC | Coordinated Universal Time |

## Quick start

```bash
# Every command takes --license <token>; a debug build also accepts --dev.
kshana-pro mission build scenarios/pro-power/mission/programme.toml --out out/ --utc 2026-09-30T00:00:00Z --dev
kshana-pro mission check out/ --rerun --dev
```

`build` prints the dossier hash and writes into `out/`:

| File | What it is |
|---|---|
| `dossier.pdf` | the dossier on A4: cover, MODELLED banner, verdict summary, verification matrix, one sheet per run with vector charts, trade study, open items, reproducibility record, inputs, sign-off table, limitations |
| `dossier.html` | the same content as one self-contained page: no network request, no script, a print stylesheet |
| `dossier.json` | the same content as data, with `dossier_sha256` |
| `runs/<id>/` | per run: the scenario file unchanged, `<name>.result.json`, `<name>.chart.svg`, and for an open kind `<name>.report.json` and `<name>.report.html` |
| `trace.toml` | the trace file the requirements tooling reads (`kshana-pro mbse vcrm out/trace.toml`) |
| the requirements file | a copy, under its own name |
| `evidence-pack.html`, `evidence-pack.json` | the evidence pack of `kshana-pro mbse pack`, with its traceability graph |
| `manifest.json` | every delivered file with its SHA-256 |
| `ledger.json` | a hash-chained record of the request, the runs and the delivery |

## The programme file

A TOML file. Paths are relative to it.

```toml
title = "Navigation resilience programme: Earth, Moon and Mars"
reference = "KP-MD-0001"            # optional
issue = "1"                         # optional, default "1"
classification = "Unclassified"     # optional, default "Unclassified"
requirements = "requirements.csv"   # .csv, .reqif / .xml, or .sysml

[[scenario]]
id = "RUN-LUNAR"                    # letters, digits, '-', '_' and '.'; becomes runs/RUN-LUNAR/
path = "lunar-relay-constellation.toml"
description = "A 14-satellite lunar relay constellation, one day"   # optional
figures = ["global.availability_pct", "global.pdop.median"]         # optional

[[scenario.chart]]                  # optional, repeatable
title = "Availability along four latitudes, by longitude"
x = "grid.lon_deg"
y = ["grid.availability_pct[0]", "grid.availability_pct[9]"]

[[link]]                            # optional: a requirement's own runs column also links
requirement = "LUN-001"
runs = ["RUN-LUNAR"]

[trade]                             # optional: one pro-trade-study scenario
path = "trade-lunar-relay.toml"
description = "Two lunar relay designs, ranked"                      # optional

[[sign_off]]                        # optional: rows of the sign-off table
role = "Prepared by"
name = ""
```

| Key | Meaning |
|---|---|
| `title`, `reference`, `issue`, `classification` | shown on the cover and in the footer of every page |
| `requirements` | the requirements file, read by the requirements tooling: CSV, ReqIF or SysML v2, in the subsets [MBSE.md](MBSE.md) describes. Each requirement has an identifier, a text, a verification method (test, analysis, inspection or demonstration), an acceptance criterion, and optionally the runs that verify it |
| `[[scenario]]` | one scenario of any kind the binary runs, open or Pro. `id` is the run identifier requirements link to |
| `figures` | the figures the run's sheet leads with. Each must be a scalar the result marks as computed (see below); naming an input is refused with the reason. Without it the sheet lists every computed scalar |
| `[[scenario.chart]]` | one chart: the `y` columns of the result against its `x` column. Paths follow the engine's path grammar (`timeline.t_s`, `epochs[].t_s`, `timeline.bands[3].cn0_effective_dbhz`, a row of a table as `grid.availability_pct[9]`). When `x` is a column of names the chart is drawn as bars, otherwise as lines. Every `y` must have as many values as `x` |
| `[[link]]` | further requirement-to-run links |
| `[trade]` | a `pro-trade-study` scenario; it runs as run `TRADE` and gets its own section |
| `[[sign_off]]` | the rows of the sign-off table; three empty rows when none is given |

The programme is checked before anything runs: a missing scenario file, a requirements
file with no requirements, a link to a requirement or a run that does not exist, a run
identifier that is not usable as a directory name, a requirements file named like one of
the files the build writes, and a Pro kind the licence does not carry are all refused,
and nothing is written. Three more refusals protect the verdicts themselves:

- **Run identifiers that differ only in letter case** (`RUN-A` and `run-a`), and any
  spelling of `TRADE` for a scenario. Each identifier becomes a directory, and a file
  system that ignores case (the default on macOS and Windows) would hold both runs in
  one directory, so one run would overwrite the other and a verdict would be read from
  the wrong run.
- **A scenario value that is not a finite number.** TOML allows `nan` and `inf` and the
  engine takes them: a threshold of `nan` is never crossed, so the run reports a figure
  no real threshold would give, and the result echoes the input as `null`. The refusal
  names the key (`user.altitude_km = inf`), also inside a variant of a trade study.
- **A criterion limit that is not finite** (`<= 1e999 m` reads as infinity and would
  hold for every value): the requirements file does not load.

A scenario that fails while it runs stops the build with the engine's message. Runs
that finished before it stay under `runs/`, and no dossier, manifest or ledger record
is written.

## What the command does

1. **Runs.** Each scenario's exact bytes are run by the embedded engine. An open kind
   runs unchanged; a Pro kind needs its entitlement. The files under `runs/<id>/` are
   the ones the open engine's own command line writes beside a scenario, report
   included, so the report's `scenario_sha256` and `result_sha256` are the hashes of the
   two files beside it. The open engine reports only its own kinds: a Pro kind is
   written without a report, and the dossier says that its integrity rests on the
   result hash alone.
2. **Matrix.** `trace.toml` is written and loaded, and the verification cross-reference
   matrix and the evidence pack are built by the requirements tooling. Criteria, units,
   integrity checks, basis labels and open items behave exactly as [MBSE.md](MBSE.md)
   describes; nothing is re-implemented.
3. **Dossier.** The body is composed and written as JSON, HTML and PDF.

### Figures

A figure is a number the run computed. The result document says which those are: every
entry of its `units` block carries a `provenance`, and only entries with
`provenance = "computed"` are figures. The sheet shows:

- **headline figures**: the computed scalars the programme names, else all of them;
- **outputs of another provenance class** (`modelled`, `derived`, `closed-form`,
  `internal-consistency`, `measured`), each with its class;
- **inputs and constants** the run echoes (`input`, `modelled-input`, `spec`,
  `published`, `constant`, `measured-or-input`), listed as inputs and never as figures;
- **computed columns**, summarised by count, minimum and maximum.

A value in an array of objects with at most 12 elements is listed with its index
(`timeline.bands[3].min_cn0_dbhz`) and the element's name. A numeric field with no units
entry is counted and not shown: without a provenance nothing says whether it is a figure.

### Labels

| Label or word | Meaning in the dossier |
|---|---|
| VALIDATED | the open engine's verification matrix records the capability as checked against an external oracle (a published dataset, an independent tool, a standard's test vectors) |
| MODELLED | the capability is a documented model that is not externally validated; the engine's run-level label and its not-modelled list say what it leaves out |
| PARTNER | the run relies on a discipline the engine does not provide (hardware or a partner's model); the number is conditional on it |
| basis | the weakest label a verdict rests on. A verdict on a MODELLED or PARTNER basis is flagged next to the verdict and is an open item |
| met | the criterion holds on every linked run |
| not met | the criterion fails on at least one linked run |
| not verified | a field is missing, a unit does not match, an array is not reduced, the run's files fail an integrity check, or no run is linked. A wrong unit is never read as met |
| open item | something a reviewer must close before sign-off, exactly as the requirements tooling lists it: a not-met or not-verified line, a verdict on a MODELLED or PARTNER basis, a requirement with no run or no criterion, and a requirement whose method is test, inspection or demonstration even when its criterion is met |

A label grades the capability as the engine's verification matrix records it. It does
not grade a programme's scenario inputs: a VALIDATED capability does not make an
illustrative input measured.

## The worked example

`scenarios/pro-power/mission/` holds one programme over five runs of the open engine on
public data, plus one trade study. The five scenario files are the engine's own
examples, copied unchanged ([PROVENANCE.md](../scenarios/pro-power/mission/PROVENANCE.md)
gives the engine commit and each file's SHA-256). The twelve requirements and their
limits are illustrative: they were written for this example and are no agency's.

| Run | Scenario (kind) | What it is |
|---|---|---|
| `RUN-LEO` | `leo-pnt-end-to-end.toml` (`leo-pnt-chain`) | a LEO PNT chain on a generic 1080 km constellation: signal, pass, navigation message, fused fix, PPP |
| `RUN-LUNAR` | `lunar-relay-constellation.toml` (`constellation-design`) | a 14-satellite lunar relay constellation, scored over the whole Moon for one day |
| `RUN-SPECTRUM` | `l-band-waterfall-jamming.toml` (`spectrum`) | the L band under a scripted sequence of three jammers |
| `RUN-CAMPAIGN` | `campaign-jam-spoof-holdover-integrity.toml` (`campaign`) | a chained mission: nominal, jamming, spoofing, holdover, integrity alarm, recovery |
| `RUN-MARS` | `mars-orbit-pnt.toml` (`body-pnt`) | a Mars orbiter positioned by twelve navigation satellites and a range from Earth, over one sol |
| `TRADE` | `trade-lunar-relay.toml` (`pro-trade-study`) | two lunar relay designs ranked by availability |

The verdicts are real findings in the engine's output. Each value below is read from the
run's result file, and `tests/mission_dossier.rs` compares each with a direct run of the
open engine:

| Requirement | Criterion | Value read | Verdict |
|---|---|---|---|
| LEO-001 | `ppp.cases[1].median_convergence_min <= 10 min` (240 LEO satellites) | 7.5 min | met |
| LEO-002 | `ppp.cases[0].median_convergence_min <= 10 min` (GPS and Galileo alone) | 11.5 min | not met |
| LEO-003 | `fusion.fused.rms_error_3d_m <= 0.5 m`, method test | 0.434888 m | met, and an open item: the test itself still needs its own record |
| LUN-001 | `global.availability_pct >= 95 %` | 21.3286 % | not met |
| LUN-002 | `global.pdop.median <= 6` | 5.22 | met |
| SPC-001 | `timeline.bands[3].min_cn0_dbhz >= 25 dB-Hz` (GPS L5) | 44.0752 dB-Hz | met |
| SPC-002 | `timeline.bands[0].min_cn0_dbhz >= 25 dB-Hz` (GPS L1 C/A) | 3.23286 dB-Hz | not met |
| CMP-001 | `timeline.channels.time_error_ns.values <= timeline.channels.guard_ns.values for all values` | the 50 ns guard is exceeded at 183 of 458 timeline steps; the first is 51.7825 ns | not met |
| CMP-002 | `min(timeline.events[].t_s) <= 30 min`, method demonstration | 1200 s | met, and an open item for the method |
| MARS-001 | `fom.rms_error_relays_m <= 5 m` | 1.8827 m | met |
| MARS-002 | `fom.availability_relays >= 99 %` | 1 (every epoch) | met |
| OPS-001 | none, method inspection, no run | | not verified |

Twelve requirements: 7 met, 4 not met, 1 not verified. Of the 11 verdicts (one per
requirement and linked run), none rests on a VALIDATED basis, 7 rest on a MODELLED basis
and 4 on a PARTNER basis, so all 7 met requirements are flagged, and the dossier lists
15 open items. The trade study ranks the two lunar designs by the same availability
figure: 21.3286 % for the 14-satellite design of `RUN-LUNAR` and 60.629 % for the
22-satellite design with a second, north-lingering elliptical shell. Neither reaches the
95 % of LUN-001, and the dossier shows that.

## How a recipient checks a dossier

With Kshana Pro:

```bash
kshana-pro mission check out/            # re-hash every file
kshana-pro mission check out/ --rerun    # and run every stored scenario again
```

`check` exits with status 0 when everything agrees and 1 otherwise, naming each file or
run it failed on. It checks that:

1. the body of `dossier.json` hashes to `dossier_sha256` (the SHA-256 of the body as
   canonical JSON: object keys sorted, no insignificant white space; the `--utc` issue
   stamp is outside the body, so the same inputs always give the same hash);
2. every file the body lists (the trace, the requirements copy, everything under
   `runs/`) hashes to the value the body records;
3. every file `manifest.json` lists, the three faces of the dossier and the evidence
   pack included, hashes to the value the manifest records;
4. `ledger.json` verifies and its last record carries the dossier hash;
5. with `--rerun`, each stored scenario, run again, gives a result with the recorded
   SHA-256. When it does not, the fields that differ are listed with both values.

With the free open engine only, for any run of an open kind, the reproducibility record
gives the command and the hashes:

```bash
mkdir check && cp out/runs/RUN-MARS/mars-orbit-pnt.toml check/ && cd check
kshana mars-orbit-pnt.toml
shasum -a 256 mars-orbit-pnt.result.json     # compare with the dossier's Result SHA-256
```

This was run for the five open runs of the worked example
(`tests/mission_dossier.rs::the_free_engine_reproduces_every_open_run`, on macOS arm64,
open engine 0.28.0 at the commit PROVENANCE.md names, and again for Kshana Pro 0.2.0
with open engine 0.29.0, the release tree `38feb76`):

- with a release build of the open engine, all five result files, and the five
  `report.json` files, are byte-identical to the dossier's (on both engines);
- with an unoptimised debug build of the open engine, four of the five are
  byte-identical. The LEO chain differs in 33 floating-point fields, by a relative
  4.8e-5 at most (the largest is `navmsg.sisre_orbit_rms_m`, 5.2634e-6 m against
  5.2637e-6 m). One value a verdict reads moves, in its tenth digit (the RMS error of
  LEO-003, 0.43488783762 m against 0.43488783767 m), and no verdict changes (measured
  on engine 0.28.0).

So the dossier states byte identity for a release build of the engine on the same
platform, and says that another platform or an unoptimised build can give slightly
different floating-point values.

The ledger has three records per build, each chained to the one before by its hash:
the request (the SHA-256 over the programme, the requirements and every scenario), the
runs (a digest of every result hash) and the delivery (the dossier hash and the `--utc`
stamp). Building again into the same directory appends three more; a ledger that does
not verify is refused.

## Licence

The commands need the `pro-mission-dossier` feature, which the Enterprise tier carries;
a licence can also grant it by name. A Pro kind inside a programme still needs its own
entitlement: the worked example's trade study needs the Pro tier. Without a licence the
binary runs at Evaluation tier and refuses. `--dev` switches the check off in debug
builds only; a release build refuses it. For access, write to contact@ashforde.org.

## Limitations

- Simulation evidence counts as analysis (ECSS-E-ST-10-02C). For a test, inspection or
  demonstration requirement a met criterion supports the requirement but does not
  replace the test, inspection or demonstration; the dossier lists it as an open item.
- The PDF is identical between two builds except for the trailer identifier (`/ID`),
  which the PDF library fills with random characters. `dossier.json` and `dossier.html`
  are byte-identical between two builds with the same `--utc` stamp.
- Charts are drawn only for the paths the programme names. The chart the engine drew
  for each run is kept beside its result and embedded in the HTML page. A column longer
  than 2000 samples is thinned by a constant stride, and the chart says so.
- A dossier whose text is ASCII uses the PDF built-in fonts; typographic characters in
  the engine's notes are written out (a Greek sigma as "sigma"). A dossier with text
  those fonts cannot show (a translated title, requirements in Greek or Cyrillic,
  accented Latin) is set throughout in Noto Sans, embedded (OFL 1.1,
  `assets/fonts/`), with a Unicode map so the text can be searched and copied; the
  footer's honesty line stays in a built-in font. Noto Sans covers Latin, Greek and
  Cyrillic; other scripts (Chinese, Japanese, Korean, Arabic, Hebrew, Indic) still
  become `?` in the PDF. The HTML and JSON keep all text as it is.
- A scenario is run from its own text. A data file a scenario names by relative path is
  not copied into the dossier.
- The dossier hash, the manifest and the ledger are not signed: they show that the
  files agree with each other, not who issued them. Pass the dossier hash to the
  recipient by a separate route when origin matters.
- A dossier produced by a debug build says so ("development build"). A dossier for
  delivery is produced by a release build.
- The trade section shows what the `pro-trade-study` pack returns (ranking, winner, its
  own assurance gate); the free engine does not reproduce the trade itself, only each
  variant's scenario.
- The limits of the requirements tooling apply unchanged: see the Limitations of
  [MBSE.md](MBSE.md).
- Nothing here is a certification, a qualification or an acceptance by any authority.
