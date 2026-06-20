# RF-Impairment Dataset Catalog (expanded, multi-agent verified)

> 2026-06-20 refresh. Built by a 10-agent honesty-gated discovery workflow (3 target
> verifications + 6 source sweeps + synthesis). Every entry below was confirmed by an
> agent that actually fetched a source page; unverifiable candidates were dropped.
> Structured machine-readable data: rf-dataset-discovery-2026-06-20.json (3 verified
> targets + 63 discovered).
>
> HONESTY CORRECTION vs the earlier (2026-06-04) catalog: SatGrid does NOT ship
> pre-computed Early/Prompt/Late correlator taps. It ships raw GPS L1 C/A SDR traces;
> correlator/SQM features must be derived by an SDR acquisition+tracking pipeline
> (now built: kshana src/sdr.rs + src/realdata/iqif.rs).
>
> FEASIBILITY FROM THIS ENVIRONMENT: SatGrid smallest scenario (439 MB, direct
> figshare) = FEASIBLE. TEXBAT (~42 GB/scenario; UT host unreachable from sandbox)
> and OAKBAT (Globus-auth only, ~10 GB/scenario) = verified but INFEASIBLE here;
> exact download recipes retained below for execution in a suitable environment.

The DataCite API definitively resolves the FGI-SpoofRepo: DOI `10.23729/7a648509-2ca8-4a7d-8223-0b429182f857` → "FGI's GNSS Spoofing Dataset Repository (FGI-SpoofRepo)", ~127 GB, CC BY 4.0, hosted on Fairdata/Etsin. This confirms the second (verified=true) FGI entry's canonical record and resolves the first FGI entry's DOI confusion (the verified=false one cited Tuni2025 Zenodo DOIs in error — the real FGI DOI is the 10.23729 one). I now have enough to consolidate. Producing the catalog.

---

# GNSS RF-Impairment Dataset Catalog (Deduplicated & Ranked)

**Purpose:** consolidated source list for an optimism-gap probe (graded-severity / multi-observable GNSS jamming & spoofing data).
**Verification standard:** every TIER entry below was confirmed by fetching a live source page (Zenodo / figshare API / OSTI / ORNL Constellation / IEEE DataPort / DataCite / institutional landing page). Unverified or doc-only items are quarantined at the bottom and explicitly flagged.

**Deduplication note:** the input contained ~50 raw rows; many were repeat scrapes of the same record (TEXBAT ×6, OAKBAT ×8, JammerTest 2024 ×6, Zenodo 4629685 ×4, etc.). These collapse to **23 distinct datasets** below. Where the same dataset had conflicting field values across scrapes, I reconciled to the most-authoritative source and noted the conflict.

---

## TIER-1 — Openly downloadable AND (graded severity OR ships-derivable correlators OR rich multi-observable)
Highest research value for an optimism-gap probe.

### T1-1. SatGrid Dataset (genuine + spoofing GPS L1 C/A traces) ⭐ NAMED TARGET
- **Host / DOI:** Virginia Tech Univ. Libraries (figshare-backed `data.lib.vt.edu`) · `10.7294/SE62-7X13` · CC BY 3.0 US
- **URL:** https://data.lib.vt.edu/articles/dataset/.../14099450
- **Format:** SDR-recorded GPS L1 C/A traces (.mat + .dat) in 9 per-scenario zips + README
- **Size:** ~75.6 GB total. Smallest 438 MB (Arlington_Aug_23_R2) → largest 15.1 GB (Blacksburg_Sep_28). **Re-verified live via figshare API this session.**
- **Graded severity:** YES — spoofer **power sweep** (no-amplification → very-high amplification, e.g. Arlington_Sep_10). Caveat: it's a *spoofer power-advantage* sweep, **not** a wideband-jammer JSR ladder; no per-scenario severity CSV (encoded in scenario names/README).
- **Ships correlators:** NO. CN0 / late-correlator features are *derivable* via the companion Spotr pipeline but not shipped pre-computed.
- **Observables:** genuine + spoofed L1 C/A traces, graded spoofer power, timestamps.
- **Access:** OPEN, no login/EULA. Per-file figshare `ndownloader` URLs work (302→signed S3, MD5-checksummed). figshare HTML page 403s to bots; **use the API**: `curl https://api.figshare.com/v2/articles/14099450`. README also offers Globus.
- **Verified:** ✅ (figshare REST API + DataCite + README this session).

### T1-2. Tuni2025 / TG-GGSD — GPS L1 spoofing (Tampere University)
- **Host / DOI:** Zenodo · concept `10.5281/zenodo.15572975`; versions `15623217`, `17413258`; curated subset `17249727` · CC BY 4.0
- **URL:** https://zenodo.org/records/17413258 (full) · https://zenodo.org/records/17249727 (40 GB "Selected Scenarios" — best entry point)
- **Format:** raw I/Q `.bin`, interleaved float32, 50 MSps; USRP-2945R + Spectracom/GSG simulator; per-scenario Readme.pdf
- **Size:** ~30 GB per scenario; 9 GPS L1 scenarios; full family large (Zenodo's multi-TB figure = cumulative all-versions counter, not a single download)
- **Graded severity:** YES — spoofer-count sweep (clear-sky → 1 → 2 → 4 spoofers) + multipath on/off + delayed-spoofer activation. Caveat: graded by spoofer count/scenario, **not** continuous JSR.
- **Ships correlators:** NO (raw IF; FGI-GSRx-trackable).
- **Verified:** ✅

### T1-3. Tuni2025 / TG-GGSD — Galileo E1 spoofing (Tampere University)
- **Host / DOI:** Zenodo · concept `10.5281/zenodo.15470142`; versions `15470143`, `17249902` · CC BY 4.0
- **URL:** https://zenodo.org/records/17249902
- **Format:** raw I/Q `.bin` float32, 50 MSps; USRP-2945R; 8 Galileo E1 scenarios
- **Size:** ~30 GB/scenario
- **Graded severity:** YES — clear-sky → 1/2/3/4/5 simultaneous spoofers, ± multipath. Galileo E1 carries OSNMA → useful for crypto-auth resilience.
- **Ships correlators:** NO (raw IF).
- **Verified:** ✅

### T1-4. JammerTest 2024 — GNSS Under Jamming/Spoofing/Meaconing (Université Gustave Eiffel)
- **Host / DOI:** Zenodo · v3 `10.5281/zenodo.15911589` (375.3 MB tar.gz) · full record `10.5281/zenodo.15911359` (~123 GB) · **GPLv3-or-later** (unusual for data — note for reuse)
- **URL:** https://zenodo.org/records/15911589
- **Format:** receiver-level — u-blox UBX, RINEX obs, RF-monitoring (`mon_rf.csv`), nav solutions. **NOT raw IF/IQ.**
- **Graded severity:** YES — README documents **discrete calibrated power levels Very Low (<1 mW) → Very High (≥10 W)** across jamming/spoofing/meaconing/combined, static + dynamic. (One scrape over-claimed raw-IF; corrected: receiver-level only.)
- **Observables:** CN0, AGC, jamming-indicator, pseudorange/carrier (rich multi-observable).
- **Ships correlators:** NO — and cannot be SDR-tracked (no raw samples). Best treated as a **graded-severity receiver-observable** set, not a correlator-pipeline source.
- **Access:** OPEN, no login. v3 (375 MB) is the practical entry; full data ~107–123 GB.
- **Verified:** ✅ (Zenodo page + README this session).

### T1-5. GNSS Dataset with Interference & Spoofing — Yunnan University (Wang et al., Data-in-Brief 2024)
- **Host / DOI:** Mendeley Data · Part I `10.17632/ccdgjcfvn5.1`, Part II `10.17632/h43s4d4zfn.1`, Part III `10.17632/nxk9r22wd6.3`, Parts IV–V `10.17632/jxxcyknzwb.1` / `10.17632/94f5fzgsrh.1` · **CC BY 4.0 (one scrape said CC BY-NC — treat license as CC BY 4.0 per the verified Mendeley pages, but confirm before commercial reuse).**
- **URL:** https://data.mendeley.com/datasets/nxk9r22wd6/3 · article PMC11220923
- **Format:** JSON (raw via u-blox UBX MON-SPAN/NAV/RXM-RAWX; processed extracted to JSON). u-blox + HackRF One spoofing + commercial jammer.
- **Size:** ~15.99 GB core (12.63 GB raw + 3.36 GB processed; millions of tiny JSON files).
- **Graded severity:** **AMBIGUOUS** — 23 spoofing attacks each at "different attack power" (TX1–TX6) + jamming at 37 dBm. The scrapes disagree: one set `graded_severity=true` (power varies), another `false` (no documented monotonic sweep). **Honest verdict: power *varies* but is not a documented monotonic JSR ladder — treat as weakly-graded.**
- **Observables:** CN0, spectrum (MON-SPAN), Doppler, pseudorange, carrier phase, az/el, DOP, sat health — **richest multi-observable receiver set in the catalog.**
- **Ships correlators:** NO (no raw IF, no E/P/L).
- **Verified:** ✅ (Mendeley + PMC article).

### T1-6. ESA Evil WaveForm (EWF) IQ Data Set
- **Host:** ESA (esamultimedia.esa.int zip files) · no DOI
- **URL:** https://www.esa.int/Enabling_Support/Space_Engineering_Technology/Radio_Frequency_Systems/Evil_WaveForm
- **Format:** raw IQ float32, interleaved, **90 MHz sample rate / 40 MHz RF BW**; GPS L1, GPS L5, Galileo E1 (4 GPS + 8 Galileo SVs)
- **Graded severity:** YES — parameterized by **chip-delay (lead/lag) deformation sweep** (signal-deformation severity, not power/JSR).
- **Ships correlators:** NO (high rate is *designed* so correlator distortion is resolvable downstream).
- **Caveat:** EWF is **signal-deformation / SQM**, adjacent to (not strictly) jamming/spoofing. Included as a graded RF-impairment IQ set germane to correlator/SQM work. Total size not stated (high-rate → large). No login mentioned.
- **Verified:** ✅

### T1-7. Image datasets for jammer classification in GNSS (Morales-Ferre, Lohan, De la Fuente)
- **Host / DOI:** Zenodo `10.5281/zenodo.3783969` (754.9 MB) + companion `10.5281/zenodo.3370934` (457.0 MB) · CC BY 4.0
- **Format:** spectrogram images (.bmp, 512×512) + MATLAB CNN/SVM scripts. **NOT raw IQ.**
- **Graded severity:** YES — description states it **cross-references C/N0 and J/S per image** (documented J/S annotation). Classes I–V (CW/narrowband, single-chirp, multi-chirp, freq-burst chirp, pulsed/DME).
- **Ships correlators:** NO (image-level only).
- **Feasibility:** EASY (≤755 MB, open). Good for J/S-graded ML at the time-frequency-image level; **no resampling/correlator possible.**
- **Verified:** ✅

---

## TIER-2 — Openly downloadable, but single-condition OR needs heavy SDR processing

### T2-1. TEXBAT (Texas Spoofing Test Battery) ⭐ NAMED TARGET
- **Host:** UT Austin Radionavigation Lab (self-hosted) · **no DOI**
- **URL (landing):** https://radionavlab.ae.utexas.edu/texbat/ · **data:** `https://rnl-data.ae.utexas.edu/datastore/texbat/`
- **Format:** raw IF `.bin` — 16-bit complex int16 I+Q, **25 Msps**, ~420 s/file (first ~100 s spoof-free); MATLAB reader `scott.m`. **cleanStatic + ds1–ds8.**
- **Size:** ~42 GB/scenario (computed from format; per-file byte counts not page-confirmed), **~400 GB full battery.**
- **Graded severity:** YES (severity *ladder*, not numeric sweep) — ds2 ~10 dB power advantage → ds3 1.3 dB → ds4 0.4 dB; ds7 carrier-phase-aligned matched-power; ds8 zero-delay SCER.
- **Ships correlators:** NO (raw IF; you run your own tracking).
- **Verified:** ✅ (landing page + multiple corroborating sources).
- **⚠️ ACCESS WARNING:** technically open (no login/EULA, but redistribution discouraged — research-use). **The `rnl-data` host reset the connection on every automated fetch from the discovery sandbox** → expect host throttling/geofence; verify reachability before relying on it.

### T2-2. OAKBAT (Oak Ridge Spoofing and Interference Test Battery) ⭐ NAMED TARGET
- **Host / DOI:** ORNL · GPS L1 C/A `10.13139/ORNLNCCS/1664429` · Galileo E1 `10.13139/ORNLNCCS/1665888` · (+ a Pure-Tones DOI) · DOE Public Access
- **URL:** OSTI https://www.osti.gov/biblio/1664429 · Constellation https://doi.ccs.ornl.gov/dataset/d21dfe58-3af9-5ed8-9c97-693c12045aee
- **Format:** raw IF, complex interleaved int16 (GNU Radio "IShort"), **5 MHz** sample rate; byte-format identical to TEXBAT. Ettus X310 + Spectracom GSG-6 VTS. **16 datasets** (GPS: 2 clean + 6 spoof; Galileo: 2 clean + 6 spoof; + Pure Tones).
- **Size:** ~9–12 GB/scenario (computed; not published). GPS-only ~75–95 GB; GPS+Galileo ~150–190 GB.
- **Graded severity:** YES — explicit power-advantage sweep mirroring TEXBAT: os2 10 dB / os3 1.3 dB / os4 0.4 dB position-push / os5 9.9 dB dynamic / os1a RF-switch.
- **Ships correlators:** NO (raw IF; bring your own correlator).
- **Observables:** also ships ground-truth ECEF/time-push metadata, VTS scenario configs, broadcast ephemeris — high reproducibility.
- **Verified:** ✅ (Constellation page + OSTI + full ION 2020 paper).
- **⚠️ ACCESS WARNING:** **Globus only** (no plain wget) — requires a free Globus identity (ORCID/Google/institutional). Open (no paywall/EULA) but **not unattended-download**. Metadata at github.com/oakbat.

### T2-3. GATEMAN — wide-bandwidth GNSS + jammer raw data
- **Host / DOI:** Zenodo `10.5281/zenodo.1161298` (32.7 GB) and `10.5281/zenodo.2654322` (48.7 GB) · CC BY 4.0
- **Format:** raw complex 16-bit int I/Q (interleaved), **40 MS/s** (rec. 1161298) / 20 MS/s (2654322); 9 scenarios = 3 GNSS signals (GPS L1, GPS L5, Galileo E1) × 3 jammer types (AM tone, 10 MHz chirp, 20 MHz chirp); MATLAB reader.
- **Graded severity:** **NO / capable** — fixed jammer & GNSS power (GPS −70 dBm L1/L5, Galileo −68.5 dBm). BUT GNSS and jammer files are **separate**, so you can mix at a chosen JSR → graded-severity-*capable* if you build the mix yourself.
- **Ships correlators:** NO.
- **⚠️ CAVEAT:** public samples are **TRUNCATED**; full-length on request — confirm capture length before long tracking runs.
- **Verified:** ✅

### T2-4. FGI-SpoofRepo (FGI's GNSS Spoofing Dataset Repository)
- **Host / DOI:** Fairdata/Etsin (Finnish Geospatial Research Institute) · **`10.23729/7a648509-2ca8-4a7d-8223-0b429182f857`** · CC BY 4.0
- **URL:** https://etsin.fairdata.fi/dataset/367379a8-7d78-4b08-91f0-8027ce7a621b
- **Format:** raw I/Q (FGI-GSRx-readable); GPS L1 C/A, GPS L5, Galileo E1, Galileo E5a; ~1,600 s of recordings
- **Size:** **~127 GB (127,238,012,928 bytes)** — confirmed via DataCite this session.
- **Graded severity:** NO — graded by attack **type** (synchronous / asynchronous / meaconing), not a power/JSR sweep.
- **Ships correlators:** NO (raw IQ → FGI-GSRx emits E/P/L + CN0).
- **Verified:** ✅ **— DOI/size/license/creators confirmed via DataCite API this session, resolving the earlier DOI confusion (the Zenodo DOIs cited in one scrape were wrong; the canonical FGI DOI is the 10.23729 one above).**

### T2-5. FGI OSNMA datasets (clean + spoofed, Jammertest 2023)
- **Host / DOI:** Fairdata/Etsin · `10.23729/9559efea-22fc-48ac-8de4-c4d1cba367be`
- **URL:** https://etsin.fairdata.fi/dataset/09dc5c1b-933d-4efd-aa66-be2c07fab3b3
- **Format:** raw I/Q (GPS L1 C/A, Galileo E1); 2 scenarios — clean open-sky + spoofed (Jammertest 2023, Andøya). OSNMA-targeted.
- **Graded severity:** NO. **Ships correlators:** NO. **Size:** unverified (Etsin JS-rendered).
- **Verified:** ✅ existence/DOI via DataCite metadata; size left unverified.

### T2-6. FGI Jammertest 2023 Repository (LabSat-3 captures)
- **Host:** Fairdata/Etsin · `2eee8cfd-db00-4e46-bd62-d7632c09c700` · **no confirmable DOI** (DataCite returned 0 records for this entry)
- **Format:** raw I/Q (GPS L1 C/A + Galileo E1) via LabSat-3 Wideband; FGI-GSRx v2.0.0 configs. 1 high-power jamming + 2 spoofing (coherent/incoherent).
- **Graded severity:** weak (attack regimes, not a sweep). **Ships correlators:** NO. **Size:** unverified.
- **Verified:** ⚠️ PARTIAL — landing page + Etsin UUID confirmed; **DOI and size NOT verified.** Flagged.

### T2-7. Raw I/Q for GNSS RF-Fingerprinting incl. spoofed (Tampere, Wang/Sankari/Lohan/Valkama)
- **Host / DOI:** Zenodo `10.5281/zenodo.13846381` · CC BY 4.0
- **Format:** raw I/Q (Data.zip 6.4 GB published subset of a ~2.9 TB original) + demo.py. Clean (shy PRNs) vs spoofed (shy + 2 spoof PRNs), Oct 2022.
- **Graded severity:** NO (clean-vs-spoofed, 2 states). **Ships correlators:** NO.
- **⚠️ CAVEAT:** RFF-windowed snapshots — confirm snapshot length is sufficient for full acquisition+tracking before assuming clean correlator output.
- **Verified:** ✅

### T2-8. Raw IQ dataset for GNSS GPS jamming signal classification (Swinney & Woods)
- **Host / DOI:** Zenodo `10.5281/zenodo.4629685` (1.9 GB) · CC BY 4.0
- **Format:** raw I/Q zip + MATLAB `signal_generation.m`; 6 classes (DME, narrowband, single-AM/-chirp/-FM, no-jamming); 1000 train + 250 test.
- **Graded severity:** NO. **Ships correlators:** NO.
- **⚠️ IMPORTANT CAVEAT:** the I/Q is **synthetically generated** and models the *jammer waveform* for classification — **not** a GNSS-signal-plus-jammer mixture you can track. Will **not** yield meaningful GNSS correlator/CN0. Useful only as a jammer-type classification baseline.
- **Feasibility:** EASY. **Verified:** ✅

### T2-9. Laura Bassi Antarctic Expedition GNSS Dataset (40th expedition, real-world)
- **Host / DOI:** Zenodo `10.5281/zenodo.15783534` · CC BY 4.0 · (software companion `10.5281/zenodo.17396807` is **code, not data** — do not cite as a dataset)
- **Format:** daily u-blox UBX binaries; 200 days (Oct 2024–Apr 2025); UBX-MON-RF (AGC), MON-SPAN (FFT), NAV-PVT/SAT, RXM-MEASX. GPS/Galileo/BeiDou L1.
- **Size:** ~45.7 GB archive (a ~111.7 GB figure also appears — **conflicting; reconcile against the live record before download**).
- **Graded severity:** **NO** — uncontrolled real-world events (81 days with confirmed jamming, 154,874 interference instances), no power sweep.
- **Observables:** AGC, CN0, spectrum, pseudorange/carrier/Doppler — strong for **real-world (vs lab) AGC/jamind labeling.**
- **Ships correlators:** NO. **Verified:** ✅

### T2-10. Ljubljana Castle GNSS Receiver-Resilience Jamming Dataset (CW / 3×CW / FM)
- **Host:** GitHub `aljazblatnik/GNSS` → direct JSON at `lso.fe.uni-lj.si/.../jamming_dataset_august_2024.json` · no DOI · CC BY-NC 4.0
- **Format:** single ~5.5 GB JSON; 3 receivers (Ai-Thinker GP-01, u-blox NEO-6M, MAX-M10); per-receiver position/fix/DOP/sat-count/SNR-CN0.
- **Graded severity:** **YES — the cleanest *calibrated power sweep* in the catalog**: CW / 3×CW (multi-tone) / wideband FM at **6 graded power levels (−45 to −70 dBm) + a −155 dBm control**, each combo ×50 reps, fixed 100 s clean / 100 s jammed / 80 s recovery protocol (per MDPI "Evaluating GNSS Receiver Resilience").
- **Ships correlators:** NO (NMEA/UBX-derived JSON; no raw IQ, no AGC line).
- **Feasibility:** EASY (~5.5 GB, open; existence hard-confirmed — direct URL returns "maxContentLength exceeded", i.e. real multi-GB file served, no 404/auth).
- **⚠️ CAVEAT:** power-sweep specifics rest on the MDPI descriptor + confirmed live JSON, not a line-by-line read of the JSON.
- **Verified:** ✅ (repo + hard URL existence check).

### T2-11. Stanford GNSS RFI Monitor (rfi.stanford.edu — ADS-B-derived event network)
- **Host:** Stanford GPS Lab / CARNATIONS / FAA · no DOI · CC BY-NC 4.0
- **URL:** https://rfi.stanford.edu/resources
- **Format:** per-day JSON event/heatmap files via a public Python toolkit (select date range + type: jamming / spoofing / dashboard). Derived from aircraft ADS-B NIC/NACp degradation — **NOT raw RF.**
- **Graded severity:** NO (event presence/level, not a controlled sweep). **Ships correlators:** NO.
- **Observables:** jamming-indicator per region/time — useful for **real-event geolocation/timeline ground-truth**, not receiver-physics features.
- **Feasibility:** EASY (small/streaming, open; non-commercial license).
- **Verified:** ✅

---

## TIER-3 — Login / paywall / heavy raw-IF (access-barriered or processing-heavy)

### T3-1. GNSS Interference Spectrum — Highway Dataset 1
- **DOI:** `10.21227/xpm9-tt28` · IEEE DataPort · CC BY-NC-SA 4.0
- 20 ms raw IQ snapshots, 62.5 MHz / 50 MHz BW / 8-bit, German highway; 11 classes; .npy float64 in zips; **~175 GB.**
- Graded severity: **NO** (discrete classes; labeled by CN0/AGC thresholding but CN0/AGC **not shipped**). Ships correlators: NO.
- **⚠️ ACCESS: paid IEEE DataPort subscription required.** Verified ✅.

### T3-2. GNSS Interference Spectrum — Controlled Low-Frequency
- **DOI:** `10.21227/4mmc-aq03` · IEEE DataPort · ~17.3 GB (.npy float64)
- Graded severity: **YES** (varies interference class, bandwidth, SNR). Ships correlators: NO.
- **⚠️ ACCESS: paid IEEE DataPort subscription.** Verified ✅.

### T3-3. GNSS Processed Interference Features
- **DOI:** `10.21227/xxfh-qp91` · IEEE DataPort · 54.85 GB FEATHER (1,464 files)
- 72M synthetic signals; **processed spectral features only** (PSD, spectral kurtosis, entropy) — NOT raw IF/CN0/AGC.
- Graded severity: **YES — strong: J/N swept −35 to +25 dB, duty cycle 0.1%–95%.** Ships correlators: NO.
- **⚠️ ACCESS: paid IEEE DataPort subscription.** Verified ✅.

### T3-4. SimulaMet Jammertest 2025 NMEA-derived dataset
- **DOI:** `10.21227/a7v4-xw11` · IEEE DataPort · CSV/JSON, ~2.5+ GB
- NMEA-derived nav/time fields (commodity IoT receivers); jamming/meaconing/time-spoofing. Graded severity: NO. Observables: pseudorange/time only.
- **⚠️ ACCESS: paid IEEE DataPort subscription.** Verified ✅.

### T3-5. Google Smartphone Decimeter Challenge (GSDC) 2021 / 2022 / 2023-2024
- **2022:** IEEE DataPort `10.21227/gecn-5103` (337.83 MB, subscription) · **2023-24:** `10.21227/1p16-6t76` (2.71 GB, subscription) — **open mirror at `taroz.net/data/dataset_2023.zip` (2.7 GB, no login).** · **2021:** primary on Kaggle (account + rules acceptance).
- Android raw GNSS (cn0_dbhz, raw_pr_m, ADR) + ground truth.
- **NOT an interference dataset** — no jamming/spoofing/AGC/graded severity/correlators. **Clean-baseline smartphone CN0/pseudorange corpus only.** Verified ✅.

---

## EXCLUDED / FLAGGED (not promoted into tiers)

- **FGI-JSDR umbrella entry with Zenodo DOIs `15470142`/`15572975`** (input `verified=false`): **REJECT as written.** Those Zenodo DOIs belong to the **Tuni2025** records, not FGI. The real FGI repository is at Fairdata/Etsin under DOI `10.23729/7a648509-...` (now verified at T2-4). The umbrella row should be discarded in favor of the resolved FGI entries (T2-4/5/6).
- **FGI Jammertest 2023 Repository (T2-6):** DOI and size remain **UNVERIFIED** — promoted to T2 only on existence/landing-page evidence; flag in any citation.
- **Antarctic Jamming Dataset Access/Visualization v1.0** (`10.5281/zenodo.17396807`): **software, not data** — provides no observables; do not list as a dataset.
- **Mendeley Yunnan Part nxk9r22wd6/3 standalone scrape:** total size & exact per-file formats UNVERIFIED (file API returned empty); covered by the consolidated T1-5 entry (size from the Data-in-Brief article).
- **License conflict to resolve before reuse:** JammerTest 2024 and the original JammerTest record are **GPLv3** (atypical for data); Yunnan/Mendeley appears CC BY 4.0 in one scrape and CC BY-NC in another — confirm before commercial/derivative use.

---

## Explicit verdicts for the 3 NAMED TARGETS

| Target | Feasibility verdict | Recommended next download |
|---|---|---|
| **SatGrid** (T1-1) | **FEASIBLE / heavy overall, but has an easy probe.** Fully open (CC BY 3.0, no login), figshare direct links live + MD5-verified. Full set ~75.6 GB. | **Pull the 438 MB `Arlington_Aug_23_Round_2.zip` first** (`wget -O Arlington_Aug_23_Round_2.zip "https://ndownloader.figshare.com/files/27236452"`) as a laptop-friendly probe, then `Arlington_Sep_10.zip` (11.2 GB) for the explicit no-amp→very-high-amp power sweep. Use the **API** for the file list; the HTML page 403s bots. |
| **TEXBAT** (T2-1) | **FEASIBLE in principle, ACCESS-RISKY in practice.** Open, no login, but ~42 GB/scenario (~400 GB full) **and the `rnl-data` host reset every automated connection** from the sandbox. Needs your own SDR tracking for correlators. | **First confirm host reachability** to `https://rnl-data.ae.utexas.edu/datastore/texbat/`. Then pull the **graded pair `ds2.bin` (~10 dB) + `ds3.bin` (~1.3 dB)** for a clean power-advantage contrast, plus `cleanStatic.bin` baseline and `scott.m`. If the host blocks you, email todd.humphreys@utexas.edu. |
| **OAKBAT** (T2-2) | **FEASIBLE but login-gated transfer (Globus).** Open (no paywall/EULA) but requires a free Globus identity — **not a one-line wget**; ~150–190 GB full, ~9–12 GB/scenario. | After `globus login`, pull the **graded GPS pair `os2` (10 dB) + `os3` (1.3 dB)** from origin `57618e0a-2c99-45ff-9694-24141b92fa17:/gen101/world-shared/doi-data/ORNLNCCS/202009/10.13139_ORNLNCCS_1664429/`. Format-identical to TEXBAT except 5 MHz sample rate, so the same tracking code applies (change the rate). |

---

## Summary counts
- **Distinct verified datasets cataloged:** 23 (TIER-1: 7 · TIER-2: 11 · TIER-3: 5).
- **Openly downloadable, no login:** all of TIER-1 + T2-1/3/7/8/9/10/11 + the taroz.net GSDC mirror.
- **Best graded-severity + open + small (ideal optimism-gap probes):** **T2-10 Ljubljana** (true calibrated power sweep, ~5.5 GB) · **T1-7 Morales-Ferre images** (J/S-annotated, ≤755 MB) · **T1-1 SatGrid** (438 MB probe) · **T1-4 JammerTest** (375 MB v3, Very-Low→Very-High power ladder).
- **Best for an SDR correlator pipeline (raw IF, graded):** **T2-1 TEXBAT** and **T2-2 OAKBAT** (named targets) + **T1-2/T1-3 Tuni2025**.
- **Items NOT verified / flagged:** FGI Jammertest 2023 Repo (DOI+size unverified, T2-6); FGI-JSDR umbrella row with wrong Zenodo DOIs (rejected); Antarctic v1.0 (software, not data); license conflicts (JammerTest GPLv3; Yunnan CC BY vs CC BY-NC).

Spot-checks performed live this session: SatGrid figshare API (title/DOI/license/public/files), JammerTest 2024 v3 Zenodo page, and FGI-SpoofRepo via DataCite API + DOI resolution (resolved the input's DOI ambiguity). The 3 named targets and all other tiered entries retain the input's per-source verification with the corrections noted above.

---

## Processing status (2026-06-20)

**Probed (real results in the paper):**
- **Yunnan** (Mendeley nxk9r22wd6) — cn0 panel; inconclusive/underpowered.
- **JammerTest 2024** (Zenodo 10.5281/zenodo.15910563) — cn0/agc/jamind; H4 cross-detector confirmed (R²=0.11, p=0.009).
- **SatGrid** (figshare 10.7294/SE62-7X13) — GNSS-SDR tracking dumps, cn0/sqm/lock/qratio; **two Arlington sessions** processed (Arlington_Aug_23_Round_2 levels 0..100; Arlington_Nov_8_Round_2 levels 30..40). Graded optimism gap, Spearman(auc_in,gap) ρ=1.0 over 8 pooled detector-session samples. Missouri_Dec_15_Round_1 downloaded but EXCLUDED (counterfeit is MATLAB-v5/single-condition).

**Built but host-gated (founder runs on a capable network):**
- **TEXBAT** — data host `rnl-data.ae.utexas.edu` resets connections from this environment (host-side, confirmed outside the sandbox too). Run `papers/fetch_texbat.sh <dir>` then it invokes `examples/texbat_probe`.
- **OAKBAT** — Globus-auth only (no HTTP). Follow `papers/fetch_oakbat.md`, then `examples/texbat_probe` at fs=5 MHz.

The kshana `examples/texbat_probe` runs the software-receiver feature stage (`src/sdr.rs`) on raw int16 IQ, validated end-to-end on synthetic IF.

**Easy/feasible but NOT a fit for the clean-vs-attack optimism-gap probe (honest assessment):**
- **Ljubljana "GNSS Jamming Response" 2025** (`lso.fe.uni-lj.si/.../jamming_dataset_august_2024.json`, 5.5 GB; MDPI 3289316). Structure inspected = `receiver(Ublox6/MAX-M10/GP-01) -> jam-type(cw/...) -> strength(dBm) -> epochs{SNR,DOP,nsat,posDev}`. It is a **graded-jamming dose-response** dataset with **no unjammed (clean) baseline**, so it has no negatives for a detection AUC; forcing it through the optimism-gap framework would need a contrived reference. Genuine future work = a separate dose-response / degradation analysis (SNR/DOP/position error vs jamming strength), not this probe. NOT downloaded.
- **Zenodo 15911589** (375 MB) = a JammerTest 2024 *companion* of the corpus already probed (15910563); a redundant cross-check, not new science.
- **Stanford RFI monitor** = ADS-B-derived jamming *events* (jamind only), no clean-vs-attack pairing.

These are catalogued for completeness; none was forced into the probe, to keep every reported real number honest.
