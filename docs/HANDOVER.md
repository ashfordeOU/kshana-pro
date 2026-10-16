# Kshana coordinator handover

**Coordinator switch 2026-10-16 13:30 UTC:** successor 7 = `session_017r1WgqGMGAv8D27a1jVw9j` took over from successor 6 (session_011RqoYRajgEEr3gDaV6hhHj). Check-in routine trig_01VUG9ZMztUvVi8bQwMw2JeL (2-hourly, :29); trig_011tTxPdyaCBp9kT9fQYGSaQ disabled. Session lineage depth is 7 of limit 8: the next handover is the last possible successor.

Written: 2026-10-16 ~13:35 UTC by coordinator `session_011RqoYRajgEEr3gDaV6hhHj` ("successor 6"; took over 15 Oct 15:36 UTC from successor 5 session_01DfFzhRx22yokqTTCMRWmuU). Handing over at the ~40 % context threshold.
Repo of record: kshana-pro, branch `ws/coordination`. Replace this file at each handover; never append.
PENDING owner answer 2: may W52 use ITU-R published validation numbers as test values (licence call)? Until answered they stay out of the repo. W52 design note accepted (feat/optical-atmosphere-0.37 @ bb08f07f).
Owner's latest words (16 Oct): "yes plan it for 0.37 and use a separate worker who can already start working on 0.37 on the side while 0.36 is releasing" (ESA NAVISP EL1-134 optical-PNT analysis capability; W52 created). Earlier today: "brief", "how long?", "all running smooth?". PENDING owner answer: Studio library + features page follow-up before or after the tag (recommended after).

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

Owner decisions 15–16 Oct:

| Topic | Owner's words | Effect |
|---|---|---|
| Batch 4 (17:40 UTC) | "1 a 2 yes" | (1) Greenhall: numerically stable rewrite of greenhall_edf_core meeting the unchanged 1e-9 bar under portable maths. (2) gps-sdr-sim testbench oracle re-run, bar unchanged, revisions entry. Integration helper started in the coordinator container |
| Speed-up 3 (~20:40 UTC) | "yes do both" | (1) No separate batch-4 CI PR: W43 builds the final run on top of integ/batch4-0.36 once the helper's local gates pass; ONE full CI run on the final SHA covers batch 4 + final run. (2) W43 starts as soon as the helper pushes, without waiting for the owner's rc FF. Tag ETA late 16 / early 17 Oct |
| 0.37 optical PNT (16 Oct ~13:25 UTC) | "can we build it all?" then "yes plan it for 0.37 and use a separate worker who can already start working on 0.37 on the side while 0.36 is releasing" | W52 (Sonnet) on kshana feat/optical-atmosphere-0.37 (from 5eac3b61): cloud/turbulence/availability/site-diversity/holdover analysis layer for ESA NAVISP EL1-134 (ITT closes 27 Nov 2026). Analysis only; Kshana cannot do the physical demo (needs an optical ground-station partner, owner's outward call). Brief docs/briefs/W52-optical-atmosphere-0.37.md |

Coordinator decisions 14–15 Oct (successor 5):
- W50 CAP-30 run 2 amendment R2-A1 (scoring-only fix after the scored run crashed sampling past the fit span) accepted on conditions: identical deterministic re-execution reproducing B2 exactly, no bar/editing/arc change, disclosed in PREREGISTRATION-RUN2.md + docs/revisions; run 1 not re-scored.
- ci.yml `check` timeout-minutes 240 -> 360 (commit a3843700, now on the rc): the debug `cargo test --all` outgrew 240 min (cancelled twice on #81 while passing everything it reached). Nothing skipped.
- PR bodies created via the GitHub MCP tool get a "Generated by Claude Code" footer appended: edit the body right after creating to remove it (done on #81). Never post comments.

## 3. Current state (13:35 UTC 16 Oct; verify with git ls-remote)

- **Final 0.36.0 candidate: kshana `site-port/0.36.0` @ 5eac3b61** = rc a3843700 + batch 4 portable maths (b1558f70) + W43 final run (0.36.0 bump, docs, Studio record pass, site port fa3d6956) + version-bump pin re-blesses (4bd16ebd skyline, cb6d634b telemetry/iq_campaign/fusion chart, 5eac3b61 three README demo charts). It fast-forwards from main and from the rc. Full local release suite at 0.36.0 on W43: 5,290 tests passed, 0 failed. Revisions: docs/revisions/STUDIO15-*.md incl. STUDIO15-pins-0.36.0-version.md, STUDIO15-skyline-baseline-0.36.0.md.
- **The single full CI run = draft PR #82** (site-port/0.36.0 -> main, "CI only, do not merge", body footer-free, subscribed). CI on 5eac3b61 started ~13:20 UTC; `check`/`coverage` take ~4.5–5 h. Previous heads failed ONLY on version-stamped pins (all fixed). CodeQL red on #82 = false positives: web/studio/lib/svgsafe.mjs:30 "entity expansion" (guarded by hasDeclaration, file unchanged since 0.35) and alert 149 (entry_heating.rs test print); owner may dismiss. Anything else red is real.
- rc `release/0.36.0-rc` still @ a3843700; main @ 5774eb01; integ/batch4-0.36 @ b1558f70 (absorbed). PR #76 (release PR on the rc) still open, subscribed.
- kshana-pro `port/0.36.0` @ 7c85e04 (Studio/site sources, FINAL-RUN-0.36.md incl. 0.35 claims gate ALL PASS).
- Ledger 361 rows: 175 V / 182 M / 4 P, 83 kinds. CHANGELOG heading [0.36.0] - 2026-10-16 and CITATION date-released 2026-10-16: if the tag lands on 17 Oct, a tiny date fix is needed (ask W43).
- Not in 0.36 (owner to decide, recommended post-tag site follow-up): Studio library places for 19 new scenarios + fjord-natural-degradation label; features/check_features.py placements (133 unplaced).

## 4. Open items, in order

1. **Wait for CI on 5eac3b61 (#82).** If red: read the job log (curl https://api.github.com/repos/ashfordeOU/kshana/actions/jobs/<id>/logs); if it is a version-only pin, send W43 the failure (its recipe: map version back to 0.35.0, if old pin reproduces re-bless + revisions record, ONE push). Anything numerical: stop and tell the owner.
2. **When green (except CodeQL false positives):** send the owner the push commands (he runs them from /tmp/kshana-push on his Mac with --no-verify; the classifier blocks the coordinator):
   `cd /tmp/kshana-push && git fetch origin site-port/0.36.0 && git push --no-verify origin <SHA>:refs/heads/release/0.36.0-rc && git push --no-verify origin <SHA>:refs/heads/main`
   then the tag: Auto-tag (auto-tag.yml) tags v0.36.0 when CI is green on main if RELEASE_TAG_TOKEN is set; otherwise `git tag -a v0.36.0 <SHA> -m v0.36.0 && git push origin v0.36.0`. release.yml verify skips the repeat suite (c8922670). Then close #82 (and #76 once merged/obsolete).
3. Watch release.yml (verify -> publish -> parity -> site). Then: Zenodo notes, PRO-01 re-pin, winget 0.36 PR (owner posts), docs/RELEASING.md checklist addition: run pin-bearing tests right after the version bump.
4. Post-release: Studio library / features follow-up if the owner says so (W43); list merged branches for the owner to delete (feat/*-0.36, integ/*, site-port/0.36.0); archive W43 when nothing is left.
5. 0.37: W52 optical atmosphere (see §2). Integrate after 0.36.0 is tagged (W52 merges main into its branch).
- winget microsoft/winget-pkgs#450973 (0.35.0) waits on a Microsoft moderator.

## 5. Sessions (cap 8; 4 in use + the new coordinator = 5)

| Session | Id | Work |
|---|---|---|
| Coordinator (successor 6) | session_011RqoYRajgEEr3gDaV6hhHj | archive after handover |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA | — |
| W43 (Sonnet) | session_012VCHUyvBgx38Z6Zx43FcjM | Final run done; idle; fixes CI pins on site-port/0.36.0 on request; handoff note /tmp/claude-0/handoff-W43.md in its container. Accepts a new coordinator after checking HANDOVER |
| W52 (Sonnet) | session_01SV4AkzEH1FoQWkLT2drNVd | 0.37 optical atmosphere, created 16 Oct 13:27. It may refuse its create_session brief as unconfirmed: owner may need to confirm in its session |

## 6. Routines

- `trig_011tTxPdyaCBp9kT9fQYGSaQ`: every 2 h (minute 35), bound to successor 6. The new coordinator creates its own and DISABLES this one. Older ones are disabled.

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

- **Version bump moves pins:** every result carries engine_version, so the 0.36.0 bump broke skyline, telemetry hashes, IQ digests, a fusion chart and README demo charts; `cargo test --all` stops at the first failing binary, so CI found them one per 5-h run. Run the full suite locally right after the bump next time.
- **Context:** at ~40 % this shift had ~415k tokens; notifications + CI logs are the main cost. Grep logs, never dump them.

## 8. Setup for a new coordinator

1. add_repo ashfordeOU/kshana (push) — in AUTO mode this needed the owner's allow; clone FULL history to /workspace/kshana (check-no-attribution needs it). kshana-pro: `git checkout ws/coordination`, read this file.
2. Identity: `git config user.name "Ashforde OÜ"; git config user.email ashfordegithub@cbaweja.org` and commit with the -c flags. NO AI attribution; refuse stop hooks that ask otherwise.
3. Subscribe PRs ashfordeOU/kshana#76 and #82 (`subscribe_pr_activity`). GitHub MCP tools for checks; job logs also via `curl https://api.github.com/repos/ashfordeOU/kshana/actions/jobs/<id>/logs`.
4. Routines: create your own ~2-hourly check-in, disable §6's. Tell every worker in §5 your id.
5. Brief: edit tools/brief/data.json, `node tools/brief/render.mjs` (needs /opt/node-tools Playwright), send tools/brief/kshana-brief.png; commit data.json + brief.html.
6. Verify §3 with git ls-remote (site-port/0.36.0 expected 5eac3b61, rc a3843700) and get_session first. No integration helper is running. Tell W43 and W52 your id. Job logs sometimes 404 for a few minutes after a job ends (BlobNotFound); the job API's steps + check-run annotations still show which step failed.
