<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
T9 — Kshana capability brief for the AO/1-13503 consortium. 1–2 pp.
Drop-in for a prime's proposal annex or a teaming pitch. Contact: contact@ashforde.org
-->

# Kshana — capability brief for ESA AO/1-13503 (Quantum-Enabled PNT Demonstrator)

**Ashforde OÜ** offers a consortium the **navigation-performance and trade-study work
package** for Phase 0/A: the quantum-vs-classical PNT feasibility core, GNSS-denied
resilience, mission analysis, and timing architecture — delivered through **Kshana**, our
open-core, reproducible PNT-performance-and-resilience simulator (v0.20.0, dual-licensed: GNU AGPL-3.0-only + commercial licence). Kshana
is a *performance-simulation substrate*, not a spacecraft-systems house or a flight
product: it answers "how well does this PNT architecture perform, and how does it degrade"
with validated, reproducible numbers — exactly the evidence Phase 0/A trade studies need.

## Why this matters for the demonstrator

The demonstrator's central question is **what does quantum buy over classical for PNT, and
where does it pay off**. That comparison is Kshana's entire purpose: a single neutral code
path scores candidate architectures on the same figures of merit, with every result
reproducible from a committed scenario, a fixed seed, and a pinned engine version, and
every figure-of-merit labelled **validated-or-modelled**. No hand-tuned verdicts.

## What Kshana covers (Phase 0/A home turf)

- **Quantum-vs-classical PNT performance.** Clock models (optical / CSAC / chip-scale),
  optical & RF two-way time transfer, and a **first-principles cold-atom interferometer
  (CAI) accelerometer** (three-pulse Mach–Zehnder phase `Φ = k_eff·a·T²`, shot-noise floor,
  Coriolis & AC-Stark systematics), with scored figures of merit, holdover, and Monte-Carlo
  sweeps. *Honest scope:* most quantum sensors are modelled from **published Allan/noise
  coefficients**; only the CAI accelerometer is first-principles (validated to within ~2
  orders of the Freier et al. 2016 mobile gravimeter, 96 nm/s²/√Hz). Performance modelling,
  not device hardware design.
- **GNSS-denied resilience.** Anti-jam link budget (Kaplan & Hegarty §9.4 effective-C/N₀),
  a multi-layer spoof-detection suite (clock-aided χ² + RAIM-consistency + AGC + SQM,
  characterised against the published TEXBAT scenario parameters), and **alternative PNT** —
  cold-atom gravity-map matching (≈145 m over a 60-min GPS-denied outage where free-INS
  drifts to ~70 km), terrain-referenced navigation (TERCOM/SITAN), and IGRF-14
  magnetic-anomaly matching, fused in a combined alt-PNT navigator.
- **Mission analysis.** Walker constellation design + a gradient-free design optimiser,
  coverage / revisit / DOP / availability, dependency-free **SGP4/SDP4 validated to 4.12 mm
  vs all 666 AIAA 2006-6753 vectors**, a numerical Cowell propagator (zonal gravity through
  d/o 6, third-body Sun/Moon, SRP with conical shadow, drag, relativistic corrections), and
  orbit determination (Gauss–Newton batch + sequential).
- **Timing architecture.** Overlapping ADEV/MDEV/TDEV/HDEV with noise-type-specific
  confidence intervals (NIST SP 1065, validated to 1e-4 vs Stable32 on NBS14), a
  van-Loan-Q Kalman clock with a filter-consistency health monitor, holdover FoM, clock
  ensembles, and a two-way time-transfer model with reciprocal-delay cancellation.
- **Integrity.** Snapshot RAIM, solution-separation (MHSS) RAIM, and an **ARAIM P_HMI
  budget** with single-SV and constellation-wide fault modes (DO-316 / EU-ARAIM-TN
  parameters), plus a Stanford integrity diagram — exercised on real IGS SP3 geometry and
  real GPS+Galileo TLEs.
- **Reference frames & reproducibility.** IAU 2006 precession + IAU 2000A/2000B nutation +
  the CIO-based GCRS↔ITRS chain, **bit-exact against the SOFA/ERFA reference vectors** and
  pinned to the Vallado worked example (full CIO GCRS→ITRS to 4 mm); deterministic,
  scenario-hashed outputs.
- **Cislunar / deep-space (adjacent FutureNAV themes).** Earth–Moon CR3BP + Lagrange points,
  LunaNet/LNIS lunar ARAIM, and a v0.17.0 Mars radiometric-navigation engine
  (light-time/Shapiro, two-/one-/three-way Doppler & range, reduced-dynamic SRIF), Sun-central
  dynamics cross-checked vs JPL DE440 (137 m on a 1-day arc). *Honest scope:* simulation-
  validated; no real DSN/ESTRACK tracking-data validation yet.

## Validation & assurance culture (ECSS-friendly evidence)

Kshana is validated against **independent oracles** — AIAA/Vallado for SGP4, SOFA/ERFA for
frames, NIST SP 1065 / Stable32 for Allan, real IGS data for single-point positioning
(5.7 m 3-D RMS), ESA/agency SP3 for orbits — and ships a public **honest maturity ledger**
(`docs/CAPABILITY.md`, `docs/VALIDATION.md`, `docs/CLAIMS-VS-REALITY.md`) that states, per
capability, exactly what is *yes / partial / none*. Reproducible by construction; CI-gated;
CycloneDX SBOM and build provenance. This is the assurance discipline an ECSS study rewards.

## How it runs (a reviewer can actually use it)

One Rust core compiles to a native binary/library, a Python extension, and a zero-install
**WebAssembly browser playground**, and is callable by AI assistants over **MCP** — identical
validated logic on every surface. Reads/writes RINEX, IGS SP3, and CCSDS OEM/TDM,
interoperating with RTKLIB / GMAT / Orekit pipelines. Documented in an arXiv preprint.

## Honest boundaries (what Kshana is *not*, for this tender)

- **Not** a spacecraft-bus engineering capability (AOCS, thermal, structures, propulsion,
  EEE/power) — those are consortium partner WPs.
- **Not** a nav-RF-payload / signal / antenna designer — Kshana models the **measurement
  domain and link budget**; payload & signal design needs an RF/payload partner. Kshana
  feeds performance requirements to it.
- **Not** a quantum-hardware design/maturation house — it models device performance from
  published coefficients (CAI first-principles excepted); the quantum-payload developer is
  the consortium's technical anchor.
- **TRL ~3** simulation substrate; no flight heritage, no ESA contracts yet.

## What Ashforde adds to a consortium

- The on-thesis **navigation-performance & trade-study WP** for Phase 0/A.
- A **German-nationality SME** (ESA-STAR entity 83208) — helps the ≥3-SME quota and the
  DE-weighted geographical return.
- A runnable, citable, openly validated tool a prime can show ESA as a credibility asset.

---
*Open AGPL-3.0 core (dual-licensed; a commercial licence is available from Ashforde OÜ); a private
contract-extensible overlay (Kshana Pro) is available for proprietary / export-sensitive work under NDA
and the appropriate clearance. All external contact via **contact@ashforde.org**.*
