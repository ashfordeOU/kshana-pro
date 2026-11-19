// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Text readers shared by the report normaliser: dates and times, coordinates in the forms
//! crews write them, altitudes, the interference type and the affected systems in free
//! text, durations, yes/no answers and CSV rows. Pure functions; nothing here panics on bad
//! input.

use kshana::interference_map::time::days_from_civil;

use super::{Layer, ReportedType, ReporterRole};

pub use crate::forensics::inputs::{iso, round};

/// ISO 8601 UTC (or Unix seconds) to Unix seconds; fractional seconds accepted, and a time
/// with no seconds (`2026-11-12T05:42Z`).
pub fn parse_utc(s: &str) -> Option<f64> {
    let s = s.trim();
    crate::forensics::inputs::parse_utc(s).or_else(|| {
        let core = s.strip_suffix('Z').unwrap_or(s);
        let (d, t) = core.split_once(['T', ' '])?;
        if t.len() == 5 && t.as_bytes()[2] == b':' {
            crate::forensics::inputs::parse_utc(&format!("{d}T{t}:00Z"))
        } else {
            None
        }
    })
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// A calendar date: `2026-11-12`, `2026/11/12`, `12/11/2026` (day first), `12 NOV 2026`,
/// `12-Nov-2026`, `20261112`. Returns (year, month, day).
pub fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    let s = s.trim().trim_end_matches(['Z', ',']).trim();
    let ok = |y: i64, m: u32, d: u32| {
        ((2000..2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d))
            .then_some((y, m, d))
    };
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        return ok(
            s[0..4].parse().ok()?,
            s[4..6].parse().ok()?,
            s[6..8].parse().ok()?,
        );
    }
    let parts: Vec<&str> = s
        .split(|c: char| c == '-' || c == '/' || c == '.' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let month_name = |p: &str| {
        let u = p.to_ascii_uppercase();
        MONTHS
            .iter()
            .position(|m| u.starts_with(m))
            .map(|i| i as u32 + 1)
    };
    if parts[0].len() == 4 {
        let m = parts[1].parse().ok().or_else(|| month_name(parts[1]))?;
        return ok(parts[0].parse().ok()?, m, parts[2].parse().ok()?);
    }
    let m = parts[1].parse().ok().or_else(|| month_name(parts[1]))?;
    let y: i64 = parts[2].parse().ok()?;
    let y = if y < 100 { 2000 + y } else { y };
    ok(y, m, parts[0].parse().ok()?)
}

/// A time of day: `05:42`, `0542`, `0542Z`, `05:42:30`, `05:42 UTC`. Returns seconds of day.
pub fn parse_time_of_day(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_uppercase();
    let s = s
        .trim_end_matches("UTC")
        .trim_end_matches("GMT")
        .trim()
        .trim_end_matches('Z')
        .trim();
    let (h, m, sec) = if s.contains(':') {
        let p: Vec<&str> = s.split(':').collect();
        (
            p.first()?.parse::<f64>().ok()?,
            p.get(1)?.parse::<f64>().ok()?,
            p.get(2).map_or(Some(0.0), |x| x.parse::<f64>().ok())?,
        )
    } else if (s.len() == 4 || s.len() == 6) && s.bytes().all(|b| b.is_ascii_digit()) {
        (
            s[0..2].parse().ok()?,
            s[2..4].parse().ok()?,
            if s.len() == 6 {
                s[4..6].parse().ok()?
            } else {
                0.0
            },
        )
    } else {
        return None;
    };
    ((0.0..24.0).contains(&h) && (0.0..60.0).contains(&m) && (0.0..61.0).contains(&sec))
        .then_some(h * 3600.0 + m * 60.0 + sec)
}

/// Unix seconds of a date at seconds of day.
pub fn at(date: (i64, u32, u32), sod: f64) -> f64 {
    days_from_civil(date.0, date.1, date.2) as f64 * 86_400.0 + sod
}

/// A date-time in one field: ISO 8601, or a date and a time separated by a space.
pub fn parse_datetime(s: &str) -> Option<f64> {
    if let Some(t) = parse_utc(s) {
        return Some(t);
    }
    let s = s.trim();
    // "2026-11-12 0542Z", "12 NOV 2026 05:42", "12/11/2026 05:42 UTC".
    let toks: Vec<&str> = s.split_whitespace().collect();
    for split in (1..toks.len()).rev() {
        let (d, t) = (toks[..split].join(" "), toks[split..].join(" "));
        if let (Some(d), Some(t)) = (parse_date(&d), parse_time_of_day(&t)) {
            return Some(at(d, t));
        }
    }
    None
}

/// The UTC day of a Unix time as (year, month, day).
pub fn date_of(t: f64) -> (i64, u32, u32) {
    kshana::interference_map::time::civil_from_days((t / 86_400.0).floor() as i64)
}

/// Seconds of day of a Unix time.
pub fn sod_of(t: f64) -> f64 {
    t - (t / 86_400.0).floor() * 86_400.0
}

fn hemisphere(c: char) -> Option<f64> {
    match c {
        'N' | 'E' => Some(1.0),
        'S' | 'W' => Some(-1.0),
        _ => None,
    }
}

/// Degrees from a compact ICAO group: `3142` (ddmm), `314230` (ddmmss), `07448` (dddmm),
/// `0744830` (dddmmss), with an optional decimal part on the last unit.
fn compact(digits: &str, deg_width: usize) -> Option<f64> {
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if !int.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let frac_v: f64 = if frac.is_empty() {
        0.0
    } else {
        format!("0.{frac}").parse().ok()?
    };
    let n = int.len();
    let d: f64 = int.get(0..deg_width)?.parse().ok()?;
    match n - deg_width {
        0 => Some(d + frac_v),
        2 => {
            let m: f64 = int[deg_width..].parse().ok()?;
            (m < 60.0).then_some(d + (m + frac_v) / 60.0)
        }
        4 => {
            let m: f64 = int[deg_width..deg_width + 2].parse().ok()?;
            let s: f64 = int[deg_width + 2..].parse().ok()?;
            (m < 60.0 && s < 60.0).then_some(d + m / 60.0 + (s + frac_v) / 3600.0)
        }
        _ => None,
    }
}

/// Degrees from one or more numbers (degrees; degrees and minutes; degrees, minutes and
/// seconds). One integer of four or more digits is read as a compact ICAO group.
fn group(nums: &[String], deg_width: usize) -> Option<f64> {
    match nums {
        [one] => {
            let int_len = one.split('.').next().unwrap_or("").len();
            if int_len > 3 {
                compact(one, deg_width)
            } else {
                one.parse().ok()
            }
        }
        [d, m] => {
            let (d, m): (f64, f64) = (d.parse().ok()?, m.parse().ok()?);
            (m < 60.0).then_some(d + m / 60.0)
        }
        [d, m, s] => {
            let (d, m, s): (f64, f64, f64) = (d.parse().ok()?, m.parse().ok()?, s.parse().ok()?);
            (m < 60.0 && s < 60.0).then_some(d + m / 60.0 + s / 3600.0)
        }
        _ => None,
    }
}

/// A latitude and longitude in the forms reports use: decimal (`31.70, 74.80`,
/// `31.70N 74.80E`, `N31.70 E074.80`), degrees and minutes (`59°20.5'N 024°05.1'E`,
/// `31 42N 074 48E`), with seconds, and compact ICAO groups (`3142N07448E`,
/// `314230N0744830E`).
pub fn parse_coords(s: &str) -> Option<(f64, f64)> {
    let mut t = String::with_capacity(s.len() + 8);
    for c in s.to_ascii_uppercase().chars() {
        match c {
            '°' | '\'' | '"' | '′' | '″' | ',' | ';' | '/' | '(' | ')' => t.push(' '),
            'N' | 'S' | 'E' | 'W' => {
                t.push(' ');
                t.push(c);
                t.push(' ');
            }
            c if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == ' ' => t.push(c),
            _ => t.push(' '),
        }
    }
    let toks: Vec<&str> = t.split_whitespace().collect();
    let mut groups: Vec<(Vec<String>, Option<f64>)> = vec![];
    let mut cur: Vec<String> = vec![];
    let mut pending_sign: Option<f64> = None;
    for tok in toks {
        if let Some(h) = tok
            .chars()
            .next()
            .filter(|_| tok.len() == 1)
            .and_then(hemisphere)
        {
            if cur.is_empty() {
                // Hemisphere before the numbers (N31.70).
                pending_sign = Some(h);
            } else {
                groups.push((std::mem::take(&mut cur), Some(h)));
            }
        } else if tok.parse::<f64>().is_ok() {
            if let Some(h) = pending_sign.take() {
                groups.push((vec![tok.to_string()], Some(h)));
            } else {
                cur.push(tok.to_string());
            }
        }
    }
    if !cur.is_empty() {
        groups.push((cur, None));
    }
    let valid = |lat: f64, lon: f64| {
        ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
    };
    // Two plain numbers: decimal latitude and longitude.
    if groups.len() == 1 && groups[0].1.is_none() && groups[0].0.len() == 2 {
        let lat: f64 = groups[0].0[0].parse().ok()?;
        let lon: f64 = groups[0].0[1].parse().ok()?;
        return valid(lat, lon);
    }
    let signed: Vec<&(Vec<String>, Option<f64>)> =
        groups.iter().filter(|g| g.1.is_some()).collect();
    if signed.len() < 2 {
        return None;
    }
    let lat = group(&signed[0].0, 2)? * signed[0].1?;
    let lon = group(&signed[1].0, 3)? * signed[1].1?;
    valid(lat, lon)
}

/// A NOTAM Q-line position and radius: `5440N02520E100` (radius in nautical miles).
pub fn parse_q_position(s: &str) -> Option<(f64, f64, f64)> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 11 {
        return None;
    }
    let lat = compact(&s[0..4], 2)? * hemisphere(b[4] as char)?;
    let lon = compact(&s[5..10], 3)? * hemisphere(b[10] as char)?;
    let r_nm: f64 = s
        .get(11..)
        .filter(|r| !r.is_empty())
        .map_or(Some(999.0), |r| r.parse().ok())?;
    Some((lat, lon, r_nm * 1852.0))
}

/// An altitude or band: `FL350`, `35,000 ft`, `35000FT AMSL`, `1800-2000 ft AGL`, `SFC`,
/// `600 m`. Returns (lower ft, upper ft, reference).
pub fn parse_altitude(s: &str) -> Option<(Option<f64>, Option<f64>, String)> {
    let u = s.trim().to_ascii_uppercase();
    if u.is_empty() {
        return None;
    }
    let reference = if u.contains("FL") {
        "FL"
    } else if u.contains("AGL") {
        "AGL"
    } else if u.contains("AMSL") || u.contains("MSL") || u.contains("ALT") {
        "AMSL"
    } else if u.contains("SFC") || u.contains("GND") {
        "SFC"
    } else {
        "unstated"
    };
    let metres = words(&u).iter().any(|w| {
        w == "m"
            || w == "metres"
            || w == "meters"
            || (w.ends_with('m')
                && w[..w.len() - 1].bytes().all(|b| b.is_ascii_digit())
                && w.len() > 1)
    });
    let clean: String = u
        .chars()
        .map(|c| if c == ',' { '\0' } else { c })
        .filter(|c| *c != '\0')
        .collect();
    let mut nums: Vec<f64> = vec![];
    let mut cur = String::new();
    for c in clean.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == '.' && !cur.is_empty()) {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f64>() {
                nums.push(v);
            }
            cur.clear();
        }
    }
    let fl = reference == "FL";
    let conv = |v: f64| {
        if fl {
            v * 100.0
        } else if metres {
            round(v / 0.3048, 0)
        } else {
            v
        }
    };
    let sfc = u.contains("SFC") || u.contains("GND");
    match (nums.as_slice(), sfc) {
        ([], true) => Some((Some(0.0), Some(0.0), "SFC".into())),
        ([], false) => None,
        ([v], true) => Some((Some(0.0), Some(conv(*v)), reference.into())),
        ([v], false) => Some((Some(conv(*v)), Some(conv(*v)), reference.into())),
        ([a, b, ..], _) => Some((Some(conv(*a)), Some(conv(*b)), reference.into())),
    }
}

/// Lower-case words of a text (letters, digits and inner hyphens).
/// A hyphenated word also yields its parts (`pilot-in-command` → `pilot`, `in`, `command`).
pub fn words(s: &str) -> Vec<String> {
    let mut out = vec![];
    for w in s
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .map(|w| w.trim_matches('-').to_string())
        .filter(|w| !w.is_empty())
    {
        if w.contains('-') {
            out.extend(w.split('-').filter(|p| !p.is_empty()).map(String::from));
        }
        out.push(w);
    }
    out
}

fn has_any(text: &str, ws: &[String], needles: &[&str]) -> bool {
    needles.iter().any(|n| {
        if n.contains(' ') {
            text.contains(n)
        } else if let Some(stem) = n.strip_suffix('*') {
            ws.iter().any(|w| w.starts_with(stem))
        } else {
            ws.iter().any(|w| w == n)
        }
    })
}

/// The interference type stated in a text. Explicit spoofing words win over jamming words
/// (a spoofed receiver often also loses lock); loss and integrity words come next.
pub fn classify(s: &str) -> ReportedType {
    let l = s.to_ascii_lowercase();
    let ws = words(&l);
    if has_any(
        &l,
        &ws,
        &[
            "spoof*",
            "meacon*",
            "false position",
            "position jump",
            "position shift",
            "position shifted",
            "position jumped",
            "erroneous position",
            "wrong position",
            "time jump",
        ],
    ) {
        ReportedType::Spoofing
    } else if has_any(&l, &ws, &["jam", "jamm*", "jammed"]) {
        ReportedType::Jamming
    } else if has_any(
        &l,
        &ws,
        &[
            "signal loss",
            "loss of gps",
            "loss of gnss",
            "loss of signal",
            "gps lost",
            "gnss lost",
            "lost gps",
            "lost gnss",
            "gps loss",
            "gnss loss",
            "gps unavailable",
            "gnss unavailable",
            "gps signal lost",
            "gnss signal lost",
            "no gps",
            "no gnss",
        ],
    ) {
        ReportedType::SignalLoss
    } else if has_any(&l, &ws, &["integrity", "raim", "integrity error"]) {
        ReportedType::IntegrityError
    } else if has_any(&l, &ws, &["interference", "rfi", "disruption"]) {
        ReportedType::InterferenceUnspecified
    } else if has_any(
        &l,
        &ws,
        &[
            "not avbl",
            "not be avbl",
            "may not be available",
            "unreliable",
            "unrel",
            "u/s",
            "not available",
        ],
    ) {
        ReportedType::AnnouncedOutage
    } else {
        ReportedType::NotStated
    }
}

/// Which layer a spoofing text names.
pub fn spoofed_layer(s: &str) -> Layer {
    let l = s.to_ascii_lowercase();
    let ws = words(&l);
    let ais = has_any(
        &l,
        &ws,
        &[
            "ais spoof",
            "false ais",
            "ais target",
            "ais position",
            "ais showed",
            "ais track",
            "ais only",
        ],
    ) || (has_any(&l, &ws, &["ais"]) && !has_any(&l, &ws, &["gps", "gnss", "receiver"]));
    let gnss = has_any(
        &l,
        &ws,
        &[
            "gps", "gnss", "galileo", "glonass", "beidou", "navic", "receiver", "dgps",
        ],
    ) && !has_any(&l, &ws, &["ais only"]);
    match (gnss, ais) {
        (true, true) => Layer::Both,
        (true, false) => Layer::Gnss,
        (false, true) => Layer::Ais,
        (false, false) => Layer::NotStated,
    }
}

/// The systems a text names, as normalised names, sorted and unique.
pub fn systems(s: &str) -> Vec<String> {
    let l = s.to_ascii_lowercase();
    let ws = words(&l);
    let table: &[(&str, &[&str])] = &[
        (
            "GNSS receiver",
            &[
                "gps", "gps1", "gps2", "gnss", "gnss1", "gnss2", "galileo", "glonass", "beidou",
                "navic", "mmr", "dgps", "gpssu",
            ],
        ),
        ("FMS", &["fms", "fmc", "fmgc"]),
        ("ADS-B", &["ads-b", "adsb"]),
        ("TAWS/EGPWS", &["egpws", "taws", "gpws", "terrain"]),
        ("Autopilot", &["autopilot"]),
        ("Inertial (IRS/INS)", &["irs", "adiru", "ins", "inertial"]),
        ("RNP/RNAV", &["rnp", "rnav"]),
        ("Approach guidance (GLS/LPV/SBAS)", &["gls", "lpv", "sbas"]),
        (
            "Clock/time",
            &["clock", "utc time", "time reference", "time jump"],
        ),
        ("EFB", &["efb"]),
        ("AIS", &["ais"]),
        ("ECDIS", &["ecdis"]),
        ("Radar", &["radar", "arpa"]),
        ("Gyro", &["gyro", "gyrocompass"]),
        ("Dynamic positioning", &["dynamic positioning", "dp system"]),
        ("VDR", &["vdr"]),
    ];
    let mut out: Vec<String> = table
        .iter()
        .filter(|(_, needles)| has_any(&l, &ws, needles))
        .map(|(n, _)| n.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A reporter role named in a text.
pub fn role(s: &str) -> ReporterRole {
    let l = s.to_ascii_lowercase();
    let ws = words(&l);
    if has_any(
        &l,
        &ws,
        &[
            "pilot",
            "captain",
            "pic",
            "first officer",
            "flight crew",
            "commander",
            "f/o",
            "fo",
        ],
    ) {
        ReporterRole::Pilot
    } else if has_any(
        &l,
        &ws,
        &[
            "atc",
            "atco",
            "controller",
            "ansp",
            "tower",
            "approach control",
        ],
    ) {
        ReporterRole::Atc
    } else if has_any(
        &l,
        &ws,
        &[
            "master",
            "mariner",
            "officer of the watch",
            "oow",
            "bridge",
            "chief officer",
            "navigator",
        ],
    ) {
        ReporterRole::Mariner
    } else if has_any(&l, &ws, &["authority", "caa", "nca", "dgca", "regulator"]) {
        ReporterRole::Authority
    } else if has_any(&l, &ws, &["crew", "cabin"]) {
        ReporterRole::Crew
    } else {
        ReporterRole::NotStated
    }
}

/// A yes/no answer.
pub fn yes_no(s: &str) -> Option<bool> {
    let l = s.trim().to_ascii_lowercase();
    let ws = words(&l);
    if ws.iter().any(|w| {
        matches!(
            w.as_str(),
            "no" | "n" | "false" | "not" | "none" | "nil" | "0"
        )
    }) {
        Some(false)
    } else if ws.iter().any(|w| {
        matches!(
            w.as_str(),
            "yes"
                | "y"
                | "true"
                | "attached"
                | "preserved"
                | "1"
                | "saved"
                | "retained"
                | "downloaded"
        )
    }) {
        Some(true)
    } else {
        None
    }
}

/// A duration in minutes: `12`, `12 min`, `1.5 h`, `90 s`, `00:12`.
pub fn minutes(s: &str) -> Option<f64> {
    let l = s.trim().to_ascii_lowercase();
    if let Some((h, m)) = l.split_once(':') {
        if let (Ok(h), Ok(m)) = (h.trim().parse::<f64>(), m.trim().parse::<f64>()) {
            return Some(h * 60.0 + m);
        }
    }
    let num: String = l
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let v: f64 = num.parse().ok()?;
    let rest = l
        .split_once(num.as_str())
        .map(|x| x.1)
        .unwrap_or("")
        .trim_start();
    Some(if rest.starts_with('h') {
        v * 60.0
    } else if rest.starts_with('s') {
        v / 60.0
    } else {
        v
    })
}

/// A duration stated in a narrative near a recovery word: "restored after 12 minutes",
/// "recovered 25 min later", "normal after 1 hour".
pub fn recovery_minutes(narrative: &str) -> Option<f64> {
    let l = narrative.to_ascii_lowercase();
    for sentence in l.split(['.', ';', '\n']) {
        if ![
            "recover", "restor", "regain", "normal", "returned", "back to",
        ]
        .iter()
        .any(|k| sentence.contains(k))
        {
            continue;
        }
        let ws: Vec<&str> = sentence.split_whitespace().collect();
        for w in ws.windows(2) {
            let (n, u) = (w[0], w[1]);
            if let Ok(v) = n.parse::<f64>() {
                if u.starts_with("min") {
                    return Some(v);
                }
                if u.starts_with("hour") || u == "h" || u == "hr" || u == "hrs" {
                    return Some(v * 60.0);
                }
            }
        }
    }
    None
}

/// A report key, normalised: lower case, punctuation to single spaces.
pub fn norm_key(k: &str) -> String {
    k.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Split one CSV line (RFC 4180 quoting; a doubled quote inside quotes is a quote).
pub fn csv_fields(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, q) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            ('"', _) => q = !q,
            (',', false) => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// Split CSV text into records, joining lines while a quote is open (a narrative with a
/// line break inside quotes).
pub fn csv_records(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for line in text.lines() {
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(line);
        if cur.matches('"').count().is_multiple_of(2) {
            if !cur.trim().is_empty() && !cur.trim_start().starts_with('#') {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    #[test]
    fn coordinates_in_report_forms() {
        let want = (31.7, 74.8);
        for s in [
            "31.70, 74.80",
            "31.70N 74.80E",
            "N31.70 E74.80",
            "31 42N 074 48E",
            "31°42'N 074°48'E",
            "3142N07448E",
            "314200N0744800E",
        ] {
            assert!(close(parse_coords(s).unwrap(), want), "{s}");
        }
        assert!(close(
            parse_coords("59°20.5'N 024°05.1'E").unwrap(),
            (59.0 + 20.5 / 60.0, 24.0 + 5.1 / 60.0)
        ));
        assert!(close(
            parse_coords("33 30S 070 40W").unwrap(),
            (-33.5, -70.0 - 40.0 / 60.0)
        ));
        assert_eq!(parse_coords("over Amritsar"), None);
        let (la, lo, r) = parse_q_position("5440N02520E100").unwrap();
        assert!(close((la, lo), (54.0 + 40.0 / 60.0, 25.0 + 20.0 / 60.0)));
        assert!((r - 185_200.0).abs() < 1e-6);
    }

    #[test]
    fn dates_times_and_altitudes() {
        assert_eq!(parse_date("2026-11-12"), Some((2026, 11, 12)));
        assert_eq!(parse_date("12 NOV 2026"), Some((2026, 11, 12)));
        assert_eq!(parse_date("12/11/2026"), Some((2026, 11, 12)));
        assert_eq!(parse_time_of_day("0542Z"), Some(5.0 * 3600.0 + 42.0 * 60.0));
        assert_eq!(
            parse_time_of_day("05:42 UTC"),
            Some(5.0 * 3600.0 + 42.0 * 60.0)
        );
        let t = parse_datetime("2026-11-12 0542Z").unwrap();
        assert_eq!(iso(t), "2026-11-12T05:42:00Z");
        assert_eq!(parse_utc("2026-11-12T05:42Z"), Some(t));
        assert_eq!(parse_altitude("FL350").unwrap().0, Some(35_000.0));
        assert_eq!(parse_altitude("35,000 ft").unwrap().0, Some(35_000.0));
        let b = parse_altitude("1800-2000 ft AGL").unwrap();
        assert_eq!(
            (b.0, b.1, b.2.as_str()),
            (Some(1800.0), Some(2000.0), "AGL")
        );
        assert_eq!(parse_altitude("SFC").unwrap().1, Some(0.0));
    }

    #[test]
    fn type_systems_and_answers() {
        assert_eq!(
            classify("GPS spoofing, position jumped 60 nm"),
            ReportedType::Spoofing
        );
        assert_eq!(classify("GNSS jamming suspected"), ReportedType::Jamming);
        assert_eq!(classify("Signal loss"), ReportedType::SignalLoss);
        assert_eq!(classify("RAIM alert"), ReportedType::IntegrityError);
        assert_eq!(
            classify("GNSS MAY NOT BE AVBL"),
            ReportedType::AnnouncedOutage
        );
        assert_eq!(classify("routine flight"), ReportedType::NotStated);
        assert_eq!(
            systems("GPS1, GPS2, FMS; EGPWS terrain warning; ADS-B"),
            vec!["ADS-B", "FMS", "GNSS receiver", "TAWS/EGPWS"]
        );
        assert_eq!(
            spoofed_layer("AIS showed own ship ashore, GPS jumped"),
            Layer::Both
        );
        assert_eq!(spoofed_layer("false AIS targets only"), Layer::Ais);
        assert_eq!(yes_no("Yes, attached"), Some(true));
        assert_eq!(yes_no("not preserved"), Some(false));
        assert_eq!(minutes("1.5 h"), Some(90.0));
        assert_eq!(minutes("12 min"), Some(12.0));
        assert_eq!(
            recovery_minutes("GPS restored after 25 minutes."),
            Some(25.0)
        );
        assert_eq!(role("Captain (PIC)"), ReporterRole::Pilot);
        assert_eq!(role("Approach controller"), ReporterRole::Atc);
        assert_eq!(csv_fields(r#"a,"b, c",""""#), vec!["a", "b, c", "\""]);
    }
}
