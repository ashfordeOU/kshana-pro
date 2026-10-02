#!/usr/bin/env bash
# Run one workflow step set natively on this macOS arm64 host, log it, record exit + seconds.
# usage: run_native.sh <name> <command string>   (runs in $WT, default ~/Code/kshana-wt-actlocal)
set -uo pipefail
S="$HOME/Code/kshana-site-next/scratch/actlocal"
WT="${WT:-$HOME/Code/kshana-wt-actlocal}"
name="$1"; cmd="$2"
mkdir -p "$S/logs"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
start=$(date +%s)
( cd "$WT" && bash -euo pipefail -c "$cmd" ) > "$S/logs/$name.log" 2>&1
rc=$?
dur=$(( $(date +%s) - start ))
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "native" "$rc" "$dur" "$(git -C "$WT" rev-parse --short HEAD)$(git -C "$WT" diff --quiet HEAD || echo +picked)" "$cmd" >> "$S/results.tsv"
echo "$name rc=$rc ${dur}s"
exit $rc
