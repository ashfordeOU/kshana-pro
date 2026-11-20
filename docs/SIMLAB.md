# Simulation fidelity and lab tooling (`kshana-pro simlab`)

Research backlog E32 (items B3201, B3203, B3206, in part). Source: `src/simlab/`,
`src/cli/simlab_cmd.rs`. Tests: `tests/simlab.rs` (synthetic inputs only).

Tools for the people who run GNSS simulators, record-and-replay benches and test services. They
sit next to the test-as-code kit ([TEST-KIT.md](TEST-KIT.md)) and feed its replay-fidelity
statement. Every producing command writes a signed directory (`summary.txt`, the JSON result,
the inputs under `inputs/`, the signature) labelled **MODELLED**. Nothing here drives an
instrument or emits a signal: the commands read files the user's own bench wrote and write
files. No output says that a simulator, receiver, bench or lab is calibrated, compliant,
conformant, accredited or certified.

| Command | Feature | Tier | Output |
|---|---|---|---|
| `simlab chain <chain.toml> [--register <file.jsonl>]` | `pro-simlab-replay` | Pro | `chain.json`, `capture.toml` |
| `simlab sync <replay-set.toml>` | `pro-simlab-replay` | Pro | `sync.json` (with the replay manifest) |
| `simlab selftest <selftest.toml>` | `pro-simlab-selftest` | Pro | `selftest.json` |
| `simlab report <lab-results.toml>` | `pro-simlab-report` | Pro | `lab-report.json`, `lab-report.md` |
| `simlab compare <dir> <dir>... [--trust <key>]` | `pro-simlab-report` | Pro | `comparison.json`, `comparison.md` |
| `simlab tco <tco.toml> [--json] [--markdown <file>]` | none | free | table on stdout |
| `simlab synth <dir>` | none | free | synthetic inputs for every command |
| `simlab check <dir> --trust <key>` | none | free | verification |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]` and a
licence (`--license <token>`, the stored licence, or `--dev` in debug builds).

```sh
kshana-pro simlab synth /tmp/sl
kshana-pro simlab chain /tmp/sl/chain-v1.toml --register /tmp/sl/reg.jsonl --out /tmp/sl/c1 --signer lab --key-file key.hex
kshana-pro simlab chain /tmp/sl/chain-v2.toml --register /tmp/sl/reg.jsonl --out /tmp/sl/c2 --signer lab --key-file key.hex
kshana-pro testkit fidelity /tmp/sl/c2/capture.toml --scenario timing-holdover-under-jamming
kshana-pro simlab sync /tmp/sl/drive-good.toml --out /tmp/sl/s --signer lab --key-file key.hex
kshana-pro simlab selftest /tmp/sl/selftest-sim.toml --out /tmp/sl/st --signer lab --key-file key.hex
kshana-pro simlab report /tmp/sl/lab-a.toml --out /tmp/sl/a --signer lab --key-file key.hex
kshana-pro simlab report /tmp/sl/lab-b.toml --out /tmp/sl/b --signer lab --key-file key.hex
kshana-pro simlab compare /tmp/sl/a /tmp/sl/b --trust <pub hex> --out /tmp/sl/cmp --signer lab --key-file key.hex
kshana-pro simlab tco /tmp/sl/tco.toml
```

## 1. Recorder-chain register (B3201, gap 08:A6)

A recording is only as faithful as the chain that made it. `kshana-simlab-recorder-chain/1`
lists the stages in signal order (`lna`, `filter`, `amplifier`, `cable`, `mixer`, `adc`, `agc`,
`reference`) with gain, noise figure, bandwidth, group delay and its standard uncertainty, the
ADC bit depth, the AGC mode, the reference oscillator and its phase noise. The command computes:

* the cascade gain and the **Friis noise figure** `F = F1 + (F2 − 1)/G1 + …` (a passive loss of
  L dB has NF = L dB; a stage with gain and no stated NF is taken as noiseless and named in a
  finding, so the cascade NF is then a lower bound), with each stage's contribution;
* the recorded bandwidth (narrowest filter, or the complex sample rate), the summed group delay
  and its uncertainty in quadrature (GUM, uncorrelated stages);
* findings, each a rule of thumb: aliasing (sample rate below the analogue bandwidth,
  invalidating), bandwidth below the signal's main lobe, a 1-bit ADC (1.96 dB), an active AGC,
  a free-running TCXO or no reference, a later stage adding more than 0.5 dB to the cascade NF;
* a `capture.toml` in the `kshana-testkit-capture/1` format, so `testkit fidelity` works from
  the registered chain rather than retyped values.

With `--register <file.jsonl>` the cascade quantities (gain, NF, bandwidth, bit depth, group
delay) are appended to the hash-chained calibration register of the eLoran crate
(`calreg`, subject kind `instrument`, the chain file's SHA-256 as the report hash), after the
register's own chain is verified. Against the previous registration of the same `chain_id`, a
quantity that moved beyond its tolerance (gain 1 dB, NF 0.3 dB, bandwidth 1 %, any bit-depth
change, group delay twice its uncertainty) is listed in `changes_since_previous`. The register is
tamper-evident, not tamper-proof: keep the `register_head` the result prints somewhere else.

## 2. Synchronous multi-sensor replay (B3201, gap 08:A7)

`kshana-simlab-replay-set/1` names the reference stream (normally the GNSS I/Q recording), a
tolerance, and each stream: kind (`iq`, `imu`, `can`, `camera`, `odometry`, `other`), clock
class (`gnss`, `pps-disciplined`, `ptp`, `host`, `free-running`) and its time line (a SigMF
meta file, whose sample count comes from the data file size and `core:datatype`; or
`start_s` + `sample_rate_hz` + `samples`; or a CSV of timestamps). Shared events (a tap, a
light flash, a PPS marker) give each stream's time at the same physical instant.

Per stream: rate (median interval), jitter (standard deviation of the regular intervals), gaps
(> 2.5 × the median), non-increasing stamps, and the mapping to the reference clock:

| shared events | mapping | status |
|---|---|---|
| ≥ 2 | least-squares `t_ref = a + b·t` (offset, drift in ppm), residuals at the events | `aligned` when the largest residual ≤ tolerance (with two events the residual is zero by construction, and the result says so) |
| 1 | offset only; drift bounded by the clock class (free-running 50 ppm, host 100 ppm, disciplined 1 ppb, representative) over the duration | `aligned-offset-only` when the bound ≤ tolerance, else `misaligned` |
| 0 | only when both clocks are disciplined to GNSS time, at the declared `clock_offset_s` | `aligned-offset-only`, else `unsynchronisable` |

Jitter above the tolerance downgrades `aligned`. The **replay manifest** is the common window on
the reference clock and, per stream, the correction and the start (seconds on the stream's own
clock, and the sample index for I/Q). The verdict is `synchronised`, `limited` or
`not-synchronised`; `sensors_synchronised` is the value `testkit fidelity` needs (every non-I/Q
stream `aligned`).

## 3. Simulator self-test and replay-rig check (B3203 gap 08:A31; B3201 gap 09:D11)

One or more reference receivers, cabled to the source, log C/N0 per PRN (`t_s,prn,cn0_dbhz`),
fixes (`t_s,lat,lon,h`) and the PPS offset against the source's PPS (`t_s,offset_ns`). In
`mode = "simulator"` the expectation is the scenario's power setting (`cn0_dbhz` or
`[expected.cn0_by_prn]`), static position and PPS offset; in `mode = "replay-rig"` the expected
C/N0 per PRN is the station's own mean while the replayed file was recorded
(`expected.observations`), so the check says whether the AWG/SDR rig reproduces what the
station saw.

Per receiver, four checks against the user's tolerances: **power level** (median over PRNs of
mean C/N0 − the receiver's stated C/N0 bias − expected), **relative power** (largest per-PRN
deviation from that median, plus expected PRNs never tracked), **position** (95th-percentile
horizontal error, flat-earth approximation over metres), **time** (mean PPS offset − expected).

Attribution per check: to the **source** only when at least two receivers each deviate, with the
same sign, and agree with each other within the tolerance (the source's estimated offset is
their mean); to the **receivers** that deviate when the others pass; `unattributed` with one
receiver; `inconclusive` when all deviate but disagree. Verdict: `consistent`,
`simulator-deviation` or `rig-deviation`, `receiver-deviation`, `inconclusive`.

A consistent result is evidence that the reference receivers saw what was expected; it is not a
calibration of the simulator. A receiver's reported C/N0 is only as good as its own
calibration (`cn0_bias_db`), which the check records but cannot verify.

## 4. Comparable lab test reports (B3206, gaps 08:D4, D5, D6)

Spoofing and interference test services report the same kinds of results in different layouts
and units. `kshana-simlab-lab-results/1` maps one lab's results CSV, column by column, to a
fixed vocabulary in canonical units:

| metric | unit | meaning |
|---|---|---|
| `jsr_at_first_effect_db`, `jsr_at_loss_of_lock_db`, `jsr_at_reacquisition_db` | dB | J/S thresholds |
| `cn0_drop_db` | dB | C/N0 drop at peak interference |
| `time_to_detect_s`, `time_to_alarm_s`, `time_to_capture_s`, `reacquisition_time_s`, `ttff_s` | s | times |
| `position_error_max_m`, `position_error_95_m` | m | horizontal position error |
| `time_error_max_ns` | ns | PPS or time output error |
| `availability` | fraction | epochs with a valid fix |
| `detected`, `captured` | bool | flag raised; output pulled off |

A column maps to `field = "scenario"`, a `metric`, or the expanded uncertainty (k = 2) of one
(`uncertainty_of`), with its unit (ms, s, min; cm, m, km; ns, us, ms, s; %, fraction).
`[scenario_ids]` maps the lab's scenario names to common ids and `[scenario_sha256]` records the
hash of the scenario file each was run from. Only `conducted` and `shielded-chamber` RF paths
are accepted; over-the-air results are refused.

The normalised report lists the results, the unmapped columns, the cells that could not be
read, the core metrics of the service it lacks (spoofing: detected, captured, time to detect,
max position error, max time error; interference: J/S at loss of lock and at reacquisition,
reacquisition time, 95 % position error, availability), and an **EN 16803 topic index**: which
report contents bear on the topics of EN 16803-1 (definitions and performance metrics), EN
16803-2 (basic performance assessed by record and replay) and EN 16803-3 (security performance),
plus the uncertainty of results and the bench reference's calibration date. Parts are cited by
number and scope only. We found no accreditation scheme for EN 16803 testing; the index helps a
lab assemble its own evidence and is not a statement of conformity.

`simlab compare` takes two or more signed report directories (with `--trust`, each signature is
checked first) and, per scenario and metric, gives the difference and, when both labs stated an
expanded uncertainty, the **En number** `|x₁ − x₂| / √(U₁² + U₂²)` (ISO 13528; ≤ 1 consistent).
Booleans agree or disagree. A pair is `not-comparable` when the scenario files differ (hash),
or when an uncertainty is missing; a different RF path or an unstated hash is noted.

## 5. Simulator total cost of ownership (B3203, gap 08b:Q2)

`kshana-simlab-tco/1` lists options (buy, keep a legacy unit in service, a record-and-replay
unit, test as a service) with purchase, annual maintenance, staff and other costs, cost per
campaign and residual value. The result gives per option the nominal total, the present value
at the discount rate, the cost per campaign and the equivalent annual cost, the ranking, and per
pair the break-even campaign rate. No price is built in; the synthetic file's figures are
illustrative and say so.

## Not built in this change

* **Hardware-in-the-loop drivers for further simulators.** The test kit already drives Skydel,
  Spirent (SimREMOTE style), R&S SMBV and LabSat through editable command profiles, all
  unconfirmed templates until a lab checks them against the vendor manual. No new vendor
  command set was added: we have not read those manuals, and we do not invent commands.
* **Legacy-simulator life extension** (gap 08b:Q4) beyond what the profile mechanism gives: a
  lab can write a profile for an older unit's documented remote interface; no profile for any
  specific legacy model is shipped.
* **Simulator self-test against a model of the RF output** (spectrum and power measured with an
  instrument): the self-test here uses reference receivers only.
* **Live capture of reference-receiver logs**: the self-test reads files.
* B3202 (SIL observation generator and urban multipath model), B3204 (interference model
  library) and B3205 (antenna characterisation) belong in the open engine and are separate
  work.

## Evidence and limits

All MODELLED. The formulas are textbook (Friis, least squares, GUM quadrature, ISO 13528 En);
the thresholds (clock drift classes, gap rule, tolerance defaults in the synthetic files) are
representative rules of thumb. No bench, simulator, receiver or lab has been assessed with
these tools in this build; the tests use synthetic inputs only.
