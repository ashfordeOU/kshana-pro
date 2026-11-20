#!/usr/bin/env python3
"""Writes the SYNTHETIC demo logs of docs/TIMING-ASSURANCE.md "Clock benchmarks (E33)".

Every log here is made up: white frequency noise at a chosen level, no real clock. Run from
this folder: python3 generate.py. Then `kshana-pro timing clock-bench register bench.toml`
prints the protocol hash that bench.toml records.
"""
import random

random.seed(33)


def log(name, n, adev1, offset=0.0):
    x = 0.0
    lines = ["# SYNTHETIC white FM, not a real clock", "# tau0 = 1", "# units = ns"]
    for _ in range(n):
        lines.append(f"{x * 1e9:.6f}")
        x += adev1 * random.gauss(0.0, 1.0) + offset
    with open(name, "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


log("unit-a-baseline.txt", 3000, 3e-10)
log("unit-a-temperature.txt", 3000, 3.5e-10, 1.5e-10)
log("unit-b-baseline.txt", 3000, 3e-11)
log("unit-b-temperature.txt", 3000, 3e-11, 2e-12)
log("unit-c-baseline.txt", 3000, 5e-14)
