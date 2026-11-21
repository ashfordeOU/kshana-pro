# Space-operations signed evidence packs

Research 2026-11-20 gaps, B2205. Feature `pro-space-ops-evidence` (Pro). Command
`kshana-pro space-ops`.

Signed evidence records built from an operator's own data and the open engine's orbit tools
(SGP4, TLE parsing, the US Standard Atmosphere 1976). Every output is **MODELLED**. No record
states that an operator or a mission meets a regulation, licence condition or guideline; the
records organise evidence for the operator's own review and for whoever assesses it.

## Commands

| Command | Input | Output |
|---|---|---|
| `space-ops ca <request.toml>` | the operator's policy, the CDMs received, the decision | collision-avoidance decision record |
| `space-ops identify <ephemeris.csv> <candidates.toml>` | the operator's ephemeris (TEME, km) and candidate TLEs | identification evidence |
| `space-ops disposal <request.toml>` | disposal orbit, mass, area, Cd, the plan's deadline and its source | disposal-plan check |
| `space-ops pack <pack.toml>` | paths to the three inputs | operator evidence pack, all records signed together |
| `space-ops synth <dir>` | none | SYNTHETIC inputs for every command (free) |

Signed commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]`. The
directory holds `report.json`, `report.html`, the inputs under `inputs/` and the signature;
`kshana-pro dossier verify` checks it.

## Collision-avoidance decision record

For each conjunction data message: id, creation time, TCA, miss distance, the collision
probability **recomputed** from the message's relative state, both position covariances and the
hard-body radius, beside the value the message reports. The method is the two-dimensional
encounter-plane probability (Foster): the combined covariance projected on the plane normal to
the relative velocity, the Gaussian of the projected miss integrated over the hard-body disk on
a polar grid. It assumes a short, rectilinear encounter; long or low-relative-velocity
encounters need another method, and the record does not say otherwise.

The record checks the decision against the operator's **own stated policy** (probability
threshold, decision lead time, and the document they come from). It lists, for review: an
action other than a manoeuvre at or above the threshold, a manoeuvre below it, a decision later
than the stated lead time, a recomputed probability more than a factor of ten from the reported
one, and a post-manoeuvre probability still at or above the threshold. No threshold is built in.

CDM fields are taken as a TOML subset (`[[cdm]]` tables): relative position and velocity of
object 2 in object 1's RTN frame, lower-triangle RTN position covariances
`[rr, tr, tt, nr, nt, nn]` (m²). Reading CCSDS CDM files directly is not built (to confirm
against CCSDS 508.0 before it is).

## Identification evidence

The operator's ephemeris points against two or more candidate TLEs propagated with the open
SGP4 (WGS-72): RMS and maximum separation per candidate and the ratio of the next-best RMS to
the best. A margin under three is listed as an open point in a pack.

## Disposal-plan check

Lifetime of a near-circular orbit under drag, da/dt = -sqrt(GM a) ρ(a) Cd A/m, with ρ from the
static US Standard Atmosphere 1976, until 120 km or 200 years, against the deadline **the
operator's plan states** (with its source). Solar activity changes real lifetimes by large
factors; this is a screening estimate, labelled as such. No regulatory lifetime figure is
asserted.

## Operator evidence pack

The three records with an index and the open points, signed together. The framework or article
the pack is organised for is taken from the operator's request and shown as "to confirm"
(for example "Art. 15-style operator evidence"); the article's text has not been read for this
build.

## Not built

- CCSDS CDM (KVN/XML) parsing; probability methods for long encounters (Monte Carlo, 3-D).
- Manoeuvre planning. The record documents a decision; it does not propose one.
- Any statement that a pack satisfies a regulation.
