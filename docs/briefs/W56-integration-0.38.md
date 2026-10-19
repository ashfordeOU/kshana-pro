# W56 re-task: Kshana 0.38.0 integration helper

Owner's words (18 Oct 2026): "yes launch the workers and deliver all of it in 0.38", then "all of this delivers in 0.38 / all of it. not in 0.39 or 0.40. everything delivers in 0.38". Coordinator decided to use you for integration because all 8 sessions are in use.
Rules: docs/briefs/_COMMON-0.38.md still apply. Definition of done: docs/plans/0.38-DEFINITION-OF-DONE.md (template from 0.37; read docs/HANDOVER.md 0.37 lines for how W52 integrated).

## Branch you own (new)
- kshana: integ/0.38.0, created from origin/main (c0c77700). Merge commits only, never rebase, never force-push.
  Do NOT push main or release/*. The coordinator opens the CI-only draft PR; the owner pushes main and the tag.

## Order
1. Merge, in this order (merge commits, keep-both on match-arm conflicts, resolve carefully):
   feat/timing-standards-0.38 (760fc54d or newer), feat/referee-0.38 (91c4427b or newer), feat/machines-0.38 (808dca4e or newer),
   feat/studio-replay-0.38 (d6c29829 or newer). After each merge: cargo check + the touched tests.
2. Later, when the coordinator says they are final: feat/bench-observatory-0.38 (W57) and feat/lab-lunar-standards-0.38 (W58).
3. After all six: ONE release commit: version bump to 0.38.0 everywhere version-sync checks, CHANGELOG [0.38.0] (merge every
   [Unreleased] entry), README "What's new in 0.38" (statuses from the matrix only; NOT MET rows named honestly), CITATION.cff,
   then regenerate with the repo tools (gen_validation_artifacts, figure scripts, og-card, field-units schema, MCP install
   links via gen_mcp_install.py), never hand-edit counts. Run the pin-bearing tests right after the bump (lesson from 0.36).
4. wasm package at 0.38.0; Studio re-record at one engine commit (replaces the interim optical and holdover recordings);
   port the site from kshana-pro site/studio-0.38 (latest) with web/tools/port_site.py; check_features; screenshots of
   home, capabilities, evidence, developers, /benchmark, replay, optical, holdover at 1440 and 390, light and dark, listed
   for coordinator review.
5. Full local suite once: cargo test --release --all --no-fail-fast, MCP round_trip, wasm, python feature, check-no-attribution,
   doc-coverage, version-sync, cargo deny. Fix what fails (a branch's worker is idle and can be asked through the coordinator).
6. Push integ/0.38.0 and report SHA + the definition-of-done table with evidence.
Disk: delete target dirs between steps if space runs low. Usage: at most one subagent; no repeated full-suite runs.
