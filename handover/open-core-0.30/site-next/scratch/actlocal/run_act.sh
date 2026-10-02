#!/usr/bin/env bash
# Run one GitHub Actions job locally under act, log it, and record exit + seconds.
# usage: run_act.sh <name> <workflow-file> <event> <job> [extra act args...]
# SRC (default: scratch/actlocal/src) is the checkout act copies into the container.
set -uo pipefail
S="$HOME/Code/kshana-site-next/scratch/actlocal"
SRC="${SRC:-$S/src}"
name="$1"; wf="$2"; ev="$3"; job="$4"; shift 4
mkdir -p "$S/logs" "$S/artifacts" "$S/cache"
export DOCKER_CONFIG="$S/dockercfg"   # {} + cli-plugins symlink: avoids the credsStore hang
evfile="$S/event-$ev.json"
[ -f "$evfile" ] || evfile=""
freeport() { python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1])"; }
# own ports per run, so two act runs can overlap (the default 34567 collides)
aport=$(freeport); cport=$(freeport)
start=$(date +%s)
(
  cd "$SRC" || exit 99
  /opt/homebrew/bin/act "$ev" \
    -W "$wf" -j "$job" \
    --container-architecture "${ACT_ARCH:-linux/arm64}" \
    -P ubuntu-latest=${ACT_IMAGE:-kshana-act:ubuntu-latest} \
    --pull=false \
    --rm \
    --env CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}" \
    --env BINARYEN_CORES="${BINARYEN_CORES:-4}" \
    --artifact-server-path "$S/artifacts" \
    --cache-server-path "$S/cache" \
    --artifact-server-port "$aport" \
    --cache-server-port "$cport" \
    ${evfile:+--eventpath "$evfile"} \
    "$@"
) > "$S/logs/$name.log" 2>&1
rc=$?
dur=$(( $(date +%s) - start ))
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "act" "$rc" "$dur" "$(git -C "$SRC" rev-parse --short HEAD)$(git -C "$SRC" diff --quiet HEAD || echo +picked)" "act $ev -W $wf -j $job $*" >> "$S/results.tsv"
echo "$name rc=$rc ${dur}s"
exit $rc
