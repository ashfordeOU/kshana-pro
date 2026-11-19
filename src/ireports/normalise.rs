// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The report normaliser (`kshana-pro/interference-report-normaliser/v1`): many shapes in,
//! one [`Report`] schema out.
//!
//! | Shape | Recognised by | Records |
//! |---|---|---|
//! | DGCA-style form | `key: value` lines, DGCA field names (`Event type`, `Time of report`, `FMS screenshots`, `Logs preserved`) | separated by a `---` line |
//! | EASA occurrence form (ECR/ECCAIRS-like names) | `Occurrence ...`, `Headline`, `Registration` keys | separated by `---` |
//! | ASRS-like record | an `ACN` key, `Local Time Of Day`, `Narrative` | separated by `---` |
//! | NOTAM | `Q)` and `E)` items | one per NOTAM (blank line between) |
//! | Ship log | dated lines (`2026-06-14 0412Z <position> <text>`), `IMO`/`MMSI` header | one per dated line |
//! | CSV export | a header row | one per row |
//!
//! Field names are matched after normalisation (case, punctuation) against an alias table;
//! fields with no place in the schema are kept in `extra`. Every inference (a type read from
//! the narrative, a local time) is written to the report's `notes`. The DGCA, EASA and ASRS
//! shapes are modelled on what those regimes ask for, as summarised in Kshana research; they
//! are not the authorities' own templates or the ECCAIRS taxonomy codes.

use std::collections::BTreeMap;

use super::text::{self, iso, parse_coords};
use super::*;

/// What one field of a form means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Role,
    Reporter,
    Flight,
    Registration,
    Imo,
    Mmsi,
    Icao24,
    VesselName,
    PlatformType,
    Date,
    Time,
    DateTime,
    EndTime,
    EndDateTime,
    LocalTimeBin,
    ReportTime,
    ReportDateTime,
    Position,
    Lat,
    Lon,
    Altitude,
    Type,
    Systems,
    Recovery,
    Duration,
    Fms,
    Logs,
    Narrative,
    Layer,
}

/// Normalised key → field.
const ALIASES: &[(&str, Field)] = &[
    ("reporter role", Field::Role),
    ("reporter category", Field::Role),
    ("reporter function", Field::Role),
    ("reported by", Field::Role),
    ("function", Field::Role),
    ("function flight crew", Field::Role),
    ("reporter", Field::Reporter),
    ("reporter name", Field::Reporter),
    ("name of reporter", Field::Reporter),
    ("flight number", Field::Flight),
    ("flight no", Field::Flight),
    ("flight", Field::Flight),
    ("callsign", Field::Flight),
    ("call sign", Field::Flight),
    ("voyage", Field::Flight),
    ("voyage number", Field::Flight),
    ("aircraft registration", Field::Registration),
    ("registration", Field::Registration),
    ("aircraft reg", Field::Registration),
    ("tail number", Field::Registration),
    ("tail", Field::Registration),
    ("subject", Field::Registration),
    ("subject id", Field::Registration),
    ("imo", Field::Imo),
    ("imo number", Field::Imo),
    ("imo no", Field::Imo),
    ("mmsi", Field::Mmsi),
    ("icao24", Field::Icao24),
    ("icao 24 bit address", Field::Icao24),
    ("vessel", Field::VesselName),
    ("vessel name", Field::VesselName),
    ("ship name", Field::VesselName),
    ("aircraft type", Field::PlatformType),
    ("type of aircraft", Field::PlatformType),
    ("aircraft make model", Field::PlatformType),
    ("make model name", Field::PlatformType),
    ("aircraft 1 make model name", Field::PlatformType),
    ("vessel type", Field::PlatformType),
    ("date of occurrence utc", Field::Date),
    ("date of occurrence", Field::Date),
    ("occurrence date", Field::Date),
    ("utc date", Field::Date),
    ("date", Field::Date),
    ("time of occurrence utc", Field::Time),
    ("time of occurrence", Field::Time),
    ("occurrence time", Field::Time),
    ("utc time", Field::Time),
    ("time", Field::Time),
    ("start time", Field::Time),
    ("start utc", Field::DateTime),
    ("occurred utc", Field::DateTime),
    ("occurred start utc", Field::DateTime),
    ("occurrence utc", Field::DateTime),
    ("timestamp", Field::DateTime),
    ("utc", Field::DateTime),
    ("end time", Field::EndTime),
    ("end time utc", Field::EndTime),
    ("time ended", Field::EndTime),
    ("end utc", Field::EndDateTime),
    ("occurred end utc", Field::EndDateTime),
    ("local time of day", Field::LocalTimeBin),
    ("time of report utc", Field::ReportTime),
    ("time of report", Field::ReportTime),
    ("report time", Field::ReportTime),
    ("reporting time", Field::ReportTime),
    ("reported at", Field::ReportTime),
    ("reported utc", Field::ReportDateTime),
    ("report utc", Field::ReportDateTime),
    ("position", Field::Position),
    ("location", Field::Position),
    ("coordinates", Field::Position),
    ("lat long", Field::Position),
    ("lat lon", Field::Position),
    ("latitude longitude", Field::Position),
    ("position lat long", Field::Position),
    ("latitude", Field::Lat),
    ("lat", Field::Lat),
    ("longitude", Field::Lon),
    ("lon", Field::Lon),
    ("long", Field::Lon),
    ("flight level altitude", Field::Altitude),
    ("flight level", Field::Altitude),
    ("altitude", Field::Altitude),
    ("altitude ft", Field::Altitude),
    ("alt ft", Field::Altitude),
    ("level", Field::Altitude),
    ("altitude msl single value", Field::Altitude),
    ("altitude agl single value", Field::Altitude),
    ("event type", Field::Type),
    ("type of event", Field::Type),
    ("occurrence type", Field::Type),
    ("interference type", Field::Type),
    ("type", Field::Type),
    ("kind", Field::Type),
    ("event", Field::Type),
    ("anomaly", Field::Type),
    ("events anomaly", Field::Type),
    ("equipment affected", Field::Systems),
    ("systems affected", Field::Systems),
    ("affected systems", Field::Systems),
    ("affected equipment", Field::Systems),
    ("equipment", Field::Systems),
    ("aircraft component", Field::Systems),
    ("recovery action", Field::Recovery),
    ("action taken", Field::Recovery),
    ("actions taken", Field::Recovery),
    ("crew action", Field::Recovery),
    ("events result", Field::Recovery),
    ("result", Field::Recovery),
    ("duration", Field::Duration),
    ("duration min", Field::Duration),
    ("time to recover", Field::Duration),
    ("time to recover min", Field::Duration),
    ("recovery time", Field::Duration),
    ("fms screenshots", Field::Fms),
    ("fms screenshot", Field::Fms),
    ("screenshots attached", Field::Fms),
    ("logs preserved", Field::Logs),
    ("data preserved", Field::Logs),
    ("qar data preserved", Field::Logs),
    ("fdr qar preserved", Field::Logs),
    ("narrative", Field::Narrative),
    ("narrative 1", Field::Narrative),
    ("description", Field::Narrative),
    ("remarks", Field::Narrative),
    ("synopsis", Field::Narrative),
    ("headline", Field::Narrative),
    ("details", Field::Narrative),
    ("spoofed system", Field::Layer),
    ("spoofed layer", Field::Layer),
    ("affected layer", Field::Layer),
];

fn field_of(key: &str) -> Option<Field> {
    let k = text::norm_key(key);
    ALIASES.iter().find(|(a, _)| *a == k).map(|(_, f)| *f)
}

/// The shape of a file, from its content.
pub fn detect(content: &str) -> SourceFormat {
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let has_key = |k: &str| {
        lines.iter().any(|l| {
            l.split_once(':')
                .is_some_and(|(a, _)| text::norm_key(a) == text::norm_key(k))
        })
    };
    if content.contains("Q)") && content.contains("E)") {
        return SourceFormat::Notam;
    }
    if has_key("ACN") {
        return SourceFormat::Asrs;
    }
    if lines.first().is_some_and(|l| {
        l.matches(',').count() >= 3 && !l.contains(": ") && l.split(',').all(|c| c.len() < 40)
    }) {
        return SourceFormat::Csv;
    }
    if lines.iter().any(|l| {
        let u = l.to_ascii_uppercase();
        u.starts_with("IMO") || u.contains(" IMO ") || u.starts_with("MMSI")
    }) && lines.iter().any(|l| ship_entry_head(l).is_some())
    {
        return SourceFormat::ShipLog;
    }
    if has_key("Headline") || has_key("Occurrence class") || has_key("Occurrence category") {
        return SourceFormat::Eccairs;
    }
    if lines.iter().any(|l| ship_entry_head(l).is_some()) && !lines.iter().any(|l| l.contains(": "))
    {
        return SourceFormat::ShipLog;
    }
    SourceFormat::Dgca
}

/// Normalise one file's content. `file` is the name recorded in each report.
pub fn normalise(
    content: &str,
    file: &str,
    format: Option<SourceFormat>,
) -> Result<Vec<Report>, String> {
    let fmt = format.unwrap_or_else(|| detect(content));
    let sha = sha256_hex(content.as_bytes());
    let src = |record: usize| Source {
        format: fmt,
        file: file.to_string(),
        sha256: sha.clone(),
        record,
    };
    let reports = match fmt {
        SourceFormat::Dgca | SourceFormat::Eccairs | SourceFormat::Asrs => kv_records(content)
            .into_iter()
            .enumerate()
            .map(|(i, kv)| from_kv(&kv, src(i + 1)))
            .collect(),
        SourceFormat::Csv => csv_reports(content, &src)?,
        SourceFormat::Notam => notam_records(content)
            .into_iter()
            .enumerate()
            .map(|(i, n)| from_notam(&n, src(i + 1)))
            .collect(),
        SourceFormat::ShipLog => ship_log(content, &src),
    };
    if reports.is_empty() {
        return Err(format!("{file}: no report records found (read as {fmt:?})"));
    }
    Ok(reports)
}

fn new_report(source: Source) -> Report {
    let id = format!(
        "rep-{}",
        &sha256_hex(format!("{}#{}", source.sha256, source.record).as_bytes())[..12]
    );
    Report {
        format: REPORT_FORMAT.into(),
        schema_version: SCHEMA_VERSION,
        id,
        source,
        reporter_role: ReporterRole::NotStated,
        reporter: None,
        platform: Platform::NotStated,
        subject_id: None,
        subject_id_type: None,
        flight_or_voyage: None,
        platform_type: None,
        occurred_start_utc: None,
        occurred_end_utc: None,
        time_uncertainty_s: 60.0,
        reported_utc: None,
        position: None,
        altitude: None,
        kind: Kind::Unknown,
        reported_type: ReportedType::NotStated,
        spoofed_layer: Layer::NotStated,
        systems_affected: vec![],
        recovery_action: None,
        time_to_recover_min: None,
        fms_screenshot: None,
        logs_preserved: None,
        narrative: None,
        extra: BTreeMap::new(),
        notes: vec![],
    }
}

/// `key: value` records separated by `---` lines; indented or colon-free lines continue the
/// previous value.
fn kv_records(content: &str) -> Vec<Vec<(String, String)>> {
    let mut out = vec![];
    let mut cur: Vec<(String, String)> = vec![];
    for raw in content.lines() {
        let line = raw.trim_end();
        let t = line.trim();
        if t.starts_with("---") || t.starts_with("===") {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let kv = t.split_once(':').filter(|(k, _)| {
            let k = k.trim();
            !k.is_empty()
                && k.len() <= 48
                && k.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && k.chars()
                    .all(|c| c.is_ascii_alphanumeric() || " ./()-_&".contains(c))
        });
        match kv {
            Some((k, v)) if !indented || cur.is_empty() => {
                cur.push((k.trim().to_string(), v.trim().to_string()))
            }
            _ => {
                if let Some(last) = cur.last_mut() {
                    if !last.1.is_empty() {
                        last.1.push(' ');
                    }
                    last.1.push_str(t);
                }
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn set_subject(r: &mut Report, id: &str, ty: &str) {
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    // IMO outranks MMSI; a registration or ICAO address keeps the first given.
    let replace = matches!(
        (r.subject_id_type.as_deref(), ty),
        (None, _) | (Some("mmsi"), "imo") | (Some("icao24"), "registration")
    );
    if replace {
        let id = id
            .trim_start_matches("IMO")
            .trim_start_matches("imo")
            .trim()
            .to_string();
        if let (Some(old), Some(old_ty)) = (r.subject_id.take(), r.subject_id_type.take()) {
            r.extra.insert(old_ty, old);
        }
        r.subject_id = Some(id);
        r.subject_id_type = Some(ty.into());
    } else {
        r.extra.insert(ty.into(), id.into());
    }
}

/// Build a report from key-value pairs (forms and CSV rows).
fn from_kv(kv: &[(String, String)], source: Source) -> Report {
    let fmt = source.format;
    let mut r = new_report(source);
    let mut date: Option<(i64, u32, u32)> = None;
    let (mut tod, mut end_tod, mut rep_tod) = (None::<f64>, None::<f64>, None::<f64>);
    let (mut lat, mut lon) = (None::<f64>, None::<f64>);
    let mut type_text: Option<String> = None;
    let mut systems_text = String::new();
    let mut narrative: Vec<String> = vec![];
    let mut local_bin: Option<String> = None;
    let mut asrs_month: Option<(i64, u32)> = None;
    for (k, v) in kv {
        if v.is_empty() {
            continue;
        }
        let Some(f) = field_of(k) else {
            r.extra.insert(k.clone(), v.clone());
            continue;
        };
        match f {
            Field::Role => r.reporter_role = text::role(v),
            Field::Reporter => {
                r.reporter = Some(v.clone());
                if r.reporter_role == ReporterRole::NotStated {
                    r.reporter_role = text::role(v);
                }
            }
            Field::Flight => r.flight_or_voyage = Some(v.clone()),
            Field::Registration => set_subject(&mut r, v, "registration"),
            Field::Imo => set_subject(&mut r, v, "imo"),
            Field::Mmsi => set_subject(&mut r, v, "mmsi"),
            Field::Icao24 => set_subject(&mut r, v, "icao24"),
            Field::VesselName => {
                r.extra.insert("vessel_name".into(), v.clone());
                r.platform = Platform::Vessel;
            }
            Field::PlatformType => r.platform_type = Some(v.clone()),
            Field::Date => {
                let digits = v.trim();
                if fmt == SourceFormat::Asrs
                    && digits.len() == 6
                    && digits.bytes().all(|b| b.is_ascii_digit())
                {
                    asrs_month = digits[0..4].parse().ok().zip(digits[4..6].parse().ok());
                } else if let Some(t) = parse_utc(v) {
                    date = Some(text::date_of(t));
                    tod.get_or_insert(text::sod_of(t));
                } else {
                    date = text::parse_date(v);
                    if date.is_none() {
                        r.notes.push(format!("date '{v}' not read"));
                    }
                }
            }
            Field::Time => {
                tod = text::parse_time_of_day(v);
                if tod.is_none() {
                    if let Some(t) = text::parse_datetime(v) {
                        date = Some(text::date_of(t));
                        tod = Some(text::sod_of(t));
                    } else {
                        r.notes.push(format!("time '{v}' not read"));
                    }
                }
            }
            Field::DateTime => match text::parse_datetime(v) {
                Some(t) => {
                    date = Some(text::date_of(t));
                    tod = Some(text::sod_of(t));
                }
                None => r.notes.push(format!("date-time '{v}' not read")),
            },
            Field::EndTime => {
                end_tod = text::parse_time_of_day(v);
                if end_tod.is_none() {
                    if let Some(t) = text::parse_datetime(v) {
                        r.occurred_end_utc = Some(iso(t));
                    }
                }
            }
            Field::EndDateTime => {
                if let Some(t) = text::parse_datetime(v) {
                    r.occurred_end_utc = Some(iso(t));
                }
            }
            Field::LocalTimeBin => local_bin = Some(v.clone()),
            Field::ReportTime => {
                rep_tod = text::parse_time_of_day(v);
                if rep_tod.is_none() {
                    if let Some(t) = text::parse_datetime(v) {
                        r.reported_utc = Some(iso(t));
                    }
                }
            }
            Field::ReportDateTime => {
                if let Some(t) = text::parse_datetime(v) {
                    r.reported_utc = Some(iso(t));
                }
            }
            Field::Position => match parse_coords(v) {
                Some((la, lo)) => {
                    r.position = Some(Position {
                        lat_deg: la,
                        lon_deg: lo,
                        radius_m: None,
                        text: v.clone(),
                    })
                }
                None => {
                    r.extra.insert("location_text".into(), v.clone());
                    r.notes.push(format!("position '{v}' has no coordinates"));
                }
            },
            Field::Lat => lat = v.trim().parse().ok().or_else(|| one_axis(v)),
            Field::Lon => lon = v.trim().parse().ok().or_else(|| one_axis(v)),
            Field::Altitude => {
                if let Some((lo, hi, rf)) = text::parse_altitude(v) {
                    let rf = if rf == "unstated" && text::norm_key(k).contains("msl") {
                        "AMSL".into()
                    } else if rf == "unstated" && text::norm_key(k).contains("agl") {
                        "AGL".into()
                    } else {
                        rf
                    };
                    r.altitude = Some(Altitude {
                        lower_ft: lo,
                        upper_ft: hi,
                        reference: rf,
                        text: v.clone(),
                    });
                }
            }
            Field::Type => type_text = Some(v.clone()),
            Field::Systems => {
                systems_text.push_str(v);
                systems_text.push(' ');
            }
            Field::Recovery => r.recovery_action = Some(v.clone()),
            Field::Duration => r.time_to_recover_min = text::minutes(v),
            Field::Fms => r.fms_screenshot = text::yes_no(v),
            Field::Logs => r.logs_preserved = text::yes_no(v),
            Field::Narrative => narrative.push(v.clone()),
            Field::Layer => {
                r.spoofed_layer = match text::norm_key(v).as_str() {
                    "gnss" | "gps" => Layer::Gnss,
                    "ais" => Layer::Ais,
                    "both" | "gnss and ais" | "gps and ais" => Layer::Both,
                    _ => text::spoofed_layer(v),
                }
            }
        }
    }
    if let (Some(a), Some(b)) = (lat, lon) {
        if (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b) {
            r.position = Some(Position {
                lat_deg: a,
                lon_deg: b,
                radius_m: None,
                text: format!("{a}, {b}"),
            });
        }
    }
    if !narrative.is_empty() {
        r.narrative = Some(narrative.join(" "));
    }
    // Times.
    match (date, tod) {
        (Some(d), Some(s)) => {
            let t = text::at(d, s);
            r.occurred_start_utc = Some(iso(t));
            if let Some(e) = end_tod {
                let mut te = text::at(d, e);
                if te < t {
                    te += 86_400.0;
                }
                r.occurred_end_utc = Some(iso(te));
            }
            if let Some(rs) = rep_tod {
                let mut tr = text::at(d, rs);
                if tr < t {
                    tr += 86_400.0;
                    r.notes.push(
                        "time of report earlier than the occurrence: read as the next day".into(),
                    );
                }
                r.reported_utc = Some(iso(tr));
            }
        }
        (Some(d), None) if asrs_month.is_none() => {
            r.occurred_start_utc = Some(iso(text::at(d, 0.0)));
            r.time_uncertainty_s = 86_400.0;
            r.notes
                .push("no time of day: start set to 00:00 UTC, uncertain by a day".into());
        }
        _ => {}
    }
    if let Some((y, m)) = asrs_month {
        // De-identified ASRS-like record: month and a six-hour local-time bin only.
        let (h0, h1) = local_bin
            .as_deref()
            .and_then(|b| {
                let (a, z) = b.split_once('-')?;
                Some((
                    a.trim().get(0..2)?.parse::<f64>().ok()?,
                    z.trim().get(0..2)?.parse::<f64>().ok()?,
                ))
            })
            .unwrap_or((0.0, 24.0));
        let days = days_in(y, m);
        let start = text::at((y, m, 1), 0.0);
        r.occurred_start_utc = Some(iso(start + 0.5 * days * 86_400.0));
        r.time_uncertainty_s = 0.5 * days * 86_400.0 + 14.0 * 3600.0;
        r.notes.push(format!(
            "de-identified record: month {y:04}-{m:02} and local time {:02.0}00-{:02.0}00 only (time zone not given); \
             start set to mid-month, too coarse for episodes or cross-checks",
            h0, h1
        ));
    }
    // Type and kind.
    let stated = type_text.as_deref().map(text::classify);
    r.reported_type = match stated {
        Some(t) if t != ReportedType::NotStated => t,
        _ => {
            let n = r.narrative.clone().unwrap_or_default();
            let t = text::classify(&n);
            if t != ReportedType::NotStated {
                r.notes.push(format!(
                    "type '{}' read from the narrative",
                    serde_json::to_value(t)
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_default()
                ));
            }
            t
        }
    };
    if let Some(t) = &type_text {
        r.extra.insert("type_as_reported".into(), t.clone());
    }
    r.kind = r.reported_type.kind();
    // Systems.
    r.systems_affected = text::systems(&systems_text);
    if r.systems_affected.is_empty() {
        if let Some(n) = &r.narrative {
            r.systems_affected = text::systems(n);
            if !r.systems_affected.is_empty() {
                r.notes
                    .push("systems affected read from the narrative".into());
            }
        }
    }
    if r.kind == Kind::Spoofing && r.spoofed_layer == Layer::NotStated {
        let all = format!(
            "{} {} {}",
            type_text.clone().unwrap_or_default(),
            systems_text,
            r.narrative.clone().unwrap_or_default()
        );
        r.spoofed_layer = text::spoofed_layer(&all);
    }
    if r.time_to_recover_min.is_none() {
        if let Some(m) = r.narrative.as_deref().and_then(text::recovery_minutes) {
            r.time_to_recover_min = Some(m);
            r.notes
                .push("time to recover read from the narrative".into());
        }
    }
    infer_platform(&mut r);
    if fmt == SourceFormat::Asrs && r.reporter_role == ReporterRole::NotStated {
        r.reporter_role = ReporterRole::Pilot;
        r.notes
            .push("ASRS-like record: reporter taken as flight crew".into());
    }
    r
}

fn days_in(y: i64, m: u32) -> f64 {
    let next = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (text::at((next.0, next.1, 1), 0.0) - text::at((y, m, 1), 0.0)) / 86_400.0
}

/// One axis given alone with a hemisphere (`31 42N`, `074°48'E`).
fn one_axis(v: &str) -> Option<f64> {
    let u = v.trim().to_ascii_uppercase();
    let (lat_like, other) = if u.contains('N') || u.contains('S') {
        (true, "0E")
    } else {
        (false, "0N")
    };
    let pair = if lat_like {
        format!("{u} {other}")
    } else {
        format!("{other} {u}")
    };
    parse_coords(&pair).map(|(a, b)| if lat_like { a } else { b })
}

fn infer_platform(r: &mut Report) {
    if r.platform != Platform::NotStated {
        return;
    }
    r.platform = match r.subject_id_type.as_deref() {
        Some("imo") | Some("mmsi") => Platform::Vessel,
        Some("registration") | Some("icao24") => Platform::Aircraft,
        _ if r.flight_or_voyage.is_some() || r.altitude.is_some() => Platform::Aircraft,
        _ if r.reporter_role == ReporterRole::Mariner => Platform::Vessel,
        _ if matches!(r.reporter_role, ReporterRole::Pilot | ReporterRole::Atc) => {
            Platform::Aircraft
        }
        _ => Platform::NotStated,
    };
}

fn csv_reports(content: &str, src: &dyn Fn(usize) -> Source) -> Result<Vec<Report>, String> {
    let recs = text::csv_records(content);
    let Some(head) = recs.first() else {
        return Err("empty CSV".into());
    };
    let head = text::csv_fields(head);
    if head.len() < 2 {
        return Err("CSV header has fewer than two columns".into());
    }
    Ok(recs[1..]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let f = text::csv_fields(line);
            let kv: Vec<(String, String)> = head
                .iter()
                .zip(f.iter())
                .map(|(h, v)| (h.clone(), v.clone()))
                .collect();
            from_kv(&kv, src(i + 1))
        })
        .collect())
}

/// One NOTAM's items.
#[derive(Debug, Default)]
struct Notam {
    id: Option<String>,
    items: BTreeMap<char, String>,
}

fn notam_records(content: &str) -> Vec<Notam> {
    let mut out = vec![];
    let mut block = String::new();
    let flush = |block: &mut String, out: &mut Vec<Notam>| {
        if block.contains("Q)") || block.contains("E)") {
            out.push(parse_notam(block));
        }
        block.clear();
    };
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let starts_new = t.split_whitespace().next().is_some_and(|w| {
            w.contains('/')
                && w.len() <= 12
                && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        }) && t.contains("NOTAM");
        if t.is_empty() || starts_new {
            flush(&mut block, &mut out);
        }
        block.push_str(t);
        block.push('\n');
    }
    flush(&mut block, &mut out);
    out
}

fn parse_notam(block: &str) -> Notam {
    let mut n = Notam::default();
    let first = block.lines().next().unwrap_or("");
    if first.contains("NOTAM") {
        n.id = first.split_whitespace().next().map(String::from);
    }
    // Item markers in order: Q) A) B) C) D) E) F) G).
    let flat = block.replace('\n', " ");
    let mut marks: Vec<(usize, char)> = vec![];
    for c in ['Q', 'A', 'B', 'C', 'D', 'E', 'F', 'G'] {
        let pat = format!("{c})");
        let mut from = 0;
        while let Some(p) = flat[from..].find(&pat) {
            let at = from + p;
            let ok = at == 0 || flat.as_bytes()[at - 1] == b' ';
            if ok {
                marks.push((at, c));
                break;
            }
            from = at + pat.len();
        }
    }
    marks.sort();
    for (i, (p, c)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map_or(flat.len(), |m| m.0);
        n.items.insert(*c, flat[p + 2..end].trim().to_string());
    }
    n
}

/// `2606140000` (YYMMDDhhmm) to Unix seconds.
fn notam_time(s: &str) -> Option<f64> {
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).take(10).collect();
    if d.len() != 10 {
        return None;
    }
    let y: i64 = 2000 + d[0..2].parse::<i64>().ok()?;
    let date = (y, d[2..4].parse().ok()?, d[4..6].parse().ok()?);
    let sod = text::parse_time_of_day(&d[6..10])?;
    text::parse_date(&format!("{:04}-{:02}-{:02}", date.0, date.1, date.2))?;
    Some(text::at(date, sod))
}

fn from_notam(n: &Notam, source: Source) -> Report {
    let mut r = new_report(source);
    r.reporter_role = ReporterRole::Notam;
    r.platform = Platform::Area;
    if let Some(id) = &n.id {
        r.subject_id = Some(id.clone());
        r.subject_id_type = Some("notam".into());
    }
    let q = n.items.get(&'Q').cloned().unwrap_or_default();
    let qf: Vec<&str> = q.split('/').map(str::trim).collect();
    if let Some((la, lo, rad)) = qf.last().and_then(|p| text::parse_q_position(p)) {
        r.position = Some(Position {
            lat_deg: la,
            lon_deg: lo,
            radius_m: Some(rad),
            text: qf.last().map(|s| s.to_string()).unwrap_or_default(),
        });
    }
    if let Some(a) = n.items.get(&'A') {
        r.extra.insert("location_indicator".into(), a.clone());
    }
    if let Some(t) = n.items.get(&'B').and_then(|b| notam_time(b)) {
        r.occurred_start_utc = Some(iso(t));
    }
    match n.items.get(&'C') {
        Some(c) if c.contains("PERM") => r.notes.push("NOTAM is permanent (C) PERM)".into()),
        Some(c) => {
            if let Some(t) = notam_time(c) {
                r.occurred_end_utc = Some(iso(t));
            }
            if c.contains("EST") {
                r.notes.push("NOTAM end is estimated (EST)".into());
            }
        }
        None => {}
    }
    if let Some(d) = n.items.get(&'D') {
        r.extra.insert("schedule".into(), d.clone());
    }
    let e = n.items.get(&'E').cloned().unwrap_or_default();
    r.narrative = Some(e.clone());
    r.reported_type = text::classify(&e);
    r.kind = r.reported_type.kind();
    r.systems_affected = text::systems(&e);
    let band = match (n.items.get(&'F'), n.items.get(&'G')) {
        (Some(f), Some(g)) => {
            let lo = text::parse_altitude(f).and_then(|x| x.0);
            let hi = text::parse_altitude(g).and_then(|x| x.1);
            Some((lo, hi, format!("{f} - {g}")))
        }
        _ => {
            // Q-line lower and upper flight levels.
            let fl = |s: Option<&&str>| s.and_then(|v| v.parse::<f64>().ok()).map(|v| v * 100.0);
            let (lo, hi) = (fl(qf.get(5)), fl(qf.get(6)));
            (lo.is_some() || hi.is_some()).then(|| (lo, hi, format!("Q-line {lo:?}-{hi:?} ft")))
        }
    };
    if let Some((lo, hi, t)) = band {
        r.altitude = Some(Altitude {
            lower_ft: lo,
            upper_ft: hi,
            reference: "FL".into(),
            text: t,
        });
    }
    r.time_uncertainty_s = 0.0;
    r.notes.push(
        "NOTAM: an announcement of expected or ongoing interference, not a crew report; \
         used as context, never as an episode member"
            .into(),
    );
    r
}

/// The date and time at the start of a ship log line, and the rest of the line.
fn ship_entry_head(line: &str) -> Option<(f64, &str)> {
    let t = line.trim();
    let mut it = t.splitn(3, char::is_whitespace);
    let d = it.next()?;
    let tm = it.next()?;
    let rest = it.next().unwrap_or("");
    let date = text::parse_date(d)?;
    let sod = text::parse_time_of_day(tm)?;
    Some((text::at(date, sod), rest))
}

fn ship_log(content: &str, src: &dyn Fn(usize) -> Source) -> Vec<Report> {
    let mut header = String::new();
    let mut entries: Vec<(f64, String)> = vec![];
    for line in content.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((t, rest)) = ship_entry_head(line) {
            entries.push((t, rest.trim().to_string()));
        } else if (line.starts_with(' ') || line.starts_with('\t')) && !entries.is_empty() {
            let last = entries.last_mut().expect("checked");
            last.1.push(' ');
            last.1.push_str(line.trim());
        } else {
            header.push_str(line);
            header.push(' ');
        }
    }
    let hw: Vec<&str> = header.split_whitespace().collect();
    let after = |key: &str| {
        hw.iter()
            .position(|w| w.trim_end_matches(':').eq_ignore_ascii_case(key))
            .and_then(|i| hw.get(i + 1))
            .map(|s| {
                s.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                    .to_string()
            })
    };
    let (imo, mmsi) = (after("IMO"), after("MMSI"));
    let vessel_name = header
        .split("IMO")
        .next()
        .map(|s| s.trim().trim_end_matches(',').trim().to_string())
        .filter(|s| !s.is_empty());
    entries
        .into_iter()
        .enumerate()
        .map(|(i, (t, rest))| {
            let mut r = new_report(src(i + 1));
            r.reporter_role = ReporterRole::Mariner;
            r.platform = Platform::Vessel;
            if let Some(m) = &mmsi {
                set_subject(&mut r, m, "mmsi");
            }
            if let Some(m) = &imo {
                set_subject(&mut r, m, "imo");
            }
            if let Some(v) = &vessel_name {
                r.extra.insert("vessel_name".into(), v.clone());
            }
            r.occurred_start_utc = Some(iso(t));
            // Position: the leading run of coordinate characters.
            let (pos_text, narrative) = split_position(&rest);
            if let Some((la, lo)) = parse_coords(&pos_text) {
                r.position = Some(Position {
                    lat_deg: la,
                    lon_deg: lo,
                    radius_m: None,
                    text: pos_text.trim().to_string(),
                });
            } else {
                r.notes.push("no position at the start of the entry".into());
            }
            let narrative = narrative.trim().to_string();
            r.reported_type = text::classify(&narrative);
            r.kind = r.reported_type.kind();
            r.systems_affected = text::systems(&narrative);
            if r.kind == Kind::Spoofing {
                r.spoofed_layer = text::spoofed_layer(&narrative);
            }
            r.time_to_recover_min =
                text::recovery_minutes(&narrative).or_else(|| recovery_clock(&narrative, t));
            if r.time_to_recover_min.is_some() {
                r.notes.push("time to recover read from the entry".into());
            }
            r.narrative = Some(narrative);
            r.notes
                .push("ship log: type and systems read from the entry text".into());
            r
        })
        .collect()
}

/// Split "59°20.5'N 024°05.1'E GPS position jumped ..." into the position and the text.
fn split_position(s: &str) -> (String, String) {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let mut n = 0;
    for tok in &toks {
        let letters = tok.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let coordish = tok
            .chars()
            .all(|c| c.is_ascii_digit() || "°'\".,NSEW-′″".contains(c));
        if coordish && (letters <= 1) {
            n += 1;
        } else {
            break;
        }
    }
    (
        toks[..n].join(" "),
        toks[n..]
            .join(" ")
            .trim_start_matches(['|', '-', ':'])
            .to_string(),
    )
}

/// "Normal 04:40Z" / "restored 0440Z" after the entry time: minutes to that clock time.
fn recovery_clock(narrative: &str, t: f64) -> Option<f64> {
    let l = narrative.to_ascii_lowercase();
    let ws: Vec<&str> = l.split_whitespace().collect();
    for (i, w) in ws.iter().enumerate() {
        if ["normal", "restored", "recovered", "regained", "resumed"]
            .iter()
            .any(|k| w.starts_with(k))
        {
            for cand in ws.iter().skip(i + 1).take(3) {
                let c = cand.trim_matches(|ch: char| !(ch.is_ascii_alphanumeric() || ch == ':'));
                if !(c.ends_with('z') || c.contains(':')) {
                    continue;
                }
                if let Some(sod) = text::parse_time_of_day(c) {
                    let mut te = text::at(text::date_of(t), sod);
                    if te < t {
                        te += 86_400.0;
                    }
                    return Some(((te - t) / 60.0).round());
                }
            }
        }
    }
    None
}

/// Re-exported for the report helpers.
pub(crate) fn parse_utc(s: &str) -> Option<f64> {
    text::parse_utc(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dgca_form_fields() {
        let f = "Event type: Spoofing\nDate of occurrence (UTC): 2026-11-12\nTime of occurrence (UTC): 0542Z\n\
                 Time of report (UTC): 05:49\nAircraft registration: VT-ABC\nPosition: 31 42N 074 48E\nFlight level / altitude: FL350\n\
                 Equipment affected: GPS1, GPS2, FMS\nFMS screenshots: attached\nLogs preserved: yes\nNarrative: Position jumped 60 nm.\n  GPS restored after 12 minutes.\n";
        let r = &normalise(f, "f.txt", None).unwrap()[0];
        assert_eq!(r.source.format, SourceFormat::Dgca);
        assert_eq!(r.kind, Kind::Spoofing);
        assert_eq!(
            r.occurred_start_utc.as_deref(),
            Some("2026-11-12T05:42:00Z")
        );
        assert_eq!(r.reported_utc.as_deref(), Some("2026-11-12T05:49:00Z"));
        assert_eq!(r.platform, Platform::Aircraft);
        assert_eq!(r.time_to_recover_min, Some(12.0));
        assert_eq!(r.altitude.as_ref().unwrap().lower_ft, Some(35_000.0));
        assert_eq!(r.spoofed_layer, Layer::Gnss);
        assert_eq!(r.fms_screenshot, Some(true));
    }

    #[test]
    fn position_split_from_a_ship_log_line() {
        let (p, n) = split_position("59°20.5'N 024°05.1'E GPS position jumped");
        assert_eq!(p, "59°20.5'N 024°05.1'E");
        assert_eq!(n, "GPS position jumped");
        assert_eq!(
            recovery_clock(
                "normal 04:40Z",
                text::parse_utc("2026-06-14T04:12:00Z").unwrap()
            ),
            Some(28.0)
        );
    }
}
