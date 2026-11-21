// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Time: one integer picosecond timeline.
//!
//! * [`Ps`] is a **relative** time (a duration, or a time from the scene start) in whole
//!   picoseconds, `i64` (± 106 days).
//! * [`AbsTime`] is an **absolute** instant in picoseconds of TAI since the Loran epoch
//!   1958-01-01T00:00:00 (TAI), `i128`. GRI group epochs are multiples of the GRI from that
//!   epoch (the Loran time-of-coincidence convention); see [`AbsTime::from_utc_iso`].
//!
//! Every delay in the simulator is carried in picoseconds and applied in continuous time
//! (the waveform is evaluated analytically at each sample instant), so timing resolution
//! never depends on the sample rate.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Picoseconds per second.
pub const PS_PER_S: i64 = 1_000_000_000_000;
/// Picoseconds per microsecond.
pub const PS_PER_US: i64 = 1_000_000;
/// Picoseconds per nanosecond.
pub const PS_PER_NS: i64 = 1_000;

/// A relative time in whole picoseconds.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Ps(pub i64);

impl Ps {
    pub const ZERO: Ps = Ps(0);
    /// From seconds (rounded to the nearest picosecond).
    pub fn from_s(s: f64) -> Ps {
        Ps((s * PS_PER_S as f64).round() as i64)
    }
    /// From microseconds.
    pub fn from_us(us: f64) -> Ps {
        Ps((us * PS_PER_US as f64).round() as i64)
    }
    /// From nanoseconds.
    pub fn from_ns(ns: f64) -> Ps {
        Ps((ns * PS_PER_NS as f64).round() as i64)
    }
    pub fn as_s(self) -> f64 {
        self.0 as f64 / PS_PER_S as f64
    }
    pub fn as_us(self) -> f64 {
        self.0 as f64 / PS_PER_US as f64
    }
    pub fn as_ns(self) -> f64 {
        self.0 as f64 / PS_PER_NS as f64
    }
}

impl Add for Ps {
    type Output = Ps;
    fn add(self, o: Ps) -> Ps {
        Ps(self.0 + o.0)
    }
}
impl Sub for Ps {
    type Output = Ps;
    fn sub(self, o: Ps) -> Ps {
        Ps(self.0 - o.0)
    }
}
impl AddAssign for Ps {
    fn add_assign(&mut self, o: Ps) {
        self.0 += o.0
    }
}
impl SubAssign for Ps {
    fn sub_assign(&mut self, o: Ps) {
        self.0 -= o.0
    }
}
impl Neg for Ps {
    type Output = Ps;
    fn neg(self) -> Ps {
        Ps(-self.0)
    }
}
impl Mul<i64> for Ps {
    type Output = Ps;
    fn mul(self, k: i64) -> Ps {
        Ps(self.0 * k)
    }
}

/// An absolute instant: picoseconds of TAI since 1958-01-01T00:00:00 TAI (the Loran epoch).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct AbsTime(pub i128);

impl AbsTime {
    pub fn add_ps(self, d: Ps) -> AbsTime {
        AbsTime(self.0 + d.0 as i128)
    }
    /// Relative time from `origin` to `self` (panics on overflow beyond ± 106 days).
    pub fn since(self, origin: AbsTime) -> Ps {
        let d = self.0 - origin.0;
        assert!(d.abs() <= i64::MAX as i128, "relative time out of range");
        Ps(d as i64)
    }
    /// Whole TAI seconds since the Loran epoch (floor).
    pub fn tai_seconds(self) -> i128 {
        self.0.div_euclid(PS_PER_S as i128)
    }

    /// Parse a UTC instant `YYYY-MM-DDTHH:MM:SS[.fff]Z` and convert it to TAI since 1958
    /// with the leap-second table [`tai_minus_utc_s`].
    pub fn from_utc_iso(s: &str) -> Result<AbsTime, String> {
        let s = s.trim().trim_end_matches('Z');
        let (date, time) = s
            .split_once('T')
            .ok_or_else(|| format!("expected YYYY-MM-DDTHH:MM:SSZ, got {s}"))?;
        let d: Vec<&str> = date.split('-').collect();
        let t: Vec<&str> = time.split(':').collect();
        if d.len() != 3 || t.len() != 3 {
            return Err(format!("expected YYYY-MM-DDTHH:MM:SSZ, got {s}"));
        }
        let p = |x: &str| x.parse::<i64>().map_err(|e| format!("{x}: {e}"));
        let (y, mo, da) = (p(d[0])?, p(d[1])?, p(d[2])?);
        let (h, mi) = (p(t[0])?, p(t[1])?);
        let sec: f64 = t[2].parse().map_err(|e| format!("{}: {e}", t[2]))?;
        if !(1..=12).contains(&mo) || da < 1 || da > days_in_month(y, mo) || h > 23 || mi > 59 {
            return Err(format!("date or time out of range: {s}"));
        }
        if !(0.0..61.0).contains(&sec) {
            return Err(format!("seconds out of range: {s}"));
        }
        let days = days_from_civil(y, mo, da) - days_from_civil(1958, 1, 1);
        let whole = days as i128 * 86_400 + h as i128 * 3_600 + mi as i128 * 60;
        let leap = tai_minus_utc_s(y, mo, da) as i128;
        // integer seconds exactly; only the fractional second goes through f64
        let ps = (whole + leap) * PS_PER_S as i128 + (sec * PS_PER_S as f64).round() as i128;
        Ok(AbsTime(ps))
    }
}

/// UTC calendar position of an absolute instant (see [`AbsTime::utc_day_hour`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UtcDayHour {
    pub year: i64,
    /// Day of year with fraction: 1.0 = 1 January 00:00 UTC.
    pub day_of_year: f64,
    /// UTC hour of day, `0 ≤ h < 24`.
    pub hour_utc: f64,
}

impl AbsTime {
    /// The UTC year, fractional day of year (1.0 = 1 January 00:00) and hour of this
    /// instant, removing TAI − UTC with [`tai_minus_utc_s`] (an instant inside a leap
    /// second reads as the following second). Used by the propagation models to derive
    /// solar and seasonal terms from the scene start.
    pub fn utc_day_hour(self) -> UtcDayHour {
        let ps = PS_PER_S as i128;
        let tai_s = self.0.div_euclid(ps);
        let frac = self.0.rem_euclid(ps) as f64 / PS_PER_S as f64;
        let epoch = days_from_civil(1958, 1, 1) as i128;
        // date from TAI, remove that date's TAI − UTC, then once more with the UTC date
        let leap_of = |s: i128| {
            let (y, m, d) = civil_from_days((s.div_euclid(86_400) + epoch) as i64);
            tai_minus_utc_s(y, m, d) as i128
        };
        let utc_s = tai_s - leap_of(tai_s - leap_of(tai_s));
        let days = (utc_s.div_euclid(86_400) + epoch) as i64;
        let sod = utc_s.rem_euclid(86_400) as f64 + frac;
        let (y, _, _) = civil_from_days(days);
        let hour_utc = sod / 3600.0;
        UtcDayHour {
            year: y,
            day_of_year: (days - days_from_civil(y, 1, 1) + 1) as f64 + hour_utc / 24.0,
            hour_utc,
        }
    }
}

impl AbsTime {
    /// UTC ISO 8601 with twelve decimals (whole picoseconds), e.g.
    /// `2026-11-18T12:00:00.000000123456Z`; [`AbsTime::from_utc_iso`] reads it back exactly.
    /// An instant inside a leap second reads as the following second.
    pub fn to_utc_iso(self) -> String {
        let ps = PS_PER_S as i128;
        let tai_s = self.0.div_euclid(ps);
        let frac = self.0.rem_euclid(ps);
        let epoch = days_from_civil(1958, 1, 1) as i128;
        let leap_of = |s: i128| {
            let (y, m, d) = civil_from_days((s.div_euclid(86_400) + epoch) as i64);
            tai_minus_utc_s(y, m, d) as i128
        };
        let utc_s = tai_s - leap_of(tai_s - leap_of(tai_s));
        let (y, m, d) = civil_from_days((utc_s.div_euclid(86_400) + epoch) as i64);
        let sod = utc_s.rem_euclid(86_400);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{frac:012}Z",
            sod / 3600,
            (sod / 60) % 60,
            sod % 60
        )
    }
}

/// Proleptic Gregorian date `(y, m, d)` of a day number from 1970-01-01 (inverse of
/// [`days_from_civil`], H. Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days in month `m` (1-12) of year `y`, proleptic Gregorian (0 for a month out of range).
pub fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days from 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// TAI − UTC (s) on a UTC date, from 1972 (IERS leap-second table, last entry 2017-01-01).
/// Before 1972 the integer-second scale did not exist; 10 s is returned (the 1972 value),
/// which is adequate for the Loran epoch arithmetic this crate does.
pub fn tai_minus_utc_s(y: i64, m: i64, d: i64) -> f64 {
    // (year, month) on whose first day the new value applies.
    const TABLE: &[(i64, i64, f64)] = &[
        (1972, 1, 10.0),
        (1972, 7, 11.0),
        (1973, 1, 12.0),
        (1974, 1, 13.0),
        (1975, 1, 14.0),
        (1976, 1, 15.0),
        (1977, 1, 16.0),
        (1978, 1, 17.0),
        (1979, 1, 18.0),
        (1980, 1, 19.0),
        (1981, 7, 20.0),
        (1982, 7, 21.0),
        (1983, 7, 22.0),
        (1985, 7, 23.0),
        (1988, 1, 24.0),
        (1990, 1, 25.0),
        (1991, 1, 26.0),
        (1992, 7, 27.0),
        (1993, 7, 28.0),
        (1994, 7, 29.0),
        (1996, 1, 30.0),
        (1997, 7, 31.0),
        (1999, 1, 32.0),
        (2006, 1, 33.0),
        (2009, 1, 34.0),
        (2012, 7, 35.0),
        (2015, 7, 36.0),
        (2017, 1, 37.0),
    ];
    let _ = d;
    let mut v = 10.0;
    for &(ty, tm, val) in TABLE {
        if (y, m) >= (ty, tm) {
            v = val;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_conversions_round_trip() {
        assert_eq!(Ps::from_us(1.0).0, 1_000_000);
        assert_eq!(Ps::from_ns(10.0).0, 10_000);
        assert!((Ps::from_s(0.123456789012).as_s() - 0.123456789012).abs() < 1e-12);
    }

    #[test]
    fn loran_epoch_is_zero_and_leap_seconds_apply() {
        // 1958-01-01T00:00:00 UTC maps to 10 s of TAI with the pre-1972 convention.
        let e = AbsTime::from_utc_iso("1958-01-01T00:00:00Z").unwrap();
        assert_eq!(e.0, 10 * PS_PER_S as i128);
        let a = AbsTime::from_utc_iso("2016-12-31T00:00:00Z").unwrap();
        let b = AbsTime::from_utc_iso("2017-01-02T00:00:00Z").unwrap();
        // two days plus the 2017-01-01 leap second
        assert_eq!(b.0 - a.0, (2 * 86400 + 1) as i128 * PS_PER_S as i128);
    }

    #[test]
    fn utc_day_hour_inverts_parsing() {
        let t = AbsTime::from_utc_iso("2026-11-18T06:30:00Z").unwrap();
        let u = t.utc_day_hour();
        assert_eq!(u.year, 2026);
        assert!((u.hour_utc - 6.5).abs() < 1e-9);
        assert!(
            (u.day_of_year - (322.0 + 6.5 / 24.0)).abs() < 1e-9,
            "{}",
            u.day_of_year
        );
        let j = AbsTime::from_utc_iso("2017-01-01T00:00:00Z")
            .unwrap()
            .utc_day_hour();
        assert_eq!((j.year, j.day_of_year, j.hour_utc), (2017, 1.0, 0.0));
        let l = AbsTime::from_utc_iso("2016-12-31T23:59:59Z")
            .unwrap()
            .utc_day_hour();
        assert_eq!(l.year, 2016);
        assert!((l.hour_utc - (24.0 - 1.0 / 3600.0)).abs() < 1e-9);
        for z in [-800_000, -1, 0, 59, 365, 20_000, 1_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn utc_iso_round_trip_is_exact_to_the_picosecond() {
        for (iso, add) in [
            ("2026-11-18T12:00:00Z", 123_456_789_012i64),
            ("2016-12-31T23:59:59Z", 999_999_999_999),
            ("2024-02-29T00:00:00Z", 1),
            ("1999-01-01T00:00:00Z", 0),
        ] {
            let t = AbsTime::from_utc_iso(iso).unwrap().add_ps(Ps(add));
            let s = t.to_utc_iso();
            assert_eq!(AbsTime::from_utc_iso(&s).unwrap(), t, "{s}");
        }
        let t = AbsTime::from_utc_iso("2026-11-18T12:00:00Z")
            .unwrap()
            .add_ps(Ps(5));
        assert_eq!(t.to_utc_iso(), "2026-11-18T12:00:00.000000000005Z");
    }
}
