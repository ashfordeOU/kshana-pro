# Incident reconstruction: log time integrity and dispute evidence

Research 2026-11-20 gaps, epic E06, part 1: items **B0603** (log time-integrity checker) and
**B0606** (commercial dispute evidence). Both hang under `kshana-pro forensics` and build on the
incident forensics bundle ([INCIDENT-FORENSICS.md](INCIDENT-FORENSICS.md)), the maritime
actor-vs-victim pack (W1119 C3) and the timing assurance work ([TIMING-ASSURANCE.md](TIMING-ASSURANCE.md):
the time-bomb step classifier and the clock self-check vote).

**Evidence label: MODELLED.** Both methods are exact arithmetic on the data supplied under
thresholds fixed before the run, tested on synthetic logs and scenarios with known truth only
(`tests/incident_evidence.rs`). They have not been validated against a labelled set of real
incidents. The dispute report is **not a legal finding**: it is an engineering record of when
integrity was lost and which sources agree, mapped to generic clause questions.

| Command | Feature | Tier |
|---|---|---|
| `forensics log-time <request.toml> [--json] [--timeline <csv>] [--out <dir> [--signer --key-file]]` | `pro-log-time-integrity` | Pro |
| `forensics log-time-synth <dir>` | none (synthetic demo) | free |
| `forensics dispute <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>] [--json]` | `pro-dispute-evidence` | Pro |
| `forensics dispute templates`, `forensics dispute-synth <dir>` | none | free |

Signed directories verify offline under any licence or none with `kshana-pro dossier verify <dir> --trust <key>`.

## 1. Log time-integrity checker (B0603)

Method `kshana-pro/log-time-integrity/v1`, report format `kshana-pro-log-time-integrity/1`.

Operational logs are only evidence when their time stamps can be trusted. A VDR or ECDIS that
followed a spoofed receiver, an aircraft clock "running backwards" in its CPDLC/ACARS traffic, a
SATCOM terminal or GMDSS console showing the wrong time, an FDM time base that stepped: each
breaks the timeline an investigation or a dispute rests on. The checker reads the logs, splits
each into its clocks, and checks every clock against the reference stamp each record carries,
against the other clocks that share that reference, and against itself.

### Request

```toml
title = "MV Example, 2026-06-01"
window_start_utc = "2026-06-01T08:00:00Z"   # optional: absolute-epoch check of logs without a reference
window_end_utc = "2026-06-01T08:30:00Z"
leap_dates = []                              # extra leap-second days (YYYY-MM-DD), after the engine's table

[params]          # all optional; written into every report
step_s = 0.8               # offset change between records that is a step
offset_limit_s = 1.0       # segment offset to the reference that is reported
frozen_n = 3               # equal consecutive stamps that make a frozen clock
drift_ppm = 20.0           # drift reported beyond this, when significant at 3 sigma ...
min_drift_span_s = 600.0   # ... over at least this span
gap_cadences = 5.0         # without a reference, a forward gap beyond this many cadences is noted
bin_s = 60.0               # cross-log bin
k = 3.0                    # cross-log vote multiplier (the clock self-check's vote)
floor_s = 0.1              # cross-log vote floor

[[logs]]
name = "vdr"
kind = "vdr"               # informative: vdr, ecdis, acars, cpdlc, fdm, satcom, gmdss
format = "nmea"            # csv | json | nmea | acars-json
path = "vdr.nmea"
u_s = 0.2                  # standard uncertainty of a stamp against its reference (cross-log vote)
split = true               # one clock per NMEA talker / aircraft / source column
offset_limit_s = 1.0       # optional per-log limit (e.g. a ground station's latency)
reference_group = "ship"   # optional: logs in one group are voted together (default: per log)
reference = "VDR recorder stamp, GNSS disciplined"   # informative
```

### Formats

* **CSV schema** (`csv`): a header naming `time` (required), `ref_time`, `valid` and `source`, in
  any order, separated by `,` or `;`; or no header and `time[,ref_time]`. Times are Unix seconds
  or ISO 8601 UTC (`YYYY-MM-DD[T ]HH:MM:SS[.fff][Z]`, second 60 allowed at 23:59). `time` is the
  clock under test; `ref_time` is an independent stamp of the same record (a GNSS-disciplined
  logger, a time server); `source` splits the file into clocks.
* **JSON schema** (`json`): an array, or JSON lines, of objects with the same keys; `time` and
  `ref_time` as numbers (Unix seconds) or strings.
* **NMEA 0183** (`nmea`): a VDR or ECDIS export. Each talker is a clock (`GP`, `GN`, `II`, ...):
  its ZDA sentences when it sends any, else RMC (two-digit year read as 20yy; RMC status `V`
  counts as invalid). Text before the `$` on a line is the recorder's stamp (Unix or ISO 8601),
  the reference. Checksums are verified; bad lines are counted, not used.
* **ACARS/CPDLC JSON lines** (`acars-json`), the shape acarsdec/dumpvdl2-style decoders write:
  `timestamp` is the ground station's receive time (the reference); the aircraft's message time
  is `msg_time` (or the key named by `time_field`), failing that a `/TS<hhmmss>,<ddmmyy>` field in
  `text`. One clock per aircraft (`tail`, else `flight`). Messages with no time of their own are
  skipped. Other message-time conventions go through the CSV/JSON schema.

FDM, SATCOM and GMDSS exports use the CSV/JSON schema.

### Checks

| Kind | Severity | Rule |
|---|---|---|
| `step`, `backward` | fault | With a reference: the offset (log − reference) changes by more than `step_s` between records (`backward` when the log time itself decreases). Without: the log time decreases. |
| `week-rollover`, `century`, `wrap-32`, `gps-utc-offset` | fault | A step, or a segment's constant offset, named by the time-bomb classifier: a whole number of 1024 GPS weeks, ~100 years, 2^31/2^32 s, 17–19 s (GPS time given as UTC). |
| `frozen` | fault | `frozen_n` or more equal stamps while the reference (or the record count) advances. The record after a frozen run is compared with the run's first record, so the catch-up is not a step. |
| `drift` | fault | The slope of the offset in a segment beyond `drift_ppm`, significant at 3 sigma, over at least `min_drift_span_s`. |
| `offset` | fault | A segment's median offset beyond the limit. |
| `false-leap` | fault | 23:59:60 on a day that ended without a leap second, or a ±1 s step at midnight after such a day. |
| `leap-step` | fault | A ±1 s step at midnight after a real leap-second day: one of the clock and its reference did not apply it. |
| `leap-second` | notice | 23:59:60 on a real leap-second day. |
| `absolute-epoch` | fault | No reference, and the log starts more than a day outside the window; named by the classifier. |
| `reference-backward` | notice | The reference stamp goes backwards (records out of order, or the reference stepped). |
| `forward-jump-or-gap` | notice | No reference, and a forward gap beyond `gap_cadences`: a data gap cannot be told from a forward step. |
| `invalid-flag` | notice | Records the device flagged invalid. |

**Cross-log disagreement.** Within each reference group (by default the clocks of one log, which
share its reference stamp), every clock's median offset per bin is voted with the clock self-check's
weighted vote (`kshana-pro/timing` selfcheck, W1120 TA2). A clock outside the consensus is named
(`source-outlier`); a consensus away from zero means the reference stamp itself is off
(`reference-off`); with two clocks a disagreement cannot be attributed (`pair-disagree`).

**Corrected timeline.** The log is cut into segments at each step. A segment with reference stamps
gets a least-squares offset model (offset and drift) that is removed; a segment without one is
bridged across the step at the log's cadence (a clock that stepped back 37 s is put back where it
would have run); a log with no reference anywhere takes its first segment as logged (`assumed`).
Each corrected time has a standard uncertainty: the fit residual combined with `u_s`, growing by
half a cadence per bridge. Frozen records take their reference stamp, or the cadence from the
start of the freeze. `--timeline` writes the table (`clock,index,line,log_utc,ref_utc,corrected_utc,u_s,offset_s,segment,trusted`).

**Integrity intervals.** Runs of records whose logged time cannot be trusted (offset beyond the
limit, frozen, a false leap, or a bridged segment) become intervals with start, end (corrected
times), duration and reason; cross-log disagreements are added. The dispute report reuses them.

**Output.** Human text by default, `--json` for the report; `--out <dir>` writes
`log-time-report.json`, `timeline.csv`, `request.toml`, `inputs/` and `README.txt`, signed when
`--signer` and `--key-file` are given. Exit 3 when the verdict is `TIME INTEGRITY FAULTS`.
Floats in the report are rounded to 9 significant digits; times are ISO 8601 with milliseconds.

## 2. Commercial dispute evidence (B0606)

Method `kshana-pro/dispute-evidence/v1`, report format `kshana-pro-dispute-evidence/1`.

Owners and charterers dispute GNSS-caused time: off-hire, delay against the laycan, DP downtime on
an offshore charter, a deviation, a master's refusal to sail into a spoofing area. The report
answers, for a time window: when was GNSS position or timing integrity lost, for how long (with
the bounds the evidence allows), what the cited evidence attributes it to, and which independent
sources agree; then maps that to clause questions, citing the pack and finding IDs.

### Request

```toml
title = "MV Example: off-hire claim"
case_ref = "C-123"                       # optional
vessel_id = "636000001"                  # optional
window_start_utc = "2026-06-01T08:00:00Z"
window_end_utc = "2026-06-01T08:30:00Z"
maritime = "maritime/request.toml"       # run the maritime actor-vs-victim pack here, or
maritime_pack = "pack-dir"               # cite a pack directory made earlier (signature checked)
forensics_bundle = "bundle-dir"          # cite an incident forensics bundle (signature checked)
log_time = "log-time/request.toml"       # run the log time-integrity check here
merge_gap_s = 60.0                       # spans closer than this merge into one period

[[status_log]]                           # REPORTED: CSV utc,source,state[,note]
name = "dp-log"
kind = "dp"                              # dp | ops
path = "dp-status.csv"

[[claimed]]                              # REPORTED: each party's claimed period
label = "Charterer off-hire"
party = "charterer"
start_utc = "2026-06-01T08:10:00Z"
end_utc = "2026-06-01T08:30:00Z"

[[clauses]]
template = "off-hire"                    # see `kshana-pro forensics dispute templates`
id = "Clause 17"
text = "..."                             # optional, carried as REPORTED, never interpreted
min_period_h = 0.1                       # off-hire: minimum continuous period
# laycan_end_utc = "..."                 # delay-laycan
# decision_utc = "..."                   # safe-port: time of the decision (e.g. refusal to sail)
# dp_sources = ["DP"]                    # dp-downtime: status sources that mean downtime
# claimed = ["Charterer off-hire"]       # which claimed periods the clause is about
```

Status states `ok`, `online`, `available`, `normal`, `in-use`, `selected` and `green` are up; any
other state runs until the source's next row with another state (or the window end). By default
DP downtime is every source of a `dp` status log whose name contains "DP".

### Spans, periods and sources

Each piece of evidence becomes spans (`spans.csv`):

| Source | Kinds | Cause | Cite |
|---|---|---|---|
| Maritime pack episodes (`vessel-gnss`) | `gnss-jamming`, `gnss-spoofing`, `ship-side-fault`, `ais-manipulation` (not counted as integrity loss) | from the pack's conclusion: `external-interference`, `localised-to-vessel`, `equipment`, `undetermined` | `KS:kshana-pro/maritime-actor-victim/v1#episode-<n>@<sha256 prefix>` |
| Forensics bundle findings (`forensics:<receiver>`) | `gnss-jamming`, `gnss-spoofing`, `gnss-degradation` | `external-interference` when corroborated, else `undetermined` | `KS:kshana-pro/incident-forensics/v1#<finding id>@...` |
| Log time-integrity intervals (`log-time:<clock>`) | `timing-integrity-lost` | `timing-fault` | `KS:kshana-pro/log-time-integrity/v1#<clock>/interval-<n>@...` |
| Status logs (`status:<log>/<source>`) | `status:<state>` (not counted as integrity loss) | `reported` | `REPORTED:<file>@<sha256 prefix>#row-<n>` |

Point times come from each span's start and end; outer bounds from the onset and end bounds of the
cited evidence (and the log check's uncertainty). Integrity spans are clipped to the window and
merged into **periods** with a point duration, an inner bound (the least the evidence supports) and
an outer bound (the most it allows). A period lists its sources and the independent corroborators
they name (AIS neighbours, a Station, other forensics receivers); it is `corroborated` when two or
more independent groups show it (each log of the time check counts once), else `single-source`.
Agreement is not proof of cause.

Every **claimed period** gets its overlap with lost integrity (point, inner, outer), with reported
DP downtime, the part not explained by the evidence and the fraction explained.

### Clause templates

| Template | Questions |
|---|---|
| `off-hire` | Was integrity lost (total)? What does the evidence attribute it to (external interference vs the ship's equipment vs undetermined)? Is the longest period at least `min_period_h`? How much of each claimed period coincides? |
| `delay-laycan` | Integrity loss before `laycan_end_utc`; coincidence with each claimed delay; was the interference area-wide (corroborated)? |
| `dp-downtime` | Reported DP downtime; the part coinciding with lost GNSS integrity and the remainder; which position references or DP sources were reported lost and for how long. |
| `deviation` | A spoofing episode (false own position) in the window? An AIS-versus-own-position discrepancy? Coincidence with the claimed deviation period. |
| `safe-port` | Was interference area-wide? How often did it recur? Was it present before `decision_utc`? (Reasonableness is not judged.) |

Every clause ends with the time-base question: can the logs' time stamps be relied on for the
durations (the log check's verdict and its uncertainty; `undetermined` without a log check).
Answers are `indicated`, `not-indicated`, `undetermined` or `value`, each with its basis and cites.

### Output

`--out <dir>` (signed with `--signer`/`--key-file`):

```text
dispute-evidence.json   the report (spans, periods, claims, clauses, cited evidence, limits)
spans.csv               every span with its cite
README.txt              the human summary
request.toml            the request as given
inputs/                 the status logs
cited/                  the cited pack or bundle report, byte for byte (made earlier)
maritime/               the maritime pack, when run here
log-time/               the log time-integrity report, when run here
signature.json          Ed25519 over every file (verify free: kshana-pro dossier verify)
```

A pack or bundle directory made earlier is checked with its own signature
(`--trust <key>` pins the issuer); the result is recorded as its provenance.

## 3. Tests

`tests/incident_evidence.rs` (synthetic, known truth): a log with no reference whose clock steps
back 37 s is detected and its corrected timeline is within 1 s of the truth (the B0603 acceptance
bar); a VDR receiver's GPS week rollover is named and out-voted by the two other talkers; frozen,
drift (200 ppm), GPS-as-UTC (18 s), an aircraft clock running backwards in CPDLC traffic, a whole
log one week era early, false and real leap seconds, determinism, the CLI with `--json` and its exit
code, licence refusal. The dispute report on a synthetic DP charter (area jamming with AIS
neighbours, a VDR rollover, DP status logs, two claims, five clause templates) with the pack IDs
cited; the signed directory verifies; a maritime pack directory made earlier is cited with its
signature checked. Unit tests in `src/forensics/logtime.rs` and `src/forensics/dispute.rs`.

## 4. Not built (E06 part 1)

* B0601, B0602, B0604 and B0605 (casualty and aviation reconstruction, sealed capture and chain of
  custody, jammer prosecution packs) are later parts of E06.
* ELD (road) gap adjudication from the B0606 sources: the templates are maritime; an ELD log can
  be checked with the CSV schema of the log check, but there is no ELD clause template yet.
* No validation on real logs or real disputes: every result is MODELLED.
