# RUNNER lane handoff

State as of 2026-10-07 20:50Z, written by the previous (Opus) runner and committed by the coordinator.
Main is 2ef6541f (#34, #40, #41 and #42 merged).

## Open watch items (ashfordeOU/kshana)

| PR | head | last CI run | conclusion | vs main 2ef6541f |
|---|---|---|---|---|
| 38 | 79ff31e7 (merge of main into feat/iq-track-default) | none yet on this head; previous #524 (37641898570) @b41a61d6 success | pending | clean |
| 43 | 0020a769 | #539 (37658179841) | success | clean |
| 44 | 15107fac | #530 (37649275102) | success | CONFLICT: CHANGELOG.md, tests/iq_cli.rs |
| 48 | fee6f3d4 | #536 (37655450143) | success | CONFLICT: CHANGELOG.md, tests/iq_cli.rs |
| 50 | 8655d92c | #543 (37659343346) | in progress | CONFLICT: CHANGELOG.md, tests/iq_cli.rs |

Stack: #44 contains #38, #48 contains #44, #50 contains #48; not restacked yet. When new heads land,
report the superseded run ids (#530, #536, #543) rather than their results.

Not on the watch list but seen: #45 (#541, #542 FAILURE), #49 (#538, #544 FAILURE), #51 (#545 in progress).

## Main push runs
- #546 @11b39f67 (#34): in progress at handoff
- #547 @1b6e826b (#40): in progress
- #548 @564e9377 (#41): queued
- #549 @2ef6541f (#42): queued
- #540 @18e860a7: success

## Issue #36 (macOS fixture tolerance, W2 on fix/macos-fixture-tolerance)
- Mac probe run 37653732273 @9925c103 found 10 platform floating-point fixture failures, with no infra
  problems.
- The per-key analysis of failure #6 (lunar_joint_od A) went to W2: H0 is off by 1 ULP/2e-6 in the
  finite-difference Jacobian, and the solve amplifies that to about 1e-4 relative in x_hat.
- Pending: once W2 pushes, the coordinator dispatches a Mac probe on that sha and the runner verifies
  it.

## Local CI notes
- Native Linux reproduction needs CARGO_INCREMENTAL=0 and CARGO_PROFILE_{DEV,TEST}_DEBUG=0 (disk quota).
- cargo-deny needs a manual advisory-db clone plus --disable-fetch.
- check-no-attribution needs full history (git fetch --unshallow).
- Simulate merges with `git merge-tree --write-tree origin/main <head>`.
