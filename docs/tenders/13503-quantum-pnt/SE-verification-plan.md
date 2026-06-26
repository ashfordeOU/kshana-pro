<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
System-engineering verification plan + RTM skeleton for Kshana's navigation-performance
contribution to AO/1-13503. For a prime's SE lead. Contact: contact@ashforde.org
-->

# System Engineering Verification Plan & RTM Skeleton — Kshana navigation-performance contribution

**Tender:** ESA AO/1-13503/26/NL/RR — *Quantum-Enabled PNT Demonstrator* (FutureNAV C3), Phases 0/A/B1, study toward IOD 2030–31
**Author / owner:** Ashforde OÜ (Chakshu Baweja) — navigation-performance & quantum-vs-classical trade-study SME
**Subject system element:** Kshana PNT-performance-and-resilience simulator, v0.20.0 (open-core engine, dual-licensed: GNU AGPL-3.0-only + commercial licence)
**Reference standard:** ECSS-E-ST-10-02 (Verification) — *applied in style, not as a contractual deliverable Ashforde has produced before; see §6 residuals*
**Contact:** contact@ashforde.org

> **STATUS: DRAFT / TEMPLATE.** This plan is structured against ECSS-E-ST-10-02 but is
> populated with *placeholder* requirement identifiers. The authoritative requirement set
> is the System Requirements Document derived from the Statement of Work **FPD-10029**,
> which Ashforde has **not yet obtained**. Every requirement ID below is a placeholder
> (prefixed `NAV-PERF-*`) and the Requirements Traceability Matrix (RTM) is a **skeleton**
> to be re-keyed and re-verified once the real SoW and consortium teaming are in hand.
> Items that are founder-gated or consortium-gated are marked accordingly.

---

## 1. Purpose & scope

### 1.1 Purpose

This document defines how Ashforde OÜ will **verify the navigation-performance contribution**
it supplies to the AO/1-13503 demonstrator study, and how that verification evidence is to be
folded into the project-level verification programme owned by the consortium prime.

It is **not** a system-level Verification Plan for the demonstrator. It is the *element-level*
verification approach for one work package — the navigation-performance and
quantum-vs-classical trade-study WP — expressed in ECSS-E-ST-10-02 terms so it can be
ingested cleanly by the prime's System Engineering (SE) function.

### 1.2 Scope and honest division of responsibility

Ashforde's role in this consortium is the **navigation-performance simulation and trade-study
SME**. Ashforde is **not** the prime, is **not** the SE authority for the system, and does
**not** supply flight hardware.

The following are stated plainly so the boundary is unambiguous:

- **The named System Engineering lead is the prime's**, not Ashforde's. The overall system
  SRD, the Design, Development, Verification & Validation Plan (DDVP), the system Verification
  Control Document (VCD), and the master RTM are **prime-owned** deliverables. Ashforde
  *contributes verification inputs into* those documents; it does not author or sign them.
- **Ashforde supplies the navigation-performance verification inputs**: the requirement→model→
  test→oracle→status evidence for the PNT-performance analyses Kshana performs, plus the
  trade-study methodology and reproducible result bundles. These are inputs the prime's SE
  lead assembles into the system verification record.
- **Partner-owned engineering is explicitly out of Ashforde's verification scope** (§2.4):
  spacecraft-bus engineering (AOCS/thermal/structures/propulsion/power), navigation **RF
  payload & antenna hardware** design, **quantum payload hardware** design & maturation, and
  flight-hardware **product assurance** (radiation/EEE/MAIT). These are consortium partners'
  disciplines. Kshana is a navigation-**performance** simulator, not a hardware or bus house.

### 1.3 Applicable & reference documents

| Ref | Document | Note |
|-----|----------|------|
| AD-1 | ECSS-E-ST-10-02 | Verification — applied as the structuring standard |
| AD-2 | SoW **FPD-10029** | **NOT YET OBTAINED** — authoritative requirement source; this plan is a template until it is in hand |
| RD-1 | Kshana `docs/CAPABILITY.md` (v0.17.0) | Honest capability/scope map (validated vs modelled vs out-of-scope) |
| RD-2 | Kshana `docs/VALIDATION.md` | External-oracle validation evidence |
| RD-3 | Kshana `src/verification.rs` | Machine-checked requirement→module→test→oracle→status matrix (§3) |
| RD-4 | Kshana `docs/REPRODUCIBILITY.md` | Determinism, pinned toolchain, SBOM, build provenance |
| RD-5 | `technical-contribution.md` (this folder) | WP method text + SoW compliance matrix (template) |

---

## 2. Verification approach

Verification follows the four ECSS-E-ST-10-02 methods. For each, this section states **which
Kshana evidence supports it** and at what honesty level (validated vs modelled). Ashforde's
contribution is concentrated in *Analysis* and *Test*, with supporting *Review-of-Design* and
*Inspection*; flight-representative test/qualification belongs to the hardware partners.

### 2.1 Review-of-Design (RoD)

Verification by examination of design documentation and rationale.

- Kshana's design rationale is documented and reviewable: the capability map (RD-1) states,
  per capability, what exists, what is missing, and the roadmap; the validation ledger (RD-2)
  records each external oracle; an overclaim-closure ledger records every historical overclaim,
  how it was resolved, and the CI guard that keeps it resolved.
- The **methodology** for the quantum-vs-classical trade (single comparison frame, neutral
  code path, common figures of merit, sweep-don't-assert) is reviewable design intent that the
  prime's SE lead and ESA can assess before any number is produced (RD-5 §1).
- *Honesty note:* RoD here covers **simulation/method design**. Flight-design reviews
  (PDR/CDR-level hardware design) are partner-owned and out of Ashforde's scope.

### 2.2 Analysis (A)

Verification by mathematical model, simulation, or analytical demonstration — **Ashforde's
primary method**, since Kshana *is* a performance-analysis tool.

- **Validated analyses** (checked against an independent external oracle): orbit propagation
  (SGP4/SDP4 vs the 666 AIAA 2006-6753 vectors, 4.12 mm), reference frames/timescales
  (IAU SOFA/ERFA + Vallado, mm-level), frequency stability (Allan suite vs NIST SP 1065 /
  Stable32 on NBS14, 1e-4), integrity (RAIM/ARAIM/SBAS vs DO-229E/DO-316 with real IGS SP3),
  and ranging-code cross-correlation (vs published GPS C/A Gold −23.9 dB).
- **Modelled analyses** (first-principles or published formulae, internally / cross-checked
  only): GNSS-denied clock holdover, the quantum-inertial dead-reckoning budget (incl. the
  Coriolis & AC-Stark systematics), the **quantum-vs-classical trade engine** (the trade table,
  measured-ADEV ingestion via NNLS, and the resilience-vs-time envelope — §3.3, the D1.1
  evidence pipeline), the **hybrid 17-state tightly-coupled UKF scenario** (scored by a
  NEES/innovation-whiteness self-consistency oracle), nav-signal modulation & code-tracking,
  jamming link budget, spoof detection (TEXBAT *parameter* characterisation), CR3BP cislunar
  analysis, and alternative-PNT (gravity/terrain/magnetic).
- *Honesty note:* "validated" is reserved strictly for external-oracle checks; everything else
  is "modelled" and labelled as such in the RTM (§3). No analysis claims flight heritage.

### 2.3 Inspection (I)

Verification by examination of the item itself (here, the software artifact and its evidence).

- Software-assurance inspection is concrete and reproducible: ~1500+ unit/integration tests;
  a CycloneDX 1.5 SBOM (`scripts/gen-sbom.sh`); golden, determinism, and cross-platform CI
  gates; clippy + fmt gates; a pinned `rust-toolchain` with a committed `Cargo.lock`; and SLSA
  build provenance (RD-4).
- The **verification matrix itself is inspectable and machine-checked** (§3): its honesty
  invariants are enforced by unit tests, so the validated/modelled boundary cannot silently
  drift.
- *Honesty note:* Inspection here is **software** inspection. Physical inspection of flight
  hardware (workmanship, EEE parts) is a partner's PA discipline, out of scope.

### 2.4 Test (T)

Verification by exercising the item and measuring response.

- At Ashforde's level, "Test" means the automated test suite that exercises each model against
  its oracle, plus reproducible scenario runs that produce per-scenario figure-of-merit tables
  with hashes and seeds (so a reviewer can re-run any result).
- **Out of Ashforde's scope — partner-owned test/qualification:** environmental test,
  radiation test, RF/antenna measurement, quantum-payload characterisation, and assembly /
  integration / test (MAIT). Kshana produces *performance predictions and the test cases that
  bound them*; it does not run flight hardware, and no flight TRL is claimed (Kshana sits at
  TRL ≈ 3–4 as a simulation tool).

### 2.5 Method-to-evidence summary

| ECSS method | Ashforde evidence | Honesty level |
|-------------|-------------------|---------------|
| Review-of-Design | Capability map, validation ledger, overclaim ledger, trade methodology | Method/design only |
| Analysis | Kshana validated + modelled analyses (the bulk of the contribution) | Mixed: validated where an external oracle exists, else modelled |
| Inspection | ~1500+ tests, SBOM, golden/determinism/cross-platform gates, build provenance | Software assurance only |
| Test | Automated model-vs-oracle suite + reproducible scenario FoM bundles | Simulation test only; flight test is partner-owned |

---

## 3. The executable verification backbone

The spine of this plan is **executable**, not prose. The open engine contains
`src/verification.rs`, which builds a machine-checked matrix mapping each PNT-performance
capability to:

> **requirement → implementing module → test evidence → validation oracle → OracleKind → status**

and renders it to Markdown or CSV for direct paste into a project Verification Control Document.

### 3.1 Why this is the backbone (and what "machine-checked" honestly means)

The risk in any verification matrix is **dressing a self-consistency check up as an external
validation**. `src/verification.rs` defends against that with *unit-tested honesty invariants*:

- A row may be **`VALIDATED` only if it carries an `ExternalDataset` oracle** — an independent
  authoritative dataset, published verification vectors, or a published numeric value. A row
  whose only evidence is an internal algebraic identity or a sibling re-implementation
  **cannot** be labelled Validated (test `validated_rows_require_an_external_oracle`).
- A **`MODELLED`** row must name a module + tests and a real (non-None) oracle kind — it is a
  model, honestly one of: a loose external bracket, a same-codebase `ReferenceImpl`
  cross-check, or an `InternalConsistency` closed-form identity.
- A **`PARTNER`** row claims **nothing** — no module, no test, no oracle — by design
  (test `partner_rows_claim_nothing`); only partner rows may carry the `None` oracle kind.

The invariants are *classification* checks (a Validated row must be tagged external; partner
rows claim nothing; counts are consistent). They do **not** prove every named test/oracle
string resolves to live code — those are curated, code-reviewed references, exactly like the
citations in the validation ledger. This scoping is stated in the module's own doc comment so
the artifact does not oversell itself.

### 3.2 Representative RTM (drawn from `verification_matrix()`)

The rows below are a representative extract of the live matrix — the externally-validated core,
several modelled rows, and the four partner-owned gaps. (The matrix renders machine-generated
Markdown/CSV; this is a curated subset for the SE lead, not a hand-maintained copy.)

| Req-ID (placeholder) | Requirement | Module | Test evidence | Oracle | OracleKind | Status |
|---|---|---|---|---|---|---|
| NAV-PERF-010 | Frequency stability characterisation | `allan` | `tests/allan_reference.rs` (NBS14 vs Stable32 to 1e-4) | NIST SP 1065 / Stable32 on NBS14 | ExternalDataset | **VALIDATED** |
| NAV-PERF-020 | Integrity (RAIM / ARAIM / SBAS) | `raim`, `sbas`, `lunar` | `tests/igs_real_data.rs`, `tests/araim_dual_real_data.rs` | DO-229E/DO-316 K-factors; real IGS SP3 | ExternalDataset | **VALIDATED** |
| NAV-PERF-030 | Orbit propagation & determination | `sgp4`, `propagator`, `precise_od` | 666 AIAA 2006-6753 vectors (4.12 mm) | AIAA 2006-6753 SGP4 vectors | ExternalDataset | **VALIDATED** |
| NAV-PERF-040 | Reference frames & timescales | `frames`, `nutation`, `cio`, `timescales` | `tests/frame_reference_vectors.rs` (Vallado end-to-end) | IAU SOFA/ERFA; Vallado (0.1–4 mm) | ExternalDataset | **VALIDATED** |
| NAV-PERF-050 | Ranging-code design trade | `navsignal` (CodeFamily) | `navsignal::code_tests` (Gold ≈ −23.9 dB) | Published GPS C/A Gold (−23.9 dB) | ExternalDataset | **VALIDATED** |
| NAV-PERF-110 | GNSS-denied clock holdover | `holdover` | `holdover::tests` (vs Kalman covariance recursion; round-trip) | Same-codebase covariance recursion | ReferenceImpl | **MODELLED** |
| NAV-PERF-120 | Quantum inertial dead-reckoning resilience | `inertial::quantum_imu` (QuantumNavBudget) | `budget_tests` (bias vs integrator; VRW vs analytic integral) | AccelModel integrator + analytic closed forms | ReferenceImpl | **MODELLED** |
| NAV-PERF-121 | Quantum inertial systematics (Coriolis / AC-Stark light-shift) | `inertial::quantum_imu` (`coriolis_phase`, `ac_stark_phase`) | `quantum_imu::tests` (Φ_cor = 2·k·v·Ω·T² → 2Ω×v bias; AC-Stark symmetric cancellation) | Classical Coriolis 2Ω×v + π/2–π–π/2 symmetry (internal algebraic identity) | InternalConsistency | **MODELLED** |
| NAV-PERF-122 | Quantum-vs-classical PNT trade (timing/inertial holdover + benefit) | `quantum_trade` (`quantum_vs_classical_trade`) | `quantum_trade::tests` (benefit > 1; floor-caveat present when assumed, absent when measured) | Composed `holdover` + `QuantumNavBudget` on a neutral path; floor caveat carried on the artifact | ReferenceImpl | **MODELLED** |
| NAV-PERF-123 | Measured-ADEV ingestion (NNLS clock floor) | `quantum_trade` (`qparams_from_adev_curve`) | `quantum_trade::tests` (ADEV→q round-trip vs `q_from_allan`; NNLS non-negativity on pure white-FM) | `q_from_allan` round-trip (same-codebase converter, would fail on wrong basis power) | ReferenceImpl | **MODELLED** |
| NAV-PERF-124 | GNSS-denied resilience-vs-time envelope (clock + CAI + alt-PNT bound) | `quantum_trade` (`resilience_envelope`) | `quantum_trade::tests` (monotone; finite coast time; alt-PNT bound binding; horizon = lower bound, never ∞) | Closed-form monotone envelope + finite-horizon search (internal consistency) | InternalConsistency | **MODELLED** |
| NAV-PERF-125 | Hybrid 17-state tightly-coupled GNSS/INS UKF scenario | `fusion::hybrid_ukf` (`run_hybrid_ukf`), `fusion::tightly_coupled17` | `hybrid_ukf::tests` (matched filter self-consistent; mistuned filter flagged; CAI-floor-bounded coast; bit-reproducible) | **NEES + innovation-whiteness (NIS) vs 95% χ² bands** — pooled Monte-Carlo self-consistency oracle (Bar-Shalom §5.4); a self-consistency statement, NOT a real-world accuracy guarantee | InternalConsistency | **MODELLED** |
| NAV-PERF-130 | Spoofing detection | `spoof`, `spoof_detect`, `spoof_monitors` | `tests/spoof_texbat_validation.rs` (parameter characterisation) | TEXBAT scenario parameters (characterisation, not pinned vectors) | ExternalDataset | **MODELLED** |
| NAV-PERF-140 | GNSS-denied jamming resilience | `jamming` | `jamming::tests` (PSD-derived Q cross-check) | Anti-jam C/N₀ eq.; navsignal PSD-derived Q | InternalConsistency | **MODELLED** |
| NAV-PERF-145 | Alternative / complementary PNT bound (the resilience-envelope alt-PNT cap) | `gravimeter`, `altpnt::terrain`, `altpnt::sequential`, `mapmatch`, `igrf` | `tests/*` (gravity-map matcher ≈145 m / 60-min outage vs ≈70 km free-INS; map-matching CRLB; IGRF-14 field) | IGRF-14 coefficients; first-principles matched-filter CRLB (synthetic / low-degree maps) | InternalConsistency | **MODELLED** |
| NAV-PERF-150 | Reproducibility & software assurance | `report`, `scenario`; CI gates | `tests/golden.rs`, `tests/determinism.rs`, `tests/cross_platform_golden.rs` | Pinned golden FoM (1e-6) + SHA-256 determinism | InternalConsistency | **MODELLED** |
| NAV-PERF-900 | Spacecraft-bus engineering (AOCS/thermal/structures/propulsion/power) | — | — | — | None | **PARTNER** |
| NAV-PERF-910 | Navigation RF payload & antenna hardware design | — | — | — | None | **PARTNER** |
| NAV-PERF-920 | Quantum payload hardware design & maturation | — | — | — | None | **PARTNER** |
| NAV-PERF-930 | Flight-hardware product assurance (radiation/EEE/MAIT) | — | — | — | None | **PARTNER** |

> The `MODELLED` tag on NAV-PERF-130 is deliberate: the spoof detector is *characterised*
> against published TEXBAT scenario **parameters**, not validated against the raw IQ vectors
> (which need an SDR front-end Kshana does not have) — so it is not labelled VALIDATED even
> though its oracle is external. The honesty invariant permits this (external oracle present)
> but the status reflects the looseness honestly.

### 3.3 The now-runnable quantum-PNT verification pipeline (D1.1 evidence inputs)

Rows NAV-PERF-110/120/121/122/123/124/125/145 are no longer prose claims — they are a
**runnable pipeline** whose outputs are the verification-input artifacts that fold into the
prime's VCD. Each is a verification input with a stated oracle and an honest validated-or-
modelled label; together they are the evidence behind Deliverable **D1.1** (the GNSS-denied
resilience-benefits note, this folder).

| Verification input (artifact) | Runnable module / function | Oracle (the verification method) | Folds into VCD as | Label |
|-------------------------------|----------------------------|----------------------------------|-------------------|-------|
| **Resilience-vs-time envelope** — composed clock holdover + quantum-CAI coast + alt-PNT bound, with the finite coast-time-to-threshold FoM | `quantum_trade::resilience_envelope` | Monotone closed-form envelope + finite-horizon search; coast time always finite, horizon reported as a **lower bound** (never ∞) | NAV-PERF-124 resilience metric | **MODELLED** (InternalConsistency) |
| **Quantum-vs-classical trade table** — timing/inertial holdover + candidate ÷ baseline benefit, on one neutral code path | `quantum_trade::quantum_vs_classical_trade` | Composed `holdover` + `QuantumNavBudget`; the **floor-assumption caveat is carried on the artifact** when any clock floor is assumed, dropped when measured | NAV-PERF-122 trade metric | **MODELLED** (ReferenceImpl) |
| **Measured-ADEV ingestion** (the defensibility hinge) — NNLS fit of a partner clock's measured overlapping-ADEV curve to the holdover noise model, driving the floor from **data, not assumption** | `quantum_trade::qparams_from_adev_curve` (the twin calibration / measured-data framework) | Round-trip vs the independent `clock_state::q_from_allan` converter; NNLS non-negativity; would fail on a wrong basis power | NAV-PERF-123 timing-floor provenance | **MODELLED** (ReferenceImpl) |
| **Hybrid 17-state UKF scenario** — classical-IMU short-term + quantum-CAI/clock long-term, run over a GNSS outage | `fusion::hybrid_ukf::run_hybrid_ukf` (`kind = "hybrid-ukf"`), `fusion::tightly_coupled17` | **NEES + innovation-whiteness (NIS) consistency oracle vs 95% χ² bands** (Bar-Shalom §5.4): matched filter lands inside the bands, a *mistuned* filter is **flagged** — the discriminating self-consistency gate **is the verification method** | NAV-PERF-125 estimator-consistency metric | **MODELLED** (InternalConsistency; self-consistency, NOT accuracy) |
| **Quantum inertial systematics** — Coriolis/rotation + AC-Stark light-shift bias modes | `inertial::quantum_imu::{coriolis_phase, ac_stark_phase}` | Classical Coriolis `2Ω×v` recovery + π/2–π–π/2 symmetric cancellation (algebraic identities) | NAV-PERF-121 systematic-error allocation | **MODELLED** (InternalConsistency) |
| **Alt-PNT bound** — the gravity/terrain/magnetic map-matching fix that caps the unaided coast (the envelope's `alt_pnt_bound_m`) | `gravimeter`, `altpnt::terrain`, `altpnt::sequential` | Gravity-map matcher ≈ 145 m over a 60-min outage (vs ≈ 70 km free-INS); IGRF-14 field; matched-filter CRLB | NAV-PERF-145 alt-PNT availability bound | **MODELLED** (InternalConsistency; synthetic / low-degree maps) |

**The quantum-systematics mode is folded where relevant** (NAV-PERF-121): the Coriolis and
AC-Stark biases are the two leading deterministic CAI systematics a moving-vehicle quantum
inertial WP must allocate in its error budget; the engine carries them as algebraic-identity-
checked models, so the SE lead can trace them into the system error-budget rather than treating
the CAI floor as bias-free.

Each row is **honestly labelled MODELLED with its oracle** — none borrows the engine's
external-oracle validation islands (SGP4 / Allan-vs-Stable32 / IGS). The two estimator/
envelope rows are explicit that **self-consistency is not accuracy**: the UKF oracle certifies
the filter's reported covariance matches the spread of its own errors *under the modelled
noise*, and says nothing about field or flight accuracy.

### 3.4 How the prime's SE lead consumes this

The CSV/Markdown render of `verification_matrix()` is intended to drop into the prime's VCD as
the navigation-performance section. The prime re-keys the placeholder `NAV-PERF-*` IDs to the
real SRD requirement IDs (§4), assigns the project verification method/event per requirement,
and the matrix's own CI invariants keep Ashforde's status labels honest over the study lifetime.
The runnable pipeline of §3.3 is the verification evidence the prime cites in the VCD's
navigation-performance rows; because the artifacts are deterministic and scenario-hashed, the
prime (and ESA) can re-run any cited number.

---

## 4. SRD (System Requirement Document) skeleton

To be **populated once the real SoW (FPD-10029) is obtained.** The heading structure below
follows a conventional ECSS requirement-document layout; it is provided so the requirement
work can begin the moment the SoW is in hand. Ashforde authors only the navigation-performance
requirement subsections (marked *[Ashforde input]*); the rest is prime-owned.

```
1.  Introduction
    1.1  Purpose & scope of the SRD
    1.2  Document tree & relationship to SoW FPD-10029
    1.3  Definitions, acronyms, abbreviations
    1.4  Applicable & reference documents

2.  System overview & mission context
    2.1  Demonstrator mission objectives (quantum-enabled PNT)
    2.2  Operational concept & PNT use-cases (nominal, GNSS-denied, contested)
    2.3  External interfaces & environment

3.  Functional requirements
    3.1  PNT functional requirements
    3.2  Quantum-vs-classical comparison requirements        [Ashforde input]
    3.3  Resilience / GNSS-denied requirements               [Ashforde input]

4.  Performance requirements
    4.1  Positioning & velocity accuracy                     [Ashforde input]
    4.2  Timing stability & holdover                         [Ashforde input]
    4.3  Availability & integrity (HPL/VPL, P_HMI)           [Ashforde input]
    4.4  Resilience metrics (P_md spoof, % availability jam) [Ashforde input]

5.  Interface requirements
    5.1  Navigation payload interfaces                       (prime / payload partner)
    5.2  Quantum sensor / clock parameter interfaces         [Ashforde ingests]
    5.3  Ground / processing interfaces

6.  Environmental & operational requirements                 (prime / bus partner)

7.  Product-assurance & dependability requirements
    7.1  Software product assurance                          [Ashforde input]
    7.2  Flight-hardware PA (radiation/EEE/MAIT)             (PA partner)

8.  Verification requirements
    8.1  Verification approach & method allocation           (prime SE; §2 here feeds it)
    8.2  Navigation-performance verification matrix          [Ashforde input — §3 backbone]

Appendix A  Requirements Traceability Matrix (RTM)          (prime master; Ashforde subset)
Appendix B  Acronyms
```

---

## 5. Verification control matrix — template

The control matrix the prime's VCD will carry per requirement. Until FPD-10029 is obtained,
the Req-IDs are placeholders and the method/status are illustrative. One row per requirement;
"Method" is the ECSS method (RoD / A / I / T); "Status" is OPEN / CLOSED / PARTNER.

| Req-ID | Requirement (short) | Method (RoD/A/I/T) | Verification evidence | Status |
|--------|---------------------|--------------------|-----------------------|--------|
| NAV-PERF-010 | Frequency stability | A + I | `allan` vs NIST SP 1065 / Stable32 (1e-4); §3 row | OPEN (template) |
| NAV-PERF-020 | Integrity ARAIM | A + I | `raim`/`sbas` vs DO-229E + real IGS SP3; §3 row | OPEN (template) |
| NAV-PERF-030 | Orbit propagation | A + I | SGP4 vs 666 AIAA vectors (4.12 mm); §3 row | OPEN (template) |
| NAV-PERF-110 | Clock holdover (GNSS-denied) | A | `holdover` model vs Kalman recursion (modelled); floor-caveat carried | OPEN (template) |
| NAV-PERF-120 | Quantum dead-reckoning | A | `quantum_imu` `QuantumNavBudget` (modelled; Freier-2016 bracket) | OPEN (template) |
| NAV-PERF-122 | Quantum-vs-classical trade | A | `quantum_trade::quantum_vs_classical_trade` neutral-path benefit table (modelled) | OPEN (template) |
| NAV-PERF-123 | Measured-ADEV ingestion (defensible floor) | A | `quantum_trade::qparams_from_adev_curve` NNLS, round-trip vs `q_from_allan` (modelled) | OPEN (template) |
| NAV-PERF-124 | Resilience-vs-time envelope | A | `quantum_trade::resilience_envelope` finite coast-time FoM + alt-PNT bound (modelled) | OPEN (template) |
| NAV-PERF-125 | Hybrid 17-state UKF self-consistency | A + T | `fusion::hybrid_ukf` NEES/NIS vs χ² bands; mistuned filter flagged (modelled self-consistency, not accuracy) | OPEN (template) |
| NAV-PERF-145 | Alt-PNT availability bound | A | `gravimeter`/`altpnt` ≈145 m / 60-min outage vs ≈70 km free-INS (modelled; synthetic maps) | OPEN (template) |
| NAV-PERF-150 | Reproducibility / SW PA | I | golden/determinism/cross-platform gates; SBOM; provenance | OPEN (template) |
| NAV-PERF-900 | Spacecraft-bus engineering | (partner) | partner deliverable — not Ashforde | PARTNER |
| NAV-PERF-910 | RF payload/antenna HW | (partner) | partner deliverable — not Ashforde | PARTNER |
| NAV-PERF-920 | Quantum payload HW | (partner) | partner deliverable — not Ashforde | PARTNER |
| NAV-PERF-930 | Flight-hardware PA (MAIT) | (partner) | partner deliverable — not Ashforde | PARTNER |

*Template usage:* replace Req-IDs with real SRD IDs; set Method per the prime's verification
allocation; move Status OPEN → CLOSED only when the evidence cited has been accepted by the
prime's SE lead. The §3 machine-checked matrix is the source for the "evidence" column of the
navigation-performance rows.

---

## 6. Honest residuals

Stated plainly, because honesty is the differentiator in this bid:

1. **No ECSS formal-deliverable track record yet.** Ashforde applies ECSS-E-ST-10-02 *in
   style* and keeps the discipline (validated/modelled invariants, traceability, reproducible
   evidence), but has **not previously produced a contractual ECSS verification deliverable**
   on an ESA programme. The prime's SE function carries the formal authority and format
   ownership; Ashforde supplies disciplined inputs.

2. **The RTM is a template until the SoW is in hand.** All requirement IDs here are
   placeholders (`NAV-PERF-*`). The authoritative requirement set is the SRD derived from SoW
   **FPD-10029**, which is **not yet obtained**. The matrix in §3, while machine-checked for
   *internal honesty invariants*, must be re-keyed to the real requirement IDs and each
   compliance verdict re-checked before submission.

3. **The named SE lead is the prime's.** Ashforde does not provide the system SE authority, the
   system SRD, the DDVP, or the master VCD/RTM. Those are prime-owned. Ashforde owns only the
   navigation-performance verification *inputs*.

4. **Partner-owned engineering is excluded by design** (and recorded as `PARTNER` rows, not
   silently omitted): spacecraft-bus engineering, RF-payload/antenna **hardware** design,
   quantum-payload **hardware** design & maturation, and flight-hardware PA (radiation/EEE/MAIT).
   Kshana is a navigation-performance simulator.

5. **TRL & heritage.** Kshana is a simulation tool at TRL ≈ 3–4. No flight heritage, no ESA
   endorsement, no certification (e.g. DO-229/ARAIM certification), and no hardware capability
   is claimed. The "validated" label is used strictly for external-oracle checks; everything
   else is "modelled".

6. **Consortium-gated items** (the seat itself, the prime, the real SoW, Key-Personnel CVs,
   costing) are founder-gated and not pre-drafted here. This plan becomes contractually
   meaningful only inside a secured consortium seat.

---

*This is a DRAFT/TEMPLATE element-level verification plan, owned by Ashforde OÜ as the
navigation-performance SME. It feeds — and does not replace — the prime's system-level
verification programme. All quantitative claims trace to Kshana v0.17.0 `docs/CAPABILITY.md`
and `docs/VALIDATION.md` under the validated-or-modelled discipline. Contact:
**contact@ashforde.org**.*
