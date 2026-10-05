# Mission Requirements Document (D1.4) — FoM mapping skeleton

_Machine-checked against Kshana Pro v0.2.0 / the open engine. Every measured requirement resolves to a real FoM field and a live verification-matrix row; every follow-on row claims no field and no oracle. The bracketed tag is the evidence class of the verification row that BINDS the requirement, not the strongest evidence anywhere in its call chain._

| ID | Requirement | Tender FoM | Kshana field | Verification row | Oracle | Availability |
|----|-------------|-----------|--------------|------------------|--------|--------------|
| MRD-PNT-01 | Timing accuracy: clock-phase error during GNSS outage (RMS) | ranging/timing accuracy | `FoMScores.timing_rms_ns` | GNSS-denied clock holdover | ReferenceImpl | MEASURED (MODELLED) |
| MRD-PNT-02 | Timing accuracy: 95th-percentile clock-phase error | ranging/timing accuracy (95%) | `FoMScores.timing_p95_ns` | GNSS-denied clock holdover | ReferenceImpl | MEASURED (MODELLED) |
| MRD-RES-01 | Holdover: in-spec coast duration under GNSS denial | resilience / holdover | `FoMScores.holdover_s` | GNSS-denied clock holdover | ReferenceImpl | MEASURED (MODELLED) |
| MRD-RES-02 | Resilience-vs-time: error growth rate during outage | resilience-vs-time | `FoMScores.resilience_slope_ns_per_s` | Quantum-vs-classical PNT trade & GNSS-denied resilience (13503) | ExternalDataset | MEASURED (MODELLED) |
| MRD-AVL-01 | Service availability: fraction of run with an in-spec solution | PNT availability | `FoMScores.availability` | GNSS-denied clock holdover | ReferenceImpl | MEASURED (MODELLED) |
| MRD-INT-01 | Filter self-consistency (Kalman k-sigma containment) | estimator consistency | `FoMScores.integrity` | Onboard clock state estimation | ReferenceImpl | MEASURED (MODELLED) |
| MRD-SEC-01 | Spoof detectability bound from clock stability | security / spoof detection | `FoMScores.security` | Spoofing detection | ExternalDataset | MEASURED (MODELLED) |
| MRD-COV-01 | Coverage: fraction of time a user has a fix | coverage / continuity | `CoverageReport.coverage_fraction` | Orbit propagation & determination | ExternalDataset | MEASURED (VALIDATED) |
| MRD-COV-02 | Revisit: worst-case gap between fixes | revisit interval | `CoverageReport.max_revisit_gap_s` | Orbit propagation & determination | ExternalDataset | MEASURED (VALIDATED) |
| MRD-GEO-01 | Geometry: position dilution of precision (PDOP) | DOP / geometry quality | `Dop.pdop` | Integrity (RAIM/ARAIM/SBAS) | ExternalDataset | MEASURED (VALIDATED) |
| MRD-POS-01 | Position accuracy: Circular Error Probable (CEP, horizontal) | horizontal position accuracy | `PositioningFom.cep_m` | GNSS/INS sensor fusion | ExternalDataset | MEASURED (VALIDATED) |
| MRD-POS-02 | Position accuracy: Spherical Error Probable (SEP, 3-D) | 3-D position accuracy | `PositioningFom.sep_m` | GNSS/INS sensor fusion | ExternalDataset | MEASURED (VALIDATED) |
| MRD-POS-03 | Position accuracy: twice distance RMS (2DRMS, ~95% horizontal) | horizontal position accuracy (95%) | `PositioningFom.drms2_m` | GNSS/INS sensor fusion | ExternalDataset | MEASURED (VALIDATED) |
| MRD-INT-02 | Aviation integrity: Horizontal Protection Level (HPL) | integrity (HPL) | `SbasProtectionLevel.hpl_m` | SBAS protection level (DO-229E weighted-LS HPL/VPL) | ExternalDataset | MEASURED (VALIDATED) |
| MRD-INT-03 | Aviation integrity: Vertical Protection Level (VPL) | integrity (VPL) | `SbasProtectionLevel.vpl_m` | SBAS protection level (DO-229E weighted-LS HPL/VPL) | ExternalDataset | MEASURED (VALIDATED) |
| MRD-INT-04 | Aviation integrity: protection level on the fused INS+GNSS solution while coasting through a GNSS outage | integrity (HPL/VPL) under GNSS denial | — | — | — | PARTNER / FOLLOW-ON |

_Position-domain accuracy (CEP/SEP/2DRMS) is measured by `kshana::fom::positioning_performance`. Its kernel is externally validated (SciPy/NumPy), but each row resolves to the MODELLED fusion row because the covariance it transforms is modelled — the binding evidence, not the strongest in the chain. HPL/VPL are sourced to the DO-229E monitor that actually produces them, since `PositioningFom.hpl_m` is a pass-through. A protection level on the coasting fused INS+GNSS solution remains an honest follow-on (MRD-INT-04). This MRD is a DESIGN skeleton pending the real FPD-10030 SRD, not an agreed requirement set._
