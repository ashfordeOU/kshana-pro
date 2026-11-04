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

## 5. Work breakdown (founder, 4 Nov 2026: "plan it all and achieve it all")

Each item gets a pre-registration (bars before data), code, an oracle test, a matrix row and a register
entry. Release slots are proposed. Items marked "→ 0.41" are folded into the current release where they
fit a 0.41 phase, so research starts now and not only after December.

| ID | Item | Oracle (external) | Slot |
|---|---|---|---|
| R1.1 | RTCM3 SSR messages (orbit, clock, code and phase bias, URA) decoded into obs/1 corrections | Published SSR stream samples decoded by an independent open decoder | → 0.41 (A2b) |
| R1.2 | Galileo HAS message decoding and corrections | HAS ICD example vectors; independent decoder output | 0.42 |
| R1.3 | Observable-specific biases (Bias-SINEX) applied in PPP | IGS/CODE OSB products; station coordinates at IGS weekly solution | 0.42 |
| R1.4 | Ocean and solid-Earth loading (BLQ) in PPP | Published loading-service displacements for IGS stations | 0.42 |
| R1.5 | PPP with integer ambiguity resolution | IGS weekly coordinates, pre-registered cm bars, time to fix | 0.42 |
| R1.6 | PPP-RTK with QZSS CLAS | Published CLAS test data and reference positions | 0.43 |
| R2.1 | SBAS L1 message decoding and protection levels | Published SBAS message archives and the providers' PL reports | 0.42 |
| R2.2 | SBAS DFMC (L1/L5) | Published DFMC test messages | 0.43 |
| R2.3 | GBAS approach service protection levels | Published GBAS performance results | 0.43 |
| R2.4 | RNP containment and PBN alerting | Published RNP specifications, worked examples | 0.43 |
| R2.5 | Rail and automotive integrity frameworks | Published integrity-risk allocations | 0.44 |
| R3.1 | Ionosphere head-to-head with Arksa (one physics source, D-K13) | Pre-registered shared cases, GIM truth | → 0.41 (pre-registration) |
| R3.2 | NeQuick-G complete | Published reference-implementation test vectors | 0.42 |
| R3.3 | Native ionosphere model | GIMs and held-out station TEC | 0.42–0.43 |
| R3.4 | GPT3/VMF3-class troposphere | Published grid outputs; IGS zenith delay products | 0.42 |
| R3.5 | Native troposphere model | IGS zenith delay, held-out stations | 0.43 |
| R3.6 | Real-time TEC from Station fleets | GIMs; held-out stations | 0.43 (Pro) |
| R3.7 | Scintillation now-casting | Measured S4 and σφ records, held-out | 0.43 |
| R4.1 | Vector tracking and deep coupling at the IQ layer | Independent baseband generator; tracking-loss thresholds vs published | 0.43 |
| R4.2 | Multipath and NLOS models, 3D-mapping-aided GNSS | Open urban datasets (licence first) | 0.43 |
| R4.3 | Signal quality monitoring metrics | Published SQM thresholds and evil-waveform cases | 0.42 |
| R4.4 | CRPA beamforming and null steering | Published array results | 0.43 |
| R5.1 | Spoofing and meaconing detection on every usable public dataset | Dataset ground truth, pre-registered ROC bars | 0.42 |
| R5.2 | Interference propagation over terrain and to aircraft | Published propagation-model reference values | 0.42 |
| R5.3 | Chimera-style authentication (when published) | Specification vectors | Later |
| R6.1 | Common-view time transfer (CGGTTS) | Published CGGTTS files (format checksum) and BIPM results | → 0.41 (A2b) |
| R6.2 | PPP time transfer | Published time-link results | 0.42 |
| R6.3 | PTP, NTP and White Rabbit validated | Published logs with independent reference | 0.42 |
| R6.4 | Ensemble time scale | Published algorithm examples | 0.43 |
| R7.1 | eLoran (with additional secondary factors) | Published eLoran accuracy trials | 0.43 |
| R7.2 | R-Mode validated | Published R-Mode testbed results | 0.43 |
| R7.3 | DME/DME positioning | Published DME/DME performance | 0.43 |
| R7.4 | LEO signals of opportunity (Doppler positioning) | Published Doppler positioning results | 0.42 |
| R7.5 | 5G positioning reference signals | Published 3GPP evaluation results | 0.43 |
| R7.6 | Indoor UWB, BLE and Wi-Fi RTT | Open indoor datasets (licence first) | 0.44 |
| R7.7 | Magnetic-anomaly navigation | Open flight datasets (licence first) | 0.44 |
| R7.8 | Vision and lidar odometry | Open datasets with commercial-use licences only | 0.44 |
| R8.1 | Factor-graph smoother | Independent solver on the same problems | 0.43 |
| R8.2 | Multi-sensor integrity | Published multi-sensor PL results | 0.44 |
| R9.x | Space and lunar re-runs and measured data | As in 0.41 C/D rows | 0.41–0.43 |
| R10.x | Sector packs (one per sector) | Each sector's published standard | 0.41 onwards |
| R11.x | Open benchmark, sealed holdouts, blind challenges | Holdout scorers | Ongoing |
| R12.x | Core research P1–P5 | Private pre-registrations (hash-only) | 0.42 onwards (needs kshana-core) |

Execution order: 0.41 phases stay the spine. R1.1, R6.1 and R3.1 start now inside Phase 3.
After 0.41 the order is D-K13 first (R3 → R6 holdover → R3.4/R3.5 → R4/R5), then R1, R2, R7 and R8 by Arksa's priority (asked of Alpha).
A row is VALIDATED only with its oracle. Anything whose oracle data has an unclear licence waits as NOT ASSESSABLE.

## 6. Alpha's decisions (4 Nov 2026; VCprep ledger alpha-11040001-8cbf, alpha-11040328-3a75)

**Programme approved as written, after 0.41, with the D-K13 order first.**

One-source head-to-heads beyond the ionosphere (D-K13 rule):

| Track | Overlap with Arksa | Rule |
|---|---|---|
| R3 troposphere | Arksa's link budgets (tropospheric and rain attenuation) | One troposphere source. The PNT delay and the link attenuation may differ, but they share meteorology inputs |
| R6 time scales | Arksa's TT/TAI/UTC/GPST conversions and leap seconds | Exactly one source for time-scale conversion and leap-second tables across both products |
| R9 space and lunar | Arksa's OD and deep-space ranging | Kshana owns the receiver and PNT side; Arksa owns flight dynamics and OD. Interface: measurements in, states out |
| R5 interference | Arksa's RF interference (C/I, EPFD) | Shared emitter and propagation models |
| R8 fusion | Arksa's OD estimators | Shared estimator core where the maths is the same |

Arksa Ops priorities: R9 > R5 > R6 > R3 > R1 > R8. R2, R4, R7 and R10 serve Kshana's standalone markets and
carry no Arksa priority.

Layers:
- R12 stays Core.
- Real-time TEC from Arksa fleets (R3.6) is Core, because it needs Arksa's fleet data.
- Multi-sensor integrity (R8.2) is Pro after filing, if it is novel.
- The R7 frontier items (quantum sensors, MagNav) are research only, with no public claims until validated.

Honesty: the targets in §3 stay internal plans; public numbers are only those achieved, with a date. The
programme runs on Kshana's own capacity.

**R3.1 ionosphere head-to-head: rules agreed, with four additions.**
1. Frozen in VCprep before either side runs: the windows rule, seed, truth-file SHA-256s and metrics. The sha
   is recorded in both repositories.
2. Each product runs its own model, and one neutral, deterministic scoring script in VCprep `tools/` scores
   both outputs in one schema.
3. The CODE GIM (AIUB) and COSMIC-2 (UCAR) terms are recorded per D-DATA-1. If either is unclear, it is dropped
   before the run, and the record says so.
4. If Arksa's model wins, Kshana gets it only as a Core service call (it is a patent candidate) until counsel
   decides. If Kshana's wins, Arksa calls it through the pinned engine.

Revised order after 0.41: R3.1 freeze and neutral scorer → R6 holdover and the time-scale source → R9 → R5 →
R3.3/R3.4 → R1 → R8. R2, R4, R7 and R10 run alongside, as standalone market items.
