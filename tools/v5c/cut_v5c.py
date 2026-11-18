"""V5c D-R3 cutter: the extract of scripts/fetch_igs_clocks.sh (open engine) and the decimation
and layout of tests/fixtures/clock_state_igs_holdout_oracle/generate.py, re-implemented without
numpy, scipy or allantools. The header noise values (R, q_wf, q_rw) of that cutter need
allantools and scipy; they are not read by the predictor, so they are written as 0 here.
Usage: python3 -I cut_v5c.py <dir with the 14 .CLK.gz> <out clocks.txt>"""
import gzip, hashlib, os, sys

PRNS = "G03 G06 G08 G09 G10 G24 G25 G26 G27 G30 G32".split()
STEP = 300
UNIT = 1e-13
DOY0 = 278

def main():
    src, out_path = sys.argv[1], sys.argv[2]
    lines = []
    for doy in range(DOY0, DOY0 + 14):
        f = os.path.join(src, f"IGS0OPSFIN_2025{doy}0000_01D_30S_CLK.CLK.gz")
        day = doy - DOY0
        with gzip.open(f, "rt", encoding="ascii", errors="replace") as fh:
            for ln in fh:
                p = ln.split()
                if len(p) >= 10 and p[0] == "AS" and p[1] in PRNS:
                    s = day * 86400 + int(p[5]) * 3600 + int(p[6]) * 60 + float(p[7])
                    lines.append(f"{p[1]} {int(s)} {p[9]}\n")
    raw = "".join(lines).encode()
    series = {p: {} for p in PRNS}
    for ln in raw.decode().splitlines():
        prn, t, b = ln.split()
        t = int(t)
        if prn in series and t % STEP == 0:
            series[prn][t] = float(b)
    out = [f"# IGS final clocks, GPS IIF, decimated to {STEP} s; source extract sha256 "
           f"{hashlib.sha256(raw).hexdigest()}; V5c cutter (no fitted header noise values)\n"]
    for prn in PRNS:
        s = series[prn]
        ts = sorted(s)
        if not ts:
            print(f"{prn}: missing from the files")
            continue
        b0 = s[ts[0]]
        out.append(f"@{prn} R=0.000000e+00 q_wf=0.000000e+00 q_rw=0.000000e+00 first_bias_s={b0!r}\n")
        prev = 0
        for t in ts:
            v = round((s[t] - b0) / UNIT)
            out.append(f"{t // STEP} {v - prev}\n")
            prev = v
        gaps = 4032 - len(ts)
        print(f"{prn}: n={len(ts)} missing epochs={gaps}")
    open(out_path, "w").write("".join(out))

if __name__ == "__main__":
    main()
