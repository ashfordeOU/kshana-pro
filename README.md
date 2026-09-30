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
        ┌──────────────────────────────────────────────────────────────┐
        │                     kshana-pro  (PRIVATE)                      │
        │                                                                │
        │  OVERLAY (code)            KNOWLEDGE LAYER (never leaves)       │
        │  ┌──────────────────┐      ┌──────────────────────────────┐    │
        │  │ L1 twin          │      │ docs/tenders/  13503·13494·   │    │
        │  │ L2 resilience 🔒 │      │   lunar  (bid evidence, decks)│    │
        │  │ L3 suite (gated) │      │ papers/        (arXiv-track)  │    │
        │  │ + 17 Pro packs   │      │ docs/gtm · docs/strategy      │    │
        │  │   via ExternalPack│     │ legal/  (licence·NDA·export·  │    │
        │  │   / ErrorModel   │      │   BIPR·trademark)             │    │
        │  └────────┬─────────┘      │ docs/kshana-architecture.html │    │
        │           │                └───────────────┬──────────────┘    │
        └───────────┼────────────────────────────────┼───────────────────┘
                    │ depends on (never forks)        │ cites (never copies code)
        ┌───────────▼─────────────────────────────────▼───────────────────┐
        │                  kshana  (OPEN, AGPL-3.0-only)                   │
        │   the whole validated engine + every surface                    │
        │   · 3 quantum-PNT verticals     · 7 lunar-PNT modules           │
        │   · representativeness + gaps-to-flight ledger                  │
        │   · validated-island matrix (verification.rs, honesty gate)     │
        └──────────────────────────────────────────────────────────────────┘

   Capability flows OUT → open repo.   Evidence · strategy · money stay IN.
   Every claim, both sides, is underwritten by the VALIDATED / MODELLED
   honesty labels + the representativeness ledger.
```

See [`docs/OVERLAY-ARCHITECTURE.md`](docs/OVERLAY-ARCHITECTURE.md) for the seam map
and [`legal/`](legal) for IP/NDA/export-control/BIPR/trademark material.

**Tender / bid material** lives under [`docs/tenders/`](docs/tenders/) — the FutureNAV
pursuit dossier covering the flagship **AO/1-13503 Quantum-Enabled PNT** (deadline
17 Aug 2026), **AO/1-13494 AI & Digital Payload PNT** (9 Jul 2026) and the anticipatory
**lunar-PNT** assets, plus per-tender briefings, the ECOS/FFP cost volume, the dominance
deck, and cross-tender deliverables. Start at [`docs/tenders/INDEX.html`](docs/tenders/INDEX.html)
(or [`docs/tenders/README.md`](docs/tenders/README.md)), and read
[`docs/kshana-architecture.html`](docs/kshana-architecture.html) first if you're new.
PRIVATE — never publish any of it to the open kshana repo.

## Repository contents — where to find what

Kshana Pro is two things in one private repo: **(a)** the proprietary Rust **overlay**
(the `src/` packs, documented below), and **(b)** the **private knowledge layer** — bid
evidence, research, commercial and strategy material — that turns the open engine into
won tenders and revenue. The map:

| Area | Path | What's there |
|------|------|--------------|
| **Field guide** | [`docs/kshana-architecture.html`](docs/kshana-architecture.html) | From-zero explainer: PNT, the engine, the honesty/validated-island model, the open ⟷ private split, the three quantum-PNT areas, and how we win ESA tenders. Start here if you're new. |
| **Tender pursuits** | [`docs/tenders/`](docs/tenders/) | All ESA bid material — open [`INDEX.html`](docs/tenders/INDEX.html) / [`README.md`](docs/tenders/README.md). |
| → Quantum-PNT (flagship) | [`docs/tenders/13503-quantum-pnt/`](docs/tenders/13503-quantum-pnt/) | AO/1-13503 (deadline 17 Aug 2026): pursuit tracker, demonstrator capability across all 3 SoW areas, clarifications audit, SE-verification & software-PA evidence, TRL dossier, the validation-dataset catalog, and the honest security-validation negative. |
| → AI / digital-payload PNT | [`docs/tenders/13494-ai-payload-pnt/`](docs/tenders/13494-ai-payload-pnt/) | AO/1-13494 (deadline 9 Jul 2026): NAV-DT + AI-eval-testbed work packages. |
| → Lunar PNT | [`docs/tenders/lunar-pnt/`](docs/tenders/lunar-pnt/) | Anticipatory shelf assets vs Moonlight/LCNS (capability statement + fit map). Outreach PARKED pending founder go. |
| → Cross-tender + cost | [`docs/tenders/cross-tender/`](docs/tenders/cross-tender/), [`cost-volume/`](docs/tenders/cost-volume/), [`briefings/`](docs/tenders/briefings/) | BIPR/IP posture, D1.1 benefits & D1.4 MRD, the ECOS/FFP cost volume, per-tender briefings, the dominance deck. |
| **Research papers** | [`papers/`](papers/) | arXiv-track papers (LaTeX + figures + reproducible data) + the open-engine technical report. **arXiv-first** — nothing public until the founder announces. |
| **Commercial / GTM** | [`docs/gtm/`](docs/gtm/) | ICP & target list, outreach sequence, sales one-pager, qualification/objections. |
| **Strategy & roadmaps** | [`docs/strategy/`](docs/strategy/), [`docs/roadmaps/`](docs/roadmaps/) | Pricing proposal, productization roadmap, phase plans, the quantum-PNT-13503 and lunar-PNT build roadmaps. |
| **Legal** | [`legal/`](legal/) | Commercial licence, NDA/SOW/DPA templates, IP policy, export-control determination, BIPR & trademark briefs, counsel-review package. |
| **The overlay (code)** | [`src/`](src/), [`scenarios/`](scenarios/), [`tests/`](tests/) | The Pro packs + deliverable/deployment modules ([Status](#status--what-is-built-vs-gated)); seam map in [`docs/OVERLAY-ARCHITECTURE.md`](docs/OVERLAY-ARCHITECTURE.md). |

**Visibility rule (non-negotiable):** everything in this repo is **PRIVATE**. Capability
*code* is mirrored to the open [`kshana`](https://github.com/AshfordeOU/kshana) repo;
**bid evidence, papers, strategy, commercial and legal material never leave this repo.**

## Status — what is built vs gated

The overlay is well past its P0 seam: **seventeen Pro packs** (plus the **Pack C**
scenario-CI subscription product) are built and tested on public data across the
P0 / L1 / L3 value layers, plus the deliverable and deployment modules listed below. What stays **gated** is never a *framework* — it is the
*instance* that needs private data, export clearance, or a signed customer: a real
customer-device twin, the export-controlled L2 threat models, and the productised
ECSS/MBSE SKU. The honest line is **framework (built) vs gated instance (not built
on spec)**.

### Built Pro packs

Each runs on public data; the tier shown is the licence required to *run* it (see
[Licensing](#build--run)). The table covers the framework packs; the full list of the
sixteen registered in [`src/registry.rs`](src/registry.rs), with their tiers, comes from
`cargo run -- kinds`. `tests/readme_counts.rs` fails the build if the pack count written
here stops matching the registry.

| Pack (`kind`) | Layer | What it does | Min tier | State |
|---------------|-------|--------------|----------|-------|
| `pro-seam-check` | P0 | Smallest real `ExternalPack` — exercises the whole overlay contract end-to-end | Evaluation (free) | ✅ built + tested |
| `pro-twin` | L1 | Calibrates a clock twin to a device's *published* Allan budget; validates reproduction with the open-core Allan estimator; adds holdover-UQ **MTIE(τ)-mask compliance** (reuses the validated `kshana::allan::mtie` estimator with a conservative ±kσ band) against caller-supplied ITU-T-style masks | Pro | ✅ framework demonstrator (customer-device instance gated) |
| `pro-quantum-systematics` | L1 | Optical-clock systematic frequency-shift budget (BBR, gravitational redshift, Zeeman, extras) + uncertainty floor — formulas built, species coefficients are *cited inputs*, never baked in | Pro | ✅ framework demonstrator |
| `pro-trade-study` | L3 | Runs architecture variants through the embedded open engine, ranks them on a figure of merit, and stress-tests the winner with an **8-method MCDA concordance panel** (WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/PROMETHEE II + COPRAS; Kendall-τ concordance + rank-reversal count, folded into the ECSS Annex L / NASA-SE-6.8 reports); emits an audit-grade evidence pack with reproducible scenario hashes | Pro | ✅ framework demonstrator (licensed SKU gated) |
| `pro-optimism-gap-audit` | Pack A | The in- vs out-of-distribution **optimism-gap audit** packaged as a sold study: ingests a customer detector through the open-core eval testbed, scores the ID→OOD degradation, and emits the branded Pack A report (`report_pack_a` + `packs::detector_intake`) | Pro | ✅ built + tested |
| `pro-mbse` | L3 | ECSS/MBSE artifact generator: requirements → margins table + traceability CSV + ReqIF-lite XML (DOORS/Polarion-importable) | Enterprise | ✅ framework demonstrator (product SKU gated) |
| `pro-batch` | scale | Parallel Monte-Carlo runner: fans one base scenario across many seeds concurrently, aggregates a figure of merit (mean/std/min/max/p05/p95) | Enterprise | ✅ built + tested |
| `pro-scenario-ci` | Pack C | The **scenario-subscription CI** product (run via `kshana-pro ci`): a manifest of scenarios + metric thresholds gated against a rolling baseline (the JSON output doubles as the next run's baseline), emitting the branded Pack C regression report (`pack_c::{manifest,metrics,gate,run}` + `report_pack_c`) | Pro | ✅ built + tested |

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

- **L3 assurance stack** (`suite::{decision,multi_method,gsn,vcd,provenance,gate,vv_dossier}`)
  — the trade study upgraded from a single figure of merit to a defensible, agency-shaped
  decision: an **8-method MCDA concordance panel** (Kendall-τ agreement + rank-reversal
  count), a **GSN** claim–argument–evidence assurance argument (GSN Community Standard
  v3), an **ECSS VCD** verification-control matrix (ECSS-E-ST-10-02C), a **W3C-PROV**
  tamper-evident provenance record, a **fail-closed** multi-criteria assurance gate, and
  the capstone agency **V&V credibility dossier**.
- **L1 twin depth** (`twin::{holdover_uq,presets,optical_systematics}`) — holdover-UQ
  **MTIE(τ)-mask compliance** over the validated `kshana::allan::mtie` estimator (with a
  conservative ±kσ band), a named clock **device-preset** library, and the optical-lattice
  **systematic-shift** budget.
- **`twin::navdt`** (A10) — NAV-DT performance/resilience twin: runs the open-core
  spine end-to-end and assembles one hash-stamped, label-honest bundle.
- **`report`** (A11) — one renderer (Markdown / HTML / SVG / JSON) over the NAV-DT
  bundle + trade-study evidence + RF-impairment evaluation, MODELLED/VALIDATED labels
  carried through.
- **`pro_study` · `study_ledger` · `report_pdf` / `report_brand` / `report_pack_a` /
  `report_pack_c`** — the study → delivery pipeline. `pro_study` composes one branded
  cover + a single MODELLED banner + N pack sections (with the `report_brand` furniture);
  `report_pdf` renders a pure-Rust PDF; `study_ledger` appends a hash-chained,
  tamper-evident delivery record (request → run → delivered; `verify()` catches any
  alteration); `report_pack_a` / `report_pack_c` render the Pack A / Pack C study faces.
- **`mrd`** (A03 / D1.4) — machine-checked Mission-Requirements skeleton mapping each
  PNT requirement to the tender's figure-of-merit vocabulary. Each row resolves to the
  verification evidence that *binds* it, not the strongest in its chain; a follow-on row
  must claim no field and no oracle, and no row may credit a pass-through field as a
  computation.
- **`benefits`** (A02 / D1.1) — benefit-vs-baseline ratio engine over the trade-study
  core, with dual MODELLED/VALIDATED provenance labels and no auto-upgrade to VALIDATED.
- **`connectors`** (PR21) — STK `.e` (km, J2000/GCRS) + universal CSV exporters, the
  SpaceStream NAV-DT metrics codec, and a CI metric gate (dot-path metric, non-zero exit).
- **`serve`** (PR22) — a dependency-light, std-only blocking HTTP/1.1 service
  (`GET /health` · `/version` · `/kinds`; `POST /run` · `POST /study`) for on-prem /
  air-gapped / sovereign / hosted deployment, entitlement-gated and **fail-closed**
  (open engine + free seam-check at Evaluation until a license token is supplied).
- **`mbse`** — model-based systems engineering (MBSE) and programme tooling over the
  open engine's own outputs (`kshana-pro mbse import|trace|vcrm|sysml|impact|pack`):
  requirements imported from comma-separated values (CSV), a documented subset of the
  Requirements Interchange Format (ReqIF) and Systems Modeling Language v2 (SysML v2)
  text, each with a machine-checkable acceptance criterion over `result.json` fields
  with units; evaluation against real runs, unit-checked against the field-units
  schema; a verification cross-reference matrix (VCRM) whose evidence is each run's
  `scenario_hash`, `seed`, `engine_version` and the field paths and values read, with
  a MODELLED or PARTNER basis flagged; a SysML v2 export checked against the grammar
  subset it uses; change impact between two traces or two scenario files; and an
  offline HTML plus JSON evidence pack. See [`docs/MBSE.md`](docs/MBSE.md).
- **`license`** (PR7) — the tier mechanics: Ed25519-signed tokens (public key only in the build), fail-closed Evaluation
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

### What the embedded open engine now carries (depended-on, not duplicated)

Because Pro is a superset *by construction*, every advance in the open core is present
here too. Recent open-core capability the bid evidence relies on:

- **The three quantum-PNT demonstrator verticals** (AO/1-13503) — trusted quantum
  **timing** (optical-clock + entanglement time-transfer), **GNSS-free quantum
  navigation** (cold-atom interferometer vs nav-grade INS), and **fault/anomaly
  detection** — each a runnable scenario, bundled into one reproducible study. Capability
  is open; the bid framing is private (`docs/tenders/13503-quantum-pnt/QUANTUM-DEMONSTRATOR-CAPABILITY.md`).
- **The representativeness + gaps-to-flight ledger** (`representativeness.rs`) — a
  machine-checked record on every demonstration (external anchors · modelled assumptions ·
  gaps-to-flight · representative TRL band). This *is* the Clar 1/A3 compliance mechanism.
- **The validated-island matrix** (`verification.rs`, honesty-gated), expanded under D2
  with four new real-data islands: a **real caesium clock** and **Stable32 PHASE.DAT**
  (Allan estimators), **real ESA OPS-SAT telemetry** (anomaly ROC AUC vs scikit-learn),
  and **ICGEM/GRS80 gravity-functional synthesis** (gravity-aided nav map). Everything
  else stays honestly **MODELLED**.
- **The seven lunar-PNT modules** (lunar time, VLBI, joint OD+clock, frame realisation,
  nav service, differential PNT, interop export) backing the lunar shelf assets.

No TRL > 3, no flight heritage, no certification, no ESA endorsement — the same honesty
posture the open core enforces.

## Build & run

A **release** build depends on the **published open-core crate** (`kshana = "<version>"`),
resolved to an exact version in `Cargo.lock` and advanced with `cargo update -p kshana` as
each open-core release lands.

> **Development pin on branch `feat/pro-power` (not a release pin).** The integrated open
> engine (75 scenario kinds, counted with `cargo run -- kinds --json`) is not published
> yet, so this branch builds against a dedicated, read-only engine worktree by path:
> `kshana = { path = "../kshana-wt-pro-engine", version = "0.28" }`, which is branch
> `feat/pro-engine-base` of the open repository. That tree still reports version 0.28.0
> but is **not** the 0.28.0 on crates.io. **Release step, not done:** before any Pro
> release, replace the path with the published engine version that carries those kinds,
> run `cargo update -p kshana`, and re-run every check below with `--locked`. A path pin
> builds only on a machine that has the worktree; a release must not depend on it.

It used to track a **git dependency on the open core's `main` branch**, pinned to
`version = "0.24"`. That is why it no longer does: a branch pin resolves to whatever the
branch points at, so when the open core's history was rewritten the pinned commit stopped
being reachable from `refs/heads/main` — proven with a clean clone, which did not receive
it. GitHub still served the object by raw SHA, so nothing failed loudly, but the manifest
demanded a version the branch could no longer supply. A published version is immutable and
cannot drift that way.

For local co-development, override with a **path dep** to a sibling `../kshana` checkout
(see the comments in [`Cargo.toml`](Cargo.toml)). Pro never forks the engine — it embeds
it, so the two versions move together.

```bash
cargo test                       # unit + integration tests (proves the overlay)
cargo run -- kinds               # list open kinds (inherited) + Pro kinds + their tier
cargo run -- kinds --json        # the same as JSON, with open_count and pro_count
cargo run -- version
```

The suite is unit tests across every module plus **twenty integration tests** in
[`tests/`](tests) that prove the contract: `overlay.rs` (superset by construction),
`open_engine_reachable.rs` (every open kind the embedded engine lists is run from one of
the engine's own scenario files through the Pro dispatcher and the open ∪ Pro registry,
and must give the open engine's result byte for byte),
`e2e_interop.rs` (end-to-end), `reproducibility_twin.rs` (deterministic twin),
`no_overclaims_mission_analysis.rs` (honesty guard), the study/report/pack path
(`pro_study.rs`, `report_cover.rs`, `report_pdf.rs`, `study_ledger.rs`,
`study_intake.rs`, `pack_c.rs`, `optimism_gap_scenarios.rs`, `study_dossier.rs`),
`mbse.rs` (the MBSE commands over real open-engine runs), the
four `*_in_sync.rs` guards (each committed deliverable matches its generator output, so
docs never drift), and `readme_counts.rs` (the pack and integration-test counts in this
README match the registry and the `tests/` directory).

**Licensing (PR7).** `run` is **fail-closed**: with no `--license` it enforces the
**Evaluation** tier (open scenarios + the free `pro-seam-check` only). A value pack
needs a license token (or `--dev` for local runs in a debug build; a release build
refuses `--dev`). Open scenarios are always free.

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
`pro-trade-study`, `pro-quantum-systematics`, `pro-optimism-gap-audit`,
`pro-scenario-ci`) → **Enterprise** (+ `pro-mbse`, `pro-batch`). Tokens are
Ed25519-signed: `license issue` signs with `$KSHANA_PRO_SIGNING_KEY` (vendor only),
and a build verifies with the public key compiled in from `$KSHANA_PRO_LICENSE_PUBKEY`
(`license keygen` makes the pair). Debug builds fall back to a demo key pair so the
commands above work locally; release builds contain no demo key, and one built
without a public key runs every Pro kind at Evaluation tier.

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
cargo run -- study --dossier scenarios/dossier-holdover-csac.toml --out out/ --dev  # study dossier: HTML + JSON technical note (Pro)
cargo run -- mbse vcrm tests/fixtures/mbse/trace.toml --dev    # MBSE: verification cross-reference matrix over real runs (Enterprise, `pro-mbse`)
cargo run -- mbse pack tests/fixtures/mbse/trace.toml --out out/ --dev  # MBSE evidence pack: HTML + JSON
cargo run -- navdt scenarios/twin-csac.toml                 # A10 NAV-DT twin bundle (JSON)
cargo run -- report scenarios/trade-clock.toml --format html # A11 consolidated report (md|json|html|svg)
cargo run -- report scenarios/trade-clock.toml --pdf out.pdf # A11 pure-Rust PDF export (report_pdf)
cargo run -- ci --pack <manifest.toml> --baseline ci.json --out report.html --json ci.json  # Pack C scenario-CI regression gate (Pro)
cargo run -- mrd                                            # A03/D1.4 machine-checked MRD FoM-mapping (Markdown)
cargo run -- benefits --json                                # A02/D1.1 benefit-vs-baseline table (dual provenance labels)
cargo run -- spacestream --schema                           # A09 SpaceStream codec goldens / ICD schema
cargo run -- serve --addr 127.0.0.1:8088                    # PR22 HTTP service — GET /health /version /kinds · POST /run /study, entitlement-gated
```

**Study dossier (the paid unit).** `study --dossier <scenario.toml> --out <dir>` turns
one scenario into a reproducible evidence dossier laid out like a European Space Agency
(ESA) technical note: identification, scope, applicable and reference documents, method,
results, limitations. It writes `dossier.json`, `dossier.html` and `scenario.run.toml`
(the exact text run) and prints the dossier's SHA-256 result hash; the same scenario
gives the same hash and, without `--utc`, byte-identical files. The results carry the engine and Pro
versions, the engine's scenario hash, a VALIDATED or MODELLED tier on every figure (read
from the open verification matrix), a one-at-a-time sensitivity sweep, a holdover band
and maximum time interval error (MTIE) check for `clock` scenarios, and the Goal
Structuring Notation (GSN) argument, Verification Control Document (VCD) and W3C-PROV
provenance record from the L3 stack. It also maps the run's behaviour to the four levels
of the US Department of Homeland Security (DHS) Resilient PNT Conformance Framework (RPCF)
v2.0 and gives Prevent / Respond / Recover readings for later mapping to IEEE P1952, all
labelled **tentative, simulation-derived, not a certified level**. The optional
`[dossier]` block in the scenario sets the title, subject, figures, sweep and holdover
settings; `scenarios/dossier-holdover-csac.toml` is the worked example. There is no PDF
face for the dossier yet. Pro tier.

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
- `scenarios/dossier-holdover-csac.toml` — `study --dossier`: an open `clock` scenario
  with a `[dossier]` block; the chip-scale atomic clock's holdover through a 110-minute
  GNSS denial, written up as a study dossier.
- `scenarios/spacestream-*.json` — SpaceStream NAV-DT metrics fixtures (nominal / jamming /
  spoof-time / degraded) for the connector / `spacestream` path.

## Deployment

The overlay ships a container and two compose topologies for on-prem / air-gapped /
sovereign delivery — full notes in [`deploy/DEPLOY.md`](deploy/DEPLOY.md).

```bash
# Build the image — the context MUST be the PARENT dir so the `../kshana` path
# dependency resolves (the Dockerfile COPYs BOTH the open core and the overlay):
docker build -f kshana-pro/Dockerfile -t kshana-pro ..

# Run it — fail-closed by default (open engine + free seam-check at Evaluation):
docker run -p 8088:8088 kshana-pro
# Unlock Pro/Enterprise with a license token:
docker run -p 8088:8088 kshana-pro serve --addr 0.0.0.0:8088 --license <token>
```

The runtime image is a non-root `debian-slim` that `EXPOSE`s **8088** and carries a
`HEALTHCHECK` calling `kshana-pro healthcheck` against `/health`, so a hung server is
reported unhealthy. Two compose files cover the modes —
[`deploy/docker-compose.product.yml`](deploy/docker-compose.product.yml) (hosted
product) and [`deploy/docker-compose.sovereign.yml`](deploy/docker-compose.sovereign.yml)
(air-gapped / sovereign). Because `serve` is fail-closed, an image started with no
license still answers for the open engine and `pro-seam-check` only — never a paid pack.

## Discipline (from the open-core strategy)

- **Never fork** the open core — depend on it, extend via seams.
- **Never starve** the open core to upsell Pro (it is the funnel).
- **Never open** the moat: export-controlled data, customer calibrations, Pro IP.
- **Don't build gated *capability* on spec** — the L1/L3 *frameworks* are built and
  run on public data (a BD/bid calling card), but the gated *instances* (a customer
  device twin, a sold SKU) and **all of L2** (legal wall) wait on their triggers.
- **No GPL/copyleft third-party deps** (the open core is AGPL-3.0-only / commercial
  dual-licensed by design; a copyleft *dependency* would break the dual-licence model + ESA terms).
- **Never commit** NDA / customer / export-controlled material (see `.gitignore`).
- **Push discipline:** capability *code* mirrors to the open `kshana` repo; **bid evidence,
  papers, strategy, commercial & legal material stay in this private repo**. Nothing —
  public or private — is pushed without explicit founder go.

## Trademark & contact

"Kshana" is a trademark of Ashforde OÜ. A fork of the open engine may use the
AGPL-3.0-only code; it may not use the name or the canonical distribution.

---

*Ashforde OÜ · all external/business contact: **contact@ashforde.org** · Kshana —
open AGPL-3.0-only, commercial licence available · PRIVATE overlay: do not distribute.*
