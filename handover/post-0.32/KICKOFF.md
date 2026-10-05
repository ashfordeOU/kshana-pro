# Kickoff prompt — paste into a fresh session

Copy the block below into a new Claude Code session (cloud at claude.ai/code, a programmatic
`create_session` with `source_url=https://github.com/ashfordeOU/kshana-pro`, or a local clone)
to continue the Kshana work. It is deliberately lean — the full source of truth is
`handover/post-0.32/README.md` in this repo, which the new session reads first.

```
You are continuing work on Kshana — a Rust GNSS/PNT simulator by Ashforde OÜ.
Two repos, both on GitHub under owner `ashfordeOU`:
  • kshana      — PUBLIC engine, AGPL-3.0 (crate/CLI/Python/WASM/MCP/JetBrains surfaces)
  • kshana-pro  — PRIVATE proprietary overlay + the durable source of truth (site source,
                  brand, legal, commercial, papers, handovers)

FIRST ACTION — do this before anything else:
  Attach kshana-pro (add_repo owner=ashfordeOU repo=kshana-pro access=push, then
  register_repo_root), and read `handover/post-0.32/README.md` on its `main` branch.
  That file is the complete, current source of truth: state, rules, repo map, how to run
  sessions, the release procedure, and the prioritised remaining-work plan (§7) with steps and
  acceptance criteria. Then tell me the current state and the task you're about to start.

HARD RULES (do not break):
  1. Everything lives in git. Never rely on local-only files; never leave the only copy of
     anything in an ephemeral container, a laptop, or a Claude Doc. Commit and push early.
  2. kshana-pro is the source of truth for anything proprietary or site-source.
  3. NEVER put any individual's or prospect's name in EITHER repo (legal risk). Refer
     generically; counterparty specifics live only in kshana-pro `commercial/` and `legal/`.
  4. NO AI-attribution in commits or PRs to EITHER repo. Public CI (scripts/check-no-
     attribution.sh) fails on `Co-Authored-By: …Claude`, `anthropic`, "Generated with/by
     Claude", or the robot emoji — in file content AND commit history. Commit author must be
     `Ashforde OÜ <ashfordegithub@cbaweja.org>`; plain messages, no trailers.

CURRENT STATE (verify against live repos/registries if anything looks stale):
  • kshana 0.32.0 is RELEASED and live on crates.io, PyPI, npm, ghcr.io, JetBrains, the
    GitHub release, and kshana.dev. Public `main` is clean and green.
  • kshana-pro has drifted: `main` pins engine `=0.29.0`; a stale unmerged branch
    `claude/pro-030` pins `=0.30.0`.

HOW TO OPERATE:
  • Use the mcp__github__* tools for all GitHub ops (no `gh` CLI). Attach the public repo with
    add_repo when you need it. Work on a feature branch → PR → drive CI green → merge.
  • Release = one tag push, then automatic (tag → verify → publish → parity → site); see
    kshana `docs/RELEASING.md`. CI is ~2.5 h; don't hand-publish.
  • Pinned toolchain: Rust 1.93.0, wasm-pack 0.13.1, Python 3.x, Node. Run the repo's own
    gates (cargo fmt/clippy/test, scripts/check-*.sh) before pushing.

TASK: <pick one from §7 of the handover>. Default recommended first task —
  §7-A: bring kshana-pro current — bump the engine pin to `=0.32.0` on a fresh branch off
  pro `main` (Cargo.toml + `cargo update -p kshana`), build/test the Pro crate against 0.32.0,
  regenerate the version-stamped papers with the Pro tooling, run pro's gates, open a PR to pro
  `main`, then delete the stale `claude/pro-030`. Follow §7-A's acceptance criteria.

If anything in the handover conflicts with the live repos, trust the repos and update the
handover doc (`handover/post-0.32/README.md`) so it stays the single source of truth.
```
