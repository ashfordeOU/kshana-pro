// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! B2003: corrupt-data tolerance vectors for the receiver-to-OBC interface.
//!
//! A COTS receiver on a small spacecraft often talks to the on-board computer in NMEA 0183.
//! A radiation upset in the receiver, on the line or in a buffer reaches the OBC as a corrupt
//! sentence; the OBC must not take a bad position or time into its navigation, attitude or
//! time-tagging. This module builds a replay stream from a trace: nominal GGA and RMC
//! sentences per epoch with **corrupt vectors** interleaved, one class at a time, each with
//! the actions an OBC may take (`accept`, `flag`, `reject`, `ignore`):
//!
//! | Class | The vector | Allowed |
//! |---|---|---|
//! | `checksum_bitflip` | one payload bit flipped, checksum kept | reject |
//! | `checksum_invalid_hex` | checksum characters not hexadecimal | reject |
//! | `truncated` | sentence cut before its checksum | reject |
//! | `missing_terminator` | two sentences run together | reject |
//! | `overlong` | more than 82 characters | reject |
//! | `non_printable` | a control byte inside the sentence | reject |
//! | `garbage` | bytes that are not a sentence | reject |
//! | `empty_fields` | position fields empty, valid checksum, fix claimed | reject, flag |
//! | `out_of_range` | latitude above 90 degrees, valid checksum | reject |
//! | `no_fix` | fix quality 0 or status V, valid checksum | flag, reject |
//! | `duplicate` | the previous sentence again | flag, reject |
//! | `time_backwards` | an older sentence again | flag, reject |
//! | `position_jump` | a field changed with the checksum recomputed: off the orbit | flag, reject |
//! | `altitude_implausible` | altitude below the orbit floor, valid checksum | flag, reject |
//! | `unknown_sentence` | a sentence type the OBC does not use | ignore, reject |
//! | `nominal` | the nominal sentences around them | accept |
//!
//! The reference parser ([`Reference`]) applies those rules and is run over every stream
//! built, so the expected actions are checked to be achievable. An OBC's log (`id,action`
//! per vector) is scored: met when its action is allowed, not met otherwise, not assessed
//! when the vector is missing from the log. Test planning and evidence only; MODELLED.

use std::collections::BTreeMap;

use kshana::frames::{ecef_to_geodetic, geodetic_to_ecef, Geodetic};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::see::Epoch;
use super::{Check, LABEL};

pub const VECTORS_SCHEMA: &str = "kshana-space-rx-obc-vectors/1";
pub const SCORE_SCHEMA: &str = "kshana-space-rx-obc-score/1";
/// The NMEA 0183 sentence length limit (from `$` to the end of the checksum), characters.
pub const MAX_LEN: usize = 82;
/// GPS - UTC, seconds (the leap-second count used to turn time of week into UTC).
pub const GPS_UTC_S: f64 = 18.0;

/// One vector of the stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub id: String,
    pub class: String,
    /// The bytes as hex (exactly what is sent, without the CR LF terminator).
    pub hex: String,
    pub allowed: Vec<String>,
    pub note: String,
}

/// Plausibility limits of the reference parser.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub min_alt_m: f64,
    pub max_alt_m: f64,
    pub max_speed_mps: f64,
    pub jump_margin_m: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min_alt_m: 100_000.0,
            max_alt_m: 50_000_000.0,
            max_speed_mps: 11_000.0,
            jump_margin_m: 2_000.0,
        }
    }
}

/// NMEA checksum of the text between `$` and `*`.
pub fn checksum(body: &str) -> u8 {
    body.bytes().fold(0u8, |a, b| a ^ b)
}

fn sentence(body: &str) -> String {
    format!("${body}*{:02X}", checksum(body))
}

fn hms(tow: f64) -> String {
    let t = (tow - GPS_UTC_S).rem_euclid(86_400.0);
    let h = (t / 3600.0).floor();
    let m = ((t - h * 3600.0) / 60.0).floor();
    let s = t - h * 3600.0 - m * 60.0;
    format!("{:02}{:02}{:05.2}", h as u32, m as u32, s)
}

fn ll(lat: f64, lon: f64) -> (String, String) {
    let f = |v: f64, w: usize| {
        let a = v.abs();
        let d = a.floor();
        format!("{:0w$}{:07.4}", d as u32, (a - d) * 60.0, w = w)
    };
    (
        format!("{},{}", f(lat, 2), if lat >= 0.0 { "N" } else { "S" }),
        format!("{},{}", f(lon, 3), if lon >= 0.0 { "E" } else { "W" }),
    )
}

/// The GGA body of an epoch (`fix` quality as given).
pub fn gga_body(e: &Epoch, fix: u8) -> String {
    let g = ecef_to_geodetic(e.r);
    let (la, lo) = ll(g.lat_rad.to_degrees(), g.lon_rad.to_degrees());
    format!(
        "GPGGA,{},{},{},{},{:02},1.0,{:.1},M,0.0,M,,",
        hms(e.tow_s),
        la,
        lo,
        fix,
        e.n_sv.min(99),
        g.alt_m
    )
}

/// The RMC body of an epoch.
pub fn rmc_body(e: &Epoch, status: char) -> String {
    let g = ecef_to_geodetic(e.r);
    let (la, lo) = ll(g.lat_rad.to_degrees(), g.lon_rad.to_degrees());
    let sp = (e.v[0] * e.v[0] + e.v[1] * e.v[1] + e.v[2] * e.v[2]).sqrt() / 0.514_444;
    format!(
        "GPRMC,{},{status},{},{},{:.1},0.0,201126,,,A",
        hms(e.tow_s),
        la,
        lo,
        sp
    )
}

/// Build the stream from a trace (at least 40 epochs with fix).
pub fn build(trace: &[Epoch]) -> Result<Vec<Vector>, String> {
    let ep: Vec<&Epoch> = trace.iter().filter(|e| e.fix).collect();
    if ep.len() < 40 {
        return Err("obc: the trace needs at least 40 epochs with fix".into());
    }
    let mut v: Vec<Vector> = Vec::new();
    let push = |v: &mut Vec<Vector>, class: &str, text: Vec<u8>, allowed: &[&str], note: &str| {
        let id = format!("v{:04}-{class}", v.len() + 1);
        v.push(Vector {
            id,
            class: class.into(),
            hex: hex::encode(&text),
            allowed: allowed.iter().map(|s| s.to_string()).collect(),
            note: note.into(),
        });
    };
    let reject: &[&str] = &["reject"];
    let flag: &[&str] = &["flag", "reject"];
    // One corrupt class after each nominal epoch, in a fixed order, leaving clean epochs
    // between so the OBC's state recovers.
    let classes = [
        "checksum_bitflip",
        "checksum_invalid_hex",
        "truncated",
        "missing_terminator",
        "overlong",
        "non_printable",
        "garbage",
        "empty_fields",
        "out_of_range",
        "no_fix",
        "duplicate",
        "time_backwards",
        "position_jump",
        "altitude_implausible",
        "unknown_sentence",
    ];
    let mut prev_gga: Option<String> = None;
    let mut older_gga: Option<String> = None;
    for (k, e) in ep.iter().enumerate() {
        let gga = sentence(&gga_body(e, 1));
        let rmc = sentence(&rmc_body(e, 'A'));
        let slot = if k % 2 == 1 && k / 2 < classes.len() {
            Some(classes[k / 2])
        } else {
            None
        };
        // A position jump replaces the epoch's nominal GGA (state must not advance).
        if slot == Some("position_jump") {
            let body = gga_body(e, 1);
            let mut f: Vec<String> = body.split(',').map(String::from).collect();
            let lat: f64 = f[2][..2].parse().unwrap_or(0.0);
            f[2] = format!("{:02}{}", ((lat as u32) + 7) % 90, &f[2][2..]);
            push(
                &mut v,
                "position_jump",
                sentence(&f.join(",")).into_bytes(),
                flag,
                "latitude degrees changed by 7, checksum recomputed: off the orbit",
            );
        } else {
            push(
                &mut v,
                "nominal",
                gga.clone().into_bytes(),
                &["accept"],
                "nominal GGA",
            );
        }
        push(
            &mut v,
            "nominal",
            rmc.clone().into_bytes(),
            &["accept"],
            "nominal RMC",
        );
        match slot {
            Some("checksum_bitflip") => {
                let mut b = gga.clone().into_bytes();
                b[20] ^= 0x04;
                push(
                    &mut v,
                    "checksum_bitflip",
                    b,
                    reject,
                    "one payload bit flipped, checksum kept",
                );
            }
            Some("checksum_invalid_hex") => {
                let n = gga.len();
                push(
                    &mut v,
                    "checksum_invalid_hex",
                    format!("{}G1", &gga[..n - 2]).into_bytes(),
                    reject,
                    "checksum characters not hexadecimal",
                );
            }
            Some("truncated") => push(
                &mut v,
                "truncated",
                gga.as_bytes()[..30].to_vec(),
                reject,
                "cut before the checksum",
            ),
            Some("missing_terminator") => push(
                &mut v,
                "missing_terminator",
                format!("{gga}{rmc}").into_bytes(),
                reject,
                "two sentences without CR LF between",
            ),
            Some("overlong") => {
                let body = format!("{},{}", gga_body(e, 1), "0".repeat(40));
                push(
                    &mut v,
                    "overlong",
                    sentence(&body).into_bytes(),
                    reject,
                    "longer than 82 characters, valid checksum",
                );
            }
            Some("non_printable") => {
                let mut b = gga.clone().into_bytes();
                b.insert(25, 0x00);
                push(
                    &mut v,
                    "non_printable",
                    b,
                    reject,
                    "a NUL byte inside the sentence",
                );
            }
            Some("garbage") => push(
                &mut v,
                "garbage",
                vec![0xFF, 0x13, 0x7E, 0x24, 0x00, 0xA5, 0x2A, 0x31],
                reject,
                "bytes that are not a sentence",
            ),
            Some("empty_fields") => {
                let body = gga_body(e, 1);
                let mut f: Vec<&str> = body.split(',').collect();
                for x in f.iter_mut().take(6).skip(2) {
                    *x = "";
                }
                push(
                    &mut v,
                    "empty_fields",
                    sentence(&f.join(",")).into_bytes(),
                    &["reject", "flag"],
                    "position fields empty, fix quality 1",
                );
            }
            Some("out_of_range") => {
                let body = gga_body(e, 1);
                let mut f: Vec<String> = body.split(',').map(String::from).collect();
                f[2] = format!("95{}", &f[2][2..]);
                push(
                    &mut v,
                    "out_of_range",
                    sentence(&f.join(",")).into_bytes(),
                    reject,
                    "latitude 95 degrees",
                );
            }
            Some("no_fix") => push(
                &mut v,
                "no_fix",
                sentence(&rmc_body(e, 'V')).into_bytes(),
                flag,
                "RMC status V (no valid fix)",
            ),
            Some("duplicate") => push(
                &mut v,
                "duplicate",
                gga.clone().into_bytes(),
                flag,
                "the same GGA again",
            ),
            Some("time_backwards") => {
                if let Some(o) = &older_gga {
                    push(
                        &mut v,
                        "time_backwards",
                        o.clone().into_bytes(),
                        flag,
                        "an older GGA again",
                    );
                }
            }
            Some("altitude_implausible") => {
                let body = gga_body(e, 1);
                let mut f: Vec<String> = body.split(',').map(String::from).collect();
                f[9] = "-12.0".into();
                push(
                    &mut v,
                    "altitude_implausible",
                    sentence(&f.join(",")).into_bytes(),
                    flag,
                    "altitude below the orbit floor",
                );
            }
            Some("unknown_sentence") => push(
                &mut v,
                "unknown_sentence",
                sentence("GPXYZ,1,2,3").into_bytes(),
                &["ignore", "reject"],
                "a sentence type the OBC does not use",
            ),
            _ => {}
        }
        older_gga = prev_gga.take();
        prev_gga = Some(gga);
    }
    Ok(v)
}

/// The replay stream: every vector followed by CR LF, in order.
pub fn stream_bytes(v: &[Vector]) -> Vec<u8> {
    let mut out = Vec::new();
    for x in v {
        out.extend(hex::decode(&x.hex).unwrap_or_default());
        out.extend(b"\r\n");
    }
    out
}

/// The reference parser (stateful).
#[derive(Default)]
pub struct Reference {
    pub limits: Limits,
    last: BTreeMap<String, (f64, Option<[f64; 3]>)>,
}

fn parse_ll(v: &str, h: &str, deg_w: usize, max: f64) -> Option<f64> {
    if v.len() < deg_w + 2 || !(h == "N" || h == "S" || h == "E" || h == "W") {
        return None;
    }
    let d: f64 = v[..deg_w].parse().ok()?;
    let m: f64 = v[deg_w..].parse().ok()?;
    if m >= 60.0 {
        return None;
    }
    let x = d + m / 60.0;
    if x > max {
        return None;
    }
    Some(if h == "S" || h == "W" { -x } else { x })
}

fn parse_time(t: &str) -> Option<f64> {
    if t.len() < 6 {
        return None;
    }
    let h: f64 = t[..2].parse().ok()?;
    let m: f64 = t[2..4].parse().ok()?;
    let s: f64 = t[4..].parse().ok()?;
    (h < 24.0 && m < 60.0 && s < 61.0).then_some(h * 3600.0 + m * 60.0 + s)
}

impl Reference {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            last: BTreeMap::new(),
        }
    }

    /// The action on one line (bytes without CR LF).
    pub fn action(&mut self, b: &[u8]) -> &'static str {
        if b.len() > MAX_LEN
            || b.first() != Some(&b'$')
            || b.iter().any(|c| !(0x20..0x7F).contains(c))
        {
            return "reject";
        }
        let Ok(s) = std::str::from_utf8(b) else {
            return "reject";
        };
        let Some((body, cs)) = s[1..].rsplit_once('*') else {
            return "reject";
        };
        if body.contains('$') || body.contains('*') {
            return "reject";
        }
        match u8::from_str_radix(cs, 16) {
            Ok(c) if cs.len() == 2 && c == checksum(body) => {}
            _ => return "reject",
        }
        let f: Vec<&str> = body.split(',').collect();
        let kind = f[0].get(2..).unwrap_or("");
        let (time, lat, lon, alt, valid) = match kind {
            "GGA" if f.len() >= 10 => (
                parse_time(f[1]),
                parse_ll(f[2], f[3], 2, 90.0),
                parse_ll(f[4], f[5], 3, 180.0),
                f[9].parse::<f64>().ok(),
                f[6] != "0",
            ),
            "RMC" if f.len() >= 7 => (
                parse_time(f[1]),
                parse_ll(f[3], f[4], 2, 90.0),
                parse_ll(f[5], f[6], 3, 180.0),
                None,
                f[2] == "A",
            ),
            "GGA" | "RMC" => return "reject",
            _ => return "ignore",
        };
        let (Some(t), Some(la), Some(lo)) = (time, lat, lon) else {
            return "reject";
        };
        if !valid {
            return "flag";
        }
        if let Some(a) = alt {
            if a < self.limits.min_alt_m || a > self.limits.max_alt_m {
                return "flag";
            }
        }
        let pos = alt.map(|a| {
            geodetic_to_ecef(Geodetic {
                lat_rad: la.to_radians(),
                lon_rad: lo.to_radians(),
                alt_m: a,
            })
        });
        if let Some((t0, p0)) = self.last.get(kind) {
            let dt = t - t0;
            if dt <= 0.0 && dt > -43_200.0 {
                return "flag";
            }
            if let (Some(p), Some(p0)) = (pos, p0) {
                let d = ((p[0] - p0[0]).powi(2) + (p[1] - p0[1]).powi(2) + (p[2] - p0[2]).powi(2))
                    .sqrt();
                if d > self.limits.max_speed_mps * dt.abs() + self.limits.jump_margin_m {
                    return "flag";
                }
            }
        }
        self.last.insert(kind.into(), (t, pos));
        "accept"
    }
}

/// Run the reference parser over a stream: the action per vector id.
pub fn reference_actions(v: &[Vector], limits: &Limits) -> BTreeMap<String, String> {
    let mut r = Reference::new(limits.clone());
    v.iter()
        .map(|x| {
            let b = hex::decode(&x.hex).unwrap_or_default();
            (x.id.clone(), r.action(&b).to_string())
        })
        .collect()
}

/// The vectors document.
pub fn vectors_doc(v: &[Vector], trace_sha256: &str, limits: &Limits) -> Value {
    let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
    for x in v {
        *classes.entry(x.class.as_str()).or_default() += 1;
    }
    json!({
        "schema": VECTORS_SCHEMA,
        "label": LABEL,
        "evidence": "MODELLED (vectors built from the trace given)",
        "trace_sha256": trace_sha256,
        "interface": "NMEA 0183 GGA and RMC over a byte stream, CR LF terminated",
        "reference_limits": limits,
        "classes": classes,
        "vectors": v,
    })
}

/// Parse an OBC log: `id,action` lines (header optional).
pub fn parse_log(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut m = BTreeMap::new();
    for (ln, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l == "id,action" {
            continue;
        }
        let (id, a) = l
            .split_once(',')
            .ok_or_else(|| format!("log line {}: expected id,action", ln + 1))?;
        let a = a.trim();
        if !["accept", "flag", "reject", "ignore"].contains(&a) {
            return Err(format!(
                "log line {}: action must be accept, flag, reject or ignore",
                ln + 1
            ));
        }
        m.insert(id.trim().to_string(), a.to_string());
    }
    Ok(m)
}

/// Score an OBC log against the vectors.
pub fn score(v: &[Vector], log: &BTreeMap<String, String>) -> Value {
    let mut checks = Vec::new();
    let mut per: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    for x in v {
        let r = log.get(&x.id).map(|a| x.allowed.contains(a));
        let e = per.entry(x.class.as_str()).or_default();
        match r {
            Some(true) => e.0 += 1,
            Some(false) => e.1 += 1,
            None => e.2 += 1,
        }
        if r != Some(true) {
            checks.push(Check::new(
                &x.id,
                r,
                format!(
                    "{}: got {}, allowed {}",
                    x.note,
                    log.get(&x.id).map_or("nothing", String::as_str),
                    x.allowed.join("/")
                ),
            ));
        }
    }
    let classes: Vec<Value> = per
        .iter()
        .map(|(c, (m, n, a))| {
            json!({"class": c, "met": m, "not_met": n, "not_assessed": a,
                "verdict": if *n > 0 { "not_met" } else if *a > 0 { "not_assessed" } else { "met" }})
        })
        .collect();
    let tot = |k: usize| per.values().map(|t| [t.0, t.1, t.2][k]).sum::<usize>();
    json!({
        "schema": SCORE_SCHEMA,
        "label": LABEL,
        "evidence": "MODELLED (test evidence for the log given)",
        "met": tot(0),
        "not_met": tot(1),
        "not_assessed": tot(2),
        "classes": classes,
        "exceptions": checks,
    })
}
