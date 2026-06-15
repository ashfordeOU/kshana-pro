<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
T12 (technical-proposal contribution) + T15 (SoW compliance matrix, template) +
T10 seed (WP outline). For a prime's technical volume. Contact: contact@ashforde.org
-->

# Technical contribution — Kshana / Ashforde WP for AO/1-13503

**Role:** the navigation-performance & trade-study work package inside a prime-led
consortium. This document gives the prime ready-to-adapt method text, a SoW compliance
matrix (template, pending the real FPD-10029), and a WP outline for the WBS.

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

## 4. WP outline (T10 seed — to be finalised with the prime in PBF-7 format)

| PBF-7 field | Content |
|-------------|---------|
| **WP title** | Navigation-performance analysis & quantum-vs-classical PNT trade studies |
| **Objectives** | Quantify candidate PNT-architecture performance & resilience; close the quantum-vs-classical trade with reproducible evidence; supply performance requirements to the payload/SE WPs |
| **Inputs** | Mission/constellation concept (SE WP); quantum-device parameters (HW partner); environment & threat definitions (SE/customer); SoW Phase 0/A task list (FPD-10029) |
| **Tasks** | (a) Define comparison frame & FoMs; (b) model classical & quantum candidates on the neutral path; (c) resilience analysis (jam/spoof/alt-PNT); (d) mission analysis (coverage/DOP/availability/integrity); (e) timing-architecture analysis; (f) reproducible evidence pack + trade report |
| **Outputs** | Trade-study report; per-scenario FoM tables; reproducible scenario bundle (hashes + seeds + pinned version); performance-requirements feed to payload/SE WPs |
| **Exclusions** | Quantum/RF payload **hardware design & maturation**; spacecraft-bus engineering; flight product assurance / MAIT; carrier-phase PPP/RTK receiver work |

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
