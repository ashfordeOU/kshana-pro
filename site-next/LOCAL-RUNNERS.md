# Kshana 0.29.0: the GitHub Actions pipeline on local runners

Local-runner pass, 2026-09-30 to 2026-10-01. Nothing was pushed, tagged, published or
dispatched on GitHub. Everything here re-runs with one script:
`~/Code/kshana-site-next/scratch/actlocal/rerun.sh <commit>` (section 6).

**Release candidate tested: `feat/integration-bc` at `fb738176`** (it contains both fix
commits from `feat/ci-local-fixes`, section 2). An earlier full pass ran on `5f6f1ee6`
(section 7); it found the two defects.

**How:** act 0.2.88 with Docker Desktop 29.2.1 (VM: 14 CPUs, 7.65 GiB). The act image is
`kshana-act:ubuntu-latest`, built from `scratch/actlocal/image/Dockerfile`: the medium
`catthehacker/ubuntu:act-latest` image plus rustup and rsync, which GitHub's
`ubuntu-latest` image ships and these workflows use without installing them (the first
`deny` run on the plain medium image died with `cargo: command not found`). The full
catthehacker image (about 20 GB) was not needed. act runs `linux/arm64` containers on
this Mac by default; the rows marked **amd64** ran `linux/amd64` (x86-64) under Rosetta
(`/proc/cpuinfo` in the container reads `VirtualApple`), because GitHub's ubuntu runners
are x86-64. `BINARYEN_CORES=4` is passed to every act job (GitHub's ubuntu runners have
4 vCPUs; see section 5). `CARGO_BUILD_JOBS=4` while the receipt gate ran beside it.

The full arm64 `cargo test --all` (`ci.yml check`) was **not** rerun on `fb738176`: the
receipt gate (`scripts/gate.sh`) runs that suite on the same commit, and the parent asked
for it to be skipped. It ran green under act on `5f6f1ee` plus the fixes (section 7).

## 1. Results on `fb738176`

Exit 0 = green. Seconds are wall-clock on this machine, sharing the CPU with the gate.

### `ci.yml` (push to `main`)

| Job | How run | Exit | Seconds | Notes |
| --- | --- | --- | --- | --- |
| check | not rerun (see above) | | | covered by `scripts/gate.sh` on `fb738176`; green under act on `5f6f1ee`+fixes in 6438 s |
| package | act, arm64 | 0 | 36 | `cargo package --locked`, verification build |
| msrv | act, arm64 | 0 | 50 | "declared rust-version=1.85, active rustc=1.85.0" |
| deny | act, arm64 | 0 | 14 | `--all-features` |
| coverage | not run | | | tarpaulin needs the whole suite under instrumentation (about 92 min on GitHub, several hours here); one attempt beside two other jobs was killed by the 7.65 GiB Docker VM's out-of-memory killer (rustc SIGKILL), a resource limit, not a code fault. GitHub's job stays the verdict |
| reproducibility-matrix (ubuntu, arm64) | act, arm64 | 0 | 696 | the ubuntu leg as written |
| reproducibility-matrix (ubuntu, x86-64) | act, **amd64** | 0 | 1090 | inside `xarch-pins` below (same four suites) |
| reproducibility-matrix (macos) | native, macOS arm64 | 0 | 643 | `cargo test --test cross_platform_golden --test golden --test determinism --test sgp4_verification` |
| reproducibility-matrix (windows) | cannot run locally | | | Windows host |
| test-python-bindings | native stand-in, **x86-64** manylinux container | 0 | 333 | act cannot host PyO3/maturin-action (section 3). `manylinux.sh`: `quay.io/pypa/manylinux_2_28_x86_64`, pinned toolchain, wheel installed, `pytest tests/python` 15 passed, stubtest "no issues" (allowlist `kshana\.kshana`) |
| test-wasm-bindings | act, arm64 | 0 | 113 | wasm-pack 0.13.1, Node 20; "OK: every test file under web/ has its own step in ci.yml"; the new "Stage the package next to the Studio" step; 25 test steps green (smoke, `site.test.mjs`, `legacy-urls.test.mjs`, 22 Studio module tests in `web/playground/lib/`) |
| public-standalone | act, arm64 | 0 | 195 | |
| wasm-registry | act, arm64 | 0 | 36 | |
| mcp-check | act, arm64 | 0 | 45 | |
| mcp-deny | act, arm64 | 0 | 14 | |

### `pages.yml` (dispatch on `main`, tag `v0.29.0`): the redesigned site, built by the real workflow

| Job / step | How run | Exit | Seconds | Notes |
| --- | --- | --- | --- | --- |
| build | act, arm64, derived copy | 0 | 250 | every step as written: tag resolution, version sync on the tag, toolchain guard, "Build the WebAssembly engine and stage it for the Studio" (`web/build.sh`, 2 min 51 s), "Stage the deploy tree": **793 of 824 files staged** |
| site check | native, on the staged tree | 0 | 2 | `check_site.py` on the staged `site/` (kept as a run artefact in place of the Pages upload): 791 files (the artefact omits the 2 hidden `.gitignore` files wasm-pack writes in `pkg/`), **6,903 local href/src/url() references, 0 missing**; no test, smoke or build file staged; `404.html` present; **79 of 79 HTML pages answer 200** from a real HTTP server. A deliberately deleted `site.css` makes it exit 1 |
| Upload Pages artifact, deploy | cannot run locally | | | GitHub Pages store and the `github-pages` environment |

### `release.yml` (tag push `v0.29.0`)

| Job | How run | Exit | Seconds | Notes |
| --- | --- | --- | --- | --- |
| verify | act, **amd64**, derived copy | 0 | 6024 | all 7 steps on x86-64: toolchain guard, version sync on the tag, fmt, clippy `-D warnings`, `cargo test --all` (1 h 38 min; 224 test binaries plus doc-tests, 3,239 passed, 0 failed, 19 ignored), reproducibility guard, attribution guard |
| binaries (aarch64- and x86_64-apple-darwin) | native, macOS | 0 | 221 | `lipo -archs` arm64 / x86_64; both print the usage banner (x86_64 under Rosetta) |
| binaries (x86_64-pc-windows-msvc) | cannot run locally | | | Windows host |
| release | act, arm64, derived copy | 0 | 194 | builds, SBOM, validation summary, release notes, staging, SHA256SUMS |
| verify-release, verify-release-binaries, parity, site | cannot run locally | | | read the published Release and registries |

### Called by the Release run

| Job | How run | Exit | Seconds | Notes |
| --- | --- | --- | --- | --- |
| publish: gates (guards) | act, arm64, derived copy | 0 | 16 | toolchain, version sync on the tag, attribution |
| publish: npm-build | act, arm64, derived copy | 0 | 239 | wasm-pack with wasm-opt, SBOM inside the package, `npm pack`, run artefact |
| wheels: ubuntu x86_64 + sdist + auditwheel, and wheel-repro | native stand-in, x86-64 manylinux | 0 | 333 (same run as test-python-bindings) | auditwheel `manylinux_2_28_x86_64`; two builds byte-identical (`de0df0cc…a563c`); sdist built |
| wheels: macOS aarch64 | native | 0 | 104 | wheel, venv, pytest 15 passed, stubtest clean |
| wheels: macOS x86_64 | native, cross-compiled as CI does | 0 | 93 | `macosx_10_12_x86_64` wheel, extension is x86_64 |
| wheels: ubuntu aarch64, windows x64, windows-11-arm, arm-install-smoke | not run | | | Windows hosts / `ubuntu-24.04-arm` label; the aarch64 Linux leg is the same container on the other arch |
| mcp-publish: image | native `docker buildx`, no push | 0 / 0 | 44 (arm64) / 87 (amd64) | both platforms CI builds; each image then answers a real MCP `initialize` over stdio (`serverInfo` kshana-mcp 0.29.0) |
| mcp-publish: registry (offline half) | native | 0 | 1 | server.json stamp (a no-op on the committed file) and `mcp-publisher 1.8.1 validate` (CI's pinned version; its linux_amd64 tarball matches the workflow's SHA-256) |
| jetbrains-plugin: build | act, arm64, derived copy | 0 | 101 | `./gradlew test buildPlugin`, Java 21 |
| every registry upload, login, attestation | cannot run locally | | | tokens and the run's OpenID Connect identity |

### x86-64 platform-sensitive tests

| Job | How run | Exit | Seconds | Notes |
| --- | --- | --- | --- | --- |
| xarch-pins | act, **amd64** | 0 | 1090 | 22 suites + `cargo test --lib portable_math` (6 tests, including `the_portable_functions_return_the_pinned_bits_on_every_platform`) |

`xarch-pins` is `ci.yml`'s reproducibility-matrix job (checkout, `dtolnay/rust-toolchain@1.93.0`,
rust-cache) with its test step widened to every suite that pins exact bytes or published
figures: `cross_platform_golden`, `golden`, `determinism`, `sgp4_verification`,
`leo_navmsg_reference` (frame bytes and CRC-24Q), `published_figures_still_reproduce`,
`registry_golden`, `workshop_preset_isolation` and every `*_doc_sync` suite (14), then the
portable-maths unit pins. `derive.py` builds the list from the checkout, so a new
`*_doc_sync` file joins automatically.

### Other checks (`fb738176`)

| Check | Result |
| --- | --- |
| actionlint 1.7.11 (with shellcheck), all 15 workflows | exit 0, no findings |
| `#` inside a folded `run: >` scalar | 3 folded blocks, none contains `#` |
| Third-party actions pinned | every `uses:` is a full commit SHA except `dtolnay/rust-toolchain@1.93.0` (21) and `@1.85.0` (1). Deliberate: `scripts/check-toolchain.sh` requires that ref to equal the toolchain channel, and the action reads the toolchain from it. A SHA pin would need a matching guard change; left as is |
| Secrets the jobs name | `CARGO_REGISTRY_TOKEN`, `NPM_TOKEN`, `PYPI_API_TOKEN`, `JETBRAINS_MARKETPLACE_TOKEN`, `SONAR_TOKEN`, `GITHUB_TOKEN` (built in), variable `MCP_REGISTRY_PUBLISH`; plus the optional `JETBRAINS_CERTIFICATE_CHAIN`, `JETBRAINS_PRIVATE_KEY`, `JETBRAINS_PRIVATE_KEY_PASSWORD` (unset: the Marketplace signs). All named as present in RELEASE-RUNBOOK.md section 10. No secret value was read |

## 2. Fix commits on `feat/ci-local-fixes` (both merged into `fb738176`)

Worktree `~/Code/kshana-wt-cifix`, branched from `feat/integration-bc` at 5f6f1ee6.

1. **`d8890fb4` fix(guard): the attribution scan refuses a tree it cannot read.**
   `scripts/check-no-attribution.sh` ended both scans in `|| true`, which also swallowed
   git's own failure. Measured: a directory with a planted marker and no repository
   printed "OK: clean" and exited 0. Under act that is what a worktree gives (its `.git`
   pointer names a host path the container cannot see), so a local run would have passed
   the guard blind. Now no repository or a shallow clone fails, and `git grep`'s exit
   status is read (1 = clean, above 1 = failure). A planted marker still exits 1; the real
   tree exits 0; every workflow caller already uses `fetch-depth: 0`.
2. **`aaec0ac5` fix(ledger): link only paths whose spelling exists exactly; drop a dead link.**
   **This would have turned GitHub's `check` job and the Release `verify` job red.**
   `web/data/verification-matrix.json` carried `src/SOLAR_SYSTEM.rs`: the generator turned
   the constant `SOLAR_SYSTEM` in a `module` field into a candidate path, and on the
   case-insensitive macOS filesystem `is_file()` said yes (it is `src/solar_system.rs`).
   On Linux the ledger regenerates without that link, so
   `verification_artifacts_doc_sync::ledger_json_matches_the_matrix` and
   `ledger_links_resolve_to_committed_files` fail there and pass on macOS, which is why
   every gate on this laptop was green. Found by the x86-64 run; it fails the same way on
   arm64 Linux. The existence check now requires every path component to be listed with
   the exact spelling; the ledger is regenerated (one link removed, nothing else). A scan
   of every tracked file for case-mismatched path references finds no other.

## 3. What cannot run locally, and why

| Job / step | Why |
| --- | --- |
| Every `runs-on: macos-latest` / `windows-latest` / `windows-11-arm` / `ubuntu-24.04-arm` job | act runs Linux containers only. The macOS legs ran natively (rows above); Windows legs have no host here |
| `test-python-bindings`, `wheels` Linux legs, `wheel-repro` under act | PyO3/maturin-action starts a sibling manylinux container on the host daemon and bind-mounts `/tmp/run-maturin-action.sh`, a file it wrote inside the job container; the host has no such file, so the daemon makes an empty directory and the build dies "exec: /tmp/run-maturin-action.sh: Is a directory" (exit 126). An act limitation, not a repo defect. Stand-in: the same manylinux_2_28 container driven directly (`manylinux.sh`), green |
| `coverage` | runs, but not to completion here in the time (see its row) |
| Upload lcov / Pages artefact; attestations; creating the GitHub Release | GitHub artefact store, the run's OpenID Connect identity, publishing |
| "Require a green release verdict" (publish gates, mcp-publish image, jetbrains publish) | reads the Actions API for a green `verify` of the tag-push run |
| crates / crates-mcp / pypi-publish / npm / image push / MCP registry login+publish / JetBrains publish | registry uploads; the dry runs are in RELEASE-RUNBOOK.md section 9 |
| `pages.yml` deploy | `github-pages` environment |
| `verify-release`, `verify-release-binaries`, `parity`, `site` | only meaningful after publication |
| `sonarcloud.yml` | triggered by a finished CI run on GitHub; needs `SONAR_TOKEN` and the service |
| CodeQL (default setup, no workflow file) | GitHub-hosted |

Derived copies (`scratch/actlocal/wf/*.yml`, regenerated from the checkout by `derive.py`
on every run) change only: a checkout `ref` that is an expression is dropped (act compares
the raw text with `github.ref`, never matches, and clones the not-yet-existing tag from
GitHub; the local tree is the release commit); the upload, attest and deploy steps above
are removed (the Pages upload becomes a plain run artefact so the site can be checked);
`needs:` is dropped; a `workflow_call` trigger becomes a tag push. Every other step is the
workflow's own text.

## 4. x86-64 versus arm64

- **Ran on x86-64 Linux:** `xarch-pins` (every byte and figure pin), the full Release
  `verify` (`cargo test --all`, green on `fb738176` in 6024 s and on `5f6f1ee`+fixes in 7480 s),
  the manylinux wheel (built twice byte-identically, installed, tested), the MCP image.
- **Tests that pin bytes differing between arm64 and x86-64: none found.** Every pin suite
  gave the same verdict on linux/arm64, linux/amd64 and macOS arm64, on both trees. The one
  Linux-only failure (fix 2) is filesystem case-sensitivity, not numerics, and failed the
  same on both Linux architectures.
- Caveat: Rosetta executes x86-64 floating point exactly as an Intel CPU does, but reports
  different CPU features from GitHub's runners; code choosing a SIMD path at run time could
  take a different branch there. Nothing in the pin suites showed a sign of that.
- **Still uncovered:** the Windows leg of `reproducibility-matrix` (4 suites on
  x86_64-pc-windows-msvc with the MSVC maths library), the Windows CLI binary build and its
  smoke run, the Windows x64 and ARM64 wheels. macOS x86-64 is covered as a cross-compiled
  binary run under Rosetta and a cross-compiled wheel (its import not exercised: no x86-64
  Python here); CI does not run the test suites on Intel macOS either.

## 5. Notes for the GitHub runs

- `wasm-opt` under act with 14 visible CPUs ran 27 min (5f6f1ee) and was stopped after
  48 min and 11 CPU-hours (fb738176); with `BINARYEN_CORES=4`, as on a 4-vCPU GitHub
  runner, the whole pages build took 250 s. Binaryen's Linux release is a static musl
  build, whose allocator degrades badly with many threads. GitHub's past pages runs take
  about 4 minutes, so the 30-minute cap is not at risk there; it only matters on a
  many-core self-hosted runner.
- `Swatinem/rust-cache`'s post step prints `ENOENT … target/package/kshana-0.29.0/tests/…`
  annotations after `cargo package`; non-fatal, the job is green.
- The Docker VM's 7.65 GiB holds two heavy cargo jobs at once, not three.

## 6. Rerun on the final release commit

```
~/Code/kshana-site-next/scratch/actlocal/rerun.sh <release-commit>
QUICK=1 .../rerun.sh <commit>                       # skip check, verify, coverage
ONLY="pages-build ci-test-wasm-bindings" .../rerun.sh <commit>   # named entries only
```

It starts Docker if needed (with `DOCKER_CONFIG` pointing at a `{}` config plus a symlink
to `~/.docker/cli-plugins`, which avoids the desktop credential helper hanging background
jobs), builds the two act images on first use, points the act clone
(`scratch/actlocal/src`, a real clone, because a worktree's `.git` pointer is a host path
the container cannot follow; its push URL is disabled) and the native worktree
(`~/Code/kshana-wt-actlocal`) at the commit, regenerates the derived workflows, runs every
entry above in order with the multi-hour ones last (arm64 `check`, amd64 `verify`,
`coverage`), writes `results.tsv` and `results-<sha>.tsv`, prints the table, and exits
non-zero if any entry is red. Logs: `scratch/actlocal/logs/<name>.log`. It runs from a
snapshot of itself, so editing it mid-run is safe.

Time on a quiet machine: about 1 hour with `QUICK=1`; add about 2 hours for `check`, about
2 hours for the emulated x86-64 `verify` and several for `coverage`. If the receipt gate
already covers `check` on that commit, run
`ONLY="<everything but ci-check and ci-coverage>"` as this pass did
(`scratch/actlocal/batch-fb738176.sh` is that list).

## 7. The earlier pass on `5f6f1ee6` (found the two defects)

| Job | How run | Exit | Seconds | Tree |
| --- | --- | --- | --- | --- |
| ci check (full) | act, arm64 | 0 | 6438 | 5f6f1ee+fixes: every step; `cargo test --all` 1 h 45 min, 224 binaries + doc-tests, 3,238 passed, 0 failed, 19 ignored |
| release verify (full) | act, **amd64** | 0 | 7480 | 5f6f1ee+fixes: all 7 steps on x86-64 |
| xarch-pins | act, amd64 | **1** | 1884 | 5f6f1ee unfixed: `verification_artifacts_doc_sync` 2 of 8 failed (fix 2) |
| xarch-pins | act, amd64 | 0 | 1373 | 5f6f1ee+fixes |
| deny, mcp-deny, package, msrv, wasm-registry, mcp-check, public-standalone, test-wasm-bindings | act, arm64 | 0 each | 15 to 166 | 5f6f1ee |
| reproducibility-matrix ubuntu / macOS | act / native | 0 / 0 | 758 / 732 | 5f6f1ee |
| pages build, release build, publish gates, npm-build, jetbrains build | act, arm64 | 0 each | 1638 / 194 / 19 / 1492 / 178 | 5f6f1ee (release build +fixes) |
| manylinux x86-64, macOS wheels, macOS binaries, MCP image both arches, MCP registry validate | native | 0 each | | 5f6f1ee |

Logs of the unfixed failures: `scratch/actlocal/logs/xarch-pins-amd64.5f6f1ee-unfixed.log`,
`ci-check.5f6f1ee-unfixed-partial.log`, `ci-coverage.oom-attempt1.log`. Raw tables:
`results-5f6f1ee-final.tsv`, `history-5f6f1ee.tsv`.

## 8. Disk used

| Item | Size | Keep? |
| --- | --- | --- |
| Docker images `kshana-act:ubuntu-latest` (arm64) and `kshana-act:ubuntu-latest-amd64` | 2.98 GB + 3.14 GB (share layers with `catthehacker/ubuntu:act-latest`, which was already present and was refreshed) | yes, `rerun.sh` uses them |
| `quay.io/pypa/manylinux_2_28_x86_64` | 2.45 GB | yes, the python and wheel stand-in |
| `quay.io/pypa/manylinux_2_28_aarch64` | 2.5 GB | removed (pulled by the failed act maturin run) |
| Docker build cache from the image builds | about 3 to 4 GB (`docker system df` build cache grew 17.3 → 24.9 GB over the night, partly other agents) | prunable; not pruned because `docker builder prune` would also drop other projects' cache |
| `scratch/actlocal/` (act clone, rust-cache store, artefacts, logs) | 1.5 GB | yes |
| `~/Code/kshana-wt-actlocal` (native worktree and its target/) | 3.6 GB | yes, for the rerun; `git worktree remove` after the release |
| `~/Code/kshana-wt-cifix` | 37 MB | until the branch is deleted |

Total about 14 GB. Free space on `/` read 138 GB at the start, 49 GB at the low point (other
agents' builds), 98 GB at the end.
