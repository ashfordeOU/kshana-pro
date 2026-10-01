#!/usr/bin/env bash
# Reproduce the local-runner green on one commit: every Linux job of ci.yml under act,
# pages.yml's build job, release.yml's verify job and the Linux build jobs of the release
# side (derived copies: only the upload/attest/deploy steps removed, see derive.py), then
# the macOS-host legs natively. Writes results-<sha>.tsv and exits non-zero if any is red.
#
# usage: rerun.sh [<commit-ish>]      (default: feat/integration-bc in ~/Code/kshana)
#        QUICK=1 rerun.sh ...          skip the multi-hour jobs (check, verify, coverage)
#        ONLY="ci-deny pages-build" rerun.sh ...   run just the named entries
#
# Needs: Docker Desktop running; image kshana-act:ubuntu-latest (built from image/Dockerfile
# on first use); act at /opt/homebrew/bin/act. Nothing is pushed, tagged or published.
set -uo pipefail
S="$HOME/Code/kshana-site-next/scratch/actlocal"
# bash reads a script as it runs: run from a snapshot so editing this file mid-run is safe.
if [ -z "${RERUN_SNAPSHOT:-}" ]; then
  snap="$S/.rerun-snapshot-$$.sh"; cp "$0" "$snap"; RERUN_SNAPSHOT=1 exec bash "$snap" "$@"
fi
REF="${1:-feat/integration-bc}"
SHA="$(git -C "$HOME/Code/kshana" rev-parse --verify "$REF^{commit}")" || { echo "unknown commit $REF"; exit 2; }
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
export DOCKER_CONFIG="$S/dockercfg"   # {} + cli-plugins symlink: the desktop credsStore helper hangs background jobs
mkdir -p "$DOCKER_CONFIG"; [ -f "$DOCKER_CONFIG/config.json" ] || echo '{}' > "$DOCKER_CONFIG/config.json"
ln -sfn "$HOME/.docker/cli-plugins" "$DOCKER_CONFIG/cli-plugins"

docker info >/dev/null 2>&1 || { open -a Docker; for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 3; done; }
docker info >/dev/null 2>&1 || { echo "Docker is not running"; exit 2; }
docker image inspect kshana-act:ubuntu-latest >/dev/null 2>&1 || docker build --platform linux/arm64 -t kshana-act:ubuntu-latest "$S/image" || exit 2
docker image inspect kshana-act:ubuntu-latest-amd64 >/dev/null 2>&1 || docker build --platform linux/amd64 -t kshana-act:ubuntu-latest-amd64 "$S/image" || exit 2
AMD64() { ACT_ARCH=linux/amd64 ACT_IMAGE=kshana-act:ubuntu-latest-amd64 "$@"; }

# 1. The act source: a real clone (a worktree's .git pointer is a host path the job
#    container cannot follow), detached at the commit under test.
if [ ! -d "$S/src/.git" ]; then git clone -q --no-local "$HOME/Code/kshana" "$S/src" || exit 2; fi
# act names the repository from origin's URL: point it at the GitHub name, with pushing disabled.
git -C "$S/src" remote set-url origin https://github.com/ashfordeOU/kshana.git
git -C "$S/src" remote set-url --push origin NO-PUSH-local-runner-clone
git -C "$S/src" fetch -q "$HOME/Code/kshana" "+refs/heads/*:refs/remotes/local/*" && git -C "$S/src" checkout -q --detach "$SHA" || exit 2
# 2. The native worktree, detached at the same commit.
WT="$HOME/Code/kshana-wt-actlocal"
[ -e "$WT/.git" ] || git -C "$HOME/Code/kshana" worktree add --detach "$WT" "$SHA" || exit 2
git -C "$WT" checkout -q --detach "$SHA" || exit 2
export WT
python3 "$S/derive.py" "$S/src" "$S/wf" || exit 2
V="$(grep -m1 '^version' "$S/src/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
printf '{"ref":"refs/heads/main","inputs":{"tag":"v%s"}}\n' "$V" > "$S/event-workflow_dispatch.json"
printf '{"ref":"refs/tags/v%s"}\n' "$(grep -m1 '^version' "$S/src/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')" > "$S/event-tag.json"

short="$(git -C "$S/src" rev-parse --short HEAD)"
[ -n "${ONLY:-}" ] || : > "$S/results.tsv"
want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
slow() { [ -n "${QUICK:-}" ] && { echo "SKIP (QUICK) $1"; return 0; }; return 1; }
A="$S/run_act.sh"; N="$S/run_native.sh"

# --- act: ci.yml (push to main) ---
for j in deny mcp-deny package msrv wasm-registry mcp-check public-standalone test-wasm-bindings; do
  want "ci-$j" && "$A" "ci-$j" .github/workflows/ci.yml push "$j"
done
# test-python-bindings, wheels (Linux legs) and wheel-repro build through PyO3/maturin-action,
# which starts a SIBLING manylinux container on the host daemon and bind-mounts a script it
# wrote to the job container's /tmp: the host has no such file, so the daemon creates an
# empty directory there and the build dies "exec: /tmp/run-maturin-action.sh: Is a directory"
# (exit 126). act limitation, not a repo defect. Stand-ins: native-manylinux (the same
# manylinux_2_28 container driven directly) and native-wheel-macos.
want ci-reproducibility-matrix-ubuntu && "$A" ci-reproducibility-matrix-ubuntu .github/workflows/ci.yml push reproducibility-matrix --matrix os:ubuntu-latest
# --- act: pages.yml build (dispatch on main with the release tag) ---
want pages-build && rm -rf "$S"/artifacts/*/pages-site*
want pages-build && "$A" pages-build "$S/wf/pages-build.yml" workflow_dispatch build --input tag="v$(grep -m1 '^version' "$S/src/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
want pages-build && "$N" pages-site-check "python3 $S/check_site.py $S/artifacts"
# --- act: release side, Linux build jobs that upload nothing (tag push) ---
want release-build && "$A" release-build "$S/wf/release-build.yml" push release --eventpath "$S/event-tag.json"
want publish-gates && "$A" publish-gates "$S/wf/publish-build.yml" push gates-guards --eventpath "$S/event-tag.json"
want publish-npm-build && "$A" publish-npm-build "$S/wf/publish-build.yml" push npm-build --eventpath "$S/event-tag.json"
# x86-64 (Rosetta-emulated): GitHub's ubuntu runners are x86-64; these pin exact bytes/figures.
want xarch-pins-amd64 && AMD64 "$A" xarch-pins-amd64 "$S/wf/xarch-pins.yml" push xarch-pins
want ci-reproducibility-matrix-ubuntu-amd64 && AMD64 "$A" ci-reproducibility-matrix-ubuntu-amd64 .github/workflows/ci.yml push reproducibility-matrix --matrix os:ubuntu-latest
want jetbrains-build && "$A" jetbrains-build "$S/wf/jetbrains-build.yml" push build --eventpath "$S/event-tag.json"

# --- native (macOS arm64): the legs act cannot host ---
want native-repro-matrix-macos && "$N" native-repro-matrix-macos 'cargo test --test cross_platform_golden --test golden --test determinism --test sgp4_verification'
want native-binaries-macos && "$N" native-binaries-macos 'rustup target add x86_64-apple-darwin >/dev/null && for t in aarch64-apple-darwin x86_64-apple-darwin; do cargo build --release --locked --bin kshana --target $t; lipo -archs target/$t/release/kshana; done; for r in "" "arch -x86_64"; do t=aarch64-apple-darwin; [ -n "$r" ] && t=x86_64-apple-darwin; out="$($r ./target/$t/release/kshana 2>&1)" || true; printf "%s\n" "$out" | grep -q "usage: kshana" || { echo "FAIL: $t printed no usage banner"; exit 1; }; echo "OK: $t prints its usage banner"; done'
want native-wheel-macos && "$N" native-wheel-macos "rm -rf $S/pydist $S/venv && SOURCE_DATE_EPOCH=1700000000 maturin build --release --features python --out $S/pydist && python3 -m venv $S/venv && $S/venv/bin/pip -q install pytest numpy mypy && $S/venv/bin/pip -q install --no-index --find-links $S/pydist kshana && $S/venv/bin/python -m pytest -q tests/python && (cd $S && printf 'kshana\\\\.kshana\\n' > stubtest-allow.txt && $S/venv/bin/python -m mypy.stubtest kshana --allowlist stubtest-allow.txt)"
want native-wheel-macos-x86_64 && "$N" native-wheel-macos-x86_64 "rm -rf $S/pydist-x86 && SOURCE_DATE_EPOCH=1700000000 maturin build --release --features python --target x86_64-apple-darwin --out $S/pydist-x86 && ls $S/pydist-x86 | grep -qE 'macosx_.*_x86_64' && rm -rf $S/pydist-x86/unz && mkdir $S/pydist-x86/unz && (cd $S/pydist-x86/unz && unzip -q ../*.whl) && lipo -archs \$(find $S/pydist-x86/unz -name '*.so') | grep -qx x86_64 && echo 'OK: x86_64 macOS wheel built; extension is x86_64'"
want native-manylinux-x86_64 && WT="$S/src" "$N" native-manylinux-x86_64 "$S/manylinux.sh $S/src x86_64"
want native-mcp-image && "$N" native-mcp-image "docker buildx build -f mcp/kshana-mcp/Dockerfile --platform linux/arm64 -t kshana-mcp:local-dryrun --load . && out=\$(printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"smoke\",\"version\":\"0\"}}}' | docker run --rm -i kshana-mcp:local-dryrun 2>/dev/null) || true; printf '%s\\n' \"\$out\" | head -c 400; echo; docker image rm kshana-mcp:local-dryrun >/dev/null; printf '%s' \"\$out\" | grep -q serverInfo && echo 'OK: the image answers an MCP initialize over stdio'"
want native-mcp-image-amd64 && "$N" native-mcp-image-amd64 "docker buildx build -f mcp/kshana-mcp/Dockerfile --platform linux/amd64 -t kshana-mcp:local-dryrun --load . && out=\$(printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"smoke\",\"version\":\"0\"}}}' | docker run --rm -i --platform linux/amd64 kshana-mcp:local-dryrun 2>/dev/null) || true; printf '%s\\n' \"\$out\" | head -c 400; echo; docker image rm kshana-mcp:local-dryrun >/dev/null; printf '%s' \"\$out\" | grep -q serverInfo && echo 'OK: the image answers an MCP initialize over stdio'"
want native-mcp-registry-validate && "$N" native-mcp-registry-validate "d=$S/mcpreg; rm -rf \$d; mkdir -p \$d; cp mcp/kshana-mcp/server.json \$d/; cd \$d; V=\$(grep -m1 '^version' $WT/Cargo.toml | sed 's/.*\"\\(.*\\)\".*/\\1/'); jq --arg v \$V --arg img ghcr.io/ashfordeou/kshana-mcp:\$V '.version = \$v | .packages[0].identifier = \$img' server.json > s.tmp && mv s.tmp server.json && $S/tools/mcp-publisher validate"
want actionlint && "$N" actionlint 'actionlint'

# --- the multi-hour jobs last ---
if ! slow ci-check && want ci-check; then "$A" ci-check .github/workflows/ci.yml push check; fi
if ! slow release-verify-amd64 && want release-verify-amd64; then AMD64 "$A" release-verify-amd64 "$S/wf/release-verify.yml" push verify --eventpath "$S/event-tag.json"; fi
if ! slow ci-coverage && want ci-coverage; then "$A" ci-coverage .github/workflows/ci.yml push coverage; fi

cp "$S/results.tsv" "$S/results-$short.tsv"
echo; echo "== $short =="; column -t -s $'\t' "$S/results.tsv" | cut -c1-110
red=$(awk -F'\t' '$3!=0' "$S/results.tsv" | wc -l | tr -d ' ')
[ "$red" = 0 ] && echo "ALL GREEN ($(wc -l < "$S/results.tsv" | tr -d ' ') entries)" || echo "RED: $red entr(y/ies)"
case "$0" in *.rerun-snapshot-*) rm -f "$0" ;; esac
[ "$red" = 0 ]
