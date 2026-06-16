<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
Element-level ECSS-E-ST-10 systems-engineering + verification plan for the NAV-DT
performance / RF-evaluation work package of AO/1-13494 (FutureNAV C3 — AI & Digital
Payload Technologies PNT Demonstrator). Keyed to SoW FPD-10030. For a prime's SE lead.
Contact: contact@ashforde.org
-->

# System Engineering & Verification Plan — Kshana NAV-DT performance / RF-evaluation work package (AO/1-13494)

**Tender:** ESA AO/1-13494/26/NL/RR — *AI & Digital Payload Technologies PNT Demonstrator* (FutureNAV C3), Phases 0/A/B1, study toward a LEO IOD.
**Author / owner:** Ashforde OÜ (Chakshu Baweja) — PNT performance & resilience SME (WP-A) + AI-algorithm-evaluation SME (WP-B).
**Subject system element:** Kshana PNT-performance-and-resilience simulator (open Apache-2.0 core, v0.17.x) — the **NAV-DT achievable-performance component + the AI-algorithm evaluation testbed**, *one component of a partner-led NAV-DT, not the whole twin*.
**Reference standards:** ECSS-E-ST-10C (System Engineering general), ECSS-E-ST-10-02 (Verification), ECSS-M-ST-10C (Project planning & lifecycle) — *applied in style, not as contractual deliverables Ashforde has produced before; see §7 residuals.* Tailored ECSS set per FPD-10028 (-10 / -10-02 / -10-03 / -40).
**Contact:** contact@ashforde.org

> **STATUS: DRAFT / TEMPLATE.** This plan is structured against ECSS-E-ST-10 / -10-02 and the
> ECSS-M-ST-10C lifecycle, but is populated with **placeholder** requirement identifiers. The
> authoritative requirement set is the System Requirements Document derived from SoW
> **FPD-10030** (and shared PARD FPD-10031), which Ashforde has **not yet obtained**. Every
> requirement ID below is a placeholder (prefixed `NAV-PERF-*`) and the Requirements
> Traceability Matrix (RTM) is a **skeleton** to be re-keyed and re-verified once the real SoW
> and consortium teaming are in hand. Founder-/consortium-gated items are marked. Sister tender
> to **13503** — bid SEPARATELY; the shared ECSS docs are byte-identical but the proposals are
> distinct. Cross-references: WP descriptors [`WP-definition-navdt-aieval.md`](WP-definition-navdt-aieval.md),
> sizing/benefits note [`D1.1-D1.3-constellation-sizing-benefits.md`](D1.1-D1.3-constellation-sizing-benefits.md),
> technical contribution [`technical-contribution.md`](technical-contribution.md).

---

## 1. Purpose & scope

### 1.1 Purpose

This document defines how Ashforde OÜ will **verify the PNT-performance and AI-evaluation
contribution** it supplies to the AO/1-13494 demonstrator study, and how that verification
evidence is folded into the project-level verification programme owned by the consortium prime.

It is **not** a system-level SE/Verification Plan for the demonstrator. It is the *element-level*
SE + verification approach for two adjacent work packages — **WP-A** (the RF/performance
figure-of-merit component of the NAV-DT) and **WP-B** (the AI-algorithm evaluation testbed) —
expressed in ECSS-E-ST-10 / -10-02 terms so it can be ingested cleanly by the prime's System
Engineering function.

### 1.2 Scope and honest division of responsibility

Ashforde's role in this consortium is the **PNT-performance-simulation and AI-evaluation SME** —
a **consortium-SME work-package contributor, never prime, never a mandated Key Person.** It is
**not** the SE authority for the system and does **not** supply flight hardware.

Stated plainly so the boundary is unambiguous:

- **The named System Engineering lead is the prime's**, not Ashforde's. The system SRD, the
  Design, Development, Verification & Validation Plan (DDVP), the system Verification Control
  Document (VCD), and the master RTM are **prime-owned** deliverables. Ashforde *contributes
  verification inputs into* them; it does not author or sign them.
- **Ashforde supplies the NAV-DT-performance + AI-evaluation verification inputs**: the
  requirement→model→test→oracle→status evidence for the PNT-performance analyses (WP-A) and the
  documented evaluation protocol + operating-characteristic evidence for AI detection-classification
  algorithms (WP-B), plus reproducible scenario bundles. The prime's SE lead assembles these into
  the system verification record.
- **Partner-owned engineering is explicitly out of Ashforde's verification scope** (§2.4):
  spacecraft-bus engineering (AOCS/thermal/structures/propulsion/power), reconfigurable **payload
  / SDR / FPGA hardware** and steerable **phased-array antenna** design, the **on-board / edge AI
  inference engine** and secure in-orbit reconfiguration, the **full** NAV-DT and the high-fidelity
  payload / RF-environment (DestinE) digital twins, Core Ground Infrastructure / PNT Data Lakehouse,
  and flight-hardware **product assurance** (radiation/EEE/MAIT). Kshana is a navigation-**performance**
  and **algorithm-evaluation** simulator, not a hardware, bus, ground-segment or on-board-AI house.

### 1.3 Applicable & reference documents

| Ref | Document | Note |
|-----|----------|------|
| AD-1 | ECSS-E-ST-10C / ECSS-E-ST-10-02 | System Engineering + Verification — applied as the structuring standards |
| AD-2 | ECSS-M-ST-10C | Project planning & lifecycle — the Phase 0→B1 reviews map (§5) |
| AD-3 | SoW **FPD-10030** iss 1.0 + PARD **FPD-10031** + tailored ECSS **FPD-10028** | **NOT YET OBTAINED in full** — authoritative requirement source; this plan is a template until in hand |
| RD-1 | Kshana `docs/CAPABILITY.md` (v0.17.x) | Honest capability/scope map (validated vs modelled vs out-of-scope) |
| RD-2 | Kshana `docs/VALIDATION.md` | External-oracle validation evidence |
| RD-3 | Kshana `src/verification.rs` | Machine-checked requirement→module→test→oracle→status matrix (§3) — the honesty spine |
| RD-4 | `kshana-pro/src/mbse` (mod.rs) | ECSS/MBSE artifact generator: margins (PASS/FAIL), traceability CSV, ReqIF-lite (§4) |
| RD-5 | Kshana `docs/REPRODUCIBILITY.md` + `kshana-pro` `REPRODUCIBILITY.md` | Determinism, pinned toolchain, SBOM, build provenance, scenario hashes |
| RD-6 | `WP-definition-navdt-aieval.md` (this folder) | PBF-7 WP descriptors (WP-A NAV-DT performance, WP-B AI-eval testbed) |
| RD-7 | `D1.1-D1.3-constellation-sizing-benefits.md` (this folder) | Constellation sizing + reconfigurable-PNT benefit method/numbers |

---

## 2. Verification approach (ECSS-E-ST-10-02)

Verification follows the four ECSS methods. For each, this section states **which Kshana
evidence supports it** and at what honesty level (validated vs modelled). Ashforde's contribution
concentrates in *Analysis* and *Test*, with supporting *Review-of-Design* and *Inspection*;
flight-representative test/qualification belongs to the hardware partners.

### 2.1 Review-of-Design (RoD)

- Kshana's design rationale is documented and reviewable: the capability map (RD-1) states, per
  capability, what exists / is missing / is roadmap; the validation ledger (RD-2) records each
  external oracle; an overclaim-closure ledger records every historical overclaim, its resolution,
  and the CI guard that keeps it resolved.
- The **NAV-DT performance-component method** (sizing → FoM → integrity → resilience → benefit
  delta, RD-7) and the **AI-evaluation protocol** (stratified split, leakage guard, fixed
  operating point, distribution-shift optimism report) are reviewable design intent the prime's SE
  lead and ESA can assess *before* any number is produced.
- *Honesty note:* RoD here covers **simulation/method design**. Flight-design reviews (payload
  PDR/CDR-level hardware design) are partner-owned.

### 2.2 Analysis (A) — Ashforde's primary method

- **Validated analyses** (checked against an independent external oracle): orbit propagation
  (SGP4/SDP4 vs the 666 AIAA 2006-6753 vectors, 4.12 mm); reference frames/timescales (IAU
  SOFA/ERFA + Vallado, mm-level); integrity (snapshot/MHSS RAIM + ARAIM vs DO-229E/DO-316 with
  real IGS SP3 and real Celestrak TLE); single-point positioning (real IGS station ABMF, 5.7 m
  3-D RMS); ranging-code cross-correlation (vs published GPS C/A Gold −23.9 dB).
- **Modelled analyses** (first-principles / published formulae, internally or cross-checked):
  constellation sizing (streets-of-coverage + Walker design sweep/optimiser); the positioning/timing
  FoM; GNSS-denied jamming resilience (`availability_under_jamming`); the reconfigurable-PNT benefit
  delta; the labelled RF-impairment corpus (parameter-grounded, TEXBAT-style, **not raw IQ**).
- *Honesty note:* "validated" is reserved strictly for external-oracle checks; everything else is
  "modelled" and labelled as such in the RTM (§3). No analysis claims flight heritage.

### 2.3 Inspection (I)

- Software-assurance inspection is concrete and reproducible: ~915 library `#[test]` functions
  (fmt + clippy clean); a CycloneDX SBOM; golden / determinism / cross-platform CI gates; a pinned
  `rust-toolchain` with committed `Cargo.lock`; build provenance; scenario hashes (RD-5).
- The **verification matrix itself is inspectable and machine-checked** (§3): its honesty invariants
  are enforced by unit tests, so the validated/modelled boundary cannot silently drift.
- The **MBSE artifacts** (margins table, traceability CSV, ReqIF-lite, RD-4) are generated, not
  hand-maintained, and the margin/PASS-FAIL logic is unit-tested.
- *Honesty note:* Inspection here is **software** inspection; physical inspection of flight hardware
  (workmanship, EEE) is a partner PA discipline, out of scope.

### 2.4 Test (T)

- At Ashforde's level, "Test" means the automated suite exercising each model against its oracle,
  plus reproducible scenario runs that produce per-scenario FoM tables with hashes and seeds (a
  reviewer can re-run any result). For **WP-B**, "Test" additionally means running candidate
  detection-classification algorithms through the evaluation testbed under representative RF
  scenarios and emitting ROC/AUC/confusion/Pfa-Pmd with the leakage guard and distribution-shift
  report.
- **Out of Ashforde's scope — partner-owned test/qualification:** environmental test, radiation
  test, RF/antenna measurement, payload-HW / SDR characterisation, on-board-AI inference testing,
  and assembly / integration / test (MAIT). Kshana produces *performance predictions and the test
  cases that bound them*; it does not run flight hardware. No flight TRL is claimed (TRL ≈ 3).

### 2.5 Method-to-evidence summary

| ECSS method | Ashforde evidence | Honesty level |
|-------------|-------------------|---------------|
| Review-of-Design | Capability map, validation/overclaim ledgers, sizing+benefit method, AI-eval protocol | Method/design only |
| Analysis | Kshana validated + modelled analyses (the bulk of the contribution) | Mixed: validated where an external oracle exists, else modelled |
| Inspection | ~915 tests, SBOM, golden/determinism/cross-platform gates, build provenance, machine-checked matrix + MBSE artifacts | Software assurance only |
| Test | Automated model-vs-oracle suite + reproducible scenario FoM bundles + AI-detector evaluation runs | Simulation test only; flight test is partner-owned |

---

## 3. The executable verification backbone

The spine of this plan is **executable**, not prose. The open engine contains
`kshana/src/verification.rs`, which builds a machine-checked matrix mapping each PNT-performance /
AI-evaluation capability to:

> **requirement → implementing module → test evidence → validation oracle → OracleKind → status**

and renders it to Markdown or CSV (`to_markdown`, `to_csv`, `summarize`) for direct paste into a
project Verification Control Document.

### 3.1 What "machine-checked" honestly means

The risk in any verification matrix is **dressing a self-consistency check up as an external
validation**. `verification.rs` defends with *unit-tested honesty invariants* over `OracleKind`
∈ {`ExternalDataset`, `ReferenceImpl`, `InternalConsistency`, `NoneKind`} and `VerificationStatus`
∈ {`Validated`, `Modelled`, `PartnerOwned`}:

- A row is **`Validated` only if it carries an `ExternalDataset` oracle** (independent authoritative
  dataset / published vectors / published value). A row whose only evidence is an internal algebraic
  identity or a sibling re-implementation **cannot** be Validated.
- A **`Modelled`** row names a module + tests and a real (non-`NoneKind`) oracle — honestly one of a
  loose external bracket, a same-codebase `ReferenceImpl` cross-check, or an `InternalConsistency`
  closed-form identity.
- A **`PartnerOwned`** row claims **nothing** — no module, no test, no oracle — by design; only
  partner rows may carry `NoneKind`.

The invariants are *classification* checks (a Validated row must be tagged external; partner rows
claim nothing; counts consistent). They do **not** prove every named test/oracle string resolves to
live code — those are curated, code-reviewed references, exactly like the validation-ledger citations.
This scope is stated in the module's own doc comment so the artifact does not oversell itself.

### 3.2 Representative RTM (drawn from `verification_matrix()`, re-keyed to 13494)

A representative extract of the live matrix — the externally-validated core, several modelled rows
(including the 13494-specific sizing / AI-evaluation rows), and the partner-owned gaps. The matrix
renders machine-generated Markdown/CSV; this is a curated subset for the SE lead.

| Req-ID (placeholder) | Requirement | Target (illustrative) | Achieved (illustrative) | Module | Test evidence | Oracle | OracleKind | V-method (T/A/I/R) | Status |
|---|---|---|---|---|---|---|---|---|---|
| NAV-PERF-010 | Orbit propagation accuracy (geometry spine) | ≤ 1 cm | 4.12 mm | `sgp4`, `propagator`, `precise_od` | 666 AIAA 2006-6753 vectors | AIAA 2006-6753 SGP4 vectors | ExternalDataset | A + I | **VALIDATED** |
| NAV-PERF-020 | Integrity protection levels (RAIM/ARAIM) | HPL/VPL ≤ alert limit at P_HMI | finite HPL/VPL on real geometry | `raim`, `sbas`, `lunar` | `tests/igs_real_data.rs`, `tests/araim_dual_real_data.rs` | DO-229E/DO-316; real IGS SP3 + Celestrak TLE | ExternalDataset | A + I | **VALIDATED** |
| NAV-PERF-030 | Single-point positioning accuracy | metre-level | 5.7 m 3-D / 1.1 m H (ABMF) | `pvt` | `tests/pvt_abmf.rs` | Real IGS station ABMF surveyed coordinate | ExternalDataset | A + I | **VALIDATED** |
| NAV-PERF-040 | Reference frames & timescales | mm-level | 0.1–4 mm | `frames`, `nutation`, `cio`, `timescales` | `tests/frame_reference_vectors.rs` | IAU SOFA/ERFA; Vallado | ExternalDataset | A + I | **VALIDATED** |
| NAV-PERF-100 | Constellation sizing for coverage KPI | ≥ target coverage fraction | grid-optimal design | `walker` | `streets_of_coverage_*`, `optimize_walker_design` tests | Rider/Beste closed forms; brute-force winner cross-check | ReferenceImpl | A | **MODELLED** |
| NAV-PERF-110 | Achievable PNT FoM (accuracy/availability) | per SoW KPI | per scenario | `fom` | `fom::tests` (`positioning_performance`, `score`) | Published FoM definitions; self-consistency | InternalConsistency | A | **MODELLED** |
| NAV-PERF-120 | GNSS-denied jamming resilience | ≥ target % availability | per scenario | `jamming` | `jamming::tests` (`availability_under_jamming`) | Kaplan & Hegarty §9.4 anti-jam C/N₀; PSD-derived Q | InternalConsistency | A | **MODELLED** |
| NAV-PERF-130 | Reconfigurable-PNT benefit delta | Δ FoM > 0 in service | modelled before/after delta | `walker`, `jamming`, `raim`, `fom` | sizing+benefit runs (RD-7), scenario-hashed | Same-engine before/after; RAIM core validated | ReferenceImpl | A | **MODELLED** |
| NAV-PERF-200 | Labelled RF-impairment corpus | reproducible, stratified | seeded corpus | `impairment_eval` | `impairment_eval::tests` (`generate_corpus`, `stratified_split`) | TEXBAT scenario *parameters* (characterisation, not IQ) | ExternalDataset | A + T | **MODELLED** |
| NAV-PERF-210 | AI-detector evaluation testbed (ROC/AUC/Pfa-Pmd, leakage guard) | metrics + leakage-free + optimism report | testbed verdicts | `impairment_eval` | `roc_curve`, `auc`, `confusion_at`, `has_leakage`, `distribution_shift_report` | Closed-form ROC/AUC identities; leakage invariant test | InternalConsistency | A + I + T | **MODELLED** |
| NAV-PERF-300 | Verification cross-reference (honesty spine) | validated-only-with-external-oracle invariant holds | invariant enforced | `verification` | `validated_rows_require_an_external_oracle`, `partner_rows_claim_nothing` | Test-enforced classification invariant | InternalConsistency | I | **VALIDATED** (invariant) |
| NAV-PERF-310 | ECSS/MBSE assurance artifacts (margins/RTM/ReqIF) | PASS on all reqs; valid ReqIF | generated artifacts | `kshana-pro/src/mbse` | `mbse::tests` (`margin_of`, `to_csv`, `to_reqif`, `validate`) | Margin/PASS-FAIL closed form; ReqIF well-formedness test | InternalConsistency | I | **MODELLED** |
| NAV-PERF-900 | Spacecraft-bus engineering (AOCS/thermal/structures/propulsion/power) | — | — | — | — | — | NoneKind | (partner) | **PARTNER** |
| NAV-PERF-910 | Reconfigurable payload / SDR / FPGA + phased-array antenna HW | — | — | — | — | — | NoneKind | (partner) | **PARTNER** |
| NAV-PERF-920 | On-board / edge AI inference engine + secure in-orbit reconfiguration | — | — | — | — | — | NoneKind | (partner) | **PARTNER** |
| NAV-PERF-930 | Full NAV-DT + payload / RF-environment (DestinE) twins + Core Ground Infra | — | — | — | — | — | NoneKind | (partner) | **PARTNER** |
| NAV-PERF-940 | Flight-hardware product assurance (radiation/EEE/MAIT) | — | — | — | — | — | NoneKind | (partner) | **PARTNER** |

> *Illustrative targets/achieved are placeholders or capability-map figures, **not** SoW
> requirements;* they show the RTM shape. The "AI-detector evaluation testbed" row is **modelled**,
> not Validated, despite its leakage-guard/ROC machinery being unit-tested: an AUC here is over
> **model-derived labels** from a synthetic corpus, not a field-detection figure, so the status
> reflects that honestly. The NAV-PERF-200 corpus row is **modelled** even though its oracle is
> external (TEXBAT *parameters*), because it is characterised against parameters, not raw IQ.

### 3.3 How the prime's SE lead consumes this

The CSV/Markdown render of `verification_matrix()` drops into the prime's VCD as the
navigation-performance + AI-evaluation section. The prime re-keys the placeholder `NAV-PERF-*` IDs
to the real SRD requirement IDs (§6), assigns the project verification method/event per requirement,
and the matrix's own CI invariants keep Ashforde's status labels honest over the study lifetime.

---

## 4. MBSE / requirements-management artifacts (auto-generated)

Ashforde supplies the SE artifacts a study needs in DOORS/Polarion-ingestible form, generated (not
hand-maintained) by `kshana-pro/src/mbse` (RD-4):

| Artifact | Function | Status |
|---|---|---|
| **Margins table** (target / achieved / margin %, PASS/FAIL) | `margins(&[Requirement])`, `margin_of`, `all_compliant` — lower-is-better and higher-is-better both handled; a non-finite or zero target yields FAIL, not a silent pass | unit-tested |
| **Traceability matrix (CSV)** — DOORS/Excel-ingestible | `to_csv(&[Requirement])` → `id, requirement, verification, target, achieved, margin_pct, status` | unit-tested |
| **ReqIF-lite (XML)** — DOORS / Polarion / Capella importable | `to_reqif(title, &[Requirement])` → one SPEC-OBJECT per requirement; well-formedness + escaping unit-tested | unit-tested |
| **Validation gate** | `validate(&[Requirement])` rejects a malformed requirement set | unit-tested |

These feed the RTM (Appendix A of the SRD skeleton, §6) and the VCD margins evidence (§3). They are
**software assurance** artifacts — the honesty status is "the artifact generator is unit-tested and
deterministic", not "the requirements are met"; the latter is filled when the real SRD targets are in.

---

## 5. ECSS-M-ST-10C lifecycle / review map

The SoW runs Phase 0/A in Phase 1 (months 0–12) and Phase B1 in Phase 2 (months 13–30), with a
GATEKEEPER go/no-go between. Mapped to the ECSS-M-ST-10C project phases and reviews, with Ashforde's
WP-A/WP-B contribution per gate (Ashforde *feeds* each review; the prime *holds* it):

| ECSS-M-ST-10C phase | Review gate | SoW phase | Ashforde (WP-A / WP-B) contribution into the gate |
|---|---|---|---|
| **Phase 0** — Mission analysis / needs identification | **MDR** (Mission Definition Review, ~KO+4) | Phase 0 | Constellation/orbit sizing options (RD-7) + achievable-performance baseline + the verification *method* (RoD) for the performance component |
| **Phase A** — Feasibility | **PRR** (Preliminary Requirements Review, ~KO+11) | Phase A | Performance/integrity/resilience feasibility FoM tables (D2.6); AI-evaluation protocol definition; the `NAV-PERF-*` requirement subsections + RTM skeleton for the performance/AI-eval reqs |
| — | **GATEKEEPER** (go/no-go, ~KO+12) | Phase 1→2 | Feasibility evidence + distribution-shift optimism report (the honesty check) so the go/no-go rests on defensible numbers |
| **Phase B1** — Preliminary definition | **ISRR** (~mo17) then **FR** (Final Review, ~mo30) | Phase B1 | Built NAV-DT performance component + AI-evaluation testbed delivered; scored detector evidence; the machine-checked verification cross-reference + MBSE artifacts populated against the real SRD |

> *Honest note:* MDR/PRR/ISRR/FR are **prime-held** system reviews; the dates above are the SoW's
> indicative schedule (KO ~Q4 2026). Ashforde supplies the performance/AI-evaluation *inputs* to each;
> it does not hold any of these reviews and does not own the system-level review data packs.

---

## 6. SRD (System Requirement Document) skeleton

To be **populated once the real SoW (FPD-10030) SRD is obtained.** The structure follows a
conventional ECSS requirement-document layout; Ashforde authors only the performance / AI-evaluation
subsections (marked *[Ashforde input]*); the rest is prime-owned.

```
1.  Introduction (purpose/scope, document tree vs SoW FPD-10030, definitions, applicable docs)
2.  System overview & mission context
    2.1  Demonstrator objectives (AI & digital reconfigurable PNT payload, LEO IOD)
    2.2  OpsConcept & PNT use-cases (nominal, GNSS-denied, contested, reconfiguration)
    2.3  External interfaces & environment (incl. the NAV-DT / "SpaceStream" interface)
3.  Functional requirements
    3.1  PNT functional requirements
    3.2  Reconfigurable-PNT / benefit requirements                 [Ashforde input — D1.1/D1.3]
    3.3  AI detection-classification evaluation requirements        [Ashforde input — WP-B]
4.  Performance requirements
    4.1  Positioning/velocity accuracy & timing stability            [Ashforde input — WP-A]
    4.2  Availability & integrity (HPL/VPL, P_HMI)                   [Ashforde input — WP-A]
    4.3  Resilience metrics (% availability under jamming, outage)   [Ashforde input — WP-A]
    4.4  AI-evaluation performance (ROC/AUC, Pfa/Pmd, optimism bound) [Ashforde input — WP-B]
5.  Interface requirements
    5.1  NAV-DT / SpaceStream aggregated-metrics interface            [Ashforde ingests/feeds]
    5.2  Payload performance-envelope interface                      (prime / payload partner)
6.  Environmental & operational requirements                         (prime / bus partner)
7.  Product-assurance & dependability requirements
    7.1  Software product assurance (ECSS-Q-ST-80, tailored)         [Ashforde input]
    7.2  Flight-hardware PA (radiation/EEE/MAIT)                     (PA partner)
8.  Verification requirements
    8.1  Verification approach & method allocation                   (prime SE; §2 feeds it)
    8.2  Performance + AI-evaluation verification matrix             [Ashforde input — §3 backbone]
Appendix A  Requirements Traceability Matrix (RTM)                   (prime master; Ashforde subset, §4 ReqIF/CSV)
Appendix B  Acronyms
```

---

## 7. Honest residuals

1. **No ECSS formal-deliverable track record yet.** Ashforde applies ECSS-E-ST-10 / -10-02 and the
   ECSS-M-ST-10C lifecycle *in style* and keeps the discipline (validated/modelled invariants,
   traceability, reproducible evidence, generated MBSE artifacts), but has **not previously produced a
   contractual ECSS verification deliverable** on an ESA programme. The prime's SE function carries the
   formal authority and format ownership; Ashforde supplies disciplined inputs.
2. **The RTM is a template until the SoW is in hand.** All requirement IDs are placeholders
   (`NAV-PERF-*`); illustrative targets/achieved are capability-map figures, not SoW requirements. The
   authoritative requirement set is the SRD from FPD-10030 (+ PARD FPD-10031), **not yet obtained**; the
   §3 matrix, while machine-checked for *internal honesty invariants*, must be re-keyed and each verdict
   re-checked before submission.
3. **The named SE lead is the prime's.** Ashforde does not provide the system SE authority, SRD, DDVP,
   or master VCD/RTM, and holds no system review (MDR/PRR/ISRR/FR). It owns only the performance /
   AI-evaluation verification *inputs*.
4. **Partner-owned engineering is excluded by design** (recorded as `PARTNER` rows, not silently
   omitted): spacecraft-bus engineering; reconfigurable payload / SDR / FPGA + phased-array antenna
   hardware; the on-board / edge AI inference engine + secure in-orbit reconfiguration; the full NAV-DT
   and the payload / RF-environment (DestinE) twins + Core Ground Infrastructure; and flight-hardware PA.
5. **TRL & heritage.** Kshana is a simulation tool at TRL ≈ 3. No flight heritage, no ESA endorsement,
   no certification (e.g. DO-229/ARAIM certification), no hardware capability. "Validated" is used
   strictly for external-oracle checks; everything else is "modelled". The AI-evaluation corpus is
   **synthetic / parameter-grounded (not IQ)** and bundled detectors are **published-method baselines,
   not SOTA**.
6. **Open-source authorisation.** Kshana is Apache-2.0 background IP; under GCC Part II Option A, OSS use
   **needs ESA prior authorisation** — flag for the prime; the BIPR table is mandatory.
7. **Consortium-/founder-gated items** (the seat itself, the prime, the real SoW SRD, Key-Personnel CVs,
   costing, and the German registered office) are founder-gated and not pre-drafted here. This plan
   becomes contractually meaningful only inside a secured consortium seat.

---

*This is a DRAFT/TEMPLATE element-level SE & verification plan, owned by Ashforde OÜ as the
NAV-DT-performance + AI-evaluation SME. It feeds — and does not replace — the prime's system-level
verification programme. All quantitative claims trace to Kshana v0.17.x `docs/CAPABILITY.md` and
`docs/VALIDATION.md` under the validated-or-modelled discipline. Contact: **contact@ashforde.org**.*
