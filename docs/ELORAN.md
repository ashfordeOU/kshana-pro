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
