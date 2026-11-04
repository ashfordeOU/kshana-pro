# The PNT ownership programme: physics and research to cover all of navigation

Private (Kshana Pro, ws/coordination). Internal goal (founder, 3 Nov 2026): Kshana owns the entire
navigation and PNT space. "Own" and "dominance" are internal words (D-K14). Public copy compares on
facts. Status: proposal, sent to Arksa's Alpha for review. It does not change the 0.41 scope; tracks
start after 0.41 unless noted.

## 1. Where Kshana stands (first-pass audit, 3 Nov 2026)

Method: a keyword sweep of the 401-row verification matrix, plus the module list (305 source entries).
Keyword counts overstate some areas, so the D-K12 register audit replaces this table row by row.

| Area | Today | Gap |
|---|---|---|
| Timing and clocks (stability, holdover, MTIE/TDEV, clock logs) | Strong: oracle-validated on real clocks | Time transfer (common view, PPP time transfer, two-way) mostly modelled; PTP, NTP and White Rabbit read but not validated; no time-scale ensemble |
| Orbits, frames, Earth orientation, gravity | Strong: SGP4, Cowell, DE440, EGM2008, ITRF, EOP validated | Thin on GNSS-specific precise orbit and clock determination |
| Lunar, cislunar, deep space | Strong in breadth | Measured-data validation is thin (CAPSTONE / Artemis re-runs in 0.41) |
| Interference (jamming, receiver trust, Jammertest referee) | Broad, mostly modelled | Spoofing and meaconing validated on few public datasets; no signal-quality-monitoring suite; CRPA thin |
| Ionosphere | Klobuchar, IONEX maps, scintillation channel | No native 3D model; NeQuick-G partial; no real-time TEC from Stations |
| Troposphere | Saastamoinen inside PPP and GNSS/INS | No VMF3/GPT3-class mapping, no native model, no row of its own |
| Precise positioning | Float PPP and RTK validated on real data | PPP-AR, PPP-RTK, SSR streams (RTCM SSR, Galileo HAS, QZSS CLAS/MADOCA), observable-specific biases |
| Augmentation and integrity | ARAIM reproduced; SBAS partly | GBAS absent; SBAS DFMC; aviation RNP/PBN; integrity for rail and automotive |
| Receiver signal processing | IQ synthesis, acquisition, tracking, campaigns | Vector tracking and deep coupling absent; multipath and NLOS thin; 3D-mapping-aided GNSS thin |
| Sensor fusion | Strong INS error models; EKF/UKF | No factor-graph smoother; multi-sensor integrity thin |
| Alternative PNT | LEO PNT, gravity, terrain, celestial, XNAV, quantum: broad, partly validated | eLoran absent; R-Mode not validated; vision and lidar not validated; indoor (UWB, BLE, Wi-Fi RTT) almost absent; magnetic-anomaly navigation thin |
| Data and readers | 15+ formats, Kshana Connect | Smartphone raw logs read but not in the matrix; NTRIP and RTCM SSR streams |

## 2. Twelve research tracks

Each track: a coverage list, pre-registered bars before data, an external oracle for VALIDATED, the
model register entry (native or port, declared), and published misses. The D-K13 order (ionosphere, clock
and holdover, troposphere, receivers and interference) comes first and is not changed by this list.

| Track | Scope | Layer | First release |
|---|---|---|---|
| R1 Precise positioning | PPP-AR, PPP-RTK, SSR (RTCM SSR, IGS SSR, Galileo HAS, QZSS CLAS/MADOCA), observable-specific biases, ANTEX phase centres, ocean and solid-Earth loading; validated on IGS products and open reference-station data | Open | 0.42 |
| R2 Augmentation and integrity | SBAS L1 and DFMC message processing and protection levels, GBAS approach service, extended ARAIM, RNP and PBN containment, integrity for rail and automotive | Open | 0.42–0.43 |
| R3 Atmosphere and space weather | Native ionosphere (first head-to-head with Arksa), full NeQuick-G, real-time TEC from Station fleets, scintillation now-casting, native troposphere with VMF3/GPT3-class mapping, solar radio bursts | Open; fleet TEC now-casting Pro | 0.42 |
| R4 Receiver physics | Vector tracking and deep coupling, multipath and NLOS models, 3D-mapping-aided GNSS, signal quality monitoring, CRPA and null steering, antenna gain patterns, smartphone duty cycling | Open | 0.43 |
| R5 Interference and authentication | Spoofing and meaconing detection validated on every usable public dataset (licences first), OSNMA and future Chimera, interference propagation over terrain and to aircraft, localisation (B3a) | Open; fleet localisation P3 Core | 0.42 |
| R6 Timing | Common-view and PPP time transfer, two-way satellite time transfer, PTP/NTP/White Rabbit validated, ensemble time scales, optical clocks, telecom, grid and finance compliance packs | Open; time to breach P2 Core until filed | 0.42 |
| R7 Alternative PNT | eLoran, R-Mode, DME/DME, LEO signals of opportunity, 5G positioning reference signals, terrestrial beacons, UWB/BLE/Wi-Fi RTT indoor, magnetic-anomaly and gravity navigation, vision and lidar odometry, quantum sensors | Open | 0.43–0.44 |
| R8 Fusion | Factor-graph smoother, multi-sensor integrity, MEMS/FOG/RLG error models from Allan data, odometry and barometer; validated on open driving and urban datasets (licences first) | Open | 0.43 |
| R9 Space and lunar | GNSS in GEO, HEO and cislunar orbits against measured data, LunaNet and lunar services, autonomous deep-space navigation, X-ray pulsar navigation, Mars | Open | 0.41–0.43 |
| R10 Sector packs | Aviation, maritime, rail, automotive, UAV, surveying, agriculture, telecom, grid, finance: each a scenario set, compliance mapping and Connect feeds | Open; audit packs Pro | 0.41 onwards |
| R11 Evidence infrastructure | Open benchmark with sealed holdouts, conformance kits, data-licence register, cross-platform reproducibility, external blind challenges (Jammertest 2027) | Open; scorecards Pro | Ongoing |
| R12 Core research | P1–P5, fleet learning, threat forecasting | Core (trade secret) | 0.42 |

## 3. Targets (proposals, not results)

| Measure | Today (3 Nov 2026) | 0.42 | 0.43 | 1.0 |
|---|---|---|---|---|
| Verification rows | 401 | 460 | 520 | 600+ |
| VALIDATED against an external oracle | 176 | 220 | 270 | 350+ |
| Native models in the register, scored | 11 of 20 listed | ionosphere + clock | + troposphere, receivers | every model with an unclean licence replaced |
| Open areas with no rows (GBAS, eLoran, vector tracking, factor graph, indoor) | 5 | 3 | 0 | 0 |

Proposed dates: 0.41 December 2026; 0.42 Q1 2027; 0.43 Q2 2027; 0.44 Q3 2027; 1.0 with long-term support
Q4 2027. One builder seat sets the pace; the dates move if the seat count changes.

## 4. Rules that keep this credible
- A row is VALIDATED only with an independent external oracle. Misses are published.
- Pre-registration before data. Open-track bars are public; Pro and Core bars are hash-only (D-K7).
- Data and models with unclear licences go into the register flagged, and are replaced by native models.
- Never "certified"; never "dominate" in public; no defence positioning (civil and commercial first).
- No method in open Kshana or Pro without a layer check (D-K11): a method is Core until counsel decides.
