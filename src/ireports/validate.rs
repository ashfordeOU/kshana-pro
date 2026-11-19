// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Mandatory fields per reporting regime. Each regime is a list of rules; a report is
//! `complete` when no mandatory rule fails. The rules paraphrase what each regime asks for,
//! as summarised in Kshana research (05, 07, 07b); check them against the authority's own
//! text before relying on them for a filing.
//!
//! | Regime | Source (as summarised) | Deadline rule |
//! |---|---|---|
//! | `dgca` | DGCA India circular, 10 Nov 2025: type (jamming / spoofing / signal loss / integrity error), coordinates, affected equipment, logs and FMS screenshots | report within 10 minutes of the occurrence |
//! | `easa` | Regulation (EU) No 376/2014 occurrence reporting, ECR/ECCAIRS-like content | report within 72 hours |
//! | `asrs` | NASA ASRS-like voluntary report (narrative first) | filing within 10 days (FAA AC 00-46, recommended only) |
//! | `maritime` | Ship reports; UK MGN 719 and the RIN survey's GNSS-vs-AIS gap | none |
//! | `generic` | The common minimum for an episode definition (EASA/EUROCONTROL action plan, Feb 2026) | none |

use serde::{Deserialize, Serialize};

use super::{Kind, Layer, Platform, Report, ReportedType};

/// The regimes, by id.
pub const REGIMES: &[&str] = &["dgca", "easa", "asrs", "maritime", "generic"];

/// Mandatory (a filing would be refused or incomplete) or recommended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Mandatory,
    Recommended,
}

/// One failed rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub field: String,
    pub severity: Severity,
    pub rule: String,
}

/// The validation of one report under one regime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Validation {
    pub report_id: String,
    pub regime: String,
    /// `complete` or `incomplete`.
    pub status: String,
    pub issues: Vec<Issue>,
    /// Minutes from occurrence to report, where both are known.
    #[serde(default)]
    pub report_delay_min: Option<f64>,
}

impl Validation {
    pub fn complete(&self) -> bool {
        self.status == "complete"
    }
}

struct Ctx {
    issues: Vec<Issue>,
}

impl Ctx {
    fn need(&mut self, ok: bool, field: &str, sev: Severity, rule: &str) {
        if !ok {
            self.issues.push(Issue {
                field: field.into(),
                severity: sev,
                rule: rule.into(),
            });
        }
    }
}

/// Validate `r` under `regime` (one of [`REGIMES`]).
pub fn validate(r: &Report, regime: &str) -> Result<Validation, String> {
    use Severity::*;
    let mut c = Ctx { issues: vec![] };
    let start = r.start_t();
    let delay = match (start, r.reported_t()) {
        (Some(s), Some(p)) => Some(super::text::round((p - s) / 60.0, 1)),
        _ => None,
    };
    let aircraft = r.platform == Platform::Aircraft;
    let has_pos = r.position.is_some();
    let precise_time = start.is_some() && r.time_uncertainty_s <= 300.0;
    match regime {
        "dgca" => {
            c.need(
                precise_time,
                "occurred_start_utc",
                Mandatory,
                "date and time of occurrence (UTC), to the minute",
            );
            c.need(
                r.reported_utc.is_some(),
                "reported_utc",
                Mandatory,
                "time of report, to check the 10-minute rule",
            );
            if let Some(d) = delay {
                c.need(
                    d <= 10.0,
                    "reported_utc",
                    Mandatory,
                    &format!("report within 10 minutes of the occurrence (was {d} min)"),
                );
            }
            c.need(
                matches!(
                    r.reported_type,
                    ReportedType::Jamming
                        | ReportedType::Spoofing
                        | ReportedType::SignalLoss
                        | ReportedType::IntegrityError
                ),
                "reported_type",
                Mandatory,
                "type: jamming, spoofing, signal loss or integrity error",
            );
            c.need(
                has_pos,
                "position",
                Mandatory,
                "coordinates of the occurrence",
            );
            c.need(
                r.altitude.is_some() || !aircraft,
                "altitude",
                Mandatory,
                "flight level or altitude",
            );
            c.need(
                !r.systems_affected.is_empty(),
                "systems_affected",
                Mandatory,
                "affected equipment",
            );
            c.need(
                r.logs_preserved == Some(true),
                "logs_preserved",
                Mandatory,
                "logs preserved",
            );
            c.need(
                r.fms_screenshot == Some(true) || !aircraft,
                "fms_screenshot",
                Mandatory,
                "FMS screenshots",
            );
            c.need(
                r.subject_id.is_some() || r.flight_or_voyage.is_some(),
                "subject_id",
                Mandatory,
                "aircraft registration or flight number",
            );
            c.need(
                r.recovery_action.is_some(),
                "recovery_action",
                Recommended,
                "crew action taken",
            );
        }
        "easa" => {
            c.need(
                precise_time,
                "occurred_start_utc",
                Mandatory,
                "UTC date and time of the occurrence",
            );
            c.need(
                r.subject_id.is_some(),
                "subject_id",
                Mandatory,
                "aircraft registration",
            );
            c.need(
                has_pos || r.extra.contains_key("location_text"),
                "position",
                Mandatory,
                "location",
            );
            c.need(
                r.reported_type != ReportedType::NotStated,
                "reported_type",
                Mandatory,
                "occurrence category / event type",
            );
            c.need(r.narrative.is_some(), "narrative", Mandatory, "narrative");
            c.need(
                r.reporter_role != super::ReporterRole::NotStated,
                "reporter_role",
                Mandatory,
                "reporter category",
            );
            if let Some(d) = delay {
                c.need(
                    d <= 72.0 * 60.0,
                    "reported_utc",
                    Mandatory,
                    &format!("report within 72 hours (was {d} min)"),
                );
            }
            c.need(
                r.altitude.is_some() || !aircraft,
                "altitude",
                Recommended,
                "flight level or altitude",
            );
            c.need(
                r.flight_or_voyage.is_some(),
                "flight_or_voyage",
                Recommended,
                "flight number",
            );
        }
        "asrs" => {
            c.need(r.narrative.is_some(), "narrative", Mandatory, "narrative");
            c.need(
                start.is_some(),
                "occurred_start_utc",
                Mandatory,
                "date (month at least)",
            );
            c.need(
                r.extra.contains_key("location_text") || has_pos,
                "position",
                Recommended,
                "locale reference",
            );
            if let Some(d) = delay {
                c.need(
                    d <= 10.0 * 24.0 * 60.0,
                    "reported_utc",
                    Recommended,
                    &format!("filed within 10 days (was {d} min)"),
                );
            }
        }
        "maritime" => {
            c.need(
                matches!(r.subject_id_type.as_deref(), Some("imo") | Some("mmsi")),
                "subject_id",
                Mandatory,
                "IMO or MMSI number",
            );
            c.need(
                precise_time,
                "occurred_start_utc",
                Mandatory,
                "date and time (UTC)",
            );
            c.need(has_pos, "position", Mandatory, "position");
            c.need(
                r.reported_type != ReportedType::NotStated,
                "reported_type",
                Mandatory,
                "what happened (jamming, spoofing, loss)",
            );
            c.need(
                r.kind != Kind::Spoofing || r.spoofed_layer != Layer::NotStated,
                "spoofed_layer",
                Mandatory,
                "for spoofing: whether the GNSS receiver or the AIS picture was affected",
            );
            c.need(
                !r.systems_affected.is_empty(),
                "systems_affected",
                Recommended,
                "systems affected (PNT dependencies, MGN 719)",
            );
            c.need(
                r.recovery_action.is_some() || r.time_to_recover_min.is_some(),
                "recovery_action",
                Recommended,
                "action taken and time to recover",
            );
        }
        "generic" => {
            c.need(
                precise_time,
                "occurred_start_utc",
                Mandatory,
                "start time (UTC), to five minutes",
            );
            c.need(has_pos, "position", Mandatory, "position");
            c.need(
                r.reported_type != ReportedType::NotStated,
                "reported_type",
                Mandatory,
                "type",
            );
            c.need(
                r.altitude.is_some() || !aircraft,
                "altitude",
                Recommended,
                "altitude (aircraft)",
            );
            c.need(
                r.time_to_recover_min.is_some() || r.occurred_end_utc.is_some(),
                "time_to_recover_min",
                Recommended,
                "end time or time to recover",
            );
        }
        other => {
            return Err(format!(
                "unknown regime '{other}' (one of {})",
                REGIMES.join(", ")
            ))
        }
    }
    let complete = !c.issues.iter().any(|i| i.severity == Mandatory);
    Ok(Validation {
        report_id: r.id.clone(),
        regime: regime.into(),
        status: if complete { "complete" } else { "incomplete" }.into(),
        issues: c.issues,
        report_delay_min: delay,
    })
}

/// The regime that fits a report's shape, for a default.
pub fn default_regime(r: &Report) -> &'static str {
    match (r.source.format, r.platform) {
        (super::SourceFormat::Dgca, _) => "dgca",
        (super::SourceFormat::Eccairs, _) => "easa",
        (super::SourceFormat::Asrs, _) => "asrs",
        (_, Platform::Vessel) => "maritime",
        _ => "generic",
    }
}
