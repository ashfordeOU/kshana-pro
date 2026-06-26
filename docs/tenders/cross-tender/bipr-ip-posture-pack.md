<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
B14 — single BIPR/IP posture pack usable in BOTH FutureNAV C3 proposals
(AO/1-13494 AI & Digital Payload; AO/1-13503 Quantum-Enabled PNT).
Extends — does NOT duplicate — 13503-quantum-pnt/BIPR-declaration.md.
Contact: contact@ashforde.org
-->

# BIPR / IP posture pack — both FutureNAV C3 tenders

**PRIVATE — Ashforde OÜ internal working document.** One IP-posture pack a prime can lift into
either proposal's IPR/contract annex. It is the **shared** posture; the per-tender Background-IP
table is the existing `13503-quantum-pnt/BIPR-declaration.md` (the canonical declaration — adapt it
to each prime's template), and the ESA prior-authorisation *route specifics* are in
`bipr-prior-auth-route.md` in this folder.

**Date: 2026-06-16 · Contact: contact@ashforde.org**

> **Cross-reference, not duplication.** The mandatory Background-IP declaration text lives in
> [`../13503-quantum-pnt/BIPR-declaration.md`](../13503-quantum-pnt/BIPR-declaration.md) (re-usable
> for 13494 by swapping the tender reference). This pack adds: (1) the one-line shared posture for
> both tenders, (2) the **no-GPL/no-copyleft guarantee** with its evidence chain, and (3) the
> **Background ↔ Foreground boundary** stated as a clean contractual line. For the component/repo/tag/
> licence inventory and the explicit prior-authorisation request, see `bipr-prior-auth-route.md`.

---

## 1. The shared posture (one line, both tenders)

> Ashforde brings **Kshana** (open-core PNT-performance-and-resilience engine, **dual-licensed —
> GNU AGPL-3.0-only or a commercial licence from Ashforde OÜ**) as **Background IP** used as the
> analysis *tool*; the contract-funded **results** (trade reports, FoM tables, scenario bundles, any
> device-specific digital twin) are **Foreground IP** governed by the contract's IPR clauses, with
> ESA receiving the usage rights the contract specifies. Kshana enters the activity under the
> **commercial licence**, under which **no copyleft attaches** to ESA's or the prime's use; the
> AGPL-3.0-only governs only the separate open-source track. *(Commercial-licence grant terms to be
> confirmed by counsel.)*

This is identical for both AO/1-13494 and AO/1-13503; only the tender reference and the per-tender
WP/result wording change.

---

## 2. The mandatory Background-IP table (where it lives, how to use it)

The mandatory background-IP declaration table is **already authored** in
[`../13503-quantum-pnt/BIPR-declaration.md`](../13503-quantum-pnt/BIPR-declaration.md):

| Item | Owner | Licence | Nature |
|------|-------|---------|--------|
| **Kshana** — open-core PNT-performance-and-resilience simulator (engine) | Ashforde OÜ | **AGPL-3.0-only + commercial** (dual-licensed) | Software / simulation substrate used to perform the navigation-performance & trade WP |
| **Kshana Pro** — private contract-extensible overlay | Ashforde OÜ | Proprietary (NDA) | Optional; only relevant post-award (digital-twin Foreground IP). Not required to perform Phase 0/A |

**To use for 13494:** copy that file's §1–§5, change the title/reference to AO/1-13494/26/NL/RR, and
keep the table identical (Kshana is the same Background IP for both). **Do not re-state it here** —
this pack points at it.

---

## 3. No-GPL / no-copyleft guarantee (the contamination-risk answer)

ESA contracts guard against open-source contamination. Kshana's posture removes that risk:

- **Permissive-only dependency tree.** Every transitive dependency is restricted to a permissive
  allow-list — **Apache-2.0, MIT, BSD-2/3-Clause, ISC, Unicode, Zlib, CC0**. Copyleft licences
  (GPL/LGPL/AGPL/MPL/EPL/CDDL) are **rejected at build time**.
- **Enforced in CI by `cargo-deny`** (`deny.toml` allow-list). A copyleft dependency entering the
  tree **fails the build** — the guarantee is machine-enforced, not a promise.
- **CycloneDX SBOM + build provenance** accompany releases, so the exact open-source content used is
  **declarable and verifiable** by the prime and by ESA.

This gives the prime a one-line, evidence-backed answer to the IPR annex: *"the SME's background
engine is brought in under a commercial licence (no copyleft attaches), and its dependency tree is
CI-enforced permissive-only with an SBOM — no copyleft anywhere."* *(Commercial-licence grant terms
to be confirmed by counsel.)* The component/repo/tag/licence inventory that backs this is in
`bipr-prior-auth-route.md`.

---

## 4. Background ↔ Foreground boundary (the clean contractual line)

| | **Background IP (stays Ashforde's; AGPL-3.0 + commercial)** | **Foreground IP (created under the contract)** |
|---|---|---|
| **What** | The Kshana engine: the simulation/analysis code and its validated/modelled models | The study *results*: trade reports, FoM tables, scenario bundles, methodology write-ups; any customer/device-specific digital twin or custom pack |
| **Ownership** | Ashforde OÜ | Per the contract's IPR clauses; Ashforde retains its Foreground per the agreed split, ESA gets the usage rights the contract specifies |
| **Licence to ESA/prime** | Commercial licence (use/run/modify/integrate/redistribute; no copyleft attaches) — AGPL-3.0-only available on the open track (patent grant via GPLv3 §11). *(Grant terms — counsel.)* | Contract IPR terms (e.g. GCC Part II), plus any export-control terms |
| **First Pro instance** | — | A **customer/device-specific digital twin** would be the first Kshana **Pro** Foreground-IP instance, delivered under the agreed NDA/IPR and any export-control terms — distinct from the open Background engine |

**The line to hold in the proposal:** Kshana-the-engine is **Background and stays Ashforde's —
dual-licensed (AGPL-3.0 + commercial) and brought into the activity under the commercial licence**;
**everything ESA funds is Foreground** and follows the contract. The Pro overlay is **not** required
to perform Phase 0/A — it is a post-award Foreground vehicle only.

---

## 5. Confirmations for either bid (checklist)

- [ ] Kshana licence = AGPL-3.0-only + commercial (confirmed; `LICENSE`, `LICENSING.md`).
- [ ] Commercial-licence grant terms for ESA / the prime confirmed by counsel.
- [ ] No GPL/copyleft dependencies (confirmed; `deny.toml` permissive allow-list, CI-enforced).
- [ ] CycloneDX SBOM available for the released engine version used.
- [ ] Background ↔ Foreground split agreed with the prime against the draft contract's IPR clauses.
- [ ] ESA open-source-use prior authorisation requested via the prime's BIPR route (see
      `bipr-prior-auth-route.md`).
- [ ] Per-tender Background-IP table copied from `13503-quantum-pnt/BIPR-declaration.md` and
      re-referenced for 13494.

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open-core PNT performance & resilience simulator (AGPL-3.0-only + commercial licence) ·
Background IP only; contract-funded results are Foreground · TRL ~3, no flight heritage, no certification,
no ESA endorsement. Prime/partner outreach is PARKED pending explicit founder go.*
