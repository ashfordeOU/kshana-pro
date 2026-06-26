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
- **Dev:** path dependency `kshana = { path = "../kshana", version = "0.20" }` so
  seams can co-evolve with the overlay during development.
- **Release:** pin the published crate (`kshana = "0.20"`) or a git tag
  (`kshana = { git = "…/kshana", tag = "v0.20.0" }`) so a Pro build is
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

impl MyProPack {
    // Convention: a `KIND` const + a `meta_static()` associated fn so the registry
    // can name and describe the pack without constructing one.
    pub const KIND: &'static str = "pro-my-pack";
    pub fn meta_static() -> ScenarioMeta { /* name: Self::KIND, description, fields */ }
}

impl Scenario for MyProPack {
    fn run(&self) -> Result<RunOutput, KshanaError> { /* compose open-engine calls */ }
}
impl ExternalPack for MyProPack {
    fn kind_name(&self) -> &'static str { Self::KIND }
    fn meta(&self) -> ScenarioMeta { Self::meta_static() }
}
```

Register it in `registry::build_pro_pack` (match `MyProPack::KIND`) and
`registry::pro_kinds` (push `MyProPack::meta_static()`). The
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
- **P0 — Seam proof** (`packs::seam_check`, `pro-seam-check`) — ✅ the smallest real
  `ExternalPack`; free at Evaluation, it exercises the whole overlay contract end-to-end.
- **L1 — Optical-clock systematics** (`premium`, `packs::quantum_systematics_pack`,
  `pro-quantum-systematics`) — ✅ framework built: a BBR / gravitational-redshift /
  Zeeman systematic-shift budget + uncertainty floor; species coefficients are *cited
  inputs*, never baked in.
- **Pack A — Optimism-gap audit** (`packs::{optimism_gap_audit, detector_intake}` +
  `report_pack_a`, `pro-optimism-gap-audit`) — ✅ the in- vs out-of-distribution detector
  audit packaged as a sold study (Pro tier).
- **Pack C — Scenario-subscription CI** (`pack_c::{manifest, metrics, gate, run}` +
  `report_pack_c`, run via `kshana-pro ci`, `pro-scenario-ci`) — ✅ a manifest of
  scenarios + metric thresholds gated against a rolling baseline (the JSON output is the
  next run's baseline). Entitlement: `license::ensure_ci_entitled` (Pro tier).
- **Scale — Batch Monte-Carlo** (`batch`, `packs::batch_pack`, `pro-batch`) — ✅ fans one
  base scenario across many seeds and aggregates a figure of merit (Enterprise tier).
- **ECSS/MBSE** (`mbse`, `packs::mbse_pack`, `pro-mbse`) — ✅ framework built: requirements
  → margins + traceability CSV + ReqIF-lite XML; the productised SKU is demand-gated
  (`suite::product_gated`).

Above the packs sits the **deliverable / service layer** — `pro_study` (composes one
branded study from N pack sections), `report` / `report_pdf` / `report_pack_a` /
`report_pack_c` (the rendered faces), `study_ledger` (a hash-chained delivery record),
`serve` (the HTTP service), `connectors` (STK / CSV / CI gate), and `license` +
`registry` (the superset dispatcher + tier gate). See §6 for the module map and the
study / deployment pipelines.

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

---

## 6. The overlay at a glance (module map + pipelines)

**Overlay module map.** Every Pro caller enters through `registry` (the superset
dispatcher) and the `license` gate; `pro-*` kinds route to a Pro pack, everything else
to the embedded open engine.

```mermaid
flowchart TB
  CLI["kshana-pro CLI / serve"] --> REG["registry — superset dispatcher<br/>peek_kind → pro-* vs open"]
  REG --> LIC["license — tiers + HMAC token<br/>run_gated · ensure_ci_entitled (fail-closed)"]
  LIC --> PACKS
  subgraph PACKS["Pro packs (ExternalPack · const KIND + meta_static)"]
    P1["seam_check — P0 proof"]
    P2["twin_pack + twin / premium — L1 clock twin · systematics"]
    P3["trade_study_pack + suite — L3 trade + evidence"]
    P4["mbse_pack + mbse — L3 ECSS/MBSE"]
    P5["optimism_gap_audit + detector_intake — Pack A"]
    P6["pack_c (manifest·metrics·gate·run) — Pack C scenario-CI"]
    P7["batch_pack + batch — scale Monte-Carlo"]
    PG["resilience 🔒 — L2 export wall (fail-closed)"]
  end
  PACKS --> OPEN["kshana — open engine (embedded)<br/>api::run_scenario · ErrorModel · ExternalPack seams"]
  PACKS --> DELIV
  subgraph DELIV["Deliverable / service"]
    R["report · report_pdf · report_brand · report_pack_a · report_pack_c"]
    PS["pro_study (compose) · study_ledger (hash-chain)"]
    SV["serve — GET /health /version /kinds · POST /run /study"]
    CN["connectors — STK .e · CSV · CI gate · spacestream"]
  end
```

**Study → report → ledger pipeline.** A study request is gated, run through the embedded
open engine, composed into one branded artifact, and recorded tamper-evidently.

```mermaid
flowchart LR
  A["intake<br/>serve POST /study<br/>deterministic study_id"] --> B{"license gate<br/>(Pro tier+)"}
  B -->|denied| X["403 fail-closed<br/>(no licensee leak)"]
  B -->|allowed| C["run packs via registry<br/>→ kshana::api::run_scenario<br/>(+ reproducible scenario_hash)"]
  C --> D["pack renderers<br/>report_pack_a / report_pack_c / report"]
  D --> E["pro_study.compose<br/>1 cover + 1 MODELLED banner + N sections<br/>(report_brand furniture)"]
  E --> F["render face<br/>HTML / SVG / JSON · report_pdf → pure-Rust PDF"]
  F --> G["study_ledger.append<br/>hash-chained record · verify() detects tamper"]
```

**Deployment.** The Dockerfile builds both crates from the parent context and ships a
fail-closed service.

```mermaid
flowchart TB
  subgraph BUILD["docker build -f kshana-pro/Dockerfile .. (context = PARENT)"]
    K1["COPY kshana (open core)"] --> K3["cargo build --release --locked<br/>(../kshana path dep resolves)"]
    K2["COPY kshana-pro (overlay)"] --> K3
  end
  K3 --> IMG["runtime: debian-slim · non-root · EXPOSE 8088<br/>HEALTHCHECK → kshana-pro healthcheck /health"]
  IMG --> SVC["ENTRYPOINT kshana-pro · CMD serve --addr 0.0.0.0:8088<br/>fail-closed: open + free seam-check until --license"]
  SVC --> D1["deploy/docker-compose.product.yml — hosted"]
  SVC --> D2["deploy/docker-compose.sovereign.yml — air-gapped"]
```
