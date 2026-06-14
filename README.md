# Kshana Pro

**Proprietary overlay on the open [Kshana](https://github.com/AshfordeOU/kshana) PNT-resilience engine.**
Private — All rights reserved (see [`LICENSE`](LICENSE)). **Not open source. Do not distribute.**

---

## What this is

Kshana Pro is a **strict superset** of open Kshana. It does **not** fork the open
engine — it **depends on it** as a Cargo dependency and extends it through the open
core's public, semver-stable trait seams. Every open feature is present *by
construction*; Pro adds the value that must stay closed.

```
        ┌─────────────────────────────────────────────┐
        │              kshana-pro (private)            │
        │   L1 twin · L2 resilience · L3 suite (gated) │
        │   + Pro packs  ── via ExternalPack/ErrorModel│
        └───────────────────┬─────────────────────────┘
                            │ depends on (never forks)
        ┌───────────────────▼─────────────────────────┐
        │            kshana (open, Apache-2.0)         │
        │   the whole validated engine + every surface │
        └─────────────────────────────────────────────┘
```

See [`docs/OVERLAY-ARCHITECTURE.md`](docs/OVERLAY-ARCHITECTURE.md) for the seam map
and [`legal/`](legal) for IP/NDA/export-control/BIPR/trademark material.

## Status — what is built vs gated

This repo currently holds the **P0 foundation only** (the architecture seam),
per the open-core build discipline. The three value layers are deliberately
**gated** and not implemented on spec.

| Layer | Module | Attaches via | Trigger | State |
|-------|--------|--------------|---------|-------|
| — (P0) | `packs::seam_check`, `registry` | `ExternalPack` | on-spec | ✅ built + tested |
| L1 | `twin`, `packs::twin_pack` | `kshana::models::ClockModel`/`ErrorModel` + `allan` | NDA partner + contract | ✅ **framework demonstrator** (customer-device instance gated) |
| L2 | `resilience` | `kshana::models::ErrorModel` / `ExternalPack` | export clearance + customer | 🔒 gated stub (legal wall) |
| L3 | `suite`, `packs::trade_study_pack` | `ExternalPack` + `kshana::api::run_scenario` | second paying customer | ✅ **framework demonstrator** (licensed SKU gated) |

**L1 (`pro-twin`)** — calibrates a clock digital twin to a device's *published*
Allan-deviation budget and validates (with the open-core Allan estimator) that the
twin reproduces it. The framework is the moat; a real customer-device twin just
swaps published points for NDA characterisation data — `twin::customer_instance_gated`
guards that path.

**L3 (`pro-trade-study`)** — runs architecture variants through the embedded open
engine, ranks them on a figure of merit, and emits an audit-grade evidence pack with
reproducible scenario hashes. Built on open capabilities; packaging it as a licensed
Kshana Pro SKU (full ECSS/MBSE) is demand-gated — `suite::product_gated` guards that.

**L2 (`resilience`)** stays a fail-closed seam guard: real threat data is export-
controlled (a legal wall), so nothing is built until the export assessment clears.

No gated *capability* is built on spec: only the L1/L3 *frameworks* (runnable on
public data) and the L2 guard.

## Build & run

The dev build uses a **path dependency** to the local open-core checkout
(`../kshana`). For a release build, pin the published crate or a git tag instead
(see the comment in [`Cargo.toml`](Cargo.toml)).

```bash
cargo test                       # unit + integration tests (proves the overlay)
cargo run -- kinds               # list open kinds (inherited) + Pro kinds (added)
cargo run -- run scenario.toml   # auto-routes: pro-* → overlay, else → open engine
cargo run -- version
```

The superset in one command — the same binary runs an **open** scenario
(delegated unchanged) and a **Pro** scenario:

```bash
cargo run -- run ../kshana/scenarios/ephemeris.toml   # [routed to: open engine (ephemeris)]
cargo run -- run scenarios/twin-csac.toml             # L1 twin calibration + validation (PASS)
cargo run -- run scenarios/trade-clock.toml           # L3 quantum-vs-classical trade + evidence pack
```

The L1/L3 demonstrators (runnable examples in [`scenarios/`](scenarios)):
- `scenarios/twin-csac.toml` — calibrates a clock twin to a representative CSAC ADEV
  budget; validates it reproduces the curve within tolerance.
- `scenarios/trade-clock.toml` — trades an optical-lattice vs a CSAC architecture by
  timing holdover; emits the ranked evidence pack (`evidence_pack_markdown` in the JSON).

## Discipline (from the open-core strategy)

- **Never fork** the open core — depend on it, extend via seams.
- **Never starve** the open core to upsell Pro (it is the funnel).
- **Never open** the moat: export-controlled data, customer calibrations, Pro IP.
- **Don't build gated *capability* on spec** — the L1/L3 *frameworks* are built and
  run on public data (a BD/bid calling card), but the gated *instances* (a customer
  device twin, a sold SKU) and **all of L2** (legal wall) wait on their triggers.
- **No GPL/copyleft deps** (breaks the licence model + ESA terms).
- **Never commit** NDA / customer / export-controlled material (see `.gitignore`).

## Trademark

"Kshana" is a trademark of Ashforde OÜ. A fork of the open engine may use the
Apache-2.0 code; it may not use the name or the canonical distribution.
