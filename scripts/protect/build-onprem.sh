#!/usr/bin/env bash
# Build the hardened on-prem binary carrying the patent-candidate methods and verify it is
# safe to ship (owner decision 19 November 2026). This is the ONLY build that includes the
# `unfiled-methods` methods; it goes to vetted, contracted, machine-bound customers, never
# to the public or Desktop. See docs/OFFLINE-PROTECTION.md.
#
#   scripts/protect/build-onprem.sh [--out <dir>]
#
# Builds `kshana-pro` with the `release-onprem` profile (fat LTO, one codegen unit,
# panic=abort, stripped; see Cargo.toml) and the `unfiled-methods` feature, then runs the
# binary check (scripts/protect/symbol-check.sh --require-stripped): it must carry no
# Arksa-exclusive or Core name and no symbol table. The methods themselves are node-locked
# at runtime (require_bound): they run only under a machine-bound pro-eloran licence.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"

out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="${2:?--out needs a dir}"; shift 2;;
    *) echo "usage: build-onprem.sh [--out <dir>]" >&2; exit 2;;
  esac
done

target="${CARGO_TARGET_DIR:-$root/target}"
echo "build-onprem: cargo build --profile release-onprem --features unfiled-methods -p kshana-pro --bin kshana-pro"
cargo build --profile release-onprem --features unfiled-methods -p kshana-pro --bin kshana-pro

bin="$target/release-onprem/kshana-pro"
[ -x "$bin" ] || { echo "build-onprem: binary not found at $bin" >&2; exit 1; }

echo "build-onprem: symbol-check --require-stripped"
"$here/symbol-check.sh" --require-stripped "$bin"

sha="$(sha256sum "$bin" | cut -d' ' -f1)"
echo "build-onprem: OK  $bin"
echo "build-onprem: sha256  $sha"

if [ -n "$out" ]; then
  mkdir -p "$out"
  cp "$bin" "$out/"
  echo "$sha  kshana-pro" > "$out/kshana-pro.sha256"
  echo "build-onprem: copied to $out/kshana-pro (+ .sha256)"
fi
