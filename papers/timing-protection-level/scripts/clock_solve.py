#!/usr/bin/env python3
"""Rigorous single-point receiver clock-bias trajectory from raw L1 pseudoranges.

Fix the antenna at its independently-known clean (dual-band) position, compute GPS
satellite ECEF positions + clock corrections from broadcast ephemeris (IS-GPS-200),
and robust-solve the receiver clock bias each epoch:  b(t) = median_s [ PR_s - |X_s - X_rx|
+ c*dt_sat_s ] / c .  Clean-epoch residual RMS validates the solver before any
spoof-window deviation is believed.  Pure stdlib (no numpy).
"""
import csv, math, sys, statistics as st
from datetime import datetime, timezone, timedelta

C = 299792458.0
MU = 3.986005e14
OMEGA_E = 7.2921151467e-5
F_REL = -4.442807633e-10
GPS_EPOCH = datetime(1980, 1, 6, tzinfo=timezone.utc)
# RINEX observation epochs are GPS time already; the rinex.csv 'time' field is GPS
# time labelled as a civil timestamp, so no UTC->GPS leap offset is applied here.
# (Empirically verified: a time-offset sweep minimises per-sat residual RMS at 0 s,
# ~8 m; each 1 s of offset costs ~495 m of along-track range error.)
LEAP = 0


def d2f(s):
    s = s.strip().replace("D", "E").replace("d", "E")
    return float(s) if s else 0.0


def parse_nav(path):
    """Return {prn: [eph dicts]} for GPS only."""
    eph = {}
    iono = None
    with open(path) as f:
        lines = f.readlines()
    i = 0
    # header
    while i < len(lines):
        if "GPSA" in lines[i]:
            vals = lines[i][:60]
            iono = [d2f(vals[5 + 12 * k : 5 + 12 * (k + 1)]) for k in range(4)]
        if "END OF HEADER" in lines[i]:
            i += 1
            break
        i += 1
    while i < len(lines):
        ln = lines[i]
        if not ln.strip() or ln[0] != "G":
            i += 1
            continue
        prn = ln[0:3].strip()
        yr, mo, da, hh, mm = (int(ln[4:8]), int(ln[9:11]), int(ln[12:14]),
                              int(ln[15:17]), int(ln[18:20]))
        ss = int(ln[21:23])
        toc = datetime(yr, mo, da, hh, mm, ss, tzinfo=timezone.utc)
        af0, af1, af2 = d2f(ln[23:42]), d2f(ln[42:61]), d2f(ln[61:80])
        orb = []
        for k in range(7):
            i += 1
            l = lines[i].rstrip("\n")
            l = l + " " * (80 - len(l))
            orb.extend([d2f(l[4:23]), d2f(l[23:42]), d2f(l[42:61]), d2f(l[61:80])])
        e = dict(
            prn=prn, toc=toc, af0=af0, af1=af1, af2=af2,
            iode=orb[0], crs=orb[1], deltan=orb[2], m0=orb[3],
            cuc=orb[4], ecc=orb[5], cus=orb[6], sqrta=orb[7],
            toe=orb[8], cic=orb[9], omega0=orb[10], cis=orb[11],
            i0=orb[12], crc=orb[13], omega=orb[14], omegadot=orb[15],
            idot=orb[16], week=orb[18], tgd=orb[22],
        )
        eph.setdefault(prn, []).append(e)
        i += 1
    return eph, iono


def gps_seconds(dt):
    return (dt - GPS_EPOCH).total_seconds()


def sat_state(e, t_gps_abs):
    """ECEF pos + clock correction (s) at absolute GPS seconds t_gps_abs."""
    toe_abs = e["week"] * 604800.0 + e["toe"]
    tk = t_gps_abs - toe_abs
    if tk > 302400:
        tk -= 604800
    elif tk < -302400:
        tk += 604800
    a = e["sqrta"] ** 2
    n0 = math.sqrt(MU / a**3)
    n = n0 + e["deltan"]
    M = e["m0"] + n * tk
    E = M
    for _ in range(15):
        E = M + e["ecc"] * math.sin(E)
    sinE, cosE = math.sin(E), math.cos(E)
    nu = math.atan2(math.sqrt(1 - e["ecc"] ** 2) * sinE, cosE - e["ecc"])
    phi = nu + e["omega"]
    du = e["cuc"] * math.cos(2 * phi) + e["cus"] * math.sin(2 * phi)
    dr = e["crc"] * math.cos(2 * phi) + e["crs"] * math.sin(2 * phi)
    di = e["cic"] * math.cos(2 * phi) + e["cis"] * math.sin(2 * phi)
    u = phi + du
    r = a * (1 - e["ecc"] * cosE) + dr
    ii = e["i0"] + di + e["idot"] * tk
    xp = r * math.cos(u)
    yp = r * math.sin(u)
    Omega = e["omega0"] + (e["omegadot"] - OMEGA_E) * tk - OMEGA_E * e["toe"]
    cosO, sinO = math.cos(Omega), math.sin(Omega)
    cosi, sini = math.cos(ii), math.sin(ii)
    x = xp * cosO - yp * cosi * sinO
    y = xp * sinO + yp * cosi * cosO
    z = yp * sini
    # clock correction (relativity + polynomial + Tgd)
    toc_abs = gps_seconds(e["toc"])
    dtc = t_gps_abs - toc_abs
    drel = F_REL * e["ecc"] * e["sqrta"] * sinE
    dt_sv = e["af0"] + e["af1"] * dtc + e["af2"] * dtc**2 + drel - e["tgd"]
    return x, y, z, dt_sv


def best_eph(eph_list, t_abs):
    return min(eph_list, key=lambda e: abs(t_abs - (e["week"] * 604800.0 + e["toe"])))


def lla_to_ecef(lat, lon, h):
    a, f = 6378137.0, 1 / 298.257223563
    e2 = f * (2 - f)
    lat, lon = math.radians(lat), math.radians(lon)
    N = a / math.sqrt(1 - e2 * math.sin(lat) ** 2)
    x = (N + h) * math.cos(lat) * math.cos(lon)
    y = (N + h) * math.cos(lat) * math.sin(lon)
    z = (N * (1 - e2) + h) * math.sin(lat)
    return x, y, z


def parse_t(ts):
    ts = ts.strip()
    fmt = "%Y-%m-%d %H:%M:%S.%f" if "." in ts else "%Y-%m-%d %H:%M:%S"
    return datetime.strptime(ts, fmt).replace(tzinfo=timezone.utc)


def main():
    rinex_csv = sys.argv[1]
    navpvt_csv = sys.argv[2]
    nav = sys.argv[3]
    clean_min = float(sys.argv[4]) if len(sys.argv) > 4 else 15.0
    out = sys.argv[5] if len(sys.argv) > 5 else "/tmp/jt/clock.csv"

    eph, _ = parse_nav(nav)
    print(f"nav: {len(eph)} GPS PRNs, {sum(len(v) for v in eph.values())} records")

    # receiver position from clean baseline PVT median
    pv = list(csv.DictReader(open(navpvt_csv)))
    t0 = int(pv[0]["itow_ms"])
    clean = [r for r in pv if (int(r["itow_ms"]) - t0) / 1000.0 < clean_min * 60]
    lat = st.median(float(r["lat_deg"]) for r in clean)
    lon = st.median(float(r["lon_deg"]) for r in clean)
    h = st.median(float(r["h_m"]) for r in clean)
    Xr = lla_to_ecef(lat, lon, h)
    print(f"rx LLA = {lat:.7f},{lon:.7f},{h:.1f}  ECEF={tuple(round(v,1) for v in Xr)}")

    # group rinex by epoch
    rows = csv.DictReader(open(rinex_csv))
    epochs = {}
    for r in rows:
        if not r["satellite"].startswith("G"):
            continue
        pr = r["pseudorange_L1"]
        if not pr:
            continue
        epochs.setdefault(r["time"], []).append((r["satellite"], float(pr),
                                                  float(r["carrier_phase_L1"] or 0),
                                                  float(r["snr_L1"] or 0)))
    ts_sorted = sorted(epochs)
    te0 = parse_t(ts_sorted[0])
    results = []
    for ts in ts_sorted:
        dt = parse_t(ts)
        t_abs = gps_seconds(dt) + LEAP
        biases, spread = [], []
        for sat, pr, _cp, snr in epochs[ts]:
            if sat not in eph:
                continue
            e = best_eph(eph[sat], t_abs)
            if abs(t_abs - (e["week"] * 604800.0 + e["toe"])) > 7200:
                continue
            # transmit-time iteration
            tau = pr / C
            for _ in range(3):
                xs, ys, zs, dts = sat_state(e, t_abs - tau)
                # Sagnac: rotate sat into rx frame at receive time
                th = OMEGA_E * tau
                xr = xs * math.cos(th) + ys * math.sin(th)
                yr = -xs * math.sin(th) + ys * math.cos(th)
                rng = math.dist((xr, yr, zs), Xr)
                tau = (rng - C * dts) / C
            resid = pr - rng + C * dts  # = c*b_rx + iono + tropo + noise
            biases.append(resid / C * 1e9)  # ns
        if len(biases) < 5:
            continue
        med = st.median(biases)
        mad = st.median(abs(b - med) for b in biases)
        tmin = (dt - te0).total_seconds() / 60.0
        results.append((tmin, med, len(biases), mad * 1.4826))

    # clean-epoch validation: clock should be smooth; fit linear, report residual
    clean_r = [r for r in results if r[0] < clean_min]
    if len(clean_r) > 10:
        xs = [r[0] for r in clean_r]
        ys = [r[1] for r in clean_r]
        n = len(xs)
        mx, my = sum(xs) / n, sum(ys) / n
        b1 = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
        b0 = my - b1 * mx
        res = [y - (b0 + b1 * x) for x, y in zip(xs, ys)]
        rms = math.sqrt(sum(r * r for r in res) / len(res))
        print(f"CLEAN-EPOCH VALIDATION: drift={b1*60:.2f} ns/min, "
              f"detrended clock RMS={rms:.2f} ns over {n} epochs, "
              f"cross-sat MAD-sigma median={st.median(r[3] for r in clean_r):.1f} ns")

    with open(out, "w") as f:
        f.write("t_min,clock_ns,n_sats,xsat_sigma_ns\n")
        for r in results:
            f.write(f"{r[0]:.4f},{r[1]:.3f},{r[2]},{r[3]:.2f}\n")
    print(f"wrote {len(results)} epochs -> {out}")
    # quick window summary
    def win(a, b):
        w = [r for r in results if a <= r[0] < b]
        if not w:
            return "n/a"
        return f"clock {st.median(r[1] for r in w):8.1f} ns (min {min(r[1] for r in w):.0f}, max {max(r[1] for r in w):.0f}), nsat~{st.median(r[2] for r in w):.0f}"
    print("baseline 25-33 min:", win(25, 33))
    print("capture  34-39 min:", win(34, 40))
    print("recovery 40-42 min:", win(40, 42))


if __name__ == "__main__":
    main()
