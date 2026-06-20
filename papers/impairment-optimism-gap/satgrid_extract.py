#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Extract SatGrid GNSS-SDR tracking-dump features into a tidy CSV (multi-scenario).

SatGrid ships per-channel GNSS-SDR Tracking_dump files (MATLAB v7.3 / HDF5) under a
``Genuine/`` baseline and ``Counterfeit/`` recordings. Scenario layouts vary: some have
a spoofer-amplification sweep (``Counterfeit/<level>/mat/*.mat``, levels like 0..100 or
30..40), others ship a single counterfeit condition (``Counterfeit/mat/*.mat``). File
names differ across scenarios too, but every ``.mat`` carries the same tracking
variables, and the PRN is read from inside the file rather than its name.

This flattens one or more scenarios to a single tidy CSV the Rust ``realdata::satgrid``
adapter consumes (the analogue of running an SDR before the SQM adapter, kept in Python
so the published crate needs no HDF5 dependency).

Usage:
    python3 satgrid_extract.py <out.csv> <scenario_dir> [<scenario_dir> ...]

The first 10% of each series is dropped as tracking pull-in transient and each channel
is subsampled to ~2000 epochs.
"""
import csv
import glob
import os
import sys

import h5py
import numpy as np

COLS = ["abs_E", "abs_L", "CN0_SNV_dB_Hz", "carrier_lock_test", "Prompt_I", "Prompt_Q", "PRN"]
TARGET_EPOCHS = 2000


def read_channel(path):
    """Return the needed series from one tracking .mat (HDF5), or None."""
    try:
        with h5py.File(path, "r") as f:
            if not all(c in f for c in COLS):
                return None
            return {c: np.array(f[c]).ravel() for c in COLS}
    except OSError:
        return None


def emit(writer, scenario, source, level, series):
    n = len(series["PRN"])
    if n == 0:
        return 0
    start = int(n * 0.10)
    stride = max(1, (n - start) // TARGET_EPOCHS)
    prn_mode = int(np.median(series["PRN"][start:])) if n > start else int(series["PRN"][0])
    wrote = 0
    for i in range(start, n, stride):
        cn0 = float(series["CN0_SNV_dB_Hz"][i])
        ae = float(series["abs_E"][i])
        al = float(series["abs_L"][i])
        lock = float(series["carrier_lock_test"][i])
        pi = float(series["Prompt_I"][i])
        pq = float(series["Prompt_Q"][i])
        if not all(np.isfinite(v) for v in (cn0, ae, al, lock, pi, pq)):
            continue
        writer.writerow([scenario, source, level, prn_mode, f"{cn0:.4f}",
                         f"{ae:.6g}", f"{al:.6g}", f"{lock:.6f}", f"{pi:.6g}", f"{pq:.6g}"])
        wrote += 1
    return wrote


def counterfeit_levels(root):
    """Yield (level, mat_dir) for a scenario's counterfeit recordings.

    A flat ``Counterfeit/mat`` (no sweep) yields a single level ``cf``; otherwise each
    numeric ``Counterfeit/<level>/mat`` subdir yields that level.
    """
    cdir = os.path.join(root, "Counterfeit")
    flat = os.path.join(cdir, "mat")
    if os.path.isdir(flat):
        yield "cf", flat
        return
    for entry in sorted(os.listdir(cdir)) if os.path.isdir(cdir) else []:
        sub = os.path.join(cdir, entry, "mat")
        if os.path.isdir(sub):
            yield entry, sub


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)
    out = sys.argv[1]
    scenario_dirs = sys.argv[2:]
    total = 0
    with open(out, "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["scenario", "source", "level", "prn", "cn0", "abs_e", "abs_l",
                    "lock", "prompt_i", "prompt_q"])
        for root in scenario_dirs:
            scenario = os.path.basename(os.path.normpath(root))
            for p in sorted(glob.glob(os.path.join(root, "Genuine", "mat", "*.mat"))):
                s = read_channel(p)
                if s is not None:
                    total += emit(w, scenario, "genuine", "na", s)
            for level, mat_dir in counterfeit_levels(root):
                for p in sorted(glob.glob(os.path.join(mat_dir, "*.mat"))):
                    s = read_channel(p)
                    if s is not None:
                        total += emit(w, scenario, "counterfeit", level, s)
            print(f"  {scenario}: done")
    print(f"wrote {total} rows -> {out}")


if __name__ == "__main__":
    main()
