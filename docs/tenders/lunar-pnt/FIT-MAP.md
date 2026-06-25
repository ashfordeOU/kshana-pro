<!--
PRIVATE — Ashforde OÜ internal positioning material. kshana-pro repo only; NEVER publish in the open kshana repo.
Capability crosswalk: Kshana lunar suite → ESA lunar-PNT need, with runnable scenario, oracle, and honest label.
Contact: contact@ashforde.org
-->

# Kshana lunar suite → ESA lunar-PNT need (fit map, PRIVATE)

Each row maps a **Kshana capability** (a module that exists and is committed in the kshana tree) to the
**class of ESA lunar-PNT need** it addresses, the **runnable scenario** that demonstrates it, the
**oracle** it is checked against, and its **honest label**. Labels are the same the open repo enforces in
`src/verification.rs` (and `tests/no_overclaims.rs`): **Validated** requires an *external* oracle;
everything else is **Modelled** (calibrated to public references, tested). "Validated by reuse" means the
capability is built on a kernel that is itself externally validated.

> Kshana **models the class** of system; it is not Moonlight, LCNS, NovaMoon, or any ESA programme, and is
> not endorsed by ESA. Constellation / station parameters in the scenarios are illustrative and
> public-source. No TRL > 3, no flight heritage, no certification claim.

## Lunar suite (net-new capability)

| # | Kshana module | Scenario `kind` | ESA lunar-PNT need it addresses | Oracle | Label |
|---|---|---|---|---|---|
| 1 | `lunar_time` | `lunar-time-offset` | **Coordinated Lunar Time** — a lunar time scale and its relativistic rate vs terrestrial time; LunaNet time. | Published lunar-clock-rate band (~56–59 µs/day) + closed-form relativistic identity + reversible TT↔LTC round-trip. | Modelled |
| 2 | `lunar_vlbi` | `lunar-vlbi` | **Lunar geodetic VLBI** — multi-technique geodesy for a NovaMoon-class station; Earth-baseline delay to a lunar beacon. | Far-field limit reproduces the engine's independent Δ-DOR delay to machine precision; partials finite-difference-checked. | Modelled |
| 3 | `lunar_combination` | `lunar-joint-od-clock` | **Joint orbit-and-clock determination** fusing VLBI + ranging + ISL — the core NovaMoon estimation function. | Internal consistency (covariance realism, NEES ≈ parameter count); headline = with-vs-without-VLBI station-error contrast (~2183 m → ~3.6 m, ~615×). | Modelled |
| 4 | `lunar_frame_realise` | `lunar-frame-realisation` | **Lunar reference-frame realisation** — tie an estimated network to a datum + ICRF (a lunar reference system). | Recovers an injected 7-parameter Helmert transform to machine precision (noiseless); noisy errors scale with σ. | Modelled |
| 5 | `lunar_service` | `moonlight-service-volume` | **Moonlight / LCNS navigation service** — service-volume DOP, coverage, availability, integrity. | DOP kernel validated against `gnss_lib_py`; PL machinery reduces to the existing south-pole case. | Validated by reuse (DOP); service envelope Modelled |
| 6 | `lunar_dpnt` | `lunar-differential-pnt` | **Differential PNT** — a NovaMoon-class reference station broadcasting corrections (lunar SBAS/DGNSS analogue). | Common-mode cancellation exact at zero baseline; residual grows with baseline (~24 m → ~0.008 m at 50 km, ~3186×); user PL reuses DO-229E. | Modelled |
| 7 | `lunar_interop` | `lunar-interop-export` | **LunaNet / IOAG interoperability** — frame + time + ephemeris export and conformance. | CCSDS OEM round-trips through the engine's own parser; metadata + KIF round-trip; conformance flags missing time-system. | Modelled |

## Reused validated foundation (why the lunar suite is credible, not greenfield)

| Capability | Used by the lunar suite for | Oracle | Label |
|---|---|---|---|
| Earth orientation / time scales (`timescales`, `frames`, `precession`, `nutation`, `cio`) | Placing Earth VLBI stations in ICRF; TT/TAI/UTC chaining for lunar time. | SOFA/ERFA reference vectors (bit-for-bit). | **Validated** |
| Dilution of precision (`orbit::dop`) | Lunar service-volume geometry (row 5). | `gnss_lib_py` reference. | **Validated** |
| SGP4 propagation | Earth-segment geometry context. | AIAA/Vallado 2006-6753 verification set (~mm worst-case). | **Validated** |
| Force-model OD (`precise_od`, `lunar_od` GRAIL gravity) | Lunar gravity context; orbit propagation heritage. | Fit against agency precise ephemerides (sub-metre reduced-dynamic; lunar fit ~metre-level vs LRO). | **Validated** (Earth) / Modelled (lunar fit) |
| Δ-DOR / radiometric (`deepspace_od`, `radiometric`) | Far-field cross-check for lunar VLBI (row 2). | Internal reference implementation. | Modelled |
| CCSDS OEM/OMM/TDM I/O (`oem`, `omm`, `tdm`, `interchange`) | LunaNet/IOAG export + round-trip (row 7). | Format round-trip / standards conformance. | Modelled |

## How this maps onto a bid (honest posture)

- **What Ashforde offers:** open, reproducible **mission-analysis and benefits-case** capability for a
  lunar-PNT work package — quantifying time, geodesy, OD+clock, frame, service-volume integrity,
  differential performance, and interoperability **before** hardware exists, with every number labelled
  validated or modelled. Plus the same engineering-and-assurance evidence the FutureNAV C3 assets carry.
- **What Ashforde does *not* claim:** a flight lunar PNT product, certification, flight heritage, or any
  ESA endorsement. Kshana is the *substrate and the evidence*, not the mission.
- **Eligibility:** ESA-STAR entity ID 83208 (SME). Nationality is operative-German per the validated
  registration — relevant to participating-state / geo-return rules in any future lunar AO.
- **Sequencing:** these are **shelf assets**. Outreach is **PARKED** pending explicit founder go; no
  prime/partner names appear here.

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open AGPL-3.0-only (commercial licence available) PNT
simulator · models the class of lunar PNT systems · TRL ~3, no flight heritage, no certification, no ESA
endorsement · validated (external oracle) / modelled labels per the open repo's `verification.rs`.*
