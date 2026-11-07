# B2 pre-registration (written before any oracle comparison was run)

Written 2026-10-01T03:28Z by the B2 batch agent, branch `feat/val-b2` (worktree
`~/Code/kshana-wt-val-b2`, cut from `feat/validation-uplift` 3ed491b5). Every tolerance is the
plan's (`VALIDATION-0.30-PLAN.md`, B2, fixed 2026-09-30). Where the plan left an operational
detail open it is fixed here, before the first comparison. A later change is a new comparison,
disclosed as such in the row record, and cannot promote the row.

Disclosures about prior knowledge, made now:

- M093: while scoping I evaluated GM/R/c^2 with Kshana's constants (3.13981e-11) and read the
  paper's L_m (3.13881(15)e-11). The plan's 1e-4 relative tolerance predates that.
- M088: the row's own oracle text, written before the plan, already states a correlation of about
  -0.988 against the paper's -0.97 and a sub-millimetre CRLB against the paper's 12 cm. The plan's
  tolerances predate this batch.
- M038, M091: nothing has been computed.

## M038 (row 90) Lunar geodetic VLBI

- Kind: Library. ANISE 0.10.1 (MPL-2.0, https://github.com/nyx-space/anise), the version pinned by
  `xval/anise-lunar-od/Cargo.lock`, run from that crate (never linked into kshana).
- Kernels: `de440s.bsp`, `moon_pa_de440_200625.bpc`, `earth_latest_high_prec.bpc` (the copy fetched
  2026-09-30) from `~/Code/kshana-oracles/data/naif/`; UT1-UTC (Bulletin A column) from
  `data/iers/finals2000A.all`, linearly interpolated, as an input to Kshana only.
- Geometry: epochs 2024-01-01T00:00:00 UTC + k hours, k = 0..24 (25 epochs, regardless of lunar
  elevation). Stations, WGS-84 geodetic (deg, deg, m): Goldstone (40.4256, -116.8893, 1000),
  Canberra (-35.4014, 148.9819, 688), Madrid (40.4314, -4.2481, 830). Baselines: Goldstone-Canberra,
  Goldstone-Madrid, Madrid-Canberra (75 delays). Beacon: selenographic (0, 0, 0) on Kshana's
  1737.4 km sphere, i.e. body vector (1 737 400, 0, 0) m, placed in MOON_PA_DE440 on the oracle side.
- Oracle delay: for each station at the common reception epoch t (TDB), the converged Newtonian
  light time LT_i with the station at t and the beacon at t - LT_i, every position from ANISE
  (Earth and Moon from the SPK, Earth orientation ITRF93 -> J2000, Moon orientation MOON_PA_DE440
  -> J2000), iterated to below 1e-15 s, in the barycentric frame (the Earth's barycentric
  displacement over LT from ANISE's velocity and acceleration at t). tau_A = LT_2 - LT_1. No
  Shapiro term, no BCRS-to-GCRS scale transformation, no media.
- Kshana delay: `lunar_vlbi::geometric_delay_s(station_inertial_position(g1, jd_tt, jd_ut1),
  station_inertial_position(g2, ...), beacon_inertial_position(sel, jd_tt))`.
- Tolerance: |tau_K - tau_A| <= 1 ps at every one of the 75 (epoch, baseline) pairs.
- Partials: central difference of tau_A with respect to the beacon's J2000 position, step 1 km per
  axis, against `delay_partials_beacon` at Kshana's own positions; relative error
  |d_K - d_A| / |d_A| (vector norms) <= 1e-6 at every pair.
- PROMOTE only if both hold everywhere.

## M043 (row 95) Lunar joint multi-technique OD and clock

- Kind: Library. Orekit 12.2 (Apache-2.0) Range and InterSatellitesRange models generating the
  observations, BatchLSEstimator and Kshana both solving; VLBI by the M038 route.
- Tolerance: recovered states 1e-6 m, clocks 1e-15 s, on identical independently generated
  observations.
- Precondition check, done before building anything: Kshana's joint solve must be able to take
  observations it did not generate. If `lunar_combination` exposes no such entry point, the
  comparison cannot be run without adding a public API, which is feature work outside a batch's
  defect-fix licence; the row is then recorded BLOCKED (stays MODELLED) with that reason.

## M088 (row 153) Lunar datum null-space classification

- Kind: Reference. Sosnica et al. 2025, arXiv:2510.15484 (`data/papers/arXiv-2510.15484.pdf`,
  SHA-256 d9bb2be2d367af9972e04ff644294273af1654347dc054c14249828984035856): r(T_X, scale) = -0.97
  (Sect. 6, Fig. 10) and the 1-sigma errors of Table 7 (ILRF vs DE430: sigma(T_X) = 0.0307 m,
  sigma(Sc) = 0.0184e-6).
- Comparison A, the paper's geometry: Kshana `lunar_datum::datum7_point_jacobian_body` evaluated on
  the paper's Table 6 ILRF PA coordinates of the five legacy reflectors (NGLR-1 excluded: DE430,
  the comparison frame of Table 7, has no coordinates for it), equal weights (the paper gives the
  per-reflector formal errors only as "a few mm"), 7-parameter normal matrix; correlation of
  t_x and scale.
- Comparison B, the row's own computation: `lunar_identifiability::llr_identifiability(0.003,
  t0 = JD 2460310.5, 29.5 d, 6 h)` and `lunar_llr_geometry::llr_datum_observability(0.003, same)`;
  correlation of t_x and scale, and the origin (t_x) CRLB.
- Tolerances: every correlation within 0.05 of -0.97; the origin CRLB of B within 20 % of
  0.0307 m. A CRLB for A is not formed: it needs per-reflector sigmas the paper does not print.
- PROMOTE only if every comparison holds.

## M091 (row 156) Lunar LLR datum geometry substrate

- Kind: Measured. ILRS CRD lunar normal points (EUROLAS Data Center, DGFI-TUM).
- Deviation from the plan's data, decided before any comparison: the committed
  `tests/fixtures/lunar_llr/normal_points` slice is 2015-04..06, but
  `lunar_orientation::de440_moon_pa` (the placement the row claims) covers only 2024-01-01 to
  2025-12-31 TDB and clamps to an endpoint outside it, so a 2015 epoch would be placed with a 2024
  orientation. The comparison therefore uses the same three months of 2024 (2024-04, 2024-05,
  2024-06) for the same five targets, fetched from
  `https://edc.dgfi.tum.de/pub/slr/data/npt_crd/<target>/2024/<target>_2024<mm>.npt`. The 2015 slice
  is run as well, as a recorded finding about the clamp, never as a promotion basis.
- Claim narrowed in writing: the reflector PA catalogue, the station catalogue and the DE440 PA
  body-to-inertial placement. The Moon centre is DE440 (spiceypy 8.2.0 `spkpos('MOON', et,
  'J2000', 'NONE', 'EARTH')` on `de440s.bsp`, `naif0012.tls`), not the module's analytic series.
- Points used: every normal point whose station is in `lunar_llr_geometry::stations()` (Grasse
  7845, APOLLO 7045, Wettzell 8834, Matera 7941); others are counted and skipped.
- Model: t0 = CRD transmit epoch (UTC), TOF = CRD two-way time of flight, tb = t0 + TOF/2.
  r_sta(t) = `lunar_vlbi::station_inertial_position(catalogue geodetic, jd_tt, jd_ut1)` with
  UT1-UTC from finals2000A.all (Bulletin A, linear interpolation), no polar motion (the module's
  stated caveat). r_ref(tb) = Moon_DE440(tb) + `lunar_orientation::de440_moon_pa_body_to_inertial`
  (catalogue PA vector). Predicted one-way = (|r_ref(tb) - r_sta(t0)| + |r_sta(t0+TOF) - r_ref(tb)|)/2
  + Earth and Moon Shapiro delay (2GM/c^2 ln((r1+r2+rho)/(r1+r2-rho)), per leg, averaged)
  + Mendes-Pavlis 2004 zenith delay times the FCULa mapping function (IERS Conventions 2010
  Sect. 9.2) from the normal point's own pressure, temperature and humidity record and the CRD
  wavelength, at the up-leg elevation. Measured one-way = c TOF / 2. No tides, no station
  eccentricity, no relativistic time-scale transformation.
- Tolerance: RMS of (measured - predicted) over all used 2024 normal points <= 10 m.
- PROMOTE only if that holds.

## M093 (row 159) Relativistic clock-rate to frame coupling (P1)

- Kind: P1. Ashby and Patla 2024, AJ 167:149, arXiv:2402.11150, Eq. (10) and Table I:
  L_m = 3.13881(15)e-11.
- Kshana value: `lunar_gauge::rate_frame_jacobian(t).d_alpha_d_scale` (epoch-independent).
- P1 precondition: the test must make Kshana compute the value from the publication's inputs.
  `rate_frame_jacobian` takes no GM or radius argument, so Kshana computes it from its own
  constants; this is recorded, and the comparison is run anyway.
- Tolerance: |K - L_m| / L_m <= 1e-4.
- PROMOTE only if the tolerance holds and the P1 precondition is met.
