#!/usr/bin/env bash
# Reproduce the campaign-watch demonstration sequence and the bundled packs' first
# baseline reports.
#
#   scenarios/pro-power/watch-demo/run-demo.sh <output-dir> [kshana-pro binary]
#
# Run from the repository root after `cargo build` (the default binary is
# target/debug/kshana-pro; `--dev` is the development entitlement path and exists only
# in debug builds). Every run is real: the open engine runs each revision, the watch
# gates it against the last passing run, and the run is appended to one hash-chained
# history. Time stamps are the wall-clock time of each run.
set -euo pipefail

out="${1:?usage: run-demo.sh <output-dir> [kshana-pro binary]}"
bin="${2:-target/debug/kshana-pro}"
demo="scenarios/pro-power/watch-demo"
work="$out/work"
rm -rf "$out"
mkdir -p "$work" "$out/runs" "$out/packs"
cp "$demo/pack.toml" "$work/pack.toml"

log="$out/SHOWCASE.md"
{
  echo "# Campaign watch: demonstration sequence"
  echo
  echo "Made by \`$demo/run-demo.sh\` with \`$bin\` ($("$bin" version))."
  echo "Every run is a real open-engine run; the sequence of scenario revisions is a"
  echo "demonstration, labelled as such on every page."
  echo
  echo "| Run | Revision | Revision SHA-256 | Scenario hash | Seed | Engine | Exit | Command |"
  echo "|---|---|---|---|---|---|---|---|"
} > "$log"

last_pass=""
i=0
for rev in "$demo"/revisions/r*.toml; do
  name="$(basename "$rev" .toml)"
  cp "$rev" "$work/mission.toml"
  utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  cmd=("$bin" ci --pack "$work/pack.toml" --dev --utc "$utc"
       --json "$out/runs/$name.ci.json" --out "$out/runs/$name.report.html"
       --history "$out/history.jsonl" --trend "$out/watch.trend.html")
  if [ -n "$last_pass" ]; then
    cmd+=(--baseline "$last_pass")
  fi
  set +e
  "${cmd[@]}" 2> "$out/runs/$name.stderr.txt"
  code=$?
  set -e
  sha="$(shasum -a 256 "$rev" | cut -d' ' -f1)"
  shash="$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['scenario_hashes'][0])" "$out/runs/$name.ci.json")"
  engine="$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['engine_version'])" "$out/runs/$name.ci.json")"
  seed="$(grep -m1 '^seed = ' "$rev" | cut -d' ' -f3)"
  shown="${cmd[*]}"
  echo "| $i | \`$name\` | \`$sha\` | \`$shash\` | $seed | $engine | $code | \`${shown//|/\\|}\` |" >> "$log"
  if [ "$code" -eq 0 ]; then
    last_pass="$out/runs/$name.ci.json"
  fi
  i=$((i + 1))
done

# What moved into the regression, and what a documentation-only revision moved.
"$bin" ci diff "$out/runs/r3-weaker-jammer.ci.json" "$out/runs/r4-stronger-jammer.ci.json" \
  --json "$out/diff-r3-r4.json" --dev > "$out/diff-r3-r4.txt"
"$bin" ci diff "$out/runs/r1-published.ci.json" "$out/runs/r2-documentation.ci.json" \
  --json "$out/diff-r1-r2.json" --dev > "$out/diff-r1-r2.txt"
"$bin" ci history verify "$out/history.jsonl" --dev > "$out/history-verify.txt"

{
  echo
  echo "Diffs: \`$bin ci diff runs/r3-weaker-jammer.ci.json runs/r4-stronger-jammer.ci.json\`"
  echo "(diff-r3-r4.txt) and \`$bin ci diff runs/r1-published.ci.json runs/r2-documentation.ci.json\`"
  echo "(diff-r1-r2.txt). History check: \`$bin ci history verify history.jsonl\`:"
  echo "$(cat "$out/history-verify.txt")"
  echo
  echo "## The five bundled packs: first baseline reports"
  echo
  echo "| Pack | Exit | Engine | Scenario file SHA-256 (engine scenario hash where the kind states one) | Seed (top-level, per entry) | Command |"
  echo "|---|---|---|---|---|---|"
} >> "$log"

for pack in leo-pnt-baseline constellation-baseline spectrum-baseline campaign-baseline solar-system-baseline; do
  utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  cmd=("$bin" ci --pack "scenarios/packs/$pack/pack.toml" --dev --utc "$utc"
       --json "$out/packs/$pack.ci.json" --out "$out/packs/$pack.report.html")
  set +e
  "${cmd[@]}" 2> "$out/packs/$pack.stderr.txt"
  code=$?
  set -e
  engine="$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['engine_version'])" "$out/packs/$pack.ci.json")"
  hashes="$(python3 -c "
import json,sys
r=json.load(open(sys.argv[1]))
print('<br>'.join(m['label']+': '+m['scenario_sha256'] for m in r['watch']['members'])+'<br>engine scenario hashes: '+(', '.join(r['scenario_hashes']) or 'none stated'))" "$out/packs/$pack.ci.json")"
  seeds=""
  for f in scenarios/packs/$pack/*.toml; do
    [ "$(basename "$f")" = pack.toml ] && continue
    sd="$(grep -m1 '^seed = ' "$f" | cut -d' ' -f3 || true)"
    seeds="$seeds$(basename "$f"): ${sd:-none (a deterministic kind)}<br>"
  done
  echo "| \`$pack\` | $code | $engine | $hashes | ${seeds:-none} | \`${cmd[*]}\` |" >> "$log"
done
rm -rf "$work"
echo "wrote $out"
