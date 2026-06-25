<!--
PRIVATE — Ashforde OÜ internal. Do not distribute outside controlled outreach.
Lives in the private kshana-pro repo only; never publish in the open kshana repo.
Purpose: ready-to-paste Kshana LUNAR-PNT credibility copy for ESA lunar-programme outreach
(Moonlight / LCNS, NovaMoon-class geodetic/timing/differential station, LunaNet/IOAG interop).
Kshana is heritage/evidence + a mission-analysis substrate — NOT the thing being flown.
Contact for all external use: contact@ashforde.org
-->

# Kshana — lunar-PNT capability-statement copy for ESA outreach

**Audience:** ESA lunar-PNT programme primes / partners (Moonlight/LCNS, a NovaMoon-class lunar
geodetic-timing-differential station, LunaNet/IOAG interoperability) evaluating Ashforde OÜ as a
(sub)contractor or mission-analysis partner.
**Role of Kshana here:** *open, reproducible evidence of lunar-PNT engineering capability* and a
ready mission-analysis / benefits-case substrate — **not** a flight product and **not** any ESA
programme. Keep that framing intact.

---

## A. Tight drop-in (3–4 sentences)

> Ashforde OÜ's lunar-PNT engineering capability is independently demonstrated by **Kshana**, our
> open, reproducible engine, which now carries an end-to-end lunar PNT suite: Coordinated Lunar Time
> (the relativistic Earth–Moon clock rate, reproduced inside the published ~56–59 µs/day band),
> Earth-baseline lunar geodetic VLBI, joint multi-technique orbit-and-clock determination, lunar
> reference-frame realisation, navigation service-volume integrity, differential PNT, and
> LunaNet/IOAG-aligned interoperability export. Every result is reproducible from a committed
> scenario, a fixed seed, and a pinned engine version, and every figure of merit is labelled
> *validated* (against an independent oracle) or *modelled* — the same honesty invariant the code
> enforces in its public test suite. Kshana **models the class of system** ESA's Moonlight/LCNS and a
> NovaMoon-class lunar station build; it is neither affiliated with nor endorsed by any ESA programme.
> It is released open-core under the AGPL-3.0 with a commercial licence available, giving a clean route
> to deliver and maintain proprietary, export-sensitive lunar work under contract.

## B. Per-concept bridge sentences (append the one that fits the conversation)

- **Moonlight / LCNS:** *Kshana sweeps a lunar service volume for DOP, coverage, availability and an
  ARAIM-style integrity protection level over an illustrative LCNS-class constellation, reusing the
  geometry kernel that is independently validated against `gnss_lib_py` — a neutral, reproducible way
  to quantify what a candidate lunar nav service buys, before hardware exists.*
- **NovaMoon-class geodetic/timing/differential station:** *Kshana models each of the station's core
  functions as a separate, testable observable — VLBI delay, joint orbit+clock estimation, frame
  realisation, lunar time, differential corrections — so a mission-analysis team can study the design
  trade openly rather than in a private one-off notebook.*
- **LunaNet / IOAG interoperability:** *Kshana emits lunar ephemeris and time products as
  LunaNet/IOAG-aligned CCSDS OEM with the correct lunar reference frame and time-system tags, and
  round-trips them through its own parser as a conformance check — evidence of standards-fluency, not a
  certified conformance test.*

*(Phrase every bridge as engineering capability and reproducible evidence — never as code reuse into,
or endorsement by, a named programme.)*

---

## C. Fuller half-page version (paragraph + evidence bullets)

**Ashforde OÜ** is an Estonian SME building assured, reproducible software for space operations. Our
lunar-PNT capability is independently evidenced by **Kshana**, an open engine that now models the full
chain a lunar PNT architecture needs — time, geodesy, orbit-and-clock determination, reference frame,
navigation-service integrity, differential corrections, and interoperability — as one reproducible,
honestly-scoped simulator. Rather than asserting verdicts, Kshana fixes a scenario, a seed, and an
engine version, and labels every figure of merit as externally validated or modelled.

- **Coordinated Lunar Time, from first principles.** A relativistic Earth–Moon clock-rate model
  decomposes the secular rate into its self-potential and kinetic terms and reproduces it **inside the
  published ~56–59 µs/day band** (≈57 µs/day at a representative epoch), with reversible TT↔LTC
  conversions and an inverse-variance lunar ensemble. *Modelled; cross-checked against the published
  lunar-clock-rate band and an internal round-trip identity.*
- **Earth-baseline lunar geodetic VLBI.** A near-field VLBI delay observable for an Earth baseline and
  a lunar-surface beacon, with analytic partials; in the far-field limit it **reproduces the engine's
  independently-built Δ-DOR delay to machine precision**, and it exposes the ~59 µs lunar-distance
  near-field correction the far-field model omits. *Modelled; reference-implementation cross-check.*
- **The observability result that matters.** In a joint multi-technique fit (VLBI + lunar-local ranging
  + inter-satellite ranging), adding a single Earth-baseline VLBI station **sharpens a lunar surface
  station's recovered 3-D position from ~2183 m to ~3.6 m — a ~615× improvement** — because VLBI
  restores the weakly-observed direction that ranging alone leaves loose. *Modelled, simulated
  closed-loop recovery; reported as the with-vs-without-VLBI contrast.*
- **Reference-frame realisation.** A seven-parameter Helmert fit ties an estimated lunar network to a
  datum with an ICRF orientation tie, recovering an injected transform to machine precision in the
  noiseless case and degrading honestly with measurement noise. *Modelled; internal-consistency oracle.*
- **Navigation service-volume integrity.** A selenographic grid × time sweep yields DOP, coverage,
  availability and an ARAIM-style protection level over an illustrative LCNS-class constellation,
  **reusing the dilution-of-precision kernel validated against `gnss_lib_py`**. *Geometry kernel
  validated by reuse; the lunar service envelope is modelled.*
- **Differential PNT.** A NovaMoon-class reference station's corrections cancel common-mode orbit and
  clock error: at a 50 km user baseline a standalone **~24 m error drops to ~0.008 m**, with the
  residual growing with baseline as decorrelation predicts. *Modelled; internal-consistency oracle.*
- **Interoperability.** Lunar ephemeris/time export as LunaNet/IOAG-aligned CCSDS OEM (correct lunar
  reference-frame and time-system tags), round-tripped through the engine's own parser as a conformance
  check. *Modelled; round-trip / field-conformance oracle.*
- **Built on a validated terrestrial core.** Reference-frame transforms agree bit-for-bit with
  SOFA/ERFA; SGP4 matches the AIAA/Vallado verification set; force-model fit against agency precise
  ephemerides gives sub-metre reduced-dynamic residuals — the lunar suite reuses this validated
  machinery rather than re-deriving it.
- **Reproducible, citable, open.** Every figure regenerates from `scenario + seed + engine version`;
  outputs are schema-versioned and self-describing. AGPL-3.0-only with a commercial licence available;
  archived on Zenodo with a DOI and documented in arXiv preprints (a lunar geodetic-VLBI observability
  preprint is in preparation).

*Optional eligibility line (include where relevant):* Ashforde OÜ is registered in ESA-STAR (entity
ID 83208) as an SME and is eligible to bid.

---

## D. Usage notes / guardrails

- **Always use** the business address **contact@ashforde.org** (never the personal address).
- **Kshana models the *class* of system, it is not the system.** Never say or imply Kshana is, powers,
  or is endorsed by Moonlight, LCNS, NovaMoon, LunaNet, or any ESA programme. Example constellation /
  station parameters are *illustrative, public-source, not affiliated with ESA*.
- **Do not overclaim.** Claim validation and reproducibility, **not** TRL levels, certification, flight
  heritage, or ESA endorsement. The lunar suite is **modelled** (calibrated to public oracles) except
  where it reuses an already-validated kernel (DOP vs `gnss_lib_py`; Earth orientation vs SOFA/ERFA) —
  say "validated by reuse", not "validated", for those.
- **Numbers, stated honestly:** the ~615× VLBI sharpening (~2183 m → ~3.6 m) and the ~3186× differential
  reduction (~24 m → ~0.008 m) are **simulated closed-loop** results (truth shares the observation
  model), deterministic and seeded, **not real-data validated**. Quote them as what they are: an
  observability / decorrelation demonstration in an open simulator. The lunar time rate is
  reference-dependent; always quote the **band** (56–59 µs/day), not a single absolute figure.
- **Keep the commercial thesis out of ESA-facing copy.** Outreach framing is open, neutral,
  reproducible heritage that strengthens a consortium. The broader market strategy stays in private
  strategy notes; it does not belong in any artifact that could reach ESA or a prime.
- **To tailor to a named concept,** open with the matching section-B bridge and foreground the two or
  three evidence bullets most relevant (e.g. lead with VLBI + joint OD + frame for a geodetic-station
  conversation; lead with service-volume + differential for a Moonlight/LCNS conversation).
- **Cite the preprint only once it is posted** (arXiv-first; nothing public until the founder announces).
