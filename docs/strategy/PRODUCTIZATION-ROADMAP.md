# Kshana Productization Roadmap

**From "simulation substrate" to a product — one spine, three packs, sequenced.**
Private (Kshana Pro). Never publish to the open repo.

---

## 0. The honest starting point

A three-part audit of `kshana` (open) and `kshana-pro` (private) found that **the hard
parts are already built.** The engines for all three packs, the overlay architecture,
licensing, the dependency-light serve layer, and the honesty gate are DONE and tested.
What is missing is **not engine work** — it is the **customer-facing product layer** and
the **commercial / legal / go-to-market layer**.

This roadmap therefore is *finish + package + sell*, not *build from scratch*. Any task
below that proposes re-implementing an existing engine is wrong by construction — the
audit's job was to stop exactly that.

### What already exists (do NOT rebuild)

| Area | State | Evidence |
|---|---|---|
| Pack A engine (RF-impairment optimism gap) | DONE, published | `kshana/src/impairment_{eval,ml,study}.rs`; arXiv:2606.22054 |
| Pack B engine (holdover + conditional TPL) | DONE, validated | `kshana/src/holdover.rs`, `kshana/src/tpl.rs` |
| Pack C engine (resilience scoring) | DONE, oracle-tested | `kshana/src/resilience/` (9 files, 35 oracle tests) |
| Open report pipeline (JSON + SVG + self-contained HTML, scenario hash, KIF) | DONE | `kshana/src/report.rs`, `kshana/src/api.rs`, `kshana/src/interchange.rs` |
| Scenario library | DONE (46 scenarios, 34 kinds) | `kshana/scenarios/`, `kshana/src/api.rs` |
| Honesty gate | DONE (7 retired overclaims banned in CI) | `kshana/tests/no_overclaims.rs` |
| Overlay architecture (superset-by-construction, never forks) | DONE | `kshana-pro/src/registry.rs`, `docs/OVERLAY-ARCHITECTURE.md` |
| Commercial report renderer (MD/HTML/SVG/JSON, fingerprint) | DONE | `kshana-pro/src/report.rs` |
| Licensing / tier enforcement (Ed25519 tokens, HMAC until 2026-09-26; Eval/Pro/Enterprise) | DONE | `kshana-pro/src/license/` |
| Serve layer (`/health`, `/version`, `/kinds`, `POST /run`, entitlement-gated) | DONE | `kshana-pro/src/serve/` |
| Six Pro packs (twin, trade-study, mbse, batch, quantum-systematics, seam-check) | BUILT (public-data demonstrators) | `kshana-pro/src/packs/` |
| Legal foundation (SLA, IP policy, NDA ×2, export-control, BIPR, trademark brief) | DRAFTS | `kshana-pro/legal/` |
| Web commercial section + `contact@ashforde.org` | DONE | `kshana/web/index.html` (Editions/Commercial) |

### The genuine gaps (what this roadmap delivers)

- **Open spine completion:** per-FoM MODELLED/VALIDATED labels surfaced in the report,
  a multi-scenario *study* (suite) format + aggregation, study metadata + naming +
  timestamp, a one-command study UX, and scenario validation. (Thin, reproducible —
  stays open; reproducibility is the credibility.)
- **Commercial product layer:** a branded cover + executive summary + provenance block
  on the report, PDF output, multi-pack **ProStudy** orchestration, a study-request
  intake + tracking motion.
- **Pack packaging:** turn each engine into a buyable pack — Pack A first (customer-detector
  intake + field-vs-bench report template), then Pack C (scenario versioning/subscription +
  CI harness), then Pack B (export-control-gated, partner-led).
- **Commercial/legal:** Commercial License/EULA, Pricing, Data-Processing Agreement, SOW
  template, MSA; per-pack one-pagers; a sample study report; "request a study" CTA.

---

## 1. The open / commercial boundary (decide once, hold the line)

| Layer | Home | Why |
|---|---|---|
| Engines (A/B/C compute) | **open** `kshana` | Already open; reproducibility = credibility = lead-gen |
| Thin reproducible report (JSON/SVG/plain HTML), study/suite format, labels, validation | **open** `kshana` | The open studies must render & reproduce cleanly; costs nothing competitively |
| Branded report (cover, exec summary, PDF, Ashforde identity), multi-pack orchestration | **commercial** `kshana-pro` | The polish + composition is the paid layer |
| Curated/commercial scenario packs, versioning, subscription, access control | **commercial** `kshana-pro` | Curation + delivery is monetizable |
| Customer-detector intake, study tracking, intake portal | **commercial** `kshana-pro` | The "service" surface |
| Pricing, EULA, SOW, DPA, MSA, one-pagers, sample report | **commercial** `kshana-pro` | Never public |

**Hard rules (the moat):** keep `MODELLED` + "not a certification" on every surface; sell
*reproducible evidence that feeds the customer's own V&V*, never "we certify"; cite only
live preprints (currently arXiv:2606.22054), described as preprint, not peer-reviewed; no
TRL/heritage/ESA-endorsement implications; `contact@ashforde.org` only; never reintroduce a
retired-overclaim phrase (see `kshana/tests/no_overclaims.rs`).

---

## 2. Phases & sequencing

Rationale: complete the **shared spine** (serves all three packs) → ship **Pack A** (engine
done, paper published, R&D buyers accept MODELLED, services-led validates price) → **Pack C**
(recurring revenue, needs versioning/subscription) → **Pack B** (largest TAM but gated on
export-control clearance + validation maturity → genuinely later, partner-led).

```
Phase 0  Spine completion        (open + commercial foundation; serves all packs)
Phase 1  Pack A → shippable      + Legal Tier-1 + sample report + "request a study"
Phase 2  Pack C → subscription   (scenario versioning + CI harness)
Phase 3  Pack B → partner pilot  (export-control-gated; critical-infra timing)
X-cut    MSA, trademark, GTM/financial model, README graduation, honesty gate green
```

### Phase 0 — Spine completion (buildable now)
Detailed TDD tasks: see `PLAN-phase-0-1-spine-and-pack-a.md` (tasks O1–O8, P1–P7).
- **Open (`kshana`):** per-FoM validation label in report; study metadata (additive, does
  not perturb scenario hash); study title + generation timestamp in footer/HTML; `--study-name`;
  suite/study manifest + aggregation; `--validate` linting; honesty gate stays green.
- **Commercial (`kshana-pro`):** branded cover + executive summary + provenance block in
  `report.rs`; inline per-FoM labels; PDF export; `ProStudy` multi-pack orchestration; study
  intake schema + `POST /study`; study tracking ledger.
- **DoD:** one command turns a scenario (or a suite) into a branded, MODELLED-labelled,
  provenance-stamped report (HTML + PDF); a multi-pack study composes A/B/C sections.

### Phase 1 — Pack A "Optimism-Gap Audit" → shippable (lead)
Detailed TDD tasks: see `PLAN-phase-0-1-spine-and-pack-a.md` (tasks A1–A6) + Legal/GTM track.
- **Commercial:** `pro-optimism-gap-audit` pack; customer-detector intake format (bring your
  own detector scores/corpus); Pack-A report template (field-vs-bench degradation headline,
  R²/p, figures); Pack-A scenario templates.
- **Legal Tier-1 (blockers — parallel doc track):** `COMMERCIAL-LICENSE.md` (EULA), `PRICING.md`
  (rate card incl. Pack A study), `DATA-PROCESSING-AGREEMENT.md`, `SOW-TEMPLATE.md`.
- **GTM:** `docs/packs/PACK-A.md` one-pager (ICP = detector R&D teams; JTBD = "know field-vs-bench
  degradation before you ship"); committed **sample study report** (Pack A, from published data);
  "request a study" CTA + intake form on `kshana.dev`.
- **DoD:** a design partner can request a Pack A study, sign Tier-1 terms, and receive a branded
  reproducible report. README status line graduates (see X-cut).

### Phase 2 — Pack C "Scenario/Scoring CI Harness" → subscription (expansion)
Roadmap granularity now; gets its own detailed plan when Phase 1 has ≥1 reference.
- Scenario versioning/release layer (tag, release notes, manifest, hash-locked sets).
- `pro-regression-suite` pack: run a versioned scenario set with gates → pass/fail scorecard.
- Scenario-level access control / subscription (per-customer scenario entitlement).
- CI integration guide (GitHub Actions / GitLab CI snippets) wiring `kshana-pro gate`.
- GTM: `docs/packs/PACK-C.md`; subscription pricing tier.
- **DoD:** a receiver vendor pins CI to a versioned scenario set and regresses each build.

### Phase 3 — Pack B "Timing Resilience / Holdover Risk" → partner pilot (gated)
Roadmap granularity; **blocked on export-control clearance** (founder/counsel) before delivery.
- Legal: complete `EXPORT-CONTROL-ASSESSMENT.md` determinations + `CUSTOMER-SCREENING-FORM.md`
  + L2 access-control mechanism. (Gate before any L2 resilience delivery.)
- `pro-timing-resilience` pack; Pack-B report template (TPL margin, time-to-breach, holdover
  curve, RTM for timing requirements); critical-infra scenario templates (4 h / 24 h / 1-week
  holdover; denial + spoof profiles).
- GTM: `docs/packs/PACK-B.md`; partner-led outreach (telecom/grid/finance).
- **DoD:** a design-partner timing-resilience pilot delivered under clearance + NDA.

### Cross-cutting / ongoing
- `MSA-TEMPLATE.md` (stitches SLA + IP + DPA + SOW); needed for primes/large customers.
- **Trademark** clearance search + EUIPO filing — *founder/attorney action* (Kshana Inc./kshana.ai
  coexistence risk flagged in `legal/TRADEMARK-FILING-BRIEF.md`).
- `GTM-STRATEGY.md` + simple financial model (CAC/LTV, pricing tiers, design-partner funnel).
- **README graduation:** keep `kshana/README.md` line `v0.20.0 · a simulation substrate, not yet
  a product` until a design partner is *paying for a defined deliverable*; then graduate to
  (honest) `v1.0 · reproducible PNT-resilience analysis on an open engine` — still MODELLED, still
  no certification claim.
- Honesty gate green on every change; extend `no_overclaims.rs` scan if new public surfaces appear.

---

## 3. Founder-gated / external (I cannot do these — by design)

| Item | Owner | Note |
|---|---|---|
| Any `git push` / release / tag | Founder | Push only when explicitly told; never change repo visibility |
| Pricing numbers sign-off | Founder | I draft structure; you set the figures |
| Counsel review of EULA/MSA/DPA/SOW | Counsel | Templates are drafts "subject to counsel" |
| Trademark clearance search + filing | Attorney | Coexistence risk flagged |
| Export-control determination for L2 (competent authority to confirm: probably Estonia's Strategic Goods Commission) | Counsel | Gates Pack B delivery |
| arXiv submissions/replacements | Founder | Authenticated manual step |
| Customer data | Founder | Real customer/threat data stays out of public repo |

---

## 4. Definition of done (the whole program)

1. Spine: one command → branded, MODELLED-labelled, provenance-stamped report (HTML + PDF),
   single-scenario or multi-pack study; open thin renderer reproduces it byte-stably.
2. Pack A live: ICP one-pager + sample report + intake + Tier-1 legal + a paying design partner.
3. Pack C live: versioned scenario subscription + CI harness + a subscribing vendor.
4. Pack B live: export-control cleared + a partner pilot.
5. README graduated; honesty gate green throughout; nothing overclaimed.
