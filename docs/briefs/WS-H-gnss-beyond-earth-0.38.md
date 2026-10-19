# 0.38 WS-H: GNSS beyond Earth

Owner's words (19 Oct 2026, coordinator session): "fold it into 0.38, all of it".
Rules: docs/briefs/_COMMON-0.38.md apply in full. Basis and citations: public sources only (ICG Space Service Volume
booklet and SSV literature, published antenna-pattern releases where the licence allows, peer-reviewed papers,
public mission descriptions). Pre-register bars before any scored run; nothing VALIDATED without an external public
oracle; MODELLED otherwise. Branches from origin/integ/0.38.0 (latest); merge commits only; no force-push; no PRs.

## H1 (W54): Space Service Volume and high orbits — kshana feat/ssv-0.38
1. `gnss-ssv` kind: along-trajectory sweep (LEO, MEO, GEO, HEO, lunar transfer) for GPS, Galileo, BeiDou, GLONASS:
   signals in view (>=1, >=4), outage durations, main-lobe vs sidelobe, C/N0, DOP, per the ICG SSV metrics.
   Reuse src/antenna.rs and src/earth_gnss_lunar.rs.
2. Published transmit antenna patterns where licensed; otherwise a labelled parametric shaped-beam + sidelobe model.
   Compare with the ICG SSV booklet availability figures (pre-registered).
3. Limb-ray ionosphere/plasmasphere delay and loss for grazing signals; plasmaspheric TEC observable.
4. Weak-signal acquisition/tracking model (integration, aiding, 10-25 dB-Hz).
5. Onboard GNSS navigation filter (EKF, orbital dynamics, SRP, third body, clock) for HEO and lunar transfer.
6. Formation relative navigation for a multi-spacecraft HEO constellation.
All surfaces or N/A with reason; scenarios; docs/SSV.md; ledger rows.

## H2 (W57): lunar orbit and surface as navigation and geodetic infrastructure — kshana feat/lunar-infra-0.38
1. Onboard Earth-GNSS OD in lunar orbit (ELFO, NRHO) vs independent ground radiometric OD; consistency metrics.
   Reuse cislunar_srif, cislunar_arc_recovery, earth_gnss_lunar, lunar_combination.
2. Joint surface-orbital processing with carrier phase and integer ambiguity resolution (reuse RTK/LAMBDA code).
3. Station and orbiters co-estimating the lunar frame (lunar_frame*, lunar_datum, lunar-station-geodesy).
4. Space-tie co-location (GNSS + VLBI + laser ranging on one spacecraft): frame-tie precision study (FIM).
5. Lunar geodesy observables: libration and Love numbers h2/k2/l2 sensitivity (FIM); tie to lunar time modules.
All surfaces or N/A; scenarios; docs/LUNAR-INFRASTRUCTURE.md; ledger rows.

## H3 (W53): Studio views after H1/H2 — kshana-pro site/studio-0.38
SSV visibility/outage view, formation view, lunar OD consistency view, joint surface-orbital view.
