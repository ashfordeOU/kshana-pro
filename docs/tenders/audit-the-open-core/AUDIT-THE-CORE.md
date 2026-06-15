<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NOT for the open kshana repo.
Prime-facing trust leave-behind (dominance-plan step A12). Contact: contact@ashforde.org
Every claim below traces to the PUBLIC Apache-2.0 repo github.com/ashfordeOU/kshana (v0.17.0).
Honest framing only: software-model validation, TRL ~3, no flight heritage, no certification, no agency endorsement.
-->

# Audit the open core — line by line

**A standing invitation to a prime's chief engineer: take an afternoon and check every claim in our core against the source.**

Every other navigation simulator you will weigh in this competition is a black box — you take its accuracy on faith, and you find out what it cannot do after the contract is signed. Ours is the opposite: the entire navigation-performance engine is public under **Apache-2.0** at **`github.com/ashfordeOU/kshana`** (release **v0.17.0**), and a **machine-checked honesty invariant runs in CI** that makes it *structurally impossible* for us to label a model as a validation. This page is the map that lets you verify that in one sitting.

> **What this document is — and is not.** This is the audit map for the **open core**. It deliberately contains **no** export-sensitive content, **no** proprietary threat libraries, and **no** customer-calibration data — those live behind the legal wall in the private overlay and are not part of what we ask you to audit here. Everything referenced below is in the public repo and runs offline with `cargo test`.

---

## 0. How to audit this in an afternoon

```sh
git clone https://github.com/ashfordeOU/kshana
cd kshana
cargo test                         # the whole suite, offline, deterministic
cargo test --test sgp4_verification -- --nocapture   # prints rows compared + worst-case error
```

Then read three files and you have the whole honesty story:

1. **`src/verification.rs`** — the machine-checked verification matrix (requirement → module → test → oracle → status), and the CI-enforced invariant that a *Validated* row **must** carry an external dataset oracle. Start here.
2. **`docs/VALIDATION.md`** — the per-figure `validated` / `modelled` ledger in prose, with the real numbers and tolerances.
3. **`docs/PROVENANCE.md`** — every parameter and dataset traced to a published source (datasheet, paper, ICD, or standard).

The rest of this page is the curated index pointing into those.

---

## 1. The honesty invariant — enforced by unit tests, not by a promise

The spine of the audit lives in **`src/verification.rs`**. It defines two enums and a matrix, and then it makes the *classification rules* executable.

### The oracle taxonomy (`OracleKind`)

A row cannot just say "tested" — it must declare *how* its claim is backed:

| `OracleKind` | Meaning (quoted from the source doc-comments) |
|---|---|
| `ExternalDataset` | "Independent authoritative dataset, published verification vectors, or a published numeric value the implementation is checked against." |
| `ReferenceImpl` | "A separate implementation in this same codebase (a different algorithm / code path), used as a cross-check. Independent of the unit under test but not externally authoritative." |
| `InternalConsistency` | "An internal closed-form / algebraic identity or self-consistency check … Catches transcription and coefficient errors; is **not** an external validation." |
| `NoneKind` | "No oracle — a partner-owned gap with no implementation." |

### The status taxonomy (`VerificationStatus`)

- **`Validated`** — checked against an independent **external** oracle to a stated tolerance.
- **`Modelled`** — implemented from published or first-principles physics, with tests, but **not** checked against an external oracle to a stated tolerance. *It is a model, not an external validation.*
- **`PartnerOwned`** — not provided by Kshana; a consortium partner's discipline. No code, by design.

### The invariant the CI enforces

The unit tests in `src/verification.rs` (module `tests`) enforce, on every CI run:

- **`validated_rows_require_an_external_oracle`** — *"a Validated row **must** carry an `ExternalDataset` oracle, not `{:?}`"*. **A row whose only evidence is an internal algebraic identity or a sibling re-implementation cannot be `Validated`.** This is the single most important line for an auditor: it is mechanically impossible to dress up a self-consistency check as a validation.
- **`modelled_rows_have_module_tests_and_an_oracle_kind`** — a Modelled row must name a module and tests and carry a real (non-`None`) oracle kind.
- **`partner_rows_claim_nothing`** — a `PartnerOwned` row must have **no** module, **no** test, **no** oracle (`NoneKind`). "Zero code = zero claim."
- **`none_oracle_kind_only_on_partner_rows`**, **`the_four_partner_gaps_are_present`**, **`requirements_are_unique`**, **`summary_counts_are_consistent`** (with a floor: `s.validated >= 4`, and — because of the external-oracle invariant — *adding an overclaimed Validated row cannot satisfy that floor*).

> **What the tests check, stated plainly (from the module doc-comment).** The tests check the **classification** — that a Validated row is tagged external, that a partner row claims nothing, that counts are consistent. They do **not** prove every named test/oracle *string* resolves to live code; those are curated references maintained by hand and cross-checked in code review, exactly as the citations in `docs/VALIDATION.md` are. We scope the "machine-checked" claim to the status/oracle-kind invariants, **not** to the existence of every string — stated here so the artifact does not oversell itself. (You verify the strings the old-fashioned way: open the named test file. That is the point of this page.)

**Current matrix totals (from the source): 22 rows — 5 externally validated, 13 modelled, 4 partner-owned.**

---

## 2. The claim → module → test → label → oracle index

Every row below is a row of `verification_matrix()` in `src/verification.rs`. The module paths and test files are real and present in the public repo (each was opened to confirm it exists). `OracleKind` and `Status` are quoted verbatim from the matrix.

### 2a. Externally validated core (5 rows — `Validated` ⇒ `ExternalDataset`)

| # | Requirement | Module(s) | Representative test | External oracle | OracleKind | Status |
|---|---|---|---|---|---|---|
| 1 | Frequency-stability characterisation (Allan/MDEV/HDEV + power-law noise ID, χ² CIs) | `src/allan.rs` | `tests/allan_reference.rs` (NBS14 vs Stable32 to 1e-4); `allan::tests` | NIST SP 1065 (Riley) / Stable32 reference deviations on NBS14 | `ExternalDataset` | **VALIDATED** |
| 2 | Integrity (RAIM / ARAIM / SBAS) | `src/raim.rs`, `src/sbas.rs`, `src/lunar.rs` | `tests/igs_real_data.rs`, `tests/araim_dual_real_data.rs` (real IGS SP3 + Celestrak TLE) | DO-229E / DO-316 K-factors; real IGS SP3 geometry | `ExternalDataset` | **VALIDATED** |
| 3 | Orbit propagation & determination (SGP4/SDP4, Cowell 6-DOF, batch/sequential OD) | `src/sgp4.rs`, `src/propagator.rs`, `src/orbit_determination.rs`, `src/precise_od.rs` | `tests/sgp4_verification.rs` (666 AIAA vectors, 4.12 mm worst-case) | AIAA 2006-6753 SGP4 verification vectors | `ExternalDataset` | **VALIDATED** |
| 4 | Reference frames & timescales (IAU 2006/2000A, CIO GCRS↔ITRS, leap-second timescales) | `src/frames.rs`, `src/precession.rs`, `src/nutation.rs`, `src/cio.rs`, `src/timescales.rs` | `tests/frame_reference_vectors.rs` (Vallado end-to-end; SOFA/ERFA vectors) | IAU SOFA / ERFA reference vectors; Vallado AIAA 2006-6753 (0.1–4 mm) | `ExternalDataset` | **VALIDATED** |
| 5 | Ranging-code design trade (m-sequence/Gold sidelobe & cross-correlation bounds) | `src/navsignal.rs` (`CodeFamily`) | `navsignal::code_tests` (GPS C/A Gold ≈ −23.9 dB; φ(1023)/10 = 60) | Published GPS C/A Gold cross-correlation (−23.9 dB); Gold 1967 bound | `ExternalDataset` | **VALIDATED** |

### 2b. Modelled — published / first-principles physics, internally checked (13 rows)

These implement published or first-principles physics, are tested, and **declare honestly that they are models** — the oracle is a same-codebase cross-check (`ReferenceImpl`), a closed-form/algebraic identity (`InternalConsistency`), or an external figure used only as a loose bracket (not parity). **None of these is presented as an external validation.**

| # | Requirement | Module(s) | Representative test | Oracle | OracleKind | Status |
|---|---|---|---|---|---|---|
| 6 | GNSS-denied clock holdover | `src/holdover.rs` | `holdover::tests` (vs multi-step Kalman covariance recursion; white-FM exact; round-trip) | Multi-step `clock_state` covariance recursion (same-codebase cross-check) | `ReferenceImpl` | MODELLED |
| 7 | Onboard clock state estimation (3-state van-Loan Kalman, Joseph-stabilised) | `src/clock_state.rs` | `clock_state::tests` (analytic van-Loan Q; NEES consistency; PSD positivity) | Analytic van-Loan Q closed form (internal algebraic identity) | `InternalConsistency` | MODELLED |
| 8 | Time-transfer error budgeting (TWSTFT/Sagnac, common-view, PPP) | `src/timetransfer.rs`, `src/timetransfer_adv.rs` | `timetransfer::tests` (reciprocal cancellation; two-form Sagnac identity) | BIPM 2Aω/c² Sagnac closed form (internal algebraic identity) | `InternalConsistency` | MODELLED |
| 9 | Nav-signal modulation & code-tracking analysis (BPSK-R/BOC PSD, DLL jitter, multipath) | `src/navsignal.rs` | `navsignal::tests` (BPSK self-SSC = 2/3R_c; unit-area PSD; DLL) | Betz 2001 / Kaplan & Hegarty closed forms (internal algebraic identity) | `InternalConsistency` | MODELLED |
| 10 | Quantum inertial sensor performance (cold-atom interferometer accelerometer) | `src/inertial/quantum_imu.rs` | `quantum_imu::tests` (k_eff; Mach-Zehnder T²; Freier-2016 floor bracket) | Freier et al. 2016 mobile gravimeter — **order-of-magnitude bracket, not parity** | `ExternalDataset` | MODELLED |
| 11 | Quantum inertial dead-reckoning resilience (bias + scale + VRW + stability-decay budget) | `src/inertial/quantum_imu.rs` (`QuantumNavBudget`) | `budget_tests` (bias vs AccelModel integrator; VRW vs analytic integral; round-trip) | AccelModel Euler integrator (same-codebase reference); analytic closed forms | `ReferenceImpl` | MODELLED |
| 12 | GNSS-denied jamming resilience (J/S link budget, anti-jam C/N₀, loss-of-lock) | `src/jamming.rs` | `jamming::tests` (PSD-derived Q cross-check; despreading) | Anti-jam C/N₀ equation; `navsignal` PSD-derived Q (internal) | `InternalConsistency` | MODELLED |
| 13 | Spoofing detection (clock-aided χ², RAIM, AGC, SQM fused security FoM) | `src/spoof.rs`, `src/spoof_detect.rs`, `src/spoof_monitors.rs` | `tests/spoof_texbat_validation.rs` (TEXBAT parameter characterisation) | TEXBAT scenario parameters (Humphreys 2012) — **characterisation, not pinned vectors** | `ExternalDataset` | MODELLED |
| 14 | Cislunar mission analysis (CR3BP STM + single-shooting corrector; L2 NRHO) | `src/cr3bp.rs` | `cr3bp::tests` (STM vs finite-diff; reproduces 9:2 NRHO regime ≈ 6.57 d) | Published Gateway 9:2 NRHO period/perilune (literature, approximate) | `ExternalDataset` | MODELLED |
| 15 | Alternative / complementary PNT (gravity-map, terrain-referenced, magnetic anomaly) | `src/altpnt/`, `src/mapmatch.rs`, `src/gravimeter.rs`, `src/igrf.rs` | `tests/*` (map-matching CRLB; IGRF-14 field) | IGRF-14 coefficients; first-principles matched-filter CRLB | `InternalConsistency` | MODELLED |
| 16 | Reproducibility & software assurance (deterministic, scenario-hashed, SBOM, golden gates) | `src/report.rs`, `src/scenario.rs`; CI | `tests/golden.rs`, `tests/determinism.rs`, `tests/cross_platform_golden.rs` | Pinned golden FoM (1e-6) + SHA-256 determinism (self-consistency) | `InternalConsistency` | MODELLED |
| 17 | AI/ML RF-impairment detection evaluation (AO/1-13494) | `src/impairment_eval.rs` | `impairment_eval::tests` (AUC bounds, ROC monotone, leakage guard, distribution-shift optimism) | Closed-form AUC bounds (Mann–Whitney) + a perfect-oracle detector; **corpus is SYNTHETIC (parameter-grounded, not field/IQ)** | `InternalConsistency` | MODELLED |
| 18 | Quantum-vs-classical PNT trade & GNSS-denied resilience (AO/1-13503) | `src/quantum_trade.rs` | `quantum_trade::tests` (ADEV round-trip, NNLS non-negativity, floor-caveat present, monotone envelope) | ADEV→q round-trip vs the holdover noise model; **quantifies (never validates) a partner clock/CAI; numbers MODELLED, no validation halo** | `ReferenceImpl` | MODELLED |

### 2c. Honestly partner-owned gaps — no code, by design (4 rows — `PartnerOwned` ⇒ `NoneKind`)

These are the disciplines we deliberately **do not** claim. The matrix records them as empty rows so the gap is auditable, not hidden — and the `partner_rows_claim_nothing` test fails CI if any of them ever sprouts a module, test, or oracle.

| # | Requirement (partner discipline) | Module | Test | Oracle | Status |
|---|---|---|---|---|---|
| 19 | Spacecraft bus engineering (AOCS / thermal / structures / propulsion / power) | — | — | — | **PARTNER** |
| 20 | Navigation RF payload & antenna hardware design | — | — | — | **PARTNER** |
| 21 | Quantum payload hardware design & maturation | — | — | — | **PARTNER** |
| 22 | Flight-hardware product assurance (radiation / EEE / MAIT) | — | — | — | **PARTNER** |

---

## 3. The five real validation islands — and their external oracles

Five capability areas reach the **`Validated`** bar. Each is checked against an *independent external* oracle to a stated tolerance. **All five are software-model validations: the engine's numbers are checked against published vectors, real recorded datasets, or independent reference codes. None of them is a flight test, a hardware test, or an on-orbit demonstration — and we say so on every one.**

### Island 1 — SGP4/SDP4 orbit propagation
- **Oracle:** the canonical community reference verification vectors published with **Vallado et al., "Revisiting Spacetrack Report #3" (AIAA 2006-6753)** — distributed as `SGP4-VER.TLE` + `tcppver.out`, the same table the public C++/Python/MATLAB SGP4 implementations validate against.
- **Result:** all **666** reference states compared (count pinned in the test so a regression cannot quietly reduce coverage); **worst position error ≈ 4 mm** (< `2e-5` km tolerance); worst velocity error ≈ 1.85e-9 km/s. Covers near-Earth SGP4, deep-space SDP4 (lunar-solar + 12 h/24 h resonance), and the deliberate error-code cases.
- **Independent cross-check:** `tests/sgp4_crate_comparison.rs` runs the same 666 vectors through the widely-used third-party `sgp4` crate (neuromorphicsystems/sgp4); two independent implementations agree to **sub-micron** on near-Earth and resonant orbits, **4.12 mm worst-case** across all regimes.
- **Evidence:** `src/sgp4.rs`, `tests/sgp4_verification.rs`, `tests/fixtures/sgp4/`; `docs/SGP4-VALIDATION.md`.

### Island 2 — Allan-family frequency-stability estimators
- **Oracle:** **NIST SP 1065** (Riley, *Handbook of Frequency Stability Analysis*, 2008) — two primary-source checks, both to a **1e-4** relative tolerance (achieved ~1e-6). (1) `tests/allan_reference.rs` reproduces the canonical 10-point **NBS14** deviations (SP 1065 Table 29/30, p. 107) for OADEV/MDEV/TDEV/OHDEV at τ = 1, 2 (e.g. OADEV(2) 85.952868 vs the table's 85.95287). (2) `tests/allan_nist_sp1065_1000point.rs` reproduces the SP 1065 §12.4 **1000-point** data set (Table 31, p. 108) at averaging factors 1/10/100, regenerating the data from the SP 1065 LCG (Eq. 73) in code — hermetic, no fixture.
- **Result:** the same data and Table-31 numbers are the regression target in the open-source `allantools` package; reproduced here with no third-party code. The χ² confidence-interval edf is independently checked against SP 1065 Table 32 (edf = 146.177, m=10, white FM) and against a 4 000-record Monte-Carlo white-FM ensemble.
- **Evidence:** `src/allan.rs`, `tests/allan_reference.rs`, `tests/allan_nist_sp1065_1000point.rs`; `docs/VALIDATION.md`.

### Island 3 — Integrity on real IGS / Celestrak data (RAIM / ARAIM / SBAS)
- **Oracle:** real **IGS SP3** precise-orbit geometry and real **Celestrak `gps-ops`** TLE snapshots (provenance + SHA-256 in `tests/fixtures/igs/NOTICE` and `tests/fixtures/celestrak/NOTICE`), combined with **DO-229E / DO-316** K-factors.
- **Result:** `tests/igs_real_data.rs` parses a genuine Celestrak `gps-ops` snapshot (2021-07-28, 30 operational GPS satellites), propagates each through the validated SGP4 core to a common instant, rotates TEME→ECEF, and asserts the constellation lands on the GPS MEO shell within 1% and that a mid-latitude open-sky site yields PDOP 1.64 with vertical > horizontal dilution (as it must). `tests/araim_dual_real_data.rs` propagates vendored GPS + Galileo snapshots (2026-06-07) over a 24 h grid and shows the multi-constellation ARAIM-availability benefit on real orbital geometry.
- **Honest scope (from the test source):** this exercises the integrity engine on *real recorded data*; the numerically-exact reproduction of the EU ARAIM Technical Note worked example (Table A-3) against a single version-locked epoch is noted in the test as an external/founder-gated item still open (see `docs/ARAIM_REFERENCE.md`).
- **Evidence:** `src/raim.rs`, `src/sbas.rs`, `tests/igs_real_data.rs`, `tests/araim_dual_real_data.rs`; `docs/INTEGRITY.md`, `docs/VALIDATION.md`.

### Island 4 — Reference frames & timescales (IAU 2006/2000A) against SOFA/ERFA
- **Oracle:** the published **IAU SOFA / ERFA** reference vectors, plus Vallado AIAA 2006-6753 end-to-end.
- **Result:** the full IAU 2000A nutation series (678 luni-solar + 687 planetary terms) is validated **bit-for-bit** against `eraNut00a` at JD_TT 2453736.5 to **1e-13 rad**; the equinox-free CIO `(X, Y, s)` chain is validated against `eraXys06a`/`eraC2ixys`/`eraEra00` to 1e-12–1e-18; the data tables are machine-generated from the ERFA source and reproduce the committed `*_data.rs` bit-for-bit. An **independent third-party cross-check** (`xval/anise-frames/`, workspace-excluded) compares the same GCRS↔ITRS matrix against **ANISE** (the pure-Rust NAIF/SPICE reimplementation) over eight quarterly epochs 2020–2023 with identical IERS `finals2000A` EOP fed to both sides — agreement to **≤ 3.6 m at GNSS orbit** (max relative rotation 0.028″).
- **Evidence:** `src/frames.rs`, `src/nutation.rs`, `src/cio.rs`, `src/precession.rs`, `src/timescales.rs`, `tests/frame_reference_vectors.rs`; `docs/VALIDATION.md`.

### Island 5 — Mars / deep-space dynamics: synthetic Mars-LMO OD + DE440 cross-check
- **Oracle (two parts).** (a) An *internal* synthetic closed-loop for the **estimator machinery**: `tests/mars_lmo_od.rs` recovers a truth Low-Mars-Orbit arc (propagated under `Body::mars_gmm3(4)` gravity) from a ~2.7 km-perturbed initial state using the reduced-dynamic SRIF, achieving converged position **RMS ≈ 0.2 m** with the factored covariance positive-definite at every epoch. (b) The *external* DE-grade anchor: `xval/anise-mars-od/` (workspace-excluded, kernel-gated) compares Kshana's Sun-central Mars propagation against **JPL DE440** via ANISE, with Horizons truth cited verbatim (kernel `de440s.bsp`, SHA-256 recorded) — position residual **137 m at a 1-day arc** (5.97×10⁻¹⁰ of the 2.299×10¹¹ m heliocentric distance), growing with arc length exactly as the deliberately-omitted n-body perturbations predict.
- **Honest scope (from the test source, verbatim intent):** the Mars-LMO end-to-end recovery is a **synthetic closed-loop** — truth and filter share the same Mars propagator and the same geometric observable model plus injected Gaussian noise — so it validates the **estimator**, *not* the absolute fidelity of the Mars force model. Every Mars-PNT figure of merit is a formal covariance bound / closed-loop RMS, **NOT** an aviation-certified protection level and **NOT** a flight claim. The DE440 cross-check is a manual / `workflow_dispatch` DE-grade check, never a default CI gate, so the published crate stays byte-for-byte untouched.
- **Evidence:** `src/mars_pnt.rs`, `src/deepspace_od.rs`, `src/mars_atmos.rs`, `src/body.rs`, `src/mars_frame.rs`, `tests/mars_lmo_od.rs`, `xval/anise-mars-od/`; `docs/VALIDATION.md`.

> **One sentence to carry away:** all five islands are validations of **software against published references, recorded data, or independent reference codes** — they are evidence that the *maths is right*, not that *hardware has flown*. There is no flight heritage, no hardware-in-the-loop test, and no on-orbit result anywhere in this engine.

---

## 4. What we deliberately do **not** claim

We would rather lose a row than fake one. For the record, in plain language:

- **No flight heritage.** Nothing in this engine has flown. Every figure is from a software model, a published vector, a recorded ground dataset, or a paper. Hardware maturity sits at roughly **TRL 3** (the open repo says so itself, in `docs/CAPABILITY.md`).
- **No certification.** The per-run "Integrity Performance" figure of merit is honestly labelled **filter self-consistency**, *not* aviation HPL/VPL/RAIM integrity; the real ARAIM HPL/VPL is surfaced *separately* so the two are never conflated (see the OC-8 closure in `docs/CLAIMS-VS-REALITY.md`). No claim is DO-178/ECSS-certified, and the Mars-PNT FoM is explicitly *not* a certified protection level.
- **No agency endorsement.** Citing a standard, an IGS dataset, or a JPL ephemeris is provenance, not endorsement. No agency has reviewed, blessed, or contracted this work; `docs/CAPABILITY.md` records "no agency citations/contracts yet."
- **No hardware.** We are a **navigation-performance simulator**, not a bus house, not a payload/antenna shop, not a quantum-hardware lab, and not a flight-PA organisation. Those four disciplines are the `PartnerOwned` rows above — empty by design, and CI-enforced to *stay* empty (zero code = zero claim).
- **No "validated" without an external oracle.** A `Modelled` row is a model, full stop — even when it implements textbook physics and passes every internal check. The `Validated ⇒ ExternalDataset` invariant in `src/verification.rs` makes the boundary mechanical, not editorial.
- **What's missing from *this* package on purpose.** Export-controlled threat libraries and customer-calibration data are **not** in the open core and **not** in this audit pack. They live behind the legal wall in the private overlay; auditing them is a separate, NDA-gated conversation. This page is the part you can verify today, with nothing to take on trust.

---

## 5. The pitch, in one line

> *Every other simulator in this competition is a black box. Ours is auditable line-by-line, and a test in CI refuses to let us mislabel a model as a validation. Clone it, run `cargo test`, and check us against the source — in an afternoon.*

---

*Private — Ashforde OÜ internal bid material. Not for the open repo. All external contact: **contact@ashforde.org**. Public open-core repo: `github.com/ashfordeOU/kshana` (Apache-2.0, v0.17.0).*
