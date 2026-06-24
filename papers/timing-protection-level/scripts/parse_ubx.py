#!/usr/bin/env python3
"""Minimal UBX reader: extract NAV-CLOCK and NAV-PVT time series from a .ubx file.

NAV-CLOCK (0x01 0x22, len 20): iTOW(U4 ms) clkB(I4 ns) clkD(I4 ns/s) tAcc(U4 ns) fAcc(U4 ps)
NAV-PVT   (0x01 0x07, len 92): iTOW(U4) ... tAcc(U4 ns) nano(I4) fixType(U1) flags(U1)
                               flags2(U1) numSV(U1) lon(I4 1e-7 deg) lat(I4) height(I4 mm)
                               hMSL(I4) hAcc(U4 mm) vAcc(U4) ...
Outputs two CSVs next to the input. Pure stdlib.
"""
import struct
import sys
import os

SYNC = b"\xb5\x62"


def _ck(payload):
    a = b = 0
    for x in payload:
        a = (a + x) & 0xFF
        b = (b + a) & 0xFF
    return a, b


def iter_messages(buf):
    i = 0
    n = len(buf)
    while i + 8 <= n:
        if buf[i] != 0xB5 or buf[i + 1] != 0x62:
            i += 1
            continue
        cls = buf[i + 2]
        mid = buf[i + 3]
        length = buf[i + 4] | (buf[i + 5] << 8)
        end = i + 6 + length + 2
        if end > n:
            break
        body = buf[i + 2 : i + 6 + length]  # cls,id,len,payload for checksum
        ca, cb = buf[i + 6 + length], buf[i + 7 + length]
        a, b = _ck(body)
        if (a, b) != (ca, cb):
            i += 1
            continue
        payload = buf[i + 6 : i + 6 + length]
        yield cls, mid, payload
        i = end


def parse(path):
    with open(path, "rb") as f:
        buf = f.read()
    clock = []  # (itow_ms, clkB_ns, clkD_nsps, tAcc_ns, fAcc_ps)
    pvt = []  # (itow_ms, tAcc_ns, nano_ns, fixType, numSV, lat_deg, lon_deg, h_m)
    for cls, mid, p in iter_messages(buf):
        if cls == 0x01 and mid == 0x22 and len(p) >= 20:
            itow, clkB, clkD, tAcc, fAcc = struct.unpack_from("<IiiII", p, 0)
            clock.append((itow, clkB, clkD, tAcc, fAcc))
        elif cls == 0x01 and mid == 0x07 and len(p) >= 92:
            itow = struct.unpack_from("<I", p, 0)[0]
            tAcc = struct.unpack_from("<I", p, 12)[0]
            nano = struct.unpack_from("<i", p, 16)[0]
            fixType = p[20]
            numSV = p[23]
            lon = struct.unpack_from("<i", p, 24)[0] * 1e-7
            lat = struct.unpack_from("<i", p, 28)[0] * 1e-7
            height = struct.unpack_from("<i", p, 32)[0] / 1000.0
            pvt.append((itow, tAcc, nano, fixType, numSV, lat, lon, height))
    return clock, pvt


def write_csv(rows, header, path):
    with open(path, "w") as f:
        f.write(",".join(header) + "\n")
        for r in rows:
            f.write(",".join(str(x) for x in r) + "\n")


def main():
    if len(sys.argv) < 2:
        print("usage: parse_ubx.py <file.ubx> [outdir]")
        sys.exit(1)
    path = sys.argv[1]
    outdir = sys.argv[2] if len(sys.argv) > 2 else os.path.dirname(path) or "."
    base = os.path.splitext(os.path.basename(path))[0]
    clock, pvt = parse(path)
    print(f"{path}: NAV-CLOCK={len(clock)} NAV-PVT={len(pvt)}")
    if clock:
        write_csv(
            clock,
            ["itow_ms", "clkB_ns", "clkD_nsps", "tAcc_ns", "fAcc_ps"],
            os.path.join(outdir, base + ".navclock.csv"),
        )
    if pvt:
        write_csv(
            pvt,
            ["itow_ms", "tAcc_ns", "nano_ns", "fixType", "numSV", "lat_deg", "lon_deg", "h_m"],
            os.path.join(outdir, base + ".navpvt.csv"),
        )


if __name__ == "__main__":
    main()
