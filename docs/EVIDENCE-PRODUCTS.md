# Evidence products E4 to E7 (roadmap 0.42)

Four Pro products that turn Kshana data into signed evidence. Each writes a report directory and
signs it with the signed-dossier format; `verify` checks it offline with no licence. Every bar is
pre-registered in a dated file before scoring; misses are published.

| Product | Command | Licence | Status of its bars |
|---|---|---|---|
| E4 PNT service verification study (user side) | `kshana-pro verify-service run <study.toml>` | `pro-pnt-verification` (Pro) | Public sample: 3 of 6 bars met (`docs/evidence/e4-pnt-verification/RESULTS.md`) |
| E5 AIS and position integrity scoring | `kshana-pro integrity score <request.toml>` | `pro-ais-integrity` (Pro) | V3 and V3b NOT MET (`docs/evidence/v3-m4-offset-successor/`); classes come from the open M4 v3 decision |
| E6 framework evidence packs | `kshana-pro frameworks pack <id> --evidence <dirs>` | `pro-framework-pack` (Pro) | No detection claim; "mapped to" only |
| E7 insurer scorecard (exposure summary) | `kshana-pro exposure run <request.toml>` | `pro-insurer-scorecard` (Pro) | No new detection; counts the signed evidence |

All take `--out <dir> --signer <name> --key-file <seed file> [--utc <iso>] [--license <token>]`.

## E4: independent verification studies of new PNT services

A study file names the service, its public source, the **pre-registration file and its SHA-256**
(the run refuses a mismatch), and:

- `[geometry]`: LEO shells of the published design, GNSS reference presets, the sweep (latitude
  and longitude steps, duration, step, mask, PDOP threshold) and the UERE values the study
  **assumes** for the derived 95 % horizontal accuracy (`2 × median HDOP × UERE`). Run through the
  open engine's `leo-pvt` polar mode.
- `[integrity]`: Galileo OSNMA pages (open plain page format) and the public key; seeded
  single-bit flips in ADKD 0 bits with the CRC recomputed, each verified by the open verifier.
- `[[bar]]`: `leo-availability`, `leo-median-hdop`, `gnss-availability`, `fused-availability`
  (per latitude range, `ge` or `le`), `osnma-tamper-detection`, `osnma-clean-failures`.

Output: `verification-study.json`, `report.html`, `study.toml`. `kshana-pro verify-service
template` prints an empty study. Sample: `examples/pnt-verification/`, signed result
`report-templates/samples/pnt-verification-public/`.

**Scope:** users on or near the Earth's surface. Lunar studies, the referee engine's space part and
any study that designs, proves, approves or operates a spacecraft are Arksa-exclusive (roadmap
section 1a) and are not in this crate.

## E5: AIS and position integrity scoring

Request: `ais` (CSV `t_s,id,lat,lon`, or an open M4 AIS document), optional `rf` (CSV
`t_s,lat,lon`, or an open M4 RF document), `[[fleet]]` (name, ids) and `[[area]]` (name and a
latitude/longitude box).

- **Classes** per identity: NORMAL, GAP (AIS silent > 600 s), IDENTITY-DUPLICATE (consecutive
  reports imply > 70 kn), POSITION-OFFSET (RF fixes persistently away from the AIS track), from
  the open engine's M4 v3 decision. Without RF fixes only the AIS checks run (`ais-only`).
- **Scores** per group (all, each fleet, each area): identities per class, the integrity score (the
  share NORMAL) with its exact 95 % interval, RF fixes with no matching AIS (areas), and a fleet's
  identities not seen.
- **Offset candidates:** the stage-2 parallel-track test (V3b's committed pair) lists identities
  whose AIS track has a persistent parallel RF track. Its bars were not met (false-alarm rate
  0.072 on synthetic scenes), so a candidate never changes a class; it is a lead for review.

Output: `integrity-score.json`, `report.html`, `vessels.csv`. Limits written into every scoring:
stage 1's RF access prediction is the open M2 survey layer over a 6 h window (another sensor needs
its own access model, not built); fixes carry no identity; classes describe data consistency,
never intent; no real data scored. Sample: `examples/ais-integrity/` (a synthetic subset of the
open M4 generator's development seed 50), signed result `report-templates/samples/ais-integrity-public/`.

## E6: framework evidence packs

`kshana-pro frameworks list` shows the seven templates (`report-templates/frameworks/`): EASA SIB
2022-02R4, NIST IR 8323r1, DHS RPCF, IMO Circular Letter No. 5037 (**draft**: the circular's text
was not read in this work; its duties follow the open IMO rows until it is), MiFID II RTS 25, the
EU Space Act (**proposal**, COM(2025) 335; topic-level paraphrase to check against the adopted
text) and NIS 2. Each duty is paraphrased with its reference, **mapped to** the Kshana evidence
products that speak to it, and states what Kshana does not provide. These extend the open engine's
six framework mappings with the Pro products.

`pack` reads each evidence directory, verifies its signature (and the signer, with `--trust`), and
maps only verified evidence. Per duty: `evidence mapped`, `no evidence supplied` or `not covered by
Kshana`. A pack never says a framework is met. Sample: `framework-pack-nist-public` (the E3, E4,
E5 and timing-audit samples mapped to NIST IR 8323r1).

## E7: insurer scorecard

Request: the insured fleet or route, the period, signed E1 forensic report directories and one
signed E5 scoring with the group to report. The summary counts incidents in the period (jamming and
spoofing indicated, findings meeting the claim-grade bar, time under findings), copies the group's
integrity score, rejects any source whose signature does not verify or whose signer is not
trusted, and is signed. It adds no detection; each source keeps its own validation status. Not an
underwriting recommendation or rating. Sample: `examples/insurer-exposure/request.toml`, signed
result `report-templates/samples/insurer-exposure-public/summary/` (all inputs synthetic).

## Evidence rows

- **V3** (`docs/evidence/v3-m4-offset-successor/`): the M4 position-offset bar re-run as a new
  pre-registration with a new method. V3 missed (offset recall 0.861 to 0.879, FA 0.132); V3b,
  pre-registered before its code, missed (0.829 to 0.896, FA 0.072). Both published. The open
  engine's v1 to v3 misses stay published.
- **V6** (`docs/evidence/v6-spoofing-and-area-trust/2026-11-15_DECISION.md`): E5 v3 spoofing not
  scored (no blind data; a method is written down for a future blind set); B5 v4 not registered
  (no method can meet T2 as defined).

## Equipment-resilience rating and voyage risk score (W1119 C7)

Two signed outputs for underwriters (licence: `pro-insurer-scorecard`):

* `kshana-pro exposure equipment <equipment.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>]`
  rates a vessel's PNT equipment 0–100 (bands A ≥ 80, B ≥ 60, C ≥ 40, D) from, per device
  (`[[devices]]` with `name`, `role` = `primary_gnss` / `secondary_gnss` / `eloran` / `rmode` /
  `inertial` / `timing`): a signed receiver scorecard directory (`scorecard`), an OSNMA
  scorecard (`osnma_scorecard`, W1119 C6), a PICS scorecard if present (`pics`), or, without
  test evidence, declared capabilities (`[devices.declared]`), capped at 50. A scorecard whose
  signature fails, or whose signer is not among the `--trust` keys, counts for nothing. The
  vessel rating is 0.6 × primary GNSS + 0.4 × the best independent alternative.
* `kshana-pro exposure voyage <voyage.toml> --out <dir> ...` scores a voyage 0–100 (low,
  moderate, elevated, high): `waypoints`, `departure_utc`, `events` (unified event files with
  the interference history; area-console history converts with `ievents::from_area_event`),
  `corridor_km` (default 25) and `equipment` (an equipment request or an
  `equipment-rating.json`). Exposure E = 1 − exp(−Σwᵢ/5), with wᵢ = kind weight × confidence ×
  exp(−age/180 d) × the share of the route within the event's reach; risk = 100 × E × (0.2 +
  0.8 × (1 − rating/100)). The output lists the contributing events and legs.

The method text is written into every output. A rating of the evidence supplied, not an
underwriting recommendation, premium advice, a forecast or a certification.

Evidence: **MODELLED** (weights are stated engineering choices, tested for bounded and
monotonic behaviour in `tests/insurer_voyage.rs`; not fitted to claims or loss data).

## OSNMA status block in every signed pack (B1404)

Every Pro pack is signed with the signed-dossier format (`src/signed_dossier.rs`), so the block
lives there and every pack can carry it: scorecards, forensics, timing, frameworks, exposure,
studies and plain dossiers.

* `kshana-pro dossier osnma-status <pages|stream.ubx> --out <block.json> [--merkle-root <hex>]
  [--public-key <id:type:hex>[,...]] [--reference-time <gst-s>]` runs the open OSNMA verifier
  this build embeds and writes the block (free).
* Every signing command takes `--osnma-status <block.json>` (handled once for all commands, like
  `--sample`). The signature is then `kshana-dossier-signature/2` and its signed statement holds
  the block. `kshana-pro dossier verify` and `scripts/verify-dossier-signature.py` print it.

The block has six fields and nothing more:

| Field | Meaning |
|---|---|
| `status` | `authenticated`, `failed` or `unavailable` (the open verifier's overall status); `not_recorded` for a migrated older pack |
| `key_id` | public key id of the last DSM-KROOT that verified, or null |
| `chain_id` | chain id of that KROOT, or null |
| `merkle_root` | the Merkle root (SHA-256, hex) the verifier was given, or null |
| `verifier` | `kshana <version>`, the open verifier that produced the status |
| `freshness_checked` | whether freshness was enforced against an independent reference time |

**Scope.** The block records the open verifier's status of the navigation data and the trust
anchors it used. It is not a time authentication or a position authentication, and it holds no
comparison of receiver time with any trusted time. With `freshness_checked: false` the status
reads "authentic, not necessarily fresh". The open verifier's advisory applies: not type-approved
navigation equipment, not a navigation integrity service. Evidence class: MODELLED for the
synthetic samples. Nothing is validated against a live receiver here.

**Compatibility and migration.**

* A signature without a block stays format 1, byte for byte as before.
* Format-1 packs, including every committed sample, verify unchanged and read as "OSNMA not
  recorded".
* `kshana-pro dossier migrate <dir> --trust <old signer key> --signer <name> --key-file <file>
  [--osnma-status <block.json>]` re-signs a format-1 pack as format 2 (Pro). The block defaults to
  `not_recorded`, and the SHA-256 of the replaced signature file is kept in `supersedes`.
* A pack is migrated only if its old signature verifies against the pinned key and no file
  changed.
* Verification refuses a format-2 statement without a block, a format-1 statement with one, a
  block with any other field, and any change to the block (it is signed).

Tests: `tests/osnma_evidence.rs`.
