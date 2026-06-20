# Design spec — "Anticipating the optimism gap" (AI/ML RF-impairment detection in PNT)

> **Status:** design, approved verbally 2026-06-19. **Storage:** this paper + its drafts live in `kshana-pro/papers/` (PRIVATE); the supporting *code* lives in `kshana` (PUBLIC, for reproducibility). **Nothing is pushed or submitted without explicit founder go.**
> **Quality bar:** MIT/arXiv-level — precise falsifiable claims, real statistics (bootstrap CIs, DeLong, trend tests), honest scope, a one-command reproducible artifact, prior-art positioning strong enough to survive a hostile reviewer.

## 0. Decisions locked

- **Type:** full research paper (not a tools/JOSS object).
- **Code location:** `kshana` (open). Reproducibility is the brand and the paper's strength.
- **Authorship/affiliation:** Chakshu Baweja, Ashforde OÜ — matching the existing arXiv paper.
- **Optional "gap predictor" stretch:** IN, and made the headline contribution.
- **Venue:** arXiv (eess.SP) first; then IEEE/ION PLANS 2027 or IEEE TAES. (ION GNSS+ 2026 abstract deadline of 3 Mar 2026 already passed.)
- **ESA relevance:** NAVISP EL1-120 (ML interference-detection benchmark DB) / EL1-117; and directly on-thesis for tender AO/1-13494 (AI digital payload, "digital twin for validation & performance assessment"). Credibility/visibility only — **no TRL, heritage, certification, or endorsement implied.**

## 1. The honest, publishable thesis

It is folklore in GNSS interference detection that **lab/in-distribution AUC overstates field performance** for ML detectors; the Fraunhofer IIS group (Heublein, Ott et al., 2024–25) has documented this sim-to-real / generalization gap with public datasets. What is **missing** is a *reproducible, open, controlled* characterization of **how that evaluation optimism behaves** as a function of impairment type and severity — because real field datasets are scarce and proprietary, so the gap stays anecdotal.

We take the same methodological stance that made **ImageNet-C** (Hendrycks & Dietterich, 2019) highly cited: a *controlled* benchmark deliberately isolates a variable that messy real data cannot. We measure the optimism gap under **controlled distribution shift in a physically-grounded synthetic RF-impairment corpus**, and we are explicit that this characterizes **mechanism and regime**, not the absolute field-gap magnitude.

**The differentiator (headline):** we go beyond *measuring* the gap to **predicting** it — a model that anticipates a detector's out-of-distribution AUC from **in-distribution-only** statistics, validated by leave-one-detector-out / leave-one-class-out cross-validation. "You can anticipate the optimism gap before you ever touch field data" is the contribution that lifts this from a benchmark to a method.

This turns Kshana's machine-checked honesty discipline (validated-vs-modelled labels, no-overclaim CI guard) into the paper's scientific spine rather than a disclaimer.

## 2. Precise claims and hypotheses

Let a *detector* `d` map a case's observables to a scalar (higher ⇒ more impaired). For an impairment class `c ∈ {jamming, spoof-time, spoof-position, multipath}`, define the **per-class one-vs-nominal AUC** at severity scale `s`:

> `AUC_c(d, s)` = Mann–Whitney AUC of `d`'s scores on class-`c` positives vs Nominal negatives, drawn from a corpus generated at `severity_scale = s`.

The **in-distribution** corpus uses `s = 1.0`; an **out-of-distribution** corpus uses `s < 1.0` (subtler, out-of-tuning-regime impairments — the `severity_scale` knob already in `CorpusConfig`). For learned detectors, AUC_in is computed on a **held-out ID test split** (never training data); for physics baselines on the same test split for comparability.

> **Optimism gap:** `Δ(d, c, s) = AUC_c(d, 1.0) − AUC_c(d, s)`, with `s < 1`.

**Hypotheses (all falsifiable):**

- **H1 (monotone scaling law).** `Δ(d, c, s)` increases with shift magnitude `(1 − s)` — i.e. decreases in `s`. *Test:* Spearman ρ between `(1−s)` and `Δ` across the `s`-grid, per (d,c); expect ρ > 0 with significant p.
- **H2 (learned overfit more).** At matched in-distribution AUC, **learned** detectors (logistic regression, MLP) exhibit a **larger** `Δ` than **physics-based** baselines (energy/AGC/SQM/parity/fused). *Test:* compare `Δ` distributions across detector families with bootstrap CIs; report the family gap with a CI that excludes 0.
- **H3 (class heterogeneity).** `Δ` differs systematically by impairment class; the matched-power **spoof-time** class (deliberately the hard, near-nominal case) behaves distinctly. *Test:* per-class `Δ` with CIs; report ordering.
- **H4 (predictability — headline).** A regressor using **ID-only** features predicts held-out `Δ` (or `AUC_out`) with high accuracy across **unseen detectors and unseen classes**. *Test:* leave-one-detector-out and leave-one-class-out CV; report R² and RMSE of predicted-vs-actual `Δ`, with a baseline (predict-the-mean) for contrast.

**Explicitly NOT claimed:** that the synthetic `Δ` equals the deployed field gap. §6 gives a falsification protocol for real data.

## 3. Experimental design

- **Independent variables:** detector `d` (≈7: 5 physics + ≥2 learned) × impairment class `c` (4) × shift scale `s ∈ {0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9}` × seed (N = 30 independent corpus draws). `n_per_class` ≈ 400.
- **Dependent variables:** `AUC_c(d, s)`, `Δ(d, c, s)`, per-class Pd at a fixed operating Pfa (Neyman–Pearson), the fitted scaling-law slope, and the gap-predictor's CV error.
- **Protocol per seed:**
  1. Generate ID corpus (`s = 1.0`); `stratified_split` into train/test with the **strengthened near-duplicate leakage guard** (see §5/PLAN) enforced.
  2. Train learned detectors on the ID train split only.
  3. For each `s` in the grid, generate an OOD corpus (fresh draw) and compute `AUC_c(d, 1.0)` (ID test) and `AUC_c(d, s)` (OOD) → `Δ(d, c, s)` for every detector and class.
  4. Compute ID-only feature vector per (d, c) for the gap predictor.
- **Aggregation:** across the 30 seeds, report mean `Δ` with bootstrap and across-seed 95% CIs; fit the scaling law; run the predictor CV.

## 4. Statistics (all hand-derived, oracle-tested)

- **Mann–Whitney AUC** — already in `impairment_eval::auc` (validated vs the binormal closed form `AUC = Φ(d'/√2)`). Reuse.
- **Bootstrap 95% CI** — percentile method, B = 2000 case-resamples, for AUC and for Δ; plus the across-seed empirical 2.5/97.5 percentiles. *Oracle test:* coverage on a known binormal AUC; CI brackets the analytic value.
- **DeLong AUC variance / CI** (DeLong 1988; Sun & Xu 2014 fast form) — parametric cross-check on the single-AUC CI and a z-test on Δ treating ID/OOD as independent samples. *Oracle test:* reproduce a published worked DeLong variance to tolerance.
- **Spearman rank correlation** (+ asymptotic p) — the monotone-trend test for H1. *Oracle test:* ρ = 1 for a monotone sequence; reproduce a textbook ρ on tied data.
- **Ridge regression** (closed form `(XᵀX + λI)⁻¹Xᵀy`) — the gap predictor. *Oracle test:* recovers known coefficients on a synthetic linear system; λ = 0 matches OLS.

## 5. The gap predictor (headline contribution)

**Goal:** predict `Δ(d, c)` (equivalently `AUC_out`) **without any OOD labels**, from statistics of the ID corpus alone.

**ID-only features per (detector, class):**
- `AUC_in` (the in-distribution per-class AUC);
- `d'` estimate `= (mean_pos − mean_neg)/pooled_sd` of the ID scores;
- ID score-distribution **overlap** and **variance ratio** (positive vs negative);
- **tail/margin** statistics near the operating threshold (skew/kurtosis of the score gap);
- per-class **Pd at target Pfa** on ID.
- *(Ablation only, clearly flagged:* a self-generated micro-perturbation slope `∂AUC/∂s` near `s=1` — uses the generator, no OOD labels — reported separately so the core claim rests on pure ID-distribution statistics.)*

**Model:** ridge regression (interpretable, hand-testable) predicting `Δ`. Report learned coefficients (which ID signals predict fragility).

**Validation:** leave-one-**detector**-out and leave-one-**class**-out CV → predict `Δ` for the held-out detector/class never seen in training. Metrics: R², RMSE, predicted-vs-actual scatter, vs a predict-the-mean baseline. **Dominant result target:** high R² across unseen detectors/classes from a *simple* ID-only model.

**Honesty:** the predictor anticipates the **synthetic** OOD gap; we claim a controlled-study capability + an open instrument, not a field-gap predictor. §6 says how to confirm/refute on real data.

## 6. Honest scope + falsification protocol (load-bearing)

- The corpus is **synthetic, parameter-grounded** (measurement-domain observables composed from Kshana's jamming/AGC/SQM/parity models), **never raw IQ or field captures**. Every AUC is "AUC over model-derived labels," stated as such.
- The OOD axis is a **controlled severity shift**, so the measured `Δ` is **synthetic→synthetic** — a mechanism/regime characterization, **not** the real deployment gap. This is stated as a first-class limitation, not buried.
- **Falsification protocol:** the exact procedure to confirm or refute H1–H4 on a real labelled corpus (e.g. a public field/IQ interference dataset) — same per-class AUC, same shift definition, same predictor features — so the claims are testable by others. We pre-register what result would falsify each hypothesis.
- All in-repo artifacts keep their machine-checked `MODELLED` labels; the `no_overclaims` + `dominance_demonstrators` CI guards must stay green.

## 7. Contributions (as the paper will state them)

1. An **open, reproducible, physically-grounded testbed** for evaluating RF-impairment detectors under controlled distribution shift (the instrument; extends the existing `impairment-eval` scenario).
2. A **characterization (scaling law)** of how evaluation optimism grows with shift magnitude, differs between learned and physics-based detectors, and varies by impairment class — with CIs and significance.
3. **A predictor that anticipates the optimism gap from ID-only statistics** — validated across unseen detectors/classes (the headline).
4. **Honest scoping + a falsification protocol** that makes every claim testable on real data.

## 8. Paper structure (IEEE/ION two-column)

1. **Introduction** — the optimism problem; folklore → measured → predicted; contributions.
2. **Related work** — sim-to-real gap (Fraunhofer IIS / Heublein–Ott); controlled robustness benchmarks (ImageNet-C); AUC & DeLong; GNSS interference detection (Kaplan & Hegarty; integrity literature).
3. **The testbed** — corpus, observables, detector panel (incl. learned); honest scope.
4. **Methods** — optimism-gap definition; experiment design; statistics; the gap predictor.
5. **Results** — scaling law (H1); learned-vs-physics (H2); per-class (H3); **predictor CV (H4, headline)**.
6. **Limitations & falsification protocol** — synthetic→synthetic; how to test on real data.
7. **Reproducibility** — one-command artifact; scenario+seed+engine; SBOM/SLSA.
8. **Conclusion.**

Target length ~8–10 pages two-column; ~25–35 references.

## 9. Reproducibility artifact

Every figure/number regenerable from one command (a Kshana bin/example or `cargo test`-gated experiment), pinned by `scenario + seed + engine version`, cross-platform golden-hash gated, results emitted as versioned self-describing JSON. The released testbed is the citation magnet.

## 10. Risks & mitigations

| Risk | Mitigation |
|---|---|
| "Synthetic, so what?" | ImageNet-C-style framing + falsification protocol; claim mechanism+predictability, not field magnitude. |
| "Tooling repackage, no new science" | The **predictor** (H4) is the new science; scaling-law CIs + learned-vs-physics finding are genuine results. |
| "Δ is just AUC_in − AUC_out, trivial" | Novelty is the *predictability* and the controlled *scaling law*, not the metric. |
| Learned detectors don't actually overfit (H2 fails) | That is itself a publishable, honest finding; report it. |
| ID/OOD split leakage attacked | Strengthened near-duplicate-parameter leakage guard (built first). |
| Overclaim creep | CI honesty guards stay green; every artifact MODELLED-labelled. |

## 11. Build plan

See `PLAN-impairment-optimism-gap.md` for the TDD task breakdown (open-repo code), then drafting (this private folder), then an adversarial multi-agent review pass.
