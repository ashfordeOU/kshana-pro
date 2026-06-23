# Kshana Pro — Data Processing & Handling Agreement (DPA)

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-23
**Review gate:** ⚠️ legal/privacy template — have counsel review (GDPR + any export-control overlap) before relying on it. Bracketed `[…]` items are deal-/counsel-set.

How Ashforde handles a licensee's inputs when delivering a Study Report — the answer to the
customer question "where does my detector/device/threat data go, for how long, and can I
audit it?". Attaches to the [`COMMERCIAL-LICENSE.md`](COMMERCIAL-LICENSE.md) and the executed
NDA; export-controlled inputs additionally follow
[`EXPORT-CONTROL-ASSESSMENT.md`](EXPORT-CONTROL-ASSESSMENT.md).

---

## 1. Scope of data
Customer inputs may include: detector scores/labels (Pack A intake), device/clock specs,
mission/threat parameters, and any files supplied to scope a study. **Customers are
encouraged to submit the minimum necessary** — e.g. Pack A needs detector *scores + labels*,
not raw RF recordings or model weights.

## 2. Roles & purpose
Ashforde processes customer inputs **solely** to produce the agreed Study Report and support
it. No use for any other purpose; no sale or sharing of customer data; no use to train models
for other customers.

## 3. Location & sub-processors
Processing/storage location: `[EU — Estonia/Germany; specify]`. Sub-processors (if any) listed
and flowed-down `[list or "none"]`. Cross-border transfer only with appropriate safeguards.

## 4. Retention & deletion
Inputs retained only as long as needed for the engagement + `[retention window]`, then
deleted. On termination or request, inputs deleted within `[N days]` (deliverables already
issued to the customer are unaffected). Reproducibility is preserved via the scenario hash +
seed + engine version, **not** by retaining raw customer inputs longer than necessary.

## 5. Security
Access on a need-to-know basis; `[encryption at rest / in transit]`; export-controlled
inputs held under the access-control mechanism in
[`EXPORT-CONTROL-ASSESSMENT.md`](EXPORT-CONTROL-ASSESSMENT.md) (e.g. restricted decryption,
verified nominees only).

## 6. Audit & incident response
Customer audit rights `[scope/frequency]`. Personal-data or security incident: notify the
customer without undue delay `[≤72h target]`, with cause, impact, and remediation.

## 7. Personal data
Studies are typically engineering data, not personal data. Where any personal data is
incidentally involved, standard GDPR controller/processor terms apply `[insert / reference MSA]`.

---
**Contact:** [contact@ashforde.org](mailto:contact@ashforde.org) · Ashforde OÜ
*Open items for counsel: processing location + sub-processor list; retention windows; GDPR
controller/processor classification; alignment with NDA + MSA + export-control access controls.*
