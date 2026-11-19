# Lunar time and lunar PNT evidence (Kshana Pro)

The Pro layer on top of the open engine's lunar modules: lunar time with a GUM uncertainty
budget (TCL or TL), the sensitivity to the undefined lunar equipotential `W0_L`, a "Circular T
for the Moon" processing chain, an Earth–Moon two-way time-transfer (TWTT) holdover calculator,
a broadcast encoding of the lunar-time-minus-UTC offset, and a LUNEX (lunar RINEX) prototype.
Code: `src/lunar_pro/`; tests: `tests/lunar_pro.rs` (and `tests/lunar_pro/*.rs`); CLI:
`kshana-pro lunar ...`; licence feature `pro-timing-holdover` (Pro edition). Decoding a
broadcast offset, writing its conformance vectors or the synthetic example, and validating a
LUNEX file need no licence.

The open engine (crate `kshana`, AGPL) is used through the dependency and never copied:
`lunar_time` (L_G, c², the Moon's velocity), `lunar_gauge` (the lunar surface potential),
`ephem` (analytic Moon and Sun), `ephem_provider` (JPL kernels), `timescales` (Julian dates,
leap seconds, two-part JD), `clock_specs` and `powerlaw` (clock noise), `holdover` (coast
variance), `realdata::clk` (the Circular T parser) and `rinex_obs` (the RINEX 3 parser).

**Evidence status: MODELLED.** Every output says so. Nothing here is validated against measured
lunar data, because none exists yet. Where a figure is compared with a published number, it is a
model-against-model cross-check, and is labelled as one. The broadcast offset encoding (KLTO-1)
is a **proposal pending LunaNet LNIS AD5, not a standard**. LUNEX is a **prototype proposal for
the IGS RINEX Committee, not a standard**.

Research behind it: `03-lunar-deep-space.md` (research of 19 Nov 2026), §2 and gaps 1, 2, 4, 5,
8 and 30.

## D1. Lunar time with GUM uncertainty, TCL or TL, W0 sensitivity (gaps 1, 8)

`kshana-pro lunar time --from <scale> --to <scale> --at <YYYY-MM-DDTHH:MM:SS>` converts an
instant between UTC, TAI, TT, TCG, TDB, TCB, TCL and TL and prints the budget of every term.
`kshana-pro lunar rates` prints the secular rates; `kshana-pro lunar w0` the `W0_L` table.

| Relation | Source | Uncertainty |
|---|---|---|
| TAI = TT − 32.184 s, UTC = TAI − ΔAT | definitions; open engine leap-second table | exact (a leap second announced after the table was built is not included) |
| TT from TCG, `L_G` | IAU 2000 B1.9 | exact |
| TDB − TT | USNO Circular 179 eq. 2.6 (seven terms) | rectangular, 10 µs half-width (the circular's stated accuracy) |
| TDB from TCB, `L_B`, `TDB0` | IAU 2006 B3 | exact |
| TCL from TCB, `L_CL` | LTE440 (default) **or** Kshana first principles (`--rate-source first-principles`) | LTE440: its ±1e-17 fit; first principles: ephemeris, averaging window and truncation terms (below) |
| Periodic TCL − TT | `−v_EMB·r/c²` (≈126 µs monthly) and the lunar eccentricity term (≈0.48 µs) from the open engine's analytic series | ≈0.12 µs (Sun's barycentric motion left out, series distance error vs JPL Horizons) and 0.25 µs half-width truncation |
| TL from TCL, `L_L = W0_L/c²` | the chosen `W0_L` candidate (`--w0`) | the candidate's own: Turyshev's 2e-15 knowledge of `L_L` for physical candidates; zero for `conventional:<value>` |

Every term is a named input; terms with the same name are the same input, so converting TCL to
TL cancels every TCL term and leaves only `W0_L`. The secular terms grow with time since the
1977 origin because a rate uncertainty does.

**First principles.** `L_CL = L_C + ΔL`, with `L_C` from the IERS Conventions 2010 and
`ΔL = [(GM_E − GM_M)⟨1/r⟩ + (μ_E − μ_M)(⟨v²⟩/2 + ⟨U_tide,⊙⟩)]/c²` averaged over the 18.6-year
nodal cycle of the open engine's analytic Moon. It gives `ΔL = 1.70931e-11` against LTE440's
`1.70935e-11` (a difference of 3.6e-16, 0.03 ns/day), inside its own uncertainty of about
0.43 ns/day, which the ephemeris term dominates (analytic series against the vendored JPL
Horizons vectors, about 3e-4 in distance). Give two JPL kernels with `--kernel a.bsp --kernel
b.bsp` (for example DE440 and DE421) and the ephemeris term becomes their measured difference.
The open engine exposes DE421 and DE440 only for the lunar orientation, which does not enter a
rate, so no DE421-vs-DE440 figure is claimed without kernels.

**Rates (checked by tests).**

| Rate | Kshana | Published | Test |
|---|---|---|---|
| TCL − TT | 58.7378 µs/day (u 0.0009 ns/day, LTE440) | OSTP 58.7; LTE440 `6.798355238e-10` | within ±0.05 of OSTP; LTE440's two statements agree through `L_B` to < 3e-17 |
| TCL − TT, first principles | 58.73782 µs/day (u 0.43 ns/day) | LTE440 | within 2u |
| TL − TT | 56.0259 µs/day (default `W0_L`) | Ashby and Patla 2024: 56.02 | within ±0.01 for every built-in `W0_L` candidate |
| TCL − TL | 2.7119 µs/day (default) | Turyshev: 2.712 (selenoid), 2.711 (pole) | exact for his candidate |
| TCG − TCL | 1.4769 µs/day | LTE440 (derived) | within 1e-3 |

The "≈56 µs/day" figure is the TL rate; TCL runs ≈58.74 µs/day ahead of TT. The 0.006 µs/day
residual between our TL rate and Ashby and Patla's 56.02 is a cross-model difference
(their degree-350 field and averaging against our LTE440 rate and degree-2 `W0_L`); the test's
±0.01 is one unit in their last quoted digit.

**W0 sensitivity.** `d(rate)/d(W0_L) = −9.613e-4 ns/day per m²/s²`, i.e. 1.56e-3 ns/day per
metre of lunar height (the open engine's topographic spread per metre agrees). The four built-in
candidates span about 1.8 ns/day (equivalent to about 1.1 km of height). An undefined `W0_L`
therefore costs ≈0.17–0.87 ns/day of TL rate, and since 1977 about 3 µs of TL − UTC (the
dominant term of the TL budget today). A conventional `W0_L` adopted by IAU/IAG removes it.

```text
kshana-pro lunar time --from utc --to tl --at 2026-11-19T00:00:00 --license <token>
UTC -> TL: TL - UTC = 70.204756028498 s  (u_c 3153.944 ns, U(k=2) 6307.888 ns)
```

## D2. "Circular T for the Moon" (gap 2)

`kshana-pro lunar circular --comparisons <csv> (--earth-csv <csv> | --circular-t <file>...)
--month YYYY-MM --out <dir> [--option tl|tcl] [--w0 conventional:<m2/s2>]` writes
`circular.txt` and `circular.json`. Try it with `kshana-pro lunar circular example --out <dir>`,
which writes SYNTHETIC comparisons, Earth series and the truth.

* **Comparisons** (`mjd_utc,lunar_clock,earth_lab,reading_s,u_a_ns,u_b_ns`): link-calibrated
  `TL(k) − UTC(k_E)` from relayed time transfer, with statistical and calibration uncertainty.
* **Earth laboratories**: `[UTC − UTC(k)]` from BIPM Circular T Section 1 (the open engine's
  parser) or a CSV; interpolated, never extrapolated.
* **Reduction**: `[TL − TL(k)] = (TL − UTC)_D1 − (TL(k) − UTC(k_E)) + [UTC − UTC(k_E)]`.
* **Estimate**: weighted straight line within ±2.5 days of each Circular T date (MJDs ending in
  4 and 9, 0h UTC); `uA` from the fit (χ²-scaled); `uB` from link calibration, the Earth
  laboratory's Circular T `u` and the D1 model `u`.
* **Publication**: Section 1 in the Circular T layout (the engine's parser reads it back, which a
  test checks), Section 2 with `TL(k) − UTC` in seconds, Section 3 with the model, ephemerides
  and uncertainties as CGPM 2026 Resolution 6 asks. Every table says it is a prototype and not a
  BIPM publication.

Tests: recovery of the synthetic truth within 4 uA at every date; coverage of 1.96 uA between
0.91 and 0.99 over 60 seeds; parser read-back; CSV and Circular T inputs agree; TCL option.

**Finding.** With today's model the D1 uncertainty, not the link, dominates `uB`: about 3 µs for
TL with a physical `W0_L` candidate and about 0.19 µs for TCL (the periodic terms). A lunar
Circular T at the nanosecond level needs a conventional `W0_L` and a numerically integrated
periodic series (LTE440-class) in place of the analytic one.

## D3. Earth–Moon TWTT holdover calculator (gap 4)

`kshana-pro lunar holdover --clock rafs --link-ns 1 --n 10 --cal-ns 0.5 --interval 3600
--budget-ns 10` gives the time error between contacts and the longest contact interval that
keeps `k·σ` inside the budget. Clocks: the open engine's lunar classes (`optical-master`,
`passive-h-maser`, `rafs`, `mini-rafs`) and clock classes (`tcxo`, `ocxo`, `csac`, `uso`,
`dsac`), IEEE 1139 coefficients (`--h h0,h-1,h-2`) or Allan points (`--adev tau:adev,...`, fitted
by the engine).

With `q_wf = h_0/2`, `q_rw = 2π² h_{-2}`, `σ_F² = 2 ln 2 h_{-1}` and `σ_L² = σ_link²/n`:

* phase only: `σ²(u) = σ_L² + q_wf u + q_rw u³/3 + σ_F² u²`;
* phase and frequency (frequency from the last two contacts): `σ²(u) = σ_L²[(1+u/T)² + (u/T)²]
  + q_wf(u + u²/T) + q_rw(u³/3 + u²T/3) + 2σ_F² u²`;

plus a constant calibration term. The worst point is `u = T`. Flicker FM uses the `τ·σ_y(τ)`
rule of thumb (an approximation). Tests: `σ(τ) = τσ_y(τ)` against the engine's Allan deviation
for each noise type, the named lunar clocks reproduce their cited one-day time error, the
required interval matches the white-FM and random-walk-FM closed forms, and the
phase-and-frequency variance matches a Monte Carlo simulation within 4 %.

## D4. KLTO-1: TL–UTC broadcast offset encoding (gap 5)

**A proposal pending LunaNet LNIS AD5, not a standard.** A 147-bit message (19 bytes): version,
scale (TCL/TL), reference (UTC or a UTC(k) index), issue of data, week and time of applicability
(16 s), integer seconds, `a0` (2^-32 s, 0.23 ns), `a1` (2^-50 s/s), `a2` (2^-64 s/s²), a validity
window (1 h to 7 days) and an uncertainty class (0.5 ns to 1 ms, "> 1 ms", "not monitored").
`offset(t) = n + a0 + a1·dt + a2·dt²`. Field widths follow the precedent of the GPS UTC parameters
only as a design reference.

* `kshana-pro lunar offset fit --option tl --centre 2026-11-19T12:00:00` fits a message to the
  D1 model over the window; the class bounds the model's expanded uncertainty plus the largest
  fit residual. The `a2` term is needed: over a week, dropping it costs microseconds (tested).
* `kshana-pro lunar offset decode <hex>` and `kshana-pro lunar offset vectors --out <file>`
  (free). The conformance vectors are committed at
  `tests/fixtures/lunar_pro/klto1_conformance_vectors.json`.
* Not yet encoded: a pending leap second (GPS-style `ΔtLSF`, `WN_LSF`, `DN` would be added); the
  fit refuses a window that contains one.

Tests: 5 000 random messages round-trip; quantisation stays inside half an LSB per coefficient
and inside the stated bound over the window; malformed and out-of-range input is refused; the
fitted message reproduces the D1 model inside its class bound; the vectors are self-consistent
and reproducible.

## D5. LUNEX prototype (gap 30)

**A prototype proposal for the IGS RINEX Committee, not a standard.** A LUNEX observation file
is syntactically a RINEX 3.05 observation file plus new header records, so existing RINEX
readers keep working on the body (the tests read every file with the open engine's RINEX 3
parser):

* `LUNEX VERSION / TYPE`, `LUNAR PLATFORM` (`SURFACE`, `LANDER`, `ROVER`, `ORBIT`),
  `LUNAR REF FRAME` (`MOON_PA_DE440`, `MOON_ME_DE421`, `MOON_PA_DE421`, `MOON_ME_DE440_ME421`,
  `MCI_ICRF`, `ILURF_PROTO`), `LUNAR TIME SCALE` (tag, `W0_L`, realisation code);
* time-system tags `GPS`, `GAL`, `UTC`, `TAI`, `TT` and the lunar `TCL`, `TL`;
* LunaNet AFS observables as system `L`, band `9` (2492.028 MHz), attributes `D` (data), `P`
  (pilot), `X` (both).

`kshana-pro lunar lunex convert --csv <in.csv> --out <file.lnx>` converts a LuGRE-style CSV
(`time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz`; modelled on the kind of
fields the LuGRE open dataset carries, not its exact schema). `kshana-pro lunar lunex validate
<file>` (free) checks the records, tags, frame, platform, epoch order and counts, interval and
plausible observable ranges (codes E001–E014, warning W101).

Tests: CSV to LUNEX to text round-trips byte for byte and value for value; the engine's parser
reads the same values; clean files (Earth GNSS and a TL-tagged AFS file) validate; ten injected
faults are each caught; bad CSV is refused.

## Honest limits

* No lunar clock, TWTT link or lunar observation data exists yet: everything is MODELLED.
* The TCL periodic series is analytic (≈0.2 µs); LTE440 claims picoseconds by numerical
  integration. A pre-registered cross-check against LTE440's kernels, Turyshev and
  Defraigne/Bourgoin (research note §6.1) is not done yet.
* The TCL origin convention (`TCL = TT` at T0 for the selenocentre/geocentre pair) is
  illustrative until confirmed against the IAU 2024 Resolution II text.
* KLTO-1 and LUNEX field choices are proposals; nothing is taken from LNIS AD5 (not public).
* Flicker FM in D3 uses the `τ·σ_y(τ)` rule of thumb, not an exact prediction variance.
