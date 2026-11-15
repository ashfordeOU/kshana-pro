# E4 pre-registration: the public sample of an independent PNT service verification study (roadmap 0.42 E4)

Written and committed on **2026-11-15 (12:15 UTC)**, alone, before the study code exists and
before any figure named below has been computed by anyone for this study. A bar may be tightened
after the run, never loosened. Results go in `RESULTS.md` next to this file, met or missed.

## 0. Scope and edition

E4 here is the **ground and user-side generic part**: user-side coverage, geometry-based accuracy
and integrity of a new PNT service, for users on or near the Earth's surface. Lunar studies, the
referee engine's space part and any study that designs, proves, approves or operates a spacecraft
are **Arksa-exclusive** (roadmap section 1a) and are not built here.

## 1. The sample service and the data

* **LEO service:** Xona Pulsar as published (Leclère, Marathe and Reid, ION GNSS+ 2025,
  arXiv 2509.19551, cited by the open engine's `data/leo-signals/xona-pulsar.toml`): 258
  satellites in 18 planes at about 1080 km, inclined 53° and 97°. Shells as in the open
  scenario `xona-pulsar-end-to-end.toml`: 192 satellites in 12 planes at 53°, 66 in 6 planes at
  97°, Walker phasing 1. This is the **published design**, not the deployed constellation
  (most satellites are not launched); the study says so.
* **GNSS reference:** the open engine's `gps-baseline` and `galileo` presets.
* **User side:** the open engine's `leo-pvt` kind, mode `polar` (kshana 0.41.0): latitudes 0° to
  90° every 10°, longitudes every 45°, 24 h from the mode's epoch (2026-01-01T00:00:00 UTC) every
  600 s, elevation mask 10° for every system, PDOP threshold 6.
* **Integrity sample:** Galileo OSNMA at protocol level on the open engine's **synthetic** sample
  stream `examples/osnma/pages-good.txt` and its throwaway public key (made by the open engine's
  generator; not a recording of any signal), copied unchanged into this repository with its
  provenance. The open OSNMA verifier (`kshana::osnma::cli::verify_report`) is the user receiver.

All of it is MODELLED: geometry from published orbital designs, integrity on synthetic data.

## 2. Bars

| Id | Quantity | Bar |
|---|---|---|
| E4-S1 coverage | LEO alone: availability (fix and PDOP ≤ 6) at each latitude 0° to 60° | **≥ 0.99** at every one |
| E4-S2 polar coverage | LEO alone: availability at 70°, 80° and 90° | **≥ 0.99** at every one |
| E4-S3 geometry accuracy | LEO alone: median HDOP at each latitude 0° to 90° | **≤ 2.0** at every one |
| E4-S4 combined | GPS + Galileo + LEO: availability at each latitude 0° to 90° | **≥ 0.999** at every one |
| E4-S5 integrity | OSNMA tampering: 200 single-bit flips (ChaCha8 seed 4242), each in a bit that ADKD 0 authenticates (word types 1 to 5, the ICD Table 14 fields the open `navdata` module lists), in a page of a sub-frame followed by at least two more sub-frames of the same satellite; the page's CRC recomputed (an attacker can do that); the whole stream verified once per flip. Share of flips after which the tampered satellite is reported `failed` | **≥ 0.99**, and the untampered stream reports **no** `failed` satellite |

Reported without a bar: mean satellites in view and median PDOP, HDOP, VDOP per latitude for GNSS,
LEO and both; a modelled 95 % horizontal accuracy `2 × median HDOP × UERE` for **assumed** UERE
values of 0.5 m and 1.0 m (Xona's range error is not used: it is not in the cited source).

The sample study **passes** only if E4-S1 to E4-S5 all hold; each is reported on its own. The
product claim made from this sample is about the study method and template ("pre-registered,
reproducible, signed, MODELLED"), never that a service is approved or fit for any use.

## 3. Process

1. This file, committed alone.
2. The study kind, its template and the tamper campaign, written without running the sample.
3. The sample run once; `RESULTS.md` with every number; a test pins each bar's outcome; the sample
   report signed with a demo key and published under `report-templates/samples/`.
