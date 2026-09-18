#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Generate the four paper figures from the optimism-study artifact JSON.

Reads the self-describing artifact produced by
`cargo run --release --example optimism_study` and writes PDF + PNG figures:
  fig1_scaling.{pdf,png}      H1: optimism gap vs distribution shift, by class
  fig2_family.{pdf,png}       H2: per-detector gap by family + matched pairs
  fig3_per_class.{pdf,png}    H3: per-class mean gap
  fig4_predictor.{pdf,png}    H4 (headline): predicted-vs-actual, both CV splits

Usage:
  python3 make_figures.py [artifact.json] [out_dir]
Defaults: artifact = ../../../kshana/paper-artifacts/optimism-study.json, out = ./figures
"""
import json
import os
import statistics as st
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ARTIFACT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    HERE, "..", "..", "kshana", "paper-artifacts", "optimism-study.json"
)
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "figures")
os.makedirs(OUT, exist_ok=True)

with open(ARTIFACT) as f:
    d = json.load(f)

cells = d["cells"]
classes = d["classes"]
PHYSICS = {"energy(cn0-drop)", "agc-excess", "sqm-imbalance", "raim-parity", "fused(max-z)"}


def save(fig, name):
    for ext in ("pdf", "png"):
        fig.savefig(os.path.join(OUT, f"{name}.{ext}"), bbox_inches="tight", dpi=150)
    plt.close(fig)
    print(f"wrote {name}.pdf / .png")


def family(det):
    if det in PHYSICS:
        return "physics"
    if det.startswith("logreg-"):
        return "learned (1-feature)"
    return "learned (multi-feature)"


# ── Fig 1: scaling — mean gap vs (1 - severity), per class + pooled ──────────
fig, ax = plt.subplots(figsize=(5.0, 3.4), layout="constrained")
sevs = sorted({c["severity"] for c in cells})
for cl in classes:
    xs = [1.0 - s for s in sevs]
    ys = [st.mean([c["mean_gap"] for c in cells if c["class"] == cl and c["severity"] == s]) for s in sevs]
    ax.plot(xs, ys, marker="o", label=cl, alpha=0.85)
pooled = [st.mean([c["mean_gap"] for c in cells if c["severity"] == s]) for s in sevs]
ax.plot([1.0 - s for s in sevs], pooled, marker="s", color="black", lw=2.4, label="all (pooled)")
ax.set_xlabel("distribution shift  (1 − severity scale)")
ax.set_ylabel("optimism gap  ΔAUC = AUC$_{in}$ − AUC$_{out}$")
ax.set_title("H1: the optimism gap grows with shift magnitude")
ax.legend(fontsize=7, frameon=False)
ax.grid(alpha=0.3)
save(fig, "fig1_scaling")

# ── Fig 2: per-detector mean gap by family + matched pairs ───────────────────
det_gap = {det: st.mean([c["mean_gap"] for c in cells if c["detector"] == det]) for det in d["detectors"]}
order = sorted(d["detectors"], key=lambda x: det_gap[x], reverse=True)
colors = {"physics": "#4C72B0", "learned (multi-feature)": "#C44E52", "learned (1-feature)": "#8172B3"}
fig, ax = plt.subplots(figsize=(6.2, 3.6), layout="constrained")
bars = ax.bar(range(len(order)), [det_gap[x] for x in order],
              color=[colors[family(x)] for x in order])
ax.set_xticks(range(len(order)))
ax.set_xticklabels(order, rotation=45, ha="right", fontsize=7)
ax.set_ylabel("mean optimism gap  ΔAUC")
ax.set_title("H2: gap is set by evidence breadth, not by learning")
handles = [plt.Rectangle((0, 0), 1, 1, color=colors[k]) for k in colors]
ax.legend(handles, list(colors.keys()), fontsize=7, frameon=False)
ax.grid(alpha=0.3, axis="y")
save(fig, "fig2_family")

# ── Fig 3: per-class mean gap ────────────────────────────────────────────────
fig, ax = plt.subplots(figsize=(4.6, 3.2), layout="constrained")
cl_gap = {cl: st.mean([c["mean_gap"] for c in cells if c["class"] == cl]) for cl in classes}
clo = sorted(classes, key=lambda x: cl_gap[x], reverse=True)
ax.bar(range(len(clo)), [cl_gap[x] for x in clo], color="#55A868")
ax.set_xticks(range(len(clo)))
ax.set_xticklabels(clo, rotation=20, ha="right", fontsize=8)
ax.set_ylabel("mean optimism gap  ΔAUC")
ax.set_title("H3: class heterogeneity (spoof-time smallest)")
ax.grid(alpha=0.3, axis="y")
save(fig, "fig3_per_class")

# ── Fig 4: predicted-vs-actual scatter, both CV splits (headline) ────────────
gp = d["gap_predictor"]
fig, axes = plt.subplots(1, 2, figsize=(7.2, 3.5), layout="constrained")
for ax, key, title in (
    (axes[0], "cv_leave_one_detector_out", "leave-one-detector-out"),
    (axes[1], "cv_leave_one_class_out", "leave-one-class-out"),
):
    cv = gp[key]
    pa = cv["scatter"]
    xs = [p["actual"] for p in pa]
    ys = [p["predicted"] for p in pa]
    ax.scatter(xs, ys, s=14, alpha=0.6, color="#C44E52", edgecolor="none")
    lo = min(min(xs), min(ys))
    hi = max(max(xs), max(ys))
    ax.plot([lo, hi], [lo, hi], "k--", lw=1, alpha=0.7)
    ax.set_xlabel("actual gap ΔAUC")
    ax.set_ylabel("predicted gap ΔAUC")
    ax.set_title(f"{title}\nR² = {cv['r2']:.2f}, RMSE = {cv['rmse']:.3f}", fontsize=9)
    ax.grid(alpha=0.3)
fig.suptitle("H4 (headline): ID-only features predict the held-out optimism gap",
             fontsize=10)
save(fig, "fig4_predictor")

print("done →", OUT)
