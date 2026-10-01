#!/usr/bin/env bash
# Recreate the local 0.30 workspace in a fresh machine or container, with the same paths the
# handover, GOAL.md and the validation plan use (~/Code/...).
#
# usage (from a checkout of kshana-pro on branch handover/open-core-0.30):
#   bash handover/open-core-0.30/restore.sh            # workspace, open repo, validation branch
#   WITH_DATA=1 bash handover/open-core-0.30/restore.sh  # also download the oracle datasets (~1 GB)
#   WITH_TOOLS=1 ...                                    # also run kshana-oracles/setup.sh (tools)
#
# Needs: git, rsync, gh (authenticated with read access to ashfordeOU/kshana-pro) for WITH_DATA.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CODE="${CODE:-$HOME/Code}"
mkdir -p "$CODE"

echo "== 1. Open repository: $CODE/kshana"
if [ ! -d "$CODE/kshana/.git" ]; then
  git clone https://github.com/ashfordeOU/kshana.git "$CODE/kshana"
fi
git -C "$CODE/kshana" fetch -q origin

echo "== 2. Validation branch from the bundle (unpushed on purpose: the open repo is public)"
B="$HERE/bundles/feat-validation-uplift-7737bedc.bundle"
git -C "$CODE/kshana" bundle verify "$B" >/dev/null
git -C "$CODE/kshana" fetch -q "$B" feat/validation-uplift:feat/validation-uplift 2>/dev/null \
  || echo "   (branch feat/validation-uplift already present; left as it is)"
git -C "$CODE/kshana" log --oneline -1 feat/validation-uplift
if [ ! -d "$CODE/kshana-wt-validation" ]; then
  git -C "$CODE/kshana" worktree add -q "$CODE/kshana-wt-validation" feat/validation-uplift
fi

echo "== 3. Workspace: $CODE/kshana-site-next"
mkdir -p "$CODE/kshana-site-next"
rsync -a "$HERE/site-next/" "$CODE/kshana-site-next/"
# The original file name was HANDOVER.md; it is renamed here only because the Pro repo ignores it.
[ -f "$CODE/kshana-site-next/HANDOVER.md" ] || cp "$CODE/kshana-site-next/SITE-HANDOVER.md" "$CODE/kshana-site-next/HANDOVER.md"
mkdir -p "$CODE/kshana-site-next/audit/bundles" "$CODE/kshana-site-next/scratch/validation"
cp "$B" "$CODE/kshana-site-next/audit/bundles/"

echo "== 4. Oracle toolchain scaffolding: $CODE/kshana-oracles"
mkdir -p "$CODE/kshana-oracles"
rsync -a "$HERE/oracles/" "$CODE/kshana-oracles/"

if [ "${WITH_DATA:-0}" = 1 ]; then
  echo "== 5. Oracle datasets (release asset workspace-open-core-0.30 on ashfordeOU/kshana-pro)"
  T="$(mktemp -d)"
  gh release download workspace-open-core-0.30 --repo ashfordeOU/kshana-pro --dir "$T" \
    --pattern 'kshana-oracles-data.tar.gz*'
  (cd "$T" && h="$(cut -d' ' -f1 kshana-oracles-data.tar.gz.sha256)" \
    && if command -v sha256sum >/dev/null; then echo "$h  kshana-oracles-data.tar.gz" | sha256sum -c -; \
       else echo "$h  kshana-oracles-data.tar.gz" | shasum -a 256 -c -; fi)
  tar -xzf "$T/kshana-oracles-data.tar.gz" -C "$CODE/kshana-oracles"
  rm -rf "$T"
fi

if [ "${WITH_TOOLS:-0}" = 1 ]; then
  echo "== 6. Oracle tools (Python venv, Orekit, RTKLIB, NaveGo, xval builds): setup.sh"
  (cd "$CODE/kshana-oracles" && bash setup.sh)
fi

cat <<EOF

Restored. Next:
  1. Read handover/open-core-0.30/README.md (cloud notes), then
     docs/plans/2026-10-01-open-core-0.30-handover.md in kshana-pro.
  2. The release steps (receipt gate, pre-push contract, push to main, tag) run on the
     founder's Mac only. See README.md, "What a cloud session must not do".
EOF
