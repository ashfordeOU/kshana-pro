<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
A tight, paste-ready ECSS-E-ST-10-02 verification-evidence fragment for a prime's
Verification Control Document (VCD). Shared by BOTH AO/1-13494 and AO/1-13503 (the engine
evidence is the same; the proposals are bid SEPARATELY). Companion to the per-tender
SE-verification-plan.md. Contact: contact@ashforde.org
-->

# ECSS-E-ST-10-02 verification-evidence fragment — Kshana PNT-performance & AI-evaluation contribution

**Owner:** Ashforde OÜ (Chakshu Baweja) — PNT-performance / resilience + AI-algorithm-evaluation SME.
**Applies to:** AO/1-13494 (NAV-DT performance / AI-eval WP) **and** AO/1-13503 (quantum-vs-classical trade WP) — the engine evidence is shared; the two bids stay separate.
**Subject element:** **Kshana** — dual-licensed (GNU AGPL-3.0-only + commercial licence), open and auditable line-by-line; evidence below generated at **v0.18.0** (re-run against the current validated release line before submission) — *one component of a partner-led NAV-DT, not the whole twin*.
**Standards:** ECSS-E-ST-10-02 (Verification), applied in style; ECSS-E-ST-10C / ECSS-M-ST-10C lifecycle map in the per-tender [`SE-verification-plan.md`](../13494-ai-payload-pnt/SE-verification-plan.md).
**Contact:** contact@ashforde.org

> **Purpose.** A drop-in VCD fragment a prime's SE lead can paste into the
> navigation-performance + AI-evaluation section of the project Verification Control
> Document. It is the *condensed* evidence table; the full element-level plan (methods,
> lifecycle/review map, SRD skeleton, residuals) is the per-tender `SE-verification-plan.md`.
> **All requirement IDs are placeholders** (`NAV-PERF-*`) to be re-keyed to the real SRD
> (SoW FPD-10030 / 13503 equivalent), **not yet obtained**.

---

## 1. Method allocation (ECSS-E-ST-10-02)

Ashforde's contribution concentrates in **Analysis (A)** and **Test (T)**, with supporting
**Review-of-Design (R)** and **Inspection (I)**. Flight-representative test / qualification
(environmental, radiation, RF/antenna, MAIT) is **partner-owned** and recorded as such — not
silently omitted.

## 2. The machine-checked spine

The evidence below is rendered from `kshana/src/verification.rs`, a matrix mapping
**requirement → module → test → oracle → OracleKind → status**. A **unit-tested honesty
invariant** permits the **VALIDATED** label *only* where an `ExternalDataset` (independent
authoritative dataset / published vectors / independent library) backs the row; anything
self-consistent is **MODELLED**; partner rows claim nothing. The boundary therefore cannot
silently drift over the study. (The invariant is a *classification* check, not a proof that
every cited test string resolves to live code — those are curated, code-reviewed references.)

**Validated islands: 8** (external-oracle backed), all green on CI across Linux/macOS/Windows.

## 3. Verification Control Document fragment

| Req-ID (placeholder) | Requirement / capability | V-method | Test evidence | External oracle | OracleKind | Status |
|---|---|---|---|---|---|---|
| NAV-PERF-010 | Orbit propagation (SGP4/SDP4) — geometry spine | A + I | `tests/sgp4_verification.rs`, `sgp4_crate_comparison.rs` (666 vectors, 4.12 mm) | AIAA 2006-6753 verification vectors | ExternalDataset | **VALIDATED** |
| NAV-PERF-040 | Reference frames & timescales (IAU 2006/2000A, CIO chain) | A + I | `tests/frame_reference_vectors.rs` (0.1–4 mm, bit-for-bit) | IAU SOFA / ERFA; Vallado | ExternalDataset | **VALIDATED** |
| NAV-PERF-020 | Integrity protection levels (RAIM / ARAIM / SBAS) | A + I | `tests/igs_real_data.rs`, `araim_dual_real_data.rs` (real geometry) | DO-229E / DO-316 K-factors; real IGS SP3 + Celestrak TLE | ExternalDataset | **VALIDATED** |
| NAV-PERF-050 | Frequency-stability characterisation (ADEV/MDEV/TDEV/HDEV) | A | `tests/allan_reference.rs` (to 1e-4) | NIST SP 1065 (Riley) / Stable32 on NBS14 | ExternalDataset | **VALIDATED** |
| NAV-PERF-060 | Ranging-code design trade (m-sequence / Gold bounds) | A | `navsignal::code_tests` (GPS C/A ≈ −23.9 dB) | Published GPS C/A Gold cross-correlation; Gold 1967 bound | ExternalDataset | **VALIDATED** |
| NAV-PERF-070 | GNSS geometry / dilution of precision (GDOP/PDOP/HDOP/VDOP/TDOP) | A | `tests/dop_reference.rs` (8 geometries, to 1e-6) | **gnss_lib_py 1.0.4 (Stanford NAV Lab)** — independent library | ExternalDataset | **VALIDATED** |
| NAV-PERF-210a | AI-detector evaluation **metrics** (AUC/ROC/confusion/P_d-P_md/precision/F1) | A + I | `tests/eval_metrics_reference.rs` (5 datasets × 24 thresholds; exact + < 1e-9) | **scikit-learn 1.9.0** (Pedregosa et al., JMLR 2011) — independent library | ExternalDataset | **VALIDATED** |
| NAV-PERF-410 | Quantum-trade numerical **kernels** (ADEV NNLS fit · χ² bands · van-Loan clock Q) | A | `tests/scipy_reference.rs` (NNLS+Q exact; χ² < 5e-4 at dof ≥ 48) | **scipy 1.17.1** — `optimize.nnls` / `stats.chi2.ppf` / `linalg.expm` | ExternalDataset | **VALIDATED** |
| NAV-PERF-300 | Verification cross-reference (honesty invariant) | I | `validated_rows_require_an_external_oracle`, `partner_rows_claim_nothing` | Test-enforced classification invariant | InternalConsistency | **VALIDATED** (invariant) |
| NAV-PERF-100 | Constellation sizing for coverage KPI | A | `streets_of_coverage_*`, `optimize_walker_design` | Rider/Beste closed forms; brute-force winner cross-check | ReferenceImpl | **MODELLED** |
| NAV-PERF-110 | Achievable PNT figure-of-merit (accuracy / availability) | A | `fom::tests` | Published FoM definitions; self-consistency | InternalConsistency | **MODELLED** |
| NAV-PERF-120 | GNSS-denied jamming resilience (% availability) | A | `jamming::tests` (`availability_under_jamming`) | Kaplan & Hegarty §9.4 anti-jam C/N₀; PSD-derived Q | InternalConsistency | **MODELLED** |
| NAV-PERF-200 | Labelled RF-impairment corpus | A + T | `impairment_eval::tests` (`generate_corpus`, `stratified_split`) | TEXBAT scenario **parameters** (characterisation, **not** raw IQ) | ExternalDataset | **MODELLED** |
| NAV-PERF-210b | AI-detector **verdict on a real fielded device** | A + T | — | No field-detection oracle (synthetic-label AUC only) | InternalConsistency | **MODELLED** |
| NAV-PERF-400 | Quantum-vs-classical **device-benefit** numbers | A | `quantum_trade::tests`, scenario-hashed | Partner device data (kernels validated; device numbers are partner input) | ReferenceImpl | **MODELLED** |
| NAV-PERF-900 | Spacecraft-bus engineering (AOCS/thermal/structures/propulsion/power) | (partner) | — | — | NoneKind | **PARTNER** |
| NAV-PERF-910 | Reconfigurable payload / SDR / FPGA + phased-array antenna HW | (partner) | — | — | NoneKind | **PARTNER** |
| NAV-PERF-920 | On-board / edge AI inference engine + secure in-orbit reconfiguration | (partner) | — | — | NoneKind | **PARTNER** |
| NAV-PERF-930 | Full NAV-DT + payload / RF-environment (DestinE) twins + Core Ground Infra | (partner) | — | — | NoneKind | **PARTNER** |
| NAV-PERF-940 | Flight-hardware product assurance (radiation / EEE / MAIT) | (partner) | — | — | NoneKind | **PARTNER** |

> **Re-classification note (since the earlier draft plan):** the AI-evaluation **metrics**
> (NAV-PERF-210a) are now **VALIDATED** against scikit-learn — previously carried as modelled.
> The **corpus** (200), the **real-device verdict** (210b) and the **device-benefit** numbers
> (400) remain honestly **MODELLED**: the testbed's *ruler* is externally validated; the
> *labels it grades* and the *device numbers it quantifies* are not.

## 4. Further external-oracle analyses (catalogued in `docs/VALIDATION.md`)

Beyond the 8 machine-checked islands, additional analyses are anchored to external references
(recorded in the validation ledger, available as VCD inputs on request):

- Single-point positioning vs a surveyed IGS station (**ABMF, 5.7 m 3-D / 1.1 m H**, real GPS observations) — `tests/pvt_abmf.rs`.
- Force-model fit vs agency precise ephemerides — **ESA/ESOC Galileo 0.61 m**, **Swarm-A 0.10 m** 3-D RMS (24 h).
- Sun-central Mars dynamics vs **JPL DE440** (137 m @ 1-day arc).
- EGM2008 geopotential (d/o 70): acceleration = ∇V to **< 1e-6**.
- GCRS→ITRS vs an independent SPICE engine (**ANISE**): ≤ 0.86 m ground / ≤ 3.6 m GNSS orbit.

## 5. Software-assurance inspection evidence

- **950+** library unit tests; `fmt` + `clippy` clean; CycloneDX **SBOM**; pinned toolchain + committed `Cargo.lock`; build provenance.
- **Bit-identical** input + shape goldens on the 3-OS CI matrix; every result reproducible from `scenario + seed + engine version`.
- ~96 % line coverage on `src/`, CI-gated ≥ 85 %.

## 6. Honest residuals (condensed — full list in `SE-verification-plan.md` §7)

1. **Placeholder IDs.** All `NAV-PERF-*` IDs and any targets are placeholders; the authoritative requirement set is the SRD from the real SoW, **not yet obtained** — re-key and re-check before submission.
2. **The prime owns the VCD.** Ashforde supplies these *inputs*; it does not author or sign the system VCD/RTM and holds no system review (MDR/PRR/ISRR/FR).
3. **No ECSS formal-deliverable track record yet** — the discipline is applied in style, not as a previously-contracted ESA deliverable.
4. **TRL & heritage.** Kshana is a simulation/evaluation tool at **TRL ≈ 3** — no flight heritage, no ESA endorsement, no certification. "Validated" is reserved strictly for external-oracle checks.
5. **Open-source authorisation.** Kshana is Background IPR, dual-licensed (GNU AGPL-3.0-only + commercial). If brought under the open AGPL-3.0 track, OSS prior-authorisation (GCC Part II Option A) applies and AGPL copyleft attaches; Kshana is therefore brought under the **commercial licence** as proprietary Background IPR, which avoids both. *(Counsel to confirm the treatment.)* Flag for the prime; BIPR table mandatory.
6. **Founder/consortium-gated:** the seat, the prime, the real SRD, Key-Personnel CVs, and costing are not pre-drafted here.

---

*Paste-ready ECSS-E-ST-10-02 verification-evidence fragment, owned by Ashforde OÜ as the
NAV-DT-performance + AI-evaluation SME — it feeds, and does not replace, the prime's
system-level verification programme. All quantitative claims trace to Kshana v0.18.0
`docs/CAPABILITY.md` / `docs/VALIDATION.md` under the validated-or-modelled discipline.
Contact: **contact@ashforde.org**.*
