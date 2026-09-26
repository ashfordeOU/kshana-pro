# Kshana Pro — Commercial Licence Agreement (EULA)

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-23
**Review gate:** ⚠️ commercial/legal template — have counsel review before using in any signed order. Bracketed `[…]` items are deal- or counsel-set; do not rely on this as legal advice.

The end-user terms under which a licensee uses **Kshana Pro**, receives **PNT-Resilience
Study Reports**, or takes the open engine under commercial terms instead of the AGPL (§2A). It sits on top of [`IP-POLICY.md`](IP-POLICY.md) (IP ownership) and
[`SUPPORT-SLA.md`](SUPPORT-SLA.md) (support/warranty), and attaches to an order form / MSA.
Open Kshana is licensed to the public under AGPL-3.0. A licensee who wants it on commercial
terms instead ("Option B" in the open repo's `LICENSING.md`) takes it under §2A, which applies
only when the order form names the Engine.

---

## 1. Definitions
- **Engine** — the open Kshana core (AGPL-3.0).
- **Pro** — the proprietary Kshana Pro overlay and any deliverables produced with it.
- **Study Report** — a MODELLED PNT-resilience analysis deliverable (e.g. Pack A optimism-gap audit).
- **Tier** — Evaluation / Pro / Enterprise, as enforced by the `license` module.

## 2. Grant of licence
Subject to payment and these terms, Ashforde grants the licensee a **non-exclusive,
non-transferable, non-sublicensable** licence to use Pro and the Study Reports for the
licensee's internal business purposes, for the Tier and term stated in the order form.

## 2A. Commercial licence to the Engine (Option B) — DRAFT, pending counsel
> Added 2026-09-26 so that the public `LICENSING.md` Option B has text behind it. It is
> wording for counsel to mark up (`commercial/counsel/COMMERCIAL-LICENCE-FINALISATION.md`,
> issue 1), not agreed terms. Applies only when the order form names the Engine.

1. **Grant.** Subject to payment and these terms, Ashforde grants the licensee a
   non-exclusive, non-transferable licence, for the term and scope in the order form, to
   use, copy and modify the Engine version(s) named there, and to combine it with the
   licensee's own software, **without** the obligations of the GNU AGPL-3.0, including its
   §13 duty to offer source to network users.
2. **Distribution and hosting.** Where the order form permits it, the licensee may
   distribute the Engine, modified or not, in object code **as part of** its own product,
   and may run a modified Engine as part of a network service, in each case under the
   licensee's own terms to its end users. `[Source-code redistribution, standalone
   redistribution and sublicensing: counsel to set.]`
3. **Separation from Pro.** The Pro restrictions in §3 do not apply to the Engine except
   the marking and trademark restrictions. Hosting the Engine as part of a service, where
   permitted by (2), is not a breach of §3.
4. **Notices.** The licensee keeps Ashforde's copyright notice and the third-party
   dependency notices with every distributed copy.
5. **No AGPL obligations, no AGPL rights.** Copies taken under this section are not
   received under the AGPL. Any warranty or indemnity for the Engine comes only from §7
   and §8 of this agreement (and `SUPPORT-SLA.md`), not from the AGPL.
6. **Public versions stay public.** Nothing here withdraws the AGPL from anyone else, and
   the licensee may also use any public version under the AGPL on the AGPL's terms.
7. **Trademarks.** As in §5: no right to the "Kshana" name or marks beyond identifying
   that the product embeds the Engine, in the form set in the order form.
8. **Export.** §10 applies. The Engine as published is public; any non-public build,
   scenario or tailored work supplied under this section is subject to the export-control
   determination first.

## 3. Restrictions
The licensee shall not: redistribute, sublicense, resell, or host Pro as a service to third
parties; reverse-engineer the proprietary overlay; remove or alter the MODELLED / "not a
certification" / provenance markings or the trademark notices; or represent a Study Report
as a certification, peer-reviewed result, or guarantee of field performance.

## 4. Deliverables & nature of the work
Study Reports are **MODELLED decision-support evidence**, reproducible from scenario + seed +
engine version. They are **not** a certification, not a warranty of real-world performance,
and not regulatory approval. The licensee remains responsible for its own verification,
validation, and certification. Published methodology cited in a report (e.g. an arXiv
preprint) is **not peer-reviewed** unless explicitly stated.

## 5. Intellectual property
Background and Foreground IP follow [`IP-POLICY.md`](IP-POLICY.md). The licensee owns its
input data and the specific report instance delivered to it; Ashforde retains all rights in
the Engine, the Pro overlay, methods, and templates. "Kshana" and associated marks are
trademarks of Ashforde OÜ; no trademark licence is granted beyond identifying the deliverable.

## 6. Fees & payment
Per the order form / [`PRICING.md`](PRICING.md). `[payment terms — e.g. net 30]`. Taxes
exclusive. Non-payment is a material breach (§9).

## 7. Warranties & disclaimer
Ashforde warrants the deliverables will be produced with reasonable professional skill and
care. **EXCEPT AS EXPRESSLY STATED, PRO AND THE STUDY REPORTS ARE PROVIDED "AS IS"**, without
implied warranties of merchantability or fitness for a particular purpose. Open Kshana is
provided under its own (AGPL) terms with no warranty.

## 8. Liability
Subject to counsel: liability cap `[e.g. fees paid in the preceding 12 months]`; exclusion of
indirect/consequential loss; standard carve-outs (death/personal injury, fraud, IP indemnity,
confidentiality breach) — align with [`SUPPORT-SLA.md`](SUPPORT-SLA.md) §5 and the MSA.

## 9. Term & termination
Term per the order form. Either party may terminate for uncured material breach `[30 days]`.
On termination the Pro licence ends and Pro materials must stop being used; delivered Study
Reports already paid for survive for the licensee's internal use; data handling on
termination follows [`DATA-PROCESSING-AGREEMENT.md`](DATA-PROCESSING-AGREEMENT.md).

## 10. Confidentiality, export, governing law
Confidentiality per the executed NDA ([`NDA-TEMPLATE-MUTUAL.md`](NDA-TEMPLATE-MUTUAL.md)).
Export-controlled work follows [`EXPORT-CONTROL-ASSESSMENT.md`](EXPORT-CONTROL-ASSESSMENT.md)
and requires the appropriate clearance. Governing law / jurisdiction: `[Estonia or Germany —
align with the MSA]`.

---
**Contact:** [contact@ashforde.org](mailto:contact@ashforde.org) · Ashforde OÜ
*Open issues for counsel: §2A Engine grant (scope of redistribution and sublicensing, and
whether inbound contribution terms support relicensing); liability cap & carve-outs; governing law (EE vs DE); consumer-law
exclusions (B2B only); interaction with the MSA as the master contract.*
