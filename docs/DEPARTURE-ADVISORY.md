# Departure go/no-go and pre-entry warning against verified zones

Research 2026-11-20 gaps, backlog B0806. Module `src/exposure/departure.rs`, command
`kshana-pro exposure departure` (`src/cli/evidence_cmd.rs`), tests `tests/departure_advisory.rs`.

Builds on what was there: the unified interference event format (`src/ievents.rs`), into which
area-console events convert (`ievents::from_area_event`), and the voyage risk model of
`src/exposure/voyage.rs` (same kind weights; the optional equipment rating comes from
`exposure equipment`). What this adds: verified zones, timing along the passage, the go/no-go
status and the checks.

`exposure departure <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-departure-advisory`, Pro). The request names the passage (waypoints, decision time, planned
speed), the **verified zones** (polygon, `type` = `vts` | `sts` | `ice` | `pilotage`,
`verified_by`, `verified_utc`; an unverified zone is refused), the event files and optionally the
equipment rating.

Per zone: entry ETA (planned speed), pre-entry warning time (`warn_lead_min`, 60 min), the events
whose affected circle touches the zone and started before the decision time, whether one is active,
the zone risk, a status and a confidence, and recommended checks for the zone type and the kinds of
interference recorded. The departure advisory is the worst status of the zones on the passage.

Method (also in every advisory): event weight = kind (spoofing or meaconing 1.0, jamming 0.6,
outage 0.3, other 0.4) × confidence × exp(−age / `recency_h`); exposure E = 1 − exp(−Σw); risk =
100 × E × zone criticality (pilotage, STS 1.0; ice 0.9; VTS 0.8) × (0.2 + 0.8 × (1 − rating/100)).
HOLD-AND-VERIFY when interference is active and risk ≥ `hold_risk` (35), or risk ≥ 2 × `hold_risk`;
PROCEED-WITH-CHECKS at risk ≥ `checks_risk` (15); else PROCEED. Confidence 1 − Π(1 − cᵢ ×
recency) over the zone's events; with none, 0.5 when the record is live, 0.2 when stale.

Only the affected place of each event is used; `source_estimate` is never read and the advisory
counts how many it ignored. An **advisory**: the decision to sail or to enter a zone is the
master's, and no reports is not evidence of no interference. **MODELLED**: weights are engineering
choices exercised on a synthetic passage (`exposure departure-synth`). Nothing here fuses sensors
for navigation or locates an emitter.
