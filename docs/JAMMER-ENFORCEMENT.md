# Jammer enforcement evidence pack

Research 2026-11-20 gaps, backlog B0605. Module `src/enforcement.rs`, command
`kshana-pro enforcement` (`src/cli/enforcement_cmd.rs`), tests `tests/enforcement.rs`.

Reuses the sealed capture and chain of custody module (`src/custody`, B0604): a capture directory
named in the request is verified and its chain exported into the pack unchanged.

`enforcement pack <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-jammer-enforcement-pack`, Pro) reads a victim-effect CSV
(`victim_id,kind,start_utc,end_utc,lat,lon,effect,source[,cn0_drop_db]`; `effect` one of
`loss_of_fix`, `degraded`, `cn0_drop`, `integrity_alert`, `service_outage`, or `none` for a
receiver present and not affected), the device-off or surrender time the authority records, and
optionally a custody directory with trusted keys. It writes a signed pack with:

* **timeline** of every victim effect (REPORTED or MEASURED as stated), the device-off time and
  the custody events;
* **effect on victims**: episodes per victim, affected seconds, effect kinds, the largest C/N0
  drop, who reported it, the most victims affected at once, and whether each victim's effects
  ended within `cessation_s` of the device-off time;
* **affected area**: `cell_deg` cells holding a victim position while affected, beside cells of
  receivers present and not affected. **No emitter position, bearing or direction-finding result
  is taken, computed or output**; an input column that looks like one is refused. Locating a device
  is the authority's matter;
* **custody chain** verification and export;
* **method statement for experts**: inputs and hashes, rules, labels, cessation rule, scope,
  reproduction;
* **checklist** derived from published cases, each item cited, with a status.

## Checklist and sources

| id | item | supplied by the pack | cited |
|---|---|---|---|
| C1 | complaint or report from the affected system's operator, with times | victim rows with source and times | FCC 13-106 para 3 |
| C2 | effect on the victim system documented | per-victim effects and durations | FCC 13-106 paras 8, 10; Nantes 2017 |
| C3 | repeated or continuing interference over time | timeline, episodes, concurrency | FCC 13-106 para 8 |
| C4 | band of the emissions recorded | sealed capture metadata and custody | FCC 13-106 para 4, n.7 |
| C5 | interference ceased when the device was switched off or surrendered | cessation check | FCC 13-106 para 4 |
| C6 | records kept intact from collection to case file | custody verification, input hashes, signature | FCC 13-106 para 4 |
| C7 | duration the device was left operating | first and last effect | FCC 13-106 para 12; Nantes 2017 |
| C8 | device identified and linked to the person; statements | for the authority | FCC 13-106 para 4 |
| C9 | device tested by a laboratory | for the authority | FCC 14-92 para 8 |
| C10 | legal basis and sanction, mitigation, ability to pay | for the authority | FCC 13-106 paras 1, 9, 13-14; DA 16-184 |
| C11 | locating the device | for the authority (never in this pack) | FCC 13-106 para 4 |

Sources (read 2026-11-21):

* FCC, *Gary P. Bojczak*, Notice of Apparent Liability for Forfeiture, FCC 13-106 (adopted 1 Aug
  2013, released 2 Aug 2013): vehicle GPS jammer; interference to the Newark Liberty GBAS during
  pre-deployment testing; proposed forfeiture USD 31,875.
* FCC Enforcement Bureau, *Gary P. Bojczak*, Order and Consent Decree, DA 16-184 (2016).
* FCC, *C.T.S. Technology Co., Limited*, Notice of Apparent Liability for Forfeiture, FCC 14-92
  (released 19 Jun 2014): jammer marketing; purchased devices tested by the FCC Office of
  Engineering and Technology.
* Tribunal correctionnel de Nantes: EUR 2,000 fine for a GPS jammer left running in a car at
  Nantes Atlantique airport, flights delayed; reported by The Connexion and summarised by the
  Resilient Navigation and Timing Foundation, 15 Sep 2017.

An NAL is a proposed forfeiture, not a court judgment; the checklist takes from these cases what
evidence they relied on, not their legal outcome. Not a legal opinion: admissibility and the
offence are for the authority and the court. Exercised on a SYNTHETIC case
(`enforcement synth`).
