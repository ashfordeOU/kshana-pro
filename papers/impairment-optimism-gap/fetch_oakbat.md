# Fetching OAKBAT and running the raw-IF probe

OAKBAT (Oak Ridge Spoofing and Interference Test Battery) is distributed through
**Globus only** (no direct HTTP), so it cannot be scripted with `curl`/`wget` and was
not fetched in the build sandbox. The steps below run on any machine with a free Globus
account. OAKBAT is the same datatype as TEXBAT (complex int16 little-endian I/Q) but at
**5 Msps**, ~10-12 GB per scenario.

## 1. Download via Globus

1. Create a free account at <https://www.globus.org> and install **Globus Connect
   Personal** (or use the web file manager with any existing endpoint).
2. Open the ORNL Constellation DOI page and click **"Download Dataset on Globus"**:
   - GPS:     <https://doi.ccs.ornl.gov/dataset/d21dfe58-3af9-5ed8-9c97-693c12045aee>
   - Galileo: <https://doi.ccs.ornl.gov/dataset/0695f7df-7322-5b20-917f-4bff13534e09>
3. This opens the Globus file manager on source endpoint/origin
   `57618e0a-2c99-45ff-9694-24141b92fa17`, path
   `/gen101/world-shared/doi-data/ORNLNCCS/202009/10.13139_ORNLNCCS_1664429/`
   (Galileo under the `1665888` path). Transfer the clean baseline plus the
   graded-power spoofing scenarios you want into a local folder, e.g. `oakbat/`.

## 2. Run the SDR feature-stage probe

Label the clean recording `nominal:` and each spoofing scenario with its power bin
(use the scenario's documented power/JSR ordering for the bin names so the severity axis
is monotone). For example, with a clean file and three graded-power spoof files:

```bash
cd kshana
cargo run --release --example texbat_probe -- \
    ../oakbat/oakbat-probe-result.json 5000000 8 0 \
    nominal:../oakbat/clean.bin \
    p_hi:../oakbat/spoof_high.bin \
    p_mid:../oakbat/spoof_mid.bin \
    p_lo:../oakbat/spoof_low.bin
```

Arguments: `<out.json> <fs_hz=5000000> <window_sec=8> <if_hz=0> label:path ...`.
The probe acquires and tracks every PRN in the first `window_sec` of each file, derives
the SQM (Early-Late) and prompt-power detectors, and computes the cross-detector
optimism gap with the lowest-power spoof scenario as the in-distribution reference. The
result JSON has the same `texbat-optimism-probe/1` schema as the TEXBAT run, so both
fold into the paper the same way as SatGrid.

## Note on honesty

Keep these raw-IF results clearly labelled as **field raw-IF processed by our own
software receiver** (acquisition + tracking validated on synthetic IF, not against a
reference receiver). They are a genuine raw-IF test of the feature stage, not a claim of
receiver-grade tracking accuracy.
