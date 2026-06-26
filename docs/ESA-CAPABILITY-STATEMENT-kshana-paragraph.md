<!--
PRIVATE — Ashforde OÜ internal. Do not distribute outside controlled outreach.
Lives in the private kshana-pro repo only; never publish in the open kshana repo.
Purpose: ready-to-paste Kshana credibility/heritage copy for ESA prime outreach
and capability statements. MAESTRO-aware (Kshana is heritage/evidence, NOT the
thing being bid into AO/1-1-13313).
Contact for all external use: contact@ashforde.org
-->

# Kshana — capability-statement copy for ESA outreach

**Audience:** ESA tender primes / partners evaluating Ashforde OÜ as a (sub)contractor.
**Role of Kshana here:** *evidence of engineering and assurance capability* — not the
product being bid. Keep that framing intact.

---

## A. Tight drop-in (3–4 sentences)

> Ashforde OÜ's space-domain engineering credibility is independently demonstrated by
> **Kshana**, our open, reproducible engine for PNT-resilience comparison (positioning,
> navigation, and timing). Kshana is validated against independent oracles — SGP4 against
> the AIAA/Vallado verification set, reference-frame transforms against SOFA/ERFA, and its
> force-model fit against agency precise ephemerides (sub-metre reduced-dynamic residual
> over a multi-hour arc) — with every result reproducible from a committed scenario, a
> fixed seed, and a pinned engine version, and every figure-of-merit labelled
> *validated-or-modelled*. The same engine runs natively, from Python, and zero-install in
> the browser via WebAssembly, and is documented in a Zenodo-archived, citable preprint.
> It is released open-core under the GNU AGPL-3.0 (dual-licensed; a commercial licence is
> available) with a private, contract-extensible overlay
> (**Kshana Pro**) — evidence both of our engineering depth and of a sustainable model for
> delivering and maintaining proprietary, export-sensitive work under contract.

## B. MAESTRO-aware bridge sentence (append when the statement accompanies the AO/1-1-13313 bid)

> This is direct evidence of the engineering and assurance culture Ashforde brings to
> MAESTRO: results that are validated against independent oracles, fully reproducible, and
> honestly scoped — exactly the qualities an agentic co-worker for flight operations must
> inherit.

*(Note: phrased as culture/qualities, **not** code reuse. MAESTRO is a separate,
greenfield product; do not imply Kshana powers it.)*

---

## C. Fuller half-page version (paragraph + evidence bullets)

**Ashforde OÜ** is an Estonian SME building assured, reproducible software for space
operations. Our engineering capability is independently evidenced by **Kshana**, an open,
reproducible engine for comparing positioning-, navigation- and timing-resilience across
quantum and classical sensor architectures. Rather than asserting verdicts, Kshana fixes a
scenario, a seed, and an engine version, routes candidates through a single neutral code
path, and labels every figure-of-merit as externally validated or modelled — an
assurance discipline carried through to the output format itself.

- **Validated against independent oracles.** SGP4 against the AIAA/Vallado verification
  set; reference-frame transforms against SOFA/ERFA reference vectors; force-model fit
  against agency precise ephemerides (sub-metre reduced-dynamic residual over a multi-hour
  arc); metre-level single-point positioning on real IGS observations.
- **Reproducible by construction.** Every figure and number regenerates from
  `scenario + seed + engine version`; outputs are schema-versioned and self-describing,
  with per-module provenance carried inline.
- **One engine, three targets.** A single Rust core compiles to a native binary/library, a
  Python extension, and a zero-install WebAssembly build, and is callable by AI assistants
  over MCP — identical validated logic across all surfaces.
- **Standards-fluent.** Reads/writes RINEX, IGS SP3, and CCSDS OEM/OMM, interoperating with
  established pipelines (RTKLIB, GMAT, Orekit).
- **Engineering hygiene.** High line-coverage with a CI gate, a CycloneDX SBOM, and SLSA
  build provenance on every release.
- **Citable and open.** Dual-licensed under the GNU AGPL-3.0-only with a commercial licence
  available, archived on Zenodo with a DOI
  (10.5281/zenodo.20528627) and documented in an arXiv preprint.
- **Sustainable commercial model.** Open core plus a private overlay (**Kshana Pro**) for
  proprietary and export-sensitive work delivered under NDA and the appropriate clearance —
  demonstrating we can both open-source for transparency and hold/extend controlled IP
  under contract.

*Optional eligibility line (include where relevant):* Ashforde OÜ is registered in
ESA-STAR (entity ID 83208) as an SME and is eligible to bid.

---

## D. Usage notes / guardrails

- **Always use** the business address **contact@ashforde.org** (never the personal address).
- **Do not overclaim.** Kshana is a *simulation substrate, not a flight product*; claim
  validation and reproducibility, **not** TRL levels, certification, or ESA endorsement.
- **Keep Kshana ≠ MAESTRO.** In a MAESTRO context, Kshana is heritage/credibility; MAESTRO
  is the bid. Use section B's wording to connect them without implying code reuse.
- **Numbers:** the "sub-metre" residual refers to the reduced-dynamic fit (~0.07 m at
  degree/order 12 over ~8 h); the 24-hour, degree/order-70 headline is ~0.61 m. Quote the
  range honestly if a reviewer presses for specifics.
- **To tailor to a named prime,** swap the opening clause to name the collaboration context
  and foreground the one or two evidence bullets most relevant to their work package.
