#!/usr/bin/env bash
# Layer guard: the Pro build must never link Kshana Core.
# Core (private repository kshana-core) runs only as an Ashforde service; it is never a
# dependency of this crate, never vendored here and never present in a Pro binary.
# Fails if any manifest, the lockfile, a vendored path or a built binary names it.
set -euo pipefail
cd "$(dirname "$0")/.."
pat='kshana[-_]core'
fail=0
hits=$(git ls-files -z -- 'Cargo.toml' '*/Cargo.toml' 'Cargo.lock' '.cargo/*' | xargs -0 grep -nEi "$pat" || true)
if [ -n "$hits" ]; then echo "core-guard: Core named in a manifest or lockfile:"; echo "$hits"; fail=1; fi
paths=$(git ls-files | grep -Ei "(^|/)$pat(/|$)" || true)
if [ -n "$paths" ]; then echo "core-guard: Core sources vendored:"; echo "$paths"; fail=1; fi
for b in target/release/kshana-pro target/debug/kshana-pro; do
  if [ -f "$b" ] && grep -aqEi "$pat" "$b"; then echo "core-guard: $b contains Core symbols"; fail=1; fi
done
[ $fail -eq 0 ] && echo "core-guard: OK (no Kshana Core in the Pro build)"
exit $fail
