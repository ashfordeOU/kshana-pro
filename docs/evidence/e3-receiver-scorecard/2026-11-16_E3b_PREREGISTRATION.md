# E3b pre-registration: the independent-oracle bar E3-B2 (a), ellipsoidal error and arrival-time truth (16 November 2026)

**Status: APPROVED by the owner, 16 Nov 2026.** (Was PROPOSED; approval recorded in `docs/evidence/BARS-TO-GREEN-0.42.md` before any new evaluation data were generated or read.) Written on 2026-11-15 (22:05 UTC) and committed alone, before any code of
scorecard v2 exists and before any case of the held-out campaign below is generated. It takes
effect on the owner's approval (dated 2026-11-16). The E3 sample result (`RESULTS.md`: E3-B2 (a)
MISSED on 5 of 15 events) stays published whatever happens here. A bar may be tightened, never
loosened. Results go in `E3b_RESULTS.md` next to this file, misses included.

## 0. What was seen first (disclosure)

E3's results, the public campaign and its signed sample, `src/scorecards/mod.rs` (position error
by `haversine_m`, a sphere of the open engine's Earth radius; truth aligned by the receiver's own
epoch time) and the open generator's instructor log (`nmea_synth`: `position_error_m` from
`ne_offset_m`, local north and east offsets with the WGS-84 meridian and prime-vertical radii at
the mid latitude, rounded to 0.1 m) were read. The eleven library analogues of the E3 campaign are
development data now. The held-out cases below have not been generated; seeds 31001 to 31016
appear in no campaign or scenario file of this repository (searched 2026-11-15).

## 1. Diagnosis of the miss (from the published results only)

E3-B2 (a) asks that each event's scorecard `max_error_m` equal the generator's largest
`position_error_m` over the event's rows with a valid fix, within 1.0 m. Missed on 5 of 15 events:

1. **Meaconing (`hormuz-meaconing`): 6.4 m against the generator's 136 m.** The re-broadcast
   delays time as well as position. The scorecard takes the truth at the time the receiver
   **states**, which is delayed by the same 20 s, so the delayed position matches the delayed
   truth almost exactly. The generator compares the fix with the truth at the **true** time. Root
   cause: the scorecard's time base is the unit under test's own clock.
2. **Four drag-offs of 0.6 to 8 km: differences up to 0.4 %** (8 022.5 m against 8 006 m;
   1 995.4 m against 2 003.5 m). The scorecard measures on a sphere, the generator on the WGS-84
   radii; the sphere's scale differs from the ellipsoid's by a few tenths of a per cent depending
   on latitude and bearing, which fits both the size and the changing sign of the differences
   (unverified per event). The 1.0 m tolerance was written for metre-scale errors; at kilometres,
   only a computation on the same figure of the Earth can meet it.

## 2. The method (fixed now; `kshana-pro/receiver-scorecard/v2`)

`kshana-pro/receiver-scorecard/v1` unchanged except:

1. **Error on the ellipsoid.** Position error is the WGS-84 geodesic distance between fix and
   truth (Karney, "Algorithms for geodesics", J. Geodesy 87, 2013), replacing the spherical
   haversine. The scorecard does not call the generator's `ne_offset_m`: the oracle stays a
   different route (the generator's instructor log), and agreement is expected to well under
   1 m at these distances because both now use the WGS-84 figure (unverified until run).
2. **Truth at the arrival time.** Each receiver epoch is stamped with its **arrival time** on a
   clock independent of the unit under test, and the truth is taken at that time. For a recorded
   campaign, the capture's host timestamps (a log without them is not scored for errors, and the
   report says so). For an analogue, the time at which the generator emitted that epoch (its
   epoch time `t_s`, the stream position at the stated output rate), not any time written inside
   the NMEA sentences. The receiver's stated time is still read and still scored for the time
   error.
3. The campaign file may override an analogue's generator seed per case (`analogue_seed`), to be
   added to the campaign format; nothing else of the format changes.

**Development (allowed, published before scoring):** the E3 public campaign (eleven cases,
fifteen events) is re-scored with v2 to check the code implements this text; the table is
committed as `E3b_dev.json` before the held-out campaign is generated.

## 3. Data: the held-out campaign (SYNTHETIC; no download)

`examples/scorecards/e3b-heldout-campaign.toml`, committed with this method before its first
scoring, unit under test the open engine's NMEA receiver model (kshana 0.41.0), MODELLED:

* the same eleven library analogues as E3, each with a new generator seed **31001 to 31011** (in
  the campaign file's case order);
* five new analogue cases in the threat library's format, written to these parameters before any
  is generated: drag-offs to **0.5 km, 3 km and 12 km** (seeds 31012 to 31014; the library's
  drag-off ramp and raised C/N0 otherwise), and re-broadcast delays growing to **8 s and 30 s**
  (seeds 31015 and 31016; the library's meaconing timing otherwise).

Campaign bars (`[bars]`) as the E3 campaign file.

## 4. Claim bars (unchanged)

E3 pre-registration, word for word in effect: **E3-B1** two scorings byte-identical, the signed
directory verifies, a one-byte change fails; **E3-B2 (a)** every event's `max_error_m` equals the
generator's largest `position_error_m` over `[onset, end]` rows with a valid fix, within 1.0 m;
**E3-B2 (b)** `misleading_unwarned` equals the generator count within 2 epochs per event;
**E3-B3** over spoofing-class events, the sum of `misleading_unmonitored` at most half the sum of
`misleading_unwarned`. E3b is met only if all hold on the held-out campaign.

## 5. Scoring

```
kshana-pro scorecard run examples/scorecards/e3b-heldout-campaign.toml --out <dir> --signer <name> --key-file <seed file>
cargo test --release --test receiver_scorecard e3b_ -- --nocapture
```

run once; the tests (in the existing `tests/receiver_scorecard.rs`) then pin each bar's outcome.

## 6. Miss and publication

Any event outside a tolerance, or any bar not held, is a miss, published in `E3b_RESULTS.md` with
every event's two numbers. The permitted sentence of E3-B3 is used only if E3-B3 holds again.

## 7. Feasibility (for the owner; not part of the method)

About half a day to a day: one distance function, one time base, a seed override and five
analogue files. No download, small disk. Risk low to moderate: remaining differences could come
from the generator's 0.1 m rounding (well inside 1 m), the logged-row cadence (if the instructor
log skips epochs, the scorecard's maximum over all epochs can exceed the oracle's maximum over
logged rows on a fast ramp) or the mid-latitude approximation at 12 km (expected well under
1 m). This is the fastest bar to turn green honestly.
