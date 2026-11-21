# Cislunar catalogue build-up: tracklet association and initial orbit determination

Module `src/cislunar_catalogue.rs`, command `kshana-pro cislunar-catalogue`
(`src/cli/cislunar_cmd.rs`), tests in `tests/space_ops_evidence.rs` (module
`cislunar_catalogue`). Feature `pro-cislunar-catalogue` (Pro).

Reused unchanged from the open engine:

* the Earth–Moon CR3BP (`kshana::cr3bp::propagate_cr3bp`, `EARTH_MOON_MU`);
* batch least squares (`kshana::batch_ls::gauss_newton`, `fd_jacobian`).

What this adds: association of identity-free angles-only tracklets and an initial orbit per
object. A study of how a cislunar catalogue builds up; planning and evidence only.

`cislunar-catalogue run <request.json> --out <dir> --signer <name> --key-file <file>`. The
request carries tracklets of geocentric right ascension and declination with times in normalised
CR3BP units (1 TU = 375 190 s, 1 LU = 384 400 km), the angular sigma, the range and range-rate
hypothesis bounds and grid, and the link gate.

1. **Attributable** per tracklet: mean epoch, angles and angle rates (straight-line fit).
2. **Association** for each pair of tracklets (Milani et al. 2004 admissible-region approach):
   * the earlier tracklet's attributable with a hypothesised range and range rate is a full state;
     the best hypothesis on the grid is found by propagating to the later tracklet;
   * the state is then refined by Gauss–Newton on both tracklets together;
   * the pair is linked when the normalised RMS angle residual is within the gate.
   Linked tracklets form objects (connected components); single tracklets are reported
   unassociated.
3. **Initial orbit** per object: a six-state batch fit to all its observations, with the formal
   covariance `(HᵀWH)⁻¹`; position and velocity 1-sigma.

On synthetic data the report also gives association purity and completeness, and per object
the position error against truth and whether it lies within 3 sigma.

Limits: CR3BP only (no solar gravity, radiation pressure or lunar eccentricity), geocentric
observer without light time or aberration, linearised covariance, links joined transitively.
**MODELLED**. `cislunar-catalogue synth <dir>` writes three CR3BP objects (near L4, near L5,
lunar vicinity) seen in nine shuffled tracklets with 1 arcsec noise.
