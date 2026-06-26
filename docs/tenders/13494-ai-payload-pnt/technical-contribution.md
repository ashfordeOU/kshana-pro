<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
Technical-proposal contribution + SoW compliance matrix (template) + WBS work-package
definitions, for AO/1-13494/26/NL/RR (FutureNAV C3 — AI & Digital Payload Technologies
PNT Demonstrator, Phases 0/A/B1; SoW FPD-10030 iss 1.0, 20/05/2026).
SISTER tender to 13503 — bid SEPARATELY; combined proposals forbidden.
Contact: contact@ashforde.org
-->

# Technical contribution & SoW compliance matrix — Ashforde / Kshana for AO/1-13494/26/NL/RR

**Programme:** ESA FutureNAV, Component 3 — AI & Digital Payload Technologies PNT Demonstrator (Phases 0/A/B1).
**SoW reference:** FPD-10030 iss 1.0 (20/05/2026); esa-star Activity 1000046439.
**Deadline:** 9 July 2026 13:00 Amsterdam (esa-star only; no late submissions). Q&A by ~29 June 2026.

**Ashforde's role (fixed, do not exceed):** a single **consortium-SME work-package seat** — never prime, never a mandated Key Person. Ashforde contributes two adjacent work packages inside a prime-led consortium:

- **WP-A — PNT performance & resilience prediction component of the NAV-DT** (the RF/performance figure-of-merit layer of the Navigation Digital Twin; *not* the whole twin).
- **WP-B — AI-algorithm evaluation testbed + labelled RF-impairment scenario generator** (the SoW's "Characterisation & Evaluation of Machine-Learning Techniques" layer, plus its labelled corpus).

> **Labelling discipline (the credibility asset — keep it in the proposal).** Every capability claim below is tagged **validated** (checked against an external oracle — an independent authoritative dataset / published verification vectors / a published numeric value) or **modelled** (published or first-principles physics, tested, self- or cross-checked). This is the same `validated`/`modelled` invariant the code enforces in `kshana/src/verification.rs`. Do not let a prime's editor flatten it.

> **What Kshana is, honestly.** Kshana is an open **AGPL-3.0 Rust PNT performance-and-resilience simulator** — **dual-licensed** (GNU AGPL-3.0-only for the open track, or a commercial licence from Ashforde OÜ), and public and auditable line-by-line either way — TRL ~3, no flight heritage, no agency contracts, no certification. It is **one component** of the NAV-DT — the achievable-performance / RF-impairment-evaluation engine — and a prime can audit it line-by-line. It is **not** the spacecraft bus, the payload HW, the ground infrastructure, or the full digital twin.

---

## 1. The moat — lead with this

A **unit-tested verification matrix that refuses to label a result "validated" without an external oracle** (`kshana/src/verification.rs`), riding on an **open, auditable AGPL-3.0 core a prime can read line-by-line** (dual-licensed; a commercial licence is available). Against the black-box GNSS simulators in this market — GMV (magicGNSS / Service Volume Simulator), Qascom (SimSAFE / QA707), M3 Systems (StellaNGC), IFEN (NavX-NCS), Spirent, Safran/Skydel, ANavS — the differentiator is *auditability of the performance claim*, not raw signal-generation horsepower (which the primes already own). For a 30-month study that must defend an achievable-performance number to a GATEKEEPER review and an ISRR, an auditable performance oracle is the asset.

Mechanical scored-criteria lift for the prime's consortium: Ashforde is **DE-nationality** in ESA-STAR (top geo-return) and **SME** — both directly improve a prime's scored consortium composition (≥3 SMEs, ≥30% of budget to non-LSI operators).

---

## 2. Work-package definitions (paste-into-WBS, PBF-7 style)

Two adjacent leaf WPs. Both are grounded in built, tested modules in the open Kshana core (`kshana` crate, v0.17.x; 915 `#[test]` functions across the library, `fmt`+`clippy` clean) and the Pro overlay (`kshana-pro` crate, the trade-study / MBSE-assurance framework). Phasing aligns to the SoW: contribution is light in Phase 0/A (analysis & requirements support) and deepens in Phase B1 (the testbed and the performance-component build).

### WP-A — PNT performance & resilience prediction component of the NAV-DT

| Field | Content |
|---|---|
| **WP title** | PNT performance & resilience prediction component of the NAV-DT |
| **WP manager** | Ashforde OÜ (SME) |
| **Phasing** | Phase 0/A: analysis input. Phase B1: build & deliver the component + its verification cross-reference. |
| **Objectives** | Provide the **RF/performance figure-of-merit engine** that supplies the NAV-DT's *achievable-performance* assessment: predict ranging/timing accuracy, availability, integrity (RAIM/ARAIM protection levels), and GNSS-denied resilience for the candidate IOD constellation/orbit and RF environment — as an auditable, reproducible, externally-cross-checked component the prime integrates behind the NAV-DT interface. |
| **Inputs** | (from prime / consortium) IOD mission concept & constellation/orbit options (LEO); candidate payload performance envelope (ranging error budget, antenna/link assumptions) from the payload WP; RF-environment definition (iono/tropo/multipath/interference scenarios) from the environment-model WP; NAV-DT interface contract (the "SpaceStream" aggregated-metrics format) from the ground-segment WP; target PNT KPIs from the user-segment WP. |
| **Tasks** | A1. Map the candidate constellation/orbit geometry and visibility (LEO IOD), feeding sizing. A2. Compute the positioning/timing figure-of-merit set per scenario — position/velocity error, timing stability, availability — via the FoM engine (`kshana/src/fom.rs`: `positioning_performance`, `score`). A3. Compute snapshot/sequential **integrity** protection levels (HPL/VPL) via real least-squares RAIM (`kshana/src/raim.rs`: chi-squared fault detection + slope-based HPL/VPL, after Parkinson & Axelrad / Brown). A4. Quantify **GNSS-denied resilience** (% availability under jamming, position-error growth under outage) using the open jamming/resilience models. A5. Sweep & Monte-Carlo the scenario space to map *where* the candidate architecture meets KPIs and where it does not, each result carrying a reproducible `scenario_hash`. A6. Wrap A2–A5 as a NAV-DT-consumable component against the prime's interface; emit the **verification cross-reference** (`kshana/src/verification.rs`) for the system-engineering verification control document. |
| **Outputs** | O-A1: achievable-performance dataset (FoM tables: accuracy/availability/integrity/resilience per scenario, each with uncertainty and a validated/modelled tag) → feeds **D2.16** and **D2.6**. O-A2: integrity-budget contribution (HPL/VPL, RAIM/ARAIM) → **D2.16/D2.6**. O-A3: resilience benefits evidence (GNSS-denied performance) → **D1.1**. O-A4: constellation/orbit sizing inputs → **D1.3/D1.4**. O-A5: machine-checked verification cross-reference for the performance-component requirements → **D2.2/D2.3**. O-A6: the integrated NAV-DT performance component (Phase B1), reproducible (scenario-hashed). |
| **EXCLUSIONS (explicit — partner-owned / out of scope; DO NOT BID)** | The **full** NAV-DT (Kshana is only the RF/performance component); the high-fidelity *payload* digital twin (payload-HW partner); DestinE-style Digital Twins of iono/tropo/multipath (environment / Earth-observation partner — Ashforde *consumes* their environment definition, does not build it); Core Ground Infrastructure, PNT Data Lakehouse / Navigation Data Space (ground-segment partner); on-board / edge AI inference engine; secure in-orbit update/reconfiguration; steerable phased-array antennas & radiation patterns; SDR/FPGA/payload hardware; receiver acquisition/tracking/correlator and raw-IQ processing; LCA / debris / eco-design. |

### WP-B — AI-algorithm evaluation testbed + labelled RF-impairment scenario generator

| Field | Content |
|---|---|
| **WP title** | AI-algorithm evaluation testbed + labelled RF-impairment scenario generator |
| **WP manager** | Ashforde OÜ (SME) |
| **Phasing** | Phase A: method definition & evaluation-protocol design (feeds feasibility). Phase B1: deliver the testbed + corpus generator, run the consortium's anomaly/RF-impairment detection-classification algorithms through it under representative RF conditions. |
| **Objectives** | Own the SoW's **"Characterisation & Evaluation of Machine-Learning Techniques"**: provide (1) a **labelled, parameter-grounded synthetic RF-impairment corpus generator** and (2) a **detector-agnostic evaluation testbed** that scores *any* candidate anomaly/RF-impairment detection-classification algorithm with the operating-characteristic metrics a reviewer expects — ROC/AUC, confusion matrix, Pfa/Pmd at a chosen operating point, per-impairment-class breakdown — with a leakage guard, stratified split, and an explicit distribution-shift optimism report. The testbed is the **referee**, not a classifier; bundled detectors are transparent published-method baselines, not SOTA. |
| **Inputs** | (from prime / consortium) the consortium's candidate detection/classification algorithms (behind the testbed's detector interface); the RF-impairment taxonomy and severity ranges to be evaluated (jamming, spoofing, multipath, RF anomalies) from the payload/AI WPs; representative RF scenario definitions / operating points from the environment & system WPs; on-board/edge implementation constraints (latency, memory, observable availability) from the on-board-AI partner, used to scope which observables a detector may use. |
| **Tasks** | B1. Generate a labelled, parameter-grounded corpus over the impairment taxonomy (`kshana/src/impairment_eval.rs`: `generate_corpus`, `CorpusConfig`, `ImpairmentClass` — composing the jamming / signal-quality-AGC / detection models; **TEXBAT-style parameters, never raw IQ or field captures**). B2. Implement the evaluation protocol: stratified train/test split with disjoint partition keys (`stratified_split`), exact-duplication **leakage guard** (`Split::has_leakage`). B3. Score candidate detectors through the `ImpairmentDetector` trait: ROC (`roc_curve`), AUC (`auc`), confusion at operating point (`confusion_at`, `Confusion::pd`/`pmd`), per-class breakdown (`evaluate`). B4. Produce the **distribution-shift optimism report** (`distribution_shift_report`) — quantify how much in-tuning-regime scores overstate out-of-regime performance, the honesty check an ESA reviewer will look for. B5. Define & document the evaluation operating protocol (fixed seed, pinned engine version, declared operating point) so results are reproducible and comparable across algorithms. B6. Run the consortium algorithms through B1–B4 under representative RF scenarios; deliver the scored evidence. |
| **Outputs** | O-B1: the labelled RF-impairment corpus generator (configurable, seeded, reproducible) → ML-characterisation input for **D2.6** and Phase-B1 algorithm-evaluation evidence. O-B2: the detector-agnostic evaluation testbed (ROC/AUC/confusion/Pfa-Pmd, leakage guard, stratified split) → Phase-B1 "verification of AI algorithms" evidence. O-B3: the distribution-shift optimism report per algorithm → feasibility/honesty evidence for **D2.6** and the GATEKEEPER. O-B4: documented evaluation protocol → **D2.3** (SE plan) verification approach for AI elements. O-B5: scored operating-characteristic evidence for the consortium's detection-classification algorithms (Phase B1). |
| **EXCLUSIONS (explicit — partner-owned / out of scope; DO NOT BID)** | The detection/classification **algorithms themselves** as a delivered product (Ashforde evaluates them; the AI-algorithm partner builds them — bundled detectors are baselines, not SOTA, and are not offered as the mitigation engine); the **on-board / edge inference engine** and its implementation; secure in-orbit update of algorithms; raw-IQ / signal-capture front-end and any real field-capture corpus (the corpus is synthetic and parameter-grounded, and is reported as exactly that — an AUC here is over model-derived labels, not a field-detection performance figure); steerable phased-array antennas; SDR/FPGA/payload hardware; the mitigation actuation path (payload reconfiguration). |

> **WP-A / WP-B boundary.** WP-A predicts *system performance* of the candidate architecture (the NAV-DT achievable-performance component). WP-B evaluates *AI detection-classification algorithms* against a labelled impairment corpus (the ML-characterisation layer). They are adjacent and share the open core's reproducibility (`scenario_hash`) and the same verification-matrix honesty invariant, but they answer different SoW questions and can be costed/scheduled independently.

---

## 3. Phase 0/A/B1 deliverables Ashforde contributes to

Ashforde is a **contributor** to the deliverables below — the prime owns and integrates each one. Kshana is the RF/performance component of the NAV-DT, **not the whole twin**.

| Deliverable (SoW) | Ashforde contribution | WP | Validated / Modelled |
|---|---|---|---|
| **D1.1** System Scenario + Benefits Analysis + Tech SoTA | GNSS-denied resilience benefits evidence; achievable-performance baseline for the benefit case (accuracy/availability/integrity under nominal & degraded RF). | WP-A | modelled (physics models; resilience cross-checked where an oracle exists) |
| **D1.3** Mission Concept Trade-Off & Justification | Constellation/orbit/mission-geometry feasibility & sizing for the LEO IOD, run through the Pro trade-study framework (ranked, scenario-hashed, audit-grade). | WP-A | modelled |
| **D1.4** Mission Requirement Document (MRD) | Performance-side sizing numbers (achievable accuracy/availability/integrity envelopes) to anchor mission requirements. | WP-A | modelled |
| **D2.2** PA/QA Plan | The machine-checked **verification matrix** contribution (requirement → module → test → oracle → status), enforcing the validated-only-with-external-oracle invariant — the PA/QA cross-reference spine. | WP-A | validated (invariant is test-enforced) |
| **D2.3** System Engineering Plan | The ECSS-E-ST-10-02 verification cross-reference method for the performance component **and** the documented AI-algorithm evaluation protocol (how detection-classification algorithms are verified). | WP-A + WP-B | validated (verification method) / modelled (eval protocol) |
| **D2.6** Feasibility Assessment | Achievable-performance feasibility (FoM tables with uncertainty + integrity protection levels); ML-technique characterisation feasibility (corpus + testbed + distribution-shift optimism report). | WP-A + WP-B | modelled |
| **D2.16** NAV-DT Architecture + Digital-Twin Integration Feasibility | The **achievable-performance assessment + algorithm-verification** sub-component of the NAV-DT, and the feasibility of integrating it behind the NAV-DT/"SpaceStream" interface (consuming aggregated metrics; feeding predicted performance/resilience). **Scope: the RF/performance component only — not the full twin, not the payload twin, not the iono/tropo/multipath DestinE twins.** | WP-A (+ WP-B verification) | modelled (component); integration feasibility = analysis |

Secondary / on-request support (cost only if the prime requests it): **D2.5** concept/architecture trade (performance-side inputs to the system trade), **D2.12** risk register (performance/AI-evaluation risk items), **D2.15** critical-technology list (AI-evaluation method maturity). Not claimed as primary deliverables.

---

## 4. SoW compliance matrix — TEMPLATE (pending real FPD-10030 SRD requirement IDs)

> **TEMPLATE — fill the `Req ID` column from the issued FPD-10030 SRD once the real requirement identifiers are available.** Rows below are organised by the four contribution themes named in the SoW scope (AI evaluation; RF-environment scenarios; digital-twin performance; verification). `Status` ∈ {Compliant · Partial · N/A}. `Evidence module` cites the built, tested code. The matrix is itself an instance of the `kshana/src/verification.rs` discipline — claims tagged validated/modelled, partner-owned rows claim nothing.

### Theme 1 — AI-algorithm characterisation & evaluation ("Characterisation & Evaluation of Machine-Learning Techniques")

| Req ID (TBD) | Requirement (paraphrased pending SRD) | Ashforde contribution | Status | Evidence module |
|---|---|---|---|---|
| `[SRD-AI-xx]` | Provide a method to characterise/evaluate ML techniques for RF-impairment detection-classification | Detector-agnostic evaluation testbed scoring any algorithm via the `ImpairmentDetector` trait | Compliant | `kshana/src/impairment_eval.rs` (`evaluate`) |
| `[SRD-AI-xx]` | Report standard operating-characteristic metrics (ROC/AUC, confusion, Pfa/Pmd) | ROC curve, AUC, confusion at operating point, Pd/Pmd, per-class breakdown | Compliant | `kshana/src/impairment_eval.rs` (`roc_curve`, `auc`, `confusion_at`, `Confusion`) |
| `[SRD-AI-xx]` | Guard against evaluation-methodology error (train/test leakage, optimistic scoring) | Stratified split with disjoint keys + exact-duplication leakage guard + distribution-shift optimism report | Compliant | `kshana/src/impairment_eval.rs` (`stratified_split`, `Split::has_leakage`, `distribution_shift_report`) |
| `[SRD-AI-xx]` | Detection-classification under on-board/edge implementation constraints | Evaluate detectors restricted to constrained observable sets / operating points (constraints supplied by on-board-AI partner) | Partial | `kshana/src/impairment_eval.rs` — *constraint envelope is a partner input; on-board inference engine is out of scope* |
| `[SRD-AI-xx]` | The detection/classification/mitigation algorithm itself | — | N/A | *Partner-owned (AI-algorithm WP). Bundled detectors are transparent published-method baselines, not SOTA, and not offered as the product.* |

### Theme 2 — RF-environment & impairment scenarios

| Req ID (TBD) | Requirement (paraphrased pending SRD) | Ashforde contribution | Status | Evidence module |
|---|---|---|---|---|
| `[SRD-RF-xx]` | Provide a labelled corpus of RF-impairment scenarios for algorithm evaluation | Parameter-grounded synthetic corpus generator over jamming/spoofing/multipath/anomaly classes, seeded & reproducible | Compliant | `kshana/src/impairment_eval.rs` (`generate_corpus`, `CorpusConfig`, `ImpairmentClass`) |
| `[SRD-RF-xx]` | Scenarios representative of operational RF conditions | Compose jamming, signal-quality/AGC and detection models into measurement-domain observables (TEXBAT-style **parameters**) | Partial | `kshana/src/impairment_eval.rs` (composes `jamming`, `spoof_monitors`, `detection`) — *parameter-grounded, not raw-IQ / not field captures; reported as exactly that* |
| `[SRD-RF-xx]` | High-fidelity RF-environment twin (iono/tropo/multipath, DestinE integration) | — | N/A | *Partner-owned (environment / Earth-observation WP). Ashforde consumes the environment definition, does not build it.* |
| `[SRD-RF-xx]` | Raw-IQ generation / signal-capture front-end | — | N/A | *Partner-owned (payload/SDR WP). Out of scope.* |

### Theme 3 — Digital-twin performance (NAV-DT achievable-performance component)

| Req ID (TBD) | Requirement (paraphrased pending SRD) | Ashforde contribution | Status | Evidence module |
|---|---|---|---|---|
| `[SRD-DT-xx]` | NAV-DT shall predict achievable PNT performance (accuracy/availability) | Positioning/timing figure-of-merit engine producing accuracy/availability per scenario, with uncertainty | Compliant | `kshana/src/fom.rs` (`positioning_performance`, `score`, `FoMScores`) |
| `[SRD-DT-xx]` | NAV-DT shall predict integrity performance (protection levels) | Real least-squares snapshot/sequential RAIM: chi-squared fault detection + slope-based HPL/VPL | Compliant | `kshana/src/raim.rs` (`snapshot_raim`) |
| `[SRD-DT-xx]` | NAV-DT shall predict resilience under degraded/denied RF | GNSS-denied resilience FoM (% availability under jamming, error growth under outage) | Compliant | `kshana/src/fom.rs` + open jamming/resilience models |
| `[SRD-DT-xx]` | NAV-DT performance assessment shall be reproducible / auditable | Scenario-hashed reproducible outputs + audit-grade evidence pack (ranked trade study) | Compliant | `kshana-pro/src/suite/mod.rs` (`run_trade_study`, `evidence_pack_markdown`) |
| `[SRD-DT-xx]` | NAV-DT shall integrate with NAVEX via aggregated-metrics ("SpaceStream") interface | Performance component consumes aggregated metrics & feeds predicted performance behind the prime's interface | Partial | *Interface contract is a prime input; Ashforde delivers the component, not the SpaceStream link or NAVEX side* |
| `[SRD-DT-xx]` | High-fidelity payload digital twin (HW behaviour) | — | N/A | *Partner-owned (payload-HW WP). Kshana is the RF/performance component, not the payload twin.* |
| `[SRD-DT-xx]` | Core Ground Infrastructure / Data Lakehouse / Navigation Data Space | — | N/A | *Partner-owned (ground-segment WP).* |

### Theme 4 — Verification & PA/QA

| Req ID (TBD) | Requirement (paraphrased pending SRD) | Ashforde contribution | Status | Evidence module |
|---|---|---|---|---|
| `[SRD-VV-xx]` | Provide a verification cross-reference (requirement → implementation → test → status) per ECSS-E-ST-10-02 | Machine-checked verification matrix with per-row status, module, test, oracle | Compliant | `kshana/src/verification.rs` (`verification_matrix`, `summarize`, `to_markdown`, `to_csv`) |
| `[SRD-VV-xx]` | The validated/modelled boundary shall be enforced, not asserted | Test-enforced invariant: a `Validated` row **must** carry an `ExternalDataset` oracle; a self-consistency check cannot be Validated | Compliant (validated) | `kshana/src/verification.rs` (`OracleKind`, `VerificationStatus`) |
| `[SRD-VV-xx]` | Generate ECSS/MBSE assurance artifacts (margins table, traceability, ReqIF for DOORS/Polarion) | Requirements → margins (PASS/FAIL) + traceability CSV + ReqIF-lite export | Compliant | `kshana-pro/src/packs/mbse_pack.rs` (via `kshana-pro/src/mbse`) |
| `[SRD-VV-xx]` | Software PA per ECSS-Q-ST-80 (tailored Cat B/C/D, PARD FPD-10031) | Open, auditable AGPL-3.0 core (dual-licensed; commercial licence available) a prime can audit line-by-line; `fmt`+`clippy` clean; 915 library `#[test]` functions | Partial | *Open repo + CI; full flight-PA process is the prime's PA WP. **If brought under the open AGPL-3.0 track, OSS prior-authorisation (GCC Part II Option A) applies and AGPL copyleft attaches; Kshana is therefore brought under the commercial licence as proprietary Background IPR, which avoids both — flag for the prime; BIPR table mandatory. (Counsel to confirm the treatment.)*** |
| `[SRD-VV-xx]` | Flight software PA / qualification, spacecraft-bus & payload PA | — | N/A | *Partner-owned (prime PA + payload WP). No module, no test, no oracle, by design.* |

---

## 5. Open items to resolve before submission (founder-gated)

1. **Eligibility (the #1 risk — state openly, do not hide).** Ashforde OÜ is **DE-nationality** in ESA-STAR (eligible, top geo-return) **but its registered office is in Estonia**, which is **not** a 13494 Participating State (BE CA CH CZ DE ES GB IT LV NL NO PL SE). Award requires a participating-state registered office. **Fix:** German branch/subsidiary **+** an ESA Q&A clarification submitted by ~29 June 2026. Light esa-star registration is required at submission (held).
2. **Open-source authorisation.** The OSS prior-auth route + copyleft only bite on the **open (AGPL) track**: if brought under the open AGPL-3.0 track, OSS prior-authorisation (GCC Part II Option A) applies and AGPL copyleft attaches. Kshana is therefore brought under the **commercial licence** as ordinary proprietary Background IPR, which avoids both *(counsel to confirm the treatment)*. Flag to the prime; the **BIPR (background-IP) table is mandatory** and must list Kshana (dual-licensed AGPL-3.0-only + commercial), with the third-party dependency tree noted as permissive-only, no GPL.
3. **SRD requirement IDs.** The §4 matrix is a **template**; fill `Req ID` from the issued FPD-10030 SRD.
4. **Prime/partner outreach is PARKED.** This document **prepares the asset and defines the seat**; it is not an instruction to contact anyone. No outreach until explicit founder go.

---

*PRIVATE — Ashforde OÜ internal working document. Contact: contact@ashforde.org. All quantitative claims trace to Kshana v0.17.x and are labelled validated (external oracle) or modelled (physics model, self-/cross-checked).*
