<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; never publish in
the open kshana repo. Clean PBF-7-style work-package descriptors for the two adjacent
WPs a prime drops into a 13494 consortium WBS. AO/1-13494/26/NL/RR, SoW FPD-10030.
Contact: contact@ashforde.org
-->

# Work-package descriptors (PBF-7 paste-in) — Ashforde / Kshana for AO/1-13494/26/NL/RR

**Programme:** ESA FutureNAV, Component 3 — AI & Digital Payload Technologies PNT Demonstrator (Phases 0/A/B1).
**SoW reference:** FPD-10030 iss 1.0 (20/05/2026); esa-star Activity 1000046439.
**Owner / operator:** Ashforde OÜ (SME) — **consortium-SME work-package contributor, never prime, never a mandated Key Person.**
**Contact:** contact@ashforde.org

> **PURPOSE — this is the PBF-7 paste-in form.** The narrative WP rationale, full
> objectives/inputs/tasks/outputs prose, and the SoW deliverable cross-walk live in
> [`technical-contribution.md`](technical-contribution.md) §2–§4 (the *why* and the detailed
> method). **This file is the clean, field-by-field WBS descriptor** a prime drops straight into
> a Cost-Breakdown / PBF-7 work-package sheet — to be cross-referenced with, not duplicated from,
> the technical contribution. Phasing, manpower and price are **founder-/consortium-gated** and
> left as `[TBD — prime + founder]`. Cross-references: capability brief
> [`kshana-capability-brief.md`](kshana-capability-brief.md), SE/verification plan
> [`SE-verification-plan.md`](SE-verification-plan.md), sizing/benefit method
> [`D1.1-D1.3-constellation-sizing-benefits.md`](D1.1-D1.3-constellation-sizing-benefits.md).

> **TEMPLATE NOTE.** WP/WBS numbers (`WP-x.y`), man-hours, price and exact phasing are filled by
> the prime against the issued FPD-10030 SRD and the consortium WBS. Every capability cited is a
> built, tested module in the open Apache-2.0 Kshana core (v0.17.x; ~915 library `#[test]`
> functions, fmt + clippy clean) and the `kshana-pro` overlay. **TRL ~3, no flight heritage, no
> certification, no ESA endorsement.**

---

## WP-A — PNT performance & resilience prediction component of the NAV-DT

| PBF-7 field | Content |
|---|---|
| **WP no.** | `[TBD — prime WBS]` |
| **WP title** | PNT performance & resilience prediction component of the NAV-DT |
| **Responsible operator** | Ashforde OÜ (SME, DE-nationality in ESA-STAR) |
| **Phasing (planned)** | Phase 0/A: analysis & requirements input. Phase B1: build & deliver the component + its verification cross-reference. `[Exact months/effort TBD — prime + founder]` |
| **WP objective** | Provide the **RF/performance figure-of-merit engine** supplying the NAV-DT's *achievable-performance* assessment: predict ranging/timing accuracy, availability, integrity (RAIM/ARAIM protection levels) and GNSS-denied resilience for the candidate LEO IOD constellation/orbit and RF environment — as an auditable, reproducible, externally-cross-checked component the prime integrates behind the NAV-DT interface. **One component, not the whole twin.** |
| **Inputs (from prime / consortium)** | IOD mission concept & constellation/orbit options (LEO); candidate payload performance envelope (ranging error budget, antenna/link assumptions) from the payload WP; RF-environment definition (iono/tropo/multipath/interference) from the environment-model WP; the NAV-DT / "SpaceStream" aggregated-metrics interface contract from the ground-segment WP; target PNT KPIs from the user-segment WP. |
| **Tasks** | **A1** Map candidate constellation/orbit geometry & visibility (LEO IOD), feeding sizing — `kshana/src/walker.rs` (`pdop_sweep`, `coverage_revisit`, `optimize_walker_design`, `min_satellites_streets_of_coverage`); see [`D1.1-D1.3-constellation-sizing-benefits.md`](D1.1-D1.3-constellation-sizing-benefits.md). **A2** Compute the positioning/timing FoM set per scenario (position/velocity error, timing stability, availability) — `kshana/src/fom.rs` (`positioning_performance`, `score`). **A3** Compute snapshot/sequential **integrity** protection levels (HPL/VPL) via real least-squares RAIM/ARAIM — `kshana/src/raim.rs` (`snapshot_raim` + ARAIM budget). **A4** Quantify **GNSS-denied resilience** (% availability under jamming, error growth under outage) — `kshana/src/jamming.rs` (`availability_under_jamming`). **A5** Sweep / Monte-Carlo the scenario space to map where the architecture meets KPIs, each result carrying a reproducible `scenario_hash`. **A6** Wrap A2–A5 as a NAV-DT-consumable component against the prime's interface; emit the **verification cross-reference** — `kshana/src/verification.rs` + the MBSE artifacts (`kshana-pro/src/mbse`). |
| **Outputs / deliverable items** | **O-A1** achievable-performance dataset (FoM tables: accuracy/availability/integrity/resilience per scenario, each with uncertainty + a validated/modelled tag) → **D2.16 / D2.6**. **O-A2** integrity-budget contribution (HPL/VPL, RAIM/ARAIM) → **D2.16 / D2.6**. **O-A3** GNSS-denied resilience benefit evidence → **D1.1**. **O-A4** constellation/orbit sizing inputs → **D1.3 / D1.4**. **O-A5** machine-checked verification cross-reference (RTM/ReqIF/margins) → **D2.2 / D2.3**. **O-A6** the integrated NAV-DT performance component (Phase B1), reproducible (scenario-hashed). |
| **Milestones / review feed** | MDR: sizing options + performance baseline. PRR: feasibility FoM tables + RTM skeleton. GATEKEEPER: defensible feasibility numbers. ISRR/FR (Phase B1): delivered component + populated verification cross-reference. (See [`SE-verification-plan.md`](SE-verification-plan.md) §5.) |
| **Interfaces** | Consumes the NAV-DT / SpaceStream aggregated-metrics interface and the payload performance envelope; feeds predicted performance/resilience back. The interface contract is a **prime input** — Ashforde delivers the component, not the SpaceStream link or the NAVEX side. |
| **Verification method (ECSS-E-ST-10-02)** | Primarily **Analysis** (the engine *is* a performance-analysis tool) + **Inspection** (tests, SBOM, machine-checked matrix). Integrity/orbit/frames cores are **Validated** (external oracle); sizing/FoM/resilience are **Modelled**. |
| **Honesty status** | **modelled** overall (the integrity/orbit/positioning cores it composes are externally validated; the NAV-DT achievable-performance synthesis is modelled). TRL ~3. |
| **EXCLUSIONS (partner-owned / out of scope — DO NOT BID)** | The **full** NAV-DT (this is only the RF/performance component); the high-fidelity *payload* digital twin; DestinE-style iono/tropo/multipath twins (Ashforde *consumes* the environment definition); Core Ground Infrastructure / PNT Data Lakehouse / Navigation Data Space; on-board / edge AI inference engine; secure in-orbit reconfiguration; steerable phased-array antennas & radiation patterns; SDR/FPGA/payload hardware; receiver acquisition/tracking/correlator and raw-IQ; the reconfiguration mechanism and the "app-store-for-space" / spacecraft-as-a-service platform; LCA / debris / eco-design. |
| **Effort / price** | `[TBD — prime + founder; ECOS/PBF via prime; fee ≤ 8% of Total Company Cost (KAF); respect Phase-1 ≤ €1.0M / Phase-2 ≤ €2.5M caps]` |

---

## WP-B — AI-algorithm evaluation testbed + labelled RF-impairment scenario generator

| PBF-7 field | Content |
|---|---|
| **WP no.** | `[TBD — prime WBS]` |
| **WP title** | AI-algorithm evaluation testbed + labelled RF-impairment scenario generator |
| **Responsible operator** | Ashforde OÜ (SME, DE-nationality in ESA-STAR) |
| **Phasing (planned)** | Phase A: method definition & evaluation-protocol design (feeds feasibility). Phase B1: deliver the testbed + corpus generator; run the consortium's anomaly / RF-impairment detection-classification algorithms through it under representative RF conditions. `[Exact months/effort TBD]` |
| **WP objective** | Own the SoW's **"Characterisation & Evaluation of Machine-Learning Techniques"**: provide (1) a **labelled, parameter-grounded synthetic RF-impairment corpus generator** and (2) a **detector-agnostic evaluation testbed** that scores *any* candidate anomaly/RF-impairment detection-classification algorithm with the operating-characteristic metrics a reviewer expects — ROC/AUC, confusion matrix, Pfa/Pmd at a chosen operating point, per-impairment-class breakdown — with a leakage guard, stratified split, and an explicit distribution-shift optimism report. The testbed is the **referee, not a classifier**. |
| **Inputs (from prime / consortium)** | The consortium's candidate detection/classification algorithms (behind the testbed's detector interface); the RF-impairment taxonomy & severity ranges (jamming, spoofing, multipath, RF anomalies) from the payload/AI WPs; representative RF scenario definitions / operating points from the environment & system WPs; on-board/edge implementation constraints (latency, memory, observable availability) from the on-board-AI partner, used to scope which observables a detector may use. |
| **Tasks** | **B1** Generate a labelled parameter-grounded corpus over the impairment taxonomy — `kshana/src/impairment_eval.rs` (`generate_corpus`, `CorpusConfig`, `ImpairmentClass`; composing the jamming / signal-quality-AGC / detection models; **TEXBAT-style parameters, never raw IQ or field captures**). **B2** Implement the evaluation protocol: stratified train/test split with disjoint partition keys (`stratified_split`) + exact-duplication **leakage guard** (`Split::has_leakage`). **B3** Score candidate detectors through the `ImpairmentDetector` trait: ROC (`roc_curve`), AUC (`auc`), confusion at operating point (`confusion_at`, `Confusion::pd`/`pmd`), per-class breakdown (`evaluate`). **B4** Produce the **distribution-shift optimism report** (`distribution_shift_report`) — quantify how much in-tuning-regime scores overstate out-of-regime performance. **B5** Define & document the evaluation operating protocol (fixed seed, pinned engine version, declared operating point) for reproducible, comparable results. **B6** Run the consortium algorithms through B1–B4 under representative RF scenarios; deliver the scored evidence. |
| **Outputs / deliverable items** | **O-B1** the labelled RF-impairment corpus generator (configurable, seeded, reproducible) → ML-characterisation input for **D2.6** + Phase-B1 evidence. **O-B2** the detector-agnostic evaluation testbed (ROC/AUC/confusion/Pfa-Pmd, leakage guard, stratified split) → Phase-B1 "verification of AI algorithms" evidence. **O-B3** the distribution-shift optimism report per algorithm → feasibility/honesty evidence for **D2.6** and the GATEKEEPER. **O-B4** documented evaluation protocol → **D2.3** SE-plan verification approach for AI elements. **O-B5** scored operating-characteristic evidence for the consortium's detection-classification algorithms (Phase B1). |
| **Milestones / review feed** | PRR: evaluation-protocol definition. GATEKEEPER: distribution-shift optimism report (the honesty check on whether detectors generalise). ISRR/FR: delivered testbed + corpus generator + scored detector evidence. |
| **Interfaces** | Candidate algorithms enter behind the `ImpairmentDetector` trait; constraint envelopes (observables/latency) are a partner input. Ashforde delivers the referee + corpus, **not** the algorithms and **not** the on-board inference path. |
| **Verification method (ECSS-E-ST-10-02)** | **Analysis + Inspection + Test** — the leakage guard, ROC/AUC and well-formedness are unit-tested invariants; detector evaluation is a reproducible test run. |
| **Honesty status** | **modelled** — the testbed machinery is unit-tested, but an AUC here is over **model-derived labels** from a **synthetic, parameter-grounded** corpus (not raw IQ / not field captures), reported as exactly that. Bundled detectors are **transparent published-method baselines, not SOTA**, and are not offered as the mitigation engine. TRL ~3. |
| **EXCLUSIONS (partner-owned / out of scope — DO NOT BID)** | The detection/classification **algorithms themselves** as a delivered product (Ashforde evaluates them; the AI partner builds them); the **on-board / edge inference engine** and its implementation; secure in-orbit update of algorithms; raw-IQ / signal-capture front-end and any real field-capture corpus; steerable phased-array antennas; SDR/FPGA/payload hardware; the mitigation actuation path (payload reconfiguration). |
| **Effort / price** | `[TBD — prime + founder; ECOS/PBF via prime; fee ≤ 8% TCC (KAF)]` |

---

## WP-A / WP-B boundary (one line for the WBS)

**WP-A predicts *system performance* of the candidate architecture** (the NAV-DT achievable-performance
component). **WP-B evaluates *AI detection-classification algorithms*** against a labelled impairment
corpus (the ML-characterisation layer). They are adjacent, share the open core's reproducibility
(`scenario_hash`) and the same verification-matrix honesty invariant (`kshana/src/verification.rs`), but
answer different SoW questions and can be **costed and scheduled independently** in the WBS.

---

## Consortium-composition value (for the prime's PBF / scoring sheet)

- **SME** seat → lifts the ≥3-SME and ≥30%-of-budget-to-non-LSI consortium-composition criteria.
- **DE-nationality** in ESA-STAR (entity 83208) → top geo-return.
- **Single, narrow, fixed-scope** WP with clean interfaces → low integration/costing risk, no scope creep into partner-owned hardware.

> **⚠ Founder-/consortium-gated, do not pre-fill:** the German registered office (EE seat ≠ a 13494
> participating state — the #1 eligibility risk, needs a German branch/subsidiary + an ESA Q&A
> clarification by ~29 Jun 2026); ESA-STAR full registration (Light suffices at submission, full before
> award); the consortium seat + the partner-owned mandated Key Person; the founder CV / man-hours
> (supporting role, **not** Key-Personnel); the price; the real SoW FPD-10030 SRD cross-check; and the
> **BIPR table** (Kshana Apache-2.0 is background IP — OSS use needs **ESA prior authorisation** under
> GCC Part II Option A; flag to the prime). **Prime/partner outreach is PARKED** until explicit founder go.

---

*PRIVATE — Ashforde OÜ internal working document. Contact: contact@ashforde.org. Every capability cited
traces to built, tested modules in the open Kshana core (v0.17.x) and the `kshana-pro` overlay, labelled
validated (external oracle) or modelled (physics model, self-/cross-checked). This is the PBF-7 paste-in
form of the WP definitions detailed in [`technical-contribution.md`](technical-contribution.md).*
