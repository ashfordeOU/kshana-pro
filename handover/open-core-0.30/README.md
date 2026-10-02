# Open core 0.30: cloud workspace

This branch (`handover/open-core-0.30` in the private `ashfordeOU/kshana-pro`) carries everything a
cloud session needs to work on kshana 0.30 without the founder's Mac. It was packed on 2026-10-01.

Start with `docs/plans/2026-10-01-open-core-0.30-handover.md` (on this branch and on `main`). Its
paths (`~/Code/...`) are reproduced by `restore.sh`.

## 1. Restore the workspace (once per container)

```
git clone -b handover/open-core-0.30 https://github.com/ashfordeOU/kshana-pro.git ~/Code/kshana-pro
WITH_DATA=1 bash ~/Code/kshana-pro/handover/open-core-0.30/restore.sh
```

| Restored path | From | Contents |
|---|---|---|
| `~/Code/kshana` | public GitHub clone | the open repository, `main` = v0.29.3 or later |
| `~/Code/kshana` branch `feat/validation-uplift`, worktree `~/Code/kshana-wt-validation` | `bundles/feat-validation-uplift-7737bedc.bundle` | the 0.30 validation work: 42 commits on top of `38feb769`, 93 VALIDATED (unpushed on purpose; the open repo is public and keeps only `main`) |
| `~/Code/kshana-site-next/GOAL.md`, `HANDOVER.md` | `site-next/` | the live tracker and the site and Studio handover |
| `~/Code/kshana-site-next/research/` | `site-next/research/` | `VALIDATION-0.30-PLAN.md`, `VALIDATION-UPLIFT.md`, the 55 promotion records and pre-registrations, the Studio UX baseline |
| `~/Code/kshana-site-next/explorations/2026-10-01-studio-rebuild` | `site-next/explorations/` | the Studio source (Simple and Advanced views) |
| `~/Code/kshana-site-next/explorations/2026-09-27-combo` | `site-next/explorations/` | the kshana.dev site source (`build.py`, `check.py`) |
| `~/Code/kshana-site-next/features/` | `site-next/features/` | `check_nav.mjs`, `check_features.py`, the feature register, `serve.py` |
| `~/Code/kshana-site-next/scratch/actlocal/` | `site-next/scratch/actlocal/` | the local-runner scripts (they need `act` and Docker; the macOS legs need a Mac) |
| `~/Code/kshana-oracles/` | `oracles/` plus the release asset | `README.md`, `env.sh`, `setup.sh`, `sources.tsv`, the requirement locks, `data/MANIFEST.md`; with `WITH_DATA=1` the datasets (`data/`, `kernels/`, `gg_torque/`) from the release `workspace-open-core-0.30` |

The oracle *tools* (Python venv, Orekit, RTKLIB, NaveGo under Octave, the xval builds) are not
packed. `WITH_TOOLS=1` runs `setup.sh`, which rebuilds them from their upstream sources. Read
`~/Code/kshana-oracles/README.md` first: some need system packages (a JDK, Octave, a C compiler).
GMAT, GNSS-SDR and NeQuick G were never installed anywhere (founder decisions 5 and 6).

Not packed, because each is regenerated or not needed: `node_modules` (run `npm ci` where a
`package.json` exists); the local runners' artefacts and logs; the promo video sources (the video
only needs new counts at the very end, and that step is done on the Mac).

## 2. What a cloud session can do

Every step of the handover's section 5 up to and including step 6. That means:
- merge `feat/validation-uplift` onto `main`;
- apply the founder's decisions;
- the row work, with an adversarial verifier per batch;
- the Studio fixes and housekeeping;
- the version bump and the CHANGELOG;
- `cargo test`, the generators' `--check` runs, and the site's `check.py`, `check.py --nav` and `check_features`.

## 3. What a cloud session must not do

- **Never push to `ashfordeOU/kshana`** (no branch, no tag). The founder keeps only `main` there,
  and `main` is pushed only after the receipt gate and the local runners pass. Those run on the
  founder's Mac, where the untracked pre-push contract `.ci-native` lives.
- No `pages.yml` dispatch, no release, no registry action.

## 4. How a cloud session hands its work back

Commit on a branch in `~/Code/kshana`, for example `feat/release-0.30`, cut from `main`. Then pack
it and push the bundle to this branch of the Pro repo:

```
cd ~/Code/kshana
git bundle create ~/Code/kshana-pro/handover/open-core-0.30/bundles/release-0.30-<shortsha>.bundle \
  origin/main..feat/release-0.30 feat/release-0.30
cd ~/Code/kshana-pro
git add handover/open-core-0.30/bundles
git commit -m "0.30 work: <what>"
git push origin handover/open-core-0.30
```

Copy back any workspace files you changed, such as GOAL.md, the records and the Studio or site
sources, under `handover/open-core-0.30/site-next/`. Log what you did in GOAL.md, with times and
the founder's words for every decision.

The Mac session then:
1. fetches the bundle;
2. runs the receipt gate and the local runners;
3. pushes `main` and the tag;
4. deploys and runs the live check (handover steps 7 and 8).

## 5. Rules (from the founder; all in force)

Section 4 of the handover lists all of them. The ones a fresh cloud session most often breaks:
- Commits use `ashfordeOU <236818772+ashfordeOU@users.noreply.github.com>`.
- **NO `Co-Authored-By`, "Generated with" or session trailer lines** in any kshana or kshana-pro commit.
- No AI or vendor names in repository content.
- Never `--no-verify`.
- Never `git stash`, `reset`, `clean` or `checkout -- <file>` in fan-out work.
- Every abbreviation spelled out at first use.
- No prices.
- A changed published number is a revision with a CHANGELOG line.
- Never loosen a tolerance after seeing a result.
- Never change a quality gate to pass.
- Never handle the founder's credentials.
