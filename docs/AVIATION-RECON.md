# Aviation and drone event reconstruction

Research 2026-11-20 gaps, backlog B0602. Module `src/aviation/event_recon.rs`, command
`kshana-pro aviation-recon` (`src/cli/aviation_recon_cmd.rs`), tests `tests/aviation_recon.rs`.

Builds on the FDM GNSS event detector (`src/fdm.rs`, `forensics fdm`), whose events go on the
timeline unchanged. What this adds: putting the sources on one time base and explaining their
altitude disagreements.

`aviation-recon run <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-aviation-event-recon`, Pro). Inputs:

| input | columns |
|---|---|
| `fdm` | the FDM detector's columns; needs `time`, `gnss_alt_ft`, `baro_alt_ft` (lat/lon for tracker alignment) |
| `adsb` | `timestamp,lat,lon,alt_baro_ft` and optionally `alt_geom_ft` (ellipsoid), `aircraft_id`, `nic`, `nacp` |
| `tracker` | `timestamp,lat,lon,alt_ft` (GNSS, MSL): a portable tracker or a drone's flight log |
| `isa_dev_c` | ISA temperature deviation, if known |
| `geoid_ft` | geoid height above the ellipsoid; else estimated in the calibration window |

1. **Alignment**: whole-second clock offset per source by least squares: ADS-B barometric altitude
   against FDM barometric altitude over the record; tracker position against FDM GNSS position in
   the calibration window (`calib_s`, taken as clean).
2. **Comparison**: per FDM row, GNSS-minus-baro for each GNSS-derived source against what is
   explained: the calibration offset, the first-order ISA term
   `ISA dev / 288.15 × (h − h_cal)`, and the datum for ADS-B geometric height.
3. **Episodes** where a residual exceeds `alt_tol_ft` (or barometric sources disagree beyond
   `baro_tol_ft`, or GNSS altitude is missing), each classed as `common_gnss_divergence`,
   `single_source`, `barometric_disagreement` or `gnss_loss`.

The pack reports how much the atmosphere, the datum and the clock offsets accounted for, and marks
FDM detector events that the explanation absorbs.

**Alignment and comparison only**: no source is fused into a navigation solution and nothing
estimates an aircraft position or an interference source. **MODELLED**: exercised on a SYNTHETIC
King Air-type flight (`aviation-recon synth`: climb to FL250 in ISA+15, ADS-B clock +2 s, tracker
−5 s, a 2,000 ft GNSS altitude divergence in cruise, reproduced as one common-GNSS episode). No
operator or published event data were used; where there is no truth the pack says what the data
show, not what happened.
