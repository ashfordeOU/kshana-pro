<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Per-prime tailored capability-brief TEMPLATE (dominance-plan step A13).
OUTREACH PARKED: prime SELECTION and any contact stay gated until the founder explicitly says go.
This file is a fill-in template — replace every {{PLACEHOLDER}} before use. Do NOT populate a real prime
name speculatively; rendering a brief is a drafting step, sending it is a separate, founder-gated step.
Contact: contact@ashforde.org
-->

# Kshana — capability brief for {{PRIME_NAME}}
### {{TENDER_NAME}} ({{TENDER_REF}})

**PRIVATE — Ashforde OÜ internal working document.** Prime-facing leave-behind once the founder unparks outreach. Not for external distribution.
**Date: {{DATE}} · Contact: contact@ashforde.org**

> RENDER NOTE (delete before sending): outreach is PARKED. Producing this brief does not authorise sending it.
> Fill every `{{PLACEHOLDER}}`, choose the right archetype block where marked, and delete all RENDER NOTEs.

---

## 1. One honest identity

Kshana is **{{ONE_LINE_HONEST_IDENTITY}}**. Ashforde OÜ (sole founder, Chakshu Baweja) offers your consortium a single, tightly-scoped SME work-package seat — **{{WP_WE_OWN_SHORT}}** — and nothing more.

We bid strictly as a **consortium-SME work-package contributor — never prime, never a mandated Key Person.** Kshana is an open Apache-2.0 Rust core ({{TEST_COUNT}} library tests, fmt + clippy clean, adversarially reviewed) that {{PRIME_NAME}} can audit line-by-line. **TRL ~3** software-model substrate. **No flight heritage, no agency contracts, no certification, no ESA endorsement** — stated plainly.

---

## 2. The work-package we own (and only this)

{{WP_DESCRIPTION_PARAGRAPH}}

- **WP scope (in):** {{WP_IN_SCOPE_BULLETS}}
- **WP scope (out, partner/prime-owned):** {{WP_OUT_OF_SCOPE_BULLETS}}
- **Deliverables it feeds:** {{DELIVERABLES_IT_FEEDS}}
- **Clean interfaces:** {{INTERFACE_INPUTS_OUTPUTS}}

---

## 3. Score-delta sheet — what an Ashforde seat does to your scored proposal

Mapped to the FutureNAV evaluation weights **20 / 30 / 30 / 10 / 10** (Background · Understanding · Quality · Management/Costing · Admin compliance).

| Weight | Criterion | What an Ashforde WP adds | Direction |
|---|---|---|---|
| 20% | Background & experience | {{SCORE_20_BACKGROUND}} | ↑ |
| 30% | Understanding of requirements & objectives | {{SCORE_30_UNDERSTANDING}} | ↑ |
| 30% | Quality & suitability of proposed programme | {{SCORE_30_QUALITY}} | ↑ |
| 10% | Management, costing, planning | {{SCORE_10_MANAGEMENT}} | ↑ |
| 10% | Compliance with administrative tender conditions | {{SCORE_10_ADMIN}} | ↑ |

**Consortium-composition arithmetic (scored, not a KAF).** Adding Ashforde mechanically moves the numbers the prime must hit anyway:
- A **German-nationality SME** (ESA-STAR entity {{ESA_STAR_ENTITY_ID}}) → one of the required **≥3 SMEs**, and budget share toward the **≥30%-to-non-LSI** quota.
- Feeds the **DE-weighted geographical return** ({{GEO_RETURN_NOTE}}).
- A **low-friction, fixed-scope, cap-compliant** WP that does not have to be re-costed.

**Open registered-office item (disclosed, not hidden).** Ashforde's registered office is currently in Estonia (not a participating state). Light ESA-STAR registration suffices at submission; a participating-state registered office is required **by award**. Fix in flight: a German branch/subsidiary plus an ESA Q&A clarification. Flagged up front so no rival raises it first.

---

## 4. The audit-the-core hook (why us over the black-box incumbents)

{{AUDIT_THE_CORE_HOOK}}

The structural differentiator: a **unit-tested verification matrix that refuses to mark any capability "validated" without an independent external oracle** (`src/verification.rs`), sitting on an **open Apache-2.0 core {{PRIME_NAME}} can read line-by-line** — versus the closed simulators in this space ({{INCUMBENT_LIST}}). For a study whose credibility rests on *defensible* numbers, a tool that publishes its own validated-vs-modelled boundary and its own optimism is an evidence asset, not a feature.

---

## 5. Validated-or-modelled capability table

Every claim is tagged **validated** (checked against an independent external oracle) or **modelled** (published / first-principles physics with tests, no external oracle yet). This labelling is enforced in code by the verification matrix — a row cannot be marked validated on a self-consistency check.

| # | Capability (mapped to this tender) | Kshana module | Status | Oracle |
|---|---|---|---|---|
{{CAPABILITY_TABLE_ROWS}}

> RENDER NOTE: pull rows from the open repo `docs/CAPABILITY.md` + `src/verification.rs`. Use the
> archetype blocks below as the starting set. NEVER promote a row to "validated" beyond what the matrix allows.
> The five externally-validated islands are: SGP4/SDP4 (AIAA 2006-6753, 4.12 mm) · reference frames &
> timescales (SOFA/ERFA + Vallado, 0.1–4 mm) · Allan/ADEV (NIST SP 1065 / Stable32 on NBS14, 1e-4) ·
> integrity RAIM/ARAIM (DO-229E/DO-316 + real IGS SP3 / Celestrak TLE) · ranging-code Gold trade
> (published GPS C/A −23.9 dB). Everything else is modelled (incl. the AI-evaluation testbed's harness
> logic, the quantum-trade engine, the CAI accelerometer, GNSS-denied resilience, alt-PNT). The four
> partner-owned hardware/PA gaps (bus · RF-payload/antenna · quantum-payload HW · flight PA) carry NO claim.

---

## 6. BIPR / IP line (clean, declarable)

- **Background IPR:** Kshana — open **Apache-2.0** PNT-performance-and-resilience engine ({{ENGINE_VERSION}}), owned by Ashforde OÜ. Used as the analysis tool; the **results** (trade reports, FoM tables, scenario bundles) are the deliverables and follow the contract's Foreground-IPR terms.
- **No copyleft anywhere in the dependency tree** — permissive allow-list (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0), enforced in CI by `cargo-deny` (`deny.toml`); avoids the open-source contamination risk ESA contracts guard against.
- **Apache-2.0 prior-authorisation note:** open-source use as a tool may require **ESA prior authorisation** under GCC Part II (Option A); declarable via the prime's BIPR/BIPR-table route. Ashforde supplies the open-source-use declaration (component + repo + release tag + permissive SBOM + no-GPL confirmation).
- **Foreground / Pro overlay:** a customer/device-specific digital twin would be the first **Kshana Pro** Foreground-IP instance, delivered under the agreed NDA/IPR and any export-control terms — distinct from the Apache-2.0 background engine.

---

## 7. What this offer is — and is not

**Is:** a defined, auditable, fixed-scope SME work-package — {{WP_WE_OWN_SHORT}} — ready for {{PRIME_NAME}} to fold into a {{TENDER_REF}} consortium proposal.

**Is not:** a prime, a Key Person, the whole system, or any partner-owned hardware/ground/platform element. Ashforde does not satisfy the consortium or Key-Personnel mandates alone and does not claim to.

*Prime/partner outreach is **PARKED** pending explicit founder go. This brief defines the asset only; producing it does not authorise contacting anyone.*

---
*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator · capability claims labelled validated (external oracle) or modelled throughout · TRL ~3, no flight heritage, no certification, no ESA endorsement.*

<!--
========================================================================================
PLACEHOLDER SCHEMA (fill every one; leave none in a rendered brief)
----------------------------------------------------------------------------------------
{{PRIME_NAME}}               Target prime / partner organisation name. PARKED: do not populate a real
                             name until the founder unparks selection. Use an ARCHETYPE label otherwise.
{{TENDER_NAME}}              Human tender title, e.g. "Quantum-Enabled PNT Demonstrator (FutureNAV C3)".
{{TENDER_REF}}               ESA reference, e.g. "AO/1-13503/26/NL/RR".
{{DATE}}                     Render date, ISO (YYYY-MM-DD).
{{ONE_LINE_HONEST_IDENTITY}} One honest line on what Kshana is for this tender/WP.
{{WP_WE_OWN_SHORT}}          Short name of the single WP we own (e.g. "the navigation-performance & quantum-vs-classical trade WP").
{{WP_DESCRIPTION_PARAGRAPH}} One paragraph describing the WP we own for this tender.
{{WP_IN_SCOPE_BULLETS}}      Bullet list — what is inside our WP.
{{WP_OUT_OF_SCOPE_BULLETS}}  Bullet list — partner/prime-owned items we explicitly do NOT bid.
{{DELIVERABLES_IT_FEEDS}}    The SoW/PARD deliverable IDs our WP feeds.
{{INTERFACE_INPUTS_OUTPUTS}} The clean inputs/outputs at the WP boundary.
{{SCORE_20_BACKGROUND}}      Score-delta sentence for the 20% Background criterion.
{{SCORE_30_UNDERSTANDING}}   Score-delta sentence for the 30% Understanding criterion.
{{SCORE_30_QUALITY}}         Score-delta sentence for the 30% Quality criterion.
{{SCORE_10_MANAGEMENT}}      Score-delta sentence for the 10% Management/Costing criterion.
{{SCORE_10_ADMIN}}           Score-delta sentence for the 10% Admin-compliance criterion.
{{ESA_STAR_ENTITY_ID}}       ESA-STAR entity id (held in private notes; do not hardcode in any public-bound file).
{{GEO_RETURN_NOTE}}          Tender-specific DE geo-return note (e.g. "DE is top return at 48.89% on 13503").
{{AUDIT_THE_CORE_HOOK}}      Paragraph on the auditable-open-core hook for this tender.
{{INCUMBENT_LIST}}           The closed-simulator incumbents this WP competes with (honest, code-backed claim only).
{{CAPABILITY_TABLE_ROWS}}    The validated-or-modelled rows for this tender's WP (pull from CAPABILITY.md / verification.rs).
{{ENGINE_VERSION}}           Kshana engine version (e.g. v0.17.0).
{{TEST_COUNT}}               Library test count at render time (e.g. 913+).
========================================================================================
-->
