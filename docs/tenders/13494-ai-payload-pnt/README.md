<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; never publish in the open repo.
Index for the ESA AO/1-13494 (AI & Digital Payload Technologies PNT Demonstrator) pursuit.
Contact: contact@ashforde.org
-->

# Tender 13494 — AI & Digital Payload Technologies PNT Demonstrator (Phases 0/A/B1)

**ESA ref:** AO/1-13494/26/NL/RR · FutureNAV Component 3 · SoW FPD-10030 · €3.5M FFP · **deadline 9 Jul 2026 13:00 Amsterdam** (Q&A ~29 Jun).
**Sister tender to 13503** (Quantum-Enabled PNT) — bid **separately** (combined proposals forbidden); shared PARD/ECSS docs are byte-identical across the two packages.

**Honest position:** pursue as a **consortium-SME work-package contributor — never prime, never a mandated Key Person.** Ashforde owns the **RF-performance component of the NAV-DT + the AI-algorithm evaluation testbed + the machine-checked ECSS verification matrix**. 13494's ground segment *is* a Navigation Digital Twin, making it Kshana's strongest honest fit.

## Contents
- [`TENDER-13494-PURSUIT.md`](TENDER-13494-PURSUIT.md) — master pursuit tracker (N1–N22; hard gate = consortium seat).
- [`kshana-capability-brief.md`](kshana-capability-brief.md) — 1–2 pp prime-facing capability brief.
- [`technical-contribution.md`](technical-contribution.md) — paste-into-WBS WP definitions (WP-A NAV-DT performance component, WP-B AI-evaluation testbed) + SoW compliance-matrix template.

## Cross-tender assets (reuse from the 13503 folder unless tender-specific)
BIPR/IPR posture, SE-verification-plan, software-PA-evidence, TRL-maturation-dossier, OSIP note, and the **two-tender** `../futurenav-dominance-plan.html` apply across both tenders.

## Built and verified in the open `kshana` repo (strengthens this bid)
- `src/impairment_eval.rs` — labelled synthetic RF-impairment corpus + detector-agnostic ROC/AUC/confusion/Pfa-Pmd evaluation testbed (leakage guard, stratified split, distribution-shift optimism report). Owns the SoW's "Characterisation & Evaluation of Machine-Learning Techniques".
- `src/verification.rs` — machine-checked verification matrix (a row is *validated* only with an external oracle).
- FoM engine + RAIM/ARAIM + holdover → the NAV-DT achievable-performance/resilience component.

All claims labelled **validated** (external oracle) or **modelled**. 913 lib tests, fmt+clippy clean, adversarially reviewed. Nothing flight, no TRL>3, no hardware.

## Founder-gated (cannot be drafted from code/audit)
German registered-office/branch (eligibility KAF — EE seat is the #1 risk) · ESA-STAR full registration · the consortium seat + named Key Person (a partner's) · CV/man-hours · costing · obtaining the real SoW FPD-10030 cross-check · prime/partner outreach (**PARKED** until founder go).

_Last updated 2026-06-15. PRIVATE — do not publish._
