# Post-0.32.0 handover — continue the work from anywhere

**Packed:** 2026-10-05, right after `kshana 0.32.0` shipped to every registry.
**Audience:** a fresh Claude Code session (cloud or local) with **no prior chat history**.
**Promise:** read this file top to bottom and you have everything — no founder's Mac, no
local-only files, no lost context. Everything referenced here is in git (this repo or the
public `ashfordeOU/kshana`), or is named with its exact location.

> If you only read one section, read **§2 Golden rules** and **§7 Remaining work** — then
> use the copy-paste kickoff prompt in **§8**.

---

## 1. Where things stand (ground truth as of 2026-10-05)

### Public engine — `ashfordeOU/kshana` (AGPL-3.0)
- **`0.32.0` is RELEASED and live on every channel**: crates.io (`kshana` + `kshana-mcp`),
  PyPI (`pip install kshana`), npm, ghcr.io (`ghcr.io/ashfordeou/kshana-mcp:0.32.0`),
  JetBrains Marketplace, the GitHub Release (`v0.32.0`), and kshana.dev (redeployed). The
  release pipeline's `parity` job confirmed each registry actually serves 0.32.0.
- **What 0.32.0 added:** the GNSS **IQ layer** surfaced on the **CLI** (`kshana iq
  scene|acquire|track|sweep|labfit|frontend|inventory|info|extract|convert|decimate`) and in
  **Python** (`kshana.iq_scene`, `iq_scene_broadcast`, `iq_acquire`, `iq_track`, `iq_labfit`,
  `iq_frontend`, `iq_signals`). Phase A = CLI/Python for scene/acquire/track/sweep/labfit;
  Phase A.2 = propagation channel, SigMF output, front-end DSP, broadcast-ephemeris scenes.
  **Software-only, additive, no transmit, no interference/spoofer waveform synthesis.**
- **`main` is clean**: all feature branches merged and deleted; CI green; no attribution
  trailers in history (the repo's `scripts/check-no-attribution.sh` enforces this).

### Private repo — `ashfordeOU/kshana-pro` (this repo; the durable source of truth)
- **`main` still pins `kshana = "=0.29.0"`** (Cargo.toml line ~57). It has **drifted three
  engine releases behind.**
- A **stale, unmerged** branch `claude/pro-030` pins `=0.30.0` (one "chore: update to kshana
  0.30.0" commit: pin + lockfile + regenerated version-stamped papers + one test tweak — no
  deep code change). **It was never merged and is now superseded** by the plan in §7-A.
- This repo already carries the proprietary material: `legal/` (NDA templates incl.
  `NDA-TEMPLATE-MUTUAL.md`, export-control, DPA, IP policy), `commercial/`
  (partners, deals, offers, pricing, go-live checklist), `papers/`, `assets/`, `docs/`,
  and the prior cloud-workspace handover `handover/open-core-0.30/` (site + Studio source,
  oracle toolchain, git bundles — still valid; see §4 and §6).

### The IQ layer is CLI + Python only (by design)
- It is **NOT** exposed in the browser: the WASM API (`src/wasm.rs` in the public repo)
  exports no `iq_*` functions, and the IQ layer is not a scenario "kind" the Studio runs.
  `iq::cli` is `#[cfg(not(target_arch = "wasm32"))]`.
- The Studio/site run the **model-level cousins** (`tracking-loop` scenario kind,
  `receiver_trust`) — the thing `iq::labfit` fits to — and the site *describes* the IQ layer.
- Browser/WASM + MCP exposure of the IQ layer was **deliberately deferred to Phase B** (§7-B).

---

## 2. Golden rules (these prevent the past confusion — follow them exactly)

1. **Everything lives in git. Nothing depends on a local machine.** There are exactly two
   repos: public `ashfordeOU/kshana` and private `ashfordeOU/kshana-pro`. If something matters
   and is not in one of them, **put it there** before the container dies. Never leave the only
   copy of anything in an ephemeral container's home dir, a Claude Doc, or a laptop.
2. **kshana-pro is the source of truth for everything proprietary or source-of-site:** the
   site/Studio *source* (the public `web/` tree is OUTPUT, built from here), brand/design
   tokens, reports/papers, legal, commercial, oracle toolchain, and these handovers.
3. **Never put an individual's or prospect's name in either repo.** It is a legal risk.
   Refer to counterparties generically ("the prospective receiver/simulator partner"). Their
   real details live in `commercial/partners/` and `commercial/deals/` **only**, and even
   there prefer role/company over personal names. (A personal name once leaked into a public
   docs draft and had to be scrubbed — do not repeat it.)
4. **No AI-authorship attribution in commits or PRs to EITHER repo.** The public repo's CI
   (`scripts/check-no-attribution.sh`) fails the build on `Co-Authored-By: …Claude`,
   `anthropic`, "Generated with/by Claude", or the 🤖 emoji — in file content AND in commit
   messages across full history. Keep commit author = `Ashforde OÜ
   <ashfordegithub@cbaweja.org>`; write plain messages with no trailer. (kshana-pro has no
   such guard yet, but keep the same convention.)
5. **Commit and push early and often.** Containers are reclaimed on inactivity; a half-done
   branch that was never pushed is lost. Push WIP to a branch rather than holding it locally.
6. **The release is one tag push, fully automatic after that.** Do not hand-publish to
   registries. See §5.

---

## 3. Repo map — what lives where

### Public `ashfordeOU/kshana` (clone: `git clone https://github.com/ashfordeOU/kshana`)
- `src/` engine; `src/iq/` the IQ layer (`cli/` is wasm-gated); `src/python.rs` PyO3 bindings;
  `src/wasm.rs` the browser API; `kshana.pyi` the Python type stub (**must** match `python.rs`
  — CI `test-python-bindings` runs `mypy.stubtest`).
- `web/` the **built** kshana.dev + Studio (OUTPUT of `web/tools/port_site.py`; do not hand-edit
  — `web/site.test.mjs` verifies every file's SHA against `web/PORT-MANIFEST.json`).
- `docs/RELEASING.md` the authoritative release procedure; `scripts/check-*.sh` the CI guards;
  `.github/workflows/` `ci.yml` (full ~2.5 h suite: `check` + `coverage`), `auto-tag.yml`,
  `release.yml`.
- `CHANGELOG.md`, `Cargo.toml` (version = single source of truth), `mcp/kshana-mcp/` the MCP
  server, `ide/jetbrains/` the plugin.

### Private `ashfordeOU/kshana-pro` (this repo)
- `handover/` — these handovers. **`handover/open-core-0.30/`** still holds the site/Studio
  **source** (`site-next/`), the **oracle toolchain** setup (`oracles/`), git **bundles**, and
  `restore.sh`. **`handover/post-0.32/`** (this one) is current.
- `legal/` — `NDA-TEMPLATE-MUTUAL.md` (+ one-way), export control, DPA, IP policy, counsel pkg.
- `commercial/` — `partners/`, `deals/`, `offers/`, `PRICING.md`, `GO-LIVE-CHECKLIST.md`,
  `payment-details.toml`, `counsel/`, `export-control/`.
- `papers/` — the research papers (reports/figures carry an engine-version stamp → regenerated
  on each engine bump, see §7-A).
- `src/`, `tests/`, `scenarios/`, `Cargo.toml` — the Pro overlay crate (depends on the public
  `kshana` crate as a pinned library; never forks it).
- `assets/`, `data/`, `deploy/`, `docs/` (incl. `docs/plans/`), `Dockerfile`.

### Not in git (must be re-materialized if needed)
- Claude Docs artifacts created in chat (e.g. a mutual-NDA working doc, the 0.32.0 plan doc)
  live on claude.ai, **not** in git. If any is canonical, copy its content into this repo
  (`legal/` or `docs/plans/`). See §7-C for the NDA reconciliation task.

---

## 4. The prior handover is still valid — reuse it for the site/Studio source

`handover/open-core-0.30/README.md` + `restore.sh` reconstruct the founder's `~/Code/...`
workspace **from git** (no Mac needed):

```
git clone -b handover/open-core-0.30 https://github.com/ashfordeOU/kshana-pro.git ~/Code/kshana-pro
WITH_DATA=1 bash ~/Code/kshana-pro/handover/open-core-0.30/restore.sh
```

It restores: the public repo clone, the site source (`~/Code/kshana-site-next`, incl. the
Studio rebuild and the kshana.dev `build.py`/`check.py`), and the oracle toolchain
(`~/Code/kshana-oracles`). **The site `web/` tree in the public repo is produced from this
source** via `web/tools/port_site.py --site <built site> --studio <studio folder>`. You need
this when a task regenerates the site (e.g. §7-D full re-render).

---

## 5. How to cut a release (public repo) — summary; `docs/RELEASING.md` is authoritative

A release is **one tag push**; everything after is automatic:
`tag → verify → publish → parity → site`.

1. In the public repo, on a branch off `main`:
   - `CHANGELOG.md`: rename `[Unreleased]` → `[X.Y.Z] - YYYY-MM-DD`, add a fresh `[Unreleased]`.
   - Bump `Cargo.toml` **and every surface `scripts/check-version-sync.sh` lists**: `Cargo.lock`,
     `mcp/kshana-mcp/Cargo.toml` (version + its `kshana` dep major.minor) and `Cargo.lock`,
     `ide/jetbrains/gradle.properties` + a new `plugin.xml` change-notes entry, README status
     line + release badge + install pins, `README.crates.md`/`README.npm.md`/`README.pypi.md`
     badges, citation metadata (`CITATION.cff` version+date, `codemeta.json`, `.zenodo.json`),
     and the site strings in `web/index.html`, `web/studio/channels.json`,
     `web/PORT-MANIFEST.json` (recompute the SHAs of any `web/` file you touch —
     `web/site.test.mjs` checks them). Run `scripts/check-version-sync.sh` until it says OK.
   - Regenerate the three README demo charts (`docs/assets/{clock-holdover,
     inertial-deadreckoning,orbit-gnss-challenged}.svg`): `cargo run -- scenarios/<name>.toml`
     and copy the emitted `.chart.svg` over each (only the footer version stamp moves;
     `cargo test --test published_figures_still_reproduce` proves it).
2. Merge to `main`. Wait for CI green (the `check` job alone runs ~2.5 h; `coverage` too).
3. **Auto-tag** pushes `vX.Y.Z` on the green commit → **`release.yml`** verifies again, builds,
   publishes all registries, polls `parity`, deploys the site. Monitor the run; if a publish
   job flakes, **Re-run failed jobs** on that run (don't re-tag).

**Gotchas learned the hard way:** the `iq::cli` module must stay wasm-gated; `kshana.pyi` must
match `src/python.rs` (stubtest); the `web/` version strings are gated by `check-version-sync`
but deep `web/` pages keep accurate run-provenance stamps (don't blind-replace — a
`site/refresh-X` branch-name string and changelog refs live in there too).

---

## 6. How to run a NEW container / session to do this work

### Option A — Claude Code on the web (recommended; what this work used)
1. Go to **claude.ai/code**, pick the environment (or create one). The environment fixes the
   network policy, env vars, setup script, and which repos/connectors it can reach. For docs
   on any of this, the in-session `read_documentation` tool has pages
   (`github.access`, `environment.network`, `environment.secrets`, `environment.setup_script`,
   `session.resources`, …).
2. Start a session **on the repo you'll work in** (public `kshana` for engine/release work;
   this repo for Pro work). To pull in the *other* repo mid-session, use the `add_repo`
   tool (owner `ashfordeOU`, repo `kshana` or `kshana-pro`, `access: "push"`), then
   `register_repo_root` so its CLAUDE.md/skills load. GitHub ops go through the
   `mcp__github__*` tools (no `gh` CLI).
3. Develop on a feature branch, commit (clean author, no attribution), push, open a PR with
   the `mcp__github__create_pull_request` tool, drive CI green, merge.

### Option B — spawn a session programmatically (from inside a session)
Use the `mcp__claude-code-remote__create_session` tool: pass `source_url`
(`https://github.com/ashfordeOU/kshana-pro` or the public repo), optional `source_revision`,
a `prompt` (paste §8), and `model`. It returns a session id you can `send_message` to and
`get_session`/`list_events` to watch. Scheduled follow-ups: `create_trigger` / `send_later`.

### Option C — local machine
```
git clone https://github.com/ashfordeOU/kshana.git          # public engine
git clone https://github.com/ashfordeOU/kshana-pro.git      # this repo
```
Toolchain is pinned: **Rust 1.93.0** (`rust-toolchain.toml`), **wasm-pack 0.13.1**,
Python 3.x for the wheel/stub checks, Node for the `web/*.test.mjs` suite. For the oracle
toolchain (Orekit/RTKLIB/NaveGo/SPICE) see `handover/open-core-0.30/oracles/README.md`.
Run the repo's own gates before pushing: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test --all`, and the `scripts/check-*.sh`.

**Whichever option:** obey §2. Push early; keep the only copy in git; no names; no attribution.

---

## 7. Remaining work — the plan (prioritized, each with acceptance criteria)

### A. Bring kshana-pro current: engine pin → `=0.32.0`  *(do first; unblocks everything)*
**Why:** pro `main` is on `=0.29.0`; the engine is at `0.32.0`. Engine changes have been
additive, so expect a clean "refresh", not a port.
**Steps (fresh branch off pro `main`, supersedes the stale `claude/pro-030`):**
1. `Cargo.toml`: `kshana = "=0.32.0"`; `cargo update -p kshana` (crates.io serves 0.32.0).
2. `cargo build` / `cargo test` the Pro crate against 0.32.0; fix any (expected: none, or
   trivial) API drift. The `=0.30.0` branch also tweaked `tests/campaign_watch.rs` — re-check.
3. Regenerate the version-stamped papers/reports/figures under `papers/` with the Pro tooling
   (same kind of change the `=0.30.0` commit made — reports show `engine 0.3x.0`). Do NOT
   hand-edit stamps; run the generators.
4. Run pro's gates (`.github/workflows/ci.yml` locally where possible).
5. PR to pro `main`; merge; then delete `claude/pro-030`.
**Done when:** pro `main` builds+tests on `=0.32.0`, papers regenerated, PR merged, stale
branch deleted.

### B. Phase B for the IQ layer *(the real deferred engineering; public repo)*
Prioritize by audience value:
1. **MCP exposure** (highest value) — add `kshana iq` capabilities to `mcp/kshana-mcp` so an
   agent can drive scene/acquire/track/frontend. Design a file-path/size contract (IQ works on
   large files — don't stream GB through the protocol). Round-trip tests in
   `mcp/kshana-mcp/tests/`.
2. **Promote the IQ layer to the validation ledger** — wire a bit-exact cross-check vs
   `gps-sdr-sim` (and other reference receivers) into CI and add Validated rows. Credibility
   for a serious receiver partner. (Today the IQ additions are `pub(crate)` with no new ledger
   rows.)
3. **WASM/Studio IQ demo** (optional, marketing) — `iq::scene/acq/frontend/signals` already
   compile for wasm; add `#[wasm_bindgen]` wrappers + a short-synthetic-scene Studio view.
   De-prioritized: the browser can't carry real GB captures.
- JetBrains IQ awareness: low value; skip unless asked.

### C. Prospective-partner deliverables *(cheap, high value; keep OUT of the public repo)*
1. **Validation summary against released 0.32.0** — a one-page, name-free summary of what the
   IQ layer does and its validation basis. Store in this repo (`commercial/partners/` or
   `papers/`), **never** in public `kshana`. (The public release already attaches a generic
   `kshana-validation-summary.html`.)
2. **A Python/notebook worked example** — "load an IQ recording → `iq_acquire` → `iq_track` →
   plot", using the published wheel. Store here or as a gist; it turns the wheel into day-one
   value.
3. **NDA reconciliation** — the canonical mutual NDA is `legal/NDA-TEMPLATE-MUTUAL.md`. A
   polished branded mutual-NDA draft + a `python-docx` builder (`.docx` with the Ashforde
   wordmark, superscript ®, CONFIDENTIAL watermark) were produced in an earlier chat as
   **local/Claude-Doc artifacts and are NOT in git**. If that polished version is preferred,
   recreate it into `legal/` (doc + builder script) so it is not lost. Outreach waits on the
   partner's reply + a signed NDA; drafts are in `commercial/partners/OUTREACH-DRAFTS.md`.

### D. Repo hygiene
1. **Full site re-render at 0.32.0** (public, optional/cosmetic) — deep kshana.dev pages still
   carry accurate `0.31.0`/earlier run-provenance stamps while the home page says 0.32.0. A
   clean re-port from the site source (§4) at the 0.32.0 tree makes the whole site consistent.
   Not functional; do only if the inconsistency matters.
2. **File the macOS-local test-failures issue** (public) — ~11 external-oracle comparisons
   (Orekit/mpmath/numpy/SPICE/lunar frame packs) fail only on a local macOS checkout; CI
   macOS is green. Open a tracking issue so it isn't rediscovered each release.

---

## 8. Copy-paste kickoff prompt for a fresh session

> You are continuing work on Kshana (public `ashfordeOU/kshana`, AGPL-3.0) and the private
> `ashfordeOU/kshana-pro`. **First action: read `handover/post-0.32/README.md` in
> kshana-pro** — it is the complete, current source of truth. Then confirm back to me the
> current state and the task you're about to start.
>
> Hard rules: everything lives in git (never rely on local-only files); kshana-pro is the
> durable source of truth; **never put any individual's or prospect's name in either repo**;
> **no AI-attribution trailers** in commits/PRs (public CI `check-no-attribution.sh` enforces
> it; author = `Ashforde OÜ <ashfordegithub@cbaweja.org>`); commit and push early.
>
> Attach both repos with `add_repo` (+`register_repo_root`) as needed; use the `mcp__github__*`
> tools for GitHub.
>
> Task: <pick one from §7 — e.g. "A: bump kshana-pro to engine =0.32.0 (refresh + papers + PR,
> supersede claude/pro-030)">. Follow that section's steps and acceptance criteria. If anything
> in the handover looks stale, re-verify against the live repos/registries before acting.

---

## 9. One-glance status table

| Area | State | Next |
|---|---|---|
| kshana 0.32.0 release | ✅ live on all registries + site | — |
| Public `main` hygiene | ✅ clean, green, no attribution | keep it so |
| kshana-pro engine pin | ⚠️ `main`=0.29.0; stale `claude/pro-030`=0.30.0 | §7-A → `=0.32.0` |
| IQ layer surfaces | CLI + Python only | §7-B (MCP, ledger, optional WASM) |
| Partner deliverables | NDA template in `legal/`; outreach drafts in `commercial/` | §7-C (summary, example, NDA reconcile) |
| Site full re-render @0.32 | deep pages carry older provenance stamps | §7-D1 (optional) |
| macOS-local test fails | CI green; local-only | §7-D2 (file issue) |

*Keep this file updated as the single source of truth. When a §7 item lands, tick it here.*
