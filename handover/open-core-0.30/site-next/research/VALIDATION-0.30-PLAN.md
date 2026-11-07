# Validation 0.30 plan: 51 target rows in 10 parallel batches

Written 2026-09-30, after wave 0. Source study: `research/VALIDATION-UPLIFT.md`. Founder decision
of 2026-09-30: "Get it to 134/223 for 0.30", which means adopting both policies P1 and P2.

**134 is the ceiling, not a quota.** A row is promoted only if its oracle agrees within the tolerance
written below. The expected outcome, allowing for the high-risk rows the study names, is well under
134; five rows are already known to depend on a decision or a missing input (see "Known blockers").

## State at the start

- Branch `feat/validation-uplift` in `~/Code/kshana-wt-validation`, cut from `feat/integration-bc`
  (5f6f1ee6). Wave-0 policy commit **3ed491b5**: the promotion rule is written in `docs/VALIDATION.md`
  ("The promotion rule"), and every VALIDATED row declares its oracle kind in
  `validated_oracle_basis()` in `src/verification.rs`, enforced by unit tests there and by
  `tests/verification_rows_declare_an_oracle_basis.rs`.
- Matrix: 83 VALIDATED, 136 MODELLED, 4 PARTNER (re-derive with the parser, never from this line).
  Three existing VALIDATED rows are flagged by the rule (Integrity (RAIM/ARAIM/SBAS) and the two
  Standish/Horizons rows); their fate is an owner decision and is outside this plan's count.
- Oracle toolchain and datasets: `~/Code/kshana-oracles/` (README.md there; `source env.sh` before
  any generator; `data/MANIFEST.md` lists every dataset with licence, size and SHA-256).
- Wave 0 found one small repository defect: the committed `xval/*/Cargo.lock` files pin kshana
  0.22.0, so `cargo build --locked` fails in every xval crate. `setup.sh` builds a copy under
  `~/Code/kshana-oracles/build/xval-src/` instead. B1 and B2 (the only batches that edit xval
  crates) should refresh the lock of each crate they touch, in its own commit.
- Row ids: `M001`-`M136` number the MODELLED rows in matrix order, as in the study; the matrix
  row number is given beside each.

## The honesty rule for every batch agent

1. **Fix the tolerance before the first comparison.** Write it in the test, and in the promotion
   record, before running the oracle. A tolerance changed after seeing the result is a new
   comparison and must be disclosed as such in the record; it cannot promote the row.
2. **Promote only on agreement.** A row is promoted only if the oracle agrees within the stated
   tolerance, on the whole claim the row makes (or on a claim narrowed in writing before the run).
3. **Disagreement is a result, not a failure.** If the oracle disagrees, the row stays MODELLED, and
   the disagreement is recorded as a finding: the measured gap, the oracle, the tolerance, and the
   likely cause. The integrator writes it into the row's oracle text in `src/verification.rs`
   (which regenerates `docs/MODELLED-RATIONALE.md`; that file is generated, never hand-edited) and
   into `CHANGELOG.md` under "Findings". Report it; do not tune it away.
4. **What never counts** (from the rule in `docs/VALIDATION.md`): our own second implementation, a
   numpy script re-evaluating the same closed form, agreement with our own earlier output or our
   own papers, a transcription of a citation's constants, a design choice, a post-hoc tolerance.
   `~/Documents/PNT_Research` is read-only and its `_artifacts/data/*_table.csv` files are
   prototype outputs, never oracles.
5. **Licences.** Nothing under `~/Code/kshana-oracles/data` is vendored wholesale. A batch may commit a
   small fixture slice with a `NOTICE.md` giving source URL, licence, retrieval date and SHA-256.
   GPL-3.0 data (JammerTest 2024) needs a NOTICE entry saying so; CC BY data needs attribution;
   tools under GPL/LGPL/EUPL are run as tools and never linked or copied into the crate.

## How batches stay out of each other's files

- **Nobody but the integrator edits `src/verification.rs`, `CHANGELOG.md`, `README.md`, the
  generated docs (`docs/VERIFICATION-MATRIX.md`, `docs/MODELLED-RATIONALE.md`, the ledger JSON), or
  any existing test file.** Every new oracle comparison goes in a **new** test file
  `tests/<slug>_oracle.rs` with its fixture in `tests/fixtures/<slug>_oracle/` (generator script,
  committed reference values, `NOTICE.md`).
- A batch edits `src/` only to fix a defect its oracle exposes, only in the modules listed for it
  below, and never a `mod.rs`. A defect fix is itself a finding for the CHANGELOG.
- Each batch works in its own worktree and branch off `feat/validation-uplift`
  (`git -C ~/Code/kshana worktree add ~/Code/kshana-wt-vu-bN -b feat/vu-bN feat/validation-uplift`),
  commits green checkpoints, and never uses `git stash`, `reset`, `clean` or `checkout -- <file>`.
- Each batch writes one **promotion record** per row to
  `~/Code/kshana-site-next/research/validation-0.30/records/<Mnnn>.md` (outside the repo): verdict
  (AGREES / DISAGREES / BLOCKED), tolerance and when it was fixed, measured agreement, oracle and
  dataset with SHA-256, the proposed row text (capability, tests, oracle, `OracleBasis` kind and
  oracle test for `validated_oracle_basis()`), and any finding.
- **Integration, one agent, after the batches:** merges the batch branches, applies each AGREES
  record to `src/verification.rs` (status, `oracle_kind: ExternalDataset`, oracle text naming source
  and tolerance, tests field, and the `validated_oracle_basis()` entry), writes every DISAGREES
  record into the row's oracle text as a finding, regenerates the artifacts with
  `cargo run --bin gen_validation_artifacts`, updates CHANGELOG, and runs the full gate. The count is
  then read from the generated `docs/VERIFICATION-MATRIX.md` header, never typed.

Machine rules for every batch: every build and install under `nice -n 15` with `CARGO_BUILD_JOBS=3`;
scratch under `~/Code/kshana-site-next/scratch/validation/`, never `/tmp`; git identity
`-c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com`; no trailers;
no AI or vendor names in repository content; never `--no-verify`; no push.

## The batches

Paths in the "Data" column are relative to `~/Code/kshana-oracles/`. "Test" is the new test the
batch writes (file and function); the row's promotion rests on it. Tolerances are the study's,
fixed here before any comparison has been run.

### B1. Lunar and solar-system geometry from NAIF kernels (5 rows)

Modules (defect fixes only): `src/lunar_service.rs`, `src/lunar_interop.rs`,
`src/lunar_ephemeris.rs`, `src/lunar_orientation.rs`, `src/body.rs`. Drivers: `xval/anise-service-geometry`
and `xval/anise-frames` (B1 is the only batch that edits these two crates). Generators run with
`.venv/bin/python` (spiceypy 8.2.0, oem 0.4.5) and Orekit 12.2 (`orekit/cp.sh`).

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M045 (row 100) | Lunar joint communications-and-navigation geometry | ANISE 0.10 azimuth, elevation and range from a location on the Moon in MOON_PA_DE440 (Library) | `data/naif/de440s.bsp`, `moon_pa_de440_200625.bpc`, `moon_de440_250416.tf`, `pck00011.tpc` | azimuth and elevation 1e-6 deg, range 1 mm | `tests/lunar_service_geometry_oracle.rs::look_angles_match_anise_at_selenographic_sites` |
| M047 (row 102) | Lunar interoperability export, scoped to interchange conformance | the independent `oem` 0.4.5 parser plus the Orekit 12.2 CCSDS OEM parser as a second reader (Library) | none (Kshana-written OEM, two readers) | tokens exact; states to the written precision | `tests/lunar_interop_oem_oracle.rs::lunar_oem_decodes_identically_in_two_independent_readers` |
| M076 (row 138) | Lunar service volume from real, retrieved constellation geometry | published coverage and DOP statistics for the same geometries: LANS reference constellation, NASA NTRS 20250009447; LNCSS case studies, NAVIGATION 70(4) navi.613, CC BY (Reference) | fetch both papers to `data/papers/` (not in wave 0: NTRS and ION pages need a browser) | coverage within 2 percentage points, median PDOP within 5 %, at the papers' mask, grid and span | `tests/lunar_service_volume_published_oracle.rs::coverage_and_pdop_match_the_published_statistics` |
| M090 (row 155) | DE440 lunar principal-axis orientation provider | SPICE (spiceypy `pxform`) evaluating `moon_pa_de440_200625.bpc` directly at 2 000 random off-node epochs (Library) | `data/naif/moon_pa_de440_200625.bpc`, `moon_de440_250416.tf`, `naif0012.tls` | rotation angle at most 1.7e-5 rad (30 m at the mean lunar radius), unless the module already documents a tighter interpolation bound, in which case that bound | `tests/lunar_pa_orientation_spice_oracle.rs::interpolated_rotation_matches_direct_kernel_evaluation_off_node` |
| M109 (row 182) | Physical constants of every solar-system body | NAIF `pck00011.tpc` and `gm_de440.tpc` read by spiceypy `bodvrd` (Reference) | `data/naif/pck00011.tpc`, `data/naif/gm_de440.tpc` | GM, radii, pole and prime-meridian constants equal to the printed digits; J2 only where a cited paper prints it (else J2 is scoped out of the claim) | `tests/body_constants_naif_oracle.rs::constants_match_naif_pck00011_and_gm_de440` |

### B2. Lunar radiometry, datum and timescale (5 rows)

Modules: `src/lunar_vlbi.rs`, `src/lunar_combination.rs`, `src/lunar_identifiability.rs`,
`src/lunar_datum.rs`, `src/lunar_llr_geometry.rs`, `src/lunar_gauge.rs`. Driver: `xval/anise-lunar-od`
(B2 only). Order inside the batch: M038 before M043, which reuses its delay route.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M038 (row 90) | Lunar geodetic VLBI | ANISE converged light time (aberration CN) from a lunar surface point to two Earth stations; the difference is the near-field delay (Library) | `data/naif/de440s.bsp`, `moon_pa_de440_200625.bpc`, `earth_latest_high_prec.bpc` (the copy fetched 2026-09-30), `data/iers/finals2000A.all` | delay 1 ps over one day of epochs and three baselines; partials against a central difference of the ANISE delay to 1e-6 relative | `tests/lunar_vlbi_anise_oracle.rs::near_field_delay_matches_anise_light_time_difference` |
| M043 (row 95) | Lunar joint multi-technique OD and clock | Orekit 12.2 native Range and InterSatellitesRange models generate the observations, Orekit BatchLSEstimator and Kshana both solve; VLBI through the M038 route (Library) | `orekit/`, `data/naif/` | recovered states 1e-6 m, clocks 1e-15 s, on identical, independently generated observations | `tests/lunar_joint_od_orekit_models_oracle.rs::orekit_generated_observations_recover_the_same_states` |
| M088 (row 153) | Lunar datum null-space classification | Sosnica et al. 2025, arXiv:2510.15484, reported correlation and CRLB magnitudes under the paper's geometry (Reference) | `data/papers/arXiv-2510.15484.pdf` | correlations within 0.05, CRLB within 20 % | `tests/lunar_datum_sosnica_oracle.rs::correlation_and_crlb_match_the_published_geometry` |
| M091 (row 156) | Lunar LLR datum geometry substrate | the 337 archived ILRS normal points already committed (`tests/fixtures/lunar_llr/normal_points`), with DE440 through ANISE (Measured) | committed normal points; `data/naif/de440s.bsp`, `moon_pa_de440_200625.bpc` | one-way range residual RMS at most 10 m, with the Shapiro delay and a Mendes-Pavlis tropospheric delay applied (both published closed forms), no tides | `tests/lunar_llr_geometry_range_oracle.rs::reflector_ranges_match_ilrs_normal_points` |
| M093 (row 159) | Relativistic clock-rate to frame coupling (P1) | Ashby and Patla 2024, AJ 167:149: L_m = 3.13881e-11; Kshana computes d_alpha/d_scale = +U_moon/c^2 from the paper's GM and radius (P1) | the paper (already the oracle of the lunar coordinate time row) | 1e-4 relative | `tests/lunar_rate_frame_coupling_oracle.rs::rate_frame_jacobian_matches_the_published_self_potential` |

### B3. Time, clocks and Earth orientation (8 rows)

Modules: `src/clock_state.rs`, `src/timetransfer.rs`, `src/timetransfer_adv.rs`, `src/frame_eop.rs`,
`src/realtime_frame_eop.rs`, `src/eop.rs`, `src/integrity/hetero_budget.rs`, `src/telecom_timing.rs`.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M002 (row 42) | Onboard clock state estimation | IGS final 30 s clocks of GPS Block IIF rubidium satellites; Q from the measured ADEV of the first half, scored on the held-out second half (Measured) | `data/realdata/igs/IGS0OPSFIN_2025229..242*_CLK.CLK.gz` (14 days) | one-step and 1 h prediction errors inside the filter's own 95 % chi-square band on at least 90 % of epochs; NEES mean in [2.4, 3.6] | `tests/clock_state_igs_holdout_oracle.rs::three_state_filter_is_consistent_on_held_out_igs_clocks` |
| M003 (row 43) | Time-transfer error budgeting | BIPM Circular T Section 6 link uncertainties uA/uB by technique (TWSTFT, GPS PPP, GPS CV) (Reference) | `data/bipm/circular-t/cirt.405`-`cirt.464` | each technique's budget total within a factor 1.5 of the published uA; the budget inputs are taken from the literature and committed in their own commit before Circular T is read | `tests/time_transfer_budget_circular_t_oracle.rs::budget_totals_are_within_1p5x_of_circular_t_ua` |
| M034 (row 86) | Joint UT1 and polar-motion error | IERS Bulletin A stated prediction accuracy, S_xy = 0.00068 (MJD-MJD0)^0.80 arcsec and S_t = 0.00025 (MJD-MJD0)^0.75 s, against measured predicted-minus-final (Reference) | `data/iers/bulletinA/bulletina-xxxvi-*.txt` to `-xxxix-*.txt` (2023 to 2026), `data/iers/finals2000A.all` (frozen 2026-09-30 copy) | measured RMS per horizon within a factor 1.5 of the formula, over at least two years of vintages | `tests/joint_eop_error_bulletin_a_oracle.rs::measured_prediction_error_matches_the_iers_accuracy_formula` |
| M035 (row 87) | Offline default Earth-orientation input is a real IERS product | astropy 8.0.1 `utils.iers` finals2000A reader, which reads the I/P vintage flags independently (Library) | the embedded extract; `data/iers/finals2000A.all` for context | row counts per vintage and MJD span exact | `tests/embedded_eop_census_astropy_oracle.rs::vintage_census_matches_astropy` |
| M073 (row 135) | Operational-style Earth-orientation prediction error | archived Bulletin A vintages scored against later finals, and the 2nd EOP PCC published MAE ranges (day-10 UT1-UTC MAE 0.36 to 3.13 ms, 56 methods) (Reference) | `data/iers/bulletinA/`, `data/iers/finals2000A.all`, `data/iers/EOP_20_C04_one_file_1962-now.txt`, `data/iers/eoppcc2_s40623-022-01753-9.pdf` | Kshana predictor MAE within 1.5 times Bulletin A's own MAE at 1 to 10 days over the same windows, and inside the 2nd EOP PCC range | `tests/operational_eop_predictor_bulletin_a_oracle.rs::predictor_mae_is_within_1p5x_of_bulletin_a_and_inside_the_pcc_range` |
| M083 (row 147) | Heterogeneous UTC(k) traceability-bias integrity overbound | BIPM UTC-UTC(k) series for about 80 laboratories with their published uncertainties (Measured) | `data/bipm/utclab/utc-*`, `data/bipm/circular-t/` | empirical exceedance of the overbound b at the 1e-2 tail at or below the allocation, with a binomial 95 % confidence bound | `tests/hetero_budget_utc_k_oracle.rs::overbound_exceedance_is_within_the_allocation_on_real_utc_k` |
| M101 (row 170) | ITU-T telecom synchronisation masks (P1, at risk) | ITU-T G.8272 (07/2025), G.8272.1, G.8273.2, G.8271.1: only mask values the Recommendations print as numbers at stated tau, evaluated by Kshana's mask functions (P1) | download the Recommendations to `data/papers/itu-t/` (free, but served through an interactive page) | exact to the printed digits | `tests/telecom_masks_itu_printed_values_oracle.rs::mask_values_at_printed_tau_match_the_recommendations` |
| M102 (row 171) | Oscillator holdover presets (claim must narrow) | measured units of the same models: the 5071A phase record; a published measured SA.45s CSAC curve (Measured) | `data/realdata/cs5071a/5071A_phase.txt`; the CSAC curve must be found and fetched | one-sided: the measured unit's ADEV at or below the preset curve at every tau | `tests/oscillator_presets_measured_oracle.rs::measured_units_sit_at_or_below_their_preset_curves` |

### B4. Orbital, attitude and entry dynamics, and the thermosphere (8 rows)

Modules: `src/launch.rs`, `src/eo_payload.rs`, `src/cw_dynamics.rs`, `src/bplane.rs`,
`src/attitude_budget.rs`, `src/attitude_dynamics.rs`, `src/reentry.rs`, `src/space_weather.rs`.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M018 (row 62) | Launch-window and ascent geometry | Orekit 12.2 KeplerianOrbit built from the burnout state at the site with the computed azimuth reads back the inclination; Orekit event detection gives the daily plane-crossing times (Library) | `orekit/` | inclination 1e-9 rad; opportunity times 1 s | `tests/launch_geometry_orekit_oracle.rs::burnout_azimuth_reproduces_the_target_inclination_in_orekit` |
| M020 (row 64) | EO payload footprint and coverage geometry | Skyfield 1.54 / SGP4 2.26 and a WGS-84 geodesic, already in `tests/eo_payload_coverage_reference.rs` (Library) | none | the existing 1 % node-spacing and 0.3 deg limb bars, declared as the row's accuracy (they predate this decision, so they are not post-hoc) | the existing `tests/eo_payload_coverage_reference.rs` (no new file; the integrator only relabels) |
| M057 (row 112) | Clohessy-Wiltshire / Hill relative motion | Orekit numerical two-body propagation of chief and deputy, differenced into LVLH (Library) | `orekit/` | below 1e-3 m for a 100 m separation over a third of an orbit, with the linearisation bound rho^2/r stated | `tests/cw_dynamics_orekit_oracle.rs::cw_matches_nonlinear_orekit_within_the_linearisation_bound` |
| M060 (row 117) | B-plane targeting and patched-conic gravity assist | GMAT BdotT/BdotR/BPlane parameters, run headless (Library) | GMAT is not installed by wave 0 (see blockers) | B.T and B.R 1e-6 km; turn angle 1e-9 rad over a grid of hyperbolic states | `tests/bplane_gmat_oracle.rs::bplane_matches_gmat_over_a_hyperbolic_grid` |
| M021 (row 66) | 3-DOF attitude and pointing error budget | Basilisk 2.9.1 GravityGradientEffector (ISC) (Library) | `.venv` (bsk) | torque vector 1e-12 relative over random attitudes and inertia tensors; worst case at the 45 deg peak | `tests/attitude_gg_torque_basilisk_oracle.rs::gravity_gradient_torque_matches_basilisk` |
| M056 (row 111) | Torque-free rigid-body attitude dynamics | Basilisk spacecraft hub rigid-body propagation (Library) | `.venv` (bsk) | quaternion and body rates 1e-9 over 1e4 s torque-free, general inertia | `tests/attitude_dynamics_basilisk_oracle.rs::torque_free_motion_matches_basilisk` |
| M019 (row 63) | Ballistic re-entry corridor (Allen-Eggers) | reconstructed ballistic entries: Stardust SRC (Desai and Qualls, AIAA 2008-1198), Genesis and Hayabusa reconstructions (Measured); Allen and Eggers NACA Report 1381 for the model definition | `data/papers/NACA-TR-1381_Allen-Eggers.pdf`; the reconstructions must be obtained (AIAA is paywalled; prefer NASA NTRS versions) | peak deceleration and velocity at peak g within 15 % of the reconstruction | `tests/reentry_reconstruction_oracle.rs::peak_deceleration_matches_reconstructed_entries` |
| M017 (row 60) | Space weather and activity-driven thermospheric density | accelerometer-derived densities, CHAMP, GRACE, GRACE-FO, Swarm, TU Delft (Measured) | not fetched: thermosphere.tudelft.nl was unreachable on 2026-09-30 | activity-corrected density within a factor 2 of measured orbit-averaged density at 400 to 500 km, across solar minimum and maximum | `tests/space_weather_density_accelerometer_oracle.rs::activity_corrected_density_within_factor_two` |

### B5. Signals, spectrum and Earth GNSS at the Moon (4 rows)

Modules: `src/navsignal.rs` (M004 only), `src/sigmf.rs`, `src/earth_gnss_lunar.rs`, `src/tracking_loop.rs`.
`src/spectrum.rs` carries VALIDATED unit tests: edit it only for a defect, and never its tests.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M004 (row 44) | Nav-signal modulation and code-tracking analysis | published SSC and RMS-bandwidth tables: Betz 2001, NAVIGATION 48(4); Hein et al. 2006, InsideGNSS MBOC (Reference / P1). Multipath envelope stays outside the claim | the papers (Betz is paywalled; Hein is free on the web) | SSC 0.1 dB, Gabor bandwidth 0.5 %, at the stated front-end bandwidths | `tests/nav_signal_ssc_published_oracle.rs::ssc_and_gabor_bandwidth_match_the_published_tables` |
| M106 (row 177) | SigMF input and output, and Welch spectra of complex IQ | scipy 1.18.1 `signal.welch` with identical window, overlap and density scaling; sigmf-python 1.13.0 writing and reading recordings; the SigMF v1.2.6 schema validating Kshana's metadata (Library) | `data/sigmf/SigMF-1.2.6/`, `data/sigmf/sigmf-python-1.13.0/`. Upstream publishes no standalone set of example recordings (checked 2026-09-30), so the externally produced recording is one sigmf-python writes | PSD 1e-12 relative; decoded samples exact; metadata fields identical; schema-valid | `tests/sigmf_welch_oracle.rs::welch_and_sigmf_io_match_scipy_and_sigmf_python` |
| M039 (row 91) | Earth GNSS at lunar distance | LuGRE flight data (C/N0 at the Moon), CC BY 4.0, with the measured GPS IIR/IIR-M transmit patterns substituted for the Airy pattern (Measured) | `data/lugre/LuGRE.zip` (contents in `LuGRE.contents.txt`), `data/navcen/GPS_IIR_IIR-M_LM/` (Appendix B data) | per-link predicted C/N0 within +/-3 dB of LuGRE measured, for IIR and IIR-M satellites | `tests/earth_gnss_lunar_lugre_oracle.rs::predicted_cn0_matches_lugre_for_iir_satellites` |
| M068 (row 128) | Tracking-loop loss of lock and spoof pull-in | GNSS-SDR (GPL-3.0, run as a tool) tracking the same synthetic IF at calibrated C/N0 (Library) | GNSS-SDR is not installed by wave 0 (see blockers) | PLL/DLL jitter within 10 %; loss-of-lock C/N0 within 1 dB-Hz | `tests/tracking_loop_gnss_sdr_oracle.rs::jitter_and_loss_of_lock_match_gnss_sdr` |

### B6. GNSS positioning, ionosphere and constellation availability (5 rows)

Modules: `src/lambda.rs`, `src/leo_fusion/joint_pvt.rs`, `src/leo_fusion/ntn.rs`, `src/leo_pass.rs`,
`src/constellation.rs` (coverage functions only; its VALIDATED unit tests are not touched).

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M059 (row 116) | Carrier-phase integer ambiguity resolution (LAMBDA) | RTKLIB v2.4.2-p13 `lambda()` compiled from C (driver pattern `tests/fixtures/time_transfer_error_budgeting/rtklib_driver.c`) (Library) | `RTKLIB/src/lambda.c` | fixed integer vector identical on 300 random covariances; the two best squared norms to 1e-9 relative | `tests/lambda_rtklib_oracle.rs::ils_solution_matches_rtklib_lambda_on_300_covariances` |
| M127 (row 214) | Joint GNSS and LEO pseudorange positioning with inter-system biases | ITRF2020 station coordinate (propagated with its velocity) and RTKLIB `rnx2rtkp` single-point solutions on the same observations (Reference + Library). The committed ABMF excerpt is GPS-only, two epochs: fetch a multi-GNSS daily RINEX 3 file and its broadcast navigation for an IGS station in ITRF2020 (BKG `obs_v3`, `BRDC_v3`) | `data/igs/ITRF2020_GNSS.SSC.txt`, `RTKLIB/app/rnx2rtkp/gcc/rnx2rtkp` | position within 3 m of the ITRF2020 coordinate at 95 % of at least 100 epochs; inter-system bias within 1 ns of RTKLIB's | `tests/joint_pvt_itrf_rtklib_oracle.rs::multi_gnss_fix_matches_itrf2020_and_rtklib` |
| M129 (row 216) | 5G NTN positioning accuracy from signal bandwidth (P1, at risk) | a published worked figure of the time-of-arrival CRLB from RMS bandwidth, printed as a number and computed independently of Kshana (Kay 1993 ch. 3, Rife and Boorstyn 1974 give only closed forms; a 3GPP NTN positioning study or a textbook example is needed) (P1) | to be found | exact to the printed digits | `tests/ntn_crlb_published_value_oracle.rs::toa_crlb_matches_the_published_worked_value` |
| M136 (row 223) | Ionosphere sounding: slant TEC from a dual-band delay | CODE global ionosphere map and CODE DCBs on the real ABMF dual-frequency observations (Reference) | `data/igs/CODG1330.18I.Z`, `data/igs/P1P21805.DCB.Z`, `data/igs/P1C11805.DCB.Z`; committed `tests/fixtures/igs/ABMF00GLP_R_20181330000_01D_30S_MO.rnx` | geometry-free slant TEC after DCB correction within 3 TECU of the GIM-derived value (GIM accuracy is itself 2 to 8 TECU; say so in the record) | `tests/geometry_free_tec_gim_oracle.rs::slant_tec_matches_the_code_gim_within_3_tecu` |
| M111 (row 186) | Coverage and DOP maps for arbitrary designs | published availability specifications: BeiDou OS Performance Standard v3.0 (PDOP at most 6 at a 5 deg mask, availability at least 99 %), Galileo OS SDD DOP availability (Reference, one-sided). Moon, Mars and arbitrary designs stay outside the claim | the two documents, to `data/papers/` | one-sided: the preset constellations meet the published availability | `tests/constellation_availability_published_spec_oracle.rs::presets_meet_the_published_dop_availability` |

### B7. LEO navigation message (3 rows)

Modules: `src/leo_navmsg/fit.rs`, `src/leo_navmsg/truth.rs`, `src/leo_navmsg/elements.rs`,
`src/leo_navmsg/services.rs`. `src/leo_navmsg/sisre.rs` and `codec.rs` carry VALIDATED tests: not touched.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M120 (row 206) | LEO broadcast-ephemeris fitter and SISRE against fit interval | real LEO precise orbits as truth (GRACE-FO GNV1B from PO.DAAC, or Sentinel-3/6 POD) with Liu et al. 2025 SISRE, Remote Sensing 17(16):2894, CC BY (Measured + Reference) | not fetched: PO.DAAC and Copernicus need an account; MDPI refuses scripted downloads (fetch in a browser) | fitted SISRE within 1.5 times the published value at matching altitude and arc | `tests/leo_navmsg_fit_real_orbit_oracle.rs::fitted_sisre_matches_liu_2025_on_real_orbits` |
| M124 (row 210) | Liu et al. 22-parameter model and the ATOMIC polynomial | as M120 | as M120 | within 1.5 times the published SISRE per altitude | `tests/leo_navmsg_fit_real_orbit_oracle.rs::liu22_model_matches_the_published_sisre` |
| M125 (row 211) | Single-frequency ionospheric and UTC services | Galileo NeQuick G JRC reference implementation (EUPL-1.2, run as a tool) for Az; the GPS and Galileo ICD worked cases for the three leap-second UTC cases (Library + Reference) | NeQuick G needs a registration at the GSC (see blockers) | Az 1e-6 sfu; UTC 1 ns across the three cases | `tests/leo_navmsg_services_oracle.rs::az_and_utc_match_nequick_g_and_the_icd_cases` |

### B8. Integrity linear algebra (P2) and emitter geolocation (P1) (6 rows)

Modules: `src/lunar_common_mode.rs`, `src/integrity/tpl_scalar.rs`, `src/integrity/gls_commonmode.rs`,
`src/benchmark/stanford.rs`, `src/benchmark/coverage.rs`, `src/geolocation.rs`.

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M041 (row 93) | Common-mode integrity blindness (P2) | numpy `linalg.lstsq`/SVD on the committed 366-epoch real inter-ephemeris sample (P2) | committed fixture | 1e-3 relative (the existing bar) | the existing `tests/lunar_common_mode_integrity_reference.rs` (no new file; the integrator relabels) |
| M080 (row 143) | Composed timing PL, scalar MHSS with H = 1_N (P2) | numpy recomputing the subset least-squares variances, separations and their standard deviations, and the protection level from them at Kshana's stated multipliers (P2); the validated claim is the linear algebra, the multipliers are inputs | committed fixture from the generator | 1e-9 relative | `tests/tpl_scalar_numpy_oracle.rs::scalar_mhss_pl_matches_numpy` |
| M085 (row 149) | GLS common-mode whitening and the Mahalanobis identity (P2) | numpy `linalg.cholesky`/`solve` (LAPACK) against the hand-rolled Cholesky, already in `tests/gls_reference.rs` (P2) | committed fixture | 1e-12 | the existing `tests/gls_reference.rs` (the integrator relabels) |
| M025 (row 72) | Stanford integrity-diagram epoch classifier | P2 does not reach a rule-based classifier. The only independent tool found is gLAB (CC BY-NC-ND 4.0) (Library) | gLAB is not installed (licence decision first) | region counts exact | `tests/tib_glab_oracle.rs::stanford_regions_match_glab` |
| M026 (row 73) | Integrity-coverage scorer | as M025 | as M025 | rates 1e-12 | `tests/tib_glab_oracle.rs::coverage_rates_match_glab` |
| M058 (row 113) | TDOA/FDOA emitter geolocation (P1) | Ho and Chan 1993, IEEE TAES 29(3); Ho and Xu 2004, IEEE TSP 52(9): CRLB values printed for their geometries (P1) | the papers (paywalled) | 1 % where the value is printed in a table; where only a plot exists, the reading uncertainty stated before reading, and only if it is at most 1 % | `tests/geolocation_crlb_published_oracle.rs::tdoa_fdoa_crlb_matches_the_published_values` |

### B9. Interference and spoofing on JammerTest 2024 (3 rows)

Modules: `src/jamming.rs`, `src/spoof.rs`, `src/spoof_detect.rs`, `src/spoof_monitors.rs`, `src/tpl.rs`.
Data: `data/jammertest2024/GNSS_DATASET_JAMMING_SPOOFING.tar.gz` (GPL-3.0-or-later; a committed
slice needs a NOTICE entry) and `data/jammertest2024/Transmissionplan-Jammertest-2024.pdf` (the
published powers, positions and timings; cite, do not vendor). The `realdata::jammertest` adapter
and `examples/jammertest_probe.rs` already read this layout.

| Row | Capability | Oracle | Tolerance | Test |
|---|---|---|---|---|
| M009 (row 50) | GNSS-denied jamming resilience | measured C/N0 of the stationary receivers against the documented jammer EIRP and geometry (Measured) | predicted C/N0 drop within +/-3 dB of measured | `tests/jamming_jammertest_cn0_oracle.rs::predicted_cn0_drop_matches_the_stationary_receivers` |
| M010 (row 51) | Spoofing detection (observable-level monitors) | published spoofing onset times on the RINEX observables; the fused FoM, SQM and AGC need TEXBAT IF data and stay outside the claim (Measured) | detection within 10 s of every published onset, zero alarms in each pre-onset window | `tests/spoof_detection_jammertest_oracle.rs::monitors_detect_the_published_onsets_without_false_alarms` |
| M011 (row 52) | Timing protection level under spoofing | measured served-time error at the detection epoch (Measured) | measured time error at or below the TPL at every detected epoch over all spoof scenarios | `tests/tpl_jammertest_coverage_oracle.rs::measured_time_error_is_inside_the_tpl_at_every_detection` |

### B10. Inertial sensing, INS/GNSS fusion and alternative PNT (4 rows)

Modules: `src/inertial/quantum_imu.rs`, `src/inertial/coast.rs`, `src/fusion/gnss_ins_ekf.rs`,
`src/fusion/closed_loop.rs`, `src/mapmatch.rs`, `src/altpnt/` (magnetic leg only).

| Row | Capability | Oracle | Data | Tolerance | Test |
|---|---|---|---|---|---|
| M005 (row 46) | Quantum inertial sensor performance (promotable only if narrowed) | the Cheinet 2008 transfer function and k_eff T^2 scale factor, already matched in `tests/quantum_inertial_sensor_reference.rs` (Reference); the QPN term becomes a one-sided floor | none | transfer function and scale factor 1e-9 | the existing test; needs the owner to accept the narrower wording before the integrator relabels |
| M008 (row 49) | GNSS/INS sensor fusion, scoped to the loosely coupled filter | NaveGo v1.4 (LGPL-3.0, under Octave) loosely coupled solution on its own example dataset (Library) | `NaveGo/` examples | position RMS within 1.2 times NaveGo's on the same data; innovations consistent | `tests/gnss_ins_navego_dataset_oracle.rs::loosely_coupled_rms_within_1p2x_of_navego` |
| M012 (row 54) | Alternative PNT, magnetic-anomaly map matching only | DAF-MIT MagNav open flight data, tail-stinger magnetometer, GNSS truth (Measured) | `data/magnav/Flt1002_train.h5`; read `data/magnav/LICENSE.md` and `DATA_SHARING_AGREEMENT.md` before committing any slice; the survey maps come from the MagNav.jl artifacts | position error along the flight lines within 1.5 times the CRLB the module predicts | `tests/magnetic_map_matching_magnav_oracle.rs::map_matching_error_within_1p5x_crlb` |
| M061 (row 121) | INS/TRN coasting error growth | NaveGo free-inertial Monte Carlo, 300 seeds, same IMU error profile (Library) | `NaveGo/` | RMS position error per term within 5 % at each coast duration | `tests/ins_coast_navego_montecarlo_oracle.rs::coast_error_terms_match_the_navego_monte_carlo` |

## Coverage check

Wave 1 (11): M004, M018, M020, M035, M045, M047, M057, M059, M090, M106, M109. Wave 1b (1): M005.
Wave 2 (20): M002, M003, M010, M017, M019, M021, M034, M038, M043, M056, M060, M061, M073, M076, M083,
M091, M111, M125, M127, M136. Wave 3 (10): M008, M009, M011, M012, M039, M068, M088, M102, M120, M124.
P1 (4): M058, M093, M101, M129. P2 (5): M025, M026, M041, M080, M085. 51 rows, each in exactly one batch
(B1 5, B2 5, B3 8, B4 8, B5 4, B6 5, B7 3, B8 6, B9 3, B10 4).

## Known blockers (decide or fetch before the batch starts)

- **Owner decisions:** M005 (accept the narrowed wording); M102 (narrow to the presets with public
  measured units, the 5071A and the SA.45s, or leave the row MODELLED: no public OX-208 or 8040C
  record was found); M025 and M026 (gLAB is CC BY-NC-ND 4.0; Ashforde is a commercial entity, so
  using it as an oracle needs a licence reading or permission, otherwise both rows stay MODELLED).
- **Tools not installed by wave 0** (heavy, or gated): GMAT (M060, headless build from SourceForge),
  GNSS-SDR (M068, builds from source; install when no release gate is running), NeQuick G JRC (M125,
  needs a GSC registration), gLAB (M025/M026, licence first).
- **Data not fetched by wave 0:** TU Delft thermosphere densities (M017, host unreachable
  2026-09-30); GRACE-FO GNV1B or Sentinel POD orbits (M120/M124, need an Earthdata or Copernicus
  account); the Liu et al. 2025 PDF, the LANS and LNCSS papers, the BeiDou and Galileo service
  documents and the ITU-T Recommendations (fetch in a browser to `data/papers/`, then rerun
  `setup.sh` so the manifest hashes them; add each to `sources.tsv` if it has a stable URL); a
  multi-GNSS RINEX 3 day for M127; a measured SA.45s ADEV curve for M102; paywalled papers (Betz
  2001, Ho and Chan 1993, Desai and Qualls 2008).
- **P1 rows at risk:** M101 and M129 promote only if a printed value independent of the transcribed
  formula exists; M058 only if the CRLB is printed (or readable within 1 %).

With every blocker above left unresolved, the reachable ceiling is 134 minus the rows it holds
(at most M005, M017, M025, M026, M060, M068, M101, M102, M120, M124, M125, M129), before any oracle
has disagreed. Re-derive the final count from the generated matrix header after integration.
