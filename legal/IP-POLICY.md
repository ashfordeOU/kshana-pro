# Kshana — Intellectual Property Policy

**Owner:** Ashforde OÜ · **Status:** working draft (PR4) · **Date:** 2026-06-14
**Review gate:** ⚠️ have IP counsel review before relying on this for any signed agreement or bid.

This policy governs how IP is created, owned, and licensed across the open Kshana
engine and the proprietary Kshana Pro overlay. It exists so that (a) the open core
stays cleanly Apache-2.0, (b) the proprietary moat stays owned and closed, and
(c) contract/grant work banks Foreground IP for Ashforde.

---

## 1. The two-track IP boundary

| | Open Kshana | Kshana Pro |
|---|---|---|
| Licence | Apache-2.0 | Proprietary (All rights reserved) |
| Repo | public `AshfordeOU/kshana` | private `kshana-pro` (depends on open) |
| Contains | validated engine, public anchors, honesty ledger, interchange format, distribution | L1 device twins · L2 export-controlled resilience · L3 trade-study/assurance · enterprise enhancements |
| Role | Background IPR; the funnel | Foreground/proprietary; the moat |

**Test for placing a feature in Pro (all three required):** it is (a) valuable,
(b) something that cannot/should not be open (export-controlled, customer-
confidential, or hard-won proprietary IP), and (c) compounds the open core. If any
is false, it belongs in the open core.

## 2. Background vs Foreground IP

- **Background IPR** = everything owned/controlled by Ashforde *before* (or outside)
  a given engagement — principally the **open Kshana engine** and the Pro overlay
  framework. Background is **licensed** into an engagement, never assigned.
- **Foreground IP** = results created *during* a funded engagement (e.g. a device
  twin, a calibration pipeline, methodology). Ashforde's default position:
  **Ashforde owns Foreground**, the funder receives a licence/usage rights scoped
  to the contract. Under ESA R&D IPR terms this is the standard split (Background
  retained; Foreground owned by the contractor with ESA usage rights). See
  `BIPR-DECLARATION-TEMPLATE.md`.

## 3. Inbound contributions (open core)

- External contributions to the open repo are accepted under the **Apache-2.0
  inbound=outbound** convention (a contribution is licensed under the project
  licence). No CLA is required for that model; if a CLA/DCO is later adopted it is
  a deliberate, documented change.
- **Never** merge GPL/AGPL/other copyleft code or dependencies into the open core
  — it would relicense-contaminate the engine and break ESA/Pro terms. The
  `deny.toml` licence gate in the open repo enforces this in CI.

## 4. Customer / partner data

- Device characterisation data, calibration sets, threat libraries, and any
  customer-confidential material are **received under NDA** (see the NDA
  templates), stay **out of every repo**, and are **never** used to train or seed
  open-core features.
- Foreground built *from* such data (e.g. a fitted twin) is Pro IP and is delivered
  under the engagement's licence terms only.

## 5. Trademark

"Kshana" is an Ashforde OÜ trademark (filing in progress — see
`TRADEMARK-FILING-BRIEF.md`). The Apache-2.0 licence covers the *code*, not the
*name*: a fork may use the code, not the Kshana name or the canonical
distribution. Keep Ashforde the cited author/maintainer in all releases.

## 6. Authorship & attribution discipline

- Open-core commits and artifacts are authored as **Chakshu Baweja
  <contact@ashforde.org>** with no third-party/tooling attribution.
- The business email **contact@ashforde.org** is used in all external-facing
  artifacts; the personal address is not.

## 7. Open issues for counsel
- [ ] Confirm Estonian (OÜ) vs German treatment of Foreground ownership given the
      ESA-STAR DE nationality determination.
- [ ] Decide CLA/DCO posture for the open repo before the first sizeable external PR.
- [ ] Confirm trademark classes/jurisdictions (see filing brief).
