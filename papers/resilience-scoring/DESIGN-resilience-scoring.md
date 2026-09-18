# DESIGN — Open PNT-Resilience Scoring & the Decision-Instability of Single-Number Ratings

> Paper #3 of the kshana program. **Capability code → public `kshana` (OPEN); paper draft → this folder in `kshana-pro` (PRIVATE).** TDD throughout, hand-derived oracle tests, every sub-score tagged Validated/Modelled. Branch `resilience-scoring` off `main`, **local only, nothing pushed without explicit go.** Neutral branch name (lesson from paper #1).

## 1. The gap (from our own 22-agent, red-teamed research)

Every authoritative source — DHS/CISA, RethinkPNT/Firesmith, Yang Yuanxi, CSIS, the EU PNT Portal, Safran — gestures at the **same missing object: a quantified, reproducible, vendor-neutral PNT-resilience score checked against a published framework.** The frameworks exist:

- **DHS/CISA Resilient PNT Conformance Framework (RPCF) v2.0 (31 May 2022)** — maturity **Levels 0–4** + 7 technique categories: Obfuscate, Limit, Verify, Isolate, Diversify, Mitigate, Recover ("OLVIDMR" is *our* mnemonic; cite DHS's own category names).
- **RethinkPNT / Firesmith** — Resist–Detect–Respond–Recover (RDRR) function model + the PNT assurance case (a documented body of evidence) + Figure-8 resilience-timeline metrics.
- **Yang Yuanxi** — comprehensive → resilient → intelligent PNT; criteria availability / reliability / continuity / accuracy + the resilient functional & stochastic models.

But **every existing instrument is self-attestation or pure concept** — no engine, no measurement, no evidence. kshana already deterministically simulates almost every threat and mitigation those rubrics grade (jamming, spoofing, meaconing, RAIM/ARAIM, spoof monitors, FoM holdover/availability/integrity/security, INS/gravity/terrain diversification, clock holdover), and carries an oracle-checked Validated/Modelled honesty matrix. **The wedge is the thin, honest scoring + assurance layer on top.**

## 2. The scientific contribution (why this is a paper, not just a product)

A scoring rubric is engineering. The *finding* is the hazard the red-team flagged: the whole edifice rides on **collapsing a multi-dimensional, modelled-majority reality into one number or one Level.** We study that collapse.

- **H1 — Decision-instability (headline, falsifiable, cautionary).** A single-number / single-Level PNT-resilience rating is *decision-unstable*: the ranking of architectures, and the assigned RPCF Level, **flip under defensible choices of (a) dimension weighting and (b) threat-scenario mix.** We quantify with top-1 flip rate over the weighting simplex, Kendall-τ dispersion between rankings, and RPCF-Level-flip rate, across a scenario-mix ensemble.
  - *Pre-registered refutation:* REFUTED if the top architecture is stable in ≥95% of defensible weight draws **and** no architecture's Level flips. Report whichever way it lands (paper #1 honestly refuted its H2 — same discipline).
- **H2 — Measured beats declared (constructive, empirical).** A small set of *measured* quantities — above all **bounded-vs-unbounded degradation** under sustained denial (the pivotal L2-vs-L3 discriminator) — separate architectures that share a *declared* Level. Self-attestation is insufficient; an oracle-checked engine supplies what a checklist cannot.
  - *Pre-registered:* show ≥2 architectures with identical declared-Level inputs but opposite bounded/unbounded verdicts under the same scenario.
- **H3 — Apparent redundancy ≠ effective redundancy (diversity).** Common-mode coupling collapses diversity sub-scores: architectures that look redundant (N sources) lose most of their resilience under a common-mode attack that defeats correlated sources at once. Quantify the diversity-score gap between nominal and common-mode-aware accounting.
  - *Pre-registered:* show the diversity sub-score / holdover of a "redundant" architecture drops by a stated margin once common-mode coupling is modelled vs. the independence-assumed baseline.

The constructive complement to H1 is a **principled reporting protocol**: per-dimension sub-scores + provenance + a **rank-range**, never a phantom single number — exactly what the frameworks lack and what an open, deterministic, oracle-checked engine can supply.

## 3. Honest scope (non-negotiable; baked into code and paper)

- **Controlled synthetic study**, parameter-grounded architectures — *not* a field measurement. First-class limitation, stated up front (mirrors paper #1's synthetic→synthetic honesty).
- Framing is **"simulation-derived self-assessment aligned to RPCF v2.0"** — never DHS/IEEE *certification*, "compliant", "endorsed", or "achieved Level X". Use "aligned to", "simulation-derived".
- Every sub-score carries a **Validated vs Modelled** tag on the face of every result (kshana is ~13 validated / ~12 modelled families — disclosed, not hidden). The score is honest *because* it shows its own modelled-majority.
- Timing-domain + detection FoMs only (kshana's `FoMScores` are timing-domain; position-domain CEP/SEP/HPL is honestly stubbed in `fom.rs`). The resilience score must not present a timing model as position accuracy.
- No certification, accreditation, or TRL claim > 3.

## 4. What gets built (public `kshana`, the dominant capability)

A complete, deterministic resilience scoring + assurance engine — the "top bets" shortlist, all verified net-new at the module level:

1. **`src/resilience/arch.rs`** — `PntArchitecture`: a system as a set of PNT sources/sensors/mitigations with attributes (source kind, band, independence group, present technique categories). New abstraction (verified absent).
2. **`src/resilience/score.rs`** — the scoring engine: architecture × simulated behaviour under a scenario ensemble → **per-dimension sub-scores** mapped to RPCF categories, RDRR functions, and Yang criteria; tentative RPCF Level via the framework's level criteria; each sub-score traceable to scenario+seed+engine+oracle and tagged Validated/Modelled.
3. **`src/resilience/timeline.rs`** — resilience-timeline KPIs (detect / react / recover / loss-duration) over the existing time-stepped engine, and the **bounded-vs-unbounded** degradation verdict.
4. **`src/resilience/diversity.rs`** — common-mode / SPOF / diversity analysis over the architecture's sources and their (in)dependence groups ("your apparent redundancy is illusory").
5. **`src/resilience/study.rs`** — the instability experiment: architecture panel × weighting-simplex × scenario-mix → rank-stability metrics (H1), declared-vs-measured contrasts (H2), diversity collapse (H3).
6. **`src/resilience/stats.rs`** — seeded Dirichlet simplex sampling, Kendall-τ, rank-range / top-1 flip-rate, percentile bootstrap — each with a hand-derived oracle test.
7. **`examples/resilience_report.rs`** — one-command self-describing JSON artifact (all sub-scores, instability metrics, provenance: engine version + seeds + config hash), the reproducibility object the paper cites.
8. **Auditable assurance report** — deterministic HTML/JSON, SHA-256 integrity-hashed, **non-certification + validated/modelled disclaimer on its face** (reuse `report`/`permalink` infra).
9. **`docs/RESILIENCE-CROSSWALK.md`** — kshana capability → RPCF/RDRR/Yang requirement map, with Validated/Modelled labels.
10. **`src/verification.rs`** Modelled row(s) + **`src/api.rs`** scenario dispatcher (`kind="resilience"`).

`src/lib.rs`: `pub mod resilience;` (a directory module: `arch`, `score`, `timeline`, `diversity`, `study`, `stats`).

Design conventions (match the optimism-gap build): dependency-light (reuse `rand`/`rand_chacha`; no new crates), deterministic/seeded, every numeric routine has a hand-derived oracle test, all artifacts carry the MODELLED label + synthetic caveat.

## 5. Venue & sequencing

arXiv **eess.SP** first (cross-list **cs.LG**/**cs.CR** as fitting), same as paper #1; IEEE/ION PLANS 2027 / IEEE TAES as the archival target. Code public in `kshana` (reproducibility = brand); draft PRIVATE here. **Nothing pushed without explicit founder go**; arXiv-first before any public method exposure.

## 6. Self-review notes

- Falsifiable claims with pre-registered thresholds: ✓ (H1/H2/H3).
- Honest scope is structural (Validated/Modelled tags, "aligned not certified"): ✓.
- Net-new vs. existing confirmed by code recon + red-team: scoring/aggregation/mapping, architecture object, timeline KPIs, common-mode/SPOF, instability study = new; FoMs, detectors, scenarios = consumed not duplicated.
- Risk: H1 could be *weakly* unstable (boring). Mitigation: report effect size honestly; even "stable under these conditions, unstable under those" is a publishable, useful boundary.
