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

**Tender / bid material** lives under [`docs/tenders/`](docs/tenders/) — the FutureNAV
pursuit dossier (per-tender briefings, the ECOS/FFP cost volume, the dominance deck,
and the per-tender / cross-tender deliverables). Start at
[`docs/tenders/INDEX.html`](docs/tenders/INDEX.html) (or [`docs/tenders/README.md`](docs/tenders/README.md)).
PRIVATE — never publish any of it to the open kshana repo.

## Status — what is built vs gated

The overlay is well past its P0 seam: **six Pro packs** are built and tested on
public data across the L1 and L3 value layers, plus the deliverable and deployment
modules listed below. What stays **gated** is never a *framework* — it is the
*instance* that needs private data, export clearance, or a signed customer: a real
customer-device twin, the export-controlled L2 threat models, and the productised
ECSS/MBSE SKU. The honest line is **framework (built) vs gated instance (not built
on spec)**.

### Built Pro packs

Each runs on public data; the tier shown is the licence required to *run* it (see
[Licensing](#build--run)).

| Pack (`kind`) | Layer | What it does | Min tier | State |
|---------------|-------|--------------|----------|-------|
| `pro-seam-check` | P0 | Smallest real `ExternalPack` — exercises the whole overlay contract end-to-end | Evaluation (free) | ✅ built + tested |
| `pro-twin` | L1 | Calibrates a clock twin to a device's *published* Allan budget; validates reproduction with the open-core Allan estimator | Pro | ✅ framework demonstrator (customer-device instance gated) |
| `pro-quantum-systematics` | L1 | Optical-clock systematic frequency-shift budget (BBR, gravitational redshift, Zeeman, extras) + uncertainty floor — formulas built, species coefficients are *cited inputs*, never baked in | Pro | ✅ framework demonstrator |
| `pro-trade-study` | L3 | Runs architecture variants through the embedded open engine, ranks them on a figure of merit, emits an audit-grade evidence pack with reproducible scenario hashes | Pro | ✅ framework demonstrator (licensed SKU gated) |
| `pro-mbse` | L3 | ECSS/MBSE artifact generator: requirements → margins table + traceability CSV + ReqIF-lite XML (DOORS/Polarion-importable) | Enterprise | ✅ framework demonstrator (product SKU gated) |
| `pro-batch` | scale | Parallel Monte-Carlo runner: fans one base scenario across many seeds concurrently, aggregates a figure of merit (mean/std/min/max/p05/p95) | Enterprise | ✅ built + tested |

### Layer attach points & gates

| Layer | Modules | Attaches via | Gate / trigger | State |
|-------|---------|--------------|----------------|-------|
| P0 | `packs::seam_check`, `registry` | `ExternalPack` | on-spec | ✅ built + tested |
| L1 | `twin`, `premium`, `packs::{twin,quantum_systematics}_pack` | `kshana::models::ClockModel`/`ErrorModel` + `allan` | `twin::customer_instance_gated` (NDA partner + contract) | ✅ frameworks built; customer instance gated |
| L2 | `resilience` | `kshana::models::ErrorModel` / `ExternalPack` | export clearance + customer | 🔒 fail-closed stub (legal wall) |
| L3 | `suite`, `mbse`, `packs::{trade_study,mbse}_pack` | `ExternalPack` + `kshana::api::run_scenario` | `suite::product_gated` (productised SKU) | ✅ frameworks built; SKU gated |

### Supporting modules (built)

Beyond the packs, the overlay ships the deliverable and deployment surface a
study/bid needs:

- **`twin::navdt`** (A10) — NAV-DT performance/resilience twin: runs the open-core
  spine end-to-end and assembles one hash-stamped, label-honest bundle.
- **`report`** (A11) — one renderer (Markdown / HTML / SVG / JSON) over the NAV-DT
  bundle + trade-study evidence + RF-impairment evaluation, MODELLED/VALIDATED labels
  carried through.
- **`mrd`** (A03 / D1.4) — machine-checked Mission-Requirements skeleton mapping each
  PNT requirement to the tender's figure-of-merit vocabulary; guards position-domain
  FoMs as `PartnerFollowOn`.
- **`benefits`** (A02 / D1.1) — benefit-vs-baseline ratio engine over the trade-study
  core, with dual MODELLED/VALIDATED provenance labels and no auto-upgrade to VALIDATED.
- **`connectors`** (PR21) — STK `.e` (km, J2000/GCRS) + universal CSV exporters, the
  SpaceStream NAV-DT metrics codec, and a CI metric gate (dot-path metric, non-zero exit).
- **`serve`** (PR22) — a dependency-light, std-only blocking HTTP/1.1 service
  (`/health`, `/version`, `/kinds`, `POST /run`) for on-prem / air-gapped / sovereign /
  hosted deployment, entitlement-gated.
- **`license`** (PR7) — the tier mechanics: HMAC-SHA256 tokens, fail-closed Evaluation
  default, per-`kind` tier mapping.

### What is NOT built on spec (gated)

- A real **customer-device clock twin** — the framework swaps *published* Allan points
  for NDA characterisation data; `twin::customer_instance_gated` guards it.
- All of **L2 resilience** — real threat data is export-controlled; the module stays a
  fail-closed seam guard until the export assessment clears.
- The productised **ECSS/MBSE SKU** — the `pro-mbse` framework runs today; packaging it
  as a sold, supported deliverable is demand-gated via `suite::product_gated`.

No gated *capability* is built on spec — only the *frameworks* (runnable on public
data) and the L2 guard.

## Build & run

The dev build uses a **path dependency** to the local open-core checkout
(`../kshana`). For a release build, pin the published crate or a git tag instead
(see the comment in [`Cargo.toml`](Cargo.toml)).

```bash
cargo test                       # unit + integration tests (proves the overlay)
cargo run -- kinds               # list open kinds (inherited) + Pro kinds + their tier
cargo run -- version
```

The suite is unit tests across every module plus **seven integration tests** in
[`tests/`](tests) that prove the contract: `overlay.rs` (superset by construction),
`e2e_interop.rs` (end-to-end), `reproducibility_twin.rs` (deterministic twin),
`no_overclaims_mission_analysis.rs` (honesty guard), and the `*_in_sync.rs` trio
(each committed deliverable matches its generator output, so docs never drift).

**Licensing (PR7).** `run` is **fail-closed**: with no `--license` it enforces the
**Evaluation** tier (open scenarios + the free `pro-seam-check` only). A value pack
needs a license token (or `--dev` for local runs). Open scenarios are always free.

```bash
# Open scenario — always free, no license needed:
cargo run -- run ../kshana/scenarios/ephemeris.toml         # [routed to: open engine]

# Local exploration of Pro packs without a license:
cargo run -- run scenarios/twin-csac.toml --dev            # L1 twin (PASS)
cargo run -- run scenarios/trade-clock.toml --dev          # L3 trade + evidence pack

# The real licensed flow — issue, then run under that tier:
TOK=$(cargo run -q -- license issue --tier pro --licensee "ACME Space" --days 90)
cargo run -- license verify "$TOK"                          # shows entitled Pro kinds
cargo run -- run scenarios/twin-csac.toml --license "$TOK"  # allowed (Pro tier)
cargo run -- run scenarios/batch-clock-mc.toml --license "$TOK"  # DENIED (Enterprise-only)
```

Tiers: **Evaluation** (free `pro-seam-check`) → **Pro** (`pro-twin`,
`pro-trade-study`, `pro-quantum-systematics`) → **Enterprise** (+ `pro-mbse`,
`pro-batch`). Tokens are HMAC-SHA256 signed; the vendor secret is
`$KSHANA_PRO_LICENSE_SECRET` (a demo secret is used otherwise — never for production).

**Connectors & CI (PR21).** Export an ephemeris to enterprise tools, or gate a
metric in CI. The open core already emits CCSDS OEM/SP3 (GMAT/Orekit); Pro adds STK
`.e` (km, J2000/GCRS) + universal CSV, and a metric gate with a CI exit code.

```bash
cargo run -- export ../kshana/scenarios/ephemeris.toml --format stk > orbit.e
cargo run -- export ../kshana/scenarios/ephemeris.toml --format csv > orbit.csv
# CI gate: nonzero exit fails the pipeline (dot-path supports array indices):
cargo run -- gate ../kshana/scenarios/ephemeris.toml --metric samples.0.alt_km --op gt --threshold 400
```

**Deliverable & deployment subcommands.** Beyond `run` / `export` / `gate`, the
overlay exposes the study-deliverable and service surface (full list: `kshana-pro`
with no args, or see [`src/main.rs`](src/main.rs) `usage()`):

```bash
cargo run -- navdt scenarios/twin-csac.toml                 # A10 NAV-DT twin bundle (JSON)
cargo run -- report scenarios/trade-clock.toml --format html # A11 consolidated report (md|json|html|svg)
cargo run -- mrd                                            # A03/D1.4 machine-checked MRD FoM-mapping (Markdown)
cargo run -- benefits --json                                # A02/D1.1 benefit-vs-baseline table (dual provenance labels)
cargo run -- spacestream --schema                           # A09 SpaceStream codec goldens / ICD schema
cargo run -- serve --addr 127.0.0.1:8080                    # PR22 HTTP service (/health /version /kinds /run), entitlement-gated
```

The Pro packs (runnable examples in [`scenarios/`](scenarios)):
- `scenarios/twin-csac.toml` — `pro-twin`: calibrates a clock twin to a representative
  CSAC ADEV budget; validates it reproduces the curve within tolerance.
- `scenarios/trade-clock.toml` — `pro-trade-study`: trades an optical-lattice vs a CSAC
  architecture by timing holdover; emits the ranked evidence pack (`evidence_pack_markdown`).
- `scenarios/quantum-systematics-sr.toml` — `pro-quantum-systematics`: an Sr optical-clock
  systematic-shift budget (coefficients supplied in the scenario, never baked in).
- `scenarios/mbse-srd.toml` — `pro-mbse`: a requirements set rendered to a margins table +
  traceability CSV + ReqIF-lite XML.
- `scenarios/batch-clock-mc.toml` — `pro-batch`: a Monte-Carlo ensemble of a clock scenario
  across many seeds (Enterprise tier).
- `scenarios/spacestream-*.json` — SpaceStream NAV-DT metrics fixtures (nominal / jamming /
  spoof-time / degraded) for the connector / `spacestream` path.

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
