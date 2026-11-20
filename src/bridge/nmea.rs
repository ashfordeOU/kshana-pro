// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Reader of the bridge sentences the consistency aid uses, from a recorded IEC 61162-1
//! (NMEA 0183) log or a capture of IEC 61162-450 datagrams (unwrapped by the open engine's
//! [`kshana::iec61162_450`]).
//!
//! Read: `RMC`, `GGA`, `GNS` (position, fix status, UTC), `VTG` (course and speed over
//! ground), `ZDA` (UTC), `HDT`, `THS` (true heading), `VHW` (speed through water),
//! `VBW` (water and ground speeds), `DPT`, `DBT` (depth) and `TTM` (radar tracked target).
//! Everything else is counted and ignored. A sentence whose `*hh` checksum does not match is
//! skipped and counted; one with no checksum is read and counted.
//!
//! **Sources.** A sentence's source is the TAG block's `s:` field when the line carries one,
//! else its talker (`GP`, `HE`, `VD`, ...). Each source has a role ([`Role`]); the defaults
//! follow the usual talkers and a configuration overrides them.
//!
//! **Epochs.** A new epoch starts at each `RMC` of the primary GNSS source (its `GGA` or
//! `GNS` when it writes no `RMC`); every sentence until the next one belongs to it. The
//! epoch's time is its index divided by the configured output rate, or the TAG block's `c:`
//! time when every epoch carries one: the bridge's own cadence, so the GNSS time is a
//! measurement to check rather than the time base.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What a sentence source is on the bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// A GNSS receiver: position, speed and course over ground, UTC.
    Gnss,
    /// A GNSS compass (heading from two or more antennas).
    GnssCompass,
    /// A gyro compass (or another heading source independent of GNSS).
    Gyro,
    /// A speed log (water track, or bottom track from a Doppler log).
    Log,
    /// An echo-sounder.
    EchoSounder,
    /// A radar (tracked targets).
    Radar,
    /// Read but not used.
    Ignored,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Gnss => "gnss",
            Role::GnssCompass => "gnss-compass",
            Role::Gyro => "gyro",
            Role::Log => "log",
            Role::EchoSounder => "echo-sounder",
            Role::Radar => "radar",
            Role::Ignored => "ignored",
        }
    }

    /// Whether the source is exposed to radio-frequency interference with GNSS.
    pub fn gnss_family(self) -> bool {
        matches!(self, Role::Gnss | Role::GnssCompass)
    }
}

/// The default role of a talker.
pub fn default_role(talker: &str) -> Role {
    match talker {
        "GP" | "GN" | "GL" | "GA" | "GB" | "BD" | "GQ" | "GI" => Role::Gnss,
        "HE" | "HN" => Role::Gyro,
        "VD" | "VW" | "VM" => Role::Log,
        "SD" | "SS" => Role::EchoSounder,
        "RA" => Role::Radar,
        _ => Role::Ignored,
    }
}

/// One GNSS source's observation in an epoch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GnssObs {
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    /// Fix valid (RMC status A, GGA quality above 0, GNS mode not N).
    pub valid: bool,
    pub sog_kn: Option<f64>,
    pub cog_deg: Option<f64>,
    /// UTC as seconds of the day, and the date when stated.
    pub sod: Option<f64>,
    pub date: Option<(i64, u32, u32)>,
}

/// A radar tracked target (TTM).
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub source: String,
    pub number: u32,
    pub range_m: f64,
    pub bearing_deg: f64,
    /// Bearing relative to the ship's head (`R`) rather than true (`T`).
    pub relative: bool,
}

/// Everything read in one epoch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Epoch {
    /// Seconds from the first epoch, on the bridge's cadence.
    pub t_s: f64,
    /// GNSS sources (role [`Role::Gnss`]).
    pub gnss: BTreeMap<String, GnssObs>,
    /// Heading sources with their role ([`Role::Gyro`] or [`Role::GnssCompass`]).
    pub heading: BTreeMap<String, (Role, f64)>,
    /// Log water speeds, knots: longitudinal and transverse (starboard positive), per source.
    pub water: BTreeMap<String, (f64, Option<f64>)>,
    /// Log ground speeds (bottom track), knots, per source.
    pub ground: BTreeMap<String, (f64, f64)>,
    /// Depth below the transducer (or keel, as the sentence states it), metres, per source.
    pub depth_m: BTreeMap<String, f64>,
    pub targets: Vec<Target>,
    /// The TAG block `c:` time of the epoch's first sentence, Unix seconds.
    pub tag_time: Option<i64>,
}

/// What was read.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReadSummary {
    pub lines: usize,
    pub sentences_used: usize,
    pub bad_checksum: usize,
    pub no_checksum: usize,
    pub malformed: usize,
    pub ignored: usize,
    /// IEC 61162-450 datagrams and TAG blocks seen, and TAG blocks dropped.
    pub datagrams: usize,
    pub tag_blocks: usize,
    pub bad_tag_blocks: usize,
    /// Sentence counts by type.
    pub by_type: BTreeMap<String, usize>,
    /// Every source seen, with its role.
    pub sources: BTreeMap<String, Role>,
    /// The source whose RMC/GGA/GNS opens each epoch.
    pub primary_gnss: Option<String>,
    /// Whether epoch times came from TAG blocks rather than the configured rate.
    pub tag_timed: bool,
}

/// One parsed sentence.
#[derive(Clone, Debug, PartialEq)]
enum Rec {
    Fix {
        kind: &'static str,
        lat: Option<f64>,
        lon: Option<f64>,
        valid: bool,
        sod: Option<f64>,
        date: Option<(i64, u32, u32)>,
        sog: Option<f64>,
        cog: Option<f64>,
    },
    Vtg {
        sog: Option<f64>,
        cog: Option<f64>,
    },
    Zda {
        sod: Option<f64>,
        date: Option<(i64, u32, u32)>,
    },
    Heading(f64),
    Water(f64, Option<f64>),
    WaterGround(Option<(f64, Option<f64>)>, Option<(f64, f64)>),
    Depth(f64),
    Ttm {
        number: u32,
        range_m: f64,
        bearing: f64,
        relative: bool,
    },
}

fn num(s: Option<&&str>) -> Option<f64> {
    s.and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|x| x.is_finite())
}

/// `ddmm.mmmm` (or `dddmm.mmmm`) with a hemisphere letter, to signed degrees.
fn angle(v: Option<&&str>, hemi: Option<&&str>) -> Option<f64> {
    let v = v?.trim();
    if v.is_empty() {
        return None;
    }
    let x: f64 = v.parse().ok()?;
    let deg = (x / 100.0).trunc();
    let min = x - deg * 100.0;
    if !(0.0..60.0).contains(&min) {
        return None;
    }
    let d = deg + min / 60.0;
    match hemi.map(|h| h.trim()) {
        Some("S") | Some("W") => Some(-d),
        Some("N") | Some("E") => Some(d),
        _ => None,
    }
}

/// `hhmmss.ss` to seconds of the day.
fn time_of_day(v: Option<&&str>) -> Option<f64> {
    let v = v?.trim();
    if v.len() < 6 {
        return None;
    }
    let h: f64 = v.get(0..2)?.parse().ok()?;
    let m: f64 = v.get(2..4)?.parse().ok()?;
    let s: f64 = v.get(4..)?.parse().ok()?;
    (h < 24.0 && m < 60.0 && s < 61.0).then_some(h * 3600.0 + m * 60.0 + s)
}

/// RMC `ddmmyy` to (year, month, day).
fn rmc_date(v: Option<&&str>) -> Option<(i64, u32, u32)> {
    let v = v?.trim();
    if v.len() != 6 {
        return None;
    }
    let d: u32 = v.get(0..2)?.parse().ok()?;
    let m: u32 = v.get(2..4)?.parse().ok()?;
    let y: i64 = v.get(4..6)?.parse().ok()?;
    ((1..=31).contains(&d) && (1..=12).contains(&m)).then_some((2000 + y, m, d))
}

/// Split a checked sentence into (talker, type, fields). Errors: `Err(true)` for a bad
/// checksum, `Err(false)` for a malformed line.
fn split(line: &str) -> Result<(String, String, Vec<&str>, bool), bool> {
    let body = line
        .strip_prefix('$')
        .or_else(|| line.strip_prefix('!'))
        .ok_or(false)?;
    let (body, checked) = match body.rsplit_once('*') {
        Some((b, ck)) => {
            let want = u8::from_str_radix(ck.trim(), 16).map_err(|_| false)?;
            if kshana::interop::testbench::nmea_checksum(b) != want {
                return Err(true);
            }
            (b, true)
        }
        None => (body, false),
    };
    let mut f: Vec<&str> = body.split(',').collect();
    let head = f.remove(0);
    if head.len() < 5 || !head.is_ascii() {
        return Err(false);
    }
    let (talker, kind) = head.split_at(head.len() - 3);
    Ok((talker.to_string(), kind.to_string(), f, checked))
}

fn parse(kind: &str, f: &[&str]) -> Result<Option<Rec>, ()> {
    let g = |i: usize| f.get(i);
    Ok(Some(match kind {
        "RMC" => Rec::Fix {
            kind: "RMC",
            sod: time_of_day(g(0)),
            valid: g(1).map(|s| s.trim()) == Some("A"),
            lat: angle(g(2), g(3)),
            lon: angle(g(4), g(5)),
            sog: num(g(6)),
            cog: num(g(7)),
            date: rmc_date(g(8)),
        },
        "GGA" => Rec::Fix {
            kind: "GGA",
            sod: time_of_day(g(0)),
            lat: angle(g(1), g(2)),
            lon: angle(g(3), g(4)),
            valid: num(g(5)).is_some_and(|q| q > 0.0),
            date: None,
            sog: None,
            cog: None,
        },
        "GNS" => Rec::Fix {
            kind: "GNS",
            sod: time_of_day(g(0)),
            lat: angle(g(1), g(2)),
            lon: angle(g(3), g(4)),
            valid: g(5).is_some_and(|m| m.trim().chars().any(|c| c != 'N')),
            date: None,
            sog: None,
            cog: None,
        },
        "VTG" => Rec::Vtg {
            cog: num(g(0)),
            sog: num(g(4)),
        },
        "ZDA" => {
            let (d, m, y) = (num(g(1)), num(g(2)), num(g(3)));
            Rec::Zda {
                sod: time_of_day(g(0)),
                date: match (y, m, d) {
                    (Some(y), Some(m), Some(d)) => Some((y as i64, m as u32, d as u32)),
                    _ => None,
                },
            }
        }
        "HDT" => match num(g(0)) {
            Some(h) => Rec::Heading(h.rem_euclid(360.0)),
            None => return Ok(None),
        },
        // THS: heading and mode; V is not valid.
        "THS" => match (num(g(0)), g(1).map(|s| s.trim())) {
            (Some(h), Some(m)) if m != "V" => Rec::Heading(h.rem_euclid(360.0)),
            _ => return Ok(None),
        },
        // VHW: heading T, heading M, speed through water knots at 4.
        "VHW" => match num(g(4)) {
            Some(v) => Rec::Water(v, None),
            None => return Ok(None),
        },
        // VBW: water long, water trans, status, ground long, ground trans, status, ...
        "VBW" => {
            let ok = |i: usize| g(i).map(|s| s.trim()) == Some("A");
            let water = match (num(g(0)), ok(2)) {
                (Some(l), true) => Some((l, num(g(1)))),
                _ => None,
            };
            let ground = match (num(g(3)), num(g(4)), ok(5)) {
                (Some(l), Some(t), true) => Some((l, t)),
                _ => None,
            };
            if water.is_none() && ground.is_none() {
                return Ok(None);
            }
            Rec::WaterGround(water, ground)
        }
        // DPT: depth relative to the transducer, offset (positive: to the waterline,
        // negative: to the keel). The depth below the transducer is reported.
        "DPT" => match num(g(0)) {
            Some(d) => Rec::Depth(d),
            None => return Ok(None),
        },
        // DBT: feet, f, metres, M, fathoms, F.
        "DBT" => match num(g(2)).or_else(|| num(g(0)).map(|ft| ft * 0.3048)) {
            Some(d) => Rec::Depth(d),
            None => return Ok(None),
        },
        // TTM: number, distance, bearing, T/R, speed, course, T/R, CPA, TCPA, units, ...
        "TTM" => {
            let number = num(g(0)).ok_or(())? as u32;
            let dist = num(g(1)).ok_or(())?;
            let bearing = num(g(2)).ok_or(())?;
            let relative = g(3).map(|s| s.trim()) == Some("R");
            let unit = match g(9).map(|s| s.trim()) {
                Some("K") => 1000.0,
                Some("S") => 1609.344,
                _ => 1852.0,
            };
            Rec::Ttm {
                number,
                range_m: dist * unit,
                bearing: bearing.rem_euclid(360.0),
                relative,
            }
        }
        _ => return Ok(None),
    }))
}

/// One line split from its TAG block: (source, `c:` time, sentence).
fn tag_split(line: &str) -> Result<(Option<String>, Option<i64>, &str), ()> {
    let Some(rest) = line.strip_prefix('\\') else {
        return Ok((None, None, line));
    };
    let end = rest.find('\\').ok_or(())?;
    let body = &rest[..end];
    let (fields, ck) = body.rsplit_once('*').ok_or(())?;
    let ck = u8::from_str_radix(ck.trim(), 16).map_err(|_| ())?;
    if fields.bytes().fold(0u8, |a, b| a ^ b) != ck {
        return Err(());
    }
    let mut src = None;
    let mut c = None;
    for f in fields.split(',') {
        if let Some(v) = f.strip_prefix("s:") {
            src = Some(v.to_string());
        } else if let Some(v) = f.strip_prefix("c:") {
            c = v.parse().ok();
        }
    }
    Ok((src, c, &rest[end + 1..]))
}

/// How sources map to roles and which one opens an epoch.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReaderConfig {
    /// Role overrides by source (a talker or a TAG `s:` value).
    #[serde(default)]
    pub sources: BTreeMap<String, Role>,
    /// The GNSS source that opens each epoch (default: the first GNSS source seen).
    #[serde(default)]
    pub primary_gnss: Option<String>,
    /// The output rate of the primary GNSS source, Hz (default 1).
    #[serde(default)]
    pub rate_hz: Option<f64>,
}

impl ReaderConfig {
    fn role(&self, source: &str, talker: &str) -> Role {
        if let Some(r) = self.sources.get(source) {
            return *r;
        }
        if let Some(r) = self.sources.get(talker) {
            return *r;
        }
        default_role(talker)
    }
}

/// Read a log or a capture into epochs.
pub fn read(bytes: &[u8], cfg: &ReaderConfig) -> Result<(Vec<Epoch>, ReadSummary), String> {
    let mut sum = ReadSummary::default();
    // A binary IEC 61162-450 capture: count its datagrams; the TAG blocks are read below
    // with their source and time, so the text is kept as it is apart from the headers.
    let text = String::from_utf8_lossy(bytes);
    let un = kshana::iec61162_450::unwrap(bytes);
    sum.datagrams = un.datagrams;
    let mut lines: Vec<String> = Vec::new();
    for part in text.split("UdPbC") {
        for raw in part.split(['\n', '\r']) {
            let l = raw.trim_matches(|c: char| c == '\0' || c.is_whitespace());
            if !l.is_empty() {
                lines.push(l.to_string());
            }
        }
    }
    // Pass 1: parse every line.
    let mut recs: Vec<(String, Role, Option<i64>, Rec)> = Vec::new();
    for l in &lines {
        sum.lines += 1;
        let (src, c, sentence) = match tag_split(l) {
            Ok(x) => x,
            Err(()) => {
                sum.bad_tag_blocks += 1;
                continue;
            }
        };
        if src.is_some() || c.is_some() {
            sum.tag_blocks += 1;
        }
        let (talker, kind, fields, checked) = match split(sentence) {
            Ok(x) => x,
            Err(true) => {
                sum.bad_checksum += 1;
                continue;
            }
            Err(false) => {
                sum.malformed += 1;
                continue;
            }
        };
        if !checked {
            sum.no_checksum += 1;
        }
        let source = src.unwrap_or_else(|| talker.clone());
        let mut role = cfg.role(&source, &talker);
        match parse(&kind, &fields) {
            Ok(Some(rec)) => {
                // A heading from a GNSS source is a GNSS compass.
                if matches!(rec, Rec::Heading(_)) && role == Role::Gnss {
                    role = Role::GnssCompass;
                }
                if role == Role::Ignored {
                    sum.ignored += 1;
                    continue;
                }
                *sum.by_type.entry(kind.clone()).or_default() += 1;
                sum.sources.entry(source.clone()).or_insert(role);
                sum.sentences_used += 1;
                recs.push((source, role, c, rec));
            }
            Ok(None) => sum.ignored += 1,
            Err(()) => sum.malformed += 1,
        }
    }
    // The primary GNSS source and the sentence type that opens its epochs.
    let primary = cfg.primary_gnss.clone().or_else(|| {
        recs.iter()
            .find(|r| r.1 == Role::Gnss && matches!(r.3, Rec::Fix { .. }))
            .map(|r| r.0.clone())
    });
    let Some(primary) = primary else {
        return Err("no GNSS position sentence (RMC, GGA or GNS) in the log".into());
    };
    let opener = ["RMC", "GGA", "GNS"]
        .into_iter()
        .find(|k| {
            recs.iter()
                .any(|r| r.0 == primary && matches!(r.3, Rec::Fix { kind, .. } if kind == *k))
        })
        .ok_or_else(|| format!("the primary GNSS source '{primary}' writes no position"))?;
    sum.primary_gnss = Some(primary.clone());

    // Pass 2: group into epochs.
    let rate = cfg.rate_hz.unwrap_or(1.0);
    if !(rate > 0.0 && rate.is_finite()) {
        return Err("rate_hz must be positive".into());
    }
    let mut epochs: Vec<Epoch> = vec![Epoch::default()];
    let mut started = false;
    for (source, role, c, rec) in recs {
        if matches!(rec, Rec::Fix { kind, .. } if kind == opener) && source == primary {
            if started {
                epochs.push(Epoch::default());
            }
            started = true;
        }
        let e = epochs.last_mut().expect("one epoch");
        if e.tag_time.is_none() {
            e.tag_time = c;
        }
        match (role, rec) {
            (
                Role::Gnss,
                Rec::Fix {
                    lat,
                    lon,
                    valid,
                    sod,
                    date,
                    sog,
                    cog,
                    ..
                },
            ) => {
                let o = e.gnss.entry(source).or_default();
                // The first fix sentence of an epoch decides validity; later ones fill in.
                if o.lat_deg.is_none() && o.sod.is_none() {
                    o.valid = valid;
                } else {
                    o.valid = o.valid && valid;
                }
                if valid {
                    o.lat_deg = o.lat_deg.or(lat);
                    o.lon_deg = o.lon_deg.or(lon);
                    o.sog_kn = o.sog_kn.or(sog);
                    o.cog_deg = o.cog_deg.or(cog);
                }
                o.sod = o.sod.or(sod);
                o.date = o.date.or(date);
            }
            (Role::Gnss, Rec::Vtg { sog, cog }) => {
                let o = e.gnss.entry(source).or_default();
                o.sog_kn = o.sog_kn.or(sog);
                o.cog_deg = o.cog_deg.or(cog);
            }
            (Role::Gnss, Rec::Zda { sod, date }) => {
                let o = e.gnss.entry(source).or_default();
                o.sod = o.sod.or(sod);
                o.date = o.date.or(date);
            }
            (r @ (Role::Gyro | Role::GnssCompass), Rec::Heading(h)) => {
                e.heading.insert(source, (r, h));
            }
            (Role::Log, Rec::Water(l, t)) => {
                e.water.insert(source, (l, t));
            }
            (Role::Log, Rec::WaterGround(w, g)) => {
                if let Some(w) = w {
                    e.water.insert(source.clone(), w);
                }
                if let Some(g) = g {
                    e.ground.insert(source, g);
                }
            }
            (Role::EchoSounder, Rec::Depth(d)) => {
                e.depth_m.insert(source, d);
            }
            (
                Role::Radar,
                Rec::Ttm {
                    number,
                    range_m,
                    bearing,
                    relative,
                },
            ) => e.targets.push(Target {
                source,
                number,
                range_m,
                bearing_deg: bearing,
                relative,
            }),
            // A sentence from a source whose role does not use it.
            _ => {}
        }
    }
    // Drop sentences before the first opener when they made an epoch with no GNSS.
    if epochs.len() > 1 && !epochs[0].gnss.contains_key(&primary) {
        epochs.remove(0);
    }
    let tag_timed = epochs.iter().all(|e| e.tag_time.is_some()) && !epochs.is_empty();
    let t0 = epochs.first().and_then(|e| e.tag_time).unwrap_or(0);
    for (i, e) in epochs.iter_mut().enumerate() {
        e.t_s = if tag_timed {
            (e.tag_time.unwrap_or(t0) - t0) as f64
        } else {
            i as f64 / rate
        };
    }
    sum.tag_timed = tag_timed;
    Ok((epochs, sum))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(body: &str) -> String {
        format!(
            "${body}*{:02X}",
            kshana::interop::testbench::nmea_checksum(body)
        )
    }

    #[test]
    fn reads_the_bridge_sentences_into_epochs() {
        let log = [
            s("GPRMC,120000.00,A,5100.000,N,00124.000,E,12.0,45.0,030626,,,A"),
            s("HEHDT,44.5,T"),
            s("VDVBW,11.5,0.1,A,12.0,0.2,A,,,,"),
            s("SDDPT,35.2,0.5"),
            s("RATTM,03,1.50,120.0,T,0.0,0.0,T,0.0,0.0,N,MARK,T,,"),
            s("GPRMC,120001.00,A,5100.003,N,00124.004,E,12.1,45.2,030626,,,A"),
            "$HEHDT,44.7,T*00".to_string(),
            s("GPVTG,45.2,T,,M,12.1,N,22.4,K,A"),
        ]
        .join("\r\n");
        let (ep, sum) = read(log.as_bytes(), &ReaderConfig::default()).unwrap();
        assert_eq!(ep.len(), 2);
        assert_eq!(sum.bad_checksum, 1);
        assert_eq!(sum.primary_gnss.as_deref(), Some("GP"));
        let e = &ep[0];
        let g = &e.gnss["GP"];
        assert!(g.valid);
        assert!((g.lat_deg.unwrap() - 51.0).abs() < 1e-9);
        assert!((g.lon_deg.unwrap() - (1.0 + 24.0 / 60.0)).abs() < 1e-9);
        assert_eq!(g.sod, Some(43_200.0));
        assert_eq!(g.date, Some((2026, 6, 3)));
        assert_eq!(e.heading["HE"], (Role::Gyro, 44.5));
        assert_eq!(e.ground["VD"], (12.0, 0.2));
        assert_eq!(e.depth_m["SD"], 35.2);
        assert_eq!(e.targets[0].number, 3);
        assert!((e.targets[0].range_m - 2778.0).abs() < 1e-6);
        assert_eq!(ep[1].t_s, 1.0);
        assert!(ep[1].heading.is_empty(), "the corrupt HDT is skipped");
    }

    #[test]
    fn tag_blocks_give_source_and_time_and_a_gnss_heading_is_a_compass() {
        let tag = |f: &str| format!("\\{f}*{:02X}\\", f.bytes().fold(0u8, |a, b| a ^ b));
        let log = [
            format!(
                "{}{}",
                tag("s:GP0001,c:1780000000"),
                s("GPRMC,120000,A,5100.0,N,00124.0,E,12,45,030626,,,A")
            ),
            format!("{}{}", tag("s:GP0001,c:1780000000"), s("GPHDT,45.0,T")),
            format!(
                "{}{}",
                tag("s:GP0001,c:1780000002"),
                s("GPRMC,120002,A,5100.0,N,00124.0,E,12,45,030626,,,A")
            ),
            "\\s:XX*00\\$GPHDT,1,T".to_string(),
        ]
        .join("\n");
        let (ep, sum) = read(log.as_bytes(), &ReaderConfig::default()).unwrap();
        assert!(sum.tag_timed);
        assert_eq!(sum.bad_tag_blocks, 1);
        assert_eq!(ep[1].t_s, 2.0);
        assert_eq!(ep[0].heading["GP0001"].0, Role::GnssCompass);
    }

    #[test]
    fn no_gnss_is_an_error() {
        assert!(read(s("HEHDT,1,T").as_bytes(), &ReaderConfig::default()).is_err());
    }
}
