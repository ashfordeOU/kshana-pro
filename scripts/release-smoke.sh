#!/usr/bin/env bash
# PRO-14: release build + signed-licence-token smoke test.
#
# Builds kshana-pro in debug and in release with a CI-only licence public key compiled in,
# issues a token with the matching CI-only seed, runs one Pro pack (campaign watch, a Pro
# tier feature) under that token with both binaries, and asserts the two reports are
# byte-identical. Also checks the release build refuses the pack without a token.
#
# The seed below is a throwaway for this test only: it is not the vendor key and signs
# nothing that ships. Run from the repository root.
set -euo pipefail
cd "$(dirname "$0")/.."

SEED=4b7368616e612d63692d736d6f6b652d6f6e6c792d6e6f742d612d76656e646f   # 32 bytes, CI only
PACK=scenarios/packs/campaign-baseline/pack.toml
UTC=2026-09-30T00:00:00Z
OUT="${RELEASE_SMOKE_DIR:-target/release-smoke}"
rm -rf "$OUT"; mkdir -p "$OUT"

export KSHANA_PRO_SIGNING_KEY=$SEED
# An empty licence store, so no token activated on this machine is picked up.
export KSHANA_PRO_HOME="$OUT/licence-store"
# The public key comes from the debug binary built without a key (it only reads the seed).
cargo build --locked --bin kshana-pro
PUB=$(target/debug/kshana-pro license pubkey)
[ "${#PUB}" -eq 64 ] || { echo "FAIL: public key is not 64 hex characters" >&2; exit 1; }

# Rebuild both profiles with the key compiled in (a build without one verifies no token).
export KSHANA_PRO_LICENSE_PUBKEY=$PUB
cargo build --locked --bin kshana-pro
cargo build --release --locked --bin kshana-pro
cp target/debug/kshana-pro "$OUT/kshana-pro-debug"
cp target/release/kshana-pro "$OUT/kshana-pro-release"

TOKEN=$("$OUT/kshana-pro-release" license issue --tier pro --licensee "CI smoke" --days 1 2>/dev/null | tail -n 1)
"$OUT/kshana-pro-release" license verify "$TOKEN" >/dev/null || { echo "FAIL: release build rejects the token" >&2; exit 1; }

for p in debug release; do
  "$OUT/kshana-pro-$p" ci --pack "$PACK" --utc "$UTC" --license "$TOKEN" \
    --json "$OUT/ci-$p.json" --out "$OUT/ci-$p.html" >"$OUT/ci-$p.log" 2>&1 \
    || { echo "FAIL: $p run under the Pro token failed" >&2; tail -n 20 "$OUT/ci-$p.log" >&2; exit 1; }
done

# Without a token the release build must refuse the Pro pack (fail closed).
if "$OUT/kshana-pro-release" ci --pack "$PACK" --utc "$UTC" --json "$OUT/ci-none.json" >"$OUT/ci-none.log" 2>&1; then
  echo "FAIL: release build ran a Pro pack with no token" >&2; exit 1
fi

cmp "$OUT/ci-debug.json" "$OUT/ci-release.json" \
  || { echo "FAIL: release and debug reports differ" >&2; diff "$OUT/ci-debug.json" "$OUT/ci-release.json" | head -n 40 >&2; exit 1; }
echo "OK: release build, Pro token verified, release and debug reports identical ($(wc -c <"$OUT/ci-release.json") bytes)"
