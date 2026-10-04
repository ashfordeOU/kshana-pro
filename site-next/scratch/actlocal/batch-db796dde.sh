#!/usr/bin/env bash
# db796dde pass, priority order (parent 2026-10-01): pages first, full arm64 check skipped (gate.sh covers it).
cd "$(dirname "$0")"
export CARGO_BUILD_JOBS=4
R() { ONLY="$1" ./rerun.sh db796dde >/dev/null 2>&1; }
for e in actionlint pages-build ci-test-wasm-bindings ci-package ci-msrv ci-deny ci-mcp-deny ci-wasm-registry ci-mcp-check ci-public-standalone \
         ci-reproducibility-matrix-ubuntu xarch-pins-amd64 native-repro-matrix-macos native-manylinux-x86_64 native-wheel-macos \
         release-verify-amd64 \
         release-build publish-gates publish-npm-build jetbrains-build native-binaries-macos native-wheel-macos-x86_64 \
         native-mcp-image native-mcp-image-amd64 native-mcp-registry-validate; do
  R "$e"
done
echo BATCH_DONE
