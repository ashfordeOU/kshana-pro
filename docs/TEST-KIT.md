# Test-as-code kit (research 2026-11-20, gaps 07, 08, 08b, 11, 12)

Receiver tests written once in a neutral form, compiled for whichever lab simulator is on the
bench, run in CI against every receiver build, and scored the same way each time. Code:
`src/testkit/`; CLI `kshana-pro testkit`; packs in `testkit/packs/`; tests `tests/testkit.rs`.

**Evidence label: MODELLED.** No receiver has been scored with the kit in this build: the
tests use synthetic receiver logs and a loopback simulator. Pack parameters are
representative. The simulator command profiles are the unconfirmed templates of
`kshana-eloran::sync` unless a lab passes its own confirmed profile (`--profile`). A verdict is
test evidence for the scenario run; it never says a receiver is compliant, conformant or
certified, and the packs are *modelled on* the documents they name, not test methods of them.

## Why

Simulator scenarios do not move between vendors; receiver makers rerun past incidents
nightly with their own tooling; record-and-replay benches have fidelity limits nobody states;
ESA tenders each buy a separate resilience test bench; IMCA IN 1730 names DP threats but no
test method; the DHS Resilient PNT Conformance Framework and IEEE P1952 define resilience
levels with no runnable tests. The kit gives one open-format scenario, one scoring method and
signed verdicts that a regression diff can compare across builds.

## Licence tiers

| Command | Feature | Tier |
|---|---|---|
| `validate`, `packs [list]`, `show`, `sample`, `verify` | none | free |
| `compile`, `run`, `from-incident`, `from-library`, `fidelity`, `packs export` | `pro-test-kit` | Pro |
| `diff`, `run --baseline` | `pro-test-kit-regression` | Enterprise |

Both features have explicit entries in `license::explicit_tier` and in the signed tier
manifest (`commercial/tier-manifest.json`, `pro` edition).

## 1. Neutral scenario (`kshana-testkit-scenario/1`)

TOML or JSON. Sections: `[scene]` (start UTC, duration), `[[signal]]` (constellation and
signals; GPS L1 C/A when absent), `[trajectory]` (static, `[[trajectory.waypoint]]` with
speeds, or time-tagged `[[trajectory.sample]]`; platform `static|land|vessel|aircraft|uas|timing`),
`[environment]` (ionosphere, troposphere, multipath, obscuration, synchronised `sensors`),
`[[threat]]` (signal-parameter level), `[expected]` (behaviour in words and the misleading and
recovery thresholds) and `[[criterion]]`. See the header of `src/testkit/spec.rs` for a full
example.

Threat types: `jamming` (waveform cw/wideband/chirp/pulsed, centre, bandwidth, J/S),
`spoofing` with a technique (`simplistic` step, `position_drag` by rate, acceleration limit or
final offset over the ramp, `time_push`, `meaconing` delay), `cn0_offset`, `power_step`,
`outage`, `sat_mask`. A threat may instead name a threat-library scenario
(`library = "<id>"`, no `type`): its analogue events are expanded at signal-parameter level
(a C/N0 drop of d dB becomes an L1 wideband jammer at J/S = d + 10 dB, stated as
representative; a drag-off becomes `position_drag`; a time spoof `time_push`; a replay delay
`meaconing`). A timing platform takes the library's timing profile when it has one.

Criteria (`metric`, `op` `<=` or `>=`, `value`, optional `window`, `threat`, `threshold`,
`required`, `level` 1 to 4): `detection_latency_s`, `misleading_epochs`,
`max_unflagged_h_error_m`, `max_unflagged_time_error_ns`, `recovery_s`, `availability`,
`max_h_error_m`, `p95_h_error_m`, `max_time_error_ns`, `false_alarms`. Windows: `baseline`,
`threat`, `after`, `from_onset`, `all`. The validator refuses the words compliant, conformant,
certified and certification in a scenario's own text.

## 2. Compiler and validator

```sh
kshana-pro testkit validate <scenario> [--adapter skydel|spirent|rs-smbv|labsat|gps-sdr-sim|all] [--profile <confirmed.toml>] [--json]
kshana-pro testkit compile  <scenario> --adapter <name> --out <dir> [--profile <confirmed.toml>]
```

A scenario is a file or a pack scenario (`dp-vessel/dp-slow-drift` or `dp-slow-drift`).
Profile adapters get `commands.txt` (setup and timed schedule), `schedule.json`,
`trajectory.nmea`, `trajectory.csv`, and `spoofer.csv` (the counterfeit's per-second offset
for the simulator's own spoofing scenario, as PICS exports do). gps-sdr-sim gets `run.sh` and a
10 Hz `motion.csv`. Every compile writes `COMPILE.json`.

Each finding is `native`, `approximated`, `manual` (the operator sets it, as the finding says)
or `unsupported`. Examples: gps-sdr-sim generates GPS L1 C/A only and has no run-time
controls; LabSat replays a file and generates no jammer or spoofer; no simulator profile
carries environment models or non-GNSS sensors (manual); a spoofer on a profile adapter is a
schedule marker plus offsets (approximated). **No spoofer input is written for gps-sdr-sim**:
a counterfeit signal file is attack IQ, which only the vetted `attack-iq` route produces.

## 3. CI runner, signed verdicts, regression diff

```sh
kshana-pro testkit run pack:dp-vessel my-scenarios/ --build fw-4.2.1 --out verdicts \
    --logs recorded/ --signer "Lab" --key-file lab.key [--capture capture.toml] [--baseline verdicts/fw-4.2.0]
kshana-pro testkit run pack:uas --build fw-4.2.1 --out verdicts --adapter skydel \
    --sim 10.0.0.5:4820 --receiver 10.0.0.9:5000 [--sim-trajectory C:\\scen\\trajectory.nmea] --signer Lab --key-file lab.key
kshana-pro testkit diff verdicts/fw-4.2.0 verdicts/fw-4.2.1 --trust <key hex> --markdown summary.md
kshana-pro testkit verify verdicts/fw-4.2.1 --trust <key hex>
```

Offline, the receiver output for each scenario is `<id>.json` (the PICS device log,
`kshana-pics-device-log/1`) or `<id>.nmea` in the log directory; a missing one is recorded as
`not-run`. Live, the runner sets the simulator's start epoch, loads the trajectory, arms it and
sends the timed schedule (in real time for host-timed profiles), while reading the receiver's
NMEA from a TCP port; conducted or shielded set-ups only.

Scoring (method `kshana-pro/testkit/v1`) is against the scenario's truth at each epoch
(moving or static). A metric whose data the log lacks is **not assessed**, never met. A
scenario passes when every required criterion is met. `level_supported` is the highest
resilience level (modelled on the DHS RPCF) whose criteria, and every lower level's, are met.

Each build directory holds per scenario `verdict.json` (`kshana-testkit-verdict/1`),
`scenario.toml`, `receiver-log.json` and `signature.json` (Ed25519 dossier signature), plus
`summary.json`, `summary.csv` and a signature over the whole build. A build is never
overwritten. `diff` verifies both builds' signatures when given `--trust`, and reports per
scenario `regression` (pass to anything worse), `fixed`, `now-run`, `worse`, `better`,
`unchanged`, `new`, `removed`, whether the scenario itself changed (SHA-256), and per criterion
the measured values with their trend. It exits 1 on a regression or a signature problem.

### Incident to regression

```sh
kshana-pro testkit from-incident --log field.nmea --incident incident.toml --out inc-2026-031.toml
```

The incident file (`kshana-testkit-incident/1`: kind, onset and end in log seconds, platform,
optional surveyed truth and known threat parameters, detection and recovery bars) and the
field log become a neutral scenario and the field log on the scenario clock
(`inc-2026-031.json`). Truth is the surveyed position, the mean of the clean fixes before the
onset (static), or the clean track bridged across the incident. Spoofing parameters are
fitted from the offsets during the incident; J/S, meaconing delay and a time offset without a
logged time error come from the incident file or a stated assumption. Every estimate is listed
in `[incident.derived]`. Running the scenario on the field log reproduces the incident's
verdict; every later build runs against the same scenario.

`from-library <id>` writes a threat-library scenario as a neutral scenario with default
criteria.

## 4. Replay fidelity

```sh
kshana-pro testkit fidelity capture.toml --scenario dp-gnss-jamming
```

A capture description (`kshana-testkit-capture/1`: bit depth, bandwidth, clipping fraction,
AGC mode, reference oscillator, receiver PLL bandwidth, recorder and replayer phase-noise
points, recorded and synchronised sensors) is scored against the scenario on five limits:
quantisation loss and jamming headroom, front-end saturation and AGC, LO phase-noise doubling
(recorder and replayer add in quadrature: +3 dB for identical units), missing synchronised
sensors, and the reference oscillator's time error for timing criteria. Grade `faithful`,
`limited` or `not-faithful`, a score out of 100, and one line per limit. Given to `run` with
`--capture`, the statement and a qualification in words go into every verdict. The rules and
their sources are in the header of `src/testkit/fidelity.rs`; they are rules of thumb, not a
calibration of the bench.

## 5. Scenario packs

| Pack | Scenarios | Modelled on |
|---|---|---|
| `dp-vessel` | position reference disagreement, slow drift, stable but wrong, GNSS jamming | IMCA IN 1730 themes (the note gives no test method) |
| `aviation` | post-spoof recovery, post-spoof time and position reset, jamming on approach | EASA SIB 2022-02 themes |
| `uas` | hover drag-off, jamming in transit, meaconing | published UAS spoofing and jamming events |
| `timing` | matched-power time push (threat library, TEXBAT ds3 profile), holdover under jamming, meaconing delay | threat library; ITU-T G.8272 holdover theme |
| `rpcf` | resilience test procedures for levels 1 to 4 | DHS Resilient PNT Conformance Framework levels; IEEE P1952 draft topics by subject |

`kshana-pro testkit packs` lists them; `show <pack/id>` prints one; `packs export <pack> --out <dir>`
writes them as files to edit. `sample <scenario> --behaviour good|bad` writes a SYNTHETIC
receiver log to try the runner.

## Not done here

* No receiver, simulator or replay bench has been run with the kit; the verdicts in the tests
  are on synthetic logs and a loopback simulator.
* The simulator profiles stay unconfirmed templates; a lab confirms its own and passes it with
  `--profile`.
* Environment models (ionosphere storms, multipath, obscuration) and non-GNSS sensors are
  reported as manual steps; the kit does not drive them.
* No spoofer signal is generated for any adapter, and gps-sdr-sim gets no counterfeit input.
* The live runner reads NMEA over TCP only (no serial, no UBX); other outputs go through the
  offline path after conversion to the device log.
* The fidelity statement covers the five limits above; it does not measure a bench.
