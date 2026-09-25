<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Partner outreach: draft emails

**Owner:** Ashforde OÜ · **Status:** DRAFTS ONLY — nothing has been sent · **Date:** 2026-09-25

> **Do not send any of these without the founder reading and approving each one.**
> They are written to go from `contact@ashforde.org` only. They quote nothing
> non-public: no prices, no study offer page, no Kshana Pro material. The timing study
> is behind the Pack B export-control hold (`legal/EXPORT-CONTROL-DETERMINATION.md`
> section 5), so these emails ask about **data, validation and standards**, and mention
> studies only as a future possibility. Record each send in the table at the end of
> [`TIMING-PARTNERS.md`](TIMING-PARTNERS.md).

Organisation names are listed in full in [`TIMING-PARTNERS.md`](TIMING-PARTNERS.md): NPL (National Physical Laboratory), PTB (Physikalisch-Technische Bundesanstalt), JRC (the European Commission's Joint Research Centre), INRIM, RISE, LNE-SYRTE, VSL, EANTC (European Advanced Networking Test Center) and IIS (Fraunhofer Institute for Integrated Circuits). AGPL means the GNU Affero General Public License; API means application programming interface.

Every draft spells out its abbreviations, because the recipient may not share our
vocabulary. Replace every `[bracketed]` item before sending. Each figure a draft quotes
is checked in the "Facts used" list at the end.

---

## 1. National metrology institute: data and validation (template)

For NPL, VSL, PTB, INRIM, RISE and LNE-SYRTE. Tailored versions for NPL and Metrosert
follow.

**Subject:** Open, reproducible holdover predictions: would you test them against measured clocks?

Dear [Time and Frequency team],

I lead Ashforde OÜ, a small Estonian company that builds Kshana, an open-source engine
for the resilience of positioning, navigation and timing (PNT). One thing it does is
predict how long a clock stays inside a time-error budget after the Global Navigation
Satellite System (GNSS) signal is lost ("holdover"), with an uncertainty band, and
check the result against the maximum time interval error (MTIE) masks that telecom
timing specifications use.

The engine is published under the GNU Affero General Public License on GitHub
(github.com/ashfordeOU/kshana) and on the Rust, Python and JavaScript package
registries. Every result is reproducible from its input file, random seed and engine
version, and each figure is labelled either VALIDATED (it reproduces a published
reference) or MODELLED (it does not yet). The estimators underneath our holdover
predictions reproduce published references, but a composed prediction for a real site
is still MODELLED: it has not been compared with a real clock's holdover. I would like
to change that honestly.

Would your laboratory consider one of these?

1. Sharing, or pointing us to, a published data set of a clock's time error during a
   real loss of reference, so we can compare our prediction against it and publish
   the comparison, whatever it shows.
2. A short technical review of the holdover method by one of your staff.

The related method is set out in an arXiv preprint, *A Conditional Timing Protection
Level: Holdover-Limited Undetected Time Error Under GNSS Spoofing* (arXiv:2606.24210).
It is a preprint and has not been peer-reviewed.

I am not selling anything in this email. If a comparison is useful to you, I would be
glad to do the work on our side and share every input and result.

Kind regards,
[Name]
Ashforde OÜ · Tallinn, Estonia · contact@ashforde.org · ashforde.org

---

## 2. NPL (United Kingdom), tailored

**To:** the route in `TIMING-PARTNERS.md` row 1 (products and services enquiries).
**Subject:** Holdover sizing for critical infrastructure: an open method, and your TQE 32 review

Dear NPL Time and Frequency team,

Your report *Holdover Atomic Clock Landscape Review* (NPL Report TQE 32, July 2024)
sets out why holdover clocks matter for critical national infrastructure when the
Global Navigation Satellite System (GNSS) is disrupted. We build an open-source engine,
Kshana, that tries to answer the operator's side of that question before they buy a
clock: for a given site and oscillator, how long does time stay inside the budget, with
what uncertainty, and does it pass the maximum time interval error (MTIE) mask their
specification names?

The engine is open (GNU Affero General Public License, github.com/ashfordeOU/kshana),
every result is reproducible from input file, seed and version, and each figure is
labelled VALIDATED or MODELLED. The estimators reproduce published references; a
composed holdover prediction for a real site is still MODELLED, because we have not yet
compared one with a real clock's holdover.

Two questions:

1. Is there a published NPL data set of time error during holdover (for example from
   NPLTime service nodes or a test campaign) that we could compare against, with the
   comparison published whatever it shows?
2. Would someone in the department be willing to spend an hour reviewing the method?
   The related protection-level method is in arXiv:2606.24210 (a preprint, not
   peer-reviewed).

We are a small Estonian company and have no customer for this yet, so this is an
honest request for scrutiny, not a sales approach.

Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## 3. Metrosert (Estonia), tailored

**To:** <https://metrosert.ee/en/contact/>
**Subject:** Time and frequency standard: an Estonian open-source timing tool, and a possible collaboration

Dear Metrosert team,

I read that Metrosert is developing Estonia's national time and frequency measurement
standard to support companies that depend on accurate time. Ashforde OÜ is a Tallinn
company that builds Kshana, an open-source engine for the resilience of positioning,
navigation and timing. Among other things it predicts how long a clock keeps correct
time when the Global Navigation Satellite System (GNSS) signal is lost, and checks the
result against telecom timing masks.

As the new standard takes shape, would it be useful to talk about:

1. using the open engine to size holdover for the clocks you are evaluating (we would
   do the work and publish the method and inputs); and
2. in time, comparing our predictions with your measurements, so that our results can
   be labelled as validated against an Estonian reference rather than only modelled?

The engine is published under the GNU Affero General Public License at
github.com/ashfordeOU/kshana. I would be glad to meet in Tallinn or online.

Lugupidamisega / Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## 4. Timing consultancy or test house: channel (template)

For Chronos Technology, and later a test house such as EANTC or Fraunhofer IIS.
**Send only after** the export-control determination and pricing exist; until then use
the shorter "introduction" version below.

**Subject (introduction version):** Predicting holdover before hardware is bought: open tool, looking for a timing partner

Dear [team],

Your team measures and installs resilient timing for critical infrastructure. We build
the complementary piece: Kshana, an open-source engine that predicts, before anything
is bought, how long a site's clock will hold time after losing the Global Navigation
Satellite System (GNSS) signal, and whether it will pass the maximum time interval
error (MTIE) mask the operator's specification names. It does not measure hardware
and does not replace measurement; it helps decide what to measure and what to buy.

Everything is reproducible from the input file, seed and engine version, and each
figure is labelled VALIDATED or MODELLED. The engine is open (GNU Affero General Public
License, github.com/ashfordeOU/kshana).

We are looking for one timing partner who sees operators' holdover questions first
hand, to test whether a short, reproducible sizing report would be useful to them
alongside your measurement work. Would a 30-minute call be worthwhile?

Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## 5. European Commission Joint Research Centre: standards and evaluation

**To:** the Complementary PNT route in `TIMING-PARTNERS.md` row 6.
**Subject:** An open, reproducible scoring engine for timing resilience: relevant to your complementary PNT assessment?

Dear colleagues,

Your Complementary Positioning, Navigation and Timing (C-PNT) activity researches, tests
and assesses technologies for a resilient European system. Ashforde OÜ, an Estonian
company, maintains Kshana, an open-source engine that turns a resilience scenario
(threat, clock or sensor suite, budget) into figures of merit, such as holdover time
inside a time-error budget, each labelled VALIDATED or MODELLED and reproducible from
its inputs.

If an open, inspectable method for comparing timing-resilience options would help your
assessments, we would welcome guidance on what evidence you would need to take it
seriously, and whether any open-access route at the JRC would let us validate it
against measured data.

The engine is published under the GNU Affero General Public License at
github.com/ashfordeOU/kshana. The related timing method is in arXiv:2606.24210, a
preprint that has not been peer-reviewed.

Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## 6. Test-equipment vendor: log formats (Calnex)

**To:** <https://calnexsol.com/contact-us/>
**Subject:** Reading exported time-error logs in an open-source holdover tool

Dear Calnex team,

Many timing engineers export time interval error (TIE) and phase measurements from your
instruments. We maintain Kshana, an open-source engine that predicts holdover and
checks maximum time interval error (MTIE) masks, and we want it to read those exports
correctly rather than guess at the format.

Is there public documentation of the export formats, or sample files we may use for
testing? We would credit the source and keep any sample data out of public
repositories unless you say otherwise.

Kind regards,
[Name]
Ashforde OÜ · contact@ashforde.org

---

## Facts used in these drafts (checked 2026-09-25)

| Claim | Source |
|---|---|
| The engine is public on GitHub under the AGPL | GitHub API, `repos/ashfordeOU/kshana`: not private, licence AGPL-3.0 |
| It is on the Rust, Python and JavaScript registries | crates.io `kshana` 0.27.2; Python Package Index `kshana` 0.27.2; npm `kshana` 0.27.2 |
| Figures are labelled VALIDATED or MODELLED; results reproduce from input, seed and version | public engine documentation and `commercial/export-control/CLASSIFICATION-REQUEST-PACK.md` section 2 |
| arXiv:2606.24210 title | arXiv listing <https://arxiv.org/abs/2606.24210> |
| NPL Report TQE 32, title and date | <https://eprintspublications.npl.co.uk/9987/1/TQE32.pdf> |
| Metrosert is developing a national time and frequency standard | <https://metrosert.ee/en/time-and-frequency_national/> |
| JRC C-PNT "researches, tests and assesses" | <https://joint-research-centre.ec.europa.eu/projects-and-activities/complementary-position-navigation-and-timing_en> |
| "The estimators reproduce published references; a composed prediction is MODELLED" | Kshana Pro `src/twin/holdover_uq.rs` documentation: the MTIE estimator is the open core's validated one, the composition is labelled Modelled, and "no flight/in-orbit-validated holdover is claimed". **Re-check against the engine's validation matrix before sending.** |
