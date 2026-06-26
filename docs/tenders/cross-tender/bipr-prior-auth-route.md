<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
C06 — both-tender-ready BIPR table + the dual-license route into ESA contracts (commercial-licence
PRIMARY; open-source-software-use prior-authorisation as the FALLBACK for the open AGPL track) + the
component/repo/tag/licence inventory + cargo-deny + CycloneDX SBOM evidence.
Cross-refs B14 (bipr-ip-posture-pack.md) and 13503-quantum-pnt/BIPR-declaration.md — does not duplicate.
Contact: contact@ashforde.org
-->

# BIPR & licensing route (dual-license) — both FutureNAV C3 tenders

**PRIVATE — Ashforde OÜ internal working document.** The matured, both-tender-ready Background-IP
table **plus** the route by which Kshana enters the activity. Kshana is **dual-licensed — GNU
AGPL-3.0-only (open track) or a commercial licence from Ashforde OÜ**. The **primary route** for the
bid is the **commercial licence** (no copyleft attaches, and the open-source-software prior-auth
route is not triggered); the **fallback route** is the open AGPL-3.0 track with an explicit ESA
open-source-software-use prior-authorisation request. Both are documented here with the same evidence
chain (component/repo/tag/licence inventory + `cargo-deny` + CycloneDX SBOM). *(The commercial-licence
grant terms are the load-bearing dependency — confirm with counsel.)* The posture and the
Background↔Foreground line are in [`bipr-ip-posture-pack.md`](bipr-ip-posture-pack.md) (B14), and the
canonical per-tender declaration is
[`../13503-quantum-pnt/BIPR-declaration.md`](../13503-quantum-pnt/BIPR-declaration.md).

**Date: 2026-06-16 · Contact: contact@ashforde.org**

> **Why the route matters.** Under ESA's GCC (Part II, Option A IPR), **open-source software may not
> be used to perform a contract without ESA's prior authorisation**, and AGPL-3.0 is strong (network)
> copyleft. Both concerns are avoided by bringing Kshana in under Ashforde's **commercial licence** as
> proprietary Background IPR. The open-source-software-use prior-authorisation request (§2B) is retained
> as a fallback only if the consortium elects the open AGPL-3.0 track. *(Counsel to confirm the
> treatment.)*
>
> Ashforde = one SME WP seat, never prime, never a mandated Key Person. TRL ~3, no flight heritage,
> no certification, no ESA endorsement. Outreach is **PARKED**.

---

## 1. The Background-IP table (both tenders — identical)

| Item | Owner | Licence | Repository / release | Nature |
|---|---|---|---|---|
| **Kshana** — open-core PNT-performance-and-resilience simulator (engine) | Ashforde OÜ | **Dual-licensed: AGPL-3.0-only or commercial** (brought in under the **commercial licence**) | `github.com/ashfordeOU/kshana` @ release tag (pin the exact tag used at bid time, e.g. `v0.20.x`) | Software / simulation substrate — the analysis tool used to perform the navigation-performance & trade WP |
| **Kshana Pro** — private contract-extensible overlay | Ashforde OÜ | Proprietary (NDA) | private repo | Optional; post-award Foreground vehicle only. **Not** required to perform Phase 0/A; not part of the request |

Only **Kshana** is in scope for the licensing route; Kshana Pro is proprietary and out of scope.

---

## 2A. Primary route — commercial licence (recommended)

Kshana is brought into the activity under Ashforde's **commercial licence**, as proprietary Background
IPR. Consequences:
- **No copyleft attaches** to ESA's or the prime's use, integration or redistribution.
- The open-source-software-use prior-authorisation route (GCC Part II Option A) is **not triggered**,
  because the engine is not being used under an open-source licence in the activity.
- The AGPL-3.0-only governs only the separate open-source track and is not the basis of entry here.

*(Commercial-licence grant terms — scope, redistribution, sublicensing to ESA / the prime — to be
confirmed by counsel.)*

---

## 2B. Fallback route — open AGPL-3.0 track + prior authorisation

If the consortium instead elects to take Kshana under the open **AGPL-3.0-only** track, then OSS
prior-authorisation applies and the AGPL's copyleft attaches; file this request in the prime's BIPR
annex:

> **Open-source-software-use prior-authorisation request.** The consortium will use the following
> open-source software as a **tool** to perform the activity, and requests ESA's prior authorisation
> for that use under the GCC (Part II):
>
> - **Component:** Kshana — PNT-performance-and-resilience simulator (engine).
> - **Owner / contributor:** Ashforde OÜ (SME, DE nationality).
> - **Licence (open track):** GNU AGPL-3.0-only (copyleft; includes a patent grant via GPLv3 §11).
> - **Repository:** `github.com/ashfordeOU/kshana`.
> - **Release tag used:** *pin the exact tag at bid time* (the bid-time release the work is performed against).
> - **Dependency licence inventory:** permissive-only — Apache-2.0 / MIT / BSD-2/3-Clause / ISC /
>   Unicode / Zlib / CC0 (see §3); **no GPL/LGPL/MPL/EPL/CDDL** anywhere in the tree.
> - **Evidence:** `cargo-deny` permissive-only allow-list (CI-enforced, build fails on copyleft) +
>   a CycloneDX SBOM for the pinned release (see §3–§4).
> - **Use:** the engine is used as an analysis tool; the contract-funded **results** are Foreground IP.

Adapt the tender reference per ITT (AO/1-13494 or AO/1-13503); the request body is otherwise identical.

> **Note on AGPL.** On this fallback track the AGPL's network-copyleft and source-availability
> obligations apply to the *open engine*; they do not reach the contract-funded **results** (Foreground
> IP) or the proprietary Pro overlay. The primary route (§2A) avoids the copyleft question entirely.
> *(Counsel to confirm.)*

---

## 3. Component / repo / tag / licence inventory (the evidence the route rests on)

| Layer | What is declared | How it is evidenced |
|---|---|---|
| **The component itself** | Kshana engine, dual-licensed (AGPL-3.0-only + commercial), at the pinned release tag | `LICENSE` (AGPL-3.0) + `LICENSING.md` (the dual-licence terms) in the repo at the tag |
| **The dependency tree** | Every transitive dependency is permissive-only (Apache-2.0/MIT/BSD/ISC/Unicode/Zlib/CC0) | `cargo-deny` allow-list (`deny.toml`); copyleft is **rejected at build time** in CI |
| **The exact content** | The precise open-source content used at the pinned release | **CycloneDX SBOM** generated for the release (declarable, verifiable) |
| **No copyleft in the dependency tree** | No GPL/LGPL/MPL/EPL/CDDL dependency present (the engine's own AGPL open-track licence is not a dependency) | `cargo-deny` fails the build if any copyleft licence enters the tree |
| **Build provenance** | The build is reproducible and attributable | deterministic builds + build provenance accompanying the release |

> **Pin the tag at bid time.** The single founder action that makes this airtight is recording the
> **exact release tag** the work is performed against, and generating the **SBOM for that tag**. The
> inventory then resolves to a fixed, verifiable artifact set.

---

## 4. The evidence chain — how a prime/ESA verifies it offline

1. Clone the repo at the pinned tag; confirm `LICENSE` = AGPL-3.0 and read `LICENSING.md` (the
   dual-licence terms — open AGPL track or commercial).
2. Run the `cargo-deny` check against `deny.toml` — it passes only if every dependency licence is on
   the permissive allow-list; a copyleft dependency **fails** the check.
3. Generate / read the **CycloneDX SBOM** for the tag — it lists every component and its licence, so
   the exact open-source content is declarable and verifiable.
4. (Optional) run `cargo test` — the suite is offline and deterministic; the verification matrix
   (`src/verification.rs`) shows the validated/modelled boundary (see `audit-the-open-core/`).

This gives the prime a **machine-checkable** answer to the IPR annex: a dual-licensed background engine
brought in under a commercial licence (no copyleft attaches), a permissive-only dependency tree, an
SBOM, a reproducible build — and, if the open AGPL track is elected instead, a ready-to-file ESA
prior-authorisation request. *(Commercial-licence grant terms to be confirmed by counsel.)*

---

## 5. Foreground note (post-award; not part of the licensing request)

Foreground IP generated under the contract (analysis results, any device-specific digital twin,
custom packs) follows the contract's IPR clauses. A customer/device-specific quantum/payload digital
twin would be the first **Kshana Pro** Foreground-IP instance, delivered under the agreed NDA/IPR and
any export-control terms — distinct from the open Background engine, which remains the dual-licensed
(AGPL-3.0-only + commercial) open core. (Detail in
[`bipr-ip-posture-pack.md`](bipr-ip-posture-pack.md) §4.)

---

## 6. Founder actions to close this (gated)

- [ ] **Confirm the commercial-licence grant terms** (scope, redistribution, sublicensing to ESA / the
      prime) with counsel — this underwrites the primary route (§2A).
- [ ] **Pin the exact release tag** used at bid time (both tenders can share the same tag).
- [ ] **Generate the CycloneDX SBOM** for that tag and attach it to each proposal's BIPR annex.
- [ ] Confirm the `cargo-deny` CI run is green at the pinned tag (permissive-only dependency tree).
- [ ] If electing the open AGPL track instead, file the §2B prior-authorisation request via each
      prime's BIPR route (per ITT reference).

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open-core PNT performance & resilience simulator (AGPL-3.0-only + commercial licence) ·
Background IP only; permissive-only dependency tree, no copyleft, SBOM-backed · TRL ~3, no flight heritage,
no certification, no ESA endorsement. Prime/partner outreach is PARKED pending explicit founder go.*
