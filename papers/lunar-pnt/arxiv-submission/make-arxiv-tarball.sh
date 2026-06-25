#!/usr/bin/env bash
# Assemble a self-contained arXiv source package for the lunar-PNT paper.
#
# This directory is already a flat, self-contained copy: lunar-pnt.tex with
# \graphicspath{{./}}, the sections/ tree, the single lunar-pnt.bib, and the
# five figure PDFs all live at the archive root. arXiv needs everything in one
# tree, so this script precompiles the bibliography to a .bbl (arXiv does not
# have to run BibTeX when the .bbl ships), verifies the package builds
# standalone, and tars it with lunar-pnt.tex at the archive root.
#
# Usage:  bash make-arxiv-tarball.sh
# Output: dist/lunar-pnt-arxiv-src.tar.gz  (+ a verified build under build/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"

build="$here/build"
dist="$here/dist"
pkg="$build/pkg"
rm -rf "$build" "$dist"
mkdir -p "$pkg/sections" "$dist"

figs=(fig1_headline.pdf fig2_sigma_vlbi.pdf fig3_n_earth.pdf \
      fig4_seed_mc.pdf fig5_cross_suite.pdf)

# 1. Sources (already flattened; copy verbatim).
cp lunar-pnt.tex  "$pkg/lunar-pnt.tex"
cp sections/*.tex "$pkg/sections/"
cp lunar-pnt.bib  "$pkg/"
for f in "${figs[@]}"; do cp "$f" "$pkg/"; done

# 2. Precompile the bibliography to a .bbl from the flattened sources.
( cd "$pkg" && tectonic --keep-intermediates lunar-pnt.tex >/dev/null 2>&1 )
test -s "$pkg/lunar-pnt.bbl" || { echo "FAIL: no lunar-pnt.bbl produced"; exit 1; }
# Drop intermediates that must not ship (keep .bbl, .tex, .bib, .pdf figures).
( cd "$pkg" && rm -f lunar-pnt.aux lunar-pnt.log lunar-pnt.out lunar-pnt.blg lunar-pnt.pdf )

# 3. Verify the package builds standalone from a pristine copy.
verify="$build/verify"
cp -R "$pkg" "$verify"
( cd "$verify" && tectonic lunar-pnt.tex >/dev/null 2>&1 ) || { echo "FAIL: standalone build"; exit 1; }
pages="$(command -v pdfinfo >/dev/null && pdfinfo "$verify/lunar-pnt.pdf" | awk '/Pages/{print $2}' || echo '?')"
rm -f "$verify"/lunar-pnt.pdf "$verify"/lunar-pnt.bbl  # verify dir is throwaway

# 4. Tar with lunar-pnt.tex at the archive root (arXiv detects the main file by \documentclass).
#    COPYFILE_DISABLE=1 stops macOS bsdtar from bundling ._* AppleDouble metadata files,
#    which arXiv would otherwise have to strip on upload.
( cd "$pkg" && COPYFILE_DISABLE=1 tar czf "$dist/lunar-pnt-arxiv-src.tar.gz" \
    lunar-pnt.tex lunar-pnt.bbl lunar-pnt.bib "${figs[@]}" sections )

echo "OK: dist/lunar-pnt-arxiv-src.tar.gz  (standalone build: ${pages} pp)"
echo "Contents:"
tar tzf "$dist/lunar-pnt-arxiv-src.tar.gz" | sed 's/^/  /'
