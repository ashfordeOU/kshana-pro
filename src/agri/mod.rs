// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Agriculture, construction and survey operations (research 2026-11-20 gaps, epic E35)
//!
//! Tools for machines that steer or grade by GNSS (tractors, sprayers, forestry harvesters,
//! construction machine control) and for the people who check their work:
//!
//! * [`log`]: machine logs: a neutral CSV (time, position, fix quality, satellites, C/N0,
//!   correction age, steering engaged, cross-track error) and NMEA 0183 `GGA` sentences.
//! * [`lines`]: guidance lines (A-B and curves) in a neutral form, with converters between
//!   GeoJSON, KML, an ISO 11783-10 (ISOXML) guidance-pattern subset, CSV and an A point plus a
//!   heading; and **guidance-line realignment** after an outage or a correction-source change
//!   (the shift that puts stored lines back on the rows actually planted, from a pass driven
//!   on existing rows and/or re-measured markers, with its uncertainty).
//! * [`drift`]: **pass-to-pass drift** from a log against an A-B line: within-pass drift
//!   (a pass that starts right and slowly walks off), adjacent-pass spacing error, and the
//!   overlap and miss area it implies for the implement width.
//! * [`resilience`]: the **autosteer GNSS-loss resilience score** from a log (availability of
//!   the fix the work needs, recovery time, steering continuity, accuracy while degraded), and
//!   an indicative **outage diagnosis** per episode: interference-like, correction-feed-like or
//!   ionosphere-like, from the signatures in the log. No emitter is located.
//! * [`field`]: the **field planner**: parallel swath lines over a field boundary for a
//!   heading and a width, with the working time and, from an advisory, the hours to do it in.
//! * [`spaceweather`]: Kp from the GFZ file and NOAA SWPC products, the NOAA G scale, a
//!   **scintillation advisor** and a **PPP-versus-RTK advisor** per field and hour, and a
//!   plain-language translation for growers. These run locally; a hosted service could add
//!   live ionospheric maps and regionally calibrated thresholds (see the docs page).
//! * [`photo`]: consistency checks of **geotagged photos** submitted as evidence of field work
//!   (for example under an agricultural subsidy scheme): EXIF position against the parcel,
//!   GPS time against camera time, repeated or rounded coordinates, impossible travel.
//!
//! **Evidence label: MODELLED.** Thresholds and weights are named parameters with stated
//! defaults; nothing here has been checked against a machine maker's own figures, and no
//! output is a statement that a machine, a receiver or a claim meets any standard or rule.
//! The space-weather backtest uses the published GFZ Kp series for May 2024 and compares the
//! advisory with the dates of reported farm outages only (qualitative).
//!
//! **Out of scope by design:** locating an interference source, steering or control of a
//! machine, and any fusion of GNSS with other sensors.

pub mod drift;
pub mod field;
pub mod geo;
pub mod lines;
pub mod log;
pub mod photo;
pub mod resilience;
pub mod sample;
pub mod spaceweather;

/// Evidence label of every output of this module.
pub const LABEL: &str = "MODELLED";

/// Pass-to-pass drift, guidance-line realignment and the field planner (Pro).
pub const FEATURE_GUIDANCE: &str = "pro-agri-guidance";
/// The autosteer resilience score and the outage diagnosis (Pro).
pub const FEATURE_RESILIENCE: &str = "pro-agri-resilience";
/// The scintillation and PPP-versus-RTK advisors (Pro).
pub const FEATURE_ADVISOR: &str = "pro-agri-advisor";
/// Geotagged-photo consistency checks over a parcel set (Enterprise).
pub const FEATURE_PHOTO: &str = "pro-agri-photo";

/// Percentile (nearest rank on a sorted copy) of `v`, `p` in 0..=100. `None` when empty.
pub(crate) fn percentile(v: &[f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let rank = ((p / 100.0) * s.len() as f64).ceil() as usize;
    Some(s[rank.clamp(1, s.len()) - 1])
}

/// Root mean square of `v` (0 when empty).
pub(crate) fn rms(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}

/// Round to `d` decimals for stable JSON.
pub(crate) fn round(x: f64, d: i32) -> f64 {
    let f = 10f64.powi(d);
    (x * f).round() / f
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date (y, m, d) of a day count since 1970-01-01.
pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Unix seconds of an ISO 8601 UTC time `YYYY-MM-DDTHH:MM[:SS][Z]` or a date `YYYY-MM-DD`.
pub fn parse_utc(s: &str) -> Result<i64, String> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = match s.split_once(['T', ' ']) {
        Some((d, t)) => (d, t),
        None => (s, "00:00:00"),
    };
    let dp: Vec<&str> = date.split(['-', ':']).collect();
    let tp: Vec<&str> = time.split(':').collect();
    let num = |x: Option<&&str>| -> Result<i64, String> {
        x.map(|v| v.trim())
            .unwrap_or("0")
            .split('.')
            .next()
            .unwrap_or("0")
            .parse::<i64>()
            .map_err(|_| format!("'{s}': not an ISO 8601 UTC time"))
    };
    if dp.len() != 3 {
        return Err(format!("'{s}': not an ISO 8601 UTC time"));
    }
    let (y, m, d) = (num(dp.first())?, num(dp.get(1))?, num(dp.get(2))?);
    if !(1..=12).contains(&m) || d < 1 || d > kshana_eloran::time::days_in_month(y, m) {
        return Err(format!("'{s}': date out of range"));
    }
    let (hh, mm, ss) = (num(tp.first())?, num(tp.get(1))?, num(tp.get(2))?);
    Ok(days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// ISO 8601 UTC text of unix seconds.
pub fn utc_text(t: i64) -> String {
    let (y, m, d) = civil_from_days(t.div_euclid(86_400));
    let s = t.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

/// A `lat,lon` pair in degrees.
pub fn parse_latlon(s: &str) -> Result<(f64, f64), String> {
    let (a, b) = s
        .split_once(',')
        .ok_or_else(|| format!("'{s}': expected lat,lon in degrees"))?;
    let lat: f64 = a
        .trim()
        .parse()
        .map_err(|_| format!("'{s}': bad latitude"))?;
    let lon: f64 = b
        .trim()
        .parse()
        .map_err(|_| format!("'{s}': bad longitude"))?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=360.0).contains(&lon) {
        return Err(format!("'{s}': out of range"));
    }
    Ok((lat, lon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip_and_utc() {
        for z in [-1000i64, 0, 19_000, 19_853, 20_500] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        let t = parse_utc("2024-05-10T18:00:00Z").unwrap();
        assert_eq!(utc_text(t), "2024-05-10T18:00:00Z");
        assert_eq!(parse_utc("2024-05-10").unwrap() + 18 * 3600, t);
        assert!(parse_utc("2024-13-10").is_err());
    }

    #[test]
    fn stats() {
        assert_eq!(percentile(&[3.0, 1.0, 2.0, 4.0], 50.0), Some(2.0));
        assert_eq!(percentile(&[3.0, 1.0, 2.0, 4.0], 95.0), Some(4.0));
        assert!((rms(&[3.0, 4.0]) - (12.5f64).sqrt()).abs() < 1e-12);
    }
}
