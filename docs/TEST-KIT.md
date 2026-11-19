# Test-as-code kit (research 2026-11-20, gaps 07, 08, 08b, 11, 12)

Receiver tests written once in a neutral form and compiled for whichever lab simulator is on
the bench. This part: the scenario spec, the compiler and validator, and the scenario packs;
the CI runner with signed verdicts, the regression diff, incident-to-regression and replay
fidelity follow in the next part. Code:
`src/testkit/`; CLI `kshana-pro testkit`; packs in `testkit/packs/`; tests `tests/testkit.rs`.

**Evidence label: MODELLED.** No receiver has been scored with the kit in this build. Pack parameters are
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
| `validate`, `packs [list]`, `show` | none | free |
| `compile`, `from-library`, `packs export` | `pro-test-kit` | Pro |

The feature has an explicit entry in `license::explicit_tier` and in the signed tier
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

## 3. Scenario packs

| Pack | Scenarios | Modelled on |
|---|---|---|
| `dp-vessel` | position reference disagreement, slow drift, stable but wrong, GNSS jamming | IMCA IN 1730 themes (the note gives no test method) |
| `aviation` | post-spoof recovery, post-spoof time and position reset, jamming on approach | EASA SIB 2022-02 themes |
| `uas` | hover drag-off, jamming in transit, meaconing | published UAS spoofing and jamming events |
| `timing` | matched-power time push (threat library, TEXBAT ds3 profile), holdover under jamming, meaconing delay | threat library; ITU-T G.8272 holdover theme |
| `rpcf` | resilience test procedures for levels 1 to 4 | DHS Resilient PNT Conformance Framework levels; IEEE P1952 draft topics by subject |

`kshana-pro testkit packs` lists them; `show <pack/id>` prints one; `packs export <pack> --out <dir>`
writes them as files to edit.

## Not done here

* No receiver or simulator has been run with the kit.
* The simulator profiles stay unconfirmed templates; a lab confirms its own and passes it with
  `--profile`.
* Environment models (ionosphere storms, multipath, obscuration) and non-GNSS sensors are
  reported as manual steps; the kit does not drive them.
* No spoofer signal is generated for any adapter, and gps-sdr-sim gets no counterfeit input.
