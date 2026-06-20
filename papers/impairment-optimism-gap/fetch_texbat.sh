#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
#
# Fetch TEXBAT raw-IF scenarios and run the kshana SDR feature-stage optimism-gap probe.
#
# WHY THIS IS A SCRIPT, NOT A RESULT IN THE PAPER: the TEXBAT data host
# (rnl-data.ae.utexas.edu) refuses connections from many networks (it reset every
# request from the build sandbox, though the radionavlab.ae.utexas.edu landing page is
# reachable). Run this from a network that can reach the UT data store. No login or
# EULA is required, but the files are LARGE: each scenario is complex int16 I/Q at
# 25 Msps for ~420 s, about 42 GB per file. The default selection below is ~168 GB;
# trim SCENARIOS if you only want a quick probe.
#
# Usage:
#   ./fetch_texbat.sh /path/to/texbat_dir [window_sec]
#
# After download it runs:
#   cargo run --release --example texbat_probe -- \
#       <dir>/texbat-probe-result.json 25000000 <window_sec> 0 \
#       nominal:cleanStatic.bin p10:ds2.bin p1_3:ds3.bin p0_4:ds4.bin
# (cleanStatic = authentic; ds2 ~10 dB / ds3 ~1.3 dB / ds4 ~0.4 dB spoofer power
#  advantage = the graded severity axis; window_sec defaults to 8.)
set -euo pipefail

DIR="${1:?usage: fetch_texbat.sh <dir> [window_sec]}"
WINDOW="${2:-8}"
BASE="https://rnl-data.ae.utexas.edu/datastore/texbat"
# Authentic baseline + the three matched/graded-power time-push scenarios.
SCENARIOS=(cleanStatic ds2 ds3 ds4)

mkdir -p "$DIR"
cd "$DIR"
echo ">> TEXBAT fetch into $DIR (each file ~42 GB)"
for s in "${SCENARIOS[@]}"; do
  if [[ -f "$s.bin" ]]; then
    echo "   $s.bin present, skipping"
  else
    echo "   downloading $s.bin ..."
    # -C - resumes a partial download if interrupted.
    curl -fL -C - -o "$s.bin" "$BASE/$s.bin"
  fi
done
# Optional MATLAB reader, for reference.
[[ -f scott.m ]] || curl -fL -o scott.m "$BASE/scott.m" || true

# Locate the kshana crate (this script lives in kshana-pro/papers/impairment-optimism-gap).
KSHANA="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../kshana" && pwd)"
echo ">> running the SDR feature-stage probe (window ${WINDOW}s) ..."
( cd "$KSHANA" && cargo run --release --example texbat_probe -- \
    "$DIR/texbat-probe-result.json" 25000000 "$WINDOW" 0 \
    "nominal:$DIR/cleanStatic.bin" "p10:$DIR/ds2.bin" "p1_3:$DIR/ds3.bin" "p0_4:$DIR/ds4.bin" )
echo ">> done -> $DIR/texbat-probe-result.json"
