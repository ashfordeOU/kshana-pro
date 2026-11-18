// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! External time-reference data: importers that turn published time-scale comparisons
//! into [`crate::timebase::TimeRefModel`]s, and Modified Julian Date helpers.
//!
//! * [`circular_t`]: BIPM Circular T section 1 (`UTC − UTC(k)` every 5 days).
//!
//! MJD = JD − 2 400 000.5; MJD 40587 is 1970-01-01. UTC days are taken as 86 400 s (a
//! leap second inside a day is not represented in the fractional part).

pub mod circular_t;

use crate::sync::{civil_from_days, UtcStamp};
use crate::time::days_from_civil;

/// MJD of 1970-01-01.
pub const MJD_UNIX_EPOCH: i64 = 40_587;

/// MJD of a calendar date (0h).
pub fn mjd_from_ymd(y: i64, m: i64, d: i64) -> i64 {
    days_from_civil(y, m, d) + MJD_UNIX_EPOCH
}

/// Calendar date of an integer MJD.
pub fn ymd_from_mjd(mjd: i64) -> (i64, i64, i64) {
    civil_from_days(mjd - MJD_UNIX_EPOCH)
}

/// Fractional MJD of a UTC stamp.
pub fn mjd_from_utc(u: UtcStamp) -> f64 {
    (u.days + MJD_UNIX_EPOCH) as f64 + u.ns_of_day as f64 / 86_400e9
}

/// UTC stamp of a fractional MJD (rounded to the nanosecond).
pub fn utc_from_mjd(mjd: f64) -> UtcStamp {
    let day = mjd.floor();
    UtcStamp {
        days: day as i64 - MJD_UNIX_EPOCH,
        ns_of_day: 0,
    }
    .plus((mjd - day) * 86_400.0)
}

/// Fractional MJD of `YYYY-MM-DDTHH:MM:SS[.f]Z`.
pub fn mjd_from_iso(s: &str) -> Result<f64, String> {
    Ok(mjd_from_utc(UtcStamp::parse(s)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mjd_known_dates() {
        assert_eq!(mjd_from_ymd(1858, 11, 17), 0);
        assert_eq!(mjd_from_ymd(2000, 1, 1), 51_544);
        assert_eq!(mjd_from_ymd(2026, 11, 18), 61_362);
        assert_eq!(ymd_from_mjd(51_544), (2000, 1, 1));
        assert_eq!(ymd_from_mjd(0), (1858, 11, 17));
        let m = mjd_from_iso("2026-11-18T12:00:00Z").unwrap();
        assert_eq!(m, 61_362.5);
        assert_eq!(utc_from_mjd(m).iso(), "2026-11-18T12:00:00.000000000Z");
        assert_eq!(utc_from_mjd(61_359.75).iso(), "2026-11-15T18:00:00.000000000Z");
    }
}
