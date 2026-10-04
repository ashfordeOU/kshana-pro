#!/usr/bin/env python3
"""Record the engine runs behind the Home page's campaign and constellation visuals.

Runs the engine binary of the site's one engine checkout (src/data/sources.json: repo;
target/release/kshana there) on bundled scenario files, each from a
fresh scratch directory (the engine writes result.json, chart.svg and report.html next to the
TOML it reads, so the worktree is never written), and keeps:

  src/data/runs/<file>/scenario.toml   the exact TOML that ran (a byte copy of the bundled file)
  src/data/runs/<file>/result.json     the engine's result, unmodified
  src/data/runs/<file>/report.html     the engine's own reproducible HTML report, unmodified
  src/data/runs/index.json             engine version, engine commit, scenario SHA-256, seed and
                                       the engine's one-line summary for every run
  src/data/runs/home.json              the extracts the Home page draws (below)

Extracts (every number is read from result.json or the scenario TOML; nothing is typed):
  campaign       the mission timeline's channels on its 10 s grid, the phases and their member
                 runs, the engine's own events, and a longer event list derived from the channels
                 (threshold crossings, tracking changes, phase hand-offs), each with its time
  constellation  for each of the 102 satellites its circular two-body orbit in the run's inertial
                 frame (plane normal, position at t = 0, radius, mean motion), recovered from the
                 engine's 25 ground-track samples and the body's rotation rate, and checked back
                 against every sample (the residual is recorded; the build refuses more than 0.2°)

build.py never runs the engine: it reads these files, the way it reads the Studio's recordings.
Re-run after an engine change:  /opt/homebrew/bin/python3 src/tools/record_runs.py
"""
import hashlib, json, math, os, re, shutil, subprocess, sys, tempfile, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from _sources import engine_tree  # noqa: E402  (src/data/sources.json: the one engine checkout)
WT = engine_tree()
ENGINE = os.path.join(WT, "target", "release", "kshana")
OUT = os.path.join(SRC, "data", "runs")
RUNS = {"campaign": "campaign-jam-spoof-holdover-integrity", "constellation": "constellation-multi-gnss-coverage"}


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def run(name):
    src_toml = os.path.join(WT, "scenarios", name + ".toml")
    raw = open(src_toml, "rb").read()
    tmp = tempfile.mkdtemp(prefix="kshana-run-")
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
        d = os.path.join(OUT, name)
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, "scenario.toml"), "wb").write(raw)
        shutil.copyfile(os.path.join(tmp, "scenarios", name + ".result.json"), os.path.join(d, "result.json"))
        shutil.copyfile(os.path.join(tmp, "scenarios", name + ".report.html"), os.path.join(d, "report.html"))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return raw, res, out.splitlines()[0]


def r(v, n=3):
    return None if v is None else round(v, n)


def hms(s):
    s = int(round(s))
    return f"T+{s // 3600:02d}:{s % 3600 // 60:02d}:{s % 60:02d}"


# ------------------------------------------------------------------------------ campaign
def campaign_extract(res, toml):
    tl = res["timeline"]
    t = tl["t_s"]
    ch = tl["channels"]
    V = {k: c["values"] for k, c in ch.items()}
    phases = []
    for i, p in enumerate(tl["phases"]):
        tp = toml["phases"][i]
        assert tp["name"] == p["name"], (tp["name"], p["name"])
        runs = []
        for tr, pr in zip(tp["runs"], p["runs"]):
            s = tr["scenario"]
            kind = s.get("kind", "clock")
            assert kind == pr["kind"], (kind, pr["kind"])
            runs.append({"kind": kind, "seed": s.get("seed"), "hash": (pr.get("scenario_hash") or pr["result_sha256"])[:12],
                         "jammer_dbw": (s.get("jammer") or {}).get("power_dbw"), "mask_deg": s.get("mask_deg")})
        phases.append({"name": p["name"], "t0": p["t0_s"], "t1": p["t1_s"], "ended_by": p["ended_by"],
                       "carried": {k: r(v) for k, v in (p.get("carried") or {}).items()}, "runs": runs})

    def ph(ts):
        return next((p for p in phases if p["t0"] <= ts < p["t1"]), phases[-1])

    def jammer(p):
        return next((x["jammer_dbw"] for x in p["runs"] if x["kind"] == "jamming"), None)

    def jam_km(p):  # receiver-to-jammer distance, from the phase's jamming run (WGS84)
        s = next((x for x in toml["phases"][phases.index(p)]["runs"] if x["scenario"].get("kind") == "jamming" and x["scenario"].get("jammer")), None)
        if not s:
            return None
        s = s["scenario"]
        a, f = 6378137.0, 1 / 298.257223563
        e2 = f * (2 - f)
        la, lo, h = math.radians(s["receiver"]["lat_deg"]), math.radians(s["receiver"]["lon_deg"]), s["receiver"].get("alt_m", 0.0)
        N = a / math.sqrt(1 - e2 * math.sin(la) ** 2)
        rx = [(N + h) * math.cos(la) * math.cos(lo), (N + h) * math.cos(la) * math.sin(lo), (N * (1 - e2) + h) * math.sin(la)]
        return math.dist(rx, s["jammer"]["position_ecef_m"]) / 1000

    def mask(p):
        return next((x["mask_deg"] for x in p["runs"] if x["kind"] == "jamming"), None)

    ev = []  # [t, level, message]; level in ok, info, warn, spoof, crit
    add = lambda ts, lv, m: ev.append([ts, lv, m])
    nruns = res["reproducibility"]["runs_total"]
    add(0.0, "ok", f"Campaign loaded · {len(phases)} phases · {nruns} member runs · seed {res['seed']}")
    for p in phases:
        i0 = t.index(p["t0"])
        trk, cn, pl, te = V["tracking"][i0], V["cn0_dbhz"][i0], V["protection_level_m"][i0], V["time_error_ns"][i0]
        name = p["name"]
        j = jammer(p)
        if name == "nominal":
            add(p["t0"] + 10, "info", f"{int(trk)} satellites tracking · mean C/N0 {cn:.1f} dB-Hz · PL {pl:.1f} m")
        elif name == "jamming":
            add(p["t0"], "warn", f"Jammer on at {j:g} dBW, {jam_km(p):.1f} km away · mean C/N0 {cn:.1f} dB-Hz")
            add(p["t0"] + 10, "warn", f"C/N0 {cn - V['cn0_floor_dbhz'][i0]:.1f} dB above the {V['cn0_floor_dbhz'][i0]:g} dB-Hz tracking floor · {int(trk)} still tracking")
        elif name == "spoofing":
            sp = next(x for x in toml["phases"][phases.index(p)]["runs"] if x["scenario"].get("kind") == "spoof")["scenario"]["attack"]
            add(p["t0"] + 10, "spoof", f"Time push {sp['rate_ns_per_s']:g} ns/s on the receiver clock")
        elif name == "holdover":
            add(p["t0"], "crit", f"Jammer to {j:g} dBW · {int(trk)} satellites tracking · GNSS lost")
            add(p["t0"] + 10, "info", f"Clock holdover from {p['carried'].get('time_error_ns', 0):.1f} ns (carried) · inertial coasting")
        elif name == "integrity-alarm":
            add(p["t0"], "warn", f"Jammer back to {j:g} dBW · reacquired above a {mask(p):g}° mask · {int(trk)} tracking")
        elif name == "recovery":
            add(p["t0"], "ok", f"GNSS re-synchronised · {int(trk)} tracking · time error {te:.1f} ns · PL {pl:.1f} m")
    for e in tl["events"]:
        add(e["t_s"], "spoof", e["label"][0].upper() + e["label"][1:])
    sp_p = next(p for p in phases if p["name"] == "spoofing")
    if sp_p["ended_by"].startswith("end_at"):
        i1 = max(i for i, ts in enumerate(t) if ts < sp_p["t1"])
        add(sp_p["t1"], "spoof", f"Spoofing excluded after {sp_p['t1'] - sp_p['t0']:g} s · offset {V['time_error_ns'][i1]:.1f} ns pulled in")
    # threshold crossings, read from the channels
    def first(pred, t0=0):
        return next((i for i, ts in enumerate(t) if ts >= t0 and pred(i)), None)
    i = first(lambda k: V["cn0_dbhz"][k] < V["cn0_floor_dbhz"][k])
    if i is not None:
        add(t[i], "crit", f"C/N0 {V['cn0_dbhz'][i]:.1f} dB-Hz, below the {V['cn0_floor_dbhz'][i]:g} dB-Hz floor")
    i = first(lambda k: V["time_error_ns"][k] > V["guard_ns"][k])
    if i is not None:
        add(t[i], "crit", f"Clock time error {V['time_error_ns'][i]:.1f} ns crosses the {V['guard_ns'][i]:g} ns guard")
    i = first(lambda k: V["position_error_m"][k] is not None and V["position_threshold_m"][k] is not None and V["position_error_m"][k] > V["position_threshold_m"][k])
    if i is not None:
        add(t[i], "warn", f"Inertial position error {V['position_error_m'][i]:.0f} m passes its {V['position_threshold_m'][i]:g} m threshold")
    hold = next(p for p in phases if p["name"] == "holdover")
    ih = [k for k, ts in enumerate(t) if hold["t0"] <= ts < hold["t1"] and V["position_error_m"][k] is not None]
    if ih:
        k = ih[-1]
        add(t[k], "warn", f"Inertial position error {V['position_error_m'][k] / 1000:.1f} km at the end of holdover")
    ia = next(p for p in phases if p["name"] == "integrity-alarm")
    i = first(lambda k: t[k] < ia["t1"] and V["protection_level_m"][k] is not None and V["alert_limit_m"][k] is not None and V["protection_level_m"][k] > V["alert_limit_m"][k], ia["t0"])
    if i is not None:
        add(t[i], "crit", f"INTEGRITY ALARM · PL {V['protection_level_m'][i]:.1f} m exceeds the {V['alert_limit_m'][i]:g} m alert limit")
    nnull = sum(1 for k, ts in enumerate(t) if ia["t0"] <= ts < ia["t1"] and V["protection_level_m"][k] is None)
    if nnull:
        k = first(lambda k: V["protection_level_m"][k] is None, ia["t0"])
        add(t[k], "crit", f"No protection level: too few satellites at {nnull} of {sum(1 for ts in t if ia['t0'] <= ts < ia['t1'])} epochs")
    ev.sort(key=lambda e: (e[0], ["ok", "info", "warn", "spoof", "crit"].index(e[1])))
    for e in ev:
        e.insert(1, hms(e[0]))
    keep = ["time_error_ns", "guard_ns", "cn0_dbhz", "cn0_floor_dbhz", "protection_level_m", "alert_limit_m", "alarm", "tracking", "position_error_m", "position_threshold_m"]
    return {"title": res["title"], "label": res["label"], "seed": res["seed"], "step_s": tl["step_s"], "duration_s": tl["duration_s"],
            "t": t, "ch": {k: [r(x, 2) for x in V[k]] for k in keep}, "units": {k: ch[k]["unit"] for k in keep},
            "labels": {k: ch[k]["label"] for k in keep}, "phases": phases, "events": ev, "engine_events": tl["events"],
            "receiver": next(x["scenario"]["receiver"] for x in toml["phases"][0]["runs"] if x["scenario"].get("kind") == "jamming")}


# ------------------------------------------------------------------------------ constellation
def unit(lat, lon):
    la, lo = math.radians(lat), math.radians(lon)
    return [math.cos(la) * math.cos(lo), math.cos(la) * math.sin(lo), math.sin(la)]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def norm(a):
    n = math.sqrt(dot(a, a))
    return [x / n for x in a]


def rot(v, k, th):  # Rodrigues: rotate v about unit axis k by th
    c, s = math.cos(th), math.sin(th)
    kv = cross(k, v)
    kd = dot(k, v)
    return [v[i] * c + kv[i] * s + k[i] * kd * (1 - c) for i in range(3)]


def constellation_extract(res):
    body = res["body"]
    w = math.radians(body["rotation_rate_deg_s"])
    mu, R = body["mu_km3_s2"], body["radius_km"]
    ts = res["tracks"]["times_s"]
    sats, worst = [], 0.0
    for s in res["tracks"]["satellites"]:
        # ground track (body-fixed) to the run's inertial frame: undo the body's rotation
        U = [unit(la, lo + math.degrees(w * t)) for la, lo, t in zip(s["lat_deg"], s["lon_deg"], ts)]
        h = norm(cross(U[0], U[1]))
        ang = sum(math.acos(max(-1, min(1, dot(U[i], U[i + 1])))) for i in range(len(U) - 1))
        n = ang / (ts[-1] - ts[0])
        a = (mu / (n * n)) ** (1 / 3)
        err = max(math.degrees(math.acos(max(-1, min(1, dot(rot(U[0], h, n * t), u))))) for t, u in zip(ts, U))
        worst = max(worst, err)
        sats.append({"c": s["constellation"], "id": s["id"], "h": [r(x, 6) for x in h], "u0": [r(x, 6) for x in U[0]], "a_km": r(a, 1), "n": n})
    assert worst < 0.2, f"circular-orbit reconstruction residual {worst:.3f} deg"
    g = res["global"]
    return {"body": body["name"], "radius_km": R, "rotation_rate_deg_s": body["rotation_rate_deg_s"], "duration_s": res["inputs"]["duration_s"],
            "mask_deg": res["inputs"]["mask_deg"], "residual_deg": round(worst, 4), "sats": sats,
            "constellations": [{"name": c["name"], "satellites": c["satellites"], "mean_visible": c["mean_visible"], "source": c["source"]} for c in res["constellations"]],
            "global": {"availability_pct": g["availability_pct"], "pdop_median": g["pdop"]["median"], "mean_visible": g["mean_visible"], "min_visible": g["min_visible"]},
            "total": res["total_satellites"]}


def main():
    if not os.access(ENGINE, os.X_OK):
        sys.exit(f"engine binary not found: {ENGINE}")
    ver = sh(ENGINE, "--version").split()[-1]
    commit = sh("git", "-C", WT, "rev-parse", "HEAD")[:7]
    dirty = bool(sh("git", "-C", WT, "status", "--porcelain", "--untracked-files=no"))
    os.makedirs(OUT, exist_ok=True)
    index = {"engine": {"version": ver, "commit": commit, "tree": sh("git", "-C", WT, "rev-parse", "--abbrev-ref", "HEAD"), "dirty": dirty, "kinds": len(json.loads(sh(ENGINE, "kinds", "--json")))}, "runs": {}}
    home = {}
    for key, name in RUNS.items():
        raw, res, line = run(name)
        toml = tomllib.loads(raw.decode("utf-8"))
        assert res.get("engine_version", ver) == ver  # constellation-design results carry no version field; the binary's is recorded
        index["runs"][name] = {"file": name + ".toml", "kind": res["kind"], "sha256": hashlib.sha256(raw).hexdigest(), "seed": toml.get("seed"),
                               "engine_version": ver, "summary": line, "published": f"assets/runs/{name}/"}
        home[key] = campaign_extract(res, toml) if key == "campaign" else constellation_extract(res)
        home[key]["file"] = name
    json.dump(index, open(os.path.join(OUT, "index.json"), "w"), indent=1)
    json.dump(home, open(os.path.join(OUT, "home.json"), "w"), separators=(",", ":"))
    print(f"engine {ver} @ {commit}{' (dirty)' if dirty else ''} · {index['engine']['kinds']} kinds")
    for n, v in index["runs"].items():
        print(f"  {n}: {v['summary'][:110]}")
    print(f"  constellation residual {home['constellation']['residual_deg']}° · {len(home['campaign']['events'])} campaign events")


if __name__ == "__main__":
    main()
