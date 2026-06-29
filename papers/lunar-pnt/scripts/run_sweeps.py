#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Drive the PUBLIC kshana binary over the lunar-joint-OD configs and aggregate the
observability sweeps for the paper. No private engine logic lives here: every number
comes from the open `kshana <scenario.toml>` artifact JSON.

Experiments
-----------
  headline      : default config (seed 42) -> with/without-VLBI contrast.
  sigma_vlbi    : log grid of VLBI delay sigma x SWEEP_SEEDS -> median/IQR station error
                  (the information-not-count control: fixed obs count, varying VLBI info).
  n_earth       : number of Earth baselines x SWEEP_SEEDS -> median station error + factor.
  seed_mc       : default config x MC_SEEDS -> distribution of the improvement factor.
  cross_suite   : lunar-time-offset / lunar-vlbi / lunar-differential-pnt default artifacts.

Reproducibility: fix the binary (a pinned kshana release), this file, and the seed lists.
Output: data/sweeps.json (aggregates) consumed by make_figures.py.

Usage:  KSHANA_BIN=/path/to/kshana python3 run_sweeps.py
        (defaults to the sibling kshana release build if KSHANA_BIN is unset)
"""
from __future__ import annotations

import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
PAPER = HERE.parent
DATA = PAPER / "data"
DATA.mkdir(parents=True, exist_ok=True)

# Seed lists are fixed -> the whole study is deterministic given a pinned binary.
SWEEP_SEEDS = list(range(1, 33))      # 32 seeds for median/IQR sweeps
MC_SEEDS = list(range(1, 201))        # 200 seeds for the improvement-factor distribution
SIGMA_VLBI_GRID = [1e-12, 3e-12, 1e-11, 3e-11, 1e-10, 3e-10,
                   1e-9, 3e-9, 1e-8, 3e-8, 1e-7]   # delay sigma (s); default is 1e-11 (~3 mm)
N_EARTH_GRID = [1, 2, 3, 4, 6, 8, 10]


def find_binary() -> str:
    env = os.environ.get("KSHANA_BIN")
    if env and Path(env).exists():
        return env
    candidates = [
        PAPER / ".." / ".." / ".." / "kshana" / "target" / "release" / "kshana",
        PAPER / ".." / ".." / ".." / "kshana" / "target" / "debug" / "kshana",
    ]
    for c in candidates:
        c = c.resolve()
        if c.exists():
            return str(c)
    found = shutil.which("kshana")
    if found:
        return found
    sys.exit("error: kshana binary not found; set KSHANA_BIN=/path/to/kshana")


BIN = find_binary()


def run_joint(workdir: Path, **fields) -> dict:
    """Run one lunar-joint-od-clock scenario and return its result JSON."""
    toml = ['kind = "lunar-joint-od-clock"']
    for k, v in fields.items():
        if isinstance(v, bool):  # must precede int: TOML booleans are lowercase
            toml.append(f"{k} = {str(v).lower()}")
        elif isinstance(v, float):
            toml.append(f"{k} = {v:.16g}")
        else:
            toml.append(f"{k} = {v}")
    stem = "lc_" + "_".join(f"{k}{v}" for k, v in fields.items())
    stem = stem.replace(".", "p").replace("-", "m").replace("+", "")[:120]
    path = workdir / f"{stem}.toml"
    path.write_text("\n".join(toml) + "\n")
    subprocess.run([BIN, str(path)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return json.loads((workdir / f"{stem}.result.json").read_text())


def run_named(workdir: Path, kind: str) -> dict:
    """Run a default scenario of `kind` and return its result JSON."""
    path = workdir / f"{kind}.toml"
    path.write_text(f'kind = "{kind}"\n')
    subprocess.run([BIN, str(path)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return json.loads((workdir / f"{kind}.result.json").read_text())


def quartiles(xs):
    xs = sorted(xs)
    n = len(xs)
    med = statistics.median(xs)
    q1 = statistics.median(xs[: n // 2])
    q3 = statistics.median(xs[(n + 1) // 2:])
    return {"median": med, "q1": q1, "q3": q3, "min": xs[0], "max": xs[-1], "n": n}


def main():
    # Record only the binary basename (not the absolute path) for portable, clean provenance.
    out = {"binary": Path(BIN).name, "seeds_sweep": SWEEP_SEEDS, "seeds_mc": MC_SEEDS}
    with tempfile.TemporaryDirectory() as td:
        wd = Path(td)

        # 1. Headline (default config, seed 42).
        print("headline ...", flush=True)
        out["headline"] = run_joint(wd, seed=42)

        # 2. sigma_vlbi sweep (information-not-count control).
        print("sigma_vlbi sweep ...", flush=True)
        sv_rows = []
        for sv in SIGMA_VLBI_GRID:
            with_errs, range_errs = [], []
            for s in SWEEP_SEEDS:
                r = run_joint(wd, seed=s, sigma_vlbi_s=sv)
                with_errs.append(r["with_vlbi"]["station_pos_err_m"])
                range_errs.append(r["without_vlbi"]["station_pos_err_m"])
            sv_rows.append({"sigma_vlbi_s": sv,
                            "with_vlbi": quartiles(with_errs),
                            "range_only": quartiles(range_errs)})
        out["sigma_vlbi_sweep"] = sv_rows

        # 3. n_earth sweep (number of Earth baselines) on a NON-STARVED 6-satellite
        #    network, so the datum-defect ladder (3 -> 1 -> 0) and the CRLB design
        #    curve are the true datum defect, not an observation-count artefact.
        print("n_earth sweep ...", flush=True)
        ne_rows = []
        for ne in N_EARTH_GRID:
            errs, factors = [], []
            crlb_m, defect = None, None
            # 200 seeds so the estimator-RMS markers are stable enough to read the
            # efficiency (RMS/CRLB) off the figure; CRLB/defect are geometry-only.
            for s in MC_SEEDS:
                r = run_joint(wd, seed=s, n_earth=ne, n_sat=6)
                errs.append(r["with_vlbi"]["station_pos_err_m"])
                factors.append(r["station_observability_improvement_factor"])
                # Fisher-information observability is geometry-only (seed-independent).
                crlb_m = r["observability"]["station_pos_crlb_m"]
                defect = r["observability"]["defect"]
            rms = (sum(e * e for e in errs) / len(errs)) ** 0.5
            ne_rows.append({"n_earth": ne,
                            "with_vlbi": quartiles(errs),
                            "with_vlbi_rms_m": rms,
                            "crlb_m": crlb_m,
                            "defect": defect,
                            "improvement_factor": quartiles(factors)})
        out["n_earth_sweep"] = ne_rows

        # 4. seed Monte-Carlo at default config (improvement-factor distribution).
        print("seed Monte-Carlo ...", flush=True)
        factors, with_errs = [], []
        for s in MC_SEEDS:
            r = run_joint(wd, seed=s)
            factors.append(r["station_observability_improvement_factor"])
            with_errs.append(r["with_vlbi"]["station_pos_err_m"])
        crlb_default = out["headline"]["observability"]["station_pos_crlb_m"]
        rms_default = (sum(e * e for e in with_errs) / len(with_errs)) ** 0.5
        out["seed_mc"] = {"improvement_factor": quartiles(factors),
                          "with_vlbi_station_err_m": quartiles(with_errs),
                          "with_vlbi_station_rms_m": rms_default,
                          "crlb_m": crlb_default,
                          "efficiency_rms_over_crlb":
                              (rms_default / crlb_default) if crlb_default else None,
                          "factors": factors}

        # 4b. Representative ELFO (Moonlight/LCNS design family) — geometry-generality check:
        #     the datum-defect structure is not an artefact of the illustrative circular placement.
        print("ELFO geometry ...", flush=True)
        ELFO = dict(orbit_radius_km=9750.7, orbit_ecc=0.6383,
                    orbit_inc_deg=57.7, orbit_argp_deg=90.0)
        # Datum-defect ladder + CRLB curve on a non-starved 6-satellite, 3-plane ELFO
        # (defect/CRLB are geometry-only, so one seed suffices).
        elfo_curve = []
        for ne in N_EARTH_GRID:
            r = run_joint(wd, seed=1, n_sat=6, orbit_planes=3, n_earth=ne, **ELFO)
            elfo_curve.append({"n_earth": ne,
                               "defect": r["observability"]["defect"],
                               "crlb_m": r["observability"]["station_pos_crlb_m"]})
        # Estimator efficiency at the 6-station ELFO (MC_SEEDS).
        errs = []
        for s in MC_SEEDS:
            r = run_joint(wd, seed=s, n_sat=6, orbit_planes=3, n_earth=6, **ELFO)
            errs.append(r["with_vlbi"]["station_pos_err_m"])
        rms6 = (sum(e * e for e in errs) / len(errs)) ** 0.5
        crlb6 = next(c["crlb_m"] for c in elfo_curve if c["n_earth"] == 6)
        # Restore-vs-sharpen across constellation richness (n_earth = 6, geometry-only CRLB).
        elfo_rs = []
        for ns, pl in [(3, 1), (3, 3), (6, 1), (6, 3)]:
            r = run_joint(wd, seed=1, n_sat=ns, orbit_planes=pl, n_earth=6, **ELFO)
            elfo_rs.append({"n_sat": ns, "planes": pl,
                            "with_crlb_m": r["observability"]["station_pos_crlb_m"],
                            "with_defect": r["observability"]["defect"],
                            "without_crlb_m": r["observability_without_vlbi"]["station_pos_crlb_m"],
                            "without_defect": r["observability_without_vlbi"]["defect"]})
        out["elfo"] = {"params": ELFO, "curve": elfo_curve, "restore_sharpen": elfo_rs,
                       "rms6_m": rms6, "crlb6_m": crlb6,
                       "efficiency_rms_over_crlb": (rms6 / crlb6) if crlb6 else None}

        # 5. Cross-suite sanity (default artifacts).
        print("cross-suite ...", flush=True)
        out["cross_suite"] = {
            "lunar_time": run_named(wd, "lunar-time-offset"),
            "lunar_vlbi": run_named(wd, "lunar-vlbi"),
            "lunar_dpnt": run_named(wd, "lunar-differential-pnt"),
        }

    (DATA / "sweeps.json").write_text(json.dumps(out, indent=2))
    hl = out["headline"]
    mc = out["seed_mc"]["improvement_factor"]
    print(f"\nwrote {DATA / 'sweeps.json'}")
    print(f"headline: with-VLBI {hl['with_vlbi']['station_pos_err_m']:.2f} m vs "
          f"range-only {hl['without_vlbi']['station_pos_err_m']:.1f} m "
          f"({hl['station_observability_improvement_factor']:.0f}x)")
    print(f"seed MC improvement factor: median {mc['median']:.0f}x "
          f"(IQR {mc['q1']:.0f}-{mc['q3']:.0f}, n={mc['n']})")


if __name__ == "__main__":
    main()
