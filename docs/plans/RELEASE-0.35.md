# Release 0.35.0 plan: "Trusted fix"

Founder decision, 2026-10-09: every item below ships in 0.35.0. Coordinator owns this file.
Each item has a fixed **0.35 scope** (what "done" means for this release) so nothing slips
silently. Anything beyond that scope is listed as follow-on, not dropped.

Context: a maritime competitor launched a hardware receiver with a 0-100 per-fix trust
score. Kshana's answer is software that works with the receiver already installed, is open
source, and states every threshold before a run. Public text names no competitor.

## Standing rules for every workstream

- Public repo (ashfordeOU/kshana) unless marked **pro** (ashfordeOU/kshana-pro).
- Synthetic or openly licensed public data only. No partner recordings or results.
- No anti-jam or anti-spoof performance numbers in public text until counsel answers N6.
- No RF, IQ or waveform synthesis of a jammer or spoofer; nothing transmits. Generated
  NMEA text for training and demos is allowed.
- No names of individuals, companies, competitors, vessels or prospects in the public repo.
- Thresholds pre-registered in config. Never relax a test bar. Never skip, disable or
  quarantine tests. Keep default test runtime small: coverage is already near its cap.
- New crate dependencies need coordinator approval first (licence, cargo-deny, MSRV,
  wasm build).
- CHANGELOG: one subsection per workstream under Unreleased; the coordinator resolves
  merge order. CLI: add a subcommand arm in src/main.rs, nothing else there.
- Commit author `Ashforde OÜ <ashfordegithub@cbaweja.org>`, no AI attribution. Workers
  push their own branch only, after founder authorisation in their session. Only the
  coordinator opens PRs, sets PR bodies and merges. Never push tags.
- Every PR gets a Fable critic review before merge.
- Advisory-software caveat wherever output can reach navigation: not type-approved
  equipment, operator stays responsible.

## Workstreams

| WS | Items | Worker | Branch | Depends on |
|---|---|---|---|---|
| A | Maritime trust core: vessel model, moving-platform monitors, 0-100 score, live + gate modes, synthetic Baltic demo | W9 (running) | feat/maritime-trust | none |
| B | 1 Signal K plugin, 1 OpenCPN integration, 9 reference build | W10 | feat/marine-integrations | A's live JSON and $PKSHT format |
| C | 2 Bridge-simulator training scenarios (NMEA stream generator) | W11 | feat/nmea-training | none; A's demo may adopt it later |
| D | 3 OSNMA verification | W12 | feat/osnma-verify | none |
| E | 4 Public interference map, 10 route risk | W13 | feat/interference-map | data licence review first |
| F | 5 Telemetry exporters, 6 signed evidence packs | W14 | feat/trust-telemetry | A's per-epoch output |
| G | 7 Compliance mapping, 8 simulator test-bench export | W15 | feat/compliance-bench | none |
| H | 11 Grants (NAVISP, EUSPA, DIANA), 12 independent audit (**pro**) | W16 | pro: commercial/grants | none |

### A. Maritime trust core (W9)
0.35 scope: the six deliverables in W9's brief (platform model; NMEA-only moving-platform
monitors plus UBX-SEC-SIG and reported OSNMA status; documented 0-100 score with reasons;
`receiver-trust live` from stdin/file/TCP/UDP with JSON lines, `$PKSHT` and opt-in gate
mode; synthetic Baltic demo pinned by test; docs/MARITIME-TRUST.md and a Studio view).
Follow-on: serial input, IEC 61162-450 (UDP multicast) framing.

### B. Marine integrations (W10)
- **Signal K:** a Node plugin in `integrations/signalk/` that runs or connects to
  `kshana receiver-trust live`, publishes trust score, band and reasons as Signal K paths
  under a documented namespace, and raises a Signal K notification on the untrusted band.
  Published to npm under the existing package scope only with coordinator sign-off.
- **OpenCPN:** 0.35 scope is a documented, tested setup in which OpenCPN reads Kshana's
  gate-mode NMEA over TCP (so OpenCPN sees an invalid fix when trust collapses) and shows
  `$PKSHT` in its NMEA debug and dashboard. A native C++ OpenCPN plugin (score panel) is
  scoped and prototyped; it ships in 0.35 only if it passes review in time, otherwise
  follow-on. Stated plainly in docs either way.
- **Reference build:** `deploy/reference-build/`: parts list (single-board computer plus
  a commodity multi-band receiver, no brand endorsement implied), OS image steps, systemd
  unit, container option, and the Signal K wiring. Document, test the unit file in CI
  with a dry run.

### C. Bridge-simulator training scenarios (W11)
`kshana nmea-scenario`: from a scenario TOML, write or stream (file, TCP, UDP, real time
or accelerated) a full bridge NMEA set (GGA, RMC, VTG, GSV, GSA, GNS, ZDA, HDT, VBW) for a
vessel track, with scripted events: jamming (fix loss, C/N0 fall, satellite loss),
position drag-off spoof, time spoof, meaconing-style replay delay, and recovery. Includes
an instructor event log (what happened when) so a trainer can debrief. A library of
training scenarios in `scenarios/training/`. Output checksum-valid, deterministic per seed.
This writes NMEA text only.

### D. OSNMA verification (W12)
0.35 scope: a pure-Rust OSNMA receiver-side verifier per the published Galileo OSNMA
SIS ICD and Receiver Guidelines: DSM-KROOT verification against the public key (ECDSA
P-256 / P-521), Merkle tree check, TESLA chain key verification, MACK/tag verification
(HMAC-SHA-256 and CMAC-AES), navigation data authentication status per satellite.
Input: I/NAV page bits from u-blox UBX-RXM-SFRBX and a documented plain page format.
Validated against the official public OSNMA test vectors (licence checked first) and
wired into A as an authentication monitor. Crypto crates need approval (RustCrypto family).
Follow-on: public-key renewal and revocation handling beyond what the vectors exercise.

### E. Public interference map and route risk (W13)
First deliverable is a **data licence review** (docs/data/INTERFERENCE-DATA-SOURCES.md):
for each candidate ADS-B and AIS source, its licence, whether commercial use and
redistribution of derived maps is allowed, attribution text. Only sources that pass are
ingested. Then: ADS-B accuracy-field (NIC/NACp) degradation aggregated on a hex grid per
day; AIS anomaly detectors (positions on land, circular patterns, implausible jumps);
`kshana interference-map` producing GeoJSON plus a Studio map page; a regional
route-exposure summary for a polyline route (share of route through degraded cells, per
day). Methods and caveats documented (aircraft accuracy fields drop for reasons other
than interference). Daily publication pipeline is follow-on unless trivial.

### F. Telemetry and evidence packs (W14)
- **Telemetry:** from live mode, a Prometheus `/metrics` endpoint, OpenTelemetry (OTLP)
  export behind a cargo feature, and syslog in CEF/LEEF for SIEMs. Documented metric
  names, a sample Grafana dashboard JSON.
- **Evidence packs:** `kshana receiver-trust evidence`: a self-contained, hash-chained
  bundle for an event window (raw log slice with its SHA-256, config, per-epoch results,
  reasons, engine version, a human-readable PDF/HTML summary), signed with Ed25519 and
  verifiable with `kshana evidence verify`. Optional RFC 3161 timestamp token input.
  States clearly it is a technical record, not a legal opinion.

### G. Compliance mapping and test bench (W15)
- **Compliance mapping:** `docs/compliance/` maps Kshana capabilities and evidence
  outputs to clauses of public frameworks (US PNT Conformance Framework, IMO and EASA
  interference guidance, NIS2 Article 21 risk measures, EN 16803 by clause number only,
  no reproduced text). `kshana compliance-report` fills the mapping for a given set of
  runs. Wording: "supports evidence for", never "certifies" or "complies".
- **Test bench export:** export Kshana scenarios to formats laboratory simulators
  accept: user motion files (CSV trajectory), NMEA motion, and the documented scripting
  interfaces where public. No vendor name in marketing text; format names in docs only
  where needed for interoperability. Method docs for running a receiver against a
  scenario; no published results.

### H. Grants and audit (W16, pro)
In kshana-pro `commercial/grants/`: fit assessments and draft outlines for ESA NAVISP
(Element 2 and 3, Estonia as participating state), EUSPA calls, and NATO DIANA; a
calendar of deadlines; an audit scoping note and request for proposal for an independent
review of the trust-score method. Founder submits; nothing is sent externally by a worker.

## Release gate for 0.35.0

1. Every workstream PR merged with a Fable review, CI green.
2. ROADMAP, README feature list, docs index and site updated; placement rows for the new
   features (with W1b's placement-row work).
3. Coverage job time measured on the release candidate. If it exceeds 300 minutes, the
   coordinator raises it with the founder before tagging (W1b's opt-level work first).
4. Public-text scan: no names, no performance numbers, no competitor reference.
5. Studio: rebuild the Studio's packaged wasm from the 0.35 engine (it is still 0.31.0, so
   it cannot run new scenario kinds such as the vessel platform); port W17's views
   (port_site.py STUDIO_PAGES entries, demo assets) and add their tests to ci.yml.
6. Normal release flow: bump commit, main CI, Auto-tag, Release workflow, Pages check.

## Sessions (started 2026-10-09)

| Worker | Session | Branch |
|---|---|---|
| W9 | session_0152cP5CFP8yUG3zsVp5gcoy | feat/maritime-trust |
| W10 | session_0189muufZnUWoYZ12SH8jxLc | feat/marine-integrations |
| W11 | session_01TXLnsnQRNmTJEs6efhkGQz | feat/nmea-training |
| W12 | session_01Vnxir8oZ8fYdV7FcCMZQAk | feat/osnma-verify |
| W13 | session_01CbLTuiNyyj3wGPwNE7Rgxm | feat/interference-map |
| W14 | session_01EHuXAergYmDpPWprBviAZe | feat/trust-telemetry |
| W15 | session_01K7EwRN9nAMFR2oABNLH1cA | feat/compliance-bench |
| W16 | session_012ALsjTj5mB6yjGycqu6mjN | pro/grants-audit (kshana-pro) |
