# Avionics test library

`kshana-pro avionics` (module `kshana_pro::avionics`): executable test procedures for airborne
GNSS equipment, an SBAS message-sequence generator for failure tests, ARAIM behaviour checks,
a recovery-time scorecard per avionics type, an evidence pack and a tool-qualification support
kit. Research 2026-11-20 gaps, epic E12 (items B1201, B1203, B1205, B1206 first part).

**Evidence label: MODELLED.** No avionics unit has been tested with this library in this
build. The procedures and example values are representative; the scoring is shown on
synthetic equipment logs only.

## What the procedures are, and are not

Each procedure is *modelled on* an RTCA or EUROCAE document (or a public WG-C ARAIM document)
and cites it by document, revision and section only. The paid documents' text and tables are
not reproduced. Where a check needs a numeric limit that is not openly published (a time to
alert, a fast-correction time-out, an alert limit), the procedure names a **parameter** and
the applicant supplies the value from their own copy, with its source, in a parameter file.
A check whose limit was not supplied is **not assessed**, never met. A verdict is test
evidence against the stated criteria for the run made; it never says that the equipment meets
a standard. The tool-qualification support kit is material for the applicant's own process;
it is not a qualification of the tool.

## Commands

| Command | Licence feature | Tier |
|---|---|---|
| `list`, `show <id>`, `validate <procedure>`, `sample <procedure> --behaviour good\|bad`, `verify <dir> --trust <key>` | none | free |
| `generate <procedure\|sequence.toml> --out <dir>` | `pro-avionics-tests` | Pro |
| `run <procedure> --out <dir> --signer --key-file [--params] [--log]` | `pro-avionics-tests` | Pro |
| `recovery --fleet --verdicts --out --signer --key-file [--target --target-source] [--trust]` | `pro-avionics-recovery` | Pro |
| `evidence --trace --runs --out --signer --key-file --trust` | `pro-avionics-kit` | Enterprise |
| `kit --out --signer --key-file` | `pro-avionics-kit` | Enterprise |

`run` exits 1 when any check is not met. Every output directory is signed with
`signed_dossier` (Ed25519, SHA-256 per file) and is never overwritten.

## Formats

Examples: `avionics/examples/` (parameter, fleet and trace files; demonstration values only).


- `kshana-avionics-procedure/1` (TOML): id, version, title, purpose, `kind`
  (`sbas-sequence`, `araim` or `gbas-events`), `modelled_on`, `[[parameter]]` (name, unit, meaning, source;
  `public_value` and `public_source` only for openly published values), then `[sequence]` or
  `[araim]`.
- `kshana-avionics-params/1` (TOML): `supplied_by`, `[values]`, `[sources]` (per value: the
  document, revision and section the applicant took it from).
- `kshana-avionics-dut-log/1` (JSON): device, procedure id, source; `epochs` (t_s, mode,
  used_prns, hpl_m, vpl_m, alert) for SBAS procedures; `araim_cases` (id, vpl_m, hpl_m,
  emt_m, sigma_v_acc_m, available_vertical, available_horizontal, excluded_constellations)
  for ARAIM procedures. Modes, least to most SBAS-dependent: `none`, `gnss-only`,
  `sbas-npa`, `sbas-apv`, `sbas-pa`.
- `kshana-avionics-verdict/1` (JSON, method `kshana-pro/avionics/v1`): limits in force with
  their sources, one check per expectation (met, not met, not assessed, measured value,
  detail), counts and `pass` / `fail` / `incomplete`.
- `kshana-sbas-sequence/1` (stand-alone sequence TOML) and `kshana-sbas-stream/1` (the
  generated stream with expectations); `frames.csv` is the neutral frame file
  (`t_s,mt,hex,why`; empty when silent).
- `kshana-avionics-fleet/1`, `kshana-avionics-recovery-scorecard/1`,
  `kshana-avionics-trace/1`, `kshana-avionics-evidence-pack/1`, `kshana-avionics-tool-kit/1`.

## SBAS framing and the sequence generator (B1201)

Frames are the public SBAS L1 layout: 250 bits, preamble rotating 0x53, 0x9A, 0xC6, 6-bit
message type, 212 data bits, CRC-24Q over the first 226 bits. Built: MT 0, 1, 2 to 5, 6, 7,
26, 63, encode and decode, refusing out-of-range fields.

The generator sends one message per second: event-driven messages first (MT0 while a
do-not-use event is active; MT1 at an IODP change; MT6 at an alarm onset with IODF 3 for the
satellite's block, and at its end), then the most overdue scheduled stream (mask, each
fast-correction block, MT6, MT7, MT26 blocks), else MT63. The schedule intervals are
test-design choices; set them to the maximum update intervals of your copy of the MOPS to
test the worst case. Output is deterministic (frames hash).

Events and the expectation each produces:

| Event | Expectation | Limit parameter | Reference time |
|---|---|---|---|
| `udrei` 14 or 15 | satellite excluded until the event ends; used again after | `alert_time_s`; `return_time_s` | first message carrying it; event end |
| `withhold_fast` | every satellite of the block excluded; used again after | `fast_timeout_s`; `return_time_s` | the block's last message; event end |
| `do_not_use` (MT0) | mode at most `gnss-only` | `alert_time_s` | first MT0 |
| `silence` | mode at most `gnss-only` | `message_loss_s` | last frame |
| `fc_step` | satellite stays in use | none | event start |
| `givei`, `iodp_change` | informational | none | event start |

A precondition check confirms the equipment used every affected satellite before the first
event, so an exclusion can be shown.

Built-in SBAS procedures: `sbas-udrei-do-not-use`, `sbas-udrei-not-monitored`,
`sbas-fast-correction-timeout`, `sbas-mt0-do-not-use`, `sbas-message-loss`,
`sbas-fast-correction-step`.

## ARAIM checks (B1203)

On the open engine's reference airborne algorithm (`kshana::araim_reference`, open engine
0.41.0):

- `araim-published-vectors`: the WG-C ARAIM Technical Subgroup's worked examples (ADD v3.1
  Appendix D; Milestone 3 Annex A §A.IX), each output against the tolerance the source
  states. The Milestone 3 protection levels are not assessed: the source's own values are not
  reproducible together (recorded in the open engine).
- `araim-h-dual-constellation`: a GPS and Galileo case; reference VPL, HPL, EMT and vertical
  accuracy sigma; the equipment's values compared within `tol_pl_m` (public value 0.05 m,
  TOL_PL of the ADD v3.1, Table 4) and its availability decisions compared with the reference
  decision under the applicant's `hal_m` and `val_m` (and optional `emt_limit_m`,
  `sigma_acc_limit_m`).
- `araim-institutional-unusable`: Galileo declared unusable by the ISM; the equipment must
  exclude it and match the GPS-only reference.
- `araim-leo-augmentation`: six LEO satellites with hypothetical ISM values added; the
  reference reports the change in VPL and HPL with and without them.

Vertical results are V-ARAIM readiness evidence only (no V-ARAIM equipment standard is issued).

## GBAS test automation (B1204)

Procedures of kind `gbas-events` (`src/avionics/gbas.rs`) state a timeline of stimulus events
as the GBAS VDB test bench produces them (`vdb-loss`, `correction-stale`, `fas-data-invalid`,
`protection-over-alert`, and an informational `ssid-mismatch`). Each scored event expects the
equipment to leave the GBAS approach (mode at most `gnss-only`; the log's mode `gbas-gls` ranks
above the SBAS modes) within a named limit after the event starts and while it lasts; what
happens after it ends is recorded, not scored. The precondition is the GBAS approach in use
before the first event. The limits (`vdb_loss_limit_s`, `correction_timeout_s`, `fas_alert_s`,
`alert_time_s`) come from the applicant's copy of RTCA DO-253; without them a check is not
assessed. No VDB message is encoded or decoded here: the VDB decoder belongs in the open engine
and the bench produces the signal. Built-in: `gbas-vdb-loss-and-fas`. The event semantics are
to confirm against the applicant's copy of DO-253 and the bench documentation. Tests:
`tests/gbas_procedures.rs`; the tool kit's verification (F3) scores it with demonstration
limits.

## Recovery-time scorecard (B1205)

Reads the test kit's scenario verdicts (`kshana-pro testkit run`, for example the aviation
pack's post-spoof recovery and reset scenarios), takes each `recovery_s` criterion as one run,
assigns runs to avionics types through the fleet file (receiver names per type, aircraft
counts), and reports per type: runs, recovered, not recovered, not assessed, median, 95th
percentile, worst, and the share recovered within the operator's target. Types are listed
worst first; with aircraft counts it gives the fleet-weighted share. No target is built in.
With `--trust`, only verdicts whose signature verifies are used.

## Evidence pack and tool-qualification support kit (B1206, first part)

- `evidence`: the applicant's trace file maps their requirement identifiers to procedures.
  The pack lists every run found (verdict, counts, log hash, signature status), the
  requirement-to-run matrix (`pass`, `fail`, `incomplete`, `no run`; only runs signed by a
  trusted key count) and `TEST-RECORDS.md`.
- `kit`: runs the tool's verification cases on this build (CRC-24Q check value, round trip of
  every message type and parity failure on a flipped bit, deterministic streams that decode,
  scoring that separates a reacting from a non-reacting synthetic log for every procedure, the
  ARAIM published vectors) and writes `VERIFICATION-RESULTS.json`,
  `CONFIGURATION-INDEX.json` (tool and open engine versions, every procedure's hash, formats),
  `TOOL-OPERATIONAL-REQUIREMENTS.md` (a draft), `APPLICANT-ACTIONS.md` and
  `KNOWN-LIMITATIONS.md`. The applicant decides whether the tool needs a qualification and at
  which level (RTCA DO-178C §12.2, RTCA DO-330) and maps the material to the DO-330 Annex A
  objectives themselves.

## Not built yet

- B1202 DFMC SBAS (DO-401A / ED-259B) vectors and L5-only scenarios: the DFMC message layouts
  are in the paid MOPS, which is not frozen; no vectors are written from guesses.
- B1204 GBAS VDB decoder and DO-253 airborne automation.
- B1206 second part: DO-178C/DO-254-style record templates beyond the test records, ETSO/TSO
  deviation mapping.
- B1207 is open-engine work (kshana); B1208 waits for the standardised spoofing-detection
  outputs to be defined.
- SBAS message types 9, 10, 17, 18, 24, 25, 27, 28; simulator import of the frame file.
