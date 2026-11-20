<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
Index of ESA tender pursuits. Contact: contact@ashforde.org
-->

# Tender pursuits (PRIVATE)

Internal bid-pursuit material for ESA (and other) tenders involving Kshana / Ashforde OÜ.
**Private — never publish in the open kshana repo.** All external contact: **contact@ashforde.org**.

> **Start here:** open [`INDEX.html`](INDEX.html) — the single clickable index of this entire
> dossier (deck, briefings, cost volume, and every per-tender / cross-tender document).
> This folder is the **canonical home** for all FutureNAV bid material.

| Tender | What | Status | Folder |
|--------|------|--------|--------|
| **AO/1-13494/26/NL/RR** | AI &amp; Digital Payload Technologies PNT Demonstrator (FutureNAV, Phases 0/A/B1), €3.5M study, deadline **9 Jul 2026** | Pursue **as consortium SME** — two fundable WPs (NAV-DT performance layer + AI-evaluation testbed); hard gate is securing a seat | [`13494-ai-payload-pnt/`](13494-ai-payload-pnt/) |
| **AO/1-13503/26/NL/RR** | Quantum-Enabled PNT Demonstrator (FutureNAV C3, Phases 0/A/B1), €3.5M study, deadline **17 Aug 2026** (extended from 9 Jul per Clar 2) | Pursue **as consortium SME** — hard gate is securing a seat (T7); see [`13503-quantum-pnt/CLARIFICATIONS-AUDIT.md`](13503-quantum-pnt/CLARIFICATIONS-AUDIT.md) | [`13503-quantum-pnt/`](13503-quantum-pnt/) |

**December 2026 bids (added 19 Nov 2026):**

| Tender | What | Status | Folder |
|--------|------|--------|--------|
| **AO/1-13695 (NAVISP EL1-132)** | Development and validation of a flexible eLoran/GNSS signal simulator, >500 k€ (Work Plan €800k, 24 months), closes **4 Jan 2027 13:00**, clarifications by 28 Dec. Open to DE, **not EE** | **Conditional GO** as prime via Ashforde UG with an RF/eLoran hardware partner; gate review 7 Dec, hard stop 14 Dec (UG validated in esa-star, German NAVISP delegate letter, partner LOI). Decision memo, compliance matrix, proposal draft, WP/cost skeleton (DRAFT ≈ €604k at €750/day), risks, clarification + helpdesk questions | [`13695-eloran-simulator/`](13695-eloran-simulator/) |
| **UK Dstl RFI 2026/S 000-107427** | Resilient timing and positioning system for degraded GNSS (±30 ns, <5 m, 12 h holdover, jam/spoof incl. acquisition, TRL 9, no ITAR/fibre); RFI closes **9 Dec 2026 17:00 UK** | **GO (capability response only).** Kshana is the test-and-evidence layer, not a TRL 9 system; includes the non-UK-SME eligibility question | [`dstl-rtps-rfi-2026/`](dstl-rtps-rfi-2026/) |
| **AO/1-13705 (S3-SD-08)** | Debris-sensor operational software (OD, scheduling, track correlation), 200–500 k€, closes **1 Dec 2026**, clarifications 24 Nov; EE eligible | **NO-GO as prime** (core functions not built, no sensor partner, 12 days); partner-only if invited by 24 Nov | [`13705-debris-sensor-sw/`](13705-debris-sensor-sw/) |

**Adjacent program-area positioning (not a current AO):**

| Area | What | Status | Folder |
|--------|------|--------|--------|
| **Lunar PNT** | Anticipatory fit assets for ESA's lunar PNT programme (Moonlight/LCNS, NovaMoon-class geodetic-timing-differential station, LunaNet/IOAG interop): capability statement + module→need fit map | **Shelf assets, outreach PARKED** pending founder go | [`lunar-pnt/`](lunar-pnt/) |

## Top-level deliverables (this folder)

- [`INDEX.html`](INDEX.html) — master clickable index of the whole dossier.
- [`futurenav-dominance-plan.html`](futurenav-dominance-plan.html) — the master deck (battle plan, personas, embedded briefing + C04 screenshots, competitor intel). A loose convenience copy also sits at `~/Downloads/`.
- [`briefings/`](briefings/) — the four per-tender briefing pages (what-it-asks + how-Kshana-fits, honest validated/modelled labels).
- [`cost-volume/`](cost-volume/) — the **C04** ECOS/FFP cost-volume calculator (market-researched €750/day; worked ≈€196,345; fee ≤8%) + its screenshot.

## Current state (2026-06-18)

- **8 external-oracle validated islands** in the open core (SGP4/AIAA, Allan/NIST, frames/ERFA, integrity/DO-229E, DOP/gnss_lib_py, ML-eval/scikit-learn, quantum-trade kernels/scipy, Mars/DE440) — all green on CI.
- **C04 cost volume delivered** (market-researched, not yet ECOS-registered).
- **Founder-gated remainder:** confirm/register the ECOS day-rate, lock the consortium seat + named Key Person, then submit.

## Conventions

- One folder per tender, named `<id>-<short-slug>`.
- Each folder has a master tracker `TENDER-<id>-PURSUIT.md` as the single source of truth,
  plus the draft-ready deliverables (capability brief, technical contribution + SoW
  compliance matrix, eligibility Q&A, BIPR declaration, consortium target map, OSIP note).
- A canonical copy of the source gap-audit (`gap-audit.html`) lives in the folder.
- **Founder-gated items** (prime search/outreach, CV, costing, the consortium seat, the real
  SoW) are marked in the tracker and are *not* drafted speculatively.

## Standing rules

- This material stays in the **private** kshana-pro repo. Capability/credibility copy for
  *external* outreach is in [`../ESA-CAPABILITY-STATEMENT-kshana-paragraph.md`](../ESA-CAPABILITY-STATEMENT-kshana-paragraph.md).
- Kshana = heritage/evidence; never overclaim TRL, certification, or ESA endorsement.
- Keep every quantitative claim labelled **validated-or-modelled** (see the open repo's
  `docs/CAPABILITY.md`).
