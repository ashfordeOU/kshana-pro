<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
C06 — both-tender-ready BIPR table + the explicit ESA open-source-software-use prior-authorisation
request (component/repo/tag/licence inventory) + cargo-deny + CycloneDX SBOM evidence.
Cross-refs B14 (bipr-ip-posture-pack.md) and 13503-quantum-pnt/BIPR-declaration.md — does not duplicate.
Contact: contact@ashforde.org
-->

# BIPR Apache-2.0 prior-authorisation route — both FutureNAV C3 tenders

**PRIVATE — Ashforde OÜ internal working document.** The matured, both-tender-ready Background-IP
table **plus** the explicit ESA **open-source-software-use prior-authorisation request** and its
evidence chain (component/repo/tag/licence inventory + `cargo-deny` + CycloneDX SBOM). This is the
*route specifics*; the posture and the Background↔Foreground line are in
[`bipr-ip-posture-pack.md`](bipr-ip-posture-pack.md) (B14), and the canonical per-tender declaration
is [`../13503-quantum-pnt/BIPR-declaration.md`](../13503-quantum-pnt/BIPR-declaration.md).

**Date: 2026-06-16 · Contact: contact@ashforde.org**

> **Why prior authorisation matters.** Under ESA's GCC (Part II, Option A IPR), **open-source
> software may not be used to perform a contract without ESA's prior authorisation.** Ashforde uses
> Kshana (Apache-2.0) as the analysis tool, so the prime's BIPR route must carry an explicit
> authorisation request. This document gives the prime everything it needs to file that request.
>
> Ashforde = one SME WP seat, never prime, never a mandated Key Person. TRL ~3, no flight heritage,
> no certification, no ESA endorsement. Outreach is **PARKED**.

---

## 1. The Background-IP table (both tenders — identical)

| Item | Owner | Licence | Repository / release | Nature |
|---|---|---|---|---|
| **Kshana** — open PNT-performance-and-resilience simulator (engine) | Ashforde OÜ | **Apache-2.0** | `github.com/ashfordeOU/kshana` @ release tag (pin the exact tag used at bid time, e.g. `v0.17.x`) | Software / simulation substrate — the analysis tool used to perform the navigation-performance & trade WP |
| **Kshana Pro** — private contract-extensible overlay | Ashforde OÜ | Proprietary (NDA) | private repo | Optional; post-award Foreground vehicle only. **Not** required to perform Phase 0/A; not part of the prior-auth request |

Only **Kshana (Apache-2.0)** is in scope for the open-source prior-authorisation request. Kshana Pro
is proprietary and out of scope for it.

---

## 2. The explicit prior-authorisation request (drop into the prime's BIPR annex)

> **Open-source-software-use prior-authorisation request.** The consortium will use the following
> open-source software as a **tool** to perform the activity, and requests ESA's prior authorisation
> for that use under the GCC (Part II):
>
> - **Component:** Kshana — PNT-performance-and-resilience simulator (engine).
> - **Owner / contributor:** Ashforde OÜ (SME, DE nationality).
> - **Licence:** Apache-2.0 (permissive; includes a patent grant; no copyleft obligation).
> - **Repository:** `github.com/ashfordeOU/kshana`.
> - **Release tag used:** *pin the exact tag at bid time* (the bid-time release the work is performed against).
> - **Dependency licence inventory:** permissive-only — Apache-2.0 / MIT / BSD-2/3-Clause / ISC /
>   Unicode / Zlib / CC0 (see §3); **no GPL/LGPL/AGPL/MPL/EPL/CDDL** anywhere in the tree.
> - **Evidence:** `cargo-deny` permissive-only allow-list (CI-enforced, build fails on copyleft) +
>   a CycloneDX SBOM for the pinned release (see §3–§4).
> - **Use:** the engine is used as an analysis tool; the contract-funded **results** are Foreground IP
>   and the Background engine remains Apache-2.0.

Adapt the tender reference per ITT (AO/1-13494 or AO/1-13503); the request body is otherwise identical.

---

## 3. Component / repo / tag / licence inventory (the evidence the request rests on)

| Layer | What is declared | How it is evidenced |
|---|---|---|
| **The component itself** | Kshana engine, Apache-2.0, at the pinned release tag | `LICENSE` (Apache-2.0) + `NOTICE` in the repo at the tag |
| **The dependency tree** | Every transitive dependency is permissive-only (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0) | `cargo-deny` allow-list (`deny.toml`); copyleft is **rejected at build time** in CI |
| **The exact content** | The precise open-source content used at the pinned release | **CycloneDX SBOM** generated for the release (declarable, verifiable) |
| **No copyleft** | No GPL/LGPL/AGPL/MPL/EPL/CDDL present | `cargo-deny` fails the build if any copyleft licence enters the tree |
| **Build provenance** | The build is reproducible and attributable | deterministic builds + build provenance accompanying the release |

> **Pin the tag at bid time.** The single founder action that makes this airtight is recording the
> **exact release tag** the work is performed against, and generating the **SBOM for that tag**. The
> inventory then resolves to a fixed, verifiable artifact set.

---

## 4. The evidence chain — how a prime/ESA verifies it offline

1. Clone the repo at the pinned tag; confirm `LICENSE` = Apache-2.0 and read `NOTICE`.
2. Run the `cargo-deny` check against `deny.toml` — it passes only if every dependency licence is on
   the permissive allow-list; a copyleft dependency **fails** the check.
3. Generate / read the **CycloneDX SBOM** for the tag — it lists every component and its licence, so
   the exact open-source content is declarable and verifiable.
4. (Optional) run `cargo test` — the suite is offline and deterministic; the verification matrix
   (`src/verification.rs`) shows the validated/modelled boundary (see `audit-the-open-core/`).

This gives the prime a **machine-checkable** answer to the IPR annex: Apache-2.0 background,
permissive-only tree, no copyleft, SBOM available, build reproducible — and a ready-to-file ESA
prior-authorisation request.

---

## 5. Foreground note (post-award; not part of the prior-auth request)

Foreground IP generated under the contract (analysis results, any device-specific digital twin,
custom packs) follows the contract's IPR clauses. A customer/device-specific quantum/payload digital
twin would be the first **Kshana Pro** Foreground-IP instance, delivered under the agreed NDA/IPR and
any export-control terms — distinct from the open Background engine, which remains Apache-2.0. (Detail
in [`bipr-ip-posture-pack.md`](bipr-ip-posture-pack.md) §4.)

---

## 6. Founder actions to close this (gated)

- [ ] **Pin the exact release tag** used at bid time (both tenders can share the same tag).
- [ ] **Generate the CycloneDX SBOM** for that tag and attach it to each proposal's BIPR annex.
- [ ] Confirm the `cargo-deny` CI run is green at the pinned tag (permissive-only).
- [ ] File the §2 prior-authorisation request via each prime's BIPR route (per ITT reference).

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open Apache-2.0 PNT performance & resilience simulator ·
Background IP only; permissive-only dependency tree, no copyleft, SBOM-backed · TRL ~3, no flight heritage,
no certification, no ESA endorsement. Prime/partner outreach is PARKED pending explicit founder go.*
