<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Pre-rendered ARCHETYPE brief (dominance-plan step A13) — quantum-hardware-anchor class (AO/1-13503).
ARCHETYPE ONLY: no real partner named. OUTREACH PARKED until the founder explicitly says go.
Rendered from TEMPLATE.md with the {{PRIME_NAME}} placeholder bound to the ARCHETYPE label, not a candidate.
Contact: contact@ashforde.org
-->

# Kshana — capability brief for [Quantum-hardware anchor — ARCHETYPE]
### Quantum-Enabled PNT Demonstrator (FutureNAV C3, Phase 0/A · AO/1-13503/26/NL/RR)

**PRIVATE — Ashforde OÜ internal working document.** ARCHETYPE brief on the shelf. Not a sent document; no partner selected.
**Date: 2026-06-15 · Contact: contact@ashforde.org**

> ARCHETYPE NOTE: this brief is pre-rendered against the *class* of quantum-hardware anchor — the atomic-clock /
> cold-atom-interferometer developer that is the demonstrator's technical core and the source of the mandatory
> quantum-nav Key Person. No candidate is named. Outreach is PARKED; producing this asset does not authorise contact.

---

## 1. One honest identity

Kshana is **the ground-side performance-and-trade engine that quantifies — never validates — your quantum device's benefit for PNT.** Ashforde OÜ (sole founder, Chakshu Baweja) offers one tightly-scoped SME seat — **the navigation-performance & quantum-vs-classical trade-study WP** — that sits *downstream* of your hardware and turns its measured numbers into scored mission benefit. We do not touch your device, your lab, or your Key-Person role.

We bid strictly as a **consortium-SME work-package contributor — never prime, never a mandated Key Person.** Kshana is an open Apache-2.0 Rust core (913+ library tests, fmt + clippy clean, adversarially reviewed) you and the prime can audit line-by-line. **TRL ~3** software-model substrate. **No flight heritage, no agency contracts, no certification, no ESA endorsement** — stated plainly.

---

## 2. The work-package we own (and only this)

You are the technical anchor: the quantum device and the mandatory **quantum-nav Key Person** are yours. Ashforde owns the **complementary** WP — the neutral, reproducible engine that scores *what quantum buys over classical, and where it pays off*. We ingest your **measured** Allan/noise data and produce a holdover-and-resilience benefit table; we never assert device performance, and the artifact carries the floor-assumption caveat so the number is honest.

- **WP scope (in):** quantum-vs-classical trade engine (measured-ADEV ingestion via NNLS, timing/inertial holdover benefit, GNSS-denied resilience-vs-time envelope); GNSS-denied resilience (anti-jam link budget, multi-layer spoof suite, alternative PNT — gravity-map / terrain / magnetic-anomaly matching); mission analysis (Walker design + optimiser, coverage/revisit/DOP); timing architecture (ADEV/MDEV/TDEV/HDEV with NIST-SP-1065 CIs, van-Loan Kalman clock, holdover, two-way time transfer); integrity (RAIM/MHSS/ARAIM); the machine-checked ECSS verification cross-reference.
- **WP scope (out, partner/prime-owned):** quantum / atomic-clock / cold-atom **hardware design and maturation** (yours); the mandatory quantum-nav Key Person (yours); system engineering / ECSS ownership / submission (prime's); nav RF payload / signal / antenna design; spacecraft bus (AOCS/thermal/structures/power); flight product assurance.
- **Deliverables it feeds:** the Phase-0/A "what does quantum buy" trade study, achievable-performance and resilience assessments, mission-concept sizing, and the PA/QA + SEP + ECSS-E-ST-10-02 verification cross-reference.
- **Clean interfaces:** consumes **your measured device data** (ADEV/noise PSDs, per-shot SNR) + reference geometry; emits reproducible, scenario-hashed benefit tables, resilience envelopes, integrity diagrams, and the verification matrix. One narrow boundary; no overlap with your hardware WP.

---

## 3. Score-delta sheet — what an Ashforde seat does to your consortium's scored proposal

Mapped to the FutureNAV evaluation weights **20 / 30 / 30 / 10 / 10**.

| Weight | Criterion | What an Ashforde WP adds | Direction |
|---|---|---|---|
| 20% | Background & experience | An openly auditable PNT-performance + quantum-trade codebase as concrete evidence; a DE-SME presence for the consortium-composition view. | ↑ |
| 30% | Understanding of requirements & objectives | Directly instruments the demonstrator's central question — *what does quantum buy over classical for PNT, and where* — with a single neutral code path scoring candidate architectures on identical figures of merit. | ↑ |
| 30% | Quality & suitability of proposed programme | A verification matrix that refuses "validated" without an external oracle; every FoM reproducible from a committed scenario + fixed seed + pinned version — direct PA/QA, SEP, ECSS-E-ST-10-02 evidence. | ↑ |
| 10% | Management, costing, planning | A single, narrow, fixed-scope, cap-compliant SME WP with a clean interface to your hardware data — low integration risk, nothing to re-cost. | ↑ |
| 10% | Compliance with administrative tender conditions | DE nationality + SME status mechanically lift consortium-composition scoring (≥3 SMEs, ≥30% non-LSI budget). | ↑ |

**Consortium-composition arithmetic (scored, not a KAF).** A **German-nationality SME** (ESA-STAR entity held in private notes) → one of the required **≥3 SMEs** and budget toward **≥30%-to-non-LSI**; feeds the **DE-weighted geographical return** (DE is the top return at ~48.89% on 13503); a **cap-compliant** WP that does not have to be re-costed.

**Open registered-office item (disclosed, not hidden).** Ashforde's registered office is currently in Estonia (not a participating state). Light ESA-STAR registration suffices at submission; a participating-state registered office is required **by award**. Fix in flight: a German branch/subsidiary plus an ESA Q&A clarification.

---

## 4. The audit-the-core hook (why us over the black-box incumbents)

A quantum demonstrator's benefit case is only as credible as the engine that computes it. If that engine is a black box, the panel must take the "quantum wins" verdict on trust. Ashforde's engine is **open Apache-2.0, readable line-by-line, reproducible from a committed scenario + fixed seed + pinned version**, and ships a public honest maturity ledger.

The structural differentiator: a **unit-tested verification matrix that refuses to mark any capability "validated" without an independent external oracle** (`src/verification.rs`), on an open core — versus the closed simulators in this space (GMV, Qascom, IFEN, ANavS, M3 Systems, Spirent, Safran-Skydel). Critically, the trade engine is built to **quantify, never validate** your device: the numbers are MODELLED with no validation halo, and the trade artifact carries the floor-assumption caveat — exactly the honesty an ESA Phase-0/A trade study rewards, and exactly what keeps your hardware claims yours, cleanly separated from our scoring.

---

## 5. Validated-or-modelled capability table

Every claim is tagged **validated** (independent external oracle) or **modelled** (published / first-principles physics with tests, no external oracle yet). The labelling is enforced in code.

| # | Capability | Kshana module | Status | Oracle |
|---|---|---|---|---|
| 1 | Quantum-vs-classical PNT trade engine (measured-ADEV ingestion, benefit table, resilience envelope) | `quantum_trade` | **modelled** | ADEV→q round-trip vs holdover model; **quantifies, never validates** a partner device |
| 2 | Frequency stability (ADEV/MDEV/TDEV/HDEV + noise-type CIs) | `allan` | **validated** | NIST SP 1065 / Stable32 on NBS14 (1e-4) |
| 3 | Cold-atom interferometer (CAI) accelerometer — first principles (Φ = k_eff·a·T², shot-noise floor, Coriolis/AC-Stark) | `inertial::quantum_imu` | **modelled** | Freier et al. 2016 mobile gravimeter — order-of-magnitude bracket, **not** parity |
| 4 | GNSS-denied clock holdover (coast-error growth, holdover-to-threshold) | `holdover` | **modelled** | multi-step Kalman covariance recursion (same-codebase cross-check) |
| 5 | Onboard clock state estimation (van-Loan Kalman, Joseph-stabilised) | `clock_state` | **modelled** | analytic van-Loan Q closed form |
| 6 | GNSS-denied resilience (anti-jam C/N₀, spoof suite, alt-PNT) | `jamming`, `spoof*`, `altpnt`, `gravimeter`, `igrf` | **modelled** | Kaplan & Hegarty; TEXBAT *parameters* (characterisation) |
| 7 | SGP4/SDP4 orbit propagation | `sgp4`, `propagator` | **validated** | AIAA 2006-6753 — 666 vectors, 4.12 mm |
| 8 | Reference frames & timescales (IAU 2006/2000A, CIO GCRS↔ITRS) | `frames`, `precession`, `nutation`, `cio`, `timescales` | **validated** | SOFA/ERFA vectors + Vallado (0.1–4 mm) |
| 9 | Integrity — snapshot/MHSS RAIM, ARAIM P_HMI budget | `raim`, `sbas`, `lunar` | **validated** | DO-229E/DO-316 + real IGS SP3 / Celestrak TLE |
| 10 | Machine-checked verification matrix | `verification` | **validated** (classification invariants) | unit-tested status/oracle-kind invariants |
| — | Quantum-payload HW · spacecraft bus · RF-payload/antenna HW · flight PA | — | **partner-owned** | no module, no claim, by design |

> Note: most quantum sensors are modelled from **published Allan/noise coefficients**; only the CAI
> accelerometer is first-principles, and even that is bracketed (within ~2 orders of Freier et al. 2016,
> 96 nm/s²/√Hz), not parity-matched. The quantum-payload **hardware** is explicitly your discipline — Kshana
> holds no device-design claim.

---

## 6. BIPR / IP line (clean, declarable)

- **Background IPR:** Kshana — open **Apache-2.0** PNT-performance-and-resilience engine (v0.17.0), owned by Ashforde OÜ. Used as the analysis tool; the **results** are the deliverables under the contract's Foreground-IPR terms.
- **No copyleft anywhere in the dependency tree** — permissive allow-list (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0), CI-enforced by `cargo-deny` (`deny.toml`).
- **Apache-2.0 prior-authorisation note:** open-source use as a tool may require **ESA prior authorisation** under GCC Part II (Option A); declarable via the prime's BIPR/BIPR-table route. Ashforde supplies the open-source-use declaration (component + repo + release tag + permissive SBOM + no-GPL confirmation).
- **Foreground / Pro overlay:** a **customer/device-specific quantum-payload digital twin** would be the first **Kshana Pro** Foreground-IP instance, delivered under the agreed NDA/IPR and any export-control terms — distinct from the Apache-2.0 background engine, and from your device IP, which stays yours.

---

## 7. What this offer is — and is not

**Is:** a defined, auditable, fixed-scope SME work-package — the navigation-performance & quantum-vs-classical trade WP — ready to fold into your 13503 consortium proposal as the downstream scoring engine for your hardware's benefit.

**Is not:** a prime, a Key Person, a quantum-hardware capability, or any partner-owned bus/RF/ground element. Ashforde does not satisfy the consortium or Key-Personnel mandates alone and does not claim to.

*Prime/partner outreach is **PARKED** pending explicit founder go. This archetype brief defines the asset only; producing it does not authorise contacting anyone.*

---
*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator · capability claims labelled validated (external oracle) or modelled throughout · TRL ~3, no flight heritage, no certification, no ESA endorsement.*
