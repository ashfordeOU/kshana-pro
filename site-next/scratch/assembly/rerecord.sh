#!/bin/zsh
# Rerun every site recorder on the final engine tree, then build, check and the feature register.
set -e
cd ~/Code/kshana-site-next/explorations/2026-09-27-combo
L=~/Code/kshana-site-next/scratch/assembly/logs; P=/opt/homebrew/bin/python3
export KSHANA_ENGINE_TREE=~/Code/kshana-wt-integration-bc CARGO_BUILD_JOBS=8
for t in record_runs gen_waterfall; do $P src/tools/$t.py > $L/rr-$t.log 2>&1 || { echo "$t FAILED"; tail -3 $L/rr-$t.log; exit 1; }; done
for n in $($P -c "import json;print(' '.join(json.load(open('src/data/runs/index.json'))['runs']))"); do d=assets/runs/$n; mkdir -p $d; cp src/data/runs/$n/scenario.toml $d/$n.toml.txt; cp src/data/runs/$n/result.json $d/result.json; cp src/data/runs/$n/report.html $d/report.html; done
for t in gen_capabilities gen_missions_runs record_cli_session gen_evidence_data gen_missions_page gen_publications gen_developers_data; do $P src/tools/$t.py > $L/rr-$t.log 2>&1 || { echo "$t FAILED"; tail -3 $L/rr-$t.log; exit 1; }; done
$P src/tools/gen_missions_page.py --check; node src/tools/gen_footprint.mjs --check
echo "recorders ok at $(git -C $KSHANA_ENGINE_TREE rev-parse --short HEAD)"
$P build.py > $L/rr-build.log 2>&1; grep "files to publish" $L/rr-build.log
$P check.py > $L/rr-check.log 2>&1 && tail -1 $L/rr-check.log || { tail -15 $L/rr-check.log; exit 1; }
