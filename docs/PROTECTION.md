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
| `pro` | a Pro licence at the licence tier `license::explicit_tier` names (Evaluation, Pro, Enterprise; unchanged) | `pro-twin`, `pro-trade-study`, `pro-uncertainty`, `pro-batch`, `pro-quantum-systematics`, `pro-optimism-gap-audit`, `pro-conflict-resilience-study`; features `pro-scenario-ci`, `pro-study-dossier`, `pro-signed-dossier`, `pro-uncertainty`, `pro-station-fleet`, `pro-service-ha`, `pro-desktop` |
| `arksa` | only a token with the `arksa` claim, signed by Arksa's issuer | kinds `pro-mbse`, `pro-design-optimise`, `pro-spectrum-coexistence`, `pro-lunar-signal-security`, `pro-beacon-placement`, `pro-lunar-time-budget-study`, `pro-cislunar-observability-study`, `pro-optical-rf-hybrid-study`, `pro-realtime-frame-eop-study`, `pro-beacon-dop`, `pro-surface-beacon-dop-study`; features (commands) `pro-mbse` (`mbse`: SysML v2, ReqIF, VCRM), `pro-design-optimise` (`design`), `pro-spectrum-coexistence` (`coexist`), `pro-mission-dossier` (`mission`), `pro-navdt` (`navdt`), `pro-spacestream` (`spacestream` codec), `pro-mrd` (`mrd`). Since X3 none of their code is in this repository: it is in `ashfordeOU/arksa-kpro` (section 6) |
| `core` | nobody: Core never ships from this crate | none here (P1 to P5 and B6 live only on Arksa servers) |

Code: `src/license/tiers.rs` (`Edition`, `ARKSA_ENTRIES` (the hidden entries by hash),
`compiled_edition`) and `src/extension.rs` (the registration hook). `kshana-pro kinds --json`
prints each Pro kind's `edition`.

Owner decision, 14 November 2026: the lunar surface beacon studies `pro-beacon-dop` and
`pro-surface-beacon-dop-study` are Arksa-exclusive too (they were `pro` before). No
M-workstream (space RF survey) code exists in either repository yet.

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
* **Key slot.** The Arksa issuer keys come only from a registered extension
  (`license::embedded_arksa_issuers` returns `extension::issuer_keys()`). Kshana Pro on its
  own has no Arksa issuer at all, so it refuses every token that carries the claim. The
  arksa-kpro crate reads `KSHANA_PRO_ARKSA_ISSUER_PUBKEY` (64 hex characters, or several
  separated by commas) at its build time; with none set, its debug build trusts the
  placeholder TEST key `arksa_kpro::PLACEHOLDER_ARKSA_ISSUER_PUBKEY` (private half only in
  the tests) and its release build trusts none. No private key is in either repository.
* **Issuing.** `license::issue_arksa(seed, claims)` sets the claim and signs a v2 token. Only
  Arksa's issuer holds such a seed (docs/LICENSING-KEYS.md, section 5). The vendor CLI does
  not issue Arksa tokens.
* **Gate.** `Entitlement::allows` takes the edition from the verified manifest. An
  Arksa-exclusive kind needs `arksa` and an unexpired token; no licence tier and no `features`
  grant unlocks it. A Core kind never runs. A `pro-` kind the manifest does not list is
  refused. `license::check` gives the error: for an Arksa-exclusive or Core kind always the
  same neutral text, "this capability is not available under this licence; it is provided
  only within Arksa", which names no kind and no method. `registry::run_gated`, the Arksa commands
  (through `cli::require_feature` or the module's own gate, in arksa-kpro), the service and
  Desktop Pro (both through `run_gated`) all go through it. Pro kinds behave as before.

Tests: `tests/arksa_entitlement.rs` here (old tokens verify without the claim; the issuer
slot only; a Pro key cannot grant it; tampered claims, key IDs, tier, expiry and licensee
fail; a retired issuer key is refused; this crate contains no Arksa-exclusive kind and
refuses each of them by hash with the neutral message, for a standalone Enterprise token
naming every Arksa id in `features`, for Evaluation and even for an Arksa token; it trusts no
Arksa issuer of its own; the names are read from the term list at run time, so the test
source names none), plus the unit tests in `src/license`. In arksa-kpro,
`tests/arksa_entitlement.rs` checks the hook: exactly the eleven kinds and seven features are
added and all are in the manifest at `arksa`; a standalone Pro token is refused for each,
neutrally; an Arksa token runs each kind through `run_gated`; an expired one runs none; the
server binary refuses at Evaluation and lists the Arksa commands.

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
  gives; every listed name compiled in; every kind and feature a registered extension adds
  listed at `arksa`, by hash). Library users (Desktop Pro) verify the signature
  and form on the first gate check. A manifest that fails makes every Pro kind refuse. Kinds
  it does not list are refused.
* **Keys.** The committed signature is made with a TEST key (private half only in
  `tests/tier_manifest.rs`; public half `tiers::PLACEHOLDER_MANIFEST_PUBKEY`). A release signs
  the manifest with the vendor's offline manifest key and sets
  `KSHANA_PRO_TIER_MANIFEST_PUBKEY` and `KSHANA_PRO_TIER_MANIFEST_SIG` at build time
  (docs/LICENSING-KEYS.md, section 6).
* **Regenerate** after adding, removing or re-tiering a kind:
  `KSHANA_PRO_REGEN_TIER_MANIFEST=1 cargo test --test tier_manifest`. The hidden entries
  come from `tiers::ARKSA_ENTRIES` (hash and type); this crate does not know their names.
  CI fails while the committed file or signature is stale. Regenerated on 14 November 2026
  for X3 with the test key: 33 entries, 18 at `arksa` (11 kinds, 7 features).
* **Limit.** The manifest is embedded at compile time. Its signature catches an edit between
  the vendor and the build; it does not stop someone who patches a built binary. That is why
  the Arksa-exclusive code is not in this repository at all (sections 4 and 6).

Tests: `tests/tier_manifest.rs` (the placeholder is the test key; the committed manifest is
what the code implies; the signature is current; start-up check; counts; a tampered manifest,
another key, a bad signature and every wrong form are refused; unknown kinds are refused even
for Enterprise) and `src/license/tiers.rs`.

## 4. X4: shipped builds carry no Arksa-exclusive or Core code

* **Not in this crate (since X3).** The nine Arksa-exclusive packs of section 1a, the two
  lunar surface beacon studies, the modules `mbse`, `design`, `mission`,
  `spectrum_coexistence`, `lunar_oed`, `mrd`, `connectors::spacestream`, `twin::navdt`,
  `suite::gsn`, `suite::vcd` and the commands `mbse`, `design`, `coexist`, `mission`,
  `navdt`, `spacestream`, `mrd` live in `ashfordeOU/arksa-kpro` (section 6). The cargo
  feature `arksa` is gone. The release packages and Desktop Pro cannot contain that code: it
  is not in their dependency graph. Shared helpers stay here: the criterion grammar and unit
  parser (`src/criterion.rs`, `src/units.rs`), the PDF text helper (`src/pdf_text.rs`), the
  report PDF layout (`report_pdf`, now public for the extension) and `util::dot_path_number`.
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
  `tests/protect_scripts.rs` checks that this crate's binary passes and that a file
  carrying a term fails. (Before X3 it also checked that an `arksa` build failed; that build
  no longer exists here. Keeping the term list in step with the moved code is now an
  arksa-kpro task, section 7.)
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

## 6. X3: the move to `ashfordeOU/arksa-kpro` (done 14 November 2026)

**Dependency direction (fixed):** `arksa-kpro` depends on `kshana-pro` (a git dependency
pinned to one commit) and on `kshana` (the open engine, the same exact version). Never the
reverse. `scripts/protect/source-guard.sh` (CI step "Arksa source guard (X3)") fails when any
`[arksa]` or `[core]` term of `protect/private-terms.txt`, or the server-side crate's name,
appears in a tracked source file, manifest, lockfile, test, fixture, scenario or the Desktop
Pro app (documentation is out of scope; output by term ID only).

**Moved** (copied to arksa-kpro, deleted here): the eleven Arksa-exclusive packs (the nine of
section 1a plus `pro-beacon-dop` and `pro-surface-beacon-dop-study`, owner decision);
`src/mbse/` (`criterion` and `units` stay here), `src/design/`, `src/mission/`,
`src/spectrum_coexistence/`, `src/lunar_oed.rs`, `src/mrd.rs`,
`src/connectors/spacestream.rs`; `src/twin/navdt.rs`, `src/suite/gsn.rs`, `src/suite/vcd.rs`
(owner decision); the seven commands; the tests `design_optimise`, `mbse`, `mission_dossier`,
`spectrum_coexistence`, `mrd_deliverable_in_sync`, `no_overclaims_mission_analysis`,
`spacestream_schema_in_sync`, `e2e_interop`, `reproducibility_twin`, the `with_arksa` half of
`arksa_entitlement` and the `mbse` test of `modelled_label`; `tests/fixtures/mbse/`,
`scenarios/mbse-srd.toml`, `scenarios/pro-power/{design,coexistence,mission}/`,
`scenarios/spacestream-*.json`; `docs/MBSE.md`, `docs/MISSION-DOSSIER.md`,
`docs/DESIGN-OPTIMISER.md`, `docs/SPECTRUM-COEXISTENCE.md`,
`docs/tenders/cross-tender/D1.4-mrd-fom-mapping.md`,
`docs/tenders/13494-ai-payload-pnt/SpaceStream-ICD.md`. The committed study outputs the moved
pack tests compare against (`papers/{cislunar-observability,lunar-time-budget,optical-rf-hybrid,realtime-frame-eop,surface-beacon-dop}/`)
were copied. The copies here were removed on 15 November 2026 (owner decision; every file was
checked byte for byte against the private Arksa repository first), and their titles and file
names were added to the `[pro]` section of the term list so the leak gate catches them in the
open repository. Two of their headings were already public and are left out of the list.

**What Kshana Pro products changed** (owner decision: they keep working without the moved
code, with honest, neutral content):

* `pro-trade-study`: the result no longer has the `gsn` and `vcd` fields, and the assurance
  pack Markdown no longer has the GSN and VCD sections. Gate, provenance, TRL table, ECSS
  Annex L and NASA §6.8 reports are unchanged.
* `suite::vv_dossier` (the credibility dossier): the GSN top goal and the VCD coverage are
  replaced by the count of assurance-gate checks closed (`checks_total`, `checks_closed`,
  `checks_all_closed`); the M&S Management factor and `evidence_complete` use that count.
* Study dossier (`kshana-pro study --dossier`): section 5.5 has a plain table of the study's
  own checks (`assurance.checks`, ids `CHK-010` to `CHK-050`, with evidence and status), in
  place of the GSN argument and the VCD; the trade appendix carries the gate and provenance
  only; the reference documents drop the GSN standard and the ECSS verification standard and
  are renumbered (W3C PROV is RD-4).
* Consolidated report (`kshana-pro report`): its figures-of-merit table is now every numeric
  figure the open run reports under `<subject>.fom`, each with its label and oracle
  (`report::FomSummary`, JSON field `figures`, was `bundle`); the NAV-DT wording is gone.

**The hook.** `src/extension.rs`: an extension crate calls `extension::register` once with
its kinds (metadata, recorded tier, parser, registry factory), command features, commands and
Arksa issuer keys. The dispatcher (`registry::run`, `run_gated`, `pro_kinds`, `pro_registry`,
`kinds_json`), the licence code (`explicit_tier`, `pro_command_features`,
`embedded_arksa_issuers`), the tier code (`compiled_edition`, `startup_check`) and the command
line (`cli::main_with_args`, usage, dispatch) consult it. The `kshana-pro` binary is now a
thin `main` over the library's `cli` module, so the arksa-kpro binary runs the same command
line (`arksa-kpro serve`, `run`, every Pro command, plus the Arksa ones). Registering grants
nothing: every extension id must be at `arksa` in the signed manifest, and runs still need the
claim.

**Stays here:** the `arksa` claim, the issuer slot and `license::check`; the tier manifest with
its hashed entries; the symbol check, the source guard, the leak gate and the term list.

## 7. What remains: X6, X7, X8 (not started)

* **X6, results-only service API for Arksa-exclusive kinds.** Verdict, evidence class and
  signature out; intermediates withheld; per-tenant query budgets; probing alerts (many
  near-identical queries); watermarked outputs; an access log. Runs on Arksa servers, with
  tests for each. X3 is done: `arksa-kpro serve` today is the full Pro service with the Arksa
  kinds added, and returns their full results; X6 wraps that in arksa-kpro.
* **Term list and the moved code.** The symbol check can no longer prove here that the term
  list matches the Arksa code (that build is gone). arksa-kpro should run
  `symbol-check.sh` on its own release binary and require it to FAIL (it needs this
  repository's term list, so a CI checkout of kshana-pro at the pinned rev).
* **Committed study outputs.** Removed on 15 November 2026 (section 6).
* **X7, trade-secret register and patent-before-disclosure rule.** A register (private) of
  each secret method with its owner, date and status; P1, P2, P5 and B6 filed before any
  disclosure, P4 kept secret. Needs the owner and counsel; no code.
* **X8, marketing rule in CI (done, 15 November 2026).** `scripts/protect/arksa-copy-check.py`
  allows "in Arksa" claims only for the four outcome names the owner approved (Mission
  navigation digital twin; Lunar navigation and timing assurance; Independent interference
  referee reports; Spaceborne GNSS receiver interference assurance) and fails on any other
  claim and on any term of the private list within 300 characters of a claim. Scopes: `kpro`
  (this repository's docs and marketing; `ci.yml`), `open` (the open repository;
  `open-leak-gate.yml`), `site` (a website source tree). Findings print term IDs only. Tests:
  `tests/copy_check.rs`.

## 8. Honest limits

* Source protection is that both repositories are private. An `arksa-kpro` build contains
  the Arksa-exclusive code; it is for Arksa servers and contracted Arksa Sovereign installs
  only, never a customer device.
* Hashed manifest entries hide names from a casual reader, not from someone who guesses
  them: a short name can be checked against its hash.
* The binary check and the source guard find the listed terms only. A new Arksa-exclusive
  module must add its names to `protect/private-terms.txt`.
