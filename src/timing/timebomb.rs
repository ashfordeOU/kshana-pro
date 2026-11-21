// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Time-bomb auditor for timing receivers and grandmasters (W1120 TA1)
//!
//! Research 05 (gap G7) and 07b: fielded timing equipment fails on future epochs. Telstra's
//! national outage of 8 Jul 2026 was reported as a GPS rollover-type firmware flaw after a
//! vendor warning was not acted on (secondary sources; to verify against ACMA/Telstra); a
//! 2025 bug announced a leap second on an invalid date after a long run without one; the 2019
//! week-number rollover and the 2004 rollover of a legacy GPSDO still bite. Three parts:
//!
//! * **Vectors** ([`generate`]): for each hazard epoch, files a lab replays through what it
//!   already has: `replay.nmea` (ZDA + RMC at 1 Hz) and `replay.ubx` (UBX NAV-TIMEGPS,
//!   NAV-TIMEUTC, NAV-TIMELS at 1 Hz) for serial replay into a consumer of receiver output
//!   (an NTP server's refclock, gpsd/chrony, a grandmaster or GPSDO fed by an external
//!   receiver), and `simulator.json` (start epoch in UTC and GPS time, the LNAV UTC
//!   parameters to set, and the gps-sdr-sim command line from the eLoran crate's adapter) for
//!   an RF simulator feeding a receiver's antenna input. `expected.csv` is the correct output.
//! * **Analyser** ([`analyse`]): reads what the device logged during the replay (NMEA, UBX or
//!   CSV) and reports time jumps, a week-number rollover, a century error, a 32-bit wrap, a
//!   GPS-UTC offset confusion, a leap second not applied, applied at the wrong time or
//!   announced when none is due, and stuck time.
//! * **Fleet** ([`fleet_check`]): a device inventory (vendor, model, firmware, firmware date)
//!   against the epoch hazards and a vendor-advisory table the operator extends. The seed
//!   table (`timebomb-advisories.toml`) carries only entries traceable to the research
//!   reports, each with its source and verification status, plus clearly labelled examples.
//!
//! **Labels.** Vectors are COMPUTED (the expected time follows from the calendar, the GPS
//! week arithmetic and the stated leap-second count; future leap seconds are hypothetical).
//! An analysis is only as good as the capture it reads; it is not a certification of the
//! device. Leap seconds after 2017-01-01 are not announced; every leap vector is synthetic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The licence feature of the auditor (vectors, analyser and fleet check).
pub const FEATURE: &str = "pro-timing-timebomb";
/// Format tags.
pub const VECTOR_FORMAT: &str = "kshana-pro-timebomb-vector/1";
pub const ANALYSIS_FORMAT: &str = "kshana-pro-timebomb-analysis/1";
pub const FLEET_FORMAT: &str = "kshana-pro-timebomb-fleet/1";

/// The seed advisory table, embedded.
pub const SEED_ADVISORIES: &str = include_str!("timebomb-advisories.toml");

/// 1980-01-06T00:00:00Z as Unix seconds (the GPS epoch).
pub const GPS_EPOCH_UNIX: i64 = 315_964_800;
pub const WEEK_S: i64 = 604_800;
pub const DAY_S: i64 = 86_400;
/// GPS − UTC since 2017-01-01 (no leap second announced since).
pub const GPS_MINUS_UTC_NOW: i64 = 18;
/// GPS week of the last leap second (the end of 2016-12-31 is day 7 of week 1929).
pub const LAST_LEAP_WEEK: i64 = 1929;

pub const COMPUTED_LABEL: &str = "COMPUTED: the expected output follows from the calendar, the GPS week arithmetic and the stated leap-second count; future leap seconds are hypothetical. Replay results depend on the lab set-up.";
pub const ANALYSIS_LABEL: &str = "LOG ANALYSIS: findings about the device output captured during one replay, against the vector's computed expected time. Not a certification of the device; a clean replay shows only that this vector did not expose a fault.";

// ---------------------------------------------------------------------------------------
// Time labels
// ---------------------------------------------------------------------------------------

/// A UTC time label: day number from 1970-01-01 and second of the day, 0..=86400 (86400 is
/// the inserted leap second 23:59:60).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Label {
    pub days: i64,
    pub sod: i64,
}

impl Label {
    pub fn from_unix(u: i64) -> Label {
        Label {
            days: u.div_euclid(DAY_S),
            sod: u.rem_euclid(DAY_S),
        }
    }
    /// The label as seconds (23:59:60 and the next 00:00:00 coincide).
    pub fn seconds(self) -> i64 {
        self.days * DAY_S + self.sod
    }
    pub fn ymd(self) -> (i64, i64, i64) {
        kshana_eloran::time::civil_from_days(self.days)
    }
    pub fn hms(self) -> (i64, i64, i64) {
        if self.sod >= DAY_S {
            return (23, 59, 60);
        }
        (self.sod / 3600, (self.sod / 60) % 60, self.sod % 60)
    }
    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        let (h, mi, s) = self.hms();
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    }
    /// Parse `YYYY-MM-DD[THH:MM:SS[.f]][Z]` (second 60 allowed).
    pub fn parse_iso(s: &str) -> Result<Label, String> {
        let t = s.trim().trim_end_matches('Z');
        let (date, time) = t.split_once(['T', ' ']).unwrap_or((t, "00:00:00"));
        let d: Vec<&str> = date.split('-').collect();
        let tm: Vec<&str> = time.split(':').collect();
        let bad = || format!("expected YYYY-MM-DDTHH:MM:SSZ, got '{s}'");
        if d.len() != 3 || tm.len() != 3 {
            return Err(bad());
        }
        let p = |x: &str| x.trim().parse::<i64>().map_err(|_| bad());
        let (y, mo, da, h, mi) = (p(d[0])?, p(d[1])?, p(d[2])?, p(tm[0])?, p(tm[1])?);
        let sec = p(tm[2].split('.').next().unwrap_or(""))?;
        if !(1..=12).contains(&mo)
            || da < 1
            || da > kshana_eloran::time::days_in_month(y, mo)
            || h > 23
            || mi > 59
            || sec > 60
        {
            return Err(bad());
        }
        if sec == 60 && !(h == 23 && mi == 59) {
            return Err(format!("second 60 is only valid at 23:59: '{s}'"));
        }
        Ok(Label {
            days: kshana_eloran::time::days_from_civil(y, mo, da),
            sod: h * 3600 + mi * 60 + sec,
        })
    }
}

/// GPS week and time of week of GPS seconds since the GPS epoch.
pub fn gps_week_tow(gps_s: i64) -> (i64, i64) {
    (gps_s.div_euclid(WEEK_S), gps_s.rem_euclid(WEEK_S))
}

/// The UTC Unix second at which GPS week `week` starts, with GPS − UTC = `leap`.
pub fn week_start_unix(week: i64, leap: i64) -> i64 {
    GPS_EPOCH_UNIX + week * WEEK_S - leap
}

/// The GPS week containing the UTC date `days` (from 1970-01-01) at 00:00 UTC.
pub fn gps_week_of_days(days: i64) -> i64 {
    (days * DAY_S - GPS_EPOCH_UNIX + GPS_MINUS_UTC_NOW).div_euclid(WEEK_S)
}

// ---------------------------------------------------------------------------------------
// Vector specifications
// ---------------------------------------------------------------------------------------

/// A leap-second event at the end of UTC day `day` (days from 1970-01-01).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeapEvent {
    pub day: i64,
    /// `true` inserts 23:59:60; `false` deletes 23:59:59.
    pub insert: bool,
}

/// The LNAV UTC parameters (IS-GPS-200 subframe 4 page 18) a simulator should broadcast.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LnavUtc {
    pub delta_t_ls_s: i64,
    pub delta_t_lsf_s: i64,
    /// Full week of the leap-second effectivity, and its 8-bit broadcast value.
    pub wn_lsf_full: i64,
    pub wn_lsf_8bit: i64,
    /// Day number 1..=7 of the effectivity.
    pub dn: i64,
    pub note: String,
}

/// One time-bomb vector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VectorSpec {
    pub id: String,
    pub title: String,
    pub hazard: String,
    pub source: String,
    /// The label at which the hazard bites (UTC Unix seconds as a label).
    pub boundary: Label,
    pub pre_s: i64,
    pub post_s: i64,
    /// GPS − UTC at the start of the vector.
    pub gps_minus_utc_s: i64,
    pub leap: Option<LeapEvent>,
    pub lnav: LnavUtc,
    /// The date is hypothetical (a leap second nobody has announced).
    pub synthetic: bool,
}

/// One epoch of a vector: the correct UTC label, GPS seconds and GPS − UTC.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Epoch {
    pub k: usize,
    pub utc: Label,
    pub gps_s: i64,
    pub gps_minus_utc_s: i64,
}

/// Options of the catalogue.
#[derive(Clone, Debug)]
pub struct CatalogueOptions {
    /// "Now" (UTC Unix seconds), for the next leap-ambiguity crossing.
    pub now_unix: i64,
    /// The last day of the month at whose end the synthetic leap vectors act.
    pub leap_day: Label,
    pub pre_s: i64,
    pub post_s: i64,
    /// Firmware (pivot) dates: one pivot-rollover vector each.
    pub pivots: Vec<Label>,
    /// Extra boundaries: one custom vector each.
    pub custom: Vec<Label>,
}

impl Default for CatalogueOptions {
    fn default() -> Self {
        CatalogueOptions {
            now_unix: 1_795_000_000,
            leap_day: Label::parse_iso("2027-06-30").expect("valid"),
            pre_s: 120,
            post_s: 180,
            pivots: Vec::new(),
            custom: Vec::new(),
        }
    }
}

fn lnav_now(note: &str) -> LnavUtc {
    LnavUtc {
        delta_t_ls_s: GPS_MINUS_UTC_NOW,
        delta_t_lsf_s: GPS_MINUS_UTC_NOW,
        wn_lsf_full: LAST_LEAP_WEEK,
        wn_lsf_8bit: LAST_LEAP_WEEK.rem_euclid(256),
        dn: 7,
        note: note.into(),
    }
}

/// The ids of the built-in vectors.
pub const BUILTIN_IDS: [&str; 8] = [
    "gps-wnro-2038",
    "gps-wn8192",
    "ntp-era-2036",
    "unix-2038",
    "rmc-century-2080",
    "leap-insert",
    "leap-delete",
    "lsf-ambiguity",
];

/// The next week `LAST_LEAP_WEEK + 128 n` (n >= 1) that starts after `now_unix`.
pub fn next_lsf_crossing(now_unix: i64) -> i64 {
    let mut w = LAST_LEAP_WEEK + 128;
    while week_start_unix(w, GPS_MINUS_UTC_NOW) <= now_unix {
        w += 128;
    }
    w
}

/// The vector catalogue.
pub fn catalogue(o: &CatalogueOptions) -> Vec<VectorSpec> {
    let base = |id: &str, title: &str, hazard: &str, source: &str, boundary: i64| VectorSpec {
        id: id.into(),
        title: title.into(),
        hazard: hazard.into(),
        source: source.into(),
        boundary: Label::from_unix(boundary),
        pre_s: o.pre_s,
        post_s: o.post_s,
        gps_minus_utc_s: GPS_MINUS_UTC_NOW,
        leap: None,
        lnav: lnav_now("current parameters: no leap second pending"),
        synthetic: false,
    };
    let mut v = vec![
        base(
            "gps-wnro-2038",
            "GPS 10-bit week-number rollover (week 3072)",
            "LNAV broadcasts the week modulo 1024; a receiver without a correct resolution falls back 1024 weeks (19.6 years) at the start of week 3072",
            "IS-GPS-200 (10-bit WN); rollovers of 1999 and 2019 (research 05 [83][87][88])",
            week_start_unix(3072, GPS_MINUS_UTC_NOW),
        ),
        base(
            "gps-wn8192",
            "GPS 13-bit week-number rollover (week 8192)",
            "CNAV broadcasts the week modulo 8192; a 13-bit counter wraps at week 8192",
            "IS-GPS-200 (CNAV 13-bit WN)",
            week_start_unix(8192, GPS_MINUS_UTC_NOW),
        ),
        base(
            "ntp-era-2036",
            "NTP era 0 ends (2036-02-07T06:28:16Z)",
            "the 32-bit seconds field of an NTP timestamp wraps; software without era handling jumps back 136 years",
            "RFC 5905, section 6 (eras); research 05 [102]",
            (1i64 << 32) - 2_208_988_800,
        ),
        base(
            "unix-2038",
            "Signed 32-bit Unix time overflows (2038-01-19T03:14:08Z)",
            "a signed 32-bit time_t wraps to 1901",
            "POSIX time_t; research 05 [102]",
            1i64 << 31,
        ),
        base(
            "rmc-century-2080",
            "Two-digit year of NMEA RMC crosses a parser pivot (2080-01-01)",
            "RMC gives the year as two digits; a parser pivoting at 80 reads 2080 as 1980",
            "NMEA 0183 RMC date field ddmmyy",
            kshana_eloran::time::days_from_civil(2080, 1, 1) * DAY_S,
        ),
    ];
    let ld = o.leap_day;
    let lw = gps_week_of_days(ld.days);
    let ldn = (ld.days * DAY_S - GPS_EPOCH_UNIX + GPS_MINUS_UTC_NOW).rem_euclid(WEEK_S) / DAY_S + 1;
    for insert in [true, false] {
        let delta = if insert { 1 } else { -1 };
        v.push(VectorSpec {
            id: if insert { "leap-insert" } else { "leap-delete" }.into(),
            title: format!(
                "Synthetic {} leap second at the end of {}",
                if insert { "positive" } else { "negative" },
                ld.iso().get(..10).unwrap_or_default()
            ),
            hazard: if insert {
                "23:59:60 must appear and GPS - UTC must step from 18 s to 19 s; firmware that drops, repeats or mistimes it steps the output by 1 s".into()
            } else {
                "23:59:59 must be skipped and GPS - UTC must step from 18 s to 17 s; a negative leap second has never occurred and is rarely tested".into()
            },
            source: "IS-GPS-200 UTC parameters; ITU-R TF.460-6 leap seconds (hypothetical date: none is announced)".into(),
            boundary: Label {
                days: ld.days + 1,
                sod: 0,
            },
            pre_s: o.pre_s,
            post_s: o.post_s,
            gps_minus_utc_s: GPS_MINUS_UTC_NOW,
            leap: Some(LeapEvent {
                day: ld.days,
                insert,
            }),
            lnav: LnavUtc {
                delta_t_ls_s: GPS_MINUS_UTC_NOW,
                delta_t_lsf_s: GPS_MINUS_UTC_NOW + delta,
                wn_lsf_full: lw,
                wn_lsf_8bit: lw.rem_euclid(256),
                dn: ldn,
                note: "set these before the start; after the event the broadcast delta_t_ls becomes delta_t_lsf".into(),
            },
            synthetic: true,
        });
    }
    let w = next_lsf_crossing(o.now_unix);
    let mut lsf = base(
        "lsf-ambiguity",
        &format!(
            "Leap-second week ambiguity: {} weeks since the last leap second (week {w})",
            w - LAST_LEAP_WEEK
        ),
        "WN_LSF is broadcast as 8 bits; every 128 weeks without a leap second the week of the last one becomes ambiguous, and firmware that resolves it wrongly announces a leap second that is not due",
        "IS-GPS-200 (8-bit WN_LSF); time-nuts, Nov 2025: a bug triggered after more than 256 weeks without a leap second (research 05 [81])",
        week_start_unix(w, GPS_MINUS_UTC_NOW),
    );
    lsf.lnav = lnav_now("no leap second is due: correct output stays continuous; WN_LSF is the 2016-12-31 event's week, broadcast modulo 256");
    v.push(lsf);
    for p in &o.pivots {
        let pw = gps_week_of_days(p.days);
        let mut s = base(
            &format!("pivot-rollover-{}", p.iso().get(..10).unwrap_or_default()),
            &format!(
                "Firmware-pivot rollover: 1024 weeks after week {pw} (firmware date {})",
                p.iso().get(..10).unwrap_or_default()
            ),
            "a receiver that resolves the 10-bit week against a fixed pivot (often its firmware build week) falls back 1024 weeks when the true week reaches pivot + 1024",
            "IS-GPS-200 (10-bit WN); the March 2004 rollover of a legacy GPSDO (time-nuts, Nov 2024; research 05 [82])",
            week_start_unix(pw + 1024, GPS_MINUS_UTC_NOW),
        );
        s.lnav = lnav_now("current parameters");
        v.push(s);
    }
    for c in &o.custom {
        let mut s = base(
            &format!("custom-{}", c.iso().replace(':', "")),
            &format!("Custom epoch {}", c.iso()),
            "an epoch the operator chose (a vendor advisory's trigger date, a contract date)",
            "operator",
            c.seconds(),
        );
        s.lnav = lnav_now("current parameters");
        v.push(s);
    }
    v
}

impl VectorSpec {
    /// The epochs of the vector at 1 Hz, from `boundary - pre_s` to `boundary + post_s`.
    pub fn epochs(&self) -> Vec<Epoch> {
        let start = self.boundary.seconds() - self.pre_s;
        let mut lab = Label::from_unix(start);
        let mut leap = self.gps_minus_utc_s;
        let mut gps = start - GPS_EPOCH_UNIX + leap;
        let n = (self.pre_s + self.post_s).max(1) as usize;
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            out.push(Epoch {
                k,
                utc: lab,
                gps_s: gps,
                gps_minus_utc_s: leap,
            });
            gps += 1;
            lab = match self.leap {
                Some(LeapEvent { day, insert: true })
                    if lab.days == day && lab.sod == DAY_S - 1 =>
                {
                    Label {
                        days: day,
                        sod: DAY_S,
                    }
                }
                Some(LeapEvent { day, insert: true }) if lab.days == day && lab.sod == DAY_S => {
                    leap += 1;
                    Label {
                        days: day + 1,
                        sod: 0,
                    }
                }
                Some(LeapEvent { day, insert: false })
                    if lab.days == day && lab.sod == DAY_S - 2 =>
                {
                    leap -= 1;
                    Label {
                        days: day + 1,
                        sod: 0,
                    }
                }
                _ if lab.sod >= DAY_S - 1 => Label {
                    days: lab.days + 1,
                    sod: 0,
                },
                _ => Label {
                    days: lab.days,
                    sod: lab.sod + 1,
                },
            };
        }
        out
    }

    /// The index of the first epoch at or after the leap event: the epoch after 23:59:59 for an
    /// insertion (23:59:60), the epoch after 23:59:58 for a deletion (00:00:00 of the next day,
    /// 23:59:59 being skipped).
    pub fn leap_index(&self) -> Option<usize> {
        let l = self.leap?;
        let last = if l.insert { DAY_S - 1 } else { DAY_S - 2 };
        self.epochs()
            .iter()
            .position(|e| e.utc.days == l.day && e.utc.sod == last)
            .map(|i| i + 1)
    }
}

// ---------------------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------------------

/// The lab position written into RMC (no effect on time; Greenwich by default).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            lat_deg: 51.4779,
            lon_deg: -0.0015,
            h_m: 46.0,
        }
    }
}

fn hhmmss(l: Label) -> String {
    let (h, m, s) = l.hms();
    format!("{h:02}{m:02}{s:02}.00")
}

/// ZDA + RMC for each epoch (CRLF line ends).
pub fn nmea(spec: &VectorSpec, pos: &Position) -> String {
    use kshana::nmea_synth::nmea::{frame, lat_field, lon_field};
    let (la, ns) = lat_field(pos.lat_deg);
    let (lo, ew) = lon_field(pos.lon_deg);
    let mut o = String::new();
    for e in spec.epochs() {
        let (y, mo, d) = e.utc.ymd();
        let t = hhmmss(e.utc);
        o.push_str(&frame(&format!("GPZDA,{t},{d:02},{mo:02},{y:04},00,00")));
        o.push_str("\r\n");
        o.push_str(&frame(&format!(
            "GPRMC,{t},A,{la},{ns},{lo},{ew},0.0,0.0,{d:02}{mo:02}{:02},,,A,V",
            y.rem_euclid(100)
        )));
        o.push_str("\r\n");
    }
    o
}

/// A UBX frame with its Fletcher checksum.
pub fn ubx_frame(class: u8, id: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0xB5, 0x62, class, id];
    f.extend((payload.len() as u16).to_le_bytes());
    f.extend_from_slice(payload);
    let (mut a, mut b) = (0u8, 0u8);
    for x in &f[2..] {
        a = a.wrapping_add(*x);
        b = b.wrapping_add(a);
    }
    f.push(a);
    f.push(b);
    f
}

/// NAV-TIMEGPS, NAV-TIMEUTC and NAV-TIMELS for each epoch (field layout of the u-blox M8
/// protocol description; check the protocol version of the device under test).
pub fn ubx(spec: &VectorSpec) -> Vec<u8> {
    let mut o = Vec::new();
    let leap_k = spec.leap_index();
    let eps = spec.epochs();
    let event_gps = leap_k.and_then(|k| eps.get(k)).map(|e| e.gps_s);
    for e in &eps {
        let (week, tow) = gps_week_tow(e.gps_s);
        let itow = ((tow * 1000) as u32).to_le_bytes();
        // NAV-TIMEGPS (0x01 0x20, 16 bytes).
        let mut p = Vec::with_capacity(16);
        p.extend(itow);
        p.extend(0i32.to_le_bytes());
        p.extend((week as i16).to_le_bytes());
        p.push(e.gps_minus_utc_s as i8 as u8);
        p.push(0x07);
        p.extend(20u32.to_le_bytes());
        o.extend(ubx_frame(0x01, 0x20, &p));
        // NAV-TIMEUTC (0x01 0x21, 20 bytes).
        let (y, mo, d) = e.utc.ymd();
        let (h, mi, s) = e.utc.hms();
        let mut p = Vec::with_capacity(20);
        p.extend(itow);
        p.extend(20u32.to_le_bytes());
        p.extend(0i32.to_le_bytes());
        p.extend((y as u16).to_le_bytes());
        p.extend([mo as u8, d as u8, h as u8, mi as u8, s as u8, 0x07]);
        o.extend(ubx_frame(0x01, 0x21, &p));
        // NAV-TIMELS (0x01 0x26, 24 bytes).
        let mut p = Vec::with_capacity(24);
        p.extend(itow);
        p.extend([0u8, 0, 0, 0]);
        p.push(2); // srcOfCurrLs: GPS
        p.push(e.gps_minus_utc_s as i8 as u8);
        let (src_change, change, to_event, wn, dn) = match (spec.leap, event_gps) {
            (Some(l), Some(g)) if e.gps_s < g => (
                2u8,
                if l.insert { 1i8 } else { -1 },
                (g - e.gps_s) as i32,
                spec.lnav.wn_lsf_full as u16,
                spec.lnav.dn as u16,
            ),
            _ => (0u8, 0i8, 0i32, 0u16, 0u16),
        };
        p.push(src_change);
        p.push(change as u8);
        p.extend(to_event.to_le_bytes());
        p.extend(wn.to_le_bytes());
        p.extend(dn.to_le_bytes());
        p.extend([0u8, 0, 0]);
        p.push(if src_change != 0 { 0x03 } else { 0x01 });
        o.extend(ubx_frame(0x01, 0x26, &p));
    }
    o
}

/// The expected output: `k,utc,gps_week,gps_tow_s,gps_minus_utc_s,event`.
pub fn expected_csv(spec: &VectorSpec) -> String {
    let mut o = String::from("k,utc,gps_week,gps_tow_s,gps_minus_utc_s,event\n");
    let li = spec.leap_index();
    let bk = spec.pre_s as usize;
    for e in spec.epochs() {
        let (w, tow) = gps_week_tow(e.gps_s);
        let ev = if Some(e.k) == li {
            "leap"
        } else if e.k == bk {
            "boundary"
        } else {
            ""
        };
        o.push_str(&format!(
            "{},{},{w},{tow},{},{ev}\n",
            e.k,
            e.utc.iso(),
            e.gps_minus_utc_s
        ));
    }
    o
}

/// The simulator sheet: start epochs and the gps-sdr-sim command line.
pub fn simulator_json(spec: &VectorSpec, pos: &Position) -> serde_json::Value {
    use kshana_eloran::sync::gpssdrsim::{GpsSdrSimConfig, MotionInput};
    use kshana_eloran::sync::UtcStamp;
    let eps = spec.epochs();
    let first = eps[0];
    let (w, tow) = gps_week_tow(first.gps_s);
    let start = UtcStamp {
        days: first.utc.days,
        ns_of_day: first.utc.sod.min(DAY_S - 1) * 1_000_000_000,
    };
    let cfg = GpsSdrSimConfig {
        nav_file: "brdc.nav".into(),
        output: format!("{}.bin", spec.id).into(),
        motion: MotionInput::StaticLlh {
            lat_deg: pos.lat_deg,
            lon_deg: pos.lon_deg,
            h_m: pos.h_m,
        },
        start: Some(start),
        overwrite_toc_toe: true,
        duration_s: Some((spec.pre_s + spec.post_s) as f64),
        ..Default::default()
    };
    serde_json::json!({
        "format": VECTOR_FORMAT,
        "vector": spec.id,
        "start_utc": first.utc.iso(),
        "start_gps": {"week": w, "week_mod_1024": w.rem_euclid(1024), "week_mod_8192": w.rem_euclid(8192), "tow_s": tow},
        "boundary_utc": spec.boundary.iso(),
        "duration_s": spec.pre_s + spec.post_s,
        "lnav_utc_parameters": spec.lnav,
        "position": pos,
        "gps_sdr_sim": {
            "command_line": cfg.command_line().unwrap_or_default(),
            "notes": "-T overwrites TOC/TOE with the start so any RINEX navigation file can be moved to the future epoch; gps-sdr-sim takes the UTC parameters from the RINEX header (LEAP SECONDS, DELTA-UTC) and cannot set WN_LSF or DN: use a simulator that can for the leap vectors. Check -t/-T against the build in use (eLoran crate adapter notes)."
        },
        "other_simulators": "set the scenario start to start_utc (or start_gps) and the UTC parameters above in the simulator's navigation-data settings; the Skydel, Spirent and R&S SMBV adapters of the eLoran crate take the start epoch the same way",
    })
}

/// One generated file with its hash.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VectorFile {
    pub path: String,
    pub sha256: String,
}

/// Write every vector of `specs` under `out/<id>/` and a `manifest.json`; returns the manifest.
pub fn generate(
    out: &std::path::Path,
    specs: &[VectorSpec],
    pos: &Position,
) -> Result<serde_json::Value, String> {
    let io = |p: &std::path::Path, e: std::io::Error| format!("{}: {e}", p.display());
    let mut entries = Vec::new();
    for s in specs {
        let dir = out.join(&s.id);
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        let files: Vec<(&str, Vec<u8>)> = vec![
            (
                "vector.json",
                serde_json::to_vec_pretty(&serde_json::json!({
                    "format": VECTOR_FORMAT,
                    "label": COMPUTED_LABEL,
                    "spec": s,
                    "position": pos,
                }))
                .expect("serialises"),
            ),
            ("replay.nmea", nmea(s, pos).into_bytes()),
            ("replay.ubx", ubx(s)),
            ("expected.csv", expected_csv(s).into_bytes()),
            (
                "simulator.json",
                serde_json::to_vec_pretty(&simulator_json(s, pos)).expect("serialises"),
            ),
        ];
        let mut fl = Vec::new();
        for (name, bytes) in files {
            let p = dir.join(name);
            std::fs::write(&p, &bytes).map_err(|e| io(&p, e))?;
            fl.push(VectorFile {
                path: format!("{}/{name}", s.id),
                sha256: hex::encode(Sha256::digest(&bytes)),
            });
        }
        entries.push(serde_json::json!({
            "id": s.id, "title": s.title, "boundary_utc": s.boundary.iso(),
            "synthetic": s.synthetic, "files": fl,
        }));
    }
    let manifest = serde_json::json!({
        "format": VECTOR_FORMAT,
        "label": COMPUTED_LABEL,
        "vectors": entries,
        "replay": "Serial: send replay.nmea (or replay.ubx) to the device under test at one epoch per second (NMEA: a ZDA+RMC pair per second, 9600 8N1 unless the device needs otherwise), starting on a second boundary; capture what the device reports (NMEA out, UBX, or a CSV of lab time and device time) and run `kshana-pro timing timebomb analyse <vector dir> <capture>`. RF: see simulator.json.",
    });
    let p = out.join("manifest.json");
    std::fs::write(
        &p,
        serde_json::to_vec_pretty(&manifest).expect("serialises"),
    )
    .map_err(|e| io(&p, e))?;
    Ok(manifest)
}

/// Read a vector directory's `vector.json`.
pub fn read_vector(dir: &std::path::Path) -> Result<VectorSpec, String> {
    let p = dir.join("vector.json");
    let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?;
    if v.get("format").and_then(|x| x.as_str()) != Some(VECTOR_FORMAT) {
        return Err(format!("{}: not a time-bomb vector", p.display()));
    }
    serde_json::from_value(v["spec"].clone()).map_err(|e| format!("{}: {e}", p.display()))
}

// ---------------------------------------------------------------------------------------
// Device-log readers
// ---------------------------------------------------------------------------------------

/// One time report of the device under test.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct DeviceRecord {
    /// Lab capture time (Unix seconds), when the capture has one.
    pub lab_s: Option<f64>,
    pub utc: Label,
    /// The device flagged the time valid (RMC status A, UBX validUTC).
    pub valid: bool,
}

fn parse_lab_time(tok: &str) -> Option<f64> {
    let t = tok
        .trim()
        .trim_start_matches('[')
        .trim_end_matches([']', ',', ':']);
    if t.is_empty() {
        return None;
    }
    if let Ok(x) = t.parse::<f64>() {
        return x.is_finite().then_some(x);
    }
    let l = Label::parse_iso(t).ok()?;
    let frac = t
        .trim_end_matches('Z')
        .rsplit_once('.')
        .and_then(|(_, f)| format!("0.{f}").parse::<f64>().ok())
        .filter(|_| t.contains(':'))
        .unwrap_or(0.0);
    Some(l.seconds() as f64 + frac)
}

fn nmea_ok(sentence: &str) -> Option<&str> {
    let s = sentence.trim();
    let body = s.strip_prefix('$')?;
    match body.split_once('*') {
        Some((b, cs)) => {
            let want = u8::from_str_radix(cs.get(..2)?, 16).ok()?;
            (kshana::nmea_synth::nmea::checksum(b) == want).then_some(b)
        }
        None => Some(body),
    }
}

fn hms_field(f: &str) -> Option<(i64, i64, i64)> {
    let f = f.split('.').next()?;
    if f.len() != 6 {
        return None;
    }
    let p = |r: std::ops::Range<usize>| f.get(r)?.parse::<i64>().ok();
    Some((p(0..2)?, p(2..4)?, p(4..6)?))
}

/// Read NMEA output: ZDA when the log has any, else RMC (year read as 20yy). A line may
/// start with a lab time stamp (Unix seconds or ISO 8601) before the `$`.
pub fn read_nmea(text: &str) -> (Vec<DeviceRecord>, usize) {
    let has_zda = text.contains("ZDA,");
    let mut out = Vec::new();
    let mut skipped = 0;
    for line in text.lines() {
        let Some(i) = line.find('$') else {
            continue;
        };
        let lab = parse_lab_time(&line[..i]);
        let Some(body) = nmea_ok(&line[i..]) else {
            skipped += 1;
            continue;
        };
        let f: Vec<&str> = body.split(',').collect();
        let kind = f[0].get(2..).unwrap_or("");
        let rec = match (kind, has_zda) {
            ("ZDA", true) if f.len() >= 5 => (|| {
                let (h, m, s) = hms_field(f[1])?;
                let d = f[2].parse::<i64>().ok()?;
                let mo = f[3].parse::<i64>().ok()?;
                let y = f[4].parse::<i64>().ok()?;
                Some((y, mo, d, h, m, s, true))
            })(),
            ("RMC", false) if f.len() >= 10 => (|| {
                let (h, m, s) = hms_field(f[1])?;
                let dt = f[9];
                if dt.len() != 6 {
                    return None;
                }
                let p = |r: std::ops::Range<usize>| dt.get(r)?.parse::<i64>().ok();
                Some((2000 + p(4..6)?, p(2..4)?, p(0..2)?, h, m, s, f[2] == "A"))
            })(),
            _ => continue,
        };
        match rec {
            Some((y, mo, d, h, m, s, valid))
                if (1..=12).contains(&mo)
                    && (1..=31).contains(&d)
                    && h < 24
                    && m < 60
                    && s <= 60 =>
            {
                out.push(DeviceRecord {
                    lab_s: lab,
                    utc: Label {
                        days: kshana_eloran::time::days_from_civil(y, mo, d),
                        sod: h * 3600 + m * 60 + s,
                    },
                    valid,
                })
            }
            _ => skipped += 1,
        }
    }
    (out, skipped)
}

/// Read UBX output: NAV-TIMEUTC when present, else NAV-TIMEGPS (UTC = GPS − leapS).
pub fn read_ubx(bytes: &[u8]) -> (Vec<DeviceRecord>, usize) {
    let mut utc = Vec::new();
    let mut gps = Vec::new();
    let mut bad = 0;
    let mut i = 0;
    while i + 8 <= bytes.len() {
        if bytes[i] != 0xB5 || bytes[i + 1] != 0x62 {
            i += 1;
            continue;
        }
        let (class, id) = (bytes[i + 2], bytes[i + 3]);
        let len = u16::from_le_bytes([bytes[i + 4], bytes[i + 5]]) as usize;
        let end = i + 6 + len + 2;
        if end > bytes.len() {
            break;
        }
        let (mut a, mut b) = (0u8, 0u8);
        for x in &bytes[i + 2..i + 6 + len] {
            a = a.wrapping_add(*x);
            b = b.wrapping_add(a);
        }
        if a != bytes[end - 2] || b != bytes[end - 1] {
            bad += 1;
            i += 2;
            continue;
        }
        let p = &bytes[i + 6..i + 6 + len];
        match (class, id, len) {
            (0x01, 0x21, 20) => {
                let y = u16::from_le_bytes([p[12], p[13]]) as i64;
                let (mo, d, h, mi, s) = (
                    p[14] as i64,
                    p[15] as i64,
                    p[16] as i64,
                    p[17] as i64,
                    p[18] as i64,
                );
                if (1..=12).contains(&mo) && (1..=31).contains(&d) && h < 24 && mi < 60 && s <= 60 {
                    utc.push(DeviceRecord {
                        lab_s: None,
                        utc: Label {
                            days: kshana_eloran::time::days_from_civil(y, mo, d),
                            sod: h * 3600 + mi * 60 + s,
                        },
                        valid: p[19] & 0x04 != 0,
                    });
                } else {
                    bad += 1;
                }
            }
            (0x01, 0x20, 16) => {
                let itow = u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as i64;
                let week = i16::from_le_bytes([p[8], p[9]]) as i64;
                let leap = p[10] as i8 as i64;
                let u = GPS_EPOCH_UNIX + week * WEEK_S + itow.div_euclid(1000) - leap;
                gps.push(DeviceRecord {
                    lab_s: None,
                    utc: Label::from_unix(u),
                    valid: p[11] & 0x03 == 0x03,
                });
            }
            _ => {}
        }
        i = end;
    }
    (if utc.is_empty() { gps } else { utc }, bad)
}

/// Read a CSV capture: `[lab_time,]device_time` per line (Unix seconds or ISO 8601; second
/// 60 allowed in ISO); a header line is skipped.
pub fn read_csv(text: &str) -> (Vec<DeviceRecord>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0;
    let mut first = true;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let header = std::mem::replace(&mut first, false);
        let cols: Vec<&str> = l.split([',', ';']).map(str::trim).collect();
        let (lab, dev) = match cols.as_slice() {
            [d] => (None, *d),
            [a, d, ..] => (parse_lab_time(a), *d),
            _ => continue,
        };
        // A non-finite or absurd Unix time (NaN became 1970, -inf the minimum i64, and a later
        // subtraction overflowed) is a skipped row, not a device time.
        let utc = match dev.parse::<f64>() {
            Ok(x) if x.is_finite() && x.abs() < 1e12 => Some(Label::from_unix(x.floor() as i64)),
            Ok(_) => None,
            Err(_) => Label::parse_iso(dev).ok(),
        };
        match utc {
            Some(u) => out.push(DeviceRecord {
                lab_s: lab,
                utc: u,
                valid: true,
            }),
            // A header line is not a fault.
            None if header => {}
            None => skipped += 1,
        }
    }
    (out, skipped)
}

// ---------------------------------------------------------------------------------------
// Analyser
// ---------------------------------------------------------------------------------------

/// One fault found in the capture.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fault {
    /// `week-rollover`, `century`, `wrap-32`, `gps-utc-offset`, `leap-not-applied`,
    /// `leap-mistimed`, `unexpected-leap`, `time-jump`, `stuck-time`, `no-output`.
    pub kind: String,
    /// Index of the expected epoch where it shows.
    pub k: usize,
    pub expected_utc: String,
    pub device_utc: String,
    /// The step of the device time against the expected time (s).
    pub step_s: i64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Analysis {
    pub format: &'static str,
    pub label: &'static str,
    pub vector: String,
    pub capture_format: String,
    pub device_records: usize,
    pub unreadable_lines: usize,
    pub paired: usize,
    pub pairing: String,
    /// Device minus expected time at the first paired record (s).
    pub initial_offset_s: Option<i64>,
    pub invalid_flagged: usize,
    pub faults: Vec<Fault>,
    pub notes: Vec<String>,
    pub verdict: String,
}

const W1024: i64 = 1024 * WEEK_S;

/// Name a step of the device time.
pub fn classify_step(step: i64) -> (&'static str, String) {
    let a = step.abs();
    let near = |x: i64, tol: i64| (a - x).abs() <= tol;
    let leap_tol = 20;
    if a % W1024 <= leap_tol || W1024 - a % W1024 <= leap_tol {
        let n = (a as f64 / W1024 as f64).round() as i64;
        if n >= 1 {
            let kind = if n % 8 == 0 {
                format!("{n} x 1024 weeks ({} x 8192)", n / 8)
            } else {
                format!("{n} x 1024 weeks")
            };
            return (
                "week-rollover",
                format!(
                    "{} by {kind} ({:+} s beyond the whole weeks): the GPS week was resolved in the wrong 1024-week era",
                    if step < 0 { "back" } else { "forward" },
                    step - step.signum() * n * W1024
                ),
            );
        }
    }
    let century_days = [36_524, 36_525];
    if century_days.iter().any(|d| near(d * DAY_S, DAY_S)) {
        return (
            "century",
            format!(
                "{} by about 100 years: a two-digit year read in the wrong century",
                if step < 0 { "back" } else { "forward" }
            ),
        );
    }
    if near(1 << 32, DAY_S) || near(1 << 31, DAY_S) {
        return (
            "wrap-32",
            format!(
                "{} by 2^{} s: a 32-bit seconds counter wrapped",
                if step < 0 { "back" } else { "forward" },
                if near(1 << 32, DAY_S) { 32 } else { 31 }
            ),
        );
    }
    if (17..=19).contains(&a) {
        return (
            "gps-utc-offset",
            format!("{step:+} s: GPS time given as UTC (or the leap-second count applied twice)"),
        );
    }
    ("time-jump", format!("{step:+} s"))
}

/// How capture records are paired with the vector's epochs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pairing {
    /// The epoch of the first record (default 0: the capture starts with the replay).
    pub start_k: usize,
    /// The lab time of epoch 0, when the capture is stamped: records are then paired by
    /// `round(lab - replay_start)` and `start_k` is ignored.
    pub replay_start_lab: Option<f64>,
}

/// Analyse a capture against a vector, the capture starting with the replay.
pub fn analyse(
    spec: &VectorSpec,
    recs: &[DeviceRecord],
    capture_format: &str,
    unreadable: usize,
) -> Analysis {
    analyse_with(spec, recs, capture_format, unreadable, Pairing::default())
}

/// Analyse a capture against a vector with an explicit pairing. The device's own time never
/// sets the pairing, so a constant error (18 s, 1024 weeks) shows from the first record.
pub fn analyse_with(
    spec: &VectorSpec,
    recs: &[DeviceRecord],
    capture_format: &str,
    unreadable: usize,
    pairing_opt: Pairing,
) -> Analysis {
    let eps = spec.epochs();
    let n = eps.len();
    let mut notes = Vec::new();
    let mut faults = Vec::new();
    let invalid_flagged = recs.iter().filter(|r| !r.valid).count();
    let first = recs.first();
    let by_lab = !recs.is_empty() && recs.iter().all(|r| r.lab_s.is_some());
    let lab0 = pairing_opt
        .replay_start_lab
        .filter(|_| by_lab)
        .or(first.and_then(|r| r.lab_s))
        .unwrap_or(0.0);
    let k0 = if pairing_opt.replay_start_lab.is_some() && by_lab {
        0
    } else {
        pairing_opt.start_k
    };
    let pairing = match (by_lab, pairing_opt.replay_start_lab) {
        (true, Some(t)) => format!("by lab time stamp, epoch 0 at lab time {t}"),
        (true, None) => format!("by lab time stamp, the first record at epoch {k0}"),
        (false, _) => format!(
            "in order, one record per second, the first record at epoch {k0} (no lab time stamps)"
        ),
    };
    let mut pairs: Vec<(usize, DeviceRecord)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (i, r) in recs.iter().enumerate() {
        let k = if by_lab {
            let dk = (r.lab_s.unwrap_or(lab0) - lab0).round();
            if dk < 0.0 {
                continue;
            }
            k0 + dk as usize
        } else {
            k0 + i
        };
        if k >= n || !seen.insert(k) {
            continue;
        }
        pairs.push((k, *r));
    }
    let resid: Vec<(usize, i64, DeviceRecord)> = pairs
        .iter()
        .map(|(k, r)| (*k, r.utc.seconds() - eps[*k].utc.seconds(), *r))
        .collect();
    let leap_k = spec.leap_index();
    let near_leap = |k: usize| leap_k.is_some_and(|l| (k as i64 - l as i64).abs() <= 2);
    // Stuck time: three or more consecutive records with the same label.
    let mut stuck_mask = vec![false; resid.len()];
    let mut i = 0;
    while i < resid.len() {
        let mut j = i + 1;
        while j < resid.len() && resid[j].2.utc == resid[i].2.utc {
            j += 1;
        }
        if j - i >= 3 {
            for m in stuck_mask.iter_mut().take(j).skip(i) {
                *m = true;
            }
            faults.push(Fault {
                kind: "stuck-time".into(),
                k: resid[i].0,
                expected_utc: eps[resid[i].0].utc.iso(),
                device_utc: resid[i].2.utc.iso(),
                step_s: 0,
                detail: format!(
                    "the device repeated {} for {} records",
                    resid[i].2.utc.iso(),
                    j - i
                ),
            });
        }
        i = j;
    }
    // Steps of the residual between records outside stuck spans.
    let live: Vec<(usize, i64, DeviceRecord)> = resid
        .iter()
        .zip(&stuck_mask)
        .filter(|(_, s)| !**s)
        .map(|(r, _)| *r)
        .collect();
    let initial = live.first().map(|r| r.1);
    if let Some(r0) = live.first().filter(|r| r.1 != 0) {
        let (kind, detail) = classify_step(r0.1);
        faults.push(Fault {
            kind: kind.into(),
            k: r0.0,
            expected_utc: eps[r0.0].utc.iso(),
            device_utc: r0.2.utc.iso(),
            step_s: r0.1,
            detail: format!("from the first record: {detail}"),
        });
    }
    let mut idx = 1;
    while idx < live.len() {
        let (k, r, rec) = live[idx];
        let prev = live[idx - 1].1;
        let step = r - prev;
        if step == 0 {
            idx += 1;
            continue;
        }
        // A one-record blip of 1 s that returns at once.
        let back = live.get(idx + 1).is_some_and(|x| x.1 == prev);
        if step.abs() == 1 && back {
            if near_leap(k) && step == -1 && spec.leap.is_some_and(|l| l.insert) {
                notes.push(format!(
                    "the leap second was shown as a repeated 23:59:59 at {} (a common variant; the time after the event is right)",
                    eps[k].utc.iso()
                ));
            } else {
                notes.push(format!(
                    "a one-record {step:+} s blip at {} (capture jitter or a mislabelled second)",
                    eps[k].utc.iso()
                ));
            }
            idx += 2;
            continue;
        }
        let (kind, detail) = if step.abs() == 1 {
            match spec.leap {
                Some(l) if near_leap(k) => (
                    "leap-not-applied",
                    format!(
                        "{step:+} s at the {} leap second: the device did not {} it",
                        if l.insert { "positive" } else { "negative" },
                        if l.insert { "insert" } else { "delete" }
                    ),
                ),
                Some(_) => (
                    "leap-mistimed",
                    format!("{step:+} s away from the leap event: the leap second was applied at the wrong time"),
                ),
                None => (
                    "unexpected-leap",
                    format!("{step:+} s with no leap second due: a leap second announced or applied wrongly"),
                ),
            }
        } else {
            let (k2, d) = classify_step(step);
            (k2, d)
        };
        faults.push(Fault {
            kind: kind.into(),
            k,
            expected_utc: eps[k].utc.iso(),
            device_utc: rec.utc.iso(),
            step_s: step,
            detail,
        });
        idx += 1;
    }
    // Missing output.
    let mut ks: Vec<usize> = pairs.iter().map(|p| p.0).collect();
    ks.sort_unstable();
    for w in ks.windows(2) {
        if w[1] - w[0] > 5 {
            faults.push(Fault {
                kind: "no-output".into(),
                k: w[0] + 1,
                expected_utc: eps[w[0] + 1].utc.iso(),
                device_utc: String::new(),
                step_s: 0,
                detail: format!("no record for {} s", w[1] - w[0] - 1),
            });
        }
    }
    if pairs.len() < n / 2 {
        notes.push(format!(
            "only {} of {} epochs have a record: the capture does not cover the vector",
            pairs.len(),
            n
        ));
    }
    if let Some(l) = leap_k {
        if !ks.iter().any(|&k| (k as i64 - l as i64).abs() <= 1) {
            notes.push(
                "no record around the leap event: the leap handling is not tested by this capture"
                    .into(),
            );
        }
    }
    if invalid_flagged > 0 {
        notes.push(format!(
            "{invalid_flagged} records flagged invalid by the device"
        ));
    }
    faults.sort_by_key(|f| f.k);
    let verdict = if pairs.is_empty() {
        "no device record could be paired with the vector".to_string()
    } else if faults.is_empty() {
        "no fault seen in this replay".to_string()
    } else {
        let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
        for f in &faults {
            *kinds.entry(f.kind.as_str()).or_default() += 1;
        }
        format!(
            "{} fault(s): {}",
            faults.len(),
            kinds
                .iter()
                .map(|(k, n)| format!("{k} x{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Analysis {
        format: ANALYSIS_FORMAT,
        label: ANALYSIS_LABEL,
        vector: spec.id.clone(),
        capture_format: capture_format.into(),
        device_records: recs.len(),
        unreadable_lines: unreadable,
        paired: pairs.len(),
        pairing,
        initial_offset_s: initial,
        invalid_flagged,
        faults,
        notes,
        verdict,
    }
}

/// Read a capture in `format` (`nmea`, `ubx` or `csv`).
pub fn read_capture(format: &str, bytes: &[u8]) -> Result<(Vec<DeviceRecord>, usize), String> {
    match format {
        "nmea" => Ok(read_nmea(&String::from_utf8_lossy(bytes))),
        "ubx" => Ok(read_ubx(bytes)),
        "csv" => Ok(read_csv(&String::from_utf8_lossy(bytes))),
        o => Err(format!(
            "unknown capture format '{o}' (expected nmea, ubx or csv)"
        )),
    }
}

// ---------------------------------------------------------------------------------------
// Fleet check
// ---------------------------------------------------------------------------------------

/// One device of the inventory.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: String,
    #[serde(default)]
    pub site: Option<String>,
    pub vendor: String,
    pub model: String,
    #[serde(default)]
    pub firmware: Option<String>,
    /// `YYYY-MM-DD`: the firmware's build or release date.
    #[serde(default)]
    pub firmware_date: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    /// How the 10-bit GPS week is resolved: `pivot`, `full` (CNAV, or another source of the
    /// era) or `unknown` (default).
    #[serde(default)]
    pub week_resolution: Option<String>,
    /// Uses NTP 64-bit timestamps (default: unknown, tested).
    #[serde(default)]
    pub ntp: Option<bool>,
    /// Runs a signed 32-bit time_t (default: unknown, tested).
    #[serde(default)]
    pub time_t_32bit: Option<bool>,
    /// Reads NMEA RMC dates from an external receiver.
    #[serde(default)]
    pub nmea_input: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub devices: Vec<Device>,
}

/// One advisory of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advisory {
    pub id: String,
    /// `vendor-security-advisory`, `vendor-time-advisory`, `community-report` or `example`.
    pub kind: String,
    pub vendor: String,
    /// Model names (case-insensitive); `*` for every model of the vendor. `vendor` may list
    /// alternative names separated by `|`.
    pub models: Vec<String>,
    /// Firmware at and above this is not affected.
    #[serde(default)]
    pub fixed_in: Option<String>,
    /// Firmware below this is not affected.
    #[serde(default)]
    pub affected_from: Option<String>,
    pub hazard: String,
    /// For a time bomb: the date it acts (`YYYY-MM-DD`).
    #[serde(default)]
    pub trigger_date: Option<String>,
    /// The vector to replay for it.
    #[serde(default)]
    pub vector: Option<String>,
    pub source: String,
    /// How the entry was checked.
    pub verification: String,
    /// Hazard classes read by other checks (`unsigned-firmware`: the device does not verify
    /// its upgrade images; read by the appliance posture report).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryTable {
    #[serde(default)]
    pub advisory: Vec<Advisory>,
}

impl AdvisoryTable {
    pub fn parse(src: &str) -> Result<AdvisoryTable, String> {
        let t: AdvisoryTable = toml::from_str(src).map_err(|e| format!("advisory table: {e}"))?;
        for a in &t.advisory {
            if ![
                "vendor-security-advisory",
                "vendor-time-advisory",
                "community-report",
                "example",
            ]
            .contains(&a.kind.as_str())
            {
                return Err(format!("advisory {}: unknown kind '{}'", a.id, a.kind));
            }
            if a.kind == "example" && !a.id.starts_with("EXAMPLE-") {
                return Err(format!(
                    "advisory {}: an example's id must start with EXAMPLE-",
                    a.id
                ));
            }
            if a.id.starts_with("CVE-") && a.kind != "vendor-security-advisory" {
                return Err(format!(
                    "advisory {}: a CVE id belongs to a vendor-security-advisory",
                    a.id
                ));
            }
        }
        Ok(t)
    }
    /// The seed table.
    pub fn seed() -> AdvisoryTable {
        AdvisoryTable::parse(SEED_ADVISORIES).expect("the seed advisory table parses")
    }
}

/// Compare dotted versions numerically (non-numeric parts compared as text).
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| -> Vec<String> {
        s.trim()
            .trim_start_matches(['v', 'V'])
            .split(['.', '-', '_'])
            .map(str::to_string)
            .collect()
    };
    let (x, y) = (split(a), split(b));
    for i in 0..x.len().max(y.len()) {
        let p = x.get(i).map(String::as_str).unwrap_or("0");
        let q = y.get(i).map(String::as_str).unwrap_or("0");
        let o = match (p.parse::<u64>(), q.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            _ => p.cmp(q),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    std::cmp::Ordering::Equal
}

/// One finding for a device.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    /// The advisory id or the vector id of an epoch hazard.
    pub id: String,
    pub source_kind: String,
    /// `affected`, `review`, `test`, `not-affected`.
    pub status: String,
    /// `act`, `plan`, `watch`, `info`.
    pub priority: String,
    pub date: Option<String>,
    pub days_from_now: Option<i64>,
    pub vector: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DeviceResult {
    pub id: String,
    pub site: Option<String>,
    pub vendor: String,
    pub model: String,
    pub firmware: Option<String>,
    pub findings: Vec<Finding>,
    pub worst: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FleetReport {
    pub format: &'static str,
    pub label: String,
    pub title: Option<String>,
    pub utc: String,
    pub advisories: usize,
    pub devices: Vec<DeviceResult>,
    pub counts: BTreeMap<String, usize>,
}

fn prio_rank(p: &str) -> u8 {
    match p {
        "act" => 0,
        "plan" => 1,
        "watch" => 2,
        _ => 3,
    }
}

fn date_prio(days: i64) -> &'static str {
    if days <= 365 {
        "act"
    } else if days <= 5 * 365 {
        "plan"
    } else {
        "watch"
    }
}

/// Check every device of `inv` against the epoch hazards and `table`, at `now_unix`.
pub fn fleet_check(
    inv: &Inventory,
    table: &AdvisoryTable,
    now_unix: i64,
    utc: &str,
) -> Result<FleetReport, String> {
    let now = Label::from_unix(now_unix);
    let day = |s: &str| Label::parse_iso(s).map(|l| l.days);
    let mut out = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for d in &inv.devices {
        if d.id.trim().is_empty() || !ids.insert(d.id.clone()) {
            return Err(format!("device id '{}' is empty or repeated", d.id));
        }
        let mut f: Vec<Finding> = Vec::new();
        let mut epoch = |id: &str, kind: &str, at: i64, status: &str, detail: String| {
            let lab = Label::from_unix(at);
            let dd = lab.days - now.days;
            f.push(Finding {
                id: id.into(),
                source_kind: kind.into(),
                status: status.into(),
                priority: date_prio(dd).into(),
                date: Some(lab.iso().get(..10).unwrap_or_default().to_string()),
                days_from_now: Some(dd),
                vector: Some(id.into()),
                detail,
            });
        };
        let wr = d.week_resolution.as_deref().unwrap_or("unknown");
        if wr != "full" {
            epoch(
                "gps-wnro-2038",
                "epoch hazard",
                week_start_unix(3072, GPS_MINUS_UTC_NOW),
                "test",
                format!("10-bit week rollover; week resolution '{wr}'"),
            );
        }
        if wr != "full" {
            if let Some(fd) = &d.firmware_date {
                let fdays = day(fd).map_err(|e| format!("device {}: firmware_date: {e}", d.id))?;
                let pw = gps_week_of_days(fdays);
                let at = week_start_unix(pw + 1024, GPS_MINUS_UTC_NOW);
                let id = format!("pivot-rollover-{fd}");
                epoch(
                    &id,
                    "epoch hazard",
                    at,
                    "test",
                    format!(
                        "if the firmware resolves the week against its own date (week {pw}), it falls back 1024 weeks on this date; generate with --pivot {fd}"
                    ),
                );
            }
        }
        if d.ntp != Some(false) {
            epoch(
                "ntp-era-2036",
                "epoch hazard",
                (1i64 << 32) - 2_208_988_800,
                "test",
                format!(
                    "NTP era rollover; NTP use {}",
                    d.ntp
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "unknown".into())
                ),
            );
        }
        if d.time_t_32bit != Some(false) {
            epoch(
                "unix-2038",
                "epoch hazard",
                1i64 << 31,
                if d.time_t_32bit == Some(true) {
                    "affected"
                } else {
                    "test"
                },
                format!(
                    "signed 32-bit time_t {}",
                    match d.time_t_32bit {
                        Some(true) => "stated",
                        _ => "unknown",
                    }
                ),
            );
        }
        if d.nmea_input == Some(true) {
            epoch(
                "rmc-century-2080",
                "epoch hazard",
                kshana_eloran::time::days_from_civil(2080, 1, 1) * DAY_S,
                "test",
                "reads two-digit RMC years".into(),
            );
        }
        epoch(
            "lsf-ambiguity",
            "epoch hazard",
            week_start_unix(next_lsf_crossing(now_unix), GPS_MINUS_UTC_NOW),
            "test",
            "8-bit WN_LSF ambiguity (every 128 weeks without a leap second)".into(),
        );
        f.push(Finding {
            id: "leap-insert/leap-delete".into(),
            source_kind: "epoch hazard".into(),
            status: "test".into(),
            priority: "info".into(),
            date: None,
            days_from_now: None,
            vector: Some("leap-insert".into()),
            detail: "no leap second is announced; replay the synthetic positive and negative leap vectors once per firmware".into(),
        });
        for a in &table.advisory {
            if !a
                .vendor
                .split('|')
                .any(|v| v.trim().eq_ignore_ascii_case(d.vendor.trim()))
                || !a
                    .models
                    .iter()
                    .any(|m| m == "*" || m.eq_ignore_ascii_case(&d.model))
            {
                continue;
            }
            let fw = d.firmware.as_deref();
            let (status, why) = match (fw, &a.fixed_in, &a.affected_from) {
                (Some(v), _, Some(from)) if version_cmp(v, from).is_lt() => (
                    "not-affected",
                    format!("firmware {v} below the first affected {from}"),
                ),
                (Some(v), Some(fix), _) if version_cmp(v, fix).is_lt() => {
                    ("affected", format!("firmware {v} below the fixed {fix}"))
                }
                (Some(v), Some(fix), _) => (
                    "not-affected",
                    format!("firmware {v} at or above the fixed {fix}"),
                ),
                (None, _, _) => (
                    "review",
                    "firmware version not in the inventory".to_string(),
                ),
                (Some(v), None, _) => (
                    "review",
                    format!("firmware {v}; the table gives no fixed version: check the advisory"),
                ),
            };
            let (date, days) = match &a.trigger_date {
                Some(t) => {
                    let dd = day(t).map_err(|e| format!("advisory {}: trigger_date: {e}", a.id))?
                        - now.days;
                    (Some(t.clone()), Some(dd))
                }
                None => (None, None),
            };
            let priority = match status {
                "affected" => "act",
                "review" => days.map(date_prio).unwrap_or("plan"),
                _ => "info",
            };
            f.push(Finding {
                id: a.id.clone(),
                source_kind: a.kind.clone(),
                status: status.into(),
                priority: priority.into(),
                date,
                days_from_now: days,
                vector: a.vector.clone(),
                detail: format!(
                    "{}; {why}; source: {}; {}",
                    a.hazard, a.source, a.verification
                ),
            });
        }
        f.sort_by(|x, y| {
            prio_rank(&x.priority).cmp(&prio_rank(&y.priority)).then(
                x.days_from_now
                    .unwrap_or(i64::MAX)
                    .cmp(&y.days_from_now.unwrap_or(i64::MAX)),
            )
        });
        let worst = f
            .first()
            .map(|x| x.priority.clone())
            .unwrap_or_else(|| "info".into());
        out.push(DeviceResult {
            id: d.id.clone(),
            site: d.site.clone(),
            vendor: d.vendor.clone(),
            model: d.model.clone(),
            firmware: d.firmware.clone(),
            findings: f,
            worst,
        });
    }
    out.sort_by(|a, b| {
        prio_rank(&a.worst)
            .cmp(&prio_rank(&b.worst))
            .then(a.id.cmp(&b.id))
    });
    let mut counts = BTreeMap::new();
    for d in &out {
        *counts.entry(d.worst.clone()).or_insert(0) += 1;
    }
    Ok(FleetReport {
        format: FLEET_FORMAT,
        label: format!(
            "COMPUTED from the inventory as supplied and an advisory table of {} entries (seed entries traceable to the research reports, examples labelled EXAMPLE-); a match is a prompt to check the vendor's advisory, not a finding that a device is vulnerable.",
            table.advisory.len()
        ),
        title: inv.title.clone(),
        utc: utc.into(),
        advisories: table.advisory.len(),
        devices: out,
        counts,
    })
}

/// The fleet report as CSV, one finding per line.
pub fn fleet_csv(r: &FleetReport) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut o = String::from("device,site,vendor,model,firmware,finding,kind,status,priority,date,days_from_now,vector,detail\n");
    for d in &r.devices {
        for f in &d.findings {
            o.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                q(&d.id),
                q(d.site.as_deref().unwrap_or("")),
                q(&d.vendor),
                q(&d.model),
                q(d.firmware.as_deref().unwrap_or("")),
                q(&f.id),
                q(&f.source_kind),
                f.status,
                f.priority,
                f.date.as_deref().unwrap_or(""),
                f.days_from_now.map(|x| x.to_string()).unwrap_or_default(),
                q(f.vector.as_deref().unwrap_or("")),
                q(&f.detail)
            ));
        }
    }
    o
}
