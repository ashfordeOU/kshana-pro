<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
DRAFT software product-assurance evidence pack for AO/1-13503 (FutureNAV Quantum-Enabled
PNT Demonstrator). For the prime's product-assurance / quality annex.
Author / owner: Ashforde OÜ — Chakshu Baweja. Contact: contact@ashforde.org
-->

# Software Product-Assurance evidence pack — Kshana (ECSS-Q-ST-80)

**DRAFT / TEMPLATE — for consortium PA integration.**
**Tender:** ESA AO/1-13503 — *Quantum-Enabled PNT Demonstrator* (FutureNAV), Phases 0 / A / B1, study toward IOD 2030–31.
**Standard mapped:** ECSS-Q-ST-80C Rev.1 — *Space product assurance — Software product assurance*.
**Subject software:** Kshana — open PNT-performance-and-resilience simulator, engine v0.17.0 (Apache-2.0).
**Author / owner:** Ashforde OÜ — Chakshu Baweja.
**Contact:** contact@ashforde.org
**Status:** DRAFT. The criticality-category assignment, the formal PA plan sign-off, and the configuration-management plan formalisation are **founder-gated** and await the real Statement of Work (FPD-10029) and the consortium teaming arrangement. Numeric and process claims below reflect the engine as released; section 7 lists what is **not yet** in place.

---

## 1. Purpose and scope

This pack is the evidence base for the **one** product-assurance slice Ashforde can speak to honestly: **software product assurance** for the navigation-performance and quantum-vs-classical trade-study work package. It maps Kshana's existing software-quality infrastructure to the relevant ECSS-Q-ST-80 process areas so a consortium PA lead can fold a navigation-performance contribution into a project-level Software Product Assurance Plan (SPAP) and an ECSS-E-ST-40 / ECSS-E-ST-10-02 verification control document.

Ashforde's honest role in the consortium is the **navigation-performance and quantum-vs-classical trade-study SME** within a **prime-led** consortium. Ashforde is **not** the prime and **not** a hardware supplier. Kshana is a **simulator**, not flight software and not a receiver.

### 1.1 SCOPE — what this pack covers

- Software product assurance for **simulation / analysis software** used to produce navigation-performance results, trade-study figures, and verification evidence.
- The ECSS-Q-ST-80 process areas where simulation/analysis software is assessable: software dependability & safety (as it applies to a non-flight analysis tool), software V&V, software configuration & documentation management, software problem / non-conformance handling, reuse of existing software, and quality records.
- Applicability to software of criticality **categories B / C / D** in the ECSS-Q-ST-80 sense. The trade-study tool is expected to fall at the **lower-criticality end (C/D)**: it produces analysis evidence for design decisions, not flight-control output. The final category assignment is the project's to make against the FPD-10029 SoW and the consortium criticality analysis (see §7).

### 1.2 OUT OF SCOPE — explicitly a consortium partner's responsibility

This pack covers **software PA only**. The following are **NOT** Kshana's and **NOT** Ashforde's; they are a consortium partner's role and are listed so the boundary is unambiguous, not implied by silence:

- **Flight-hardware product assurance** under ECSS-Q-ST-30 / -60 / -70: radiation (TID / TNID / SEE), EEE-component and COTS selection and de-rating, parts/materials/processes, MAIT (manufacturing, assembly, integration & test), and hardware qualification.
- **Spacecraft-bus** engineering.
- **RF-payload / antenna HARDWARE** design (radiation patterns, front-end, EIRP/G-T sizing).
- **Quantum-payload HARDWARE** design and maturation (cold-atom physics package, laser system, vacuum/magnetic-shield engineering).
- **Embedded / flight software** for the demonstrator: Kshana is a ground-side performance simulator. If any of the trade-study models is later targeted for re-implementation as onboard software, that is a separate development under the project's flight-software PA regime — not this pack.

Where Kshana models a quantum or RF effect (e.g. the cold-atom-interferometer noise budget, the jamming link budget), it is a **navigation-performance model**, not a hardware design or a hardware-qualification artifact.

---

## 2. Subject-software identification and integrity baseline

| Attribute | Value | Evidence |
|---|---|---|
| Product | Kshana — PNT-performance-and-resilience simulator (engine) | `README.md`, `docs/CAPABILITY.md` (open repo) |
| Version baseline | v0.17.0 | `Cargo.toml` `version`; CI version-sync gate `scripts/check-version-sync.sh` |
| Licence | Apache-2.0 (permissive open source, incl. patent grant) | `LICENSE`, `NOTICE` |
| Language / toolchain | Rust, pinned toolchain channel | `rust-toolchain.toml`; drift gate `scripts/check-toolchain.sh` |
| Dependency baseline | Committed lockfile; permissive-only licence allow-list (no GPL/copyleft), CI-enforced | `Cargo.lock`; `deny.toml` (cargo-deny) |
| Software bill of materials | CycloneDX 1.5 SBOM, deterministic (timestamp-free serial) | `scripts/gen-sbom.sh`; attached to tagged releases |
| Build provenance | SLSA build-provenance attestation over binary + SBOM | release workflow (`actions/attest-build-provenance`); verifiable with `gh attestation verify` |

The version + lockfile + SBOM + provenance attestation together form the **configuration baseline**: a reviewer can name the exact engine version, enumerate every dependency at its exact version and licence, and cryptographically trace a release artifact to the source that produced it.

---

## 3. Requirement → evidence mapping (ECSS-Q-ST-80 process areas)

The mapping below pairs ECSS-Q-ST-80 software-PA process areas (clause numbering indicative; to be reconciled against the project's tailored SPAP) with concrete, existing Kshana evidence. "Existing" means present in the released open engine today; "draft / founder-gated" means it must be formalised for the contract (see §7).

| # | ECSS-Q-ST-80 process area | Requirement intent | Kshana evidence (existing) | Status |
|---|---|---|---|---|
| PA-1 | Software dependability & safety (Q-ST-80 §6.2) | Dependability provisions proportionate to criticality; defensive design; no silent wrong answers in safety-relevant computation | Numerical solvers fail **closed**, not silent: `solve_kepler_checked` returns an error rather than a wrong root on non-convergence; the covariance update is Joseph-stabilised and Cholesky-checked in CI; a filter-consistency health monitor (NIS/NEES vs χ² bands) flips to *inconsistent* under tuning mismatch. The **machine-checked verification matrix** (`src/verification.rs`) enforces, by unit test, that a capability may be tagged **VALIDATED** only if it carries an independent external oracle — preventing a self-consistency check being dressed up as an external validation. | Existing; proportionate to a C/D analysis tool |
| PA-2 | Software V&V (Q-ST-80 §6.3) | Verification against requirements with documented oracles, tolerances and traceability; reproducible verification runs | ~1500+ unit/integration tests (Rust). A formal **requirement → module → test → oracle → OracleKind → status** matrix (`src/verification.rs`, `verification_matrix()`) — the executable backbone for an ECSS-E-ST-10-02 verification cross-reference. External-oracle validations include: Allan deviation vs **NIST SP 1065 / NBS14**; SGP4/SDP4 vs all **666 AIAA 2006-6753** vectors (4.12 mm); reference frames vs **IAU SOFA/ERFA + Vallado**; RAIM/ARAIM/SBAS vs **DO-229E + real IGS SP3**; ranging-code cross-correlation vs published **GPS C/A −23.9 dB**. Modelled-only items (clock holdover, quantum-inertial budget, CR3BP NRHO, nav-signal/jamming/spoof characterisation, alt-PNT) are tagged **MODELLED**, not validated, with their oracle kind stated. | Existing |
| PA-3 | Software configuration & documentation management (Q-ST-80 §6.4) | Identified, version-controlled, reproducible builds; controlled documentation; declarable software content | Git version control; pinned toolchain + committed `Cargo.lock`; CycloneDX SBOM; SLSA provenance; controlled documentation set (`docs/CAPABILITY.md`, `VALIDATION.md`, `REPRODUCIBILITY.md`, `INTEGRITY.md`, `CLAIMS-VS-REALITY.md`); a CI version-sync gate keeps the documented status consistent with the build. | Existing (a **formal CM plan** is a §7 gap) |
| PA-4 | Software problem / non-conformance handling (Q-ST-80 §6.3.5) | Defined problem-reporting and corrective-action route; closure with evidence; prevention of regression | The **overclaim closure ledger** `docs/CLAIMS-VS-REALITY.md` records 14 catalogued non-conformances (capability described more strongly than code delivered), each with original statement, resolution (de-claimed or superseded), evidence, and status — all GREEN. A **regression guard** (`tests/no_overclaims.rs`) scans public surfaces every CI run and fails if a retired overclaim phrase reappears, so a closed item cannot silently regress. This is a working NCR-and-corrective-action discipline; it is not yet a formal ECSS NCR register (see §7). | Existing (informal); formal NCR register is a §7 gap |
| PA-5 | Reuse of existing software (Q-ST-80 §6.2.7 / Annex on reuse) | Reused / OTS / open-source software assessed for fitness, licence, provenance and known limitations before reuse | Kshana **is** the reused software: a pre-existing, open-source (Apache-2.0) OTS component brought as **Background IPR** (`BIPR-declaration.md`). Its provenance is declarable (SBOM + SLSA), its dependency tree is permissive-only with no GPL/copyleft (cargo-deny `deny.toml`), and its **known limitations are documented up-front** rather than discovered in integration: `docs/CAPABILITY.md` is an explicit scope map (yes / partial / none) and `CLAIMS-VS-REALITY.md` is the closed-overclaim record. The **validated vs modelled** boundary tells the integrator exactly which results carry an external oracle. | Existing |
| PA-6 | Quality records (Q-ST-80 §6.5) | Objective evidence retained and retrievable: test results, reviews, baselines, provenance | CI run records (tests, clippy, fmt, reproducibility, cross-platform golden, no-overclaim, version-sync gates); per-scenario golden hashes committed in `tests/golden/`; the CycloneDX SBOM and SLSA attestation per release; the documentation set under version control. All are retrievable from the repository and release artifacts. | Existing |
| PA-7 | Software code quality / coding standards (Q-ST-80 §6.3.4) | Enforced coding standard and static analysis | `clippy` (lint) and `rustfmt` (format) gates enforced in CI; `.editorconfig`, `rustfmt.toml` committed; memory-safety properties from the Rust language and `#![forbid]`-style discipline. | Existing |
| PA-8 | Determinism & reproducibility of results (Q-ST-80 §6.3 verification repeatability) | Verification results must be repeatable | No wall-clock, no unseeded randomness; every stochastic process driven by a seeded `ChaCha8Rng` in fixed draw order. Byte-identical `result.json` on the same machine (`scripts/check-reproducible.sh`); FoM values pinned to 1e-6 across the 3-OS CI matrix; input/shape goldens pinned exactly per scenario; documented cross-platform ULP caveat (`docs/REPRODUCIBILITY.md`). | Existing |

---

## 4. Software dependability & safety — honest position for a C/D analysis tool

Kshana produces **analysis evidence**, not flight-control commands or telemetry, so its dependability case is that of a **lower-criticality (C/D)** support tool: the risk it must contain is *a wrong number reaching a design decision unflagged*, not a real-time hazard. The provisions that address that risk are concrete:

- **Fail-closed numerics** — solvers return errors rather than plausible-but-wrong answers when an algorithm leaves its valid domain (`solve_kepler_checked`, the AstroError contract on `vis_viva`-class helpers, guarded covariance updates).
- **An enforced honesty boundary** — the `src/verification.rs` invariants make the validated/modelled distinction a unit-tested property, not a prose assertion that can drift. A model cannot be presented as an external validation without an external-dataset oracle.
- **Self-consistency monitoring** — the filter-health monitor surfaces when an estimator's tuning is inconsistent with its data, so a degenerate run is flagged rather than reported as a clean result.

What this pack does **not** claim: any flight-software dependability case, any criticality-A provision, any independent dependability/safety analysis (FMECA, SFMECA, hazard analysis) of the simulator. None is appropriate for a ground analysis tool, and none has been performed. If the project's criticality analysis places any reused model higher than C/D, the additional ECSS-Q-ST-80 provisions for that category are a project-level activity to be defined in the SPAP.

---

## 5. Reuse-of-software statement (Kshana as OTS / BIPR)

Under ECSS-Q-ST-80's reuse provisions, reused and OTS software must be assessed before reuse. Kshana is offered as reused open-source OTS software and its assessment is already documented:

- **Identification & provenance** — engine v0.17.0, Apache-2.0; declarable dependency inventory (CycloneDX SBOM) with exact versions and licences; SLSA build provenance binds artifacts to source.
- **Licence fitness** — permissive (Apache-2.0, patent grant); transitive tree restricted to a permissive allow-list (Apache-2.0 / MIT / BSD-2/3 / ISC / Unicode / Zlib / CC0), **no GPL/copyleft**, enforced at build time by cargo-deny. This removes the open-source contamination risk ESA contracts guard against (see `BIPR-declaration.md`).
- **Fitness-for-purpose & known limitations** — stated up-front in `docs/CAPABILITY.md` (a per-capability yes/partial/none scope map) and `docs/CLAIMS-VS-REALITY.md` (the closed-overclaim ledger). The integrator is told exactly which capabilities carry an external oracle (**validated**) and which are first-principles/published models checked only internally (**modelled**), so no reused result is taken at a higher assurance level than it holds.

---

## 6. Verification cross-reference (extract)

The full machine-checked matrix lives in `src/verification.rs`; an extract showing the validated/modelled/partner classification used throughout this pack:

| Requirement area | Capability | Oracle | Oracle kind | Status |
|---|---|---|---|---|
| Frequency stability | Allan/MDEV/HDEV + power-law noise ID, χ² CIs | NIST SP 1065 / Stable32 on NBS14 (1e-4) | External dataset | **VALIDATED** |
| Orbit propagation | SGP4/SDP4 position & velocity | 666 AIAA 2006-6753 vectors (4.12 mm) | External dataset | **VALIDATED** |
| Reference frames | TEME→GCRS→ITRS reduction | IAU SOFA/ERFA + Vallado | External dataset | **VALIDATED** |
| Integrity | Snapshot/MHSS/ARAIM HPL/VPL | DO-229E + real IGS SP3 | External dataset | **VALIDATED** |
| Ranging-code design | Gold-code cross-correlation | Published GPS C/A −23.9 dB | External dataset | **VALIDATED** |
| GNSS-denied clock holdover | van-Loan coast-error growth to threshold; quantum-clock classes | Closed-form / first-principles (long-τ governed by a stated assumed red-noise floor) | Internal consistency | **MODELLED** |
| Quantum-inertial dead-reckoning | CAI bias + scale-factor + VRW + long-term budget over a holdover | First-principles Mach–Zehnder; below & within ~2 orders of the published Freier 2016 mobile-gravimeter floor | Internal consistency / loose external | **MODELLED** |
| Nav-signal / jamming / spoof | Modulation analysis; J/S link budget; TEXBAT-parameter spoof characterisation | Published formulae / TEXBAT scenario parameters | Internal consistency | **MODELLED** |
| Cislunar / alt-PNT | CR3BP NRHO; gravity/terrain/magnetic nav | Published L-point values / first-principles | Internal consistency | **MODELLED** |
| Flight-hardware PA (radiation/EEE/MAIT) | — | — | None | **PARTNER-OWNED** |
| Spacecraft bus / RF & quantum payload HARDWARE | — | — | None | **PARTNER-OWNED** |

The matrix's unit tests enforce that a **VALIDATED** row must carry an **external-dataset** oracle and that a **PARTNER-OWNED** row claims no module, test or oracle — so the boundary in §1.2 is machine-checked, not just asserted here.

---

## 7. Gaps and founder-gated items (honest)

These are required for a contract-grade software-PA posture and are **not yet** in place. They are stated plainly because honesty is the bid's differentiator; none is a code capability that can be conjured, and several depend on the real SoW and teaming.

| Gap | What is missing | Owner / gate |
|---|---|---|
| **Formal PA plan sign-off** | A signed project-tailored Software Product Assurance Plan (SPAP) referencing this evidence, aligned to the prime's project PA plan and the tailored ECSS-Q-ST-80 requirement set. This pack is its input, not a substitute. | Founder-gated; awaits FPD-10029 SoW + prime's PA plan |
| **Criticality categorisation** | The B/C/D (or A) category for each reused model must be assigned by the project criticality analysis against the SoW. This pack assumes C/D and flags that explicitly. | Consortium criticality analysis (prime-led) |
| **Configuration-management plan formalisation** | The CM discipline exists (version control, lockfile, SBOM, provenance, golden baselines) but is not yet written up as a formal ECSS-Q-ST-80-compliant Software Configuration Management Plan with baselines, change control board, and CR/PR workflow. | Founder-gated; to be formalised for the contract |
| **Formal NCR register** | The overclaim ledger + regression guard is a working corrective-action discipline, but a formal non-conformance / problem-report register with the project's classification, disposition and review-board route is not yet established. | Founder-gated; to align with the project NRB |
| **Independent SW-PA audit** | No independent (third-party / prime-PA-led) software product-assurance audit of Kshana has been performed. The verification-matrix invariants are self-checked in CI; an external PA audit is a separate, welcome activity. | Prime / ESA PA; not yet performed |
| **Cross-platform bit-identity** | Results are bit-identical per platform and agree to 1e-6 across OS; full bit-identical-across-OS is an open item, stated honestly (platform libm ULP differences). | Documented limitation (`REPRODUCIBILITY.md`) |
| **Flight-software regime** | If any model is re-targeted as onboard software, a flight-software development under the project's flight-SW PA regime is required — out of scope here. | Project flight-SW PA (if applicable) |

---

## 8. Summary

Kshana brings a **real, existing software-quality infrastructure** that maps cleanly onto the ECSS-Q-ST-80 software-PA process areas appropriate to a criticality-C/D navigation-performance analysis tool: a ~1500+-test V&V base with documented external oracles and a machine-checked validated/modelled boundary, a configuration baseline (pinned toolchain + lockfile + CycloneDX SBOM + SLSA provenance), enforced code-quality and reproducibility gates, a working problem/closure discipline (the overclaim ledger + regression guard), and an honest reuse/OTS assessment with a permissive, copyleft-free dependency tree.

What it does **not** do is equally clear and is the point of §1.2 and §7: this is **software PA only**. Flight-hardware PA (radiation, EEE/COTS, MAIT, qualification), the spacecraft bus, and the RF and quantum **hardware** are a consortium partner's responsibility; the formal PA-plan sign-off, the CM-plan formalisation, and the independent SW-PA audit are founder-gated and await the real SoW and teaming. Within its honest scope, Kshana is ready to be the navigation-performance and trade-study software substrate, with the assurance evidence to back it.

---

*DRAFT — to be reconciled against ESA AO/1-13503 FPD-10029 and the consortium PA plan. PA / quality matters: **contact@ashforde.org**. Author / owner: Ashforde OÜ — Chakshu Baweja.*
