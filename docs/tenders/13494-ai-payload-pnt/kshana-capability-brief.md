# KSHANA — Capability Brief for ESA Tender AO/1-13494/26/NL/RR
### AI & Digital Payload Technologies — PNT Demonstrator (FutureNAV, Component 3), Phases 0/A/B1

**PRIVATE — Ashforde OÜ internal working document.** Prime-facing leave-behind. Not for external distribution.
**Date: 2026-06-15 · Contact: contact@ashforde.org**

---

## 1. One honest identity

Kshana is **one component of the NAV-DT, not the whole twin.** Ashforde OÜ (sole founder, Chakshu Baweja) offers a consortium SME a single, tightly-scoped work-package seat covering three things it can do well and can prove:

1. **The RF-performance component of the Navigation Digital Twin (NAV-DT)** — a Figure-of-Merit engine that predicts ranging/timing accuracy, availability, and integrity (RAIM/ARAIM), plus GNSS-denied resilience.
2. **A detector-agnostic AI-algorithm evaluation testbed** — the harness that owns the SoW's *"Characterisation & Evaluation of Machine-Learning Techniques"* line: ROC/AUC, confusion, Pfa/Pmd, leakage guard, stratified split, distribution-shift optimism report — paired with a labelled synthetic RF-impairment scenario generator.
3. **A machine-checked verification matrix** — the engineering artefact that refuses to mark a requirement "validated" without an external oracle.

Kshana is an open Apache-2.0 Rust core (911+ library tests, fmt + clippy clean, adversarially reviewed) a prime can audit line-by-line. **TRL ~3.** No flight heritage, no agency contracts, no certification — stated plainly.

---

## 2. Capability → module → status

Every claim is labelled **validated** (checked against an external oracle) or **modelled** (parameter-grounded, no independent oracle yet).

| # | Capability (mapped to SoW) | Kshana module | Status |
|---|---|---|---|
| 1 | PNT Figure-of-Merit engine: ranging/timing accuracy, availability, integrity (RAIM/ARAIM), GNSS-denied resilience — feeds **D1.1** benefits, **D2.6** feasibility, **D2.16** NAV-DT achievable-performance | FoM / performance core | **modelled** |
| 2 | Labelled synthetic RF-impairment scenario generator (parameter-grounded — TEXBAT params, **not raw IQ**) | `src/impairment_eval.rs` | **modelled** |
| 3 | Detector-agnostic AI-algorithm evaluation testbed: ROC/AUC, confusion, Pfa/Pmd, leakage guard, stratified split, distribution-shift optimism report — owns SoW *"Characterisation & Evaluation of ML Techniques"*; bundled detectors are transparent published-method baselines (**not SOTA**) | `src/impairment_eval.rs` | **validated** |
| 4 | Machine-checked verification matrix (validated only against an external oracle) — the **D2.2** PA/QA + **D2.3** SEP + ECSS-E-ST-10-02 verification cross-reference contribution | `src/verification.rs` | **validated** |
| 5 | Constellation / orbit / mission-geometry feasibility for the LEO IOD — **D1.3** mission-concept trade, **D1.4** MRD sizing | mission-geometry core | **modelled** |

These five contributions touch the NAV-DT achievable-performance assessment, the anomaly/RF detection-classification-mitigation **evaluation** layer, and the verification cross-reference — concentrated in **Task 1 (D1.1, D1.3, D1.4)**, **Task 2 (D2.2, D2.3, D2.6, D2.16)**, and the **Task 3** NAV-DT algorithm-verification and performance-benchmark deliverables.

---

## 3. Explicitly NOT in scope (partner-owned gaps)

Ashforde will **not** bid these. They are partner- or prime-owned, and naming them honestly is part of the offer:

- On-board AI / edge inference engine; secure in-orbit update / reconfiguration / fault-containment
- Steerable phased-array antennas, radiation-pattern design, antenna IF hardware
- Payload hardware: SDR, FPGA, reconfigurable processing chains
- Receiver acquisition / tracking / correlator, raw-IQ processing
- DestinE-style Digital Twins, PNT Data Lakehouse / Navigation Data Space, Core Ground Infrastructure / ground stations
- "App-store for space" platform; spacecraft-as-a-service platform engineering
- LCA / debris / eco-design / sustainability assessment

Kshana provides the RF-performance and algorithm-evaluation **layers** of the NAV-DT; the high-fidelity payload + RF-environment twin, the SpaceStream interface, and the closed loop to NAVEX are prime/partner-led, with Kshana feeding the achievable-performance and algorithm-verification slices.

---

## 4. Score-uplift logic (against the 20 / 30 / 30 / 10 / 10 criteria)

How a Kshana WP seat raises a prime's scored proposal:

- **20% — Background & experience:** Adds a working, test-covered, openly auditable PNT-performance and ML-evaluation codebase as concrete evidence, not a heritage claim. DE-SME presence also feeds the consortium-composition view (criterion 1).
- **30% — Understanding of requirements & objectives:** Kshana directly instruments the SoW's harder-to-fake lines — *"Characterisation & Evaluation of ML Techniques"*, NAV-DT *achievable-performance assessment + algorithm verification*, and the verification cross-reference — demonstrating depth on the AI-evaluation and performance-prediction objectives.
- **30% — Quality & suitability of proposed programme:** The verification matrix (refuses "validated" without an external oracle), leakage guard, stratified split, and distribution-shift optimism report are direct, auditable quality/engineering evidence for the PA/QA, SEP, and ECSS-E-ST-10-02 verification approach.
- **10% — Management, costing, planning:** A single, narrow, fixed-scope SME WP is low-risk to integrate and cost; clear interfaces, no scope creep into partner-owned hardware.
- **10% — Compliance with administrative tender conditions:** DE nationality in ESA-STAR + SME status mechanically lift consortium-composition scoring (≥3 SMEs, ≥30% non-LSI budget). *Open-source note:* Kshana's Apache-2.0 core requires **ESA prior authorisation** before use under GCC Part II Option A — to be requested by the prime via the BIPR/BIPR-table route; Kshana background IP is declarable.

---

## 5. The moat

A **unit-tested verification matrix that refuses to mark anything "validated" without an external oracle**, sitting on an **open Apache-2.0 core a prime can audit line-by-line** — versus the black-box simulators of GMV (magicGNSS / Service Volume Simulator), Qascom (SimSAFE / QA707), M3 Systems (StellaNGC), IFEN (NavX-NCS), Spirent, Safran-Skydel, ANavS. For a Phase 0/A/B1 study whose credibility rests on *defensible* achievable-performance numbers and on proving ML detectors actually generalise, an auditable evaluation testbed that reports its own optimism is a structural differentiator, not a feature.

---

## 6. Geo-return value & the registered-office risk (stated openly)

- **Value:** Ashforde OÜ is **DE-nationality in ESA-STAR** (top geo-return) and an **SME** — both mechanically improve a prime's scored consortium criteria for a Component-3 study.
- **Risk (#1 founder-gated item):** Ashforde OÜ's **registered office is in Estonia, NOT a participating state.** The tender's registered-office rule requires every operator — including each subcontractor — to hold a registered office in a participating state (BE CA CH CZ DE ES GB IT LV NL NO PL SE) **by award**. Light esa-star registration suffices at submission (9 July 2026), but award eligibility is not yet met.
- **Fix:** German branch / subsidiary, plus an ESA Q&A clarification by the ~29 June 2026 clarification deadline. This is disclosed up front; no eligibility claim is made beyond submission-stage Light registration until the office is established.

---

## 7. What this offer is — and is not

**Is:** a defined, auditable, fixed-scope SME work-package — the RF-performance component of the NAV-DT + the AI-algorithm evaluation testbed + the machine-checked verification matrix — ready for a prime to fold into a 13494 consortium proposal.

**Is not:** a prime, a Key Person, the whole NAV-DT, or any of the partner-owned hardware/ground/platform elements. Ashforde does not satisfy the consortium or Key-Personnel mandates alone and does not claim to.

*Prime/partner outreach is parked pending explicit founder go. This document defines the asset and the partnering criteria only.*

---
*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator · capability claims labelled validated (external oracle) or modelled throughout.*