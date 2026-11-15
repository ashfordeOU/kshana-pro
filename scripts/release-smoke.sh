#!/usr/bin/env bash
# PRO-14: release build + signed-licence-token smoke test.
#
# Builds kshana-pro in debug and in release with a CI-only licence public key compiled in,
# issues a token with the matching CI-only seed, runs one Pro pack (campaign watch, a Pro
# tier feature) under that token with both binaries, and asserts the two reports are
# byte-identical. Also checks the release build refuses the pack without a token.
#
# Release preparation (roadmap S1, key custody S7-a/S7-b) adds, on the same release binary:
# a tampered token is refused; the build is told its
# root key (KSHANA_PRO_ROOT_KID), a revocation list signed by the issuing key that tries to
# retire a key is refused, and a root-signed list retiring the issuing key is accepted and
# then refuses that key's token (the rotation drill, on throwaway keys).
#
# The seeds below are throwaways for this test only: they are not vendor keys and sign
# nothing that ships. Run from the repository root.
set -euo pipefail
cd "$(dirname "$0")/.."

SEED=4b7368616e612d63692d736d6f6b652d6f6e6c792d6e6f742d612d76656e646f   # 32 bytes, CI only
ROOT_SEED=4b7368616e612d63692d736d6f6b652d726f6f742d6f6e6c792d746573742121   # 32 bytes, CI only
TGT="${CARGO_TARGET_DIR:-target}"
PACK=scenarios/packs/campaign-baseline/pack.toml
UTC=2026-09-30T00:00:00Z
OUT="${RELEASE_SMOKE_DIR:-target/release-smoke}"
rm -rf "$OUT"; mkdir -p "$OUT"

export KSHANA_PRO_SIGNING_KEY=$SEED
# An empty licence store, so no token activated on this machine is picked up.
export KSHANA_PRO_HOME="$OUT/licence-store"
# The public key comes from the debug binary built without a key (it only reads the seed).
cargo build --locked --bin kshana-pro
PUB=$("$TGT/debug/kshana-pro" license pubkey)
[ "${#PUB}" -eq 64 ] || { echo "FAIL: public key is not 64 hex characters" >&2; exit 1; }
ROOT_PUB=$(KSHANA_PRO_SIGNING_KEY=$ROOT_SEED "$TGT/debug/kshana-pro" license pubkey)
ROOT_KID=$(KSHANA_PRO_SIGNING_KEY=$ROOT_SEED "$TGT/debug/kshana-pro" license pubkey 2>&1 >/dev/null | sed -n 's/^key id: //p')
ISSUING_KID=$("$TGT/debug/kshana-pro" license pubkey 2>&1 >/dev/null | sed -n 's/^key id: //p')
[ "${#ROOT_KID}" -eq 16 ] && [ "${#ISSUING_KID}" -eq 16 ] || { echo "FAIL: key IDs" >&2; exit 1; }

# Rebuild both profiles with the keys compiled in (a build without one verifies no token):
# the issuing key signs tokens, the root key only manages keys (S7-b).
export KSHANA_PRO_LICENSE_PUBKEY=$PUB,$ROOT_PUB
export KSHANA_PRO_ROOT_KID=$ROOT_KID
cargo build --locked --bin kshana-pro
cargo build --release --locked --bin kshana-pro
cp "$TGT/debug/kshana-pro" "$OUT/kshana-pro-debug"
cp "$TGT/release/kshana-pro" "$OUT/kshana-pro-release"

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

REL="$OUT/kshana-pro-release"
# A tampered token is refused (one payload character changed).
BAD="$(printf '%s' "$TOKEN" | cut -c1)"; case "$BAD" in 7) BAD=8 ;; *) BAD=7 ;; esac
TAMPERED="$BAD$(printf '%s' "$TOKEN" | cut -c2-)"
if "$REL" license verify "$TAMPERED" >/dev/null 2>&1; then echo "FAIL: release build accepted a tampered token" >&2; exit 1; fi

# S7-b on the release binary: the issuing key may not retire keys; the root may.
REG="$OUT/register.jsonl"
"$REL" license issue --tier pro --licensee "CI smoke register" --days 1 --register "$REG" >/dev/null 2>&1 \
  || { echo "FAIL: could not issue into a register" >&2; exit 1; }
if "$REL" license revocations build --register "$REG" --out "$OUT/by-issuing.txt" --retire "$ROOT_KID" >/dev/null 2>&1; then
  echo "FAIL: the vendor tool built a key-retiring list with the issuing key" >&2; exit 1
fi
# Built anyway (root not declared to the tool): installed builds must still refuse it.
( unset KSHANA_PRO_ROOT_KID; "$REL" license revocations build --register "$REG" --out "$OUT/by-issuing.txt" --retire "$ROOT_KID" >/dev/null 2>&1 ) \
  || { echo "FAIL: could not build the test list" >&2; exit 1; }
if "$REL" license revocations install "$OUT/by-issuing.txt" >/dev/null 2>&1; then
  echo "FAIL: release build accepted a key-retiring list signed by the issuing key" >&2; exit 1
fi
KSHANA_PRO_SIGNING_KEY=$ROOT_SEED "$REL" license revocations build --register "$REG" --out "$OUT/by-root.txt" --retire "$ISSUING_KID" >/dev/null 2>&1 \
  || { echo "FAIL: the root could not build a key-retiring list" >&2; exit 1; }
"$REL" license revocations install "$OUT/by-root.txt" >/dev/null 2>&1 \
  || { echo "FAIL: release build refused the root-signed list" >&2; exit 1; }
if "$REL" license activate "$TOKEN" >/dev/null 2>&1; then
  echo "FAIL: a token of the retired issuing key still activates" >&2; exit 1
fi
echo "OK: release build, Pro token verified, release and debug reports identical ($(wc -c <"$OUT/ci-release.json") bytes); tampered token refused; only the root retires keys (rotation drill on throwaway keys)"
