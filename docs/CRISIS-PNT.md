# Crisis PNT service-availability model

Module `src/crisis_pnt.rs`, command `kshana-pro crisis-pnt` (`src/cli/crisis_cmd.rs`), tests in
`tests/eloran_service.rs` (module `crisis`). Feature `pro-crisis-pnt-availability` (Pro).

A planning and evaluation tool. Given an outage scenario (GNSS denied over a polygon for a
duration) and the services that remain, it computes per grid cell and user class the modelled
service availability, accuracy class and, for timing users, the holdover against the denial. It
computes no navigation or timing solution and fuses nothing.

Reused unchanged:

* **eLoran**: the network planner of the eLoran crate (`kshana_eloran::plan::Planner`), on the
  stations marked `on_air`: field strength and SNR against the stated noise, weighted HDOP, 95 %
  horizontal accuracy, and availability over independent station outages at the class's
  accuracy target, with the chosen propagation model;
* **LEO coverage**: the open engine's Walker geometry (`walker::coverage_half_angle_rad`,
  `walker::street_half_width_rad`): continuous where the streets of coverage close and the
  latitude is within reach, else the expected fraction of time in view;
* **holdover**: the open engine's closed-form holdover for a clock class
  (`holdover::holdover_for_class`: TCXO, OCXO, RAFS, CSAC, USO) to the class's timing threshold.

The request names the area and grid, the denial (`start_utc`, `duration_h`, optional `polygon`),
the eLoran stations (`on_air`), the eLoran timing figure, the LEO service (geometry, and the
provider's accuracy, timing and availability with `source_of_figures`), and user classes
(`need` = `position` with `accuracy95_m`, or `timing` with `timing_ns` and `clock`; required
`availability`; allowed `sources`).

Per cell and class:

* the sources that meet the need: GNSS where not denied; holdover if the clock holds the
  threshold for the whole denial; eLoran where the planner's accuracy meets the target (timing:
  where a fix exists and the stated eLoran timing meets it); LEO where in view and the stated
  figure meets it;
* availability `1 − Π(1 − aᵢ)` over those sources (independent);
* the best figure and its class (A ≤ 10 m, B ≤ 100 m, C ≤ 1 km; T1 ≤ 100 ns, T2 ≤ 1 µs, T3 ≤ 1 ms);
* served or not at the required availability.

Per class: the share of the area and of the denied cells served, cells by source, and the
holdover and hours beyond it.

Limits: independent failures only; LEO figures as the provider states them; representative
clock classes. **MODELLED**. `crisis-pnt synth <dir>` writes a synthetic 48 h denial with four
eLoran stations (one off air), a LEO service and three user classes.
