# B3 pre-registration (written before any oracle comparison was run)

Written 2026-10-01T03:24Z by the B3 batch agent, branch `feat/val-b3` (off `feat/validation-uplift`
3ed491b5). Nothing below has been compared against an oracle yet. Every tolerance is the plan's
(`VALIDATION-0.30-PLAN.md`, B3), and where the plan left an operational detail open, the choice is
fixed here, before the first comparison. Any later change is a new comparison and is disclosed as
such in the row record; it cannot promote the row.

## M035 (row 87) Offline default Earth-orientation input is a real IERS product

- Oracle kind: Library. astropy 8.0.1 `astropy.utils.iers.IERS_A.read` (BSD-3-Clause),
  https://docs.astropy.org/en/stable/utils/iers.html.
- Input: the data lines of `tools/finals2000A_2026.txt` (the `#` comment header is not part of the
  IERS format and is stripped before astropy reads the file; nothing else is altered).
- Comparison, all exact (integer counts and MJD values):
  1. total rows astropy reads = the row count the bare `realtime-frame-eop` default run reports;
  2. rows astropy reads with an unmasked Bulletin B UT1 (`UT1_UTC_B`) = the reported final rows;
  3. rows with a Bulletin A UT1 and a masked Bulletin B UT1 = the reported prediction-only rows
     (`predicted_rows.n`), and their first and last MJD = the reported `first_mjd`, `last_mjd`;
  4. the whole-file MJD span astropy reads = the span the row claims (MJD 61173 to 61204);
  5. every row the run calls prediction-only carries astropy's `P` flag for UT1 (`UT1Flag_A`) and
     for polar motion (`PolPMFlag_A`), and every row it calls final carries `I`; the I/P census
     astropy reads is recorded.
- PASS only if all five hold exactly.

## M034 (row 86) Joint UT1 and polar-motion error

- Oracle kind: Reference. IERS Bulletin A's printed accuracy statement,
  S_xy = 0.00068 (MJD-MJD0)^0.80 arcsec and S_t = 0.00025 (MJD-MJD0)^0.75 s, read from each issue
  (MJD0 parsed from the issue's own formula line).
- Data: every Bulletin A issue in `data/iers/bulletinA/` (Vol. XXXVI-XXXIX, 2023-2026) whose
  40-day target has a Bulletin B final in the frozen `data/iers/finals2000A.all` (2026-09-30 copy).
- Kshana pipeline under test: each issue is turned into an as-issued finals2000A body (its own
  prediction table as prediction-only rows, its combined-EOP table rows up to MJD0 as the rapid
  rows, the cutoff row MJD0); `frame_eop::predicted_vs_final_ut1` measures the UT1 residual and
  `frame_eop::archived_vintage_comparison` (field `pm_archived`) the pole residual, each against the
  frozen file's Bulletin B finals. The test aggregates per horizon over all issues (RMS).
- Horizons in the claim: 10, 20, 30 and 40 days (the four Bulletin A tabulates). Other horizons
  are reported but do not count.
- Comparison: UT1 RMS / S_t(h), and pole RMS / (sqrt(2) S_xy(h)), because Kshana's pole residual is
  the magnitude of the 2-D pole error and S_xy is stated per coordinate (equal, independent x and y
  errors assumed; stated here before the run).
- Tolerance: every one of the 8 ratios in [1/1.5, 1.5]. At least two years of issues.
- Scope (narrowed in writing before the run): this tests the predicted-versus-final measurement the
  row emits (the vintage-differenced residual and its per-horizon RMS) against IERS's own accuracy
  statement. The identical-epoch-set and quadrature identity remain covered by the existing
  internal tests and are not what this oracle validates.

## M073 (row 135) Operational-style Earth-orientation prediction error

- Oracle kind: Reference. The archived Bulletin A predictions themselves scored against the same
  later finals (IERS), and the 2nd EOP PCC result (Sliwinska-Bronowicz et al., Earth Planets Space
  2022, doi:10.1186/s40623-022-01753-9, CC BY 4.0): day-10 UT1-UTC MAE 0.36 to 3.13 ms over 56
  methods.
- Same as-issued bodies as M034. The operational predictor is fitted by Kshana on the as-issued
  rapid rows only (the union of the issue's own and the two previous issues' combined-EOP tables,
  each date taking its latest as-issued value at or before MJD0; nothing after MJD0), with the
  crate default configuration (15-day window). `archived_vintage_comparison` gives the operational
  (`ut1_operational`, `pm_operational`) and the Bulletin A (`ut1_archived`, `pm_archived`) residual
  per issue per horizon.
- MAE = mean absolute residual over all issues at each horizon h = 1..10 days.
- Tolerance: for UT1 and for the pole magnitude, Kshana MAE <= 1.5 x Bulletin A MAE at every
  h in 1..10 (20 conditions), AND Kshana day-10 UT1-UTC MAE inside [0.36, 3.13] ms. PASS only if
  all 21 hold.

## M083 (row 147) Heterogeneous UTC(k) traceability-bias integrity overbound

- Oracle kind: Measured. BIPM per-laboratory [UTC-UTC(k)] files `data/bipm/utclab/utc-*` with
  their published uncertainties (columns uA, uB, u in ns), BIPM Time Department,
  https://webtai.bipm.org/ftp/pub/tai/other-products/utcr/ (as in the manifest).
- Every laboratory file, every row that carries the published u column.
- Overbound per row: `integrity_bias_overbound` with expanded_uncertainty = published u
  (combined standard uncertainty), coverage_factor = 1, ageing = 0, target tail 1e-2.
- Exceedance: |UTC-UTC(k)| > b. Statistic pooled over all rows; per-laboratory rates recorded.
- Tolerance: pooled exceedance fraction <= 1e-2. The Clopper-Pearson 95 % interval is recorded.
  (Secondary, informational only: the same with uB in place of u.)

## M002 (row 42) Onboard clock state estimation

- Oracle kind: Measured. IGS final 30 s satellite clocks, days 2025-229 to 2025-242
  (`data/realdata/igs/IGS0OPSFIN_*_CLK.CLK.gz`), GPS Block IIF satellites whose IGS metadata
  (`igs_satellite_metadata.snx`) gives a rubidium frequency standard over the whole span.
  allantools 2024.6 computes the ADEV.
- Preparation (generator): phase decimated to 300 s; first half = days 229-235, held-out second
  half = days 236-242. On the first half only: remove a quadratic, compute allantools overlapping
  ADEV at tau = 300 * 2^k s up to 38 400 s, and fit sigma_y^2(tau) = 3R/tau^2 + q_wf/tau + q_rw tau/3
  by non-negative least squares on relative residuals. q_drift = 0. No outlier editing; missing
  epochs are coasted.
- Filter: `ClockState3::new(q_wf, q_rw, 0)`, diffuse initial covariance, measurement variance R,
  run through the first half as warm-up, scored on the second half only.
- Criteria, per satellite: (a) one-step innovation nu^2/S <= 3.841 on >= 90 % of second-half epochs;
  (b) 1 h (12-step) prediction error e^2/(P_00 + R) <= 3.841 on >= 90 % of second-half epochs;
  (c) consistency: the NEES of the plan needs the true state, which real data does not give, so it
  is operationalised as the sum of three consecutive non-overlapping normalised one-step
  innovations (chi-square with 3 degrees of freedom when consistent); its mean must lie in
  [2.4, 3.6].
- PASS only if (a), (b) and (c) hold for every selected satellite.

## Rows not run

- M003 (row 43): see its record; no Kshana function produces a per-technique budget total.
- M101 (row 170): the ITU-T Recommendations are not in `data/papers/` (browser download pending).
- M102 (row 171): owner decision pending (narrowing to the 5071A and SA.45s presets) and the SA.45s
  measured curve is not fetched.

## Amendment to M002, 2026-10-01T03:37Z, before any M002 comparison was run

The selection rule above cannot be applied: `igs_satellite_metadata.snx` has no
FREQUENCY_STANDARD block, and the IGS clock files do not say which standard is active
(the existing `tests/slot_timing_igs_holdout.rs` records the same). Replaced, before any M002
data was processed, by: every GPS Block IIF satellite with a PRN in the window, as listed by
`scripts/fetch_igs_clocks.sh` (G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32), read from the
extract that script produces (`data/realdata/igs/gps_iif_clocks.txt`); the clock type is not
claimed. PASS still requires every satellite. Further details fixed now: decimation keeps the
epochs at whole multiples of 300 s; for the ADEV estimate only, missing first-half epochs are
filled by linear interpolation (the filter itself coasts across them); the filter starts at
x = [first measurement, 0, 0] with P0 = diag((1e-6 s)^2, (1e-9)^2, (1e-16 /s)^2); the 1 h
prediction is one predict(3600 s) from the updated state at epoch k, scored against the
measurement at k + 3600 s when present.
