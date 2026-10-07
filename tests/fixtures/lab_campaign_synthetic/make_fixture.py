"""Regenerate the synthetic lab-campaign fixture `basic/` from REAL engine output.

The fixture is the output of the engine's own `kshana iq campaign` on Kshana-generated scenes.
Nothing here is a lab recording or a measurement of any receiver.

The two recordings are built from an `iq scene --no-noise` signal, scaled by a stated C/N0
profile, plus seeded complex Gaussian noise of unit power. This is a signal-power profile on the
legitimate signal, not an interference waveform; nothing synthesises a jammer or spoofer.

- `synthetic-ramp`: 2 GPS L1 C/A satellites. The lab "states" a broadband event whose J/S steps
  from 20 dB to 30 dB. The scene's C/N0 follows the engine's analytic value for that J/S
  (`(C/N0)eff = [1/(C/N0) + (J/S)/(Q*Rc)]^-1`, Q = 1, Rc = 1.023 MHz).
- `synthetic-gap`: 1 satellite; the signal is absent for 1.5 s (an outage).

Usage (needs numpy, and an engine built from the campaign branch named in README.md):

    KSHANA=/path/to/kshana python3 make_fixture.py <scratch-dir>

Only campaign.json, cells/, scorecard.json and DIGEST are copied into basic/. Every path the
engine records is relative to the scratch directory, so no local path enters the repository.
"""
import json
import os
import shutil
import subprocess
import sys

import numpy as np

KSHANA = os.environ["KSHANA"]
WORK = os.path.abspath(sys.argv[1])
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "basic")
FS = 4.092e6
NOMINAL = 45.0
RC = 1.023e6
Q = 1.0  # the engine's broadband Q today (jamming::q_factor)


def run(*args):
    subprocess.run([KSHANA, *args], cwd=WORK, check=True, stdout=subprocess.DEVNULL)


def eff_cn0_db(js_db):
    cn0 = 10 ** (NOMINAL / 10)
    js = 10 ** (js_db / 10)
    return 10 * np.log10(1.0 / (1.0 / cn0 + js / (Q * RC)))


def recording(name, duration, prns, dopplers, profile, seed):
    """Write <name>.bin (ci16_le) + <name>.bin.json sidecar + <name>.truth.csv."""
    sig = f"{name}.sig.cf32"
    run("iq", "scene", sig, "--rate", str(FS), "--duration", str(duration),
        "--signal", "gps-l1ca", "--prn", ",".join(map(str, prns)),
        "--doppler", ",".join(map(str, dopplers)), "--cn0", str(NOMINAL),
        "--no-noise", "--seed", str(seed), "--truth", f"{name}.truth.csv")
    s = np.fromfile(os.path.join(WORK, sig), dtype=np.complex64).astype(np.complex128)
    t = np.arange(len(s)) / FS
    gain_db = np.array([profile(x) for x in t[:: int(FS // 1000)]])  # 1 ms knots
    g = np.repeat(10 ** (gain_db / 20), int(FS // 1000))[: len(s)]
    rng = np.random.default_rng(seed)
    n = (rng.standard_normal(len(s)) + 1j * rng.standard_normal(len(s))) / np.sqrt(2)
    x = s * g + n
    iq = np.empty(2 * len(x), dtype="<i2")
    iq[0::2] = np.clip(np.round(x.real * 4096), -32768, 32767)
    iq[1::2] = np.clip(np.round(x.imag * 4096), -32768, 32767)
    iq.tofile(os.path.join(WORK, f"{name}.bin"))
    with open(os.path.join(WORK, f"{name}.bin.json"), "w") as f:
        json.dump({"format": "ci16_le", "sample_rate_hz": FS, "center_hz": 1575.42e6,
                   "description": "synthetic: Kshana scene signal x C/N0 profile + unit noise"}, f)
    os.remove(os.path.join(WORK, sig))
    for extra in (sig + ".json",):
        p = os.path.join(WORK, extra)
        if os.path.exists(p):
            os.remove(p)


def ramp_profile(t):
    """C/N0 relative to nominal (dB) for the stated J/S steps; amplitude gain = this/2 in dB."""
    if 4.0 <= t < 8.0:
        return eff_cn0_db(20.0) - NOMINAL
    if 8.0 <= t < 12.0:
        return eff_cn0_db(30.0) - NOMINAL
    return 0.0


def gap_profile(t):
    return -300.0 if 4.0 <= t < 5.5 else 0.0


def write(rel, text):
    p = os.path.join(WORK, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w") as f:
        f.write(text)


def main():
    shutil.rmtree(WORK, ignore_errors=True)
    os.makedirs(WORK)
    recording("synthetic-ramp", 16.0, [3, 11], [1250.0, -2300.0], ramp_profile, 21)
    recording("synthetic-gap", 10.0, [7], [2300.0], gap_profile, 22)
    write("conds/ramp.toml", """schema = "kshana.test-conditions/1"
[recording]
id = "synthetic-ramp"
path = "../synthetic-ramp.bin"
settle_s = 1.0
[receiver]
dut = "synthetic scene (no receiver)"
front_end = "ideal"
[[expected]]
signal = "gps-l1ca"
ids = [3, 11]
nominal_cn0_dbhz = 45.0
truth = "../synthetic-ramp.truth.csv"
[[event]]
id = "ramp-1"
kind = "interference"
type = "broadband"
onset_s = 4.0
offset_s = 12.0
[event.power]
quantity = "js_db"
interpolation = "step"
points = [[4.0, 20.0], [8.0, 30.0]]
""")
    write("conds/gap.toml", """schema = "kshana.test-conditions/1"
[recording]
id = "synthetic-gap"
path = "../synthetic-gap.bin"
settle_s = 1.0
[receiver]
dut = "synthetic scene (no receiver)"
[[expected]]
signal = "gps-l1ca"
ids = [7]
[[event]]
id = "gap-1"
kind = "outage"
type = "unknown"
onset_s = 4.0
offset_s = 5.5
""")
    write("designs.toml", """schema = "kshana.loop-design/1"
[[design]]
name = "baseline"
[[design]]
name = "narrow-pll"
extends = "baseline"
[design.carrier]
pll_bw_hz = 8.0
[design.code]
bw_hz = 1.0
""")
    write("campaign.toml", """schema = "kshana.campaign/1"
name = "synthetic-fixture"
[inputs]
conditions = ["conds/*.toml"]
designs = "designs.toml"
[[frontend]]
name = "raw"
[[frontend]]
name = "agc-3bit"
bits = 3
[scoring]
baseline_window_s = 2.0
js_bin_db = 5.0
[scoring.bars]
max_reacq_s = 3.0
min_availability = 0.5
""")
    run("iq", "campaign", "campaign.toml", "--out", "out", "--workers", "4")
    shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(os.path.join(OUT, "cells"))
    for f in ("campaign.json", "scorecard.json", "DIGEST"):
        shutil.copy(os.path.join(WORK, "out", f), os.path.join(OUT, f))
    for f in sorted(os.listdir(os.path.join(WORK, "out", "cells"))):
        shutil.copy(os.path.join(WORK, "out", "cells", f), os.path.join(OUT, "cells", f))
    # No local path may enter the repository.
    for root, _, files in os.walk(OUT):
        for f in files:
            text = open(os.path.join(root, f), encoding="utf-8").read()
            assert WORK not in text and "/tmp/" not in text, f"local path in {f}"


if __name__ == "__main__":
    main()
