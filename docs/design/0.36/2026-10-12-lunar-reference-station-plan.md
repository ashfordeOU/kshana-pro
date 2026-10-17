# 0.36: lunar reference-station study (implementation plan)

Status: founder-approved for 0.36 (2026-10-12). This file sits on the design branch `design/0.36-lunar-station`
and is not merged. Implementation branches are cut from `release/0.36.0-rc`.

Coordinator notes against the current repo state:

- Base: `release/0.36.0-rc` (7b5cbf96 or later), not `release/0.35.0-rc`. 0.35.0 is released.
- Ledger baseline: 270 rows (134 VALIDATED / 132 MODELLED / 4 PARTNER) before this work and before W21's
  parked branches. Section 5's expected numbers shift. Report the regenerated numbers.
- Open founder decisions: H2 (renaming the existing wording) is deferred, so do H1 only. C2 (Zenodo timestamp)
  is text only.
- web/ is re-ported once at the end by the site worker. Never hand-edit it.

Names marked (new) are proposals and may be renamed. Names marked (existing) were read in the 0.35 tree; check
each one against the current rc before relying on it.

## 0. Rules this work must keep

1. No affiliation and no endorsement. ESA's NovaMoon is a public mission concept. Kshana models a *lunar
   differential reference station of the class publicly described for NovaMoon*. In code, scenario names,
   kinds, file names, CLI output and README, use the generic term `lunar reference station`. "NovaMoon" may
   appear only in a sourced citation ("the class publicly described in ...") in docs and in the
   pre-registration. It never appears in a kind name, a figure title, a CLI flag or marketing text. No ESA
   logo or colours.
2. No personal names in anything that ships: docs, CHANGELOG, figures, and code comments added by this work.
   Cite sources by title, identifier (arXiv id, DOI, URL) and publisher only.
3. Every new number is MODELLED unless a row is checked against an independent external oracle under a
   pre-registered bar. The headline prediction (Workstream C) is a pre-registered prediction, not a
   validation, and is labelled that way everywhere.
4. No performance claim. Nothing in 0.36 may state or imply that Kshana shows NovaMoon, Moonlight or any real
   system "achieves sub-metre". Allowed: "under the stated public assumptions, the model gives X; MODELLED; not
   a statement about the real system".
5. Pre-register before running. For A9, B and C, PREREGISTRATION.md is committed in its own commit, with its
   hash recorded, before the first comparison or prediction run. Disagreeing results are recorded with their
   numbers and never tuned away. House style: tests/fixtures/maritime_trust/PREREGISTRATION.md and
   tests/lunar_service_volume_published_oracle.rs.
6. Additivity. Existing outputs of every existing kind stay bit-identical. New behaviour lives in new kinds or
   new report fields only. Do not re-bless existing golden pins.
7. Licensing of sources. Record each source's licence in the fixture's NOTICE.md. Commit transcribed numbers
   with page or table references. Do not commit a PDF unless its licence allows redistribution.
8. ROADMAP fit. ROADMAP.md says lunar work is "maintained; new work driven by a concrete user or a published
   result". The trigger here is a published result (the sources below). Say so in the CHANGELOG entry.

## 1. Why (for the CHANGELOG and the docs intro)

ESA has publicly described a lunar south-pole reference station. It monitors the lunar navigation service,
computes local differential corrections, hosts an atomic clock as a lunar time reference, and carries a laser
retroreflector and a VLBI transmitter for the lunar reference frame. Kshana already models each piece
separately: differential PNT, Lunar Coordinate Time, clock classes including a miniature RAFS, lunar laser
ranging, lunar VLBI, and frame realisation. 0.36 composes them into one reproducible, open study of what such a
station could do under public assumptions. It pre-registers that study so it can be scored against published
results and, eventually, flight results.

## 2. Public sources (cite without personal names)

Record each source in tests/fixtures/lunar_station/SOURCES.md (new): title, URL, date retrieved, SHA-256 of
the downloaded file, and licence.

- S1: arXiv:2602.08432 (latest version, v3 at the time of writing), the paper describing the station as a
  local differential, geodetic and timing station. Extract every numeric claim with its page, table or figure:
  accuracy before and after corrections, coverage, clock type and stability, survey and position assumptions,
  latency, message rates, and the assumed constellation.
- S2: IOAG Cislunar PNT Workshop slides, "NOVAMOON: A LANS Monitoring and Local Differential Station"
  (ioag.org). Same extraction. Note where S1 and S2 differ.
- S3: ICG WG-B Lunar PNT, June 2024 (unoosa.org, LunarPNT_Jun24_01_04.pdf). Covers the single-station
  south-pole coverage claim, the miniRAFS "lunar time reference" laboratory, and the nanosecond-level time
  target.
- S4: IOAG Cislunar 2026, "Moonlight Programme Development Status" (ioag.org). Covers the Moonlight
  constellation (satellite count, orbit type, elements if public) and the signal-in-space error (SISE) figures.
- S5: IOAG Cislunar 2026, "Lunar Reference Time: ESA's Views" (ioag.org). Covers lunar time definitions. Check
  them for consistency with src/lunar_time.rs.
- S6: LunaNet LSIS V1.0 (already used by lunar_afs), plus any SISE or URE requirement already in the tree
  (LUNAR_SIGMA_URE_M in src/lunar.rs).

Known discrepancy to record, not resolve: public sources disagree on launch timing (first lander from about
2030 to 2032). The plan does not depend on it, because Workstream C's scoring is triggered by publication of
data, not by a date. If a needed number is not public, use the existing in-tree illustrative value and label
it `ILLUSTRATIVE (no public source)` in the TOML and in the pre-registration.

## 3. Workstreams and order

| # | Workstream | Ledger effect |
|---|---|---|
| H | Hygiene (README contradiction, naming) | none |
| A | New kind `lunar-station-coverage` (Monte Carlo coverage, accuracy and PL over the south-polar region and a day) | +1 MODELLED |
| B | Comparison against the published claims in S1 to S4 | +1, VALIDATED only if it passes its registered bar, else MODELLED with the finding |
| C | Frozen pre-registered prediction plus scoring protocol | no status row; a predictions register |
| D | Station time laboratory: miniRAFS holding LTC | +1 MODELLED |
| E | Station geodesy: retroreflector and VLBI beacon at the site, and their effect on the frame datum | +0 or +1 MODELLED |
| F | Data readiness: lunar station clock file | none |
| R | Docs, schema, counts, CHANGELOG, release | regenerated ledger |

Order:
1. H.
2. A. B, C and D depend on it.
3. B and C. C freezes A's configuration.
4. D and E, in parallel.
5. F.
6. R.

Commit per step, and keep `cargo test --all` green at every commit.

## H. Hygiene (first, one commit)

H1. README.md, the "Lunar PNT suite" row (around line 258), ends with "All **MODELLED** against internal
consistency ...". This contradicts docs/VERIFICATION-MATRIX.md, where "Lunar coordinate time", "Lunar
reference-frame realisation" and "Lunar navigation service volume" are VALIDATED.
- Rewrite the row so each piece carries its ledger status. For example: "Evidence tiers per piece are in the
  verification matrix: LTC rate, frame realisation and service-volume geometry are VALIDATED; VLBI, joint OD +
  clock, differential PNT and the interop export are MODELLED."
- Keep "not affiliated with or endorsed by any agency, no TRL / heritage claim".
- Grep every surface that copies the sentence (README*, docs, packaging) and fix them all. web/ is
  regenerated by the site build, so leave it for the site port.

H2 (deferred by the founder; do not do). This would change the "NovaMoon-class" wording in src/verification.rs,
src/api.rs, scenarios/lunar-differential-pnt.toml and the src/lunar_dpnt.rs module docs.

Acceptance:
- cargo test --all is green.
- scripts/check-doc-coverage.sh is green.
- No ledger status changes.

## A. New kind `lunar-station-coverage`

A1. Question answered: "For a differential reference station at a known south-polar site, what positioning
accuracy and protection level does a user get, as a function of where the user is and when, under stated
assumptions about the constellation, its SISE, the station's survey accuracy, correction latency and link
quantization?"

It is a sweep and Monte Carlo wrapper over existing tested functions in src/lunar_dpnt.rs, with no new physics.

A2. Files:
- src/lunar_station.rs (new): scenario struct, runner, report struct and SVG.
- src/lib.rs: add `pub mod lunar_station;`.
- src/api.rs:
  - ScenarioKind::LunarStationCoverage;
  - the string mapping in both directions (follow the LunarDpnt arms);
  - a ScenarioMeta entry with required and optional fields;
  - dispatch;
  - summary text.
- src/registry.rs: ScenarioId::from_static("lunar-station-coverage").
- src/bundled_scenarios.rs: bundled!("lunar-station-coverage").
- src/advanced_report.rs: map the kind to its ledger row name or names.
- scenarios/lunar-station-coverage.toml (new): fully commented, with every input labelled with its source id
  (S1..S6) or ILLUSTRATIVE.
- docs/field-units-schema.json: units and provenance for every numeric field. tests/field_units_global.rs fails
  otherwise. Do NOT add the kind to UNCOVERED_KINDS.
- tests/lunar_station_coverage_reference.rs (new).
- docs/LUNAR-REFERENCE-STATION.md (new).

A3. Inputs (TOML). Where the meaning is the same, reuse the names and defaults of LunarDpntScenario.

Constellation:
- n_sats, sma_km, eccentricity, inc_deg, argp_deg (existing meanings and defaults), OR ephemeris_path pointing
  to a tabulated Moon-centred ephemeris. The ephemeris loads through the loader lunar_service already uses
  (lunar_ephemeris::LunarEphemeris).
- If S4 gives public Moonlight elements, also ship scenarios/lunar-station-coverage-moonlight-public.toml with
  each element sourced.
- lunar_dpnt's builder caps the constellation at 24 satellites.
- mask_deg (default: lunar_service's value).

Station:
- station_lat_deg (-89.0), station_lon_deg (0.0), station_height_m (0.0).
- survey_sigma_m (existing default 0.30, ILLUSTRATIVE; S1 may give a value).

Error model (common-mode, as in lunar_dpnt):
- orbit_err_m and clock_err_m, OR sise_sigma_m (new). With sise_sigma_m, draw per-satellite orbit-error
  vectors and clock errors so the line-of-sight projected SISE has that 1-sigma. Document the split (default
  50/50 in variance, ILLUSTRATIVE).
- Draw per trial from rand_chacha::ChaCha8Rng, seeded from `seed` and the trial index.
- noise_m, latency_s and quantization_bits keep their existing meanings and defaults.

User grid and time:
- grid_lat_min_deg (-90), grid_lat_max_deg (-80), grid_lat_step_deg (1.0).
- Use the ring rule from tests/lunar_service_volume_published_oracle.rs: round(360 cos lat) longitudes per
  ring, plus the pole. Reuse that code if it is a library function. Otherwise lift it into a shared helper used
  by both callers, without copying it.
- t_start_s (0), t_span_s (86400), t_step_s (600).
- n_trials (default 200).
- Refuse n_points * n_epochs * n_trials > 5e7 with an error that names the input to lower.

Thresholds are inputs stated before the run, never fitted:
- accuracy_threshold_m (1.0) and percentile (95).
- alert_limit_m (ILLUSTRATIVE; state that no public value exists).
- residual_sigma_m and p_hmi (existing meanings, passed to lunar_dgnss_protection_level).
- seed (42).

A4. Algorithm per grid point g and epoch t:
1. Compute satellite MCMF positions at t. Compute the visible sets from the station and from g with mask_deg,
   reusing lunar_service visibility.
2. If g sees fewer than 4 satellites, or the station does not see every satellite g uses, mark the epoch
   unavailable with a reason (user_lt4 / station_missing_sat) and compute no error.
3. For each trial k:
   - Draw the common-mode errors.
   - Compute differential_corrections.
   - Apply the survey bias via survey_biased_corrections, with a per-trial survey draw.
   - Compute corrected_user_range_errors.
   - Add ageing and quantization per satellite (correction_ageing_orbit_range_sigmas,
     correction_ageing_clock_range_sigma_m, correction_quantization_step_m, uniform_quantization_sigma_m):
     ageing as zero-mean Gaussian draws, quantization as uniform draws.
   - Compute the position error via user_position_error_m.
   - Record the corrected 3-D, horizontal and vertical error, and the uncorrected error from
     raw_user_range_errors for the same draw.
4. Compute the protection level via lunar_dgnss_protection_level, once per (g, t).

Aggregate per point:
- the percentile of corrected 3-D, horizontal and vertical error over the available (t, k);
- the same percentile for the uncorrected error;
- availability: the fraction of epochs not marked unavailable;
- coverage: the fraction of epochs whose per-epoch percentile is below accuracy_threshold_m;
- HPL/VPL max and median;
- the fraction of epochs with HPL below alert_limit_m.

Aggregate over the region:
- area-weighted (cos lat) means;
- the worst point and its value;
- along each azimuth, the distance from the station (on a sphere of radius R_MOON_M) at which coverage first
  drops below 95 %, as a list of (azimuth_deg, radius_km).

Determinism: use a fixed iteration order (lat ring, lon, epoch, trial). If rayon is used, reduce per-point
results in index order so the output is bit-identical.

A5. Report LunarStationCoverageReport (new), serde, with a unit entry for every numeric field:
- inputs_echo;
- n_grid_points, n_epochs, n_trials;
- region: availability, coverage_fraction, corrected_p{N}_3d_m / _h_m / _v_m, uncorrected_p{N}_3d_m,
  improvement_factor, hpl_max_m, vpl_max_m, hpl_below_alert_fraction, worst_point {lat, lon, value};
- coverage_radius_km: a list of {azimuth_deg, radius_km};
- points: per point {lat, lon, distance_km, availability, coverage_fraction, corrected_p_3d_m,
  uncorrected_p_3d_m, hpl_median_m, unavailable_reasons};
- budget_at_station_and_at_edge: the CorrectionLinkBudget at the station and at the worst point, for one
  representative epoch;
- the label "MODELLED" and figure_tiers, as other kinds have;
- note: the honesty statement (rules 1, 3 and 4) verbatim;
- units.

SVG lunar_station_coverage_svg (new):
- A south-polar stereographic map (lat -80 to -90) of the corrected percentile error per point, with the
  station marked and the accuracy_threshold_m contour drawn.
- A side panel of region figures.
- Use crate::palette::chart and the footer/advisory pattern of lunar_dpnt_svg. Handle light and dark the same
  way existing SVGs do.

A6. Tests:
1. Reduces to lunar-differential-pnt: one point at the existing scenario's baseline and azimuth, one epoch,
   n_trials=1, same seed and inputs. Corrected and uncorrected errors equal LunarDpntScenario::run() to the
   bit, or to 1e-12 m if the RNG stream necessarily differs (explain why in the test).
2. Zero-baseline floor: the point at the station, with survey 0, latency 0, quantization off and noise 0. The
   corrected error is 0 to 1e-9 m in every trial.
3. Survey floor: the point at the station with survey_sigma_m = s. The corrected 3-D RMS is within a
   statistical band of the survey_position_transfer prediction. State the band before running.
4. Monotonicity:
   - Coverage is non-decreasing as accuracy_threshold_m increases.
   - The corrected percentile is non-decreasing with distance along an azimuth, within Monte Carlo noise
     (large n_trials on a few points).
5. Availability accounting: a constructed three-satellite case gives unavailable/user_lt4 and no error value.
6. Determinism: the same seed gives byte-identical JSON, and a different seed changes the Monte Carlo figures.
7. The work guard refuses an oversized grid with the documented message.
8. Additivity: tests/lunar_differential_pnt_reference.rs and every existing golden stay green with no fixture
   changes.
9. Independent re-computation:
   - examples/gen_lunar_station_ref_inputs.rs (new) exports 20 fixed (point, epoch, trial) cases.
   - scripts/gen_lunar_station_ref.py (new) recomputes them with numpy only: the visible set, the corrected
     range errors, the WLS position error (optionally RTKLIB lsq() via the existing harness of the
     differential test), and the per-point percentile with numpy.percentile (method stated).
   - Register the tolerances before the first run in tests/fixtures/lunar_station/PREREGISTRATION.md
     (suggested: range 1e-9 m, position 1e-6 m, percentile exact on identical samples).
   - This checks the arithmetic, not the model, so the row stays MODELLED.

A7. Ledger row (src/verification.rs):
- requirement: "Lunar reference-station coverage and accuracy map".
- capability: "Monte Carlo sweep of the lunar differential-PNT model over a south-polar user grid and a day of
  epochs: corrected and uncorrected percentile position error, availability, coverage against a stated
  threshold, protection levels, coverage radius per azimuth, and the correction-link budget at the station and
  at the worst point".
- module: "lunar_station (over lunar_dpnt, lunar_service)".
- tests: the A6 tests, by name.
- oracle: "Arithmetic re-computed by an independent numpy script on exported cases
  (scripts/gen_lunar_station_ref.py); reduces exactly to the lunar-differential-pnt kind at one point. The
  error model, the decorrelation model and every threshold are this project's own or ILLUSTRATIVE; nothing
  here is a statement about any real system".
- oracle_kind: InternalConsistency.
- status: Modelled.

## B. Comparison against the published claims

B1. Steps:
1. Read S1 to S4. In tests/fixtures/lunar_station_published/CLAIMS.md (new), list every numeric claim the A
   kind can produce under the same assumptions. Examples: "about 10 m without corrections and sub-metre with
   them over the south pole"; "a single station covers the whole south pole" (using the source's region);
   coverage percentages; accuracy-versus-distance figures. For each claim, record the source id, page,
   table/figure, exact wording, number, and the assumptions the source states.
2. Classify each claim as:
   - comparable;
   - partly comparable (list the missing assumptions and the in-tree values used);
   - not comparable (say why).
   Score only the first two.
3. Write tests/fixtures/lunar_station_published/PREREGISTRATION.md in the maritime-trust layout. It covers
   what is compared against what, the tolerance per claim, the inputs per case, and what the comparison does
   NOT establish. Suggested bars (stricter is allowed):
   - accuracy within a factor of 1.5 at the stated percentile;
   - coverage within 5 percentage points;
   - "covers the whole south pole" scored as coverage_fraction >= 0.95 over the source's region at the
     source's threshold.
   Commit the pre-registration alone and record the hash in its header. Then run.
4. Record the results in the pre-registration's "First comparison run" section, and in
   tests/lunar_station_published_oracle.rs (new) as a pinned finding (style:
   tests/lunar_service_volume_published_oracle.rs, which records a 4-of-12 disagreement).

B2. State this limit before the run: the uncorrected error is driven by the injected SISE, so setting SISE
from the source and then "reproducing" its uncorrected accuracy is close to circular. The informative
comparisons are corrected accuracy and coverage extent.

B3. Ledger row: "Lunar reference-station accuracy and coverage against published claims". The oracle_kind is
the published-reference variant (check the OracleKind enum name).
- VALIDATED only if every scored claim passes its registered bar against the independent source.
- Otherwise MODELLED, with the finding in `oracle`, as the navi.613 row does.
- Expect MODELLED: under-specified published claims are a publishable finding in themselves.

## C. Frozen pre-registered prediction for flight data

C1. Ships in 0.36: docs/predictions/lunar-reference-station.md and
docs/predictions/lunar-reference-station.result.json (new). The prediction document contains:
1. The exact scenario TOML (the Moonlight-public example if S4 allowed it, else the default), engine version
   0.36.0, commit hash, seed, and the SHA-256 of the result JSON. Define in the pre-registration which commit
   is frozen. The recommended choice is the commit that adds the prediction inputs, before the version-bump PR.
2. The predicted figures, each labelled "MODELLED, under the stated public assumptions":
   - corrected and uncorrected p95 3-D error at the station, at 50, 100 and 200 km, and at the region edge;
   - availability;
   - coverage fraction;
   - coverage radius per azimuth;
   - HPL/VPL.
3. The scoring protocol, fixed now:
   - which published quantity each prediction will be compared with when flight or test results are
     published by any party;
   - the bar (suggested: a factor of 2 for accuracies, 10 percentage points for coverage);
   - if the assumptions differ, re-run with the published assumptions and report both, never editing the
     frozen one;
   - a miss is published as prominently as a hit.
4. A statement that this is not a claim about the real system and that no party endorsed it.

Guard test tests/prediction_frozen.rs (new):
- It re-runs the frozen TOML and asserts the stored result hash.
- On failure it says "do not edit the frozen prediction; add a new dated prediction file and keep the old one".
- If cross-platform floating point makes a byte hash fragile, follow how the existing golden pins handle
  platforms (read scripts/check-reproducible.sh first).

C2 (text only, pending the founder): the text for timestamping the prediction hash in the 0.36 Zenodo deposit.

Add a short "Pre-registered predictions" heading to docs/VALIDATION.md. C adds no ledger status row.

## D. Station time laboratory: a miniRAFS holding LTC

D1. Question: "If the station's miniRAFS is steered to LTC and then free-runs, or is steered every N hours, how
does its time error against LTC grow, and how long does it stay within 1, 10 and 100 ns?" S3 states a
nanosecond-level target: quote it, but do not adopt it as a claim.

D2. Prefer extending lunar-time-budget (src/lunar_time_budget_scenario.rs) with an optional block, if it fits.
Otherwise create a new kind, lunar-time-lab.

Inputs:
- clock = "mini-rafs" (existing LunarClock::MiniRafs in src/clock_specs.rs), or a measured clock_library
  device card;
- steer_interval_s (default: free-run) and steer_residual_ns (ILLUSTRATIVE);
- no lunar thermal model unless one is sourced.

Error growth comes from the clock power law (x_clock_s) plus the LTC realisation term. The relativistic rate is
VALIDATED (lunar_time.rs). topographic_spread_ns_per_day applies if the station height is not modelled
exactly; say which.

Outputs, all with units:
- the x(t) curve;
- time-to-threshold for 1, 10 and 100 ns;
- the dominant term at each horizon.

Tests:
- With steering off, it reduces to the existing budget clock term.
- Time-to-threshold by bisection agrees with the closed form for the dominant noise type (reuse the crossover
  approach).
- At every t, the error is ordered optical < PHM < RAFS < miniRAFS.

Ledger: +1 MODELLED row, "Lunar station time laboratory: miniRAFS holdover against LTC". The LTC rate keeps its
own VALIDATED row; say so in `oracle`.

## E. Station geodesy: retroreflector and VLBI beacon at the site

E1. Question: "How much does adding one retroreflector and one VLBI beacon at the station site tighten the
lunar frame datum, compared with today's five retroreflector arrays alone?"

E2. No new physics: use lunar-llr-datum, lunar-vlbi-fim and the multi-technique datum information work.
1. Read how these kinds take reflector and beacon catalogues (realdata::llr_crd). If that input is missing,
   add optional extra_reflectors / extra_beacons inputs. Outputs must be bit-identical when they are absent.
2. Ship scenarios/lunar-station-geodesy-baseline.toml and scenarios/lunar-station-geodesy-with-station.toml
   with the same schedule and seed.
3. Report each Helmert parameter's 1-sigma before and after, and the ratio. Put the table in
   docs/LUNAR-REFERENCE-STATION.md.

Tests:
- An added site never increases any variance (the Fisher difference is PSD).
- An empty extra list gives bit-identical outputs.

Ledger: if only examples and an additive input were added, extend the existing row's tests text and add no
row. If there is a new report block, add +1 MODELLED row.

Caveat to state: south-polar sites have poor Earth visibility for LLR. Use the existing visibility logic, not
continuous ranging.

## F. Data readiness

F1. Check whether realdata::clk::rinex_clock_series accepts a header time system other than GPS/UTC, such as a
lunar time scale. If it does not, make the time system explicit in the parsed output and refuse an unknown one
with a clear error, rather than silently treating it as GPS time. Fixture: a synthetic three-line RINEX clock
file with a station clock record, marked synthetic in its NOTICE.md.

F2. Define no corrections-file format until a public one exists. Say so in the doc.

## R. Docs, schema, counts, CHANGELOG, release

R1. docs/LUNAR-REFERENCE-STATION.md (new) contains:
- the purpose;
- the honesty block (rules 1, 3 and 4) at the top;
- run commands;
- an inputs table with source ids;
- an outputs table with units;
- the figure;
- what each check can and cannot tell;
- the B finding table;
- a link to the C prediction;
- E's table;
- D's time-to-threshold table.

Every caption carries MODELLED and the lunar advisory statement.

R2. Update docs/SCENARIOS.md, docs/ARCHITECTURE.md and docs/VALIDATION.md. Also update:
- docs/MODELLED-RATIONALE.md: one row per new MODELLED ledger row, with the reason and what would promote it;
- docs/SURFACES.md: which surfaces expose the kind (CLI, Python, wasm/Studio, MCP via run_scenario);
- the README scenario table and counts.

R3. Regenerate the ledger with `cargo run --bin gen_validation_artifacts`. Then fix every surface stating the
totals; tests/verification_artifacts_doc_sync.rs and tests/scenario_count_doc_sync.rs name the stale sites. Do
not hand-edit generated files.

R4. The kind count rises by 1, or by 2 if D is a new kind. Bundled scenarios rise by 4 or 5. Update every
guarded site.

R5. Add field-units-schema entries for every new field. tests/field_units_global.rs must pass without adding
to UNCOVERED_KINDS.

R6. CHANGELOG under [Unreleased] (then [0.36.0]): an "Added (lunar reference-station study)" block with one line
per workstream, each labelled MODELLED. B's line states "<n of m claims within the pre-registered bar>
(pre-registered <hash>)". End the block with: "Prompted by published material on a planned ESA lunar reference
station (sources in docs/LUNAR-REFERENCE-STATION.md). Not affiliated with or endorsed by any agency; no
statement about the performance of any real system." Under Changed: "README: the lunar suite row states each
piece's evidence tier (it said all were MODELLED)."

R7. Python and MCP: nothing to add if kinds are exposed generically. Check that list_scenario_kinds shows the
new kind and that the MCP tool-table tests pass. Add one row to docs/AGENTS.md "What to keep with the answer":
"Lunar reference-station results: MODELLED; under stated public assumptions; not a statement about any real
system; not affiliated."

R8. Studio: optional. If added, record it with the existing tooling under web/studio/recorded/, not by hand.
This is done by the site worker.

R9. Release: follow docs/RELEASING.md. The founder merges.

## 4. Definition of done

- H1 is merged. H2 stays deferred.
- lunar-station-coverage runs from the CLI, Python and MCP run_scenario. Example scenarios run in under 60 s on
  a laptop at defaults; lower the defaults if they do not.
- A6 tests 1 to 9 pass, and A's pre-registration was committed before the numpy comparison ran.
- B has CLAIMS.md, PREREGISTRATION.md (hash recorded before the run), a pinned finding test, and a ledger row
  with the honest outcome.
- C has the frozen prediction file, the result JSON, its hash, the scoring protocol and the guard test.
- D has the time-lab output, tests and a ledger row.
- E has the two example scenarios, the comparison table, tests and the ledger text.
- F1 has the time-system handling and its fixture.
- All doc surfaces are updated, the ledger is regenerated, and every count guard is green.
- cargo fmt, cargo clippy --all-targets -- -D warnings, cargo test --all and scripts/gate.sh are all green
  (read gate.sh first).
- A wording grep of the diff finds:
  - "novamoon" only in sourced citations;
  - no personal names added;
  - "sub-metre" only in a labelled MODELLED result or a quoted source claim;
  - "validated" never applied to a MODELLED row;
  - no "certified", "approved", "endorsed" or "partner of ESA".

## 5. Expected ledger arithmetic

Report the regenerated numbers. The baseline is now 270/134 plus W21's branches; this work adds +3 to +4 rows.

## 6. Risks

- Sources too vague: B records "not comparable" with reasons, and the row stays MODELLED.
- Results better than the source's claims: report them as they are, never as "beats the spec". List the
  omitted effects (multipath, terrain masking at the pole, receiver hardware).
- Runtime: lower the defaults and keep the guard.
- Perceived affiliation: follow rule 1 and use generic naming. The founder reviews public wording before
  release.
- Terrain masking at the pole is out of scope for 0.36 (realdata::lola_dem exists, and a horizon mask is a
  0.37 candidate). State it as the first limitation in the doc.
- Unclear source licences: transcribe numbers with references, and commit no PDFs or figures.

## 7. Out of scope for 0.36

- terrain or horizon masking;
- any correction-message format;
- any NovaMoon hardware modelling beyond S1 to S4;
- any change to an existing kind's default outputs;
- outreach, or anything addressed to ESA (the founder's call).
