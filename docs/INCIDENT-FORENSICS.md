# Incident forensics (roadmap 0.42 E1)

PRIVATE. This page describes a Pro method and belongs to this private repository only.

Lead paid product (owner decision K8, 15 November 2026: E1 first, then E3). Price list
(ROADMAP-0.42 section 1c, private sales sheet): evidence pack, incident forensics, one-time per
incident, **$4,900 standard; $14,900 claim-grade with expert review**. Pre-revenue: no customer
has bought or used it.

## 1. What it does

Input: an incident file (`kind = "pro-incident-forensics"`): a case id, a UTC window, an
optional area box, the subject (vessel, aircraft, Station or area) and its identifier in the
AIS or ADS-B data, and the data files:

| Input | Read with | Evidence class of the data |
|---|---|---|
| `[[receiver]]` logs (subject, Station or other; static or vessel) | the open engine's receiver-trust readers (UBX, RINEX + nav, Android, SBF, NMEA, RTKLIB, NovAtel, RTCM 3, GNSS-SDR) | MEASURED |
| `[[ais]]` | the open interference-map CSV columns (`timestamp, vessel_id, lat, lon[, sog_kn]`) | REPORTED |
| `[[adsb]]` | the open interference-map CSV columns (`timestamp, aircraft_id, lat, lon, alt_baro_ft, nic/nacp`) | REPORTED |
| `[[statement]]` (crew, NAVTEX, NOTAM, authority) | recorded as given | REPORTED |
| `[[custody]]` | chain-of-custody records, with the sender's stated hash | stated |

Output: a signed bundle (`src/forensics/bundle.rs` lists the files): `report.json` (the record),
`report.html`, `report.pdf`, `findings.csv`, `timeline.csv`, `timeline.svg`, `area.geojson`,
`tracks.geojson`, `recipe.toml`, `inputs/` (every input byte for byte), `evidence/<n>-<receiver>/`
(an open-engine evidence pack per receiver log, for the window) and `signature.json`.

The report answers, in six sections: **what happened** (verdict per category and findings),
**when and where** (timeline, onset and end bounds, affected area as modelled, tracks), **how
confident** (the points rule, limits), **what it rests on** (input hashes, coverage, checklist),
expert review (claim-grade), **how to reproduce it** (versions, method hash, recipe, command).

## 2. Method `kshana-pro/incident-forensics/v1`

Every parameter is a constant of `detect::MethodParams::V1`, written into each report with its
SHA-256. Changing one means a new method id.

* **Receiver logs.** The open receiver-trust monitors at fixed thresholds (calibration 60 s
  static, 180 s vessel; C/N0 drop 6 dB; 4 satellites lost; 50 m jump; vessel limits from the
  incident or the defaults). Alarm epochs closer than 30 s form an episode; under 10 s is a
  transient. Attribution from the monitor families: jamming-like (C/N0 drop, AGC, jamming
  indicator, loss of lock, solve failure), spoofing-like (C/N0 rise and spread; position jump,
  RAIM, clock, kinematics, heading/course, speed log, sea level, time consistency, OSNMA). A
  family counts when at least 20 % of the episode's epochs carry it. **Breadth discriminator:**
  the share of baseline satellites lost or 6 dB down at the worst epoch; a jammer takes at least
  60 %. A narrow loss whose affected satellites sit 10 degrees or more below the unaffected
  ones is obstruction-like and **not attributed** to interference.
* **AIS.** Per vessel, in the window and area: jumps (at least 500 m, over 50 kn, within
  900 s), reported speed over 50 kn, circular tracks (300 degrees of turning within 3600 s, at
  least 10 reports), and three or more vessels at one position (1e-4 degree, 300 s bins). One
  finding per file and detector (spoofing-like).
* **ADS-B.** Per aircraft: NIC below 7 or NACp below 8 from an aircraft that reported at or
  above them earlier (jamming-like; aircraft never seen above them are counted as unassessed),
  and jumps (5 km, over 900 kn). One finding per file and detector.
* **Confidence.** Up to 4 points: sustained (30 s), two or more indicators of the attributed
  kind (receiver; for tracks, two or more vessels or aircraft), the discriminator (receiver;
  for tracks, three or more reports), and corroboration by a compatible finding of another
  source within 900 s. High = 4 (so never from one source), medium = 2 or 3, low otherwise.
  Not probabilities.
* **Times.** Onset between the last clean and the first alarmed epoch (or report); end between
  the last alarmed and the first clean one.
* **Area (MODELLED).** The extent of the affected observers at their last clean position, plus
  5 km, with the unaffected observers inside counted. No emitter location.

## 3. Variants and the checklist

| Item | Standard | Claim-grade |
|---|---|---|
| C1 every input hashed | required | required |
| C2 receiver logs carry UTC | required | required |
| C3 every input covers 90 % of the window | shown | required |
| C4 receiver baselines end before the window | shown | required |
| C5 chain of custody for every input | shown | required |
| C6 sender-stated hashes match | required | required |
| C7 two independent sources with data | shown | required |
| C8 every attributed finding corroborated | shown | shown: uncorroborated findings stay, labelled indicative |
| C9 open evidence pack per receiver log | shown | required |
| C10 expert review signed | n/a | pending until the reviewer signs |

Status: `final` (or `final-with-checklist-failures`), `awaiting-expert-review`,
`claim-grade-checklist-not-met` (the CLI exits 3; the bundle is still written and says so).

**Expert review.** The claim-grade report carries the review fields empty. The reviewer runs
`kshana-pro forensics review <bundle> --reviewer ... --qualification ... --organisation ...
--independence ... --decision concur|concur-with-reservations|do-not-concur --statement-file ...
--key-file <reviewer key>`: it writes `review/expert-review.json`, signed with the reviewer's
Ed25519 key over the SHA-256 of `report.json`. The issuer then re-signs the bundle
(`kshana-pro dossier sign`). No reviewer is named or contracted yet; nothing fills in a reviewer.

## 4. Verification (free)

`kshana-pro forensics verify <bundle> --trust <issuer key> [--reviewer-trust <key>]`, under any
licence or none: the bundle signature and every file hash, every input against the hash the
report states, each evidence pack under the issuer key (the open `kshana evidence verify` checks
them too), and the review's signature and report hash. `kshana-pro dossier verify` and
`scripts/verify-dossier-signature.py` check the bundle signature alone.

**Reproduce:** from the bundle folder, `kshana-pro forensics run recipe.toml --out <new> --utc
<generated_utc> --key-file <issuer key> --signer <issuer>` with the same versions gives
byte-identical `report.json`, CSV, GeoJSON, HTML and evidence-pack data files. The PDF differs
only in its trailer `/ID`.

## 5. Delivery and licence

* `pro-incident-forensics` is listed at `pro` in the signed tier manifest
  (`license::tiers::PACK_DELIVERED_KINDS`) and has the Pro licence tier.
* It registers only when a verified Pro pack that provides it is loaded:
  `forensics::ForensicsActivator` checks the payload's `forensics/method.json` against this
  build's method id and parameter hash. Release pipeline: `kshana-pro forensics pack-payload
  payload/` then `kshana-pro packs build --from payload/ --pack-id pro-forensics --provides
  pro-incident-forensics ...`.
* Desktop Pro loads installed packs at start (`bridge::load_packs`); the Pro home's "Investigate
  an incident" job is ready when the kind is registered and the licence runs it, locked without
  a licence. The CLI loads them with `--app-data <dir>`; `--dev` skips pack and licence in debug
  builds only.
* In this build the method code is compiled into kshana-pro and activated by the pack; shipping
  it as native code inside the pack waits for the host's native loader (docs/PRO-PACKS.md,
  section 9).

## 6. Tests

`tests/incident_forensics.rs`: the five open training scenarios (jamming, drag-off, time spoof,
combined attributed with onset bounds; the fjord's natural degradation not attributed); a
coherent synthetic incident with two receivers, AIS and ADS-B (corroborated, high confidence,
evidence classes, area, tracks, timeline); the open synthetic AIS/ADS-B sample (circles,
implausible speed, integrity drop, unassessed aircraft); bundle signing, free verification,
reproduction from the recipe, tampering; claim-grade checklist and review (refusals, re-sign,
reviewer pinning, review moved to another report); bad incidents; the command line.
`tests/forensics_pack.rs`: listed before loading, refused until a verified pack loads, method
mismatch and missing kind refused, licence tier gate after loading.

## 7. Gaps against the roadmap bar

* **V2 not met** (scored once on 15 Nov 2026, `docs/evidence/v2-forensics-detection/`). On the
  pre-registered synthetic labelled set ten of eleven bars pass; B3 (cross-attribution) misses:
  4 of 12 moderate-jamming cases are also indicated as spoofing. The public-event part was empty
  and is not scored. E1 stays unclaimed; every report keeps its V2 limit.
* **No public sample on a public event yet.** Needs openly licensed data of a documented
  public interference event (receiver logs or ADS-B/AIS); JammerTest only through the open
  engine's existing download path, never committed.
* AIS on-land detection (the open map's land mask) and emitter localisation are not used.
* Claim-grade has no contracted reviewer.

## Maritime incident evidence pack: actor vs victim (W1119 C3)

`kshana-pro forensics maritime <request.toml> --out <dir> --signer <name> --key-file <file>`
(licence: `pro-incident-forensics`) answers the insurer's question after a GNSS incident at
sea: was the vessel's GNSS attacked, or was its AIS report manipulated?

Inputs (`request.toml`): `title`, `vessel_id` (its id in the AIS CSV), `nmea` (the vessel's own
NMEA 0183 log or VDR export: GGA, RMC, GSV, GSA, and HDT/VHW where present), optional `ais`
(CSV `timestamp,vessel_id,lat,lon[,sog_kn]` of the vessel and its neighbours), optional
`station_events` (a JSON array of unified interference events, `kshana-pnt-interference-event/1`)
and optional `[params]` (every threshold of method `kshana-pro/maritime-actor-victim/v1`).

The method finds episodes in the vessel's own NMEA: **jamming** (C/N0 drop against the
calibration window, loss of fix), **spoofing** (position jump above the vessel speed limit, GNSS
SOG/COG departing from the log and gyro, C/N0 raised and uniform across satellites, false
positions clustering with the neighbours') or a **ship-side fault** (no fix while the signals
are normal). It then compares the vessel's AIS broadcast with its own NMEA, and asks whether
AIS neighbours or a Station saw the same anomaly. The conclusion is one of: no event, victim
of external interference, localised to the vessel, AIS manipulation, ship-side fault, or
inconclusive.

The signed directory holds `evidence-pack.json` (findings labelled MEASURED / DERIVED /
REPORTED, episodes with onset and end bounds, the verdict, uncertainty and limits),
`events.json` and `events.geojson` (unified events), `timeline.csv`, `inputs/` and
`signature.json`. `kshana-pro forensics maritime-synth <case> <dir>` writes a synthetic
scenario of each class to try it. It is an evidence pack, not a legal opinion.

Evidence: **MODELLED** (synthetic scenarios with known truth, `tests/maritime_evidence.rs`);
not validated on a labelled set of real incidents.
