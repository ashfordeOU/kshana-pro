## [0.29.0] - 2026-09-30

Twelve new scenario kinds and three new ways to read a run. The engine now covers the
radio spectrum, the whole solar system, constellations around any body, campaigns that
chain scenarios into one mission, and positioning, navigation and timing (PNT) from low
Earth orbit (LEO) from the signal to the fix. Every run can also be written as an
animation, a full report and a set of interoperability files for other tools.

| | 0.28.0 | 0.29.0 |
| --- | --- | --- |
| Scenario kinds | 63 | 75 |
| Scenario files (plus one suite manifest) | 77 | 138, of which 132 are bundled for `kshana example` |
| Verification-matrix rows | 174 | 223 |
| of which VALIDATED against an external oracle | 66 | 83 |
| of which MODELLED (stated model, checked for internal consistency) | 104 | 136 |
| of which PARTNER (needs a partner's hardware or data) | 4 | 4 |

Where the growth comes from:

| Area | Kinds | Scenario files | Matrix rows (validated + modelled) |
| --- | --- | --- | --- |
| Spectrum | 1 | 1 | 3 (1 + 2) |
| Solar system and positioning around any body | 2 | 3 | 6 (2 + 4) |
| Constellation design | 1 | 3 | 3 (2 + 1) |
| Campaigns | 1 | 5 | 3 (0 + 3) |
| Maritime, road and rail scenarios | 0 | 5 | 0 |
| Animation, reports, interoperability exports | 0 | 0 | 0 |
| LEO signal designs and the multi-band spectrum | 1 | 4 | 4 (2 + 2) |
| LEO pass and link budget | 1 | 6 | 9 (6 + 3) |
| LEO navigation message | 1 | 5 | 9 (3 + 6) |
| LEO positioning, timing and fusion | 3 | 8 | 8 (1 + 7) |
| LEO end-to-end chain | 1 | 3 | 1 (0 + 1) |
| LEO resilience, focus-area and end-user scenarios | 0 | 18 | 3 (0 + 3) |
| **Total added** | **12** | **61** | **49 (17 + 32)** |

The LEO capability is system-agnostic: it runs on generic, stated parameters and needs no
named system. Named systems are optional presets, each in its own file with its public
source. One preset, Celeste IOD (in-orbit demonstration), uses figures presented at the ESA
NAVISP LEO-PNT workshop, 2026 (ESA is the European Space Agency; NAVISP is its Navigation
Innovation and Support Programme); it lives in one file, `src/celeste_iod.rs`, with its five
repository-only scenarios, so it can be withheld without a source edit, and none of its
workshop figures is repeated in this changelog.

**Read "Revisions to published numbers" before upgrading.** One engine output changes (the
`ephemeris` kind's inertial-frame columns, after a sign fix in the nutation matrix), and
the documentation audit corrected figures that earlier documents had printed.

<details>
<summary><b>Abbreviations used in this entry</b></summary>

Statuses: **VALIDATED** means checked against an independent external oracle (a published
table, another group's software, or measured data); **MODELLED** means a stated model
checked for internal consistency only; **PARTNER** means the check needs a partner's
hardware or data.

| Abbreviation | Meaning |
| --- | --- |
| 3GPP, 5G, NTN | 3rd Generation Partnership Project; fifth-generation mobile network; non-terrestrial network |
| AD, BC | calendar eras (anno Domini, before Christ) |
| AIAA | American Institute of Aeronautics and Astronautics |
| AltBOC, BOC, BPSK, MBOC | alternative binary offset carrier; binary offset carrier; binary phase-shift keying; multiplexed binary offset carrier (signal modulations) |
| ATOMIC | Autonomous Time and Orbit Determination for Microsatellite Constellations (a published LEO ephemeris model) |
| C/A, L1, L2, L2C, L5 | coarse/acquisition code and the carrier and civil-signal names of the Global Positioning System |
| C/N0 | carrier-to-noise density ratio |
| CIO | Celestial Intermediate Origin |
| CLI | command-line interface |
| CRC-24Q | 24-bit cyclic redundancy check (Qualcomm polynomial) |
| CSAC | chip-scale atomic clock |
| CSS, HTML, SVG, XML | Cascading Style Sheets; HyperText Markup Language; Scalable Vector Graphics; Extensible Markup Language |
| CSV, JSON, PDF, TOML, URL | comma-separated values; JavaScript Object Notation; Portable Document Format; Tom's Obvious Minimal Language; uniform resource locator |
| CW | continuous wave |
| CZML, KML, GeoJSON | Cesium Language; Keyhole Markup Language; Geographic JSON (map and globe formats) |
| DE441 | Development Ephemeris 441 of the Jet Propulsion Laboratory |
| DOI | digital object identifier |
| DOP, GDOP, PDOP, HDOP, VDOP | dilution of precision: geometric, position, horizontal, vertical |
| E1, E5, E5a, E5b | Galileo signal names |
| E3F | a slot name in the Global Positioning System constellation table |
| ECEF | Earth-centred, Earth-fixed |
| EE | equation of the equinoxes |
| EGM2008 | Earth Gravitational Model 2008 |
| EIRP | equivalent isotropically radiated power |
| ERFA, SOFA | Essential Routines for Fundamental Astronomy; Standards of Fundamental Astronomy (reference astronomy libraries) |
| ESA, NAVISP | European Space Agency; its Navigation Innovation and Support Programme |
| FDMA | frequency-division multiple access |
| GCRF, GCRS | Geocentric Celestial Reference Frame; Geocentric Celestial Reference System |
| GLONASS | Russia's Global Navigation Satellite System |
| GNSS, GPS | global navigation satellite system; Global Positioning System |
| IAU | International Astronomical Union |
| ICD, OS SIS ICD, OS SDD | interface control document; Galileo Open Service Signal-In-Space Interface Control Document; Galileo Open Service Service Definition Document |
| ICRF | International Celestial Reference Frame |
| IMO | International Maritime Organization |
| INS, MEMS | inertial navigation system; micro-electro-mechanical system |
| IOD | in-orbit demonstration |
| IQ | in-phase and quadrature |
| IS-GPS-200 | the interface specification of the Global Positioning System |
| ISS | International Space Station |
| ITRF, ITRS, PEF, TEME | International Terrestrial Reference Frame; International Terrestrial Reference System; pseudo-Earth-fixed; true equator, mean equinox (reference frames) |
| ITU, ITU-R | International Telecommunication Union; its Radiocommunication Sector |
| J/S | jammer-to-signal ratio |
| J2 to J6, J2000 | zonal harmonics of the gravity field; the epoch 2000-01-01 12:00 |
| JPL | Jet Propulsion Laboratory |
| LEO, MEO | low Earth orbit; medium Earth orbit |
| MBSE | model-based systems engineering |
| NEES | normalised estimation error squared |
| NeQuick-G | the Galileo ionosphere model |
| OEM, SP3, TLE | Orbit Ephemeris Message; Standard Product 3 (precise orbits); two-line element set |
| PNT, PPP | positioning, navigation and timing; precise point positioning |
| PSD, SSC | power spectral density; spectral separation coefficient |
| RAIM, SQM | receiver autonomous integrity monitoring; signal quality monitoring |
| RINEX, RTCM | Receiver Independent Exchange Format; Radio Technical Commission for Maritime Services |
| RMS | root mean square |
| RTKLIB | an open-source GNSS positioning library, used here as an oracle |
| SGP4 | Simplified General Perturbations 4 (the orbit model of two-line element sets) |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SigMF | Signal Metadata Format |
| SISRE | signal-in-space range error |
| SPS PS | Standard Positioning Service Performance Standard |
| SRTM | Shuttle Radar Topography Mission |
| STK | Systems Tool Kit (its `.e` ephemeris file format) |
| STL | Satellite Time and Location |
| SVID | space vehicle identifier |
| T/P/F | a Walker constellation's total satellites, planes and phasing |
| TEC | total electron content |
| UHF | ultra high frequency |
| UTC | Coordinated Universal Time |
| WGS 84 | World Geodetic System 1984 |
| X1, X5 | signal names of the Xona Pulsar system |

</details>

### Revisions to published numbers

A changed published number is a revision and is recorded, never corrected silently. This
release carries one revision to engine output and several to figures printed in the
documentation.

#### Engine output

- **The TEME→GCRS reduction turned by 2·Δψ: the nutation matrix had the sign of Δψ
  reversed.** `nutation::numat` built `Rx(−(ε̄+Δε))·Rz(Δψ)·Rx(ε̄)` where SOFA's `iauNumat`
  builds `Rx(−(ε̄+Δε))·Rz(−Δψ)·Rx(ε̄)`, so `nutation_matrix`, `nutation_matrix_2000a`,
  `teme_to_gcrs` and `gcrs_to_teme` rotated by twice the nutation in longitude, about
  25 arcseconds. Every check on the chain was a property test (proper rotation, round
  trip, a non-zero nutation contribution) that a sign slip passes; the end-to-end
  Vallado test covered TEME→PEF, TEME→ITRF and GCRS→ITRS but not TEME→GCRS. On the
  Vallado example (AIAA 2006-6753, 2004-04-06) the chain missed the published GCRF by
  1 145 m; it now lands 0.11 m from it, and the matrix equals ERFA's `numat` to 1e-14.
  `tests/frame_reference_vectors.rs::teme_to_gcrs_matches_vallado_gcrf` and
  `nutation::tests::nutation_matrix_matches_erfa_numat` both fail on the old sign. The
  CIO (Celestial Intermediate Origin) consistency test in `src/cio.rs` had put the
  resulting ~130 m disagreement down to "≈ 2·EE"; the residual is now 5 cm and its bound
  is tightened from 250 m to 1 m.
  **This moves published numbers, recorded here as a revision:** the `ephemeris` kind's
  `gcrs_r_m` and `gcrs_v_m_s` columns (about 0.8–1.0 km for the bundled ISS (International
  Space Station) scenario; its TEME, Earth-fixed, ground-track, look-angle and Doppler
  columns do not change), and `Propagator::position_in_frame` / `state_gcrs` for the GCRS
  and ITRS frames. No bundled scenario's summary line, chart or golden hash reads these
  columns. The new interoperability exports use the corrected chain: the first
  `orbit-sgp4-gps` satellite's CZML position agrees with an ERFA reduction of the same TLE
  (propagated by the `sgp4` Python package) to 2.3 cm.

#### Figures printed in the documentation

The documentation was audited word by word against runs of this release's engine. Where a
printed figure did not match the run, the document was corrected, and each correction is
listed here, old → new. In every case below the engine output did not change; the document
had misread, mis-rounded or outlived it.

- **Documentation audit of the spectrum, constellation, campaign, animation, report and
  interoperability pages, and a new `docs/SOLAR-SYSTEM.md`.** Every figure was re-run with
  this engine. Two published numbers move:
  `docs/CONSTELLATION-DESIGN.md` gave the 5 000-satellite coverage test as "about 0.13 s in
  a debug build"; that did not reproduce (1.24 s on a loaded laptop), so the page now gives
  the prefilter share the test prints (9.9 % of 19 440 000 pair tests) and the release
  binary's 0.14 s for the bundled `leo-pnt-mega-shell`. The same page gave E3F's crossing
  as "off by 0.050 deg"; no test prints that figure, so the page now states only the
  0.06 deg bar the test enforces and the 7.30 against 7.36 deg derivation behind it. No
  engine output changes.
- **LEO-PNT documents: revised published figures (re-run on this release's engine).**
  `docs/LEO-SIGNAL.md` band trade: the first-order ionospheric delay of `generic-s` is
  3.24 m (was printed 3.25 m) and of `generic-c-wide` 0.77 m (was 0.78 m).
  `docs/LEO-NAVMSG.md` encode and decode: the decoded message's SISRE was printed
  0.119 cm where the run gave 0.122 cm; the page now prints 0.129 cm, the figure after the
  platform-independence fix under Fixed below. `docs/LEO-PASS.md` LEO-versus-GNSS pass:
  32 dB less free-space loss at the pass peak (was "26 to 32 dB"). For the signal and pass
  figures the engine output did not change; the documents had misread or mis-rounded it.
- **Tutorials, the worked pipeline example and the `/kshana-run` command, audited
  against this release's engine.** Revisions to published figures, old → new: the kinds that
  write `<scenario>.table.csv` are six, not four (`leo-navmsg` and `telecom-timing`
  were missing; `docs/tutorials/README.md`, `commands/kshana-run.md`); the cold-atom
  ½bT² crossing in Tutorial 3 is 18,443 s, not 18,440 s; the `oem-interop` round-trip
  error in `docs/examples/multi-tool-pipeline.md` is the measured 4.88e-7 km, not
  "~1e-7 km"; the Tutorial 2 CSAC holdover band is the 2000–3200 s the test asserts,
  not "~2600–4400 s"; Tutorial 1 quotes the SP3 rows and geometry block at the
  precision the engine writes them. No engine output changed.
- **Validation, quantum, SGP4, animation, claims and integrity pages.**
  `docs/VALIDATION.md`: the enforced gate on the simulated one-second Allan deviation of a
  white-frequency-noise clock is 20 % (was printed 25 %; the 25 % gate is the one on the
  curve at 1, 10 and 100 s); the velocity-random-walk row now states its enforced 20 % gate
  beside the about 12 % observed; and the chip-scale atomic clock's availability in
  `orbit-gnss-challenged` is about 0.73 (was printed about 0.83).
  `docs/QUANTUM.md`: the vibration-limited per-shot noise in the page's worked cold-atom
  example is about 46 times its shot-noise floor (was "~45×"; 46.49 recomputed), and the range
  "1–50 µg/√Hz" for fielded devices, which contradicted a device the page itself cites,
  is replaced by the two cited devices' published figures.
  `docs/SGP4-VALIDATION.md`: the worst position difference is 4.12 mm (was "≈ 4 mm"), with
  the satellite named.
  `docs/ANIMATION.md`: the video-encoder example read the 24 frames-per-second frames at 12;
  it now uses 24, the rate the frame manifest states.
  `docs/CLAIMS-VS-REALITY.md`: the jamming model has 10 tests (was 8), and the coupled
  filter's "2.97 m against 48.8 m" is replaced by what its test asserts (the coupled
  position error below 0.6 times the decoupled one, winning at least 90 of 100 trials).
  `docs/INTEGRITY.md`: the "Today" stamp read v0.22.0; it now names this release, and two
  capabilities built since (the constellation-wide fault mode and the integrity support
  message) leave its gap list.

### Added

#### Spectrum

- **`spectrum` scenario kind: an L-band spectrum model and waterfall**
  (`src/spectrum.rs`, `src/sigmf.rs`, `docs/SPECTRUM.md`). The whole GNSS L band as one
  power spectral density (PSD): GPS L1 coarse/acquisition (C/A) and L2 civil (L2C)
  (BPSK(1)), Galileo E1 (multiplexed binary offset carrier, MBOC(6,1,1/11), or BOC(1,1)),
  GPS L5 and Galileo E5a (BPSK(10)), a kT noise floor with a receiver noise figure, and
  continuous-wave (CW), narrowband, chirp and matched-noise jammers on a scripted
  timeline. Each jammer is scored per band by its spectral separation coefficient (SSC),
  giving the jammer-to-signal ratio (J/S) and effective carrier-to-noise density (C/N0)
  per band per row. The chart is an SVG waterfall with C/N0 bars; result.json carries the
  grid block-averaged in power. Optional `[iq]` draws the model as IQ samples, writes and
  reads a Signal Metadata Format (SigMF) recording (`cf32_le`, `ci16_le`) and compares a
  Welch estimate with the model; optional `[recording]` estimates a real SigMF file.
  Bundled example `scenarios/l-band-waterfall-jamming.toml`: a chirp takes L1 C/A and E1
  at 10 s, a CW tone on the L1 carrier holds C/A at 17.98 dB-Hz after the chirp stops
  while E1, whose spectrum has a null there, recovers; L5 and E5a are untouched.
  - New VALIDATED row: the signal PSDs and SSCs, against the BPSK(n) main lobe of
    2n x 1.023 MHz, the BOC(1,1) lobes centred at +/-1.023 MHz, the Parseval closed forms
    behind the published -61.8 / -64.8 / -67.8 dB/Hz SSCs, and the textbook Q = 1 (CW)
    and 1.5 (matched). The BOC(1,1) PSD maximum is at +/-0.759 MHz, not at the lobe
    centre; the test pins both.
  - Two new MODELLED rows: the waterfall and C/N0 timeline, which reduce exactly to the
    `jamming` kind's chain (cross-checked in every report), and the SigMF codec and Welch
    estimator. No third-party recording is in the repository.
  - `navsignal`: an MBOC variant and `spectral_separation_coeff_offset`.
  - The report prints the C/N0 the `jamming` kind's representative Q table would give
    beside the spectrum-derived one; for a CW tone on the C/A carrier they differ by
    1.8 dB.
  Three new matrix rows: one validated (signal spectra and spectral separation
  coefficients) and two modelled.

#### Solar system

- **Solar-system ephemeris and positioning around any body.** Two new scenario kinds.
  - `solar-system`: the Sun, the eight planets, Pluto, the Moon, Phobos, Deimos, Io, Europa,
    Ganymede, Callisto and Titan at one epoch: heliocentric position and velocity in the
    International Celestial Reference Frame (ICRF), gravitational parameter, radii, J2 (the
    second zonal harmonic) where published, sidereal rotation, the International Astronomical Union (IAU) pole and prime
    meridian, an orbit track over one revolution, and the light time, one-way and two-way
    range, solar Shapiro delay and Sun separation from an observer body and for any extra
    link. Planets come from the Jet Propulsion Laboratory (JPL) Keplerian elements of Standish and Williams (Table 1,
    1800 AD to 2050 AD; Tables 2a/2b, 3000 BC to 3000 AD); the moons from JPL mean elements
    with the IAU synchronous rotation rate, and Titan from the IAU rotation model.
    Example: `scenarios/solar-system-tour.toml`.
  - `body-pnt`: an orbiter or a surface lander around any of those bodies, navigating with
    pseudoranges from a Walker constellation around the body and a clock-free two-way range
    from Earth; dilution of precision, formal uncertainty and seeded least-squares fixes with
    and without the Earth link. Examples: `scenarios/mars-orbit-pnt.toml`,
    `scenarios/europa-surface-pnt.toml`.
  - `Body` gains Mercury, Venus, Jupiter, Saturn, Uranus, Neptune, Pluto and the seven moons,
    a name lookup and a physical record; `AnalyticSolarSystem` gives any body relative to any
    other through the existing `EphemerisProvider` seam.
  - VALIDATED against JPL Horizons (Development Ephemeris DE441; fixtures and queries in
    `tests/fixtures/solar_system/`): Mercury to Saturn and the Earth from Table 1 within twice
    the stated nominal error (worst 1.87 times), all eight planets from Tables 2a/2b (worst
    1.71 times), and the Earth to Mars and Jupiter light time. MODELLED: Uranus and Neptune
    from Table 1, which exceed the stated error against DE441 (2.0 and 5.2 times), Pluto, the
    moons, the body constants and the `body-pnt` results.
  - Finding: the Montenbruck & Gill lunar series in `src/ephem.rs` is already referred to the
    J2000 equinox by its own precession term (0.05 degrees from Horizons), so it is used
    without a further precession rotation.
  Six new matrix rows: two validated (planet positions, light time) and four modelled.

#### Constellations around any body

- **Constellation design at scale (`constellation-design` kind, `src/constellation.rs`).**
  Walker delta and Walker star patterns (T/P/F), explicit element lists and multi-shell
  designs, several constellations per run, around the Earth, the Moon, Mars or any other planet, Pluto or major moon
  from the body constants the `solar-system` kind added. Presets from published nominal elements: the Global Positioning System
  (GPS) baseline and expandable 24-slot constellation (Standard Positioning Service
  Performance Standard, SPS PS, 2020), Galileo Walker 24/3/1 (Open Service Service
  Definition Document issue 1.1), BeiDou medium Earth orbit Walker 24/3/1 plus
  geostationary and inclined geosynchronous satellites (Open Service Performance Standard
  3.0) and GLONASS 24/3/1 (Interface Control Document 5.1). Coverage and dilution of
  precision (DOP) over a latitude/longitude grid: satellites in view, GDOP, PDOP, HDOP and
  VDOP (geometric, position, horizontal, vertical) and availability per cell, globally and
  at the worst site, with one receiver clock per constellation. A visibility prefilter
  (coverage half-angle plus a sub-satellite latitude band, exact on a spherical body) runs
  5 000 satellites on a 10 deg grid in about 0.13 s in a debug build.
  - VALIDATED: the Walker generator reproduces Galileo OS SDD Table 23 and the GLONASS ICD
    slot formula; the GPS preset reproduces the SPS PS equatorial-crossing column (35 of
    36 locations within 0.0108 deg; E3F within 0.06 deg, the table's own row being
    inconsistent); the GPS baseline global HDOP distribution matches SPS PS Appendix B
    (median 0.940 against 0.94, 95 % 1.255 against 1.25, mean 0.965 against 0.96; bar 0.03).
  - MODELLED: arbitrary designs, other bodies, the BeiDou phase and inclined-geosynchronous
    nodes, two-body orbits with optional J2, geometry only.
  - Scenarios: `constellation-multi-gnss-coverage` (102 satellites, availability 100 %,
    median PDOP 0.95, 31.0 in view above 10 deg), `leo-pnt-mega-shell` (5 000 satellites,
    availability 99.22 % at PDOP 3 or less above 20 deg, 0 % at the polar caps, prefilter
    keeps 11.5 % of the pair tests), `lunar-relay-constellation` (14 satellites around the
    Moon, availability 21.33 % overall and highest over the south polar region). All three
    are bundled for `kshana example`. Notes: `docs/CONSTELLATION-DESIGN.md`.
  Three new matrix rows: two validated (Walker generator and GNSS presets, GPS global
  DOP) and one modelled.
- **A campaign driven by the spectrum model, and constellations around any body.**
  `scenarios/campaign-spectrum-holdover-integrity.toml` chains the
  `spectrum` kind with a clock holdover and an integrity monitor (a chirp takes L1 C/A
  and E1; a CW tone then keeps C/A down while E1 recovers and the receiver falls back to
  a Galileo-only sky), pinned against the stand-alone waterfall example in
  `tests/campaign_composition_reference.rs`. `constellation-design` now resolves its
  central body through `Body::by_name`, so a constellation can be laid around any planet,
  Pluto or major moon with the constants the `solar-system` kind uses.

#### Campaigns

- **Campaigns: many scenarios composed into one run.** A new `campaign` kind
  (`src/campaign.rs`, documented in `docs/CAMPAIGNS.md`) runs members of existing kinds
  through the same dispatch as the command line and reads numbers back out of their
  results. Four sections, in any combination:
  - `[[phases]]`: a chained mission on one timeline. Per-kind presets read clock time
    error against its guard, the mean effective carrier-to-noise density ratio (C/N0)
    against the tracking floor, the vertical protection level against the alert limit,
    position error, satellites tracking and alarm flags, held onto a common grid with phase
    boundaries and events. State is handed on by `carry`, `handoff` and `end_at`.
  - `[sweep]`: one to three dotted keys of any kind, optionally with a seeded ensemble at
    every node.
  - `[monte_carlo]`: realisation k at base seed + k, with mean, spread, percentiles and a
    fixed-seed bootstrap 95% confidence interval.
  - `[compose]`: shared values bound into several members, with a combined best and worst
    summary.
  Every result carries a campaign hash and a digest over every member result. Four bundled
  scenarios: `campaign-jam-spoof-holdover-integrity` (nominal, jamming, spoofing ended at
  the clock monitor's 370 s detection, holdover carrying the 37.0 ns spoofed offset with
  an inertial unit coasting, an integrity alarm, recovery), `campaign-sweep-jammer-power`,
  `campaign-shared-jammer-sea-road` and `campaign-monte-carlo-clock-holdover`.
  `tests/campaign_composition_reference.rs` pins the composition identities: a one-phase
  campaign reproduces the stand-alone run bit for bit on three kinds, a fixed-seed
  ensemble is byte-stable, and on a white-frequency-noise clock the ensemble mean lies
  inside the reported interval with the spread inside the chi-square interval of
  sqrt(q_wf * tau) = 16.459 ns. Three new modelled matrix rows.

#### Animation

- **Animation export (`--animate svg|html|frames|all`, `src/animation.rs`,
  [`docs/ANIMATION.md`](docs/ANIMATION.md)).** Any run whose result carries a time series
  can now be written as an animated Scalable Vector Graphics (SVG) file (Cascading Style
  Sheets keyframes, no script: the traces draw in behind a moving time cursor), a single
  self-contained HyperText Markup Language (HTML) player (play and pause, scrub, speed,
  every panel on one synced cursor, event markers that seek on click; no external asset),
  or a numbered SVG frame sequence with a `manifest.json` stating frames per second,
  duration and frame times for a video encoder. A campaign plays as its phases with its
  alarms marked; a spectrum run animates its waterfall row by row. The player follows
  `prefers-color-scheme`, and under `prefers-reduced-motion` both the SVG and the player
  show the finished picture instead of moving. `--animate-fps` and `--animate-duration`
  set the playback; `kshana::api::animate_toml` and `kshana::animation::animate_result`
  are the library entry points, and `result.json` gains an `animation` block only when
  `--animate` runs. Output is a pure function of the result and the options, byte-identical
  on a re-run, with no timestamp. A kind with no sampled time axis is refused with
  "no time series to animate" and nothing is written. The exporter draws the run's own
  samples and adds no number, so it is MODELLED (internal consistency) and carries no
  verification-matrix row; no published number changes. Tests: `tests/animation.rs`
  (determinism, frame count, no external address, well-formed XML, the reduced-motion
  path, and every bundled scenario with a time series animating).

#### Reports

- **Advanced run reports (`src/advanced_report.rs`, [docs/REPORTS.md](docs/REPORTS.md)).**
  Every command-line interface (CLI) run now writes `<scenario>.report.html`, a printable
  HyperText Markup Language (HTML) report, and `<scenario>.report.json`, the same content
  as a machine-readable JavaScript Object Notation (JSON) document. Sections: an executive
  summary; every scenario input with its unit, read from the field-units schema (the
  result's `units` block), else the field-name suffix, else stated as not stated; the
  results with the run's chart, every scalar and a summary of every numeric column; for a
  campaign, the sweep node table, the Monte Carlo mean, standard deviation, 5th / 50th /
  95th percentiles and bootstrap confidence interval with a histogram per metric, the
  chain phase table, or the composition's members and combined summary (and the grid of a
  `sweep` or `sweep-nd` run); an events timeline; the VALIDATED / MODELLED / PARTNER label,
  oracle and test evidence of every verification-matrix row the run's kinds exercise, read
  from the matrix through a kind-to-row crosswalk (a row that grades one input path, such
  as the Simplified General Perturbations 4 (SGP4) path of `orbit` or the measured-record
  path of `slot-timing`, is listed only when the run took that path, and the Shuttle
  Radar Topography Mission (SRTM) reader row is listed for no kind, since no scenario
  field reads an SRTM tile); the not-modelled statements and
  assumptions, each quoted with its source; and a reproducibility record (engine version,
  the source commit when the build sets `KSHANA_GIT_COMMIT`, Secure Hash Algorithm 256-bit
  (SHA-256) digests of the scenario file and the result document, seed, platform and the
  exact command). The report reads no clock: same scenario, seed and engine build give a
  byte-identical report, and the only timestamp it can show is the one `--study-name`
  already writes. A print stylesheet fits A4 and US Letter, repeats table headers and keeps
  rows and figures whole across pages; a Portable Document Format (PDF) file is made with
  the browser's "Save as PDF". There is no `--report pdf` option, because rendering the
  charts into a PDF would need a new, heavy dependency. Tests: `tests/advanced_report.rs`
  reports every bundled scenario (no empty section, no placeholder text, labels equal to
  the matrix, the scenario digest equal to the file's, byte-identical on a re-run) and
  `tests/advanced_report_cli.rs` re-runs the recorded command in a fresh directory and
  requires a byte-identical `result.json`. No published number changes, and the kind,
  scenario-file and matrix-row counts are unchanged.
- **The report, the animation and the exports work together.** The advanced report gains an
  "Animation and exports" section: the run's animated drawing embedded as an inert image (or
  the reason there is none), links to the animation and export files written beside it,
  and every interoperability format with whether it applies, why not, and its
  specification; `report.json` carries the same `companions` block. The HTML animation
  player lists the export files written beside it. The command-line interface (CLI) now
  renders the exports before it writes the report, so both can name the files.

#### Interoperability exports

- **Interoperability exports and imports (`--export`, `src/interop/`, docs/INTEROP.md).**
  `kshana <scenario.toml> --export <format>` writes the scenario's geometry as CZML (the
  Cesium Language), KML (Keyhole Markup Language 2.2, an Open Geospatial Consortium
  standard, with `gx:Track` time-tagged tracks), GeoJSON (Internet Engineering Task Force
  Request for Comments 7946) and Ansys STK (Systems Tool Kit) `.e` ephemerides
  (`EphemerisTimePosVel`, metres, one file per moving object), and the `spectrum` kind's
  synthesised IQ (in-phase and quadrature) snapshot as a SigMF (Signal Metadata Format)
  pair through the existing `src/sigmf.rs`. `--export all` writes every format that
  applies and prints why the others do not; `--export list` reports without running.
  Moving objects are written in the Geocentric Celestial Reference System (CZML
  `INERTIAL`, STK `ICRF`) and Earth-fixed WGS 84 (World Geodetic System 1984) longitude,
  latitude and height (KML, GeoJSON); fixed sites in CZML are `FIXED`. The `jamming` kind
  adds two jammer footprints from its own link equations. `--import-route <file.geojson>`
  writes a GeoJSON `LineString` into the track inputs of `terrain-nav`, `terrain-slam`,
  `gravity-map` and `combined-altpnt`. Exports are byte-deterministic and carry no
  timestamp. `tests/interop_formats.rs` validates every export against its published
  specification, compares exported satellite states with the engine's own to 1 mm, and
  runs every bundled scenario through every format, each either exporting or stating why
  not (the table in docs/INTEROP.md). No published number changes: the `spectrum` kind's
  IQ synthesis moved into one shared function and its result document is byte-identical,
  and `PassesScenario` gains `propagator()` and `epoch_calendar()` used by its own run.
  Kind, scenario-file and verification-matrix counts are unchanged.
  An orbit or integrity scenario with no `epoch` is dated by its satellites' own data:
  `t = 0` is the earliest TLE (two-line element set), broadcast-ephemeris or SP3
  (Standard Product 3) epoch, and each such satellite is rotated into the GCRS and
  Earth-fixed frames at its own instant; 2000-01-01T00:00:00Z is used only when no
  satellite carries an epoch. The SP3 and OEM (Orbit Ephemeris Message) exports keep
  their 2000-01-01 label. To support this, `Sgp4::epoch_jd_utc`,
  `Sp3Interpolator::jd_ut1` and `Propagator::own_jd_utc` are added; nothing that existed
  reads them.

#### Low-Earth-orbit navigation: signal

- **LEO-PNT signal designs and a multi-band spectrum.** A new `leo-signal` kind and a
  `spectrum` kind that reaches beyond the L band: one kind, four scenario files (three
  bundled; the Celeste IOD file is repository-only) and four matrix rows (two VALIDATED,
  two MODELLED).
  - `leo-signal`: parameterised low Earth orbit (LEO) positioning, navigation and timing
    (PNT) signal designs for any system (band, transmit bandwidth, ITU allocation;
    acquisition, data and pilot components with BPSK(n), BOC(m,n), MBOC or flat spectra,
    power shares, FDMA sub-carriers, code lengths and data rates), from compiled-in public
    preset files under `data/leo-signals/` (Xona Pulsar X1/X5, Iridium STL, a Starlink
    signal of opportunity, CentiSpace, a representative C-band design and representative
    UHF/L/S/C/wide-C designs, each citing its source URL) or written inline. Per signal:
    the band-limited PSD and in-band power fractions, Gabor bandwidth, band-limited
    early-late code-tracking jitter against C/N0 and spacing (Betz & Kolodziejski 2009)
    and the ranging accuracy, the acquisition search space, detection probability and
    mean serial and code-parallel acquisition time, the SSC into and from GPS L1 C/A,
    Galileo E1, GPS L5, Galileo E5a, E5b and AltBOC with the C/N0 loss, CW, wideband and
    matched J/S tolerance from the spectrum kind's SSC code, a band trade (ionospheric
    delay, free-space loss, ranging at equal C/N0 and equal EIRP, jammer tolerance), and
    optional shape checks against a described measurement. Examples:
    `scenarios/leo-band-trade.toml`, `scenarios/xona-pulsar-signals.toml`,
    `scenarios/celeste-iod-classical-pilot-signals.toml` (the Celeste IOD bands and
    signal configuration presented at the ESA NAVISP LEO-PNT workshop, 2026; withheld with
    the Celeste IOD preset, see "LEO-PNT end to end" below). Notes in `docs/LEO-SIGNAL.md`.
  - `spectrum`: bands may now be a preset signal design (every component drawn,
    band-limited, C/N0 and J/S referred to the tracked component) or a custom carrier and
    modulation; `[[panels]]` add waterfalls over other frequency ranges on one timeline; a
    `wideband` (barrage) jammer joins the four waveforms. The report adds
    `bands[].tracked_power_dbw`, `bands[].design` and `panels`, and one `not_modelled`
    entry. Plain bands give the same numbers as before. Example:
    `scenarios/multi-band-jamming-waterfall.toml`.
  - `navsignal` gains the sine integral, the BPSK power-in-band and band-limited Gabor
    bandwidth closed forms, the band-limited early-late jitter for any spectrum and its
    Gabor bound, the offset BPSK SSC closed form, the Galileo E5 AltBOC(15,10) spectrum
    and a modulation-label parser; `Modulation::label` prints BPSK(1/3) as a fraction.
  - Four matrix rows: VALIDATED band-limited closed forms (90.3 % of BPSK power in the
    main lobe, the jitter reducing to Kaplan & Hegarty's coherent form, 0.0039564 chips at
    45 dB-Hz, B_L 1 Hz, d 1 chip, T 20 ms, and to its Gabor bound, and the BPSK self-SSC
    2/(3 R_c); the Gabor closed form and the offset SSC against its Parseval form are
    stated as internal cross-checks); VALIDATED maximum LEO Doppler (Xona X1 33.2 kHz
    inside the published 32 to 34 kHz, Iridium within 0.5 kHz of 36 kHz); MODELLED
    `leo-signal`; MODELLED multi-band spectrum.

#### Low-Earth-orbit navigation: link

- **LEO-PNT pass and per-band link budget: the `leo-pass` kind.** LEO-PNT is positioning,
  navigation and timing from satellites in low Earth orbit (LEO). A user (ground, maritime,
  air or indoor, static or moving) and one or more LEO satellites from a designed pass,
  explicit elements, a two-line element set (TLE) through SGP4, or a Walker constellation
  from the `constellation-design` code. Per satellite, band and epoch: look angles, range,
  closed-form range rate and range acceleration (checked every run against central
  differences), Doppler and Doppler rate, free-space loss, EIRP (equivalent isotropically
  radiated power) with an isoflux, Gaussian-beam or flat satellite pattern, a patch user
  antenna, ITU-R P.676 gaseous attenuation, ITU-R P.838/P.618 rain attenuation, P.618
  tropospheric scintillation, ITU-R P.2109 building entry loss for an indoor user,
  polarisation mismatch, system noise temperature and C/N0 (carrier-to-noise density); the
  first-order ionospheric delay per band from a Klobuchar or vertical-TEC (total electron
  content) slant TEC below the satellite, and ionosphere-free band pairs with their noise
  amplification. Galileo or GPS satellites are evaluated with the same receiver from their
  interface-document received powers, so the bell-shaped LEO pass and the flat MEO (medium
  Earth orbit) carriers share one plot. An `[iot]` section gives time to first fix, energy
  per fix and battery life against duty cycle (MODELLED).
  - System-agnostic: the engine needs no preset. Optional presets, each in its own file and
    marked PUBLIC (with URL), REPRESENTATIVE or WORKSHOP: generic multi-band (UHF, L, S, C),
    generic C band, Xona Pulsar X1/X5 (arXiv 2509.19551), Iridium STL, Starlink as a
    Doppler-only signal of opportunity, CentiSpace, and Celeste IOD (in-orbit
    demonstration), whose signal parameters were presented at the ESA NAVISP LEO-PNT
    workshop, 2026, and live in `src/celeste_iod.rs` so they can be withheld (see "LEO-PNT
    end to end" below).
  - Public building blocks in `src/leo_link/` (geometry, antenna, itu, iono, energy,
    presets) for other modules to call.
  - Scenarios: `leo-pass-vs-gnss-cn0`, `leo-indoor-uhf`, `leo-iot-energy`,
    `leo-pass-xona-pulsar`, `leo-pass-iridium`, `leo-pass-celeste-iod-multiband`.
  - Nine matrix rows. VALIDATED (6): ITU-R P.838-3 coefficients against its Table 5; P.618-14
    rain attenuation and scintillation against the ITU-R Study Group 3 validation examples;
    P.2109 building entry loss against the Study Group 3 workbook; first-order ionospheric
    scaling against the IS-GPS-200 group-delay ratio, with the free-space loss; the static-user
    maximum Doppler of a circular orbit against arXiv 2509.19551 Table 1. MODELLED (3): the
    pass and link budget, the presets, and the low-energy fix budget. One kind and six
    scenario files. Notes in `docs/LEO-PASS.md`.

#### Low-Earth-orbit navigation: navigation message

- **LEO navigation message (`leo-navmsg` kind, `src/leo_navmsg/`).** The broadcast
  ephemeris and clock message of a low Earth orbit (LEO) positioning, navigation and
  timing (PNT) satellite, for any orbit, carrier and model; named presets are optional
  data and every capability runs without them. One kind, five scenario files and
  nine matrix rows (three VALIDATED, six MODELLED). See `docs/LEO-NAVMSG.md`.
  - Message content: SVID, issue of data, band and signal health; week, time of week and
    a second-order clock polynomial; the ephemeris in one of four models — the Galileo
    OS SIS ICD 16-parameter Keplerian set, that set plus along-track, cross-track and
    radial correction polynomials, the Liu et al. 2025 22-parameter model
    (doi 10.3390/rs17162894), or the ATOMIC zero-clock ECEF polynomial — and a Klobuchar
    set, NeQuick-G coefficients with the effective ionisation level, and
    system-time-to-UTC parameters with the ICD leap-second cases.
  - Fitter: a truth orbit integrated with zonal J2–J6 or EGM2008 gravity and drag, a
    seeded free or steered clock, a Levenberg–Marquardt fit on non-singular elements, then
    linear least-squares correction polynomials and a clock fitted net of the user's
    relativistic term.
  - Four analyses and scenarios: `leo-navmsg-fit-interval-trade` (signal-in-space range
    error (SISRE) versus fit interval and update period), `leo-navmsg-model-comparison`
    (four models, bits, and Kshana's 22-parameter fit at the Liu et al. altitudes beside
    the published figures, MODELLED), `leo-navmsg-midpass-update` (continuity at each
    message switch in a pass) and `leo-navmsg-encode-decode` (Kshana's own documented
    binary frame with CRC-24Q and a quantisation budget, a RINEX-4-style block labelled a
    Kshana extension, and a CSV table).
  - VALIDATED: the global-average SISRE weights against the published medium-orbit and
    geostationary table of Montenbruck et al. 2018; the Galileo user algorithm against
    RTKLIB on four real Galileo broadcast ephemerides (`tests/leo_navmsg_reference.rs`);
    CRC-24Q against the catalogue check value and the RTCM 10403 1005 example frame.
  - Presets, one file each with sources: Xona Pulsar and Pulsar-0, Iridium, Starlink,
    CentiSpace, a representative C-band system, ATOMIC, and Celeste IOD (the only preset
    using material presented at the ESA NAVISP LEO-PNT workshop, 2026, kept in
    `src/celeste_iod.rs` with its scenarios so it can be withheld).

#### Low-Earth-orbit navigation: fusion

- **Fused MEO + LEO positioning, navigation and timing: `leo-pvt`, `leo-ppp` and
  `ntn-positioning`.** Three new scenario kinds in `src/leo_fusion/`, system-agnostic: every
  constellation is Walker shells, element sets or a GNSS preset, every signal a carrier, a
  chip rate and a carrier-to-noise density (C/N0) envelope, every error budget an explicit
  one-sigma, and named low-Earth-orbit (LEO) systems are optional presets, one file each with
  their sources marked public, workshop or derived (Xona Pulsar X1/X5, Iridium Satellite Time and Location (STL), Starlink
  signals of opportunity, CentiSpace, a representative C-band system, the ATOMIC zero-clock
  ephemeris model, and one ESA Celeste in-orbit-demonstration preset whose workshop-derived
  parameters live in `src/celeste_iod.rs` so they can be withheld).
  - `leo-pvt` has four modes: Doppler positioning (batch least squares on range rate with
    clock-drift and velocity states, a Doppler-only signals-of-opportunity mode, the
    single-pass along-track and cross-track accuracy, and the Doppler, Doppler-rate and jerk
    envelope); joint GNSS + LEO weighted least squares with one clock per system (the
    inter-system bias) or a known offset, per-measurement sigmas as inputs, and the DOP
    against the number of LEO satellites; polar and Arctic coverage against latitude; and LEO
    time transfer to Coordinated Universal Time (UTC) against C/N0 and the receiver
    oscillator with the IS-GPS-200 system-time-to-UTC expression.
  - `leo-ppp`: a float precise point positioning (PPP) extended Kalman filter on
    ionosphere-free code and phase, GNSS only and with LEO augmentation, with its
    convergence time and a Monte Carlo NEES (normalised estimation error squared)
    consistency test. The bundled scenario gives 7.4 min with four MEO systems and 4.8, 3.2,
    2.7 and 2.3 min with 60, 96, 192 and 288 LEO satellites, beside the 9.6, 7.0, 3.2, 2.1 and
    1.3 min of Li et al. (J. Geod. 93:749, 2019) as a MODELLED comparison of the trend.
  - `ntn-positioning`: 5G non-terrestrial network (NTN) positioning in the 3GPP n256
    mobile-satellite S band from the Cramér-Rao bound on time of arrival and Doppler.
  - Scenarios: `leo-doppler-positioning`, `starlink-sop-doppler-positioning`,
    `meo-leo-fused-pvt`, `leo-ppp-convergence`, `ntn-5g-positioning`,
    `polar-arctic-leo-coverage`, `leo-timing-utc` (bundled) and `celeste-iod-fused-pvt`
    (repository-only, withheld with the Celeste IOD preset).
  - Eight matrix rows: the LEO Doppler envelope VALIDATED against the published Iridium
    (±36 kHz: 35.9 kHz, within 5%) and Xona Pulsar X1 (32 to 34 kHz: 33.6 kHz from the
    97 deg shell, with no widening) figures
    (`tests/leo_doppler_reference.rs`); Doppler positioning, joint pseudorange positioning,
    PPP convergence, NTN bounds, LEO timing, polar coverage and the presets MODELLED. Three
    kinds and eight scenario files. Documentation: `docs/LEO-PNT-FUSION.md`.
- **LEO-PNT end to end: the `leo-pnt-chain` kind, and the LEO stages wired together.**
  One low Earth orbit (LEO) positioning, navigation and timing (PNT) system followed from
  its signal design to the user's position, each stage the engine's own kind on its own
  scenario table, with values handed on in code
  ([`docs/LEO-PNT.md`](docs/LEO-PNT.md#end-to-end-leo-pnt-chain)):
  - `leo-pass` bands may name a `leo-signal` design (`signal = "xona-x5"`): the band takes
    the design's centre, transmit bandwidth and tracked-component chip rate, splits its
    EIRP (equivalent isotropically radiated power) across the design's components, and
    reports every epoch's tracked-component C/N0 (carrier-to-noise density) and
    band-limited code-tracking jitter (the ranging error; the `iono_free` pair noise uses
    it too). A band without a design gives the same output as before.
  - `leo-pnt-chain` runs signal -> pass -> navigation message -> fused `leo-pvt` joint fix
    -> optional `leo-ppp`. Every LEO system of the positioning stage that leaves them unset
    takes a C/N0 line in sin(elevation) fitted to the pass, the message's signal-in-space
    range error (SISRE: its representation error at the pass satellite's orbit, with a
    stated `od_sisre_m` orbit-determination term in root-sum-square), and the design's
    carrier and chip rate; the precise point positioning (PPP) cases take the SISRE. Every
    hand-off is listed with its value and unit, and `tests/leo_pnt_chain.rs` checks that
    each equals the upstream output and that upstream changes move the downstream figures.
    `LeoNavmsgScenario::broadcast_sisre` is the new message entry point.
  - Scenarios: `leo-pnt-end-to-end` (a generic 1080 km constellation and the representative
    `generic-l` signal), `xona-pulsar-end-to-end` (the public X5 signal), both bundled, and
    the optional, repository-only `celeste-iod-end-to-end`.
  - The Celeste IOD (in-orbit demonstration) presets of the pass, message and fused
    positioning areas now live in one file, `src/celeste_iod.rs`, which `build.rs` compiles
    in only when it exists (the `kshana_celeste` configuration flag). Its five scenarios
    (`scenarios/*celeste-iod*.toml`) are repository-only, so deleting that file and those
    scenarios withholds every number presented at the ESA NAVISP LEO-PNT workshop, 2026,
    with no source edit; `tests/workshop_preset_isolation.rs` checks which files may name
    the preset or carry its band-plan numbers.
  - Every LEO kind in the advanced report's crosswalk, with its path-specific rows listed
    only when the run took that path (a rain rate, a building, an analysis or mode that ran).
    `leo-pvt` Doppler windows gain `t_s` and `leo-ppp` cases gain `t_min`, so both animate;
    `leo-pass` and `leo-pnt-chain` export CZML, KML, GeoJSON and STK ephemerides, and the
    other LEO kinds state why a format does not apply.

#### Low-Earth-orbit navigation: resilience, focus areas and end-user scenarios

- **LEO-PNT resilience, one scenario per experiment focus area and one per end-user
  vertical, and `docs/LEO-PNT.md` as the overview of every LEO kind and scenario.** Every
  scenario runs without any Celeste data and is bundled.
  - `leo-pass` gains a `[spoofer]` section: a spoofer counterfeits, self-consistently in
    range and range rate, the signals of a claimed position that jumps and/or is pushed
    from the true one after an onset (per LEO band, and the MEO GNSS signal), with or
    without the ionosphere. Two monitors run at a stated false-alarm probability: Doppler
    and pass-geometry consistency (measured range rates against those predicted from the
    orbits and an independent prior position and velocity; a generalised least-squares
    chi-square over a window, on the GNSS, LEO and all channels, frequency-lock-loop noise
    from C/N0) and cross-band consistency (the step of each LEO band pair's
    model-corrected geometry-free combination). The result gives the statistics per epoch
    (so it animates), each pair, and when each monitor detects. Building blocks in
    `leo_link::spoof`. Absent section, unchanged output.
  - Every `leo-pass` band pair also reports the slant total electron content (TEC) its
    geometry-free combination recovers at the pass peak and that estimate's code-noise
    sigma (ionosphere sounding). This adds two fields to every multi-band `leo-pass`
    result; no existing number changes.
  - `leo-pvt` timing mode takes `trace = true`: every row then reports its time error and
    predicted sigma at every epoch, with the same draws, so the row statistics are
    unchanged and the run animates and chains in a campaign.
  - A campaign now exports each member scenario that has geometry (phase runs, the sweep
    and Monte Carlo base scenarios, composed members with their shared values bound) as
    its own file set, the member label in the file name; before, a campaign exported
    nothing. The interoperability table in `docs/INTEROP.md` changes accordingly.
  - A campaign sweep key may index an array of tables by position (`system.2.sisre_m`).
  - Resilience scenarios: `leo-resilience-multiband-diversity` (a 50 MHz L5-band barrage
    swept in power: every L-band signal falls, the UHF, S- and C-band LEO signals keep
    their C/N0), `leo-resilience-js-margin` (J/S margin from received power: GPS L5,
    Galileo E5a and Xona X5 lost at -105, -100 and -95 dBW, a -135 dBW generic LEO signal
    still tracking at -90 dBW), `leo-resilience-spoof-doppler` (a 30 m jump missed by the
    GNSS-only Doppler test and detected by the test on every channel 105 s after the
    onset), `leo-resilience-spoof-monitors` (four spoofers against both monitors) and
    `leo-resilience-gnss-jammed-leo-carries` (GNSS jammed, the S- and C-band LEO layer
    tracked, receiver autonomous integrity monitoring on the LEO layer alone alarming at 2
    of 30 epochs).
  - Focus-area scenarios: `leo-focus-ppp-altitude`, `leo-focus-ntn-bandwidth`,
    `leo-focus-iot-eirp`, `leo-focus-science-iono-sounding`, `leo-focus-data-services`,
    `leo-focus-indoor-uhf`, `leo-focus-fused-pnt-sisre` (resilience in L, S and C is
    `leo-resilience-multiband-diversity`).
  - Vertical campaigns, each animated and exporting its LEO passes:
    `leo-vertical-autonomous-vehicle`, `leo-vertical-rail-maritime` (the bundled maritime
    and rail scenarios plus LEO), `leo-vertical-critical-infrastructure-timing`,
    `leo-vertical-polar-arctic`, `leo-vertical-5g-network-timing`,
    `leo-vertical-asset-tracking-iot`. One-line results in `docs/LEO-PNT.md`.
  - Three MODELLED matrix rows: the Doppler and pass-geometry spoofing monitor, the
    cross-band spoofing monitor, and ionosphere sounding. `tests/leo_resilience_verticals.rs`
    checks the monitors (silent before the onset, the Doppler statistic quadratic in a
    jump, the ionospheric step of a ground spoofer and none from an ionosphere-aware one),
    the sounding identity, the timing trace against the row statistics, and that a
    campaign's export equals each member's own.
  - The `leo-pass` page moves to `docs/LEO-PASS.md`; `docs/LEO-PNT.md` is the overview.

#### Maritime, road and rail scenarios

- **Maritime, road and rail scenarios.** Five bundled examples on existing kinds, so
  ships, road vehicles and trains each have a worked case. Every figure is MODELLED; none
  is validated against measured data.
  - `scenarios/maritime-strait-jamming.toml` (`jamming`): a 50 W broadband jammer on a
    30 m mast, 30 km across open water from a ship. Mean jammer-to-signal ratio (J/S)
    about 48 dB, and availability falls from 1.00 to 0.00.
  - `scenarios/maritime-port-approach-coast.toml` (`ins-trn-coast`): a navigation-grade
    inertial navigation system (INS) at 12 knots with GNSS lost and no aiding. It crosses
    10 m after 281 s and 100 m after 833 s, the harbour-approach and ocean-waters figures
    of International Maritime Organization (IMO) Resolution A.1046(27), used here as
    budgets.
  - `scenarios/maritime-spoof-position-push.toml` (`spoof-detect`): a +2 dB,
    non-carrier-aligned spoofer pushing four satellites to drag a ship 500 m. The fused
    monitor detects it (score 0.70 against a 0.50 threshold) on receiver autonomous
    integrity monitoring (RAIM) and signal quality monitoring (SQM); power monitoring
    alone misses it.
  - `scenarios/automotive-urban-canyon.toml` (`gnss-ins`): a car through a 15 s
    underpass and a 60 s roadside-jammer outage, against a 1.5 m half-lane budget.
    Fused outage root-mean-square (RMS) error is 1.9 m for an automotive-class
    micro-electro-mechanical system (MEMS) unit and 0.7 m for a tactical-grade
    comparator; free-running, the MEMS unit drifts to 850.8 m.
  - `scenarios/rail-tunnel-coast.toml` (`ins-trn-coast`): a tactical-grade INS on a
    train at 160 km/h in a tunnel. It crosses 2 m (track discrimination) after 35.8 s,
    about 1.6 km in, and 20 m after 107.3 s. This is INS only: the engine has no
    odometer model, so it is the pessimistic bound.
  The README scenario-file count moves from 77 to 82. All five are bundled for
  `kshana example` and listed in the browser playground.

#### Model Context Protocol server

- **The Model Context Protocol (MCP) server reaches every kind and every view of a run:
  seven new tools, fourteen in all.** `run_scenario` already dispatched every kind; what an
  agent lacked was a way to a valid scenario of a kind, and the three outputs the command
  line writes beside a result. New tools:
  - `list_example_scenarios` and `get_example_scenario` serve the bundled reference
    scenarios (the `kshana example` table), each with its kind and the first sentence of its
    own header, byte for byte the file under `scenarios/`. A scenario that is in the
    repository but not bundled is refused with the reason. Every kind but `lunar-llr-datum`
    (whose data slice ships with the repository only) has at least one example.
  - `report_scenario` returns the run's report as JSON (JavaScript Object Notation) or as
    the printable HTML (HyperText Markup Language) page (`docs/REPORTS.md`).
  - `animate_scenario` returns the run's time series as an animated SVG (Scalable Vector
    Graphics) drawing, an HTML player or numbered frames, after a JSON summary of what was
    drawn (`docs/ANIMATION.md`). A reply carries at most 120 frames; the command line
    writes any length.
  - `list_export_formats`, `export_interop` and `import_route` serve the interoperability
    exports and the GeoJSON route import (`docs/INTEROP.md`): CZML (Cesium Language), KML
    (Keyhole Markup Language), GeoJSON, the STK (Systems Tool Kit) ephemeris `.e` and
    SigMF (Signal Metadata Format), with a JSON index of the files (suffix, size, and
    SHA-256, the 256-bit Secure Hash Algorithm digest) and the binary SigMF sample file as
    base64.

  The library gains an off-by-default `bundled-scenarios` feature that exposes the
  reference-scenario table as `kshana::bundled_scenarios`; only the MCP server turns it on,
  so the Python wheel and the WebAssembly module still do not carry the scenario text.
  Study suites (`--study`) stay command-line only, because a suite names files on disk and
  the server reads none. The round-trip tests run each new tool against bundled scenarios
  and check the engine's numbers: spectrum, the solar system, constellations around the
  Moon and Europa, a campaign sweep, and the low Earth orbit (LEO) signal, pass,
  navigation-message, fused positioning, precise point positioning,
  non-terrestrial-network and end-to-end kinds.
  Every surface that states the MCP tool count or lists the tools now says fourteen,
  except the playground page under `web/`, which is replaced with the site.

### Changed

- **`<scenario>.report.html` is now the advanced run report** described under Added, and
  the CLI's `wrote …` line names `<scenario>.report.json` as well. The one-page scorecard
  it replaced is unchanged as `RunOutput::html_report()`, which the Python and WebAssembly
  bindings still return. `OracleKind::modelled_reason()` exposes the sentence
  `docs/MODELLED-RATIONALE.md` prints for each MODELLED row; that document is unchanged.
- **The README says what Kshana Pro builds on.** The Editions section and the Kshana Pro
  line under "Support & professional services" now state what Pro's model-based
  systems-engineering (MBSE) and programme tooling does and which of the open engine's
  published outputs it reads (each run's `result.json` and `report.json`, the scenario
  file, the field-units schema and the verification matrix's labels), and what the clock
  digital twins, trade studies and evidence packs rest on in the open engine. Wording
  only: no engine output and no published number changes.
- **Compatibility.** The public enum `navsignal::Modulation` gains an `Mboc` variant,
  which is a breaking change for a caller that matches it exhaustively. The
  `ephemeris` kind's `gcrs_r_m` and `gcrs_v_m_s` columns change (see "Revisions to
  published numbers"). A run is still reproduced by the engine version that made it.

### Fixed

- **`export_table_csv` on the MCP server named four kinds as the only ones with a CSV
  (comma-separated values) table; six publish one.** The tool always returned the table for `telecom-timing` and for
  `leo-navmsg` (its `encode-decode` analysis on a `kepler16` or `kepler-rac` message model),
  but its description, its refusal message and the documents that list the kinds left both
  out, so an agent was told not to ask. They are named now, and a round-trip test fetches
  both tables.
- **The low Earth orbit (LEO) navigation-message frame was not the same bytes on every
  platform, nor in every build of one platform.** `scenarios/leo-navmsg-encode-decode.toml`
  encoded a frame with cyclic redundancy check `0x110315` in the release build on aarch64
  macOS, `0x898BD8` in a debug build of the same source on the same machine, and
  `0x6A82D9` in the WebAssembly (WASM) build; `leo-navmsg-celeste-iod.toml` gave
  `0x6EDB2D` natively against `0xC6E4F0` in the browser. Three causes, each traced to
  the first diverging value:
  - *Between platforms.* The truth orbit was bit-identical on both; the fit's starting
    point was not. `truth::state_to_elements` takes the osculating inclination from an
    `acos` and the argument of perigee from an `atan2`, and the two mathematics libraries
    round those differently in the last place (over 20 000 arguments they disagree for
    781 sines, 858 cosines, 1 931 exponentials and 1 899 two-argument arctangents).
  - *Between build profiles.* An optimised build on macOS merges a sine and a cosine of
    one argument into a single call to the system's combined routine, and its sine is
    not the lone `sin`'s for 379 of 200 000 arguments; an unoptimised build makes the
    two calls. The kind takes a sine and a cosine together in thirteen places.
  - *In the clock.* The normal sampler of `rand_distr` calls the host `exp` and `ln` in
    its two rare branches: of four million seeded draws, three differed in the last bit
    between the native and the WASM build.

  The Levenberg–Marquardt fit, which on this arc stops at its 80-iteration limit and not
  at a minimum, carries one unit in the last place into different quantised fields. The
  kind now computes every transcendental through the new `src/portable_math.rs`, which
  calls the pure-Rust `libm` crate (already in the dependency tree, now named in
  `Cargo.toml`), compiled from one source for every target and not a library call the
  optimiser rewrites: `leo_navmsg::{truth, fit, elements, codec, sisre, services}` and
  the scenario code. Integer powers on that path are spelt out as square-and-multiply,
  because `f64::powi` has unspecified precision, and the truth clock draws its normal
  deviates from `portable_math::standard_normal` (Marsaglia's polar method on the
  generator's raw output). The shared routines the truth orbit integrates through are
  written once, generic over the mathematics library, with a crate-internal `_portable`
  entry point beside the existing one: the spherical-harmonic acceleration, the zonal and
  drag accelerations, the J2 secular rates and the Klobuchar delay. The existing entry
  points are unchanged to the last bit, so no other kind's number moves.
  `leo_navmsg::tests::the_encoded_frame_is_the_same_bytes_on_every_platform` pins the
  whole 171-byte frame and one check value per ephemeris model, in the ordinary
  (unoptimised) test profile, on every platform and with no baseline-host gate;
  `the_kind_never_calls_the_host_mathematics_library` reads the kind's sources from disk
  and fails on an inherent transcendental call, on a `rand_distr` sampler, or on a call
  into a module that has not been reviewed. After the change a debug build, a release
  build and the WASM build produce byte-identical result documents for all five
  `leo-navmsg` scenarios.
  **This moves numbers, recorded here as a revision.** The `leo-navmsg` kind is new in
  this release, so nothing published in an earlier release moves, but every `leo-navmsg`
  figure does, natively and in the WASM build, and the `navmsg` stage of `leo-pnt-chain`
  with it: the orbit figures because the fit now starts from the portable elements, the
  clock figures because the truth clock is a different realisation of the same process.
  `docs/LEO-NAVMSG.md` is regenerated from the new results; its published check value
  for the encode-and-decode scenario was `0x110315` and is `0x19105F` (position within
  1.126 mm of the exact message where it was 0.665 mm, clock within 0.185 mm where it
  was 0.451 mm).
  The size of the changes in the sub-millimetre figures is the fit's sensitivity, not an
  error in either set: the fit is unchanged and still stops at its iteration limit.
- **Seeded resampling drew different indices on a 32-bit target.** `Rng::gen_range` over
  `usize` takes 64 bits from the generator on a 64-bit host and 32 bits on a 32-bit one,
  so the same seed gave other indices, and left the stream elsewhere, in the WASM build.
  Three places drew that way: the percentile bootstraps in `eval_stats` (`bootstrap_ci`,
  `bootstrap_auc_ci`), the shuffle before each training pass in `impairment_ml`, and the
  permutation test in `impairment_study`. They now draw through
  `portable_math::uniform_index`, which always samples as `u64`: exactly what a 64-bit
  host did, so no native number moves.
  **This moves published numbers of the WASM package, recorded here as a revision:** the
  `quantum-anomaly-detect` bootstrap interval of the area under the curve
  (`quantum_auc_ci`, `trade.foms[0].ci95`) was `[0.9901265, 0.9938305]` in the browser
  and is now the native `[0.99035875, 0.993861]`.
- **STK ephemeris file names from mover ids.** A mover id holding a `/` or a space (a
  constellation shell and a satellite, `Pulsar inclined/S1-0163`) made the CLI panic on
  `--export stk`; each id now passes through a safe file part (letters, digits, `-`, `_`).
- **The TEME→GCRS nutation sign**, described under "Revisions to published numbers"
  above because it moves published output.

