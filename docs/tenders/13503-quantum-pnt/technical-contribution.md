<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
T12 (technical-proposal contribution) + T15 (SoW compliance matrix, template) +
T10 (PBF-7 WP descriptor, §4 — paste-into-WBS form). For a prime's technical volume.
Contact: contact@ashforde.org
-->

# Technical contribution — Kshana / Ashforde WP for AO/1-13503

**Role:** the navigation-performance & trade-study work package inside a prime-led
consortium. This document gives the prime ready-to-adapt method text, a SoW compliance
matrix (template, pending the real FPD-10029), and a **paste-into-WBS PBF-7 WP descriptor**
(§4) for the ground-side quantum-vs-classical PNT trade & GNSS-denied resilience engine —
the WP whose Deliverable D1.1 is detailed in
[`D1.1-gnss-denied-resilience-benefits.md`](D1.1-gnss-denied-resilience-benefits.md).

> All quantitative claims trace to Kshana v0.17.0 `docs/CAPABILITY.md` / `docs/VALIDATION.md`
> and are labelled **validated** (vs an external oracle) or **modelled** (physics model,
> self-/cross-checked). Keep that labelling in the proposal — it is the credibility asset.

---

## 1. Quantum-vs-classical PNT trade method (the feasibility core)

The demonstrator's raison d'être is a defensible quantum-vs-classical comparison. Our method:

1. **Fix the comparison frame.** One scenario (orbit/constellation, environment, mission
   profile), one seed, one pinned engine version. Both the quantum and classical candidate
   route through the **same neutral code path** — no per-candidate tuning.
2. **Score on common figures of merit.** Position/velocity error, timing stability
   (ADEV/holdover), availability/integrity (HPL/VPL), and resilience (P_md under spoof, %
   availability under jamming). Each FoM is reported with its uncertainty and its
   **validated-or-modelled** label.
3. **Sweep, don't assert.** Monte-Carlo and parameter sweeps map *where* quantum wins
   (e.g. CAI-disciplined dead-reckoning bounding a 120 s GNSS outage to a few hundred metres
   vs kilometres for a navigation-grade free INS) and where it does not pay off.
4. **Trace every number.** Scenario-hashed, reproducible outputs + an audit-grade evidence
   record, so an ESA reviewer can re-run the trade.

**Coverage & honesty.** Quantum sensors are modelled from published Allan/noise coefficients
(CSAC, optical clocks; optical/RF time transfer), **except** the cold-atom interferometer
accelerometer, which is first-principles (Mach–Zehnder phase, shot-noise floor, Coriolis &
AC-Stark systematics) and validated to within ~2 orders of the Freier et al. 2016 device.
Device-level hardware feasibility is the quantum-hardware partner's WP; we provide the
**system-performance** half of the trade and ingest the partner's device parameters.

## 2. GNSS-denied / resilient PNT

- **Jamming:** geometry-driven J/S link budget + the anti-jam effective-C/N₀ equation
  (despreading gain + spectral-separation factor), per-satellite loss-of-lock, and an
  `availability_under_jamming` FoM over a Walker constellation. *(modelled; link-budget level)*
- **Spoofing:** an integrated multi-layer detector — clock-aided χ²/Neyman–Pearson, RAIM
  consistency, AGC, and signal-quality (SQM) — fused per epoch, with `1 − P_md` as the
  security FoM. *Characterised against the published TEXBAT scenario parameters* (Humphreys
  et al., ION GNSS 2012), reproducing the documented detectability pattern including the
  negative results.
- **Alternative PNT:** cold-atom gravity-map matching (≈145 m over a 60-min outage),
  terrain-referenced navigation (TERCOM/SITAN, batch + sequential), and IGRF-14
  magnetic-anomaly matching, fused in a combined alt-PNT navigator with a CRLB argument that
  the joint posterior beats any single field. *(modelled; synthetic + low-degree maps —
  high-resolution real maps are a stated follow-on)*

## 3. Mission analysis & timing

- **Constellation/orbit:** Walker design + gradient-free optimiser; coverage/revisit/DOP/
  availability; SGP4/SDP4 **validated to 4.12 mm vs all 666 AIAA 2006-6753 vectors**; a
  Cowell propagator (zonal d/o 6, third-body, SRP, drag, relativity) pinned to analytic
  Kepler truth to sub-metre over 24 h; OD (Gauss–Newton batch + sequential).
- **Timing:** ADEV/MDEV/TDEV/HDEV with noise-type-specific CIs (NIST SP 1065, **validated to
  1e-4 vs Stable32 on NBS14**); van-Loan Kalman clock + filter-health monitor; holdover FoM;
  two-way time transfer with reciprocal-delay cancellation.
- **Integrity:** snapshot/MHSS/ARAIM P_HMI budget (single-SV + constellation-wide faults),
  exercised on real IGS SP3 geometry and real GPS+Galileo TLEs.
- **Frames & reproducibility:** IAU 2006/2000A + CIO GCRS↔ITRS **bit-exact vs SOFA/ERFA**,
  Vallado-pinned; deterministic scenario-hashed outputs.

## 4. WP descriptor — PBF-7 form (paste-into-WBS)

> The broad navigation-performance WP of earlier drafts is sharpened here into the **one
> WP Ashforde owns end-to-end through the open neutral engine**. It is written in PBF-7
> fields so the prime can paste it straight into the WBS; founder-gated fields (WP number,
> effort/cost, dependencies on the teamed prime) are marked. All figures trace to Kshana
> v0.17.0 modules named inline, under the validated-or-modelled discipline.

| PBF-7 field | Content |
|-------------|---------|
| **WP number** | *[founder/prime-gated — assigned in the WBS]* |
| **WP title** | **Ground-side quantum-vs-classical PNT trade & GNSS-denied resilience engine** |
| **Responsible** | Ashforde OÜ (consortium SME; navigation-performance & trade-study WP contributor — **not** prime, **not** a mandated Key Person) |
| **Objectives** | Through the open neutral engine, **quantify** the PNT benefit a quantum candidate (cold-atom CAI + optical clock) buys over a classical baseline (nav-grade INS + CSAC), and the GNSS-denied resilience of each, with reproducible, honestly-labelled evidence. **Quantifies, never validates the device** — device feasibility, maturation and the quantum Key Person are the hardware partner's WP. |
| **Inputs** | (1) **A partner's quantum-clock / cold-atom performance spec** (σ_y(1 s), ADEV profile, atom number N, contrast C, pulse separation T, cycle time, residual bias, scale-factor) **OR a measured ADEV trace** (τ, σ_y(τ)) — the latter is the defensibility hinge (NNLS ingestion); (2) classical baseline spec (CSAC/USO ADEV; nav-grade INS bias/scale-factor/VRW); (3) mission/environment & threshold set (from SE WP); (4) SoW Phase 0/A task list (FPD-10029). |
| **Tasks** | **T-a Trade table** — `QuantumNavBudget` (CAI-physics-derived `q_va` timing-holdover surrogate + position-drift) + `holdover` clock holdover, composed into `quantum_vs_classical_trade`: timing-holdover, position/inertial-holdover and an **availability/benefit table** (candidate ÷ baseline), every row tagged measured-vs-assumed floor. *(`src/inertial/quantum_imu.rs`, `src/holdover.rs`, `src/quantum_trade.rs`)* <br> **T-b GNSS-denied resilience-vs-time envelope** — `resilience_envelope` composes clock holdover + quantum-inertial coast + an alt-PNT (gravity/terrain/magnetic map-matching) bound into one error-vs-time curve, with the **finite** coast-time-to-threshold FoM (horizon reported as a lower bound, never ∞). *(`src/quantum_trade.rs`; alt-PNT bound from `src/gravimeter.rs`/`src/altpnt`)* <br> **T-c Measured-ADEV ingestion** — `qparams_from_adev_curve` NNLS-fits a partner's measured overlapping-ADEV curve to the holdover noise model, so the floor is driven by **data not assumption**; round-trip-checked against `q_from_allan`, dropping the floor caveat. *(`src/quantum_trade.rs`)* <br> **T-d Evidence pack** — deterministic, scenario-hashed run bundle (trade table + resilience envelope + FoM tables) with the MODELLED provenance banner and the validated-or-modelled labels carried on every artifact face. |
| **Outputs** | (1) **Ranked quantum-vs-classical trade table** (timing/inertial holdover + benefit ratios, floor-provenance per row); (2) **GNSS-denied resilience figure-of-merit** (the finite coast-time-to-threshold from the composed envelope, with whether the alt-PNT bound is binding); (3) **reproducible evidence pack** (scenario hashes + seeds + pinned engine version) an ESA reviewer can re-run. These are Deliverable **D1.1** (`D1.1-gnss-denied-resilience-benefits.md`) and feed the prime's VCD navigation-performance rows (`SE-verification-plan.md` §3.3). |
| **Verification** | Per-output oracle stated and honestly labelled (`src/verification.rs`, machine-checked): trade/holdover/ingestion = MODELLED (ReferenceImpl, floor caveat structural); resilience envelope = MODELLED (InternalConsistency, finite-horizon honesty pinned by regression); the optional hybrid 17-state UKF scenario is scored by a **NEES + innovation-whiteness self-consistency oracle vs χ² bands** (a self-consistency statement, NOT a real-world accuracy guarantee). |
| **Effort / cost** | *[founder/prime-gated — ECOS/PBF via the prime]* |
| **Dependencies** | Quantum-device spec or measured ADEV (HW partner); threshold/mission set (SE WP). *[teaming-gated]* |
| **Exclusions (by design)** | Quantum/RF payload **hardware design, maturation & TRL advancement**; the quantum Key Person; spacecraft-bus engineering; flight product assurance / MAIT; carrier-phase PPP/RTK receiver work. **No TRL > 3 / flight heritage / certification / ESA endorsement.** |

## 5. SoW compliance matrix — **TEMPLATE** (T15: reconcile against the real FPD-10029)

> ⚠ The real requirement-level Phase 0/A task list lives in the SoW **FPD-10029** (referenced
> by the draft contract) — **not yet obtained**. The rows below are keyed to the audit's
> §C Phase 0/A needs as a placeholder. Replace the left column with the actual SoW
> requirement IDs once the SoW is in hand, and re-check each compliance verdict.

| SoW need (placeholder — from audit §C) | Kshana compliance | Status label | Evidence |
|----------------------------------------|-------------------|--------------|----------|
| Quantum-vs-classical PNT performance trade | Full method + neutral-path scoring; quantum from coefficients (CAI first-principles) | **Compliant (performance layer)** | §1; `docs/QUANTUM.md` |
| Resilient / GNSS-denied PNT use-cases | Jamming, spoof (TEXBAT-param), alt-PNT (gravity/terrain/magnetic), ARAIM | **Compliant** | §2; `docs/CAPABILITY.md` |
| Mission analysis (orbit/constellation/coverage/DOP) | Walker + optimiser; SGP4 4.12 mm; Cowell; OD | **Compliant** | §3; `tests/` AIAA validation |
| Timing architecture (clocks/holdover/transfer) | Allan suite (Stable32-validated); Kalman clock; TWSTFT | **Compliant** | §3; `docs/VALIDATION.md` |
| Integrity (ARAIM/SBAS) | Snapshot/MHSS/ARAIM P_HMI; Stanford diagram | **Compliant** | §3; `docs/ARAIM_REFERENCE.md` |
| Reference frames / traceability | IAU 2006/2000A + CIO, SOFA bit-exact | **Compliant** | §3; `tests/frame_reference_vectors.rs` |
| RF / signal & link analysis for nav payload | Friis + jamming link budget + measurement-domain GNSS sim | **Partial** — link/feasibility level; no signal/payload design | audit §C (MODERATE); needs RF partner |
| Cislunar / deep-space PNT (if in concept) | CR3BP, LunaNet ARAIM, Mars radiometric (DE440 cross-check) | **Partial** — sim-validated only | §3; `docs/CAPABILITY.md` deep-space row |
| Quantum payload hardware design & maturation | — (out of scope; partner WP) | **Not compliant (by design)** | capability brief "honest boundaries" |
| Spacecraft-bus engineering (AOCS/thermal/…) | — (out of scope; partner WP) | **Not compliant (by design)** | capability brief "honest boundaries" |

## 6. Risks & mitigations (Ashforde-side)

| Risk | Mitigation |
|------|-----------|
| Eligibility — EE-seated OÜ vs Participating-State requirement | DE registered office/branch (T2) + ESA Q&A (T3) before commit |
| No quantum-nav Key Person on Ashforde's side | Key Person comes from the hardware partner (T8); founder owns the performance WP |
| Deep-space / RF claims could be over-read | Keep validated-or-modelled labels; explicit "honest boundaries" section |
| SoW detail unknown until FPD-10029 obtained | T15 — compliance matrix is a template; reconcile before submission |

---
*Background IPR & licensing: see [`BIPR-declaration.md`](BIPR-declaration.md). Contact:
**contact@ashforde.org**.*
