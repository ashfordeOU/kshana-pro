Updated: 2026-10-07T11:42:42Z · Coordinator: session_015EnPpasos1W6PQWi7rHJxu

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
| kshana-pro | main | db2210a | engine `=0.32.0`; PR 5 + PR 6 merged; Mac CI on main: running (unverified) |
| kshana-pro | partners/iq-deliverables | dbc8c81 | PR 7 open (W3) |
| kshana-pro | claude/pro-030 | d94b280 | stale, superseded by PR 5; delete approved, blocked by proxy (founder deletes) |
| kshana | main | f570219 | v0.32.0 released; CI green (run 37283754192) |
| kshana | feat/mcp-iq | cff687f | W1 WIP, no PR yet |
| kshana | feat/iq-validation-ledger | ef75a49 | W2 WIP, no PR yet |

## Running now
| What | Sha | Where to look |
|---|---|---|
| kshana-pro CI on main push | db2210a | Actions › CI (self-hosted `macmini-kshana-pro`) |
| Runner: amd64 build of act image, then VERIFY PR 7 | db2210a / dbc8c81 | runner session transcript |

## Sessions (environment AshfordeKSH)
| Id | Lane | Branch | State |
|---|---|---|---|
| session_015EnPpasos1W6PQWi7rHJxu | Coordinator | ws/coordination | active |
| session_012dpyTZ3bYvwaTG67ckWjpU | Runner (verify/watch only, never writes code) | — | active; PR 5 verified GREEN (Linux 802 passed + Mac run 37614073848) |
| session_01U47Y1DtpBY9SupJum3iE6Q | W1 §7-B1 IQ over MCP (public) | feat/mcp-iq @ cff687f | PR 34 open (public), footer stripped; local gates green (MCP `cargo test --locked` 33 passed; root iq_cli 10 passed; fmt; clippy root+MCP+python; no-attribution; doc-coverage 985/985; version-sync); cargo deny not run (not installed); public CI pending (unverified). Next: await coordinator review of new pub API `iq::cli::execute` + work-dir/budget contract |
| session_01H3zn1aikR8DL8xLVyNASRf | W2 §7-B2 IQ ledger rows (public) | feat/iq-validation-ledger | redirected 11:33 (existing gps-sdr-sim row stays MODELLED); row list owed |
| session_01BDhQakgc2eHdqnQoRfx2s9 | W3 §7-C1/C2 partner summary + notebook (pro) | partners/iq-deliverables | PR 7 open; fixing one claim + merging main |
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
