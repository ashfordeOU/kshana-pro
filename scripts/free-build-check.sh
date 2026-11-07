#!/usr/bin/env bash
# Roadmap F10e: the free build ships no Pro code. Fails when any binary given carries a
# marker of the Pro overlay: the crate's own name in its symbols and panic paths, a Pro
# kind or feature name, or the proprietary notice of its sources.
#
#   scripts/free-build-check.sh <binary> [<binary> ...]
#
# Run on every free artefact before release (the free Kshana Desktop, the open CLI and
# Station). tests/free_build_check.rs runs it on the Pro binary, where it must fail, so
# the markers keep matching what a Pro build contains.
set -euo pipefail
[ $# -ge 1 ] || { echo "usage: free-build-check.sh <binary> [<binary> ...]" >&2; exit 2; }
MARKERS='kshana_pro|kshana-pro|PROPRIETARY — Kshana Pro|pro-station-fleet|pro-signed-dossier|pro-spectrum-coexistence|pro-design-optimise|pro-uncertainty|pro-mission-dossier|pro-scenario-ci|pro-trade-study|pro-twin'
bad=0
for b in "$@"; do
  [ -f "$b" ] || { echo "no such file: $b" >&2; exit 2; }
  n=$(LC_ALL=C grep -a -o -E "$MARKERS" "$b" | sort | uniq -c | sort -rn || true)
  if [ -n "$n" ]; then
    echo "FAIL: $b carries Pro markers:"
    echo "$n" | head -n 12
    bad=1
  else
    echo "OK: $b carries no Pro marker"
  fi
done
exit $bad
