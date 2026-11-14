# Protecting Arksa-exclusive Pro and Core (roadmap 0.42, X1 to X8)

PRIVATE. This page names Arksa-exclusive kinds and the checks that keep them out of shipped
builds. It belongs to this private repository only.

Source of the rules: `docs/plans/ROADMAP-0.42.md` (branch `ws/coordination`), sections 1a and
1b (owner, 14 November 2026). The rule follows where code runs: only code that never ships
stays secret. Anything already published stays public.

## 1. Four edition tiers

Every Pro scenario kind and Pro command feature has one edition tier, written in the signed
tier manifest (section 3):

| Tier | Runs for | Kinds and features in this repository |
|---|---|---|
| `open-seam` | anyone, no licence | `pro-seam-check` |
| `pro` | a Pro licence at the licence tier `license::explicit_tier` names (Evaluation, Pro, Enterprise; unchanged) | `pro-twin`, `pro-trade-study`, `pro-uncertainty`, `pro-batch`, `pro-quantum-systematics`, `pro-optimism-gap-audit`, `pro-beacon-dop`, `pro-surface-beacon-dop-study`, `pro-conflict-resilience-study`; features `pro-scenario-ci`, `pro-study-dossier`, `pro-signed-dossier`, `pro-uncertainty`, `pro-station-fleet`, `pro-service-ha`, `pro-desktop` |
| `arksa` | only a token with the `arksa` claim, signed by Arksa's issuer | kinds `pro-mbse`, `pro-design-optimise`, `pro-spectrum-coexistence`, `pro-lunar-signal-security`, `pro-beacon-placement`, `pro-lunar-time-budget-study`, `pro-cislunar-observability-study`, `pro-optical-rf-hybrid-study`, `pro-realtime-frame-eop-study`; features (commands) `pro-mbse` (`mbse`: SysML v2, ReqIF, VCRM), `pro-design-optimise` (`design`), `pro-spectrum-coexistence` (`coexist`), `pro-mission-dossier` (`mission`), `pro-navdt` (`navdt`), `pro-spacestream` (`spacestream` codec), `pro-mrd` (`mrd`) |
| `core` | nobody: Core never ships from this crate | none here (P1 to P5 and B6 live only on Arksa servers) |

Code: `src/license/tiers.rs` (`Edition`, `ARKSA_KINDS`, `ARKSA_FEATURES`, `compiled_edition`).
`kshana-pro kinds --json` now also prints each Pro kind's `edition`.

Open questions for the owner: `pro-beacon-dop` and `pro-surface-beacon-dop-study` are lunar
surface beacon studies; section 1a lists "lunar packs (time budget, beacon placement,
observability, signal security, lunar OED)" and not these two, so they stay `pro` until the
owner decides. No M-workstream (space RF survey) code exists in this repository yet.

## 2. X1: the `arksa` entitlement claim

* **Claim.** `LicenseClaims.arksa` (JSON `"arksa": true`). Optional on the wire and left out
  when false, so every token issued before it existed has exactly the same bytes and still
  verifies; it simply lacks the claim. `Entitlement.arksa` carries the verified value.
* **Only Arksa's issuer grants it.** The keyring has a separate Arksa issuer slot
  (`Keyring::add_arksa_issuer`). A token whose payload carries `arksa: true` is verified only
  against the Arksa issuer keys; every other token only against the Pro keys. So a Pro key
  cannot grant the claim, adding the claim to a Pro token breaks its signature, and removing
  it from an Arksa token breaks it too (the token's key is then unknown to the Pro set).
  Retired key IDs apply to both sets.
* **Key slot.** `KSHANA_PRO_ARKSA_ISSUER_PUBKEY` (64 hex characters, or several separated by
  commas) at build time, read only by a build with the `arksa` feature
  (`license::embedded_arksa_issuers`). A default build has no Arksa issuer at all, so it
  refuses every token that carries the claim. The placeholder
  `license::PLACEHOLDER_ARKSA_ISSUER_PUBKEY` is a TEST key: its private half exists only in
  `tests/arksa_entitlement.rs`; a debug build with the `arksa` feature and no key set trusts
  it for local runs; a release build never does. No private key is in the repository.
* **Issuing.** `license::issue_arksa(seed, claims)` sets the claim and signs a v2 token. Only
  Arksa's issuer holds such a seed (docs/LICENSING-KEYS.md, section 5). The vendor CLI does
  not issue Arksa tokens.
* **Gate.** `Entitlement::allows` takes the edition from the verified manifest. An
  Arksa-exclusive kind needs `arksa` and an unexpired token; no licence tier and no `features`
  grant unlocks it. A Core kind never runs. A `pro-` kind the manifest does not list is
  refused. `license::check` gives the error: for an Arksa-exclusive or Core kind always the
  same neutral text, "this capability is not available under this licence; it is provided
  only within Arksa", which names no kind and no method. `registry::run_gated`, the `mbse`,
  `mission`, `design`, `coexist`, `navdt`, `spacestream` and `mrd` commands, the service and
  Desktop Pro (both through `run_gated`) all go through it. Pro kinds behave as before.

Tests: `tests/arksa_entitlement.rs` (old tokens verify without the claim; the issuer slot
only; a Pro key cannot grant it; tampered claims, key IDs, tier, expiry and licensee fail; a
retired issuer key is refused; a standalone Pro token at Enterprise tier that even names
every Arksa-exclusive id in `features` is refused for each of them with the neutral message;
an Arksa-entitled token runs each of the nine kinds; an expired one runs none; a default
build refuses them by hash), plus the unit tests in `src/license`.

## 3. X2: the signed tier manifest

* **File.** `commercial/tier-manifest.json` with its Ed25519 signature
  `commercial/tier-manifest.json.sig` (hex, over the exact file bytes). Format
  `kshana-pro-tier-manifest`, version 1; one entry per kind or feature: `kind` (the name, for
  `open-seam` and `pro`) or `kind_sha256` (for `arksa` and `core`), `type` (`kind` or
  `feature`), `tier`. Hidden tiers are written by hash only, so a default build embeds the
  manifest without carrying a single Arksa-exclusive name and still refuses those names.
* **Verified at start-up.** `tiers::startup_check` runs first in `kshana-pro` (any command,
  the service included): the signature must verify, the form must hold (names only in open
  tiers, hashes only in hidden ones, no entry twice, known tiers and types), and the manifest
  must agree with the code (every compiled kind and feature listed at the tier the code
  gives; every listed name compiled in). Library users (Desktop Pro) verify the signature
  and form on the first gate check. A manifest that fails makes every Pro kind refuse. Kinds
  it does not list are refused.
* **Keys.** The committed signature is made with a TEST key (private half only in
  `tests/tier_manifest.rs`; public half `tiers::PLACEHOLDER_MANIFEST_PUBKEY`). A release signs
  the manifest with the vendor's offline manifest key and sets
  `KSHANA_PRO_TIER_MANIFEST_PUBKEY` and `KSHANA_PRO_TIER_MANIFEST_SIG` at build time
  (docs/LICENSING-KEYS.md, section 6).
* **Regenerate** after adding, removing or re-tiering a kind:
  `KSHANA_PRO_REGEN_TIER_MANIFEST=1 cargo test --features arksa --test tier_manifest`.
  CI fails while the committed file or signature is stale.
* **Limit.** The manifest is embedded at compile time. Its signature catches an edit between
  the vendor and the build; it does not stop someone who patches a built binary. That is why
  the Arksa-exclusive code is not in shipped binaries at all (section 4).

Tests: `tests/tier_manifest.rs` (the placeholder is the test key; the committed manifest is
what the code implies; the signature is current; start-up check; counts; a tampered manifest,
another key, a bad signature and every wrong form are refused; unknown kinds are refused even
for Enterprise) and `src/license/tiers.rs`.

## 4. X4: shipped builds carry no Arksa-exclusive or Core code

* **Cargo feature `arksa`, off by default.** With it off, these are not compiled at all: the
  nine Arksa-exclusive packs; the modules `mbse`, `design`, `mission`, `spectrum_coexistence`,
  `lunar_oed`, `mrd` and `connectors::spacestream`; the commands `mbse`, `design`, `coexist`,
  `mission`, `navdt`, `spacestream`, `mrd`; their usage lines and tier entries. The default
  build, the release packages and Desktop Pro (which links the overlay with default features)
  build without them. To keep the Pro products whole, three shared helpers moved out of the
  gated modules: the result-path criterion grammar and unit parser (`src/criterion.rs`,
  `src/units.rs`, used by Pack C) and the PDF text helper (`src/pdf_text.rs`); the engine
  version probe moved to `report_brand`. The gated modules re-export them. Older pages
  (docs/MBSE.md, docs/MISSION-DOSSIER.md, docs/DESIGN-OPTIMISER.md,
  docs/SPECTRUM-COEXISTENCE.md) show `cargo run -- <command>`; these commands now need
  `cargo run --features arksa -- <command>`.
* **Still in the default build (moves with X3).** `twin::navdt` (the consolidated report and
  the trade study are built on its bundle) and `suite::gsn` / `suite::vcd` (the Pro study
  dossier writes a GSN argument and a VCD). Their command (`navdt`) is gated; the code is not.
  Removing them means splitting the report and the study dossier, which changes sold Pro
  products; it needs the owner's decision. Their names are in the `[arksa-pending]` section
  of the term list: checked in the open repository now, in binaries once X3 lands.
* **Stripped release builds.** `[profile.release] strip = "symbols"`, `debug = false` in this
  crate and in `apps/desktop-pro/src-tauri`. Stripping removes the symbol table, not string
  literals or panic paths, so the check below scans strings too.
* **The check.** `scripts/protect/symbol-check.sh [--require-stripped] <binary|bundle>...`
  scans every file (a directory recursively, e.g. a macOS `.app`) for the `[arksa]` and
  `[core]` terms of `protect/private-terms.txt` and fails on any; with `--require-stripped` it
  also fails when an executable's symbol table names the overlay. It prints term IDs (first
  12 hex characters of the term's SHA-256), never terms; `scripts/protect/term-id.sh <ID>`
  looks one up locally.
* **Where it runs.** CI (`ci.yml`, on the release binary the release smoke builds, plus
  clippy and the tests of the `arksa` build); Desktop Pro (`desktop-pro.yml`, a release build
  of the app binary); the signed Desktop Pro release (`desktop-pro-release.yml`, the whole
  `.app` before the disk image is signed); the Windows package (`packages.yml`).
  `tests/protect_scripts.rs` checks that the default binary passes and that a binary built
  with `arksa` FAILS, so the term list keeps matching the code.
* **Run locally:** `cargo build --release --bin kshana-pro && scripts/protect/symbol-check.sh
  --require-stripped target/release/kshana-pro`.

## 5. X5: the leak gate on the open repository

* `scripts/protect/open-leak-gate.sh` clones the open repository read-only (anonymous,
  shallow, credentials cleared; nothing is pushed) or takes a checkout (`--dir`), and scans
  every tracked text file and every tracked path for every term of
  `protect/private-terms.txt` (Pro, Arksa-exclusive, pending and Core).
* Matches already published before the gate existed stay public and are listed in
  `protect/open-leak-baseline.txt` (open path and term ID; six today, all anchors of the
  editions page). Any other match fails the gate. Output is "<open path>  term <ID>" only.
* Workflow `.github/workflows/open-leak-gate.yml` in this private repository: hourly, on
  changes to the term list or the gate, on a `repository_dispatch` of type `open-repo-push`,
  and by hand. Triggering it on every open-repository push needs a dispatch step in the open
  repository with a token for this one; that is not wired (the open repository was not
  touched), and it is an owner step.
* The term list lives only here. Never copy it, or a line of it, to a public place.

## 6. X3: plan (repository created; nothing moved yet)

The owner created the private repository `ashfordeOU/arksa-kpro` (14 November 2026). This task
did not move code there; the move is the next step.

**Dependency direction (fixed):** `arksa-kpro` depends on `kshana-pro` (for the
`ExternalPack` seam, `license`, `registry`, `report_pdf`, `report_brand`, `brand_pdf`,
`pdf_text`, `criterion`, `units`) and on `kshana` (the open engine). Never the reverse:
`kshana-pro` and `kshana` must not name `arksa-kpro` in any manifest, lockfile or source, and
the `[arksa]` terms plus a guard like `scripts/core-guard.sh` will check it.

**Move to `arksa-kpro`, as a crate of its own:**

* Packs: `src/packs/mbse_pack.rs`, `design_optimise_pack.rs`, `spectrum_coexistence_pack.rs`,
  `lunar_signal_security_pack.rs`, `beacon_placement_pack.rs`,
  `lunar_time_budget_study_pack.rs`, `cislunar_observability_study_pack.rs`,
  `optical_rf_hybrid_study_pack.rs`, `realtime_frame_eop_study_pack.rs`.
* Modules: `src/mbse/` (less `criterion` and `units`, which stay in `kshana-pro`),
  `src/design/`, `src/mission/`, `src/spectrum_coexistence/`, `src/lunar_oed.rs`,
  `src/mrd.rs`, `src/connectors/spacestream.rs`.
* Commands now in `src/main.rs` under `#[cfg(feature = "arksa")]`: `mbse_cmd`, `mission_cmd`,
  `design_cmd`, `coexist_cmd`, `navdt_cmd`, `arksa_feature`, the `mrd` and `spacestream` arms
  and `USAGE_ARKSA` (an `arksa-kpro` binary, or a server entry point there).
* Tests: `tests/design_optimise.rs`, `mbse.rs`, `mission_dossier.rs`,
  `spectrum_coexistence.rs`, `mrd_deliverable_in_sync.rs`, `no_overclaims_mission_analysis.rs`,
  `spacestream_schema_in_sync.rs`, `e2e_interop.rs`, `reproducibility_twin.rs`, the
  `with_arksa` half of `arksa_entitlement.rs`, the `mbse` test of `modelled_label.rs`.
* Fixtures, scenarios and docs: `tests/fixtures/mbse/`, `scenarios/mbse-srd.toml`,
  `scenarios/pro-power/{design,coexistence,mission}/`, `scenarios/spacestream-*.json`,
  `docs/MBSE.md`, `docs/MISSION-DOSSIER.md`, `docs/DESIGN-OPTIMISER.md`,
  `docs/SPECTRUM-COEXISTENCE.md`, `docs/tenders/cross-tender/D1.4-mrd-fom-mapping.md`,
  `docs/tenders/13494-ai-payload-pnt/SpaceStream-ICD.md`.
* Later (after the owner decides what the sold Pro report and study dossier keep):
  `src/twin/navdt.rs`, `src/suite/gsn.rs`, `src/suite/vcd.rs`; then their names move from
  `[arksa-pending]` to `[arksa]`.

**Stays in `kshana-pro`:** the `arksa` claim, the Arksa issuer slot and `license::check`;
the tier manifest with its hashed entries (so Pro builds still refuse the names); the
symbol check, the leak gate and the term list.

**Steps:** (1) create the crate in `arksa-kpro` with the files above, registering its packs
through `ExternalPack::register_into` into `kshana_pro::registry::pro_registry()` and running
them through `kshana_pro::registry::run_gated` (or a dispatcher there that calls
`license::check` first); (2) give `kshana-pro` a registration hook so the server binary can
add the Arksa packs (today the dispatcher's match table is internal); (3) delete the files
and the `arksa` feature from `kshana-pro`; (4) the Arksa server build links `arksa-kpro`;
Desktop Pro and the packages cannot contain it because it is not in their dependency graph;
(5) Core stays in the private `kshana-core`, Arksa servers only.

## 7. What remains: X6, X7, X8 (not started)

* **X6, results-only service API for Arksa-exclusive kinds.** Verdict, evidence class and
  signature out; intermediates withheld; per-tenant query budgets; probing alerts (many
  near-identical queries); watermarked outputs; an access log. Runs on Arksa servers, with
  tests for each. Needs X3 (the kinds live in the server-only crate) and the service design.
* **X7, trade-secret register and patent-before-disclosure rule.** A register (private) of
  each secret method with its owner, date and status; P1, P2, P5 and B6 filed before any
  disclosure, P4 kept secret. Needs the owner and counsel; no code.
* **X8, marketing rule in CI.** A copy check over the site and docs (open repository and
  site sources) that fails on method names, equations, parameters or diagrams of "In Arksa"
  capabilities before their filing date; outcome names only. It can reuse the term list and
  the leak gate's reporting by ID; it needs the owner's list of allowed outcome names.

## 8. Honest limits

* Source protection is that this repository is private. A build with the `arksa` feature
  contains the Arksa-exclusive code; it is for Arksa servers and contracted Arksa Sovereign
  installs only, never a customer device.
* Hashed manifest entries hide names from a casual reader, not from someone who guesses
  them: a short name can be checked against its hash.
* The binary check finds the listed terms only. A new Arksa-exclusive module must add its
  names to `protect/private-terms.txt` (and `tests/protect_scripts.rs` fails when the
  `arksa` build no longer matches any term).
