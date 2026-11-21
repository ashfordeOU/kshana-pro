# Combined eLoran and GNSS signal-file scenario

Module `src/hybrid_sim.rs`, command `kshana-pro hybrid-sim` (`src/cli/hybrid_sim_cmd.rs`),
tests in `tests/eloran_gnss_sdr.rs` (module `hybrid`). Feature `pro-hybrid-signal-sim` (Pro).

Already there and reused unchanged:

* the eLoran crate's scene simulator (`kshana_eloran::Scenario`, `Scene::run`): 100 kHz
  LORAN-C/eLoran pulses, groundwave and skywave propagation, station faults, SigMF output and a
  per-pulse truth CSV (`kshana-pro eloran run`);
* the software GNSS bridge (`src/eloran_gnss.rs`, `src/eloran_gnss_sdr.rs`): GPS L1 C/A and
  Galileo E1-B complex baseband from the open engine's GNSS IQ scene on the eLoran scenario's
  trajectory and time base, a C/N0 degradation timeline, and a SigMF file sink.

What this adds: one request that drives both, faults on one scene clock, and one shared truth.

`hybrid-sim run <request.toml> --out <dir> --signer <name> --key-file <file>` reads the eLoran
scenario, a RINEX 3 navigation file, GNSS settings and `[[fault]]` tables:

| `kind` | fields | realised as |
|---|---|---|
| `asf-error` | `chain`, `station`, `from_s`, `to_s`, `error_ns` | an added delay on that station's signal over the window: at the receiver, an ASF error of that size |
| `skywave` | `delay_us`, `ratio_db` | a fixed skywave through the eLoran propagation model, whole run |
| `gnss-outage` | `from_s`, `to_s`, optional `sats` | no signal from those satellites (all if absent) |
| `gnss-interference` | `from_s`, `to_s`, `js_db`, optional `sats` | the C/N0 drop of `(C/N0)_eff⁻¹ = (C/N0)⁻¹ + (J/S)/(Q·R_c)` at `[gnss] reference_cn0_dbhz` (45) and `q_rc_hz` (1.023e6) |

No interference waveform is synthesised. The open engine's test-condition format records
interference as metadata only, and this follows it: the effect on the receiver input, a lower
C/N0, is what is written.

Outputs, signed as one directory: `eloran.sigmf-*` and `eloran.truth.csv`, `gnss.sigmf-*` and
`gnss.truth.csv`, `truth.json` and `truth-timeline.csv`. `truth.json` (`kshana-pro-hybrid-sim-truth/1`)
holds:

* the common start (sample 0 of both files is `start_utc`; GPS week and time of week);
* each stream's sample rate, centre frequency and sample count;
* every fault with its window, layer and realisation;
* the SHA-256 of every output file.

`truth-timeline.csv` gives, per step, the eLoran faults active and the GNSS state (nominal,
outage, interference with its C/N0 offset). Per-satellite detail is in `gnss.truth.csv`.

**Files only**: the module opens no device and has no transmit path. A GNSS file is for a
software receiver or a conducted replay, never a radiating antenna (the bridge's output
policy). MODELLED: synthetic signals. `hybrid-sim synth <dir> --nav <rinex>` writes a two-station
scenario with all four fault kinds.
