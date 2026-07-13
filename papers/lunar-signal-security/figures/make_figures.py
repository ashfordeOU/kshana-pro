# SPDX-License-Identifier: AGPL-3.0-only
# Figure pipeline for "A Closed-Form Link-Budget Assessment of the Spoofing and
# Jamming Attack Surface of the LunaNet Augmented Forward Signal" (PRIVATE, kshana-pro).
#
# Every number below is the SAME closed form the open Kshana engine evaluates
# (kshana::linkbudget free_space_loss_db / received_signal_power_dbw and
# kshana::jamming required_tx_power_dbw), reimplemented here so the figure is
# reproducible by hand. The P1 baseline scenario is
# src/packs/lunar_signal_security_pack.rs (LunarSignalSecurityScenario::default):
#   f = 2.4 GHz, P_sat = 13 dBW (20 W), G_sat = 13 dBi (EIRP 26 dBW),
#   G_user = 3 dBi, slant = 3000 km, GPS ref = -125 dBW,
#   attacker G_tx = 6 dBi, spoof J/S = 3 dB, jam J/S = 30 dB.
# Anchor cross-checks (must match the pack / ledger): spoof 0.022 mW @ 1 km,
# 8.9 mW @ 20 km, 0.222 W @ 100 km; jam 111 W @ 100 km.
import math
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

C = 299_792_458.0  # m/s


def fspl_db(d_m: float, f_hz: float) -> float:
    """Free-space path loss (dB): (4 pi d f / c)^2. Mirrors kshana::linkbudget."""
    return 20.0 * math.log10(4.0 * math.pi * d_m * f_hz / C)


# --- P1 baseline (pack defaults) ---------------------------------------------
F_HZ = 2.4e9
EIRP_DBW = 13.0 + 13.0          # P_sat + G_sat = 26 dBW
G_USER_DBI = 3.0
SLANT_M = 3.0e6                 # 3000 km
GPS_REF_DBW = -125.0
ATK_G_TX_DBI = 6.0
SPOOF_JS_DB = 3.0
JAM_JS_DB = 30.0

# Received AFS power at the surface (with user gain) and the isotropic level the
# J/S budget is referenced to (user gain cancels between signal and attacker).
P_RX_ISO_DBW = EIRP_DBW - fspl_db(SLANT_M, F_HZ)      # -143.6 dBW
P_RX_DBW = P_RX_ISO_DBW + G_USER_DBI                  # -140.6 dBW
DEFICIT_DB = GPS_REF_DBW - P_RX_DBW                   # 15.6 dB


def required_tx_dbw(d_m: float, js_db: float) -> float:
    """Attacker transmit power (dBW) to reach a target J/S at standoff d.
    Inverse free-space link, isotropic-referenced: mirrors jamming::required_tx_power_dbw."""
    required_iso_rx = P_RX_ISO_DBW + js_db
    return required_iso_rx + fspl_db(d_m, F_HZ) - ATK_G_TX_DBI


def dbw_to_w(p_dbw: float) -> float:
    return 10.0 ** (p_dbw / 10.0)


def main() -> None:
    # print the anchor cross-checks so a reviewer sees they match the pack
    for d_km, label in [(1, "1 km"), (20, "20 km"), (100, "100 km")]:
        ps = dbw_to_w(required_tx_dbw(d_km * 1e3, SPOOF_JS_DB))
        pj = dbw_to_w(required_tx_dbw(d_km * 1e3, JAM_JS_DB))
        print(f"  {label:>6}: spoof {ps*1e3:10.4g} mW   jam {pj:10.4g} W")
    print(f"  P_rx = {P_RX_DBW:.2f} dBW ; deficit vs GPS = {DEFICIT_DB:.2f} dB")

    d = [10.0 ** (x / 40.0) for x in range(int(40 * 2), int(40 * 5.301) + 1)]  # 100 m .. 200 km
    spoof_w = [dbw_to_w(required_tx_dbw(x, SPOOF_JS_DB)) for x in d]
    jam_w = [dbw_to_w(required_tx_dbw(x, JAM_JS_DB)) for x in d]
    d_km = [x / 1e3 for x in d]

    fig, ax = plt.subplots(figsize=(3.4, 2.7))
    ax.loglog(d_km, spoof_w, color="#0f766e", lw=1.8, label="spoof capture (J/S = 3 dB)")
    ax.loglog(d_km, jam_w, color="#b45309", lw=1.8, ls="--", label="denial jam (J/S = 30 dB)")

    # anchor markers
    for d_km_a, js, w_scale in [(1, SPOOF_JS_DB, 1), (20, SPOOF_JS_DB, 1), (100, SPOOF_JS_DB, 1)]:
        w = dbw_to_w(required_tx_dbw(d_km_a * 1e3, js))
        ax.plot([d_km_a], [w], "o", color="#0f766e", ms=4)
    ax.plot([100], [dbw_to_w(required_tx_dbw(100e3, JAM_JS_DB))], "s", color="#b45309", ms=4)

    ax.annotate("0.022 mW\n@ 1 km", (1, dbw_to_w(required_tx_dbw(1e3, SPOOF_JS_DB))),
                textcoords="offset points", xytext=(6, -2), fontsize=6.5)
    ax.annotate("0.222 W\n@ 100 km", (100, dbw_to_w(required_tx_dbw(100e3, SPOOF_JS_DB))),
                textcoords="offset points", xytext=(-20, 6), fontsize=6.5)
    ax.annotate("111 W\n@ 100 km", (100, dbw_to_w(required_tx_dbw(100e3, JAM_JS_DB))),
                textcoords="offset points", xytext=(-30, -14), fontsize=6.5)

    ax.set_xlabel("attacker standoff (km)", fontsize=8)
    ax.set_ylabel("required transmit power (W)", fontsize=8)
    ax.tick_params(labelsize=7)
    ax.grid(True, which="both", ls=":", lw=0.4, alpha=0.6)
    ax.legend(fontsize=6.5, loc="upper left", framealpha=0.9)
    fig.tight_layout(pad=0.3)
    fig.savefig("fig1a_attacker_power.pdf")
    print("  wrote fig1a_attacker_power.pdf")


if __name__ == "__main__":
    main()
