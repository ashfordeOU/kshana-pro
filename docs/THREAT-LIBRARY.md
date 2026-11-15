# Threat scenario library (roadmap 0.42, E2)

Twelve curated real interference events as reproducible scenarios, each with its provenance
and licence notes. Code: `src/threat_library/`; files: `threat-library/scenarios/<id>.toml`
(format `kshana-pro-threat-scenario/1`); kind `pro-threat-scenario`; feature
`pro-threat-library`; both at the `pro` edition in the signed tier manifest and the `pro`
licence tier.

## The rule: no source data

No TEXBAT recording, JammerTest file, AIS feed or any other source data is in the library, its
exports or its feed. A scenario cites published descriptions and gives parameters; where the
original data is a dataset (JammerTest, TEXBAT) the `[replay]` table says where the user fetches
it, the file names to expect and where the event times come from (always the publisher's
documentation, never the scenario file). The validator refuses `licence.redistributed = true`, a
dataset scenario without `[replay]`, a source without an address or citation, and the words
compliant, conform(ant) and certified. `feed verify` refuses any file that is not a scenario.

## Edition 2026-11

| id | family | analogue | timing profile |
|---|---|---|---|
| baltic-gulf-of-finland-jamming | baltic | vessel, 30 dB jamming | |
| baltic-tartu-approach-jamming | baltic | static, two episodes | |
| baltic-timing-holdover-under-jamming | baltic | | 6 h denial |
| hormuz-position-displacement | hormuz | vessel, 8 km drag-off | |
| hormuz-meaconing-delay | hormuz | vessel, 20 s re-broadcast delay | |
| red-sea-bab-el-mandeb-jamming | red-sea | vessel, partial jamming | |
| red-sea-jamming-then-drag-off | red-sea | vessel, jamming then 3 km drag-off | |
| jammertest-stepped-power-jamming | jammertest | static, three power steps | |
| jammertest-position-spoofing | jammertest | vessel, 2 km matched drag-off | |
| jammertest-time-spoofing | jammertest | static, UTC +2 s | push 50 ns/s to 10 µs |
| texbat-ds3-matched-power-time-push | texbat | static, UTC +1 s | push 20 ns/s to 2 µs, one receiver |
| texbat-ds4-matched-power-position-push | texbat | static, 600 m matched drag-off | |

Every analogue parameter is **representative** of the published description and says so in
`parameter_basis`; none is read off a recording. Sources are public notices and advisories
(EASA SIB 2022-02 and revisions, Traficom, MARAD MSCI advisories, JMIC/UKMTO notes, an
operator's statement), the JammerTest organisers' published plans and the JammerTest 2024
dataset DOI, and the TEXBAT paper (Humphreys et al., ION GNSS 2012). `verify_before_use` names
what to check against the source.

## Commands

```sh
kshana-pro threats list                         # ids and titles, no licence
kshana-pro threats show <id>                    # provenance, licence notes, replay (Pro)
kshana-pro threats export <id> --out <dir>      # files below (Pro)
kshana-pro run <file with kind = "pro-threat-scenario", scenario = "<id>">
kshana-pro threats feed build --out <dir> --edition 2026-11 --signer <name> --key-file <key> [--previous <feed.json>]
kshana-pro threats feed verify <dir> --trust <publisher key> [--previous <feed.json>]   # free
```

An export holds `scenario.toml`, `PROVENANCE.txt`, `analogue/receiver.nmea` (the analogue
receiver stream from the open `kshana::nmea_synth`), `analogue/instructor.json` (what was
injected when, true track), `analogue/receiver-trust.toml` (runs with the open engine),
`analogue/monitors.json`, `testbench/<id>.*` (the open I4 writers of
`kshana::interop::testbench`: motion CSV and metadata, waypoints, truth NMEA, events CSV and
TOML, conditions) and `timing/profile.csv`.

## Monthly feed

`feed.json` lists the edition, the SHA-256 of the previous edition's `feed.json`, and per
scenario its version, SHA-256 and the edition it last changed in; the directory is signed as a
whole (dossier signature). Verification checks the signature, each hash, each scenario against
the library rules, that nothing but scenario files is present, and the chain to a previous
edition when given.

## Not done here

* Exports to vendor simulator formats (Skydel, SimGEN, R&S, LabSat) beyond the open
  test-bench files: they belong to the open I4 workstream, which the pinned engine (0.41.0)
  does not carry yet.
* An aviation analogue (ADS-B NIC/NACp): the Tartu scenario is a static ground receiver.
* No subscription server: the feed is a signed directory the vendor publishes.
