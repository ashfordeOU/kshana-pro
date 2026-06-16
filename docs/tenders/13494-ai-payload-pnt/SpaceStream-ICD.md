<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; never publish.
Contact: contact@ashforde.org
-->

# SpaceStream — NAVEX → NAV-DT Interface Control Document (DESIGN PROPOSAL)

**Status: interface DESIGN PROPOSAL.** This document describes a *proposed* ground-side
message interface for the closed-loop "SpaceStream" link between a reconfigurable PNT
payload (NAVEX, **a partner-/programme-owned concept — not Ashforde hardware, and not
assumed to exist**) and the ground Navigation Digital Twin (NAV-DT). It is **not** an
agreed or flight interface; the authoritative interface will be defined by the real
**SoW FPD-10030 SRD**. Ashforde provides the ground-side **schema + codec**, one
component of a partner-led NAV-DT.

**Implementation:** `kshana-pro` `src/connectors/spacestream.rs` (codec + JSON-Schema
export); golden sample messages in `kshana-pro/scenarios/spacestream-*.json`.

## 1. Scope and the hard data rule

SpaceStream carries **only aggregated per-pass metrics and alerts**. It **never**
carries raw IQ, sample streams, or any field/raw measurement data. This is enforced in
two places:
- the JSON Schema is **closed** (`additionalProperties: false`) and contains no IQ /
  sample / waveform field, and
- a unit test (`schema_forbids_raw_sample_or_iq_fields`) fails the build if such a field
  is ever introduced.

Rationale: the NAV-DT predicts performance/resilience from *summaries*; keeping raw
signal data off the link is both the bandwidth-correct design and the
export-control-correct one (no raw RF/threat data crosses the interface).

## 2. Versioning

- `format` is the constant `spacestream-navex-navdt` — how a consumer recognises the
  message before parsing the body.
- `schema_version` is **semver**; compatibility is by **major** version. A consumer
  refuses a message whose major version it does not speak (`SpaceStreamMessage::parse`).
- Current version: **1.0.0**.

## 3. Field dictionary (each field → its NAV-DT consumer)

| Field | Type / unit | Meaning | NAV-DT consumer |
|-------|-------------|---------|-----------------|
| `format` | string const | Message recognition tag. | Ingest router — accept/reject. |
| `schema_version` | string (semver) | Producer schema version. | Ingest router — compatibility gate. |
| `pass_id` | string | Opaque per-pass identifier. | Pass indexer / time-series key. |
| `pass_timestamp_utc` | string (ISO-8601 UTC) | Pass epoch. **Deterministic**, never wall-clock. | Time alignment of the twin's predictions. |
| `ranging_timing.timing_rms_ns` | number (ns) | RMS clock-phase error over the pass. | Timing-accuracy performance model. |
| `ranging_timing.timing_p95_ns` | number (ns) | 95th-percentile timing error. | Tail-risk / spec-margin model. |
| `availability` | number `[0,1]` | Fraction of the pass with an in-spec solution. | Availability prediction & SLA tracking. |
| `integrity.araim_available` | bool | Whether an ARAIM solution was available. | Integrity-availability model. |
| `integrity.self_consistency` | number `[0,1]` or null | Filter self-consistency (Kalman k-σ containment). **Not** aviation HPL/VPL. | Estimator-health monitor. |
| `integrity.alert` | bool | Any integrity alert fired this pass. | Alarm aggregation. |
| `rf_impairment_alert` | object or null | Aggregated impairment classification (see below). `null` = nominal. | Resilience / threat-posture model. |
| `rf_impairment_alert.label` | string | `nominal` / `jamming` / `spoof-time` / `spoof-position` / `multipath`. | Impairment classifier consumer. |
| `rf_impairment_alert.confidence` | number `[0,1]` | Classifier confidence. | Confidence-weighted fusion. |
| `rf_impairment_alert.params` | object{string→number} | A few aggregated decision parameters (e.g. `js_db`, `cn0_drop_db`, `agc_excess_db`, `sqm_metric`). **No raw IQ.** | Diagnostic drill-down. |
| `link.cn0_dbhz` | number (dB-Hz) | Aggregated carrier-to-noise summary. | Link-budget / margin model. |
| `link.margin_db` | number (dB) | Link margin summary. | Link-budget / margin model. |

The RF-impairment label vocabulary aligns with the open core's
`kshana::impairment_eval::ImpairmentClass` (the AI-evaluation testbed), so a NAVEX-side
classifier and the ground evaluation share one vocabulary.

## 4. Composition with the open core

A message is built from open-core figures of merit:
`SpaceStreamMessage::from_pass(pass_id, epoch, &FoMScores, araim_available, link, alert)`
takes the timing/availability/self-consistency directly from `kshana::fom::FoMScores`
and the pass epoch from a `kshana::`-derived ephemeris sample's Julian date — so the
interface is grounded in the same engine that produces the rest of the bid's numbers.

## 5. Machine-readable schema

`spacestream::json_schema()` emits a JSON Schema (draft 2020-12) for the message; ship
it alongside the ICD for a NAV-DT consumer to validate against. The four golden sample
messages in `scenarios/spacestream-*.json` are checked against the canonical schema by a
unit test, so the examples cannot drift from the contract.

Emit it from the binary with `kshana-pro spacestream --schema`, or the four golden
messages with `kshana-pro spacestream`. The canonical schema is embedded below and held
byte-identical to the codec by `tests/spacestream_schema_in_sync.rs` — so this ICD
cannot drift from the code:

<!-- BEGIN GENERATED: `kshana-pro spacestream --schema` -->
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "availability": {
      "maximum": 1.0,
      "minimum": 0.0,
      "type": "number"
    },
    "format": {
      "const": "spacestream-navex-navdt"
    },
    "integrity": {
      "additionalProperties": false,
      "properties": {
        "alert": {
          "type": "boolean"
        },
        "araim_available": {
          "type": "boolean"
        },
        "self_consistency": {
          "type": [
            "number",
            "null"
          ]
        }
      },
      "required": [
        "araim_available",
        "alert"
      ],
      "type": "object"
    },
    "link": {
      "additionalProperties": false,
      "properties": {
        "cn0_dbhz": {
          "type": "number"
        },
        "margin_db": {
          "type": "number"
        }
      },
      "required": [
        "cn0_dbhz",
        "margin_db"
      ],
      "type": "object"
    },
    "pass_id": {
      "type": "string"
    },
    "pass_timestamp_utc": {
      "description": "ISO-8601 UTC; deterministic, never wall-clock",
      "type": "string"
    },
    "ranging_timing": {
      "additionalProperties": false,
      "properties": {
        "timing_p95_ns": {
          "type": "number"
        },
        "timing_rms_ns": {
          "type": "number"
        }
      },
      "required": [
        "timing_rms_ns",
        "timing_p95_ns"
      ],
      "type": "object"
    },
    "rf_impairment_alert": {
      "additionalProperties": false,
      "properties": {
        "confidence": {
          "maximum": 1.0,
          "minimum": 0.0,
          "type": "number"
        },
        "label": {
          "type": "string"
        },
        "params": {
          "additionalProperties": {
            "type": "number"
          },
          "type": "object"
        }
      },
      "required": [
        "label",
        "confidence",
        "params"
      ],
      "type": [
        "object",
        "null"
      ]
    },
    "schema_version": {
      "description": "semver; compatibility by major",
      "type": "string"
    }
  },
  "required": [
    "format",
    "schema_version",
    "pass_id",
    "pass_timestamp_utc",
    "ranging_timing",
    "availability",
    "integrity",
    "link"
  ],
  "title": "SpaceStream NAVEX→NAV-DT message",
  "type": "object"
}
```
<!-- END GENERATED -->

## 6. What this is NOT (honest boundary)

- Not an agreed/flight interface — a design proposal pending FPD-10030.
- Not an implication that NAVEX hardware exists or is Ashforde's.
- Not a raw-data pipe — aggregated metrics/alerts only.
- The integrity field is filter self-consistency, **not** an aviation HPL/VPL/RAIM
  integrity guarantee.

_Last updated 2026-06-15. PRIVATE — do not publish._
