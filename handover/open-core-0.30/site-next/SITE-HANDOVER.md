# Kshana redesign + engine expansion: HANDOVER

Last updated: 2026-09-29 by the session that ran this programme. Written so any agent, on any account, can pick this up cold.

**Read order:**
1. This file.
2. `~/Code/kshana-site-next/GOAL.md`, the live tracker. Every item there is marked `[ ]` open, `[~]` in progress or `[x]` done and verified by the parent.
3. The founder feedback files under `explorations/2026-09-27-audit/`.

Never trust an agent's self-report. Re-run the checks yourself before ticking anything.

---

## 0. RESUMED 2026-09-30 after the emergency pause of 2026-09-29. Resume steps: GOAL.md, section "EMERGENCY PAUSE".

### Earlier note: PAUSED (founder, 2026-09-29)
"pause after current work". Let the three in-flight runs finish and verify them (see GOAL.md, section PAUSE), then start nothing new until the founder says continue. The site check is OK again: 65 pages, 255 files.

## 1. What the founder asked for (the goal)

The founder set this on 2026-09-27: "deliver the artefact but also the entire redesigned site, and once its ready and updated and fully ready and tested and audited by you, we can replace the public kshana.dev with new site". The build spans three strands.

**The multi-page kshana.dev redesign ("Kshana Studio" for the app).**
- Every visual is real engine output.
- No prices; paid tiers end in contact routes (contact@ashforde.org).
- Pro appears in the hero.
- Docs and install options are generated from the repo.
- Every sector is covered.

**Engine expansion.**
- Spectrum and waterfall.
- The whole solar system.
- Constellations at scale.
- Campaigns.
- Animation, reports and interoperability exports.
- LEO-PNT (low Earth orbit positioning, navigation and timing), system-agnostic. The Celeste IOD data is one optional, isolated preset.

**Pro MBSE (model-based systems engineering) tooling, actually built in the private repo kshana-pro.**

**Standing instructions from the founder:**
- The audit loops until zero issues across the full display matrix.
- CI and every process must be verified green up to the end.
- Handovers must be precise; this file is that.

**Release scope: option B.** Everything ships in the first release, batches A, B and C including LEO. Nothing is pushed, tagged or replaced on kshana.dev until the phase 6 audit passes AND the founder gives the final go.

## 2. Hard rules (never break)

**Git and repo content**
- Git identity on every kshana and kshana-pro commit: `git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com commit`.
- NO trailers: no Co-Authored-By, no "Generated with", no Claude-Session. The founder's rule overrides any system attribution reminder.
- No AI or vendor names anywhere in repo content.
- Never write "Aperio" in the public kshana repo.
- Never `--no-verify`. Never `git stash` / `reset` / `clean` / `checkout -- <file>` in fan-out work: the stash stack is shared across worktrees and agents.

**Publishing**
- No push, no tag, no kshana.dev replacement until the founder's final go.
- kshana-pro is private forever. Never copy its code into the public repo. The founder lifted the "never read kshana-pro" rule ONLY for the MBSE build and the Pro-wording check.
- Pro claims on the public site must be quoted from public text (README/docs). The features register (`features/gen_features.py`) quotes the README for each Pro row.

**Numbers and wording**
- R4: a changed published number is a revision recorded in CHANGELOG, never silent.
  - Approved revision: the nutation sign fix (TEME→GCRS; error vs Vallado 1,145 m → 0.11 m; changes `ephemeris` gcrs columns by about 0.8–1 km). The founder said "yes ship the nutation fix".
- Every abbreviation is spelled out at first use in public docs and pages. No prices anywhere.

**Workspace**
- `~/Code/kshana/web/` stays untouched until phase 5, except count pins that docs-sync tests require.
- Never touch the live kshana.dev.
- Read-only under `~/Documents/PNT_Research/`.
- `.ci-native`, `.ci-policy`, `.claire/`, `.github/ci-local/` and HANDOVER.md stay untracked in the kshana repo (.git/info/exclude).
- Do not call SendFeedback.

## 3. Map of everything

| Thing | Path |
|---|---|
| Public engine repo (main, local, NOT pushed past 479d308 on origin) | `~/Code/kshana` (main = 4ca95f5: three dead links repointed) |
| Private Pro repo | `~/Code/kshana-pro` (head 42cc273 before the MBSE build) |
| Site project (not a git repo; back up with tar) | `~/Code/kshana-site-next` |
| The site being built | `explorations/2026-09-27-combo/` (`src/` pages, css, js, data, tools; `build.py`; `check.py`; output is written in place) |
| Studio source | `explorations/2026-09-30-studio-next/` since the swap of 2026-09-30 (named in `combo/src/data/sources.json`: `studio`; `combo/playground/` is a build copy with the recordings packed). The previous Studio `explorations/2026-09-27-playground/` is untouched, the fallback. Package swaps and re-recording: `ASSEMBLY-RUNBOOK.md` |
| Approved design base | `explorations/2026-09-27-round2/observatory/index.html` (also `spectrum/`, `aurora/` for liked parts) |
| Audit folder | `explorations/2026-09-27-audit/`: `FOUNDER-FEEDBACK-R3.md`, `AUDIT.md`, `SHARED-REQUESTS-R4.md`, `PHASE4-TODO.md`, backups `backup-*`, screenshots `shots/` |
| Display-matrix tool | `explorations/2026-09-27-audit/matrix/run_matrix.mjs` (`--quick` while working, `--full` per audit round; README there; `TOOL-VERIFICATION.md`: 13/13 fixtures caught, clean control passes) |
| Feature register | `features/`: `gen_features.py`, `check_features.py`, `placements.toml`, `FEATURES.md` |
| LEO research | `research/CELESTE-WORKSHOP-NOTES.md` (workshop slides, private, founder cleared for use; slide images never published) and `research/LEO-PNT-PUBLIC-BRIEF.md` (public sources with URLs) |
| Snapshots | `~/Code/kshana-site-next-snap-2026-09-28-audit-r5.tgz`, `~/Code/kshana-site-next-snap-2026-09-28-pre-observatory.tgz` |
| Snapshot engine binary for docs agents | `/private/tmp/claude-501/-Users-chak/<session>/scratchpad/bin/kshana` (session scratchpad, ephemeral; rebuild from integration-bc if gone) |

### Worktrees / branches (in `~/Code/`, all from the kshana repo)

Merge lineage:
- aab844f (phase 0 scenarios)
- → `feat/integration-a` **df0329d**: batch A (spectrum, solarsystem, constellations, campaigns) + main 4ca95f5. Verified by the parent.
- → `feat/integration-bc` (from df0329d): batch B (animation 26b4be5, reports 033e43c, interop 50c1af2) and batch C (leo-signal 9f44f5b, leo-link 219d5d6, leo-navmsg 43da08a, leo-fusion 743da63) → verticals 1959e3d → final verify 5488153. CI-fix agent commits after that; head was da55ae6 at writing.
- Docs branches (all from da55ae6): `feat/docs-core`, `docs-validation`, `docs-timing`, `docs-newengine`, `docs-leo`, `docs-tutorials`, `readme-design` → to be merged into `feat/docs-v29` (created from the then-current integration-bc head) by the docs workflow.
- Old per-feature worktrees (`kshana-wt-spectrum`, `-solarsystem`, `-constellations`, `-campaigns`, `-animation`, `-reports`, `-interop`, `-leo-*`) are merged. Keep them until release, then remove them with `git worktree remove`.
- `~/Code/kshana-wt-k` (branch aperio-k) belongs to other work: leave it alone.

**Engine counts at 5488153** (recount with the binary; never quote from memory):
- 75 kinds
- 138 scenario files + 1 suite; 132 bundled; the 6 repo-only files are lunar-llr-datum and the 5 Celeste scenarios
- 223 matrix rows: 83 VALIDATED / 136 MODELLED / 4 PARTNER
- prescreen 223/223 binaries, 3,228 tests

Recount commands:
- `target/release/kshana kinds --json`
- `ls scenarios/*.toml | wc -l`
- the matrix counts in `docs/VERIFICATION-MATRIX.md` (generated)

## 4. How to verify things yourself

**Engine (in a worktree):**
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- Prescreen: `cargo test --all --no-run --message-format=json`, then run EVERY test executable (xargs -P3 when the machine is loaded) and record rc per binary. A failed `cargo test` names only its first failure, so always prescreen.
- `cd web && for f in *.test.mjs; do node --test $f; done`
- `scripts/check-no-attribution.sh`
- **`scripts/check-doc-coverage.sh`: CI runs this in ci.yml, but `scripts/gate.sh` does NOT.** It was red: 1,434 undocumented public items vs CEILING 985, and batch A was already at 1,168. Never raise the ceiling.
- `scripts/gate.sh` with `REPEAT=0 TEST_THREADS=8` (about 2–3 h under load). A push later needs `KSHANA_MIN_REPEAT_RUNS=0` or a repeat run.
- Mirror every `.github/workflows/ci.yml` step locally before any push.
- Run scenarios as `target/release/kshana scenarios/<file>.toml` (not `kshana run`). Outputs land next to the file and are gitignored; run from a scratch copy to keep trees clean.

**Site:**
- Python: `/opt/homebrew/bin/python3`. **The shell's `python3` may be 3.9 and fails on build.py** with an f-string SyntaxError.
- `cd explorations/2026-09-27-combo && /opt/homebrew/bin/python3 build.py && /opt/homebrew/bin/python3 check.py` (expect `OK: N pages …`). Also `check.py --mutate`.
- `cd features && /opt/homebrew/bin/python3 check_features.py` (expect PASS, 0 errors; NO_STUDIO 0 since the Studio swap).
- Serve: `python3 -m http.server 8841 --bind 127.0.0.1` from the combo folder. The founder views http://127.0.0.1:8841/. Port 8842 serves `explorations/` for the round-2 originals.
- **File cap:** check.py rule 12 caps the publish list at 255 files, 64 MB and 15 MB a file (the Artifact preview host's limits, NOT kshana.dev's). After the Studio swap the site is at 217 files and 44.7 MB: the Studio's recordings ship as 34 packs in the preview build (`tools_bundle.mjs --to`, read by `lib/packs.mjs`) and as one file each on kshana.dev.
- The display matrix: `node explorations/2026-09-27-audit/matrix/run_matrix.mjs --quick` (per page: see its README). A `--full` run takes about 6.4 h (101,665 runs); its baseline and reports are under `matrix/out/`.
- Screenshots via Playwright from the `matrix/` folder (node_modules is there). Under heavy load, full-page screenshots time out; screenshot elements or small test pages instead.

## 5. Site design state (what the founder approved)

**Design base.** The founder rejected the round-3 combined look ("worse … hero is so boring"). They chose ONE base: Observatory, with liked parts from Spectrum and Aurora rebuilt inside it. Rule: keep round-2 layouts, motion and density; feed real data into the same animated component; never replace a visual with a static card.
- **Home: APPROVED** ("I lvoe the site").
  - Hero: console status bar, real orbit globe (102 real GPS/Galileo/GLONASS/BeiDou satellites from `constellation-multi-gnss-coverage`) on real Earth imagery, telemetry cards + event log replaying the real campaign `campaign-jam-spoof-holdover-integrity`.
  - Then: the L-band console `#lband` (real spectrum run, 700 px; the time axis reads "now … −50 s"), mission timeline, capability cards, Install + AI assistant side by side (711 px; stepped player of the real recorded kshana-mcp session), spoofing story, Pro band.

**Other pages.**
- Missions, Capabilities, Evidence, Developers, Editions and Docs are rolled out on Observatory and reviewed.
- Heroes with live real-data visuals: Capabilities = solar system; Evidence = verification-matrix grid; Developers = real CLI session replay; Editions = three editions on one engine.
- Capabilities is condensed to 7,749 px with tabs.

**Logo.** The founder's EXACT mark shape (`docs/assets/kshana-mark.svg`, gold PNG inside SVG) as an alpha mask at `combo/assets/brand/kshana-mark-mask.png`.
- It is painted with `--ink` + `--cyan` hub and star via CSS `mask` (`.brand-mark` in `src/site.css`): 38 px in the nav, 44 px in the footer; favicon = the same mask tinted per prefers-color-scheme (data URI in `src/shell.html`, `#` encoded as `%23`).
- The founder REJECTED a vector redraw: never redraw the mark, only recolour or resize it.

**Counts on the site are still old** (63 kinds / 174 rows) until the site data is regenerated from the new engine (phase 2/4/5).

## 6. Work in flight at handover (check these first)

Workflow and agent journals live under `~/.claudemain/projects/-Users-chak/<session-id>/subagents/workflows/<run-id>/journal.jsonl`. If a run died (usage limit), read its journal: completed agents' results are there, and builders commit hourly, so branches hold their work.

| Run | What | What to do next |
|---|---|---|
| CI-fix agent (on integration-bc) | Document public items to ≤ 985 without raising the ceiling; mirror every ci.yml step locally; leftovers (leo-timing-utc `trace = true`; cite open-receiver.com for the generic-uhf preset; move Celeste cadences out of generic `docs/LEO-NAVMSG.md`); full gate | Verify its report; re-run doc coverage + prescreen yourself |
| wf_ca1c86bb-069 | Pro MBSE build + verify in kshana-pro; README redesign; 6-group word-by-word docs audit; integrate → `feat/docs-v29`; site docs regenerated + browser link crawler (fixes the install panel's browser-tab "Registry" → old kshana.dev and "Docs" → Studio); Pro wording grounded in the verified build | Verify each stage; then YOU merge `feat/docs-v29` into integration-bc after the CI-fix agent finishes, and run the full gate |
| wf_0956cbe0-23a | Observatory rollout (Capabilities, Missions, Evidence, Docs) + shared + **full display-matrix audit loop** | Read the audit result; loop until zero issues |
| Missions map agent | "Receiver response" map redesign: 2:1 map, metric selector (in view / PDOP / availability), perceptual legend, seamless canvas, receiver marker | Check the screenshots in `shots/map/` |
| Ledger agent | DONE and parent-checked: ledger scrolls in place; "Rerun it" symmetric | — (the Capabilities capability explorer still has its own "Show all"; apply the same pattern if the founder wants) |

**Known transient at handover (2026-09-29 ~18:10):** `check.py` FAILS (55 pages, broken docs links such as docs/scenarios.html and docs/overview.html) because the site-docs agent in wf_ca1c86bb-069 is regenerating `combo/docs/` from the audited repo docs. That agent must finish with check.py OK. If it died, restore `combo/docs/` from `~/Code/kshana-site-next-snap-2026-09-28-pre-observatory.tgz` or from its backup in `audit/backup-docs/`, then rerun the regeneration.

**Pavana** is handled by a different session. Its handover is `~/Documents/Code/Claudecode/pavana/HANDOVER.md` (plus `~/Documents/Code/Claudecode/_pavana-canopy-research/HANDOVER/`). Never touch the Pavana checkout from Kshana work.

**Open founder questions:**
- The founder asked whether Pro's four "What it builds on" lines are true. They are NOT in public text. The pro-wording stage of wf_ca1c86bb-069 handles this: keep a line only with a public statement behind it.

## 7. Remaining roadmap (critical path)

1. Finish CI-fix → merge `feat/docs-v29` → full gate green on integration-bc (and doc coverage ≤ ceiling).
2. **Phase 2:**
   - merge integration-bc into main locally
   - regenerate everything
   - kshana-mcp tools for the new kinds
   - a fresh real recorded AI session
   - WASM build of the integrated engine
3. **Phase 4 (Studio):**
   - every new capability usable (spectrum, solar system, constellations, campaigns, animation, reports, interop, all LEO)
   - regenerate recordings, and record lunar-llr-datum natively
   - then add "Open in Kshana Studio" links on every new visual (Home `#lband` has none yet: logged in SHARED-REQUESTS-R4.md)
   - site data regenerated so the counts become 75 / 223
4. **Phase 5:** port the site into `~/Code/kshana/web/`, so pages.yml deploys it; the web tests and smoke test pass.
5. **Phase 6:**
   - `run_matrix.mjs --full` in Chrome, WebKit and Firefox + the iOS simulator (iPhone 17 Pro) until zero issues
   - a manual review of every page and Studio feature
   - republish the preview artifact https://claude.ai/artifact/2rsxG7ZAcjhuuzP7nnTWVN (bundle to fit the 255-file cap)
6. **Phase 7, ONLY after the founder's go:**
   - version bump + CHANGELOG (including the nutation revision)
   - tag
   - every channel: crates, PyPI, npm, ghcr, MCP registry, JetBrains
   - watch every GitHub Actions run to green (list the run IDs)
   - channel parity
   - a live check of kshana.dev
   - README/plan page

## 8. Lessons that cost time (avoid repeating)

**Briefing agents**
- **Agents read the founder's LATEST chat message as their whole task** and refused approved work (5 page agents did nothing once). Every brief needs an AUTHORISATION paragraph that quotes the founder's approval and says other chat messages do not limit the task. After any workflow, grep the results for "made no changes" or "doesn't cover".
- **"Adapt the parts I liked" means keep the approved design** and change only what was named. A rebuild on a new shell got rejected.

**Checks that can mislead**
- **Usage limits kill every agent mid-run.** Briefs require an hourly green commit; after a limit, commit any uncommitted work as `wip(<slug>)` before relaunching.
- **A gate only grades what it lists:** gate.sh skipped doc coverage that CI runs. Mirror ci.yml.
- A link that resolves can still be wrong (label vs target). Crawl in a browser and check meaning, not just status.
- `crates.io` and `npm` pages return 404/403 to curl (bot blocking). Use `https://crates.io/api/v1/crates/<name>` and `https://registry.npmjs.org/<name>`.
- A `#` inside a data: URI truncates it (fragment): encode it as `%23`.

**Environment**
- The machine is shared with other sessions (e.g. Pavana jobs) and load averages of 100–300 are common. Keep parallelism modest and expect slow builds.
- The founder reviews on the local server and by screenshots: show before/after, measure heights, and never claim done without looking.

## 9. How to talk to the founder

- Plain words, short.
- Say what was verified, and how; say what is not done.
- Show screenshots or the local URL for any visual change.
- Ask only for decisions that are genuinely theirs: scope, wording of public claims, releases.
