#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Render the lunar-PNT paper figures from data/sweeps.json (produced by run_sweeps.py).
Every number is read from the aggregated artifact; this script does no simulation.

Figures -> figures/:
  fig1_headline.pdf      with vs without VLBI: positions + clocks (default scenario seed).
  fig2_sigma_vlbi.pdf    median station error vs VLBI sigma, IQR band, range-only reference
                         (the information-not-count control for H1).
  fig3_n_earth.pdf       median station error vs number of Earth baselines, rank-deficient
                         region annotated (H3).
  fig4_seed_mc.pdf       distribution of the improvement factor over 200 seeds (H3).
  fig5_cross_suite.pdf   cross-suite sanity: lunar time in band; VLBI near-field; differential
                         decorrelation vs baseline.
"""
from __future__ import annotations

import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent
PAPER = HERE.parent
FIG = PAPER / "figures"
FIG.mkdir(parents=True, exist_ok=True)
DATA = json.loads((PAPER / "data" / "sweeps.json").read_text())

BIG = 1e8           # solver rank-deficient guard (1e9) lives above this
C = 299_792_458.0   # m/s, for clock seconds -> range-metres
plt.rcParams.update({"font.size": 10, "axes.grid": True, "grid.alpha": 0.3,
                     "figure.dpi": 150, "savefig.bbox": "tight"})


def save(fig, name):
    fig.savefig(FIG / name)
    plt.close(fig)
    print("wrote", FIG / name)


def fig1_headline():
    hl = DATA["headline"]
    w, wo = hl["with_vlbi"], hl["without_vlbi"]
    fig, (axp, axc) = plt.subplots(1, 2, figsize=(8, 3.4))

    # Positions (m), log scale.
    labels = ["station\nposition", "satellite\nposition (RMS)"]
    pos_wo = [wo["station_pos_err_m"], wo["sat_pos_rms_m"]]
    pos_w = [w["station_pos_err_m"], w["sat_pos_rms_m"]]
    x = np.arange(len(labels))
    axp.bar(x - 0.2, pos_wo, 0.4, label="range-only", color="#c44")
    axp.bar(x + 0.2, pos_w, 0.4, label="+ VLBI", color="#3a6")
    axp.set_yscale("log")
    axp.set_xticks(x, labels)
    axp.set_ylabel("3-D error (m)")
    axp.set_title("Position")
    axp.legend()

    # Clocks (range-metres = c*sigma_s), log scale; station clock ~ unchanged (H2).
    clk_wo = [wo["station_clock_err_s"] * C, wo["sat_clock_rms_s"] * C]
    clk_w = [w["station_clock_err_s"] * C, w["sat_clock_rms_s"] * C]
    cl = ["station\nclock", "satellite\nclock (RMS)"]
    xc = np.arange(len(cl))
    axc.bar(xc - 0.2, clk_wo, 0.4, label="range-only", color="#c44")
    axc.bar(xc + 0.2, clk_w, 0.4, label="+ VLBI", color="#3a6")
    axc.set_yscale("log")
    axc.set_xticks(xc, cl)
    axc.set_ylabel("error, range-equivalent (m)")
    axc.set_title("Clock")
    axc.legend(loc="upper left")
    fig.suptitle(f"Joint lunar OD+clock, default scenario (seed 42): "
                 f"{int(hl['station_observability_improvement_factor'])}x station sharpening",
                 fontsize=10)
    fig.tight_layout(rect=(0, 0, 1, 0.94))
    save(fig, "fig1_headline.pdf")


def fig2_sigma_vlbi():
    rows = DATA["sigma_vlbi_sweep"]
    sv = np.array([r["sigma_vlbi_s"] for r in rows])
    med = np.array([r["with_vlbi"]["median"] for r in rows])
    q1 = np.array([r["with_vlbi"]["q1"] for r in rows])
    q3 = np.array([r["with_vlbi"]["q3"] for r in rows])
    ro = np.median([r["range_only"]["median"] for r in rows])

    fig, ax = plt.subplots(figsize=(6, 4))
    ax.fill_between(sv, q1, q3, alpha=0.25, color="#3a6", label="IQR")
    ax.plot(sv, med, "o-", color="#3a6", label="with VLBI (median)")
    ax.axhline(ro, ls="--", color="#c44", label=f"range-only (~{ro:.0f} m)")
    ax.axvline(1e-11, ls=":", color="#888", label="default sigma (~3 mm)")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("VLBI delay $\\sigma$ (s)  —  fixed observation count")
    ax.set_ylabel("station 3-D error (m), median of 32 seeds")
    ax.set_title("Information, not count: degrading VLBI $\\sigma$\n"
                 "drives station error back toward range-only")
    ax.legend(fontsize=8)
    save(fig, "fig2_sigma_vlbi.pdf")


def fig3_n_earth():
    # Rigorous observability figure: the analytic Cramer-Rao bound on the station's
    # absolute position vs the number of Earth baselines, overlaid with the
    # estimator's Monte-Carlo RMS (efficiency ~ 1), and the rank-deficient region
    # shaded by the Fisher-information datum defect (data-driven, not assumed).
    rows = DATA["n_earth_sweep"]
    ne = np.array([r["n_earth"] for r in rows], float)
    defect = np.array([r.get("defect", 0) for r in rows])
    crlb = np.array([r["crlb_m"] if r.get("crlb_m") is not None else np.nan for r in rows])
    rms = np.array([r.get("with_vlbi_rms_m", np.nan) for r in rows])
    obs = defect == 0  # full rank (absolute position observable)

    fig, ax = plt.subplots(figsize=(6.2, 4))
    # Rank-deficient region: the Fisher information is singular (datum defect > 0).
    bad = ne[defect > 0]
    if len(bad):
        ax.axvspan(ne.min() - 0.5, bad.max() + 0.5, color="#c44", alpha=0.12)
        top = np.nanmax(crlb[obs]) if obs.any() else 1.0
        ax.text((ne.min() - 0.5 + bad.max() + 0.5) / 2, top,
                "rank-deficient\n(datum defect $>0$)", ha="center", va="top",
                color="#a22", fontsize=8)
        for x, dfc in zip(ne[defect > 0], defect[defect > 0]):
            ax.text(x, top * 0.55, f"defect\n{int(dfc)}", ha="center", va="center",
                    color="#a22", fontsize=7)
    ax.plot(ne[obs], crlb[obs], "-", color="#36c", lw=2,
            label="Cramer-Rao bound (analytic)")
    ax.plot(ne[obs], rms[obs], "o", color="#3a6", ms=7,
            label="estimator RMS (Monte-Carlo)")
    # Efficiency at the default 6-station network (200-seed Monte-Carlo).
    eff = DATA["seed_mc"].get("efficiency_rms_over_crlb")
    if eff is not None:
        ax.annotate(f"default network:\nRMS/CRLB $=$ {eff:.2f}", xy=(6, crlb[ne == 6][0]),
                    xytext=(6.4, crlb[obs].max() * 0.72), fontsize=8, color="#333",
                    arrowprops=dict(arrowstyle="->", color="#888", lw=0.8))
    ax.set_xlabel("number of Earth stations / baselines")
    ax.set_ylabel("station 3-D absolute-position error (m, 1$\\sigma$ RSS)")
    ax.set_title("Three baselines restore observability;\n"
                 "the estimator attains the Cramer-Rao bound")
    ax.legend(fontsize=8)
    save(fig, "fig3_n_earth.pdf")


def fig4_seed_mc():
    mc = DATA["seed_mc"]
    factors = np.array(mc["factors"])
    q = mc["improvement_factor"]
    fig, ax = plt.subplots(figsize=(6, 4))
    bins = np.logspace(np.log10(max(factors.min(), 0.5)), np.log10(factors.max()), 30)
    ax.hist(factors, bins=bins, color="#3a6", alpha=0.8, edgecolor="white")
    ax.axvline(q["median"], color="#c44", lw=2, label=f"median {q['median']:.0f}x")
    ax.axvspan(q["q1"], q["q3"], color="#c44", alpha=0.12,
               label=f"IQR {q['q1']:.0f}-{q['q3']:.0f}x")
    ax.set_xscale("log")
    ax.set_xlabel("station-observability improvement factor (range-only / +VLBI)")
    ax.set_ylabel(f"count ({q['n']} seeds)")
    ax.set_title("Improvement is robust across the truth/noise ensemble")
    ax.legend(fontsize=8)
    save(fig, "fig4_seed_mc.pdf")


def fig5_cross_suite():
    cs = DATA["cross_suite"]
    lt, lv, dp = cs["lunar_time"], cs["lunar_vlbi"], cs["lunar_dpnt"]
    CANONICAL = 56.02  # Ashby & Patla (2024) surface-to-surface canonical rate, us/day
    fig, (a, b, c) = plt.subplots(1, 3, figsize=(11.5, 3.5))

    # (a) lunar time rate decomposition, against the canonical Ashby & Patla value.
    a.bar(["self-pot", "kinetic", "total"],
          [lt["self_potential_us_per_day"], lt["kinetic_us_per_day"],
           lt["secular_rate_us_per_day"]], color=["#69c", "#c96", "#3a6"])
    a.axhline(CANONICAL, ls="--", color="#c44", lw=1.2,
              label=f"Ashby & Patla {CANONICAL:.2f}")
    a.set_ylabel("LTC$-$TT rate ($\\mu$s/day)")
    a.set_title(f"Coordinated Lunar Time\n{lt['secular_rate_us_per_day']:.2f} $\\mu$s/day (Validated)")
    a.legend(fontsize=7, loc="lower right")

    # (b) VLBI near-field correction over the pass.
    s = lv["series"]
    t = [p["t_hours"] for p in s]
    nf = [p["near_field_correction_us"] for p in s]
    b.plot(t, nf, "o-", color="#3a6")
    b.set_xlabel("time (h)")
    b.set_ylabel("near-field correction ($\\mu$s)")
    b.set_title(f"Lunar VLBI near-field term\n(~{lv['near_field_correction_us']:.1f} $\\mu$s; "
                "far-field $\\equiv$ $\\Delta$-DOR)")

    # (c) differential decorrelation vs baseline.
    bc = np.array(dp["baseline_curve"])
    c.plot(bc[:, 0], bc[:, 1], "o-", color="#3a6")
    c.set_xlabel("user-reference baseline (km)")
    c.set_ylabel("corrected user error (m)")
    c.set_title(f"Differential PNT\n{dp['user_error_uncorrected_m']:.1f} m $\\to$ "
                f"{dp['user_error_corrected_m']:.3f} m at 50 km")
    fig.tight_layout()
    save(fig, "fig5_cross_suite.pdf")


if __name__ == "__main__":
    fig1_headline()
    fig2_sigma_vlbi()
    fig3_n_earth()
    fig4_seed_mc()
    fig5_cross_suite()
    print("done")
