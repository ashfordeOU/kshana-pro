#!/usr/bin/env python3
"""Write the multi-source trust demo table (roadmap 0.42 F3).

SYNTHETIC: one hour at 1 s of a site with OSNMA-authenticated Galileo, open GNSS, a LEO time
service (passes of 10 min every 30 min), PTP and eLoran, each a white-noise measurement of one
synthetic rubidium clock at its stated level, with three injected events: a re-broadcast time
push on both GNSS sources that OSNMA does not flag (minute 10, 2 ns/s), jamming (minutes 25-35)
and a PTP asymmetry step of +500 ns (minutes 30-50). random.Random(20261115). Not a measurement.

    python3 examples/multi-source/generate.py
"""
import math
import os
import random

HERE = os.path.dirname(os.path.abspath(__file__))
R = random.Random(20261115)
SIG = {"osnma": 15e-9, "gnss": 15e-9, "leo": 40e-9, "ptp": 50e-9, "eloran": 60e-9}
ORDER = ["osnma", "gnss", "leo", "ptp", "eloran"]

x, y = 0.0, 2e-11
rows = ["# SYNTHETIC: examples/multi-source/generate.py; not a measurement.\n",
        "t_s," + ",".join(f"{s}_status,{s}_offset_ns" for s in ORDER) + "\n"]
for t in range(3600):
    x += y + R.gauss(0.0, 3e-11)
    f = {s: 0.0 for s in ORDER}
    ok = {s: True for s in ORDER}
    if 600 <= t < 1500:
        f["gnss"] = f["osnma"] = 2e-9 * (t - 600)
    if 1500 <= t < 2100:
        ok["gnss"] = ok["osnma"] = False
    if 1800 <= t < 3000:
        f["ptp"] = 500e-9
    if (t - 300) % 1800 >= 600:
        ok["leo"] = False
    cells = []
    for s in ORDER:
        n = R.gauss(0.0, SIG[s])
        if ok[s]:
            cells.append(f"ok,{(x + n + f[s]) * 1e9:.3f}")
        else:
            cells.append("unavailable,")
    rows.append(f"{t}," + ",".join(cells) + "\n")
with open(os.path.join(HERE, "sources.csv"), "w") as fh:
    fh.writelines(rows)
