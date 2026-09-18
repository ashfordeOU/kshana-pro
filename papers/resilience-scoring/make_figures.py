#!/usr/bin/env python3
"""Generate the figures for the resilience-scoring paper from the study artifact.

Reads kshana/paper-artifacts/resilience-study.json (produced by
`cargo run --release --example resilience_report -- paper-artifacts/resilience-study.json`)
and writes five PDF figures into ./figures/. Deterministic; no network.
"""
import json
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = os.path.dirname(os.path.abspath(__file__))
ART = os.path.join(HERE, "..", "..", "..", "kshana", "paper-artifacts", "resilience-study.json")
OUT = os.path.join(HERE, "figures")
os.makedirs(OUT, exist_ok=True)

plt.rcParams.update(
    {
        "font.size": 9,
        "figure.dpi": 150,
        "savefig.bbox": "tight",
        "axes.grid": True,
        "grid.alpha": 0.3,
    }
)

with open(ART) as f:
    D = json.load(f)

THREATS = ["nominal", "wideband_jam", "spoofing", "meaconing", "combined"]
THREAT_LABELS = ["Nominal", "Wideband\njam", "Spoofing", "Meaconing", "Combined"]
ARCHS = list(D["profiles"].keys())


def save(fig, name):
    p = os.path.join(OUT, name)
    fig.savefig(p)
    plt.close(fig)
    print("wrote", p)


# Fig 1: rank ranges (broad simplex) -- the headline instability.
def fig_rank_ranges():
    rr = D["h1_instability"]["broad_simplex"]["rank_ranges"]
    items = sorted(rr.items(), key=lambda kv: (kv[1][0], kv[1][1]))
    names = [k for k, _ in items]
    los = [v[0] + 1 for _, v in items]  # 1-indexed ranks for readability
    his = [v[1] + 1 for _, v in items]
    fig, ax = plt.subplots(figsize=(5.0, 2.8))
    y = range(len(names))
    for i, (lo, hi) in enumerate(zip(los, his)):
        ax.plot([lo, hi], [i, i], color="#444", lw=6, solid_capstyle="round", alpha=0.85)
        ax.plot([lo], [i], "o", color="#1f77b4", ms=6)
        ax.plot([hi], [i], "o", color="#d62728", ms=6)
    ax.set_yticks(list(y))
    ax.set_yticklabels(names)
    ax.set_xlabel("Rank attained across defensible weightings x threats (1 = best)")
    ax.set_xticks(range(1, len(ARCHS) + 1))
    ax.invert_yaxis()
    ax.set_title("Rank instability of a single composite score")
    save(fig, "fig1_rank_ranges.pdf")


# Fig 2: composite (equal weight) per architecture x threat.
def fig_composite_heatmap():
    import numpy as np

    M = np.array(
        [[D["profiles"][a][t]["composite_equal_weight"] for t in THREATS] for a in ARCHS]
    )
    fig, ax = plt.subplots(figsize=(5.0, 3.0))
    im = ax.imshow(M, aspect="auto", cmap="viridis", vmin=0, vmax=1)
    ax.set_xticks(range(len(THREATS)))
    ax.set_xticklabels(THREAT_LABELS)
    ax.set_yticks(range(len(ARCHS)))
    ax.set_yticklabels(ARCHS)
    for i in range(len(ARCHS)):
        for j in range(len(THREATS)):
            ax.text(j, i, f"{M[i, j]:.2f}", ha="center", va="center",
                    color="white" if M[i, j] < 0.55 else "black", fontsize=7)
    ax.grid(False)
    fig.colorbar(im, ax=ax, label="Composite score (equal weights)")
    ax.set_title("Composite score by architecture and threat")
    save(fig, "fig2_composite_heatmap.pdf")


# Fig 3: assigned RPCF Level per architecture x threat.
def fig_level_heatmap():
    import numpy as np

    M = np.array([[D["profiles"][a][t]["level"] for t in THREATS] for a in ARCHS])
    fig, ax = plt.subplots(figsize=(5.0, 3.0))
    im = ax.imshow(M, aspect="auto", cmap="cividis", vmin=0, vmax=4)
    ax.set_xticks(range(len(THREATS)))
    ax.set_xticklabels(THREAT_LABELS)
    ax.set_yticks(range(len(ARCHS)))
    ax.set_yticklabels(ARCHS)
    for i in range(len(ARCHS)):
        for j in range(len(THREATS)):
            ax.text(j, i, str(M[i, j]), ha="center", va="center",
                    color="white" if M[i, j] < 2 else "black", fontsize=8)
    ax.grid(False)
    fig.colorbar(im, ax=ax, label="Tentative RPCF Level (0-4)")
    ax.set_title("A single Level is scenario-dependent, not an architecture property")
    save(fig, "fig3_level_heatmap.pdf")


# Fig 4: diversity collapse (independence-assumed vs common-mode-aware).
def fig_diversity_collapse():
    import numpy as np

    rows = D["h3_diversity_collapse"]
    names = [r["arch"] for r in rows]
    assumed = [r["independence_assumed"] for r in rows]
    aware = [r["common_mode_aware"] for r in rows]
    order = sorted(range(len(names)), key=lambda i: -assumed[i])
    names = [names[i] for i in order]
    assumed = [assumed[i] for i in order]
    aware = [aware[i] for i in order]
    x = np.arange(len(names))
    w = 0.4
    fig, ax = plt.subplots(figsize=(5.2, 2.9))
    ax.bar(x - w / 2, assumed, w, label="Independence assumed", color="#1f77b4")
    ax.bar(x + w / 2, aware, w, label="Common-mode aware", color="#d62728")
    ax.set_xticks(x)
    ax.set_xticklabels(names, rotation=30, ha="right")
    ax.set_ylabel("Effective diversity (Hill N2)")
    ax.legend()
    ax.set_title("Apparent vs effective redundancy under common-mode RF denial")
    save(fig, "fig4_diversity_collapse.pdf")


# Fig 5: weighting-concentration sweep + weighting-only-per-scenario separation.
def fig_weighting_sensitivity():
    import numpy as np

    sweep = D["h1_alpha_sweep"]
    alphas = [r["alpha"] for r in sweep]
    flip = [r["top1_flip_rate"] for r in sweep]
    wo = D["h1_weighting_only_per_scenario"]
    order = ["nominal", "wideband_jam", "spoofing", "meaconing", "combined"]
    labels = ["Nominal", "Jam", "Spoof", "Meacon", "Combined"]
    wo_flip = [wo[t]["top1_flip_rate"] for t in order]

    fig, (a1, a2) = plt.subplots(1, 2, figsize=(6.2, 2.7), constrained_layout=True)
    a1.plot(alphas, flip, "o-", color="#d62728")
    a1.set_xscale("log")
    a1.set_xlabel("Dirichlet $\\alpha$ (broad $\\rightarrow$ near-equal)")
    a1.set_ylabel("Top-1 winner flip rate")
    a1.set_title("Winner instability vs prior", fontsize=9)
    a2.bar(range(len(order)), wo_flip, color="#1f77b4", width=0.6)
    a2.set_xticks(range(len(order)))
    a2.set_xticklabels(labels, rotation=20, ha="right")
    a2.set_ylabel("Re-weighting-only flip rate")
    a2.set_title("Re-weighting only, per scenario", fontsize=9)
    save(fig, "fig5_weighting_sensitivity.pdf")


if __name__ == "__main__":
    fig_rank_ranges()
    fig_composite_heatmap()
    fig_level_heatmap()
    fig_diversity_collapse()
    fig_weighting_sensitivity()
    print("done")
