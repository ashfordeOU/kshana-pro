# Plan: Kshana Pro Overlay → Agency-Dominant

Date: 2026-06-29
Goal: Build the two shared substrates (PackRegistry seam + assurance/provenance/uncertainty spine) and deepen L1 (clock twin) and L3 (trade & assurance) to ESA/NASA-credible, field-dominant depth — coordinated, honesty-firewall-preserving, superset-by-construction.
Architecture: Public `kshana` holds science primitives + registry; private `kshana-pro` holds packs/presets/evidence. Two substrates feed two verticals (L1, L3); L2 stays a fail-closed gate.
Tech stack: Rust (edition 2021, pre-1.0 semver, WASM-safe `no_std`+`alloc` for new public primitives), TDD with known-answer oracles (NBS-140, allantools), Markdown/SVG/PDF evidence packs.
Design ref: `docs/plans/2026-06-29-kshana-pro-overlay-agency-dominant-design.md`

## Conventions
- **TDD everywhere:** write failing test → verify FAIL → implement → verify PASS → commit.
- **Public crate** = `/Users/chak/Documents/Code/Claudecode/spacerepo/kshana` (branch `feat/overlay-registry-assurance`).
- **Private crate** = `/Users/chak/Documents/Code/Claudecode/spacerepo/kshana-pro` (branch `feat/overlay-agency-dominant`).
- Build/test the **public crate** with care — a long build may be running in another terminal; prefer `cargo test -p <crate> <module>` scoped runs, and never `cargo clean`.
- Each task ≈ a single committable change. Phase 0 tasks are fully specified; Phase 1–3 task groups list deliverable + oracle + test and are expanded into micro-tasks just-in-time against the Phase 0 APIs (their exact bodies depend on those APIs).
- Honesty gate: a new number may carry `ValidationTier::Validated` **only** after its oracle is registered in `verification.rs` with `OracleKind::ExternalDataset`. Otherwise `Modelled`.

---

# PHASE 0 — The Spine (public `kshana`)

Branch: `feat/overlay-registry-assurance` in the public crate.

## Substrate B — `PackRegistry` seam

### Task 0.1: New module skeleton `kshana::registry`
**Files:** `kshana/src/registry.rs` (create), `kshana/src/lib.rs` (modify: add `pub mod registry;`)
**Steps:**
1. Failing test (in `registry.rs` `#[cfg(test)]`): `let r = PackRegistry::with_builtins(); assert!(r.contains(&ids::JAMMING));` — expect FAIL (module/type absent → compile error).
2. Implement: `ScenarioId(Cow<'static,str>)` with `from_static`; `ids` module with one `pub const` per current `ScenarioKind` variant (mirror the `as_str` table in `api.rs:431-528`); `ScenarioFactory` trait (object-safe: `id`, `build`, defaulted `metadata`); `PackRegistry { scenarios: BTreeMap<ScenarioId, Box<dyn ScenarioFactory>> }` with `default`, `register`, `build`, `contains`, `ids`; `with_builtins()` initially registering only a `JammingFactory` (the existing `Scenario` impl) as the worked example.
3. Verify: `cargo test -p kshana registry::` → PASS.
4. Commit: `feat(registry): PackRegistry skeleton with ScenarioId + ScenarioFactory`

### Task 0.2: `ScenarioFactory` for every built-in kind via an adapter
**Files:** `kshana/src/registry.rs` (modify)
**Steps:**
1. Failing test: `for id in ids::ALL { assert!(PackRegistry::with_builtins().contains(id)); }` — expect FAIL (only jamming registered).
2. Implement a `FnFactory { id, run: fn(&str)->Result<RunOutput,String> }` adapter whose `build(cfg)` stores the TOML src and returns a `Box<dyn Scenario>` that calls the existing per-kind inline logic. Bridge each `ScenarioKind` arm of `run_toml_inner` to a factory by extracting each arm body into `fn run_<kind>(src:&str)->Result<RunOutput,String>` (mechanical) and registering `FnFactory` for it. Define `ids::ALL: &[ScenarioId]`.
3. Verify: `cargo test -p kshana registry::` → PASS.
4. Commit: `feat(registry): register all built-in kinds via FnFactory adapter`

### Task 0.3: Route `run_toml_inner` through the registry (behavior-preserving)
**Files:** `kshana/src/api.rs` (modify)
**Steps:**
1. Failing test: `assert_eq!(run_toml(JAMMING_TOML)?.summary, /* current summary */)` plus a golden test asserting the JSON `scenario_hash` is unchanged for 3 representative scenarios — expect PASS today, must stay PASS after refactor (regression guard).
2. Implement: replace the body of `run_toml_inner` with `let kind = ScenarioKind::classify(src)?; PackRegistry::with_builtins().build(&kind.to_id(), &ScenarioConfig::from_toml(src))?.run()`. Add `ScenarioKind::to_id(&self)->ScenarioId`. Keep `ScenarioKind` and `classify` intact (typed front-end).
3. Verify: `cargo test -p kshana api::` and the golden hash tests → PASS (byte-identical outputs).
4. Commit: `refactor(api): dispatch via PackRegistry, outputs byte-identical`

### Task 0.4: `ExternalPack` → `register_into` registration seam
**Files:** `kshana/src/api.rs` (modify), `kshana/docs/ARCHITECTURE.md` (modify)
**Steps:**
1. Failing test: a dummy in-test pack implementing `ExternalPack` registers into a registry and is then `build`-able by its id — expect FAIL (no `register_into`).
2. Implement: add `fn register_into(reg: &mut PackRegistry) where Self: Sized;` (defaulted to no-op for back-compat) and keep `kind_name`/`meta`. Document the additive-overlay pattern in ARCHITECTURE.md.
3. Verify: `cargo test -p kshana api::external_pack` → PASS.
4. Commit: `feat(api): ExternalPack::register_into additive registration seam`

### Task 0.5: WASM + `no_std` compile guard for the registry
**Files:** `kshana/src/registry.rs` (modify), `kshana/.github/workflows/ci.yml` (modify)
**Steps:**
1. Failing check: `cargo build -p kshana --target wasm32-unknown-unknown` — confirm registry compiles (BTreeMap from `alloc`, no `std::HashMap`). If a `std`-only type leaked in, fix it.
2. Add a CI job `wasm-registry` running that target build.
3. Verify: target build PASS locally.
4. Commit: `ci(registry): wasm32 build guard for the registry seam`

## Substrate A — Assurance / Provenance / Uncertainty

### Task 0.6: `kshana::assurance::provenance` — PROV record type
**Files:** `kshana/src/assurance/mod.rs` (create), `kshana/src/assurance/provenance.rs` (create), `kshana/src/lib.rs` (modify)
**Steps:**
1. Failing test: build a `ProvRecord` from `{engine_commit, input_hashes, seed, lockfile_hash, tolerance}` and assert its `merkle_root()` is stable for fixed inputs and changes when any field changes.
2. Implement `ProvRecord` (W3C-PROV Entity/Activity/Agent fields) + `merkle_root()` over SHA-256 of canonicalized fields; `serde` round-trip. `no_std`+`alloc` clean.
3. Verify: `cargo test -p kshana assurance::provenance` → PASS.
4. Commit: `feat(assurance): W3C-PROV provenance record with Merkle root`

### Task 0.7: `kshana::assurance::uncertainty` — χ²/EDF confidence intervals
**Files:** `kshana/src/assurance/uncertainty.rs` (create)
**Steps:**
1. Failing test: Greenhall combined-EDF for white-FM, ADEV, N, m against a reference value table (from NIST SP 1065 / Greenhall 2003) to 1e-3 relative; CI = `s²·edf/χ²(p,edf)`.
2. Implement EDF for the AVAR/MVAR/TVAR/HVAR family + per-τ noise-id (B1/lag-1 ACF) + χ² inverse (use existing stats dep or a vetted rational approximation).
3. Verify: `cargo test -p kshana assurance::uncertainty` → PASS within tolerance.
4. Commit: `feat(assurance): Greenhall EDF + chi2 confidence intervals`

### Task 0.8: `kshana::assurance::oracle` — external-oracle registration harness
**Files:** `kshana/src/assurance/oracle.rs` (create), `kshana/src/verification.rs` (modify)
**Steps:**
1. Failing test: registering an `ExternalDataset` oracle + known-answer vector and asserting `fom_label::tier_for(fom)` returns `Validated`; with no oracle it returns `Modelled`.
2. Implement an `Oracle { name, kind: OracleKind, vectors: Vec<Kat> }` harness that feeds the verification matrix; assert the honesty constraint (Validated ⇒ ExternalDataset) holds.
3. Verify: `cargo test -p kshana assurance::oracle verification::` → PASS.
4. Commit: `feat(assurance): external-oracle harness wired to the honesty gate`

### Task 0.9: CI — superset-by-construction gates
**Files:** `kshana/.github/workflows/ci.yml` (modify), `kshana/tests/conformance.rs` (create)
**Steps:**
1. Add `assert_registry_conforms(&PackRegistry)` to public crate test-support: every `ids::ALL` id resolves + builds + passes a shared behavioral contract.
2. CI job `public-standalone`: build+test public crate alone (FOSS_ONLY analog). CI job `conformance`: run `assert_registry_conforms(&PackRegistry::with_builtins())`.
3. Verify: `cargo test -p kshana --test conformance` → PASS.
4. Commit: `ci: public-standalone + registry conformance gates`

### Task 0.10: Pro side — `pro_registry()` additive superset + additivity test
**Files:** `kshana-pro/src/registry.rs` (modify), `kshana-pro/tests/superset.rs` (create)
**Steps:**
1. Failing test: `assert!(pro_registry().ids().is_superset_of(kshana::PackRegistry::with_builtins().ids()))` and the delta == the expected pro-only ids — expect FAIL (no `pro_registry`).
2. Implement `pub fn pro_registry() -> kshana::PackRegistry { let mut r = kshana::PackRegistry::with_builtins(); ProPack::register_into(&mut r); r }`; make existing pro packs register additively.
3. Verify: `cargo test -p kshana-pro superset` → PASS; run `assert_registry_conforms(&pro_registry())` in pro CI.
4. Commit: `feat(pro): pro_registry() additive superset + CI additivity proof`

**Phase 0 exit criteria (battle-test):** public crate builds standalone + on wasm32; all built-in scenarios produce byte-identical outputs through the registry; conformance + additivity gates green; honesty gate proven on a sample oracle flip.

---

# PHASE 1 — L1 Clock Twin (science public, packs private)

Each task group = deliverable + oracle + test; expanded to micro-tasks JIT.

- **T1a `kshana::clock::powerlaw`** (public): fit all 5 power-law noises + drift to ADEV points. Oracle: allantools-generated curves + NBS-140. Test: recover known h_α to 1e-3.
- **T1b `kshana::clock::sde`** (public): Zucca-Tavella 3-state Φ + closed-form Q; clock-class switch (Cs 2-state / Rb·maser·optical 3-state). Oracle: σ_y²(τ)=q₁/τ+q₂τ/3+q₃τ³/20 algebraic identity. Test: simulated series ADEV matches the closed form.
- **T1c `kshana::clock::stats`** (public): MDEV, TDEV, overlapping HDEV, ThéoH/TOTVAR. Oracle: **NBS-140 9-point KAT** (ADEV(1)=91.22945) + NBS14 1000-pt. Test: KAT to published precision → flips these to **Validated**.
- **T1d** (public): wire χ²/EDF CIs (Task 0.7) onto every emitted stability point.
- **T2 `kshana/tests/clock_kat.rs`** (public): NBS-140 + NBS14 known-answer vectors + allantools cross-val to 1e-3. This is the oracle that earns the Validated labels.
- **T3a `kshana-pro` device presets** (private): CSAC SA.45s, OCXO, Rb PRS10, Galileo RAFS/PHM, Sr/Yb optical — datasheet ADEV/aging points. Test: twin reproduces each datasheet budget within stated tolerance.
- **T3b optical systematics** (private packs; formulas may be public): lattice AC-Stark (scalar/vector/tensor/hyperpolarizability), DC-Stark, density, micromotion+quadrupole, Dick-effect+QPN. Output: per-effect shift±uncertainty table summed in quadrature.
- **T4 holdover** (private pack): `x(t)=x₀+y₀t+½Dt²+ε(t)` with UQ band; report vs G.8272.1 / 1.5µs-24h / GNSS interval.
- **T5 V&V dossier + customer instance** (private, gated): NASA-STD-7009B CAS (8 factors) + ASME V&V 40 Context-of-Use; customer twin behind the existing NDA gate.

**Phase 1 exit (battle-test):** end-to-end `clock` scenario → registry → twin pack → evidence pack → report with NBS-validated stability stats + UQ bands; honesty gate green; presets reproduce datasheets.

---

# PHASE 2 — L3 Trade & Assurance (math public, packs private)

- **P0a `kshana::mcda::wsm`** (public): weighted decision matrix + value-function normalization + knock-out criteria; M=1 reproduces today's single-metric ranking (back-compat golden test).
- **P0b `kshana::mcda::ahp`** (public): pairwise → principal-eigenvector weights + **Consistency Ratio**, with a CR<0.10 gate. Oracle: Saaty RI table; test: known CR examples.
- **P1 `kshana::mcda::pareto`** (public): non-dominated set + knee-point (max distance-to-chord / Branke). Test: dominance KATs.
- **P2 `kshana::mcda::sensitivity`** (public): tornado over weights + Monte-Carlo/SMAA P(winner) + min-weight-change-to-flip (uses Task 0.7 uncertainty). Test: synthetic study with a known flip point.
- **P3 `kshana::mcda::utility`** (public): value/utility functions w/ risk attitude (MAUT).
- **P4 evidence pack** (public emitter / private content): extend `pack_c` hash table to full PROV record (Task 0.6) + Merkle chain.
- **P5 GSN/VCD** (public structure / private instance): Claim-Argument-Evidence + ECSS Verification-Control-Document traceability matrix.
- **P6 output shaping** (private packs): ECSS Annex L Trade-off Report + NASA Table 6.8-1 decision report; TRL per variant (ISO 16290) + TRL-6 knock-out.
- **P7 assurance gate** (private): regression gate → multi-criteria (winner-stability, Pareto-membership, CR<0.10, provenance-completeness, P(winner) threshold, hash-chain integrity).

**Phase 2 exit (battle-test):** end-to-end trade study of ≥3 variants → WSM+Pareto+sensitivity → PROV evidence pack → ECSS-Annex-L-shaped report; assurance gate green; back-compat golden test passes.

---

# PHASE 3 — Agency wrapper & surfacing

- **3a V&V dossier generator** (consumes Substrate A) — shared by L1 T5 and L3 P5.
- **3b** Update `kshana-pro/docs/OVERLAY-ARCHITECTURE.md` + the public web diagram to reflect new depth and any **real** Modelled→Validated flips (only after flips land).
- **3c** Feed `project_kshana_validation_campaign`: register each new public primitive's oracle as a campaign flip.

**Phase 3 exit:** diagram + docs match the now-true capability; campaign ledger updated; no claim outruns its oracle.

---

# Execution strategy
- Phase 0 is the critical path and is fully specified above. On completion, expand Phase 1 (then 2, then 3) task groups into micro-tasks against the real Phase 0 APIs.
- The build is highly parallelizable per-module; subagent-driven execution (fresh subagent per task, two-stage review) is the recommended mode. A multi-agent workflow is an option for fan-out once Phase 0 lands.
