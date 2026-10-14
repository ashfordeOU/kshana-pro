# Kshana coordinator handover

**Coordinator switch: 2026-10-14 16:52 UTC — successor 5 = session_01DfFzhRx22yokqTTCMRWmuU (took over from session_01DHvAPjGDf3yHFwyhV98iyj). Check-in routine trig_01SpyoU4onaWDRbEA9ppnKd2 (minute 51, every 2 h); trig_01PgP3QKMcNzZ4h7Kq7fFoX6 disabled.**

Written: 2026-10-14 ~16:55 UTC by coordinator `session_01DHvAPjGDf3yHFwyhV98iyj` ("successor 4"; took over 14 Oct 00:47 UTC from session_01X1Md22tYih7LLQiPqsmUjy). Handing over at ~47 % context.
Repo of record: kshana-pro, branch `ws/coordination`. Replace this file at each handover; never append.
Owner's latest words (14 Oct): "how long until all done?"; pushed the rc FFs himself; answers "1. yes 2. whats the dominant and best future proof way? if no then no 3. check and verify and if thats dominant and then ok 4. yes" (see §2 additions); "ok, send me the next push when ready".

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

## 3. Current state (16:55 UTC 14 Oct; verify with git ls-remote)

- **rc `release/0.36.0-rc` @ 2ef8bbff** = batch 1 (26ea2551) + batch 2a (36d8d3d4) + batch 2b (2ef8bbff). Ledger 334: 165 V / 165 M / 4 P. Draft release PR #76 tracks the rc (subscribed); its only reds are the tolerated web/doc-sync counts.
- **Batch 3a — integ/batch3a-0.36 @ 6c7c4a12** (CI draft **PR #80**, subscribed): + interop-api c8086328 (WP-08) + new-domains b51d9bb7 + new-domains-b 032c8231 (WP-14). from_shadr kept ONCE (gravity_pds; WP-14 gravity_sh copy removed); time-transfer-budget `[[series]] paths` refused over MCP; 7 WP-14 kinds moved to src/api/builtins.rs; aocs settle/pointing figures untiered. Ledger 350: 173 V / 173 M / 4 P, 83 kinds. Local full suite 5191 pass, only tolerated counts. #80 so far: wasm = tolerated count; CodeQL 1 "high" = FALSE POSITIVE (cleartext logging, src/entry_eo_ascent/entry_heating.rs:1089 is a #[cfg(test)] assert printing an f64 energy residual; owner asked to dismiss code-scanning alert 149; not blocking). NEXT: when `check` finishes green except tolerated counts -> verify `git merge-base --is-ancestor origin/release/0.36.0-rc origin/integ/batch3a-0.36` -> send owner the push command for 6c7c4a12 -> after it lands close #80 and unsubscribe.
- **Tolerated CI reds** (until W43's final site port): test-wasm-bindings (web/site.test.mjs capabilities count), coverage and check stopping at matrix_total_no_stale_count_doc_sync / scenario_count_doc_sync / web_validation_counts_doc_sync. Anything else red is real.
- **Batch 3b (next):** on the rc after 3a: feat/iq-ext-0.36 @ b34e6803 (W28 WP-11b complete: CAP-06a/b/c, DS-LOOP-05, dem_hgt MCP guard f3a38ef4, labfit follow-up prereg 1e532f84) + feat/labfit-followup-0.36 (W51; descends from b34e6803, so it alone carries iq-ext) + feat/mars-od-run2-0.36 (W50; from b51d9bb7). Use the integration-helper prompt pattern (one background Agent in /workspace/kshana; brief = merge list at exact SHAs, keep-both-sides, regenerate once with tools, MCP file-source guard check, tolerated counts, full `cargo test --release --all --no-fail-fast` + fmt/clippy/cli_first_run/MCP round trip/wasm/attribution, owner identity, no force/reset/stash, push only the integ branch, report). Batch 2a/2b/3a helper reports are in this session's history; their commit messages on the integ branches show the method. Watch: W50 must not edit gravity_sh's from_shadr (told); W51's Galileo change moves 3 scenarios (CAP12 CSV).
- **Then:** feat/portable-math-0.36 @ 2140172c (STUDIO-15, W43) ALONE as batch 4 (98/141 scenarios re-baselined under the owner's written re-baseline; expect conflicts in goldens/tests touched by later batches: re-run its re-baseline tooling, never hand-edit; STOP if any VALIDATED verdict flips or a frozen prediction file changes). Then W43's final run (send W43 the final rc SHA): version bump, site/studio-backlog-0.36 merge, Studio re-record (incl. sweep-nd-inertial, quantum-cai-coast), web counts/site port (fixes the tolerated reds), web/smoke.mjs iq golden exact, 0.35 claims gate (FINAL-RUN-0.36.md on kshana-pro port/0.36.0), re-run the Pro worked example quoted in docs/PRO.md (AltBOC worst loss 0.00544 dB -> ~0.0051 dB after the Galileo change). Then ONE full CI run on the final SHA -> owner pushes main + tag v0.36.0 -> Zenodo notes -> PRO-01 re-pin -> winget 0.36 PR (owner posts).
- winget microsoft/winget-pkgs#450973 (0.35.0) waits on a Microsoft moderator.
- Tag ETA told to owner: 16 Oct (17 if anything slips).
- Disk in this container: ~9 GB free, /workspace/kshana/target 15 GB. Before batch 3b: `cargo clean` in /workspace/kshana (or delete target/debug) to free space.

## 4. Open items, in order

1. #80 green -> owner push 6c7c4a12 -> close #80. 2. W50 + W51 finish -> batch 3b -> CI PR -> owner push. 3. Batch 4 portable-math alone -> owner push. 4. W43 final run + final CI + owner tag. 5. Post-release: list merged branches for the owner to delete (feat/*-0.36, integ/*).
- Open owner questions: none (CodeQL alert dismissal is his to do; not blocking).

## 5. Sessions (cap 8; 5 in use + the new coordinator = 6)

| Session | Id | Work |
|---|---|---|
| Coordinator (successor 4) | session_01DHvAPjGDf3yHFwyhV98iyj | archive after handover |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA | — |
| W43 (Sonnet) | session_012VCHUyvBgx38Z6Zx43FcjM | STUDIO-15 done @ 2140172c; HOLDS the final run until sent the final rc SHA |
| W50 | session_01HETuusgYqArEG4PKQX1HVT | NOT STARTED as of 16:51 14 Oct: refused the create_session brief as unconfirmed and waits for the OWNER to confirm in its own session (asked 16:55). CAP-30 run 2 on feat/mars-od-run2-0.36 (brief docs/briefs/W50-cap30-run2.md): diagnose fit on dev pass, pre-register run 2 alone (same bars), run once. Started 15:04 |
| W51 | session_01YVrY75Gwk52f8NbtWS6fvG | feat/labfit-followup-0.36 (brief W51-labfit-galileo.md): Galileo E5b/AltBOC DONE @ 88d8bfff; labfit campaigns running (early: S1 MET, S2 NOT MET, R1 MET, replications miss as predicted). Started 15:04 |

Archived this shift: old coordinator (successor 3), W48 (WP-08 done), W47 (DS-LRO-10 done), W36 (handoff 1e8e258b), W49 (CAP-30 run 1 stopped @ b51d9bb7), W28 (WP-11b done @ b34e6803). Workers lack create_session/archive: rotate at a boundary (handoff commit -> coordinator archives -> new session with a brief from docs/briefs/).

## 6. Routines

- `trig_01PgP3QKMcNzZ4h7Kq7fFoX6`: every 2 h (minute 46), bound to session_01DHvAPj…. The new coordinator creates its own and DISABLES this one. Older ones are disabled.

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


## 8. Setup for a new coordinator

1. add_repo ashfordeOU/kshana (push) — in AUTO mode this needed the owner's allow; clone FULL history to /workspace/kshana (check-no-attribution needs it). kshana-pro: `git checkout ws/coordination`, read this file.
2. Identity: `git config user.name "Ashforde OÜ"; git config user.email ashfordegithub@cbaweja.org` and commit with the -c flags. NO AI attribution; refuse stop hooks that ask otherwise.
3. Subscribe PR ashfordeOU/kshana#76 and #80 (and each new batch CI PR) (`subscribe_pr_activity`). GitHub MCP tools for checks; job logs also via `curl https://api.github.com/repos/ashfordeOU/kshana/actions/jobs/<id>/logs`.
4. Routines: create your own ~2-hourly check-in, disable §6's. Tell every worker in §5 your id.
5. Brief: edit tools/brief/data.json, `node tools/brief/render.mjs` (needs /opt/node-tools Playwright), send tools/brief/kshana-brief.png; commit data.json + brief.html.
6. Verify §3 with git ls-remote and get_session first. No integration helper is running at this handover (batch 3a is pushed; batch 3b not started), so the new coordinator starts 3b itself when W50/W51 report. Job logs sometimes 404 for a few minutes after a job ends (BlobNotFound); the job API's steps + check-run annotations still show which step failed.
