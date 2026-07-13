# SPDX-License-Identifier: AGPL-3.0-only
# Figure pipeline for "A User-Level Error Budget for Realizing Coordinated Lunar
# Time: Frame-Realization versus Clock-Stability Contributions" (PRIVATE, kshana-pro).
#
# Left panel  : the seven-term realization error budget at tau = 1 day (sorted,
#               vs the 1 ns requirement line) -- the paper's headline result.
#               Numbers are the committed P3 Table 1 values reproduced by the
#               kshana-pro lunar-time-budget study (ledger oracle:
#               0.009/0.035/0.901/0.995/1.732/50.035/151.238 ns).
# Right panel : the tau-swept RSS budget x_sigma(tau) from the committed
#               lunar_time_budget.csv, marking the clock-vs-frame crossover, so the
#               frame-limited regime is shown not to be a single-tau artifact.
import csv
import math
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

HERE = os.path.dirname(__file__)
CSV = os.path.join(HERE, "..", "lunar_time_budget.csv")

# --- P3 Table 1: seven-term realization budget at tau = 1 day (ns) ------------
# (label, error_ns, provenance)  provenance in {"V": Validated, "M": Modelled/Imported}
TERMS = [
    ("Ground optical master clock", 0.009, "V"),
    ("Optical two-way transfer", 0.035, "M"),
    ("Post-processed frame/ephemeris", 0.901, "M"),
    ("Space passive H-maser (PHM)", 0.995, "V"),
    ("RF two-way transfer", 1.732, "M"),
    ("Real-time frame/ephemeris", 50.035, "M"),
    ("500 g miniRAFS (SWaP)", 151.238, "V"),
]
REQ_NS = 1.0


def load_csv_sweep():
    tau, xsig, xclock, xframe = [], [], [], []
    xover_tau = xover_x = None
    with open(CSV) as f:
        for line in f:
            if line.startswith("#"):
                if "crossover_tau_s=" in line:
                    for tok in line.split():
                        if tok.startswith("crossover_tau_s="):
                            xover_tau = float(tok.split("=")[1])
                        if tok.startswith("crossover_x_s="):
                            xover_x = float(tok.split("=")[1])
                continue
            break
    with open(CSV) as f:
        rdr = csv.DictReader(r for r in f if not r.startswith("#"))
        for row in rdr:
            tau.append(float(row["tau_s"]))
            xsig.append(float(row["x_sigma_s"]) * 1e9)  # -> ns
            xclock.append(float(row["x_clock_s"]) * 1e9)
            xframe.append(float(row["x_frame_realisation_s"]) * 1e9)
    return tau, xsig, xclock, xframe, xover_tau, xover_x


def main():
    fig, (axL, axR) = plt.subplots(1, 2, figsize=(7.0, 2.9))

    # --- left: sorted budget bar chart vs 1 ns ---
    terms = sorted(TERMS, key=lambda t: t[1])
    labels = [t[0] for t in terms]
    vals = [t[1] for t in terms]
    colors = ["#2dd4bf" if t[2] == "V" else "#b45309" for t in terms]
    y = range(len(terms))
    axL.barh(list(y), vals, color=colors, height=0.62)
    axL.set_xscale("log")
    axL.axvline(REQ_NS, color="#111827", ls="--", lw=1.0)
    axL.set_yticks(list(y))
    axL.set_yticklabels(labels, fontsize=6.3)
    axL.set_xlabel("timing error at $\\tau=1$ day (ns)", fontsize=7.5)
    axL.tick_params(axis="x", labelsize=7)
    axL.set_xlim(3e-3, 4e2)
    axL.text(REQ_NS * 1.15, 0.15, "1 ns\nrequirement", fontsize=6.2, color="#111827")
    for yi, v in zip(y, vals):
        axL.text(v * 1.15, yi, f"{v:g}", va="center", fontsize=5.8)
    axL.set_title("(a) realization budget, sorted", fontsize=8)

    # --- right: clock-vs-frame crossover (the two crossing terms) + RSS ---
    tau, xsig, xclock, xframe, xover_tau, xover_x = load_csv_sweep()
    axR.loglog(tau, xclock, color="#0f766e", lw=1.7, label="clock $x_{\\rm clk}(\\tau)$")
    axR.loglog(tau, xframe, color="#b45309", lw=1.7, ls="-", label="frame $x_{\\rm frame}$")
    axR.loglog(tau, xsig, color="#6b7280", lw=1.0, ls=":", label="RSS $x_\\Sigma$")
    axR.axhline(REQ_NS, color="#111827", ls="--", lw=0.8)
    if xover_tau:
        axR.axvline(xover_tau, color="#111827", ls=":", lw=0.8)
        axR.plot([xover_tau], [xover_x * 1e9], "o", color="#111827", ms=4)
        axR.annotate(
            f"crossover $\\tau^*\\approx${xover_tau/86400:.2f} d\n({xover_x*1e9:.2f} ns)",
            (xover_tau, xover_x * 1e9),
            textcoords="offset points", xytext=(-60, 12), fontsize=6.0,
        )
    axR.set_xlabel("averaging time $\\tau$ (s)", fontsize=7.5)
    axR.set_ylabel("timing error (ns)", fontsize=7.5)
    axR.tick_params(labelsize=7)
    axR.set_ylim(1e-3, 5e1)
    axR.set_title("(b) clock-vs-frame crossover", fontsize=8)
    axR.grid(True, which="both", ls=":", lw=0.4, alpha=0.5)
    axR.legend(fontsize=5.6, loc="lower right", framealpha=0.9)

    fig.tight_layout(pad=0.4)
    fig.savefig(os.path.join(HERE, "fig1_ltc_budget.pdf"))
    print(f"  wrote fig1_ltc_budget.pdf ; crossover tau*={xover_tau:.4g}s "
          f"({xover_tau/86400:.3f} d), clock=frame at x*={xover_x*1e9:.4g} ns")
    idx = min(range(len(tau)), key=lambda i: abs(tau[i] - 86400.0))
    print(f"  at tau=1day: x_clock={xclock[idx]:.4g} ns, x_frame={xframe[idx]:.4g} ns, "
          f"x_sigma={xsig[idx]:.3f} ns")


if __name__ == "__main__":
    main()
