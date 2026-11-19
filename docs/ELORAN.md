# Kshana Pro eLoran/GNSS signal simulator

PRIVATE · Kshana Pro · crate `crates/kshana-eloran`, command `kshana-pro eloran`.

A software-defined LORAN/eLoran signal simulator for testing hybrid eLoran/GNSS receivers
and resilient PNT systems. The signal, propagation, timing and data channel are all in
software. The output goes to files (SigMF), to LF hardware (cables or antenna simulators,
never over the air) and to the built-in receiver analysis tools.

## Editions and licence

| Feature id | What | Tier |
|---|---|---|
| `pro-eloran` | scenario generation, files, truth, receiver analysis | Enterprise |
| `pro-eloran-hw` | driving LF hardware outputs and captures | Enterprise |
| — | `eloran validate`, `eloran example` | free |

Both features are listed in the signed tier manifest (`commercial/tier-manifest.json`). The
manifest in the repository is signed with the test key; a release re-signs it with the vendor
manifest key (docs/LICENSING-KEYS.md §6).

## Quick start

```bash
kshana-pro eloran example > scene.toml
kshana-pro eloran validate scene.toml
kshana-pro eloran run scene.toml --out out/scene --license <token>
```

`run` writes:

| File | Content |
|---|---|
| `out/scene.sigmf-data` / `.sigmf-meta` | interleaved real samples, one channel per `[[outputs]]` element (`ri16_le` or `rf32_le`); the metadata carries the scenario SHA-256 and the field-to-full-scale factor |
| `out/scene.truth.csv` | per station per group: emission time, time of arrival, delay and its geometric/PF/SF/ASF parts, ECD, field strength, SNR, skywave delay and ratio, time-reference offset, emission error, receiver position and heading, bearing, distance |
| `out/scene.run.json` | scenario path and SHA-256, licensee, run statistics (pulses, peak, clipping, real-time factor), evidence label |

## Service and desktop

The Kshana Pro service (`kshana-pro serve`, docs/SERVICE.md) serves the simulator through
`src/eloran_service.rs`, the same calls the command line makes:

| Route | What | Licence |
|---|---|---|
| `GET /eloran/example` | the example scenario | free |
| `POST /eloran/validate` | body: scenario TOML → `{ok, scenario_sha256, ...}`, or 422 `{ok: false, error}` | free |
| `POST /eloran/run` | body: scenario TOML (query `format=ri16\|rf32\|none`). With `--data-dir`: an `eloran-run` job (202; the job's result is the run record, its artefacts `run.json`, `truth.csv`, `recording.sigmf-meta`; the recording stays in the job's `inputs/`). Without: a synchronous run of at most 8 million samples (200), kept under the system temporary directory (`kshana-pro-eloran-runs/<run_id>/`). The run record carries `run_id`, the scenario SHA-256, the statistics, the file paths, the recording's SHA-256 and the first 20 truth rows | `pro-eloran` |
| `POST /eloran/rx` | `{"run": <run_id>, "scenario"?: <TOML, default the run's>, "per_loop"?, "cri_blanking"?, "truth"?}` → the receiver report (as `eloran rx --truth`): per-station ToA error statistics and cycle errors, position fix and error, time offsets per reference. Reads at most 256 MiB of recording | `pro-eloran` |
| `GET /eloran/gri`, `GET /eloran/gri/{chain}` | the GRI database (`region`, `active_on`, `search`); entries with their `[[chains]]` snippet | free |

A run's identifier is its job identifier (tenant, operation and request), so the same
scenario is the same run, and another tenant's run answers as one that does not exist.
Hardware output (`eloran stream`, `pro-eloran-hw`) is never served over HTTP: it stays on
the command line, conducted use only.

Kshana Desktop Pro has an **eLoran** page (Enterprise, signed in to the service;
`apps/desktop-pro/pro/desktop/eloran.mjs`): a scenario editor with the example preloaded and
a Validate button (hash or error), the chain map and table of the GRI database (region,
status, confidence; Insert adds a chain's `[[chains]]` entry, and a zero-offset
`[[time_refs]]` entry for a reference the scenario lacks), run controls with the run
statistics and a truth preview (a job is followed until it finishes), and the receiver
results.

## Architecture

```
scenario.toml ─▶ Scenario::build ─▶ Scene ─┬─▶ SampleSink: SigMF file │ LF hardware │ memory
                                           └─▶ truth (per station per group)
Scene = chains (signal definition, stations, faults, data-channel modulator)
      + timebase (truth TAI timeline, UTC(k) and GNSS time references)
      + propagation (groundwave PF/SF/ASF + skywave, behind one trait)
      + receiver trajectory + antenna output channels (E-field, crossed H-loops)
      + interference and noise
```

* **Exact timing.** Time is an integer picosecond timeline. Each pulse is evaluated
  analytically at every sample instant, and the sample time relative to the pulse arrival is
  computed in integer arithmetic. Delay resolution is therefore independent of the sample rate
  (tested: a 10 ns reference offset moves the output by 10.00 ns; arrival accurate to < 0.05 ns).
* **Data-driven signals.** A signal definition is data (carrier, envelope family or table, pulse
  offsets, phase codes). LORAN-C per COMDTINST M16562.4A is the default; emerging definitions are
  configuration. Data channels are modulators that edit each group's pulses.
* **Multi-chain.** Any number of chains, each with its own GRI (40 000–99 990 µs in 10 µs steps) on
  the Loran epoch. Cross-rate interference comes from true superposition.
* **Multi-time-reference.** Each station is steered to a UTC(k) model (offset, drift, steps,
  seeded clock noise), optionally with its own transmitter clock. GNSS system times are explicit.
* **Deterministic.** The same scenario and seed give identical samples (CI checks the hash).

## Scenario reference

See the module documentation of `crates/kshana-eloran/src/scenario.rs` and the example
scenarios in `crates/kshana-eloran/scenarios/`. The sections are `[scene]`, `[receiver]`,
`[[outputs]]`, `[[time_refs]]`, `[gnss_time_residual_ns]`, `[propagation.ground]`,
`[propagation.sky]`, `[[interference]]`, `[[chains]]` with `[[chains.stations]]` and an
optional `ldc`, and `[file]`.

## Evidence labels

Every model states its label in its module documentation. A model is VALIDATED only where an
independent external reference backs it (signal specification, ITU-R recommendations, GRWAVE,
recorded live signals); otherwise it is MODELLED. A run record carries the label of the run.

## Policy

* Hardware output goes into cables or antenna simulators only. Never over the air.
* Proprietary: the crate is `publish = false`, and nothing of it goes to the open repository.

## Scenario packs

Ready-made application scenarios live in `crates/kshana-eloran/scenarios/packs/<domain>/`.
Each file starts with a header stating its purpose, what it exercises, the truth and receiver
metrics to read, its duration settings and the provenance of every value. Real (historic)
chain geometry is copied unchanged from the GRI database (`data/gri_db.json`, the entry id is
cited next to each chain and a test checks the values); everything else (receiver sites,
tracks, coastline polygons, faults, noise levels, time-reference offsets beyond the Circular T
snapshot) is marked ILLUSTRATIVE.

| Domain | Scenario | What it is for |
|---|---|---|
| timing | `prtc-backup.toml` | eLoran-only holdover of a static timing receiver; UTC(k) and caesium clock noise; documented one-day truth-only run |
| timing | `multi-time-reference.toml` | two chains on two UTC(k) with different offsets and drifts, and a UTC(k) step fault |
| timing | `receiver-calibration.toml` | single station at a known distance, high SNR, E-field, for receiver delay calibration |
| timing | `transmitter-faults.toml` | timing step, transmitter clock step, off, blink, power step and ECD step |
| maritime | `harbour-approach.toml` | ship track with course changes, crossed loops, Millington ASF over a coastline, temporal ΔASF (differential-eLoran setting) |
| maritime | `coastal-passage-night.toml` | night skywave (ionospheric, fading) and atmospheric noise on a coastal track |
| maritime | `cri-and-cw.toml` | cross-rate interference from an adjacent chain plus CW interferers |
| maritime | `ldc-eurofix-dloran.toml` | Eurofix data channel carrying DLoran corrections, warnings, DGNSS and authentication |
| rail | `electrified-line.toml` | `railway_traction` impulsive noise during a train passage, H-field loops, heading changes, land paths |
| rail | `gnss-denied-segment.toml` | eLoran-only, two-chain all-in-view segment of a hybrid test (GNSS degradation set in the GNSS bridge or simulator) |
| aviation | `fast-receiver.toml` | ≈ 70 m/s receiver with altitude, E-field |

The files carry the long durations a study needs (some truth-only, `[file] format = "none"`);
`crates/kshana-eloran/tests/packs.rs` runs each for 0.3 s and checks truth, clipping and the
GRI-database provenance.

## Validation ledger

The evidence label, reference, test, tolerance and known limitations of every model and
capability, and a requirements-coverage summary, are in
[`crates/kshana-eloran/docs/VALIDATION.md`](../crates/kshana-eloran/docs/VALIDATION.md).

## Bench, validation and system tools (phase 2)

| Command | What it does | Licence |
|---|---|---|
| `eloran awg <scenario> --out <base>` | Compile a periodic scenario (static receiver, no time-varying effects) into one repeating segment of whole phase-code cycles for an arbitrary waveform generator; report with segment length and model error | Enterprise |
| `eloran accept transmitter <capture> --gri <µs> --first-group <s> --out <report.json>` | Transmitter acceptance from a SigMF capture (dummy load or sampled antenna current): per-pulse phase code, standard zero crossing against the schedule, ECD, amplitude; capture-level ECD mean/spread, timing offset/jitter, phase-code errors, amplitude stability, power fraction in 90–110 kHz. Default limits are illustrative: set them from the transmitter specification | Enterprise |
| `eloran accept software --out <report.json>` | Software-only acceptance tests: ToA/ECD/ASF 10 ns resolution, SNR range, skywave delay range | Enterprise |
| `eloran sync measure --tic <csv> --out <report.json>` | eLoran-vs-GNSS sync from time-interval-counter readings: mean offset, TDEV/MTIE, drift, alarms against 10 ns, correction for the calibration file | Enterprise |
| `eloran sync certify --tic <csv> --out <dir> --gnss-sim <name> --counter <model> [...]` / `sync verify-cert <dir>` | Sync-Verify: eLoran-to-GNSS alignment certificate for any GNSS simulator: offset ± U (k = 2), TDEV per decade, drift, guard-banded decision (conforms / does not conform / inconclusive); the readings are stored and hash-bound, the certificate recomputes from them and can be signed with the dossier key | Enterprise |
| `eloran noise-experiment --out <report.json>` / `eloran rx ... --impulse clip\|blank` | Impulsive LF noise: equal-power Gaussian vs Middleton Class A, matched-filter timing with no / clip / blank mitigation (default run: 72 ns Gaussian, 75 ns Class A linear, 30 ns clipped, 17 ns blanked; MODELLED, validate against recorded noise); the receiver can apply the mitigation to a recording | Enterprise |
| `eloran events <validate\|merge\|publish> <files>` | PNT interference events in one schema (`kshana-pnt-interference-event/1`) across GNSS, eLoran, R-Mode and LEO: validation, merging of independent reports into corroborated events (confidence 1 − Π(1 − c)), publication with grid-cell coarsening and hashed observers, JSON / GeoJSON / CSV | Enterprise |
| `eloran edo [<edo.toml>] --out <report.json>` | eLoran-disciplined oscillator bench (EL1-145 class): oscillator ADEV/ageing, GNSS and eLoran references with noise, bias and outages, type-2 discipline loop, GNSS-to-eLoran switchover (peak, settling time, uncalibrated offset), holdover, MTIE/TDEV | Enterprise |
| `eloran timeref circular-t <file> --labs A,B --start <utc>` | `[[time_refs]]` fitted to BIPM Circular T UTC−UTC(k) values | free |
| `eloran asfgrid <scenario> --box lat1,lat2,lon1,lon2 --res <deg> --out <base>` | ASF per station on a grid with the scenario's ground model (CSV + GeoJSON); importable as an `asf_grid` ground model | Enterprise |
| `eloran fidelity <recording> --meta <capture.json> --scenario <toml> --out <report.json>` | Recording vs simulation at signal, observable and solution level, with pass/fail per metric | Enterprise |
| `eloran import <log> --profile <csv-profile.toml> --out <obs.json>` | Receiver log → common observation records (CSV profiles; NMEA, vendor-split and TIC importers are in the library) | Enterprise |
| `eloran conformance list` | The conformance library (16 cases "modelled on" eLoran/Loran-C receiver standards; limits illustrative until confirmed) | free |
| `eloran campaign run <plan.toml> --out <dir>` / `verify <dir>` | Pre-registered validation campaign: per-case results, summary, RVM, hash-chained manifest; verify detects any modification | Enterprise / free |
| `eloran plan <plan.toml> --out <base>` | Network planner: coverage, weighted HDOP, 95 % accuracy, availability under station outages, site selection, CRI risk | Enterprise |
| `eloran twin <twin.toml> --out <result.json>` | National-network digital twin: transmitter clocks, monitor stations, alarms with LDC warnings, PI steering, KPIs | Enterprise |

Library-only (wire them into lab tooling or a later command):

* **Hardware drivers** behind one `Device` trait (opened from the command line with `stream --device` and `bench`, above): SoapySDR, Ettus UHD (C API), Red Pitaya, Digilent Analog Discovery, generic SCPI/LXI arbitrary waveform generators (editable command profiles), a file device, and a **virtual bench** (latency, clock offset, jitter, quantisation, crosstalk, lock loss, underruns, loopback) for CI. Hardware health aggregation with alarms. Drivers for real devices compile but have not run on hardware.
* **Instruments** (`instr`): counters, scopes, spectrum analysers, power meters, frequency counters over TCP/serial with editable profiles; TAPR TICC parser; JSON-lines evidence log. **Calibration wizards** (`accept::calibrate`) for simulator delay/gain/linearity and receiver timing with a GUM report; **acceptance suite** (`accept::suite`) with RVM output; **calibration register** (`calreg`) with a hash chain.
* **GNSS simulator adapters** (`sync`): Skydel, Spirent, R&S SMBV/SMW, LabSat, gps-sdr-sim, each with a configurable command profile to confirm against the vendor manual; GNSS degradation timelines translated per simulator; timing-reference distribution model with cable delays; Circular T importer (`timeref`).
* **Validation** (`valid`): fidelity comparator, inverse modelling (ASF, conductivity, skywave, transmitter offsets), GRWAVE / P.368 / ASF-survey cross-check harnesses that can promote a model to VALIDATED.
* **Field** (`field`): unattended LF recording station with chain detection, health and upload manifest; receiver log importers.
* **Propagation** (`prop`): terrain groundwave (integral equation over SRTM/ASCII DEMs), P.832/GeoJSON conductivity atlases, urban effects, ASF grids, ionosonde/space-weather inputs, Monte Carlo propagation uncertainty.
* **Receiver and system**: real-time streaming receiver (`rt`), differential eLoran reference station (`dloran`), threat models (`threat`: jamming, spoofing, meaconing, forged data messages — **Enterprise vetted-buyer gating required before exposing them**, as for `attack-iq`), network planner (`plan`), digital twin (`twin`).
* **kshana-pro**: low-cost GNSS path to an SDR with degradation (`eloran_gnss_sdr`), GNSS time transfer and common view for UTC(k) traceability (`eloran_timexfer`), hybrid eLoran/GNSS fusion with cross-system integrity (`eloran_fusion`), eLoran in multi-source trust (`eloran_trust`).

Signed V&V evidence: a campaign directory (or any run output) can be signed with the existing `kshana-pro dossier sign` and verified offline with `kshana-pro dossier verify`.

CI: `.github/workflows/eloran.yml` runs format, strict clippy (with and without the hardware driver features) and the full engine test suite on every push, and a nightly virtual-bench, acceptance and campaign run.

Planner note: with the default station availability (0.999) and service-availability target (0.999), three stations cannot meet the target at any point (all three must be up: 0.997). Use more stations, or set the targets to the service you are planning.

## Bench readiness: hardware ready before it arrives

The aim: when the hardware arrives, it only needs plugging in and testing. Everything that can
be done from datasheets is done in software first.

| Command | What it does | Licence |
|---|---|---|
| `eloran bench catalog [--category <c>] [--lf]` / `bench show <id>` | Hardware catalogue: published specifications of LF sources, SDRs, AWGs, GNSS simulators, references, counters, scopes, eLoran receivers and antennas, each value cited to the vendor's datasheet, product page or manual. `--lf` lists the sources that put 90–110 kHz on their output directly | free |
| `eloran bench check --inventory <bench.toml>` | Readiness of a bench before buying or wiring: band, sample rate, DAC resolution, driver, 10 MHz and PPS, reference fan-out, TIC resolution, digitiser; Pass / Warn / Unknown / Fail with the source of each value; which acceptance tests the bench can run | Enterprise |
| `eloran bench twin <id>` | Predicted eLoran output of one device from its datasheet (spec twin): zero-crossing bias and spread, ECD and waveform SNR per output level | Enterprise |
| `eloran bench kit --inventory <bench.toml> --out <dir>` | The bench kit: assessment, cable-by-cable wiring plan with the attenuator pads, first-plug procedure with the exact commands, instrument profiles, device addresses, twin predictions | Enterprise |
| `eloran bench rehearse --inventory <bench.toml> --out <report.json>` | The acceptance suite with every device replaced by its spec twin; the TIC and counter tests predict from the published jitter, resolution and Allan deviation whether they would pass | Enterprise |
| `eloran bench probe (--inventory <bench.toml> \| --host <ip[:port]>)` | Find and identify connected instruments, read-only (`*IDN?`, serial ports, `uhd_find_devices`, `SoapySDRUtil --find`); never configures an output | Enterprise + hardware |
| `eloran bench firstplug --inventory <bench.toml> --out <dir> [--rehearse] [--signer <name> --key-file <file>]` | First plug-in check: the source plays the calibration train into its own loopback at several levels; each level is compared with the spec twin (gain, SNR, zero-crossing spread, ECD) and deviations are diagnosed (50 Ω/high-Z load mismatch, attenuator, clipping, noise floor, unlocked clock, coupling). Captures are stored as 16-bit files, hashed and bound with the measurements and the device identity; the directory can be signed with the dossier key | Enterprise + hardware (`--rehearse`: Enterprise) |
| `eloran bench verify-evidence <dir>` | Re-check first plug-in evidence: capture hashes, the binding hash, every number recomputed from the captures, and the signature | Enterprise |
| `eloran bench accept --inventory <bench.toml> --profiles <dir> --out <report.json>` | The acceptance suite on the real bench: the eLoran source by its driver, instruments by their profiles | Enterprise + hardware |

`eloran stream --device` now opens every driver from one address: `twin:<catalog-id>`,
`file:<base>`, `redpitaya:<host>`, `uhd:<args>`, `soapy:<args>`, `digilent:<index>` and
`scpi-awg:<host>[@profile]`. Build features: `eloran-redpitaya`, `eloran-uhd`, `eloran-soapy`,
`eloran-digilent` (the SCPI AWG and file drivers are always in).

Inventory (TOML; examples in `crates/kshana-eloran/scenarios/bench/`):

```toml
name = "LF bench"
[requirements]
need_gnss = true
dut_input_vpp = 0.05      # receiver's maximum antenna-input level: sets the attenuator pads
[[device]]
catalog = "redpitaya-stemlab-125-14"
role = "eloran_source"    # eloran_source | gnss_source | reference | tic | counter |
host = "192.168.1.50"     # spectrum | power | digitiser | eloran_rx | antenna | distribution
```

What it settles before purchase: many RF SDRs cannot output 100 kHz at all, because their
lowest tuning frequency is in the tens of MHz and their outputs are AC-coupled. The check marks
them FAIL as eLoran sources, with the datasheet value; they remain usable as GNSS sources.

Evidence: catalogue values are **datasheet values** (`confidence` per entry; `null` where no
reliable public source was found, reported as Unknown, never as a pass). Twin results are
**MODELLED** (converter only: quantisation, published bandwidth, coupling, jitter); analogue
effects the datasheet does not state are not in them. Instrument command profiles written by
the kit are **generic placeholders** to confirm against each programming manual. Nothing in
this section has been measured on hardware.

## Live monitor (GUI)

`kshana-pro eloran monitor <scenario> --out <dir> [--recording <base>] [--interval <s>]` runs the
scenario (or replays a recording) through the real-time receiver and writes:

* `<dir>/monitor.jsonl`: a header line, one snapshot line per interval (spectrum, pulse view per
  station, per-station SNR/ECD/field/bearing/lock/cycle slips, the latest position or time fix,
  health: real-time factor, latency, peak level, clipping), and a summary line with the truth
  comparison (ToA error per station, cycle errors, final fix error);
* `<dir>/monitor.html`: a self-contained dashboard (map, station table, spectrum, pulse view,
  health tiles, fix, truth table, timeline player, light/dark). To follow a long run live, serve
  the directory (`python3 -m http.server`) and open `monitor.html?live=monitor.jsonl`.

The service offers the same as `POST /eloran/monitor` (JSON, or `?format=html` for the page;
synchronous, Enterprise), and Desktop Pro's eLoran page has a **Live monitor** card that shows
the dashboard in a sandboxed frame.

## Python SDK

`sdk/python` (package `kshana_eloran`, no third-party dependencies): `Eloran` wraps every
`kshana-pro eloran` command, `ServiceClient` the `/eloran/*` service routes, plus readers
(`read_sigmf`, `read_truth`, `read_monitor`) and a `ScenarioBuilder`. See `sdk/python/README.md`.
The generated service clients in `clients/` include the monitor route.
