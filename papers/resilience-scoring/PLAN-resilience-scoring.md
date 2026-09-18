# PLAN — resilience scoring engine + instability study (kshana OPEN, TDD)

> Implements `DESIGN-resilience-scoring.md`. All code in **public `kshana`** on branch `resilience-scoring` (off `main`); paper in this PRIVATE folder. **TDD throughout** (failing test → minimal impl → pass). Targeted dev tests (`cargo test --lib resilience::`); final full `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --all`. **Nothing pushed.** Run `scripts/check-no-attribution.sh` before any commit.

**Deps already in tree:** `rand 0.8`, `rand_chacha 0.3`, `rand_distr 0.4` (Dirichlet/Gamma), `serde`/`serde_json`, `sha2 0.10` + `hex 0.4` (integrity hash). No new crates.

**Module layout (dir module like `src/fusion/`):**
```
src/resilience/mod.rs       // re-exports; module doc with honesty scope
src/resilience/stats.rs     // simplex sampling + rank-stability metrics
src/resilience/arch.rs      // PntArchitecture + framework enums
src/resilience/score.rs     // per-dimension sub-scores + composite + RPCF Level
src/resilience/timeline.rs  // detect/react/recover/loss + bounded/unbounded
src/resilience/diversity.rs // common-mode / SPOF / diversity
src/resilience/study.rs     // instability experiment (H1/H2/H3)
src/resilience/report.rs    // integrity-hashed assurance report (HTML/JSON)
examples/resilience_report.rs
```
`src/lib.rs`: add `pub mod resilience;`.

**Conventions:** every numeric routine has a hand-derived oracle test; deterministic/seeded (`ChaCha8Rng`); all scores in `[0,1]`; every `DimensionScore` carries a `VerificationStatus` + `OracleKind` (reuse `crate::verification`) so the modelled-majority is on the face of the data. Oracle-test style: see `src/allan.rs`, `src/fom.rs` on this branch.

---

### T1 — `resilience::stats` (no deps on other new modules)
**File:** `src/resilience/stats.rs`

Signatures:
```rust
pub fn dirichlet_weights(alpha: &[f64], seed: u64) -> Vec<f64>; // sums to 1, all >=0
pub fn kendall_tau(a: &[f64], b: &[f64]) -> f64;                // [-1,1]
pub fn rank_of(scores: &[f64]) -> Vec<usize>;                   // 0 = best (highest score)
pub fn top1_flip_rate(rankings: &[Vec<usize>]) -> f64;          // vs modal top
pub fn rank_ranges(rankings: &[Vec<usize>], n_items: usize) -> Vec<(usize, usize)>;
pub fn percentile_ci(samples: &[f64], alpha: f64) -> (f64, f64);
```
Oracle tests (hand-derived):
- [ ] `dirichlet_weights`: sum == 1 (to 1e-12); all ≥ 0; **seeded determinism** (same seed → identical vec); with `alpha=[K,K,K]` large K, mean of each component over many seeds ≈ 1/3 (±0.02).
- [ ] `kendall_tau`: identical order → +1; reversed → −1; hand example `a=[1,2,3,4]`, `b=[1,2,4,3]` → 1 discordant of 6 pairs → τ = (5−1)/6 = **0.6667** (±1e-9).
- [ ] `rank_of`: `[0.3,0.9,0.5]` → ranks `[2,0,1]`; ties broken by index, stable.
- [ ] `top1_flip_rate`: all rankings share top → 0.0; half differ → 0.5.
- [ ] `rank_ranges`: item that is best once and worst once → `(0, n-1)`.
- [ ] `percentile_ci`: brackets the median of `1..=100`.

### T2 — `resilience::arch`
**File:** `src/resilience/arch.rs`

```rust
pub enum SourceKind { GnssL1, GnssL5, GnssMultiBand, Inertial, Clock, Terrain, Gravity, Magnetic, SignalOfOpportunity, Eloran }
pub enum TechniqueCategory { Obfuscate, Limit, Verify, Isolate, Diversify, Mitigate, Recover } // DHS RPCF 7; "OLVIDMR" = our mnemonic (doc-comment it)
pub enum RdrrFunction { Resist, Detect, Respond, Recover }
pub enum YangCriterion { Availability, Reliability, Continuity, Accuracy }
pub struct PntSource { pub kind: SourceKind, pub independence_group: u32, pub quality: f64 /*0..1*/ }
pub struct PntArchitecture { pub name: String, pub sources: Vec<PntSource>, pub techniques: BTreeSet<TechniqueCategory> }
impl PntArchitecture {
    pub fn independent_group_count(&self) -> usize;
    pub fn has(&self, t: TechniqueCategory) -> bool;
}
```
Oracle tests:
- [ ] `independent_group_count`: 3 sources groups `{1,1,2}` → 2.
- [ ] `has`: technique-set membership correct.
- [ ] doc-comment states "OLVIDMR is a kshana mnemonic; DHS RPCF names the 7 categories Obfuscate/Limit/Verify/Isolate/Diversify/Mitigate/Recover".

### T3 — `resilience::score` (depends T2)
**File:** `src/resilience/score.rs`

A documented, defensible reduction from a per-architecture, per-scenario **behaviour summary** to per-dimension sub-scores. `SimSummary` is the honest seam: parameter-grounded behaviour (holdover, availability-under-attack, detector AUC, integrity, security), every field Modelled unless sourced from a Validated kshana path.
```rust
pub struct SimSummary { pub holdover_s: f64, pub availability: f64, pub detect_auc: f64,
                        pub integrity: f64, pub security: f64, pub bounded: bool }
pub struct DimensionScore { pub value: f64, pub status: VerificationStatus, pub oracle_kind: OracleKind, pub basis: String }
pub struct ResilienceProfile {
    pub rpcf: BTreeMap<TechniqueCategory, DimensionScore>,
    pub rdrr: BTreeMap<RdrrFunction, DimensionScore>,
    pub yang: BTreeMap<YangCriterion, DimensionScore>,
    pub level: u8, pub level_basis: String,
}
pub fn score(arch: &PntArchitecture, sim: &SimSummary) -> ResilienceProfile;
pub fn composite(profile: &ResilienceProfile, dim_weights: &[f64]) -> f64; // weighted mean of the 7 RPCF sub-scores, [0,1]
pub fn assign_level(profile: &ResilienceProfile, bounded: bool) -> (u8, String);
```
Documented mapping (each sub-score in `[0,1]`, monotone in its driver, MODELLED):
- Verify ← `detect_auc` (and integrity); Diversify ← `independent_group_count` (normalized) ; Recover ← `holdover_s` (saturating) + `bounded`; Mitigate ← `availability`; Isolate/Limit/Obfuscate ← technique presence × quality. RDRR/Yang are re-projections of the same drivers (documented).
- `assign_level`: Level 0..4 by threshold ladder on min sub-score + the **bounded** flag as the L2→L3 gate (unbounded degradation caps at ≤ Level 2).

Oracle tests:
- [ ] Monotonicity: raising `detect_auc` strictly raises the Verify sub-score, others fixed.
- [ ] `composite` bounds: all sub-scores 1.0 → composite 1.0; all 0.0 → 0.0; hand-weighted example matches `Σ w_i s_i` to 1e-12.
- [ ] `assign_level`: an architecture with strong sub-scores but `bounded=false` is capped at Level ≤ 2; same scores with `bounded=true` reaches the higher level.
- [ ] every `DimensionScore` from a Modelled driver carries `status=Modelled`, `oracle_kind != NoneKind`.

### T4 — `resilience::timeline` (depends nothing new)
**File:** `src/resilience/timeline.rs`
```rust
pub struct ResilienceTimeline { pub detect_s: f64, pub react_s: f64, pub recover_s: f64, pub loss_duration_s: f64, pub bounded: bool }
pub fn timeline(series: &[(f64 /*t*/, f64 /*err*/)], threshold: f64, react_lag: f64, horizon: f64) -> ResilienceTimeline;
pub fn bounded_verdict(series: &[(f64, f64)], threshold: f64, horizon: f64) -> bool;
```
Oracle tests (hand-built series):
- [ ] series breaches at t=10, recovers below threshold at t=40 → `detect_s=10`, `loss_duration_s=30`, `bounded=true`.
- [ ] monotone-growing error never recovering before `horizon` → `bounded=false`, `recover_s = horizon` (or NaN-guarded sentinel, documented).
- [ ] never breaches → `detect_s=0`, `loss_duration_s=0`, `bounded=true`.

### T5 — `resilience::diversity` (depends T2)
**File:** `src/resilience/diversity.rs`
```rust
pub fn effective_diversity(arch: &PntArchitecture) -> f64;            // inverse-Simpson over independence groups (Hill N2)
pub fn common_mode_loss(arch: &PntArchitecture, defeated: &BTreeSet<SourceKind>) -> f64; // fraction of weighted capability lost
pub fn spofs(arch: &PntArchitecture, viability: f64) -> Vec<u32>;     // groups whose removal drops effective_diversity below viability
```
Oracle tests (hand-computed):
- [ ] 3 sources, groups `{1,2,3}`, equal quality → inverse-Simpson = **3.0**; all in group `{1}` → **1.0**.
- [ ] `common_mode_loss`: all-GNSS arch, defeated = {all GNSS kinds} → 1.0; arch with 1 of 2 equal sources defeated → 0.5.
- [ ] `spofs`: a 2-group arch where removing either drops below viability → both flagged; a 4-independent-group arch at low viability → none.

### T6 — `resilience::study` (depends T1,T2,T3,T5) — HEADLINE
**File:** `src/resilience/study.rs`
```rust
pub struct ScenarioMix { pub name: String, pub per_arch: BTreeMap<String, SimSummary> }
pub struct StudyConfig { pub archs: Vec<PntArchitecture>, pub mixes: Vec<ScenarioMix>,
                         pub n_weight_draws: usize, pub dirichlet_alpha: Vec<f64>, pub seed: u64 }
pub struct InstabilityResult {
    pub top1_flip_rate: f64, pub kendall_tau_mean: f64, pub kendall_tau_ci: (f64,f64),
    pub level_flip_rate: f64, pub rank_ranges: BTreeMap<String,(usize,usize)>,
    pub n_evaluations: usize,
}
pub fn run_instability(cfg: &StudyConfig) -> InstabilityResult;       // H1
pub fn declared_vs_measured(cfg: &StudyConfig) -> Vec<(String,String,bool /*same declared*/, bool /*same bounded*/)>; // H2
pub fn diversity_collapse(archs: &[PntArchitecture]) -> Vec<(String,f64 /*indep-assumed*/, f64 /*common-mode-aware*/)>; // H3
```
Method: for each (weight draw × scenario mix) → composite-score every arch → rank → collect; aggregate top-1 flip rate, pairwise Kendall-τ dispersion (mean + bootstrap CI), Level-flip rate (does an arch's assigned Level change across draws/mixes?), per-arch rank-range.
Oracle tests:
- [ ] **Stability control:** all architectures identical SimSummary across mixes → `top1_flip_rate=0`, `kendall_tau_mean=1`, `level_flip_rate=0`.
- [ ] **Instability witness:** 2 archs constructed so arch A wins under Verify-heavy weights and arch B wins under Diversify-heavy weights → `top1_flip_rate > 0` and rank-range of each = `(0,1)`. Deterministic by seed.
- [ ] `diversity_collapse`: a "redundant" all-GNSS arch shows common-mode-aware diversity << independence-assumed.

### T7 — artifact generator
**File:** `examples/resilience_report.rs`
- [ ] Build a **defensible architecture panel** (e.g. GNSS-only L1; multi-band GNSS; GNSS+INS; GNSS+INS+clock+eLoran; etc.) and a **scenario-mix ensemble** (nominal, wideband jam, spoof, meaconing, combined) with parameter-grounded `SimSummary` per (arch,mix) — each driver value documented + Modelled.
- [ ] Run `run_instability` + `declared_vs_measured` + `diversity_collapse`; write versioned self-describing JSON (all sub-scores, instability metrics, provenance: `engine_version`, seeds, config hash, MODELLED label + synthetic caveat) to a path arg. Manual run: `cargo run --release --example resilience_report -- paper-artifacts/resilience-study.json`.

### T8 — assurance report (integrity-hashed)
**File:** `src/resilience/report.rs`
```rust
pub fn assurance_report_json(profile: &ResilienceProfile, provenance: &Provenance) -> String;
pub fn assurance_report_html(profile: &ResilienceProfile, provenance: &Provenance) -> String;
pub fn integrity_hash(report_bytes: &[u8]) -> String; // sha2 + hex
```
- Report MUST render the **non-certification disclaimer** + the **Validated/Modelled split** on its face; never the words "certified"/"compliant"/"endorsed".
Oracle tests:
- [ ] `integrity_hash` matches a known SHA-256 of a fixed byte string (hand-verified vector).
- [ ] HTML/JSON contain the disclaimer string + per-dimension status tags; contain no banned word ("certified","compliant","endorsed").

### T9 — dispatcher + verification rows + crosswalk
- [ ] `src/api.rs`: add `ScenarioKind::Resilience` (+ `as_str`, `classify`, `list_scenario_kinds`, `run()` dispatch) running a resilience scenario from TOML → JSON/SVG/summary.
- [ ] `src/verification.rs`: add Modelled row(s) — "PNT-resilience composite scoring & instability study" (module `resilience`; oracle = hand-derived per-metric formulas + rank-stability identities; `InternalConsistency`; `Modelled`). Keep `tests/no_overclaims.rs` green (no Validated on modelled outputs).
- [ ] `docs/RESILIENCE-CROSSWALK.md`: kshana capability → RPCF/RDRR/Yang requirement map, each labelled Validated/Modelled; explicit "aligned to, not certified" header.

### T10 — gate (no push)
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all` (long cold build) — all green incl `no_overclaims`.
- [ ] `./scripts/check-no-attribution.sh` → clean.
- [ ] Commit locally on `resilience-scoring` (author `Chakshu Baweja <contact@ashforde.org>`, **no** automated-tool attribution). **Do not push.**

---

### Phase 2 — findings + paper (kshana-pro PRIVATE)
- Run the artifact; extract real H1/H2/H3 numbers + figures.
- Draft per the design (IEEEtran two-column, 0 authored em-dashes, inline `thebibliography`); cite DHS RPCF v2.0, RethinkPNT/Firesmith, Yang, plus measurement/aggregation-instability prior art.
- Multi-lens adversarial review (overclaim / stats / novelty / honesty-scope) → revise.
- Hold for founder go before any push or submission.
