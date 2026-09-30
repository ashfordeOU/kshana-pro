# Campaign watch: regression and scenario subscription for campaigns and statistics

A free Kshana user can re-run a campaign and compare the numbers by eye. Campaign watch
turns a scenario pack into something that keeps proving itself: a Pro user subscribes a
set of scenarios and campaigns to a watch, and `kshana-pro ci` fails the pipeline when a
figure that matters gets worse or a requirement stops being met. It tells noise from
change for Monte Carlo figures using the ensemble's own statistics, and it keeps a
tamper-evident history of every run.

Campaign watch extends the existing scenario subscription (`kshana-pro ci`, the Pack C
module). A pack without `[[watch]]` tables behaves exactly as before: same pack hash, same
report, byte for byte. Every number comes from an open-engine run through
`kshana_pro::registry::run`; the watch reads result documents, it never recomputes them.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| C/A | coarse/acquisition (the GPS L1 civil code) |
| C/N0 | carrier-to-noise density ratio |
| CI | continuous integration |
| GNSS | Global Navigation Satellite System |
| GPS | Global Positioning System |
| HDOP | horizontal dilution of precision |
| HTML | HyperText Markup Language |
| J/S | jammer-to-signal power ratio |
| JSON | JavaScript Object Notation |
| JSONL | JSON Lines (one JSON document per line) |
| LEO | low Earth orbit |
| PDOP | position dilution of precision |
| PNT | positioning, navigation and timing |
| PPP | precise point positioning |
| RMS | root mean square |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SISRE | signal-in-space range error |
| SPS PS | GPS Standard Positioning Service Performance Standard |
| SVG | Scalable Vector Graphics |
| TOML | Tom's Obvious Minimal Language (the pack and scenario file format) |

## Quick start

```bash
# First run: the baseline. Requirements are checked even without a baseline.
kshana-pro ci --pack scenarios/packs/campaign-baseline/pack.toml \
  --json base.json --history history.jsonl --trend trend.html --license <token>

# Every later run: gate against the last passing report, append to the history.
kshana-pro ci --pack scenarios/packs/campaign-baseline/pack.toml --baseline base.json \
  --json ci.json --out report.html --history history.jsonl --trend trend.html \
  --license <token>
echo $?            # 0: every watch holds; 1: a regression, an unmet requirement,
                   # or a watched figure that could not be read

kshana-pro ci history verify history.jsonl
kshana-pro ci diff base.json ci.json
```

In a debug build `--dev` replaces `--license` (the development entitlement path; a
release build refuses it).

## The manifest: `[[watch]]`

A watch names one figure of one entry and says what worse means for it.

```toml
[[entries]]
path = "campaign-jam-spoof-holdover-integrity.toml"
label = "mission"

[[watch]]
name = "time error at the end of holdover"         # optional; the watch's identity
entry = "mission"                                   # an entry label of this pack
metric = "campaign.phase.holdover.end.time_error_ns"
direction = "lower"                                 # higher | lower | target
tolerance = { rel = 0.02 }                          # rel and/or abs
requirement = "watched >= 36.4 ns and watched <= 62.7 ns"
note = "docs/CAMPAIGNS.md: 36.4 to 62.7 ns over the holdover phase."
```

| Key | Meaning |
|---|---|
| `name` | Display name and identity across runs. Without it the identity is `entry::metric`. Two watches may not share one. |
| `entry` | The label of the entry whose result is read. An unknown label is refused when the manifest is parsed. |
| `metric` | The figure: a result path in the grammar of the MBSE criteria (`docs/MBSE.md`, section 2), so `timeline.bands[0].min_cn0_dbhz`, `global.availability_pct` and aggregates such as `max(timeline.channels.time_error_ns.values[])` all work; or a named campaign figure (below); or, with `crossing`, the name of a sweep metric. |
| `direction` | `higher` or `lower` is better, or `target` (closer to `target` is better). Without it the Pack C name table decides (for example `availability` is higher-is-better, a leaf ending in `_m` or `_ns` lower-is-better). The unit never decides. With no direction at all the watch is informational: it reports any change and never regresses. |
| `target` | The target value for `direction = "target"`, in the figure's unit. |
| `tolerance` | `rel` (a fraction of the baseline magnitude) and/or `abs` (in the figure's unit). The allowed worsening is the larger of the two. Without it the run's default relative threshold applies (`--thresholds`, 5 % by default). Each must be finite and not negative: `nan` is refused. |
| `requirement` | An absolute acceptance criterion in the MBSE criterion grammar, evaluated against the entry's result with units checked. The word `watched` names the watched figure in its own unit. It is checked on every run, with or without a baseline. |
| `ensemble` | `{ mean = <path>, std = <path>, n = <path>, test = "mean" \| "spread" \| "both" }`: the Monte Carlo statistics of the figure. Such a watch is decided by the statistical gate, not by a tolerance (setting both is refused). |
| `alpha` | The significance level of the statistical gate, between 0 and 0.5; default 0.01. |
| `crossing` | `{ below = x }` or `{ above = x }`: watch the first value of the sweep's first axis at which `metric` crosses `x`. |
| `note` | Free text: where a requirement comes from. |

The pack hash covers the watch tables (one canonical line each, after the entries) only
when there are any, so existing baselines of packs without watches stay valid, and
editing a watch changes the hash of a watched pack.

### What counts as a figure

A watched pack collects figures, not every number. A numeric leaf is a figure when its
entry in the result's own `units` block has provenance `computed`, `modelled` or
`derived`, the engine's classes for a number the run produced. Excluded are inputs and
fixed references (`input`, `modelled-input`, `spec`, `published`, `constant`,
`measured`, `measured-or-input`), algebraic restatements of inputs and self-checks
(`closed-form`, `internal-consistency`), and computed leaves that only describe the
output's own layout: a leaf starting `n_` or `source_n_`, `nfft`, `bin_width_hz`,
`row_duration_s`, `bins_compared`, `segments`, anything under `reproducibility`,
`sigmf`, `coords`, `axes` or `shape`, and any key containing `:`. Each figure carries the
unit its entry states. Arrays longer than eight elements are not flattened; read a long
series through a watch aggregate or a campaign figure. A result with no `units` block
falls back to every numeric leaf, and the report lists that entry.

`modelled` is a figure class because the engine uses it for the output of a documented
model: the LEO chain's `navmsg.sisre_rms_m` and fused position errors carry it.

On `l-band-waterfall-jamming` the legacy extractor collects 199 metrics; the figure set
has 112. The receiver noise figure and the waterfall grid size are no longer among them.

In a watched pack only the watches decide the verdict. Every other figure is carried in
the report and in `ci diff`, and never fails the run.

### Named campaign figures

For a `kind = "campaign"` result, these names resolve under `campaign.`:

| Figure | Unit | What it is |
|---|---|---|
| `phase.<p>.t0_s`, `.t1_s`, `.duration_s` | s | the phase window on the mission timeline |
| `phase.<p>.end.<channel>` | the channel's | the channel's value at the last grid time the phase owns |
| `phase.<p>.carried.<channel>` | the channel's | the value carried into the phase from the end of the previous one |
| `first_alarm_s` | s | the first grid time at which the alarm channel is 1 (absent when it never alarms) |
| `alarm_share` | 1 | the share of grid times with an alarm value at which the alarm is up |
| `event.e<k>.t_s` | s | the time of the k-th timeline event, from 1 |
| `handoff.h<k>.value` | the handoff's | the k-th number handed between phases, from 1 |
| `mc.<m>.<stat>` | the metric's | a Monte Carlo metric's `mean`, `std`, `n`, `p05`, `p50`, `p95`, `ci95_low`, `ci95_high` |
| `sweep.<m>.min`, `.max` | the metric's | the smallest and largest node value of a sweep metric |

Phase and metric names have every character other than a letter, digit or `_` replaced
by `_`: phase `integrity-alarm` is `campaign.phase.integrity_alarm`. The engine gives grid
time `t` to the phase whose window `[t0, t1)` holds it (the last phase also owns its
end), so the end value of a phase is its value at the last grid time before `t1`. A null
value there makes the figure absent, never guessed. `carried` is the engine's own end
value of the previous phase, read at that run's last sample; it can differ from the
previous phase's last grid value by up to one grid step of drift.

On the bundled mission these read exactly the stand-alone numbers (tested): the spoofing
phase lasts the spoof run's detection time, 370 s; the jamming phase ends at the mean
C/N0 of the stand-alone jamming run's satellites at its latest epoch, 27.7 dB-Hz; the
time error at the end of holdover is the value the campaign's timeline holds at 3360 s.

A sweep crossing is read along the first axis, at the first value of every other axis,
in node order. A metric that never crosses makes the watch *not found*.

## How the statistical gate decides

A relative tolerance is the wrong test for a Monte Carlo mean. The final time error of a
free-running chip-scale atomic clock over 200 seeds has a mean near zero; the next 200
seeds move that mean by 156 % while nothing changed. The gate compares the two ensembles
as samples instead, with the test and the significance level fixed in the manifest before
the first run.

- **Mean** (`test = "mean"` or `"both"`): a two-sample Welch t-test, two-sided, on the two
  means. A regression only when p is below alpha **and** the current mean is worse in the
  watch's direction. A significant change in the better direction is an improvement.
- **Spread** (`test = "spread"` or `"both"`): a one-sided F-test on the ratio of the
  current variance to the baseline variance. A regression when p is below alpha, that is
  when the spread grew beyond what two samples of one distribution give at that level.

The engine reports the population standard deviation; the tests use the sample variance
`std² n / (n - 1)`. Both p-values, the t and F statistics and their degrees of freedom
are in the report. The distributions come from the regularised incomplete beta function,
checked in unit tests against reference values.

**What alpha means.** Alpha is the probability of calling a change when there is none. At
0.01, one watched ensemble in a hundred re-run with fresh seeds is expected to trip a
test by chance. A smaller alpha raises fewer false alarms and needs a larger change to
react.

Measured on `campaign-monte-carlo-clock-holdover` (200 seeds from 42; the open document
gives the interval on the mean as [-2.52, 1.59] ns and the standard deviation as
14.93 ns):

| Run | Mean | Std | Welch p | F | F p | Verdict |
|---|---|---|---|---|---|---|
| base seed 1042, same physics | -1.16 ns (was -0.45) | 15.79 ns | 0.646 | 1.12 | 0.215 | not a regression |
| `q_wf` times four (closed-form std 16.459 to 32.918 ns) | | 29.85 ns | | 4.00 | 2.4e-21 | spread regressed |

The baseline report keeps each watch's ensemble statistics, so any report can be the next
baseline.

## History, trend and diff

- `--history <file>` appends one record per run to an append-only, hash-chained JSONL
  file: index, the caller's time stamp (`--utc`), engine version, pack identity and hash,
  a SHA-256 of the metrics, the verdict, the failed watches, every watched figure with its
  status and requirement limits, each member result's SHA-256, the previous hash and the
  record's own hash. Each line stores the exact bytes that were hashed, so verification
  re-hashes what is written. The file is verified before a run, and a run refuses to
  append to a broken history or to the history of another pack (one history file follows
  one pack).
- `kshana-pro ci history verify <file>` exits 0 and prints the record count and last
  hash, or exits 1 and names the first record that fails and why (a changed byte, a
  broken link, a wrong index). An empty file is a valid, empty history.
- `--trend <out.html>` (needs `--history`) writes a self-contained HTML page with one
  inline SVG sparkline per watched figure, one point per run, requirement limits as
  dashed lines, the points of failed runs in red, and a dotted line where the engine
  version changed; below it, one row per run, a failed run's row in red. The same data is
  written as JSON next to it (`out.json`).
- `--exact` records each member result's SHA-256 and reports every entry whose result
  changed against the baseline. A changed hash is reported, not failed; only a watch
  fails a run.
- `kshana-pro ci diff <old.json> <new.json> [--json <out>]` lists every figure that moved
  (old, new, change, relative change, unit, whether the member result's SHA-256 changed)
  and the watched figures that moved, and counts the rest as unchanged.

## Reviewing an engine upgrade

1. Run the pack with the current engine: `kshana-pro ci --pack pack.toml --exact --json old.json`.
2. Build Pro against the new engine and run the same pack:
   `kshana-pro ci --pack pack.toml --exact --baseline old.json --json new.json --history history.jsonl`.
3. `kshana-pro ci diff old.json new.json` lists exactly which figures moved and which
   member results changed. The watches say whether any move matters; `--exact` says
   whether anything moved at all. The history marks the engine-version boundary on the
   trend.

A published figure that moves is a revision to record, not a silent change.

## The five bundled packs

Each pack is under `scenarios/packs/<id>/` with its scenario files, `pack.toml` and a
`PROVENANCE.md` giving the open-engine commit and the SHA-256 of every file. Each runs
with exit code 0 on its first run and writes a byte-identical `ci.json` on a second run
with the same `--utc` (tested). Requirements are the figures the open documentation
states.

| Pack | Entries | Watches (requirement from the open documentation) |
|---|---|---|
| `leo-pnt-baseline` | the generic 1080 km LEO PNT chain; the Xona Pulsar chain (arXiv 2509.19551) | SISRE handed on (below 0.2505 m: the document states 0.250 m); fused GNSS + LEO 3D position error (better than GNSS alone); tracked C/N0 peak; median code jitter; PPP convergence (faster with the LEO layer) |
| `constellation-baseline` | the GPS baseline constellation under SPS PS Appendix B conditions; four-system coverage; a lunar relay constellation | GPS availability (at least 98 %) and worst-site availability (at least 88 %), SPS PS Table 3.8-1; HDOP median, 95th percentile and mean (0.94, 1.25, 0.96, bar 0.03); fewest satellites in view; PDOP 95th percentile; lunar availability |
| `spectrum-baseline` | the L-band waterfall jamming example; the multi-band waterfall | per-band minimum C/N0, share of rows tracking (L2C 100 %), first loss of lock, L5 above the 25 dB-Hz threshold, worst in-band J/S |
| `campaign-baseline` | the chained jamming, spoofing, holdover and integrity mission; the 200-seed clock ensemble; the jammer-power sweep | spoofing detection (at most 370 s); C/N0 at the end of jamming (at least 25 dB-Hz); satellites tracking; time error carried into and at the end of holdover (36.4 to 62.7 ns); largest time error; share under alarm; the ensemble mean (Welch test; interval contains 0) and spread (F-test; chi-square 99 % band 14.32 to 18.55 ns); the jammer power at which availability falls below one half (-30 to -25 dBW) |
| `solar-system-baseline` | the solar-system tour; a Mars orbiter; a Europa surface user | two link light times (informational: no better or worse); RMS position errors; availability with the Earth range (never below relays only, an invariant of the model) |

The GPS scenario of `constellation-baseline` is derived from the open four-system file
(its `PROVENANCE.md` lists the edits); every other scenario is copied verbatim. No pack
uses the engine's optional workshop preset; a test scans every pack for its name.

### The demonstration sequence

`scenarios/pro-power/watch-demo/` holds six revisions of the open chained mission and a
script, `run-demo.sh`, that runs them in order through one watch, gating each against the
last passing run and appending each to one history. Revision 2 changes only a comment and
moves nothing; revision 3 weakens the jamming-phase jammer; revision 4 strengthens it, the
C/N0 falls below the 25 dB-Hz floor, every satellite is lost and the run fails; revision 5
withdraws it; revision 6 makes the spoofer ramp faster, so it is detected 60 s earlier.
It is a demonstration sequence and is labelled as one wherever it is shown. All six runs
used one engine version, so its trend shows no engine boundary.

## Licence

`kshana-pro ci` and its sub-forms need the `pro-scenario-ci` feature. Without a licence
the binary runs at Evaluation tier and refuses. `--dev` switches the check off in debug
builds only.

## Limitations

- A figure with no declared direction (in the manifest or the name table) is
  informational: its changes are reported and never fail a run.
- The statistical gate needs the ensemble's standard deviation and count as well as its
  mean. Tests on ensembles with correlated realisations, or on percentiles, are not
  provided; the F-test assumes roughly normal samples and is sensitive to heavy tails.
- A watch proves that a MODELLED figure stayed stable or moved; it does not prove the
  figure is true. The evidence label each result states (VALIDATED, MODELLED, PARTNER or
  MIXED) is carried next to every watched figure.
- A watched figure that cannot be read, and a requirement that cannot be verified (a
  missing unit, a unit mismatch), fail the run: the gate fails closed.
- A watched pack refuses a `--baseline` that is a report of another pack (another `id`):
  its watches would find no baseline and the run would pass without comparing anything.
  A report of the same pack with edited watches is accepted; a watch whose identity is new
  reads *no baseline* until the next run.
- The end of a campaign phase is read on the campaign's grid; the engine's own carried
  value can differ by one grid step of drift.
- Criteria compare values; there is no arithmetic between fields.
