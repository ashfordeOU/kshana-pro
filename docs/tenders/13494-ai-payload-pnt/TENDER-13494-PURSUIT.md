<!--
PRIVATE — Ashforde OÜ internal. Lives in the private kshana-pro repo only.
Never publish in the open kshana repo. Master tracker for the ESA AO/1-13494
(AI & Digital Payload Technologies — PNT Demonstrator, Phases 0/A/B1) pursuit.
Contact: contact@ashforde.org
-->

# TENDER-13494-PURSUIT — AI & Digital Payload Technologies PNT Demonstrator

**ESA reference:** AO/1-13494/26/NL/RR · FutureNAV, Component 3 · Phases 0 / A / B1
SoW ref FPD-10030 iss 1.0 (20/05/2026) · esa-star Activity 1000046439
**Type:** Study contract (FFP), €3.5M/contract cap (Phase 1 [0+A] ≤ €1.0M; Phase 2 [B1] ≤ €2.5M), 30 months, dual-source (up to 2 parallel awards)
**Deadline:** **9 Jul 2026, 13:00 Amsterdam** (esa-star only, no late) · **Q&A by ~29 Jun 2026**
**Decision (founder):** pursue **as a consortium SME** (RF-performance / NAV-DT-component / ML-evaluation WP) — **not** as prime.
**The one hard gate:** **N7 — secure a consortium seat.** No prime → no bid this cycle.

> **SISTER tender to 13503 (Quantum-Enabled PNT).** Bid **SEPARATELY** — combined proposals are forbidden. The 005/006/007 docs (shared PARD FPD-10031 + tailored ECSS) are byte-identical to the 13503 package; reuse the cross-checked compliance reading, but file a distinct proposal.
> Companion artifacts to be built in this folder (see N9/N12/N3/N13 etc. below): `gap-audit.html`, `kshana-capability-brief.md`, `technical-contribution.md` (SoW cross-check), `ESA-QA-draft.md`, `BIPR-declaration.md`, `consortium-target-map.md` (template, search gated), `software-PA-evidence.md`, `SE-verification-plan.md`.

---

## Timeline reality (anchored to today, 2026-06-15)

| Milestone | Date | Days left |
|-----------|------|-----------|
| **Today** | 2026-06-15 | — |
| Q&A questions close | **2026-06-29** | 14 |
| **Submission deadline** | **2026-07-09 13:00 Amsterdam** | **24** |

⚠ The real window is **24 days**, and the consortium seat (N7) is the binding constraint, not the writing. The seat must close by **~29 Jun** (same horizon as the Q&A deadline that resolves the eligibility question) or there is **no honest bid this cycle** — there is no solo fallback here (consortium ≥5 operators + a mandated Key Person). Phase-tech split for reference: Phase 1 = months 0–12 (MDR ~KO+4, PRR ~KO+11) then **GATEKEEPER** go/no-go; Phase 2 = months 13–30 (ISRR ~mo17, Final Review ~mo30). KO ~Q4 2026.

---

## Status legend

✅ done · 🔧 wip · ▢ to do · 🔒 gated (blocked until an upstream item clears) · ⛔ parked (founder hold)

---

## Phase 0 · Go/No-Go & eligibility — by 2026-06-18

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| N1 | Confirm decision: pursue **as consortium SME** (RF-performance / NAV-DT-component / ML-eval WP) — **not** prime, **not** a mandated Key Person | F | now | ✅ done — recorded in this tracker |
| N2 | 🔑 **Eligibility fix (#1 founder-gated risk):** Ashforde OÜ is **EE-seated** → award needs a **registered office in a Participating State**. Decide **German branch / subsidiary** (DE-nationality already in ESA-STAR, top geo-return, but EE office ≠ participating-state office) | F + legal | 2026-06-18 | ▢ to do |
| N3 | Draft + submit **ESA Q&A**: confirm whether a German branch/subsidiary (vs current EE registered office) satisfies the registered-office rule for entity 83208 by award; clarify timing relative to submission (Light reg only required at submission) | F · draft to prepare | by 2026-06-29 | ▢ to do — draft the Q&A asset; founder submits |
| N4 | Confirm **ESA-STAR registration** status: **Light** required at **submission**; full reg (bank + financials) before **award** | F | 2026-06-18 | ▢ to do |
| N5 | Go/No-Go decision rule: **if no seat (N7) by ~29 Jun → do NOT bid** (no solo fallback). Resolve eligibility (N2/N3) **before** committing | F | 2026-06-29 | 🔒 gated (N7 / N3) |

## Phase 1 · Partnering — by 2026-06-29 (critical path) — **[PARKED until founder go]**

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| N6 | Build the **consortium target map**: nav-systems primes + on-board-AI/edge-compute groups + reconfigurable-digital-payload/SDR + phased-array antenna + bus/SE + digital-twin/ground-segment houses (need ≥5 operators incl. prime, ≥3 SMEs, ≥30% budget to non-LSI, ≥€500k via genuine open competition) | F | 2026-06-20 | ⛔ parked — **prime search gated until founder says go.** Prepare template + selection criteria only (no names) |
| N7 | 🔑 **Secure a consortium seat** — LoI / teaming agreement **(HARD GATE)** — pitch Kshana as the RF-performance / NAV-DT-component / ML-evaluation WP + an SME slot (lifts ≥3-SME quota + DE geo-return) | F + P | ~2026-06-29 | 🔒 gated (N6) |
| N8 | Confirm the **mandatory Key Person** (≥1 with extensive **space-segment 'spacecraft-as-a-service'** experience) is held **by a partner** — **never** Ashforde; confirm team also has **digital-twin + on-board-AI** depth | P + F | ~2026-06-29 | 🔒 gated (N7) — partner-owned by design |

## Phase 2 · Bid contribution — 2026-06-22 → 2026-07-07 (parallel once teamed)

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| N9 | **Kshana capability brief** (1–2 pp): the **RF-performance component of the NAV-DT** + the **ML-evaluation testbed** + live playground demo; every claim labelled *validated (external oracle)* or *modelled* | F · to draft | 2026-06-25 | ▢ to do |
| N10 | Define **Kshana's WP** (objectives/inputs/tasks/outputs/exclusions) for the WBS — scoped to NAV-DT achievable-performance + ML-technique characterisation/evaluation; **explicitly exclude** the partner-owned gaps (see below) | F + P | 2026-07-03 | 🔒 gated (N7) |
| N11 | Founder **CV** + man-hour allocation table (supporting-role format, **not** Key-Personnel) | F | 2026-07-03 | 🔒 gated (N7); founder-only inputs |
| N12 | **Technical-proposal contribution**: NAV-DT achievable-performance method (FoM engine), ML-technique characterisation/evaluation (ROC/AUC/Pfa-Pmd, leakage guard, distribution-shift report), verification-matrix contribution + SoW compliance-matrix rows (D1.1/D2.6/D2.16/Task-3) | F · to draft | 2026-07-07 | 🔒 gated (N15 SoW cross-check) — draft against the **real SoW (FPD-10030)** + prime |
| N13 | **BIPR declaration**: Kshana (Apache-2.0) as Background IPR; **GCC Part II Option A** IPR; ⚠ **open-source software may NOT be used without ESA PRIOR AUTHORISATION** — flag and request it; no GPL | F · to draft | 2026-07-07 | ▢ to do — BIPR table is **mandatory** |
| N14 | **Financial input**: Ashforde WP cost via **ECOS/PBF** (through prime); **fee ≤ 8% of Total Company Cost** (KAF); respect Phase-1 ≤ €1.0M / Phase-2 ≤ €2.5M caps | F + P | 2026-07-08 | 🔒 gated (N7); founder/prime inputs |
| N15 | 🔑 Obtain + **cross-check the real SoW (FPD-10030 iss 1.0)** for Phase 0/A/B1 task + deliverable detail (D1.x / D2.x / Task 3); confirm WP alignment and the "validated/modelled" labelling holds against actual wording | F · analyze | 2026-06-27 | ▢ to do — **founder to obtain the SoW**; the compliance matrix stays a template until then |
| N21 | **Software-PA evidence pack (ECSS-Q-ST-80, tailored Cat B/C/D via PARD FPD-10031, Class Gamma)** — map Kshana's SW-quality infra (911+ tests, fmt+clippy clean, repro/CI gates, verification matrix) to software-PA process areas (the one PA slice Kshana owns) | F · to draft | 2026-07-07 | ▢ to do — flight-hardware/Class-Gamma safety PA stays a partner's |
| N22 | **SE verification plan + RTM skeleton (ECSS-E-ST-10-02; tailored set FPD-10028: -10/-10-02/-10-03/-40)** — backed by the machine-checked `verification` matrix (validated only with an external oracle) | F · to draft | 2026-07-07 | ▢ to do — named SE lead stays the prime's |

## Phase 3 · Submission — 2026-07-07 → 2026-07-09

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| N16 | Final **compliance review** of Ashforde's contribution: **8 KAFs** (Art.12 quals/exclusions, free-competition cert, binding FFP price, **fee ≤8% TCC** via ECOS, budget caps — all pass/fail), **13 Declarations of Compliance**, **≤100-page** limit (price > €1M), CV folder, **every operator (incl. each subcontractor) has a Participating-State registered office by award** | P + F | 2026-07-07 | 🔒 gated (N7) |
| N17 | **Prime submits** via esa-star (**no late submissions**) | P | **2026-07-09 13:00 Amsterdam** | 🔒 gated (N7) |
| N18 | **Sister-tender hygiene:** keep 13494 and **13503** proposals **separate** (combined forbidden); confirm no shared-content rule is breached and the 005/006/007 reuse is admin-only | F + P | 2026-07-07 | ▢ to do |
| N19/20 | **Post-award (future):** award → NAV-DT performance-component engagement (Foreground IP → Pro); watch **GATEKEEPER** (KO+12) and Phase B2+ continuation | F + P | post-award | 🔒 gated |

---

## What is already built vs what is founder-gated

**Open-`kshana` capability already BUILT (Apache-2.0, auditable line-by-line):**
- **ML-algorithm evaluation testbed** (`src/impairment_eval.rs`) — detector-agnostic ROC/AUC, confusion, Pfa/Pmd, leakage guard, stratified split, distribution-shift optimism report. This **owns the SoW's "Characterisation & Evaluation of Machine-Learning Techniques."** Bundled detectors are **transparent published-method baselines, not SOTA**.
- **RF-impairment scenario corpus / generator** (`src/impairment_eval.rs`) — labelled **synthetic** impairment scenarios, **parameter-grounded (TEXBAT params, NOT IQ)**. Feeds the ML-eval testbed and the NAV-DT representative-scenario need.
- **Machine-checked verification matrix** (`src/verification.rs`) — refuses a *validated* label without an **external oracle**; the D2.2/D2.3 PA-QA + ECSS-E-ST-10-02 verification cross-ref contribution.
- **RF-performance / Figure-of-Merit engine** — predicts ranging/timing accuracy, availability, integrity (RAIM/ARAIM), GNSS-denied resilience; feeds **D1.1** benefits, **D2.6** feasibility, **D2.16** NAV-DT achievable-performance. *(This is the RF-performance component of the NAV-DT — one component, **never** the whole twin.)*
- **Constellation / orbit / mission-geometry feasibility** for the LEO IOD — supports **D1.3 / D1.4** sizing.
- Quality bar: **911+ lib tests, fmt + clippy clean, adversarially reviewed.**

> ⚠ Note: `quantum_trade` capability belongs to the **13503** (Quantum-Enabled PNT) pursuit, **not** this tender. Do not import it into the 13494 contribution.

**Genuine gaps — partner-owned / out of scope — DO NOT BID:** on-board AI/edge inference engine; secure in-orbit update/reconfiguration; steerable phased-array antennas / radiation patterns; payload HW / SDR / FPGA; DestinE / lakehouse / ground-station infrastructure; LCA / debris / eco-design; receiver acquisition/tracking/correlator / raw-IQ. **Kshana is ONE COMPONENT of the NAV-DT, never the whole twin.**

**Founder-gated (cannot be drafted from code/audit alone):**
- **N2 / N4** — German registered-office (branch/subsidiary) decision + ESA-STAR registration status (Light at submission, full before award). *#1 founder-gated risk.*
- **N6 / N7** — actual prime search & the consortium seat (**explicitly parked until founder says go**; N7 is the hard gate).
- **N8** — the named **Key Person** (space-segment 'spacecraft-as-a-service'): **must come from a partner**, never Ashforde.
- **N11 / N14** — founder CV / man-hours (supporting role) and costing (need the teamed prime + founder inputs; fee ≤8% TCC via ECOS).
- **N15** — the **real SoW FPD-10030** (founder to obtain from the esa-star tender package); the compliance matrix is a template pending it.

**Decision rule:** if no seat (N7) by ~29 Jun, **do not bid** — there is no honest solo path (consortium + mandated Key Person). Resolve eligibility (N2/N3) **before** committing. Keep the 13494 and 13503 proposals strictly separate.

---

_Last updated 2026-06-15. Sources: SoW FPD-10030 iss 1.0 (20/05/2026); esa-star Activity 1000046439; shared PARD FPD-10031 + tailored ECSS (FPD-10028) — 005/006/007 byte-identical to the 13503 package; Kshana open repo (`src/impairment_eval.rs`, `src/verification.rs`, FoM engine), 911+ lib tests; Ashforde ESA-STAR record (entity 83208, DE nationality, EE registered office)._
