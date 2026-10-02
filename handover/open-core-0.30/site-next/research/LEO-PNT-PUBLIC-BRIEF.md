# Public-source brief on ESA Celeste and LEO-PNT (as of 2026-09-28)

**Sources and method.** Every claim below comes from web pages or PDFs I fetched. A few figures are labelled **DERIVED**: I computed those myself from standard orbital geometry, so they have no citation. Anything I couldn't confirm is marked **UNCONFIRMED**.

**Four headline findings:**
- **No Celeste signal specification is public.** ESA has not published an interface control document, and no measured numbers from the Celeste satellites (received signal strength, positioning results) are public yet.
- **Xona Pulsar is the best-documented LEO-PNT signal.** Its ION GNSS+ 2025 paper publishes the full signal table.
- **Simulator figures are available for the physics.** Doppler magnitude and rate, pass length, precise-positioning convergence time and broadcast-orbit fit accuracy all have published or derivable numbers.
- **RINEX 4.02 has no LEO navigation records**, and I found no open recordings of LEO-PNT signals.

## 1. ESA Celeste

**Goals**
- Celeste is an in-orbit demonstration (IOD) of 11 satellites plus 1 spare. It tests autonomous precise orbit determination, timing stability and radionavigation signals in several frequency bands. It was approved in 2022, is backed by 15 nations, and two industrial contracts were awarded in 2024. https://www.esa.int/Applications/Satellite_navigation/Celeste/Celeste_IOD_-_Facts_and_figures
- Its results are meant to "support the European Union's decision towards the establishment of an operational navigation layer in LEO", alongside Galileo and EGNOS. https://www.esa.int/Newsroom/Press_Releases/Celeste_s_first_satellites_launched_to_explore_LEO-based_satellite_navigation

**The first two satellites (IOD-1 and IOD-2)**

| | IOD-1 | IOD-2 |
|---|---|---|
| Size and mass | 12U CubeSat, 20 kg | 16U CubeSat, 30 kg |
| Prime | GMV (Spain), with OHB (Germany) as core partner | Thales Alenia Space France (prime), Thales Alenia Space Italy (space segment) |
| Other named suppliers | Platform by Alén Space; payload by GMV, Beyond Gravity and OHB | Deployable L-band quadrifilar-helix antenna by Anywaves |
| First signal | 8 April 2026, 10:38 CET, received at ESTEC and at GMV's Lisbon station | 16 April 2026 (Wikipedia, Thales Alenia Space) or 17 April (ESA page) |

Sources for the table: ESA facts page (above); ION GNSS+ abstract 16907 https://www.ion.org/gnss/abstracts.cfm?paperID=16907 ; Anywaves https://anywaves.com/resources/blog/celeste-leo-pnt-demonstrator/ ; ESA first-signal article https://www.esa.int/Applications/Satellite_navigation/Celeste/ESA_s_Celeste_broadcasts_first_navigation_signal_from_low_Earth_orbit ; InsideGNSS https://insidegnss.com/gmvs-celeste-iod-1-transmits-first-navigation-signal-from-leo/ ; Wikipedia https://en.wikipedia.org/wiki/Celeste_(LEO-PNT)

- **Launch:** both satellites flew on 28 March 2026 at 10:14 CET on a Rocket Lab Electron from Māhia, New Zealand. IOD-1's international designator is 2026-065A (ESA press release above; https://space.skyrocket.de/doc_sdat/celeste-a-1.htm). The GMV ION abstract says 24 March, which was the planned launch date, not the actual one.
- **Orbit:** ESA gives only "quasi polar, between 500 and 600 km". The GMV paper gives a 510 km near-polar sun-synchronous orbit (abstract 16907), and Thales Alenia Space's press release also says 510 km: https://www.thalesaleniaspace.com/en/press-releases/iod-2-satellite-built-thales-alenia-space-esas-celeste-mission-lifted-today-new
  - **UNCONFIRMED:** the exact inclination.
- **IOD-1 ground segment:** mission control is at Tres Cantos near Madrid, and GMV's focusPOD software computes orbits and clocks. Test receivers are at ESTEC and Indra, and a six-month experiment campaign runs from Barcelona (abstract 16907).
- **IOD-1 payload:** an on-board orbit determination and time synchronisation (ODTS) chain, a clock management and control unit, a software-defined-radio signal generator and a multi-frequency GNSS receiver. https://www.ion.org/gnss/abstracts.cfm?paperID=17017
  - **UNCONFIRMED:** the clock type.

**Satellites IOD-3 to IOD-11 ("Pathfinder B")**
- Launches start in 2027. They add S-band two-way signals using 5G waveforms, C-band for jamming resilience, UHF for indoor use, and on-board self-calibration (ESA facts page; abstract 16907).

**Later phases and EU plans**
- ESA member states approved the next phase, the "In-Orbit Preparatory" (IOP) phase, at the November 2025 ESA ministerial conference (CM25). It covers:
  - enabling technology (integrated RF and digital modules, antennas, miniaturised clocks);
  - industrialisation;
  - a set of in-orbit validation satellites, leading towards an EU or commercial operational LEO-PNT layer.

  https://www.esa.int/Applications/Satellite_navigation/ESA_Navigation_at_2025_Ministerial_Council ; tender listing https://esastar-publication-ext.sso.esa.int/ESATenderActions/details/183928
- **UNCONFIRMED:** any EU budget or dates for the operational layer.

**Frequency bands and the ITU deadline**
- IOD-1 and IOD-2 transmit in L-band and S-band. Pathfinder B adds C-band and UHF (ESA facts page).
- The transmissions help "bring into use" the L- and S-band ITU frequency filings for a future operational European LEO-PNT system (ESA first-signal article).
- A secondary source reports the ITU deadline as May 2026 and quotes ESA's programme head on the time pressure it created: https://eurowaypoint.com/celeste-leo-pnt-first-signal-itu-deadline/
  - **UNCONFIRMED:** the ITU filing name and number.

**Signal structure**
- The only first-party description is: GNSS-like L- and S-band signals, synchronised to GNSS time, plus a time-of-transmission correction message relative to GNSS time (abstract 16907).
- A third-party blog lists the following, with an explicit "to be confirmed" flag. None of it comes from a primary document: https://open-receiver.com/blogs/leo-pnt-signals-celeste/
  - L-band at 1191.795 MHz (the Galileo E5 centre), using per-satellite codes shared on one carrier (CDMA), with a 50/50 pilot/data power split;
  - S-band around 2.5 GHz and UHF at 465 MHz, each using separate frequency channels (FDMA);
  - a 341-chip, 1 ms acquisition code;
  - 500 symbols/s on the UHF data channel.
- **UNCONFIRMED:** chip rates, transmit power, and the Celeste ephemeris model and update rate.
- Closest published analogue (a different mission, ATOMIC, not Celeste): https://insidegnss.com/first-steps-toward-a-fully-operational-leo-pnt-payload/
  - on-board ephemeris is a 6th-order polynomial valid for about 1 minute, refreshed every 30 s;
  - the clock (a Microsemi SA.45s chip-scale atomic clock) is steered to GNSS time, so the message carries no clock terms ("zero-clock" model);
  - orbit error is under 10 cm (1σ) per axis; clock error is 24 cm (1σ) with up to 1 m peaks, so timing dominates the signal-in-space range error.

**First results (2025–2026)**
- On 27 August 2026, IOD-2 transmitted 5G New Radio downlink positioning reference signals and narrowband positioning reference signals (both 3GPP standard) in S-band. ESA reports them received and being analysed, with no numbers. Participation in Jammertest in September 2026 was planned. https://www.esa.int/Applications/Satellite_navigation/Celeste/ESA_s_Celeste_demonstrates_5G_positioning_signals_from_low_Earth_orbit
- **UNCONFIRMED:** any published received-signal-strength (C/N0), ranging-error or positioning numbers. The ION 2026 abstracts list these metrics as assessed but give no values.

## 2. Other LEO-PNT systems

**Xona Pulsar** (source unless noted: Leclère, Marathe and Reid, ION GNSS+ 2025, https://arxiv.org/abs/2509.19551)

| Parameter | X1 (L1 band) | X5 (L5 band) |
|---|---|---|
| Carrier | 1593.3225 MHz | 1190.51625 MHz |
| Modulation | EFQPSK | EFQPSK + code shift keying (CSK), 8 bits/symbol |
| Chip rate, code | 1.023 Mchip/s, small-set Kasami, 1 ms | 10.23 Mchip/s, extended Gold, 1 ms |
| Data rate | 1000 symbols/s | 500 symbols/s = 4000 bit/s |
| Received power (min to max) | −148.2 to −139.1 dBW | −144.9 to −136.2 dBW |

- **Constellation:** 258 satellites in 18 planes at about 1080 km, with 53° and 97° inclinations. Data and pilot channels are in quadrature with a 50/50 split, and the pilot has 100 ms overlay codes.
- **Pulsar-0:** launched 23 June 2025 into a 97°, about 520 km orbit (same paper).
- **Company claims:** 350+ passes over four continents, and "1.5 cm native accuracy" (the quantity measured is not defined). https://www.xonaspace.com/news/seeing-is-believing
- **FCC authorisation:** Xona says six production satellites launch in October 2026. https://www.satellitetoday.com/imagery-and-sensing/2026/08/03/xona-gets-approval-to-deploy-commercial-navigation-constellation/
- **Independent measurements:** DLR recorded X1/X5 with a 30 m dish at Weilheim. https://www.ion.org/gnss/abstracts.cfm?paperID=16999
- **Single-satellite Doppler positioning:** meter-level results. https://arxiv.org/abs/2602.06376

**Iridium STL / Iridium PNT** (source: https://rntfnd.org/wp-content/uploads/Recent-PNT-Improvements-and-Test-Results-Based-on-Low-Earth-Orbit-Satellites.pdf)
- 66 satellites transmitting at 1616–1626 MHz using QPSK at 25,000 symbols/s. PNT "bursts" fit in one 90 ms frame.
- Doppler up to ±36 kHz; each satellite has 48 spot beams; raw power 300–2400 times GPS; bursts can be received through up to 39 dB of attenuation.
- NIST timing tests: with a miniature atomic clock, maximum offset from UTC(NIST) under 40 ns and time-interval error under 80 ns over 40 days. https://www.nist.gov/publications/validating-timing-performance-improvement-ionospheric-corrections-iridium-pnt-receivers

**Starlink signals of opportunity** (Kozhaya, Saroufim and Kassas, NAVIGATION 72(1), 2025: https://navi.ion.org/content/72/1/navi.685)
- Starlink's 4/3 ms frame carries a 240 MHz OFDM beacon; using all of it gives about 18 dB more processing gain.
- C/N0 about 57 dB-Hz; Doppler error σ about 30 Hz.
- 2 m 3D positioning error in 20 s with 3 satellites. Code phase and carrier phase are not usable for ranging.

**CentiSpace (Beijing Future Navigation Technology)** (source: https://pmc.ncbi.nlm.nih.gov/articles/PMC10301026/)
- Four experimental satellites launched September–December 2022.
- BPSK augmentation signals near 157x and 117x MHz at a 2.046 Mchip/s code rate.
- Orbit determination accuracy: 0.89 cm radial, 2.35 cm along-track, 1.26 cm cross-track (multi-GNSS).
- Adding two satellites cut PPP convergence to 10 cm from 14.2 to 5.7 min: https://dl.acm.org/doi/10.1007/s10291-023-01589-0
- **UNCONFIRMED:** "Geely/Geespace" corporate details beyond press coverage, which I did not verify.

**TrustPoint**
- A C-band PNT constellation. Its third satellite flew in June 2025, and a NovAtel receiver tracked it live in December 2025. Soft service launch planned for 2027. https://spacenews.com/trustpoint-sets-2027-target-for-initial-rollout-of-leo-based-navigation-services/ ; https://www.gpsworld.com/trustpoint-novatel-demonstrate-c-band-pnt-in-gnss-denied-conditions/
- EnduroSat US will build 40 satellites (announced 10 September 2026). https://satnews.com/2026/09/10/trustpoint-contracts-endurosat-us-to-manufacture-40-satellites-for-c-band-pnt-constellation/
- **UNCONFIRMED:** TrustPoint signal parameters.

**Orbital Clock:** I found nothing public.

## 3. Physics and technique for a simulator

**Pass geometry** (DERIVED; circular orbit, overhead pass, Earth rotation ignored)

| Altitude | Period | Longest pass above 0° | Longest pass above 10° | Slant range at 0° / 10° / 90° elevation |
|---|---|---|---|---|
| 510 km | 94.8 min | 11.7 min | 7.5 min | 2601 / 1720 / 510 km |
| 1080 km | 106.8 min | 18.5 min | 13.4 min | 3866 / 2914 / 1080 km |

- Xona reports passes of "about ten minutes typically" (arXiv 2509.19551).

**Received signal strength vs elevation**
- Free-space path loss is 32.45 + 20·log10(distance in km) + 20·log10(frequency in MHz) dB. https://arxiv.org/pdf/2605.26943
- DERIVED at 1191.8 MHz from 510 km altitude: 148.1 dB at zenith and 162.3 dB at the horizon, a 14 dB swing that the transmit antenna pattern must compensate.
- Distance to GPS is about 20 times greater, which Xona equates to 26 dB. The published min/max received power in the table in §2 is the best public stand-in for the full pass envelope.

**Doppler**
- Xona X1 (L1 band) maximum Doppler is 32–34 kHz, compared with about 4 kHz for GPS L1.
- Doppler rate is up to about 250 times MEO GNSS. Jerk (rate of change of Doppler rate) is ±1.26 Hz/s², compared with ±0.0006 Hz/s² for GPS.
- Doppler-rate differences between satellites in the same plane reach about 200 Hz/s (all arXiv 2509.19551).
- DERIVED at L1: maximum Doppler 37.0 kHz at 510 km and 32.9 kHz at 1080 km; maximum Doppler rate about 550 Hz/s at 510 km and about 220 Hz/s at 1080 km, at zenith on an overhead pass.
- Iridium reports ±36 kHz (RNTF PDF above).

**Doppler positioning**
- Starlink: 2 m in 20 s (NAVIGATION paper above).
- Pulsar single-satellite positioning: meter-level (arXiv 2602.06376).

**PPP convergence with LEO augmentation**
- Simulated, mid-latitudes: adding 60, 96, 192 or 288 LEO satellites cuts multi-GNSS convergence from 9.6 min to 7.0, 3.2, 2.1 and 1.3 min. https://link.springer.com/article/10.1007/s00190-018-1195-2
- Real data: CentiSpace 14.2 → 5.7 min (above).

**LEO orbit determination and broadcast ephemeris**
- On-board GNSS-based orbit determination is at centimetre level (CentiSpace figures above; ATOMIC about 10 cm).
- Liu et al. 2025 extend the 16-parameter Keplerian broadcast ephemeris to 22 parameters (adding ȧ, ṅ, Crs3, Crc3, Crs1, Crc1). With a 20-minute fit arc, the signal-in-space range error is 8.88 cm at 320 km, 6.21 cm at 475 km, 2.87 cm at 786 km, 2.11 cm at 966 km and 0.75 cm at 1336 km, in about 490 bits. https://doi.org/10.3390/rs17162894
- Xie et al. 2018: 4 extra parameters improve the fit by about 10× over the GPS 16-parameter model, giving under 10 cm with a 20-minute fit for 400–1400 km. https://link.springer.com/article/10.1007/s10291-018-0719-9 (paywalled; the figures come from the search-index abstract).

**On-board clocks**
- ATOMIC uses a chip-scale atomic clock (Allan deviation 1e-10 to 1e-11 at 1–1000 s), steered to GNSS (InsideGNSS above).
- Miniaturised clocks are an explicit technology line in the Celeste IOP phase (ESA CM25 page).
- **UNCONFIRMED:** the Celeste and Pulsar clock types.

**Ionosphere**
- LEO signals cross only part of the ionosphere, so a GNSS vertical-TEC map must be scaled to the fraction above the receiver (and for a LEO satellite at ~500 km, much of the electron content lies above it). https://doi.org/10.3390/computers13010004 ; https://link.springer.com/article/10.1007/s10291-018-0710-5
- For signals from 500 km satellites, one model uses a single-layer height of 1600 km (search-result excerpt; primary source not verified).
- Iridium ionospheric corrections improve time-interval error by up to 30 ns (NIST, above).

## 4. Open data and RINEX

- **RINEX 4.00, 4.01 and 4.02** (the current IGS versions) define system codes only for G, R, E, J, C, I and S (GPS, GLONASS, Galileo, QZSS, BeiDou, NavIC, SBAS). They have no LEO navigation-message records; the only mention of LEO concerns receivers carried on satellites. I checked the PDFs directly: https://files.igs.org/pub/data/format/rinex_4.02.pdf ; rinex_4.03 returns 404.
- **Proposed workaround:** convert satellite two-line element sets (TLEs) into RINEX 4 navigation records with small format changes. It is accurate only to "a few tens of km", so it is suitable for visibility and geometry simulation, not ranging. https://arxiv.org/abs/2401.17767
- **Open datasets:** I found none with LEO-PNT signals or LEO-PNT RINEX files. The Starlink, OneWeb and Iridium recordings in Kassas-group papers appear to be private captures. https://people.engineering.osu.edu/sites/default/files/2023-05/Kassas_Navigation_with_multi_constellation_LEO_satellite_signals_of_opportunity_Starlink_OneWeb_Orbcomm_and_Iridium.pdf

## Open questions not answered publicly

1. The Celeste signal-in-space specification: chip rates, codes, equivalent radiated power, S-band and C-band frequencies, and message format.
2. The Celeste ephemeris and clock model (polynomial or Keplerian, validity period, update rate) and the on-board clock types on IOD-1 and IOD-2.
3. Exact IOD orbital elements and inclination (sources say 510 km, "500–560" or "500–600 km").
4. The ITU filing names and numbers, and confirmation that the May 2026 bring-into-use deadline was met.
5. Any measured Celeste C/N0, ranging-error, time-of-transmission-error, 5G positioning or Jammertest results.
6. Launch dates and launcher for IOD-3 to IOD-11, and the IOP contract awards.
7. Pulsar broadcast-ephemeris model, clock type and what "1.5 cm" measures; TrustPoint signal parameters.
8. Any RINEX extension for LEO navigation messages from the IGS RINEX working group.