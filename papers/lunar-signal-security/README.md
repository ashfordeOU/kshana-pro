# Lunar AFS signal-security link-budget study (P1)

Academic manuscript: **"A Closed-Form Link-Budget Assessment of the Spoofing and
Jamming Attack Surface of the LunaNet Augmented Forward Signal."**

This is a neutral, results-first reframe of an earlier branded report titled
*"The Lunar Navigation Signal Is Unauthenticated: Sizing the Spoofing and Jamming
Attack Surface of the LunaNet Augmented Forward Signal,"* which arXiv moderation
declined. The declarative-thesis title and the branded-report form (cover +
MODELLED banner) read as a position/technical report rather than a neutral
research article. **Content and every number are unchanged; only the framing is
academic** — IEEEtran two-column, graded-strength contributions, per-term
Validated/Modelled provenance, and an explicit *Limitations and honesty boundary*
section, matching the four accepted arXiv papers in this program.

## Build

```
tectonic main.tex            # -> main.pdf (5 pp)
python3 figures/make_figures.py   # -> figures/fig1a_attacker_power.pdf
```

## Provenance (honesty firewall)

All numbers reproduce the **P1 baseline scenario**
`src/packs/lunar_signal_security_pack.rs`
(`LunarSignalSecurityScenario::default`), which composes the open
`kshana::linkbudget` / `kshana::jamming` closed forms:

- **Validated** (cross-checked closed form): received power −140.6 dBW, 15.6 dB
  deficit (12–18 dB swept band), attacker power vs standoff (0.022 mW @ 1 km,
  8.9 mW @ 20 km, 0.222 W @ 100 km spoof; 111 W @ 100 km jam), horizon reach
  (2 m → 5 km … 1000 m → 61 km), OSNMA field sizing (20 bit/s = 40% of a 50 bit/s
  budget).
- **Modelled** (representative scenario / threshold input): the AFS operating point
  (EIRP, slant, carrier, user antenna, nav-data rate), the 3 dB / 30 dB J/S
  thresholds, the six-attribute qualitative cells.
- The whole-hemisphere orbital reach is a **uniform-beam idealisation**; a real
  antenna pattern narrows it to a computed sub-hemispheric cap (noted in-text).

`figures/make_figures.py` prints the anchor cross-checks so a reviewer can confirm
the figure matches the engine by hand.

Do not commit built artifacts (`main.pdf`, `figures/*.pdf`) unless the release
process asks for them; they regenerate deterministically from source.
