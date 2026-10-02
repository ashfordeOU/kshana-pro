# ESA NAVISP LEO-PNT workshop notes ("Using NAVISP to develop LEO-PNT downstream", 2026-09-28)

Source: slides shown live at the ESA Navigation Innovation and Support Programme (NAVISP) LEO-PNT workshop, attended by the founder, who cleared the information for use (2026-09-28). Slides are marked "ESA UNCLASSIFIED – Limited Distribution", so the slide images and verbatim data stay private. Public outputs cite public sources where one exists, otherwise "presented at the ESA NAVISP LEO-PNT workshop, 2026".

## Slide 13: "Where are we in Celeste IOD?" (courtesy R. Sarnadas, C. Vazquez, N. Bni Lam)
- An L-band frequency has been brought into use (an International Telecommunication Union (ITU) milestone).
- Complete navigation messages were received from Celeste In-Orbit Demonstration (IOD) satellite 1 (16 April 2026) and IOD-2 (27 April 2026). They include a clock model, and the message updated during the pass.
- C/N0 plot over a ~300 s run: LEO satellite "L14" rises from ~41 to a peak of ~57.5 dB-Hz at ~150 s, then falls below 40 dB-Hz by ~270 s. That is a bell-shaped pass lasting ~180–200 s. Galileo E12/E16/E25/E31/E33 are flat at ~44–51 dB-Hz. The LEO peak is ~7–10 dB above Galileo.
- The navigation message is shown as a RINEX-like "EPH L04 LNAV" record, with ephemeris records at 08:00 and 08:05 (5-minute spacing), plus a CSV decode.
- CSV fields: SVID; IOD; Band; WeekNumber; ToW; STRFI; t0c; af0, af1, af2; t0e; m0; sqrtA; ecc; deltaN; omega0; omegaDot; iDot; i0; omega; crc, crs, cic, cis, cuc, cus; a0..a5; c0..c2; r0..r4; shs.
  - Reading: a GPS-style Keplerian ephemeris plus harmonic terms, with extra polynomial or residual terms (a0–a5, c0–c2, r0–r4) that a short LEO fit interval needs. This is an inference, not stated on the slide.

## Disclaimer slide
- The Celeste IOD signals are a vehicle for in-orbit experiments of LEO-PNT concepts and technologies, and for experimenting with different frequency bands and innovative signal features.
- Future projects (Celeste In-Orbit Pathfinder (IOP), EU LEO-PNT) will use their own signal design, which may or may not reuse the IOD bands and features.
  - Consequence for Kshana: model signals as a parameterised signal design (band, modulation, chip rate, power, message model), never hard-code "the Celeste signal".

## Slide: "Overview of Celeste IOD Frequency Bands"
| Signal | Centre frequency | Allocation | Tx bandwidth | Use cases (as stated) |
|---|---|---|---|---|
| E5 | 1191.795 MHz | RNSS | 51.15 MHz | Faster user adoption (RNSS band in use); joint GNSS/LEO-PNT use cases |
| SR | 2492.028 MHz | RDSS | 15 MHz | Faster user adoption |
| SM | 2170.00 MHz | MSS | 0.2 MHz and 5 MHz | Positioning using 5G NTN-enabled user terminals |
| UHF* | 465.465 MHz | EESS | 10 MHz | Indoor positioning and timing; low-energy positioning |
| C | 5019.861 MHz | RNSS | 19.7 MHz | Frequency diversity; lower ionospheric delay enabling single-frequency (SF) services |
| Cext* | B1 5071.011 MHz / B2 5122.161 MHz | Non-RNSS | B1 120 MHz / B2 240 MHz | Higher code ranging accuracy |

Abbreviations:
- RNSS = Radio Navigation Satellite Service
- RDSS = Radio Determination Satellite Service
- MSS = Mobile Satellite Service
- EESS = Earth Exploration-Satellite Service
- NTN = Non-Terrestrial Network (3GPP 5G)
- \* = marked with an asterisk on the slide (meaning not shown)

Derived facts for Kshana, computed and not on the slide:
- 1191.795 MHz = 116.5 × 10.23 MHz, the Galileo E5 centre. 51.15 MHz = 5 × 10.23 MHz, the full AltBOC width.
- 2492.028 MHz is the S-band RDSS/RNSS allocation used by NavIC S (2492.028 MHz).
- The first-order ionospheric delay scales as 1/f². Taking the E5 delay as 1: C band is (1191.795/5019.861)² = 0.0564, UHF is (1191.795/465.465)² = 6.556.
- Free-space loss difference relative to E5 at the same range: UHF is −8.2 dB, S (2492) is +6.4 dB, C (5020) is +12.5 dB.

## Slide: "Celeste IOD Signal Configurations"
- The bands share similar, common modulation types to keep things simple.
- Three signal components are considered, not all present in every signal:
  - a low-complexity acquisition component
  - a data component, which carries the navigation message
  - a pilot component, used for ranging
- The goal of the IOD is to experiment with live signals and de-risk signal-in-space (SIS) features and conditions.

## Slide: "Signal Configurations #1 – Classical Pilot"
BPSK(n) is binary phase-shift keying at n × 1.023 Mchip/s. FDMA is frequency-division multiple access. PfA and PfB are two data options, per the slide.

| SIS plan configuration | Tx bandwidth | Pilot | Data | Acquisition |
|---|---|---|---|---|
| E5#1 | 51.15 MHz | BPSK(10) | PfA: BPSK(1); PfB: BPSK(10) | none |
| S#1 | 15 MHz | BPSK(5) | BPSK(1) FDMA | BPSK(1/3) FDMA |
| UHF#1 | 10 MHz | BPSK(5) | BPSK(1) FDMA | BPSK(1/3) FDMA |
| CR#1 | 19.7 MHz | BPSK(10) | BPSK(10) | BPSK(1/3) |

- The slide shows power spectral density (PSD) plots for each band (E5 1170–1215 MHz, S 2486–2498, UHF 461–470, C 5012–5028): pilot, data and acquisition components overlaid.
- "#1" means other configurations (#2 and later) exist but are not shown here.
- Derived: BPSK(10) null-to-null is 20.46 MHz, slightly wider than CR's 19.7 MHz Tx bandwidth, so the band edge filters it. BPSK(5) is 10.23 MHz null-to-null, which fits exactly inside UHF's 10 MHz. BPSK(1/3) is 0.341 Mchip/s.

## Slide: "IOD-1 and -2 signals" (measured PSDs)
- E5, ±25 MHz span:
  - IOD-1 peaks near −133 dBW/Hz; IOD-2 near −140 dBW/Hz
  - a narrow central BPSK(1)-like peak sits on a broad BPSK(10)-like main lobe (nulls near ±10 MHz)
  - sidelobes near ±15 MHz at about −155 to −158; floor about −160 dBW/Hz
- S band, ±7.5 MHz span:
  - narrow FDMA components highlighted near −6 MHz (red box) and −3.3 MHz (cyan box), at about −150 and −154 dBW/Hz
  - a broad main lobe at about −157 across roughly −2 to +4.5 MHz
  - band edges roll off to about −175
- These are measured receiver-side PSDs. They are good qualitative shape targets for the multi-band spectrum model, but not an oracle: the slide gives no absolute calibration or receiver gain.

## Slide: "Navigation Message"
The message has four parts:
- **Ephemeris data:** a set of Galileo orbital parameters plus a set of corrections for the along-track, cross-track and radial components.
- **Synchronisation data:** Week Number and time of week (TOW), plus a time-of-transmission correction as a second-order polynomial.
- **Auxiliary data:** space-vehicle identifier (SVID) and signal health status.
- **Other services:** ionospheric corrections for single-frequency (SF) users, and the system-time-to-UTC offset.

This matches the CSV decode on slide 13:
- Galileo Keplerian set: sqrtA, ecc, deltaN, omega0, omegaDot, iDot, i0, omega, and the c** harmonic corrections.
- a0–a5, c0–c2 and r0–r4: the along-track, cross-track and radial correction polynomials (inferred).
- af0–af2: the second-order clock polynomial.

## Slide: "Celeste IOD – Call for 3rd Party Experimentation"
- **What it is:** test and validate next-generation PNT with real live signals using the Celeste IOD mission.
- **Who can apply:** industry, research and academia from ESA Participating States to the Celeste programme.
- **How to engage:** register through the ESA Open Space Innovation Platform (OSIP) campaign and sign the attached Terms of Use.
- **Experiment focus areas:**
  - high-accuracy positioning (PPP)
  - PNT resilience (L/S/C band)
  - 5G/6G NTN compatibility
  - low-energy PNT for IoT and mobile
  - LEO-PNT science experiments
  - additional PNT data services
  - indoor navigation (UHF)
  - MEO + LEO fused PNT
- **Potential end-user verticals:**
  - autonomous vehicles
  - railway and maritime
  - critical infrastructure
  - polar and Arctic users
  - wireless networks 5G/6G
  - asset tracking and IoT
