#!/usr/bin/env python3
"""Kshana Pro artefacts for the site: read the REAL outputs of Kshana Pro runs and write the one
data file the Editions and Home Pro sections draw (src/data/pro-power.json), plus the few Pro
files the site links (assets/pro/).

Nothing here is typed: every number is read from a file a `kshana-pro` command wrote.

Inputs (the showcase folders the Pro build and its verifiers wrote, override with KSHANA_PRO_SHOWCASE):
  design-optimiser/showcase/{lunar-global,lunar-south-polar}/design.result.json
  design-optimiser/showcase/open-engine/reproduction.csv   (front designs re-run in the FREE engine)
  uncertainty/showcase/clock-band/uq.result.json            (confidence band, probability of a limit)
  uncertainty/showcase/leo-sensitivity/uq.result.json       (Sobol' sensitivity ranking)
  mission-dossier/showcase/{dossier.json,dossier.pdf,site/dossier-page-0[12].png}
  campaign-watch/showcase/{watch.trend.json,runs/*.ci.json,SHOWCASE.md,history-verify.txt,public-text/*.log}
                                                            (+ the revisions' jammer power, from kshana-pro scenarios/pro-power/watch-demo)
  spectrum-coexistence/showcase/coexistence.result.json
  service-jobs/showcase/public-text/{drive.log,crash-restart.log}
The public wording the site quotes is docs/PRO.md of the open repository; its checkout is recorded
here (public_text) so build.py and features/gen_features.py can check every quote against it.

Usage: /opt/homebrew/bin/python3 src/tools/gen_pro_power.py
"""
import csv, json, os, re, shutil, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.abspath(os.path.join(HERE, "..", ".."))
SHOW = os.path.expanduser(os.environ.get("KSHANA_PRO_SHOWCASE", "~/Code/kshana-site-next/scratch/pro"))
# The open-repository checkout that carries the public Pro text (docs/PRO.md) until it is merged
# into the site's engine checkout (src/data/sources.json: repo). build.py looks in repo first.
PRO_TEXT = os.path.expanduser(os.environ.get("KSHANA_PRO_TEXT_REPO", "~/Code/kshana-wt-pro-public"))
OUT_JSON = os.path.join(SITE, "src", "data", "pro-power.json")
OUT_ASSETS = os.path.join(SITE, "assets", "pro")


def J(*p):
    return json.load(open(os.path.join(SHOW, *p), encoding="utf-8"))


def git(*a):
    return subprocess.run(["git", "-C", PRO_TEXT, *a], capture_output=True, text=True).stdout.strip()


def r(x, n=4):
    return None if x is None else round(float(x), n)


# ---------------------------------------------------------------- design optimiser
repro = list(csv.DictReader(open(os.path.join(SHOW, "design-optimiser/showcase/open-engine/reproduction.csv"))))
design = []
for folder, region in (("lunar-global", "Whole Moon"), ("lunar-south-polar", "South polar cap")):
    d = J("design-optimiser/showcase", folder, "design.result.json")
    assert d["kind"] == "pro-design-optimise" and d["exhaustive"], folder
    rep = {x["front_id"]: x for x in repro if x["study"] == folder}
    front = []
    for f in d["front"]:
        m = rep.get(f["id"])
        assert m and m["index_result_sha256"] == f["result_sha256"], f"{folder} {f['id']}: index and result disagree"
        front.append({"id": f["id"], "sat": f["objectives"]["satellites"], "av": f["objectives"]["availability_pct"],
                      "knee": f["knee"], "v": f["variables"], "sha": f["result_sha256"],
                      "free": m["match"] == "yes" and m["open_engine_result_sha256"] == f["result_sha256"]})
    ref = d["reference"]
    design.append({
        "study": folder, "region": region, "base_kind": d["base"]["kind"], "title": d["title"], "label": d["label"],
        "evaluated": [[e["objectives"]["satellites"], e["objectives"]["availability_pct"]] for e in d["evaluated"] if e["status"] == "ok"],
        "front": front, "front_points": d["front_summary"]["distinct_objective_points"],
        "knee": {"id": d["knee"]["front_id"], "sat": d["knee"]["objectives"]["satellites"], "av": d["knee"]["objectives"]["availability_pct"]},
        "reference": {"label": ref["label"], "sat": ref["objectives"]["satellites"], "av": ref["objectives"]["availability_pct"],
                      "statement": ref["statement"]},
        "runs": d["work"]["evaluations"], "rejected": d["work"]["infeasible"], "engine_runs": d["reproducibility"]["engine_runs"],
        "method": d["search"]["method_used"], "front_statement": d["front_statement"],
        "variables": [v["name"] for v in d["space"]["variables"]],
        "engine": d["engine_version"], "overlay": d["overlay_version"], "seed": d["seed"],
        "digest": d["reproducibility"]["run_digest"], "not_modelled": d["not_modelled"],
    })

# ---------------------------------------------------------------- uncertainty: band + probability
u = J("uncertainty/showcase/clock-band/uq.result.json")
assert u["kind"] == "pro-uncertainty"
b = u["bands"]["time_error"]
lim = u["outputs"]["final_error"]["limit"]
band = {
    "title": u["title"], "label": u["label"], "base_kind": u["base"]["kind"], "n": u["runs"]["ok"], "unit": b["unit"], "time_unit": b["time_unit"],
    "limit": b["limit"]["value"], "limit_statement": b["limit"]["statement"],
    "t": b["t"], "p05": [r(x, 3) for x in b["p05"]], "p50": [r(x, 3) for x in b["p50"]], "p95": [r(x, 3) for x in b["p95"]],
    "nominal": [r(x, 3) for x in b["nominal"]], "share": [r(x, 4) for x in b["share_meeting_limit"]],
    "final": {"p": lim["probability"], "lo": r(lim["probability_ci95_low"]), "hi": r(lim["probability_ci95_high"]),
              "statement": lim["statement"], "met": lim["n_met"], "of": lim["n_realisations"]},
    "input": {"name": u["inputs"][0]["name"], "dist": u["inputs"][0]["distribution"], "source": u["inputs"][0]["source"]},
    "engine": u["engine_version"], "overlay": u["overlay_version"], "seed": u["seed"], "method": u["sampling"]["method"],
    "digest": u["reproducibility"]["run_digest"],
}
s = J("uncertainty/showcase/leo-sensitivity/uq.result.json")
OUTNAME = {"fused_error": "Fused position error, with the low-Earth-orbit layer", "gnss_only_error": "Position error, GNSS alone",
           "peak_cn0": "Peak carrier-to-noise density of the pass"}
INNAME = {"od_sisre": "Low-Earth-orbit orbit and clock error", "eirp": "Low-Earth-orbit transmit power",
          "gps_sisre": "GPS signal-in-space range error", "galileo_sisre": "Galileo signal-in-space range error"}
sens = {"title": s["title"], "label": s["label"], "base_kind": s["base"]["kind"], "n": s["runs"]["ok"], "cost": s["sensitivity"]["cost"], "method": s["sensitivity"]["method"],
        "inputs": [{"key": i["name"], "name": INNAME[i["name"]], "dist": i["distribution"], "unit": i.get("unit"), "source": i["source"]} for i in s["inputs"]],
        "outputs": [], "engine": s["engine_version"], "overlay": s["overlay_version"], "seed": s["seed"], "digest": s["reproducibility"]["run_digest"]}
for o in s["output_order"]:
    so, oo = s["sensitivity"]["outputs"][o], s["outputs"][o]
    sens["outputs"].append({"key": o, "name": OUTNAME[o], "unit": oo["unit"], "mean": r(oo["mean"]), "p05": r(oo["p05"]), "p95": r(oo["p95"]),
                            "inputs": sorted([{"key": i["input"], "total": r(i["total"]), "lo": r(i["total_ci95_low"]), "hi": r(i["total_ci95_high"]),
                                               "first": r(i["first_order"]), "rank": i["rank"]} for i in so["inputs"]], key=lambda x: x["rank"]),
                            "reliable": so.get("reliable", True) if so.get("reason") is None else False})

# ---------------------------------------------------------------- mission dossier
dd = J("mission-dossier/showcase/dossier.json")
body = dd["body"]
pages = int(re.search(r"Pages:\s+(\d+)", subprocess.run(["pdfinfo", os.path.join(SHOW, "mission-dossier/showcase/dossier.pdf")], capture_output=True, text=True).stdout).group(1))
rows = []
for m in body["matrix"]:
    c = m["cells"][0] if m["cells"] else None
    v = c["values"][0] if c and c.get("values") else None
    rows.append({"id": m["requirement"], "text": m["text"], "method": m["method"], "criterion": m["criterion"], "verdict": m["verdict"],
                 "run": c["run"] if c else None, "basis": c["basis"] if c else None,
                 "value": v["value"] if v else None, "unit": v["unit"] if v else None, "detail": v["detail"] if v else None,
                 "open": [o["category"] for o in body["open_items"] if o["requirement"] == m["requirement"]]})
runs = [{"id": sh["id"], "kind": sh["kind"], "title": sh["title"]} for sh in body["sheets"]]
dossier = {"title": body["identification"]["title"], "reference": body["identification"]["reference"], "banner": body["banner"],
           "summary": body["summary"], "matrix": rows, "runs": runs, "trade": {"title": body["trade"]["title"], "winner": body["trade"]["winner"]},
           "pages": pages, "sha": dd["dossier_sha256"], "issued": dd["issued_utc"], "engine": body["identification"]["engine_versions"],
           "overlay": body["identification"]["overlay_version"], "build": body["identification"]["build_profile"],
           "limitations": body["limitations"]}

# ---------------------------------------------------------------- campaign watch
# The six-revision demonstration (campaign-watch/showcase: runs/, watch.trend.json, SHOWCASE.md,
# history-verify.txt) and the bundled campaign pack run twice (showcase/public-text/*.log).
CW = "campaign-watch/showcase"
tr = J(CW, "watch.trend.json")
exits = {}
for ln in open(os.path.join(SHOW, CW, "SHOWCASE.md"), encoding="utf-8"):
    m = re.match(r"\|\s*(\d+)\s*\|\s*`([\w-]+)`\s*\|(?:[^|]*\|){4}\s*(\d+)\s*\|", ln)
    if m:
        exits[int(m.group(1))] = (m.group(2), int(m.group(3)))
assert sorted(exits) == list(range(tr["records"])), "SHOWCASE.md and the trend disagree on the runs"
hv = open(os.path.join(SHOW, CW, "history-verify.txt"), encoding="utf-8").read()
mh = re.search(r"(\d+) record\(s\), chain intact, last hash ([0-9a-f]{64})", hv)
assert mh and mh.group(2) == tr["last_hash"], "history check and trend disagree"


def _jam(rev):
    """The jamming-phase jammer power of a demonstration revision (the one power_dbw the revisions change)."""
    t = open(os.path.expanduser(f"~/Code/kshana-pro/scenarios/pro-power/watch-demo/revisions/{rev}.toml"), encoding="utf-8").read()
    return float(re.findall(r"^power_dbw = (-?[\d.]+)", t, re.M)[0])


runs_cw = []
for r_ in tr["runs"]:
    name, rc = exits[r_["index"]]
    ci = J(CW, "runs", f"{name}.ci.json")
    assert ci["watch"]["passed"] == (r_["verdict"] == "pass") and ci["label"].startswith("MODELLED")
    runs_cw.append({"name": name, "verdict": r_["verdict"], "exit": rc, "failures": r_["failures"], "utc": r_["time_utc"],
                    "jammer_dbw": _jam(name)})
figs_cw = [{"id": f["id"], "unit": f["unit"], "evidence": f["evidence"], "limits": f["limits"],
            "status": [p["status"] for p in f["points"]], "failed": [p["failed"] for p in f["points"]],
            "requirement": [p["requirement"] for p in f["points"]], "value": [p["value"] for p in f["points"]]} for f in tr["figures"]]
PT = os.path.join(SHOW, CW, "public-text")
lg = {n: open(os.path.join(PT, n), encoding="utf-8").read() for n in ("ci1.log", "ci2.log", "ci-diff.log", "ci-hv.log")}
mw = [re.search(r"\[ci\] (PASS|FAIL) \((\d+) watch\(es\)\)", lg[n]) for n in ("ci1.log", "ci2.log")]
md = re.search(r"(\d+) figure\(s\) moved, (\d+) unchanged", lg["ci-diff.log"])
mx = re.search(r"exact: (\d+) result\(s\) changed", lg["ci2.log"])
mhv = re.search(r"(\d+) record\(s\), chain intact", lg["ci-hv.log"])
ov = re.search(r"kshana-pro ([\d.]+) \(overlay\)", open(os.path.join(SHOW, CW, "SHOWCASE.md"), encoding="utf-8").read()).group(1)
watch = {"title": tr["title"], "label": tr["label"], "engine": ci["engine_version"], "overlay": ov, "runs": runs_cw, "figures": figs_cw,
         "history": {"records": int(mh.group(1)), "intact": True, "last": mh.group(2)},
         "pack": {"verdicts": [m.group(1) for m in mw], "watches": int(mw[0].group(2)), "changed": int(mx.group(1)),
                  "moved": int(md.group(1)), "figures": int(md.group(1)) + int(md.group(2)), "records": int(mhv.group(1))},
         "base_scenario": "campaign-jam-spoof-holdover-integrity"}

# ---------------------------------------------------------------- spectrum coexistence
co = J("spectrum-coexistence/showcase/coexistence.result.json")
assert co["kind"] == "pro-spectrum-coexistence" and co["label"].startswith("MODELLED")
VICTIM = {"gps-l1ca": "GPS L1 C/A", "galileo-e1": "Galileo E1", "gps-l5": "GPS L5", "galileo-e5a": "Galileo E5a",
          "galileo-e5b": "Galileo E5b", "galileo-e5-altboc": "Galileo E5 AltBOC"}
plans = []
for p in co["plans"]:
    cz = {c["gnss"]: c for c in p["caused"]}
    plans.append({"label": p["label"], "sats": p["constellation"]["total_satellites"], "worst": p["worst_loss_caused_db"],
                  "verdict": p["verdict_caused"]["status"], "verdict_received": p["verdict_received"]["status"],
                  "front": p["label"] in co["pareto"]["front"],
                  "caused": [{"victim": v, "overlap": cz[v]["spectral_overlap"], "loss": cz[v]["worst_loss_db"], "status": cz[v]["verdict"]["status"],
                              "in_view": (cz[v]["worst_cell"] or {}).get("satellites_in_view"), "margin": cz[v]["verdict"]["margin_db"]} for v in co["victims"]],
                  "received_max": max((x["worst_loss_db"] for x in p["received"] if x["spectral_overlap"]), default=None)})
mp = next(p for p in co["plans"] if p["label"] == co["summary"]["map_plan"])
mvals = [x for row in mp["map_loss_db"] for x in row if x is not None]
coexist = {"title": co["title"], "label": co["label"], "bound": co["bound"], "limit": co["limit"]["max_degradation_db"],
           "limit_source": co["limit"]["limit_source"], "victims": [{"id": v, "name": VICTIM[v]} for v in co["victims"]],
           "plans": plans, "summary": co["summary"], "runs": len(co["members"]), "member_kinds": sorted({m["kind"] for m in co["members"]}),
           "front": co["pareto"]["front"], "scenario_hash": co["scenario_hash"], "engine": co["engine_version"], "overlay": co["overlay_version"],
           "map": {"plan": mp["label"], "lat": co["grid"]["lat_deg"], "lon": co["grid"]["lon_deg"], "step": co["grid"]["grid_step_deg"],
                   "max": max(mvals), "cells": [[None if x is None else round(x / max(mvals), 3) for x in row] for row in mp["map_loss_db"]]}}

# ---------------------------------------------------------------- on-premises job service
# The public-text drive of the service (showcase/public-text: drive.log, crash-restart.log,
# openapi.json), the run docs/PRO.md's worked example describes.
SJ = os.path.join(SHOW, "service-jobs/showcase/public-text")
dl = open(os.path.join(SJ, "drive.log"), encoding="utf-8").read()
cl = open(os.path.join(SJ, "crash-restart.log"), encoding="utf-8").read()


def _j(pat, txt=dl):
    m = re.search(pat, txt, re.S | re.M)
    assert m, pat
    return m


sub = _j(r"^submit (\d+) (\{.*?\})$")
stt = _j(r"^status (\d+) (\{.*?\})$")
res = _j(r"^resubmit same request (\d+) (\{.*?\})$")
lv = _j(r"^ledger/verify (\d+) (\{.*?\n\})")
oa = _j(r"^openapi (\d+) ([\d.]+) paths (\d+)$")
bd = _j(r"^board (\d+) (\d+) bytes; external refs: \[(.*?)\]$")
rst = _j(r"^after restart status (\d+) (\w+) attempts (\d+)$")
nl1 = _j(r"^no licence, pro-twin job (\d+) (\{.*?\})$")
nl2 = _j(r"^no licence, GET /jobs (\d+) (\{.*?\})$")
nl3 = _j(r"^no licence, open run job (\d+) (\{.*?\})$")
kb = _j(r"^before kill counts (\{.*?\})$", cl)
ka = _j(r"^after restart counts (\{.*?\})$", cl)
kl = _j(r"^ledger verify (\{.*?\})$", cl)
kj = re.findall(r"^job ([0-9a-f]{12}) (\S+) (\w+) attempts (\d+)$", cl, re.M)
lit = lambda s: json.loads(s.replace("'", '"').replace("True", "true").replace("False", "false"))
S1, T1, R1, L1 = json.loads(sub.group(2)), json.loads(stt.group(2)), json.loads(res.group(2)), json.loads(lv.group(2))
jobs = {"steps": [
    {"req": "POST /jobs", "code": int(sub.group(1)), "state": S1["state"], "created": S1["created"], "job": S1["job_id"]},
    {"req": "GET /jobs/{id}", "code": int(stt.group(1)), "state": T1["state"], "attempts": T1["attempts"], "kind": T1["kind"]},
    {"req": "POST /jobs (same request)", "code": int(res.group(1)), "state": R1["state"], "created": R1["created"], "same": R1["job_id"] == S1["job_id"]},
    {"req": "GET /ledger/verify", "code": int(lv.group(1)), "verified": L1["verified"], "intact": L1["chain_intact"], "records": L1["records"],
     "delivered": L1["delivered_jobs"], "mismatches": len(L1["store_mismatches"])},
    {"req": "GET /openapi.json", "code": int(oa.group(1)), "openapi": oa.group(2), "paths": int(oa.group(3))},
    {"req": "GET /", "code": int(bd.group(1)), "external": [x for x in bd.group(3).split(",") if x.strip()]},
    {"req": "restart, GET /jobs/{id}", "code": int(rst.group(1)), "state": rst.group(2), "attempts": int(rst.group(3))},
    {"req": "kill -9 while running, restart", "before": lit(kb.group(1)), "after": lit(ka.group(1)),
     "jobs": [{"id": a, "kind": b, "state": c, "attempts": int(d)} for a, b, c, d in kj], "ledger": lit(kl.group(1))},
    {"req": "no licence: POST /jobs (Pro scenario)", "code": int(nl1.group(1)), "error": json.loads(nl1.group(2))["error"],
     "queued_after": json.loads(nl2.group(2))["total"]},
    {"req": "no licence: POST /jobs (open scenario)", "code": int(nl3.group(1)), "state": json.loads(nl3.group(2))["state"]},
]}

# ---------------------------------------------------------------- files the site links
os.makedirs(OUT_ASSETS, exist_ok=True)
shutil.copyfile(os.path.join(SHOW, "mission-dossier/showcase/dossier.pdf"), os.path.join(OUT_ASSETS, "mission-dossier.pdf"))
for n in ("01", "02"):
    subprocess.run(["cwebp", "-quiet", "-q", "82", "-resize", "600", "0", os.path.join(SHOW, f"mission-dossier/showcase/site/dossier-page-{n}.png"),
                    "-o", os.path.join(OUT_ASSETS, f"mission-dossier-page-{n}.webp")], check=True)

data = {
    "_comment": "Generated by src/tools/gen_pro_power.py from real Kshana Pro outputs; do not edit by hand.",
    "public_text": {"checkout": PRO_TEXT.replace(os.path.expanduser("~"), "~"), "file": "docs/PRO.md",
                    "branch": git("rev-parse", "--abbrev-ref", "HEAD"), "commit": git("rev-parse", "HEAD")},
    "design": design, "band": band, "sensitivity": sens, "dossier": dossier, "watch": watch, "coexist": coexist, "jobs": jobs,
    "files": {"dossier_pdf": "assets/pro/mission-dossier.pdf", "dossier_pages": ["assets/pro/mission-dossier-page-01.webp", "assets/pro/mission-dossier-page-02.webp"]},
}
json.dump(data, open(OUT_JSON, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
print(f"wrote {OUT_JSON} ({os.path.getsize(OUT_JSON):,} bytes) and {len(os.listdir(OUT_ASSETS))} files in assets/pro/")
for d in design:
    print(f"  design {d['study']}: {d['runs']} runs, front {len(d['front'])} at {d['front_points']} points, knee {d['knee']}, free-engine matches {sum(f['free'] for f in d['front'])}/{len(d['front'])}")
print(f"  band: {band['n']} runs, P(|final| <= {band['limit']} {band['unit']}) = {band['final']['p']} [{band['final']['lo']}, {band['final']['hi']}]")
print(f"  sensitivity: {sens['n']} runs; top driver of each output: " + ", ".join(f"{o['key']}={o['inputs'][0]['key']} {o['inputs'][0]['total']}" for o in sens["outputs"]))
print(f"  watch: {len(runs_cw)} revisions, verdicts {[r_['verdict'] for r_ in runs_cw]}, exits {[r_['exit'] for r_ in runs_cw]}, history {watch['history']['records']} intact; pack {watch['pack']}")
print(f"  coexist: {coexist['summary']['plans_compatible']}/{coexist['summary']['plans']} compatible, {coexist['runs']} runs, worst {[p['worst'] for p in plans]}, front {coexist['front']}")
print(f"  jobs: {[(s['req'], s.get('code')) for s in jobs['steps']]}")
print(f"  dossier: {dossier['summary']['requirements']} requirements, {dossier['summary']['met']} met / {dossier['summary']['not_met']} not met / {dossier['summary']['not_verified']} not verified, {dossier['summary']['open_items']} open items, {pages} pages")
