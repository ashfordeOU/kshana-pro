#!/usr/bin/env bash
# Roadmap 0.42 X5: the leak gate. Run from this PRIVATE repository against the OPEN one.
#
#   scripts/protect/open-leak-gate.sh [--url <open repo URL>] [--ref <branch|tag>]
#                                     [--dir <existing checkout>] [--baseline <file>]
#                                     [--write-baseline]
#
# Fetches the open repository read-only (a shallow clone into a temporary directory; no
# credentials, nothing is ever pushed) or takes an existing checkout (--dir), and scans
# every tracked text file and every tracked path for the terms of protect/private-terms.txt
# (all sections: Arksa-exclusive, Core, pending and Pro module names and method terms).
#
# Matches already in the baseline (protect/open-leak-baseline.txt: text that was published
# before this gate existed, which stays public) are accepted. Any other match fails the
# gate (exit 1). A match is reported as "<open-repo path>  <term ID>": the path is public
# already; the term is never printed (scripts/protect/term-id.sh looks an ID up locally).
#
# --write-baseline rewrites the baseline from the current matches. Use it only after the
# owner has checked that every new match is meant to be public.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
TERMS="${KSHANA_PRO_PRIVATE_TERMS:-$here/../../protect/private-terms.txt}"
BASELINE="$here/../../protect/open-leak-baseline.txt"
URL="https://github.com/ashfordeOU/kshana"
REF="main"
DIR=""
write=0
while [ $# -gt 0 ]; do
  case "$1" in
    --url) URL="$2"; shift 2 ;;
    --ref) REF="$2"; shift 2 ;;
    --dir) DIR="$2"; shift 2 ;;
    --baseline) BASELINE="$2"; shift 2 ;;
    --write-baseline) write=1; shift ;;
    *) echo "usage: open-leak-gate.sh [--url U] [--ref R] [--dir D] [--baseline F] [--write-baseline]" >&2; exit 2 ;;
  esac
done
[ -f "$TERMS" ] || { echo "leak-gate: no term list at $TERMS" >&2; exit 2; }
printf '%s' "$REF" | grep -Eq '^[A-Za-z0-9._/-]+$' || { echo "leak-gate: bad ref" >&2; exit 2; }

work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
if [ -z "$DIR" ]; then
  DIR="$work/open"
  # Read-only: an anonymous shallow clone of one ref. Credentials are cleared so a runner's
  # stored token can never be used against the open repository.
  GIT_TERMINAL_PROMPT=0 git -c credential.helper= clone --quiet --depth 1 --branch "$REF" -- "$URL" "$DIR"
fi
git -C "$DIR" rev-parse --git-dir >/dev/null 2>&1 || { echo "leak-gate: $DIR is not a git checkout" >&2; exit 2; }
head_sha=$(git -C "$DIR" rev-parse HEAD)

# Every term (all sections), one per line.
awk '/^[[:space:]]*#/ || /^[[:space:]]*$/ || /^\[.*\]$/ { next } { print }' "$TERMS" > "$work/terms"
n_terms=$(wc -l < "$work/terms" | tr -d ' ')

term_id() { printf '%s' "$1" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12; }

: > "$work/found"
# Contents: git grep prints "<path>NUL<match>" per match (-z, --only-matching), turned into
# "<path>TAB<match>" (a NUL cannot be held in a shell variable).
git -C "$DIR" grep -I -z -o -F -f "$work/terms" -- . 2>/dev/null \
  | tr '\0' '\t' | LC_ALL=C sort -u \
  | while IFS=$'\t' read -r path term; do
      printf '%s\t%s\n' "$path" "$(term_id "$term")"
    done >> "$work/found" || true
# Paths: a tracked file or directory named after a term.
git -C "$DIR" ls-files | LC_ALL=C grep -F -f "$work/terms" | while IFS= read -r path; do
  while IFS= read -r t; do
    case "$path" in *"$t"*) printf '%s\t%s\n' "path:$path" "$(term_id "$t")" ;; esac
  done < "$work/terms"
done >> "$work/found" || true
LC_ALL=C sort -u "$work/found" -o "$work/found"

if [ $write -eq 1 ]; then
  {
    echo "# Open-repository matches accepted by the leak gate (roadmap 0.42 X5): text published"
    echo "# before the gate existed, which stays public. <path><TAB><term ID>. Regenerate only"
    echo "# with scripts/protect/open-leak-gate.sh --write-baseline after the owner checks it."
    echo "# Open repository HEAD when written: $head_sha"
    cat "$work/found"
  } > "$BASELINE"
  echo "leak-gate: baseline written ($(wc -l < "$work/found" | tr -d ' ') match(es))"
  exit 0
fi

grep -v '^#' "$BASELINE" 2>/dev/null | LC_ALL=C sort -u > "$work/base" || : > "$work/base"
LC_ALL=C comm -23 "$work/found" "$work/base" > "$work/new"
LC_ALL=C comm -13 "$work/found" "$work/base" > "$work/gone"
n_new=$(wc -l < "$work/new" | tr -d ' ')
n_base=$(wc -l < "$work/base" | tr -d ' ')
n_gone=$(wc -l < "$work/gone" | tr -d ' ')
echo "leak-gate: open repository at $head_sha; $n_terms private term(s); $n_base accepted match(es) in the baseline, $n_gone of them no longer present"
if [ "$n_new" -gt 0 ]; then
  echo "leak-gate: FAIL: $n_new new match(es) of private terms in the open repository:"
  sed 's/\t/  term /' "$work/new"
  exit 1
fi
echo "leak-gate: OK (no new private term in the open repository)"
