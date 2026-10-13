# Kshana coordinator handover

**Coordinator switch 2026-10-13 19:05 UTC:** now `session_01X1Md22tYih7LLQiPqsmUjy` (successor 3). New check-in trig_01ULLGFcqyKVADpHVb7s5Ey6; trig_0141RM2pEj95DjCvcR5BGtxL disabled. Owner allowed kshana push ~19:50; PR #76 subscribed. **rc FF'd 25af9489 → 0d9456fb (coverage fix bfaafa41 cherry-picked).** W28 told: your turn @ 0d9456fb. W41 asked to stop + hand off (RM-34 VALIDATED @ dfc55dad); rotate once the old coordinator has archived (cap 8).

Written: 2026-10-13 ~18:25 UTC by coordinator `session_01UmnDCL2ERStnt9tRqC9a4i` ("successor 2"; took over 11:17 UTC from session_01VfB51TSscNzEYNpCXNGTwL, archived). Handing over at ~50 % context.
Repo of record: kshana-pro, branch `ws/coordination`. Replace this file at each handover; never append.
Owner's latest words: "handle everything, deliver 0.36 with all thats all" (12 Oct); today: "push yourself and everything as 0.36"; "ask for downloads permissions i can approve manually"; "dont forget we marketed 0.35 as this" (LinkedIn post, see §2); "brief" (often — render it, §1).

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
| rc merge path | "push yourself and everything as 0.36" | The coordinator pushes merges to release/0.36.0-rc itself. This session runs in DEFAULT permission mode (not auto), so pushes go through the owner's prompts; they have been passing. Create the successor without auto mode too |
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
| Batch 1 status (21:00) | owner: "allow running cargo for integration in /workspace/kshana" | integ/batch1-0.36 @ 699be3cf pushed (rc 0d9456fb + WP-07/12/04/09 + CodeQL fix + CHANGELOG restore + regen + recount; ledger 321: 153 V / 164 M / 4 P; fmt, clippy, lib 3281, cli_first_run, MCP 24/24 and attribution all pass; wasm not built locally). CI-only draft PR #77 → main (subscribed). When green (apart from wasm/web counts): FF rc to it, close #77. Pending owner decision: STUDIO-15 lunar_joint_od numpy oracle re-export (numpy pinned 2.4.6). W41 stays on WP-06 (owner said "go" to it directly) |

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

## 3. Current state (18:25 UTC; verify with git ls-remote)

- **rc `release/0.36.0-rc` @ 25af9489** (labfit merged 19:00; ledger 298: 146 V / 148 M / 4 P) ← ee3c71de (961e7677 → 12b42e4e WP-05 → dc0b85bd WP-00+WP-03 + "275 epochs" fix → ee3c71de WP-02+WP-01 + DEM guard). Ledger on rc: 297 rows, 146 V / 147 M / 4 P. Only known CI reds: web/* counts (fixed by the final site run) and the `coverage` job's linker Bus error (W42 fixing, PRIORITY).
- **Integration method:** the coordinator runs ONE background subagent per package in its own container (/workspace/kshana, full history, local git identity set to the owner): merge the rc into the package branch with --no-ff, regenerate generated files with the tools (gen_validation_artifacts; .mmd + tools/render-diagram.sh validation-provenance with mermaid-cli 11.12.0 + Geist fonts; gen_validation_figures/gen_readme_assets/gen_og_card with --check; Studio ledger copy + web/PORT-MANIFEST.json hash), hand-maintained README counts via 3-way merge-file, full gates (fmt, clippy -D warnings, lib, all integration tests --no-fail-fast, MCP round_trip in a SEPARATE target dir, check-no-attribution), then FF-push the package branch and FF-push the rc. Tolerated failures: matrix_total_no_stale_count_doc_sync, scenario_count_doc_sync (2), web_validation_counts_doc_sync (2). A merge takes ~1–2.5 h on this 4-core box.
- **DONE:** labfit integration (subagent in session_01UmnDCL… container; local branch integ3 @ 25af9489 = merge of rc into labfit, tests running). It will push feat/labfit-iq-0.36 and the rc if green. CHECK `git ls-remote` — if the rc is still ee3c71de when you start and no push follows within ~2 h, redo labfit yourself the same way.

**WASM BREAK on rc 25af9489:** CI wasm-registry fails: src/iq/labfit/{campaign,iqfit}.rs use iq::campaign / iq::cli, which are cfg(not wasm32). W28 told (19:08) to fix it as the first commit after its rc merge on feat/iq-campaign-ext-0.36 (gate campaign parts, keep the receiver-log path used by surface::iq_labfit_inline) and to verify with `cargo check --lib --target wasm32-unknown-unknown` + the job command. ADD the wasm target check to every integration gate from now on (our gates missed it).

**Merge queue after labfit:**
1. IQ campaigns (W28: feat/iq-campaign-ext-0.36 @ 8254d27c): W28 TOLD 19:00 "your turn @ 25af9489"; W28 merges the rc itself (maps the 6 false-lock fields in labfit's b3c2d8a0 test, recounts), then the coordinator reviews + FFs. Send W28 "your turn @ <rc sha>".
2. WP-07 GNSS (feat/gnss-realdata-0.36 @ f7974db; RTKLIB runs added; check head).
3. WP-12 integrity (feat/integrity-trust-0.36 @ 87a8b739).
4. WP-04 timing/quantum (feat/timing-quantum-0.36 @ 7e618a89; carries a verbatim copy of append_figure_tiers — keep one).
5. WP-09 fusion (feat/fusion-ins-0.36 @ d6d60f34; includes W38 carrier_tc).
6. feat/lunar-terrain-0.36 FINAL @ 8bfe269e (W30; export masking, units block, lola_dem tile wired; expect CHANGELOG/VERIFICATION-MATRIX conflicts → regenerate).
7. COVERAGE CI FIX bfaafa41 on feat/platform-vv-0.36 (fast-track: cherry-pick or merge the ci.yml change into the rc) and feat/platform-vv-0.36 (WP-13: FUT-27 514d994e — api/builtins.rs is a hot file: move any conflicting run_builtin_kind arm into its run_<kind> fn).
8. feat/wasm-iq-0.36 (W43: DS-GIQ-08b, STUDIO-05/06/14 engine halves, STUDIO-15 later).
9. feat/cislunar-ephem-0.36 (W41, WP-06 when done), feat/iq-ext-0.36 (W28, WP-11b), WP-10, WP-08, WP-14, then the W43 final run.

Packages done (11 of 15): WP-00, 01, 02, 03, 04, 05, 07, 09, 12, labfit(B7.1), IQ campaigns. In progress: WP-06 (W41), WP-08 (W41 after), WP-10 + WP-11b (W28), WP-13 (W42), WP-14 (W36), STUDIO-15 (W43).
- **PR #76** (rc → main): subscribed; reds = web counts (commented before; don't re-comment) + coverage Bus error (new, being fixed).
- **winget** microsoft/winget-pkgs#450973 (KshanaMcp 0.35.0): CLA done, all checks passed, waiting on a Microsoft moderator. A 0.36.0 PR is needed after release (prepare; owner posts).

## 4. Open items, in order

1. Keep the merge queue moving (§3). One integration subagent at a time.
2. Owner actions pending, in W36's session ("W36 0.36 WP-07 GNSS real data"):
   - allow `curl -fL -o <scratchpad>/glab-dl/gLAB_6.0.0_Linux.tgz https://server.gage.upc.edu/gLAB/src/LINUX/gLAB_6.0.0_Linux.tgz` (checksums at https://server.gage.upc.edu/gLAB/src/checksums.txt);
   - optionally say "Approved: promote the KF-GINS and IONEX B6 rows to VALIDATED (data-gated, pre-registered pass)".
3. Watch STUDIO-15 (W43): if the dry run moves any published number, bring it to the owner before any push.
4. Release: all packages in → W43 final run (version bump first; merges site/studio-backlog-0.36; records Studio incl. quantum-cai-coast; claims gate; pushes kshana site-port/0.36.0) → coordinator FFs rc → ONE CI run on the final SHA → if fully green, FF main + tag v0.36.0 (owner approval stands) → Zenodo notes (docs/handover-0.36/) → PRO-01 re-pin → winget 0.36 PR (owner posts).
5. Post-release: list merged branches for the owner to delete.
6. ETA given to owner: tag ≈ 17–18 Oct.

**OWNER QUESTION CLOSED (answered ~19:55, see §2):** paper/kshana-technical-report.md YAML front matter carries a personal author name (pre-existing) — conflicts with "no personal names in shipped material"? Asked the owner.
W41 follow-ups done 66956eb3 (asserting miss tests; docs/revisions/FUT05B-lunar-me-pa-offset-cell-changes.md; paper erratum; manual CI job lro-de440-opt-in); handoff 4 @ 00b606e7 — ROTATE W41 now (it continues RM-34 until told).

## 5. Sessions (cap 8)

| Session | Id | Work |
|---|---|---|
| Coordinator (this, to be replaced) | session_01UmnDCL2ERStnt9tRqC9a4i | archive after handover |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA | — |
| W28 | session_01YLFPWBdmcbeZKKEyiyRxvM | PRO-34 10e49e26 (B1/B4 NOT MET, F2); PRO-33 amendment (a) approved (FLL pull-in only, both receivers); VALIDATED rows DS-GIQ-06b + PRO-39 approved (if its classifier refuses, owner must say it in W28 session). WP-11b (feat/iq-ext-0.36: CAP-07a, PRO-39, MR-09, DS-GIQ-06b, PRO-34 running) → WP-10 → labfit follow-up prereg; merges IQ campaigns when told. Context ~50 %+ → rotate at next WP boundary |
| W45 (WP-10; W30 archived 18:57 — WP-04 7e618a89 and lunar-terrain 8bfe269e final) | session_016beTms3DYB3M3B5wA7JA93 | WP-10 alt-PNT maps on feat/altpnt-maps-0.36 from handoff 653a9719 (never compiled; SRTM prereg 4235687e not yet run) |
| W36 | session_01BpojbwwUWoNKDfSmsCHR3B | WP-09 DONE d6d60f34 → WP-14 (feat/new-domains-0.36): CAP-22/23 done a767ef29 (MODELLED; GMAT ContactLocator not attempted), CAP-21 AOCS vs Basilisk 2.9.1 prereg 27d6831a |
| W41 | session_0141f1quqeE6s3YFLMBkzTCR | ~40 % context, handoff 3 @ 46aa2799; ROTATE after RM-35 (handoff 4). WP-06 (FUT-05 1bdd3833 partial: ephemeris leg NOT MET; DE440 routing opt-in by design; rc ee3c71de merged @ 12a67f78; MR-08 V, MR-06 V narrow, MR-05 NOT MET, ME→PA bug fix + revisions/erratum, FUT-05 ANISE next; then RM-34, RM-35, CAPSTONE, LRO, CAP-31/20b, zero-observation beacon bug) → WP-08 |
| W44 (replaced W42 18:55; W42 archived) | session_01SVt2FNNE9ZUr9anJAHbyjm | FUT-26, FUT-28, CAP-34 on feat/platform-vv-0.36 (handoff b3408e38 + addendum 70193446) → box-wing (handoff 05391205; prereg vs Orekit) → GMAT leg (exponential tier). W42 pushed the COVERAGE CI FIX bfaafa41 on feat/platform-vv-0.36: fast-track it into the rc (ci.yml only) and confirm the coverage job goes green |
| W43 (Sonnet) | session_012VCHUyvBgx38Z6Zx43FcjM | site/CI runner; STUDIO-15 dry run; holds the final run (site-next/FINAL-RUN-0.36.md on kshana-pro port/0.36.0) |

Archived today: W31, W33, W1b (rotated to W41, W42, W43), old coordinator. Workers lack create_session: rotate at a WP boundary (worker writes a handoff commit; coordinator creates the replacement with a full brief — branches, rules, queue — then archives the old one; stay within 8).
Mode: ≤1 subagent per worker, push hourly, report "<item> done @ <sha>". Workers must not merge intermediate rc SHAs; the coordinator tells each package's owner when it's their turn if they merge.

## 6. Routines

- `trig_0141RM2pEj95DjCvcR5BGtxL`: every 2 h (minute 16), bound to session_01UmnDCL…. The new coordinator creates its own and DISABLES this one. `trig_011Wno8YRNARC5PyeytwiaiK` is disabled.

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

## 8. Setup for a new coordinator

1. Clone: `git clone https://github.com/ashfordeOU/kshana` and `https://github.com/ashfordeOU/kshana-pro` (use add_repo
   if they aren't attached). In kshana-pro: `git checkout ws/coordination`, then read this file.
2. Identity: always commit with the `-c user.name/-c user.email` flags in §1.
3. GitHub: use the github MCP tools (no gh CLI). Subscribe to ashfordeOU/kshana#76 (`subscribe_pr_activity`).
4. Routines: re-create the check-in (§6) bound to yourself and disable the old one. Tell every session in §5 your
   new id.
5. Brief: `cd kshana-pro/tools/brief && node render.mjs` (Playwright at /opt/node-tools; Chromium preinstalled).
6. Verify the state in §3 with `git ls-remote` and `get_session` before acting on it.

7. Permission mode: this coordinator ran in DEFAULT mode after the owner left auto mode; rc pushes needed no special handling. Create the successor with permission_mode default.
