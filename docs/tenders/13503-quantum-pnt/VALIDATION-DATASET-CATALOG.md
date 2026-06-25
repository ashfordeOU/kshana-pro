<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
D2 validated-island expansion (AO/1-13503): verified catalog of REAL public datasets to convert
MODELLED quantum-PNT results to externally-VALIDATED. Built via an adversarial find->verify->synthesize
agent sweep (42 found, 37 survived independent verification, 5 rejected). Contact: contact@ashforde.org
-->

# Quantum-PNT validation datasets — verified catalog (AO/1-13503, D2)

**PRIVATE.** Real, accessible, license-checked public datasets that let us turn MODELLED quantum-PNT
results into externally-**VALIDATED** islands. Each candidate was independently re-fetched and
adversarially verified (default REJECT unless real + accessible + licensable + carries a usable oracle).
Counts: **42 found · 37 survived · 5 rejected**. Honesty rule unchanged: we label VALIDATED only against
an external oracle; everything else stays MODELLED. Raw data is **gitignored**; we commit only derived
scalars + the validation test + a fetch script, and cite the source.

## Two grades of oracle (labelled honestly)

- **STRONGEST — known-answer / shipped reference scalar:** NBS14/Stable32/Cs5071A timing suite
  (deterministic ADEV to 1e-4), OPSSAT-AD (peer-reviewed ESA F1/AUC), BIPM Circular T (cross-issue
  byte-identical overlap + published u), TEXBAT/OAKBAT (time-stamped clean/spoof segmentation).
- **WEAKER — literature constant / self-labels:** Merlet 2010 single transcribed scalar, broadcast-vs-
  precise clock paper, IGETS (paywalled companion-paper oracle) — support a *literature-anchored* bound,
  not independent data-driven validation.

## Recommended first islands (build order)

1. NBS14 + allantools/Stable32 timing suite (NBS14 1000-point SP1065 Table 31, NBS14 10-point Table 30, PHASE.DAT, Cs5071A) -> ADEV/allan: highest credibility, near-zero download for the synthetic sets, deterministic 1e-4 known-answer, converts the timing/ADEV kernel MODELLED->VALIDATED immediately
2. OPSSAT-AD (ESA OPS-SAT anomaly benchmark) -> AUC/eval_stats: REAL ESA mission housekeeping telemetry, CC BY 4.0, peer-reviewed F1/AUC reference band, small CSV no-auth download, strongest anomaly oracle for the ESA bid
3. TEXBAT ds2/ds3/ds7 (Texas Spoofing Test Battery) -> TPL: canonical GNSS spoofing oracle with time-stamped clean/spoof segmentation and a clean quantitative anchor (ds7 1.273 us clock pull by t=468 s); the project already has fetch/probe scripts (#147/#149/#152)
4. EuRoC MAV Visual-Inertial Dataset -> CAI/quantum_imu: best single INS dead-reckoning drift validator, mm-level Vicon/Leica ground-truth trajectory + published ADIS16448 sensor.yaml Allan-variance oracle, open academic
5. BIPM Circular T -> ADEV/allan: most authoritative timing reference value source, CC BY 4.0, deterministic cross-issue byte-identical overlap oracle + published per-point uncertainty; pairs the synthetic NBS14 anchors with a real institutional time-scale oracle

> NOTE (reconciliation): the synthesizer listed **EuRoC** among first islands, but an independent
> verifier **rejected** EuRoC on licensing ('In Copyright', blocks commercial). EuRoC is therefore
> **NEEDS-COUNSEL, not a clean P0**. The clean opening set is the public-domain NBS14/Cs5071A timing
> suite + OPSSAT-AD (CC BY 4.0) + TEXBAT (free-for-research; we already have fetch/probe scripts).

## Catalog at a glance

| Prio | Vertical | Kernel | Dataset | License (short) |
|---|---|---|---|---|
| P0 | anomaly | AUC/eval_stats | OPSSAT-AD — ESA OPS-SAT Satellite Telemetry Anomaly Detect… | CC BY 4.0 (dataset); MIT (code). gitignore… |
| P0 | nav | CAI/quantum_imu | EuRoC MAV Visual-Inertial Dataset (Machine Hall + Vicon Ro… ⚠️ | ETH Research Collection open academic (CC … |
| P0 | security | TPL | TEXBAT (Texas Spoofing Test Battery) v1.1 — ds1-ds8 | Free for research ('use, copy, and modify … |
| P0 | timing | ADEV/allan | NBS14 1000-point frequency test suite (NIST SP1065 Table 3… | NIST / U.S. Government public domain (cite… |
| P0 | timing | ADEV/allan | NBS Monograph 140 Annex 8.E 9-point test data (NIST SP1065… | NIST / U.S. Government public domain (cite… |
| P0 | timing | ADEV/allan | Stable32 PHASE.DAT 1001-point reference dataset (allantool… | allantools fixtures LGPL-3.0; PHASE.DAT or… |
| P0 | timing | ADEV/allan | Cs5071A cesium-clock vs hydrogen-maser phase dataset (real… | allantools repo LGPL-3.0; data has no sepa… |
| P1 | anomaly | AUC/eval_stats | NASA SMAP & MSL Spacecraft Telemetry Anomaly Dataset (tele… | BSD 3-Clause [CORRECTED from candidate's A… |
| P1 | nav | new | ICGEM Static Global Gravity Field Models (EIGEN-6C4, XGM20… | EIGEN-6C4 / GFZ family CC BY 4.0 (DOI 10.5… |
| P1 | nav | CAI/quantum_imu | AQG#B01 Absolute Quantum (Cold-Atom) Gravimeter Field Eval… | CC BY 4.0. gitignore raw zip, cite DOI. |
| P1 | nav | CAI/quantum_imu | Imperial College Strontium Differential Atom Interferomete… | CC BY 4.0. gitignore raw, cite concept DOI… |
| P1 | security | TPL | OAKBAT (Oak Ridge Spoofing and Interference Test Battery) … | US DOE/ORNL (UT-Battelle won't assert copy… |
| P1 | security | TPL | OAKBAT (Oak Ridge Spoofing and Interference Test Battery) … | US DOE/ORNL (UT-Battelle won't assert righ… |
| P1 | security | TPL | Tuni2025 TG-GGSD — GPS Spoofing Datasets | CC BY 4.0 (dataset); MIT (code). gitignore… |
| P1 | security | TPL | Raw IQ Dataset for GNSS/GPS Jamming Signal Classification … | CC BY 4.0; cite both DOIs (4629685 + 37839… |
| P1 | security | DOP/gnss | GNSS Dataset Under Jamming, Spoofing, and Meaconing Condit… | GPL-3.0-or-later (note copyleft before red… |
| P1 | timing | ADEV/allan | BIPM Circular T monthly [UTC - UTC(k)] data files | CC BY 4.0 (https://www.bipm.org/en/copyrig… |
| P1 | timing | ADEV/allan | IGS / CDDIS precise satellite & station clock products (.c… | IGS open data — 'made openly available for… |
| P2 | anomaly | AUC/eval_stats | Server Machine Dataset (SMD) — OmniAnomaly (NetManAIOps, K… | MIT (NetManAIOps). gitignore raw, preserve… |
| P2 | anomaly | AUC/eval_stats | NASA C-MAPSS Turbofan Engine Degradation Simulation Data S… | US Govt public domain ('use at own risk');… |
| P2 | anomaly | AUC/eval_stats | NASA/IMS Bearing Data Set (Univ. Cincinnati Center for Int… | Open/public ('use at own risk'); acknowled… |
| P2 | anomaly | AUC/eval_stats | FEMTO-ST PRONOSTIA Bearing Dataset (IEEE PHM 2012 Prognost… | Open/public ('use at own risk'); cite FEMT… |
| P2 | nav | new | IGETS Level-1 Superconducting Gravimeter Data — Strasbourg… | CC BY 4.0. FTP needs free registration. gi… |
| P2 | nav | CAI/quantum_imu | Cold-Atom (CAG) vs Optical FG5 Absolute Gravimeter Compari… | arXiv nonexclusive-distrib license (prepri… |
| P2 | nav | CAI/quantum_imu | KITTI Raw Data — OXTS RT3003 RTK-GNSS/IMU Ground Truth | CC BY-NC-SA 3.0 — NON-COMMERCIAL + ShareAl… |
| P2 | security | TPL | Tuni2025 TG-GGSD — Galileo Spoofing Datasets | CC BY 4.0. gitignore raw .bin, attribute R… |
| P2 | security | DOP/gnss | GNSS Dataset (with Interference and Spoofing) — Part III (… | CC BY 4.0. gitignore raw, cite DOI 10.1763… |
| P2 | timing | ADEV/allan | BIPM Circular T time-link tables (TWSTFT & TWGPPP two-way … | CC BY 4.0 (BIPM); lab-contributed data may… |
| P2 | timing | DOP/gnss | GNSS broadcast navigation (BRDC) vs MGEX precise clock - G… | IGS/MGEX open ('openly shared, without res… |

---

## Full detail

## P0 datasets

### [P0 · anomaly] OPSSAT-AD — ESA OPS-SAT Satellite Telemetry Anomaly Detection Benchmark

- **Maps to kernel:** `AUC/eval_stats`
- **DOI:** 10.5281/zenodo.12588359
- **Access:** https://zenodo.org/records/12588359 (DOI 10.5281/zenodo.12588359); companion code https://github.com/kplabs-pl/OPS-SAT-AD; paper DOI 10.1038/s41597-025-05035-3
- **Format:** Two CSVs: segments.csv (18 MB raw fragments) + dataset.csv (508 kB features, with 'anomaly' 0/1 + 'train' split columns).
- **License:** CC BY 4.0 (dataset); MIT (code). gitignore raw, cite Ruszczak et al. 2025.
- **Quantity validated:** Supervised/unsupervised anomaly classification AUCROC, AUCPR, F1 vs ground-truth per-fragment anomaly labels (the 'anomaly' column in dataset.csv).
- **Reference oracle:** Ruszczak et al., Scientific Data 2025, DOI 10.1038/s41597-025-05035-3 (arXiv 2407.04730, PMC12041257): BEST SUPERVISED FCNN F1=0.946, AUCPR=0.979, AUCROC=0.989, Acc=0.977, MCC=0.932; BEST UNSUPERVISED MO-GAAL F1=0.726. 2,123 fragments, 9 channels (3 magnetometer + 6 photodiode), 20% anomalous; official split 'train' column = 1,494 train / 529 test.
- **Validation recipe:** Download dataset.csv (no auth) + segments.csv. Structural sanity (exact integer match, tol 0): recover 2,123 fragments, 9 channels, 20% anomalous, split 1,494/529. Detector quality on the official test split: PASS if best supervised F1 >= 0.85 and AUCROC >= 0.90 (within ~0.10 of FCNN ref); unsupervised F1 >= 0.60 (within ~0.13 of MO-GAAL). Treat published numbers as a reference band, not bit-exact (hyperparameter/seed variance). REAL ESA mission housekeeping telemetry with peer-reviewed reference numbers — strongest anomaly oracle for an ESA bid.

### [P0 · nav] EuRoC MAV Visual-Inertial Dataset (Machine Hall + Vicon Room, 11 sequences)

- **Maps to kernel:** `CAI/quantum_imu`
- **DOI:** 10.3929/ethz-b-000690084
- **Access:** https://projects.asl.ethz.ch/datasets/euroc-mav/; data DOI https://doi.org/10.3929/ethz-b-000690084 -> https://www.research-collection.ethz.ch/handle/20.500.11850/690084
- **Format:** ROS bag + ASCII/CSV (IMU, images, ground-truth state); YAML sensor calibration with noise parameters.
- **License:** ETH Research Collection open academic (CC variant could not be read off the 500-erroring record; de-facto free with required BibTeX citation, Burri et al. IJRR 2016 DOI 10.1177/0278364915620033). gitignore raw, commit derived scalars; verify CC variant on first manual download.
- ⚠️ LICENSE CONTESTED — a second finder's copy was REJECTED: ETH Research Collection rights statement reads 'In Copyright' (NOT Creative Commons). Treat as NEEDS-COUNSEL before any commercial/ESA-bid use; do not rely on as clean P0.
- **Quantity validated:** INS dead-reckoning position/attitude drift vs time (free-inertial integration error) compared against Vicon/Leica ground-truth trajectory; plus IMU Allan-variance noise parameters vs published sensor.yaml values.
- **Reference oracle:** Two oracles. (1) Per-sequence state_groundtruth_estimate0/data.csv: Vicon 6-DoF pose (sub-mm/sub-deg, V-room) and Leica Nova MS50 3D position (1 mm, Machine-Hall). (2) Published ADIS16448 noise model in imu0/sensor.yaml: gyro_noise_density=1.6968e-04 rad/s/sqrt(Hz), gyro_random_walk=1.9393e-05, accel_noise_density=2.0000e-3 m/s^2/sqrt(Hz), accel_random_walk=3.0000e-3 (per Kalibr: density = AD at tau=1s, random walk = +1/2-slope fit at tau=3s).
- **Validation recipe:** Download a sequence (gitignore raw). Allan-variance: estimate white-noise density from IMU stream, assert within factor ~2 of sensor.yaml (random-walk factor ~3-5; order-of-magnitude reference, not certified truth). Dead-reckoning: integrate IMU-only strapdown (handle gravity, body-to-world frame, known V1_01 orientation-truth caveat), compare drift growth vs ground truth — expect ~cubic position error for accel-bias-dominated drift; integrated trajectory RMSE-vs-time reproducible to <1% across runs (deterministic). Best single INS dead-reckoning drift validator.

### [P0 · security] TEXBAT (Texas Spoofing Test Battery) v1.1 — ds1-ds8

- **Maps to kernel:** `TPL`
- **Access:** https://radionavlab.ae.utexas.edu/texbat/ (landing); https://rnl-data.ae.utexas.edu/datastore/texbat/ (download index, ds1.bin-ds8.bin + cleanStatic/cleanDynamic + scott.m + 2 PDFs)
- **Format:** Raw IF binary, 16-bit interleaved complex I/Q at 25 Msps (.bin) + MATLAB reader (scott.m).
- **License:** Free for research ('use, copy, and modify these data for research purposes'); cite Humphreys et al. ION GNSS 2012; redistribution discouraged. gitignore raw .bin, commit derived scalars.
- **Quantity validated:** Spoof detection probability Pd = 1 - Pmd and false-alarm rate Pfa of a delay/replay/spoof detector, as a function of time across each trace; time-to-detect.
- **Reference oracle:** Per-record labelled scenario taxonomy (Table I, Humphreys et al. ION GNSS 2012) + time-stamped clean/spoof segmentation. cleanStatic/cleanDynamic = spoof-free baselines (Pfa denominator). ds1 Static Switch; ds2 Static Overpowered Time Push (+10 dB); ds3 Static Matched-Power Time Push (+1.3 dB); ds4 Static Position Push (+0.4 dB); ds5 Dynamic Overpowered (+9.9 dB); ds6 Dynamic Matched-Power (+0.8 dB); ds7 power-matched carrier-phase-aligned time-push; ds8 ds7+zero-delay SCER. First ~100 s spoof-free; spoof onset 90-100 s. CLEANEST QUANT ANCHOR: ds7 onset t=110 s, 1.2 m/s code drift -> 1.273 us clock offset by t=468 s.
- **Validation recipe:** Download ds2/ds3/ds7 (easy/medium/hard; files are 39-47 GB each, ~330 GB total — gitignore raw). Run raw IF -> L1 C/A acquisition/correlator -> SQM/CUSUM detector. PASS: zero spoof-flags in clean prefix t<100s (Pfa<=5%); sustained flag within the onset grace window (ds2/ds5 within ~5s of ~100s; ds3/ds4/ds7 up to ~20s; ds7 detect in [110,130]s). Optional ds7 quant: estimated clock offset reaches ~1.27 us near t=468s, +/-10%. Cross-check published ds3 AUC within +/-0.1. Report Pd/Pfa per scenario. Canonical security oracle.

### [P0 · timing] NBS14 1000-point frequency test suite (NIST SP1065 Table 31)

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication1065.pdf (canonical) or https://tf.nist.gov/general/pdf/2220.pdf; cross-check mirror https://github.com/aewallin/allantools/blob/master/tests/nbs14/test_nbs14_1000point.py
- **Format:** Algorithmic recurrence (regenerated in code) + SP1065 PDF reference table.
- **License:** NIST / U.S. Government public domain (cite NIST; attribution requested). allantools mirror files LGPL-3.0 (irrelevant — we reuse only the public-domain numbers/algorithm).
- **Quantity validated:** ADEV/OADEV/MDEV/HDEV/OHDEV/HTOTDEV/TOTDEV/TDEV at averaging factors AF=1,10,100 on a deterministic 1000-point series.
- **Reference oracle:** NIST SP1065 Table 31 (verbatim). Series from generator seed n0=1234567890, n_{i+1}=16807*n_i mod 2147483647, normalized /2147483647 (first iterates 395529916, 1209410747, 633705974). Normal ADEV=2.922319e-01/9.965736e-02/3.897804e-02; OADEV=2.922319e-01/9.159953e-02/3.241343e-02; MDEV=2.922319e-01/6.172376e-02/2.170921e-02; HDEV=2.943883e-01/1.052754e-01/3.910861e-02; TOTDEV=2.922319e-01/9.134743e-02/3.406530e-02; StdDev(AF=1)=2.884664e-01 (~1/sqrt(12)). Identical values in allantools nbs14_1000_devs[].
- **Validation recipe:** Regenerate the 1000-point series in code from the published recurrence (no download needed for the data; download SP1065 PDF only to cite). Compute ADEV/OADEV/MDEV/HDEV/OHDEV/HTOTDEV/TOTDEV/TDEV at AF=1,10,100 using overlapping estimators with SP1065/Stable32 endpoint conventions. Assert relative error <= 1e-4 vs Table 31 (loosen Theo1/MTOT/TTOT to 1e-3). Caveat: naive textbook AF=1 ADEV lands ~4e-4 off due to estimator convention, not data error.

### [P0 · timing] NBS Monograph 140 Annex 8.E 9-point test data (NIST SP1065 Tables 29-30) / allantools NBS14 10-point

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://tf.nist.gov/general/pdf/2220.pdf (Tables 29-30); mirror https://raw.githubusercontent.com/aewallin/allantools/master/tests/nbs14/test_nbs14_10point.py
- **Format:** Tiny numeric table (9 freq / 10 phase) in PDF and as Python lists.
- **License:** NIST / U.S. Government public domain (cite NIST). allantools mirror LGPL-3.0 (we author our own test against the public-domain numbers).
- **Quantity validated:** ADEV/OADEV/MDEV/HDEV/OHDEV/HTOTDEV/TOTDEV/TDEV and sample std dev at tau=1 and tau=2 on the classic 9 frequency / 10 phase points.
- **Reference oracle:** SP1065 Table 30 (verbatim). Inputs: freq=[892,809,823,798,671,644,883,903,677]; phase=[0.0,103.11111,123.22222,157.33333,166.44444,48.55555,-96.33333,-2.22222,111.88889,0.0]. ADEV=(91.22945,115.8082); OADEV=(91.22945,85.95287); MDEV=(91.22945,74.78849); TDEV=(52.67135,86.35831); HDEV=(70.80607,116.7980); OHDEV=(70.80607,85.61487); HTOTDEV=(70.80607,91.16396) [CORRECTED: candidate notes mislabeled OHDEV as HTOTDEV]; TOTDEV=(91.22945,93.90379); std dev=(100.9770,102.6039). Identical in allantools nbs14_devs[].
- **Validation recipe:** Hard-code the 9-freq/10-phase inputs (or read Table 30). Compute each deviation at tau=1,2 and assert relative error <= 1e-4 (the printed precision floor and the tolerance allantools uses). This is a tiny hand-enterable smoke oracle designed to catch off-by-one indexing errors. Label as algorithm-correctness validation, NOT a real-clock-performance claim (synthetic teaching data).

### [P0 · timing] Stable32 PHASE.DAT 1001-point reference dataset (allantools tests/phasedat)

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://github.com/aewallin/allantools/tree/master/tests/phasedat (raw: https://raw.githubusercontent.com/aewallin/allantools/master/tests/phasedat/PHASE.DAT , phase_dat_oadev.txt , phase_dat_adev.txt)
- **Format:** Plain-text data file + plain-text Stable32 reference tables (.txt).
- **License:** allantools fixtures LGPL-3.0; PHASE.DAT originates from commercial Stable32 (Hamilton Technical Services) — do NOT redistribute raw; commit only derived scalars + test; cite allantools and Stable32.
- **Quantity validated:** Per-tau ADEV/OADEV/MDEV/TDEV/HDEV/OHDEV/TOTDEV/MTOTDEV/TTOTDEV/HTOTDEV/THEO1/MTIE/TIErms across the full octave/decade tau ladder, each with Stable32 min/sigma/max confidence bounds.
- **Reference oracle:** Stable32-computed Sigma column per AF/tau in the bundled .txt tables. Verified anchors: OADEV Sigma=2.9223e-01 at AF=1/tau=1 (N=999), 2.0102e-01 at AF=2 (phase_dat_oadev.txt, 'Sigma Type: Overlapping Allan', Confidence Factor 0.683); Normal ADEV Sigma=2.9223e-01 at AF=1, 2.0510e-01 at AF=2 (phase_dat_adev.txt). Independent COMMERCIAL Stable32 oracle.
- **Validation recipe:** Fetch PHASE.DAT (do NOT commit raw — it ships with commercial Stable32; gitignore it, ship a fetch script). Compute each deviation per tau and assert relative error <= 1e-4 vs the Stable32 Sigma column (relax total/Theo1 variants to 1e-3 for edge/bias-correction differences). This is exactly the comparison allantools' own passing test (test_phasedat.py) performs.

### [P0 · timing] Cs5071A cesium-clock vs hydrogen-maser phase dataset (real hardware, Jan-Feb 2014) + Stable32 reference deviations

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://github.com/aewallin/allantools/tree/master/tests/Cs5071A (data: 5071A_phase.txt.gz; oracle: adev_decade.txt; provenance: Cs5071A_all.py)
- **Format:** Gzipped plain-text phase file (556990 rows) + plain-text Stable32 reference tables (.txt).
- **License:** allantools repo LGPL-3.0; data has no separate license — attribute aewallin/allantools (Anders Wallin); gitignore raw, commit derived scalars only.
- **Quantity validated:** ADEV/OADEV/MDEV/HDEV/OHDEV/TDEV/TOTDEV/MTIE/TIErms vs averaging time tau for a REAL 5071A cesium standard (vs H-maser), with Stable32 min/sigma/max confidence bounds. 556,990 points @ 1 Hz.
- **Reference oracle:** Stable32 Allan deviation table adev_decade.txt (N=556990): tau=1s -> sigma=3.3171e-10 [3.3128e-10, 3.3215e-10]; tau=2s -> 1.6033e-10; tau=4s -> 8.0510e-11; tau=10s -> 3.2709e-11. Parallel decade+all tables for OADEV/MDEV/HDEV/OHDEV/TDEV/TOTDEV/MTIE/TIErms.
- **Validation recipe:** gunzip 5071A_phase.txt.gz (gitignore raw). Run ingest->ADEV/OADEV/MDEV/HDEV/TDEV/MTIE/TIErms at DECADE taus (fast subset). PASS if our ADEV(tau) falls within the Stable32 [min,max] band OR relative error <= 1e-3 for core estimators; use 1% for TOTDEV (end-effect/bias differences). Rate=1 Hz so tau in s == tau in samples — do not double-apply a rate factor. This is the real-hardware ingest->ADEV pipeline validator.

## P1 datasets

### [P1 · anomaly] NASA SMAP & MSL Spacecraft Telemetry Anomaly Dataset (telemanom / Hundman et al. 2018)

- **Maps to kernel:** `AUC/eval_stats`
- **DOI:** 10.1145/3219819.3219845
- **Access:** https://github.com/khundman/telemanom; labels https://raw.githubusercontent.com/khundman/telemanom/master/labeled_anomalies.csv; data via Kaggle https://www.kaggle.com/datasets/patrickfleith/nasa-anomaly-detection-dataset-smap-msl or git clone (original S3 now 403); paper arXiv:1802.04431
- **Format:** Per-channel .npy arrays (train/test) + labeled_anomalies.csv (chan_id,spacecraft,anomaly_sequences,class,num_values; 82 data rows).
- **License:** BSD 3-Clause [CORRECTED from candidate's Apache-2.0], Caltech/JPL 2018, US Govt sponsorship — permissive; honor no-endorsement clause, cite Hundman et al. 2018.
- **Quantity validated:** ROC/PR AUC and Precision/Recall/F1 (raw AND point-adjusted) of an anomaly detector vs the anomaly_sequences ground-truth labels.
- **Reference oracle:** labeled_anomalies.csv anomaly_sequences column = per-channel [start,end] index pairs (105 sequences = 69 SMAP + 36 MSL; 82 channels; 496,444 values). Published anchor (Hundman et al. 2018, Table 2, telemanom LSTM + non-parametric dynamic thresholding WITH pruning p=0.13): Precision=87.5%, Recall=80.0%, Fbeta=0.5=0.71 [CORRECTED: candidate's 'F1 ~0.71' is actually Fbeta=0.5, NOT F1]. Without pruning P=48.9%/R=84.8%/F0.5=0.47.
- **Validation recipe:** Clone repo or download Kaggle mirror (gitignore raw .npy). Score your detector against the anomaly_sequences labels; report achieved P/R/F directly with sanity gate (point-adjusted F1 > 0.3 baseline). To hit the paper anchor, re-implement the telemanom protocol and check P/R/F0.5 near 0.875/0.80/0.71 within +/-0.05 absolute. ALWAYS state protocol (raw vs point-adjusted, with/without pruning) — point-adjustment inflates F1. Generic spacecraft housekeeping (closest PUBLIC analogue to PNT-subsystem anomaly), not quantum-specific.

### [P1 · nav] ICGEM Static Global Gravity Field Models (EIGEN-6C4, XGM2019e, GOCO06s, EGM2008)

- **Maps to kernel:** `new`
- **DOI:** 10.5880/icgem.2015.1
- **Access:** https://icgem.gfz.de/tom_longtime (model list); https://icgem.gfz.de/calcgrid (calc service); gfc download e.g. https://icgem.gfz.de/getmodel/gfc/.../EIGEN-6C4.gfc; record https://dataservices.gfz-potsdam.de/icgem/showshort.php?id=escidoc:1119897
- **Format:** ASCII .gfc spherical-harmonic coefficient files; gridded ASCII/GeoTIFF from calc service.
- **License:** EIGEN-6C4 / GFZ family CC BY 4.0 (DOI 10.5880/icgem.2015.1); EGM2008 is NGA US-Gov free-but-cite (use EIGEN/XGM as primary to keep licensing unambiguous).
- **Quantity validated:** Gravity functional values (geoid undulation N, gravity anomaly Dg, gravity disturbance dg, height anomaly) computed from spherical-harmonic coefficients at given lat/lon/height — the reference gravity map for a gravity-aided/map-matching nav filter.
- **Reference oracle:** Primary (deterministic): gravity functionals from ICGEM's own calculation service on a user grid for a named model — our independent SH synthesis from the same .gfc must reproduce them to numerical precision. Secondary (consistency envelope): EGM2008 vs EIGEN-6C4 (both d/o 2190) near-equivalent at cm-level geoid. Anchor: EGM2008 geoid vs GPS/levelling +/-5 to +/-10 cm (Pavlis et al. 2012, doi:10.1029/2011JB008916).
- **Validation recipe:** Download a .gfc (use EIGEN-6C4 or XGM2019e for clean CC BY licensing; EGM2008 is NGA terms). Implement SH synthesis locally; capture a reference functional grid from the ICGEM calc service once (it can be throttled). Assert geoid agreement < 1 mm and gravity anomaly < 0.01 mGal at matched nodes — a code-correctness check on the SH reader/synthesizer. CRITICAL: match tide system (tide-free/zero-tide/mean-tide), max degree, and reference-ellipsoid GM/a/flat/omega EXACTLY or differences blow up to cm-dm (the #1 false-fail).

### [P1 · nav] AQG#B01 Absolute Quantum (Cold-Atom) Gravimeter Field Evaluation — Cooke et al. 2021

- **Maps to kernel:** `CAI/quantum_imu`
- **DOI:** 10.5194/gi-10-65-2021
- **Access:** https://gi.copernicus.org/articles/10/65/2021/ (article); https://zenodo.org/record/4279110 (raw data, DOI 10.5281/zenodo.4279110)
- **Format:** Peer-reviewed PDF (numeric tables/figures) + raw measurement zips on Zenodo (112 MB).
- **License:** CC BY 4.0. gitignore raw zip, cite DOI.
- **Quantity validated:** Cold-atom absolute gravimeter sensitivity (nm/s^2/sqrt(Hz) -> integrated nm/s^2 after 1 h), repeatability, and measured bias/offset vs an independent FG5 optical absolute gravimeter.
- **Reference oracle:** Cooke et al. 2021 (GI 10:65-79, DOI 10.5194/gi-10-65-2021, CC BY 4.0): sensitivity ~10 nm/s^2 after 1 h (low-noise; 20 urban; <10 after 24 h); repeatability better than 50 nm/s^2; AQG#B01-vs-FG5#228 offset 110 +/- 31 nm/s^2 (Larzac), 44 +/- 66 nm/s^2 (Montpellier); drift -0.02 +/- 0.04 nm/s^2/day.
- **Validation recipe:** Validate a simulated cold-atom-gravimeter sensitivity/bias model against the published numbers (and/or process the Zenodo raw time series). Sensitivity: integrated 1 h noise within the regime band +/-50%. Repeatability: assert <= 50 nm/s^2. Offset: per-site |model - reported| <= 2-sigma (Larzac <= 62, Montpellier <= 132 nm/s^2). If processing raw, validate derived ADEV follows tau^-1/2 white-noise consistent with 10 nm/s^2 @ 1 h. Validates the bias MODEL (Coriolis/orientation ~150 nm/s^2 known effect), not absolute accuracy.

### [P1 · nav] Imperial College Strontium Differential Atom Interferometer data+code (Nature 2026)

- **Maps to kernel:** `CAI/quantum_imu`
- **DOI:** 10.5281/zenodo.19592552
- **Access:** https://zenodo.org/records/19592552 (concept DOI 10.5281/zenodo.19592551, cited by Nature); paper https://www.nature.com/articles/s41586-026-10617-1; preprint arXiv:2504.09158
- **Format:** ZIP (~134 MB): CSV, NPY, NPZ, HDF5, 4 Jupyter notebooks.
- **License:** CC BY 4.0. gitignore raw, cite concept DOI 10.5281/zenodo.19592551.
- **Quantity validated:** Atom-interferometer differential phase/frequency stability as Allan deviation vs averaging time, and injected-signal recovery.
- **Reference oracle:** Nature 2026 / arXiv:2504.09158: per-shot SQL differential phase noise sigma_dphi=43.5(16) mrad/shot; overlapping ADEV white-noise scaling exponent s ~ -0.465 to -0.463 (tau^-0.5); tolerates 4*pi rad/shot injected laser phase noise with no significant increase; signal recovery over 7 freqs at 17.3 mrad amplitude, Q>5000. In-deposit: '2026-01-20-DAI-Analysis' Monte-Carlo ADEV results + residual timeseries regenerate Figure 4.
- **Validation recipe:** Download data_analysis.zip (134 MB, gitignore raw). FIRST close the last-mile check: open the archive, confirm a machine-readable ADEV reference array exists (not just a plotted figure) and the four notebooks recompute the scalars. ADEV at 1 s within 2-sigma of 43.5 mrad (~+/-7%); scaling exponent |s - (-0.5)| <= 0.05; recovered amplitude within +/-10% of 17.3 mrad and Q>5000. Loosen to +/-15% if comparing against a figure-derived curve. Rare openly-licensed quantum-sensor dataset bundling its own reference ADEV.

### [P1 · security] OAKBAT (Oak Ridge Spoofing and Interference Test Battery) — GPS L1 C/A

- **Maps to kernel:** `TPL`
- **DOI:** 10.13139/ORNLNCCS/1664429
- **Access:** https://www.osti.gov/biblio/1664429 (DOI 10.13139/ORNLNCCS/1664429); download via Globus https://doi.ccs.ornl.gov/dataset/d21dfe58-3af9-5ed8-9c97-693c12045aee; oracle paper https://www.osti.gov/servlets/purl/1836467
- **Format:** Raw IF binary, complex interleaved int16 ('IShort'), 5 MHz sample rate.
- **License:** US DOE/ORNL (UT-Battelle won't assert copyright/patent; effectively public-domain US-Gov work); attribution via DOI requested. gitignore raw.
- **Quantity validated:** Spoof/interference detection Pd (must be 1.0 over 6 spoof sets) and Pfa (0.0 over 2 clean baselines), plus detection latency vs documented onset.
- **Reference oracle:** Labelled clean-vs-spoofed split + documented onset, from the ION GNSS+ 2020 paper (OSTI 1836467) [NOT github.com/oakbat, which is EMPTY]. 8 GPS sets = 2 clean baselines + 6 spoof. All start 2020-03-19 10:00:00 GPS Time, ~480 s, 5 MHz. Active-spoof onset t~120 s: os2 time-push +10 dB ('2-min mark'); os3 +1.3 dB; os4 position-push +0.4 dB / 600 m ECEF-Z; os5 dynamic +9.9 dB; os1a static RF-switch 600 m. Magnitude: os2 best-SV CN0 ~44->~50 dB-Hz; os4 +1.5 dB.
- **Validation recipe:** Download via Globus (free account; gitignore raw). Run raw IF (complex int16 'IShort', 5 MHz) -> acquisition/correlator -> detector. PASS: Pd=1.0 (all 6 spoof flagged), Pfa=0.0 (neither baseline flagged); detected onset within +/-5 s of t=120 s, no alarm in 0-115 s. Optional os2 CN0 step within +/-2 dB of ~6 dB rise. COTS-simulator-generated -> fully-specified ground truth. NO published Pd scalar — VALIDATED means reproduce the labelled separation + onset, not match a published number.

### [P1 · security] OAKBAT (Oak Ridge Spoofing and Interference Test Battery) — Galileo E1

- **Maps to kernel:** `TPL`
- **DOI:** 10.13139/ORNLNCCS/1665888
- **Access:** https://www.osti.gov/biblio/1665888 (DOI 10.13139/ORNLNCCS/1665888); download via Globus https://doi.ccs.ornl.gov/dataset/0695f7df-7322-5b20-917f-4bff13534e09; oracle paper https://www.osti.gov/servlets/purl/1836467
- **Format:** Raw IF binary, complex interleaved int16 ('IShort'), 5 MHz sample rate.
- **License:** US DOE/ORNL (UT-Battelle won't assert rights; DOE Public Access Plan); attribution via DOI. gitignore raw.
- **Quantity validated:** Spoof/interference detection Pd and Pfa per Galileo-E1 scenario vs the labelled baseline-vs-spoofing split; multi-constellation (GPS+Galileo) generalisation.
- **Reference oracle:** Same taxonomy as OAKBAT-GPS, Galileo E1, 'identical in every parameter other than the GNSS constellation' (ION GNSS+ 2020 paper, OSTI 1836467). 8 sets = 2 clean + 6 spoof (os1a, os2, os3, os4, os5, +1). Common start 2020-03-19 09:59:42 UTC; onset t=120 s; same per-scenario dB/push severities. NOTE: os1a is intentionally near-undetectable (perfect simulator sync).
- **Validation recipe:** Download via Globus (gitignore raw; confirm Globus serves bytes first). Run Galileo E1 BOC(1,1) acquisition/correlator -> detector. PASS: Pfa=0 on 2 clean baselines AND Pd>=5/6 spoof sets (allow os1a as the single miss; catching it = stretch goal). First-alarm within +/-5 s of t=120 s, none before t=110 s. If reporting ROC, AUC>=0.9. Extends spoof testing to Galileo (TEXBAT does not cover) — directly ESA-relevant. NO published Pd scalar.

### [P1 · security] Tuni2025 TG-GGSD — GPS Spoofing Datasets

- **Maps to kernel:** `TPL`
- **DOI:** 10.5281/zenodo.15623217
- **Access:** https://zenodo.org/records/15623217 (GPS scenarios — CORRECTED primary; candidate's 15470143 is the Galileo record) (DOI 10.5281/zenodo.15623217); descriptor https://doi.org/10.1109/IEEEDATA.2025.3627116; open PDF https://trepo.tuni.fi/handle/10024/233230
- **Format:** Raw I/Q, interleaved 32-bit float binary, 50 MSps (~30 GB/scenario).
- **License:** CC BY 4.0 (dataset); MIT (code). gitignore raw .bin, commit derived Pd/Pfa + test.
- **Quantity validated:** Spoof detection Pd = 1 - Pmd and Pfa vs per-scenario clean (clear-sky) vs spoofing labels; spoofer-count discrimination.
- **Reference oracle:** Peer-reviewed descriptor's 'Summary of Tuni2025 GNSS Clean and Spoofing Scenarios' table (Rahman et al., IEEE Data Descriptions 2025). Record 15623217: 9 GPS E1 scenarios — C-5, C-7 clean; SS-17/18/20/27/28/29/33 spoofed (varying spoofer counts; SS-33 = delayed/time-segmented onset). Genuine NovAtel GNSS-703 sky signals + Spectracom GSG-6 spoofer, USRP NI-2954, 50 MSps. No published Pd scalar — oracle is the labels.
- **Validation recipe:** Download a small labelled subset (1 clean + 2-3 spoofed; ~30 GB/file, multi-TB total — gitignore raw). Parse interleaved 32-bit float I/Q at 50 MSps (readable by open-source FGI-GSRx). Compute per-file detector scores. PASS: Pd=1.0 and Pfa=0.0 over the processed subset (cleanly-labelled lab data), or spoofed-file statistic exceeds clean 99th-pct by >=3 sigma. Spoofer-count (if attempted): +/-1. Pin the exact record + version in the bid (many closely-named siblings exist).

### [P1 · security] Raw IQ Dataset for GNSS/GPS Jamming Signal Classification (Swinney & Woods)

- **Maps to kernel:** `TPL`
- **DOI:** 10.5281/zenodo.4629685
- **Access:** https://zenodo.org/records/4629685 (DOI 10.5281/zenodo.4629685); companion 2019 set https://zenodo.org/records/3783969 (DOI 10.5281/zenodo.3783969)
- **Format:** Raw I/Q in Raw_IQ_Dataset.zip (1.9 GB) + signal_generation.m (MATLAB).
- **License:** CC BY 4.0; cite both DOIs (4629685 + 3783969). gitignore raw.
- **Quantity validated:** Jamming detection Pd (clean vs any-jamming) and 6-class jammer-type classification accuracy / confusion matrix.
- **Reference oracle:** Per-sample class labels (6 classes: DME, narrowband, single AM, single chirp, single FM, no-jamming; 1000 train + 250 test) = bit-exact ground truth. Plausibility band (Morales-Ferre et al., Sensors 2019 19(22):4841, PMC6891345): SVM 94.90% / CNN 91.36% overall; per-class CNN no-jammer ~98.4%, DME ~99%, chirp >91%, NB ~78%, AM ~91%, FM ~89%. Swinney & Woods 2021 up to 98% transfer learning on this exact set.
- **Validation recipe:** Download Raw_IQ_Dataset.zip (1.9 GB, no auth; gitignore raw). Detection: require Pd>=0.95 at Pfa<=0.05 on clean-vs-any-jamming. 6-class: balanced accuracy >=0.85 (above 1/6 chance, consistent with 91-95% literature band), report full confusion matrix. Bit-exact deterministic check = recovery of the per-sample labels. Best fit for the JAMMING/interference vertical (vs spoof/replay sets). DISCLOSE: software-generated synthetic signals (acceptable for a sim-representative demonstrator).

### [P1 · security] GNSS Dataset Under Jamming, Spoofing, and Meaconing Conditions (Jammertest 2024)

- **Maps to kernel:** `DOP/gnss`
- **DOI:** 10.5281/zenodo.15911589
- **Access:** https://zenodo.org/records/15911589 (DOI 10.5281/zenodo.15911589 — CORRECTED: candidate's 15911359 is README-only); concept DOI 10.5281/zenodo.15910563 (resolves to latest)
- **Format:** Per-scenario scenario.json + info.txt + mon_rf.csv + nav_pvt.csv + rinex.csv + *.ubx (tar.gz).
- **License:** GPL-3.0-or-later (note copyleft before redistributing derived data); cite concept DOI 10.5281/zenodo.15910563. gitignore raw.
- **Quantity validated:** Attack detection/classification (Jamming vs Spoofing vs Meaconing vs Jamming+Spoofing vs clean) at receiver-observable level; detection onset; position-error-under-spoof.
- **Reference oracle:** Self-labelled ground truth in scenario.json + folder path: (1) attack_type in {Jamming, Spoofing, Meaconing, Jamming+Spoofing}; (2) attack_log[] UTC onset/end timestamps (whole-minute granularity); (3) power_min/max + frequency_band; (4) spoofing attack_parameters.simulated_position (e.g. 69.27701401, 15.969328354, 45 m HAE). 24 scenarios, 31 sessions, 13.65 h attack time. Receiver features: mon_rf.csv (jamInd/jammingState/agcCnt/noisePerMS), nav_pvt.csv (lat/lon/height/hAcc/vAcc/fixType/numSV/pDOP). NOTE candidate's '~129 GB' is wrong — actual ~358 MB tar.gz.
- **Validation recipe:** Download GNSS_DATASET_JAMMING_SPOOFING.tar.gz (~358 MB from record 15911589; gitignore raw). Parse scenario.json labels (exact string match). Classification: 4-class accuracy >=0.80 (macro-F1>=0.75) or binary clean-vs-attack >=0.90. Detection onset: flag within +/-30 s of attack_log onset (whole-minute log, 1 Hz data). Spoof position-error: report ||PVT - simulated_position||, register the jump within 30 s of onset. No explicit clean baseline folder — use pre-onset segments as clean reference; trust structured power_min/max over prose. Receiver-level resilience (UBX/RINEX/nav-solution), not correlator-level.

### [P1 · timing] BIPM Circular T monthly [UTC - UTC(k)] data files

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://webtai.bipm.org/ftp/pub/tai/Circular-T/cirt/ (e.g. cirt.461); format spec https://webtai.bipm.org/ftp/pub/tai/other-products/notes/cirt_format_v0.3.txt; landing https://www.bipm.org/en/time-ftp/circular-t
- **Format:** Plain ASCII fixed-column (.NNN) with MD5/SHA512 checksums + format spec.
- **License:** CC BY 4.0 (https://www.bipm.org/en/copyright) — download/derive/publish derived scalars permitted with attribution; BIPM name/logo protected. Contributed-lab data may carry lab restrictions — gitignore raw.
- **Quantity validated:** [UTC - UTC(k)] offset series in ns at 5-day cadence for ~80 national timing labs, each with BIPM combined standard uncertainty u (k=1); across issues, frequency stability (ADEV/MDEV) of a UTC(k) series vs UTC.
- **Reference oracle:** Two oracles. (1) Deterministic cross-issue overlap (strongest): the same standard MJD endpoint is the first column of one issue and the last column of the previous issue, byte-identical. Verified: MJD 61159 gives OP=0.3 ns, NIST=1.0 ns in BOTH cirt.460 (last col) and cirt.461 (first col). (2) Published per-point uncertainty u (uA/uB/u columns) per the format spec.
- **Validation recipe:** Download two consecutive cirt.NNN (gitignore raw). Parse the fixed-column layout (Labname A4,A21; MJDs I5; UTC-UTC(k) F8.1 ns; uncertainties 3xF5.1). Round-trip oracle: assert EXACT equality (0.0 ns tol) on overlapping MJD endpoints between adjacent issues. Parse-fidelity: reproduce each [UTC-UTC(k)] and u to +/-0.05 ns (half the print resolution). For a derived ADEV/MDEV of a UTC(k) series, gate residual vs BIPM values within published u (k=1) at every common MJD. Attribute BIPM; do not imply endorsement.

### [P1 · timing] IGS / CDDIS precise satellite & station clock products (.clk / .clk_30) incl. IGS Final combined solutions

- **Maps to kernel:** `ADEV/allan`
- **DOI:** 10.5067/GNSS/GNSS_IGSCLK5_001
- **Access:** No-login: https://files.igs.org/pub/product/ (by GPS week); login (free Earthdata): https://cddis.nasa.gov/archive/gnss/products/; mirrors IGN ftp://igs.ensg.ign.fr/pub/igs/products/, SIO ftp://garner.ucsd.edu/pub/products/; oracle table https://igs.org/products/; Earthdata catalog DOI https://doi.org/10.5067/GNSS/GNSS_IGSCLK5_001
- **Format:** RINEX clock (.clk 5-min, .clk_30 30-sec), ASCII, gzip/UNIX-compressed (.Z).
- **License:** IGS open data — 'made openly available for use without restriction... free of cost', covers derived clock products; attribution to IGS + contributing orgs required. CDDIS path requires free Earthdata Login; no-login mirrors available. gitignore raw.
- **Quantity validated:** Satellite/station clock offset series, the RMS difference of an independent product (e.g. Rapid) vs the IGS Final combination, and ADEV/MDEV of satellite atomic clocks over tau from minutes to a day (from .clk_30).
- **Reference oracle:** IGS published product accuracy table (igs.org/products): Final ~75 ps RMS / ~20 ps SDev (Sat 30s, Stn 5min, 12-19 day latency); Rapid ~75 ps RMS / ~25 ps SDev; Ultra-Rapid observed ~150 ps RMS / ~50 ps SDev; predicted ~3 ns RMS / ~1.5 ns SDev. The Final combined product is the consensus ground-truth reference.
- **Validation recipe:** Download a Rapid (igr) and Final (igs) .clk for the same day >3 weeks old (so both exist; gitignore raw). CRITICAL: clock products carry an arbitrary per-file datum (constant + linear) — align to a common reference satellite/station and remove a per-epoch mean BEFORE differencing, or you get ns-level junk instead of ps-level. Compute per-satellite RMS of (Rapid - Final); PASS if <= 150 ps (2x the ~75 ps Rapid figure, allowing datum-alignment residuals), soft target ~75 ps. Secondary: MDEV of .clk_30 satellite clock should be white-FM-like, within factor ~3 of literature Rb/Cs at tau=1 day.

## P2 datasets

### [P2 · anomaly] Server Machine Dataset (SMD) — OmniAnomaly (NetManAIOps, KDD 2019)

- **Maps to kernel:** `AUC/eval_stats`
- **DOI:** 10.1145/3292500.3330672
- **Access:** https://github.com/NetManAIOps/OmniAnomaly/tree/master/ServerMachineDataset
- **Format:** Plain text: train/, test/, test_label/ (binary labels), interpretation_label/.
- **License:** MIT (NetManAIOps). gitignore raw, preserve copyright notice + cite.
- **Quantity validated:** Per-machine and aggregate Precision/Recall/F1 (raw + point-adjusted) and ROC/PR AUC vs binary test_label ground truth.
- **Reference oracle:** ServerMachineDataset/test_label/ (28 files machine-1-1.txt..machine-3-11.txt, one 0/1 per line) = literal labels. Published anchor: OmniAnomaly point-adjusted best-F1 on SMD ~0.886 (POT) / ~0.96 (grid-search), Su et al. KDD 2019. CRITICAL (arXiv 2603.18985): WITHOUT point-adjustment F1 collapses to ~0.2 — point-adjustment heavily inflates scores. 28 machines, 38 dims, 4.16% anomaly ratio.
- **Validation recipe:** Clone repo (gitignore raw in-tree data). Primary self-consistency oracle: assert label vector lengths match test series, aggregate anomaly ratio == 4.16%, computed P/R/F1 deterministic to +/-1e-6 across reruns. Benchmark vs published ~0.88 point-adjusted F1: within +/-0.10 absolute under the SAME point-adjustment + thresholding protocol — do NOT gate VALIDATED on hitting 0.88 exactly. CROSS-DOMAIN server-KPI ONLY: validates generic pipeline mechanics (loading/labelling/scoring), NEVER a PNT/quantum claim.

### [P2 · anomaly] NASA C-MAPSS Turbofan Engine Degradation Simulation Data Set (FD001-FD004 + N-CMAPSS)

- **Maps to kernel:** `AUC/eval_stats`
- **Access:** https://phm-datasets.s3.amazonaws.com/NASA/6.+Turbofan+Engine+Degradation+Simulation+Data+Set.zip (item #6, ~12 MB); N-CMAPSS item #17 (~15.7 GB); landing https://www.nasa.gov/intelligent-systems-division/discovery-and-systems-health/pcoe/pcoe-data-set-repository/; PHM mirror https://data.phmsociety.org/nasa/
- **Format:** Plain-text space-delimited .txt (train/test/RUL); N-CMAPSS as HDF5; zipped.
- **License:** US Govt public domain ('use at own risk'); acknowledge NASA PCoE + donors (ETH Zurich/PARC) + cite Saxena & Goebel 2008. gitignore raw.
- **Quantity validated:** Remaining Useful Life (RUL) prediction error and PHM08 asymmetric score vs supplied ground-truth RUL_FDxxx.txt; for N-CMAPSS, component-level failure-mode labels.
- **Reference oracle:** RUL_FDxxx.txt shipped in CMAPSSData.zip (RUL_FD001.txt = 100 integers, one per test engine, e.g. 112,98,69,82,91...). PHM08 asymmetric scoring (Saxena et al. 2008, paper PDF ships in zip): s=exp(-d/13)-1 for d<0, s=exp(d/10)-1 for d>=0, d=pred-true. N-CMAPSS HDF5 ships per-cycle health-state + component failure-mode labels.
- **Validation recipe:** Download #6 (no auth, gitignore raw). DETERMINISTIC self-consistency (the real VALIDATED test): given a fixed prediction vector, RMSE-vs-RUL_FDxxx.txt and the PHM08 score are pure functions of the labels — assert your scoring implementation reproduces a hand-computed value to atol 1e-9. Secondary literature sanity band: FD001 RMSE ~[10,20]. DOMAIN CAVEAT: turbofan run-to-failure, NOT PNT and NOT point-anomaly labels — validates the SCORING/RUL machinery only; label as cross-domain prognostics, never quantum-PNT.

### [P2 · anomaly] NASA/IMS Bearing Data Set (Univ. Cincinnati Center for Intelligent Maintenance Systems)

- **Maps to kernel:** `AUC/eval_stats`
- **Access:** https://phm-datasets.s3.amazonaws.com/NASA/4.+Bearings.zip (~1.08 GB, no auth); landing https://data.phmsociety.org/nasa/ + NASA PCoE repository
- **Format:** ASCII text vibration files + README documenting failure mode (the oracle).
- **License:** Open/public ('use at own risk'); acknowledge NASA PCoE + IMS Univ. Cincinnati; cite Lee/Qiu/Yu/Lin 2007. gitignore raw.
- **Quantity validated:** Fault-onset detection lead time and fault-type classification vs the documented failure mode + run-to-failure endpoint in the README.
- **Reference oracle:** README documented end-of-test failure modes (one per set): Set 1 = inner-race defect bearing 3 + roller defect bearing 4; Set 2 = outer-race failure bearing 1; Set 3 = outer-race failure bearing 3. Secondary: published health-indicator/fault-onset results (Lee/Qiu/Yu/Lin 2007) report onset well before end-of-test on Set 1 bearing 3.
- **Validation recipe:** Download (gitignore raw). Each file = 1 s, 20,480 pts @ 20 kHz, ~every 10 min. Fault-type: require correct failure-mode label on all 3 sets (3/3 exact). Fault-onset lead time: detector must flag degradation strictly before the final file (>0 lead) and within +/-10% of test duration of the literature-reported Set-1-bearing-3 onset (final ~10-15% of run). Detection-lead-time validation, NOT per-timestamp ROC (coarse labels). CROSS-DOMAIN mechanical — label as such.

### [P2 · anomaly] FEMTO-ST PRONOSTIA Bearing Dataset (IEEE PHM 2012 Prognostics Data Challenge)

- **Maps to kernel:** `AUC/eval_stats`
- **Access:** https://phm-datasets.s3.amazonaws.com/NASA/10.+FEMTO+Bearing.zip (~1.16 GB, no auth); mirror https://github.com/wkzs111/phm-ieee-2012-data-challenge-dataset; platform paper hal.science/hal-00719503
- **Format:** CSV acc_xxxxx.csv (vibration) + temp_xxxxx.csv per bearing; zipped.
- **License:** Open/public ('use at own risk'); cite FEMTO-ST + Nectoux et al. 2012 + NASA PCoE; no DOI. gitignore raw.
- **Quantity validated:** RUL prediction percentage error vs actual end-of-life of each truncated test bearing; IEEE PHM 2012 asymmetric Challenge score.
- **Reference oracle:** Published per-bearing actual RUL (post-challenge FEMTO scoring doc + literature): Bearing1_3=5730s, 1_4=339s, 1_5=1610s, 1_6=1460s. Asymmetric score: Score_i = exp(-ln0.5*Er/5) if Er<=0 else exp(ln0.5*Er/20), Er=100*(ActRUL-RUL)/ActRUL, final = mean of 11. Leaderboard anchor: winning total score 0.28 (Porotsky & Bluvband, ALD Group).
- **Validation recipe:** Download (gitignore raw). Validate the SCORING MACHINERY exactly: feed known (ActRUL, predicted) pairs from the published winner table into your %Er + asymmetric-score functions, require exact match to formula (tol 1e-6) — the formula + ActRUL constants are the true oracle. For a model's per-bearing %Er, match a published pipeline within +/-5 pp; aggregate Score within +/-0.05 absolute. NOTE: actual-RUL oracle is NOT in the zip — source constants from the post-challenge doc/literature (cross-check 2-3 sources). CROSS-DOMAIN proxy.

### [P2 · nav] IGETS Level-1 Superconducting Gravimeter Data — Strasbourg (SG C026 / iOSG#23)

- **Maps to kernel:** `new`
- **DOI:** 10.5880/igets.st.l1.001
- **Access:** https://dataservices.gfz-potsdam.de/igets/showshort.php?id=escidoc:2308899 (DOI 10.5880/igets.st.l1.001); FTP ftp://igetsftp.gfz-potsdam.de/Strasbourg/ (free registration)
- **Format:** ASCII GGP/IGETS .ggp Level-1 files, 1-min sampling + per-station doc PDF.
- **License:** CC BY 4.0. FTP needs free registration. gitignore raw, cite DOI.
- **Quantity validated:** Long-term gravity time-series stability and tidal/atmospheric residual structure; cross-instrument agreement between co-located SG C026 and iOSG#23 as the noise/stability oracle.
- **Reference oracle:** No single scalar ships with the data; oracle is self-derived + companion literature (paywalled). Cross-instrument residual stability over the 2016-2018 parallel window; solid-Earth tide model residual level. Companion papers: Rosat/Hinderer PUAG 2022 (doi:10.1007/s00024-022-03000-4, C026 vs iOSG#23 noise/drift); Rosat & Hinderer SNM/PSD vs Peterson NLNM (Strasbourg crosses NLNM ~1 mHz). Exact pass numbers not pinned (full text blocked).
- **Validation recipe:** Register and download the 1-min .ggp Level-1 gravity + pressure (gitignore raw). Apply tide + atmospheric pressure reduction. Cross-instrument: residual RMS over 2016-2018 parallel window < ~10 nm/s^2 (sub-microGal), +/-30% band pending the exact published figure. SNM/PSD: 5th-percentile PSD tracks the published Strasbourg curve within ~3 dB, crosses NLNM ~1 mHz. Pin exact thresholds only after reading the paywalled companion papers. Terrestrial geodetic gravity (high-stability reference signal), NOT a nav trajectory.

### [P2 · nav] Cold-Atom (CAG) vs Optical FG5 Absolute Gravimeter Comparison — Merlet et al. 2010

- **Maps to kernel:** `CAI/quantum_imu`
- **DOI:** 10.1088/0026-1394/47/4/L01
- **Access:** https://arxiv.org/abs/1005.0357 (PDF https://arxiv.org/pdf/1005.0357v2); journal DOI 10.1088/0026-1394/47/4/L01 (paywalled)
- **Format:** Peer-reviewed paper (Metrologia) + open arXiv preprint PDF (no raw data).
- **License:** arXiv nonexclusive-distrib license (preprint freely accessible; journal paywalled). Commit derived scalar + citation only.
- **Quantity validated:** Measured bias (difference) between a cold-atom absolute gravimeter and the optical FG5 reference, with combined uncertainty.
- **Reference oracle:** LNE-SYRTE CAG vs FG5#220 agreement = 4.3 +/- 6.4 microGal (= 43 +/- 64 nm/s^2), Metrologia 47 (2010) L9-L11. A single published scalar with combined uncertainty — NOT raw data.
- **Validation recipe:** Use as a literature-anchored consistency bound, NOT data processing (no downloadable raw measurement series exists). Hard-code the published 4.3 +/- 6.4 microGal constant; PASS if our modelled atom-vs-optical bias is within the 1-sigma band |bias - 4.3| <= 6.4 microGal (stricter k=2: <= 12.8). Document the convention. Honest label: literature oracle / sanity bound, NOT 'VALIDATED against an external dataset' (it is a hand-transcribed constant, not independent processing).

### [P2 · nav] KITTI Raw Data — OXTS RT3003 RTK-GNSS/IMU Ground Truth

- **Maps to kernel:** `CAI/quantum_imu`
- **DOI:** 10.1177/0278364913491297
- **Access:** https://www.cvlibs.net/datasets/kitti/raw_data.php (free login required); odometry GT https://www.cvlibs.net/datasets/kitti/eval_odometry.php; paper https://www.cvlibs.net/publications/Geiger2013IJRR.pdf
- **Format:** Text/CSV (OXTS GPS/IMU), binary LiDAR, PNG images; devkit provided.
- **License:** CC BY-NC-SA 3.0 — NON-COMMERCIAL + ShareAlike + registration gate. license_ok=FALSE for commercial/ESA-bid context; requires legal sign-off.
- **Quantity validated:** INS/odometry trajectory dead-reckoning drift vs the OXTS RT3003 RTK-GNSS/IMU fused ground-truth trajectory; KITTI-style translational (%) and rotational (deg/m) error over 100-800 m subsequences.
- **Reference oracle:** (1) Per-frame fused OXTS RT3003 pose (oxts text, 30 values/frame); KITTI published spec 0.02 m resolution / 0.1 deg, L1/L2 RTK integer-fix [NOT the candidate's '<10 cm' gloss]. (2) KITTI odometry benchmark GT trajectories sequences 00-10 + published drift metric (translational % and rotational deg/m over 100..800 m subsequences).
- **Validation recipe:** LICENSE BLOCKER FIRST: CC BY-NC-SA 3.0 is NON-COMMERCIAL + ShareAlike with a click-through gate — needs counsel sign-off before use in an ESA/commercial bid even under derived-scalars-only. If cleared: register, download (gitignore raw), run dead-reckoning/odometry, report KITTI metrics — pure-INS short-window (<=100m/~10s) horizontal drift to a few % of path length; VIO-class translational <~2-3%, rotational <~0.01 deg/m. GT is RTK-GNSS-AIDED -> validates dead-reckoning drift ONLY, NOT GNSS-independence. Automotive, not space — log representativeness gaps.

### [P2 · security] Tuni2025 TG-GGSD — Galileo Spoofing Datasets

- **Maps to kernel:** `TPL`
- **DOI:** 10.5281/zenodo.17249902
- **Access:** https://zenodo.org/records/17249902 (DOI 10.5281/zenodo.17249902); descriptor https://doi.org/10.1109/IEEEDATA.2025.3627116
- **Format:** Raw I/Q, interleaved 32-bit float binary, 50 MSps (~30 GB/scenario).
- **License:** CC BY 4.0. gitignore raw .bin, attribute Rahman et al.
- **Quantity validated:** Spoof detection Pd and Pfa vs labelled clean-vs-spoofing split; spoofer-count + spoofed-PRN-set classification.
- **Reference oracle:** Per-scenario labels (no published reference scalar; descriptor defines Pd/Pf/ROC/latency but reports NO benchmark number). 8 Galileo-E1 scenarios: C-1, C-3 clear-sky (clean); SS-1 (1 spoofer), SS-3 (3), SS-5 (5), SS-11 (1+multipath), SS-12 (2+MP), SS-13 (4+MP). Spoofed PRNs deliberately distinct from genuine; each scenario README documents PRN config.
- **Validation recipe:** Download a subset (~30 GB/scenario, ~18 TB all versions — gitignore raw). Requires a working Galileo-E1 BOC(1,1) acquisition/correlation front-end. Binary clean-vs-spoof over the 8 scenarios: require Pd=1.0 AND Pfa=0.0 exactly (clearly-separable lab data). Spoofer-count/PRN-set: exact match to README on >=5/6 spoofing scenarios (allow one ambiguous multipath case). Do NOT claim a numeric Pd 'reference-value' tolerance — none published. Verify the per-PRN README tables on download (full-text was bot-blocked).

### [P2 · security] GNSS Dataset (with Interference and Spoofing) — Part III (Yunnan Univ.)

- **Maps to kernel:** `DOP/gnss`
- **DOI:** 10.17632/nxk9r22wd6.3
- **Access:** https://data.mendeley.com/datasets/nxk9r22wd6/3 (DOI 10.17632/nxk9r22wd6.3); Data in Brief paper https://www.sciencedirect.com/science/article/pii/S2352340924002713 (PMC11220923)
- **Format:** Receiver-observable JSON logs (RINEX-style observables); 1 s cadence.
- **License:** CC BY 4.0. gitignore raw, cite DOI 10.17632/nxk9r22wd6.3.
- **Quantity validated:** Detection Pd (clean vs jammed, clean vs HackRF-spoofed) and clean-vs-attack classification from receiver observables.
- **Reference oracle:** DATE-level scenario labels only (CONFIRMED): clean/normal Sep 12-30 2023; interference+spoofing (HackRF One + commercial jammer) Dec 21 2023. Observables (JSON, 1 s, 5 constellations, 8 bands): C/N0, spectrum, Doppler, pseudorange, carrier phase, sat health, position, DOP. NO published Pd/accuracy scalar; per-second/per-interval label separating jammed-vs-spoofed-vs-normal within Dec 21 NOT confirmed to ship ('irregularly emitted').
- **Validation recipe:** Download Part III (gitignore raw). FIRST inspect the included scripts/README for an attack-interval annotation log. Defensible coarse oracle: observable-feature detector (C/N0 collapse / DOP spike / position jump), clean Sep days vs Dec-21 attack day — require Pd>=0.95, Pfa<=0.05, day-level AUC>=0.95. Do NOT claim sample-level spoof-vs-jam classification as VALIDATED unless a per-interval label is found in the download. Observable-level (good for a feature detector, not IQ/correlator). HackRF replay-class attack relevant to delay/replay detection.

### [P2 · timing] BIPM Circular T time-link tables (TWSTFT & TWGPPP two-way satellite time/frequency transfer)

- **Maps to kernel:** `ADEV/allan`
- **Access:** https://webtai.bipm.org/ftp/pub/tai/timelinks/lkc/ (link .dat + dlk comparisons); paired uStb/uCal oracle in https://webtai.bipm.org/ftp/pub/tai/Circular-T/cirt/cirt.461 Section 5; ReadMe TM137 v12 in lkc/
- **Format:** ASCII 4-column link .dat + dlk comparison data/gif; uStb/uCal in Circular T Section 5 ASCII.
- **License:** CC BY 4.0 (BIPM); lab-contributed data may carry restrictions — gitignore raw, attribute BIPM + contributing labs.
- **Quantity validated:** Two-way satellite time-transfer link offset and stability (TDEV/MDEV) between two labs, and the cross-technique residual (TWSTFT vs GPS-PPP vs TWGPPP on the same baseline).
- **Reference oracle:** Two oracles. (1) Circular T Section 5 per-link uStb/uCal (e.g. CH/PTB TWGPPP uStb=0.3 uCal=1.2; SU/PTB TWSDRR 0.3/3.0; CNES/PTB GPSPPP 0.3/2.6) — published stability bound. (2) lkc per-link: each lnk .dat ships MJD/Link/Smth/Resid (4 ASCII cols) + .gif reporting MDEV/TDEV/residual std-dev (e.g. chptb GPS-P3 Dec2024 Sigma=0.551 ns); each dlk comparison (e.g. opptb.t3ta5 = TWSTFT-GPSPPP) ships cross-technique residual dLink mean/std-dev.
- **Validation recipe:** Download one YYMM link+dlk pair (gitignore raw) and the matching cirt.NNN. Recompute the dLink mean/std-dev from the .dat and assert within +/-0.05 ns or +/-2% of BIPM's printed value (we read the same data). For TDEV/MDEV vs plotted points: agree within ~10-15% per tau. For Section 5 uStb: consistency check only — computed monthly link stability within a factor ~2 of stated uStb (conservative standard uncertainty, NOT a point estimate; never use as exact-match).

### [P2 · timing] GNSS broadcast navigation (BRDC) vs MGEX precise clock - GPS & Galileo clock-error oracle

- **Maps to kernel:** `DOP/gnss`
- **DOI:** 10.3390/s20236832
- **Access:** Broadcast nav: https://cddis.nasa.gov/archive/gnss/data/daily/ (merged BRDM00DLR, DOI 10.5067/GNSS/gnss_daily_multi_b_001); MGEX precise (truth): https://cddis.nasa.gov/archive/gnss/products/mgex; oracle paper https://www.mdpi.com/1424-8220/20/23/6832
- **Format:** RINEX nav (.rnx/.gz) + MGEX RINEX-clock (.clk, .Z/.gz).
- **License:** IGS/MGEX open ('openly shared, without restriction', EOSDIS guidance, citation required); CDDIS Earthdata Login; reference paper CC BY 4.0. gitignore raw.
- **Quantity validated:** Broadcast-vs-precise satellite clock-error time series and its RMS/percentile statistics (SISRE clock component) per constellation.
- **Reference oracle:** Alonso et al., 'Galileo Broadcast Ephemeris and Clock Errors Analysis: 1 Jan 2017 to 31 Jul 2020,' Sensors 2020, 20(23):6832, DOI 10.3390/s20236832 [CORRECTED from candidate's wrong DOI 10.3390/s20247130, which is an unrelated ECG paper; PMC7731340 is correct]. Galileo F/NAV clock error mean -0.3 cm, sigma 21 cm, 68th pct 15 cm, 95th pct 34 cm; GPS LNAV mean -0.7 cm, 68th 31 cm, 95th 97 cm; SISA 3.12 m; NTE=4.42*SISA=13.79 m.
- **Validation recipe:** Download merged BRDC nav + an MGEX final precise clock (document the AC; sampling varies 30s-5min) for a comparable multi-month window (gitignore raw). Follow the paper's methodology (satellite/epoch screening, antenna-phase-center, broadcast-vs-precise reference-point/relativity/TGD handling). Compute broadcast-minus-precise clock error per constellation; compare 68th/95th percentiles within +/-25% rel, sigma +/-25%, mean (~0) +/-2 cm absolute. SISA/NTE are exact definitional constants. Do NOT label VALIDATED on a few-days window vs multi-year aggregates — loosen to +/-40% and document representativeness if short.

---

## Where we stay honestly MODELLED (gaps)

Where NO good public oracle exists, stay honestly MODELLED:

1. SPACE-NATIVE quantum-PNT data. Every 'real' dataset here is a terrestrial/automotive/spacecraft-housekeeping ANALOGUE. There is no public on-orbit atomic-clock-in-space stability series, no spaceborne cold-atom-IMU flight data, and no in-flight quantum-PNT subsystem telemetry. Flight/space representativeness must be logged as a gap (Clar 1/A3 representativeness ledger), not validated.

2. QUANTUM-PNT-SPECIFIC ANOMALY/FAULT labels. The anomaly vertical is covered only by generic spacecraft housekeeping (telemanom, OPSSAT-AD = magnetometer/photodiode) and cross-domain mechanical/turbofan sets (IMS, FEMTO, C-MAPSS, SMD). No public dataset ships labelled faults of a clock or quantum-sensor subsystem. Anomaly-detector performance on a PNT clock/IMU fault stays MODELLED.

3. GRAVITY-AIDED / GNSS-FREE NAVIGATION ground truth. ICGEM gives a gravity-field reference map and IGETS gives high-stability terrestrial gravity, but there is NO public gravity-matching nav trajectory with both a real gravimeter-on-a-moving-platform stream AND a ground-truth path. Gravity-aided nav fix accuracy stays MODELLED; only the map-reader/SH-synthesis code-correctness is VALIDATED.

4. SUPERCONDUCTING-GRAVIMETER / atom-vs-optical stability EXACT scalars. IGETS-Strasbourg's cross-instrument oracle and the AQG/CAG bias numbers live in paywalled or prose-only sources (Merlet 2010 is a single hand-transcribed constant). These support a literature consistency-bound, not independent data-driven validation — keep the underlying instrument MODELS as MODELLED-with-literature-anchor.

5. PUBLISHED Pd/ROC reference numbers for GNSS spoof/jam. TEXBAT/OAKBAT/Tuni2025/Jammertest ship labels + onset, but NO canonical published per-detector Pd/Pfa/AUC scalar to match bit-for-bit (only scattered third-party ds3 AUC). Detection performance is VALIDATED as 'reproduces the labelled separation + onset', NOT as 'matches a published detection probability'.

6. TWSTFT raw-campaign I/Q. BIPM time-link products give processed link offsets + published uStb, but no open raw two-way-satellite I/Q dump exists; the lowest-level two-way-transfer signal processing stays MODELLED.

7. New kernels needed. Gravity-functional (spherical-harmonic synthesis) validation and superconducting-gravimeter noise/PSD validation map to 'new' — no existing kernel (ADEV, CAI, AUC, DOP, TPL) covers them; building these two kernels is the prerequisite to converting the ICGEM and IGETS islands.

## Rejected candidates (and why)

- **T2L2 (Time Transfer by Laser Link, Jason-2) - SLR ranging data via EUROLAS / ILRS** — Quantity mismatch: the access_url (EDC Jason-2 normal-point/full-rate CRD) provides SLR RANGING (two-way time-of-flight to the retroreflector), not the T2L2 onboard-clock time-transfer product. I confirmed this by downloading and parsing a real file -- only ranging/cal/met records, no clock timestam…
- **White Rabbit / two-way fibre time-transfer - published stability references (NO single open labelled dataset)** — No single openly downloadable, labelled White Rabbit time-transfer measurement dataset exists. (1) CERN WR site is an open-hardware project overview, ships specs/gateware, not measurement data deposits. (2) Radio Science DOI 10.1029/2020RS007232 resolves to a real Wiley page but both the DOI redirec…
- **Google Smartphone Decimeter Challenge 2022 — GNSS+IMU with NovAtel SPAN Ground Truth** — LICENSE (primary): Kaggle competition data is under a restrictive competition-use/non-commercial-academic license. Confirmed verbatim Kaggle rules clauses: participants "agree not to transmit, duplicate, publish, redistribute or otherwise provide or make available the Competition Data to any party n…
- **A DATASET for GPS Spoofing Detection on Autonomous Vehicles** — BLOCKING: not accessible to us. (1) IEEE DataPort landing page states verbatim "This dataset requires an IEEE DataPort Subscription to access" - download is gated behind a paid/member login; dblp marks it "Closed access". (2) The lead author's GitHub mirror, the only plausible open copy, has an EMPT…
- **EuRoC MAV Visual-Inertial Dataset** — LICENSE BLOCKS COMMERCIAL USE. Finder claimed "Creative Commons" - this is FALSE/misrepresented. The actual ETH Research Collection rights statement is "In Copyright - Non-Commercial Use Permitted" (rightsstatements.org InC-NC/1.0). Per the canonical definition (rightsstatements.org/page/InC-NC/1.0)…

## Synthesis notes / corrections carried forward

Deduplicated catalog of 31 quantum-PNT validation datasets across four verticals (timing, nav, anomaly, security), built from the verified candidate pool after merging exact duplicates: TEXBAT (2x->1), OAKBAT-GPS (2x->1), BIPM Circular T (2x->1), IGS/CDDIS clock products (2x->1, including the IGS Final combined solutions entry), C-MAPSS (2x->1), Jammertest 2024 (2x->1), and the standalone allantools/NBS14 entry folded into the NBS14 10-point entry it duplicates.

Two grades of oracle exist and the catalog labels both honestly. STRONGEST (known-answer / shipped reference scalar): the NBS14 / Stable32 / Cs5071A timing suite (deterministic ADEV to 1e-4), OPSSAT-AD (peer-reviewed ESA F1/AUC), BIPM Circular T (cross-issue byte-identical overlap + published u), EuRoC (mm-level ground-truth trajectory + sensor.yaml noise model), TEXBAT/OAKBAT (time-stamped clean/spoof segmentation). WEAKER (literature constant or self-labels, not independent data processing): Merlet 2010 (a single transcribed scalar — explicitly NOT 'VALIDATED against a dataset'), the broadcast-vs-precise clock paper, IGETS (paywalled companion-paper oracle).

CORRECTIONS carried forward from verification (do not propagate the original errors): (1) BRDC/MGEX paper DOI is 10.3390/s20236832, NOT the candidate's 10.3390/s20247130 (an unrelated ECG paper). (2) telemanom license is BSD-3-Clause not Apache-2.0, and its '0.71' is Fbeta=0.5 not F1. (3) NBS14 10-point HTOTDEV=91.16396 not the mislabeled OHDEV value. (4) Tuni2025 GPS data is at Zenodo 15623217, not the Galileo record 15470143 the candidate cited. (5) Jammertest data is at Zenodo 15911589 (~358 MB), not the README-only 15911359 / claimed ~129 GB. (6) KITTI is CC BY-NC-SA 3.0 NON-COMMERCIAL — flagged license_ok=false, needs counsel sign-off for the ESA/commercial bid. (7) OAKBAT's github.com/oakbat is empty — oracle comes from the ION GNSS+ 2020 paper, not the repo.

Kernel mapping: timing stability -> ADEV/allan (7 datasets); GNSS clock/observable error -> DOP/gnss (4); quantum-IMU/gravimeter & INS drift -> CAI/quantum_imu (5); anomaly scoring -> AUC/eval_stats (6); GNSS spoof/jam detection -> TPL (8); gravity-map nav reference and superconducting-gravimeter stability -> 'new' (2, no existing kernel fits a spherical-harmonic gravity-functional or SG-noise oracle).

Cross-cutting policy for every dataset: gitignore raw data, commit only derived scalars + the validation test, cite the source. Honesty constraint enforced throughout: server-KPI (SMD), turbofan (C-MAPSS), and bearing (IMS, FEMTO) sets are CROSS-DOMAIN — they validate the scoring/RUL/detector machinery only and must never be labelled quantum-PNT validation; OAKBAT/TEXBAT/Tuni 'VALIDATED' means reproducing the labelled clean/spoof separation + onset, not matching a published Pd number (none exists).

---

*Ashforde OÜ · contact@ashforde.org · D2 validated-island expansion · raw data gitignored, derived
scalars + tests + fetch scripts only · VALIDATED requires an external oracle, else MODELLED.*
