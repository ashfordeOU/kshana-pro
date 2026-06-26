<!--
PRIVATE — Ashforde OÜ internal. Lives in the private kshana-pro repo only.
Never publish in the open kshana repo. Master tracker for the ESA AO/1-13503
(Quantum-Enabled PNT Demonstrator) pursuit. Contact: contact@ashforde.org
-->

# TENDER-13503-PURSUIT — Quantum-Enabled PNT Demonstrator

**ESA reference:** AO/1-13503/26/NL/RR · FutureNAV, Component 3 · Phases 0 / A / B1
**Type:** Study contract (FFP), €3.5M, 30 months, up to 2 parallel awards
**Deadline:** **17 Aug 2026, 13:00 Amsterdam** (esa-star only) — *extended from 9 Jul per Clarification 2, 19 Jun 2026.*
**Decision (founder):** pursue **as a consortium SME** (nav-performance / trade-study WP) — **not** as prime.
**The one hard gate:** **T7 — secure a consortium seat.** No prime → no bid this cycle.

> **📋 Clarifications 1–3 audited 2026-06-25 → [`CLARIFICATIONS-AUDIT.md`](CLARIFICATIONS-AUDIT.md). Net-positive on every count.** Key effects: (Clar 2) **deadline +39 days → 17 Aug** — the old 6-day-short compression is GONE, T7 has a realistic runway; (Clar 1/A1) a **single focused application area is compliant** — anchor on our strongest software-heavy area (secure time dissemination / quantum time transfer, or anomaly detection for quantum systems); (Clar 1/A3) **simulation environments are an explicitly compliant way to demonstrate feasibility/performance/operational benefits** with "representativeness justified + gaps-to-flight identified" = our validated/modelled discipline — **upgrades Kshana from heritage to a requirement-compliant demonstration vehicle**; (Clar 1/A2) open competition for *critical pre-developments* must complete pre-submission (≥€500k/contract) — we enter as a **named consortium WP (Route A)**, not a hardware pre-dev; (Clar 3) **FPD-10031 PA&S is Reference, no SoC required** — our CI/SBOM/SLSA/verification-matrix = the "internal PA process" expected. ⚠ 13494 may still be **9 Jul** — founder to confirm.

> **NEW capability evidence:** [`QUANTUM-DEMONSTRATOR-CAPABILITY.md`](QUANTUM-DEMONSTRATOR-CAPABILITY.md) — all three SoW §1.5.2 application areas now built + runnable + reproducible in open `kshana` (`quantum-time-transfer`, `quantum-gnss-free-nav`, `quantum-anomaly-detect` + a unified `--study` suite), each a MODELLED simulation-based demonstration with a machine-checked representativeness + gaps-to-flight record = the Clar 1/A3 compliance mechanism. Built 2026-06-25 per [[project_kshana_quantum_pnt_13503]].
>
> Companion artifacts in this folder: [`gap-audit.html`](gap-audit.html) (the full capability/eligibility audit), [`kshana-capability-brief.md`](kshana-capability-brief.md) (T9), [`technical-contribution.md`](technical-contribution.md) (T12 + SoW compliance matrix), [`ESA-QA-draft.md`](ESA-QA-draft.md) (T3), [`BIPR-declaration.md`](BIPR-declaration.md) (T13), [`consortium-target-map.md`](consortium-target-map.md) (T5 — template, search gated), [`osip-positioning-note.md`](osip-positioning-note.md) (T18), [`software-PA-evidence.md`](software-PA-evidence.md) (T21), [`SE-verification-plan.md`](SE-verification-plan.md) (T22), [`TRL-maturation-dossier.md`](TRL-maturation-dossier.md) (T23).

---

## Timeline reality (re-baselined 2026-06-25 to the extended deadline)

| Milestone | Date | Days left (from 2026-06-25) |
|-----------|------|-----------|
| **Today** | 2026-06-25 | — |
| **Submission deadline** | **2026-08-17 13:00 Amsterdam** | **~53** |

✅ The deadline extension (Clar 2) removes the previous ~6-day-short compression. There is now a
realistic ~7.5-week runway: the consortium seat (T7) should still close as early as possible (it remains
the hard gate), but the "seat by ~29 Jun or pivot to OSIP" cliff no longer applies. If a seat is secured,
the bid-contribution and compliance-review phases (T10–T16) fit comfortably before 17 Aug. The
decision rule still holds: **no seat → do not force a weak solo bid → pivot to OSIP / future phases.**

---

## Status legend

✅ done · 🔧 wip · ▢ to do · 🔒 gated (blocked until an upstream item clears) · ⛔ parked (founder hold)

---

## Phase 0 · Go/No-Go & eligibility — by 2026-06-18

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| T1 | Confirm decision: pursue **as consortium SME** (not prime) | F | now | ✅ done — recorded in this tracker |
| T2 | Eligibility fix: decide **German registered office / branch** (EE-seated OÜ is an award risk) | F + legal | 2026-06-18 | ▢ to do |
| T3 | Draft + submit **ESA Q&A**: does DE-nationality (entity 83208) satisfy "registered office in a Participating State"? | F · draft ready | by 2026-06-29 | 🔧 draft prepared → [`ESA-QA-draft.md`](ESA-QA-draft.md); founder to submit |
| T4 | Confirm **ESA-STAR full registration** status (bank + financials before award) | F | 2026-06-18 | ▢ to do |

## Phase 1 · Partnering — by 2026-06-29 (critical path)

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| T5 | Build the **consortium target map**: DE/CH nav-systems primes + quantum-hardware groups (clock/cold-atom) + RF-payload + bus/SE | F | 2026-06-20 | ⛔ parked — **prime search gated until founder says go.** Template + selection criteria ready → [`consortium-target-map.md`](consortium-target-map.md) |
| T6 | **Outreach** to primes/partners — pitch Kshana as the nav-performance WP + an SME slot (helps ≥3-SME quota + DE return) | F | 2026-06-22+ | ⛔ parked — depends on T5 (founder go) |
| T7 | 🔑 **Secure a consortium seat** — LoI / teaming agreement **(HARD GATE)** | F + P | ~2026-06-29 | 🔒 gated (T6) |
| T8 | Secure the **mandatory quantum-nav Key Person** (likely from the hardware partner) | P + F | ~2026-06-29 | 🔒 gated (T7) |

## Phase 2 · Bid contribution — 2026-06-22 → 2026-07-07 (parallel once teamed)

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| T9 | **Kshana capability brief** (1–2 pp) → Phase 0/A nav-analysis WP + live playground/MCP demo | F · drafted | 2026-06-25 | ✅ draft ready → [`kshana-capability-brief.md`](kshana-capability-brief.md) |
| T10 | Define **Kshana's WP** (PBF-7: objectives/inputs/tasks/outputs/exclusions) for the WBS | F + P | 2026-07-03 | 🔒 gated (T7) — WP outline seeded in [`technical-contribution.md`](technical-contribution.md) §4 |
| T11 | Founder **CV** + man-hour allocation table (Key-Personnel format) | F | 2026-07-03 | 🔒 gated (T7); founder-only inputs |
| T12 | **Technical-proposal contribution**: quantum-vs-classical trade method, resilience, mission analysis + SoW compliance-matrix rows | F · drafted | 2026-07-07 | 🔧 wip — draft ready → [`technical-contribution.md`](technical-contribution.md); finalise w/ real SoW (T15) + prime |
| T13 | **BIPR declaration**: Kshana (dual-licensed GNU AGPL-3.0-only + commercial) as Background IPR, brought under the **commercial licence** (no copyleft attaches; AGPL governs only the open track) *(commercial-licence grant terms to be confirmed by counsel)*; third-party dependency tree permissive-only, no GPL | F · drafted | 2026-07-07 | ✅ draft ready → [`BIPR-declaration.md`](BIPR-declaration.md) |
| T14 | **Financial input**: Ashforde WP cost via ECOS/PBF (through prime); SME 35% advance | F + P | 2026-07-08 | 🔒 gated (T7); founder/prime inputs |
| T15 | Obtain + cross-check the **real SoW (FPD-10029)** for Phase 0/A task detail; confirm WP alignment | F · analyze | 2026-06-27 | ▢ to do — **founder to obtain the SoW**; compliance matrix is a template pending it ([`technical-contribution.md`](technical-contribution.md) §5) |
| T21 | **Software-PA (ECSS-Q-ST-80) evidence pack** — maps Kshana's SW-quality infra (tests/SBOM/repro/CI gates/verification matrix) to the software-PA process areas (the one PA slice Kshana owns) | F · drafted | 2026-07-07 | ✅ draft ready → [`software-PA-evidence.md`](software-PA-evidence.md) — flight-hardware PA stays a partner's |
| T22 | **SE verification plan + RTM skeleton (ECSS-E-ST-10-02)** — backed by the machine-checked `verification` matrix in open kshana | F · drafted | 2026-07-07 | ✅ draft ready → [`SE-verification-plan.md`](SE-verification-plan.md) — named SE lead stays the prime's |
| T23 | **TRL3→7 maturation dossier** — Kshana's performance-evidence / trade-closure support to the hardware maturation case | F · drafted | 2026-07-07 | ✅ draft ready → [`TRL-maturation-dossier.md`](TRL-maturation-dossier.md) — TRL advancement stays the hardware's |

## Phase 3 · Submission — 2026-07-07 → 2026-07-09

| ID | Step | Owner | Due | Status |
|----|------|-------|-----|--------|
| T16 | Final **compliance review** of Ashforde's contribution (8 KAFs, ≤100-pp budget, CV folder) | P + F | 2026-07-07 | 🔒 gated (T7) |
| T17 | **Prime submits** via esa-star | P | **2026-07-09 13:00** | 🔒 gated (T7) |
| T18 | (Parallel, separate) **ESDI OSIP** co-sponsored-research application — positioning for later phases | F · drafted | OSIP cadence | 🔧 wip — note ready → [`osip-positioning-note.md`](osip-positioning-note.md); needs a Swiss research link |
| T19/20 | **Post-award (future):** award → kicks off the digital-twin engagement (Foreground IP → Pro L1); watch sister ITT 1-13494 + Phase B2+ | F + P | post-award | 🔒 gated |

---

## What is already built vs what is founder-gated

**Built and ready in this folder (draft-ready items, all done):**
- T3 ESA eligibility Q&A draft
- T9 Kshana capability brief (1–2 pp)
- T12 technical-proposal contribution + SoW compliance-matrix template
- T13 BIPR declaration
- T18 OSIP positioning note
- T5 consortium target-map **template + selection criteria** (no names — search gated)
- T21 software-PA (ECSS-Q-ST-80) evidence pack — *added 2026-06-15*
- T22 SE verification plan + RTM skeleton (ECSS-E-ST-10-02) — *added 2026-06-15*
- T23 TRL3→7 maturation dossier — *added 2026-06-15*

**Open-`kshana` capability built 2026-06-15** (strengthens the §C analysis layer and narrows the §D/§E residuals — see [`gap-audit.html`](gap-audit.html) build banner): GNSS-denied clock-holdover calculator (`src/holdover.rs`), composed quantum-inertial dead-reckoning budget (`QuantumNavBudget`), ranging-code design-trade (`navsignal::CodeFamily`, GPS C/A −23.9 dB anchor), and a machine-checked verification matrix (`src/verification.rs`). Each TDD'd against closed-form/published anchors, then hardened by an adversarial review that caught and fixed real overclaims (notably: the clock-holdover headline is floor-governed for very stable clocks — now stated honestly; the verification matrix now permits a *validated* label only with an external oracle). **The four hardware/track-record badges (§D bus, §D RF-payload, §D quantum-hardware, §E flight-PA) deliberately did NOT flip to "strength"** — genuinely a partner's, and faking them would forfeit the honesty that is the differentiator.

**Founder-gated (cannot be drafted from code/audit alone):**
- T2 / T4 — German seat decision + ESA-STAR full registration (legal/financial)
- T5 / T6 — actual prime search & outreach (**explicitly parked until founder says go**)
- T7 / T8 — the consortium seat + the named quantum-nav Key Person (the hard gate)
- T10 / T11 / T14 — WP definition, founder CV/man-hours, and costing (need the teamed prime + founder inputs)
- T15 — the real SoW FPD-10029 (founder to obtain from the esa-star tender package)

**Decision rule (from the audit):** if no seat (T7) by ~late June, do **not** force a weak solo bid — pivot to T18 (OSIP) and target future phases / the sister tender 1-13494. Resolve eligibility (T2/T3) **before** committing to any bid.

---

_Last updated 2026-06-15. Sources: 7-document tender package in `1-13503_14062026014125/`; Kshana v0.17.0 `docs/CAPABILITY.md` / `docs/VALIDATION.md`; Ashforde ESA-STAR record (entity 83208, DE nationality)._
