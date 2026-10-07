Updated: 2026-10-07T12:05:42Z · Coordinator: session_015EnPpasos1W6PQWi7rHJxu

# Kshana — LIVE status (single source of what is happening now)

Rules: see `docs/RECOVERY.md`. If `Updated` is >30 min old while anything below is
running, assume the coordinator is gone and take over from this file.

## Goal
Continue the post-0.32.0 plan (`handover/post-0.32/README.md` §7 on `main`) after the
2026-10-07 move to the Claude Team account (old sessions lost; git intact).
No external deadline set. Next engine release (0.33.0) not yet scheduled.

## Repo heads (from `git ls-remote`, 2026-10-07T11:40:06Z)
| Repo | Branch | Sha | State |
|---|---|---|---|
| kshana-pro | main | 1ca4c90 | engine `=0.32.0`; PR 5, 6, 7, 8 merged; handover §9 current |
| kshana-pro | partners/iq-deliverables | 4c7b6e2 | PR 7 MERGED → 78ab489 |
| kshana-pro | claude/pro-030 | d94b280 | stale, superseded by PR 5; delete approved, blocked by proxy (founder deletes) |
| kshana | main | f570219 | v0.32.0 released; CI green (run 37283754192) |
| kshana | feat/mcp-iq | a477eec | PR 34: guard fix pushed (15 doc-sync/source-guard binaries 60 passed; MCP 33 passed); CI re-running; runner re-verify queued |
| kshana | feat/iq-validation-ledger | bb7edf8 | PR 35 open (W2); CI pending; full suite via runner + CI |

## Running now
| What | Sha | Where to look |
|---|---|---|
| kshana-pro CI on main push | db2210a | Actions › CI (self-hosted `macmini-kshana-pro`) |
| Runner: amd64 build of act image, then VERIFY PR 7 | db2210a / dbc8c81 | runner session transcript |

## Sessions (environment AshfordeKSH)
| Id | Lane | Branch | State |
|---|---|---|---|
| session_015EnPpasos1W6PQWi7rHJxu | Coordinator | ws/coordination | active |
| session_012dpyTZ3bYvwaTG67ckWjpU | Runner: native Linux x86_64 VERIFY + GH WATCH, read-only (rows recorded by coordinator) | — | PR 35 @bb7edf87 native started 12:05:24Z (guards, paper, site tests, doc_sync, clippy, test --all, reproducible, doc-coverage, sbom, repeatability N=3); then PR 34 @a477eec re-verify + CI watch |
| session_01U47Y1DtpBY9SupJum3iE6Q | W1 §7-B1 IQ over MCP (public) | feat/mcp-iq @ a477eec | PR 34: CI red on 6fae76b (run 37615528447, scenario_count_doc_sync MCP README guard scanned server.rs only) → fixed a477eec (guard scans server.rs + iq.rs, asserts iq_tool_router is merged); local: 15 doc_sync/source_guards binaries 60 passed, MCP 33 passed, clippy OK; CI re-run pending (unverified); idle |
| session_01H3zn1aikR8DL8xLVyNASRf | W2 §7-B2 IQ ledger rows (public) (row recorded by coordinator) | feat/iq-validation-ledger | bb7edf8: 26e1fe6 rows+artifacts, 225176e guard-forced counts (PNG re-rendered, Chromium/Liberation Sans), bb7edf8 droppable; clippy/no-attribution/version-sync/doc-coverage/site tests OK; PR 35 open, idle |
| session_01BDhQakgc2eHdqnQoRfx2s9 | W3 §7-C1/C2 partner summary + notebook (pro) | partners/iq-deliverables @ 4c7b6e2 | DONE — PR 7 merged (78ab489); idle |
| session_01YCEg68jFdLERESsPcMmyMz | W4 pro hygiene | chore/pro-hygiene | DONE — PR 6 merged (db2210a); idle |

## Decisions
- 2026-10-07 No AI attribution in commits/PRs (repo rule beats harness defaults). The GitHub tool
  appends a footer to PR bodies on create → strip it with an update right after.
- 2026-10-07 campaign-watch fixture compares metrics within 8 ulp: macOS arm64 vs Linux x86-64
  differ 1–2 ulp (evidence: Mac run 37612852644 red, c3c20c1 fix, run 37614073848 green).
- 2026-10-07 D1.4 fusion rows are VALIDATED (NaveGo drive, GNSS present throughout); coasting
  NOT validated; MRD-INT-04 stays follow-on (PR 6).
- 2026-10-07 The gps-sdr-sim ledger row stays MODELLED (pre-registered bar f88f2969 failed on
  230/1152 fields); never relax a pre-registered bar after the fact.
- Workers open PRs; only the coordinator merges.
- 2026-10-07 W2 rows approved: "GNSS IQ scene signal geometry vs gps-sdr-sim" ExternalDataset/VALIDATED (pre-registered 7777da5, pushed 11:15:27Z before any run; 11 ch, pseudorange 1.6e-3 m vs bar 0.05 m); "GNSS IQ acquisition on gps-sdr-sim I/Q" MODELLED (bar B2 failed: 21 unsimulated PRNs cross the Gaussian threshold). Guard-forced count edits to README/web approved; hand glyph-edit of validation-provenance.png rejected (re-render or revert); unforced README image refresh split into a droppable commit.

## Open for the founder
1. Delete stale branches — founder approved 11:40; cloud git proxy refused (HTTP 403 on ref
   delete, pushes OK) → founder to delete from GitHub UI or a local clone: `claude/pro-030`
   (d94b280), `chore/pro-hygiene` (b398677, merged), `handover/kickoff` (73d5451),
   `handover/post-0.32` (a96bf49), `handover/where-is-what` (31f6e22),
   `salvage/local-artifacts` (e493d8c), `partners/baseband-brief` (2ad7b6b, merged). All verified
   merged or content-on-main at 11:41.
2. File the public macOS-local test-failures issue (§7-D2)?
3. Optional full site re-render at 0.32.0 (§7-D1)?
4. Public engine: `iq_track` default `acq_coherent=1` (~667 Hz Doppler bins) false-locked 2/3
   channels ~500 Hz off in the W3 notebook; change the default in 0.33.0? (behaviour change)

## Next
- Merge PR 7 when Mac CI green + runner VERIFY green.
- Review W2 row proposal; review W1 MCP design; merge their public PRs after CI (~2.5 h `check`).
- Fold the 0.32.0 status into `handover/post-0.32/README.md` §9 once PR 7 lands.

## Log (newest first, UTC)
- 12:05 [RUNNER] kshana PR 34 @6fae76b4 partial native: toolchain/fmt/no-attribution/version-sync/mcp-check/deny/mcp-deny/wasm32/clippy/public-standalone all 0; test --all stopped (superseded by a477eec).
- 12:05:24 PR 8 merged (Mac CI run 37617589011 success) → kshana-pro main 1ca4c90.
- 12:04:54 Coordinator container restarted ~12:00 (disk kept, heartbeat process lost → restarted). W1 pushed a477eec fixing the PR 34 doc-sync guard.
- 12:03:24 [W1] PR 34 → a477eec: README tool guard now scans iq.rs; repro failed before, 60/60 doc-sync+source_guards pass after.
- 11:57:58 PR 34 RED → W1 asked to widen tests/scenario_count_doc_sync.rs to scan server.rs + iq.rs. kshana-pro PR 8 opened (handover §9 status).
- 11:56:07 [RUNNER] kshana PR 34 @cff687fb: RED, run 37615528447 coverage: scenario_count_doc_sync::the_mcp_readme_lists_exactly_the_tools_the_server_serves; guard scans only server.rs, IQ tools live in mcp/kshana-mcp/src/iq.rs; same expected on 6fae76b4.
- 11:56:47 PR 7 merged → kshana-pro main 78ab489.
- 11:55:44 [RUNNER] kshana-pro PR 7 @4c7b6e2: GREEN, Mac CI #56 (37616091434) success; native 804 passed, 0 failed, 9 ignored; notebook exit 0.
- 11:51:36 [RUNNER] kshana-pro main @db2210a: GREEN, Mac CI #55 (37615463568) success.
- 11:56 [RUNNER] PR 34 partial native: toolchain/fmt/no-attribution/version-sync/mcp-check/wasm32 0; deny + mcp-deny 0 using a locally cloned advisory-db (f246cde) with --disable-fetch (sandbox proxy blocks RustSec fetch); clippy/test/package/msrv running.
- 11:56:12 W2 opened kshana PR 35 (IQ ledger rows); footer verified absent. PR 34 and 35 both edit CHANGELOG [Unreleased] → expect a conflict on the second merge.
- 11:53:25 W2 pushed bb7edf8 (conditions met); opening PR; runner queued for full suite.
- 11:50:52 [RUNNER] kshana PR 34 @6fae76b4 native verification started.
- 11:50:19 Runner: PR 6 GREEN native (804 passed) + Mac #52; PR 7 GREEN native (804 passed on merge with db2210a) and notebook re-executed from the PyPI 0.32.0 wheel (8/8 cells, outputs match except timings); Mac CI main #55 + PR 7 #56 queued on the single runner. Act image amd64 build NOT PROVEN in cloud (TLS proxy + Docker Hub 429).
- 11:48:57 W2 report reviewed; rows approved, PNG glyph edit rejected, unpaused with conditions.
- 11:45:34 [W1] PR 34 → 6fae76b (execute grammar = public API doc, budget in bytes, acq_coherent comment); waiting on CI; idle.
- 11:45:24 Check-in: W1 idle (PR 34 awaiting CI); W3 pushed 4c7b6e2 to PR 7 (claim fixed, main merged), tests running; W2 paused — built 2 rows (scene geometry Validated, acquisition Modelled) + 117-file site/README refresh without the requested proposal; asked for pre-registration evidence and how web/ was regenerated; W4 idle.
- 11:44:56 [W3] PR 7 local gates green: 802 passed, 0 failed, 9 ignored (merge 0fbf7c5); head 4c7b6e2.
- 11:44:17 W1 opened kshana PR 34 (MCP IQ tools); design reviewed + approved; runner queued.
- 11:44:13 [W3] pushed partners/iq-deliverables 4c7b6e2 (merge main =0.32.0 + claim fix); fmt+clippy OK; tests running.
- 11:43:37 [W1] PR 34 (kshana, feat/mcp-iq @ cff687f) open: 6 MCP IQ tools; local gates green; awaiting design review + CI.
- 11:42:42 RECOVERY.md added. Branch deletion approved by founder; proxy 403 → handed back.
- 11:40:06 LIVE.md + RECOVERY.md created on `ws/coordination`.
- 11:38:36 PR 6 merged → main db2210a (Mac CI check success, run 37614594723).
- 11:33 W2 redirected (existing gps-sdr-sim row); W3 claims checked against src/verification.rs.
- 11:32 W3 opened PR 7.
- 11:31:29 PR 5 merged (engine `=0.32.0`), Mac CI green on c99d0e7.
- 11:30 W4 opened PR 6.
- 11:18 Mac CI red on PR 5 (campaign_watch 1–2 ulp) → fixed c3c20c1.
- 11:15 Runner + W1–W4 sessions created.
- 11:14 PR 5 opened (engine 0.32.0 bump).
- 10:39 Coordinator session started on the Team account; state recovered from git.
