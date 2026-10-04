#!/usr/bin/env python3
"""Compare every engine result the site publishes with the Studio's own recording of the same
scenario (what the Studio computes when the link is followed): numeric leaf by leaf.
Run: /opt/homebrew/bin/python3 compare_results.py [--out reports/site-vs-studio-results.md]"""
import glob, json, os, sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "explorations", "2026-09-27-combo")
STUDIO = os.path.normpath(os.path.join(SITE, json.load(open(os.path.join(SITE, "src/data/sources.json")))["studio"]))
site = {}
for p in glob.glob(os.path.join(SITE, "assets", "*", "*", "result.json")):
    site[os.path.basename(os.path.dirname(p))] = (json.load(open(p)), os.path.relpath(p, SITE))
b = os.path.join(SITE, "assets", "missions-runs", "results.json")
if os.path.exists(b):
    for n, r in json.load(open(b)).items():
        site.setdefault(n, (r, "assets/missions-runs/results.json"))


def studio(name):
    for d in ("", "celeste/"):
        p = os.path.join(STUDIO, d, "recorded", name + ".json")
        if os.path.exists(p):
            r = json.load(open(p))
            return (json.loads(r["json"]) if isinstance(r["json"], str) else r["json"]), r.get("source", "browser build")
    return None, None


def walk(p, a, b, out):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            walk(f"{p}.{k}", a.get(k), b.get(k), out)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b)):
            walk(f"{p}[{i}]", x, y, out)
    elif a != b:
        num = all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in (a, b))
        den = max(abs(a), abs(b)) if num else 0
        out.append((p, a, b, (abs(a - b) / den if den else 0.0) if num else None))


rows = []
for name in sorted(site):
    doc, where = site[name]
    sdoc, how = studio(name)
    if sdoc is None:
        rows.append((name, where, "NOT IN THE STUDIO", "", "")); continue
    out = []; walk("", doc, sdoc, out)
    skip = [o for o in out if o[0].split(".")[-1] in ("scenario_hash", "generated_utc", "engine_version", "git_commit", "build", "command")]
    out = [o for o in out if o not in skip]
    nums = [o for o in out if o[3] is not None]
    other = [o for o in out if o[3] is None]
    worst = max(nums, key=lambda o: o[3]) if nums else None
    verdict = "identical" if not out else ("equal to 1e-9" if not other and worst[3] <= 1e-9 else ("equal to 1e-6" if not other and worst[3] <= 1e-6 else "DIFFERS"))
    rows.append((name, where, verdict, f"{len(nums)} numbers, {len(other)} other" if out else "", f"{worst[0]}: site {worst[1]!r} vs Studio {worst[2]!r} (relative {worst[3]:.2e})" if worst else (f"{other[0][0]}: {str(other[0][1])[:40]!r} vs {str(other[0][2])[:40]!r}" if other else "")))
md = "# The site's published engine results against the Studio's recording of the same scenario\n\n| Scenario | Site file | Verdict | Differing leaves | Worst |\n|---|---|---|---|---|\n"
for r in rows:
    md += "| " + " | ".join(r) + " |\n"
out_p = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(ROOT, "scratch/assembly/reports/site-vs-studio-results.md")
open(out_p, "w").write(md)
for r in rows:
    print(f"{r[0]:46s} {r[2]:16s} {r[3]:24s} {r[4][:120]}")
print(f"{len(rows)} published results; {sum(1 for r in rows if r[2] == 'DIFFERS')} differ from the Studio's")

# ---- the headline figures on the Missions engine-run cards (js/missions-runs.mjs: chips), against
#      the Studio's recording: each figure must be a number of that result at the printed precision.
import re
s = open(os.path.join(SITE, "js", "missions-runs.mjs"), encoding="utf-8").read()
RUNS = json.loads(s[s.index("export default ") + len("export default "):].rstrip().rstrip(";"))
SCALES = [1, 100, 1e-3, 1e3, 1 / 60, 1 / 3600, 1 / 86400, 1e9, 1e6, 1e-6, 1e-9, 1e2 / 1, 1e-2]


def leaves(v, out):
    if isinstance(v, bool): return
    if isinstance(v, (int, float)): out.append(v)
    elif isinstance(v, list):
        for x in v: leaves(x, out)
        out.append(len(v))
    elif isinstance(v, dict):
        for x in v.values(): leaves(x, out)


print("\nMissions engine-run cards: headline figures against the Studio's recording")
md2 = "\n## Missions engine-run cards: headline figures against the Studio's recording\n\n| Card | Figures on the card | Found in the Studio's result | Not found |\n|---|---|---|---|\n"
bad = 0
for name, d in RUNS["runs"].items():
    chips = d.get("chips") if isinstance(d, dict) else None
    if not chips: continue
    sdoc, how = studio(name)
    if sdoc is None:
        print(f"  {name}: NOT IN THE STUDIO"); bad += 1; continue
    L = []; leaves(sdoc, L)
    found, missing = [], []
    for val, label in chips:
        m = re.search(r"-?\d[\d,]*(?:\.\d+)?", str(val))
        if not m: continue
        t = m.group(0).replace(",", ""); dec = len(t.split(".")[1]) if "." in t else 0; x = float(t)
        ok = any(round(v * sc, dec) == x for v in L for sc in SCALES)
        (found if ok else missing).append(f"{val} {label}")
    bad += bool(missing)
    print(f"  {name:46s} {len(found)}/{len(found) + len(missing)}" + (f"   NOT FOUND: {missing}" if missing else ""))
    md2 += f"| {name} | {len(found) + len(missing)} | {len(found)} | {'; '.join(missing)} |\n"
open(out_p, "a").write(md2)
print(f"{bad} card(s) show a figure the Studio's result does not contain at that precision")
