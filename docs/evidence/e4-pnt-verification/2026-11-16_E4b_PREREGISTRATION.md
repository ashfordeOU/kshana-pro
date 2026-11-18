# E4b pre-registration: the public sample study, admissible-design geometry and session-long tamper evidence (roadmap 0.42 E4)

**Status: APPROVED by the owner, 16 Nov 2026.** (Was PROPOSED; approval recorded in `docs/evidence/BARS-TO-GREEN-0.42.md` before any new evaluation data were generated or read.) Written on 2026-11-15 (22:00 UTC) and committed alone, before any code of
the method below exists and before the data of section 3 have been downloaded or opened for this
study. It takes effect on the owner's approval (dated 2026-11-16). The E4 sample result
(`RESULTS.md`: S1, S2 and S5a NOT MET) stays published whatever happens here. A bar may be
tightened, never loosened. Results go in `E4b_RESULTS.md` next to this file, met or missed.

Scope as E4 section 0: the ground and user-side generic part only.

## 0. What was seen first (disclosure)

* E4's results, its signed sample (`report-templates/samples/pnt-verification-public/`,
  including the list of the 64 undetected flips), `src/pnt_study/mod.rs`, and the open engine's
  OSNMA verifier and LEO presets (kshana 0.41.0) were read.
* The diagnosis below was computed on 2026-11-15 from that published list and the published
  sample stream only.
* The Galileo OSNMA test vectors (section 3) are known to the open engine: its verifier reads
  their CSV layout and was developed against them, so they are **not blind for the verifier's
  behaviour on clean data**. No tamper campaign has been run on them by any Kshana code (no such
  test exists in either repository as far as this one shows; unverified beyond that). The flip
  seed below has never been used.

## 1. Diagnosis of the misses (from the published results only)

### S1 and S2 (LEO-alone availability >= 0.99): a property of the service design, as modelled

S1: worst 0.969 at 10° (0.972 at 0°). S2: 0.960 at 70°, 0.599 at 80°, 0.000 at 89.9° (median PDOP
6.12 with 8 satellites in view). Where satellites are in view the geometry is good (S3 met:
median HDOP <= 1.96). The misses are samples where LEO-alone PDOP exceeds 6. Near the pole only
the 97° shell is visible (the 53° shell's coverage edge at 1 080 km and a 10° mask is about 75°
latitude), and its satellites appear at similar elevations, which inflates VDOP (3.8 to 5.8).

Two modelling choices the source does not state were fixed by the E4 study without saying it
was a choice: the split of the 258 satellites (E4: 192 at 53° and 66 at 97°, from the open
end-to-end scenario; the open engine's Pulsar preset uses 168 and 90) and the Walker pattern of
the polar shell (the study file has no pattern field, so the open engine's default, delta,
applies; the preset uses star for the 97° shell) (unverified: from reading
`src/pnt_study/mod.rs` and the preset, not from a run). The verdict may depend on them.

**No method change can make a published design more available.** A Kshana method that turned
S1 or S2 green would be measuring something other than the service. What a method can do is
measure the design honestly across what the source leaves open; that is section 2.1. The
expected outcome for S2 is still a miss.

### S5a (OSNMA tamper detection >= 0.99): 136 of 200

From the published list of undetected flips and the published stream (2 satellites, 24
sub-frames each, 720 s):

* **52 of the 64 misses are flips in the first six sub-frames** (18 to 23 sub-frames before the
  stream end); about 55 of the 200 flips fell there (eligible pages are spread evenly over
  22 sub-frames), so almost every early flip was missed, against 12 of about 145 later ones.
  The study reads each satellite's status **once, at the end of the stream**, and the open
  verifier keeps a satellite `failed` only for its failure memory (default 600 s,
  `failure_memory_s`). Two mechanisms fit (unverified which): (H1) the failure happened more than
  600 s before the end and had expired when the status was read; (H2) the flipped data came
  before the TESLA chain was authenticated and was no longer held when it could be checked.
* **All 12 later misses are flips in bits 6 to 15 of word types 1 to 4: the IODnav field.** The
  open verifier assembles ADKD 0 navigation data only when word types 1 to 4 carry the same
  IODnav (`osnma/navdata.rs`); a flipped IODnav makes the sub-frame's set inconsistent, the data
  are never assembled, the tag is never checked against them, and no failure is raised. The
  ADKD 0 tag covers the IODnav bits, so the bits as received could have been checked.

## 2. The method (fixed now; `kshana-pro/pnt-verification/v2`)

### 2.1 Geometry (S1 to S4)

The open `leo-pvt` polar mode as E4, run on **every admissible configuration** of the published
design: satellite split {192 / 66 (E4), 168 / 90 (open preset)} x polar-shell pattern {delta,
star}; 12 planes at 53° (delta, phasing 1) and 6 at 97° (phasing 1), 1 080 km, in all four.
Grid: latitudes 0° to 90° every 10° (the pole row as the engine gives it), longitudes every 15°,
24 h from 2026-01-01T00:00:00 UTC every 120 s; mask 10°; PDOP threshold 6; GNSS presets as E4.
A bar is met **only if it is met in all four configurations** (the strict reading: the design is
not credited with assumptions its source does not make). Every configuration's numbers are
published.

### 2.2 Integrity (S5)

The user receiver is the open verifier with three policies, all fixed now:

1. **Session-long failure memory.** `failure_memory_s` at least the stream's duration: a failed
   tag keeps the satellite `failed` until the end of the session. Reason: a mismatch is evidence
   of tampering, and a verification study reads the status after the session. S5b (no failed
   satellite on the untampered stream) guards against false failures being remembered.
2. **Hold until the chain is verified.** Navigation data and tags received before the TESLA
   chain is authenticated are held (no depth limit within the session) and checked once it is.
   Reason: the chain authenticates earlier keys by hashing, so early tags are checkable.
3. **Check the bits as received.** For ADKD 0, when word types 1 to 5 are present for a
   sub-frame but their IODnav values differ, the navigation data are assembled from the words as
   received and the tag is checked; a mismatch is a failure. Reason: the tag covers exactly what
   the satellite transmitted, so a genuine data-set change verifies and a flipped IODnav fails.

Implemented either as an open-engine verifier change released before scoring or as a Pro-side
wrapper over the open verifier's tag and key checks; either is committed before the data are
opened. The detection rule, the eligibility rule and the readout (status at the end of the
stream) are E4's, unchanged.

**Development (allowed, published before scoring):** on the E4 sample stream only
(`examples/pnt-verification/pages-good.txt`, published) with E4's seed 4242, to check the code
implements this text; the table, including which of H1 and H2 held, is committed as
`E4b_dev.json` before the test vectors are opened. No policy above may change after this file is
committed.

## 3. Data

* **Geometry:** published design only (Leclère, Marathe and Reid, ION GNSS+ 2025,
  arXiv 2509.19551) and the open presets. No download. MODELLED.
* **Integrity (new download, owner approval needed):** the Galileo OSNMA test vectors published by
  the European GNSS Service Centre, `https://www.gsc-europa.eu/sites/default/files/sites/all/files/Test_vectors.zip`
  (10 319 251 bytes, Last-Modified 2025-09-08, HTTP header read 2026-11-15), with the public key
  and Merkle tree root they ship with. Licence: GSC website terms (redistribution not assumed;
  unverified): scored locally, never committed; only flip outcomes and counts are published. The
  streams used are those the archive's own documentation lists as nominal (expected to
  authenticate); that list is read from the documentation and committed (`E4b_streams.json`)
  before any flip is run. Each selected stream: 200 flips, ChaCha8 seed **20261123**, E4's
  eligibility rule (ADKD 0 bits of word types 1 to 5, the CRC recomputed, a sub-frame followed by
  at least two more of the same satellite).

## 4. Bars (unchanged)

E4 section 2: S1 LEO availability >= 0.99 at every latitude 0° to 60°; S2 >= 0.99 at 70°, 80°, 90°;
S3 median LEO HDOP <= 2.0 at every latitude 0° to 90°; S4 GPS + Galileo + LEO availability >= 0.999
at every latitude; S5 tamper detection >= 0.99 per selected stream and no `failed` satellite on
any untampered stream. The study passes only if S1 to S5 all hold.

## 5. Scoring

```
kshana-pro verify-service run examples/pnt-verification-v2/study.toml --out <dir> --signer <name> --key-file <seed file>
KSHANA_PRO_OSNMA_TV=<local dir> cargo test --release --test evidence_products e4b_scored -- --ignored --nocapture
```

The study file carries this file's SHA-256 (the run refuses a mismatch), as E4's does. Run once.

## 6. Miss and publication

Any bar failing is a miss, published in `E4b_RESULTS.md` with every configuration's and every
stream's numbers. The E4 product claim stays about the study method and template, never that a
service is approved or fit for a use.

## 7. Feasibility (for the owner; not part of the method)

* S5: about 1 to 2 days; 10 MB download. Risk moderate: the IODnav mechanism is clear from the
  published list; the early-flip mechanism (H1 or H2) is settled on development data first. Real
  test-vector streams may have other gaps (for example sub-frames whose tags are never sent).
* **S1 and S2: not reachable by any Kshana method if the published design does not provide the
  availability; very high risk of a repeat miss** (S2 at 80° and the pole in particular). Turning
  them green is not an engineering task. Whether a service-level verdict of a third-party design
  should gate a Kshana release is a scope decision for the owner; this file does not change it.
