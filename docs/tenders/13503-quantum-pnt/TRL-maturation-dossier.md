<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; never publish in the open kshana repo.
TRL maturation dossier for Kshana toward the AO/1-13503 IOD (2030-31).
Owner: Ashforde OÜ / Chakshu Baweja. Contact: contact@ashforde.org
-->

# TRL Maturation Dossier — Kshana toward the AO/1-13503 Quantum-Enabled PNT Demonstrator (IOD 2030-31)

> **STATUS: DRAFT / TEMPLATE.** This dossier states Ashforde's honest position today and the
> role Kshana would play within a prime-led consortium. Sections marked **[founder-gated]** or
> **[awaits SoW / teaming]** cannot be finalised from code and audit alone; they depend on the
> real Statement of Work (**FPD-10029**), the consortium WBS, and the named partners. Numbers
> taken from Kshana are stated at their honest verification level — **validated** (checked against
> an independent external oracle) or **modelled** (first-principles / published formula, checked
> only internally). No flight heritage, ESA endorsement, certification, or hardware capability is
> claimed.
>
> **Tender:** ESA AO/1-13503 — Quantum-Enabled PNT Demonstrator (FutureNAV), Phases 0 / A / B1,
> study toward In-Orbit Demonstration (IOD) 2030-31.
> **Ashforde's role:** the navigation-performance and quantum-vs-classical trade-study SME within
> a prime-led consortium. **Not** prime, **not** the hardware-maturation vehicle.
> **Owner / contact:** Ashforde OÜ — contact@ashforde.org

---

## 0. The one honest claim this dossier makes

Kshana is a **TRL ~3–4 navigation-performance simulation substrate**. It is *not* the article being
matured to flight. The thing AO/1-13503 matures toward IOD is **hardware** — a quantum PNT payload,
its RF/antenna chain, and the spacecraft bus that flies them. **TRL advancement in this programme is
the hardware's.** Kshana's job is to make that maturation case *defensible*: to supply the
performance evidence, close the quantum-vs-classical trade, and quantify GNSS-denied resilience, so
that each phase gate is met with reproducible, externally-anchored numbers rather than assertion.

A simulator does not "reach TRL 7". The demonstrator does. Kshana **supports** the climb; it does not
**drive** it. Every claim below is written to that discipline.

---

## 1. Honest current-state TRL assessment

### 1.1 Where Kshana sits

| | Assessment |
|---|---|
| **Object** | Kshana — open-core PNT-performance-and-resilience simulator (v0.20.0, dual-licensed: GNU AGPL-3.0-only + commercial licence), plus a private contract-extensible overlay (Kshana Pro) |
| **TRL (the software analysis tool itself)** | **~3–4.** Analytical and experimental proof-of-concept of the modelling, with externally-validated core components; **not** integrated into a relevant operational environment and carrying **no flight heritage** |
| **What it is** | A neutral, reproducible performance simulator that scores candidate PNT architectures on common figures of merit and labels every result *validated* or *modelled* |
| **What it is NOT** | A spacecraft-systems house, a hardware-maturation vehicle, an RF/quantum-payload designer, or a flight product. It does not itself climb to TRL 7. |

The TRL ~3–4 figure is the **software analysis tool's own** readiness, not a system TRL. It is offered
honestly: some core components are validated to a high standard (below), but the tool has not been
integrated with mission hardware in a relevant environment, which is what a higher TRL would require —
and which is, by design, the consortium's hardware work, not Kshana's.

### 1.2 Evidence basis — externally validated core

These capabilities are checked **bit-for-bit or to a stated numerical tolerance against an independent
external oracle**. They are the credibility anchors a study reviewer can re-run.

| Capability | External oracle | Result |
|---|---|---|
| Orbit propagation (SGP4/SDP4) | AIAA 2006-6753 — all 666 reference vectors | position to **4.12 mm**, velocity to ~1 mm/s |
| Reference frames (precession / nutation / CIO GCRS↔ITRS) | IAU SOFA/ERFA reference vectors + Vallado worked example | nutation Δψ/Δε to 1e-13 rad; full CIO GCRS→ITRS to **4 mm** |
| Allan deviation family (ADEV/MDEV/TDEV/HDEV + edf) | NIST SP 1065 closed forms; Stable32 on the canonical NBS14 dataset | matches Stable32 deviations to **1e-4** |
| Single-point positioning (PVT) | real IGS data, station ABMF (Guadeloupe, 2018-05-13) vs surveyed coordinate | **5.7 m 3-D RMS / 1.1 m horizontal** |
| RAIM / ARAIM / SBAS geometry | DO-229E formulation; exercised on real IGS SP3 + real GPS+Galileo TLEs | reproduces published integrity behaviour |
| Ranging-code cross-correlation | published GPS C/A Gold-code value | **−23.9 dB**, as published |

### 1.3 Evidence basis — modelled physics (first-principles / published, internally checked)

These are real, bounded models built from published physics and checked **internally** (against
closed forms, conservation laws, hand computation, or Monte-Carlo self-consistency). They are stated
as **modelled**, never validated, and several carry explicit honesty caveats.

- **GNSS-denied clock holdover** (`src/holdover.rs`): closed-form van-Loan coast-error growth,
  holdover-to-threshold, and quantum-clock classes. The long-τ result is **governed by an assumed
  red-noise floor, stated honestly** — it is a parameterised model, not a measured device curve.
- **Quantum-inertial dead-reckoning budget** (`src/inertial/quantum_imu.rs`, `QuantumNavBudget`):
  bias + scale-factor + velocity-random-walk + long-term-stability terms over a holdover; the
  first-principles cold-atom-interferometer accelerometer phase model is checked to lie within ~2
  orders of the published Freier et al. 2016 mobile gravimeter (96 nm/s²/√Hz). Modelled, not a flight
  budget.
- **CR3BP / NRHO** (`src/cr3bp.rs`), **nav-signal modulation analysis** (`src/navsignal.rs`),
  **jamming link budget** (`src/jamming.rs`), **spoof characterisation** (TEXBAT *parameters*, not raw
  IQ), and **alternative-PNT** (gravity / terrain / magnetic) — all modelled and internally checked,
  with their boundaries documented in `docs/CAPABILITY.md`.

### 1.4 Evidence basis — software-quality and verification infrastructure

The assurance discipline an ECSS study rewards, all present today:

- A **machine-checked verification matrix** (`src/verification.rs`, `verification_matrix()`): each row
  is *requirement → module → test → oracle → OracleKind → status*, with **unit-tested honesty
  invariants** — a row may carry status **VALIDATED only if it is backed by an `ExternalDataset`
  oracle**. This is the executable backbone for an ECSS-E-ST-10-02 verification cross-reference, and it
  *also* carries explicit **PartnerOwned** rows for the hardware Kshana does not provide (see §4).
- **~1500+ unit/integration tests**; **golden**, **determinism**, and **cross-platform** CI gates;
  `clippy` + `fmt` gates.
- **CycloneDX 1.5 SBOM** (`scripts/gen-sbom.sh`), pinned MSRV via committed `rust-toolchain`, committed
  `Cargo.lock`, **SLSA build provenance**, and a documented reproducibility procedure
  (`docs/REPRODUCIBILITY.md`).
- A public **honest maturity ledger** (`docs/CAPABILITY.md`, `docs/VALIDATION.md`,
  `docs/CLAIMS-VS-REALITY.md`) stating, per capability, exactly what is *yes / partial / none*, with a
  CI guard that keeps resolved overclaims resolved.

---

## 2. Kshana's role across the demonstrator phases — and what it is NOT

### 2.1 What Kshana DOES across Phases 0 / A / B1

| Phase | Kshana's contribution (the navigation-performance work package) |
|---|---|
| **Phase 0** (feasibility) | **Mission analysis** — Walker/constellation design + gradient-free optimiser, coverage / revisit / DOP / availability, SGP4-validated orbits. **Quantum-vs-classical trade framing** — a single neutral code path scoring candidate clock and inertial architectures on common figures of merit, every result reproducible from a committed scenario + fixed seed + pinned engine version. |
| **Phase A** (preliminary definition) | **Nav-performance trade studies** — clock holdover, quantum-inertial dead-reckoning budgets, timing architecture (ADEV family + van-Loan-Q Kalman clock + two-way time transfer). **GNSS-denied resilience benchmarking** — anti-jam link budget, multi-layer spoof detection, alt-PNT (gravity-map / terrain / magnetic) fused navigator. Feeds **performance requirements** down to the RF and quantum-payload partners. |
| **Phase B1** (consolidation toward PDR) | **Performance-requirement verification evidence** — turning the SoW's nav-performance requirements into the simulator-side rows of an ECSS-E-ST-10-02 verification cross-reference matrix, with each row tagged *validated* or *modelled* and tied to a re-runnable scenario. **Trade closure** supporting the demonstrator's preliminary design. |

In one line: Kshana answers **"how well does this PNT architecture perform, and how does it degrade?"**
with reproducible, oracle-anchored numbers — exactly the analytical evidence the early study phases
consume.

### 2.2 What Kshana is NOT (the consortium's hardware-maturation work)

Kshana is a navigation-**performance simulator**. It is explicitly **not**, and does not pretend to be:

- The **quantum payload hardware** — cold-atom / clock device design, build, and maturation. Kshana
  models device *performance* from published coefficients (with the CAI accelerometer first-principles
  excepted); it does not design or mature the device.
- The **RF payload / antenna hardware** — radiation patterns, front-end, signal-generation hardware.
  Kshana models the **measurement domain and link budget**; the payload and antenna *hardware* are a
  partner's.
- The **spacecraft bus** — AOCS/GNC, thermal, structures, propulsion, EEE/power.
- **Flight-hardware product assurance** — radiation, EEE parts, MAIT. Kshana speaks to **software** PA
  only.

These four exclusions are not inferred from silence — they are **encoded as `PartnerOwned` rows in the
machine-checked verification matrix** (`src/verification.rs`), so the boundary is part of the tool's own
honest self-description, not just prose in a bid.

---

## 3. TRL 3 → TRL 5 → TRL 7 evidence roadmap (how Kshana's analysis feeds each gate)

The TRL ladder below is the **demonstrator hardware's**. Kshana appears in the right-hand column as the
**analysis substrate that supplies evidence for the gate** — never as the thing being matured. The
honest caveat throughout: **TRL advancement is the hardware's, supported-but-not-driven by simulation.**

| Gate / milestone | Hardware TRL focus (consortium-owned) | Kshana's supporting evidence (analysis only) |
|---|---|---|
| **PRR / early Phase 0–A** (TRL 3 → toward 4) | Quantum-payload and RF concepts at analytical/lab proof-of-concept | **Quantum-vs-classical trade closure** — neutral, reproducible scoring of candidate architectures on common FoM; mission-analysis constellation/coverage/DOP trades; GNSS-denied resilience baseline. Establishes *which* architecture the hardware should mature. |
| **Toward TRL 5** (Phase A → B1) | Components validated in a relevant environment (breadboard / lab-representative) | **Performance-requirement derivation and verification-by-analysis** — translate measured/lab device parameters into end-to-end nav-performance predictions (holdover, dead-reckoning budgets, timing architecture, integrity), feeding the partners' requirement allocation; benchmark resilience (jamming, spoofing, alt-PNT) against requirement thresholds. |
| **B1 / PDR** | Preliminary design consolidated; requirement traceability established | **Verification cross-reference contribution** — populate the simulator-side rows of an ECSS-E-ST-10-02 matrix, each tagged *validated* (external oracle) or *modelled*, each tied to a committed, re-runnable scenario; close the headline performance trades supporting PDR. |
| **Toward TRL 7** (later phases, post-AO/1-13503 study scope) | System prototype demonstrated in an operational / flight-representative environment (the IOD) | **Analysis-side verification support** — pre-flight performance predictions, GNSS-denied resilience envelopes, and post-test reconciliation of flight/representative data against modelled predictions. **The TRL 7 claim is the hardware's, demonstrated in flight; Kshana provides the predictive and reconciliation analysis, not the demonstration.** |

**Honest caveat, restated for each gate:** the simulator can *predict* and *verify-by-analysis*; it
cannot *demonstrate in a relevant or operational environment*. The environment-integration evidence —
the substance of TRL 5, 6, 7 — is generated by the hardware partners' breadboards, EMs, and the IOD
itself. Where a row would require Kshana to "validate" a quantity it has only **modelled**, the matrix
will (and the honesty invariant enforces this) refuse the VALIDATED tag.

---

## 4. Partner-owned hardware-maturation gaps (stated plainly)

These are **not** Kshana capabilities and **not** Ashforde deliverables. They are the substance of the
demonstrator's TRL climb and belong to consortium partners. They are listed here so the boundary is
unambiguous, and they correspond one-to-one to the `PartnerOwned` rows in
`src/verification.rs`.

1. **Quantum-payload hardware design & maturation** — cold-atom / clock device engineering, build, test,
   and TRL advancement. *Ashforde provides performance models, not the device.* This partner is the
   consortium's technical anchor.
2. **RF-payload / antenna hardware design** — signal-generation hardware, front-end, antenna radiation
   patterns. *Ashforde models the measurement domain and link budget and feeds performance requirements
   to this partner.*
3. **Spacecraft-bus engineering** — AOCS/GNC, thermal, structures, propulsion, EEE/power.
4. **Flight-hardware product assurance** — radiation, EEE parts, MAIT. *Ashforde's PA scope is
   software only.*

A defensible AO/1-13503 bid therefore requires a prime plus, at minimum, a quantum-hardware group, an
RF/payload partner, and bus/SE coverage. Ashforde slots in as the **navigation-performance / trade-study
SME** alongside them — a German-nationality SME (ESA-STAR entity 83208) that also helps the ≥3-SME quota
and the DE-weighted geographical return.

---

## 5. Founder-gated and residual items

These cannot be completed from code and audit alone. They are flagged so this dossier reads as the
**draft / template** it is.

- **[awaits SoW]** Re-baseline §2 and §3 against the real **Statement of Work (FPD-10029)** once
  obtained from the esa-star tender package. The phase-to-contribution mapping above is built from the
  public tender framing and Kshana's capability ledger; the exact task IDs, inputs, outputs, and
  exclusions must be reconciled to the SoW.
- **[awaits teaming]** Confirm Kshana's work-package boundary against the **consortium WBS** and the
  named partners — in particular the requirement-allocation interface to the quantum and RF/payload
  partners, and the mandatory quantum-nav Key Person (expected from the hardware partner).
- **[founder-gated]** Founder CV, man-hour allocation, and WP costing (ECOS/PBF, through the prime);
  the German registered-office / branch eligibility decision and ESA-STAR full registration (bank +
  financials) before any award.
- **[residual, honest]** Items Kshana itself has not yet validated externally and would not claim at any
  TRL gate: raw-vector spoof validation (TEXBAT IQ / licensed Spirent), real DSN/ESTRACK tracking-data
  validation for the deep-space path, NaveGo/KF-GINS cross-validation of the fusion stack, and DE-grade
  ephemeris in the core. These remain **modelled** until an external oracle exists, by design.

---

*Open AGPL-3.0 core (dual-licensed; a commercial licence is available from Ashforde OÜ); a private
contract-extensible overlay (Kshana Pro) is available for proprietary / export-sensitive work under NDA
and the appropriate clearance. Owner: Ashforde OÜ / Chakshu Baweja. All external contact via
**contact@ashforde.org**.*
