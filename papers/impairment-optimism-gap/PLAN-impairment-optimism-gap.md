# Build plan — optimism-gap study code (kshana OPEN, TDD)

> Implements the code behind `DESIGN-impairment-optimism-gap.md`. All code in the **public `kshana`** repo; the **paper** in this private folder. **TDD throughout** (write failing test → minimal impl → pass). Targeted tests during dev (`cargo test --lib <mod>::`); full `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --all` at the end. **Nothing pushed without explicit go.** Run `scripts/check-no-attribution.sh` before any commit.

**New/modified files**
- `src/impairment_eval.rs` (modify) — strengthen leakage guard; per-class AUC helper.
- `src/eval_stats.rs` (new) — bootstrap CI, DeLong, Spearman, ridge + SPD solver.
- `src/impairment_ml.rs` (new) — standardizer + LogisticRegression + MLP detectors.
- `src/impairment_study.rs` (new) — per-class/optimism-gap, experiment grid, gap predictor + CV.
- `examples/optimism_study.rs` (new) — one-command artifact: writes results JSON.
- `src/lib.rs` (modify) — `mod eval_stats; mod impairment_ml; mod impairment_study;`.
- `src/verification.rs` (modify) — Modelled rows for the new capability; keep guards green.

Design conventions: dependency-light (reuse `rand`/`rand_chacha`/`rand_distr` already in tree; no new crates), deterministic (seeded), every numeric routine has a hand-derived oracle test, all artifacts carry the `MODELLED` label.

---

### T0 — Strengthen the train/test leakage guard
**File:** `src/impairment_eval.rs`
- [ ] Test: build a `Split`, add to `test` a case with the **same class** and severity within `eps` of a `train` case (different key) → `near_duplicate_leakage(eps)` is `true`; a clean `stratified_split` is `false` at a small `eps`.
- [ ] Impl: `Split::near_duplicate_leakage(&self, eps: f64) -> bool` — true if any test case shares class AND `|severity − train.severity| ≤ eps` with some train case. Keep `has_leakage` (exact-key) as-is.
- [ ] Doc: update the module roadmap note — the near-duplicate guard is now implemented (no longer a TODO).

### T1 — `eval_stats`: bootstrap CI
**File:** `src/eval_stats.rs` (new), register in `lib.rs`
- [ ] Test: `bootstrap_auc_ci(pos, neg, 2000, seed, 0.05)` brackets the analytic binormal AUC `Φ(d'/√2)` for separated Gaussians; `lo ≤ point ≤ hi`; degenerate one-class → NaN/handled.
- [ ] Test: generic `bootstrap_ci(&[f64], b, seed, alpha)` brackets the mean of a known sample.
- [ ] Impl: percentile bootstrap, seeded `ChaCha8Rng`; reuse `impairment_eval::auc`.

### T2 — `eval_stats`: DeLong AUC variance + CI
- [ ] Test: reproduce a hand-computed DeLong variance on a small fixed `pos`/`neg` set (placement values worked out by hand); `delong_ci` brackets the AUC; wider when n small.
- [ ] Impl: `delong_auc_variance(pos, neg)` (structural components V10/V01, Sun & Xu fast form) and `delong_ci(pos, neg, alpha)` (logit or normal interval).

### T3 — `eval_stats`: Spearman ρ + p
- [ ] Test: ρ=+1 on a strictly increasing pair; ρ=−1 reversed; reproduce a textbook tied-rank ρ to 1e-9; asymptotic p small for strong monotone, ~1 for shuffled.
- [ ] Impl: rank with tie-averaging; ρ = Pearson on ranks; asymptotic `t = ρ√((n−2)/(1−ρ²))`.

### T4 — `eval_stats`: ridge regression + SPD solver
- [ ] Test (solver): Gaussian-elimination solve of a known SPD system matches the hand solution to 1e-10.
- [ ] Test (ridge): recovers known coefficients on a noiseless synthetic linear system; `λ = 0` equals OLS; `λ → large` shrinks coeffs toward 0.
- [ ] Impl: `ridge_fit(X: &[Vec<f64>], y: &[f64], lambda) -> Vec<f64>` via `(XᵀX + λI)⁻¹Xᵀy`; `ridge_predict(coeffs, x)`. Include intercept column.

### T5 — `impairment_ml`: features + standardizer
**File:** `src/impairment_ml.rs` (new), register in `lib.rs`
- [ ] Test: `features(&CaseObservables)` returns the fixed 5-vector `[cn0_drop_db, agc_excess_db, sqm_el_metric.abs(), parity_stat, js_db]`; `Standardizer::fit(train).transform(x)` gives ~0 mean/unit-var on the train set.
- [ ] Impl: feature map + `Standardizer{mean,std}` (guard std≥1e-9).

### T6 — `impairment_ml`: LogisticRegression detector
- [ ] Test: on a linearly separable synthetic corpus `evaluate(&fitted, corpus, 0.05).auc > 0.95`; deterministic (zero-init or seeded) → identical weights across runs; training loss decreases over epochs.
- [ ] Impl: `LogisticRegression::fit(train: &[LabeledCase], epochs, lr) -> Self` (binary `is_impaired` label, standardized features, full-batch GD); `impl ImpairmentDetector` (`score` = logit).

### T7 — `impairment_ml`: MLP detector
- [ ] Test: 1-hidden-layer MLP separates a non-linear (XOR-like) toy set (AUC high) where logistic regression cannot; reproducible by seed; loss decreases.
- [ ] Impl: `Mlp::fit(train, hidden, epochs, lr, seed) -> Self` (ReLU hidden, sigmoid out, backprop SGD, seeded init); `impl ImpairmentDetector` (`score` = pre-sigmoid logit).

### T8 — `impairment_study`: per-class AUC + optimism gap
**File:** `src/impairment_study.rs` (new), register in `lib.rs`
- [ ] Test: `auc_per_class(det, corpus, class)` uses class positives vs Nominal negatives (sanity: oracle detector → ~1); `optimism_gap` on a subtler OOD corpus is `> 0` for a learned detector overfit to ID.
- [ ] Impl: `auc_per_class`, `optimism_gap(det, in_test, out_corpus, class)`.

### T9 — `impairment_study`: experiment grid + aggregation
- [ ] Test (small grid): detectors × classes × `s∈{0.3,0.6,0.9}` × seeds=3 → result has the right shape; each cell `lo ≤ mean ≤ hi`; Spearman trend present per (d,c).
- [ ] Impl: `run_grid(GridConfig)` looping detectors/classes/`s`/seeds (train learned per seed on ID train split with the T0 guard asserted); aggregate mean Δ + bootstrap & across-seed CIs + Spearman ρ on `(1−s, Δ)`.

### T10 — `impairment_study`: gap predictor + CV (HEADLINE)
- [ ] Test: `id_features(det, in_corpus, class)` returns the ID-only feature vector (AUC_in, d′, overlap, var-ratio, tail stats, Pd@Pfa); `fit_gap_predictor` + `loocv_by_detector`/`loocv_by_class` yield `R² > 0` (beats predict-the-mean) on generated rows; deterministic.
- [ ] Impl: ID-only feature extraction; ridge gap predictor; leave-one-detector-out and leave-one-class-out CV returning `(r2, rmse)` + predicted-vs-actual pairs. (Ablation flag for the self-perturbation slope feature, reported separately.)

### T11 — Results artifact (one command)
**File:** `examples/optimism_study.rs` (new)
- [ ] Impl: run the full grid + predictor; write versioned self-describing JSON (all aggregates, scaling-law slopes + ρ/p, learned-vs-physics gaps + CIs, predictor CV R²/RMSE + scatter, provenance: engine version + seeds + config hash) to a path arg; carry the `MODELLED — synthetic, never field/IQ` label and the synthetic→synthetic caveat. Manual run: `cargo run --release --example optimism_study -- paper-artifacts/optimism-study.json`.

### T12 — Verification matrix + honesty guards
**File:** `src/verification.rs` (+ `docs/CAPABILITY.md`)
- [ ] Add Modelled row(s): the optimism-gap scaling-law study + ID-only gap predictor + learned detectors (ReferenceImpl/InternalConsistency oracle; never Validated). Update CAPABILITY.md.
- [ ] Run `tests/no_overclaims.rs` + `tests/dominance_demonstrators.rs` — must stay green (no `VALIDATED` on modelled study outputs).

### T13 — Gate (no push)
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all` (expect a long cold build) — all green.
- [ ] `./scripts/check-no-attribution.sh` → clean.
- [ ] Commit locally on a branch (author `Chakshu Baweja <contact@ashforde.org>`, **no** automated-tool attribution). **Do not push.**

---

### Phase 2 — write the paper (kshana-pro/papers/, PRIVATE)
- Draft per §8 of the design from the results JSON; generate figures from the artifact.
- Adversarial multi-agent review (overclaim / novelty / stats-rigor / honesty-scope lenses) → revise.
- Hold for founder go before any arXiv/venue submission.
