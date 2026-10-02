#!/usr/bin/env python3
"""Record the engine runs behind the Capabilities page's new visuals and write their data.

Runs the engine binary of the site's one engine checkout (src/data/sources.json: repo) on its own
bundled scenario files, each from a fresh scratch directory, so the worktree is never written:

  l-band-waterfall-jamming              spectrum             the signature L-band waterfall
  solar-system-tour                     solar-system         every body the engine knows, one epoch
  europa-surface-pnt, mars-orbit-pnt    body-pnt             positioning at a Jovian moon and at Mars
  constellation-multi-gnss-coverage     constellation-design GPS, Galileo, BeiDou and GLONASS
  leo-pnt-mega-shell                    constellation-design 5 000 satellites in four Walker shells
  lunar-relay-constellation             constellation-design a relay set around the Moon
  campaign-jam-spoof-holdover-integrity campaign (chain)     jamming, spoofing, holdover, integrity
  campaign-sweep-jammer-power           campaign (sweep)
  campaign-monte-carlo-clock-holdover   campaign (Monte Carlo)
  campaign-shared-jammer-sea-road       campaign (compose)

and reads the Studio's own recordings (the folder src/data/sources.json names as studio, recorded/)
for the six ways GNSS fails, so those open in Kshana Studio. The integrity and coasting cards read
the chained campaign's own channels.

Writes:
  assets/capabilities/capabilities.json     the compact extract the page draws (every number is
                                            read from a result.json, a recording or a scenario TOML)
(Two published files in all. The engine's per-run reports are not published for the runs no other
page publishes, because the host's 255-file limit is reached; each run's scenario TOML is inside the
JSON, and `kshana <file>.toml` writes the same report locally.)
Runs already published by another page (the waterfall and its campaign under assets/waterfall/,
the chained campaign and the multi-GNSS run under assets/runs/) are linked, not copied; the
script checks that the published result is byte-identical to the fresh run and stops if not.
The published file count is kept low on purpose (the host allows 255 files).

build.py never runs the engine; it publishes assets/ as it finds it.
Re-run after an engine change:  /opt/homebrew/bin/python3 src/tools/gen_capabilities.py
(and src/tools/fetch_body_imagery.py first if the body list changes).
"""
import hashlib, json, math, os, re, shutil, subprocess, sys, tempfile, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
from _sources import engine_tree, studio_dir  # noqa: E402  (src/data/sources.json: the one engine checkout)
WT = engine_tree()
ENGINE = os.path.join(WT, "target", "release", "kshana")
STUDIO_REC = os.path.join(studio_dir(), "recorded")
OUT = os.path.join(SITE, "assets", "capabilities")
ATLAS = os.path.join(HERE, "bodies-atlas.json")

REUSED = {  # published by another page: link, never copy
    "l-band-waterfall-jamming": "assets/waterfall/l-band-waterfall-jamming/",
    "constellation-multi-gnss-coverage": "assets/runs/constellation-multi-gnss-coverage/",
    "campaign-jam-spoof-holdover-integrity": "assets/runs/campaign-jam-spoof-holdover-integrity/",
}
REPORTED = set()  # the host allows 255 published files and the site is at the limit: no per-run reports here
RUNS = ["l-band-waterfall-jamming", "solar-system-tour", "europa-surface-pnt", "mars-orbit-pnt",
        "constellation-multi-gnss-coverage", "leo-pnt-mega-shell", "lunar-relay-constellation",
        "campaign-jam-spoof-holdover-integrity", "campaign-sweep-jammer-power",
        "campaign-monte-carlo-clock-holdover", "campaign-shared-jammer-sea-road"]
BAND_NAMES = {"gps-l1ca": "GPS L1 C/A", "galileo-e1": "Galileo E1", "gps-l2c": "GPS L2C", "gps-l5": "GPS L5", "galileo-e5a": "Galileo E5a"}


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def r(x, n=2):
    if x is None or (isinstance(x, float) and not math.isfinite(x)):
        return None
    return round(x, n)


def run(name):
    raw = open(os.path.join(WT, "scenarios", name + ".toml"), "rb").read()
    tmp = tempfile.mkdtemp(prefix="kshana-cap-")
    try:
        p = os.path.join(tmp, name + ".toml")
        open(p, "wb").write(raw)
        line = sh(ENGINE, p, cwd=tmp).splitlines()[0]
        res_raw = open(os.path.join(tmp, name + ".result.json"), "rb").read()
        if name in REPORTED:
            shutil.copyfile(os.path.join(tmp, name + ".report.html"), os.path.join(OUT, name + ".report.html"))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    if name in REUSED:
        pub = os.path.join(SITE, REUSED[name], "result.json")
        if not os.path.exists(pub) or open(pub, "rb").read() != res_raw:
            sys.exit(f"{name}: the copy published at {REUSED[name]} differs from a fresh run; re-run its owner's generator first")
    toml = raw.decode("utf-8")
    t = tomllib.loads(toml)
    res = json.loads(res_raw)
    seed = res.get("seed", t.get("seed"))
    meta = {"file": name + ".toml", "kind": t.get("kind"), "sha256": hashlib.sha256(raw).hexdigest(), "seed": seed,
            "engine_version": res.get("engine_version") or ENG["version"], "summary": line}
    if name in REUSED:
        meta["report"] = REUSED[name] + "report.html"
        meta["toml_url"] = REUSED[name] + name + ".toml.txt"
        meta["result_url"] = REUSED[name] + "result.json"
    else:
        meta["toml"] = toml
        if name in REPORTED:
            meta["report"] = f"assets/capabilities/{name}.report.html"
    return res, t, meta


# ------------------------------------------------------------------ engine identity
os.makedirs(OUT, exist_ok=True)
ver_line = sh(ENGINE, "--version")
ENG = {"version": ver_line.split()[-1], "commit": sh("git", "-C", WT, "rev-parse", "HEAD")[:7],
       "tree": sh("git", "-C", WT, "rev-parse", "--abbrev-ref", "HEAD"),
       "dirty": bool(sh("git", "-C", WT, "status", "--porcelain", "--untracked-files=no")),
       "kinds": len(json.loads(sh(ENGINE, "kinds", "--json")))}
if ENG["dirty"]:
    sys.exit("the engine worktree has uncommitted changes; record from a clean tree")
res, toml, meta = {}, {}, {}
for n in RUNS:
    res[n], toml[n], meta[n] = run(n)
    print(" ", meta[n]["summary"][:150])


# ------------------------------------------------------------------ spectrum (signature waterfall)
def spectrum():
    s = res["l-band-waterfall-jamming"]
    w = s["waterfall"]
    floor = w["noise_floor_dbw_per_hz"]
    # hundredths of a dB above the thermal floor, as integers: the only transform applied to the grid.
    # Hundredths, not tenths, because the satellite signals themselves add only 0.02-0.07 dB to the
    # floor in a 3 MHz cell; tenths rounded them away and left the signal-only windows blank.
    rows = [[max(0, round((v - floor) * 100)) for v in row] for row in w["psd_dbw_per_hz"]]
    tl = s["timeline"]
    bands = []
    for b, bt in zip(s["bands"], tl["bands"]):
        assert b["name"] == bt["name"]
        if b["name"] not in BAND_NAMES:
            sys.exit(f"band {b['name']} has no display name")
        bands.append({"id": b["name"], "name": BAND_NAMES[b["name"]], "centre_hz": b["centre_hz"], "rx_bw_hz": b["rx_bandwidth_hz"],
                      "modulation": b["modulation"], "nominal_cn0": r(b["nominal_cn0_dbhz"], 2), "signal_dbw": b["signal_power_dbw"],
                      "cn0": [r(v, 2) for v in bt["cn0_effective_dbhz"]], "js": [r(v, 2) for v in bt["js_db"]],
                      "lost": [1 if st == "LOST" else 0 for st in bt["status"]], "min_cn0": r(bt["min_cn0_dbhz"], 2),
                      "min_t": bt["min_cn0_t_s"], "first_loss": bt["first_loss_t_s"], "tracking_fraction": r(bt["tracking_fraction"], 4),
                      "worst_js": r(bt["worst_js_db"], 2)})
    jam = [{"name": j["name"], "waveform": j["waveform"], "centre_hz": j["centre_hz"], "bw_hz": j.get("bandwidth_hz") or 0,
            "on_s": j["on_s"], "off_s": j.get("off_s"), "eirp_dbw": j.get("eirp_dbw"), "range_m": j.get("range_m"),
            "received_dbw": r(j.get("received_power_dbw"), 2),
            "per_band": [{"band": pb.get("band"), "js": r(pb.get("js_db"), 2), "cn0": r(pb.get("cn0_effective_dbhz"), 2)} for pb in j["per_band"]]}
           for j in s["jammers"]]
    iq = s["iq"]
    return {"freq_hz": w["freq_hz"], "bin_hz": w["bin_width_hz"], "t_s": w["t_s"], "row_s": w["row_duration_s"],
            "floor_dbw_hz": r(floor, 3), "peak_db": r(w["peak_dbw_per_hz"] - floor, 2), "rows_scale": 100, "rows": rows,
            "source_bins": w["source_n_freq"], "bands": bands, "jammers": jam, "t": tl["t_s"],
            "threshold": s["receiver"]["tracking_threshold_dbhz"], "degraded_margin": s["receiver"]["degraded_margin_db"],
            "iq": {"t_s": iq["t_s"], "centre_hz": iq["centre_hz"], "f": [round(v) for v in iq["freq_offset_hz"]],
                   "welch": [r(v - floor, 2) for v in iq["welch_dbw_per_hz"]], "model": [r(v - floor, 2) for v in iq["model_dbw_per_hz"]],
                   "nfft": iq["nfft"], "segments": iq["segments"], "n": iq["n_samples"], "fs_hz": iq["sample_rate_hz"],
                   "power_ratio": r(iq["comparison"]["integrated_power_ratio"], 5), "label": iq["label"]},
            "not_modelled": s["not_modelled"], "label": s["label"]}


# ------------------------------------------------------------------ Studio recordings (six ways, integrity, coasting)
def rec(name):
    d = json.load(open(os.path.join(STUDIO_REC, name + ".json")))
    j = json.loads(d["json"]) if isinstance(d["json"], str) else d["json"]
    m = re.search(r"^\s*seed\s*=\s*(\d+)", d["toml"], re.M)
    seed = j.get("seed", int(m.group(1)) if m else None)
    return j, {"file": d["file"], "engine_version": d["engine_version"], "seed": seed, "summary": d["summary"]}


def thin(seq, n):
    if len(seq) <= n:
        return list(seq)
    step = (len(seq) - 1) / (n - 1)
    return [seq[round(i * step)] for i in range(n)]


def six():
    out = {}
    j, m = rec("jamming-demo")
    ep = j["epochs"]
    out["jam"] = {**m, "tab": "signal", "t": [e["t"] for e in ep], "visible": [e["visible"] for e in ep], "tracking": [e["tracking"] for e in ep],
                  "cn0": [r(sum(s["cn0_effective_dbhz"] for s in e["sats"]) / max(1, len(e["sats"])), 2) for e in ep],
                  "cn0_nom": [r(sum(s["cn0_nominal_dbhz"] for s in e["sats"]) / max(1, len(e["sats"])), 2) for e in ep],
                  "fom": {k: r(v, 3) for k, v in j["fom"].items()}}
    prns = sorted({sv["prn"] for e in ep for sv in e["sats"]})
    per = {p: [next((r(sv["cn0_effective_dbhz"], 2) for sv in e["sats"] if sv["prn"] == p), None) for e in ep] for p in prns}
    nom = {p: [next((r(sv["cn0_nominal_dbhz"], 2) for sv in e["sats"] if sv["prn"] == p), None) for e in ep] for p in prns}
    thr = re.search(r"DEFAULT_TRACKING_THRESHOLD_DBHZ: f64 = ([\d.]+)", open(os.path.join(WT, "src", "jamming.rs")).read())
    out["jam"].update({"prn": prns, "eff": [per[p] for p in prns], "nom": [nom[p] for p in prns], "threshold": float(thr.group(1))})
    j, m = rec("spoof-attack")
    out["spoof"] = {**m, "tab": "timeseries", "threshold_ns": j["threshold_ns"],
                    "clocks": [{"id": j[k]["id"], "detect_s": j[k]["detect_time_s"], "bound_ns": r(j[k]["min_detectable_ns"], 2),
                                "t": [p["t"] for p in j[k]["series"]], "offset": [r(p["offset_ns"], 3) for p in j[k]["series"]]} for k in ("quantum", "classical")]}
    j, m = rec("clock-holdover")
    out["clock"] = {**m, "tab": "holdover", "threshold_ns": j["threshold_ns"],
                    "clocks": [{"id": j[k]["spec"]["id"], "holdover_s": j[k]["fom"]["holdover_s"],
                                "t": [p["t"] for p in thin(j[k]["series"], 241)], "err": [r(abs(p["error_ns"]), 5) for p in thin(j[k]["series"], 241)],
                                "gnss": [p["gnss"] for p in thin(j[k]["series"], 241)]} for k in ("quantum", "classical")]}
    j, m = rec("ephemeris")
    sm = thin(j["samples"], 280)
    out["orbit"] = {**m, "tab": "ground", "source": j["source"], "lat": [r(p["lat_deg"], 2) for p in sm], "lon": [r(p["lon_deg"], 2) for p in sm],
                    "t": [r(p["t_s"], 1) for p in sm], "alt_km": [r(j["alt_min_km"], 1), r(j["alt_max_km"], 1)], "n": j["n_samples"]}
    j, m = rec("integrity-raim")
    out["integ"] = {**m, "tab": "timeseries", "al_h": j["al_h_m"], "al_v": j["al_v_m"], "t": [e["t_s"] for e in j["epochs"]],
                    "hpl": [r(e["hpl_m"], 2) for e in j["epochs"]], "vpl": [r(e["vpl_m"], 2) for e in j["epochs"]],
                    "avail": [1 if e["available"] else 0 for e in j["epochs"]], "n_ok": j["samples_available"], "n": j["samples_total"]}
    j, m = rec("small-uas-jammed-nav")
    out["coast"] = {**m, "tab": "timeseries", "threshold_m": j["threshold_m"],
                    "imus": [{"id": j[k]["spec"]["id"], "holdover_s": r(j[k]["fom"]["holdover_s"], 1),
                              "t": [r(p["t"], 1) for p in thin(j[k]["series"], 321)], "err": [r(p["error_m"], 3) for p in thin(j[k]["series"], 321)],
                              "denied": [0 if p["gnss"] == "nominal" else 1 for p in thin(j[k]["series"], 321)]} for k in ("quantum", "classical")]}
    return out


def ic_cards():
    """Section 03: the Stanford diagram of the RAIM run, and the INS coasting error budget."""
    j, m = rec("integrity-raim")
    pts = j["stanford"]["points"]
    regions = {}
    for q in pts:
        regions[q["region"]] = regions.get(q["region"], 0) + 1
    integ = {**m, "tab": "timeseries", "al": j["stanford"]["alert_limit_m"], "err": [r(q["error_m"], 3) for q in pts],
             "pl": [r(q["pl_m"], 3) for q in pts], "region": [q["region"] for q in pts], "regions": regions}
    j, m = rec("ins-trn-coast")
    dur_s = j["trn"]["mission_duration_s"]
    ts = [round(10 ** (math.log10(1.0) + k * (math.log10(dur_s) - math.log10(1.0)) / 120), 4) for k in range(121)]
    law = lambda c, t: c["coefficient_si"] * t ** c["exponent"]
    contrib = [{"name": c["name"], "law": c["law"], "class": c["class"], "e": [r(law(c, t), 5) for t in ts]} for c in j["contributions"] if c["coefficient_si"] > 0]
    assert j["combination"] == "rss", j["combination"]
    comb = lambda t: math.sqrt(sum(law(c, t) ** 2 for c in j["contributions"]))
    for c in j["crossings"]:  # the rss of the engine's own laws must land on the engine's own crossings
        assert abs(comb(c["coast_s"]) - c["threshold_m"]) / c["threshold_m"] < 0.005, (c, comb(c["coast_s"]))
    coast = {**m, "tab": "overview", "t": ts, "contrib": contrib, "combined": [r(comb(t), 5) for t in ts], "grade": j["imu"]["grade"],
             "speed": j["motion"]["speed_m_s"], "crossings": [{"thr": c["threshold_m"], "t": r(c["coast_s"], 2), "dom": c["dominant_contribution"]} for c in j["crossings"]],
             "grades": [{"grade": g["grade"], "x": [{"thr": x["threshold_m"], "t": r(x["coast_s"], 2)} for x in g["crossings"]]} for g in j["grade_table"]],
             "label": j["label"]}
    return {"integ": integ, "coast": coast}


# ------------------------------------------------------------------ constellation designer
def unit(lat, lon):
    la, lo = math.radians(lat), math.radians(lon)
    return (math.cos(la) * math.cos(lo), math.cos(la) * math.sin(lo), math.sin(la))


def angle(a, b):
    d = max(-1.0, min(1.0, sum(x * y for x, y in zip(a, b))))
    return math.acos(d)


def walker(shell, raan0):
    """Every satellite of a Walker shell from the engine's own shell parameters, at t = 0: plane p,
    slot k -> RAAN = raan0 + p * raan_spacing, argument of latitude = k * in_plane_spacing + p * phase_offset."""
    sats = []
    per = shell["total"] // shell["planes"]
    i = math.radians(shell["inclination_deg"])
    for p in range(shell["planes"]):
        O = math.radians(raan0 + p * shell["raan_spacing_deg"])
        for k in range(per):
            u = math.radians(k * shell["in_plane_spacing_deg"] + p * shell["phase_offset_deg"])
            sats.append((math.cos(O) * math.cos(u) - math.sin(O) * math.sin(u) * math.cos(i),
                         math.sin(O) * math.cos(u) + math.cos(O) * math.sin(u) * math.cos(i),
                         math.sin(u) * math.sin(i)))
    return sats


def constellation(name):
    c = res[name]
    t = toml[name]
    body = c["body"]
    w = math.radians(body["rotation_rate_deg_s"])
    times = c["tracks"]["times_s"]
    sats = []
    for s in c["tracks"]["satellites"]:
        # inertial unit vectors (body-fixed frame at t = 0), then the mean motion from the swept angle
        v = [unit(la, lo + math.degrees(w * tt)) for la, lo, tt in zip(s["lat_deg"], s["lon_deg"], times)]
        swept = sum(angle(a, b) for a, b in zip(v, v[1:]))
        n = swept / (times[-1] - times[0])
        a_km = (body["mu_km3_s2"] / n ** 2) ** (1 / 3)
        row = {"c": s["constellation"], "id": s["id"], "a_km": round(a_km, 1),
               "lat": [r(x, 2) for x in s["lat_deg"]], "lon": [r(x, 2) for x in s["lon_deg"]]}
        # an eccentric shell (the lunar frozen orbits): the radius at each sample from the shell's
        # own elements, with the argument of latitude recovered from the sub-satellite latitude
        m = re.match(r"S(\d+)-", s["id"])
        shells_t = (t.get("constellation") or [{}])[0].get("shell") or []
        if m and int(m.group(1)) <= len(shells_t) and shells_t[int(m.group(1)) - 1].get("eccentricity"):
            st = shells_t[int(m.group(1)) - 1]
            e, a, inc, om = st["eccentricity"], st["semi_major_axis_km"], math.radians(st["inclination_deg"]), math.radians(st.get("argp_deg", 0.0))
            rs = []
            la = s["lat_deg"]
            for k, lat in enumerate(la):
                su = max(-1.0, min(1.0, math.sin(math.radians(lat)) / math.sin(inc)))
                nxt = la[k + 1] if k + 1 < len(la) else la[k] + (la[k] - la[k - 1])
                u = math.asin(su) if nxt >= lat else math.pi - math.asin(su)
                rs.append(round(a * (1 - e * e) / (1 + e * math.cos(u - om)), 1))
            row["r_km"] = rs
        sats.append(row)
    g = c["grid"]
    out = {**meta[name], "body": body["name"], "radius_km": body["radius_km"], "rot_deg_s": body["rotation_rate_deg_s"],
           "times": times, "shown": c["tracks"]["shown"], "total": c["total_satellites"], "sats": sats,
           "constellations": [{"name": k["name"], "satellites": k["satellites"], "mean_visible": r(k["mean_visible"], 2),
                               "shells": [{kk: (r(vv, 3) if isinstance(vv, float) else vv) for kk, vv in sh.items()} for sh in k["shells"]],
                               "source": k.get("source")} for k in c["constellations"]],
           "global": {k: (r(v, 3) if not isinstance(v, dict) else {kk: r(vv, 3) for kk, vv in v.items()}) for k, v in c["global"].items()},
           "inputs": c["inputs"], "work": c.get("work"), "label": c["label"],
           "grid": {"lat": g["lat_deg"], "lon": g["lon_deg"], "availability": [[r(x, 1) for x in row] for row in g["availability_pct"]],
                    "pdop": [[r(x, 2) for x in row] for row in g["mean_pdop"]], "visible": [[r(x, 1) for x in row] for row in g["mean_visible"]]}}
    if name == "leo-pnt-mega-shell":
        # every one of the 5 000 satellites, rebuilt from the shells' Walker parameters, checked
        # against the engine's own 120 downsampled tracks at t = 0
        shells_t = t["constellation"][0]["shell"]
        shells_r = c["constellations"][0]["shells"]
        pts, walkers = [], []
        for st, sr in zip(shells_t, shells_r):
            raan0 = st.get("raan0_deg", 0.0)
            pts.extend(walker(sr, raan0))
            walkers.append({"planes": sr["planes"], "total": sr["total"], "raan0": raan0, "raan_spacing": sr["raan_spacing_deg"],
                            "in_plane": sr["in_plane_spacing_deg"], "phase_offset": sr["phase_offset_deg"],
                            "inc": sr["inclination_deg"], "alt_km": sr["altitude_km"], "period_min": r(sr["period_min"], 4)})
        worst = 0.0
        for s in c["tracks"]["satellites"]:
            v0 = unit(s["lat_deg"][0], s["lon_deg"][0])
            worst = max(worst, min(angle(v0, p) for p in pts))
        out["walker"] = walkers
        out["walker_residual_deg"] = round(math.degrees(worst), 4)
        assert len(pts) == c["total_satellites"] and out["walker_residual_deg"] < 0.1, out["walker_residual_deg"]
        print(f"  mega shell: {len(pts)} satellites rebuilt, worst residual against the engine tracks {out['walker_residual_deg']} deg")
    return out


# ------------------------------------------------------------------ solar system and body-pnt
def solar():
    s = res["solar-system-tour"]
    au = 1.495978707e11
    bodies = []
    for b in s["bodies"]:
        tr = thin(b["track_m"], 91)
        ol = b.get("observer_link") or {}
        bodies.append({"name": b["name"], "class": b["class"], "parent": b["parent"], "label": b["label"], "naif": b["naif_id"],
                       "pos_au": [r(x / au, 6) for x in b["position_m"]], "rel_km": [r(x / 1e3, 1) for x in b["parent_relative_position_m"]],
                       "frame": b["track_frame"], "track_km" if b["track_frame"] == "parent-centred" else "track_au":
                           [[r(x / 1e3, 0) for x in p] for p in tr] if b["track_frame"] == "parent-centred" else [[r(x / au, 5) for x in p] for p in tr],
                       "r_km": r(b["radius_mean_m"] / 1e3, 1), "r_eq_km": r(b["radius_equatorial_m"] / 1e3, 1), "gm": b["gm_m3_s2"], "j2": b["j2"],
                       "rot_h": r(b["sidereal_rotation_period_h"], 4), "pole_ra": b["pole_ra_deg"], "pole_dec": b["pole_dec_deg"],
                       "pm_deg": r(b["prime_meridian_deg"], 3), "dist_au": r(b["heliocentric_distance_au"], 5),
                       "period_d": r(b["orbital_period_d"], 3), "ecl_lon": r(b["ecliptic_longitude_deg"], 3), "ecl_lat": r(b["ecliptic_latitude_deg"], 3),
                       "method": b["method"], "error": b.get("nominal_error"),
                       "light_s": r(ol.get("one_way_light_time_s"), 2), "range_km": r((ol.get("geometric_distance_m") or 0) / 1e3, 0) if ol else None,
                       "sun_sep": r(ol.get("sun_separation_deg"), 2), "shapiro_s": ol.get("shapiro_delay_s")})
    links = [{"from": l["from"], "to": l["to"], "light_s": r(l["one_way_light_time_s"], 2), "two_way_s": r(l["two_way_light_time_s"], 2),
              "dist_km": r(l["geometric_distance_m"] / 1e3, 0), "sun_sep": r(l["sun_separation_deg"], 2)} for l in s["links"]]
    return {**meta["solar-system-tour"], "epoch": s["epoch"], "frame": s["frame"], "label": s["label"], "table": s["standish_table"],
            "observer": s["observer"], "bodies": bodies, "links": links}


def body_pnt(name):
    b = res[name]
    ep = b["epochs"]
    return {**meta[name], "body": b["body"]["name"], "user": toml[name].get("user", {}), "relays": b["n_relays"],
            "relay_alt_km": r(b["relay_altitude_m"] / 1e3, 1), "relay_period_h": r(b["relay_period_s"] / 3600, 3),
            "light_s": r(b["earth_to_body"]["one_way_light_time_s"], 1), "fom": {k: r(v, 4) for k, v in b["fom"].items()},
            "t": [e["t_s"] for e in ep], "n_vis": [e["n_relays_visible"] for e in ep], "earth": [1 if e["earth_visible"] else 0 for e in ep],
            "sigma_r": [r(e["formal_sigma_relays_m"], 3) for e in ep], "sigma_e": [r(e["formal_sigma_with_earth_m"], 3) for e in ep],
            "label": b["label"]}


# ------------------------------------------------------------------ campaigns
def campaigns():
    c = res["campaign-jam-spoof-holdover-integrity"]["timeline"]
    chain = {**meta["campaign-jam-spoof-holdover-integrity"], "title": res["campaign-jam-spoof-holdover-integrity"]["title"],
             "step_s": c["step_s"], "duration_s": c["duration_s"], "t": c["t_s"],
             "phases": [{"name": p["name"], "t0": p["t0_s"], "t1": p["t1_s"], "ended_by": p["ended_by"], "carried": p["carried"],
                         "kinds": [x["kind"] for x in p["runs"]]} for p in c["phases"]],
             "events": c["events"], "runs_total": res["campaign-jam-spoof-holdover-integrity"]["reproducibility"]["runs_total"],
             "ch": {k: {"unit": v.get("unit"), "v": [r(x, 3) for x in v["values"]]} for k, v in c["channels"].items()}}
    sw = res["campaign-sweep-jammer-power"]["sweep"]
    ax = sw["axes"]["jammer_power"]
    sweep = {**meta["campaign-sweep-jammer-power"], "title": res["campaign-sweep-jammer-power"]["title"], "key": ax["key"], "unit": ax["unit"],
             "x": ax["values"], "scenario_kind": sw["scenario_kind"],
             "m": {k: [r(n["metrics"][k], 3) for n in sw["nodes"]] for k in sw["metrics"]}, "units": {k: v["unit"] for k, v in sw["metrics"].items()}}
    mc = res["campaign-monte-carlo-clock-holdover"]["monte_carlo"]
    fe = mc["metrics"]["final_error"]
    monte = {**meta["campaign-monte-carlo-clock-holdover"], "title": res["campaign-monte-carlo-clock-holdover"]["title"], "runs": mc["runs"],
             "scenario_kind": mc["scenario_kind"], "base_seed": mc["base_seed"], "path": fe["path"], "unit": fe["unit"],
             "stats": {k: r(fe[k], 3) for k in ("mean", "std", "p05", "p50", "p95", "ci95_low", "ci95_high")},
             "samples": [r(x, 3) for x in fe["samples"]],
             "rms": {k: r(v, 3) for k, v in mc["metrics"]["timing_rms"].items() if k not in ("samples", "path", "unit")}}
    co = res["campaign-shared-jammer-sea-road"]["compose"]
    compose = {**meta["campaign-shared-jammer-sea-road"], "title": res["campaign-shared-jammer-sea-road"]["title"],
               "shared": {k: v for k, v in co["shared"].items()},
               "members": [{"name": k, "kind": v["kind"], "metrics": {kk: r(vv, 3) for kk, vv in v["metrics"].items()}} for k, v in co["members"].items()],
               "units": co["metric_units"]}
    return {"chain": chain, "sweep": sweep, "monte": monte, "compose": compose}


data = {"engine": ENG, "spectrum": {**meta["l-band-waterfall-jamming"], **spectrum()}, "six": six(), "ic": ic_cards(),
        "constellations": [constellation(n) for n in ("constellation-multi-gnss-coverage", "leo-pnt-mega-shell", "lunar-relay-constellation")],
        "solar": solar(), "body_pnt": {"Europa": body_pnt("europa-surface-pnt"), "Mars": body_pnt("mars-orbit-pnt")},
        "campaigns": campaigns()}
if os.path.exists(ATLAS):
    data["imagery"] = json.load(open(ATLAS))
    missing = {b["name"] for b in data["solar"]["bodies"]} - set(data["imagery"]["bodies"])
    if missing:
        sys.exit(f"no imagery for {sorted(missing)}: run src/tools/fetch_body_imagery.py")
else:
    sys.exit("src/tools/bodies-atlas.json missing: run src/tools/fetch_body_imagery.py first")
p = os.path.join(OUT, "capabilities.json")
json.dump(data, open(p, "w"), ensure_ascii=False, separators=(",", ":"))
print(f"wrote {os.path.relpath(p, SITE)} ({os.path.getsize(p) / 1e3:.0f} kB), engine v{ENG['version']} {ENG['commit']} ({ENG['tree']})")
