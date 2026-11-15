#!/usr/bin/env bash
# Roadmap 0.42 C10: the macOS signing hook for a Pro pack payload, run by the release
# pipeline BEFORE `kshana-pro packs build --platform macos-*`. The free Kshana Desktop app
# runs with library validation, so it loads only native code signed by the same Apple team:
# every Mach-O file of the payload is signed with our Developer ID and Team ID here.
#
#   scripts/packs/sign-macos-payload.sh <payload dir>
#
# Uses the existing pipeline secrets only (no new secret): the signing identity must already
# be in the runner's keychain (imported from APPLE_CERTIFICATE / APPLE_CERTIFICATE_PASSWORD by
# the workflow, as in desktop-pro-release.yml), and APPLE_TEAM_ID names the team. Pass the
# same team ID to `packs build --apple-team-id "$APPLE_TEAM_ID"`; the signed pack manifest
# records it, and the Desktop host refuses a payload signed by any other team.
set -euo pipefail
dir="${1:?usage: sign-macos-payload.sh <payload dir>}"
: "${APPLE_TEAM_ID:?set APPLE_TEAM_ID (pipeline secret)}"
[ "$(uname)" = "Darwin" ] || { echo "sign-macos-payload: run on macOS" >&2; exit 2; }
ID="$(security find-identity -v -p codesigning | awk -F'"' -v t="($APPLE_TEAM_ID)" 'index($2, "Developer ID Application") && index($2, t) {print $2; exit}')"
[ -n "$ID" ] || { echo "sign-macos-payload: no Developer ID Application identity for the team in the keychain" >&2; exit 1; }
n=0
while IFS= read -r -d '' f; do
  if file -b "$f" | grep -q "Mach-O"; then
    codesign --force --timestamp --options runtime --sign "$ID" "$f"
    codesign --verify --strict "$f"
    got="$(codesign -dv "$f" 2>&1 | awk -F= '/^TeamIdentifier=/{print $2}')"
    [ "$got" = "$APPLE_TEAM_ID" ] || { echo "sign-macos-payload: $f signed by another team" >&2; exit 1; }
    n=$((n + 1))
  fi
done < <(find "$dir" -type f -print0)
echo "sign-macos-payload: signed and verified $n Mach-O file(s) for team $APPLE_TEAM_ID"
