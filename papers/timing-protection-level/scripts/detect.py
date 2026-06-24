#!/usr/bin/env python3
"""Run the TPL clock-aided CUSUM monitor on the REAL solved clock trajectory.

Holdover model = quadratic oscillator-drift fit on an early clean sub-segment.
The monitor standardises (GNSS time - holdover prediction) by the clean robust
sigma and runs the same one-sided CUSUM as kshana's tpl::Cusum. Before measuring
detection it VALIDATES no-false-alarm on a held-out clean window (fit on t<t_fit,
test on t_fit..t_clean). Then reports detection latency from spoof onset and the
realized protected time error versus the eventual sustained (captured) error.
"""
import csv, sys, math, statistics as st

clock_csv = sys.argv[1]
t_fit = float(sys.argv[2]) if len(sys.argv) > 2 else 8.0     # fit holdover on t<t_fit
t_clean = float(sys.argv[3]) if len(sys.argv) > 3 else 12.0  # clean up to here (held-out)
KREF = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
H = float(sys.argv[5]) if len(sys.argv) > 5 else 5.0

rows = [(float(r["t_min"]), float(r["clock_ns"])) for r in csv.DictReader(open(clock_csv))]


def polyfit2(pts):
    # quadratic least squares y = a t^2 + b t + c
    sx = [sum(t ** k for t, _ in pts) for k in range(5)]
    sy = [sum((t ** k) * y for t, y in pts) for k in range(3)]
    M = [[sx[2], sx[1], sx[0]], [sx[3], sx[2], sx[1]], [sx[4], sx[3], sx[2]]]
    v = [sy[0], sy[1], sy[2]]
    # solve 3x3
    A = [M[i][:] + [v[i]] for i in range(3)]
    for c in range(3):
        p = max(range(c, 3), key=lambda r: abs(A[r][c]))
        A[c], A[p] = A[p], A[c]
        piv = A[c][c]
        A[c] = [x / piv for x in A[c]]
        for r in range(3):
            if r != c:
                f = A[r][c]
                A[r] = [A[r][k] - f * A[c][k] for k in range(4)]
    return A[0][3], A[1][3], A[2][3]  # a,b,c  (for t^2,t,1) order: careful


fitpts = [(t, y) for t, y in rows if t < t_fit]
a, b, c = polyfit2(fitpts)
hold = lambda t: a * t * t + b * t + c

clean_fit = [y - hold(t) for t, y in fitpts]
med = st.median(clean_fit)
sigma = 1.4826 * st.median(abs(x - med) for x in clean_fit)
print(f"holdover quad fit (t<{t_fit:.0f}): {a:.1f} t^2 + {b:.1f} t + {c:.0f};  "
      f"robust sigma = {sigma:.0f} ns over {len(fitpts)} epochs")


def run_cusum(seq):
    s = 0.0
    for t, y in seq:
        z = (y - hold(t)) / sigma
        s = max(0.0, s + z - KREF)
        if s > H:
            return t, y - hold(t)
    return None, None


# VALIDATION: held-out clean window must not alarm
heldout = [(t, y) for t, y in rows if t_fit <= t < t_clean]
va_t, _ = run_cusum(heldout)
print(f"held-out clean window {t_fit:.0f}-{t_clean:.0f} min: "
      + ("FALSE ALARM at %.2f (model too tight)" % va_t if va_t else "no false alarm  [OK]"))

# spoof onset: first epoch where residual sustains > 5 sigma for >=10 epochs
run = 0
t_on = None
for t, y in rows:
    if t >= t_clean and abs(y - hold(t)) > 5 * sigma:
        run += 1
        if run >= 10:
            t_on = t
            break
    elif t >= t_clean:
        run = 0

# detection on full record (only count alarms at/after t_clean)
al_t, al_r = run_cusum([(t, y) for t, y in rows if t >= t_fit])
# sustained captured error = median over final 2 minutes
tmax = rows[-1][0]
finalw = [y - hold(t) for t, y in rows if t > tmax - 2]
sustained = st.median(finalw)

print(f"spoof onset (residual >5sigma sustained, t>{t_clean:.0f}): "
      + (f"{t_on:.2f} min" if t_on else "not found"))
if al_t is not None and t_on is not None:
    lat = (al_t - t_on) * 60.0
    print(f"CUSUM ALARM (kref={KREF}, h={H}) at t={al_t:.2f} min")
    print(f"  detection latency from onset      : {lat:.1f} s")
    print(f"  realized protected time error     : {abs(al_r):.0f} ns")
    print(f"  sustained UNCONTROLLED (captured) : {abs(sustained):.0f} ns "
          f"({abs(sustained)/1000:.2f} us)")
    print(f"  protection factor (uncontrolled/protected) : "
          f"{abs(sustained)/abs(al_r):.0f}x")
