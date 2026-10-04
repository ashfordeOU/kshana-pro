#!/usr/bin/env bash
cd "$(dirname "$0")"
for j in package msrv wasm-registry mcp-check public-standalone test-wasm-bindings; do
  ./run_act.sh "ci-$j" .github/workflows/ci.yml push "$j"
done
echo BATCH1_DONE
