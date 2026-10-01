#!/usr/bin/env python3
"""Record the engine runs behind Home's L-band console (the waterfall section) and write its data.

Runs the engine binary of the site's one engine checkout (src/data/sources.json: repo; the one
record_runs.py uses) on two bundled scenario files, each from a fresh scratch
directory, so the worktree is never written:

  l-band-waterfall-jamming              the spectrum kind: 5 bands, 3 jammers, 60 one-second rows
  campaign-spectrum-holdover-integrity  a campaign whose phases are driven by the spectrum run

and keeps, for each run, the published copies the section links to:

  assets/waterfall/<file>/<file>.toml.txt   the exact TOML that ran (a byte copy of the bundled file)
  assets/waterfall/<file>/result.json       the engine's result, unmodified
  assets/waterfall/<file>/report.html       the engine's own reproducible HTML report, unmodified

plus the compact extract the page draws:

  assets/waterfall/home-waterfall.json      engine version and commit, each scenario's SHA-256 and
                                            seed, the waterfall rows (only the columns the page
                                            shows, as tenths of a dB above the thermal noise floor),
                                            the per-band C/N0, J/S and lock status per row, the
                                            jammers, and the campaign's phases and headline figures

Every number is read from result.json or the scenario TOML; nothing is typed. E6 is not in the
spectrum kind's model (see its `not_modelled`), so no band outside the result's own `bands` list
is ever drawn. build.py never runs the engine; it publishes assets/ as it finds it.
Re-run after an engine change:  /opt/homebrew/bin/python3 src/tools/gen_waterfall.py
"""
import hashlib, json, os, shutil, subprocess, sys, tempfile, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
from _sources import engine_tree  # noqa: E402  (src/data/sources.json: the one engine checkout)
WT = engine_tree()
ENGINE = os.path.join(WT, "target", "release", "kshana")
OUT = os.path.join(SITE, "assets", "waterfall")
SPEC, CAMP = "l-band-waterfall-jamming", "campaign-spectrum-holdover-integrity"
# Display names for the result's band ids (words, not figures). A band id missing here fails the run.
BAND_NAMES = {"gps-l1ca": "GPS L1 C/A", "galileo-e1": "Galileo E1", "gps-l2c": "GPS L2C", "gps-l5": "GPS L5", "galileo-e5a": "Galileo E5a"}
MARGIN_HZ = 6e6  # frequency shown either side of a band group's widest receiver or jammer half-width


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def run(name):
    raw = open(os.path.join(WT, "scenarios", name + ".toml"), "rb").read()
    tmp = tempfile.mkdtemp(prefix="kshana-wf-")
    try:
        # The engine prints the command it was given into its report ("Command to reproduce").
        # Give it the scenario's place in the repository, scenarios/<file>.toml, from a scratch
        # directory laid out the same way, so a published report names a command a reader can
        # run and never a path on the machine that recorded it.
        rel = os.path.join("scenarios", name + ".toml")
        os.makedirs(os.path.join(tmp, "scenarios"))
        open(os.path.join(tmp, rel), "wb").write(raw)
        line = sh(ENGINE, rel, cwd=tmp).splitlines()[0]
        d = os.path.join(OUT, name)
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, name + ".toml.txt"), "wb").write(raw)
        shutil.copyfile(os.path.join(tmp, "scenarios", name + ".result.json"), os.path.join(d, "result.json"))
        shutil.copyfile(os.path.join(tmp, "scenarios", name + ".report.html"), os.path.join(d, "report.html"))
        res = json.load(open(os.path.join(d, "result.json")))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return raw, res, line


def r(x, n=2):
    return None if x is None else round(x, n)


def segments(res):
    """One frequency window per group of bands sharing a carrier: the widest receiver or jammer
    half-width around it plus a margin, clipped to the grid."""
    w = res["waterfall"]
    groups = {}
    for b in res["bands"]:
        groups.setdefault(round(b["centre_hz"]), []).append(b["rx_bandwidth_hz"] / 2)
    for j in res["jammers"]:
        for fc in groups:
            if abs(j["centre_hz"] - fc) < 1e6:
                groups[fc].append((j.get("bandwidth_hz") or 0) / 2)
    segs = []
    for fc in sorted(groups):
        half = max(groups[fc]) + MARGIN_HZ
        lo, hi = max(w["f_min_hz"], fc - half), min(w["f_max_hz"], fc + half)
        cols = [i for i, f in enumerate(w["freq_hz"]) if lo - w["bin_width_hz"] / 2 <= f <= hi + w["bin_width_hz"] / 2]
        edge = lambda i, sgn: w["freq_hz"][i] + sgn * w["bin_width_hz"] / 2
        segs.append({"fc": fc / 1e6, "lo": r(max(w["f_min_hz"], edge(cols[0], -1)) / 1e6, 3),
                     "hi": r(min(w["f_max_hz"], edge(cols[-1], 1)) / 1e6, 3), "c0": cols[0], "c1": cols[-1]})
    return segs


def main():
    if not os.access(ENGINE, os.X_OK):
        sys.exit(f"engine binary not found: {ENGINE}")
    ver = sh(ENGINE, "--version").split()[-1]
    commit = sh("git", "-C", WT, "rev-parse", "HEAD")[:7]
    dirty = bool(sh("git", "-C", WT, "status", "--porcelain", "--untracked-files=no"))
    os.makedirs(OUT, exist_ok=True)

    raw_s, s, line_s = run(SPEC)
    raw_c, c, line_c = run(CAMP)
    assert s["kind"] == "spectrum" and c["kind"] == "campaign"
    assert s["engine_version"] == ver and c["engine_version"] == ver
    toml_s, toml_c = tomllib.loads(raw_s.decode()), tomllib.loads(raw_c.decode())

    w, tl = s["waterfall"], s["timeline"]
    floor = w["noise_floor_dbw_per_hz"]
    segs = segments(s)
    keep = [i for g in segs for i in range(g["c0"], g["c1"] + 1)]
    thr = s["receiver"]["tracking_threshold_dbhz"]
    bands = []
    for meta, row in zip(s["bands"], tl["bands"]):
        assert meta["name"] == row["name"]
        bands.append({"id": meta["name"], "name": BAND_NAMES[meta["name"]], "mod": meta["modulation"],
                      "fc": meta["centre_hz"] / 1e6, "bw": r(meta["rx_bandwidth_hz"] / 1e6, 3), "nominal": r(meta["nominal_cn0_dbhz"], 1),
                      "cn0": [r(x, 1) for x in row["cn0_effective_dbhz"]], "js": [r(x, 1) for x in row["js_db"]],
                      "st": "".join("L" if x == "LOCKED" else "X" for x in row["status"]),
                      "first_loss": row["first_loss_t_s"], "min": r(row["min_cn0_dbhz"], 1), "worst_js": r(row["worst_js_db"], 1)})
    assert all(set(b["st"]) <= {"L", "X"} for b in bands) and len(bands) == len(s["bands"])

    # Campaign: its phases, and the figures each one's card states (read from the chained channels).
    ct = c["timeline"]
    ch = ct["channels"]
    t = ct["t_s"]
    ph = []
    for p in ct["phases"]:
        idx = [i for i, x in enumerate(t) if p["t0_s"] <= x < p["t1_s"]] or [len(t) - 1]
        te = [abs(ch["time_error_ns"]["values"][i]) for i in idx if ch["time_error_ns"]["values"][i] is not None]
        pl = [ch["protection_level_m"]["values"][i] for i in idx if ch["protection_level_m"]["values"][i] is not None]
        e1 = [ch["cn0_e1_dbhz"]["values"][i] for i in idx]
        ca = [ch["cn0_l1ca_dbhz"]["values"][i] for i in idx]
        ph.append({"name": p["name"], "t0": p["t0_s"], "t1": p["t1_s"], "ended_by": p["ended_by"],
                   "kinds": [x["kind"] for x in p["runs"]], "carried": list((p.get("carried") or {}).keys()),
                   "te_max_ns": r(max(te), 2) if te else None, "pl_min_m": r(min(pl), 1) if pl else None, "pl_max_m": r(max(pl), 1) if pl else None,
                   "e1_dbhz": r(min(e1), 1), "ca_dbhz": r(min(ca), 1),
                   "alarm_s": sum(ct["step_s"] for i in idx if ch["alarm"]["values"][i] == 1.0)})
    guard = next(x for x in ch["guard_ns"]["values"] if x is not None)
    al = next(x for x in ch["alert_limit_m"]["values"] if x is not None)

    doc = {
        "engine": {"version": ver, "commit": commit, "tree": sh("git", "-C", WT, "rev-parse", "--abbrev-ref", "HEAD"), "dirty": dirty},
        "spectrum": {"file": SPEC, "sha256": hashlib.sha256(raw_s).hexdigest(), "seed": toml_s.get("seed"), "summary": line_s,
                     "published": f"assets/waterfall/{SPEC}/", "label": s["label"], "not_modelled": s["not_modelled"]},
        "campaign": {"file": CAMP, "sha256": hashlib.sha256(raw_c).hexdigest(), "seed": toml_c.get("seed"), "summary": line_c,
                     "published": f"assets/waterfall/{CAMP}/", "title": c["title"], "label": c["label"],
                     "duration_s": ct["duration_s"], "phases": ph, "guard_ns": guard, "al_m": al},
        "step_s": s["step_s"], "duration_s": s["duration_s"], "t": tl["t_s"],
        "thr": thr, "floor": r(floor, 2), "peak": r(w["peak_dbw_per_hz"], 2), "bin_mhz": w["bin_width_hz"] / 1e6,
        "f": [r(w["freq_hz"][i] / 1e6, 3) for i in keep],
        "segs": [{"fc": g["fc"], "lo": g["lo"], "hi": g["hi"], "n": g["c1"] - g["c0"] + 1} for g in segs],
        # dB above the thermal floor, in tenths, row by row (time down), only the kept columns
        "psd": [[max(0, round((row[i] - floor) * 10)) for i in keep] for row in w["psd_dbw_per_hz"]],
        "bands": bands,
        "jammers": [{"name": j["name"], "waveform": j["waveform"], "fc": j["centre_hz"] / 1e6, "bw": r((j.get("bandwidth_hz") or 0) / 1e6, 3),
                     "on": j["on_s"], "off": j.get("off_s"), "rx_dbw": r(j["received_power_dbw"], 1)} for j in s["jammers"]],
    }
    json.dump(doc, open(os.path.join(OUT, "home-waterfall.json"), "w"), ensure_ascii=False, separators=(",", ":"))
    print(f"engine {ver} @ {commit}{' (dirty)' if dirty else ''}")
    print(f"  {line_s[:120]}")
    print(f"  {line_c[:120]}")
    print(f"  {len(doc['t'])} rows x {len(doc['f'])} columns in {len(segs)} windows · "
          f"{os.path.getsize(os.path.join(OUT, 'home-waterfall.json')):,} bytes")


if __name__ == "__main__":
    main()
