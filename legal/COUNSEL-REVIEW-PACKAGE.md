# Kshana Pro — Counsel-Review Package

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

> **This is an organizing brief for counsel — it is NOT legal advice.** It assembles the
> Tier-1 commercial/legal drafts in this repository, records each draft's status and its
> embedded "open items for counsel," and consolidates the questions qualified counsel must
> resolve. **Nothing in `legal/` is binding, final, or fit for signature until it has been
> reviewed by qualified counsel and executed by the parties.** All bracketed `[…]` items in
> the drafts are deal- or counsel-set placeholders.

---

## 1. Purpose & scope

Ashforde OÜ intends to sign its first paid engagement (a **Pack A — Optimism-Gap Audit**
Study Report). The goal of this package is to get the **Tier-1 commercial stack reviewed
before that first signature**, so that the licence, SOW, DPA, support/SLA, IP, NDA, export,
and trademark posture are coherent and defensible. This brief organizes the drafts and the
open questions; it does not draft, opine, or decide.

## 2. Entity & context summary (for counsel)

- **Contracting entity:** Ashforde OÜ, registered in **Estonia**.
- **Operative business nationality:** For EU/ESA purposes the operative nationality has been
  determined as **Germany (DE)** (ESA-STAR entity ID 83208). The entity is therefore split
  between its **Estonian registration** and its **German operative footprint**. This makes
  **governing law / jurisdiction for customer contracts an open question counsel must settle**
  (Estonia vs Germany vs the customer's jurisdiction).
- **Nature of the deliverable:** Study Reports are **MODELLED decision-support evidence**,
  reproducible from scenario hash + seed + engine version. They are explicitly **"not a
  certification,"** not a warranty of real-world performance, and not regulatory approval. The
  customer remains responsible for its own verification, validation, and certification.
- **Honesty framing:** Any cited methodology (e.g. the optimism-gap result, arXiv:2606.22054)
  is an **arXiv preprint and is not peer-reviewed**. There are no TRL, heritage,
  ESA-endorsement, or certification claims.
- **Customer-data / GDPR exposure:** Customers may send their own inputs (e.g. detector
  scores/labels, device/clock specs, mission/threat parameters). This is engineering data and
  is **usually not personal data**, but it triggers DPA / data-handling obligations and must be
  confirmed against GDPR.

## 3. Document inventory

All files are **working drafts** unless noted. "Blocking" means it must be reviewed before the
first paid signature; "Later" means it can follow before scaling or for specific motions (ESA
bids, L2 export work).

| File (`legal/`) | One-line purpose | Status | Gate |
|---|---|---|---|
| `COMMERCIAL-LICENSE.md` | End-user licence (EULA) for Pro + Study Reports; attaches to order form / MSA | working draft | **BLOCKING** |
| `PRICING.md` | Commercial structure / rate card (all figures are founder-set placeholders) | working draft — structure only | **BLOCKING** (commercial terms must be coherent at signature; founder also to set figures) |
| `DATA-PROCESSING-AGREEMENT.md` | How customer inputs are handled (GDPR + export overlap) | working draft | **BLOCKING** |
| `SOW-TEMPLATE.md` | Scope contract for one Study Report engagement (e.g. Pack A) | working draft template | **BLOCKING** |
| `SUPPORT-SLA.md` | Support tiers, SLA, warranty, indemnification, liability cap | commercial template (PR23) | **BLOCKING** |
| `IP-POLICY.md` | Background/Foreground IP split; open-vs-Pro boundary; trademark; attribution | working draft (PR4) | **BLOCKING** |
| `NDA-TEMPLATE-MUTUAL.md` | Mutual NDA for consortium / bid discussions | draft template (PR4) | **BLOCKING** (if any pre-signature disclosure happens) |
| `NDA-TEMPLATE-ONEWAY.md` | One-way NDA where Ashforde receives partner/device data | draft template (PR4) | **BLOCKING** (if customer/partner data is received before signature) |
| `EXPORT-CONTROL-ASSESSMENT.md` | Dual-use self-screen; mandatory gate before L2 resilience work | working self-assessment (PR5) | LATER (BLOCKING before any Pack B / L2 work) |
| `TRADEMARK-FILING-BRIEF.md` | "Kshana" filing prep + same-name coexistence risk | filing brief (PR3) | LATER (does not block first signature; address before filing / scaling) |
| `BIPR-DECLARATION-TEMPLATE.md` | Background-IPR declaration for ESA bids | draft template (PR6) | LATER (only for ESA bids) |
| **MSA (Master Services Agreement)** | **MISSING — no template exists** | **GAP** | **BLOCKING (recommend counsel produce one)** |

**Gap flag:** There is currently **no Master Services Agreement (MSA) template.** Several
drafts (`COMMERCIAL-LICENSE.md`, `SOW-TEMPLATE.md`, `SUPPORT-SLA.md`, `DATA-PROCESSING-AGREEMENT.md`)
assume an MSA exists as the master contract that the SOW attaches to and that carries the
caps, carve-outs, and governing-law clauses. Counsel should produce an MSA (or confirm an
order-form-only structure) before the first signature.

## 4. Consolidated open questions (for counsel)

This section pulls together each draft's embedded "open items for counsel" plus the
cross-cutting questions, grouped by theme. File references in parentheses point counsel to the
relevant draft.

### 4.1 Governing law, jurisdiction & dispute resolution
- Settle **governing law / jurisdiction for customer contracts**: Estonia (registration) vs
  Germany (operative nationality) vs the customer's jurisdiction.
  (`COMMERCIAL-LICENSE.md` §10; `SUPPORT-SLA.md` §7; `IP-POLICY.md` §7; both NDA templates §9.)
- Choose a **dispute-resolution mechanism** (courts vs arbitration; venue; language).
- Confirm whether the EE-vs-DE split affects **Foreground IP ownership treatment**
  (`IP-POLICY.md` §7) and any consumer-law exclusions (intended B2B-only).

### 4.2 Liability, warranties & the MODELLED / "not a certification" disclaimer
- Confirm the **liability cap and carve-outs** and align them across the EULA, the SLA, and the
  (to-be-created) MSA. Current drafts propose a cap of **fees paid in the preceding 12 months**,
  exclusion of indirect/consequential loss, and standard carve-outs (IP indemnity,
  confidentiality, export). (`COMMERCIAL-LICENSE.md` §8; `SUPPORT-SLA.md` §6.)
- Confirm the **warranty disclaimer is watertight**: deliverables "as is" except for
  reasonable professional skill and care; **no implied warranty of merchantability or fitness
  for a particular purpose**; the MODELLED / "not a certification" framing creates **no implied
  performance or fitness warranty** and is enforceable in the chosen jurisdiction.
  (`COMMERCIAL-LICENSE.md` §4, §7; `SUPPORT-SLA.md` §4.)
- Confirm **acceptance / deemed-acceptance wording** and **change-order mechanics** in the SOW.
  (`SOW-TEMPLATE.md` §7, §10, open-items note.)
- Confirm warranty/indemnity interaction with the embedded open engine's "as is" open-source
  terms (note: see the licence discrepancy in §4.8). (`SUPPORT-SLA.md` §7 open items.)

### 4.3 Intellectual property (the ownership split)
- Confirm the **IP split**: the **customer owns its inputs and the specific delivered report
  instance**; **Ashforde retains the Engine, the Pro overlay, methods, and templates** as
  Background/Foreground IP. (`IP-POLICY.md` §1–§2; `COMMERCIAL-LICENSE.md` §5; `SOW-TEMPLATE.md` §9.)
- Confirm the **Foreground IP default** (Ashforde owns Foreground; funder/customer gets scoped
  usage rights) holds under both EE/DE treatment and ESA R&D IPR terms.
  (`IP-POLICY.md` §2, §7; `BIPR-DECLARATION-TEMPLATE.md` §C.)
- Decide the **CLA/DCO posture** for the open repo before the first sizeable external PR.
  (`IP-POLICY.md` §7.)

### 4.4 Data protection / GDPR
- Confirm **DPA adequacy** under GDPR and alignment with the NDA, MSA, and export-control access
  controls. (`DATA-PROCESSING-AGREEMENT.md` open items.)
- Set **data residency / processing location** (EU — Estonia/Germany; to specify) and the
  **sub-processor list** (or "none") with flow-down. (`DATA-PROCESSING-AGREEMENT.md` §3.)
- Set **retention windows** and deletion timelines (reproducibility is preserved via scenario
  hash + seed + engine version, not by retaining raw inputs). (`DATA-PROCESSING-AGREEMENT.md` §4.)
- Confirm the **controller/processor classification** and that **customer detector data is
  typically NOT personal data** (with terms for the incidental case).
  (`DATA-PROCESSING-AGREEMENT.md` §7.)

### 4.5 Export control (dual-use)
- Confirm **export-control flow-down clauses** in the customer contracts, especially for the
  future **Pack B (timing-resilience / L2)** work, where inputs/outputs are treated as
  **controlled until classified**. Cross-reference **`EXPORT-CONTROL-ASSESSMENT.md`** and its
  **determination questionnaire / per-item screen** (§2 item-by-item table, §4 actions
  checklist).
- Confirm the regime: per the DE operative nationality, **EU Dual-Use Reg (EU) 2021/821** as
  implemented in Germany (AWV), administered by **BAFA**; intangible technology transfer is in
  scope. (`EXPORT-CONTROL-ASSESSMENT.md` §1.)
- **No L2 work begins** until (a) this assessment is counsel-reviewed, (b) a per-item
  classification is recorded, and (c) the customer's end-use/end-user is screened.
  (`EXPORT-CONTROL-ASSESSMENT.md` §3–§4.) Confirm there is **no indemnity for customer
  export-control breaches.** (`SUPPORT-SLA.md` §5.)

### 4.6 Trademark
- Address the **"Kshana" coexistence risk**: a same-name software company (Kshana Inc. /
  kshana.ai — a fintech, different field but also software) exists, creating potential overlap
  in **Class 9 (software)** and **Class 42 (SaaS)**. A **formal clearance search** and an
  **attorney coexistence opinion** are required before filing; EU-first (EUIPO) with PNT-tight
  goods/services wording is the proposed posture. Confirm classes and jurisdictions.
  (`TRADEMARK-FILING-BRIEF.md` KEY RISK box, §6, §3; `IP-POLICY.md` §7.) Does not block the
  first signature, but should be resolved before scaling / public filing.

### 4.7 The missing MSA
- **Recommend counsel produce a Master Services Agreement (MSA)** that the **SOW attaches to**,
  and that carries the liability caps/carve-outs, governing-law, warranty, indemnity, and
  confidentiality/export flow-down — so the per-engagement SOW stays a thin scope document.
  Several drafts already reference "the MSA" as the master contract.
  (`COMMERCIAL-LICENSE.md` §8, §10; `SUPPORT-SLA.md` §6, §7; `SOW-TEMPLATE.md` open items;
  `DATA-PROCESSING-AGREEMENT.md` §7.)

### 4.8 Open-source / dual-license interaction
- The commercial offering is built on an **open core licensed open-source-OR-commercial**, with
  the proprietary Pro overlay carved out. Confirm the **commercial licence cleanly carves the
  proprietary overlay** from the open-core terms and that the dual-license model is internally
  consistent. (`COMMERCIAL-LICENSE.md` §1, §5; `IP-POLICY.md` §1–§3.)
- **RESOLVED — the open core is AGPL-3.0-only, dual-licensed with a commercial licence.** The
  earlier inconsistency (some drafts said Apache-2.0, others AGPL-3.0) has been resolved against the
  live source of truth: the open `kshana` engine was relicensed **Apache-2.0 → AGPL-3.0-only +
  commercial dual-licence at v0.19.0** (current v0.20.0); the repo `LICENSE` is the GNU AGPL-3.0,
  `Cargo.toml` carries `license = "AGPL-3.0-only"`, and `LICENSING.md` states the dual-licence terms.
  The Apache-2.0 references across the bid + legal drafts (`IP-POLICY.md`, `SUPPORT-SLA.md`,
  `BIPR-DECLARATION-TEMPLATE.md`, the BIPR pack/declaration/route, the tender briefs) have been
  updated to the **AGPL-3.0-only + commercial** framing, with ESA/the prime routed to the
  **commercial licence** so no copyleft attaches. **Remaining for counsel:** confirm the
  **commercial-licence grant terms** (the basis on which the engine enters ESA contracts copyleft-free
  and the basis of the §2A "primary route" in the BIPR prior-auth doc), that the commercial licence
  cleanly carves the proprietary overlay from the open-core terms, and the inbound CLA/DCO dual-licence
  grant for external contributions (`IP-POLICY.md` §3, §7). NB: AGPL-3.0 is network copyleft — the
  "no copyleft for ESA/the prime" position now rests on the commercial licence, not on the open licence.

## 5. Prioritized checklists

### 5.1 BEFORE FIRST SIGNATURE (blocking subset)
- [ ] Settle **governing law / jurisdiction** (EE vs DE vs customer) and dispute resolution (§4.1).
- [ ] Confirm **liability cap, carve-outs, and warranty disclaimers**; verify the MODELLED /
      "not a certification" disclaimer is watertight (no implied fitness/performance warranty) (§4.2).
- [ ] Confirm the **IP ownership split** — customer owns inputs + report instance; Ashforde
      retains engine/overlay/methods/templates (§4.3).
- [ ] Confirm **DPA adequacy**, residency, sub-processors, retention, and the personal-data
      classification (§4.4).
- [ ] **Confirm the commercial-licence grant terms** for the AGPL-3.0-only open core — the
      open-core licence discrepancy is resolved (open core = AGPL-3.0-only + commercial); confirm the
      commercial licence cleanly carves the proprietary overlay and underwrites the copyleft-free
      "primary route" for ESA/the prime (§4.8).
- [ ] **Produce / confirm the MSA** as the master contract the SOW attaches to (§4.7).
- [ ] Review the **EULA, PRICING, DPA, SOW, SUPPORT-SLA, IP-POLICY, and the applicable NDA**
      for internal consistency and cross-references.
- [ ] (Founder action, parallel) Set every `[€PLACEHOLDER]` in `PRICING.md` deliberately before quoting.

### 5.2 BEFORE SCALING (later)
- [ ] Counsel-review **`EXPORT-CONTROL-ASSESSMENT.md`** and complete per-item classification +
      BAFA ruling where uncertain — **mandatory before any Pack B / L2 work** (§4.5).
- [ ] Run the **formal trademark clearance search** and obtain a **coexistence opinion** for the
      "Kshana" mark in Classes 9/42; decide jurisdictions; then file (§4.6).
- [ ] Finalize the **`BIPR-DECLARATION-TEMPLATE.md`** against a specific ESA ITT's IPR Annex
      before any ESA submission (§4.3).
- [ ] Decide the **CLA/DCO posture** for the open repo before the first sizeable external PR (§4.3).

## 6. What to send counsel (and in what order)

Send the drafts in this order, with this brief on top:

1. `legal/COUNSEL-REVIEW-PACKAGE.md` (this brief — read first).
2. `legal/IP-POLICY.md` (sets the IP and open/closed boundary the rest depends on).
3. `legal/COMMERCIAL-LICENSE.md` (the EULA).
4. `legal/SUPPORT-SLA.md` (warranty, indemnity, liability cap).
5. `legal/DATA-PROCESSING-AGREEMENT.md` (GDPR / data handling).
6. `legal/SOW-TEMPLATE.md` and `legal/PRICING.md` (the per-engagement contract + commercial terms).
7. `legal/NDA-TEMPLATE-MUTUAL.md` and `legal/NDA-TEMPLATE-ONEWAY.md` (confidentiality).
8. `legal/EXPORT-CONTROL-ASSESSMENT.md` (dual-use; gate for Pack B / L2).
9. `legal/TRADEMARK-FILING-BRIEF.md` and `legal/BIPR-DECLARATION-TEMPLATE.md` (later motions).
10. **Request:** an **MSA template** the SOW can attach to (no draft exists yet).

**Contact:** [contact@ashforde.org](mailto:contact@ashforde.org) · Ashforde OÜ
