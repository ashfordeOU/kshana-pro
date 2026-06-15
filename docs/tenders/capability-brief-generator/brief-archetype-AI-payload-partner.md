<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Pre-rendered ARCHETYPE brief (dominance-plan step A13) — AI-payload-partner class (AO/1-13494).
ARCHETYPE ONLY: no real partner named. OUTREACH PARKED until the founder explicitly says go.
Rendered from TEMPLATE.md with the {{PRIME_NAME}} placeholder bound to the ARCHETYPE label, not a candidate.
Contact: contact@ashforde.org
-->

# Kshana — capability brief for [AI / digital-payload partner — ARCHETYPE]
### AI & Digital Payload Technologies — PNT Demonstrator (FutureNAV C3, Phases 0/A/B1 · AO/1-13494/26/NL/RR)

**PRIVATE — Ashforde OÜ internal working document.** ARCHETYPE brief on the shelf. Not a sent document; no partner selected.
**Date: 2026-06-15 · Contact: contact@ashforde.org**

> ARCHETYPE NOTE: this brief is pre-rendered against the *class* of AI / digital-payload partner — the operator
> that owns the on-board AI / edge-inference engine, reconfigurable payload (SDR/FPGA), and the RF/antenna chain.
> No candidate is named. Outreach is PARKED; producing this asset does not authorise contacting anyone.

---

## 1. One honest identity

Kshana is **the AI-algorithm *evaluation* testbed and RF-performance layer of the NAV-DT — not the on-board AI engine, and not your payload.** Ashforde OÜ (sole founder, Chakshu Baweja) offers one tightly-scoped SME seat — **the RF-performance component of the NAV-DT + the detector-agnostic AI-algorithm evaluation testbed + the machine-checked verification matrix** — that sits *beside* your payload and proves your detectors actually generalise. We build neither your inference engine nor your hardware.

We bid strictly as a **consortium-SME work-package contributor — never prime, never a mandated Key Person.** Kshana is an open Apache-2.0 Rust core (913+ library tests, fmt + clippy clean, adversarially reviewed) you and the prime can audit line-by-line. **TRL ~3** software-model substrate. **No flight heritage, no agency contracts, no certification, no ESA endorsement** — stated plainly.

---

## 2. The work-package we own (and only this)

You own the on-board AI / edge inference, the reconfigurable payload, and the RF/antenna chain. Ashforde owns the **complementary** WP: the harness that owns the SoW's *"Characterisation & Evaluation of Machine-Learning Techniques"* line — a detector-agnostic ROC/AUC/confusion/Pfa-Pmd evaluation testbed with a leakage guard, stratified split, and a distribution-shift optimism report — paired with a labelled synthetic RF-impairment corpus and the RF-performance / FoM component of the NAV-DT. We evaluate your detectors; we never replace them.

- **WP scope (in):** detector-agnostic AI-algorithm evaluation testbed (ROC/AUC, confusion, Pfa/Pmd, leakage guard, stratified split, distribution-shift optimism report); labelled synthetic RF-impairment scenario generator (parameter-grounded — TEXBAT params, **not raw IQ**); PNT FoM engine (ranging/timing accuracy, availability, integrity, GNSS-denied resilience) = the NAV-DT achievable-performance component; constellation / orbit / mission-geometry feasibility; the machine-checked ECSS verification cross-reference.
- **WP scope (out, partner/prime-owned):** on-board AI / edge inference engine; secure in-orbit update / reconfiguration / fault-containment; steerable phased-array antennas, radiation-pattern / antenna IF hardware; payload hardware (SDR, FPGA, reconfigurable processing chains); receiver acquisition / tracking / correlator, raw-IQ processing; DestinE-style digital twins, PNT data lakehouse / Navigation Data Space, core ground infrastructure; system engineering / ECSS ownership / submission (prime's); spacecraft bus; flight product assurance.
- **Deliverables it feeds:** D1.1 benefits, D1.3 mission-concept trade, D1.4 MRD sizing; D2.2 PA/QA + D2.3 SEP + ECSS-E-ST-10-02 verification cross-reference; D2.6 feasibility, D2.16 NAV-DT achievable-performance; and the Task-3 NAV-DT algorithm-verification / performance-benchmark deliverables.
- **Clean interfaces:** consumes **your detector outputs / scores** + scenario definitions; emits reproducible, scenario-hashed ROC/AUC evaluation reports (with the optimism report), FoM tables, integrity diagrams, and the verification matrix. One narrow boundary; no overlap with your payload or inference WP.

---

## 3. Score-delta sheet — what an Ashforde seat does to your consortium's scored proposal

Mapped to the FutureNAV evaluation weights **20 / 30 / 30 / 10 / 10**.

| Weight | Criterion | What an Ashforde WP adds | Direction |
|---|---|---|---|
| 20% | Background & experience | An openly auditable PNT-performance + ML-evaluation codebase as concrete evidence (not a heritage claim); a DE-SME presence for the consortium-composition view. | ↑ |
| 30% | Understanding of requirements & objectives | Instruments the SoW's harder-to-fake lines — *"Characterisation & Evaluation of ML Techniques"*, NAV-DT achievable-performance + algorithm verification — proving depth on the AI-evaluation and performance-prediction objectives, not just asserting it. | ↑ |
| 30% | Quality & suitability of proposed programme | A verification matrix that refuses "validated" without an external oracle, plus a leakage guard, stratified split, and a distribution-shift optimism report — direct, auditable PA/QA, SEP, and ECSS-E-ST-10-02 evidence that your detectors are evaluated honestly. | ↑ |
| 10% | Management, costing, planning | A single, narrow, fixed-scope, cap-compliant SME WP with a clean interface to your detector outputs — low integration risk, nothing to re-cost, no creep into your payload. | ↑ |
| 10% | Compliance with administrative tender conditions | DE nationality + SME status mechanically lift consortium-composition scoring (≥3 SMEs, ≥30% non-LSI budget). | ↑ |

**Consortium-composition arithmetic (scored, not a KAF).** A **German-nationality SME** (ESA-STAR entity held in private notes) → one of the required **≥3 SMEs** and budget toward **≥30%-to-non-LSI**; feeds the **DE-weighted geographical return** (DE is the top return on the FutureNAV demonstrators); a **cap-compliant** WP that does not have to be re-costed.

**Open registered-office item (disclosed, not hidden).** Ashforde's registered office is currently in Estonia (not a participating state). Light ESA-STAR registration suffices at submission; a participating-state registered office is required **by award**. Fix in flight: a German branch/subsidiary plus an ESA Q&A clarification.

---

## 4. The audit-the-core hook (why us over the black-box incumbents)

In an AI-payload demonstrator, the credibility risk is the classic one: a detector scores beautifully in the lab and falls over in the field. The panel needs proof the evaluation itself isn't optimistic. Ashforde's testbed is built to expose exactly that — a **leakage guard, a stratified split, and an explicit distribution-shift (in- vs out-of-regime) optimism report** — and the whole engine is **open Apache-2.0, readable line-by-line, reproducible from a committed scenario + fixed seed + pinned version**.

The structural differentiator: a **unit-tested verification matrix that refuses to mark any capability "validated" without an independent external oracle** (`src/verification.rs`), on an open core — versus the closed simulators in this space (GMV magicGNSS / SVS, Qascom SimSAFE / QA707, M3 Systems StellaNGC, IFEN NavX-NCS, Spirent, Safran-Skydel, ANavS). For a study whose credibility rests on proving ML detectors *generalise*, an auditable evaluation testbed that reports its own optimism is a structural asset you can show ESA — and it keeps your detector IP yours, cleanly separated from our scoring.

---

## 5. Validated-or-modelled capability table

Every claim is tagged **validated** (independent external oracle) or **modelled** (published / first-principles physics with tests, no external oracle yet). The labelling is enforced in code.

| # | Capability | Kshana module | Status | Oracle |
|---|---|---|---|---|
| 1 | Detector-agnostic AI-algorithm evaluation testbed (ROC/AUC/confusion/Pfa-Pmd, leakage guard, stratified split, distribution-shift optimism) | `impairment_eval` | **modelled** | closed-form AUC bounds (Mann–Whitney) + perfect-oracle detector; corpus SYNTHETIC, not IQ |
| 2 | Labelled synthetic RF-impairment scenario generator (TEXBAT params, not raw IQ) | `impairment_eval` | **modelled** | parameter-grounded; bundled detectors are published-method baselines, **not SOTA** |
| 3 | Machine-checked verification matrix (validated only with an external oracle) | `verification` | **validated** (classification invariants) | unit-tested status/oracle-kind invariants |
| 4 | PNT Figure-of-Merit engine (accuracy / availability / integrity / GNSS-denied resilience) = NAV-DT achievable-performance component | FoM / performance core | **modelled** | first-principles; internal cross-checks |
| 5 | GNSS-denied resilience (anti-jam C/N₀, spoof suite, alt-PNT) | `jamming`, `spoof*`, `altpnt` | **modelled** | Kaplan & Hegarty; TEXBAT *parameters* (characterisation) |
| 6 | Integrity — snapshot/MHSS RAIM, ARAIM P_HMI budget | `raim`, `sbas`, `lunar` | **validated** | DO-229E/DO-316 + real IGS SP3 / Celestrak TLE |
| 7 | SGP4/SDP4 orbit propagation (mission-geometry feasibility) | `sgp4`, `propagator`, `walker` | **validated** | AIAA 2006-6753 — 666 vectors, 4.12 mm |
| 8 | Reference frames & timescales (IAU 2006/2000A, CIO GCRS↔ITRS) | `frames`, `precession`, `nutation`, `cio`, `timescales` | **validated** | SOFA/ERFA vectors + Vallado (0.1–4 mm) |
| 9 | Frequency stability (ADEV/MDEV/TDEV/HDEV + noise-type CIs) | `allan` | **validated** | NIST SP 1065 / Stable32 on NBS14 (1e-4) |
| 10 | Nav-signal modulation & code-tracking analysis (BPSK-R/BOC PSD, DLL jitter, multipath) | `navsignal` | **modelled** | Betz / Kaplan & Hegarty closed forms |
| — | On-board AI / edge inference · RF-payload/antenna HW · SDR/FPGA · spacecraft bus · flight PA | — | **partner-owned** | no module, no claim, by design |

> Note: row 1's *harness logic* is validated against closed-form AUC bounds, but the corpus is synthetic
> (parameter-grounded, not raw IQ) and the bundled detectors are transparent baselines, not SOTA — a high AUC
> here demonstrates the **pipeline**, not field performance. The on-board AI / edge-inference engine and all
> payload/RF **hardware** are explicitly your disciplines; Kshana models signal performance, not hardware.

---

## 6. BIPR / IP line (clean, declarable)

- **Background IPR:** Kshana — open **Apache-2.0** PNT-performance-and-resilience engine (v0.17.0), owned by Ashforde OÜ. Used as the analysis tool; the **results** are the deliverables under the contract's Foreground-IPR terms.
- **No copyleft anywhere in the dependency tree** — permissive allow-list (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0), CI-enforced by `cargo-deny` (`deny.toml`).
- **Apache-2.0 prior-authorisation note:** open-source use as a tool may require **ESA prior authorisation** under GCC Part II (Option A); declarable via the prime's BIPR/BIPR-table route. Ashforde supplies the open-source-use declaration (component + repo + release tag + permissive SBOM + no-GPL confirmation).
- **Foreground / Pro overlay:** a customer/device-specific digital twin would be the first **Kshana Pro** Foreground-IP instance, under the agreed NDA/IPR and any export-control terms — distinct from the Apache-2.0 background engine, and from your detector / payload IP, which stays yours.

---

## 7. What this offer is — and is not

**Is:** a defined, auditable, fixed-scope SME work-package — the RF-performance component of the NAV-DT + the AI-algorithm evaluation testbed + the machine-checked verification matrix — ready to fold into your 13494 consortium proposal as the honest evaluation layer for your detectors.

**Is not:** a prime, a Key Person, the whole NAV-DT, the on-board AI engine, or any partner-owned hardware/ground/platform element. Ashforde does not satisfy the consortium or Key-Personnel mandates alone and does not claim to.

*Prime/partner outreach is **PARKED** pending explicit founder go. This archetype brief defines the asset only; producing it does not authorise contacting anyone.*

---
*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator · capability claims labelled validated (external oracle) or modelled throughout · TRL ~3, no flight heritage, no certification, no ESA endorsement.*
