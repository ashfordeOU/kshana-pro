# Kshana Pro — Overlay Architecture

How the proprietary overlay attaches to the open Kshana engine **without forking
it**. This is the contract that makes Pro a strict superset of open Kshana and
keeps the two in lock-step forever.

> Internal/proprietary document. Not for the public repo.

---

## 1. The rule: depend, never fork

Kshana Pro is a separate **private** crate that lists the open `kshana` crate as a
dependency and extends it through the open core's **public, semver-stable trait
seams**. It never copies, vendors, or forks the open engine.

```
kshana-pro (private, proprietary)
  └── depends on ─▶ kshana (public, AGPL-3.0-only / commercial)  ← the whole validated engine
```

Consequence — **superset by construction**:
- Every open feature is present in a Pro build automatically (it *is* the open
  engine, embedded).
- A new open release is a one-line dependency bump; there is no merge, no drift,
  no conflict.
- Closed code physically cannot leak into the open tree (different repos).

### Dependency pinning
- **Dev:** path dependency `kshana = { path = "../kshana", version = "0.17" }` so
  seams can co-evolve with the overlay during development.
- **Release:** pin the published crate (`kshana = "0.17"`) or a git tag
  (`kshana = { git = "…/kshana", tag = "v0.17.0" }`) so a Pro build is
  reproducible and tied to an exact, released open engine.

---

## 2. The seams (already in the open core)

PR2 ("extension seams in open core") is **already satisfied** — these public
traits exist in `kshana` today and are documented as semver-stable in the open
core's own `docs/ARCHITECTURE.md` (§4 "Extending Kshana with an external pack",
§9 "A private overlay repo … plugs in via the same `ErrorModel` interface and the
`ExternalPack` contract without changing the public engine").

| Seam (open `kshana`) | Location | What Pro attaches with it |
|----------------------|----------|---------------------------|
| `api::Scenario` + `api::ExternalPack` | `src/api.rs` | New out-of-tree scenario packs (Pro packs, L3 meta-packs) |
| `models::ErrorModel` | `src/models.rs` | Device-calibrated clock/inertial twins (L1), real-parameter degradation models (L2) |
| `ephem_provider::EphemerisProvider` | `src/ephem_provider.rs` | Premium ephemeris (e.g. DE440 built-in, PR19) |
| `precise_od::ForceModel` | `src/precise_od.rs` | Higher-fidelity force models (PR19) |
| `propagator::Propagator` | `src/propagator.rs` | Alternative propagators |
| `lunar_od::LunarEnvironment` | `src/lunar_od.rs` | Lunar/cislunar environment extensions |

**No open-core change is required to stand up the overlay.** If a future Pro
feature needs a seam that does not yet exist, that becomes a small, additive,
semver-safe PR to the open core — demand-pulled by a concrete Pro need, never
speculative.

---

## 3. How a Pro pack plugs in (the proven pattern)

The P0 proof pack `packs::seam_check` is the worked example. Any Pro pack follows
the same three steps:

```rust
use kshana::api::{Scenario, ExternalPack, RunOutput, KshanaError, ScenarioMeta};

#[derive(serde::Deserialize)]
struct MyProPack { /* fields parsed from the scenario TOML */ }

impl Scenario for MyProPack {
    fn run(&self) -> Result<RunOutput, KshanaError> { /* compose open-engine calls */ }
}
impl ExternalPack for MyProPack {
    fn kind_name(&self) -> &'static str { "pro-my-pack" }
    fn meta(&self) -> ScenarioMeta { /* name, description, fields */ }
}
```

Register it in `registry::build_pro_pack` + `registry::pro_kinds`. The
`registry::run` **superset dispatcher** then routes `pro-*` kinds to the Pro pack
and delegates everything else to `kshana::api::run_scenario` unchanged — one
binary, open ∪ Pro.

**Convention:** every Pro-only kind carries the `pro-` prefix
(`registry::PRO_KIND_PREFIX`). It makes routing unambiguous and Pro kinds obvious
in scenario files and logs.

---

## 4. Where each value layer attaches

The distinction that matters: the gated **capability** (a real customer instance / a
sold SKU / controlled data) vs the **framework** that capability plugs into. The
frameworks for L1 and L3 are **built as demonstrators** (runnable on public data, no
gate crossed); only the gated instances and all of L2 wait on triggers.

- **L1 — Quantum-payload digital twin** (`twin`, `packs::twin_pack`) — ✅ **framework
  built.** `twin::calibrate` fits a published Allan-deviation budget to an open-core
  `ClockModel` (an `ErrorModel`); `twin::validate` confirms the twin reproduces the
  curve via `kshana::allan`. Demonstrated on a representative CSAC spec (`pro-twin`).
  *Gated instance:* a twin of a specific customer's device needs NDA (and likely
  export-controlled) data — `twin::customer_instance_gated`. First instance =
  AO/1-13503 Foreground IP.
- **L2 — Export-sensitive resilience** (`resilience`) — 🔒 **gated stub (legal wall).**
  Real-parameter degradation + access-controlled packs, the closed twins of the open
  `jamming`/`spoof` packs. Trigger: export-control assessment (PR5) + cleared
  customer. Nothing built on spec — `resilience::gated`.
- **L3 — Trade-study & assurance suite** (`suite`, `packs::trade_study_pack`) — ✅
  **framework built.** `suite::run_trade_study` runs architecture variants through
  `kshana::api::run_scenario`, ranks them, and `evidence_pack_markdown` emits an
  audit-grade record with reproducible scenario hashes (`pro-trade-study`). *Gated:*
  packaging as a licensed SKU with full ECSS/MBSE generation — `suite::product_gated`.
  Trigger: second paying customer.

---

## 5. Discipline (enforced by structure)

- `publish = false` in `Cargo.toml` — `cargo publish` refuses, so the closed
  overlay cannot reach crates.io by mistake.
- Gated modules **fail closed** (`KshanaError::Unsupported` with an honest reason)
  if invoked before their trigger — no fabricated capability.
- `.gitignore` blocks NDA/customer/export-controlled inputs as a backstop.
- No GPL/copyleft dependencies (preserves the licence model and ESA IPR terms).
- The open core stays the canonical reference; Pro never strips an open feature to
  upsell.
