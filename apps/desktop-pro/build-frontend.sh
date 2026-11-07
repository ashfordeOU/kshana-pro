#!/usr/bin/env bash
# PROPRIETARY — Kshana Pro overlay.
# Stage the frontend of Kshana Desktop Pro: the open Studio (with its Desktop client,
# web/studio/lib/desktop.mjs) from a checkout of the open repository, plus the Pro pages.
#   apps/desktop-pro/build-frontend.sh <open repository checkout>
set -euo pipefail
OPEN=${1:?usage: build-frontend.sh <open repository checkout>}
cd "$(dirname "$0")"
[ -f "$OPEN/web/studio/lib/desktop.mjs" ] || { echo "$OPEN has no web/studio/lib/desktop.mjs (the open Desktop patch)" >&2; exit 1; }
rm -rf dist && mkdir -p dist
(cd "$OPEN/web" && tar --exclude='*.test.mjs' -cf - studio) | (cd dist && tar -xf -)
cp -r pro dist/pro
cat > dist/index.html <<'HTML'
<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="0; url=pro/licence.html"><title>Kshana Desktop Pro</title>
HTML
echo "staged dist/ from $OPEN"
