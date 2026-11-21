# Observation scheduling for space-debris sensors

Module `src/debris_obs.rs`, command `kshana-pro debris-obs` (`src/cli/debris_cmd.rs`), tests in
`tests/space_ops_evidence.rs` (module `debris_obs`). Feature `pro-debris-obs-schedule` (Pro).

Already there and reused unchanged:

* the open engine's SGP4 and TLE parsing (`kshana::sgp4`, `kshana::tle`), CCSDS OEM parsing
  (`kshana::oem::parse_oem`), frames (`teme_to_ecef`, `look_angles`), the Sun
  (`kshana::ephem::sun_position`);
* the open engine's orbit-determination tools: analytic measurement partials for azimuth,
  elevation and range (`orbit_determination::measurement_with_partials`) and the variational
  state-transition matrix (`orbit_determination::propagate_with_stm`, two-body + J2);
* the space-operations evidence packs (`src/space_ops.rs`), whose TLE line helper is now public.

What this adds: a signed observation-scheduling evidence pack. Planning only: no observation
is taken and no orbit is determined from data.

`debris-obs plan <request.toml> --out <dir> --signer <name> --key-file <file>`:

* **sensor**: location, kind (`optical` or `radar`), minimum elevation, optional azimuth sector
  (wrapping through north when `az_min_deg > az_max_deg`), optional maximum range, Sun depression
  for an optical sensor, track and slew times, measurement interval, angle noise and (radar) range
  noise;
* **targets**: a TLE (`tle_line1`, `tle_line2`) or a CCSDS OEM file (`oem`, `oem_object`, TEME),
  a priority (1 = highest) and the prior position and velocity 1-sigma.

Outputs:

1. **Visibility windows** sampled every `step_s`: the target above the minimum elevation, inside
   the azimuth sector and range. For an optical sensor, also the sensor in darkness and the target
   sunlit (cylindrical shadow). Windows shorter than one track are dropped.
2. **Schedule**: targets in priority order, each window in time order. A track of `track_s` is
   placed at the earliest time inside the window that clears every booked track by `slew_s`; at
   most `max_passes_per_target` per target. Targets left out say why.
3. **Expected covariance improvement per scheduled track**: Fisher information
   `Σ Φᵀ Hᵀ W H Φ` of the track's measurements (azimuth and elevation, plus range for a radar)
   added to the stated prior. Each track reports the position 1-sigma before and after, and the
   ratio. Each track is evaluated against the prior at its own epoch.

Limits: sidereal-time rotation without polar motion, refraction or light time; no Moon, weather
or target brightness; no process noise or biases; a greedy scheduler. MODELLED.
`debris-obs synth <dir>` writes a radar and four synthetic debris TLEs.
