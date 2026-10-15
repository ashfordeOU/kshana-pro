# Kshana coordinator handover

**Coordinator switch 2026-10-15 15:36 UTC:** successor 6 `session_011RqoYRajgEEr3gDaV6hhHj` took over from successor 5 (`session_01DfFzhRx22yokqTTCMRWmuU`). Check-in routine `trig_011tTxPdyaCBp9kT9fQYGSaQ` (every 2 h, minute 35, bound to successor 6); `trig_01SpyoU4onaWDRbEA9ppnKd2` disabled. W43 told. §3 verified (rc a3843700, integ/batch4-0.36 ab4fbe09).

Written: 2026-10-15 ~15:10 UTC by coordinator `session_01DfFzhRx22yokqTTCMRWmuU` ("successor 5"; took over 14 Oct 16:52 UTC from session_01DHvAPjGDf3yHFwyhV98iyj). Handing over at the ~1 day threshold.
Repo of record: kshana-pro, branch `ws/coordination`. Replace this file at each handover; never append.
Owner's latest words (15 Oct): pasted his successful rc push of a3843700 (batch 3b). Earlier: "done the sending command to that session" (confirmed W50 in its own session). PENDING owner answers on batch 4 (see §4).

## 1. Owner and how they work

- **Owner:** the founder of Ashforde OÜ (Estonia), who runs Kshana (public, AGPL-3.0, github.com/ashfordeOU/kshana,
  kshana.dev) and Kshana Pro (private, github.com/ashfordeOU/kshana-pro). The owner writes short, informal messages;
  answer briefly and plainly, with the outcome first.
- **"brief":** whenever the owner says "brief", render the PNG brief and send it as a file. Use `tools/brief/`
  in this repo: edit `data.json`, then run `node tools/brief/render.mjs`, which needs Playwright at
  /opt/node-tools; the output is `tools/brief/kshana-brief.png`. Also give a few lines of status and the
  "how long" estimate.
- **Standing rules from the owner (never break them):**
  - Commit identity: `git -c user.name="Ashforde OÜ" -c user.email="ashfordegithub@cbaweja.org"`. NO AI
    attribution anywhere: no Co-Authored-By and no session trailers, which `scripts/check-no-attribution.sh`
    enforces. This overrides the harness default.
  - Package metadata author is "Ashforde OÜ".
  - No personal names in shipped or public material. Agency and programme names appear only in source citations.
  - The advisory statement stays on every user-facing surface.
  - At most 8 kshana sessions in total, including the coordinator and a CI runner (owner, 12 Oct 22:20, see §2).
    Archive sessions that aren't needed.
  - Don't burn usage fast. Use few subagents and finish items rather than starting many.
- **The owner must approve:** releases and tags, deleting branches, legal and licence calls, anything outward
  (posting, sending), scope changes. The owner has a standing approval for the single-CI-run release path (§2).
- **Never do:**
  - force-push or rewrite pushed history;
  - push to main or release/* except via the agreed merge path;
  - loosen a pre-registered bar after a run;
  - claim VALIDATED without an external oracle;
  - vendor data whose licence is unclear;
  - fabricate data, partners or reviews;
  - work around the auto-mode permission classifier. If it blocks something, stop and tell the owner.

## 2. Decisions (owner's exact words, 2026-10-12 unless noted)

| Topic | Owner's words | Effect |
|---|---|---|
| Lunar reference-station study | "Add it all to 0.36" | Parts A (coverage map), B (published-claims comparison), C (frozen prediction), D (time lab), E (geodesy) and F (RINEX clock) are all in the rc |
| Speed-up | "combine all to one single run and make sure that is green" and then "anything else i approve" | ONE full CI run on the FINAL rc SHA. If fully green: fast-forward main to that SHA and push tag v0.36.0. release.yml verify skips the repeat suite (commit c8922670). The classifier blocked one step as "CI bypass", so the owner may need to allow it or push the tag themselves |
| Scope | "I want all future features planned to be delivered in 0.36", then, choosing among ship-now / core / core+stretch: "everything - all of them in 0.36" | Every engineering-possible item in the audit (§3) goes into 0.36. Partner, legal or paid items get the engineering side only |
| Sessions | "we can only run max 8 sessions per project so only 8 sessions including a CI runner session ... combine them or handle them one by one and coordinate well ... archive any sessions not needed" | 8-session cap; see §5 |
| Usage | "do not burn the usage fast" | Minimal fan-out |
| Handover | "FRESH-SESSION HANDOVER: adopt this as a standing rule for this project." (thresholds: 40 % context, about 1 day, or losing track) | This file; repeat at each threshold |
| LinkedIn / NDA | "linkedin done, liz sent the NDA" | 0.35 LinkedIn video and post are published. The signed mutual NDA was sent to SJAWS (Liz) by the owner |

Coordinator decisions taken under the owner's standing approval (not owner words):
- **Lab-fit Q identifiability amendment:** fit δ_eff with Q stated. The original Q bars are recorded as NOT MET, not removed.
- **S2 air-tide fix:** pre-register first, then re-run the tide-dependent rows.
- **Studio:** the 2 geodesy scenarios are not in the Studio (they need the repo's LLR data).
- **Lunar prediction:** frozen with the orbit elements labelled ILLUSTRATIVE.
- **Bundled default:** accuracy_threshold_m 20 m, ILLUSTRATIVE.

Owner decisions 13 Oct (exact words where given):

| Topic | Owner's words | Effect |
|---|---|---|
| rc merge path | "push yourself and everything as 0.36" | The coordinator pushes merges to release/0.36.0-rc itself. Successor 3 ran in AUTO mode: add_repo (push) needed the owner's allow ("push access allowed"); after that FF pushes to the rc and integ branches worked; a direct fix commit onto release/* was refused by the classifier (route fixes through the batch branch) |
| Old Galileo E1 tables in published 0.31–0.35 | "we havent used for commercial so why care about it?" | Leave published versions as they are. 0.36 removes the tables (WP-00) |
| RTKLIB / gLAB | "Yes, allow" | RTKLIB builds/runs (W36). gLAB download still denied by W36's classifier → owner must allow it IN W36's session (command in §4) |
| Downloads | "ask for downloads permissions i can approve manually" | Workers retry a blocked download ONCE as a plain single command; if denied, report command+URL; coordinator asks the owner to act in that session |
| STUDIO-15 (portable maths, native==wasm byte-identical) | "Do it in 0.36" | W43; bigger than estimated (1,743 call sites/206 files; moves native last bits). Dry run first; pre-register; re-baseline only named tests with docs/revisions entries; STOP if any ledger row/figure/published number moves |
| 0.35 marketing claims | "dont forget we marketed 0.35 as this" | Final-run gate (FINAL-RUN-0.36.md, W43): NMEA-only receiver trust 0–100 with named reductions; crew-training NMEA (jamming, drag-off, time-spoof, replay) + debrief; OSNMA 18 sets / 186,382 tags / 0 failures (CHECKED PASS by W43 from gsc-europa.eu Test_vectors.zip sha256 ef9b9afc…, not committed — terms unclear); signed offline evidence packs; Prometheus/OTel/CEF/LEEF; Signal K + OpenCPN; ADS-B/AIS daily interference map; clause compliance mapping; ≥39 MCP tools; ≥132 VALIDATED; kshana.dev in browser; cargo/pip/npm/brew/scoop/JetBrains. Stop on regression |
| CODE GIM crop in history | not answered | Leave it |
| gLAB download (13 Oct ~19:55) | "glab i approve" | Relayed to W36; if its classifier still refuses, the owner approves in W36's session |
| Paper author name (13 Oct ~19:55) | "Paper author personal name necessary and stays in research papers" | Personal author names stay in research papers (paper/ front matter); the no-personal-names rule applies to other shipped/public material. Question closed |
| Speed-up 2 (13 Oct ~20:05) | "yes do both, but handle the rest properly still" | (1) BATCHED integration: several finished packages per integration branch, full gates once per batch; split on failure. (2) Full suite runs on GitHub CI via a DRAFT PR integ/batchN-0.36 → main (ci.yml only triggers on PRs to main), titled "do not merge", closed after green; locally only fast gates (fmt, clippy, lib, cli_first_run, MCP round_trip, wasm check, check-no-attribution). rc FF to the batch head only when CI is green. Batch 1 = WP-07+WP-12+WP-04+WP-09 on rc 0d9456fb (subagent in coordinator container, started 20:05). Batch 2 = lunar-terrain + wasm-iq + WP-13 so far (delete WP-04's append_figure_tiers copy). Tag ETA ≈ 16 Oct |
| STUDIO-15 scope (13 Oct ~20:10) | "c" (full portable maths in 0.36 under a written re-baseline) | W43 told: revisions record first; the frozen prediction file and hash are NOT overwritten (the portable re-run is stored beside it); paper erratum + revisions for moved paper data; 0.35 byte-identity guards re-based to a named "0.36 portable" baseline (not deleted or ignored); STOP if any VALIDATED verdict flips |
| Oracle re-record + promotions (13 Oct ~21:55) | "1. yes. 2. yes" | (1) lunar_joint_od numpy oracle re-record approved: re-export the fixtures, numpy pinned 2.4.6, bar unchanged, revisions entry (W43). (2) KF-GINS + IONEX B6 rows promoted to VALIDATED (data-gated); relayed to W36. If W36's classifier refuses, the owner types it in W36's session |
| Owner decisions (13 Oct ~23:25) | "1. yes 2. yes 3. b" | (1) lunar_observability numpy oracle re-record (numpy 2.3.5 + scipy 1.18.1 as pre-registered, bar unchanged, revisions entry) → W43. (2) lunar_station finding 15/20 → 20/20 (bar unchanged, revisions entry) → W43. (3) DS-LRO-10 LRO non-grav macro-model IN 0.36 → W41 (~40 h; tag ETA ≈ 17–18 Oct) |

Coordinator decisions today (standing rules for workers):
- A NOT MET bar is pinned by a test that ASSERTS the recorded miss — never `#[ignore]`.
- A pre-registered pass against an external oracle may add a VALIDATED row. Data-gated evidence may be VALIDATED if the data is fetched at test time with a pinned SHA-256 (fail on mismatch), a CI/reproduce path runs it, and the row states the gating. (W36's classifier refused to apply this from a peer message: KF-GINS + IONEX B6 promotions need the OWNER to say it in W36's session.)
- Count updates only via the repo tools / anchored per-phrase edits; never blind substitution. Non-count numbers that collide: "275 epochs", "137 m @ 1-day arc", "grid-resolution floor ~140 m", "135.4 m", "CCSDS 133.0".
- Published numbers that move because of a bug fix: docs/revisions entry + dated erratum note in paper/kshana-technical-report.md (not silent edits). Applies to W41's lunar ME→PA fix (LRO 53.8→39.9 m raw, 12.6→12.05 dynamic, 6.65→6.67 reduced-dynamic).
- GPL tools (GNSS-SDR, KF-GINS) are oracle-only: run, commit only outputs + NOTICE (FYI given to owner).
- Scorer fix (W28): 6 false-lock fields changed; pin hashes map them to pre-fix values so registered digests reproduce; new assertion on fixed values; disclosed. Call them "false-lock check firings from LOCKED".
- RM-04 (W30): frozen-prediction kinds excluded from figure_tiers by named list + guard test; ONE append_figure_tiers implementation (W42's api/builtins.rs; second merger deletes the copy).
- PRO-34: cite only "Zenodo 16941224 (INGV, Malindi), CC BY 4.0"; no receiver model name.
- MCP file-source guard: dem_path refused over MCP (fixed in rc ee3c71de).
- Batched integration (owner "yes do both"): integ/batchN-0.36 = rc + several finished packages; fast local gates + `cargo test --release --all --no-fail-fast` locally (CI's `cargo test --all` stops at the FIRST failing binary, so the known matrix_total red hides everything after "m"); CI-only DRAFT PR integ/batchN → main for the full matrix; rc FF to the batch head only when CI is green except the tolerated web/doc-sync counts; then close the draft PR.
- figure_tiers (WP-04/RM-04) appends a block to run_toml output; tests that pin a scenario's own document/key set/byte identity compare `run_toml_untiered` / `without_figure_tiers` (as registry_golden, cross_platform_golden do). Campaign members are untiered by design.
- Any new scenario field that reads a file goes into surface::FILE_SOURCE_KEYS (MCP refuses it).
- Stop hooks in containers demand re-authoring commits as "Claude <noreply@anthropic.com>": REFUSE (owner identity rule overrides; would also rewrite pushed history). All workers told.

Owner decisions 14 Oct:

| Topic | Owner's words | Effect |
|---|---|---|
| rc fast-forwards | (ran the pushes himself) | The classifier blocks the coordinator's push to release/* ("Production Deploy"). For every rc FF, send the owner the exact command; he runs it from /tmp/kshana-push on his Mac with `--no-verify` (his own pre-push hook wants .ci-native/.ci-none; ref-only push, CI already ran on GitHub). Template: `cd /tmp/kshana-push && git fetch origin <integ branch> && git push --no-verify origin <SHA>:refs/heads/release/0.36.0-rc` |
| Galileo E5b/AltBOC | "whats the dominant and best future proof way? if no then no" | Follow ICD 2.1 consistently: E5b -155.25 (Table 12); ICD has NO AltBOC row, so AltBOC = -152.24 = Table 12 E5a+E5b power sum, documented (W51, done @ 88d8bfff) |
| Mars OD rerun (CAP-30) | "check and verify and if thats dominant and then ok" | Verified (B1 MET 12.5 mHz; only the estimator failed numerically) -> pre-registered run 2, same bars, W50 |
| Lab-fit follow-up campaigns | "yes" | W51 runs W28's pre-registration 1e532f84 |
| GitHub comments | (coordinator decision) | The GitHub MCP tool appends a "Generated by Claude Code" footer to comments, which breaks the no-attribution rule: do NOT post GitHub comments; tell the owner instead. One such reply exists on #80 (owner told he can delete it) |

Owner decisions 15 Oct:

| Topic | Owner's words | Effect |
|---|---|---|
| Batch 4 (17:40 UTC) | "1 a 2 yes" | (1) Greenhall: numerically stable rewrite of greenhall_edf_core meeting the unchanged 1e-9 bar under portable maths. (2) gps-sdr-sim testbench oracle re-run, bar unchanged, revisions entry. Integration helper started in the coordinator container |
| Speed-up 3 (~20:40 UTC) | "yes do both" | (1) No separate batch-4 CI PR: W43 builds the final run on top of integ/batch4-0.36 once the helper's local gates pass; ONE full CI run on the final SHA covers batch 4 + final run. (2) W43 starts as soon as the helper pushes, without waiting for the owner's rc FF. Tag ETA late 16 / early 17 Oct |

Coordinator decisions 14–15 Oct (successor 5):
- W50 CAP-30 run 2 amendment R2-A1 (scoring-only fix after the scored run crashed sampling past the fit span) accepted on conditions: identical deterministic re-execution reproducing B2 exactly, no bar/editing/arc change, disclosed in PREREGISTRATION-RUN2.md + docs/revisions; run 1 not re-scored.
- ci.yml `check` timeout-minutes 240 -> 360 (commit a3843700, now on the rc): the debug `cargo test --all` outgrew 240 min (cancelled twice on #81 while passing everything it reached). Nothing skipped.
- PR bodies created via the GitHub MCP tool get a "Generated by Claude Code" footer appended: edit the body right after creating to remove it (done on #81). Never post comments.

## 3. Current state (15:10 UTC 15 Oct; verify with git ls-remote)

- **rc `release/0.36.0-rc` @ a3843700** = batch 1+2a+2b+3a (6c7c4a12) + 3b (labfit-followup 90f956fb, mars-od-run2 0576185b, regen 36dc5ba9, ci timeout a3843700). Ledger 361: 175 V / 182 M / 4 P, 83 kinds. Draft release PR #76 tracks the rc (subscribed); its only reds are the tolerated counts + CodeQL alert 149 (false positive; owner may dismiss).
- #80 (3a) and #81 (3b) CI PRs closed. No batch CI PR open.
- **Tolerated CI reds** (until W43's site port): test-wasm-bindings (capabilities count), coverage/check stopping at matrix_total_no_stale_count_doc_sync / scenario_count_doc_sync / web_validation_counts_doc_sync. CodeQL alert 149 (src/entry_eo_ascent/entry_heating.rs:1089, test-only f64 print) = false positive. Anything else red is real.
- **Batch 4 (portable maths) — STOPPED, waiting on owner.** Pushed WIP (NOT for CI/rc yet): `integ/batch4-0.36` @ ab4fbe09 = 36dc5ba9 + merge of feat/portable-math-0.36 2140172c (054fa552, 13 conflicts resolved) + ab4fbe09 (457 transcendental call sites in 62 batch-2/3 files routed through PortableFloat, 7 rand_distr -> portable_distr, 3 gen_range -> uniform_index). It lacks a3843700 (merge origin/release/0.36.0-rc into it first). Helper findings:
  - STOP 1: Greenhall EDF VALIDATED row (src/verification.rs ~2657; xval/greenhall-edf/PREREGISTRATION.md Bar 1 rel 1e-9 vs allantools): under portable maths 3/4876 points miss, worst 1.05e-8 (alpha=1,d=2,m=10000,n=100000 unmodified: 4.482098076573492 vs 4.482098105676121). Cause: cancellation in allan::greenhall_edf_core amplifies last-bit ln differences. Bar 5 dump SHA moves 5b2de183…->efdf3a36….
  - STOP 2: gps-sdr-sim testbench oracle VALIDATED row (~580): gnss-ins-maritime NMEA export moved (eb2a52fe…->e00b6d32…); oracle must be re-run (xval/testbench-gpssdrsim/run.sh, external gps-sdr-sim).
  - Covered by owner's STUDIO-15 "c" (do with revisions entries, tooling only): 13 new batch-2/3 scenarios move (acm-link, aocs-slew, aocs-slew-mekf, campaign-optimize-link-budget, contact-schedule, entry-heating-stardust, gnss-ins-aviation, gnss-ins-rail, gps-denied-gravity-nav-egm2008, gravity-map-error-mc, time-transfer-budget, +2: regenerate the list with STUDIO-15's scenario_rows dump); skyline re-bless (KSHANA_BLESS_SKYLINE_PORTABLE=1); iq_campaign digest; gnss_ins_legacy_trajectory_pin truth hashes; lunar_frame_campaign_unobserved_beacon 632.3769172997296->632.3772338224321; cr3bp_families_jpl_oracle recorded miss moved >2% (same 4 misses); jamming_crpa A3 MVDR count 202->194 (still NOT MET; RESULTS.md quotes 202); Studio ledger copy; regenerate artifacts. Frozen lunar prediction files unchanged (checked). No new FILE_SOURCE_KEYS needed.
  - Not yet run on batch 4: clippy, cli_first_run, MCP crate tests, wasm build, attribution.
  - Leftover from 3b: src/verification.rs ~1981 row text says "gravity_sh (from_shadr)" (now gravity_pds) -> fix in batch 4 or final run.

## 4. Open items, in order

1. **Owner answers batch 4** (asked 03:10 and repeated, unanswered as of 15:10 15 Oct):
   - Greenhall: (a) numerically stable rewrite of greenhall_edf_core meeting 1e-9 under portable maths (RECOMMENDED; bar unchanged); (b) keep that one function on host maths as a named documented exception; (c) accept the miss -> row becomes NOT MET finding (175->174 V). Re-registering the bar is not allowed (no loosening after a run).
   - gps-sdr-sim oracle re-run (bar unchanged, revisions entry): yes/no.
   Then: one integration helper (background Agent) finishes integ/batch4-0.36 on top of the rc a3843700 (merge rc in; apply the owner's answer; revisions entries + re-baseline via tooling; full local suite + gates; push) -> draft CI PR "do not merge" integ/batch4-0.36 -> main (strip the footer from the body) -> green except tolerated -> owner push command -> close PR.
2. **W43 final run** (send W43 the final rc SHA after batch 4): version bump, site/studio-backlog-0.36 merge, Studio re-record (incl. sweep-nd-inertial, quantum-cai-coast), web counts/site port (fixes tolerated reds), web/smoke.mjs iq golden exact, 0.35 claims gate (FINAL-RUN-0.36.md on kshana-pro port/0.36.0), docs/PRO.md AltBOC worked example re-run (0.00544 dB -> ~0.0051 dB after Galileo ICD 2.1), the verification.rs gravity_sh label. Also: once counts are fixed `check` runs the WHOLE suite — 360 min may still be tight on slow runners; if it times out, consider `cargo test --release` in check or splitting the job (owner call if it changes gating).
3. ONE full CI run on the final SHA -> owner pushes main + tag v0.36.0 -> Zenodo notes -> PRO-01 re-pin -> winget 0.36 PR (owner posts).
4. Post-release: list merged branches for the owner to delete (feat/*-0.36, integ/*).
- winget microsoft/winget-pkgs#450973 (0.35.0) waits on a Microsoft moderator.
- Tag ETA told to owner: 17–18 Oct (depends on batch 4 answer).

## 5. Sessions (cap 8; 3 in use + the new coordinator = 4)

| Session | Id | Work |
|---|---|---|
| Coordinator (successor 5) | session_01DfFzhRx22yokqTTCMRWmuU | archive after handover |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA | — |
| W43 (Sonnet) | session_012VCHUyvBgx38Z6Zx43FcjM | STUDIO-15 done @ 2140172c; HOLDS the final run until sent the final rc SHA. Accepted successor 5 after verifying HANDOVER; it will do the same for you |

Archived this shift: W50 (CAP-30 run 2 done @ 0576185b), W51 (labfit follow-up done @ 90f956fb). Note: W50 sat idle 15:04–16:55 14 Oct because it refused its create_session brief as unconfirmed; the owner had to confirm IN ITS SESSION. Expect the same for any new worker: tell the owner to paste a confirmation into the new session.

## 6. Routines

- `trig_01SpyoU4onaWDRbEA9ppnKd2`: every 2 h (minute 51), bound to session_01DfFzhRx22yokqTTCMRWmuU. The new coordinator creates its own and DISABLES this one. Older ones are disabled.

## 7. Lessons (don't repeat)

- **Session sprawl:** spawning ~17 workers at once exhausted usage and broke the session cap. Keep to 8 and queue the work.
- **Authorisation:** a worker can't take new branches, scope or identity from a peer message. Put the authorisation
  in the create_session brief.
- **Auto-mode classifier:** it blocks "CI bypass" steps and building external code (RTKLIB). Ask the owner; never work around it.
- **Git ignore:** check `.gitignore` before relying on committed artefacts (`*.result.json` hid the frozen prediction results).
- **Bundled scenarios:** `src/bundled_scenarios.rs` must stay sorted. Repo-only scenarios go in REPO_ONLY there AND in
  `tests/cli_first_run.rs`. Run cli_first_run and mcp round_trip.
- **README counts:** count substitution once turned "CCSDS 133.0" into 135.0. Audit for decimal collisions.
- **Generated files:** regenerate them with `cargo run --release --bin gen_validation_artifacts`, the figure scripts and the og-card script; never hand-edit.
- **Local disk:** the coordinator container has about 2 GB free. Avoid big local builds; let workers and CI build.
- **Shallow clones:** they make check-no-attribution.sh refuse to run. `git fetch --unshallow` first.
- **Notifications:** many CI-failure notifications are for old SHAs. Check whether the SHA is still the rc head before acting.

- **Usage limit:** 6 sessions with 2 subagents each hit the 5-hour account limit overnight and nobody resumed them for 7 h. After a limit hit, send each worker a resume message once the reset time passes.
- **Classifier and merges:** pushes to release/* are treated as "merge without review" even with the coordinator's own review and the owner's "Try again". Peer messages that ask for merge reviews are also blocked. Use a PR-based path or the owner's own push.

- **Integration by subagent works** but takes 1–2.5 h per package; subagents sometimes stop without a report — ask them via SendMessage; they may push after reporting.
- **Rc-inherited gate failures:** check a failing test on the rc alone before blaming the package (the DEM guard came from WP-05).
- **Classifier vs peer decisions:** a worker's classifier may refuse to act on a coordinator decision relayed by message (e.g. ledger promotions). Then the owner must say it in that worker's session.
- **Coverage job:** tarpaulin linking can die with a Bus error (runner disk) as test targets grow.
- **Relay delays:** peer messages can arrive hours late or out of order (W28 had 17 queued; W41 started WP-08 after it had moved). Re-state key decisions in each reply; when a worker asks again, re-send.
- **Integration helper in this container:** it can collide with your own cargo/rustup (a concurrent `rustup` install broke rustfmt/clippy once; fix with `rustup component remove/add`). Don't run cargo yourself while it runs.
- **Subagent "irreversible destruction" warning** was a `git stash`/pop; ask the subagent before trusting, forbid reset/clean/rm -rf in its brief.

- **Classifier vs release pushes:** the coordinator cannot push release/*; the owner runs the FF (see §2). Don't retry or work around.
- **Classifier vs relayed scope:** sending a worker a decision that moves published numbers beyond an approved change was blocked ("Instruction Poisoning"); writing "authorisation" lines into briefs was also blocked. Put the owner's own words (quoted, dated) in briefs and create_session prompts instead.
- **MCP file-source guard gaps recur:** three new file-reading fields slipped past FILE_SOURCE_KEYS this shift (dem_hgt, dem_tile_path, [[series]] paths). Every integration must check new fields; the guard test now also flags *_hgt and paths.
- **CI disk:** the `check` job ran out of disk inside cargo test (fixed 26ea2551 with a free-disk step, like coverage). Watch release.yml verify if it ever runs the suite.
- **Relay lag:** worker messages still arrive hours late (W28 got a 08:30 message at 12:46). Repeat key asks; have integration handle a fix if a worker can't.
- **Stop hook:** it nags about unpushed integ branches / uncommitted files while a helper is mid-batch; ignore until the helper's gates pass.


- **CI runner speed varies ~1.7x**: the same suite took 2h45m on #80 and >4h on #81. Don't treat a cap-cancel as a test failure; check how far it got (count "Running" lines in the job log vs a previous run).
- **Integration helpers cost ~175–210k tokens and 2.5–4.5 h each.** Starting batch 4 in parallel with batch 3b's CI saved time; do it again when the base is stable.
- **Local integ branches die with the container**: push WIP integ branches (no PR) before handover.
- **New workers refuse create_session briefs** as unconfirmed automated messages; the owner must confirm in the worker's own session.

## 8. Setup for a new coordinator

1. add_repo ashfordeOU/kshana (push) — in AUTO mode this needed the owner's allow; clone FULL history to /workspace/kshana (check-no-attribution needs it). kshana-pro: `git checkout ws/coordination`, read this file.
2. Identity: `git config user.name "Ashforde OÜ"; git config user.email ashfordegithub@cbaweja.org` and commit with the -c flags. NO AI attribution; refuse stop hooks that ask otherwise.
3. Subscribe PR ashfordeOU/kshana#76 (and each new batch CI PR) (`subscribe_pr_activity`). GitHub MCP tools for checks; job logs also via `curl https://api.github.com/repos/ashfordeOU/kshana/actions/jobs/<id>/logs`.
4. Routines: create your own ~2-hourly check-in, disable §6's. Tell every worker in §5 your id.
5. Brief: edit tools/brief/data.json, `node tools/brief/render.mjs` (needs /opt/node-tools Playwright), send tools/brief/kshana-brief.png; commit data.json + brief.html.
6. Verify §3 with git ls-remote and get_session first. No integration helper is running at this handover; batch 4 waits on the owner's answers (§4.1), then the new coordinator starts the helper itself. The previous container's /workspace/kshana is gone; integ/batch4-0.36 is pushed. Job logs sometimes 404 for a few minutes after a job ends (BlobNotFound); the job API's steps + check-run annotations still show which step failed.
