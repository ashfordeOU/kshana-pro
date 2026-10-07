Updated: 2026-10-07T15:56:50Z · Coordinator: session_015EnPpasos1W6PQWi7rHJxu

# Kshana — LIVE status (single source of what is happening now)

Rules: see `docs/RECOVERY.md`. If `Updated` is >30 min old while anything below is
running, assume the coordinator is gone and take over from this file.

## Goal
**Top priority (founder, 14:15):** make Kshana deliver everything the prospective receiver-test partner needs or might need, and make it the strongest open tool in its class. Requirements: kshana-pro `commercial/partners/RECEIVER-TEST-PARTNER-REQUIREMENTS.md` (R1–R9, name-free). Gap analysis → W3. Binding line: no jammer/spoofer RF synthesis, no transmit.

Ship kshana **0.33.0** (public) and bring Pro onto it, with every surface current:
1. PR 34 MCP IQ tools + PR 35 IQ ledger rows merged (CI green).
2. `iq_track` default changed (founder decision 12:43; W1 evidence-backed proposal first).
3. Site + changelogs current (kshana.dev changelog page stops at 0.31.0; Pro CHANGELOG stops at 0.2.0;
   deep pages carry old provenance stamps) — W4 audit → generator-based refresh + drift guard.
4. 0.33.0 release bump per docs/RELEASING.md (coordinator), tag → publish → parity → site.
5. kshana-pro pin → `=0.33.0`, papers regenerated, Pro CHANGELOG entry.
6. Issue #36 (macOS-local oracle failures) filed 12:44; needs a local Mac run to capture test names.
No external deadline set.

## Repo heads (from `git ls-remote`, 2026-10-07T11:40:06Z)
| Repo | Branch | Sha | State |
|---|---|---|---|
| kshana-pro | main | 3d77978 | engine `=0.32.0`; PRs 5–10 merged; CHANGELOG current; handover §4 → root site-next/ |
| kshana-pro | partners/iq-deliverables | 4c7b6e2 | PR 7 MERGED → 78ab489 |
| kshana-pro | claude/pro-030 | d94b280 | stale, superseded by PR 5; delete approved, blocked by proxy (founder deletes) |
| kshana | main | 9925c10 | PRs 35, 37 merged; 0.32.0 released |
| kshana | feat/mcp-iq | eba7d10 | PR 34: CI #517 green on a477eec; main merged (CHANGELOG resolved); new CI running (~17:10) |
| kshana | feat/iq-track-default | b41a61d | PR 38: D4 doc fix (behaviour unchanged, per-signal test); CI running |
| kshana | chore/version-sync-coverage | bd05a27 | PR 37 MERGED → main 9925c10 (CI #518 green, runner confirmed clean merge) |
| kshana | feat/iq-validation-ledger | bb7edf8 | PR 35 MERGED → 58957f8 (CI #516 14/14 green; native 3823 passed) |

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
| session_01U47Y1DtpBY9SupJum3iE6Q | W1 PR 34 + PR 38 (CI) + 0.34.0 track engine (B3.1/B3.2/B3.3/B6.1) | feat/mcp-iq @ eba7d10; feat/iq-track-default @ b41a61d; feat/034-track-engine @ d1a575a | PR 34/38 CI pending (unverified). 0.34 lane: loop-design TOML + epoch record + lock state machine proposal sent (docs/design/LOOP-DESIGN-TOML.md @ d1a575a); waiting for approval before implementing. Owed: PYTHON_API.md iq_track line after #39 |
| session_01H3zn1aikR8DL8xLVyNASRf | W2 PR 35 (ledger) + 0.33.0 release-bump owner + version-sync coverage guard | release/0.33.0, chore/version-sync-coverage | PR 35 waiting CI; prep assigned 12:50 |
| session_01BDhQakgc2eHdqnQoRfx2s9 | W3 Pro lab campaign report (L1–L6 on synthetic fixtures) | pro/lab-campaign-report-plan | plan d067277 approved 15:43; PR 14 (export pack DRAFT) awaiting founder read |
| session_01LStocVmSb3UAXorqXZT3BQ | W5 IQ defect fixes D1/D2/D3/D5/D7 for 0.33.0 (public) | fix/iq-d* | plan approved 14:52; D3 first (row recorded by coordinator) |
| session_01QU3dhUo1RBWW24nEVVzxst | W6 engine graphics onto Observatory | feat/observatory-graphics | plan approved 15:27 (palette.rs single source, palette_sync guard, colour-normalised equality evidence, dark-only charts, report light/dark, 3 orphan figure generators) |
| session_011PWAHJi1S2BLjLeD5uidXf | W7 0.34 campaign runner + scoring + F1 analytic J/S reference (public) | feat/034-campaign | spawned 15:24; schema design owed |
| session_01PwGcK67ty6g2wP1Kwx4S8k | W8 0.34 lab formats (b) → IQ monitors (a) → C/N0 profiles (c) | feat/034-formats | plan approved 15:28; (b) started; (c) waits for W5 scene PRs |
| session_01YCEg68jFdLERESsPcMmyMz | W4 site source (pro) | fix/studio-stages-test @ cf5fe30; fix/site-capability-wording @ c754521 | PR 15 (Studio selfcheck LFS preflight) and PR 16 (accurate RF/receiver wording, matches public PR 41) open; idle |

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
- 2026-10-07 No partner recordings, results or anti-jam/anti-spoof performance numbers enter either repo until counsel answers N6 (kshana-pro PR 14). Tooling is built and tested on synthetic data only.
- 2026-10-07 EXPORT CONTROL (founder 15:31): verified that public `spectrum` [iq] has synthesised jammer IQ snapshots (noise/CW/chirp, SigMF, file only) since 0.29.0; spoof_capture builds a spoofer composite in memory; a software receiver has existed since ≤0.27.2. The export pack's "no receiver/signal generator/jammer/spoofer" claim was inaccurate. Decision: correct the record everywhere, keep the features, counsel review of the corrected pack; 0.33.0 proceeds. W3 drafts the pack correction (DRAFT for counsel); W2 corrects public wording. New F3/F4/F5 capabilities stay held pending counsel.
- 2026-10-07 Loop-design schema kshana.loop-design/1 approved (W1, feat/034-track-engine d1a575a); epoch stream kshana.track-epoch/1 (CSV/JSONL/binary); reacquire default false. W6 graphics: domains Interference→coral, Spoofing→magenta, Orbit→cyan, Integrity→lime, Navigation→amber, Timing→#1F55F0/#6A98FF; charts dark-only; paper/crossover untouched.
- 2026-10-07 Founder: start 0.34.0 Lab replay now in parallel. Interference: F1 (analytic SSC J/S reference, MODELLED) and F2 (C/N0 profiles in own scenes) APPROVED and building. F3/F4/F5 (spoofer-shaped scenes, jammer waveforms, RF/transmitter export) ticked by founder but HELD by coordinator pending counsel: kshana-pro legal/EXPORT-CONTROL-ASSESSMENT.md classes adversary waveforms as potentially controlled (L2, Pro-only, kept out of git), and the public engine's uncontrolled status rests on it containing no jammer/spoofer; F5 also enables over-the-air use.
- 2026-10-07 D4 resolved as docs-only: auto rule counts the FULL tiered acquisition period (4 for untiered 1 ms codes, 1 for ≥4 ms); primary-period would mean 40–400 ms coherent. W5 decisions: D1 implement front end in track/sweep; D2 add SignalCode data-symbol getter; D3 integer scaling σ=FS/4, no flag; D5 Q CW/narrowband 1.0, broadband 2.0 (physics-correct, all moved outputs listed, regenerated by tools, stop if a pre-registered bar is touched); D7 centre-offset mixing.
- 2026-10-07 Site theme target = the current live site theme (kshana.dev home page, expected Observatory); everything else (old sections, Studio drift, report exports, og-card, README/docs screenshots) is brought onto it.
- 2026-10-07 W2 rows approved: "GNSS IQ scene signal geometry vs gps-sdr-sim" ExternalDataset/VALIDATED (pre-registered 7777da5, pushed 11:15:27Z before any run; 11 ch, pseudorange 1.6e-3 m vs bar 0.05 m); "GNSS IQ acquisition on gps-sdr-sim I/Q" MODELLED (bar B2 failed: 21 unsimulated PRNs cross the Gaussian threshold). Guard-forced count edits to README/web approved; hand glyph-edit of validation-provenance.png rejected (re-render or revert); unforced README image refresh split into a droppable commit.

## Open for the founder
1. Delete stale branches — founder approved 11:40; cloud git proxy refused (HTTP 403 on ref
   delete, pushes OK) → founder to delete from GitHub UI or a local clone: `claude/pro-030`
   (d94b280), `chore/pro-hygiene` (b398677, merged), `handover/kickoff` (73d5451),
   `handover/post-0.32` (a96bf49), `handover/where-is-what` (31f6e22),
   `salvage/local-artifacts` (e493d8c), `partners/baseband-brief` (2ad7b6b, merged). All verified
   merged or content-on-main at 11:41.
2. DONE 12:44 — macOS-local issue filed: kshana #36.
3. Optional full site re-render at 0.32.0 (§7-D1)?
4. DECIDED 12:43 (founder: change it) — was: Public engine: `iq_track` default `acq_coherent=1` (~667 Hz Doppler bins) false-locked 2/3
   channels ~500 Hz off in the W3 notebook; change the default in 0.33.0? (behaviour change)

## Next
**0.33.0 release checklist** also: (g) `git lfs install && git lfs pull --include='site-next/**'` before the re-port (Studio recordings + wasm are LFS; kshana-pro PR 15).
**0.33.0 release checklist** also: (f) old-theme sections + old-theme README/docs screenshots fixed (W4 audit → site-source PR + regeneration); target theme = the CURRENT live site theme (home page; expected Observatory) — founder 14:49. Ashforde gold/Garamond is not the target.
**0.33.0 release checklist (from the rehearsal, kshana-pro PR 11):** (a) docs prose bumps + README image regeneration IN the bump commit before recordings (re-run quoted figures); (b) placements.toml rows for 11 unplaced features + PR 34's 6 MCP IQ tools (coordinator reviews); (c) Studio channels.json version bump (8 strings); (d) eyeball nav: desktop / → /editions, /capabilities #constellations, #campaigns; (e) flip KSHANA_VERSION_SYNC_SITE default to strict.
- Merge PR 7 when Mac CI green + runner VERIFY green.
- Review W2 row proposal; review W1 MCP design; merge their public PRs after CI (~2.5 h `check`).
- Fold the 0.32.0 status into `handover/post-0.32/README.md` §9 once PR 7 lands.

## Log (newest first, UTC)
- 15:57:04 Merged kshana PR 37 (squash → main 9925c10; CI #518 run 37624082630 green). Open public PRs now 34, 38, 39, 40, 41, 42 — may need main merged for CHANGELOG [Unreleased]. Killed 8 duplicate heartbeat processes (one left).
- 15:46:37 Open PRs: kshana 34, 37, 38, 39, 40 (D3), 41 (accuracy), 42 (LFS guard); kshana-pro 14 (export pack DRAFT, awaiting founder read), 15 (Studio selfcheck LFS preflight), 16 (site capability wording). W4 PR 16 opened; W2 PR 42 opened.
- 15:44:31 W2 found a guard gap: Git LFS pointer files copied by the site build would pass site.test.mjs (manifest SHAs come from the copy). W2 → public PR chore/lfs-pointer-guard (site.test.mjs + port_site.py refuse pointers).
- 15:44:01 W2 opened kshana PR 41 (docs-only capability accuracy: IQ-layer scope, spectrum jammer IQ snapshot since 0.29, in-memory spoof composite, software receiver since 0.20.0). Follow-ups: W4 → kshana-pro site text ("not a GNSS receiver" etc.); W1 → PR 34 MCP wording scope. W6 decisions: timing #1C4FE0; 5 charts gain bg; domain-coverage-map data refresh in its own commit.
- 15:42:25 W4 PR 15: Studio selfcheck failure root-caused to missing Git LFS objects (not code); preflight + RE-PORT step. W3 Pro lab-report plan approved (re-renders engine cells, never recomputes; data-class banner; export-control notice slot pending N6); building L1–L6.
- 15:41:51 [W3] lab campaign report plan pushed: pro/lab-campaign-report-plan d067277.
- 15:40:03 Merged kshana-pro PR 13 (ae783d7, site + Studio onto Observatory). W3 opened kshana-pro PR 14 (export-control pack correction, DRAFT for counsel; N6: analysing partner jammer/spoofer recordings touches the "no anti-jam/anti-spoof performance data in any repo" gate → counsel before partner-data work). W5 opened kshana PR 40 (D3 integer scaling). W1 design API at e7453d8. W7 design approved (a: Cn0Profile SceneChannel owned by W8, now unblocked; b: rebase on track-engine; c: bars in campaign file + per-recording override; d: max_cells ok; e: reacq from offset and from loss). W3 → Pro campaign report plan.
- 15:39:42 [W3] PR 14 updated (3da57fd): F4 line + founder decision recorded.
- 15:38:52 W3 found existing jammer-IQ synthesis (spectrum [iq], since 0.29) and an in-memory spoofer composite; coordinator verified in src/spectrum.rs + docs/SPECTRUM.md. Founder chose: correct the record, keep, counsel. W2 → public docs wording PR; W3 → pack draft.
- 15:29:45 [W3] opened kshana-pro PR 14 (legal/classification-pack-iq 86a3209): export pack factual correction, DRAFT for counsel.
- 15:26:42 W8 plan approved (formats: ci4/cu8/ci12/2-bit-per-byte, multi-stream, sidecars, .sdrx, 2 GB #[ignore] + 200 MB CI test; monitors: power/AGC, C/N0 CUSUM, SQM, lock, spectral/kurtosis/pulse, each vs closed form, MODELLED rows). W1 asked to add extra_taps_chips (default empty, byte-identical) for SQM.
- 15:26:01 Approved W6 graphics plan, W1 loop-design schema (forwarded to W7). W4 opened kshana-pro PR 13 (site + Studio onto Observatory).
- 15:25:21 [W1] 0.34 track-engine: schema proposal pushed (feat/034-track-engine @ d1a575a) and sent; awaiting approval.
- 15:24:45 0.34 lanes started: W1 track engine, W7 campaign+scoring (spawned), W8 monitors/formats/C/N0 profiles (spawned); W3 → export-control pack correction (it says the engine has no receiver/no signal generator, stale since 0.31) + Pro campaign report.
- 15:05:11 Merged kshana-pro PR 11 (c9c5f7b, runbook fixes + placements) and PR 12 (f6321fa, partner requirements + gap analysis). W3 gap analysis: R8 scoring GAP, R3–R7/R9 partial; defects D1–D7; founder decisions F1–F5 pending; plan 0.34 Lab replay / 0.35 Breadth / 0.36 Calibrated. W5 spawned (defects). W4 theme audit: 10 OLD + 2 MIXED of 52 site sections, Studio OLD, 19/32 README images OLD, reports + all engine charts OLD, og-card current → W4 site+Studio PR approved; W6 spawned for engine graphics. PR 39 (docs bindings) open. W1 pushed b41a61d (D4 docs).
- 15:04:00 [W1] PR 38 → b41a61d (D4: full-period rule documented + per-signal test); gates + stubtest green.
- 14:43:37 [W3] opened kshana-pro PR 12 (partners/receiver-test-gap 4f77fe0), footer stripped.
- 14:42:24 Founder: theme target = current live site theme (Observatory if that's it). Passed to W4.
- 14:42:09 [W3] gap analysis pushed: partners/receiver-test-gap 4f77fe0 (analysis only; awaiting review).
- 14:41:43 Founder: old theme still on many site sections + old-theme README screenshots → W4 theme audit. Two salvage audits conflict on the target theme (Observatory vs Ashforde gold) → asked founder.
- 14:40:38 W1 merged main into PR 34 (eba7d10) and 34 into PR 38 (d542efa); CHANGELOG conflicts resolved (both entries kept); local gates OK. New CI ~2.5 h.
- 14:40:10 [W1] main→34 (eba7d10), 34→38 (d542efa) pushed; local gates green.
- 14:39:35 W2: kshana-pro PR 11 now includes placements for the 11 unplaced features (UNPLACED=0). Found a real 0.32.0 docs gap: PYTHON_API.md Surface and npm usage don't list the 8 iq_*/receiver_trust bindings → approved public docs PR (docs/iq-bindings-surface).
- 14:38:20 W2 rehearsal ended; kshana-pro PR 11 (runbook fixes F1–F11 + check_nav/selfcheck code fixes) opened. Nav like-for-like: main 4 BAD, rehearsal 17 BAD (mostly timing noise; 3 recurring items to eyeball).
- 14:37 [RUNNER] kshana PR 34 @a477eec GREEN CI #517; PR 34 + 38 conflict with main 58957f8 in CHANGELOG.md only; PR 37 merges clean (its check/coverage still running).
- 14:29:33 kshana PR 35 squash-merged → main 58957f8. W1: after PR 34 CI #517 finishes, merge main into 34 (CHANGELOG), then 34 into 38.
- 14:28:15 [RUNNER] kshana PR 35 @bb7edf87: GREEN CI #516 (37617204859), 14/14 jobs incl. check, coverage, 3-OS reproducibility.
- 14:24 [RUNNER] kshana PR 35 @bb7edf87: GREEN native full, test --all 3823 passed / 0 failed / 100 ignored (358 suites), repeatability 3x2751 identical, reproducible/doc-coverage/site.test/doc_sync/clippy 0; CI #516 (37617204859) running.
- 14:23:56 W2 re-port REHEARSAL GREEN end to end at 0.32.0 (public bd05a276 + local scratch commit; pro d7b2313): strict version-sync 0 WARN; port 649 files; diff vs web/ 400 files. 13 manual fixes (F1–F13: binaryen via proxy, Celeste scenario routing, channels.json hand-kept, selfcheck reporter, areas step, capabilities mirror, 8th recorder gen_footprint, check_features order, Pro public text symlink, rule-17 docs tokens + images in the bump commit, check_nav env, 11 unplaced features, wasm path bytes) → kshana-pro chore/re-port-fixes. Nav compare running (main baseline 4 BAD). W2's freed 9 GB was its own target/debug only.
- 14:23:17 Founder supplied partner correspondence (kept outside git). Name audit: public kshana history commit 7afdf69 (reachable from main) contains a full personal name, removed from the tree by 76cc62a; no current file in any branch has it. kshana-pro: a first name in this file's log (coordinator's own line) removed in 07242e7. Requirements doc committed (26add7a, branch claude/pensive-cannon-ogxhr4). W3 → gap analysis.
- 14:10:20 [RUNNER] kshana PR 34 @6fae76b4 (superseded): RED, run 37616257744 check, same doc_sync guard; fixed in a477eec (CI #517 pending).
- 14:04:23 [RUNNER] kshana PR 34 @cff687fb (superseded): RED, run 37615528447 check, same doc_sync guard; fixed in a477eec (CI #517 pending). PR 35 native test --all: 251 suites done, 0 failures so far.
- 13:29:58 Check-in: heartbeat dead again (container restart) → restarted. PR 34/35/37/38 long CI jobs (check ~2.5 h) still running; earliest finish ~14:25. Next check-in ~14:40. Planned merge order: 34 → 37 → 35 (CHANGELOG conflict fix by W2) → 38 (W1 merges main).
- 13:20 [RUNNER] kshana PR 38 @d5d67083: GREEN targeted, clippy root/python/mcp 0, iq_cli 11/11, doc_sync 51/51, mcp 33/33, pytest 20/20, stubtest clean; CI #519 (37625615404) running.
- 13:15:07 kshana-pro PR 10 merged (Mac CI 37626378875 green) → 96d64cc; PR 9 merged → 3d77978. Rehearsal runbook fixes → new branch chore/re-port-fixes.
- 13:10:37 W4 opened kshana-pro PR 10 (Pro CHANGELOG PRs 5–8, changelog_tracks_engine_pin guard, RE-PORT.md, README integration-test count 28→29). Merge after Mac CI + W2 rehearsal; then PR 9.
- 13:05:43 [RUNNER] PR 35 @bb7edf87: all gates before test --all are 0 (guards, check-paper, site.test.mjs, doc_sync 51/51, clippy); native full suite running (slow: debug orbit fits), no failures so far. PR 38 quick checks started 13:05. Decision: no extra native full suite for 34/38 (CI check job is authoritative); native only to reproduce a CI red.
- 13:04:56 W1 opened kshana PR 38 (iq_track default); runner queued (incl. full root suite).
- 13:04:34 [W1] PR 38 opened (iq_track auto ≈4 ms hand-off, stacked on 34) @ d5d6708; local gates green incl. stubtest + pytest.
- 13:03:55 W2 rehearsal STARTED: public bd05a276 (main+PR 37) + kshana-pro d7b2313 (RE-PORT.md), scratch worktrees; kshana-pro push attached (push_check ok); building wasm-pack 0.13.1 from source; step 1 (release engine, MCP, wasm) + fetch_cs5071a running detached.
- 13:02:31 W2 release build works (cold 5m51s, kshana --version 0.32.0). W2 → attach kshana-pro push + full site re-port REHEARSAL now (scratch, no web/ commits) to make release day a pure re-run.
- 12:56:22 W4 toolchain proof PARTIAL: node/marked/hljs, Python build.py (to its gates), Playwright+Chromium (executablePath /opt/pw-browsers/chromium-1194/...) work; release `kshana` build DENIED by classifier. Findings: re-port needs server.json at version first; all 7 recorders + Studio recordings must be re-run from the bump commit (recorded at pre-squash bfed2d8); check_nav.mjs needs a Linux path patch; re-record rewrites tracked kshana-pro files → companion pro PR. Passed to W2. RE-PORT.md on chore/pro-changelog d7b2313. Coordinator opened kshana-pro PR 9 (handover §4 → root site-next/).
- 12:55 [RUNNER] kshana PR 37 @bd05a276: GREEN native, version-sync warn exit 0 (4 WARN), strict exit 1 (only the 4 known site items), no-attribution 0, doc_sync 51/51; CI #518 (37624082630) running.
- 12:52:19 W2 opened kshana PR 37 (version-sync coverage; warn exit 0 / strict exit 1 on 67 chips, 67 footers, llms.txt, changelog.html).
- 12:50:40 W2 version-sync plan approved (strict: server.json + README version tokens → 0.32.0; warn-only site checks behind KSHANA_VERSION_SYNC_SITE, strict in the release PR). W4 pushed chore/pro-changelog dbc2802 (Pro CHANGELOG PRs 5–8 + changelog_tracks_engine_pin guard, negative check bites); now proving the site toolchain.
- 12:49:07 W1 iq_track proposal approved (auto ≈4 ms coherent).
- 12:48:49 [W1] iq_track default sweep done (evidence feat/iq-track-default @ f47054a); proposal sent, awaiting approval.
- 12:48:35 W4 audit approved (a–e); W4 → Pro CHANGELOG + guard + toolchain proof/runbook; W2 → version-sync coverage PR + owns release/0.33.0. W1 sweeping iq_track candidates (acq_coherent 1/2/4, finer doppler_step).
- 12:46:59 [W1] Started iq_track default task: branch feat/iq-track-default; false-lock sweep running.
- 12:45:33 Founder: file macOS issue (→ kshana #36), change iq_track default (→ W1), make site + changelogs current incl. all releases/tooling (→ W4 audit; 0.33.0 release planned).
- 12:40 [RUNNER] kshana PR 34 @a477eec: GREEN targeted, doc_sync 51/51, mcp 33/33, fmt/clippy 0, guard negative test bites; CI #517 (37618278079) running; full native test --all pending (after PR 35).
- 12:28:10 Check-in: PR 34 @a477eec run 37618278079 — all 18 non-long jobs green (deny, mcp-check, msrv, package, wasm, python, 3× reproducibility, CodeQL); check + coverage running. PR 35 @bb7edf87 run 37617204859 — same, check + coverage running. Next check-in ~13:28.
- 12:24:57 Heartbeat found dead since the ~12:00 restart (false 'running' from pgrep self-match); restarted; RECOVERY lesson added.
- 12:23:40 [RUNNER] kshana PR 34 @6fae76b4: RED, run 37616257744 coverage, same doc_sync guard (fixed in a477eec, re-verify pending).
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
