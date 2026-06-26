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
| **Kshana** — open-core PNT-performance-and-resilience simulator (engine, v0.20.0 — or the exact release tag pinned at bid time) | Ashforde OÜ | **Dual-licensed: GNU AGPL-3.0-only (open track) or a commercial licence from Ashforde OÜ** | Software / simulation substrate used to perform the navigation-performance & trade-study WP |
| **Kshana Pro** — private contract-extensible overlay | Ashforde OÜ | Proprietary (NDA) | Optional; only relevant post-award (digital-twin Foreground IP). Not required to perform Phase 0/A |

Ashforde brings **Kshana** as Background IPR — dual-licensed (GNU **AGPL-3.0-only** for open use,
or a **commercial licence** from Ashforde OÜ), and public and auditable line-by-line either way.
It is used as the analysis tool; the **results** (trade-study reports, FoM tables, scenario
bundles) are the deliverables and follow the contract's Foreground-IPR terms.

## 2. Licence characteristics relevant to ESA

- **Brought in under the commercial licence (no copyleft attaches).** Kshana is offered into the
  consortium under Ashforde's **commercial licence**, under which **no copyleft obligation
  attaches** to ESA's or the prime's use, integration or redistribution of the engine or the
  results. The GNU AGPL-3.0-only governs only the separate open-source track; it is not the basis
  on which the engine enters this activity. *(Commercial-licence grant terms to be confirmed by
  counsel.)*
- **No GPL / no copyleft anywhere in the dependency tree.** The engine's transitive
  dependencies are restricted to a permissive allow-list — **Apache-2.0, MIT, BSD-2/3-Clause,
  ISC, Unicode, Zlib, CC0** — enforced in CI by `cargo-deny` (`deny.toml`); copyleft licences
  are rejected at build time. This avoids the open-source contamination risk ESA contracts
  guard against.
- **Reproducible & auditable.** A CycloneDX SBOM and build provenance accompany releases, so
  the exact open-source content used is declarable and verifiable.

## 3. ESA open-source-use authorization

Kshana is brought in under Ashforde's **commercial licence** as proprietary Background IPR, so the
open-source-software prior-authorisation route (GCC Part II Option A) is **not triggered** and no
copyleft attaches. *(Counsel to confirm the treatment.)* Should the consortium instead elect to
take Kshana under the open **AGPL-3.0-only** track, OSS prior-authorisation would apply and the
AGPL's copyleft would attach — in which case Ashforde will provide the **open-source-software-use
declaration / authorization** identifying:
- the component (Kshana) at its repository and release tag,
- the dependency licence inventory (permissive-only, per §2, with the SBOM),
- confirmation of **no GPL/copyleft** components.

## 4. Foreground IPR (post-award note)

Foreground IPR generated under the contract (analysis results, any device-specific digital
twin, custom packs) is governed by the contract's IPR clauses. A **customer/device-specific
quantum-payload digital twin** would be the first Pro **Foreground-IP** instance and would be
delivered under the agreed NDA/IPR and any export-control terms — distinct from the open
Background engine, which remains the dual-licensed (AGPL-3.0-only + commercial) open core.

## 5. Confirmations for the bid

- [ ] Kshana licence = AGPL-3.0-only + commercial (confirmed; `LICENSE`, `LICENSING.md`).
- [ ] Commercial-licence grant terms for ESA / the prime confirmed by counsel.
- [ ] No GPL/copyleft dependencies (confirmed; `deny.toml` permissive allow-list, CI-enforced).
- [ ] SBOM available for the released engine version used.
- [ ] Background vs Foreground split agreed with the prime per the draft contract (003).

---
*Contact for IPR/licensing matters: **contact@ashforde.org**.*
