# Pack C — Scenario-Subscription CI Harness · Implementation Plan

> **For agentic workers:** executed via subagent-driven development, task-by-task, with
> spec + code-quality review between tasks. Synchronous gates only.

**Owner:** Ashforde OÜ · **Status:** working plan · **Date:** 2026-06-24

**Goal:** Ship Pack C — a versioned, subscribable library of resilience scenarios that a
receiver/chipset vendor runs their config against in CI, getting a reproducible scored
Study Report each run plus a regression/drift gate (pass/fail) vs a stored baseline.

**Architecture:** A library + CLI harness in the PRIVATE `kshana-pro` overlay (like the
ProStudy orchestrator and the open `run_suite` — NOT an `ExternalPack` scenario kind,
because it needs filesystem + baseline context). It runs each pack scenario through the
existing `crate::registry::run` (which delegates open scenarios to the open engine
unchanged — the overlay never forks the core), extracts comparable figures of merit from
the result JSON, compares them to a stored baseline under caller-supplied thresholds, and
renders a branded Pack C Study Report. MODELLED, "not a certification".

**Tech stack:** Rust; `serde`/`serde_json`/`toml`/`sha2` (already deps); reuses
`kshana::api`, `crate::registry`, `crate::report_brand`, `crate::report` (StudyCover /
StudyProvenance), `crate::license` (entitlement). Determinism: the library reads NO clock;
all timestamps caller-supplied (only `main.rs` may default `generated_utc` from the clock).

**Honesty guardrails (the moat):** every report carries the always-on MODELLED + "not a
certification" banner; the CI gate is "did your config regress against MODELLED scenarios",
decision-support for the customer's own V&V — never a pass/fail certification of fitness.
Only FoMs with a known better/worse direction are gated; unknown FoMs are reported as
informational, never silently gated. Pack id + version + content hash + engine version +
scenario hashes are surfaced for full reproducibility.

**File map (all under `kshana-pro/`):**
- `src/pack_c/mod.rs` (NEW) — module root, re-exports.
- `src/pack_c/manifest.rs` (NEW) — versioned `ScenarioPack` manifest + content hash.
- `src/pack_c/metrics.rs` (NEW) — FoM extraction from result JSON → `MetricSet`.
- `src/pack_c/gate.rs` (NEW) — direction table, thresholds, regression `compare`, verdict + exit code.
- `src/pack_c/run.rs` (NEW) — `run_pack` / `run_ci` orchestration + `CiReport`.
- `src/report_pack_c.rs` (NEW) — branded Pack C report (HTML) + ProStudy section fragment.
- `src/lib.rs` (EDIT) — `pub mod pack_c; pub mod report_pack_c;`.
- `src/license/mod.rs` (EDIT) — add Pack C feature id → Tier::Pro in `feature_min_tier`.
- `src/main.rs` (EDIT) — `ci` subcommand.
- `scenarios/packs/resilience-baseline/` (NEW) — sample versioned pack: manifest + scenario TOMLs.
- `examples/gen_sample_ci_study.rs` (NEW) — reproducible sample generator.
- `docs/packs/sample-scenario-ci-report.html` (NEW) — committed sample deliverable.
- `docs/packs/PACK-C.md` (NEW) — buyer one-pager.
- `tests/pack_c.rs` (NEW) — integration tests + fixtures under `tests/fixtures/pack-c/`.

---

## Task C1 — Versioned scenario-pack manifest

**Files:** Create `src/pack_c/mod.rs`, `src/pack_c/manifest.rs`; edit `src/lib.rs`.

`ScenarioPack { id: String, version: String, description: Option<String>, entries: Vec<PackEntry> }`,
`PackEntry { path: String, label: Option<String> }`. All `#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]`.
- `pub fn parse_pack(src: &str) -> Result<ScenarioPack, String>` — TOML; reject empty `id`,
  empty `version`, or zero entries with a clear message.
- `pub fn pack_hash(&self) -> String` — sha256 hex over a canonical string:
  `id \n version \n` then for each entry in order `path \t label.unwrap_or("") \n`. Deterministic; no clock.
- `PackEntry` label fallback: `pub fn label(&self) -> String` = label or the path stem.

TDD: parse valid pack; reject empty id / no entries; `pack_hash` stable across calls;
hash changes when version or an entry changes; `label()` falls back to path stem.

Header on every new file: `// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.`

## Task C2 — FoM extraction → MetricSet

**Files:** Create `src/pack_c/metrics.rs`; re-export from `mod.rs`.

`MetricSet(pub BTreeMap<String, f64>)` (`Clone,Debug,Default,PartialEq,Serialize,Deserialize`).
- `pub fn extract(label: &str, result_json: &str) -> Result<MetricSet, String>` — parse JSON;
  recursively collect every **numeric leaf** as `"<label>::<dotted.path>"` → value
  (e.g. `"araim::fom.raim_availability"`). Skip the provenance fields `schema_version`,
  `seed` only at the TOP level (keep nested). Booleans → not numeric (skip). Arrays:
  index into path (`fom.list.0`). Determinism: BTreeMap keeps keys sorted.
- `MetricSet::get(&self, key) -> Option<f64>`, `merge(&mut self, other)`, `keys()`.

TDD: extract flat + nested numerics from a sample JSON (use the araim shape with a `fom`
object); label prefix applied; non-numeric skipped; top-level `seed` excluded; two extracts
of the same JSON equal.

## Task C3 — Regression gate

**Files:** Create `src/pack_c/gate.rs`; re-export from `mod.rs`.

- `pub enum Direction { HigherIsBetter, LowerIsBetter }`.
- `pub fn direction_for(metric_key: &str) -> Option<Direction>` — match on the **leaf name**
  (last dotted segment, ignoring the `label::` prefix). Known table (document each):
  HigherIsBetter: `raim_availability`, `availability`, `detect_auc`, `integrity`, `security`,
  `holdover_s`. LowerIsBetter: leaf ending in `_m` (e.g. `mean_hpl_m`, `mean_vpl_m`),
  leaf ending in `_ns`, `fault_rate`, leaf containing `error`/`rmse`/`_pl`/`protection`.
  Anything else → `None` (informational, NOT gated).
- `pub struct Thresholds { pub default_rel: f64, pub per_key: BTreeMap<String, f64> }`
  with `Thresholds::new(default_rel)` and `with(key,rel)`; `rel_for(key)` → per_key or default.
  Relative degradation tolerance, e.g. 0.05 = allow 5% worse.
- `pub struct Regression { pub key: String, pub baseline: f64, pub current: f64, pub rel_change: f64, pub threshold: f64, pub direction: Direction }`.
- `pub struct GateVerdict { pub passed: bool, pub regressions: Vec<Regression>, pub improvements: Vec<String>, pub informational: Vec<String> }` (serde).
- `pub fn compare(baseline: &MetricSet, current: &MetricSet, thr: &Thresholds) -> GateVerdict`
  — for each key present in BOTH: if `direction_for` is None → informational. Else compute
  signed relative degradation (worse direction); if degradation > threshold → Regression;
  if better → improvement. `passed = regressions.is_empty()`.
- `impl GateVerdict { pub fn exit_code(&self) -> i32 { if self.passed {0} else {1} } }`.

TDD: regression beyond threshold flagged (higher-is-better drop); lower-is-better rise
flagged; within-threshold passes; unknown leaf → informational not gated; improvement
recorded; `exit_code` 0/1; missing-from-baseline key ignored (no panic).

## Task C4 — CI run orchestration

**Files:** Create `src/pack_c/run.rs`; re-export from `mod.rs`.

- `pub struct CiContext { pub generated_utc: Option<String>, pub customer: Option<String>, pub thresholds: Thresholds }` (caller-supplied; NO clock here).
- `pub struct CiReport { pub pack_id, pack_version, pack_hash: String, pub engine_version: Option<String>, pub scenario_hashes: Vec<String>, pub generated_utc: Option<String>, pub customer: Option<String>, pub metrics: MetricSet, pub verdict: Option<GateVerdict> }` (serde — this IS the baseline storage format).
- `pub fn run_pack(pack: &ScenarioPack, base_dir: &Path) -> Result<(MetricSet, Vec<String>, Option<String>), String>`
  — for each entry: read `base_dir.join(entry.path)`; `crate::registry::run(&src)` → `RunOutput`;
  `metrics::extract(&entry.label(), &out.json)` merged into the set; collect each result's
  top-level `scenario_hash` and `engine_version` (from the JSON). Returns (metrics, scenario_hashes sorted+deduped, engine_version).
- `pub fn run_ci(pack: &ScenarioPack, base_dir: &Path, baseline: Option<&CiReport>, ctx: &CiContext) -> Result<CiReport, String>`
  — run_pack; build CiReport (pack id/version/hash, engine_version, scenario_hashes,
  ctx.generated_utc/customer, metrics); if baseline present, `verdict = Some(compare(&baseline.metrics, &metrics, &ctx.thresholds))` else None.
- `CiReport::to_json(&self)` / `from_json(&str)`.

TDD (integration, in `tests/pack_c.rs`): build fixtures under `tests/fixtures/pack-c/` —
a manifest + 1–2 **fast, deterministic** open-kind scenario TOMLs (pick the fastest open
kind that emits clean numeric FoMs; the GNSS-sim/ARAIM kind emits
`raim_availability`/`mean_hpl_m` etc.). Assert: run_ci with no baseline → metrics non-empty,
verdict None, engine_version Some; run_ci twice → byte-identical `to_json` (no clock);
run_ci with a hand-built worse baseline → `verdict.passed == false` with ≥1 regression;
with an equal baseline → passed.

## Task C5 — Branded Pack C report

**Files:** Create `src/report_pack_c.rs`; edit `src/lib.rs` (`pub mod report_pack_c;`).

- `pub struct PackCReport { pub title: String, pub report: CiReport, pub cover: Option<StudyCover>, pub provenance: Option<StudyProvenance> }`.
- Builders `from_ci(title, &CiReport)`, `with_cover`, `with_provenance`.
- `pub fn section_fragment_html(&self) -> String` — the verdict banner (PASS / REGRESSIONS:n),
  pack id + version + short hash, engine version, a metrics+delta table (baseline | current |
  Δ% | gated?), and an explicit "MODELLED — not a certification" note in the fragment.
  Reuse `report_brand` colors/fonts. Per-FoM rows may show `kshana::fom_label::tier_for`.
- `pub fn to_html(&self) -> String` — full branded doc: `report_brand::cover_html` +
  `report_brand::modelled_banner_html(...)` + the section fragment + provenance block + seal.

TDD: html contains the verdict word, pack version, short pack hash, "not a certification",
the MODELLED banner; a regression verdict renders the regression rows; deterministic
(two renders equal); `section_fragment_html` usable as a ProStudy section.

## Task C6 — Entitlement, CLI, sample, one-pager

**Files:** edit `src/license/mod.rs`, `src/main.rs`; create
`scenarios/packs/resilience-baseline/` (manifest + scenarios), `examples/gen_sample_ci_study.rs`,
`docs/packs/sample-scenario-ci-report.html`, `docs/packs/PACK-C.md`.

- License: add the Pack C feature id `"pro-scenario-ci"` to `feature_min_tier` → `Tier::Pro`.
  Add `pub fn ensure_ci_entitled(ent: &Entitlement, now_unix: i64) -> Result<(), KshanaError>`
  (Unsupported error if not allowed). TDD: Evaluation denied, Pro allowed.
- CLI `ci` subcommand in `main.rs`: `--pack <manifest.toml>` (required), `--baseline <ci.json>`,
  `--thresholds k=v,...` (and bare default like `0.05`), `--out <report.html>`, `--json <ci.json>`,
  `--utc <iso8601>`, `--customer <name>`. base_dir = manifest's parent dir. `generated_utc`
  defaults to `utc_iso8601_now()` ONLY here (the clock boundary). Writes report + ci json;
  process exit code = `verdict.exit_code()` (0 if no baseline). Keep it consistent with the
  existing `--study`/`--validate` flag style; reuse the crate's existing now-helper if present.
- Sample: `examples/gen_sample_ci_study.rs` builds the resilience-baseline pack, runs run_ci
  once with no baseline (PASS-style) and once against a deliberately-better synthetic baseline
  to show a REGRESSION, fixed `generated_utc = "2026-06-24T00:00:00Z"`, customer "SAMPLE",
  writes `docs/packs/sample-scenario-ci-report.html`. Must be reproducible (byte-identical
  across runs). Label SAMPLE in the cover.
- `docs/packs/PACK-C.md`: buyer one-pager mirroring PACK-A.md style — ICP (receiver/chipset
  vendors, integrators), JTBD (catch resilience regressions in CI before they ship), how the
  subscription works (versioned packs, you run them in CI), intake (your config/scenario +
  optional baseline), deliverable (branded Study Report + machine JSON + CI exit code), honest
  scope (MODELLED, not a certification; informs your own V&V), contact via the order desk.

TDD: entitlement test (above); a CLI/sample smoke (run the example or a test that drives
run_ci over the committed sample pack and asserts a report is produced + reproducible).

---

## Gates (run SYNCHRONOUSLY after every task; never background cargo)
```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test -p kshana-pro
```
Plus the determinism/golden assertions in the new tests. Stage ONLY Pack C files by explicit
path (NEVER `git add -A`). Do not touch anything under `papers/`. Commit locally; do not push.
No AI/automation attribution anywhere in files or commit messages; author
`Chakshu Baweja <contact@ashforde.org>`. Never write the absolute machine path.
