#!/usr/bin/env bash
# Look up a private term by the ID the protection checks print (roadmap 0.42 X4, X5).
#
#   scripts/protect/term-id.sh <ID>      print the section and term with that ID
#   scripts/protect/term-id.sh --all     print every ID, section and term
#
# Run it locally only: its output names private terms. Never paste it into a public place.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
TERMS="${KSHANA_PRO_PRIVATE_TERMS:-$here/../../protect/private-terms.txt}"
[ $# -eq 1 ] || { echo "usage: term-id.sh <ID> | --all" >&2; exit 2; }
section=""
found=1
while IFS= read -r line; do
  case "$line" in
    ''|'#'*) continue ;;
    '['*']') section="${line#[}"; section="${section%]}"; continue ;;
  esac
  id=$(printf '%s' "$line" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12)
  if [ "$1" = "--all" ] || [ "$1" = "$id" ]; then
    printf '%s  [%s]  %s\n' "$id" "$section" "$line"
    found=0
  fi
done < "$TERMS"
exit $found
