<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
C01 — single-page WP-lead capability statement (scope Ashforde owns + clean prime-facing interface).
Applies to BOTH FutureNAV C3 tenders (AO/1-13494; AO/1-13503). OUTREACH PARKED — no real prime names.
Contact: contact@ashforde.org
-->

# WP-lead capability statement — the mission-analysis & benefits-case work-package

**PRIVATE — Ashforde OÜ internal working document.** A single-page statement of exactly the
work-package Ashforde owns and the **clean input/output interface** a prime feeds. Designed to drop
into either FutureNAV C3 proposal (AO/1-13494, AO/1-13503) as the SME WP-lead capability statement.

**Date: 2026-06-16 · Contact: contact@ashforde.org**

> Ashforde bids as **one SME work-package seat — never prime, never a mandated Key Person.** Kshana
> is a **dual-licensed** (GNU **AGPL-3.0-only** + **commercial licence**) open-core
> PNT-performance-and-resilience simulator, **TRL ~3**, no flight heritage,
> no certification, no ESA endorsement; capability claims are labelled **validated** (external oracle)
> or **modelled** throughout. Outreach is **PARKED** — this defines the asset, not a contact.

---

## 1. The scope Ashforde owns (and only this)

The **mission-analysis / benefits-case** work-package: turning a mission concept and an environment
into a **ranked, defensible benefit table and a sized design**, with the verification cross-reference
to back every number. Concretely:

| Scope element | What Ashforde does | Kshana module (open, auditable) | Status |
|---|---|---|---|
| **Constellation sizing** | Walker design sweep over `{planes × sats × inclination}`, gradient-free optimiser to the best design under a chosen objective (min-sats-for-coverage / max-coverage / min-worst-PDOP) | `walker` | validated geometry vs real Celestrak snapshot; optimiser returns brute-force winner |
| **DOP / coverage** | PDOP design grid, coverage-fraction & revisit-time figures of merit, streets-of-coverage geometry | `walker`, `fom` | modelled (closed-form geometry hand-verified) |
| **FoM-based benefit quantification** | Positioning/timing accuracy, availability, integrity (RAIM/ARAIM HPL/VPL), GNSS-denied resilience — per scenario, each a figure of merit with a validated/modelled tag | `fom`, `raim`, `jamming`, `quantum_trade` | RAIM/ARAIM **validated** (DO-229E/DO-316 + real IGS SP3 / Celestrak TLE); resilience/trade **modelled** |
| **MRD requirement matrix** | Requirement → capability → module → test → oracle → status rows for the mission-requirements / verification cross-reference | `verification` | machine-checked: a row is **validated** only with an external oracle |
| **Verification cross-ref** | ECSS-E-ST-10-02-style verification cross-reference rendered from the matrix (Markdown/CSV) | `verification` | self-enforced honesty invariant (CI-unit-tested) |

**Tender-specific accent (same engine, different lead question):**
- **13494 (AI & Digital Payload):** the benefit table answers *NAV-DT achievable-performance* and
  the *AI/ML RF-impairment detection-evaluation* question (ROC/AUC/Pfa-Pmd, leakage guard,
  distribution-shift report — `impairment_eval`). See `13494-ai-payload-pnt/technical-contribution.md`.
- **13503 (Quantum-Enabled PNT):** the benefit table answers the *quantum-vs-classical* trade
  (measured-ADEV ingestion → timing/inertial-holdover benefit table + GNSS-denied resilience
  envelope — `quantum_trade`). See `13503-quantum-pnt/technical-contribution.md`.

---

## 2. What Ashforde does NOT own (partner/prime-owned — explicit)

The spacecraft bus / AOCS / thermal / power; the RF/nav payload, antenna, signal-generation
hardware, SDR/FPGA; the quantum-sensor **device** hardware; the on-board / edge AI inference engine
and secure in-orbit reconfiguration; ground infrastructure / data lakehouse / DestinE-style
environment digital twins (Ashforde **consumes** the environment definition, does not build it);
receiver acquisition/tracking/correlator and raw-IQ; LCA / debris / eco-design; the **full** digital
twin; flight product assurance; and **any Key-Person role**. Ashforde is **one component**, never the
whole system.

---

## 3. The clean interface a prime feeds (in → out)

A single, well-defined boundary. The prime feeds three input bundles; Ashforde returns four output
artifacts. Everything is text-in / structured-out (TOML scenario in, JSON + SVG + Markdown/CSV out),
deterministic and **scenario-hashed** for reproducibility.

### Inputs (from prime / consortium)

| Input | Source WP | Content |
|---|---|---|
| **Scenario** | system / mission WP | mission concept, constellation/orbit options (LEO IOD), target PNT KPIs, operating points |
| **Environment** | environment / EO WP | RF environment definition — iono/tropo/multipath/interference scenarios; jamming/spoofing taxonomy & severity ranges |
| **Partner device params** | payload / quantum-HW WP | candidate payload performance envelope (ranging/timing error budget, antenna/link assumptions); for 13503, the quantum sensor's **measured ADEV / noise-budget coefficients** (Ashforde models *performance from* these — never the device itself) |

### Outputs (to prime / consortium)

| Output | Feeds | Content |
|---|---|---|
| **Ranked benefit table** | benefits-case / feasibility deliverables | per-scenario FoM set (accuracy, availability, integrity HPL/VPL, GNSS-denied resilience; for 13503, the quantum-vs-classical benefit delta), each row uncertainty-bounded and **validated/modelled-tagged**, ranked by the chosen objective |
| **Sized design** | sizing deliverables | the constellation/orbit design that meets the KPIs (planes/sats/inclination), with the design-sweep evidence and the where-it-meets / where-it-fails map |
| **MRD rows** | mission-requirements / verification deliverables | requirement → capability → module → test → oracle → status rows for the requirements traceability matrix |
| **Evidence pack** | SE verification / PA deliverables | the machine-checked verification cross-reference (Markdown/CSV), the per-result `scenario_hash`, and the reproducibility/SBOM artifacts |

**Boundary discipline:** Ashforde takes *parameters and definitions* across the interface and returns
*analysis artifacts*. It never reaches across into device hardware, payload signal generation, or the
ground/AI infrastructure — those stay behind the interface on the partner side.

---

## 4. Why this WP is a clean fit

- **Bounded & cap-compliant** — fixed scope that does not need re-costing under budget trims.
- **Auditable** — an open, auditable AGPL-3.0 core (dual-licensed; commercial licence available)
  a prime's chief engineer can verify line-by-line; the
  verification matrix refuses "validated" without an external oracle.
- **Reproducible** — deterministic, scenario-hashed, SBOM-backed results an SE/PA reviewer can re-run.
- **Composition-positive** — a DE-nationality SME filling the SME / non-LSI / DE-geo-return slots
  (see `sme-quota-geo-return-value.md`).

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open-core PNT performance & resilience simulator (AGPL-3.0-only + commercial licence) ·
mission-analysis & benefits-case WP only · TRL ~3, no flight heritage, no certification, no ESA endorsement ·
claims labelled validated (external oracle) or modelled. Prime/partner outreach is PARKED pending explicit founder go.*
