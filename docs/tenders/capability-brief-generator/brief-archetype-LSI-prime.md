<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Pre-rendered ARCHETYPE brief (dominance-plan step A13) — LSI / system-integrator prime class.
ARCHETYPE ONLY: no real prime named. OUTREACH PARKED until the founder explicitly says go.
Rendered from TEMPLATE.md with the {{PRIME_NAME}} placeholder bound to the ARCHETYPE label, not a candidate.
Contact: contact@ashforde.org
-->

# Kshana — capability brief for [LSI / system-integrator prime — ARCHETYPE]
### FutureNAV PNT Demonstrator (AO/1-13494 NAV-DT class · AO/1-13503 quantum class)

**PRIVATE — Ashforde OÜ internal working document.** ARCHETYPE brief on the shelf. Not a sent document; no prime selected.
**Date: 2026-06-15 · Contact: contact@ashforde.org**

> ARCHETYPE NOTE: this brief is pre-rendered against the *class* of large system-integrator prime — the
> operator that owns SE, ECSS deliverables, submission, and consortium management. No candidate is named.
> Outreach is PARKED; producing this asset does not authorise contacting anyone.

---

## 1. One honest identity

Kshana is **a single, auditable navigation-performance work-package inside your system — not the system, and not a competitor for your prime role.** Ashforde OÜ (sole founder, Chakshu Baweja) offers your consortium one tightly-scoped SME seat — **the navigation-performance, integrity, and trade-study WP** — and nothing more.

We bid strictly as a **consortium-SME work-package contributor — never prime, never a mandated Key Person.** Kshana is an open Apache-2.0 Rust core (913+ library tests, fmt + clippy clean, adversarially reviewed) your engineers can audit line-by-line. **TRL ~3** software-model substrate. **No flight heritage, no agency contracts, no certification, no ESA endorsement** — stated plainly.

---

## 2. The work-package we own (and only this)

For a prime that owns the whole lifecycle, Ashforde plugs in as the **navigation-performance / Figure-of-Merit / integrity / trade-study WP**: the layer that predicts achievable PNT accuracy, availability, integrity (RAIM/ARAIM), and GNSS-denied resilience — and, where the tender asks, evaluates ML/AI detectors with a leakage-guarded, optimism-reporting testbed. It sits *below* your NAV-DT / SpaceStream / ground architecture and feeds it numbers, never replacing it.

- **WP scope (in):** PNT FoM engine (ranging/timing accuracy, availability, integrity); RAIM/ARAIM P_HMI budgeting; GNSS-denied resilience (jamming/spoofing/alt-PNT); mission-geometry / constellation feasibility; the machine-checked ECSS verification cross-reference; (13494) the detector-agnostic AI-algorithm evaluation testbed + labelled synthetic RF-impairment corpus; (13503) the quantum-vs-classical trade engine.
- **WP scope (out, partner/prime-owned):** system engineering and ECSS lifecycle ownership; the high-fidelity NAV-DT / RF-environment twin; SpaceStream / NAVEX closed loop; ground segment / data lakehouse / DestinE-style twins; on-board AI / edge inference; payload, antenna, SDR/FPGA hardware; spacecraft bus (AOCS/thermal/structures/power); quantum / RF-payload hardware; flight product assurance.
- **Deliverables it feeds:** NAV-DT achievable-performance assessment, algorithm-verification / performance-benchmark slices, the PA/QA + SEP + ECSS-E-ST-10-02 verification cross-reference, and the Phase-0/A mission-concept / sizing trades.
- **Clean interfaces:** consumes scenario definitions + reference geometry/EOP; emits reproducible, scenario-hashed FoM tables, integrity diagrams, ROC/AUC evaluation reports, and the verification matrix (Markdown/CSV). One narrow boundary, no scope creep into your SE.

---

## 3. Score-delta sheet — what an Ashforde seat does to your scored proposal

Mapped to the FutureNAV evaluation weights **20 / 30 / 30 / 10 / 10**.

| Weight | Criterion | What an Ashforde WP adds | Direction |
|---|---|---|---|
| 20% | Background & experience | A working, test-covered, openly auditable PNT-performance + ML-evaluation codebase as concrete evidence (not a heritage claim); a DE-SME presence for the consortium-composition view. | ↑ |
| 30% | Understanding of requirements & objectives | Instruments the harder-to-fake SoW lines — achievable-performance assessment, algorithm verification, integrity, and (13494) "Characterisation & Evaluation of ML Techniques" — showing depth a narrative alone cannot. | ↑ |
| 30% | Quality & suitability of proposed programme | A verification matrix that refuses "validated" without an external oracle, plus leakage guard / stratified split / distribution-shift optimism report — direct, auditable PA/QA, SEP and ECSS-E-ST-10-02 evidence. | ↑ |
| 10% | Management, costing, planning | A single, narrow, fixed-scope, cap-compliant SME WP with clean interfaces — low integration risk, nothing to re-cost, no creep into your hardware/SE. | ↑ |
| 10% | Compliance with administrative tender conditions | DE nationality + SME status mechanically lift consortium-composition scoring (≥3 SMEs, ≥30% non-LSI budget). | ↑ |

**Consortium-composition arithmetic (scored, not a KAF).** Adding Ashforde moves numbers the prime must hit anyway: a **German-nationality SME** (ESA-STAR entity held in private notes) → one of the required **≥3 SMEs** and budget share toward **≥30%-to-non-LSI**; feeds the **DE-weighted geographical return** (DE is the top return on both demonstrators); a **low-friction, cap-compliant** WP that does not have to be re-costed.

**Open registered-office item (disclosed, not hidden).** Ashforde's registered office is currently in Estonia (not a participating state). Light ESA-STAR registration suffices at submission; a participating-state registered office is required **by award**. Fix in flight: a German branch/subsidiary plus an ESA Q&A clarification. Flagged up front so no rival raises it first.

---

## 4. The audit-the-core hook (why us over the black-box incumbents)

For a prime assembling a credibility case, the navigation-performance WP is usually a closed simulator the panel must take on trust. Ashforde inverts that: the engine is **open Apache-2.0, readable line-by-line, reproducible from a committed scenario + fixed seed + pinned version**, and ships a public honest maturity ledger (`docs/CAPABILITY.md`, `docs/VALIDATION.md`, `docs/CLAIMS-VS-REALITY.md`) stating per capability exactly what is *yes / partial / none*.

The structural differentiator: a **unit-tested verification matrix that refuses to mark any capability "validated" without an independent external oracle** (`src/verification.rs`), on an open core — versus the closed simulators in this space (GMV magicGNSS / SVS, Qascom SimSAFE / QA707, M3 Systems StellaNGC, IFEN NavX-NCS, Spirent, Safran-Skydel, ANavS). For a study whose credibility rests on *defensible* numbers, a tool that publishes its own validated-vs-modelled boundary is an evidence asset you can show ESA, not a feature.

---

## 5. Validated-or-modelled capability table

Every claim is tagged **validated** (independent external oracle) or **modelled** (published / first-principles physics with tests, no external oracle yet). The labelling is enforced in code — a row cannot be marked validated on a self-consistency check.

| # | Capability | Kshana module | Status | Oracle |
|---|---|---|---|---|
| 1 | SGP4/SDP4 orbit propagation | `sgp4`, `propagator` | **validated** | AIAA 2006-6753 — 666 vectors, 4.12 mm |
| 2 | Reference frames & timescales (IAU 2006/2000A, CIO GCRS↔ITRS) | `frames`, `precession`, `nutation`, `cio`, `timescales` | **validated** | SOFA/ERFA vectors + Vallado worked example (0.1–4 mm) |
| 3 | Frequency stability (ADEV/MDEV/TDEV/HDEV + noise-type CIs) | `allan` | **validated** | NIST SP 1065 / Stable32 on NBS14 (1e-4) |
| 4 | Integrity — snapshot/MHSS RAIM, ARAIM P_HMI budget | `raim`, `sbas`, `lunar` | **validated** | DO-229E/DO-316 + real IGS SP3 / Celestrak TLE |
| 5 | Ranging-code design trade (Gold/m-sequence bounds) | `navsignal` (CodeFamily) | **validated** | Published GPS C/A Gold −23.9 dB |
| 6 | PNT Figure-of-Merit engine (accuracy / availability / resilience) | FoM / performance core | **modelled** | first-principles; internal cross-checks |
| 7 | GNSS-denied resilience (anti-jam C/N₀, spoof suite, alt-PNT) | `jamming`, `spoof*`, `altpnt`, `gravimeter`, `igrf` | **modelled** | Kaplan & Hegarty; TEXBAT *parameters* (characterisation) |
| 8 | (13494) Detector-agnostic AI-algorithm evaluation testbed | `impairment_eval` | **modelled** | closed-form AUC bounds + perfect-oracle detector; corpus SYNTHETIC, not IQ |
| 9 | (13503) Quantum-vs-classical PNT trade engine | `quantum_trade` | **modelled** | ADEV→q round-trip vs holdover model; quantifies, never validates, a partner device |
| 10 | Machine-checked verification matrix | `verification` | **validated** (classification invariants) | unit-tested status/oracle-kind invariants |
| — | Spacecraft bus · RF-payload/antenna HW · quantum-payload HW · flight PA | — | **partner-owned** | no module, no claim, by design |

> Note: row 8's testbed *harness logic* is validated against closed-form AUC bounds; the bundled detectors are
> transparent published-method baselines (not SOTA) and the impairment corpus is synthetic (parameter-grounded,
> not raw IQ) — a high AUC demonstrates the pipeline, not field performance. Row 9 *quantifies* a partner's
> quantum benefit; it never validates the device.

---

## 6. BIPR / IP line (clean, declarable)

- **Background IPR:** Kshana — open **Apache-2.0** PNT-performance-and-resilience engine (v0.17.0), owned by Ashforde OÜ. Used as the analysis tool; the **results** are the deliverables and follow the contract's Foreground-IPR terms.
- **No copyleft anywhere in the dependency tree** — permissive allow-list (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0), CI-enforced by `cargo-deny` (`deny.toml`).
- **Apache-2.0 prior-authorisation note:** open-source use as a tool may require **ESA prior authorisation** under GCC Part II (Option A); declarable via your BIPR/BIPR-table route. Ashforde supplies the open-source-use declaration (component + repo + release tag + permissive SBOM + no-GPL confirmation).
- **Foreground / Pro overlay:** a customer/device-specific digital twin would be the first **Kshana Pro** Foreground-IP instance, under the agreed NDA/IPR and any export-control terms — distinct from the Apache-2.0 background engine.

---

## 7. What this offer is — and is not

**Is:** a defined, auditable, fixed-scope SME work-package — the navigation-performance / integrity / trade WP — ready to fold into your FutureNAV consortium proposal.

**Is not:** a prime, a Key Person, the whole system, or any partner-owned hardware/ground/platform element. Ashforde does not satisfy the consortium or Key-Personnel mandates alone and does not claim to.

*Prime/partner outreach is **PARKED** pending explicit founder go. This archetype brief defines the asset only; producing it does not authorise contacting anyone.*

---
*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator · capability claims labelled validated (external oracle) or modelled throughout · TRL ~3, no flight heritage, no certification, no ESA endorsement.*
