#!/usr/bin/env python3
"""Write the timing assurance v2 demo inputs (W1120 TA1-TA3).

Every log here is SYNTHETIC: written with Python's random.Random(20261120) in the layouts the
open engine's line readers take (linuxptp ptp4l, chrony statistics.log, Linux PPS assert) and
in plain CSV. No device, daemon or counter ran. The scenarios follow incidents in the research
reports (05, 07b) in shape only:

* node/: a 5G node whose GNSS PPS reads healthy while its time is 6,250 us off for two hours
  (the shape of the Turkish operator's cell during the NATO summit), seen by PTP from a
  regional grandmaster and two NTP servers; a PTP path-delay step at 06:00.
* sites/: six sites of three operators; one site diverges by 6,250 us, one operator's two
  sites drift together by 3 us, and every site moves by 13 us for 30 minutes (the SVN-23
  shape, seen against a non-GNSS monitor).
* timebomb/captures/: device output during two replays: a receiver that falls back 1024 weeks
  at the 2038 rollover, and one that does not insert the synthetic leap second.

    python3 examples/timing-assurance-v2/generate.py
"""
import datetime
import os
import random

HERE = os.path.dirname(os.path.abspath(__file__))
R = random.Random(20261120)
T0 = 1795046400  # 2026-11-19T00:00:00Z
SYN = "SYNTHETIC: written by examples/timing-assurance-v2/generate.py (random.Random(20261120)); not a measurement."


def w(path, lines):
    p = os.path.join(HERE, path)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def node():
    hours = 8
    off = lambda t: 6.25e-3 if T0 + 3 * 3600 <= t < T0 + 5 * 3600 else 0.0
    # GNSS PPS (the disciplining source): the node follows it, so it reads healthy throughout.
    pps = []
    for i, t in enumerate(range(T0, T0 + hours * 3600, 10)):
        e = R.gauss(0, 15e-9)
        ns = int(round(e * 1e9))
        if ns >= 0:
            pps.append(f"{t}.{ns:09d}#{1000 + i}")
        else:
            pps.append(f"{t - 1}.{1_000_000_000 + ns:09d}#{1000 + i}")
    w("node/pps.assert", ["# " + SYN] + pps)
    # PTP from a regional grandmaster on a GNSS-independent reference: offset = node - GM.
    ptp = ["# " + SYN]
    for t in range(T0, T0 + hours * 3600, 10):
        o = off(t) + R.gauss(0, 40e-9)
        d = 12034 + R.gauss(0, 15) + (850 if t >= T0 + 6 * 3600 else 0)
        ptp.append(
            f"ptp4l[{t}.{R.randint(0, 999):03d}]: master offset {int(round(o * 1e9)):>10d} s2 freq {R.randint(-400, 400):+6d} path delay {int(round(d)):>7d}"
        )
    w("node/ptp4l.log", ptp)
    # Two NTP servers in chrony's statistics.log (Est offset, column 5: positive = local fast).
    chrony = ["# " + SYN]
    for t in range(T0, T0 + hours * 3600, 64):
        dt = datetime.datetime.fromtimestamp(t, datetime.timezone.utc)
        for addr, sd in (("192.0.2.10", 150e-6), ("198.51.100.20", 250e-6)):
            o = off(t) + R.gauss(0, sd)
            chrony.append(
                f"{dt:%Y-%m-%d %H:%M:%S} {addr:<15} {sd:.3e} {o:+.3e} {sd:.3e} {R.gauss(0, 1e-8):+.3e} {1e-9:.3e} 0.0e+00 16 8 4 0.00"
            )
    w("node/chrony-statistics.log", chrony)
    w("node/holdover-state.csv", ["# " + SYN, f"{T0},locked"])


def sites():
    ids = [("a-1", "OpA", "north"), ("a-2", "OpA", "south"), ("b-1", "OpB", "north"),
           ("b-2", "OpB", "south"), ("c-1", "OpC", "north"), ("c-2", "OpC", "south")]
    for sid, op, _ in ids:
        rows = ["# " + SYN, "# unix_time,offset_s (site time - common non-GNSS monitor)"]
        for t in range(T0, T0 + 12 * 3600, 60):
            o = R.gauss(0, 50e-9)
            if sid == "b-2" and T0 + 2 * 3600 <= t < T0 + 3 * 3600:
                o += 6.25e-3
            if op == "OpA" and T0 + 5 * 3600 <= t < T0 + 7 * 3600:
                o += 3e-6
            if T0 + 9 * 3600 <= t < T0 + 9 * 3600 + 1800:
                o += 13e-6
            rows.append(f"{t},{o:.4e}")
        w(f"sites/{sid}.csv", rows)


def captures():
    """Device output during the replays, derived from the vectors' expected epochs."""

    def civil(days):
        d = datetime.date(1970, 1, 1) + datetime.timedelta(days=days)
        return d

    def zda(days, sod, talker="GP"):
        d = civil(days)
        h, m, s = (23, 59, 60) if sod >= 86400 else (sod // 3600, (sod // 60) % 60, sod % 60)
        body = f"{talker}ZDA,{h:02d}{m:02d}{s:02d}.00,{d.day:02d},{d.month:02d},{d.year:04d},00,00"
        cs = 0
        for ch in body:
            cs ^= ord(ch)
        return f"${body}*{cs:02X}"

    # gps-wnro-2038: boundary 2038-11-20T23:59:42Z (week 3072 at GPS - UTC = 18 s), 120 s before.
    b = 2173910382
    lab = 1900000000.000
    lines = ["# " + SYN, "# lab_unix_time NMEA (device falls back 1024 weeks at the rollover)"]
    for k in range(300):
        u = b - 120 + k
        if u >= b:
            u -= 1024 * 604800
        lines.append(f"{lab + k:.3f} {zda(u // 86400, u % 86400)}")
    w("timebomb/captures/wnro-faulty.nmea", lines)
    # leap-insert at the end of 2027-06-30: the device goes 23:59:59 -> 00:00:00 (no :60).
    end = 1814400000  # 2027-07-01T00:00:00Z
    lines = ["# " + SYN, "# device output during leap-insert (no 23:59:60)"]
    for k in range(300):
        label = end - 120 + k  # device counts seconds without the leap second
        lines.append(zda(label // 86400, label % 86400))
    w("timebomb/captures/leap-not-inserted.nmea", lines)


if __name__ == "__main__":
    node()
    sites()
    captures()
