# Lunar PNT Suite — Design & Roadmap

**Status:** approved (design), build in progress
**Date:** 2026-06-24
**Owner:** Chakshu Baweja (contact@ashforde.org)
**Repo split:** capability code → `kshana` (PUBLIC, open-core); this spec, any paper drafts, and ESA-facing fit assets → `kshana-pro` (PRIVATE)

---

## 1. Goal

Give `kshana` a complete, honestly-verified **lunar PNT** capability: a modelling suite for the
*class* of system represented by ESA's lunar PNT programmes — a lunar geodetic/timing/differential
reference station (NovaMoon-class) plus a lunar navigation constellation (Moonlight / LCNS-class),
tied together by multi-technique geodesy (VLBI + radiometric + inter-satellite links), a lunar
reference frame, a lunar time scale, and interoperability export.

The deliverable is **open-core simulation capability**, not a flight system. Everything new is a
**MODELLED** open simulator calibrated/verified against *public* oracles. Per the established
programme order this foundation is built first; an arXiv preprint and ESA fit assets are spun out of
it afterwards.

## 2. Non-goals / honesty boundary (the moat)

- kshana models the **class of system**. It **never** claims to *be* NovaMoon, Moonlight, or LCNS,
  and never implies ESA affiliation, endorsement, heritage, certification, or TRL > 3.
- Programme-specific example configs (sat count, orbit type, frequencies) are **illustrative**,
  drawn from **cited public sources**, and labelled as such ("illustrative; public-source
  parameters; not affiliated with ESA").
- No raw third-party dataset is redistributed. DE-ephemeris / GRAIL checks use derived scalars or a
  gitignored cache; only scalars/coefficients already shipped in-crate are committed.
- Every capability lands in the machine-checked verification matrix (`src/verification.rs`) with an
  honest tier. `Validated` requires an independent external oracle; otherwise `Modelled`; partner
  gaps are `PartnerOwned`. `tests/no_overclaims.rs` must stay green.

## 3. Existing foundation (what we build on)

kshana (~68k lines) already provides the substrate. Key reused modules:

- **Earth orientation / time (VALIDATED):** `timescales`, `frames`, `precession`, `nutation`,
  `cio` (SOFA/ERFA bit-for-bit), `jd2` two-part JD.
- **Lunar frames (MODELLED):** `lunar_frame` (IAU 2015 WGCCRE ME/PA + 13 libration terms),
  `lunar` (MCI/MCMF, selenographic, lunar ARAIM with LunaNet LNIS params, south-pole protection
  level).
- **Lunar dynamics (MODELLED):** `lunar_od` (GRAIL GRGM660PRIM gravity OD; honest LRO fit ≈ 6.6 m),
  `cr3bp` (Earth–Moon NRHO/halo), `propagator`/`forces`/`integrator`, `gravity_sh`.
- **Radiometric / VLBI-adjacent (mixed):** `deepspace_od` (SRIF, **Δ-DOR plane-of-sky**, light-time,
  Shapiro), `radiometric` (range/Doppler partials, media), `precise_od` (batch Gauss–Newton + STM +
  variational), `batch_ls`.
- **Timing / clocks:** `clock_state` (van-Loan), `holdover`, `allan` (NIST-validated), `tpl`,
  `timetransfer` / `timetransfer_adv` (TWSTFT, common-view, optical ISL, **ensemble timescale**).
- **Geometry / constellation / integrity:** `orbit::dop` (**VALIDATED** vs gnss_lib_py), `walker`,
  `raim` (ARAIM), `sbas` (DO-229E PLs), `filter_health` (NIS/NEES).
- **Interop / plumbing:** `oem`/`omm`/`ccsds_tdm`/`space_packet`, `interchange` (KIF), `scenario`,
  `api` (run_toml dispatcher → CLI/Python/WASM), `report`/`fom`/`chart`.

The genuine gaps are the seven phases below.

## 4. Architecture approach

**Foundation-up, dependency-ordered, each phase ends runnable.** Build the missing primitives bottom
up; every phase finishes with (a) a new module with a focused single responsibility, (b) a runnable
`scenario.rs` kind wired into `api.rs`, (c) a `verification.rs` row with its oracle + tier, and
(d) a passing fmt+clippy+test gate. This yields a demonstrable, honestly-labelled artifact at every
step rather than only at the end.

Rejected alternatives: *vertical scenario slices first* (lower layers too shallow to validate
honestly — breaks the moat); *standards-first* (front-loads spec-reading; folded in as Phase 7).

### Module-boundary discipline

Each new file has one clear responsibility and a small, well-defined public interface. New lunar
modules depend **downward** only (on existing primitives + earlier phases), never sideways on each
other's internals. Prefer several focused files over one large one; if `lunar.rs` grows unwieldy in
Phase 5, split the service-volume logic into a submodule.

### Dependency graph

```
P1 lunar_time ─┬─> P2 lunar_vlbi ─┬─> P3 lunar_combination ─┬─> P4 lunar_frame_realise ─┐
               │                  │                          │                            │
               └──────────────────┴──> (clocks/time) ────────┴──> P5 moonlight service ──┤
                                                                       │                  │
                                                                       └─> P6 lunar_dpnt ─┴─> P7 lunar_interop
```

Build order: **P1 → P2 → P3 → P4 → P5 → P6 → P7.**

## 5. The seven phases

Each phase below is broken into a bite-sized, test-first task plan, then built TDD. "DoD" =
definition of done.

### Phase 1 — Lunar Coordinate Time (LTC/TCL)

- **Goal:** a lunar coordinate time scale and the relativistic Earth↔Moon transformation.
- **New:** `src/lunar_time.rs`.
- **Public interface (sketch):**
  - `struct LunarTimeScale` — holds the secular rate model + optional periodic terms.
  - `fn ltc_minus_tt_rate(state: &EarthMoonState) -> f64` — d(LTC−TT)/dt [s/s], from the
    gravitational-potential difference + kinetic term + Earth/Sun tidal potential, divided by c².
  - `fn tt_to_ltc(jd_tt, ...) -> f64`, `fn ltc_to_tt(...)`, chained to TAI/UTC via `timescales`.
  - `fn lunar_ensemble_timescale(clocks: &[ClockSpec]) -> EnsembleTimescale` — reuse
    `timetransfer_adv` ensemble machinery to form a lunar paper-clock.
- **Composes:** `timescales`, `ephem` (Earth/Moon/Sun positions+velocities), `timetransfer_adv`,
  `body` (GM constants), `jd2`.
- **Oracle / tier:** closed-form relativistic rate (InternalConsistency) **cross-checked against
  published lunar-clock-rate figures** (~56–59 µs/day; report the band and own the
  reference-dependence — geoid vs selenoid, barycentric vs topocentric — as an explicit honesty
  caveat). The velocity term is cross-checked against DE-ephemeris-derived Moon velocity. **MODELLED.**
- **Scenario:** `lunar-time-offset` — emits the secular rate, the accumulated offset over a horizon,
  the contributing terms, and the uncertainty band.
- **DoD:** rate reproduces a published figure within the stated band; conversions round-trip to
  < 1 ns over a day; ensemble matches `timetransfer_adv` reference; verification row + scenario +
  gate green.
- **Pitfalls:** the headline µs/day number is reference-dependent — never present a single scalar
  without the band and the chosen reference. TDB≈TT approximation already in-crate is adequate for
  the rate; do not over-claim sub-ns absolute accuracy.

### Phase 2 — Lunar geodetic VLBI observable

- **Goal:** a VLBI delay observable for an Earth baseline observing a lunar beacon (NovaMoon-class),
  with partials for OD and frame work.
- **New:** `src/lunar_vlbi.rs`.
- **Public interface (sketch):**
  - `struct VlbiBaseline { station_a, station_b }` (ECEF/ITRF station vectors via `frames`).
  - `fn geometric_delay(baseline, beacon_state, epoch) -> Delay` — **near-field** delay = difference
    of geometric ranges station_a→beacon and station_b→beacon over c (Moon is near-field, not a
    plane wave); includes Earth-rotation during light-time, Shapiro (reuse `radiometric`), and
    station/beacon clock offsets.
  - `fn delay_partials(...) -> Partials` — ∂τ/∂(beacon pos), ∂τ/∂(station pos), ∂τ/∂(clock).
- **Composes:** `deepspace_od` (Δ-DOR), `radiometric` (light-time, Shapiro, media), `lunar_frame`,
  `frames`, `lunar_time` (P1, for clock terms).
- **Oracle / tier:** closed-form near-field delay; **in the far-field limit it must match the Δ-DOR
  plane-of-sky observable already in `deepspace_od`** (ReferenceImpl cross-check). **MODELLED.**
- **Scenario:** `lunar-vlbi-delay` — emits delay, delay-rate, and partials for a configured
  baseline + beacon over a pass.
- **DoD:** far-field limit matches Δ-DOR to machine precision; near-field correction has the right
  magnitude/sign vs a hand-derived two-range check; partials verified by finite-difference;
  verification row + scenario + gate green.

### Phase 3 — Joint multi-technique orbit + clock determination

- **Goal:** one estimator that jointly recovers Moonlight constellation orbits, the NovaMoon station
  position, and all clock states, fusing VLBI + 2-way range/Doppler + inter-satellite links, in
  lunar time.
- **New:** `src/lunar_combination.rs`.
- **Public interface (sketch):**
  - `struct LunarNetwork { constellation: LunarConstellation, station: LunarStation, clocks }`.
  - `struct JointSolution { orbits, station_pos, clock_params, covariance }`.
  - `fn estimate(network, observations, config) -> JointSolution` — SRIF/batch over the stacked
    observable set; lunar dynamics from `lunar_od`; STM/variational from `precise_od`.
- **Composes:** `precise_od`, `deepspace_od` (SRIF), `radiometric`, `lunar_vlbi` (P2),
  `clock_state`, `lunar_od`, `filter_health`, `lunar_time` (P1).
- **Oracle / tier:** simulate a truth network, synthesise noisy observables, estimate, and check
  (a) state recovery within the formal covariance and (b) covariance realism via NIS/NEES
  (`filter_health`). Sub-pieces (dynamics) already cross-checked vs DE440/GRAIL. **MODELLED.**
- **Scenario:** `lunar-joint-od-clock` — emits recovered states, formal σ, NIS/NEES, and the
  observable mix contribution.
- **DoD:** truth recovered within 1–2σ; NIS/NEES inside χ² bounds; deterministic (seeded);
  verification row + scenario + gate green.

### Phase 4 — Lunar reference-frame realisation

- **Goal:** *estimate* the lunar reference frame (orientation to ICRF, origin at centre of mass,
  scale) from the network, rather than only applying the IAU rotation.
- **New:** `src/lunar_frame_realise.rs`.
- **Public interface (sketch):**
  - `struct FrameDatum { origin, orientation, scale }`.
  - `fn realise_frame(solution: &JointSolution, ties) -> RealisedFrame` — 7-parameter Helmert tying
    estimated coordinates to a datum; orientation tie to ICRF from VLBI; origin from dynamics.
  - `fn helmert(from: &[Vec3], to: &[Vec3]) -> FrameDatum` (reusable 7-param fit).
- **Composes:** `lunar_frame`, `batch_ls`, P2/P3 outputs.
- **Oracle / tier:** inject a known frame transform into the truth network and recover it (Helmert
  parameters back out to machine precision; algebraic identity for the rotation). **MODELLED.**
- **Scenario:** `lunar-frame-realisation` — emits the realised datum + residuals + recovery error.
- **DoD:** injected transform recovered to < 1e-9 (translation/scale) and < 1 µas-equivalent
  (rotation) on noiseless data; degrades gracefully with noise; verification row + scenario + gate.

### Phase 5 — Moonlight nav-service + integrity

- **Goal:** lunar navigation service-volume analysis — DOP/coverage/availability + lunar ARAIM
  protection levels — from a Moonlight-class constellation.
- **Modify:** `src/lunar.rs` (+ a `lunar` submodule if it grows); reuse `walker`.
- **Public interface (sketch):**
  - `struct LunarConstellation` (illustrative LCNS-class config from public sources).
  - `fn service_volume_dop(constellation, volume_grid, epoch) -> DopMap` — reuse `orbit::dop`.
  - `fn coverage(constellation, region, horizon) -> CoverageStats`.
  - `fn lunar_protection_levels(geometry, lnis_params) -> Pl` — generalise the existing south-pole
    PL to an arbitrary service volume.
- **Composes:** `lunar` (LNIS ARAIM, south-pole PL), `walker`, `orbit::dop` (**VALIDATED kernel**).
- **Oracle / tier:** the DOP geometry kernel is already **VALIDATED** (gnss_lib_py); coverage and
  integrity use **published LCNS/LunaNet parameters** (cited). DOP rows stay Validated; the
  service/integrity composition is **MODELLED.**
- **Scenario:** `moonlight-service-volume` — emits PDOP/coverage/availability maps + PL over the
  service volume.
- **DoD:** DOP matches the validated kernel; coverage/availability self-consistent vs a hand check;
  PL generalisation reduces to the existing south-pole result as a special case; verification rows +
  scenario + gate.

### Phase 6 — Lunar differential PNT

- **Goal:** NovaMoon as a differential reference station — compute and apply differential
  corrections, with user protection levels (lunar SBAS/DGNSS analogue).
- **New:** `src/lunar_dpnt.rs`.
- **Public interface (sketch):**
  - `fn differential_corrections(station_obs, solution) -> Corrections` (orbit/clock/range).
  - `fn apply_corrections(user_obs, corrections) -> CorrectedSolution`.
  - `fn differential_protection_level(geometry, residual_model) -> Pl` — reuse `sbas` PL machinery.
- **Composes:** `sbas`, `pvt`, P3 (`JointSolution`), P5 (geometry).
- **Oracle / tier:** differential **error-cancellation identity** (common-mode orbit/clock errors
  cancel for users near the reference; residual grows with baseline) verified analytically + by
  simulation; reuse `sbas` PL. **MODELLED.**
- **Scenario:** `lunar-differential-pnt` — emits user error with/without corrections vs baseline
  distance, and the protection level.
- **DoD:** common-mode cancellation demonstrated (error drops to the de-correlation floor);
  PL bounds the corrected error at the configured integrity risk; verification row + scenario + gate.

### Phase 7 — Lunar interoperability export

- **Goal:** export the lunar frame, lunar time, and ephemeris in LunaNet/IOAG-aligned forms.
- **New:** `src/lunar_interop.rs`.
- **Public interface (sketch):**
  - `fn export_lunar_oem(solution, frame) -> String` — CCSDS OEM extended for the lunar frame.
  - `fn export_lunar_time(scale: &LunarTimeScale) -> String` — LunaNet/IOAG-aligned time metadata.
  - `fn export_kif_lunar(...)` — KIF envelope with lunar honesty labels.
  - round-trip parsers for the above.
- **Composes:** `oem`/`omm`/`ccsds_tdm`, `interchange` (KIF), P1/P3/P4 outputs.
- **Oracle / tier:** **round-trip schema conformance** (export → parse → equality) and field-name
  conformance vs **published LunaNet/IOAG specification** fields. Schema-conformance rows can be
  **Validated** (deterministic round-trip); semantic alignment is **MODELLED.**
- **Scenario:** `lunar-interop-export` — emits the export artifacts + a conformance report.
- **DoD:** round-trip is byte-stable; field names match the cited spec; verification row + scenario +
  gate.

## 6. Build discipline (every phase)

1. Break the phase into a bite-sized, test-first task list.
2. Implement each task red → green → refactor (TDD).
3. New module(s), small and focused; wire `scenario.rs` kind + `api.rs` dispatch.
4. Add `verification.rs` row(s) with oracle + tier; keep `tests/no_overclaims.rs` green.
5. Update `docs/VALIDATION.md` prose + README capability matrix (PUBLIC) honestly.
6. Gate: `cargo fmt --check`, `cargo clippy -D warnings`, full `cargo test` green.
7. Commit locally, authored `Chakshu Baweja <contact@ashforde.org>`, **no AI/tool attribution, no
   co-author footer, no machine paths**. **Push only when the founder says.**
8. Banned-text scan before any commit (no AI-tool/vendor fingerprints — the four forbidden substrings — and no personal email).

## 7. Public / private split

- **PUBLIC `kshana`:** all capability code, scenarios, tests, verification rows, README/VALIDATION
  updates.
- **PRIVATE `kshana-pro`:** this spec; any arXiv paper draft (kept private until the preprint is
  announced, per the arXiv-first rule); ESA tender/fit assets and bid framing.

## 8. Public sources to cite (for oracles / illustrative configs)

- JPL DE440 / DE421 planetary & lunar ephemerides.
- GRAIL lunar gravity (GRGM660PRIM / GRGM1200).
- IAU WGCCRE 2015 report on rotational elements (lunar ME/PA).
- IERS Conventions 2010 (relativistic time transfer formalism).
- Coordinated Lunar Time / LTC: US OSTP 2024 memorandum; recent relativistic lunar-time literature
  (clock-rate term, ~56–59 µs/day, reference-dependent).
- LunaNet Interoperability Specification (NASA/ESA); IOAG lunar communications architecture.
- ESA Moonlight / LCNS public material; ESA NovaMoon public material (used only as public
  description of the system class).

> All programme parameters used in example configs are public and cited; configs are illustrative
> and not affiliated with or endorsed by ESA.
