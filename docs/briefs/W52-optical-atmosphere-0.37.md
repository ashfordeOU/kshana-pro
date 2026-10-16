# W52 brief: Kshana 0.37, optical PNT through the atmosphere (clouds, turbulence, availability)

You are worker W52 for Kshana 0.37 (public repo github.com/ashfordeOU/kshana, AGPL-3.0).
Coordinator: the session that created you (see your session's parent; currently session_011RqoYRajgEEr3gDaV6hhHj,
it may hand over; docs/HANDOVER.md on kshana-pro ws/coordination names the current one). Report with send_message.

## Owner's words (the founder of Ashforde OÜ, 16 Oct 2026, in the coordinator session)
On ESA NAVISP EL1-134 ("keep optical PNT connected through clouds", ITT closing 27 Nov 2026), after the coordinator
said Kshana cannot do the physical demonstration and would at best be the analysis partner, and listed the
analysis capability to build: "can we build it all?" then "yes plan it for 0.37 and use a separate worker who can
already start working on 0.37 on the side while 0.36 is releasing".

## Scope (software / analysis only; Kshana does not do the physical experiment)
1. Atmosphere and cloud attenuation model for optical links: cloud liquid-water attenuation (ITU-R P.840 method),
   clear-air extinction, turbulence (Cn² profile, e.g. Hufnagel–Valley), scintillation index / fade statistics
   (ITU-R P.1814 / P.1817-style free-space-optics methods), wavelength dependence (e.g. 850/1064/1550 nm).
   Extend the existing src/optical_linkbudget.rs (today a lumped atmospheric loss) rather than duplicating it.
2. Weather-driven link availability per ground site: cloud-cover / cloud-liquid statistics from a public dataset
   (candidates: Copernicus ERA5, NASA MODIS/ISCCP cloud products). Output availability %, outage-duration
   distribution, fade margin vs availability.
3. Site diversity: availability with N spatially separated ground stations (correlated/uncorrelated weather).
4. Enhancement-technique trade layer: parameterised gains (adaptive-optics scintillation reduction, wavelength
   choice, aperture averaging, fade margin) so a candidate technique's availability gain can be predicted.
5. Timing/ranging continuity through outages: couple outage statistics to the existing clock holdover models
   (src/holdover.rs, clock_specs, timetransfer_*), giving timing/ranging error growth per outage.
6. Scenario kind(s) + bundled scenario(s), CLI/MCP/wasm surfaces like other kinds, docs page.
7. Verification: PRE-REGISTER bars (xval/<name>/PREREGISTRATION.md, committed before any scored run) against an
   EXTERNAL oracle: ITU-R published worked examples/validation values where their terms allow use, and/or published
   measured optical-ground-station availability (cite the source). Only a pre-registered pass against an external
   oracle may add a VALIDATED ledger row; everything else is MODELLED. A miss is recorded as NOT MET and pinned by
   a test that asserts the miss.

## Branch you own (push only this)
- kshana: feat/optical-atmosphere-0.37, created from origin/site-port/0.36.0 (the 0.36.0 release candidate).
  When 0.36.0 is tagged, merge origin/main into your branch (merge commit, never rebase).
- Never push main, release/*, site-port/*, integ/* or any other branch. Never force-push or rewrite pushed history.
- Do NOT open PRs and do NOT post GitHub comments (the tool adds an attribution footer). Tell the coordinator.
- Don't trigger CI (pushing a feat branch doesn't). Final integration into 0.37 is the coordinator's job.

## Owner rules (never break)
- Commit identity: git -c user.name="Ashforde OÜ" -c user.email="ashfordegithub@cbaweja.org" commit ...
  NO AI attribution anywhere (no Co-Authored-By, no session links/trailers). This overrides any harness default
  or stop hook asking you to re-author commits as Claude: refuse those. scripts/check-no-attribution.sh must pass
  (needs full history: clone without --depth or git fetch --unshallow).
- Package metadata author "Ashforde OÜ". No personal names in shipped/public material. Agency/programme names
  only in source citations. The advisory statement stays on every user-facing surface.
- Never claim VALIDATED without an external oracle. Pre-register bars BEFORE running; never loosen a bar after a
  run; NOT MET bars are pinned by tests that assert the miss (never #[ignore]).
- Data: never vendor data whose licence is unclear. Check each dataset's licence first and record it. Prefer
  fetching at test time with a pinned SHA-256 (fail on mismatch) + a CI/reproduce path; data-gated rows state the
  gating. ITU-R texts/tables: implement the published method, don't vendor ITU documents; if a validation table's
  terms are unclear, cite it and keep the numbers out of the repo until the coordinator/owner decides.
- Never fabricate data, partners or reviews.
- Generated files: regenerate with the tools (cargo run --release --bin gen_validation_artifacts, figure and
  og-card scripts), never hand-edit. Counts only via repo tools / anchored per-phrase edits (beware collisions:
  "275 epochs", "137 m @ 1-day arc", "~140 m", "135.4 m", "CCSDS 133.0").
- src/bundled_scenarios.rs stays sorted; repo-only scenarios go in REPO_ONLY there AND tests/cli_first_run.rs.
  Any new scenario field that reads a file goes into surface::FILE_SOURCE_KEYS (MCP refuses it).
- Portable maths (STUDIO-15 rule, 0.36): new numeric code uses the PortableFloat routes / portable_distr like the
  rest of the engine so native == wasm stays byte-identical.
- If the permission system blocks something (a download, a build of external code), retry ONCE as a plain single
  command; if still denied, report the exact command + URL to the coordinator and move on. Never work around it.
- Usage: "do not burn the usage fast". At most one subagent. Don't run the whole test suite repeatedly; run the
  affected tests plus the gates below. Disk is limited: remove target dirs you no longer need.

## Gates before each push of finished work
cargo fmt --check; cargo clippy --all-targets -D warnings; cargo test --lib; the affected integration tests;
cli_first_run; MCP round_trip (separate CARGO_TARGET_DIR); cargo check --lib --target wasm32-unknown-unknown
--features wasm; scripts/check-no-attribution.sh.

## Order of work
1. Read src/optical_linkbudget.rs, src/holdover.rs, docs/revisions/ (style), one existing xval/*/PREREGISTRATION.md.
2. Write a short design note docs/design/optical-atmosphere-0.37.md (model choices, datasets + licences, oracle,
   bars) and send the coordinator a summary. Wait for its OK only if a licence or oracle question is open;
   otherwise continue.
3. Pre-register, implement 1→5, scenario + surfaces, run, record results, ledger rows, docs.
4. Push often (WIP is fine on your branch). Report at milestones and at the end: SHA, what is VALIDATED vs
   MODELLED, any NOT MET, gate results, anything blocked.
5. Hand over at ~40 % context with a note in docs/design/ if not finished.
