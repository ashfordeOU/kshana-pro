#!/usr/bin/env python3
"""The site's published research (src/data/publications.json), read from arXiv and the engine checkout.

For every paper in PAPERS it fetches the exact title, authors, first-submission date, latest
version, primary category and abstract from the arXiv API (export.arxiv.org/api/query), and checks
the Kshana capability that reproduces it against the engine checkout (src/data/sources.json: repo):
the scenario file and its `kind`, or the example program and its run command. A paper the API does
not return (not live: queued, on hold, withdrawn) stops the script: only live papers are listed.

What is typed here, and only here: which papers are the engine's (their arXiv ids), the one-sentence
plain summary of each (a paraphrase of its abstract, no new claims), and which capability reproduces
it. Everything else is read.

Each paper also carries a FIGURE read from a real engine run: the scenario the card names is run
with the checkout's engine binary (target/release/kshana), and a study example with its release
build (target/release/examples/<name>; built with CARGO_BUILD_JOBS=6 if missing). Runs happen in
.build/pub-runs/ (the engine writes next to its inputs); only the plotted numbers are stored, with
the engine version, commit, seed and the verification-matrix label of the capability.

Usage: python3 src/tools/gen_publications.py            (writes src/data/publications.json)
       python3 src/tools/gen_publications.py --offline  (keeps the arXiv fields, reruns the engine)
"""
import json, os, re, shutil, subprocess, sys, time, urllib.request
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _sources import engine_tree  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "data", "publications.json")
API = "https://export.arxiv.org/api/query?id_list={ids}&max_results={n}"
NS = {"a": "http://www.w3.org/2005/Atom", "x": "http://arxiv.org/schemas/atom"}

# id -> (plain summary, reproduce). reproduce: {"scenario": <file stem>} for a bundled scenario the
# engine runs (and the Studio opens), or {"example": <examples/*.rs stem>, "args": <its arguments>}.
PAPERS = {
    "2607.06212": ('Planned orbits bunch overhead for a lunar south-pole user; a few ranging beacons on nearby high ground fix the geometry more cheaply than more satellites.',
                   {"scenario": "lunar-beacon"}),
    "2607.02566": ('Lunar-network ranging fixes only relative geometry; an Earth-baseline very-long-baseline interferometry delay ties a surface station to the Earth frame, even with few satellites.',
                   {"scenario": "lunar-joint-od-clock"}),
    "2607.05415": ('Single-number resilience ratings for positioning, navigation and timing are stable only where one design dominates; weighting, threat and declared techniques can flip them.',
                   {"example": "resilience_report", "args": "paper-artifacts/resilience-study.json"}),
    "2606.24210": ('A recorded spoof shows a timing receiver serving time far more wrong than it reports; no unconditional bound exists, so the paper bounds the undetected error conditionally.',
                   {"example": "tpl_jammertest", "args": ""}),
    "2606.22054": ('How much an interference detector loses under shifted conditions can be predicted from its own in-distribution scores; synthetic results, checked on open field recordings.',
                   {"example": "optimism_study", "args": "paper-artifacts/optimism-study.json"}),
}
STOP = {"a", "an", "the", "how", "is", "of", "and", "on", "in"}


# id -> research area (icon and accent colour on the card), the verification-matrix requirement whose
# label the figure carries, and how the engine reproduces the paper (plain words, from the README).
AREA = {"2607.06212": "lunar", "2607.02566": "lunar", "2606.24210": "timing", "2607.05415": "resilience", "2606.22054": "rf"}
MATRIX_ROW = {
    "2607.06212": "Lunar surface-beacon DOP augmentation",
    "2607.02566": "Lunar absolute-station observability (datum defect)",
    "2606.24210": "Timing Protection Level under spoofing",
    "2607.05415": "Resilience-score decision-instability study",
    "2606.22054": "AI/ML RF-impairment optimism-gap study & ID-only gap predictor",
}
METHOD = {
    "2607.06212": "The lunar-beacon kind builds the ranging geometry of an orbital constellation plus the surveyed surface beacons that clear the airless horizon, on the dilution-of-precision kernel checked against an independent library, and reports each configuration's dilution of precision and its accuracy in metres.",
    "2607.02566": "The lunar-joint-od-clock kind fuses Earth-baseline interferometry delays, lunar-local ranges and inter-satellite ranges in one batch least-squares fit, runs it with and without the interferometry legs on the same seed, and reports the rank of the Fisher information, the datum defect and the Cramér–Rao bound.",
    "2606.24210": "The Timing Protection Level module adds a monitor's detection floor to the oscillator's coast over the detection latency. The example calibrates it with the scalars recovered from the public JammerTest 2024 recording; the raw recording is not shipped.",
    "2607.05415": "The resilience module scores seven reference architectures across the seven framework categories and five threats, draws the category weights at random from a Dirichlet distribution and counts how often the top rank changes. The example writes the whole study as one deterministic file.",
    "2606.22054": "The impairment study generates a synthetic, parameter-grounded corpus for four interference classes, scores thirteen detectors at four shift severities over five seeds, and fits the gap predictor with leave-one-out cross-validation and a permutation test.",
}


def sh(*a, cwd=None):
    return subprocess.run(a, capture_output=True, text=True, check=True, cwd=cwd).stdout


def matrix_label(repo, requirement):
    rows = json.load(open(os.path.join(repo, "web", "data", "verification-matrix.json"), encoding="utf-8"))["rows"]
    hit = [r for r in rows if r["requirement"] == requirement]
    assert len(hit) == 1, f"verification matrix: no single row {requirement!r}"
    return hit[0]["status"]


def run_scenario(repo, stem, work):
    d = os.path.join(work, stem)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(os.path.join(repo, "scenarios", stem + ".toml"), d)
    t = time.time()
    sh(os.path.join(repo, "target", "release", "kshana"), stem + ".toml", cwd=d)
    res = json.load(open(os.path.join(d, stem + ".result.json"), encoding="utf-8"))
    rep = json.load(open(os.path.join(d, stem + ".report.json"), encoding="utf-8"))["reproducibility"]
    return res, rep, time.time() - t


def run_example(repo, name, args, work):
    exe = os.path.join(repo, "target", "release", "examples", name)
    if not os.access(exe, os.X_OK):
        t = time.time()
        subprocess.run(["cargo", "build", "--release", "--example", name], cwd=repo, check=True, env={**os.environ, "CARGO_BUILD_JOBS": "6"})
        print(f"built example {name} in {time.time() - t:.0f} s", file=sys.stderr)
    d = os.path.join(work, name)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(os.path.join(d, "paper-artifacts"))
    t = time.time()
    out = sh(exe, *([args] if args else []), cwd=d)
    return out, (json.load(open(os.path.join(d, args), encoding="utf-8")) if args else None), time.time() - t


def f3(v):
    return float(f"{v:.4g}")


def figure(repo, pid, spec, work):
    """Run the paper's capability on the engine and keep only the numbers the card plots."""
    if pid == "2607.06212":
        res, rep, dt = run_scenario(repo, "lunar-beacon", work)
        items = [{"label": r["label"], "value": f3(r["dop"]["gdop"]), "note": f'{r["n_visible_sats"]} satellites and {r["n_visible_beacons"]} beacons in view · {r["accuracy"]["pos_3d_m"]:.1f} m (1 sigma)'} for r in res["rows"]]
        fig = {"type": "hbar", "scale": "linear", "axis": "Geometric dilution of precision at the south-polar user (lower is better)", "items": items,
               "caption": "One epoch of the bundled scenario, its default geometry: the satellites alone, the satellites plus the beacons that clear the horizon, and a 24-satellite constellation instead.",
               "seed": rep.get("seed"), "source": "scenarios/lunar-beacon.toml"}
        find = [f'In the bundled run, the one beacon that clears the horizon cuts the geometric dilution of precision from {items[0]["value"]:g} to {items[1]["value"]:g}; a 24-satellite constellation instead reaches {items[2]["value"]:g}.',
                "The satellites a south-polar user sees bunch into a small patch overhead, so the geometry is limited by their spread, not their number.",
                "A few beacons on elevated terrain reach geometry that an orbit-only design reaches only with a much larger fleet."]
    elif pid == "2607.02566":
        res, rep, dt = run_scenario(repo, "lunar-joint-od-clock", work)
        w, wo, ob, obn = res["with_vlbi"], res["without_vlbi"], res["observability"], res["observability_without_vlbi"]
        items = [{"label": "Without the interferometry tie", "value": f3(wo["station_pos_err_m"]), "note": f'Fisher information rank {obn["rank"]} of {obn["n_params"]}: the station position is not observable, no Cramér–Rao bound'},
                 {"label": "With the Earth-baseline interferometry tie", "value": f3(w["station_pos_err_m"]), "note": f'full rank {ob["rank"]} of {ob["n_params"]} · Cramér–Rao bound {ob["station_pos_crlb_m"]:.1f} m'}]
        fig = {"type": "hbar", "scale": "log", "unit": "m", "axis": "Station absolute-position error of the fit, metres (log scale)", "items": items,
               "caption": f'The bundled simulated network: {res["n_sat"]} lunar satellites, {res["n_earth"]} Earth stations, one surface station, the same seed and truth for both fits.',
               "seed": rep.get("seed"), "source": "scenarios/lunar-joint-od-clock.toml"}
        # The paper's figure was recorded on macOS; the joint fit stops on a host-dependent iterate
        # (kshana issue #36, row 6), so a Linux x86-64 run differs in the last digits. The figure keeps the
        # published values and says so, with the fresh ones, for as long as the two differ.
        PUBLISHED = (2182.0, 3.547)
        fresh = (items[0]["value"], items[1]["value"])
        if fresh != PUBLISHED:
            items[0]["value"], items[1]["value"] = PUBLISHED
            fig["provenance_note"] = (f'Recorded on macOS as published. A fresh Linux x86-64 run gives {fresh[0]:g} m / {fresh[1]:g} m '
                                      '(solver iterate host dependence, issue #36 row 6).')
        find = ["Ranging inside the lunar network leaves the cluster free to shift and rotate unseen; the clocks stay observable, so the defect is purely positional.",
                f'In the engine run ({res["n_sat"]} satellites), the Fisher information without the interferometry legs has rank {obn["rank"]} of {obn["n_params"]}; with them it has full rank and a Cramér–Rao bound of {ob["station_pos_crlb_m"]:.1f} m.',
                "With a rich constellation the interferometry tie only sharpens the bound; with a sparse one it restores the station's absolute position."]
    elif pid == "2606.24210":
        out, _, dt = run_example(repo, "tpl_jammertest", "", work)
        pts = [[float(a), float(b)] for a, b in re.findall(r"latency\s+([\d.]+) s\s+->\s+TPL\s+([\d.]+) ns", out)]
        pull = float(re.search(r"OBSERVED uncontrolled pull\s*:\s*([\d.]+) ns", out).group(1))
        claim = float(re.search(r"CLAIMED accuracy\s*:\s*<=\s*([\d.]+) ns", out).group(1))
        assert len(pts) >= 4, out
        fig = {"type": "tpl", "points": pts, "refs": [{"label": "time error the receiver served, recorded attack", "value": pull}, {"label": "accuracy the receiver claimed", "value": claim}],
               "axis": "Nanoseconds (log scale) against detection latency, seconds",
               "caption": "The example's printout: the conditional Timing Protection Level at five detection latencies, and the two scalars it recovered from the public JammerTest 2024 recording.",
               "seed": None, "source": "examples/tpl_jammertest.rs"}
        find = [f"A public recorded spoof pulled the receiver's served time by about {pull / 1e6:.2f} ms while it reported at most {claim:g} ns.",
                "Against a slow enough ramp no finite unconditional bound exists, so the bound is conditional on an independent cross-satellite check detecting the attack.",
                f"The engine's Timing Protection Level is {pts[0][1]:g} ns at {pts[0][0]:g} s detection latency and {pts[-1][1]:g} ns at {pts[-1][0]:g} s."]
    elif pid == "2607.05415":
        out, art, dt = run_example(repo, "resilience_report", spec["args"], work)
        per = art["h1_weighting_only_per_scenario"]
        names = {"nominal": "Nominal", "wideband_jam": "Wideband jamming", "spoofing": "Spoofing", "meaconing": "Meaconing", "combined": "Combined threats"}
        items = [{"label": names.get(k, k), "value": f3(v["top1_flip_rate"] * 100)} for k, v in sorted(per.items(), key=lambda kv: (list(names).index(kv[0]) if kv[0] in names else 99))]
        fig = {"type": "hbar", "scale": "linear", "unit": "%", "axis": "Draws in which re-weighting alone changes the top-ranked architecture, per cent", "items": items,
               "caption": f'{art["provenance"]["n_architectures"]} reference architectures, 2,000 random category weightings per threat.',
               "seed": art["provenance"]["seed"], "source": "examples/resilience_report.rs"}
        others = max(v["top1_flip_rate"] for k, v in per.items() if k != "nominal") * 100
        find = [f'Re-weighting alone changes the top-ranked architecture in {per["nominal"]["top1_flip_rate"] * 100:.0f}% of draws under nominal conditions, and in at most {f3(others):g}% under each active threat.',
                "A weakest-link maturity level depends on the threat assumed, not only on the architecture.",
                "Because the composite rewards declared techniques, a design that declares more can outscore a more resilient one."]
    elif pid == "2606.22054":
        out, art, dt = run_example(repo, "optimism_study", spec["args"], work)
        cv = art["gap_predictor"]["cv_leave_one_detector_out"]
        pts = [[f3(q["predicted"]), f3(q["actual"])] for q in cv["scatter"]]
        fig = {"type": "scatter", "points": pts, "r2": f3(cv["r2"]), "axis": "Predicted against actual optimism gap (area under the curve lost under shift)",
               "caption": f'{len(pts)} detector, class and severity cells; each prediction is made for a detector left out of the fit.',
               "seed": ", ".join(str(x) for x in art["provenance"]["seeds"]), "source": "examples/optimism_study.rs"}
        cls = art["gap_predictor"]["cv_leave_one_class_out"]["r2"]
        find = ["The gap between in-distribution and shifted area under the curve grows as the shift deepens, and depends on how many observables a detector uses rather than on whether it is learned.",
                f'In the engine run, a ridge model on in-distribution score statistics predicts the gap for a detector it has not seen (R² {cv["r2"]:.2f}) and for an interference class it has not seen (R² {cls:.2f}).',
                "The headline findings are synthetic; the paper then checks them on open field recordings."]
    fig["label"] = matrix_label(repo, MATRIX_ROW[pid])
    fig["capability"] = MATRIX_ROW[pid]
    fig["seconds"] = round(dt, 2)
    return fig, find


def fetch(ids):
    url = API.format(ids=",".join(ids), n=len(ids) + 5)
    for attempt in range(4):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "kshana-site-publications/1"})
            return urllib.request.urlopen(req, timeout=60).read()
        except Exception as e:  # the API rate-limits bursts; back off and retry
            if attempt == 3:
                raise
            print(f"arXiv API: {e}; retrying", file=sys.stderr)
            time.sleep(5 * (attempt + 1))


def arxiv_fields(ids):
    root = ET.fromstring(fetch(ids))
    got = {}
    for e in root.findall("a:entry", NS):
        m = re.search(r"abs/(\d{4}\.\d{4,5})v(\d+)$", e.find("a:id", NS).text.strip())
        if not m:
            continue
        pid, ver = m.group(1), int(m.group(2))
        prim = e.find("x:primary_category", NS)
        got[pid] = {
            "title": " ".join(e.find("a:title", NS).text.split()),
            "authors": [a.find("a:name", NS).text.strip() for a in e.findall("a:author", NS)],
            "published": e.find("a:published", NS).text[:10],
            "updated": e.find("a:updated", NS).text[:10],
            "version": ver,
            "category": prim.get("term") if prim is not None else "",
            "abstract": " ".join(e.find("a:summary", NS).text.split()),
        }
    missing = sorted(set(ids) - set(got))
    if missing:
        sys.exit(f"not live on arXiv (the API does not return them): {missing}. List only live papers.")
    return got


def reproduce(repo, spec):
    """The capability behind a paper, checked against the engine checkout."""
    if "scenario" in spec:
        f = os.path.join(repo, "scenarios", spec["scenario"] + ".toml")
        kind = re.search(r'^kind\s*=\s*"([^"]+)"', open(f, encoding="utf-8").read(), re.M).group(1)
        s = spec["scenario"]
        return {"kind": kind, "file": f"scenarios/{s}.toml", "studio": s,
                "command": f"kshana example {s} > {s}.toml\nkshana {s}.toml"}
    ex = spec["example"]
    src = open(os.path.join(repo, "examples", ex + ".rs"), encoding="utf-8").read()
    cmd = f"cargo run --release --example {ex}" + (f" -- {spec['args']}" if spec["args"] else "")
    readme = open(os.path.join(repo, "README.md"), encoding="utf-8").read()
    assert cmd in readme or cmd in src, f"{cmd!r} is in neither README.md nor examples/{ex}.rs"
    return {"example": f"examples/{ex}.rs", "command": cmd}


def bib_key(p):
    fam = p["authors"][0].split()[-1].lower()
    word = next(w for w in re.findall(r"[A-Za-z]+", p["title"].lower()) if w not in STOP)
    return f"{fam}{p['published'][:4]}{word}"


def main():
    repo = engine_tree()
    ids = list(PAPERS)
    work = os.path.join(HERE, "..", "..", ".build", "pub-runs")
    os.makedirs(work, exist_ok=True)
    eng = sh(os.path.join(repo, "target", "release", "kshana"), "--version").split()[-1]
    engine = {"version": eng, "commit": sh("git", "-C", repo, "rev-parse", "HEAD")[:7],
              "dirty": bool(sh("git", "-C", repo, "status", "--porcelain", "--untracked-files=no"))}
    if "--offline" in sys.argv:
        old = {p["arxiv"]: p for p in json.load(open(OUT, encoding="utf-8"))["papers"]}
        fields = {i: {k: old[i][k] for k in ("title", "authors", "published", "updated", "version", "category", "abstract")} for i in ids}
    else:
        fields = arxiv_fields(ids)
    papers = []
    for pid in ids:
        summary, spec = PAPERS[pid]
        p = {"arxiv": pid, **fields[pid], "summary": summary, "doi": f"10.48550/arXiv.{pid}",
             "reproduce": reproduce(repo, spec)}
        p["bibkey"] = bib_key(p)
        p["area"], p["method"] = AREA[pid], METHOD[pid]
        p["figure"], p["findings"] = figure(repo, pid, spec, work)
        print(f'  ran {pid}: {p["figure"]["source"]} in {p["figure"]["seconds"]} s', file=sys.stderr)
        papers.append(p)
    papers.sort(key=lambda p: p["published"], reverse=True)
    out = {"_comment": "GENERATED by src/tools/gen_publications.py from the arXiv API and the engine checkout; do not edit. "
                       "Only papers live on arXiv are listed. summary is a plain paraphrase of the abstract; figure holds numbers from a real engine run.",
           "engine": engine, "papers": papers}
    json.dump(out, open(OUT, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    open(OUT, "a").write("\n")
    for p in papers:
        print(p["arxiv"], f"v{p['version']}", p["published"], p["reproduce"]["command"].replace("\n", " ; "))


if __name__ == "__main__":
    main()
