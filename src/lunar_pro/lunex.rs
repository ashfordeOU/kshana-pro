// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D5. LUNEX prototype: a RINEX-like lunar observation file (reader, writer, validator)
//!
//! **A prototype proposal for the IGS RINEX Committee, not a standard.** The committee proposes
//! LUNEX (lunar RINEX) and wants a neutral entity to define it before the signals exist
//! (research note §0 item 10, gap 30). This prototype makes one concrete design choice to argue
//! from: a LUNEX observation file is **syntactically a RINEX 3.05 observation file** plus new
//! header records, so existing RINEX readers keep working on the observation body (the tests
//! read every file written here with the open engine's RINEX 3 parser,
//! [`kshana::rinex_obs::parse_obs`], and this module's reader uses that parser for the body).
//!
//! ## New header records (proposal)
//!
//! | Label (cols 61–80) | Content (cols 1–60) |
//! |--------------------|---------------------|
//! | `LUNEX VERSION / TYPE` | LUNEX version (A10), `PROTOTYPE` |
//! | `LUNAR PLATFORM` | `SURFACE`, `LANDER`, `ROVER` or `ORBIT` |
//! | `LUNAR REF FRAME` | the frame of `APPROX POSITION XYZ`: `MOON_PA_DE440`, `MOON_ME_DE421`, `MOON_PA_DE421`, `MOON_ME_DE440_ME421`, `MCI_ICRF` or `ILURF_PROTO` |
//! | `LUNAR TIME SCALE` | scale tag (A4: `TCL`, `TL`), `W0_L` (F16.4, m²/s², required for `TL`), realisation code (A8, e.g. `TL(k)` lab code) |
//!
//! ## Time-system tags
//! `TIME OF FIRST OBS` (cols 49–51) and every epoch are in one of `GPS`, `GAL`, `UTC`, `TAI`,
//! `TT` (Earth scales, as in RINEX) or the lunar tags `TCL` and `TL` (proposal). A lunar tag must
//! match the `LUNAR TIME SCALE` record.
//!
//! ## LunaNet AFS observables (proposal)
//! System letter `L` for the LunaNet Augmented Forward Signal, band `9` (2492.028 MHz, the
//! frequency RINEX already numbers 9 for the NavIC S band), attributes `D` (I, BPSK(1) data),
//! `P` (Q, BPSK(5) pilot) and `X` (combined): e.g. `C9P`, `L9P`, `D9P`, `S9P`.
//!
//! ## Converter
//! [`from_csv`] reads a simple CSV of LuGRE-style observations
//! (`time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz`). It is modelled on the
//! kind of fields the LuGRE open dataset carries (GPS L1 C/A and L5, Galileo E1 and E5a seen
//! from the lunar surface); it is **not** that dataset's exact schema, which has not been
//! checked here.
//!
//! Evidence label: **MODELLED** (round-trip, cross-reader and fault-injection tests). No real
//! lunar observation file has been converted yet.

use std::collections::BTreeMap;

use kshana::rinex::EpochUtc;
use kshana::rinex_obs::{ObsEpoch, Observation, SatObs};
use serde::Serialize;

/// The statement every LUNEX output carries.
pub const STATUS: &str =
    "LUNEX 0.1 PROTOTYPE: a Kshana proposal for the IGS RINEX Committee, not a standard";
pub const LUNEX_VERSION: &str = "0.1";
pub const RINEX_VERSION: &str = "3.05";
pub const TIME_TAGS: [&str; 7] = ["GPS", "GAL", "UTC", "TAI", "TT", "TCL", "TL"];
pub const LUNAR_TAGS: [&str; 2] = ["TCL", "TL"];
pub const FRAMES: [&str; 6] = [
    "MOON_PA_DE440",
    "MOON_ME_DE421",
    "MOON_PA_DE421",
    "MOON_ME_DE440_ME421",
    "MCI_ICRF",
    "ILURF_PROTO",
];
pub const PLATFORMS: [&str; 4] = ["SURFACE", "LANDER", "ROVER", "ORBIT"];

/// The LUNEX header.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Header {
    pub program: String,
    pub run_by: String,
    pub date: String,
    pub marker: String,
    pub platform: String,
    pub frame: String,
    pub approx_xyz: Option<[f64; 3]>,
    /// The time-system tag of the epochs.
    pub time_tag: String,
    /// `LUNAR TIME SCALE`: (tag, W0_L, realisation), when present.
    pub lunar_scale: Option<(String, Option<f64>, String)>,
    pub interval_s: Option<f64>,
    pub obs_types: Vec<(char, Vec<String>)>,
    pub first_obs: EpochUtc,
    pub comments: Vec<String>,
}

/// A LUNEX file.
#[derive(Clone, Debug, Serialize)]
pub struct Lunex {
    pub header: Header,
    pub epochs: Vec<ObsEpoch>,
}

fn hdr(content: &str, label: &str) -> String {
    let mut c: String = content.chars().take(60).collect();
    while c.chars().count() < 60 {
        c.push(' ');
    }
    format!("{c}{label}\n")
}

fn epoch_fields(e: &EpochUtc) -> String {
    format!(
        "{:>6}{:>6}{:>6}{:>6}{:>6}{:>13.7}",
        e.year, e.month, e.day, e.hour, e.minute, e.second
    )
}

impl Lunex {
    /// Write the file.
    pub fn to_text(&self) -> String {
        let h = &self.header;
        let mut o = String::new();
        o.push_str(&hdr(
            &format!(
                "{:>9}{:11}{:<20}{:<20}",
                RINEX_VERSION, "", "OBSERVATION DATA", "M"
            ),
            "RINEX VERSION / TYPE",
        ));
        o.push_str(&hdr(
            &format!("{:<10}{}", LUNEX_VERSION, "PROTOTYPE"),
            "LUNEX VERSION / TYPE",
        ));
        o.push_str(&hdr(
            &format!("{:<20}{:<20}{:<20}", h.program, h.run_by, h.date),
            "PGM / RUN BY / DATE",
        ));
        o.push_str(&hdr(
            "LUNEX prototype proposal for the IGS RINEX Committee",
            "COMMENT",
        ));
        o.push_str(&hdr(
            "not a standard; see kshana-pro docs/LUNAR-PRO.md",
            "COMMENT",
        ));
        for c in &h.comments {
            o.push_str(&hdr(c, "COMMENT"));
        }
        o.push_str(&hdr(&h.marker, "MARKER NAME"));
        o.push_str(&hdr(&h.platform, "LUNAR PLATFORM"));
        o.push_str(&hdr(&h.frame, "LUNAR REF FRAME"));
        if let Some(x) = h.approx_xyz {
            o.push_str(&hdr(
                &format!("{:>14.4}{:>14.4}{:>14.4}", x[0], x[1], x[2]),
                "APPROX POSITION XYZ",
            ));
        }
        if let Some((tag, w0, real)) = &h.lunar_scale {
            let w0s = w0.map_or(" ".repeat(16), |w| format!("{w:>16.4}"));
            o.push_str(&hdr(&format!("{tag:<4}{w0s}{real:<8}"), "LUNAR TIME SCALE"));
        }
        for (sys, codes) in &h.obs_types {
            for (i, chunk) in codes.chunks(13).enumerate() {
                let mut c = if i == 0 {
                    format!("{sys}  {:>3}", codes.len())
                } else {
                    " ".repeat(6)
                };
                for code in chunk {
                    c.push_str(&format!(" {code:<3}"));
                }
                o.push_str(&hdr(&c, "SYS / # / OBS TYPES"));
            }
        }
        if let Some(i) = h.interval_s {
            o.push_str(&hdr(&format!("{i:>10.3}"), "INTERVAL"));
        }
        o.push_str(&hdr(
            &format!("{}{:5}{:<3}", epoch_fields(&h.first_obs), "", h.time_tag),
            "TIME OF FIRST OBS",
        ));
        o.push_str(&hdr("", "END OF HEADER"));
        for e in &self.epochs {
            let t = &e.time;
            o.push_str(&format!(
                "> {:04} {:02} {:02} {:02} {:02}{:>11.7}  {}{:>3}\n",
                t.year,
                t.month,
                t.day,
                t.hour,
                t.minute,
                t.second,
                e.flag,
                e.sats.len()
            ));
            for s in &e.sats {
                let mut l = format!("{:<3}", s.sat);
                for ob in &s.obs {
                    match ob {
                        Some(x) => {
                            l.push_str(&format!("{:>14.3}", x.value));
                            l.push(x.lli.map_or(' ', |d| (b'0' + d) as char));
                            l.push(x.ssi.map_or(' ', |d| (b'0' + d) as char));
                        }
                        None => l.push_str(&" ".repeat(16)),
                    }
                }
                o.push_str(l.trim_end());
                o.push('\n');
            }
        }
        o
    }

    /// Read a LUNEX file: the lunar header records here, the RINEX body through the open
    /// engine's parser.
    pub fn parse(text: &str) -> Result<Lunex, String> {
        let rinex = kshana::rinex_obs::parse_obs(text)?;
        let mut h = Header {
            program: String::new(),
            run_by: String::new(),
            date: String::new(),
            marker: String::new(),
            platform: String::new(),
            frame: String::new(),
            approx_xyz: rinex.header.approx_xyz,
            time_tag: String::new(),
            lunar_scale: None,
            interval_s: rinex.header.interval_s,
            obs_types: rinex.header.obs_types.clone(),
            first_obs: rinex
                .header
                .time_of_first_obs
                .ok_or("no TIME OF FIRST OBS")?,
            comments: Vec::new(),
        };
        let mut lunex = false;
        for line in text.lines() {
            let label = line.get(60..).unwrap_or("").trim_end();
            let c = line.get(..60.min(line.len())).unwrap_or("");
            match label {
                "LUNEX VERSION / TYPE" => lunex = true,
                "PGM / RUN BY / DATE" => {
                    h.program = col(c, 0, 20);
                    h.run_by = col(c, 20, 40);
                    h.date = col(c, 40, 60);
                }
                "COMMENT" => {
                    let t = c.trim_end().to_string();
                    if !t.starts_with("LUNEX prototype proposal")
                        && !t.starts_with("not a standard;")
                    {
                        h.comments.push(t);
                    }
                }
                "MARKER NAME" => h.marker = c.trim().to_string(),
                "LUNAR PLATFORM" => h.platform = c.trim().to_string(),
                "LUNAR REF FRAME" => h.frame = c.trim().to_string(),
                "LUNAR TIME SCALE" => {
                    let tag = col(c, 0, 4);
                    let w0 = col(c, 4, 20);
                    let w0 = if w0.is_empty() {
                        None
                    } else {
                        Some(w0.parse::<f64>().map_err(|_| format!("bad W0_L '{w0}'"))?)
                    };
                    h.lunar_scale = Some((tag, w0, col(c, 20, 28)));
                }
                "TIME OF FIRST OBS" => h.time_tag = col(c, 48, 51),
                "END OF HEADER" => break,
                _ => {}
            }
        }
        if !lunex {
            return Err("not a LUNEX file (no LUNEX VERSION / TYPE record)".into());
        }
        Ok(Lunex {
            header: h,
            epochs: rinex.epochs,
        })
    }
}

/// Seconds since J2000 of a calendar epoch (precise to well below a microsecond).
pub fn secs(t: &EpochUtc) -> f64 {
    let day = kshana::timescales::julian_date(t.year, t.month, t.day, 0, 0, 0.0);
    (day - kshana::timescales::JD_J2000) * 86_400.0
        + t.hour as f64 * 3_600.0
        + t.minute as f64 * 60.0
        + t.second
}

fn col(s: &str, a: usize, b: usize) -> String {
    s.get(a.min(s.len())..b.min(s.len()))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// A validation finding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub error: bool,
    pub line: usize,
    pub message: String,
}

/// The validation result.
#[derive(Clone, Debug, Serialize)]
pub struct Validation {
    pub valid: bool,
    pub findings: Vec<Finding>,
    pub epochs: usize,
    pub status: &'static str,
}

fn err(code: &'static str, line: usize, message: String) -> Finding {
    Finding {
        code,
        error: true,
        line,
        message,
    }
}

/// The plausible range of an observable, by system and code type.
fn plausible(sys: char, code: &str, v: f64) -> Result<(), String> {
    let (lo, hi, what) = match (code.as_bytes().first(), sys) {
        // Earth GNSS seen from the Moon: ≈ 356 000 km to ≈ 450 000 km.
        (Some(b'C'), 'L') => (0.0, 1.0e8, "AFS pseudorange (m)"),
        (Some(b'C'), _) => (3.0e8, 4.6e8, "Earth-GNSS pseudorange from the Moon (m)"),
        (Some(b'S'), _) => (0.0, 70.0, "C/N0 (dB-Hz)"),
        (Some(b'D'), _) => (-2.0e4, 2.0e4, "Doppler (Hz)"),
        _ => return Ok(()),
    };
    if v < lo || v > hi {
        return Err(format!(
            "{code} = {v} outside the plausible {what} range [{lo}, {hi}]"
        ));
    }
    Ok(())
}

/// Validate a LUNEX text: structure, the proposal's records and tags, epoch order and counts,
/// and plausible observable ranges.
pub fn validate(text: &str) -> Validation {
    let mut f = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut end = None;
    let mut seen = BTreeMap::new();
    for (i, l) in lines.iter().enumerate() {
        if l.chars().count() > 80 {
            f.push(err(
                "E005",
                i + 1,
                "header line longer than 80 columns".into(),
            ));
        }
        let label = l.get(60..).unwrap_or("").trim_end();
        if !label.is_empty() {
            *seen.entry(label.to_string()).or_insert(0) += 1;
        }
        if label == "END OF HEADER" {
            end = Some(i);
            break;
        }
    }
    let Some(end) = end else {
        f.push(err("E000", 0, "no END OF HEADER".into()));
        return Validation {
            valid: false,
            findings: f,
            epochs: 0,
            status: STATUS,
        };
    };
    for req in [
        "RINEX VERSION / TYPE",
        "LUNEX VERSION / TYPE",
        "LUNAR PLATFORM",
        "LUNAR REF FRAME",
        "SYS / # / OBS TYPES",
        "TIME OF FIRST OBS",
    ] {
        if !seen.contains_key(req) {
            let code = if req == "LUNEX VERSION / TYPE" {
                "E001"
            } else {
                "E012"
            };
            f.push(err(code, 0, format!("missing header record {req}")));
        }
    }
    let parsed = match Lunex::parse(text) {
        Ok(p) => Some(p),
        Err(e) => {
            if !f.iter().any(|x| x.code == "E001") {
                f.push(err("E013", 0, format!("does not parse: {e}")));
            }
            None
        }
    };
    let mut n_epochs = 0;
    if let Some(p) = &parsed {
        let h = &p.header;
        if !TIME_TAGS.contains(&h.time_tag.as_str()) {
            f.push(err(
                "E002",
                0,
                format!(
                    "time-system tag '{}' is not one of {TIME_TAGS:?}",
                    h.time_tag
                ),
            ));
        }
        if LUNAR_TAGS.contains(&h.time_tag.as_str()) {
            match &h.lunar_scale {
                None => f.push(err(
                    "E003",
                    0,
                    format!("tag {} needs a LUNAR TIME SCALE record", h.time_tag),
                )),
                Some((tag, w0, _)) => {
                    if tag != &h.time_tag {
                        f.push(err(
                            "E003",
                            0,
                            format!(
                                "LUNAR TIME SCALE tag {tag} does not match the epochs' {}",
                                h.time_tag
                            ),
                        ));
                    }
                    if tag == "TL" && !w0.is_some_and(|w| w > 2.0e6 && w < 3.5e6) {
                        f.push(err("E003", 0, "TL needs W0_L (m2/s2, about 2.82e6)".into()));
                    }
                }
            }
        }
        if !FRAMES.contains(&h.frame.as_str()) && seen.contains_key("LUNAR REF FRAME") {
            f.push(err("E004", 0, format!("unknown lunar frame '{}'", h.frame)));
        }
        if !PLATFORMS.contains(&h.platform.as_str()) && seen.contains_key("LUNAR PLATFORM") {
            f.push(err("E011", 0, format!("unknown platform '{}'", h.platform)));
        }
        n_epochs = p.epochs.len();
        if let Some(first) = p.epochs.first() {
            if (secs(&first.time) - secs(&h.first_obs)).abs() > 1e-6 {
                f.push(err(
                    "E010",
                    0,
                    "TIME OF FIRST OBS differs from the first epoch".into(),
                ));
            }
        }
        for (k, w) in p.epochs.windows(2).enumerate() {
            let d = secs(&w[1].time) - secs(&w[0].time);
            if d <= 0.0 {
                f.push(err(
                    "E007",
                    0,
                    format!("epoch {} is not after epoch {}", k + 2, k + 1),
                ));
            } else if let Some(iv) = h.interval_s {
                let r = d / iv;
                if (r - r.round()).abs() > 1e-6 {
                    f.push(Finding {
                        code: "W101",
                        error: false,
                        line: 0,
                        message: format!(
                            "epoch spacing {d} s is not a multiple of INTERVAL {iv} s"
                        ),
                    });
                }
            }
        }
        for (k, e) in p.epochs.iter().enumerate() {
            for s in &e.sats {
                let sys = s.sat.chars().next().unwrap_or(' ');
                let Some((_, codes)) = h.obs_types.iter().find(|(c, _)| *c == sys) else {
                    f.push(err(
                        "E006",
                        0,
                        format!(
                            "satellite {} (epoch {}) has no SYS / # / OBS TYPES",
                            s.sat,
                            k + 1
                        ),
                    ));
                    continue;
                };
                for (code, ob) in codes.iter().zip(&s.obs) {
                    if let Some(o) = ob {
                        if let Err(m) = plausible(sys, code, o.value) {
                            f.push(err("E008", 0, format!("{} epoch {}: {m}", s.sat, k + 1)));
                        }
                    }
                }
            }
        }
    }
    // Epoch record counts, from the text (the parser trusts the count).
    let mut i = end + 1;
    while i < lines.len() {
        let l = lines[i];
        if let Some(rest) = l.strip_prefix('>') {
            let n: usize = rest
                .get(31..34)
                .and_then(|x| x.trim().parse().ok())
                .unwrap_or(usize::MAX);
            let mut k = 0;
            while i + 1 + k < lines.len() && !lines[i + 1 + k].starts_with('>') {
                k += 1;
            }
            if n != k {
                f.push(err(
                    "E009",
                    i + 1,
                    format!("epoch declares {n} satellites, {k} records follow"),
                ));
            }
            i += 1 + k;
        } else {
            f.push(err("E014", i + 1, "line outside any epoch".into()));
            i += 1;
        }
    }
    Validation {
        valid: !f.iter().any(|x| x.error),
        findings: f,
        epochs: n_epochs,
        status: STATUS,
    }
}

/// Converter options.
#[derive(Clone, Debug)]
pub struct CsvOptions {
    pub marker: String,
    pub platform: String,
    pub frame: String,
    pub approx_xyz: Option<[f64; 3]>,
    pub time_tag: String,
    pub lunar_scale: Option<(String, Option<f64>, String)>,
    pub interval_s: Option<f64>,
    pub date: String,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            marker: "LUNAR-RX".into(),
            platform: "LANDER".into(),
            frame: "MOON_PA_DE440".into(),
            approx_xyz: None,
            time_tag: "GPS".into(),
            lunar_scale: None,
            interval_s: None,
            date: "19700101 000000 UTC".into(),
        }
    }
}

fn parse_time(s: &str) -> Result<EpochUtc, String> {
    let bad = || format!("bad time '{s}' (YYYY-MM-DDTHH:MM:SS[.f])");
    let (d, t) = s
        .trim()
        .trim_end_matches('Z')
        .split_once('T')
        .ok_or_else(bad)?;
    let dp: Vec<&str> = d.split('-').collect();
    let tp: Vec<&str> = t.split(':').collect();
    if dp.len() != 3 || tp.len() != 3 {
        return Err(bad());
    }
    Ok(EpochUtc {
        year: dp[0].parse().map_err(|_| bad())?,
        month: dp[1].parse().map_err(|_| bad())?,
        day: dp[2].parse().map_err(|_| bad())?,
        hour: tp[0].parse().map_err(|_| bad())?,
        minute: tp[1].parse().map_err(|_| bad())?,
        second: tp[2].parse().map_err(|_| bad())?,
    })
}

/// Convert a LuGRE-style CSV (`time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz`;
/// `#` comments and a header line allowed; an empty field is a missing observation) to LUNEX.
/// For each system the observation codes are `C<sig> L<sig> D<sig> S<sig>` for each signal in
/// order of first appearance.
pub fn from_csv(text: &str, opt: &CsvOptions) -> Result<Lunex, String> {
    struct Row {
        t: EpochUtc,
        key: String,
        sv: String,
        sig: String,
        v: [Option<f64>; 4],
    }
    let mut rows = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("time,") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() != 7 {
            return Err(format!(
                "line {}: expected 7 fields, found {}",
                i + 1,
                f.len()
            ));
        }
        let sv = f[1].to_string();
        if sv.len() != 3
            || !sv.as_bytes()[0].is_ascii_uppercase()
            || sv[1..].parse::<u32>().is_err()
        {
            return Err(format!(
                "line {}: bad satellite '{sv}' (e.g. G05, E11, L01)",
                i + 1
            ));
        }
        let sig = f[2].to_string();
        if sig.len() != 2 {
            return Err(format!(
                "line {}: bad signal '{sig}' (band digit + attribute, e.g. 1C)",
                i + 1
            ));
        }
        let mut v = [None; 4];
        for k in 0..4 {
            if !f[3 + k].is_empty() {
                v[k] = Some(
                    f[3 + k]
                        .parse::<f64>()
                        .map_err(|_| format!("line {}: bad number '{}'", i + 1, f[3 + k]))?,
                );
            }
        }
        rows.push(Row {
            t: parse_time(f[0])?,
            key: f[0].to_string(),
            sv,
            sig,
            v,
        });
    }
    if rows.is_empty() {
        return Err("no observations".into());
    }
    // Observation codes per system.
    let mut obs_types: Vec<(char, Vec<String>)> = Vec::new();
    for r in &rows {
        let sys = r.sv.as_bytes()[0] as char;
        let idx = match obs_types.iter().position(|(c, _)| *c == sys) {
            Some(i) => i,
            None => {
                obs_types.push((sys, Vec::new()));
                obs_types.len() - 1
            }
        };
        let codes = &mut obs_types[idx].1;
        if !codes.iter().any(|c| c[1..] == r.sig) {
            for p in ["C", "L", "D", "S"] {
                codes.push(format!("{p}{}", r.sig));
            }
        }
    }
    // Epochs in time order; satellites in first-seen order.
    let mut order: Vec<(f64, String)> = Vec::new();
    let mut by_epoch: BTreeMap<String, (EpochUtc, Vec<(String, String, [Option<f64>; 4])>)> =
        BTreeMap::new();
    for r in rows {
        let e = by_epoch.entry(r.key.clone()).or_insert_with(|| {
            order.push((secs(&r.t), r.key.clone()));
            (r.t, Vec::new())
        });
        e.1.push((r.sv, r.sig, r.v));
    }
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut epochs = Vec::new();
    for (_, key) in &order {
        let (t, recs) = &by_epoch[key];
        let mut sats: Vec<SatObs> = Vec::new();
        for (sv, sig, v) in recs {
            let sys = sv.as_bytes()[0] as char;
            let codes = &obs_types
                .iter()
                .find(|(c, _)| *c == sys)
                .expect("declared")
                .1;
            let si = match sats.iter().position(|s| &s.sat == sv) {
                Some(i) => i,
                None => {
                    sats.push(SatObs {
                        sat: sv.clone(),
                        obs: vec![None; codes.len()],
                    });
                    sats.len() - 1
                }
            };
            for (k, p) in ["C", "L", "D", "S"].iter().enumerate() {
                let code = format!("{p}{sig}");
                let j = codes.iter().position(|c| *c == code).expect("declared");
                if let Some(x) = v[k] {
                    if sats[si].obs[j].is_some() {
                        return Err(format!("duplicate {code} for {sv} at {key}"));
                    }
                    sats[si].obs[j] = Some(Observation {
                        value: x,
                        lli: None,
                        ssi: None,
                    });
                }
            }
        }
        epochs.push(ObsEpoch {
            time: *t,
            flag: 0,
            sats,
        });
    }
    Ok(Lunex {
        header: Header {
            program: "kshana-pro lunex".into(),
            run_by: "".into(),
            date: opt.date.clone(),
            marker: opt.marker.clone(),
            platform: opt.platform.clone(),
            frame: opt.frame.clone(),
            approx_xyz: opt.approx_xyz,
            time_tag: opt.time_tag.clone(),
            lunar_scale: opt.lunar_scale.clone(),
            interval_s: opt.interval_s,
            obs_types,
            first_obs: epochs[0].time,
            comments: vec![format!(
                "converted from a LuGRE-style CSV ({} epochs)",
                epochs.len()
            )],
        },
        epochs,
    })
}
