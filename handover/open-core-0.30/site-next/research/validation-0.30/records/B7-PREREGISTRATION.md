# B7 pre-registration (M120, M124): written before any Kshana run on real orbits

Fixed at 2026-10-01T03:29Z, before the fixture was generated and before the first comparison.
What had been read at this point: the Liu et al. 2025 paper (Remote Sensing 17(16):2894, CC BY,
read in a browser from https://www.mdpi.com/2072-4292/17/16/2894), its Tables 1-10, Eq. 1-9 and
its data-availability statement; the Kshana source of `leo_navmsg::{elements,fit,sisre,truth}`.
No Kshana fit of any real orbit had been run.

## Oracle

- Kind: Measured (real precise orbits as truth) + Reference (published SISRE and RMS components
  for the same satellites, the same days and the same orbit product).
- Truth: TU Graz IfG (ITSG) operational reduced-dynamic orbits, 10 s, celestial frame,
  https://ftp.tugraz.at/pub/ITSG/satelliteOrbitProducts/operational/ (free, no registration,
  "can be used for ... any other application", acknowledgment requested; cite
  Suesser-Rechberger et al. 2022, doi 10.1016/j.asr.2022.03.014). These are the files the
  paper's data-availability statement names. Days, from the paper's Table 3: GRACE-1 2017-06-01,
  GRACEFO-1 2024-01-01, Sentinel-2A 2024-01-01, Sentinel-6A 2024-01-01. HY-2A (CDDIS, needs an
  Earthdata account) is left out.
- Conversion to Earth-fixed: celestial to terrestrial with pyerfa 2.0.1.5 (IAU 2006/2000A,
  `c2t06a`) and IERS finals2000A (the frozen 2026-09-30 copy in ~/Code/kshana-oracles), velocity
  by the time derivative of the rotation. This is data-format conversion, as GROOPS did for the
  paper; it is not the oracle.
- Published values: Tables 4, 5, 6 and 8 of the paper, rows "16" and "22 (a_dot, n_dot, Crs3,
  Crc3, Crs1, Crc1)", 20 min and 30 min columns: RMS A, C, R and SISRE in cm.

## Kshana under test

`leo_navmsg::fit::fit_message` with `ModelKind::Kepler16` (M120) and `ModelKind::Liu22` (M124),
10 s samples, on consecutive non-overlapping arcs of 20 min (72 per day) and 30 min (48 per
day) from 00:00:00 GPS time; residuals message minus truth at every fit sample (both ends
included), projected on the truth's along/cross/radial frame; RMS pooled over all arcs of the
day; SISRE with `leo_navmsg::sisre::sisre_weights(r, 0 deg)` at the day's mean orbit radius,
`sqrt(w_R^2 R^2 + w_AC^2 (A^2 + C^2))` on the pooled RMS components (Kshana's own definition).
Every arc must fit; a fit error counts as a failure.

The only source change allowed for this: an additive public constructor of
`leo_navmsg::truth::TruthOrbit` from tabulated Earth-fixed states (the fitter takes a
`TruthOrbit` and had no way to accept a real orbit). It changes no existing behaviour.

## Tolerances (fixed now)

- G1 components: Kshana RMS A, C and R each within a factor 1.5 of the printed value,
  two-sided (ratio in [1/1.5, 1.5]), for every satellite, both arcs, the row's model.
- G2 SISRE (the plan's tolerance): Kshana SISRE within a factor 1.5 of the printed SISRE,
  two-sided, every satellite, both arcs.
- G3 weights: Kshana `sisre_weights(R_E + h, 0 deg)` gives w_R and sqrt(w_AC^2) within 0.005 of
  the paper's Table 2 at 400, 600, 800, 1000, 1200 and 1400 km.

A row promotes only if G1, G2 and G3 all pass for its model (M120: Kepler16; M124: Liu22).

## Known before the run (from reading the paper alone)

The printed SISRE values are reproduced to the printed digits by sqrt(w_R R^2 + w_AC (A^2 + C^2))
with the Table 2 numbers (w_R, w_AC unsquared; 320 km uses the 400 km row; others linear in
altitude), for example GRACE-A 16 at 20 min 37.66 cm, GRACE-C 22 at 20 min 6.21 cm,
Sentinel-6A 22 at 20 min 0.75 cm, not by the paper's own Eq. 9 (w_R^2, w_AC^2), which
Table 2's values satisfy as w_R^2 + 2 w_AC^2 = 1. Kshana uses Eq. 9. For identical components
G2 therefore carries a convention bias of about 0.78 (Kshana lower). G1 is free of it, which is
why it is a gate.

Claim narrowing (written now): M120 is assessed only for the 16-parameter Keplerian fitter and
its orbit-only SISRE against fit interval on real orbits; the correction polynomials
(`kepler-rac`), the clock fit, the update-period trade and Kshana's integrated truth are outside
what this oracle can validate. M124 is assessed for the Liu22 model only; the ATOMIC ECEF
polynomial is outside it.
