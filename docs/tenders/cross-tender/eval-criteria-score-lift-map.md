<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
C03 — two FIREWALLED "how-Ashforde-lifts-the-score" briefs (one per ITT), mapping Ashforde's
contribution to each weighted evaluation criterion. AO/1-13494 and AO/1-13503 are SISTER tenders —
bid SEPARATELY; combined proposals forbidden. Keep the two briefs firewalled.
OUTREACH PARKED — no real prime names. Contact: contact@ashforde.org
-->

# Evaluation-criteria score-lift map — Ashforde per ITT

**PRIVATE — Ashforde OÜ internal working document.** Two **firewalled** briefs — one per ITT —
mapping Ashforde's contribution to each weighted evaluation criterion. A prime can lift the relevant
brief into its proposal's self-assessment / criterion-by-criterion rationale.

**Date: 2026-06-16 · Contact: contact@ashforde.org**

> **FIREWALL.** AO/1-13494 (AI & Digital Payload) and AO/1-13503 (Quantum-Enabled PNT) are **sister
> tenders** — bid **SEPARATELY**, combined proposals are forbidden. The two briefs below are kept
> firewalled: do not let one tender's content leak into the other's proposal.
>
> Ashforde = one SME WP seat, never prime, never a mandated Key Person. Kshana is **dual-licensed** —
> GNU **AGPL-3.0-only** (open track) or a **commercial licence** from Ashforde OÜ —
> **TRL ~3**, no flight heritage, no certification, no ESA endorsement; claims labelled **validated**
> (external oracle) or **modelled**. Outreach is **PARKED**.

**Shared evaluation weighting (both tenders):** 20 % Background · 30 % Understanding · 30 %
Programmatic/Engineering quality · 10 % Management & Cost · 10 % Admin & competition compliance.

---

## BRIEF A — AO/1-13494/26/NL/RR · AI & Digital Payload Technologies PNT Demonstrator

| Weight | Criterion | What Ashforde adds | Direction |
|---|---|---|---|
| **20 %** | **Background & experience** | A **validated open core** (Kshana, AGPL-3.0-only + commercial licence) with **five externally-validated islands** — SGP4/SDP4 (AIAA 2006-6753, 4.12 mm) · reference frames & timescales (SOFA/ERFA + Vallado, 0.1–4 mm) · Allan/ADEV (NIST SP 1065 / Stable32 on NBS14, 1e-4) · integrity RAIM/ARAIM (DO-229E/DO-316 + real IGS SP3 / Celestrak TLE) · ranging-code Gold trade (published GPS C/A −23.9 dB) — auditable line-by-line, plus a **DE-nationality SME** record. | ↑ |
| **30 %** | **Understanding of requirements** | Demonstrated grasp of the NAV-DT achievable-performance question and the RF-impairment problem: a **NAV-DT performance component** (ranging/timing accuracy, availability, integrity, GNSS-denied resilience) and a **detector-agnostic AI/ML RF-impairment evaluation testbed** (ROC/AUC/confusion/Pfa-Pmd, leakage guard, stratified split, distribution-shift optimism report — `impairment_eval`). The corpus is **synthetic/parameter-grounded (not IQ)** — stated honestly. | ↑ |
| **30 %** | **Programmatic / engineering quality** | The **SE spine**: a machine-checked **verification matrix** (`verification.rs`) that refuses "validated" without an external oracle, an **RTM/MRD** contribution, and an **ECSS-E-ST-10-02 verification cross-reference**; deterministic, scenario-hashed, SBOM-backed reproducibility; software-PA evidence (915+ lib tests, fmt+clippy clean, adversarially reviewed). | ↑ |
| **10 %** | **Management & cost** | A **fixed-scope, cap-compliant** SME WP that does not need re-costing under budget trims; fee ≤ the KAF cap via the prime's ECOS/PBF route; supporting-role man-hours (founder-gated cost detail — see C04 stub). | ↑ |
| **10 %** | **Admin & competition compliance** | A **DE-nationality SME** improving the scored consortium composition (≥3 SMEs, ≥30 % non-LSI, DE geo-return — see `sme-quota-geo-return-value.md`); a clean **BIPR** (Kshana Background IPR, dual-licensed AGPL-3.0-only + commercial; brought in under the **commercial licence** so no copyleft attaches to ESA's/the prime's use, *(commercial-licence grant terms to be confirmed by counsel)*; permissive-only dependency tree, SBOM, prior-auth route ready — see `bipr-ip-posture-pack.md`). Registered-office fix in flight, disclosed (founder-gated). | ↑ |

**Lead line for Brief A:** *auditability of the achievable-performance number* + *a referee-grade
AI/ML evaluation testbed* — the evidence assets a NAV-DT study must defend at GATEKEEPER and ISRR.

---

## BRIEF B — AO/1-13503/26/NL/RR · Quantum-Enabled PNT Demonstrator

| Weight | Criterion | What Ashforde adds | Direction |
|---|---|---|---|
| **20 %** | **Background & experience** | The same **validated open core** + **five externally-validated islands** (as Brief A) + **DE-nationality SME**. The Allan/ADEV island (NIST SP 1065 / Stable32) and the integrity island are directly on-point for a timing/clock-led quantum-PNT study. | ↑ |
| **30 %** | **Understanding of requirements** | Demonstrated grasp of the **quantum-vs-classical** trade: a trade engine (`quantum_trade`) that ingests a partner's **measured ADEV** (NNLS fit, round-trip-checked), produces a **timing/inertial-holdover benefit table** carrying the floor-assumption caveat, and a **GNSS-denied resilience-vs-time envelope** — it **quantifies** a partner's quantum benefit, it **never validates the device** (stated honestly). Underpinned by a first-principles CAI accelerometer model bracketed (not parity-matched) against Freier 2016. | ↑ |
| **30 %** | **Programmatic / engineering quality** | The **SE spine**: machine-checked **verification matrix**, **RTM/MRD**, **ECSS-E-ST-10-02 verification cross-reference**; deterministic/scenario-hashed/SBOM reproducibility; software-PA evidence. Same auditable spine as Brief A, applied to the quantum-trade WP. | ↑ |
| **10 %** | **Management & cost** | A **fixed-scope, cap-compliant** SME WP; fee ≤ KAF cap via the prime's ECOS/PBF route; supporting-role man-hours (founder-gated cost detail — C04 stub). | ↑ |
| **10 %** | **Admin & competition compliance** | **DE-nationality SME** lifting the scored composition (≥3 SMEs, ≥30 % non-LSI, DE geo-return); clean **BIPR** (Kshana Background IPR, dual-licensed AGPL-3.0-only + commercial; brought in under the **commercial licence** so no copyleft attaches *(grant terms to be confirmed by counsel)*; permissive-only dependency tree, SBOM, prior-auth route). Mandatory quantum-nav Key Person is **partner-owned** (never Ashforde). Registered-office fix in flight, disclosed (founder-gated). | ↑ |

**Lead line for Brief B:** *quantify the quantum benefit without overclaiming the device* — a
defensible, auditable trade study that survives review precisely because it labels its own
floor-assumptions and never claims device validation.

---

## Cross-cutting honesty guardrails (apply to BOTH briefs)

- **Five validated islands, everything else modelled.** Do not let a prime's editor promote a
  modelled row to "validated". The list of the five is fixed (see Brief A 20 %).
- **The AI-evaluation testbed and the quantum-trade engine are MODELLED** (harness logic / trade
  math), and their corpora/inputs are synthetic or partner-supplied — a high AUC or a large benefit
  delta demonstrates the **pipeline**, not field performance or a validated device.
- **No TRL>3, no flight, no certification, no ESA endorsement, no Key-Person claim.**
- **Four partner-owned hardware/PA gaps carry NO claim** (bus · RF-payload/antenna · quantum/payload
  HW · flight PA).

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open-core PNT performance & resilience simulator (AGPL-3.0-only + commercial licence) ·
TRL ~3, no flight heritage, no certification, no ESA endorsement · claims labelled validated (external oracle)
or modelled · sister tenders kept firewalled. Prime/partner outreach is PARKED pending explicit founder go.*
