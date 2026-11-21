# Maritime casualty reconstruction (grounding and collision)

Research 2026-11-20 gaps, backlog B0601. Module `src/forensics/casualty.rs`, command
`kshana-pro casualty` (`src/cli/casualty_cmd.rs`), tests `tests/casualty.rs`.

Builds on the maritime actor-vs-victim evidence pack (`kshana-pro forensics maritime`,
`src/forensics/maritime.rs`), which already gave interference episodes with bounded onsets,
AIS-vs-own-NMEA consistency and neighbour corroboration. What this adds: the true-vs-displayed
track, radar checks, the casualty geometry and a causation statement.

`casualty run <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-casualty-reconstruction`, Pro) reads:

| input | what |
|---|---|
| `nmea` | the VDR NMEA export: RMC/GGA/GSV and the ship's own gyro heading (HDT) and log speed (VHW) |
| `ais` | AIS CSV of the vessel and, for a collision, the other ship |
| `radar` | `timestamp,target,range_m,bearing_deg` (true bearings) to charted objects and to the other ship |
| `[hazard]` | grounding: the charted danger (`lat`, `lon`, `radius_m`) |
| `other_vessel` | collision: the other ship's AIS identifier and radar target name |
| `[[reference]]` | charted, radar-conspicuous objects the radar log names |
| `data_source` | where the data come from (required) |

Outputs, signed as one directory: `report.json` / `report.html`, `tracks.csv` (displayed and
reconstructed positions, 1-sigma, divergence, per `sample_s`), the embedded actor-vs-victim
pack under `interference/`, and the inputs with their SHA-256.

* **Displayed track**: the GNSS fixes the VDR recorded.
* **Reconstructed true track**: the displayed track outside interference; inside it, dead
  reckoning from the last clean fix on gyro heading and log speed. Its 1-sigma grows linearly
  with time from the stated heading, log and unknown-current errors (biases over the window).
* **Radar fixes** on charted objects are **independent checks** beside the reconstruction.
  Nothing is filtered, fused or matched to a chart.
* **Collision**: the radar's relative track of the other ship gives CPA/TCPA without GNSS.

Causation rules (fixed before the run; thresholds in `[params]`, written into the pack):

| causation | when |
|---|---|
| `interference` | an episode in the look-back window; divergence at the casualty above k sigma; displayed track clear of the danger by more than `safety_m` while the reconstructed one met it; no independent means showed the error for `min_warning_s` |
| `interference_with_navigational_factor` | as above but radar showed the error (or a collision course) for `min_warning_s` or more; or the ship ran without a GNSS fix into the casualty |
| `navigational_error` | no interference (or none that moved the displayed position beyond its uncertainty) and the displayed picture itself met the danger, or radar showed a collision course for the warning time |
| `undetermined` | otherwise |

Each statement carries the divergence and its sigma, the onset bounds of each episode, a
rule-based confidence (a ranking aid, not a probability) and the limits.

Fixtures: **SYNTHETIC** only (`casualty synth <case> <dir>`: `grounding-spoofed`,
`grounding-spoofed-radar`, `grounding-jammed`, `grounding-nav-error`, `collision-nav-error`).
No published casualty data are bundled or were used. Evidence label **MODELLED**. An engineering
reconstruction, not a finding of fact or of blame. Nothing here estimates where an interference
source was.
