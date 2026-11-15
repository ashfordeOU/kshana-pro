#!/usr/bin/env python3
"""Write the demo timing fleet's logs (roadmap 0.42 T2 demo).

Two clocks are REAL public data: GPS satellite rubidium clocks G03 and G25 from the IGS final
clock combination (tests/fixtures/timing_v5_igs/clocks.txt, see its NOTICE.md), written as
TimeLab phase logs. They stand in for free-running site references; they are satellite clocks.

Every other log is SYNTHETIC: written here with Python's random.Random(20261115) in the layouts
the open engine's readers take (TICC timestamp mode, linuxptp ptp4l, chrony tracking.log,
White Rabbit stat lines, ntpd loopstats, Linux PPS assert). No device, daemon or counter ran.

    python3 examples/timing-fleet/generate.py
"""
import datetime
import math
import os
import random

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
LOGS = os.path.join(HERE, "logs")
R = random.Random(20261115)
START = datetime.datetime(2026, 11, 14, 0, 0, 0)
SYN = "SYNTHETIC: written by examples/timing-fleet/generate.py (random.Random(20261115)); not a measurement."


def igs(prn):
    cur, first, out = None, 0.0, []
    acc = 0
    for line in open(os.path.join(ROOT, "tests/fixtures/timing_v5_igs/clocks.txt")):
        if line.startswith("#"):
            continue
        if line.startswith("@"):
            cur = line.split()[0][1:]
            first = float(line.split("first_bias_s=")[1].split()[0])
            acc = 0
            continue
        if cur != prn:
            continue
        i, d = line.split()
        acc += int(d)
        out.append((int(i), first + acc * 1e-13))
    return out


def write(name, text):
    with open(os.path.join(LOGS, name), "w") as f:
        f.write(text)


def freerun(n, tau0, q_wf, q_rw, y0=0.0, drift=0.0):
    x, y, out = 0.0, y0, []
    pxx = q_wf * tau0 + q_rw * tau0**3 / 3
    pxy = q_rw * tau0**2 / 2
    pyy = q_rw * tau0
    l11 = math.sqrt(pxx)
    l21 = pxy / l11
    l22 = math.sqrt(max(pyy - l21 * l21, 0.0))
    for _ in range(n):
        out.append(x)
        e1, e2 = R.gauss(0, 1), R.gauss(0, 1)
        x += y * tau0 + drift * tau0 * tau0 / 2 + l11 * e1
        y += drift * tau0 + l21 * e1 + l22 * e2
    return out


def main():
    os.makedirs(LOGS, exist_ok=True)
    # Real: IGS G03 and G25, the last 4 days of the file, TimeLab single column at 300 s.
    for prn, name in (("G03", "gm-fra-1.timelab"), ("G25", "ref-fra-0.timelab")):
        rows = igs(prn)[-1152:]
        body = "".join(f"{v:.16e}\n" for _, v in rows)
        write(name, f"# REAL public data: IGS final clock combination, GPS {prn} (rubidium), 2025-09-10 to 2025-09-14,\n"
                    f"# from tests/fixtures/timing_v5_igs/clocks.txt (see its NOTICE.md). A satellite clock standing in for a site reference.\n"
                    f"# tau0 = 300\n# units = s\n# data = phase\n" + body)
    # TICC timestamp mode, chA, 1 s ticks, 3 h: a rubidium against the counter's reference.
    x = freerun(10800, 1.0, 9e-22, 2.7e-25, y0=2e-11)
    lines = [f"# {SYN}", "# TICC timestamp mode, channel A"]
    for k, v in enumerate(x):
        t = 1000.0 + k + v + R.gauss(0, 60e-12)
        lines.append(f"{t:.12f} chA")
    write("gm-ldn-1.ticc", "\n".join(lines) + "\n")
    # ptp4l boundary clock servo, 16 s, 12 h, locked.
    lines = [f"# {SYN}"]
    t0 = 50000.0
    lines.append(f"ptp4l[{t0:.3f}]: master offset       -812 s0 freq   +2100 path delay       512")
    lines.append(f"ptp4l[{t0+16:.3f}]: master offset       -120 s1 freq   +2210 path delay       511")
    for k in range(2, 2700):
        off = round(R.gauss(0, 8))
        fr = 2200 + round(R.gauss(0, 3))
        pd = 510 + round(R.gauss(0, 1.5))
        lines.append(f"ptp4l[{t0 + 16*k:.3f}]: master offset {off:10d} s2 freq {fr:+7d} path delay {pd:9d}")
    write("bc-ldn-2.ptp4l.log", "\n".join(lines) + "\n")
    # chrony tracking.log, 64 s, 12 h.
    lines = [f"# {SYN}",
             "   Date (UTC) Time     IP Address   St   Freq ppm   Skew ppm     Offset L Co  Offset sd Rem. corr. Root delay Root disp. Max. error",
             "=" * 127]
    for k in range(675):
        t = START + datetime.timedelta(seconds=64 * k)
        off = R.gauss(0, 2e-6)
        lines.append(f"{t:%Y-%m-%d %H:%M:%S} C0A80001     2     -1.234      0.010 {off:10.3e} N  1  2.0e-07  -1.0e-07  1.0e-03  2.0e-04  5.0e-04")
    write("srv-ldn-3.tracking.log", "\n".join(lines) + "\n")
    # White Rabbit stat lines, 10 s, 2 h.
    lines = [f"# {SYN}"]
    sec0 = 1763078400 + 37
    lines.append(f"lnk:1 rx:10 tx:10 lock:0 sec:{sec0} nsec:0 cko:5400 crtt:184200 setp:9000 temp:44.8")
    for k in range(1, 720):
        lines.append(f"lnk:1 rx:{10+k} tx:{10+k} lock:1 sec:{sec0 + 10*k} nsec:0 cko:{round(R.gauss(0, 40))} crtt:{184200 + round(R.gauss(0, 20))} setp:9000 temp:{44.8 + R.gauss(0, 0.05):.2f}")
    write("wr-fra-2.wrstat.log", "\n".join(lines) + "\n")
    # ntpd loopstats, 64 s, 24 h.
    mjd = (START.date() - datetime.date(1858, 11, 17)).days
    lines = [f"# {SYN}"]
    for k in range(1350):
        s = 64 * k
        lines.append(f"{mjd + s // 86400} {s % 86400:.3f} {R.gauss(0, 5e-5):.9f} -12.345 0.000012 0.004 6")
    write("ntp-fra-4.loopstats", "\n".join(lines) + "\n")
    # Linux PPS assert, 1 s, 1 h: a host clock disciplined to the pulse.
    lines = [f"# {SYN}"]
    base = 1763078400
    for k in range(3600):
        off_ns = R.gauss(0, 300)
        sec = base + k
        if off_ns < 0:
            sec -= 1
            off_ns += 1e9
        lines.append(f"{sec}.{int(round(off_ns)):09d}#{k + 1}")
    write("pps-ldn-5.assert", "\n".join(lines) + "\n")
    # A small inline fleet for the registry and label tests.
    xs = freerun(400, 10.0, 2.5e-23, 7.5e-27, y0=1e-10)
    tl = "".join(f"{v*1e9:.6f}\n" for v in xs)
    pt = "".join(f"ptp4l[{1000 + 4*k}.000]: master offset {round(R.gauss(0, 20)):6d} s2 freq +1200 path delay 500\n" for k in range(200))
    with open(os.path.join(ROOT, "scenarios/timing-holdover-inline.toml"), "w") as f:
        f.write(f'''# Timing fleet with inline logs (pro-timing-holdover). {SYN}
kind = "pro-timing-holdover"
title = "Inline demo fleet"
fleet = "inline-demo"
window_h = 1
horizon_h = 4

[[limits]]
name = "PRTC-A style (user-set)"
value_s = 1.1e-6

[[limits]]
name = "Internal 100 ns (user-set)"
value_s = 100e-9

[[clocks]]
id = "osc-1"
vendor = "Vendor A"
role = "grandmaster"
[clocks.source]
format = "timelab"
text = """
# tau0 = 10
# units = ns
{tl}"""

[[clocks]]
id = "bc-1"
vendor = "Vendor B"
role = "boundary"
class = "ocxo"
[clocks.source]
format = "ptp4l"
text = """
{pt}"""
''')


if __name__ == "__main__":
    main()
