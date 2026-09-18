#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Generate the SatGrid graded-severity figure from the probe artifact.

Reads satgrid-probe-result.json (produced by `cargo run --example satgrid_probe`) and
plots each detector's genuine-vs-counterfeit AUC against the spoofer amplification
level, showing how in-distribution (level-0) separability collapses, and inverts past
0.5, as the spoofer power rises.

Usage: python3 make_satgrid_figure.py [satgrid-probe-result.json] [out_dir]
"""
import json
import os
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ART = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "satgrid-probe-result.json")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "figures")
os.makedirs(OUT, exist_ok=True)

with open(ART) as f:
    d = json.load(f)

# Schema v2: per_scenario.<name>.auc_by_detector_and_level. Plot the session with the
# widest amplification sweep (the clearest collapse-and-inversion curve).
if "per_scenario" in d:
    sessions = d["per_scenario"]
    name = max(sessions, key=lambda k: len(sessions[k]["levels"]))
    sess = sessions[name]
    levels = [int(x) for x in sess["levels"]]
    table = sess["auc_by_detector_and_level"]
else:  # schema v1 fallback
    levels = [int(x) for x in d["provenance"]["levels"]]
    table = d["auc_by_detector_and_level"]
labels = {"cn0": "C/N0", "sqm": "SQM (E-L)", "lock": "lock test", "qratio": "Q ratio"}
markers = {"cn0": "o", "sqm": "s", "lock": "^", "qratio": "D"}

fig, ax = plt.subplots(figsize=(3.4, 2.6), layout="constrained")
for det in sorted(table):
    ys = [table[det][str(lv)] for lv in levels]
    ax.plot(levels, ys, marker=markers.get(det, "o"), ms=4, lw=1.3,
            label=labels.get(det, det))
ax.axhline(0.5, color="0.5", ls="--", lw=0.8)
ax.text(2, 0.505, "chance", color="0.4", fontsize=7, va="bottom")
ax.set_xlabel("spoofer amplification level")
ax.set_ylabel("genuine-vs-counterfeit AUC")
ax.set_xticks(levels)
ax.set_ylim(0.35, 0.72)
ax.legend(fontsize=7, frameon=False, ncol=2, loc="upper right")
ax.grid(True, alpha=0.25, lw=0.5)
for ext in ("pdf", "png"):
    fig.savefig(os.path.join(OUT, f"fig5_satgrid.{ext}"), dpi=200, bbox_inches="tight")
print("wrote", os.path.join(OUT, "fig5_satgrid.pdf"))
