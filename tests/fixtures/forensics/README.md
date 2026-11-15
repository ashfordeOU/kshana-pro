# Incident forensics test inputs

Public, synthetic material only. Nothing here is a measurement or a real incident.

| File | Source |
|---|---|
| `training/*.toml` | The open engine's NMEA training library (`scenarios/training/` of the open repository, as shipped in the `kshana` 0.41.0 crate), copied unchanged. The tests generate the NMEA streams from them with `kshana::nmea_synth`; each scenario's instructor log says what was injected when. |
| `ais.csv`, `adsb.csv` | The open repository's synthetic interference-map inputs (`examples/interference-map/input/`, made by its `generate_inputs.py`, no random numbers), copied unchanged. Mid-Atlantic positions, made-up identifiers. |

No TEXBAT or JammerTest data is used or committed. The training scenarios are a development set
(the method was exercised on them while it was written), not the labelled set of roadmap V2.
