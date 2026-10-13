# Kshana coordinator handover

**OWNER ANSWERS (13 Oct ~14:05):** merge path: "push yourself and everything as 0.36" (coordinator pushes to release/0.36.0-rc; session left auto mode, so pushes go through the owner's permission prompts). E1 in 0.31–0.35: "we havent used for commercial so why care about it?" → leave the published versions as they are. RTKLIB/gLAB: "Yes, allow". CODE GIM: not answered (default: leave in history).
**OWNER (15:30): STUDIO-15 (portable maths, native==wasm byte-identical, re-records 4 pinned tests, ~40 h) IN 0.36.** W43 does it after STUDIO-14, pre-registered, re-baselines only those 4 with docs/revisions entries, stop if any ledger row/figure moves.
**OWNER (15:32): 0.35 marketing claims must hold in 0.36** (LinkedIn post: NMEA-only receiver trust 0–100 with reasons; crew-training NMEA with jamming/drag-off/time-spoof/replay + debrief; OSNMA 18 sets / 186,382 tags / 0 failures; offline-verifiable evidence packs; Prometheus/OTel/CEF/LEEF; Signal K + OpenCPN; ADS-B/AIS daily interference map; clause compliance mapping; ≥39 MCP tools; ≥132 VALIDATED; kshana.dev in-browser; cargo/pip/npm/brew/scoop/JetBrains). Added as a final-run gate for W43 (PASS/FAIL each, stop on regression).
**rc:** 961e7677 → 12b42e4e (WP-05, 14:07) → dc0b85bd (WP-00+WP-03 merged with regeneration + "275 epochs" README fix; ledger 282: 138 V / 140 M / 4 P; 15:20). WP-02+WP-01 integration running (coordinator subagent). Merge queue (17:15): WP-02+WP-01 (running) → labfit → IQ campaigns (W28 merges) → WP-07 (head after RTKLIB runs) → WP-12 87a8b739 → WP-04 7e618a89 → feat/wasm-iq-0.36 (W43: DS-GIQ-08b, STUDIO-05/06/14 engine halves, later STUDIO-15) → remaining WPs as they finish → W43 final run (merges site/studio-backlog-0.36 first). Earlier: WP-00+WP-03 via merge of rc into feat/time-frames-0.36 with regeneration (a coordinator subagent), then WP-02+WP-01, labfit, WP-07, WP-12. Workers told not to merge intermediate rc SHAs.

**Since the switch (11:30):** W33 WP-01 DE440 tier @ 615ee97c (orbits 0.017 m vs Orekit; Sun/Moon position bars missed, MODELLED); W36 confirmed onto WP-09; W1b holds the final run, needs LUNAR-REFERENCE-STATION.md final and will itself hit the rc block; owner asked once for the merge path (recommended PR per package).

**Coordinator switch:** 2026-10-13 11:17 UTC, session_01UmnDCL2ERStnt9tRqC9a4i took over from session_01VfB51TSscNzEYNpCXNGTwL. Check-in routine trig_0141RM2pEj95DjCvcR5BGtxL (every 2 h, bound to the new coordinator); old trig_011Wno8YRNARC5PyeytwiaiK disabled.

Written: 2026-10-13 ~11:20 UTC by coordinator `session_01VfB51TSscNzEYNpCXNGTwL` (took over 2026-10-12 22:31 UTC from session_015EnPpasos1W6PQWi7rHJxu, now archived).
Repo of record: kshana-pro, branch `ws/coordination` (private; names people and commercial matters). Replace this file at each handover; never append.
Owner's latest words: "handle everything, deliver 0.36 with all thats all" (12 Oct 22:40); "handle everything the best and most dominant way possible" (22:40); "Try again" (13 Oct ~10:35, about the blocked rc merge; the retry was blocked again).

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

## 3. Current state (13 Oct ~11:20 UTC; verify with git ls-remote)

- **rc:** `release/0.36.0-rc` @ `961e7677`, UNCHANGED. NOTHING HAS BEEN MERGED SINCE THE HANDOVER, because of the block below.
- **THE BLOCK:** the auto-mode classifier refuses `git push <sha>:refs/heads/release/0.36.0-rc` as "Merge Without Review" (3 attempts, including after the owner said "Try again"). It also refused a send_message asking W1b for a merge review, and one plain resume message that mentioned re-running figures. Do NOT work around it.
  Options put to the owner, none chosen yet:
  (1) the owner runs the push himself;
  (2) the owner adds a permission rule for the push in this session;
  (3) the coordinator opens one PR per package into release/0.36.0-rc and the owner merges each.
  Option (3) also gives a reviewable record. Recommend it if the owner stays silent.
- **PR #76** (rc → main): subscribed. The reds are only the stale web/ counts (75 vs 76 kinds; 263 vs 274 rows). W1b's final port fixes them. Already commented on; do not re-comment.
- **Packages READY (coordinator-reviewed: pre-registration order, Ashforde identity, no attribution, honest labels):**

| Package | Branch @ sha | Notes |
|---|---|---|
| WP-05 lunar station (LRS-01/02/03/06) | feat/lunar-station-ext-0.36 @ 12b42e4e | FF from rc; 278 rows. Merge FIRST |
| WP-00 docs truth pass | docs/truth-pass-0.36 @ 496625fe (was 8737212a; +E5a C.3/C.4 oracle fetched at test time) | Galileo E1 tables removed (ICD terms: non-commercial redistribution only), user-supplied loader + fetch script; API break noted in CHANGELOG. LRS-05 left out. W31 still to move the E5a fixtures C3/C4 to fetch-at-test-time and report whether E1 files were in the published 0.35 crate/wheels (owner legal question if yes) |
| WP-02 gravity/OD | feat/gravity-od-0.36 @ 31a9908b | 5 VALIDATED; misses recorded MODELLED; RM-09 bars were tightened post-run (allowed; originals must stay recorded) |
| labfit B7.1 | feat/labfit-iq-0.36 @ 37e8f07c | Most bars NOT MET (recorded honestly); estimator fixed in 5b13e3c2, bundled with the code, disclosed |
| WP-07 GNSS | feat/gnss-realdata-0.36 @ f7974db (was d521166; + RTKLIB oracle runs 15:20) | 283 rows; PPP, RTK, SP3 clock, RINEX 4 nav VALIDATED; many data-gated items not run (RTKLIB/gLAB/CDDIS). CODE GIM slice removed (still in git history at 0901cca: owner's call) |
| WP-12 integrity | feat/integrity-trust-0.36 @ 87a8b739 | Reviewed by successor 2: identity clean, pre-registrations precede results. CAP-14 VALIDATED (27/27 vs OpenSSL); CAP-05a/05b/13 MODELLED (cn0 bar missed 0.0231 vs 0.02, stated on the profile); CAP-05c data-gated. Open: UBX vs CSV false-alarm diff; lunar export site not masked, lola_dem.rs not wired. W30 now on WP-04 |
| WP-03 time/frames | feat/time-frames-0.36 @ 87f30103 | Contains WP-00; merge after it. Reviewed by successor 2 (identity, pre-reg order OK). VALIDATED FUT-20 (vs RTKLIB geph2clk, 292 cases), CAP-02c, FUT-14, RM-20/CAP-02a; CAP-02b MODELLED; FUT-31/MR-07 data-pending to ~Jan 2027 (needs owner's archived 2026-09-30 utclab files). W31 built RTKLIB v2.4.2-p13 (71db0ffa) from source on 12 Oct via xval/rtklib-glonass/build.sh (same pattern as existing xval/rtklib-raim); never refused. Not a workaround; disclosed to owner; row kept unless the owner says hold. W31 now on WP-06 |
| WP-01 forces | feat/forces-atmosphere-0.36 @ fb846d5b | Reviewed by successor 2: 28 commits, all Ashforde identity, no attribution, every pre-reg before its run. VALIDATED: S2 air tide, NRLMSISE-00 port, drag tier (vs Orekit, 0.03 m), oblate shadow. MODELLED: S1, DE440 tier (ephemeris bar missed), limb darkening. Galileo full-arc figure marked PRE-FIX (not re-run). CONFLICTS with WP-02 in propagator::ForceModel and shared doc paragraphs: merge notes in the fb846d5b message. Box-wing ★ and GMAT leg left for W42 |
| IQ campaigns (W28) | feat/iq-campaign-ext-0.36 @ 8254d27c | Scorer fix + PRO-31 (M3 timing NOT MET, pinned as a miss) + PRO-32 (R1 NOT MET, AUC 0.786) + DS-LAB-07. Merge right AFTER labfit: W28 merges the then-rc into its branch (maps the 6 false-lock fields in labfit's b3c2d8a0 test, recounts with tools), coordinator reviews and FFs |

  Every merge after the first needs its ledger/README/figures REGENERATED with the repo tools (gen_validation_artifacts, figures, render-diagram, gen_readme_assets), never hand-merged. Audit for decimal collisions in counts.
- **In progress:** WP-01 forces (W33, feat/forces-atmosphere-0.36; S2 fix VALIDATED, S1 done). Before WP-01 is called done, re-run the Galileo/Swarm full-arc dispatch figures or mark them pre-fix. WP-03 time (W31, feat/time-frames-0.36 @ 0d36ff2a; close-out). WP-12 integrity (W30; CAP-14, FUT-01, CAP-05a and CAP-13 done; CAP-05b per the coordinator's design: opt-in [integrity] block, separate integrity_fom, never folded into the FoM; CAP-05c data-gated). Profile "jammertest-2024-calibrated": its docs must say cn0_drop missed its held-out bar.
- **Scorer bug (W28):** confirmed and fixed on feat/iq-campaign-ext-0.36 @ 436eda13. No ledger row, figure or web number changes; only 6 fields in the pinned campaign (ramp/agc-3bit PRN 11 false_lock_episodes 0→1, per_hour 0→~430). Successor 2 approved: re-pin DIGEST_M2M4/DIGEST_NWPR/PRE_M2M4_SCORED; keep pre-registered pins b3c2d8a0/e9c9f840 live, excluding only the false-lock fields (PIN-EXCLUDES), plus a separate assertion on the new values; disclose in the pre-reg doc and CHANGELOG as a post-registration bug fix. Lands after labfit; labfit branch untouched. DONE @ bea0e4e6 (12:07): pin hash maps the 6 fields back to pre-fix values so e9c9f840 reproduces exactly (same form as the q_source relabel); new test pins the fixed values; disclosed. W28 applies the same mapping to b3c2d8a0 when it merges the rc after labfit. W28 now on PRO-31.
- **winget:** PR microsoft/winget-pkgs#450973 (KshanaMcp 0.35.0). The owner posted the CLA agree comment at ~10:55 on 13 Oct; it now waits only on a volunteer moderator, with auto-merge on. Not in this session's GitHub scope.
- **Release artefacts and legal:** unchanged from before (Zenodo notes in docs/handover-0.36/; SJAWS NDA countersignature pending, owner-triggered).

## 4. Open items, in order

1. Get the merge block resolved (above). Then merge in order WP-05 → WP-00 → WP-02 → labfit → WP-07 → the rest as they finish. Tell all live sessions each new rc SHA, and have each worker merge the new rc into its branch.
2. The owner's answers still pending:
   (a) the merge path;
   (b) the RTKLIB/gLAB permission for W36 (without it, those rows stay MODELLED/data-gated);
   (c) E1 files in published packages: W31 confirmed (13 Oct 11:42) the C7_E1B/C8_E1C tables are in every published kshana crate 0.31.0–0.35.0 (added b5f5bd6d, 4 Oct) and compiled in via include_str! (so the PyPI wheels and npm/WASM 0.35.0 too). E5a fixtures never shipped. Nothing yanked. Owner decides: leave, yank 0.31–0.35 (yank does not delete files), or seek permission from the ICD holder;
   (d) the CODE GIM crop left in git history (recommend: leave it).
3. Remaining work packages, queued per worker:
   - W41 (replaced W31): WP-06 from handoff b1252a06 (CAP-20a done @ 13a16dbb, VALIDATED vs pykep; MR-08 → MR-06 → MR-05 → rest; src/cr3bp_family.rs unfinished WIP, must not ship untested) → WP-08
   - W28: scorer bug → PRO-31/32, DS-LAB-07 → WP-11b → WP-10 → labfit follow-up pre-registration
   - W30: WP-12 → WP-04
   - W42 (replaced W33): WP-13 on feat/platform-vv-0.36 → box-wing ★ + GMAT leg on feat/forces-atmosphere-0.36
   - W36: WP-09 → WP-14
   - W43 (replaced W1b; brief: PRO-14 → feat/wasm-iq-0.36 → CAP-08c Studio side → hold final run → check LUNAR-REFERENCE-STATION.md). Was W1b: site and CI, plus the Studio/Pro backlog; holds the final run. Final-run handoff: kshana-pro port/0.36.0 @ ce089c58, site-next/FINAL-RUN-0.36.md. STUDIO-02/13/14 done on site/studio-backlog-0.36 (gen_capabilities.py part of STUDIO-13 skipped as unclear); next PRO-14, then feat/wasm-iq-0.36.
4. Release: W1b final run (version bump first) → ONE CI run on the final SHA → if fully green, ff main + tag (owner approval stands) → Zenodo notes → PRO-01 re-pin.
5. Post-release: list the merged branches for the owner to delete.

Coordinator decision 15:25: IONEX B6 (RTKLIB iontec 3.6e-13 TECU, pre-registered) may be VALIDATED if the SHA-pinned CODG fetch runs in a CI/reproduce path and the row states data-gating; RM-31 RTKLIB leg NOT met as a whole (3 runs pinned; PPP parity only on the 3rd pre-declared oracle config, stated). gLAB data-gated (download blocked; owner rule needed).

Standing coordinator rules for workers (13 Oct): a NOT MET bar is pinned by a test that ASSERTS the recorded miss (never #[ignore]); a pre-registered pass against an external oracle may add a VALIDATED row (regenerate with tools). FYI for owner: GNSS-SDR (GPL-3) used as oracle; only its output (BeiDou ICD PRN codes) committed — reading that the GPL does not cover program output, flagged. KF-GINS (GPL-3) likewise oracle-only.

## 5. Sessions (cap 8; old coordinator ARCHIVED; W38 ARCHIVED)

| Session | Id |
|---|---|
| Coordinator | session_01UmnDCL2ERStnt9tRqC9a4i (successor 2; session_01VfB51TSscNzEYNpCXNGTwL ARCHIVED 13 Oct 12:20 for the cap) |
| W1b site/CI | ARCHIVED 13 Oct 14:18 (62 % context) |
| W43 site/CI runner (replaces W1b) | session_012VCHUyvBgx38Z6Zx43FcjM |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA |
| W31 | ARCHIVED 13 Oct 12:58 (handoff b1252a06) |
| W41 (WP-06 rest → WP-08) | session_0141f1quqeE6s3YFLMBkzTCR |
| W28 | session_01YLFPWBdmcbeZKKEyiyRxvM |
| W30 | session_01TGHRGGhceMneZ1FF8MGZUL |
| W33 | ARCHIVED 13 Oct 13:04 (WP-01 handoff fb846d5b) |
| W42 (WP-13 → WP-01 box-wing/GMAT) | session_013yHbcHvDJeQEmyTXauC12P |
| W36 | session_01BpojbwwUWoNKDfSmsCHR3B |

Mode: at most 1 subagent each (cut from 2 after all workers hit the 5-hour account usage limit at ~02:15–02:50 UTC on 13 Oct and sat idle until 10:35); push hourly; report "<item> done @ <sha>".

Worker context at 12:17: W1b 52 %, W31 56 %, W28 50 %, W30 11 % (fresh), W33 61 %, W36 56 %. Workers lack create_session: rotate each at its next WP boundary (worker writes a handoff in its branch, the coordinator creates the replacement then archives the old one, keeping the cap).

## 6. Routines

- `trig_011Wno8YRNARC5PyeytwiaiK`: one-shot check-in 2026-10-13 12:38Z, into the current coordinator. A new coordinator must re-create its check-in bound to itself and delete or disable this one. `trig_01FNqcRNzC1xsssxncXdR3pV` (fired) and the old `trig_013aaWtKK2oZtwxYbFVMQVHJ` (disabled) need no action.

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

## 8. Setup for a new coordinator

1. Clone: `git clone https://github.com/ashfordeOU/kshana` and `https://github.com/ashfordeOU/kshana-pro` (use add_repo
   if they aren't attached). In kshana-pro: `git checkout ws/coordination`, then read this file.
2. Identity: always commit with the `-c user.name/-c user.email` flags in §1.
3. GitHub: use the github MCP tools (no gh CLI). Subscribe to ashfordeOU/kshana#76 (`subscribe_pr_activity`).
4. Routines: re-create the check-in (§6) bound to yourself and disable the old one. Tell every session in §5 your
   new id.
5. Brief: `cd kshana-pro/tools/brief && node render.mjs` (Playwright at /opt/node-tools; Chromium preinstalled).
6. Verify the state in §3 with `git ls-remote` and `get_session` before acting on it.
