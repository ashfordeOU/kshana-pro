# Kshana 0.29.0: release runbook

Written 2026-09-30 by the release-preparation agent. Everything here is prepared and
dry-run; **nothing has been pushed, tagged or published.** The founder's rule stands: no
push, no tag, no publish until the founder's final go.

Read `HANDOVER.md` section 2 (hard rules) first. This file is the ordered path from
"the founder says go" to "every channel live and verified".

Short version: nothing is red, no secret is missing, and the release is **not** ready to
press yet: section 9c lists what blocks it (the engine and the new site are not on `main`,
and the final commit has not been through the full gate). After the go it takes about 5 to
7 hours (section 9b).

**Where things are**

| Thing | Path |
| --- | --- |
| Release branch | `feat/release-prep`, worktree `~/Code/kshana-wt-release` (from `feat/docs-v29` acd1c51) |
| Main checkout (push from here: it holds the push hooks' contract) | `~/Code/kshana`, branch `main`, 2 commits ahead of `origin/main` 479d308 |
| Scratch (logs, scripts, artefacts) | `~/Code/kshana-site-next/scratch/release/` |
| Dry-run results table | `scratch/release/dryrun-results.tsv`, logs in `scratch/release/logs/` |
| Tag message, ready | `scratch/release/TAG_MESSAGE.txt` |
| Release notes as the workflow will write them | `scratch/release/RELEASE_NOTES.md` (from `scripts/changelog-extract.sh 0.29.0`) |
| Bump script (the repo has none) | `scratch/release/bump_version.py` |
| Changelog assembly script | `scratch/release/build_changelog.py` |

Git identity for every commit and the tag:
`git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com …`
No trailers. Never `--no-verify`. Never `git stash`, `reset`, `clean` or `checkout -- <file>`.

---

## 1. The version, and why

**0.29.0.**

- The README ("Versioning & releases") and the CHANGELOG header say the project follows
  Semantic Versioning, and that before 1.0 the scenario and result schema may still
  change, with breaking changes called out.
- The last release is 0.28.0 (tag `v0.28.0`; crates.io, npm and PyPI all serve 0.28.0,
  checked through their APIs on 2026-09-30).
- This release adds twelve scenario kinds and new public API, so it is not a patch.
  It also changes one engine output (the `ephemeris` kind's GCRS columns) and adds a
  variant to a public enum (`navsignal::Modulation::Mboc`). Before 1.0, Semantic
  Versioning carries new features and breaking changes in the minor number. So: 0.29.0.
- `docs/RELEASING.md` names no number; it says to bump `Cargo.toml` and every surface
  `scripts/check-version-sync.sh` lists, which is done.

## 2. What is already done (on `feat/release-prep`)

- Version 0.29.0 on every pinned surface; `scripts/check-version-sync.sh` says OK, also
  with `GITHUB_REF_NAME=v0.29.0`, and fails with a wrong tag (control run).
- CHANGELOG: `[Unreleased]` became `[0.29.0] - 2026-09-30` with a fresh empty
  `[Unreleased]` above it (a test needs that heading to exist). The entry has a summary
  table, an abbreviations block, a "Revisions to published numbers" section (the
  nutation fix and every figure the documentation audits corrected), and the new
  capabilities grouped by area.
- The README images were regenerated from a 0.29.0 build
  (`tools/gen_readme_assets.py --check` exits 0).
- Dry runs: see section 9.

**The date is pinned in two places.** If the tag is not pushed on 2026-09-30, change
both before tagging: the heading `## [0.29.0] - 2026-09-30` in `CHANGELOG.md`, and
`date-released` in `CITATION.cff`.

## 3. Before the founder is asked for the go

These are owned by other agents or the parent. Each must be true before step G1.

| # | Condition | How to check |
| --- | --- | --- |
| P1 | `feat/integration-bc` has passed its full gate | the CI-fix agent's report; re-run `scripts/check-doc-coverage.sh` (expect `985 (ceiling: 985)`) and the prescreen yourself |
| P2 | `feat/mcp-newkinds` (c2510ee at writing) is verified and merged into `feat/integration-bc` | `git -C ~/Code/kshana log --oneline feat/integration-bc..feat/mcp-newkinds` prints nothing |
| P3 | `feat/wasm-parity` is verified and merged, or the founder accepts the known gap (42 of 137 scenarios byte-identical in the browser build at the time of writing) | the wasm-parity agent's report |
| P4 | The new site is ported into `~/Code/kshana/web/` (phase 5). **`pages.yml` builds kshana.dev from `web/` in the tag with `web/build.sh`. If the port is not in the tag, the release redeploys the OLD site with the new engine.** | `ls ~/Code/kshana/web`; the web tests; see the two traps in section 10 |
| P5 | Any public Pro text (`feat/pro-public-text`) is approved by the founder and merged | the parent |
| P6 | Phase 6 audit passed to the level the founder accepts for tonight | `GOAL.md` |
| P7 | Founder decisions in section 11 are answered | the founder |

## 4. Order of merges

```
feat/integration-bc  (after its gate; contains feat/docs-v29, then mcp-newkinds, wasm-parity, the web port)
        │  git merge --no-ff
        ▼
      main  (local, ~/Code/kshana)
        ▲
        │  rebase onto main, then fast-forward merge
feat/release-prep  (the version bump and the CHANGELOG entry: always LAST)
```

The release-prep commit goes last because every other branch still writes into
`[Unreleased]` and still carries 0.28.0.

---

## 5. From "go" to "live": the steps

Run every command from `~/Code/kshana` unless the step says otherwise. Use
`CARGO_BUILD_JOBS=3` while the machine is shared.

### G0. Freeze and look

```bash
cd ~/Code/kshana
git status --short                       # expect: nothing (untracked .ci-native etc. are excluded)
git fetch origin
git rev-parse origin/main                # expect: 479d3080afa9311f4b47e45dfd1a4df7afb335f6
git log --oneline origin/main..main      # expect: the 2 local commits (aab844f, 4ca95f5)
git remote -v                            # expect: ONLY origin = github.com/ashfordeOU/kshana (no private remote)
git branch backup/main-pre-0.29.0 main   # a name for the old tip, so a rollback needs no reset
```

- Verify: `origin/main` has not moved. If it has, stop and read what landed.
- Rollback: nothing to roll back. Delete the backup branch at the very end (step G12).

### G1. Merge `feat/integration-bc` into `main` (local)

```bash
cd ~/Code/kshana
git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com \
    merge --no-ff feat/integration-bc \
    -m "Merge feat/integration-bc: spectrum, solar system, constellations, campaigns, animation, reports, exports and low-Earth-orbit navigation"
```

- Expect: a clean merge (main's two commits are already inside integration-bc through
  `feat/integration-a` df0329d).
- Verify: `git log --oneline -1`; `git diff --stat feat/integration-bc main` prints nothing.
- Rollback (not pushed yet): `git revert -m 1 <merge sha>`.

### G2. Put the release commit on top

```bash
cd ~/Code/kshana-wt-release
git rebase main
```

- If it applies cleanly, continue at "After the rebase".
- **If it conflicts** (likely in `mcp/kshana-mcp/Cargo.toml`, `Cargo.lock` files,
  `CHANGELOG.md`, the README or `web/index.html`): run `git rebase --abort`, then redo
  the bump on a fresh branch instead of resolving by hand:

```bash
cd ~/Code/kshana
git worktree add ~/Code/kshana-wt-release2 -b feat/release-prep-2 main
cd ~/Code/kshana-wt-release2
/opt/homebrew/bin/python3 ~/Code/kshana-site-next/scratch/release/bump_version.py 0.28.0 0.29.0 <YYYY-MM-DD>
# CHANGELOG: the reference copy of the [0.29.0] entry is feat/release-prep's file
# (also saved as ~/Code/kshana-site-next/scratch/release/CHANGELOG-reference.md):
git show feat/release-prep:CHANGELOG.md | sed -n '/^## \[0.29.0\]/,/^## \[0.28.0\]/p' | sed '$d' > ~/Code/kshana-site-next/scratch/release/entry-0.29.0.md
git diff acd1c51 main -- CHANGELOG.md        # what main's [Unreleased] gained since the entry was written
# In CHANGELOG.md: replace everything from "## [Unreleased]" up to "## [0.28.0]" with
#   "## [Unreleased]" + a blank line + entry-0.29.0.md,
# then move each bullet the diff above shows into the entry by hand (new MCP tools under
# "### Added"; any changed published number under "### Revisions to published numbers"),
# and add the two link lines at the foot of the file:
#   [Unreleased]: https://github.com/AshfordeOU/kshana/compare/v0.29.0...HEAD
#   [0.29.0]: https://github.com/AshfordeOU/kshana/compare/v0.28.0...v0.29.0
```

The same hand-move applies after a clean rebase: **any bullet another branch added to
`[Unreleased]` must end up inside `[0.29.0]`**, or the release notes will miss it and a
test (`the_unreleased_release_notes_state_the_live_matrix_counts`) will read stale counts
later. Check with `sed -n '/^## \[Unreleased\]/,/^## \[0.29.0\]/p' CHANGELOG.md`: it must
print the two headings and nothing between them.

**After the rebase (or the redo), always:**

```bash
cd ~/Code/kshana-wt-release            # or -release2
CARGO_BUILD_JOBS=3 cargo build --release --locked --bin kshana
target/release/kshana --version                            # expect: kshana 0.29.0
target/release/kshana kinds --json | /opt/homebrew/bin/python3 -c 'import sys,json; print(len(json.load(sys.stdin)))'   # expect 75; if it differs, the CHANGELOG table and README counts must follow
/opt/homebrew/bin/python3 tools/gen_readme_assets.py        # regenerate; then:
/opt/homebrew/bin/python3 tools/gen_readme_assets.py --check   # expect exit 0
scripts/check-version-sync.sh                               # expect: OK … at v0.29.0
GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v0.29.0 scripts/check-version-sync.sh   # expect: release tag v0.29.0 matches the manifest
grep -rn -F '0.28.0' . --exclude-dir=target --exclude-dir=.git --exclude=CHANGELOG.md --exclude-dir=xval
#   expect ONLY: docs/RELEASING.md (parity table), docs/WHEEL_TAGS.md, docs/SLOT-TIMING.md,
#   docs/REPRODUCIBILITY.md (2), scripts/check-version-sync.sh (2), realdata-clock.yml, src/clock_state.rs
bash scripts/changelog-extract.sh 0.29.0 | wc -c            # expect: under 125000 (GitHub's release-body limit); 56 040 at writing
grep -n '^## \[' CHANGELOG.md | head -3                      # expect: [Unreleased], [0.29.0] - <tag day>, [0.28.0]
```

Commit what changed (identity above, no trailers), then:

```bash
cd ~/Code/kshana
git merge --ff-only feat/release-prep       # or feat/release-prep-2
git log --format='%H %an <%ae>%n%B' origin/main..main | grep -i -E 'co-authored|generated with|claude|anthropic'   # expect: nothing
```

- Rollback (not pushed): `git revert <sha>` for the release commit.

### G3. Gate the exact commit that will be pushed

The pre-push hook (`.git/hooks/pre-push`, contract in `~/Code/kshana/.ci-native`) refuses a
push unless `.gate-receipt.json` names **exactly HEAD on a clean tree**. So the gate runs
after the last commit, not before.

```bash
cd ~/Code/kshana
git status --short                                  # expect: nothing
.github/ci-local/run-local-ci.sh                    # the fast tier of the local CI manifest
scripts/check-doc-coverage.sh                       # expect: 985 (ceiling: 985). NOT part of gate.sh, but CI runs it
CARGO_BUILD_JOBS=3 cargo package --locked           # expect: Packaged ~759 files, ~3.9MiB compressed (limit 10 MB)
TEST_THREADS=8 scripts/gate.sh                      # full: cargo test --all, then the repeatability loop (3 runs)
#   under load, the shortcut is:  REPEAT=0 TEST_THREADS=8 scripts/gate.sh   (then the push needs KSHANA_MIN_REPEAT_RUNS=0)
cat .gate-receipt.json                              # expect: "commit" = git rev-parse HEAD, "exit_code": 0, "tree": "clean"
```

- Expect: about 65 minutes on a quiet machine for `cargo test --all`; 2 to 3 hours at a
  load average over 100.
- A failed `cargo test` names only its first failure. If it is red, prescreen: build with
  `cargo test --all --no-run --message-format=json`, run every test executable, record
  the exit code of each (HANDOVER section 4).
- Rollback: none needed; nothing has left the machine.

### G4. Push `main`

```bash
cd ~/Code/kshana
git push origin main                                 # with REPEAT=0 above:  KSHANA_MIN_REPEAT_RUNS=0 git push origin main
```

- Expect: the hook prints its checks, then the push goes through. Branch protection on
  `main` forbids force pushes; it requires no status checks and no reviews.
- Verify: `git ls-remote origin main` equals `git rev-parse main`.
- This starts, on GitHub: **CI** (`ci.yml`), **CodeQL**, and after CI succeeds
  **SonarQube Cloud**. `JOSS draft PDF` starts only if `paper/**` changed.
- Rollback: `git revert <sha…>` and push again. Never force-push.

### G5. Watch CI to green on that commit

```bash
SHA=$(git -C ~/Code/kshana rev-parse main)
gh run list --repo ashfordeOU/kshana --commit "$SHA" --json databaseId,workflowName,status,conclusion \
   --jq '.[] | "\(.databaseId) \(.workflowName) \(.status) \(.conclusion)"'
gh run watch <CI run id> --repo ashfordeOU/kshana --exit-status
```

- Expect 14 CI jobs green: `check`, `package`, `msrv`, `deny`, `coverage`,
  `reproducibility-matrix` ×3, `test-python-bindings`, `test-wasm-bindings`,
  `public-standalone`, `wasm-registry`, `mcp-check`, `mcp-deny`. Last release: 77 minutes
  (`coverage` is the long one; `check` took 68). Expect longer now: the suite has grown.
- Write down the run IDs (CI, CodeQL, SonarQube Cloud) for the final board.
- **Do not tag until CI is green on exactly this commit** (`docs/RELEASING.md`).
- If red: fix on a branch, merge, re-gate (G3), push, watch again. Nothing is published yet.

### G6. Tag and push the tag — the point of no return is inside this run

```bash
cd ~/Code/kshana
SHA=$(git rev-parse main)
git -c user.name=ashfordeOU -c user.email=236818772+ashfordeOU@users.noreply.github.com \
    tag -a v0.29.0 "$SHA" -F ~/Code/kshana-site-next/scratch/release/TAG_MESSAGE.txt
git for-each-ref refs/tags/v0.29.0 --format='%(taggername) %(taggeremail) | %(contents:subject)'   # expect: ashfordeOU … | v0.29.0 — …
git push origin v0.29.0                              # with a REPEAT=0 receipt: KSHANA_MIN_REPEAT_RUNS=0 git push origin v0.29.0
```

- HEAD must still be the gated commit with a clean tree, or the hook refuses the tag push too.
- Expect: one **Release** run starts (`release.yml`, event `push`, ref `v0.29.0`).
- Rollback **while `verify` is still running or has failed** (nothing is published until
  `verify` is green): `git push origin :refs/tags/v0.29.0 && git tag -d v0.29.0`, fix, re-tag.
- Rollback after `publish / crates.io` has run: see section 7. The version number is spent.

### G7. Watch the Release run

```bash
gh run list --repo ashfordeOU/kshana --workflow release.yml --limit 1
gh run watch <Release run id> --repo ashfordeOU/kshana --exit-status
gh run view  <Release run id> --repo ashfordeOU/kshana --json jobs --jq '.jobs[] | "\(.conclusion // .status)  \(.name)"'
```

Order and the last release's timings (v0.28.0, run 36332724800, 1 h 58 min in all):

| Stage | Jobs | v0.28.0 took | If it fails |
| --- | --- | --- | --- |
| verify | `verify`; `CLI binary` ×3 in parallel | 83 min | nothing is published; delete the tag, fix, re-tag |
| GitHub Release | `release`, `verify-release`, `verify-release` ×2 | 5 min | registries unaffected; re-run failed jobs |
| build | `publish / pre-publish gates`, `publish / wheels` ×6, `wheel rebuild-and-diff`, `pip install on ARM64 Linux (smoke)`, `publish / npm package (build)` | 4 min | nothing is published yet; re-run failed jobs |
| crates.io | `publish / crates.io` | 1 min | **first irreversible upload** |
| registries | `publish / PyPI`, `publish / npm`, `publish / crates.io (kshana-mcp)` | 2 min | re-run failed jobs (every publish is idempotent) |
| image, plugin | `mcp-publish / OCI image (ghcr.io)`, `jetbrains / build`, `jetbrains / Publish to JetBrains Marketplace` | 27 min (the image) | re-run failed jobs |
| MCP registry | `mcp-publish / MCP registry (OIDC)` | under 1 min | re-run failed jobs |
| parity | `channel parity` (polls for up to 45 min) | under 1 min | the run is red and names the missing channel |
| site | `deploy kshana.dev` dispatches `pages.yml` on `main` with `tag=v0.29.0` | under 1 min | dispatch `pages` by hand (G9) |

Retry rule (`docs/RELEASING.md`): use **Re-run failed jobs** on this run. Do not dispatch
`release.yml` by hand for a retry: that path rebuilds the GitHub Release and publishes nothing.

### G8. Verify every channel (after `channel parity` is green, check again yourself)

```bash
cd ~/Code/kshana
/opt/homebrew/bin/python3 scripts/check_channel_parity.py 0.29.0
#   expect 5 required "OK" lines (crates.io kshana, crates.io kshana-mcp, npm, PyPI with 7 files, ghcr.io),
#   and docs.rs + MCP registry as best-effort; final line "OK: every required channel serves 0.29.0."
```

One by one (the registries' web pages block `curl`; use their APIs):

| Channel | Command | Expect |
| --- | --- | --- |
| crates.io `kshana` | `curl -s -A 'kshana-release-check (contact@ashforde.org)' https://crates.io/api/v1/crates/kshana/0.29.0 \| jq '.version \| {num, yanked, crate_size}'` | `num` 0.29.0, `yanked` false, `crate_size` about 4 083 000 |
| crates.io `kshana-mcp` | same URL with `kshana-mcp/0.29.0` | `num` 0.29.0, `yanked` false |
| npm | `curl -s https://registry.npmjs.org/kshana \| jq '."dist-tags", .versions."0.29.0".dist.tarball'` | `latest` 0.29.0 and a tarball URL |
| PyPI | `curl -s https://pypi.org/pypi/kshana/0.29.0/json \| jq '[.urls[].filename]'` | 7 files: 6 wheels (Linux x86_64 and aarch64 `manylinux_2_28`, macOS arm64 and x86_64, Windows amd64 and arm64) and the `.tar.gz` |
| ghcr.io | `TOKEN=$(curl -s "https://ghcr.io/token?scope=repository:ashfordeou/kshana-mcp:pull" \| jq -r .token); curl -s -o /dev/null -w '%{http_code}\n' -H "Authorization: Bearer $TOKEN" -H 'Accept: application/vnd.oci.image.index.v1+json' https://ghcr.io/v2/ashfordeou/kshana-mcp/manifests/0.29.0` | 200 |
| MCP registry | `curl -s 'https://registry.modelcontextprotocol.io/v0/servers/io.github.ashfordeOU%2Fkshana-mcp/versions/0.29.0' \| jq '.server.version // .version'` | 0.29.0 |
| JetBrains Marketplace (not in the parity script) | `curl -s 'https://plugins.jetbrains.com/api/plugins/32181/updates?size=2' \| jq '.[] \| {version, approve, listed}'` | first entry `version` 0.29.0; `approve` can lag while JetBrains reviews |
| GitHub Release | `gh release view v0.29.0 --repo ashfordeOU/kshana --json assets,isDraft,isPrerelease --jq '{isDraft, isPrerelease, assets: [.assets[].name]}'` | not a draft; 8 assets: `kshana`, `kshana-aarch64-apple-darwin`, `kshana-x86_64-apple-darwin`, `kshana-x86_64-pc-windows-msvc.exe`, `kshana-mcp`, `kshana-sbom.cdx.json`, `kshana-validation-summary.html`, `SHA256SUMS` |
| Provenance | `gh release download v0.29.0 --repo ashfordeOU/kshana -p SHA256SUMS -p kshana-aarch64-apple-darwin -D /tmp/k029 && (cd /tmp/k029 && shasum -a 256 -c SHA256SUMS --ignore-missing && gh attestation verify kshana-aarch64-apple-darwin --repo ashfordeOU/kshana && chmod +x kshana-aarch64-apple-darwin && ./kshana-aarch64-apple-darwin --version)` | checksum OK, attestation verified, `kshana 0.29.0` |
| A real install | `cargo install kshana --version 0.29.0 --locked --root /tmp/k029-cargo && /tmp/k029-cargo/bin/kshana --version`; `python3 -m venv /tmp/k029-py && /tmp/k029-py/bin/pip install kshana==0.29.0 && /tmp/k029-py/bin/python -c 'import kshana; print(kshana.version())'`; `npm view kshana@0.29.0 version` | 0.29.0 three times |
| docs.rs (best-effort, can take hours) | `curl -s https://docs.rs/crate/kshana/0.29.0/status.json` | `"doc_status":true` |

All of these were run against 0.28.0 on 2026-09-30 and answered as the "Expect" column
describes, so the commands themselves are known to work.

### G9. kshana.dev

The `deploy kshana.dev` job dispatches `pages.yml`. Find and watch that run:

```bash
gh run list --repo ashfordeOU/kshana --workflow pages.yml --limit 1
gh run watch <pages run id> --repo ashfordeOU/kshana --exit-status
```

Live check (allow a few minutes for the content delivery network):

```bash
curl -s https://kshana.dev/ | grep -o -E '"softwareVersion": "[^"]+"|id="ver-chip">[^<]+'   # expect 0.29.0 and v0.29.0
curl -s https://kshana.dev/pkg/package.json | grep '"version"'                               # expect 0.29.0
curl -s -o /dev/null -w '%{http_code} %{size_download}\n' https://kshana.dev/pkg/kshana_bg.wasm   # expect 200 and several megabytes
for p in build.sh smoke.mjs README.md share.test.mjs; do curl -s -o /dev/null -w "$p %{http_code}\n" https://kshana.dev/$p; done   # expect 404 four times
```

Then open every page in a browser and run one scenario in Kshana Studio (it must report
engine 0.29.0), and run the link crawler and the display matrix `--quick` against the live
address. The exact grep strings above are what the current `web/index.html` carries; if
the ported site marks its version differently, check the same two facts by eye.

- Manual dispatch, if the job did not run:
  `gh workflow run pages.yml --repo ashfordeOU/kshana --ref main -f tag=v0.29.0`
- **Rollback of the site** (the same operation, with the previous tag):
  `gh workflow run pages.yml --repo ashfordeOU/kshana --ref main -f tag=v0.28.0`

### G10. After the release: the documents that state what the registries serve

On a branch from `main`, one commit (identity above), then gate and push as in G3 and G4:

- `docs/RELEASING.md`: add `0.29.0` to the parity table row that reads "nothing", once
  `check_channel_parity.py 0.29.0` has said so.
- `docs/WHEEL_TAGS.md` line 8 ("the six wheels PyPI serves for 0.28.0") and
  `docs/REPRODUCIBILITY.md` (the SBOM download example): move to 0.29.0 and run the
  example command once.
- `GOAL.md` phase 7 and `HANDOVER.md`: record the run IDs (CI, CodeQL, SonarQube Cloud,
  Release, pages) and the channel results.
- The plan page and the README link targets, as `GOAL.md` phase 7 lists.

This push goes to `main` only. It does not change the tag or the published artefacts, and
kshana.dev keeps showing the tag's content until the next release.

### G11. Owed step: pin the published engine in kshana-pro

`~/Code/kshana-pro/Cargo.toml` line 59 builds against a local engine worktree:

```toml
kshana = { path = "../kshana-wt-pro-engine", version = "0.28" }
```

After `crates.io kshana 0.29.0` is live (G8), in `~/Code/kshana-pro` on its release branch:

```bash
# 1. replace that line with the published version
#      kshana = "0.29.0"
#    (keep any `features = [...]` the line carries; the file's own comment at lines 51-58 says the same)
cargo update -p kshana
grep -A3 'name = "kshana"' Cargo.lock        # expect: version = "0.29.0", source = "registry+https://github.com/rust-lang/crates.io-index", and a checksum
cargo build --locked && cargo test --locked  # then the repository's whole gate with --locked
```

- Expect: Pro builds and passes against the crate from crates.io, with no path dependency
  left (`grep -n 'path *= *"\.\./kshana' Cargo.toml` prints nothing).
- kshana-pro is private forever: commit there, never copy anything into the public repo.
- **Heads-up before the release too:** as soon as the engine worktree Pro points at
  carries 0.29.0, the requirement `version = "0.28"` no longer matches and Pro stops
  resolving. Until the crate is published, the interim line is
  `kshana = { path = "../kshana-wt-pro-engine", version = "0.29" }`.
- Rollback: restore the path line.

### G12. Clean up

```bash
cd ~/Code/kshana
git branch -d backup/main-pre-0.29.0
git worktree list            # remove merged worktrees with `git worktree remove <path>`; leave ~/Code/kshana-wt-k (aperio-k) alone
```

---

## 6. The runs to watch to green (fill in the IDs)

| When | Workflow | Trigger | Run ID | Result |
| --- | --- | --- | --- | --- |
| G4 | CI | push to `main` | | |
| G4 | CodeQL | push to `main` (default setup) | | |
| G5 | SonarQube Cloud | after CI succeeds on `main` | | |
| G4 | JOSS draft PDF | only if `paper/**` changed | | |
| G6 | Release (includes publish, wheels, MCP server, JetBrains plugin as called workflows) | tag `v0.29.0` | | |
| G9 | pages | dispatched by Release `deploy kshana.dev` | | |
| G10 | CI, CodeQL, SonarQube Cloud again | the follow-up docs push | | |

Not started by a release: `agency-od-validation`, `frame-xval`, `golden-pins`,
`lunar-od-xval`, `release-notes`, `wheels` (dispatch only) and `realdata-clock` (weekly
schedule and dispatch).

## 7. Rollback, per channel

Publishing cannot be undone on most channels. The release pipeline is built so that the
verdict comes first; after that, the honest rollback is a fixed **0.29.1**.

| Channel | Can it be undone? | What to do |
| --- | --- | --- |
| Local merges (G1, G2) | yes | `git revert` (merge: `git revert -m 1 <sha>`); `backup/main-pre-0.29.0` names the old tip |
| `main` on GitHub (G4) | yes, forward only | revert commit, gate, push. Never force-push |
| Tag, before `verify` is green | yes | `git push origin :refs/tags/v0.29.0 && git tag -d v0.29.0` |
| Tag, after any registry upload | no | leave the tag; release 0.29.1 |
| crates.io (`kshana`, `kshana-mcp`) | yank only, never delete; the number cannot be reused | `cargo yank --version 0.29.0 kshana` and `cargo yank --version 0.29.0 kshana-mcp` (founder's token); undo with `cargo yank --undo` |
| PyPI | yank in the project's web settings (pip then skips it unless pinned); a deleted file name can never be uploaded again | yank 0.29.0, release 0.29.1 |
| npm | `npm deprecate kshana@0.29.0 "<reason>"`; `npm unpublish kshana@0.29.0` is allowed only within 72 hours and the number cannot be reused | deprecate, then `npm dist-tag add kshana@0.28.0 latest` so `npm install kshana` returns the previous version |
| ghcr.io image | yes | delete the `0.29.0` version under the package's settings (the owner is a user account, so: `gh api /user/packages/container/kshana-mcp/versions --jq '.[] | "\(.id) \(.metadata.container.tags)"'` to find the id, then `gh api -X DELETE /user/packages/container/kshana-mcp/versions/<id>`; the token needs the `delete:packages` scope, which the current one lacks), and re-point `latest`: `docker buildx imagetools create -t ghcr.io/ashfordeou/kshana-mcp:latest ghcr.io/ashfordeou/kshana-mcp:0.28.0` |
| MCP registry | a version can be marked `deprecated` or `deleted` | `mcp-publisher status --status deprecated --message "<reason>" io.github.ashfordeOU/kshana-mcp 0.29.0` (flags before the names; it needs a registry login, and the release uses the run's GitHub OpenID Connect identity, so do it from a dispatched workflow step or with `mcp-publisher login github`) |
| JetBrains Marketplace | yes | in the vendor portal, hide or remove update 0.29.0 of plugin 32181 |
| GitHub Release | yes | `gh release edit v0.29.0 --repo ashfordeOU/kshana --draft` to hide it, or `gh release delete v0.29.0` (the tag stays) |
| kshana.dev | yes | `gh workflow run pages.yml --repo ashfordeOU/kshana --ref main -f tag=v0.28.0` |
| kshana-pro pin | yes | restore the path dependency line |
| Zenodo | no | if the Zenodo to GitHub link is on, a GitHub Release creates a new version record under the concept DOI 10.5281/zenodo.20528627; a record cannot be deleted, only superseded. Check the record after G8 |

## 8. Local mirror of the workflows

The repository keeps its own mirror list in `~/Code/kshana/.github/ci-local/manifest.yml`
(untracked, main checkout only), run by `.github/ci-local/run-local-ci.sh` (`--full`
adds the multi-hour jobs). The table below is read from the workflow files themselves
on 2026-09-30, step by step. "Here" means this macOS arm64 laptop.

### `ci.yml` (push to `main`, pull requests)

| Job | Step | Local command | Mirrors? |
| --- | --- | --- | --- |
| check | Toolchain pin guard | `./scripts/check-toolchain.sh` | yes |
| check | Format | `cargo fmt --all -- --check` | yes |
| check | Clippy | `cargo clippy --all-targets -- -D warnings` | yes |
| check | Test | `cargo test --all` (through `scripts/gate.sh`, which also writes the push receipt) | yes, on macOS; CI runs it on Linux x86-64, so last-digit differences can only show there |
| check | Reproducibility guard | `./scripts/check-reproducible.sh` | yes |
| check | Repository hygiene guard | `./scripts/check-no-attribution.sh` (needs full history, as CI's `fetch-depth: 0`) | yes |
| check | Version consistency guard | `./scripts/check-version-sync.sh` | yes |
| check | Documentation-coverage ratchet | `./scripts/check-doc-coverage.sh` | yes (**not** run by `scripts/gate.sh`) |
| check | SBOM generation guard | `./scripts/gen-sbom.sh > /dev/null` | yes |
| package | | `cargo package --locked` | yes |
| msrv | assert compiler, build | `RUSTUP_TOOLCHAIN=1.85.0 rustc --version && RUSTUP_TOOLCHAIN=1.85.0 cargo build --all --locked` | yes (toolchain 1.85.0 is installed here) |
| deny | cargo-deny | `cargo deny --all-features check advisories licenses bans sources` | yes (the local manifest omits `--all-features`: use this line) |
| coverage | tarpaulin | `cargo tarpaulin --engine llvm --out Lcov --output-dir coverage --exclude-files 'src/*_data.rs' --exclude-files 'src/main.rs' --exclude-files 'tests/*' --exclude-files 'web/*' --timeout 3600 --fail-under 85` | partly: it runs here, but takes hours and the percentage on macOS is not the Linux one. The verdict that counts is the CI job |
| coverage | Upload lcov report | none | no: uploads a run artefact |
| reproducibility-matrix | 3 operating systems | `cargo test --test cross_platform_golden --test golden --test determinism --test sgp4_verification` | the macOS leg only; Ubuntu and Windows need those hosts |
| test-python-bindings | build, install, smoke, stub check | `maturin build --features python --out dist && python3 -m venv .venv-ci && .venv-ci/bin/pip install pytest numpy mypy && .venv-ci/bin/pip install --no-index --find-links dist kshana && .venv-ci/bin/python -m pytest -q tests/python && (cd /tmp && printf 'kshana\\.kshana\n' > stubtest-allow.txt && ~/Code/kshana/.venv-ci/bin/python -m mypy.stubtest kshana --allowlist stubtest-allow.txt)` | partly: CI builds inside the `manylinux_2_28` container, which needs Docker (down at writing) |
| test-wasm-bindings | build | `wasm-pack build --target web --out-dir web/pkg --release --no-opt -- --features wasm` (wasm-pack 0.13.1 is installed here, the pinned version) | yes |
| test-wasm-bindings | every web test has a step | `diff <(find web -maxdepth 1 -name '*.test.mjs' \| sort) <(grep -oE '^ +run: node web/[A-Za-z0-9_.-]+\.test\.mjs$' .github/workflows/ci.yml \| awk '{print $3}' \| sort -u)` | yes |
| test-wasm-bindings | smoke and 14 test files | `node web/smoke.mjs && for f in web/*.test.mjs; do node "$f" \|\| break; done` | yes (CI uses Node 20; this machine has Node 26) |
| public-standalone | | `cargo build -p kshana && cargo test -p kshana --lib --test conformance --test external_pack --test registry_golden` | yes |
| wasm-registry | | `cargo build -p kshana --lib --target wasm32-unknown-unknown` | yes |
| mcp-check | | `(cd mcp/kshana-mcp && cargo test --locked)` | yes |
| mcp-deny | | `(cd mcp/kshana-mcp && cargo deny check advisories licenses bans sources)` | yes |

Every job also runs `actions/checkout`, `dtolnay/rust-toolchain@1.93.0`,
`Swatinem/rust-cache`, `taiki-e/install-action` or a `setup-*` action: these prepare the
runner and have no local counterpart beyond "the tool is installed at the pinned version".

### `pages.yml` (dispatch on `main`, with a tag)

| Job | Step | Local command | Mirrors? |
| --- | --- | --- | --- |
| build | Resolve the release tag | `gh release view --repo ashfordeOU/kshana --json tagName --jq .tagName` | only after the GitHub Release exists |
| build | checkout the tag | `git worktree add /tmp/k-pages v0.29.0` | only after the tag exists; before that, use the release commit |
| build | The tag and the manifest agree | `GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v0.29.0 ./scripts/check-version-sync.sh` | yes |
| build | Toolchain pin guard | `./scripts/check-toolchain.sh` | yes |
| build | Build playground | `./web/build.sh` (wasm-pack release build with the size pass, then stages `web/pkg`, `web/scenarios`, `web/assets`) | yes |
| build | Stage the deploy tree | `rsync -a --delete --exclude '*.test.mjs' --exclude 'smoke.mjs' --exclude 'build.sh' --exclude '/README.md' web/ site/ && find site -type f \| wc -l` then serve it: `python3 -m http.server -d site 8000` | yes |
| build | Upload Pages artifact | none | no: GitHub Pages artefact store |
| deploy | Deploy to GitHub Pages | none | no: needs the `github-pages` environment, which deploys only from `main` |

### `release.yml` (tag push)

| Job | Step | Local command | Mirrors? |
| --- | --- | --- | --- |
| verify | 7 steps | `./scripts/check-toolchain.sh && ./scripts/check-version-sync.sh && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all && ./scripts/check-reproducible.sh && ./scripts/check-no-attribution.sh` (set `GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v0.29.0` for the version check) | yes, on macOS |
| binaries | 3 targets | `cargo build --release --locked --bin kshana --target aarch64-apple-darwin`; with `rustup target add x86_64-apple-darwin`, the Intel one too, then `lipo -archs` | the two macOS targets; Windows (`x86_64-pc-windows-msvc`) needs a Windows host |
| release | build | `cargo build --release && cargo build --release --manifest-path mcp/kshana-mcp/Cargo.toml` | yes (macOS binaries; CI's are Linux) |
| release | SBOM | `./scripts/gen-sbom.sh > kshana-sbom.cdx.json` | yes |
| release | validation summary | `cargo run --release --bin validation_report -- kshana-validation-summary.html` | yes |
| release | release notes | `bash scripts/changelog-extract.sh 0.29.0 > RELEASE_NOTES.md` | yes |
| release | stage assets, SHA256SUMS | `mkdir assets && cp … assets/ && (cd assets && shasum -a 256 -- * > ../SHA256SUMS)` | yes, without the three downloaded binaries |
| release | Attest build provenance | none | no: signs with the run's OpenID Connect identity |
| release | create the GitHub Release | none | no: publishes |
| verify-release, verify-release-binaries | | the "GitHub Release" and "Provenance" rows of G8 | only after publication |
| publish, mcp-publish, jetbrains | called workflows | see below | |
| parity | | `python3 scripts/check_channel_parity.py 0.29.0` | only after publication |
| site | | `gh workflow run pages.yml --repo ashfordeOU/kshana --ref main -f tag=v0.29.0` | it IS the deploy: only on the founder's go |

### `publish.yml` and `wheels.yml` (called by the Release run)

| Job | Step | Local command | Mirrors? |
| --- | --- | --- | --- |
| gates | release verdict | `./scripts/check-release-verdict.sh <sha>` | no: reads the Actions API for a green `verify` of the tag-push run |
| gates | guards | `./scripts/check-toolchain.sh && ./scripts/check-version-sync.sh && ./scripts/check-no-attribution.sh` | yes |
| build-wheels (wheels.yml) | 6 wheels | `SOURCE_DATE_EPOCH=1700000000 maturin build --release --features python --out dist` | the macOS arm64 wheel only. Linux x86-64 and aarch64 build in the `manylinux_2_28` container (Docker), Windows x64 and arm64 on Windows runners |
| build-wheels | source distribution | `maturin sdist --out dist` | yes |
| build-wheels | auditwheel tag check | `auditwheel show dist/*.whl` | no: Linux wheels only |
| build-wheels | SBOM beside the wheels | `./scripts/gen-sbom.sh > dist/kshana-sbom.cdx.json` | yes |
| wheel-repro | build twice, compare bytes | two `maturin build` runs and `shasum` | no: the claim is about the manylinux container |
| arm-install-smoke | | none | no: needs an ARM64 Linux host |
| npm-build | build | `wasm-pack build --target web --out-dir web/pkg --release -- --features wasm && cp README.npm.md web/pkg/README.md` | yes |
| npm-build | SBOM into the package, patch `files` | `./scripts/gen-sbom.sh > web/pkg/kshana-sbom.cdx.json` and the `node -e` patch in the workflow | yes |
| npm-build | pack | `mkdir -p npm-dist && npm pack ./web/pkg --pack-destination npm-dist` | yes |
| npm-build | attest | none | no |
| crates | token check | none | no: `CARGO_REGISTRY_TOKEN` exists only in the run |
| crates | package | `cargo package --locked` | yes |
| crates | publish | `cargo publish --dry-run --locked` | the dry run only |
| crates-mcp | publish | `(cd mcp/kshana-mcp && cargo publish --dry-run)` | **no, until `kshana 0.29.0` is on crates.io**: the requirement `kshana = "^0.29"` has nothing to resolve to. Stand-in: `cargo publish --dry-run --config "patch.crates-io.kshana.path='<checkout>'"` |
| pypi-publish | upload | `twine check dist/*` | the metadata check only |
| npm | upload | `npm publish ./npm-dist/<tarball> --dry-run` | the dry run only (mind the leading `./`: without it npm reads the path as a GitHub shorthand) |

### `mcp-publish.yml` and `jetbrains-plugin.yml` (called by the Release run)

| Job | Step | Local command | Mirrors? |
| --- | --- | --- | --- |
| image | verdict, version sync | as above | the version check only |
| image | build and push, 2 platforms | `docker buildx build -f mcp/kshana-mcp/Dockerfile --platform linux/amd64,linux/arm64 -t kshana-mcp:0.29.0-dryrun .` (no `--push`) | needs the Docker daemon, which was **down** at writing; arm64 is native here, amd64 runs under emulation |
| image | attest image | none | no |
| registry | stamp server.json | `jq --arg v 0.29.0 --arg img ghcr.io/ashfordeou/kshana-mcp:0.29.0 '.version = $v \| .packages[0].identifier = $img' mcp/kshana-mcp/server.json` | yes (a no-op on the committed file) |
| registry | validate | `(cd mcp/kshana-mcp && mcp-publisher validate)` | yes (this machine has mcp-publisher 1.6.0; CI pins 1.8.1) |
| registry | login, publish | none | no: GitHub OpenID Connect identity of the run; the image must already be public on ghcr.io |
| build (JetBrains) | version sync | `./scripts/check-version-sync.sh` | yes |
| build (JetBrains) | build | `(cd ide/jetbrains && ./gradlew test buildPlugin --no-daemon --console=plain)` with Java 21 | yes |
| publish (JetBrains) | upload | none | no: `JETBRAINS_MARKETPLACE_TOKEN` |

## 9. Dry-run results, as measured (2026-09-30, on `feat/release-prep`)

One at a time, `CARGO_BUILD_JOBS=3`, load average 180 to 490 throughout. No network
publish. The raw table is `scratch/release/dryrun-results.tsv` (name, exit code, seconds,
command); each log is `scratch/release/logs/<name>.log`. Branch head at the runs: 29e0f20
(the two later commits change CHANGELOG wording only).

### The artefacts

| Dry run | Command | Exit | Result |
| --- | --- | --- | --- |
| Crate file list | `cargo package --locked --list` | 0 | 759 files (323 under `src/`, 232 `tests/`, 139 `scenarios/`, 26 `tools/`, 7 `data/`) |
| Crate package | `cargo package --locked` | 0 | `kshana-0.29.0.crate` **4 083 178 bytes** (3.9 MiB; 13.4 MiB unpacked). The crates.io limit is 10 MB: 41 % of it. 0.28.0 was 3 295 788 bytes. SHA-256 `06e62360…db6eca87`. Verification build green in 61 s |
| Crate publish, `kshana` | `cargo publish --dry-run --locked` | 0 | "aborting upload due to dry run" |
| MCP crate file list | `cd mcp/kshana-mcp && cargo package --locked --list` | 0 | 11 files |
| Crate publish, `kshana-mcp` | `cd mcp/kshana-mcp && cargo publish --dry-run` | **101** | expected, and not mirrorable before the release: "failed to select a version for the requirement `kshana = "^0.29"`", because `kshana 0.29.0` is not on crates.io yet. The workflow publishes `kshana` first and retries this one 5 times for the index |
| Crate publish, `kshana-mcp`, against the local engine | `cargo publish --dry-run --config "patch.crates-io.kshana.path='<worktree>'"` | 0 | packaged, compiled against `kshana 0.29.0`, "aborting upload due to dry run". `kshana-mcp-0.29.0.crate` **21 269 bytes** (66.5 KiB unpacked) |
| Python wheel | `SOURCE_DATE_EPOCH=1700000000 maturin build --release --features python --out dist` | 0 | `kshana-0.29.0-cp39-abi3-macosx_11_0_arm64.whl` **4 567 970 bytes** (this platform only; the other five wheels build on GitHub) |
| Python source distribution | `maturin sdist --out dist` | 0 | `kshana-0.29.0.tar.gz` **4 135 241 bytes**, 759 entries |
| Python checks | `twine check`; install in a fresh environment; `kshana.version()`; `pytest -q tests/python`; `mypy.stubtest kshana` | 0 each | both files PASSED; `0.29.0`; 15 passed; stub check clean |
| npm package, as asked (existing build) | `npm pack ./pkg` on a copy of `scratch/wasm-v29/pkg/` with the README and SBOM staged as `publish.yml` does | 0 | `kshana-0.28.0.tgz` 2 774 320 bytes, 7 files. `scratch/wasm-parity/pkg/` **does not exist**, so the `wasm-v29` package was used. It is stamped 0.28.0 because it was built before the bump |
| npm package, rebuilt at 0.29.0 | `./web/build.sh` (wasm-pack 0.13.1, release) then the `publish.yml` staging and `npm pack ./web/pkg` | 0 | `kshana-0.29.0.tgz` **2 774 442 bytes**, 7 files; `kshana_bg.wasm` 8 436 547 bytes (the live 0.28.0 one is 5 000 827). SHA-256 `1b1a3e23…f1c48813` |
| npm publish | `npm publish ./npm-dist/kshana-0.29.0.tgz --access public --dry-run` | 0 | `+ kshana@0.29.0` (dry run) |
| WASM smoke | `node web/smoke.mjs` | 0 | "wasm smoke OK — kshana 0.29.0 …" |
| Site stage (the CURRENT `web/`, not the redesign) | the `pages.yml` rsync into `scratch/release/site/` | 0 | 182 of 200 files staged, 10 MB |
| Docker image | `docker build -f mcp/kshana-mcp/Dockerfile .` | **not run** | the Docker daemon is not running (`docker info` fails; Docker Desktop is installed but was not started after the restart). Not mirrored |
| JetBrains plugin | `cd ide/jetbrains && ./gradlew test buildPlugin --no-daemon --console=plain` | 0 | BUILD SUCCESSFUL in 4 min 37 s; `kshana-jetbrains-0.29.0.zip` **17 535 bytes** |
| MCP registry manifest | `cd mcp/kshana-mcp && mcp-publisher validate` | 0 | "server.json is valid" (against the live registry schema; local tool 1.6.0, CI pins 1.8.1). The workflow's `jq` stamp is a no-op on the committed file. Description 98 characters (limit 100) |
| Release binary | `cargo build --release --locked --bin kshana` | 0 | 11 480 608 bytes; `kshana --version` prints `kshana 0.29.0`; 75 kinds; 132 bundled scenarios |
| MCP server binary | `cd mcp/kshana-mcp && cargo build --release --locked` | 0 | 15 903 632 bytes |
| SBOM | `./scripts/gen-sbom.sh` | 0 | CycloneDX 1.5, 66 components, 27 838 bytes |
| Validation summary | `cargo run --release --bin validation_report -- kshana-validation-summary.html` | 0 | 74 672 bytes |
| Release notes | `bash scripts/changelog-extract.sh 0.29.0` | 0 | 56 154 bytes (GitHub's limit for a release body is 125 000 characters) |

### The gates

| Gate | Command | Exit | Result |
| --- | --- | --- | --- |
| Toolchain pin | `./scripts/check-toolchain.sh` | 0 | rustc 1.93.0 matches |
| Format | `cargo fmt --all -- --check` | 0 | |
| Version sync | `./scripts/check-version-sync.sh` | 0 | every surface at v0.29.0 |
| Version sync, as the tag | `GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v0.29.0 ./scripts/check-version-sync.sh` | 0 | "release tag v0.29.0 matches the manifest". Control: with `v0.28.0` it exits 1 |
| Attribution | `./scripts/check-no-attribution.sh` | 0 | clean, content and history |
| Dependency audit | `cargo deny --all-features check advisories licenses bans sources` | 0 | all four ok |
| Dependency audit, MCP | `cd mcp/kshana-mcp && cargo deny check advisories licenses bans sources` | 0 | all four ok |
| README images | `python3 tools/gen_readme_assets.py --check` | 0 | regenerated at 0.29.0; 9 files changed, the version stamp only |
| Documentation coverage | `./scripts/check-doc-coverage.sh` | 0 | 985 (ceiling 985): **no headroom**; one new undocumented public item turns CI red |
| Reproducibility | `./scripts/check-reproducible.sh` | 0 | |
| Version, count and changelog tests | `cargo test --locked` for the 14 `*_doc_sync` binaries, `cli_first_run`, `registry_golden`, `published_figures_still_reproduce`, `workshop_preset_isolation`, `no_overclaims`, `tutorials`, `scenario_coverage` | 0 | 21 test binaries, 90 tests, 0 failed |
| Demo charts | the three README chart scenarios re-run with the release binary, compared byte for byte | 0 | identical |
| MCP lockfile | `cd mcp/kshana-mcp && cargo metadata --locked` | 0 | the hand-bumped lockfile is consistent |
| MCP protocol tests | `cd mcp/kshana-mcp && cargo test --locked` | 0 | 7 passed |
| Web tests | `node web/<each>.test.mjs` | 0 | 14 of 14; every file has its `ci.yml` step |
| Workflow scan | section 10 | | no folded-scalar comment, no self-hosted label, all actions pinned |

### Not run on this branch

These were deliberately left to step G3, because the push hook needs them on the final
commit anyway and the CI-fix agent is running the same code through the full gate on
`feat/integration-bc` (this branch differs from it only in version strings, README
images and the CHANGELOG):

- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all` (the whole suite; about 223 test binaries, 3 228 tests at 5488153)
- `cargo tarpaulin` (coverage), the minimum-supported-compiler build
  (`RUSTUP_TOOLCHAIN=1.85.0 cargo build --all --locked`), `cargo build -p kshana --lib
  --target wasm32-unknown-unknown` as its own step (the wasm-pack build above compiled
  the same target with the `wasm` feature)
- the Docker image build (daemon down)
- anything on Linux or Windows (wheels, binaries, the reproducibility matrix legs)

## 9b. How long "go" to "live" takes

| Phase | Time |
| --- | --- |
| G1 to G2: merges, rebase, rebuild, regenerate | 30 to 45 min under load |
| G3: local gate on the final commit | 65 min quiet; 2 to 3 h at today's load (`REPEAT=0`), more with the 3 repeat runs |
| G5: CI on `main` | 77 min last time; expect about 90 |
| G7: Release run | 1 h 58 min last time; expect about 2 h 15 min |
| G8 to G9: checks and the site | 20 min |
| **Total** | **about 5 to 7 hours after the go**, of which about 3.5 hours are GitHub's and cannot be shortened |

## 9c. What blocks a release right now

| # | Blocker | Kind | Owner |
| --- | --- | --- | --- |
| B1 | The founder's go has not been given | rule | founder |
| B2 | `main` does not contain the engine yet: `feat/integration-bc` is not merged, and its full gate result is not in hand | pending | CI-fix agent, parent |
| B3 | The redesigned site is not in `web/` (phase 5). A release today redeploys the old site | pending | parent |
| B4 | `feat/mcp-newkinds` (c2510ee) and `feat/wasm-parity` are not merged; each will conflict with or precede the version bump | pending | their agents, parent |
| B5 | The full local gate has not run on the release commit, so there is no push receipt | pending | step G3 |
| B6 | `cargo publish --dry-run` for `kshana-mcp` cannot pass before `kshana 0.29.0` is on crates.io (stand-in with a patch passed) | not mirrorable | by design |
| B7 | The Docker image build was not mirrored (daemon down). The image builds only in the Release run, 27 min, after crates.io is already public | not mirrored | start Docker Desktop and run the section 8 command before the go |
| B8 | Five of six wheels, the Windows and Intel binaries, the wheel byte-reproducibility check, and every attestation exist only on GitHub runners | not mirrorable | watch the Release run |
| B9 | Documentation coverage sits exactly at its ceiling (985 of 985) | risk, not red | anyone adding public items |
| B10 | The date (2026-09-30) is pinned in `CHANGELOG.md` and `CITATION.cff` | risk if the tag slips past midnight | step G2 |
| B11 | Founder decisions in section 11, above all whether the Celeste preset ships in the crate | decision | founder |

Nothing is red. No secret is missing (names checked; values cannot be).

## 10. Traps checked

| Trap | Result |
| --- | --- |
| A `#` comment inside a folded `run: >` scalar | none. Two folded `run: >` blocks exist (`ci.yml` lines 217 and 249); neither holds a `#`. The pre-push contract also runs `.github/ci-local/check-workflow-run-blocks.py` |
| Secrets a job needs | `CARGO_REGISTRY_TOKEN`, `PYPI_API_TOKEN`, `NPM_TOKEN`, `JETBRAINS_MARKETPLACE_TOKEN` are all set on the repository (names read with `gh secret list`; they published 0.28.0 four days ago). A value that has since expired would only show in the run. `SONAR_TOKEN` is set. The three optional JetBrains signing secrets are **not** set: the workflow then publishes unsigned and the Marketplace signs, by design |
| Variables | `MCP_REGISTRY_PUBLISH=true` is set, so the MCP registry step runs |
| Environments | `crates-io`, `crates-io-mcp`, `pypi`, `npm`, `ghcr-image`, `mcp-registry`, `jetbrains-marketplace` have no protection rules (no reviewer will be asked). `github-pages` deploys from `main` only, which is why `pages.yml` is dispatched on `main` |
| Self-hosted runner labels | none. Every job runs on GitHub-hosted runners: `ubuntu-latest`, `macos-latest`, `windows-latest`, `windows-11-arm`, `ubuntu-24.04-arm` |
| Actions pinned by version | every third-party action is pinned to a full commit SHA, except `dtolnay/rust-toolchain@1.93.0` and `@1.85.0`, where the ref is the toolchain selector (the workflow comments say why) and `scripts/check-toolchain.sh` guards it |
| Branch protection | `main`: no force pushes, no deletions, no required checks, no required reviews |
| **The site comes from `web/` in the tag** | `pages.yml` runs `web/build.sh` on the tag. The redesigned site lives in `~/Code/kshana-site-next`; until phase 5 ports it into `web/`, a release redeploys the old site |
| **`web/index.html` strings** | `scripts/check-version-sync.sh` check 5 greps `web/index.html` for five exact strings (`"softwareVersion": "0.29.0"`, `>v0.29.0<`, `favicon.svg?v=0.29.0`, `style.css?v=0.29.0`, `app.js?v=0.29.0`). A ported page that drops any of them turns CI, the release `verify` job and the pages build red. Keep them or change the script in the same commit |
| **Web tests are listed by name in `ci.yml`** | `test-wasm-bindings` fails if `web/*.test.mjs` on disk and the `run: node web/….test.mjs` steps differ, in either direction. A port that adds or removes a test file must edit `ci.yml` |
| Deploy tree excludes | `pages.yml` drops `*.test.mjs`, `smoke.mjs`, `build.sh` and `/README.md` from the deploy. A new build tool or source folder under `web/` needs its own exclude or it is served |
| The push hook | a push needs a gate receipt for exactly HEAD on a clean tree (`scripts/check-gate-receipt.sh`), and the contract file `.ci-native` exists only in `~/Code/kshana`. Push from there, not from a worktree |
| The release date | pinned in `CHANGELOG.md` and `CITATION.cff` (section 2) |
| Local manifest drift | `.github/ci-local/manifest.yml` runs `cargo deny check …` without `--all-features` and the MCP one through `--manifest-path`; CI's lines are in section 8. Low risk, but the local run inspects 8 fewer crates than CI |

## 11. Decisions that are the founder's

1. **The go itself**, and whether tonight's scaled-down audit is enough (`GOAL.md`, DEADLINE).
2. **Celeste in the published crate.** The crate tarball as built today contains
   `src/celeste_iod.rs` and the five `scenarios/*celeste-iod*.toml` files, so the
   workshop-derived preset ships to crates.io, PyPI (source distribution) and GitHub.
   The design lets it be withheld without a source edit: delete that file and those five
   scenarios. Ship or withhold?
3. **Host product names** in `docs/integrations.md` and older CHANGELOG entries (assistant
   and editor names as integration targets). Already public in 0.28.0; `GOAL.md` lists it
   as open.
4. **The 0.29.0 CHANGELOG wording**, in particular the opening summary and the
   "Revisions to published numbers" section: it is the text of the GitHub Release.
