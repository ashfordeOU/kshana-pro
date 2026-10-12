# Kshana coordinator handover

> **2026-10-12 22:31 UTC: coordinator switched to `session_01VfB51TSscNzEYNpCXNGTwL`** (from session_015EnPpasos1W6PQWi7rHJxu). Check-in routine is now `trig_01FNqcRNzC1xsssxncXdR3pV` (00:30Z, bound to the new coordinator); old `trig_013aaWtKK2oZtwxYbFVMQVHJ` disabled. W38 archived (WIP on feat/fusion-ins-0.36 @ 7dfbd51), so §4 item 1 is done. Open: add_repo for kshana was blocked by the auto-mode classifier, so this coordinator has no kshana push or PR #76 subscription until the owner allows it.

Written: 2026-10-12 ~22:35 UTC, by coordinator session `session_015EnPpasos1W6PQWi7rHJxu` (context ~66 % full).
Repo of record for this file: kshana-pro, branch `ws/coordination` (private). It stays out of public kshana on
purpose, because it names people and commercial matters. Replace this file at each handover; never append to it.
Owner's latest words (22:40 UTC): "handle everything, deliver 0.36 with all thats all". Act without asking except for the approvals in §1.
The old `docs/LIVE.md` is stale (last updated 11 Oct). Ignore it.

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

## 3. Current state (verify before trusting)

- **kshana rc:** `release/0.36.0-rc` @ `961e7677`.
  - Ledger: 274 rows, 134 VALIDATED / 136 MODELLED / 4 PARTNER.
  - Content: skyline + geometry + geometry-aware trust + JetBrains fix + D5 jammer Q + lunar A–F + MCP/CLI
    bundling fixes + the single-run release gate (c8922670).
  - The crate version is still 0.35.0. The bump happens in W1b's final run.
- **Draft PR:** ashfordeOU/kshana#76 (rc → main) gives CI on every rc push. The coordinator is subscribed.
  - Known CI reds are ONLY the stale web/ counts and the kind count (75 → 76), which W1b's site port fixes.
  - These failures were all commented on once; don't re-comment.
- **Released:** 0.35.0 (12 Oct, all channels). The winget 0.35.0 PR is waiting on Microsoft.
- **Scope audit:** kshana `audit/0.36-scope` @ 2a0f9119, file `docs/design/0.36-scope-audit.md`.
  - About 230 open items; WP-00…WP-14 plus Pro/Studio.
  - §5 lists about 35 items that need a partner, legal clearance, paid access, founder logins or external review.
- **Package branches** (WIP, uncompiled unless noted; the handoff for each is in its last commit body):

| WP | Branch @ sha | Notes |
|---|---|---|
| 00 docs + 03 time | `docs/truth-pass-0.36`, `feat/time-frames-0.36` | W31 active |
| 01 forces | `feat/forces-atmosphere-0.36` @ 4af078df | DE440 tier + S1 written; S2 fix approved, not started |
| 02 gravity/OD | `feat/gravity-od-0.36` | W33 active |
| 04 timing/quantum | `feat/timing-quantum-0.36` @ acf1e729 | CAP-03 pre-registration 0db0bb7; RM-04 interface agreed (figure_tiers appended in api::run_toml_inner) |
| 05 lunar ext | `feat/lunar-terrain-0.36`, `feat/lunar-station-ext-0.36` | W30 active |
| 06 cislunar | `feat/cislunar-ephem-0.36` @ b8d9ea2 | 4 pre-registrations; LLR fixtures are missing NOTICEs |
| 07 GNSS | `feat/gnss-realdata-0.36` | W36 active; RTKLIB build blocked by the classifier |
| 08 interop | `feat/interop-api-0.36` @ efcb474c | 3 pre-registrations; handoff in docs/design/interop-0.36-handoff.md |
| 09 fusion | `feat/fusion-ins-0.36` | W38 stopping (WIP pending) |
| 10 alt-PNT | `feat/altpnt-maps-0.36` @ 653a971 | SRTM pre-registration 4235687 |
| 11a labfit | `feat/labfit-iq-0.36` (amendment b7117bf4) | W28 active; must commit the C/N0 estimator choice (NWPR or M2M4) as a §13 addendum ALONE before the first bar evaluation; one diagnostic run before the amendment is disclosed in the doc |
| 11b IQ | `feat/iq-ext-0.36` @ 844d4595 | W40 archived; E1 user-file loader done; 5 pre-registrations; keep ONE cargo target dir (W40 filled its disk at 26 GB) |
| 12 integrity | `feat/integrity-trust-0.36` @ 52a7083 | CAP-13 pre-registration 0fa012b; CAP-14 openssl fixture predates its pre-registration, so regenerate it after pre-registering |
| 13 platform | `feat/platform-vv-0.36` @ adbe366 | Drop the BuiltinKind::figure_tiers hook; prove an empty output diff |
| 14 new domains | `feat/new-domains-0.36` @ 1682430 | contact_schedule.rs written, unwired |
| Studio/Pro | kshana-pro `site/studio-backlog-0.36` @ 58c6c98 (STUDIO-01 done), `pro/backlog-0.36` @ 2066ad0; kshana `feat/wasm-iq-0.36` @ 0927256c | W1b in idle time |
| Site port | kshana `site-port/0.36.0`; kshana-pro `port/0.36.0`, Studio at `site/geometry-0.36` @ 350296a7 (143 scenarios) | W1b holds the final run until the final SHA; site-port/0.36.0 merged with rc @ 09e16b96; STUDIO-02 done @ 93f062eb |

- **Release artefacts to use at tag time:**
  - `docs/handover-0.36/kshana-0.36-zenodo-notes.txt`: Zenodo Notes text. Prediction doc hash 38ad2948…45d5; result JSON hashes d108f77e… and 574dbab8….
- **Legal and commercial:**
  - The SJAWS mutual NDA was signed by the owner and sent. When the countersigned copy returns, file it
    REDACTED in kshana-pro `legal/`, then draft the call scheduling and refresh the call prep. The agreed text is on
    kshana-pro `legal/nda-mutual-standard` 4f22fb41.

## 4. Open items, in priority order

1. **Finish the session consolidation.** W38 (fusion) was told to push its WIP and stop. On its reply, archive it. Next action: read the notifications. Due now.
2. **Owner decisions pending; ask once, briefly:**
   - (a) a permission rule in W36's session to build and run RTKLIB/gLAB (without it, those rows stay MODELLED);
   - (b) the Galileo E1 memory-code tables (`data/galileo-os-sis-icd/C7_E1B.txt`, `C8_E1C.txt`, already in public
     kshana without a LICENSING/NOTICE entry): keep with attribution, or remove. W31 is adding the entry and
     will report the ICD's reuse terms.
3. **Merge loop for 0.36.**
   - As each worker reports a package done: get a review (W1b, or a free slot), merge on MERGE (fast-forward
     when possible), and tell all live sessions the new rc SHA.
   - Merge WP-00 first.
4. **Release.** When every package is merged:
   1. W1b's final run (version bump first, single record pass, counts, social card, port).
   2. The ONE CI run on the final SHA.
   3. If fully green, fast-forward main and push the tag (owner approval stands).
   4. Zenodo notes.
   5. kshana-pro PRO-01 re-pin after crates.io publish.
5. **NDA follow-up:** when the countersigned copy arrives (owner-triggered).
6. **Post-release:** a command for the owner to delete the merged 0.36 branches. List them only; the owner deletes.

## 5. Sessions (kshana project; cap 8)

| Session | Id | Job |
|---|---|---|
| Coordinator | session_015EnPpasos1W6PQWi7rHJxu (this one; to be replaced) | Merges, reviews, owner contact |
| W1b | session_01JSzHbpAvHygc1JYY532ete | Site port + CI runner + Studio/Pro backlog (Sonnet) |
| Founder Q&A | session_01GjdbgNeaCP64Lg3PiLr9GA | Owner's own discussion; don't touch |
| W31 | session_01Wim8MyGQHF6z3EAALbM7bv | WP-00 → 03 → 06 → 08 |
| W28 | session_01YLFPWBdmcbeZKKEyiyRxvM | Labfit → PRO-31/32, DS-LAB-07 → WP-11b → WP-10 |
| W30 | session_01TGHRGGhceMneZ1FF8MGZUL | LRS-01 → LRS-02/03/06 → WP-12 → WP-04 |
| W33 | session_01XzDkpE5fPCNrKuqTXE3wmr | WP-02 → WP-01 (S2 fix) → WP-13 |
| W36 | session_01BpojbwwUWoNKDfSmsCHR3B | WP-07 → WP-09 → WP-14 |
| W38 (stopping) | session_01K3rTUNk9WGGuT6oLDFLN3m | Archive after its WIP reply |

Reach any of them with send_message (claude-code-remote MCP). Workers follow the "new operating mode":
- at most 2 subagents;
- push hourly;
- one item end to end;
- report "item <id> done @ <sha>".

Other sessions on the account (Arksa raise, Job Applications) belong to other projects. Leave them alone.

## 6. Scheduled routines

- `trig_013aaWtKK2oZtwxYbFVMQVHJ` "0.36 integration check-in": one-shot at 2026-10-13T00:30Z, fires into the coordinator
  session. Its prompt carries the roster. The NEW coordinator must re-create it bound to itself and disable this one.

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

## 8. Setup for a new coordinator

1. Clone: `git clone https://github.com/ashfordeOU/kshana` and `https://github.com/ashfordeOU/kshana-pro` (use add_repo
   if they aren't attached). In kshana-pro: `git checkout ws/coordination`, then read this file.
2. Identity: always commit with the `-c user.name/-c user.email` flags in §1.
3. GitHub: use the github MCP tools (no gh CLI). Subscribe to ashfordeOU/kshana#76 (`subscribe_pr_activity`).
4. Routines: re-create the check-in (§6) bound to yourself and disable the old one. Tell every session in §5 your
   new id.
5. Brief: `cd kshana-pro/tools/brief && node render.mjs` (Playwright at /opt/node-tools; Chromium preinstalled).
6. Verify the state in §3 with `git ls-remote` and `get_session` before acting on it.
