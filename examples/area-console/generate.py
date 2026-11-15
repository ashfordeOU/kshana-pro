#!/usr/bin/env python3
"""Write the two-site area console demo (roadmap 0.42 F1).

SYNTHETIC, a MODELLED analogue of published events: the timeline follows the shape of the
published Baltic jamming descriptions carried by the threat library
(threat-library/scenarios/baltic-gulf-of-finland-jamming.toml and
baltic-tartu-approach-jamming.toml): a wide-area jamming episode seen by both areas at once, a
local anomaly at one receiver, a stale Station and a feed-only detection. No recording was used;
no station, receiver or feed exists. The feeds stand for customer-supplied third-party data; the
Tartu feed carries emitter estimates on purpose, to show that the console withholds them (F2).

    python3 examples/area-console/generate.py
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
T0 = 1794614400  # 2026-11-14 00:00:00 UTC
STEP = 2
DUR = 3600
NOTE = "# SYNTHETIC: examples/area-console/generate.py; a MODELLED analogue, not a measurement.\n"


def m(minute):
    return T0 + int(minute * 60)


def states(name, spans, gaps=()):
    """spans: list of (from_min, to_min, state); default nominal."""
    lines = [NOTE, "t_s,state\n"]
    for t in range(T0, T0 + DUR, STEP):
        if any(m(a) <= t < m(b) for a, b in gaps):
            continue
        st = "nominal"
        for a, b, s in spans:
            if m(a) <= t < m(b):
                st = s
        lines.append(f"{t},{st}\n")
    with open(os.path.join(HERE, "sources", name), "w") as f:
        f.writelines(lines)


def feed(name, rows):
    lines = [NOTE, "t_start_s,t_end_s,kind,confidence,lat_deg,lon_deg\n"]
    for a, b, kind, c, pos in rows:
        lat, lon = pos if pos else ("", "")
        lines.append(f"{m(a)},{m(b)},{kind},{c},{lat},{lon}\n")
    with open(os.path.join(HERE, "sources", name), "w") as f:
        f.writelines(lines)


# Gulf of Finland approaches: three Stations and one ship receiver.
states("gof-st-1.csv", [(10, 25, "degraded"), (14, 20, "untrusted")])
states("gof-st-2.csv", [(10.5, 24, "degraded")])
states("gof-st-3.csv", [(11, 25, "degraded")], gaps=[(30, 35)])
states("gof-rx-1.csv", [(12, 22, "degraded"), (40, 45, "untrusted")])
feed("gof-feed.csv", [
    (11, 24, "jamming", 0.8, None),
    (55, 56, "interference", 0.3, None),
])
# Tartu approach: three Stations.
states("tartu-st-1.csv", [(10.5, 23, "degraded")])
states("tartu-st-2.csv", [(11, 22, "degraded")])
states("tartu-st-3.csv", [])
feed("tartu-feed.csv", [
    (11, 22, "jamming", 0.7, (58.38, 26.72)),
    (50, 52, "jamming", 0.6, (58.31, 26.95)),
])
