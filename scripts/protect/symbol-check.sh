#!/usr/bin/env bash
# Roadmap 0.42 X4: no Arksa-exclusive or Core name in a shipped build.
#
#   scripts/protect/symbol-check.sh [--require-stripped] <binary|bundle dir|file> [...]
#
# Scans every file given (a directory is scanned recursively: a macOS .app, an unpacked
# installer) for the [arksa] and [core] terms of protect/private-terms.txt, as fixed
# strings in the raw bytes (symbols, string literals, panic paths, resources). Fails when
# any appears. With --require-stripped it also fails when an executable still carries a
# symbol table naming the overlay (release profiles strip symbols; see Cargo.toml).
#
# A match is reported by term ID (first 12 hex characters of the term's SHA-256), never by
# the term, so the private list does not reach a log. Look an ID up locally with
#   scripts/protect/term-id.sh <ID>
#
# Run it on the default release build, Desktop Pro and every package before release. It
# is meant to FAIL on a build with the `arksa` feature (tests/protect_scripts.rs checks
# that it does), so the term list keeps matching what the Arksa-exclusive code contains.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
TERMS="${KSHANA_PRO_PRIVATE_TERMS:-$here/../../protect/private-terms.txt}"
SECTIONS="${KSHANA_PRO_SYMBOL_SECTIONS:-arksa core}"

require_stripped=0
if [ "${1:-}" = "--require-stripped" ]; then require_stripped=1; shift; fi
[ $# -ge 1 ] || { echo "usage: symbol-check.sh [--require-stripped] <binary|dir> [...]" >&2; exit 2; }
[ -f "$TERMS" ] || { echo "symbol-check: no term list at $TERMS" >&2; exit 2; }

# The terms of the chosen sections, one per line.
terms_file="$(mktemp)"; trap 'rm -f "$terms_file"' EXIT
awk -v want=" $SECTIONS " '
  /^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
  /^\[.*\]$/ { s = substr($0, 2, length($0) - 2); on = index(want, " " s " ") > 0; next }
  on { print }
' "$TERMS" > "$terms_file"
n_terms=$(wc -l < "$terms_file" | tr -d ' ')
[ "$n_terms" -gt 0 ] || { echo "symbol-check: no terms in sections: $SECTIONS" >&2; exit 2; }

term_id() { printf '%s' "$1" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12; }

files=()
for p in "$@"; do
  if [ -d "$p" ]; then
    while IFS= read -r -d '' f; do files+=("$f"); done < <(find "$p" -type f -print0)
  elif [ -f "$p" ]; then
    files+=("$p")
  else
    echo "symbol-check: no such file or directory: $p" >&2; exit 2
  fi
done

bad=0
for f in "${files[@]}"; do
  hits=()
  while IFS= read -r t; do
    [ -n "$t" ] && hits+=("$(term_id "$t")")
  done < <(LC_ALL=C grep -a -o -F -f "$terms_file" -- "$f" | LC_ALL=C sort -u || true)
  if [ ${#hits[@]} -gt 0 ]; then
    echo "FAIL: $f carries ${#hits[@]} private term(s): ${hits[*]}"
    bad=1
  fi
  if [ $require_stripped -eq 1 ] && command -v nm >/dev/null 2>&1; then
    # An executable or library whose symbol table still names the overlay is not stripped.
    if file "$f" 2>/dev/null | grep -Eq 'ELF|Mach-O|PE32'; then
      syms=$(nm "$f" 2>/dev/null | grep -c 'kshana_pro' || true)
      if [ "${syms:-0}" -gt 0 ]; then
        echo "FAIL: $f is not stripped ($syms symbols name the overlay)"
        bad=1
      fi
    fi
  fi
done

if [ $bad -eq 0 ]; then
  echo "symbol-check: OK (${#files[@]} file(s), $n_terms term(s) in: $SECTIONS)"
fi
exit $bad
