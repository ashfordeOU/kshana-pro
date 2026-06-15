# Kshana Pro — Support, SLA, Warranty & Indemnification (PR23)

**Owner:** Ashforde OÜ · **Status:** commercial template (PR23) · **Date:** 2026-06-15
**Review gate:** ⚠️ commercial/legal template — have counsel review before using in a signed order form or MSA. Figures (response times, caps) are placeholders to set per deal.

This defines the support tiers, service levels, warranty, and indemnification that
attach to a Kshana Pro licence (the recurring-revenue wrapper around the
licence/entitlement mechanics in [`IP-POLICY.md`](IP-POLICY.md) and the
`license` module).

---

## 1. Support tiers

Support maps to the licence tier (Evaluation / Pro / Enterprise — see the `license`
module). Open Kshana is community-supported (GitHub issues, best-effort); paid
support applies to Pro/Enterprise.

| | Community (open) | Pro | Enterprise |
|---|---|---|---|
| Channel | GitHub issues | Email (contact@ashforde.org) | Email + scheduled calls |
| Hours | Best-effort | Business hours (CET) | Business hours; extended by arrangement |
| Severity-1 response* | — | 1 business day | 4 business hours |
| Severity-2 response* | — | 2 business days | 1 business day |
| Maintenance / updates | Public releases | Pro releases + patches | + qualified/validated builds |
| Roadmap influence | — | Feature requests | Prioritised input |
| Named contact | — | — | Yes |

\* Response = first substantive reply (not resolution). Targets, not guarantees,
unless an SLA addendum says otherwise.

**Severity definitions.** S1 = production down / no workaround. S2 = major function
impaired, workaround exists. S3 = minor issue / question. S4 = enhancement request.

## 2. Service-level objectives (SLA)

For an Enterprise SLA addendum (placeholders — set per contract):

- **Response-time SLO:** as per the table above, measured from a complete report
  during covered hours.
- **Availability** (only if Ashforde hosts/managed-services the deployment):
  target **99.5%** monthly, excluding scheduled maintenance and force majeure.
  Self-hosted/on-prem/air-gapped deployments carry **no availability SLA** (the
  customer operates the service — see `deploy/DEPLOY.md`).
- **Service credits** (hosted only): e.g. 5% of the monthly fee per 0.5% below
  target, capped at 25%; credits are the sole remedy for missed availability.
- **Exclusions:** customer misconfiguration, customer infrastructure/network,
  unsupported modifications, use outside the licensed scope, force majeure.

## 3. Maintenance & versioning

- Pro tracks the open core (SemVer); security patches backported to the current
  minor for the support term.
- **End-of-life:** a minor version is supported for [12] months after its successor
  ships, or per the order form.
- Qualified/validated builds (Enterprise) are pinned and change-controlled.

## 4. Warranty

- **Limited warranty:** for the support term, the licensed software will perform
  substantially per its documentation; the sole remedy for a breach is repair,
  replacement, or a pro-rata refund of fees for the affected period.
- **Disclaimer:** except for the limited warranty, the software is provided **"as
  is"** (consistent with `LICENSE` and the Apache-2.0 terms of the embedded open
  engine). **No fitness-for-a-particular-purpose** warranty — Kshana Pro is a
  simulation/analysis tool; the customer is responsible for validating results for
  their use, mission, or certification.
- The validation/honesty discipline (tiered claims, cited anchors) supports but does
  not substitute for the customer's own verification.

## 5. Indemnification

- **IP indemnity (Ashforde → customer):** Ashforde will defend the customer against
  third-party claims that the **Pro overlay** (as delivered, unmodified) infringes a
  copyright/trademark/trade secret, and pay resulting damages/settlements, subject to
  prompt notice, control of defence, and cooperation. Standard carve-outs:
  modifications, combinations not supplied by Ashforde, use outside scope, the
  open-source components (governed by their own licences). Remedy options: procure a
  licence, modify, or refund.
- **Customer indemnity (customer → Ashforde):** for the customer's data/inputs
  (including any device or threat data under NDA) and use of outputs.
- **No indemnity for export-control breaches** by the customer; L2/export-controlled
  material is governed by [`EXPORT-CONTROL-ASSESSMENT.md`](EXPORT-CONTROL-ASSESSMENT.md).

## 6. Liability cap

- Aggregate liability capped at **the fees paid in the 12 months** preceding the
  claim; **no liability for indirect/consequential** loss (lost profits, mission
  loss). The IP indemnity (§5) and the customer's confidentiality/export obligations
  are the usual carve-outs from the cap — confirm with counsel.

## 7. Open issues for counsel
- [ ] Align caps/credits with the standard MSA and the ESA contract terms where Pro
      is delivered as Foreground.
- [ ] Confirm warranty/indemnity interaction with the embedded Apache-2.0 engine's
      "as is" terms.
- [ ] Jurisdiction/governing law (Estonia vs the DE operative nationality).
