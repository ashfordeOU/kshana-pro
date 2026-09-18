# Release-Readiness Assessment — Lunar-PNT Paper (arXiv draft)

**Lead editor verdict: NOT release-ready as-is; one focused revision pass takes it there. Release-ready after the integration plan below.**

---

## 1. Where it stands

This is a strong, honest, fully-reproducible paper let down by stale currency and citation hygiene — not by its science. I verified the actual files, the bib (15 keys), the engine (Cargo.toml v0.22.0), and the verification matrix directly.

**What is already excellent (do not touch):**
- **The result reproduces from the live AGPL engine.** Every headline number falls straight out of running the public `kshana <scenario>` binary, several byte-for-byte against committed artifacts: 200-seed median 91x (recomputed 91.147x), range-only ~793 m (792.87 m), with-VLBI ~13 m (13.45 m), IQR 38-208x (37.80-207.68), min 0.91, max 10442; cross-suite LTC 57.04 us/day (+57.50 self-potential / -0.46 kinetic), VLBI near-field 58.9 us on a 10727 km baseline, differential PNT 24.5 m -> 0.008 m at 50 km (3186x). The single-seed default (615x) is correctly NOT presented as the headline; the paper pre-discloses it as upper-tail.
- **Honesty is machine-enforced and intact.** `src/verification.rs` + `tests/no_overclaims.rs` pass; the no-overclaims gate blocks labelling any internal-consistency/sibling-reimpl oracle as Validated. Four lunar rows are genuinely externally Validated (coordinate-time vs Ashby & Patla 2024 + DE440; frame-realisation vs Umeyama-SVD; nav-service-volume vs ANISE; cislunar-NRHO vs JPL); the core joint-OD/VLBI/dPNT/interop rows are honestly Modelled. I confirmed the LTC row is `VerificationStatus::Validated` / `OracleKind::ExternalDataset` in the source.
- **The methodology section is the cleanest in the paper** — code-faithful (16 params, Gauss-Newton snapshot batch LS, scaled units, clock-sync pseudo-observable), with two minor precision fixes only.
- **The limitations section is correct and load-bearing** — the non-affiliation disclaimer must be preserved verbatim.

**What blocks release (currency + citations, not correctness):**
The abstract and intro name four real programmes/standards (Moonlight/LCNS, NovaMoon, LunaNet/IOAG, Coordinated Lunar Time) as the paper's entire motivation and cite **nothing credible** for any of them. The one lunar-time citation used — `petit2010iers` (IERS Conventions 2010) — is an Earth-only document that does not define lunar time and is the **wrong source**. The intro calls NovaMoon a 'concept' (ESA-adopted at CM25, Nov 2025) and Moonlight 'will field' (past prime contract, in industrial build), and even contradicts its own abstract ('a defined Coordinated Lunar Time' vs 'has been called for'). The single most relevant real-world event of the last 18 months — **LuGRE's first GNSS fix on the lunar surface (March 2025)** — is absent from the entire paper. For a 2026 lunar-PNT arXiv submission these are conspicuous, reviewer-flagging gaps.

## 2. Dominant positioning (the one frame to lead with)

**Make NovaMoon the corroborating witness, not a competitor.** ESA's adopted NovaMoon reference station independently carries an Earth-baseline VLBI transmitter — exactly the tie this paper's observability result identifies as necessary. State plainly: *the Earth-baseline geodetic VLBI tie our result shows is required is the design choice ESA has already committed to in NovaMoon.* That turns an 85-author, ESA-adopted programme into third-party validation of the thesis while staying scrupulously unaffiliated.

Stack three wedges on top: (1) **Currency** — lunar PNT moved from study to procurement in 2024-2026 (Telespazio EUR123M Oct 2024; Thales Alenia Space space segment Mar 2025; EUR176M + NovaMoon at CM25 Nov 2025; LNIS V5 + AFS Jan 2025; LuGRE Mar 2025; TCL defined by IAU 2024 Res II), so an open, regenerable observability tool fills a real, dated gap that primes currently fill with private one-off studies. (2) **Cross-agency scope** — ESA LCNS, NASA LCRNS, and JAXA LNSS all broadcast the same one-way Augmented Forward Signal under LNIS V5, so the weak-absolute-observability gap is common to all three, not ESA-specific. (3) **Honesty as moat** — machine-enforced per-figure VALIDATED-vs-MODELLED labelling, an information-not-count control proving geometry not data-volume, and pre-disclosure of the upper-tail seed — none of which LuPNT or closed mission-analysis advertise.

## 3. Honesty findings (both improve the paper)

- **Under-claim to fix:** the cross-suite caption labels the whole panel 'Modelled', but the LTC rate is Validated in the engine's own matrix. Splitting it per-panel (LTC Validated; VLBI near-field + dPNT Modelled) is both more accurate and stronger — nothing is overclaimed because the validation already exists in code.
- **Provenance gap to fix:** `baweja2026kshana` is pinned to v0.17.0 while the artifacts are produced by v0.22.0. The abstract's 'reproduce from a committed engine version' promise is undercut by the mismatch; bump the bib.

## 4. Additional analyses worth adding (all runnable from the existing engine, zero fabrication)

The strongest, lowest-effort additions are the **information-not-count sigma_vlbi curve** and the **n_earth rank sweep** promoted to labelled figures (the data already exists, regenerated live and byte-identical to committed artifacts), plus an **LTC reference-surface sensitivity table** reconciling 57.04 against the canonical 56.02 (arithmetic on existing outputs). An **LTE440 cross-check** is a credible stated future-work line (or modest validation appendix). A **per-state selectivity table** makes the 'selective' claim auditable. None require new Monte Carlo runs.

## 5. Bottom line

The science, reproducibility, and honesty discipline are arXiv-grade and rare. The gap is a single editorial pass: add ~9 real citations, fix one wrong-source and one mis-citation, update three stale-maturity phrasings, add the LuGRE reality anchor and the NovaMoon corroboration sentence, split one caption label, and bump one bib version. No new runs, no fabrication. After that, release.