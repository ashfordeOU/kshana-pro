#!/usr/bin/env bash
# Build the per-day interference-map layers (kshana-interference-map/v1) that a regional
# exposure dataset (`kshana-pro briefing regional`), a briefing or a transit history reads,
# with the open engine. Nothing is downloaded here: the raw ADS-B or AIS files are fetched by
# the user under their own terms, and derived layers may be redistributed only where the
# source licence allows it (otherwise ship this script and the request, not the layers).
#
#   scripts/exposure/build-regional-inputs.sh <adsb|ais> <raw-file-or-dir> <dataset> <out-dir>
#
#   <dataset>  adsb: adsb-lol | custom; ais: noaa-marinecadastre | kystverket | custom
#              (custom also needs KSHANA_LICENCE, KSHANA_LICENCE_URL, KSHANA_ATTRIBUTION)
#
# The open engine writes one map per UTC day (<source>-<dataset>-<YYYY-MM-DD>.geojson) on the
# pre-registered 0.5 degree grid; list <out-dir> under `layers = [...]` in the request. The
# observatory's daily product (`kshana interference-map daily`) is a different format and is
# not read as a layer.
set -euo pipefail
[ $# -eq 4 ] || { sed -n '2,17p' "$0" >&2; exit 2; }
kind="$1"; raw="$2"; dataset="$3"; out="$4"
case "$kind" in adsb|ais) ;; *) echo "source must be adsb or ais" >&2; exit 2 ;; esac
command -v kshana >/dev/null || { echo "the open engine (kshana 0.41.0) is not on PATH" >&2; exit 2; }
extra=()
if [ "$dataset" = custom ]; then
  extra=(--licence "${KSHANA_LICENCE:?}" --licence-url "${KSHANA_LICENCE_URL:?}" --attribution "${KSHANA_ATTRIBUTION:?}")
fi
mkdir -p "$out"
kshana interference-map "$kind" "$raw" --dataset "$dataset" --out "$out" --cell-deg 0.5 "${extra[@]}"
ls "$out"/*.geojson
