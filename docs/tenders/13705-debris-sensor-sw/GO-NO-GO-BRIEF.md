<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
-->

# AO/1-13705 · S3-SD-08 Enhancing instrumentation capabilities: go/no-go brief

**Recommendation: NO-GO as prime for the 1 Dec 2026 deadline.** Only exception: an eligible sensor
operator or prime that is already bidding asks Ashforde in for a small, defined software task by
**Tue 24 Nov**. Otherwise, download the documents for learning, and prepare the ground-tracking
OD gap in Arksa for a re-issue or follow-on.

## Facts (esa-star snapshot 15 Nov 2026, tender 195614)

- **Tender:** 1-13705, Open Competition (EXPRO+), ESOC, initiating service OPS-S. Programme S2P P3 Space Debris (E/0719-03).
- **Price range:** 200–500 k€. ECOS required.
- **Dates:**
  - **Closes Tue 1 Dec 2026, 13:00** (12 days from today).
  - **Clarifications by Tue 24 Nov, 13:00.**
  - A clarification was published (last update 4 Nov).
- **Scope (esa-star text):** "operational software to support the integration of new European
  instrumentation for observing space debris in key areas, i.e. orbit determination, scheduling, and track
  correlation, especially also for small-sized space debris capabilities."
- **Eligible:** CZ, NO, CH, CA, SE, PT, SK, IT, FI, GR, **EE**, **DE**, PL. **Ashforde OÜ is eligible now.**
- **Weights:**
  - **Background and experience 30%** (heavy);
  - understanding and programme of work 55% (the snapshot mangled the exact figure);
  - time dedication, costing and admin 15%.

## Fit with Arksa / arksa-kpro (honest)

| Need | What exists | Status |
|---|---|---|
| Orbit determination from **radar/optical tracks** (range, range-rate, angles; CCSDS TDM) | Arksa engine EKF/UKF with smoothers (checked against GMAT, engine-only and not wired to UI/API); GNSS OD 10.5 cm vs IGS (scratch runs, not in the validation suite). **No ground-tracking measurement models and no TDM reader** (Arksa gap audit, 15 Oct 2026, main 2e8a817a; re-check on current main) | **To build**: the core of the tender |
| Track correlation / association (tracklet-to-object, small debris) | Not found in Arksa or arksa-kpro | **To build** |
| Sensor scheduling / tasking | Arksa Flows has time and event triggers (operations automation), not sensor tasking. No observation planner found | **To build** |
| Catalogue / CDM ingest, conjunction screening | CDM ingest, catalogue ingest, conjunction board; SOCRATES conjunctions matched to the ms (Iridium 33 × Cosmos 2251 replay) | Adjacent (supports, but not asked) |
| Validation culture, evidence records | 2,951 validation cases: 2,911 pass, 40 recorded misses, 0 failures (Arksa main, results feed 12 Nov 2026) | A differentiator in the write-up, not a substitute for OD heritage |
| arksa-kpro (Arksa-exclusive Kshana Pro methods) | Lunar, spectrum, MBSE, mission dossier; **nothing for debris sensors** | No fit |
| Team background (30% weight) | The founder co-authored the SpaceOps 2023 paper on the data systems of the ESA Space Safety programme (as RHEA at ESOC). No debris-sensor OD delivery heritage; no ESA contract as prime | Weak |

## What a sensor partner must bring

- **An eligible operator** of radar or optical debris sensors (or a builder of new European sensors)
  in one of the listed countries. Its instruments are presumably what the software must integrate
  (inference from the scope; confirm in the SoW and `AD_RD.zip`).
- **Real measurement data** and its formats, plus the sensor models and noise characteristics.
- **OD/correlation heritage**: an existing OD or correlation chain, or staff who have operated one, to
  cover the 30% background criterion.
- **Key persons**, and ideally the **prime role**, with Ashforde as software subcontractor for a bounded
  piece (for example data ingest and the validation/evidence harness, or a scheduling module). That only
  works if Ashforde's piece is something already partly built.

## Realistic chance

- **Low** (inference: well under 10% as prime). The reasons:
  - 12 days left;
  - the three core functions (track OD, correlation, scheduling) are not built in Arksa;
  - no partner identified;
  - heavy weight on background;
  - ESOC Space Safety incumbents are likely to bid.
- As a subcontractor: possible only if invited by a bidder this week.

## Conflict check (must answer even for a subcontract)

FACTS.md lists the founder as Space Systems and DevOps Engineer at ESA/ESOC from 2021, with Space Safety
programme (S2P) data-systems work. **This tender comes from ESOC OPS-S, the Space Safety programme.** If the
founder still works at or for ESA/ESOC, especially in S2P, bidding raises conflict-of-interest and
"Non Benefits" (EXPRO/TC §1.8) questions. **(Unverified: founder to confirm before any involvement.)**

## Founder actions

| Date | Action |
|---|---|
| **Fri 20 Nov** | Decide: no-go (recommended) or partner-only. Confirm the ESA/ESOC conflict position |
| Fri 20 Nov (optional) | Download for learning and a future re-issue: `ews-1-13705.pdf` (SoW), `etc-1-13705.pdf`, `ecc-1-13705.pdf`, `ept-1-13705.docx`, **`AD_RD.zip`** (applicable/reference documents, 4.4 MB), `eli-1-13705.pdf`, plus the clarification published around 4 Nov |
| **Tue 24 Nov 13:00** | Last clarification date. Only relevant if a bidder invites Ashforde |
| Q1 2027 | Put ground-tracking OD (range/Doppler/angles, TDM reader) on the Arksa roadmap if the SoW confirms demand. It also closes the FreeFlyer/Meridian gap OD-1 |

Sources: esa-star snapshot 2026-11-15 (summary 195614, `document_manifests/195614.json`, `tenders.csv`);
vcprep research report 10 (fit 4, "prime possible with sensor partner"); `research/competitors/GAP_AUDIT_FREEFLYER_MERIDIAN.md`;
FACTS.md; arksa-kpro README (eb899c0).
