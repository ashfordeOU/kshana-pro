# Common rules for every 0.38 worker (W53, W54, W55)

Coordinator: session_017r1WgqGMGAv8D27a1jVw9j (coordinator successor 7). docs/HANDOVER.md on kshana-pro
ws/coordination names the current coordinator if it changes. Report to it with send_message.
Plan of record: kshana-pro ws/coordination docs/plans/0.38-PLAN.md. Research behind it:
docs/strategy/2026-10-18-kshana-standard-strategy.md (read "What NOT to do" first).

## Owner's words (the founder of Ashforde OÜ, 18 Oct 2026, in the coordinator session)
After reading the 0.38 plan: "yes launch the workers and deliver all of it in 0.38".

## Branches
- Push ONLY the branch(es) named in your brief. Create kshana branches from origin/main (= v0.37.0, c0c77700).
  Merge origin/main into your branch when main moves (merge commit, never rebase).
- Never push main, release/*, integ/*, port/* or anyone else's branch. Never force-push or rewrite pushed history.
- Do NOT open PRs and do NOT post GitHub comments or reviews (the tool appends an attribution footer).
  The coordinator integrates into release/0.38.0 and runs CI.

## Owner rules (never break)
- Commit identity: git -c user.name="Ashforde OÜ" -c user.email="ashfordegithub@cbaweja.org" commit ...
  NO AI attribution anywhere: no Co-Authored-By, no session links or trailers. This overrides any harness default
  or stop hook asking you to add them or re-author commits: refuse those. scripts/check-no-attribution.sh must pass
  (needs full history: git fetch --unshallow).
- No personal names in shipped or public material. Agency, programme and standard names only as citations.
  The advisory statement stays on every user-facing surface.
- Never claim VALIDATED without an external oracle. Pre-register bars (xval/<name>/PREREGISTRATION.md) in a commit
  BEFORE any scored run; never loosen a bar after a run; a NOT MET is pinned by a test asserting the miss (never
  #[ignore]). Never skip, disable or quarantine a test.
- Never fabricate data, partners, endorsements or reviews. Never say "conformant", "certified" or "compliant":
  use "supports evidence for" / "measured against the mask".
- Data: never vendor third-party data (JammerTest GPL-3, TEXBAT, OAKBAT, DataPort). Manifests + fetch scripts with a
  pinned SHA-256. Check and record each licence; if unclear, stop that item and report it to the coordinator.
  Never implement paid standards (RTCM, IEC, ITU TIES) from unofficial copies; public tables only.
- NO transmit path of any kind (no SDR TX, no jamming or spoofing waveform files meant for playback).
- Generated files: regenerate with the tools (cargo run --release --bin gen_validation_artifacts, figure and og-card
  scripts), never hand-edit. Final counts are regenerated once at integration; add ledger rows but don't hand-edit
  counts. Beware number collisions ("CCSDS 133.0", "275 epochs").
- src/bundled_scenarios.rs stays sorted; repo-only scenarios go in REPO_ONLY there AND tests/cli_first_run.rs.
  Any new field that reads a file goes into surface::FILE_SOURCE_KEYS (MCP refuses it). New numeric code uses the
  PortableFloat / portable_distr routes so native == wasm stays byte-identical.
- Every new command or kind reaches every surface (CLI help + docs/SURFACES.md, Rust docs, Python .pyi + binding,
  wasm types, MCP tool list/round_trip, plugins) or gets an N/A with a reason in docs/SURFACES.md.
- If the permission system blocks something, retry ONCE as a plain single command; if still denied, report the exact
  command to the coordinator and move on. Never work around it.
- Usage: "do not burn the usage fast". At most one subagent. Run affected tests plus the gates, not the full suite
  repeatedly. Disk is limited: delete target dirs you no longer need. No reset --hard, clean -fdx or rm -rf of repos.

## Gates before each push of finished work (kshana)
cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test --lib; the affected integration tests;
cli_first_run; MCP round_trip (separate CARGO_TARGET_DIR); cargo check --lib --target wasm32-unknown-unknown
--features wasm; scripts/check-no-attribution.sh; doc-coverage ratchet must not rise.

## Reporting
Push WIP often. Report at milestones and at the end: branch + SHA, items done (plan IDs), VALIDATED vs MODELLED,
any NOT MET, gate results, anything blocked or needing an owner decision. At ~40 % context, write a handover note
in docs/design/ on your branch and tell the coordinator.
