#!/bin/sh
# Build the dossier: HTML, a first PDF to find the chapter pages, the HTML again with the contents page numbers,
# the final PDF, then the PDF metadata.  Usage: ./make.sh <path to a kshana checkout at v0.40.0>
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
SRC=$1
OUT="$HERE/../Kshana-capability-dossier-v0.40.0.pdf"
TMP=$(mktemp -d)
python3 "$HERE/build.py" "$SRC"
node "$HERE/render.mjs" "$HERE/Kshana-capability-dossier-v0.40.0.html" "$TMP/pass1.pdf"
python3 "$HERE/finish.py" toc "$TMP/pass1.pdf" "$TMP/toc.json"
python3 "$HERE/build.py" "$SRC" --toc "$TMP/toc.json"
node "$HERE/render.mjs" "$HERE/Kshana-capability-dossier-v0.40.0.html" "$TMP/pass2.pdf"
python3 "$HERE/finish.py" check "$TMP/pass2.pdf" "$TMP/toc.json"
python3 "$HERE/finish.py" meta "$TMP/pass2.pdf" "$OUT"
rm -rf "$TMP"
