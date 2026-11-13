#!/usr/bin/env bash
# PROPRIETARY — Kshana Pro overlay.
# Stage the frontend of Kshana Desktop Pro: the open Studio (with its Desktop client,
# web/studio/lib/desktop.mjs) from a checkout of the open repository, plus the Pro pages.
#   apps/desktop-pro/build-frontend.sh <open repository checkout>
#
# When the checkout carries the open Desktop shell (apps/desktop/shell, 0.42 U1), it is staged
# unchanged as studio/desktop/ (as the open apps/desktop/build-frontend.sh does) and the Pro
# overlay (pro/desktop/overlay.mjs and pro.css) is added to the staged copy of its index.html:
# the window opens the shell with the Pro home. Without the shell (an older open tag), the
# window opens the standalone Pro home, pro/home.html.
set -euo pipefail
OPEN=${1:?usage: build-frontend.sh <open repository checkout>}
cd "$(dirname "$0")"
[ -f "$OPEN/web/studio/lib/desktop.mjs" ] || { echo "$OPEN has no web/studio/lib/desktop.mjs (the open Desktop patch)" >&2; exit 1; }
rm -rf dist && mkdir -p dist
(cd "$OPEN/web" && tar --exclude='*.test.mjs' -cf - studio) | (cd dist && tar -xf -)
[ -d "$OPEN/web/fonts" ] && (cd "$OPEN/web" && tar -cf - fonts) | (cd dist && tar -xf -)
(tar --exclude='*.test.mjs' -cf - pro) | (cd dist && tar -xf -)

first=pro/home.html
SHELL_DIR="$OPEN/apps/desktop/shell"
if [ -f "$SHELL_DIR/index.html" ] && [ -f "$SHELL_DIR/shell.js" ]; then
  mkdir -p dist/studio/desktop
  (cd "$SHELL_DIR" && tar --exclude='*.test.mjs' -cf - .) | (cd dist/studio/desktop && tar -xf -)
  page=dist/studio/desktop/index.html
  if grep -q '</head>' "$page" && grep -q '</body>' "$page"; then
    sed -e 's#</head>#<link rel="stylesheet" href="../../pro/desktop/pro.css"></head>#' \
        -e 's#</body>#<script type="module" src="../../pro/desktop/overlay.mjs"></script></body>#' \
        -e 's#<title>Kshana Desktop</title>#<title>Kshana Desktop Pro</title>#' "$page" > "$page.pro"
    mv "$page.pro" "$page"
    grep -q 'pro/desktop/overlay.mjs' "$page" || { echo "the Pro overlay was not added to $page" >&2; exit 1; }
    first=studio/desktop/index.html
    echo "open Desktop shell staged with the Pro overlay"
  else
    rm -rf dist/studio/desktop
    echo "the open shell's index.html has no </head> or </body>: standalone Pro home"
  fi
else
  echo "the open checkout has no Desktop shell (apps/desktop/shell): standalone Pro home"
fi
cat > dist/index.html <<HTML
<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="0; url=$first"><title>Kshana Desktop Pro</title>
HTML
echo "staged dist/ from $OPEN (first page: $first)"
