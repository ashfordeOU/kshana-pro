# Kshana Pro — Statement of Work (SOW) Template

**Owner:** Ashforde OÜ · **Status:** working draft template · **Date:** 2026-06-23
**Review gate:** ⚠️ commercial template — have counsel review before signing. Bracketed `[…]`
items are filled per engagement. This SOW attaches to the
[`COMMERCIAL-LICENSE.md`](COMMERCIAL-LICENSE.md), [`SUPPORT-SLA.md`](SUPPORT-SLA.md),
[`DATA-PROCESSING-AGREEMENT.md`](DATA-PROCESSING-AGREEMENT.md), and the NDA.

The scope contract for a single Study Report engagement (e.g. a Pack A Optimism-Gap Audit).

---

## 1. Parties & engagement
**Provider:** Ashforde OÜ. **Client:** `[name]`. **Effective date:** `[date]`.
**Study:** `[e.g. Pack A — Optimism-Gap Audit]`. **Tier:** `[Pro / Enterprise]`.

## 2. Objective
`[e.g. Quantify the optimism gap (in-distribution vs out-of-distribution AUC) of the Client's
GNSS interference detector, as MODELLED decision-support for the Client's own V&V.]`

## 3. Scope
**In scope:** `[the named pack run, the inputs processed, the analyses included]`.
**Out of scope:** certification; guarantee of field performance; real-hardware testing;
anything not listed. The study is **MODELLED and is not a certification**.

## 4. Client inputs
Per [`DATA-PROCESSING-AGREEMENT.md`](DATA-PROCESSING-AGREEMENT.md) and the relevant intake
format (e.g. [`../docs/packs/DETECTOR-INTAKE-FORMAT.md`](../docs/packs/DETECTOR-INTAKE-FORMAT.md)):
`[detector scores + labels + distribution tags / device specs / threat parameters]`, due by
`[date]`. Delays shift the timeline.

## 5. Deliverables
1. A branded **PNT-Resilience Study Report** (HTML + PDF) — headline result, per-figure
   VALIDATED/MODELLED labels, provenance block, MODELLED + "not a certification" banner.
2. The machine-readable **evidence** (result JSON) + the **scenario hash + seed + engine
   version** for reproducibility.
3. `[optional: a short readout call]`.

## 6. Timeline & milestones
Inputs received `[T0]` → draft report `[T0 + N business days]` → final `[+ M days]`.

## 7. Acceptance criteria
Accepted when the report is delivered in the agreed formats, reproduces from the supplied
scenario hash + seed + engine version, and contains the agreed analyses. Acceptance window
`[N days]`; deemed accepted if no written rejection within it.

## 8. Fees
Per [`PRICING.md`](PRICING.md) / the order form: `[€ / day-rate + estimate]`. `[payment terms]`.

## 9. IP & confidentiality
Per [`IP-POLICY.md`](IP-POLICY.md) and the [`COMMERCIAL-LICENSE.md`](COMMERCIAL-LICENSE.md):
Client owns its inputs and the delivered report instance; Ashforde retains the engine, overlay,
methods, and templates. Confidentiality per the NDA.

## 10. Change control
Scope changes by written change order (impact on fee/timeline noted). No oral changes.

---
**Provider:** Ashforde OÜ — `[signatory]` · **Client:** `[signatory]`
**Contact:** [contact@ashforde.org](mailto:contact@ashforde.org)
*Open items for counsel: acceptance/deemed-acceptance wording; liability flow-down to the MSA;
change-order mechanics.*
