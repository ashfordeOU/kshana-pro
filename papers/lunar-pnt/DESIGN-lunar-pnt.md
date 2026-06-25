# DESIGN — Earth-baseline VLBI and the observability of a lunar surface station in joint orbit-and-clock determination

> Paper #5 of the kshana program. **Capability code → public `kshana` (OPEN, AGPL-3.0-only); paper
> draft + analysis scripts → this folder in `kshana-pro` (PRIVATE).** The lunar suite is already built
> and committed in the open tree (seven modules, seven scenarios); this paper spins a falsifiable
> scientific result out of it. Analysis scripts call the **public** binary and parse its artifact JSON —
> no private engine logic. Branch `lunar-pnt` off `main`, **local only, nothing pushed without explicit
> go.** arXiv-first: nothing public until the founder announces.

## 1. The gap

Lunar PNT is moving from concept to procurement: ESA's Moonlight/LCNS lunar navigation service, a
NovaMoon-class lunar geodetic-timing-differential reference station, the LunaNet/IOAG interoperability
framework, and a defined Coordinated Lunar Time. Every one of these rests on **orbit-and-clock
determination of a lunar PNT infrastructure tied into a terrestrial/inertial frame** — and on knowing
*which measurement types make which states observable*. That design question is today answered in
private, one-off studies. There is **no open, citable tool** in which a lunar PNT network's observability
can be reproduced from a committed scenario, with every figure of merit labelled validated or modelled —
the same gap the kshana program has documented for terrestrial and deep-space PNT.

We address it with: (a) an open lunar PNT toolkit (already built in `kshana`), and (b) one clean,
falsifiable observability result obtained with it.

## 2. The scientific contribution (why this is a paper, not just a tool)

The finding is an **observability** result, not a "more data helps" result. A lunar PNT cluster measured
only by *lunar-local* observables (station↔satellite ranging, inter-satellite ranging, station clock-sync)
determines its **internal** geometry well but leaves its **absolute placement and orientation relative to
Earth/ICRF weakly observable** — there is a near-null direction in the normal equations. A single
**Earth-baseline geodetic VLBI** observable to the lunar beacon is an *angular, inter-continental*
measurement that directly senses that direction, so it restores full 3-D observability of the lunar
surface station's absolute position.

- **H1 — VLBI restores station observability (headline, falsifiable).** In a snapshot batch fit, adding
  Earth-baseline VLBI sharpens the recovered lunar surface station 3-D position by ≥10× versus
  lunar-local ranging alone, **and the gain is driven by VLBI information, not observation count.**
  - *Reproducible artifact:* the open scenario `lunar-joint-od-clock` reports the with-vs-without-VLBI
    station-error contrast directly (default config: ~3.6 m vs ~2183 m, ~615×).
  - *Information-not-count test (the key control):* degrade the VLBI delay σ at fixed observation count;
    the median (over seeds) station error must rise monotonically from sub-metre toward the range-only
    level as σ_vlbi grows. If the extra observations carried no information, error would be flat.
  - *Pre-registered refutation:* REFUTED if VLBI gives < 2× improvement at default precision, OR if the
    σ_vlbi sweep leaves the median station error within 2× of the tight-VLBI floor across four decades of
    σ (i.e. the improvement is observation-count, not information).
- **H2 — The benefit is selective (mechanism, falsifiable).** VLBI sharpens *absolute* states (station
  position, satellite positions, satellite clocks) but **not the station clock**, which the clock-sync
  pseudo-observable already pins. This is the honest mechanism check: VLBI is not a universal improver.
  - *Pre-registered:* station 3-D position error changes ≥10× with/without VLBI **while** the station
    clock error changes < 2×. (Artifact already shows 2.88e-10 s → 2.87e-10 s — essentially unchanged.)
- **H3 — Robust across the ensemble (robustness).** The improvement is a property of the geometry, not a
  lucky draw: the median improvement factor over N independent truth/noise seeds is large with a reported
  spread (IQR), and a sweep over the number of Earth baselines (`n_earth`) shows monotone improvement
  with diminishing returns.

The constructive complement is the **open toolkit itself** as the reproducible vehicle: lunar coordinate
time (reproduces the published ~56–59 µs/day band), VLBI (far-field limit reproduces the engine's
independent Δ-DOR delay to machine precision), joint OD+clock, frame realisation, nav service-volume
integrity, differential PNT, and LunaNet/IOAG export — each labelled validated/modelled.

## 3. Honest scope (non-negotiable; baked into code and paper)

- **Simulated closed-loop recovery.** Truth is generated from, and recovered through, the *same*
  observation model; this is a controlled **observability study, not a validation against real VLBI or
  ranging data.** Stated as the first-class limitation, up front (mirrors the optimism-gap and
  resilience-scoring papers' synthetic→synthetic honesty).
- **Snapshot batch fit, no force model in the solver.** A static geometric Gauss–Newton fit, not a full
  dynamic orbit determination. The observability conclusion is about measurement geometry, and is framed
  only as such. (Force-model OD heritage exists separately in `precise_od`/`lunar_od` and is cited, not
  conflated.)
- **Modelled, calibrated to public oracles.** The result is MODELLED. The only externally *validated*
  kernels are reused, not claimed for this result: dilution-of-precision against `gnss_lib_py`, and Earth
  orientation against SOFA/ERFA. Say "validated by reuse" for those; everything lunar-specific is
  modelled with an internal-consistency oracle (covariance realism / NEES ≈ parameter count, recovery of
  an injected truth).
- **Models the class of system.** Constellation/station parameters are illustrative, public-source, and
  **not affiliated with or endorsed by ESA, Moonlight, LCNS, NovaMoon, or LunaNet.** Kshana models the
  class; it is not the mission.
- **No TRL > 3, no certification, no flight heritage, no agency endorsement.**

## 4. What gets built (this paper)

The capability is already in the open repo. This paper adds only **private analysis + draft** in this
folder:

1. `scripts/run_sweeps.py` — drives the **public** `kshana` binary over: (a) the default headline config;
   (b) a σ_vlbi sweep × N seeds; (c) an n_earth sweep × N seeds; (d) a seed Monte-Carlo at default. Parses
   each `*.result.json`. Deterministic (fixed seed list). Emits `data/*.json` / `data/*.csv`.
2. `scripts/make_figures.py` — renders, from `data/`, the paper figures into `figures/`:
   - Fig 1: headline bar/contrast (station, satellite, clock errors, with vs without VLBI, log scale).
   - Fig 2: σ_vlbi sensitivity — median station error vs σ_vlbi with IQR band, range-only reference line
     (the information-not-count evidence; H1).
   - Fig 3: n_earth sweep — median station error / improvement factor vs number of baselines (H3).
   - Fig 4: seed Monte-Carlo — distribution (histogram/violin) of the improvement factor (H3).
   - Fig 5 (supporting): cross-suite sanity — lunar-time rate in band, VLBI far-field≡Δ-DOR residual,
     differential decorrelation vs baseline.
3. `DRAFT-lunar-pnt.md` — full markdown draft (all numbers from `data/`).
4. `lunar-pnt.tex` + `sections/*.tex` + `arxiv-submission/` + `make-arxiv-tarball.sh` — arXiv build,
   mirroring `papers/open-pnt-simulator` and the public `paper/arxiv` skeleton. Author Chakshu Baweja,
   Ashforde OÜ, `contact@ashforde.org`.
5. `data/` — committed aggregated sweep outputs (the reproducibility object the paper cites). Raw
   per-run JSON regenerates from the public binary + the committed seed list; only aggregates are stored.

Every number in the draft traces to `data/`; `data/` regenerates from the public `kshana` release + the
committed scenario configs and seed list. Reproducibility = the brand.

## 5. Venue & sequencing

arXiv **astro-ph.IM** (Instrumentation and Methods for Astrophysics) primary; cross-list **eess.SP**.
Archival target: a geodesy/navigation venue (Journal of Geodesy / NAVIGATION / ION or IEEE PLANS).
Code is public in `kshana` (reproducibility = brand); the draft is PRIVATE here. **arXiv-first — no public
method exposure before posting; nothing pushed/posted without explicit founder go.**

## 6. Self-review notes

- Falsifiable claims with pre-registered thresholds: ✓ (H1 with the information-not-count control, H2
  selective mechanism, H3 robustness).
- The headline number is reproducible from the *public* artifact (`lunar-joint-od-clock.result.json`); the
  sweeps come from the public binary via committed configs — no private engine logic in the result. ✓
- Honest scope is structural (simulated closed-loop stated up front; modelled label; "validated by reuse"
  reserved for DOP/Earth-orientation; class-of-system framing; no TRL/endorsement). ✓
- Risk: the σ_vlbi curve is noisy at the loosest σ on a single seed (confirmed in probing). Mitigation:
  report the **median over N seeds** with an IQR band — the noise at loose σ is exactly why the
  Monte-Carlo framing is needed and is itself part of the honest story.
- Net-new vs existing: the *finding* (VLBI restores absolute observability, selectively) and its
  controlled evidence are new; the engine, scenarios, and oracles are consumed, not duplicated.
