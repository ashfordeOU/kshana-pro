# Writes two INVENTED binary logs for the browser checks of the RTCM 3 and Septentrio SBF readers: rtcm3-msm4.bin and sbf-synthetic.sbf.
# Both are laid out as the engine's own tests lay out their synthetic frames (kshana src/receiver_trust/machines/rtcm3.rs tests, tests/receiver_trust_sbf.rs);
# neither is a capture from a real receiver. In both, the carrier-to-noise density falls 14 dB for a stretch so the monitors have something to see.
# Run: python3 rtcm_sbf_mk.py OUTDIR
import os, struct, sys
out = sys.argv[1]; os.makedirs(out, exist_ok=True)

def crc24q(b):
    c = 0
    for x in b:
        c ^= x << 16
        for _ in range(8):
            c <<= 1
            if c & 0x1000000: c ^= 0x1864CFB
    return c & 0xFFFFFF

class W:
    def __init__(s): s.bits = []
    def put(s, v, n): s.bits += [(v >> i) & 1 for i in range(n - 1, -1, -1)]
    def bytes(s):
        b = s.bits + [0] * (-len(s.bits) % 8)
        return bytes(int("".join(map(str, b[i:i + 8])), 2) for i in range(0, len(b), 8))

def msm4(tow_ms, sats):  # GPS 1074, one signal (id 2, L1 C/A) per satellite; sats = [(prn, cnr dB-Hz int)]
    w = W(); w.put(1074, 12); w.put(7, 12); w.put(tow_ms, 30); w.put(0, 19)
    mask = 0
    for prn, _ in sats: mask |= 1 << (64 - prn)
    w.put(mask, 64); w.put(1 << (32 - 2), 32)
    for _ in sats: w.put(1, 1)
    for _ in sats: w.put(70, 8)
    for _ in sats: w.put(512, 10)
    for bits in (15, 22, 4, 1):
        for _ in sats: w.put(0, bits)
    for _, c in sats: w.put(c, 6)
    p = w.bytes(); f = bytes([0xD3, len(p) >> 8, len(p) & 255]) + p
    c = crc24q(f); return f + bytes([c >> 16, (c >> 8) & 255, c & 255])

with open(os.path.join(out, "rtcm3-msm4.bin"), "wb") as fh:
    for t in range(150):
        drop = 14 if 70 <= t <= 100 else 0
        fh.write(msm4(100_000 + t * 1000, [(prn, 44 - prn % 3 - drop) for prn in range(1, 9)]))

def crc16(b):
    c = 0
    for x in b:
        c ^= x << 8
        for _ in range(8):
            c = ((c << 1) ^ 0x1021) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
    return c

def block(bid, tow_ms, wnc, body, rev=0):
    d = bytearray(2) + struct.pack("<H", bid | (rev << 13)) + b"\0\0" + struct.pack("<IH", tow_ms, wnc) + body
    while (len(d) + 2) % 4: d.append(0)
    d[4:6] = struct.pack("<H", len(d) + 2)
    d[0:2] = struct.pack("<H", crc16(bytes(d[2:])))
    return b"$@" + bytes(d)

def pvt(tow, wnc, k):
    b = bytearray(83 - 12); b[0] = 4
    struct.pack_into("<d", b, 2, 0.4 * 0 + 1.2043 + k * 1e-8); struct.pack_into("<d", b, 10, 0.2793); struct.pack_into("<d", b, 18, 12.5); b[60] = 8
    return block(4007, tow, wnc, bytes(b))

def meas(tow, wnc, cn0):
    raw = int((cn0 - 10.0) / 0.25); b = bytearray([6, 20, 0, 0, 0])
    for svid in range(1, 7):
        sb = bytearray(20); sb[1] = 0; sb[2] = svid; sb[15] = raw; b += sb
    return block(4027, tow, wnc, bytes(b))

with open(os.path.join(out, "sbf-synthetic.sbf"), "wb") as fh:
    for t in range(150):
        tow = 200_000 + t * 1000
        fh.write(pvt(tow, 2300, t)); fh.write(meas(tow, 2300, 45.0 - (14 if 80 <= t <= 110 else 0)))
