# Background Intellectual Property Rights (BIPR) Declaration — Template

**⚠️ DRAFT TEMPLATE (PR6) — for ESA bids. Confirm wording against the specific ITT's
IPR clauses and Annexes; have counsel verify before submission.**

Purpose: declare the Background IPR Ashforde brings into an ESA contract, so the
Foreground/Background split is clear and Ashforde retains ownership of its
pre-existing engine while granting ESA the usage rights the contract requires.

---

## A. Bidder / contractor
- **Legal entity:** Ashforde OÜ
- **Registration:** [Estonian reg no.] · **Operative nationality (ESA-STAR):** Germany (DE)
- **ESA-STAR entity ID:** 83208 · **SSO contact:** contact@ashforde.org
- **Tender:** [AO/ITT reference and title, e.g. AO/1-13503 — Quantum-Enabled PNT Demonstrator]

## B. Background IPR brought into the contract

| # | Background item | Form | Owner | Licence basis | Made available to ESA as |
|---|-----------------|------|-------|---------------|--------------------------|
| 1 | **Kshana** — open-core PNT-resilience simulation engine | Software (Rust/Python/WASM), dual-licensed | Ashforde OÜ | AGPL-3.0-only or commercial; brought into the contract under the **commercial licence** (counsel to confirm grant terms) | Background; licensed for use in the work |
| 2 | **Kshana Pro framework** — proprietary overlay framework (extension seams, dispatcher, twin/assurance scaffolding) | Software (proprietary) | Ashforde OÜ | Proprietary; licensed for the contract | Background; licensed for use in the work |
| 3 | Validation corpus & methodology (public anchors, honesty ledger, reproducibility tooling) | Data + methods | Ashforde OÜ | Mixed (public anchors / proprietary methods) | Background |
| 4 | [Add any device-twin pipeline IP if pre-existing at contract start] | Software/method | Ashforde OÜ | Proprietary | Background |

> Items already public (e.g. the open Kshana engine) are **non-exclusively
> available** under their open licence and are listed so the boundary with
> Foreground is unambiguous. Listing them as Background does **not** convert them to
> Foreground or transfer ownership.

## C. Foreground IP expected to be created
- Device-specific quantum-payload **twin** (calibrated to the consortium's payload).
- Trade-study / assurance **methodology and evidence packs** specific to the study.
- **Default position:** Foreground is **owned by Ashforde** (the creating
  contractor); ESA receives the usage rights set out in the ESA R&D IPR clauses of
  the contract (typically rights to use for its own purposes / the programme), not
  ownership. Confirm against the specific contract's IPR Annex.

## D. Third-party / open-source components
- The Kshana engine's **third-party dependencies** are restricted to permissively
  licensed (non-copyleft) components; a dependency/licence manifest (`deny.toml`-gated
  in the open repo) is available on request. **No GPL/AGPL/copyleft third-party
  dependencies** are included. (The engine's own open-track licence is AGPL-3.0; into
  the contract it is brought under Ashforde's commercial licence — see §B.)

## E. Declarations
- The Bidder owns or controls the Background IPR listed in §B and is entitled to
  make it available as stated.
- Making the Background available does not infringe third-party rights.
- The Bidder will keep the Background/Foreground boundary documented through the
  contract.

Signed for Ashforde OÜ: ____________________________
Name/Title: Chakshu Baweja, Founder · Date: ______________
