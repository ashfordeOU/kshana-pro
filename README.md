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
| L1 | `twin` | `kshana::models::ErrorModel` | NDA partner + contract | 🔒 gated stub |
| L2 | `resilience` | `kshana::models::ErrorModel` / `ExternalPack` | export clearance + customer | 🔒 gated stub |
| L3 | `suite` | `ExternalPack` + open `sweep` | second paying customer | 🔒 gated stub |

The gated modules are **seam guards**: they document where the layer attaches and
fail closed with an honest error if invoked. No proprietary capability is built
until its trigger exists.

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
echo 'kind = "pro-seam-check"' | tee /tmp/s.toml && cargo run -- run /tmp/s.toml  # [routed to: Pro overlay]
```

## Discipline (from the open-core strategy)

- **Never fork** the open core — depend on it, extend via seams.
- **Never starve** the open core to upsell Pro (it is the funnel).
- **Never open** the moat: export-controlled data, customer calibrations, Pro IP.
- **Don't build L1/L2/L3 on spec** — only the P0 seam is on-spec; the rest is
  partner/contract/customer-triggered.
- **No GPL/copyleft deps** (breaks the licence model + ESA terms).
- **Never commit** NDA / customer / export-controlled material (see `.gitignore`).

## Trademark

"Kshana" is a trademark of Ashforde OÜ. A fork of the open engine may use the
Apache-2.0 code; it may not use the name or the canonical distribution.
