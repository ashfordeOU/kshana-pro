#!/usr/bin/env python3
"""Derive act-runnable copies of the release-side workflows from the checkout's own files.

Only the steps that cannot run off GitHub are removed or replaced, each named below;
every other step is the workflow's own text. Re-run on every checkout (rerun.sh does),
so a change to a real workflow is always what gets exercised.

usage: derive.py <checkout> <outdir>
"""
import sys, pathlib, copy, yaml

src = pathlib.Path(sys.argv[1]) / ".github" / "workflows"
out = pathlib.Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
notes = []


def load(name):
    d = yaml.safe_load((src / name).read_text())
    if True in d:  # YAML 1.1 reads the bare key `on` as a boolean
        d["on"] = d.pop(True)
    return d


def dump(d, name, why):
    (out / name).write_text("# DERIVED by scratch/actlocal/derive.py; do not edit\n" + yaml.safe_dump(d, sort_keys=False, width=1000))
    notes.append(f"{name}: {why}")


def drop_steps(job, pred):
    before = len(job["steps"])
    job["steps"] = [s for s in job["steps"] if not pred(s)]
    return before - len(job["steps"])


def local_checkout(job):
    """act copies the local tree only when a checkout's `ref` is absent or literally equal
    to github.ref; an expression such as `${{ inputs.tag || github.ref }}` never is, so act
    clones from GitHub, where the tag does not exist yet. Drop `ref` (the local tree IS the
    release commit); every other checkout input (fetch-depth) stays."""
    for s in job["steps"]:
        if str(s.get("uses", "")).startswith("actions/checkout@") and isinstance(s.get("with"), dict):
            s["with"].pop("ref", None)
            if not s["with"]:
                s.pop("with")


def uses(prefix):
    return lambda s: str(s.get("uses", "")).startswith(prefix)


PUSH_TAG = {"push": {"tags": ["v*"]}}

# pages.yml build job: the checkout names refs/tags/<tag>, which act would clone from
# GitHub (the tag does not exist before the release); drop `ref` so act copies the local
# release commit, which is what the tag will point at. Deploy job and upload-pages-artifact
# removed (GitHub Pages artefact store + github-pages environment).
d = load("pages.yml")
b = d["jobs"]["build"]
for s in b["steps"]:
    if str(s.get("uses", "")).startswith("actions/checkout@"):
        s.pop("with", None)
# upload-pages-artifact -> a plain run artefact of the same `site` tree, kept by act's
# artifact server under scratch/actlocal/artifacts/, so the staged site can be served and
# link-checked locally (check_site.py).
up = [s for s in b["steps"] if str(s.get("uses", "")).startswith("actions/upload-pages-artifact@")]
assert len(up) == 1, "pages: upload-pages-artifact step not found"
art = [s for s in load("ci.yml")["jobs"]["coverage"]["steps"] if str(s.get("uses", "")).startswith("actions/upload-artifact@")][0]["uses"]
b["steps"][b["steps"].index(up[0])] = {"name": "(local) keep the staged site as a run artefact", "uses": art,
                                       "with": {"name": "pages-site", "path": "site", "if-no-files-found": "error"}}
d["jobs"] = {"build": b}
dump(d, "pages-build.yml", "checkout ref dropped (local tree = release commit); upload-pages-artifact -> upload-artifact of site/; deploy removed")

# release.yml release job: builds, SBOM, validation summary, notes, staging and SHA256SUMS.
# Removed: download of the macOS/Windows binaries (built by the `binaries` job on hosts
# act has not got; replaced by mkdir of the assets dir it created), attestation (run's
# OpenID Connect identity) and the GitHub Release upload.
d = load("release.yml")
r = copy.deepcopy(d["jobs"]["release"])
r.pop("needs", None)
local_checkout(r)
for i, s in enumerate(r["steps"]):
    if str(s.get("uses", "")).startswith("actions/download-artifact@"):
        r["steps"][i] = {"name": "(local) assets dir the binaries download would create", "run": "mkdir -p assets"}
n = drop_steps(r, uses("actions/attest-build-provenance@")) + drop_steps(r, uses("softprops/action-gh-release@"))
assert n == 2, "release: attest/gh-release steps not found"
d["jobs"] = {"release": r}
d["on"] = {**PUSH_TAG, "workflow_dispatch": d["on"]["workflow_dispatch"]}
dump(d, "release-build.yml", "needs dropped; binaries download -> mkdir assets; attest + gh-release removed")

# release.yml verify job, verbatim except the checkout `ref` (see local_checkout).
d = load("release.yml")
v = copy.deepcopy(d["jobs"]["verify"])
local_checkout(v)
d["jobs"] = {"verify": v}
d["on"] = {**PUSH_TAG, "workflow_dispatch": d["on"]["workflow_dispatch"]}
dump(d, "release-verify.yml", "verify job; checkout ref dropped (act would clone the not-yet-existing tag)")

# publish.yml npm-build: `needs: gates` dropped (gates reads the Actions API for a green
# verify of the tag-push run), attestation removed. Upload-artifact stays (act serves it).
d = load("publish.yml")
j = copy.deepcopy(d["jobs"]["npm-build"])
j.pop("needs", None)
n = drop_steps(j, uses("actions/attest-build-provenance@"))
assert n == 1
g = copy.deepcopy(d["jobs"]["gates"])
# gates: the verdict step reads the Actions API; everything else in it runs.
drop_steps(g, lambda s: "check-release-verdict" in str(s.get("run", "")))
d["jobs"] = {"gates-guards": g, "npm-build": j}
d["on"] = PUSH_TAG
dump(d, "publish-build.yml", "gates without the Actions-API verdict step; npm-build without needs/attest")

# wheels.yml: not derived. Its Linux legs and wheel-repro build through PyO3/maturin-action,
# which act cannot host (sibling-container /tmp mount); see manylinux.sh for the stand-in.

# jetbrains-plugin.yml build job (the publish job needs the Marketplace token).
d = load("jetbrains-plugin.yml")
d["jobs"] = {"build": d["jobs"]["build"]}
d["jobs"]["build"].pop("if", None)
d["on"] = PUSH_TAG
dump(d, "jetbrains-build.yml", "build job only; trigger push-tag instead of workflow_call")


# x86-64 byte pins: GitHub's ubuntu runners are x86-64, act here is arm64 unless told
# otherwise. This job is ci.yml's reproducibility-matrix setup (checkout, the pinned
# toolchain, rust-cache) with the test step widened to every suite that pins exact bytes
# or published figures: the reproducibility set, the leo-navmsg frame pins, the
# portable-maths unit pins, and every *_doc_sync / published-figure test. Run it with
# ACT_ARCH=linux/amd64 (Rosetta-emulated x86-64).
d = load("ci.yml")
x = copy.deepcopy(d["jobs"]["reproducibility-matrix"])
x.pop("strategy", None)
x["runs-on"] = "ubuntu-latest"
x["timeout-minutes"] = 240
tests = sorted(p.stem for p in (pathlib.Path(sys.argv[1]) / "tests").glob("*.rs")
               if p.stem.endswith("_doc_sync") or p.stem in (
                   "cross_platform_golden", "golden", "determinism", "sgp4_verification",
                   "leo_navmsg_reference", "published_figures_still_reproduce",
                   "workshop_preset_isolation", "registry_golden"))
x["steps"][-1] = {"name": "x86-64 byte and figure pins",
                  "run": "set -euo pipefail\nuname -m\ncargo test --no-fail-fast " + " ".join("--test " + s for s in tests)
                  + "\ncargo test --lib portable_math\n"}
d["jobs"] = {"xarch-pins": x}
dump(d, "xarch-pins.yml", "reproducibility-matrix setup; tests = %d suites + lib portable_math" % len(tests))

# mcp-publish.yml registry job's offline half: stamp + validate server.json. The image job
# is exercised natively with docker buildx (no push); login/publish need the run's OIDC.
print("\n".join(notes))
