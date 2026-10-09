<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Draft outline: ESA NAVISP Element 2 outline proposal

**Status:** draft outline, not a submission. **Gated:** Estonia's NAVISP eligibility and support-letter route are unresolved (sources conflict; see [`PROGRAMMES.md`](PROGRAMMES.md) section 1). Do not invest further until ESA confirms. Co-funding rate is **UNKNOWN**.
**Kshana facts used:** public repository README, ROADMAP and docs only. No partner names or data; no anti-jam or anti-spoof performance figures.

## 1. Problem

Industry building PNT products and services for commercial and institutional markets lacks a neutral, reproducible way to quantify how a system behaves when GNSS is degraded, and how far a receiver's output can be trusted during an interference event. Evidence is usually bespoke and not repeatable by a third party. `[FOUNDER: add a sourced market statement.]`

## 2. Solution

### Already done (public)

- Open-source (AGPL-3.0) Rust engine with 75 scenario kinds covering orbit geometry, clocks, inertial navigation, GNSS/INS fusion, integrity, jamming link budget, time-spoof and multi-layer spoof detection, and holdover. Bit-for-bit reproducibility. Capabilities labelled VALIDATED, MODELLED or PARTNER (README, ROADMAP).
- Receiver-log assessment with pre-stated events and tolerances, per-epoch nominal/degraded/untrusted states, event outcomes and hashed inputs (`docs/RECEIVER-TRUST.md`).
- Roadmap priority 1 (public): telecom checks (MTIE and TDEV against ITU-T masks), longer holdover scenarios, measured clock-data ingestion (ROADMAP).

### What the activity would fund (`[FOUNDER: confirm]`)

- Productising the receiver-log assessment and evidence packaging for industrial users.
- Independent methodology review and publication of its findings.
- Additional log formats and documentation for ESA and industry evaluation.

## 3. Work packages

| WP | Title | Duration | Effort |
|---|---|---|---|
| 1 | Requirements and industry user consultation | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 2 | Method and software extension | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 3 | Independent review (`AUDIT-SCOPE.md`) and corrective work | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 4 | Demonstration on industry-supplied logs | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 5 | Release, documentation, final report | `[PLACEHOLDER]` | `[PLACEHOLDER]` |

## 4. Deliverables

Requirements note; released software and tests; independent review report with response; demonstration report with stated agreements and disagreements; documentation. Software delivered to ESA in object code only under the stated Element 2 terms [S10], with the source released publicly under AGPL-3.0 by Ashforde OÜ as owner. `[COUNSEL: confirm compatibility before proposing.]`

## 5. Team

Ashforde OÜ, Tallinn. Roles only: `[FOUNDER: roles and employment status]`. Subcontractor: independent reviewer, selected under `AUDIT-RFP.md`; none named here.

## 6. Budget shape

| Line | Amount |
|---|---|
| Personnel | `[PLACEHOLDER]` |
| Subcontract: independent review | `[PLACEHOLDER]` |
| Other direct costs | `[PLACEHOLDER]` |
| Total activity cost | `[PLACEHOLDER; ESA range is EUR 60k to 2M per activity, S2]` |
| ESA contribution / company co-funding | `[UNKNOWN rate]` |

## 7. Risks

| Risk | Mitigation |
|---|---|
| Estonia not eligible or no delegation route | Ask ESA before any drafting effort |
| ESA IPR clauses (50-63) restrict open release | Counsel reads the clauses first |
| No national support letter | Contact the Estonian space contact point via ESA's guidance |
| Co-funding burden on a small company | Obtain the rate; do not submit if unaffordable |
| Simulator is not a certified product | State limits and labels plainly |
