<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Capability evidence for AO/1-13503: Kshana's simulation-based demonstration of all three SoW §1.5.2
application areas, anchored on Clarifications 1-3. Contact: contact@ashforde.org
-->

# Kshana — Quantum-Enabled PNT demonstrator capability (AO/1-13503)

**PRIVATE.** Evidence that Kshana is the open, reproducible, **requirement-compliant simulation-based
demonstration environment** for a quantum-PNT demonstrator across **all three** SoW §1.5.2 application
areas. Anchored on the ITT clarifications (see [`CLARIFICATIONS-AUDIT.md`](CLARIFICATIONS-AUDIT.md)).
Business contact **contact@ashforde.org**.

## Why this is compliant (the clarification hooks)

- **Clar 1 / A3** — ESA states a *simulation environment* is a compliant way to demonstrate feasibility,
  performance and operational benefits, *"provided representativeness is justified and remaining gaps
  towards flight are clearly identified."* Kshana makes that a **machine-checked output**: every
  demonstration carries a `representativeness` record (external anchors · modelled assumptions ·
  gaps-to-flight · representative TRL band), with invariants enforced in code (`src/representativeness.rs`,
  `tests`). Our honesty discipline **is** the A3 compliance mechanism.
- **Clar 1 / A1** — a single focused area is compliant; depth is rewarded. We can lead the bid with any
  one of the three areas below, each built to dominant depth, or present them as a coherent demonstrator.
- **Clar 3** — FPD-10031 PA&S is a Reference (no SoC); an internal PA process is expected. Kshana's CI
  gate + machine-checked `verification.rs` + SBOM/SLSA **is** that internal PA process (see
  [`software-PA-evidence.md`](software-PA-evidence.md)).

## The three application areas — built, runnable, reproducible (in open `kshana`)

| SoW §1.5.2 area | Kshana scenario | What it demonstrates | Honest result | Status |
|---|---|---|---|---|
| Optical timing / quantum time transfer / **secure time dissemination** | `quantum-time-transfer` | End-to-end quantum (optical-lattice clock + entanglement link) vs classical (CSAC + RF) time-transfer budget; reused timing protection level; delay/replay-attack security FoM (1−P_md); clock-anomaly detection + CUSUM latency; quantum-vs-classical trade | quantum chain ~1.2 ps vs classical ~5.3 ns (default config); security P_d=1.0 vs a 5 ns attack at the modelled monitor | MODELLED (ADEV/TPL/detection validated-by-reuse) |
| Quantum sensors for **spacecraft navigation** (GNSS-free) | `quantum-gnss-free-nav` | Quantum (cold-atom interferometer) vs classical nav-grade INS dead-reckoning over a GNSS outage; position-error growth; holdover; observability (bias unobservable without a fix → error grows) | quantum holds a 100 m bound ~44 700 s vs ~530 s classical; ~8000× lower error over a 300 s outage (default) | MODELLED (CAI validated-by-reuse ~2 orders vs Freier 2016) |
| **Fault / anomaly detection** for quantum systems | `quantum-anomaly-detect` | Labelled fault catalog (clock jump/drift/lock-loss; sensor bias/dropout); detection ROC AUC (bootstrap CI) + minimum-detectable fault; quantum-clock-aided vs classical monitor | quantum-aided AUC 0.99 vs classical 0.76; detects ~3× smaller faults (default) | MODELLED (AUC cross-checked vs validated `eval_stats`/scikit-learn) |

All three run from the public engine (`kshana <scenario>.toml`) and bundle into one study
(`kshana --study scenarios/quantum-pnt-demonstrator.suite.toml`) — a single reproducible evidence pack.
Every figure of merit is emitted as a uniform `TradeEvidence` object (fixed frame: scenario+seed+engine;
per-FoM quantum-vs-classical with a validated/modelled label) carrying its representativeness record.

> **Honest framing (kept verbatim in the proposal):** these are **MODELLED** simulation demonstrations,
> not real-data validations; each names its gaps-to-flight; Kshana models the *class* of system; example
> device/link parameters are illustrative, public-source. **No TRL > 3, no flight heritage, no
> certification, no ESA endorsement.** Where a result rides on an externally-validated kernel
> (ADEV vs Stable32/NIST; CAI vs Freier 2016; AUC vs scikit-learn; DOP vs gnss_lib_py) we say
> "validated by reuse" — not "validated".

## What Ashforde offers the consortium (the WP)

A named consortium WP delivering the **simulation-based feasibility-, performance- and
operational-benefit demonstration** for the selected application area(s) at TRL 3/4→4/5→5/6, with
representativeness justified and gaps-to-flight enumerated (A3) — paired with the hardware partner's
lab/EM work for device-level maturation. Plus the same engineering-and-assurance evidence the FutureNAV
C3 assets carry. Route in: named consortium member (mission-analysis/SE/software), **not** a hardware
critical pre-development (so the open-competition-before-submission rule of Clar 1/A2 does not gate us).

## Honest gaps (stated, not hidden)

- The quantum-vs-classical results are simulation demonstrations; real hardware-in-the-loop and flight
  telemetry are the gaps-to-flight named in each scenario's representativeness record.
- The entanglement time-transfer link and the anomaly detection-statistic are modelled (shot-limited /
  Gaussian) — calibrated to published behaviour, not measured on flight hardware.
- Validated-island expansion (turning more results from MODELLED to externally-VALIDATED against public
  datasets) is a tracked follow-on, pursued where a public dataset exists.

---

*Ashforde OÜ · contact@ashforde.org · Kshana — open AGPL-3.0-only (commercial licence available) ·
TRL ~3, no flight heritage, no certification, no ESA endorsement · validated (external oracle) / modelled
labels per the open repo's `verification.rs` + per-result `representativeness` ledger · outreach PARKED
pending founder go.*
