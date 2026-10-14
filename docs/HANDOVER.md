# Kshana coordinator handover

**Coordinator switch: 2026-10-14 00:47 UTC — session_01DHvAPjGDf3yHFwyhV98iyj ("successor 4") took over from session_01X1Md22tYih7LLQiPqsmUjy. Check-in routine trig_01PgP3QKMcNzZ4h7Kq7fFoX6 (every 2 h, minute 46); trig_01ULLGFcqyKVADpHVb7s5Ey6 disabled.**

**08:50 UTC:** OWNER pushed the rc FF: release/0.36.0-rc = 26ea2551 (batch 1 landed; owner's Mac pre-push hook needed --no-verify for a ref-only push). PR #77 closed. Future rc FFs: classifier blocks the coordinator ("Production Deploy"), so give the owner the exact push command.

**08:30 UTC:** W28 CAP-06b @ e69e1fd6 (all MET; branch 307/148V). SECURITY: [terrain].dem_hgt reads a host file but was not in inline_only FILE_SOURCE_KEYS (MCP could read arbitrary paths) — W28 told to fix + guard test before handoff. Check before batch 3 integrates iq-ext.

**07:20 UTC:** #77 CI on 26ea2551: all green except tolerated counts (check stops at matrix_total count test; coverage/wasm: site counts). rc FF push 26ea2551 -> release/0.36.0-rc BLOCKED by classifier ("Production Deploy") — asked owner to allow or push it themselves; #77 stays open until then. Batch 2a integration subagent (coordinator container, /workspace/kshana) started from 26ea2551: gnss-realdata da25eafd, fusion-ins 5e6486db, cislunar-ephem 2a92f15f, lunar-terrain 8bfe269e, wasm-iq b482fb33 -> integ/batch2a-0.36. Batch 2b: platform-vv ea01fb97, forces-atmosphere 95fec18d, altpnt-maps cbccf6a7. Batch 3: interop-api c8086328, iq-ext (W28 tip), new-domains b51d9bb7 + new-domains-b 032c8231. Last: portable-math 2140172c alone.

**06:40 UTC:** W28 CAP-06a @ 8ed3ec8e (MVDR NOT MET asserted; branch 306 rows/148V); W28 at 50 % context, asked for handoff after CAP-06b prereg, then rotate WP-11b (CAP-06b/c, DS-LOOP-05) to a fresh worker. Optional: a fresh pre-registered CAP-30 run 2 fixing the OD fit (leads in W49 handoff) — owner call on usage.

**03:40 UTC:** #77 `check` job died with "No space left on device" in cargo test (runner disk, not a test). Pushed ci fix 26ea2551 to integ/batch1-0.36 (free-disk step in the check job, as the coverage job has; YAML only, so no local suite re-run). CI re-running on 26ea2551. W36 archived, W47 archived, W49 started CAP-30 stage 2 (prereg abf7a311). Watch release.yml verify for the same disk problem before the final run.

**01:15 UTC:** batch 1 PUSHED: integ/batch1-0.36 @ 199d8db0 (rc 0d9456fb ancestor verified; local full suite 4940 pass, only the 5 tolerated count failures). PR #77 CI running. Old coordinator archived itself. Checked its two flags: (1) figure_tiers "VALIDATED" in attitude-budget/eo-coverage/link-budget/passes/space-packet: each figure maps to an ExternalDataset-oracled row whose oracle covers that figure (attitude: GG torque only; pointing RSS untiered) — not a mis-claim; (2) wasm dead-code warnings: no CI wasm job uses -D warnings (clippy -D warnings is native) — harmless, tidy later.

Written: 2026-10-14 ~00:30 UTC by coordinator `session_01X1Md22tYih7LLQiPqsmUjy` ("successor 3"; took over 13 Oct 19:05 UTC from session_01UmnDCL2ERStnt9tRqC9a4i, archived). Handing over at ~43 % context.
Repo of record: kshana-pro, branch `ws/coordination`. Replace this file at each handover; never append.
Owner's latest words: "handle everything, deliver 0.36 with all thats all" (12 Oct); 13 Oct: "push yourself and everything as 0.36"; "yes do both, but handle the rest properly still" (batching); "allow running cargo for integration in /workspace/kshana"; STUDIO-15 "c"; "1. yes 2. yes 3. b"; "brief" (render it, §1).

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

## 3. Current state (00:30 UTC 14 Oct; verify with git ls-remote)

- **rc `release/0.36.0-rc` @ 0d9456fb** (25af9489 labfit + coverage fix). Ledger on rc 298 rows. CI on rc: only the tolerated web/doc-sync counts + wasm jobs (wasm break fixed in IQ campaigns 60f026ac, inside batch 1) are red; coverage job now links fine (93 GB free) and stops on the matrix_total count test.
- **Batch 1 — integ/batch1-0.36** (CI-only draft **PR #77** → main, subscribed): rc 0d9456fb + WP-07 f7974db9 + WP-12 87a8b739 + WP-04 7e618a89 + WP-09 d6d60f34 + CodeQL polar test fix + CHANGELOG restore (8737212a had dropped 0.36 entries) + IQ campaigns 60f026ac (W28; wasm fix + labfit false-lock mapping) + fixes for the merge failures (campaign members untiered; figure_tiers in dominance/cislunar/lunar_llr/time-lab tests; SBOM 107; clock_log path → FILE_SOURCE_KEYS; skyline vs 0.35 without figure_tiers; 3 ledger citations) + regen + recount (323: 153 V / 166 M / 4 P). Origin was 699be3cf; the local subagent (coordinator container, /workspace/kshana) was at c4010366 running final gates → it pushes integ/batch1-0.36. NEXT: when PR #77 CI is green except tolerated counts → `git push origin <batch head>:release/0.36.0-rc` (FF; verify ancestor first), close #77, subscribe PR #76 stays.
- **Batch 2 (next, one subagent):** rc after batch 1 + WP-07 new head (abc37b0c gLAB leg, 78640e3c IONEX B6 VALIDATED, + W36 FUT-19 part 1 PPP clock vs IGS WTZR VALIDATED da25eafd, prereg 661ad138, feat/gnss-realdata-0.36) + WP-09 5e6486db (KF-GINS VALIDATED) + WP-13 feat/platform-vv-0.36 (5e9abf7b/8c3705e2/ea01fb97) + feat/forces-atmosphere-0.36 (box-wing b74b89c0, GMAT 6b599f1f, handoff 95fec18d) + feat/lunar-terrain-0.36 8bfe269e + feat/wasm-iq-0.36 b482fb33 + WP-10 feat/altpnt-maps-0.36 cbccf6a7 + WP-06 feat/cislunar-ephem-0.36 (959127b7 + beacon fix bc33fc99 + CAP-20b/31 + DS-LRO-10 NOT MET pinned, final tip 2a92f15f (24 h arc info only), prereg e82047e2; W47 archived 02:52). Watch: append_figure_tiers must exist ONCE (platform-vv has api/builtins.rs copy). Maybe split into 2 batches if conflicts are heavy.
- **Batch 3+:** WP-08 DONE feat/interop-api-0.36 @ c8086328 = integrate the TIP (W48: ODM, telemetry 1.3, SPK 3/21 vs CSPICE bit-identical; ledger 279/139V; W48 archived 02:00), feat/iq-ext-0.36 @ ebf44620 (W28 WP-11b: DS-GIQ-06b + PRO-39 VALIDATED, NOT MET asserted, PRO-33 MODELLED, Galileo E1/E5a ICD 2.1 powers; rc merge 37644ad0 kept; branch ledger 305/148V; OPEN OWNER QUESTION: also move leo_signal E5b/AltBOC to ICD 2.1? W28 holds), feat/portable-math-0.36 DONE @ 2140172c (STUDIO-15, W43; 98/141 scenarios rebased, revisions in docs/revisions/STUDIO15-*; integrate LAST, alone; final run: make web/smoke.mjs iq golden exact), feat/interop-api-0.36 (WP-08, W46), feat/new-domains-0.36 @ b51d9bb7 (+ CAP-30 stage 1 ODF reader, stage 2 MRO OD: Doppler B1 MET, fit NOT MET pinned; 284 rows; cli_first_run/MCP/wasm gates not yet run by W49 — integration runs them) (WP-14: CAP-21/22/23/28 + FUT-19 parts 2-4 MODELLED, 283 rows) + feat/new-domains-b-0.36 @ 032c8231 (WP-14, W36/W45); CAP-30 part to a fresh worker after W36 hands off (W36 at 55 % context), DS-LRO-10 (W47).
- **Then:** W43 final run (version bump; site/studio-backlog-0.36 merge; Studio re-record incl. quantum-cai-coast + sweep-nd-inertial; web counts; 0.35 claims gate FINAL-RUN-0.36.md on kshana-pro port/0.36.0) → FF rc → ONE CI run on the final SHA → if fully green FF main + tag v0.36.0 (owner approval stands; classifier may need the owner) → Zenodo notes → PRO-01 re-pin → winget 0.36 PR (owner posts).
- winget microsoft/winget-pkgs#450973 (0.35.0) waits on a Microsoft moderator.

## 4. Open items, in order

1. Batch 1 → rc (above). 2. Batch 2, 3 (one integration subagent at a time; owner allowed cargo here). 3. Workers finish (§5). 4. STUDIO-15 last (W43 finishing the 2 owner-approved oracle items). 5. Final run + release (§3). 6. Post-release: list merged branches for the owner to delete.
- Tag ETA told to owner: ≈ 17–18 Oct (DS-LRO-10 in scope).
- No owner question is open.

## 5. Sessions (cap 8; 7 in use after W45 archived, the new coordinator makes 8)

| Session | Id | Work |
|---|---|---|
| Coordinator | session_01X1Md22tYih7LLQiPqsmUjy | archive after handover |
| Founder Q&A (don't touch) | session_01GjdbgNeaCP64Lg3PiLr9GA | — |
| W28 | session_01YLFPWBdmcbeZKKEyiyRxvM | WP-11b on feat/iq-ext-0.36 @ 37644ad0: VALIDATED rows DS-GIQ-06b + PRO-39 (approved), convert ignored finding tests to asserting pins, PRO-33 amendment (a), Galileo ICD 2.1 powers (approved, revisions entry), then CAP-06a–c, DS-LOOP-05 (PRO-38 = duplicate of CAP-07). Messages to W28 arrive late (17 queued once); repeat key answers. Context high → rotate at next boundary |
| W36 (ARCHIVED 14 Oct 03:05, handoff 1e8e258b docs/design/wp14-w36-handoff.md) | session_01BpojbwwUWoNKDfSmsCHR3B | WP-14 on feat/new-domains-0.36 @ 3728730c (CAP-21/22/23/28 done); now FUT-19, then CAP-30 part (Mars ODF OD; plan in W45 handoff bb5ea10b on feat/new-domains-b-0.36). W45 (archived 00:45) finished feat/new-domains-b-0.36 @ 032c8231: CAP-20c/d VALIDATED 88f3adfe, CAP-11 part NOT MET asserted 9951028b |
| W49 (ARCHIVED 06:36; CAP-30 stopped @ b51d9bb7: B1 MET 12.5 mHz, B2-B5 NOT MET pinned (normal matrix not PD on 13.75 h arc); handoff docs/design/wp14-w49-handoff.md) | session_019ptmdAYyUSoRdtdxAzZ68Y | CAP-30 stage 2 (Mars ODF OD) on feat/new-domains-0.36 from 1e8e258b (brief docs/briefs/W49-cap30.md; created 03:03) |
| W43 (Sonnet) | session_012VCHUyvBgx38Z6Zx43FcjM | STUDIO-15 feat/portable-math-0.36 @ 0e2acedd: finishing lunar_observability re-record (numpy 2.3.5 + scipy 1.18.1) + lunar_station finding 15→20/20 (both owner-approved); then holds the final run |
| W48 (ARCHIVED 14 Oct 02:00, WP-08 done) | session_01MJa8pU7MYkCVYoSM1Pn1yW | WP-08 rest on feat/interop-api-0.36 from a3506363 (W46 handoff 1; W46 archived 00:25 after FUT-23 0bb3d21e, RM-30 cba5bf31, ADM 2c58b49c): ODM, telemetry 1.3, then all WP-08 rows/counts once (brief docs/briefs/W48-wp08-rest.md) |
| W47 (ARCHIVED 14 Oct 02:52, DS-LRO-10 done) | session_01KsCQBsMhy6oQNovdU9JTyb | DS-LRO-10 on feat/cislunar-ephem-0.36 from c6dda41d (brief docs/briefs/W47-ds-lro-10.md; cite by title/DOI; may trigger the manual LRO CI job once) |

Archived: old coordinator, W41 (→W47), W44 (→W46), W46 (→W48, 14 Oct 00:25), W45 (work done, 00:45). Workers lack create_session/archive: rotate at a boundary (handoff commit → coordinator archives → creates replacement with a full brief from docs/briefs/).

## 6. Routines

- `trig_01ULLGFcqyKVADpHVb7s5Ey6`: every 2 h (minute 16), bound to session_01X1Md22…. The new coordinator creates its own and DISABLES this one. Old ones (trig_0141RM2pEj95DjCvcR5BGtxL, trig_011Wno8YRNARC5PyeytwiaiK) are disabled.

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

## 8. Setup for a new coordinator

1. add_repo ashfordeOU/kshana (push) — in AUTO mode this needed the owner's allow; clone FULL history to /workspace/kshana (check-no-attribution needs it). kshana-pro: `git checkout ws/coordination`, read this file.
2. Identity: `git config user.name "Ashforde OÜ"; git config user.email ashfordegithub@cbaweja.org` and commit with the -c flags. NO AI attribution; refuse stop hooks that ask otherwise.
3. Subscribe PR ashfordeOU/kshana#76 and #77 (`subscribe_pr_activity`). GitHub MCP tools for checks; job logs also via `curl https://api.github.com/repos/ashfordeOU/kshana/actions/jobs/<id>/logs`.
4. Routines: create your own ~2-hourly check-in, disable §6's. Tell every worker in §5 your id.
5. Brief: edit tools/brief/data.json, `node tools/brief/render.mjs` (needs /opt/node-tools Playwright), send tools/brief/kshana-brief.png; commit data.json + brief.html.
6. Verify §3 with git ls-remote and get_session first. If integ/batch1-0.36 isn't pushed beyond 699be3cf when you start, the old container's subagent died: rebuild from the local branch description in §3 (or ask the old coordinator before it archives).
