#!/usr/bin/env python3
"""Generate src/js/evidence-data.mjs: the data behind the Evidence page's validated-results bento.

Every number is computed by the engine's own validated modules against the same external
oracle its tests use, or read from the engine's committed validation record. Nothing is typed.

  sgp4      src/sgp4.rs on all AIAA 2006-6753 verification vectors (tests/fixtures/sgp4/):
            the position error of every compared row (the test prints only the worst)
  cowell    src/propagator.rs against the Orekit 12.2 Dormand-Prince 8(5,3) reference states
            (tests/fixtures/numerical_cowell_propagator/): |dr| at every epoch, per force tier
  allan     src/allan.rs on the NIST SP 1065 section 12.4 1000-point data set (generated in code,
            as the test does): four deviations across the averaging ladder, plus the SP 1065
            Table 31 values the test asserts against (parsed from tests/allan_nist_sp1065_1000point.rs)
  raim      src/raim.rs kernels on every SciPy case in tests/raim_reference.rs (cases, tolerances
            and SciPy values parsed from the test source)
  holdover  src/slot_timing.rs on the measured Cs 5071A record (realdata-cache/cs5071a, fetched
            by the engine's scripts/fetch_cs5071a.sh): fit on the first third, predicted and
            measured breach on the held-out two thirds, and the naive control
  orbits    the residual tables of docs/AGENCY-ORBIT-VALIDATION.md (Galileo E11, Swarm-A, LRO)

The engine tree is the site's one engine checkout (src/data/sources.json: repo; KSHANA_ENGINE_TREE
overrides it). The probe (src/tools/evidence_probe/main.rs) links its `kshana` crate by path and is
built in the site project's scratch directory (scratch/evidence-probe; KSHANA_PROBE_BUILD overrides
it), outside the engine tree and outside the published site, so neither is written.

usage:  /opt/homebrew/bin/python3 src/tools/gen_evidence_data.py           write the module
        /opt/homebrew/bin/python3 src/tools/gen_evidence_data.py --check   exit 1 if it is stale
"""
import json, math, os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(HERE)
OUT = os.path.join(SRC, "js", "evidence-data.mjs")
sys.path.insert(0, HERE)
from _sources import engine_tree, studio_dir  # noqa: E402  (src/data/sources.json: the one engine checkout)
TREE = engine_tree()
BUILD = os.path.expanduser(os.environ.get("KSHANA_PROBE_BUILD") or os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(SRC))), "scratch", "evidence-probe"))


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def rd(rel):
    return open(os.path.join(TREE, rel), encoding="utf-8").read()


def raim_cases():
    """Every (kernel, args..., scipy, abstol, reltol) row of tests/raim_reference.rs."""
    t = rd("tests/raim_reference.rs")
    rows = []
    for blk in t.split("#[test]")[1:]:
        m = re.search(r"close\(\s*(\w+)\([^)]*\),\s*want,\s*([-\d.eE+]+),\s*([-\d.eE+]+)", blk)
        c = re.search(r"let cases:[^=]*=\s*&\[(.*?)\];", blk, re.S)
        if not (m and c):
            continue
        for tup in re.findall(r"\(([^()]*)\)", c.group(1)):
            vals = [v.strip() for v in tup.split(",") if v.strip()]
            rows.append([m.group(1)] + vals + [m.group(2), m.group(3)])
    return rows


def sp1065_table31():
    """The SP 1065 Table 31 values tests/allan_nist_sp1065_1000point.rs asserts against."""
    t = rd("tests/allan_nist_sp1065_1000point.rs")
    out = {}
    for est, key in (("overlapping_adev", "oadev"), ("modified_adev", "mdev"), ("time_deviation", "tdev"), ("hadamard_adev", "ohdev")):
        m = re.search(r"for \(m, want\) in \[([^\]]*)\]\s*\{\s*let got = " + est + r"\(", t)
        out[key] = [[int(a), float(b)] for a, b in re.findall(r"\((\d+)(?:usize)?,\s*([-\d.eE+]+)\)", m.group(1))]
    return out


def orbit_tables():
    """The residual tables of docs/AGENCY-ORBIT-VALIDATION.md, per dataset."""
    t = rd("docs/AGENCY-ORBIT-VALIDATION.md")
    out = []
    # headings are matched by their first word, so a spelled-out orbit name ("medium Earth orbit") still matches
    for sec, name in (("### Galileo ", "Galileo E11"), ("### Swarm-A ", "Swarm-A"), ("### LRO ", "LRO")):
        assert t.count("\n" + sec) == 1, f"docs/AGENCY-ORBIT-VALIDATION.md: expected one heading starting {sec!r}"
        body = t[t.index("\n" + sec) + 1:]
        body = body[: body.index("\n### ", 5)] if "\n### " in body[5:] else body
        tbl = body[body.index("| Run |"):]
        for line in tbl.splitlines()[2:]:
            if not line.startswith("|"):
                break
            c = [x.strip() for x in line.strip("|").split("|")]
            rms = float(re.search(r"([\d.]+) m", c[4]).group(1))
            rtn = [float(x) for x in re.findall(r"[\d.]+", c[5])[:3]]
            out.append({"set": name, "run": c[0].replace("`", ""), "arc": c[1], "do": c[2], "n": int(c[3]), "rms_m": rms, "rtn_m": rtn})
    bar = float(re.search(r"GREEN\*\* \(< ([\d.]+) m bar\)", t).group(1))
    return out, bar


def probe():
    os.makedirs(os.path.join(BUILD, "src"), exist_ok=True)
    open(os.path.join(BUILD, "Cargo.toml"), "w").write(
        '[package]\nname = "evidence_probe"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n\n'
        f'[dependencies]\nkshana = {{ path = "{TREE}" }}\n\n[workspace]\n\n[profile.release]\nopt-level = 3\n')
    src = open(os.path.join(HERE, "evidence_probe", "main.rs")).read()
    dst = os.path.join(BUILD, "src", "main.rs")
    if not os.path.exists(dst) or open(dst).read() != src:
        open(dst, "w").write(src)
    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=BUILD, check=True)
    cases = os.path.join(BUILD, "raim-cases.txt")
    open(cases, "w").write("\n".join(" ".join(r) for r in raim_cases()) + "\n")
    hold = cs5071a()
    args = [os.path.join(BUILD, "target", "release", "evidence_probe"), TREE, cases] + ([hold] if hold else [])
    return json.loads(sh(*args, cwd=BUILD))


def cs5071a():
    """The measured Cs 5071A record, as scripts/fetch_cs5071a.sh caches it (git-ignored, third-party
    data). Looked for in the engine tree, then in ~/Code/kshana; the .gz must carry the SHA-256 the
    engine's fetch script pins, so the record is the one the VALIDATED row was checked against."""
    import gzip, hashlib
    want = re.search(r'EXPECT_SHA256="([0-9a-f]{64})"', rd("scripts/fetch_cs5071a.sh")).group(1)
    for base in (TREE, os.path.expanduser("~/Code/kshana")):
        d = os.path.join(base, "realdata-cache", "cs5071a")
        gz, txt = os.path.join(d, "5071A_phase.txt.gz"), os.path.join(d, "5071A_phase.txt")
        if os.path.exists(gz) and hashlib.sha256(open(gz, "rb").read()).hexdigest() == want:
            if not os.path.exists(txt) or open(txt, "rb").read() != gzip.decompress(open(gz, "rb").read()):
                continue
            return txt
    return None


def sig(x, n=4):
    return float(f"{x:.{n}g}") if isinstance(x, (int, float)) and x is not None and math.isfinite(x) else x


def generate():
    p = probe()
    version = re.search(r'^version\s*=\s*"([^"]+)"', rd("Cargo.toml"), re.M).group(1)
    commit = sh("git", "-C", TREE, "rev-parse", "HEAD")[:7]
    dirty = bool(sh("git", "-C", TREE, "status", "--porcelain", "--untracked-files=no"))
    # --- SGP4: one cell per compared row, grouped by test satellite
    sg = p["sgp4"]
    sg_all = [e for c in sg for e in c["err_m"]]
    sgp4 = {"cells": [[c["sat"], 1 if c["deep"] else 0, [sig(e, 3) for e in c["err_m"]]] for c in sg],
            "n": len(sg_all), "sats": len(sg), "worst_m": sig(max(sg_all)), "median_m": sig(sorted(sg_all)[len(sg_all) // 2]),
            "tol_m": 2.0e-5 * 1000, "deep_rows": sum(len(c["err_m"]) for c in sg if c["deep"])}
    tolm = re.search(r"const TOL_KM: f64 = ([\d.eE+-]+);", rd("tests/sgp4_verification.rs"))
    sgp4["tol_m"] = sig(float(tolm.group(1)) * 1000)
    # --- Cowell: |dr| per tier and regime at every epoch
    cw = p["cowell"]
    tiers = {"T1": "two-body", "T2": "+ J2", "T3": "+ J2 to J6 zonal", "T4": "+ Sun and Moon", "T5": "+ solar radiation pressure", "T6": "+ drag"}
    cons = [e for c in cw if c["tier"] != "T6" for e in c["err_m"]]
    cowell = {"series": [{"tier": c["tier"], "label": tiers[c["tier"]], "regime": c["regime"], "t_h": [sig(t / 3600, 4) for t in c["t_s"]], "err_m": [sig(e, 3) for e in c["err_m"]]} for c in cw],
              "worst_conservative_m": sig(max(cons)), "epochs": sum(len(c["t_s"]) for c in cw),
              "t6_worst_m": sig(max(e for c in cw if c["tier"] == "T6" for e in c["err_m"])),
              "tol_m": {k: float(v) for k, v in re.findall(r'"(T\d)" => ([\d.eE+-]+),', rd("tests/numerical_cowell_propagator_reference.rs"))},
              "epochs_conservative": sum(len(c["t_s"]) for c in cw if c["tier"] != "T6")}
    # --- Allan
    al = p["allan"]
    allan = {"m": al["m"], "curves": {k: [sig(v, 5) for v in al[k]] for k in ("oadev", "mdev", "tdev", "ohdev")}, "ref": sp1065_table31(),
             "tol": float(re.search(r"const TOL: f64 = ([\d.eE+-]+);", rd("tests/allan_nist_sp1065_1000point.rs")).group(1))}
    worst_rel = 0.0
    for k, pts in allan["ref"].items():
        for pt in pts:                      # [m, SP 1065 value, the engine's value at full precision]
            got = al[k][al["m"].index(pt[0])]
            pt.append(got)
            worst_rel = max(worst_rel, abs(got - pt[1]) / pt[1])
    allan["worst_rel"] = sig(worst_rel, 2)
    # --- RAIM: fraction of each case's tolerance band used
    rm = p["raim"]
    raim = {"cells": [[r["fn"], sig(abs(r["got"] - r["want"]) / r["band"], 2)] for r in rm], "n": len(rm),
            "pass": sum(1 for r in rm if abs(r["got"] - r["want"]) <= r["band"]),
            "kernels": sorted({r["fn"] for r in rm}, key=[r["fn"] for r in rm].index)}
    # --- holdover
    h = p["holdover"]
    hold = None
    if h:
        bar = float(re.search(r"const BAR: f64 = ([\d.]+);", rd("tests/slot_timing_cs5071a_holdout.rs")).group(1))
        hold = {"samples": h["samples"], "fit": h["split"], "syncs": h["syncs"], "bar": bar,
                "rows": [{k: sig(v, 4) for k, v in r.items()} for r in h["rows"]], "naive_ratio": sig(h["naive_ratio_1ns"], 3),
                "measured": [[sig(t, 5), sig(v, 4)] for t, v in h["measured"]], "predicted": [[sig(t, 5), sig(v, 4)] for t, v in h["predicted"]]}
        rs = [r["ratio"] for r in hold["rows"]]
        hold["lo"], hold["hi"] = sig(min(rs), 3), sig(max(rs), 3)
    orbits, orbit_bar = orbit_tables()
    gal = next(r for r in orbits if r["set"] == "Galileo E11" and r["run"].startswith("Full-arc"))
    swa = next(r for r in orbits if r["set"] == "Swarm-A" and "reduced-dynamic" in r["run"])
    studio = {k: studio_run(f) for k, f in STUDIO_RUNS.items()}
    # The strings the page prints statically (data-ev="key"); evidence.mjs rewrites them from here
    # and --check fails when the page's static copy differs.
    facts = {
        "commit": commit, "sgp4_n": str(sgp4["n"]), "sgp4_worst_mm": f"{sgp4['worst_m'] * 1000:.2f}",
        "raim_n": str(raim["n"]), "cowell_worst": f"{cowell['worst_conservative_m']:.2f}",
        "galileo": f"{gal['rms_m']:.2f}", "swarm": f"{swa['rms_m']:.2f}",
        "allan_n": str(sum(len(v) for v in allan["ref"].values())),
        "hold_lo": f"{hold['lo']:.2f}" if hold else "", "hold_hi": f"{hold['hi']:.2f}" if hold else "", "hold_bar": f"{hold['bar']:g}" if hold else "",
    }
    facts.update({f"seed_{k}": str(v["seed"]) for k, v in studio.items() if v["seed"] is not None})
    return {
        "engine": {"version": version, "commit": commit, "dirty": dirty, "tree": sh("git", "-C", TREE, "rev-parse", "--abbrev-ref", "HEAD")},
        "sgp4": sgp4, "cowell": cowell, "allan": allan, "raim": raim, "holdover": hold,
        "orbits": {"rows": orbits, "bar_m": orbit_bar}, "studio": studio, "facts": facts,
    }


# The Studio recording each tile links to (the module at work in a recorded scenario).
PG_REC = os.path.join(studio_dir(), "recorded")
STUDIO_RUNS = {"sgp4": "orbit-sgp4-gps", "raim": "integrity-raim", "clock": "clock-holdover", "hold": "slot-timing-ocxo-leo"}


def studio_run(f):
    r = json.load(open(os.path.join(PG_REC, f + ".json"), encoding="utf-8"))
    m = re.search(r"^\s*seed\s*=\s*(\d+)", r["toml"], re.M)
    return {"file": f, "engine_version": r["engine_version"], "seed": int(m.group(1)) if m else None}


def check_page(d):
    """The page's static data-ev strings and Studio links must match the generated data."""
    page = open(os.path.join(SRC, "pages", "evidence.html"), encoding="utf-8").read()
    bad = []
    for key, txt in re.findall(r'data-ev="(\w+)">([^<]*)<', page):
        if key == "commit" and txt == "{{BUILD}}":
            continue  # build.py fills it with the checkout's commit and refuses to build unless this module was generated at that commit
        if d["facts"].get(key) != txt:
            bad.append(f"data-ev={key}: page says {txt!r}, generated {d['facts'].get(key)!r}")
    for f in re.findall(r"scenario=([\w-]+)&amp;", page):
        if f not in STUDIO_RUNS.values():
            bad.append(f"Studio link to {f} is not one of the generator's recorded runs")
    return bad


def render(d):
    return ("// GENERATED by src/tools/gen_evidence_data.py from the engine's validated modules and their\n"
            "// external oracles. Do not edit; re-run the generator (it has --check).\n"
            "export default " + json.dumps(d, separators=(",", ":"), ensure_ascii=False) + ";\n")


if __name__ == "__main__":
    data = generate()
    text = render(data)
    bad = check_page(data)
    if "--check" in sys.argv:
        cur = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if cur != text:
            bad.insert(0, "js/evidence-data.mjs is stale: run src/tools/gen_evidence_data.py")
        if bad:
            sys.exit("\n".join(bad))
        print("evidence-data.mjs is current; the page's static figures match it")
    else:
        open(OUT, "w", encoding="utf-8").write(text)
        print(f"wrote {os.path.relpath(OUT, SRC)} ({len(text):,} bytes)")
        for b in bad:
            print("WARNING page:", b)
