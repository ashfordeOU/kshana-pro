<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
T13 — Background IPR declaration for AO/1-13503. For the prime's IPR/contract annex.
Contact: contact@ashforde.org
-->

# Background IPR declaration — Ashforde OÜ for AO/1-13503

Draft Background IPR statement for the consortium's contract/IPR annex. Adapt to the prime's
template and the draft contract's actual IPR clauses (003).

---

## 1. Background IPR brought to the activity

| Item | Owner | Licence | Nature |
|------|-------|---------|--------|
| **Kshana** — open PNT-performance-and-resilience simulator (engine, v0.17.0) | Ashforde OÜ | **Apache-2.0** (open source) | Software / simulation substrate used to perform the navigation-performance & trade-study WP |
| **Kshana Pro** — private contract-extensible overlay | Ashforde OÜ | Proprietary (NDA) | Optional; only relevant post-award (digital-twin Foreground IP). Not required to perform Phase 0/A |

Ashforde brings **Kshana (the open Apache-2.0 engine)** as Background IPR. It is used as the
analysis tool; the **results** (trade-study reports, FoM tables, scenario bundles) are the
deliverables and follow the contract's Foreground-IPR terms.

## 2. Licence characteristics relevant to ESA

- **Permissive open source (Apache-2.0).** ESA, the prime, and consortium partners may use,
  run, modify, and redistribute the engine under Apache-2.0, including a patent grant. No
  copyleft obligation attaches to ESA's or the prime's use of the results.
- **No GPL / no copyleft anywhere in the dependency tree.** The engine's transitive
  dependencies are restricted to a permissive allow-list — **Apache-2.0, MIT, BSD-2/3-Clause,
  ISC, Unicode, Zlib, CC0** — enforced in CI by `cargo-deny` (`deny.toml`); copyleft licences
  are rejected at build time. This avoids the open-source contamination risk ESA contracts
  guard against.
- **Reproducible & auditable.** A CycloneDX SBOM and build provenance accompany releases, so
  the exact open-source content used is declarable and verifiable.

## 3. ESA open-source-use authorization

The activity uses open-source software (Kshana, Apache-2.0) as a tool. Where the draft
contract requires it, Ashforde will provide the **open-source-software-use declaration /
authorization** identifying:
- the component (Kshana, Apache-2.0), its repository and release tag,
- the dependency licence inventory (permissive-only, per §2, with the SBOM),
- confirmation of **no GPL/copyleft** components.

## 4. Foreground IPR (post-award note)

Foreground IPR generated under the contract (analysis results, any device-specific digital
twin, custom packs) is governed by the contract's IPR clauses. A **customer/device-specific
quantum-payload digital twin** would be the first Pro **Foreground-IP** instance and would be
delivered under the agreed NDA/IPR and any export-control terms — distinct from the open
Background engine, which remains Apache-2.0.

## 5. Confirmations for the bid

- [ ] Kshana licence = Apache-2.0 (confirmed; `LICENSE`, `NOTICE`).
- [ ] No GPL/copyleft dependencies (confirmed; `deny.toml` permissive allow-list, CI-enforced).
- [ ] SBOM available for the released engine version used.
- [ ] Background vs Foreground split agreed with the prime per the draft contract (003).

---
*Contact for IPR/licensing matters: **contact@ashforde.org**.*
