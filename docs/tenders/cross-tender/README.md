<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Index of cross-tender / dominance-plan strategy assets shared across BOTH FutureNAV C3 tenders.
Contact: contact@ashforde.org
-->

# Cross-tender strategy assets (PRIVATE)

**PRIVATE — Ashforde OÜ internal bid material.** Strategy / dominance-plan assets that apply to
**both** ESA FutureNAV Component-3 tenders. Never publish in the open kshana repo. All external
contact: **contact@ashforde.org**.

## Which tenders these cover

| Tender | What | Folder |
|---|---|---|
| **AO/1-13494/26/NL/RR** | AI & Digital Payload Technologies PNT Demonstrator (FutureNAV C3, Phases 0/A/B1) | [`../13494-ai-payload-pnt/`](../13494-ai-payload-pnt/) |
| **AO/1-13503/26/NL/RR** | Quantum-Enabled PNT Demonstrator (FutureNAV C3, Phases 0/A/B1) | [`../13503-quantum-pnt/`](../13503-quantum-pnt/) |

**Shared facts (both):** close **9 Jul 2026** (esa-star only, no late); same **20/30/30/10/10**
evaluation; same consortium rule (**≥5 operators incl. prime · ≥3 SMEs · ≥30 % budget to non-LSI ·
≥€500k via open competition**); same **registered-office-in-a-Participating-State** KAF (by award).
**Bid SEPARATELY — combined proposals are forbidden; keep the two firewalled.**

## Standing rules (apply to every file in this folder)

- Ashforde bids as **one consortium-SME work-package seat — never prime, never a mandated Key Person.**
- Kshana = open **Apache-2.0** PNT-performance-and-resilience simulator, **TRL ~3**, **no flight
  heritage, no certification, no ESA endorsement**; every quantitative claim labelled **validated**
  (independent external oracle) or **modelled** (published / first-principles, tested) — the same
  invariant the code enforces in the open repo's `src/verification.rs` (five externally-validated
  islands; everything else modelled).
- **Prime/partner outreach is PARKED** pending explicit founder go. These are shelf assets; producing
  them does not authorise contacting anyone. **No real prime/partner names** appear in this folder.
- Business contact **contact@ashforde.org** only.

## Index

| ID | File | One-line summary |
|---|---|---|
| **B04** | [`sme-quota-geo-return-value.md`](sme-quota-geo-return-value.md) | One-page-per-tender quota arithmetic showing how a DE-SME Ashforde seat is score-positive on the ≥5/≥3-SME/≥30 %-non-LSI/≥€500k rule + the DE 48.89 % top geo-return weighting. |
| **B14** | [`bipr-ip-posture-pack.md`](bipr-ip-posture-pack.md) | Single shared BIPR/IP posture pack for both proposals — no-GPL/no-copyleft guarantee (cargo-deny + SBOM) + the Background↔Foreground boundary; cross-refs the canonical `13503/BIPR-declaration.md` (does not duplicate). |
| **C01** | [`wp-scope-interface-capability-statement.md`](wp-scope-interface-capability-statement.md) | Single-page WP-lead statement of the mission-analysis/benefits-case scope Ashforde owns + the clean input (scenario/environment/device-params) → output (ranked benefit table / sized design / MRD rows / evidence pack) interface. |
| **C03** | [`eval-criteria-score-lift-map.md`](eval-criteria-score-lift-map.md) | Two firewalled per-ITT briefs mapping Ashforde's contribution to each weighted criterion (20/30/30/10/10) for 13494 (NAV-DT perf + RF-impairment AI-eval) and 13503 (quantum-vs-classical trade). |
| **C05** | [`kaf-doc-admin-checklist.md`](kaf-doc-admin-checklist.md) | Ashforde-as-subcontractor readiness checklist per ITT — the KAF1–KAF7 list + Declarations of Compliance: what Ashforde must declare/furnish (founder-gated price/sign/CV items flagged, not fabricated). |
| **C06** | [`bipr-apache2-prior-auth-route.md`](bipr-apache2-prior-auth-route.md) | Both-tender-ready BIPR table + the explicit ESA open-source-software-use prior-authorisation request (component/repo/tag/licence inventory) with cargo-deny + CycloneDX SBOM evidence (cross-refs B14). |
| **C07** | [`partner-selection-criteria.md`](partner-selection-criteria.md) | Partner/prime selection criteria + the "why team Ashforde" value narrative (nationality fit DE>CH, ESA heritage, facilities, complementarity, SME-sub appetite). NAMES PARKED. |
| **C08** | [`partner-target-rubric.md`](partner-target-rubric.md) | Per-role weighted scoring rubric (nationality 20 / heritage 20 / complementarity 25 / facilities 10 / SME appetite 15 / Key-Person 10) with decision bands + hard gates; blank candidate table by design. NAMES PARKED. |

## Founder-gated companions (drafted elsewhere / not in this folder)

**C02 — registered-office eligibility clarification** and **C04 — ECOS/FFP cost volume** are
**FOUNDER-GATED** and are **not** drafted speculatively here. C02 (whether a German branch/subsidiary
satisfies the participating-state registered-office rule for the EE-seated entity, and the timing
relative to submission) is an **ESA Q&A / esa-star filing** the founder submits — its draft lives in
each tender folder's Q&A asset (e.g. `13494-ai-payload-pnt/ESA-QA-draft.md`, `13503-quantum-pnt/ESA-QA-draft.md`)
and the registered-office decision is tracked in each `TENDER-…-PURSUIT.md`. C04 (the binding FFP
cost volume) depends on **founder day-rates** and the teamed prime's ECOS/PBF route; **no prices are
fabricated** and nothing is filed from this folder. Both are referenced (not duplicated) by C05 and the
score-lift map.

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator ·
TRL ~3, no flight heritage, no certification, no ESA endorsement · claims labelled validated (external oracle)
or modelled · sister tenders kept firewalled. Prime/partner outreach is PARKED pending explicit founder go.*
