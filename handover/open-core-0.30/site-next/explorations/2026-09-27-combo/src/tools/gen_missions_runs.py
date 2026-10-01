#!/usr/bin/env python3
"""Record the engine runs behind the Missions page's orbit console and its next-release cards.

Runs the engine binary of the site's one engine checkout (src/data/sources.json: repo) on bundled
scenario files, each from a fresh scratch directory (the engine writes its outputs
next to the TOML it reads, so the worktree is never written), and writes:

  src/js/missions-runs.mjs                 what the page draws (below), as an ES module
  assets/missions-runs/<file>.report.html  the engine's own report, unmodified (orbit console runs)
  assets/missions-runs/results.json        the engine's result.json for the same runs, unmodified

Every number the page shows is read from a result.json or the scenario TOML; nothing is typed.

Orbit console (Missions #orbits), one entry per run:
  constellation-design  the Walker shells as the engine builds them (src/constellation.rs:
                        WalkerSpec::elements, secular J2 as Sat::new), re-propagated here and checked
                        against every sample of every ground track the engine emitted (the worst
                        angular residual is recorded; the script refuses more than 0.05 deg). The
                        GPS/Galileo/BeiDou/GLONASS presets are carried as the circular two-body orbits
                        src/tools/record_runs.py recovers for Home (same 0.2 deg check). Plus the
                        latitude profile of the engine's coverage grid (mean over longitude), and the
                        grid itself per cell (availability, mean and fewest in view, mean PDOP) for the map.
  body-pnt              the relay Walker and the orbiter user exactly as src/body_pnt.rs builds them
                        (EqOrbit with its J2 rates), re-propagated here; the satellites-in-view count
                        this reproduces must equal the engine's at every epoch, or the script stops.
                        Plus the engine's per-epoch receiver response.
Next-release cards: engine version, build, scenario SHA-256, seed and the engine's one-line summary.

build.py never runs the engine. Re-run after an engine change:
  /opt/homebrew/bin/python3 src/tools/gen_missions_runs.py
"""
import hashlib, json, math, os, re, shutil, subprocess, sys, tempfile, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(HERE)
SITE = os.path.dirname(SRC)
sys.path.insert(0, HERE)
import record_runs as RR  # noqa: E402  (Home's recorder: reused for the preset constellation fit)

from _sources import engine_tree  # noqa: E402  (src/data/sources.json: the one engine checkout)
WT = engine_tree()
ENGINE = os.path.join(WT, "target", "release", "kshana")
OUT_JS = os.path.join(SRC, "js", "missions-runs.mjs")
OUT_ASSETS = os.path.join(SITE, "assets", "missions-runs")

ORBIT = ["constellation-multi-gnss-coverage", "leo-pnt-mega-shell", "lunar-relay-constellation", "mars-orbit-pnt"]
# Runs whose report and result this page publishes (Home already publishes the multi-GNSS run under assets/runs/).
PUBLISH = ["leo-pnt-mega-shell", "lunar-relay-constellation", "mars-orbit-pnt", "europa-surface-pnt", "campaign-shared-jammer-sea-road"]
# Low Earth orbit (LEO) positioning, navigation and timing cards: system-agnostic scenario files only.
LEO = ["leo-pass-vs-gnss-cn0", "leo-doppler-positioning", "meo-leo-fused-pvt", "leo-ppp-convergence", "leo-pnt-end-to-end", "leo-timing-utc",
       "ntn-5g-positioning", "leo-navmsg-fit-interval-trade", "leo-band-trade",
       "leo-resilience-gnss-jammed-leo-carries", "leo-resilience-spoof-monitors", "leo-vertical-critical-infrastructure-timing",
       "leo-vertical-5g-network-timing", "leo-vertical-autonomous-vehicle", "leo-vertical-rail-maritime"]
CARDS = ["constellation-multi-gnss-coverage", "leo-pnt-mega-shell", "lunar-relay-constellation", "mars-orbit-pnt", "europa-surface-pnt",
         "solar-system-tour", "campaign-shared-jammer-sea-road", "campaign-jam-spoof-holdover-integrity", "l-band-waterfall-jamming",
         "campaign-sweep-jammer-power", "campaign-monte-carlo-clock-holdover", "campaign-spectrum-holdover-integrity"] + LEO
D2R = math.pi / 180


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def run(name):
    raw = open(os.path.join(WT, "scenarios", name + ".toml"), "rb").read()
    tmp = tempfile.mkdtemp(prefix="kshana-mx-")
    try:
        # The engine prints the command it was given into its report ("Command to reproduce").
        # Give it the scenario's place in the repository, scenarios/<file>.toml, from a scratch
        # directory laid out the same way, so a published report names a command a reader can
        # run and never a path on the machine that recorded it.
        rel = os.path.join("scenarios", name + ".toml")
        os.makedirs(os.path.join(tmp, "scenarios"))
        open(os.path.join(tmp, rel), "wb").write(raw)
        out = sh(ENGINE, rel, cwd=tmp)
        res = json.load(open(os.path.join(tmp, "scenarios", name + ".result.json")))
        rep = open(os.path.join(tmp, "scenarios", name + ".report.html"), encoding="utf-8").read()
        resraw = open(os.path.join(tmp, "scenarios", name + ".result.json"), encoding="utf-8").read()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return raw, res, resraw, rep, out.splitlines()[0]


def r(v, n=3):
    return None if v is None else round(v, n)


def kepler(m, e):
    E = m if e < 0.8 else math.pi
    for _ in range(60):
        d = (E - e * math.sin(E) - m) / (1 - e * math.cos(E))
        E -= d
        if abs(d) < 1e-13:
            break
    return E


# ------------------------------------------------------------------ constellation-design: Walker shells
def walker_shells(res, toml):
    """Shell parameters as the engine builds WalkerSpec, merged from the result and the TOML."""
    body = res["body"]
    R, mu = body["radius_km"], body["mu_km3_s2"]
    j2c = {"Earth": 1.08262668e-3}.get(body["name"])  # checked below against the tracks; the Moon run has j2 = false
    out = []
    for ci, (c_res, c_toml) in enumerate(zip(res["constellations"], toml["constellation"])):
        for s_res, s_toml in zip(c_res["shells"], c_toml.get("shell", [])):
            a = s_toml.get("semi_major_axis_km") or (R + s_res["altitude_km"])
            out.append({"c": ci, "pattern": s_res["pattern"], "total": s_res["total"], "planes": s_res["planes"], "phasing": s_res["phasing"],
                        "a_km": a, "e": s_toml.get("eccentricity", 0.0), "i": s_res["inclination_deg"], "raan0": s_toml.get("raan0_deg", 0.0),
                        "argp": s_toml.get("argp_deg", 0.0), "m0": s_toml.get("m0_deg", 0.0)})
    return out, R, mu, j2c


def expand(shells, R, mu, j2c, j2_on):
    sats = []
    for k, s in enumerate(shells):
        P, T = s["planes"], s["total"]
        d_raan = (2 * math.pi if s["pattern"] == "delta" else math.pi) / P
        d_m, d_f = 2 * math.pi * P / T, 2 * math.pi * s["phasing"] / T
        a = s["a_km"]
        n = math.sqrt(mu / a ** 3)
        rd = ad = 0.0
        if j2_on and j2c:
            p = a * (1 - s["e"] ** 2)
            f = n * j2c * (R / p) ** 2
            ci = math.cos(s["i"] * D2R)
            rd, ad = -1.5 * f * ci, 0.75 * f * (5 * ci * ci - 1)
        for pl in range(P):
            for j in range(T // P):
                sats.append({"shell": k, "a": a, "e": s["e"], "i": s["i"] * D2R, "raan": (s["raan0"] * D2R + pl * d_raan) % (2 * math.pi),
                             "argp": s["argp"] * D2R, "m0": (s["m0"] * D2R + j * d_m + pl * d_f) % (2 * math.pi), "n": n, "rd": rd, "ad": ad})
    return sats


def pos_fixed(s, t, w):
    E = kepler(s["m0"] + s["n"] * t, s["e"])
    rr = s["a"] * (1 - s["e"] * math.cos(E))
    nu = E if s["e"] == 0 else 2 * math.atan2(math.sqrt(1 + s["e"]) * math.sin(E / 2), math.sqrt(1 - s["e"]) * math.cos(E / 2))
    su, cu = math.sin(s["argp"] + s["ad"] * t + nu), math.cos(s["argp"] + s["ad"] * t + nu)
    si, ci = math.sin(s["i"]), math.cos(s["i"])
    so, co = math.sin(s["raan"] + s["rd"] * t - w * t), math.cos(s["raan"] + s["rd"] * t - w * t)
    x, y, z = rr * cu, rr * su * ci, rr * su * si
    return [x * co - y * so, x * so + y * co, z]


def check_tracks(res, sats):
    """Worst angle (deg) between the re-propagated sub-satellite point and every engine track sample."""
    w = res["body"]["rotation_rate_deg_s"] * D2R
    ts = res["tracks"]["times_s"]
    stride = res["tracks"]["stride"]
    worst, n = 0.0, 0
    for tr in res["tracks"]["satellites"]:
        sh_i, j = tr["id"][1:].split("-")
        # global index: the satellites of earlier shells, then j-1 in this one
        k = sum(1 for s in sats if s["shell"] < int(sh_i) - 1) + int(j) - 1
        assert k % stride == 0, (tr["id"], k, stride)
        s = sats[k]
        for la, lo, t in zip(tr["lat_deg"], tr["lon_deg"], ts):
            p = pos_fixed(s, t, w)
            u = RR.unit(la, lo)
            m = math.sqrt(RR.dot(p, p))
            worst = max(worst, math.degrees(math.acos(max(-1, min(1, RR.dot(p, u) / m)))))
            n += 1
    return worst, n


def lat_profile(g):
    lat = g["lat_deg"]
    # mean (or min / max) over the longitudes of each latitude row; a cell with no fix has no DOP
    # (null in the engine's grid) and is left out of that row's DOP, never counted as zero
    def row(key, fn=lambda v: sum(v) / len(v)):
        return [r(fn([x for x in v if x is not None]), 3) if any(x is not None for x in v) else None for v in g[key]]
    return {"lat": lat, "mean_visible": row("mean_visible"), "mean_pdop": row("mean_pdop"), "availability_pct": row("availability_pct"),
            "min_visible": row("min_visible", min), "max_pdop": row("max_pdop", max)}


def cell_grids(g):
    # the engine's per-cell grid for the orbit console map (row 0 is the southernmost latitude, column 0
    # is the westernmost longitude): mean and fewest satellites in view, and mean PDOP (null where the
    # cell never had a fix; the engine writes null there and the page draws it as "no fix", never as a number)
    def sig(v):
        return None if v is None else (round(v, 2) if v < 10 else round(v, 1) if v < 100 else round(v))
    return {"lon": g["lon_deg"], "mean_visible": [[round(v, 1) for v in row] for row in g["mean_visible"]],
            "min_visible": [[int(v) for v in row] for row in g["min_visible"]], "mean_pdop": [[sig(v) for v in row] for row in g["mean_pdop"]]}


def constellation_extract(name, res, toml):
    g = res["global"]
    body = res["body"]
    common = {"kind": "constellation-design", "body": body["name"], "radius_km": body["radius_km"], "rotation_rate_deg_s": body["rotation_rate_deg_s"],
              "duration_s": res["inputs"]["duration_s"], "mask_deg": res["inputs"]["mask_deg"], "pdop_threshold": res["inputs"]["pdop_threshold"],
              "epochs": res["inputs"]["epochs"], "j2": res["inputs"]["j2"], "total": res["total_satellites"],
              "constellations": [{"name": c["name"], "satellites": c["satellites"], "mean_visible": r(c["mean_visible"]), "source": c["source"]} for c in res["constellations"]],
              "global": {"availability_pct": r(g["availability_pct"], 2), "fix_pct": r(g["fix_pct"], 2), "pdop_median": g["pdop"]["median"], "pdop_p95": g["pdop"]["p95"],
                         "mean_visible": r(g["mean_visible"], 2), "min_visible": g["min_visible"], "worst_site_pct": r(g["worst_site_availability_pct"], 2)},
              "profile": lat_profile(res["grid"]), "avail_grid": [[round(v) for v in row] for row in res["grid"]["availability_pct"]],
              "cells": cell_grids(res["grid"]),
              "tracks_shown": res["tracks"]["shown"]}
    if all(c["shells"] for c in res["constellations"]) and "constellation" in toml and all(c.get("shell") for c in toml["constellation"]):
        shells, R, mu, j2c = walker_shells(res, toml)
        sats = expand(shells, R, mu, j2c, res["inputs"]["j2"])
        assert len(sats) == res["total_satellites"], (len(sats), res["total_satellites"])
        worst, n = check_tracks(res, sats)
        assert worst < 0.05, f"{name}: Walker re-propagation residual {worst:.4f} deg"
        common.update({"model": "walker", "shells": shells, "mu_km3_s2": mu, "j2c": j2c if res["inputs"]["j2"] else 0, "residual_deg": round(worst, 5), "residual_samples": n})
    else:
        c = RR.constellation_extract(res)
        common.update({"model": "circular", "sats": [{**s, "n": s["n"]} for s in c["sats"]], "residual_deg": c["residual_deg"],
                       "residual_samples": len(res["tracks"]["satellites"]) * len(res["tracks"]["times_s"])})
    return common


# ------------------------------------------------------------------ body-pnt: relays and orbiter as body_pnt.rs builds them
def eq_orbit(mu, re, j2, a, e, inc, raan0, u0):
    n = math.sqrt(mu / a ** 3)
    p = a * (1 - e * e)
    k = (j2 or 0.0) * (re / p) ** 2
    ci = math.cos(inc)
    return {"a": a, "e": e, "i": inc, "raan": raan0, "argp": 0.0, "m0": u0, "n": n, "rd": -1.5 * n * k * ci, "ad": 0.75 * n * k * (5 * ci * ci - 1),
            "md": n * (1 + 0.75 * k * math.sqrt(1 - e * e) * (3 * ci * ci - 1))}


def eq_pos(o, t):
    E = kepler(o["m0"] + o["md"] * t, o["e"])
    x, y = o["a"] * (math.cos(E) - o["e"]), o["a"] * math.sqrt(1 - o["e"] ** 2) * math.sin(E)
    sw, cw = math.sin(o["argp"] + o["ad"] * t), math.cos(o["argp"] + o["ad"] * t)
    so, co = math.sin(o["raan"] + o["rd"] * t), math.cos(o["raan"] + o["rd"] * t)
    si, ci = math.sin(o["i"]), math.cos(o["i"])
    return [(cw * co - sw * so * ci) * x + (-sw * co - cw * so * ci) * y, (cw * so + sw * co * ci) * x + (-sw * so + cw * co * ci) * y, (sw * si) * x + (cw * si) * y]


def chord_clears(a, b, R):
    d = [b[i] - a[i] for i in range(3)]
    dd = RR.dot(d, d)
    t = max(0.0, min(1.0, -RR.dot(a, d) / dd))
    c = [a[i] + t * d[i] for i in range(3)]
    return math.sqrt(RR.dot(c, c)) > R


def body_extract(name, res, toml):
    b = res["body"]
    mu, R, j2, re = b["gm_m3_s2"], b["radius_mean_m"], b["j2"], b["j2_reference_radius_m"] or b["radius_mean_m"]
    c, u = toml.get("constellation", {}), toml.get("user", {})
    P, S = c.get("planes", 3), c.get("sats_per_plane", 4)
    f, T = c.get("phasing_f", 1.0), P * S
    a = R + c.get("altitude_km", 3 * R / 1e3) * 1e3
    inc = c.get("inclination_deg", 60.0) * D2R
    relays = [eq_orbit(mu, re, j2, a, 0.0, inc, 2 * math.pi * p / P, 2 * math.pi * (s / S + f * p / T)) for p in range(P) for s in range(S)]
    ep = res["epochs"]
    out = {"kind": "body-pnt", "body": b["name"], "radius_m": R, "gm_m3_s2": mu, "j2": j2, "re_m": re, "rotation_period_h": b["sidereal_rotation_period_h"],
           "user": {k: u.get(k) for k in ("kind", "lat_deg", "lon_deg", "altitude_km", "inclination_deg", "eccentricity", "raan_deg", "u0_deg")},
           "relays": {"planes": P, "per_plane": S, "phasing_f": f, "altitude_km": c.get("altitude_km"), "inclination_deg": c.get("inclination_deg", 60.0),
                      "mask_deg": c.get("mask_deg", 10.0), "sigma_range_m": c.get("sigma_range_m", 1.0), "period_s": r(res["relay_period_s"], 1)},
           "earth_link": res["earth_link_used"], "light_time_s": r(res["earth_to_body"]["one_way_light_time_s"], 1),
           "sun_separation_deg": r(res["earth_to_body"]["sun_separation_deg"], 1), "epoch": res["epoch"]["input"],
           "fom": {k: r(v, 4) for k, v in res["fom"].items()},
           "t": [e["t_s"] for e in ep], "n_vis": [e["n_relays_visible"] for e in ep], "earth_vis": [1 if e["earth_visible"] else 0 for e in ep],
           "err_relays": [r(e["error_relays_m"], 3) for e in ep], "err_earth": [r(e["error_with_earth_m"], 3) for e in ep], "pdop": [r(e["pdop"], 3) for e in ep],
           "duration_s": toml.get("duration_s"), "step_s": toml.get("step_s")}
    if u.get("kind") == "orbiter":
        uo = eq_orbit(mu, re, j2, R + u.get("altitude_km", 400.0) * 1e3, u.get("eccentricity", 0.0), u.get("inclination_deg", 75.0) * D2R,
                      u.get("raan_deg", 0.0) * D2R, u.get("u0_deg", 0.0) * D2R)
        bad = 0
        for e in ep:
            up = eq_pos(uo, e["t_s"])
            nv = sum(1 for o in relays if chord_clears(up, eq_pos(o, e["t_s"]), R))
            bad += nv != e["n_relays_visible"]
            rr = math.sqrt(RR.dot(up, up))
            assert abs(rr - e["user_radius_m"]) < 1.0, (rr, e["user_radius_m"])
        assert bad == 0, f"{name}: re-propagated relays disagree with the engine's in-view count at {bad} epochs"
        out["check"] = {"epochs": len(ep), "in_view_mismatches": 0, "user_radius_max_err_m": 1.0}
    return out


# ------------------------------------------------------------------ campaign (compose) extract
def compose_extract(res, toml):
    c = res["compose"]
    mem = []
    for m in toml["compose"]["members"]:
        mr = c["members"][m["label"]]
        s = m["scenario"]
        mem.append({"label": m["label"], "kind": mr["kind"], "metrics": {k: r(v, 3) for k, v in mr["metrics"].items()},
                    "receiver": s.get("receiver"), "result_sha256": mr["result_sha256"][:12]})
    return {"kind": "campaign", "title": res["title"], "shared": {k: v["value"] for k, v in c["shared"].items()}, "members": mem,
            "combined": {k: {kk: (r(vv, 3) if isinstance(vv, float) else vv) for kk, vv in v.items()} for k, v in c["combined"].items()}}


# ------------------------------------------------------------------ LEO cards: headline figures, read from the result
def leo_chips(name, res):
    """Two to four [value, label] pairs for a card, each value read from the run's result.json and
    formatted here (nothing is typed). A scenario of a kind without an extract keeps the engine's
    own one-line summary, which the page prints instead."""
    def f(v, n):
        return f"{v:.{n}f}"
    if isinstance(res.get("comparison"), dict) and "leo_peak_cn0_dbhz" in res["comparison"]:      # leo-pass beside GNSS
        c = res["comparison"]
        return [[f"{f(c['leo_peak_cn0_dbhz'], 1)} dB-Hz", "low-orbit peak C/N0"], [f"{f(c['gnss_median_cn0_dbhz'], 1)} dB-Hz", "GNSS median C/N0"],
                [f"{f(c['leo_pass_duration_s'], 0)} s", "pass duration"]]
    if res.get("mode") == "doppler" and res.get("doppler"):
        d = res["doppler"]
        return [[f"{f(d['horizontal_error_m'], 1)} m", "horizontal error"], [str(d["n_sats"]), "satellites"], [str(d["n_obs"]), "Doppler measurements"]]
    if res.get("mode") == "joint" and res.get("joint"):
        j = res["joint"]
        return [[f"{f(j['gnss']['rms_error_3d_m'], 2)} m", "GNSS alone, RMS error"], [f"{f(j['fused']['rms_error_3d_m'], 2)} m", "fused with low orbit"],
                [f(j["fused"]["median_pdop"], 2), "median PDOP, fused"]]
    if res.get("mode") == "timing" and res.get("timing"):
        t = res["timing"]
        rms = [r_["stats"]["rms_s"] * 1e9 for r_ in t["rows"]]
        return [[f"{f(t['median_measurement_sigma_s'] * 1e9, 1)} ns", "median measurement sigma"], [f"{f(min(rms), 1)} to {f(max(rms), 1)} ns", "RMS time error across the cases"],
                [str(len(t["rows"])), "clock and signal-level cases"]]
    if res.get("cases") and "median_convergence_min" in res["cases"][0]:                           # leo-ppp
        a, b = res["cases"][0], res["cases"][-1]
        return [[f"{f(a['median_convergence_min'], 1)} min", "to converge, GNSS only"], [f"{f(b['median_convergence_min'], 1)} min", f"with {b['n_leo']} low-orbit satellites"],
                [f"{f(res['criterion_horizontal_m'] * 100, 0)} cm", "horizontal criterion"]]
    if res.get("kind") == "leo-pnt-chain":
        return [[f"{f(res['pass']['peak_cn0_dbhz'], 1)} dB-Hz", "peak C/N0 over the pass"], [f"{f(res['navmsg']['sisre_rms_m'], 2)} m", "signal-in-space range error, RMS"],
                [f"{f(res['fusion']['fused']['rms_error_3d_m'], 2)} m", "fused fix, RMS error"]]
    if "doppler_pass" in res and res.get("signals"):                                                 # ntn-positioning
        return [[str(res["n_satellites"]), "satellites"], [f(res["mean_in_view"], 1), "in view on average"]] + \
               [[f"{f(s['toa_median_sigma_3d_m'], 1)} m", f"median 3-D sigma, {s['name']}"] for s in res["signals"]]
    if res.get("fit_interval_trade"):
        model = res["message_config"]["model"]
        rows = sorted((x for x in res["fit_interval_trade"]["rows"] if x["model"] == model), key=lambda x: x["fit_interval_s"])
        return [[f"{f(x['sisre_max_m'] * 1000, 1)} mm", f"worst range error, {f(x['fit_interval_s'], 0)} s fit"] for x in (rows[0], rows[-1])]
    if res.get("kind") == "leo-signal" and res.get("trade"):
        best = min(res["trade"]["rows"], key=lambda x: x["jitter_equal_cn0_m"])
        return [[str(len(res["signals"])), "signal designs"], [f"{f(best['jitter_equal_cn0_m'] * 100, 1)} cm", "finest code jitter at equal C/N0"],
                [f"{f(best['centre_hz'] / 1e6, 0)} MHz", "its carrier"]]
    return None


def main():
    if not os.access(ENGINE, os.X_OK):
        sys.exit(f"engine binary not found: {ENGINE}")
    ver = sh(ENGINE, "--version").split()[-1]
    commit = sh("git", "-C", WT, "rev-parse", "HEAD")[:7]
    dirty = bool(sh("git", "-C", WT, "status", "--porcelain", "--untracked-files=no"))
    runs, results, cards, toml_text = {}, {}, {}, {}
    os.makedirs(OUT_ASSETS, exist_ok=True)
    for name in CARDS:
        raw, res, resraw, rep, line = run(name)
        toml = tomllib.loads(raw.decode("utf-8"))
        # the scenario's own opening sentence: its first comment paragraph, cut at the first full stop
        para = []
        for l in raw.decode("utf-8").splitlines():
            if not l.startswith("#"):
                break
            t = l.lstrip("#").strip()
            if not t:
                if para:
                    break
                continue
            para.append(t)
        first = " ".join(para)
        m = re.search(r"^(.+?[.!?])(\s|$)", first)
        first = m.group(1) if m else first
        # where the engine's own report, result and TOML are published on the site: this page's copies,
        # or the copies Home already publishes (src/tools/record_runs.py, src/tools/gen_waterfall.py)
        pub = None
        if name in PUBLISH:
            pub = {"report": f"assets/missions-runs/{name}.report.html", "json": "assets/missions-runs/results.json", "bundle": True, "toml": None}
        else:
            for d in (f"assets/runs/{name}", f"assets/waterfall/{name}"):
                if os.path.exists(os.path.join(SITE, d, "report.html")):
                    pub = {"report": f"{d}/report.html", "json": f"{d}/result.json", "bundle": False, "toml": f"{d}/{name}.toml.txt"}
                    break
        cards[name] = {"file": name + ".toml", "kind": toml.get("kind"), "sha256": hashlib.sha256(raw).hexdigest(), "seed": toml.get("seed"),
                       "summary": line, "about": first, "pub": pub}
        toml_text[name] = raw.decode("utf-8")
        if name in ORBIT:
            runs[name] = constellation_extract(name, res, toml) if res.get("kind") == "constellation-design" else body_extract(name, res, toml)
        elif name == "europa-surface-pnt":
            runs[name] = body_extract(name, res, toml)
        elif name == "campaign-shared-jammer-sea-road":
            runs[name] = compose_extract(res, toml)
        elif name in LEO:
            chips = leo_chips(name, res)
            if chips:
                runs[name] = {"kind": toml.get("kind"), "chips": chips}
        if name in PUBLISH:
            for w in ("claude", "anthropic", "aperio"):
                assert w not in rep.lower(), f"{name} report names {w}"
            open(os.path.join(OUT_ASSETS, name + ".report.html"), "w", encoding="utf-8").write(rep)
            results[name] = json.loads(resraw)
    json.dump(results, open(os.path.join(OUT_ASSETS, "results.json"), "w"), separators=(",", ":"))
    data = {"engine": {"version": ver, "commit": commit, "tree": sh("git", "-C", WT, "rev-parse", "--abbrev-ref", "HEAD"), "dirty": dirty}, "cards": cards, "runs": runs, "toml": toml_text,
            "publish": PUBLISH}
    js = ("// Generated by src/tools/gen_missions_runs.py from runs of the engine binary. Do not edit.\n"
          f"// engine {ver} @ {commit}{' (dirty)' if dirty else ''} · {len(cards)} scenario files\n"
          "export default " + json.dumps(data, ensure_ascii=False, separators=(",", ":")) + ";\n")
    open(OUT_JS, "w", encoding="utf-8").write(js)
    print(f"engine {ver} @ {commit}{' (dirty)' if dirty else ''}")
    for n, v in runs.items():
        extra = f"residual {v['residual_deg']} deg over {v['residual_samples']} samples" if "residual_deg" in v else (f"in-view check {v['check']}" if "check" in v else "")
        print(f"  {n}: {extra}")
    print(f"  wrote {os.path.relpath(OUT_JS, SITE)} ({os.path.getsize(OUT_JS) // 1024} KB), {len(PUBLISH)} reports + results.json")


if __name__ == "__main__":
    main()
