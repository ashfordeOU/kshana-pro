// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! TAPR TICC (Time Interval Counter / time-stamping counter, an open-hardware Arduino
//! shield using the TI TDC7200) serial output parser and reader.
//!
//! # Format implemented
//!
//! The TICC prints ASCII lines on its USB serial port (115200 baud, 8N1; set the line
//! externally, see [`super::SerialTransport`]). This parser implements the following line
//! kinds, after the TICC operation manual (TAPR, firmware 2017 and later):
//!
//! * **Comment / banner**: lines starting with `#` (start-up banner, configuration dump).
//! * **Timestamp mode** (the default mode): `<seconds>.<fraction> ch<X>` — the time of an
//!   input edge on channel `X` (`A` or `B`) in seconds since the TICC started, with up to
//!   12 fractional digits (1 ps resolution), e.g. `104.897999725822 chA`. The value is
//!   parsed exactly as integer seconds plus integer picoseconds (no `f64` rounding of large
//!   timestamps).
//! * **Time-interval mode**: `<seconds> TI(A->B)` (also accepted: `TI(B->A)`), a signed
//!   interval from an A edge to the following B edge.
//! * **Period mode** lines have the timestamp layout (`<period> chA`) and parse as
//!   timestamps; interpreting them as periods is the caller's choice.
//!
//! Anything else (e.g. the debug mode's raw columns, or the timeLab mode) parses as
//! [`TiccLine::Other`] and is ignored by the reader. Firmware versions differ in banner
//! text and in the exact mode keywords: **check a capture of your unit's output against
//! this description (NEEDS CONFIRMATION for firmware other than the one the manual
//! describes)**.
//!
//! # Use as a time-interval counter
//!
//! For a reference-PPS→DUT-PPS measurement in timestamp mode, connect the reference PPS to
//! channel A and the device under test to channel B. [`intervals_ns`] pairs each A edge with
//! the next B edge less than half a period later and returns `t_B − t_A`; [`Ticc`] reads the
//! serial stream and implements [`super::TicSource`]. The TICC is free-running, so
//! "arm on PPS" only records a note and discards nothing.

use super::profile::{InstrumentProfile, Role, TiccSettings};
use super::{ScpiSession, SharedLog, TicSource};

/// A timestamp: whole seconds and picoseconds (`0 ≤ ps < 10¹²`), or a signed interval
/// (`seconds` carries the sign; `ps` is always added).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TiccTime {
    pub seconds: i64,
    pub ps: i64,
}

impl TiccTime {
    /// Exact difference `self − other` in picoseconds.
    pub fn minus_ps(self, other: TiccTime) -> i128 {
        (self.seconds as i128 - other.seconds as i128) * 1_000_000_000_000
            + (self.ps - other.ps) as i128
    }
    pub fn as_s(self) -> f64 {
        self.seconds as f64 + self.ps as f64 * 1e-12
    }
}

/// One parsed line.
#[derive(Clone, Debug, PartialEq)]
pub enum TiccLine {
    Comment(String),
    Timestamp { channel: char, t: TiccTime },
    Interval { from: char, to: char, ps: i128 },
    Other(String),
}

/// Parse a decimal number `[-]S[.F]` exactly into picoseconds (fraction up to 12 digits;
/// more digits are rejected).
pub fn parse_ps(s: &str) -> Option<i128> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
    if ip.is_empty() || !ip.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if fp.len() > 12 || !fp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: i128 = ip.parse().ok()?;
    let frac: i128 = if fp.is_empty() {
        0
    } else {
        format!("{fp:0<12}").parse().ok()?
    };
    let v = whole * 1_000_000_000_000 + frac;
    Some(if neg { -v } else { v })
}

/// Parse one line of TICC output.
pub fn parse_line(line: &str) -> TiccLine {
    let l = line.trim();
    if let Some(c) = l.strip_prefix('#') {
        return TiccLine::Comment(c.trim().to_string());
    }
    let mut it = l.split_whitespace();
    let (Some(v), Some(tag), None) = (it.next(), it.next(), it.next()) else {
        return TiccLine::Other(l.to_string());
    };
    let Some(ps) = parse_ps(v) else {
        return TiccLine::Other(l.to_string());
    };
    if let Some(ch) = tag.strip_prefix("ch") {
        let mut c = ch.chars();
        if let (Some(x), None) = (c.next(), c.next()) {
            if x.is_ascii_alphabetic() && ps >= 0 {
                return TiccLine::Timestamp {
                    channel: x.to_ascii_uppercase(),
                    t: TiccTime {
                        seconds: (ps / 1_000_000_000_000) as i64,
                        ps: (ps % 1_000_000_000_000) as i64,
                    },
                };
            }
        }
    }
    if let Some(inner) = tag.strip_prefix("TI(").and_then(|r| r.strip_suffix(')')) {
        if let Some((a, b)) = inner.split_once("->") {
            if let (Some(a), Some(b)) = (a.chars().next(), b.chars().next()) {
                return TiccLine::Interval { from: a, to: b, ps };
            }
        }
    }
    TiccLine::Other(l.to_string())
}

/// Parse a whole capture.
pub fn parse(text: &str) -> Vec<TiccLine> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_line)
        .collect()
}

/// Pair each `start` edge with the next `stop` edge within `max_gap_s` after it and return
/// the intervals `t_stop − t_start` in ns. Interval-mode lines `TI(start->stop)` are passed
/// through. Unpaired edges are dropped.
pub fn intervals_ns(lines: &[TiccLine], start: char, stop: char, max_gap_s: f64) -> Vec<f64> {
    let max_ps = (max_gap_s * 1e12) as i128;
    let mut out = Vec::new();
    let mut pending: Option<TiccTime> = None;
    let mut stops: Vec<TiccTime> = Vec::new();
    for l in lines {
        match l {
            TiccLine::Timestamp { channel, t } if *channel == start => {
                // a newer start replaces an unpaired older one
                pending = Some(*t);
                // stops that arrived before the start may belong to it if printed out of
                // order (the TICC prints channels as they complete)
                if let Some(pos) = stops.iter().position(|s| {
                    let d = s.minus_ps(*t);
                    d >= 0 && d <= max_ps
                }) {
                    out.push(stops[pos].minus_ps(*t) as f64 / 1e3);
                    stops.clear();
                    pending = None;
                }
            }
            TiccLine::Timestamp { channel, t } if *channel == stop => match pending {
                Some(a) => {
                    let d = t.minus_ps(a);
                    if d >= 0 && d <= max_ps {
                        out.push(d as f64 / 1e3);
                        pending = None;
                    } else {
                        stops.push(*t);
                    }
                }
                None => {
                    stops.push(*t);
                    if stops.len() > 4 {
                        stops.remove(0);
                    }
                }
            },
            TiccLine::Interval { from, to, ps } if *from == start && *to == stop => {
                out.push(*ps as f64 / 1e3);
            }
            _ => {}
        }
    }
    out
}

/// A TICC on a serial port as a time-interval counter.
pub struct Ticc {
    pub session: ScpiSession,
    pub settings: TiccSettings,
    lines: Vec<TiccLine>,
}

impl Ticc {
    pub fn new(session: ScpiSession, settings: TiccSettings) -> Ticc {
        Ticc {
            session,
            settings,
            lines: Vec::new(),
        }
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Ticc, String> {
        if p.role != Role::Ticc {
            return Err(format!("profile {} is not a TICC profile", p.id));
        }
        Ok(Ticc::new(
            p.connect(log)?,
            p.ticc.clone().unwrap_or_default(),
        ))
    }
}

impl TicSource for Ticc {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn arm_pps(&mut self) -> Result<(), String> {
        self.session
            .note("TICC is free-running: arm_pps is a no-op; pairing starts at the next A edge");
        self.lines.clear();
        Ok(())
    }
    fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String> {
        let s = &self.settings;
        let mut got = Vec::new();
        for _ in 0..s.max_lines {
            let line = self.session.read_line()?;
            let parsed = parse_line(&line);
            if matches!(parsed, TiccLine::Comment(_) | TiccLine::Other(_)) {
                continue;
            }
            self.lines.push(parsed);
            got = intervals_ns(&self.lines, s.start_channel, s.stop_channel, 0.5);
            if got.len() >= n {
                break;
            }
        }
        if got.len() < n {
            return Err(format!(
                "{}: only {} of {n} intervals in {} lines",
                self.session.id,
                got.len(),
                s.max_lines
            ));
        }
        self.lines.clear();
        Ok(got
            .into_iter()
            .take(n)
            .map(|v| v - self.settings.offset_ns)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::SerialTransport;
    use super::*;

    const CAPTURE: &str = "\
# TICC Time Interval Counter by TAPR/KA7OEI/N8UR 2016
# Version: 20170309.1
# Mode: Timestamp
1.000000123456 chA
1.000000173456 chB
2.000000123460 chA
2.000000173461 chB
3.000000173470 chB
3.000000123470 chA
garbage line here
1234567890.999999999999 chA
1234567891.000000000049 chB
-0.000000001234 TI(A->B)
";

    #[test]
    fn parses_every_line_kind_exactly() {
        let l = parse(CAPTURE);
        assert!(matches!(&l[0], TiccLine::Comment(c) if c.starts_with("TICC")));
        assert_eq!(
            l[3],
            TiccLine::Timestamp {
                channel: 'A',
                t: TiccTime {
                    seconds: 1,
                    ps: 123_456
                }
            }
        );
        assert!(matches!(l[9], TiccLine::Other(_)));
        assert_eq!(
            l[12],
            TiccLine::Interval {
                from: 'A',
                to: 'B',
                ps: -1234
            }
        );
        // exact large timestamps (f64 would lose the picoseconds)
        let (TiccLine::Timestamp { t: a, .. }, TiccLine::Timestamp { t: b, .. }) = (&l[10], &l[11])
        else {
            panic!()
        };
        assert_eq!(b.minus_ps(*a), 50);
        assert_eq!(parse_ps("1.5"), Some(1_500_000_000_000));
        assert_eq!(parse_ps("1.0000000000001"), None);
        assert_eq!(parse_ps("x"), None);
    }

    #[test]
    fn pairs_a_to_b_including_out_of_order_lines() {
        let v = intervals_ns(&parse(CAPTURE), 'A', 'B', 0.5);
        assert_eq!(v.len(), 5, "{v:?}");
        assert!((v[0] - 50.0).abs() < 1e-9);
        assert!((v[1] - 50.001).abs() < 1e-9);
        assert!((v[2] - 50.0).abs() < 1e-9); // B printed before A
        assert!((v[3] - 0.05).abs() < 1e-9);
        assert!((v[4] + 1.234).abs() < 1e-12);
    }

    #[test]
    fn reader_over_a_serial_file() {
        let d = std::env::temp_dir().join(format!("kshana-ticc-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let rx = d.join("ticc.txt");
        std::fs::write(&rx, CAPTURE).unwrap();
        let t = SerialTransport::open_split(&rx, &d.join("tx")).unwrap();
        let log = super::super::MeasurementLog::memory().shared();
        let mut ticc = Ticc::new(
            ScpiSession::new("ticc", Box::new(t), Some(log.clone())),
            TiccSettings {
                offset_ns: 0.5,
                ..TiccSettings::default()
            },
        );
        ticc.arm_pps().unwrap();
        let r = ticc.read_n(2).unwrap();
        assert!((r[0] - 49.5).abs() < 1e-9 && (r[1] - 49.501).abs() < 1e-9);
        // the stream ends before 10 more intervals
        assert!(ticc.read_n(10).is_err());
        assert!(log.lock().unwrap().entries().iter().any(|e| e.kind == "note"));
        std::fs::remove_dir_all(&d).ok();
    }
}
