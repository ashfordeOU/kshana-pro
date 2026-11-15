# E4 public sample study: results

Pre-registration: `2026-11-15_PREREGISTRATION.md` (committed alone, `5b3ded07`, before the study
code; its SHA-256 `0215b7a6…f9876` is in `examples/pnt-verification/study.toml` and checked at every
run). Run once by `tests/evidence_products.rs` (`e4_sample_study_is_as_published_and_signed`);
the signed sample is `report-templates/samples/pnt-verification-public/` (demo key next to it).
MODELLED throughout.

| Bar | Result |
|---|---|
| E4-S1 LEO availability ≥ 0.99 at 0° to 60° | **NOT MET**: worst 0.969 at 10° (0.972 at 0°; 1.000 at 20°, 30°, 60°; 0.999 at 40°; 0.995 at 50°) |
| E4-S2 LEO availability ≥ 0.99 at 70°, 80°, 90° | **NOT MET**: 0.960 at 70°, 0.599 at 80°, 0.000 at the pole (the sweep's last row is 89.9°; median PDOP 6.12 there, just above the threshold of 6, with 8 satellites in view) |
| E4-S3 LEO median HDOP ≤ 2.0 at 0° to 90° | MET: worst 1.96 at 89.9° (0.73 to 0.98 from 0° to 70°) |
| E4-S4 GPS + Galileo + LEO availability ≥ 0.999 at 0° to 90° | MET: 1.000 at every latitude |
| E4-S5a OSNMA tamper detection ≥ 0.99 | **NOT MET**: 136 of 200 flips detected (0.68) |
| E4-S5b no failed satellite on the untampered stream | MET: overall `authenticated`, 0 failed |

**The sample study does not pass** (S1, S2 and S5a not met). Published as it came out.

Reported without a bar: LEO alone, mean satellites in view 7.5 to 12.7, median PDOP 1.93 to 3.01
from 0° to 70°; modelled 95 % horizontal accuracy (`2 × median HDOP × UERE`) 0.73 to 1.96 m for an
assumed UERE of 0.5 m and 1.47 to 3.92 m for 1.0 m (assumptions, not Xona figures).

## What it shows (descriptive, after the run)

- **Coverage.** The published Pulsar design alone gives strong geometry where it has satellites in
  view, but its PDOP rises above 6 for a few per cent of samples near the equator and at 70°, and
  often near the pole, where the 97° shell's satellites cluster at similar elevations (VDOP 3.8 to
  5.8). With GPS and Galileo the combination is available everywhere. The study is of the
  published design (most satellites not launched), on a coarse grid (8 longitudes, 145 epochs).
- **Integrity.** 64 of 200 flips in ADKD 0 bits were not reported `failed` (spread over word types 1
  to 5 and both satellites). The sample stream is the open engine's small synthetic one (24
  sub-frames, two satellites); the pre-registration's eligibility rule (two later sub-frames of the
  same satellite) does not ensure that a tag covering the flipped sub-frame's data is transmitted
  and verified in this stream, so a flip in data no tag covers is not detected by design. The bar
  as written measured that, and it is not met. A study on a stream with full tag coverage (or the
  official test vectors on the owner's computer) is a new pre-registration.
- **What the product claim is.** The template, the pre-registration hash check, the signed report
  and the reproducibility are built and tested; the sample's service-level bars are reported as
  they came out. Nothing here says a service is approved or fit for a use.
