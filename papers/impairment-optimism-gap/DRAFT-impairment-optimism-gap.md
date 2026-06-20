# Anticipating the Optimism Gap: An In-Distribution Signal for Distribution-Shift Degradation of RF-Impairment Detectors

**Chakshu Baweja**
Ashforde OÜ — contact@ashforde.org

> **Status:** PRIVATE working draft. **The canonical, submission-formatted manuscript is now `paper.tex` (compiled `paper.pdf`)** — human-prose, em-dash-free, 23 references, two-column IEEEtran. This markdown is the extended working version kept for reference; prefer `paper.tex` for any wording/submission. Both carry the same numbers. All numbers come from `cargo run --release --example optimism_study` at engine version `kshana 0.19.0`, seeds `{1,2,3,4,5}`; figures from `make_figures.py`. **Nothing is submitted or pushed without explicit founder go.**

---

## Abstract

Machine-learning detectors for GNSS/PNT radio-frequency (RF) impairments — jamming, spoofing, multipath — are routinely reported with high in-distribution (ID) area-under-the-ROC-curve (AUC), yet they degrade under deployment shift. Estimating a model's out-of-distribution (OOD) performance without OOD labels is an active topic in image classification ("accuracy on the line," agreement-on-the-line, ATC). We bring that question to RF-impairment detection and ask a sharper variant: can the *optimism gap* `Δ = AUC_in − AUC_out` be anticipated **for a detector, or an impairment class, never seen during fitting** — transfer across *estimands*, not just across *distributions*. Using an open, parameter-grounded synthetic testbed with controlled severity shift, over **thirteen** detectors (five transparent physics baselines, full-feature logistic-regression and four MLP capacities, and three single-feature learned controls) and four impairment classes, we find: (H1) a predominantly monotone severity→gap relationship (mean Spearman ρ = 0.50; 42/52 trends positive, 30 significant; mean ΔAUC rising 0.002→0.064 as the shift deepens); (H2) the gap is set by **evidence breadth, not learning** — a single-feature learned detector and the physics baseline reading the same observable have *identical* gaps, while multi-feature learned detectors degrade far less (mean ΔAUC 0.013 vs 0.029 physics / 0.033 single-feature learned); (H3) marked class heterogeneity, with the deliberately-hard matched-power spoof-time class showing the smallest gap; and (H4, headline) an ID-only ridge model predicts the held-out gap for an unseen detector (leave-one-detector-out R² = 0.47) and an unseen class (leave-one-class-out R² = 0.46), both significant under a 2000-fold label-permutation null (p < 0.001), and the predictability **survives removing `auc_in`** (which is itself one term of the target): the shape-only predictor still reaches R² = 0.34 (detector) and 0.27 (class), so the result is not a definitional artefact. We are explicit that this characterises **mechanism and regime on synthetic data over five seeds**, not the absolute field gap, and we give a pre-registered falsification protocol naming concrete public datasets for confirming or refuting every hypothesis on real labelled data.

---

## 1. Introduction

It is folklore in GNSS interference detection that ID AUC overstates field performance for learned detectors; recent work documents real-world data discrepancies for learned GNSS interference classifiers (e.g. the Fraunhofer IIS group, 2024–25; van der Merwe et al. / Ott et al. on real-world discrepancies, arXiv:2503.23775). Separately, a substantial machine-learning literature *predicts* a classifier's OOD accuracy without OOD labels — "accuracy on the line" (Miller et al., 2021), agreement-on-the-line (Baek et al., 2022), Average Thresholded Confidence (Garg et al., 2022), and regression on dataset statistics (Deng & Zheng, 2021). What is missing is (a) a *reproducible, open, controlled* characterisation of how evaluation optimism behaves in **RF-impairment detection**, on **AUC** rather than accuracy, and (b) a test of whether the gap transfers across **detectors and impairment classes**, not merely across input distributions for a fixed model.

We adopt the methodological stance of ImageNet-C (Hendrycks & Dietterich, 2019): a controlled benchmark isolates a variable messy real data cannot — while being explicit (unlike ImageNet-C, which corrupts *real* images) that our corpus is **fully parametric**, so we claim mechanism/regime and validate transfer only via a falsification protocol.

**Contributions.**
1. An open, reproducible, parameter-grounded **testbed** for evaluating RF-impairment detectors under controlled distribution shift.
2. A **characterisation** of how evaluation optimism grows with shift magnitude, is governed by detector evidence breadth, and varies by impairment class — with confidence intervals, a permutation null, and a matched-dimensionality control.
3. The headline: **predicting the optimism gap from ID-only statistics, and showing it transfers to held-out detectors and held-out impairment classes** — a transfer across *estimands* that the accuracy-prediction literature (which fixes the task and model and predicts across input distributions) does not address. The signal is significant (permutation p < 0.001) and survives excluding the one feature that is definitionally part of the target.
4. **Honest scoping and a pre-registered falsification protocol** that name concrete public datasets and make every claim testable on real data.

---

## 2. Related work

**Predicting performance under distribution shift.** A substantial literature estimates a model's OOD performance without OOD labels: Miller et al. (2021) show a strong linear ID↔OOD accuracy correlation ("accuracy on the line"); Baek et al. (2022) predict OOD accuracy from *ID-only* model agreement ("agreement on the line"); Garg et al. (2022) use Average Thresholded Confidence (ATC) and prove fundamental limits; Deng & Zheng (2021) regress accuracy on dataset-level feature statistics; Guillory et al. (2021) use difference-of-confidences; calibration-under-shift is studied by Ovadia et al. (2019). **Our relation, stated precisely:** that line predicts *accuracy* for a *fixed task and model* across *input distributions*. We differ in three ways that survive a hostile read — (i) the target is **AUC/ROC** (operating-point-free, the right metric for detection), (ii) the features are detection-native (tail margin between score tails, `Pd` at a target `Pfa`), and (iii) we test transfer across **held-out detectors and held-out impairment classes** (across estimands), not across input distributions for one model. We do not claim to be first to predict OOD performance from ID statistics; we claim the cross-detector / cross-class transfer in RF-impairment AUC is new and useful.

**GNSS interference detection and the real-world gap.** Learned GNSS interference detectors are an active area (surveys and works using C/N₀, AGC, and spectral features); their degradation under real-world data discrepancies is documented (arXiv:2503.23775; Fraunhofer IIS releases). Our corpus is a fully simulated benchmark; we situate it with synthetic-robustness / domain-randomisation work and explicitly distinguish our task from **OOD-detection** (flagging individual anomalous inputs), which it is not.

**ROC/AUC inference.** The Mann–Whitney AUC and its variance (DeLong, DeLong & Clarke-Pearson, 1988; fast form Sun & Xu, 2014) underpin our single-AUC intervals; every statistical routine is cross-checked against a closed form.

---

## 3. The testbed

**Corpus.** A class-balanced, labelled, **parameter-grounded** synthetic corpus over five measurement-domain observables a real receiver exposes — jammer-to-signal ratio, effective-C/N₀ drop, AGC power excess, Early/Late SQM imbalance, RAIM parity statistic — composed from the engine's jamming, AGC, SQM and parity models with seeded Gaussian measurement noise. **No raw IQ or field captures.** Five classes: Nominal, Jamming, SpoofTime (matched-power, deliberately the hard near-nominal case), SpoofPosition, Multipath. A scalar `severity_scale ∈ (0,1]` multiplies impaired-class severity: `s = 1.0` is the ID regime; `s < 1` produces subtler impairments. `s = 1.0` reproduces the ID corpus bit-for-bit.

**Detector panel (13).** Four published-method baselines — energy(C/N₀-drop), AGC-excess, SQM-imbalance, RAIM-parity — plus a transparent in-house max-z combiner (the five "physics" detectors); full-feature learned detectors — logistic regression (deterministic full-batch gradient descent over standardised features) and four one-hidden-layer ReLU→sigmoid MLPs at capacities {4, 8, 16, 32} (seeded backpropagation SGD); and three **single-feature learned controls** — logistic regression restricted to one observable each (cn0-drop, agc, parity), at matched input dimensionality to the single-observable physics baselines (the H2 control). Learned detectors train on an ID training split only.

**Honest scope (load-bearing).** Every AUC is an AUC *over model-derived labels on synthetic data*. The corpus is a separability/pipeline harness, not a difficulty benchmark; a high ID AUC shows the pipeline reads the right observable — **not** field performance. Two leakage guards protect the partition: an exact-key guard and a near-duplicate observable guard (per-component L∞, class-gated), asserted on every training split.

---

## 4. Methods

**Per-class AUC and the optimism gap.** For detector `d`, class `c`, severity `s`, let `AUC_c(d,s)` be the Mann–Whitney AUC of `d`'s scores on class-`c` positives vs Nominal negatives at `severity_scale = s`. `AUC_c(d,1.0)` uses a held-out ID test split (for learned and physics detectors alike). The **optimism gap** is `Δ(d,c,s) = AUC_c(d,1.0) − AUC_c(d,s)`, `s < 1`.

**Hypotheses (falsifiable).** H1: `Δ` increases with `(1 − s)` (Spearman ρ per `(d,c)`). H2: detector family / evidence breadth governs `Δ` (matched-dimensionality control). H3: `Δ` differs by class. H4 (headline): an ID-only regressor predicts held-out `Δ` across unseen detectors and unseen classes (leave-one-out CV, R² vs predict-the-mean, with a label-permutation null).

**Experiment grid.** Detectors (13) × classes (4) × severities `s ∈ {0.2, 0.4, 0.6, 0.8}` × **seeds `{1..5}`** (a small-N replication — see Limitations), `n_per_class = 400`, `frac_train = 0.7`. Per seed: generate the ID corpus, stratify-split (asserting the near-duplicate guard), train learned detectors on the ID train split, measure held-out ID per-class AUC, then per severity generate a fresh independent OOD corpus and record `Δ`. Aggregated per `(d,c,s)`: mean `Δ`, a percentile-bootstrap CI (B = 2000) over the five seed values, and the across-seed standard error; per `(d,c)` a scaling trend (Spearman ρ + p, OLS slope, and the mean ID AUC).

**Statistics (hand-derived, oracle-tested).** Mann–Whitney AUC (checked vs the binormal form `Φ(d′/√2)`); percentile bootstrap (checked to bracket a known mean/AUC) — note the per-cell gap CIs are seed-bootstraps, *not* DeLong; DeLong variance/Wald CI (checked against a hand-worked Var = 1/18) is available for single-AUC inference; Spearman ρ with a large-sample p (checked: ρ = ±1 monotone/reversed, textbook tied ρ = 0.9487); ridge via the normal equations with an unpenalised intercept (checked: exact OLS recovery, λ = 0 = OLS, heavy-λ shrinkage). No third-party statistics dependency.

**The gap predictor (headline).** Six **ID-only** features per `(detector, class)` from the held-out ID test set — `AUC_in`; the `d′` separation `(μ_pos − μ_neg)/σ_pooled`; the score-distribution overlap; the positive/negative variance ratio; the tail margin `(q05_pos − q95_neg)/σ_pooled`; and `Pd` at the target `Pfa` — plus, separately and ablatably, a self-perturbation slope from a mild self-imposed micro-shift (independently-seeded probe corpora; no OOD labels). The target is the realised gap (mean `Δ` over the severity sweep). A ridge regressor (features standardised **on the training fold only**, applied to the held-out fold; λ = 0.1 fixed a priori, not tuned on test folds) predicts the gap; we validate with leave-one-detector-out and leave-one-class-out CV vs a predict-the-mean baseline (out-of-fold R²), and a 2000-fold label-permutation null for significance. Because the target `Δ = AUC_in − mean_OOD` literally contains `AUC_in`, we additionally report a **shape-only** predictor with `AUC_in` removed, whose survival establishes non-tautological predictability.

---

## 5. Results

All numbers from the released artifact (`optimism-study/1`): 13 detectors, 208 cells, 52 trends, 260 predictor rows, 5 seeds. Figures are generated from the artifact by `make_figures.py`.

### 5.1 H1 — a (predominantly) monotone scaling trend (Fig. 1)

The gap grows as the OOD regime gets subtler. Pooled mean ΔAUC by severity scale:

| `s` | 0.8 | 0.6 | 0.4 | 0.2 |
|---|---|---|---|---|
| mean ΔAUC | 0.002 | 0.010 | 0.019 | 0.064 |

Across the 52 `(detector, class)` trends the Spearman ρ between `(1 − s)` and `Δ` has mean 0.50; **42/52 are positive, 30/52 positive at p < 0.05**. We call this a scaling *trend*, not a law: the non-positive trends coincide with a near-chance ID AUC (the artifact records each trend's `id_auc_mean`), i.e. off-target detectors with no separability to lose. The monotonicity itself is unsurprising — attenuating the observable a detector reads must lower its AUC; the interesting, non-trivial part (and what enables §5.4) is the **cross-detector variation in the rate of collapse** at matched ID AUC.

### 5.2 H2 — evidence breadth, not learning (Fig. 2)

Mean ΔAUC pooled over classes and severities, by family:

| family | mean ΔAUC |
|---|---|
| physics baselines | 0.029 |
| single-feature learned controls | 0.033 |
| full-feature learned (logreg + MLPs) | 0.013 |

The decisive evidence is the **matched-dimensionality control**: a single-feature learned detector and the physics baseline reading the *same* observable have **identical** optimism gaps —

| observable | physics ΔAUC | single-feature learned ΔAUC |
|---|---|---|
| C/N₀-drop | 0.025 | 0.025 |
| AGC-excess | 0.051 | 0.051 |
| RAIM-parity | 0.023 | 0.023 |

(A one-feature logistic detector is a monotone transform of that observable, so its per-class AUC — and hence its gap — equals the physics detector's exactly.) The conclusion is unambiguous: **the optimism gap is governed by evidence breadth, not by whether the detector is learned.** Narrow-evidence detectors (single physics observable, or single-feature learned) have large gaps; the multi-feature learned detectors integrate all five observables and degrade far less. This sharpens, rather than contradicts, the field folklore: over-fitting to the tuning regime is a property of narrow-evidence detectors. (Real learned models can additionally overfit nuisance correlations a controlled generator lacks; §6 pre-registers that outcome.)

### 5.3 H3 — class heterogeneity (Fig. 3)

| class | spoof-position | multipath | jamming | spoof-time |
|---|---|---|---|---|
| mean ΔAUC | 0.030 | 0.029 | 0.025 | 0.011 |

The matched-power **spoof-time** class — deliberately near-nominal — shows the smallest gap: its ID separability is already modest, so there is less optimism to deflate. The classes with the most impressive ID numbers have the most to lose — a cautionary ordering for practitioners who rank detectors on ID AUC.

### 5.4 H4 — predicting the gap from ID-only statistics (headline, Fig. 4)

An ID-only ridge regressor predicts the held-out optimism gap for a detector — and for a class — **never seen in training**:

| CV protocol | folds | R² | RMSE | permutation-null p (B=2000) |
|---|---|---|---|---|
| leave-one-detector-out | 13 | **0.47** | 0.028 | **< 0.001** |
| leave-one-class-out | 4 | **0.46** | 0.029 | **< 0.001** |

Both beat the predict-the-mean baseline (R² = 0), and both are significant: across 2000 label permutations none reached the observed R² (p ≈ 5×10⁻⁴). The cross-detector result strengthened markedly with the wider panel (13 folds vs the 7-detector pilot's R² = 0.26), which is the expected behaviour of a leave-one-out estimate as the number of folds grows.

**The result is not a tautology of `AUC_in`.** Because `Δ = AUC_in − mean_OOD`, `AUC_in` is one additive term of the target, and it is the largest standardised coefficient (+0.13). Re-fitting with `AUC_in` **excluded**, the shape-only predictor *survives*: leave-one-detector-out R² = **0.34** and leave-one-class-out R² = **0.27**. So genuine, non-definitional predictability resides in the score-distribution **shape**. Ablating instead the one generator-touching feature (self-perturbation slope) barely moves the full result (0.47→0.45, 0.46→0.43), so the headline does not rest on probing the simulator either.

Among the shape features, the largest standardised coefficients are the score-distribution overlap (+0.07; more ID overlap ⇒ larger gap) and the tail margin (−0.03; a wider ID margin ⇒ a smaller gap). Consistent with a fragility pattern: **a high ID AUC achieved on a heavily-overlapping, thin-margin score distribution is associated with a larger optimism gap**, and that pattern is visible before any OOD data is seen.

---

## 6. Limitations and falsification protocol

**Limitations.** (1) The corpus is **synthetic and parameter-grounded** end to end — unlike ImageNet-C, which corrupts real images — so we claim mechanism/regime, not the field gap; every AUC is "AUC over model-derived labels." (2) The OOD axis is a controlled severity shift, so `Δ` is **synthetic→synthetic**. (3) Replication is **only 5 seeds and 4 severity levels**, so the per-cell bootstrap CIs and across-seed SEs are wide and should be read as a 5-seed spread; the cross-class result rests on 4 folds. (4) The single-feature learned controls equal the physics baselines *by construction* (monotone transform), so the matched-dimensionality control is a demonstration of the mechanism, not an independent measurement. These are stated as first-class limitations.

**Path to real-data validation.** Kshana is a simulator, not a receiver: its harness consumes measurement-domain observables (C/N₀-drop, AGC excess, SQM imbalance, RAIM parity, J/S), so a faithful real-data probe requires a labelled corpus *in that schema* with a defined shift axis. Concrete public candidates — the **GNSS Interference Spectrum Highway Dataset 1** (IEEE DataPort; 8 interference + 3 non-interference classes) and **dual-frequency C/N₀-heatmap** interference datasets — provide spectrograms or C/N₀ series and would need a receiver-domain **feature-extraction front-end** (spectrogram/C/N₀ → the five observables) before the harness can score them. Building that front-end (or sourcing a dataset already in measurement-domain schema) is the explicit next step; we did **not** run a real-data probe and do **not** report one, rather than present a feature-mismatched or fabricated result.

**Falsification protocol (pre-registered).** On such a real labelled corpus, with the shift axis, sample sizes, and primary endpoint fixed *before* unblinding, recompute:
- **H1** — Spearman ρ between shift magnitude and `Δ` per `(d,c)` with ID AUC > 0.6. *Refuted* if fewer than half of those trends are positive, or the mean ρ ≤ 0 (α = 0.05).
- **H2** — `Δ` by family with bootstrap CIs and the matched-dimensionality control. Our claim (evidence breadth governs the gap; multi-feature degrade less) is *refuted* if, on real data, multi-feature learned detectors show a larger `Δ` CI than narrow-evidence detectors at matched ID AUC — the folklore-expected outcome, which we would report as the more interesting result.
- **H3** — the per-class `Δ` ordering. *Refuted* if the rank correlation between the real-data class ordering and ours is ≤ 0.
- **H4 (primary endpoint)** — leave-one-detector-out and leave-one-class-out R² of the *same* ID-only features, with the same permutation null. *Refuted* if either out-of-fold R² fails to beat predict-the-mean at permutation p < 0.05.

All in-repo artifacts keep machine-checked `MODELLED` labels; the no-overclaim and demonstrator CI guards remain green.

---

## 7. Reproducibility

Every number regenerates from one command:

```
cargo run --release --example optimism_study -- paper-artifacts/optimism-study.json
python3 make_figures.py        # the four figures, from that artifact
```

The artifact is versioned, self-describing JSON carrying every aggregate, the scaling-law trends (ρ/p/slope and each trend's mean ID AUC), the learned-vs-physics gaps and the matched-dimensionality pairs, the predictor's CV (R²/RMSE plus the full predicted-vs-actual scatter), the permutation-null p-values, the **shape-only (no `auc_in`)** and no-self-slope ablations, the coefficients (intercept-labelled), and provenance: engine name and version, config hash, seeds and full configuration. Statistical routines are unit-tested against closed forms; the corpus is bit-for-bit reproducible from `(config, seed)`. (CI tests exercise a smaller grid; the artifact config — MLP capacities {4,8,16,32}, 1200 epochs, 5 seeds — is the one behind these numbers.) Code is released under AGPL-3.0 in the `kshana` crate.

---

## 8. Conclusion

Evaluation optimism in RF-impairment detection is usually a disclaimer. We make it measurable, and — the headline — *anticipable*: from in-distribution statistics alone, a simple model forecasts how much a detector's reported AUC will deflate under controlled shift, and the signal transfers to detectors and impairment classes it has never seen (R² = 0.47 / 0.46, permutation p < 0.001), surviving removal of the one feature definitionally part of the target. A matched-dimensionality control shows the gap is governed by detector evidence breadth, not by learning. The result is honestly scoped to synthetic mechanism over five seeds, positioned against the accuracy-prediction literature, and shipped with a pre-registered falsification protocol naming concrete public datasets. The practitioner takeaway is concrete: **a high in-distribution AUC bought on a thin, heavily-overlapping score margin is a warning sign, readable before deployment.**

---

## References (to be completed in submission format)

1. J. P. Miller et al., "Accuracy on the Line: On the Strong Correlation Between Out-of-Distribution and In-Distribution Generalization," ICML 2021.
2. C. Baek, Y. Jiang, A. Raghunathan, Z. Kolter, "Agreement-on-the-Line: Predicting the Performance of Neural Networks under Distribution Shift," NeurIPS 2022.
3. S. Garg, S. Balakrishnan, Z. Lipton, B. Neyshabur, H. Sedghi, "Leveraging Unlabeled Data to Predict Out-of-Distribution Performance" (ATC), ICLR 2022.
4. W. Deng and L. Zheng, "Are Labels Always Necessary for Classifier Accuracy Evaluation?," CVPR 2021.
5. D. Guillory et al., "Predicting With Confidence on Unseen Distributions," ICCV 2021.
6. Y. Ovadia et al., "Can You Trust Your Model's Uncertainty? Evaluating Predictive Uncertainty Under Dataset Shift," NeurIPS 2019.
7. D. Hendrycks and T. Dietterich, "Benchmarking Neural Network Robustness to Common Corruptions and Perturbations," ICLR 2019.
8. (Ott / van der Merwe et al.), "Evaluation of (Un-)Supervised Machine Learning Methods for GNSS Interference Classification with Real-World Data Discrepancies," arXiv:2503.23775, 2025.
9. GNSS Interference Spectrum Highway Dataset 1, IEEE DataPort. *(dataset; exact citation to be finalised.)*
10. E. R. DeLong, D. M. DeLong, D. L. Clarke-Pearson, "Comparing the Areas under Two or More Correlated ROC Curves," *Biometrics*, 1988.
11. X. Sun and W. Xu, "Fast Implementation of DeLong's Algorithm…," *IEEE Signal Processing Letters*, 2014.
12. E. D. Kaplan and C. J. Hegarty, *Understanding GPS/GNSS: Principles and Applications*, 3rd ed., Artech House.
13. T. Gneiting and P. Vogel, "Receiver Operating Characteristic (ROC) Curves," arXiv:1809.04808, 2018.
14. *(GNSS integrity / RAIM-ARAIM, SQM, AGC monitoring literature — to be filled.)*

---

### Open items before submission
- [ ] Finalise the Fraunhofer IIS / arXiv:2503.23775 and IEEE DataPort dataset citations in submission format.
- [x] Generate the four figures from the artifact (`make_figures.py` → `figures/`).
- [x] Expand the detector panel (now 13) and add the matched-input-dimensionality H2 control.
- [ ] **Real-data probe**: build (or source) a receiver-domain feature front-end so the H4 ID-only features can run on a public dataset (Spectrum Highway / C/N₀-heatmap). Required for IEEE PLANS / TAES; arXiv (eess.SP) is reachable with the synthetic study + protocol alone. **Founder-gated** (dataset access / front-end build).
- [ ] Compile the two-column LaTeX (`paper.tex`) once a TeX toolchain is available; trim to ~8–10 pages.
- [ ] Founder go before any arXiv/venue submission.
