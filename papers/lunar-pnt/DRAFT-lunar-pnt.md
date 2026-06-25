# Earth-baseline VLBI restores the observability of a lunar surface station in joint orbit-and-clock determination

**Chakshu Baweja** — Ashforde OÜ, Estonia — contact@ashforde.org

> **Draft (PRIVATE, arXiv-first).** Capability code is open in `kshana` (AGPL-3.0-only); this draft and
> its analysis scripts are private until posted. Every number is reproduced by `scripts/run_sweeps.py`
> driving the public `kshana` binary; figures by `scripts/make_figures.py`. Nothing is posted or pushed
> without explicit go.

## Abstract

Lunar positioning, navigation, and timing (PNT) — ESA's Moonlight/LCNS navigation service, a
NovaMoon-class lunar geodetic-timing-differential reference station, the LunaNet/IOAG interoperability
framework, and a defined Coordinated Lunar Time — all rest on determining the orbits and clocks of a
lunar PNT infrastructure and tying it into a terrestrial/inertial frame. A recurring design question is
*which measurement types make which states observable*. We study one instance of it in an open,
reproducible simulator: in a snapshot batch fit of a small lunar PNT network, lunar-local observables
(station-to-satellite ranging, inter-satellite ranging, station clock-sync) determine the network's
internal geometry but leave a lunar surface station's **absolute** 3-D position weakly observable.
Adding a single Earth-baseline geodetic Very Long Baseline Interferometry (VLBI) observable to the lunar
beacon restores it: across a 200-seed truth/noise ensemble the median recovered station 3-D error falls
from ~793 m (range-only) to ~13 m, a median improvement factor of **91×** (inter-quartile range
38–208×). We show with a controlled sweep that the gain is VLBI *information*, not observation count —
degrading the VLBI delay precision at fixed observation count drives the median station error
overall back toward the range-only level — and that the benefit is *selective*: VLBI sharpens
absolute positions and satellite clocks but leaves the station clock, already pinned by clock-sync,
unchanged. A minimum of three Earth stations is required for a full-rank solution. All results are
deterministic, reproduce from a committed scenario, seed list, and engine version, and every figure of
merit is labelled validated or modelled. This is a simulated closed-loop observability study, not a
validation against real VLBI or ranging data; we state that as the first-class limitation. The toolkit
that produces it — lunar time, VLBI, joint OD+clock, frame realisation, navigation service-volume
integrity, differential PNT, and CCSDS/LunaNet interoperability — is released open-core.

## 1. Introduction

Lunar PNT is moving from concept to procurement. ESA's Moonlight initiative will field a lunar
communications and navigation service (LCNS) from a small constellation; a NovaMoon-class lunar
geodetic-timing-differential station concept brings multi-technique geodesy (VLBI, ranging), joint
orbit-and-clock determination, lunar reference-frame realisation, lunar timekeeping, and differential
corrections to the surface; LunaNet and the IOAG are defining cross-agency interoperability; and a
Coordinated Lunar Time has been called for, with its relativistic rate (~57 µs/day faster than
terrestrial time) now a standards question. Every one of these rests on the same engineering core:
**determining the orbits and clocks of a lunar PNT infrastructure and tying it into a terrestrial or
inertial reference frame.**

A first-order design question for that core is *observability*: given a set of measurement types, which
states (positions, clocks, frame parameters) are well determined, and which are not? The answer drives
architecture — whether a lunar nav service needs Earth-baseline geodetic ties, how many, and how precise.
Today this question is answered in private, one-off studies whose assumptions are neither shared nor
reproducible. There is no open, citable tool in which a lunar PNT network's observability can be
regenerated from a committed scenario, with every figure of merit labelled as externally validated or
modelled — the gap the kshana program has documented for terrestrial and deep-space PNT, here for the
lunar case.

We contribute (i) an open lunar PNT toolkit (seven modules in the `kshana` engine) and (ii) one clean,
falsifiable observability result obtained with it: **Earth-baseline VLBI restores the observability of a
lunar surface station's absolute 3-D position in joint orbit-and-clock determination, and does so through
information rather than observation count, selectively, and only above a minimum baseline count.**

## 2. Methodology

### 2.1 The estimation problem

We model a lunar PNT network as: one lunar **surface station** at a fixed selenographic location; a small
**constellation** of lunar satellites on circular orbits; and a set of **Earth ground stations**. The
unknown state stacks the station 3-D position correction, each satellite's 3-D position correction, and
every asset's clock offset (station and satellites) — 16 parameters in the default configuration
(3 sats: 3 + 3×3 + 1 + 3 = 16). The observation model comprises:

- **Earth-baseline geodetic VLBI** delay to the lunar beacon (the surface station), differenced across
  pairs of Earth stations — an *angular, inter-continental* observable;
- **lunar-local ranging**, station↔satellite and inter-satellite (ISL);
- **Earth→satellite ranging** and a **station clock-sync** pseudo-observable, included so that the
  range-only configuration is itself full-rank (otherwise the clock/position degeneracy is trivial and
  the contrast uninteresting).

Truth states are drawn from a seeded generator; clean observables are computed from truth and corrupted
by per-observable Gaussian noise at each technique's stated σ (default VLBI delay σ = 1e-11 s ≈ 3 mm;
lunar range σ and ISL σ at metre level). The estimator is a Gauss–Newton snapshot batch least-squares
fit with a numerically-differentiated Jacobian; parameters are carried in a scaled internal unit to keep
the normal matrix well-conditioned across the ~c² dynamic range between clock and position partials.

This is a **static geometric** observability study: there is no force-model propagation inside the
solver. The conclusion is about measurement geometry and is framed only as such. (Force-model orbit
determination heritage exists separately in the engine and is cited, not conflated.)

### 2.2 The observability claim and its controls

The central claim is that VLBI *restores observability*, not merely *adds data*. We test it three ways:

1. **With vs without VLBI.** Run both configurations on the same seed and truth; report the station 3-D
   error contrast and the per-state breakdown (station/satellite position, station/satellite clock).
2. **Information-not-count control (the key control).** Hold the observation set fixed and sweep the VLBI
   delay σ over four decades. If the improvement were merely from the extra observations, the station
   error would be flat in σ. If it is from VLBI *information*, the error must rise toward the range-only
   level as σ grows (the observations remain but carry vanishing weight).
3. **Robustness.** Repeat with vs without across a 200-seed ensemble (improvement-factor distribution),
   and sweep the number of Earth stations/baselines.

### 2.3 Reproducibility and honesty labelling

Every number is produced by the public `kshana <scenario.toml>` binary; `scripts/run_sweeps.py`
generates the configurations, runs them, and aggregates; `scripts/make_figures.py` renders the figures.
The seed lists are fixed, so the study is deterministic given a pinned engine. Each figure of merit
carries the same label the engine enforces in `src/verification.rs` and `tests/no_overclaims.rs`:
**Validated** requires an external oracle; everything else is **Modelled**. The lunar joint-OD result is
**Modelled** (internal-consistency oracle: recovery of an injected truth plus formal-covariance/NEES
realism). The dilution-of-precision geometry the service-volume module reuses, and the Earth-orientation
transforms the Earth stations use, are externally **Validated** (against `gnss_lib_py` and SOFA/ERFA
respectively) — "validated by reuse", not claimed for this result.

## 3. Related work

Lunar navigation architecture and LCNS signal/service studies; deep-space radiometric tracking and
Δ-DOR; lunar reference-frame and Coordinated-Lunar-Time definition; and open PNT/OD tooling (GMAT,
Orekit, RTKLIB; the kshana engine itself). Our contribution is not a new estimator but an **open,
reproducible observability finding** for the lunar joint-OD problem, with the information-not-count
control that distinguishes a geometric observability claim from a data-volume effect.

## 4. Results

### 4.1 VLBI restores absolute station observability

In the default scenario (3 satellites, 6 Earth stations, seed 42), adding Earth-baseline VLBI reduces the
recovered lunar surface station 3-D position error from **2183 m (range-only) to 3.55 m**, a **615×**
sharpening; satellite position RMS falls from 717 m to 2.20 m. The fit converges in two iterations with a
weighted RMS residual of 0.05 (40 observations, 16 parameters). Figure 1.

That single seed is favourable. Across a **200-seed** truth/noise ensemble the median recovered station
error with VLBI is **13.4 m** (IQR 5.9–23.0 m) versus a range-only median of ~793 m — a **median
improvement factor of 91×** (IQR 38–208×; Figure 4). We report the median as the headline result; the
default-scenario seed sits in the upper tail. In a small fraction of seeds the factor is near unity
(minimum 0.91 over 200 seeds): VLBI is not guaranteed to help on every draw, and we say so.

### 4.2 Information, not observation count

Holding the observation set fixed and degrading the VLBI delay σ from 1e-12 s to 1e-7 s drives the median
station error from **1.6 m to ~391 m** (rising overall, with one minor non-monotonic step near σ=1e-10 s), approaching the range-only median of ~793 m
(Figure 2). The extra observations are present throughout; only their information content changes. This
is the decisive control: the improvement is VLBI **information**, not the fifteen additional
observations. The default σ (~3 mm delay precision, representative of geodetic VLBI) sits near the
informative end of the curve.

### 4.3 The benefit is selective

VLBI sharpens *absolute* states. In the default scenario the satellite clock RMS improves by ~569×
(4.1e-6 s → 7.3e-9 s) alongside the position gains — but the **station clock error is unchanged**
(2.882e-10 s → 2.868e-10 s, ratio 1.005), because the clock-sync pseudo-observable already pins it
(Figure 1, right). VLBI is not a universal improver; it supplies exactly the absolute-geometry
information the lunar-local set lacks, and nothing the lunar-local set already provides.

### 4.4 A minimum baseline count is required

Sweeping the number of Earth stations shows the joint fit is **rank-deficient with one or two stations**
(no finite solution) and becomes well-posed at **three**, after which the median station error decreases
with more baselines and diminishing returns (~30 m at 3 stations to ~10 m at 10; Figure 3). A geodetic
VLBI tie to a lunar PNT network therefore has a minimum-infrastructure threshold, not merely a
nice-to-have margin.

### 4.5 Cross-suite sanity

The same engine produces physically sensible numbers across the lunar chain (Figure 5): Coordinated
Lunar Time at **57.04 µs/day**, inside the published 56–59 µs/day band, decomposed into a +57.50 µs/day
self-potential term and a −0.46 µs/day kinetic term; a lunar-distance VLBI **near-field correction of
58.9 µs** on a 10 727 km baseline, with the far-field limit reproducing the engine's independent Δ-DOR
delay to machine precision (unit-test oracle); and differential PNT decorrelation reducing a 24.5 m
standalone user error to **0.008 m at a 50 km baseline** (3186×), growing with baseline as expected.

## 5. The open lunar PNT toolkit

The result above is one finding from a seven-module open lunar suite in `kshana`, each a runnable scenario
with a verification-matrix row: lunar coordinate time (`lunar_time`), Earth-baseline lunar geodetic VLBI
(`lunar_vlbi`), joint multi-technique OD+clock (`lunar_combination`), reference-frame realisation
(`lunar_frame_realise`), Moonlight/LCNS-class navigation service-volume integrity (`lunar_service`),
differential PNT (`lunar_dpnt`), and LunaNet/IOAG-aligned CCSDS export (`lunar_interop`). The suite reuses
the engine's externally-validated terrestrial core (reference frames vs SOFA/ERFA; DOP vs `gnss_lib_py`;
SGP4 vs the AIAA/Vallado set) rather than re-deriving it.

## 6. Limitations

- **Simulated closed-loop.** Truth is generated from and recovered through the same observation model.
  This is an observability study, **not** a validation against real VLBI or ranging data. It bounds what
  geometry *can* determine under stated noise, not what a real campaign *will* achieve.
- **Snapshot, geometric.** No force-model propagation in the solver; no dynamic arc, no unmodelled
  forces, no real ephemeris or station-coordinate errors, no troposphere/ionosphere/relativistic delay
  modelling beyond what the observable carries; Earth-orientation polar motion set to zero.
- **Modelled.** The joint-OD result is modelled with an internal-consistency oracle; only the reused DOP
  and Earth-orientation kernels are externally validated.
- **Illustrative parameters.** Constellation and station parameters are illustrative and public-source;
  the engine **models the class** of system. It is **not** Moonlight, LCNS, NovaMoon, or LunaNet, and is
  **not affiliated with or endorsed by ESA.** No TRL > 3, no flight heritage, no certification.

## 7. Discussion

For a NovaMoon-class geodetic-timing-differential station or a Moonlight/LCNS architecture, the finding
is a concrete, reproducible statement of why an Earth-baseline geodetic tie matters: lunar-local
measurements alone determine the cluster's internal geometry but leave its absolute placement weakly
observable, and a small number of precise Earth VLBI baselines close that gap — selectively, above a
three-station threshold, through information rather than count. The open toolkit lets a mission-analysis
team reproduce and vary this trade openly, before hardware exists, with the honesty labelling intact.

## 8. Availability and reproducibility

The engine is open source (`kshana`, AGPL-3.0-only, commercial licence available) with each release
archived on Zenodo. The analysis scripts, the committed aggregate data, and the seed lists reproduce
every figure from the public binary. (ArXiv identifiers, the Zenodo DOI for the lunar release, and the
repository URL are inserted at posting.)

## Acknowledgements / funding

Ashforde OÜ. No external funding. The author thanks the open-source PNT and geodesy communities whose
public references and conventions this work calibrates against.
