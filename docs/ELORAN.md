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
| `eloran survey-plan <survey.toml> --out <plan.json>` | Integrity-driven ASF survey planning: residual ASF per station as a Gaussian process (σ growing with path length, correlation length, survey noise); protection level from weighted HDOP; greedy choice of survey points (or from candidate routes and ports) that maximises the area meeting the PL target (MODELLED prior, to be fitted from survey data) | Enterprise |
| `eloran rmode <rmode.toml> --out <report.json>` | MF R-Mode (DGNSS beacons 283.5–315 kHz, CW ranging tones at fc ± 225 Hz, IALA G1187 is the specification): complex-baseband synthesis per channel with MSK data, noise and night skywave; groundwave delay from the smooth-earth model at the beacon frequency; tone-phase ranging with cycle resolution from the initialisation fix (or the tone beat at high SNR); least-squares fix. Example: 1.4 m by day, 73 m with night skywave (MODELLED) | Enterprise |
| `eloran rmode-vdes <vdes.toml> --out <report.json>` | VDES R-Mode (VHF): radio horizon (4/3 earth), two-ray sea multipath bias, Cramér–Rao burst timing, least-squares fix; burst parameters are placeholders pending IALA G1139/G1158 (MODELLED) | Enterprise |
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

## Self-calibrating simulator (selfcal, W1119 A7)

`kshana-pro eloran selfcal <scenario.toml> --config <selfcal.json> --observations <obs.json>
--register <register.jsonl> --key-file <key> --out <result.json>` (Enterprise) closes the loop
between a reference receiver and the simulator (`valid::selfcal`):

1. generate the scenario from the current parameters (Millington groundwave over the
   conductivity map with the calibrated sections, ionospheric skywave at the current height,
   per-station ASF bias as a transmitter clock offset);
2. compare the reference observations (groundwave ToA corrected for the known receiver clock;
   skywave-minus-groundwave delay) with the simulator's prediction, scored with the fidelity
   comparator's metric rules (and, from the library, the full signal-level comparator on a
   recorded capture at the first and last pass);
3. one damped Gauss-Newton step of inverse modelling on σ per section (log10, with
   ground-class priors), ASF bias per station and skywave height, with uncertainty;
4. regenerate and repeat until the cost stops falling.

`selfcal.json` holds `{"config": {...SelfCalConfig...}, "sites": [{"id", "position"}]}`;
`obs.json` is a list of `{"site", "chain", "station", "kind": "ground_toa" | "sky_minus_ground",
"value_ns", "sigma_ns"}`. Every pass is one hash-chained calibration-register entry (parameters,
deltas, fidelity, scenario hash, recording digest), and the run is closed by an HMAC-SHA256
signature over the last entry, the calibrated scenario hash and the recording digest.
`kshana-pro eloran selfcal verify --register <register.jsonl> --result <result.json> --key-file
<key>` (free) re-checks the chain, every record digest and the signature. HMAC is symmetric
(the key holder can sign); for a public-key signature, sign the output with `kshana-pro dossier
sign`. Limits: an ASF bias and a transmitter timing offset cannot be told apart from ToA;
receiver clocks must be known; several sites with different path fractions are needed to
separate a section's σ from the station biases. Evidence: MODELLED (recovers a synthetic truth
world within its uncertainty; not yet run on a live recording).

## Coherent LF + L-band capture sets (capset, W1119 A15)

`kshana-pro eloran capset record <scenario.toml> --nav <rinex-3 nav> --out <dir> --id <set-id>`
(Enterprise) records one scenario as two SigMF streams on one timeline: `<id>_lf` (the eLoran
channels, real `rf32_le`) and `<id>_gnss` (GPS L1 C/A + Galileo E1-B complex baseband,
`ci16_le` at 1575.42 MHz, from the kshana-pro software GNSS generator through the engine's
`capset::BasebandGenerator` seam). `<id>.capset.json` carries, per stream, the start instant
as integer TAI picoseconds, the sample rate (and the exact rate ratio), the clock source and
the cable delay to the common reference plane; each SigMF file repeats it under
`global."kshana:capset"` and lists the markers as annotations. A PPS marker of known shape is
injected into both streams at the same reference-plane instants (a 10 µs Gaussian on the LF
stream, a Gaussian-enveloped chirp on the baseband).

`kshana-pro eloran capset check <dir>/<id>.capset.json` (free) checks every data file's
SHA-256 and finds each marker in each stream (correlation with the analytic marker, then a
fractional-delay refinement). After the declared start times and delays, the two streams must
agree within the stated tolerance (default 5 ns); a mis-stamped start, wrong cable delay or
wrong sample rate fails the check. The report gives a Cramér–Rao σ of each marker time from
the local noise (about 1 ns for the baseband marker at 2 MHz). Evidence: MODELLED (simulated
streams); with hardware the marker is an injected PPS-locked pulse whose own injection delay
must be calibrated.

## Tactical transmitters and signals of opportunity (tactical, W1119 A16)

Library `kshana_eloran::tactical` (NAVISP-EL1-151 / EL1-152 scenario modelling):

* **Deployable transmitters**: transmitter power, mast height, top loading and loss
  resistance give the short-monopole efficiency and radiated power; coverage and service
  radius come from the network planner; `deployable_scenario` writes a simulator scenario with
  the unknown initial ASF/timing offsets as transmitter clock errors.
  `kshana-pro eloran tactical templates` lists three illustrative classes (man-pack, vehicle,
  container; assumed parameters, not vendor data).
* **Rapid ASF calibration**: a reference receiver at a surveyed site feeds a scalar Kalman
  filter (random-walk transmitter timing) that converges on the correction beyond PF from a
  wide prior; `spatial_residual_ns` shows how the correction degrades away from the site.
* **Signals of opportunity**: AM/LW carriers at known sites and frequencies with unknown
  phase, made usable by a reference station (differential phase) and cycle resolution against
  the eLoran fix, fused with eLoran pseudoranges. `kshana-pro eloran tactical study
  [--scenario <fusion.json>]` (Enterprise) reports weighted HDOP and Monte Carlo RMS / 95 %
  error for eLoran alone and fused, and the ambiguity success rate. In the built-in
  weak-geometry template (three eLoran stations within 50° of azimuth, two LW carriers on the
  other side) the fused error is far smaller than eLoran alone.

Limits: groundwave only (daytime), homogeneous ground in the fusion study, residual
propagation errors as random biases with stated σ. Evidence: MODELLED.

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

## Observation format, reference benchmark and timing integrity

| Command | What it does | Licence |
|---|---|---|
| `eloran elof validate <file>` | Validate an **ELOF v0.1** file (eLoran Observation & Scenario Format: JSON lines with a header record, or the compact CSV profile): version, units, required fields, declared stations, ranges, phase code against group parity, monotonic time | free |
| `eloran elof convert <input> --out <file> [--from elof\|truth-csv\|obs-json] [--to jsonl\|csv] [--scenario <toml>]` | Convert a simulator truth table, imported receiver records or another ELOF profile to ELOF | Enterprise |
| `eloran bench-eloran list` | **eLoran-Bench** v0.2: eight reference scenarios (static, ship, drone, night skywave, cross-rate, impulsive noise, LDC, two UTC(k)) with their pre-registered bars and the registration hash | free |
| `eloran bench-eloran reference --out <dir>` | The in-house software receiver's ELOF observations on every scenario | Enterprise |
| `eloran bench-eloran score --obs <dir> --receiver <name> --out <dir> [--leaderboard <file>] [--key-file <f> --signer <name>]` | Score a receiver's ELOF observations against exact truth: sealed scorecard (Ed25519-signed with a dossier key), leaderboard update; refuses bars or scenarios changed after registration | Enterprise |
| `eloran tpl <scenario> [--risk <p>] [--alert-ns <ns>] [--time-ref <id>] [--grid lat1,lat2,lon1,lon2 --res <deg>]` | **ASF-aware timing protection level** for UTC(k) time transfer at the receiver position: per-station ASF σ from conductivity class, terrain and seasonal variation (Monte Carlo), cross-station correlation from shared ground classes, GLS timing solution, station-fault (solution separation) and ASF-model-error hypotheses; `--grid` gives an availability map | Enterprise |

ELOF carries per-station ToA (as a pseudo-delay against the nominal Loran schedule), ECD, SNR,
field strength, phase code, cycle flags, decided LDC symbols, the receiver's UTC(k) time
solution, ASF metadata, receiver and antenna metadata, simulator truth when the stimulus is
simulated, and the scenarios it relates to by SHA-256. Specification:
`crates/kshana-eloran/docs/ELOF.md`. Evidence: **MODELLED** (internal round-trip and rejection
tests; no external receiver writes ELOF yet).

eLoran-Bench (`crates/kshana-eloran/scenarios/bench-eloran/`): `registration.toml` holds the
SHA-256 of `bars.toml` and of every scenario, written before any scoring; the scorer re-simulates
each scenario from its registered file (truth never comes from the submission) and refuses to
run if a hash differs. Bars are **illustrative** (set by Kshana, not from a standard) and were
set after observing the in-house receiver on the fixed seeds, so they are blind only for other
receivers. Evidence: **MODELLED** (simulated stimuli; no live captures in v0.1).

Timing protection level (`crates/kshana-eloran/src/tpl/`, patent candidate E2, do not publish):
Monte Carlo coverage tests with fixed seeds build the bound from one seed and test it on
independent conductivity draws; the error exceeds the bound no more often than the stated rate
(one-sided binomial test at 10⁻³) under fault-free, station-bias and ASF-model-error worlds, and
a bound that assumes independent station errors fails the same test. Evidence: **MODELLED**.
The input spreads and priors are assumptions, and risks below about 10⁻³ rest on Gaussian tail
extrapolation. Diurnal/weather ASF and skywave are not in the bound.

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

## Receiver integrity guards (W1119, CONFIDENTIAL patent candidates)

Kshana Pro only; do not describe these mechanisms publicly before counsel decides on filing
(research 04 §2). Evidence label for all of them: **MODELLED** (internal Monte Carlo and the
crate's own simulator, threat models and receiver; no recorded attacks yet).

- **Meacon / relay detection from the propagation signature (E4)**,
  `kshana_eloran::guard::meacon`. Per station, the measured arrival delay, ECD, field
  strength, skywave delay/ratio, bearing and phase-code alignment are compared with the
  propagation model's prediction for the claimed path, normalised by the model and
  measurement σ (the delay and field σ can come from `prop::uncertainty`). Classes:
  `Consistent`, `TimingOnly` (transmitter timing fault, or a relay that matches field and
  ECD) and `Relay`. Without a clock reference the delay uses the other stations'
  leave-one-out clock (cross-station TOA geometry). Measured rates (MC, 1500 cases per class):
  clean ≈ 1e-3 false alarms per station; relay → `Relay` 99.9 %; transmitter fault →
  `TimingOnly` 99.2 %, taken for a relay 0.5 %. End-to-end runs with `threat::Meaconer` and
  station timing faults through the receiver are classified correctly.
- **Cross-system attribution (E1)**, `kshana_pro::eloran_attrib`. From the GNSS-only fix, the
  eLoran-only fix (and its leave-one-station-out variants), the local clock prediction and the
  E4 verdicts, each cause is scored by a generalised log-likelihood with an Occam penalty:
  clean, GNSS spoofed, GNSS jammed, eLoran station fault, eLoran meaconing/relay, local clock
  fault. Output: posterior-like scores, the cause, the station, the exclusion to apply and an
  explanation. Measured (400 cases per class): 0 clean false alarms; every class identified at
  ≥ 99 % (with the right station). Limits: snapshot, one cause at a time, no whole-band relay
  hypothesis, GNSS loss is attributed to jamming.
- **Delay-bounded LDC authentication (E7)**, `kshana_eloran::ldc::delay_auth`. The HMAC tag
  proves origin (and its frame number stops replays at a later frame); the arrival delay of
  the carrying pulses must also lie within `k·√(σ_ASF² + σ_TOA² + σ_clock²)` of the predicted
  `PF + SF + ASF`, so a relay of authentic frames is rejected (`Late`) although its MAC
  verifies. The bound is the replay protection level (largest hideable relay delay).
  `σ_ASF` can come straight from a `prop::uncertainty` run. Measured over 600 propagation
  Monte Carlo draws (mapped-area priors): 0.67 % false rejects at `k` for a nominal 0.1 %
  (the ASF tails are heavier than Gaussian; widen `k` if 0.1 % is required); `k = 1`
  rejects 28 %.
- **Spoof-safe online ASF learning (E3)**, `kshana_eloran::guard::asf_learn`. Per-station
  Kalman refinement of the ASF from GNSS-derived truth, only while the timing protection
  level is under its threshold and no E1 attribution alarm is active (an alarm rolls back to
  the newest verified checkpoint). The learnt correction's component in the GNSS-attack
  subspace (common mode = time push; bearing geometry = position push) is held inside the
  ASF map's prior and stays in the integrity budget; a common-mode CUSUM catches drags.
  Measured: clean learning 50 → ≤ 8 ns RMS in 6 h (20 maps); a 0.2 ns/s GNSS time drag over
  3 h moves no station > 15 ns (unguarded: 1.1 µs); a 360 m position drag stays within the
  per-station subspace bound (≈ 95 ns here); wired to E1, a 0.5 ns/s drag is attributed to
  GNSS spoofing after 170 s and the ASF moves 2 ns.
- CLI: `kshana-pro eloran guard attribute <epoch.json> --out <report.json>` and
  `kshana-pro eloran guard meacon <signatures.json> --out <report.json>` (Enterprise,
  feature `pro-eloran`).
