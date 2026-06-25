<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; NEVER publish in the open kshana repo.
Master implementation plan: build a dominant, requirement-compliant Quantum-Enabled PNT
demonstrator capability across ALL THREE application areas, derived from Clarifications 1-3
to AO/1-13503/26/NL/RR. Capability CODE -> PUBLIC kshana; bid evidence + this plan -> PRIVATE.
Contact: contact@ashforde.org
-->

# Quantum-Enabled PNT Demonstrator — domination build plan (AO/1-13503)

> **For agentic workers:** execute task-by-task, TDD, frequent local commits, explicit paths only.
> **Capability code → PUBLIC `kshana` (AGPL-3.0-only).** **Bid evidence + this plan → PRIVATE `kshana-pro`.**
> **PUSH ONLY WHEN FOUNDER SAYS.** Honesty is the moat: every new capability lands a `verification.rs`
> row (Validated only with an external oracle; else Modelled), keeps `tests/no_overclaims.rs` green, and
> carries a representativeness + gaps-to-flight record (the Clar 1/A3 compliance mechanism). No
> TRL/flight/certification/ESA-endorsement claims. No AI-assistant attribution or tool-vendor path
> strings in repo content/history. Commits authored `Chakshu Baweja <contact@ashforde.org>`, no footer.

**Goal:** make Kshana the open, reproducible, requirement-compliant **simulation-based demonstration
environment** for a quantum-enabled PNT demonstrator across all three SoW §1.5.2 application areas —
trusted quantum timing, GNSS-free quantum navigation, and fault/anomaly detection for quantum systems —
so an Ashforde consortium WP can demonstrate feasibility, performance and operational benefits (with
representativeness justified and gaps-to-flight identified) at a depth no competitor's private notebook
can match.

**Why this wins (anchored to the clarifications):**
- **Clar 1/A3** — ESA states a *simulation environment* is a compliant way to demonstrate feasibility,
  performance and operational benefits, *"provided representativeness is justified and remaining gaps
  towards flight are clearly identified."* → We make representativeness + gaps-to-flight a **first-class,
  machine-checked output** of every demonstration. Our honesty discipline becomes the compliance proof.
- **Clar 1/A1** — a single focused area is compliant and depth is rewarded. → We build each vertical to
  *dominant depth* so the bid can lead with any one (or a coherent multi-pillar concept) without sacrificing
  feasibility.
- **Clar 1/A2** — open competition for critical pre-developments completes pre-submission. → We enter as a
  named consortium WP (software/SE/mission-analysis); not gated.
- **Clar 3** — FPD-10031 PA&S is Reference, no SoC; internal PA process expected. → Kshana's CI/SBOM/SLSA/
  `verification.rs` IS that internal PA process; we extend it, we don't bolt on bureaucracy.
- **Clar 2** — deadline 17 Aug 2026. → ~7.5-week runway makes a deep multi-vertical build feasible.

**Architecture:** a cross-cutting **foundation** (compliant-demonstration spine + unified trade harness +
quantum device error-model library) that all three **verticals** (A timing, B navigation, C anomaly) plug
into, closed by an **integration + battle-test + bid-evidence** phase. Dependency order: Foundation → A,
B, C (parallel-capable) → Integration.

**Tech stack:** Rust, no new heavy deps (reuse `rand`/`rand_chacha`, native `[f64;3]`/matrices, existing
`batch_ls`/`kalman`/`estimator`); Python+matplotlib for paper/figures only; `tectonic` for any preprint.

---

## What already exists (REUSE — not greenfield)

| Area | Existing kshana modules to reuse |
|---|---|
| Timing | `allan` (ADEV/MDEV/TDEV/HDEV, **validated vs Stable32/NIST**), `clock_state`, `holdover`, `ensemble`, `timescales`, `timetransfer`, `timetransfer_adv`, `tpl` (certified bound + CUSUM, arXiv:2606.24210), `filter_health`, `kalman`, `timegeo` |
| Quantum/nav | `quantum_trade` (CAI budget, **CAI validated ~2 orders vs Freier 2016**), `hybrid`, `estimator`, `kalman`, `particle_filter`, `orbit_determination`, `precise_od`, `deepspace_od`, `batch_ls`, `mapmatch`, `gravimeter`, `igrf`, `gravity_sh`, `lunar_combination` (observability/NEES pattern) |
| Anomaly/resilience | `resilience/*` (arch/score/timeline/diversity/study/panel/stats), `detection`, `spoof_detect`, `spoof_monitors`, `security`, `filter_health`, `impairment_eval`, `impairment_ml`, `impairment_study`, `eval_stats` (**validated vs scikit-learn**) |
| Trade & evidence | `crossover` (quantum-vs-classical crossover map), `quantum_trade`, `fom`/`fom_label`, `study`/`suite`/`sweep`, `report`/`permalink`, `verification` (honesty matrix; **8 validated islands**) |

**Net-new is therefore: a representativeness/gaps ledger, an entanglement/optical time-transfer chain, a
secure-time integrity layer, a GNSS-free quantum-OD simulator, a quantum-fault catalog + detectors, and
the integrated demonstrator study — plus new external-validated islands.** Each defined below.

---

## Phase F — Foundation: the compliant-demonstration spine (build FIRST)

### F1 — Representativeness & gaps-to-flight ledger  ⭐ (the A3 compliance mechanism)
- **Net-new:** `src/representativeness.rs` — a typed record attached to every demonstration output:
  `Representativeness { anchors: Vec<Anchor>, modelled_assumptions: Vec<String>, gaps_to_flight: Vec<Gap>, trl_band: (u8,u8) }` where each `Anchor` cites an external oracle / published source and each `Gap`
  names a remaining step to flight. A `representativeness_report()` renders it; an invariant ties it to the
  `verification.rs` label (a Validated claim must list its external anchor; a Modelled claim must list its
  gaps).
- **Reuse:** `verification`, `fom_label`.
- **Tests (TDD):** schema round-trip; invariant test (Validated ⇒ ≥1 external anchor; Modelled ⇒ ≥1 gap);
  a "no empty gaps-to-flight on a flight-relevant claim" lint.
- **Validation oracle:** internal consistency + a documented mapping of the struct fields to the exact
  Clar 1/A3 wording ("representativeness justified", "gaps to flight identified").
- **e2e battle test:** every new scenario below emits a `representativeness` block; a workspace test asserts
  presence + non-emptiness across all quantum scenarios.
- **Verification row:** Modelled / InternalConsistency. **Bid evidence:** this is the artifact the proposal
  points to for A3 compliance.

### F2 — Unified quantum-vs-classical trade harness
- **Net-new:** `src/qtrade.rs` (or extend `quantum_trade`) — one trade API: `TradeFrame` (scenario+seed+
  engine) → candidates routed through one neutral path → common FoM vector → sweep/Monte-Carlo → a
  `TradeEvidence` object (per-FoM value + CI + validated/modelled label + representativeness). All three
  verticals emit `TradeEvidence`.
- **Reuse:** `crossover`, `quantum_trade`, `study`, `sweep`, `fom`.
- **Tests:** reproduce the existing crossover headline numbers through the new API (regression); CI/bootstrap
  correctness vs hand value; determinism (same seed ⇒ identical evidence).
- **Validation oracle:** existing crossover/quantum-trade validated kernels (vs scipy) — validated-by-reuse.
- **e2e battle test:** one `TradeFrame` run per vertical produces a complete `TradeEvidence`.
- **Verification row:** Modelled (kernels validated-by-reuse). **Bid evidence:** the "fix-frame, common-FoM,
  sweep, trace" method text in `technical-contribution.md` §1.

### F3 — Quantum device error-model library
- **Net-new:** `src/quantum_devices.rs` — published-coefficient error models with explicit anchors:
  optical-lattice clock (ADEV), CSAC, **cold-atom interferometer** (reuse the validated `quantum_trade`
  Mach–Zehnder/shot-noise model), optical TWSTFT link, and a **quantum/entanglement-based time-transfer
  link** (single-photon timing jitter, pair-rate-limited precision) — each modelled from cited literature,
  each labelled, each with a representativeness record.
- **Reuse:** `quantum_trade` (CAI), `allan`, `timetransfer_adv`.
- **Tests:** each device's stability/precision reproduces its published coefficient within a stated band;
  CAI re-anchored to Freier 2016 (regression of the existing validation).
- **Validation oracle:** published device coefficients (literature); CAI vs Freier 2016 (existing external
  anchor). Target: ≥1 new ExternalDataset island (optical-clock ADEV vs a published curve).
- **e2e battle test:** instantiate every device, assert finite physical outputs + populated anchors.
- **Verification row:** Modelled (+ one Validated where a public dataset exists). **Bid evidence:** device
  coverage table for the technical volume.

---

## Phase A — Trusted Quantum Timing (time transfer + secure dissemination + timing anomaly)

### A1 — Optical/quantum clock ensemble + disciplined holdover
- **Net-new:** extend `clock_state`/`holdover`/`ensemble` with optical-clock ADEV models + an
  inverse-variance disciplined ensemble + holdover under discipline loss.
- **Reuse:** `allan` (validated), `clock_state`, `ensemble`, `holdover`, `kalman`.
- **Tests:** ensemble = inverse-variance hand value; holdover floor honesty (very stable clock ⇒ floor-
  governed, stated); ADEV of the model matches the device coefficient.
- **Validation oracle:** ADEV vs Stable32/NIST (existing validated) + published optical-clock stability.
- **e2e battle test:** multi-clock ensemble over a horizon, reproducible.
- **Verification row:** Modelled (ADEV validated-by-reuse).

### A2 — End-to-end time-transfer chain
- **Net-new:** `src/timetransfer_chain.rs` — clock → link (optical TWSTFT, T2L2-class, quantum/entanglement)
  → dissemination → user, with a full error budget (reciprocal-delay cancellation, asymmetry, tropo,
  relativistic, link jitter). Scenario `kind="quantum-time-transfer"`.
- **Reuse:** `timetransfer`, `timetransfer_adv`, `quantum_devices` (F3), `timescales`.
- **Tests:** reciprocal cancellation exact at symmetric link (closed-form); residual scales with asymmetry;
  quantum link precision tracks pair-rate; round-trip < tolerance.
- **Validation oracle:** closed-form reciprocal-cancellation identity + published two-way time-transfer
  budget figures. Target: an ExternalDataset island vs a published TWSTFT/T2L2 budget if data permits.
- **e2e battle test:** full chain scenario run + JSON/SVG + representativeness block.
- **Verification row:** Modelled (+ Validated if external budget anchored).

### A3 — Secure time dissemination + integrity
- **Net-new:** `src/secure_time.rs` — time-signal spoof/tamper/delay-attack detection + a timing protection
  level (reuse `tpl` certified bound + CUSUM) + an authentication-delay model; security FoM `1−P_md`.
- **Reuse:** `tpl` (validated TPL bound), `spoof_detect`, `spoof_monitors`, `detection`, `security`.
- **Tests:** detector ROC vs injected delay/replay attacks; TPL bound monotonicity; Pfa control.
- **Validation oracle:** TPL bound (existing) + detection ROC vs a labelled attack set.
- **e2e battle test:** scenario exercises nominal + attacked time signal; reports detection + protected.
- **Verification row:** Modelled (TPL validated-by-reuse).

### A4 — Timing anomaly/fault detection
- **Net-new:** extend `tpl` CUSUM + `filter_health` for online clock-health + time-transfer-fault detectors
  (frequency jump, drift, lock-loss, link dropout) with Pfa/Pmd + detection latency.
- **Reuse:** `tpl`, `filter_health`, `detection`, `resilience::stats`.
- **Tests:** ROC vs the F-stage fault catalog (see C1); detection-latency vs change magnitude.
- **Validation oracle:** ROC vs injected faults (internal) + CUSUM ARL vs theory.
- **e2e battle test:** fault-injected timing stream → detector report.
- **Verification row:** Modelled / InternalConsistency.

### A5 — Timing vertical trade + battle test
- **Net-new:** quantum-vs-classical timing trade via F2 `TradeEvidence`; consolidate A1–A4 into the
  `quantum-time-transfer` scenario's full output.
- **Tests:** trade reproduces expected crossover behaviour; representativeness populated.
- **e2e battle test:** full scenario, reproducibility, adversarial review, no_overclaims, README/VALIDATION
  + `scenario_count_doc_sync` bump.
- **Bid evidence:** `kshana-pro/.../13503-quantum-pnt/D1.x-trusted-timing.md` benefits deliverable.

---

## Phase B — GNSS-free quantum spacecraft navigation

### B1 — Quantum-IMU model (CAI accel + cold-atom gyro)
- **Net-new:** extend `quantum_trade`/`attitude_budget` to a full quantum-IMU error model (bias/scale/
  noise/Coriolis/AC-Stark for accel; cold-atom gyro ARW/bias).
- **Reuse:** `quantum_trade` (CAI validated vs Freier), `attitude_budget`.
- **Tests:** accel noise floor vs shot-noise; gyro ARW vs coefficient; CAI re-anchored to Freier.
- **Validation oracle:** CAI vs Freier 2016 (existing external) + published cold-atom gyro figures.
- **e2e battle test:** IMU error propagation over an outage.
- **Verification row:** Modelled (CAI validated-by-reuse).

### B2 — GNSS-free onboard OD simulator
- **Net-new:** `src/quantum_nav_od.rs` — quantum-IMU + clock (+ optional alt-PNT: gravity/terrain/mag)
  sequential + batch OD under GNSS denial; outage-bounding error growth. Scenario
  `kind="quantum-gnss-free-nav"`.
- **Reuse:** `estimator`, `kalman`, `batch_ls`, `orbit_determination`, `precise_od`, `gravimeter`/`igrf`/
  `mapmatch`, the `lunar_combination` observability pattern.
- **Tests:** closed-loop truth recovery; covariance realism (NEES ≈ params); outage error bounded vs free-INS
  divergence; determinism.
- **Validation oracle:** closed-loop recovery + NEES (internal) + classical-INS divergence vs analytic.
- **e2e battle test:** full GNSS-denied scenario; outage-bounding headline.
- **Verification row:** Modelled / InternalConsistency.

### B3 — Observability & covariance analysis
- **Net-new:** reuse the `lunar_combination` eigenstructure/condition-number approach for the quantum-nav
  state: which states are observable with which sensor set; σ_sensor sweeps (information-not-count control,
  as in the lunar VLBI paper).
- **Reuse:** `lunar_combination` (pattern), `batch_ls`.
- **Tests:** rank/observability thresholds; σ-sweep monotonic trend (with honest noise caveat).
- **Validation oracle:** internal consistency (formal covariance).
- **e2e battle test:** observability sweep artifact.
- **Verification row:** Modelled / InternalConsistency.

### B4 — Nav vertical trade + battle test
- **Net-new:** quantum-vs-classical nav trade via F2; consolidate B1–B3.
- **Tests + e2e:** as A5 (reproducibility, adversarial review, no_overclaims, doc sync).
- **Bid evidence:** `D1.x-gnss-free-quantum-nav.md` benefits deliverable (complements existing
  `D1.1-gnss-denied-resilience-benefits.md`).

---

## Phase C — Fault/anomaly detection for quantum PNT systems

### C1 — Quantum-system fault model + labelled fault catalog
- **Net-new:** `src/quantum_faults.rs` — a labelled fault library: clock faults (freq jump, drift, lock-loss,
  flicker excursion), quantum-sensor faults (bias step, scale error, dropout, fringe-lock loss), link faults;
  each with a generator + ground-truth label for eval.
- **Reuse:** `clock_state`, `quantum_devices` (F3), `rand_chacha`.
- **Tests:** each fault produces its defined signature; labels are deterministic per seed.
- **Validation oracle:** definitional + injected (internal).
- **e2e battle test:** generate a labelled multi-fault stream.
- **Verification row:** Modelled / InternalConsistency.

### C2 — Online health/anomaly detectors
- **Net-new:** extend `detection`/`filter_health`/`resilience` with change detection (CUSUM/GLRT),
  innovation/χ² monitoring, multi-sensor consistency voting; per-detector ROC/Pfa-Pmd + latency.
- **Reuse:** `detection`, `filter_health`, `resilience::stats`, `tpl` (CUSUM), `raim` (consistency).
- **Tests:** ROC vs C1 catalog; Pfa control; multi-sensor voting beats single (diversity).
- **Validation oracle:** ROC vs labelled catalog (internal) + GLRT/CUSUM vs theory.
- **e2e battle test:** detector bank over the C1 stream → ROC report.
- **Verification row:** Modelled / InternalConsistency.

### C3 — Quantum-PNT anomaly eval harness
- **Net-new:** apply the optimism-gap / honest-eval methodology to quantum-fault detection: train/eval split,
  optimism gap, calibration; reuse `impairment_eval`/`eval_stats`/`resilience::study`. Scenario
  `kind="quantum-anomaly-detect"`.
- **Reuse:** `impairment_eval`, `impairment_ml`, `eval_stats` (validated vs scikit-learn), `resilience::study`.
- **Tests:** eval metrics vs scikit-learn (validated-by-reuse); optimism-gap reproduces on synthetic.
- **Validation oracle:** `eval_stats` vs scikit-learn (existing external).
- **e2e battle test:** full eval scenario + honest report.
- **Verification row:** Modelled (eval-stats validated-by-reuse).

### C4 — Anomaly vertical trade + battle test
- **Net-new:** quantum-vs-classical anomaly-detection comparison via F2; consolidate C1–C3.
- **Tests + e2e:** as A5/B4.
- **Bid evidence:** `D1.x-quantum-anomaly-detection.md` benefits deliverable.

---

## Phase D — Integration, battle-testing & bid evidence

### D1 — Unified "Quantum-Enabled PNT Demonstrator" study
- **Net-new:** a `suite` manifest + study that runs all three verticals into one reproducible, branded
  evidence pack (the integrated demonstrator artifact the prime can show ESA). Scenario/suite
  `quantum-pnt-demonstrator`.
- **Reuse:** `suite`, `study`, `report`, `permalink`, `representativeness` (F1).
- **Tests:** suite runs all three; aggregate evidence object well-formed; representativeness present.
- **e2e battle test:** one-command full-demonstrator run, reproducible from seed+version.
- **Verification row:** Modelled / InternalConsistency.

### D2 — Validated-island expansion (the credibility lever)
- **Net-new:** wherever a public dataset exists, add an ExternalDataset oracle + reference test to push
  capabilities from Modelled → Validated (optical-clock ADEV vs published; time-transfer vs a published
  budget; gyro vs published). Grow the validated-island count beyond the current 8.
- **Tests:** each new island is a reference test vs the external dataset (gitignore raw data; commit only
  derived scalars).
- **Validation oracle:** the external datasets themselves.
- **Verification rows:** new Validated rows.

### D3 — Full-suite battle test (the gate)
- **Net-new:** the consolidated gate: `cargo fmt --check`, `cargo clippy -D warnings`, full `cargo test`,
  `tests/no_overclaims.rs`, `tests/scenario_count_doc_sync.rs`, e2e runs of every new scenario, a
  reproducibility check (same seed+version ⇒ identical artifacts), and an **adversarial review** pass over
  the whole program (numbers vs artifacts, overclaim scan, honesty). README + `docs/VALIDATION.md` updated;
  scenario count bumped.
- **e2e battle test:** this phase *is* the battle test.

### D4 — Bid evidence (kshana-pro PRIVATE)
- **Net-new/updated:** thread the A3 representativeness/gaps framing + per-vertical capability through
  `technical-contribution.md` (§1 method + per-vertical sections + PBF-7 WP descriptor),
  `TRL-maturation-dossier.md` (cite A3), `software-PA-evidence.md` (cite Clar 3 = internal PA process), the
  SoW compliance matrix; add the three `D1.x-*.md` benefits deliverables; refresh `kshana-capability-brief.md`
  and the `gap-audit.html` badges; update `TENDER-13503-PURSUIT.md`.
- **Tests/validation:** every quantitative claim in the bid evidence traces to a committed artifact (the
  reproducibility object), labelled validated/modelled.

---

## Honesty moat (applies to every task — non-negotiable)
- Every capability is MODELLED unless it carries an external-dataset oracle (then Validated). "Validated by
  reuse" only for capabilities built directly on a validated kernel.
- Every demonstration emits a **representativeness + gaps-to-flight** record (F1) — the A3 compliance proof.
- No TRL>3 / flight / space-qualification / certification / ESA-endorsement claims. Models the *class* of
  quantum-PNT system; example device/constellation parameters are illustrative, public-source, not affiliated.
- `tests/no_overclaims.rs` stays green; each phase adds its `verification.rs` row + `docs/VALIDATION.md`
  entry + README count bump. Raw external datasets gitignored (only derived scalars committed).
- Build pattern per task (mirror the lunar suite): extract exact existing APIs → bite-sized TDD plan →
  build → wire `scenario.rs` kind + `api.rs` dispatch + verification row → gate → banned scan → commit LOCAL
  (explicit paths, authored `Chakshu Baweja <contact@ashforde.org>`, no footer). **Push only when told.**

## Public / private split
- **PUBLIC `kshana`:** all capability code, scenarios, tests, verification rows, README/VALIDATION (F1–F3,
  A1–A5, B1–B4, C1–C4, D1–D3).
- **PRIVATE `kshana-pro`:** this plan, per-phase TDD plans, and ALL bid evidence (D4) — technical
  contribution, TRL dossier, software-PA, SoW compliance, D1.x benefits, capability brief, tracker.

## Self-review (writing-plans)
- Coverage: all three SoW §1.5.2 areas (timing A, nav B, anomaly C) + the A3 compliance mechanism (F1) +
  the trade method (F2) + device library (F3) + integration/battle-test/bid evidence (D). ✓
- Net-new vs reuse stated per task; no duplication of existing modules. ✓
- Test + validation oracle + e2e battle test specified per task. ✓
- Honesty structural (representativeness ledger, validated/modelled, no_overclaims). ✓
- Risk: scope is large (~20 tasks). Mitigation: Foundation first (unblocks all), verticals are
  parallel-capable, each task is independently shippable + gated; the integrated study (D1) and battle test
  (D3) close the loop. Validated-island expansion (D2) is the credibility lever and should be pursued
  opportunistically where public data exists.
