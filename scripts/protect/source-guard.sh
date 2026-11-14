#!/usr/bin/env bash
# Roadmap 0.42 X3: this repository's own source carries no Arksa-exclusive code.
#
#   scripts/protect/source-guard.sh [--dir <repository>]
#
# The Arksa-exclusive kinds and commands moved to a private server-side crate that depends
# on this one. The dependency runs one way only: nothing here may name that crate or bring
# its code back. This guard scans every tracked file of the source scope below (Rust
# sources, manifests, lockfiles, tests, fixtures, scenarios and the Desktop Pro app) for
#
#   * every [arksa] and [core] term of protect/private-terms.txt (fixed strings), and
#   * the server-side crate's name, in either spelling (built below, so this file does not
#     match itself),
#
# and fails on any match. Documentation (docs/, papers/, *.md) is out of scope: the private
# docs of this repository may name what moved. A match is printed as "<path>  term <ID>"
# (first 12 hex characters of the term's SHA-256), never the term itself; look an ID up
# locally with scripts/protect/term-id.sh <ID>.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
TERMS="${KSHANA_PRO_PRIVATE_TERMS:-$here/../../protect/private-terms.txt}"
dir="$here/../.."
if [ "${1:-}" = "--dir" ]; then dir="${2:?--dir needs a path}"; shift 2; fi
[ $# -eq 0 ] || { echo "usage: source-guard.sh [--dir <repository>]" >&2; exit 2; }
[ -f "$TERMS" ] || { echo "source-guard: no term list at $TERMS" >&2; exit 2; }

terms_file="$(mktemp)"; trap 'rm -f "$terms_file"' EXIT
awk '
  /^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
  /^\[.*\]$/ { s = substr($0, 2, length($0) - 2); on = (s == "arksa" || s == "core"); next }
  on { print }
' "$TERMS" > "$terms_file"
crate="arksa"; printf '%s\n%s\n' "$crate-kpro" "${crate}_kpro" >> "$terms_file"
n_terms=$(wc -l < "$terms_file" | tr -d ' ')

term_id() { printf '%s' "$1" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12; }

# The source scope: code, manifests, lockfiles, tests, fixtures, scenarios, examples, the
# Desktop Pro app. Markdown and other documentation are left out on purpose.
scope=(
  'Cargo.toml' 'Cargo.lock' 'build.rs' '.cargo/*'
  'src/*' 'tests/*' 'benches/*' 'examples/*' 'scenarios/*'
  'apps/*/src-tauri/src/*' 'apps/*/src-tauri/Cargo.toml' 'apps/*/src-tauri/Cargo.lock'
  'apps/*/src-tauri/build.rs'
)
files=()
while IFS= read -r -d '' f; do
  case "$f" in *.md) continue ;; esac
  files+=("$f")
done < <(git -C "$dir" ls-files -z -- "${scope[@]}")

bad=0
for f in "${files[@]}"; do
  # A tracked path that itself names a term (e.g. a moved module's path).
  while IFS= read -r t; do
    [ -n "$t" ] && { echo "FAIL: path $f  term $(term_id "$t")"; bad=1; }
  done < <(printf '%s\n' "$f" | LC_ALL=C grep -o -F -f "$terms_file" | LC_ALL=C sort -u || true)
  [ -f "$dir/$f" ] || continue
  while IFS= read -r t; do
    [ -n "$t" ] && { echo "FAIL: $f  term $(term_id "$t")"; bad=1; }
  done < <(LC_ALL=C grep -a -o -F -f "$terms_file" -- "$dir/$f" | LC_ALL=C sort -u || true)
done

if [ $bad -eq 0 ]; then
  echo "source-guard: OK (${#files[@]} file(s), $n_terms term(s))"
fi
exit $bad
